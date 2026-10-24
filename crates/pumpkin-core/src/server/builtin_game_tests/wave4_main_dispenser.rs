use pumpkin_data::{
    Block,
    block_properties::{DispenserLikeProperties, Facing, WheatLikeProperties},
};
use pumpkin_gametest::{GameTestHelper, GameTestResult};
use pumpkin_util::math::position::BlockPos;

use super::build_stone_floor;

async fn place_dispenser(
    helper: &GameTestHelper,
    pos: &BlockPos,
    facing: Facing,
) -> GameTestResult<BlockPos> {
    let mut props = DispenserLikeProperties::default(&Block::DISPENSER);
    props.facing = facing;
    helper
        .set_block(pos, props.to_state_id(&Block::DISPENSER))
        .await?;
    Ok(pos.add(0, 1, 0))
}

pub(super) async fn test_dispenser_bone_meal_grows_wheat(
    helper: GameTestHelper,
) -> GameTestResult<()> {
    build_stone_floor(&helper, 5, 5).await?;

    let dispenser = BlockPos::new(1, 1, 2);
    let power = place_dispenser(&helper, &dispenser, Facing::East).await?;
    let farmland = BlockPos::new(2, 0, 2);
    let wheat = BlockPos::new(2, 1, 2);

    helper
        .set_block_default(&farmland, &Block::FARMLAND)
        .await?;
    let wheat_props = WheatLikeProperties { age: 0 };
    helper
        .set_block(&wheat, wheat_props.to_state_id(&Block::WHEAT))
        .await?;
    helper
        .set_container_item(&dispenser, 0, "minecraft:bone_meal", 1)
        .await?;
    helper.pulse_redstone(&power, 4).await?;

    helper
        .start_sequence()
        .then_idle(10)
        .then_execute(move |helper| async move {
            let state = helper.block_state_id(&wheat).await;
            let age = WheatLikeProperties::from_state_id(state).age;
            helper.assert_true(age > 0, "bone meal did not advance the wheat age")?;
            helper.assert_container_empty(&dispenser).await
        })
        .then_succeed();
    Ok(())
}

pub(super) async fn test_dispenser_chest_equips_donkey(
    helper: GameTestHelper,
) -> GameTestResult<()> {
    build_stone_floor(&helper, 5, 5).await?;

    let dispenser = BlockPos::new(1, 1, 2);
    let power = place_dispenser(&helper, &dispenser, Facing::East).await?;
    let donkey = BlockPos::new(2, 1, 2);

    helper
        .execute_command(
            &donkey,
            "summon minecraft:donkey ~ ~ ~ {Tame:1b,ChestedHorse:0b}",
        )
        .await?;
    helper
        .set_container_item(&dispenser, 0, "minecraft:chest", 1)
        .await?;
    helper.pulse_redstone(&power, 4).await?;

    helper
        .start_sequence()
        .then_idle(10)
        .then_execute(move |helper| async move {
            helper
                .assert_container_empty(&dispenser)
                .await?;
            let chested = helper
                .execute_command(
                    &donkey,
                    "execute if data entity @e[type=minecraft:donkey,sort=nearest,limit=1] {ChestedHorse:1b} run time query gametime",
                )
                .await?;
            helper.assert_true(chested > 0, "dispenser did not equip the tamed donkey with a chest")
        })
        .then_succeed();
    Ok(())
}

pub(super) async fn test_dispenser_cod_bucket_places_cod(
    helper: GameTestHelper,
) -> GameTestResult<()> {
    build_stone_floor(&helper, 5, 5).await?;

    let dispenser = BlockPos::new(1, 1, 2);
    let power = place_dispenser(&helper, &dispenser, Facing::East).await?;
    let water = BlockPos::new(2, 1, 2);
    helper.set_block_default(&water, &Block::WATER).await?;
    helper
        .set_container_item(&dispenser, 0, "minecraft:cod_bucket", 1)
        .await?;
    helper.pulse_redstone(&power, 4).await?;

    helper
        .start_sequence()
        .then_idle(10)
        .then_execute(move |helper| async move {
            helper.assert_entity_present("minecraft:cod").await?;
            helper
                .assert_container_contains_count(&dispenser, "minecraft:bucket", 1)
                .await
        })
        .then_succeed();
    Ok(())
}

pub(super) async fn test_dispenser_brush_scrapes_armadillo(
    helper: GameTestHelper,
) -> GameTestResult<()> {
    build_stone_floor(&helper, 5, 5).await?;

    let dispenser = BlockPos::new(1, 1, 2);
    let power = place_dispenser(&helper, &dispenser, Facing::East).await?;
    let armadillo = BlockPos::new(2, 1, 2);
    for pos in [
        BlockPos::new(2, 1, 1),
        BlockPos::new(2, 1, 3),
        BlockPos::new(3, 1, 2),
    ] {
        helper.set_block_default(&pos, &Block::BARRIER).await?;
    }
    helper
        .spawn_entity("minecraft:armadillo", &armadillo)
        .await?;
    helper
        .set_container_item(&dispenser, 0, "minecraft:brush", 1)
        .await?;
    helper.pulse_redstone(&power, 4).await?;

    helper
        .start_sequence()
        .then_idle(10)
        .then_execute(move |helper| async move {
            helper
                .assert_item_entity_present("minecraft:armadillo_scute", &armadillo, 2.0)
                .await
        })
        .then_succeed();
    Ok(())
}
