use pumpkin_data::Block;
use pumpkin_gametest::{GameTestHelper, GameTestResult};
use pumpkin_util::math::position::BlockPos;

use super::build_stone_floor;

pub(super) async fn test_copper_golem_transfers_items_between_chests(
    helper: GameTestHelper,
) -> GameTestResult<()> {
    build_stone_floor(&helper, 9, 5).await?;

    let copper_chest_pos = BlockPos::new(2, 1, 2);
    let chest_pos = BlockPos::new(6, 1, 2);
    let golem_pos = BlockPos::new(4, 1, 2);

    helper
        .set_block_default(&copper_chest_pos, &Block::COPPER_CHEST)
        .await?;
    helper.set_block_default(&chest_pos, &Block::CHEST).await?;
    helper
        .set_container_item(&copper_chest_pos, 0, "minecraft:copper_ingot", 3)
        .await?;

    let absolute = helper.absolute_pos(&golem_pos);
    helper
        .execute_command(
            &golem_pos,
            &format!(
                "summon minecraft:copper_golem {} {} {} {{PersistenceRequired:1b}}",
                absolute.0.x, absolute.0.y, absolute.0.z
            ),
        )
        .await?;
    helper
        .assert_entity_present("minecraft:copper_golem")
        .await?;

    helper.succeed_when(move |helper| async move {
        helper.assert_container_empty(&copper_chest_pos).await?;
        helper
            .assert_container_contains_count(&chest_pos, "minecraft:copper_ingot", 3)
            .await
    })?;
    Ok(())
}
