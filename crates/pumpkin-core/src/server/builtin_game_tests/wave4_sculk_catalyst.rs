use pumpkin_data::{Block, block_properties::SculkCatalystLikeProperties};
use pumpkin_gametest::{GameTestHelper, GameTestResult};
use pumpkin_util::math::position::BlockPos;

use super::build_stone_floor;

pub(super) async fn test_sculk_catalyst_bloom_and_spread(
    helper: GameTestHelper,
) -> GameTestResult<()> {
    build_stone_floor(&helper, 5, 5).await?;

    let catalyst_pos = BlockPos::new(1, 1, 2);
    let death_pos = BlockPos::new(3, 1, 2);
    helper
        .set_block_default(&catalyst_pos, &Block::SCULK_CATALYST)
        .await?;

    let absolute = helper.absolute_pos(&death_pos);
    helper
        .execute_command(
            &death_pos,
            &format!(
                "summon minecraft:zombie {} {} {} {{Tags:[\"wave4_catalyst_zombie\"],PersistenceRequired:1b}}",
                absolute.0.x, absolute.0.y, absolute.0.z
            ),
        )
        .await?;
    let killed = helper
        .execute_command(&death_pos, "kill @e[tag=wave4_catalyst_zombie,limit=1]")
        .await?;
    helper.assert_true(killed > 0, "expected the summoned zombie to be killed")?;

    let Some(nbt) = helper.block_entity_nbt(&catalyst_pos).await? else {
        return Err(helper.fail("sculk catalyst block entity is missing"));
    };
    let charge = nbt
        .get_list("cursors")
        .and_then(|cursors| cursors.first())
        .and_then(|cursor| cursor.extract_compound())
        .and_then(|cursor| cursor.get_int("charge"));
    helper.assert_true(
        charge == Some(5),
        format!("expected an adult zombie to add 5 cursor charge, found {charge:?}"),
    )?;

    let mut bloom = SculkCatalystLikeProperties::default(&Block::SCULK_CATALYST);
    bloom.bloom = true;
    helper
        .assert_block_state(&catalyst_pos, bloom.to_state_id(&Block::SCULK_CATALYST))
        .await?;

    helper.succeed_when(move |helper| async move {
        let mut clear = SculkCatalystLikeProperties::default(&Block::SCULK_CATALYST);
        clear.bloom = false;
        helper
            .assert_block_state(&catalyst_pos, clear.to_state_id(&Block::SCULK_CATALYST))
            .await?;

        let mut spread = false;
        let mut floor_converted = false;
        for x in 0..5 {
            for z in 0..5 {
                let block =
                    Block::from_state_id(helper.block_state_id(&BlockPos::new(x, 1, z)).await);
                if block == &Block::SCULK_VEIN || block == &Block::SCULK {
                    spread = true;
                }
                let floor =
                    Block::from_state_id(helper.block_state_id(&BlockPos::new(x, 0, z)).await);
                if floor == &Block::SCULK {
                    floor_converted = true;
                }
            }
        }
        helper.assert_true(spread, "expected sculk spread around the catalyst")?;
        helper.assert_true(
            floor_converted,
            "expected the stone floor to be converted to sculk",
        )
    })?;
    Ok(())
}
