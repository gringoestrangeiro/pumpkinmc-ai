use std::sync::atomic::{AtomicBool, AtomicI32, Ordering};
use std::sync::{Arc, Mutex, Weak};

use pumpkin_data::Block;
use pumpkin_data::attributes::Attributes;
use pumpkin_data::entity::EntityType;
use pumpkin_data::sound::{Sound, SoundCategory};
use pumpkin_data::tag::{self, Taggable};
use pumpkin_nbt::compound::NbtCompound;
use pumpkin_util::math::position::BlockPos;
use pumpkin_util::math::vector3::Vector3;

use crate::entity::{
    Entity, EntityBase,
    ai::goal::{
        Controls, Goal, active_target::ActiveTargetGoal, look_around::RandomLookAroundGoal,
        look_at_entity::LookAtEntityGoal, melee_attack::MeleeAttackGoal, revenge::RevengeGoal,
        swim::SwimGoal, wander_around::WanderAroundGoal,
    },
    ai::pathfinder::NavigatorGoal,
    ai::util::default_random_pos,
    mob::{Mob, MobEntity},
};
use crate::world::World;

const REPELLENT_DETECTION_RANGE_HORIZONTAL: i32 = 8;
const REPELLENT_DETECTION_RANGE_VERTICAL: i32 = 4;
const REPELLENT_PACIFY_TIME: i32 = 200;
const REPELLENT_SCAN_INTERVAL: i32 = 20;
const REPELLENT_DESIRED_DISTANCE: f64 = 8.0;
const REPELLENT_SPEED_MULTIPLIER: f64 = 1.0;
const RETREAT_DESIRED_DISTANCE: f64 = 15.0;
const RETREAT_SPEED_MULTIPLIER: f64 = 1.3;
const RETREAT_DURATION_MIN: i32 = 100;
const RETREAT_DURATION_MAX: i32 = 400;

pub struct HoglinEntity {
    pub mob_entity: MobEntity,
    pub immune_to_zombification: AtomicBool,
    pub time_in_overworld: AtomicI32,
    pub cannot_be_hunted: AtomicBool,
    pub is_baby: AtomicBool,
    nearest_repellent: Mutex<Option<BlockPos>>,
    pacified_ticks: AtomicI32,
    avoid_target: Mutex<Option<Arc<dyn EntityBase>>>,
    avoid_ticks: AtomicI32,
}

impl HoglinEntity {
    pub const CONVERSION_TIME: i32 = 300;
    pub const XP_REWARD: u32 = 5;

    pub fn new(entity: Entity) -> Arc<Self> {
        let mob_entity = MobEntity::new(entity);
        {
            let mut attributes = mob_entity
                .living_entity
                .attributes
                .write()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            if let Some(health) = attributes.get_mut(&Attributes::MAX_HEALTH.id) {
                health.base_value = 40.0;
                health.dirty.store(true, Ordering::Relaxed);
            }
            if let Some(speed) = attributes.get_mut(&Attributes::MOVEMENT_SPEED.id) {
                speed.base_value = 0.3;
                speed.dirty.store(true, Ordering::Relaxed);
            }
            if let Some(knockback_res) = attributes.get_mut(&Attributes::KNOCKBACK_RESISTANCE.id) {
                knockback_res.base_value = 0.6;
                knockback_res.dirty.store(true, Ordering::Relaxed);
            }
            if let Some(attack_kb) = attributes.get_mut(&Attributes::ATTACK_KNOCKBACK.id) {
                attack_kb.base_value = 1.0;
                attack_kb.dirty.store(true, Ordering::Relaxed);
            }
            if let Some(damage) = attributes.get_mut(&Attributes::ATTACK_DAMAGE.id) {
                damage.base_value = 6.0;
                damage.dirty.store(true, Ordering::Relaxed);
            }
        }
        mob_entity.living_entity.health.store(40.0);

        let hoglin = Self {
            mob_entity,
            immune_to_zombification: AtomicBool::new(false),
            time_in_overworld: AtomicI32::new(0),
            cannot_be_hunted: AtomicBool::new(false),
            is_baby: AtomicBool::new(false),
            nearest_repellent: Mutex::new(None),
            pacified_ticks: AtomicI32::new(0),
            avoid_target: Mutex::new(None),
            avoid_ticks: AtomicI32::new(0),
        };
        let mob_arc = Arc::new(hoglin);
        let mob_weak: Weak<dyn Mob> = {
            let mob_arc: Arc<dyn Mob> = mob_arc.clone();
            Arc::downgrade(&mob_arc)
        };
        let hoglin_weak: Weak<Self> = Arc::downgrade(&mob_arc);

        {
            let mut goal_selector = mob_arc
                .mob_entity
                .goals_selector
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);

            goal_selector.add_goal(0, Box::new(SwimGoal::default()));
            goal_selector.add_goal(3, Box::new(HoglinRetreatGoal::new(hoglin_weak.clone())));
            goal_selector.add_goal(3, Box::new(HoglinAvoidRepellentGoal::new(hoglin_weak)));
            goal_selector.add_goal(4, Box::new(MeleeAttackGoal::new(1.0, true)));
            goal_selector.add_goal(5, Box::new(WanderAroundGoal::new(1.0)));
            goal_selector.add_goal(
                6,
                LookAtEntityGoal::with_default(mob_weak.clone(), &EntityType::PLAYER, 8.0),
            );
            goal_selector.add_goal(7, Box::new(RandomLookAroundGoal::default()));

            let mut target_selector = mob_arc
                .mob_entity
                .target_selector
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            target_selector.add_goal(1, Box::new(RevengeGoal::new(true)));
            target_selector.add_goal(
                2,
                ActiveTargetGoal::with_default(&mob_arc.mob_entity, &EntityType::PLAYER, true)
                    .gated_by(hoglin_can_attack),
            );
        };

        mob_arc
    }

    pub fn is_immune_to_zombification(&self) -> bool {
        self.immune_to_zombification.load(Ordering::Relaxed)
    }

    pub fn set_immune_to_zombification(&self, immune: bool) {
        self.immune_to_zombification
            .store(immune, Ordering::Relaxed);
        self.mob_entity.living_entity.entity.set_synced_data(
            pumpkin_data::tracked_data::hoglin::DATA_IMMUNE_TO_ZOMBIFICATION,
            immune,
        );
    }

    pub fn is_converting(&self, world: &World) -> bool {
        !self.is_immune_to_zombification()
            && !self.mob_entity.is_no_ai()
            && world.dimension.piglins_zombify
    }

    #[must_use]
    pub fn is_adult(&self) -> bool {
        !self.is_baby.load(Ordering::Relaxed)
    }

    #[must_use]
    pub fn is_pacified(&self) -> bool {
        self.pacified_ticks.load(Ordering::Relaxed) > 0
    }

    fn refresh_nearest_repellent(&self) {
        let entity = &self.mob_entity.living_entity.entity;
        let world = entity.world.load();
        let center = entity.block_pos.load();
        let mut nearest: Option<(i32, BlockPos)> = None;

        for dy in -REPELLENT_DETECTION_RANGE_VERTICAL..=REPELLENT_DETECTION_RANGE_VERTICAL {
            for dx in -REPELLENT_DETECTION_RANGE_HORIZONTAL..=REPELLENT_DETECTION_RANGE_HORIZONTAL {
                for dz in
                    -REPELLENT_DETECTION_RANGE_HORIZONTAL..=REPELLENT_DETECTION_RANGE_HORIZONTAL
                {
                    let pos = BlockPos::new(center.0.x + dx, center.0.y + dy, center.0.z + dz);
                    if world
                        .get_block(&pos)
                        .has_tag(&tag::Block::MINECRAFT_HOGLIN_REPELLENTS)
                    {
                        let distance = dx * dx + dy * dy + dz * dz;
                        if nearest.is_none_or(|(best, _)| distance < best) {
                            nearest = Some((distance, pos));
                        }
                    }
                }
            }
        }

        *self
            .nearest_repellent
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner) = nearest.map(|(_, pos)| pos);
    }

    /// Vanilla `HoglinAi.isPosNearNearestRepellent`.
    #[must_use]
    pub fn is_pos_near_nearest_repellent(&self, pos: &BlockPos) -> bool {
        let repellent = self
            .nearest_repellent
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        repellent.as_ref().is_some_and(|repellent| {
            let dx = f64::from(pos.0.x - repellent.0.x);
            let dy = f64::from(pos.0.y - repellent.0.y);
            let dz = f64::from(pos.0.z - repellent.0.z);
            dx * dx + dy * dy + dz * dz < REPELLENT_DESIRED_DISTANCE * REPELLENT_DESIRED_DISTANCE
        })
    }

    fn nearest_repellent(&self) -> Option<BlockPos> {
        *self
            .nearest_repellent
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }

    fn avoid_target(&self) -> Option<Arc<dyn EntityBase>> {
        self.avoid_target
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .clone()
    }

    fn clear_avoid_target(&self) {
        *self
            .avoid_target
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner) = None;
        self.avoid_ticks.store(0, Ordering::Relaxed);
    }

    /// Vanilla `HoglinAi.setAvoidTarget`.
    fn set_avoid_target(&self, target: Arc<dyn EntityBase>) {
        self.set_mob_target(None);
        self.mob_entity
            .navigator
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .stop();
        *self
            .avoid_target
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner) = Some(target);
        self.avoid_ticks.store(
            rand::random_range(RETREAT_DURATION_MIN..=RETREAT_DURATION_MAX),
            Ordering::Relaxed,
        );
    }

    /// Vanilla `HoglinAi.piglinsOutnumberHoglins`.
    fn piglins_outnumber_hoglins(&self) -> bool {
        if !self.is_adult() {
            return false;
        }

        let entity = &self.mob_entity.living_entity.entity;
        let world = entity.world.load();
        let follow_range = self
            .mob_entity
            .living_entity
            .get_attribute_value(&Attributes::FOLLOW_RANGE);
        let search_box =
            entity
                .bounding_box
                .load()
                .expand(follow_range, follow_range, follow_range);
        let mut piglins = 0;
        let mut hoglins = 0;

        for candidate in world.get_entities_at_box(&search_box) {
            let candidate_entity = candidate.get_entity();
            if candidate_entity.entity_id == entity.entity_id || !candidate_entity.is_alive() {
                continue;
            }
            if *candidate_entity.entity_type == EntityType::PIGLIN {
                piglins += 1;
            } else if let Some(mob) = candidate.get_mob()
                && let Some(hoglin) = mob.cast_any().downcast_ref::<Self>()
                && hoglin.is_adult()
            {
                hoglins += 1;
            }
        }

        piglins > hoglins + 1
    }

    /// Vanilla `HoglinAi.wantsToStopFleeing`.
    fn wants_to_stop_fleeing(&self) -> bool {
        self.is_adult() && !self.piglins_outnumber_hoglins()
    }

    /// Vanilla `HoglinAi.broadcastRetreat`.
    fn broadcast_retreat(&self, target: &Arc<dyn EntityBase>) {
        let entity = &self.mob_entity.living_entity.entity;
        let world = entity.world.load();
        let search_box = entity.bounding_box.load().expand(16.0, 8.0, 16.0);

        for candidate in world.get_entities_at_box(&search_box) {
            let candidate_entity = candidate.get_entity();
            if candidate_entity.entity_id == entity.entity_id {
                continue;
            }
            if let Some(mob) = candidate.get_mob()
                && let Some(hoglin) = mob.cast_any().downcast_ref::<Self>()
                && hoglin.is_adult()
            {
                hoglin.set_avoid_target(target.clone());
            }
        }
    }

    fn tick_hoglin_ai(&self) {
        let entity = &self.mob_entity.living_entity.entity;
        if entity.age.load(Ordering::Relaxed) % REPELLENT_SCAN_INTERVAL == 0 {
            self.refresh_nearest_repellent();
        }

        if self.nearest_repellent().is_some() {
            self.pacified_ticks
                .store(REPELLENT_PACIFY_TIME, Ordering::Relaxed);
        } else {
            let pacified = self.pacified_ticks.load(Ordering::Relaxed);
            if pacified > 0 {
                self.pacified_ticks.store(pacified - 1, Ordering::Relaxed);
            }
        }

        if self.avoid_target().is_some() {
            let ticks = self.avoid_ticks.load(Ordering::Relaxed);
            if ticks > 0 {
                self.avoid_ticks.store(ticks - 1, Ordering::Relaxed);
            } else {
                self.clear_avoid_target();
            }
            if self.wants_to_stop_fleeing() {
                self.clear_avoid_target();
            }
        } else {
            self.avoid_ticks.store(0, Ordering::Relaxed);
        }
    }

    fn convert_to_zoglin(&self) {
        let entity = &self.mob_entity.living_entity.entity;
        let world = entity.world.load();
        let pos = entity.pos.load();

        if world.level_info.load().difficulty != pumpkin_util::Difficulty::Peaceful {
            world.play_sound(
                Sound::EntityHoglinConvertedToZombified,
                SoundCategory::Hostile,
                &pos,
            );
        }

        let zoglin = crate::entity::r#type::from_type(
            &EntityType::ZOGLIN,
            pos,
            &world,
            uuid::Uuid::new_v4(),
        );

        let zoglin_base = zoglin.get_entity();
        zoglin_base.set_rotation(entity.yaw.load(), entity.pitch.load());
        zoglin_base.head_yaw.store(entity.head_yaw.load());
        zoglin_base.velocity.store(entity.velocity.load());

        if let Some(living) = zoglin.get_living_entity() {
            living.set_health(self.mob_entity.living_entity.health.load());
        }

        if let Some(custom_name) = &**entity.custom_name.load() {
            zoglin_base.set_custom_name(custom_name.clone());
        }

        // Vanilla ConversionType.SINGLE keeps the baby state.
        if self.is_baby.load(Ordering::Relaxed)
            && let Some(mob) = zoglin.get_mob()
        {
            mob.spawn_as_baby();
        }

        world.spawn_entity(zoglin);
        entity.remove();
    }
}

fn hoglin_can_attack(mob: &dyn Mob) -> bool {
    mob.cast_any()
        .downcast_ref::<HoglinEntity>()
        .is_none_or(|hoglin| !hoglin.is_pacified())
}

/// Vanilla `SetWalkTargetAwayFrom.pos` for the nearest hoglin repellent.
struct HoglinAvoidRepellentGoal {
    hoglin: Weak<HoglinEntity>,
    target: Option<Vector3<f64>>,
}

impl HoglinAvoidRepellentGoal {
    const fn new(hoglin: Weak<HoglinEntity>) -> Self {
        Self {
            hoglin,
            target: None,
        }
    }
}

impl Goal for HoglinAvoidRepellentGoal {
    fn can_start(&mut self, mob: &dyn Mob) -> bool {
        let Some(hoglin) = self.hoglin.upgrade() else {
            return false;
        };
        if mob.get_mob_entity().get_target().is_some() {
            return false;
        }
        let Some(repellent) = hoglin.nearest_repellent() else {
            return false;
        };
        let entity_pos = mob.get_entity().pos.load();
        let repellent_pos = Vector3::new(
            f64::from(repellent.0.x) + 0.5,
            f64::from(repellent.0.y),
            f64::from(repellent.0.z) + 0.5,
        );
        if entity_pos.squared_distance_to_vec(&repellent_pos)
            >= REPELLENT_DESIRED_DISTANCE * REPELLENT_DESIRED_DISTANCE
        {
            return false;
        }

        for _ in 0..10 {
            if let Some(target) = default_random_pos::get_pos_away(mob, 16, 7, repellent_pos) {
                self.target = Some(target);
                return true;
            }
        }
        false
    }

    fn should_continue(&mut self, mob: &dyn Mob) -> bool {
        self.target.is_some() && !mob.is_navigator_idle()
    }

    fn start(&mut self, mob: &dyn Mob) {
        if let Some(target) = self.target {
            mob.get_mob_entity()
                .navigator
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .set_progress(NavigatorGoal::new(
                    mob.get_entity().pos.load(),
                    target,
                    REPELLENT_SPEED_MULTIPLIER,
                ));
        }
    }

    fn stop(&mut self, mob: &dyn Mob) {
        self.target = None;
        mob.get_mob_entity()
            .navigator
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .stop();
    }

    fn controls(&self) -> Controls {
        Controls::MOVE
    }
}

/// Vanilla `SetWalkTargetAwayFrom.entity` for an `AVOID_TARGET` memory.
struct HoglinRetreatGoal {
    hoglin: Weak<HoglinEntity>,
    target: Option<Vector3<f64>>,
}

impl HoglinRetreatGoal {
    const fn new(hoglin: Weak<HoglinEntity>) -> Self {
        Self {
            hoglin,
            target: None,
        }
    }
}

impl Goal for HoglinRetreatGoal {
    fn can_start(&mut self, mob: &dyn Mob) -> bool {
        let Some(hoglin) = self.hoglin.upgrade() else {
            return false;
        };
        let Some(avoid_target) = hoglin.avoid_target() else {
            return false;
        };
        let entity_pos = mob.get_entity().pos.load();
        let avoid_pos = avoid_target.get_entity().pos.load();
        if entity_pos.squared_distance_to_vec(&avoid_pos)
            >= RETREAT_DESIRED_DISTANCE * RETREAT_DESIRED_DISTANCE
        {
            return false;
        }

        for _ in 0..10 {
            if let Some(target) = default_random_pos::get_pos_away(mob, 16, 7, avoid_pos) {
                self.target = Some(target);
                return true;
            }
        }
        false
    }

    fn should_continue(&mut self, mob: &dyn Mob) -> bool {
        if self.target.is_none() || mob.is_navigator_idle() {
            return false;
        }
        self.hoglin.upgrade().is_some_and(|hoglin| {
            hoglin.avoid_target().is_some() && !hoglin.wants_to_stop_fleeing()
        })
    }

    fn start(&mut self, mob: &dyn Mob) {
        if let Some(target) = self.target {
            mob.get_mob_entity()
                .navigator
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .set_progress(NavigatorGoal::new(
                    mob.get_entity().pos.load(),
                    target,
                    RETREAT_SPEED_MULTIPLIER,
                ));
        }
    }

    fn stop(&mut self, mob: &dyn Mob) {
        self.target = None;
        mob.get_mob_entity()
            .navigator
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .stop();
    }

    fn controls(&self) -> Controls {
        Controls::MOVE | Controls::LOOK
    }
}

impl Mob for HoglinEntity {
    fn get_mob_entity(&self) -> &MobEntity {
        &self.mob_entity
    }

    fn spawn_as_baby(&self) -> bool {
        self.is_baby.store(true, Ordering::Relaxed);
        self.mob_entity.set_baby_by_age();
        true
    }

    fn mob_write_nbt(&self, nbt: &mut NbtCompound) {
        if self.is_immune_to_zombification() {
            nbt.put_bool("IsImmuneToZombification", true);
        }
        let time = self.time_in_overworld.load(Ordering::Relaxed);
        if time > 0 {
            nbt.put_int("TimeInOverworld", time);
        }
        if self.cannot_be_hunted.load(Ordering::Relaxed) {
            nbt.put_bool("CannotBeHunted", true);
        }
    }

    fn mob_read_nbt(&self, nbt: &NbtCompound) {
        if let Some(immune) = nbt.get_bool("IsImmuneToZombification") {
            self.set_immune_to_zombification(immune);
        }
        if let Some(time) = nbt.get_int("TimeInOverworld") {
            self.time_in_overworld.store(time, Ordering::Relaxed);
        }
        if let Some(cannot_hunt) = nbt.get_bool("CannotBeHunted") {
            self.cannot_be_hunted.store(cannot_hunt, Ordering::Relaxed);
        }
    }

    fn mob_tick(&self, _caller: &dyn EntityBase) {
        let entity = &self.mob_entity.living_entity.entity;
        if !entity.is_alive() {
            return;
        }

        self.tick_hoglin_ai();

        let world = entity.world.load();
        if self.is_converting(&world) {
            let time = self.time_in_overworld.fetch_add(1, Ordering::Relaxed) + 1;
            if time > Self::CONVERSION_TIME {
                self.convert_to_zoglin();
            }
        } else {
            self.time_in_overworld.store(0, Ordering::Relaxed);
        }
    }

    fn on_attack(&self, target: &dyn EntityBase) {
        throw_target(&self.mob_entity.living_entity.entity, target);

        // Vanilla `HoglinAi.onHitTarget`: outnumbered adults retreat from piglins.
        if target.get_entity().entity_type == &EntityType::PIGLIN
            && self.is_adult()
            && self.piglins_outnumber_hoglins()
            && let Some(target) = self.mob_entity.get_target()
        {
            self.set_avoid_target(target.clone());
            self.broadcast_retreat(&target);
        }
    }

    fn on_damage(
        &self,
        _damage_type: pumpkin_data::damage::DamageType,
        source: Option<&dyn EntityBase>,
    ) {
        // Vanilla `HoglinAi.wasHurtBy` erases the pacified memory.
        self.pacified_ticks.store(0, Ordering::Relaxed);

        let Some(source) = source else {
            return;
        };
        if !self.is_baby.load(Ordering::Relaxed) {
            return;
        }
        let world = self.mob_entity.living_entity.entity.world.load();
        if let Some(attacker) = world.get_entity_by_id(source.get_entity().entity_id) {
            self.set_avoid_target(attacker.clone());
            self.broadcast_retreat(&attacker);
        }
    }

    fn get_walk_target_value(&self, pos: &BlockPos) -> f32 {
        if self.is_pos_near_nearest_repellent(pos) {
            return -1.0;
        }
        let world = self.mob_entity.living_entity.entity.world.load();
        if world.get_block(&pos.down()).id == Block::CRIMSON_NYLIUM.id {
            10.0
        } else {
            0.0
        }
    }
}

/// Vanilla `HoglinBase.throwTarget`, shared with zoglins.
pub fn throw_target(attacker: &Entity, target: &dyn EntityBase) {
    let my_pos = attacker.pos.load();
    let target_pos = target.get_entity().pos.load();
    let dx = target_pos.x - my_pos.x;
    let dz = target_pos.z - my_pos.z;
    let dist = dx.hypot(dz).max(0.001);
    let vel = target.get_entity().velocity.load();
    target.get_entity().velocity.store(Vector3::new(
        vel.x + (dx / dist) * 0.5,
        0.5,
        vel.z + (dz / dist) * 0.5,
    ));
}
