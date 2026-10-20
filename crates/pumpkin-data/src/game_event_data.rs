use crate::game_event::GameEvent;
use crate::tag::{RegistryKey, Taggable};

/// `VibrationSystem.NO_VIBRATION_FREQUENCY`.
pub const NO_VIBRATION_FREQUENCY: i32 = 0;

/// `GameEvent.DEFAULT_NOTIFICATION_RADIUS`.
pub const DEFAULT_NOTIFICATION_RADIUS: i32 = 16;

const JUKEBOX_NOTIFICATION_RADIUS: i32 = 10;
const SHRIEK_NOTIFICATION_RADIUS: i32 = 32;

impl Taggable for GameEvent {
    fn tag_key() -> RegistryKey {
        RegistryKey::GameEvent
    }

    fn registry_key(&self) -> &str {
        self.name()
    }

    fn registry_id(&self) -> u16 {
        *self as u16
    }
}

impl GameEvent {
    /// `VibrationSystem.VIBRATION_FREQUENCY_FOR_EVENT`.
    #[must_use]
    pub const fn vibration_frequency(self) -> i32 {
        match self {
            Self::Step | Self::Swim | Self::Flap => 1,
            Self::ProjectileLand | Self::HitGround | Self::Splash | Self::Bounce => 2,
            Self::ItemInteractFinish | Self::ProjectileShoot | Self::InstrumentPlay => 3,
            Self::EntityAction | Self::ElytraGlide | Self::Unequip => 4,
            Self::EntityDismount | Self::Equip => 5,
            Self::EntityInteract | Self::Shear | Self::EntityMount => 6,
            Self::EntityDamage => 7,
            Self::Drink | Self::Eat => 8,
            Self::ContainerClose | Self::BlockClose | Self::BlockDeactivate | Self::BlockDetach => {
                9
            }
            Self::ContainerOpen
            | Self::BlockOpen
            | Self::BlockActivate
            | Self::BlockAttach
            | Self::PrimeFuse
            | Self::NoteBlockPlay => 10,
            Self::BlockChange => 11,
            Self::BlockDestroy | Self::FluidPickup => 12,
            Self::BlockPlace | Self::FluidPlace => 13,
            Self::EntityPlace | Self::LightningStrike | Self::Teleport => 14,
            Self::EntityDie | Self::Explode => 15,
            Self::Resonate1 => 1,
            Self::Resonate2 => 2,
            Self::Resonate3 => 3,
            Self::Resonate4 => 4,
            Self::Resonate5 => 5,
            Self::Resonate6 => 6,
            Self::Resonate7 => 7,
            Self::Resonate8 => 8,
            Self::Resonate9 => 9,
            Self::Resonate10 => 10,
            Self::Resonate11 => 11,
            Self::Resonate12 => 12,
            Self::Resonate13 => 13,
            Self::Resonate14 => 14,
            Self::Resonate15 => 15,
            _ => NO_VIBRATION_FREQUENCY,
        }
    }

    /// `GameEvent#notificationRadius`.
    #[must_use]
    pub const fn notification_radius(self) -> i32 {
        match self {
            Self::JukeboxPlay | Self::JukeboxStopPlay => JUKEBOX_NOTIFICATION_RADIUS,
            Self::Shriek => SHRIEK_NOTIFICATION_RADIUS,
            _ => DEFAULT_NOTIFICATION_RADIUS,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tag::GameEvent as GameEventTags;

    #[test]
    fn tags_match_the_game_event_registry_ids() {
        assert!(GameEvent::Step.has_tag(&GameEventTags::MINECRAFT_VIBRATIONS));
        assert!(!GameEvent::Shriek.has_tag(&GameEventTags::MINECRAFT_VIBRATIONS));
        assert!(GameEvent::Shriek.has_tag(&GameEventTags::MINECRAFT_WARDEN_CAN_LISTEN));
        assert!(
            GameEvent::SculkSensorTendrilsClicking
                .has_tag(&GameEventTags::MINECRAFT_SHRIEKER_CAN_LISTEN)
        );
    }
}
