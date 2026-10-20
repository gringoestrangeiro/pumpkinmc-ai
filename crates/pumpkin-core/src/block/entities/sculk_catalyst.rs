use super::BlockEntity;
use std::sync::{Arc, Mutex};

use pumpkin_data::fluid::Fluid;
use pumpkin_data::sound::{Sound, SoundCategory};
use pumpkin_data::world::WorldEvent;
use pumpkin_data::{BlockDirection, BlockState, BlockStateId};
use pumpkin_nbt::compound::NbtCompound;
use pumpkin_nbt::tag::NbtTag;
use pumpkin_util::math::position::BlockPos;
use pumpkin_util::random::{RandomGenerator, get_seed, xoroshiro128::Xoroshiro};
use pumpkin_world::generation::feature::features::sculk::SculkLevel;
use pumpkin_world::generation::feature::features::sculk::spreader::{ChargeCursor, SculkSpreader};
use pumpkin_world::world::BlockFlags;

use crate::world::World;

pub struct SculkCatalystBlockEntity {
    pub position: BlockPos,
    /// Vanilla `SculkCatalystBlockEntity.CatalystListener.sculkSpreader`.
    pub spreader: Mutex<SculkSpreader>,
}

impl BlockEntity for SculkCatalystBlockEntity {
    fn resource_location(&self) -> &'static str {
        Self::ID
    }

    fn get_position(&self) -> BlockPos {
        self.position
    }

    fn from_nbt(nbt: &pumpkin_nbt::compound::NbtCompound, position: BlockPos) -> Self
    where
        Self: Sized,
    {
        let spreader = spreader_from_nbt(nbt);
        Self {
            position,
            spreader: Mutex::new(spreader),
        }
    }

    fn write_nbt(&self, nbt: &mut NbtCompound) {
        if let Ok(spreader) = self.spreader.lock() {
            write_cursors(&spreader, nbt);
        }
    }

    fn chunk_data_nbt(&self) -> Option<NbtCompound> {
        let mut nbt = NbtCompound::new();
        write_cursors(&self.spreader.try_lock().ok()?, &mut nbt);
        Some(nbt)
    }

    /// Vanilla `SculkCatalystBlockEntity.serverTick`: advance the cursors
    /// every tick while the block entity is loaded.
    fn tick(&self, world: &Arc<World>) {
        let Ok(mut spreader) = self.spreader.lock() else {
            return;
        };
        let mut level = WorldSculkLevel {
            world: world.clone(),
        };
        let mut random = RandomGenerator::Xoroshiro(Xoroshiro::from_seed(get_seed()));
        spreader.update_cursors(&mut level, self.position, &mut random, true);
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}

impl SculkCatalystBlockEntity {
    pub const ID: &'static str = "minecraft:sculk_catalyst";
    #[must_use]
    pub fn new(position: BlockPos) -> Self {
        Self {
            position,
            spreader: Mutex::new(SculkSpreader::new_level_spreader()),
        }
    }

    /// Vanilla `SculkSpreader.addCursors`.
    pub fn add_cursors(&self, position: BlockPos, charge: i32) {
        if let Ok(mut spreader) = self.spreader.lock() {
            spreader.add_cursors(position, charge);
        }
    }
}

/// Live-level [`SculkLevel`] adapter. Reads and writes go straight to the
/// world instead of a generating chunk.
struct WorldSculkLevel {
    world: Arc<World>,
}

impl SculkLevel for WorldSculkLevel {
    fn sculk_get(&self, pos: BlockPos) -> Option<BlockStateId> {
        self.world.get_block_state_id_if_loaded(&pos)
    }

    fn sculk_set(&mut self, pos: BlockPos, state: &'static BlockState) {
        self.world
            .set_block_state(&pos, state.id, BlockFlags::NOTIFY_ALL);
    }

    fn sculk_is_air(&self, pos: BlockPos) -> bool {
        self.world.get_block_state(&pos).is_air()
    }

    fn sculk_is_water_source(&self, pos: BlockPos) -> bool {
        let (fluid, state) = self.world.get_fluid_and_fluid_state(&pos);
        fluid == &Fluid::WATER && state.is_source
    }

    fn sculk_is_water(&self, pos: BlockPos) -> bool {
        self.world.get_fluid(&pos) == &Fluid::WATER
    }

    fn sculk_is_face_sturdy(&self, pos: BlockPos, face: BlockDirection) -> bool {
        self.world.get_block_state(&pos).is_side_solid(face)
    }

    fn sculk_is_full_cube(&self, pos: BlockPos) -> bool {
        self.world.get_block_state(&pos).is_full_cube()
    }

    fn sculk_on_sculk_placed(
        &mut self,
        pos: BlockPos,
        old_state: BlockStateId,
        new_state: &'static BlockState,
    ) {
        crate::block::push_entities_up(&self.world, &pos, old_state.to_state(), new_state);
        self.world.play_sound_fine(
            Sound::BlockSculkSpread,
            SoundCategory::Blocks,
            &pos.to_centered_f64(),
            1.0,
            1.0,
        );
    }

    fn sculk_on_vein_spread(&mut self, pos: BlockPos) {
        self.world.play_sound_fine(
            Sound::BlockSculkSpread,
            SoundCategory::Blocks,
            &pos.to_centered_f64(),
            1.0,
            1.0,
        );
    }

    fn sculk_level_event(&mut self, pos: BlockPos, data: i32) {
        self.world
            .sync_world_event(WorldEvent::ParticlesSculkCharge, pos, data);
    }

    fn sculk_should_update(&self, pos: BlockPos) -> bool {
        self.world.level.is_chunk_loaded(&pos.chunk_position())
    }
}

fn spreader_from_nbt(nbt: &NbtCompound) -> SculkSpreader {
    let mut spreader = SculkSpreader::new_level_spreader();
    if let Some(cursors) = nbt.get_list("cursors") {
        for tag in cursors.iter().take(32) {
            if let NbtTag::Compound(cursor) = tag
                && let Some(cursor) = cursor_from_nbt(cursor)
            {
                spreader.add_cursor(cursor);
            }
        }
    }
    spreader
}

fn cursor_from_nbt(nbt: &NbtCompound) -> Option<ChargeCursor> {
    let position = nbt.get_int_array("pos")?;
    if position.len() != 3 {
        return None;
    }
    let charge = nbt.get_int("charge").unwrap_or(0).clamp(0, 1000) as u16;
    let decay_delay = nbt.get_int("decay_delay").unwrap_or(1).clamp(0, 1) as u8;
    let update_delay = u8::try_from(nbt.get_int("update_delay").unwrap_or(0).max(0)).unwrap_or(255);
    let faces = nbt.get_list("facings").map(|facings| {
        facings
            .iter()
            .filter_map(|face| match face {
                NbtTag::String(name) => direction_from_name(name),
                _ => None,
            })
            .fold(0, |bits, direction| bits | (1 << direction.to_index()))
    });
    Some(ChargeCursor {
        pos: BlockPos::new(position[0], position[1], position[2]),
        charge,
        update_delay,
        decay_delay,
        faces,
    })
}

fn write_cursors(spreader: &SculkSpreader, nbt: &mut NbtCompound) {
    let mut cursors = Vec::with_capacity(spreader.cursors().len());
    for cursor in spreader.cursors() {
        let mut tag = NbtCompound::new();
        tag.put(
            "pos",
            NbtTag::IntArray(vec![cursor.pos.0.x, cursor.pos.0.y, cursor.pos.0.z]),
        );
        tag.put_int("charge", i32::from(cursor.charge));
        tag.put_int("decay_delay", i32::from(cursor.decay_delay));
        tag.put_int("update_delay", i32::from(cursor.update_delay));
        if let Some(faces) = cursor.faces {
            let facings = BlockDirection::all()
                .into_iter()
                .filter(|direction| faces & (1 << direction.to_index()) != 0)
                .map(|direction| NbtTag::String(direction_name(direction).into()))
                .collect();
            tag.put_list("facings", facings);
        }
        cursors.push(NbtTag::Compound(tag));
    }
    nbt.put_list("cursors", cursors);
}

const fn direction_name(direction: BlockDirection) -> &'static str {
    match direction {
        BlockDirection::Down => "down",
        BlockDirection::Up => "up",
        BlockDirection::North => "north",
        BlockDirection::South => "south",
        BlockDirection::West => "west",
        BlockDirection::East => "east",
    }
}

fn direction_from_name(name: &str) -> Option<BlockDirection> {
    Some(match name {
        "down" => BlockDirection::Down,
        "up" => BlockDirection::Up,
        "north" => BlockDirection::North,
        "south" => BlockDirection::South,
        "west" => BlockDirection::West,
        "east" => BlockDirection::East,
        _ => return None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cursor_nbt_round_trip() {
        let mut spreader = SculkSpreader::new_level_spreader();
        spreader.add_cursor(ChargeCursor {
            pos: BlockPos::new(12, -4, 31),
            charge: 750,
            update_delay: 2,
            decay_delay: 0,
            faces: Some(
                (1 << BlockDirection::Up.to_index()) | (1 << BlockDirection::North.to_index()),
            ),
        });

        let mut nbt = NbtCompound::new();
        write_cursors(&spreader, &mut nbt);

        let spreader = spreader_from_nbt(&nbt);
        let cursor = &spreader.cursors()[0];
        assert_eq!(cursor.pos, BlockPos::new(12, -4, 31));
        assert_eq!(cursor.charge, 750);
        assert_eq!(cursor.update_delay, 2);
        assert_eq!(cursor.decay_delay, 0);
        assert_eq!(
            cursor.faces,
            Some((1 << BlockDirection::Up.to_index()) | (1 << BlockDirection::North.to_index()))
        );
    }
}
