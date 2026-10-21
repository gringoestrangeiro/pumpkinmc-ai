use std::sync::{
    Arc,
    atomic::{AtomicI32, AtomicU8, Ordering},
};

use crossbeam::atomic::AtomicCell;
use pumpkin_data::data_component_impl::{ConsumableImpl, EquipmentSlot, FoodImpl};
use pumpkin_data::entity::{EntityPose, EntityStatus, EntityType};
use pumpkin_data::game_event::GameEvent;
use pumpkin_data::item::Item;
use pumpkin_data::item_stack::ItemStack;
use pumpkin_data::sound::{Sound, SoundCategory};
use pumpkin_data::tag::{self, Taggable};
use pumpkin_data::world::WorldEvent;
use pumpkin_data::{
    Block,
    block_properties::{
        CaveVinesLikeProperties, CaveVinesPlantLikeProperties, NetherWartLikeProperties,
    },
};
use pumpkin_nbt::compound::NbtCompound;
use pumpkin_protocol::codec::var_int::VarInt;
use pumpkin_util::math::position::BlockPos;
use pumpkin_util::math::vector3::Vector3;
use pumpkin_world::world::BlockFlags;
use rand::RngExt;

use crate::entity::{
    Entity, EntityBase,
    ageable::{AgeableData, AgeableMob},
    ai::goal::{
        Controls, Goal, ParentHandle,
        active_target::ActiveTargetGoal,
        breed::BreedGoal,
        climb_on_top_of_powder_snow::ClimbOnTopOfPowderSnowGoal,
        escape_danger::EscapeDangerGoal,
        follow_parent::FollowParentGoal,
        leap_at_target::LeapAtTargetGoal,
        look_around::RandomLookAroundGoal,
        look_at_entity::LookAtEntityGoal,
        melee_attack::MeleeAttackGoal,
        move_to_target_pos::{MoveToTargetPos, MoveToTargetPosGoal},
        swim::SwimGoal,
        tempt::TemptGoal,
        to_goal_ticks,
        wander_around::WanderAroundGoal,
    },
    ai::pathfinder::node::PathType,
    mob::{Mob, MobEntity, equipment::RegionalDifficulty, spawn::SpawnGroupData},
    passive::animal::Animal,
    player::Player,
};
use crate::world::World;

pub const FOX_FOOD: &[&Item] = &[&Item::SWEET_BERRIES, &Item::GLOW_BERRIES];

const MIN_TICKS_BEFORE_EAT: i32 = 600;
const MAX_CROUCH_AMOUNT: f32 = 5.0;
const WAIT_TIME_BEFORE_SLEEP: i32 = 140;
const EAT_BERRIES_RANGE: i32 = 12;
const EAT_BERRIES_SPEED: f64 = 1.2;
const EAT_BERRIES_WAIT_TICKS: i32 = 40;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[repr(i32)]
pub enum FoxVariant {
    #[default]
    Red = 0,
    Snow = 1,
}

impl FoxVariant {
    #[must_use]
    pub const fn from_id(id: i32) -> Self {
        match id {
            1 => Self::Snow,
            _ => Self::Red,
        }
    }

    #[must_use]
    pub const fn id(self) -> i32 {
        self as i32
    }

    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Red => "red",
            Self::Snow => "snow",
        }
    }

    #[must_use]
    pub fn from_name(s: &str) -> Self {
        match s {
            "snow" | "minecraft:snow" => Self::Snow,
            _ => Self::Red,
        }
    }
}

/// Represents a Fox, a passive/neutral mob.
///
/// Wiki: <https://minecraft.wiki/w/Fox>
pub struct FoxEntity {
    pub mob_entity: MobEntity,
    pub ageable_data: AgeableData,
    pub variant: AtomicI32,
    pub flags: AtomicU8,
    crouch_amount: AtomicCell<f32>,
    ticks_since_eaten: AtomicI32,
}

impl FoxEntity {
    pub fn new(entity: Entity) -> Arc<Self> {
        let mob_entity = MobEntity::new(entity);
        let fox = Self {
            mob_entity,
            ageable_data: AgeableData::default(),
            variant: AtomicI32::new(FoxVariant::Red.id()),
            flags: AtomicU8::new(0),
            crouch_amount: AtomicCell::new(0.0),
            ticks_since_eaten: AtomicI32::new(0),
        };
        let mob_arc = Arc::new(fox);

        {
            let mut navigator = mob_arc
                .mob_entity
                .navigator
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            navigator.set_pathfinding_malus(PathType::DamageOther, 0.0);
            navigator.set_pathfinding_malus(PathType::DangerOther, 0.0);
        }

        {
            let mut goal_selector = mob_arc
                .mob_entity
                .goals_selector
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);

            goal_selector.add_goal(0, Box::new(SwimGoal::default()));
            goal_selector.add_goal(0, Box::new(ClimbOnTopOfPowderSnowGoal));
            goal_selector.add_goal(1, Box::new(FaceplantGoal::new()));
            goal_selector.add_goal(2, EscapeDangerGoal::new(2.2));
            goal_selector.add_goal(3, BreedGoal::new(1.0));
            goal_selector.add_goal(3, Box::new(TemptGoal::new(1.2, FOX_FOOD, false)));
            goal_selector.add_goal(5, Box::new(StalkPreyGoal::new()));
            goal_selector.add_goal(6, Box::new(FoxPounceGoal::new()));
            goal_selector.add_goal(
                7,
                Box::new(MeleeAttackGoal::new(1.2, true).gated_by(fox_can_melee)),
            );
            goal_selector.add_goal(7, Box::new(SleepGoal::new()));
            goal_selector.add_goal(8, Box::new(FollowParentGoal::new(1.25)));
            goal_selector.add_goal(10, FoxEatBerriesGoal::new());
            goal_selector.add_goal(10, Box::new(LeapAtTargetGoal::new(0.4)));
            goal_selector.add_goal(11, Box::new(WanderAroundGoal::new(1.0)));
            goal_selector.add_goal(
                12,
                LookAtEntityGoal::with_default(weak_mob(&mob_arc), &EntityType::PLAYER, 24.0),
            );
            goal_selector.add_goal(13, Box::new(RandomLookAroundGoal::default()));
        }

        {
            let mut target_selector = mob_arc
                .mob_entity
                .target_selector
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            target_selector.add_goal(
                4,
                ActiveTargetGoal::predicated(&mob_arc.mob_entity, 10, false, chicken_or_rabbit),
            );
            target_selector.add_goal(
                4,
                Box::new(ActiveTargetGoal::new(
                    &mob_arc.mob_entity,
                    &EntityType::TURTLE,
                    10,
                    false,
                    false,
                    Some(baby_turtle_on_land),
                )),
            );
            target_selector.add_goal(
                6,
                ActiveTargetGoal::predicated(&mob_arc.mob_entity, 20, false, schooling_fish),
            );
        }

        mob_arc
    }

    #[must_use]
    pub fn get_variant(&self) -> FoxVariant {
        FoxVariant::from_id(self.variant.load(Ordering::Relaxed))
    }

    pub fn set_variant(&self, variant: FoxVariant) {
        self.variant.store(variant.id(), Ordering::Relaxed);
        let entity = self.get_entity();
        entity.set_synced_data(
            pumpkin_data::tracked_data::fox::TYPE_ID,
            VarInt(variant.id()),
        );
    }

    fn get_flag(&self, flag: u8) -> bool {
        (self.flags.load(Ordering::Relaxed) & flag) != 0
    }

    fn set_flag(&self, flag: u8, value: bool) {
        let current = self.flags.load(Ordering::Relaxed);
        let new_flags = if value {
            current | flag
        } else {
            current & !flag
        };
        self.flags.store(new_flags, Ordering::Relaxed);
        let entity = self.get_entity();
        entity.set_synced_data(pumpkin_data::tracked_data::fox::FLAGS_ID, new_flags);
    }

    #[must_use]
    pub fn is_sitting(&self) -> bool {
        self.get_flag(1)
    }

    pub fn set_sitting(&self, val: bool) {
        self.set_flag(1, val);
    }

    #[must_use]
    pub fn is_crouching(&self) -> bool {
        self.get_flag(4)
    }

    pub fn set_crouching(&self, val: bool) {
        self.set_flag(4, val);
    }

    #[must_use]
    pub fn is_interested(&self) -> bool {
        self.get_flag(8)
    }

    pub fn set_interested(&self, val: bool) {
        self.set_flag(8, val);
    }

    #[must_use]
    pub fn is_pouncing(&self) -> bool {
        self.get_flag(16)
    }

    pub fn set_pouncing(&self, val: bool) {
        self.set_flag(16, val);
    }

    #[must_use]
    pub fn is_sleeping(&self) -> bool {
        self.get_flag(32)
    }

    pub fn set_sleeping(&self, val: bool) {
        self.set_flag(32, val);
    }

    #[must_use]
    pub fn is_faceplanted(&self) -> bool {
        self.get_flag(64)
    }

    pub fn set_faceplanted(&self, val: bool) {
        self.set_flag(64, val);
    }

    #[must_use]
    pub fn is_defending(&self) -> bool {
        self.get_flag(128)
    }

    pub fn set_defending(&self, val: bool) {
        self.set_flag(128, val);
    }

    pub fn clear_states(&self) {
        self.set_interested(false);
        self.set_crouching(false);
        self.set_sitting(false);
        self.set_sleeping(false);
        self.set_defending(false);
        self.set_faceplanted(false);
    }

    fn crouch_amount(&self) -> f32 {
        self.crouch_amount.load()
    }

    fn set_crouch_amount(&self, amount: f32) {
        self.crouch_amount.store(amount);
    }

    fn is_fully_crouched(&self) -> bool {
        self.crouch_amount() >= MAX_CROUCH_AMOUNT
    }

    fn wake_up(&self) {
        self.set_sleeping(false);
    }

    fn can_eat(&self, item: &ItemStack) -> bool {
        item.get_data_component::<FoodImpl>().is_some()
            && item.get_data_component::<ConsumableImpl>().is_some()
    }
}

fn weak_mob(mob: &Arc<FoxEntity>) -> std::sync::Weak<dyn Mob> {
    let mob: Arc<dyn Mob> = mob.clone();
    Arc::downgrade(&mob)
}

fn as_fox(mob: &dyn Mob) -> Option<&FoxEntity> {
    mob.cast_any().downcast_ref::<FoxEntity>()
}

#[expect(
    clippy::float_cmp,
    reason = "Vanilla compares the slope to exactly zero"
)]
fn is_path_clear(fox: &dyn EntityBase, target: &dyn EntityBase) -> bool {
    let fox_pos = fox.get_entity().pos.load();
    let target_pos = target.get_entity().pos.load();
    let zdiff = target_pos.z - fox_pos.z;
    let xdiff = target_pos.x - fox_pos.x;
    let slope = zdiff / xdiff;
    let world = fox.get_entity().world.load();

    for i in 0..6 {
        let fraction = f64::from(i) / 6.0;
        let (z, x) = if slope == 0.0 {
            (0.0, xdiff * fraction)
        } else {
            let z = zdiff * fraction;
            (z, z / slope)
        };

        for j in 1..4 {
            let pos = BlockPos::containing(fox_pos.x + x, fox_pos.y + f64::from(j), fox_pos.z + z);
            if !world.get_block_state(&pos).replaceable() {
                return false;
            }
        }
    }

    true
}

fn is_stalkable_prey(target: &dyn EntityBase) -> bool {
    let entity_type = target.get_entity().entity_type;
    entity_type == &EntityType::CHICKEN || entity_type == &EntityType::RABBIT
}

fn fox_can_melee(mob: &dyn Mob) -> bool {
    as_fox(mob).is_some_and(|fox| {
        !fox.is_sitting() && !fox.is_sleeping() && !fox.is_crouching() && !fox.is_faceplanted()
    })
}

fn alertable(fox: &FoxEntity) -> bool {
    let entity = fox.get_entity();
    let search_box = entity.bounding_box.load().expand(12.0, 6.0, 12.0);
    let world = entity.world.load();

    world
        .get_entities_and_players_at_box(&search_box)
        .iter()
        .any(|candidate| {
            let candidate_entity = candidate.get_entity();
            if candidate_entity.entity_id == entity.entity_id
                || candidate.get_living_entity().is_none()
            {
                return false;
            }
            if candidate_entity.entity_type == &EntityType::FOX {
                return false;
            }
            if is_stalkable_prey(candidate.as_ref()) || candidate_entity.entity_type.is_enemy {
                return true;
            }
            if let Some(tamable) = candidate.get_mob().and_then(|mob| mob.as_tamable()) {
                return !tamable.is_tame();
            }
            if let Some(player) = candidate.get_player() {
                return player.gamemode.load() != pumpkin_util::GameMode::Creative
                    && player.gamemode.load() != pumpkin_util::GameMode::Spectator;
            }
            candidate_entity.pose.load() != EntityPose::Sleeping
        })
}

fn has_shelter(mob: &dyn Mob) -> bool {
    let entity = mob.get_entity();
    let pos = BlockPos::containing(
        entity.pos.load().x,
        entity.bounding_box.load().max.y,
        entity.pos.load().z,
    );
    !entity.world.load().can_see_sky(&pos) && mob.get_walk_target_value(&pos) >= 0.0
}

/// Vanilla `Fox.FaceplantGoal`.
struct FaceplantGoal {
    countdown: i32,
}

impl FaceplantGoal {
    const fn new() -> Self {
        Self { countdown: 0 }
    }
}

impl Goal for FaceplantGoal {
    fn can_start(&mut self, mob: &dyn Mob) -> bool {
        as_fox(mob).is_some_and(FoxEntity::is_faceplanted)
    }

    fn should_continue(&mut self, mob: &dyn Mob) -> bool {
        self.can_start(mob) && self.countdown > 0
    }

    fn start(&mut self, _mob: &dyn Mob) {
        self.countdown = to_goal_ticks(40);
    }

    fn stop(&mut self, mob: &dyn Mob) {
        if let Some(fox) = as_fox(mob) {
            fox.set_faceplanted(false);
        }
    }

    fn tick(&mut self, _mob: &dyn Mob) {
        self.countdown -= 1;
    }

    fn controls(&self) -> Controls {
        Controls::LOOK | Controls::JUMP | Controls::MOVE
    }
}

/// Vanilla `Fox.StalkPreyGoal`.
struct StalkPreyGoal {
    target: Option<Arc<dyn EntityBase>>,
}

impl StalkPreyGoal {
    const fn new() -> Self {
        Self { target: None }
    }
}

impl Goal for StalkPreyGoal {
    fn can_start(&mut self, mob: &dyn Mob) -> bool {
        let Some(fox) = as_fox(mob) else {
            return false;
        };
        if fox.is_sleeping() {
            return false;
        }
        let Some(target) = mob.get_mob_entity().get_target() else {
            return false;
        };
        if !target.get_entity().is_alive() || !is_stalkable_prey(target.as_ref()) {
            return false;
        }
        let distance = mob
            .get_entity()
            .pos
            .load()
            .squared_distance_to_vec(&target.get_entity().pos.load());
        if distance <= 36.0
            || fox.is_crouching()
            || fox.is_interested()
            || mob.get_entity().jumping.load(Ordering::Relaxed)
        {
            return false;
        }
        self.target = Some(target);
        true
    }

    fn start(&mut self, mob: &dyn Mob) {
        if let Some(fox) = as_fox(mob) {
            fox.set_sitting(false);
            fox.set_faceplanted(false);
        }
    }

    fn stop(&mut self, mob: &dyn Mob) {
        let Some(fox) = as_fox(mob) else {
            return;
        };
        if let Some(target) = &self.target
            && is_path_clear(mob.get_entity(), target.as_ref())
        {
            fox.set_interested(true);
            fox.set_crouching(true);
            mob.get_mob_entity()
                .navigator
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .stop();
            mob.get_mob_entity()
                .look_control
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .look_at_entity(mob, target);
        } else {
            fox.set_interested(false);
            fox.set_crouching(false);
        }
    }

    fn tick(&mut self, mob: &dyn Mob) {
        let Some(target) = &self.target else {
            return;
        };
        let position = target.get_entity().pos.load();
        mob.get_mob_entity()
            .look_control
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .look_at_entity(mob, target);
        if mob
            .get_entity()
            .pos
            .load()
            .squared_distance_to_vec(&position)
            <= 36.0
        {
            if let Some(fox) = as_fox(mob) {
                fox.set_interested(true);
                fox.set_crouching(true);
            }
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
                .set_progress(crate::entity::ai::pathfinder::NavigatorGoal::new(
                    mob.get_entity().pos.load(),
                    position,
                    1.5,
                ));
        }
    }

    fn should_run_every_tick(&self) -> bool {
        true
    }

    fn controls(&self) -> Controls {
        Controls::MOVE | Controls::LOOK
    }
}

/// Vanilla `Fox.FoxPounceGoal`.
struct FoxPounceGoal {
    target: Option<Arc<dyn EntityBase>>,
}

impl FoxPounceGoal {
    const fn new() -> Self {
        Self { target: None }
    }
}

impl Goal for FoxPounceGoal {
    fn can_start(&mut self, mob: &dyn Mob) -> bool {
        let Some(fox) = as_fox(mob) else {
            return false;
        };
        if !fox.is_fully_crouched() {
            return false;
        }
        let Some(target) = mob.get_mob_entity().get_target() else {
            return false;
        };
        if !target.get_entity().is_alive() {
            return false;
        }
        let clear = is_path_clear(mob.get_entity(), target.as_ref());
        if !clear {
            mob.get_mob_entity()
                .navigator
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .set_progress(crate::entity::ai::pathfinder::NavigatorGoal::new(
                    mob.get_entity().pos.load(),
                    target.get_entity().pos.load(),
                    1.0,
                ));
            fox.set_crouching(false);
            fox.set_interested(false);
        } else {
            self.target = Some(target);
        }
        clear
    }

    fn should_continue(&mut self, mob: &dyn Mob) -> bool {
        let Some(fox) = as_fox(mob) else {
            return false;
        };
        let Some(target) = &self.target else {
            return false;
        };
        if !target.get_entity().is_alive() {
            return false;
        }
        let velocity = mob.get_entity().velocity.load();
        (velocity.y * velocity.y >= 0.05
            || mob.get_entity().pitch.load().abs() >= 15.0
            || !mob.get_entity().on_ground.load(Ordering::Relaxed))
            && !fox.is_faceplanted()
    }

    fn start(&mut self, mob: &dyn Mob) {
        let Some(fox) = as_fox(mob) else {
            return;
        };
        fox.set_sitting(false);
        mob.get_entity().jumping.store(true, Ordering::SeqCst);
        fox.set_pouncing(true);
        fox.set_interested(false);

        if let Some(target) = &self.target {
            mob.get_mob_entity()
                .look_control
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .look_at_entity(mob, target);
            let fox_pos = mob.get_entity().pos.load();
            let target_pos = target.get_entity().pos.load();
            let direction = Vector3::new(
                target_pos.x - fox_pos.x,
                target_pos.y - fox_pos.y,
                target_pos.z - fox_pos.z,
            )
            .normalize();
            let velocity = mob.get_entity().velocity.load();
            mob.get_entity()
                .set_velocity(velocity + Vector3::new(direction.x * 0.8, 0.9, direction.z * 0.8));
        }

        mob.get_mob_entity()
            .navigator
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .stop();
    }

    fn stop(&mut self, mob: &dyn Mob) {
        if let Some(fox) = as_fox(mob) {
            fox.set_crouching(false);
            fox.set_crouch_amount(0.0);
            fox.set_interested(false);
            fox.set_pouncing(false);
        }
        self.target = None;
    }

    fn tick(&mut self, mob: &dyn Mob) {
        let Some(fox) = as_fox(mob) else {
            return;
        };
        let entity = mob.get_entity();
        let Some(target) = self.target.clone() else {
            return;
        };

        mob.get_mob_entity()
            .look_control
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .look_at_entity(mob, &target);

        if !fox.is_faceplanted() {
            let movement = entity.velocity.load();
            if movement.y * movement.y < 0.03 && entity.pitch.load().abs() > f32::EPSILON {
                entity.set_pitch(entity.pitch.load() * 0.8);
            } else {
                let direction = (movement.x * movement.x + movement.z * movement.z).sqrt();
                let upwards_bias = if entity.jumping.load(Ordering::Relaxed) && movement.y > 0.0 {
                    6.5
                } else {
                    1.0
                };
                let biased_y = movement.y * upwards_bias;
                let length = direction.mul_add(direction, biased_y * biased_y).sqrt();
                if length > 1.0e-5 {
                    let sign = if biased_y > 0.0 {
                        1.0
                    } else if biased_y < 0.0 {
                        -1.0
                    } else {
                        0.0
                    };
                    let rotation =
                        -sign * (direction / length).acos() * 180.0 / std::f64::consts::PI;
                    entity.set_pitch(rotation as f32);
                }
            }
        }

        let distance = entity
            .pos
            .load()
            .squared_distance_to_vec(&target.get_entity().pos.load());
        if distance <= 4.0 {
            mob.get_mob_entity().try_attack(entity, target.as_ref());
        // Pumpkin zeroes downward velocity on landing, so vanilla's
        // `deltaMovement.y != 0` guard would never pass here.
        } else if entity.pitch.load() > 0.0
            && entity.on_ground.load(Ordering::Relaxed)
            && entity.world.load().get_block(&entity.block_pos.load()).id == Block::SNOW.id
        {
            entity.set_pitch(60.0);
            mob.set_mob_target(None);
            fox.set_faceplanted(true);
        }
    }

    fn should_run_every_tick(&self) -> bool {
        true
    }

    fn can_stop(&self) -> bool {
        false
    }

    fn controls(&self) -> Controls {
        Controls::MOVE | Controls::LOOK | Controls::JUMP
    }
}

/// Vanilla `Fox.SleepGoal`.
struct SleepGoal {
    countdown: i32,
}

impl SleepGoal {
    fn new() -> Self {
        Self {
            countdown: rand::rng().random_range(0..to_goal_ticks(WAIT_TIME_BEFORE_SLEEP)),
        }
    }

    fn can_sleep(&mut self, mob: &dyn Mob) -> bool {
        if self.countdown > 0 {
            self.countdown -= 1;
            return false;
        }
        mob.get_entity().world.load().is_bright_outside()
            && has_shelter(mob)
            && as_fox(mob).is_some_and(|fox| !alertable(fox))
            && !mob.get_entity().is_in_powder_snow()
    }
}

impl Goal for SleepGoal {
    fn can_start(&mut self, mob: &dyn Mob) -> bool {
        let movement = mob.get_mob_entity().living_entity.movement_input.load();
        if movement.length_squared() > 1.0e-7 {
            return false;
        }
        self.can_sleep(mob) || as_fox(mob).is_some_and(FoxEntity::is_sleeping)
    }

    fn should_continue(&mut self, mob: &dyn Mob) -> bool {
        self.can_sleep(mob)
    }

    fn start(&mut self, mob: &dyn Mob) {
        let Some(fox) = as_fox(mob) else {
            return;
        };
        fox.set_sitting(false);
        fox.set_crouching(false);
        fox.set_interested(false);
        mob.get_entity().jumping.store(false, Ordering::SeqCst);
        fox.set_sleeping(true);
        mob.get_mob_entity()
            .navigator
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .stop();
        let pos = mob.get_entity().pos.load();
        mob.get_mob_entity()
            .move_control
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .set_wanted_position(pos.x, pos.y, pos.z, 0.0);
    }

    fn stop(&mut self, mob: &dyn Mob) {
        self.countdown = rand::rng().random_range(0..to_goal_ticks(WAIT_TIME_BEFORE_SLEEP));
        if let Some(fox) = as_fox(mob) {
            fox.clear_states();
        }
    }

    fn controls(&self) -> Controls {
        Controls::MOVE | Controls::LOOK | Controls::JUMP
    }
}

/// Vanilla `Fox.FoxEatBerriesGoal`.
struct FoxEatBerriesGoal {
    inner: MoveToTargetPosGoal<Self>,
    ticks_waited: i32,
}

impl FoxEatBerriesGoal {
    fn new() -> Box<Self> {
        let mut this = Box::new(Self {
            inner: MoveToTargetPosGoal::new(
                ParentHandle::none(),
                EAT_BERRIES_SPEED,
                EAT_BERRIES_RANGE,
                1,
            ),
            ticks_waited: 0,
        });
        // SAFETY: `this` is heap allocated in a `Box` and outlives the handle.
        this.inner.move_to_target_pos = unsafe { ParentHandle::new(&*this) };
        this
    }

    fn on_reached_target(&self, mob: &dyn Mob) {
        let entity = mob.get_entity();
        let world = entity.world.load_full();
        if !world.level_info.load().game_rules.mob_griefing {
            return;
        }
        let bush_pos = self.inner.target_pos;
        let (block, state_id) = world.get_block_and_state_id(&bush_pos);

        if block.id == Block::SWEET_BERRY_BUSH.id {
            let mut props = NetherWartLikeProperties::from_state_id(state_id);
            let age = props.age;
            let mut count = 1 + rand::rng().random_range(0..2) + u8::from(age == 3);

            let mouth = mob
                .get_mob_entity()
                .living_entity
                .entity_equipment
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .get(&EquipmentSlot::MAIN_HAND);
            if mouth.is_empty() {
                mob.get_mob_entity().set_item_slot(
                    &EquipmentSlot::MAIN_HAND,
                    ItemStack::new(1, &Item::SWEET_BERRIES),
                );
                count -= 1;
            }
            if count > 0 {
                world.drop_stack(&bush_pos, ItemStack::new(count, &Item::SWEET_BERRIES));
            }

            world.play_sound(
                Sound::BlockSweetBerryBushPickBerries,
                SoundCategory::Blocks,
                &bush_pos.to_centered_f64(),
            );
            props.age = 1;
            world.set_block_state(
                &bush_pos,
                props.to_state_id(&Block::SWEET_BERRY_BUSH),
                BlockFlags::NOTIFY_ALL,
            );
            world.emit_game_event(GameEvent::BlockChange, bush_pos.to_centered_f64());
        } else if block.id == Block::CAVE_VINES.id {
            let mut props = CaveVinesLikeProperties::from_state_id(state_id);
            if props.berries {
                props.berries = false;
                world.drop_stack(&bush_pos, ItemStack::new(1, &Item::GLOW_BERRIES));
                world.play_sound(
                    Sound::BlockCaveVinesPickBerries,
                    SoundCategory::Blocks,
                    &bush_pos.to_centered_f64(),
                );
                world.set_block_state(
                    &bush_pos,
                    props.to_state_id(&Block::CAVE_VINES),
                    BlockFlags::NOTIFY_ALL,
                );
                world.emit_game_event(GameEvent::BlockChange, bush_pos.to_centered_f64());
            }
        } else if block.id == Block::CAVE_VINES_PLANT.id {
            let mut props = CaveVinesPlantLikeProperties::from_state_id(state_id);
            if props.berries {
                props.berries = false;
                world.drop_stack(&bush_pos, ItemStack::new(1, &Item::GLOW_BERRIES));
                world.play_sound(
                    Sound::BlockCaveVinesPickBerries,
                    SoundCategory::Blocks,
                    &bush_pos.to_centered_f64(),
                );
                world.set_block_state(
                    &bush_pos,
                    props.to_state_id(&Block::CAVE_VINES_PLANT),
                    BlockFlags::NOTIFY_ALL,
                );
                world.emit_game_event(GameEvent::BlockChange, bush_pos.to_centered_f64());
            }
        }
    }
}

fn has_berries(world: &World, pos: &BlockPos) -> bool {
    let (block, state_id) = world.get_block_and_state_id(pos);
    if block.id == Block::SWEET_BERRY_BUSH.id {
        return NetherWartLikeProperties::from_state_id(state_id).age >= 2;
    }
    if block.id == Block::CAVE_VINES.id {
        return CaveVinesLikeProperties::from_state_id(state_id).berries;
    }
    if block.id == Block::CAVE_VINES_PLANT.id {
        return CaveVinesPlantLikeProperties::from_state_id(state_id).berries;
    }
    false
}

impl Goal for FoxEatBerriesGoal {
    fn can_start(&mut self, mob: &dyn Mob) -> bool {
        if !as_fox(mob).is_some_and(|fox| !fox.is_sleeping()) {
            return false;
        }
        self.inner.can_start(mob)
    }

    fn should_continue(&mut self, mob: &dyn Mob) -> bool {
        as_fox(mob).is_some_and(|fox| !fox.is_sleeping()) && self.inner.should_continue(mob)
    }

    fn start(&mut self, mob: &dyn Mob) {
        self.ticks_waited = 0;
        if let Some(fox) = as_fox(mob) {
            fox.set_sitting(false);
        }
        self.inner.start(mob);
    }

    fn stop(&mut self, mob: &dyn Mob) {
        self.inner.stop(mob);
    }

    fn tick(&mut self, mob: &dyn Mob) {
        if self.inner.is_reached_target() {
            if self.ticks_waited >= EAT_BERRIES_WAIT_TICKS {
                self.on_reached_target(mob);
            } else {
                self.ticks_waited += 1;
            }
        } else if rand::rng().random::<f32>() < 0.05 {
            mob.get_entity().world.load().play_sound(
                Sound::EntityFoxSniff,
                SoundCategory::Neutral,
                &mob.get_entity().pos.load(),
            );
        }
        self.inner.tick(mob);
    }

    fn should_run_every_tick(&self) -> bool {
        true
    }

    fn controls(&self) -> Controls {
        self.inner.controls()
    }
}

impl MoveToTargetPos for FoxEatBerriesGoal {
    fn is_target_pos(&self, world: Arc<World>, block_pos: BlockPos) -> bool {
        has_berries(&world, &block_pos)
    }

    fn get_desired_distance_to_target(&self) -> f64 {
        2.0
    }
}

fn chicken_or_rabbit(living: &crate::entity::living::LivingEntity, _world: &World) -> bool {
    living.entity.entity_type == &EntityType::CHICKEN
        || living.entity.entity_type == &EntityType::RABBIT
}

fn baby_turtle_on_land(living: &crate::entity::living::LivingEntity, _world: &World) -> bool {
    living.entity.entity_type == &EntityType::TURTLE
        && living.entity.age.load(Ordering::Relaxed) < 0
        && !living.entity.is_in_water()
}

fn schooling_fish(living: &crate::entity::living::LivingEntity, _world: &World) -> bool {
    living.entity.entity_type == &EntityType::COD
        || living.entity.entity_type == &EntityType::SALMON
        || living.entity.entity_type == &EntityType::TROPICAL_FISH
}

impl AgeableMob for FoxEntity {
    fn get_ageable_data(&self) -> &AgeableData {
        &self.ageable_data
    }
}

impl Animal for FoxEntity {
    fn is_food(&self, item_stack: &ItemStack) -> bool {
        item_stack.item.has_tag(&tag::Item::MINECRAFT_FOX_FOOD)
            || FOX_FOOD.iter().any(|i| i.id == item_stack.item.id)
    }
}

impl Mob for FoxEntity {
    fn as_ageable(&self) -> Option<&dyn AgeableMob> {
        Some(self)
    }

    fn as_animal(&self) -> Option<&dyn Animal> {
        Some(self)
    }

    fn mob_write_nbt(&self, nbt: &mut NbtCompound) {
        nbt.put_string("Type", self.get_variant().as_str().to_string());
        nbt.put_bool("Sleeping", self.is_sleeping());
        nbt.put_bool("Sitting", self.is_sitting());
        nbt.put_bool("Crouching", self.is_crouching());
    }

    fn mob_read_nbt(&self, nbt: &NbtCompound) {
        if let Some(variant_str) = nbt.get_string("Type") {
            self.set_variant(FoxVariant::from_name(variant_str));
        }
        if let Some(sleeping) = nbt.get_bool("Sleeping") {
            self.set_sleeping(sleeping);
        }
        if let Some(sitting) = nbt.get_bool("Sitting") {
            self.set_sitting(sitting);
        }
        if let Some(crouching) = nbt.get_bool("Crouching") {
            self.set_crouching(crouching);
        }
    }

    fn get_mob_entity(&self) -> &MobEntity {
        &self.mob_entity
    }

    fn mob_set_variant_name(&self, name: &str) {
        self.set_variant(FoxVariant::from_name(name));
    }

    fn mob_tick(&self, _caller: &dyn EntityBase) {
        self.ageable_ai_step();
        self.ticks_since_eaten.fetch_add(1, Ordering::Relaxed);

        let entity = self.get_entity();
        if entity.is_alive() {
            let mouth = self
                .mob_entity
                .living_entity
                .entity_equipment
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .get(&EquipmentSlot::MAIN_HAND);
            let target_is_missing = self
                .mob_entity
                .get_target()
                .is_none_or(|target| !target.get_entity().is_alive());

            if self.can_eat(&mouth)
                && target_is_missing
                && entity.on_ground.load(Ordering::Relaxed)
                && !self.is_sleeping()
            {
                let ticks = self.ticks_since_eaten.load(Ordering::Relaxed);
                if ticks > MIN_TICKS_BEFORE_EAT {
                    let mut remaining = mouth;
                    remaining.decrement(1);
                    if remaining.item_count == 0 {
                        remaining.clear();
                    }
                    self.mob_entity
                        .set_item_slot(&EquipmentSlot::MAIN_HAND, remaining);
                    self.ticks_since_eaten.store(0, Ordering::Relaxed);
                } else if ticks > MIN_TICKS_BEFORE_EAT - 40 && rand::rng().random::<f32>() < 0.1 {
                    self.play_eating_sound(Sound::EntityFoxEat);
                    entity
                        .world
                        .load()
                        .send_entity_status(entity, EntityStatus::FoxEat, None);
                }
            }

            if target_is_missing {
                self.set_crouching(false);
                self.set_interested(false);
            }
        }

        if self.is_sleeping() || !entity.is_alive() {
            entity.jumping.store(false, Ordering::SeqCst);
            self.mob_entity
                .living_entity
                .movement_input
                .store(Vector3::default());
        }
    }

    fn post_tick(&self) {
        let entity = self.get_entity();
        let world = entity.world.load();
        let in_water = entity.is_in_water();

        if in_water || self.mob_entity.get_target().is_some() || world.is_thundering() {
            self.wake_up();
        }
        if in_water || self.is_sleeping() {
            self.set_sitting(false);
        }
        if self.is_faceplanted() && rand::rng().random::<f32>() < 0.2 {
            let block = world.get_block(&entity.block_pos.load());
            world.sync_world_event(
                WorldEvent::ParticlesDestroyBlock,
                entity.block_pos.load(),
                i32::from(block.default_state.id.as_u16()),
            );
        }

        if self.is_crouching() {
            self.set_crouch_amount((self.crouch_amount() + 0.2).min(MAX_CROUCH_AMOUNT));
        } else {
            self.set_crouch_amount(0.0);
        }
    }

    fn populate_default_equipment_slots(
        &self,
        _world: &Arc<World>,
        _difficulty: &RegionalDifficulty,
    ) {
        let mut rng = rand::rng();
        if rng.random::<f32>() < 0.2 {
            let odds = rng.random::<f32>();
            let item = if odds < 0.05 {
                &Item::EMERALD
            } else if odds < 0.2 {
                &Item::EGG
            } else if odds < 0.4 {
                if rng.random::<bool>() {
                    &Item::RABBIT_FOOT
                } else {
                    &Item::RABBIT_HIDE
                }
            } else if odds < 0.6 {
                &Item::WHEAT
            } else if odds < 0.8 {
                &Item::LEATHER
            } else {
                &Item::FEATHER
            };
            self.mob_entity
                .set_item_slot(&EquipmentSlot::MAIN_HAND, ItemStack::new(1, item));
        }
    }

    fn finalize_spawn(
        &self,
        world: &Arc<World>,
        group_data: Option<SpawnGroupData>,
    ) -> Option<SpawnGroupData> {
        self.mob_entity.finalize_spawn_base();
        let pos = self.get_entity().block_pos.load();
        let biome = world.get_biome(&pos);
        self.set_variant(
            if biome.has_tag(&tag::WorldgenBiome::MINECRAFT_SPAWNS_SNOW_FOXES) {
                FoxVariant::Snow
            } else {
                FoxVariant::Red
            },
        );
        let difficulty = RegionalDifficulty::at(world, self.get_entity().pos.load());
        self.populate_default_equipment_slots(world, &difficulty);
        group_data
    }

    fn mob_init_data_tracker(&self) {
        let entity = self.get_entity();
        let is_baby = entity.age.load(Ordering::Relaxed) < 0;
        if is_baby {
            entity.set_synced_data(pumpkin_data::tracked_data::fox::BABY_ID, true);
        }
        entity.set_synced_data(
            pumpkin_data::tracked_data::fox::TYPE_ID,
            VarInt(self.get_variant().id()),
        );
        entity.set_synced_data(
            pumpkin_data::tracked_data::fox::FLAGS_ID,
            self.flags.load(Ordering::Relaxed),
        );
    }

    fn get_ambient_sound(&self) -> Option<Sound> {
        if self.is_sleeping() {
            return Some(Sound::EntityFoxSleep);
        }
        let entity = self.get_entity();
        let world = entity.world.load();
        if !world.is_bright_outside() && rand::rng().random::<f32>() < 0.1 {
            let players = world.get_nearby_players(entity.pos.load(), 16.0);
            if players
                .iter()
                .all(|player| player.gamemode.load() == pumpkin_util::GameMode::Spectator)
            {
                return Some(Sound::EntityFoxScreech);
            }
        }
        Some(Sound::EntityFoxAmbient)
    }

    fn play_ambient_sound(&self) {
        let Some(sound) = self.get_ambient_sound() else {
            return;
        };
        let entity = self.get_entity();
        let volume = if sound == Sound::EntityFoxScreech {
            2.0
        } else {
            1.0
        };
        entity.world.load().play_sound_fine(
            sound,
            SoundCategory::Neutral,
            &entity.pos.load(),
            volume,
            self.mob_entity.living_entity.get_pitch(),
        );
    }

    fn mob_interact(&self, player: &Arc<Player>, item_stack: &mut ItemStack) -> bool {
        self.animal_interact(player, item_stack, Sound::EntityFoxAmbient)
    }
}
