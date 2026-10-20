use std::sync::atomic::{AtomicI32, Ordering};
use std::sync::{Arc, Weak};

use pumpkin_data::attributes::Attributes;
use pumpkin_data::damage::DamageType;
use pumpkin_data::entity::{EntityStatus, EntityType};
use pumpkin_data::sound::{Sound, SoundCategory};
use pumpkin_data::tag::{self, Taggable};
use pumpkin_nbt::compound::NbtCompound;
use pumpkin_util::math::vector3::Vector3;

use crate::entity::{
    Entity, EntityBase,
    ai::goal::{
        active_target::ActiveTargetGoal, look_around::RandomLookAroundGoal,
        look_at_entity::LookAtEntityGoal, melee_attack::MeleeAttackGoal, swim::SwimGoal,
        wander_around::WanderAroundGoal,
    },
    mob::{
        Mob, MobEntity,
        patrol::{LongDistancePatrolGoal, PatrolData, PatrollingMonster},
        raider::{
            HoldGroundAttackGoal, PathfindToRaidGoal, Raider, RaiderCelebrationGoal, RaiderData,
            RaiderMoveThroughVillageGoal,
        },
    },
};

pub struct RavagerEntity {
    pub mob_entity: MobEntity,
    pub raider_data: RaiderData,
    attack_tick: AtomicI32,
    stunned_tick: AtomicI32,
    roar_tick: AtomicI32,
}

impl RavagerEntity {
    const ATTACK_DURATION: i32 = 10;
    const STUN_DURATION: i32 = 40;
    const ROAR_DURATION: i32 = 20;
    const BASE_MOVEMENT_SPEED: f64 = 0.3;
    const ATTACK_MOVEMENT_SPEED: f64 = 0.35;
    const MOVEMENT_SPEED_LERP: f64 = 0.1;
    const ROAR_DAMAGE: f32 = 6.0;
    const ROAR_RADIUS: f64 = 4.0;
    const ROAR_KNOCKBACK: f64 = 4.0;
    const ROAR_KNOCKBACK_Y: f64 = 0.2;
    const STUN_CHANCE: f64 = 0.5;

    #[must_use]
    pub fn new(entity: Entity) -> Arc<Self> {
        let mob_entity = MobEntity::new(entity);
        let ravager = Self {
            mob_entity,
            raider_data: RaiderData::default(),
            attack_tick: AtomicI32::new(0),
            stunned_tick: AtomicI32::new(0),
            roar_tick: AtomicI32::new(0),
        };
        let mob_arc = Arc::new(ravager);
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
            goal_selector.add_goal(2, Box::new(HoldGroundAttackGoal::new(10.0)));
            goal_selector.add_goal(4, Box::new(MeleeAttackGoal::new(1.0, true)));
            goal_selector.add_goal(4, Box::new(LongDistancePatrolGoal::new(0.7, 0.595)));
            goal_selector.add_goal(4, Box::new(RaiderMoveThroughVillageGoal::new(1.05)));
            goal_selector.add_goal(4, Box::new(PathfindToRaidGoal::default()));
            goal_selector.add_goal(5, Box::new(RaiderCelebrationGoal));
            goal_selector.add_goal(5, Box::new(WanderAroundGoal::new(1.0)));
            goal_selector.add_goal(
                6,
                LookAtEntityGoal::with_default(mob_weak, &EntityType::PLAYER, 8.0),
            );
            goal_selector.add_goal(7, Box::new(RandomLookAroundGoal::default()));

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
                ActiveTargetGoal::with_default(&mob_arc.mob_entity, &EntityType::VILLAGER, true),
            );
            target_selector.add_goal(
                3,
                ActiveTargetGoal::with_default(&mob_arc.mob_entity, &EntityType::IRON_GOLEM, true),
            );
        };

        mob_arc
    }

    /// Vanilla `Ravager.hasLineOfSight`: stunned and roaring ravagers cannot see targets.
    fn is_stunned_or_roaring(&self) -> bool {
        self.stunned_tick.load(Ordering::Relaxed) > 0 || self.roar_tick.load(Ordering::Relaxed) > 0
    }

    fn is_immobile(&self) -> bool {
        self.attack_tick.load(Ordering::Relaxed) > 0 || self.is_stunned_or_roaring()
    }

    fn update_movement_speed(&self) {
        let moving_speed = if self.mob_entity.get_target().is_some() {
            Self::ATTACK_MOVEMENT_SPEED
        } else {
            Self::BASE_MOVEMENT_SPEED
        };
        let immobile = self.is_immobile();
        self.mob_entity
            .living_entity
            .update_attribute(&Attributes::MOVEMENT_SPEED, |speed| {
                speed.base_value = if immobile {
                    0.0
                } else {
                    speed.base_value.mul_add(
                        1.0 - Self::MOVEMENT_SPEED_LERP,
                        moving_speed * Self::MOVEMENT_SPEED_LERP,
                    )
                };
                speed.dirty.store(true, Ordering::Relaxed);
            });
    }

    fn strong_knockback(&self, target: &Entity) {
        let my_pos = self.mob_entity.living_entity.entity.pos.load();
        let target_pos = target.pos.load();
        let dx = target_pos.x - my_pos.x;
        let dz = target_pos.z - my_pos.z;
        let dd = (dx * dx + dz * dz).max(0.001);
        target.add_velocity(Vector3::new(
            dx / dd * Self::ROAR_KNOCKBACK,
            Self::ROAR_KNOCKBACK_Y,
            dz / dd * Self::ROAR_KNOCKBACK,
        ));
    }

    /// Vanilla `Ravager.roar`.
    fn roar(&self, caller: &dyn EntityBase) {
        let entity = &self.mob_entity.living_entity.entity;
        let world = entity.world.load();
        let ravager_pos = entity.pos.load();
        let griefing = world.level_info.load().game_rules.mob_griefing;
        let targets = world.get_entities_and_players_at_box(&entity.bounding_box.load().expand(
            Self::ROAR_RADIUS,
            Self::ROAR_RADIUS,
            Self::ROAR_RADIUS,
        ));

        for target in targets {
            let target_entity = target.get_entity();
            if target_entity.entity_id == entity.entity_id
                || *target_entity.entity_type == EntityType::RAVAGER
                || !target_entity.is_alive()
            {
                continue;
            }
            if !griefing && *target_entity.entity_type == EntityType::ARMOR_STAND {
                continue;
            }
            let Some(living) = target.get_living_entity() else {
                continue;
            };
            if !target_entity
                .entity_type
                .has_tag(&tag::EntityType::MINECRAFT_ILLAGER)
            {
                living.damage_with_context(
                    caller,
                    Self::ROAR_DAMAGE,
                    DamageType::MOB_ATTACK,
                    Some(ravager_pos),
                    Some(caller),
                    Some(caller),
                );
            }
            if *target_entity.entity_type != EntityType::PLAYER {
                self.strong_knockback(target_entity);
            }
        }

        world.send_entity_status(entity, EntityStatus::RavagerRoared, None);
    }
}

impl Mob for RavagerEntity {
    fn get_mob_entity(&self) -> &MobEntity {
        &self.mob_entity
    }

    fn as_patrolling_monster(&self) -> Option<&dyn PatrollingMonster> {
        Some(self)
    }

    fn as_raider(&self) -> Option<&dyn Raider> {
        Some(self)
    }

    fn mob_write_nbt(&self, nbt: &mut NbtCompound) {
        self.write_raider_nbt(nbt);
        nbt.put_int("AttackTick", self.attack_tick.load(Ordering::Relaxed));
        nbt.put_int("StunTick", self.stunned_tick.load(Ordering::Relaxed));
        nbt.put_int("RoarTick", self.roar_tick.load(Ordering::Relaxed));
    }

    fn mob_read_nbt(&self, nbt: &NbtCompound) {
        self.read_raider_nbt(nbt);
        if let Some(attack_tick) = nbt.get_int("AttackTick") {
            self.attack_tick.store(attack_tick, Ordering::Relaxed);
        }
        if let Some(stunned_tick) = nbt.get_int("StunTick") {
            self.stunned_tick.store(stunned_tick, Ordering::Relaxed);
        }
        if let Some(roar_tick) = nbt.get_int("RoarTick") {
            self.roar_tick.store(roar_tick, Ordering::Relaxed);
        }
    }

    fn mob_tick(&self, caller: &dyn EntityBase) {
        let entity = &self.mob_entity.living_entity.entity;
        if !entity.is_alive() {
            return;
        }

        self.update_movement_speed();

        let roar_tick = self.roar_tick.load(Ordering::Relaxed);
        if roar_tick > 0 {
            let next = roar_tick - 1;
            self.roar_tick.store(next, Ordering::Relaxed);
            if next == Self::ROAR_DURATION / 2 {
                self.roar(caller);
            }
        }

        let attack_tick = self.attack_tick.load(Ordering::Relaxed);
        if attack_tick > 0 {
            self.attack_tick.store(attack_tick - 1, Ordering::Relaxed);
        }

        let stunned_tick = self.stunned_tick.load(Ordering::Relaxed);
        if stunned_tick > 0 {
            let next = stunned_tick - 1;
            self.stunned_tick.store(next, Ordering::Relaxed);
            if next == 0 {
                let pos = entity.pos.load();
                entity.world.load().play_sound_fine(
                    Sound::EntityRavagerRoar,
                    SoundCategory::Hostile,
                    &pos,
                    1.0,
                    1.0,
                );
                self.roar_tick.store(Self::ROAR_DURATION, Ordering::Relaxed);
            }
        }
    }

    fn has_line_of_sight(&self, target: &Entity) -> bool {
        if self.is_stunned_or_roaring() {
            return false;
        }
        let mob_entity = self.get_mob_entity();
        mob_entity
            .sensing
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .has_line_of_sight(&mob_entity.living_entity.entity, target)
    }

    fn on_attack(&self, _target: &dyn EntityBase) {
        self.attack_tick
            .store(Self::ATTACK_DURATION, Ordering::Relaxed);
        let entity = &self.mob_entity.living_entity.entity;
        let world = entity.world.load();
        world.send_entity_status(entity, EntityStatus::StartAttacking, None);
        world.play_sound_fine(
            Sound::EntityRavagerAttack,
            SoundCategory::Hostile,
            &entity.pos.load(),
            1.0,
            1.0,
        );
    }

    fn blocked_by_item(&self, defender: &dyn EntityBase) {
        if self.roar_tick.load(Ordering::Relaxed) != 0 {
            return;
        }

        // Vanilla `Ravager.doHurtTarget` starts the attack animation before the hit is blocked.
        self.attack_tick
            .store(Self::ATTACK_DURATION, Ordering::Relaxed);
        let entity = &self.mob_entity.living_entity.entity;
        let world = entity.world.load();
        world.send_entity_status(entity, EntityStatus::StartAttacking, None);
        world.play_sound_fine(
            Sound::EntityRavagerAttack,
            SoundCategory::Hostile,
            &entity.pos.load(),
            1.0,
            1.0,
        );

        if rand::random::<f64>() < Self::STUN_CHANCE {
            self.stunned_tick
                .store(Self::STUN_DURATION, Ordering::Relaxed);
            world.play_sound_fine(
                Sound::EntityRavagerStunned,
                SoundCategory::Hostile,
                &entity.pos.load(),
                1.0,
                1.0,
            );
            world.send_entity_status(entity, EntityStatus::RavagerStunned, None);
            let attacker: &dyn EntityBase = self;
            defender.push(attacker);
        } else {
            self.strong_knockback(defender.get_entity());
        }
    }
}

impl PatrollingMonster for RavagerEntity {
    fn get_patrol_data(&self) -> &PatrolData {
        &self.raider_data.patrol_data
    }

    fn can_be_leader(&self) -> bool {
        false
    }
}

impl Raider for RavagerEntity {
    fn get_raider_data(&self) -> &RaiderData {
        &self.raider_data
    }

    fn get_celebrate_sound(&self) -> Sound {
        Sound::EntityRavagerCelebrate
    }
}
