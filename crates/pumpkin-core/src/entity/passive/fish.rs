//! What every fish shares: vanilla's `AbstractFish` and `AbstractSchoolingFish`.

use std::sync::atomic::{AtomicBool, AtomicI32, Ordering};
use std::sync::{Arc, Mutex, PoisonError};

use pumpkin_data::damage::DamageType;
use pumpkin_data::entity::EntityType;
use pumpkin_data::sound::Sound;
use pumpkin_data::tracked_data;
use pumpkin_nbt::compound::NbtCompound;
use pumpkin_util::math::vector3::Vector3;
use rand::RngExt;

use crate::entity::Entity;
use crate::entity::EntityBase;
use crate::entity::ai::control::fish_move_control::FishMoveControl;
use crate::entity::ai::goal::avoid_entity::AvoidEntityGoal;
use crate::entity::ai::goal::escape_danger::EscapeDangerGoal;
use crate::entity::ai::goal::wander_around::WanderAroundGoal;
use crate::entity::ai::goal::{Controls, Goal, to_goal_ticks};
use crate::entity::ai::pathfinder::node::PathType;
use crate::entity::ai::pathfinder::{Navigator, NavigatorGoal};
use crate::entity::breath::{MAX_AIR, tick_mob_air_supply};
use crate::entity::mob::spawn::{SchoolSpawnGroupData, SpawnGroupData};
use crate::entity::mob::{Mob, MobEntity};
use crate::world::World;

/// State shared by the bucketable, schooling fish: bucket persistence, land
/// air supply, and the flock leader a schooling fish follows.
pub struct FishData {
    from_bucket: AtomicBool,
    air_supply: AtomicI32,
    leader: Mutex<Option<Arc<dyn EntityBase>>>,
    school_size: AtomicI32,
    max_school_size: i32,
}

impl FishData {
    #[must_use]
    pub fn new(max_school_size: i32) -> Self {
        Self {
            from_bucket: AtomicBool::new(false),
            air_supply: AtomicI32::new(MAX_AIR),
            leader: Mutex::new(None),
            school_size: AtomicI32::new(1),
            max_school_size,
        }
    }

    #[must_use]
    pub fn from_bucket(&self) -> bool {
        self.from_bucket.load(Ordering::Relaxed)
    }

    pub fn set_from_bucket(&self, entity: &Entity, from_bucket: bool) {
        self.from_bucket.store(from_bucket, Ordering::Relaxed);
        entity.set_synced_data(tracked_data::abstract_fish::FROM_BUCKET, from_bucket);
    }

    pub fn tick_air_supply(&self, caller: &dyn EntityBase) {
        tick_mob_air_supply(
            caller,
            &self.air_supply,
            MAX_AIR,
            !caller.get_entity().is_in_water(),
            DamageType::DROWN,
        );
    }

    #[must_use]
    pub fn is_follower(&self) -> bool {
        self.leader
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .as_ref()
            .is_some_and(|leader| leader.get_entity().is_alive())
    }

    #[must_use]
    pub fn leader(&self) -> Option<Arc<dyn EntityBase>> {
        self.leader
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .clone()
    }

    pub fn start_following(&self, leader: &Arc<dyn EntityBase>) {
        {
            let mut slot = self.leader.lock().unwrap_or_else(PoisonError::into_inner);
            if slot.is_some() {
                return;
            }
            *slot = Some(leader.clone());
        };
        if let Some(data) = leader.get_mob().and_then(Mob::get_fish_data) {
            data.add_follower();
        }
    }

    pub fn stop_following(&self) {
        let mut slot = self.leader.lock().unwrap_or_else(PoisonError::into_inner);
        let leader = slot.take();
        drop(slot);
        if let Some(leader) = leader
            && let Some(data) = leader.get_mob().and_then(Mob::get_fish_data)
        {
            data.remove_follower();
        }
    }

    #[must_use]
    pub fn school_size(&self) -> i32 {
        self.school_size.load(Ordering::Relaxed)
    }

    #[must_use]
    pub const fn max_school_size(&self) -> i32 {
        self.max_school_size
    }

    fn add_follower(&self) {
        self.school_size.fetch_add(1, Ordering::Relaxed);
    }

    fn remove_follower(&self) {
        self.school_size.fetch_sub(1, Ordering::Relaxed);
    }

    fn set_school_size(&self, size: i32) {
        self.school_size.store(size, Ordering::Relaxed);
    }

    #[must_use]
    pub fn has_followers(&self) -> bool {
        self.school_size() > 1
    }

    #[must_use]
    pub fn can_be_followed(&self) -> bool {
        self.has_followers() && self.school_size() < self.max_school_size
    }
}

pub fn write_data(data: &FishData, nbt: &mut NbtCompound) {
    nbt.put_bool("FromBucket", data.from_bucket());
}

pub fn read_data(data: &FishData, entity: &Entity, nbt: &NbtCompound) {
    if let Some(from_bucket) = nbt.get_bool("FromBucket") {
        data.set_from_bucket(entity, from_bucket);
    }
}

pub fn init_data(data: &FishData, entity: &Entity) {
    entity.set_synced_data(tracked_data::abstract_fish::FROM_BUCKET, data.from_bucket());
}

/// Gives a new fish vanilla's water navigation and move control, in place of the
/// walking defaults every mob starts with.
pub fn init_movement(mob_entity: &MobEntity) {
    let mut navigator = Navigator::water_bound(false);
    navigator.set_pathfinding_malus(PathType::Water, 0.0);
    *mob_entity
        .navigator
        .lock()
        .unwrap_or_else(PoisonError::into_inner) = navigator;
    *mob_entity
        .move_control
        .lock()
        .unwrap_or_else(PoisonError::into_inner) = Box::new(FishMoveControl);
}

/// Gives a new fish vanilla's swimming AI in place of the walking defaults every
/// mob starts with: water navigation, panic when hurt, keeping clear of players,
/// and swimming to random spots.
pub fn init(mob_entity: &MobEntity) {
    init_movement(mob_entity);

    let mut goals = mob_entity
        .goals_selector
        .lock()
        .unwrap_or_else(PoisonError::into_inner);
    goals.add_goal(0, EscapeDangerGoal::new(1.25));
    goals.add_goal(
        2,
        Box::new(AvoidEntityGoal::new(&EntityType::PLAYER, 8.0, 1.6, 1.4)),
    );
    goals.add_goal(4, Box::new(FishSwimGoal::new(1.0, 40)));
}

/// Adds vanilla's `FollowFlockLeaderGoal` to a schooling fish.
pub fn init_schooling(mob_entity: &MobEntity) {
    mob_entity
        .goals_selector
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .add_goal(5, Box::new(FollowFlockLeaderGoal::default()));
}

/// Vanilla `AbstractSchoolingFish.tick`: a leader whose school has dispersed
/// drops its follower count so new fish can join it again.
pub fn tick_schooling(mob: &dyn Mob) {
    let Some(data) = mob.get_fish_data() else {
        return;
    };
    if !data.has_followers() || rand::random_range(0..200) != 1 {
        return;
    }

    let entity = mob.get_entity();
    let bounding_box = entity.bounding_box.load().expand_all(8.0);
    let neighbours = entity
        .world
        .load()
        .get_entities_at_box(&bounding_box)
        .into_iter()
        .filter(|other| other.get_entity().entity_type == entity.entity_type)
        .count();
    if neighbours <= 1 {
        data.set_school_size(1);
    }
}

struct FishSwimGoal {
    inner: WanderAroundGoal,
}

impl FishSwimGoal {
    const fn new(speed: f64, interval: i32) -> Self {
        Self {
            inner: WanderAroundGoal::swimming(speed, interval),
        }
    }
}

impl Goal for FishSwimGoal {
    fn can_start(&mut self, mob: &dyn Mob) -> bool {
        mob.get_fish_data().is_none_or(|data| !data.is_follower()) && self.inner.can_start(mob)
    }

    fn should_continue(&mut self, mob: &dyn Mob) -> bool {
        self.inner.should_continue(mob)
    }

    fn start(&mut self, mob: &dyn Mob) {
        self.inner.start(mob);
    }

    fn stop(&mut self, mob: &dyn Mob) {
        self.inner.stop(mob);
    }

    fn tick(&mut self, mob: &dyn Mob) {
        self.inner.tick(mob);
    }

    fn controls(&self) -> Controls {
        self.inner.controls()
    }
}

/// Vanilla `FollowFlockLeaderGoal`.
struct FollowFlockLeaderGoal {
    time_to_recalc_path: i32,
    next_start_tick: Option<i32>,
}

impl Default for FollowFlockLeaderGoal {
    fn default() -> Self {
        Self {
            time_to_recalc_path: 0,
            next_start_tick: Some(to_goal_ticks(200 + rand::random_range(0..200) % 20)),
        }
    }
}

impl FollowFlockLeaderGoal {
    fn next_start_tick(mob: &dyn Mob) -> i32 {
        to_goal_ticks(200 + mob.get_random().random_range(0..200) % 20)
    }
}

impl Goal for FollowFlockLeaderGoal {
    fn can_start(&mut self, mob: &dyn Mob) -> bool {
        let Some(data) = mob.get_fish_data() else {
            return false;
        };
        if data.has_followers() {
            return false;
        }
        if data.is_follower() {
            return true;
        }

        if let Some(ticks) = self.next_start_tick
            && ticks > 0
        {
            self.next_start_tick = Some(ticks - 1);
            return false;
        }
        self.next_start_tick = Some(Self::next_start_tick(mob));

        let entity = mob.get_entity();
        let world = entity.world.load();
        let bounding_box = entity.bounding_box.load().expand_all(8.0);
        let candidates: Vec<Arc<dyn EntityBase>> = world
            .get_entities_at_box(&bounding_box)
            .into_iter()
            .filter(|other| {
                other.get_entity().entity_type == entity.entity_type
                    && other.get_mob().and_then(Mob::get_fish_data).is_some()
            })
            .collect();

        let Some(self_entity) = world.get_entity_by_id(entity.entity_id) else {
            return false;
        };
        let leader = candidates
            .iter()
            .find(|other| {
                other
                    .get_mob()
                    .and_then(Mob::get_fish_data)
                    .is_some_and(FishData::can_be_followed)
            })
            .cloned()
            .unwrap_or(self_entity);

        let Some(leader_data) = leader.get_mob().and_then(Mob::get_fish_data) else {
            return false;
        };
        let mut room = leader_data.max_school_size() - leader_data.school_size();
        for fish in &candidates {
            if room <= 0 {
                break;
            }
            if fish.get_entity().entity_uuid == leader.get_entity().entity_uuid {
                continue;
            }
            let Some(fish_data) = fish.get_mob().and_then(Mob::get_fish_data) else {
                continue;
            };
            if fish_data.is_follower() {
                continue;
            }
            fish_data.start_following(&leader);
            room -= 1;
        }

        data.is_follower()
    }

    fn should_continue(&mut self, mob: &dyn Mob) -> bool {
        let Some(data) = mob.get_fish_data() else {
            return false;
        };
        let Some(leader) = data.leader() else {
            return false;
        };
        let mob_pos = mob.get_entity().pos.load();
        let leader_pos = leader.get_entity().pos.load();
        data.is_follower() && mob_pos.squared_distance_to_vec(&leader_pos) <= 121.0
    }

    fn start(&mut self, _mob: &dyn Mob) {
        self.time_to_recalc_path = 0;
    }

    fn stop(&mut self, mob: &dyn Mob) {
        if let Some(data) = mob.get_fish_data() {
            data.stop_following();
        }
    }

    fn tick(&mut self, mob: &dyn Mob) {
        self.time_to_recalc_path -= 1;
        if self.time_to_recalc_path > 0 {
            return;
        }
        self.time_to_recalc_path = self.get_tick_count(10);

        let Some(data) = mob.get_fish_data() else {
            return;
        };
        let Some(leader) = data.leader() else {
            return;
        };
        let pos = mob.get_entity().pos.load();
        let target = leader.get_entity().pos.load();
        mob.get_mob_entity()
            .navigator
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .set_progress(NavigatorGoal::new(pos, target, 1.0));
    }
}

/// Vanilla's `AbstractFish.travel`: in water a fish swims at a fraction of the
/// usual pace with no gravity, sinking slowly while it has nothing to chase. Out
/// of water it falls like anything else.
pub fn travel(mob: &dyn Mob, caller: &dyn EntityBase) -> bool {
    let mob_entity = mob.get_mob_entity();
    let living = &mob_entity.living_entity;
    let entity = &living.entity;
    if !entity.touching_water.load(Ordering::Relaxed) {
        return false;
    }

    entity.update_velocity_from_input(living.movement_input.load(), 0.01);
    living.make_move(caller);

    let mut velocity = entity.velocity.load() * 0.9;
    let chasing = mob_entity
        .target
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .is_some();
    if !chasing {
        velocity.y -= 0.005;
    }
    entity.velocity.store(velocity);
    true
}

/// Vanilla's `AbstractFish.aiStep` on dry land, where a stranded fish hops about.
pub fn flop(mob: &dyn Mob, sound: Sound) {
    let entity = mob.get_entity();
    if entity.touching_water.load(Ordering::Relaxed) || !entity.on_ground.load(Ordering::Relaxed) {
        return;
    }
    let mut rng = mob.get_random();
    let velocity = entity.velocity.load();
    // Sent at once, as vanilla flags the hop with needsSync.
    entity.set_velocity(Vector3::new(
        velocity.x + f64::from(rng.random::<f32>() * 2.0 - 1.0) * 0.05,
        velocity.y + 0.4,
        velocity.z + f64::from(rng.random::<f32>() * 2.0 - 1.0) * 0.05,
    ));
    entity.on_ground.store(false, Ordering::Relaxed);
    entity.play_sound(sound);
}

/// Vanilla `WaterAnimal.checkSpawnObstruction`: water animals only care about
/// being unobstructed, not about standing in a fluid.
#[must_use]
pub fn check_spawn_obstruction(mob: &dyn Mob, world: &World) -> bool {
    world
        .get_entities_at_box(&mob.get_entity().bounding_box.load())
        .is_empty()
}

/// Vanilla `AbstractSchoolingFish.finalizeSpawn`: every later fish of a spawn group follows
/// the first one.
pub fn finalize_school_spawn(
    mob_entity: &MobEntity,
    fish_data: &FishData,
    group_data: Option<SpawnGroupData>,
) -> Option<SpawnGroupData> {
    mob_entity.finalize_spawn_base();
    let Some(group_data) = group_data else {
        return Some(SpawnGroupData::School(SchoolSpawnGroupData::default()));
    };
    if let Some(leader) = group_data
        .school()
        .and_then(|school| school.leader.as_ref())
    {
        fish_data.start_following(leader);
    }
    Some(group_data)
}
