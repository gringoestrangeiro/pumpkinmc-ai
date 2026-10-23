use std::{
    sync::Arc,
    time::{Duration, Instant},
};

use pumpkin_core::{entity::EntityBase, server::Server};
use pumpkin_data::{dimension::Dimension, entity::EntityType, item::Item, item_stack::ItemStack};
use pumpkin_protocol::{
    codec::var_int::VarInt,
    java::server::play::{ActionType, SInteract},
};
use pumpkin_util::math::{position::BlockPos, vector3::Vector3};
use uuid::Uuid;

use crate::bot::HeadlessBot;
use crate::bot_tests::setup::prepare_spot;

/// A happy ghast takes a harness from a player and then carries that player.
pub async fn run_happy_ghast_tests(
    server: &Arc<Server>,
    bot: &mut HeadlessBot,
    summary_lines: &mut Vec<String>,
) -> Result<(), String> {
    let start = Instant::now();
    let world = server.get_world_from_dimension(&Dimension::OVERWORLD);

    let Some(player) = world.get_player_by_name(&bot.name) else {
        return Err(format!("bot player '{}' disappeared", bot.name));
    };
    bot.sync_client_state(server).await?;
    bot.send_command("gamemode survival").await?;
    if !wait_for_gamemode(server, &player, pumpkin_util::GameMode::Survival).await? {
        return Err("the bot did not switch to survival mode".to_string());
    }

    let spot = prepare_spot(server, &world, &player.living_entity.entity.pos.load()).await;
    let ghast = spawn_happy_ghast(&world, &spot)?;

    // The bot hands the harness to the ghast.
    give_item(&player, &Item::BROWN_HARNESS, 1);
    bot.set_held_item(0).await?;
    bot.sync_client_state(server).await?;
    bot.send_packet(&SInteract {
        entity_id: VarInt(ghast.get_entity().entity_id),
        r#type: VarInt(ActionType::Interact as i32),
        target_position: None,
        hand: Some(VarInt(0)),
        sneaking: false,
    })
    .await?;
    for _ in 0..40 {
        tokio::time::sleep(Duration::from_millis(5)).await;
        server.tick();
    }

    let wearing_harness = ghast
        .clone()
        .cast_any()
        .downcast_ref::<pumpkin_core::entity::passive::happy_ghast::HappyGhastEntity>()
        .is_some_and(
            pumpkin_core::entity::passive::happy_ghast::HappyGhastEntity::is_wearing_harness,
        );
    if !wearing_harness {
        return Err("the happy ghast did not take the harness".to_string());
    }

    // With the harness on, the next interaction makes the bot ride the ghast.
    give_item(&player, &Item::AIR, 0);
    bot.set_held_item(0).await?;
    bot.sync_client_state(server).await?;
    bot.send_packet(&SInteract {
        entity_id: VarInt(ghast.get_entity().entity_id),
        r#type: VarInt(ActionType::Interact as i32),
        target_position: None,
        hand: Some(VarInt(0)),
        sneaking: false,
    })
    .await?;

    let mut riding = false;
    for _ in 0..200 {
        tokio::time::sleep(Duration::from_millis(5)).await;
        server.tick();
        let vehicle = player.get_entity().get_vehicle();
        if vehicle
            .is_some_and(|vehicle| vehicle.get_entity().entity_id == ghast.get_entity().entity_id)
        {
            riding = true;
            break;
        }
    }
    if !riding {
        return Err("the bot did not ride the happy ghast".to_string());
    }

    // The bot rides the ghast, so it has to leave the vehicle first.
    bot.send_command("ride @s dismount").await?;
    for _ in 0..20 {
        tokio::time::sleep(Duration::from_millis(5)).await;
        server.tick();
    }
    ghast.get_entity().remove();
    // Leave the bot as the later tests expect it.
    bot.send_command("gamemode creative").await?;
    if !wait_for_gamemode(server, &player, pumpkin_util::GameMode::Creative).await? {
        return Err("the bot did not switch back to creative mode".to_string());
    }
    summary_lines.push(format!(
        "[PASS] bot:happy_ghast_takes_harness_and_carries_player ({}ms)",
        start.elapsed().as_millis()
    ));
    Ok(())
}

/// Waits until the bot has the given gamemode.
async fn wait_for_gamemode(
    server: &Arc<Server>,
    player: &Arc<pumpkin_core::entity::player::Player>,
    gamemode: pumpkin_util::GameMode,
) -> Result<bool, String> {
    for _ in 0..100 {
        if player.gamemode.load() == gamemode {
            return Ok(true);
        }
        tokio::time::sleep(Duration::from_millis(5)).await;
        server.tick();
    }
    Ok(player.gamemode.load() == gamemode)
}

/// Puts an item into the first hotbar slot of the player.
fn give_item(player: &Arc<pumpkin_core::entity::player::Player>, item: &'static Item, count: u8) {
    let inventory = player.inventory();
    let mut main_inventory = inventory
        .main_inventory
        .write()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    main_inventory[0] = ItemStack::new(count, item);
}

fn spawn_happy_ghast(
    world: &Arc<pumpkin_core::world::World>,
    spot: &BlockPos,
) -> Result<Arc<dyn EntityBase>, String> {
    let entity_type = EntityType::from_name("happy_ghast")
        .ok_or_else(|| "unknown entity type 'happy_ghast'".to_string())?;
    let entity = pumpkin_core::entity::r#type::from_type(
        entity_type,
        Vector3::new(
            f64::from(spot.0.x) + 0.5,
            f64::from(spot.0.y) + 1.0,
            f64::from(spot.0.z) + 0.5,
        ),
        world,
        Uuid::new_v4(),
    );
    world.spawn_entity(entity.clone());
    Ok(entity)
}
