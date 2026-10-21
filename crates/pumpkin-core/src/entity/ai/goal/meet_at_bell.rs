use pumpkin_data::Block;
use pumpkin_data::environment_attribute::Activity;
use pumpkin_util::math::position::BlockPos;
use pumpkin_util::math::vector2::Vector2;

use super::{Controls, Goal};
use crate::block::entities::bell::BellBlockEntity;
use crate::entity::ai::pathfinder::NavigatorGoal;
use crate::entity::mob::Mob;
use crate::world::World;
use crate::world::villager_poi::MEETING_POINT_POI_TYPE;

const MEETING_POINT_SEARCH_RADIUS: i32 = 48;
const MEETING_POINT_SEARCH_CHUNK_RADIUS: i32 = MEETING_POINT_SEARCH_RADIUS / 16 + 1;
const CLOSE_ENOUGH_TO_MEETING_POINT: f64 = 6.0;
const SOCIALIZE_DISTANCE: f64 = 8.0;

/// Vanilla villager `Activity.MEET` package: gather at the nearest bell.
pub struct MeetAtBellGoal {
    speed: f64,
    target: Option<BlockPos>,
    meeting_point: Option<BlockPos>,
    search_cooldown: i32,
}

impl MeetAtBellGoal {
    #[must_use]
    pub const fn new(speed: f64) -> Self {
        Self {
            speed,
            target: None,
            meeting_point: None,
            search_cooldown: 0,
        }
    }

    fn find_meeting_point(&mut self, mob: &dyn Mob) -> Option<BlockPos> {
        let entity = mob.get_entity();
        let world = entity.world.load_full();
        let origin = entity.block_pos.load();
        if let Some(meeting_point) = self.meeting_point
            && origin
                .to_centered_f64()
                .squared_distance_to_vec(&meeting_point.to_centered_f64())
                <= f64::from(MEETING_POINT_SEARCH_RADIUS).powi(2)
            && world.get_block_and_state(&meeting_point).0 == &Block::BELL
        {
            return Some(meeting_point);
        }

        let registered = {
            let mut poi_storage = world
                .portal_poi
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            poi_storage
                .find_closest_matching(origin, MEETING_POINT_SEARCH_RADIUS, |poi_type| {
                    poi_type == MEETING_POINT_POI_TYPE
                })
                .map(|(position, _)| position)
        };
        let found = registered
            .filter(|position| world.get_block_and_state(position).0 == &Block::BELL)
            .or_else(|| Self::find_bell_block_entity(&world, origin));
        self.meeting_point = found;
        found
    }

    /// Bells placed by world generation never pass through `set_block_state`, so they are
    /// not in the POI index; their loaded block entities are the reliable source.
    fn find_bell_block_entity(world: &World, origin: BlockPos) -> Option<BlockPos> {
        let chunk = origin.chunk_position();
        let mut candidates = Vec::new();
        for chunk_x in (chunk.x - MEETING_POINT_SEARCH_CHUNK_RADIUS)
            ..=(chunk.x + MEETING_POINT_SEARCH_CHUNK_RADIUS)
        {
            for chunk_z in (chunk.y - MEETING_POINT_SEARCH_CHUNK_RADIUS)
                ..=(chunk.y + MEETING_POINT_SEARCH_CHUNK_RADIUS)
            {
                let Some(block_entities) =
                    world.block_entities.get(&Vector2::new(chunk_x, chunk_z))
                else {
                    continue;
                };
                for (position, block_entity) in block_entities.iter() {
                    let delta = position.0 - origin.0;
                    if block_entity.resource_location() == BellBlockEntity::ID
                        && delta.x.abs() <= MEETING_POINT_SEARCH_RADIUS
                        && delta.z.abs() <= MEETING_POINT_SEARCH_RADIUS
                    {
                        candidates.push(*position);
                    }
                }
            }
        }

        let mut best: Option<(f64, BlockPos)> = None;
        for position in candidates {
            if world.get_block_and_state(&position).0 != &Block::BELL {
                continue;
            }
            let distance = position
                .to_centered_f64()
                .squared_distance_to_vec(&origin.to_centered_f64());
            if best
                .as_ref()
                .is_none_or(|(best_distance, _)| distance < *best_distance)
            {
                best = Some((distance, position));
            }
        }
        best.map(|(_, position)| position)
    }

    fn distance_to_target(mob: &dyn Mob, target: BlockPos) -> f64 {
        target
            .to_centered_f64()
            .squared_distance_to_vec(&mob.get_entity().pos.load())
    }

    fn socialize(mob: &dyn Mob, target: BlockPos) {
        if Self::distance_to_target(mob, target) >= 4.0f64.powi(2)
            || rand::random_range(0..100) != 0
        {
            return;
        }
        let entity = mob.get_entity();
        let Some(other) = entity
            .world
            .load()
            .get_nearby_entities(entity.pos.load(), SOCIALIZE_DISTANCE)
            .into_values()
            .find(|candidate| {
                candidate.get_entity().entity_uuid != entity.entity_uuid
                    && candidate.get_entity().entity_type
                        == &pumpkin_data::entity::EntityType::VILLAGER
                    && mob.has_line_of_sight(candidate.get_entity())
            })
        else {
            return;
        };
        mob.get_mob_entity()
            .look_control
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .look_at_entity(mob, &other);
    }
}

impl Goal for MeetAtBellGoal {
    fn can_start(&mut self, mob: &dyn Mob) -> bool {
        let entity = mob.get_entity();
        if entity.age.load(std::sync::atomic::Ordering::Relaxed) < 0 {
            return false;
        }
        if self.search_cooldown > 0 {
            self.search_cooldown -= 1;
            return false;
        }
        self.search_cooldown = 20;
        let world = entity.world.load();
        if world.villager_activity(&entity.block_pos.load(), false) != Activity::Meet {
            return false;
        }
        let Some(target) = self.find_meeting_point(mob) else {
            return false;
        };
        self.target = Some(target);
        true
    }

    fn should_continue(&mut self, mob: &dyn Mob) -> bool {
        let Some(target) = self.target else {
            return false;
        };
        let entity = mob.get_entity();
        let world = entity.world.load();
        world.villager_activity(&entity.block_pos.load(), false) == Activity::Meet
            && world.get_block_and_state(&target).0 == &Block::BELL
    }

    fn start(&mut self, mob: &dyn Mob) {
        let Some(target) = self.target else {
            return;
        };
        let entity = mob.get_entity();
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
        if Self::distance_to_target(mob, target) < CLOSE_ENOUGH_TO_MEETING_POINT.powi(2) {
            Self::socialize(mob, target);
            return;
        }
        let entity = mob.get_entity();
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
