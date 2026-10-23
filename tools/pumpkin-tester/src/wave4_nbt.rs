use std::sync::{Arc, PoisonError};

use pumpkin_core::{
    block::entities::{BlockEntity, beehive::BeehiveBlockEntity},
    entity::{
        EntityBase,
        passive::{
            allay::AllayEntity, bee::BeeEntity, donkey::DonkeyEntity, llama::LlamaEntity,
            mule::MuleEntity, trader_llama::TraderLlamaEntity,
        },
        r#type::from_type,
    },
    server::Server,
    world::{BlockFlags, World},
};
use pumpkin_data::{
    Block,
    block_properties::BeeNestLikeProperties,
    data_component_impl::{DataComponentImpl, ProfileImpl, ProfileProperty},
    dimension::Dimension,
    entity::{EntityPose, EntityType},
    item::Item,
    item_stack::ItemStack,
};
use pumpkin_inventory::inventory::{Inventory, SimpleInventory};
use pumpkin_nbt::{compound::NbtCompound, tag::NbtTag};
use pumpkin_util::{
    math::{position::BlockPos, vector2::Vector2, vector3::Vector3},
    text::TextComponent,
};
use uuid::Uuid;

const TEST_X: i32 = 320;
const TEST_Z: i32 = 320;
const TEST_Y: i32 = 100;

const HORSE_INVENTORY_SLOTS: usize = 15;
const ALLAY_ITEM_COUNT: u8 = 3;

/// Vanilla `Mannequin` default model-part mask (`0x7f`) without head and body (`0x05`).
const MANNEQUIN_HIDDEN_LAYERS: i8 = 0x7a;
const MANNEQUIN_DESCRIPTION: &str = "Wave 4 mannequin";

pub async fn run_wave4_nbt_tests(server: &Arc<Server>) -> Result<Vec<String>, String> {
    let world = server.get_world_from_dimension(&Dimension::OVERWORLD);
    force_load_test_chunk(&world);
    world
        .level
        .get_or_fetch_chunk(Vector2::new(TEST_X >> 4, TEST_Z >> 4), |_| ())
        .await;

    let mut lines = Vec::new();
    let mut failures = Vec::new();

    record(
        &mut lines,
        &mut failures,
        "A21a_allay_inventory",
        test_allay_inventory(server),
    );
    record(
        &mut lines,
        &mut failures,
        "A21b_bee_hive_and_flower_positions",
        test_bee_positions(server),
    );
    record(
        &mut lines,
        &mut failures,
        "A21c_beehive_honey_level_and_bees",
        test_beehive(server),
    );
    record(
        &mut lines,
        &mut failures,
        "A22a_donkey_chest_inventory",
        test_donkey_inventory(server),
    );
    record(
        &mut lines,
        &mut failures,
        "A22b_mule_chest_inventory",
        test_mule_inventory(server),
    );
    record(
        &mut lines,
        &mut failures,
        "A22c_llama_chest_inventory",
        test_llama_inventory(server),
    );
    record(
        &mut lines,
        &mut failures,
        "A22d_trader_llama_chest_inventory",
        test_trader_llama_inventory(server),
    );
    record(
        &mut lines,
        &mut failures,
        "A23_mannequin_metadata",
        test_mannequin(server),
    );

    if failures.is_empty() {
        Ok(lines)
    } else {
        Err(format!(
            "Wave 4 NBT persistence checks failed ({}):\n{}",
            failures.join(", "),
            lines.join("\n")
        ))
    }
}

fn record(
    lines: &mut Vec<String>,
    failures: &mut Vec<String>,
    name: &str,
    result: Result<(), String>,
) {
    match result {
        Ok(()) => lines.push(format!("[PASS] nbt:{name}")),
        Err(err) => {
            lines.push(format!("[FAIL] nbt:{name}: {err}"));
            failures.push(name.to_owned());
        }
    }
}

fn force_load_test_chunk(world: &World) {
    let chunk = Vector2::new(TEST_X >> 4, TEST_Z >> 4);
    let mut forced = world
        .forced_chunks
        .lock()
        .unwrap_or_else(PoisonError::into_inner);
    let mut chunk_loading = world
        .level
        .chunk_loading
        .lock()
        .unwrap_or_else(PoisonError::into_inner);
    forced.insert(chunk);
    chunk_loading.add_force_ticket(chunk);
    chunk_loading.send_change();
}

fn test_position() -> Vector3<f64> {
    Vector3::new(
        f64::from(TEST_X) + 0.5,
        f64::from(TEST_Y),
        f64::from(TEST_Z) + 0.5,
    )
}

fn spawn(entity_type: &'static EntityType, world: &Arc<World>) -> Arc<dyn EntityBase> {
    from_type(entity_type, test_position(), world, Uuid::new_v4())
}

fn round_trip_nbt(
    entity: &Arc<dyn EntityBase>,
    entity_type: &'static EntityType,
    world: &Arc<World>,
) -> Arc<dyn EntityBase> {
    let mut nbt = NbtCompound::new();
    entity.write_nbt(&mut nbt);

    let restored = spawn(entity_type, world);
    restored.read_nbt_non_mut(&nbt);
    restored
}

fn test_allay_inventory(server: &Arc<Server>) -> Result<(), String> {
    let world = server.get_world_from_dimension(&Dimension::OVERWORLD);
    let allay = spawn(&EntityType::ALLAY, &world);
    let allay_entity = allay
        .cast_any()
        .downcast_ref::<AllayEntity>()
        .ok_or_else(|| "from_type(ALLAY) did not return an AllayEntity".to_owned())?;

    let expected = ItemStack::new(ALLAY_ITEM_COUNT, &Item::DIAMOND);
    allay_entity.inventory.set_stack(0, expected.clone());

    let restored = round_trip_nbt(&allay, &EntityType::ALLAY, &world);
    let restored_allay = restored
        .cast_any()
        .downcast_ref::<AllayEntity>()
        .ok_or_else(|| "round-tripped Allay was not an AllayEntity".to_owned())?;

    if restored_allay.inventory.size() != 1 {
        return Err(format!(
            "allay inventory slot count changed: expected 1, got {}",
            restored_allay.inventory.size()
        ));
    }
    let stack = restored_allay.inventory.get_stack(0);
    if stack.item.id != expected.item.id {
        return Err(format!(
            "allay slot 0 item changed: expected {}, got {}",
            expected.item.registry_key, stack.item.registry_key
        ));
    }
    if stack.item_count != ALLAY_ITEM_COUNT {
        return Err(format!(
            "allay slot 0 count changed: expected {ALLAY_ITEM_COUNT}, got {}",
            stack.item_count
        ));
    }
    Ok(())
}

fn test_bee_positions(server: &Arc<Server>) -> Result<(), String> {
    let world = server.get_world_from_dimension(&Dimension::OVERWORLD);
    let bee = spawn(&EntityType::BEE, &world);
    let bee_entity = bee
        .cast_any()
        .downcast_ref::<BeeEntity>()
        .ok_or_else(|| "from_type(BEE) did not return a BeeEntity".to_owned())?;

    let hive_pos = BlockPos::new(TEST_X + 1, TEST_Y + 1, TEST_Z + 2);
    let flower_pos = BlockPos::new(TEST_X - 3, TEST_Y - 1, TEST_Z - 4);
    bee_entity.set_hive_pos(Some(hive_pos));
    bee_entity.set_saved_flower_pos(Some(flower_pos));

    let restored = round_trip_nbt(&bee, &EntityType::BEE, &world);
    let restored_bee = restored
        .cast_any()
        .downcast_ref::<BeeEntity>()
        .ok_or_else(|| "round-tripped Bee was not a BeeEntity".to_owned())?;

    if restored_bee.get_hive_pos() != Some(hive_pos) {
        return Err(format!(
            "bee hive position changed: expected {hive_pos:?}, got {:?}",
            restored_bee.get_hive_pos()
        ));
    }
    if restored_bee.get_saved_flower_pos() != Some(flower_pos) {
        return Err(format!(
            "bee saved flower position changed: expected {flower_pos:?}, got {:?}",
            restored_bee.get_saved_flower_pos()
        ));
    }
    Ok(())
}

fn test_beehive(server: &Arc<Server>) -> Result<(), String> {
    let world = server.get_world_from_dimension(&Dimension::OVERWORLD);
    let hive_pos = BlockPos::new(TEST_X, TEST_Y, TEST_Z);

    let mut properties = BeeNestLikeProperties::default(&Block::BEEHIVE);
    properties.r#honey_level = 4;
    world.set_block_state(
        &hive_pos,
        properties.to_state_id(&Block::BEEHIVE),
        BlockFlags::NOTIFY_ALL,
    );

    let block_entity = world
        .get_block_entity(&hive_pos)
        .ok_or_else(|| format!("placing a beehive at {hive_pos:?} created no block entity"))?;
    let hive = block_entity
        .as_any()
        .downcast_ref::<BeehiveBlockEntity>()
        .ok_or_else(|| "beehive block entity had the wrong type".to_owned())?;

    let flower_pos = BlockPos::new(TEST_X - 2, TEST_Y, TEST_Z - 2);
    let bee = spawn(&EntityType::BEE, &world);
    let bee_entity = bee
        .cast_any()
        .downcast_ref::<BeeEntity>()
        .ok_or_else(|| "from_type(BEE) did not return a BeeEntity".to_owned())?;
    bee_entity.set_has_nectar(true);
    bee_entity.set_saved_flower_pos(Some(flower_pos));
    hive.add_occupant(bee.as_ref(), &world);

    if hive.occupant_count() != 1 {
        return Err(format!(
            "beehive occupant count after add_occupant: expected 1, got {}",
            hive.occupant_count()
        ));
    }

    server.tick();

    let mut hive_nbt = NbtCompound::new();
    hive.write_nbt(&mut hive_nbt);

    world.remove_block_entity(&hive_pos);
    world.add_block_entity(Arc::new(BeehiveBlockEntity::from_nbt(&hive_nbt, hive_pos)));

    let reloaded_block_entity = world
        .get_block_entity(&hive_pos)
        .ok_or_else(|| "beehive block entity did not reload from its written NBT".to_owned())?;
    let reloaded_hive = reloaded_block_entity
        .as_any()
        .downcast_ref::<BeehiveBlockEntity>()
        .ok_or_else(|| "reloaded beehive block entity had the wrong type".to_owned())?;

    if reloaded_hive.occupant_count() != 1 {
        return Err(format!(
            "beehive occupant count after NBT reload: expected 1, got {}",
            reloaded_hive.occupant_count()
        ));
    }

    let bees = reloaded_hive
        .bees
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .clone();
    let occupant = bees
        .as_ref()
        .and_then(|bees| bees.first())
        .and_then(NbtTag::extract_compound)
        .ok_or_else(|| "reloaded beehive bees list had no occupant compound".to_owned())?;
    let entity_data = occupant
        .get_compound("entity_data")
        .ok_or_else(|| "reloaded beehive occupant had no entity_data".to_owned())?;
    if entity_data.get_string("id") != Some("minecraft:bee") {
        return Err(format!(
            "reloaded beehive occupant id changed: expected minecraft:bee, got {:?}",
            entity_data.get_string("id")
        ));
    }
    if entity_data.get_bool("HasNectar") != Some(true) {
        return Err(format!(
            "reloaded beehive occupant nectar flag changed: expected true, got {:?}",
            entity_data.get_bool("HasNectar")
        ));
    }
    if *reloaded_hive
        .flower_pos
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        != Some(flower_pos)
    {
        return Err("beehive flower_pos did not survive the NBT reload".to_owned());
    }

    let state = world.get_block_state(&hive_pos);
    if Block::from_state_id(state.id).id != Block::BEEHIVE.id {
        return Err(format!(
            "beehive block at {hive_pos:?} changed to {}",
            Block::from_state_id(state.id).name
        ));
    }
    let reloaded_properties = BeeNestLikeProperties::from_state_id(state.id);
    if reloaded_properties.r#honey_level != properties.r#honey_level {
        return Err(format!(
            "beehive honey level changed: expected {}, got {}",
            properties.r#honey_level, reloaded_properties.r#honey_level
        ));
    }
    Ok(())
}

fn sample_stack(slot: usize) -> ItemStack {
    let item = match slot % 5 {
        0 => &Item::DIAMOND,
        1 => &Item::EMERALD,
        2 => &Item::GOLD_INGOT,
        3 => &Item::IRON_INGOT,
        _ => &Item::COAL,
    };
    let count = u8::try_from(slot % 60 + 1).unwrap_or(1);
    ItemStack::new(count, item)
}

fn fill_inventory(inventory: &SimpleInventory, slots: usize) {
    for slot in 0..slots {
        inventory.set_stack(slot, sample_stack(slot));
    }
}

fn assert_inventory_survived(
    inventory: &SimpleInventory,
    slots: usize,
    entity_name: &str,
) -> Result<(), String> {
    if inventory.size() != slots {
        return Err(format!(
            "{entity_name} inventory slot count changed: expected {slots}, got {}",
            inventory.size()
        ));
    }
    for slot in 0..slots {
        let expected = sample_stack(slot);
        let actual = inventory.get_stack(slot);
        if actual.item.id != expected.item.id || actual.item_count != expected.item_count {
            return Err(format!(
                "{entity_name} slot {slot} changed: expected {} x{}, got {} x{}",
                expected.item.registry_key,
                expected.item_count,
                actual.item.registry_key,
                actual.item_count
            ));
        }
    }
    Ok(())
}

fn test_donkey_inventory(server: &Arc<Server>) -> Result<(), String> {
    let world = server.get_world_from_dimension(&Dimension::OVERWORLD);
    let donkey = spawn(&EntityType::DONKEY, &world);
    let donkey_entity = donkey
        .cast_any()
        .downcast_ref::<DonkeyEntity>()
        .ok_or_else(|| "from_type(DONKEY) did not return a DonkeyEntity".to_owned())?;
    donkey_entity.set_has_chest(true);
    fill_inventory(&donkey_entity.inventory, HORSE_INVENTORY_SLOTS);

    let restored = round_trip_nbt(&donkey, &EntityType::DONKEY, &world);
    let restored_donkey = restored
        .cast_any()
        .downcast_ref::<DonkeyEntity>()
        .ok_or_else(|| "round-tripped Donkey was not a DonkeyEntity".to_owned())?;

    if !restored_donkey.has_chest() {
        return Err("donkey lost its chest flag in NBT".to_owned());
    }
    if restored_donkey.get_inventory_size() != HORSE_INVENTORY_SLOTS {
        return Err(format!(
            "donkey chest inventory size changed: expected {HORSE_INVENTORY_SLOTS}, got {}",
            restored_donkey.get_inventory_size()
        ));
    }
    assert_inventory_survived(&restored_donkey.inventory, HORSE_INVENTORY_SLOTS, "donkey")
}

fn test_mule_inventory(server: &Arc<Server>) -> Result<(), String> {
    let world = server.get_world_from_dimension(&Dimension::OVERWORLD);
    let mule = spawn(&EntityType::MULE, &world);
    let mule_entity = mule
        .cast_any()
        .downcast_ref::<MuleEntity>()
        .ok_or_else(|| "from_type(MULE) did not return a MuleEntity".to_owned())?;
    mule_entity.set_has_chest(true);
    fill_inventory(&mule_entity.inventory, HORSE_INVENTORY_SLOTS);

    let restored = round_trip_nbt(&mule, &EntityType::MULE, &world);
    let restored_mule = restored
        .cast_any()
        .downcast_ref::<MuleEntity>()
        .ok_or_else(|| "round-tripped Mule was not a MuleEntity".to_owned())?;

    if !restored_mule.has_chest() {
        return Err("mule lost its chest flag in NBT".to_owned());
    }
    if restored_mule.get_inventory_size() != HORSE_INVENTORY_SLOTS {
        return Err(format!(
            "mule chest inventory size changed: expected {HORSE_INVENTORY_SLOTS}, got {}",
            restored_mule.get_inventory_size()
        ));
    }
    assert_inventory_survived(&restored_mule.inventory, HORSE_INVENTORY_SLOTS, "mule")
}

fn test_llama_inventory(server: &Arc<Server>) -> Result<(), String> {
    let world = server.get_world_from_dimension(&Dimension::OVERWORLD);
    let llama = spawn(&EntityType::LLAMA, &world);
    let llama_entity = llama
        .cast_any()
        .downcast_ref::<LlamaEntity>()
        .ok_or_else(|| "from_type(LLAMA) did not return a LlamaEntity".to_owned())?;
    llama_entity.set_strength(5);
    llama_entity.set_has_chest(true);
    fill_inventory(&llama_entity.inventory, HORSE_INVENTORY_SLOTS);

    let restored = round_trip_nbt(&llama, &EntityType::LLAMA, &world);
    let restored_llama = restored
        .cast_any()
        .downcast_ref::<LlamaEntity>()
        .ok_or_else(|| "round-tripped Llama was not a LlamaEntity".to_owned())?;

    if !restored_llama.has_chest() {
        return Err("llama lost its chest flag in NBT".to_owned());
    }
    if restored_llama.get_strength() != 5 {
        return Err(format!(
            "llama strength changed: expected 5, got {}",
            restored_llama.get_strength()
        ));
    }
    if restored_llama.get_inventory_size() != HORSE_INVENTORY_SLOTS {
        return Err(format!(
            "llama chest inventory size changed: expected {HORSE_INVENTORY_SLOTS}, got {}",
            restored_llama.get_inventory_size()
        ));
    }
    assert_inventory_survived(&restored_llama.inventory, HORSE_INVENTORY_SLOTS, "llama")
}

fn test_trader_llama_inventory(server: &Arc<Server>) -> Result<(), String> {
    let world = server.get_world_from_dimension(&Dimension::OVERWORLD);
    let llama = spawn(&EntityType::TRADER_LLAMA, &world);
    let llama_entity = llama
        .cast_any()
        .downcast_ref::<TraderLlamaEntity>()
        .ok_or_else(|| "from_type(TRADER_LLAMA) did not return a TraderLlamaEntity".to_owned())?;
    llama_entity.set_strength(5);
    llama_entity.set_has_chest(true);
    fill_inventory(&llama_entity.inventory, HORSE_INVENTORY_SLOTS);

    let restored = round_trip_nbt(&llama, &EntityType::TRADER_LLAMA, &world);
    let restored_llama = restored
        .cast_any()
        .downcast_ref::<TraderLlamaEntity>()
        .ok_or_else(|| "round-tripped Trader Llama was not a TraderLlamaEntity".to_owned())?;

    if !restored_llama.has_chest() {
        return Err("trader llama lost its chest flag in NBT".to_owned());
    }
    if restored_llama.get_strength() != 5 {
        return Err(format!(
            "trader llama strength changed: expected 5, got {}",
            restored_llama.get_strength()
        ));
    }
    if restored_llama.get_inventory_size() != HORSE_INVENTORY_SLOTS {
        return Err(format!(
            "trader llama chest inventory size changed: expected {HORSE_INVENTORY_SLOTS}, got {}",
            restored_llama.get_inventory_size()
        ));
    }
    assert_inventory_survived(
        &restored_llama.inventory,
        HORSE_INVENTORY_SLOTS,
        "trader llama",
    )
}

fn mannequin_profile() -> ProfileImpl {
    ProfileImpl {
        name: Some("Wave4Mannequin".to_owned()),
        id: Some([12, 34, 56, 78]),
        properties: vec![ProfileProperty {
            name: "textures".to_owned(),
            value: "dGVzdA==".to_owned(),
            signature: Some("c2ln".to_owned()),
        }],
        texture: Some("minecraft:textures/entity/mannequin/test.png".to_owned()),
        cape: Some("minecraft:textures/entity/mannequin/cape.png".to_owned()),
        elytra: Some("minecraft:textures/entity/mannequin/elytra.png".to_owned()),
        model: Some("slim".to_owned()),
    }
}

fn mannequin_input_with_description() -> NbtCompound {
    let mut nbt = NbtCompound::new();
    nbt.put("profile", mannequin_profile().write_data());
    nbt.put_byte("hidden_layers", MANNEQUIN_HIDDEN_LAYERS);
    nbt.put_string("main_hand", "left".to_owned());
    nbt.put_string("pose", "crouching".to_owned());
    nbt.put_bool("immovable", true);
    nbt.put_bool("hide_description", false);
    nbt.put(
        "description",
        NbtTag::Compound(
            TextComponent::text(MANNEQUIN_DESCRIPTION)
                .0
                .to_nbt_compound(),
        ),
    );
    nbt
}

fn mannequin_input_with_hidden_description() -> NbtCompound {
    let mut nbt = NbtCompound::new();
    nbt.put_bool("hide_description", true);
    nbt
}

fn mannequin_round_trip(world: &Arc<World>, input: &NbtCompound) -> NbtCompound {
    let mannequin = spawn(&EntityType::MANNEQUIN, world);
    mannequin.read_nbt_non_mut(input);

    let mut output = NbtCompound::new();
    mannequin.write_nbt(&mut output);
    output
}

fn test_mannequin(server: &Arc<Server>) -> Result<(), String> {
    let world = server.get_world_from_dimension(&Dimension::OVERWORLD);

    let description_input = mannequin_input_with_description();
    let mannequin = spawn(&EntityType::MANNEQUIN, &world);
    mannequin.read_nbt_non_mut(&description_input);

    if mannequin.get_entity().pose.load() != EntityPose::Crouching {
        return Err("mannequin pose after read: expected crouching".to_owned());
    }

    let mut output = NbtCompound::new();
    mannequin.write_nbt(&mut output);

    let expected_profile = mannequin_profile();
    let actual_profile = output
        .get("profile")
        .and_then(ProfileImpl::read_data)
        .ok_or_else(|| "mannequin NBT output had no readable profile".to_owned())?;
    if actual_profile != expected_profile {
        return Err(format!(
            "mannequin profile changed: expected {expected_profile:?}, got {actual_profile:?}"
        ));
    }
    if output.get_byte("hidden_layers") != Some(MANNEQUIN_HIDDEN_LAYERS) {
        return Err(format!(
            "mannequin hidden_layers changed: expected {MANNEQUIN_HIDDEN_LAYERS}, got {:?}",
            output.get_byte("hidden_layers")
        ));
    }
    if output.get_string("main_hand") != Some("left") {
        return Err(format!(
            "mannequin main_hand changed: expected left, got {:?}",
            output.get_string("main_hand")
        ));
    }
    if output.get_string("pose") != Some("crouching") {
        return Err(format!(
            "mannequin pose key changed: expected crouching, got {:?}",
            output.get_string("pose")
        ));
    }
    if output.get_bool("immovable") != Some(true) {
        return Err(format!(
            "mannequin immovable changed: expected true, got {:?}",
            output.get_bool("immovable")
        ));
    }
    if output.get_bool("hide_description") == Some(true) {
        return Err(format!(
            "mannequin hide_description changed: expected false, got {:?}",
            output.get_bool("hide_description")
        ));
    }
    let expected_description = TextComponent::text(MANNEQUIN_DESCRIPTION);
    let actual_description = output
        .get("description")
        .map(TextComponent::from_nbt)
        .ok_or_else(|| "mannequin NBT output had no description".to_owned())?;
    if actual_description != expected_description {
        return Err(format!(
            "mannequin description changed: expected {expected_description:?}, got {actual_description:?}"
        ));
    }

    let hidden_input = mannequin_input_with_hidden_description();
    let hidden_output = mannequin_round_trip(&world, &hidden_input);
    if hidden_output.get_bool("hide_description") != Some(true) {
        return Err(format!(
            "mannequin hide_description did not survive: expected true, got {:?}",
            hidden_output.get_bool("hide_description")
        ));
    }
    if hidden_output.get("description").is_some() {
        return Err("mannequin wrote a description while hide_description was true".to_owned());
    }

    Ok(())
}
