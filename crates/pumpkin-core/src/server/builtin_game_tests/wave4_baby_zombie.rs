use pumpkin_data::Block;
use pumpkin_gametest::{GameTestHelper, GameTestResult};
use pumpkin_nbt::tag::NbtTag;
use pumpkin_util::math::position::BlockPos;

use super::build_stone_floor;

pub(super) async fn test_baby_zombie_speed_and_experience_reward(
    helper: GameTestHelper,
) -> GameTestResult<()> {
    build_stone_floor(&helper, 16, 5).await?;

    let adult_catalyst = BlockPos::new(1, 1, 2);
    let baby_catalyst = BlockPos::new(13, 1, 2);
    helper
        .set_block_default(&adult_catalyst, &Block::SCULK_CATALYST)
        .await?;
    helper
        .set_block_default(&baby_catalyst, &Block::SCULK_CATALYST)
        .await?;

    let adult_pos = BlockPos::new(2, 1, 2);
    let baby_pos = BlockPos::new(14, 1, 2);
    let adult_absolute = helper.absolute_pos(&adult_pos);
    let baby_absolute = helper.absolute_pos(&baby_pos);
    helper
        .execute_command(
            &adult_pos,
            &format!(
                "summon minecraft:zombie {} {} {} {{Tags:[\"wave4_adult_zombie\"],PersistenceRequired:1b}}",
                adult_absolute.0.x, adult_absolute.0.y, adult_absolute.0.z
            ),
        )
        .await?;
    helper
        .execute_command(
            &baby_pos,
            &format!(
                "summon minecraft:zombie {} {} {} {{IsBaby:1b,Tags:[\"wave4_baby_zombie\"],PersistenceRequired:1b}}",
                baby_absolute.0.x, baby_absolute.0.y, baby_absolute.0.z
            ),
        )
        .await?;

    let baby_flag = helper
        .execute_command(
            &baby_pos,
            "execute if data entity @e[tag=wave4_baby_zombie,limit=1] {IsBaby:1b} run time query gametime",
        )
        .await?;
    helper.assert_true(baby_flag > 0, "baby zombie should carry IsBaby:1b")?;

    let adult_speed = helper
        .execute_command(
            &adult_pos,
            "attribute @e[tag=wave4_adult_zombie,limit=1] minecraft:movement_speed get 100",
        )
        .await?;
    let baby_speed = helper
        .execute_command(
            &baby_pos,
            "attribute @e[tag=wave4_baby_zombie,limit=1] minecraft:movement_speed get 100",
        )
        .await?;
    helper.assert_true(
        adult_speed == 23,
        format!("expected adult zombie movement speed 23 at scale 100, found {adult_speed}"),
    )?;
    helper.assert_true(
        baby_speed == 34,
        format!("expected baby zombie movement speed 34 at scale 100, found {baby_speed}"),
    )?;

    helper
        .execute_command(&adult_pos, "kill @e[tag=wave4_adult_zombie,limit=1]")
        .await?;
    helper
        .execute_command(&baby_pos, "kill @e[tag=wave4_baby_zombie,limit=1]")
        .await?;

    let adult_charge = cursor_charge(&helper, &adult_catalyst).await?;
    helper.assert_true(
        adult_charge == Some(5),
        format!("expected an adult zombie to add 5 cursor charge, found {adult_charge:?}"),
    )?;
    let baby_charge = cursor_charge(&helper, &baby_catalyst).await?;
    helper.assert_true(
        baby_charge == Some(12),
        format!("expected a baby zombie to add 12 cursor charge, found {baby_charge:?}"),
    )?;
    helper.succeed();
    Ok(())
}

async fn cursor_charge(
    helper: &GameTestHelper,
    catalyst_pos: &BlockPos,
) -> GameTestResult<Option<i32>> {
    let Some(nbt) = helper.block_entity_nbt(catalyst_pos).await? else {
        return Err(helper.fail("sculk catalyst block entity is missing"));
    };
    Ok(nbt
        .get_list("cursors")
        .and_then(|cursors| cursors.first())
        .and_then(NbtTag::extract_compound)
        .and_then(|cursor| cursor.get_int("charge")))
}
