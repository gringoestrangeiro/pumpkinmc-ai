use std::{
    sync::Arc,
    time::{Duration, Instant},
};

use pumpkin_core::{entity::EntityBase, server::Server};
use pumpkin_data::packet::CURRENT_MC_VERSION;
use pumpkin_data::{damage::DamageType, dimension::Dimension, entity::EntityType};
use pumpkin_protocol::{
    codec::{lp_vector_3d::LpVector3d, var_int::VarInt},
    java::client::play::CEntityVelocity,
    packet::MultiVersionJavaPacket,
};
use pumpkin_util::{
    GameMode,
    math::{position::BlockPos, vector3::Vector3},
};
use tokio::time::timeout;
use uuid::Uuid;

use crate::bot::HeadlessBot;
use crate::bot_tests::setup::prepare_spot;

/// A player pushed by mobs gets no velocity of its own from the server, and a mob hit
/// sends the vanilla knockback: vanilla only sends a player its own motion after
/// `markHurt`, and the server's copy of that motion decays with gravity like vanilla's.
pub async fn run_knockback_tests(
    server: &Arc<Server>,
    bot: &mut HeadlessBot,
    summary_lines: &mut Vec<String>,
) -> Result<(), String> {
    let start = Instant::now();
    let world = server.get_world_from_dimension(&Dimension::OVERWORLD);
    let Some(player) = world.get_player_by_name(&bot.name) else {
        return Err(format!("bot player '{}' disappeared", bot.name));
    };
    let player_id = player.get_entity().entity_id;

    bot.sync_client_state(server).await?;
    bot.send_command("gamemode survival").await?;
    bot.send_command("difficulty normal").await?;
    for _ in 0..20 {
        tokio::time::sleep(Duration::from_millis(5)).await;
        server.tick();
    }
    if player.gamemode.load() != GameMode::Survival {
        return Err("the bot did not switch to survival mode".to_string());
    }

    let spot = prepare_spot(server, &world, &player.living_entity.entity.pos.load()).await;
    let standing = Vector3::new(
        f64::from(spot.0.x) + 0.5,
        f64::from(spot.0.y),
        f64::from(spot.0.z) + 0.5,
    );
    bot.move_to(standing, true).await?;
    bot.process_teleports(20).await?;
    // Let anything the move caused (a landing, a teleport) settle first.
    for _ in 0..20 {
        tokio::time::sleep(Duration::from_millis(5)).await;
        server.tick();
        drain_own_motion(bot, player_id).await?;
    }

    // Cows standing inside the player push it every tick.
    let health = player.living_entity.health.load();
    let cows: Vec<_> = (0..3)
        .map(|_| spawn(&world, "cow", &spot))
        .collect::<Result<_, _>>()?;
    let mut pushed_motion = Vec::new();
    for tick in 0..40 {
        tokio::time::sleep(Duration::from_millis(5)).await;
        server.tick();
        for motion in drain_own_motion(bot, player_id).await? {
            pushed_motion.push((tick, motion));
        }
    }
    for cow in &cows {
        cow.get_entity().remove();
    }
    if !pushed_motion.is_empty() {
        return Err(format!(
            "pushing mobs sent the player motion about itself: {pushed_motion:?}, health {health} -> {}",
            player.living_entity.health.load()
        ));
    }

    // Let the pushes decay, then take a hit from the side.
    for _ in 0..40 {
        tokio::time::sleep(Duration::from_millis(5)).await;
        server.tick();
    }
    drain_own_motion(bot, player_id).await?;
    let attacker = spawn(&world, "zombie", &spot)?;
    attacker
        .get_entity()
        .set_pos(standing.add(&Vector3::new(1.0, 0.0, 0.0)));
    let target: &dyn EntityBase = player.as_ref();
    let hurt = target.damage_with_context(
        target,
        1.0,
        DamageType::MOB_ATTACK,
        Some(attacker.get_entity().pos.load()),
        Some(attacker.as_ref()),
        Some(attacker.as_ref()),
    );
    attacker.get_entity().remove();
    if !hurt {
        return Err("the player was not hurt by the mob attack".to_string());
    }
    let mut knockback = Vec::new();
    for _ in 0..5 {
        tokio::time::sleep(Duration::from_millis(5)).await;
        server.tick();
        knockback.extend(drain_own_motion(bot, player_id).await?);
    }
    let [motion] = knockback[..] else {
        return Err(format!(
            "expected exactly one knockback packet, got {knockback:?}"
        ));
    };
    // Vanilla knocks back to `min(0.4, dy / 2 + 0.4)` with the server's resting dy of
    // -0.0784, plus 0.4 away from the attacker on x. `sendChanges` only runs on the next
    // tick, after the player's own `travel`, so the sent motion has one step of ground
    // friction (0.546) and gravity applied: about (-0.218, 0.275, 0).
    if !(0.26..0.29).contains(&motion.y) || !(-0.23..-0.21).contains(&motion.x) {
        return Err(format!("unexpected knockback velocity {motion:?}"));
    }

    player.living_entity.set_health(20.0);
    bot.send_command("gamemode creative").await?;
    for _ in 0..20 {
        tokio::time::sleep(Duration::from_millis(5)).await;
        server.tick();
    }

    summary_lines.push(format!(
        "[PASS] bot:mob_push_and_knockback_velocity ({}ms)",
        start.elapsed().as_millis()
    ));
    Ok(())
}

/// Reads every pending packet and returns the motion the server sent about `player_id`.
async fn drain_own_motion(
    bot: &mut HeadlessBot,
    player_id: i32,
) -> Result<Vec<Vector3<f64>>, String> {
    let mut motions = Vec::new();
    while let Ok(packet) = timeout(Duration::from_millis(20), bot.next_packet()).await {
        let packet = packet?;
        if packet.id != CEntityVelocity::to_id(CURRENT_MC_VERSION) {
            continue;
        }
        let mut payload = &packet.payload[..];
        let entity_id = VarInt::decode(&mut payload)
            .map_err(|err| format!("bad motion packet entity id: {err}"))?;
        if entity_id.0 == player_id {
            let velocity = LpVector3d::read(&mut payload)
                .map_err(|err| format!("bad motion packet velocity: {err}"))?;
            motions.push(velocity.0);
        }
    }
    Ok(motions)
}

fn spawn(
    world: &Arc<pumpkin_core::world::World>,
    name: &str,
    spot: &BlockPos,
) -> Result<Arc<dyn EntityBase>, String> {
    let entity_type =
        EntityType::from_name(name).ok_or_else(|| format!("unknown entity type '{name}'"))?;
    let entity = pumpkin_core::entity::r#type::from_type(
        entity_type,
        Vector3::new(
            f64::from(spot.0.x) + 0.5,
            f64::from(spot.0.y),
            f64::from(spot.0.z) + 0.5,
        ),
        world,
        Uuid::new_v4(),
    );
    world.spawn_entity(entity.clone());
    Ok(entity)
}
