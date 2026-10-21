use std::borrow::Cow;
use std::sync::Arc;

use crate::block::entities::beehive::{BeeReleaseStatus, BeehiveBlockEntity};
use crate::block::{BlockBehaviour, BlockMetadata, BrokenArgs, GetComparatorOutputArgs};
use crate::entity::EntityBase;
use crate::entity::mob::Mob;
use crate::entity::passive::bee::BeeEntity;
use crate::entity::player::Player;
use pumpkin_data::block_properties::BeeNestLikeProperties;
use pumpkin_data::data_component_impl::{BlockStateImpl, EnchantmentsImpl};
use pumpkin_data::item::Item;
use pumpkin_data::item_stack::ItemStack;
use pumpkin_data::tag::Taggable;
use pumpkin_data::{BlockId, BlockState};
use pumpkin_util::GameMode;
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

    /// Vanilla `blocks/beehive` and `blocks/bee_nest` copy the bees and honey level
    /// onto the silk-touch drop only.
    fn modify_dropped_item(
        &self,
        stack: &mut ItemStack,
        state: &BlockState,
        has_silk_touch: bool,
        block_entity: Option<&dyn crate::block::entities::BlockEntity>,
    ) {
        if !has_silk_touch {
            return;
        }
        let Some(block_entity) = block_entity else {
            return;
        };
        if let Some(beehive) = block_entity.as_any().downcast_ref::<BeehiveBlockEntity>() {
            beehive.collect_components(stack);
        }
        set_honey_level(
            stack,
            BeeNestLikeProperties::from_state_id(state.id).honey_level,
        );
    }

    /// Vanilla `BeehiveBlock.playerWillDestroy`.
    fn player_will_destroy(&self, args: BrokenArgs<'_>) {
        if args.player.gamemode.load() != GameMode::Creative
            || !args.world.level_info.load().game_rules.block_drops
        {
            return;
        }
        let Some(block_entity) = args.world.get_block_entity(args.position) else {
            return;
        };
        let Some(beehive) = block_entity.as_any().downcast_ref::<BeehiveBlockEntity>() else {
            return;
        };
        let honey_level = BeeNestLikeProperties::from_state_id(args.state.id).honey_level;
        if beehive.is_empty() && honey_level == 0 {
            return;
        }

        let item = if args.block.id == BlockId::BEE_NEST {
            &Item::BEE_NEST
        } else {
            &Item::BEEHIVE
        };
        let mut stack = ItemStack::new(1, item);
        beehive.collect_components(&mut stack);
        set_honey_level(&mut stack, honey_level);
        args.world.drop_stack(args.position, stack);
    }

    fn broken(&self, args: BrokenArgs<'_>) {
        let Some(block_entity) = args.world.get_block_entity(args.position) else {
            return;
        };
        let Some(beehive) = block_entity.as_any().downcast_ref::<BeehiveBlockEntity>() else {
            return;
        };
        if !prevents_bee_spawns(&args.player.inventory().held_item()) {
            beehive.empty_all_living_from_hive(
                args.world,
                Some(args.player),
                args.state.id,
                BeeReleaseStatus::Emergency,
            );
            anger_nearby_bees(&args);
        }
    }
}

/// Vanilla `EnchantmentTags.PREVENTS_BEE_SPAWNS_WHEN_MINING`.
fn prevents_bee_spawns(stack: &ItemStack) -> bool {
    stack
        .get_data_component::<EnchantmentsImpl>()
        .is_some_and(|enchantments| {
            enchantments.enchantment.iter().any(|(enchantment, _)| {
                enchantment.has_tag(
                    &pumpkin_data::tag::Enchantment::MINECRAFT_PREVENTS_BEE_SPAWNS_WHEN_MINING,
                )
            })
        })
}

fn set_honey_level(stack: &mut ItemStack, honey_level: u8) {
    stack.set_data_component(BlockStateImpl {
        properties: Cow::Owned(vec![(
            Cow::Borrowed("honey_level"),
            Cow::Owned(honey_level.to_string()),
        )]),
    });
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
