use std::sync::Arc;

use super::{Controls, Goal};
use crate::entity::ai::target_predicate::TargetPredicate;
use crate::entity::ai::util::default_random_pos;
use crate::entity::predicate::EntityPredicate;
use crate::entity::{EntityBase, ai::pathfinder::NavigatorGoal, mob::Mob};
use pumpkin_data::entity::EntityType;
use pumpkin_util::math::vector3::Vector3;

const FAST_DISTANCE_SQ: f64 = 49.0;
const HORIZONTAL_RANGE: i32 = 16;
const VERTICAL_RANGE: i32 = 7;
const SEARCH_Y_RANGE: f64 = 3.0;

/// Extra condition deciding whether the mob avoids anything at all right now.
type AvoidGate = dyn Fn(&dyn Mob) -> bool + Send + Sync;
/// Extra condition on the found threat, vanilla's `AvoidEntityTargetingCondition`.
type TargetGate = dyn Fn(&dyn EntityBase) -> bool + Send + Sync;

pub struct AvoidEntityGoal {
    goal_control: Controls,
    flee_type: &'static EntityType,
    flee_distance: f64,
    slow_speed: f64,
    fast_speed: f64,
    target: Option<Arc<dyn EntityBase>>,
    flee_pos: Option<Vector3<f64>>,
    gate: Option<Box<AvoidGate>>,
    target_gate: Option<Box<TargetGate>>,
}

impl AvoidEntityGoal {
    /// Creates the goal with no gate: the mob always flees when `flee_type` is found.
    ///
    /// Use [`Self::with_gate`] when the flee decision depends on mob state (tame, trust).
    #[must_use]
    pub fn new(
        flee_type: &'static EntityType,
        flee_distance: f64,
        slow_speed: f64,
        fast_speed: f64,
    ) -> Self {
        Self::build(flee_type, flee_distance, slow_speed, fast_speed, None)
    }

    /// Like [`Self::new`], but the goal is skipped entirely while `gate` returns false.
    ///
    /// Vanilla subclasses such as `Cat$CatAvoidEntityGoal` and `Ocelot$OcelotAvoidEntityGoal`
    /// override `canUse`/`canContinueToUse` with the mob's tame or trust state; that check has to
    /// run on every start attempt, not only when the goal is registered.
    #[must_use]
    pub fn with_gate<F>(
        flee_type: &'static EntityType,
        flee_distance: f64,
        slow_speed: f64,
        fast_speed: f64,
        gate: F,
    ) -> Self
    where
        F: Fn(&dyn Mob) -> bool + Send + Sync + 'static,
    {
        Self::build(
            flee_type,
            flee_distance,
            slow_speed,
            fast_speed,
            Some(Box::new(gate)),
        )
    }

    /// Rejects a found threat for which `gate` returns false.
    ///
    /// Vanilla `Spider` passes `entity -> !armadillo.isScared()` as its
    /// `AvoidEntityTargetingCondition`, so a scared armadillo is not avoided.
    #[must_use]
    pub fn with_target_gate<F>(
        flee_type: &'static EntityType,
        flee_distance: f64,
        slow_speed: f64,
        fast_speed: f64,
        gate: F,
    ) -> Self
    where
        F: Fn(&dyn EntityBase) -> bool + Send + Sync + 'static,
    {
        let mut goal = Self::build(flee_type, flee_distance, slow_speed, fast_speed, None);
        goal.target_gate = Some(Box::new(gate));
        goal
    }

    /// Shared constructor behind [`Self::new`] and [`Self::with_gate`].
    fn build(
        flee_type: &'static EntityType,
        flee_distance: f64,
        slow_speed: f64,
        fast_speed: f64,
        gate: Option<Box<AvoidGate>>,
    ) -> Self {
        Self {
            goal_control: Controls::MOVE,
            flee_type,
            flee_distance,
            slow_speed,
            fast_speed,
            target: None,
            flee_pos: None,
            gate,
            target_gate: None,
        }
    }

    /// Whether the gate (if any) lets the goal run right now; no gate always allows.
    fn gate_allows(&self, mob: &dyn Mob) -> bool {
        self.gate.as_ref().is_none_or(|gate| gate(mob))
    }

    /// Vanilla `AvoidEntityGoal.canUse` search: the nearest entity in the mob's box grown by
    /// `maxDist` (3 vertically) that passes `avoidEntityTargeting`, a combat targeting check.
    /// So the mob must see the threat, and sneaking or invisible threats are noticed closer.
    fn find_threat(&self, mob: &dyn Mob) -> Option<Arc<dyn EntityBase>> {
        let entity = &mob.get_mob_entity().living_entity.entity;
        let pos = entity.pos.load();
        let world = entity.world.load();
        let search_area = entity.bounding_box.load().expand(
            self.flee_distance,
            SEARCH_Y_RANGE,
            self.flee_distance,
        );
        let avoid_entity_targeting =
            TargetPredicate::create_attackable().set_base_max_distance(self.flee_distance);
        let is_threat = |candidate: &dyn EntityBase| {
            EntityPredicate::ExceptCreativeOrSpectator.test(candidate)
                && self.target_gate.as_ref().is_none_or(|gate| gate(candidate))
                && candidate
                    .get_entity()
                    .bounding_box
                    .load()
                    .intersects(&search_area)
                && avoid_entity_targeting.test(&world, Some(mob), candidate)
        };

        if self.flee_type == &EntityType::PLAYER {
            world
                .get_nearest_player(pos, self.flee_distance, |player| is_threat(player.as_ref()))
                .map(|p| p as Arc<dyn EntityBase>)
        } else {
            world.get_nearest_entity(pos, self.flee_distance, Some(&[self.flee_type]), |entity| {
                is_threat(entity.as_ref())
            })
        }
    }
}

impl Goal for AvoidEntityGoal {
    fn can_start(&mut self, mob: &dyn Mob) -> bool {
        if !self.gate_allows(mob) {
            return false;
        }

        let Some(target) = self.find_threat(mob) else {
            return false;
        };

        let threat_pos = target.get_entity().pos.load();
        let Some(flee_pos) =
            default_random_pos::get_pos_away(mob, HORIZONTAL_RANGE, VERTICAL_RANGE, threat_pos)
        else {
            return false;
        };

        // Give up when the escape route does not gain any distance.
        let mob_pos = mob.get_entity().pos.load();
        if threat_pos.squared_distance_to_vec(&flee_pos)
            < threat_pos.squared_distance_to_vec(&mob_pos)
        {
            return false;
        }

        self.target = Some(target);
        self.flee_pos = Some(flee_pos);
        true
    }

    fn should_continue(&mut self, mob: &dyn Mob) -> bool {
        self.gate_allows(mob) && !mob.is_navigator_idle()
    }

    fn start(&mut self, mob: &dyn Mob) {
        if let Some(flee_pos) = self.flee_pos {
            let mob_pos = mob.get_mob_entity().living_entity.entity.pos.load();
            let mut navigator = mob
                .get_mob_entity()
                .navigator
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            navigator.set_progress(NavigatorGoal::new(mob_pos, flee_pos, self.slow_speed));
        }
    }

    fn tick(&mut self, mob: &dyn Mob) {
        if let Some(target) = &self.target {
            let mob_pos = mob.get_mob_entity().living_entity.entity.pos.load();
            let threat_pos = target.get_entity().pos.load();
            let dist_sq = mob_pos.squared_distance_to_vec(&threat_pos);
            let speed = if dist_sq < FAST_DISTANCE_SQ {
                self.fast_speed
            } else {
                self.slow_speed
            };
            let mut navigator = mob
                .get_mob_entity()
                .navigator
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            navigator.set_speed(speed);
        }
    }

    fn stop(&mut self, _mob: &dyn Mob) {
        self.target = None;
        self.flee_pos = None;
    }

    fn controls(&self) -> Controls {
        self.goal_control
    }
}
