use std::sync::Arc;

use crate::entity::{EntityBase, ai::pathfinder::NavigatorGoal, mob::Mob};

use super::{Controls, Goal};

type BreedGate = dyn Fn(&dyn Mob) -> bool + Send + Sync;

pub struct BreedGoal {
    speed: f64,
    mate: Option<Arc<dyn EntityBase>>,
    timer: i32,
    /// Vanilla subclass `canUse` conditions that depend on the mob's state.
    can_use_gate: Option<Box<BreedGate>>,
}

impl BreedGoal {
    #[must_use]
    pub fn new(speed: f64) -> Box<Self> {
        Box::new(Self {
            speed,
            mate: None,
            timer: 0,
            can_use_gate: None,
        })
    }

    #[must_use]
    pub fn with_can_use_gate<F>(mut self: Box<Self>, gate: F) -> Box<Self>
    where
        F: Fn(&dyn Mob) -> bool + Send + Sync + 'static,
    {
        self.can_use_gate = Some(Box::new(gate));
        self
    }

    fn find_mate(mob: &dyn Mob) -> Option<Arc<dyn EntityBase>> {
        let mob_entity = mob.get_mob_entity();
        if !mob_entity.is_in_love() {
            return None;
        }

        let entity = mob.get_entity();
        let pos = entity.pos.load();
        let world = entity.world.load();
        let my_type = entity.entity_type;
        let my_uuid = entity.entity_uuid;
        let animal = mob.as_animal();

        let nearby = world.get_nearby_entities(pos, 8.0);
        let mut closest: Option<(f64, Arc<dyn EntityBase>)> = None;

        for candidate in nearby.values() {
            let c_entity = candidate.get_entity();
            if c_entity.entity_uuid == my_uuid {
                continue;
            }
            if c_entity.entity_type != my_type {
                continue;
            }
            if !candidate.is_in_love() || !candidate.is_breeding_ready() || candidate.is_panicking()
            {
                continue;
            }
            if !animal.is_some_and(|animal| animal.can_breed_with(candidate.as_ref())) {
                continue;
            }

            let dist = pos.squared_distance_to_vec(&c_entity.pos.load());
            match &closest {
                Some((best_dist, _)) if dist >= *best_dist => {}
                _ => closest = Some((dist, candidate.clone())),
            }
        }

        closest.map(|(_, e)| e)
    }

    fn breed(mob: &dyn Mob, mate: &dyn EntityBase) {
        if let Some(animal) = mob.as_animal() {
            animal.spawn_child_from_breeding(mate);
        }
    }
}

impl Goal for BreedGoal {
    fn can_start(&mut self, mob: &dyn Mob) -> bool {
        let mob_entity = mob.get_mob_entity();
        if !mob_entity.is_breeding_ready() || !mob_entity.is_in_love() {
            return false;
        }

        self.mate = Self::find_mate(mob);
        if self.mate.is_none() {
            return false;
        }
        if self.can_use_gate.as_ref().is_some_and(|gate| !gate(mob)) {
            self.mate = None;
            return false;
        }
        true
    }

    fn should_continue(&mut self, _mob: &dyn Mob) -> bool {
        let Some(mate) = &self.mate else {
            return false;
        };

        if !mate.get_entity().is_alive() || mate.is_panicking() {
            return false;
        }

        mate.is_in_love() && self.timer < 60
    }

    fn start(&mut self, _mob: &dyn Mob) {
        self.timer = 0;
    }

    fn stop(&mut self, mob: &dyn Mob) {
        self.mate = None;
        self.timer = 0;
        let mut navigator = mob
            .get_mob_entity()
            .navigator
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        navigator.stop();
    }

    fn tick(&mut self, mob: &dyn Mob) {
        let Some(mate) = &self.mate else {
            return;
        };

        let mob_entity = mob.get_mob_entity();
        let mate_pos = mate.get_entity().pos.load();

        {
            let mut look_control = mob_entity
                .look_control
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            look_control.look_at_entity(mob, mate);
        };

        let my_pos = mob.get_entity().pos.load();
        let dist_sq = my_pos.squared_distance_to_vec(&mate_pos);

        {
            let mut navigator = mob_entity
                .navigator
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            navigator.set_progress(NavigatorGoal::new(my_pos, mate_pos, self.speed));
        };

        self.timer += 1;

        if self.timer >= self.get_tick_count(60) && dist_sq < 9.0 {
            Self::breed(mob, mate.as_ref());
        }
    }

    fn controls(&self) -> Controls {
        Controls::MOVE | Controls::LOOK
    }
}
