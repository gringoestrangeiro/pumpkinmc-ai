use std::sync::atomic::Ordering;
use std::sync::{Arc, Weak};

use pumpkin_data::entity::EntityType;
use pumpkin_data::tag::Taggable;
use pumpkin_data::tracked_data;
use pumpkin_data::{damage::DamageType, sound::Sound};
use pumpkin_protocol::java::client::play::CEntityStatus;
use pumpkin_util::Difficulty;

use crate::entity::custom_sound::CustomSound;
use crate::entity::{
    Entity, EntityBase,
    ai::goal::{
        Controls, Goal, active_target::ActiveTargetGoal, look_around::RandomLookAroundGoal,
        look_at_entity::LookAtEntityGoal, swim::SwimGoal, wander_around::WanderAroundGoal,
    },
    mob::{Mob, MobEntity},
};

/// Vanilla `Guardian.ATTACK_TIME`.
pub const GUARDIAN_ATTACK_DURATION: i32 = 80;
/// Vanilla `Guardian.GuardianAttackSelector`: beams only start beyond 3 blocks.
const MIN_ATTACK_DISTANCE_SQ: f64 = 9.0;

pub struct GuardianEntity {
    pub mob_entity: MobEntity,
}

/// Vanilla `Guardian.GuardianAttackGoal`: charge the beam, then deal magic and melee damage.
pub struct GuardianAttackGoal {
    attack_time: i32,
    attack_duration: i32,
    elder: bool,
}

impl GuardianAttackGoal {
    #[must_use]
    pub const fn new(elder: bool, attack_duration: i32) -> Self {
        Self {
            attack_time: -10,
            attack_duration,
            elder,
        }
    }

    fn set_active_attack_target(mob: &dyn Mob, entity_id: i32) {
        mob.get_entity()
            .set_synced_data(tracked_data::guardian::DATA_ID_ATTACK_TARGET, entity_id);
    }

    fn look_at_target(mob: &dyn Mob, target: &Arc<dyn EntityBase>) {
        mob.get_mob_entity()
            .look_control
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .look_at_entity_with_range(target, 90.0, 90.0);
    }
}

impl Goal for GuardianAttackGoal {
    fn can_start(&mut self, mob: &dyn Mob) -> bool {
        mob.get_mob_entity().get_target().is_some_and(|target| {
            target.get_entity().is_alive()
                && target
                    .get_entity()
                    .pos
                    .load()
                    .squared_distance_to_vec(&mob.get_entity().pos.load())
                    > MIN_ATTACK_DISTANCE_SQ
        })
    }

    fn should_continue(&mut self, mob: &dyn Mob) -> bool {
        let Some(target) = mob.get_mob_entity().get_target() else {
            return false;
        };
        if !target.get_entity().is_alive() {
            return false;
        }
        // Vanilla keeps elders on the beam at any distance; normal guardians drop it up close.
        self.elder
            || target
                .get_entity()
                .pos
                .load()
                .squared_distance_to_vec(&mob.get_entity().pos.load())
                > MIN_ATTACK_DISTANCE_SQ
    }

    fn start(&mut self, mob: &dyn Mob) {
        self.attack_time = -10;
        mob.get_mob_entity()
            .navigator
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .stop();
        if let Some(target) = mob.get_mob_entity().get_target() {
            Self::look_at_target(mob, &target);
        }
    }

    fn stop(&mut self, mob: &dyn Mob) {
        Self::set_active_attack_target(mob, 0);
        mob.set_mob_target(None);
    }

    fn tick(&mut self, mob: &dyn Mob) {
        let Some(target) = mob.get_mob_entity().get_target() else {
            return;
        };
        mob.get_mob_entity()
            .navigator
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .stop();
        Self::look_at_target(mob, &target);

        if !mob.has_line_of_sight(target.get_entity()) {
            mob.set_mob_target(None);
            Self::set_active_attack_target(mob, 0);
            return;
        }

        self.attack_time += 1;
        if self.attack_time == 0 {
            Self::set_active_attack_target(mob, target.get_entity().entity_id);
            if !mob.get_entity().is_silent() {
                mob.get_entity()
                    .world
                    .load()
                    .send_to_tracking_players_and_self(
                        mob.get_entity(),
                        &CEntityStatus::new(mob.get_entity().entity_id, 21),
                    );
            }
        } else if self.attack_time >= self.attack_duration {
            let mut magic_damage = 1.0;
            if mob.get_entity().world.load().level_info.load().difficulty == Difficulty::Hard {
                magic_damage += 2.0;
            }
            if self.elder {
                magic_damage += 2.0;
            }

            target.damage_with_context(
                target.as_ref(),
                magic_damage,
                DamageType::INDIRECT_MAGIC,
                None,
                Some(mob as &dyn EntityBase),
                Some(mob as &dyn EntityBase),
            );
            mob.get_mob_entity().try_attack(mob, target.as_ref());
            mob.set_mob_target(None);
            Self::set_active_attack_target(mob, 0);
        }
    }

    fn should_run_every_tick(&self) -> bool {
        true
    }

    fn controls(&self) -> Controls {
        Controls::MOVE | Controls::LOOK
    }
}

impl GuardianEntity {
    pub fn new(entity: Entity) -> Arc<Self> {
        let mob_entity = MobEntity::new(entity);
        let guardian = Self { mob_entity };
        let mob_arc = Arc::new(guardian);
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
            goal_selector.add_goal(
                4,
                Box::new(GuardianAttackGoal::new(false, GUARDIAN_ATTACK_DURATION)),
            );
            goal_selector.add_goal(7, Box::new(WanderAroundGoal::new(1.0)));
            goal_selector.add_goal(
                8,
                LookAtEntityGoal::with_default(mob_weak.clone(), &EntityType::PLAYER, 8.0),
            );
            goal_selector.add_goal(9, Box::new(RandomLookAroundGoal::default()));

            let mut target_selector = mob_arc
                .mob_entity
                .target_selector
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            target_selector.add_goal(
                1,
                ActiveTargetGoal::with_default(&mob_arc.mob_entity, &EntityType::PLAYER, true),
            );
            target_selector.add_goal(
                2,
                ActiveTargetGoal::with_default(&mob_arc.mob_entity, &EntityType::SQUID, true),
            );
            target_selector.add_goal(
                2,
                ActiveTargetGoal::with_default(&mob_arc.mob_entity, &EntityType::GLOW_SQUID, true),
            );
            target_selector.add_goal(
                3,
                ActiveTargetGoal::with_default(&mob_arc.mob_entity, &EntityType::AXOLOTL, true),
            );
        };

        mob_arc
    }
}

impl CustomSound for GuardianEntity {
    fn hurt_sound(&self) -> Option<Sound> {
        let entity = self.get_entity();
        let is_water = entity.touching_water.load(Ordering::Relaxed);
        Some(if is_water {
            Sound::EntityGuardianHurt
        } else {
            Sound::EntityGuardianHurtLand
        })
    }

    fn death_sound(&self) -> Option<Sound> {
        let entity = self.get_entity();
        let is_water = entity.touching_water.load(Ordering::Relaxed);
        Some(if is_water {
            Sound::EntityGuardianDeath
        } else {
            Sound::EntityGuardianDeathLand
        })
    }
}

impl Mob for GuardianEntity {
    fn get_mob_entity(&self) -> &MobEntity {
        &self.mob_entity
    }

    fn on_damage(&self, damage_type: DamageType, source: Option<&dyn EntityBase>) {
        try_guardian_thorns(
            self as &dyn EntityBase,
            &self.mob_entity,
            damage_type,
            source,
        );
    }
}

/// Vanilla `Guardian.hurtServer`: thorns only while stationary and not thorns-immune damage.
pub(crate) fn try_guardian_thorns(
    attacker: &dyn EntityBase,
    mob_entity: &MobEntity,
    damage_type: DamageType,
    source: Option<&dyn EntityBase>,
) {
    let moving = !mob_entity
        .navigator
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .is_idle();
    if moving
        || damage_type.has_tag(&pumpkin_data::tag::DamageType::MINECRAFT_AVOIDS_GUARDIAN_THORNS)
        || damage_type == DamageType::THORNS
    {
        return;
    }
    if let Some(src) = source
        && src.get_living_entity().is_some()
    {
        src.damage_with_context(
            src,
            2.0,
            DamageType::THORNS,
            None,
            Some(attacker),
            Some(attacker),
        );
    }
}
