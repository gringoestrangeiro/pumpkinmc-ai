use pumpkin_data::Block;
use pumpkin_gametest::{GameTestHelper, GameTestResult};
use pumpkin_util::math::position::BlockPos;

use super::build_stone_floor;

/// Vanilla zoglins attack every living entity except creepers and other
/// zoglins. Keep both mobs inside a barrier pen so the test does not depend on
/// pathfinding back from the edge of the structure.
pub(super) async fn test_zoglin_attacks_living_target(
    helper: GameTestHelper,
) -> GameTestResult<()> {
    build_stone_floor(&helper, 5, 5).await?;
    for i in 0..5 {
        for y in 1..3 {
            for pos in [
                BlockPos::new(i, y, 0),
                BlockPos::new(i, y, 4),
                BlockPos::new(0, y, i),
                BlockPos::new(4, y, i),
            ] {
                helper.set_block_default(&pos, &Block::BARRIER).await?;
            }
        }
    }

    helper
        .spawn_entity("minecraft:zoglin", &BlockPos::new(1, 1, 2))
        .await?;
    helper
        .spawn_entity("minecraft:piglin", &BlockPos::new(3, 1, 2))
        .await?;
    helper.assert_entity_present("minecraft:piglin").await?;

    helper.succeed_when(move |helper| async move {
        helper.assert_entity_not_present("minecraft:piglin").await
    })?;
    Ok(())
}
