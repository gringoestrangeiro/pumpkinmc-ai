use std::sync::Arc;
use std::sync::atomic::{AtomicI32, Ordering};

use pumpkin_data::tracked_data::command_block_minecart::{
    DATA_ID_COMMAND_NAME, DATA_ID_LAST_OUTPUT,
};
use pumpkin_nbt::compound::NbtCompound;
use pumpkin_util::math::position::BlockPos;
use pumpkin_util::math::vector2::Vector2;
use pumpkin_util::text::TextComponent;

use crate::block::entities::command_block::CommandBlockEntity;
use crate::command::{CommandSender, CommandSource};
use crate::entity::Entity;
use crate::server::Server;
use crate::world::World;

/// Vanilla `MinecartCommandBlock`. The command state is a `CommandBlockEntity`,
/// Pumpkin's equivalent of vanilla `BaseCommandBlock`.
pub(super) struct CommandBlockMinecart {
    state: Arc<CommandBlockEntity>,
    tick_count: AtomicI32,
    last_activated: AtomicI32,
}

impl CommandBlockMinecart {
    /// Vanilla `MinecartCommandBlock.ACTIVATION_DELAY`.
    const ACTIVATION_DELAY: i32 = 4;
    /// Vanilla `BaseCommandBlock.performCommand` easter egg output.
    const SEARGE_OUTPUT: &'static str = "#itzlipofutzli";

    pub(super) fn new(position: BlockPos) -> Self {
        Self {
            // Vanilla `BaseCommandBlock` tracks output by default.
            state: Arc::new(CommandBlockEntity::new(position, true, false)),
            tick_count: AtomicI32::new(0),
            last_activated: AtomicI32::new(0),
        }
    }

    pub(super) fn tick(&self) {
        self.tick_count.fetch_add(1, Ordering::Relaxed);
    }

    /// Vanilla `MinecartCommandBlock.activateMinecart` for a powered activator rail.
    pub(super) fn activate(&self, entity: &Entity) {
        let tick_count = self.tick_count.load(Ordering::Relaxed);
        if tick_count - self.last_activated.load(Ordering::Relaxed) < Self::ACTIVATION_DELAY {
            return;
        }

        self.perform_command(entity);
        self.last_activated.store(tick_count, Ordering::Relaxed);
    }

    /// Vanilla `BaseCommandBlock.performCommand`.
    fn perform_command(&self, entity: &Entity) -> bool {
        let world = entity.world.load();
        let game_time = world.get_world_age();
        if self.state.last_execution.load(Ordering::Relaxed) == game_time {
            return false;
        }

        let command = self.command();
        if command.eq_ignore_ascii_case("Searge") {
            self.set_last_output(Self::SEARGE_OUTPUT);
            self.state.success_count.store(1, Ordering::Relaxed);
            return true;
        }

        self.state.success_count.store(0, Ordering::Relaxed);
        if world.level_info.load().game_rules.command_blocks_work && !command.is_empty() {
            self.set_last_output("");
            if let Some(server) = world.server.upgrade() {
                let source = self.create_command_source(&world, entity, &server);
                server
                    .command_dispatcher
                    .load()
                    .handle_command(&source, &command);
            }
        }

        if self.state.update_last_execution.load(Ordering::Relaxed) {
            self.state
                .last_execution
                .store(game_time, Ordering::Relaxed);
        } else {
            self.state.last_execution.store(-1, Ordering::Relaxed);
        }
        self.sync_data(entity);

        true
    }

    /// Vanilla `MinecartCommandBlock.MinecartCommandBase.createCommandSourceStack`.
    ///
    /// The source follows the cart, so the position and rotation come from the
    /// entity rather than from the stored command block state.
    fn create_command_source(
        &self,
        world: &Arc<World>,
        entity: &Entity,
        server: &Arc<Server>,
    ) -> CommandSource {
        let name = TextComponent::text("@");
        CommandSource::new(
            CommandSender::CommandBlock(self.state.clone(), world.clone()),
            world.clone(),
            world.get_entity_by_id(entity.entity_id),
            entity.pos.load(),
            Vector2::new(entity.pitch.load(), entity.yaw.load()),
            name.clone().get_text(),
            name,
            server.clone(),
        )
    }

    /// Vanilla `BaseCommandBlock.setCommand`; the editor packet then calls this
    /// followed by `setTrackOutput` and an update of the synced data.
    pub(super) fn set_command(&self, entity: &Entity, command: &str, track_output: bool) {
        *self
            .state
            .command
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner) = command.to_string();
        self.state.success_count.store(0, Ordering::Relaxed);
        self.state
            .track_output
            .store(track_output, Ordering::Relaxed);
        if !track_output {
            self.set_last_output("");
        }
        self.sync_data(entity);
    }

    pub(super) fn write_nbt(&self, nbt: &mut NbtCompound) {
        self.state.write_base_nbt(nbt);
    }

    pub(super) fn read_nbt(&self, nbt: &NbtCompound) {
        self.state.read_base_nbt(nbt);
    }

    fn command(&self) -> String {
        self.state
            .command
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .clone()
    }

    fn last_output(&self) -> String {
        self.state
            .last_output
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .clone()
    }

    fn set_last_output(&self, last_output: &str) {
        *self
            .state
            .last_output
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner) = last_output.to_string();
    }

    /// Vanilla `MinecartCommandBlock.MinecartCommandBase.onUpdated`, plus the
    /// editor state the client reads from the synced entity data.
    pub(super) fn sync_data(&self, entity: &Entity) {
        entity.set_synced_data(DATA_ID_COMMAND_NAME, self.command());
        entity.set_synced_data(DATA_ID_LAST_OUTPUT, TextComponent::text(self.last_output()));
    }
}

#[cfg(test)]
mod tests {
    use super::CommandBlockMinecart;
    use pumpkin_nbt::compound::NbtCompound;
    use pumpkin_util::math::position::BlockPos;
    use std::sync::atomic::Ordering;

    #[test]
    fn command_block_minecart_round_trips_base_command_block_nbt() {
        let cart = CommandBlockMinecart::new(BlockPos::ZERO);
        *cart
            .state
            .command
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner) = "say hi".to_string();
        cart.state.success_count.store(2, Ordering::SeqCst);
        cart.state.last_execution.store(42, Ordering::SeqCst);
        cart.set_last_output("[00:00:00] hi");

        let mut nbt = NbtCompound::new();
        cart.write_nbt(&mut nbt);

        let loaded = CommandBlockMinecart::new(BlockPos::ZERO);
        loaded.read_nbt(&nbt);

        assert_eq!(loaded.command(), "say hi");
        assert_eq!(loaded.last_output(), "[00:00:00] hi");
        assert_eq!(loaded.state.success_count.load(Ordering::SeqCst), 2);
        assert_eq!(loaded.state.last_execution.load(Ordering::SeqCst), 42);
        assert!(loaded.state.track_output.load(Ordering::SeqCst));
    }
}
