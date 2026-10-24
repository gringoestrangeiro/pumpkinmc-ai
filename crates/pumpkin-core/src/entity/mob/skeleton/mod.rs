use std::sync::{Arc, Weak};

use pumpkin_data::data_component_impl::EquipmentSlot;
use pumpkin_data::entity::EntityType;
use pumpkin_data::item::Item;
use pumpkin_data::item_stack::ItemStack;
use pumpkin_util::Difficulty;

use crate::entity::{
    Entity,
    ai::goal::{
        active_target::{ActiveTargetGoal, DEFAULT_RANDOM_INTERVAL},
        avoid_entity::AvoidEntityGoal,
        bow_attack::BowAttackGoal,
        flee_sun::FleeSunGoal,
        look_around::RandomLookAroundGoal,
        look_at_entity::LookAtEntityGoal,
        melee_attack::MeleeAttackGoal,
        restrict_sun::RestrictSunGoal,
        revenge::RevengeGoal,
        water_avoiding_random_stroll::WaterAvoidingRandomStrollGoal,
    },
    mob::{Mob, MobEntity, equipment::RegionalDifficulty, spawn::SpawnGroupData},
    passive::turtle::baby_on_land_selector,
};
use crate::world::World;

pub mod bogged;
pub mod parched;
#[allow(clippy::module_inception)]
pub mod skeleton;
pub mod stray;
pub mod wither;

pub struct SkeletonEntityBase {
    pub mob_entity: MobEntity,
}

impl SkeletonEntityBase {
    /// Vanilla `AbstractSkeleton.getHardAttackInterval`.
    const HARD_ATTACK_INTERVAL: i32 = 20;
    /// Vanilla `AbstractSkeleton.getAttackInterval`.
    const ATTACK_INTERVAL: i32 = 40;

    pub fn new(entity: Entity) -> Arc<Self> {
        let mob_entity = MobEntity::new(entity);
        let mob = Self { mob_entity };
        let mob_arc = Arc::new(mob);
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
            let mut target_selector = mob_arc
                .mob_entity
                .target_selector
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);

            goal_selector.add_goal(2, Box::new(RestrictSunGoal::new()));
            goal_selector.add_goal(3, Box::new(FleeSunGoal::new(1.0)));
            goal_selector.add_goal(
                3,
                Box::new(AvoidEntityGoal::new(&EntityType::WOLF, 6.0, 1.0, 1.2)),
            );
            goal_selector.add_goal(
                4,
                Box::new(
                    MeleeAttackGoal::new(1.2, false)
                        .gated_by(|mob| !BowAttackGoal::is_holding_bow(mob)),
                ),
            );
            goal_selector.add_goal(
                4,
                Box::new(BowAttackGoal::new(1.0, Self::HARD_ATTACK_INTERVAL, 15.0)),
            );
            goal_selector.add_goal(5, Box::new(WaterAvoidingRandomStrollGoal::new(1.0)));
            goal_selector.add_goal(
                6,
                LookAtEntityGoal::with_default(mob_weak, &EntityType::PLAYER, 8.0),
            );
            goal_selector.add_goal(6, Box::new(RandomLookAroundGoal::default()));

            target_selector.add_goal(1, Box::new(RevengeGoal::new(true)));
            target_selector.add_goal(
                2,
                ActiveTargetGoal::with_default(&mob_arc.mob_entity, &EntityType::PLAYER, true),
            );
            target_selector.add_goal(
                3,
                ActiveTargetGoal::with_default(&mob_arc.mob_entity, &EntityType::IRON_GOLEM, true),
            );
            target_selector.add_goal(
                3,
                Box::new(ActiveTargetGoal::new(
                    &mob_arc.mob_entity,
                    &EntityType::TURTLE,
                    DEFAULT_RANDOM_INTERVAL,
                    true,
                    false,
                    Some(baby_on_land_selector),
                )),
            );
        };

        mob_arc
    }

    /// The bow interval vanilla `AbstractSkeleton.reassessWeaponGoal` picks: the hard one only
    /// on Hard difficulty.
    #[must_use]
    pub fn min_attack_interval(&self) -> i32 {
        if self
            .mob_entity
            .living_entity
            .entity
            .world
            .load()
            .get_difficulty()
            == Difficulty::Hard
        {
            Self::HARD_ATTACK_INTERVAL
        } else {
            Self::ATTACK_INTERVAL
        }
    }

    /// Vanilla `AbstractSkeleton.finalizeSpawn`. Equipment and `canPickUpLoot` come from the
    /// spawn equipment registry instead.
    pub fn finalize_skeleton_spawn(
        &self,
        group_data: Option<SpawnGroupData>,
    ) -> Option<SpawnGroupData> {
        self.mob_entity.finalize_spawn_base();
        self.mob_entity.maybe_wear_halloween_head();
        group_data
    }
}

impl Mob for SkeletonEntityBase {
    fn get_mob_entity(&self) -> &MobEntity {
        &self.mob_entity
    }

    fn populate_default_equipment_slots(
        &self,
        _world: &Arc<World>,
        difficulty: &RegionalDifficulty,
    ) {
        // Default armor slots (super.populateDefaultEquipmentSlots)
        if rand::random::<f32>()
            < MobEntity::MAX_WEARING_ARMOR_CHANCE * difficulty.special_multiplier
        {
            let mut armor_type = rand::random_range(0..3);
            for _ in 1..=3 {
                if rand::random::<f32>() < MobEntity::WEARING_ARMOR_UPGRADE_MATERIAL_CHANCE {
                    armor_type += 1;
                }
            }

            let partial_chance = if difficulty.base_difficulty == Difficulty::Hard {
                0.1f32
            } else {
                0.25f32
            };

            let living = &self.mob_entity.living_entity;
            let mut equipment = living
                .entity_equipment
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            let mut first = true;

            for slot in &MobEntity::EQUIPMENT_POPULATION_ORDER {
                let current = equipment.get(slot);
                if !first && rand::random::<f32>() < partial_chance {
                    break;
                }
                first = false;
                if current.is_empty()
                    && let Some(item) = MobEntity::get_equipment_for_slot(slot, armor_type)
                {
                    equipment.put(slot, ItemStack::new(1, item));
                }
            }
        }

        // AbstractSkeleton sets BOW on MAIN_HAND
        let living = &self.mob_entity.living_entity;
        let mut equipment = living
            .entity_equipment
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        equipment.put(&EquipmentSlot::MAIN_HAND, ItemStack::new(1, &Item::BOW));
    }
}
