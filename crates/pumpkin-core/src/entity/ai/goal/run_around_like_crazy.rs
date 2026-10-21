use super::{Controls, Goal};
use crate::entity::ai::pathfinder::NavigatorGoal;
use crate::entity::ai::util::default_random_pos;
use crate::entity::mob::Mob;
use pumpkin_data::entity::EntityStatus;
use pumpkin_protocol::bedrock::server::actor_event::ActorEventID;
use pumpkin_util::math::vector3::Vector3;
use rand::RngExt;

pub struct RunAroundLikeCrazyGoal {
    goal_control: Controls,
    speed_modifier: f64,
    pos_x: f64,
    pos_y: f64,
    pos_z: f64,
}

impl RunAroundLikeCrazyGoal {
    #[must_use]
    pub const fn new(speed_modifier: f64) -> Self {
        Self {
            goal_control: Controls::MOVE,
            speed_modifier,
            pos_x: 0.0,
            pos_y: 0.0,
            pos_z: 0.0,
        }
    }

    /// Vanilla `RunAroundLikeCrazyGoal.tick`: roll taming, then buck the rider.
    fn buck_rider(mob: &dyn Mob) {
        let entity = mob.get_entity();
        let Some(horse) = mob.as_abstract_horse() else {
            return;
        };

        let passenger = entity
            .passengers
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .first()
            .cloned();
        let Some(passenger) = passenger else {
            return;
        };

        if let Some(player) = passenger.get_player() {
            let temper = horse.get_temper();
            let max_temper = horse.get_max_temper();
            let mut rng = mob.get_random();
            if max_temper > 0 && rng.random_range(0..max_temper) < temper {
                horse.tame_with_name(player);
                return;
            }
            horse.modify_temper(5);
        }

        let passenger_ids: Vec<i32> = entity
            .passengers
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .iter()
            .map(|passenger| passenger.get_entity().entity_id)
            .collect();
        for passenger_id in passenger_ids {
            entity.remove_passenger(passenger_id);
        }

        horse.make_mad();
        entity.world.load().broadcast_entity_event(
            entity,
            EntityStatus::TamingFailed,
            Some(ActorEventID::TamingFailed),
        );
    }
}

impl Goal for RunAroundLikeCrazyGoal {
    fn can_start(&mut self, mob: &dyn Mob) -> bool {
        if mob.is_mob_controlled() || mob.is_tamed() || !mob.get_entity().has_passengers() {
            return false;
        }

        let Some(pos) = default_random_pos::get_pos(mob, 5, 4) else {
            return false;
        };

        self.pos_x = pos.x;
        self.pos_y = pos.y;
        self.pos_z = pos.z;
        true
    }

    fn should_continue(&mut self, mob: &dyn Mob) -> bool {
        let is_idle = mob.is_navigator_idle();

        !mob.is_tamed() && !is_idle && mob.get_entity().has_passengers()
    }

    fn start(&mut self, mob: &dyn Mob) {
        let mob_pos = mob.get_entity().pos.load();
        let mut navigator = mob
            .get_mob_entity()
            .navigator
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        navigator.set_progress(NavigatorGoal::new(
            mob_pos,
            Vector3::new(self.pos_x, self.pos_y, self.pos_z),
            self.speed_modifier,
        ));
    }

    fn tick(&mut self, mob: &dyn Mob) {
        if !mob.is_tamed() {
            let tick_delay = self.get_tick_count(50);
            if tick_delay > 0 && mob.get_random().random_range(0..tick_delay) == 0 {
                Self::buck_rider(mob);
            }
        }
    }

    fn controls(&self) -> Controls {
        self.goal_control
    }
}
