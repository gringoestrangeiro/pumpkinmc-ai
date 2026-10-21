use super::{Controls, Goal};
use crate::entity::ai::pathfinder::NavigatorGoal;
use crate::entity::mob::Mob;
use crate::entity::passive::llama::LlamaCaravan;
use crate::entity::{EntityBase, decoration::leash_knot::LeashKnotEntity};
use pumpkin_data::entity::EntityType;
use std::sync::Arc;

const CARAVAN_LIMIT: i32 = 8;

pub struct LlamaFollowCaravanGoal {
    speed_modifier: f64,
    dist_check_counter: i32,
}

impl LlamaFollowCaravanGoal {
    #[must_use]
    pub const fn new(speed_modifier: f64) -> Self {
        Self {
            speed_modifier,
            dist_check_counter: 0,
        }
    }

    fn caravan_llama(entity: &Arc<dyn EntityBase>) -> Option<&dyn LlamaCaravan> {
        entity.get_mob().and_then(Mob::as_caravan_llama)
    }

    fn self_llama(mob: &dyn Mob) -> Option<&dyn LlamaCaravan> {
        mob.as_caravan_llama()
    }

    fn first_is_leashed(
        current: &Arc<dyn EntityBase>,
        counter: i32,
        world: &crate::world::World,
    ) -> bool {
        if counter > CARAVAN_LIMIT {
            return false;
        }
        let Some(caravan) = Self::caravan_llama(current) else {
            return false;
        };
        let Some(head_id) = caravan.get_caravan_head_id() else {
            return false;
        };
        let Some(head) = world.get_entity_by_id(head_id) else {
            return false;
        };
        if head.get_entity().is_leashed() {
            true
        } else {
            Self::first_is_leashed(&head, counter + 1, world)
        }
    }

    fn join_caravan(mob: &dyn Mob, head: &Arc<dyn EntityBase>) {
        let self_id = mob.get_entity().entity_id;
        if let Some(self_caravan) = Self::self_llama(mob) {
            self_caravan.set_caravan_head_id(Some(head.get_entity().entity_id));
        }
        if let Some(head_caravan) = Self::caravan_llama(head) {
            head_caravan.set_caravan_tail_id(Some(self_id));
        }
    }

    fn leave_caravan(mob: &dyn Mob) {
        let Some(self_caravan) = Self::self_llama(mob) else {
            return;
        };
        let Some(head_id) = self_caravan.get_caravan_head_id() else {
            return;
        };
        self_caravan.set_caravan_head_id(None);
        let world = mob.get_entity().world.load();
        if let Some(head) = world.get_entity_by_id(head_id)
            && let Some(head_caravan) = Self::caravan_llama(&head)
        {
            head_caravan.set_caravan_tail_id(None);
        }
    }

    fn find_caravan_head(mob: &dyn Mob) -> Option<Arc<dyn EntityBase>> {
        let entity = mob.get_entity();
        let world = entity.world.load();
        let search = entity.bounding_box.load().expand(9.0, 4.0, 9.0);
        let llamas: Vec<Arc<dyn EntityBase>> = world
            .get_entities_at_box(&search)
            .into_iter()
            .filter(|candidate| {
                candidate.get_entity().entity_type.id == EntityType::LLAMA.id
                    || candidate.get_entity().entity_type.id == EntityType::TRADER_LLAMA.id
            })
            .filter(|candidate| candidate.get_entity().entity_id != entity.entity_id)
            .collect();

        let mob_pos = entity.pos.load();
        let mut closest: Option<(f64, Arc<dyn EntityBase>)> = None;
        for candidate in &llamas {
            let Some(caravan) = Self::caravan_llama(candidate) else {
                continue;
            };
            if !caravan.in_caravan() || caravan.has_caravan_tail() {
                continue;
            }
            let dist_sq = mob_pos.squared_distance_to_vec(&candidate.get_entity().pos.load());
            if closest.as_ref().is_none_or(|(best, _)| dist_sq <= *best) {
                closest = Some((dist_sq, candidate.clone()));
            }
        }

        if closest.is_none() {
            for candidate in &llamas {
                let Some(caravan) = Self::caravan_llama(candidate) else {
                    continue;
                };
                if !candidate.get_entity().is_leashed() || caravan.has_caravan_tail() {
                    continue;
                }
                let dist_sq = mob_pos.squared_distance_to_vec(&candidate.get_entity().pos.load());
                if closest.as_ref().is_none_or(|(best, _)| dist_sq <= *best) {
                    closest = Some((dist_sq, candidate.clone()));
                }
            }
        }

        let (dist_sq, closest) = closest?;
        if dist_sq < 4.0 {
            return None;
        }
        if !closest.get_entity().is_leashed() && !Self::first_is_leashed(&closest, 1, &world) {
            return None;
        }
        Some(closest)
    }
}

impl Goal for LlamaFollowCaravanGoal {
    fn can_start(&mut self, mob: &dyn Mob) -> bool {
        let Some(caravan) = Self::self_llama(mob) else {
            return false;
        };
        if mob.get_entity().is_leashed() || caravan.in_caravan() {
            return false;
        }
        let Some(head) = Self::find_caravan_head(mob) else {
            return false;
        };
        Self::join_caravan(mob, &head);
        true
    }

    fn should_continue(&mut self, mob: &dyn Mob) -> bool {
        let entity = mob.get_entity();
        let world = entity.world.load();
        let Some(caravan) = Self::self_llama(mob) else {
            return false;
        };
        let Some(head_id) = caravan.get_caravan_head_id() else {
            return false;
        };
        let Some(head) = world.get_entity_by_id(head_id) else {
            return false;
        };
        let Some(self_entity) = world.get_entity_by_id(entity.entity_id) else {
            return false;
        };
        if !head.get_entity().is_alive() || !Self::first_is_leashed(&self_entity, 0, &world) {
            return false;
        }

        let dist_sq = entity
            .pos
            .load()
            .squared_distance_to_vec(&head.get_entity().pos.load());
        if dist_sq > 676.0 {
            if self.speed_modifier <= 3.0 {
                self.speed_modifier *= 1.2;
                self.dist_check_counter = super::to_goal_ticks(40);
                return true;
            }
            if self.dist_check_counter == 0 {
                return false;
            }
        }
        if self.dist_check_counter > 0 {
            self.dist_check_counter -= 1;
        }
        true
    }

    fn stop(&mut self, mob: &dyn Mob) {
        Self::leave_caravan(mob);
        self.speed_modifier = 2.1;
    }

    fn tick(&mut self, mob: &dyn Mob) {
        let Some(caravan) = Self::self_llama(mob) else {
            return;
        };
        let Some(head_id) = caravan.get_caravan_head_id() else {
            return;
        };
        let entity = mob.get_entity();
        let world = entity.world.load();
        let Some(head) = world.get_entity_by_id(head_id) else {
            return;
        };
        let leash_holder = head
            .get_entity()
            .leashed_to
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .clone();
        if leash_holder
            .as_ref()
            .is_some_and(|holder| holder.cast_any().is::<LeashKnotEntity>())
        {
            return;
        }

        let self_pos = entity.pos.load();
        let head_pos = head.get_entity().pos.load();
        let delta = head_pos - self_pos;
        let distance = delta.length();
        let target = self_pos + delta.normalize() * (distance - 2.0).max(0.0);

        mob.get_mob_entity()
            .navigator
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .set_progress(NavigatorGoal::new(self_pos, target, self.speed_modifier));
    }

    fn controls(&self) -> Controls {
        Controls::MOVE
    }
}
