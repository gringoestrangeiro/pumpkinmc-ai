# Parity Tracking

**Last updated:** 2026-10-20
**Fork code commit:** `bf5edaf97`
**Target:** Java Edition 26.3. Bedrock 1.26.51 is a separate workstream.

## Headline

The public PumpkinMC tracker had **564 entries: 402 done, 155 partial, 7 planned**. Two of those planned rows are `/publish` and `/unpublish`, which vanilla registers only for the integrated server. They are excluded from the dedicated-server target, so the in-scope baseline is **562 entries: 402 done, 155 partial, 5 planned**, or **71.5%**.

This fork now has **415 done, 143 partial, 4 planned**, or **73.8%**.

| Metric | Upstream | This fork | Delta |
| :--- | ---: | ---: | ---: |
| In-scope tracker entries | 562 | 562 | 0 |
| Done | 402 | 415 | +13 |
| Partial | 155 | 143 | -12 |
| Planned | 5 | 4 | -1 |
| Strict completion, done only | 71.5% | 73.8% | +2.3 pp |
| Weighted completion, partial = 0.5 | 85.3% | 86.6% | +1.3 pp |

## What the percentage means

This is a **tracker completion rate**, not a measured behavioral parity score.

The tracker's green entries mean "an implementation source exists". They do not prove vanilla behavior. The 1.0 roadmap treats every green entry as unverified until the `T01` audit, and two entries touched by this batch were green while the behavior did not work:

- `Sculk Shrieker` was green, but shriekers did not respond to vibrations.
- `Sculk sensors, shriekers and calibrated sensors` was green, but `SculkSensorBlock::trigger` had no callers.

Commit `798734a3a` repairs both paths. They stay counted as done, but they now have code, game rules, and dispatch tests behind them.

The **73.8% is therefore an upper bound** until the green entries are audited. The honest verified number is lower. What is known for certain is that **147 tracker rows are still open** (143 partial and 4 planned).

## Per-category status

| Category | Entries | Done | Partial | Planned | Done % |
| :--- | ---: | ---: | ---: | ---: | ---: |
| Blocks | 196 | 163 | 31 | 2 | 83.2% |
| Entities | 160 | 59 | 101 | 0 | 36.9% |
| Commands | 94 | 92 | 1 | 1 | 97.9% |
| Items | 55 | 51 | 4 | 0 | 92.7% |
| Redstone | 27 | 24 | 3 | 0 | 88.9% |
| World | 16 | 13 | 3 | 0 | 81.2% |
| Combat | 14 | 13 | 0 | 1 | 92.9% |
| **Total** | **562** | **415** | **143** | **4** | **73.8%** |

Entities are still the dominant gap. They are 101 of the 147 open rows, which is why mob AI work moves the tracker percentage much more than block edge cases do.

## Rows closed or repaired by this fork

| Tracker row | Before | Work | Evidence |
| :--- | :--- | :--- | :--- |
| Entities / Passive / Bat | partial | Bat AI parity pass | `ba3743a2e`, `cargo nextest -p pumpkin-core` |
| Blocks / Blocks / Structure Block | partial | Structure block save and load | `3ee95ba18`, template capture/placement tests |
| Entities / Hostile / Sulfur Cube | partial | Sulfur cube AI and synced size | `fb850f755` |
| Commands / Vanilla / `/perf` | planned | `/perf start` and `/perf stop` ported from PR `#2958` | `ad8a17bcb` |
| Entities / Hostile / Bogged | partial | Bogged mechanics and skeleton parity improvements | `348d5a01d` |
| Entities / Passive / Cat | partial | Feline goal parity | `f873c74f7` |
| Entities / Passive / Ocelot | partial | Feline goal parity | `f873c74f7` |
| Entities / Hostile / Silverfish | partial | Complete silverfish goal set | `9bdff4525` |
| Entities / Hostile / Phantom | partial | Circle and swoop attack AI | `22293ae26` |
| Entities / Passive / Pufferfish | partial | Puff, sting, and fish swimming | `89599fe54` |
| Entities / Passive / Squid | partial | Vanilla swimming and jetting | `863285f7a` |
| Entities / Passive / Glow Squid | partial | Vanilla swimming and jetting | `863285f7a` |
| Entities / Decoration / Leash Knot | partial | Leash persistence and knot destruction | `996224bb0` |
| Blocks / Fire & Sculk / Sculk Shrieker | done, unverified | Vibration dispatch, shrieker response | `798734a3a` |
| Redstone / Power & Sensors / Sculk sensors, shriekers and calibrated sensors | done, unverified | Frequency table, listener radius, calibrated filter, step path | `798734a3a` |

The Warden row remains partial. `798734a3a` adds vibration hearing and `a227a7212` adds anger management, but sniffing and the sonic boom are still missing.

Several rows improved without closing:

- Enderman: `4ebb09882` and `7633c4a8b` fix target-death aggression and carried-block placement; the row still needs a full audit.
- Ender Dragon: `2a4fd3b44` registers the hitbox parts; part-based contact/knockback is still untested.
- Chest boats: `77ef9dd4c` adds the inventory and container interaction; water-status/buoyancy physics is still missing, so the 11 chest-boat rows stay partial.
- Fish: `863285f7a` fixes swimming, panic, and land flopping; schooling and drying out remain, so Cod, Salmon, and Tropical Fish stay partial.

## Wave 1 (2026-10-20)

Wave 1 targets one ported fix and a set of rows the tracker already lists as open. The table records implementation status against vanilla 26.3, not tracker count changes; the totals above stay pending until the Wave 1 worktrees merge. Checks marked **verify-only** need a live run, not more code reading.

| Row | Status | Evidence | Still needed |
| :--- | :--- | :--- | :--- |
| Commands / `/locate` height bug | fixed, verify-only | Port of upstream PR `#4029` in `crates/pumpkin-world/src/generation/generator/structure_finder.rs`: located starts now use `NoiseHeightSampler` and the generator's biome supplier, matching `set_structure_starts` in `proto_chunk.rs` and vanilla `ChunkGenerator.getStructureGeneratingAt`. The existing regression test at `structure_finder.rs:439` compares locate-first and generation-first starts. | `/locate structure minecraft:village_snowy` on ungenerated chunks, teleport, confirm the village sits on the surface and not under water. |
| Items / Name Tag | done, verify-only | `item/items/name_tag.rs:20-34` sets `CustomName`, sets `PersistenceRequired` for mobs and decrements unless creative, matching vanilla `NameTagItem.interactLivingEntity`; vanilla's creative path uses a copy of the stack in `Player.interactOn:806-808`, so the guard is equivalent. | Name a mob, relog, confirm the name and persistence survive, and the tag is consumed in survival but not creative. |
| Blocks / Shulker Box | partial | `block/blocks/shulker_box.rs:73-100` opens the container, but there is no vanilla `canOpen` lid-space check and no `PiglinAi.angerNearbyPiglins` call (`ShulkerBoxBlock.java:68-82`). The tracker premise is inverted: vanilla returns `true` when the animation is not `CLOSED`, then checks for a clear lid. | Place a solid block in the lid space; vanilla refuses to open, Pumpkin currently opens. |
| Blocks / Structure Block | done for the tracker gap, partial as a block | The tracker's "capture/save/load execution" gap is implemented: save and load actions work (`net/java/play/set_structure_block.rs:146-171`, helpers at `178-284`) and the block entity NBT round-trips. Still missing: corner-mode `ACTION_SCAN_AREA` and redstone-powered execution; `block/blocks/structure_block.rs:18-30` only opens the GUI. | Save a structure, load it elsewhere, then test corner DETECT and a powered structure block. |
| Commands / `/locate` coverage | partial | Structure, biome and POI branches exist (`command/commands/locate.rs`). Datapack structure ids are rejected (`wanted_structures` requires `is_vanilla()`), structure sets disabled through `enabled_structure_sets` are not filtered, flat and custom generators return not found (`structure_finder.rs:143`), and biome sampling uses a different ring order than vanilla `BlockPos.spiralAround` (`biome_finder.rs:84-110`). | Live check of all three branches, plus a datapack structure and a superflat world. |
| World / loading | done, verify-only | `level.rs:150-176` resolves canonical `dimensions/<namespace>/<name>` and falls back to `region`, `DIM-1`, `DIM1`; both paths have tests at `level.rs:1043-1096`. `world_info/anvil.rs:361-420` reads `level.dat`, checks data and level versions, and reads the seed. | Import a vanilla 26.3 world in both the canonical and legacy layouts. |
| World / saving | partial, verify-only | `save_all` (`server/mod.rs:595-618`) writes world info, players and advancements, then calls `world.save()`. `stop` saves block entities and POI before `level.shutdown` (`world/mod.rs:599-624`). `save-all flush` uses the same executor as `save-all` and does not wait for chunk writes, and `add_block_entity_nbt` still drops writes for chunks that are no longer loaded (`world/mod.rs:6373-6389`). | A/B `#3902`: fresh area, chest with items, `stop` without `save-all`, restart, reopen the chest. |

### Wave 1 code merged, live check pending

The Wave 1 code merge (`bf5edaf97`) implements 49 tracker rows. They stay **partial** in the headline until the live checks pass. If every check below passes, the counts become **464 done / 94 partial / 4 planned = 82.6%**; the current verified headline stays 415 / 143 / 4.

| Cluster | Rows | Evidence | Live check |
| :--- | ---: | :--- | :--- |
| Boats, chest boats, bamboo rafts | 22 | Port of upstream `#4073` in `entity/vehicle/boat/{physics,passengers,dismount,block_effects}.rs`; chest inventory preserved; chest-boat fallback to the container when riding fails. | Float at the surface, paddle forward/turn, sink and eject at 60 ticks, dismount on land/water, two riders in a boat, one in a chest boat, chest contents survive relog, lily-pad breakage. |
| Zombie, Zoglin, Evoker, Vindicator, Pillager, Husk, Wolf, Armadillo, Polar Bear, Stray, Endermite, Wither Skeleton, Parched | 13 | Baby zombie speed/XP, living targets, illager revenge/Johnny, husk hunger and sun immunity, wolf sit/leap, armadillo roll, polar-bear cub defence, stray/parched arrow effects, endermite persistence, wither-skeleton equipment/targets. | Bot fights and summon checks listed in the Wave 1 handoff; compare against vanilla behavior in `PARITY.md` notes. |
| Note Block (blocks + redstone), Powered Rail, Activator Rail, Pumpkin & Melon Stems, Chiseled Bookshelf, Bell, Jukebox, Ender Chest, Big Dripleaf, Sugar Cane, Campfire | 12 | Game events, rail dedupe, stem support tag, used-item statistic, bell/jukebox events, piglin anger, projectile tilt/ignition, fluid-aware cane support. | Note-block game event, 8-rail power boundary, fruit on podzol/moss/mud, bookshelf stat, bell/sculk event, comparator update, piglin anger, projectile tilt/ignition, cane beside water. |

Name Tag and world loading are already listed as done/verify-only above. Structure Block was already counted done in the previous batch. The remaining Wave 1 checks are `/locate` live coverage and the `#3902` save A/B; both stay partial until they pass.

## Command parity audit

Vanilla 26.3 registers **87 root commands** for a dedicated server once the dev-only commands are excluded. Pumpkin registers **86** of them.

| Measure | Result |
| :--- | ---: |
| Root coverage against the dedicated target | 86 / 87 = 98.9% |
| Root coverage if `/jfr` is a non-goal | 86 / 86 = 100% |

Root coverage is not subcommand coverage. A full subcommand and behavior audit against vanilla 26.3 is still pending; `/debug function` and `/version` are known gaps from this pass.

### Actually missing on a dedicated server

| Command | State | Notes |
| :--- | :--- | :--- |
| `/debug function` | missing subcommand | Pumpkin has `/debug start` and `/debug stop`; the vanilla function tracer is absent. |
| `/version` | present, wrong behavior | `/version` is an alias of `/pumpkin`, which prints project and contributor info. Vanilla prints game version, data version, protocol, build time, and pack versions. Split it into a real `/version` or document the deviation. |
| `/jfr start`, `/jfr stop` | missing | JVM Flight Recorder tooling, not gameplay. Vanilla 26.3 registers only `start` and `stop`; there is no `/jfr dump` subcommand. Recommend a documented non-goal; operators can use JVM flags or `jcmd`. |

`/locate` is marked partial by the tracker, but the current code implements structure, biome, and POI searches. It needs a live verification before the row is moved to done.

### Excluded from the parity target

`/publish` and `/unpublish` are not part of this count. Vanilla registers them only for the integrated server (`CommandSelection.includeIntegrated`), and a dedicated server must not expose them. They were removed from the denominator instead of being treated as missing commands.

Dev-only commands gated by `SharedConstants.DEBUG_DEV_COMMANDS`, `DEBUG_CHASE_COMMAND`, or running in the IDE are also out of scope: `/chase`, `/debugpath`, `/debugmobspawning`, `/wardenspawntracker`, `/spawnarmortrims`, `/serverpack`, `/debugconfig`. `/emote` is not a vanilla command; `/me` is, and Pumpkin implements it.

Pumpkin extras that are not parity gaps: `/pumpkin`, `/plugin`, `/plugins`, `/tps`, and `/raid`. Vanilla only registers `/raid` in dev mode, so it can stay as a Pumpkin debug extra or be gated later.

### Effect on the tracker

The raw tracker had 564 rows. Removing `/publish` and `/unpublish` gives the 562-row in-scope baseline. Porting `/perf` moves one planned command row to done. The current total is **415/562 = 73.8%**. Verifying `/locate` would make it **416/562 = 74.0%**, and treating `/jfr` as a non-goal would give **416/561 = 74.2%**.

## How to update this file

1. Use the tracker snapshot that matches the date in this file. The current baseline is the 562-row in-scope snapshot derived from the 564-row public snapshot collected on 2026-10-20.
2. Change a row from `partial` or `planned` to `done` only when the matching roadmap task's **Done** line is met and verified. A source file appearing is not enough.
3. If a green row is found to be false, record it in this file and either fix it or move it back to partial in the tracker. Do not let green rows hide gaps.
4. Recompute the counts and percentages. Keep the date and fork commit current.
5. Keep the weighted number as a secondary signal. The primary number is done-only, because that is what the public 71% figure uses.

## Open gaps by workstream

The 143 partial and 4 planned rows are mapped to roadmap tasks in Appendix B. The largest clusters are:

- **Entities:** 101 partial rows, mostly mob AI, pathfinding, and missing interactions.
- **Blocks:** 31 partial and 2 planned rows, mostly block-specific edge cases, redstone details, and world interaction.
- **Redstone:** 3 partial rows.
- **World:** 3 partial rows.
- **Items:** 4 partial rows.
- **Commands:** 1 partial (`/locate`) and 1 planned (`/jfr`) row. `/jfr` is a recommended non-goal, so `/locate` is the only remaining command verification.

Closing block and entity rows is the fastest way to move the strict percentage. Closing an entity row is worth about 0.18 pp; closing the whole entity gap is worth about 18.0 pp (101 / 562).
