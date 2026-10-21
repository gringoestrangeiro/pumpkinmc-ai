use std::sync::{
    Arc,
    atomic::{AtomicI32, Ordering},
};

use pumpkin_data::sound::Sound;
use pumpkin_data::tracked_data;
use pumpkin_nbt::compound::NbtCompound;
use pumpkin_protocol::codec::var_int::VarInt;
use pumpkin_util::math::boundingbox::{BoundingBox, EntityDimensions};

use crate::entity::{
    Entity, EntityBase,
    custom_sound::CustomSound,
    mob::{Mob, MobEntity, spawn::SpawnGroupData},
    passive::fish::{self, FishData},
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[repr(i32)]
pub enum SalmonSize {
    Small = 0,
    #[default]
    Medium = 1,
    Large = 2,
}

impl SalmonSize {
    #[must_use]
    pub fn from_name(name: &str) -> Option<Self> {
        match name.strip_prefix("minecraft:").unwrap_or(name) {
            "small" => Some(Self::Small),
            "medium" => Some(Self::Medium),
            "large" => Some(Self::Large),
            _ => None,
        }
    }

    #[must_use]
    pub const fn from_id(id: i32) -> Self {
        match id {
            i32::MIN..=0 => Self::Small,
            1 => Self::Medium,
            _ => Self::Large,
        }
    }

    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::Small => "small",
            Self::Medium => "medium",
            Self::Large => "large",
        }
    }

    #[must_use]
    pub const fn bounding_box_scale(self) -> f32 {
        match self {
            Self::Small => 0.5,
            Self::Medium => 1.0,
            Self::Large => 1.5,
        }
    }
}

/// Represents a Salmon, a passive fish of rivers and cold oceans.
///
/// Wiki: <https://minecraft.wiki/w/Salmon>
pub struct SalmonEntity {
    pub mob_entity: MobEntity,
    pub fish_data: FishData,
    pub size: AtomicI32,
}

impl SalmonEntity {
    pub fn new(entity: Entity) -> Arc<Self> {
        let mob_entity = MobEntity::new(entity);
        fish::init(&mob_entity);
        fish::init_schooling(&mob_entity);
        let salmon = Self {
            mob_entity,
            fish_data: FishData::new(5),
            size: AtomicI32::new(SalmonSize::Medium as i32),
        };
        let mob_arc = Arc::new(salmon);
        mob_arc.refresh_dimensions();
        mob_arc
    }

    #[must_use]
    pub fn get_variant(&self) -> SalmonSize {
        SalmonSize::from_id(self.size.load(Ordering::Relaxed))
    }

    pub fn set_variant(&self, variant: SalmonSize) {
        self.size.store(variant as i32, Ordering::Relaxed);
        self.get_entity()
            .set_synced_data(tracked_data::salmon::DATA_TYPE, VarInt(variant as i32));
        self.refresh_dimensions();
    }

    fn refresh_dimensions(&self) {
        let entity = self.get_entity();
        let scale = self.get_variant().bounding_box_scale();
        let dimensions = EntityDimensions {
            width: entity.entity_type.dimension[0] * scale,
            height: entity.entity_type.dimension[1] * scale,
            eye_height: entity.entity_type.eye_height * scale,
        };
        let pos = entity.pos.load();
        entity
            .bounding_box
            .store(BoundingBox::new_from_pos(pos.x, pos.y, pos.z, &dimensions));
        entity.entity_dimension.store(dimensions);
    }
}

impl Mob for SalmonEntity {
    fn get_mob_entity(&self) -> &MobEntity {
        &self.mob_entity
    }

    fn get_fish_data(&self) -> Option<&FishData> {
        Some(&self.fish_data)
    }

    fn mob_tick(&self, caller: &dyn EntityBase) {
        self.fish_data.tick_air_supply(caller);
        fish::tick_schooling(self);
        fish::flop(self, Sound::EntitySalmonFlop);
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

    fn finalize_spawn(
        &self,
        _world: &Arc<crate::world::World>,
        group_data: Option<SpawnGroupData>,
    ) -> Option<SpawnGroupData> {
        self.mob_entity.finalize_spawn_base();
        let roll = rand::random_range(0..95);
        let size = if roll < 30 {
            SalmonSize::Small
        } else if roll < 80 {
            SalmonSize::Medium
        } else {
            SalmonSize::Large
        };
        self.set_variant(size);
        group_data
    }

    fn mob_init_data_tracker(&self) {
        fish::init_data(&self.fish_data, self.get_entity());
        self.get_entity().set_synced_data(
            tracked_data::salmon::DATA_TYPE,
            VarInt(self.size.load(Ordering::Relaxed)),
        );
    }

    fn mob_set_variant_name(&self, name: &str) {
        if let Some(size) = SalmonSize::from_name(name) {
            self.set_variant(size);
        }
    }

    fn mob_write_nbt(&self, nbt: &mut NbtCompound) {
        fish::write_data(&self.fish_data, nbt);
        nbt.put_string("type", self.get_variant().name().to_string());
    }

    fn mob_read_nbt(&self, nbt: &NbtCompound) {
        fish::read_data(&self.fish_data, self.get_entity(), nbt);
        if let Some(name) = nbt.get_string("type") {
            self.set_variant(SalmonSize::from_name(name).unwrap_or_default());
        }
    }

    fn get_ambient_sound(&self) -> Option<Sound> {
        Some(Sound::EntitySalmonAmbient)
    }

    fn get_ambient_sound_interval(&self) -> i32 {
        120
    }

    fn as_custom_sound(&self) -> Option<&dyn CustomSound> {
        Some(self)
    }
}

impl CustomSound for SalmonEntity {
    fn death_sound(&self) -> Option<Sound> {
        Some(Sound::EntitySalmonDeath)
    }

    fn hurt_sound(&self) -> Option<Sound> {
        Some(Sound::EntitySalmonHurt)
    }
}
