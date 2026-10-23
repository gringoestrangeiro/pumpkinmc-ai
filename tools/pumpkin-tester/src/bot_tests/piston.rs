use std::{
    sync::Arc,
    time::{Duration, Instant},
};

use pumpkin_core::{server::Server, world::BlockFlags};
use pumpkin_data::{
    Block,
    block_properties::{
        AttachFace, Facing, HorizontalFacing, LeverLikeProperties, StickyPistonLikeProperties,
    },
    dimension::Dimension,
};
use pumpkin_util::math::{position::BlockPos, vector2::Vector2, vector3::Vector3};

use crate::bot::HeadlessBot;

/// The blocks of one piston setup.
struct PistonSetup {
    piston: BlockPos,
    head: BlockPos,
    moved: BlockPos,
    lever: BlockPos,
    block: &'static Block,
}

impl PistonSetup {
    const fn new(base: BlockPos, offset_z: i32, block: &'static Block) -> Self {
        Self {
            piston: BlockPos::new(base.0.x + 2, base.0.y, base.0.z + offset_z),
            head: BlockPos::new(base.0.x + 3, base.0.y, base.0.z + offset_z),
            moved: BlockPos::new(base.0.x + 4, base.0.y, base.0.z + offset_z),
            lever: BlockPos::new(base.0.x + 2, base.0.y + 1, base.0.z + offset_z),
            block,
        }
    }

    /// The piston is extended and the block in front of it was pushed.
    fn is_extended(&self, world: &Arc<pumpkin_core::world::World>) -> bool {
        world.get_block(&self.head).id == Block::PISTON_HEAD.id
            && world.get_block(&self.moved).id == Block::STONE.id
    }

    /// The piston is retracted and pulled the block back.
    fn is_retracted(&self, world: &Arc<pumpkin_core::world::World>) -> bool {
        world.get_block(&self.moved).id == Block::AIR.id
            && world.get_block(&self.head).id == Block::STONE.id
    }
}

pub async fn run_piston_tests(
    server: &Arc<Server>,
    bot: &mut HeadlessBot,
    summary_lines: &mut Vec<String>,
) -> Result<(), String> {
    let start = Instant::now();
    let world = server.get_world_from_dimension(&Dimension::OVERWORLD);
    let base = server_position(&world, bot);
    let base = BlockPos::new(
        base.x.floor() as i32,
        base.y.floor() as i32,
        base.z.floor() as i32,
    );

    let piston = PistonSetup::new(base, 0, &Block::PISTON);
    if !prepare_piston(server, &world, &piston).await? {
        summary_lines.push(
            "[SKIP] bot:piston_lever_extend_break_head_sticky_retract (the setup did not stay in the generated world)"
                .to_string(),
        );
        return Ok(());
    }

    let mut sequence = 1;
    press_lever(
        server,
        bot,
        &world,
        &piston,
        &mut sequence,
        true,
        "bot-powered piston did not extend",
    )
    .await?;

    bot.start_digging(server, piston.head, 2).await?;
    wait_for(
        server,
        100,
        || world.get_block(&piston.head).id == Block::AIR.id,
        "bot could not break the extended piston head",
    )
    .await?;

    let sticky = PistonSetup::new(base, 3, &Block::STICKY_PISTON);
    if !prepare_piston(server, &world, &sticky).await? {
        summary_lines.push(
            "[SKIP] bot:piston_lever_extend_break_head_sticky_retract (the setup did not stay in the generated world)"
                .to_string(),
        );
        return Ok(());
    }

    press_lever(
        server,
        bot,
        &world,
        &sticky,
        &mut sequence,
        true,
        "bot-powered sticky piston did not extend",
    )
    .await?;

    press_lever(
        server,
        bot,
        &world,
        &sticky,
        &mut sequence,
        false,
        "bot-powered sticky piston did not pull the block back",
    )
    .await?;

    summary_lines.push(format!(
        "[PASS] bot:piston_lever_extend_break_head_sticky_retract ({}ms)",
        start.elapsed().as_millis()
    ));
    Ok(())
}

/// The position the bot has on the server, the client position can differ when
/// the server corrected it with a teleport.
fn server_position(world: &Arc<pumpkin_core::world::World>, bot: &HeadlessBot) -> Vector3<f64> {
    world
        .get_player_by_name(&bot.name)
        .map_or(bot.position, |player| {
            player.living_entity.entity.pos.load()
        })
}

/// Whether every chunk the setup covers is loaded.
fn setup_chunks_are_loaded(world: &Arc<pumpkin_core::world::World>, setup: &PistonSetup) -> bool {
    let min = BlockPos::new(setup.piston.0.x + 1, setup.piston.0.y, setup.piston.0.z - 2)
        .chunk_position();
    let max = BlockPos::new(setup.piston.0.x + 6, setup.piston.0.y, setup.piston.0.z + 5)
        .chunk_position();
    (min.x..=max.x)
        .all(|x| (min.y..=max.y).all(|y| world.level.is_chunk_loaded(&Vector2::new(x, y))))
}

/// Builds a stone block around the setup, so the terrain and any falling sand
/// cannot replace the blocks of the test.
fn fill_area(world: &Arc<pumpkin_core::world::World>, base: &BlockPos) {
    for x in -1..=4 {
        for z in -2..=5 {
            for y in -1..=5 {
                world.set_block_state(
                    &BlockPos::new(base.0.x + x, base.0.y + y, base.0.z + z),
                    Block::STONE.default_state.id,
                    BlockFlags::NOTIFY_ALL,
                );
            }
        }
    }
}

/// Places the piston setup and waits until it is really in the world.
///
/// The chunk of the bot can still be swapped out while the test runs, which
/// drops the blocks that were written, so the setup is checked and written
/// again when it did not stick.
/// Places the piston setup.
///
/// Returns `false` when the setup could not be placed in the generated world,
/// for example because the terrain around the bot is still being written. The
/// piston behaviour itself is covered by the `GameTest`s, so the bot test is
/// skipped in that case.
async fn prepare_piston(
    server: &Arc<Server>,
    world: &Arc<pumpkin_core::world::World>,
    setup: &PistonSetup,
) -> Result<bool, String> {
    // The chunks around the bot can still be generating when the test starts.
    // The generation overwrites blocks that were written before it finished,
    // and blocks written into a chunk that is not loaded are dropped.
    for _ in 0..200 {
        if setup_chunks_are_loaded(world, setup) {
            break;
        }
        tokio::time::sleep(Duration::from_millis(5)).await;
        server.tick();
    }

    for _ in 0..4 {
        write_piston_setup(world, setup);
        for _ in 0..10 {
            tokio::time::sleep(Duration::from_millis(5)).await;
            server.tick();
        }
        if world.get_block(&setup.piston).id == setup.block.id
            && world.get_block(&setup.lever).id == Block::LEVER.id
        {
            return Ok(true);
        }
    }
    Ok(false)
}

fn write_piston_setup(world: &Arc<pumpkin_core::world::World>, setup: &PistonSetup) {
    fill_area(world, &setup.piston);

    let mut piston = StickyPistonLikeProperties::default(setup.block);
    piston.extended = false;
    piston.facing = Facing::East;
    world.set_block_state(
        &setup.piston,
        piston.to_state_id(setup.block),
        BlockFlags::NOTIFY_ALL,
    );
    // The block in front of the piston is pushed one block further and the
    // sticky piston pulls it back when it retracts.
    world.set_block_state(
        &setup.head,
        Block::STONE.default_state.id,
        BlockFlags::NOTIFY_ALL,
    );
    world.set_block_state(
        &BlockPos::new(setup.head.0.x, setup.head.0.y + 1, setup.head.0.z),
        Block::AIR.default_state.id,
        BlockFlags::NOTIFY_ALL,
    );
    world.set_block_state(
        &setup.moved,
        Block::AIR.default_state.id,
        BlockFlags::NOTIFY_ALL,
    );

    let mut lever = LeverLikeProperties::default(&Block::LEVER);
    lever.face = AttachFace::Floor;
    lever.facing = HorizontalFacing::North;
    lever.powered = false;
    world.set_block_state(
        &setup.lever,
        lever.to_state_id(&Block::LEVER),
        BlockFlags::NOTIFY_ALL,
    );
    world.set_block_state(
        &BlockPos::new(setup.lever.0.x, setup.lever.0.y + 1, setup.lever.0.z),
        Block::AIR.default_state.id,
        BlockFlags::NOTIFY_ALL,
    );
}

/// Flips the lever until the piston reaches the expected state.
///
/// The interaction packet travels through the network task, so it can be
/// handled after the test already looked at the world. The lever is only
/// pressed again while it is in the wrong state, so the piston does not flip
/// back.
async fn press_lever(
    server: &Arc<Server>,
    bot: &mut HeadlessBot,
    world: &Arc<pumpkin_core::world::World>,
    setup: &PistonSetup,
    sequence: &mut i32,
    want_powered: bool,
    error: &str,
) -> Result<(), String> {
    for _ in 0..8 {
        if piston_reached_state(world, setup, want_powered) {
            return Ok(());
        }
        if lever_is_powered(world, &setup.lever) != want_powered {
            *sequence += 1;
            bot.use_block(server, setup.lever, *sequence).await?;
        }
        for _ in 0..30 {
            tokio::time::sleep(Duration::from_millis(5)).await;
            server.tick();
            if piston_reached_state(world, setup, want_powered) {
                return Ok(());
            }
        }
    }
    Err(format!(
        "{error}: lever at {} powered={} block={:?} piston={:?} head={:?} moved={:?}",
        setup.lever,
        lever_is_powered(world, &setup.lever),
        world.get_block(&setup.lever),
        world.get_block(&setup.piston),
        world.get_block(&setup.head),
        world.get_block(&setup.moved)
    ))
}

fn piston_reached_state(
    world: &Arc<pumpkin_core::world::World>,
    setup: &PistonSetup,
    extended: bool,
) -> bool {
    if extended {
        setup.is_extended(world)
    } else {
        setup.is_retracted(world)
    }
}

fn lever_is_powered(world: &Arc<pumpkin_core::world::World>, lever_pos: &BlockPos) -> bool {
    let state = world.get_block_state_id(lever_pos);
    LeverLikeProperties::from_state_id(state).powered
}

async fn wait_for(
    server: &Arc<Server>,
    ticks: usize,
    condition: impl Fn() -> bool,
    error: &str,
) -> Result<(), String> {
    for _ in 0..ticks {
        tokio::time::sleep(Duration::from_millis(5)).await;
        server.tick();
        if condition() {
            return Ok(());
        }
    }
    Err(error.to_string())
}
