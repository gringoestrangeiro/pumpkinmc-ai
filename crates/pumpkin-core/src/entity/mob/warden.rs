use std::sync::{
    Arc,
    atomic::{AtomicI32, Ordering},
};

use pumpkin_data::{
    attributes::Attributes,
    damage::DamageType,
    effect::StatusEffect,
    entity::{EntityPose, EntityStatus, EntityType},
    particle::Particle,
    potion::Effect,
    sound::{Sound, SoundCategory},
    tag::{self, Taggable},
    tracked_data,
};
use pumpkin_nbt::compound::NbtCompound;
use pumpkin_protocol::java::client::play::CEntityStatus;
use pumpkin_util::{GameMode, math::vector3::Vector3};

use crate::entity::{
    Entity, EntityBase,
    ai::goal::{
        Controls, Goal, look_around::RandomLookAroundGoal, melee_attack::MeleeAttackGoal,
        swim::SwimGoal, wander_around::WanderAroundGoal,
    },
    mob::{
        Mob, MobEntity,
        warden_anger::{AngerLevel, AngerManagement, MAX_ANGER},
    },
};
use crate::world::World;

const EMERGE_DURATION: i32 = 134;
const DARKNESS_DISPLAY_LIMIT: i32 = 200;
const DARKNESS_DURATION: i32 = 260;
const DARKNESS_RADIUS: f64 = 20.0;
const DARKNESS_INTERVAL: i32 = 120;
const ANGERMANAGEMENT_TICK_DELAY: i32 = 20;
const DEFAULT_ANGER: i32 = 35;
const ON_HURT_ANGER_BOOST: i32 = 20;
const TOUCH_COOLDOWN_TICKS: i32 = 20;
/// Vanilla `WardenAi.SNIFFING_DURATION`: `Mth.ceil(83.2)`.
const SNIFFING_DURATION: i32 = 84;
const SNIFF_INTERVAL: i32 = 120;
const SONIC_BOOM_DISTANCE_XZ: f64 = 15.0;
const SONIC_BOOM_DISTANCE_Y: f64 = 20.0;
const SONIC_BOOM_DAMAGE: f32 = 10.0;
const SONIC_BOOM_DURATION: i32 = 60;
/// Vanilla `SonicBoom.TICKS_BEFORE_PLAYING_SOUND`: `Mth.ceil(34.0)`.
const SONIC_BOOM_SOUND_DELAY: i32 = 34;
const SONIC_BOOM_COOLDOWN: i32 = 40;
const SONIC_BOOM_KNOCKBACK_VERTICAL: f64 = 0.5;
const SONIC_BOOM_KNOCKBACK_HORIZONTAL: f64 = 2.5;

pub struct WardenEntity {
    pub mob_entity: MobEntity,
    emerge_ticks: Arc<AtomicI32>,
    sniff_cooldown: Arc<AtomicI32>,
    sonic_boom_cooldown: Arc<AtomicI32>,
    anger_management: std::sync::Mutex<AngerManagement>,
    touch_cooldown: AtomicI32,
}

/// Vanilla `TryToSniff` + `Sniffing`: hold the idle warden still while it sniffs.
struct SniffGoal {
    cooldown: Arc<AtomicI32>,
    ticks: i32,
}

impl Goal for SniffGoal {
    fn can_start(&mut self, mob: &dyn Mob) -> bool {
        self.cooldown.load(Ordering::Relaxed) == 0
            && !mob.get_mob_entity().is_no_ai()
            && mob.get_mob_entity().get_target().is_none()
            && mob.get_entity().pose.load() == EntityPose::Standing
    }

    fn should_continue(&mut self, mob: &dyn Mob) -> bool {
        self.ticks < SNIFFING_DURATION
            && !mob.get_mob_entity().is_no_ai()
            && mob.get_mob_entity().get_target().is_none()
            && mob.get_entity().pose.load() == EntityPose::Sniffing
    }

    fn start(&mut self, mob: &dyn Mob) {
        self.ticks = 0;
        let entity = mob.get_entity();
        entity.set_pose(EntityPose::Sniffing);
        entity.world.load().play_sound_fine(
            Sound::EntityWardenSniff,
            SoundCategory::Hostile,
            &entity.pos.load(),
            5.0,
            1.0,
        );
        mob.get_mob_entity()
            .navigator
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .stop();
        self.cooldown.store(SNIFF_INTERVAL, Ordering::Relaxed);
    }

    fn tick(&mut self, _mob: &dyn Mob) {
        self.ticks += 1;
    }

    fn stop(&mut self, mob: &dyn Mob) {
        let entity = mob.get_entity();
        if entity.pose.load() == EntityPose::Sniffing {
            entity.set_pose(EntityPose::Standing);
        }
    }

    fn should_run_every_tick(&self) -> bool {
        true
    }

    fn controls(&self) -> Controls {
        Controls::MOVE | Controls::LOOK
    }
}

struct EmergeGoal {
    emerge_ticks: Arc<AtomicI32>,
}

impl Goal for EmergeGoal {
    fn can_start(&mut self, _mob: &dyn Mob) -> bool {
        self.emerge_ticks.load(Ordering::Relaxed) > 0
    }

    fn start(&mut self, mob: &dyn Mob) {
        mob.get_mob_entity()
            .navigator
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .stop();
    }

    fn stop(&mut self, mob: &dyn Mob) {
        let entity = &mob.get_mob_entity().living_entity.entity;
        if entity.pose.load() == EntityPose::Emerging {
            entity.set_pose(EntityPose::Standing);
        }
    }

    fn tick(&mut self, _mob: &dyn Mob) {
        self.emerge_ticks.fetch_sub(1, Ordering::Relaxed);
    }

    fn should_run_every_tick(&self) -> bool {
        true
    }

    fn can_stop(&self) -> bool {
        false
    }

    fn controls(&self) -> Controls {
        Controls::MOVE | Controls::LOOK | Controls::JUMP | Controls::TARGET
    }
}

/// Vanilla `SonicBoom`: a 60-tick charge that fires one 10-damage beam after 34 ticks.
struct SonicBoomGoal {
    cooldown: Arc<AtomicI32>,
    ticks: i32,
    blasted: bool,
}

impl SonicBoomGoal {
    fn in_range(mob: &dyn Mob, target: &dyn EntityBase) -> bool {
        closer_than(
            mob.get_entity(),
            target.get_entity(),
            SONIC_BOOM_DISTANCE_XZ,
            SONIC_BOOM_DISTANCE_Y,
        )
    }

    fn blast(mob: &dyn Mob, target: &dyn EntityBase) {
        let entity = mob.get_entity();
        let source = entity.pos.load()
            + Vector3::new(
                0.0,
                f64::from(entity.entity_dimension.load().height) / 2.0,
                0.0,
            );
        let delta = target.get_entity().get_eye_pos() - source;
        let distance = delta.length();
        if distance <= 0.0 {
            return;
        }
        let direction = delta / distance;
        let world = entity.world.load();

        for step in 1..distance.floor() as i32 + 7 {
            world.spawn_particle(
                source + direction * f64::from(step),
                Vector3::new(0.0, 0.0, 0.0),
                0.0,
                1,
                Particle::SonicBoom,
            );
        }

        world.play_sound_fine(
            Sound::EntityWardenSonicBoom,
            SoundCategory::Hostile,
            &entity.pos.load(),
            3.0,
            1.0,
        );

        let damaged = target.damage_with_context(
            target,
            SONIC_BOOM_DAMAGE,
            DamageType::SONIC_BOOM,
            None,
            Some(mob as &dyn EntityBase),
            Some(mob as &dyn EntityBase),
        );
        if damaged {
            // Vanilla `SonicBoom` pushes directly; knockback resistance scales both axes.
            let resistance = target.get_living_entity().map_or(0.0, |living| {
                living.get_attribute_value(&Attributes::KNOCKBACK_RESISTANCE)
            });
            target.get_entity().add_velocity(Vector3::new(
                direction.x * SONIC_BOOM_KNOCKBACK_HORIZONTAL * (1.0 - resistance),
                direction.y * SONIC_BOOM_KNOCKBACK_VERTICAL * (1.0 - resistance),
                direction.z * SONIC_BOOM_KNOCKBACK_HORIZONTAL * (1.0 - resistance),
            ));
        }
    }
}

impl Goal for SonicBoomGoal {
    fn can_start(&mut self, mob: &dyn Mob) -> bool {
        if self.cooldown.load(Ordering::Relaxed) > 0 {
            return false;
        }
        mob.get_mob_entity()
            .get_target()
            .is_some_and(|target| Self::in_range(mob, target.as_ref()))
    }

    fn should_continue(&mut self, _mob: &dyn Mob) -> bool {
        self.ticks < SONIC_BOOM_DURATION
    }

    fn start(&mut self, mob: &dyn Mob) {
        self.ticks = 0;
        self.blasted = false;
        let entity = mob.get_entity();
        entity
            .world
            .load()
            .send_to_tracking_players_and_self(entity, &CEntityStatus::new(entity.entity_id, 62));
        entity.world.load().play_sound_fine(
            Sound::EntityWardenSonicCharge,
            SoundCategory::Hostile,
            &entity.pos.load(),
            3.0,
            1.0,
        );
    }

    fn tick(&mut self, mob: &dyn Mob) {
        self.ticks += 1;
        if let Some(target) = mob.get_mob_entity().get_target() {
            let target_pos = target.get_entity().pos.load();
            mob.get_mob_entity()
                .look_control
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .look_at(mob, target_pos.x, target_pos.y, target_pos.z);
            if !self.blasted
                && self.ticks >= SONIC_BOOM_SOUND_DELAY
                && Self::in_range(mob, target.as_ref())
            {
                Self::blast(mob, target.as_ref());
                self.blasted = true;
            }
        }
    }

    fn stop(&mut self, _mob: &dyn Mob) {
        self.cooldown.store(SONIC_BOOM_COOLDOWN, Ordering::Relaxed);
    }

    fn should_run_every_tick(&self) -> bool {
        true
    }

    fn controls(&self) -> Controls {
        Controls::MOVE | Controls::LOOK
    }
}

/// Vanilla `Entity.closerThan(other, distanceXZ, distanceY)`.
fn closer_than(entity: &Entity, other: &Entity, distance_xz: f64, distance_y: f64) -> bool {
    let a = entity.pos.load();
    let b = other.pos.load();
    let dx = b.x - a.x;
    let dz = b.z - a.z;
    let dy = b.y - a.y;
    dx * dx + dz * dz < distance_xz * distance_xz && dy * dy < distance_y * distance_y
}

impl WardenEntity {
    pub fn new(entity: Entity) -> Arc<Self> {
        let mob_entity = MobEntity::new(entity);
        let emerge_ticks = Arc::new(AtomicI32::new(0));
        let sniff_cooldown = Arc::new(AtomicI32::new(SNIFF_INTERVAL));
        let sonic_boom_cooldown = Arc::new(AtomicI32::new(0));
        let warden = Self {
            mob_entity,
            emerge_ticks: emerge_ticks.clone(),
            sniff_cooldown: sniff_cooldown.clone(),
            sonic_boom_cooldown: sonic_boom_cooldown.clone(),
            anger_management: std::sync::Mutex::new(AngerManagement::default()),
            touch_cooldown: AtomicI32::new(0),
        };
        let mob_arc = Arc::new(warden);

        {
            let mut goal_selector = mob_arc
                .mob_entity
                .goals_selector
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);

            goal_selector.add_goal(0, Box::new(EmergeGoal { emerge_ticks }));
            goal_selector.add_goal(0, Box::new(SwimGoal::default()));
            goal_selector.add_goal(
                1,
                Box::new(SniffGoal {
                    cooldown: sniff_cooldown,
                    ticks: 0,
                }),
            );
            goal_selector.add_goal(
                3,
                Box::new(SonicBoomGoal {
                    cooldown: sonic_boom_cooldown,
                    ticks: 0,
                    blasted: false,
                }),
            );
            goal_selector.add_goal(4, Box::new(MeleeAttackGoal::new(1.0, true)));
            goal_selector.add_goal(5, Box::new(WanderAroundGoal::new(0.5)));
            goal_selector.add_goal(7, Box::new(RandomLookAroundGoal::default()));
        };

        mob_arc
    }

    pub fn emerge(&self) {
        let entity = &self.mob_entity.living_entity.entity;
        entity.set_pose(EntityPose::Emerging);
        self.emerge_ticks.store(EMERGE_DURATION, Ordering::Relaxed);
        entity.world.load().play_sound_fine(
            Sound::EntityWardenAgitated,
            SoundCategory::Hostile,
            &entity.pos.load(),
            5.0,
            1.0,
        );
    }

    fn is_digging_or_emerging(&self) -> bool {
        matches!(
            self.mob_entity.living_entity.entity.pose.load(),
            EntityPose::Digging | EntityPose::Emerging
        )
    }

    fn anger(&self) -> std::sync::MutexGuard<'_, AngerManagement> {
        self.anger_management
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }

    fn target(&self) -> Option<Arc<dyn EntityBase>> {
        self.mob_entity
            .target
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .clone()
    }

    pub fn can_target_entity(&self, entity: &dyn EntityBase) -> bool {
        let Some(living) = entity.get_living_entity() else {
            return false;
        };
        let target = entity.get_entity();
        let this = &self.mob_entity.living_entity.entity;
        let world = this.world.load();
        if !Arc::ptr_eq(&world, &target.world.load()) {
            return false;
        }
        if entity
            .get_player()
            .is_some_and(|player| player.is_creative() || player.is_spectator())
        {
            return false;
        }
        let position = target.pos.load();
        !(self as &dyn EntityBase).is_allied_to(entity)
            && target.entity_type.id != EntityType::ARMOR_STAND.id
            && target.entity_type.id != EntityType::WARDEN.id
            && !target.invulnerable.load(Ordering::Relaxed)
            && !living.dead.load(Ordering::Relaxed)
            && world
                .worldborder
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .contains(position.x, position.z)
    }

    pub fn get_anger_level(&self) -> AngerLevel {
        AngerLevel::by_anger(self.get_active_anger())
    }

    fn get_active_anger(&self) -> i32 {
        let target = self.target();
        self.anger().get_active_anger(target.as_deref())
    }

    pub fn get_entity_angry_at(&self) -> Option<Arc<dyn EntityBase>> {
        if !self.get_anger_level().is_angry() {
            return None;
        }
        self.anger()
            .get_active_entity(|entity| self.can_target_entity(entity))
    }

    pub fn clear_anger(&self, entity: &dyn EntityBase) {
        self.anger().clear_anger(entity);
    }

    pub fn increase_anger_at(&self, entity: &Arc<dyn EntityBase>, amount: i32, play_sound: bool) {
        if self.mob_entity.is_no_ai() || !self.can_target_entity(entity.as_ref()) {
            return;
        }
        if !self.fire_anger_change_event(entity.as_ref(), amount) {
            return;
        }
        let maybe_switch_target = self
            .target()
            .is_none_or(|target| target.get_player().is_none());
        let new_anger = self.anger().increase_anger(entity, amount);
        if entity.get_player().is_some()
            && maybe_switch_target
            && AngerLevel::by_anger(new_anger).is_angry()
        {
            self.set_mob_target(None);
        }
        if play_sound {
            self.play_listening_sound();
        }
    }

    /// Fires the anger change event and reports whether it was not cancelled.
    fn fire_anger_change_event(&self, target: &dyn EntityBase, amount: i32) -> bool {
        let entity = &self.mob_entity.living_entity.entity;
        let world = entity.world.load();
        let Some(server) = world.server.upgrade() else {
            return true;
        };
        let old_anger = self.anger().anger_of(target);
        let mut event =
            crate::plugin::api::events::entity::warden_anger_change::WardenAngerChangeEvent {
                entity_id: entity.entity_id,
                target_id: target.get_entity().entity_id,
                old_anger,
                new_anger: old_anger.saturating_add(amount).min(MAX_ANGER),
                cancelled: false,
            };
        server.plugin_manager.fire_blocking(&server, &mut event);
        !event.cancelled
    }

    fn play_listening_sound(&self) {
        let entity = &self.mob_entity.living_entity.entity;
        if entity.pose.load() != EntityPose::Roaring {
            entity.world.load().play_sound_fine(
                self.get_anger_level().listening_sound(),
                SoundCategory::Hostile,
                &entity.pos.load(),
                10.0,
                1.0,
            );
        }
    }

    fn sync_client_anger_level(&self) {
        self.mob_entity.living_entity.entity.set_synced_data(
            tracked_data::warden::CLIENT_ANGER_LEVEL,
            self.get_active_anger(),
        );
    }

    fn tick_touch(&self) {
        if self.touch_cooldown.load(Ordering::Relaxed) > 0 {
            self.touch_cooldown.fetch_sub(1, Ordering::Relaxed);
            return;
        }
        if self.mob_entity.is_no_ai() || self.is_digging_or_emerging() {
            return;
        }
        let entity = &self.mob_entity.living_entity.entity;
        let world = entity.world.load();
        let bounding_box = entity.bounding_box.load();
        let touching = world
            .get_players_at_box(&bounding_box)
            .into_iter()
            .map(|player| player as Arc<dyn EntityBase>)
            .chain(world.get_entities_at_box(&bounding_box))
            .find(|other| other.get_entity().entity_id != entity.entity_id && other.is_pushable());
        if let Some(other) = touching {
            self.touch_cooldown
                .store(TOUCH_COOLDOWN_TICKS, Ordering::Relaxed);
            self.increase_anger_at(&other, DEFAULT_ANGER, true);
        }
    }

    fn update_target_from_anger(&self) {
        let target = self.target();
        if let Some(target) = &target
            && (!self.get_anger_level().is_angry() || !self.can_target_entity(target.as_ref()))
        {
            if !self.can_target_entity(target.as_ref()) {
                self.clear_anger(target.as_ref());
            }
            self.set_mob_target(None);
            return;
        }
        if target.is_none()
            && let Some(angry_at) = self.get_entity_angry_at()
        {
            self.set_mob_target(Some(angry_at));
        }
    }

    /// Adds the anger a received vibration produces and answers with the warden's listening cue.
    pub fn hear_vibration(&self, world: &World, source: Option<&dyn EntityBase>) {
        let entity = &self.mob_entity.living_entity.entity;
        if !entity.is_alive()
            || self.mob_entity.living_entity.health.load() <= 0.0
            || self.mob_entity.is_no_ai()
            || self.is_digging_or_emerging()
        {
            return;
        }
        world.send_entity_status(entity, EntityStatus::TendrilsShiver, None);
        world.play_sound_fine(
            Sound::EntityWardenTendrilClicks,
            SoundCategory::Hostile,
            &entity.pos.load(),
            5.0,
            1.0,
        );

        let Some(source) = source else {
            return;
        };
        // Vanilla blames a projectile's owner rather than the projectile itself.
        let source_id = source.get_entity().entity_id;
        let suspect = source
            .get_owner_id()
            .and_then(|owner_id| entity_by_id(world, owner_id))
            .or_else(|| entity_by_id(world, source_id));
        if let Some(suspect) = suspect {
            self.increase_anger_at(&suspect, DEFAULT_ANGER, true);
        }
    }

    pub fn apply_darkness_around(world: &World, position: Vector3<f64>, radius: f64) {
        for player in world.get_nearby_players(position, radius) {
            if !matches!(
                player.gamemode.load(),
                GameMode::Survival | GameMode::Adventure
            ) {
                continue;
            }
            let current = player.living_entity.get_effect(&StatusEffect::DARKNESS);
            if current.is_some_and(|effect| {
                effect.duration < 0 || effect.duration >= DARKNESS_DISPLAY_LIMIT
            }) {
                continue;
            }
            let darkness = Effect {
                effect_type: &StatusEffect::DARKNESS,
                duration: DARKNESS_DURATION,
                amplifier: 0,
                ambient: false,
                show_particles: false,
                show_icon: false,
                blend: true,
            };
            player.send_effect(&darkness);
            player.living_entity.add_effect(darkness);
        }
    }
}

/// Resolves an entity id to its entity, covering the player map as well.
fn entity_by_id(world: &World, id: i32) -> Option<Arc<dyn EntityBase>> {
    world
        .get_player_by_id(id)
        .map(|player| player as Arc<dyn EntityBase>)
        .or_else(|| world.get_entity_by_id(id))
}

impl Mob for WardenEntity {
    fn get_mob_entity(&self) -> &MobEntity {
        &self.mob_entity
    }

    fn remove_when_far_away(&self, _distance_sq: f64) -> bool {
        false
    }

    fn pre_damage(&self, damage_type: DamageType, _source: Option<&dyn EntityBase>) -> bool {
        !self.is_digging_or_emerging()
            || damage_type.has_tag(&tag::DamageType::MINECRAFT_BYPASSES_INVULNERABILITY)
    }

    fn on_damage(&self, _damage_type: DamageType, source: Option<&dyn EntityBase>) {
        if self.mob_entity.is_no_ai() || self.is_digging_or_emerging() {
            return;
        }
        let Some(source) = source else {
            return;
        };
        let world = self.mob_entity.living_entity.entity.world.load();
        let Some(attacker) = entity_by_id(&world, source.get_entity().entity_id) else {
            return;
        };
        self.increase_anger_at(
            &attacker,
            AngerLevel::Angry.minimum_anger() + ON_HURT_ANGER_BOOST,
            false,
        );
        if self.target().is_none() && self.can_target_entity(attacker.as_ref()) {
            self.set_mob_target(Some(attacker));
        }
    }

    fn on_attack(&self, _target: &dyn EntityBase) {
        let entity = &self.mob_entity.living_entity.entity;
        let world = entity.world.load();
        world.send_entity_status(entity, EntityStatus::StartAttacking, None);
        world.play_sound_fine(
            Sound::EntityWardenAttackImpact,
            SoundCategory::Hostile,
            &entity.pos.load(),
            10.0,
            1.0,
        );
    }

    fn get_ambient_sound(&self) -> Option<Sound> {
        let entity = &self.mob_entity.living_entity.entity;
        // Vanilla `Warden#getAmbientSound`: silent while roaring or digging out.
        (!self.is_digging_or_emerging() && entity.pose.load() != EntityPose::Roaring)
            .then(|| self.get_anger_level().ambient_sound())
    }

    fn mob_tick(&self, _caller: &dyn EntityBase) {
        let entity = &self.mob_entity.living_entity.entity;
        if !entity.is_alive() {
            return;
        }
        let age = entity.age.load(Ordering::Relaxed);
        let world = entity.world.load();

        if self.sonic_boom_cooldown.load(Ordering::Relaxed) > 0 {
            self.sonic_boom_cooldown.fetch_sub(1, Ordering::Relaxed);
        }
        if self.sniff_cooldown.load(Ordering::Relaxed) > 0 {
            self.sniff_cooldown.fetch_sub(1, Ordering::Relaxed);
        }

        self.tick_touch();

        if (age + entity.entity_id) % DARKNESS_INTERVAL == 0 {
            Self::apply_darkness_around(&world, entity.pos.load(), DARKNESS_RADIUS);
        }

        if age % ANGERMANAGEMENT_TICK_DELAY == 0 {
            self.anger()
                .tick(&world, |suspect| self.can_target_entity(suspect));
            self.sync_client_anger_level();
        }

        self.update_target_from_anger();
    }

    fn mob_write_nbt(&self, nbt: &mut NbtCompound) {
        nbt.put_compound("anger", self.anger().to_nbt());
    }

    fn mob_read_nbt(&self, nbt: &NbtCompound) {
        if let Some(anger) = nbt.get_compound("anger") {
            *self.anger() = AngerManagement::from_nbt(anger);
            self.sync_client_anger_level();
        }
    }

    fn check_spawn_obstruction(&self, world: &World) -> bool {
        let entity = &self.mob_entity.living_entity.entity;
        let bounding_box = entity.bounding_box.load();
        !world.contains_any_liquid(bounding_box)
            && world.get_entities_at_box(&bounding_box).is_empty()
            && world.is_space_empty(bounding_box)
    }
}
