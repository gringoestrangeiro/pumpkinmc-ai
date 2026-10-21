use std::sync::Arc;
use std::sync::atomic::Ordering;

use pumpkin_data::entity::{EntityStatus, EntityType};
use pumpkin_data::environment_attribute::Activity;
use pumpkin_protocol::bedrock::server::actor_event::ActorEventID;
use rand::RngExt;

use super::{Controls, Goal};
use crate::entity::EntityBase;
use crate::entity::ageable::BABY_START_AGE;
use crate::entity::ai::pathfinder::NavigatorGoal;
use crate::entity::mob::Mob;
use crate::entity::passive::villager::VillagerEntity;

const PARTNER_SEARCH_RADIUS: f64 = 8.0;
const BREED_DISTANCE_SQ: f64 = 5.0;
const BREED_COOLDOWN_TICKS: i32 = 6000;
const BIRTH_TIME_MIN: i32 = 275;
const BIRTH_TIME_MAX: i32 = 324;

/// Vanilla villager `VillagerMakeLove`: two fed adults raise a child after a short courtship.
#[derive(Default)]
pub struct VillagerMakeLoveGoal {
    partner: Option<Arc<dyn EntityBase>>,
    birth_timestamp: i64,
    search_cooldown: i32,
}

impl VillagerMakeLoveGoal {
    #[must_use]
    pub const fn new() -> Self {
        Self {
            partner: None,
            birth_timestamp: 0,
            search_cooldown: 0,
        }
    }

    fn find_partner(mob: &dyn Mob) -> Option<Arc<dyn EntityBase>> {
        let entity = mob.get_entity();
        let nearby = entity
            .world
            .load()
            .get_nearby_entities(entity.pos.load(), PARTNER_SEARCH_RADIUS);

        let mut closest: Option<(f64, Arc<dyn EntityBase>)> = None;
        for candidate in nearby.into_values() {
            if candidate.get_entity().entity_uuid == entity.entity_uuid
                || candidate.get_entity().entity_type != &EntityType::VILLAGER
                || !candidate.is_breeding_ready()
                || !mob.has_line_of_sight(candidate.get_entity())
            {
                continue;
            }
            let Some(other) = candidate.cast_any().downcast_ref::<VillagerEntity>() else {
                continue;
            };
            if !other.can_breed() {
                continue;
            }
            let distance = entity
                .pos
                .load()
                .squared_distance_to_vec(&candidate.get_entity().pos.load());
            if closest
                .as_ref()
                .is_none_or(|(best_distance, _)| distance < *best_distance)
            {
                closest = Some((distance, candidate));
            }
        }
        closest.map(|(_, partner)| partner)
    }

    fn as_villager(entity: &dyn EntityBase) -> Option<&VillagerEntity> {
        entity.cast_any().downcast_ref::<VillagerEntity>()
    }

    fn send_event(villager: &VillagerEntity, status: EntityStatus, bedrock: ActorEventID) {
        let world = villager.get_entity().world.load();
        world.send_entity_status(villager.get_entity(), status, Some(bedrock));
    }

    fn try_to_give_birth(source: &VillagerEntity, target: &VillagerEntity) {
        let Some(bed) = source.take_vacant_bed(48) else {
            Self::send_event(
                source,
                EntityStatus::VillagerAngry,
                ActorEventID::VillagerAngry,
            );
            Self::send_event(
                target,
                EntityStatus::VillagerAngry,
                ActorEventID::VillagerAngry,
            );
            return;
        };

        let world = source.get_entity().world.load_full();
        source
            .mob_entity
            .breeding_cooldown
            .store(BREED_COOLDOWN_TICKS, Ordering::Relaxed);
        target
            .mob_entity
            .breeding_cooldown
            .store(BREED_COOLDOWN_TICKS, Ordering::Relaxed);

        let child = source.get_breed_offspring(target);
        child.set_home_pos(Some(bed));
        child
            .get_entity()
            .age
            .store(BABY_START_AGE, Ordering::Relaxed);
        world.spawn_entity(child.clone() as Arc<dyn EntityBase>);
        world.send_entity_status(
            child.get_entity(),
            EntityStatus::LoveHearts,
            Some(ActorEventID::LoveHearts),
        );
    }
}

impl Goal for VillagerMakeLoveGoal {
    fn can_start(&mut self, mob: &dyn Mob) -> bool {
        let Some(villager) = mob.cast_any().downcast_ref::<VillagerEntity>() else {
            return false;
        };
        let entity = mob.get_entity();
        let world = entity.world.load();
        if world.villager_activity(&entity.block_pos.load(), false) != Activity::Idle {
            return false;
        }
        if self.search_cooldown > 0 {
            self.search_cooldown -= 1;
            return false;
        }
        self.search_cooldown = 20;
        if !villager.can_breed() {
            return false;
        }
        self.partner = Self::find_partner(mob);
        self.partner.is_some()
    }

    fn should_continue(&mut self, mob: &dyn Mob) -> bool {
        let Some(partner) = &self.partner else {
            return false;
        };
        if !partner.get_entity().is_alive() {
            return false;
        }
        let Some(other) = Self::as_villager(partner.as_ref()) else {
            return false;
        };
        let Some(villager) = mob.cast_any().downcast_ref::<VillagerEntity>() else {
            return false;
        };
        mob.get_entity().world.load().get_world_age() <= self.birth_timestamp
            && villager.can_breed()
            && other.can_breed()
    }

    fn start(&mut self, mob: &dyn Mob) {
        let Some(partner) = &self.partner else {
            return;
        };
        let Some(villager) = mob.cast_any().downcast_ref::<VillagerEntity>() else {
            return;
        };
        let Some(other) = Self::as_villager(partner.as_ref()) else {
            return;
        };
        let game_time = mob.get_entity().world.load().get_world_age();
        self.birth_timestamp = game_time
            + i64::from(
                mob.get_random()
                    .random_range(BIRTH_TIME_MIN..=BIRTH_TIME_MAX),
            );
        Self::send_event(
            villager,
            EntityStatus::InLoveHearts,
            ActorEventID::InLoveHearts,
        );
        Self::send_event(
            other,
            EntityStatus::InLoveHearts,
            ActorEventID::InLoveHearts,
        );
    }

    fn stop(&mut self, mob: &dyn Mob) {
        self.partner = None;
        self.birth_timestamp = 0;
        mob.get_mob_entity()
            .navigator
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .stop();
    }

    fn tick(&mut self, mob: &dyn Mob) {
        let Some(partner) = &self.partner else {
            return;
        };
        let Some(villager) = mob.cast_any().downcast_ref::<VillagerEntity>() else {
            return;
        };
        let Some(other) = Self::as_villager(partner.as_ref()) else {
            return;
        };

        let entity = mob.get_entity();
        let partner_pos = partner.get_entity().pos.load();
        let my_pos = entity.pos.load();

        mob.get_mob_entity()
            .look_control
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .look_at_entity(mob, partner);
        mob.get_mob_entity()
            .navigator
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .set_progress(NavigatorGoal::new(my_pos, partner_pos, 0.5));

        if my_pos.squared_distance_to_vec(&partner_pos) > BREED_DISTANCE_SQ {
            return;
        }

        let game_time = entity.world.load().get_world_age();
        if game_time >= self.birth_timestamp {
            villager.eat_and_digest_food();
            other.eat_and_digest_food();
            Self::try_to_give_birth(villager, other);
        } else if rand::random_range(0..35) == 0 {
            Self::send_event(villager, EntityStatus::LoveHearts, ActorEventID::LoveHearts);
            Self::send_event(other, EntityStatus::LoveHearts, ActorEventID::LoveHearts);
        };
    }

    fn controls(&self) -> Controls {
        Controls::MOVE | Controls::LOOK
    }
}
