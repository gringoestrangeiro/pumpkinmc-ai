use std::sync::{
    Arc, Weak,
    atomic::{AtomicBool, AtomicI32, Ordering},
};

use crossbeam::atomic::AtomicCell;
use pumpkin_data::damage::DamageType;
use pumpkin_data::effect::StatusEffect;
use pumpkin_data::entity::EntityType;
use pumpkin_data::item::Item;
use pumpkin_data::item_stack::ItemStack;
use pumpkin_data::potion::Effect;
use pumpkin_data::sound::{Sound, SoundCategory};
use pumpkin_data::structures::StructureKeys;
use pumpkin_data::tag::{self, Taggable};
use pumpkin_data::tracked_data;
use pumpkin_nbt::compound::NbtCompound;
use pumpkin_protocol::codec::var_int::VarInt;
use pumpkin_protocol::java::client::play::CEntityStatus;
use pumpkin_util::math::boundingbox::{BoundingBox, EntityDimensions};
use pumpkin_util::math::position::BlockPos;
use pumpkin_util::math::vector3::Vector3;

use crate::entity::{
    Entity, EntityBase,
    ageable::{AgeableData, AgeableMob},
    ai::goal::{
        Controls, avoid_entity::AvoidEntityGoal, escape_danger::EscapeDangerGoal,
        look_around::RandomLookAroundGoal, look_at_entity::LookAtEntityGoal,
        melee_attack::MeleeAttackGoal, revenge::RevengeGoal, swim::SwimGoal, tempt::TemptGoal,
        try_find_water::TryFindWaterGoal, wander_around::WanderAroundGoal,
    },
    ai::pathfinder::{NavigatorGoal, node::PathComputationType},
    ai::util::{default_random_pos, goal_utils},
    custom_sound::CustomSound,
    mob::{Mob, MobEntity},
    passive::{animal::Animal, fish},
    player::Player,
};
use pumpkin_world::generation::generator::structure_finder::find_nearest_structure_of_types;

const TEMPT_ITEMS: &[&Item] = &[&Item::COD, &Item::SALMON, &Item::TROPICAL_FISH];
const TOTAL_MOISTNESS_LEVEL: i32 = 2400;
const BABY_SCALE: f32 = 0.65;
const TREASURE_SEARCH_RADIUS: i32 = 50;

pub struct DolphinEntity {
    pub mob_entity: MobEntity,
    pub ageable_data: AgeableData,
    pub got_fish: AtomicBool,
    pub moistness_level: AtomicI32,
    pub treasure_pos: AtomicCell<Option<BlockPos>>,
}

impl DolphinEntity {
    pub fn new(entity: Entity) -> Arc<Self> {
        let mob_entity = MobEntity::new(entity);
        let dolphin = Self {
            mob_entity,
            ageable_data: AgeableData::default(),
            got_fish: AtomicBool::new(false),
            moistness_level: AtomicI32::new(TOTAL_MOISTNESS_LEVEL),
            treasure_pos: AtomicCell::new(None),
        };
        let mob_arc = Arc::new(dolphin);
        let dolphin_weak = Arc::downgrade(&mob_arc);
        let mob_weak: Weak<dyn Mob> = {
            let mob_arc: Arc<dyn Mob> = mob_arc.clone();
            Arc::downgrade(&mob_arc)
        };

        {
            let mut goal_selector = mob_arc
                .mob_entity
                .goals_selector
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);

            goal_selector.add_goal(0, Box::new(TryFindWaterGoal));
            goal_selector.add_goal(0, Box::new(SwimGoal::default()));
            goal_selector.add_goal(1, EscapeDangerGoal::new(1.6));
            goal_selector.add_goal(1, Box::new(DolphinSwimToTreasureGoal::new(dolphin_weak)));
            goal_selector.add_goal(2, Box::new(DolphinSwimWithPlayerGoal::new(4.0)));
            goal_selector.add_goal(3, Box::new(MeleeAttackGoal::new(1.2, true)));
            goal_selector.add_goal(4, Box::new(TemptGoal::new(1.2, TEMPT_ITEMS, false)));
            goal_selector.add_goal(5, Box::new(WanderAroundGoal::new(1.0)));
            goal_selector.add_goal(
                6,
                LookAtEntityGoal::with_default(mob_weak, &EntityType::PLAYER, 6.0),
            );
            goal_selector.add_goal(7, Box::new(RandomLookAroundGoal::default()));
            goal_selector.add_goal(
                9,
                Box::new(AvoidEntityGoal::new(&EntityType::GUARDIAN, 8.0, 1.0, 1.0)),
            );
        };

        {
            let mut target_selector = mob_arc
                .mob_entity
                .target_selector
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);

            target_selector.add_goal(1, Box::new(RevengeGoal::new(true)));
        };

        mob_arc.refresh_dimensions();
        mob_arc
    }

    #[must_use]
    pub fn got_fish(&self) -> bool {
        self.got_fish.load(Ordering::Relaxed)
    }

    pub fn set_got_fish(&self, val: bool) {
        self.got_fish.store(val, Ordering::Relaxed);
        self.get_entity()
            .set_synced_data(tracked_data::dolphin::GOT_FISH, val);
    }

    #[must_use]
    pub fn moistness_level(&self) -> i32 {
        self.moistness_level.load(Ordering::Relaxed)
    }

    pub fn set_moistness_level(&self, level: i32) {
        self.moistness_level.store(level, Ordering::Relaxed);
        self.get_entity()
            .set_synced_data(tracked_data::dolphin::MOISTNESS_LEVEL, VarInt(level));
    }

    #[must_use]
    pub fn treasure_pos(&self) -> Option<BlockPos> {
        self.treasure_pos.load()
    }

    pub fn set_treasure_pos(&self, pos: Option<BlockPos>) {
        self.treasure_pos.store(pos);
    }

    fn refresh_dimensions(&self) {
        let entity = self.get_entity();
        let scale = if self.is_baby() { BABY_SCALE } else { 1.0 };
        let dimensions = EntityDimensions {
            width: entity.entity_type.dimension[0] * scale,
            height: entity.entity_type.dimension[1] * scale,
            eye_height: entity.entity_type.eye_height * scale,
        };
        let pos = entity.pos.load();
        entity
            .bounding_box
            .store(BoundingBox::new_from_pos(pos.x, pos.y, pos.z, &dimensions));
        entity.entity_dimension.store(dimensions);
    }

    fn close_to_next_pos(&self) -> bool {
        let target = self
            .mob_entity
            .navigator
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .get_target_pos();
        target.is_some_and(|target| {
            let pos = self.get_entity().pos.load();
            let dx = f64::from(target.0.x) + 0.5 - pos.x;
            let dy = f64::from(target.0.y) + 0.5 - pos.y;
            let dz = f64::from(target.0.z) + 0.5 - pos.z;
            dx * dx + dy * dy + dz * dz <= 12.0 * 12.0
        })
    }

    fn tick_moistness(&self, caller: &dyn EntityBase) {
        let entity = self.get_entity();
        if entity.is_in_water_or_rain() {
            self.set_moistness_level(TOTAL_MOISTNESS_LEVEL);
            return;
        }

        let level = self.moistness_level() - 1;
        self.set_moistness_level(level);
        if level <= 0 {
            caller.damage(caller, 1.0, DamageType::DRY_OUT);
        }

        if entity.on_ground.load(Ordering::Relaxed) {
            let mut velocity = entity.velocity.load();
            velocity.x += f64::from(rand::random::<f32>() * 2.0 - 1.0) * 0.2;
            velocity.y += 0.5;
            velocity.z += f64::from(rand::random::<f32>() * 2.0 - 1.0) * 0.2;
            entity.set_velocity(velocity);
            entity.yaw.store(rand::random::<f32>() * 360.0);
            entity.on_ground.store(false, Ordering::Relaxed);
        }
    }
}

impl AgeableMob for DolphinEntity {
    fn get_ageable_data(&self) -> &AgeableData {
        &self.ageable_data
    }

    fn age_boundary_reached(&self) {
        self.refresh_dimensions();
    }
}

impl Animal for DolphinEntity {
    fn is_food(&self, item_stack: &ItemStack) -> bool {
        item_stack.item.has_tag(&tag::Item::MINECRAFT_FISHES)
    }
}

impl CustomSound for DolphinEntity {
    fn death_sound(&self) -> Option<Sound> {
        Some(Sound::EntityDolphinDeath)
    }

    fn hurt_sound(&self) -> Option<Sound> {
        Some(Sound::EntityDolphinHurt)
    }
}

impl Mob for DolphinEntity {
    fn as_ageable(&self) -> Option<&dyn AgeableMob> {
        Some(self)
    }

    fn as_animal(&self) -> Option<&dyn Animal> {
        Some(self)
    }

    fn as_custom_sound(&self) -> Option<&dyn CustomSound> {
        Some(self)
    }

    fn mob_write_nbt(&self, nbt: &mut NbtCompound) {
        self.write_ageable_nbt(nbt);
        nbt.put_bool("GotFish", self.got_fish());
        nbt.put_int("Moistness", self.moistness_level());
    }

    fn mob_read_nbt(&self, nbt: &NbtCompound) {
        self.read_ageable_nbt(nbt);
        if let Some(got_fish) = nbt.get_bool("GotFish") {
            self.set_got_fish(got_fish);
        }
        if let Some(moistness) = nbt.get_int("Moistness") {
            self.set_moistness_level(moistness);
        }
    }

    fn get_mob_entity(&self) -> &MobEntity {
        &self.mob_entity
    }

    fn mob_tick(&self, caller: &dyn EntityBase) {
        self.ageable_ai_step();
        if !self.mob_entity.is_no_ai() {
            self.tick_moistness(caller);
        }
    }

    fn mob_init_data_tracker(&self) {
        let entity = self.get_entity();
        if self.is_baby() {
            entity.set_synced_data(tracked_data::ageable_mob::DATA_BABY_ID, true);
        }
        entity.set_synced_data(tracked_data::dolphin::GOT_FISH, self.got_fish());
        entity.set_synced_data(
            tracked_data::dolphin::MOISTNESS_LEVEL,
            VarInt(self.moistness_level()),
        );
    }

    fn mob_interact(&self, player: &Arc<Player>, item_stack: &mut ItemStack) -> bool {
        if !item_stack.is_empty() && self.is_food(item_stack) {
            item_stack.decrement_unless_creative(player.gamemode.load(), 1);
            if self.can_age_up() {
                let seconds =
                    <Self as AgeableMob>::get_speed_up_seconds_when_feeding(-self.get_age());
                self.age_up(seconds, true);
            } else {
                self.set_got_fish(true);
            }
            let entity = self.get_entity();
            entity.world.load().play_sound(
                Sound::EntityDolphinEat,
                SoundCategory::Neutral,
                &entity.pos.load(),
            );
            return true;
        }

        self.animal_interact(player, item_stack, Sound::EntityDolphinAmbientWater)
    }

    fn can_attack(&self, target: &dyn EntityBase) -> bool {
        !self.is_baby() && self.mob_entity.living_entity.can_attack(target)
    }

    fn mob_is_pushed_by_fluids(&self) -> bool {
        false
    }

    fn check_spawn_obstruction(&self, world: &crate::world::World) -> bool {
        fish::check_spawn_obstruction(self, world)
    }

    fn get_walk_target_value(&self, _pos: &BlockPos) -> f32 {
        0.0
    }

    fn get_ambient_sound(&self) -> Option<Sound> {
        Some(if self.get_entity().is_in_water() {
            Sound::EntityDolphinAmbientWater
        } else {
            Sound::EntityDolphinAmbient
        })
    }

    fn get_ambient_sound_interval(&self) -> i32 {
        120
    }
}

/// Vanilla `Dolphin.DolphinSwimWithPlayerGoal`: swim beside a player who is
/// swimming and keep Dolphin's Grace on them.
struct DolphinSwimWithPlayerGoal {
    player: Option<Arc<Player>>,
    speed: f64,
}

impl DolphinSwimWithPlayerGoal {
    const fn new(speed: f64) -> Self {
        Self {
            player: None,
            speed,
        }
    }

    fn apply_grace(player: &Player) {
        player.add_effect(Effect {
            effect_type: &StatusEffect::DOLPHINS_GRACE,
            duration: 100,
            amplifier: 0,
            ambient: false,
            show_particles: true,
            show_icon: true,
            blend: false,
        });
    }
}

impl crate::entity::ai::goal::Goal for DolphinSwimWithPlayerGoal {
    fn can_start(&mut self, mob: &dyn Mob) -> bool {
        let entity = mob.get_entity();
        let world = entity.world.load();
        let position = entity.pos.load();
        let target_id = mob
            .get_mob_entity()
            .get_target()
            .map(|target| target.get_entity().entity_id);
        let player = {
            let players = world.players.load();
            let mut nearest: Option<(f64, Arc<Player>)> = None;
            for player in players.iter() {
                if !player.is_swimming() || Some(player.entity_id()) == target_id {
                    continue;
                }
                let distance = player
                    .get_entity()
                    .pos
                    .load()
                    .squared_distance_to_vec(&position);
                if distance > 100.0 {
                    continue;
                }
                if nearest.as_ref().is_none_or(|(best, _)| distance < *best) {
                    nearest = Some((distance, player.clone()));
                }
            }
            nearest.map(|(_, player)| player)
        };
        self.player = player;
        self.player.is_some()
    }

    fn should_continue(&mut self, mob: &dyn Mob) -> bool {
        let Some(player) = &self.player else {
            return false;
        };
        player.is_swimming()
            && mob
                .get_entity()
                .pos
                .load()
                .squared_distance_to_vec(&player.get_entity().pos.load())
                < 256.0
    }

    fn start(&mut self, _mob: &dyn Mob) {
        if let Some(player) = &self.player {
            Self::apply_grace(player);
        }
    }

    fn stop(&mut self, mob: &dyn Mob) {
        self.player = None;
        mob.get_mob_entity()
            .navigator
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .stop();
    }

    fn tick(&mut self, mob: &dyn Mob) {
        let Some(player) = &self.player else {
            return;
        };
        mob.get_mob_entity()
            .look_control
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .look_at_entity(mob, &(player.clone() as Arc<dyn EntityBase>));
        let distance_sq = mob
            .get_entity()
            .pos
            .load()
            .squared_distance_to_vec(&player.get_entity().pos.load());
        if distance_sq < 6.25 {
            mob.get_mob_entity()
                .navigator
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .stop();
        } else {
            mob.get_mob_entity()
                .navigator
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .set_progress(NavigatorGoal::new(
                    mob.get_entity().pos.load(),
                    player.get_entity().pos.load(),
                    self.speed,
                ));
        }
        if rand::random_range(0..6) == 0 {
            Self::apply_grace(player);
        }
    }

    fn controls(&self) -> Controls {
        Controls::MOVE | Controls::LOOK
    }
}

/// Vanilla `Dolphin.DolphinSwimToTreasureGoal`: swim toward the nearest
/// `minecraft:dolphin_located` structure and stop once the player is close.
struct DolphinSwimToTreasureGoal {
    dolphin: Weak<DolphinEntity>,
    stuck: bool,
}

impl DolphinSwimToTreasureGoal {
    const fn new(dolphin: Weak<DolphinEntity>) -> Self {
        Self {
            dolphin,
            stuck: false,
        }
    }

    fn is_close(mob: &dyn Mob, pos: &BlockPos) -> bool {
        let current = mob.get_entity().pos.load();
        let dx = f64::from(pos.0.x) + 0.5 - current.x;
        let dz = f64::from(pos.0.z) + 0.5 - current.z;
        dx * dx + dz * dz < 4.0 * 4.0
    }

    fn broadcast_interest(mob: &dyn Mob) {
        let entity = mob.get_entity();
        entity
            .world
            .load()
            .send_to_tracking_players_and_self(entity, &CEntityStatus::new(entity.entity_id, 38));
    }
}

impl crate::entity::ai::goal::Goal for DolphinSwimToTreasureGoal {
    fn can_start(&mut self, _mob: &dyn Mob) -> bool {
        self.dolphin
            .upgrade()
            .is_some_and(|dolphin| dolphin.got_fish())
    }

    fn should_continue(&mut self, mob: &dyn Mob) -> bool {
        let Some(dolphin) = self.dolphin.upgrade() else {
            return false;
        };
        let Some(treasure) = dolphin.treasure_pos() else {
            return false;
        };
        !Self::is_close(mob, &treasure) && !self.stuck
    }

    fn can_stop(&self) -> bool {
        false
    }

    fn controls(&self) -> Controls {
        Controls::MOVE | Controls::LOOK
    }

    fn start(&mut self, mob: &dyn Mob) {
        let Some(dolphin) = self.dolphin.upgrade() else {
            return;
        };
        self.stuck = false;
        mob.get_mob_entity()
            .navigator
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .stop();

        let entity = mob.get_entity();
        let world = entity.world.load();
        let world_gen = world.level.world_gen.load_full();
        let targets: Vec<StructureKeys> = tag::WorldgenStructure::MINECRAFT_DOLPHIN_LOCATED
            .0
            .iter()
            .filter_map(|name| StructureKeys::from_name(name))
            .collect();
        let found = find_nearest_structure_of_types(
            entity.block_pos.load(),
            &targets,
            TREASURE_SEARCH_RADIUS,
            &world_gen,
        );
        if let Some((pos, _)) = found {
            // The finder mirrors vanilla's locate position and reports y = 0,
            // so keep the dolphin's depth and lead it there horizontally.
            let y = entity.block_pos.load().0.y;
            dolphin.set_treasure_pos(Some(BlockPos::new(pos.0.x, y, pos.0.z)));
            Self::broadcast_interest(mob);
        } else {
            self.stuck = true;
        }
    }

    fn stop(&mut self, mob: &dyn Mob) {
        let Some(dolphin) = self.dolphin.upgrade() else {
            return;
        };
        let reached = dolphin
            .treasure_pos()
            .is_some_and(|pos| Self::is_close(mob, &pos));
        if dolphin.treasure_pos().is_none() || reached || self.stuck {
            dolphin.set_got_fish(false);
        }
    }

    fn tick(&mut self, mob: &dyn Mob) {
        let Some(dolphin) = self.dolphin.upgrade() else {
            return;
        };
        let Some(treasure) = dolphin.treasure_pos() else {
            return;
        };
        if !mob.is_navigator_idle() && !dolphin.close_to_next_pos() {
            return;
        }

        let entity = mob.get_entity();
        let world = entity.world.load();
        let treasure_pos = Vector3::new(
            f64::from(treasure.0.x) + 0.5,
            f64::from(treasure.0.y) + 0.5,
            f64::from(treasure.0.z) + 0.5,
        );
        let mut next_pos = default_random_pos::get_pos_towards(
            mob,
            16,
            1,
            treasure_pos,
            std::f64::consts::FRAC_PI_8,
        );
        if next_pos.is_none() {
            next_pos = default_random_pos::get_pos_towards(
                mob,
                8,
                4,
                treasure_pos,
                std::f64::consts::FRAC_PI_2,
            );
        }
        if let Some(pos) = next_pos {
            let block_pos = BlockPos::floored(pos.x, pos.y, pos.z);
            let (block, state) = world.get_block_and_state(&block_pos);
            if !goal_utils::is_water(&world, &block_pos)
                || !world
                    .block_registry
                    .is_pathfindable(block, state, PathComputationType::Water)
            {
                next_pos = default_random_pos::get_pos_towards(
                    mob,
                    8,
                    5,
                    treasure_pos,
                    std::f64::consts::FRAC_PI_2,
                );
            }
        }
        let Some(next_pos) = next_pos else {
            self.stuck = true;
            return;
        };

        mob.get_mob_entity()
            .look_control
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .look_at(mob, next_pos.x, next_pos.y, next_pos.z);
        mob.get_mob_entity()
            .navigator
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .set_progress(NavigatorGoal::new(
                mob.get_entity().pos.load(),
                next_pos,
                1.3,
            ));

        if rand::random_range(0..self.get_tick_count(80)) == 0 {
            Self::broadcast_interest(mob);
        }
    }
}
