use std::fs;
use std::sync::atomic::Ordering;

use pumpkin_data::translation;
use pumpkin_util::PermissionLvl;
use pumpkin_util::permission::{Permission, PermissionDefault, PermissionRegistry};
use pumpkin_util::text::TextComponent;

use crate::command::CommandSender;
use crate::command::argument_builder::{ArgumentBuilder, argument, command, literal};
use crate::command::argument_types::function::FunctionArgumentType;
use crate::command::context::command_context::CommandContext;
use crate::command::errors::error_types::CommandErrorType;
use crate::command::node::dispatcher::CommandDispatcher;
use crate::command::node::{CommandExecutor, CommandExecutorResult};
use crate::data::datapack::{DatapackManager, FunctionCallError};
use crate::server::debug_profiler::{StartDebugProfileError, StopDebugProfileError};

use super::function::{
    ERROR_FUNCTION_INTERNAL, ERROR_UNKNOWN_FUNCTION, FunctionSuggestionProvider,
};

const DESCRIPTION: &str = "Starts or stops a tick profiling session.";
const PERMISSION: &str = "minecraft:command.debug";

const ALREADY_RUNNING_ERROR_TYPE: CommandErrorType<0> = CommandErrorType::new(
    translation::java::COMMANDS_DEBUG_ALREADYRUNNING,
    translation::java::COMMANDS_DEBUG_ALREADYRUNNING,
);

const NOT_RUNNING_ERROR_TYPE: CommandErrorType<0> = CommandErrorType::new(
    translation::java::COMMANDS_DEBUG_NOTRUNNING,
    translation::bedrock::COMMANDS_DEBUG_NOTSTARTED,
);

const NO_RECURSION_ERROR_TYPE: CommandErrorType<0> = CommandErrorType::new(
    translation::java::COMMANDS_DEBUG_FUNCTION_NORECURSION,
    translation::java::COMMANDS_DEBUG_FUNCTION_NORECURSION,
);

const TRACE_FAILED_ERROR_TYPE: CommandErrorType<0> = CommandErrorType::new(
    translation::java::COMMANDS_DEBUG_FUNCTION_TRACEFAILED,
    translation::java::COMMANDS_DEBUG_FUNCTION_TRACEFAILED,
);

struct DebugStartExecutor;

impl CommandExecutor for DebugStartExecutor {
    fn execute(&self, context: &CommandContext) -> CommandExecutorResult {
        let server = context.server();
        let current_tick = server.tick_count.load(Ordering::Relaxed);
        server.debug_profiler.start(current_tick).map_err(
            |StartDebugProfileError::AlreadyRunning| {
                ALREADY_RUNNING_ERROR_TYPE.create_without_context()
            },
        )?;

        context.source.send_feedback(
            TextComponent::translate_cross(
                translation::java::COMMANDS_DEBUG_STARTED,
                translation::bedrock::COMMANDS_DEBUG_START,
                [],
            ),
            true,
        );

        Ok(1)
    }
}

struct DebugStopExecutor;

impl CommandExecutor for DebugStopExecutor {
    fn execute(&self, context: &CommandContext) -> CommandExecutorResult {
        let server = context.server();
        let current_tick = server.tick_count.load(Ordering::Relaxed);
        let result = server.debug_profiler.stop(current_tick).map_err(
            |StopDebugProfileError::NotRunning| NOT_RUNNING_ERROR_TYPE.create_without_context(),
        )?;

        let seconds = result.duration.as_secs_f64();
        let tps = result.ticks_per_second();
        let arguments = [
            TextComponent::text(format!("{seconds:.2}")),
            TextComponent::text(result.ticks.to_string()),
            TextComponent::text(format!("{tps:.2}")),
        ];
        let feedback = if matches!(context.source.output, CommandSender::Player(_)) {
            TextComponent::translate_cross(
                translation::java::COMMANDS_DEBUG_STOPPED,
                translation::bedrock::COMMANDS_DEBUG_STOP,
                arguments,
            )
        } else {
            // Bedrock's debug-stop translation uses printf-style placeholders that the
            // server-side console renderer cannot resolve. Non-player command sources need
            // an already-rendered message; players still receive their native translation.
            TextComponent::text(format!(
                "Stopped tick profiling after {seconds:.2} second(s) and {} tick(s) ({tps:.2} tick(s) per second)",
                result.ticks
            ))
        };
        context.source.send_feedback(feedback, true);

        Ok(result.command_result())
    }
}

/// Clears the thread-local trace writer on every exit path, including errors.
struct FunctionTraceGuard;

impl Drop for FunctionTraceGuard {
    fn drop(&mut self) {
        DatapackManager::finish_function_trace();
    }
}

fn trace_timestamp() -> String {
    let now = time::OffsetDateTime::now_local().unwrap_or_else(|_| time::OffsetDateTime::now_utc());
    now.format(time::macros::format_description!(
        "[year]-[month]-[day]_[hour].[minute].[second]"
    ))
    .unwrap_or_else(|_| "unknown".to_string())
}

struct DebugFunctionExecutor;

impl CommandExecutor for DebugFunctionExecutor {
    fn execute(&self, context: &CommandContext) -> CommandExecutorResult {
        let name = FunctionArgumentType::get(context, "name")?;

        // Vanilla's `TraceCustomExecutor` refuses to trace while a tracer is
        // already set. Pumpkin also refuses inside a running function, because
        // the call would only be queued and the trace would close too early.
        if DatapackManager::is_function_trace_active()
            || DatapackManager::is_function_execution_active()
        {
            return Err(NO_RECURSION_ERROR_TYPE.create_without_context());
        }

        let server = context.server();
        let output_name = format!("debug-trace-{}.txt", trace_timestamp());
        // Vanilla `MinecraftServer.getFile("debug")` resolves against the
        // server directory, next to `logs`.
        let debug_dir = std::env::current_dir()
            .unwrap_or_else(|_| std::path::PathBuf::from("."))
            .join("debug");
        if let Err(error) = fs::create_dir_all(&debug_dir) {
            tracing::warn!("Failed to create the debug trace directory: {error}");
            return Err(TRACE_FAILED_ERROR_TYPE.create_without_context());
        }
        let file = match fs::File::create(debug_dir.join(&output_name)) {
            Ok(file) => file,
            Err(error) => {
                tracing::warn!("Failed to create the debug trace file: {error}");
                return Err(TRACE_FAILED_ERROR_TYPE.create_without_context());
            }
        };
        if !DatapackManager::start_function_trace(file) {
            return Err(NO_RECURSION_ERROR_TYPE.create_without_context());
        }
        let _trace = FunctionTraceGuard;

        let executed = match server
            .datapack_manager
            .execute_function(server, &context.source, name)
        {
            Ok(executed) => executed,
            Err(FunctionCallError::Unknown(_)) => {
                return Err(ERROR_UNKNOWN_FUNCTION
                    .create_without_context(TextComponent::text(name.to_string())));
            }
            Err(FunctionCallError::Internal(message)) => {
                tracing::error!("Internal error tracing function {name}: {message}");
                return Err(ERROR_FUNCTION_INTERNAL.create_without_context());
            }
        };

        let function_count = DatapackManager::traced_function_count();
        let feedback = if name.starts_with('#') {
            TextComponent::translate_cross(
                translation::java::COMMANDS_DEBUG_FUNCTION_SUCCESS_MULTIPLE,
                translation::java::COMMANDS_DEBUG_FUNCTION_SUCCESS_MULTIPLE,
                [
                    TextComponent::text(executed.to_string()),
                    TextComponent::text(function_count.to_string()),
                    TextComponent::text(output_name),
                ],
            )
        } else {
            TextComponent::translate_cross(
                translation::java::COMMANDS_DEBUG_FUNCTION_SUCCESS_SINGLE,
                translation::java::COMMANDS_DEBUG_FUNCTION_SUCCESS_SINGLE,
                [
                    TextComponent::text(executed.to_string()),
                    TextComponent::text(name.to_string()),
                    TextComponent::text(output_name),
                ],
            )
        };
        context.source.send_feedback(feedback, true);

        Ok(i32::try_from(executed).unwrap_or(i32::MAX))
    }
}

pub fn register(dispatcher: &mut CommandDispatcher, registry: &PermissionRegistry) {
    registry.register_permission_or_panic(Permission::new(
        PERMISSION,
        DESCRIPTION,
        PermissionDefault::Op(PermissionLvl::Three),
    ));

    dispatcher.register(
        command("debug", DESCRIPTION)
            .requires(PERMISSION)
            .then(literal("start").executes(DebugStartExecutor))
            .then(literal("stop").executes(DebugStopExecutor))
            .then(
                literal("function").then(
                    argument("name", FunctionArgumentType)
                        .suggests(FunctionSuggestionProvider)
                        .executes(DebugFunctionExecutor),
                ),
            ),
    );
}
