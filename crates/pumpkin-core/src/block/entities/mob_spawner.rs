use std::sync::{
    Arc,
    atomic::{AtomicI32, Ordering},
};

use crossbeam::atomic::AtomicCell;
use pumpkin_data::{entity::EntityType, world::WorldEvent};
use pumpkin_nbt::compound::NbtCompound;
use pumpkin_util::math::{position::BlockPos, vector3::Vector3};

use crate::{
    block::entities::BlockEntity, entity::EntityBase, entity::mob::spawn::finalize_spawn,
    world::World,
};

/// Vanilla `BaseSpawner`. Holds the spawner state and ticks it at a position
/// supplied by the owner, which is a spawner block or a spawner minecart.
pub struct BaseSpawner {
    pub delay: AtomicI32,
    pub max_delay: AtomicI32,
    pub min_delay: AtomicI32,
    pub spawn_count: AtomicI32,
    pub spawn_range: AtomicI32,
    pub max_nearby_entities: AtomicI32,
    pub required_player_range: AtomicI32,
    pub entity_type: AtomicCell<Option<&'static EntityType>>,
}

impl BaseSpawner {
    const DEFAULT_DELAY: i32 = 20;
    const DEFAULT_MAX_SPAWN_DELAY: i32 = 800;
    const DEFAULT_MIN_SPAWN_DELAY: i32 = 200;
    const DEFAULT_SPAWN_COUNT: i32 = 4;
    const DEFAULT_SPAWN_RANGE: i32 = 4;
    const DEFAULT_MAX_NEARBY_ENTITIES: i32 = 6;
    const DEFAULT_REQUIRED_PLAYER_RANGE: i32 = 16;

    #[must_use]
    pub const fn new(entity_type: Option<&'static EntityType>) -> Self {
        Self {
            delay: AtomicI32::new(Self::DEFAULT_DELAY),
            max_delay: AtomicI32::new(Self::DEFAULT_MAX_SPAWN_DELAY),
            min_delay: AtomicI32::new(Self::DEFAULT_MIN_SPAWN_DELAY),
            spawn_count: AtomicI32::new(Self::DEFAULT_SPAWN_COUNT),
            spawn_range: AtomicI32::new(Self::DEFAULT_SPAWN_RANGE),
            max_nearby_entities: AtomicI32::new(Self::DEFAULT_MAX_NEARBY_ENTITIES),
            required_player_range: AtomicI32::new(Self::DEFAULT_REQUIRED_PLAYER_RANGE),
            entity_type: AtomicCell::new(entity_type),
        }
    }

    fn update_spawns(&self) {
        let min_delay = self.min_delay.load(Ordering::Relaxed);
        let max_delay = self.max_delay.load(Ordering::Relaxed);

        self.delay.store(
            if max_delay <= min_delay {
                min_delay
            } else {
                min_delay + rand::random_range(0..max_delay - min_delay)
            },
            Ordering::Relaxed,
        );
    }

    pub fn set_entity_type(&self, entity_type: &'static EntityType) {
        self.entity_type.store(Some(entity_type));
    }

    /// Vanilla `BaseSpawner.serverTick`. Returns whether the spawn delay was
    /// reset, in which case the owner broadcasts its own `broadcastEvent(1)`.
    #[allow(clippy::too_many_lines)]
    pub fn server_tick(&self, world: &Arc<World>, position: BlockPos) -> bool {
        if !world.level_info.load().game_rules.spawner_blocks_work {
            return false;
        }
        let Some(entity_type) = self.entity_type.load() else {
            return false;
        };
        let spawn_range = self.spawn_range.load(Ordering::Relaxed);

        let center = position.to_centered_f64();
        let max_player_dist_sq =
            (self.required_player_range.load(Ordering::Relaxed) as f64).powi(2);
        let player_nearby = world.players.load().iter().any(|p| {
            p.get_entity().pos.load().squared_distance_to_vec(&center) <= max_player_dist_sq
        });

        if !player_nearby {
            return false;
        }

        let delay_reset = self.delay.load(Ordering::Relaxed) == -1;
        if delay_reset {
            self.update_spawns();
        }

        let delay = self.delay.load(Ordering::Relaxed);
        if delay > 0 {
            self.delay.store(delay - 1, Ordering::Relaxed);
            return delay_reset;
        }

        let search_radius_horiz = (spawn_range * 2) as f64;
        let search_radius_vert = 4.0;
        let nearby_count = world
            .entities
            .load()
            .iter()
            .filter(|e| {
                let ent = e.get_entity();
                if ent.entity_type.id != entity_type.id {
                    return false;
                }
                let pos = ent.pos.load();
                (pos.x - center.x).abs() <= search_radius_horiz
                    && (pos.z - center.z).abs() <= search_radius_horiz
                    && (pos.y - center.y).abs() <= search_radius_vert
            })
            .count();

        if nearby_count as i32 >= self.max_nearby_entities.load(Ordering::Relaxed) {
            self.update_spawns();
            return true;
        }

        let mut spawned_any = false;
        let mut cancelled_any = false;
        for _ in 0..self.spawn_count.load(Ordering::Relaxed) {
            let pos = position.0;

            let spawn_pos = Vector3::new(
                pos.x as f64
                    + (rand::random::<f64>() - rand::random::<f64>()) * spawn_range as f64
                    + 0.5,
                (pos.y + rand::random_range(0..3) - 1) as f64,
                pos.z as f64
                    + (rand::random::<f64>() - rand::random::<f64>()) * spawn_range as f64
                    + 0.5,
            );
            if !world.is_space_empty(entity_type.get_spawn_bounding_box(
                spawn_pos.x,
                spawn_pos.y,
                spawn_pos.z,
            )) {
                continue;
            }
            let entity = crate::entity::r#type::from_type(
                entity_type,
                spawn_pos,
                world,
                uuid::Uuid::new_v4(),
            );
            let yaw = rand::random::<f32>() * 360.0;
            entity.get_entity().set_rotation(yaw, 0.0);

            let mut event =
                crate::plugin::api::events::entity::spawner_spawn::SpawnerSpawnEvent::new(
                    entity.get_entity().entity_id,
                    position,
                );
            if let Some(server) = world.server.upgrade() {
                server.plugin_manager.fire_blocking(&server, &mut event);
            }
            if event.cancelled {
                // Count a cancelled spawn as a completed attempt so the
                // spawner goes on its normal cooldown instead of retrying
                // every tick for as long as a plugin keeps cancelling.
                cancelled_any = true;
                continue;
            }

            // TODO: support SpawnData NBT (`entity` tag, custom_spawn_rules, equipment).
            finalize_spawn(&entity, world, None);
            world.spawn_entity(entity);
            world.sync_world_event(WorldEvent::ParticlesMobblockSpawn, position, 0);
            spawned_any = true;
        }
        if spawned_any || cancelled_any {
            self.update_spawns();
            return true;
        }

        delay_reset
    }

    pub fn write_nbt(&self, nbt: &mut NbtCompound) {
        nbt.put_short("Delay", self.delay.load(Ordering::Relaxed) as i16);
        nbt.put_short(
            "MinSpawnDelay",
            self.min_delay.load(Ordering::Relaxed) as i16,
        );
        nbt.put_short(
            "MaxSpawnDelay",
            self.max_delay.load(Ordering::Relaxed) as i16,
        );
        nbt.put_short(
            "SpawnCount",
            self.spawn_count.load(Ordering::Relaxed) as i16,
        );
        nbt.put_short(
            "SpawnRange",
            self.spawn_range.load(Ordering::Relaxed) as i16,
        );
        nbt.put_short(
            "MaxNearbyEntities",
            self.max_nearby_entities.load(Ordering::Relaxed) as i16,
        );
        nbt.put_short(
            "RequiredPlayerRange",
            self.required_player_range.load(Ordering::Relaxed) as i16,
        );

        if let Some(entity_type) = self.entity_type.load() {
            let mut spawn_entry = NbtCompound::new();

            let mut entity_nbt = NbtCompound::new();
            entity_nbt.put_string("id", format!("minecraft:{}", entity_type.resource_name));

            spawn_entry.put_compound("entity", entity_nbt);

            nbt.put_compound("SpawnData", spawn_entry);
        }
    }

    pub fn read_nbt(&self, nbt: &NbtCompound) {
        let get_num = |name: &str| {
            nbt.get_short(name)
                .map(i32::from)
                .or_else(|| nbt.get_int(name))
                .or_else(|| nbt.get_byte(name).map(i32::from))
        };

        self.delay.store(
            get_num("Delay").unwrap_or(Self::DEFAULT_DELAY),
            Ordering::Relaxed,
        );
        self.min_delay.store(
            get_num("MinSpawnDelay").unwrap_or(Self::DEFAULT_MIN_SPAWN_DELAY),
            Ordering::Relaxed,
        );
        self.max_delay.store(
            get_num("MaxSpawnDelay").unwrap_or(Self::DEFAULT_MAX_SPAWN_DELAY),
            Ordering::Relaxed,
        );
        self.spawn_count.store(
            get_num("SpawnCount").unwrap_or(Self::DEFAULT_SPAWN_COUNT),
            Ordering::Relaxed,
        );
        self.spawn_range.store(
            get_num("SpawnRange").unwrap_or(Self::DEFAULT_SPAWN_RANGE),
            Ordering::Relaxed,
        );
        self.max_nearby_entities.store(
            get_num("MaxNearbyEntities").unwrap_or(Self::DEFAULT_MAX_NEARBY_ENTITIES),
            Ordering::Relaxed,
        );
        self.required_player_range.store(
            get_num("RequiredPlayerRange").unwrap_or(Self::DEFAULT_REQUIRED_PLAYER_RANGE),
            Ordering::Relaxed,
        );

        let entity_type = nbt
            .get_compound("SpawnData")
            .and_then(|data| {
                data.get_compound("entity")
                    .and_then(|entity| entity.get_string("id"))
                    .or_else(|| data.get_string("id"))
            })
            .or_else(|| {
                nbt.get_list("SpawnPotentials")
                    .and_then(|list| list.first())
                    .and_then(|tag| tag.extract_compound())
                    .and_then(|entry| {
                        entry
                            .get_compound("data")
                            .and_then(|data| {
                                data.get_compound("entity")
                                    .and_then(|entity| entity.get_string("id"))
                                    .or_else(|| data.get_string("id"))
                            })
                            .or_else(|| {
                                entry
                                    .get_compound("entity")
                                    .and_then(|entity| entity.get_string("id"))
                            })
                    })
            })
            .or_else(|| nbt.get_string("EntityId"))
            .and_then(EntityType::from_name);

        self.entity_type.store(entity_type);
    }
}

pub struct MobSpawnerBlockEntity {
    pub position: BlockPos,
    pub spawner: BaseSpawner,
}

impl MobSpawnerBlockEntity {
    pub const ID: &'static str = "minecraft:mob_spawner";

    #[must_use]
    pub const fn new(position: BlockPos, entity_type: Option<&'static EntityType>) -> Self {
        Self {
            position,
            spawner: BaseSpawner::new(entity_type),
        }
    }

    pub fn set_entity_type(&self, entity_type: &'static EntityType) {
        self.spawner.set_entity_type(entity_type);
    }
}

impl BlockEntity for MobSpawnerBlockEntity {
    fn resource_location(&self) -> &'static str {
        Self::ID
    }

    fn get_position(&self) -> BlockPos {
        self.position
    }

    fn tick(&self, world: &Arc<World>) {
        if self.spawner.server_tick(world, self.position) {
            world.add_synced_block_event(self.position, 1, 0);
        }
    }

    fn from_nbt(nbt: &pumpkin_nbt::compound::NbtCompound, position: BlockPos) -> Self
    where
        Self: Sized,
    {
        let spawner = BaseSpawner::new(None);
        spawner.read_nbt(nbt);
        Self { position, spawner }
    }

    fn write_nbt(&self, nbt: &mut NbtCompound) {
        self.spawner.write_nbt(nbt);
    }

    fn chunk_data_nbt(&self) -> Option<NbtCompound> {
        let mut nbt = NbtCompound::new();
        self.spawner.write_nbt(&mut nbt);
        Some(nbt)
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}

#[cfg(test)]
mod tests {
    use super::BaseSpawner;
    use pumpkin_nbt::compound::NbtCompound;
    use std::sync::atomic::Ordering;

    #[test]
    fn base_spawner_round_trips_nbt() {
        let spawner = BaseSpawner::new(Some(&pumpkin_data::entity::EntityType::ZOMBIE));
        spawner.delay.store(42, Ordering::Relaxed);
        spawner.min_delay.store(100, Ordering::Relaxed);
        spawner.max_delay.store(400, Ordering::Relaxed);
        spawner.spawn_count.store(2, Ordering::Relaxed);
        spawner.spawn_range.store(6, Ordering::Relaxed);
        spawner.max_nearby_entities.store(9, Ordering::Relaxed);
        spawner.required_player_range.store(12, Ordering::Relaxed);

        let mut nbt = NbtCompound::new();
        spawner.write_nbt(&mut nbt);

        let loaded = BaseSpawner::new(None);
        loaded.read_nbt(&nbt);

        assert_eq!(loaded.delay.load(Ordering::Relaxed), 42);
        assert_eq!(loaded.min_delay.load(Ordering::Relaxed), 100);
        assert_eq!(loaded.max_delay.load(Ordering::Relaxed), 400);
        assert_eq!(loaded.spawn_count.load(Ordering::Relaxed), 2);
        assert_eq!(loaded.spawn_range.load(Ordering::Relaxed), 6);
        assert_eq!(loaded.max_nearby_entities.load(Ordering::Relaxed), 9);
        assert_eq!(loaded.required_player_range.load(Ordering::Relaxed), 12);
        assert_eq!(
            loaded.entity_type.load().map(|entity_type| entity_type.id),
            Some(pumpkin_data::entity::EntityType::ZOMBIE.id)
        );
    }
}
