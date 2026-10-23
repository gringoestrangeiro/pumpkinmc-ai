use pumpkin_data::{
    Block,
    block_properties::{DispenserLikeProperties, Facing},
};
use pumpkin_gametest::{GameTestHelper, GameTestResult};
use pumpkin_util::math::position::BlockPos;

use super::build_stone_floor;

/// Vanilla `Tadpole.ticksToBeFrog`: the tadpole grows once its age reaches this value.
const TADPOLE_TICKS_TO_BE_FROG: i32 = 24_000;

pub(super) async fn test_experience_bottle_impact_spawns_experience_orbs(
    helper: GameTestHelper,
) -> GameTestResult<()> {
    build_stone_floor(&helper, 5, 5).await?;

    let dispenser_pos = BlockPos::new(1, 1, 2);
    let target_pos = BlockPos::new(3, 1, 2);
    let power_pos = BlockPos::new(1, 2, 2);

    let mut props = DispenserLikeProperties::default(&Block::DISPENSER);
    props.facing = Facing::East;
    helper
        .set_block(&dispenser_pos, props.to_state_id(&Block::DISPENSER))
        .await?;
    // The stone block gives the thrown bottle a face to shatter on.
    helper.set_block_default(&target_pos, &Block::STONE).await?;
    helper
        .set_container_item(&dispenser_pos, 0, "minecraft:experience_bottle", 1)
        .await?;
    helper.pulse_redstone(&power_pos, 4).await?;

    helper.succeed_when(move |helper| async move {
        helper
            .assert_entity_not_present("minecraft:experience_bottle")
            .await?;
        helper.assert_container_empty(&dispenser_pos).await?;
        helper
            .assert_entity_present_in_box(
                "minecraft:experience_orb",
                BlockPos::new(2, 0, 1),
                BlockPos::new(4, 3, 3),
            )
            .await
    })?;
    Ok(())
}

pub(super) async fn test_tadpole_grows_into_frog(helper: GameTestHelper) -> GameTestResult<()> {
    build_stone_floor(&helper, 5, 5).await?;

    let spawn_pos = BlockPos::new(2, 1, 2);
    let absolute = helper.absolute_pos(&spawn_pos);
    // One tick of age is left, so the real `TadpoleEntity` tick path runs `age_up`.
    helper
        .execute_command(
            &spawn_pos,
            &format!(
                "summon minecraft:tadpole {} {} {} {{Age:{},PersistenceRequired:1b}}",
                absolute.0.x,
                absolute.0.y,
                absolute.0.z,
                TADPOLE_TICKS_TO_BE_FROG - 1
            ),
        )
        .await?;

    helper.assert_entity_present("minecraft:tadpole").await?;
    helper.succeed_when(move |helper| async move {
        helper
            .assert_entity_not_present("minecraft:tadpole")
            .await?;
        helper.assert_entity_present("minecraft:frog").await
    })?;
    Ok(())
}
