use std::sync::Arc;

use crate::block::entities::calibrated_sculk_sensor::CalibratedSculkSensorBlockEntity;
use crate::block::entities::sculk_sensor::SculkSensorBlockEntity;
use crate::block::{
    BlockBehaviour, BlockMetadata, EmitsRedstonePowerArgs, GetComparatorOutputArgs,
    GetRedstonePowerArgs, OnEntityStepArgs, OnPlaceArgs, OnScheduledTickArgs, PathComputationType,
    PlacedArgs,
};
use crate::entity::EntityBase;
use crate::world::World;
use crate::world::game_event::{
    CALIBRATED_SCULK_SENSOR_LISTENER_RADIUS, SCULK_SENSOR_LISTENER_RADIUS,
    distance_between_in_blocks, get_redstone_strength_for_distance,
};
use pumpkin_data::block_properties::{
    CalibratedSculkSensorLikeProperties, HorizontalFacing, SculkSensorLikeProperties,
    SculkSensorPhase,
};
use pumpkin_data::entity::EntityType;
use pumpkin_data::game_event::GameEvent;
use pumpkin_data::sound::{Sound, SoundCategory};
use pumpkin_data::{Block, BlockDirection, BlockId, BlockState, BlockStateId};
use pumpkin_util::math::position::BlockPos;
use pumpkin_world::tick::TickPriority;
use pumpkin_world::world::BlockFlags;

pub struct SculkSensorBlock;

impl BlockMetadata for SculkSensorBlock {
    fn ids() -> Box<[BlockId]> {
        [BlockId::SCULK_SENSOR, BlockId::CALIBRATED_SCULK_SENSOR].into()
    }
}

const fn horizontal_facing_to_dir(facing: HorizontalFacing) -> BlockDirection {
    match facing {
        HorizontalFacing::North => BlockDirection::North,
        HorizontalFacing::South => BlockDirection::South,
        HorizontalFacing::West => BlockDirection::West,
        HorizontalFacing::East => BlockDirection::East,
    }
}

/// Both sensor variants carry the same phase property under different types.
fn sculk_sensor_phase(block: &Block, state_id: BlockStateId) -> SculkSensorPhase {
    if block.id == BlockId::CALIBRATED_SCULK_SENSOR {
        CalibratedSculkSensorLikeProperties::from_state_id(state_id).sculk_sensor_phase
    } else {
        SculkSensorLikeProperties::from_state_id(state_id).sculk_sensor_phase
    }
}

impl SculkSensorBlock {
    /// `SculkSensorBlock.activate`.
    pub fn activate(
        world: &Arc<World>,
        pos: &BlockPos,
        block: &Block,
        power: u8,
        frequency: i32,
        source: Option<&dyn EntityBase>,
    ) {
        if block.id == BlockId::SCULK_SENSOR {
            let state = world.get_block_state(pos);
            let mut props = SculkSensorLikeProperties::from_state_id(state.id);
            if props.sculk_sensor_phase != SculkSensorPhase::Inactive {
                return;
            }
            if let Some(be) = world.get_block_entity(pos)
                && let Some(sensor_be) = be.as_any().downcast_ref::<SculkSensorBlockEntity>()
            {
                *sensor_be
                    .last_vibration_frequency
                    .lock()
                    .unwrap_or_else(std::sync::PoisonError::into_inner) = frequency;
            }

            let waterlogged = props.waterlogged;
            props.sculk_sensor_phase = SculkSensorPhase::Active;
            props.power = power;
            world.set_block_state(pos, props.to_state_id(block), BlockFlags::NOTIFY_ALL);
            world.schedule_block_tick(block, *pos, 30, TickPriority::Normal);
            update_neighbours(world, pos, block);
            world.emit_game_event_with_source(
                GameEvent::SculkSensorTendrilsClicking,
                pos.to_centered_f64(),
                source,
            );
            if !waterlogged {
                play_clicking_sound(world, pos);
            }
        } else if block.id == BlockId::CALIBRATED_SCULK_SENSOR {
            let state = world.get_block_state(pos);
            let mut props = CalibratedSculkSensorLikeProperties::from_state_id(state.id);
            if props.sculk_sensor_phase != SculkSensorPhase::Inactive {
                return;
            }
            let back_signal = get_back_signal(world, pos, props.facing);
            if back_signal != 0 && i32::from(back_signal) != frequency {
                return;
            }

            if let Some(be) = world.get_block_entity(pos)
                && let Some(cal_be) = be
                    .as_any()
                    .downcast_ref::<CalibratedSculkSensorBlockEntity>()
            {
                *cal_be
                    .last_vibration_frequency
                    .lock()
                    .unwrap_or_else(std::sync::PoisonError::into_inner) = frequency;
            }

            let waterlogged = props.waterlogged;
            props.sculk_sensor_phase = SculkSensorPhase::Active;
            props.power = power;
            world.set_block_state(pos, props.to_state_id(block), BlockFlags::NOTIFY_ALL);
            world.schedule_block_tick(block, *pos, 30, TickPriority::Normal);
            update_neighbours(world, pos, block);
            world.emit_game_event_with_source(
                GameEvent::SculkSensorTendrilsClicking,
                pos.to_centered_f64(),
                source,
            );
            if !waterlogged {
                play_clicking_sound(world, pos);
            }
        }
    }
}

fn update_neighbours(world: &Arc<World>, pos: &BlockPos, block: &Block) {
    world.update_neighbors(pos, None);
    world.update_neighbors_at(&pos.down(), block, None);
}

fn play_clicking_sound(world: &World, pos: &BlockPos) {
    let pitch = rand::random::<f32>().mul_add(0.2, 0.8);
    world.play_sound_fine(
        Sound::BlockSculkSensorClicking,
        SoundCategory::Blocks,
        &pos.to_centered_f64(),
        1.0,
        pitch,
    );
}

fn get_back_signal(world: &World, pos: &BlockPos, facing: HorizontalFacing) -> u8 {
    let back_dir = horizontal_facing_to_dir(facing).opposite();
    let back_pos = pos.offset(back_dir.to_offset());
    let back_state = world.get_block_state(&back_pos);
    let back_block = Block::from_state_id(back_state.id);
    world
        .block_registry
        .get_weak_redstone_power(back_block, world, &back_pos, back_state, back_dir)
}

impl BlockBehaviour for SculkSensorBlock {
    fn on_entity_step(&self, args: OnEntityStepArgs<'_>) {
        if args.entity.get_entity().entity_type == &EntityType::WARDEN {
            return;
        }
        let radius = if args.block.id == BlockId::CALIBRATED_SCULK_SENSOR {
            CALIBRATED_SCULK_SENSOR_LISTENER_RADIUS
        } else {
            SCULK_SENSOR_LISTENER_RADIUS
        };
        if sculk_sensor_phase(args.block, args.state.id) != SculkSensorPhase::Inactive {
            return;
        }

        let origin = BlockPos::containing_vec(args.entity.get_entity().pos.load());
        let distance = distance_between_in_blocks(&origin, args.position);
        let power = get_redstone_strength_for_distance(distance, radius);
        Self::activate(
            args.world,
            args.position,
            args.block,
            power,
            1,
            Some(args.entity),
        );
    }

    fn on_place(&self, args: OnPlaceArgs<'_>) -> BlockStateId {
        if args.block.id == BlockId::CALIBRATED_SCULK_SENSOR {
            let mut props = CalibratedSculkSensorLikeProperties::default(args.block);
            props.facing = args.player.living_entity.entity.get_horizontal_facing();
            props.to_state_id(args.block)
        } else {
            let props = SculkSensorLikeProperties::default(args.block);
            props.to_state_id(args.block)
        }
    }

    fn placed(&self, args: PlacedArgs<'_>) {
        if args.block.id == BlockId::CALIBRATED_SCULK_SENSOR {
            let entity = CalibratedSculkSensorBlockEntity::new(*args.position);
            args.world.add_block_entity(Arc::new(entity));
        } else if args.block.id == BlockId::SCULK_SENSOR {
            let entity = SculkSensorBlockEntity::new(*args.position);
            args.world.add_block_entity(Arc::new(entity));
        }
    }

    fn emits_redstone_power(&self, _args: EmitsRedstonePowerArgs<'_>) -> bool {
        true
    }

    fn get_weak_redstone_power(&self, args: GetRedstonePowerArgs<'_>) -> u8 {
        if args.block.id == BlockId::SCULK_SENSOR {
            let props = SculkSensorLikeProperties::from_state_id(args.state.id);
            if props.sculk_sensor_phase == SculkSensorPhase::Active {
                props.power
            } else {
                0
            }
        } else if args.block.id == BlockId::CALIBRATED_SCULK_SENSOR {
            let props = CalibratedSculkSensorLikeProperties::from_state_id(args.state.id);
            if props.sculk_sensor_phase == SculkSensorPhase::Active {
                props.power
            } else {
                0
            }
        } else {
            0
        }
    }

    fn get_comparator_output(&self, args: GetComparatorOutputArgs<'_>) -> Option<u8> {
        // Vanilla reads the frequency only while the sensor is active.
        if sculk_sensor_phase(args.block, args.state.id) != SculkSensorPhase::Active {
            return Some(0);
        }

        let be = args.world.get_block_entity(args.position)?;
        if let Some(sensor_be) = be.as_any().downcast_ref::<SculkSensorBlockEntity>() {
            return Some(
                *sensor_be
                    .last_vibration_frequency
                    .lock()
                    .unwrap_or_else(std::sync::PoisonError::into_inner) as u8,
            );
        }
        if let Some(cal_be) = be
            .as_any()
            .downcast_ref::<CalibratedSculkSensorBlockEntity>()
        {
            return Some(
                *cal_be
                    .last_vibration_frequency
                    .lock()
                    .unwrap_or_else(std::sync::PoisonError::into_inner) as u8,
            );
        }
        None
    }

    fn on_scheduled_tick(&self, args: OnScheduledTickArgs<'_>) {
        let state = args.world.get_block_state(args.position);
        if args.block.id == BlockId::SCULK_SENSOR {
            let mut props = SculkSensorLikeProperties::from_state_id(state.id);
            match props.sculk_sensor_phase {
                SculkSensorPhase::Active => {
                    props.sculk_sensor_phase = SculkSensorPhase::Cooldown;
                    props.power = 0;
                    args.world.set_block_state(
                        args.position,
                        props.to_state_id(args.block),
                        BlockFlags::NOTIFY_ALL,
                    );
                    args.world.schedule_block_tick(
                        args.block,
                        *args.position,
                        10,
                        TickPriority::Normal,
                    );
                }
                SculkSensorPhase::Cooldown => {
                    props.sculk_sensor_phase = SculkSensorPhase::Inactive;
                    props.power = 0;
                    args.world.set_block_state(
                        args.position,
                        props.to_state_id(args.block),
                        BlockFlags::NOTIFY_ALL,
                    );
                }
                SculkSensorPhase::Inactive => {}
            }
        } else if args.block.id == BlockId::CALIBRATED_SCULK_SENSOR {
            let mut props = CalibratedSculkSensorLikeProperties::from_state_id(state.id);
            match props.sculk_sensor_phase {
                SculkSensorPhase::Active => {
                    props.sculk_sensor_phase = SculkSensorPhase::Cooldown;
                    props.power = 0;
                    args.world.set_block_state(
                        args.position,
                        props.to_state_id(args.block),
                        BlockFlags::NOTIFY_ALL,
                    );
                    args.world.schedule_block_tick(
                        args.block,
                        *args.position,
                        10,
                        TickPriority::Normal,
                    );
                }
                SculkSensorPhase::Cooldown => {
                    props.sculk_sensor_phase = SculkSensorPhase::Inactive;
                    props.power = 0;
                    args.world.set_block_state(
                        args.position,
                        props.to_state_id(args.block),
                        BlockFlags::NOTIFY_ALL,
                    );
                }
                SculkSensorPhase::Inactive => {}
            }
        }
    }

    fn is_pathfindable(&self, _state: &BlockState, _computation_type: PathComputationType) -> bool {
        false
    }
}
