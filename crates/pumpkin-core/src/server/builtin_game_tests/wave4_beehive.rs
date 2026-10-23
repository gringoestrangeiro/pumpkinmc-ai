use pumpkin_data::{Block, block_properties::BeeNestLikeProperties};
use pumpkin_gametest::{GameTestHelper, GameTestResult};
use pumpkin_nbt::{compound::NbtCompound, tag::NbtTag};
use pumpkin_util::math::position::BlockPos;

use super::build_stone_floor;

pub(super) async fn test_beehive_emergency_release(helper: GameTestHelper) -> GameTestResult<()> {
    build_stone_floor(&helper, 5, 5).await?;

    let hive_pos = BlockPos::new(2, 1, 2);
    let mut bee = NbtCompound::new();
    bee.put_string("id", "minecraft:bee".to_string());
    bee.put_bool("HasNectar", false);
    let mut occupant = NbtCompound::new();
    occupant.put_compound("entity_data", bee);
    occupant.put_int("ticks_in_hive", 0);
    occupant.put_int("min_ticks_in_hive", 600);

    let mut hive_nbt = NbtCompound::new();
    hive_nbt.put_string("id", "minecraft:beehive".to_string());
    hive_nbt.put_list("bees", vec![NbtTag::Compound(occupant); 3]);
    helper
        .set_block_with_nbt(&hive_pos, Block::BEEHIVE.default_state.id, &hive_nbt)
        .await?;

    let initial_bees = helper
        .block_entity_nbt(&hive_pos)
        .await?
        .and_then(|nbt| nbt.get_list("bees").map(<[_]>::len));
    helper.assert_true(
        initial_bees == Some(3),
        format!("expected 3 stored bees, found {initial_bees:?}"),
    )?;

    // Vanilla `BeehiveBlockEntity.serverTick` empties the hive when fire is next to it.
    helper
        .set_block_default(&BlockPos::new(2, 1, 3), &Block::FIRE)
        .await?;

    helper.succeed_when(move |helper| async move {
        let stored = helper
            .block_entity_nbt(&hive_pos)
            .await?
            .and_then(|nbt| nbt.get_list("bees").map(<[_]>::len));
        helper.assert_true(
            stored == Some(0),
            format!("expected the hive to be empty after emergency release, found {stored:?}"),
        )?;
        helper.assert_entities_present("minecraft:bee", 3).await
    })?;
    Ok(())
}

/// A bee that stored its nectar in the hive fills the hive with honey when it
/// is released again.
pub(super) async fn test_beehive_honey_rises(helper: GameTestHelper) -> GameTestResult<()> {
    build_stone_floor(&helper, 7, 7).await?;

    // Bees stay in the hive while it rains and at night, the hive has to be
    // free of both for the bee to leave with its nectar.
    helper
        .execute_command(&BlockPos::new(0, 0, 0), "weather clear")
        .await?;
    helper
        .execute_command(&BlockPos::new(0, 0, 0), "time set 6000")
        .await?;

    let hive_pos = BlockPos::new(2, 1, 2);
    let flower_pos = BlockPos::new(4, 1, 4);
    helper
        .set_block_default(&flower_pos, &Block::DANDELION)
        .await?;

    let mut bee = NbtCompound::new();
    bee.put_string("id", "minecraft:bee".to_string());
    bee.put_bool("HasNectar", true);
    bee.put(
        "flower_pos",
        NbtTag::IntArray(vec![flower_pos.0.x, flower_pos.0.y, flower_pos.0.z]),
    );
    let mut occupant = NbtCompound::new();
    occupant.put_compound("entity_data", bee);
    occupant.put_int("ticks_in_hive", 600);
    occupant.put_int("min_ticks_in_hive", 600);

    let mut hive_nbt = NbtCompound::new();
    hive_nbt.put_string("id", "minecraft:beehive".to_string());
    hive_nbt.put_list("bees", vec![NbtTag::Compound(occupant)]);
    helper
        .set_block_with_nbt(&hive_pos, Block::BEEHIVE.default_state.id, &hive_nbt)
        .await?;

    helper.succeed_when(move |helper| async move {
        let state = helper.block_state_id(&hive_pos).await;
        let honey_level = BeeNestLikeProperties::from_state_id(state).honey_level;
        helper.assert_true(
            honey_level > 0,
            format!("the hive did not fill with honey, honey level: {honey_level}"),
        )
    })?;
    Ok(())
}
