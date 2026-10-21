use std::sync::atomic::{AtomicBool, AtomicI32, Ordering};
use std::sync::{Arc, Mutex};

use crossbeam::atomic::AtomicCell;
use pumpkin_data::damage::DamageType;
use pumpkin_data::entity::EntityType;
use pumpkin_data::sound::{Sound, SoundCategory};
use pumpkin_data::tag::Taggable;
use pumpkin_nbt::compound::NbtCompound;
use pumpkin_util::math::position::BlockPos;
use pumpkin_util::math::vector3::Vector3;
use rand::RngExt;
use uuid::Uuid;

use crate::entity::{
    Entity, EntityBase,
    ai::{
        control::{Control, MoveControlTrait, move_control::Operation},
        goal::{
            Controls, Goal, active_target::ActiveTargetGoal, look_at_entity::LookAtEntityGoal,
            revenge::RevengeGoal, swim::SwimGoal, track_target::TrackTargetGoal,
        },
    },
    mob::{Mob, MobEntity},
};

/// Vanilla `FLAG_IS_CHARGING`.
const FLAG_IS_CHARGING: i8 = 1;
/// Vanilla `reducedTickDelay(7)` for `VexChargeAttackGoal.canUse`.
const CHARGE_CHANCE: i32 = 4;
/// Vanilla `reducedTickDelay(7)` for `VexRandomMoveGoal.canUse`.
const RANDOM_MOVE_CHANCE: i32 = 4;
const RANDOM_MOVE_ATTEMPTS: i32 = 3;

pub struct VexEntity {
    pub mob_entity: MobEntity,
    owner_uuid: Mutex<Option<Uuid>>,
    bound_origin: AtomicCell<Option<BlockPos>>,
    has_limited_life: AtomicBool,
    limited_life_ticks: AtomicI32,
    is_charging: AtomicBool,
}

impl VexEntity {
    pub fn new(entity: Entity) -> Arc<Self> {
        entity.set_has_no_gravity(true);
        let mob_entity = MobEntity::new(entity);
        {
            let mut move_control = mob_entity
                .move_control
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            *move_control = Box::new(VexMoveControl::default());
        };

        let vex = Self {
            mob_entity,
            owner_uuid: Mutex::new(None),
            bound_origin: AtomicCell::new(None),
            has_limited_life: AtomicBool::new(false),
            limited_life_ticks: AtomicI32::new(0),
            is_charging: AtomicBool::new(false),
        };
        let mob_arc = Arc::new(vex);
        let mob_weak: std::sync::Weak<dyn Mob> = {
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
            goal_selector.add_goal(
                4,
                Box::new(VexChargeAttackGoal::new(Arc::downgrade(&mob_arc))),
            );
            goal_selector.add_goal(
                8,
                Box::new(VexRandomMoveGoal::new(Arc::downgrade(&mob_arc))),
            );
            goal_selector.add_goal(
                9,
                Box::new(LookAtEntityGoal::new(
                    mob_weak,
                    &EntityType::PLAYER,
                    3.0,
                    1.0,
                    false,
                )),
            );

            let mut target_selector = mob_arc
                .mob_entity
                .target_selector
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            target_selector.add_goal(
                1,
                Box::new(
                    RevengeGoal::new(true)
                        .ignoring(|entity_type| {
                            entity_type.has_tag(&pumpkin_data::tag::EntityType::MINECRAFT_RAIDERS)
                        })
                        .alerting_others(),
                ),
            );
            target_selector.add_goal(2, VexCopyOwnerTargetGoal::new(Arc::downgrade(&mob_arc)));
            target_selector.add_goal(
                3,
                ActiveTargetGoal::with_default(&mob_arc.mob_entity, &EntityType::PLAYER, true),
            );
        };

        mob_arc
    }

    pub fn set_owner(&self, owner: &Entity) {
        *self
            .owner_uuid
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner) = Some(owner.entity_uuid);
    }

    #[must_use]
    pub fn get_owner(&self) -> Option<Arc<dyn EntityBase>> {
        let owner_uuid = {
            let owner_uuid = self
                .owner_uuid
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            (*owner_uuid)?
        };
        self.mob_entity
            .living_entity
            .entity
            .world
            .load()
            .get_entity_by_uuid(owner_uuid)
    }

    pub fn set_bound_origin(&self, pos: BlockPos) {
        self.bound_origin.store(Some(pos));
    }

    pub fn set_limited_life(&self, life_ticks: i32) {
        self.has_limited_life.store(true, Ordering::Relaxed);
        self.limited_life_ticks.store(life_ticks, Ordering::Relaxed);
    }

    #[must_use]
    pub fn is_charging(&self) -> bool {
        self.is_charging.load(Ordering::Relaxed)
    }

    pub fn set_is_charging(&self, charging: bool) {
        if self.is_charging.swap(charging, Ordering::Relaxed) != charging {
            self.mob_entity.living_entity.entity.set_synced_data(
                pumpkin_data::tracked_data::vex::DATA_FLAGS_ID,
                if charging { FLAG_IS_CHARGING } else { 0 },
            );
        }
    }
}

impl Mob for VexEntity {
    fn get_mob_entity(&self) -> &MobEntity {
        &self.mob_entity
    }

    fn get_owner_uuid(&self) -> Option<Uuid> {
        let owner_uuid = self
            .owner_uuid
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        *owner_uuid
    }

    fn mob_tick(&self, caller: &dyn EntityBase) {
        let entity = &self.mob_entity.living_entity.entity;
        if !entity.is_alive() {
            return;
        }

        // Vanilla `Vex.tick`: no physics while the mob ticks, then gravity is off again.
        entity.no_physics.store(true, Ordering::Relaxed);
        if !entity.has_no_gravity() {
            entity.set_has_no_gravity(true);
        }

        if self.has_limited_life.load(Ordering::Relaxed) {
            let remaining = self.limited_life_ticks.fetch_sub(1, Ordering::Relaxed) - 1;
            if remaining <= 0 {
                self.limited_life_ticks.store(20, Ordering::Relaxed);
                caller.damage(caller, 1.0, DamageType::STARVE);
            }
        }
    }

    fn post_tick(&self) {
        self.mob_entity
            .living_entity
            .entity
            .no_physics
            .store(false, Ordering::Relaxed);
    }

    fn mob_write_nbt(&self, nbt: &mut NbtCompound) {
        if let Some(pos) = self.bound_origin.load() {
            let mut sub = NbtCompound::new();
            sub.put_int("x", pos.0.x);
            sub.put_int("y", pos.0.y);
            sub.put_int("z", pos.0.z);
            nbt.put_compound("bound_pos", sub);
        }
        if self.has_limited_life.load(Ordering::Relaxed) {
            nbt.put_int(
                "life_ticks",
                self.limited_life_ticks.load(Ordering::Relaxed),
            );
        }
        if let Some(owner) = *self
            .owner_uuid
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
        {
            nbt.put_uuid("owner", owner);
        }
    }

    fn mob_read_nbt(&self, nbt: &NbtCompound) {
        if let Some(sub) = nbt.get_compound("bound_pos")
            && let (Some(x), Some(y), Some(z)) =
                (sub.get_int("x"), sub.get_int("y"), sub.get_int("z"))
        {
            self.set_bound_origin(BlockPos::new(x, y, z));
        }
        if let Some(life_ticks) = nbt.get_int("life_ticks") {
            self.set_limited_life(life_ticks);
        }
        if let Some(owner) = nbt.get_uuid("owner") {
            *self
                .owner_uuid
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner) = Some(owner);
        }
    }
}

/// Vanilla `Vex.VexMoveControl`: the vex accelerates towards its wanted point, slowing down
/// only when it is already inside its own bounding box.
#[derive(Default)]
struct VexMoveControl {
    wanted_x: f64,
    wanted_y: f64,
    wanted_z: f64,
    speed_modifier: f64,
    operation: Operation,
}

impl Control for VexMoveControl {}

impl MoveControlTrait for VexMoveControl {
    fn tick(&mut self, mob: &dyn Mob) {
        if self.operation != Operation::MoveTo {
            return;
        }

        let entity = mob.get_entity();
        let pos = entity.pos.load();
        let delta = Vector3::new(
            self.wanted_x - pos.x,
            self.wanted_y - pos.y,
            self.wanted_z - pos.z,
        );
        let delta_length = delta.length();

        if delta_length < entity.bounding_box.load().get_average_side_length() {
            self.operation = Operation::Wait;
            let velocity = entity.velocity.load();
            entity.velocity.store(Vector3::new(
                velocity.x * 0.5,
                velocity.y * 0.5,
                velocity.z * 0.5,
            ));
        } else {
            let scale = self.speed_modifier * 0.05 / delta_length;
            let velocity = entity.velocity.load();
            entity.velocity.store(Vector3::new(
                velocity.x + delta.x * scale,
                velocity.y + delta.y * scale,
                velocity.z + delta.z * scale,
            ));

            let yaw = mob.get_mob_entity().get_target().map_or_else(
                || {
                    let velocity = entity.velocity.load();
                    -(velocity.x.atan2(velocity.z).to_degrees() as f32)
                },
                |target| {
                    let target_pos = target.get_entity().pos.load();
                    let tx = target_pos.x - pos.x;
                    let tz = target_pos.z - pos.z;
                    -(tx.atan2(tz).to_degrees() as f32)
                },
            );
            entity.yaw.store(yaw);
            entity.body_yaw.store(yaw);
        }
    }

    fn set_wanted_position(&mut self, x: f64, y: f64, z: f64, speed_modifier: f64) {
        self.wanted_x = x;
        self.wanted_y = y;
        self.wanted_z = z;
        self.speed_modifier = speed_modifier;
        self.operation = Operation::MoveTo;
    }

    fn has_wanted(&self) -> bool {
        self.operation == Operation::MoveTo
    }
}

struct VexChargeAttackGoal {
    vex: std::sync::Weak<VexEntity>,
}

impl VexChargeAttackGoal {
    const fn new(vex: std::sync::Weak<VexEntity>) -> Self {
        Self { vex }
    }
}

impl Goal for VexChargeAttackGoal {
    fn can_start(&mut self, mob: &dyn Mob) -> bool {
        let Some(target) = mob.get_mob_entity().get_target() else {
            return false;
        };
        if !target.get_entity().is_alive() {
            return false;
        }

        let has_wanted = mob
            .get_mob_entity()
            .move_control
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .has_wanted();

        !has_wanted
            && mob.get_random().random_range(0..CHARGE_CHANCE) == 0
            && mob
                .get_entity()
                .pos
                .load()
                .squared_distance_to_vec(&target.get_entity().pos.load())
                > 4.0
    }

    fn should_continue(&mut self, mob: &dyn Mob) -> bool {
        self.vex.upgrade().is_some_and(|vex| vex.is_charging())
            && mob
                .get_mob_entity()
                .move_control
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .has_wanted()
            && mob
                .get_mob_entity()
                .get_target()
                .is_some_and(|target| target.get_entity().is_alive())
    }

    fn start(&mut self, mob: &dyn Mob) {
        let Some(target) = mob.get_mob_entity().get_target() else {
            return;
        };
        let eye_position = target.get_eye_pos();
        mob.get_mob_entity()
            .move_control
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .set_wanted_position(eye_position.x, eye_position.y, eye_position.z, 1.0);

        if let Some(vex) = self.vex.upgrade() {
            vex.set_is_charging(true);
        }
        let entity = mob.get_entity();
        entity.world.load().play_sound_fine(
            Sound::EntityVexCharge,
            SoundCategory::Hostile,
            &entity.pos.load(),
            1.0,
            1.0,
        );
    }

    fn stop(&mut self, _mob: &dyn Mob) {
        if let Some(vex) = self.vex.upgrade() {
            vex.set_is_charging(false);
        }
    }

    fn tick(&mut self, mob: &dyn Mob) {
        let Some(target) = mob.get_mob_entity().get_target() else {
            return;
        };
        let target_entity = target.get_entity();

        if mob
            .get_entity()
            .bounding_box
            .load()
            .intersects(&target_entity.bounding_box.load())
        {
            if mob
                .get_mob_entity()
                .try_attack(mob.get_entity(), target.as_ref())
            {
                mob.on_attack(target.as_ref());
            }
            if let Some(vex) = self.vex.upgrade() {
                vex.set_is_charging(false);
            }
        } else if mob
            .get_entity()
            .pos
            .load()
            .squared_distance_to_vec(&target_entity.pos.load())
            < 9.0
        {
            let eye_position = target.get_eye_pos();
            mob.get_mob_entity()
                .move_control
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .set_wanted_position(eye_position.x, eye_position.y, eye_position.z, 1.0);
        }
    }

    fn should_run_every_tick(&self) -> bool {
        true
    }

    fn controls(&self) -> Controls {
        Controls::MOVE
    }
}

/// Vanilla `Vex.VexCopyOwnerTargetGoal`: copies the owner's current target.
struct VexCopyOwnerTargetGoal {
    vex: std::sync::Weak<VexEntity>,
    track_target_goal: TrackTargetGoal,
    target: Option<Arc<dyn EntityBase>>,
}

impl VexCopyOwnerTargetGoal {
    #[expect(
        clippy::unnecessary_box_returns,
        reason = "GoalSelector takes boxed goals"
    )]
    fn new(vex: std::sync::Weak<VexEntity>) -> Box<Self> {
        Box::new(Self {
            vex,
            track_target_goal: TrackTargetGoal::with_default(false),
            target: None,
        })
    }
}

impl Goal for VexCopyOwnerTargetGoal {
    fn can_start(&mut self, mob: &dyn Mob) -> bool {
        let Some(vex) = self.vex.upgrade() else {
            return false;
        };
        let Some(owner) = vex.get_owner() else {
            return false;
        };
        let Some(owner_mob) = owner.get_mob() else {
            return false;
        };
        let Some(target) = owner_mob.get_mob_entity().get_target() else {
            return false;
        };
        if !target.get_entity().is_alive() {
            return false;
        }

        let target_predicate =
            crate::entity::ai::target_predicate::TargetPredicate::create_non_attackable()
                .ignore_visibility()
                .ignore_distance_scaling_factor();
        if !self
            .track_target_goal
            .can_track(mob, Some(target.as_ref()), &target_predicate)
            || !mob.can_attack(target.as_ref())
        {
            return false;
        }

        self.target = Some(target);
        true
    }

    fn should_continue(&mut self, mob: &dyn Mob) -> bool {
        self.track_target_goal.should_continue(mob)
    }

    fn start(&mut self, mob: &dyn Mob) {
        mob.set_mob_target(self.target.clone());
        self.track_target_goal.set_target_mob(self.target.clone());
        self.track_target_goal.start(mob);
    }

    fn stop(&mut self, mob: &dyn Mob) {
        self.target = None;
        self.track_target_goal.stop(mob);
    }

    fn controls(&self) -> Controls {
        Controls::TARGET
    }
}

struct VexRandomMoveGoal {
    vex: std::sync::Weak<VexEntity>,
}

impl VexRandomMoveGoal {
    const fn new(vex: std::sync::Weak<VexEntity>) -> Self {
        Self { vex }
    }
}

impl Goal for VexRandomMoveGoal {
    fn can_start(&mut self, mob: &dyn Mob) -> bool {
        let has_wanted = mob
            .get_mob_entity()
            .move_control
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .has_wanted();
        !has_wanted && mob.get_random().random_range(0..RANDOM_MOVE_CHANCE) == 0
    }

    fn should_continue(&mut self, _mob: &dyn Mob) -> bool {
        false
    }

    fn tick(&mut self, mob: &dyn Mob) {
        let Some(vex) = self.vex.upgrade() else {
            return;
        };
        let bound_origin = vex
            .bound_origin
            .load()
            .unwrap_or_else(|| mob.get_entity().block_pos.load());

        let mut rng = mob.get_random();
        for _ in 0..RANDOM_MOVE_ATTEMPTS {
            let test_pos = BlockPos::new(
                bound_origin.0.x + rng.random_range(-7..8),
                bound_origin.0.y + rng.random_range(-5..6),
                bound_origin.0.z + rng.random_range(-7..8),
            );
            if mob
                .get_entity()
                .world
                .load()
                .get_block_state(&test_pos)
                .is_air()
            {
                let x = f64::from(test_pos.0.x) + 0.5;
                let y = f64::from(test_pos.0.y) + 0.5;
                let z = f64::from(test_pos.0.z) + 0.5;
                mob.get_mob_entity()
                    .move_control
                    .lock()
                    .unwrap_or_else(std::sync::PoisonError::into_inner)
                    .set_wanted_position(x, y, z, 0.25);
                if mob.get_mob_entity().get_target().is_none() {
                    mob.get_mob_entity()
                        .look_control
                        .lock()
                        .unwrap_or_else(std::sync::PoisonError::into_inner)
                        .look_at_with_range(x, y, z, 180.0, 20.0);
                }
                break;
            }
        }
    }

    fn controls(&self) -> Controls {
        Controls::MOVE
    }
}
