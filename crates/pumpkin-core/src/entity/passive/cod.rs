use std::sync::Arc;

use pumpkin_data::sound::Sound;
use pumpkin_nbt::compound::NbtCompound;

use crate::entity::{
    Entity, EntityBase,
    custom_sound::CustomSound,
    mob::{Mob, MobEntity},
    passive::fish::{self, FishData},
};

/// Represents a Cod, a common passive aquatic mob.
///
/// Wiki: <https://minecraft.wiki/w/Cod>
pub struct CodEntity {
    pub mob_entity: MobEntity,
    pub fish_data: FishData,
}

impl CodEntity {
    pub fn new(entity: Entity) -> Arc<Self> {
        let mob_entity = MobEntity::new(entity);
        fish::init(&mob_entity);
        fish::init_schooling(&mob_entity);
        Arc::new(Self {
            mob_entity,
            fish_data: FishData::new(8),
        })
    }
}

impl Mob for CodEntity {
    fn get_mob_entity(&self) -> &MobEntity {
        &self.mob_entity
    }

    fn get_fish_data(&self) -> Option<&FishData> {
        Some(&self.fish_data)
    }

    fn mob_tick(&self, caller: &dyn EntityBase) {
        self.fish_data.tick_air_supply(caller);
        fish::tick_schooling(self);
        fish::flop(self, Sound::EntityCodFlop);
    }

    fn custom_travel(&self, caller: &dyn EntityBase) -> bool {
        fish::travel(self, caller)
    }

    fn mob_is_pushed_by_fluids(&self) -> bool {
        false
    }

    fn check_spawn_obstruction(&self, world: &crate::world::World) -> bool {
        fish::check_spawn_obstruction(self, world)
    }

    fn requires_custom_persistence(&self) -> bool {
        self.fish_data.from_bucket()
    }

    fn remove_when_far_away(&self, _distance_sq: f64) -> bool {
        !self.fish_data.from_bucket()
    }

    fn set_from_bucket(&self, from_bucket: bool) {
        self.fish_data
            .set_from_bucket(self.get_entity(), from_bucket);
    }

    fn mob_init_data_tracker(&self) {
        fish::init_data(&self.fish_data, self.get_entity());
    }

    fn mob_write_nbt(&self, nbt: &mut NbtCompound) {
        fish::write_data(&self.fish_data, nbt);
    }

    fn mob_read_nbt(&self, nbt: &NbtCompound) {
        fish::read_data(&self.fish_data, self.get_entity(), nbt);
    }

    fn get_ambient_sound(&self) -> Option<Sound> {
        Some(Sound::EntityCodAmbient)
    }

    fn get_ambient_sound_interval(&self) -> i32 {
        120
    }

    fn as_custom_sound(&self) -> Option<&dyn CustomSound> {
        Some(self)
    }
}

impl CustomSound for CodEntity {
    fn death_sound(&self) -> Option<Sound> {
        Some(Sound::EntityCodDeath)
    }

    fn hurt_sound(&self) -> Option<Sound> {
        Some(Sound::EntityCodHurt)
    }
}
