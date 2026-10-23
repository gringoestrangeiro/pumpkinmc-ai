use pumpkin_data::Block;
use pumpkin_gametest::{GameTestHelper, GameTestResult};
use pumpkin_util::math::position::BlockPos;

use super::build_stone_floor;

pub(super) async fn test_guardian_beam_damages_squid(helper: GameTestHelper) -> GameTestResult<()> {
    build_stone_floor(&helper, 9, 5).await?;

    for x in 1..=7 {
        for y in 1..=2 {
            for z in 1..=3 {
                helper
                    .set_block_default(&BlockPos::new(x, y, z), &Block::WATER)
                    .await?;
            }
        }
    }
    // Walls keep the water in the pool, a guardian that swims out of the water
    // does not attack.
    for y in 1..=2 {
        for x in 0..=8 {
            helper
                .set_block_default(&BlockPos::new(x, y, 0), &Block::STONE)
                .await?;
            helper
                .set_block_default(&BlockPos::new(x, y, 4), &Block::STONE)
                .await?;
        }
        for z in 0..=4 {
            helper
                .set_block_default(&BlockPos::new(0, y, z), &Block::STONE)
                .await?;
            helper
                .set_block_default(&BlockPos::new(8, y, z), &Block::STONE)
                .await?;
        }
    }

    let guardian_pos = BlockPos::new(1, 1, 2);
    // The guardian beam reaches six blocks, the squid is closer so a guardian
    // that drifts does not miss it.
    let squid_pos = BlockPos::new(5, 1, 2);
    helper
        .execute_command(
            &guardian_pos,
            "summon minecraft:guardian ~ ~ ~ {PersistenceRequired:1b}",
        )
        .await?;
    helper
        .execute_command(
            &squid_pos,
            "summon minecraft:squid ~ ~ ~ {NoAI:1b,Silent:1b,PersistenceRequired:1b}",
        )
        .await?;
    helper.assert_entity_present("minecraft:guardian").await?;
    helper.assert_entity_present("minecraft:squid").await?;

    helper
        .start_sequence()
        .then_wait_until(move |helper| async move {
            // Vanilla `GuardianAttackGoal`: 1 indirect magic damage plus a
            // melee hit reduced by the shared hurt cooldown. The exact final
            // health depends on whether the melee hit lands that tick, so
            // assert only that the squid lost health.
            let full_health = helper
                .execute_command(
                    &squid_pos,
                    "execute if entity @e[type=minecraft:squid,nbt={Health:10.0f}] run time query gametime",
                )
                .await?;
            let squid_alive = helper
                .execute_command(
                    &squid_pos,
                    "execute if entity @e[type=minecraft:squid] run time query gametime",
                )
                .await?;
            helper.assert_true(
                full_health == 0,
                format!(
                    "guardian beam did not damage the squid, squid alive: {squid_alive}"
                ),
            )
        })
        .then_succeed();
    Ok(())
}

pub(super) async fn test_warden_sonic_boom_damages_target(
    helper: GameTestHelper,
) -> GameTestResult<()> {
    build_stone_floor(&helper, 16, 5).await?;

    let warden_pos = BlockPos::new(2, 1, 2);
    // The sonic boom reaches fifteen blocks, the golem is closer so a warden
    // that walks away does not miss it.
    let golem_pos = BlockPos::new(8, 1, 2);
    helper
        .execute_command(
            &golem_pos,
            "summon minecraft:iron_golem ~ ~ ~ {NoAI:1b,Silent:1b,PersistenceRequired:1b,UUID:[I;1,2,3,4]}",
        )
        .await?;
    helper
        .execute_command(
            &warden_pos,
            "summon minecraft:warden ~ ~ ~ {PersistenceRequired:1b,anger:{suspects:[{uuid:[I;1,2,3,4],anger:150}]}}",
        )
        .await?;
    helper.assert_entity_present("minecraft:warden").await?;
    helper.assert_entity_present("minecraft:iron_golem").await?;

    helper
        .start_sequence()
        .then_wait_until(move |helper| async move {
            // The iron golem heals again, so check as soon as the hit lands.
            let full_health = helper
                .execute_command(
                    &golem_pos,
                    "execute if entity @e[type=minecraft:iron_golem,nbt={Health:100.0f}] run time query gametime",
                )
                .await?;
            helper.assert_true(
                full_health == 0,
                "warden sonic boom did not damage the iron golem",
            )
        })
        .then_succeed();
    Ok(())
}
