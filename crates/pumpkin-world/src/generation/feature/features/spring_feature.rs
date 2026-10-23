use std::collections::HashSet;

use pumpkin_data::tag::{RegistryKey, get_tag_ids};
use pumpkin_data::{Block, BlockDirection, BlockId, BlockState};
use pumpkin_util::{math::position::BlockPos, random::RandomGenerator};

use crate::generation::proto_chunk::GenerationCache;
use crate::world::{WorldPortalExt, fluid_state_from_block_state};

pub struct SpringFeatureFeature {
    pub state: &'static BlockState,
    pub requires_block_below: bool,
    pub rock_count: i32,
    pub hole_count: i32,
    pub valid_blocks: BlockWrapper,
}

pub enum BlockWrapper {
    Single(String),
    Multi(Vec<String>),
}

impl BlockWrapper {
    /// Resolves the entries to block ids, expanding `#tag` references.
    #[must_use]
    pub fn block_ids(&self) -> HashSet<BlockId> {
        let names = match self {
            Self::Single(name) => std::slice::from_ref(name),
            Self::Multi(names) => names.as_slice(),
        };
        let mut ids = HashSet::new();
        for name in names {
            if let Some(tag) = name.strip_prefix('#') {
                let tag_ids = get_tag_ids(RegistryKey::Block, tag).unwrap_or_default();
                ids.extend(tag_ids.iter().filter_map(|&id| BlockId::new(id)));
            } else if let Some(block) = Block::from_name(name) {
                ids.insert(block.id);
            }
        }
        ids
    }

    fn contains(&self, name: &str) -> bool {
        let name_stripped = name.strip_prefix("minecraft:").unwrap_or(name);
        match self {
            Self::Single(s) => s.strip_prefix("minecraft:").unwrap_or(s) == name_stripped,
            Self::Multi(h) => h
                .iter()
                .any(|s| s.strip_prefix("minecraft:").unwrap_or(s) == name_stripped),
        }
    }
}

impl SpringFeatureFeature {
    pub fn generate<T: GenerationCache>(
        &self,
        _block_registry: &dyn WorldPortalExt,
        chunk: &mut T,
        _random: &mut RandomGenerator,
        pos: BlockPos,
    ) -> bool {
        let valid_blocks = &self.valid_blocks;

        if !valid_blocks.contains(
            GenerationCache::get_block_state(chunk, &pos.up().0)
                .to_block()
                .name,
        ) {
            return false;
        }
        if self.requires_block_below
            && !valid_blocks.contains(
                GenerationCache::get_block_state(
                    chunk,
                    &pos.offset(BlockDirection::Down.to_offset()).0,
                )
                .to_block()
                .name,
            )
        {
            return false;
        }
        let state = GenerationCache::get_block_state(chunk, &pos.0);
        if !state.to_state().is_air() && !valid_blocks.contains(state.to_block().name) {
            return false;
        }

        let mut valid = 0;
        if valid_blocks.contains(
            GenerationCache::get_block_state(
                chunk,
                &pos.offset(BlockDirection::West.to_offset()).0,
            )
            .to_block()
            .name,
        ) {
            valid += 1;
        }
        if valid_blocks.contains(
            GenerationCache::get_block_state(
                chunk,
                &pos.offset(BlockDirection::East.to_offset()).0,
            )
            .to_block()
            .name,
        ) {
            valid += 1;
        }
        if valid_blocks.contains(
            GenerationCache::get_block_state(
                chunk,
                &pos.offset(BlockDirection::North.to_offset()).0,
            )
            .to_block()
            .name,
        ) {
            valid += 1;
        }
        if valid_blocks.contains(
            GenerationCache::get_block_state(
                chunk,
                &pos.offset(BlockDirection::South.to_offset()).0,
            )
            .to_block()
            .name,
        ) {
            valid += 1;
        }
        if valid_blocks.contains(
            GenerationCache::get_block_state(
                chunk,
                &pos.offset(BlockDirection::Down.to_offset()).0,
            )
            .to_block()
            .name,
        ) {
            valid += 1;
        }
        let mut air = 0;
        if chunk.is_air(&pos.offset(BlockDirection::West.to_offset()).0) {
            air += 1;
        }
        if chunk.is_air(&pos.offset(BlockDirection::East.to_offset()).0) {
            air += 1;
        }
        if chunk.is_air(&pos.offset(BlockDirection::North.to_offset()).0) {
            air += 1;
        }
        if chunk.is_air(&pos.offset(BlockDirection::South.to_offset()).0) {
            air += 1;
        }
        if chunk.is_air(&pos.offset(BlockDirection::Down.to_offset()).0) {
            air += 1;
        }
        if valid == self.rock_count && air == self.hole_count {
            chunk.set_block_state(&pos.0, self.state);
            let (fluid, _) = fluid_state_from_block_state(self.state.id);
            chunk.schedule_fluid_tick(&pos.0, fluid);
            return true;
        }
        false
    }
}
