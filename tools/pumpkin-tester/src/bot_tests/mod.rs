mod aquatic_goals;
mod block_tuning_and_invert;
mod commands_and_gamemode;
mod happy_ghast;
mod illager_revenge;
mod iron_golem_defence;
mod knockback;
mod login_and_movement;
mod panda_genes;
mod parrot_taming;
mod piston;
mod player_actions;
mod setup;
mod sniffer;
mod status_ping;
mod vehicle;
mod world_interaction;

use std::{net::SocketAddr, sync::Arc};

use pumpkin_core::server::Server;
use pumpkin_gametest::GameTestRunner;

pub async fn run_bot_tests(
    server: &Arc<Server>,
    addr: SocketAddr,
    runner: &mut GameTestRunner,
    summary_lines: &mut Vec<String>,
) -> Result<(), String> {
    status_ping::run_bot_status_ping_test(addr, summary_lines).await?;

    let (mut bot1, bot2) =
        login_and_movement::run_login_and_movement_tests(server, addr, summary_lines).await?;

    commands_and_gamemode::run_commands_and_gamemode_tests(
        server,
        &mut bot1,
        runner,
        summary_lines,
    )
    .await?;

    knockback::run_knockback_tests(server, &mut bot1, summary_lines).await?;

    world_interaction::run_world_interaction_tests(server, &mut bot1, summary_lines).await?;

    vehicle::run_vehicle_tests(server, &mut bot1, summary_lines).await?;

    illager_revenge::run_illager_revenge_tests(server, &mut bot1, summary_lines).await?;

    iron_golem_defence::run_iron_golem_defence_tests(server, &mut bot1, summary_lines).await?;

    aquatic_goals::run_nautilus_ride_tests(server, &mut bot1, summary_lines).await?;
    aquatic_goals::run_axolotl_dry_out_tests(server, &bot1, summary_lines).await?;
    aquatic_goals::run_dolphin_got_fish_tests(server, &mut bot1, summary_lines).await?;

    panda_genes::run_panda_genes_tests(server, &mut bot1, summary_lines).await?;

    happy_ghast::run_happy_ghast_tests(server, &mut bot1, summary_lines).await?;

    parrot_taming::run_parrot_taming_tests(server, &mut bot1, summary_lines).await?;
    sniffer::run_sniffer_dig_tests(server, &mut bot1, summary_lines).await?;
    piston::run_piston_tests(server, &mut bot1, summary_lines).await?;

    block_tuning_and_invert::run_block_tuning_and_invert_tests(server, &mut bot1, summary_lines)
        .await?;

    player_actions::run_player_actions_and_disconnect_tests(server, bot1, bot2, summary_lines)
        .await?;

    Ok(())
}
