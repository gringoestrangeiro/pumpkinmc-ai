use std::sync::{
    Arc, Weak,
    atomic::{AtomicI32, AtomicI64, Ordering},
};

use pumpkin_data::block_properties::{
    CopperGolemPose, CopperGolemStatueLikeProperties, EnumVariants,
};
use pumpkin_data::entity::EntityType;
use pumpkin_data::item::Item;
use pumpkin_data::item_stack::ItemStack;
use pumpkin_data::sound::{Sound, SoundCategory};
use pumpkin_data::tag::{self, Taggable};
use pumpkin_data::{Block, BlockId};
use pumpkin_nbt::compound::NbtCompound;
use pumpkin_protocol::codec::var_int::VarInt;
use pumpkin_util::math::position::BlockPos;
use pumpkin_util::math::vector2::Vector2;
use pumpkin_util::math::vector3::Vector3;
use pumpkin_world::world::BlockFlags;

use crate::block::entities::BlockEntity;
use crate::block::entities::chest::ChestBlockEntity;
use crate::block::entities::copper_golem_statue::CopperGolemStatueBlockEntity;
use crate::entity::ai::goal::{Controls, Goal};
use crate::entity::ai::pathfinder::{NavigatorGoal, node::PathType};
use crate::entity::{
    Entity, EntityBase,
    ai::goal::{
        look_around::RandomLookAroundGoal, look_at_entity::LookAtEntityGoal, swim::SwimGoal,
        wander_around::WanderAroundGoal,
    },
    custom_sound::CustomSound,
    mob::{Mob, MobEntity},
    player::Player,
};
use pumpkin_inventory::Inventory;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[repr(i32)]
pub enum WeatherState {
    #[default]
    Unaffected = 0,
    Exposed = 1,
    Weathered = 2,
    Oxidized = 3,
}

impl WeatherState {
    #[must_use]
    pub const fn from_id(id: i32) -> Self {
        match id {
            1 => Self::Exposed,
            2 => Self::Weathered,
            3 => Self::Oxidized,
            _ => Self::Unaffected,
        }
    }

    #[must_use]
    pub const fn id(self) -> i32 {
        self as i32
    }

    #[must_use]
    pub const fn next(self) -> Self {
        match self {
            Self::Unaffected => Self::Exposed,
            Self::Exposed => Self::Weathered,
            Self::Weathered | Self::Oxidized => Self::Oxidized,
        }
    }

    #[must_use]
    pub const fn previous(self) -> Self {
        match self {
            Self::Oxidized => Self::Weathered,
            Self::Weathered => Self::Exposed,
            Self::Exposed | Self::Unaffected => Self::Unaffected,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[repr(i32)]
pub enum CopperGolemState {
    #[default]
    Idle = 0,
    GettingItem = 1,
    GettingNoItem = 2,
    DroppingItem = 3,
    DroppingNoItem = 4,
}

impl CopperGolemState {
    #[must_use]
    pub const fn from_id(id: i32) -> Self {
        match id {
            1 => Self::GettingItem,
            2 => Self::GettingNoItem,
            3 => Self::DroppingItem,
            4 => Self::DroppingNoItem,
            _ => Self::Idle,
        }
    }

    #[must_use]
    pub const fn id(self) -> i32 {
        self as i32
    }
}

/// Represents a Copper Golem, a passive creation mob made of copper blocks and lightning rod.
///
/// Wiki: <https://minecraft.wiki/w/Copper_Golem>
pub struct CopperGolemEntity {
    pub mob_entity: MobEntity,
    pub weather_state: AtomicI32,
    pub state: AtomicI32,
    pub next_weathering_tick: AtomicI64,
    pub transport_cooldown: AtomicI32,
}

/// Vanilla `CopperGolem.WEATHERING_TICK_FROM` and `WEATHERING_TICK_TO`.
const WEATHERING_TICK_FROM: i32 = 504_000;
const WEATHERING_TICK_TO: i32 = 552_000;
const TURN_TO_STATUE_CHANCE: f32 = 0.0058;
const TRANSPORT_ITEM_HORIZONTAL_SEARCH_RADIUS: i32 = 32;
const TRANSPORT_ITEM_VERTICAL_SEARCH_RADIUS: i32 = 8;
const TRANSPORTED_ITEM_MAX_STACK_SIZE: u8 = 16;
const TARGET_INTERACTION_TIME: i32 = 60;
const IDLE_COOLDOWN: i32 = 140;
const VISITED_POSITIONS_MEMORY_TIME: i64 = 6000;
const MAX_VISITED_POSITIONS: usize = 10;
const MAX_UNREACHABLE_POSITIONS: usize = 50;
const CLOSE_ENOUGH_TO_START_QUEUING_DISTANCE: f64 = 3.0;
const CLOSE_ENOUGH_TO_START_INTERACTING_WITH_TARGET_DISTANCE: f64 = 1.5;
const CLOSE_ENOUGH_TO_CONTINUE_INTERACTING_WITH_TARGET: f64 = 2.0;
const SPEED_MULTIPLIER_WHEN_IDLING: f64 = 1.0;

impl CopperGolemEntity {
    pub fn new(entity: Entity) -> Arc<Self> {
        let mob_entity = MobEntity::new(entity);
        let golem = Self {
            mob_entity,
            weather_state: AtomicI32::new(WeatherState::Unaffected.id()),
            state: AtomicI32::new(CopperGolemState::Idle.id()),
            next_weathering_tick: AtomicI64::new(-1),
            transport_cooldown: AtomicI32::new(rand::random_range(60..100)),
        };
        let mob_arc = Arc::new(golem);
        let mob_weak: Weak<dyn Mob> = {
            let mob_arc: Arc<dyn Mob> = mob_arc.clone();
            Arc::downgrade(&mob_arc)
        };
        let golem_weak = Arc::downgrade(&mob_arc);

        {
            let mut navigator = mob_arc
                .mob_entity
                .navigator
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            navigator.set_required_path_length(48.0);
            navigator.set_can_open_doors(true);
            navigator.set_pathfinding_malus(PathType::DangerFire, 16.0);
            navigator.set_pathfinding_malus(PathType::DangerOther, 16.0);
            navigator.set_pathfinding_malus(PathType::DamageFire, -1.0);
        }

        {
            let mut goal_selector = mob_arc
                .mob_entity
                .goals_selector
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);

            goal_selector.add_goal(0, Box::new(SwimGoal::default()));
            goal_selector.add_goal(
                0,
                Box::new(TransportItemsBetweenContainersGoal::new(golem_weak)),
            );
            goal_selector.add_goal(1, Box::new(WanderAroundGoal::new(1.0)));
            goal_selector.add_goal(
                2,
                LookAtEntityGoal::with_default(mob_weak, &EntityType::PLAYER, 6.0),
            );
            goal_selector.add_goal(3, Box::new(RandomLookAroundGoal::default()));
        };

        mob_arc
    }

    #[must_use]
    pub fn get_weather_state(&self) -> WeatherState {
        WeatherState::from_id(self.weather_state.load(Ordering::Relaxed))
    }

    pub fn set_weather_state(&self, state: WeatherState) {
        self.weather_state.store(state.id(), Ordering::Relaxed);
        let entity = self.get_entity();
        entity.set_synced_data(
            pumpkin_data::tracked_data::copper_golem::WEATHER_STATE,
            VarInt(state.id()),
        );
    }

    #[must_use]
    pub fn get_state(&self) -> CopperGolemState {
        CopperGolemState::from_id(self.state.load(Ordering::Relaxed))
    }

    pub fn set_state(&self, state: CopperGolemState) {
        self.state.store(state.id(), Ordering::Relaxed);
        let entity = self.get_entity();
        entity.set_synced_data(
            pumpkin_data::tracked_data::copper_golem::COPPER_GOLEM_STATE,
            VarInt(state.id()),
        );
    }

    fn update_weathering(&self, world: &Arc<crate::world::World>) {
        let next = self.next_weathering_tick.load(Ordering::Relaxed);
        if next == -2 {
            return;
        }
        let game_time = world.get_world_age();
        if next == -1 {
            self.next_weathering_tick.store(
                game_time
                    + i64::from(rand::random_range(
                        WEATHERING_TICK_FROM..=WEATHERING_TICK_TO,
                    )),
                Ordering::Relaxed,
            );
            return;
        }

        let weather_state = self.get_weather_state();
        let is_fully_oxidized = weather_state == WeatherState::Oxidized;
        if game_time >= next && !is_fully_oxidized {
            let new_state = weather_state.next();
            self.set_weather_state(new_state);
            self.next_weathering_tick.store(
                if new_state == WeatherState::Oxidized {
                    0
                } else {
                    next + i64::from(rand::random_range(
                        WEATHERING_TICK_FROM..=WEATHERING_TICK_TO,
                    ))
                },
                Ordering::Relaxed,
            );
        }

        if is_fully_oxidized && self.can_turn_to_statue(world) {
            self.turn_to_statue(world);
        }
    }

    fn can_turn_to_statue(&self, world: &crate::world::World) -> bool {
        let pos = self.get_entity().block_pos.load();
        world.get_block_state(&pos).is_air() && rand::random::<f32>() <= TURN_TO_STATUE_CHANCE
    }

    fn turn_to_statue(&self, world: &Arc<crate::world::World>) {
        let pos = self.get_entity().block_pos.load();
        let block = &Block::OXIDIZED_COPPER_GOLEM_STATUE;
        let mut props = CopperGolemStatueLikeProperties::default(block);
        props.copper_golem_pose = CopperGolemPose::from_index(rand::random_range(0..4));
        props.facing = self.get_entity().get_horizontal_facing();
        world.set_block_state(&pos, props.to_state_id(block), BlockFlags::NOTIFY_ALL);

        if let Some(block_entity) = world.get_block_entity(&pos)
            && let Some(statue) = block_entity
                .as_any()
                .downcast_ref::<CopperGolemStatueBlockEntity>()
            && let Some(name) = (**self.get_entity().custom_name.load()).clone()
        {
            *statue
                .custom_name
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner) = Some(name.get_text());
        }

        let entity = self.get_entity();
        let antenna = self
            .mob_entity
            .living_entity
            .entity_equipment
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .put(
                &pumpkin_data::data_component_impl::EquipmentSlot::SADDLE,
                ItemStack::EMPTY.clone(),
            );
        if !antenna.is_empty() {
            entity
                .world
                .load_full()
                .drop_stack(&entity.block_pos.load(), antenna);
        }
        world.play_sound(
            Sound::EntityCopperGolemBecomeStatue,
            SoundCategory::Neutral,
            &pos.to_centered_f64(),
        );
        if entity.is_leashed() {
            entity.unleash();
        }
        entity.remove();
    }

    #[must_use]
    pub fn step_sound(&self) -> Sound {
        match self.get_weather_state() {
            WeatherState::Unaffected | WeatherState::Exposed => Sound::EntityCopperGolemStep,
            WeatherState::Weathered => Sound::EntityCopperGolemWeatheredStep,
            WeatherState::Oxidized => Sound::EntityCopperGolemOxidizedStep,
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum TransportItemState {
    Travelling,
    Queuing,
    Interacting,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum ContainerInteractionState {
    PickupItem,
    PickupNoItem,
    PlaceItem,
    PlaceNoItem,
}

struct TransportItemsBetweenContainersGoal {
    golem: Weak<CopperGolemEntity>,
    target: Option<BlockPos>,
    state: TransportItemState,
    interaction: Option<ContainerInteractionState>,
    ticks_since_reaching_target: i32,
    opened_chest: Option<BlockPos>,
    visited: Vec<(BlockPos, i64)>,
    unreachable: Vec<(BlockPos, i64)>,
}

impl TransportItemsBetweenContainersGoal {
    const fn new(golem: Weak<CopperGolemEntity>) -> Self {
        Self {
            golem,
            target: None,
            state: TransportItemState::Travelling,
            interaction: None,
            ticks_since_reaching_target: 0,
            opened_chest: None,
            visited: Vec::new(),
            unreachable: Vec::new(),
        }
    }

    fn main_hand_item(golem: &CopperGolemEntity) -> ItemStack {
        golem
            .mob_entity
            .living_entity
            .entity_equipment
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .get(&pumpkin_data::data_component_impl::EquipmentSlot::MAIN_HAND)
    }

    fn target_center(pos: &BlockPos) -> Vector3<f64> {
        pos.to_centered_f64()
    }

    fn distance_to_target(golem: &CopperGolemEntity, target: &BlockPos) -> f64 {
        let entity = &golem.mob_entity.living_entity.entity;
        let center = entity.pos.load() + Vector3::new(0.0, f64::from(entity.height()) / 2.0, 0.0);
        center
            .squared_distance_to_vec(&Self::target_center(target))
            .sqrt()
    }

    fn is_wanted_block(golem: &CopperGolemEntity, block: &Block) -> bool {
        if Self::main_hand_item(golem).is_empty() {
            block.has_tag(&tag::Block::MINECRAFT_COPPER_CHESTS)
        } else {
            block.id == BlockId::CHEST || block.id == BlockId::TRAPPED_CHEST
        }
    }

    fn target_is_valid(golem: &CopperGolemEntity, target: &BlockPos) -> bool {
        let world = golem.get_entity().world.load();
        let block = world.get_block(target);
        Self::is_wanted_block(golem, block) && world.get_block_entity(target).is_some()
    }

    fn is_chest_busy(world: &crate::world::World, pos: &BlockPos) -> bool {
        world
            .get_block_entity(pos)
            .and_then(|be| {
                be.as_any()
                    .downcast_ref::<ChestBlockEntity>()
                    .map(ChestBlockEntity::get_viewer_count)
            })
            .is_some_and(|viewers| viewers > 0)
    }

    fn is_recently_listed(list: &[(BlockPos, i64)], pos: &BlockPos, now: i64) -> bool {
        list.iter()
            .any(|(entry, expiry)| entry == pos && *expiry > now)
    }

    fn select_target(&mut self, golem: &CopperGolemEntity) -> bool {
        let entity = &golem.mob_entity.living_entity.entity;
        let world = entity.world.load();
        let origin = entity.block_pos.load();
        let now = world.get_world_age();
        self.visited.retain(|(_, expiry)| *expiry > now);
        self.unreachable.retain(|(_, expiry)| *expiry > now);

        let chunk_radius = TRANSPORT_ITEM_HORIZONTAL_SEARCH_RADIUS / 16 + 1;
        let center_chunk = Vector2::new(origin.0.x >> 4, origin.0.z >> 4);
        let mut candidates: Vec<(f64, BlockPos)> = Vec::new();

        for dx in -chunk_radius..=chunk_radius {
            for dz in -chunk_radius..=chunk_radius {
                let chunk_pos = Vector2::new(center_chunk.x + dx, center_chunk.y + dz);
                let Some(chunk) = world.block_entities.get(&chunk_pos) else {
                    continue;
                };
                for block_entity in chunk.values() {
                    let pos = block_entity.get_position();
                    if (pos.0.x - origin.0.x).abs() > TRANSPORT_ITEM_HORIZONTAL_SEARCH_RADIUS
                        || (pos.0.y - origin.0.y).abs() > TRANSPORT_ITEM_VERTICAL_SEARCH_RADIUS
                        || (pos.0.z - origin.0.z).abs() > TRANSPORT_ITEM_HORIZONTAL_SEARCH_RADIUS
                    {
                        continue;
                    }
                    if !Self::is_wanted_block(golem, world.get_block(&pos))
                        || Self::is_recently_listed(&self.visited, &pos, now)
                        || Self::is_recently_listed(&self.unreachable, &pos, now)
                        || Arc::clone(block_entity).get_inventory().is_none()
                    {
                        continue;
                    }
                    candidates.push((f64::from(origin.squared_distance(&pos)), pos));
                }
            }
        }

        candidates.sort_by(|a, b| a.0.total_cmp(&b.0));
        let Some((_, target)) = candidates.first().copied() else {
            return false;
        };
        self.target = Some(target);
        self.state = TransportItemState::Travelling;
        self.visited
            .push((target, now + VISITED_POSITIONS_MEMORY_TIME));
        if self.visited.len() > MAX_VISITED_POSITIONS {
            golem
                .transport_cooldown
                .store(IDLE_COOLDOWN, Ordering::Relaxed);
            self.visited.clear();
            self.unreachable.clear();
            self.target = None;
            return false;
        }
        true
    }

    fn mark_unreachable(&mut self, golem: &CopperGolemEntity, pos: BlockPos) {
        self.on_start_travelling(golem);
        let now = golem.get_entity().world.load().get_world_age();
        self.visited.retain(|(entry, _)| *entry != pos);
        self.unreachable
            .push((pos, now + VISITED_POSITIONS_MEMORY_TIME));
        if self.unreachable.len() > MAX_UNREACHABLE_POSITIONS {
            golem
                .transport_cooldown
                .store(IDLE_COOLDOWN, Ordering::Relaxed);
            self.visited.clear();
            self.unreachable.clear();
        }
        self.target = None;
    }

    fn clear_memories(&mut self, golem: &CopperGolemEntity) {
        self.target = None;
        self.visited.clear();
        self.unreachable.clear();
        golem
            .mob_entity
            .navigator
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .stop();
    }

    fn close_opened_chest(&mut self, golem: &CopperGolemEntity) {
        let Some(pos) = self.opened_chest.take() else {
            return;
        };
        if let Some(inventory) = golem
            .get_entity()
            .world
            .load()
            .get_block_entity(&pos)
            .and_then(|be| be.get_inventory())
        {
            inventory.on_close();
        }
    }

    fn on_start_travelling(&mut self, golem: &CopperGolemEntity) {
        self.close_opened_chest(golem);
        golem.set_state(CopperGolemState::Idle);
        self.state = TransportItemState::Travelling;
        self.interaction = None;
        self.ticks_since_reaching_target = 0;
    }

    fn stop_targeting_current_target(&mut self, golem: &CopperGolemEntity) {
        self.on_start_travelling(golem);
        self.target = None;
        golem
            .mob_entity
            .navigator
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .stop();
    }

    fn interaction_state_for(
        golem: &CopperGolemEntity,
        inventory: &Arc<dyn Inventory>,
    ) -> ContainerInteractionState {
        let hand = Self::main_hand_item(golem);
        if hand.is_empty() {
            if inventory.is_empty() {
                ContainerInteractionState::PickupNoItem
            } else {
                ContainerInteractionState::PickupItem
            }
        } else if inventory.is_empty() || has_item_matching_hand(inventory.as_ref(), &hand) {
            ContainerInteractionState::PlaceItem
        } else {
            ContainerInteractionState::PlaceNoItem
        }
    }

    fn set_interaction_state(golem: &CopperGolemEntity, state: ContainerInteractionState) {
        golem.set_state(match state {
            ContainerInteractionState::PickupItem => CopperGolemState::GettingItem,
            ContainerInteractionState::PickupNoItem => CopperGolemState::GettingNoItem,
            ContainerInteractionState::PlaceItem => CopperGolemState::DroppingItem,
            ContainerInteractionState::PlaceNoItem => CopperGolemState::DroppingNoItem,
        });
    }

    const fn interaction_sound(state: ContainerInteractionState) -> Sound {
        // Vanilla registers `COPPER_GOLEM_ITEM_GET` on `entity.copper_golem.no_item_get`.
        match state {
            ContainerInteractionState::PickupItem => Sound::EntityCopperGolemNoItemGet,
            ContainerInteractionState::PickupNoItem => Sound::EntityCopperGolemNoItemNoGet,
            ContainerInteractionState::PlaceItem => Sound::EntityCopperGolemItemDrop,
            ContainerInteractionState::PlaceNoItem => Sound::EntityCopperGolemItemNoDrop,
        }
    }

    fn tick_interaction(&mut self, golem: &CopperGolemEntity) {
        let Some(target) = self.target else {
            return;
        };
        if Self::distance_to_target(golem, &target)
            > CLOSE_ENOUGH_TO_CONTINUE_INTERACTING_WITH_TARGET
        {
            self.on_start_travelling(golem);
            return;
        }

        golem
            .mob_entity
            .navigator
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .stop();
        golem
            .mob_entity
            .look_control
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .look_at_position(golem, Self::target_center(&target));

        self.ticks_since_reaching_target += 1;
        let world = golem.get_entity().world.load();
        let Some(inventory) = world
            .get_block_entity(&target)
            .and_then(|block_entity| block_entity.get_inventory())
        else {
            self.mark_unreachable(golem, target);
            return;
        };

        if self.ticks_since_reaching_target == 1 {
            inventory.on_open();
            self.opened_chest = Some(target);
            let interaction = Self::interaction_state_for(golem, &inventory);
            self.interaction = Some(interaction);
            Self::set_interaction_state(golem, interaction);
        }

        if self.ticks_since_reaching_target == 9
            && let Some(interaction) = self.interaction
        {
            world.play_sound(
                Self::interaction_sound(interaction),
                SoundCategory::Neutral,
                &target.to_centered_f64(),
            );
        }

        if self.ticks_since_reaching_target >= TARGET_INTERACTION_TIME {
            let completed = self.finish_interaction(golem, &inventory);
            self.close_opened_chest(golem);
            golem.set_state(CopperGolemState::Idle);
            self.state = TransportItemState::Travelling;
            self.interaction = None;
            self.ticks_since_reaching_target = 0;
            if completed {
                self.clear_memories(golem);
            }
        }
    }

    fn finish_interaction(
        &mut self,
        golem: &CopperGolemEntity,
        inventory: &Arc<dyn Inventory>,
    ) -> bool {
        let hand = Self::main_hand_item(golem);
        if hand.is_empty() {
            let Some(slot) =
                (0..inventory.size()).find(|slot| !inventory.get_stack(*slot).is_empty())
            else {
                self.target = None;
                return false;
            };
            let count = inventory
                .get_stack(slot)
                .item_count
                .min(TRANSPORTED_ITEM_MAX_STACK_SIZE);
            let picked = inventory.remove_stack_specific(slot, count);
            inventory.mark_dirty();
            golem.mob_entity.set_item_slot_and_drop_when_killed(
                &pumpkin_data::data_component_impl::EquipmentSlot::MAIN_HAND,
                picked,
            );
            return true;
        }

        let leftover = add_items_to_container(inventory.as_ref(), hand);
        let leftover_is_empty = leftover.is_empty();
        golem.mob_entity.set_item_slot(
            &pumpkin_data::data_component_impl::EquipmentSlot::MAIN_HAND,
            leftover,
        );
        inventory.mark_dirty();
        if leftover_is_empty {
            return true;
        }
        self.target = None;
        false
    }
}

fn has_item_matching_hand(inventory: &dyn Inventory, hand: &ItemStack) -> bool {
    (0..inventory.size()).any(|slot| {
        let stack = inventory.get_stack(slot);
        !stack.is_empty() && stack.item.id == hand.item.id
    })
}

fn add_items_to_container(inventory: &dyn Inventory, mut stack: ItemStack) -> ItemStack {
    for slot in 0..inventory.size() {
        let mut container_stack = inventory.get_stack(slot);
        if container_stack.is_empty() {
            inventory.set_stack(slot, stack);
            return ItemStack::EMPTY.clone();
        }

        if container_stack.are_items_and_components_equal(&stack)
            && container_stack.item_count < container_stack.get_max_stack_size()
        {
            let count_that_can_be_added =
                container_stack.get_max_stack_size() - container_stack.item_count;
            let count_to_add = count_that_can_be_added.min(stack.item_count);
            container_stack.item_count += count_to_add;
            stack.item_count -= count_to_add;
            inventory.set_stack(slot, container_stack);
            if stack.is_empty() {
                return ItemStack::EMPTY.clone();
            }
        }
    }
    stack
}

impl Goal for TransportItemsBetweenContainersGoal {
    fn can_start(&mut self, mob: &dyn Mob) -> bool {
        let Some(golem) = self.golem.upgrade() else {
            return false;
        };
        golem.transport_cooldown.load(Ordering::Relaxed) == 0
            && !mob.is_panicking()
            && !mob.get_entity().is_leashed()
    }

    fn should_continue(&mut self, mob: &dyn Mob) -> bool {
        self.can_start(mob)
    }

    fn start(&mut self, mob: &dyn Mob) {
        mob.get_mob_entity()
            .navigator
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .set_can_path_to_targets_below_surface(true);
        self.state = TransportItemState::Travelling;
        self.target = None;
        self.interaction = None;
        self.ticks_since_reaching_target = 0;
    }

    fn tick(&mut self, mob: &dyn Mob) {
        let Some(golem) = self.golem.upgrade() else {
            return;
        };
        if let Some(target) = self.target
            && !Self::target_is_valid(&golem, &target)
        {
            self.stop_targeting_current_target(&golem);
            return;
        }
        if self.target.is_none() && !self.select_target(&golem) {
            golem
                .transport_cooldown
                .store(IDLE_COOLDOWN, Ordering::Relaxed);
            return;
        }

        let Some(target) = self.target else {
            return;
        };
        let distance = Self::distance_to_target(&golem, &target);
        match self.state {
            TransportItemState::Travelling => {
                let world = golem.get_entity().world.load();
                if distance < CLOSE_ENOUGH_TO_START_QUEUING_DISTANCE
                    && Self::is_chest_busy(&world, &target)
                {
                    self.state = TransportItemState::Queuing;
                    mob.get_mob_entity()
                        .navigator
                        .lock()
                        .unwrap_or_else(std::sync::PoisonError::into_inner)
                        .stop();
                } else if distance <= CLOSE_ENOUGH_TO_START_INTERACTING_WITH_TARGET_DISTANCE {
                    self.state = TransportItemState::Interacting;
                    self.ticks_since_reaching_target = 0;
                    mob.get_mob_entity()
                        .navigator
                        .lock()
                        .unwrap_or_else(std::sync::PoisonError::into_inner)
                        .stop();
                } else {
                    let pos = golem.mob_entity.living_entity.entity.pos.load();
                    let stuck = mob
                        .get_mob_entity()
                        .navigator
                        .lock()
                        .unwrap_or_else(std::sync::PoisonError::into_inner)
                        .is_stuck();
                    if stuck {
                        self.mark_unreachable(&golem, target);
                        return;
                    }
                    let mut navigator = mob
                        .get_mob_entity()
                        .navigator
                        .lock()
                        .unwrap_or_else(std::sync::PoisonError::into_inner);
                    if navigator.is_idle() {
                        navigator.set_progress(NavigatorGoal::new(
                            pos,
                            Self::target_center(&target),
                            SPEED_MULTIPLIER_WHEN_IDLING,
                        ));
                    }
                }
            }
            TransportItemState::Queuing => {
                let world = golem.get_entity().world.load();
                if !Self::is_chest_busy(&world, &target) {
                    self.state = TransportItemState::Travelling;
                }
            }
            TransportItemState::Interacting => self.tick_interaction(&golem),
        }
    }

    fn stop(&mut self, mob: &dyn Mob) {
        if let Some(golem) = self.golem.upgrade() {
            self.close_opened_chest(&golem);
            golem.set_state(CopperGolemState::Idle);
        }
        {
            let mut navigator = mob
                .get_mob_entity()
                .navigator
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            navigator.stop();
            navigator.set_can_path_to_targets_below_surface(false);
        }
        self.target = None;
        self.state = TransportItemState::Travelling;
        self.interaction = None;
        self.ticks_since_reaching_target = 0;
    }

    fn should_run_every_tick(&self) -> bool {
        true
    }

    fn controls(&self) -> Controls {
        Controls::MOVE
    }
}

impl CustomSound for CopperGolemEntity {
    fn hurt_sound(&self) -> Option<Sound> {
        Some(match self.get_weather_state() {
            WeatherState::Unaffected | WeatherState::Exposed => Sound::EntityCopperGolemHurt,
            WeatherState::Weathered => Sound::EntityCopperGolemWeatheredHurt,
            WeatherState::Oxidized => Sound::EntityCopperGolemOxidizedHurt,
        })
    }

    fn death_sound(&self) -> Option<Sound> {
        Some(match self.get_weather_state() {
            WeatherState::Unaffected | WeatherState::Exposed => Sound::EntityCopperGolemDeath,
            WeatherState::Weathered => Sound::EntityCopperGolemWeatheredDeath,
            WeatherState::Oxidized => Sound::EntityCopperGolemOxidizedDeath,
        })
    }
}

impl Mob for CopperGolemEntity {
    fn as_custom_sound(&self) -> Option<&dyn crate::entity::custom_sound::CustomSound> {
        Some(self)
    }

    fn mob_write_nbt(&self, nbt: &mut NbtCompound) {
        nbt.put_long(
            "next_weather_age",
            self.next_weathering_tick.load(Ordering::Relaxed),
        );
        nbt.put_int("weather_state", self.get_weather_state().id());
    }

    fn mob_read_nbt(&self, nbt: &NbtCompound) {
        if let Some(next) = nbt.get_long("next_weather_age") {
            self.next_weathering_tick.store(next, Ordering::Relaxed);
        }
        if let Some(state) = nbt.get_int("weather_state") {
            self.set_weather_state(WeatherState::from_id(state));
        }
    }

    fn get_mob_entity(&self) -> &MobEntity {
        &self.mob_entity
    }

    fn mob_tick(&self, _caller: &dyn EntityBase) {
        let world = self.get_entity().world.load();
        self.update_weathering(&world);
        if self.transport_cooldown.load(Ordering::Relaxed) > 0 {
            self.transport_cooldown.fetch_sub(1, Ordering::Relaxed);
        }
    }

    fn mob_on_lightning_strike(
        &self,
        caller: &dyn EntityBase,
        lightning: &crate::entity::lightning::LightningBoltEntity,
    ) {
        let weather_state = self.get_weather_state();
        if weather_state != WeatherState::Unaffected {
            self.next_weathering_tick.store(-1, Ordering::Relaxed);
            self.set_weather_state(weather_state.previous());
        }
        self.mob_entity
            .living_entity
            .on_lightning_strike(caller, lightning);
    }

    fn mob_init_data_tracker(&self) {
        let entity = self.get_entity();
        entity.set_synced_data(
            pumpkin_data::tracked_data::copper_golem::WEATHER_STATE,
            VarInt(self.get_weather_state().id()),
        );
        entity.set_synced_data(
            pumpkin_data::tracked_data::copper_golem::COPPER_GOLEM_STATE,
            VarInt(self.get_state().id()),
        );
    }

    fn mob_interact(&self, _player: &Arc<Player>, item_stack: &mut ItemStack) -> bool {
        let entity = self.get_entity();
        let world = entity.world.load();

        // Honeycomb waxing
        if item_stack.item.id == Item::HONEYCOMB.id
            && self.next_weathering_tick.load(Ordering::Relaxed) != -2
        {
            self.next_weathering_tick.store(-2, Ordering::Relaxed);
            let pos = entity.pos.load();
            world.play_sound(Sound::ItemHoneycombWaxOn, SoundCategory::Blocks, &pos);
            return true;
        }

        // Axe scraping
        if item_stack.item.has_tag(&tag::Item::MINECRAFT_AXES) {
            let current_next = self.next_weathering_tick.load(Ordering::Relaxed);
            if current_next == -2 {
                self.next_weathering_tick.store(-1, Ordering::Relaxed);
                let pos = entity.pos.load();
                world.play_sound(Sound::ItemAxeScrape, SoundCategory::Blocks, &pos);
                return true;
            }

            let weather_state = self.get_weather_state();
            if weather_state != WeatherState::Unaffected {
                self.set_weather_state(weather_state.previous());
                self.next_weathering_tick.store(-1, Ordering::Relaxed);
                let pos = entity.pos.load();
                world.play_sound(Sound::ItemAxeScrape, SoundCategory::Blocks, &pos);
                return true;
            }
        }

        false
    }
}
