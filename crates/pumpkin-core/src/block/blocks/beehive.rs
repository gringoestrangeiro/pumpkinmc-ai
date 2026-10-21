use std::sync::Arc;

use crate::block::entities::beehive::{BeeReleaseStatus, BeehiveBlockEntity};
use crate::block::{BlockBehaviour, BlockMetadata, BrokenArgs, GetComparatorOutputArgs};
use crate::entity::EntityBase;
use crate::entity::mob::Mob;
use crate::entity::passive::bee::BeeEntity;
use crate::entity::player::Player;
use pumpkin_data::BlockId;
use pumpkin_data::block_properties::BeeNestLikeProperties;
use pumpkin_data::entity::EntityType;
use pumpkin_util::math::boundingbox::BoundingBox;
use pumpkin_util::math::vector3::Vector3;

pub struct BeehiveBlock;

impl BlockMetadata for BeehiveBlock {
    fn ids() -> Box<[BlockId]> {
        [BlockId::BEEHIVE, BlockId::BEE_NEST].into()
    }
}

impl BlockBehaviour for BeehiveBlock {
    fn get_comparator_output(&self, args: GetComparatorOutputArgs<'_>) -> Option<u8> {
        {
            let state_id = args.world.get_block_state_id(args.position);
            let props = BeeNestLikeProperties::from_state_id(state_id);
            Some(props.honey_level)
        }
    }

    fn broken(&self, args: BrokenArgs<'_>) {
        let Some(block_entity) = args.world.get_block_entity(args.position) else {
            return;
        };
        let Some(beehive) = block_entity.as_any().downcast_ref::<BeehiveBlockEntity>() else {
            return;
        };
        beehive.empty_all_living_from_hive(
            args.world,
            Some(args.player),
            args.state.id,
            BeeReleaseStatus::Emergency,
        );
        anger_nearby_bees(&args);
    }
}

/// Vanilla `BeehiveBlock.angerNearbyBees`.
fn anger_nearby_bees(args: &BrokenArgs<'_>) {
    let pos = args.position.0;
    let area = BoundingBox {
        min: Vector3::new(
            f64::from(pos.x - 8),
            f64::from(pos.y - 6),
            f64::from(pos.z - 8),
        ),
        max: Vector3::new(
            f64::from(pos.x + 9),
            f64::from(pos.y + 7),
            f64::from(pos.z + 9),
        ),
    };
    let entities = args.world.get_entities_and_players_at_box(&area);
    let players: Vec<Arc<Player>> = entities
        .iter()
        .filter(|entity| entity.get_entity().entity_type == &EntityType::PLAYER)
        .filter_map(|entity| {
            args.world
                .get_player_by_uuid(entity.get_entity().entity_uuid)
        })
        .collect();
    if players.is_empty() {
        return;
    }

    for entity in &entities {
        let Some(bee) = entity.cast_any().downcast_ref::<BeeEntity>() else {
            continue;
        };
        if bee.mob_entity.get_target().is_some() {
            continue;
        }
        let target: Arc<dyn EntityBase> = players[rand::random_range(0..players.len())].clone();
        bee.set_mob_target(Some(target));
    }
}
