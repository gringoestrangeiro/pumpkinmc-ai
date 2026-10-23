use std::sync::{
    Arc, Weak,
    atomic::{AtomicBool, AtomicI32, Ordering},
};

use pumpkin_data::attributes::Attributes;
use pumpkin_data::block_properties::WheatLikeProperties;
use pumpkin_data::entity::{EntityStatus, EntityType};
use pumpkin_data::game_event::GameEvent;
use pumpkin_data::item::Item;
use pumpkin_data::item_stack::ItemStack;
use pumpkin_data::sound::{Sound, SoundCategory};
use pumpkin_data::tag::{self, Taggable};
use pumpkin_data::translation;
use pumpkin_data::world::WorldEvent;
use pumpkin_data::{Block, tag::WorldgenBiome};
use pumpkin_nbt::compound::NbtCompound;
use pumpkin_protocol::codec::var_int::VarInt;
use pumpkin_util::math::position::BlockPos;
use pumpkin_util::math::vector3::Vector3;
use pumpkin_util::text::TextComponent;
use pumpkin_world::world::BlockFlags;
use rand::RngExt;

use crate::entity::{
    Entity, EntityBase,
    ageable::{AgeableData, AgeableMob},
    ai::goal::{
        Controls, Goal, ParentHandle,
        active_target::ActiveTargetGoal,
        avoid_entity::AvoidEntityGoal,
        breed::BreedGoal,
        climb_on_top_of_powder_snow::ClimbOnTopOfPowderSnowGoal,
        escape_danger::EscapeDangerGoal,
        look_at_entity::LookAtEntityGoal,
        melee_attack::MeleeAttackGoal,
        move_to_target_pos::{MoveToTargetPos, MoveToTargetPosGoal},
        revenge::RevengeGoal,
        swim::SwimGoal,
        tempt::TemptGoal,
        water_avoiding_random_stroll::WaterAvoidingRandomStrollGoal,
    },
    attributes::{Modifier, ModifierOperation},
    mob::{
        Mob, MobEntity,
        spawn::{AgeableMobGroupData, SpawnGroupData, finalize_ageable_spawn},
    },
    passive::animal::Animal,
    player::Player,
};
use crate::world::World;

const TEMPT_ITEMS: &[&Item] = &[&Item::CARROT, &Item::GOLDEN_CARROT, &Item::DANDELION];

const JUMP_DELAY_TICKS: i32 = 10;
const PANIC_JUMP_DELAY_TICKS: i32 = 3;
const JUMP_DURATION_IN_TICKS: i32 = 15;
const MORE_CARROTS_DELAY: i32 = 40;
const EVIL_ATTACK_POWER_MODIFIER: &str = "minecraft:evil";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[repr(i32)]
pub enum RabbitVariant {
    #[default]
    Brown = 0,
    White = 1,
    Black = 2,
    WhiteSplotched = 3,
    Gold = 4,
    Salt = 5,
    Evil = 99,
}

impl RabbitVariant {
    #[must_use]
    pub const fn from_id(id: i32) -> Self {
        match id {
            1 => Self::White,
            2 => Self::Black,
            3 => Self::WhiteSplotched,
            4 => Self::Gold,
            5 => Self::Salt,
            99 => Self::Evil,
            _ => Self::Brown,
        }
    }

    #[must_use]
    pub const fn id(self) -> i32 {
        self as i32
    }

    #[must_use]
    pub fn random_variant() -> Self {
        let mut rng = rand::rng();
        match rng.random_range(0..6) {
            1 => Self::White,
            2 => Self::Black,
            3 => Self::WhiteSplotched,
            4 => Self::Gold,
            5 => Self::Salt,
            _ => Self::Brown,
        }
    }
}

/// Vanilla `Rabbit.getRandomRabbitVariant`.
fn variant_for_biome(biome: &pumpkin_data::biome::Biome) -> RabbitVariant {
    let roll = rand::rng().random_range(0..100);
    if biome.has_tag(&WorldgenBiome::MINECRAFT_SPAWNS_WHITE_RABBITS) {
        if roll < 80 {
            RabbitVariant::White
        } else {
            RabbitVariant::WhiteSplotched
        }
    } else if biome.has_tag(&WorldgenBiome::MINECRAFT_SPAWNS_GOLD_RABBITS) {
        RabbitVariant::Gold
    } else if roll < 50 {
        RabbitVariant::Brown
    } else if roll < 90 {
        RabbitVariant::Salt
    } else {
        RabbitVariant::Black
    }
}

pub struct RabbitEntity {
    pub mob_entity: MobEntity,
    pub ageable_data: AgeableData,
    pub variant: AtomicI32,
    pub more_carrot_ticks: AtomicI32,
    jump_ticks: AtomicI32,
    jump_duration: AtomicI32,
    was_on_ground: AtomicBool,
    jump_delay_ticks: AtomicI32,
}

impl RabbitEntity {
    pub fn new(entity: Entity) -> Arc<Self> {
        let mob_entity = MobEntity::new(entity);
        let variant = RabbitVariant::random_variant();
        let rabbit = Self {
            mob_entity,
            ageable_data: AgeableData::default(),
            variant: AtomicI32::new(variant.id()),
            more_carrot_ticks: AtomicI32::new(0),
            jump_ticks: AtomicI32::new(0),
            jump_duration: AtomicI32::new(0),
            was_on_ground: AtomicBool::new(false),
            jump_delay_ticks: AtomicI32::new(0),
        };
        let mob_arc = Arc::new(rabbit);
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

            goal_selector.add_goal(1, Box::new(SwimGoal::default()));
            goal_selector.add_goal(1, Box::new(ClimbOnTopOfPowderSnowGoal));
            goal_selector.add_goal(1, EscapeDangerGoal::new(2.2));
            goal_selector.add_goal(2, BreedGoal::new(0.8));
            goal_selector.add_goal(3, Box::new(TemptGoal::new(1.0, TEMPT_ITEMS, false)));
            goal_selector.add_goal(
                4,
                Box::new(AvoidEntityGoal::with_gate(
                    &EntityType::PLAYER,
                    8.0,
                    2.2,
                    2.2,
                    rabbit_is_not_evil,
                )),
            );
            goal_selector.add_goal(
                4,
                Box::new(AvoidEntityGoal::with_gate(
                    &EntityType::WOLF,
                    10.0,
                    2.2,
                    2.2,
                    rabbit_is_not_evil,
                )),
            );
            goal_selector.add_goal(5, RaidGardenGoal::new(&mob_arc));
            goal_selector.add_goal(6, Box::new(WaterAvoidingRandomStrollGoal::new(0.6)));
            goal_selector.add_goal(
                11,
                LookAtEntityGoal::with_default(mob_weak, &EntityType::PLAYER, 10.0),
            );
        };

        mob_arc
    }

    #[must_use]
    pub fn get_variant(&self) -> RabbitVariant {
        RabbitVariant::from_id(self.variant.load(Ordering::Relaxed))
    }

    pub fn set_variant(&self, variant: RabbitVariant) {
        self.variant.store(variant.id(), Ordering::Relaxed);
        let entity = self.get_entity();
        entity.set_synced_data(
            pumpkin_data::tracked_data::rabbit::DATA_TYPE_ID,
            VarInt(variant.id()),
        );

        let living = &self.mob_entity.living_entity;
        {
            let mut attributes = living
                .attributes
                .write()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            if variant == RabbitVariant::Evil {
                if let Some(armor) = attributes.get_mut(&Attributes::ARMOR.id) {
                    armor.base_value = 8.0;
                    armor.dirty.store(true, Ordering::Relaxed);
                }
                if let Some(damage) = attributes.get_mut(&Attributes::ATTACK_DAMAGE.id) {
                    damage.add_or_replace_modifier(Modifier {
                        id: EVIL_ATTACK_POWER_MODIFIER.to_string(),
                        amount: 5.0,
                        operation: ModifierOperation::Add,
                    });
                }
            } else if let Some(damage) = attributes.get_mut(&Attributes::ATTACK_DAMAGE.id) {
                damage.remove_modifier(EVIL_ATTACK_POWER_MODIFIER);
            }
        }

        if variant == RabbitVariant::Evil {
            {
                let mut goal_selector = self
                    .mob_entity
                    .goals_selector
                    .lock()
                    .unwrap_or_else(std::sync::PoisonError::into_inner);
                goal_selector.add_goal(4, Box::new(MeleeAttackGoal::new(1.4, true)));

                let mut target_selector = self
                    .mob_entity
                    .target_selector
                    .lock()
                    .unwrap_or_else(std::sync::PoisonError::into_inner);
                target_selector.add_goal(1, Box::new(RevengeGoal::new(true).alerting_others()));
                target_selector.add_goal(
                    2,
                    ActiveTargetGoal::with_default(&self.mob_entity, &EntityType::PLAYER, true),
                );
                target_selector.add_goal(
                    2,
                    ActiveTargetGoal::with_default(&self.mob_entity, &EntityType::WOLF, true),
                );
            };

            if (**entity.custom_name.load()).is_none() {
                entity.set_custom_name(TextComponent::translate(
                    translation::java::ENTITY_MINECRAFT_KILLER_BUNNY,
                    [],
                ));
            }
        }
    }

    fn wants_more_food(&self) -> bool {
        self.more_carrot_ticks.load(Ordering::Relaxed) <= 0
    }

    fn set_landing_delay(&self) {
        let speed = self
            .mob_entity
            .navigator
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .get_speed();
        self.jump_delay_ticks.store(
            if speed < 2.2 {
                JUMP_DELAY_TICKS
            } else {
                PANIC_JUMP_DELAY_TICKS
            },
            Ordering::Relaxed,
        );
    }

    fn start_jumping(&self) {
        let entity = self.get_entity();
        self.mob_entity
            .living_entity
            .jumping
            .store(true, Ordering::SeqCst);
        self.jump_duration
            .store(JUMP_DURATION_IN_TICKS, Ordering::Relaxed);
        self.jump_ticks.store(0, Ordering::Relaxed);

        let mut rng = rand::rng();
        let pitch = ((rng.random::<f32>() - rng.random::<f32>()) * 0.2 + 1.0) * 0.8;
        entity.world.load().play_sound_fine(
            Sound::EntityRabbitJump,
            SoundCategory::Neutral,
            &entity.pos.load(),
            1.0,
            pitch,
        );

        let velocity = entity.velocity.load();
        if velocity.x * velocity.x + velocity.z * velocity.z < 0.01 {
            let yaw = f64::from(entity.yaw.load()).to_radians();
            let height = if self.is_baby() { 0.5 } else { 1.5 };
            entity
                .velocity
                .store(velocity + Vector3::new(-yaw.sin() * 0.1, height * 0.1, yaw.cos() * 0.1));
        }

        entity
            .world
            .load()
            .send_entity_status(entity, EntityStatus::Jump, None);
    }

    fn tick_jump_cycle(&self) {
        let ticks = self.jump_ticks.load(Ordering::Relaxed);
        let duration = self.jump_duration.load(Ordering::Relaxed);
        if ticks != duration {
            self.jump_ticks.store(ticks + 1, Ordering::Relaxed);
        } else if duration != 0 {
            self.jump_ticks.store(0, Ordering::Relaxed);
            self.jump_duration.store(0, Ordering::Relaxed);
            self.mob_entity
                .living_entity
                .jumping
                .store(false, Ordering::SeqCst);
        }
    }

    fn custom_server_ai_step(&self) {
        let entity = self.get_entity();

        if self.jump_delay_ticks.load(Ordering::Relaxed) > 0 {
            self.jump_delay_ticks.fetch_sub(1, Ordering::Relaxed);
        }

        let more_carrots = self.more_carrot_ticks.load(Ordering::Relaxed);
        if more_carrots > 0 {
            self.more_carrot_ticks.store(
                (more_carrots - rand::rng().random_range(0..3)).max(0),
                Ordering::Relaxed,
            );
        }

        if !entity.on_ground.load(Ordering::Relaxed) {
            self.was_on_ground.store(false, Ordering::Relaxed);
            return;
        }

        if !self.was_on_ground.swap(true, Ordering::Relaxed) {
            self.mob_entity
                .living_entity
                .jumping
                .store(false, Ordering::SeqCst);
            self.set_landing_delay();
        }

        if self.jump_delay_ticks.load(Ordering::Relaxed) != 0 {
            return;
        }

        if self.get_variant() == RabbitVariant::Evil
            && let Some(target) = self.mob_entity.get_target()
        {
            let target_pos = target.get_entity().pos.load();
            if entity.pos.load().squared_distance_to_vec(&target_pos) < 16.0 {
                entity.yaw.store(
                    (target_pos.z - entity.pos.load().z)
                        .atan2(target_pos.x - entity.pos.load().x)
                        .to_degrees() as f32
                        - 90.0,
                );
                self.start_jumping();
                return;
            }
        }

        if !self
            .mob_entity
            .navigator
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .is_idle()
        {
            self.start_jumping();
        }
    }
}

fn rabbit_is_not_evil(mob: &dyn Mob) -> bool {
    mob.cast_any()
        .downcast_ref::<RabbitEntity>()
        .is_none_or(|rabbit| rabbit.get_variant() != RabbitVariant::Evil)
}

/// Vanilla `Rabbit.RaidGardenGoal`.
struct RaidGardenGoal {
    inner: MoveToTargetPosGoal<Self>,
    rabbit: Weak<RabbitEntity>,
    wants_to_raid: AtomicBool,
    can_raid: AtomicBool,
}

impl RaidGardenGoal {
    #[expect(
        clippy::unnecessary_box_returns,
        reason = "GoalSelector takes boxed goals"
    )]
    fn new(rabbit: &Arc<RabbitEntity>) -> Box<Self> {
        let mut this = Box::new(Self {
            inner: MoveToTargetPosGoal::with_default(ParentHandle::none(), 0.7, 16),
            rabbit: Arc::downgrade(rabbit),
            wants_to_raid: AtomicBool::new(false),
            can_raid: AtomicBool::new(false),
        });
        // SAFETY: `this` is heap allocated in a `Box` and outlives the handle.
        this.inner.move_to_target_pos = unsafe { ParentHandle::new(&*this) };
        this
    }
}

impl Goal for RaidGardenGoal {
    fn can_start(&mut self, mob: &dyn Mob) -> bool {
        if self.inner.cooldown <= 0 {
            if !mob
                .get_entity()
                .world
                .load()
                .level_info
                .load()
                .game_rules
                .mob_griefing
            {
                return false;
            }
            self.can_raid.store(false, Ordering::Relaxed);
            self.wants_to_raid.store(
                self.rabbit
                    .upgrade()
                    .is_some_and(|rabbit| rabbit.wants_more_food()),
                Ordering::Relaxed,
            );
        }

        self.inner.can_start(mob)
    }

    fn should_continue(&mut self, mob: &dyn Mob) -> bool {
        self.can_raid.load(Ordering::Relaxed) && self.inner.should_continue(mob)
    }

    fn start(&mut self, mob: &dyn Mob) {
        self.inner.start(mob);
    }

    fn stop(&mut self, mob: &dyn Mob) {
        self.inner.stop(mob);
    }

    fn tick(&mut self, mob: &dyn Mob) {
        self.inner.tick(mob);

        let target = self.inner.get_target_pos();
        mob.get_mob_entity()
            .look_control
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .look_at(
                mob,
                f64::from(target.0.x) + 0.5,
                f64::from(target.0.y),
                f64::from(target.0.z) + 0.5,
            );

        if !self.inner.is_reached_target() {
            return;
        }

        let crops_pos = target;
        if self.can_raid.load(Ordering::Relaxed) {
            let entity = mob.get_entity();
            let world = entity.world.load_full();
            let (block, state_id) = world.get_block_and_state_id(&crops_pos);
            if block.id == Block::CARROTS.id {
                let mut props = WheatLikeProperties::from_state_id(state_id);
                if props.age == 0 {
                    world.set_block_state(
                        &crops_pos,
                        Block::AIR.default_state.id,
                        BlockFlags::NOTIFY_ALL,
                    );
                } else {
                    props.age -= 1;
                    world.set_block_state(
                        &crops_pos,
                        props.to_state_id(block),
                        BlockFlags::NOTIFY_ALL,
                    );
                    world.emit_game_event(GameEvent::BlockChange, crops_pos.to_centered_f64());
                    world.sync_world_event(
                        WorldEvent::ParticlesDestroyBlock,
                        crops_pos,
                        i32::from(state_id.to_state().id.as_u16()),
                    );
                }

                if let Some(rabbit) = self.rabbit.upgrade() {
                    rabbit
                        .more_carrot_ticks
                        .store(MORE_CARROTS_DELAY, Ordering::Relaxed);
                }
            }
        }

        self.can_raid.store(false, Ordering::Relaxed);
        self.inner.cooldown = 10;
    }

    fn should_run_every_tick(&self) -> bool {
        true
    }

    fn controls(&self) -> Controls {
        self.inner.controls()
    }
}

impl MoveToTargetPos for RaidGardenGoal {
    fn is_target_pos(&self, world: Arc<World>, block_pos: BlockPos) -> bool {
        if !self.wants_to_raid.load(Ordering::Relaxed) {
            return false;
        }
        let below = world.get_block(&block_pos);
        if !below.has_tag(&tag::Block::MINECRAFT_SUPPORTS_CROPS) {
            return false;
        }
        let (crop, state_id) = world.get_block_and_state_id(&block_pos.up());
        if crop.id != Block::CARROTS.id {
            return false;
        }
        if WheatLikeProperties::from_state_id(state_id).age != 7 {
            return false;
        }
        self.can_raid.store(true, Ordering::Relaxed);
        true
    }
}

impl AgeableMob for RabbitEntity {
    fn get_ageable_data(&self) -> &AgeableData {
        &self.ageable_data
    }
}

impl Animal for RabbitEntity {
    fn is_food(&self, item_stack: &ItemStack) -> bool {
        item_stack.item.has_tag(&tag::Item::MINECRAFT_RABBIT_FOOD)
    }
}

impl Mob for RabbitEntity {
    fn as_ageable(&self) -> Option<&dyn AgeableMob> {
        Some(self)
    }

    fn as_animal(&self) -> Option<&dyn Animal> {
        Some(self)
    }

    fn mob_write_nbt(&self, nbt: &mut NbtCompound) {
        self.write_ageable_nbt(nbt);
        nbt.put_int("RabbitType", self.get_variant().id());
        nbt.put_int(
            "MoreCarrotTicks",
            self.more_carrot_ticks.load(Ordering::Relaxed),
        );
    }

    fn mob_read_nbt(&self, nbt: &NbtCompound) {
        self.read_ageable_nbt(nbt);
        if let Some(rabbit_type) = nbt.get_int("RabbitType") {
            self.set_variant(RabbitVariant::from_id(rabbit_type));
        }
        if let Some(ticks) = nbt.get_int("MoreCarrotTicks") {
            self.more_carrot_ticks.store(ticks, Ordering::Relaxed);
        }
    }

    fn get_mob_entity(&self) -> &MobEntity {
        &self.mob_entity
    }

    fn mob_tick(&self, _caller: &dyn EntityBase) {
        self.tick_jump_cycle();
        self.ageable_ai_step();
    }

    fn custom_server_ai_step(&self, _caller: &dyn EntityBase) {
        Self::custom_server_ai_step(self);
    }

    fn finalize_spawn(
        &self,
        world: &Arc<World>,
        group_data: Option<SpawnGroupData>,
    ) -> Option<SpawnGroupData> {
        let (rabbit_data, variant) =
            if let Some(SpawnGroupData::Rabbit(rabbit_data, variant)) = group_data {
                (rabbit_data, variant)
            } else {
                let biome = world.get_biome(&self.get_entity().block_pos.load());
                (
                    AgeableMobGroupData::with_baby_spawn_chance(1.0),
                    variant_for_biome(biome),
                )
            };
        self.set_variant(variant);
        finalize_ageable_spawn(self, Some(SpawnGroupData::Rabbit(rabbit_data, variant)))
    }

    fn mob_init_data_tracker(&self) {
        let entity = self.get_entity();
        let is_baby = entity.age.load(Ordering::Relaxed) < 0;
        if is_baby {
            entity.set_synced_data(pumpkin_data::tracked_data::rabbit::DATA_BABY_ID, true);
        }
        entity.set_synced_data(
            pumpkin_data::tracked_data::rabbit::DATA_TYPE_ID,
            VarInt(self.get_variant().id()),
        );
    }

    fn get_ambient_sound(&self) -> Option<Sound> {
        Some(Sound::EntityRabbitAmbient)
    }

    fn mob_interact(&self, player: &Arc<Player>, item_stack: &mut ItemStack) -> bool {
        self.animal_interact(player, item_stack, Sound::EntityRabbitAmbient)
    }
}
