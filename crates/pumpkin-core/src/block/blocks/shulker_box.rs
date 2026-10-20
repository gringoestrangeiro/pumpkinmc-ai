use std::sync::Arc;
use std::sync::Mutex;

use crate::block::{
    GetComparatorOutputArgs, GetScreenHandlerFactoryArgs, OnPlaceArgs, OnSyncedBlockEventArgs,
    PlacedArgs,
};
use crate::block::{
    registry::BlockActionResult,
    {BlockBehaviour, NormalUseArgs},
};

use crate::block::entities::shulker_box::ShulkerBoxBlockEntity;
use pumpkin_data::BlockStateId;
use pumpkin_data::block_properties::Facing;
use pumpkin_data::translation;
use pumpkin_inventory::Inventory;
use pumpkin_inventory::generic_container_screen_handler::create_generic_9x3;
use pumpkin_inventory::player::player_inventory::PlayerInventory;
use pumpkin_inventory::screen_handler::{
    InventoryPlayer, ScreenHandlerFactory, SharedScreenHandler,
};
use pumpkin_macros::pumpkin_block_from_tag;
use pumpkin_util::math::boundingbox::BoundingBox;
use pumpkin_util::math::position::BlockPos;
use pumpkin_util::math::vector3::Vector3;
use pumpkin_util::text::TextComponent;

struct ShulkerBoxScreenFactory(Arc<dyn Inventory>);

impl ScreenHandlerFactory for ShulkerBoxScreenFactory {
    fn create_screen_handler(
        &self,
        sync_id: u8,
        player_inventory: &Arc<PlayerInventory>,
        player: &dyn InventoryPlayer,
    ) -> Option<SharedScreenHandler> {
        let handler = create_generic_9x3(sync_id, player_inventory, self.0.clone(), player);
        let screen_handler_arc = Arc::new(Mutex::new(handler));

        Some(screen_handler_arc as SharedScreenHandler)
    }

    fn get_display_name(&self) -> TextComponent {
        pumpkin_macros::translate_cross!(
            translation::java::CONTAINER_SHULKERBOX,
            translation::bedrock::CONTAINER_SHULKERBOX
        )
    }
}

#[pumpkin_block_from_tag("minecraft:shulker_boxes")]
pub struct ShulkerBoxBlock;

type EndRodLikeProperties = pumpkin_data::block_properties::EndRodLikeProperties;

impl BlockBehaviour for ShulkerBoxBlock {
    fn on_place(&self, args: OnPlaceArgs<'_>) -> BlockStateId {
        let mut props = EndRodLikeProperties::default(args.block);
        props.facing = args.direction.to_facing().opposite();
        props.to_state_id(args.block)
    }

    fn on_synced_block_event(&self, args: OnSyncedBlockEventArgs<'_>) -> bool {
        // On the server, we don't need the Animation steps for now, because the client is responsible for that.
        args.r#type == Self::OPEN_ANIMATION_EVENT_TYPE
    }

    fn placed(&self, args: PlacedArgs<'_>) {
        {
            let barrel_block_entity = ShulkerBoxBlockEntity::new(*args.position);
            args.world.add_block_entity(Arc::new(barrel_block_entity));
        }
    }

    fn normal_use(&self, args: NormalUseArgs<'_>) -> BlockActionResult {
        let state = args.world.get_block_state(args.position);
        if let Some(block_entity) = args.world.get_block_entity(args.position)
            && let Some(shulker_box) = block_entity
                .as_any()
                .downcast_ref::<ShulkerBoxBlockEntity>()
            && !Self::can_open(args.world, args.position, state, shulker_box)
        {
            return BlockActionResult::Success;
        }

        if let Some(factory) = self.get_screen_handler_factory(GetScreenHandlerFactoryArgs {
            server: args.server,
            world: args.world,
            block: args.block,
            position: args.position,
            player: args.player,
        }) {
            args.player.increment_stat(
                pumpkin_data::statistic::StatisticCategory::Custom,
                pumpkin_data::statistic::CustomStatistic::OpenShulkerBox as i32,
                1,
            );
            args.player
                .open_handled_screen(factory.as_ref(), Some(*args.position));
            crate::entity::mob::piglin_ai::PiglinAi::anger_nearby_piglins(
                args.world,
                args.player,
                true,
            );
        }

        BlockActionResult::Success
    }

    fn get_screen_handler_factory(
        &self,
        args: GetScreenHandlerFactoryArgs<'_>,
    ) -> Option<Box<dyn ScreenHandlerFactory>> {
        let block_entity = args.world.get_block_entity(args.position)?;
        let inventory = block_entity.get_inventory()?;
        Some(Box::new(ShulkerBoxScreenFactory(inventory)))
    }

    fn get_comparator_output(&self, args: GetComparatorOutputArgs<'_>) -> Option<u8> {
        crate::block::container_comparator_output(&args)
    }
}

impl ShulkerBoxBlock {
    pub const OPEN_ANIMATION_EVENT_TYPE: u8 = 1;

    fn can_open(
        world: &crate::world::World,
        position: &BlockPos,
        state: &pumpkin_data::BlockState,
        block_entity: &ShulkerBoxBlockEntity,
    ) -> bool {
        if block_entity
            .viewers
            .current
            .load(std::sync::atomic::Ordering::Relaxed)
            != 0
        {
            return true;
        }

        let facing = EndRodLikeProperties::from_state_id(state.id).facing;
        world.is_space_empty(Self::lid_open_bounding_box(position, facing))
    }

    /// Vanilla `Shulker.getProgressDeltaAabb(1.0, facing, 0.0, 0.5, pos)`.
    fn lid_open_bounding_box(position: &BlockPos, facing: Facing) -> BoundingBox {
        let (min, max) = match facing {
            Facing::North => ((0.0, 0.0, -0.5), (1.0, 1.0, 0.0)),
            Facing::South => ((0.0, 0.0, 1.0), (1.0, 1.0, 1.5)),
            Facing::West => ((-0.5, 0.0, 0.0), (0.0, 1.0, 1.0)),
            Facing::East => ((1.0, 0.0, 0.0), (1.5, 1.0, 1.0)),
            Facing::Up => ((0.0, 1.0, 0.0), (1.0, 1.5, 1.0)),
            Facing::Down => ((0.0, -0.5, 0.0), (1.0, 0.0, 1.0)),
        };
        BoundingBox::new(
            Vector3::new(
                f64::from(position.0.x) + min.0,
                f64::from(position.0.y) + min.1,
                f64::from(position.0.z) + min.2,
            ),
            Vector3::new(
                f64::from(position.0.x) + max.0,
                f64::from(position.0.y) + max.1,
                f64::from(position.0.z) + max.2,
            ),
        )
        .expand_all(-1.0e-6)
    }
}
