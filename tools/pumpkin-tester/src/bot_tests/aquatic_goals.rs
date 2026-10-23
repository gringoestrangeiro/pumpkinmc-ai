use std::{
    sync::Arc,
    time::{Duration, Instant},
};

use pumpkin_core::{
    entity::{
        EntityBase,
        mob::Mob,
        passive::{axolotl::AxolotlEntity, dolphin::DolphinEntity},
    },
    server::Server,
};
use pumpkin_data::{
    Block, dimension::Dimension, entity::EntityType, item::Item, item_stack::ItemStack,
};
use pumpkin_nbt::compound::NbtCompound;
use pumpkin_protocol::{
    codec::var_int::VarInt,
    java::server::play::{ActionType, SInteract},
};
use pumpkin_util::math::{position::BlockPos, vector3::Vector3};
use uuid::Uuid;

use crate::bot::HeadlessBot;
use crate::bot_tests::setup::{place_block, prepare_spot};

/// A tamed nautilus rides the player that interacts with it.
pub async fn run_nautilus_ride_tests(
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
    let spot = prepare_spot(server, &world, &player.living_entity.entity.pos.load()).await;
    let nautilus = spawn_tamed_nautilus(&world, &spot)?;

    bot.send_packet(&SInteract {
        entity_id: VarInt(nautilus.get_entity().entity_id),
        r#type: VarInt(ActionType::Interact as i32),
        target_position: None,
        hand: Some(VarInt(0)),
        sneaking: false,
    })
    .await?;

    let mut ridden = false;
    for _ in 0..200 {
        tokio::time::sleep(Duration::from_millis(5)).await;
        server.tick();
        let vehicle = player.get_entity().get_vehicle();
        if vehicle.is_some_and(|vehicle| {
            vehicle.get_entity().entity_id == nautilus.get_entity().entity_id
        }) {
            ridden = true;
            break;
        }
    }
    if !ridden {
        return Err("the nautilus did not mount the bot".to_string());
    }

    // Leave the bot as the later tests expect it.
    bot.send_command("ride @s dismount").await?;
    nautilus.get_entity().remove();
    summary_lines.push(format!(
        "[PASS] bot:nautilus_rides_the_player ({}ms)",
        start.elapsed().as_millis()
    ));
    Ok(())
}

/// An axolotl that is out of water runs out of air and takes dry-out damage.
pub async fn run_axolotl_dry_out_tests(
    server: &Arc<Server>,
    bot: &HeadlessBot,
    summary_lines: &mut Vec<String>,
) -> Result<(), String> {
    let start = Instant::now();
    let world = server.get_world_from_dimension(&Dimension::OVERWORLD);

    let Some(player) = world.get_player_by_name(&bot.name) else {
        return Err(format!("bot player '{}' disappeared", bot.name));
    };
    // Rain keeps the axolotl wet, the test needs dry weather.
    world.set_raining(false);
    let spot = prepare_spot(server, &world, &player.living_entity.entity.pos.load()).await;
    // The axolotl looks for water, so it is put into a dry pit. The pit has a
    // roof, because the rain keeps the axolotl wet.
    for x in -2i32..=2 {
        for z in -2i32..=2 {
            for y in 0..=3 {
                // The interior is cleared, the surrounding water and the
                // terrain walls would keep the axolotl wet.
                let block = if x.abs() < 2 && z.abs() < 2 && y < 3 {
                    &Block::AIR
                } else {
                    &Block::STONE
                };
                place_block(
                    server,
                    &world,
                    &BlockPos::new(spot.0.x + x, spot.0.y + y, spot.0.z + z),
                    block,
                )
                .await;
            }
        }
    }
    // Solid floor under the whole pit, the terrain below it can be water.
    for x in -1i32..=1 {
        for z in -1i32..=1 {
            place_block(
                server,
                &world,
                &BlockPos::new(spot.0.x + x, spot.0.y - 1, spot.0.z + z),
                &Block::STONE,
            )
            .await;
        }
    }
    let axolotl = spawn_axolotl(&world, &spot)?;

    // The axolotl has 6000 ticks of air, the test shortens the wait.
    let axolotl_entity = axolotl.clone();
    let axolotl = axolotl_entity
        .cast_any()
        .downcast_ref::<AxolotlEntity>()
        .ok_or_else(|| "the summoned axolotl is not an axolotl".to_string())?;
    axolotl
        .air_supply
        .store(5, std::sync::atomic::Ordering::Relaxed);

    let mut hurt = false;
    for _ in 0..200 {
        tokio::time::sleep(Duration::from_millis(5)).await;
        server.tick();
        if axolotl.get_mob_entity().living_entity.health.load() < 8.0 {
            hurt = true;
            break;
        }
    }
    if !hurt {
        return Err("the axolotl did not take dry-out damage".to_string());
    }

    axolotl.get_entity().remove();
    summary_lines.push(format!(
        "[PASS] bot:axolotl_dries_out_out_of_water ({}ms)",
        start.elapsed().as_millis()
    ));
    Ok(())
}

/// A dolphin that is fed by a player goes looking for its treasure.
pub async fn run_dolphin_got_fish_tests(
    server: &Arc<Server>,
    bot: &mut HeadlessBot,
    summary_lines: &mut Vec<String>,
) -> Result<(), String> {
    let start = Instant::now();
    let world = server.get_world_from_dimension(&Dimension::OVERWORLD);

    let Some(player) = world.get_player_by_name(&bot.name) else {
        return Err(format!("bot player '{}' disappeared", bot.name));
    };
    let spot = prepare_spot(server, &world, &player.living_entity.entity.pos.load()).await;
    let dolphin = spawn_dolphin(&world, &spot)?;
    let dolphin_entity = dolphin.clone();

    // The bot holds a cod and feeds it to the dolphin.
    bot.sync_client_state(server).await?;
    {
        let inventory = player.inventory();
        let mut main_inventory = inventory
            .main_inventory
            .write()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        main_inventory[0] = ItemStack::new(1, &Item::COD);
    };
    bot.set_held_item(0).await?;

    let mut got_fish = false;
    for _ in 0..200 {
        tokio::time::sleep(Duration::from_millis(5)).await;
        server.tick();
        let fed = dolphin_entity
            .cast_any()
            .downcast_ref::<pumpkin_core::entity::passive::dolphin::DolphinEntity>()
            .is_some_and(DolphinEntity::got_fish);
        if fed {
            got_fish = true;
            break;
        }
        // The interaction packet can be dropped, so it is repeated.
        bot.send_packet(&SInteract {
            entity_id: VarInt(dolphin.get_entity().entity_id),
            r#type: VarInt(ActionType::Interact as i32),
            target_position: None,
            hand: Some(VarInt(0)),
            sneaking: false,
        })
        .await?;
    }
    if !got_fish {
        return Err("the dolphin did not take the fish from the bot".to_string());
    }

    dolphin.get_entity().remove();
    summary_lines.push(format!(
        "[PASS] bot:dolphin_accepts_fish_from_a_player ({}ms)",
        start.elapsed().as_millis()
    ));
    Ok(())
}

fn spawn_named(
    world: &Arc<pumpkin_core::world::World>,
    name: &str,
    spot: &BlockPos,
    nbt: Option<NbtCompound>,
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
    if let Some(nbt) = nbt {
        entity.read_nbt_non_mut(&nbt);
    }
    world.spawn_entity(entity.clone());
    Ok(entity)
}

fn spawn_tamed_nautilus(
    world: &Arc<pumpkin_core::world::World>,
    spot: &BlockPos,
) -> Result<Arc<dyn EntityBase>, String> {
    let mut nbt = NbtCompound::new();
    nbt.put_bool("Tame", true);
    spawn_named(world, "nautilus", spot, Some(nbt))
}

fn spawn_axolotl(
    world: &Arc<pumpkin_core::world::World>,
    spot: &BlockPos,
) -> Result<Arc<dyn EntityBase>, String> {
    spawn_named(world, "axolotl", spot, None)
}

fn spawn_dolphin(
    world: &Arc<pumpkin_core::world::World>,
    spot: &BlockPos,
) -> Result<Arc<dyn EntityBase>, String> {
    spawn_named(world, "dolphin", spot, None)
}
