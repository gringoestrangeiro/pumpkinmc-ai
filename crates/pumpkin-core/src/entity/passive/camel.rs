use std::sync::{
    Arc, Weak,
    atomic::{AtomicBool, AtomicI32, AtomicI64, AtomicU8, Ordering},
};

use pumpkin_data::entity::{EntityPose, EntityType};
use pumpkin_data::item::Item;
use pumpkin_data::item_stack::ItemStack;
use pumpkin_data::sound::{Sound, SoundCategory};
use pumpkin_data::tag::{self, Taggable};
use pumpkin_data::tracked_data::TrackedData;
use pumpkin_nbt::compound::NbtCompound;
use pumpkin_protocol::java::server::play::SPlayerInput;

use crate::entity::{
    Entity, EntityBase,
    ageable::{AgeableData, AgeableMob},
    ai::goal::{
        breed::BreedGoal, escape_danger::EscapeDangerGoal, follow_parent::FollowParentGoal,
        look_around::RandomLookAroundGoal, look_at_entity::LookAtEntityGoal, swim::SwimGoal,
        tempt::TemptGoal, wander_around::WanderAroundGoal,
    },
    mob::{Mob, MobEntity},
    passive::animal::Animal,
    player::Player,
};

use super::horse::{AbstractHorse, empty_mount_inventory, open_mount_screen};

const TEMPT_ITEMS: &[&Item] = &[&Item::CACTUS];
const HUSK_TEMPT_ITEMS: &[&Item] = &[&Item::RABBIT_FOOT];

pub const FLAG_TAME: u8 = 2;
pub const FLAG_SADDLE: u8 = 4;
pub const FLAG_BRED: u8 = 8;
pub const FLAG_EATING: u8 = 16;
pub const FLAG_STANDING: u8 = 32;

pub const DASH_COOLDOWN_TICKS: i32 = 55;
const DASH_MINIMUM_DURATION_TICKS: i32 = 5;
const SITDOWN_DURATION_TICKS: i64 = 40;
const STANDUP_DURATION_TICKS: i64 = 52;
const IDLE_MINIMAL_DURATION_TICKS: i64 = 400;
const SIT_CHECK_INTERVAL: u32 = 80;

/// Vanilla `CamelHusk` is a `Camel` subclass; the kind selects its sounds,
/// food tag, tracked-data ids and breeding rules.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum CamelKind {
    Camel,
    Husk,
}

pub struct CamelEntity {
    pub mob_entity: MobEntity,
    pub ageable_data: AgeableData,
    pub flags: AtomicU8,
    pub dashing: AtomicBool,
    pub last_pose_change_tick: AtomicI64,
    pub dash_cooldown: AtomicI32,
    pub stand_ticks: AtomicI32,
    pub kind: CamelKind,
}

impl CamelEntity {
    pub fn new(entity: Entity) -> Arc<Self> {
        Self::new_with_kind(entity, CamelKind::Camel)
    }

    pub fn new_husk(entity: Entity) -> Arc<Self> {
        Self::new_with_kind(entity, CamelKind::Husk)
    }

    fn new_with_kind(entity: Entity, kind: CamelKind) -> Arc<Self> {
        let mob_entity = MobEntity::new(entity);
        let world_age = mob_entity.living_entity.entity.world.load().get_world_age();
        let camel = Self {
            mob_entity,
            ageable_data: AgeableData::default(),
            flags: AtomicU8::new(0),
            dashing: AtomicBool::new(false),
            last_pose_change_tick: AtomicI64::new((world_age - STANDUP_DURATION_TICKS - 1).max(0)),
            dash_cooldown: AtomicI32::new(0),
            stand_ticks: AtomicI32::new(0),
            kind,
        };
        let mob_arc = Arc::new(camel);
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
            goal_selector.add_goal(1, EscapeDangerGoal::new(2.0));
            if kind == CamelKind::Camel {
                goal_selector.add_goal(2, BreedGoal::new(1.0));
            }
            goal_selector.add_goal(
                3,
                Box::new(TemptGoal::new(
                    1.25,
                    if kind == CamelKind::Camel {
                        TEMPT_ITEMS
                    } else {
                        HUSK_TEMPT_ITEMS
                    },
                    false,
                )),
            );
            if kind == CamelKind::Camel {
                goal_selector.add_goal(4, Box::new(FollowParentGoal::new(1.0)));
            }
            goal_selector.add_goal(6, Box::new(WanderAroundGoal::new(0.7)));
            goal_selector.add_goal(
                7,
                LookAtEntityGoal::with_default(mob_weak, &EntityType::PLAYER, 6.0),
            );
            goal_selector.add_goal(8, Box::new(RandomLookAroundGoal::default()));
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
        entity.set_synced_data(self.flags_data(), new_flags as i8);
    }

    fn flags_data(&self) -> TrackedData {
        if self.kind == CamelKind::Husk {
            pumpkin_data::tracked_data::camel_husk::DATA_ID_FLAGS
        } else {
            pumpkin_data::tracked_data::camel::DATA_ID_FLAGS
        }
    }

    fn dash_data(&self) -> TrackedData {
        if self.kind == CamelKind::Husk {
            pumpkin_data::tracked_data::camel_husk::DASH
        } else {
            pumpkin_data::tracked_data::camel::DASH
        }
    }

    fn last_pose_change_data(&self) -> TrackedData {
        if self.kind == CamelKind::Husk {
            pumpkin_data::tracked_data::camel_husk::LAST_POSE_CHANGE_TICK
        } else {
            pumpkin_data::tracked_data::camel::LAST_POSE_CHANGE_TICK
        }
    }

    fn baby_data(&self) -> TrackedData {
        if self.kind == CamelKind::Husk {
            pumpkin_data::tracked_data::camel_husk::DATA_BABY_ID
        } else {
            pumpkin_data::tracked_data::camel::DATA_BABY_ID
        }
    }

    #[must_use]
    pub fn is_saddled(&self) -> bool {
        self.has_flag(FLAG_SADDLE)
    }

    pub fn set_saddled(&self, val: bool) {
        self.set_flag(FLAG_SADDLE, val);
    }

    #[must_use]
    pub fn is_dashing(&self) -> bool {
        self.dashing.load(Ordering::Relaxed)
    }

    pub fn set_dashing(&self, dashing: bool) {
        self.dashing.store(dashing, Ordering::Relaxed);
        let entity = self.get_entity();
        entity.set_synced_data(self.dash_data(), dashing);
        if dashing && self.dash_cooldown.load(Ordering::Relaxed) == 0 {
            self.dash_cooldown
                .store(DASH_COOLDOWN_TICKS, Ordering::Relaxed);
        }
    }

    /// Vanilla `Camel.isCamelSitting`.
    #[must_use]
    pub fn is_camel_sitting(&self) -> bool {
        self.last_pose_change_tick.load(Ordering::Relaxed) < 0
    }

    /// Vanilla `Camel.getPoseTime`.
    #[must_use]
    pub fn get_pose_time(&self) -> i64 {
        self.get_entity().world.load().get_world_age()
            - self.last_pose_change_tick.load(Ordering::Relaxed).abs()
    }

    /// Vanilla `Camel.isInPoseTransition`.
    #[must_use]
    pub fn is_in_pose_transition(&self) -> bool {
        self.get_pose_time()
            < if self.is_camel_sitting() {
                SITDOWN_DURATION_TICKS
            } else {
                STANDUP_DURATION_TICKS
            }
    }

    /// Vanilla `Camel.refuseToMove`.
    #[must_use]
    pub fn refuse_to_move(&self) -> bool {
        self.is_camel_sitting() || self.is_in_pose_transition()
    }

    /// Vanilla `Camel.canCamelChangePose`: only standing up needs headroom.
    #[must_use]
    pub fn can_camel_change_pose(&self) -> bool {
        if !self.is_camel_sitting() {
            return true;
        }
        let entity = self.get_entity();
        let pos = entity.pos.load();
        let dimensions = entity.get_dimensions(EntityPose::Standing);
        let target = pumpkin_util::math::boundingbox::BoundingBox::new_from_pos(
            pos.x,
            pos.y,
            pos.z,
            &dimensions,
        );
        entity.world.load().is_space_empty(target)
    }

    pub fn reset_last_pose_change_tick(&self, tick: i64) {
        self.last_pose_change_tick.store(tick, Ordering::Relaxed);
        self.get_entity()
            .set_synced_data(self.last_pose_change_data(), tick);
    }

    fn reset_last_pose_change_tick_to_full_stand(&self, current_time: i64) {
        self.reset_last_pose_change_tick((current_time - STANDUP_DURATION_TICKS - 1).max(0));
    }

    /// Vanilla `Camel.sitDown`.
    pub fn sit_down(&self) {
        if !self.is_camel_sitting() {
            let entity = self.get_entity();
            entity.world.load().play_sound(
                self.get_sit_down_sound(),
                SoundCategory::Neutral,
                &entity.pos.load(),
            );
            entity.set_pose(EntityPose::Sitting);
            self.reset_last_pose_change_tick(-entity.world.load().get_world_age());
        }
    }

    /// Vanilla `Camel.standUp`.
    pub fn stand_up(&self) {
        if self.is_camel_sitting() {
            let entity = self.get_entity();
            entity.world.load().play_sound(
                self.get_stand_up_sound(),
                SoundCategory::Neutral,
                &entity.pos.load(),
            );
            entity.set_pose(EntityPose::Standing);
            self.reset_last_pose_change_tick(entity.world.load().get_world_age());
        }
    }

    /// Vanilla `Camel.standUpInstantly`.
    pub fn stand_up_instantly(&self) {
        let entity = self.get_entity();
        entity.set_pose(EntityPose::Standing);
        self.reset_last_pose_change_tick_to_full_stand(entity.world.load().get_world_age());
    }

    fn get_sit_down_sound(&self) -> Sound {
        if self.kind == CamelKind::Husk {
            Sound::EntityCamelHuskSit
        } else {
            Sound::EntityCamelSit
        }
    }

    fn get_stand_up_sound(&self) -> Sound {
        if self.kind == CamelKind::Husk {
            Sound::EntityCamelHuskStand
        } else {
            Sound::EntityCamelStand
        }
    }

    fn get_dash_sound(&self) -> Sound {
        if self.kind == CamelKind::Husk {
            Sound::EntityCamelHuskDash
        } else {
            Sound::EntityCamelDash
        }
    }

    fn get_dash_ready_sound(&self) -> Sound {
        if self.kind == CamelKind::Husk {
            Sound::EntityCamelHuskDashReady
        } else {
            Sound::EntityCamelDashReady
        }
    }

    fn get_saddle_sound(&self) -> Sound {
        if self.kind == CamelKind::Husk {
            Sound::EntityCamelHuskSaddle
        } else {
            Sound::EntityCamelSaddle
        }
    }

    /// Vanilla `Camel.tick`, the server-side parts.
    fn tick_camel(&self) {
        let entity = self.get_entity();
        let world = entity.world.load();

        if self.is_dashing()
            && self.dash_cooldown.load(Ordering::Relaxed)
                < DASH_COOLDOWN_TICKS - DASH_MINIMUM_DURATION_TICKS
            && (entity.on_ground.load(Ordering::Relaxed)
                || entity.touching_water.load(Ordering::Relaxed)
                || entity.touching_lava.load(Ordering::Relaxed)
                || entity.has_vehicle())
        {
            self.set_dashing(false);
        }

        let cooldown = self.dash_cooldown.load(Ordering::Relaxed);
        if cooldown > 0 {
            let next = cooldown - 1;
            self.dash_cooldown.store(next, Ordering::Relaxed);
            if next == 0 {
                world.play_sound(
                    self.get_dash_ready_sound(),
                    SoundCategory::Neutral,
                    &entity.pos.load(),
                );
            }
        }

        if self.is_camel_sitting() && entity.touching_water.load(Ordering::Relaxed) {
            self.stand_up_instantly();
        }

        self.tick_ridden_input();
        self.tick_random_sitting();
    }

    /// Vanilla `Camel.tickRidden`: a forward input stands the camel up.
    fn tick_ridden_input(&self) {
        if !self.is_saddled() || !self.is_camel_sitting() || self.is_in_pose_transition() {
            return;
        }
        let entity = self.get_entity();
        let driver = entity
            .passengers
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .first()
            .and_then(|passenger| passenger.get_player())
            .map(|player| player.last_input.load(Ordering::Relaxed));
        if driver.is_some_and(|input| input & SPlayerInput::FORWARD != 0) {
            self.stand_up();
        }
    }

    /// Vanilla `CamelAi.RandomSitting`: the idle brain sits or stands a camel.
    fn tick_random_sitting(&self) {
        if !self.can_sit_automatically() {
            return;
        }
        if rand::random::<u32>() % SIT_CHECK_INTERVAL != 0 {
            return;
        }
        if self.is_camel_sitting() {
            self.stand_up();
        } else if !self.is_panicking() {
            self.sit_down();
        }
    }

    fn can_sit_automatically(&self) -> bool {
        let entity = self.get_entity();
        !entity.touching_water.load(Ordering::Relaxed)
            && self.get_pose_time() >= IDLE_MINIMAL_DURATION_TICKS
            && !entity.is_leashed()
            && entity.on_ground.load(Ordering::Relaxed)
            && !entity.has_passengers()
            && self.can_camel_change_pose()
    }

    /// Vanilla `Camel.handleEating`.
    fn handle_camel_eating(&self, player: &Player, item_stack: &ItemStack) -> bool {
        if !self.is_food(item_stack) {
            return false;
        }
        let mut item_used = false;
        let living = &self.mob_entity.living_entity;
        if living.health.load() < living.get_max_health() {
            living.heal(2.0);
            item_used = true;
        }

        if self.kind == CamelKind::Camel && self.get_age() == 0 && !self.mob_entity.is_in_love() {
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

        if self.can_age_up() {
            let entity = self.get_entity();
            entity.world.load().spawn_particle(
                entity.pos.load() + pumpkin_util::math::vector3::Vector3::new(0.0, 0.5, 0.0),
                pumpkin_util::math::vector3::Vector3::new(0.5, 0.5, 0.5),
                1.0,
                7,
                pumpkin_data::particle::Particle::HappyVillager,
            );
            self.age_up(10, false);
            item_used = true;
        }

        if item_used {
            let entity = self.get_entity();
            entity.world.load().play_sound(
                if self.kind == CamelKind::Husk {
                    Sound::EntityCamelHuskEat
                } else {
                    Sound::EntityCamelEat
                },
                SoundCategory::Neutral,
                &entity.pos.load(),
            );
        }
        item_used
    }
}

impl AbstractHorse for CamelEntity {
    fn has_horse_flag(&self, flag: u8) -> bool {
        self.has_flag(flag)
    }

    fn set_horse_flag(&self, flag: u8, val: bool) {
        self.set_flag(flag, val);
    }

    fn get_stand_ticks(&self) -> &AtomicI32 {
        &self.stand_ticks
    }

    fn set_horse_owner(&self, _owner: Option<uuid::Uuid>) {}

    fn get_temper(&self) -> i32 {
        0
    }

    fn set_temper(&self, _temper: i32) {}

    fn is_horse_tamed(&self) -> bool {
        // Vanilla `Camel.isTamed` is always true.
        true
    }

    fn set_horse_tamed(&self, _tame: bool) {}

    fn can_perform_rearing(&self) -> bool {
        false
    }

    fn can_jump(&self) -> bool {
        !self.refuse_to_move() && self.is_saddled()
    }

    fn handle_start_jump(&self, _jump_scale: i32) {
        let entity = self.get_entity();
        entity.world.load().play_sound(
            self.get_dash_sound(),
            SoundCategory::Neutral,
            &entity.pos.load(),
        );
        self.set_dashing(true);
    }

    fn get_horse_eating_sound(&self) -> Option<Sound> {
        Some(if self.kind == CamelKind::Husk {
            Sound::EntityCamelHuskEat
        } else {
            Sound::EntityCamelEat
        })
    }
}

impl AgeableMob for CamelEntity {
    fn get_ageable_data(&self) -> &AgeableData {
        &self.ageable_data
    }

    fn can_be_a_baby(&self) -> bool {
        self.kind == CamelKind::Camel
    }
}

impl Animal for CamelEntity {
    fn is_food(&self, item_stack: &ItemStack) -> bool {
        if self.kind == CamelKind::Husk {
            item_stack
                .item
                .has_tag(&tag::Item::MINECRAFT_CAMEL_HUSK_FOOD)
        } else {
            item_stack.item.has_tag(&tag::Item::MINECRAFT_CAMEL_FOOD)
                || item_stack.item == &Item::CACTUS
        }
    }
}

impl Mob for CamelEntity {
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
        CamelEntity::is_saddled(self)
    }

    fn set_saddled(&self, saddled: bool) {
        CamelEntity::set_saddled(self, saddled);
    }

    fn get_ambient_sound(&self) -> Option<Sound> {
        Some(if self.kind == CamelKind::Husk {
            Sound::EntityCamelHuskAmbient
        } else {
            Sound::EntityCamelAmbient
        })
    }

    fn get_ambient_sound_interval(&self) -> i32 {
        400
    }

    fn remove_when_far_away(&self, _distance_sq: f64) -> bool {
        self.kind == CamelKind::Husk
    }

    fn is_mob_controlled(&self) -> bool {
        self.kind == CamelKind::Husk
            && self
                .get_entity()
                .passengers
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .first()
                .is_some_and(|passenger| passenger.get_mob().is_some())
    }

    fn mob_write_nbt(&self, nbt: &mut NbtCompound) {
        self.write_ageable_nbt(nbt);
        nbt.put_bool("Saddle", self.is_saddled());
        nbt.put_long(
            "LastPoseTick",
            self.last_pose_change_tick.load(Ordering::Relaxed),
        );
    }

    fn mob_read_nbt(&self, nbt: &NbtCompound) {
        self.read_ageable_nbt(nbt);
        if let Some(saddle) = nbt.get_bool("Saddle") {
            self.set_saddled(saddle);
        }
        let pose_tick = nbt.get_long("LastPoseTick").unwrap_or(0);
        if pose_tick < 0 {
            self.get_entity().set_pose(EntityPose::Sitting);
        }
        self.reset_last_pose_change_tick(pose_tick);
    }

    fn get_mob_entity(&self) -> &MobEntity {
        &self.mob_entity
    }

    fn mob_tick(&self, _caller: &dyn EntityBase) {
        self.ageable_ai_step();
        self.tick_camel();
        self.horse_tick();
    }

    fn pre_damage(
        &self,
        _damage_type: pumpkin_data::damage::DamageType,
        _source: Option<&dyn EntityBase>,
    ) -> bool {
        self.stand_up_instantly();
        true
    }

    fn mob_init_data_tracker(&self) {
        let entity = self.get_entity();
        let is_baby = entity.age.load(Ordering::Relaxed) < 0;
        if is_baby {
            entity.set_synced_data(self.baby_data(), true);
        }
        entity.set_synced_data(self.flags_data(), self.flags.load(Ordering::Relaxed) as i8);
        entity.set_synced_data(self.dash_data(), self.is_dashing());
        entity.set_synced_data(
            self.last_pose_change_data(),
            self.last_pose_change_tick.load(Ordering::Relaxed),
        );
    }

    fn mob_interact(&self, player: &Arc<Player>, item_stack: &mut ItemStack) -> bool {
        if self.kind == CamelKind::Husk {
            self.mob_entity
                .persistence_required
                .store(true, Ordering::Relaxed);
        }

        let item = item_stack.get_item();

        if !self.is_baby() && player.get_entity().is_sneaking() {
            open_mount_screen(self.get_entity(), player, empty_mount_inventory(), 0);
            return true;
        }

        if item == &Item::SADDLE && !self.is_saddled() && !self.is_baby() {
            self.set_saddled(true);
            item_stack.decrement_unless_creative(player.gamemode.load(), 1);
            let entity = self.get_entity();
            entity.world.load().play_sound(
                self.get_saddle_sound(),
                SoundCategory::Neutral,
                &entity.pos.load(),
            );
            return true;
        }

        if self.is_food(item_stack) {
            if self.handle_camel_eating(player, item_stack) {
                item_stack.decrement_unless_creative(player.gamemode.load(), 1);
            }
            return true;
        }

        if self
            .get_entity()
            .passengers
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .len()
            < 2
            && !self.is_baby()
        {
            self.do_player_ride(player);
            return true;
        }

        self.animal_interact(
            player,
            item_stack,
            if self.kind == CamelKind::Husk {
                Sound::EntityCamelHuskAmbient
            } else {
                Sound::EntityCamelAmbient
            },
        )
    }
}
