use pumpkin_data::{BlockState, BlockStateId};
use pumpkin_nbt::compound::NbtCompound;
use pumpkin_util::math::boundingbox::BoundingBox;
use pumpkin_util::math::position::BlockPos;
use pumpkin_util::math::vector3::Vector3;
use pumpkin_world::generation::structure::template::{
    BlockCapture, BlockPlacer, StructureEntityInfo,
};
use pumpkin_world::level::Level;

use crate::world::World;

impl World {
    pub fn clear_synced_block_events_in_box(&self, min: &BlockPos, max: &BlockPos) {
        let mut events = self
            .synced_block_event_queue
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        events.retain(|event| {
            let pos = event.pos;
            pos.0.x < min.0.x
                || pos.0.x >= max.0.x
                || pos.0.y < min.0.y
                || pos.0.y >= max.0.y
                || pos.0.z < min.0.z
                || pos.0.z >= max.0.z
        });
    }
}

pub struct WorldBlockPlacer<'a> {
    world: &'a World,
    pub block_entity_nbts: Vec<NbtCompound>,
    pub changed_positions: Vec<(BlockPos, BlockStateId)>,
}

impl<'a> WorldBlockPlacer<'a> {
    #[must_use]
    pub const fn new(world: &'a World) -> Self {
        Self {
            world,
            block_entity_nbts: Vec::new(),
            changed_positions: Vec::new(),
        }
    }

    /// Removes block entities at positions that were overwritten without replacement NBT,
    /// the way a normal block update would. Call before [`Self::finalize`].
    pub fn remove_overwritten_block_entities(&self) {
        let replaced: std::collections::HashSet<BlockPos> = self
            .block_entity_nbts
            .iter()
            .filter_map(|nbt| {
                Some(BlockPos::new(
                    nbt.get_int("x")?,
                    nbt.get_int("y")?,
                    nbt.get_int("z")?,
                ))
            })
            .collect();

        for (position, _) in &self.changed_positions {
            if !replaced.contains(position) && self.world.get_block_entity(position).is_some() {
                self.world.remove_block_entity(position);
            }
        }
    }

    #[allow(clippy::unused_async)]
    pub fn finalize(&self) {
        for nbt in &self.block_entity_nbts {
            if let Some(block_entity) = crate::block::entities::block_entity_from_nbt(nbt) {
                self.world.add_block_entity(block_entity);
            }
        }
    }
}

impl BlockPlacer for WorldBlockPlacer<'_> {
    fn get_block_state(&self, pos: &Vector3<i32>) -> BlockStateId {
        self.world
            .get_block_state_id(&BlockPos::new(pos.x, pos.y, pos.z))
    }

    fn set_block_state(&mut self, pos: &Vector3<i32>, state: &BlockState) {
        let block_pos = BlockPos::new(pos.x, pos.y, pos.z);
        Level::set_block_state(&self.world.level, &block_pos, state.id);
        self.changed_positions.push((block_pos, state.id));
    }

    fn add_block_entity(&mut self, nbt: NbtCompound) {
        self.block_entity_nbts.push(nbt);
    }
}

impl BlockCapture for World {
    fn get_block_state(&self, pos: &Vector3<i32>) -> BlockStateId {
        self.get_block_state_id(&BlockPos::new(pos.x, pos.y, pos.z))
    }

    fn get_block_entity_nbt(&self, pos: &Vector3<i32>) -> Option<NbtCompound> {
        let block_pos = BlockPos::new(pos.x, pos.y, pos.z);
        let state_id = self.get_block_state_id_if_loaded(&block_pos)?;
        if state_id.to_state().block_entity_type == u16::MAX {
            return None;
        }

        let block_entity = self.get_block_entity(&block_pos)?;
        let mut nbt = NbtCompound::new();
        block_entity.write_internal(&mut nbt);
        Some(nbt)
    }

    fn capture_entities(&self, min: Vector3<i32>, max: Vector3<i32>) -> Vec<StructureEntityInfo> {
        let bounds = BoundingBox {
            min: Vector3::new(f64::from(min.x), f64::from(min.y), f64::from(min.z)),
            max: Vector3::new(
                f64::from(max.x) + 1.0,
                f64::from(max.y) + 1.0,
                f64::from(max.z) + 1.0,
            ),
        };
        let min_pos = Vector3::new(f64::from(min.x), f64::from(min.y), f64::from(min.z));

        self.get_entities_at_box(&bounds)
            .into_iter()
            .map(|entity| {
                let position = entity.get_entity().pos.load() - min_pos;
                let mut nbt = NbtCompound::new();
                entity.write_nbt(&mut nbt);
                StructureEntityInfo::new(position, BlockPos::containing_vec(position).0, nbt)
            })
            .collect()
    }
}
