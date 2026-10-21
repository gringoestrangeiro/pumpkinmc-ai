use std::sync::{
    Arc, Weak,
    atomic::{AtomicI32, Ordering},
};

use crossbeam::atomic::AtomicCell;
use pumpkin_data::attributes::Attributes;
use pumpkin_data::entity::{EntityStatus, EntityType};
use pumpkin_data::item::Item;
use pumpkin_data::item_stack::ItemStack;
use pumpkin_data::sound::{Sound, SoundCategory};
use pumpkin_data::tag::{self, Taggable};
use pumpkin_nbt::compound::NbtCompound;
use pumpkin_protocol::codec::var_int::VarInt;
use pumpkin_util::math::position::BlockPos;
use pumpkin_util::math::vector3::Vector3;

use crate::entity::ai::brain::activity::ActivityData;
use crate::entity::ai::brain::memory::{GlobalPos, MemoryModuleId, PackedMemories, types};
use crate::entity::ai::brain::{Brain, BrainProvider};
use crate::entity::ai::pathfinder::{Navigator, NavigatorGoal};
use crate::entity::ai::util::land_random_pos;
use crate::entity::{
    Entity, EntityBase,
    ageable::{AgeableData, AgeableMob},
    ai::goal::{
        breed::BreedGoal, follow_parent::FollowParentGoal, look_around::RandomLookAroundGoal,
        look_at_entity::LookAtEntityGoal, swim::SwimGoal, tempt::TemptGoal,
        wander_around::WanderAroundGoal,
    },
    item::ItemEntity,
    mob::{Mob, MobEntity},
    passive::animal::Animal,
    player::Player,
};

pub const SNIFFER_FOOD: &[&Item] = &[&Item::TORCHFLOWER_SEEDS, &Item::PITCHER_POD];
pub const SNIFFER_BABY_START_AGE: i32 = -48000;
pub const DIGGING_DROP_SEED_OFFSET_TICKS: i32 = 120;
const SNIFFING_COOLDOWN_TICKS: i32 = 9600;
const DIGGING_MIN_TICKS: i32 = 160;
const DIGGING_MAX_TICKS: i32 = 180;
const RISING_TICKS: i32 = 40;
const SEARCHING_MAX_TICKS: i32 = 600;
const IDLE_SNIFF_CHANCE: i32 = 40;
const MAX_EXPLORED_POSITIONS: usize = 20;

static SNIFFER_MEMORIES: &[MemoryModuleId] = &[types::SNIFFER_EXPLORED_POSITIONS.id()];

fn sniffer_activities(_mob: &dyn Mob) -> Vec<ActivityData> {
    Vec::new()
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[repr(i32)]
pub enum SnifferState {
    #[default]
    Idling = 0,
    FeelingHappy = 1,
    Scenting = 2,
    Sniffing = 3,
    Searching = 4,
    Digging = 5,
    Rising = 6,
}

impl SnifferState {
    #[must_use]
    pub const fn from_id(id: i32) -> Self {
        match id {
            1 => Self::FeelingHappy,
            2 => Self::Scenting,
            3 => Self::Sniffing,
            4 => Self::Searching,
            5 => Self::Digging,
            6 => Self::Rising,
            _ => Self::Idling,
        }
    }

    #[must_use]
    pub const fn id(self) -> i32 {
        self as i32
    }
}

/// Represents a Sniffer, a large passive mob that digs up ancient seeds.
///
/// Wiki: <https://minecraft.wiki/w/Sniffer>
pub struct SnifferEntity {
    pub mob_entity: MobEntity,
    pub ageable_data: AgeableData,
    pub state: AtomicI32,
    pub drop_seed_at_tick: AtomicI32,
    state_ticks: AtomicI32,
    state_duration: AtomicI32,
    sniff_cooldown: AtomicI32,
    dig_target: AtomicCell<Option<BlockPos>>,
}

impl SnifferEntity {
    pub fn new(entity: Entity) -> Arc<Self> {
        let mob_entity = MobEntity::new(entity);
        {
            let mut navigator = Navigator::ground();
            navigator.set_can_float(true);
            *mob_entity
                .navigator
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner) = navigator;
        };
        let sniffer = Self {
            mob_entity,
            ageable_data: AgeableData::default(),
            state: AtomicI32::new(SnifferState::Idling.id()),
            drop_seed_at_tick: AtomicI32::new(0),
            state_ticks: AtomicI32::new(0),
            state_duration: AtomicI32::new(0),
            sniff_cooldown: AtomicI32::new(0),
            dig_target: AtomicCell::new(None),
        };
        let mob_arc = Arc::new(sniffer);
        *mob_arc
            .mob_entity
            .brain
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner) =
            mob_arc.make_brain(&PackedMemories::empty());
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

            goal_selector.add_goal(0, Box::new(SwimGoal::default()));
            goal_selector.add_goal(1, BreedGoal::new(1.0));
            goal_selector.add_goal(2, Box::new(TemptGoal::new(1.2, SNIFFER_FOOD, false)));
            goal_selector.add_goal(3, Box::new(FollowParentGoal::new(1.1)));
            goal_selector.add_goal(4, Box::new(WanderAroundGoal::new(1.0)));
            goal_selector.add_goal(
                5,
                LookAtEntityGoal::with_default(mob_weak, &EntityType::PLAYER, 6.0),
            );
            goal_selector.add_goal(6, Box::new(RandomLookAroundGoal::default()));
        };

        mob_arc
    }

    #[must_use]
    pub fn get_state(&self) -> SnifferState {
        SnifferState::from_id(self.state.load(Ordering::Relaxed))
    }

    pub fn set_state(&self, state: SnifferState) {
        self.state.store(state.id(), Ordering::Relaxed);
        let entity = self.get_entity();
        entity.set_synced_data(
            pumpkin_data::tracked_data::sniffer::STATE,
            VarInt(state.id()),
        );
    }

    pub fn transition_to(&self, state: SnifferState) {
        let entity = self.get_entity();
        let world = entity.world.load();
        let pos = entity.pos.load();

        self.state_ticks.store(0, Ordering::Relaxed);
        match state {
            SnifferState::Idling => {
                self.state_duration.store(0, Ordering::Relaxed);
                self.set_state(SnifferState::Idling);
            }
            SnifferState::FeelingHappy => {
                world.play_sound(Sound::EntitySnifferHappy, SoundCategory::Neutral, &pos);
                self.state_duration
                    .store(rand::random_range(40..=100), Ordering::Relaxed);
                self.set_state(SnifferState::FeelingHappy);
            }
            SnifferState::Scenting => {
                self.state_duration
                    .store(rand::random_range(40..=80), Ordering::Relaxed);
                self.set_state(SnifferState::Scenting);
                self.on_scenting_start();
            }
            SnifferState::Sniffing => {
                world.play_sound(Sound::EntitySnifferSniffing, SoundCategory::Neutral, &pos);
                self.state_duration
                    .store(rand::random_range(40..=80), Ordering::Relaxed);
                self.set_state(SnifferState::Sniffing);
            }
            SnifferState::Searching => {
                self.state_duration
                    .store(SEARCHING_MAX_TICKS, Ordering::Relaxed);
                self.set_state(SnifferState::Searching);
            }
            SnifferState::Digging => {
                self.state_duration.store(
                    rand::random_range(DIGGING_MIN_TICKS..=DIGGING_MAX_TICKS),
                    Ordering::Relaxed,
                );
                self.set_state(SnifferState::Digging);
                self.on_digging_start();
            }
            SnifferState::Rising => {
                world.play_sound(
                    Sound::EntitySnifferDiggingStop,
                    SoundCategory::Neutral,
                    &pos,
                );
                self.state_duration.store(RISING_TICKS, Ordering::Relaxed);
                self.set_state(SnifferState::Rising);
            }
        }
    }

    fn on_scenting_start(&self) {
        let entity = self.get_entity();
        let world = entity.world.load();
        let pos = entity.pos.load();
        let pitch = if self.is_baby() { 1.3 } else { 1.0 };
        world.play_sound_fine(
            Sound::EntitySnifferScenting,
            SoundCategory::Neutral,
            &pos,
            1.0,
            pitch,
        );
    }

    fn on_digging_start(&self) {
        let entity = self.get_entity();
        let world = entity.world.load();
        let current_ticks = world.level_time.try_lock().map_or(0, |t| t.world_age);
        let drop_tick = current_ticks as i32 + DIGGING_DROP_SEED_OFFSET_TICKS;
        self.drop_seed_at_tick.store(drop_tick, Ordering::Relaxed);
        entity.set_synced_data(
            pumpkin_data::tracked_data::sniffer::DROP_SEED_AT_TICK,
            VarInt(drop_tick),
        );
        world.send_entity_status(entity, EntityStatus::SnifferDiggingSound, None);
    }

    pub fn on_digging_complete(&self, success: bool) {
        if success {
            let pos = self
                .get_entity()
                .get_block_pos_below_that_affects_my_movement();
            self.store_explored_position(pos);
        }
    }

    /// Vanilla `Sniffer.tick`: the automatic scenting, sniffing, searching and digging cycle.
    fn tick_state(&self) {
        if self.sniff_cooldown.load(Ordering::Relaxed) > 0 {
            self.sniff_cooldown.fetch_sub(1, Ordering::Relaxed);
        }

        let ticks = self.state_ticks.fetch_add(1, Ordering::Relaxed) + 1;
        let duration = self.state_duration.load(Ordering::Relaxed);
        match self.get_state() {
            SnifferState::Idling => {
                if self.sniff_cooldown.load(Ordering::Relaxed) == 0
                    && self.can_sniff()
                    && rand::random_range(0..IDLE_SNIFF_CHANCE) == 0
                {
                    self.transition_to(SnifferState::Scenting);
                }
            }
            SnifferState::Scenting => {
                if ticks >= duration {
                    self.transition_to(SnifferState::Sniffing);
                }
            }
            SnifferState::Sniffing => {
                if !self.can_sniff() {
                    self.transition_to(SnifferState::Idling);
                    return;
                }
                if ticks >= duration {
                    if let Some(target) = self.calculate_dig_position() {
                        self.dig_target.store(Some(target));
                        let entity = self.get_entity();
                        let mob_pos = entity.pos.load();
                        let destination = Vector3::new(
                            f64::from(target.0.x) + 0.5,
                            f64::from(target.0.y),
                            f64::from(target.0.z) + 0.5,
                        );
                        self.mob_entity
                            .navigator
                            .lock()
                            .unwrap_or_else(std::sync::PoisonError::into_inner)
                            .set_progress(NavigatorGoal::new(mob_pos, destination, 1.25));
                        self.transition_to(SnifferState::Searching);
                    } else {
                        self.transition_to(SnifferState::Idling);
                    }
                }
            }
            SnifferState::Searching => {
                if !self.can_sniff() {
                    self.stop_navigation();
                    self.transition_to(SnifferState::Idling);
                    return;
                }
                let target = self.dig_target.load();
                if target.is_none() || ticks >= duration {
                    self.stop_navigation();
                    self.transition_to(SnifferState::Idling);
                } else if let Some(target) = target {
                    let entity = self.get_entity();
                    let pos = entity.pos.load();
                    let target_center = Vector3::new(
                        f64::from(target.0.x) + 0.5,
                        f64::from(target.0.y),
                        f64::from(target.0.z) + 0.5,
                    );
                    if pos.squared_distance_to_vec(&target_center) <= 2.25
                        && self.can_dig_at(target.down())
                    {
                        self.stop_navigation();
                        self.transition_to(SnifferState::Digging);
                    } else if !self.can_dig_at(target.down()) {
                        self.stop_navigation();
                        self.transition_to(SnifferState::Idling);
                    }
                }
            }
            SnifferState::Digging => {
                if !self.can_dig() {
                    self.transition_to(SnifferState::Idling);
                    return;
                }
                self.drop_seed();
                if ticks >= duration {
                    self.transition_to(SnifferState::Rising);
                }
            }
            SnifferState::Rising => {
                if ticks >= duration {
                    self.on_digging_complete(true);
                    self.sniff_cooldown
                        .store(SNIFFING_COOLDOWN_TICKS, Ordering::Relaxed);
                    self.dig_target.store(None);
                    self.transition_to(SnifferState::FeelingHappy);
                }
            }
            SnifferState::FeelingHappy => {
                if ticks >= duration {
                    self.transition_to(SnifferState::Idling);
                }
            }
        }
    }

    fn stop_navigation(&self) {
        self.mob_entity
            .navigator
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .stop();
    }

    #[must_use]
    pub fn can_sniff(&self) -> bool {
        let entity = self.get_entity();
        !self.is_tempted()
            && !self.is_panicking()
            && !self.mob_entity.living_entity.is_in_water()
            && !self.is_in_love()
            && entity.on_ground.load(Ordering::Relaxed)
            && !entity.has_vehicle()
            && !entity.is_leashed()
    }

    /// Vanilla `Sniffer.isTempted`: a nearby player holding sniffer food.
    #[must_use]
    pub fn is_tempted(&self) -> bool {
        let living = &self.mob_entity.living_entity;
        let range = living.get_attribute_value(&Attributes::TEMPT_RANGE);
        let world = living.entity.world.load();
        world
            .get_nearest_player(living.entity.pos.load(), range, |player| {
                player.living_entity.is_part_of_game()
                    && [
                        player.inventory().held_item(),
                        player.inventory().off_hand_item(),
                    ]
                    .iter()
                    .any(|stack| stack.item.has_tag(&tag::Item::MINECRAFT_SNIFFER_FOOD))
            })
            .is_some()
    }

    /// Vanilla `Sniffer.calculateDigPosition`, trying wider spots until a diggable one is found.
    fn calculate_dig_position(&self) -> Option<BlockPos> {
        for idx in 0..5 {
            let Some(pos) = land_random_pos::get_pos(self, 10 + 2 * idx, 3) else {
                continue;
            };
            let block = BlockPos::floored(pos.x, pos.y, pos.z);
            if self.can_dig_at(block.down()) {
                return Some(block);
            }
        }
        None
    }

    #[must_use]
    pub fn get_head_position(&self) -> Vector3<f64> {
        let entity = self.get_entity();
        let pos = entity.pos.load();
        let yaw = f64::from(entity.yaw.load());
        let yaw_rad = yaw.to_radians();
        let forward = Vector3::new(-yaw_rad.sin(), 0.0, yaw_rad.cos());
        pos + forward * 2.25
    }

    #[must_use]
    pub fn get_head_block(&self) -> BlockPos {
        let head_pos = self.get_head_position();
        let entity = self.get_entity();
        let pos = entity.pos.load();
        BlockPos::floored(head_pos.x, pos.y + 0.2, head_pos.z)
    }

    #[must_use]
    pub fn can_dig(&self) -> bool {
        let entity = self.get_entity();
        !self.is_tempted()
            && !self.is_panicking()
            && !self.is_baby()
            && !self.mob_entity.living_entity.is_in_water()
            && entity.on_ground.load(Ordering::Relaxed)
            && !entity.has_vehicle()
            && self.is_diggable_block(self.get_head_block().down())
    }

    fn is_diggable_block(&self, pos: BlockPos) -> bool {
        let entity = self.get_entity();
        let world = entity.world.load();
        let block_state = world.get_block_state(&pos);
        let block = pumpkin_data::Block::from_state_id(block_state.id);

        block.has_tag(&tag::Block::MINECRAFT_SNIFFER_DIGGABLE_BLOCK)
            && !self
                .explored_positions()
                .iter()
                .any(|explored| explored.pos == pos)
    }

    fn can_dig_at(&self, pos: BlockPos) -> bool {
        if !self.is_diggable_block(pos) {
            return false;
        }

        let destination = Vector3::new(
            f64::from(pos.0.x) + 0.5,
            f64::from(pos.0.y),
            f64::from(pos.0.z) + 0.5,
        );
        self.mob_entity
            .navigator
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .can_reach_within(&self.mob_entity.living_entity, destination, 1.0)
    }

    fn explored_positions(&self) -> Vec<GlobalPos> {
        self.mob_entity
            .brain
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .get(types::SNIFFER_EXPLORED_POSITIONS)
            .cloned()
            .unwrap_or_default()
    }

    fn store_explored_position(&self, pos: BlockPos) {
        let entity = self.get_entity();
        let world = entity.world.load();
        let Some(dimension) =
            pumpkin_data::dimension::Dimension::from_name(world.dimension.minecraft_name)
        else {
            return;
        };
        let mut explored = self.explored_positions();
        explored.truncate(MAX_EXPLORED_POSITIONS - 1);
        explored.insert(0, GlobalPos::new(dimension, pos));
        self.mob_entity
            .brain
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .set(types::SNIFFER_EXPLORED_POSITIONS, explored);
    }

    pub fn drop_seed(&self) {
        let entity = self.get_entity();
        let world = entity.world.load_full();
        let current_tick = world.get_world_age() as i32;

        if self.drop_seed_at_tick.load(Ordering::Relaxed) == current_tick {
            let head_pos = self.get_head_position();
            if world.level_info.load().game_rules.mob_drops
                && let Some(loot_table) = world.get_loot_table("minecraft:gameplay/sniffer_digging")
            {
                for item_stack in loot_table.generate_loot(rand::random::<i64>()) {
                    let item_entity = Entity::new(world.clone(), head_pos, &EntityType::ITEM);
                    let item_arc = Arc::new(ItemEntity::new(item_entity, item_stack));
                    world.spawn_entity(item_arc as Arc<dyn EntityBase>);
                }
            }
            world.play_sound(
                Sound::EntitySnifferDropSeed,
                SoundCategory::Neutral,
                &head_pos,
            );
        }
    }
}

impl AgeableMob for SnifferEntity {
    fn get_ageable_data(&self) -> &AgeableData {
        &self.ageable_data
    }

    fn get_baby_start_age(&self) -> i32 {
        SNIFFER_BABY_START_AGE
    }
}

impl Animal for SnifferEntity {
    fn is_food(&self, item_stack: &ItemStack) -> bool {
        item_stack.item.has_tag(&tag::Item::MINECRAFT_SNIFFER_FOOD)
            || SNIFFER_FOOD.iter().any(|i| i.id == item_stack.item.id)
    }

    fn can_breed_with(&self, partner: &dyn EntityBase) -> bool {
        let Some(partner) = partner.cast_any().downcast_ref::<Self>() else {
            return false;
        };
        let is_ready = |state: SnifferState| {
            matches!(
                state,
                SnifferState::Idling | SnifferState::Scenting | SnifferState::FeelingHappy
            )
        };
        is_ready(self.get_state()) && is_ready(partner.get_state())
    }

    /// Vanilla `Sniffer.spawnChildFromBreeding`: lays a sniffer egg instead of a live child.
    fn spawn_child_from_breeding(&self, partner: &dyn EntityBase) {
        let entity = self.get_entity();
        let world = entity.world.load_full();
        let pos = entity.pos.load();

        let item_stack = ItemStack::new(1, &Item::SNIFFER_EGG);
        let egg_entity = Entity::new(world.clone(), pos, &EntityType::ITEM);
        let egg_arc = Arc::new(ItemEntity::new(egg_entity, item_stack));

        self.finalize_spawn_child_from_breeding(partner);
        world.play_sound(Sound::BlockSnifferEggPlop, SoundCategory::Neutral, &pos);
        world.spawn_entity(egg_arc as Arc<dyn EntityBase>);
    }
}

impl Mob for SnifferEntity {
    fn as_ageable(&self) -> Option<&dyn AgeableMob> {
        Some(self)
    }

    fn as_animal(&self) -> Option<&dyn Animal> {
        Some(self)
    }

    fn mob_write_nbt(&self, _nbt: &mut NbtCompound) {}

    fn mob_read_nbt(&self, nbt: &NbtCompound) {
        if nbt.get_compound("Brain").is_none() {
            *self
                .mob_entity
                .brain
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner) =
                self.make_brain(&PackedMemories::empty());
        }
    }

    fn get_mob_entity(&self) -> &MobEntity {
        &self.mob_entity
    }

    fn get_ambient_sound(&self) -> Option<Sound> {
        if matches!(
            self.get_state(),
            SnifferState::Digging | SnifferState::Searching
        ) {
            None
        } else {
            Some(Sound::EntitySnifferIdle)
        }
    }

    fn get_max_head_rotation(&self) -> f32 {
        50.0
    }

    fn make_brain(&self, packed: &PackedMemories) -> Brain {
        BrainProvider {
            memory_types: SNIFFER_MEMORIES,
            sensor_types: &[],
            activities: sniffer_activities,
        }
        .make_brain(self, packed)
    }

    fn mob_tick(&self, _caller: &dyn EntityBase) {
        self.ageable_ai_step();
        if self.is_panicking() && self.get_state() != SnifferState::Idling {
            self.stop_navigation();
            self.transition_to(SnifferState::Idling);
            return;
        }
        if self.is_in_love() && self.get_state() != SnifferState::Idling {
            self.stop_navigation();
            self.transition_to(SnifferState::Idling);
            return;
        }
        self.tick_state();
    }

    fn mob_init_data_tracker(&self) {
        let entity = self.get_entity();
        let is_baby = entity.age.load(Ordering::Relaxed) < 0;
        if is_baby {
            entity.set_synced_data(pumpkin_data::tracked_data::sniffer::BABY_ID, true);
        }
        entity.set_synced_data(
            pumpkin_data::tracked_data::sniffer::STATE,
            VarInt(self.get_state().id()),
        );
        entity.set_synced_data(
            pumpkin_data::tracked_data::sniffer::DROP_SEED_AT_TICK,
            VarInt(self.drop_seed_at_tick.load(Ordering::Relaxed)),
        );
    }

    fn mob_interact(&self, player: &Arc<Player>, item_stack: &mut ItemStack) -> bool {
        self.animal_interact(player, item_stack, Sound::EntitySnifferEat)
    }
}
