use std::{
    sync::Arc,
    time::{Duration, Instant},
};

use pumpkin_core::{entity::EntityBase, server::Server, world::BlockFlags, world::World};
use pumpkin_data::{Block, dimension::Dimension, entity::EntityType};
use pumpkin_protocol::java::server::play::{SMoveVehicle, SPaddleBoat};
use pumpkin_util::math::{boundingbox::BoundingBox, position::BlockPos, vector3::Vector3};

use crate::bot::HeadlessBot;

/// Half size of the water pool, the outer ring is the wall.
const POOL_RADIUS: i32 = 2;

pub async fn run_vehicle_tests(
    server: &Arc<Server>,
    bot: &mut HeadlessBot,
    summary_lines: &mut Vec<String>,
) -> Result<(), String> {
    let start = Instant::now();
    let world = server.get_world_from_dimension(&Dimension::OVERWORLD);

    let base_x = bot.position.x.floor() as i32;
    let base_y = bot.position.y.floor() as i32;
    let base_z = bot.position.z.floor() as i32;
    // The pool has to stay inside the bot's loaded chunk: the pool blocks are
    // written before the boat is summoned, and an unloaded chunk discards
    // them, which leaves the boat without water under it. The pool is clamped
    // into the chunk, so the bot stays close enough to reach the boat.
    let pool_x = base_x.clamp(
        (base_x / 16) * 16 + POOL_RADIUS,
        (base_x / 16) * 16 + 15 - POOL_RADIUS,
    );
    let pool_z = base_z.clamp(
        (base_z / 16) * 16 + POOL_RADIUS,
        (base_z / 16) * 16 + 15 - POOL_RADIUS,
    );
    let center = Vector3::new(
        f64::from(pool_x) + 0.5,
        f64::from(base_y - 1),
        f64::from(pool_z) + 0.5,
    );

    build_pool(&world, pool_x, base_y, pool_z);

    let boat = summon_boat(server, bot, &world, &center, base_y).await?;
    let player = mount_boat(server, bot, &world, &boat).await?;
    paddle_boat(server, bot, &world, &boat, &player, &center, base_y).await?;
    dismount_boat(server, bot, &player).await?;

    summary_lines.push(format!(
        "[PASS] bot:boat_ride_paddle_submerge_dismount ({}ms)",
        start.elapsed().as_millis()
    ));
    Ok(())
}

async fn summon_boat(
    server: &Arc<Server>,
    bot: &mut HeadlessBot,
    world: &Arc<World>,
    center: &Vector3<f64>,
    base_y: i32,
) -> Result<Arc<dyn EntityBase>, String> {
    bot.send_command(&format!(
        "summon minecraft:oak_boat {} {} {}",
        center.x,
        f64::from(base_y),
        center.z
    ))
    .await?;
    wait_ticks(server, 10).await;

    let area = BoundingBox {
        min: Vector3::new(center.x - 3.0, center.y - 2.0, center.z - 3.0),
        max: Vector3::new(center.x + 3.0, center.y + 4.0, center.z + 3.0),
    };
    world
        .get_entities_at_box(&area)
        .into_iter()
        .find(|entity| entity.get_entity().entity_type == &EntityType::OAK_BOAT)
        .ok_or_else(|| "summoned oak boat was not found in the pool".to_string())
}

async fn mount_boat(
    server: &Arc<Server>,
    bot: &mut HeadlessBot,
    world: &Arc<World>,
    boat: &Arc<dyn EntityBase>,
) -> Result<Arc<pumpkin_core::entity::player::Player>, String> {
    bot.send_command("ride @s mount @e[type=minecraft:oak_boat,limit=1]")
        .await?;
    wait_ticks(server, 10).await;

    let player = world
        .get_player_by_name(&bot.name)
        .ok_or_else(|| format!("bot player '{}' disappeared", bot.name))?;
    settle_teleports(server, bot, &player).await?;
    if !player.get_entity().has_vehicle() {
        return Err("bot did not mount the oak boat after /ride".to_string());
    }
    let mounted = player
        .get_entity()
        .get_vehicle()
        .ok_or_else(|| "bot vehicle disappeared after mount".to_string())?;
    if mounted.get_entity().entity_id != boat.get_entity().entity_id {
        return Err("bot mounted a different boat than the summoned one".to_string());
    }
    if let Some(pending) = player
        .awaiting_teleport
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .as_ref()
    {
        return Err(format!("bot has a pending teleport: {pending:?}"));
    }
    if !player.has_client_loaded() {
        return Err("bot player is not marked client-loaded".to_string());
    }
    if boat
        .get_controlling_passenger()
        .is_none_or(|passenger| passenger.get_entity().entity_id != player.get_entity().entity_id)
    {
        return Err("bot is not the controlling passenger of the boat".to_string());
    }
    Ok(player)
}

async fn paddle_boat(
    server: &Arc<Server>,
    bot: &mut HeadlessBot,
    world: &Arc<World>,
    boat: &Arc<dyn EntityBase>,
    player: &Arc<pumpkin_core::entity::player::Player>,
    center: &Vector3<f64>,
    base_y: i32,
) -> Result<(), String> {
    let before = boat.get_entity().pos.load();
    bot.send_packet(&SPaddleBoat {
        left_paddle: true,
        right_paddle: true,
    })
    .await?;
    bot.send_packet(&SMoveVehicle {
        x: before.x + 2.0,
        y: before.y,
        z: before.z,
        yaw: 90.0,
        pitch: 0.0,
        on_ground: false,
    })
    .await?;
    wait_ticks(server, 10).await;

    let after = boat.get_entity().pos.load();
    if after.x - before.x < 1.0 {
        return Err(format!(
            "boat did not move after SMoveVehicle and SPaddleBoat: {before:?} -> {after:?}"
        ));
    }
    let player_pos = player.get_entity().pos.load();
    if player_pos.squared_distance_to_vec(&after) > 4.0 {
        return Err(format!(
            "player did not follow the moving boat: player {player_pos:?}, boat {after:?}"
        ));
    }

    bot.send_packet(&SMoveVehicle {
        x: after.x,
        y: center.y,
        z: after.z,
        yaw: 90.0,
        pitch: 0.0,
        on_ground: false,
    })
    .await?;
    wait_ticks(server, 30).await;
    // The pool holds water from `base_y - 1` to `base_y + 1`, a boat counts as
    // submerged while it sits at or below that surface.
    let pos = boat.get_entity().pos.load();
    if pos.y > f64::from(base_y) + 1.5 {
        let block = world.get_block(&BlockPos::new(
            pos.x.floor() as i32,
            pos.y.floor() as i32,
            pos.z.floor() as i32,
        ));
        return Err(format!(
            "boat did not become submerged: pos={pos:?}, block_id={}, in_water={}, center={center:?}, before={before:?}, after={after:?}, player={:?}",
            block.id,
            boat.get_entity().is_in_water(),
            player.get_entity().pos.load()
        ));
    }
    Ok(())
}

async fn dismount_boat(
    server: &Arc<Server>,
    bot: &mut HeadlessBot,
    player: &Arc<pumpkin_core::entity::player::Player>,
) -> Result<(), String> {
    bot.send_command("ride @s dismount").await?;
    wait_ticks(server, 10).await;
    settle_teleports(server, bot, player).await?;
    if player.get_entity().has_vehicle() {
        return Err("bot still has a vehicle after /ride dismount".to_string());
    }
    Ok(())
}

fn build_pool(world: &Arc<World>, center_x: i32, y: i32, center_z: i32) {
    for x in -POOL_RADIUS..=POOL_RADIUS {
        for z in -POOL_RADIUS..=POOL_RADIUS {
            let pos = BlockPos::new(center_x + x, y - 2, center_z + z);
            world.set_block_state(&pos, Block::STONE.default_state.id, BlockFlags::NOTIFY_ALL);

            let is_wall = x.abs() == POOL_RADIUS || z.abs() == POOL_RADIUS;
            for offset in -1..=1 {
                let pos = BlockPos::new(center_x + x, y + offset, center_z + z);
                let block = if is_wall {
                    &Block::STONE
                } else {
                    &Block::WATER
                };
                world.set_block_state(&pos, block.default_state.id, BlockFlags::NOTIFY_ALL);
            }
        }
    }
}

async fn settle_teleports(
    server: &Arc<Server>,
    bot: &mut HeadlessBot,
    player: &std::sync::Arc<pumpkin_core::entity::player::Player>,
) -> Result<(), String> {
    for _ in 0..40 {
        if player
            .awaiting_teleport
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .is_none()
        {
            return Ok(());
        }
        wait_ticks(server, 2).await;
        bot.process_teleports(64).await?;
    }
    if let Some(pending) = player
        .awaiting_teleport
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .as_ref()
    {
        return Err(format!("bot still has a pending teleport: {pending:?}"));
    }
    Ok(())
}

async fn wait_ticks(server: &Arc<Server>, ticks: usize) {
    for _ in 0..ticks {
        tokio::time::sleep(Duration::from_millis(5)).await;
        server.tick();
    }
}
