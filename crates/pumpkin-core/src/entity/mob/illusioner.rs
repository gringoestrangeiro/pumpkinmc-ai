use std::sync::atomic::{AtomicI32, AtomicU8, Ordering};
use std::sync::{Arc, Weak};

use pumpkin_data::data_component_impl::EquipmentSlot;
use pumpkin_data::effect::StatusEffect;
use pumpkin_data::entity::EntityType;
use pumpkin_data::item::Item;
use pumpkin_data::item_stack::ItemStack;
use pumpkin_data::potion::Effect;
use pumpkin_data::sound::Sound;
use pumpkin_data::tag::{self, Taggable};
use pumpkin_nbt::compound::NbtCompound;

use crate::entity::{
    Entity, EntityBase,
    ai::goal::{
        Controls, Goal, active_target::ActiveTargetGoal, avoid_entity::AvoidEntityGoal,
        bow_attack::BowAttackGoal, look_around::RandomLookAroundGoal,
        look_at_entity::LookAtEntityGoal, revenge::RevengeGoal, swim::SwimGoal,
        wander_around::WanderAroundGoal,
    },
    mob::{
        Mob, MobEntity,
        equipment::RegionalDifficulty,
        evoker::IllagerSpell,
        patrol::{LongDistancePatrolGoal, PatrolData, PatrollingMonster},
        raider::{
            HoldGroundAttackGoal, ObtainRaidLeaderBannerGoal, PathfindToRaidGoal, Raider,
            RaiderCelebrationGoal, RaiderData, RaiderMoveThroughVillageGoal,
        },
    },
};

pub struct IllusionerEntity {
    pub mob_entity: MobEntity,
    pub raider_data: RaiderData,
    spell_casting_tick_count: AtomicI32,
    current_spell: AtomicU8,
}

impl IllusionerEntity {
    #[must_use]
    pub fn new(entity: Entity) -> Arc<Self> {
        let mob_entity = MobEntity::new(entity);
        let illusioner = Self {
            mob_entity,
            raider_data: RaiderData::default(),
            spell_casting_tick_count: AtomicI32::new(0),
            current_spell: AtomicU8::new(IllagerSpell::None as u8),
        };
        let mob_arc = Arc::new(illusioner);
        let mob_weak: Weak<dyn Mob> = {
            let mob_arc: Arc<dyn Mob> = mob_arc.clone();
            Arc::downgrade(&mob_arc)
        };
        let illusioner_weak: Weak<Self> = Arc::downgrade(&mob_arc);

        // Vanilla `Illusioner.finalizeSpawn` equips a bow before the entity is tracked.
        mob_arc
            .mob_entity
            .living_entity
            .entity_equipment
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .put(&EquipmentSlot::MAIN_HAND, ItemStack::new(1, &Item::BOW));

        {
            let mut goal_selector = mob_arc
                .mob_entity
                .goals_selector
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);

            goal_selector.add_goal(0, Box::new(SwimGoal::default()));
            goal_selector.add_goal(1, Box::new(ObtainRaidLeaderBannerGoal));
            goal_selector.add_goal(
                1,
                Box::new(IllusionerCastingSpellGoal::new(illusioner_weak.clone())),
            );
            goal_selector.add_goal(2, Box::new(HoldGroundAttackGoal::new(10.0)));
            goal_selector.add_goal(
                3,
                Box::new(AvoidEntityGoal::new(&EntityType::CREAKING, 8.0, 1.0, 1.2)),
            );
            goal_selector.add_goal(4, Box::new(LongDistancePatrolGoal::new(0.7, 0.595)));
            goal_selector.add_goal(4, Box::new(RaiderMoveThroughVillageGoal::new(1.05)));
            goal_selector.add_goal(4, Box::new(PathfindToRaidGoal::default()));
            goal_selector.add_goal(
                4,
                Box::new(IllusionerMirrorSpellGoal::new(illusioner_weak.clone())),
            );
            goal_selector.add_goal(
                5,
                Box::new(IllusionerBlindnessSpellGoal::new(illusioner_weak)),
            );
            goal_selector.add_goal(5, Box::new(RaiderCelebrationGoal));
            goal_selector.add_goal(6, Box::new(BowAttackGoal::new(0.5, 20, 15.0)));
            goal_selector.add_goal(8, Box::new(WanderAroundGoal::new(0.6)));
            goal_selector.add_goal(
                9,
                LookAtEntityGoal::with_default(mob_weak, &EntityType::PLAYER, 3.0),
            );
            goal_selector.add_goal(10, Box::new(RandomLookAroundGoal::default()));

            let mut target_selector = mob_arc
                .mob_entity
                .target_selector
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            target_selector.add_goal(
                1,
                Box::new(
                    RevengeGoal::new(true)
                        .ignoring(|entity_type| {
                            entity_type.has_tag(&tag::EntityType::MINECRAFT_RAIDERS)
                        })
                        .alerting_others(),
                ),
            );
            target_selector.add_goal(
                2,
                ActiveTargetGoal::with_default(&mob_arc.mob_entity, &EntityType::PLAYER, true),
            );
            target_selector.add_goal(
                3,
                ActiveTargetGoal::with_default(&mob_arc.mob_entity, &EntityType::VILLAGER, true),
            );
            target_selector.add_goal(
                3,
                ActiveTargetGoal::with_default(&mob_arc.mob_entity, &EntityType::IRON_GOLEM, true),
            );
        };

        mob_arc
    }

    #[must_use]
    pub fn is_casting_spell(&self) -> bool {
        self.spell_casting_tick_count.load(Ordering::Relaxed) > 0
    }

    pub fn set_is_casting_spell(&self, spell: IllagerSpell) {
        self.current_spell.store(spell as u8, Ordering::Relaxed);
        let entity = &self.mob_entity.living_entity.entity;
        entity.set_synced_data(
            pumpkin_data::tracked_data::illusioner::SPELL_CASTING_ID,
            spell as u8 as i8,
        );
    }

    #[must_use]
    pub fn get_current_spell(&self) -> IllagerSpell {
        IllagerSpell::from_u8(self.current_spell.load(Ordering::Relaxed))
    }

    #[must_use]
    pub fn get_spell_casting_time(&self) -> i32 {
        self.spell_casting_tick_count.load(Ordering::Relaxed)
    }

    pub fn set_spell_casting_time(&self, ticks: i32) {
        self.spell_casting_tick_count
            .store(ticks, Ordering::Relaxed);
    }
}

impl Mob for IllusionerEntity {
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
        nbt.put_int("SpellTicks", self.get_spell_casting_time());
    }

    fn mob_read_nbt(&self, nbt: &NbtCompound) {
        self.read_raider_nbt(nbt);
        if let Some(ticks) = nbt.get_int("SpellTicks") {
            self.set_spell_casting_time(ticks);
        }
    }

    fn mob_tick(&self, _caller: &dyn EntityBase) {
        let ticks = self.spell_casting_tick_count.load(Ordering::Relaxed);
        if ticks > 0 {
            self.spell_casting_tick_count
                .store(ticks - 1, Ordering::Relaxed);
        }
    }
}

impl PatrollingMonster for IllusionerEntity {
    fn get_patrol_data(&self) -> &PatrolData {
        &self.raider_data.patrol_data
    }
}

impl Raider for IllusionerEntity {
    fn get_raider_data(&self) -> &RaiderData {
        &self.raider_data
    }

    fn get_celebrate_sound(&self) -> Sound {
        Sound::EntityIllusionerAmbient
    }
}

/// Vanilla `SpellcasterIllager.SpellcasterCastingSpellGoal`.
struct IllusionerCastingSpellGoal {
    illusioner: Weak<IllusionerEntity>,
}

impl IllusionerCastingSpellGoal {
    const fn new(illusioner: Weak<IllusionerEntity>) -> Self {
        Self { illusioner }
    }
}

impl Goal for IllusionerCastingSpellGoal {
    fn can_start(&mut self, _mob: &dyn Mob) -> bool {
        self.illusioner
            .upgrade()
            .is_some_and(|illusioner| illusioner.is_casting_spell())
    }

    fn should_continue(&mut self, _mob: &dyn Mob) -> bool {
        self.illusioner
            .upgrade()
            .is_some_and(|illusioner| illusioner.is_casting_spell())
    }

    fn start(&mut self, _mob: &dyn Mob) {
        if let Some(illusioner) = self.illusioner.upgrade() {
            illusioner
                .mob_entity
                .navigator
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .stop();
        }
    }

    fn stop(&mut self, _mob: &dyn Mob) {
        if let Some(illusioner) = self.illusioner.upgrade() {
            illusioner.set_is_casting_spell(IllagerSpell::None);
        }
    }

    fn tick(&mut self, _mob: &dyn Mob) {
        if let Some(illusioner) = self.illusioner.upgrade()
            && let Some(target) = illusioner.mob_entity.get_target()
        {
            illusioner
                .mob_entity
                .look_control
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .look_at_entity_with_range(&target, 30.0, 30.0);
        }
    }

    fn controls(&self) -> Controls {
        Controls::MOVE | Controls::LOOK
    }
}

/// Vanilla `Illusioner.IllusionerMirrorSpellGoal`.
struct IllusionerMirrorSpellGoal {
    illusioner: Weak<IllusionerEntity>,
    warmup_delay: i32,
    next_attack_tick: i32,
}

impl IllusionerMirrorSpellGoal {
    const CASTING_TIME: i32 = 20;
    const CASTING_INTERVAL: i32 = 340;
    const INVISIBILITY_DURATION: i32 = 1200;

    const fn new(illusioner: Weak<IllusionerEntity>) -> Self {
        Self {
            illusioner,
            warmup_delay: 0,
            next_attack_tick: 0,
        }
    }
}

impl Goal for IllusionerMirrorSpellGoal {
    fn can_start(&mut self, _mob: &dyn Mob) -> bool {
        let Some(illusioner) = self.illusioner.upgrade() else {
            return false;
        };
        if illusioner.is_casting_spell()
            || illusioner
                .mob_entity
                .living_entity
                .entity
                .age
                .load(Ordering::Relaxed)
                < self.next_attack_tick
            || illusioner
                .mob_entity
                .living_entity
                .has_effect(&StatusEffect::INVISIBILITY)
        {
            return false;
        }
        illusioner
            .mob_entity
            .get_target()
            .is_some_and(|target| target.get_entity().is_alive())
    }

    fn should_continue(&mut self, _mob: &dyn Mob) -> bool {
        if self.warmup_delay <= 0 {
            return false;
        }
        self.illusioner.upgrade().is_some_and(|illusioner| {
            illusioner
                .mob_entity
                .get_target()
                .is_some_and(|target| target.get_entity().is_alive())
        })
    }

    fn start(&mut self, _mob: &dyn Mob) {
        self.warmup_delay = 20;
        if let Some(illusioner) = self.illusioner.upgrade() {
            illusioner.set_spell_casting_time(Self::CASTING_TIME);
            self.next_attack_tick = illusioner
                .mob_entity
                .living_entity
                .entity
                .age
                .load(Ordering::Relaxed)
                + Self::CASTING_INTERVAL;
            illusioner.set_is_casting_spell(IllagerSpell::Disappear);
            illusioner
                .mob_entity
                .living_entity
                .entity
                .play_sound(Sound::EntityIllusionerPrepareMirror);
        }
    }

    fn tick(&mut self, _mob: &dyn Mob) {
        self.warmup_delay -= 1;
        if self.warmup_delay == 0
            && let Some(illusioner) = self.illusioner.upgrade()
        {
            illusioner.mob_entity.living_entity.add_effect(Effect {
                effect_type: &StatusEffect::INVISIBILITY,
                duration: Self::INVISIBILITY_DURATION,
                amplifier: 0,
                ambient: false,
                show_particles: true,
                show_icon: true,
                blend: false,
            });
            illusioner
                .mob_entity
                .living_entity
                .entity
                .play_sound(Sound::EntityIllusionerCastSpell);
        }
    }
}

/// Vanilla `Illusioner.IllusionerBlindnessSpellGoal`.
struct IllusionerBlindnessSpellGoal {
    illusioner: Weak<IllusionerEntity>,
    warmup_delay: i32,
    next_attack_tick: i32,
    last_target_id: i32,
}

impl IllusionerBlindnessSpellGoal {
    const CASTING_TIME: i32 = 20;
    const CASTING_INTERVAL: i32 = 180;
    const BLINDNESS_DURATION: i32 = 400;

    const fn new(illusioner: Weak<IllusionerEntity>) -> Self {
        Self {
            illusioner,
            warmup_delay: 0,
            next_attack_tick: 0,
            last_target_id: 0,
        }
    }
}

impl Goal for IllusionerBlindnessSpellGoal {
    fn can_start(&mut self, _mob: &dyn Mob) -> bool {
        let Some(illusioner) = self.illusioner.upgrade() else {
            return false;
        };
        if illusioner.is_casting_spell()
            || illusioner
                .mob_entity
                .living_entity
                .entity
                .age
                .load(Ordering::Relaxed)
                < self.next_attack_tick
        {
            return false;
        }
        let Some(target) = illusioner.mob_entity.get_target() else {
            return false;
        };
        if !target.get_entity().is_alive() || target.get_entity().entity_id == self.last_target_id {
            return false;
        }
        let world = illusioner.mob_entity.living_entity.entity.world.load_full();
        RegionalDifficulty::at(
            &world,
            illusioner.mob_entity.living_entity.entity.pos.load(),
        )
        .effective_difficulty
            > 2.0
    }

    fn should_continue(&mut self, _mob: &dyn Mob) -> bool {
        if self.warmup_delay <= 0 {
            return false;
        }
        self.illusioner.upgrade().is_some_and(|illusioner| {
            illusioner
                .mob_entity
                .get_target()
                .is_some_and(|target| target.get_entity().is_alive())
        })
    }

    fn start(&mut self, _mob: &dyn Mob) {
        self.warmup_delay = 20;
        if let Some(illusioner) = self.illusioner.upgrade() {
            illusioner.set_spell_casting_time(Self::CASTING_TIME);
            self.next_attack_tick = illusioner
                .mob_entity
                .living_entity
                .entity
                .age
                .load(Ordering::Relaxed)
                + Self::CASTING_INTERVAL;
            if let Some(target) = illusioner.mob_entity.get_target() {
                self.last_target_id = target.get_entity().entity_id;
            }
            illusioner.set_is_casting_spell(IllagerSpell::Blindness);
            illusioner
                .mob_entity
                .living_entity
                .entity
                .play_sound(Sound::EntityIllusionerPrepareBlindness);
        }
    }

    fn tick(&mut self, _mob: &dyn Mob) {
        self.warmup_delay -= 1;
        if self.warmup_delay == 0
            && let Some(illusioner) = self.illusioner.upgrade()
        {
            if let Some(target) = illusioner.mob_entity.get_target()
                && let Some(living) = target.get_living_entity()
            {
                living.add_effect(Effect {
                    effect_type: &StatusEffect::BLINDNESS,
                    duration: Self::BLINDNESS_DURATION,
                    amplifier: 0,
                    ambient: false,
                    show_particles: true,
                    show_icon: true,
                    blend: false,
                });
            }
            illusioner
                .mob_entity
                .living_entity
                .entity
                .play_sound(Sound::EntityIllusionerCastSpell);
        }
    }
}
