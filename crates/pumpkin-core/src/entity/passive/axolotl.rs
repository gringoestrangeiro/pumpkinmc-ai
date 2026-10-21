use std::sync::{
    Arc, Weak,
    atomic::{AtomicBool, AtomicI32, Ordering},
};

use pumpkin_data::damage::DamageType;
use pumpkin_data::effect::StatusEffect;
use pumpkin_data::entity::EntityType;
use pumpkin_data::item::Item;
use pumpkin_data::item_stack::ItemStack;
use pumpkin_data::potion::Effect;
use pumpkin_data::sound::{Sound, SoundCategory};
use pumpkin_data::tag::{self, Taggable};
use pumpkin_nbt::compound::NbtCompound;
use pumpkin_protocol::codec::var_int::VarInt;
use pumpkin_util::math::boundingbox::{BoundingBox, EntityDimensions};
use rand::RngExt;

use crate::entity::{
    Entity, EntityBase,
    ageable::{AgeableData, AgeableMob},
    ai::goal::{
        active_target::ActiveTargetGoal, breed::BreedGoal, escape_danger::EscapeDangerGoal,
        follow_parent::FollowParentGoal, look_around::RandomLookAroundGoal,
        look_at_entity::LookAtEntityGoal, melee_attack::MeleeAttackGoal, revenge::RevengeGoal,
        swim::SwimGoal, tempt::TemptGoal, try_find_water::TryFindWaterGoal,
        wander_around::WanderAroundGoal,
    },
    breath,
    custom_sound::CustomSound,
    mob::{Mob, MobEntity},
    passive::{animal, animal::Animal, fish},
    player::Player,
};

const TEMPT_ITEMS: &[&Item] = &[&Item::TROPICAL_FISH_BUCKET];
const TOTAL_PLAYDEAD_TIME: i32 = 200;
const AXOLOTL_TOTAL_AIR_SUPPLY: i32 = 6000;
const REGEN_BUFF_BASE_DURATION: i32 = 100;
const REGEN_BUFF_MAX_DURATION: i32 = 2400;
const PLAYER_REGEN_DETECTION_RANGE: f64 = 20.0;
const RARE_VARIANT_CHANCE: i32 = 1200;
const BABY_DIMENSIONS: EntityDimensions = EntityDimensions {
    width: 0.375,
    height: 0.21,
    eye_height: 0.09375,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[repr(i32)]
pub enum AxolotlVariant {
    #[default]
    Lucy = 0,
    Wild = 1,
    Gold = 2,
    Cyan = 3,
    Blue = 4,
}

impl AxolotlVariant {
    #[must_use]
    pub const fn from_id(id: i32) -> Self {
        match id {
            1 => Self::Wild,
            2 => Self::Gold,
            3 => Self::Cyan,
            4 => Self::Blue,
            _ => Self::Lucy,
        }
    }

    #[must_use]
    pub fn from_name(name: &str) -> Option<Self> {
        match name.strip_prefix("minecraft:").unwrap_or(name) {
            "lucy" => Some(Self::Lucy),
            "wild" => Some(Self::Wild),
            "gold" => Some(Self::Gold),
            "cyan" => Some(Self::Cyan),
            "blue" => Some(Self::Blue),
            _ => None,
        }
    }

    #[must_use]
    pub const fn id(self) -> i32 {
        self as i32
    }

    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::Lucy => "lucy",
            Self::Wild => "wild",
            Self::Gold => "gold",
            Self::Cyan => "cyan",
            Self::Blue => "blue",
        }
    }

    #[must_use]
    pub fn random_variant() -> Self {
        let mut rng = rand::rng();
        if rng.random_range(0..RARE_VARIANT_CHANCE) == 0 {
            Self::Blue
        } else {
            match rng.random_range(0..4) {
                1 => Self::Wild,
                2 => Self::Gold,
                3 => Self::Cyan,
                _ => Self::Lucy,
            }
        }
    }
}

pub struct AxolotlEntity {
    pub mob_entity: MobEntity,
    pub ageable_data: AgeableData,
    pub variant: AtomicI32,
    pub playing_dead: AtomicBool,
    pub from_bucket: AtomicBool,
    pub play_dead_ticks: AtomicI32,
    pub air_supply: AtomicI32,
    hurt_by_entity: AtomicBool,
}

impl AxolotlEntity {
    pub fn new(entity: Entity) -> Arc<Self> {
        let mob_entity = MobEntity::new(entity);
        let variant = AxolotlVariant::random_variant();
        let axolotl = Self {
            mob_entity,
            ageable_data: AgeableData::default(),
            variant: AtomicI32::new(variant.id()),
            playing_dead: AtomicBool::new(false),
            from_bucket: AtomicBool::new(false),
            play_dead_ticks: AtomicI32::new(0),
            air_supply: AtomicI32::new(AXOLOTL_TOTAL_AIR_SUPPLY),
            hurt_by_entity: AtomicBool::new(false),
        };
        let mob_arc = Arc::new(axolotl);
        let mob_weak: Weak<dyn Mob> = {
            let mob_arc: Arc<dyn Mob> = mob_arc.clone();
            Arc::downgrade(&mob_arc)
        };

        mob_arc.refresh_dimensions();

        {
            let mut navigator = mob_arc
                .mob_entity
                .navigator
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            *navigator = crate::entity::ai::pathfinder::Navigator::amphibious(false);
            navigator
                .set_pathfinding_malus(crate::entity::ai::pathfinder::node::PathType::Water, 0.0);
        }

        {
            let mut goal_selector = mob_arc
                .mob_entity
                .goals_selector
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);

            goal_selector.add_goal(0, Box::new(TryFindWaterGoal));
            goal_selector.add_goal(0, Box::new(SwimGoal::default()));
            goal_selector.add_goal(1, EscapeDangerGoal::new(1.5));
            goal_selector.add_goal(2, BreedGoal::new(1.0));
            goal_selector.add_goal(3, Box::new(TemptGoal::new(1.25, TEMPT_ITEMS, false)));
            goal_selector.add_goal(4, Box::new(FollowParentGoal::new(1.25)));
            goal_selector.add_goal(5, Box::new(MeleeAttackGoal::new(1.2, false)));
            goal_selector.add_goal(6, Box::new(WanderAroundGoal::new(1.0)));
            goal_selector.add_goal(
                7,
                LookAtEntityGoal::with_default(mob_weak, &EntityType::PLAYER, 6.0),
            );
            goal_selector.add_goal(8, Box::new(RandomLookAroundGoal::default()));
        };

        {
            let mut target_selector = mob_arc
                .mob_entity
                .target_selector
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);

            target_selector.add_goal(1, Box::new(RevengeGoal::new(true)));
            target_selector.add_goal(
                2,
                ActiveTargetGoal::with_default(&mob_arc.mob_entity, &EntityType::DROWNED, false),
            );
            target_selector.add_goal(
                2,
                ActiveTargetGoal::with_default(&mob_arc.mob_entity, &EntityType::GUARDIAN, false),
            );
            target_selector.add_goal(
                2,
                ActiveTargetGoal::with_default(
                    &mob_arc.mob_entity,
                    &EntityType::ELDER_GUARDIAN,
                    false,
                ),
            );
            target_selector.add_goal(
                3,
                ActiveTargetGoal::with_default(&mob_arc.mob_entity, &EntityType::SQUID, false),
            );
            target_selector.add_goal(
                3,
                ActiveTargetGoal::with_default(&mob_arc.mob_entity, &EntityType::GLOW_SQUID, false),
            );
            target_selector.add_goal(
                3,
                ActiveTargetGoal::with_default(&mob_arc.mob_entity, &EntityType::COD, false),
            );
            target_selector.add_goal(
                3,
                ActiveTargetGoal::with_default(&mob_arc.mob_entity, &EntityType::SALMON, false),
            );
            target_selector.add_goal(
                3,
                ActiveTargetGoal::with_default(
                    &mob_arc.mob_entity,
                    &EntityType::TROPICAL_FISH,
                    false,
                ),
            );
        };

        mob_arc
    }

    #[must_use]
    pub fn get_variant(&self) -> AxolotlVariant {
        AxolotlVariant::from_id(self.variant.load(Ordering::Relaxed))
    }

    pub fn set_variant(&self, variant: AxolotlVariant) {
        self.variant.store(variant.id(), Ordering::Relaxed);
        let entity = self.get_entity();
        entity.set_synced_data(
            pumpkin_data::tracked_data::axolotl::DATA_VARIANT,
            VarInt(variant.id()),
        );
    }

    #[must_use]
    pub fn is_playing_dead(&self) -> bool {
        self.playing_dead.load(Ordering::Relaxed)
    }

    pub fn set_playing_dead(&self, playing_dead: bool) {
        self.playing_dead.store(playing_dead, Ordering::Relaxed);
        let entity = self.get_entity();
        entity.set_synced_data(
            pumpkin_data::tracked_data::axolotl::DATA_PLAYING_DEAD,
            playing_dead,
        );
    }

    fn start_playing_dead(&self) {
        self.set_playing_dead(true);
        self.play_dead_ticks
            .store(TOTAL_PLAYDEAD_TIME, Ordering::Relaxed);
        self.mob_entity.set_target(None);
        self.mob_entity
            .navigator
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .stop();
        self.get_mob_entity().living_entity.add_effect(Effect {
            effect_type: &StatusEffect::REGENERATION,
            duration: TOTAL_PLAYDEAD_TIME,
            amplifier: 0,
            ambient: false,
            show_particles: true,
            show_icon: true,
            blend: false,
        });
    }

    #[must_use]
    pub fn is_from_bucket(&self) -> bool {
        self.from_bucket.load(Ordering::Relaxed)
    }

    pub fn set_from_bucket(&self, from_bucket: bool) {
        self.from_bucket.store(from_bucket, Ordering::Relaxed);
        let entity = self.get_entity();
        entity.set_synced_data(
            pumpkin_data::tracked_data::axolotl::FROM_BUCKET,
            from_bucket,
        );
    }

    fn refresh_dimensions(&self) {
        let entity = self.get_entity();
        let dimensions = if self.is_baby() {
            BABY_DIMENSIONS
        } else {
            EntityDimensions {
                width: entity.entity_type.dimension[0],
                height: entity.entity_type.dimension[1],
                eye_height: entity.entity_type.eye_height,
            }
        };
        let pos = entity.pos.load();
        entity
            .bounding_box
            .store(BoundingBox::new_from_pos(pos.x, pos.y, pos.z, &dimensions));
        entity.entity_dimension.store(dimensions);
    }

    fn handle_air_supply(&self, caller: &dyn EntityBase) {
        breath::tick_mob_air_supply(
            caller,
            &self.air_supply,
            AXOLOTL_TOTAL_AIR_SUPPLY,
            !self.get_entity().is_in_water_or_rain(),
            DamageType::DRY_OUT,
        );
    }

    fn breed_variant(&self, partner: &dyn EntityBase) -> AxolotlVariant {
        if rand::random_range(0..RARE_VARIANT_CHANCE) == 0 {
            return AxolotlVariant::Blue;
        }
        let partner_variant = partner
            .cast_any()
            .downcast_ref::<AxolotlEntity>()
            .map_or_else(|| self.get_variant(), AxolotlEntity::get_variant);
        if rand::random_range(0..2) == 0 {
            self.get_variant()
        } else {
            partner_variant
        }
    }

    /// Vanilla `Axolotl.applySupportingEffects`: the player who helped an
    /// axolotl kill something gets regeneration and loses mining fatigue.
    fn apply_supporting_effects(&self, player: &Player) {
        let current = player.get_effect(&StatusEffect::REGENERATION);
        let previous = current.as_ref().map_or(0, |effect| effect.duration);
        if current.is_none() || previous <= REGEN_BUFF_MAX_DURATION - 1 {
            let duration = (REGEN_BUFF_BASE_DURATION + previous).min(REGEN_BUFF_MAX_DURATION);
            player.add_effect(Effect {
                effect_type: &StatusEffect::REGENERATION,
                duration,
                amplifier: 0,
                ambient: false,
                show_particles: true,
                show_icon: true,
                blend: false,
            });
        }
        player.remove_effect(&StatusEffect::MINING_FATIGUE);
    }
}

impl AgeableMob for AxolotlEntity {
    fn get_ageable_data(&self) -> &AgeableData {
        &self.ageable_data
    }

    fn age_boundary_reached(&self) {
        self.refresh_dimensions();
    }
}

impl Animal for AxolotlEntity {
    fn is_food(&self, item_stack: &ItemStack) -> bool {
        item_stack.item.has_tag(&tag::Item::MINECRAFT_AXOLOTL_FOOD)
    }
}

impl CustomSound for AxolotlEntity {
    fn death_sound(&self) -> Option<Sound> {
        Some(Sound::EntityAxolotlDeath)
    }

    fn hurt_sound(&self) -> Option<Sound> {
        Some(Sound::EntityAxolotlHurt)
    }
}

impl Mob for AxolotlEntity {
    fn as_ageable(&self) -> Option<&dyn AgeableMob> {
        Some(self)
    }

    fn as_animal(&self) -> Option<&dyn Animal> {
        Some(self)
    }

    fn as_custom_sound(&self) -> Option<&dyn CustomSound> {
        Some(self)
    }

    fn mob_write_nbt(&self, nbt: &mut NbtCompound) {
        self.write_ageable_nbt(nbt);
        nbt.put_int("Variant", self.get_variant().id());
        nbt.put_bool("FromBucket", self.is_from_bucket());
    }

    fn mob_read_nbt(&self, nbt: &NbtCompound) {
        self.read_ageable_nbt(nbt);
        if let Some(variant) = nbt.get_int("Variant") {
            self.set_variant(AxolotlVariant::from_id(variant));
        }
        if let Some(from_bucket) = nbt.get_bool("FromBucket") {
            self.set_from_bucket(from_bucket);
        }
    }

    fn get_mob_entity(&self) -> &MobEntity {
        &self.mob_entity
    }

    fn mob_tick(&self, caller: &dyn EntityBase) {
        self.ageable_ai_step();

        let play_dead_ticks = self.play_dead_ticks.load(Ordering::Relaxed);
        if play_dead_ticks > 0 {
            let remaining = play_dead_ticks - 1;
            self.play_dead_ticks.store(remaining, Ordering::Relaxed);
            if remaining == 0 {
                self.set_playing_dead(false);
            }
        }

        if !self.mob_entity.is_no_ai() {
            self.handle_air_supply(caller);
        }
    }

    fn custom_travel(&self, caller: &dyn EntityBase) -> bool {
        if !self.is_playing_dead() {
            return false;
        }
        // Vanilla stops both controls while playing dead, so the AI's movement
        // input is ignored and only drag moves the body.
        let living = &self.mob_entity.living_entity;
        let entity = &living.entity;
        let mut velocity = entity.velocity.load();
        if entity.touching_water.load(Ordering::Relaxed) {
            velocity = velocity * 0.9;
        } else {
            velocity.x *= 0.91;
            velocity.y -= caller.get_gravity();
            velocity.z *= 0.91;
        }
        entity.velocity.store(velocity);
        living.make_move(caller);
        true
    }

    fn can_attack(&self, target: &dyn EntityBase) -> bool {
        !self.is_playing_dead() && self.mob_entity.living_entity.can_attack(target)
    }

    fn pre_damage(&self, _damage_type: DamageType, source: Option<&dyn EntityBase>) -> bool {
        self.hurt_by_entity
            .store(source.is_some(), Ordering::Relaxed);
        true
    }

    fn modify_incoming_damage(&self, amount: f32, _damage_type: DamageType) -> f32 {
        let living = &self.mob_entity.living_entity;
        let health = living.health.load(Ordering::Relaxed);
        let max_health = living.get_max_health();
        if !self.mob_entity.is_no_ai()
            && rand::random_range(0..3) == 0
            && ((rand::random_range(0..3) as f32) < amount || health / max_health < 0.5)
            && amount < health
            && self.get_entity().is_in_water()
            && self.hurt_by_entity.load(Ordering::Relaxed)
            && !self.is_playing_dead()
        {
            self.start_playing_dead();
        }
        amount
    }

    fn on_attack(&self, target: &dyn EntityBase) {
        if target.get_entity().is_alive() {
            return;
        }
        let Some(target_living) = target.get_living_entity() else {
            return;
        };
        let player_id = target_living.last_hurt_by_player_id.load(Ordering::Relaxed);
        if player_id == 0 {
            return;
        }
        let entity = self.get_entity();
        let world = entity.world.load();
        let Some(player) = world.get_player_by_id(player_id) else {
            return;
        };
        let distance_sq = entity
            .pos
            .load()
            .squared_distance_to_vec(&player.get_entity().pos.load());
        if distance_sq <= PLAYER_REGEN_DETECTION_RANGE * PLAYER_REGEN_DETECTION_RANGE {
            self.apply_supporting_effects(&player);
        }
    }

    fn on_breed_offspring(&self, partner: &dyn EntityBase, baby: &dyn EntityBase) {
        let Some(baby) = baby.cast_any().downcast_ref::<AxolotlEntity>() else {
            return;
        };
        baby.set_variant(self.breed_variant(partner));
        baby.mob_entity
            .persistence_required
            .store(true, Ordering::Relaxed);
    }

    fn mob_init_data_tracker(&self) {
        let entity = self.get_entity();
        let is_baby = entity.age.load(Ordering::Relaxed) < 0;
        if is_baby {
            entity.set_synced_data(pumpkin_data::tracked_data::axolotl::DATA_BABY_ID, true);
        }
        entity.set_synced_data(
            pumpkin_data::tracked_data::axolotl::DATA_VARIANT,
            VarInt(self.get_variant().id()),
        );
        entity.set_synced_data(
            pumpkin_data::tracked_data::axolotl::DATA_PLAYING_DEAD,
            self.is_playing_dead(),
        );
        entity.set_synced_data(
            pumpkin_data::tracked_data::axolotl::FROM_BUCKET,
            self.is_from_bucket(),
        );
    }

    fn mob_set_variant_name(&self, name: &str) {
        if let Some(variant) = AxolotlVariant::from_name(name) {
            self.set_variant(variant);
        }
    }

    fn mob_interact(&self, player: &Arc<Player>, item_stack: &mut ItemStack) -> bool {
        let item = item_stack.get_item();

        if item == &Item::WATER_BUCKET {
            let entity = self.get_entity();
            let world = entity.world.load();
            if let Some(server) = world.server.upgrade() {
                let mut event = crate::plugin::api::events::player::player_bucket_entity::PlayerBucketEntityEvent {
                    player: player.clone(),
                    entity_id: entity.entity_id,
                    bucket_item: "axolotl_bucket".to_string(),
                    cancelled: false,
                };
                server.plugin_manager.fire_blocking(&server, &mut event);
                if event.cancelled {
                    return false;
                }
            }
            crate::item::items::bucket::fill_mob_bucket_stack(
                item_stack,
                player,
                &Item::AXOLOTL_BUCKET,
                self,
            );
            let pos = entity.pos.load();
            world.play_sound(Sound::ItemBucketFillAxolotl, SoundCategory::Neutral, &pos);
            entity.remove();
            return true;
        }

        let bucket_food = item == &Item::TROPICAL_FISH_BUCKET;
        let interacted = self.animal_interact(player, item_stack, Sound::EntityAxolotlIdleAir);
        if interacted && bucket_food {
            animal::give_bucket_food_remainder(player, item_stack);
        }
        interacted
    }

    fn set_from_bucket(&self, from_bucket: bool) {
        AxolotlEntity::set_from_bucket(self, from_bucket);
    }

    fn requires_custom_persistence(&self) -> bool {
        self.is_from_bucket()
    }

    fn remove_when_far_away(&self, _distance_sq: f64) -> bool {
        !self.is_from_bucket() && (**self.get_entity().custom_name.load()).is_none()
    }

    fn check_spawn_obstruction(&self, world: &crate::world::World) -> bool {
        fish::check_spawn_obstruction(self, world)
    }

    fn mob_is_pushed_by_fluids(&self) -> bool {
        false
    }

    fn get_walk_target_value(&self, _pos: &pumpkin_util::math::position::BlockPos) -> f32 {
        0.0
    }

    fn get_ambient_sound(&self) -> Option<Sound> {
        if self.is_playing_dead() {
            return None;
        }
        Some(if self.get_entity().is_in_water() {
            Sound::EntityAxolotlIdleWater
        } else {
            Sound::EntityAxolotlIdleAir
        })
    }

    fn get_ambient_sound_interval(&self) -> i32 {
        120
    }
}
