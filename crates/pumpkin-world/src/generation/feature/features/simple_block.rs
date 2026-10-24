use pumpkin_data::block_properties::DoubleBlockHalf;
use pumpkin_data::fluid::Fluid;
use pumpkin_data::{Block, BlockState, BlockStateId, Property};
use pumpkin_util::{math::position::BlockPos, random::RandomGenerator};

use crate::generation::proto_chunk::GenerationCache;
use crate::{
    generation::block_state_provider::BlockStateProvider,
    world::{BlockAccessor, WorldPortalExt, fluid_from_block_state, fluid_state_from_block_state},
};

/// Blocks whose vanilla class extends `DoublePlantBlock`.
const DOUBLE_PLANT_BLOCKS: [&Block; 10] = [
    &Block::TALL_SEAGRASS,
    &Block::SUNFLOWER,
    &Block::LILAC,
    &Block::ROSE_BUSH,
    &Block::PEONY,
    &Block::TALL_GRASS,
    &Block::LARGE_FERN,
    &Block::PITCHER_CROP,
    &Block::PITCHER_PLANT,
    &Block::SMALL_DRIPLEAF,
];

/// Vanilla `SimpleBlockFeature.scheduleTick` delay.
const SCHEDULED_TICK_DELAY: u16 = 1;

pub struct SimpleBlockFeature {
    pub to_place: BlockStateProvider,
    pub schedule_tick: Option<bool>,
}

impl SimpleBlockFeature {
    pub fn generate<T: GenerationCache>(
        &self,
        block_registry: &dyn WorldPortalExt,
        chunk: &mut T,
        random: &mut RandomGenerator,
        pos: BlockPos,
    ) -> bool {
        let state = self.to_place.get(random, pos, chunk, block_registry);
        let block = Block::from_state_id(state.id);
        let block_accessor: &dyn BlockAccessor = chunk;
        if !block_registry.can_place_at(block, state, block_accessor, &pos) {
            return false;
        }

        if DOUBLE_PLANT_BLOCKS.contains(&block) {
            let above_state = GenerationCache::get_block_state(chunk, &pos.up().0);
            if !above_state.to_state().is_air()
                && (!same_fluid_state(state.id, above_state)
                    || !above_state.to_state().replaceable())
            {
                return false;
            }
            place_double_plant(chunk, state, pos);
        } else {
            chunk.set_block_state(&pos.0, state);
        }

        if self.schedule_tick == Some(true) {
            let placed_block =
                Block::from_state_id(GenerationCache::get_block_state(chunk, &pos.0));
            chunk.schedule_block_tick(&pos.0, placed_block, SCHEDULED_TICK_DELAY);
        }
        true
    }
}

/// Vanilla `Objects.equals(FluidState, FluidState)` for the fluids held by two block states.
fn same_fluid_state(a: BlockStateId, b: BlockStateId) -> bool {
    let (fluid_a, state_a) = fluid_state_from_block_state(a);
    let (fluid_b, state_b) = fluid_state_from_block_state(b);
    fluid_a.id == fluid_b.id && state_a.level == state_b.level && state_a.falling == state_b.falling
}

/// Vanilla `DoublePlantBlock.placeAt`.
fn place_double_plant<T: GenerationCache>(
    chunk: &mut T,
    state: &'static BlockState,
    lower_pos: BlockPos,
) {
    let upper_pos = lower_pos.up();
    let lower = copy_waterlogged_from(chunk, lower_pos, state, DoubleBlockHalf::Lower);
    chunk.set_block_state(&lower_pos.0, lower);
    let upper = copy_waterlogged_from(chunk, upper_pos, state, DoubleBlockHalf::Upper);
    chunk.set_block_state(&upper_pos.0, upper);
}

/// Vanilla `DoublePlantBlock.copyWaterloggedFrom`, applied after setting `half`.
fn copy_waterlogged_from<T: GenerationCache>(
    chunk: &T,
    pos: BlockPos,
    state: &'static BlockState,
    half: DoubleBlockHalf,
) -> &'static BlockState {
    let block = Block::from_state_id(state.id);
    let Some(properties) = block.properties(state.id) else {
        return state;
    };
    let is_water_at = fluid_from_block_state(GenerationCache::get_block_state(chunk, &pos.0))
        .matches_type(&Fluid::WATER);
    let mut props = properties.to_props();
    for prop in &mut props {
        match prop {
            Property::DoubleBlockHalf(value) => *value = half,
            Property::Waterlogged(value) => *value = is_water_at,
            _ => {}
        }
    }
    BlockState::from_id(block.from_properties(&props).to_state_id(block))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::generation::block_state_provider::SimpleStateProvider;
    use crate::generation::generator::{GeneratorInit, VanillaGenerator, WorldGenerator};
    use crate::generation::proto_chunk::ProtoChunk;
    use crate::generation::proto_chunk_test::stage_parity::TestBlockRegistry;
    use pumpkin_data::block_properties::TallSeagrassLikeProperties;
    use pumpkin_data::dimension::Dimension;
    use pumpkin_util::random::legacy_rand::LegacyRand;
    use pumpkin_util::world_seed::Seed;

    #[test]
    fn double_plant_places_both_halves() {
        let world_gen = WorldGenerator::Noise(Box::new(VanillaGenerator::new(
            Seed(42),
            Dimension::OVERWORLD,
        )));
        let mut chunk = ProtoChunk::new(0, 0, &world_gen);
        chunk.set_block_state(0, 63, 0, Block::GRASS_BLOCK.default_state);
        let feature = SimpleBlockFeature {
            to_place: BlockStateProvider::Simple(SimpleStateProvider {
                state: Block::TALL_GRASS.default_state,
            }),
            schedule_tick: None,
        };
        let mut random = RandomGenerator::Legacy(LegacyRand::from_seed(42));

        assert!(feature.generate(
            &TestBlockRegistry,
            &mut chunk,
            &mut random,
            BlockPos::new(0, 64, 0)
        ));

        let half_at = |y| {
            let state = GenerationCache::get_block_state(&chunk, &BlockPos::new(0, y, 0).0);
            assert_eq!(state.to_block_id(), Block::TALL_GRASS.id);
            TallSeagrassLikeProperties::from_state_id(state).half
        };
        assert_eq!(half_at(64), DoubleBlockHalf::Lower);
        assert_eq!(half_at(65), DoubleBlockHalf::Upper);
    }
}
