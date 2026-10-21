use std::sync::{
    Arc, Mutex, Weak,
    atomic::{AtomicBool, AtomicI32, Ordering},
};

use crossbeam::atomic::AtomicCell;
use pumpkin_data::data_component_impl::PotionContentsImpl;
use pumpkin_data::entity::EntityType;
use pumpkin_data::item_stack::ItemStack;
use pumpkin_data::particle::Particle;
use pumpkin_data::sound::{Sound, SoundCategory};
use pumpkin_data::tag::{self, Taggable};
use pumpkin_nbt::compound::NbtCompound;
use pumpkin_nbt::tag::NbtTag;
use pumpkin_util::GameMode;
use pumpkin_util::math::position::BlockPos;
use pumpkin_util::math::vector3::Vector3;
use uuid::Uuid;

use crate::entity::{
    Entity, EntityBase,
    ai::{
        brain::memory::{GlobalPos, MemoryValue, codec},
        control::flying_move_control::FlyingMoveControl,
        goal::{Controls, Goal, water_avoiding_random_flying::WaterAvoidingRandomFlyingGoal},
        pathfinder::{Navigator, NavigatorGoal},
    },
    item::ItemEntity,
    mob::{Mob, MobEntity},
    player::Player,
};

use pumpkin_inventory::{Inventory, SimpleInventory};

/// Vanilla `Allay.ITEM_PICKUP_REACH`.
const ITEM_PICKUP_REACH: f64 = 1.0;
/// Vanilla `AllayAi.MAX_DISTANCE_TO_WANTED_ITEM`.
const MAX_DISTANCE_TO_WANTED_ITEM: f64 = 32.0;
const ITEM_PICKUP_COOLDOWN_DURATION: i32 = 60;
const MAX_NOTEBLOCK_DISTANCE: f64 = 1024.0;
const TIME_TO_FORGET_NOTEBLOCK: i32 = 600;
const LIKED_PLAYER_FOLLOW_DISTANCE: f64 = 64.0;
const SPEED_MULTIPLIER_WHEN_RETRIEVING_ITEM: f64 = 1.75;
const SPEED_MULTIPLIER_WHEN_FOLLOWING_DEPOSIT_TARGET: f64 = 2.25;
const CLOSE_ENOUGH_TO_TARGET: f64 = 4.0;
const TOO_FAR_FROM_TARGET: f64 = 16.0;
const CLOSE_ENOUGH_TO_THROW: f64 = 3.0;
const THROW_HAND_Y_DISTANCE_FROM_EYE: f64 = 0.2;
const THROW_VELOCITY: Vector3<f64> = Vector3::new(0.2, 0.3, 0.2);
/// Vanilla `Allay.THROW_SOUND_PITCHES`.
const THROW_SOUND_PITCHES: [f32; 16] = [
    0.5625, 0.625, 0.75, 0.9375, 1.0, 1.0, 1.125, 1.25, 1.5, 1.875, 2.0, 2.25, 2.5, 3.0, 3.75, 4.0,
];

pub struct AllayEntity {
    pub mob_entity: MobEntity,
    pub dancing: AtomicBool,
    pub can_duplicate: AtomicBool,
    pub duplication_cooldown: AtomicI32,
    pub owner: AtomicCell<Option<Uuid>>,
    /// Vanilla keeps this in the brain's `LIKED_NOTEBLOCK_POSITION` memory. Allays do
    /// not change dimension, so a bare position carries the same information.
    pub liked_noteblock: Mutex<Option<BlockPos>>,
    pub liked_noteblock_cooldown: AtomicI32,
    pub item_pickup_cooldown: AtomicI32,
    pub inventory: SimpleInventory,
}

impl AllayEntity {
    pub fn new(entity: Entity) -> Arc<Self> {
        let mob_entity = MobEntity::new(entity);
        let allay = Self {
            mob_entity,
            dancing: AtomicBool::new(false),
            can_duplicate: AtomicBool::new(true),
            duplication_cooldown: AtomicI32::new(0),
            owner: AtomicCell::new(None),
            liked_noteblock: Mutex::new(None),
            liked_noteblock_cooldown: AtomicI32::new(0),
            item_pickup_cooldown: AtomicI32::new(0),
            inventory: SimpleInventory::new(1),
        };
        let mob_arc = Arc::new(allay);
        let mob_weak: Weak<dyn Mob> = {
            let mob_arc: Arc<dyn Mob> = mob_arc.clone();
            Arc::downgrade(&mob_arc)
        };
        let allay_weak = Arc::downgrade(&mob_arc);

        {
            let mut navigator = Navigator::flying();
            navigator.set_can_open_doors(false);
            navigator.set_can_float(true);
            navigator.set_required_path_length(48.0);
            *mob_arc
                .mob_entity
                .navigator
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner) = navigator;
        };
        *mob_arc
            .mob_entity
            .move_control
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner) =
            Box::new(FlyingMoveControl::new(20, true));
        mob_arc.mob_entity.set_can_pick_up_loot(true);

        {
            let mut goal_selector = mob_arc
                .mob_entity
                .goals_selector
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);

            goal_selector.add_goal(
                0,
                Box::new(crate::entity::ai::goal::swim::SwimGoal::default()),
            );
            goal_selector.add_goal(
                1,
                crate::entity::ai::goal::escape_danger::EscapeDangerGoal::new(1.25),
            );
            goal_selector.add_goal(
                3,
                Box::new(AllayGoToWantedItemGoal::new(allay_weak.clone())),
            );
            goal_selector.add_goal(
                4,
                Box::new(AllayGoAndGiveItemsToTargetGoal::new(allay_weak)),
            );
            goal_selector.add_goal(8, Box::new(WaterAvoidingRandomFlyingGoal::new(1.0)));
            goal_selector.add_goal(
                9,
                crate::entity::ai::goal::look_at_entity::LookAtEntityGoal::with_default(
                    mob_weak,
                    &EntityType::PLAYER,
                    6.0,
                ),
            );
            goal_selector.add_goal(
                10,
                Box::new(crate::entity::ai::goal::look_around::RandomLookAroundGoal::default()),
            );
        };

        mob_arc
    }

    #[must_use]
    pub fn is_dancing(&self) -> bool {
        self.dancing.load(Ordering::Relaxed)
    }

    pub fn set_dancing(&self, dancing: bool) {
        self.dancing.store(dancing, Ordering::Relaxed);
        let entity = self.get_entity();
        entity.set_synced_data(pumpkin_data::tracked_data::allay::DATA_DANCING, dancing);
    }

    #[must_use]
    pub fn can_duplicate(&self) -> bool {
        self.can_duplicate.load(Ordering::Relaxed)
    }

    pub fn set_can_duplicate(&self, can_duplicate: bool) {
        self.can_duplicate.store(can_duplicate, Ordering::Relaxed);
        let entity = self.get_entity();
        entity.set_synced_data(
            pumpkin_data::tracked_data::allay::DATA_CAN_DUPLICATE,
            can_duplicate,
        );
    }

    fn main_hand_item(&self) -> ItemStack {
        self.mob_entity
            .living_entity
            .entity_equipment
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .get(&pumpkin_data::data_component_impl::EquipmentSlot::MAIN_HAND)
    }

    fn set_main_hand_item(&self, stack: ItemStack) {
        self.mob_entity.set_item_slot(
            &pumpkin_data::data_component_impl::EquipmentSlot::MAIN_HAND,
            stack,
        );
    }

    #[must_use]
    pub fn has_item_in_hand(&self) -> bool {
        !self.main_hand_item().is_empty()
    }

    fn is_on_pickup_cooldown(&self) -> bool {
        self.item_pickup_cooldown.load(Ordering::Relaxed) > 0
    }

    /// Vanilla `Allay.wantsToPickUp`; mob griefing is checked by the caller.
    fn wants_to_pick_up(&self, stack: &ItemStack) -> bool {
        let hand = self.main_hand_item();
        !hand.is_empty()
            && self.can_add_item(stack)
            && Self::allay_considers_item_equal(&hand, stack)
    }

    fn allay_considers_item_equal(first: &ItemStack, second: &ItemStack) -> bool {
        first.item.id == second.item.id
            && first.get_data_component::<PotionContentsImpl>()
                == second.get_data_component::<PotionContentsImpl>()
    }

    fn can_add_item(&self, stack: &ItemStack) -> bool {
        let slot = self.inventory.get_stack(0);
        slot.is_empty()
            || (slot.are_items_and_components_equal(stack)
                && slot.item_count < slot.get_max_stack_size())
    }

    fn add_item(&self, stack: &mut ItemStack) {
        let mut slot = self.inventory.get_stack(0);
        let max_stack_size = if slot.is_empty() {
            stack.get_max_stack_size()
        } else {
            slot.get_max_stack_size()
        };
        let amount = stack
            .item_count
            .min(max_stack_size.saturating_sub(slot.item_count));
        let taken = stack.split(amount);
        if slot.is_empty() {
            slot = taken;
        } else {
            slot.item_count = slot.item_count.saturating_add(taken.item_count);
        }
        self.inventory.set_stack(0, slot);
    }

    fn pick_up_item(&self, item: &ItemEntity) -> bool {
        let stack = item
            .get_item_stack()
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .clone();
        if stack.is_empty() || !self.wants_to_pick_up(&stack) {
            return false;
        }

        let slot = self.inventory.get_stack(0);
        let max_stack_size = if slot.is_empty() {
            stack.get_max_stack_size()
        } else {
            slot.get_max_stack_size()
        };
        let taken = stack
            .item_count
            .min(max_stack_size.saturating_sub(slot.item_count));
        if taken == 0
            || !self
                .mob_entity
                .living_entity
                .pickup(item.get_entity(), u32::from(taken))
        {
            return false;
        }

        let mut picked = stack.copy_with_count(taken);
        self.add_item(&mut picked);

        let mut item_stack = item
            .get_item_stack()
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        item_stack.item_count = item_stack.item_count.saturating_sub(taken);
        let emptied = item_stack.is_empty();
        drop(item_stack);

        if emptied {
            item.get_entity().remove();
        } else {
            item.init_data_tracker();
        }
        true
    }

    /// The nearest item entity this allay would pick up, matching vanilla's
    /// `NEAREST_VISIBLE_WANTED_ITEM` sensor.
    fn nearest_wanted_item(&self) -> Option<Arc<dyn EntityBase>> {
        if self.is_on_pickup_cooldown() || !self.has_item_in_hand() {
            return None;
        }
        let entity = &self.mob_entity.living_entity.entity;
        let world = entity.world.load();
        if !world.level_info.load().game_rules.mob_griefing {
            return None;
        }
        let pos = entity.pos.load();
        let search_box = entity.bounding_box.load().expand(
            MAX_DISTANCE_TO_WANTED_ITEM,
            MAX_DISTANCE_TO_WANTED_ITEM,
            MAX_DISTANCE_TO_WANTED_ITEM,
        );
        let max_distance_sq = MAX_DISTANCE_TO_WANTED_ITEM * MAX_DISTANCE_TO_WANTED_ITEM;

        let mut candidates: Vec<(f64, Arc<dyn EntityBase>)> = world
            .entities
            .load()
            .iter()
            .filter_map(|candidate| {
                let item = candidate.get_item_entity()?;
                let item_entity = item.get_entity();
                let distance_sq = item_entity.pos.load().squared_distance_to_vec(&pos);
                let stack = item
                    .get_item_stack()
                    .lock()
                    .unwrap_or_else(std::sync::PoisonError::into_inner);
                (item_entity.is_alive()
                    && item_entity.bounding_box.load().intersects(&search_box)
                    && distance_sq < max_distance_sq
                    && item.get_pickup_delay() == 0
                    && self.wants_to_pick_up(&stack))
                .then(|| (distance_sq, candidate.clone()))
            })
            .collect();
        candidates.sort_by(|a, b| a.0.total_cmp(&b.0));
        candidates
            .into_iter()
            .map(|(_, candidate)| candidate)
            .next()
    }

    /// Vanilla `AllayAi.getItemDepositPosition`: the block above a liked note block,
    /// otherwise the liked player's position.
    fn deposit_position(&self) -> Option<Vector3<f64>> {
        let entity = &self.mob_entity.living_entity.entity;
        let world = entity.world.load();
        let pos = entity.pos.load();

        let liked_noteblock = *self
            .liked_noteblock
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if let Some(noteblock) = liked_noteblock {
            let close_enough = noteblock.to_centered_f64().squared_distance_to_vec(&pos)
                <= MAX_NOTEBLOCK_DISTANCE * MAX_NOTEBLOCK_DISTANCE;
            let still_liked = close_enough
                && self.liked_noteblock_cooldown.load(Ordering::Relaxed) > 0
                && world.get_block(&noteblock).id == pumpkin_data::BlockId::NOTE_BLOCK;
            if still_liked {
                return Some(noteblock.up().to_centered_f64());
            }
            *self
                .liked_noteblock
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner) = None;
        }

        let owner = self.owner.load()?;
        let player = world.get_player_by_uuid(owner)?;
        let player_entity = player.get_entity();
        let mode = player.gamemode.load();
        (matches!(mode, GameMode::Survival | GameMode::Creative)
            && player_entity.pos.load().squared_distance_to_vec(&pos)
                <= LIKED_PLAYER_FOLLOW_DISTANCE * LIKED_PLAYER_FOLLOW_DISTANCE)
            .then(|| player_entity.pos.load())
    }

    fn throw_item_at(&self, target: Vector3<f64>) {
        let stack = self.inventory.remove_stack_specific(0, 1);
        if stack.is_empty() {
            return;
        }

        let entity = &self.mob_entity.living_entity.entity;
        let world = entity.world.load_full();
        let pos = entity.pos.load();
        let hand_pos = Vector3::new(
            pos.x,
            entity.get_eye_y() - THROW_HAND_Y_DISTANCE_FROM_EYE,
            pos.z,
        );
        let direction =
            Vector3::new(target.x - pos.x, target.y + 1.0 - pos.y, target.z - pos.z).normalize();
        let velocity = Vector3::new(
            direction.x * THROW_VELOCITY.x,
            direction.y * THROW_VELOCITY.y,
            direction.z * THROW_VELOCITY.z,
        );
        let item = ItemEntity::new_with_velocity(
            Entity::new(world.clone(), hand_pos, &EntityType::ITEM),
            stack,
            velocity,
            ItemEntity::DEFAULT_PICKUP_DELAY,
        );
        world.spawn_entity(Arc::new(item));

        self.item_pickup_cooldown
            .store(ITEM_PICKUP_COOLDOWN_DURATION, Ordering::Relaxed);
        if world.get_world_age() % 7 == 0 && rand::random::<f64>() < 0.9 {
            let pitch = THROW_SOUND_PITCHES[rand::random_range(0..THROW_SOUND_PITCHES.len())];
            world.play_sound_raw(
                Sound::EntityAllayItemThrown as u16,
                SoundCategory::Neutral,
                &hand_pos,
                1.0,
                pitch,
            );
        }
    }

    /// Vanilla `AllayAi.hearNoteblock`, called from the game event dispatcher.
    pub fn hear_noteblock(&self, pos: BlockPos) {
        let mut liked = self
            .liked_noteblock
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if liked.is_none() {
            *liked = Some(pos);
            self.liked_noteblock_cooldown
                .store(TIME_TO_FORGET_NOTEBLOCK, Ordering::Relaxed);
        } else if *liked == Some(pos) {
            self.liked_noteblock_cooldown
                .store(TIME_TO_FORGET_NOTEBLOCK, Ordering::Relaxed);
        }
    }
}

struct AllayGoToWantedItemGoal {
    allay: Weak<AllayEntity>,
    target: Option<Arc<dyn EntityBase>>,
}

impl AllayGoToWantedItemGoal {
    const fn new(allay: Weak<AllayEntity>) -> Self {
        Self {
            allay,
            target: None,
        }
    }

    fn refresh_target(&mut self, allay: &AllayEntity) -> bool {
        self.target = allay.nearest_wanted_item();
        self.target.is_some()
    }
}

impl Goal for AllayGoToWantedItemGoal {
    fn can_start(&mut self, _mob: &dyn Mob) -> bool {
        let Some(allay) = self.allay.upgrade() else {
            return false;
        };
        self.refresh_target(&allay)
    }

    fn should_continue(&mut self, _mob: &dyn Mob) -> bool {
        let Some(allay) = self.allay.upgrade() else {
            return false;
        };
        let Some(target) = &self.target else {
            return false;
        };
        let entity = &allay.mob_entity.living_entity.entity;
        let pos = entity.pos.load();
        let still_wanted = target.get_item_entity().is_some_and(|item| {
            let stack = item
                .get_item_stack()
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            allay.wants_to_pick_up(&stack)
        });
        target.get_entity().is_alive()
            && target.get_entity().pos.load().squared_distance_to_vec(&pos)
                <= MAX_DISTANCE_TO_WANTED_ITEM * MAX_DISTANCE_TO_WANTED_ITEM
            && !allay.is_on_pickup_cooldown()
            && allay.has_item_in_hand()
            && still_wanted
    }

    fn start(&mut self, mob: &dyn Mob) {
        let Some(allay) = self.allay.upgrade() else {
            return;
        };
        let entity = &allay.mob_entity.living_entity.entity;
        let pos = entity.pos.load();
        if let Some(target) = &self.target {
            let target_pos = target.get_entity().pos.load();
            mob.get_mob_entity()
                .navigator
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .set_progress(NavigatorGoal::new(
                    pos,
                    target_pos,
                    SPEED_MULTIPLIER_WHEN_RETRIEVING_ITEM,
                ));
        }
    }

    fn tick(&mut self, mob: &dyn Mob) {
        let Some(allay) = self.allay.upgrade() else {
            return;
        };
        let Some(target) = self.target.clone() else {
            return;
        };
        let entity = &allay.mob_entity.living_entity.entity;
        let pos = entity.pos.load();
        let target_pos = target.get_entity().pos.load();
        let target_block = BlockPos::floored_v(target_pos);
        mob.get_mob_entity()
            .look_control
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .look_at_entity(mob, &target);
        let reach = entity.bounding_box.load().expand(
            ITEM_PICKUP_REACH,
            ITEM_PICKUP_REACH,
            ITEM_PICKUP_REACH,
        );

        if reach.intersects(&target.get_entity().bounding_box.load()) {
            if let Some(item) = target.get_item_entity() {
                allay.pick_up_item(item);
            }
            mob.get_mob_entity()
                .navigator
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .stop();
            return;
        }

        let mut navigator = mob
            .get_mob_entity()
            .navigator
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if navigator.is_idle() || navigator.get_target_pos() != Some(target_block) {
            navigator.set_progress(NavigatorGoal::new(
                pos,
                target_pos,
                SPEED_MULTIPLIER_WHEN_RETRIEVING_ITEM,
            ));
        }
    }

    fn stop(&mut self, mob: &dyn Mob) {
        mob.get_mob_entity()
            .navigator
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .stop();
        self.target = None;
    }

    fn should_run_every_tick(&self) -> bool {
        true
    }

    fn controls(&self) -> Controls {
        Controls::MOVE | Controls::LOOK
    }
}

struct AllayGoAndGiveItemsToTargetGoal {
    allay: Weak<AllayEntity>,
    last_target: Option<Vector3<f64>>,
}

impl AllayGoAndGiveItemsToTargetGoal {
    const fn new(allay: Weak<AllayEntity>) -> Self {
        Self {
            allay,
            last_target: None,
        }
    }
}

impl Goal for AllayGoAndGiveItemsToTargetGoal {
    fn can_start(&mut self, _mob: &dyn Mob) -> bool {
        let Some(allay) = self.allay.upgrade() else {
            return false;
        };
        let Some(target) = allay.deposit_position() else {
            return false;
        };
        let entity = &allay.mob_entity.living_entity.entity;
        let distance_sq = target.squared_distance_to_vec(&entity.pos.load());
        !allay.inventory.is_empty() || distance_sq >= TOO_FAR_FROM_TARGET * TOO_FAR_FROM_TARGET
    }

    fn should_continue(&mut self, mob: &dyn Mob) -> bool {
        self.can_start(mob)
    }

    fn start(&mut self, mob: &dyn Mob) {
        let Some(allay) = self.allay.upgrade() else {
            return;
        };
        let Some(target) = allay.deposit_position() else {
            return;
        };
        let pos = allay.mob_entity.living_entity.entity.pos.load();
        mob.get_mob_entity()
            .navigator
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .set_progress(NavigatorGoal::new(
                pos,
                target,
                SPEED_MULTIPLIER_WHEN_FOLLOWING_DEPOSIT_TARGET,
            ));
        self.last_target = Some(target);
    }

    fn tick(&mut self, mob: &dyn Mob) {
        let Some(allay) = self.allay.upgrade() else {
            return;
        };
        let Some(target) = allay.deposit_position() else {
            return;
        };
        let entity = &allay.mob_entity.living_entity.entity;
        let pos = entity.pos.load();
        let eye_pos = entity.get_eye_pos();
        let distance_to_eye = target.squared_distance_to_vec(&eye_pos).sqrt();

        if !allay.inventory.is_empty() && distance_to_eye < CLOSE_ENOUGH_TO_THROW {
            allay.throw_item_at(target);
            return;
        }

        let mut navigator = mob
            .get_mob_entity()
            .navigator
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let target_moved = self
            .last_target
            .is_none_or(|last| last.squared_distance_to_vec(&target) > 4.0);
        let stop_distance = if allay.inventory.is_empty() {
            CLOSE_ENOUGH_TO_TARGET
        } else {
            CLOSE_ENOUGH_TO_THROW
        };
        if target.squared_distance_to_vec(&pos) <= stop_distance * stop_distance {
            navigator.stop();
        } else if navigator.is_idle() || target_moved {
            navigator.set_progress(NavigatorGoal::new(
                pos,
                target,
                SPEED_MULTIPLIER_WHEN_FOLLOWING_DEPOSIT_TARGET,
            ));
        }
        self.last_target = Some(target);
    }

    fn stop(&mut self, mob: &dyn Mob) {
        mob.get_mob_entity()
            .navigator
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .stop();
        self.last_target = None;
    }

    fn should_run_every_tick(&self) -> bool {
        true
    }

    fn controls(&self) -> Controls {
        Controls::MOVE | Controls::LOOK
    }
}

impl Mob for AllayEntity {
    fn mob_write_nbt(&self, nbt: &mut NbtCompound) {
        nbt.put_bool("CanDuplicate", self.can_duplicate());
        nbt.put_int(
            "DuplicationCooldown",
            self.duplication_cooldown.load(Ordering::Relaxed),
        );
        if let Some(owner) = self.owner.load() {
            nbt.put_uuid("Owner", owner);
        }
        if !self.inventory.is_empty() {
            let mut items = Vec::new();
            for slot in 0..self.inventory.size() {
                let stack = self.inventory.get_stack(slot);
                if !stack.is_empty() {
                    let mut item_nbt = NbtCompound::new();
                    stack.write_item_stack(&mut item_nbt);
                    items.push(NbtTag::Compound(item_nbt));
                }
            }
            if !items.is_empty() {
                nbt.put_list("Inventory", items);
            }
        }

        // Vanilla keeps these in the brain as `LIKED_NOTEBLOCK_POSITION` and
        // `LIKED_NOTEBLOCK_COOLDOWN_TICKS` memories.
        let liked_noteblock = *self
            .liked_noteblock
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if let Some(pos) = liked_noteblock {
            let world = self.mob_entity.living_entity.entity.world.load();
            if let Some(dimension) =
                pumpkin_data::dimension::Dimension::from_name(world.dimension.minecraft_name)
            {
                let global_pos = GlobalPos::new(dimension, pos);
                if let Some(tag) = (codec::GLOBAL_POS.encode)(&global_pos as &dyn MemoryValue) {
                    let mut brain = nbt.get_compound("Brain").cloned().unwrap_or_default();
                    let mut memories = brain.get_compound("memories").cloned().unwrap_or_default();

                    let mut memory = NbtCompound::new();
                    memory.put("value", tag);
                    memories.put("minecraft:liked_noteblock", NbtTag::Compound(memory));

                    let cooldown = self.liked_noteblock_cooldown.load(Ordering::Relaxed);
                    if cooldown > 0 {
                        let mut cooldown_memory = NbtCompound::new();
                        cooldown_memory.put_int("value", cooldown);
                        memories.put(
                            "minecraft:liked_noteblock_cooldown_ticks",
                            NbtTag::Compound(cooldown_memory),
                        );
                    }

                    brain.put("memories", NbtTag::Compound(memories));
                    nbt.put("Brain", NbtTag::Compound(brain));
                }
            }
        }
    }

    fn mob_read_nbt(&self, nbt: &NbtCompound) {
        if let Some(can) = nbt.get_bool("CanDuplicate") {
            self.set_can_duplicate(can);
        }
        if let Some(memories) = nbt
            .get_compound("Brain")
            .and_then(|brain| brain.get_compound("memories"))
        {
            if let Some(tag) = memories
                .get_compound("minecraft:liked_noteblock")
                .and_then(|memory| memory.get("value"))
                && let Some(value) = (codec::GLOBAL_POS.decode)(tag)
                && let Some(global_pos) = value.as_any().downcast_ref::<GlobalPos>()
            {
                *self
                    .liked_noteblock
                    .lock()
                    .unwrap_or_else(std::sync::PoisonError::into_inner) = Some(global_pos.pos);
            }
            if let Some(cooldown) = memories
                .get_compound("minecraft:liked_noteblock_cooldown_ticks")
                .and_then(|memory| memory.get_int("value"))
            {
                self.liked_noteblock_cooldown
                    .store(cooldown, Ordering::Relaxed);
            }
        }
        if let Some(cd) = nbt.get_int("DuplicationCooldown") {
            self.duplication_cooldown.store(cd, Ordering::Relaxed);
        }
        if let Some(owner) = nbt.get_uuid("Owner") {
            self.owner.store(Some(owner));
        }
        if let Some(items) = nbt.get_list("Inventory")
            && let Some(stack) = items.first().and_then(NbtTag::extract_compound)
            && let Some(stack) = ItemStack::read_item_stack(stack)
        {
            self.inventory.set_stack(0, stack);
        }
    }

    fn get_mob_entity(&self) -> &MobEntity {
        &self.mob_entity
    }

    fn mob_tick(&self, _caller: &dyn EntityBase) {
        let cd = self.duplication_cooldown.load(Ordering::Relaxed);
        if cd > 0 {
            let next = cd - 1;
            self.duplication_cooldown.store(next, Ordering::Relaxed);
            if next == 0 {
                self.set_can_duplicate(true);
            }
        }

        let noteblock_cooldown = self.liked_noteblock_cooldown.load(Ordering::Relaxed);
        if noteblock_cooldown > 0 {
            self.liked_noteblock_cooldown
                .store(noteblock_cooldown - 1, Ordering::Relaxed);
        }
        let pickup_cooldown = self.item_pickup_cooldown.load(Ordering::Relaxed);
        if pickup_cooldown > 0 {
            self.item_pickup_cooldown
                .store(pickup_cooldown - 1, Ordering::Relaxed);
        }
    }

    fn custom_travel(&self, caller: &dyn EntityBase) -> bool {
        let entity = &self.mob_entity.living_entity.entity;
        let input = self.mob_entity.living_entity.movement_input.load();
        let drag = if entity.touching_water.load(Ordering::Relaxed) {
            entity.update_velocity_from_input(input, 0.02);
            0.8
        } else if entity.touching_lava.load(Ordering::Relaxed) {
            entity.update_velocity_from_input(input, 0.02);
            0.5
        } else {
            // Vanilla `travelFlying(input, speed)`: the move control already scaled
            // `movement_input` by the flying speed.
            entity.update_velocity_from_input(input, 1.0);
            0.91
        };
        self.mob_entity.living_entity.make_move(caller);

        entity.velocity.store(entity.velocity.load() * drag);
        true
    }

    fn mob_init_data_tracker(&self) {
        let entity = self.get_entity();
        entity.set_synced_data(
            pumpkin_data::tracked_data::allay::DATA_DANCING,
            self.is_dancing(),
        );
        entity.set_synced_data(
            pumpkin_data::tracked_data::allay::DATA_CAN_DUPLICATE,
            self.can_duplicate(),
        );
    }

    fn mob_interact(&self, player: &Arc<Player>, item_stack: &mut ItemStack) -> bool {
        let item = item_stack.get_item();
        let hand = self.main_hand_item();

        if self.is_dancing()
            && self.can_duplicate()
            && item.has_tag(&tag::Item::MINECRAFT_DUPLICATES_ALLAYS)
        {
            item_stack.decrement_unless_creative(player.gamemode.load(), 1);
            self.set_can_duplicate(false);
            self.duplication_cooldown.store(6000, Ordering::Relaxed);

            let entity = self.get_entity();
            let world = entity.world.load_full();
            let pos = entity.pos.load();
            world.spawn_particle(
                pos + Vector3::new(0.0, f64::from(entity.height()), 0.0),
                Vector3::new(0.5, 0.5, 0.5),
                1.0,
                7,
                Particle::Heart,
            );
            world.play_sound(Sound::BlockAmethystBlockChime, SoundCategory::Neutral, &pos);

            let new_allay = Self::new(Entity::new(world.clone(), pos, &EntityType::ALLAY));
            world.spawn_entity(new_allay);
            return true;
        }

        if hand.is_empty() && !item_stack.is_empty() {
            let item_to_give = item_stack.copy_with_count(1);
            self.set_main_hand_item(item_to_give);
            item_stack.decrement_unless_creative(player.gamemode.load(), 1);
            let entity = self.get_entity();
            entity.world.load().play_sound(
                Sound::EntityAllayItemGiven,
                SoundCategory::Neutral,
                &entity.pos.load(),
            );
            self.owner.store(Some(player.gameprofile.id));
            return true;
        }

        if !hand.is_empty() && item_stack.is_empty() {
            self.set_main_hand_item(ItemStack::EMPTY.clone());
            let entity = self.get_entity();
            entity.world.load().play_sound(
                Sound::EntityAllayItemTaken,
                SoundCategory::Neutral,
                &entity.pos.load(),
            );
            self.mob_entity.living_entity.swing_hand();

            for slot in 0..self.inventory.size() {
                let stack = self.inventory.remove_stack(slot);
                if !stack.is_empty() {
                    entity
                        .world
                        .load()
                        .drop_stack(&entity.block_pos.load(), stack);
                }
            }

            self.owner.store(None);
            let mut hand = hand;
            player.inventory.insert_stack_anywhere(&mut hand);
            return true;
        }

        false
    }
}
