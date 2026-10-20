use std::any::Any;

use crate::block::registry::BlockActionResult;
use crate::entity::player::Player;
use crate::item::{ItemBehaviour, ItemMetadata};
use crate::server::Server;
use pumpkin_data::data_component::DataComponent;
use pumpkin_data::data_component_impl::{DataComponentImpl, LodestoneTarget, LodestoneTrackerImpl};
use pumpkin_data::item::Item;
use pumpkin_data::item_stack::ItemStack;
use pumpkin_data::sound::{Sound, SoundCategory};
use pumpkin_data::{Block, BlockDirection};
use pumpkin_util::math::position::BlockPos;
use pumpkin_util::math::vector3::Vector3;

pub struct CompassItem;

impl ItemMetadata for CompassItem {
    fn ids() -> Box<[u16]> {
        Box::new([Item::COMPASS.id, Item::RECOVERY_COMPASS.id])
    }
}

impl ItemBehaviour for CompassItem {
    fn inventory_tick(&self, item: &mut ItemStack, player: &Player) -> bool {
        if item.item.id != Item::COMPASS.id {
            return false;
        }
        let Some(tracker) = item.get_data_component::<LodestoneTrackerImpl>() else {
            return false;
        };
        if !tracker.tracked {
            return false;
        }
        let Some(target) = tracker.target.clone() else {
            return false;
        };

        let world = player.world();
        if target.dimension != world.dimension.minecraft_name {
            return false;
        }

        // Vanilla checks the lodestone point-of-interest. Pumpkin does not track
        // points of interest, so the lodestone block is the equivalent check. Leave
        // the tracker alone while the target chunk is not loaded.
        let target_pos = BlockPos::new(target.x, target.y, target.z);
        let Some(target_state) = world.get_block_state_id_if_loaded(&target_pos) else {
            return false;
        };
        if Block::from_state_id(target_state) == &Block::LODESTONE {
            return false;
        }

        item.set_data_component(LodestoneTrackerImpl {
            target: None,
            tracked: true,
        });
        true
    }

    fn use_on_block(
        &self,
        item: &mut ItemStack,
        player: &Player,
        location: BlockPos,
        _face: BlockDirection,
        _cursor_pos: Vector3<f32>,
        block: &Block,
        _server: &Server,
    ) -> BlockActionResult {
        if block.id == Block::LODESTONE.id && item.item.id == Item::COMPASS.id {
            let world = player.world();
            world.play_sound(
                Sound::ItemLodestoneCompassLock,
                SoundCategory::Players,
                &location.to_f64(),
            );

            let mut lodestone_compass = ItemStack::new(1, &Item::COMPASS);
            let tracker = LodestoneTrackerImpl {
                target: Some(LodestoneTarget {
                    dimension: world.dimension.minecraft_name.to_string(),
                    x: location.0.x,
                    y: location.0.y,
                    z: location.0.z,
                }),
                tracked: true,
            };
            lodestone_compass
                .patch
                .push((DataComponent::LodestoneTracker, Some(tracker.to_dyn())));

            let gamemode = player.gamemode.load();
            if gamemode != pumpkin_util::GameMode::Creative && item.item_count == 1 {
                *item = lodestone_compass;
            } else {
                item.decrement_unless_creative(gamemode, 1);
                let was_added = player
                    .inventory
                    .insert_stack_anywhere(&mut lodestone_compass);
                if !was_added && !lodestone_compass.is_empty() {
                    world.drop_stack(&player.position().to_block_pos(), lodestone_compass);
                }
            }
            BlockActionResult::Success
        } else {
            BlockActionResult::Pass
        }
    }

    fn as_any(&self) -> &dyn Any {
        self
    }
}
