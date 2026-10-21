use std::sync::{
    Arc, Weak,
    atomic::{AtomicBool, AtomicI32, AtomicU8, Ordering},
};
use uuid::Uuid;

use pumpkin_data::{
    advancement::Advancement,
    attributes::Attributes,
    damage::DamageType,
    data_component_impl::{EquipmentSlot, EquippableImpl, FoodImpl},
    effect::StatusEffect,
    entity::EntityStatus,
    entity::EntityType,
    item::Item,
    item_stack::ItemStack,
    particle::Particle,
    potion::Effect,
    sound::{Sound, SoundCategory},
    tag::{self, Taggable},
    tracked_data,
    zombie_nautilus_variant::ZombieNautilusVariant,
};
use pumpkin_nbt::compound::NbtCompound;
use pumpkin_protocol::codec::var_int::VarInt;
use pumpkin_util::math::boundingbox::{BoundingBox, EntityDimensions};
use pumpkin_util::math::position::BlockPos;
use pumpkin_util::math::vector3::Vector3;

use crate::entity::{
    Entity, EntityBase,
    ageable::{AgeableData, AgeableMob},
    ai::goal::{
        active_target::{ActiveTargetGoal, TargetCondition},
        breed::BreedGoal,
        escape_danger::EscapeDangerGoal,
        look_around::RandomLookAroundGoal,
        look_at_entity::LookAtEntityGoal,
        melee_attack::MeleeAttackGoal,
        revenge::RevengeGoal,
        tempt::TemptGoal,
        wander_around::WanderAroundGoal,
    },
    breath,
    custom_sound::CustomSound,
    mob::{Mob, MobEntity},
    passive::{
        animal,
        animal::Animal,
        fish,
        horse::{AbstractHorse, empty_mount_inventory, open_mount_screen},
        tamable::{TamableAnimal, TamableData},
    },
    player::Player,
};

const DASH_COOLDOWN_TICKS: i32 = 40;
const DASH_MINIMUM_DURATION_TICKS: i32 = 5;
const EFFECT_DURATION: i32 = 60;
const EFFECT_REFRESH_RATE: i64 = 40;
const NAUTILUS_TOTAL_AIR_SUPPLY: i32 = 300;
const BABY_SCALE: f32 = 0.5;
const IN_WATER_SPEED_MODIFIER: f64 = 0.011;
const TEMPT_ITEMS: &[&Item] = &[
    &Item::COD,
    &Item::COOKED_COD,
    &Item::SALMON,
    &Item::COOKED_SALMON,
    &Item::PUFFERFISH,
    &Item::TROPICAL_FISH,
    &Item::PUFFERFISH_BUCKET,
    &Item::COD_BUCKET,
    &Item::SALMON_BUCKET,
    &Item::TROPICAL_FISH_BUCKET,
];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NautilusKind {
    Nautilus,
    Zombie,
}

pub struct NautilusEntity {
    pub mob_entity: MobEntity,
    pub ageable_data: AgeableData,
    pub tamable_data: TamableData,
    pub is_dashing: AtomicBool,
    pub dash_cooldown: AtomicI32,
    pub is_saddled: AtomicBool,
    pub variant: AtomicU8,
    pub kind: NautilusKind,
    stand_ticks: AtomicI32,
    air_supply: AtomicI32,
}

impl NautilusEntity {
    pub fn new(entity: Entity) -> Arc<Self> {
        Self::with_kind(entity, NautilusKind::Nautilus)
    }

    pub fn new_zombie(entity: Entity) -> Arc<Self> {
        Self::with_kind(entity, NautilusKind::Zombie)
    }

    fn with_kind(entity: Entity, kind: NautilusKind) -> Arc<Self> {
        let variant = if kind == NautilusKind::Zombie {
            let world = entity.world.load();
            ZombieNautilusVariant::select_for_biome(
                world.get_biome(&entity.block_pos.load()).registry_id,
            )
        } else {
            ZombieNautilusVariant::Temperate
        };
        let mob_entity = MobEntity::new(entity);
        let nautilus = Self {
            mob_entity,
            ageable_data: AgeableData::default(),
            tamable_data: TamableData::default(),
            is_dashing: AtomicBool::new(false),
            dash_cooldown: AtomicI32::new(0),
            is_saddled: AtomicBool::new(false),
            variant: AtomicU8::new(variant.id()),
            kind,
            stand_ticks: AtomicI32::new(0),
            air_supply: AtomicI32::new(NAUTILUS_TOTAL_AIR_SUPPLY),
        };

        let mob_arc = Arc::new(nautilus);
        let mob_weak: Weak<dyn Mob> = {
            let mob_arc: Arc<dyn Mob> = mob_arc.clone();
            Arc::downgrade(&mob_arc)
        };

        {
            let mut navigator = mob_arc
                .mob_entity
                .navigator
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            *navigator = crate::entity::ai::pathfinder::Navigator::water_bound(false);
            navigator
                .set_pathfinding_malus(crate::entity::ai::pathfinder::node::PathType::Water, 0.0);
        };

        {
            let mut goal_selector = mob_arc
                .mob_entity
                .goals_selector
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);

            goal_selector.add_goal(0, EscapeDangerGoal::new(1.6));
            if kind == NautilusKind::Nautilus {
                goal_selector.add_goal(2, BreedGoal::new(0.4));
            }
            let tempt_speed = if kind == NautilusKind::Zombie {
                0.9
            } else {
                1.3
            };
            goal_selector.add_goal(3, Box::new(TemptGoal::new(tempt_speed, TEMPT_ITEMS, false)));
            let attack_speed = if kind == NautilusKind::Zombie {
                0.5
            } else {
                0.6
            };
            goal_selector.add_goal(4, Box::new(MeleeAttackGoal::new(attack_speed, true)));
            goal_selector.add_goal(5, Box::new(WanderAroundGoal::swimming(1.0, 120)));
            goal_selector.add_goal(
                6,
                LookAtEntityGoal::with_default(mob_weak, &EntityType::PLAYER, 6.0),
            );
            goal_selector.add_goal(7, Box::new(RandomLookAroundGoal::default()));
        };

        {
            let mut target_selector = mob_arc
                .mob_entity
                .target_selector
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);

            target_selector.add_goal(1, Box::new(RevengeGoal::new(false)));
            target_selector.add_goal(
                2,
                ActiveTargetGoal::with_default(&mob_arc.mob_entity, &EntityType::PUFFERFISH, false)
                    .when(TargetCondition::Adult),
            );
        };

        mob_arc.refresh_dimensions();
        mob_arc
    }

    #[must_use]
    pub fn is_dashing(&self) -> bool {
        self.is_dashing.load(Ordering::Relaxed)
    }

    pub fn set_dashing(&self, dashing: bool) {
        self.is_dashing.store(dashing, Ordering::Relaxed);
        self.get_entity()
            .set_synced_data(tracked_data::nautilus::DASH, dashing);
    }

    #[must_use]
    pub fn get_variant(&self) -> ZombieNautilusVariant {
        ZombieNautilusVariant::from_id(self.variant.load(Ordering::Relaxed)).unwrap_or_default()
    }

    pub fn set_variant(&self, variant: ZombieNautilusVariant) {
        self.variant.store(variant.id(), Ordering::Relaxed);
        self.get_entity().set_synced_data(
            tracked_data::zombie_nautilus::DATA_VARIANT_ID,
            VarInt(i32::from(variant.id())),
        );
    }

    fn refresh_dimensions(&self) {
        let entity = self.get_entity();
        let scale = if self.is_baby() { BABY_SCALE } else { 1.0 };
        let dimensions = EntityDimensions {
            width: entity.entity_type.dimension[0] * scale,
            height: entity.entity_type.dimension[1] * scale,
            eye_height: entity.entity_type.eye_height * scale,
        };
        let pos = entity.pos.load();
        entity
            .bounding_box
            .store(BoundingBox::new_from_pos(pos.x, pos.y, pos.z, &dimensions));
        entity.entity_dimension.store(dimensions);
    }

    fn is_water(&self) -> bool {
        self.get_entity().is_submerged_in_water()
    }

    pub fn ambient_sound(&self) -> Sound {
        match self.kind {
            NautilusKind::Zombie => {
                if self.is_water() {
                    Sound::EntityZombieNautilusAmbient
                } else {
                    Sound::EntityZombieNautilusAmbientLand
                }
            }
            NautilusKind::Nautilus => {
                let is_baby = self.is_baby();
                if is_baby {
                    if self.is_water() {
                        Sound::EntityBabyNautilusAmbient
                    } else {
                        Sound::EntityBabyNautilusAmbientLand
                    }
                } else if self.is_water() {
                    Sound::EntityNautilusAmbient
                } else {
                    Sound::EntityNautilusAmbientLand
                }
            }
        }
    }

    pub fn dash_sound(&self) -> Sound {
        match self.kind {
            NautilusKind::Zombie => {
                if self.is_water() {
                    Sound::EntityZombieNautilusDash
                } else {
                    Sound::EntityZombieNautilusDashLand
                }
            }
            NautilusKind::Nautilus => {
                if self.is_water() {
                    Sound::EntityNautilusDash
                } else {
                    Sound::EntityNautilusDashLand
                }
            }
        }
    }

    pub fn dash_ready_sound(&self) -> Sound {
        match self.kind {
            NautilusKind::Zombie => {
                if self.is_water() {
                    Sound::EntityZombieNautilusDashReady
                } else {
                    Sound::EntityZombieNautilusDashReadyLand
                }
            }
            NautilusKind::Nautilus => {
                if self.is_water() {
                    Sound::EntityNautilusDashReady
                } else {
                    Sound::EntityNautilusDashReadyLand
                }
            }
        }
    }

    pub fn eat_sound(&self) -> Sound {
        match self.kind {
            NautilusKind::Zombie => Sound::EntityZombieNautilusEat,
            NautilusKind::Nautilus => {
                if self.is_baby() {
                    Sound::EntityBabyNautilusEat
                } else {
                    Sound::EntityNautilusEat
                }
            }
        }
    }

    pub fn swim_sound(&self) -> Sound {
        match self.kind {
            NautilusKind::Zombie => Sound::EntityZombieNautilusSwim,
            NautilusKind::Nautilus => {
                if self.is_baby() {
                    Sound::EntityBabyNautilusSwim
                } else {
                    Sound::EntityNautilusSwim
                }
            }
        }
    }

    fn try_to_tame(&self, player: &Player) {
        let entity = self.get_entity();
        let world = entity.world.load();
        if rand::random_range(0..3) == 0 {
            TamableAnimal::tame(self, player.gameprofile.id);
            self.mob_entity
                .navigator
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .stop();
            world.send_entity_status(entity, EntityStatus::TamingSucceeded, None);
            player.trigger_advancement_criterion(Advancement::HUSBANDRY_TAME_AN_ANIMAL, "tame");
        } else {
            world.send_entity_status(entity, EntityStatus::TamingFailed, None);
        }
        world.play_sound(self.eat_sound(), SoundCategory::Neutral, &entity.pos.load());
    }

    fn do_player_ride(&self, player: &Arc<Player>) {
        if player.get_entity().has_vehicle() {
            return;
        }
        let entity = self.get_entity();
        if entity.has_passengers() {
            return;
        }
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

    fn try_equip_armor(&self, player: &Player, item_stack: &mut ItemStack) -> bool {
        if !self.is_tame() || self.is_baby() {
            return false;
        }
        let Some(equippable) = item_stack.get_data_component::<EquippableImpl>() else {
            return false;
        };
        if !equippable.equip_on_interact || equippable.slot == &EquipmentSlot::SADDLE {
            return false;
        }
        let slot = (*equippable.slot).clone();
        if !crate::entity::mob::equipment::is_equippable_in_slot(
            self.get_entity().entity_type,
            item_stack,
            &slot,
        ) {
            return false;
        }
        let equip_sound = equippable.equip_sound.clone();
        let stack = item_stack.split_unless_creative(player.gamemode.load(), 1);
        let entity = self.get_entity();
        entity.world.load().play_sound_event(
            &equip_sound,
            SoundCategory::Neutral,
            &entity.pos.load(),
        );
        let mut previous = self.mob_entity.set_item_slot(&slot, stack);
        if !previous.is_empty()
            && !player.inventory.insert_stack_anywhere(&mut previous)
            && !previous.is_empty()
        {
            entity
                .world
                .load()
                .drop_stack(&entity.block_pos.load(), previous);
        }
        true
    }

    fn try_feed(&self, player: &Player, item_stack: &mut ItemStack) -> bool {
        let health = self.mob_entity.living_entity.health.load();
        let max_health = self.mob_entity.living_entity.get_max_health();
        if health >= max_health {
            return false;
        }
        let heal = item_stack
            .get_data_component::<FoodImpl>()
            .map_or(1.0, |food| 2.0 * food.nutrition as f32);
        let bucket_food = item_stack
            .item
            .has_tag(&tag::Item::MINECRAFT_NAUTILUS_BUCKET_FOOD);
        item_stack.decrement_unless_creative(player.gamemode.load(), 1);
        self.mob_entity.living_entity.heal(heal);
        if bucket_food {
            animal::give_bucket_food_remainder(player, item_stack);
        }
        let entity = self.get_entity();
        entity.world.load().play_sound(
            self.eat_sound(),
            SoundCategory::Neutral,
            &entity.pos.load(),
        );
        true
    }

    fn travel_in_water(&self, caller: &dyn EntityBase) -> bool {
        let living = &self.mob_entity.living_entity;
        let entity = &living.entity;
        if !entity.touching_water.load(Ordering::Relaxed) {
            return false;
        }
        entity.update_velocity_from_input(living.movement_input.load(), IN_WATER_SPEED_MODIFIER);
        living.make_move(caller);
        entity.velocity.store(entity.velocity.load() * 0.9);
        true
    }

    fn handle_air_supply(&self, caller: &dyn EntityBase) {
        breath::tick_mob_air_supply(
            caller,
            &self.air_supply,
            NAUTILUS_TOTAL_AIR_SUPPLY,
            !self.get_entity().is_in_water(),
            DamageType::DRY_OUT,
        );
    }
}

impl TamableAnimal for NautilusEntity {
    fn get_tamable_data(&self) -> &TamableData {
        &self.tamable_data
    }
}

impl AgeableMob for NautilusEntity {
    fn get_ageable_data(&self) -> &AgeableData {
        &self.ageable_data
    }

    fn age_boundary_reached(&self) {
        self.refresh_dimensions();
    }

    fn can_be_a_baby(&self) -> bool {
        self.kind == NautilusKind::Nautilus
    }
}

impl Animal for NautilusEntity {
    fn is_food(&self, item_stack: &ItemStack) -> bool {
        if !self.is_tame() && !self.is_baby() {
            item_stack
                .item
                .has_tag(&tag::Item::MINECRAFT_NAUTILUS_TAMING_ITEMS)
        } else {
            item_stack.item.has_tag(&tag::Item::MINECRAFT_NAUTILUS_FOOD)
        }
    }

    fn baby_spawned_from_breeding(&self, baby: &dyn EntityBase, _partner: &dyn EntityBase) {
        let Some(baby) = baby.cast_any().downcast_ref::<Self>() else {
            return;
        };
        if self.is_tame() {
            baby.set_tame(true);
            baby.set_owner(self.get_owner());
        }
    }
}

impl CustomSound for NautilusEntity {
    fn hurt_sound(&self) -> Option<Sound> {
        Some(match self.kind {
            NautilusKind::Zombie => {
                if self.is_water() {
                    Sound::EntityZombieNautilusHurt
                } else {
                    Sound::EntityZombieNautilusHurtLand
                }
            }
            NautilusKind::Nautilus => {
                let is_baby = self.is_baby();
                if is_baby {
                    if self.is_water() {
                        Sound::EntityBabyNautilusHurt
                    } else {
                        Sound::EntityBabyNautilusHurtLand
                    }
                } else if self.is_water() {
                    Sound::EntityNautilusHurt
                } else {
                    Sound::EntityNautilusHurtLand
                }
            }
        })
    }

    fn death_sound(&self) -> Option<Sound> {
        Some(match self.kind {
            NautilusKind::Zombie => {
                if self.is_water() {
                    Sound::EntityZombieNautilusDeath
                } else {
                    Sound::EntityZombieNautilusDeathLand
                }
            }
            NautilusKind::Nautilus => {
                let is_baby = self.is_baby();
                if is_baby {
                    if self.is_water() {
                        Sound::EntityBabyNautilusDeath
                    } else {
                        Sound::EntityBabyNautilusDeathLand
                    }
                } else if self.is_water() {
                    Sound::EntityNautilusDeath
                } else {
                    Sound::EntityNautilusDeathLand
                }
            }
        })
    }
}

impl AbstractHorse for NautilusEntity {
    fn has_horse_flag(&self, _flag: u8) -> bool {
        false
    }

    fn set_horse_flag(&self, _flag: u8, _val: bool) {}

    fn get_stand_ticks(&self) -> &AtomicI32 {
        &self.stand_ticks
    }

    fn set_horse_owner(&self, owner: Option<Uuid>) {
        self.set_owner(owner);
    }

    fn get_temper(&self) -> i32 {
        0
    }

    fn set_temper(&self, _temper: i32) {}

    fn is_horse_tamed(&self) -> bool {
        self.is_tame()
    }

    fn set_horse_tamed(&self, tame: bool) {
        self.set_tame(tame);
    }

    fn can_perform_rearing(&self) -> bool {
        false
    }

    fn can_jump(&self) -> bool {
        self.is_saddled()
    }

    fn handle_start_jump(&self, jump_scale: i32) {
        if self.dash_cooldown.load(Ordering::Relaxed) > 0 {
            return;
        }
        let entity = self.get_entity();
        let world = entity.world.load();
        world.play_sound(
            self.dash_sound(),
            SoundCategory::Neutral,
            &entity.pos.load(),
        );
        self.set_dashing(true);
        self.dash_cooldown
            .store(DASH_COOLDOWN_TICKS, Ordering::Relaxed);

        let passenger = entity
            .passengers
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .first()
            .cloned();
        if let Some(passenger) = passenger
            && let Some(player) = passenger.get_player()
        {
            let amount = if jump_scale >= 90 {
                1.0
            } else {
                0.4 + 0.4 * jump_scale as f32 / 90.0
            };
            let speed = self
                .mob_entity
                .living_entity
                .get_attribute_value(&Attributes::MOVEMENT_SPEED);
            let momentum = if entity.is_in_water() { 1.2 } else { 0.5 };
            let scale = f64::from(amount) * speed * f64::from(entity.get_block_speed_factor());
            entity.add_velocity(player.get_looking_vector() * momentum * scale);
        }
    }

    fn handle_stop_jump(&self) {}

    fn get_horse_eating_sound(&self) -> Option<Sound> {
        Some(self.eat_sound())
    }
}

impl Mob for NautilusEntity {
    fn as_tamable(&self) -> Option<&dyn TamableAnimal> {
        Some(self)
    }

    fn as_ageable(&self) -> Option<&dyn AgeableMob> {
        Some(self)
    }

    fn as_animal(&self) -> Option<&dyn Animal> {
        Some(self)
    }

    fn as_custom_sound(&self) -> Option<&dyn CustomSound> {
        Some(self)
    }

    fn as_abstract_horse(&self) -> Option<&dyn AbstractHorse> {
        Some(self)
    }

    fn spawn_as_baby(&self) -> bool {
        if self.kind == NautilusKind::Zombie {
            return false;
        }
        self.set_baby(true);
        true
    }

    fn mob_write_nbt(&self, nbt: &mut NbtCompound) {
        self.write_ageable_nbt(nbt);
        self.write_tamable_nbt(nbt);
        nbt.put_bool("Saddled", self.is_saddled.load(Ordering::Relaxed));
        nbt.put_int("DashCooldown", self.dash_cooldown.load(Ordering::Relaxed));
        if self.kind == NautilusKind::Zombie {
            nbt.put_string(
                "variant",
                format!("minecraft:{}", self.get_variant().to_name()),
            );
        }
    }

    fn mob_read_nbt(&self, nbt: &NbtCompound) {
        self.read_ageable_nbt(nbt);
        self.read_tamable_nbt(nbt);
        if let Some(saddled) = nbt.get_bool("Saddled") {
            self.is_saddled.store(saddled, Ordering::Relaxed);
            if saddled {
                self.mob_entity
                    .set_item_slot(&EquipmentSlot::SADDLE, ItemStack::new(1, &Item::SADDLE));
            }
        }
        if let Some(dash) = nbt.get_int("DashCooldown") {
            self.dash_cooldown.store(dash, Ordering::Relaxed);
        }
        if self.kind == NautilusKind::Zombie
            && let Some(variant) = nbt.get_string("variant")
            && let Some(variant) = ZombieNautilusVariant::from_name(variant)
        {
            self.set_variant(variant);
        }
    }

    fn get_mob_entity(&self) -> &MobEntity {
        &self.mob_entity
    }

    fn mob_init_data_tracker(&self) {
        let entity = self.get_entity();
        if self.kind == NautilusKind::Nautilus && self.is_baby() {
            entity.set_synced_data(tracked_data::ageable_mob::DATA_BABY_ID, true);
        }
        self.set_tame(self.is_tame());
        self.set_owner(self.get_owner());
        entity.set_synced_data(tracked_data::nautilus::DASH, self.is_dashing());
        if self.kind == NautilusKind::Zombie {
            entity.set_synced_data(
                tracked_data::zombie_nautilus::DATA_VARIANT_ID,
                VarInt(i32::from(self.get_variant().id())),
            );
        }
    }

    fn mob_set_variant_name(&self, name: &str) {
        if self.kind == NautilusKind::Zombie
            && let Some(variant) = ZombieNautilusVariant::from_name(name)
        {
            self.set_variant(variant);
        }
    }

    fn custom_travel(&self, caller: &dyn EntityBase) -> bool {
        self.travel_in_water(caller)
    }

    fn mob_tick(&self, caller: &dyn EntityBase) {
        self.ageable_ai_step();

        let entity = self.get_entity();
        let world = entity.world.load();

        let passenger = entity
            .passengers
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .first()
            .cloned();
        if let Some(passenger) = passenger
            && let Some(player) = passenger.get_player()
        {
            let has_effect = player.has_effect(&StatusEffect::BREATH_OF_THE_NAUTILUS);
            if !has_effect || world.get_world_age() % EFFECT_REFRESH_RATE == 0 {
                player.add_effect(Effect {
                    effect_type: &StatusEffect::BREATH_OF_THE_NAUTILUS,
                    duration: EFFECT_DURATION,
                    amplifier: 0,
                    ambient: true,
                    show_particles: true,
                    show_icon: true,
                    blend: true,
                });
            }
        }

        if self.is_dashing()
            && self.dash_cooldown.load(Ordering::Relaxed)
                < DASH_COOLDOWN_TICKS - DASH_MINIMUM_DURATION_TICKS
        {
            self.set_dashing(false);
        }

        let cooldown = self.dash_cooldown.load(Ordering::Relaxed);
        if cooldown > 0 {
            let next = cooldown - 1;
            self.dash_cooldown.store(next, Ordering::Relaxed);
            if next == 0 {
                world.play_sound(
                    self.dash_ready_sound(),
                    SoundCategory::Neutral,
                    &entity.pos.load(),
                );
            }
        }

        if entity.is_in_water() {
            let velo = entity.velocity.load();
            let speed = velo.length();
            let prob = (speed * 2.0).clamp(0.15, 1.0);
            if rand::random::<f64>() < prob {
                world.spawn_particle(
                    entity.pos.load() + Vector3::new(0.0, 0.25, 0.0),
                    Vector3::new(0.4, 0.4, 0.4),
                    0.5,
                    2,
                    Particle::Bubble,
                );
            }
        }

        if !self.mob_entity.is_no_ai() {
            self.handle_air_supply(caller);
        }
    }

    fn mob_interact(&self, player: &Arc<Player>, item_stack: &mut ItemStack) -> bool {
        self.mob_entity
            .persistence_required
            .store(true, Ordering::Relaxed);

        if self.is_baby() {
            return self.animal_interact(player, item_stack, self.eat_sound());
        }
        if self.is_tame() && player.get_entity().is_sneaking() {
            open_mount_screen(self.get_entity(), player, empty_mount_inventory(), 0);
            return true;
        }

        let item = item_stack.get_item();
        if !item_stack.is_empty() {
            if !self.is_tame() && self.is_food(item_stack) {
                let bucket_food = item.has_tag(&tag::Item::MINECRAFT_NAUTILUS_BUCKET_FOOD);
                item_stack.decrement_unless_creative(player.gamemode.load(), 1);
                self.try_to_tame(player);
                if bucket_food {
                    animal::give_bucket_food_remainder(player, item_stack);
                }
                return true;
            }

            if self.is_food(item_stack) && self.try_feed(player, item_stack) {
                return true;
            }

            if self.try_equip_armor(player, item_stack) {
                return true;
            }

            if self.is_tame()
                && !self.is_baby()
                && !self.is_saddled.load(Ordering::Relaxed)
                && item == &Item::SADDLE
            {
                item_stack.decrement_unless_creative(player.gamemode.load(), 1);
                self.is_saddled.store(true, Ordering::Relaxed);
                self.mob_entity
                    .set_item_slot(&EquipmentSlot::SADDLE, ItemStack::new(1, &Item::SADDLE));
                let entity = self.get_entity();
                let sound = if self.is_water() {
                    Sound::ItemNautilusSaddleUnderwaterEquip
                } else {
                    Sound::ItemNautilusSaddleEquip
                };
                entity
                    .world
                    .load()
                    .play_sound(sound, SoundCategory::Neutral, &entity.pos.load());
                return true;
            }
        }

        if self.is_tame() && !player.get_entity().is_sneaking() && !self.is_food(item_stack) {
            self.do_player_ride(player);
            return true;
        }

        self.animal_interact(player, item_stack, self.eat_sound())
    }

    fn is_saddled(&self) -> bool {
        self.is_saddled.load(Ordering::Relaxed)
    }

    fn can_attack(&self, target: &dyn EntityBase) -> bool {
        !self.is_tame()
            && !self.is_baby()
            && self.get_entity().is_in_water()
            && self.mob_entity.living_entity.can_attack(target)
    }

    fn can_be_saddled(&self) -> bool {
        self.is_tame() && !self.is_baby() && self.get_entity().is_alive()
    }

    fn set_saddled(&self, saddled: bool) {
        self.is_saddled.store(saddled, Ordering::Relaxed);
    }

    fn requires_custom_persistence(&self) -> bool {
        self.is_tame()
    }

    fn remove_when_far_away(&self, _distance_sq: f64) -> bool {
        true
    }

    fn check_spawn_obstruction(&self, world: &crate::world::World) -> bool {
        fish::check_spawn_obstruction(self, world)
    }

    fn mob_is_pushed_by_fluids(&self) -> bool {
        false
    }

    fn get_walk_target_value(&self, _pos: &BlockPos) -> f32 {
        0.0
    }

    fn sun_protection_slot(&self) -> EquipmentSlot {
        if self.kind == NautilusKind::Zombie {
            EquipmentSlot::BODY
        } else {
            EquipmentSlot::HEAD
        }
    }

    fn get_ambient_sound(&self) -> Option<Sound> {
        Some(self.ambient_sound())
    }

    fn get_ambient_sound_interval(&self) -> i32 {
        120
    }
}
