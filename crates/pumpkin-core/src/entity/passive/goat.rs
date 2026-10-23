use std::sync::{
    Arc, Weak,
    atomic::{AtomicBool, AtomicI32, Ordering},
};

use pumpkin_data::attributes::Attributes;
use pumpkin_data::effect::StatusEffect;
use pumpkin_data::entity::{EntityStatus, EntityType};
use pumpkin_data::item::Item;
use pumpkin_data::item_stack::ItemStack;
use pumpkin_data::sound::{Sound, SoundCategory};
use pumpkin_data::tag::{self, Taggable};
use pumpkin_nbt::compound::NbtCompound;
use pumpkin_util::math::position::BlockPos;
use pumpkin_util::math::vector3::Vector3;
use rand::RngExt;

use crate::entity::{
    Entity, EntityBase,
    ageable::{AgeableData, AgeableMob},
    ai::goal::{
        Controls, Goal,
        breed::BreedGoal,
        escape_danger::EscapeDangerGoal,
        follow_parent::FollowParentGoal,
        long_jump::{LongJumpGoal, default_acceptable_landing},
        look_around::RandomLookAroundGoal,
        look_at_entity::LookAtEntityGoal,
        swim::SwimGoal,
        tempt::TemptGoal,
        wander_around::WanderAroundGoal,
    },
    combat::knockback_after_resistance,
    custom_sound::CustomSound,
    mob::{Mob, MobEntity, spawn::SpawnGroupData},
    passive::animal::Animal,
    player::Player,
};
use crate::world::World;

const TEMPT_ITEMS: &[&Item] = &[&Item::WHEAT];

const RAM_PREPARE_TIME: i32 = 20;
const RAM_MIN_DISTANCE: f64 = 4.0;
const RAM_MAX_DISTANCE: f64 = 7.0;
const RAM_CHARGE_TICKS: i32 = 200;
const TIME_BETWEEN_RAMS_MIN: i32 = 600;
const TIME_BETWEEN_RAMS_MAX: i32 = 6000;
const TIME_BETWEEN_RAMS_SCREAMER_MIN: i32 = 100;
const TIME_BETWEEN_RAMS_SCREAMER_MAX: i32 = 300;
const ADULT_RAM_KNOCKBACK_FORCE: f64 = 2.5;
const BABY_RAM_KNOCKBACK_FORCE: f64 = 1.0;
const RAM_SPEED: f64 = 3.0;

pub struct GoatEntity {
    pub mob_entity: MobEntity,
    pub ageable_data: AgeableData,
    pub is_screaming: AtomicBool,
    pub has_left_horn: AtomicBool,
    pub has_right_horn: AtomicBool,
    ram_cooldown: AtomicI32,
}

impl GoatEntity {
    pub fn new(entity: Entity) -> Arc<Self> {
        let mob_entity = MobEntity::new(entity);
        let goat = Self {
            mob_entity,
            ageable_data: AgeableData::default(),
            is_screaming: AtomicBool::new(false),
            has_left_horn: AtomicBool::new(true),
            has_right_horn: AtomicBool::new(true),
            ram_cooldown: AtomicI32::new(
                rand::rng().random_range(TIME_BETWEEN_RAMS_MIN..=TIME_BETWEEN_RAMS_MAX),
            ),
        };
        let mob_arc = Arc::new(goat);
        let mob_weak: Weak<dyn Mob> = {
            let mob_arc: Arc<dyn Mob> = mob_arc.clone();
            Arc::downgrade(&mob_arc)
        };

        mob_arc
            .mob_entity
            .navigator
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .set_can_float(true);

        {
            let mut goal_selector = mob_arc
                .mob_entity
                .goals_selector
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);

            goal_selector.add_goal(0, Box::new(SwimGoal::default()));
            goal_selector.add_goal(1, EscapeDangerGoal::new(2.0));
            goal_selector.add_goal(2, BreedGoal::new(1.0));
            goal_selector.add_goal(3, Box::new(TemptGoal::new(1.25, TEMPT_ITEMS, false)));
            goal_selector.add_goal(4, Box::new(FollowParentGoal::new(1.25)));
            goal_selector.add_goal(5, RamGoal::new(&mob_arc));
            goal_selector.add_goal(
                5,
                Box::new(LongJumpGoal::new(
                    5,
                    5,
                    3.571_428_8,
                    600,
                    1200,
                    goat_long_jump_sound,
                    Sound::EntityGoatStep,
                    default_acceptable_landing,
                )),
            );
            goal_selector.add_goal(6, Box::new(WanderAroundGoal::new(1.0)));
            goal_selector.add_goal(
                7,
                LookAtEntityGoal::with_default(mob_weak, &EntityType::PLAYER, 6.0),
            );
            goal_selector.add_goal(8, Box::new(RandomLookAroundGoal::default()));
        };

        mob_arc
    }

    #[must_use]
    pub fn is_screaming(&self) -> bool {
        self.is_screaming.load(Ordering::Relaxed)
    }

    pub fn set_screaming(&self, screaming: bool) {
        self.is_screaming.store(screaming, Ordering::Relaxed);
        let entity = self.get_entity();
        entity.set_synced_data(
            pumpkin_data::tracked_data::goat::DATA_IS_SCREAMING_GOAT,
            screaming,
        );
    }

    #[must_use]
    pub fn has_left_horn(&self) -> bool {
        self.has_left_horn.load(Ordering::Relaxed)
    }

    pub fn set_has_left_horn(&self, has_horn: bool) {
        self.has_left_horn.store(has_horn, Ordering::Relaxed);
        let entity = self.get_entity();
        entity.set_synced_data(
            pumpkin_data::tracked_data::goat::DATA_HAS_LEFT_HORN,
            has_horn,
        );
    }

    #[must_use]
    pub fn has_right_horn(&self) -> bool {
        self.has_right_horn.load(Ordering::Relaxed)
    }

    pub fn set_has_right_horn(&self, has_horn: bool) {
        self.has_right_horn.store(has_horn, Ordering::Relaxed);
        let entity = self.get_entity();
        entity.set_synced_data(
            pumpkin_data::tracked_data::goat::DATA_HAS_RIGHT_HORN,
            has_horn,
        );
    }

    fn reset_ram_cooldown(&self) {
        let range = if self.is_screaming() {
            TIME_BETWEEN_RAMS_SCREAMER_MIN..=TIME_BETWEEN_RAMS_SCREAMER_MAX
        } else {
            TIME_BETWEEN_RAMS_MIN..=TIME_BETWEEN_RAMS_MAX
        };
        self.ram_cooldown
            .store(rand::rng().random_range(range), Ordering::Relaxed);
    }

    fn ram_knockback_force(&self) -> f64 {
        if self.is_baby() {
            BABY_RAM_KNOCKBACK_FORCE
        } else {
            ADULT_RAM_KNOCKBACK_FORCE
        }
    }

    fn impact_sound(&self) -> Sound {
        if self.is_screaming() {
            Sound::EntityGoatScreamingRamImpact
        } else {
            Sound::EntityGoatRamImpact
        }
    }

    fn prepare_ram_sound(&self) -> Sound {
        if self.is_screaming() {
            Sound::EntityGoatScreamingPrepareRam
        } else {
            Sound::EntityGoatPrepareRam
        }
    }

    fn eat_sound(&self) -> Sound {
        if self.is_screaming() {
            Sound::EntityGoatScreamingEat
        } else {
            Sound::EntityGoatEat
        }
    }
}

fn goat_long_jump_sound(mob: &dyn Mob) -> Sound {
    if mob
        .cast_any()
        .downcast_ref::<GoatEntity>()
        .is_some_and(GoatEntity::is_screaming)
    {
        Sound::EntityGoatScreamingLongJump
    } else {
        Sound::EntityGoatLongJump
    }
}

/// Vanilla `GoatAi.RamTarget` and `PrepareRamNearestTarget`.
struct RamGoal {
    goat: Weak<GoatEntity>,
    target: Option<Arc<dyn EntityBase>>,
    target_block: BlockPos,
    preparing: bool,
    prepare_ticks: i32,
    charge_ticks: i32,
    ram_direction: Vector3<f64>,
    search_cooldown: i32,
}

impl RamGoal {
    #[expect(
        clippy::unnecessary_box_returns,
        reason = "GoalSelector takes boxed goals"
    )]
    fn new(goat: &Arc<GoatEntity>) -> Box<Self> {
        Box::new(Self {
            goat: Arc::downgrade(goat),
            target: None,
            target_block: BlockPos::ZERO,
            preparing: false,
            prepare_ticks: 0,
            charge_ticks: 0,
            ram_direction: Vector3::default(),
            search_cooldown: 0,
        })
    }

    fn find_target(mob: &dyn Mob) -> Option<Arc<dyn EntityBase>> {
        let entity = mob.get_entity();
        let world = entity.world.load();
        let mob_griefing = world.level_info.load().game_rules.mob_griefing;
        let search_box = entity.bounding_box.load().expand(7.0, 3.0, 7.0);
        let mobile_pos = entity.pos.load();

        world
            .get_entities_and_players_at_box(&search_box)
            .into_iter()
            .filter(|candidate| {
                let candidate_entity = candidate.get_entity();
                candidate_entity.entity_id != entity.entity_id
                    && candidate_entity.entity_type != &EntityType::GOAT
                    && (mob_griefing || candidate_entity.entity_type != &EntityType::ARMOR_STAND)
                    && candidate.get_living_entity().is_some()
                    && mob.can_attack(candidate.as_ref())
            })
            .filter(|candidate| {
                let distance =
                    mobile_pos.squared_distance_to_vec(&candidate.get_entity().pos.load());
                (RAM_MIN_DISTANCE * RAM_MIN_DISTANCE..=RAM_MAX_DISTANCE * RAM_MAX_DISTANCE)
                    .contains(&distance)
                    && mob.has_line_of_sight(candidate.get_entity())
            })
            .min_by(|a, b| {
                let a = mobile_pos.squared_distance_to_vec(&a.get_entity().pos.load());
                let b = mobile_pos.squared_distance_to_vec(&b.get_entity().pos.load());
                a.total_cmp(&b)
            })
    }

    fn finish(&mut self, mob: &dyn Mob) {
        let entity = mob.get_entity();
        entity
            .world
            .load()
            .send_entity_status(entity, EntityStatus::EndRam, None);
        if let Some(goat) = self.goat.upgrade() {
            goat.reset_ram_cooldown();
        }
        self.target = None;
        self.preparing = false;
        self.prepare_ticks = 0;
        self.charge_ticks = 0;
        mob.get_mob_entity()
            .navigator
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .stop();
    }

    fn ram_horn_breaking_block(mob: &dyn Mob) -> bool {
        let entity = mob.get_entity();
        let velocity = entity.velocity.load();
        let horizontal = Vector3::new(velocity.x, 0.0, velocity.z);
        if horizontal.length_squared() < 1.0e-7 {
            return false;
        }
        let pos = entity.pos.load();
        let direction = horizontal.normalize();
        let facing = BlockPos::containing(pos.x + direction.x, pos.y, pos.z + direction.z);
        let world = entity.world.load();
        world
            .get_block(&facing)
            .has_tag(&tag::Block::MINECRAFT_SNAPS_GOAT_HORN)
            || world
                .get_block(&facing.up())
                .has_tag(&tag::Block::MINECRAFT_SNAPS_GOAT_HORN)
    }
}

impl Goal for RamGoal {
    fn can_start(&mut self, mob: &dyn Mob) -> bool {
        let Some(goat) = self.goat.upgrade() else {
            return false;
        };
        if goat.ram_cooldown.load(Ordering::Relaxed) != 0 || goat.is_in_love() {
            return false;
        }
        if self.search_cooldown > 0 {
            self.search_cooldown -= 1;
            return false;
        }
        let Some(target) = Self::find_target(mob) else {
            self.search_cooldown = 10;
            return false;
        };
        self.target = Some(target.clone());
        self.target_block = target.get_entity().block_pos.load();
        self.preparing = true;
        self.prepare_ticks = RAM_PREPARE_TIME;
        self.charge_ticks = 0;
        true
    }

    fn should_continue(&mut self, _mob: &dyn Mob) -> bool {
        let Some(target) = &self.target else {
            return false;
        };
        if !target.get_entity().is_alive() {
            return false;
        }
        if self.preparing {
            target.get_entity().block_pos.load() == self.target_block
        } else {
            self.charge_ticks > 0
        }
    }

    fn start(&mut self, _mob: &dyn Mob) {}

    fn stop(&mut self, mob: &dyn Mob) {
        self.finish(mob);
    }

    #[expect(clippy::too_many_lines)]
    fn tick(&mut self, mob: &dyn Mob) {
        let Some(target) = self.target.clone() else {
            return;
        };
        let entity = mob.get_entity();

        if self.preparing {
            self.prepare_ticks -= 1;
            if self.prepare_ticks > 0 {
                return;
            }
            self.preparing = false;
            self.charge_ticks = RAM_CHARGE_TICKS;
            let ram_pos = target.get_entity().pos.load();
            let goat_pos = entity.pos.load();
            let dx = (ram_pos.x - goat_pos.x).signum() * 0.5;
            let dz = (ram_pos.z - goat_pos.z).signum() * 0.5;
            self.ram_direction = Vector3::new(goat_pos.x - ram_pos.x, 0.0, goat_pos.z - ram_pos.z);
            if self.ram_direction.length_squared() > 1.0e-7 {
                self.ram_direction = self.ram_direction.normalize();
            }
            let destination = Vector3::new(
                f64::from(self.target_block.0.x) + 0.5 + dx,
                f64::from(self.target_block.0.y),
                f64::from(self.target_block.0.z) + 0.5 + dz,
            );
            mob.get_mob_entity()
                .navigator
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .set_progress(crate::entity::ai::pathfinder::NavigatorGoal::new(
                    goat_pos,
                    destination,
                    RAM_SPEED,
                ));
            let sound = self
                .goat
                .upgrade()
                .map_or(Sound::EntityGoatPrepareRam, |goat| goat.prepare_ram_sound());
            entity.world.load().play_sound_fine(
                sound,
                SoundCategory::Neutral,
                &goat_pos,
                1.0,
                mob.get_mob_entity().living_entity.get_pitch(),
            );
            entity
                .world
                .load()
                .send_entity_status(entity, EntityStatus::StartRam, None);
            return;
        }

        self.charge_ticks -= 1;

        let rammed = {
            let Some(goat) = self.goat.upgrade() else {
                return;
            };
            let box_to_check = entity.bounding_box.load();
            let world = entity.world.load();
            let mob_griefing = world.level_info.load().game_rules.mob_griefing;
            world
                .get_entities_and_players_at_box(&box_to_check)
                .into_iter()
                .find(|candidate| {
                    let candidate_entity = candidate.get_entity();
                    candidate_entity.entity_id != entity.entity_id
                        && candidate_entity.entity_type != &EntityType::GOAT
                        && (mob_griefing
                            || candidate_entity.entity_type != &EntityType::ARMOR_STAND)
                        && candidate.get_living_entity().is_some()
                        && mob.can_attack(candidate.as_ref())
                })
                .inspect(|candidate| {
                    let damaged = mob.get_mob_entity().try_attack(entity, candidate.as_ref());
                    // Vanilla `RamTarget.tick` knocks the target back whether or not the hit
                    // landed, halved when a shield blocked it, scaled by the goat's speed and
                    // through `LivingEntity.knockback`, so knockback resistance applies.
                    let Some(living_target) = candidate.get_living_entity() else {
                        return;
                    };
                    let living = &mob.get_mob_entity().living_entity;
                    let effect_level = |effect| {
                        living
                            .get_effect(effect)
                            .map_or(0, |effect| effect.amplifier + 1)
                    };
                    let speed_boost = 0.25
                        * (effect_level(&StatusEffect::SPEED)
                            - effect_level(&StatusEffect::SLOWNESS))
                            as f32;
                    let speed = living.get_attribute_value(&Attributes::MOVEMENT_SPEED) * RAM_SPEED;
                    let speed_factor = (speed as f32 * 1.65).clamp(0.2, 3.0) + speed_boost;
                    let blocking_factor = if !damaged && living_target.is_blocking() {
                        0.5
                    } else {
                        1.0
                    };
                    let resistance =
                        living_target.get_attribute_value(&Attributes::KNOCKBACK_RESISTANCE);
                    living_target.entity.apply_knockback(
                        knockback_after_resistance(
                            f64::from(blocking_factor * speed_factor) * goat.ram_knockback_force(),
                            resistance,
                        ),
                        self.ram_direction.x,
                        self.ram_direction.z,
                    );
                })
        };

        if rammed.is_some() || Self::ram_horn_breaking_block(mob) {
            let sound = self
                .goat
                .upgrade()
                .map_or(Sound::EntityGoatRamImpact, |goat| goat.impact_sound());
            entity.world.load().play_sound_fine(
                sound,
                SoundCategory::Neutral,
                &entity.pos.load(),
                1.0,
                1.0,
            );
            self.charge_ticks = 0;
            return;
        }

        if self.charge_ticks <= 0 {
            return;
        }

        let target_pos = target.get_entity().pos.load();
        mob.get_mob_entity()
            .navigator
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .set_progress(crate::entity::ai::pathfinder::NavigatorGoal::new(
                entity.pos.load(),
                target_pos,
                RAM_SPEED,
            ));
    }

    fn should_run_every_tick(&self) -> bool {
        true
    }

    fn controls(&self) -> Controls {
        Controls::MOVE | Controls::LOOK | Controls::JUMP
    }
}

impl AgeableMob for GoatEntity {
    fn get_ageable_data(&self) -> &AgeableData {
        &self.ageable_data
    }

    fn age_boundary_reached(&self) {
        let mut attributes = self
            .mob_entity
            .living_entity
            .attributes
            .write()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if let Some(damage) = attributes.get_mut(&Attributes::ATTACK_DAMAGE.id) {
            damage.base_value = if self.is_baby() { 1.0 } else { 2.0 };
            damage.dirty.store(true, Ordering::Relaxed);
        }
    }
}

impl CustomSound for GoatEntity {
    fn death_sound(&self) -> Option<Sound> {
        Some(if self.is_screaming() {
            Sound::EntityGoatScreamingDeath
        } else {
            Sound::EntityGoatDeath
        })
    }

    fn hurt_sound(&self) -> Option<Sound> {
        Some(if self.is_screaming() {
            Sound::EntityGoatScreamingHurt
        } else {
            Sound::EntityGoatHurt
        })
    }
}

impl Animal for GoatEntity {
    fn is_food(&self, item_stack: &ItemStack) -> bool {
        item_stack.item.has_tag(&tag::Item::MINECRAFT_GOAT_FOOD)
            || TEMPT_ITEMS.iter().any(|i| i.id == item_stack.item.id)
    }
}

impl Mob for GoatEntity {
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
        nbt.put_bool("IsScreamingGoat", self.is_screaming());
        nbt.put_bool("HasLeftHorn", self.has_left_horn());
        nbt.put_bool("HasRightHorn", self.has_right_horn());
    }

    fn mob_read_nbt(&self, nbt: &NbtCompound) {
        if let Some(screaming) = nbt.get_bool("IsScreamingGoat") {
            self.set_screaming(screaming);
        }
        if let Some(left) = nbt.get_bool("HasLeftHorn") {
            self.set_has_left_horn(left);
        }
        if let Some(right) = nbt.get_bool("HasRightHorn") {
            self.set_has_right_horn(right);
        }
    }

    fn get_mob_entity(&self) -> &MobEntity {
        &self.mob_entity
    }

    fn mob_tick(&self, _caller: &dyn EntityBase) {
        if self.ram_cooldown.load(Ordering::Relaxed) > 0 {
            self.ram_cooldown.fetch_sub(1, Ordering::Relaxed);
        }
        self.ageable_ai_step();
    }

    fn finalize_spawn(
        &self,
        _world: &Arc<World>,
        group_data: Option<SpawnGroupData>,
    ) -> Option<SpawnGroupData> {
        self.mob_entity.finalize_spawn_base();
        let mut rng = rand::rng();
        self.set_screaming(rng.random::<f64>() < 0.02);
        self.age_boundary_reached();
        if !self.is_baby() && rng.random::<f32>() < 0.1 {
            if rng.random::<bool>() {
                self.set_has_left_horn(false);
            } else {
                self.set_has_right_horn(false);
            }
        }
        self.reset_ram_cooldown();
        group_data
    }

    fn mob_init_data_tracker(&self) {
        let entity = self.get_entity();
        let is_baby = entity.age.load(Ordering::Relaxed) < 0;
        if is_baby {
            entity.set_synced_data(pumpkin_data::tracked_data::goat::DATA_BABY_ID, true);
        }
        entity.set_synced_data(
            pumpkin_data::tracked_data::goat::DATA_IS_SCREAMING_GOAT,
            self.is_screaming(),
        );
        entity.set_synced_data(
            pumpkin_data::tracked_data::goat::DATA_HAS_LEFT_HORN,
            self.has_left_horn(),
        );
        entity.set_synced_data(
            pumpkin_data::tracked_data::goat::DATA_HAS_RIGHT_HORN,
            self.has_right_horn(),
        );
    }

    fn get_ambient_sound(&self) -> Option<Sound> {
        Some(if self.is_screaming() {
            Sound::EntityGoatScreamingAmbient
        } else {
            Sound::EntityGoatAmbient
        })
    }

    fn mob_interact(&self, player: &Arc<Player>, item_stack: &mut ItemStack) -> bool {
        self.animal_interact(player, item_stack, self.eat_sound())
    }
}
