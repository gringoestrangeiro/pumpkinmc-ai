use super::spawn::{RANDOM_SPAWN_BONUS_ID, SpawnGroupData, ZombieGroupData};
use super::{Mob, MobEntity};
use crate::entity::ai::goal::active_target::DEFAULT_RANDOM_INTERVAL;
use crate::entity::ai::goal::break_door::BreakDoorGoal;
use crate::entity::ai::goal::destroy_egg::DestroyEggGoal;
use crate::entity::ai::goal::look_around::RandomLookAroundGoal;
use crate::entity::ai::goal::move_through_village::MoveThroughVillageGoal;
use crate::entity::ai::goal::revenge::RevengeGoal;
use crate::entity::ai::goal::spear_use::SpearUseGoal;
use crate::entity::ai::goal::water_avoiding_random_stroll::WaterAvoidingRandomStrollGoal;
use crate::entity::ai::goal::zombie_attack::ZombieAttackGoal;
use crate::entity::attributes::{Modifier, ModifierOperation};
use crate::entity::living::LivingEntity;
use crate::entity::mob::equipment::RegionalDifficulty;
use crate::entity::passive::turtle::baby_on_land_selector;
use crate::entity::{
    Entity,
    ai::goal::{Goal, active_target::ActiveTargetGoal, look_at_entity::LookAtEntityGoal},
};
use crate::world::World;
use pumpkin_data::attributes::Attributes;
use pumpkin_data::data_component_impl::EquipmentSlot;
use pumpkin_data::entity::EntityType;
use pumpkin_data::item::Item;
use pumpkin_data::item_stack::ItemStack;
use pumpkin_nbt::compound::NbtCompound;
use pumpkin_util::Difficulty;
use rand::RngExt;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Weak};

pub mod drowned;
pub mod husk;
#[allow(clippy::module_inception)]
pub mod zombie;
pub mod zombie_villager;

/// Vanilla's `AbstractVillager` target class: villagers and wandering traders.
pub(crate) fn is_abstract_villager(target: &LivingEntity, _world: &World) -> bool {
    target.entity.entity_type == &EntityType::VILLAGER
        || target.entity.entity_type == &EntityType::WANDERING_TRADER
}

pub struct ZombieEntityBase {
    pub mob_entity: MobEntity,
    pub can_break_doors: AtomicBool,
    pub is_baby: AtomicBool,
}

impl ZombieEntityBase {
    const SPEED_MODIFIER_BABY_ID: &str = "minecraft:baby";
    pub const ZOMBIE_LEADER_CHANCE: f32 = 0.05;
    const BREAK_DOOR_CHANCE: f32 = 0.1;

    pub fn new(entity: Entity) -> Arc<Self> {
        Self::with_can_break_doors(entity, false)
    }

    pub fn with_can_break_doors(entity: Entity, can_break_doors: bool) -> Arc<Self> {
        let zombie = Self::without_behaviour_goals(entity, can_break_doors);
        zombie.add_behaviour_goals();
        zombie
    }

    /// Vanilla `Zombie.registerGoals` minus `addBehaviourGoals`, which `Drowned` replaces.
    pub(super) fn without_behaviour_goals(entity: Entity, can_break_doors: bool) -> Arc<Self> {
        let mob_entity = MobEntity::new(entity);
        let zombie = Self {
            mob_entity,
            can_break_doors: AtomicBool::new(can_break_doors),
            is_baby: AtomicBool::new(false),
        };
        let mob_arc = Arc::new(zombie);
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

            if can_break_doors {
                goal_selector.add_goal(1, Box::new(BreakDoorGoal::default()));
            }
            goal_selector.add_goal(4, DestroyEggGoal::new(1.0, 3));
            goal_selector.add_goal(
                8,
                LookAtEntityGoal::with_default(mob_weak, &EntityType::PLAYER, 8.0),
            );
            goal_selector.add_goal(8, Box::new(RandomLookAroundGoal::default()));
        };

        mob_arc
    }

    /// Vanilla `Zombie.addBehaviourGoals`.
    fn add_behaviour_goals(&self) {
        let mut goal_selector = self
            .mob_entity
            .goals_selector
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let mut target_selector = self
            .mob_entity
            .target_selector
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);

        goal_selector.add_goal(2, Box::new(SpearUseGoal::new()));
        goal_selector.add_goal(3, ZombieAttackGoal::new(1.0, false));
        goal_selector.add_goal(6, Box::new(MoveThroughVillageGoal::new(1.0, true, 4.0)));
        goal_selector.add_goal(7, Box::new(WaterAvoidingRandomStrollGoal::new(1.0)));

        target_selector.add_goal(
            1,
            Box::new(
                RevengeGoal::new(true).alerting_others_except(|entity_type| {
                    entity_type == &EntityType::ZOMBIFIED_PIGLIN
                }),
            ),
        );
        target_selector.add_goal(
            2,
            ActiveTargetGoal::with_default(&self.mob_entity, &EntityType::PLAYER, true),
        );
        target_selector.add_goal(
            3,
            ActiveTargetGoal::predicated(
                &self.mob_entity,
                DEFAULT_RANDOM_INTERVAL,
                false,
                is_abstract_villager,
            ),
        );
        target_selector.add_goal(
            3,
            ActiveTargetGoal::with_default(&self.mob_entity, &EntityType::IRON_GOLEM, true),
        );
        target_selector.add_goal(
            5,
            Box::new(ActiveTargetGoal::new(
                &self.mob_entity,
                &EntityType::TURTLE,
                DEFAULT_RANDOM_INTERVAL,
                true,
                false,
                Some(baby_on_land_selector),
            )),
        );
    }

    #[must_use]
    pub fn can_break_doors(&self) -> bool {
        self.can_break_doors.load(Ordering::Relaxed)
    }

    pub fn set_can_break_doors(&self, can_break_doors: bool, mob: &dyn Mob) {
        if self
            .can_break_doors
            .swap(can_break_doors, Ordering::Relaxed)
            != can_break_doors
        {
            let mut stopped = {
                let mut goal_selector = self
                    .mob_entity
                    .goals_selector
                    .lock()
                    .unwrap_or_else(std::sync::PoisonError::into_inner);
                if can_break_doors {
                    goal_selector.add_goal(1, Box::new(BreakDoorGoal::default()));
                    Vec::new()
                } else {
                    goal_selector.remove_goals::<BreakDoorGoal>()
                }
            };
            for goal in &mut stopped {
                goal.stop(mob);
            }
        }
    }
}

impl ZombieEntityBase {
    /// Vanilla `Zombie.setBaby`: the baby state is only the synced flag, it never ages up.
    pub fn set_baby(&self, baby: bool) {
        self.mob_entity.set_baby_flag(
            &self.is_baby,
            pumpkin_data::tracked_data::zombie::BABY,
            baby,
        );
        // Vanilla `Zombie.setBaby` swaps the transient `SPEED_MODIFIER_BABY` modifier.
        self.mob_entity
            .living_entity
            .update_attribute(&Attributes::MOVEMENT_SPEED, |speed| {
                speed.remove_modifier(Self::SPEED_MODIFIER_BABY_ID);
                if baby {
                    speed.add_or_replace_modifier(Modifier {
                        id: Self::SPEED_MODIFIER_BABY_ID.to_string(),
                        amount: 0.5,
                        operation: ModifierOperation::MultiplyBase,
                    });
                }
            });
    }

    #[must_use]
    pub fn is_baby(&self) -> bool {
        self.is_baby.load(Ordering::Relaxed)
    }

    /// Vanilla `Zombie.getSpawnAsBabyOdds`.
    #[must_use]
    pub fn get_spawn_as_baby_odds() -> bool {
        rand::random::<f32>() < 0.05
    }

    /// Vanilla `Zombie.finalizeSpawn` for the zombie-family mob `mob` built on this base.
    /// Equipment and `canPickUpLoot` come from the spawn equipment registry instead.
    pub fn finalize_zombie_spawn(
        &self,
        mob: &dyn Mob,
        world: &Arc<World>,
        group_data: Option<SpawnGroupData>,
    ) -> Option<SpawnGroupData> {
        self.mob_entity.finalize_spawn_base();
        let difficulty_modifier =
            RegionalDifficulty::at(world, self.mob_entity.living_entity.entity.pos.load())
                .special_multiplier;
        let group_data = group_data.unwrap_or_else(|| {
            SpawnGroupData::Zombie(ZombieGroupData {
                is_baby: Self::get_spawn_as_baby_odds(),
                can_spawn_jockey: true,
            })
        });
        if let SpawnGroupData::Zombie(zombie_data) = &group_data {
            if zombie_data.is_baby {
                self.set_baby(true);
            }
            self.set_can_break_doors(
                rand::random::<f32>() < difficulty_modifier * Self::BREAK_DOOR_CHANCE,
                mob,
            );
        }
        self.mob_entity.maybe_wear_halloween_head();
        // Vanilla `Zombie.randomizeReinforcementsChance`.
        let reinforcements_chance = rand::random::<f64>() * 0.1;
        if handle_attributes(&self.mob_entity, difficulty_modifier, reinforcements_chance) {
            self.set_can_break_doors(true, mob);
        }
        Some(group_data)
    }

    #[must_use]
    pub fn get_base_experience_reward(&self) -> u32 {
        let xp = self
            .mob_entity
            .living_entity
            .entity
            .entity_type
            .experience_reward;
        if self.is_baby() {
            // Vanilla `Zombie.getBaseExperienceReward`: babies are worth 2.5x.
            (f64::from(xp) * 2.5) as u32
        } else {
            xp
        }
    }
}

/// Vanilla `Zombie.handleAttributes`, given the subclass's `randomizeReinforcementsChance`
/// roll. Returns whether the zombie rolled the leader bonus, which also lets vanilla zombies
/// break doors.
pub fn handle_attributes(
    mob_entity: &MobEntity,
    difficulty_modifier: f32,
    reinforcements_chance: f64,
) -> bool {
    const LEADER_ZOMBIE_BONUS_ID: &str = "minecraft:leader_zombie_bonus";
    const ZOMBIE_RANDOM_SPAWN_BONUS_ID: &str = "minecraft:zombie_random_spawn_bonus";

    let living = &mob_entity.living_entity;
    let mut rng = rand::rng();
    living.set_attribute_base(&Attributes::SPAWN_REINFORCEMENTS, reinforcements_chance);
    living.update_attribute(&Attributes::KNOCKBACK_RESISTANCE, |knockback_resistance| {
        knockback_resistance.add_or_replace_modifier(Modifier {
            id: RANDOM_SPAWN_BONUS_ID.to_string(),
            amount: rng.random::<f64>() * 0.05,
            operation: ModifierOperation::Add,
        });
    });
    let follow_range_modifier = rng.random::<f64>() * 1.5 * f64::from(difficulty_modifier);
    if follow_range_modifier > 1.0 {
        living.update_attribute(&Attributes::FOLLOW_RANGE, |follow_range| {
            follow_range.add_or_replace_modifier(Modifier {
                id: ZOMBIE_RANDOM_SPAWN_BONUS_ID.to_string(),
                amount: follow_range_modifier,
                operation: ModifierOperation::MultiplyTotal,
            });
        });
    }

    if rng.random::<f32>() >= difficulty_modifier * ZombieEntityBase::ZOMBIE_LEADER_CHANCE {
        return false;
    }
    living.update_attribute(&Attributes::SPAWN_REINFORCEMENTS, |spawn_reinforcements| {
        spawn_reinforcements.add_or_replace_modifier(Modifier {
            id: LEADER_ZOMBIE_BONUS_ID.to_string(),
            amount: rng.random::<f64>().mul_add(0.25, 0.5),
            operation: ModifierOperation::Add,
        });
    });
    living.update_attribute(&Attributes::MAX_HEALTH, |max_health| {
        max_health.add_or_replace_modifier(Modifier {
            id: LEADER_ZOMBIE_BONUS_ID.to_string(),
            amount: rng.random::<f64>().mul_add(3.0, 1.0),
            operation: ModifierOperation::MultiplyTotal,
        });
    });
    living.set_health(living.get_max_health());
    true
}

impl Mob for ZombieEntityBase {
    fn get_mob_entity(&self) -> &MobEntity {
        &self.mob_entity
    }

    fn spawn_as_baby(&self) -> bool {
        self.set_baby(true);
        true
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

        let weapon_chance = if difficulty.base_difficulty == Difficulty::Hard {
            0.05f32
        } else {
            0.01f32
        };
        if rand::random::<f32>() < weapon_chance {
            let r = rand::random_range(0..6);
            let weapon_item = match r {
                0 => &Item::IRON_SWORD,
                1 => &Item::IRON_SPEAR,
                _ => &Item::IRON_SHOVEL,
            };
            let living = &self.mob_entity.living_entity;
            let mut equipment = living
                .entity_equipment
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            equipment.put(&EquipmentSlot::MAIN_HAND, ItemStack::new(1, weapon_item));
        }
    }

    fn mob_write_nbt(&self, nbt: &mut NbtCompound) {
        // Vanilla Zombie.addAdditionalSaveData; shared by every zombie-family mob.
        nbt.put_bool("IsBaby", self.is_baby());
        if self.can_break_doors() {
            nbt.put_bool("CanBreakDoors", true);
        }
    }

    fn mob_read_nbt(&self, nbt: &NbtCompound) {
        self.set_baby(nbt.get_bool("IsBaby").unwrap_or(false));
        if let Some(can_break_doors) = nbt.get_bool("CanBreakDoors") {
            self.set_can_break_doors(can_break_doors, self);
        }
    }
}
