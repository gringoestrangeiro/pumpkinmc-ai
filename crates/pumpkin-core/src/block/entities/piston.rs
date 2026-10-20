use std::sync::Arc;
use std::sync::atomic::Ordering;

use crossbeam::atomic::AtomicCell;
use pumpkin_data::block_properties::Axis as BlockAxis;
use pumpkin_data::{Block, BlockDirection, BlockState};
use pumpkin_nbt::compound::NbtCompound;
use pumpkin_util::math::{boundingbox::BoundingBox, position::BlockPos, vector3::Vector3};
use pumpkin_world::block::BlockStateCodec;

use crate::world::{BlockFlags, World};

use super::BlockEntity;

pub struct PistonBlockEntity {
    pub position: BlockPos,
    pub pushed_block_state: &'static BlockState,
    pub facing: BlockDirection,
    pub current_progress: AtomicCell<f32>,
    pub last_progress: AtomicCell<f32>,
    pub extending: bool,
    pub source: bool,
    /// World age this entity last ticked, `-1` if never. Vanilla `lastTicked`.
    pub last_ticked: AtomicCell<i64>,
}

impl PistonBlockEntity {
    pub const ID: &'static str = "minecraft:piston";

    /// Vanilla `PistonMovingBlockEntity` constructor for a fresh animation.
    #[must_use]
    pub fn new(
        position: BlockPos,
        facing: BlockDirection,
        pushed_block_state: &'static BlockState,
        extending: bool,
        source: bool,
    ) -> Self {
        Self {
            position,
            facing,
            pushed_block_state,
            current_progress: 0.0.into(),
            last_progress: 0.0.into(),
            extending,
            source,
            last_ticked: (-1i64).into(),
        }
    }

    const fn movement_direction(&self) -> BlockDirection {
        if self.extending {
            self.facing
        } else {
            self.facing.opposite()
        }
    }

    /// Vanilla `PistonBaseBlock.checkIfExtend` `TRIGGER_DROP`: the cell two ahead is
    /// still extending and barely started, or it ticked this tick.
    #[must_use]
    pub fn should_drop_instead_of_pull(&self, world: &World) -> bool {
        self.extending
            && (self.last_progress.load() < 0.5 || self.last_ticked.load() == world.get_world_age())
    }

    /// Vanilla's `getAmountExtended`: how far back from the block's final position
    /// the visual is at a given animation progress. Negative for extending.
    fn amount_extended(&self, progress: f32) -> f32 {
        if self.extending {
            progress - 1.0
        } else {
            1.0 - progress
        }
    }

    fn dir_vec(dir: BlockDirection, scale: f64) -> Vector3<f64> {
        let off = dir.to_offset();
        Vector3::new(
            f64::from(off.x) * scale,
            f64::from(off.y) * scale,
            f64::from(off.z) * scale,
        )
    }

    /// Ports vanilla `PistonBlockEntity.pushEntities`: pushes entities whose
    /// bounding box intersects the moving block's swept volume during this tick.
    fn push_entities(&self, world: &Arc<World>, new_progress: f32) {
        let last = self.last_progress.load();
        let delta = f64::from(new_progress - last);
        if delta <= 0.0 {
            return;
        }

        let motion_dir = self.movement_direction();
        let amount = f64::from(self.amount_extended(last));

        // Block AABB at the current visual position (start of this tick).
        // For source=true (the piston head BE), vanilla uses the piston-head
        // collision shape — a 4-pixel-thick slab at the front face — rather than
        // the full cube. Without this, the head "sweeps" too much volume and
        // drags entities (e.g. end crystals) back with it during retraction.
        let raw_block_aabb = BoundingBox::from_block(&self.position);
        let block_aabb = if self.source {
            Self::head_shape_aabb(raw_block_aabb, self.facing)
        } else {
            raw_block_aabb
        }
        .shift(Self::dir_vec(self.facing, amount));

        // Stretch by motion to get the swept volume (one tick of animation).
        let motion = Self::dir_vec(motion_dir, delta);
        let swept = block_aabb.stretch(motion);

        let launches_entities =
            Block::from_state_id(self.pushed_block_state.id) == &Block::SLIME_BLOCK;

        for entity in world.get_entities_at_box(&swept) {
            let e = entity.get_entity();
            if e.no_physics.load(Ordering::Relaxed) {
                continue;
            }
            // Player movement is client-authoritative; vanilla still nudges them
            // via Entity.move(PISTON), but we skip them here to avoid teleport jank.
            if entity.get_player().is_some() {
                continue;
            }

            // Vanilla `moveCollidedEntities` replaces the movement axis with a full
            // block per tick when the moved block is a slime block.
            if launches_entities {
                let axis = motion_dir.to_axis().into();
                let unit = Self::dir_vec(motion_dir, 1.0);
                let mut velocity = e.velocity.load();
                velocity.set_axis(axis, unit.get_axis(axis));
                e.velocity.store(velocity);
                e.send_velocity();
            }

            let entity_aabb = e.bounding_box.load();
            let intersection = Self::intersection_size(swept, motion_dir, entity_aabb);
            if intersection <= 0.0 {
                continue;
            }
            let push_amount = intersection.min(delta) + 0.01;
            Self::move_entity(e, motion_dir, push_amount);

            // For a retracting head, vanilla also shoves the entity OUT of the
            // piston body cube. Without this, entities get pulled into the
            // piston and "stick" to it (looks like a sticky-piston drag).
            // For a retract-head BE, `self.position` already IS the piston
            // block position (it replaces the piston during animation).
            if !self.extending && self.source {
                Self::push_out_of_piston_body(e, &self.position, motion_dir, delta);
            }
        }
    }

    /// Vanilla `PistonMovingBlockEntity.moveStuckEntities`: a horizontally moving honey
    /// block drags the entities standing on it. Vertical moves do not drag.
    fn move_stuck_entities(&self, world: &Arc<World>, new_progress: f32) {
        if Block::from_state_id(self.pushed_block_state.id) != &Block::HONEY_BLOCK {
            return;
        }

        let motion_dir = self.movement_direction();
        if motion_dir.to_axis() == BlockAxis::Y {
            return;
        }

        let delta = f64::from(new_progress - self.last_progress.load());
        if delta <= 0.0 {
            return;
        }

        let sticky_top = self
            .pushed_block_state
            .get_block_collision_shapes()
            .map(|shape| shape.max.y)
            .fold(f64::NEG_INFINITY, f64::max);
        if !sticky_top.is_finite() {
            return;
        }

        let query = BoundingBox::new(
            Vector3::new(0.0, sticky_top, 0.0),
            Vector3::new(1.0, 1.500_001_000_000_000_1, 1.0),
        )
        .at_pos(self.position)
        .shift(Self::dir_vec(
            self.facing,
            f64::from(self.amount_extended(self.last_progress.load())),
        ));

        // Players are excluded by `get_entities_at_box`; their movement is client
        // authoritative, same as `push_entities`.
        for entity in world.get_entities_at_box(&query) {
            let e = entity.get_entity();
            if e.no_physics.load(Ordering::Relaxed) {
                continue;
            }
            if !e.on_ground.load(Ordering::Relaxed) {
                continue;
            }
            let pos = e.pos.load();
            let supported = e.get_supporting_block_pos() == Some(self.position);
            let within_footprint = pos.x >= query.min.x
                && pos.x <= query.max.x
                && pos.z >= query.min.z
                && pos.z <= query.max.z;
            if !supported && !within_footprint {
                continue;
            }
            Self::move_entity(e, motion_dir, delta);
        }
    }

    /// Vanilla `getIntersectionSize`: how much `entity` overlaps `swept` along
    /// `motion_dir`. Positive means the entity is in the path of the moving block.
    fn intersection_size(
        swept: BoundingBox,
        motion_dir: BlockDirection,
        entity: BoundingBox,
    ) -> f64 {
        match motion_dir {
            BlockDirection::East => swept.max.x - entity.min.x,
            BlockDirection::West => entity.max.x - swept.min.x,
            BlockDirection::Up => swept.max.y - entity.min.y,
            BlockDirection::Down => entity.max.y - swept.min.y,
            BlockDirection::South => swept.max.z - entity.min.z,
            BlockDirection::North => entity.max.z - swept.min.z,
        }
    }

    fn move_entity(entity: &crate::entity::Entity, dir: BlockDirection, distance: f64) {
        let new_pos = entity.pos.load() + Self::dir_vec(dir, distance);
        entity.set_pos(new_pos);
    }

    /// Vanilla `push`: when a piston head retracts, shove entities that ended up
    /// inside the piston-body cube back out the opposite direction (slightly past
    /// the move they just got, so the net motion is essentially zero).
    fn push_out_of_piston_body(
        entity: &crate::entity::Entity,
        piston_pos: &BlockPos,
        motion_dir: BlockDirection,
        amount: f64,
    ) {
        let body_aabb = BoundingBox::from_block(piston_pos);
        let entity_aabb = entity.bounding_box.load();
        if !body_aabb.intersects(&entity_aabb) {
            return;
        }
        let back = motion_dir.opposite();
        let e = Self::intersection_size(body_aabb, back, entity_aabb) + 0.01;
        let f = Self::intersection_size(
            body_aabb,
            back,
            Self::aabb_intersection(body_aabb, entity_aabb),
        ) + 0.01;
        if (e - f).abs() < 0.01 {
            let distance = e.min(amount) + 0.01;
            Self::move_entity(entity, back, distance);
        }
    }

    /// Approximation of vanilla `PistonHeadBlock` collision shape: a 4-pixel
    /// (0.25 block) slab at the front face of the block in the facing direction.
    fn head_shape_aabb(block_aabb: BoundingBox, facing: BlockDirection) -> BoundingBox {
        const HEAD_THICKNESS: f64 = 0.25;
        let mut min = block_aabb.min;
        let mut max = block_aabb.max;
        match facing {
            BlockDirection::East => min.x = max.x - HEAD_THICKNESS,
            BlockDirection::West => max.x = min.x + HEAD_THICKNESS,
            BlockDirection::Up => min.y = max.y - HEAD_THICKNESS,
            BlockDirection::Down => max.y = min.y + HEAD_THICKNESS,
            BlockDirection::South => min.z = max.z - HEAD_THICKNESS,
            BlockDirection::North => max.z = min.z + HEAD_THICKNESS,
        }
        BoundingBox::new(min, max)
    }

    const fn aabb_intersection(a: BoundingBox, b: BoundingBox) -> BoundingBox {
        BoundingBox::new(
            Vector3::new(
                a.min.x.max(b.min.x),
                a.min.y.max(b.min.y),
                a.min.z.max(b.min.z),
            ),
            Vector3::new(
                a.max.x.min(b.max.x),
                a.max.y.min(b.max.y),
                a.max.z.min(b.max.z),
            ),
        )
    }

    /// Vanilla `PistonMovingBlockEntity.saveAdditional`. Writes `progressO`.
    fn write_fields(&self, nbt: &mut NbtCompound) {
        nbt.put_compound(
            BLOCK_STATE,
            BlockStateCodec::to_nbt(self.pushed_block_state.id),
        );
        nbt.put_byte(FACING, self.facing.to_index() as i8);
        nbt.put_float(PROGRESS, self.last_progress.load());
        nbt.put_bool(EXTENDING, self.extending);
        nbt.put_bool(SOURCE, self.source);
    }

    pub fn finish(&self, world: &Arc<World>) {
        if self.last_progress.load() < 1.0 {
            // Vanilla `finalTick` parks the animation at its end first, so a second
            // `finish` on the same instance is a no-op.
            self.current_progress.store(1.0);
            self.last_progress.store(1.0);
            let pos = self.position;
            world.remove_block_entity(&pos);
            if world.get_block(&pos) == &Block::MOVING_PISTON {
                let state = if self.source {
                    Block::AIR.default_state.id
                } else {
                    world.update_from_neighbor_shapes(self.pushed_block_state.id, &pos)
                };
                world.set_block_state(&pos, state, BlockFlags::NOTIFY_ALL);
                world.update_neighbors(&pos, None);
            }
        }
    }
}

const BLOCK_STATE: &str = "blockState";
const FACING: &str = "facing";
const PROGRESS: &str = "progress";
const EXTENDING: &str = "extending";
const SOURCE: &str = "source";

impl BlockEntity for PistonBlockEntity {
    fn resource_location(&self) -> &'static str {
        Self::ID
    }

    fn get_position(&self) -> BlockPos {
        self.position
    }

    fn tick(&self, world: &Arc<World>) {
        self.last_ticked.store(world.get_world_age());
        let current_progress = self.current_progress.load();
        self.last_progress.store(current_progress);
        if current_progress >= 1.0 {
            let pos = self.position;
            world.remove_block_entity(&pos);
            if world.get_block(&pos) == &Block::MOVING_PISTON {
                let updated_state_id =
                    world.update_from_neighbor_shapes(self.pushed_block_state.id, &pos);
                if BlockState::from_id(updated_state_id).is_air() {
                    // Vanilla places the moved state and destroys it again, so a block
                    // that cannot survive here (rail, torch) drops instead of vanishing.
                    let flags = BlockFlags::FORCE_STATE
                        | BlockFlags::MOVED
                        | BlockFlags::SKIP_BLOCK_ENTITY_REPLACED_CALLBACK;
                    world.set_block_state(&pos, self.pushed_block_state.id, flags);
                    if !self.pushed_block_state.is_air() {
                        world.break_block(&pos, None, BlockFlags::NOTIFY_ALL);
                    }
                } else {
                    let updated_state = BlockState::from_id(updated_state_id)
                        .set_waterlogged(false)
                        .map_or(updated_state_id, |state| state.id);
                    world.set_block_state(
                        &pos,
                        updated_state,
                        BlockFlags::NOTIFY_ALL | BlockFlags::MOVED,
                    );
                    world.update_neighbors(&pos, None);
                }
            }
            return;
        }
        let new_progress = (current_progress + 0.5).min(1.0);
        self.push_entities(world, new_progress);
        self.move_stuck_entities(world, new_progress);
        self.current_progress.store(new_progress);
    }

    fn from_nbt(nbt: &pumpkin_nbt::compound::NbtCompound, position: BlockPos) -> Self
    where
        Self: Sized,
    {
        // Vanilla `DEFAULT_BLOCK_STATE` is air, also for an unknown saved state.
        let pushed_block_state = nbt
            .get_compound(BLOCK_STATE)
            .and_then(BlockStateCodec::from_nbt)
            .map_or(Block::AIR.default_state, BlockState::from_id);
        let facing = nbt.get_byte(FACING).unwrap_or(0);
        let last_progress = nbt.get_float(PROGRESS).unwrap_or(0.0);
        let extending = nbt.get_bool(EXTENDING).unwrap_or(false);
        let source = nbt.get_bool(SOURCE).unwrap_or(false);
        Self {
            pushed_block_state,
            position,
            facing: BlockDirection::from_index(facing as u8).unwrap_or(BlockDirection::Down),
            current_progress: last_progress.into(),
            last_progress: last_progress.into(),
            extending,
            source,
            last_ticked: (-1i64).into(),
        }
    }

    fn write_nbt(&self, nbt: &mut NbtCompound) {
        self.write_fields(nbt);
    }

    fn chunk_data_nbt(&self) -> Option<NbtCompound> {
        let mut nbt = NbtCompound::new();
        self.write_fields(&mut nbt);
        Some(nbt)
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}

#[cfg(test)]
mod tests {
    use super::{BlockEntity, PistonBlockEntity};
    use pumpkin_data::{Block, BlockDirection};
    use pumpkin_nbt::compound::NbtCompound;
    use pumpkin_util::math::position::BlockPos;

    #[test]
    fn moved_block_survives_save_and_load() {
        let entity = PistonBlockEntity::new(
            BlockPos::new(3, 64, 5),
            BlockDirection::East,
            Block::OAK_STAIRS.default_state,
            true,
            false,
        );
        entity.last_progress.store(0.5);
        let mut nbt = NbtCompound::new();
        entity.write_nbt(&mut nbt);

        let loaded = PistonBlockEntity::from_nbt(&nbt, entity.position);
        assert_eq!(
            loaded.pushed_block_state.id,
            Block::OAK_STAIRS.default_state.id
        );
        assert_eq!(loaded.facing, BlockDirection::East);
        assert_eq!(loaded.last_progress.load().to_bits(), 0.5f32.to_bits());
        assert!(loaded.extending);
        assert!(!loaded.source);
    }
}
