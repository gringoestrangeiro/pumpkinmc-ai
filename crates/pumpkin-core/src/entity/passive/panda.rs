use std::sync::{
    Arc, Mutex, Weak,
    atomic::{AtomicBool, AtomicI32, AtomicU8, Ordering},
};

use pumpkin_data::attributes::Attributes;
use pumpkin_data::entity::{EntityStatus, EntityType};
use pumpkin_data::item::Item;
use pumpkin_data::item_stack::ItemStack;
use pumpkin_data::particle::Particle;
use pumpkin_data::sound::{Sound, SoundCategory};
use pumpkin_data::tag::{self, Taggable};
use pumpkin_nbt::compound::NbtCompound;
use pumpkin_protocol::bedrock::server::actor_event::ActorEventID;
use pumpkin_protocol::codec::var_int::VarInt;
use pumpkin_util::math::position::BlockPos;
use pumpkin_util::math::vector3::Vector3;

use crate::entity::ageable::BABY_START_AGE;
use crate::entity::ai::goal::melee_attack::MeleeAttackGoal;
use crate::entity::ai::goal::revenge::RevengeGoal;
use crate::entity::ai::goal::{Controls, Goal, breed::BreedGoal, to_goal_ticks};
use crate::entity::{
    Entity, EntityBase,
    ageable::{AgeableData, AgeableMob},
    ai::goal::{
        avoid_entity::AvoidEntityGoal, escape_danger::EscapeDangerGoal,
        follow_parent::FollowParentGoal, look_around::RandomLookAroundGoal,
        look_at_entity::LookAtEntityGoal, swim::SwimGoal, tempt::TemptGoal,
        wander_around::WanderAroundGoal,
    },
    mob::{Mob, MobEntity, spawn},
    passive::animal::Animal,
    player::Player,
};

const TEMPT_ITEMS: &[&Item] = &[&Item::BAMBOO];

pub const FLAG_SNEEZE: u8 = 2;
pub const FLAG_ROLL: u8 = 4;
pub const FLAG_SIT: u8 = 8;
pub const FLAG_ON_BACK: u8 = 16;

const TOTAL_ROLL_STEPS: i32 = 32;
const TOTAL_UNHAPPY_TIME: i32 = 32;
const EAT_TICK_INTERVAL: i32 = 5;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[repr(u8)]
pub enum PandaGene {
    #[default]
    Normal = 0,
    Lazy = 1,
    Worried = 2,
    Playful = 3,
    Brown = 4,
    Weak = 5,
    Aggressive = 6,
}

impl PandaGene {
    #[must_use]
    pub const fn from_id(id: u8) -> Self {
        match id {
            1 => Self::Lazy,
            2 => Self::Worried,
            3 => Self::Playful,
            4 => Self::Brown,
            5 => Self::Weak,
            6 => Self::Aggressive,
            _ => Self::Normal,
        }
    }

    #[must_use]
    pub const fn id(self) -> u8 {
        self as u8
    }

    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Normal => "normal",
            Self::Lazy => "lazy",
            Self::Worried => "worried",
            Self::Playful => "playful",
            Self::Brown => "brown",
            Self::Weak => "weak",
            Self::Aggressive => "aggressive",
        }
    }

    #[must_use]
    pub fn from_name(name: &str) -> Self {
        match name {
            "lazy" => Self::Lazy,
            "worried" => Self::Worried,
            "playful" => Self::Playful,
            "brown" => Self::Brown,
            "weak" => Self::Weak,
            "aggressive" => Self::Aggressive,
            _ => Self::Normal,
        }
    }

    #[must_use]
    pub const fn is_recessive(self) -> bool {
        matches!(self, Self::Brown | Self::Weak)
    }

    /// Vanilla `Panda.Gene.getVariantFromGenes`.
    #[must_use]
    pub const fn variant_from_genes(main: Self, hidden: Self) -> Self {
        if main.is_recessive() {
            if main as u8 == hidden as u8 {
                main
            } else {
                Self::Normal
            }
        } else {
            main
        }
    }

    #[must_use]
    pub fn random_gene() -> Self {
        match rand::random_range(0..16) {
            0 => Self::Lazy,
            1 => Self::Worried,
            2 => Self::Playful,
            4 => Self::Aggressive,
            3 | 5..=8 => Self::Weak,
            9 | 10 => Self::Brown,
            _ => Self::Normal,
        }
    }
}

pub struct PandaEntity {
    pub mob_entity: MobEntity,
    pub ageable_data: AgeableData,
    pub main_gene: AtomicU8,
    pub hidden_gene: AtomicU8,
    pub flags: AtomicU8,
    pub eat_counter: AtomicI32,
    pub sneeze_counter: AtomicI32,
    pub unhappy_counter: AtomicI32,
    got_bamboo: AtomicBool,
    did_bite: AtomicBool,
    roll_counter: AtomicI32,
    roll_delta: Mutex<Option<Vector3<f64>>>,
    unhappy_cooldown: AtomicI32,
}

impl PandaEntity {
    pub fn new(entity: Entity) -> Arc<Self> {
        let mob_entity = MobEntity::new(entity);
        mob_entity.set_can_pick_up_loot(true);
        let gene = PandaGene::random_gene();
        let hidden = PandaGene::random_gene();
        let panda = Self {
            mob_entity,
            ageable_data: AgeableData::default(),
            main_gene: AtomicU8::new(gene.id()),
            hidden_gene: AtomicU8::new(hidden.id()),
            flags: AtomicU8::new(0),
            eat_counter: AtomicI32::new(0),
            sneeze_counter: AtomicI32::new(0),
            unhappy_counter: AtomicI32::new(0),
            got_bamboo: AtomicBool::new(false),
            did_bite: AtomicBool::new(false),
            roll_counter: AtomicI32::new(0),
            roll_delta: Mutex::new(None),
            unhappy_cooldown: AtomicI32::new(0),
        };
        let mob_arc = Arc::new(panda);
        mob_arc.set_attributes();
        let mob_weak: Weak<dyn Mob> = {
            let mob_arc: Arc<dyn Mob> = mob_arc.clone();
            Arc::downgrade(&mob_arc)
        };
        let panda_weak = Arc::downgrade(&mob_arc);

        {
            let mut goal_selector = mob_arc
                .mob_entity
                .goals_selector
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);

            goal_selector.add_goal(0, Box::new(SwimGoal::default()));
            goal_selector.add_goal(2, EscapeDangerGoal::new(2.0));
            goal_selector.add_goal(
                2,
                BreedGoal::new(1.0).with_can_use_gate({
                    let panda = panda_weak.clone();
                    move |_mob| panda.upgrade().is_some_and(|panda| panda.can_breed())
                }),
            );
            goal_selector.add_goal(
                3,
                Box::new(MeleeAttackGoal::new(1.2, true).gated_by(panda_can_perform_action)),
            );
            goal_selector.add_goal(4, Box::new(TemptGoal::new(1.0, TEMPT_ITEMS, false)));
            goal_selector.add_goal(
                6,
                Box::new(AvoidEntityGoal::with_gate(
                    &EntityType::PLAYER,
                    8.0,
                    2.0,
                    2.0,
                    panda_can_avoid,
                )),
            );
            goal_selector.add_goal(
                6,
                Box::new(AvoidEntityGoal::with_gate(
                    &EntityType::ZOMBIE,
                    4.0,
                    2.0,
                    2.0,
                    panda_can_avoid,
                )),
            );
            goal_selector.add_goal(7, Box::new(PandaSitGoal::new(panda_weak.clone())));
            goal_selector.add_goal(8, Box::new(PandaLieOnBackGoal::new(panda_weak.clone())));
            goal_selector.add_goal(8, Box::new(PandaSneezeGoal::new(panda_weak.clone())));
            goal_selector.add_goal(
                9,
                LookAtEntityGoal::with_default(mob_weak, &EntityType::PLAYER, 6.0),
            );
            goal_selector.add_goal(10, Box::new(RandomLookAroundGoal::default()));
            goal_selector.add_goal(12, Box::new(PandaRollGoal::new(panda_weak)));
            goal_selector.add_goal(13, Box::new(FollowParentGoal::new(1.25)));
            goal_selector.add_goal(14, Box::new(WanderAroundGoal::new(1.0)));
        };

        {
            let mut target_selector = mob_arc
                .mob_entity
                .target_selector
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            target_selector.add_goal(
                1,
                Box::new(
                    RevengeGoal::new(true)
                        .alerting_others()
                        .alerting_only(panda_is_aggressive)
                        .continuing_while(panda_has_not_bitten),
                ),
            );
        };

        mob_arc
    }

    fn tick_count(&self) -> i32 {
        self.get_entity().world.load().get_world_age() as i32
    }

    #[must_use]
    pub fn get_main_gene(&self) -> PandaGene {
        PandaGene::from_id(self.main_gene.load(Ordering::Relaxed))
    }

    pub fn set_main_gene(&self, gene: PandaGene) {
        self.main_gene.store(gene.id(), Ordering::Relaxed);
        let entity = self.get_entity();
        entity.set_synced_data(
            pumpkin_data::tracked_data::panda::MAIN_GENE_ID,
            gene.id() as i8,
        );
    }

    #[must_use]
    pub fn get_hidden_gene(&self) -> PandaGene {
        PandaGene::from_id(self.hidden_gene.load(Ordering::Relaxed))
    }

    pub fn set_hidden_gene(&self, gene: PandaGene) {
        self.hidden_gene.store(gene.id(), Ordering::Relaxed);
        let entity = self.get_entity();
        entity.set_synced_data(
            pumpkin_data::tracked_data::panda::HIDDEN_GENE_ID,
            gene.id() as i8,
        );
    }

    #[must_use]
    pub fn get_variant(&self) -> PandaGene {
        PandaGene::variant_from_genes(self.get_main_gene(), self.get_hidden_gene())
    }

    #[must_use]
    pub fn is_lazy(&self) -> bool {
        self.get_variant() == PandaGene::Lazy
    }

    #[must_use]
    pub fn is_worried(&self) -> bool {
        self.get_variant() == PandaGene::Worried
    }

    #[must_use]
    pub fn is_playful(&self) -> bool {
        self.get_variant() == PandaGene::Playful
    }

    #[must_use]
    pub fn is_brown(&self) -> bool {
        self.get_variant() == PandaGene::Brown
    }

    #[must_use]
    pub fn is_weak(&self) -> bool {
        self.get_variant() == PandaGene::Weak
    }

    #[must_use]
    pub fn is_aggressive(&self) -> bool {
        self.get_variant() == PandaGene::Aggressive
    }

    /// Vanilla `Panda.setAttributes`: weak pandas have half health, lazy pandas walk slowly.
    fn set_attributes(&self) {
        let living = &self.mob_entity.living_entity;
        living.set_attribute_base(&Attributes::MAX_HEALTH, 20.0);
        living.set_attribute_base(&Attributes::MOVEMENT_SPEED, 0.15);
        if self.is_weak() {
            living.set_attribute_base(&Attributes::MAX_HEALTH, 10.0);
            if living.health.load() > 10.0 {
                living.health.store(10.0);
            }
        }
        if self.is_lazy() {
            living.set_attribute_base(&Attributes::MOVEMENT_SPEED, 0.07);
        }
    }

    fn get_one_of_genes_randomly(&self) -> PandaGene {
        if rand::random::<bool>() {
            self.get_main_gene()
        } else {
            self.get_hidden_gene()
        }
    }

    /// Vanilla `Panda.setGeneFromParents`.
    fn set_gene_from_parents(&self, parent1: &Self, parent2: &Self) {
        self.set_main_gene(parent1.get_one_of_genes_randomly());
        self.set_hidden_gene(parent2.get_one_of_genes_randomly());
        if rand::random_range(0..32) == 0 {
            self.set_main_gene(PandaGene::random_gene());
        }
        if rand::random_range(0..32) == 0 {
            self.set_hidden_gene(PandaGene::random_gene());
        }
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
            pumpkin_data::tracked_data::panda::DATA_ID_FLAGS,
            new_flags as i8,
        );
    }

    #[must_use]
    pub fn is_sneezing(&self) -> bool {
        self.has_flag(FLAG_SNEEZE)
    }

    pub fn set_sneezing(&self, val: bool) {
        self.set_flag(FLAG_SNEEZE, val);
        if !val {
            self.sneeze_counter.store(0, Ordering::Relaxed);
        }
    }

    fn set_sneeze_counter(&self, value: i32) {
        self.sneeze_counter.store(value, Ordering::Relaxed);
        self.get_entity().set_synced_data(
            pumpkin_data::tracked_data::panda::SNEEZE_COUNTER,
            VarInt(value),
        );
    }

    #[must_use]
    pub fn is_sitting(&self) -> bool {
        self.has_flag(FLAG_SIT)
    }

    pub fn set_sitting(&self, val: bool) {
        self.set_flag(FLAG_SIT, val);
    }

    #[must_use]
    pub fn is_on_back(&self) -> bool {
        self.has_flag(FLAG_ON_BACK)
    }

    pub fn set_on_back(&self, val: bool) {
        self.set_flag(FLAG_ON_BACK, val);
    }

    #[must_use]
    pub fn is_rolling(&self) -> bool {
        self.has_flag(FLAG_ROLL)
    }

    pub fn set_rolling(&self, val: bool) {
        self.set_flag(FLAG_ROLL, val);
    }

    #[must_use]
    pub fn is_eating(&self) -> bool {
        self.eat_counter.load(Ordering::Relaxed) > 0
    }

    fn set_eating(&self, eating: bool) {
        let value = i32::from(eating);
        self.eat_counter.store(value, Ordering::Relaxed);
        self.get_entity().set_synced_data(
            pumpkin_data::tracked_data::panda::EAT_COUNTER,
            VarInt(value),
        );
    }

    fn set_eat_counter(&self, value: i32) {
        self.eat_counter.store(value, Ordering::Relaxed);
        self.get_entity().set_synced_data(
            pumpkin_data::tracked_data::panda::EAT_COUNTER,
            VarInt(value),
        );
    }

    #[must_use]
    pub fn can_perform_action(&self) -> bool {
        !(self.is_on_back()
            || self.is_eating()
            || self.is_rolling()
            || self.is_sitting()
            || self.is_worried() && self.get_entity().world.load().is_thundering())
    }

    #[must_use]
    pub fn is_scared(&self) -> bool {
        self.is_worried() && self.get_entity().world.load().is_thundering()
    }

    fn held_item(&self) -> ItemStack {
        self.mob_entity
            .living_entity
            .entity_equipment
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .get(&pumpkin_data::data_component_impl::EquipmentSlot::MAIN_HAND)
    }

    fn set_held_item(&self, stack: ItemStack) {
        self.mob_entity.set_item_slot(
            &pumpkin_data::data_component_impl::EquipmentSlot::MAIN_HAND,
            stack,
        );
    }

    fn try_to_sit(&self) {
        if self.mob_entity.living_entity.is_in_water() {
            return;
        }
        self.mob_entity
            .navigator
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .stop();
        self.set_sitting(true);
    }

    /// Vanilla `Panda.handleEating`.
    fn handle_eating(&self) {
        let held = self.held_item();
        if !self.is_eating()
            && self.is_sitting()
            && !self.is_scared()
            && !held.is_empty()
            && rand::random_range(0..80) == 1
        {
            self.set_eating(true);
        } else if held.is_empty() || !self.is_sitting() {
            self.set_eating(false);
        }

        if !self.is_eating() {
            return;
        }

        if self.eat_counter.load(Ordering::Relaxed) % EAT_TICK_INTERVAL == 0 {
            let entity = self.get_entity();
            entity.world.load().play_sound_fine(
                Sound::EntityPandaEat,
                SoundCategory::Neutral,
                &entity.pos.load(),
                0.5 + 0.5 * rand::random_range(0..2) as f32,
                1.0 + (rand::random::<f32>() - rand::random::<f32>()) * 0.2,
            );
        }

        if self.eat_counter.load(Ordering::Relaxed) > 80 && rand::random_range(0..20) == 1 {
            if self.eat_counter.load(Ordering::Relaxed) > 100
                && self
                    .held_item()
                    .item
                    .has_tag(&tag::Item::MINECRAFT_PANDA_EATS_FROM_GROUND)
            {
                self.set_held_item(ItemStack::EMPTY.clone());
                let entity = self.get_entity();
                let world = entity.world.load_full();
                world.emit_game_event(pumpkin_data::game_event::GameEvent::Eat, entity.pos.load());
                self.set_sitting(false);
            }
            self.set_eating(false);
            return;
        }

        self.set_eat_counter(self.eat_counter.load(Ordering::Relaxed) + 1);
    }

    /// Vanilla `Panda.handleRoll`.
    fn handle_roll(&self) {
        let counter = self.roll_counter.fetch_add(1, Ordering::Relaxed) + 1;
        if counter > TOTAL_ROLL_STEPS {
            self.set_rolling(false);
            return;
        }

        let entity = self.get_entity();
        let movement = entity.velocity.load();
        if counter == 1 {
            let angle = f64::from(entity.yaw.load()).to_radians();
            let multiplier = if self.is_baby() { 0.1 } else { 0.2 };
            let delta = Vector3::new(
                movement.x - angle.sin() * multiplier,
                0.0,
                movement.z + angle.cos() * multiplier,
            );
            *self
                .roll_delta
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner) = Some(delta);
            entity
                .velocity
                .store(Vector3::new(delta.x, delta.y + 0.27, delta.z));
        } else {
            let delta = *self
                .roll_delta
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            let Some(delta) = delta else {
                return;
            };
            if counter != 7 && counter != 15 && counter != 23 {
                entity
                    .velocity
                    .store(Vector3::new(delta.x, movement.y, delta.z));
            } else {
                entity.velocity.store(Vector3::new(
                    0.0,
                    if entity.on_ground.load(Ordering::Relaxed) {
                        0.27
                    } else {
                        movement.y
                    },
                    0.0,
                ));
            }
        }
    }

    /// Vanilla `Panda.afterSneeze`.
    fn after_sneeze(&self) {
        let entity = self.get_entity();
        let world = entity.world.load();
        let pos = entity.pos.load();
        let body_yaw = f64::from(entity.head_yaw.load()).to_radians();
        let width = f64::from(entity.width());
        let sneeze_pos = Vector3::new(
            pos.x - f64::midpoint(width, 1.0) * body_yaw.sin(),
            pos.y + entity.get_eye_height() - 0.1,
            pos.z + f64::midpoint(width, 1.0) * body_yaw.cos(),
        );
        world.spawn_particle(
            sneeze_pos,
            Vector3::new(0.0, 0.0, 0.0),
            0.0,
            1,
            Particle::Sneeze,
        );
        world.play_sound(
            Sound::EntityPandaSneeze,
            SoundCategory::Neutral,
            &sneeze_pos,
        );

        let nearby = world.get_nearby_entities(pos, 10.0);
        for other in nearby.values() {
            let Some(panda) = other.cast_any().downcast_ref::<Self>() else {
                continue;
            };
            if !panda.is_baby()
                && panda.get_entity().on_ground.load(Ordering::Relaxed)
                && !panda.mob_entity.living_entity.is_in_water()
                && panda.can_perform_action()
            {
                panda
                    .mob_entity
                    .living_entity
                    .jumping
                    .store(true, Ordering::SeqCst);
            }
        }

        if world.level_info.load().game_rules.mob_drops
            && let Some(loot_table) = world.get_loot_table("minecraft:gameplay/panda_sneeze")
        {
            for stack in loot_table.generate_loot(rand::random::<i64>()) {
                entity.spawn_at_location(stack);
            }
        }
    }

    /// Vanilla `PandaBreedGoal.canFindBamboo`.
    fn can_find_bamboo(&self) -> bool {
        let entity = self.get_entity();
        let world = entity.world.load();
        let origin = entity.block_pos.load();
        for y_offset in 0..3 {
            for r in 0..8 {
                let mut x = 0;
                while x <= r {
                    let mut z = if x < r && x > -r { r } else { 0 };
                    while z <= r {
                        let pos =
                            BlockPos::new(origin.0.x + x, origin.0.y + y_offset, origin.0.z + z);
                        if world.get_block(&pos) == &pumpkin_data::Block::BAMBOO {
                            return true;
                        }
                        z = if z > 0 { -z } else { 1 - z };
                    }
                    x = if x > 0 { -x } else { 1 - x };
                }
            }
        }
        false
    }

    /// Vanilla `PandaBreedGoal.canUse`'s bamboo gate; also raises the unhappy counter.
    fn can_breed(&self) -> bool {
        if self.unhappy_counter.load(Ordering::Relaxed) != 0 {
            return false;
        }
        if self.can_find_bamboo() {
            return true;
        }

        let now = self.tick_count();
        if self.unhappy_cooldown.load(Ordering::Relaxed) <= now {
            self.set_unhappy_counter(TOTAL_UNHAPPY_TIME);
            self.unhappy_cooldown.store(now + 600, Ordering::Relaxed);
            let entity = self.get_entity();
            let world = entity.world.load();
            if let Some(player) = world.get_nearest_player(entity.pos.load(), 8.0, |player| {
                player.living_entity.is_part_of_game()
            }) {
                self.mob_entity
                    .look_control
                    .lock()
                    .unwrap_or_else(std::sync::PoisonError::into_inner)
                    .look_at_entity(self, &(player as Arc<dyn EntityBase>));
            }
        }
        false
    }

    fn set_unhappy_counter(&self, value: i32) {
        self.unhappy_counter.store(value, Ordering::Relaxed);
        self.get_entity().set_synced_data(
            pumpkin_data::tracked_data::panda::UNHAPPY_COUNTER,
            VarInt(value),
        );
    }

    fn tick_unhappy(&self) {
        let counter = self.unhappy_counter.load(Ordering::Relaxed);
        if counter > 0 {
            if let Some(target) = self.mob_entity.get_target() {
                let target_pos = target.get_entity().pos.load();
                self.mob_entity
                    .look_control
                    .lock()
                    .unwrap_or_else(std::sync::PoisonError::into_inner)
                    .look_at(
                        self,
                        target_pos.x,
                        target.get_entity().get_eye_y(),
                        target_pos.z,
                    );
            }
            if counter == TOTAL_UNHAPPY_TIME - 3 || counter == 14 {
                let entity = self.get_entity();
                entity.world.load().play_sound(
                    Sound::EntityPandaCantBreed,
                    SoundCategory::Neutral,
                    &entity.pos.load(),
                );
            }
            self.set_unhappy_counter(counter - 1);
        }
    }
}

impl AgeableMob for PandaEntity {
    fn get_ageable_data(&self) -> &AgeableData {
        &self.ageable_data
    }

    fn get_baby_start_age(&self) -> i32 {
        BABY_START_AGE
    }
}

impl Animal for PandaEntity {
    fn is_food(&self, item_stack: &ItemStack) -> bool {
        item_stack.item.has_tag(&tag::Item::MINECRAFT_PANDA_FOOD)
            || item_stack.item == &Item::BAMBOO
    }

    fn baby_spawned_from_breeding(&self, baby: &dyn EntityBase, partner: &dyn EntityBase) {
        let Some(baby) = baby.cast_any().downcast_ref::<Self>() else {
            return;
        };
        let Some(partner) = partner.cast_any().downcast_ref::<Self>() else {
            return;
        };
        baby.set_gene_from_parents(self, partner);
        baby.set_attributes();
    }
}

impl Mob for PandaEntity {
    fn as_ageable(&self) -> Option<&dyn AgeableMob> {
        Some(self)
    }

    fn finalize_spawn(
        &self,
        _world: &Arc<crate::world::World>,
        group_data: Option<spawn::SpawnGroupData>,
    ) -> Option<spawn::SpawnGroupData> {
        let group_data = group_data.unwrap_or(spawn::SpawnGroupData::Ageable(
            spawn::AgeableMobGroupData::with_baby_spawn_chance(0.2),
        ));
        spawn::finalize_ageable_spawn(self, Some(group_data))
    }

    fn as_animal(&self) -> Option<&dyn Animal> {
        Some(self)
    }

    fn mob_write_nbt(&self, nbt: &mut NbtCompound) {
        self.write_ageable_nbt(nbt);
        nbt.put_string("MainGene", self.get_main_gene().as_str().to_string());
        nbt.put_string("HiddenGene", self.get_hidden_gene().as_str().to_string());
    }

    fn mob_read_nbt(&self, nbt: &NbtCompound) {
        self.read_ageable_nbt(nbt);
        let main = nbt
            .get_string("MainGene")
            .map_or(PandaGene::Normal, PandaGene::from_name);
        let hidden = nbt
            .get_string("HiddenGene")
            .map_or(PandaGene::Normal, PandaGene::from_name);
        self.set_main_gene(main);
        self.set_hidden_gene(hidden);
        self.set_attributes();
    }

    fn get_mob_entity(&self) -> &MobEntity {
        &self.mob_entity
    }

    fn get_ambient_sound(&self) -> Option<Sound> {
        Some(if self.is_aggressive() {
            Sound::EntityPandaAggressiveAmbient
        } else if self.is_worried() {
            Sound::EntityPandaWorriedAmbient
        } else {
            Sound::EntityPandaAmbient
        })
    }

    fn mob_tick(&self, _caller: &dyn EntityBase) {
        if self.is_worried() {
            let entity = self.get_entity();
            let world = entity.world.load();
            if world.is_thundering() && !self.mob_entity.living_entity.is_in_water() {
                self.set_sitting(true);
                self.set_eating(false);
            } else if !self.is_eating() {
                self.set_sitting(false);
            }
        }

        if self.mob_entity.get_target().is_none() {
            self.got_bamboo.store(false, Ordering::Relaxed);
            self.did_bite.store(false, Ordering::Relaxed);
        }

        self.tick_unhappy();

        if self.is_sneezing() {
            let counter = self.sneeze_counter.load(Ordering::Relaxed) + 1;
            self.set_sneeze_counter(counter);
            if counter > 20 {
                self.set_sneezing(false);
                self.after_sneeze();
            } else if counter == 1 {
                let entity = self.get_entity();
                entity.world.load().play_sound(
                    Sound::EntityPandaPreSneeze,
                    SoundCategory::Neutral,
                    &entity.pos.load(),
                );
            }
        }

        if self.is_rolling() {
            self.handle_roll();
        } else {
            self.roll_counter.store(0, Ordering::Relaxed);
        }

        if self.is_sitting() {
            self.get_entity().pitch.store(0.0);
        }

        self.handle_eating();
        self.ageable_ai_step();
    }

    fn mob_init_data_tracker(&self) {
        let entity = self.get_entity();
        let is_baby = entity.age.load(Ordering::Relaxed) < 0;
        if is_baby {
            entity.set_synced_data(pumpkin_data::tracked_data::panda::DATA_BABY_ID, true);
        }
        entity.set_synced_data(
            pumpkin_data::tracked_data::panda::MAIN_GENE_ID,
            self.main_gene.load(Ordering::Relaxed) as i8,
        );
        entity.set_synced_data(
            pumpkin_data::tracked_data::panda::HIDDEN_GENE_ID,
            self.hidden_gene.load(Ordering::Relaxed) as i8,
        );
        entity.set_synced_data(
            pumpkin_data::tracked_data::panda::DATA_ID_FLAGS,
            self.flags.load(Ordering::Relaxed) as i8,
        );
        entity.set_synced_data(
            pumpkin_data::tracked_data::panda::UNHAPPY_COUNTER,
            VarInt(self.unhappy_counter.load(Ordering::Relaxed)),
        );
        entity.set_synced_data(
            pumpkin_data::tracked_data::panda::SNEEZE_COUNTER,
            VarInt(self.sneeze_counter.load(Ordering::Relaxed)),
        );
        entity.set_synced_data(
            pumpkin_data::tracked_data::panda::EAT_COUNTER,
            VarInt(self.eat_counter.load(Ordering::Relaxed)),
        );
    }

    fn mob_interact(&self, player: &Arc<Player>, item_stack: &mut ItemStack) -> bool {
        if self.is_scared() {
            return false;
        }
        if self.is_on_back() {
            self.set_on_back(false);
            return true;
        }
        if self.is_food(item_stack) {
            if self.mob_entity.get_target().is_some() {
                self.got_bamboo.store(true, Ordering::Relaxed);
            }

            let age = self.get_age();
            if self.can_age_up() {
                item_stack.decrement_unless_creative(player.gamemode.load(), 1);
                self.age_up(Self::get_speed_up_seconds_when_feeding(-age), true);
                return true;
            }

            if self.is_baby() {
                return self.mob_entity.mob_interact(player, item_stack);
            }

            if !self.is_baby() && !self.is_in_love() {
                item_stack.decrement_unless_creative(player.gamemode.load(), 1);
                self.mob_entity
                    .set_love_ticks(600, Some(player.gameprofile.id));
                let entity = self.get_entity();
                entity.world.load().send_entity_status(
                    entity,
                    EntityStatus::InLoveHearts,
                    Some(ActorEventID::InLoveHearts),
                );
                return true;
            }

            if self.is_sitting() || self.mob_entity.living_entity.is_in_water() {
                return false;
            }

            self.try_to_sit();
            self.set_eating(true);
            let current = self.held_item();
            if !current.is_empty() && player.gamemode.load() != pumpkin_util::GameMode::Creative {
                self.get_entity().spawn_at_location(current);
            }
            self.set_held_item(ItemStack::new(1, item_stack.get_item()));
            item_stack.decrement_unless_creative(player.gamemode.load(), 1);
            return true;
        }

        if item_stack.get_item() == &Item::LEAD && !self.get_entity().is_leashed() {
            return false;
        }

        self.mob_entity.mob_interact(player, item_stack)
    }

    fn on_damage(
        &self,
        _damage_type: pumpkin_data::damage::DamageType,
        _source: Option<&dyn EntityBase>,
    ) {
        self.set_sitting(false);
    }

    fn on_attack(&self, _target: &dyn EntityBase) {
        if !self.is_aggressive() {
            self.did_bite.store(true, Ordering::Relaxed);
        }
    }
}

fn panda_can_perform_action(mob: &dyn Mob) -> bool {
    mob.cast_any()
        .downcast_ref::<PandaEntity>()
        .is_some_and(PandaEntity::can_perform_action)
}

fn panda_can_avoid(mob: &dyn Mob) -> bool {
    mob.cast_any()
        .downcast_ref::<PandaEntity>()
        .is_some_and(|panda| panda.is_worried() && panda.can_perform_action())
}

fn panda_is_aggressive(mob: &dyn Mob) -> bool {
    mob.cast_any()
        .downcast_ref::<PandaEntity>()
        .is_some_and(PandaEntity::is_aggressive)
}

fn panda_has_not_bitten(mob: &dyn Mob) -> bool {
    mob.cast_any()
        .downcast_ref::<PandaEntity>()
        .is_some_and(|panda| {
            !panda.got_bamboo.load(Ordering::Relaxed) && !panda.did_bite.load(Ordering::Relaxed)
        })
}

/// Vanilla `Panda.PandaSitGoal`.
struct PandaSitGoal {
    panda: Weak<PandaEntity>,
    cooldown: i32,
}

impl PandaSitGoal {
    const fn new(panda: Weak<PandaEntity>) -> Self {
        Self { panda, cooldown: 0 }
    }
}

impl Goal for PandaSitGoal {
    fn can_start(&mut self, _mob: &dyn Mob) -> bool {
        let Some(panda) = self.panda.upgrade() else {
            return false;
        };
        let now = panda.tick_count();
        if self.cooldown > now
            || panda.is_baby()
            || panda.mob_entity.living_entity.is_in_water()
            || !panda.can_perform_action()
            || panda.unhappy_counter.load(Ordering::Relaxed) > 0
        {
            return false;
        }
        !panda.held_item().is_empty()
    }

    fn should_continue(&mut self, _mob: &dyn Mob) -> bool {
        let Some(panda) = self.panda.upgrade() else {
            return false;
        };
        if panda.mob_entity.living_entity.is_in_water() {
            return false;
        }
        if !panda.is_lazy() && rand::random_range(0..to_goal_ticks(600)) == 1 {
            return false;
        }
        rand::random_range(0..to_goal_ticks(2000)) != 1
    }

    fn start(&mut self, _mob: &dyn Mob) {
        self.cooldown = 0;
    }

    fn tick(&mut self, _mob: &dyn Mob) {
        let Some(panda) = self.panda.upgrade() else {
            return;
        };
        if !panda.is_sitting() && !panda.held_item().is_empty() {
            panda.try_to_sit();
        }
    }

    fn stop(&mut self, _mob: &dyn Mob) {
        let Some(panda) = self.panda.upgrade() else {
            return;
        };
        let held = panda.held_item();
        if !held.is_empty() {
            panda.get_entity().spawn_at_location(held);
            panda.set_held_item(ItemStack::EMPTY.clone());
            let wait = if panda.is_lazy() {
                rand::random_range(10..60)
            } else {
                rand::random_range(10..160)
            };
            self.cooldown = panda.tick_count() + wait * 20;
        }
        panda.set_sitting(false);
    }

    fn controls(&self) -> Controls {
        Controls::MOVE
    }
}

/// Vanilla `Panda.PandaLieOnBackGoal`.
struct PandaLieOnBackGoal {
    panda: Weak<PandaEntity>,
    cooldown: i32,
}

impl PandaLieOnBackGoal {
    const fn new(panda: Weak<PandaEntity>) -> Self {
        Self { panda, cooldown: 0 }
    }
}

impl Goal for PandaLieOnBackGoal {
    fn can_start(&mut self, _mob: &dyn Mob) -> bool {
        let Some(panda) = self.panda.upgrade() else {
            return false;
        };
        self.cooldown < panda.tick_count()
            && panda.is_lazy()
            && panda.can_perform_action()
            && rand::random_range(0..to_goal_ticks(400)) == 1
    }

    fn should_continue(&mut self, _mob: &dyn Mob) -> bool {
        let Some(panda) = self.panda.upgrade() else {
            return false;
        };
        if panda.mob_entity.living_entity.is_in_water() {
            return false;
        }
        if !panda.is_lazy() && rand::random_range(0..to_goal_ticks(600)) == 1 {
            return false;
        }
        rand::random_range(0..to_goal_ticks(2000)) != 1
    }

    fn start(&mut self, _mob: &dyn Mob) {
        if let Some(panda) = self.panda.upgrade() {
            panda.set_on_back(true);
        }
        self.cooldown = 0;
    }

    fn stop(&mut self, _mob: &dyn Mob) {
        if let Some(panda) = self.panda.upgrade() {
            panda.set_on_back(false);
            self.cooldown = panda.tick_count() + 200;
        }
    }
}

/// Vanilla `Panda.PandaSneezeGoal`.
struct PandaSneezeGoal {
    panda: Weak<PandaEntity>,
}

impl PandaSneezeGoal {
    const fn new(panda: Weak<PandaEntity>) -> Self {
        Self { panda }
    }
}

impl Goal for PandaSneezeGoal {
    fn can_start(&mut self, _mob: &dyn Mob) -> bool {
        let Some(panda) = self.panda.upgrade() else {
            return false;
        };
        panda.is_baby()
            && panda.can_perform_action()
            && ((panda.is_weak() && rand::random_range(0..to_goal_ticks(500)) == 1)
                || rand::random_range(0..to_goal_ticks(6000)) == 1)
    }

    fn should_continue(&mut self, _mob: &dyn Mob) -> bool {
        false
    }

    fn start(&mut self, _mob: &dyn Mob) {
        if let Some(panda) = self.panda.upgrade() {
            panda.set_sneezing(true);
        }
    }
}

/// Vanilla `Panda.PandaRollGoal`.
struct PandaRollGoal {
    panda: Weak<PandaEntity>,
}

impl PandaRollGoal {
    const fn new(panda: Weak<PandaEntity>) -> Self {
        Self { panda }
    }
}

impl Goal for PandaRollGoal {
    fn can_start(&mut self, _mob: &dyn Mob) -> bool {
        let Some(panda) = self.panda.upgrade() else {
            return false;
        };
        let entity = panda.get_entity();
        if !(panda.is_baby() || panda.is_playful())
            || !entity.on_ground.load(Ordering::Relaxed)
            || !panda.can_perform_action()
        {
            return false;
        }

        let angle = f64::from(entity.yaw.load()).to_radians();
        let x_dir = -angle.sin();
        let z_dir = angle.cos();
        let x_step = if x_dir.abs() > 0.5 {
            x_dir.signum() as i32
        } else {
            0
        };
        let z_step = if z_dir.abs() > 0.5 {
            z_dir.signum() as i32
        } else {
            0
        };
        let block_pos = entity.block_pos.load();
        let world = entity.world.load();
        if world
            .get_block(&BlockPos::new(
                block_pos.0.x + x_step,
                block_pos.0.y - 1,
                block_pos.0.z + z_step,
            ))
            .is_air()
        {
            return true;
        }

        (panda.is_playful() && rand::random_range(0..to_goal_ticks(60)) == 1)
            || rand::random_range(0..to_goal_ticks(500)) == 1
    }

    fn should_continue(&mut self, _mob: &dyn Mob) -> bool {
        false
    }

    fn start(&mut self, _mob: &dyn Mob) {
        if let Some(panda) = self.panda.upgrade() {
            panda.set_rolling(true);
        }
    }

    fn controls(&self) -> Controls {
        Controls::MOVE | Controls::LOOK | Controls::JUMP
    }
}
