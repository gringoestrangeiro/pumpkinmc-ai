use pumpkin_data::block_properties::{Facing, PistonType};
use pumpkin_data::{Block, BlockState, BlockStateId, FacingExt};
use pumpkin_macros::pumpkin_block;
use pumpkin_util::GameMode;
use pumpkin_util::math::position::BlockPos;
use pumpkin_world::world::BlockFlags;

use crate::block::blocks::piston::piston::try_move;
use crate::block::{
    BlockBehaviour, BrokenArgs, GetStateForNeighborUpdateArgs, OnNeighborUpdateArgs,
    OnStateReplacedArgs, PathComputationType,
};

use super::piston::PistonProps;
use super::piston_extension::MovingPistonProps;

pub(crate) type PistonHeadProperties = pumpkin_data::block_properties::PistonHeadLikeProperties;

#[pumpkin_block("minecraft:piston_head")]
pub struct PistonHeadBlock;

impl PistonHeadBlock {
    /// Vanilla `PistonHeadBlock.isFittingBase`: the head belongs to an extended base of
    /// the matching type and facing.
    fn is_fitting_base(arm_state: &BlockState, potential_base: &BlockState) -> bool {
        let arm_props = PistonHeadProperties::from_state_id(arm_state.id);
        let base_block = match arm_props.r#type {
            PistonType::Sticky => &Block::STICKY_PISTON,
            PistonType::Normal => &Block::PISTON,
        };
        if Block::from_state_id(potential_base.id) != base_block {
            return false;
        }
        let base_props = PistonProps::from_state_id(potential_base.id);
        base_props.extended && base_props.facing == arm_props.facing
    }

    /// Vanilla `pos.relative(state.getValue(FACING).getOpposite())`.
    fn base_pos(head_state: &BlockState, pos: &BlockPos) -> BlockPos {
        let props = PistonHeadProperties::from_state_id(head_state.id);
        pos.offset(props.facing.opposite().to_block_direction().to_offset())
    }
}

impl BlockBehaviour for PistonHeadBlock {
    /// Vanilla `playerWillDestroy`: breaking the head in creative also removes the base,
    /// passing the breaking player through so protection and plugins see it.
    fn player_will_destroy(&self, args: BrokenArgs<'_>) {
        if args.player.gamemode.load() != GameMode::Creative {
            return;
        }
        let base_pos = Self::base_pos(args.state, args.position);
        let base_state = args.world.get_block_state(&base_pos);
        if Self::is_fitting_base(args.state, base_state) {
            args.world
                .break_block(&base_pos, Some(args.player), BlockFlags::SKIP_DROPS);
        }
    }

    /// Vanilla `affectNeighborsAfterRemoval`: removing the head in any other way destroys
    /// the base with its drops.
    fn on_state_replaced(&self, args: OnStateReplacedArgs<'_>) {
        if args.moved {
            return;
        }
        let state = BlockState::from_id(args.old_state_id);
        let base_pos = Self::base_pos(state, args.position);
        let base_state = args.world.get_block_state(&base_pos);
        if Self::is_fitting_base(state, base_state) {
            args.world
                .break_block(&base_pos, None, BlockFlags::NOTIFY_ALL);
        }
    }

    /// Vanilla `updateShape`/`canSurvive`: the head only survives next to an extended
    /// base of the same facing, or next to a matching `moving_piston`.
    fn get_state_for_neighbor_update(
        &self,
        args: GetStateForNeighborUpdateArgs<'_>,
    ) -> BlockStateId {
        let props = PistonHeadProperties::from_state_id(args.state_id);
        if args.direction.opposite() != props.facing.to_block_direction() {
            return args.state_id;
        }
        let head_state = BlockState::from_id(args.state_id);
        let base_state = BlockState::from_id(args.neighbor_state_id);
        let survives = Self::is_fitting_base(head_state, base_state)
            || (Block::from_state_id(args.neighbor_state_id) == &Block::MOVING_PISTON
                && MovingPistonProps::from_state_id(args.neighbor_state_id).facing == props.facing);
        if survives {
            args.state_id
        } else {
            BlockStateId::AIR
        }
    }

    fn on_neighbor_update(&self, args: OnNeighborUpdateArgs<'_>) {
        let head_state_id = args.world.get_block_state_id(args.position);
        let head_props = PistonHeadProperties::from_state_id(head_state_id);
        if head_props.facing != Facing::Up {
            return;
        }
        let piston_pos = args.position.offset(
            head_props
                .facing
                .opposite()
                .to_block_direction()
                .to_offset(),
        );
        let piston_block = args.world.get_block(&piston_pos);
        if &Block::PISTON == piston_block || &Block::STICKY_PISTON == piston_block {
            let up_pos = args
                .position
                .offset(head_props.facing.to_block_direction().to_offset());
            let upper_block = args.world.get_block(&up_pos);
            if upper_block != &Block::REDSTONE_BLOCK {
                //Then somebody probably broke the redstone block, try to check if piston should still be extended.
                try_move(args.world, piston_block, &piston_pos);
            }
        }
    }

    fn is_pathfindable(&self, _state: &BlockState, _computation_type: PathComputationType) -> bool {
        false
    }
}
