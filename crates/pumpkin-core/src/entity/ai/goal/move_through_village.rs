use pumpkin_util::math::position::BlockPos;

use super::{Controls, Goal};
use crate::entity::ai::pathfinder::NavigatorGoal;
use crate::entity::mob::Mob;

/// A small, goal-based stand-in for vanilla `MoveThroughVillageGoal`.
///
/// Vanilla walks between occupied village POIs; Pumpkin tracks job sites rather
/// than door POIs, so a zombie villager walks to the nearest unvisited job site
/// in the village. The night and distance gates match the vanilla goal.
pub struct MoveThroughVillageGoal {
    speed: f64,
    only_at_night: bool,
    distance_to_poi: f64,
    target: Option<BlockPos>,
    visited: Vec<BlockPos>,
    cooldown: i32,
}

impl MoveThroughVillageGoal {
    #[must_use]
    pub const fn new(speed: f64, only_at_night: bool, distance_to_poi: f64) -> Self {
        Self {
            speed,
            only_at_night,
            distance_to_poi,
            target: None,
            visited: Vec::new(),
            cooldown: 0,
        }
    }

    fn find_poi(&self, mob: &dyn Mob) -> Option<BlockPos> {
        let entity = mob.get_entity();
        let world = entity.world.load();
        if self.only_at_night
            && !world
                .level_time
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .is_night()
        {
            return None;
        }

        let origin = entity.block_pos.load();
        let poi = world
            .villager_poi
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .get_nearest_job_site(origin, 32)?;
        (!self.visited.contains(&poi)).then_some(poi)
    }

    fn distance_to(&self, mob: &dyn Mob, target: BlockPos) -> f64 {
        mob.get_entity()
            .pos
            .load()
            .squared_distance_to_vec(&target.to_centered_f64())
    }

    fn move_to(&self, mob: &dyn Mob, target: BlockPos) {
        let mob_pos = mob.get_entity().pos.load();
        mob.get_mob_entity()
            .navigator
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .set_progress(NavigatorGoal::new(
                mob_pos,
                target.to_centered_f64(),
                self.speed,
            ));
    }
}

impl Goal for MoveThroughVillageGoal {
    fn can_start(&mut self, mob: &dyn Mob) -> bool {
        if self.cooldown > 0 {
            self.cooldown -= 1;
            return false;
        }
        if mob.get_mob_entity().get_target().is_some() {
            return false;
        }
        let Some(poi) = self.find_poi(mob) else {
            return false;
        };
        if self.distance_to(mob, poi) <= self.distance_to_poi * self.distance_to_poi {
            return false;
        }
        self.target = Some(poi);
        true
    }

    fn should_continue(&mut self, mob: &dyn Mob) -> bool {
        let Some(target) = self.target else {
            return false;
        };
        !mob.is_navigator_idle()
            && self.distance_to(mob, target) > self.distance_to_poi * self.distance_to_poi
    }

    fn start(&mut self, mob: &dyn Mob) {
        if let Some(target) = self.target {
            self.move_to(mob, target);
        }
    }

    fn tick(&mut self, mob: &dyn Mob) {
        if mob.is_navigator_idle()
            && let Some(target) = self.target
        {
            self.move_to(mob, target);
        }
    }

    fn stop(&mut self, mob: &dyn Mob) {
        if let Some(target) = self.target
            && self.visited.len() < 15
        {
            self.visited.push(target);
        }
        self.target = None;
        self.cooldown = 200;
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
