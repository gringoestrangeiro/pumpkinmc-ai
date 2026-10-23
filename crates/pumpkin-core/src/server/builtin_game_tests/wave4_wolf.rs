use pumpkin_gametest::{GameTestHelper, GameTestResult};
use pumpkin_util::math::position::BlockPos;

use super::build_stone_floor;

pub(super) async fn test_wolf_sits_then_leaps_at_sheep(
    helper: GameTestHelper,
) -> GameTestResult<()> {
    build_stone_floor(&helper, 9, 9).await?;

    let sheep_pos = BlockPos::new(4, 1, 4);
    let sitting_wolf_pos = BlockPos::new(4, 1, 6);
    summon(
        &helper,
        &sheep_pos,
        "minecraft:sheep",
        "\"wave4_sheep\"",
        "",
    )
    .await?;
    summon(
        &helper,
        &sitting_wolf_pos,
        "minecraft:wolf",
        "\"wave4_wolf_sit\"",
        "Owner:[I;1,2,3,4],Sitting:1b,",
    )
    .await?;

    let sequence = helper.start_sequence();
    sequence
        .then_execute_after(40, move |helper| async move {
            helper.assert_entity_present("minecraft:sheep").await?;
            helper
                .assert_entity_present_in_box(
                    "minecraft:wolf",
                    BlockPos::new(3, 1, 5),
                    BlockPos::new(5, 2, 7),
                )
                .await?;
            let sitting = helper
                .execute_command(
                    &sitting_wolf_pos,
                    "execute if data entity @e[tag=wave4_wolf_sit,limit=1] {Sitting:1b} run time query gametime",
                )
                .await?;
            helper.assert_true(sitting > 0, "sitting wolf should keep Sitting:1b")?;
            Ok(())
        })
        .then_execute(move |helper| async move {
            helper
                .execute_command(
                    &sitting_wolf_pos,
                    "data modify entity @e[tag=wave4_wolf_sit,limit=1] Sitting set value 0b",
                )
                .await?;
            let standing = helper
                .execute_command(
                    &sitting_wolf_pos,
                    "execute if data entity @e[tag=wave4_wolf_sit,limit=1] {Sitting:0b} run time query gametime",
                )
                .await?;
            helper.assert_true(standing > 0, "Sitting should clear after data modify")?;

            for (index, pos) in [
                BlockPos::new(1, 1, 4),
                BlockPos::new(7, 1, 4),
                BlockPos::new(4, 1, 1),
                BlockPos::new(4, 1, 7),
            ]
            .into_iter()
            .enumerate()
            {
                summon(
                    &helper,
                    &pos,
                    "minecraft:wolf",
                    &format!("\"wave4_wolf_pack\",\"wave4_wolf_pack_{index}\""),
                    "Owner:[I;1,2,3,4],Sitting:0b,",
                )
                .await?;
            }
            for index in 0..4 {
                helper
                    .execute_command(
                        &sitting_wolf_pos,
                        &format!(
                            "damage @e[tag=wave4_wolf_pack_{index},limit=1] 1 minecraft:player_attack by @e[tag=wave4_sheep,limit=1]"
                        ),
                    )
                    .await?;
            }
            Ok(())
        })
        .then_wait_until(move |helper| async move {
            // Vanilla wolves leap at the target when the random leap check
            // succeeds; assert they reach the sheep rather than sampling a
            // narrow airborne box on one tick.
            helper
                .assert_entity_present_in_box(
                    "minecraft:wolf",
                    BlockPos::new(2, 1, 2),
                    BlockPos::new(6, 3, 6),
                )
                .await
        })
        .then_wait_until(move |helper| async move {
            helper.assert_entity_not_present("minecraft:sheep").await
        })
        .then_succeed();
    Ok(())
}

async fn summon(
    helper: &GameTestHelper,
    relative: &BlockPos,
    entity: &str,
    tags: &str,
    extra_nbt: &str,
) -> GameTestResult<()> {
    let absolute = helper.absolute_pos(relative);
    helper
        .execute_command(
            relative,
            &format!(
                "summon {entity} {} {} {} {{{extra_nbt}Tags:[{tags}],PersistenceRequired:1b}}",
                absolute.0.x, absolute.0.y, absolute.0.z
            ),
        )
        .await?;
    Ok(())
}
