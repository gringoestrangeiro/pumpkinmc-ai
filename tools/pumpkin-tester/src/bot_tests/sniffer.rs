use std::{
    sync::Arc,
    time::{Duration, Instant},
};

use pumpkin_core::{entity::EntityBase, server::Server, world::BlockFlags};
use pumpkin_data::{Block, dimension::Dimension, entity::EntityType};
use pumpkin_util::math::{boundingbox::BoundingBox, position::BlockPos, vector3::Vector3};
use uuid::Uuid;

use crate::bot::HeadlessBot;

/// A sniffer digs up a seed from the ground next to it.
pub async fn run_sniffer_dig_tests(
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
    let bot_pos = player.living_entity.entity.pos.load();

    // The sniffer looks for a diggable block up to eighteen blocks away, so the
    // ground around it has to be diggable as well.
    let center = BlockPos::new(
        bot_pos.x.floor() as i32,
        bot_pos.y.floor() as i32 - 1,
        bot_pos.z.floor() as i32,
    );
    for x in -20..=20 {
        for z in -20..=20 {
            world.set_block_state(
                &BlockPos::new(center.0.x + x, center.0.y, center.0.z + z),
                Block::SUSPICIOUS_SAND.default_state.id,
                BlockFlags::NOTIFY_ALL,
            );
        }
    }

    let sniffer = spawn_sniffer(&world, &center)?;
    let dig_area = BoundingBox::from_block(&center).expand(24.0, 8.0, 24.0);

    let mut dug = false;
    for _ in 0..600 {
        tokio::time::sleep(Duration::from_millis(5)).await;
        server.tick();
        let dropped = world
            .get_entities_at_box(&dig_area)
            .into_iter()
            .any(|entity| entity.get_entity().entity_type == &EntityType::ITEM);
        if dropped {
            dug = true;
            break;
        }
    }
    if !dug {
        return Err("the sniffer did not dig up a seed".to_string());
    }

    sniffer.get_entity().remove();
    summary_lines.push(format!(
        "[PASS] bot:sniffer_digs_up_a_seed ({}ms)",
        start.elapsed().as_millis()
    ));
    Ok(())
}

fn spawn_sniffer(
    world: &Arc<pumpkin_core::world::World>,
    center: &BlockPos,
) -> Result<Arc<dyn EntityBase>, String> {
    let entity_type = EntityType::from_name("sniffer")
        .ok_or_else(|| "unknown entity type 'sniffer'".to_string())?;
    let entity = pumpkin_core::entity::r#type::from_type(
        entity_type,
        Vector3::new(
            f64::from(center.0.x) + 0.5,
            f64::from(center.0.y) + 1.0,
            f64::from(center.0.z) + 0.5,
        ),
        world,
        Uuid::new_v4(),
    );
    world.spawn_entity(entity.clone());
    Ok(entity)
}
