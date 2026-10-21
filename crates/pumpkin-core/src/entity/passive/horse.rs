use std::sync::{
    Arc, Weak,
    atomic::{AtomicI32, AtomicU8, Ordering},
};

use crossbeam::atomic::AtomicCell;
use pumpkin_data::advancement::Advancement;
use pumpkin_data::entity::{EntityStatus, EntityType};
use pumpkin_data::item::Item;
use pumpkin_data::item_stack::ItemStack;
use pumpkin_data::particle::Particle;
use pumpkin_data::sound::{Sound, SoundCategory};
use pumpkin_data::tag::{self, Taggable};
use pumpkin_inventory::inventory::{Inventory, SimpleInventory};
use pumpkin_nbt::compound::NbtCompound;
use pumpkin_nbt::tag::NbtTag;
use pumpkin_protocol::bedrock::server::actor_event::ActorEventID;
use pumpkin_protocol::codec::var_int::VarInt;
use pumpkin_util::math::vector3::Vector3;
use rand::RngExt;
use uuid::Uuid;

use crate::entity::{
    Entity, EntityBase,
    ageable::{AgeableData, AgeableMob},
    ai::goal::{
        ambient_stand::AmbientStandGoal, breed::BreedGoal, escape_danger::EscapeDangerGoal,
        follow_parent::FollowParentGoal, look_around::RandomLookAroundGoal,
        look_at_entity::LookAtEntityGoal, run_around_like_crazy::RunAroundLikeCrazyGoal,
        swim::SwimGoal, tempt::TemptGoal, wander_around::WanderAroundGoal,
    },
    mob::{Mob, MobEntity},
    passive::animal::Animal,
    player::Player,
};

const TEMPT_ITEMS: &[&Item] = &[
    &Item::GOLDEN_APPLE,
    &Item::ENCHANTED_GOLDEN_APPLE,
    &Item::GOLDEN_CARROT,
];

pub const FLAG_TAME: u8 = 2;
pub const FLAG_SADDLE: u8 = 4;
pub const FLAG_BRED: u8 = 8;
pub const FLAG_EATING: u8 = 16;
pub const FLAG_STANDING: u8 = 32;
pub const FLAG_OPEN_MOUTH: u8 = 64;

/// Vanilla `AbstractHorse.getMaxTemper` for horses, donkeys and mules.
pub const MAX_TEMPER: i32 = 100;
/// Vanilla `AbstractHorse.INVENTORY_ROWS`.
pub const INVENTORY_ROWS: usize = 3;

/// Vanilla `AbstractHorse`, the parts shared by horses, donkeys, mules, llamas and camels.
pub trait AbstractHorse: Animal + AgeableMob {
    fn has_horse_flag(&self, flag: u8) -> bool;

    fn set_horse_flag(&self, flag: u8, val: bool);

    fn get_stand_ticks(&self) -> &AtomicI32;

    fn set_horse_owner(&self, owner: Option<Uuid>);

    fn get_temper(&self) -> i32;

    fn set_temper(&self, temper: i32);

    fn get_max_temper(&self) -> i32 {
        MAX_TEMPER
    }

    fn modify_temper(&self, amount: i32) -> i32 {
        let temper = (self.get_temper() + amount).clamp(0, self.get_max_temper());
        self.set_temper(temper);
        temper
    }

    fn is_horse_tamed(&self) -> bool {
        self.has_horse_flag(FLAG_TAME)
    }

    fn set_horse_tamed(&self, tame: bool) {
        self.set_horse_flag(FLAG_TAME, tame);
    }

    fn is_horse_eating(&self) -> bool {
        self.has_horse_flag(FLAG_EATING)
    }

    fn set_horse_eating(&self, eating: bool) {
        self.set_horse_flag(FLAG_EATING, eating);
    }

    fn is_horse_standing(&self) -> bool {
        self.has_horse_flag(FLAG_STANDING)
    }

    fn set_horse_standing(&self, ticks: i32) {
        self.set_horse_eating(false);
        self.set_horse_flag(FLAG_STANDING, true);
        self.get_stand_ticks().store(ticks, Ordering::Relaxed);
    }

    fn clear_horse_standing(&self) {
        self.set_horse_flag(FLAG_STANDING, false);
        self.get_stand_ticks().store(0, Ordering::Relaxed);
    }

    fn can_perform_rearing(&self) -> bool {
        true
    }

    /// Vanilla `AbstractHorse.getAmbientStandSound`: the ambient sound unless overridden.
    fn get_horse_ambient_stand_sound(&self) -> Option<Sound> {
        self.get_ambient_sound()
    }

    fn get_horse_eating_sound(&self) -> Option<Sound> {
        None
    }

    fn play_horse_jump_sound(&self) {
        let entity = self.get_entity();
        entity.world.load().play_sound(
            Sound::EntityHorseJump,
            SoundCategory::Neutral,
            &entity.pos.load(),
        );
    }

    fn play_horse_angry_sound(&self) {}

    /// Vanilla `AbstractHorse.standIfPossible`.
    fn stand_if_possible(&self) {
        if self.can_perform_rearing() {
            self.set_horse_standing(20);
        }
    }

    /// Vanilla `AbstractHorse.makeMad`.
    fn make_mad(&self) {
        if !self.is_horse_standing() {
            self.stand_if_possible();
            self.play_horse_angry_sound();
        }
    }

    /// Vanilla `AbstractHorse.tameWithName`.
    fn tame_with_name(&self, player: &Player) -> bool {
        self.set_horse_owner(Some(player.gameprofile.id));
        self.set_horse_tamed(true);
        player.trigger_advancement_criterion(Advancement::HUSBANDRY_TAME_AN_ANIMAL, "tame");
        self.get_entity().world.load().broadcast_entity_event(
            self.get_entity(),
            EntityStatus::TamingSucceeded,
            Some(ActorEventID::TamingSucceeded),
        );
        true
    }

    /// Vanilla `AbstractHorse.doPlayerRide`.
    fn do_player_ride(&self, player: &Arc<Player>) {
        self.set_horse_eating(false);
        self.clear_horse_standing();
        if player.get_entity().has_vehicle() {
            return;
        }

        let entity = self.get_entity();
        player
            .get_entity()
            .set_rotation(entity.yaw.load(), entity.pitch.load());

        let world = entity.world.load();
        let Some(vehicle) = world.get_entity_by_id(entity.entity_id) else {
            return;
        };
        let Some(passenger) = world.get_player_by_id(player.entity_id()) else {
            return;
        };
        entity.add_passenger(vehicle, passenger as Arc<dyn EntityBase>);
    }

    /// Vanilla `AbstractHorse.onPlayerJump`: the server side of a charged jump.
    fn on_player_jump(&self, jump_amount: i32) {
        if Mob::is_saddled(self) {
            if jump_amount > 0 {
                self.stand_if_possible();
            }
        }
    }

    /// Vanilla `AbstractHorse.handleStartJump`.
    fn handle_start_jump(&self, _jump_scale: i32) {
        self.stand_if_possible();
        self.play_horse_jump_sound();
    }

    fn handle_stop_jump(&self) {}

    fn can_jump(&self) -> bool {
        Mob::is_saddled(self)
    }

    /// Vanilla `AbstractHorse.aiStep` timers that affect the synced flags.
    fn horse_tick(&self) {
        let stand_ticks = self.get_stand_ticks().load(Ordering::Relaxed);
        if stand_ticks > 0 {
            let next = stand_ticks - 1;
            self.get_stand_ticks().store(next, Ordering::Relaxed);
            if next <= 0 {
                self.clear_horse_standing();
            }
        }
    }

    /// Vanilla `AbstractHorse.handleEating` for horses, donkeys and mules.
    fn handle_horse_eating(&self, player: &Player, item_stack: &ItemStack) -> bool {
        let mut item_used = false;
        let (heal, age_up, temper, in_love) = horse_food_values(item_stack);

        let living = &self.get_mob_entity().living_entity;
        if living.health.load() < living.get_max_health() && heal > 0.0 {
            living.heal(heal);
            item_used = true;
        }

        let entity = self.get_entity();
        let world = entity.world.load();
        if self.is_baby() && age_up > 0 && !self.is_age_locked() {
            world.spawn_particle(
                entity.pos.load() + Vector3::new(0.0, 0.5, 0.0),
                Vector3::new(0.5, 0.5, 0.5),
                1.0,
                7,
                Particle::HappyVillager,
            );
            self.age_up(age_up, false);
            item_used = true;
        }

        if in_love
            && self.is_horse_tamed()
            && self.get_age() == 0
            && !self.get_mob_entity().is_in_love()
        {
            self.get_mob_entity()
                .set_love_ticks(600, Some(player.gameprofile.id));
            world.send_entity_status(
                entity,
                EntityStatus::InLoveHearts,
                Some(ActorEventID::InLoveHearts),
            );
            item_used = true;
        }

        if temper > 0
            && (item_used || !self.is_horse_tamed())
            && self.get_temper() < self.get_max_temper()
        {
            self.modify_temper(temper);
            item_used = true;
        }

        if item_used {
            if let Some(sound) = self.get_horse_eating_sound() {
                world.play_sound(sound, SoundCategory::Neutral, &entity.pos.load());
            }
        }

        item_used
    }
}

fn horse_food_values(item_stack: &ItemStack) -> (f32, i32, i32, bool) {
    let item = item_stack.get_item();
    if item == &Item::WHEAT {
        (2.0, 20, 3, false)
    } else if item == &Item::SUGAR {
        (1.0, 30, 3, false)
    } else if item == &Item::HAY_BLOCK {
        (20.0, 180, 0, false)
    } else if item == &Item::APPLE {
        (3.0, 60, 3, false)
    } else if item == &Item::RED_MUSHROOM {
        (3.0, 0, 3, false)
    } else if item == &Item::CARROT {
        (3.0, 60, 3, false)
    } else if item == &Item::GOLDEN_CARROT {
        (4.0, 60, 5, true)
    } else if item == &Item::GOLDEN_APPLE || item == &Item::ENCHANTED_GOLDEN_APPLE {
        (10.0, 240, 10, true)
    } else {
        (0.0, 0, 0, false)
    }
}

/// Reads a chested horse inventory. Slots past `capacity` are ignored, like the
/// vanilla `ItemStackWithSlot.isValidInContainer` check.
pub(crate) fn read_horse_inventory(
    inventory: &SimpleInventory,
    nbt: &NbtCompound,
    capacity: usize,
) {
    let mut items = inventory
        .stacks
        .write()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    items.fill_with(|| ItemStack::EMPTY.clone());
    let Some(list) = nbt.get_list("Items") else {
        return;
    };
    for tag in list {
        let Some(compound) = tag.extract_compound() else {
            continue;
        };
        let Some(slot) = compound.get_byte("Slot") else {
            continue;
        };
        let slot = slot as usize;
        if slot < capacity
            && let Some(stack) = ItemStack::read_item_stack(compound)
        {
            items[slot] = stack;
        }
    }
}

pub(crate) fn write_horse_inventory(inventory: &SimpleInventory, nbt: &mut NbtCompound) {
    let items = inventory
        .stacks
        .read()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    let mut list = Vec::new();
    for (slot, stack) in items.iter().enumerate() {
        if stack.is_empty() {
            continue;
        }
        let mut compound = NbtCompound::new();
        compound.put_byte("Slot", slot as i8);
        stack.write_item_stack(&mut compound);
        list.push(NbtTag::Compound(compound));
    }
    nbt.put("Items", NbtTag::List(list));
}

/// Mounts without a chest still get the saddle and body slots.
pub(crate) fn empty_mount_inventory() -> Arc<dyn Inventory> {
    Arc::new(SimpleInventory::new(0))
}

/// Vanilla `AbstractHorse.openCustomInventoryScreen`.
pub(crate) fn open_mount_screen(
    entity: &Entity,
    player: &Arc<Player>,
    inventory: Arc<dyn Inventory>,
    inventory_columns: usize,
) -> bool {
    if entity.has_passengers() && !entity.has_passenger(player.entity_id()) {
        return false;
    }
    let Some(mount) = entity.world.load().get_entity_by_id(entity.entity_id) else {
        return false;
    };
    player
        .open_mount_screen(&mount, inventory, inventory_columns)
        .is_some()
}

pub(crate) fn drop_horse_inventory(entity: &Entity, inventory: &SimpleInventory) {
    for slot in 0..inventory.size() {
        entity.spawn_at_location(inventory.remove_stack(slot));
    }
}

pub struct HorseEntity {
    pub mob_entity: MobEntity,
    pub ageable_data: AgeableData,
    pub variant: AtomicI32,
    pub flags: AtomicU8,
    pub temper: AtomicI32,
    pub stand_ticks: AtomicI32,
    pub owner: AtomicCell<Option<Uuid>>,
}

impl HorseEntity {
    pub fn new(entity: Entity) -> Arc<Self> {
        let mob_entity = MobEntity::new(entity);
        let mut rng = rand::rng();
        let color = rng.random_range(0..7);
        let style = rng.random_range(0..5);
        let variant = color | (style << 8);

        let horse = Self {
            mob_entity,
            ageable_data: AgeableData::default(),
            variant: AtomicI32::new(variant),
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
            goal_selector.add_goal(2, BreedGoal::new(1.0));
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
    pub fn get_variant(&self) -> i32 {
        self.variant.load(Ordering::Relaxed)
    }

    pub fn set_variant(&self, variant: i32) {
        self.variant.store(variant, Ordering::Relaxed);
        let entity = self.get_entity();
        entity.set_synced_data(
            pumpkin_data::tracked_data::horse::DATA_ID_TYPE_VARIANT,
            VarInt(variant),
        );
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
            pumpkin_data::tracked_data::horse::DATA_ID_FLAGS,
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

impl AgeableMob for HorseEntity {
    fn get_ageable_data(&self) -> &AgeableData {
        &self.ageable_data
    }
}

impl Animal for HorseEntity {
    fn is_food(&self, item_stack: &ItemStack) -> bool {
        item_stack.item.has_tag(&tag::Item::MINECRAFT_HORSE_FOOD)
            || item_stack.item == &Item::WHEAT
            || item_stack.item == &Item::SUGAR
            || item_stack.item == &Item::HAY_BLOCK
            || item_stack.item == &Item::APPLE
            || item_stack.item == &Item::GOLDEN_CARROT
            || item_stack.item == &Item::GOLDEN_APPLE
            || item_stack.item == &Item::ENCHANTED_GOLDEN_APPLE
    }
}

impl AbstractHorse for HorseEntity {
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
        Some(Sound::EntityHorseEat)
    }

    fn play_horse_angry_sound(&self) {
        let entity = self.get_entity();
        entity.world.load().play_sound(
            Sound::EntityHorseAngry,
            SoundCategory::Neutral,
            &entity.pos.load(),
        );
    }
}

impl Mob for HorseEntity {
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
        HorseEntity::is_saddled(self)
    }

    fn set_saddled(&self, saddled: bool) {
        HorseEntity::set_saddled(self, saddled);
    }

    fn get_ambient_sound(&self) -> Option<Sound> {
        Some(Sound::EntityHorseAmbient)
    }

    fn get_ambient_sound_interval(&self) -> i32 {
        400
    }

    fn mob_write_nbt(&self, nbt: &mut NbtCompound) {
        self.write_ageable_nbt(nbt);
        nbt.put_int("Variant", self.get_variant());
        nbt.put_bool("Tame", self.is_tame());
        nbt.put_int("Temper", self.temper.load(Ordering::Relaxed));
        if let Some(owner) = self.owner.load() {
            nbt.put_uuid("Owner", owner);
        }
    }

    fn mob_read_nbt(&self, nbt: &NbtCompound) {
        self.read_ageable_nbt(nbt);
        if let Some(variant) = nbt.get_int("Variant") {
            self.set_variant(variant);
        }
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
            entity.set_synced_data(pumpkin_data::tracked_data::horse::DATA_BABY_ID, true);
        }
        entity.set_synced_data(
            pumpkin_data::tracked_data::horse::DATA_ID_TYPE_VARIANT,
            VarInt(self.get_variant()),
        );
        entity.set_synced_data(
            pumpkin_data::tracked_data::horse::DATA_ID_FLAGS,
            self.flags.load(Ordering::Relaxed) as i8,
        );
    }

    fn mob_interact(&self, player: &Arc<Player>, item_stack: &mut ItemStack) -> bool {
        let item = item_stack.get_item();
        let should_open_inventory =
            !self.is_baby() && self.is_tame() && player.get_entity().is_sneaking();
        let holding_golden_dandelion = item == &Item::GOLDEN_DANDELION;

        if !self.get_entity().has_passengers()
            && !should_open_inventory
            && (!self.is_baby() || !holding_golden_dandelion)
        {
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
            return self.animal_interact(player, item_stack, Sound::EntityHorseAmbient);
        }
        if should_open_inventory {
            open_mount_screen(self.get_entity(), player, empty_mount_inventory(), 0);
            return true;
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
