use std::{
    sync::Arc,
    time::{Duration, Instant},
};

use pumpkin_core::server::Server;
use pumpkin_data::{Block, block_properties::OakTrapdoorLikeProperties, dimension::Dimension};
use pumpkin_util::math::position::BlockPos;

use crate::bot::HeadlessBot;
use crate::bot_tests::setup::place_block;

pub async fn run_world_interaction_tests(
    server: &Arc<Server>,
    bot1: &mut HeadlessBot,
    summary_lines: &mut Vec<String>,
) -> Result<(), String> {
    let start = Instant::now();
    let world = server.get_world_from_dimension(&Dimension::OVERWORLD);

    let base_x = bot1.position.x.floor() as i32;
    let base_y = bot1.position.y.floor() as i32;
    let base_z = bot1.position.z.floor() as i32;

    let trapdoor_pos = BlockPos::new(base_x + 1, base_y, base_z);
    let break_pos = BlockPos::new(base_x - 1, base_y + 1, base_z);

    // Fill the columns with stone, so the terrain or falling sand cannot
    // replace the blocks of the test.
    for pos in [&trapdoor_pos, &break_pos] {
        for y in -1..=3 {
            place_block(
                server,
                &world,
                &BlockPos::new(pos.0.x, pos.0.y + y, pos.0.z),
                &Block::STONE,
            )
            .await;
        }
    }
    place_block(
        server,
        &world,
        &BlockPos::new(trapdoor_pos.0.x, trapdoor_pos.0.y + 1, trapdoor_pos.0.z),
        &Block::AIR,
    )
    .await;

    place_block(server, &world, &trapdoor_pos, &Block::OAK_TRAPDOOR).await;
    place_block(server, &world, &break_pos, &Block::GOLD_BLOCK).await;

    // 1. Right-click oak trapdoor via SUseItemOn -> should toggle open
    // 2. Left-click gold block in Creative mode via SPlayerAction(StartedDigging) -> should break to AIR
    // The packets can be dropped while the server is busy, so they are sent
    // again while the world does not show the result yet.
    let mut trapdoor_opened = false;
    let mut block_broken = false;
    for attempt in 0..60 {
        if attempt % 10 == 0 {
            bot1.use_block(server, trapdoor_pos, 1).await?;
            bot1.start_digging(server, break_pos, 2).await?;
        }
        tokio::time::sleep(Duration::from_millis(5)).await;
        server.tick();

        let state = world.get_block_state(&trapdoor_pos);
        if world.get_block(&trapdoor_pos).id == Block::OAK_TRAPDOOR.id
            && OakTrapdoorLikeProperties::from_state_id(state.id).open
        {
            trapdoor_opened = true;
        }

        if world.get_block(&break_pos).id == Block::AIR.id {
            block_broken = true;
        }

        if trapdoor_opened && block_broken {
            break;
        }
    }

    if !trapdoor_opened {
        return Err(
            "Bot SUseItemOn right-click did not open the oak trapdoor in the world".to_string(),
        );
    }
    if !block_broken {
        return Err(
            "Bot SPlayerAction(StartedDigging) in Creative mode did not break the gold block"
                .to_string(),
        );
    }

    summary_lines.push(format!(
        "[PASS] bot:world_block_use_and_break ({}ms)",
        start.elapsed().as_millis()
    ));
    Ok(())
}
