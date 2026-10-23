//! Vanilla `Mob.finalizeSpawn`: the step run on freshly created mobs before they enter the world.

use std::sync::Arc;

use pumpkin_data::attributes::Attributes;
use pumpkin_data::data_component_impl::EquipmentSlot;
use pumpkin_data::effect::StatusEffect;
use pumpkin_data::item::Item;
use pumpkin_data::item_stack::ItemStack;
use pumpkin_data::wolf_variant::WolfVariant;
use rand::RngExt;

use crate::entity::EntityBase;
use crate::entity::attributes::{Modifier, ModifierOperation};
use crate::entity::mob::{Mob, MobEntity};
use crate::entity::passive::axolotl::AxolotlVariant;
use crate::entity::passive::fox::FoxVariant;
use crate::entity::passive::llama::LlamaVariant;
use crate::entity::passive::rabbit::RabbitVariant;
use crate::entity::passive::tropical_fish::TropicalFishVariant;
use crate::world::World;

pub(crate) const RANDOM_SPAWN_BONUS_ID: &str = "minecraft:random_spawn_bonus";

/// State shared by every mob of one spawn group (vanilla `SpawnGroupData`).
pub enum SpawnGroupData {
    Ageable(AgeableMobGroupData),
    /// Vanilla `Horse.HorseGroupData`: the coat color of a herd.
    Horse(AgeableMobGroupData, i32),
    Llama(AgeableMobGroupData, LlamaVariant),
    Rabbit(AgeableMobGroupData, RabbitVariant),
    Fox(AgeableMobGroupData, FoxVariant),
    /// Vanilla `Axolotl.AxolotlGroupData`: each member picks one of these two variants.
    Axolotl(AgeableMobGroupData, [AxolotlVariant; 2]),
    WolfPack(AgeableMobGroupData, WolfVariant),
    School(SchoolSpawnGroupData),
    TropicalFish(SchoolSpawnGroupData, TropicalFishVariant),
    /// The effect the first spider of a group rolled on hard difficulty.
    SpiderEffects(Option<&'static StatusEffect>),
    Zombie(ZombieGroupData),
}

impl SpawnGroupData {
    /// The `AgeableMobGroupData` part of the group data classes that extend it.
    const fn ageable_mut(&mut self) -> Option<&mut AgeableMobGroupData> {
        match self {
            Self::Ageable(data)
            | Self::Horse(data, _)
            | Self::Llama(data, _)
            | Self::Rabbit(data, _)
            | Self::Fox(data, _)
            | Self::Axolotl(data, _)
            | Self::WolfPack(data, _) => Some(data),
            _ => None,
        }
    }

    /// The `SchoolSpawnGroupData` part of the group data classes that extend it.
    #[must_use]
    pub const fn school(&self) -> Option<&SchoolSpawnGroupData> {
        match self {
            Self::School(data) | Self::TropicalFish(data, _) => Some(data),
            _ => None,
        }
    }
}

/// Vanilla `AgeableMob.AgeableMobGroupData`.
pub struct AgeableMobGroupData {
    group_size: i32,
    should_spawn_baby: bool,
    baby_spawn_chance: f32,
}

impl AgeableMobGroupData {
    const DEFAULT_BABY_SPAWN_CHANCE: f32 = 0.05;

    #[must_use]
    pub const fn new(should_spawn_baby: bool, baby_spawn_chance: f32) -> Self {
        Self {
            group_size: 0,
            should_spawn_baby,
            baby_spawn_chance,
        }
    }

    #[must_use]
    pub const fn with_should_spawn_baby(should_spawn_baby: bool) -> Self {
        Self::new(should_spawn_baby, Self::DEFAULT_BABY_SPAWN_CHANCE)
    }

    #[must_use]
    pub const fn with_baby_spawn_chance(baby_spawn_chance: f32) -> Self {
        Self::new(true, baby_spawn_chance)
    }

    #[must_use]
    pub const fn group_size(&self) -> i32 {
        self.group_size
    }
}

/// Vanilla `AbstractSchoolingFish.SchoolSpawnGroupData`.
#[derive(Default)]
pub struct SchoolSpawnGroupData {
    /// The first fish of the school. Vanilla builds the data from `this` inside
    /// `finalizeSpawn`; here [`finalize_spawn`] fills it in afterwards.
    pub leader: Option<Arc<dyn EntityBase>>,
}

/// Vanilla `Zombie.ZombieGroupData`: one baby roll shared by the whole group.
#[derive(Clone, Copy)]
pub struct ZombieGroupData {
    pub is_baby: bool,
    pub can_spawn_jockey: bool,
}

/// Vanilla `AgeableMob.finalizeSpawn`: the members after the first of a group may spawn as
/// babies, so a mob finalized on its own never does.
pub fn finalize_ageable_spawn<M: Mob + ?Sized>(
    mob: &M,
    group_data: Option<SpawnGroupData>,
) -> Option<SpawnGroupData> {
    let mut group_data = group_data.unwrap_or(SpawnGroupData::Ageable(
        AgeableMobGroupData::with_should_spawn_baby(true),
    ));
    if let Some(data) = group_data.ageable_mut() {
        if data.should_spawn_baby
            && data.group_size > 0
            && rand::random::<f32>() <= data.baby_spawn_chance
        {
            mob.spawn_as_baby();
        }
        data.group_size += 1;
    }
    mob.get_mob_entity().finalize_spawn_base();
    Some(group_data)
}

/// Vanilla `SpecialDates.isHalloween`, in the server's local time.
#[must_use]
pub fn is_halloween() -> bool {
    let now = time::OffsetDateTime::now_local().unwrap_or_else(|_| time::OffsetDateTime::now_utc());
    now.month() == time::Month::October && now.day() == 31
}

impl MobEntity {
    /// Vanilla `Mob.finalizeSpawn`: random follow range bonus and left-handedness.
    pub fn finalize_spawn_base(&self) {
        let mut rng = rand::rng();
        {
            let mut attributes = self
                .living_entity
                .attributes
                .write()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            if let Some(follow_range) = attributes.get_mut(&Attributes::FOLLOW_RANGE.id)
                && !follow_range
                    .modifiers
                    .iter()
                    .any(|modifier| modifier.id == RANDOM_SPAWN_BONUS_ID)
            {
                follow_range.add_or_replace_modifier(Modifier {
                    id: RANDOM_SPAWN_BONUS_ID.to_string(),
                    // Triangle distribution around 0.
                    amount: 0.114_85 * f64::from(rng.random::<f32>() - rng.random::<f32>()),
                    operation: ModifierOperation::MultiplyBase,
                });
            }
        }
        self.set_left_handed(rng.random::<f32>() < 0.05);
    }

    /// The pumpkin head `Zombie.finalizeSpawn` and `AbstractSkeleton.finalizeSpawn` give
    /// bareheaded mobs on Halloween.
    pub fn maybe_wear_halloween_head(&self) {
        let bareheaded = self
            .living_entity
            .entity_equipment
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .get(&EquipmentSlot::HEAD)
            .is_empty();
        let mut rng = rand::rng();
        if bareheaded && is_halloween() && rng.random::<f32>() < 0.25 {
            let head = if rng.random::<f32>() < 0.1 {
                &Item::JACK_O_LANTERN
            } else {
                &Item::CARVED_PUMPKIN
            };
            self.set_item_slot(&EquipmentSlot::HEAD, ItemStack::new(1, head));
            self.living_entity
                .equipment_drop_chances
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .insert(EquipmentSlot::HEAD, 0.0);
        }
    }

    /// Queues a mob to be added and mounted onto this one when it enters the world.
    pub fn add_pending_rider(&self, rider: Arc<dyn EntityBase>) {
        self.pending_riders
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .push(rider);
    }

    pub(crate) fn take_pending_riders(&self) -> Vec<Arc<dyn EntityBase>> {
        std::mem::take(
            &mut *self
                .pending_riders
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner),
        )
    }
}

/// Finalizes `entity` if it is a mob, and returns the group data for the next mob of the group.
pub fn finalize_spawn(
    entity: &Arc<dyn EntityBase>,
    world: &Arc<World>,
    group_data: Option<SpawnGroupData>,
) -> Option<SpawnGroupData> {
    let Some(mob) = entity.get_mob() else {
        return group_data;
    };
    let mut group_data = mob.finalize_spawn(world, group_data);
    if let Some(SpawnGroupData::School(school) | SpawnGroupData::TropicalFish(school, _)) =
        &mut group_data
        && school.leader.is_none()
    {
        school.leader = Some(entity.clone());
    }
    group_data
}

impl MobEntity {
    /// Baby state kept only as a negative age and the shared baby flag (hoglins, zoglins and
    /// zombified piglins, which are not `AgeableMob`s here).
    pub fn set_baby_by_age(&self) {
        let entity = &self.living_entity.entity;
        entity
            .age
            .store(-24000, std::sync::atomic::Ordering::Relaxed);
        entity.set_synced_data(pumpkin_data::tracked_data::ageable_mob::DATA_BABY_ID, true);
    }

    /// Baby state kept as a mob's own flag and synced key (vanilla zombies and piglins), so it
    /// never ages up.
    pub fn set_baby_flag(
        &self,
        flag: &std::sync::atomic::AtomicBool,
        tracked: pumpkin_data::tracked_data::TrackedData,
        baby: bool,
    ) {
        flag.store(baby, std::sync::atomic::Ordering::Relaxed);
        let entity = &self.living_entity.entity;
        entity.set_synced_data(tracked, baby);
        // Pumpkin's negative age is what reports the Bedrock baby flag.
        entity.age.store(
            if baby { -24000 } else { 0 },
            std::sync::atomic::Ordering::Relaxed,
        );
    }
}
