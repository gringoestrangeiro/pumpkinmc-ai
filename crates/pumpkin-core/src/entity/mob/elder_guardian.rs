use std::sync::atomic::Ordering;
use std::sync::{Arc, Weak};

use pumpkin_data::damage::DamageType;
use pumpkin_data::effect::StatusEffect;
use pumpkin_data::entity::EntityType;
use pumpkin_data::potion::Effect;
use pumpkin_protocol::java::client::play::{CGameEvent, GameEvent};
use pumpkin_util::GameMode;

use crate::entity::{
    Entity, EntityBase,
    ai::goal::{
        active_target::ActiveTargetGoal, look_around::RandomLookAroundGoal,
        look_at_entity::LookAtEntityGoal, move_towards_restriction::MoveTowardsRestrictionGoal,
        swim::SwimGoal, wander_around::WanderAroundGoal,
    },
    mob::{
        Mob, MobEntity,
        guardian::{GuardianAttackGoal, try_guardian_thorns},
    },
};

const EFFECT_INTERVAL: i32 = 1200;
const EFFECT_RADIUS: f64 = 50.0;
const EFFECT_DURATION: i32 = 6000;
const EFFECT_AMPLIFIER: u8 = 2;
const EFFECT_DISPLAY_LIMIT: i32 = 1200;
const ELDER_ATTACK_DURATION: i32 = 60;
const HOME_RADIUS: i32 = 16;
const STROLL_INTERVAL: i32 = 400;

pub struct ElderGuardianEntity {
    pub mob_entity: MobEntity,
}

impl ElderGuardianEntity {
    pub fn new(entity: Entity) -> Arc<Self> {
        let mob_entity = MobEntity::new(entity);
        mob_entity
            .persistence_required
            .store(true, Ordering::Relaxed);
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
                Box::new(GuardianAttackGoal::new(true, ELDER_ATTACK_DURATION)),
            );
            goal_selector.add_goal(5, Box::new(MoveTowardsRestrictionGoal::new(1.0)));
            goal_selector.add_goal(
                7,
                Box::new(WanderAroundGoal::with_interval(1.0, STROLL_INTERVAL)),
            );
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

impl Mob for ElderGuardianEntity {
    fn get_mob_entity(&self) -> &MobEntity {
        &self.mob_entity
    }

    fn mob_tick(&self, _caller: &dyn EntityBase) {
        let entity = &self.mob_entity.living_entity.entity;
        if !entity.is_alive() {
            return;
        }

        // Vanilla `ElderGuardian.customServerAiStep`: the timer includes the entity id.
        let tick_count = entity.age.load(Ordering::Relaxed);
        if (tick_count + entity.entity_id) % EFFECT_INTERVAL == 0 {
            let pos = entity.pos.load();
            let world = entity.world.load();
            let packet = CGameEvent::new(GameEvent::PlayElderGuardianMobAppearance, 1.0);

            for player in world.get_nearby_players(pos, EFFECT_RADIUS) {
                if matches!(
                    player.gamemode.load(),
                    GameMode::Creative | GameMode::Spectator
                ) {
                    continue;
                }
                let current = player
                    .living_entity
                    .get_effect(&StatusEffect::MINING_FATIGUE);
                if current.is_some_and(|effect| {
                    effect.duration < 0 || effect.duration >= EFFECT_DISPLAY_LIMIT
                }) {
                    continue;
                }
                player.try_send_client_packet(&packet);
                player.living_entity.add_effect(Effect {
                    effect_type: &StatusEffect::MINING_FATIGUE,
                    duration: EFFECT_DURATION,
                    amplifier: EFFECT_AMPLIFIER,
                    ambient: false,
                    show_particles: true,
                    show_icon: true,
                    blend: false,
                });
            }
        }

        if !self.mob_entity.has_position_target() {
            self.mob_entity
                .position_target
                .store(entity.block_pos.load());
            self.mob_entity
                .position_target_range
                .store(HOME_RADIUS, Ordering::Relaxed);
        }
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
