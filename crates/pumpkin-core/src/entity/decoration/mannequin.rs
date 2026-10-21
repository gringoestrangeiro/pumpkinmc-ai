use std::sync::{
    Mutex,
    atomic::{AtomicBool, AtomicU8, Ordering},
};

use pumpkin_data::{
    damage::DamageType,
    data_component_impl::{DataComponentImpl, ProfileImpl},
    entity::EntityPose,
    tracked_data::mannequin,
};
use pumpkin_nbt::{compound::NbtCompound, tag::NbtTag};
use pumpkin_protocol::codec::var_int::VarInt;
use pumpkin_util::{math::vector3::Vector3, text::TextComponent};

use crate::entity::{Entity, EntityBase, living::LivingEntity};

/// Vanilla `Mannequin`: an immovable, player-shaped living entity. The client
/// renders it from the profile and model-part metadata, so the server only has
/// to keep those values and the pose in sync.
pub struct MannequinEntity {
    living_entity: LivingEntity,
    profile: Mutex<ProfileImpl>,
    description: Mutex<TextComponent>,
    hide_description: AtomicBool,
    immovable: AtomicBool,
    /// Vanilla `DATA_PLAYER_MODE_CUSTOMISATION`: visible model-part mask.
    visible_layers: AtomicU8,
    /// Vanilla `Avatar.DEFAULT_MAIN_HAND` is right, encoded as 0.
    main_hand: AtomicU8,
}

const ALL_LAYERS: u8 = 0x7f;
const DEFAULT_DESCRIPTION: &str = "entity.minecraft.mannequin.label";

impl MannequinEntity {
    #[must_use]
    pub fn new(entity: Entity) -> Self {
        let living_entity = LivingEntity::new(entity);
        Self {
            living_entity,
            profile: Mutex::new(ProfileImpl::default()),
            description: Mutex::new(TextComponent::translate(DEFAULT_DESCRIPTION, [])),
            hide_description: AtomicBool::new(false),
            immovable: AtomicBool::new(false),
            visible_layers: AtomicU8::new(ALL_LAYERS),
            main_hand: AtomicU8::new(0),
        }
    }

    #[must_use]
    pub fn get_profile(&self) -> ProfileImpl {
        self.profile
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .clone()
    }

    pub fn set_profile(&self, profile: ProfileImpl) {
        *self
            .profile
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner) = profile.clone();
        self.get_entity()
            .set_synced_data(mannequin::DATA_PROFILE, profile);
    }

    #[must_use]
    pub fn is_immovable(&self) -> bool {
        self.immovable.load(Ordering::Relaxed)
    }

    pub fn set_immovable(&self, immovable: bool) {
        self.immovable.store(immovable, Ordering::Relaxed);
        self.get_entity()
            .set_synced_data(mannequin::DATA_IMMOVABLE, immovable);
        self.get_entity()
            .no_physics
            .store(immovable, Ordering::Relaxed);
    }

    fn update_description(&self) {
        let description = if self.hide_description.load(Ordering::Relaxed) {
            None
        } else {
            Some(
                self.description
                    .lock()
                    .unwrap_or_else(std::sync::PoisonError::into_inner)
                    .clone(),
            )
        };
        self.get_entity()
            .set_synced_data(mannequin::DATA_DESCRIPTION, description);
    }

    fn set_description(&self, description: TextComponent) {
        *self
            .description
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner) = description;
        self.update_description();
    }

    fn set_hide_description(&self, hide: bool) {
        self.hide_description.store(hide, Ordering::Relaxed);
        self.update_description();
    }

    fn set_main_hand(&self, main_hand: u8) {
        self.main_hand.store(main_hand, Ordering::Relaxed);
        self.get_entity().set_synced_data(
            mannequin::DATA_PLAYER_MAIN_HAND,
            VarInt(i32::from(main_hand)),
        );
    }

    fn set_visible_layers(&self, layers: u8) {
        self.visible_layers.store(layers, Ordering::Relaxed);
        self.get_entity()
            .set_synced_data(mannequin::DATA_PLAYER_MODE_CUSTOMISATION, layers);
    }

    fn set_pose(&self, pose: EntityPose) {
        self.get_entity().set_pose(pose);
    }
}

const fn pose_name(pose: EntityPose) -> &'static str {
    match pose {
        EntityPose::Crouching => "crouching",
        EntityPose::Swimming => "swimming",
        EntityPose::FallFlying => "fall_flying",
        EntityPose::Sleeping => "sleeping",
        _ => "standing",
    }
}

fn pose_from_name(name: &str) -> EntityPose {
    match name {
        "crouching" => EntityPose::Crouching,
        "swimming" => EntityPose::Swimming,
        "fall_flying" => EntityPose::FallFlying,
        "sleeping" => EntityPose::Sleeping,
        _ => EntityPose::Standing,
    }
}

impl EntityBase for MannequinEntity {
    fn get_entity(&self) -> &Entity {
        &self.living_entity.entity
    }

    fn get_living_entity(&self) -> Option<&LivingEntity> {
        Some(&self.living_entity)
    }

    fn init_data_tracker(&self) {
        let entity = self.get_entity();
        entity.set_synced_data(mannequin::DATA_PROFILE, self.get_profile());
        entity.set_synced_data(mannequin::DATA_IMMOVABLE, self.is_immovable());
        entity.set_synced_data(
            mannequin::DATA_DESCRIPTION,
            if self.hide_description.load(Ordering::Relaxed) {
                None
            } else {
                Some(
                    self.description
                        .lock()
                        .unwrap_or_else(std::sync::PoisonError::into_inner)
                        .clone(),
                )
            },
        );
        entity.set_synced_data(
            mannequin::DATA_PLAYER_MAIN_HAND,
            VarInt(i32::from(self.main_hand.load(Ordering::Relaxed))),
        );
        entity.set_synced_data(
            mannequin::DATA_PLAYER_MODE_CUSTOMISATION,
            self.visible_layers.load(Ordering::Relaxed),
        );
        entity.set_synced_data(mannequin::DATA_POSE, VarInt(entity.pose.load() as i32));
    }

    fn damage(&self, caller: &dyn EntityBase, amount: f32, damage_type: DamageType) -> bool {
        self.living_entity
            .damage_with_context(caller, amount, damage_type, None, None, None)
    }

    fn damage_with_context(
        &self,
        caller: &dyn EntityBase,
        amount: f32,
        damage_type: DamageType,
        position: Option<Vector3<f64>>,
        source: Option<&dyn EntityBase>,
        cause: Option<&dyn EntityBase>,
    ) -> bool {
        self.living_entity
            .damage_with_context(caller, amount, damage_type, position, source, cause)
    }

    fn write_custom_nbt(&self, nbt: &mut NbtCompound) {
        let profile = self.get_profile();
        nbt.put("profile", profile.write_data());
        nbt.put_byte(
            "hidden_layers",
            (ALL_LAYERS & !self.visible_layers.load(Ordering::Relaxed)) as i8,
        );
        nbt.put_string(
            "main_hand",
            if self.main_hand.load(Ordering::Relaxed) == 1 {
                "left".to_owned()
            } else {
                "right".to_owned()
            },
        );
        nbt.put_string("pose", pose_name(self.get_entity().pose.load()).to_owned());
        nbt.put_bool("immovable", self.is_immovable());
        if self.hide_description.load(Ordering::Relaxed) {
            nbt.put_bool("hide_description", true);
        } else {
            let description = self
                .description
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .clone();
            if description != TextComponent::translate(DEFAULT_DESCRIPTION, []) {
                nbt.put(
                    "description",
                    NbtTag::Compound(description.0.to_nbt_compound()),
                );
            }
        }
    }

    fn read_custom_nbt(&self, nbt: &NbtCompound) {
        if let Some(profile) = nbt.get("profile").and_then(ProfileImpl::read_data) {
            self.set_profile(profile);
        }
        if let Some(hidden_layers) = nbt.get_byte("hidden_layers") {
            self.set_visible_layers(ALL_LAYERS & !(hidden_layers as u8));
        }
        if let Some(main_hand) = nbt.get_string("main_hand") {
            self.set_main_hand(u8::from(main_hand == "left"));
        }
        if let Some(pose) = nbt.get_string("pose") {
            self.set_pose(pose_from_name(pose));
        }
        self.set_immovable(nbt.get_bool("immovable").unwrap_or(false));
        self.set_hide_description(nbt.get_bool("hide_description").unwrap_or(false));
        let description = nbt.get("description").map_or_else(
            || TextComponent::translate(DEFAULT_DESCRIPTION, []),
            TextComponent::from_nbt,
        );
        self.set_description(description);
    }

    fn cast_any(&self) -> &dyn std::any::Any {
        self
    }
}
