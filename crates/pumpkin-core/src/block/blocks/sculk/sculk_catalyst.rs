use std::sync::Arc;

use crate::block::{BlockBehaviour, BlockMetadata, OnPlaceArgs, OnScheduledTickArgs};
use crate::world::World;
use pumpkin_data::particle::Particle;
use pumpkin_data::sound::{Sound, SoundCategory};
use pumpkin_data::{Block, BlockId, BlockStateId, block_properties::SculkCatalystLikeProperties};
use pumpkin_util::math::position::BlockPos;
use pumpkin_util::math::vector3::Vector3;
use pumpkin_world::tick::TickPriority;
use pumpkin_world::world::BlockFlags;
use rand::RngExt;

pub struct SculkCatalystBlock;

impl BlockMetadata for SculkCatalystBlock {
    fn ids() -> Box<[BlockId]> {
        [BlockId::SCULK_CATALYST].into()
    }
}

impl SculkCatalystBlock {
    /// Vanilla `SculkCatalystBlockEntity.CatalystListener.PULSE_TICKS`.
    pub const PULSE_TICKS: u16 = 8;

    /// Vanilla `SculkCatalystBlockEntity.CatalystListener.bloom`: pulse the
    /// block, emit the soul particles and play the bloom sound.
    pub fn bloom(world: &Arc<World>, position: &BlockPos) {
        let block = &Block::SCULK_CATALYST;
        let state = world.get_block_state(position);
        let mut props = SculkCatalystLikeProperties::from_state_id(state.id);
        props.bloom = true;
        world.set_block_state(position, props.to_state_id(block), BlockFlags::NOTIFY_ALL);
        world.schedule_block_tick(block, *position, Self::PULSE_TICKS, TickPriority::Normal);

        world.spawn_particle(
            position
                .to_centered_f64()
                .add(&Vector3::new(0.0, 0.65, 0.0)),
            Vector3::new(0.2, 0.0, 0.2),
            0.0,
            2,
            Particle::SculkSoul,
        );
        world.play_sound_fine(
            Sound::BlockSculkCatalystBloom,
            SoundCategory::Blocks,
            &position.to_centered_f64(),
            2.0,
            0.6 + rand::rng().random::<f32>() * 0.4,
        );
    }
}

impl BlockBehaviour for SculkCatalystBlock {
    fn on_place(&self, args: OnPlaceArgs<'_>) -> BlockStateId {
        let mut props = SculkCatalystLikeProperties::default(args.block);
        props.bloom = false;
        props.to_state_id(args.block)
    }

    fn on_scheduled_tick(&self, args: OnScheduledTickArgs<'_>) {
        let state = args.world.get_block_state(args.position);
        let mut props = SculkCatalystLikeProperties::from_state_id(state.id);
        if props.bloom {
            props.bloom = false;
            args.world.set_block_state(
                args.position,
                props.to_state_id(args.block),
                BlockFlags::NOTIFY_ALL,
            );
        }
    }
}
