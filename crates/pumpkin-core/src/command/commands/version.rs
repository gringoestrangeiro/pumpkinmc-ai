use pumpkin_data::packet::CURRENT_MC_VERSION;
use pumpkin_data::translation;
use pumpkin_util::PermissionLvl;
use pumpkin_util::permission::{Permission, PermissionDefault, PermissionRegistry};
use pumpkin_util::text::TextComponent;
use pumpkin_world::chunk::format::anvil::WORLD_DATA_VERSION;
use time::format_description::StaticFormatDescription;
use time::format_description::well_known::Rfc3339;
use time::{OffsetDateTime, UtcOffset};

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

/// `java.util.Date::to_string`'s layout without the timezone and year, which
/// are appended separately because the timezone depends on the local offset.
const DATE_FORMAT: StaticFormatDescription = time::macros::format_description!(
    "[weekday repr:short] [month repr:short] [day padding:zero] [hour]:[minute]:[second]"
);

/// Renders an ISO 8601 build time like vanilla, which prints
/// `java.util.Date::to_string()` in the JVM default timezone.
fn format_build_time(build_time: &str, offset: UtcOffset) -> String {
    let Ok(timestamp) = OffsetDateTime::parse(build_time, &Rfc3339) else {
        return build_time.to_string();
    };
    let timestamp = timestamp.to_offset(offset);
    let date = timestamp.format(DATE_FORMAT).unwrap_or_default();
    format!("{date} {} {}", format_time_zone(offset), timestamp.year())
}

/// Renders the timezone like `java.util.TimeZone::get_display_name`: `UTC` at a
/// zero offset, `GMT+05:30` or `GMT-03:00` otherwise.
fn format_time_zone(offset: UtcOffset) -> String {
    if offset.is_utc() {
        return "UTC".to_string();
    }

    let sign = if offset.is_negative() { '-' } else { '+' };
    let hours = offset.whole_hours().unsigned_abs();
    let minutes = offset.minutes_past_hour().unsigned_abs();
    format!("GMT{sign}{hours:02}:{minutes:02}")
}

struct Executor;

impl CommandExecutor for Executor {
    fn execute(&self, context: &CommandContext) -> CommandExecutorResult {
        let protocol = CURRENT_MC_VERSION.protocol_version();
        let local_offset = UtcOffset::current_local_offset().unwrap_or(UtcOffset::UTC);
        let build_time = format_build_time(BUILD_TIME, local_offset);
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
                [TextComponent::text(build_time)],
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

#[cfg(test)]
mod tests {
    use time::UtcOffset;
    use time::macros::offset;

    use super::format_build_time;

    #[test]
    fn build_time_matches_vanilla_date_format_in_utc() {
        assert_eq!(
            format_build_time("2026-09-15T11:20:48+00:00", UtcOffset::UTC),
            "Tue Sep 15 11:20:48 UTC 2026"
        );
    }

    #[test]
    fn build_time_matches_vanilla_date_format_with_negative_offset() {
        assert_eq!(
            format_build_time("2026-09-15T11:20:48+00:00", offset!(-3)),
            "Tue Sep 15 08:20:48 GMT-03:00 2026"
        );
    }

    #[test]
    fn build_time_matches_vanilla_date_format_with_positive_offset() {
        assert_eq!(
            format_build_time("2026-01-05T12:30:00+00:00", offset!(+5:30)),
            "Mon Jan 05 18:00:00 GMT+05:30 2026"
        );
    }
}
