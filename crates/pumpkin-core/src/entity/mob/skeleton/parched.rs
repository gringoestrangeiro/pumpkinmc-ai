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

pub struct ParchedSkeletonEntity {
    entity: Arc<SkeletonEntityBase>,
}

impl ParchedSkeletonEntity {
    pub fn new(entity: Entity) -> Arc<Self> {
        let entity = SkeletonEntityBase::new(entity);
        let parched = Self { entity };
        Arc::new(parched)
    }
}

impl Mob for ParchedSkeletonEntity {
    fn get_mob_entity(&self) -> &MobEntity {
        &self.entity.mob_entity
    }

    fn get_ambient_sound(&self) -> Option<Sound> {
        Some(Sound::EntityParchedAmbient)
    }

    fn get_min_attack_interval(&self) -> Option<i32> {
        let world = self.get_entity().world.load();
        if world.get_difficulty() == pumpkin_util::Difficulty::Hard {
            Some(50)
        } else {
            Some(70)
        }
    }

    fn get_arrow_projectile(&self) -> Option<ItemStack> {
        // Vanilla `Parched.getArrow`: weakness 600 on the fired arrow.
        let mut arrow = ItemStack::new(1, &Item::ARROW);
        arrow.set_data_component(PotionContentsImpl {
            potion_id: None,
            custom_color: None,
            custom_effects: vec![StatusEffectInstance {
                effect_id: Cow::Borrowed("minecraft:weakness"),
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
