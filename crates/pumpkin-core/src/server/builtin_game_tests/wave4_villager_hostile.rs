use pumpkin_data::{
    Block,
    block_properties::{BedPart, HorizontalFacing, WhiteBedLikeProperties},
};
use pumpkin_gametest::{GameTestHelper, GameTestResult};
use pumpkin_util::math::position::BlockPos;

use super::build_stone_floor;

// LIVE_TEST A24: villager food pickup, breeding, bell meeting and bed sleep.
// LIVE_TEST A25: wither primary head, Vex owner target, spear use, hoglin repellent.

pub(super) async fn test_villager_picks_up_food(helper: GameTestHelper) -> GameTestResult<()> {
    build_stone_floor(&helper, 5, 5).await?;
    summon(
        &helper,
        &BlockPos::new(2, 1, 2),
        "minecraft:villager",
        "\"wave4_villager_food\"",
        "",
    )
    .await?;
    helper
        .spawn_item("minecraft:bread", 3, &BlockPos::new(2, 1, 1))
        .await?;

    helper.succeed_when(move |helper| async move {
        helper.assert_entity_present("minecraft:villager").await?;
        helper.assert_entity_not_present("minecraft:item").await
    })?;
    Ok(())
}

/// A24: villagers breed while idle, gather at the bell, and sleep at night.
///
/// These behaviors depend on the global day time, so they run in one test:
/// separate tests set conflicting times while running in parallel.
pub(super) async fn test_villager_breeding_bell_and_bed(
    helper: GameTestHelper,
) -> GameTestResult<()> {
    build_stone_floor(&helper, 16, 4).await?;
    // Walls keep the wandering villagers inside the structure bounds.
    for y in 1..=2 {
        for x in 0..16 {
            helper
                .set_block_default(&BlockPos::new(x, y, 3), &Block::STONE)
                .await?;
        }
        for z in 0..4 {
            helper
                .set_block_default(&BlockPos::new(0, y, z), &Block::STONE)
                .await?;
            helper
                .set_block_default(&BlockPos::new(15, y, z), &Block::STONE)
                .await?;
        }
    }
    for x in [2, 6, 9, 12] {
        place_bed(&helper, &BlockPos::new(x, 1, 2)).await?;
    }
    helper
        .set_block_default(&BlockPos::new(14, 1, 2), &Block::BELL)
        .await?;

    for (index, pos) in [BlockPos::new(3, 1, 1), BlockPos::new(5, 1, 1)]
        .into_iter()
        .enumerate()
    {
        summon(
            &helper,
            &pos,
            "minecraft:villager",
            &format!("\"wave4_villager_breed_{index}\""),
            "FoodLevel:12,",
        )
        .await?;
    }
    summon(
        &helper,
        &BlockPos::new(1, 1, 1),
        "minecraft:villager",
        "\"wave4_villager_schedule\"",
        "",
    )
    .await?;

    // The idle window of the villager schedule runs from 10 to 2000 ticks, the
    // breeding has to fit into it.
    helper
        .execute_command(&BlockPos::new(0, 0, 0), "time set 100")
        .await?;

    let mut expected_bed = WhiteBedLikeProperties::default(&Block::WHITE_BED);
    expected_bed.facing = HorizontalFacing::North;
    expected_bed.part = BedPart::Head;
    expected_bed.occupied = true;
    let expected_bed = expected_bed.to_state_id(&Block::WHITE_BED);

    helper
        .start_sequence()
        .then_wait_until(move |helper| async move {
            // Two fed parents, the schedule villager and the baby.
            helper
                .assert_entities_present("minecraft:villager", 4)
                .await
        })
        .then_execute(move |helper| async move {
            helper
                .execute_command(&BlockPos::new(0, 0, 0), "time set 13000")
                .await?;
            Ok(())
        })
        .then_wait_until(move |helper| async move {
            let mut occupied = false;
            for x in [2, 6, 9, 12] {
                if helper.block_state_id(&BlockPos::new(x, 1, 1)).await == expected_bed {
                    occupied = true;
                    break;
                }
            }
            helper.assert_true(occupied, "no villager occupied a bed at night")
        })
        .then_execute(move |helper| async move {
            helper
                .execute_command(&BlockPos::new(0, 0, 0), "time set 9500")
                .await?;
            Ok(())
        })
        .then_wait_until(move |helper| async move {
            // The meet goal stops six blocks short of the bell. The command
            // position has to be absolute, `positioned` does not resolve
            // relative to the test structure.
            let bell = helper.absolute_pos(&BlockPos::new(14, 1, 2));
            let command = format!(
                "execute positioned {} {} {} if entity @e[type=minecraft:villager,distance=..7] run time query gametime",
                bell.0.x, bell.0.y, bell.0.z
            );
            let at_bell = helper
                .execute_command(&BlockPos::new(14, 1, 2), &command)
                .await?;
            helper.assert_true(at_bell > 0, "the villager did not gather at the bell")
        })
        .then_succeed();
    Ok(())
}

pub(super) async fn test_wither_primary_head_targets_living(
    helper: GameTestHelper,
) -> GameTestResult<()> {
    build_stone_floor(&helper, 9, 5).await?;

    summon(
        &helper,
        &BlockPos::new(1, 1, 2),
        "minecraft:wither",
        "\"wave4_wither\"",
        "Invul:0,",
    )
    .await?;
    summon(
        &helper,
        &BlockPos::new(7, 1, 2),
        "minecraft:chicken",
        "\"wave4_wither_target\"",
        "",
    )
    .await?;

    helper.succeed_when(move |helper| async move {
        helper.assert_entity_not_present("minecraft:chicken").await
    })?;
    Ok(())
}

pub(super) async fn test_vex_copies_owner_target(helper: GameTestHelper) -> GameTestResult<()> {
    build_stone_floor(&helper, 6, 4).await?;
    // A glass box keeps the zombie next to its target but out of melee range, so
    // only the vex can damage the villager.
    for pos in [
        BlockPos::new(0, 1, 2),
        BlockPos::new(2, 1, 2),
        BlockPos::new(1, 1, 1),
        BlockPos::new(1, 1, 3),
    ] {
        helper.set_block_default(&pos, &Block::GLASS).await?;
    }
    summon(
        &helper,
        &BlockPos::new(1, 1, 2),
        "minecraft:zombie",
        "\"wave4_vex_owner\"",
        "UUID:[I;1987654321,1234567890,555555555,777777777],",
    )
    .await?;
    summon(
        &helper,
        &BlockPos::new(5, 1, 2),
        "minecraft:villager",
        "\"wave4_vex_target\"",
        "NoAI:1b,",
    )
    .await?;
    summon(
        &helper,
        &BlockPos::new(3, 2, 2),
        "minecraft:vex",
        "\"wave4_vex\"",
        "owner:[I;1987654321,1234567890,555555555,777777777],",
    )
    .await?;

    helper.succeed_when(move |helper| async move {
        let full_health = helper
            .execute_command(
                &BlockPos::new(5, 1, 2),
                "execute if entity @e[tag=wave4_vex_target,nbt={Health:20.0f}] run time query gametime",
            )
            .await?;
        helper.assert_true(
            full_health == 0,
            "vex did not copy its owner's target and damage the villager",
        )
    })?;
    Ok(())
}

pub(super) async fn test_piglin_uses_golden_spear(helper: GameTestHelper) -> GameTestResult<()> {
    build_stone_floor(&helper, 9, 5).await?;

    summon(
        &helper,
        &BlockPos::new(2, 1, 2),
        "minecraft:piglin",
        "\"wave4_piglin_spear\"",
        "IsImmuneToZombification:1b,",
    )
    .await?;
    summon(
        &helper,
        &BlockPos::new(6, 1, 2),
        "minecraft:villager",
        "\"wave4_piglin_spear_target\"",
        "NoAI:1b,",
    )
    .await?;

    helper
        .execute_command(
            &BlockPos::new(2, 1, 2),
            "item replace entity @e[tag=wave4_piglin_spear,limit=1] weapon.mainhand with minecraft:golden_spear",
        )
        .await?;
    helper
        .execute_command(
            &BlockPos::new(2, 1, 2),
            "damage @e[tag=wave4_piglin_spear,limit=1] 1 minecraft:player_attack by @e[tag=wave4_piglin_spear_target,limit=1]",
        )
        .await?;

    helper.succeed_when(move |helper| async move {
        let full_health = helper
            .execute_command(
                &BlockPos::new(6, 1, 2),
                "execute if entity @e[tag=wave4_piglin_spear_target,nbt={Health:20.0f}] run time query gametime",
            )
            .await?;
        helper.assert_true(
            full_health == 0,
            "piglin did not use the golden spear on the villager",
        )
    })?;
    Ok(())
}

pub(super) async fn test_zombified_piglin_uses_golden_spear(
    helper: GameTestHelper,
) -> GameTestResult<()> {
    build_stone_floor(&helper, 9, 5).await?;

    summon(
        &helper,
        &BlockPos::new(2, 1, 2),
        "minecraft:zombified_piglin",
        "\"wave4_zpiglin_spear\"",
        "",
    )
    .await?;
    summon(
        &helper,
        &BlockPos::new(6, 1, 2),
        "minecraft:villager",
        "\"wave4_zpiglin_spear_target\"",
        "NoAI:1b,",
    )
    .await?;

    helper
        .execute_command(
            &BlockPos::new(2, 1, 2),
            "item replace entity @e[tag=wave4_zpiglin_spear,limit=1] weapon.mainhand with minecraft:golden_spear",
        )
        .await?;
    helper
        .execute_command(
            &BlockPos::new(2, 1, 2),
            "damage @e[tag=wave4_zpiglin_spear,limit=1] 1 minecraft:player_attack by @e[tag=wave4_zpiglin_spear_target,limit=1]",
        )
        .await?;

    helper.succeed_when(move |helper| async move {
        let full_health = helper
            .execute_command(
                &BlockPos::new(6, 1, 2),
                "execute if entity @e[tag=wave4_zpiglin_spear_target,nbt={Health:20.0f}] run time query gametime",
            )
            .await?;
        helper.assert_true(
            full_health == 0,
            "zombified piglin did not use the golden spear on the villager",
        )
    })?;
    Ok(())
}

pub(super) async fn test_hoglin_avoids_warped_fungus(helper: GameTestHelper) -> GameTestResult<()> {
    build_stone_floor(&helper, 16, 4).await?;
    helper
        .set_block_default(&BlockPos::new(1, 1, 2), &Block::WARPED_NYLIUM)
        .await?;
    helper
        .set_block_default(&BlockPos::new(1, 2, 2), &Block::WARPED_FUNGUS)
        .await?;

    summon(
        &helper,
        &BlockPos::new(4, 1, 2),
        "minecraft:hoglin",
        "\"wave4_hoglin_repellent\"",
        "",
    )
    .await?;

    let fungus = helper.absolute_pos(&BlockPos::new(1, 2, 2));
    helper.succeed_when(move |helper| async move {
        let retreated = helper
            .execute_command(
                &BlockPos::new(0, 0, 0),
                &format!(
                    "execute positioned {} {} {} if entity @e[type=minecraft:hoglin,distance=6..] run time query gametime",
                    fungus.0.x, fungus.0.y, fungus.0.z
                ),
            )
            .await?;
        helper.assert_true(
            retreated > 0,
            "hoglin should walk away from the warped fungus",
        )?;
        Ok(())
    })?;
    Ok(())
}

/// Places a bed the way the game does: the foot block goes down and the bed
/// places its head one block north of it.
async fn place_bed(helper: &GameTestHelper, foot: &BlockPos) -> GameTestResult<()> {
    let mut foot_state = WhiteBedLikeProperties::default(&Block::WHITE_BED);
    foot_state.facing = HorizontalFacing::North;
    foot_state.part = BedPart::Foot;
    helper
        .set_block(foot, foot_state.to_state_id(&Block::WHITE_BED))
        .await?;
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
