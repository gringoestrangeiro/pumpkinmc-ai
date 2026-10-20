use super::FlatLayer;
use crate::chunk_system::StagedChunkEnum;
use crate::generation::Seed;
use crate::generation::positions::chunk_pos::{start_block_x, start_block_z};
use crate::generation::proto_chunk::ProtoChunk;
use crate::generation::structure::placement::GlobalStructureCache;
use pumpkin_data::Block;
use pumpkin_data::chunk::Biome;
use pumpkin_data::dimension::Dimension;
use pumpkin_data::structures::{
    StructurePlacementCalculator, StructurePlacementType, StructureSet,
};
use rustc_hash::FxHashSet;

pub struct FlatGenerator {
    pub seed: u64,
    pub dimension: Dimension,
    pub layers: Vec<FlatLayer>,
    pub biome: String,
    pub structure_calculator: StructurePlacementCalculator,
    pub enabled_structure_sets: Option<FxHashSet<usize>>,
    pub global_structure_cache: GlobalStructureCache,
}

impl FlatGenerator {
    #[must_use]
    pub fn new(seed: Seed, dimension: Dimension, layers: Vec<FlatLayer>, biome: String) -> Self {
        Self::new_with_structures(seed, dimension, layers, biome, None)
    }

    /// Creates a flat generator with the world's `structure_overrides`, which
    /// decide the structure sets a flat world may place. Vanilla's flat
    /// generator builds its structure state from seed 0 for the stronghold
    /// rings (`ChunkGeneratorStructureState.createForFlat`).
    #[must_use]
    pub fn new_with_structures(
        seed: Seed,
        dimension: Dimension,
        layers: Vec<FlatLayer>,
        biome: String,
        structure_overrides: Option<&[String]>,
    ) -> Self {
        let biome_id = flat_biome_id(&biome);
        let enabled_structure_sets = super::parse_enabled_structure_sets(structure_overrides);
        let strongholds_enabled = enabled_structure_sets
            .as_ref()
            .is_none_or(|enabled| enabled.contains(&strongholds_index()));
        let global_structure_cache = match &StructureSet::STRONGHOLDS.placement.placement_type {
            StructurePlacementType::ConcentricRings(rings)
                if strongholds_enabled
                    && dimension == Dimension::OVERWORLD
                    && ProtoChunk::get_allowed_biomes(&StructureSet::STRONGHOLDS)
                        .contains(&u16::from(biome_id)) =>
            {
                GlobalStructureCache::with_background_fixed_biome_strongholds(rings, biome_id)
            }
            _ => GlobalStructureCache::new(),
        };
        Self {
            seed: seed.0,
            dimension,
            layers,
            biome,
            structure_calculator: StructurePlacementCalculator::new(seed.0 as i64),
            enabled_structure_sets,
            global_structure_cache,
        }
    }

    #[must_use]
    pub fn biome(&self) -> &'static Biome {
        Biome::from_name(self.biome.strip_prefix("minecraft:").unwrap_or(&self.biome))
            .unwrap_or(&Biome::PLAINS)
    }

    pub fn step_to_biomes(&self, chunk: &mut ProtoChunk) {
        chunk.flat_biome_map.fill(self.biome().id);
        chunk.stage = StagedChunkEnum::Biomes;
    }

    pub fn step_to_noise(&self, chunk: &mut ProtoChunk) {
        let start_x = start_block_x(chunk.x);
        let start_z = start_block_z(chunk.z);
        for x in 0..16 {
            for z in 0..16 {
                let mut current_y = chunk.bottom_y() as i32;
                for layer in &self.layers {
                    let block = Block::from_name(&layer.block);
                    let state = block.map_or(Block::AIR.default_state, |b| b.default_state);
                    for _ in 0..layer.height {
                        if current_y < chunk.bottom_y() as i32 + chunk.height() as i32 {
                            chunk.set_block_state(start_x + x, current_y, start_z + z, state);
                            current_y += 1;
                        } else {
                            break;
                        }
                    }
                }
            }
        }
        chunk.stage = StagedChunkEnum::Noise;
    }

    pub const fn step_to_surface(&self, chunk: &mut ProtoChunk) {
        chunk.stage = StagedChunkEnum::Surface;
    }

    pub const fn step_to_carvers(&self, chunk: &mut ProtoChunk) {
        chunk.snapshot_wg_heightmaps();
        chunk.stage = StagedChunkEnum::Carvers;
    }
}

fn flat_biome_id(name: &str) -> u8 {
    Biome::from_name(name.strip_prefix("minecraft:").unwrap_or(name))
        .unwrap_or(&Biome::PLAINS)
        .id
}

fn strongholds_index() -> usize {
    StructureSet::NAMES
        .iter()
        .position(|&name| name == "strongholds")
        .unwrap_or(usize::MAX)
}
