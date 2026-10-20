use crate::entity::vehicle::boat::BoatEntity;

#[allow(clippy::wildcard_imports)]
use super::*;

impl JavaClient {
    pub fn handle_player_command(
        &self,
        player: &Arc<Player>,
        command: &SPlayerCommand,
        server: &Arc<Server>,
    ) {
        if command.entity_id != player.entity_id().into() {
            return;
        }
        if !player.has_client_loaded() {
            return;
        }
        player.update_last_action_time();

        let entity = &player.get_entity();
        match command.action {
            Action::StartSprinting => {
                if !entity.is_sprinting() {
                    send_cancellable_blocking! {{
                        server;
                        PlayerToggleSprintEvent::new(player.clone(), true);
                        'after: {
                            player.set_sprinting(event.is_sprinting);
                            player.update_player_pose();
                        }
                    }}
                }
            }
            Action::StopSprinting => {
                if entity.is_sprinting() {
                    send_cancellable_blocking! {{
                        server;
                        PlayerToggleSprintEvent::new(player.clone(), false);
                        'after: {
                            player.set_sprinting(event.is_sprinting);
                            player.update_player_pose();
                        }
                    }}
                }
            }
            Action::LeaveBed => player.wake_up(),

            Action::OpenVehicleInventory => {
                let vehicle = player
                    .living_entity
                    .entity
                    .vehicle
                    .lock()
                    .unwrap_or_else(std::sync::PoisonError::into_inner)
                    .clone();
                if let Some(vehicle) = vehicle
                    && let Some(boat) = vehicle.cast_any().downcast_ref::<BoatEntity>()
                {
                    boat.open_inventory(player);
                }
            }
            Action::StartHorseJump | Action::StopHorseJump => {
                debug!("todo");
            }
            // Vanilla `Player#tryToStartFallFlying`.
            Action::StartFlyingElytra => {
                if !entity.is_fall_flying()
                    && !player.living_entity.is_in_liquid()
                    && player.living_entity.can_glide(player.as_ref())
                {
                    let mut event = crate::plugin::api::events::entity::entity_toggle_glide::EntityToggleGlideEvent::new(
                        entity.entity_id,
                        true,
                    );
                    server.plugin_manager.fire_blocking(server, &mut event);
                    if !event.cancelled && event.is_gliding {
                        entity.set_fall_flying(true);
                    }
                }
            }
            // <= 1.21.5
            Action::StartSneaking | Action::StopSneaking => {
                self.handle_player_input(
                    player,
                    &SPlayerInput {
                        input: SPlayerInput::SNEAK,
                    },
                    server,
                );
            }
        }
    }
}
