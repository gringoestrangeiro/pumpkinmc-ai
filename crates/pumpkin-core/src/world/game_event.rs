use std::sync::Arc;

use pumpkin_data::BlockId;
use pumpkin_data::block_properties::{
    CalibratedSculkSensorLikeProperties, SculkSensorLikeProperties, SculkSensorPhase,
    SculkShriekerLikeProperties,
};
use pumpkin_data::entity::EntityType;
use pumpkin_data::game_event::GameEvent;
use pumpkin_data::game_event_data::NO_VIBRATION_FREQUENCY;
use pumpkin_data::tag::{GameEvent as GameEventTags, Taggable};
use pumpkin_util::math::position::BlockPos;
use pumpkin_util::math::vector2::Vector2;
use pumpkin_util::math::vector3::Vector3;

use crate::block::blocks::redstone::sculk_sensor::SculkSensorBlock;
use crate::block::blocks::sculk::sculk_shrieker::SculkShriekerBlock;
use crate::block::entities::BlockEntity;
use crate::block::entities::calibrated_sculk_sensor::CalibratedSculkSensorBlockEntity;
use crate::block::entities::sculk_sensor::SculkSensorBlockEntity;
use crate::block::entities::sculk_shrieker::SculkShriekerBlockEntity;
use crate::entity::EntityBase;
use crate::entity::mob::warden::WardenEntity;
use crate::world::World;

pub const SCULK_SENSOR_LISTENER_RADIUS: i32 = 8;
pub const CALIBRATED_SCULK_SENSOR_LISTENER_RADIUS: i32 = 16;
const SCULK_SHRIEKER_LISTENER_RADIUS: i32 = 8;
const WARDEN_LISTENER_RADIUS: i32 = 16;

pub fn dispatch(
    world: &Arc<World>,
    event: GameEvent,
    position: Vector3<f64>,
    source: Option<&dyn EntityBase>,
) {
    if !is_valid_vibration(event, source) {
        return;
    }

    let origin = BlockPos::containing_vec(position);
    let candidates = loaded_block_entities_in_radius(world, &origin, event.notification_radius());
    for block_entity in candidates {
        if let Some(sensor) = block_entity
            .as_any()
            .downcast_ref::<SculkSensorBlockEntity>()
        {
            receive_sensor_vibration(
                world,
                event,
                &origin,
                &sensor.position,
                SCULK_SENSOR_LISTENER_RADIUS,
                source,
            );
        } else if let Some(sensor) = block_entity
            .as_any()
            .downcast_ref::<CalibratedSculkSensorBlockEntity>()
        {
            receive_sensor_vibration(
                world,
                event,
                &origin,
                &sensor.position,
                CALIBRATED_SCULK_SENSOR_LISTENER_RADIUS,
                source,
            );
        } else if let Some(shrieker) = block_entity
            .as_any()
            .downcast_ref::<SculkShriekerBlockEntity>()
        {
            receive_shrieker_vibration(world, event, &origin, &shrieker.position, source);
        }
    }

    receive_warden_vibration(world, event, position, source);
}

fn loaded_block_entities_in_radius(
    world: &World,
    center: &BlockPos,
    radius: i32,
) -> Vec<Arc<dyn BlockEntity>> {
    let min = center.add(-radius, -radius, -radius).chunk_position();
    let max = center.add(radius, radius, radius).chunk_position();
    let mut entities = Vec::new();
    for chunk_x in min.x..=max.x {
        for chunk_z in min.y..=max.y {
            let chunk_pos = Vector2::new(chunk_x, chunk_z);
            if let Some(chunk) = world.block_entities.get(&chunk_pos) {
                entities.extend(chunk.values().cloned());
            }
        }
    }
    entities
}

fn receive_sensor_vibration(
    world: &Arc<World>,
    event: GameEvent,
    origin: &BlockPos,
    sensor_pos: &BlockPos,
    listener_radius: i32,
    source: Option<&dyn EntityBase>,
) {
    if !event.has_tag(&GameEventTags::MINECRAFT_VIBRATIONS) {
        return;
    }
    let frequency = event.vibration_frequency();
    if frequency == NO_VIBRATION_FREQUENCY {
        return;
    }
    if matches!(event, GameEvent::BlockPlace | GameEvent::BlockDestroy) && origin == sensor_pos {
        return;
    }

    let distance = distance_between_in_blocks(origin, sensor_pos);
    if distance > listener_radius as f32 {
        return;
    }

    let block = world.get_block(sensor_pos);
    if block.id != BlockId::SCULK_SENSOR && block.id != BlockId::CALIBRATED_SCULK_SENSOR {
        return;
    }
    let state = world.get_block_state(sensor_pos);
    let phase = if block.id == BlockId::CALIBRATED_SCULK_SENSOR {
        CalibratedSculkSensorLikeProperties::from_state_id(state.id).sculk_sensor_phase
    } else {
        SculkSensorLikeProperties::from_state_id(state.id).sculk_sensor_phase
    };
    if phase != SculkSensorPhase::Inactive {
        return;
    }

    let power = get_redstone_strength_for_distance(distance, listener_radius);
    SculkSensorBlock::activate(world, sensor_pos, block, power, frequency, source);
}

fn receive_shrieker_vibration(
    world: &Arc<World>,
    event: GameEvent,
    origin: &BlockPos,
    shrieker_pos: &BlockPos,
    source: Option<&dyn EntityBase>,
) {
    if !event.has_tag(&GameEventTags::MINECRAFT_SHRIEKER_CAN_LISTEN) {
        return;
    }
    if distance_between_in_blocks(origin, shrieker_pos) > SCULK_SHRIEKER_LISTENER_RADIUS as f32 {
        return;
    }

    let state = world.get_block_state(shrieker_pos);
    let props = SculkShriekerLikeProperties::from_state_id(state.id);
    if props.shrieking {
        return;
    }
    let Some(player) = source.and_then(|source| SculkShriekerBlock::try_get_player(world, source))
    else {
        return;
    };
    SculkShriekerBlock::try_shriek(world, shrieker_pos, &player);
}

fn receive_warden_vibration(
    world: &Arc<World>,
    event: GameEvent,
    position: Vector3<f64>,
    source: Option<&dyn EntityBase>,
) {
    if !event.has_tag(&GameEventTags::MINECRAFT_WARDEN_CAN_LISTEN) {
        return;
    }

    for entity in world
        .get_nearby_entities(position, f64::from(event.notification_radius()))
        .values()
    {
        if entity.get_entity().entity_type != &EntityType::WARDEN {
            continue;
        }
        let Some(warden) = entity.cast_any().downcast_ref::<WardenEntity>() else {
            continue;
        };
        let warden_pos = entity.get_entity().block_pos.load();
        if distance_between_in_blocks(&BlockPos::containing_vec(position), &warden_pos)
            > WARDEN_LISTENER_RADIUS as f32
        {
            continue;
        }
        warden.hear_vibration(world, source);
    }
}

fn is_valid_vibration(event: GameEvent, source: Option<&dyn EntityBase>) -> bool {
    let Some(source) = source else {
        return true;
    };
    if source.is_spectator() {
        return false;
    }
    !(event.has_tag(&GameEventTags::MINECRAFT_IGNORE_VIBRATIONS_SNEAKING)
        && source.get_entity().is_sneaking())
}

pub fn distance_between_in_blocks(origin: &BlockPos, destination: &BlockPos) -> f32 {
    f64::from(origin.squared_distance(destination)).sqrt() as f32
}

/// `VibrationSystem.getRedstoneStrengthForDistance`.
pub fn get_redstone_strength_for_distance(distance: f32, listener_radius: i32) -> u8 {
    let power_scale = 15.0 / f64::from(listener_radius);
    (15 - (power_scale * f64::from(distance)).floor() as i32).max(1) as u8
}

#[cfg(test)]
mod tests {
    use super::get_redstone_strength_for_distance;

    #[test]
    fn redstone_strength_matches_the_vanilla_distance_curve() {
        assert_eq!(get_redstone_strength_for_distance(0.0, 8), 15);
        assert_eq!(get_redstone_strength_for_distance(4.0, 8), 8);
        assert_eq!(get_redstone_strength_for_distance(8.0, 8), 1);
        assert_eq!(get_redstone_strength_for_distance(0.0, 16), 15);
        assert_eq!(get_redstone_strength_for_distance(8.0, 16), 8);
        assert_eq!(get_redstone_strength_for_distance(16.0, 16), 1);
    }
}
