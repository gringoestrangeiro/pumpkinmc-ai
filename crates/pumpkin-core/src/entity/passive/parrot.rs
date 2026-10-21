use std::sync::{
    Arc, Weak,
    atomic::{AtomicBool, AtomicI32, AtomicU8, Ordering},
};

use crossbeam::atomic::AtomicCell;
use pumpkin_data::damage::DamageType;
use pumpkin_data::effect::StatusEffect;
use pumpkin_data::entity::{EntityStatus, EntityType};
use pumpkin_data::item_stack::ItemStack;
use pumpkin_data::sound::{Sound, SoundCategory};
use pumpkin_data::tag::{self, Taggable};
use pumpkin_nbt::compound::NbtCompound;
use pumpkin_protocol::bedrock::server::actor_event::ActorEventID;
use pumpkin_protocol::codec::var_int::VarInt;
use pumpkin_util::Difficulty;
use pumpkin_util::math::boundingbox::BoundingBox;
use pumpkin_util::math::position::BlockPos;
use pumpkin_util::math::vector3::Vector3;

use crate::entity::ai::control::flying_move_control::FlyingMoveControl;
use crate::entity::ai::pathfinder::Navigator;
use crate::entity::{
    Entity, EntityBase,
    ai::goal::{
        Controls, Goal, escape_danger::EscapeDangerGoal, follow_mob::FollowMobGoal,
        follow_owner::FollowOwnerGoal, look_at_entity::LookAtEntityGoal, swim::SwimGoal,
        water_avoiding_random_flying::WaterAvoidingRandomFlyingGoal,
    },
    mob::{Mob, MobEntity},
    passive::{
        animal::Animal,
        tamable::{TamableAnimal, TamableData},
    },
    player::Player,
};

/// Duration in ticks of the poison a parrot gets from eating a cookie, matching
/// vanilla `Parrot.mobInteract`.
const COOKIE_POISON_DURATION: i32 = 900;
/// Vanilla `ShoulderRidingEntity.RIDE_COOLDOWN`.
const SHOULDER_RIDE_COOLDOWN: i32 = 100;
const IMITATE_INTERVAL: i32 = 400;
const IMITATE_RANGE: f64 = 20.0;
const JUKEBOX_RANGE_SQ: f64 = 3.46 * 3.46;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[repr(u8)]
pub enum ParrotVariant {
    #[default]
    RedBlue = 0,
    Blue = 1,
    Green = 2,
    YellowBlue = 3,
    Gray = 4,
}

impl ParrotVariant {
    pub const COUNT: u8 = 5;

    #[must_use]
    pub const fn from_id(id: u8) -> Self {
        match id {
            1 => Self::Blue,
            2 => Self::Green,
            3 => Self::YellowBlue,
            4 => Self::Gray,
            _ => Self::RedBlue,
        }
    }

    #[must_use]
    pub const fn id(self) -> u8 {
        self as u8
    }
}

/// Represents a Parrot, a passive flying mob that can mimic nearby mob sounds.
///
/// Wiki: <https://minecraft.wiki/w/Parrot>
pub struct ParrotEntity {
    pub mob_entity: MobEntity,
    pub tamable_data: TamableData,
    pub variant: AtomicU8,
    ride_cooldown: AtomicI32,
    party_parrot: AtomicBool,
    jukebox: AtomicCell<Option<BlockPos>>,
}

impl ParrotEntity {
    pub fn new(entity: Entity) -> Arc<Self> {
        let mob_entity = MobEntity::new(entity);
        {
            *mob_entity
                .move_control
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner) =
                Box::new(FlyingMoveControl::new(10, false));
            let mut navigator = Navigator::flying();
            navigator.set_can_open_doors(false);
            navigator.set_can_float(true);
            *mob_entity
                .navigator
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner) = navigator;
        }
        let parrot = Self {
            mob_entity,
            tamable_data: TamableData::default(),
            variant: AtomicU8::new(ParrotVariant::RedBlue.id()),
            ride_cooldown: AtomicI32::new(0),
            party_parrot: AtomicBool::new(false),
            jukebox: AtomicCell::new(None),
        };
        let mob_arc = Arc::new(parrot);
        let mob_weak: Weak<dyn Mob> = {
            let mob_arc: Arc<dyn Mob> = mob_arc.clone();
            Arc::downgrade(&mob_arc)
        };
        let shoulder_weak = Arc::downgrade(&mob_arc);

        {
            let mut goal_selector = mob_arc
                .mob_entity
                .goals_selector
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);

            goal_selector.add_goal(0, EscapeDangerGoal::new(1.25));
            goal_selector.add_goal(0, Box::new(SwimGoal::default()));
            goal_selector.add_goal(
                1,
                LookAtEntityGoal::with_default(mob_weak, &EntityType::PLAYER, 8.0),
            );
            goal_selector.add_goal(
                2,
                Box::new(crate::entity::ai::goal::sit_when_ordered_to::SitWhenOrderedToGoal::new()),
            );
            goal_selector.add_goal(2, FollowOwnerGoal::new(1.0, 5.0, 1.0));
            goal_selector.add_goal(2, Box::new(WaterAvoidingRandomFlyingGoal::new(1.0)));
            goal_selector.add_goal(3, Box::new(LandOnOwnersShoulderGoal::new(shoulder_weak)));
            goal_selector.add_goal(3, FollowMobGoal::new(1.0, 3.0, 7.0));
        };

        mob_arc
    }

    #[must_use]
    pub fn get_variant(&self) -> ParrotVariant {
        ParrotVariant::from_id(self.variant.load(Ordering::Relaxed))
    }

    pub fn set_variant(&self, variant: ParrotVariant) {
        self.variant.store(variant.id(), Ordering::Relaxed);
        let entity = self.get_entity();
        entity.set_synced_data(
            pumpkin_data::tracked_data::parrot::DATA_VARIANT_ID,
            VarInt(i32::from(variant.id())),
        );
    }

    #[must_use]
    pub fn is_flying(&self) -> bool {
        !self.get_entity().on_ground.load(Ordering::Relaxed)
    }

    #[must_use]
    pub fn party_parrot(&self) -> bool {
        self.party_parrot.load(Ordering::Relaxed)
    }

    /// Vanilla `Parrot.setRecordPlayingNearby`.
    pub fn set_record_playing_nearby(&self, jukebox: BlockPos, is_playing: bool) {
        self.jukebox.store(Some(jukebox));
        self.party_parrot.store(is_playing, Ordering::Relaxed);
    }

    #[must_use]
    pub fn can_sit_on_shoulder(&self) -> bool {
        self.ride_cooldown.load(Ordering::Relaxed) > SHOULDER_RIDE_COOLDOWN
    }

    /// Vanilla `ShoulderRidingEntity.setEntityOnShoulder`.
    pub fn set_entity_on_shoulder(&self, player: &Player) -> bool {
        let entity = self.get_entity();
        let mut tag = NbtCompound::new();
        entity.write_nbt(&mut tag);

        if player.set_entity_on_shoulder(tag) {
            entity.remove();
            true
        } else {
            false
        }
    }

    /// Vanilla `Parrot.imitateNearbyMobs`.
    pub fn imitate_nearby_mobs(world: &crate::world::World, entity: &dyn EntityBase) -> bool {
        let entity_data = entity.get_entity();
        if !entity_data.is_alive() || entity_data.is_silent() || rand::random_range(0..2) != 0 {
            return false;
        }

        let pos = entity_data.pos.load();
        let nearby = world.get_nearby_entities(pos, IMITATE_RANGE);
        let candidates: Vec<&Arc<dyn EntityBase>> = nearby
            .values()
            .filter(|candidate| {
                let candidate_entity = candidate.get_entity();
                candidate.get_mob().is_some()
                    && !candidate_entity.is_silent()
                    && imitated_sound(candidate_entity.entity_type).is_some()
            })
            .collect();

        let Some(candidate) = candidates.get(rand::random_range(0..candidates.len().max(1))) else {
            return false;
        };
        let Some(sound) = imitated_sound(candidate.get_entity().entity_type) else {
            return false;
        };
        let candidate_pos = candidate.get_entity().pos.load();
        world.play_sound_fine(
            sound,
            SoundCategory::Neutral,
            &candidate_pos,
            0.7,
            get_pitch(),
        );
        true
    }

    /// Vanilla `Parrot.getAmbient`: usually the parrot call, rarely one of its imitations.
    pub fn get_ambient(world: &crate::world::World) -> Sound {
        if world.level_info.load().difficulty != Difficulty::Peaceful
            && rand::random_range(0..1000) == 0
        {
            let sounds = IMITATED_SOUNDS;
            return sounds[rand::random_range(0..sounds.len())];
        }
        Sound::EntityParrotAmbient
    }

    #[must_use]
    pub fn get_pitch() -> f32 {
        (rand::random::<f32>() - rand::random::<f32>()) * 0.2 + 1.0
    }

    /// Vanilla `Parrot.mobInteract` taming branch.
    fn try_tame(&self, player: &Player, item_stack: &mut ItemStack) {
        item_stack.decrement_unless_creative(player.gamemode.load(), 1);
        if !self.get_entity().is_silent() {
            let world = self.get_entity().world.load();
            world.play_sound_fine(
                Sound::EntityParrotEat,
                SoundCategory::Neutral,
                &self.get_entity().pos.load(),
                1.0,
                1.0 + (rand::random::<f32>() - rand::random::<f32>()) * 0.2,
            );
        }

        let entity = self.get_entity();
        let world = entity.world.load();
        if rand::random_range(0..10) == 0 {
            self.tame(player.gameprofile.id);
            world.send_entity_status(
                entity,
                EntityStatus::TamingSucceeded,
                Some(ActorEventID::TamingSucceeded),
            );
        } else {
            world.send_entity_status(
                entity,
                EntityStatus::TamingFailed,
                Some(ActorEventID::TamingFailed),
            );
        }
    }

    /// Feeds the parrot a cookie: it is poisoned and then killed, as in vanilla
    /// `Parrot.mobInteract`.
    fn eat_cookie(&self, player: &Arc<Player>, item_stack: &mut ItemStack) {
        item_stack.decrement_unless_creative(player.gamemode.load(), 1);

        self.mob_entity
            .living_entity
            .add_effect(pumpkin_data::potion::Effect {
                effect_type: &StatusEffect::POISON,
                duration: COOKIE_POISON_DURATION,
                amplifier: 0,
                ambient: false,
                show_particles: true,
                show_icon: true,
                blend: true,
            });

        // Vanilla guards this call with `player.isCreative() || !this.isInvulnerable()`,
        // but `hurt` re-checks invulnerability itself and `player_attack` doesn't bypass
        // it, so the guard only skips a call that would do nothing anyway.
        self.damage_with_context(
            self,
            f32::MAX,
            DamageType::PLAYER_ATTACK,
            None,
            Some(player.as_ref()),
            Some(player.as_ref()),
        );
    }

    fn tick_jukebox(&self) {
        let Some(jukebox) = self.jukebox.load() else {
            return;
        };
        let entity = self.get_entity();
        let pos = entity.pos.load();
        let center = Vector3::new(
            f64::from(jukebox.0.x) + 0.5,
            f64::from(jukebox.0.y) + 0.5,
            f64::from(jukebox.0.z) + 0.5,
        );
        let world = entity.world.load();
        if pos.squared_distance_to_vec(&center) > JUKEBOX_RANGE_SQ
            || world.get_block(&jukebox) != &pumpkin_data::Block::JUKEBOX
        {
            self.party_parrot.store(false, Ordering::Relaxed);
            self.jukebox.store(None);
        }
    }
}

impl Animal for ParrotEntity {
    fn is_food(&self, _item_stack: &ItemStack) -> bool {
        false
    }
}

impl TamableAnimal for ParrotEntity {
    fn get_tamable_data(&self) -> &TamableData {
        &self.tamable_data
    }
}

impl Mob for ParrotEntity {
    fn as_animal(&self) -> Option<&dyn Animal> {
        Some(self)
    }

    fn as_tamable(&self) -> Option<&dyn TamableAnimal> {
        Some(self)
    }

    fn get_mob_entity(&self) -> &MobEntity {
        &self.mob_entity
    }

    fn finalize_spawn(
        &self,
        _world: &Arc<crate::world::World>,
        group_data: Option<crate::entity::mob::spawn::SpawnGroupData>,
    ) -> Option<crate::entity::mob::spawn::SpawnGroupData> {
        self.mob_entity.finalize_spawn_base();
        // Vanilla `Parrot.finalizeSpawn` rolls a random variant for every fresh parrot.
        self.set_variant(ParrotVariant::from_id(rand::random_range(
            0..ParrotVariant::COUNT,
        )));
        group_data
    }

    fn get_ambient_sound(&self) -> Option<Sound> {
        let world = self.get_entity().world.load_full();
        Some(Self::get_ambient(&world))
    }

    fn mob_write_nbt(&self, nbt: &mut NbtCompound) {
        nbt.put_int("Variant", i32::from(self.get_variant().id()));
    }

    fn mob_read_nbt(&self, nbt: &NbtCompound) {
        let variant = nbt.get_int("Variant").unwrap_or(0).clamp(0, 4) as u8;
        self.set_variant(ParrotVariant::from_id(variant));
    }

    fn mob_tick(&self, _caller: &dyn EntityBase) {
        self.ride_cooldown.fetch_add(1, Ordering::Relaxed);
        self.tick_jukebox();

        if rand::random_range(0..IMITATE_INTERVAL) == 0 {
            let entity = self.get_entity();
            let world = entity.world.load();
            Self::imitate_nearby_mobs(&world, self);
        }
    }

    fn mob_init_data_tracker(&self) {
        let entity = self.get_entity();
        entity.set_synced_data(
            pumpkin_data::tracked_data::tamable_animal::DATA_FLAGS_ID,
            self.get_tame_flags(),
        );
        entity.set_synced_data(
            pumpkin_data::tracked_data::tamable_animal::DATA_OWNERUUID_ID,
            self.get_owner(),
        );
        entity.set_synced_data(
            pumpkin_data::tracked_data::parrot::DATA_VARIANT_ID,
            VarInt(i32::from(self.variant.load(Ordering::Relaxed))),
        );
    }

    fn mob_interact(&self, player: &Arc<Player>, item_stack: &mut ItemStack) -> bool {
        let item = item_stack.get_item();

        if !self.is_tame() && item.has_tag(&tag::Item::MINECRAFT_PARROT_FOOD) {
            self.try_tame(player, item_stack);
            return true;
        }

        if !item.has_tag(&tag::Item::MINECRAFT_PARROT_POISONOUS_FOOD) {
            if !self.is_flying() && self.is_tame() && self.is_owned_by(&player.gameprofile.id) {
                self.set_ordered_to_sit(!self.is_ordered_to_sit());
                return true;
            }
            return self.mob_entity.mob_interact(player, item_stack);
        }

        self.eat_cookie(player, item_stack);
        true
    }

    fn on_damage(&self, _damage_type: DamageType, _source: Option<&dyn EntityBase>) {
        self.set_ordered_to_sit(false);
    }

    fn remove_when_far_away(&self, _distance_sq: f64) -> bool {
        // Untamed parrots do not have a despawn exception beyond the vanilla age threshold.
        !self.is_tame()
    }
}

impl ParrotEntity {
    fn get_tame_flags(&self) -> u8 {
        let mut flags = 0u8;
        if self.is_in_sitting_pose() {
            flags |= 1;
        }
        if self.is_tame() {
            flags |= 4;
        }
        flags
    }
}

/// Vanilla `LandOnOwnersShoulderGoal`.
struct LandOnOwnersShoulderGoal {
    parrot: Weak<ParrotEntity>,
}

impl LandOnOwnersShoulderGoal {
    const fn new(parrot: Weak<ParrotEntity>) -> Self {
        Self { parrot }
    }

    fn owner(parrot: &ParrotEntity) -> Option<Arc<Player>> {
        let uuid = parrot.get_owner()?;
        parrot.get_entity().world.load().get_player_by_uuid(uuid)
    }
}

impl Goal for LandOnOwnersShoulderGoal {
    fn can_start(&mut self, _mob: &dyn Mob) -> bool {
        let Some(parrot) = self.parrot.upgrade() else {
            return false;
        };
        let Some(owner) = Self::owner(&parrot) else {
            return false;
        };
        let entity = owner.get_entity();
        let can_sit = !owner.is_spectator()
            && !owner.is_flying()
            && !entity.is_in_water()
            && !entity.is_in_powder_snow();
        !parrot.is_ordered_to_sit() && can_sit && parrot.can_sit_on_shoulder()
    }

    fn start(&mut self, _mob: &dyn Mob) {}

    fn tick(&mut self, _mob: &dyn Mob) {
        let Some(parrot) = self.parrot.upgrade() else {
            return;
        };
        if parrot.is_in_sitting_pose() || parrot.get_entity().is_leashed() {
            return;
        }
        let Some(owner) = Self::owner(&parrot) else {
            return;
        };
        let parrot_box = parrot.get_entity().bounding_box.load();
        let owner_box = owner.get_entity().bounding_box.load();
        if parrot_box.intersects(&owner_box) {
            parrot.set_entity_on_shoulder(&owner);
        }
    }

    fn controls(&self) -> Controls {
        Controls::empty()
    }
}

/// Vanilla `Parrot.MOB_SOUND_MAP`, used by both the ambient imitation and
/// `imitateNearbyMobs`.
const IMITATED_SOUNDS: &[Sound] = &[
    Sound::EntityParrotImitateBlaze,
    Sound::EntityParrotImitateBogged,
    Sound::EntityParrotImitateBreeze,
    Sound::EntityParrotImitateCamelHusk,
    Sound::EntityParrotImitateSpider,
    Sound::EntityParrotImitateCreaking,
    Sound::EntityParrotImitateCreeper,
    Sound::EntityParrotImitateDrowned,
    Sound::EntityParrotImitateElderGuardian,
    Sound::EntityParrotImitateEnderDragon,
    Sound::EntityParrotImitateEndermite,
    Sound::EntityParrotImitateEvoker,
    Sound::EntityParrotImitateGhast,
    Sound::IntentionallyEmpty,
    Sound::EntityParrotImitateGuardian,
    Sound::EntityParrotImitateHoglin,
    Sound::EntityParrotImitateHusk,
    Sound::EntityParrotImitateIllusioner,
    Sound::EntityParrotImitateMagmaCube,
    Sound::EntityParrotImitateParched,
    Sound::EntityParrotImitatePhantom,
    Sound::EntityParrotImitatePiglin,
    Sound::EntityParrotImitatePiglinBrute,
    Sound::EntityParrotImitatePillager,
    Sound::EntityParrotImitateRavager,
    Sound::EntityParrotImitateShulker,
    Sound::EntityParrotImitateSilverfish,
    Sound::EntityParrotImitateSkeleton,
    Sound::EntityParrotImitateSlime,
    Sound::EntityParrotImitateSpider,
    Sound::EntityParrotImitateStray,
    Sound::EntityParrotImitateVex,
    Sound::EntityParrotImitateVindicator,
    Sound::EntityParrotImitateWarden,
    Sound::EntityParrotImitateWitch,
    Sound::EntityParrotImitateWither,
    Sound::EntityParrotImitateWitherSkeleton,
    Sound::EntityParrotImitateZoglin,
    Sound::EntityParrotImitateZombie,
    Sound::EntityParrotImitateZombieHorse,
    Sound::EntityParrotImitateZombieNautilus,
    Sound::EntityParrotImitateZombieVillager,
];

fn imitated_sound(entity_type: &EntityType) -> Option<Sound> {
    match entity_type.id {
        id if id == EntityType::BLAZE.id => Some(Sound::EntityParrotImitateBlaze),
        id if id == EntityType::BOGGED.id => Some(Sound::EntityParrotImitateBogged),
        id if id == EntityType::BREEZE.id => Some(Sound::EntityParrotImitateBreeze),
        id if id == EntityType::CAMEL_HUSK.id => Some(Sound::EntityParrotImitateCamelHusk),
        id if id == EntityType::CAVE_SPIDER.id => Some(Sound::EntityParrotImitateSpider),
        id if id == EntityType::CREAKING.id => Some(Sound::EntityParrotImitateCreaking),
        id if id == EntityType::CREEPER.id => Some(Sound::EntityParrotImitateCreeper),
        id if id == EntityType::DROWNED.id => Some(Sound::EntityParrotImitateDrowned),
        id if id == EntityType::ELDER_GUARDIAN.id => Some(Sound::EntityParrotImitateElderGuardian),
        id if id == EntityType::ENDER_DRAGON.id => Some(Sound::EntityParrotImitateEnderDragon),
        id if id == EntityType::ENDERMITE.id => Some(Sound::EntityParrotImitateEndermite),
        id if id == EntityType::EVOKER.id => Some(Sound::EntityParrotImitateEvoker),
        id if id == EntityType::GHAST.id => Some(Sound::EntityParrotImitateGhast),
        id if id == EntityType::HAPPY_GHAST.id => Some(Sound::IntentionallyEmpty),
        id if id == EntityType::GUARDIAN.id => Some(Sound::EntityParrotImitateGuardian),
        id if id == EntityType::HOGLIN.id => Some(Sound::EntityParrotImitateHoglin),
        id if id == EntityType::HUSK.id => Some(Sound::EntityParrotImitateHusk),
        id if id == EntityType::ILLUSIONER.id => Some(Sound::EntityParrotImitateIllusioner),
        id if id == EntityType::MAGMA_CUBE.id => Some(Sound::EntityParrotImitateMagmaCube),
        id if id == EntityType::PARCHED.id => Some(Sound::EntityParrotImitateParched),
        id if id == EntityType::PHANTOM.id => Some(Sound::EntityParrotImitatePhantom),
        id if id == EntityType::PIGLIN.id => Some(Sound::EntityParrotImitatePiglin),
        id if id == EntityType::PIGLIN_BRUTE.id => Some(Sound::EntityParrotImitatePiglinBrute),
        id if id == EntityType::PILLAGER.id => Some(Sound::EntityParrotImitatePillager),
        id if id == EntityType::RAVAGER.id => Some(Sound::EntityParrotImitateRavager),
        id if id == EntityType::SHULKER.id => Some(Sound::EntityParrotImitateShulker),
        id if id == EntityType::SILVERFISH.id => Some(Sound::EntityParrotImitateSilverfish),
        id if id == EntityType::SKELETON.id => Some(Sound::EntityParrotImitateSkeleton),
        id if id == EntityType::SLIME.id => Some(Sound::EntityParrotImitateSlime),
        id if id == EntityType::SPIDER.id => Some(Sound::EntityParrotImitateSpider),
        id if id == EntityType::STRAY.id => Some(Sound::EntityParrotImitateStray),
        id if id == EntityType::VEX.id => Some(Sound::EntityParrotImitateVex),
        id if id == EntityType::VINDICATOR.id => Some(Sound::EntityParrotImitateVindicator),
        id if id == EntityType::WARDEN.id => Some(Sound::EntityParrotImitateWarden),
        id if id == EntityType::WITCH.id => Some(Sound::EntityParrotImitateWitch),
        id if id == EntityType::WITHER.id => Some(Sound::EntityParrotImitateWither),
        id if id == EntityType::WITHER_SKELETON.id => {
            Some(Sound::EntityParrotImitateWitherSkeleton)
        }
        id if id == EntityType::ZOGLIN.id => Some(Sound::EntityParrotImitateZoglin),
        id if id == EntityType::ZOMBIE.id => Some(Sound::EntityParrotImitateZombie),
        id if id == EntityType::ZOMBIE_HORSE.id => Some(Sound::EntityParrotImitateZombieHorse),
        id if id == EntityType::ZOMBIE_NAUTILUS.id => {
            Some(Sound::EntityParrotImitateZombieNautilus)
        }
        id if id == EntityType::ZOMBIE_VILLAGER.id => {
            Some(Sound::EntityParrotImitateZombieVillager)
        }
        _ => None,
    }
}

/// Vanilla `Level.setRecordPlayingNearby` search box: `new AABB(pos).inflate(3.0)`.
#[must_use]
pub fn jukebox_parrot_box(pos: BlockPos) -> BoundingBox {
    BoundingBox {
        min: Vector3::new(
            f64::from(pos.0.x) - 3.0,
            f64::from(pos.0.y) - 3.0,
            f64::from(pos.0.z) - 3.0,
        ),
        max: Vector3::new(
            f64::from(pos.0.x) + 4.0,
            f64::from(pos.0.y) + 4.0,
            f64::from(pos.0.z) + 4.0,
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::COOKIE_POISON_DURATION;
    use pumpkin_data::item::Item;
    use pumpkin_data::tag::{self, Taggable};

    /// The interaction is gated on the vanilla `parrot_poisonous_food` tag rather than
    /// on a hardcoded cookie id, so check the tag actually resolves the way the
    /// interaction assumes.
    #[test]
    fn cookie_is_poisonous_parrot_food() {
        assert!(Item::COOKIE.has_tag(&tag::Item::MINECRAFT_PARROT_POISONOUS_FOOD));
    }

    /// Seeds tame a parrot in vanilla and must not reach the poison branch.
    #[test]
    fn parrot_food_is_not_poisonous() {
        assert!(!Item::WHEAT_SEEDS.has_tag(&tag::Item::MINECRAFT_PARROT_POISONOUS_FOOD));
        assert!(!Item::COOKED_CHICKEN.has_tag(&tag::Item::MINECRAFT_PARROT_POISONOUS_FOOD));
    }

    #[test]
    fn poison_lasts_45_seconds() {
        assert_eq!(COOKIE_POISON_DURATION, 900);
    }
}
