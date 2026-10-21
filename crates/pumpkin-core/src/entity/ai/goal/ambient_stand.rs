use crate::entity::ai::goal::{Controls, Goal};
use crate::entity::mob::Mob;
use crate::entity::passive::horse::AbstractHorse;
use rand::RngExt;

/// Vanilla `RandomStandGoal`: periodically rear and play the ambient stand sound.
#[derive(Default)]
pub struct AmbientStandGoal {
    cooldown: i32,
}

impl AmbientStandGoal {
    fn reset_cooldown(&mut self, mob: &dyn Mob) {
        self.cooldown = -mob.get_ambient_sound_interval();
    }
}

impl Goal for AmbientStandGoal {
    fn should_continue(&mut self, _mob: &dyn Mob) -> bool {
        false
    }

    fn can_start(&mut self, mob: &dyn Mob) -> bool {
        let Some(horse) = mob.as_abstract_horse() else {
            return false;
        };
        self.cooldown += 1;
        if self.cooldown > 0 && mob.get_random().random_range(0..1000) < self.cooldown {
            self.reset_cooldown(mob);
            let immobile = horse.is_horse_standing()
                || horse.is_horse_eating()
                || (mob.get_entity().has_passengers() && mob.is_saddled());
            return !immobile && mob.get_random().random_range(0..10) == 0;
        }
        false
    }

    fn start(&mut self, mob: &dyn Mob) {
        let Some(horse) = mob.as_abstract_horse() else {
            return;
        };
        horse.stand_if_possible();
        if let Some(sound) = horse.get_horse_ambient_stand_sound() {
            let entity = mob.get_entity();
            entity.world.load().play_sound(
                sound,
                pumpkin_data::sound::SoundCategory::Neutral,
                &entity.pos.load(),
            );
        }
    }

    fn should_run_every_tick(&self) -> bool {
        true
    }

    fn controls(&self) -> Controls {
        Controls::empty()
    }
}
