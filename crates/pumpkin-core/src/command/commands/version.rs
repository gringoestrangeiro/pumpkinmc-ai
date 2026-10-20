use pumpkin_data::packet::CURRENT_MC_VERSION;
use pumpkin_data::translation;
use pumpkin_util::PermissionLvl;
use pumpkin_util::permission::{Permission, PermissionDefault, PermissionRegistry};
use pumpkin_util::text::TextComponent;
use pumpkin_world::chunk::format::anvil::WORLD_DATA_VERSION;

use crate::command::argument_builder::{ArgumentBuilder, command};
use crate::command::context::command_context::CommandContext;
use crate::command::node::dispatcher::CommandDispatcher;
use crate::command::node::{CommandExecutor, CommandExecutorResult};

const DESCRIPTION: &str = "Displays the server version information.";
const PERMISSION: &str = "minecraft:command.version";

/// Values from the matching `version.json` that Pumpkin does not otherwise
/// need at runtime.
const DATA_VERSION_SERIES: &str = "main";
const RESOURCE_PACK_VERSION: &str = "97.1";
const DATA_PACK_VERSION: &str = "121.0";

const BUILD_TIME: &str = env!("BUILD_TIME");

struct Executor;

impl CommandExecutor for Executor {
    fn execute(&self, context: &CommandContext) -> CommandExecutorResult {
        let protocol = CURRENT_MC_VERSION.protocol_version();
        let lines = [
            TextComponent::translate_cross(
                translation::java::COMMANDS_VERSION_HEADER,
                translation::java::COMMANDS_VERSION_HEADER,
                [],
            ),
            TextComponent::translate_cross(
                translation::java::COMMANDS_VERSION_ID,
                translation::java::COMMANDS_VERSION_ID,
                [TextComponent::text(CURRENT_MC_VERSION.to_string())],
            ),
            TextComponent::translate_cross(
                translation::java::COMMANDS_VERSION_NAME,
                translation::java::COMMANDS_VERSION_NAME,
                [TextComponent::text(CURRENT_MC_VERSION.to_string())],
            ),
            TextComponent::translate_cross(
                translation::java::COMMANDS_VERSION_DATA,
                translation::java::COMMANDS_VERSION_DATA,
                [TextComponent::text(WORLD_DATA_VERSION.to_string())],
            ),
            TextComponent::translate_cross(
                translation::java::COMMANDS_VERSION_SERIES,
                translation::java::COMMANDS_VERSION_SERIES,
                [TextComponent::text(DATA_VERSION_SERIES)],
            ),
            TextComponent::translate_cross(
                translation::java::COMMANDS_VERSION_PROTOCOL,
                translation::java::COMMANDS_VERSION_PROTOCOL,
                [
                    TextComponent::text(protocol.to_string()),
                    TextComponent::text(format!("0x{protocol:x}")),
                ],
            ),
            TextComponent::translate_cross(
                translation::java::COMMANDS_VERSION_BUILD_TIME,
                translation::java::COMMANDS_VERSION_BUILD_TIME,
                [TextComponent::text(BUILD_TIME)],
            ),
            TextComponent::translate_cross(
                translation::java::COMMANDS_VERSION_PACK_RESOURCE,
                translation::java::COMMANDS_VERSION_PACK_RESOURCE,
                [TextComponent::text(RESOURCE_PACK_VERSION)],
            ),
            TextComponent::translate_cross(
                translation::java::COMMANDS_VERSION_PACK_DATA,
                translation::java::COMMANDS_VERSION_PACK_DATA,
                [TextComponent::text(DATA_PACK_VERSION)],
            ),
            TextComponent::translate_cross(
                translation::java::COMMANDS_VERSION_STABLE_YES,
                translation::java::COMMANDS_VERSION_STABLE_YES,
                [],
            ),
        ];

        for line in lines {
            context.source.send_message(line);
        }

        Ok(1)
    }
}

pub fn register(dispatcher: &mut CommandDispatcher, registry: &PermissionRegistry) {
    registry.register_permission_or_panic(Permission::new(
        PERMISSION,
        DESCRIPTION,
        PermissionDefault::Op(PermissionLvl::Two),
    ));

    dispatcher.register(
        command("version", DESCRIPTION)
            .requires(PERMISSION)
            .executes(Executor),
    );
}
