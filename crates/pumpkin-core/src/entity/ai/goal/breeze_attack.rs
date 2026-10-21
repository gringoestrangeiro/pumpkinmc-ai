use std::sync::Weak;

use pumpkin_data::Block;
use pumpkin_data::attributes::Attributes;
use pumpkin_data::effect::StatusEffect;
use pumpkin_data::entity::EntityType;
use pumpkin_data::sound::{Sound, SoundCategory};
use pumpkin_util::math::position::BlockPos;
use pumpkin_util::math::vector3::Vector3;
use pumpkin_world::clip::{ClipBlock, ClipContext, ClipFluid};
use rand::{RngExt, seq::SliceRandom};

use crate::entity::{
    EntityBase, EntityPose,
    ai::{
        goal::{Controls, Goal},
        util::default_random_pos,
    },
    mob::{Mob, breeze::BreezeEntity},
    projectile::{
        ThrownItemEntity,
        wind_charge::{WIND_CHARGE_GRAVITY, WindChargeEntity},
    },
};

/// Vanilla `Shoot.SHOOT_INITIAL_DELAY_TICKS`, `Math.round(15.0F)`.
const SHOOT_INITIAL_DELAY_TICKS: i32 = 15;
/// Vanilla `Shoot.SHOOT_RECOVER_DELAY_TICKS`, `Math.round(4.0F)`.
const SHOOT_RECOVER_DELAY_TICKS: i32 = 4;
/// Vanilla `Shoot.SHOOT_COOLDOWN_TICKS`, `Math.round(10.0F)`.
const SHOOT_COOLDOWN_TICKS: i32 = 10;
/// Vanilla `Shoot.ATTACK_RANGE_MAX_SQRT`.
const ATTACK_RANGE_MAX_SQRT: f64 = 256.0;
/// Vanilla `LongJump.INHALING_DURATION_TICKS`, `Math.round(10.0F)`.
const INHALING_DURATION_TICKS: i32 = 10;
/// Vanilla `LongJump.JUMP_COOLDOWN_TICKS`.
const JUMP_COOLDOWN_TICKS: i32 = 10;
/// Vanilla `LongJump.JUMP_COOLDOWN_WHEN_HURT_TICKS`.
const JUMP_COOLDOWN_WHEN_HURT_TICKS: i32 = 2;
/// Vanilla `LongJump` behavior duration.
const LONG_JUMP_MAX_DURATION: i32 = 200;
/// Vanilla `LongJump.MAX_JUMP_VELOCITY_MULTIPLIER`.
const MAX_JUMP_VELOCITY_MULTIPLIER: f64 = 0.058_333_334;
/// Vanilla `LongJump.ALLOWED_ANGLES`.
const ALLOWED_ANGLES: [i32; 5] = [40, 55, 60, 75, 80];
/// Vanilla `BreezeAi.SPEED_MULTIPLIER_WHEN_SLIDING`.
const SPEED_MULTIPLIER_WHEN_SLIDING: f64 = 0.6;
/// Vanilla `BreezeUtil.MAX_LINE_OF_SIGHT_TEST_RANGE`.
const MAX_LINE_OF_SIGHT_TEST_RANGE: f64 = 50.0;
/// Five seconds is long enough for a slide across the 24-block follow range; the timer keeps a
/// blocked slide from holding combat forever.
const SLIDE_TIMEOUT_TICKS: i32 = 100;

#[derive(Clone, Copy, PartialEq, Eq)]
enum BreezePhase {
    Idle,
    Shooting,
    Inhaling,
    Jumping,
}

/// Vanilla `Shoot`, `LongJump`, `ShootWhenStuck` and `Slide` for the goal-based Breeze.
///
/// The brain runs those as separate behaviors gated by memories; the goal selector has no
/// memories, so one goal keeps the same phases and timers in order. It owns movement and look
/// while a target is present, which is what `Activity.FIGHT` does in vanilla.
pub struct BreezeCombatGoal {
    breeze: Weak<BreezeEntity>,
    phase: BreezePhase,
    shoot_cooldown: i32,
    jump_cooldown: i32,
    shoot_ready: bool,
    attack_ticks: i32,
    charging_ticks: i32,
    recovering_ticks: i32,
    fired: bool,
    jump_target: Option<BlockPos>,
    inhaling_ticks: i32,
    leaving_water: bool,
    jump_ticks: i32,
    slide_target: Option<Vector3<f64>>,
    slide_ticks: i32,
}

impl BreezeCombatGoal {
    #[must_use]
    pub const fn new(breeze: Weak<BreezeEntity>) -> Self {
        Self {
            breeze,
            phase: BreezePhase::Idle,
            shoot_cooldown: 0,
            jump_cooldown: 0,
            shoot_ready: false,
            attack_ticks: 0,
            charging_ticks: 0,
            recovering_ticks: 0,
            fired: false,
            jump_target: None,
            inhaling_ticks: 0,
            leaving_water: false,
            jump_ticks: 0,
            slide_target: None,
            slide_ticks: 0,
        }
    }

    fn target(&self) -> Option<std::sync::Arc<dyn EntityBase>> {
        self.breeze
            .upgrade()
            .and_then(|breeze| breeze.mob_entity.get_target())
    }

    fn tick_cooldowns(&mut self) {
        self.shoot_cooldown = self.shoot_cooldown.saturating_sub(1);
        self.jump_cooldown = self.jump_cooldown.saturating_sub(1);
    }

    fn tick_slide(&mut self, breeze: &BreezeEntity) {
        let Some(target) = self.slide_target else {
            return;
        };
        let entity = &breeze.mob_entity.living_entity.entity;
        if entity.pos.load().squared_distance_to_vec(&target) < 1.0 || self.slide_ticks == 0 {
            self.slide_target = None;
        } else {
            self.slide_ticks -= 1;
        }
    }

    fn start_shooting(&mut self, breeze: &BreezeEntity) {
        let entity = &breeze.mob_entity.living_entity.entity;
        entity.set_pose(EntityPose::Shooting);
        entity.world.load().play_sound_fine(
            Sound::EntityBreezeInhale,
            SoundCategory::Hostile,
            &entity.pos.load(),
            1.0,
            1.0,
        );
        self.phase = BreezePhase::Shooting;
        self.attack_ticks = SHOOT_INITIAL_DELAY_TICKS + 1 + SHOOT_RECOVER_DELAY_TICKS;
        self.charging_ticks = SHOOT_INITIAL_DELAY_TICKS;
        self.recovering_ticks = 0;
        self.fired = false;
    }

    fn finish_shooting(&mut self, breeze: &BreezeEntity) {
        let entity = &breeze.mob_entity.living_entity.entity;
        if entity.pose.load() == EntityPose::Shooting {
            entity.set_pose(EntityPose::Standing);
        }
        self.phase = BreezePhase::Idle;
        self.shoot_ready = false;
        self.shoot_cooldown = SHOOT_COOLDOWN_TICKS;
    }

    fn tick_shooting(&mut self, breeze: &BreezeEntity, mob: &dyn Mob, target: &ArcTarget) {
        let entity = &breeze.mob_entity.living_entity.entity;
        let target_pos = target.get_entity().pos.load();
        breeze
            .mob_entity
            .look_control
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .look_at(mob, target_pos.x, target_pos.y, target_pos.z);

        self.attack_ticks -= 1;
        if self.charging_ticks > 0 {
            self.charging_ticks -= 1;
        } else if !self.fired {
            self.fired = true;
            self.recovering_ticks = SHOOT_RECOVER_DELAY_TICKS;
            shoot_wind_charge(breeze, target.as_ref());
        } else if self.recovering_ticks > 0 {
            self.recovering_ticks -= 1;
        }

        if self.attack_ticks <= 0 {
            self.finish_shooting(breeze);
        }
    }

    fn try_start_long_jump(
        &mut self,
        breeze: &BreezeEntity,
        mob: &dyn Mob,
        target: &ArcTarget,
    ) -> bool {
        if self.jump_cooldown > 0 {
            return false;
        }
        if self.slide_target.is_some() {
            return false;
        }
        let entity = &breeze.mob_entity.living_entity.entity;
        if !entity.on_ground.load(std::sync::atomic::Ordering::Relaxed) && !entity.is_in_water() {
            return false;
        }
        if entity
            .touching_lava
            .load(std::sync::atomic::Ordering::Relaxed)
        {
            return false;
        }

        let target_pos = target.get_entity().pos.load();
        let mob_pos = entity.pos.load();
        let distance = mob_pos.squared_distance_to_vec(&target_pos).sqrt();
        let follow_range = breeze
            .mob_entity
            .living_entity
            .get_attribute_value(&Attributes::FOLLOW_RANGE);
        if distance > follow_range {
            mob.set_mob_target(None);
            return false;
        }
        if distance - 4.0 <= 0.0 {
            return false;
        }
        if !can_jump_from_current_position(breeze) {
            return false;
        }

        let mut rng = mob.get_random();
        let behind = random_point_behind_target(target, &mut rng);
        let Some(jump_target) = snap_to_surface(entity.world.load().as_ref(), behind) else {
            return false;
        };
        let world = entity.world.load();
        let block_below = match jump_target.0.y.checked_sub(1) {
            Some(y) => BlockPos::new(jump_target.0.x, y, jump_target.0.z),
            None => return false,
        };
        if Block::from_state_id(world.get_block_state(&block_below).id) == &Block::LAVA {
            return false;
        }
        let target_center = Vector3::new(
            f64::from(jump_target.0.x) + 0.5,
            f64::from(jump_target.0.y),
            f64::from(jump_target.0.z) + 0.5,
        );
        if !has_line_of_sight(breeze, target_center)
            && !has_line_of_sight(breeze, target_center.add_raw(0.0, 4.0, 0.0))
        {
            return false;
        }

        self.jump_target = Some(jump_target);
        self.inhaling_ticks = INHALING_DURATION_TICKS;
        self.phase = BreezePhase::Inhaling;
        entity.set_pose(EntityPose::Inhaling);
        entity.world.load().play_sound_fine(
            Sound::EntityBreezeCharge,
            SoundCategory::Hostile,
            &entity.pos.load(),
            1.0,
            1.0,
        );
        breeze
            .mob_entity
            .look_control
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .look_at(mob, target_center.x, target_center.y, target_center.z);
        true
    }

    fn tick_inhaling(&mut self, breeze: &BreezeEntity, mob: &dyn Mob) {
        self.inhaling_ticks -= 1;
        if self.inhaling_ticks > 0 {
            return;
        }

        let entity = &breeze.mob_entity.living_entity.entity;
        let Some(jump_target) = self.jump_target else {
            entity.set_pose(EntityPose::Standing);
            self.phase = BreezePhase::Idle;
            return;
        };
        let target_pos = Vector3::new(
            f64::from(jump_target.0.x) + 0.5,
            f64::from(jump_target.0.y),
            f64::from(jump_target.0.z) + 0.5,
        );

        let Some(velocity) = calculate_optimal_jump_vector(breeze, mob, target_pos) else {
            entity.set_pose(EntityPose::Standing);
            self.phase = BreezePhase::Idle;
            self.jump_target = None;
            return;
        };

        self.leaving_water = entity.is_in_water();
        entity.world.load().play_sound_fine(
            Sound::EntityBreezeJump,
            SoundCategory::Hostile,
            &entity.pos.load(),
            1.0,
            1.0,
        );
        entity.set_pose(EntityPose::LongJumping);
        entity.yaw.store(entity.body_yaw.load());
        entity.set_velocity(velocity);
        self.jump_ticks = LONG_JUMP_MAX_DURATION;
        self.phase = BreezePhase::Jumping;
    }

    fn tick_jumping(&mut self, breeze: &BreezeEntity) {
        let entity = &breeze.mob_entity.living_entity.entity;
        let on_ground = entity.on_ground.load(std::sync::atomic::Ordering::Relaxed);
        let landed_in_water = entity.is_in_water() && !self.leaving_water;
        self.jump_ticks -= 1;
        let timed_out = self.jump_ticks <= 0;
        if !on_ground && !landed_in_water && !timed_out {
            return;
        }
        if timed_out {
            entity.set_pose(EntityPose::Standing);
            self.phase = BreezePhase::Idle;
            self.jump_target = None;
            return;
        }

        entity.world.load().play_sound_fine(
            Sound::EntityBreezeLand,
            SoundCategory::Hostile,
            &entity.pos.load(),
            1.0,
            1.0,
        );
        entity.set_pose(EntityPose::Standing);
        let living = &breeze.mob_entity.living_entity;
        let recently_hurt = living
            .last_attacked_time
            .load(std::sync::atomic::Ordering::Relaxed)
            > 0
            && living.entity.age.load(std::sync::atomic::Ordering::Relaxed)
                - living
                    .last_attacked_time
                    .load(std::sync::atomic::Ordering::Relaxed)
                < 100;
        self.jump_cooldown = if recently_hurt {
            JUMP_COOLDOWN_WHEN_HURT_TICKS
        } else {
            JUMP_COOLDOWN_TICKS
        };
        self.shoot_ready = true;
        self.phase = BreezePhase::Idle;
        self.jump_target = None;
    }

    fn try_slide(&mut self, breeze: &BreezeEntity, mob: &dyn Mob, target: &ArcTarget) -> bool {
        if self.slide_target.is_some() {
            return false;
        }
        let entity = &breeze.mob_entity.living_entity.entity;
        if !entity.on_ground.load(std::sync::atomic::Ordering::Relaxed)
            || entity.is_in_water()
            || entity.pose.load() != EntityPose::Standing
        {
            return false;
        }

        let breeze_pos = entity.pos.load();
        let target_pos = target.get_entity().pos.load();
        let mut destination = None;
        let center = Vector3::new(
            f64::from(entity.block_pos.load().0.x) + 0.5,
            f64::from(entity.block_pos.load().0.y),
            f64::from(entity.block_pos.load().0.z) + 0.5,
        );
        let horizontal_dist_sq =
            (target_pos.x - center.x).powi(2) + (target_pos.z - center.z).powi(2);
        let within_inner_circle =
            horizontal_dist_sq < 16.0 && (target_pos.y - center.y).abs() < 10.0;
        if within_inner_circle
            && let Some(away) = default_random_pos::get_pos_away(breeze, 5, 5, target_pos)
            && has_line_of_sight(breeze, away)
            && target_pos.squared_distance_to_vec(&away)
                > target_pos.squared_distance_to_vec(&breeze_pos)
        {
            destination = Some(away);
        }

        let destination = destination.unwrap_or_else(|| {
            let mut rng = mob.get_random();
            if rng.random::<bool>() {
                random_point_behind_target(target, &mut rng)
            } else {
                random_point_in_middle_circle(breeze_pos, target_pos, &mut rng)
            }
        });
        let destination_block = BlockPos::floored(destination.x, destination.y, destination.z);
        let destination_center = Vector3::new(
            f64::from(destination_block.0.x) + 0.5,
            f64::from(destination_block.0.y),
            f64::from(destination_block.0.z) + 0.5,
        );

        breeze
            .mob_entity
            .move_control
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .set_wanted_position(
                destination_center.x,
                destination_center.y,
                destination_center.z,
                SPEED_MULTIPLIER_WHEN_SLIDING,
            );
        self.slide_target = Some(destination_center);
        self.slide_ticks = SLIDE_TIMEOUT_TICKS;
        true
    }
}

type ArcTarget = std::sync::Arc<dyn EntityBase>;

impl Goal for BreezeCombatGoal {
    fn can_start(&mut self, mob: &dyn Mob) -> bool {
        self.target()
            .is_some_and(|target| target.get_entity().is_alive() && mob.can_attack(target.as_ref()))
    }

    fn should_continue(&mut self, mob: &dyn Mob) -> bool {
        self.can_start(mob)
    }

    fn start(&mut self, _mob: &dyn Mob) {
        self.phase = BreezePhase::Idle;
        self.shoot_ready = true;
        self.jump_target = None;
        self.slide_target = None;
    }

    fn stop(&mut self, _mob: &dyn Mob) {
        let was_shooting = self.phase == BreezePhase::Shooting;
        if let Some(breeze) = self.breeze.upgrade() {
            let entity = &breeze.mob_entity.living_entity.entity;
            if matches!(
                entity.pose.load(),
                EntityPose::Shooting | EntityPose::Inhaling | EntityPose::LongJumping
            ) {
                entity.set_pose(EntityPose::Standing);
            }
        }
        if was_shooting {
            self.shoot_ready = false;
            self.shoot_cooldown = SHOOT_COOLDOWN_TICKS;
        }
        self.phase = BreezePhase::Idle;
        self.jump_target = None;
        self.slide_target = None;
    }

    fn tick(&mut self, mob: &dyn Mob) {
        self.tick_cooldowns();
        let Some(breeze) = self.breeze.upgrade() else {
            return;
        };
        self.tick_slide(&breeze);
        let Some(target) = self.target() else {
            return;
        };

        let entity = &breeze.mob_entity.living_entity.entity;
        let stuck = mob.is_passenger()
            || entity.is_in_water()
            || breeze
                .mob_entity
                .living_entity
                .has_effect(&StatusEffect::LEVITATION);
        if stuck {
            self.shoot_ready = true;
        }

        match self.phase {
            BreezePhase::Shooting => self.tick_shooting(&breeze, mob, &target),
            BreezePhase::Inhaling => self.tick_inhaling(&breeze, mob),
            BreezePhase::Jumping => self.tick_jumping(&breeze),
            BreezePhase::Idle => {
                let distance_sq = entity
                    .pos
                    .load()
                    .squared_distance_to_vec(&target.get_entity().pos.load());
                if self.shoot_ready
                    && self.shoot_cooldown == 0
                    && entity.pose.load() == EntityPose::Standing
                    && self.slide_target.is_none()
                {
                    if distance_sq < ATTACK_RANGE_MAX_SQRT {
                        self.start_shooting(&breeze);
                        return;
                    }
                    self.shoot_ready = false;
                }

                if self.try_start_long_jump(&breeze, mob, &target) {
                    return;
                }
                if self.target().is_some() {
                    self.try_slide(&breeze, mob, &target);
                }
            }
        }
    }

    fn should_run_every_tick(&self) -> bool {
        true
    }

    fn controls(&self) -> Controls {
        Controls::MOVE | Controls::LOOK
    }
}

fn random_point_behind_target(target: &ArcTarget, rng: &mut impl RngExt) -> Vector3<f64> {
    let target_entity = target.get_entity();
    let view_angle =
        target_entity.head_yaw.load() + 180.0 + (next_gaussian(rng) as f32) * 90.0 / 2.0;
    let radius = rng.random_range(4.0f32..8.0);
    let direction = Vector3::from_yaw_pitch(view_angle, 0.0).multiply(
        f64::from(radius),
        f64::from(radius),
        f64::from(radius),
    );
    target_entity.pos.load() + direction
}

fn random_point_in_middle_circle(
    breeze_pos: Vector3<f64>,
    target_pos: Vector3<f64>,
    rng: &mut impl RngExt,
) -> Vector3<f64> {
    let direction = target_pos.sub(&breeze_pos);
    let distance = direction.length() - pumpkin_util::math::lerp(rng.random::<f64>(), 8.0, 4.0);
    let direction = direction.normalize().multiply(distance, distance, distance);
    breeze_pos + direction
}

fn next_gaussian(rng: &mut impl RngExt) -> f64 {
    let u1 = rng.random::<f64>().max(f64::MIN_POSITIVE);
    let u2 = rng.random::<f64>();
    (-2.0 * u1.ln()).sqrt() * (std::f64::consts::TAU * u2).cos()
}

fn shoot_wind_charge(breeze: &BreezeEntity, target: &dyn EntityBase) {
    let entity = &breeze.mob_entity.living_entity.entity;
    let target_entity = target.get_entity();
    let target_pos = target_entity.pos.load();
    let world = entity.world.load();
    let breeze_pos = entity.pos.load();
    let height = f64::from(entity.entity_dimension.load().height);
    let target_y = target_pos.y
        + f64::from(target_entity.entity_dimension.load().height)
            * if target.is_passenger() { 0.8 } else { 0.3 };
    let firing_y = breeze_pos.y + height / 2.0 + 0.3;
    let xd = target_pos.x - breeze_pos.x;
    let yd = target_y - firing_y;
    let zd = target_pos.z - breeze_pos.z;
    let uncertainty = 5.0 - f64::from((world.level_info.load().difficulty as i32) * 4);

    let spawn_pos = Vector3::new(breeze_pos.x, firing_y, breeze_pos.z);
    let projectile =
        crate::entity::Entity::new(world.clone(), spawn_pos, &EntityType::BREEZE_WIND_CHARGE);
    let thrown = ThrownItemEntity::new(projectile, entity, WIND_CHARGE_GRAVITY);
    // `ThrownItemEntity::new` anchors the projectile at the shooter's eye, re-anchor it.
    thrown.entity.pos.store(spawn_pos);
    thrown.set_velocity(xd, yd, zd, 0.7, uncertainty);
    world.spawn_entity(std::sync::Arc::new(WindChargeEntity::new_breeze(thrown)));

    world.play_sound_fine(
        Sound::EntityBreezeShoot,
        SoundCategory::Hostile,
        &breeze_pos,
        1.5,
        1.0,
    );
}

fn can_jump_from_current_position(breeze: &BreezeEntity) -> bool {
    let entity = &breeze.mob_entity.living_entity.entity;
    let current_pos = entity.block_pos.load();
    let world = entity.world.load();
    if Block::from_state_id(world.get_block_state(&current_pos).id) == &Block::HONEY_BLOCK {
        return false;
    }
    for offset in 1..=4 {
        let state = world.get_block_state(&BlockPos::new(
            current_pos.0.x,
            current_pos.0.y + offset,
            current_pos.0.z,
        ));
        if !state.is_air() && Block::from_state_id(state.id) != &Block::WATER {
            return false;
        }
    }
    true
}

fn snap_to_surface(world: &crate::world::World, target: Vector3<f64>) -> Option<BlockPos> {
    let below = world.clip(&ClipContext::new(
        target,
        target.add_raw(0.0, -10.0, 0.0),
        ClipBlock::Collider,
        ClipFluid::None,
    ));
    if !below.is_miss {
        return Some(BlockPos::floored(below.location.x, below.location.y, below.location.z).up());
    }
    let above = world.clip(&ClipContext::new(
        target,
        target.add_raw(0.0, 10.0, 0.0),
        ClipBlock::Collider,
        ClipFluid::None,
    ));
    if above.is_miss {
        None
    } else {
        Some(BlockPos::floored(above.location.x, above.location.y, above.location.z).up())
    }
}

fn has_line_of_sight(breeze: &BreezeEntity, target: Vector3<f64>) -> bool {
    let entity = &breeze.mob_entity.living_entity.entity;
    let from = entity.pos.load();
    let follow_range = breeze
        .mob_entity
        .living_entity
        .get_attribute_value(&Attributes::FOLLOW_RANGE);
    if target.squared_distance_to_vec(&from).sqrt() > MAX_LINE_OF_SIGHT_TEST_RANGE.max(follow_range)
    {
        return false;
    }
    entity
        .world
        .load()
        .clip(&ClipContext::new(
            from,
            target,
            ClipBlock::Collider,
            ClipFluid::None,
        ))
        .is_miss
}

fn calculate_optimal_jump_vector(
    breeze: &BreezeEntity,
    mob: &dyn Mob,
    target_pos: Vector3<f64>,
) -> Option<Vector3<f64>> {
    let max_jump_velocity = MAX_JUMP_VELOCITY_MULTIPLIER
        * breeze
            .mob_entity
            .living_entity
            .get_attribute_value(&Attributes::FOLLOW_RANGE);
    let mut angles = ALLOWED_ANGLES;
    let mut rng = mob.get_random();
    angles.shuffle(&mut rng);
    for angle in angles {
        if let Some(vector) =
            calculate_jump_vector_for_angle(breeze, mob, target_pos, max_jump_velocity, angle)
        {
            return Some(vector);
        }
    }
    None
}

fn calculate_jump_vector_for_angle(
    breeze: &BreezeEntity,
    mob: &dyn Mob,
    target_pos: Vector3<f64>,
    max_jump_velocity: f64,
    angle: i32,
) -> Option<Vector3<f64>> {
    let entity = &breeze.mob_entity.living_entity.entity;
    let mob_pos = entity.pos.load();
    let direction_plane = Vector3::new(target_pos.x - mob_pos.x, 0.0, target_pos.z - mob_pos.z)
        .normalize()
        .multiply(0.5, 0.5, 0.5);
    let target_position = target_pos.sub(&direction_plane);
    let direction = target_position.sub(&mob_pos);
    let angle_rad = f64::from(angle) * std::f64::consts::PI / 180.0;
    let xz_angle = direction.z.atan2(direction.x);
    let r2 = direction.x * direction.x + direction.z * direction.z;
    let r = r2.sqrt();
    let y = direction.y;
    let gravity = mob.get_mob_gravity();
    let sin_2_angle = (2.0 * angle_rad).sin();
    let cos_angle_sq = angle_rad.cos().powi(2);
    let v0_sq = r2 * gravity / (r * sin_2_angle - 2.0 * y * cos_angle_sq);
    if v0_sq < 0.0 {
        return None;
    }
    let v0 = v0_sq.sqrt();
    if v0 > max_jump_velocity {
        return None;
    }
    let v0_r = v0 * angle_rad.cos();
    let v0_y = v0 * angle_rad.sin();
    Some(
        Vector3::new(v0_r * xz_angle.cos(), v0_y, v0_r * xz_angle.sin()).multiply(0.95, 0.95, 0.95),
    )
}
