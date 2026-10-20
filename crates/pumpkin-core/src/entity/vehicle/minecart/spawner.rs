use pumpkin_data::entity::EntityStatus;
use pumpkin_nbt::compound::NbtCompound;

use crate::block::entities::mob_spawner::BaseSpawner;
use crate::entity::Entity;

/// Vanilla `MinecartSpawner`: a spawner that ticks at the cart's position.
pub(super) struct SpawnerMinecart {
    spawner: BaseSpawner,
}

impl SpawnerMinecart {
    pub(super) const fn new() -> Self {
        Self {
            spawner: BaseSpawner::new(None),
        }
    }

    pub(super) fn tick(&self, entity: &Entity) {
        let world = entity.world.load();
        if self.spawner.server_tick(&world, entity.block_pos.load()) {
            // Vanilla `MinecartSpawner.broadcastEvent`: entity event 1 resets the
            // client's spawn delay and spin.
            world.send_entity_status(entity, EntityStatus::Jump, None);
        }
    }

    pub(super) fn write_nbt(&self, nbt: &mut NbtCompound) {
        self.spawner.write_nbt(nbt);
    }

    pub(super) fn read_nbt(&self, nbt: &NbtCompound) {
        self.spawner.read_nbt(nbt);
    }
}
