use std::sync::{
    Arc,
    atomic::{AtomicI32, Ordering},
};

use pumpkin_data::dye_color::DyeColor;
use pumpkin_data::sound::Sound;
use pumpkin_data::tracked_data;
use pumpkin_nbt::compound::NbtCompound;
use pumpkin_protocol::codec::var_int::VarInt;

use crate::entity::{
    Entity, EntityBase,
    custom_sound::CustomSound,
    mob::{Mob, MobEntity, spawn::SpawnGroupData},
    passive::fish::{self, FishData},
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum TropicalFishPattern {
    #[default]
    Kob,
    Sunstreak,
    Snooper,
    Dasher,
    Brinely,
    Spotty,
    Flopper,
    Stripey,
    Glitter,
    Blockfish,
    Betty,
    Clayfish,
}

impl TropicalFishPattern {
    const ALL: &'static [Self] = &[
        Self::Kob,
        Self::Sunstreak,
        Self::Snooper,
        Self::Dasher,
        Self::Brinely,
        Self::Spotty,
        Self::Flopper,
        Self::Stripey,
        Self::Glitter,
        Self::Blockfish,
        Self::Betty,
        Self::Clayfish,
    ];

    #[must_use]
    pub fn from_name(name: &str) -> Option<Self> {
        Some(match name.strip_prefix("minecraft:").unwrap_or(name) {
            "kob" => Self::Kob,
            "sunstreak" => Self::Sunstreak,
            "snooper" => Self::Snooper,
            "dasher" => Self::Dasher,
            "brinely" => Self::Brinely,
            "spotty" => Self::Spotty,
            "flopper" => Self::Flopper,
            "stripey" => Self::Stripey,
            "glitter" => Self::Glitter,
            "blockfish" => Self::Blockfish,
            "betty" => Self::Betty,
            "clayfish" => Self::Clayfish,
            _ => return None,
        })
    }

    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::Kob => "kob",
            Self::Sunstreak => "sunstreak",
            Self::Snooper => "snooper",
            Self::Dasher => "dasher",
            Self::Brinely => "brinely",
            Self::Spotty => "spotty",
            Self::Flopper => "flopper",
            Self::Stripey => "stripey",
            Self::Glitter => "glitter",
            Self::Blockfish => "blockfish",
            Self::Betty => "betty",
            Self::Clayfish => "clayfish",
        }
    }

    /// Vanilla's packed pattern id: the base (`0` small, `1` large) in the low
    /// byte and the pattern index in the next one.
    #[must_use]
    pub const fn packed_id(self) -> i32 {
        match self {
            Self::Kob => 0,
            Self::Sunstreak => 256,
            Self::Snooper => 512,
            Self::Dasher => 768,
            Self::Brinely => 1024,
            Self::Spotty => 1280,
            Self::Flopper => 1,
            Self::Stripey => 257,
            Self::Glitter => 513,
            Self::Blockfish => 769,
            Self::Betty => 1025,
            Self::Clayfish => 1281,
        }
    }

    #[must_use]
    pub const fn from_packed_id(id: i32) -> Self {
        match id {
            256 => Self::Sunstreak,
            512 => Self::Snooper,
            768 => Self::Dasher,
            1024 => Self::Brinely,
            1280 => Self::Spotty,
            1 => Self::Flopper,
            257 => Self::Stripey,
            513 => Self::Glitter,
            769 => Self::Blockfish,
            1025 => Self::Betty,
            1281 => Self::Clayfish,
            _ => Self::Kob,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TropicalFishVariant {
    pub pattern: TropicalFishPattern,
    pub base_color: DyeColor,
    pub pattern_color: DyeColor,
}

impl TropicalFishVariant {
    #[must_use]
    pub const fn new(
        pattern: TropicalFishPattern,
        base_color: DyeColor,
        pattern_color: DyeColor,
    ) -> Self {
        Self {
            pattern,
            base_color,
            pattern_color,
        }
    }

    #[must_use]
    pub const fn packed_id(self) -> i32 {
        self.pattern.packed_id()
            | (self.base_color.id() as i32) << 16
            | (self.pattern_color.id() as i32) << 24
    }

    #[must_use]
    pub fn from_packed_id(id: i32) -> Self {
        Self {
            pattern: TropicalFishPattern::from_packed_id(id & 0xFFFF),
            base_color: DyeColor::White,
            pattern_color: DyeColor::White,
        }
        .with_colors((id >> 16) & 0xFF, (id >> 24) & 0xFF)
    }

    fn with_colors(self, base: i32, pattern: i32) -> Self {
        Self {
            pattern: self.pattern,
            base_color: DyeColor::by_id((base & 0xFF) as u8).unwrap_or(DyeColor::White),
            pattern_color: DyeColor::by_id((pattern & 0xFF) as u8).unwrap_or(DyeColor::White),
        }
    }
}

const COMMON_VARIANTS: &[TropicalFishVariant] = &[
    TropicalFishVariant::new(
        TropicalFishPattern::Stripey,
        DyeColor::Orange,
        DyeColor::Gray,
    ),
    TropicalFishVariant::new(TropicalFishPattern::Flopper, DyeColor::Gray, DyeColor::Gray),
    TropicalFishVariant::new(TropicalFishPattern::Flopper, DyeColor::Gray, DyeColor::Blue),
    TropicalFishVariant::new(
        TropicalFishPattern::Clayfish,
        DyeColor::White,
        DyeColor::Gray,
    ),
    TropicalFishVariant::new(
        TropicalFishPattern::Sunstreak,
        DyeColor::Blue,
        DyeColor::Gray,
    ),
    TropicalFishVariant::new(TropicalFishPattern::Kob, DyeColor::Orange, DyeColor::White),
    TropicalFishVariant::new(
        TropicalFishPattern::Spotty,
        DyeColor::Pink,
        DyeColor::LightBlue,
    ),
    TropicalFishVariant::new(
        TropicalFishPattern::Blockfish,
        DyeColor::Purple,
        DyeColor::Yellow,
    ),
    TropicalFishVariant::new(
        TropicalFishPattern::Clayfish,
        DyeColor::White,
        DyeColor::Red,
    ),
    TropicalFishVariant::new(
        TropicalFishPattern::Spotty,
        DyeColor::White,
        DyeColor::Yellow,
    ),
    TropicalFishVariant::new(
        TropicalFishPattern::Glitter,
        DyeColor::White,
        DyeColor::Gray,
    ),
    TropicalFishVariant::new(
        TropicalFishPattern::Clayfish,
        DyeColor::White,
        DyeColor::Orange,
    ),
    TropicalFishVariant::new(TropicalFishPattern::Dasher, DyeColor::Cyan, DyeColor::Pink),
    TropicalFishVariant::new(
        TropicalFishPattern::Brinely,
        DyeColor::Lime,
        DyeColor::LightBlue,
    ),
    TropicalFishVariant::new(TropicalFishPattern::Betty, DyeColor::Red, DyeColor::White),
    TropicalFishVariant::new(TropicalFishPattern::Snooper, DyeColor::Gray, DyeColor::Red),
    TropicalFishVariant::new(
        TropicalFishPattern::Blockfish,
        DyeColor::Red,
        DyeColor::White,
    ),
    TropicalFishVariant::new(
        TropicalFishPattern::Flopper,
        DyeColor::White,
        DyeColor::Yellow,
    ),
    TropicalFishVariant::new(TropicalFishPattern::Kob, DyeColor::Red, DyeColor::White),
    TropicalFishVariant::new(
        TropicalFishPattern::Sunstreak,
        DyeColor::Gray,
        DyeColor::White,
    ),
    TropicalFishVariant::new(
        TropicalFishPattern::Dasher,
        DyeColor::Cyan,
        DyeColor::Yellow,
    ),
    TropicalFishVariant::new(
        TropicalFishPattern::Flopper,
        DyeColor::Yellow,
        DyeColor::Yellow,
    ),
];

/// Represents a Tropical Fish, a passive fish of warm oceans.
///
/// Wiki: <https://minecraft.wiki/w/Tropical_Fish>
pub struct TropicalFishEntity {
    pub mob_entity: MobEntity,
    pub fish_data: FishData,
    pub variant: AtomicI32,
}

impl TropicalFishEntity {
    pub fn new(entity: Entity) -> Arc<Self> {
        let mob_entity = MobEntity::new(entity);
        fish::init(&mob_entity);
        fish::init_schooling(&mob_entity);
        Arc::new(Self {
            mob_entity,
            fish_data: FishData::new(8),
            variant: AtomicI32::new(0),
        })
    }

    #[must_use]
    pub fn get_variant(&self) -> TropicalFishVariant {
        TropicalFishVariant::from_packed_id(self.variant.load(Ordering::Relaxed))
    }

    #[must_use]
    pub fn get_pattern(&self) -> TropicalFishPattern {
        self.get_variant().pattern
    }

    #[must_use]
    pub fn get_base_color(&self) -> DyeColor {
        self.get_variant().base_color
    }

    #[must_use]
    pub fn get_pattern_color(&self) -> DyeColor {
        self.get_variant().pattern_color
    }

    pub fn set_variant(&self, variant: TropicalFishVariant) {
        let packed = variant.packed_id();
        self.variant.store(packed, Ordering::Relaxed);
        self.get_entity().set_synced_data(
            tracked_data::tropical_fish::DATA_ID_TYPE_VARIANT,
            VarInt(packed),
        );
    }

    pub fn set_pattern_name(&self, name: &str) {
        if let Some(pattern) = TropicalFishPattern::from_name(name) {
            let mut variant = self.get_variant();
            variant.pattern = pattern;
            self.set_variant(variant);
        }
    }

    pub fn set_base_color_name(&self, name: &str) {
        if let Some(color) = DyeColor::by_name(name.strip_prefix("minecraft:").unwrap_or(name)) {
            let mut variant = self.get_variant();
            variant.base_color = color;
            self.set_variant(variant);
        }
    }

    pub fn set_pattern_color_name(&self, name: &str) {
        if let Some(color) = DyeColor::by_name(name.strip_prefix("minecraft:").unwrap_or(name)) {
            let mut variant = self.get_variant();
            variant.pattern_color = color;
            self.set_variant(variant);
        }
    }
}

impl Mob for TropicalFishEntity {
    fn get_mob_entity(&self) -> &MobEntity {
        &self.mob_entity
    }

    fn get_fish_data(&self) -> Option<&FishData> {
        Some(&self.fish_data)
    }

    fn mob_tick(&self, caller: &dyn EntityBase) {
        self.fish_data.tick_air_supply(caller);
        fish::tick_schooling(self);
        fish::flop(self, Sound::EntityTropicalFishFlop);
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
        let variant = if rand::random::<f32>() < 0.9 {
            COMMON_VARIANTS[rand::random_range(0..COMMON_VARIANTS.len())]
        } else {
            TropicalFishVariant::new(
                TropicalFishPattern::ALL[rand::random_range(0..TropicalFishPattern::ALL.len())],
                DyeColor::by_id(rand::random_range(0..16)).unwrap_or_default(),
                DyeColor::by_id(rand::random_range(0..16)).unwrap_or_default(),
            )
        };
        self.set_variant(variant);
        group_data
    }

    fn mob_init_data_tracker(&self) {
        fish::init_data(&self.fish_data, self.get_entity());
        self.get_entity().set_synced_data(
            tracked_data::tropical_fish::DATA_ID_TYPE_VARIANT,
            VarInt(self.variant.load(Ordering::Relaxed)),
        );
    }

    fn mob_write_nbt(&self, nbt: &mut NbtCompound) {
        fish::write_data(&self.fish_data, nbt);
        nbt.put_int("Variant", self.variant.load(Ordering::Relaxed));
    }

    fn mob_read_nbt(&self, nbt: &NbtCompound) {
        fish::read_data(&self.fish_data, self.get_entity(), nbt);
        if let Some(variant) = nbt.get_int("Variant") {
            self.set_variant(TropicalFishVariant::from_packed_id(variant));
        }
    }

    fn get_ambient_sound(&self) -> Option<Sound> {
        Some(Sound::EntityTropicalFishAmbient)
    }

    fn get_ambient_sound_interval(&self) -> i32 {
        120
    }

    fn as_custom_sound(&self) -> Option<&dyn CustomSound> {
        Some(self)
    }
}

impl CustomSound for TropicalFishEntity {
    fn death_sound(&self) -> Option<Sound> {
        Some(Sound::EntityTropicalFishDeath)
    }

    fn hurt_sound(&self) -> Option<Sound> {
        Some(Sound::EntityTropicalFishHurt)
    }
}
