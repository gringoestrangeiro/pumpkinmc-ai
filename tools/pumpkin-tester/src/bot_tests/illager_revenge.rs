use std::{
    sync::Arc,
    time::{Duration, Instant},
};

use pumpkin_core::{entity::EntityBase, server::Server};
use pumpkin_data::{dimension::Dimension, entity::EntityType};
use pumpkin_protocol::{
    codec::var_int::VarInt,
    java::server::play::{ActionType, SInteract},
};
use pumpkin_util::{
    GameMode,
    math::{position::BlockPos, vector3::Vector3},
};

use crate::bot::HeadlessBot;
use crate::bot_tests::setup::prepare_spot;

/// A bot that attacks an evoker makes the evoker retaliate against it.
pub async fn run_illager_revenge_tests(
    server: &Arc<Server>,
    bot: &mut HeadlessBot,
    summary_lines: &mut Vec<String>,
) -> Result<(), String> {
    let start = Instant::now();
    let world = server.get_world_from_dimension(&Dimension::OVERWORLD);

    let Some(player) = world.get_player_by_name(&bot.name) else {
        return Err(format!("bot player '{}' disappeared", bot.name));
    };
    // A creative player is not a valid target, and an earlier test switches the
    // bot to creative.
    bot.sync_client_state(server).await?;
    bot.send_command("gamemode survival").await?;
    for _ in 0..100 {
        if player.gamemode.load() == GameMode::Survival {
            break;
        }
        bot.send_command("gamemode survival").await?;
        tokio::time::sleep(Duration::from_millis(5)).await;
        server.tick();
    }
    if player.gamemode.load() != GameMode::Survival {
        return Err("the bot did not switch to survival mode".to_string());
    }

    let bot_pos = player.living_entity.entity.pos.load();
    let spot = prepare_spot(server, &world, &bot_pos).await;
    let evoker = spawn_evoker(&world, &spot)?;

    // The bot attacks for real, so the evoker records the bot as its attacker.
    let attack = || SInteract {
        entity_id: VarInt(evoker.get_entity().entity_id),
        r#type: VarInt(ActionType::Attack as i32),
        target_position: None,
        hand: Some(VarInt(0)),
        sneaking: false,
    };
    bot.sync_client_state(server).await?;
    bot.send_packet(&attack()).await?;

    // The evoker has to take the bot as its target, the fangs it shoots can
    // miss, so the target is what the test checks.
    let bot_id = player.get_entity().entity_id;
    let mut retaliated = false;
    for attempt in 0..600 {
        tokio::time::sleep(Duration::from_millis(5)).await;
        server.tick();
        // The evoker flies around, so it is put back in reach of the bot.
        let evoker_pos = evoker.get_entity().pos.load();
        if (evoker_pos.x - bot_pos.x).abs() > 3.0 || (evoker_pos.z - bot_pos.z).abs() > 3.0 {
            evoker.get_entity().teleport(
                Vector3::new(
                    f64::from(spot.0.x) + 0.5,
                    f64::from(spot.0.y),
                    f64::from(spot.0.z) + 0.5,
                ),
                None,
                None,
                &world,
            );
        }
        let targets_the_bot = evoker
            .get_mob()
            .and_then(|mob| mob.get_mob_entity().get_target())
            .is_some_and(|target| target.get_entity().entity_id == bot_id);
        if targets_the_bot || player.living_entity.health.load() < 20.0 {
            retaliated = true;
            break;
        }
        // The attack packet can be dropped while the server is busy, so the bot
        // hits the evoker again until a hit lands. Repeating the hit after that
        // kills the evoker before it retaliates.
        if attempt % 20 == 19
            && evoker
                .get_living_entity()
                .is_some_and(|living| living.health.load() >= living.get_max_health())
        {
            bot.sync_client_state(server).await?;
            bot.send_packet(&attack()).await?;
        }
    }
    if !retaliated {
        return Err(format!(
            "the evoker did not retaliate against the bot, evoker health {}, evoker alive {}, bot health {}, evoker at {:?}, bot at {:?}",
            evoker
                .get_mob()
                .map_or(0.0, |mob| mob.get_mob_entity().living_entity.health.load()),
            evoker.get_entity().is_alive(),
            player.living_entity.health.load(),
            evoker.get_entity().pos.load(),
            player.living_entity.entity.pos.load()
        ));
    }

    // Leave the bot as the later tests expect it: creative mode and full
    // health, and no evoker left shooting at it.
    evoker.get_entity().remove();
    player.living_entity.set_health(20.0);
    bot.send_command("gamemode creative").await?;
    for _ in 0..100 {
        if player.gamemode.load() == GameMode::Creative {
            break;
        }
        tokio::time::sleep(Duration::from_millis(5)).await;
        server.tick();
    }
    if player.gamemode.load() != GameMode::Creative {
        return Err("the bot did not switch back to creative mode".to_string());
    }

    summary_lines.push(format!(
        "[PASS] bot:illager_retaliates_against_attacker ({}ms)",
        start.elapsed().as_millis()
    ));
    Ok(())
}

fn spawn_evoker(
    world: &Arc<pumpkin_core::world::World>,
    spot: &BlockPos,
) -> Result<Arc<dyn EntityBase>, String> {
    let entity_type = EntityType::from_name("evoker")
        .ok_or_else(|| "unknown entity type 'evoker'".to_string())?;
    let position = Vector3::new(
        f64::from(spot.0.x) + 0.5,
        f64::from(spot.0.y),
        f64::from(spot.0.z) + 0.5,
    );
    let entity =
        pumpkin_core::entity::r#type::from_type(entity_type, position, world, uuid::Uuid::new_v4());
    if let Some(mob) = entity.get_mob() {
        mob.get_mob_entity()
            .persistence_required
            .store(true, std::sync::atomic::Ordering::Relaxed);
    }
    world.spawn_entity(entity.clone());
    Ok(entity)
}
