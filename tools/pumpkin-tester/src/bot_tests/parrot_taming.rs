use std::{
    sync::Arc,
    time::{Duration, Instant},
};

use pumpkin_core::{
    entity::{
        EntityBase,
        passive::{parrot::ParrotEntity, tamable::TamableAnimal},
    },
    server::Server,
};
use pumpkin_data::{dimension::Dimension, entity::EntityType, item::Item, item_stack::ItemStack};
use pumpkin_nbt::compound::NbtCompound;
use pumpkin_protocol::{
    codec::var_int::VarInt,
    java::server::play::{ActionType, SInteract},
};
use pumpkin_util::math::{position::BlockPos, vector3::Vector3};
use uuid::Uuid;

use crate::bot::HeadlessBot;
use crate::bot_tests::setup::prepare_spot;

/// The bot tames a parrot with seeds.
pub async fn run_parrot_taming_tests(
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
    let spot = prepare_spot(server, &world, &bot_pos).await;

    let parrot = spawn_parrot(&world, &spot)?;

    // Seeds tame a parrot, but only in one of ten tries.
    give_item(&player, &Item::WHEAT_SEEDS, 64);
    bot.set_held_item(0).await?;
    let mut attempts = 0;
    for _ in 0..150 {
        if parrot_tamed(&parrot) {
            break;
        }
        attempts += 1;
        // A parrot flies around, so it is put back on the prepared spot when
        // it wanders off.
        let parrot_pos = parrot.get_entity().pos.load();
        let spot_pos = Vector3::new(
            f64::from(spot.0.x) + 0.5,
            f64::from(spot.0.y) + 0.5,
            f64::from(spot.0.z) + 0.5,
        );
        if parrot_pos.squared_distance_to_vec(&spot_pos) > 4.0 {
            if !parrot.get_entity().is_alive() {
                return Err("the parrot died while being tamed".to_string());
            }
            parrot.get_entity().teleport(spot_pos, None, None, &world);
        }
        bot.send_packet(&SInteract {
            entity_id: VarInt(parrot.get_entity().entity_id),
            r#type: VarInt(ActionType::Interact as i32),
            target_position: None,
            hand: Some(VarInt(0)),
            sneaking: false,
        })
        .await?;
        for _ in 0..5 {
            tokio::time::sleep(Duration::from_millis(5)).await;
            server.tick();
        }
    }
    if !parrot_tamed(&parrot) {
        return Err(format!(
            "the bot could not tame the parrot with seeds after {attempts} attempts, held item {:?}, parrot at {:?}, health {}, gamemode {:?}, client loaded {}",
            player.inventory().held_item().item.id,
            parrot.get_entity().pos.load(),
            player.living_entity.health.load(),
            player.gamemode.load(),
            player.has_client_loaded()
        ));
    }
    if parrot_owner(&parrot) != Some(player.get_entity().entity_uuid) {
        return Err("the tamed parrot is not owned by the bot".to_string());
    }

    perch_on_shoulder(server, &world, &player, &parrot).await?;

    summary_lines.push(format!(
        "[PASS] bot:bot_tames_a_parrot_with_seeds ({}ms)",
        start.elapsed().as_millis()
    ));
    Ok(())
}

/// Waits out the parrot's shoulder cooldown and lets it perch on the bot.
async fn perch_on_shoulder(
    server: &Arc<Server>,
    world: &Arc<pumpkin_core::world::World>,
    player: &Arc<pumpkin_core::entity::player::Player>,
    parrot: &Arc<dyn EntityBase>,
) -> Result<(), String> {
    // Vanilla `ShoulderRidingEntity` refuses to perch while its ride
    // cooldown (100 ticks) is still running.
    let mut cooldown_passed = false;
    for _ in 0..200 {
        let Some(entity) = parrot.cast_any().downcast_ref::<ParrotEntity>() else {
            return Err("the parrot is not a parrot".to_string());
        };
        if entity.can_sit_on_shoulder() {
            cooldown_passed = true;
            break;
        }
        tokio::time::sleep(Duration::from_millis(5)).await;
        server.tick();
    }
    if !cooldown_passed {
        return Err(format!(
            "the parrot shoulder cooldown did not run out, parrot at {:?}",
            parrot.get_entity().pos.load()
        ));
    }

    // Vanilla `LandOnOwnersShoulderGoal` perches the parrot when its box
    // touches the box of the owner, so the parrot is put next to the bot. The
    // parrot is put back every tick because a push can separate the boxes
    // before the goal ticks.
    let mut landed = false;
    for _ in 0..100 {
        let owner_pos = player.living_entity.entity.pos.load();
        parrot.get_entity().teleport(
            Vector3::new(owner_pos.x + 0.25, owner_pos.y, owner_pos.z + 0.25),
            None,
            None,
            world,
        );
        tokio::time::sleep(Duration::from_millis(5)).await;
        server.tick();
        if shoulder_parrot(player).is_some() {
            landed = true;
            break;
        }
    }
    if !landed {
        return Err(format!(
            "the tamed parrot did not land on the shoulder, parrot at {:?}, bot at {:?}",
            parrot.get_entity().pos.load(),
            player.living_entity.entity.pos.load()
        ));
    }
    Ok(())
}

/// The parrot that sits on the player's shoulder.
fn shoulder_parrot(player: &Arc<pumpkin_core::entity::player::Player>) -> Option<NbtCompound> {
    let left = player
        .shoulder_entity_left
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .clone();
    if left.is_some() {
        return left;
    }
    player
        .shoulder_entity_right
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .clone()
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

/// The owner of a parrot.
fn parrot_owner(parrot: &Arc<dyn EntityBase>) -> Option<Uuid> {
    let parrot = parrot.clone();
    parrot
        .cast_any()
        .downcast_ref::<ParrotEntity>()
        .and_then(TamableAnimal::get_owner)
}

/// Whether a parrot is tamed.
fn parrot_tamed(parrot: &Arc<dyn EntityBase>) -> bool {
    let parrot = parrot.clone();
    parrot
        .cast_any()
        .downcast_ref::<ParrotEntity>()
        .is_some_and(TamableAnimal::is_tame)
}

fn spawn_parrot(
    world: &Arc<pumpkin_core::world::World>,
    spot: &BlockPos,
) -> Result<Arc<dyn EntityBase>, String> {
    let entity_type = EntityType::from_name("parrot")
        .ok_or_else(|| "unknown entity type 'parrot'".to_string())?;
    let entity = pumpkin_core::entity::r#type::from_type(
        entity_type,
        Vector3::new(
            f64::from(spot.0.x) + 0.5,
            f64::from(spot.0.y) + 0.5,
            f64::from(spot.0.z) + 0.5,
        ),
        world,
        Uuid::new_v4(),
    );
    // The parrot flies around and would despawn far away from the player.
    let mut nbt = NbtCompound::new();
    nbt.put_bool("PersistenceRequired", true);
    entity.read_nbt_non_mut(&nbt);
    world.spawn_entity(entity.clone());
    Ok(entity)
}
