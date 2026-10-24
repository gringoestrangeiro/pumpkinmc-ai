use pumpkin_data::data_component_impl::{PotionContentsImpl, StatusEffectInstance};
use pumpkin_data::item::Item;
use pumpkin_data::item_stack::ItemStack;
use pumpkin_data::sound::Sound;
use std::borrow::Cow;
use std::sync::Arc;

use crate::entity::{
    Entity,
    mob::{Mob, MobEntity, skeleton::SkeletonEntityBase},
};

pub struct StraySkeletonEntity {
    entity: Arc<SkeletonEntityBase>,
}

impl StraySkeletonEntity {
    pub fn new(entity: Entity) -> Arc<Self> {
        let entity = SkeletonEntityBase::new(entity);
        let stray = Self { entity };
        Arc::new(stray)
    }
}

impl Mob for StraySkeletonEntity {
    fn get_mob_entity(&self) -> &MobEntity {
        &self.entity.mob_entity
    }

    fn finalize_spawn(
        &self,
        _world: &Arc<crate::world::World>,
        group_data: Option<crate::entity::mob::spawn::SpawnGroupData>,
    ) -> Option<crate::entity::mob::spawn::SpawnGroupData> {
        self.entity.finalize_skeleton_spawn(group_data)
    }

    fn get_ambient_sound(&self) -> Option<Sound> {
        Some(Sound::EntityStrayAmbient)
    }

    fn get_min_attack_interval(&self) -> Option<i32> {
        Some(self.entity.min_attack_interval())
    }

    fn get_arrow_projectile(&self) -> Option<ItemStack> {
        // Vanilla `Stray.getArrow`: slowness 600 on the fired arrow.
        let mut arrow = ItemStack::new(1, &Item::ARROW);
        arrow.set_data_component(PotionContentsImpl {
            potion_id: None,
            custom_color: None,
            custom_effects: vec![StatusEffectInstance {
                effect_id: Cow::Borrowed("minecraft:slowness"),
                amplifier: 0,
                duration: 600,
                ambient: false,
                show_particles: true,
                show_icon: true,
            }],
            custom_name: None,
        });
        Some(arrow)
    }
}
