use pumpkin_data::BlockStateId;
use pumpkin_macros::pumpkin_block;
use pumpkin_world::world::BlockFlags;
use std::sync::Arc;

use crate::block::BlockBehaviour;
use crate::block::CanPlaceAtArgs;
use crate::block::OnNeighborUpdateArgs;
use crate::block::OnPlaceArgs;
use crate::block::OnStateReplacedArgs;
use crate::block::PlacedArgs;
use crate::entity::EntityBase;
use crate::world::World;
use pumpkin_data::Block;
use pumpkin_util::math::position::BlockPos;

use super::super::block_receives_redstone_power;
use super::RailProperties;
use super::common::{
    can_place_rail_at, compute_placed_rail_shape, rail_placement_is_valid,
    update_flanking_rails_shape,
};

// Vanilla registers both powered and activator rails with `PoweredRailBlock`; each rail only
// propagates power to rails of its own block type, which is why `args.block` is threaded through.
#[pumpkin_block("minecraft:powered_rail", "minecraft:activator_rail")]
pub struct PoweredRailBlock;

impl BlockBehaviour for PoweredRailBlock {
    fn on_place(&self, args: OnPlaceArgs<'_>) -> BlockStateId {
        let mut rail_props = RailProperties::default(args.block);
        let player_facing = args.player.get_entity().get_horizontal_facing();

        rail_props.set_waterlogged(args.replacing.water_source());
        rail_props.set_straight_shape(compute_placed_rail_shape(
            args.world,
            args.position,
            player_facing,
        ));

        rail_props.to_state_id(args.block)
    }

    fn placed(&self, args: PlacedArgs<'_>) {
        update_flanking_rails_shape(args.world, args.block, args.state_id, args.position);

        self.update_powered_state(args.world, args.block, args.position);

        let final_state_id = args.world.get_block_state_id(args.position);
        let rail_props = RailProperties::new(final_state_id, args.block);

        self.update_connected_rails(args.world, args.block, args.position, &rail_props, true, 0);
        self.update_connected_rails(args.world, args.block, args.position, &rail_props, false, 0);

        for direction in rail_props.directions() {
            let neighbor_pos = args.position.offset(direction.to_offset());

            if let Some(neighbor_rail) =
                Self::find_rail_at_position(args.world, args.block, &neighbor_pos)
            {
                self.update_powered_state_internal(
                    args.world,
                    neighbor_rail.0,
                    &neighbor_pos,
                    false,
                );
                self.update_connected_rails(
                    args.world,
                    args.block,
                    &neighbor_pos,
                    &neighbor_rail.1,
                    true,
                    0,
                );
                self.update_connected_rails(
                    args.world,
                    args.block,
                    &neighbor_pos,
                    &neighbor_rail.1,
                    false,
                    0,
                );
            }

            let up_pos = neighbor_pos.up();
            if let Some(neighbor_rail) =
                Self::find_rail_at_position(args.world, args.block, &up_pos)
            {
                self.update_powered_state_internal(args.world, neighbor_rail.0, &up_pos, false);
                self.update_connected_rails(
                    args.world,
                    args.block,
                    &up_pos,
                    &neighbor_rail.1,
                    true,
                    0,
                );
                self.update_connected_rails(
                    args.world,
                    args.block,
                    &up_pos,
                    &neighbor_rail.1,
                    false,
                    0,
                );
            }

            let down_pos = neighbor_pos.down();
            if let Some(neighbor_rail) =
                Self::find_rail_at_position(args.world, args.block, &down_pos)
            {
                self.update_powered_state_internal(args.world, neighbor_rail.0, &down_pos, false);
                self.update_connected_rails(
                    args.world,
                    args.block,
                    &down_pos,
                    &neighbor_rail.1,
                    true,
                    0,
                );
                self.update_connected_rails(
                    args.world,
                    args.block,
                    &down_pos,
                    &neighbor_rail.1,
                    false,
                    0,
                );
            }
        }
    }

    fn on_neighbor_update(&self, args: OnNeighborUpdateArgs<'_>) {
        if !rail_placement_is_valid(args.world, args.block, args.position) {
            args.world
                .break_block(args.position, None, BlockFlags::NOTIFY_ALL);
            return;
        }

        self.update_powered_state(args.world, args.block, args.position);

        let state_id = args.world.get_block_state_id(args.position);
        let rail_props = RailProperties::new(state_id, args.block);

        self.update_connected_rails(args.world, args.block, args.position, &rail_props, true, 0);
        self.update_connected_rails(args.world, args.block, args.position, &rail_props, false, 0);
    }

    fn on_state_replaced(&self, args: OnStateReplacedArgs<'_>) {
        let state_id = args.old_state_id;
        let rail_props = RailProperties::new(state_id, args.block);

        if rail_props.shape().is_ascending() {
            args.world.update_neighbor(&args.position.up(), args.block);
        }

        args.world.update_neighbor(args.position, args.block);
        args.world
            .update_neighbor(&args.position.down(), args.block);

        let directions = rail_props.directions();
        for direction in directions {
            let neighbor_pos = args.position.offset(direction.to_offset());

            if let Some(neighbor_rail) =
                Self::find_rail_at_position(args.world, args.block, &neighbor_pos)
            {
                self.update_powered_state(args.world, neighbor_rail.0, &neighbor_pos);
                self.update_connected_rails(
                    args.world,
                    args.block,
                    &neighbor_pos,
                    &neighbor_rail.1,
                    true,
                    0,
                );
                self.update_connected_rails(
                    args.world,
                    args.block,
                    &neighbor_pos,
                    &neighbor_rail.1,
                    false,
                    0,
                );
            }

            let up_pos = neighbor_pos.up();
            if let Some(neighbor_rail) =
                Self::find_rail_at_position(args.world, args.block, &up_pos)
            {
                self.update_powered_state(args.world, neighbor_rail.0, &up_pos);
                self.update_connected_rails(
                    args.world,
                    args.block,
                    &up_pos,
                    &neighbor_rail.1,
                    true,
                    0,
                );
                self.update_connected_rails(
                    args.world,
                    args.block,
                    &up_pos,
                    &neighbor_rail.1,
                    false,
                    0,
                );
            }

            let down_pos = neighbor_pos.down();
            if let Some(neighbor_rail) =
                Self::find_rail_at_position(args.world, args.block, &down_pos)
            {
                self.update_powered_state(args.world, neighbor_rail.0, &down_pos);
                self.update_connected_rails(
                    args.world,
                    args.block,
                    &down_pos,
                    &neighbor_rail.1,
                    true,
                    0,
                );
                self.update_connected_rails(
                    args.world,
                    args.block,
                    &down_pos,
                    &neighbor_rail.1,
                    false,
                    0,
                );
            }
        }
    }

    fn can_place_at(&self, args: CanPlaceAtArgs<'_>) -> bool {
        can_place_rail_at(args.block_accessor, args.position)
    }
}

impl PoweredRailBlock {
    fn is_powered_by_other_rails(
        &self,
        world: &World,
        block: &Block,
        pos: &BlockPos,
        state: &RailProperties,
        direction: bool,
        distance: u8,
    ) -> bool {
        if distance >= 8 {
            return false;
        }

        let mut x = pos.0.x;
        let mut y = pos.0.y;
        let mut z = pos.0.z;
        let mut check_down = true;
        let mut next_shape = state.shape();

        match next_shape {
            pumpkin_data::block_properties::RailShape::NorthSouth => {
                if direction {
                    z += 1;
                } else {
                    z -= 1;
                }
            }
            pumpkin_data::block_properties::RailShape::EastWest => {
                if direction {
                    x -= 1;
                } else {
                    x += 1;
                }
            }
            pumpkin_data::block_properties::RailShape::AscendingEast => {
                if direction {
                    x -= 1;
                } else {
                    x += 1;
                    y += 1;
                    check_down = false;
                }
                next_shape = pumpkin_data::block_properties::RailShape::EastWest;
            }
            pumpkin_data::block_properties::RailShape::AscendingWest => {
                if direction {
                    x -= 1;
                    y += 1;
                    check_down = false;
                } else {
                    x += 1;
                }
                next_shape = pumpkin_data::block_properties::RailShape::EastWest;
            }
            pumpkin_data::block_properties::RailShape::AscendingNorth => {
                if direction {
                    z += 1;
                } else {
                    z -= 1;
                    y += 1;
                    check_down = false;
                }
                next_shape = pumpkin_data::block_properties::RailShape::NorthSouth;
            }
            pumpkin_data::block_properties::RailShape::AscendingSouth => {
                if direction {
                    z += 1;
                    y += 1;
                    check_down = false;
                } else {
                    z -= 1;
                }
                next_shape = pumpkin_data::block_properties::RailShape::NorthSouth;
            }
            _ => return false,
        }

        if self.is_powered_at_position(
            world,
            block,
            &BlockPos::new(x, y, z),
            direction,
            distance,
            next_shape,
        ) {
            return true;
        }

        check_down
            && self.is_powered_at_position(
                world,
                block,
                &BlockPos::new(x, y - 1, z),
                direction,
                distance,
                next_shape,
            )
    }

    fn is_powered_at_position(
        &self,
        world: &World,
        block: &Block,
        pos: &BlockPos,
        direction: bool,
        distance: u8,
        expected_shape: pumpkin_data::block_properties::RailShape,
    ) -> bool {
        if !std::ptr::eq(world.get_block(pos), block) {
            return false;
        }

        let state_id = world.get_block_state_id(pos);
        let rail_props = RailProperties::new(state_id, block);
        let rail_shape = rail_props.shape();

        match expected_shape {
            pumpkin_data::block_properties::RailShape::EastWest => {
                if matches!(
                    rail_shape,
                    pumpkin_data::block_properties::RailShape::NorthSouth
                        | pumpkin_data::block_properties::RailShape::AscendingNorth
                        | pumpkin_data::block_properties::RailShape::AscendingSouth
                ) {
                    return false;
                }
            }
            pumpkin_data::block_properties::RailShape::NorthSouth => {
                if matches!(
                    rail_shape,
                    pumpkin_data::block_properties::RailShape::EastWest
                        | pumpkin_data::block_properties::RailShape::AscendingEast
                        | pumpkin_data::block_properties::RailShape::AscendingWest
                ) {
                    return false;
                }
            }
            _ => {}
        }

        if !rail_props.is_powered() {
            return false;
        }

        if block_receives_redstone_power(world, pos) {
            return true;
        }

        self.is_powered_by_other_rails(world, block, pos, &rail_props, direction, distance + 1)
    }

    fn update_powered_state(&self, world: &Arc<World>, block: &Block, pos: &BlockPos) {
        self.update_powered_state_internal(world, block, pos, true);
    }

    fn update_powered_state_internal(
        &self,
        world: &Arc<World>,
        block: &Block,
        pos: &BlockPos,
        propagate: bool,
    ) {
        let state_id = world.get_block_state_id(pos);
        let mut rail_props = RailProperties::new(state_id, block);
        let current_powered = rail_props.is_powered();

        let direct_power = block_receives_redstone_power(world, pos);

        let rail_power = self.is_powered_by_other_rails(world, block, pos, &rail_props, true, 0)
            || self.is_powered_by_other_rails(world, block, pos, &rail_props, false, 0);

        let should_be_powered = direct_power || rail_power;

        if current_powered != should_be_powered {
            rail_props.set_powered(should_be_powered);
            world.set_block_state(pos, rail_props.to_state_id(block), BlockFlags::NOTIFY_ALL);

            world.update_neighbor(&pos.down(), block);

            if rail_props.shape().is_ascending() {
                world.update_neighbor(&pos.up(), block);
            }

            if propagate {
                let updated_rail_props = RailProperties::new(rail_props.to_state_id(block), block);
                self.update_connected_rails(world, block, pos, &updated_rail_props, true, 0);
                self.update_connected_rails(world, block, pos, &updated_rail_props, false, 0);
            }
        }
    }

    fn update_connected_rails(
        &self,
        world: &Arc<World>,
        block: &Block,
        pos: &BlockPos,
        state: &RailProperties,
        direction: bool,
        distance: u8,
    ) {
        if distance >= 8 {
            return;
        }

        let mut x = pos.0.x;
        let mut y = pos.0.y;
        let mut z = pos.0.z;
        let mut check_down = true;
        let mut next_shape = state.shape();

        match next_shape {
            pumpkin_data::block_properties::RailShape::NorthSouth => {
                if direction {
                    z += 1;
                } else {
                    z -= 1;
                }
            }
            pumpkin_data::block_properties::RailShape::EastWest => {
                if direction {
                    x -= 1;
                } else {
                    x += 1;
                }
            }
            pumpkin_data::block_properties::RailShape::AscendingEast => {
                if direction {
                    x -= 1;
                } else {
                    x += 1;
                    y += 1;
                    check_down = false;
                }
                next_shape = pumpkin_data::block_properties::RailShape::EastWest;
            }
            pumpkin_data::block_properties::RailShape::AscendingWest => {
                if direction {
                    x -= 1;
                    y += 1;
                    check_down = false;
                } else {
                    x += 1;
                }
                next_shape = pumpkin_data::block_properties::RailShape::EastWest;
            }
            pumpkin_data::block_properties::RailShape::AscendingNorth => {
                if direction {
                    z += 1;
                } else {
                    z -= 1;
                    y += 1;
                    check_down = false;
                }
                next_shape = pumpkin_data::block_properties::RailShape::NorthSouth;
            }
            pumpkin_data::block_properties::RailShape::AscendingSouth => {
                if direction {
                    z += 1;
                    y += 1;
                    check_down = false;
                } else {
                    z -= 1;
                }
                next_shape = pumpkin_data::block_properties::RailShape::NorthSouth;
            }
            _ => return,
        }

        let next_pos = BlockPos::new(x, y, z);
        self.update_rail_at_position(world, block, &next_pos, direction, distance, next_shape);

        if check_down {
            let down_pos = BlockPos::new(x, y - 1, z);
            self.update_rail_at_position(world, block, &down_pos, direction, distance, next_shape);
        }
    }

    fn update_rail_at_position(
        &self,
        world: &Arc<World>,
        block: &Block,
        pos: &BlockPos,
        direction: bool,
        distance: u8,
        expected_shape: pumpkin_data::block_properties::RailShape,
    ) {
        if !std::ptr::eq(world.get_block(pos), block) {
            return;
        }

        let state_id = world.get_block_state_id(pos);
        let rail_props = RailProperties::new(state_id, block);
        let rail_shape = rail_props.shape();

        let shapes_compatible = match expected_shape {
            pumpkin_data::block_properties::RailShape::EastWest => !matches!(
                rail_shape,
                pumpkin_data::block_properties::RailShape::NorthSouth
                    | pumpkin_data::block_properties::RailShape::AscendingNorth
                    | pumpkin_data::block_properties::RailShape::AscendingSouth
            ),
            pumpkin_data::block_properties::RailShape::NorthSouth => !matches!(
                rail_shape,
                pumpkin_data::block_properties::RailShape::EastWest
                    | pumpkin_data::block_properties::RailShape::AscendingEast
                    | pumpkin_data::block_properties::RailShape::AscendingWest
            ),
            _ => true,
        };

        if shapes_compatible {
            self.update_powered_state_internal(world, block, pos, false);

            self.update_connected_rails(world, block, pos, &rail_props, direction, distance + 1);
        }
    }

    fn find_rail_at_position(
        world: &World,
        block: &Block,
        pos: &BlockPos,
    ) -> Option<(&'static Block, RailProperties)> {
        let found = world.get_block(pos);
        std::ptr::eq(found, block).then(|| {
            let state_id = world.get_block_state_id(pos);
            let rail_props = RailProperties::new(state_id, found);
            (found, rail_props)
        })
    }
}
