use std::sync::Arc;

use pumpkin_data::data_component_impl::EquipmentSlot;
use pumpkin_data::effect::StatusEffect;
use pumpkin_data::potion::Effect;

use crate::entity::mob::equipment::RegionalDifficulty;
use crate::entity::mob::spawn::SpawnGroupData;
use crate::entity::mob::zombie::ZombieEntityBase;
use crate::entity::{
    Entity, EntityBase,
    mob::{Mob, MobEntity},
};
use crate::world::World;
use pumpkin_nbt::compound::NbtCompound;

pub struct HuskEntity {
    entity: Arc<ZombieEntityBase>,
}

impl HuskEntity {
    pub fn new(entity: Entity) -> Arc<Self> {
        let entity = ZombieEntityBase::new(entity);
        let zombie = Self { entity };
        Arc::new(zombie)
    }

    #[must_use]
    pub fn with_can_break_doors(entity: Entity, can_break_doors: bool) -> Arc<Self> {
        let entity = ZombieEntityBase::with_can_break_doors(entity, can_break_doors);
        let zombie = Self { entity };
        Arc::new(zombie)
    }
}

impl Mob for HuskEntity {
    fn get_mob_entity(&self) -> &MobEntity {
        &self.entity.mob_entity
    }

    fn spawn_as_baby(&self) -> bool {
        self.entity.spawn_as_baby()
    }

    fn finalize_spawn(
        &self,
        world: &Arc<World>,
        group_data: Option<SpawnGroupData>,
    ) -> Option<SpawnGroupData> {
        self.entity.finalize_zombie_spawn(self, world, group_data)
    }

    fn get_base_experience_reward(&self) -> u32 {
        self.entity.get_base_experience_reward()
    }

    fn burns_in_daylight(&self) -> bool {
        // Vanilla `Husk.isSunSensitive`.
        false
    }

    fn on_attack(&self, target: &dyn EntityBase) {
        let empty_hand = self
            .entity
            .mob_entity
            .living_entity
            .entity_equipment
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .get(&EquipmentSlot::MAIN_HAND)
            .is_empty();
        if !empty_hand {
            return;
        }
        let Some(living) = target.get_living_entity() else {
            return;
        };
        let world = self.entity.mob_entity.living_entity.entity.world.load();
        let difficulty = RegionalDifficulty::at(
            &world,
            self.entity.mob_entity.living_entity.entity.pos.load(),
        );
        // Vanilla `Husk.doHurtTarget`: 140 ticks per effective difficulty.
        living.add_effect(Effect {
            effect_type: &StatusEffect::HUNGER,
            duration: 140 * difficulty.effective_difficulty as i32,
            amplifier: 0,
            ambient: false,
            show_particles: true,
            show_icon: true,
            blend: false,
        });
    }

    fn mob_write_nbt(&self, nbt: &mut NbtCompound) {
        self.entity.mob_write_nbt(nbt);
    }

    fn mob_read_nbt(&self, nbt: &NbtCompound) {
        self.entity.mob_read_nbt(nbt);
    }
}
