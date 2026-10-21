use std::sync::{
    Arc, Weak,
    atomic::{AtomicBool, AtomicI32, AtomicU8, Ordering},
};

use crossbeam::atomic::AtomicCell;
use pumpkin_data::entity::EntityType;
use pumpkin_data::item::Item;
use pumpkin_data::item_stack::ItemStack;
use pumpkin_data::sound::{Sound, SoundCategory};
use pumpkin_data::tag::{self, Taggable};
use pumpkin_inventory::inventory::SimpleInventory;
use pumpkin_nbt::compound::NbtCompound;
use pumpkin_protocol::codec::var_int::VarInt;
use rand::RngExt;
use uuid::Uuid;

use crate::entity::{
    Entity, EntityBase,
    ageable::{AgeableData, AgeableMob},
    ai::goal::{
        active_target::ActiveTargetGoal, breed::BreedGoal, escape_danger::EscapeDangerGoal,
        follow_parent::FollowParentGoal, llama_follow_caravan::LlamaFollowCaravanGoal,
        look_around::RandomLookAroundGoal, look_at_entity::LookAtEntityGoal,
        ranged_attack::RangedAttackGoal, revenge::RevengeGoal,
        run_around_like_crazy::RunAroundLikeCrazyGoal, swim::SwimGoal, tempt::TemptGoal,
        wander_around::WanderAroundGoal,
    },
    mob::{Mob, MobEntity, RangedAttackMob},
    passive::animal::{Animal, get_carpet_color_from_item},
    passive::llama::LlamaCaravan,
    passive::wandering_trader::WanderingTraderEntity,
    player::Player,
    projectile::llama_spit::LlamaSpitEntity,
};

use super::horse::{
    AbstractHorse, drop_horse_inventory, read_horse_inventory, write_horse_inventory,
};

const TEMPT_ITEMS: &[&Item] = &[&Item::HAY_BLOCK];
const MAX_STRENGTH: i32 = 5;

pub const FLAG_TAME: u8 = 2;
pub const FLAG_BRED: u8 = 8;
pub const FLAG_EATING: u8 = 16;
pub const FLAG_STANDING: u8 = 32;
pub const FLAG_OPEN_MOUTH: u8 = 64;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[repr(i32)]
pub enum TraderLlamaVariant {
    #[default]
    Creamy = 0,
    White = 1,
    Brown = 2,
    Gray = 3,
}

impl TraderLlamaVariant {
    #[must_use]
    pub const fn from_id(id: i32) -> Self {
        match id {
            1 => Self::White,
            2 => Self::Brown,
            3 => Self::Gray,
            _ => Self::Creamy,
        }
    }

    #[must_use]
    pub const fn id(self) -> i32 {
        self as i32
    }

    #[must_use]
    pub fn random_variant() -> Self {
        let mut rng = rand::rng();
        match rng.random_range(0..4) {
            1 => Self::White,
            2 => Self::Brown,
            3 => Self::Gray,
            _ => Self::Creamy,
        }
    }
}

pub struct TraderLlamaEntity {
    pub mob_entity: MobEntity,
    pub ageable_data: AgeableData,
    pub variant: AtomicI32,
    pub strength: AtomicI32,
    pub carpet_color: AtomicCell<Option<u8>>,
    pub flags: AtomicU8,
    pub has_chest: AtomicBool,
    pub temper: AtomicI32,
    pub stand_ticks: AtomicI32,
    pub owner: AtomicCell<Option<Uuid>>,
    pub caravan_head_id: AtomicI32,
    pub caravan_tail_id: AtomicI32,
    pub inventory: SimpleInventory,
}

impl TraderLlamaEntity {
    pub fn new(entity: Entity) -> Arc<Self> {
        let mob_entity = MobEntity::new(entity);
        let mut rng = rand::rng();
        let variant = TraderLlamaVariant::random_variant();
        let max_strength = if rng.random::<f32>() < 0.04 { 5 } else { 3 };
        let strength = 1 + rng.random_range(0..max_strength);

        let llama = Self {
            mob_entity,
            ageable_data: AgeableData::default(),
            variant: AtomicI32::new(variant.id()),
            strength: AtomicI32::new(strength),
            carpet_color: AtomicCell::new(None),
            flags: AtomicU8::new(0),
            has_chest: AtomicBool::new(false),
            temper: AtomicI32::new(0),
            stand_ticks: AtomicI32::new(0),
            owner: AtomicCell::new(None),
            caravan_head_id: AtomicI32::new(0),
            caravan_tail_id: AtomicI32::new(0),
            inventory: SimpleInventory::new(MAX_STRENGTH as usize * super::horse::INVENTORY_ROWS),
        };
        let mob_arc = Arc::new(llama);
        let mob_weak: Weak<dyn Mob> = {
            let mob_arc: Arc<dyn Mob> = mob_arc.clone();
            Arc::downgrade(&mob_arc)
        };
        let ranged_weak: Weak<dyn RangedAttackMob> = {
            let ranged_arc: Arc<dyn RangedAttackMob> = mob_arc.clone();
            Arc::downgrade(&ranged_arc)
        };

        {
            let mut goal_selector = mob_arc
                .mob_entity
                .goals_selector
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            let mut target_selector = mob_arc
                .mob_entity
                .target_selector
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);

            goal_selector.add_goal(0, Box::new(SwimGoal::default()));
            goal_selector.add_goal(1, Box::new(RunAroundLikeCrazyGoal::new(1.2)));
            goal_selector.add_goal(1, Box::new(EscapeDangerGoal::new(2.0)));
            goal_selector.add_goal(2, Box::new(LlamaFollowCaravanGoal::new(2.1)));
            goal_selector.add_goal(3, Box::new(EscapeDangerGoal::new(1.2)));
            goal_selector.add_goal(
                3,
                Box::new(RangedAttackGoal::new(ranged_weak, 1.25, 40, 20.0)),
            );
            goal_selector.add_goal(4, Box::new(BreedGoal::new(1.0)));
            goal_selector.add_goal(5, Box::new(TemptGoal::new(1.25, TEMPT_ITEMS, false)));
            goal_selector.add_goal(6, Box::new(FollowParentGoal::new(1.0)));
            goal_selector.add_goal(7, Box::new(WanderAroundGoal::new(0.7)));
            goal_selector.add_goal(
                8,
                LookAtEntityGoal::with_default(mob_weak, &EntityType::PLAYER, 6.0),
            );
            goal_selector.add_goal(9, Box::new(RandomLookAroundGoal::default()));

            target_selector.add_goal(1, Box::new(RevengeGoal::new(true)));
            target_selector.add_goal(
                2,
                ActiveTargetGoal::with_default(&mob_arc.mob_entity, &EntityType::WOLF, true),
            );
        };

        mob_arc
    }

    #[must_use]
    pub fn get_variant(&self) -> TraderLlamaVariant {
        TraderLlamaVariant::from_id(self.variant.load(Ordering::Relaxed))
    }

    pub fn set_variant(&self, variant: TraderLlamaVariant) {
        self.variant.store(variant.id(), Ordering::Relaxed);
        let entity = self.get_entity();
        entity.set_synced_data(
            pumpkin_data::tracked_data::trader_llama::DATA_VARIANT_ID,
            VarInt(variant.id()),
        );
    }

    #[must_use]
    pub fn get_strength(&self) -> i32 {
        self.strength.load(Ordering::Relaxed)
    }

    pub fn set_strength(&self, strength: i32) {
        let strength = strength.clamp(1, MAX_STRENGTH);
        self.strength.store(strength, Ordering::Relaxed);
        let entity = self.get_entity();
        entity.set_synced_data(
            pumpkin_data::tracked_data::trader_llama::DATA_STRENGTH_ID,
            VarInt(strength),
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
            pumpkin_data::tracked_data::trader_llama::DATA_ID_FLAGS,
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
    pub fn has_chest(&self) -> bool {
        self.has_chest.load(Ordering::Relaxed)
    }

    pub fn set_has_chest(&self, val: bool) {
        self.has_chest.store(val, Ordering::Relaxed);
        let entity = self.get_entity();
        entity.set_synced_data(pumpkin_data::tracked_data::trader_llama::DATA_ID_CHEST, val);
    }

    /// Vanilla `Llama.getInventoryColumns`: chest slots are `strength` rows.
    #[must_use]
    pub fn get_inventory_columns(&self) -> usize {
        if self.has_chest() {
            self.get_strength().max(0) as usize
        } else {
            0
        }
    }

    #[must_use]
    pub fn get_inventory_size(&self) -> usize {
        self.get_inventory_columns() * super::horse::INVENTORY_ROWS
    }

    fn handle_llama_eating(&self, player: &Player, item_stack: &ItemStack) -> bool {
        let item = item_stack.get_item();
        let (age_up, temper, heal) = if item == &Item::WHEAT {
            (10, 3, 2.0)
        } else if item == &Item::HAY_BLOCK {
            (90, 6, 10.0)
        } else {
            return false;
        };

        let mut item_used = false;
        let living = &self.mob_entity.living_entity;
        if living.health.load() < living.get_max_health() {
            living.heal(heal);
            item_used = true;
        }

        if item == &Item::HAY_BLOCK
            && self.is_tame()
            && self.get_age() == 0
            && !self.mob_entity.is_in_love()
        {
            self.mob_entity
                .set_love_ticks(600, Some(player.gameprofile.id));
            let entity = self.get_entity();
            entity.world.load().send_entity_status(
                entity,
                pumpkin_data::entity::EntityStatus::InLoveHearts,
                Some(pumpkin_protocol::bedrock::server::actor_event::ActorEventID::InLoveHearts),
            );
            item_used = true;
        }

        if self.is_baby() && !self.is_age_locked() {
            let entity = self.get_entity();
            entity.world.load().spawn_particle(
                entity.pos.load() + pumpkin_util::math::vector3::Vector3::new(0.0, 0.5, 0.0),
                pumpkin_util::math::vector3::Vector3::new(0.5, 0.5, 0.5),
                1.0,
                7,
                pumpkin_data::particle::Particle::HappyVillager,
            );
            self.age_up(age_up, false);
            item_used = true;
        }

        if temper > 0 && (item_used || !self.is_tame()) && self.get_temper() < self.get_max_temper()
        {
            self.modify_temper(temper);
            item_used = true;
        }

        if item_used {
            let entity = self.get_entity();
            entity.world.load().play_sound(
                Sound::EntityLlamaEat,
                SoundCategory::Neutral,
                &entity.pos.load(),
            );
        }
        item_used
    }
}

impl LlamaCaravan for TraderLlamaEntity {
    fn get_caravan_head_id(&self) -> Option<i32> {
        match self.caravan_head_id.load(Ordering::Relaxed) {
            0 => None,
            id => Some(id),
        }
    }

    fn set_caravan_head_id(&self, id: Option<i32>) {
        self.caravan_head_id
            .store(id.unwrap_or(0), Ordering::Relaxed);
    }

    fn get_caravan_tail_id(&self) -> Option<i32> {
        match self.caravan_tail_id.load(Ordering::Relaxed) {
            0 => None,
            id => Some(id),
        }
    }

    fn set_caravan_tail_id(&self, id: Option<i32>) {
        self.caravan_tail_id
            .store(id.unwrap_or(0), Ordering::Relaxed);
    }
}

impl AbstractHorse for TraderLlamaEntity {
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

    fn get_max_temper(&self) -> i32 {
        30
    }

    fn can_perform_rearing(&self) -> bool {
        false
    }

    fn handle_horse_eating(&self, player: &Player, item_stack: &ItemStack) -> bool {
        self.handle_llama_eating(player, item_stack)
    }

    fn get_horse_eating_sound(&self) -> Option<Sound> {
        Some(Sound::EntityLlamaEat)
    }

    fn play_horse_angry_sound(&self) {
        let entity = self.get_entity();
        entity.world.load().play_sound(
            Sound::EntityLlamaAngry,
            SoundCategory::Neutral,
            &entity.pos.load(),
        );
    }

    /// Vanilla `TraderLlama.doPlayerRide`: a llama leashed to a trader refuses riders.
    fn do_player_ride(&self, player: &Arc<Player>) {
        let holder = self
            .get_entity()
            .leashed_to
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .clone();
        if holder.is_some_and(|holder| holder.cast_any().is::<WanderingTraderEntity>()) {
            return;
        }
        AbstractHorse::do_player_ride(self, player);
    }
}

impl TraderLlamaEntity {
    pub fn spit(&self, target: &Arc<dyn EntityBase>) {
        let entity = self.get_entity();
        let world = entity.world.load_full();

        let spit_entity = Entity::new(world.clone(), entity.pos.load(), &EntityType::LLAMA_SPIT);
        let spit = LlamaSpitEntity::new_shot(spit_entity, entity);

        let mob_pos = entity.pos.load();
        let target_entity = target.get_entity();
        let target_pos = target_entity.pos.load();
        let target_height = f64::from(target_entity.entity_dimension.load().height);

        let dx = target_pos.x - mob_pos.x;
        let dy = (target_pos.y + target_height / 3.0) - spit.get_entity().pos.load().y;
        let dz = target_pos.z - mob_pos.z;
        let horizontal_distance = dx.hypot(dz);
        let yo = horizontal_distance * 0.2;

        spit.thrown.set_velocity(dx, dy + yo, dz, 1.5, 10.0);

        if !entity.silent.load(Ordering::Relaxed) {
            world.play_sound(Sound::EntityLlamaSpit, SoundCategory::Neutral, &mob_pos);
        }

        let spit_arc: Arc<dyn EntityBase> = Arc::new(spit);
        world.spawn_entity(spit_arc);
    }
}

impl AgeableMob for TraderLlamaEntity {
    fn get_ageable_data(&self) -> &AgeableData {
        &self.ageable_data
    }
}

impl Animal for TraderLlamaEntity {
    fn is_food(&self, item_stack: &ItemStack) -> bool {
        item_stack.item.has_tag(&tag::Item::MINECRAFT_LLAMA_FOOD)
            || item_stack
                .item
                .has_tag(&tag::Item::MINECRAFT_LLAMA_TEMPT_ITEMS)
            || item_stack.item == &Item::WHEAT
            || item_stack.item == &Item::HAY_BLOCK
    }
}

impl Mob for TraderLlamaEntity {
    fn as_ageable(&self) -> Option<&dyn AgeableMob> {
        Some(self)
    }

    fn as_animal(&self) -> Option<&dyn Animal> {
        Some(self)
    }

    fn as_abstract_horse(&self) -> Option<&dyn AbstractHorse> {
        Some(self)
    }

    fn as_caravan_llama(&self) -> Option<&dyn LlamaCaravan> {
        Some(self)
    }

    fn get_ambient_sound(&self) -> Option<Sound> {
        Some(Sound::EntityLlamaAmbient)
    }

    fn get_ambient_sound_interval(&self) -> i32 {
        400
    }

    fn mob_write_nbt(&self, nbt: &mut NbtCompound) {
        self.write_ageable_nbt(nbt);
        nbt.put_int("Variant", self.get_variant().id());
        nbt.put_int("Strength", self.get_strength());
        nbt.put_bool("ChestedHorse", self.has_chest());
        nbt.put_bool("Tame", self.is_tame());
        nbt.put_int("Temper", self.temper.load(Ordering::Relaxed));
        if let Some(owner) = self.owner.load() {
            nbt.put_uuid("Owner", owner);
        }
        if self.has_chest() {
            write_horse_inventory(&self.inventory, nbt);
        }
    }

    fn mob_read_nbt(&self, nbt: &NbtCompound) {
        self.read_ageable_nbt(nbt);
        if let Some(strength) = nbt.get_int("Strength") {
            self.set_strength(strength);
        }
        if let Some(variant) = nbt.get_int("Variant") {
            self.set_variant(TraderLlamaVariant::from_id(variant));
        }
        if let Some(chested) = nbt.get_bool("ChestedHorse") {
            self.set_has_chest(chested);
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
        read_horse_inventory(&self.inventory, nbt, self.get_inventory_size());
    }

    fn drop_custom_death_loot(&self, _params: &crate::world::loot::LootContextParameters) {
        drop_horse_inventory(self.get_entity(), &self.inventory);
        if self.has_chest() {
            self.get_entity()
                .spawn_at_location(ItemStack::new(1, &Item::CHEST));
            self.set_has_chest(false);
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
            entity.set_synced_data(pumpkin_data::tracked_data::trader_llama::DATA_BABY_ID, true);
        }
        entity.set_synced_data(
            pumpkin_data::tracked_data::trader_llama::DATA_VARIANT_ID,
            VarInt(self.get_variant().id()),
        );
        entity.set_synced_data(
            pumpkin_data::tracked_data::trader_llama::DATA_STRENGTH_ID,
            VarInt(self.get_strength()),
        );
        entity.set_synced_data(
            pumpkin_data::tracked_data::trader_llama::DATA_ID_CHEST,
            self.has_chest(),
        );
        entity.set_synced_data(
            pumpkin_data::tracked_data::trader_llama::DATA_ID_FLAGS,
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
                if !self.has_chest() && item == &Item::CHEST {
                    self.set_has_chest(true);
                    item_stack.decrement_unless_creative(player.gamemode.load(), 1);
                    let entity = self.get_entity();
                    entity.world.load().play_sound(
                        Sound::EntityLlamaChest,
                        SoundCategory::Neutral,
                        &entity.pos.load(),
                    );
                    return true;
                }
            }
        }

        if self.is_tame()
            && !self.is_baby()
            && let Some(color) = get_carpet_color_from_item(item)
        {
            self.carpet_color.store(Some(color));
            item_stack.decrement_unless_creative(player.gamemode.load(), 1);
            let entity = self.get_entity();
            entity.world.load().play_sound(
                Sound::EntityLlamaSwag,
                SoundCategory::Neutral,
                &entity.pos.load(),
            );
            return true;
        }

        if self.get_entity().has_passengers() || self.is_baby() {
            return self.animal_interact(player, item_stack, Sound::EntityLlamaAmbient);
        }
        if should_open_inventory {
            return false;
        }

        self.do_player_ride(player);
        true
    }
}

impl RangedAttackMob for TraderLlamaEntity {
    fn perform_ranged_attack(&self, target: &Arc<dyn EntityBase>, _power: f32) {
        self.spit(target);
    }
}
