use std::sync::{
    Arc, Weak,
    atomic::{AtomicI32, AtomicU8, Ordering},
};

use crossbeam::atomic::AtomicCell;
use pumpkin_data::entity::EntityType;
use pumpkin_data::item::Item;
use pumpkin_data::item_stack::ItemStack;
use pumpkin_data::sound::{Sound, SoundCategory};
use pumpkin_data::tag::{self, Taggable};
use pumpkin_nbt::compound::NbtCompound;
use uuid::Uuid;

use crate::entity::{
    Entity, EntityBase,
    ageable::{AgeableData, AgeableMob},
    ai::goal::{
        ambient_stand::AmbientStandGoal, escape_danger::EscapeDangerGoal,
        follow_parent::FollowParentGoal, look_around::RandomLookAroundGoal,
        look_at_entity::LookAtEntityGoal, run_around_like_crazy::RunAroundLikeCrazyGoal,
        swim::SwimGoal, tempt::TemptGoal, wander_around::WanderAroundGoal,
    },
    mob::{Mob, MobEntity},
    passive::animal::Animal,
    player::Player,
};

use super::horse::{AbstractHorse, FLAG_SADDLE, FLAG_TAME};

const TEMPT_ITEMS: &[&Item] = &[&Item::RED_MUSHROOM];

pub struct ZombieHorseEntity {
    pub mob_entity: MobEntity,
    pub ageable_data: AgeableData,
    pub flags: AtomicU8,
    pub temper: AtomicI32,
    pub stand_ticks: AtomicI32,
    pub owner: AtomicCell<Option<Uuid>>,
}

impl ZombieHorseEntity {
    pub fn new(entity: Entity) -> Arc<Self> {
        let mob_entity = MobEntity::new(entity);
        let horse = Self {
            mob_entity,
            ageable_data: AgeableData::default(),
            flags: AtomicU8::new(0),
            temper: AtomicI32::new(0),
            stand_ticks: AtomicI32::new(0),
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

            goal_selector.add_goal(0, Box::new(SwimGoal::default()));
            goal_selector.add_goal(1, Box::new(RunAroundLikeCrazyGoal::new(1.2)));
            goal_selector.add_goal(1, Box::new(EscapeDangerGoal::new(1.2)));
            goal_selector.add_goal(3, Box::new(TemptGoal::new(1.25, TEMPT_ITEMS, false)));
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
            pumpkin_data::tracked_data::zombie_horse::DATA_ID_FLAGS,
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
}

impl AbstractHorse for ZombieHorseEntity {
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

    fn get_horse_eating_sound(&self) -> Option<Sound> {
        Some(Sound::EntityZombieHorseEat)
    }

    fn play_horse_angry_sound(&self) {
        let entity = self.get_entity();
        entity.world.load().play_sound(
            Sound::EntityZombieHorseAngry,
            SoundCategory::Neutral,
            &entity.pos.load(),
        );
    }
}

impl AgeableMob for ZombieHorseEntity {
    fn get_ageable_data(&self) -> &AgeableData {
        &self.ageable_data
    }

    fn can_age_up(&self) -> bool {
        false
    }
}

impl Animal for ZombieHorseEntity {
    fn is_food(&self, item_stack: &ItemStack) -> bool {
        item_stack
            .item
            .has_tag(&tag::Item::MINECRAFT_ZOMBIE_HORSE_FOOD)
            || item_stack.item == &Item::RED_MUSHROOM
    }
}

impl Mob for ZombieHorseEntity {
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
        ZombieHorseEntity::is_saddled(self)
    }

    fn set_saddled(&self, saddled: bool) {
        ZombieHorseEntity::set_saddled(self, saddled);
    }

    fn get_ambient_sound(&self) -> Option<Sound> {
        Some(Sound::EntityZombieHorseAmbient)
    }

    fn get_ambient_sound_interval(&self) -> i32 {
        400
    }

    fn is_mob_controlled(&self) -> bool {
        self.get_entity()
            .passengers
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .first()
            .is_some_and(|passenger| passenger.get_mob().is_some())
    }

    fn remove_when_far_away(&self, _distance_sq: f64) -> bool {
        true
    }

    fn sun_protection_slot(&self) -> pumpkin_data::data_component_impl::EquipmentSlot {
        pumpkin_data::data_component_impl::EquipmentSlot::BODY
    }

    fn mob_write_nbt(&self, nbt: &mut NbtCompound) {
        self.write_ageable_nbt(nbt);
        nbt.put_bool("Tame", self.is_tame());
        nbt.put_int("Temper", self.temper.load(Ordering::Relaxed));
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
        if let Some(owner) = nbt.get_uuid("Owner") {
            self.owner.store(Some(owner));
        }
    }

    fn get_mob_entity(&self) -> &MobEntity {
        &self.mob_entity
    }

    fn mob_tick(&self, _caller: &dyn EntityBase) {
        self.ageable_ai_step();
        self.horse_tick();
    }

    fn mob_init_data_tracker(&self) {
        let entity = self.get_entity();
        let is_baby = entity.age.load(Ordering::Relaxed) < 0;
        if is_baby {
            entity.set_synced_data(pumpkin_data::tracked_data::zombie_horse::DATA_BABY_ID, true);
        }
        entity.set_synced_data(
            pumpkin_data::tracked_data::zombie_horse::DATA_ID_FLAGS,
            self.flags.load(Ordering::Relaxed) as i8,
        );
    }

    fn mob_interact(&self, player: &Arc<Player>, item_stack: &mut ItemStack) -> bool {
        self.mob_entity
            .persistence_required
            .store(true, Ordering::Relaxed);
        let item = item_stack.get_item();
        let should_open_inventory =
            !self.is_baby() && self.is_tame() && player.get_entity().is_sneaking();

        if !self.get_entity().has_passengers() && !should_open_inventory {
            if !item_stack.is_empty() {
                if self.is_food(item_stack) {
                    if self.handle_horse_eating(player, item_stack) {
                        item_stack.decrement_unless_creative(player.gamemode.load(), 1);
                    }
                    return true;
                }
                if !self.is_tame() {
                    self.make_mad();
                    return true;
                }
            }
        }

        if self.get_entity().has_passengers() || self.is_baby() {
            return false;
        }
        if should_open_inventory {
            return false;
        }

        if self.is_tame() && item == &Item::SADDLE && !self.is_saddled() {
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

        self.do_player_ride(player);
        true
    }
}
