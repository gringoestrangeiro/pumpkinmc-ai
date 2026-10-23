use std::{sync::Arc, time::Duration};

use pumpkin_core::{server::Server, world::BlockFlags};
use pumpkin_data::Block;
use pumpkin_util::math::{position::BlockPos, vector3::Vector3};

/// Places a block and waits until the chunk is loaded, so the block is not
/// lost. Chunks load in the background, so a block that is placed right after
/// the bot logs in can end up in a chunk that is not loaded yet.
pub async fn place_block(
    server: &Arc<Server>,
    world: &Arc<pumpkin_core::world::World>,
    pos: &BlockPos,
    block: &Block,
) {
    for _ in 0..100 {
        world.set_block_state(pos, block.default_state.id, BlockFlags::NOTIFY_ALL);
        if world.get_block(pos) == block {
            return;
        }
        // The block was written into a chunk that is not loaded, so the server
        // is ticked until the chunk is there.
        for _ in 0..5 {
            tokio::time::sleep(Duration::from_millis(5)).await;
            server.tick();
        }
    }
}

/// A spot next to the bot with free space above solid ground. The terrain at
/// the bot spawn can be water or a hill, so the spot is cleared and given a
/// stone floor. Four blocks are cleared above the floor, because a mob that
/// does not fit into its spot suffocates in the terrain above it.
pub async fn prepare_spot(
    server: &Arc<Server>,
    world: &Arc<pumpkin_core::world::World>,
    pos: &Vector3<f64>,
) -> BlockPos {
    let block_pos = BlockPos::new(
        (pos.x + 2.0).floor() as i32,
        pos.y.floor() as i32,
        pos.z.floor() as i32,
    );
    for offset in 0..4 {
        place_block(server, world, &block_pos.up_height(offset), &Block::AIR).await;
    }
    place_block(server, world, &block_pos.down(), &Block::STONE).await;
    block_pos
}
