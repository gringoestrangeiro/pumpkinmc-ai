#[allow(clippy::wildcard_imports)]
use super::*;
use crate::entity::vehicle::minecart::MinecartEntity;
use pumpkin_protocol::java::server::play::SSetCommandMinecart;

impl JavaClient {
    pub fn handle_set_command_minecart(
        &self,
        player: &Arc<Player>,
        packet: &SSetCommandMinecart<'_>,
    ) {
        if !player.can_use_game_master_blocks() {
            player.send_system_message(&TextComponent::translate_cross(
                translation::java::ADVMODE_NOTALLOWED,
                translation::bedrock::ADVMODE_NOTALLOWED,
                [],
            ));
            return;
        }

        let world = player.world();
        let Some(entity) = world.get_entity_by_id(packet.entity_id.0) else {
            return;
        };
        let Some(minecart) = entity.cast_any().downcast_ref::<MinecartEntity>() else {
            return;
        };

        minecart.set_command(packet.command, packet.track_output);

        if !packet.command.is_empty() {
            let command_blocks_work = world.level_info.load().game_rules.command_blocks_work;
            let (java_key, bedrock_key) = if command_blocks_work {
                (
                    translation::java::ADVMODE_SETCOMMAND_SUCCESS,
                    translation::bedrock::ADVMODE_SETCOMMAND_SUCCESS,
                )
            } else {
                (
                    translation::java::ADVMODE_SETCOMMAND_DISABLED,
                    translation::java::ADVMODE_SETCOMMAND_DISABLED,
                )
            };
            player.send_system_message(&TextComponent::translate_cross(
                java_key,
                bedrock_key,
                [TextComponent::text(packet.command.to_string())],
            ));
        }
    }
}
