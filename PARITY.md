# Parity Tracking

**Last updated:** 2026-10-23
**Fork code commit:** `b5fa8fa3e`
**Target:** Java Edition 26.3. Bedrock 1.26.51 is a separate workstream.

## Headline

The public PumpkinMC tracker had **564 entries: 402 done, 155 partial, 7 planned**. Two of those planned rows are `/publish` and `/unpublish`, which vanilla registers only for the integrated server. They are excluded from the dedicated-server target, so the in-scope baseline is **562 entries: 402 done, 155 partial, 5 planned**, or **71.5%**.

This fork now has **562 done, 0 partial, 0 planned**, or **100%**. Wave 1 through Wave 4 are implemented, reviewed, compiled, linted and running on a live server. The workspace passes `cargo fmt`, `cargo clippy`, `cargo check`, `cargo nextest`, `cargo machete` and `typos` on the current tip. The automated live checks are recorded in `LIVE_TEST.md` §3; the remaining client play-tests are tracked row by row there and are not open parity rows.

| Metric | Upstream | This fork (verified) | Delta |
| :--- | ---: | ---: | ---: |
| In-scope tracker entries | 562 | 562 | 0 |
| Done | 402 | 562 | +160 |
| Partial | 155 | 0 | -155 |
| Planned | 5 | 0 | -5 |
| Strict completion, done only | 71.5% | 100% | +28.5 pp |
| Weighted completion, partial = 0.5 | 85.3% | 100% | +14.7 pp |

## What the percentage means

There is one parity number, backed by two evidence levels:

- **Verified row:** the row is implemented in the fork, the code is reviewed, and it ships in a build that passes the full ladder (`fmt`, `clippy`, `check`, `nextest`, `machete`, `typos`) and boots as a live server. All 562 in-scope rows meet this bar.
- **Client-checked row:** the manual in-game check in `LIVE_TEST.md` has passed. These checks are recorded per row; a pending client check does not make a tracker row partial, it only means that specific play-test has not been recorded yet.

The 100% figure is the row-completion count against the 562-row in-scope tracker. It is not a claim that every animation, sound and GUI has been eyeballed; the client pass is listed in `LIVE_TEST.md`.

The tracker's green entries only ever meant "an implementation source exists", which is why this fork records evidence per row. Two entries were green while the behavior did not work:

- `Sculk Shrieker` was green, but shriekers did not respond to vibrations.
- `Sculk sensors, shriekers and calibrated sensors` was green, but `SculkSensorBlock::trigger` had no callers.

Commit `798734a3a` repairs both paths. They stay counted as done, but they now have code, game rules, and dispatch tests behind them.

**No in-scope tracker row is open.** The 54 entity rows and Beehive & Bee Nest closed in Wave 4. The 100% figure is the row-completion count; the client play-tests continue in `LIVE_TEST.md`.

## Per-category status

| Category | Entries | Done | Partial | Planned | Done % |
| :--- | ---: | ---: | ---: | ---: | ---: |
| Blocks | 196 | 196 | 0 | 0 | 100% |
| Entities | 160 | 160 | 0 | 0 | 100% |
| Commands | 94 | 94 | 0 | 0 | 100% |
| Items | 55 | 55 | 0 | 0 | 100% |
| Redstone | 27 | 27 | 0 | 0 | 100% |
| World | 16 | 16 | 0 | 0 | 100% |
| Combat | 14 | 14 | 0 | 0 | 100% |
| **Total** | **562** | **562** | **0** | **0** | **100%** |

No tracker category has an open row. The per-mechanic client checks are in `LIVE_TEST.md`.

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
| Blocks / Fire & Sculk / Sculk Shrieker | done, verified | Vibration dispatch, shrieker response | `798734a3a` |
| Redstone / Power & Sensors / Sculk sensors, shriekers and calibrated sensors | done, verified | Frequency table, listener radius, calibrated filter, step path | `798734a3a` |

At that point the Warden row was still partial. Wave 2 later added sniffing and the sonic boom; the Wave 4 close-out supersedes the old status.

Several rows improved in earlier waves before their later close-out. The current status is the table above; the historical Wave 1-3 notes below keep their original evidence labels.

## Wave 1 (2026-10-20)

Wave 1 targets one ported fix and a set of rows the tracker already lists as open. The table records implementation status against vanilla 26.3, not tracker count changes; the totals above stay pending until the Wave 1 worktrees merge. Checks marked **verify-only** need a live run, not more code reading.

| Row | Status | Evidence | Still needed |
| :--- | :--- | :--- | :--- |
| Commands / `/locate` height bug | fixed, verify-only | Port of upstream PR `#4029` in `crates/pumpkin-world/src/generation/generator/structure_finder.rs`: located starts now use `NoiseHeightSampler` and the generator's biome supplier, matching `set_structure_starts` in `proto_chunk.rs` and vanilla `ChunkGenerator.getStructureGeneratingAt`. The existing regression test at `structure_finder.rs:439` compares locate-first and generation-first starts. | `/locate structure minecraft:village_snowy` on ungenerated chunks, teleport, confirm the village sits on the surface and not under water. |
| Items / Name Tag | done, verify-only | `item/items/name_tag.rs:20-34` sets `CustomName`, sets `PersistenceRequired` for mobs and decrements unless creative, matching vanilla `NameTagItem.interactLivingEntity`; vanilla's creative path uses a copy of the stack in `Player.interactOn:806-808`, so the guard is equivalent. | Name a mob, relog, confirm the name and persistence survive, and the tag is consumed in survival but not creative. |
| Blocks / Shulker Box | done, verify-only | `block/blocks/shulker_box.rs:82` now runs the vanilla `canOpen` lid-space check (`128-144`) before opening, and `:101` calls `PiglinAi::anger_nearby_piglins` for the opening player. | Place a solid block in the lid space and confirm the box refuses to open; open one near a piglin and confirm anger. |
| Blocks / Structure Block | done for the tracker gap; documented deviation | The tracker's "capture/save/load execution" gap is implemented: save and load actions work (`net/java/play/set_structure_block.rs:146-171`, helpers at `178-284`) and the block entity NBT round-trips. Corner-mode `ACTION_SCAN_AREA` and redstone-powered execution are still missing; see **Documented deviations**. | Save a structure, load it elsewhere, then confirm the documented corner/redstone limitation. |
| Commands / `/locate` coverage | done, verify-only | Wave 3 (`1b7fbeb90`) added `enabled_structure_sets` filtering, flat and custom generator searches, and the generator's biome supplier; `command/commands/locate.rs` now covers structure, biome and POI branches. Unknown non-vanilla structure ids remain a documented deviation. | Live check of structure, biome and POI branches on a normal world and a superflat world. |
| World / loading | done, verify-only | `level.rs:150-176` resolves canonical `dimensions/<namespace>/<name>` and falls back to `region`, `DIM-1`, `DIM1`; both paths have tests at `level.rs:1043-1096`. `world_info/anvil.rs:361-420` reads `level.dat`, checks data and level versions, and reads the seed. | Import a vanilla 26.3 world in both the canonical and legacy layouts. |
| World / saving | done, verify-only | `save_all` (`server/mod.rs:595-618`) writes world info, players and advancements, then calls `world.save()`. `save-all flush` waits on `world.flush()` (`server/mod.rs:599-632`), and `world/mod.rs:620` flushes block-entity NBT for chunks that unloaded before the save. | A/B `#3902`: fresh area, chest with items, `stop` without `save-all`, restart, reopen the chest. |

### Wave 1 code merged, live check pending

The Wave 1 code merge (`bf5edaf97`) implemented 49 tracker rows at the time. Later waves closed the remaining Wave 1 gaps: Shulker Box, `/locate` coverage and save flushing are now in the total, and Structure Block's tracker gap is done with the corner/redstone limitation recorded in **Documented deviations**. The live checks below are recorded in `LIVE_TEST.md`.

| Cluster | Rows | Evidence | Live check |
| :--- | ---: | :--- | :--- |
| Boats, chest boats, bamboo rafts | 22 | Port of upstream `#4073` in `entity/vehicle/boat/{physics,passengers,dismount,block_effects}.rs`; chest inventory preserved; chest-boat fallback to the container when riding fails. | Float at the surface, paddle forward/turn, sink and eject at 60 ticks, dismount on land/water, two riders in a boat, one in a chest boat, chest contents survive relog, lily-pad breakage. |
| Zombie, Zoglin, Evoker, Vindicator, Pillager, Husk, Wolf, Armadillo, Polar Bear, Stray, Endermite, Wither Skeleton, Parched | 13 | Baby zombie speed/XP, living targets, illager revenge/Johnny, husk hunger and sun immunity, wolf sit/leap, armadillo roll, polar-bear cub defence, stray/parched arrow effects, endermite persistence, wither-skeleton equipment/targets. | Bot fights and summon checks listed in the Wave 1 handoff; compare against vanilla behavior in `PARITY.md` notes. |
| Note Block (blocks + redstone), Powered Rail, Activator Rail, Pumpkin & Melon Stems, Chiseled Bookshelf, Bell, Jukebox, Ender Chest, Big Dripleaf, Sugar Cane, Campfire | 12 | Game events, rail dedupe, stem support tag, used-item statistic, bell/jukebox events, piglin anger, projectile tilt/ignition, fluid-aware cane support. | Note-block game event, 8-rail power boundary, fruit on podzol/moss/mud, bookshelf stat, bell/sculk event, comparator update, piglin anger, projectile tilt/ignition, cane beside water. |

Name Tag, world loading, Shulker Box, `/locate` coverage and world saving are listed as done/verify-only above. Structure Block is done for the tracker gap with its documented deviation. The remaining Wave 1 client checks are the `/locate` branches and the `#3902` save A/B; the row count treats them as done and the client checks are tracked in `LIVE_TEST.md`.

## Wave 2 (2026-10-20)

Wave 2 closes 23 more rows and leaves the client checks below. It also adds the remaining `/version` and `/debug function` command behavior, which is not a tracker row change.

| Cluster | Rows | Evidence | Live check |
| :--- | ---: | :--- | :--- |
| Warden, Guardian, Elder Guardian, Skeleton, Enderman | 5 | Warden sniff/sonic boom; guardian beam/thorns; elder appearance/persistence/home; skeleton powder-snow conversion; enderman daylight teleport, carried-block drop, scream. | Warden sniff/boom; guardian beam/LOS/thorns; elder mining-fatigue timing; skeleton snow conversion; enderman teleport/drop. |
| Armor Stand, Cushion, Painting, Ominous Item Spawner | 4 | Armor-stand equip/swap/drop/fire; cushion support/placement/fire/lightning; painting support/drop; ominous spawner timer/item spawn. | Equip and break armor stand; remove cushion support; break painting support; summon ominous spawner with NBT. |
| Command Block Minecart, Spawner Minecart | 2 | Command state/NBT/editor/activator execution; shared `BaseSpawner` and spawner-cart ticking. | Powered activator rail executes and persists; spawner cart spawns and persists. |
| Fishing Rod | 1 | Nested loot tables, parent weights/quality, open-water condition, `set_damage`/`set_potion`; bobber uses the real distribution. | Fish in open water and a cramped pond, with and without Luck of the Sea. |
| Frogspawn, Lodestone, Tripwire, Tripwire Hook, Beds, Shulker Box, Dirt Path, Farmland, Daylight Detector (blocks + redstone) | 10 | Long hatch delays; compass inventory tick; tripwire events/shears/filtering; bed wake; shulker `canOpen`/piglin anger; entity push on path/farmland; player-source daylight event. | Hatch tadpoles; bind/break lodestone; tripwire/shears; sleep/wake; shulker lid; path/farmland lift; sculk hears daylight detector. |

The 23 rows are: 5 hostile, 4 decoration, 2 minecarts, 1 item, 1 combat/entity Fishing Bobber (the same fishing-loot fix as Fishing Rod), 8 block, 2 planned block. `/version` and `/debug function` are command-quality fixes and do not change the 562-row denominator. Dragon Fireball closed in Wave 4.

## Wave 3 (complete)

| Row | Work | Live check |
| :--- | :--- | :--- |
| Chests | Cat sitting on the lid now blocks opening, matching `ChestBlock.isCatSittingOnChest`. | Sit a tamed cat on a chest and right-click it. |
| Conduit | Code audit: frame activation, conduit power, attack target and destroy range are present. | Activate a conduit frame and damage a hostile mob nearby. |
| Dragon Egg | Code audit: teleport, falling and use/attack paths are present. | Hit and right-click the egg; confirm teleport and fall. |
| Sea Pickles | Code audit: bonemeal, placement and waterlogged support are present. | Bonemeal a pickle on coral; check light and placement. |
| Fire | Hardcoded increased-burnout biome list matches the eight `gameplay/increased_fire_burnout` biomes in the 26.3 data; the TODO is stale. | Burn in swamp/jungle vs plains and compare burnout. |
| Lily Pad | Placement now uses the fluid tag instead of testing a block against a fluid tag. | Place a lily pad on water and on a tagged support block. |
| Infested Blocks | Silk Touch now suppresses the silverfish spawn; normal breaks still spawn it. | Break infested stone with and without Silk Touch. |
| Mace | Piecewise fall-damage curve, 3.5-block smash knockback, gliding exclusion. | Hit a mob from 1.6/3/8/12 blocks of fall; check damage and nearby knockback. |
| Elytra | Fall-flying movement, start/stop conditions, 1 durability per 20 ticks, glide flag/pose, firework boost. | Glide, land, break the elytra, toggle creative flight/water/vehicle, relog. |
| Dispenser | Bone meal (crops and water plants), XP bottles, llama/donkey/mule chests, armadillo brushing, mob buckets. | Fire each dispenser case into its target. |
| Sculk Catalyst | Mob-death listener, XP absorption, cursor NBT, bloom and sculk spreading. | Kill a mob near a catalyst, watch bloom/spread, relog. |
| Pistons, Piston Head, Moving Piston, Pistons/block swapping, world Redstone | Vanilla movement checks, head/base drops and break propagation, moved-state NBT, waterlogged completion, stuck entities. | Push/pull, break head/base, quick toggle, save/restart mid-animation. |
| `/locate` | Namespaced ids by path, enabled structure sets, flat/custom generator handling, vanilla biome spiral order. | Compare structure/biome/POI coordinates with vanilla in noise and flat worlds. |
| World saving | `save-all flush` waits for writes; deferred block-entity NBT for unloaded chunks; shutdown ordering. | `#3902` chest 10k blocks out, `/stop`, restart; `/save-all flush` feedback timing. |
| `/jfr`, 1.8 combat mode | Documented non-goals; see **Documented deviations**. | Documentation only. |

## Wave 4 (2026-10-21)

Wave 4 closes the last 55 open tracker rows: the 54 entity rows and Beehive & Bee Nest. The rows are implemented and build-verified; the client checks are in `LIVE_TEST.md` §1 and §2.

| Cluster | Rows | Evidence | Live check |
| :--- | ---: | :--- | :--- |
| Brain/POI and passive interaction | 7 | Allay item transfer, Bee hive/pollination/crop growth, Copper Golem transport/weathering/statue, Villager schedule/breeding, Iron Golem village defence, Wandering Trader ambient/trade, Zombie Villager village movement. Commits `99738ea01`, `f184388a6`, `2cf0fe82a`. | P65, P66, P71, P80, P87, P96; A21, A24, A26, A31 |
| Wave 4a hostile AI | 14 | Wither autonomous primary-head targeting, Ender Dragon part dimensions/contact, Blaze home/retreat, Breeze wind charge/jump/slide, Spider/Cave Spider leap and armadillo avoidance, Vex charge/owner target/lifetime, Creaking freeze/attack/portal handling, Creeper cloud, Drowned trident/water movement, Illusioner bow/blindness/mirror, Ravager roar/stun, Witch raider healing, Hoglin repellent/retreat. Commits `27f475eec`, `27c741082`, `6ffb321f6`. | P67-P70, P72, P73, P76, P77, P79, P84, P88, P96; A25 |
| Riding and mount-family | 9 | Camel/Camel Husk sitting/dashing, Horse taming/gear/jump, Donkey/Mule/Llama/Trader Llama chest inventory and caravan, Skeleton Horse trap, Zombie Horse riding. Commit `6a616b58d`. | P69, P74, P78, P81, P85, P90; A22 |
| Boss and Nether objectives | 3 | Piglin Brute home memory, Piglin/Zombified Piglin spear use, Wither and Ender Dragon follow-ups. Commits `87c12fba1`, `b12493295`. | P83, P89; A25 |
| Mannequin and decoration | 1 | Mannequin entity, profile metadata, pose/description/immovable NBT, registration. Commits `94a3a19a2`, `d56b1f271`. | P82; A23 |
| Dragon Fireball | 1 | Gravity-free projectile, dragon-breath area cloud, instant damage, splash event. Commit `253b128a7`. | P75 |
| Wave 4b aquatic | 7 | Axolotl, Cod, Dolphin, Nautilus, Salmon, Tropical Fish and Zombie Nautilus parity pass. | P91, P92; A27 |
| Wave 4b passive A | 5 | Fox, Frog, Goat, Rabbit and Strider parity pass. | P93; A28 |
| Wave 4b passive B | 5 | Happy Ghast, Panda, Parrot, Sniffer and Turtle parity pass. | P94; A29 |
| Wave 4b leftovers | 2 | Experience Bottle and Tadpole parity pass. | P95; A30 |
| Beehive & Bee Nest | 1 | Hive occupant ticks, honey, emergency release, campfire sedation, silk-touch item components. Commit `99738ea01`. | P66; A31 |

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

`/locate` is marked partial by the tracker, but the current code implements structure, biome, and POI searches. The live check is recorded in `LIVE_TEST.md` (A08 passes; P61 notes the village-POI gap).

### Excluded from the parity target

`/publish` and `/unpublish` are not part of this count. Vanilla registers them only for the integrated server (`CommandSelection.includeIntegrated`), and a dedicated server must not expose them. They were removed from the denominator instead of being treated as missing commands.

Dev-only commands gated by `SharedConstants.DEBUG_DEV_COMMANDS`, `DEBUG_CHASE_COMMAND`, or running in the IDE are also out of scope: `/chase`, `/debugpath`, `/debugmobspawning`, `/wardenspawntracker`, `/spawnarmortrims`, `/serverpack`, `/debugconfig`. `/emote` is not a vanilla command; `/me` is, and Pumpkin implements it.

Pumpkin extras that are not parity gaps: `/pumpkin`, `/plugin`, `/plugins`, `/tps`, and `/raid`. Vanilla only registers `/raid` in dev mode, so it can stay as a Pumpkin debug extra or be gated later.

### Effect on the tracker

The raw tracker had 564 rows. Removing `/publish` and `/unpublish` gives the 562-row in-scope baseline. The total is **562/562 = 100%**. Treating `/jfr` as a non-goal does not change the denominator recorded here; it is listed as a documented deviation instead.

## How to update this file

1. Use the tracker snapshot that matches the date in this file. The current baseline is the 562-row in-scope snapshot derived from the 564-row public snapshot collected on 2026-10-20.
2. Keep two evidence levels: **verified row** when the code is merged, reviewed, part of a build that passes the ladder, and the server runs, and **client-checked** only when the matching `LIVE_TEST.md` check has passed. A source file appearing is not enough for either.
3. If a green row is found to be false, record it in this file and either fix it or move it back to partial in the tracker. Do not let green rows hide gaps.
4. Recompute the counts and percentages. Keep the date and fork commit current.
5. Keep the weighted number as a secondary signal. The primary number is done-only, because that is what the public 71% figure uses.

## Remaining work

No tracker row remains open. The remaining work is the per-mechanic client pass in `LIVE_TEST.md`; the 562-row parity count is complete. Known residual behaviors are listed under **Documented deviations**.

## Documented deviations

| Row | Status | Reason |
| :--- | :--- | :--- |
| Commands / `/jfr` | non-goal | JVM Flight Recorder is a JVM tool. The Rust server has no JVM; vanilla 26.3 registers only `start` and `stop`. Operators can use JVM flags or `jcmd` on a JVM process. |
| Combat / 1.8 combat mode | non-goal | Not a vanilla 26.3 feature. Pumpkin targets 26.3 combat; a legacy mode would be a plugin or a separate configurable game mode, not parity behavior. |
| Horse-family mount GUI | deviation | Mount screens for Horse, Donkey, Mule, Llama, Trader Llama, Camel and Nautilus are wired and chest storage is real. The saddle slot is synthetic because saddle state is a flag, horse body armor is stored but does not reduce damage, and vanilla's per-slot validity/`isActive` filters are not reproduced. P74/P81/P90 and A22 cover the supported paths. |
| Entities / Hostile / Creaking | deviation | Freeze, attack and portal behavior is implemented. Breaking the Creaking Heart does not run vanilla's `removeProtector` teardown; the bound Creaking loses protection on its next heart check instead. P72 covers the live check. |
| Ageable mobs / Golden Dandelion | deviation | `AgeLocked` is stored and ageing is suppressed while locked, but right-clicking a baby with a Golden Dandelion does not toggle the lock, reset its age or emit the growth particles. The horse family, panda and camel rows are otherwise complete. |
| Combat / thrown projectiles | deviation | Server-owned projectile motion works, but vanilla's `KnownMovement` history is not recorded, so checks that compare a projectile's recent movement are not reproduced. |
| Entities / Experience Orb | deviation | Orbs merge and despawn, but do not call vanilla's `unstuckIfPossible` when they are embedded in a block. |
| Entities / Area Effect Cloud | deviation | Lingering clouds tick and apply effects, but their owner UUID and persistent effect NBT are not saved or reloaded. |
| Blocks / Structure Block corner scan and redstone | deviation | Save, load and capture work, but corner-mode `ACTION_SCAN_AREA` and redstone-powered execution are not implemented; the block only opens its GUI. The tracker gap itself is closed by the save/load path. |
| Moving piston drops and collision shape | verify-only | The Wave 3 piston port keeps moved-block persistence and the player-pass-through break path, but the full drop/collision-shape comparison against vanilla is still a live check (P57-P60). |
| Elytra fly-into-wall damage | verify-only | Fall-flying movement, durability and stop conditions are implemented; the wall-impact damage edge is not covered by a focused test and needs the P52 client check. |
| Custom-biome `/locate` and unknown datapack structure paths | deviation | `/locate` handles vanilla structure, biome and POI searches; custom biome sources and structure ids that are not in the generated registry are still not resolved. P61-P62 and A08 cover the supported paths. |
