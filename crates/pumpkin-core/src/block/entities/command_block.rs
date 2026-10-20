use std::sync::{
    Mutex as StdMutex,
    atomic::{AtomicBool, AtomicI64, AtomicU32, Ordering},
};

use pumpkin_nbt::compound::NbtCompound;
use pumpkin_util::math::position::BlockPos;

use super::BlockEntity;

// todo: CustomName
pub struct CommandBlockEntity {
    pub position: BlockPos,
    pub powered: AtomicBool,
    pub condition_met: AtomicBool,
    pub auto: AtomicBool,
    pub dirty: AtomicBool,
    pub command: StdMutex<String>,
    pub last_output: StdMutex<String>,
    pub track_output: AtomicBool,
    pub success_count: AtomicU32,
    pub update_last_execution: AtomicBool,
    pub last_execution: AtomicI64,
}

impl CommandBlockEntity {
    pub const ID: &'static str = "minecraft:command_block";
    #[must_use]
    pub const fn new(position: BlockPos, track_output: bool, is_chain: bool) -> Self {
        Self {
            position,
            powered: AtomicBool::new(false),
            condition_met: AtomicBool::new(false),
            auto: AtomicBool::new(is_chain),
            dirty: AtomicBool::new(false),
            command: StdMutex::new(String::new()),
            last_output: StdMutex::new(String::new()),
            track_output: AtomicBool::new(track_output),
            success_count: AtomicU32::new(0),
            update_last_execution: AtomicBool::new(true),
            last_execution: AtomicI64::new(-1),
        }
    }

    fn command(&self) -> String {
        self.command
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .clone()
    }

    fn last_output(&self) -> String {
        self.last_output
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .clone()
    }

    /// Vanilla `BaseCommandBlock.save`.
    pub fn write_base_nbt(&self, nbt: &mut NbtCompound) {
        nbt.put_string("Command", self.command());
        nbt.put_int(
            "SuccessCount",
            self.success_count.load(Ordering::SeqCst).cast_signed(),
        );
        let track_output = self.track_output.load(Ordering::SeqCst);
        nbt.put_bool("TrackOutput", track_output);
        if track_output {
            nbt.put_string("LastOutput", self.last_output());
        }

        let update_last_execution = self.update_last_execution.load(Ordering::SeqCst);
        nbt.put_bool("UpdateLastExecution", update_last_execution);
        let last_execution = self.last_execution.load(Ordering::SeqCst);
        if update_last_execution && last_execution != -1 {
            nbt.put_long("LastExecution", last_execution);
        }
    }

    /// Vanilla `BaseCommandBlock.load`.
    pub fn read_base_nbt(&self, nbt: &NbtCompound) {
        *self
            .command
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner) =
            nbt.get_string("Command").unwrap_or("").to_string();
        self.success_count.store(
            nbt.get_int("SuccessCount").unwrap_or(0).cast_unsigned(),
            Ordering::SeqCst,
        );

        let track_output = nbt.get_bool("TrackOutput").unwrap_or(true);
        self.track_output.store(track_output, Ordering::SeqCst);
        *self
            .last_output
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner) = if track_output {
            nbt.get_string("LastOutput").unwrap_or("").to_string()
        } else {
            String::new()
        };

        let update_last_execution = nbt.get_bool("UpdateLastExecution").unwrap_or(true);
        self.update_last_execution
            .store(update_last_execution, Ordering::SeqCst);
        self.last_execution.store(
            if update_last_execution {
                nbt.get_long("LastExecution").unwrap_or(-1)
            } else {
                -1
            },
            Ordering::SeqCst,
        );
    }

    fn write_sync_nbt(&self, nbt: &mut NbtCompound) {
        self.write_base_nbt(nbt);
        nbt.put_bool("auto", self.auto.load(Ordering::SeqCst));
        nbt.put_bool("conditionMet", self.condition_met.load(Ordering::SeqCst));
        nbt.put_bool("powered", self.powered.load(Ordering::SeqCst));
    }
}

impl BlockEntity for CommandBlockEntity {
    fn resource_location(&self) -> &'static str {
        Self::ID
    }
    fn get_position(&self) -> BlockPos {
        self.position
    }

    fn from_nbt(nbt: &pumpkin_nbt::compound::NbtCompound, position: BlockPos) -> Self
    where
        Self: Sized,
    {
        let entity = Self::new(
            position,
            nbt.get_bool("TrackOutput").unwrap_or(true),
            nbt.get_bool("auto").unwrap_or(false),
        );
        entity.read_base_nbt(nbt);
        entity.condition_met.store(
            nbt.get_bool("conditionMet").unwrap_or(false),
            Ordering::SeqCst,
        );
        entity
            .powered
            .store(nbt.get_bool("powered").unwrap_or(false), Ordering::SeqCst);
        entity
    }

    fn write_nbt(&self, nbt: &mut NbtCompound) {
        self.write_sync_nbt(nbt);
    }

    fn chunk_data_nbt(&self) -> Option<NbtCompound> {
        let mut nbt = NbtCompound::new();
        self.write_sync_nbt(&mut nbt);
        Some(nbt)
    }

    fn is_dirty(&self) -> bool {
        self.dirty.load(Ordering::Relaxed)
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}
