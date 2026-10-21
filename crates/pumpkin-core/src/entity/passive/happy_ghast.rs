use std::sync::{
    Arc, Weak,
    atomic::{AtomicBool, AtomicI32, AtomicI64, Ordering},
};

use pumpkin_data::data_component_impl::{EquipmentSlot, EquippableImpl, IDSet};
use pumpkin_data::entity::EntityType;
use pumpkin_data::item::Item;
use pumpkin_data::item_stack::ItemStack;
use pumpkin_data::sound::{Sound, SoundCategory};
use pumpkin_data::tag::{self, Taggable};
use pumpkin_nbt::compound::NbtCompound;

use crate::entity::{
    Entity, EntityBase,
    ageable::{AgeableData, AgeableMob},
    ai::goal::{
        look_around::RandomLookAroundGoal, look_at_entity::LookAtEntityGoal, swim::SwimGoal,
        tempt::TemptGoal, wander_around::WanderAroundGoal,
    },
    mob::{Mob, MobEntity},
    passive::animal::Animal,
    player::Player,
};

pub const HAPPY_GHAST_FOOD: &[&Item] = &[&Item::SNOWBALL];
/// Vanilla `HappyGhast` maximum passenger count.
pub const MAX_PASSENGERS: usize = 4;
const STILL_TIMEOUT_ON_LOAD_GRACE_PERIOD: i32 = 60;
const MAX_STILL_TIMEOUT: i32 = 10;
const FAST_HEALING_TICKS: i32 = 20;
const SLOW_HEALING_TICKS: i32 = 600;

/// Represents a Happy Ghast, a passive flying mob.
///
/// Wiki: <https://minecraft.wiki/w/Happy_Ghast>
pub struct HappyGhastEntity {
    pub mob_entity: MobEntity,
    pub ageable_data: AgeableData,
    pub server_still_timeout: AtomicI32,
    pub leash_holder_time: AtomicI32,
    pub is_leash_holder: AtomicBool,
    pub stays_still: AtomicBool,
    last_passenger_count: AtomicI32,
    spawned_at: AtomicI64,
}

impl HappyGhastEntity {
    pub fn new(entity: Entity) -> Arc<Self> {
        let mob_entity = MobEntity::new(entity);
        let spawned_at = mob_entity.living_entity.entity.world.load().get_world_age();
        let happy_ghast = Self {
            mob_entity,
            ageable_data: AgeableData::default(),
            server_still_timeout: AtomicI32::new(0),
            leash_holder_time: AtomicI32::new(0),
            is_leash_holder: AtomicBool::new(false),
            stays_still: AtomicBool::new(false),
            last_passenger_count: AtomicI32::new(0),
            spawned_at: AtomicI64::new(spawned_at),
        };
        let mob_arc = Arc::new(happy_ghast);
        let mob_weak: Weak<dyn Mob> = {
            let mob_arc: Arc<dyn Mob> = mob_arc.clone();
            Arc::downgrade(&mob_arc)
        };

        {
            let mut goal_selector = mob_arc
                .mob_entity
                .goals_selector
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);

            goal_selector.add_goal(0, Box::new(SwimGoal::default()));
            goal_selector.add_goal(
                1,
                Box::new(TemptGoal::with_stop_distance(
                    1.0,
                    HAPPY_GHAST_FOOD,
                    false,
                    7.0,
                )),
            );
            goal_selector.add_goal(2, Box::new(WanderAroundGoal::new(1.0)));
            goal_selector.add_goal(
                3,
                LookAtEntityGoal::with_default(mob_weak, &EntityType::PLAYER, 6.0),
            );
            goal_selector.add_goal(4, Box::new(RandomLookAroundGoal::default()));
        };

        mob_arc
    }

    pub fn set_server_still_timeout(&self, timeout: i32) {
        self.server_still_timeout.store(timeout, Ordering::Relaxed);
        self.sync_stay_still_flag();
    }

    #[must_use]
    pub fn is_on_still_timeout(&self) -> bool {
        self.stays_still.load(Ordering::Relaxed)
            || self.server_still_timeout.load(Ordering::Relaxed) > 0
    }

    fn set_leash_holder(&self, is_leash_holder: bool) {
        self.is_leash_holder
            .store(is_leash_holder, Ordering::Relaxed);
        let entity = self.get_entity();
        entity.set_synced_data(
            pumpkin_data::tracked_data::happy_ghast::IS_LEASH_HOLDER,
            is_leash_holder,
        );
    }

    fn sync_stay_still_flag(&self) {
        let stays_still = self.server_still_timeout.load(Ordering::Relaxed) > 0;
        self.stays_still.store(stays_still, Ordering::Relaxed);
        let entity = self.get_entity();
        entity.set_synced_data(
            pumpkin_data::tracked_data::happy_ghast::STAYS_STILL,
            stays_still,
        );
    }

    #[must_use]
    pub fn is_wearing_harness(&self) -> bool {
        !self
            .mob_entity
            .living_entity
            .entity_equipment
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .get(&EquipmentSlot::BODY)
            .is_empty()
    }

    fn clear_home(&self) {
        self.mob_entity
            .position_target_range
            .store(-1, Ordering::Relaxed);
    }

    fn restriction_radius(&self) -> i32 {
        if !self.is_baby() && !self.is_wearing_harness() {
            64
        } else {
            32
        }
    }

    /// Vanilla `HappyGhast.checkRestriction`.
    fn check_restriction(&self) {
        let entity = self.get_entity();
        if entity.is_leashed() || entity.has_passengers() {
            return;
        }
        let radius = self.restriction_radius();
        if self
            .mob_entity
            .position_target_range
            .load(Ordering::Relaxed)
            != radius
            || !self
                .mob_entity
                .is_in_position_target_range_pos(&entity.block_pos.load())
        {
            self.mob_entity
                .position_target
                .store(entity.block_pos.load());
            self.mob_entity
                .position_target_range
                .store(radius, Ordering::Relaxed);
        }
    }

    /// Vanilla `HappyGhast.scanPlayerAboveGhast`.
    fn scan_player_above_ghast(&self) -> bool {
        let entity = self.get_entity();
        let bb = entity.bounding_box.load();
        let detection_min = pumpkin_util::math::vector3::Vector3::new(
            bb.min.x - 1.0,
            bb.max.y - 1.0e-5,
            bb.min.z - 1.0,
        );
        let detection_max = pumpkin_util::math::vector3::Vector3::new(
            bb.max.x + 1.0,
            bb.max.y + f64::from(entity.height()) / 2.0,
            bb.max.z + 1.0,
        );

        let world = entity.world.load();
        for player in world.players.load().iter() {
            if player.is_spectator() {
                continue;
            }
            let mut root: Arc<dyn EntityBase> = Arc::clone(player) as Arc<dyn EntityBase>;
            while let Some(vehicle) = root.get_entity().get_vehicle() {
                root = vehicle;
            }
            if root.cast_any().is::<HappyGhastEntity>() {
                continue;
            }
            let pos = root.get_entity().pos.load();
            if pos.x >= detection_min.x
                && pos.x <= detection_max.x
                && pos.y >= detection_min.y
                && pos.y <= detection_max.y
                && pos.z >= detection_min.z
                && pos.z <= detection_max.z
            {
                return true;
            }
        }
        false
    }

    /// Vanilla `HappyGhast.doPlayerRide`.
    fn do_player_ride(&self, player: &Arc<Player>) {
        if player.get_entity().has_vehicle() {
            return;
        }
        let entity = self.get_entity();
        let world = entity.world.load();
        let Some(vehicle) = world.get_entity_by_id(entity.entity_id) else {
            return;
        };
        let Some(passenger) = world.get_player_by_id(player.entity_id()) else {
            return;
        };
        entity.add_passenger(vehicle, passenger as Arc<dyn EntityBase>);
    }

    /// Vanilla `HappyGhast.addPassenger`/`removePassenger`: the harness sounds and the
    /// server-side still timeout that lets a rider take control.
    fn tick_passengers(&self) {
        let entity = self.get_entity();
        let count = entity
            .passengers
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .len() as i32;
        let previous = self.last_passenger_count.swap(count, Ordering::Relaxed);

        if count > previous && previous == 0 && !entity.is_silent() {
            entity.world.load().play_sound(
                Sound::EntityHappyGhastHarnessGogglesDown,
                SoundCategory::Neutral,
                &entity.pos.load(),
            );
        }
        if count < previous {
            self.set_server_still_timeout(MAX_STILL_TIMEOUT);
            if count == 0 {
                self.clear_home();
                if !entity.is_silent() {
                    entity.world.load().play_sound(
                        Sound::EntityHappyGhastHarnessGogglesUp,
                        SoundCategory::Neutral,
                        &entity.pos.load(),
                    );
                }
            }
        }

        if count > 0 && self.scan_player_above_ghast() {
            self.set_server_still_timeout(MAX_STILL_TIMEOUT);
        } else if count > previous && previous == 0 {
            self.set_server_still_timeout(0);
        }
    }

    /// Vanilla `HappyGhast.continuousHeal`.
    fn continuous_heal(&self) {
        let entity = self.get_entity();
        let living = &self.mob_entity.living_entity;
        if !entity.is_alive() || living.health.load() >= living.get_max_health() {
            return;
        }
        let world = entity.world.load();
        let pos = entity.block_pos.load();
        let fast_healing = world.is_raining_at(&pos);
        let interval = if fast_healing {
            FAST_HEALING_TICKS
        } else {
            SLOW_HEALING_TICKS
        };
        if world.get_world_age() % i64::from(interval) == 0 {
            living.set_health(living.health.load() + 1.0);
        }
    }

    /// Vanilla `HappyGhast.mobInteract` harness branch.
    fn try_equip_harness(&self, player: &Arc<Player>, item_stack: &mut ItemStack) -> bool {
        let Some(equippable) = item_stack.get_data_component::<EquippableImpl>() else {
            return false;
        };
        if *equippable.slot != EquipmentSlot::BODY {
            return false;
        }
        if let Some(allowed) = &equippable.allowed_entities {
            let entity = self.get_entity();
            let matches = match allowed {
                IDSet::Tag(tag) => entity.entity_type.is_tagged_with(tag).unwrap_or(false),
                IDSet::IDs(ids) => ids.iter().any(|id| id.id == entity.entity_type.id),
            };
            if !matches {
                return false;
            }
        }

        let previous = self.mob_entity.set_item_slot(
            &EquipmentSlot::BODY,
            ItemStack::new(1, item_stack.get_item()),
        );
        item_stack.decrement_unless_creative(player.gamemode.load(), 1);
        if !previous.is_empty() {
            let mut previous = previous;
            if !player.inventory().insert_stack_anywhere(&mut previous) {
                self.get_entity().spawn_at_location(previous);
            }
        }
        true
    }
}

impl AgeableMob for HappyGhastEntity {
    fn get_ageable_data(&self) -> &AgeableData {
        &self.ageable_data
    }
}

impl Animal for HappyGhastEntity {
    fn is_food(&self, item_stack: &ItemStack) -> bool {
        item_stack
            .item
            .has_tag(&tag::Item::MINECRAFT_HAPPY_GHAST_FOOD)
            || HAPPY_GHAST_FOOD.iter().any(|i| i.id == item_stack.item.id)
    }
}

impl crate::entity::custom_sound::CustomSound for HappyGhastEntity {
    fn death_sound(&self) -> Option<Sound> {
        Some(if self.is_baby() {
            Sound::EntityGhastlingDeath
        } else {
            Sound::EntityHappyGhastDeath
        })
    }

    fn hurt_sound(&self) -> Option<Sound> {
        Some(if self.is_baby() {
            Sound::EntityGhastlingHurt
        } else {
            Sound::EntityHappyGhastHurt
        })
    }
}

impl Mob for HappyGhastEntity {
    fn as_ageable(&self) -> Option<&dyn AgeableMob> {
        Some(self)
    }

    fn as_animal(&self) -> Option<&dyn Animal> {
        Some(self)
    }

    fn as_custom_sound(&self) -> Option<&dyn crate::entity::custom_sound::CustomSound> {
        Some(self)
    }

    fn get_ambient_sound(&self) -> Option<Sound> {
        Some(if self.is_baby() {
            Sound::EntityGhastlingAmbient
        } else {
            Sound::EntityHappyGhastAmbient
        })
    }

    fn get_ambient_sound_interval(&self) -> i32 {
        let interval = 80;
        if self.get_entity().has_passengers() {
            interval * 6
        } else {
            interval
        }
    }

    fn mob_write_nbt(&self, nbt: &mut NbtCompound) {
        nbt.put_int(
            "still_timeout",
            self.server_still_timeout.load(Ordering::Relaxed),
        );
    }

    fn mob_read_nbt(&self, nbt: &NbtCompound) {
        if let Some(timeout) = nbt.get_int("still_timeout") {
            self.set_server_still_timeout(timeout);
        }
    }

    fn get_mob_entity(&self) -> &MobEntity {
        &self.mob_entity
    }

    fn mob_tick(&self, _caller: &dyn EntityBase) {
        self.ageable_ai_step();

        let leash_time = self.leash_holder_time.load(Ordering::Relaxed);
        if leash_time > 0 {
            self.leash_holder_time.fetch_sub(1, Ordering::Relaxed);
        }
        self.set_leash_holder(leash_time > 0);

        let still_timeout = self.server_still_timeout.load(Ordering::Relaxed);
        if still_timeout > 0 {
            let entity = self.get_entity();
            let world_age = entity.world.load().get_world_age();
            if world_age - self.spawned_at.load(Ordering::Relaxed)
                > i64::from(STILL_TIMEOUT_ON_LOAD_GRACE_PERIOD)
            {
                self.server_still_timeout.fetch_sub(1, Ordering::Relaxed);
            }
            self.sync_stay_still_flag();
        }

        self.tick_passengers();
        self.continuous_heal();
    }

    fn get_mob_gravity(&self) -> f64 {
        0.0
    }

    fn get_mob_y_velocity_drag(&self) -> Option<f64> {
        Some(0.95)
    }

    fn can_add_passenger(&self, _passenger: &dyn EntityBase) -> bool {
        self.get_entity()
            .passengers
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .len()
            < MAX_PASSENGERS
    }

    fn get_controlling_passenger(&self) -> Option<Arc<dyn EntityBase>> {
        let first = self
            .get_entity()
            .passengers
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .first()
            .cloned()?;
        if self.is_wearing_harness() && self.is_on_still_timeout() && first.get_player().is_some() {
            return None;
        }
        Some(first)
    }

    fn mob_init_data_tracker(&self) {
        let entity = self.get_entity();
        let is_baby = entity.age.load(Ordering::Relaxed) < 0;
        if is_baby {
            entity.set_synced_data(pumpkin_data::tracked_data::happy_ghast::BABY_ID, true);
        }
        entity.set_synced_data(
            pumpkin_data::tracked_data::happy_ghast::IS_LEASH_HOLDER,
            self.is_leash_holder.load(Ordering::Relaxed),
        );
        entity.set_synced_data(
            pumpkin_data::tracked_data::happy_ghast::STAYS_STILL,
            self.stays_still.load(Ordering::Relaxed),
        );
    }

    fn custom_server_ai_step(&self, _caller: &dyn EntityBase) {
        self.check_restriction();
    }

    fn mob_interact(&self, player: &Arc<Player>, item_stack: &mut ItemStack) -> bool {
        if self.is_baby() {
            return self.animal_interact(player, item_stack, Sound::EntityGhastlingAmbient);
        }

        if !item_stack.is_empty() && self.try_equip_harness(player, item_stack) {
            return true;
        }

        if self.is_wearing_harness() && !player.get_entity().is_sneaking() {
            self.do_player_ride(player);
            return true;
        }

        self.mob_entity.mob_interact(player, item_stack)
    }
}
