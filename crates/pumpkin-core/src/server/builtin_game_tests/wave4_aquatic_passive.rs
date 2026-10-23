use pumpkin_data::Block;
use pumpkin_data::block_properties::WheatLikeProperties;
use pumpkin_gametest::{GameTestHelper, GameTestResult};
use pumpkin_util::math::position::BlockPos;

use super::build_stone_floor;

/// Vanilla `Fox.FoxPounceGoal`: the fox stalks a chicken, crouches until
/// `isFullyCrouched` and then leaps, so the prey dies to the pounce hit.
pub(super) async fn test_fox_pounces_at_chicken(helper: GameTestHelper) -> GameTestResult<()> {
    build_stone_floor(&helper, 9, 9).await?;

    let fox_pos = BlockPos::new(1, 1, 1);
    let chicken_pos = BlockPos::new(7, 1, 7);
    summon(&helper, &fox_pos, "minecraft:fox", "\"wave4_fox\"", "").await?;
    // One point of health so a single pounce hit is enough.
    summon(
        &helper,
        &chicken_pos,
        "minecraft:chicken",
        "\"wave4_fox_prey\"",
        "Health:1.0f,",
    )
    .await?;

    helper.assert_entity_present("minecraft:fox").await?;
    helper.assert_entity_present("minecraft:chicken").await?;

    helper.succeed_when(move |helper| async move {
        helper.assert_entity_present("minecraft:fox").await?;
        helper.assert_entity_not_present("minecraft:chicken").await
    })?;
    Ok(())
}

/// Vanilla `Frog.TongueGoal`: the frog targets a size-1 slime, pulls it in and
/// kills it with the tongue attack.
pub(super) async fn test_frog_tongue_eats_small_slime(
    helper: GameTestHelper,
) -> GameTestResult<()> {
    build_stone_floor(&helper, 9, 9).await?;

    let frog_pos = BlockPos::new(2, 1, 2);
    let slime_pos = BlockPos::new(5, 1, 2);
    summon(&helper, &frog_pos, "minecraft:frog", "\"wave4_frog\"", "").await?;
    // Slime `Size` is the internal size minus one, so `Size:0` is a size-1 slime,
    // the only size `Frog.canEat` accepts.
    summon(
        &helper,
        &slime_pos,
        "minecraft:slime",
        "\"wave4_frog_prey\"",
        "Size:0,",
    )
    .await?;

    helper.assert_entity_present("minecraft:frog").await?;
    helper.assert_entity_present("minecraft:slime").await?;

    helper.succeed_when(move |helper| async move {
        helper.assert_entity_present("minecraft:frog").await?;
        helper.assert_entity_not_present("minecraft:slime").await
    })?;
    Ok(())
}

/// Vanilla `Rabbit.RaidGardenGoal`: a rabbit with no `MoreCarrotTicks` cooldown
/// walks to mature carrots and eats one age level off the crop.
pub(super) async fn test_rabbit_raids_mature_carrots(helper: GameTestHelper) -> GameTestResult<()> {
    build_stone_floor(&helper, 9, 9).await?;

    let farmland_pos = BlockPos::new(2, 1, 2);
    let carrots_pos = BlockPos::new(2, 2, 2);
    helper
        .set_block_default(&farmland_pos, &Block::FARMLAND)
        .await?;
    let mut carrots = WheatLikeProperties::from_state_id(Block::CARROTS.default_state.id);
    carrots.age = 7;
    helper
        .set_block(&carrots_pos, carrots.to_state_id(&Block::CARROTS))
        .await?;

    let rabbit_pos = BlockPos::new(5, 1, 2);
    summon(
        &helper,
        &rabbit_pos,
        "minecraft:rabbit",
        "\"wave4_rabbit\"",
        "",
    )
    .await?;
    helper.assert_entity_present("minecraft:rabbit").await?;

    let mut eaten = WheatLikeProperties::from_state_id(Block::CARROTS.default_state.id);
    eaten.age = 6;
    let eaten_state = eaten.to_state_id(&Block::CARROTS);
    helper.succeed_when(move |helper| async move {
        helper.assert_block_state(&carrots_pos, eaten_state).await
    })?;
    Ok(())
}

/// Vanilla `Strider.floatStrider`: the strider stands on the 0.5-high lava
/// surface and keeps floating there instead of sinking or suffocating.
pub(super) async fn test_strider_walks_on_lava(helper: GameTestHelper) -> GameTestResult<()> {
    build_stone_floor(&helper, 5, 5).await?;

    // Stone rim keeps the lava source block from flowing out of the test bed.
    for x in 0..5 {
        helper
            .set_block_default(&BlockPos::new(x, 1, 0), &Block::STONE)
            .await?;
        helper
            .set_block_default(&BlockPos::new(x, 1, 4), &Block::STONE)
            .await?;
    }
    for z in 0..5 {
        helper
            .set_block_default(&BlockPos::new(0, 1, z), &Block::STONE)
            .await?;
        helper
            .set_block_default(&BlockPos::new(4, 1, z), &Block::STONE)
            .await?;
    }
    for x in 1..4 {
        for z in 1..4 {
            helper
                .set_block_default(&BlockPos::new(x, 1, z), &Block::LAVA)
                .await?;
        }
    }

    let strider_pos = BlockPos::new(2, 1, 2);
    summon(
        &helper,
        &strider_pos,
        "minecraft:strider",
        "\"wave4_strider\"",
        "",
    )
    .await?;
    helper.assert_entity_present("minecraft:strider").await?;

    helper
        .start_sequence()
        .then_idle(40)
        .then_execute(move |helper| async move {
            // Feet at y = 1.5 over the lava means a block 0.75 above is air;
            // a strider that sank a full block would still be inside lava.
            let above_surface = helper
                .execute_command(
                    &strider_pos,
                    "execute at @e[tag=wave4_strider,limit=1] if block ~ ~0.75 ~ minecraft:air run time query gametime",
                )
                .await?;
            helper.assert_true(
                above_surface > 0,
                "strider sank into the lava instead of floating on its surface",
            )?;
            Ok(())
        })
        .then_succeed();
    Ok(())
}

/// Vanilla `Turtle.TurtleLayEggGoal`: a turtle that carries an egg walks to a
/// sand block near its home and lays the egg, clearing `has_egg`.
pub(super) async fn test_turtle_lays_egg_on_sand(helper: GameTestHelper) -> GameTestResult<()> {
    build_stone_floor(&helper, 9, 9).await?;
    for x in 0..9 {
        for z in 0..9 {
            helper
                .set_block_default(&BlockPos::new(x, 1, z), &Block::SAND)
                .await?;
        }
    }

    let turtle_pos = BlockPos::new(4, 2, 4);
    summon(
        &helper,
        &turtle_pos,
        "minecraft:turtle",
        "\"wave4_turtle\"",
        "has_egg:1b,",
    )
    .await?;
    helper.assert_entity_present("minecraft:turtle").await?;

    helper.succeed_when(move |helper| async move {
        let laid = helper
            .execute_command(
                &turtle_pos,
                "execute if data entity @e[tag=wave4_turtle,limit=1] {has_egg:0b} run time query gametime",
            )
            .await?;
        helper.assert_true(laid > 0, "turtle never laid its egg")
    })?;
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
