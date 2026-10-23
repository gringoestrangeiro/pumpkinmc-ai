# Worldgen & mob spawning glitches (triage notes)

Observed by hand on a live PumpkinMC-AI server (Java 26.3, protocol 777, seed
`1794318007682891862`) while comparing against a vanilla 26.3 reference server on the
same seed. Target: Java Edition 26.3.

Each item lists what was seen, where to look, and what to verify. Nothing here has been
fixed. Treat the code pointers as starting points, not confirmed causes, except where
stated.

---

## 1. Lava generated in caves does not flow

**Seen:** a cave wall with a lava source block embedded in stone that was completely
static. After placing a block in front of it and removing that block, the lava started
flowing normally. So the fluid is placed correctly, but nothing ever asked it to flow.

**Screenshot:** the one supplied in the original report (lava source in a dirt/stone cave
wall, not flowing).

**Where to look:**

- `crates/pumpkin-world/src/generation/carver/mod.rs:436-444` — `place_carved_block`.
  The fluid tick is scheduled only when **all three** hold:
  ```rust
  if overworld && should_schedule_fluid_update && state.is_liquid() {
      schedule_fluid_tick_for_state(run.chunk, pos.x, pos.y, pos.z, state);
  }
  ```
- `overworld` is derived from `run.ctx.carver_aquifer.is_some()`
  (`carver/mod.rs:329`, `carver/cave.rs:253`). The Nether has no aquifer, so `overworld`
  is `false` there and **carved lava in the Nether never gets a fluid tick scheduled at
  all**. The same gate also drops lava in the Overworld whenever the aquifer reports
  `should_schedule_fluid_update == false`.
- `crates/pumpkin-world/src/generation/noise/aquifer_sampler.rs:112-120` — the aquifer's
  `should_schedule_fluid_update` flag is the signal that decides this. Check whether the
  flag is set for lava sources (not just water).
- `crates/pumpkin-world/src/generation/structure/structures/mod.rs:441-444` — the
  equivalent call is **commented out**:
  ```rust
  // let fluid_state = world.get_fluid_state(&block_pos);
  // if !fluid_state.is_empty() {
  //     world.schedule_fluid_tick(&block_pos, fluid_state.fluid(), 0);
  // }
  ```
  Lava placed by a structure never starts flowing either.

**Verify:** vanilla schedules a fluid update for every liquid block placed during
generation, including the Nether and structures. Compare vanilla `LiquidBlock.shouldSpreadLiquid`
/ the `neighborUpdate` path against `carver_aquifer`'s `should_schedule_fluid_update`
semantics and make sure lava is covered.

**Note:** the "place a block, remove it, now it flows" behaviour matches vanilla's
neighbour-update path: a block change next to the lava triggers the spread. So the
client-side trigger exists; the *generation-time* scheduling is what's missing.

---

## 2. Mob spawning density is lower than vanilla

**Seen:** noticeably fewer mobs than the vanilla reference server in the same biome.
Passive animals (pigs, sheep, cows) are the clearest case, but hostile spawns also look
thin. Not zero — the world is not empty, just sparse.

**Where to look:**

- `crates/pumpkin-core/src/world/mod.rs:2125-2134` — the passive-spawn gate:
  ```rust
  let spawn_passives = self.get_time_of_day() % 400 == 0;
  ```
  Passive spawning only runs on ticks where `time_of_day % 400 == 0`, i.e. once every
  400 ticks (20 s). Vanilla's passive spawn pass runs on a different cadence. Confirm
  the intended interval against vanilla `NaturalSpawner`.
- `crates/pumpkin-core/src/world/natural_spawner.rs:409-412` — the global cap:
  ```rust
  self.mob_category_counts.0[category.id].load(Relaxed)
      < category.max * self.spawnable_chunk_count / MAGIC_NUMBER
  ```
  with `MAGIC_NUMBER = 17 * 17` (`natural_spawner.rs:29`). `spawnable_chunk_count` is
  `tracker.loaded_active_chunks.len()` (`crates/pumpkin-core/src/world/mod.rs:542`).
  Note `SpawnState::empty()` initialises it to `0` and `set_spawnable_chunk_count` is
  never called, so the value depends entirely on the `update_active_chunks` rebuild.
  If that rebuild is late or the active-chunk set is smaller than vanilla's spawnable
  set, the cap scales down proportionally.
- `crates/pumpkin-core/src/world/natural_spawner.rs:539-556` — `get_random_pos_within`
  picks a Y with `rng.next_inbetween_i32(min_y, temp_y)`, then
  `spawn_category_for_position` (`natural_spawner.rs:725-737`) **bails out if the block
  at that position is solid or a full cube**:
  ```rust
  if state.is_solid_block() || state.is_full_cube() {
      return batch_buffer;
  }
  ```
  A random Y across the full column means most picks land inside stone and are
  discarded. Vanilla instead picks a surface position and searches a small vertical
  window around it. This looks like a significant source of lost spawns.
- `crates/pumpkin-core/src/world/natural_spawner.rs:751-753` — the spawn search walks
  `x += rng().random_range(0..6) - rng().random_range(0..6)`, i.e. steps of -5..+5 per
  iteration, with `max = ceil(random * 4)` iterations (so up to ~3 steps of up to 10
  blocks). Compare the search radius and step count against vanilla's
  `NaturalSpawner.isSpawnPositionOk` / spawn search.

**Verify:** compare mob counts for the same seed and same player position between
Pumpkin and the vanilla 26.3 reference. `pumpkin-gametest` or a bot harness is the
right tool; a manual count of pigs/sheep/cows within a fixed radius in a fixed biome
is enough to start.

---

## 3. Trees at biome borders, and biome rarity

**Seen:** trees appear to spawn on the border between biomes rather than inside a
single biome, and rare biomes seem to show up far more often than they should.

**Caveat:** this is a rough visual impression, not a measurement. It may partly be
vanilla behaviour — vanilla does place some features across biome edges. Verify before
changing anything.

**Where to look:**

- `crates/pumpkin-world/src/generation/feature/placed_features.rs:510-530` —
  `BiomePlacementModifier` decides whether a feature may be placed by checking whether
  the placed feature is in `biome.features`. This is per-block-position, so a tree whose
  trunk lands on the boundary biome will use that biome's feature list.
- `crates/pumpkin-world/src/generation/biome.rs` — `get_biome_blend` handles biome
  boundary blending. Check the blend radius and how the blend is sampled against
  vanilla's `BiomeManager`/`getBiomeBlend`.
- `crates/pumpkin-world/src/biome/position_finder.rs` and
  `crates/pumpkin-world/src/biome/multi_noise.rs` — climate sampling and
  `quantize_coord`. The rarity of rare biomes comes from the climate sampler mapping
  (temperature / continentalness / erosion / depth / weirdness → biome), so suspect the
  noise settings or the quantisation step here.

**Verify:** sample the biome at a grid of positions on both servers with the same seed
and diff the results. The repo has `.vanilla-26.3/vanilla_commands` and
`.vanilla-26.3/vanilla_mobs` with decompiled vanilla sources for comparison, and
`crates/pumpkin-world/src/generation/proto_chunk_test/` has parity-test scaffolding
(`biome_hashes.rs`, `stage_parity.rs`) worth reusing.

---

## Notes for whoever picks this up

- The live server used for these observations is still running: Java on `25577`,
  Bedrock on `19140`, log at `/tmp/opencode/pumpkin-fresh/server.log`, console via
  `printf 'cmd\n' > /tmp/opencode/pumpkin-fresh/console.in`. A vanilla 26.3 reference
  server is on `25566` in `/tmp/opencode/vanilla` with the same seed.
- `/tmp/opencode/console.sh <fifo> <log> <command...>` sends console commands and
  prints only the new log lines.
- These are all gameplay-visible changes, so they need a real-client play test and a
  capture before they can go up as a PR (see `AGENTS.md`, "Pull requests").
- The known boat-physics test failure in `tools/pumpkin-tester` (`boat did not become
  submerged`) is pre-existing and unrelated, but it aborts the tester suite before the
  GameTest phase runs, so it will get in the way of automated verification.
