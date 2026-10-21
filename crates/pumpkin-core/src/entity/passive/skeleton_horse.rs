use std::sync::{
    Arc, Weak,
    atomic::{AtomicBool, AtomicI32, AtomicU8, Ordering},
};

use crossbeam::atomic::AtomicCell;
use pumpkin_data::data_component_impl::EquipmentSlot;
use pumpkin_data::entity::EntityType;
use pumpkin_data::item::Item;
use pumpkin_data::item_stack::ItemStack;
use pumpkin_data::sound::{Sound, SoundCategory};
use pumpkin_nbt::compound::NbtCompound;
use pumpkin_util::math::vector3::Vector3;
use rand::RngExt;
use uuid::Uuid;

use crate::entity::{
    Entity, EntityBase, LightningBoltEntity,
    ageable::{AgeableData, AgeableMob},
    ai::goal::{
        ambient_stand::AmbientStandGoal, follow_parent::FollowParentGoal,
        look_around::RandomLookAroundGoal, look_at_entity::LookAtEntityGoal,
        run_around_like_crazy::RunAroundLikeCrazyGoal, wander_around::WanderAroundGoal,
    },
    mob::{Mob, MobEntity, spawn::finalize_spawn},
    passive::animal::Animal,
    player::Player,
    r#type::from_type,
};

use super::horse::{
    AbstractHorse, FLAG_SADDLE, FLAG_TAME, empty_mount_inventory, open_mount_screen,
};

const TRAP_MAX_LIFE: i32 = 18000;

pub struct SkeletonHorseEntity {
    pub mob_entity: MobEntity,
    pub ageable_data: AgeableData,
    pub flags: AtomicU8,
    pub temper: AtomicI32,
    pub stand_ticks: AtomicI32,
    pub is_trap: AtomicBool,
    pub trap_time: AtomicI32,
    pub owner: AtomicCell<Option<Uuid>>,
}

impl SkeletonHorseEntity {
    pub fn new(entity: Entity) -> Arc<Self> {
        let mob_entity = MobEntity::new(entity);
        let horse = Self {
            mob_entity,
            ageable_data: AgeableData::default(),
            flags: AtomicU8::new(0),
            temper: AtomicI32::new(0),
            stand_ticks: AtomicI32::new(0),
            is_trap: AtomicBool::new(false),
            trap_time: AtomicI32::new(0),
            owner: AtomicCell::new(None),
        };
        let mob_arc = Arc::new(horse);
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

            goal_selector.add_goal(1, Box::new(RunAroundLikeCrazyGoal::new(1.2)));
            goal_selector.add_goal(4, Box::new(FollowParentGoal::new(1.0)));
            goal_selector.add_goal(6, Box::new(WanderAroundGoal::new(0.7)));
            goal_selector.add_goal(
                7,
                LookAtEntityGoal::with_default(mob_weak, &EntityType::PLAYER, 6.0),
            );
            goal_selector.add_goal(8, Box::new(RandomLookAroundGoal::default()));
            goal_selector.add_goal(9, Box::new(AmbientStandGoal::default()));
        };

        mob_arc
    }

    #[must_use]
    pub fn has_flag(&self, flag: u8) -> bool {
        (self.flags.load(Ordering::Relaxed) & flag) != 0
    }

    pub fn set_flag(&self, flag: u8, val: bool) {
        let current = self.flags.load(Ordering::Relaxed);
        let new_flags = if val { current | flag } else { current & !flag };
        self.flags.store(new_flags, Ordering::Relaxed);
        let entity = self.get_entity();
        entity.set_synced_data(
            pumpkin_data::tracked_data::skeleton_horse::DATA_ID_FLAGS,
            new_flags as i8,
        );
    }

    #[must_use]
    pub fn is_tame(&self) -> bool {
        self.has_flag(FLAG_TAME)
    }

    pub fn set_tame(&self, val: bool) {
        self.set_flag(FLAG_TAME, val);
    }

    #[must_use]
    pub fn is_saddled(&self) -> bool {
        self.has_flag(FLAG_SADDLE)
    }

    pub fn set_saddled(&self, val: bool) {
        self.set_flag(FLAG_SADDLE, val);
    }

    #[must_use]
    pub fn is_trap(&self) -> bool {
        self.is_trap.load(Ordering::Relaxed)
    }

    pub fn set_trap(&self, trap: bool) {
        self.is_trap.store(trap, Ordering::Relaxed);
    }

    /// Vanilla `SkeletonHorse.aiStep`: trap lifetime expiry.
    fn tick_trap(&self) {
        if !self.is_trap() {
            return;
        }

        let entity = self.get_entity();
        let world = entity.world.load();
        let nearby_player = world
            .get_nearby_players(entity.pos.load(), 10.0)
            .iter()
            .any(|player| player.living_entity.is_alive() && !player.is_spectator());

        if nearby_player {
            self.trigger_trap();
            return;
        }

        let trap_time = self.trap_time.fetch_add(1, Ordering::Relaxed);
        if !self.mob_entity.persistence_required.load(Ordering::Relaxed)
            && trap_time >= TRAP_MAX_LIFE
        {
            entity.remove();
        }
    }

    /// Vanilla `SkeletonTrapGoal.tick`: lightning, then a skeleton rider and three
    /// skeleton horses around the trap horse.
    fn trigger_trap(&self) {
        self.set_trap(false);
        self.set_tame(true);
        self.set_age(0);

        let entity = self.get_entity();
        let world = entity.world.load_full();
        let pos = entity.pos.load();

        let bolt_entity = Entity::new(world.clone(), pos, &EntityType::LIGHTNING_BOLT);
        let bolt = LightningBoltEntity::new(bolt_entity);
        bolt.set_visual_only(true);
        world.spawn_entity(Arc::new(bolt) as Arc<dyn EntityBase>);

        let Some(horse) = world.get_entity_by_id(entity.entity_id) else {
            return;
        };
        let skeleton = Self::create_skeleton(&world, pos);
        world.spawn_entity(skeleton.clone());
        horse.get_entity().add_passenger(horse.clone(), skeleton);

        let mut rng = rand::rng();
        for _ in 0..3 {
            let other = from_type(&EntityType::SKELETON_HORSE, pos, &world, Uuid::new_v4());
            if let Some(horse) = other.cast_any().downcast_ref::<SkeletonHorseEntity>() {
                horse.set_tame(true);
                horse.set_age(0);
                horse
                    .mob_entity
                    .persistence_required
                    .store(true, Ordering::Relaxed);
                horse.set_trap(false);
            }
            let _ = finalize_spawn(&other, &world, None);
            other.get_entity().set_pos(pos);
            let velocity = other.get_entity().velocity.load();
            other.get_entity().velocity.store(
                velocity
                    + Vector3::new(
                        (rng.random_range(0.0..1.0) - rng.random_range(0.0..1.0)) * 1.1485,
                        0.0,
                        (rng.random_range(0.0..1.0) - rng.random_range(0.0..1.0)) * 1.1485,
                    ),
            );
            other
                .get_entity()
                .velocity_dirty
                .store(true, Ordering::Relaxed);
            world.spawn_entity(other.clone());

            let other_skeleton = Self::create_skeleton(&world, pos);
            world.spawn_entity(other_skeleton.clone());
            other
                .get_entity()
                .add_passenger(other.clone(), other_skeleton);
        }
    }

    fn create_skeleton(world: &Arc<crate::world::World>, pos: Vector3<f64>) -> Arc<dyn EntityBase> {
        let skeleton = from_type(&EntityType::SKELETON, pos, world, Uuid::new_v4());
        let _ = finalize_spawn(&skeleton, world, None);
        skeleton.get_entity().set_pos(pos);
        if let Some(mob) = skeleton.get_mob() {
            mob.get_mob_entity()
                .persistence_required
                .store(true, Ordering::Relaxed);
            let has_helmet = mob
                .get_mob_entity()
                .living_entity
                .entity_equipment
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .equipment
                .get(&EquipmentSlot::HEAD)
                .is_some_and(|stack| !stack.is_empty());
            if !has_helmet {
                mob.get_mob_entity()
                    .set_item_slot(&EquipmentSlot::HEAD, ItemStack::new(1, &Item::IRON_HELMET));
            }
        }
        skeleton
    }
}

impl AbstractHorse for SkeletonHorseEntity {
    fn has_horse_flag(&self, flag: u8) -> bool {
        self.has_flag(flag)
    }

    fn set_horse_flag(&self, flag: u8, val: bool) {
        self.set_flag(flag, val);
    }

    fn get_stand_ticks(&self) -> &AtomicI32 {
        &self.stand_ticks
    }

    fn set_horse_owner(&self, owner: Option<Uuid>) {
        self.owner.store(owner);
    }

    fn get_temper(&self) -> i32 {
        self.temper.load(Ordering::Relaxed)
    }

    fn set_temper(&self, temper: i32) {
        self.temper.store(temper, Ordering::Relaxed);
    }
}

impl AgeableMob for SkeletonHorseEntity {
    fn get_ageable_data(&self) -> &AgeableData {
        &self.ageable_data
    }

    fn can_age_up(&self) -> bool {
        false
    }
}

impl Animal for SkeletonHorseEntity {
    fn is_food(&self, _item_stack: &ItemStack) -> bool {
        false
    }
}

impl Mob for SkeletonHorseEntity {
    fn as_ageable(&self) -> Option<&dyn AgeableMob> {
        Some(self)
    }

    fn as_animal(&self) -> Option<&dyn Animal> {
        Some(self)
    }

    fn as_abstract_horse(&self) -> Option<&dyn AbstractHorse> {
        Some(self)
    }

    fn is_saddled(&self) -> bool {
        SkeletonHorseEntity::is_saddled(self)
    }

    fn set_saddled(&self, saddled: bool) {
        SkeletonHorseEntity::set_saddled(self, saddled);
    }

    fn get_ambient_sound(&self) -> Option<Sound> {
        Some(Sound::EntitySkeletonHorseAmbient)
    }

    fn get_ambient_sound_interval(&self) -> i32 {
        400
    }

    fn mob_write_nbt(&self, nbt: &mut NbtCompound) {
        self.write_ageable_nbt(nbt);
        nbt.put_bool("Tame", self.is_tame());
        nbt.put_int("Temper", self.temper.load(Ordering::Relaxed));
        nbt.put_bool("SkeletonTrap", self.is_trap());
        nbt.put_int("SkeletonTrapTime", self.trap_time.load(Ordering::Relaxed));
        if let Some(owner) = self.owner.load() {
            nbt.put_uuid("Owner", owner);
        }
    }

    fn mob_read_nbt(&self, nbt: &NbtCompound) {
        self.read_ageable_nbt(nbt);
        if let Some(tame) = nbt.get_bool("Tame") {
            self.set_tame(tame);
        }
        if let Some(temper) = nbt.get_int("Temper") {
            self.temper.store(temper, Ordering::Relaxed);
        }
        self.set_trap(nbt.get_bool("SkeletonTrap").unwrap_or(false));
        self.trap_time.store(
            nbt.get_int("SkeletonTrapTime").unwrap_or(0),
            Ordering::Relaxed,
        );
        if let Some(owner) = nbt.get_uuid("Owner") {
            self.owner.store(Some(owner));
        }
    }

    fn get_mob_entity(&self) -> &MobEntity {
        &self.mob_entity
    }

    fn mob_tick(&self, _caller: &dyn EntityBase) {
        self.ageable_ai_step();
        self.tick_trap();
        self.horse_tick();
    }

    fn mob_init_data_tracker(&self) {
        let entity = self.get_entity();
        let is_baby = entity.age.load(Ordering::Relaxed) < 0;
        if is_baby {
            entity.set_synced_data(
                pumpkin_data::tracked_data::skeleton_horse::DATA_BABY_ID,
                true,
            );
        }
        entity.set_synced_data(
            pumpkin_data::tracked_data::skeleton_horse::DATA_ID_FLAGS,
            self.flags.load(Ordering::Relaxed) as i8,
        );
    }

    fn mob_interact(&self, player: &Arc<Player>, item_stack: &mut ItemStack) -> bool {
        // Vanilla `SkeletonHorse.mobInteract` refuses everything until tamed.
        if !self.is_tame() {
            return false;
        }

        let item = item_stack.get_item();
        if player.get_entity().is_sneaking() {
            open_mount_screen(self.get_entity(), player, empty_mount_inventory(), 0);
            return true;
        }

        if item == &Item::SADDLE && !self.is_saddled() && !self.is_baby() {
            self.set_saddled(true);
            item_stack.decrement_unless_creative(player.gamemode.load(), 1);
            let entity = self.get_entity();
            entity.world.load().play_sound(
                Sound::EntityHorseSaddle,
                SoundCategory::Neutral,
                &entity.pos.load(),
            );
            return true;
        }

        if !self.is_baby() && !self.get_entity().has_passengers() {
            self.do_player_ride(player);
            return true;
        }

        false
    }
}
