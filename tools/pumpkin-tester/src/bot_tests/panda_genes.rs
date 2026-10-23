use std::{
    sync::Arc,
    time::{Duration, Instant},
};

use pumpkin_core::{
    entity::{
        EntityBase,
        ageable::AgeableMob,
        passive::panda::{PandaEntity, PandaGene},
    },
    server::Server,
};
use pumpkin_data::{
    Block, dimension::Dimension, entity::EntityType, item::Item, item_stack::ItemStack,
};
use pumpkin_protocol::{
    codec::var_int::VarInt,
    java::server::play::{ActionType, SInteract},
};
use pumpkin_util::math::{boundingbox::BoundingBox, position::BlockPos, vector3::Vector3};
use uuid::Uuid;

use crate::bot::HeadlessBot;
use crate::bot_tests::setup::place_block;

/// A panda that the bot feeds with bamboo goes into the love mode, and two
/// pandas in love breed. The baby inherits one gene from each parent.
pub async fn run_panda_genes_tests(
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

    // The bot feeds a panda with bamboo, which puts it in love. The panda
    // stands on a platform next to the bot.
    let feed_spot = build_platform(server, &world, &bot_pos, 2).await;
    give_bamboo(&player, 10);
    bot.set_held_item(0).await?;
    let fed = spawn_panda(&world, &feed_spot, 0)?;
    for _ in 0..5 {
        if panda_in_love(&fed) {
            break;
        }
        bot.send_packet(&SInteract {
            entity_id: VarInt(fed.get_entity().entity_id),
            r#type: VarInt(ActionType::Interact as i32),
            target_position: None,
            hand: Some(VarInt(0)),
            sneaking: false,
        })
        .await?;
        for _ in 0..20 {
            tokio::time::sleep(Duration::from_millis(5)).await;
            server.tick();
        }
    }
    if !panda_in_love(&fed) {
        fed.get_entity().remove();
        return Err("the bot could not feed the panda with bamboo".to_string());
    }
    fed.get_entity().remove();

    // The pandas follow the bamboo in the hand of the bot, so the hand is empty
    // while they breed.
    give_bamboo(&player, 0);
    bot.set_held_item(0).await?;

    // The pandas avoid players, so they breed on a platform that is out of the
    // reach of the bot.
    let spot = build_platform(server, &world, &bot_pos, 12).await;

    // Vanilla lets a random gene slip through, so a baby that does not inherit
    // is bred again with a fresh pair.
    let mut last_error = String::new();
    for _ in 0..3 {
        match breed_and_check(server, &world, &spot).await {
            Ok(()) => {
                summary_lines.push(format!(
                    "[PASS] bot:panda_genes_are_inherited_from_both_parents ({}ms)",
                    start.elapsed().as_millis()
                ));
                return Ok(());
            }
            Err(err) => last_error = err,
        }
    }
    Err(last_error)
}

/// Breeds one pair of pandas and checks the genes of the baby.
async fn breed_and_check(
    server: &Arc<Server>,
    world: &Arc<pumpkin_core::world::World>,
    spot: &BlockPos,
) -> Result<(), String> {
    let first = spawn_panda(world, spot, -1)?;
    let second = spawn_panda(world, spot, 0)?;
    // Disjoint genes make a swapped inheritance visible.
    set_genes(&first, PandaGene::Normal, PandaGene::Brown);
    set_genes(&second, PandaGene::Weak, PandaGene::Aggressive);
    // Both pandas are in love from the start, so the breed goal keeps them
    // together while they look for a mate.
    set_love_ticks(&first, 600);
    set_love_ticks(&second, 600);

    let mut baby: Option<Arc<dyn EntityBase>> = None;
    for _ in 0..600 {
        tokio::time::sleep(Duration::from_millis(5)).await;
        server.tick();
        let found = world
            .get_entities_at_box(&BoundingBox::from_block(spot).expand(8.0, 4.0, 8.0))
            .into_iter()
            .find(|entity| {
                entity
                    .cast_any()
                    .downcast_ref::<PandaEntity>()
                    .is_some_and(AgeableMob::is_baby)
            });
        if let Some(found) = found {
            baby = Some(found);
            break;
        }
    }
    let Some(baby) = baby else {
        let state = format!(
            "in love: {} and {}, love ticks: {} and {}, positions: {:?} and {:?}",
            panda_in_love(&first),
            panda_in_love(&second),
            panda_love_ticks(&first),
            panda_love_ticks(&second),
            first.get_entity().pos.load(),
            second.get_entity().pos.load()
        );
        first.get_entity().remove();
        second.get_entity().remove();
        return Err(format!("the pandas did not breed, {state}"));
    };

    let first_genes = panda_genes(&first);
    let second_genes = panda_genes(&second);
    let baby_genes = panda_genes(&baby);
    let inherits_both = (first_genes.contains(&baby_genes[0])
        && second_genes.contains(&baby_genes[1]))
        || (second_genes.contains(&baby_genes[0]) && first_genes.contains(&baby_genes[1]));

    first.get_entity().remove();
    second.get_entity().remove();
    baby.get_entity().remove();

    if !inherits_both {
        return Err(format!(
            "the baby genes {baby_genes:?} do not come from the parents {first_genes:?} and {second_genes:?}"
        ));
    }
    Ok(())
}

/// A flat platform with bamboo at the edges, so the pandas have ground to
/// breed on and can smell bamboo.
async fn build_platform(
    server: &Arc<Server>,
    world: &Arc<pumpkin_core::world::World>,
    bot_pos: &Vector3<f64>,
    distance: i32,
) -> BlockPos {
    let spot = BlockPos::new(
        (bot_pos.x + f64::from(distance)).floor() as i32,
        bot_pos.y.floor() as i32,
        bot_pos.z.floor() as i32,
    );
    for x in -3..=3 {
        for z in -3..=3 {
            place_block(
                server,
                world,
                &BlockPos::new(spot.0.x + x, spot.0.y - 1, spot.0.z + z),
                &Block::STONE,
            )
            .await;
        }
    }
    for (x, z) in [(-3, -3), (-3, 3), (3, -3), (3, 3)] {
        place_block(
            server,
            world,
            &BlockPos::new(spot.0.x + x, spot.0.y, spot.0.z + z),
            &Block::BAMBOO,
        )
        .await;
    }
    spot
}

/// Puts bamboo into the first hotbar slot of the player.
fn give_bamboo(player: &Arc<pumpkin_core::entity::player::Player>, count: u8) {
    let inventory = player.inventory();
    let mut main_inventory = inventory
        .main_inventory
        .write()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    main_inventory[0] = ItemStack::new(count, &Item::BAMBOO);
}

/// Puts a panda into the love mode.
fn set_love_ticks(panda: &Arc<dyn EntityBase>, ticks: i32) {
    let panda = panda.clone();
    if let Some(panda) = panda.cast_any().downcast_ref::<PandaEntity>() {
        panda.mob_entity.set_love_ticks(ticks, None);
    }
}

/// Sets the genes of a panda.
fn set_genes(panda: &Arc<dyn EntityBase>, main: PandaGene, hidden: PandaGene) {
    let panda = panda.clone();
    if let Some(panda) = panda.cast_any().downcast_ref::<PandaEntity>() {
        panda.set_main_gene(main);
        panda.set_hidden_gene(hidden);
    }
}

/// The remaining love ticks of a panda.
fn panda_love_ticks(panda: &Arc<dyn EntityBase>) -> i32 {
    let panda = panda.clone();
    panda
        .cast_any()
        .downcast_ref::<PandaEntity>()
        .map_or(0, |panda| {
            panda
                .mob_entity
                .love_ticks
                .load(std::sync::atomic::Ordering::Relaxed)
        })
}

/// Whether a panda is in love.
fn panda_in_love(panda: &Arc<dyn EntityBase>) -> bool {
    let panda = panda.clone();
    panda
        .cast_any()
        .downcast_ref::<PandaEntity>()
        .is_some_and(EntityBase::is_in_love)
}

/// The main and the hidden gene of a panda.
fn panda_genes(panda: &Arc<dyn EntityBase>) -> [PandaGene; 2] {
    let panda = panda.clone();
    panda
        .cast_any()
        .downcast_ref::<PandaEntity>()
        .map_or([PandaGene::Normal; 2], |panda| {
            [panda.get_main_gene(), panda.get_hidden_gene()]
        })
}

fn spawn_panda(
    world: &Arc<pumpkin_core::world::World>,
    spot: &BlockPos,
    offset_x: i32,
) -> Result<Arc<dyn EntityBase>, String> {
    let entity_type =
        EntityType::from_name("panda").ok_or_else(|| "unknown entity type 'panda'".to_string())?;
    let entity = pumpkin_core::entity::r#type::from_type(
        entity_type,
        Vector3::new(
            f64::from(spot.0.x + offset_x) + 0.5,
            f64::from(spot.0.y),
            f64::from(spot.0.z) + 0.5,
        ),
        world,
        Uuid::new_v4(),
    );
    world.spawn_entity(entity.clone());
    Ok(entity)
}
