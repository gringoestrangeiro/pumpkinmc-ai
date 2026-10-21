use std::sync::{
    Arc, Mutex, Weak,
    atomic::{AtomicBool, AtomicI32, AtomicU8, Ordering},
};

use pumpkin_data::BlockStateId;
use pumpkin_data::block_properties::{
    DoubleBlockHalf, NetherWartLikeProperties, SunflowerProperties, TorchflowerCropLikeProperties,
    WheatLikeProperties,
};
use pumpkin_data::entity::EntityType;
use pumpkin_data::item_stack::ItemStack;
use pumpkin_data::sound::{Sound, SoundCategory};
use pumpkin_data::tag::{self, Taggable};
use pumpkin_data::world::WorldEvent;
use pumpkin_data::{Block, BlockId};
use pumpkin_nbt::compound::NbtCompound;
use pumpkin_nbt::tag::NbtTag;
use pumpkin_util::math::position::BlockPos;
use pumpkin_util::math::vector2::Vector2;
use pumpkin_util::math::vector3::Vector3;
use pumpkin_world::world::BlockFlags;

use crate::block::entities::beehive::BeehiveBlockEntity;
use crate::entity::ai::control::flying_move_control::FlyingMoveControl;
use crate::entity::ai::goal::water_avoiding_random_flying::WaterAvoidingRandomFlyingGoal;
use crate::entity::ai::goal::{Controls, Goal};
use crate::entity::ai::pathfinder::node::PathType;
use crate::entity::ai::pathfinder::{Navigator, NavigatorGoal};
use crate::entity::{
    Entity, EntityBase,
    ageable::{AgeableData, AgeableMob},
    ai::goal::{
        active_target::{ActiveTargetGoal, TargetCondition},
        breed::BreedGoal,
        follow_parent::FollowParentGoal,
        look_around::RandomLookAroundGoal,
        look_at_entity::LookAtEntityGoal,
        melee_attack::MeleeAttackGoal,
        reset_universal_anger::ResetUniversalAngerGoal,
        revenge::RevengeGoal,
        swim::SwimGoal,
        tempt::TemptGoal,
    },
    mob::{
        Mob, MobEntity,
        neutral::{NeutralData, NeutralMob},
    },
    passive::animal::Animal,
    player::Player,
};

pub const FLAG_ROLL: u8 = 2;
pub const FLAG_HAS_STUNG: u8 = 4;
pub const FLAG_HAS_NECTAR: u8 = 8;

const TICKS_BEFORE_GOING_TO_KNOWN_FLOWER: i32 = 600;
const TICKS_WITHOUT_NECTAR_BEFORE_GOING_HOME: i32 = 3600;
const MAX_CROPS_GROWABLE: i32 = 10;
const TOO_FAR_DISTANCE: f64 = 48.0;
const HIVE_CLOSE_ENOUGH_DISTANCE: f64 = 2.0;
const HIVE_SEARCH_DISTANCE: i32 = 20;
const COOLDOWN_BEFORE_LOCATING_NEW_HIVE: i32 = 200;
const COOLDOWN_BEFORE_LOCATING_NEW_FLOWER: i32 = 200;
const MIN_FIND_FLOWER_RETRY_COOLDOWN: i32 = 20;
const MAX_FIND_FLOWER_RETRY_COOLDOWN: i32 = 60;
const MAX_TRAVELLING_TICKS: i32 = 2400;
const MAX_BLACKLISTED_TARGETS: usize = 3;
const TICKS_BEFORE_HIVE_DROP: i32 = 60;
const GROW_CHANCE: i32 = 30;
const POLLINATION_TICKS: i32 = 600;
const MIN_POLLINATION_TICKS: i32 = 400;
const FLOWER_SEARCH_RADIUS: i32 = 5;

fn is_angry(mob: &dyn Mob) -> bool {
    mob.as_neutral().is_some_and(NeutralMob::is_angry)
}

/// Angry and still holding its stinger.
fn can_sting(mob: &dyn Mob) -> bool {
    is_angry(mob)
        && !mob
            .cast_any()
            .downcast_ref::<BeeEntity>()
            .is_some_and(BeeEntity::has_stung)
}

pub struct BeeEntity {
    pub mob_entity: MobEntity,
    pub ageable_data: AgeableData,
    pub neutral_data: NeutralData,
    pub flags: AtomicU8,
    pub ticks_without_nectar: AtomicI32,
    pub cannot_enter_hive_ticks: AtomicI32,
    pub crops_grown_since_pollination: AtomicI32,
    pub time_since_sting: AtomicI32,
    pub hive_pos: Mutex<Option<BlockPos>>,
    pub saved_flower_pos: Mutex<Option<BlockPos>>,
    pub pollinating: AtomicBool,
    pub remaining_cooldown_before_locating_new_hive: AtomicI32,
    pub remaining_cooldown_before_locating_new_flower: AtomicI32,
    pub underwater_ticks: AtomicI32,
    pub blacklisted_hives: Mutex<Vec<BlockPos>>,
}

impl BeeEntity {
    #[expect(clippy::too_many_lines)]
    pub fn new(entity: Entity) -> Arc<Self> {
        let mob_entity = MobEntity::new(entity);
        let bee = Self {
            mob_entity,
            ageable_data: AgeableData::default(),
            neutral_data: NeutralData::default(),
            flags: AtomicU8::new(0),
            ticks_without_nectar: AtomicI32::new(0),
            cannot_enter_hive_ticks: AtomicI32::new(0),
            crops_grown_since_pollination: AtomicI32::new(0),
            time_since_sting: AtomicI32::new(0),
            hive_pos: Mutex::new(None),
            saved_flower_pos: Mutex::new(None),
            pollinating: AtomicBool::new(false),
            remaining_cooldown_before_locating_new_hive: AtomicI32::new(0),
            remaining_cooldown_before_locating_new_flower: AtomicI32::new(rand::random_range(
                MIN_FIND_FLOWER_RETRY_COOLDOWN..=MAX_FIND_FLOWER_RETRY_COOLDOWN,
            )),
            underwater_ticks: AtomicI32::new(0),
            blacklisted_hives: Mutex::new(Vec::new()),
        };
        let mob_arc = Arc::new(bee);
        let mob_weak: Weak<dyn Mob> = {
            let mob_arc: Arc<dyn Mob> = mob_arc.clone();
            Arc::downgrade(&mob_arc)
        };
        let bee_weak = Arc::downgrade(&mob_arc);

        {
            let mut navigator = Navigator::flying();
            navigator.set_can_open_doors(false);
            navigator.set_can_float(false);
            navigator.set_required_path_length(48.0);
            navigator.set_pathfinding_malus(PathType::DamageFire, -1.0);
            navigator.set_pathfinding_malus(PathType::Water, -1.0);
            navigator.set_pathfinding_malus(PathType::WaterBorder, 16.0);
            navigator.set_pathfinding_malus(PathType::Cocoa, -1.0);
            navigator.set_pathfinding_malus(PathType::Fence, -1.0);
            *mob_arc
                .mob_entity
                .navigator
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner) = navigator;
        }
        *mob_arc
            .mob_entity
            .move_control
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner) =
            Box::new(FlyingMoveControl::new(20, true));

        {
            let mut goal_selector = mob_arc
                .mob_entity
                .goals_selector
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);

            goal_selector.add_goal(
                0,
                Box::new(MeleeAttackGoal::new(1.4, true).gated_by(can_sting)),
            );
            goal_selector.add_goal(1, Box::new(BeeEnterHiveGoal::new(bee_weak.clone())));
            goal_selector.add_goal(2, BreedGoal::new(1.0));
            goal_selector.add_goal(3, Box::new(TemptGoal::new(1.25, &[], false)));
            goal_selector.add_goal(4, Box::new(BeePollinateGoal::new(bee_weak.clone())));
            goal_selector.add_goal(5, Box::new(FollowParentGoal::new(1.25)));
            goal_selector.add_goal(5, Box::new(BeeLocateHiveGoal::new(bee_weak.clone())));
            goal_selector.add_goal(5, Box::new(BeeGoToHiveGoal::new(bee_weak.clone())));
            goal_selector.add_goal(6, Box::new(BeeGoToKnownFlowerGoal::new(bee_weak.clone())));
            goal_selector.add_goal(7, Box::new(BeeGrowCropGoal::new(bee_weak)));
            goal_selector.add_goal(8, Box::new(WaterAvoidingRandomFlyingGoal::new(1.0)));
            goal_selector.add_goal(9, Box::new(SwimGoal::default()));
            goal_selector.add_goal(
                10,
                LookAtEntityGoal::with_default(mob_weak, &EntityType::PLAYER, 6.0),
            );
            goal_selector.add_goal(11, Box::new(RandomLookAroundGoal::default()));
        };

        {
            let mut target_selector = mob_arc
                .mob_entity
                .target_selector
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);

            // Vanilla BeeHurtByOtherGoal.
            target_selector.add_goal(
                1,
                Box::new(
                    RevengeGoal::new(true)
                        .alerting_others()
                        .alerting_in_sight()
                        .continuing_while(is_angry),
                ),
            );
            target_selector.add_goal(
                2,
                ActiveTargetGoal::with_default(&mob_arc.mob_entity, &EntityType::PLAYER, true)
                    .when(TargetCondition::AngryAt)
                    .gated_by(can_sting),
            );
            target_selector.add_goal(3, ResetUniversalAngerGoal::new(true));
        };

        mob_arc
    }

    #[must_use]
    pub fn has_flag(&self, flag: u8) -> bool {
        (self.flags.load(Ordering::Relaxed) & flag) != 0
    }

    pub fn set_flag(&self, flag: u8, val: bool) {
        let current = self.flags.load(Ordering::Relaxed);
        let new_flags = if val { current | flag } else { current & !flag };
        self.flags.store(new_flags, Ordering::Relaxed);
        let entity = self.get_entity();
        entity.set_synced_data(
            pumpkin_data::tracked_data::bee::DATA_FLAGS_ID,
            new_flags as i8,
        );
    }

    #[must_use]
    pub fn has_nectar(&self) -> bool {
        self.has_flag(FLAG_HAS_NECTAR)
    }

    pub fn set_has_nectar(&self, val: bool) {
        if val {
            self.reset_ticks_without_nectar_since_exiting_hive();
        }
        self.set_flag(FLAG_HAS_NECTAR, val);
    }

    #[must_use]
    pub fn has_stung(&self) -> bool {
        self.has_flag(FLAG_HAS_STUNG)
    }

    pub fn set_has_stung(&self, val: bool) {
        self.set_flag(FLAG_HAS_STUNG, val);
    }

    #[must_use]
    pub fn is_rolling(&self) -> bool {
        self.has_flag(FLAG_ROLL)
    }

    pub fn set_rolling(&self, val: bool) {
        self.set_flag(FLAG_ROLL, val);
    }

    #[must_use]
    pub fn get_hive_pos(&self) -> Option<BlockPos> {
        *self
            .hive_pos
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }

    pub fn set_hive_pos(&self, pos: Option<BlockPos>) {
        *self
            .hive_pos
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner) = pos;
    }

    #[must_use]
    pub fn has_hive(&self) -> bool {
        self.get_hive_pos().is_some()
    }

    fn drop_hive(&self) {
        self.set_hive_pos(None);
        self.remaining_cooldown_before_locating_new_hive
            .store(COOLDOWN_BEFORE_LOCATING_NEW_HIVE, Ordering::Relaxed);
    }

    fn is_hive_blacklisted(&self, pos: &BlockPos) -> bool {
        self.blacklisted_hives
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .contains(pos)
    }

    fn blacklist_hive(&self, pos: BlockPos) {
        let mut blacklisted = self
            .blacklisted_hives
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        blacklisted.push(pos);
        while blacklisted.len() > MAX_BLACKLISTED_TARGETS {
            blacklisted.remove(0);
        }
    }

    fn clear_hive_blacklist(&self) {
        self.blacklisted_hives
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .clear();
    }

    #[must_use]
    pub fn get_saved_flower_pos(&self) -> Option<BlockPos> {
        *self
            .saved_flower_pos
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }

    pub fn set_saved_flower_pos(&self, pos: Option<BlockPos>) {
        *self
            .saved_flower_pos
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner) = pos;
    }

    #[must_use]
    pub fn has_saved_flower_pos(&self) -> bool {
        self.get_saved_flower_pos().is_some()
    }

    fn drop_flower(&self) {
        self.set_saved_flower_pos(None);
        self.remaining_cooldown_before_locating_new_flower.store(
            rand::random_range(MIN_FIND_FLOWER_RETRY_COOLDOWN..=MAX_FIND_FLOWER_RETRY_COOLDOWN),
            Ordering::Relaxed,
        );
    }

    #[must_use]
    pub fn is_pollinating(&self) -> bool {
        self.pollinating.load(Ordering::Relaxed)
    }

    fn reset_ticks_without_nectar_since_exiting_hive(&self) {
        self.ticks_without_nectar.store(0, Ordering::Relaxed);
    }

    #[must_use]
    pub fn is_tired_of_looking_for_nectar(&self) -> bool {
        self.ticks_without_nectar.load(Ordering::Relaxed) > TICKS_WITHOUT_NECTAR_BEFORE_GOING_HOME
    }

    fn is_too_far_away(&self, pos: &BlockPos) -> bool {
        self.get_entity()
            .pos
            .load()
            .squared_distance_to_vec(&pos.to_centered_f64())
            > TOO_FAR_DISTANCE * TOO_FAR_DISTANCE
    }

    fn is_hive_valid(&self) -> bool {
        let Some(pos) = self.get_hive_pos() else {
            return false;
        };
        if self.is_too_far_away(&pos) {
            return false;
        }
        self.get_entity()
            .world
            .load()
            .get_block_entity(&pos)
            .is_some_and(|be| be.as_any().is::<BeehiveBlockEntity>())
    }

    fn is_hive_near_fire(&self) -> bool {
        let Some(pos) = self.get_hive_pos() else {
            return false;
        };
        let world = self.get_entity().world.load_full();
        world.get_block_entity(&pos).is_some_and(|be| {
            be.as_any()
                .downcast_ref::<BeehiveBlockEntity>()
                .is_some_and(|hive| hive.is_fire_nearby(&world))
        })
    }

    #[must_use]
    pub fn wants_to_enter_hive(&self) -> bool {
        if self.cannot_enter_hive_ticks.load(Ordering::Relaxed) > 0
            || self.is_pollinating()
            || self.has_stung()
            || self.mob_entity.get_target().is_some()
        {
            return false;
        }
        let entity = &self.mob_entity.living_entity.entity;
        let wants = self.has_nectar()
            || self.is_tired_of_looking_for_nectar()
            || entity
                .world
                .load()
                .bees_stay_in_hive(&entity.block_pos.load());
        wants && !self.is_hive_near_fire()
    }
}

impl AgeableMob for BeeEntity {
    fn get_ageable_data(&self) -> &AgeableData {
        &self.ageable_data
    }
}

impl Animal for BeeEntity {
    fn is_food(&self, item_stack: &ItemStack) -> bool {
        item_stack.item.has_tag(&tag::Item::MINECRAFT_FLOWERS)
            || item_stack.item.has_tag(&tag::Item::MINECRAFT_BEE_FOOD)
    }
}

impl NeutralMob for BeeEntity {
    fn get_neutral_data(&self) -> &NeutralData {
        &self.neutral_data
    }

    /// Bees calm down once the target is gone, unlike the other neutral mobs.
    fn stays_angry_with_target(&self) -> bool {
        false
    }
}

impl Mob for BeeEntity {
    fn as_ageable(&self) -> Option<&dyn AgeableMob> {
        Some(self)
    }

    fn as_animal(&self) -> Option<&dyn Animal> {
        Some(self)
    }

    fn as_neutral(&self) -> Option<&dyn NeutralMob> {
        Some(self)
    }

    fn mob_write_nbt(&self, nbt: &mut NbtCompound) {
        self.write_ageable_nbt(nbt);
        nbt.put_bool("HasNectar", self.has_nectar());
        nbt.put_bool("HasStung", self.has_stung());
        nbt.put_int(
            "TicksSincePollination",
            self.ticks_without_nectar.load(Ordering::Relaxed),
        );
        nbt.put_int(
            "CannotEnterHiveTicks",
            self.cannot_enter_hive_ticks.load(Ordering::Relaxed),
        );
        nbt.put_int(
            "CropsGrownSincePollination",
            self.crops_grown_since_pollination.load(Ordering::Relaxed),
        );
        if let Some(hive) = self.get_hive_pos() {
            nbt.put(
                "hive_pos",
                NbtTag::IntArray(vec![hive.0.x, hive.0.y, hive.0.z]),
            );
        }
        if let Some(flower) = self.get_saved_flower_pos() {
            nbt.put(
                "flower_pos",
                NbtTag::IntArray(vec![flower.0.x, flower.0.y, flower.0.z]),
            );
        }
    }

    fn mob_read_nbt(&self, nbt: &NbtCompound) {
        self.read_ageable_nbt(nbt);
        if let Some(nectar) = nbt.get_bool("HasNectar") {
            self.set_has_nectar(nectar);
        }
        if let Some(stung) = nbt.get_bool("HasStung") {
            self.set_has_stung(stung);
        }
        if let Some(ticks) = nbt.get_int("TicksSincePollination") {
            self.ticks_without_nectar.store(ticks, Ordering::Relaxed);
        }
        if let Some(cannot) = nbt.get_int("CannotEnterHiveTicks") {
            self.cannot_enter_hive_ticks
                .store(cannot, Ordering::Relaxed);
        }
        if let Some(crops) = nbt.get_int("CropsGrownSincePollination") {
            self.crops_grown_since_pollination
                .store(crops, Ordering::Relaxed);
        }
        if let Some(&[x, y, z]) = nbt.get_int_array("hive_pos") {
            self.set_hive_pos(Some(BlockPos::new(x, y, z)));
        }
        if let Some(&[x, y, z]) = nbt.get_int_array("flower_pos") {
            self.set_saved_flower_pos(Some(BlockPos::new(x, y, z)));
        }
    }

    fn get_mob_entity(&self) -> &MobEntity {
        &self.mob_entity
    }

    fn mob_tick(&self, caller: &dyn EntityBase) {
        self.ageable_ai_step();
        let entity = &self.mob_entity.living_entity.entity;

        if entity.touching_water.load(Ordering::Relaxed) {
            self.underwater_ticks.fetch_add(1, Ordering::Relaxed);
        } else {
            self.underwater_ticks.store(0, Ordering::Relaxed);
        }
        if self.underwater_ticks.load(Ordering::Relaxed) > 20 {
            caller.damage(caller, 1.0, pumpkin_data::damage::DamageType::DROWN);
        }

        if self.has_stung() {
            let time = self.time_since_sting.fetch_add(1, Ordering::Relaxed) + 1;
            if time >= 1200 {
                self.mob_entity.living_entity.set_health(0.0);
            }
        }

        if !self.has_nectar() {
            self.ticks_without_nectar.fetch_add(1, Ordering::Relaxed);
        }

        if self.cannot_enter_hive_ticks.load(Ordering::Relaxed) > 0 {
            self.cannot_enter_hive_ticks.fetch_sub(1, Ordering::Relaxed);
        }
        if self
            .remaining_cooldown_before_locating_new_hive
            .load(Ordering::Relaxed)
            > 0
        {
            self.remaining_cooldown_before_locating_new_hive
                .fetch_sub(1, Ordering::Relaxed);
        }
        if self
            .remaining_cooldown_before_locating_new_flower
            .load(Ordering::Relaxed)
            > 0
        {
            self.remaining_cooldown_before_locating_new_flower
                .fetch_sub(1, Ordering::Relaxed);
        }

        let target = self.mob_entity.get_target();
        let rolling = is_angry(self)
            && !self.has_stung()
            && target.is_some_and(|target| {
                target
                    .get_entity()
                    .pos
                    .load()
                    .squared_distance_to_vec(&entity.pos.load())
                    < 4.0
            });
        self.set_rolling(rolling);

        if entity.age.load(Ordering::Relaxed) % 20 == 0 {
            if self.has_hive() && !self.is_hive_valid() {
                self.set_hive_pos(None);
            }
            if let Some(flower) = self.get_saved_flower_pos() {
                let world = entity.world.load();
                if world.is_loaded(&flower) && !attracts_bees(world.get_block_and_state(&flower).1)
                {
                    self.drop_flower();
                }
            }
        }
    }

    fn mob_init_data_tracker(&self) {
        let entity = self.get_entity();
        let is_baby = entity.age.load(Ordering::Relaxed) < 0;
        if is_baby {
            entity.set_synced_data(pumpkin_data::tracked_data::bee::DATA_BABY_ID, true);
        }
        entity.set_synced_data(
            pumpkin_data::tracked_data::bee::DATA_FLAGS_ID,
            self.flags.load(Ordering::Relaxed) as i8,
        );
    }

    fn custom_travel(&self, caller: &dyn EntityBase) -> bool {
        let entity = &self.mob_entity.living_entity.entity;
        if entity.touching_water.load(Ordering::Relaxed)
            || entity.touching_lava.load(Ordering::Relaxed)
        {
            return false;
        }

        // Vanilla `Bee` inherits `LivingEntity.travel`, whose air path scales the
        // move-control input by the flying speed the move control already applied.
        entity.update_velocity_from_input(self.mob_entity.living_entity.movement_input.load(), 1.0);
        self.mob_entity.living_entity.make_move(caller);

        let mut velocity = entity.velocity.load();
        velocity.x *= 0.91;
        velocity.z *= 0.91;
        velocity.y = (velocity.y - self.get_mob_gravity()) * 0.98;
        entity.velocity.store(velocity);
        true
    }

    fn mob_interact(&self, player: &Arc<Player>, item_stack: &mut ItemStack) -> bool {
        self.animal_interact(player, item_stack, Sound::EntityBeePollinate)
    }
}

fn attracts_bees(state: &'static pumpkin_data::BlockState) -> bool {
    let block = Block::from_state_id(state.id);
    if !block.has_tag(&tag::Block::MINECRAFT_BEE_ATTRACTIVE) {
        return false;
    }
    if block.is_waterlogged(state.id) {
        return false;
    }
    if block.id == BlockId::SUNFLOWER {
        return SunflowerProperties::from_state_id(state.id).half == DoubleBlockHalf::Upper;
    }
    true
}

/// The `minecraft:bee_growables` branch of vanilla's `BeeGrowCropGoal`.
fn growable_state(
    world: &Arc<crate::world::World>,
    pos: &BlockPos,
    block: &Block,
    state_id: BlockStateId,
) -> Option<BlockStateId> {
    match block.id {
        BlockId::WHEAT
        | BlockId::CARROTS
        | BlockId::POTATOES
        | BlockId::PUMPKIN_STEM
        | BlockId::MELON_STEM => {
            let props = WheatLikeProperties::from_state_id(state_id);
            (props.age < 7).then(|| {
                let mut grown = props;
                grown.age += 1;
                grown.to_state_id(block)
            })
        }
        BlockId::BEETROOTS | BlockId::SWEET_BERRY_BUSH => {
            let props = NetherWartLikeProperties::from_state_id(state_id);
            (props.age < 3).then(|| {
                let mut grown = props;
                grown.age += 1;
                grown.to_state_id(block)
            })
        }
        BlockId::TORCHFLOWER_CROP => {
            let props = TorchflowerCropLikeProperties::from_state_id(state_id);
            (props.age < 2).then(|| {
                let mut grown = props;
                grown.age += 1;
                grown.to_state_id(block)
            })
        }
        BlockId::CAVE_VINES | BlockId::CAVE_VINES_PLANT => world
            .block_registry
            .bone_meal(block, world, pos, state_id)
            .then(|| world.get_block_state_id(pos)),
        // Vanilla also advances pitcher crops. Their lower and upper halves are
        // separate states, so growing them here needs a shape-aware handler.
        _ => None,
    }
}

struct BeeEnterHiveGoal {
    bee: Weak<BeeEntity>,
}

impl BeeEnterHiveGoal {
    const fn new(bee: Weak<BeeEntity>) -> Self {
        Self { bee }
    }
}

impl Goal for BeeEnterHiveGoal {
    fn can_start(&mut self, _mob: &dyn Mob) -> bool {
        let Some(bee) = self.bee.upgrade() else {
            return false;
        };
        let Some(hive) = bee.get_hive_pos() else {
            return false;
        };
        let pos = bee.get_entity().pos.load();
        if !bee.wants_to_enter_hive()
            || hive.to_centered_f64().squared_distance_to_vec(&pos)
                > HIVE_CLOSE_ENOUGH_DISTANCE * HIVE_CLOSE_ENOUGH_DISTANCE
        {
            return false;
        }
        let world = bee.get_entity().world.load();
        let Some(block_entity) = world.get_block_entity(&hive) else {
            return false;
        };
        let Some(beehive) = block_entity.as_any().downcast_ref::<BeehiveBlockEntity>() else {
            return false;
        };
        if beehive.is_full() {
            bee.set_hive_pos(None);
            return false;
        }
        true
    }

    fn should_continue(&mut self, _mob: &dyn Mob) -> bool {
        false
    }

    fn start(&mut self, _mob: &dyn Mob) {
        let Some(bee) = self.bee.upgrade() else {
            return;
        };
        let Some(hive) = bee.get_hive_pos() else {
            return;
        };
        let world = bee.get_entity().world.load();
        if let Some(block_entity) = world.get_block_entity(&hive)
            && let Some(beehive) = block_entity.as_any().downcast_ref::<BeehiveBlockEntity>()
        {
            beehive.add_occupant(bee.as_ref(), &world);
        }
    }
}

struct BeeLocateHiveGoal {
    bee: Weak<BeeEntity>,
}

impl BeeLocateHiveGoal {
    const fn new(bee: Weak<BeeEntity>) -> Self {
        Self { bee }
    }

    fn find_nearby_hives_with_space(bee: &BeeEntity) -> Vec<BlockPos> {
        let entity = bee.get_entity();
        let world = entity.world.load();
        let pos = entity.block_pos.load();
        let chunk_radius = HIVE_SEARCH_DISTANCE.div_euclid(16) + 1;
        let center_chunk = Vector2::new(pos.0.x >> 4, pos.0.z >> 4);
        let mut hives = Vec::new();

        for dx in -chunk_radius..=chunk_radius {
            for dz in -chunk_radius..=chunk_radius {
                let chunk_pos = Vector2::new(center_chunk.x + dx, center_chunk.y + dz);
                let Some(chunk) = world.block_entities.get(&chunk_pos) else {
                    continue;
                };
                for block_entity in chunk.values() {
                    let Some(beehive) = block_entity.as_any().downcast_ref::<BeehiveBlockEntity>()
                    else {
                        continue;
                    };
                    if beehive.is_full()
                        || pos.squared_distance(&beehive.position)
                            > HIVE_SEARCH_DISTANCE * HIVE_SEARCH_DISTANCE
                    {
                        continue;
                    }
                    hives.push(beehive.position);
                }
            }
        }
        hives.sort_by_key(|hive| pos.squared_distance(hive));
        hives
    }
}

impl Goal for BeeLocateHiveGoal {
    fn can_start(&mut self, _mob: &dyn Mob) -> bool {
        let Some(bee) = self.bee.upgrade() else {
            return false;
        };
        bee.remaining_cooldown_before_locating_new_hive
            .load(Ordering::Relaxed)
            == 0
            && !bee.has_hive()
            && bee.wants_to_enter_hive()
    }

    fn should_continue(&mut self, _mob: &dyn Mob) -> bool {
        false
    }

    fn start(&mut self, _mob: &dyn Mob) {
        let Some(bee) = self.bee.upgrade() else {
            return;
        };
        bee.remaining_cooldown_before_locating_new_hive
            .store(COOLDOWN_BEFORE_LOCATING_NEW_HIVE, Ordering::Relaxed);
        let hives = Self::find_nearby_hives_with_space(&bee);
        if hives.is_empty() {
            return;
        }
        for hive in &hives {
            if !bee.is_hive_blacklisted(hive) {
                bee.set_hive_pos(Some(*hive));
                return;
            }
        }
        bee.clear_hive_blacklist();
        bee.set_hive_pos(Some(hives[0]));
    }
}

struct BeeGoToHiveGoal {
    bee: Weak<BeeEntity>,
    travelling_ticks: i32,
    ticks_stuck: i32,
}

impl BeeGoToHiveGoal {
    const fn new(bee: Weak<BeeEntity>) -> Self {
        Self {
            bee,
            travelling_ticks: 0,
            ticks_stuck: 0,
        }
    }

    fn has_reached_target(bee: &BeeEntity, target: &BlockPos) -> bool {
        bee.get_entity()
            .pos
            .load()
            .squared_distance_to_vec(&target.to_centered_f64())
            <= HIVE_CLOSE_ENOUGH_DISTANCE * HIVE_CLOSE_ENOUGH_DISTANCE
    }

    fn drop_and_blacklist(&mut self, bee: &BeeEntity) {
        if let Some(hive) = bee.get_hive_pos() {
            bee.blacklist_hive(hive);
        }
        bee.drop_hive();
    }
}

impl Goal for BeeGoToHiveGoal {
    fn can_start(&mut self, mob: &dyn Mob) -> bool {
        let Some(bee) = self.bee.upgrade() else {
            return false;
        };
        let Some(hive) = bee.get_hive_pos() else {
            return false;
        };
        if bee.is_too_far_away(&hive)
            || mob.get_home().is_some()
            || !bee.wants_to_enter_hive()
            || Self::has_reached_target(&bee, &hive)
        {
            return false;
        }
        bee.get_entity()
            .world
            .load()
            .get_block(&hive)
            .has_tag(&tag::Block::MINECRAFT_BEEHIVES)
    }

    fn should_continue(&mut self, mob: &dyn Mob) -> bool {
        self.can_start(mob)
    }

    fn start(&mut self, _mob: &dyn Mob) {
        self.travelling_ticks = 0;
        self.ticks_stuck = 0;
    }

    fn tick(&mut self, mob: &dyn Mob) {
        let Some(bee) = self.bee.upgrade() else {
            return;
        };
        let Some(hive) = bee.get_hive_pos() else {
            return;
        };
        self.travelling_ticks += 1;
        if self.travelling_ticks > MAX_TRAVELLING_TICKS {
            self.drop_and_blacklist(&bee);
            return;
        }

        let entity = bee.get_entity();
        let pos = entity.pos.load();
        let mut navigator = mob
            .get_mob_entity()
            .navigator
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if navigator.is_in_progress() {
            return;
        }

        if navigator.is_stuck() {
            self.ticks_stuck += 1;
            if self.ticks_stuck > TICKS_BEFORE_HIVE_DROP {
                bee.drop_hive();
                self.ticks_stuck = 0;
                return;
            }
        } else if bee.is_too_far_away(&hive) {
            bee.drop_hive();
            return;
        }

        navigator.set_progress(NavigatorGoal::new(pos, hive.to_centered_f64(), 1.0));
    }

    fn stop(&mut self, mob: &dyn Mob) {
        self.travelling_ticks = 0;
        self.ticks_stuck = 0;
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

struct BeeGoToKnownFlowerGoal {
    bee: Weak<BeeEntity>,
    travelling_ticks: i32,
}

impl BeeGoToKnownFlowerGoal {
    const fn new(bee: Weak<BeeEntity>) -> Self {
        Self {
            bee,
            travelling_ticks: 0,
        }
    }
}

impl Goal for BeeGoToKnownFlowerGoal {
    fn can_start(&mut self, mob: &dyn Mob) -> bool {
        let Some(bee) = self.bee.upgrade() else {
            return false;
        };
        let Some(flower) = bee.get_saved_flower_pos() else {
            return false;
        };
        mob.get_home().is_none()
            && bee.ticks_without_nectar.load(Ordering::Relaxed) > TICKS_BEFORE_GOING_TO_KNOWN_FLOWER
            && bee
                .get_entity()
                .pos
                .load()
                .squared_distance_to_vec(&flower.to_centered_f64())
                > HIVE_CLOSE_ENOUGH_DISTANCE * HIVE_CLOSE_ENOUGH_DISTANCE
    }

    fn should_continue(&mut self, mob: &dyn Mob) -> bool {
        self.can_start(mob)
    }

    fn start(&mut self, _mob: &dyn Mob) {
        self.travelling_ticks = 0;
    }

    fn tick(&mut self, mob: &dyn Mob) {
        let Some(bee) = self.bee.upgrade() else {
            return;
        };
        let Some(flower) = bee.get_saved_flower_pos() else {
            return;
        };
        self.travelling_ticks += 1;
        if self.travelling_ticks > MAX_TRAVELLING_TICKS {
            bee.drop_flower();
            return;
        }

        let entity = bee.get_entity();
        let pos = entity.pos.load();
        let mut navigator = mob
            .get_mob_entity()
            .navigator
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if navigator.is_in_progress() {
            return;
        }
        if bee.is_too_far_away(&flower) {
            bee.drop_flower();
            return;
        }
        navigator.set_progress(NavigatorGoal::new(pos, flower.to_centered_f64(), 1.0));
    }

    fn stop(&mut self, mob: &dyn Mob) {
        self.travelling_ticks = 0;
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

struct BeePollinateGoal {
    bee: Weak<BeeEntity>,
    successful_pollinating_ticks: i32,
    last_sound_played_tick: i32,
    pollinating_ticks: i32,
}

impl BeePollinateGoal {
    const fn new(bee: Weak<BeeEntity>) -> Self {
        Self {
            bee,
            successful_pollinating_ticks: 0,
            last_sound_played_tick: 0,
            pollinating_ticks: 0,
        }
    }

    fn find_nearby_flower(bee: &BeeEntity) -> Option<BlockPos> {
        let entity = bee.get_entity();
        let world = entity.world.load();
        let center = entity.block_pos.load();
        for dx in -FLOWER_SEARCH_RADIUS..=FLOWER_SEARCH_RADIUS {
            for dy in -FLOWER_SEARCH_RADIUS..=FLOWER_SEARCH_RADIUS {
                for dz in -FLOWER_SEARCH_RADIUS..=FLOWER_SEARCH_RADIUS {
                    if dx.abs() + dy.abs() + dz.abs() > FLOWER_SEARCH_RADIUS {
                        continue;
                    }
                    let pos = BlockPos::new(center.0.x + dx, center.0.y + dy, center.0.z + dz);
                    let (_, state) = world.get_block_and_state(&pos);
                    if !attracts_bees(state) {
                        continue;
                    }
                    let mut navigator = bee
                        .mob_entity
                        .navigator
                        .lock()
                        .unwrap_or_else(std::sync::PoisonError::into_inner);
                    if navigator
                        .create_path(&bee.mob_entity.living_entity, pos.to_centered_f64(), 1)
                        .is_some_and(|path| path.can_reach())
                    {
                        return Some(pos);
                    }
                }
            }
        }
        None
    }
}

impl Goal for BeePollinateGoal {
    fn can_start(&mut self, _mob: &dyn Mob) -> bool {
        let Some(bee) = self.bee.upgrade() else {
            return false;
        };
        if bee
            .remaining_cooldown_before_locating_new_flower
            .load(Ordering::Relaxed)
            > 0
            || bee.has_nectar()
            || bee.get_entity().world.load().is_raining()
        {
            return false;
        }
        let Some(flower) = Self::find_nearby_flower(&bee) else {
            bee.remaining_cooldown_before_locating_new_flower.store(
                rand::random_range(MIN_FIND_FLOWER_RETRY_COOLDOWN..=MAX_FIND_FLOWER_RETRY_COOLDOWN),
                Ordering::Relaxed,
            );
            return false;
        };
        bee.set_saved_flower_pos(Some(flower));
        true
    }

    fn should_continue(&mut self, _mob: &dyn Mob) -> bool {
        let Some(bee) = self.bee.upgrade() else {
            return false;
        };
        if bee.get_saved_flower_pos().is_none()
            || !bee.is_pollinating()
            || bee.get_entity().world.load().is_raining()
        {
            return false;
        }
        if self.successful_pollinating_ticks > MIN_POLLINATION_TICKS {
            rand::random::<f32>() < 0.2
        } else {
            true
        }
    }

    fn start(&mut self, _mob: &dyn Mob) {
        let Some(bee) = self.bee.upgrade() else {
            return;
        };
        self.successful_pollinating_ticks = 0;
        self.pollinating_ticks = 0;
        self.last_sound_played_tick = 0;
        bee.pollinating.store(true, Ordering::Relaxed);
        bee.reset_ticks_without_nectar_since_exiting_hive();
    }

    fn tick(&mut self, mob: &dyn Mob) {
        let Some(bee) = self.bee.upgrade() else {
            return;
        };
        let Some(flower) = bee.get_saved_flower_pos() else {
            return;
        };
        self.pollinating_ticks += 1;
        if self.pollinating_ticks > POLLINATION_TICKS {
            bee.drop_flower();
            bee.pollinating.store(false, Ordering::Relaxed);
            bee.remaining_cooldown_before_locating_new_flower
                .store(COOLDOWN_BEFORE_LOCATING_NEW_FLOWER, Ordering::Relaxed);
            return;
        }

        let entity = bee.get_entity();
        let flower_pos = flower.to_centered_f64() + Vector3::new(0.0, 0.6, 0.0);
        let bee_pos = entity.pos.load();
        let distance = bee_pos.squared_distance_to_vec(&flower_pos).sqrt();
        if distance > 1.0 {
            let mut navigator = mob
                .get_mob_entity()
                .navigator
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            if navigator.is_idle() {
                navigator.set_progress(NavigatorGoal::new(bee_pos, flower_pos, 0.35));
            }
            return;
        }

        mob.get_mob_entity()
            .navigator
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .stop();
        mob.get_mob_entity()
            .look_control
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .look_at_position(mob, flower.to_centered_f64());
        self.successful_pollinating_ticks += 1;
        if rand::random::<f32>() < 0.05
            && self.successful_pollinating_ticks > self.last_sound_played_tick + 60
        {
            self.last_sound_played_tick = self.successful_pollinating_ticks;
            entity.world.load().play_sound(
                Sound::EntityBeePollinate,
                SoundCategory::Neutral,
                &bee_pos,
            );
        }
    }

    fn stop(&mut self, mob: &dyn Mob) {
        let Some(bee) = self.bee.upgrade() else {
            return;
        };
        if self.successful_pollinating_ticks > MIN_POLLINATION_TICKS {
            bee.set_has_nectar(true);
        }
        bee.pollinating.store(false, Ordering::Relaxed);
        mob.get_mob_entity()
            .navigator
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .stop();
        bee.remaining_cooldown_before_locating_new_flower
            .store(COOLDOWN_BEFORE_LOCATING_NEW_FLOWER, Ordering::Relaxed);
    }

    fn controls(&self) -> Controls {
        Controls::MOVE
    }

    fn should_run_every_tick(&self) -> bool {
        true
    }
}

struct BeeGrowCropGoal {
    bee: Weak<BeeEntity>,
}

impl BeeGrowCropGoal {
    const fn new(bee: Weak<BeeEntity>) -> Self {
        Self { bee }
    }
}

impl Goal for BeeGrowCropGoal {
    fn can_start(&mut self, _mob: &dyn Mob) -> bool {
        let Some(bee) = self.bee.upgrade() else {
            return false;
        };
        bee.crops_grown_since_pollination.load(Ordering::Relaxed) < MAX_CROPS_GROWABLE
            && rand::random::<f32>() >= 0.3
            && bee.has_nectar()
            && bee.is_hive_valid()
    }

    fn should_continue(&mut self, mob: &dyn Mob) -> bool {
        self.can_start(mob)
    }

    fn tick(&mut self, _mob: &dyn Mob) {
        if rand::random_range(0..GROW_CHANCE) != 0 {
            return;
        }
        let Some(bee) = self.bee.upgrade() else {
            return;
        };
        let entity = bee.get_entity();
        let world = entity.world.load_full();
        let bee_pos = entity.block_pos.load();

        for height in 1..=2 {
            let pos = BlockPos::new(bee_pos.0.x, bee_pos.0.y - height, bee_pos.0.z);
            let (block, state_id) = world.get_block_and_state_id(&pos);
            if !block.has_tag(&tag::Block::MINECRAFT_BEE_GROWABLES) {
                continue;
            }
            let Some(grown) = growable_state(&world, &pos, block, state_id) else {
                continue;
            };
            world.sync_world_event(WorldEvent::ParticlesBeeGrowth, pos, 15);
            world.set_block_state(&pos, grown, BlockFlags::NOTIFY_ALL);
            bee.crops_grown_since_pollination
                .fetch_add(1, Ordering::Relaxed);
        }
    }
}

/// Drops the nectar a bee carries after it enters a hive.
pub fn drop_off_nectar(bee: &dyn EntityBase) {
    if let Some(bee) = bee.cast_any().downcast_ref::<BeeEntity>() {
        bee.set_has_nectar(false);
        bee.crops_grown_since_pollination
            .store(0, Ordering::Relaxed);
    }
}
