//! Vanilla `WanderingTraderSpawner` (`net.minecraft.world.entity.npc.wanderingtrader`).
//!
//! Registered on the overworld only, like vanilla's `ServerLevel` custom spawners.

use std::sync::{Arc, PoisonError};

use pumpkin_data::entity::{EntityType, SpawnLocation};
use pumpkin_data::tag::Taggable;
use pumpkin_nbt::compound::NbtCompound;
use pumpkin_nbt::tag::NbtTag;
use pumpkin_util::HeightMap;
use pumpkin_util::math::position::BlockPos;
use pumpkin_util::math::vector3::Vector3;
use pumpkin_world::chunk::ChunkHeightmapType;
use uuid::Uuid;

use crate::entity::passive::trader_llama::TraderLlamaEntity;
use crate::entity::passive::wandering_trader::WanderingTraderEntity;
use crate::entity::{Entity, EntityBase};
use crate::world::World;
use crate::world::natural_spawner::{is_valid_empty_spawn_block, is_valid_spawn_floor};

/// Vanilla `WanderingTraderSpawner.DEFAULT_TICK_DELAY`.
pub const DEFAULT_TICK_DELAY: i32 = 1200;
/// Vanilla `WanderingTraderSpawner.DEFAULT_SPAWN_DELAY`.
pub const DEFAULT_SPAWN_DELAY: i32 = 24000;
/// Vanilla `WanderingTraderSpawner.MIN_SPAWN_CHANCE`.
pub const MIN_SPAWN_CHANCE: i32 = 25;
/// Vanilla `WanderingTraderSpawner.MAX_SPAWN_CHANCE`.
const MAX_SPAWN_CHANCE: i32 = 75;
/// Vanilla `WanderingTraderSpawner.SPAWN_CHANCE_INCREASE`.
const SPAWN_CHANCE_INCREASE: i32 = 25;
/// Vanilla `WanderingTraderSpawner.SPAWN_ONE_IN_X_CHANCE`.
const SPAWN_ONE_IN_X_CHANCE: i32 = 10;
/// Vanilla `WanderingTraderSpawner.NUMBER_OF_SPAWN_ATTEMPTS`.
const NUMBER_OF_SPAWN_ATTEMPTS: i32 = 10;
/// Vanilla `WanderingTraderSpawner.spawn`'s search radius around the meeting point.
const SEARCH_RADIUS: i32 = 48;
/// Vanilla `WanderingTraderSpawner.spawn`'s `trader.setDespawnDelay(48000)`.
const TRADER_DESPAWN_DELAY: i32 = 48000;
/// Vanilla `WanderingTraderSpawner.spawn`'s llama loop count.
const TRADER_LLAMA_COUNT: i32 = 2;
/// Vanilla `WanderingTraderSpawner.tryToSpawnLlamaFor`'s radius.
const TRADER_LLAMA_RADIUS: i32 = 4;

/// Namespace the spawn timer is persisted under, matching vanilla's
/// `WanderingTraderData.TYPE` id.
pub const SAVED_DATA_NAMESPACE: &str = "minecraft:wandering_trader";
/// Vanilla `WanderingTraderData.CODEC`'s `spawn_delay` field.
pub const SPAWN_DELAY_KEY: &str = "spawn_delay";
/// Vanilla `WanderingTraderData.CODEC`'s `spawn_chance` field.
pub const SPAWN_CHANCE_KEY: &str = "spawn_chance";

/// Vanilla `WanderingTraderData`: the spawn delay and chance that survive a restart.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct WanderingTraderData {
    pub spawn_delay: i32,
    pub spawn_chance: i32,
}

impl Default for WanderingTraderData {
    fn default() -> Self {
        Self {
            spawn_delay: DEFAULT_SPAWN_DELAY,
            spawn_chance: MIN_SPAWN_CHANCE,
        }
    }
}

/// Reads the saved spawn timer from a world's persisted custom data.
///
/// Pumpkin keeps vanilla's `WanderingTraderData` fields in
/// `pumpkin_custom_data.nbt` instead of vanilla's per-dimension
/// `data/wandering_trader.dat`.
#[must_use]
pub fn stored_data(custom_data: &NbtCompound) -> WanderingTraderData {
    let Some(NbtTag::Compound(namespace)) = custom_data.child_tags.get(SAVED_DATA_NAMESPACE) else {
        return WanderingTraderData::default();
    };
    WanderingTraderData {
        spawn_delay: namespace
            .get_int(SPAWN_DELAY_KEY)
            .unwrap_or(DEFAULT_SPAWN_DELAY),
        spawn_chance: namespace
            .get_int(SPAWN_CHANCE_KEY)
            .unwrap_or(MIN_SPAWN_CHANCE),
    }
}

/// Vanilla `WanderingTraderSpawner`.
pub struct WanderingTraderSpawner {
    tick_delay: i32,
    data: WanderingTraderData,
}

impl WanderingTraderSpawner {
    #[must_use]
    pub const fn new(data: WanderingTraderData) -> Self {
        Self {
            tick_delay: DEFAULT_TICK_DELAY,
            data,
        }
    }

    #[must_use]
    pub const fn data(&self) -> WanderingTraderData {
        self.data
    }

    /// Vanilla `WanderingTraderSpawner.tick`.
    pub fn tick(&mut self, world: &Arc<World>) {
        if !world.level_info.load().game_rules.spawn_wandering_traders {
            return;
        }

        if self.advance(rand::random_range(0..100)) && Self::try_spawn_world(world) {
            self.data.spawn_chance = MIN_SPAWN_CHANCE;
        }
    }

    /// Vanilla `WanderingTraderSpawner.tick`'s timer and chance handling.
    ///
    /// `roll` stands in for vanilla's `random.nextInt(100)` and is compared
    /// against the chance the spawner had before this attempt raised it.
    fn advance(&mut self, roll: i32) -> bool {
        self.tick_delay -= 1;
        if self.tick_delay > 0 {
            return false;
        }
        self.tick_delay = DEFAULT_TICK_DELAY;

        self.data.spawn_delay -= DEFAULT_TICK_DELAY;
        if self.data.spawn_delay > 0 {
            return false;
        }
        self.data.spawn_delay = DEFAULT_SPAWN_DELAY;

        let chance = self.data.spawn_chance;
        self.data.spawn_chance =
            (chance + SPAWN_CHANCE_INCREASE).clamp(MIN_SPAWN_CHANCE, MAX_SPAWN_CHANCE);
        roll <= chance
    }

    /// Vanilla `WanderingTraderSpawner.spawn`.
    fn try_spawn_world(world: &Arc<World>) -> bool {
        let Some(reference_pos) = Self::reference_pos(world) else {
            return true;
        };

        if rand::random_range(0..SPAWN_ONE_IN_X_CHANCE) != 0 {
            return false;
        }

        let Some(spawn_pos) = Self::find_spawn_position_near(world, &reference_pos, SEARCH_RADIUS)
        else {
            return false;
        };
        if !Self::has_enough_space(world, &spawn_pos) {
            return false;
        }
        if world
            .get_biome(&spawn_pos)
            .is_tagged_with("minecraft:without_wandering_trader_spawns")
            == Some(true)
        {
            return false;
        }

        let trader = WanderingTraderEntity::new(Entity::from_uuid(
            Uuid::new_v4(),
            world.clone(),
            bottom_center(&spawn_pos),
            &EntityType::WANDERING_TRADER,
        ));
        let spawned_trader: Arc<dyn EntityBase> = trader.clone();
        if !world.spawn_entity(spawned_trader.clone()) {
            return false;
        }

        for _ in 0..TRADER_LLAMA_COUNT {
            Self::try_to_spawn_llama_for(world, &spawned_trader);
        }

        // Vanilla sets these after the llamas spawned.
        trader.set_despawn_delay(TRADER_DESPAWN_DELAY);
        trader.set_wander_target(Some(reference_pos));
        // Vanilla `trader.setHomeTo(referencePos, 16)`.
        trader.mob_entity.set_home_to(reference_pos, 16);
        true
    }

    /// Vanilla's `level.getRandomPlayer()` followed by the meeting POI lookup.
    ///
    /// Pumpkin does not track meeting POIs yet, so there is never a bell within
    /// 48 blocks and the player's own position is the reference point, exactly
    /// like vanilla's `Optional::orElse` fallback. `None` means the level has no
    /// players, which vanilla treats as a completed attempt.
    fn reference_pos(world: &Arc<World>) -> Option<BlockPos> {
        let players = world.players.load();
        if players.is_empty() {
            return None;
        }
        let player = &players[rand::random_range(0..players.len())];
        Some(player.get_entity().block_pos.load())
    }

    /// Vanilla `WanderingTraderSpawner.tryToSpawnLlamaFor`.
    fn try_to_spawn_llama_for(world: &Arc<World>, trader: &Arc<dyn EntityBase>) {
        let Some(spawn_pos) = Self::find_spawn_position_near(
            world,
            &trader.get_entity().block_pos.load(),
            TRADER_LLAMA_RADIUS,
        ) else {
            return;
        };

        let llama = TraderLlamaEntity::new(Entity::from_uuid(
            Uuid::new_v4(),
            world.clone(),
            bottom_center(&spawn_pos),
            &EntityType::TRADER_LLAMA,
        ));
        if world.spawn_entity(llama.clone()) {
            llama.get_entity().leash_to(trader.clone());
        }
    }

    /// Vanilla `WanderingTraderSpawner.findSpawnPositionNear`.
    fn find_spawn_position_near(
        world: &Arc<World>,
        reference: &BlockPos,
        radius: i32,
    ) -> Option<BlockPos> {
        for _ in 0..NUMBER_OF_SPAWN_ATTEMPTS {
            let x = reference.0.x + rand::random_range(0..radius * 2) - radius;
            let z = reference.0.z + rand::random_range(0..radius * 2) - radius;
            let y = world.get_heightmap_height(
                heightmap_type(EntityType::WANDERING_TRADER.spawn_restriction.heightmap),
                x,
                z,
            );
            let spawn_pos = BlockPos::new(x, y, z);
            if is_spawn_position_ok(world, &spawn_pos) {
                return Some(spawn_pos);
            }
        }
        None
    }

    /// Vanilla `WanderingTraderSpawner.hasEnoughSpace`: the 2x3x2 box above the
    /// spawn position must be free of collision shapes.
    fn has_enough_space(world: &Arc<World>, spawn_pos: &BlockPos) -> bool {
        for x in spawn_pos.0.x..=spawn_pos.0.x + 1 {
            for y in spawn_pos.0.y..=spawn_pos.0.y + 2 {
                for z in spawn_pos.0.z..=spawn_pos.0.z + 1 {
                    let pos = BlockPos::new(x, y, z);
                    if world
                        .get_block_state(&pos)
                        .get_block_collision_shapes_at(&pos)
                        .next()
                        .is_some()
                    {
                        return false;
                    }
                }
            }
        }
        true
    }
}

/// Vanilla `SpawnPlacementTypes.ON_GROUND` for the wandering trader.
fn is_spawn_position_ok(world: &Arc<World>, spawn_pos: &BlockPos) -> bool {
    if !world
        .worldborder
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .contains_block(spawn_pos.0.x, spawn_pos.0.z)
    {
        return false;
    }

    let entity_type = &EntityType::WANDERING_TRADER;
    let SpawnLocation::OnGround = entity_type.spawn_restriction.location else {
        return false;
    };

    let below = spawn_pos.down();
    if !is_valid_spawn_floor(world.get_block_state(&below), entity_type) {
        return false;
    }

    is_valid_empty_spawn_block(world.get_block_state(spawn_pos), entity_type)
        && is_valid_empty_spawn_block(world.get_block_state(&spawn_pos.up()), entity_type)
}

/// Vanilla `Vec3.atBottomCenterOf`: an entity's feet in the middle of the block.
fn bottom_center(pos: &BlockPos) -> Vector3<f64> {
    Vector3::new(
        f64::from(pos.0.x) + 0.5,
        f64::from(pos.0.y),
        f64::from(pos.0.z) + 0.5,
    )
}

/// Maps a vanilla `Heightmap.Types` to the chunk heightmap Pumpkin keeps loaded.
const fn heightmap_type(height_map: HeightMap) -> ChunkHeightmapType {
    match height_map {
        HeightMap::WorldSurfaceWg
        | HeightMap::WorldSurface
        | HeightMap::OceanFloorWg
        | HeightMap::OceanFloor => ChunkHeightmapType::WorldSurface,
        HeightMap::MotionBlocking => ChunkHeightmapType::MotionBlocking,
        HeightMap::MotionBlockingNoLeaves => ChunkHeightmapType::MotionBlockingNoLeaves,
    }
}

#[cfg(test)]
mod tests {
    use super::{
        SAVED_DATA_NAMESPACE, SPAWN_CHANCE_KEY, SPAWN_DELAY_KEY, WanderingTraderData,
        WanderingTraderSpawner, stored_data,
    };
    use pumpkin_nbt::compound::NbtCompound;
    use pumpkin_nbt::tag::NbtTag;

    #[test]
    fn spawn_timing_and_chance_follow_vanilla() {
        let mut spawner = WanderingTraderSpawner::new(WanderingTraderData::default());

        let mut attempts = Vec::new();
        for tick in 1..=100_000 {
            if spawner.advance(0) {
                attempts.push((tick, spawner.data().spawn_chance));
            }
        }

        // The first window opens after 1200 ticks, but the 24000 tick spawn
        // delay has to run out first: 20 windows. Each window raises the chance
        // by 25, from the 25 minimum up to the 75 maximum.
        assert_eq!(attempts[0], (24000, 50));
        assert_eq!(attempts[1], (48000, 75));
        assert_eq!(attempts[2], (72000, 75));
        assert_eq!(attempts[3], (96000, 75));
    }

    #[test]
    fn a_roll_above_the_chance_skips_the_spawn_attempt() {
        let mut skipping = WanderingTraderSpawner::new(WanderingTraderData::default());
        let mut passing = WanderingTraderSpawner::new(WanderingTraderData::default());
        for _ in 0..23_999 {
            assert!(!skipping.advance(0));
            assert!(!passing.advance(0));
        }

        // The roll is compared against the chance from before this attempt
        // raised it, so a default chance of 25 accepts 25 and rejects 26.
        assert!(!skipping.advance(26));
        assert!(passing.advance(25));
    }

    #[test]
    fn stored_data_round_trips_and_defaults_to_vanilla_values() {
        let mut custom_data = NbtCompound::new();
        assert_eq!(stored_data(&custom_data), WanderingTraderData::default());

        let mut namespace = NbtCompound::new();
        namespace.put_int(SPAWN_DELAY_KEY, 12000);
        namespace.put_int(SPAWN_CHANCE_KEY, 60);
        custom_data.put(SAVED_DATA_NAMESPACE, NbtTag::Compound(namespace.clone()));

        assert_eq!(
            stored_data(&custom_data),
            WanderingTraderData {
                spawn_delay: 12000,
                spawn_chance: 60,
            }
        );
    }
}
