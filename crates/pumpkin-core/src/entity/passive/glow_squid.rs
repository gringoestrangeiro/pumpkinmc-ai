use std::sync::Arc;
use std::sync::atomic::AtomicI32;

use crate::entity::{
    Entity, EntityBase,
    breath::MAX_AIR,
    mob::{Mob, MobEntity},
    passive::squid::{SquidMovement, tick_squid_air_supply},
};

/// Represents a Glow Squid, a passive aquatic mob that emits a glowing particle effect.
///
/// Wiki: <https://minecraft.wiki/w/Glow_Squid>
pub struct GlowSquidEntity {
    pub mob_entity: MobEntity,
    pub movement: SquidMovement,
    /// Vanilla `AgeableWaterCreature` air supply, drained while out of water.
    pub air_supply: AtomicI32,
}

impl GlowSquidEntity {
    pub fn new(entity: Entity) -> Arc<Self> {
        Arc::new(Self {
            mob_entity: MobEntity::new(entity),
            movement: SquidMovement::new(),
            air_supply: AtomicI32::new(MAX_AIR),
        })
    }
}

impl Mob for GlowSquidEntity {
    fn get_mob_entity(&self) -> &MobEntity {
        &self.mob_entity
    }

    fn mob_tick(&self, caller: &dyn EntityBase) {
        tick_squid_air_supply(caller, &self.air_supply);
        self.movement.tick(self);
    }

    fn custom_travel(&self, caller: &dyn EntityBase) -> bool {
        SquidMovement::travel(self, caller)
    }

    fn mob_is_pushed_by_fluids(&self) -> bool {
        false
    }
}
