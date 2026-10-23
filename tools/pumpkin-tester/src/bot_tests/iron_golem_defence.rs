use std::{
    sync::Arc,
    time::{Duration, Instant},
};

use pumpkin_core::{entity::EntityBase, server::Server};
use pumpkin_data::{dimension::Dimension, entity::EntityType};
use pumpkin_nbt::{compound::NbtCompound, tag::NbtTag};
use pumpkin_util::{
    GameMode,
    math::{position::BlockPos, vector3::Vector3},
};
use uuid::Uuid;

use crate::bot::HeadlessBot;
use crate::bot_tests::setup::prepare_spot;

/// An iron golem attacks a player that a nearby villager dislikes.
pub async fn run_iron_golem_defence_tests(
    server: &Arc<Server>,
    bot: &mut HeadlessBot,
    summary_lines: &mut Vec<String>,
) -> Result<(), String> {
    let start = Instant::now();
    let world = server.get_world_from_dimension(&Dimension::OVERWORLD);

    let Some(player) = world.get_player_by_name(&bot.name) else {
        return Err(format!("bot player '{}' disappeared", bot.name));
    };
    // A creative player is never a target of the golem, and an earlier test
    // switches the bot to creative.
    bot.sync_client_state(server).await?;
    bot.send_command("gamemode survival").await?;
    if !wait_for_gamemode(server, &player, GameMode::Survival).await? {
        return Err("the bot did not switch to survival mode".to_string());
    }

    let bot_pos = player.living_entity.entity.pos.load();
    let spot = prepare_spot(server, &world, &bot_pos).await;

    // A villager that dislikes the bot: the gossip weight of "major_negative"
    // is -5, so one entry with a value of 100 gives a reputation of -500.
    let villager = spawn_villager(&world, &spot, player.get_entity().entity_uuid)?;
    let golem = spawn_golem(&world, &spot)?;

    let bot_id = player.get_entity().entity_id;
    let mut defended = false;
    for _ in 0..600 {
        tokio::time::sleep(Duration::from_millis(5)).await;
        server.tick();
        let targets_the_bot = golem
            .get_mob()
            .and_then(|mob| mob.get_mob_entity().get_target())
            .is_some_and(|target| target.get_entity().entity_id == bot_id);
        if targets_the_bot || player.living_entity.health.load() < 20.0 {
            defended = true;
            break;
        }
    }
    if !defended {
        return Err(format!(
            "the iron golem did not attack the disliked player, gamemode {:?}, reputation {:?}, gossips {} {:?}, bot {}",
            player.gamemode.load(),
            villager_reputation(&villager, player.get_entity().entity_uuid),
            villager_gossip_count(&villager),
            villager_gossip_targets(&villager),
            player.get_entity().entity_uuid
        ));
    }

    // Leave the bot as the later tests expect it.
    golem.get_entity().remove();
    villager.get_entity().remove();
    player.living_entity.set_health(20.0);
    bot.send_command("gamemode creative").await?;
    if !wait_for_gamemode(server, &player, GameMode::Creative).await? {
        return Err("the bot did not switch back to creative mode".to_string());
    }

    summary_lines.push(format!(
        "[PASS] bot:iron_golem_defends_village_against_disliked_player ({}ms)",
        start.elapsed().as_millis()
    ));
    Ok(())
}

async fn wait_for_gamemode(
    server: &Arc<Server>,
    player: &Arc<pumpkin_core::entity::player::Player>,
    gamemode: GameMode,
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

/// The uuids a villager has gossip about.
fn villager_gossip_targets(villager: &Arc<dyn EntityBase>) -> Vec<String> {
    let villager = villager.clone();
    villager
        .cast_any()
        .downcast_ref::<pumpkin_core::entity::passive::villager::VillagerEntity>()
        .map_or_else(Vec::new, |villager| {
            villager
                .gossips
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .keys()
                .map(ToString::to_string)
                .collect()
        })
}

/// The number of players a villager has gossip about.
fn villager_gossip_count(villager: &Arc<dyn EntityBase>) -> usize {
    let villager = villager.clone();
    villager
        .cast_any()
        .downcast_ref::<pumpkin_core::entity::passive::villager::VillagerEntity>()
        .map_or(0, |villager| {
            villager
                .gossips
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .len()
        })
}

/// The reputation a villager has for a player.
fn villager_reputation(villager: &Arc<dyn EntityBase>, player_uuid: Uuid) -> Option<i32> {
    let villager = villager.clone();
    villager
        .cast_any()
        .downcast_ref::<pumpkin_core::entity::passive::villager::VillagerEntity>()
        .map(|villager| villager.get_player_reputation(player_uuid))
}

fn spawn_villager(
    world: &Arc<pumpkin_core::world::World>,
    spot: &BlockPos,
    player_uuid: Uuid,
) -> Result<Arc<dyn EntityBase>, String> {
    let entity_type = EntityType::from_name("villager")
        .ok_or_else(|| "unknown entity type 'villager'".to_string())?;
    let entity = pumpkin_core::entity::r#type::from_type(
        entity_type,
        spot_position(spot),
        world,
        Uuid::new_v4(),
    );
    let uuid = player_uuid.as_u128();
    let mut gossip = NbtCompound::new();
    gossip.put(
        "Target",
        NbtTag::IntArray(vec![
            (uuid >> 96) as i32,
            (uuid >> 64) as i32,
            (uuid >> 32) as i32,
            uuid as i32,
        ]),
    );
    gossip.put_string("Type", "major_negative".to_string());
    gossip.put_int("Value", 100);
    let mut nbt = NbtCompound::new();
    nbt.put("Gossips", NbtTag::List(vec![NbtTag::Compound(gossip)]));
    entity.read_nbt_non_mut(&nbt);
    world.spawn_entity(entity.clone());
    Ok(entity)
}

fn spawn_golem(
    world: &Arc<pumpkin_core::world::World>,
    spot: &BlockPos,
) -> Result<Arc<dyn EntityBase>, String> {
    let entity_type = EntityType::from_name("iron_golem")
        .ok_or_else(|| "unknown entity type 'iron_golem'".to_string())?;
    let entity = pumpkin_core::entity::r#type::from_type(
        entity_type,
        spot_position(spot),
        world,
        Uuid::new_v4(),
    );
    world.spawn_entity(entity.clone());
    Ok(entity)
}

fn spot_position(spot: &BlockPos) -> Vector3<f64> {
    Vector3::new(
        f64::from(spot.0.x) + 0.5,
        f64::from(spot.0.y),
        f64::from(spot.0.z) + 0.5,
    )
}
