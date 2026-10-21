use pumpkin_data::entity::EntityPose;
use pumpkin_data::environment_attribute::Activity;
use pumpkin_util::math::position::BlockPos;

use super::{Controls, Goal};
use crate::entity::ai::pathfinder::NavigatorGoal;
use crate::entity::mob::Mob;

const CLOSE_ENOUGH_TO_BED: f64 = 2.0;

/// Vanilla villager `Activity.REST`: walk home so `SleepInBed` can put the villager to bed.
pub struct SleepInBedGoal {
    speed: f64,
    target: Option<BlockPos>,
}

impl SleepInBedGoal {
    #[must_use]
    pub const fn new(speed: f64) -> Self {
        Self {
            speed,
            target: None,
        }
    }

    fn is_resting(mob: &dyn Mob) -> bool {
        let entity = mob.get_entity();
        entity.world.load().villager_activity(
            &entity.block_pos.load(),
            entity.age.load(std::sync::atomic::Ordering::Relaxed) < 0,
        ) == Activity::Rest
    }

    fn is_sleeping(mob: &dyn Mob) -> bool {
        mob.get_entity().pose.load() == EntityPose::Sleeping
    }
}

impl Goal for SleepInBedGoal {
    fn can_start(&mut self, mob: &dyn Mob) -> bool {
        if !Self::is_resting(mob) {
            return false;
        }
        let Some(home) = mob.get_home() else {
            return false;
        };
        self.target = Some(home);
        true
    }

    fn should_continue(&mut self, mob: &dyn Mob) -> bool {
        let Some(target) = self.target else {
            return false;
        };
        (Self::is_resting(mob) || Self::is_sleeping(mob)) && mob.get_home() == Some(target)
    }

    fn start(&mut self, mob: &dyn Mob) {
        let Some(target) = self.target else {
            return;
        };
        let entity = mob.get_entity();
        if target
            .to_centered_f64()
            .squared_distance_to_vec(&entity.pos.load())
            < CLOSE_ENOUGH_TO_BED.powi(2)
        {
            return;
        }
        mob.get_mob_entity()
            .navigator
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .set_progress(NavigatorGoal::new(
                entity.pos.load(),
                target.to_centered_f64(),
                self.speed,
            ));
    }

    fn stop(&mut self, mob: &dyn Mob) {
        self.target = None;
        mob.get_mob_entity()
            .navigator
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .stop();
    }

    fn tick(&mut self, mob: &dyn Mob) {
        let Some(target) = self.target else {
            return;
        };
        let entity = mob.get_entity();
        if target
            .to_centered_f64()
            .squared_distance_to_vec(&entity.pos.load())
            < CLOSE_ENOUGH_TO_BED.powi(2)
        {
            return;
        }
        mob.get_mob_entity()
            .navigator
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .set_progress(NavigatorGoal::new(
                entity.pos.load(),
                target.to_centered_f64(),
                self.speed,
            ));
    }

    fn controls(&self) -> Controls {
        Controls::MOVE
    }
}
