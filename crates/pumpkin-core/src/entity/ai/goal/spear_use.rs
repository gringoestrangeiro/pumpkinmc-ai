use std::sync::Arc;

use pumpkin_data::attributes::Attributes;
use pumpkin_data::damage::DamageType;
use pumpkin_data::data_component_impl::{EquipmentSlot, KineticWeaponImpl};
use pumpkin_data::item_stack::ItemStack;
use pumpkin_util::Hand;
use pumpkin_util::math::vector3::Vector3;

use super::{Controls, Goal};
use crate::entity::EntityBase;
use crate::entity::ai::pathfinder::NavigatorGoal;
use crate::entity::mob::Mob;

const APPROACH_DISTANCE: f64 = 10.0;
const IN_RANGE_DISTANCE: f64 = 2.0;
const FLEE_DURATION: i32 = 100;
const FLEE_DISTANCE: f64 = 7.0;

/// Vanilla `SpearUseGoal`: approach with a kinetic weapon, hold it for its
/// charge duration, stab, then back off while the weapon cools down.
///
/// The goal is used by piglins and zombified piglins, the two goal-based mobs
/// that can spawn with a golden spear.
pub struct SpearUseGoal {
    target: Option<Arc<dyn EntityBase>>,
    charging_ticks: i32,
    fleeing_ticks: i32,
    flee_position: Option<Vector3<f64>>,
    engaged: bool,
}

impl SpearUseGoal {
    #[must_use]
    pub const fn new() -> Self {
        Self {
            target: None,
            charging_ticks: 0,
            fleeing_ticks: 0,
            flee_position: None,
            engaged: false,
        }
    }

    fn main_hand(mob: &dyn Mob) -> ItemStack {
        mob.get_mob_entity()
            .living_entity
            .entity_equipment
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .get(&EquipmentSlot::MAIN_HAND)
    }

    fn kinetic_weapon(mob: &dyn Mob) -> Option<KineticWeaponImpl> {
        Self::main_hand(mob)
            .get_data_component::<KineticWeaponImpl>()
            .cloned()
    }

    fn is_using_item(mob: &dyn Mob) -> bool {
        mob.get_mob_entity()
            .living_entity
            .item_in_use
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .is_some()
    }

    fn stop_using(mob: &dyn Mob) {
        mob.get_mob_entity().living_entity.clear_active_hand();
    }

    fn attack_damage(mob: &dyn Mob) -> f32 {
        mob.get_mob_entity()
            .living_entity
            .get_attribute_value(&Attributes::ATTACK_DAMAGE) as f32
    }

    fn pick_flee_position(mob: &dyn Mob, target: &dyn EntityBase) -> Option<Vector3<f64>> {
        let mob_pos = mob.get_entity().pos.load();
        let target_pos = target.get_entity().pos.load();
        let away = mob_pos - target_pos;
        let horizontal = Vector3::new(away.x, 0.0, away.z);
        let direction = if horizontal.length_squared() > 1.0e-6 {
            horizontal.normalize()
        } else {
            Vector3::new(1.0, 0.0, 0.0)
        };
        let destination = mob_pos + direction.multiply(FLEE_DISTANCE, 0.0, FLEE_DISTANCE);
        let world = mob.get_entity().world.load();
        let block_pos = pumpkin_util::math::position::BlockPos::floored(
            destination.x,
            destination.y,
            destination.z,
        );
        world
            .get_block_state(&block_pos)
            .is_air()
            .then_some(destination)
    }

    fn move_to(&self, mob: &dyn Mob, destination: Vector3<f64>, speed: f64) {
        let mob_pos = mob.get_entity().pos.load();
        mob.get_mob_entity()
            .navigator
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .set_progress(NavigatorGoal::new(mob_pos, destination, speed));
    }
}

impl Default for SpearUseGoal {
    fn default() -> Self {
        Self::new()
    }
}

impl Goal for SpearUseGoal {
    fn can_start(&mut self, mob: &dyn Mob) -> bool {
        let Some(target) = mob.get_mob_entity().get_target() else {
            return false;
        };
        if !target.get_entity().is_alive()
            || Self::kinetic_weapon(mob).is_none()
            || Self::is_using_item(mob)
        {
            return false;
        }
        self.target = Some(target);
        self.charging_ticks = 0;
        self.fleeing_ticks = 0;
        self.flee_position = None;
        self.engaged = false;
        true
    }

    fn should_continue(&mut self, mob: &dyn Mob) -> bool {
        let Some(target) = &self.target else {
            return false;
        };
        target.get_entity().is_alive()
            && Self::kinetic_weapon(mob).is_some()
            && (self.fleeing_ticks > 0 || self.engaged || self.charging_ticks > 0)
    }

    fn start(&mut self, mob: &dyn Mob) {
        mob.get_mob_entity().set_attacking(true);
    }

    fn stop(&mut self, mob: &dyn Mob) {
        Self::stop_using(mob);
        mob.get_mob_entity().set_attacking(false);
        mob.get_mob_entity()
            .navigator
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .stop();
        self.target = None;
        self.charging_ticks = 0;
        self.fleeing_ticks = 0;
        self.flee_position = None;
        self.engaged = false;
    }

    fn tick(&mut self, mob: &dyn Mob) {
        let Some(target) = self.target.clone() else {
            return;
        };
        let mob_pos = mob.get_entity().pos.load();
        let target_pos = target.get_entity().pos.load();
        let distance = mob_pos.squared_distance_to_vec(&target_pos).sqrt();

        if self.fleeing_ticks > 0 {
            self.fleeing_ticks -= 1;
            if let Some(position) = self.flee_position {
                if mob.is_navigator_idle() {
                    self.move_to(mob, position, 1.0);
                }
            } else if let Some(position) = Self::pick_flee_position(mob, target.as_ref()) {
                self.flee_position = Some(position);
                self.move_to(mob, position, 1.0);
            }
            return;
        }

        if self.engaged {
            self.charging_ticks -= 1;
            if self.charging_ticks <= 0 {
                Self::stop_using(mob);
                target.damage(
                    mob.get_entity(),
                    Self::attack_damage(mob),
                    DamageType::SPEAR,
                );
                let away = mob_pos - target_pos;
                let horizontal = Vector3::new(away.x, 0.0, away.z);
                if horizontal.length_squared() > 1.0e-6 {
                    let direction = horizontal.normalize();
                    target
                        .get_entity()
                        .apply_knockback(0.5, direction.x, direction.z);
                    target.get_entity().send_velocity();
                }
                self.engaged = false;
                self.fleeing_ticks = FLEE_DURATION;
                self.flee_position = None;
            }
            return;
        }

        if distance > APPROACH_DISTANCE {
            if mob.is_navigator_idle() {
                self.move_to(mob, target_pos, 1.0);
            }
            return;
        }

        if distance <= IN_RANGE_DISTANCE && !Self::is_using_item(mob) {
            let weapon = Self::kinetic_weapon(mob);
            let stack = Self::main_hand(mob);
            let duration = weapon.map_or(20, |weapon| weapon.delay_ticks.max(1));
            mob.get_mob_entity()
                .living_entity
                .set_active_hand(Hand::Right, stack, duration);
            self.charging_ticks = duration;
            self.engaged = true;
            return;
        }

        if mob.is_navigator_idle() {
            self.move_to(mob, target_pos, 1.0);
        }
    }

    fn controls(&self) -> Controls {
        Controls::MOVE | Controls::LOOK
    }
}
