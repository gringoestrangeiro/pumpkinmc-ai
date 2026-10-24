use pumpkin_data::attributes::Attributes;
use pumpkin_data::data_component_impl::EquipmentSlot;
use pumpkin_data::effect::StatusEffect;
use pumpkin_data::entity::EntityType;
use pumpkin_data::item::Item;
use pumpkin_data::item_stack::ItemStack;
use pumpkin_data::potion::Effect;
use pumpkin_data::sound::Sound;
use pumpkin_data::tag::{self, Taggable};
use std::sync::Arc;

use crate::entity::{
    Entity, EntityBase,
    ai::goal::active_target::ActiveTargetGoal,
    mob::{
        Mob, MobEntity, equipment::RegionalDifficulty, skeleton::SkeletonEntityBase,
        spawn::SpawnGroupData,
    },
    projectile::arrow::ArrowEntity,
};
use crate::world::World;

pub struct WitherSkeletonEntity {
    entity: Arc<SkeletonEntityBase>,
}

impl WitherSkeletonEntity {
    pub fn new(entity: Entity) -> Arc<Self> {
        let entity = SkeletonEntityBase::new(entity);
        let skeleton = Self { entity };
        let mob_arc = Arc::new(skeleton);
        {
            let mut target_selector = mob_arc
                .entity
                .mob_entity
                .target_selector
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            // Vanilla `WitherSkeleton.registerGoals`: AbstractPiglin at 3.
            for piglin in [
                &EntityType::PIGLIN,
                &EntityType::PIGLIN_BRUTE,
                &EntityType::ZOMBIFIED_PIGLIN,
            ] {
                target_selector.add_goal(
                    3,
                    ActiveTargetGoal::with_default(&mob_arc.entity.mob_entity, piglin, true),
                );
            }
        }
        mob_arc
    }
}

impl Mob for WitherSkeletonEntity {
    fn get_mob_entity(&self) -> &MobEntity {
        &self.entity.mob_entity
    }

    fn get_min_attack_interval(&self) -> Option<i32> {
        Some(self.entity.min_attack_interval())
    }

    fn on_attack(&self, target: &dyn EntityBase) {
        if let Some(living) = target.get_living_entity() {
            living.add_effect(Effect {
                effect_type: &StatusEffect::WITHER,
                duration: 200,
                amplifier: 0,
                ambient: false,
                show_particles: true,
                show_icon: true,
                blend: false,
            });
        }
    }

    fn get_arrow(&self, arrow: ArrowEntity) -> ArrowEntity {
        // vanilla lights it for 100 seconds
        arrow.set_flame(true);
        arrow
    }

    fn get_ambient_sound(&self) -> Option<Sound> {
        Some(Sound::EntityWitherSkeletonAmbient)
    }

    fn populate_default_equipment_slots(
        &self,
        _world: &Arc<World>,
        _difficulty: &RegionalDifficulty,
    ) {
        // Vanilla `WitherSkeleton.populateDefaultEquipmentSlots`: stone sword only, no armor.
        let living = &self.entity.mob_entity.living_entity;
        living
            .entity_equipment
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .put(
                &EquipmentSlot::MAIN_HAND,
                ItemStack::new(1, &Item::STONE_SWORD),
            );
    }

    fn populate_default_equipment_enchantments(&self, _difficulty: &RegionalDifficulty) {
        // Vanilla `WitherSkeleton.populateDefaultEquipmentEnchantments` is empty.
    }

    fn can_replace_current_item(
        &self,
        new_item: &ItemStack,
        current_item: &ItemStack,
        slot: &EquipmentSlot,
    ) -> bool {
        // Vanilla `WitherSkeleton.canHoldItem` rejects its disliked weapons.
        if new_item
            .item
            .has_tag(&tag::Item::MINECRAFT_WITHER_SKELETON_DISLIKED_WEAPONS)
        {
            return false;
        }
        crate::entity::mob::equipment::can_replace_current_item(
            &self.entity.mob_entity,
            self.get_preferred_weapon_type(),
            new_item,
            current_item,
            slot,
        )
    }

    fn finalize_spawn(
        &self,
        _world: &Arc<World>,
        group_data: Option<SpawnGroupData>,
    ) -> Option<SpawnGroupData> {
        let group_data = self.entity.finalize_skeleton_spawn(group_data);
        self.entity
            .mob_entity
            .living_entity
            .set_attribute_base(&Attributes::ATTACK_DAMAGE, 4.0);
        group_data
    }
}
