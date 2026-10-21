use super::{Controls, Goal};
use crate::entity::mob::Mob;
use crate::world::World;
use pumpkin_data::attributes::Attributes;
use pumpkin_data::entity::EntityPose;
use pumpkin_data::fluid::Fluid;
use pumpkin_data::sound::{Sound, SoundCategory};
use pumpkin_data::tag::{self, Taggable};
use pumpkin_util::math::position::BlockPos;
use pumpkin_util::math::vector3::Vector3;
use rand::RngExt;
use rand::seq::SliceRandom;

/// Vanilla `LongJumpToRandomPos`: prepare on the ground, then leap to a random
/// landing spot that walking cannot reach.
pub struct LongJumpGoal {
    controls: Controls,
    max_height: i32,
    max_width: i32,
    max_velocity_multiplier: f32,
    jump_sound: fn(&dyn Mob) -> Sound,
    landing_sound: Sound,
    acceptable_landing: fn(&World, &BlockPos) -> bool,
    cooldown_min: i32,
    cooldown_max: i32,
    cooldown: i32,
    target: Option<BlockPos>,
    jump: Option<Vector3<f64>>,
    prepare_ticks: i32,
    launched: bool,
}

impl LongJumpGoal {
    #[must_use]
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        max_height: i32,
        max_width: i32,
        max_velocity_multiplier: f32,
        cooldown_min: i32,
        cooldown_max: i32,
        jump_sound: fn(&dyn Mob) -> Sound,
        landing_sound: Sound,
        acceptable_landing: fn(&World, &BlockPos) -> bool,
    ) -> Self {
        let mut goal = Self {
            controls: Controls::MOVE | Controls::LOOK | Controls::JUMP,
            max_height,
            max_width,
            max_velocity_multiplier,
            jump_sound,
            landing_sound,
            acceptable_landing,
            cooldown_min,
            cooldown_max,
            cooldown: 0,
            target: None,
            jump: None,
            prepare_ticks: 0,
            launched: false,
        };
        goal.reset_cooldown();
        goal
    }

    fn reset_cooldown(&mut self) {
        self.cooldown = rand::rng().random_range(self.cooldown_min..=self.cooldown_max);
    }

    fn find_jump(mob: &dyn Mob, goal: &LongJumpGoal) -> Option<(BlockPos, Vector3<f64>)> {
        let entity = mob.get_entity();
        let world = entity.world.load_full();
        let origin = entity.block_pos.load();

        for _ in 0..20 {
            let candidate = BlockPos::new(
                origin.0.x + rand::rng().random_range(-goal.max_width..=goal.max_width),
                origin.0.y + rand::rng().random_range(-goal.max_height..=goal.max_height),
                origin.0.z + rand::rng().random_range(-goal.max_width..=goal.max_width),
            );
            if candidate.0.x == origin.0.x && candidate.0.z == origin.0.z {
                continue;
            }
            if !(goal.acceptable_landing)(&world, &candidate) {
                continue;
            }
            if let Some(jump) = calculate_jump_vector(mob, &candidate, goal.max_velocity_multiplier)
            {
                return Some((candidate, jump));
            }
        }

        None
    }
}

/// Vanilla `LongJumpUtil.calculateJumpVectorForAngle` without the collision sampling.
fn calculate_jump_vector(
    mob: &dyn Mob,
    target: &BlockPos,
    max_velocity_multiplier: f32,
) -> Option<Vector3<f64>> {
    let entity = mob.get_entity();
    let living = &mob.get_mob_entity().living_entity;
    let mob_pos = entity.pos.load();
    let target_center = Vector3::new(
        f64::from(target.0.x) + 0.5,
        f64::from(target.0.y) + 0.5,
        f64::from(target.0.z) + 0.5,
    );
    let flat = Vector3::new(
        target_center.x - mob_pos.x,
        0.0,
        target_center.z - mob_pos.z,
    );
    if flat.length_squared() < 1.0e-8 {
        return None;
    }
    let direction = target_center - flat.normalize() * 0.5 - mob_pos;
    let r2 = direction.x * direction.x + direction.z * direction.z;
    let r = r2.sqrt();
    let y = direction.y;
    let gravity = living.get_attribute_value(&Attributes::GRAVITY);
    let max_velocity =
        living.get_attribute_value(&Attributes::JUMP_STRENGTH) * f64::from(max_velocity_multiplier);

    let mut angles = [65.0_f64, 70.0, 75.0, 80.0];
    let mut rng = rand::rng();
    angles.shuffle(&mut rng);

    for angle in angles {
        let radians = angle.to_radians();
        let denominator = r * (2.0 * radians).sin() - 2.0 * y * radians.cos().powi(2);
        if denominator.abs() < 1.0e-10 {
            continue;
        }
        let velocity_sq = r2 * gravity / denominator;
        if velocity_sq < 0.0 {
            continue;
        }
        let velocity = velocity_sq.sqrt();
        if velocity > max_velocity {
            continue;
        }
        let horizontal = velocity * radians.cos();
        return Some(
            Vector3::new(
                direction.x / r * horizontal,
                velocity * radians.sin(),
                direction.z / r * horizontal,
            ) * 0.95,
        );
    }

    None
}

/// Vanilla `LongJumpToRandomPos.defaultAcceptableLandingSpot`.
#[must_use]
pub fn default_acceptable_landing(world: &World, pos: &BlockPos) -> bool {
    world.get_block_state(&pos.down()).is_solid_render()
}

/// Vanilla `FrogAi.isAcceptableLandingSpot`.
#[must_use]
pub fn frog_acceptable_landing(world: &World, pos: &BlockPos) -> bool {
    let below = pos.down();
    if world.get_fluid(pos).id != Fluid::EMPTY.id
        || world.get_fluid(&below).id != Fluid::EMPTY.id
        || world.get_fluid(&pos.up()).id != Fluid::EMPTY.id
    {
        return false;
    }
    if world
        .get_block(pos)
        .has_tag(&tag::Block::MINECRAFT_FROG_PREFER_JUMP_TO)
        || world
            .get_block(&below)
            .has_tag(&tag::Block::MINECRAFT_FROG_PREFER_JUMP_TO)
    {
        return true;
    }
    default_acceptable_landing(world, pos)
}

impl Goal for LongJumpGoal {
    fn can_start(&mut self, mob: &dyn Mob) -> bool {
        if self.cooldown > 0 {
            self.cooldown -= 1;
            return false;
        }

        let entity = mob.get_entity();
        if !entity.on_ground.load(std::sync::atomic::Ordering::Relaxed)
            || entity.is_in_water()
            || entity
                .touching_lava
                .load(std::sync::atomic::Ordering::Relaxed)
            || entity.has_passengers()
        {
            self.reset_cooldown();
            return false;
        }

        let Some((target, jump)) = Self::find_jump(mob, self) else {
            self.reset_cooldown();
            return false;
        };

        self.target = Some(target);
        self.jump = Some(jump);
        self.prepare_ticks = 40;
        self.launched = false;
        true
    }

    fn should_continue(&mut self, mob: &dyn Mob) -> bool {
        if self.launched {
            !mob.get_entity()
                .on_ground
                .load(std::sync::atomic::Ordering::Relaxed)
        } else {
            self.prepare_ticks > 0 && !mob.get_entity().is_in_water()
        }
    }

    fn start(&mut self, mob: &dyn Mob) {
        let Some(target) = self.target else {
            return;
        };
        let entity = mob.get_entity();
        let pos = entity.pos.load();
        let target_pos = Vector3::new(
            f64::from(target.0.x) + 0.5,
            entity.get_eye_height() + pos.y,
            f64::from(target.0.z) + 0.5,
        );
        entity.yaw.store(
            (target_pos.z - pos.z)
                .atan2(target_pos.x - pos.x)
                .to_degrees() as f32
                - 90.0,
        );
        mob.get_mob_entity()
            .navigator
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .stop();
    }

    fn stop(&mut self, mob: &dyn Mob) {
        let entity = mob.get_entity();
        if self.launched && entity.on_ground.load(std::sync::atomic::Ordering::Relaxed) {
            let velocity = entity.velocity.load();
            entity
                .velocity
                .store(Vector3::new(velocity.x * 0.1, velocity.y, velocity.z * 0.1));
            entity.world.load().play_sound_fine(
                self.landing_sound,
                SoundCategory::Neutral,
                &entity.pos.load(),
                2.0,
                1.0,
            );
        }
        entity.set_pose(EntityPose::Standing);
        self.target = None;
        self.jump = None;
        self.prepare_ticks = 0;
        self.launched = false;
        self.reset_cooldown();
    }

    fn tick(&mut self, mob: &dyn Mob) {
        if self.launched {
            return;
        }
        if self.prepare_ticks > 0 {
            self.prepare_ticks -= 1;
            return;
        }
        let Some(jump) = self.jump else {
            return;
        };
        let entity = mob.get_entity();
        entity.set_velocity(jump);
        entity.set_pose(EntityPose::LongJumping);
        entity.world.load().play_sound_fine(
            (self.jump_sound)(mob),
            SoundCategory::Neutral,
            &entity.pos.load(),
            1.0,
            1.0,
        );
        self.launched = true;
    }

    fn should_run_every_tick(&self) -> bool {
        true
    }

    fn controls(&self) -> Controls {
        self.controls
    }
}
