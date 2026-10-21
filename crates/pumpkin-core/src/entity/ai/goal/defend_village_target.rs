use std::sync::Arc;

use pumpkin_data::entity::EntityType;

use super::{Controls, Goal};
use crate::entity::EntityBase;
use crate::entity::ai::goal::track_target::TrackTargetGoal;
use crate::entity::mob::Mob;
use crate::entity::passive::villager::VillagerEntity;
use crate::entity::player::Player;

/// Vanilla `DefendVillageTargetGoal`: an iron golem attacks players a nearby
/// villager dislikes enough.
pub struct DefendVillageTargetGoal {
    track_target_goal: TrackTargetGoal,
    potential_target: Option<Arc<dyn EntityBase>>,
}

impl DefendVillageTargetGoal {
    #[must_use]
    pub fn new() -> Box<Self> {
        Box::new(Self::default())
    }
}

impl Default for DefendVillageTargetGoal {
    fn default() -> Self {
        Self {
            track_target_goal: TrackTargetGoal::with_default(false),
            potential_target: None,
        }
    }
}

impl Goal for DefendVillageTargetGoal {
    fn can_start(&mut self, mob: &dyn Mob) -> bool {
        let entity = mob.get_entity();
        let world = entity.world.load();
        let grow = entity.bounding_box.load().expand(10.0, 8.0, 10.0);
        let villagers = world.get_entities_at_box(&grow);
        let players = world.get_players_at_box(&grow);

        self.potential_target = None;
        for villager_entity in &villagers {
            if villager_entity.get_entity().entity_type != &EntityType::VILLAGER {
                continue;
            }
            let Some(villager) = villager_entity.cast_any().downcast_ref::<VillagerEntity>() else {
                continue;
            };
            for player in &players {
                if villager.get_player_reputation(player.get_entity().entity_uuid) <= -100 {
                    self.potential_target = Some(player.clone() as Arc<dyn EntityBase>);
                }
            }
        }

        let Some(target) = &self.potential_target else {
            return false;
        };
        target
            .cast_any()
            .downcast_ref::<Player>()
            .is_none_or(|player| !player.is_spectator() && !player.is_creative())
    }

    fn should_continue(&mut self, mob: &dyn Mob) -> bool {
        self.track_target_goal.should_continue(mob)
    }

    fn start(&mut self, mob: &dyn Mob) {
        self.track_target_goal
            .set_target_mob(self.potential_target.clone());
        self.track_target_goal.start(mob);
        mob.set_mob_target(self.potential_target.clone());
    }

    fn stop(&mut self, mob: &dyn Mob) {
        self.potential_target = None;
        self.track_target_goal.stop(mob);
    }

    fn controls(&self) -> Controls {
        Controls::TARGET
    }
}
