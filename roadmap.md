# PumpkinMC-AI 1.0 Roadmap

**Parity target:** Java Edition **26.3** (`CURRENT_MC_VERSION` / `LOWEST_SUPPORTED_MC_VERSION` in `crates/pumpkin-data/src/generated/packet.rs`) and Bedrock Edition **1.26.51** (protocol 2193 in `crates/pumpkin-world/src/lib.rs`).

**Snapshot date:** 2026-10-20.

**Repository state:** fork `fork-ai-main` code tip `bf5edaf97`; the docs update is the current tip, ahead of `origin/master` at `fb7c8e223`. Wave 1 code is merged; the docs and live checks are the remaining work. The working tree keeps only the unrelated `typos.toml` edit.

**Upstream state used for this audit:** `Pumpkin-MC/Pumpkin` had **352 open issues** and **313 open pull requests** when the inventory was collected. The public tracker had **562 in-scope entries: 402 done, 155 partial, 5 planned** after excluding the integrated-server-only `/publish` and `/unpublish` commands.

This file is a work plan, not a claim that upstream's tracker is correct. Every green tracker entry was treated as "a source file exists", not as "vanilla parity is proven". Unchecked items were checked against the fork's code where the code was reachable; the rest carry an evidence label or are explicitly marked "verify". Where the fork already contains an implementation that upstream's issue predates, the task says **verify and close**, not "implement from scratch".

---

## 0. How to use this roadmap

- Each workstream has an owner role, a scope, task IDs, dependencies, and a concrete exit condition.
- Each task has a priority:
  - **P0**: blocks 1.0. Crashes, data loss, security, stable ABI, release engineering, or a core mechanic that is absent.
  - **P1**: required for the 100% parity claim but not on the critical release path.
  - **P2**: edge case, polish, or a documented deviation candidate.
- A task is only done when its **Done** line is satisfied. "Code exists" is not done.
- Cross-cutting issues appear in more than one task. The appendix assigns each open issue one primary workstream; the task text lists the rest.
- Open PRs are **in flight, not merged**. Do not implement the same change without checking the PR first. In-flight PRs are cited as `PR #...`.
- Anything that changes player-visible behavior needs a real-client capture before merge, per `AGENTS.md` and `.github/PULL_REQUEST_TEMPLATE.md`.
- Do not edit `crates/pumpkin-data/src/generated/` by hand. Extend `tools/pumpkin-codegen` and regenerate with `cargo run --locked -p pumpkin-codegen`.
- This document is expected to change. Update the task state in the same PR that closes it.

### 0.1 Evidence labels

Every claim below is one of:

- **[code]** verified by reading the fork's source at `bf5edaf97`.
- **[issue]** reported by an open upstream issue; not independently reproduced.
- **[tracker]** reported by the parity tracker; not independently verified.
- **[pr]** an open pull request exists; not merged.
- **[vanilla]** checked against the decompiled 26.3 server or its shipped data.

If a task has no label, it is a plan derived from the cited code and issues.

### 0.2 Assumptions

1. Java 26.3 is the single supported Java protocol version at 1.0. Older Java clients are out of scope unless a plugin provides them. This matches the current `LOWEST_SUPPORTED_MC_VERSION = 26.3`.
2. Bedrock 1.26.51 is supported through the shared engine. Bedrock-specific parity is a separate workstream, not a second full parity target.
3. "100% parity" means observable vanilla behavior: blocks, items, entities, AI, world generation, commands, data packs, protocol, and server operations. It does not require byte-identical internal code, nor bug-for-bug reproduction of vanilla crashes.
4. World-generation seed parity is a goal for the supported generator. Exact parity for every structure and feature is gated by the golden-seed harness in `Q04`; deviations must be listed in `T14`.
5. The fork's 26.3 content merge is real, not a stub: `cushion`, `straw_bed`, `shelf_mushroom`, `poplar` trees, dappled forest, abandoned camps, wool/concrete stairs and slabs, and `red_shrub` all exist in generated data and have runtime registrations. The remaining 26.3 work is verification and edge cases (`H09`).
6. "Stable ABI" means plugin binaries built against 1.0.x continue to load and run on every 1.0.y patch. It does not mean the ABI is frozen across 1.x minors.

### 0.3 Glossary

- **ABI**: binary interface a plugin is compiled against.
- **BE**: block entity, not Bedrock Edition. The roadmap spells out "Bedrock" when it means the edition.
- **Brain**: vanilla's memory/sensor/activity AI stack, not the goal selector.
- **MSPT**: milliseconds per tick; 50 MSPT is 20 TPS.
- **POI**: point of interest (bed, workstation, bell) used by villagers and raids.
- **RCON**: remote console protocol.
- **WIT**: WebAssembly Interface Types, the Wasm plugin ABI.
- **MSMP**: Pumpkin's management server protocol.
- **Vanilla**: the official Mojang Java 26.3 server, unless a sentence says Bedrock.
- **Green tracker entry**: the public tracker says "done"; it only means a source file exists until `T01` proves otherwise.

### 0.4 References

- Public parity tracker: <https://pumpkinmc.org/tracker>
- Upstream issues: <https://github.com/Pumpkin-MC/Pumpkin/issues>
- Upstream pull requests: <https://github.com/Pumpkin-MC/Pumpkin/pulls>
- Fork: <https://github.com/gringoestrangeiro/pumpkinmc-ai>
- 1.0 release issue: <https://github.com/Pumpkin-MC/Pumpkin/issues/449>
- Mob AI tracker: <https://github.com/Pumpkin-MC/Pumpkin/issues/3468>
- GameTest tracker: <https://github.com/Pumpkin-MC/Pumpkin/issues/3867>

---

## 1. Definition of done for 1.0

1.0 ships only when all eleven gates are green.

| Gate | Requirement | Evidence |
| :--- | :--- | :--- |
| **G1 Stability** | No known crash, panic, deadlock, or data-loss bug reachable from a client, a data pack, or a normal console command. Zero open `panic`-labeled issues that are reproducible on `fork-ai-main`. | `R07`, `W15`, `Q10` |
| **G2 Plugin ABI** | Native and Wasm plugin ABIs are versioned, documented, and covered by compatibility tests. `PLUGIN_API_VERSION` and WIT v0.2 are final for 1.0. | `R02`, `R03`, `M12` |
| **G3 Community test** | A private alpha with external server owners and plugin developers has run for at least two weeks, with a triaged issue list and no unresolved P0 report. | `R09` |
| **G4 Platforms** | Every platform promised in `#449` has a published, smoke-tested artifact: Windows x86_64/arm64, Linux x86_64/aarch64 (musl and gnu), macOS x86_64/arm64, FreeBSD x86_64/aarch64, Android aarch64/armv7/x86_64, Docker amd64/arm64/armv7. | `R04`, `O07` |
| **G5 Save integrity** | `stop`, `save-all`, crash, and kill -9 during a save never lose committed chunks, block entities, entities, player data, or level.dat. Round-trip tests cover vanilla 26.3 worlds and all three chunk formats. | `I01`-`I03`, `Q06` |
| **G6 Security** | No unauthenticated path traversal, arbitrary file read/write, decompression bomb, unbounded allocation, auth bypass, proxy spoof, or plugin signature bypass. | `O05` |
| **G7 Performance** | 20 TPS at the reference load (see `PF11`), no unbounded memory growth per join/leave, and no regression against the 1.0 benchmark baseline. | `W18`, `Q12` |
| **G8 Documentation** | Install, upgrade, configure, administer, and write-a-plugin docs exist. Every crate in `#1482` has module-level docs. | `W19`, `S01`-`S04` |
| **G9 QA** | GameTest runs headless in CI; the bot suite covers join, movement, combat, inventory, containers, redstone, and worldgen smoke tests; protocol fuzzing runs on every PR. | `W17` |
| **G10 Packaging** | Signed artifacts, checksums, SBOM, Docker tags, release notes, and a reproducible release workflow. | `R05`, `R12` |
| **G11 Parity debt** | Every tracker `partial`/`planned` entry is closed, or listed in `T14` as an accepted deviation with a vanilla reference and a reason. Zero unexplained gaps. | `T01`, `T14` |

### 1.1 Explicit non-goals for 1.0

- Multi-version Java protocol support. The server speaks 26.3 only; older clients are a plugin concern.
- A Bukkit/Paper compatibility layer. The plugin API is native/Wasm, not Bukkit.
- Bug-for-bug parity with vanilla crashes, duplications, or undefined behavior.
- A built-in web dashboard or hosted service. The management protocol and metrics endpoints are enough; a dashboard can be a plugin or an external tool.
- Bedrock experimental features that Mojang ships behind an experiment toggle, unless the vanilla Bedrock server enables them by default.

---

## 2. Parallelization model

### 2.1 Roles

| Role | Responsibility |
| :--- | :--- |
| **Integrator** | Owns `master`, the merge queue, `Cargo.toml`/`Cargo.lock`, generated data regeneration, and cross-workstream conflicts. One person, not an agent. |
| **Workstream owner** | Owns one `W` section, triages its issues, reviews its PRs, keeps its task states current. |
| **Task agent** | Claims one task, works only in its file scope, runs the verification ladder, hands off with a PR. |
| **QA owner** | Owns `W17`, the bot suite, GameTest, fuzzing, and the release candidate checklist. |
| **Release owner** | Owns `W0`, the platform matrix, signing, packaging, and the final tag. |

### 2.2 Branch and PR rules

- One issue per PR. Fork work targets `fork-ai-main`; upstream work targets `master` only after the human review and play-test in `AGENTS.md`. Branch name: `<type>/<track>-<slug>`, for example `fix/w4-piston-retraction`.
- Conventional commit with scope: `fix(block): ...`, `feat(entity): ...`, `perf(world): ...`.
- No mixed workstreams in one PR. If a fix needs a foundation change, land the foundation first.
- Do not reformat untouched code, do not commit unrelated lockfile/generated churn, and do not fold review fixes into a second commit after handoff.
- Every PR description follows `.github/PULL_REQUEST_TEMPLATE.md`, including the AI-assistance section. No capture, no merge for player-visible changes.
- A task agent may not open an upstream PR. Fork PRs are fine; upstream submission requires the human review and play-test described in `AGENTS.md`.

### 2.3 Hot-file ownership

These files are touched by several workstreams. Only the listed owner merges changes to them; other workstreams submit a patch or ask the owner to land the shared part first.

| Hot file / directory | Owner | Consumers |
| :--- | :--- | :--- |
| `crates/pumpkin-core/src/world/mod.rs` | `W9` | `W4`, `W6`, `W8`, `W10`, `W11`, `W13` |
| `crates/pumpkin-core/src/entity/player.rs` | `W7` | `W5`, `W6`, `W14` |
| `crates/pumpkin-core/src/entity/mob/mod.rs` | `W1` | `W2`, `W3` |
| `crates/pumpkin-core/src/entity/ai/**` | `W1` | `W2` |
| `crates/pumpkin-core/src/entity/living.rs` | `W6` | `W2`, `W5`, `W7` |
| `crates/pumpkin-core/src/block/registry.rs` | `W4` | `W5`, `W8` |
| `crates/pumpkin-core/src/item/items/mod.rs` | `W5` | `W3`, `W7` |
| `crates/pumpkin-core/src/command/commands/mod.rs` | `W14` | `W0`, `W13` |
| `crates/pumpkin-core/src/plugin/**` | `W13` | `W1`-`W12` event additions |
| `crates/pumpkin-core/src/net/**` | `W11` | `W12`, `W13` |
| `crates/pumpkin-world/**` | `W8`/`W9` split by directory | `W10`, `W11` |
| `crates/pumpkin-protocol/**` | `W11` | `W12`, `W13` |
| `crates/pumpkin-config/**` | `W15` | `W0`, `W11`, `W12` |
| `crates/pumpkin-data/src/generated/**` | Integrator | everyone, via codegen only |
| `assets/**` | Integrator/data owner | `W4`, `W5`, `W8`, `W16` |
| `.github/workflows/**` | `W0`/`W17` | everyone |
| `Cargo.toml`, `Cargo.lock` | Integrator | everyone |

### 2.4 Shared interfaces to freeze first

Land these before parallel work fans out, or every workstream will invent a second pattern:

1. `Mob::create_navigation` / `create_move_control` hook (`A01`).
2. `TargetPredicate` + anger/team source (`A05`).
3. `Brain` memory/sensor/activity interfaces (`A08`-`A11`).
4. `LootFunction` execution interface (`E15`).
5. `RecipeType` dynamic interface (`E13`).
6. `GameEvent` dispatch/vibration interface (`D07`).
7. `World::emit_game_event` contract for plugins and sculk (`D07`, `M03`).
8. Chunk ticket API (`I05`).
9. Plugin ABI version negotiation (`R02`, `R03`).
10. `DataPack` registry loader interface (`N04`).

### 2.5 Workstream map

| ID | Workstream | Owner | Primary paths | Depends on | Exit condition |
| :--- | :--- | :--- | :--- | :--- | :--- |
| **W0** | Release engineering | Release owner | `.github/`, packaging, docs index | all | All gates green |
| **W1** | Mob AI foundation | AI owner | `entity/ai/**`, `entity/mob/mod.rs` | none | Nav, targets, brain, pathfinding stable |
| **W2** | Mob parity | AI owner + mob agents | `entity/mob/**`, `entity/passive/**` | W1 | Every mob matches its vanilla goal/brain set |
| **W3** | Entities, vehicles, decoration | Entity owner | `entity/vehicle/**`, `entity/decoration/**`, `entity/projectile/**` | W1 (physics) | Entity behavior and persistence parity |
| **W4** | Blocks and redstone | Block owner | `block/**` | W10 (ticks), W9 (block-entity save) | All block behaviors and redstone parity |
| **W5** | Items, inventory, recipes, loot | Item owner | `item/**`, `inventory/**`, `data/datapack/**`, `world/loot.rs` | W4, W16 | Item use, containers, recipes, loot parity |
| **W6** | Combat | Combat owner | `entity/living.rs`, `entity/projectile/**` | W3, W5 | Damage, projectiles, explosions, shields parity |
| **W7** | Player systems | Player owner | `entity/player.rs`, `entity/player/**` | W5, W9 | Movement, sleep, respawn, stats, UI parity |
| **W8** | World generation | Worldgen owner | `pumpkin-world/generation/**` | none | Seed parity and structure/feature parity |
| **W9** | Persistence and chunk lifecycle | Storage owner | `pumpkin-world/chunk/**`, `world/mod.rs` | none | No data loss; tickets and saves correct |
| **W10** | World simulation | World owner | `world/**` (ticks, fluid, light, explosion) | W9 | Ticks, fluids, light, portals, borders parity |
| **W11** | Java protocol | Protocol owner | `pumpkin-protocol/**`, `net/java/**` | none | No decode panic, correct packets/state machine |
| **W12** | Bedrock | Bedrock owner | `net/bedrock/**`, `pumpkin-protocol/bedrock/**` | W11 patterns | Login, movement, chunks, inventory stable |
| **W13** | Plugin API | API owner | `plugin/**`, `pumpkin-plugin-*` | W0 (ABI) | Stable ABI, event and world APIs complete |
| **W14** | Commands and data packs | Command owner | `command/**`, `data/datapack/**` | W5, W16 | Every vanilla command and pack type works |
| **W15** | Operations, config, security | Ops owner | `pumpkin-config/**`, `server/**`, `net/management/**` | none | Config, RCON, query, proxy, packaging, security |
| **W16** | Data parity | Data owner | `pumpkin-data/**` (via codegen), `advancement/**`, `enchantment/**` | Integrator | All registries and data-driven behavior present |
| **W17** | QA and testing | QA owner | `pumpkin-gametest/**`, tests, CI | all | Release candidate suite green |
| **W18** | Performance | Perf owner | cross-cutting | W1, W9, W11 | Baseline met, no regressions |
| **W19** | Docs and i18n | Docs owner | `docs/**`, translations, README | all | Docs gate green |
| **W20** | Parity audit | QA owner | tracker, issues | all | Green entries audited; deviations documented |

### 2.6 PR-first policy

There are **313 open pull requests** upstream. Many of them already fix an issue in this roadmap. For every task:

1. Search Appendix C and the upstream PR list for the issue number and a keyword from the task.
2. If a PR exists, the task is **review, rebase, test, and land**, not "write it again".
3. If the PR is stale or wrong, comment with the specific problem and either adopt it or open a replacement that cites it.
4. Do not open a competing PR for the same issue. Duplicate work is the fastest way to burn the 1.0 budget.
5. A PR that has been open for months without review is a triage item (`S11`), not an excuse to start over.

The same rule applies to the local branches in Appendix H.

---

## 3. Release gates (W0)

### R01 — Freeze the 1.0 scope and parity target `P0`

**Scope:** Write the 1.0 scope in `README.md` and this file: Java 26.3, Bedrock 1.26.51, supported platforms, plugin ABI versions, and explicit non-goals. Freeze it before feature work fans out.

**Done:** A single "1.0 scope" section exists and every workstream links to it. No task may add a new registry, platform, or protocol version without the release owner's approval.

**Evidence:** `#449` is stale: it marks stress testing, memory audit, tick consistency, cross-platform verification, protocol fuzzing, and unwrap removal as done, but the code still contains 211 `TODO`/`unimplemented` markers and multiple open panic issues. Do not use `#449`'s checkboxes as evidence.

### R02 — Stable native plugin ABI `P0`

**Scope:** Finalize the native plugin ABI for 1.0: public types in `pumpkin-plugin-api`, the `#[plugin_method]` macro boundary, `PLUGIN_API_VERSION`, symbol visibility, error types, and the loading/unloading contract. Decide which types are `#[non_exhaustive]` and which are frozen.

**Done:**
- `cargo semver-checks` or an equivalent ABI diff runs in CI for `pumpkin-plugin-api`.
- A fixture plugin built against 1.0.0 loads and runs on 1.0.x without recompilation.
- The version check rejects incompatible plugins with a clear error, not a panic.
- The ABI rules are documented in `pumpkin-plugin-api/README.md`.

**Issues/PRs:** `#1407` (closed tracker), `#3593`, `#3701`, `#2056`, `#3089`, `#2828` (draft native cdylib loading).

### R03 — Stable Wasm plugin ABI v0.2 `P0`

**Scope:** Finish WIT v0.2, generate bindings, make `plugin-v0_2` the default host, and keep v0.1 plugins loadable. The WIT is mirrored to a public repository and other-language SDKs depend on it; breaking changes must be additive or versioned.

**Done:**
- `cargo run --locked -p pumpkin-codegen -- wit` is clean.
- v0.1 and v0.2 fixture plugins both load, call host APIs, register commands/events, and unload.
- The v0.2 feature is on by default in the release build, or the release notes say why not.
- `crates/pumpkin-plugin-wit` has a compatibility policy and a changelog.

**Issues/PRs:** `#3096`, `#3484`, `#3908`, `#3927`, `#3962`, `#4096`, `#4098`, `#2828`.

### R04 — Platform build matrix `P0`

**Scope:** Build and smoke-test every promised platform. Current release workflow builds Linux x86_64/aarch64, Windows x86_64/arm64, macOS (arm64 runner), Linux musl x86_64/aarch64, and Android aarch64. Missing: FreeBSD, macOS x86_64, Android armv7/x86_64, Docker armv7.

**Done:** `release.yml`, `docker.yml`, and `rust.yml` cover the matrix in G4. Each artifact boots a void world, accepts a Java 26.3 client, accepts a Bedrock 1.26.51 client where applicable, saves, and stops. A matrix table in the release notes names the exact artifact and smoke-test result.

**Issues/PRs:** `#449`, `#3628` (armv7), `#4052` (Docker tags), `#1809`, `#3410`, `#3664`, `#3485`.

### R05 — Release artifacts and supply chain `P0`

**Scope:** Signed binaries (SignPath where available), SHA-256 checksums, SBOM, Docker image tags (`latest`, `1.0`, `1.0.x`, commit), reproducible build flags, and a download page that matches the artifacts.

**Done:** A release candidate can be produced from a clean tag by one workflow, with no manual file copying. Checksums and signatures verify. The Docker image runs as a non-root user and persists `/data`.

**Issues/PRs:** `#4052`, `#3410`, `#1809`.

### R06 — Upgrade and migration policy `P0`

**Scope:** Define what happens to a world, config, plugin, and data pack when upgrading from 0.x/alpha to 1.0. Add `--forceUpgrade`/converter behavior where needed, and a compatibility report for a world that cannot be upgraded.

**Done:** A pre-1.0 world and a vanilla 26.3 world both load, convert where necessary, and save without losing block entities, entities, or structures. A failed upgrade leaves the original untouched.

**Issues/PRs:** `#118`, `#1554`, `#2626`, `#4031`.

### R07 — Crash and panic budget `P0`

**Scope:** Remove reachable panics from network, disk, data pack, command, and plugin boundaries. Keep `unwrap`/`expect` out of those paths. Make the panic hook write a crash report and exit with a defined code. No `process::abort` from a Rayon worker panic.

**Done:** `cargo clippy` denies `unwrap`/`expect`/`panic`/`todo!` in boundary crates. Fuzzing finds no panic in 24 hours of corpus replay. Every fixed panic has a regression test or a corpus entry.

**Issues/PRs:** `#3911`, `#3992`, `#3993`, `#3995`, `#3999`, `#4000`, `#4001`, `#4003`, `#4055`, `#4056`, `#3716`, `#3777`, `#3782`, `#3319`, `#2552`, `#2057`, `#3089`.

### R08 — Security release gate `P0`

**Scope:** Run the checklist in `O05` before every release candidate. Treat any unauthenticated remote crash, file write, auth bypass, or plugin signature bypass as a release blocker.

**Done:** `SECURITY.md` has a threat model and a disclosure process. The `O05` checklist is signed off by the release owner.

### R09 — Community test run `P0`

**Scope:** Recruit server owners and plugin developers, ship a release candidate, collect crash reports and parity reports, triage them into this roadmap, and run for at least two weeks.

**Done:** At least 10 external servers, at least 5 plugin developers, a public test report, and zero unresolved P0 reports. The report lists every known deviation with an issue link.

**Issues/PRs:** `#449` (the only unchecked release item besides the ABI).

### R10 — Documentation gate `P0`

**Scope:** Install, upgrade, configure, administer, back up, and write-a-plugin docs. Link them from the README and the download page.

**Done:** A new user can go from download to a working server with a plugin in under 15 minutes using only the docs. Every config field has a description and a default.

**Issues/PRs:** `#1482`, `#3049`, `#1726`.

### R11 — Licensing and attribution `P1`

**Scope:** Confirm GPL-3.0 for the server, MIT/Apache-2.0 for the plugin crates, the NOTICE file, the Mojang EULA disclaimer, and the status of the AGPL proposal (`#2478`). Do not relicense existing contributions without author permission.

**Done:** `LICENSE`, `NOTICE`, `README.md`, and the plugin crate manifests agree. The AGPL proposal is either rejected with a note or accepted with a contributor audit.

### R12 — Release automation `P0`

**Scope:** One workflow that bumps the version, updates the changelog, tags, builds, signs, publishes GitHub release artifacts, pushes Docker tags, syncs WIT, and deploys docs.

**Done:** A dry run on a release candidate produces the same artifacts as a manual run. The workflow fails closed if a signature, checksum, or smoke test fails.

### R13 — Parity debt exit criteria `P0`

**Scope:** Before 1.0, every entry in `tracker_gaps.tsv` is either closed or listed in `T14` as an accepted deviation. No "partial" entry may remain unclassified.

**Done:** The appendix B table has no `open` rows. `T14` is the only place where a deviation can live, and each row names the vanilla behavior, the Pumpkin behavior, and why it is acceptable.

---

## 4. Mob AI foundation (W1)

The authoritative mob-AI tracker is `#3468`. Its diagnosis is correct: most mob bugs are not missing per-mob goals, they are missing navigation types, move-control behavior, target predicates, and the Brain stack. Land W1 before mass-porting W2.

### A01 — Navigation type selection `P0`

**Scope:** Add a `create_navigation` hook on `Mob` and select the right navigator per family: ground (default), water-bound, amphibious, flying, wall-climber. Wire the corresponding move/look controls.

**Evidence:** `[code]` `Navigator::flying()`, `water_bound()`, `amphibious()`, and `wall_climber()` have zero call sites. Every mob uses `Navigator::default()` (ground). Fish, squid, dolphins, drowned, turtles, bats, bees, parrots, allays, vexes, phantoms, blazes, ghasts, the wither, spiders, and cave spiders are all affected.

**Done:** Each listed family has a navigation type and a test that it paths through its native medium. `#4005`, `#4090`, `#4063`, `#4126`, `#2162` are reproduced before and fixed after.

**Issues/PRs:** `#3468` §3, `#3649`, `#3718`, `#4030`, `#4039`.

### A02 — `can_float` and water pathfinding `P0`

**Scope:** Verify the `can_float` wiring. `SwimGoal::can_start` already calls `navigator.set_can_float(true)` (`ai/goal/swim.rs:35`), and `Navigator` applies `mob_data.can_swim` plus the water malus. The remaining work is to confirm every water-capable mob registers `SwimGoal`, that the malus is 0 when floating and 8 when not, and that `#2194` is actually fixed for each family.

**Evidence:** `[code]` the old TODO is gone; `swim.rs` sets the flag. `#3468` was written before that change, so its `can_float` bullet is stale. Do not re-add a second flag setter.

**Done:** A drowned, zombie, and wolf can path through shallow water; a fish cannot path on land; a mob without `SwimGoal` still has the 8.0 water malus. Regression test for `#2194`.

**Issues/PRs:** `#3468` §3.

### A03 — Move control, jump control, body rotation `P0`

**Scope:** Port vanilla `MoveControl` faithfully. Start from the Java `MoveControl.tick` and `LivingEntity.travel` rather than from `#3468`'s summary, because the input reset may belong in the entity tick rather than the `Operation::Wait` branch. Then verify:
- stale movement input is cleared when the controller returns to `Wait`;
- strafe `isWalkable` check;
- collision-shape jump condition, not just `yd > step_height`;
- liquid exit in `Jumping`;
- wire `JumpControl` and `BodyRotationControl`, which are currently dead code.

**Done:** Mobs no longer drift, walk in air, or fail to step up a block. `#2194`, `#1336`, and `#2232` have regression coverage.

**Issues/PRs:** `#3468` §3, `#4092` (draft foundation rework).

### A04 — Water-avoiding stroll and `MoveToBlockGoal` `P1`

**Scope:** Add `WaterAvoidingRandomStrollGoal` using `LandRandomPos` and switch every mob that vanilla uses it for. Add the `MoveToBlockGoal` base class needed by turtle egg laying, cat sitting, fox berries, and villager work.

**Evidence:** `[code]` `WanderAroundGoal` always uses the default random position (10, 7); `LandRandomPos` exists after `#3440` but is not used by a water-avoiding variant.

**Done:** Mobs stopped choosing water targets for ordinary strolling; turtle/cat/fox/villager goals compile against one base class.

### A05 — Target predicates, anger, and teams `P0`

**Scope:** Add `can_target` / `is_angry_at` to `ActiveTargetGoal`; add `ResetUniversalAngerTargetGoal` and anger memory; exclude creative/spectator; resolve the authoritative team source for ally checks.

**Evidence:** `[code]` `ActiveTargetGoal` only has a visibility chance gate. `#3430` (iron golems attack unprovoked), `#2998` (wolves aggro unprovoked), `#2789` (wolves ignore prey), `#3376` (mobs do not aggro), `#1760` (zombies attack creative players), and `#3455` (team source) are all symptoms.

**Done:** Each named issue has a before/after bot test. A single `TargetPredicate` implementation is used by goals and Brain sensors.

**Issues/PRs:** `#3455`, `#3468` §3, `#4107` (draft temperament/target rework).

### A06 — Knockback and knockback resistance `P1`

**Scope:** Audit every attack path against `strength * (1.0 - knockback_resistance)`. Expose the attribute on every living entity with vanilla base values. Fix explosion knockback separately (`F03`).

**Done:** A zombie, player, and iron golem take the same horizontal knockback as vanilla for the same hit. `#3515` and `#3464` have regression tests.

**Issues/PRs:** `#1013` (referenced by `#3468`), `#4112`.

### A07 — Pathfinding correctness `P0`

**Scope:** Fix zombie corner/head-height pathing, mobs standing on or walking through air, step-up regressions, and node penalties for water, doors, fences, and powder snow. Reuse paths and caches where vanilla does, without changing behavior.

**Done:** A pathfinding corpus (zombie around a corner, spider on a wall, fish through water, villager through a door) passes. `#1336`, `#2194`, `#2232` are closed.

**Issues/PRs:** `#3955`, `#4053`, `#4086`.

### A08 — Brain core `P0`

**Scope:** Complete the Brain stack: `Brain`, `Activity`, `ActivityData`, `MemoryMap`, `MemoryModuleType`, `MemorySlot`, `MemoryStatus`, `ExpirableValue`, `WalkTarget`, `NearestVisibleLivingEntities`. The Phase 0 framework exists in `crates/pumpkin-core/src/entity/ai/brain/` (~579 lines) but no mob uses activities.

**Done:** A test mob can switch activities on a memory change, serialize/deserialize its brain, and dump it through `/debug`. The interface is frozen for W2.

**Evidence:** `[code]` Phase 0 exists; `sensing.rs` is 36 lines; no activity consumers.

**Issues/PRs:** `#3468` §4, `#3490` (Phase 0).

### A09 — Brain sensors and targeting `P0`

**Scope:** Port the 27 sensors and `TargetingConditions`. Start with the shared set: `NearestLivingEntitySensor`, `NearestVisibleLivingEntitySensor`, `HurtBySensor`, `PlayerSensor`, `NearestItemSensor`, `TemptingSensor`, `NearestBedSensor`, `IsInWaterSensor`, `GolemSensor`, `AdultSensor`.

**Done:** Sensors drive the Phase 1 piglin pilot and at least one hostile mob. No per-mob hand-rolled "nearest entity" scan remains in migrated mobs.

**Issues/PRs:** `#3468` §4 Phase 2.

### A10 — Brain behaviors, gossip, and village `P0`

**Scope:** Port the 119 behaviors (including `warden/` and `declarative/`), the 3 gossip classes, and the 9 village classes. Behaviors are the actual content of brain mobs; sensors alone do nothing.

**Done:** The Phase 1-3 mobs in `B29`-`B33` run on behaviors, not goals. Gossip and village data drive iron-golem and raid behavior.

**Issues/PRs:** `#3468` §4.

### A11 — Brain NBT and debug tooling `P1`

**Scope:** Make the Brain NBT round-trip vanilla-identical: `Brain.memories.<id>.{value,ttl}`. Generalize the hand-written piglin memory NBT from `#3440` so existing saves stay valid. Add a `/debug` brain dump and a test that a saved brain reloads.

**Done:** A piglin saved by vanilla 26.3 loads with its memories intact; a Pumpkin-saved piglin loads in vanilla. The debug dump lists activity, memories, and sensors.

**Issues/PRs:** `#3468` §4.

### A12 — Natural spawning caps and cadence `P1`

**Scope:** Align mob caps, spawn attempts, pack size, biome spawn lists, and despawn distance with vanilla. Add configurable mob-rate options without changing defaults.

**Done:** A 10-minute spawn test in a fixed seed produces the same mob category counts as vanilla within the documented tolerance. `#3356`, `#3618`, `#3861`, `#3603` are closed.

**Issues/PRs:** `#3356`, `#3603`, `#3861`. Local branch `feat/vanilla-parity-batch` already has `d7c08db76 fix(spawning): align natural spawn caps and cadence with vanilla` and `9ec254314 fix(entity): apply night sky darkening to monster spawn checks`; verify and land or supersede them before starting over.

### A13 — Despawn, persistence, and visibility `P0`

**Scope:** Fix mob despawn when no player is nearby, entity persistence across simulation-distance changes, per-player visibility, and non-persistent entities. This is also the root of the GameTest headless failures.

**Evidence:** `[code]` `check_despawn` already returns early when no player is found, so that part of `#3867` is stale. The remaining blocker is `clean_memory`, which collects entity chunks without a watching player and hands them to `World::remove_entities_in_chunks`. `#3385`, `#3734`, `#3867`.

**Done:** A mob with no nearby player persists according to vanilla rules; a forced chunk keeps its entities across unload/reload. GameTest can run with no player.

**Issues/PRs:** `#3385`, `#3734`, `#3867`.

### A14 — Entity collision and physics `P0`

**Scope:** Fix mob-vs-block and mob-vs-mob collision after being hit, unusual collision boxes, and pushing. `#4121` reports unusual collision; `#2232` reports hitbox ignoring after a hit.

**Done:** A bot can push a mob, hit it, and observe the same separation as vanilla. Collision tests cover standing on slabs/stairs, inside bamboo, and against a wall.

**Issues/PRs:** `#4121`, `#2232`, `#3845`.

### A15 — Mob audio, animation, and hit reaction `P1`

**Scope:** Play hurt/death/ambient/attack sounds, crit and sweep particles, and the correct animation/entity-event packet. `#2566` reports no audio, no crits, and no reaction to being hit. `#2991` reports missing animations. `#4123` reports the warden hit animation packet.

**Done:** A real client sees and hears the vanilla reaction for a sword hit, a crit, a sweep, and a mob death.

**Issues/PRs:** `#2566`, `#2991`, `#4123`, `#3830`.

### A16 — AI performance `P1`

**Scope:** Stop preallocating every mob's pathfinding heap, reuse paths and block caches, limit searches, and avoid strong `Arc` cycles in goals. Keep behavior identical.

**Done:** The reference mob-heavy benchmark improves or stays flat; no goal leaks a mob. `#4053`, `#3955`, `#4086`.

### A17 — Raid and POI integration `P1`

**Scope:** Connect village POIs, reputation/gossip, raid triggering, wave spawning, bell revealing, and hero-of-the-village. `world/raid.rs` and `world/villager_poi.rs` exist; verify the full path.

**Done:** A bot triggers a raid in a village, sees waves, bells reveal raiders, and the raid ends with the vanilla rewards. `#3631`, `#4124`.

**Issues/PRs:** `#3631`, `#4124`, `#3455`.

---

## 5. Mob parity (W2)

The per-mob checklist is `#3468` §1-2 and §5. Each task below is a shippable unit. A task is done only when the mob's goal/brain set matches vanilla 26.3, its loot/trade data is correct, and a bot or GameTest exercises the signature behavior.

### B01 — Zombie family `P0`

**Scope:** Drowned water/land goals (`DrownedGoToWater`, `DrownedTridentAttack`, `DrownedAttack`, `DrownedGoToBeach`, `DrownedSwimUp`), `SpearUseGoal`, `MoveThroughVillageGoal`, `ZombieAttackTurtleEggGoal`, drowned/husk conversion timers and NBT, baby speed/XP modifiers.

**Done:** A drowned throws a trident in water and on land, does not burn while jumping in water, and converts after the vanilla timer. A husk converts when submerged. `#3069`, `#2162`, `#3784`.

**Issues/PRs:** `#3468` §1, `#4039`.

### B02 — Skeleton family `P0`

**Scope:** Replace the simultaneous bow+melee registration with `reassessWeaponGoal()` on held-item change. Add `RestrictSunGoal`, `FleeSunGoal`, `AvoidEntityGoal<Wolf>`. Verify stray/bogged/parched/wither skeleton do not inherit the bow incorrectly.

**Done:** A skeleton with a bow shoots and keeps distance; a skeleton with a sword closes and melees; a skeleton in sunlight seeks shade. `#2537` (referenced) and `#3988` are covered.

**Issues/PRs:** `#3468` §1, `#3988`.

### B03 — Spider and cave spider `P0`

**Scope:** `LeapAtTargetGoal`, `SpiderAttackGoal`, light-gated `SpiderTargetGoal`, `AvoidEntityGoal`, wall-climber navigation, powder-snow climb.

**Done:** A spider climbs a wall, leaps at a player, and only targets in the correct light. A cave spider avoids armadillos.

**Issues/PRs:** `#3468` §1.

### B04 — Enderman `P0`

**Scope:** `EndermanFreezeWhenLookedAtGoal`, gravity/teleport target validation, `ResetUniversalAngerTargetGoal`, carried-block placement validation, water-potion damage, anger reset on player death.

**Done:** An enderman freezes when looked at, falls instead of hovering, only places carried blocks where they survive, and resets anger. `#2872`, `#3785`, `#3566`, `#3806`.

**Issues/PRs:** `#3468` §1, `#3815`, `#3816`.

### B05 — Iron golem `P0`

**Scope:** Remove the unconditional player target (`#3430`); add `MoveBackToVillageGoal`, `GolemRandomStrollInVillageGoal`, `LookAtPlayerGoal`, `ResetUniversalAngerTargetGoal`; wire reputation/gossip.

**Done:** An iron golem ignores an unprovoked player, attacks a player who lowers village reputation, and returns to the village. `#3430`, `#4124`.

**Issues/PRs:** `#3468` §1.

### B06 — Blaze `P1`

**Scope:** Flying navigation, zero gravity, `MoveTowardsRestrictionGoal`, fireball attack cooldown and sound. `#1960` reports blazes cannot fly.

**Done:** A blaze hovers, moves toward its home area, and fires at the vanilla cadence.

**Issues/PRs:** `#3468` §1, `#3790`, `#4113`.

### B07 — Vex `P1`

**Scope:** `VexChargeAttackGoal`, `VexRandomMoveGoal`, `VexCopyOwnerTargetGoal`, flying movement, summoned lifetime.

**Done:** A vex charges, copies the evoker's target, and dies when its lifetime expires.

**Issues/PRs:** `#3468` §1.

### B08 — Phantom `P1`

**Scope:** `PhantomAttackStrategyGoal`, `PhantomSweepAttackGoal`, `PhantomCircleAroundAnchorGoal`, `PhantomAttackPlayerTargetGoal`, target selector, swoop damage.

**Done:** A phantom circles at altitude, swoops, and damages a sleeping/awake player per vanilla. `#3629`.

**Issues/PRs:** `#3468` §1, `#3715`.

### B09 — Guardian and elder guardian `P1`

**Scope:** Beam attack (charge, aim, damage, sound), `MoveTowardsRestrictionGoal`, `RandomStrollGoal`, two `LookAtPlayerGoal`s, `RandomLookAroundGoal`, `GuardianMoveControl`, water pathfinding malus 0, elder mining fatigue and spike damage.

**Done:** A guardian charges and fires a beam; an elder applies mining fatigue and deals spike damage.

**Issues/PRs:** `#3468` §1.

### B10 — Wither `P1`

**Scope:** `WitherDoNothingGoal`, `RangedAttackGoal` (skull projectiles), `WaterAvoidingRandomFlyingGoal`, flying movement, `HurtByTargetGoal`, `NearestAttackableTargetGoal`, half-health armor/explosion phases.

**Done:** A wither flies, fires skulls, becomes armored at half health, and explodes on death. `#2220`-adjacent dragon/wither combat is covered in `F10`.

**Issues/PRs:** `#3468` §1.

### B11 — Ender dragon `P1`

**Scope:** Drive knockback and damage from the `EnderDragonPart` hitboxes (wing/head/neck), gate on `hurt_time == 0`, exclude creative/spectator, use push semantics, add the 2-tick contact guard, register parts as hit-testable entities, and open the exit portal on death.

**Done:** A player standing on the dragon's body is not knocked back every tick; wing/head hits deal the vanilla damage; the exit portal opens. `#2220`, `#3561`, `#3382`, `#2065`.

**Issues/PRs:** `#3468` §1, `#3407`, `#3661`.

### B12 — Magma cube and sulfur cube `P1`

**Scope:** Magma cube `CubeMobAttackGoal` and target; sulfur cube entity, tempt/search goals, size sync, block absorption/ejection. The fork already has `sulfur_cube.rs` and `fb850f755`; verify against vanilla 26.3.

**Done:** Both cubes jump-attack, split, and (sulfur) absorb/eject blocks. `#3114`.

**Issues/PRs:** `#3114`, `#3795`.

### B13 — Silverfish and endermite `P2`

**Scope:** `ClimbOnTopOfPowderSnowGoal`, `SilverfishWakeUpFriendsGoal`, `SilverfishMergeWithStoneGoal`, Silk Touch check, specialised silverfish AI on infested-block break. `#3615`.

**Done:** Breaking infested stone with Silk Touch does not spawn a silverfish; a silverfish wakes nearby infested blocks and merges into stone.

**Issues/PRs:** `#3468` §1, `#3886`.

### B14 — Illagers and witch `P1`

**Scope:** Illusioner bow/spells/avoid, vindicator Johnny and break-door, pillager/evoker avoid goals, witch heal-raiders and attack-player targets, ravager stun/roar.

**Done:** An illusioner casts blindness/mirror and shoots; a Johnny vindicator attacks every mob; a witch heals a hurt raider. `#1302` loot is covered in `B34`.

**Issues/PRs:** `#3468` §1.

### B15 — Creeper `P1`

**Scope:** Cat/ocelot avoid goals and the potion-effect cloud on explosion. `#3626`.

**Done:** A creeper flees a cat and leaves the correct area-effect cloud when it has an effect.

**Issues/PRs:** `#3468` §1, `#3663`.

### B16 — Wolf `P0`

**Scope:** `SitWhenOrderedToGoal`, `LeapAtTargetGoal`, `TamableAnimalPanicGoal`, `ResetUniversalAngerTargetGoal`, `isAngryAt` gating, prey target list (sheep, rabbit, fox, baby turtle, skeleton variants, bat), `NonTameRandomTargetGoal`.

**Done:** A wolf stays seated when told, does not aggro unprovoked, shows angry eyes when angry, and hunts its vanilla prey. `#2789`, `#2998`.

**Issues/PRs:** `#3468` §2, `#2795`.

### B17 — Cat and ocelot `P1`

**Scope:** Sit, lie on bed, sit on block, relax on owner, avoid players, leap/pounce, ocelot tempt/avoid, `NonTameRandomTargetGoal`.

**Done:** A cat sits, lies on a bed, relaxes on its owner, and pounces at a target. An ocelot avoids players and can be tempted.

**Issues/PRs:** `#3468` §2, `#3984`.

### B18 — Parrot `P1`

**Scope:** Taming, `SitWhenOrderedToGoal`, `FollowOwnerGoal`, `LandOnOwnersShoulderGoal`, `FollowMobGoal`, `ParrotWanderGoal`, flying navigation and move control.

**Done:** A parrot can be tamed, follows, lands on the owner's shoulder, and flies.

**Issues/PRs:** `#3468` §2.

### B19 — Rabbit, polar bear, fox, panda `P1`

**Scope:** Rabbit crop raid and hop control; polar bear attack/melee/anger/stand; fox prey targets and the full fox goal set (faceplant, stalk, pounce, shelter, sleep, berries, item search, perch, village stroll, defend trusted, powder snow); panda panic/breed/attack/avoid/sit/lie/sneeze/roll/hurt.

**Done:** Each signature behavior is exercised by GameTest. `#3262` (rabbit movement/animation).

**Issues/PRs:** `#3468` §2, `#4093`.

### B20 — Horse family `P1`

**Scope:** `RunAroundLikeCrazyGoal`, `MountPanicGoal`, `RandomStandGoal`, taming/temper, ridden movement control, saddle/armor, chest inventory for donkey/mule, skeleton trap, zombie horse riding.

**Done:** An untamed horse bucks a player, taming succeeds after the vanilla cycle, a donkey chest opens, and a skeleton trap spawns riders.

**Issues/PRs:** `#3468` §2, `#3582`-adjacent villager work.

### B21 — Llama and trader llama `P1`

**Scope:** `LlamaFollowCaravanGoal`, `LlamaHurtByTargetGoal`, `LlamaAttackWolfGoal`, `TraderLlamaDefendWanderingTraderGoal`, caravan formation, chest inventory/screen.

**Done:** A llama follows a caravan, spits at wolves, and a trader llama defends the wandering trader.

**Issues/PRs:** `#3468` §2, `#3961` (chest boat, not llama).

### B22 — Turtle and strider `P1`

**Scope:** Turtle panic/breed/lay egg/go to water/go home/travel/stroll; strider lava navigation, lava walking, cold/suffocating state.

**Done:** A turtle returns home and lays eggs; a strider walks on lava and shivers when cold.

**Issues/PRs:** `#3468` §2.

### B23 — Aquatic mobs `P0`

**Scope:** Water-bound navigation and smooth swim controls for cod, salmon, tropical fish, pufferfish, squid, glow squid, dolphin, tadpole, axolotl, nautilus, drowned. Add fish panic/avoid/swim/school, pufferfish puff, squid random/flee, dolphin breath/treasure/swim-with-player/jump/play/avoid.

**Done:** Fish school and flee, squid jet, dolphins surface to breathe and lead to treasure, axolotls play dead, and none bob on land. `#4126`, `#4005`, `#4090`, `#4063`.

**Issues/PRs:** `#3468` §2, `#3718`, `#4030`, `#3649`, `#4050`.

### B24 — Bee `P1`

**Scope:** All twelve bee goals: attack, enter/validate/locate hive, validate/locate flower, pollinate, go to hive/known flower, grow crop, wander; hive/flower memory, night/rain docking, stinger/nectar, anger.

**Done:** A bee enters its hive at night, exits in the morning, pollinates, grows crops, and loses its stinger.

**Issues/PRs:** `#3468` §2.

### B25 — Bat `P2`

**Scope:** Rest, attach, flight, flying navigation.

**Done:** A bat hangs, flies erratically, and rests.

**Issues/PRs:** `#3468` §2.

### B26 — Villager `P0`

**Scope:** Migrate to the Brain stack (`VillagerAi`, `VillagerGoalPackages`, activities sleep/work/meet), sensors (`VillagerBabies`, `VillagerHostiles`, `SecondaryPoi`, `NearestBed`), gossip/reputation, breeding/baby behavior, work-site and bed claims, restock, schedule.

**Done:** A village has working beds, work sites, meeting, breeding, restock, gossip, iron-golem spawning, and villager sleep. `#3761`, `#3742`, `#4124`, `#3581`.

**Issues/PRs:** `#3582` (draft), `#3693`, `#3774`.

### B27 — Wandering trader `P1`

**Scope:** Natural spawning lifecycle (1200-tick check, 24000-tick delay, 25-75 failure chance, gamerule gates, level.dat persistence of delay/chance/id), placement (bell within 48 blocks, height/biome), 48000-tick despawn, two leashed trader llamas, trade offers/unlocks, night invisibility/day milk, hostile avoidance, saved wander targets.

**Done:** A trader spawns on the vanilla cadence, trades correctly, despawns, and is persisted across restart. `#3364`, `#3083`.

**Issues/PRs:** `#3765`. Local branch `feat/vanilla-parity-batch` already has `0847909c5 feat(entity): port wandering trader spawning and lifecycle`; verify and land or supersede it.

### B28 — Raids `P1`

**Scope:** Raid trigger from bad omen, wave composition, raider pathing through the village, bell revealing, hero of the village, raid persistence.

**Done:** A bot with bad omen enters a village, triggers a raid, clears waves, and gets the vanilla reward. `#3631`, `#4124`.

**Issues/PRs:** `#3673`.

### B29 — Piglin and piglin brute `P0`

**Scope:** Phase 1 Brain pilot: `PiglinAi`, `PiglinSpecificSensor`, `PiglinBruteAi`; delete `PiglinActivity` and the timer-to-NBT hack; rebind `GoToWantedItemGoal` to a generic item sensor + walk target. Barter, admire, celebrate, anger, retreat, brute home.

**Done:** A piglin barters, admires gold, celebrates a hunt, retreats from a hoglin, and keeps its memories across save/load. `#3376`, `#2998` symptoms improve.

**Issues/PRs:** `#3468` §4 Phase 1, `#3630`.

### B30 — Hoglin and zoglin `P1`

**Scope:** Repellent/retreat activities, throw target and velocity, conversion, zoglin living-target selection beyond players. `#3974`.

**Done:** A hoglin retreats from warped fungus and throws a player; a zoglin attacks non-player living entities.

**Issues/PRs:** `#4095`.

### B31 — Frog, tadpole, axolotl `P1`

**Scope:** Frog tongue attack, breed/egg laying, frog-specific jump; tadpole growth; axolotl brain migration, play-dead, drying, bucket data.

**Done:** A frog eats a small magma cube, lays eggs, and tadpoles grow; an axolotl plays dead and keeps its color in a bucket.

**Issues/PRs:** `#3468` §4 Phase 3, `#3971`.

### B32 — Warden `P0`

**Scope:** Vibration listening, sniffing, anger-based target selection, sonic boom, emergence/dig, darkness effect, shrieker summon. The fork has `warden.rs`, `warden_spawn_tracker.rs`, and an anger PR; verify the full chain.

**Done:** A warden hears a vibration, investigates, sniffs, angers, and fires a sonic boom that ignores walls and deals vanilla damage. `#4122`, `#4123`.

**Issues/PRs:** `#3612`.

### B33 — Remaining brain mobs `P1`

**Scope:** Allay collection/delivery/note-block, copper golem sorting/oxidation/statue, happy ghast harness/riding, camel sit/stand, armadillo rolling, goat ram/long jump/milk, sniffer sniff/search/dig, creaking heart/stillness, breeze wind charge/slide/jump, nautilus swim/shell/ride.

**Done:** Each mob's signature behavior is exercised; each has a Brain NBT round-trip test.

**Issues/PRs:** `#3468` §4 Phase 3, `#4115`, `#4050`.

### B34 — Mob loot, drops, and trades `P1`

**Scope:** Correct loot tables and drop conditions for witch, zombie, skeleton, creeper, sheep, mooshroom, chicken, rabbit, fish, wither, dragon, and raid mobs. Trade data for villagers and wandering traders. `#1302`, `#2776`, `#2769`, `#3240`, `#3225`.

**Done:** A kill harness compares every mob's drops against vanilla for 100 kills with and without Looting.

**Issues/PRs:** `#3237`, `#4105`, `#3765`.

### B35 — Shulker, ghast, slime, snow golem, ominous item spawner `P1`

**Scope:** Verify the mobs `#3468` calls "already matching" and finish the ones it does not list. Shulker peek/teleport/bullet/armor; ghast fireball and flight; slime/magma cube size and split; snow golem snowball and melt; ominous item spawner item selection and spawn. Also verify zombified piglin spear behavior and group alert.

**Done:** Each mob's signature behavior is exercised by GameTest; the tracker's "partial" rows for these mobs are closed or downgraded with a note.

**Issues/PRs:** `#3468` §1, `#2359`.

---

## 6. Entities, vehicles, and decoration (W3)

### C01 — Item frames `P1`

**Scope:** Placement validation (support block, survival), rotation, item insertion/removal, drops, glow variant, map display.

**Done:** An item frame can be placed, rotated, filled, emptied, and broken with the correct drop. `#3767`, `#3103`.

**Issues/PRs:** `#3776`.

### C02 — Paintings `P1`

**Scope:** Placement, variant selection, collision, drops, destruction by arrows/fire, width/height.

**Done:** A painting places at the vanilla variant, drops itself when broken, and has the correct hitbox. `#3622`, `#3108`, `#3103`.

**Issues/PRs:** `#3775`.

### C03 — Armor stands `P1`

**Scope:** Equipment slots, player interaction (equip/swap/remove), pose, drops on break, fire/burn damage tags, death particles, name.

**Done:** A player can equip and swap armor, the stand drops its equipment, and it burns per vanilla. `#3614`, `#1621`.

**Issues/PRs:** `#3414`, `#3684`.

### C04 — Boats and rafts `P0`

**Scope:** Water-status/buoyancy controller, placement at the raycast hit, breaking by hand, item drop, passenger movement, chest boat inventory, bamboo raft collision, `#1679`.

**Done:** A boat floats, bobs, moves with passengers, can be broken, and a chest boat opens its inventory. `#4073`, `#3061`, `#3068`, `#3577`, `#3082`, `#1679`.

**Issues/PRs:** `#4073`, `#3961`.

### C05 — Minecarts `P1`

**Scope:** Rail physics, uphill movement, hopper minecart stack pickup, TNT/spawner/command-block minecart state and ticking, activator rail.

**Done:** A minecart climbs a powered slope, a hopper minecart picks up a stack at once, and a command-block minecart runs its command. `#3071`, `#3880`.

**Issues/PRs:** `#4021` (Bedrock experimental).

### C06 — Falling blocks `P1`

**Scope:** Cobweb slowdown, landing/drop behavior, position sync, fluid replacement, `#3672`, `#2467`.

**Done:** Sand slows in cobwebs, lands as a block, drops as an item when it cannot land, and clients see it fall.

**Issues/PRs:** `#3417`, `#3479`, `#3672`, `#3681`.

### C07 — Experience orbs `P2`

**Scope:** Float in water, merge, pickup, despawn, 32-bit limit behavior. `#3624`, `#3092`, `#2094`.

**Done:** Orbs float, merge, and are picked up with the correct XP.

**Issues/PRs:** `#3644`.

### C08 — Item entities `P1`

**Scope:** Water pickup, stacking on contact, hopper interaction, despawn, explosion pre-merge, `#3368`.

**Done:** Items floating in water can be picked up; dropped stacks merge; a hopper picks up a whole stack.

**Issues/PRs:** `#4054`, `#3460`.

### C09 — Leash knots and leads `P2`

**Scope:** Persist leashes, destroy knot on hit, leash physics, untie on distance.

**Done:** A leashed mob keeps its lead across save/load and the knot breaks when hit.

**Issues/PRs:** `#3800`.

### C10 — 26.3 decoration entities `P1`

**Scope:** Cushion placement/sitting/color, straw bed sleep/destroy, mannequin, shelf mushroom entity behavior.

**Done:** Each entity places, interacts, drops, and persists. `#3871`.

**Issues/PRs:** `#3729`, `#3871`.

### C11 — Entity metadata, NBT, and data tracker `P0`

**Scope:** Correct metadata for every entity, data-tracker initialization for natural spawns, NBT round-trip, custom names, variants, equipment, leashes, and persistent flags. `#3359`, `#2533`, `#3717`.

**Done:** A vanilla-saved entity loads with the same metadata; a Pumpkin-saved entity loads in vanilla. A metadata diff test covers every tracked field.

**Issues/PRs:** `#3717`.

### C12 — Entity tracking, visibility, and persistence `P0`

**Scope:** Per-player visibility (show/hide, visible-by-default), non-persistent entities, tracking range, despawn/respawn across chunk unload, `#3385`, `#3734`.

**Done:** A plugin can hide an entity from one player, a non-persistent entity is removed on unload, and a persistent entity survives. `#3734`.

**Issues/PRs:** `#3734`, `#3432`.

### C13 — Spawn eggs `P1`

**Scope:** Spawn the correct entity with the correct variant, baby/adult, and data; consume in survival; dispenser behavior. `#4036`, `#2536`.

**Done:** A spawn egg spawns the right entity and baby; a dispenser spawns it.

**Issues/PRs:** `#4036`, `#3436`.

### C14 — Taming, shearing, and interaction `P1`

**Scope:** Tame animals, shear sheep/mooshroom/beehive, milk cow/goat, saddle, dye, name tag, lead. `#2535`, `#3081`, `#3830`.

**Done:** Each interaction produces the vanilla item/state/sound.

**Issues/PRs:** `#3829`.

### C15 — Entity events and animations `P1`

**Scope:** Correct entity-event packets for attack, hurt, death, shear, love, taming, villager trade, warden attack, and animation states. `#2991`, `#4123`, `#3830`.

**Done:** A real client shows the vanilla animation for each event; packet captures match vanilla IDs.

---

## 7. Blocks and redstone (W4)

### D01 — Redstone engine correctness `P0`

**Scope:** Wire propagation/update order, hard vs soft powering, tick priority, neighbor updates, state desync, daylight detector output, and `#2699`, `#2601`.

**Done:** A redstone test suite (wire, torch, repeater, comparator, piston, observer, rail, daylight detector) matches vanilla timing and power for 50 circuits.

**Issues/PRs:** `#1402` (tracker is stale: it marks pistons broken but the code exists), `#3863`.

### D02 — Pistons `P0`

**Scope:** Movement checks, retraction, block swapping, moving-piston NBT, head/base removal, player context, client sync, `#3900`, `#3833`, `#3144`, `#1564`.

**Done:** Sticky/normal pistons push/pull the same blocks as vanilla, do not corrupt on rapid toggles, persist moving blocks across save/load, and clients see the movement. `#3057`, `#4026`, `#4047`, `#4019`, `#4007`, `#2995`.

**Issues/PRs:** `#1402`, `#3057`, `#4026`, `#4047`, `#4019`, `#4007`, `#2995`.

### D03 — Rails `P1`

**Scope:** Power extension boundaries for powered/activator rails, slopes, curves, minecart activation, `#3071`.

**Done:** A rail circuit powers the exact vanilla block range; a minecart climbs and activates.

**Issues/PRs:** `#1402`.

### D04 — Dispensers and droppers `P0`

**Scope:** Bonemeal, experience bottles, filled buckets (water/mob/powder snow), shears, chest equipment, brush/armadillo, boat/minecart placement, `#3822`, `#3627`, `#3625`, `#3518`, `#3972`, `#3970`, `#3969`.

**Done:** A dispenser behaves like vanilla for every item in the dispenser behavior registry; unsupported items drop instead of duplicating. `#3920`, `#3695`, `#3645`.

**Issues/PRs:** `#3640`, `#3695`, `#3920`.

### D05 — Hoppers `P1`

**Scope:** Cooldown uses `tickedGameTime`, whole-stack pickup, double chests, hopper minecart, `#3881`, `#3880`, `#3965`.

**Done:** A hopper moves items at the vanilla rate and picks up a stack at once.

**Issues/PRs:** `#3965`, `#3804`.

### D06 — Observers `P0`

**Scope:** Detect the correct state changes, pulse length, `#3105`. PR `#3863` claims a fix; verify.

**Done:** An observer pulses once for a block update, not for its own placement, and matches vanilla timing.

**Issues/PRs:** `#3863`.

### D07 — Sculk sensors, shriekers, and vibrations `P0`

**Scope:** Implement a vibration/game-event dispatch system: frequency per event, `GameEvent` listener radius, sensor phase/power, calibrated sensor filtering, shrieker summon, warden hearing. Today `world.emit_game_event` only fires a plugin event; `SculkSensorBlock::trigger` has no callers. `#3107`, `#4122`.

**Done:** A sensor detects a step, a block place, a shriek, and a projectile; a shrieker summons a warden; a calibrated sensor filters by frequency. `#4122` (warden sonic boom) is covered in `B32`.

**Issues/PRs:** `#1402` marks sculk done, which is wrong at the vibration layer.

### D08 — Bells `P1`

**Scope:** Ring by hand/arrow, game event, raider glow, animation, `#3003`, `#3002`, `#3631`.

**Done:** A bell rings, glows raiders, and plays the animation.

**Issues/PRs:** `#3673`.

### D09 — Tripwire and hooks `P1`

**Scope:** State-change game events, shears durability, trap-avoidance filtering, `#3623`, `#3574`.

**Done:** A tripwire emits the vanilla events, shears lose durability, and trap-avoiding entities do not trigger it.

**Issues/PRs:** `#3584`, `#3632`.

### D10 — Containers and block entities `P0`

**Scope:** Chests (double, diagonal extension), ender chest, shulker box, lectern, jukebox, composter, campfire, furnace family, crafting table, barrel, hopper, dispenser/dropper, beacon, brewing stand, enchanting table, anvil, smithing table, stonecutter, loom, cartography table, grindstone, crafter, vault, trial spawner. `#4011`, `#3548`, `#3293`, `#3020`, `#2698`, `#3621`, `#3643`, `#3646`.

**Done:** Every container opens/closes with the correct game events, persists its contents, and its comparator output is correct.

**Issues/PRs:** `#4015`, `#3549`, `#3546`, `#3592`, `#3655`, `#3656`, `#3658`, `#3660`, `#3860`.

### D11 — Beds and respawn anchors `P1`

**Scope:** Sleeping damage/wake, occupied bed, explosion fire, uncharged anchor outside the Nether, straw bed, `#4064`, `#3888`, `#3581`, `#3514`, `#3521`.

**Done:** Sleeping is safe, damage wakes the player, occupied beds are handled, and explosions create fire.

**Issues/PRs:** `#3904`, `#4097`, `#3523`, `#3547`, `#3693`.

### D12 — Game-event coverage across blocks `P1`

**Scope:** Emit the correct game event for container open/close, jukebox insert/eject, chiseled bookshelf, bell, tripwire, button, note block, door/trapdoor, lever, sculk, and block place/destroy. This is the data source for `D07`.

**Done:** A game-event capture for a scripted player session matches vanilla's event sequence for 20 interactions.

### D13 — Block edge cases `P1`

**Scope:** Falling blocks/snow/ice, farmland/dirt path, sugar cane, lily pad, big dripleaf, crops/light, mushrooms, scaffolding, infested blocks, `#2467`, `#3672`, `#3781`, `#3855`, `#3837`, `#3560`, `#3558`, `#3557`, `#3552`, `#3516`, `#3111`, `#3060`, `#1871`, `#3615`.

**Done:** Each named issue has a regression test or a real-client check.

**Issues/PRs:** `#3415`, `#3416`, `#3417`, `#3438`, `#3555`, `#3556`, `#3559`, `#3562`, `#3585`, `#3592`, `#3826`, `#3868`, `#3869`, `#3871`.

### D14 — Fire spread and burnout `P1`

**Scope:** Natural lava fire, floating fire, environment-attribute-driven burnout, `#3772`, `#3659`.

**Done:** Fire spreads at the vanilla rate, burns out based on the environment attribute, and does not float after its fuel is gone.

**Issues/PRs:** `#3869`.

### D15 — Copper weathering and waxing `P1`

**Scope:** Preserve block state on waxing, oxidation ticks, scraping, `#3567`.

**Done:** Waxing a trapdoor keeps orientation/open state; oxidation advances on random ticks.

**Issues/PRs:** `#4116`.

### D16 — 26.3 block set `P1`

**Scope:** Verify and finish the 26.3 blocks: 16 wool stairs/slabs, 16 concrete stairs/slabs, shelf mushroom (small/large, bone meal, drops, stew, bounciness), red shrub (placement, composting, spread), poplar wood set (sapling, growth, leaves, boats, signs, recipes), cushion, straw bed. The fork has most of these; audit the runtime, recipes, tags, and loot.

**Done:** Every 26.3 block/item has a behavior test and a recipe/loot test. `#3519` can be closed or narrowed.

**Issues/PRs:** `#3519`, `#3729`, `#3871`, `#3741`.

### D17 — Placement and support validation `P1`

**Scope:** A sweep of `can_place_at`/support predicates for torches, signs, banners, buttons, pressure plates, rails, plants, snow, cactus, sugar cane, lily pad, scaffolding, and redstone components.

**Done:** A placement matrix test compares Pumpkin with vanilla for 200 block/face combinations.

### D18 — Block entity NBT and save `P0`

**Scope:** Correct `write_nbt`/`read_nbt` for every block entity, remove NBT when the block is removed, preserve data on place, `/data` edits the live entity, and save on `stop`. `#3902`, `#3859`, `#3875`, `#3807`, `#2626`.

**Done:** A chest, furnace, hopper, sign, banner, skull, jukebox, lectern, beehive, and trial spawner survive save/load and `/data` round-trip.

**Issues/PRs:** `#3634`, `#3859`, `#3875`, `#3807`.

### D19 — Block-specific stragglers `P1`

**Scope:** Close the remaining tracker gaps that do not belong to a larger family: conduit (power/attack), dragon egg (teleport/fall), lodestone (compass binding), note block (instrument/tune), structure block and jigsaw block (capture/save/load/execute), beehive/bee nest (release/tick), frogspawn (hatch), sea pickles (growth/light), pumpkin/melon stems (growth/fruit), daylight detector (light update), moving piston (player context), big dripleaf (projectile tilt), scaffolding (placement), infested blocks (Silk Touch/AI), barrier/light blocks, debug stick, mob spawner, creaking heart, decorated pot, flower pot, candles/cake, cauldron, and end portal frame.

**Done:** Each block has a behavior test; the tracker rows are closed or moved to `T14`. `#3107`, `#3293`, `#3020`, `#3548`, `#3560`, `#3558`, `#3557`, `#3552`, `#3615`, `#1871`.

**Issues/PRs:** `#3860`, `#3416`, `#3555`, `#3556`, `#3559`, `#3810`, `#3868`.

### D20 — Mining, hardness, tools, and block drops `P1`

**Scope:** Block hardness, tool tier and correct-tool checks, break progress, instant-break blocks, creative break, drops with Silk Touch/Fortune, XP drops, break particles/sounds, waterlogged breaks, and spawn-protection checks. `#3111`, `#3120`, `#3250`, `#3615`, `#3737`, `#3415`.

**Done:** A mining matrix test compares break time and drops for 100 block/tool combinations with vanilla; a real client sees the correct break overlay and sound.

**Issues/PRs:** `#3415`, `#3737`.

### D21 — Crop and plant growth `P1`

**Scope:** Random-tick growth, bone meal, hydration, light level, stems, saplings, leaf decay, vines, bamboo, sugar cane, cactus, kelp, coral, chorus, nether wart, cocoa, berries, mushrooms, pumpkins/melons, and sea pickles. `#3516`, `#3558`, `#3557`, `#3292`, `#3060`, `#3250`.

**Done:** A growth harness advances random ticks and compares the block state sequence with vanilla for each plant.

**Issues/PRs:** `#3558`, `#3557`, `#3592`.

---

## 8. Items, inventory, recipes, and loot (W5)

### E01 — Buckets `P0`

**Scope:** Water/mob/powder snow bucket parity: return empty bucket, preserve mob data, use seagrass water, replace replaceable blocks, dispenser behavior, hand context. `#3973`, `#3971`, `#3969`, `#3933`, `#3518`.

**Done:** Every bucket interaction produces the vanilla item and block state, including axolotl/tadpole/fish colors and powder snow visibility.

**Issues/PRs:** `#3940`, `#3976`, `#3986`, `#3990`, `#4059`.

### E02 — Fishing `P1`

**Scope:** Bobber flight, water detection, bite timing, loot table with luck/enchantment context, XP, rod durability, `#3522`, `#3051`.

**Done:** Fishing yields the vanilla loot distribution (with and without Luck of the Sea), XP, and durability loss.

**Issues/PRs:** `#3821`.

### E03 — Fireworks `P1`

**Scope:** Flight duration, power (1/2/3), explosions, colors, shape, elytra boost, crossbow launch, `#4089`, `#3462`, `#3697`, `#1274`.

**Done:** A rocket with Power 3 flies and explodes like vanilla; an elytra boost works.

**Issues/PRs:** `#3462`, `#3697`.

### E04 — Wind charge, mace, spear, trident `P1`

**Scope:** Wind charge launch/burst, mace smash damage/fall reset, spear use/lunge, trident throw/riptide/return, `#3873`, `#3876`, `#3918`, `#1552`.

**Done:** Each weapon matches vanilla damage, knockback, cooldown, and sound.

**Issues/PRs:** `#3876`, `#3905`.

### E05 — Consumables, potions, brewing `P1`

**Scope:** Eating/drinking times, saturation/hunger, potion effects, brewing stand recipes, splash/lingering, tipped arrows, `#3412`, `#2433`.

**Done:** A brewing stand produces every vanilla potion; eating/drinking restores the right hunger/effects.

**Issues/PRs:** `#3412`.

### E06 — Tools `P1`

**Scope:** Shears (sheep, mooshroom, beehive, tripwire), hoe, axe, shovel, brush, spyglass, goat horn, `#3081`, `#3113`, `#3822`, `#2321`.

**Done:** Each tool's durability, drops, and sound match vanilla.

**Issues/PRs:** `#3829`, `#3348`.

### E07 — Maps, compass, clock, ender eye `P2`

**Scope:** Map rendering/updates, player icon direction, compass/lodestone, clock, ender eye, `#4110`.

**Done:** A map updates as the player explores; a lodestone compass points to its lodestone.

**Issues/PRs:** `#4110`.

### E08 — Bundles and books `P1`

**Scope:** Bundle insert/remove/drop, tooltip, sync; writable book, book signing, knowledge book, `#3896`.

**Done:** Bundle clicks match vanilla; a written book persists and can be signed.

**Issues/PRs:** `#3896`.

### E09 — Spawn eggs and taming items `P1`

**Scope:** Covered by `C13`/`C14`; this task owns the item-side behavior and dispenser interaction.

**Done:** Spawn eggs and taming items behave through both hand and dispenser.

### E10 — Inventory click, dupe, offhand, shift `P0`

**Scope:** Shift-click, drag, double-click, offhand pickup, crafting result refill, cursor stack, dupe fixes (`#3917`, `#3847`, `#3292`, `#3232`, `#2833`, `#4104`).

**Done:** An inventory fuzz test performs 10,000 random clicks and never creates or destroys items. `#4118`, `#4119`, `#4057`, `#3922`.

**Issues/PRs:** `#4118`, `#4119`, `#4057`, `#3922`, `#4104`, `#3307`.

### E11 — Equipment, death, durability, curses `P1`

**Scope:** Armor equip/unequip, death drops, durability loss, Curse of Binding, Curse of Vanishing, `/item`, `#3391`, `#3542`, `#3039`, `#4120`.

**Done:** Armor is not deleted on death, curses work, and durability persists across drop/pickup.

**Issues/PRs:** `#3395`, `#3543`, `#3544`, `#3641`, `#4120`.

### E12 — Container UIs `P1`

**Scope:** Anvil, enchanting table, grindstone, loom, smithing table, stonecutter, cartography table, merchant, beacon, crafter, horse/donkey, boat chest. Every screen opens, syncs, and closes.

**Done:** A bot opens every container, moves an item, closes, and sees the same state after reopen.

### E13 — Recipe types `P0`

**Scope:** `smithing_transform`, `smithing_trim`, `stonecutting`, `crafting_transmute`, `crafting_decorated_pot`, `crafting_special_*` (firework, map cloning/extending, banner, book, shield, repair, tipped arrow), brewing, and the merchant recipe API needed for plugin parity. Today `parse_recipe` only handles shaped/shapeless, cooking, and brewing.

**Done:** Every vanilla 26.3 recipe loads and can be crafted; the recipe book shows it. `#3917`.

**Issues/PRs:** `#3096` recipe checklist.

### E14 — Recipe loading and reload `P1`

**Scope:** Datapack recipe loading, vanilla data pack, tag ingredients, recipe book sync, `/recipe`, knowledge book.

**Done:** A datapack recipe replaces a vanilla recipe after `/reload`; the client recipe book updates.

**Issues/PRs:** `#731`, `#3276`.

### E15 — Loot functions `P0`

**Scope:** Implement the vanilla loot-function set: `set_count`, `set_damage`, `set_components`, `set_nbt`, `set_name`, `set_lore`, `copy_components`, `copy_name`, `copy_state`, `copy_custom_data`, `enchant_randomly`, `enchant_with_levels`, `looting_enchant`, `apply_bonus`, `furnace_smelt`, `explosion_decay`, `limit_count`, `set_stew_effect`, `set_potion`, `set_custom_model_data`, `fill_player_head`, `set_banner_pattern`, `set_instrument`, `set_fireworks`, `set_firework_explosion`, `set_written_book_pages`, `toggle_tooltips`, `set_ominous_bottle_amplifier`, `set_goat_horn_sound`, `set_random_dyes`, `set_random_potion`, `set_container_loot`, `set_loot_table`, `set_enchantments`, `modify_contents`, `filtered`, `sequence`, `reference`, plus any 26.3 additions. Today only `set_count`, `apply_bonus`, and `looting_enchant` are parsed.

**Done:** Every vanilla loot table loads without warnings; a loot harness compares 1,000 rolls per table with vanilla. `#3456`, `#4020`.

**Issues/PRs:** `#3456`, `#3507`, `#3725`, `#3237`, `#3418`. Local branch `feat/vanilla-parity-batch` already has `eb91eed91 feat(loot): support the copy_components loot function`; verify and land or supersede it.

### E16 — Loot context and RNG `P0`

**Scope:** Deterministic seed-based random sequences (1.20+), context parameters (tool, luck, explosion radius, killer, block state, weather, fire), entity properties, `#4020`.

**Done:** The same world seed and loot event produce the same loot as vanilla. A regression test fails if the RNG sequence changes.

**Issues/PRs:** `#4020`.

### E17 — Item components and NBT codecs `P0`

**Scope:** Every data component round-trips through NBT and network; custom data does not crash the client; invalid NBT is rejected safely; `#3084`, `#3073`, `#3728`, `#3894`.

**Done:** A component corpus round-trips; a fuzzed item stack never panics.

**Issues/PRs:** `#3706`, `#3728`, `#3894`, `#3999`.

### E18 — Item behavior hand context `P1`

**Scope:** Pass the used hand to `normal_use`/`normal_use_with_rotation`, write the result back to the correct hand, and fire item events. `#3575`, `#4059`.

**Done:** Using an item from the offhand consumes/returns in the offhand. `#4104`.

**Issues/PRs:** `#3575`, `#4059`.

---

## 9. Combat (W6)

### F01 — Damage pipeline `P0`

**Scope:** Armor, enchantments, resistance, absorption, i-frames, crits, sweep, attack cooldown, attack damage attribute. `#3383`, `#3140`, `#3515`, `#1330`, `#1622`, `#3062`.

**Done:** A damage harness compares final damage for 50 attacker/defender/weapon combinations with vanilla.

**Issues/PRs:** `#4080`, `#4120`.

### F02 — Shields `P0`

**Scope:** Block melee/projectile, axe disable, knockback, sound, `#3520`.

**Done:** A shield blocks a melee hit and an arrow, is disabled by an axe, and shows the vanilla animation. `#3525`, `#3544`, `#3637`.

**Issues/PRs:** `#3525`, `#3544`, `#3637`.

### F03 — Knockback `P0`

**Scope:** Attack knockback, explosion knockback, projectile knockback, resistance, stale motion packets, `#3515`, `#3464`, `#4112`.

**Done:** The same hit produces the same horizontal/vertical velocity as vanilla for a player, zombie, and iron golem.

**Issues/PRs:** `#3476`, `#3793`, `#4112`. Local branch `feat/vanilla-parity-batch` already has `6a21ce72e fix(world): scale explosion knockback by knockback resistance`; verify and land or supersede it.

### F04 — Attack range, line of sight, permissions `P1`

**Scope:** Server-side interaction range, line of sight, invalid entity handling, `#3513`, `#4058`.

**Done:** An attack outside range is rejected; an attack through a wall is rejected; an invalid entity id does not panic. `#1800`, `#3561`.

**Issues/PRs:** `#4058`. Local branch `feat/vanilla-parity-batch` already has `f67c091d3 fix(network): reject attacks outside the entity interaction range`; verify and land or supersede it.

### F05 — Bows, crossbows, and arrows `P1`

**Scope:** Bow charge/velocity/accuracy, crossbow loading states and the `#3844` client kick, multishot, piercing, tipped arrows, arrow damage/position/pickup, crit particles, `#3489`, `#3388`, `#2180`.

**Done:** A bow and crossbow shoot with vanilla velocity and damage; loading a crossbow does not kick the client; an arrow sticks, can be picked up, and shows the correct particles. `#4099`.

**Issues/PRs:** `#4099`, `#3844`, `#3978`-adjacent projectile work.

### F06 — Tridents and riptide `P1`

**Scope:** Throw, return, riptide in rain/water, impaling, loyalty, channeling, `#3918`, `#1552`.

**Done:** A trident returns, riptide launches in rain, and channeling summons lightning.

### F07 — End crystals `P1`

**Scope:** Placement, damage, knockback, explosion, healing the dragon, `#3690`.

**Done:** An end crystal placed on obsidian heals the dragon and explodes with vanilla damage. `#3694`.

**Issues/PRs:** `#3694`.

### F08 — TNT and explosions `P0`

**Scope:** Fuse, priming, damage, knockback, block destruction, fire, water resistance, `#4084`, `#3140`, `#3378`.

**Done:** TNT explodes after 80 ticks, destroys the vanilla block set, damages entities, and does not crash the client.

**Issues/PRs:** `#4112`, `#3547`.

### F09 — Bed and respawn-anchor explosions `P1`

**Scope:** Fire, block destruction, damage, `#3514`, `#3521`.

**Done:** A bed explodes in the Nether/End with fire; an uncharged anchor outside the Nether explodes.

**Issues/PRs:** `#3523`, `#3547`.

### F10 — Dragon and wither combat `P1`

**Scope:** Part hitboxes, contact damage, knockback, health bar, death sequence, exit portal, wither skulls, armor phases. Covered by `B10`/`B11` for AI; this task owns the combat/damage side.

**Done:** A player can damage the dragon by hitting a wing/head, takes the correct contact damage, and sees the boss bar.

**Issues/PRs:** `#3407`, `#3661`.

### F11 — Mob combat parity `P1`

**Scope:** Hoglin throw, creeper cloud, warden sonic boom, witch potions, blaze fireballs, guardian beam, ravager roar/stun, evoker fangs, shulker bullet.

**Done:** Each attack's damage, cooldown, sound, and particle match vanilla.

### F12 — Death, drops, XP, totem `P1`

**Scope:** Death animation, drops, XP, totem of undying, keep-inventory per dimension, mob drops gamerule, `#2566`, `#2776`, `#2769`, `#3092`, `#4117`, `#4105`, `#4125`.

**Done:** A death produces the vanilla drops/XP; a totem saves the player; keep-inventory respects the dimension. `#4117`, `#4105`.

**Issues/PRs:** `#4117`, `#4105`, `#3853`.

### F13 — 1.8 combat mode `P2`

**Scope:** Optional legacy combat (`#1404`). Decide with the release owner whether this is 1.0 or a plugin/config feature. If kept, implement attack cooldown bypass, sword blocking, and old knockback.

**Done:** A config profile reproduces 1.8 hit timing and blocking; default remains modern.

**Issues/PRs:** `#1404`, `#626`.

### F14 — Knockback configuration `P2`

**Scope:** `KnockbackSync`, custom knockback, legacy PvP config (`#626`). Keep defaults vanilla.

**Done:** Config changes do not affect vanilla defaults; a test verifies the default profile.

### F15 — Combat VFX `P1`

**Scope:** Crit/sweep particles, attack sounds, hurt flashes, death particles, `#1622`, `#3062`, `#3388`, `#2566`.

**Done:** A real client sees the vanilla VFX for a crit, sweep, arrow hit, and death.

---

## 10. Player systems (W7)

### G01 — Movement, flying, swimming `P0`

**Scope:** Walk/sprint/sneak/fly/swim states, collision, step height, water movement, creative flight, `#3899`, `#2625`, `#3648`, `#3647`, `#3070`.

**Done:** A bot movement trace matches vanilla for 20 scenarios; a real client does not desync.

**Issues/PRs:** `#3978`.

### G02 — Elytra `P0`

**Scope:** Equip/unequip, glide start/stop, firework boost, landing damage, removal while flying, death while gliding, `#3899`, `#3648`, `#3647`, `#3070`, `#1807`, `#1551`, `#1274`.

**Done:** A player glides, boosts, lands without false damage, and stops gliding when the elytra is removed. `#3459`, `#3657`, `#3978`.

**Issues/PRs:** `#3459`, `#3657`, `#3978`.

### G03 — Teleport and dimension change `P0`

**Scope:** `/tp`, portal travel, respawn, vehicle teleport, relative coordinates, chunk load before arrival, `#3389`, `#3836`, `#3464`.

**Done:** A player teleports across dimensions without visual desync or void death. `#3466`.

**Issues/PRs:** `#3466`.

### G04 — Health, damage, death, respawn `P0`

**Scope:** Damage sources, armor, absorption, death screen, respawn anchor/bed, hardcore spectator, `#4064`, `#3888`, `#1583`, `#2205`, `#4042`.

**Done:** Death and respawn work in every dimension; hardcore death puts the player in spectator. `#3506`, `#4042`.

**Issues/PRs:** `#3506`, `#4042`, `#3853`.

### G05 — Hunger, saturation, effects `P1`

**Scope:** Hunger drain, saturation, status effects, particles, HUD sync, `/effect`, `#2573`, `#2520`, `#764`, `#765`.

**Done:** A player's hunger and effects match vanilla after eating, sprinting, and taking potions.

**Issues/PRs:** `#2520`.

### G06 — Sleep and beds `P1`

**Scope:** Sleep, wake, occupied bed, damage while sleeping, respawn point, straw bed, `#4064`, `#3888`, `#3581`.

**Done:** Sleeping skips the night, damage wakes the player, and the spawn point is set. `#3904`, `#4097`, `#3693`.

**Issues/PRs:** `#3904`, `#4097`, `#3693`.

### G07 — Inventory and equipment interaction `P0`

**Scope:** Player inventory clicks, armor slots, offhand, cursor, drop, swap, `#2833`, `#3232`, `#3391`, `#3542`, `#4104`.

**Done:** The inventory fuzz test in `E10` also covers the player inventory.

### G08 — Chat and tab list `P1`

**Scope:** Chat, signed messages, tab list name/header/footer, team colors, `#3379`.

**Done:** Sending a message does not change the tab-list name; team colors render.

**Issues/PRs:** `#3379`.

### G09 — Gamemode and spectator `P1`

**Scope:** `/gamemode`, creative flight, spectator interactions, spectator camera, `#3053`, `#2600`, `#2224`, `#3538`.

**Done:** Gamemode changes keep rotation/position where vanilla does; spectator cannot interact. `#3744`.

**Issues/PRs:** `#3744`.

### G10 — Experience `P2`

**Scope:** XP gain, levels, orbs, 32-bit limit, `/xp`, `#2094`, `#3092`.

**Done:** XP gain/level-up matches vanilla; the integer limit is handled. `#3446`, `#3473`.

### G11 — Player data save/load `P0`

**Scope:** Atomic player save, periodic snapshot ordering, disconnect save, inventory/effects/advancements/stats, `#3511`, `#3512`, `#4048` (memory).

**Done:** A kill -9 during a save never leaves a truncated player file; the newest snapshot wins. `#3634`.

**Issues/PRs:** `#3634`.

### G12 — Input and text fields `P2`

**Scope:** Key repeat, text field editing, sign/book input, `#2222`.

**Done:** Text fields accept repeated keys and edits like vanilla.

### G13 — KeepInventory per dimension `P1`

**Scope:** `#4125` reports the gamerule applies across dimensions. Verify the gamerule lookup is per-world/dimension.

**Done:** Dying in the Nether with keepInventory off drops items in the Nether; with it on, items are kept only where the rule applies.

### G14 — Player collision and push `P1`

**Scope:** Player-vs-mob push, stale motion, vehicle push, `#4121`, `#3464`.

**Done:** Pushing a mob/player produces the vanilla velocity and does not launch the player upward.

**Issues/PRs:** `#3476`, `#3793`.

---

## 11. World generation (W8)

### H01 — Seed parity harness `P0`

**Scope:** Build a harness that generates a fixed set of chunks for a given seed and compares block states, biomes, heightmaps, structures, and entities with a vanilla 26.3 reference. Store golden hashes per region, not whole worlds, so the repository stays small.

**Done:** `cargo nextest` can run the harness against a set of reference seeds. Every worldgen PR runs the harness for the affected structure/feature. Deviations are listed in `T14`.

**Evidence:** `[code]` `pumpkin-world/src/generation/proto_chunk_test` exists but is not a seed-parity gate.

### H02 — Overworld terrain parity `P0`

**Scope:** Noise router, density functions, surface rules, carvers, aquifer, ore/stone distribution, biome placement. Fix caves in oceans, jungle terrain, badlands biome borders, underfilled trees, and incorrect ore/stone. `#2092`, `#2362`, `#2151`, `#1949`, `#2027`.

**Done:** The H01 harness passes for overworld chunks at the reference seeds; each named issue has a before/after chunk diff.

### H03 — Nether generation `P1`

**Scope:** Nether noise, lava sea, fortress/bastion placement, nether trees not generating through solid terrain, `#2466`, `#4079`.

**Done:** Nether chunks match the reference; trees do not replace solid terrain.

### H04 — End generation `P2`

**Scope:** End islands, chorus plants, end city, gateway, podium, spike, dragon fight geometry.

**Done:** End chunks and the dragon fight match the reference.

### H05 — Feature parity `P0`

**Scope:** Trees across chunk borders, birch spacing, old-growth spruce layers, bottom logs, geodes, dripstone caves, deep dark/sculk, coral reefs, glow lichen, seagrass on water ravines, snow placement, leaf sapling drops, `#4070`, `#4076`, `#4077`, `#4035`, `#3126`, `#3818`, `#2781`, `#3724`, `#1300`, `#2885`, `#3040`, `#3404`, `#3250`.

**Done:** The H01 harness covers every feature type; each named issue has a seed where it reproduces and a seed where it passes.

**Issues/PRs:** `#3041`, `#3438`.

### H06 — Structures `P0`

**Scope:** Per-dimension structure state and `/locate`, jigsaw placement, village paths, mineshaft fences, trail ruins, swamp huts, stronghold ring sampling, woodland mansion burial, ruined portals near bedrock, shipwreck stairs, floating snow in villages, `#3926`, `#2501`, `#3712`, `#4078`, `#4034`, `#4033`, `#4027`, `#3124`, `#3098`, `#3100`, `#3005`, `#2879`, `#2877`, `#3879`.

**Done:** `/locate` and generation agree for every structure; a structure-state test covers two dimensions in one world; each named issue is fixed or documented. `#4029`.

**Issues/PRs:** `#4029`, `#3428`, `#3901`, `#3903`.

### H07 — Structure NBT, entities, loot, processors `P0`

**Scope:** Parse and place structure template entities, apply processors, waterlog, schedule fluid ticks, place loot chests, preserve block entities, upgrade data versions, `#2877`, `#3926`, `#3902`.

**Done:** A village, ancient city, trial chamber, and abandoned camp place with their entities, chests, and block entities. `#3919`.

**Issues/PRs:** `#3919`.

### H08 — World types `P1`

**Scope:** Normal, flat, large biomes, amplified, single biome, debug all-block-states. Currently only flat, noise, and end generators exist. `#3778`.

**Done:** Creating a world of each type produces the vanilla terrain and config; flat worlds support the vanilla preset format.

**Issues/PRs:** `#3778`.

### H09 — 26.3 Wilderness Bound `P1`

**Scope:** Verify the fork's 26.3 content against `#3519`: dappled forest biome/climate/noise, poplar and fallen poplar trees, leaf distributions, shelf mushrooms, red shrub patches, abandoned camp variants/structures/loot/explorer maps, poplar wood set, wool/concrete stairs and slabs, cushions, straw beds. Most of this exists; the task is to audit behavior, recipes, tags, loot, and spawn lists, then close or narrow `#3519`.

**Done:** A fresh 26.3 world generates the biome and camps; every listed block/item has a behavior and recipe test; no 26.3 content is only a registry id.

**Issues/PRs:** `#3519`, `#3729`, `#3871`, `#3741`.

### H10 — Biome spawn lists `P1`

**Scope:** Correct passive/neutral/hostile spawn lists, including swamp/mangrove slimes and village animals. `#3618`, `#4124`.

**Done:** A spawn test in each biome produces the vanilla category distribution.

**Issues/PRs:** `#3861`.

### H11 — Cross-chunk feature ordering `P1`

**Scope:** Feature order, chunk border writes, post-processing, and structure starts crossing chunk boundaries. `#4070`, `#3126`.

**Done:** No tree, structure, or feature is truncated at a chunk border in the reference seeds.

### H12 — Datapack worldgen `P2`

**Scope:** Load dimension types, biomes, noise settings, configured/placed features, structures, template pools, processor lists, and structure sets from datapacks. `#731`.

**Done:** A datapack that adds a biome, feature, and structure works after `/reload` or restart.

**Issues/PRs:** `#731`, `#3903`.

### H13 — Locate and structure finder `P1`

**Scope:** `/locate structure`, `/locate biome`, `/locate poi`, per-dimension caches, `#3926`.

**Done:** `/locate` returns the same coordinates as vanilla for the reference seeds; it does not sample the triggering chunk's biomes.

**Issues/PRs:** `#4029`.

---

## 12. Persistence and chunk lifecycle (W9)

### I01 — World load/save 26.x `P0`

**Scope:** `level.dat`, `DIM1`, `DIM-1`, chunk NBT, block entities, entities, structures, post-processing, heightmaps, lighting, biomes. `#2626`, `#3902`, `#1554`.

**Done:** A vanilla 26.3 world loads, saves, and reloads without losing data. A Pumpkin world loads in vanilla. `#3634`.

**Issues/PRs:** `#3634`.

### I02 — Atomic saves `P0`

**Scope:** Write to a temporary file, fsync, rename; never truncate the last good file before the new one succeeds. Player data and level data. `#3511`, `#3512`.

**Done:** `kill -9` during a save at 100 random points leaves a loadable file. A test injects write failures.

**Issues/PRs:** `#3634`.

### I03 — Region writer correctness `P0`

**Scope:** In-place region writes must not overwrite the next chunk, sector allocation, compression, header timestamps, `#4031`.

**Done:** A region round-trip test writes/reads 1,024 chunks with random deletions and never corrupts a neighbor.

**Issues/PRs:** `#4031`.

### I04 — Chunk formats and converter `P1`

**Scope:** Anvil, Linear, Pump; `--convert-world` for all three; preserve unknown tags; `#118`, `#1554`.

**Done:** Convert a vanilla world in each direction and compare chunk content; unknown data is preserved or reported.

**Issues/PRs:** `#118`, `#3707`.

### I05 — Tickets, forceload, spawn chunks `P0`

**Scope:** Forced chunks get a real chunk ticket; `/forceload` works; spawn chunks stay loaded; ticket levels and removal; `#3867`, `#2592`.

**Evidence:** `[code]` `World::forced_chunks` only feeds `ActiveChunkTracker`; `ChunkLoading::add_force_ticket` is called only from the GameTest manager. `Level::fetch_chunk` adds a FULL ticket and drops it immediately.

**Done:** `/forceload add` keeps a chunk loaded with no player nearby; spawn chunks tick; GameTest forced chunks survive the test.

**Issues/PRs:** `#3867`, `#2592`.

### I06 — Chunk lifecycle and memory `P0`

**Scope:** `clean_memory` must not wipe forced chunks or entities; entity saver must watch forced chunks; unloading saves entities/block entities; `#1555`, `#3385`, `#4048`.

**Done:** A forced chunk with a chest and a mob survives 10 minutes with no player; memory returns to baseline after unload.

**Issues/PRs:** `#3848`.

### I07 — Chunk delivery `P1`

**Scope:** Send chunks to a moving client at more than 1 chunk/tick; prioritize visible chunks; cancel sends on teleport; `#3116`.

**Done:** A flying player receives the view distance without stalling; chunk send rate scales with bandwidth.

### I08 — Paper/Purpur world import `P1`

**Scope:** Preserve `game_rules`, weather, wandering-trader state, and Paper world settings on import; `#3367`, `#3447`.

**Done:** A Paper world imports with its gamerules and weather intact.

**Issues/PRs:** `#3447`.

### I09 — `save-all` and `stop` semantics `P0`

**Scope:** `stop` saves everything, including chunks away from spawn and block entities; `save-all` flushes; `save-off`/`save-on`; `#3902`.

**Done:** Place a chest 10,000 blocks from spawn, fill it, `/stop`, restart, and the contents are there without `/save-all`.

**Issues/PRs:** `#3902`.

### I10 — World border, time, weather persistence `P1`

**Scope:** Border center/size/damage, day time, weather cycle, `#3448`.

**Done:** Border and time survive restart; border clamp uses the center block. `#3475`.

**Issues/PRs:** `#3475`.

### I11 — Entity persistence `P0`

**Scope:** Entities survive chunk unload/reload, dimension change, and server restart; duplicate UUIDs are prevented; `#3385`, `#3272`, `#3734`.

**Done:** A named mob in an unloaded chunk returns with the same UUID and data. `#3800`.

**Issues/PRs:** `#3800`.

### I12 — Player data `P0`

**Scope:** Covered by `G11`; this task owns the file format and migration. Inventory, ender chest, effects, advancements, stats, respawn, last death location.

**Done:** A player file from vanilla 26.3 loads; a Pumpkin file loads in vanilla.

### I13 — World conversion and upgrade `P1`

**Scope:** `--convert-world`, data version upgrades, legacy structures, failed-upgrade rollback, `#118`, `#1554`.

**Done:** A 1.20 world upgrades to 26.3 without losing block entities; a failed upgrade leaves the source untouched.

**Issues/PRs:** `#118`, `#1554`.

---

## 13. World simulation (W10)

### J01 — Lighting `P0`

**Scope:** Sky/block light propagation, shape occlusion, sky cut height, light updates on block change, chunk relight, `#2427`.

**Done:** A lighting test compares light values with vanilla for 50 structures; a client sees no dark/black chunks.

**Issues/PRs:** `#3238`, `#4106`.

### J02 — Fluid physics `P0`

**Scope:** Water/lava spread, waterlogged blocks, bubble columns, magma bubbles, seagrass, fluid interaction, `#877`, `#2599`, `#3239`, `#3099`, `#4022`.

**Done:** A fluid test compares spread distance, level, and source creation with vanilla for 20 setups.

**Issues/PRs:** `#4022`.

### J03 — Scheduled and random ticks `P0`

**Scope:** Only tick chunks in `active_chunks`; fluid ticks; block ticks longer than 255; tick priorities; `#3551`.

**Done:** A redstone/fluid circuit outside the active area does not tick; a scheduled tick at 300 ticks fires at 300.

**Issues/PRs:** `#3553`, `#3639`, `#4010`.

### J04 — Neighbor updates and support `P1`

**Scope:** Block update ordering, support removal, snow/ice/farmland/plant updates, `#3855`.

**Done:** Breaking a support block removes the dependent block with the vanilla update order.

**Issues/PRs:** `#3415`, `#3868`.

### J05 — Explosions `P0`

**Scope:** Covered by `F08` for combat; this task owns block destruction, ray casting, water absorption, item drops, and explosion resistance.

**Done:** An explosion destroys the vanilla block set and drops the vanilla items.

### J06 — Portals and dimensions `P1`

**Scope:** Nether/End portal creation, travel, cooldowns, portal search/creation, exit portal, `#2065`, `#3389`.

**Done:** A player enters a portal, arrives at the correct coordinates, and returns; the dragon exit portal opens.

**Issues/PRs:** `#2065`.

### J07 — Weather and lightning `P2`

**Scope:** Rain/thunder cycles, lightning, fire, entity damage, `#1220`.

**Done:** Weather transitions and lightning match vanilla.

### J08 — Spawn and void `P1`

**Scope:** Safe spawn search, spawn chunks, void damage, `/tp` to ungenerated chunks, `#3836`, `#3075`.

**Done:** A player teleported to an ungenerated chunk lands on generated ground or takes void damage only below the world.

**Issues/PRs:** `#3075`.

### J09 — Fire spread `P1`

**Scope:** Covered by `D14`; this task owns the world-level fire tick and burnout.

### J10 — Fluid interactions `P1`

**Scope:** Lava/water stone/cobblestone/obsidian, lava flow in the Nether, water in the End, `#3239`.

**Done:** The interaction matrix matches vanilla.

### J11 — World border `P1`

**Scope:** Covered by `I10`; this task owns damage, clamp, and visual border packets.

### J12 — Scoreboard, teams, and boss bars `P1`

**Scope:** Verify and complete objectives/criteria, display slots, scores, teams (colors, friendly fire, collision, visibility, push rules), `/scoreboard`, `/team`, boss bars (color, style, flags, player visibility), and the authoritative team source for AI (`A05`). `world/scoreboard.rs` is ~1,269 lines and `world/bossbar.rs` exists; the task is parity, not a rewrite.

**Done:** A scoreboard/team/bossbar script matches vanilla output and client rendering; team friendly fire is enforced in combat (`#4080`); `#3455` has one source of truth.

**Issues/PRs:** `#3455`, `#4080`.

### J13 — Block entity ticking `P1`

**Scope:** Verify the tick rate and state machine of every ticking block entity: furnace family, hopper, brewing stand, mob spawner, crafter, trial spawner, vault, bell, beehive, sculk sensor/shrieker/catalyst, command block, structure block, jigsaw, copper golem statue, creaking heart, and campfire. Many are implemented; the task is a rate/ordering audit.

**Done:** A tick harness compares progress, cooldowns, and state transitions with vanilla for each block entity.

---

## 14. Java protocol (W11)

### K01 — Packet codec robustness `P0`

**Scope:** Bounds-check every length/count, cap nesting, cap decompressed size, reject invalid states, do not allocate the declared length up front, `#3716`, `#3782`, `#3777`, `#3319`, `#3065`, `#3911`, `#3992`, `#3993`, `#3999`, `#4000`, `#4003`, `#4055`, `#3706`.

**Done:** The fuzz corpus runs for 24 hours without a panic, OOM, or hang. Every fixed crash has a corpus entry.

**Issues/PRs:** `#3911`, `#3992`, `#3993`, `#3999`, `#4000`, `#4003`, `#4055`, `#3706`.

### K02 — Packet events and connection state `P0`

**Scope:** Decode packets only in the correct state; fire `PacketReceivedEvent`/`PacketSentEvent` for every state; do not let a plugin handler corrupt the stream; `#4006`, `#3716`.

**Done:** A plugin can observe and cancel a packet in handshake, status, login, config, and play without a panic. `#3354`.

**Issues/PRs:** `#3354`, `#4006`.

### K03 — Join, tracking, and chunk sending `P1`

**Scope:** Join sequence, player info before entity tracking, chunk cache, packet spam kick, duplicate login, `#3802`, `#2923`, `#3116`.

**Done:** A bot joins a fresh server 100 times without a kick; two clients can join simultaneously. `#3708`, `#4060`.

**Issues/PRs:** `#3708`, `#4060`.

### K04 — Entity metadata, equipment, animation `P1`

**Scope:** Metadata encoding, equipment packets, entity events, animation, `#2517`, `#4036`, `#4123`, `#2991`.

**Done:** A metadata diff test covers every entity type; a real client sees equipment and animations.

### K05 — Movement, teleport, vehicle packets `P1`

**Scope:** Player movement, vehicle move, teleport confirm, invalid positions, `#3389`, `#3464`, `#3996`.

**Done:** A movement trace matches vanilla; invalid positions are rejected.

### K06 — Container and recipe packets `P1`

**Scope:** Container set content/slot, recipe book, merchant, stonecutter, smithing, `#3782`, `#3917`, `#3922`.

**Done:** A container action round-trips without a decode error.

### K07 — Chat and tab list `P2`

**Scope:** Chat, signed messages, tab list, `#3379`.

**Done:** Chat and tab list render correctly for a real client.

### K08 — Compression and encryption `P1`

**Scope:** zlib-rs, encrypted write buffering, compression threshold, `#2482`, `#2489`, `#1781`.

**Done:** A mineflayer bot connects; compression round-trips at every threshold.

**Issues/PRs:** `#2482`, `#2489`.

### K09 — Status, query, LAN `P2`

**Scope:** Server list ping, query, LAN broadcast, `#3913`.

**Done:** The server appears in the client list with the correct player count; query returns the vanilla fields.

**Issues/PRs:** `#3913`.

### K10 — Multi-version policy `P1`

**Scope:** Document that 26.3 is the only Java version. If `#1781`/`#3819` propose multi-version, decide and record it. Do not add protocol translation without a design.

**Done:** `README.md` and the protocol crate say one version; an older client gets a clear disconnect.

**Issues/PRs:** `#1781`, `#3819`.

### K11 — Protocol fuzzing `P0`

**Scope:** A corpus-driven fuzzer for every serverbound packet, run in CI for pull requests that touch `pumpkin-protocol`.

**Done:** `cargo fuzz` targets exist; CI runs a smoke corpus; a nightly job runs longer.

### K12 — Chat signing and reporting `P2`

**Scope:** Verify `allow_chat_reports`, secure-profile enforcement, message signatures, `last_seen` acknowledgement, and the vanilla disconnect/error behavior. The code has a partial implementation in `net/java/play/chat_message.rs` and `chat_ack.rs`; do not assume it is complete.

**Done:** A signed chat message is accepted, an unsigned message is rejected when reports are enabled, and the acknowledgement window is bounded. If full reporting is out of scope, list it in `T14`.

---

## 15. Bedrock (W12)

### L01 — NetherNet and TLS `P0`

**Scope:** Fix the TLS ClientHello/plaintext HTTP 400 failure, session cleanup, max packet size, `#3738`, `#4001`, `#3245`.

**Done:** A Bedrock 1.26.51 client joins over NetherNet on Linux and Windows. `#3738`.

**Issues/PRs:** `#3245`, `#4001`.

### L02 — Multiple Bedrock versions `P1`

**Scope:** Decide whether 1.0 supports one Bedrock version or a range. If a range, add version negotiation and per-version packet mappings; `#3735`.

**Done:** The supported version(s) are documented; a client on an unsupported version gets a clear message.

**Issues/PRs:** `#3735`.

### L03 — Login, auth, skins, resource packs `P0`

**Scope:** Xbox auth, bans/whitelist on Bedrock login, skin download not blocking join, resource pack negotiation, pack response, `#3487`, `#2370`, `#3820`, `#4061`, `#4067`, `#3841`.

**Done:** A banned player is rejected; a whitelisted player joins; a resource pack applies. `#4061`, `#4067`.

**Issues/PRs:** `#3820`, `#4061`, `#4067`, `#3841`.

### L04 — Movement, crawling, swimming `P1`

**Scope:** Crawling in one-block gaps, swimming state, fall damage near blocks, `#3831`, `#2104`, `#3064`.

**Done:** A Bedrock client crawls, swims, and takes fall damage like vanilla. `#3831`.

### L05 — Entity spawn and chunk generation `P0`

**Scope:** Bedrock entity spawn on Gen-Pool threads without a Tokio runtime, chunk generation panic, `#2291`, `#3396`.

**Done:** A Bedrock client can explore new chunks for 30 minutes without a panic.

**Issues/PRs:** `#2291`.

### L06 — Inventory and item stacks `P1`

**Scope:** Hotbar slot bounds, item stack requests, inventory sync, `#3396`, `#4056`.

**Done:** A Bedrock client can move items without a kick or desync.

**Issues/PRs:** `#4056`.

### L07 — Chunk packets and palettes `P1`

**Scope:** Sub-chunk palette, all-air sub-chunks, blob payload lifetime, `#3427`, `#3467`, `#3244`.

**Done:** Bedrock chunks render correctly and memory does not grow per chunk sent.

**Issues/PRs:** `#3427`, `#3467`, `#3244`.

### L08 — Proxy support `P1`

**Scope:** Bungee/Velocity/Vine forwarding for Bedrock, proxy header parsing, `#3340`.

**Done:** A Bedrock client joins through Velocity with the correct UUID and address.

### L09 — Kick and stop `P2`

**Scope:** Kick players on `/stop`, disconnect reason, `#2981`.

**Done:** `/stop` disconnects Bedrock clients cleanly.

### L10 — Advancements `P2`

**Scope:** Bedrock advancement display/unparsed form, `#2901`.

**Done:** Bedrock clients see translated advancement names and descriptions.

### L11 — Experimental minecart improvements `P2`

**Scope:** `#4021`; decide whether to implement the Bedrock experiment. Default off.

**Done:** Documented as out of scope or behind a config flag.

### L12 — Bedrock test harness `P1`

**Scope:** A scriptable Bedrock client or a recorded packet replay for login, movement, chunk load, inventory, and disconnect.

**Done:** CI runs the harness against a release candidate.

---

## 16. Plugin API (W13)

### M01 — ABI finalization `P0`

**Scope:** Covered by `R02`/`R03`; this task owns the API-side design: event trait objects, host function signatures, error types, thread rules, and the `Send + Sync` contract.

**Done:** The ABI document is published; a fixture plugin compiles against it.

### M02 — WIT surface completeness `P0`

**Scope:** Implement the missing WIT APIs from `#3096`: locator/waypoint bar, recipe types (`COMPLEX`, `MERCHANT`, `POTION`, `SMITHING`, `STONECUTTING`), `GAME_EVENT`, `TRIM_MATERIAL`, `TRIM_PATTERN`, `DAMAGE_TYPE`, `WOLF_VARIANT`, `JUKEBOX_SONG`, `BANNER_PATTERN`, `PAINTING_VARIANT`, `INSTRUMENT`, `CAT_VARIANT`, `FROG_VARIANT`, `CHICKEN_VARIANT`, `COW_VARIANT`, `PIG_VARIANT`, `ZOMBIE_NAUTILUS_VARIANT`, `SULFUR_CUBE_ARCHETYPE`, `ADVANCEMENT_BUILDER`.

**Done:** Every checklist item has a WIT type, host implementation, and a fixture call. `#3484` is closed.

**Issues/PRs:** `#3096`, `#3484`, `#3908`, `#3927`.

### M03 — Event dispatcher completeness `P0`

**Scope:** Every event fires in every connection state; packet events; banned-player join; team membership; block/entity/inventory events; unload/reload. `#4006`, `#3716`, `#2871`, `#3455`.

**Done:** An event coverage matrix lists every event, its state, its cancellation semantics, and a test. `#3354`.

**Issues/PRs:** `#4006`, `#3354`, `#3716`, `#2871`.

### M04 — Runtime fixes `P0`

**Scope:** Native futures polled on `GLOBAL_RUNTIME`, Wasm re-entry/concurrency, TypeScript command registration, plugin-utils init, unload cleanup, HTTP output, `#3593`, `#3702`, `#3701`, `#2056`, `#3089`, `#3086`.

**Done:** The plugin fixture suite runs native and Wasm plugins concurrently for 1,000 calls without a panic or deadlock.

**Issues/PRs:** `#3593`, `#3702`, `#3701`, `#2056`, `#3089`, `#3086`, `#4098`.

### M05 — Command sender and handler stubs `P0`

**Scope:** `get-command-sender-type` and `require-with-handler-id` must not trap the plugin instance; `#3163`.

**Done:** A plugin command can inspect its sender and require a handler; a failure returns an error, not a trap.

**Issues/PRs:** `#3163`, `#3508`.

### M06 — Persistent Data Container `P1`

**Scope:** Implement a persistent, namespaced key/value store on entities, items, and chunks, `#635`.

**Done:** A plugin writes data, restarts the server, and reads it back.

**Issues/PRs:** `#635`.

### M07 — World-edit, chunk, entity, NBT APIs `P1`

**Scope:** Read/write sections in bulk, set/resend biomes, load chunks on demand, read/write/spawn entities from NBT, world edit follow-ups, `#2810`, `#3554`, `#3719`, `#3720`, `#3721`.

**Done:** A WorldEdit-style fixture plugin edits a region, sets biomes, and spawns an NBT entity.

**Issues/PRs:** `#2810`, `#3554`, `#3719`, `#3720`, `#3721`.

### M08 — Registry and recipe APIs `P1`

**Scope:** Dynamic registries, registry keys for every type, recipe registration, block/item lookup, `#2098`, `#2211`, `#3908`.

**Done:** A plugin registers a block, item, and recipe at load time and they work after restart.

**Issues/PRs:** `#2098`, `#2211`, `#3908`, `#3887`.

### M09 — Enchanting events `P2`

**Scope:** Mutable enchanting-table events, `#2632`.

**Done:** A plugin changes the offered enchantments and the client sees the change.

### M10 — GUI, dashboard, metrics `P2`

**Scope:** GUI dashboard, OpenTelemetry metrics, `/perf` integration, `#3246`, `#3921`, `#4072`.

**Done:** A plugin or operator can read server metrics; the dashboard is optional.

**Issues/PRs:** `#3921`, `#4072`, `#2958`.

### M11 — API docs and SDKs `P1`

**Scope:** Rust API docs, WIT docs, examples, and at least one external-language SDK build. `#3096`.

**Done:** A new plugin author can build a working plugin from the docs in under 30 minutes.

### M12 — ABI compatibility tests `P0`

**Scope:** Build fixture plugins against the 1.0 ABI, run them on every 1.0.x commit, and fail if a symbol or type changes. `R02`/`R03`.

**Done:** CI has a compatibility job; a breaking change requires an explicit version bump.

### M13 — Plugin permissions, config, reload, and distribution `P1`

**Scope:** Plugin permissions, Bukkit-style config file helpers, `/plugin` list/info/reload/tab completion, plugin dependency failures that do not take down healthy plugins, and a documented distribution/signing path. `#3355`, `#4083`, `#3979`, `#3914`.

**Done:** A plugin declares permissions, reads/writes a config file, is listed/reloaded, and a broken dependency does not disable unrelated plugins.

**Issues/PRs:** `#3355`, `#4083`, `#3979`.

---

## 17. Commands and data packs (W14)

### N01 — Missing commands `P1`

**Scope:** `/jfr` and `/perf`. `/perf` has PR `#2958`; `/jfr` has no implementation. Decide whether `/jfr` is 1.0 (JFR is a profiling tool, not gameplay). `/publish` and `/unpublish` are integrated-server-only commands and are excluded from the dedicated-server target.

**Done:** Each command exists, is registered, has a permission level, and matches vanilla output. `#2958`.

**Issues/PRs:** `#2958`.

### N02 — Command correctness `P0`

**Scope:** `/execute` (store, if block, predicates), `/setblock` (block states, NBT), `/function` (recursion, scheduling), `/tick` (sprint message), `/effect`, `/gamemode`, `/give` (100 stack cap), `/locate`, `/forceload`, `/data`, `/save-all`, `/worldborder`, `/spreadplayers`, `/random`, `/return`, `/waypoint`, `/dialog`, `/ride`, `/rotate`, `/damage`, `/loot`, `/recipe`, `/advancement`, `/bossbar`, `/team`, `/scoreboard`, `/tag`, `/schedule`.

**Done:** A command conformance suite compares output, side effects, and error messages with vanilla for every command. `#3758`, `#3779`, `#3759`, `#3760`, `#1985`, `#3538`, `#2592`.

**Issues/PRs:** `#3987`, `#4111`, `#3813`, `#3827`, `#3763`, `#4066`.

### N03 — Permissions and selectors `P0`

**Scope:** Permission levels, selector parsing, entity selector limits, `/execute as/at`, command blocks, `#4069`, `#3513`.

**Done:** A non-op cannot run an op command; a selector with a large radius is bounded; `@e` works in every command that accepts it. `#4069`.

**Issues/PRs:** `#4069`.

### N04 — Datapack loaders `P0`

**Scope:** Load every vanilla data pack registry: dimension type, biome, noise settings, configured/placed feature, structure, structure set, template pool, processor list, enchantment, damage type, trim material, trim pattern, banner pattern, painting variant, instrument, jukebox song, wolf/cat/frog/chicken/cow/pig/zombie-nautilus variant, sulfur cube archetype, trade, loot modifier, predicate, item modifier, function, tag, test instance. Today `data/datapack/` has only context provider, damage type, dynamic registry, function, loot table, recipe, structure, test, and trade loaders.

**Done:** A data pack that adds one entry of each type loads after `/reload`; `/datapack list` shows it; the registry is synced to clients. `#731`.

**Issues/PRs:** `#731`, `#3903`.

### N05 — Reload and function scheduling `P1`

**Scope:** `/reload` swaps data without ticking frozen; load functions run on the next tick; function recursion is bounded; `#3276`, `#3759`, `#3392`.

**Done:** A reload with a load function does not deadlock; a recursive function stops at the vanilla limit. `#3813`.

**Issues/PRs:** `#3394`, `#3813`, `#3433`.

### N06 — Tags and registry keys `P1`

**Scope:** Parse all tag types, sync tags to clients, resolve registry keys, `/tag` suggestions, `#3906`.

**Done:** A tag added by a data pack affects recipes, loot, and mob behavior.

**Issues/PRs:** `#3906`.

### N07 — Command translations `P2`

**Scope:** Translate command descriptions and argument errors, `#960`, `#3018`, `#3004`.

**Done:** A client with a non-English locale sees translated command descriptions.

**Issues/PRs:** `#960`, `#3018`, `#3004`, `#3854`.

### N08 — Console command handling `P2`

**Scope:** Empty console command is ignored, stdin task stops on shutdown, `#4075`, `#3788`.

**Done:** Pressing Enter on an empty console line does nothing; `/stop` exits cleanly.

**Issues/PRs:** `#4111`, `#3788`. Local branch `fix/console-ignore-empty-command` and PR `#4111` already implement the empty-command fix; verify and land it instead of reimplementing.

### N09 — Command blocks `P1`

**Scope:** Command block state, editing, activation, conditional/chain, output, minecart command block.

**Done:** A command block runs on redstone and a chain executes in order; a command-block minecart runs its command.

### N10 — Recipe and loot JSON `P0`

**Scope:** Covered by `E13`-`E16`; this task owns the data-pack parsing side and error reporting.

**Done:** A malformed recipe/loot table produces a clear error and does not abort the whole pack.

---

## 18. Operations, configuration, security, packaging (W15)

### O01 — Config validation `P0`

**Scope:** Invalid config must fail loudly with a field path, not silently start with defaults. Validate ports, addresses, view distance, compression, worlds, plugins. `#3431`, `#2985`, `#3420`, `#3373`, `#2504`, `#3594`.

**Done:** A config with a bad type/port/range refuses to start and prints the exact field. A valid config round-trips.

**Issues/PRs:** `#3420`, `#3373`, `#3594`, `#2504`.

### O02 — Server property parity `P1`

**Scope:** Audit every vanilla `server.properties` feature against `pumpkin.toml`: max players, view/simulation distance, spawn protection, PvP, difficulty/hardcore, allow-flight, allow-nether, spawn-monsters/npcs, generate-structures, max-world-size, network compression, idle timeout, rate limit, entity broadcast range, function/op permission level, command blocks, status, hide online players, force gamemode, proxy, secure profile, RCON, query, resource pack, max tick time, sync chunk writes, region compression, pause when empty, initial packs, bug report link.

**Done:** A table maps every vanilla property to a Pumpkin field or a documented non-goal. Missing features are implemented or listed in `T14`.

### O03 — RCON `P1`

**Scope:** Implement the vanilla RCON protocol: config (`enable-rcon`, `rcon.port`, `rcon.password`), auth, command execution, multi-packet responses, and logging. No RCON implementation exists in the fork today.

**Done:** An RCON client authenticates, runs `/list`, `/say`, and `/stop`, and receives the correct response. A wrong password is rejected. The port is not exposed unless enabled.

**Evidence:** `[code]` no `rcon` module or config field.

### O04 — Query, LAN, proxy `P1`

**Scope:** Query protocol, LAN broadcast, Bungee/Velocity/Vine forwarding, HAProxy PROXY v2, constant-time secret comparison, `#3913`, `#3257`, `#3832`, `#3915`, `#1110`, `#3340`.

**Done:** Query returns the vanilla fields; a proxy-forwarded player gets the correct UUID/address; a wrong secret is rejected in constant time.

**Issues/PRs:** `#3257`, `#3832`, `#3913`, `#3915`.

### O05 — Security hardening `P0`

**Scope:**
- path traversal and symlinks in world/data pack/plugin loading (`#3392`, `#3433`);
- bounded decompression and packet lengths (`#3911`, `#3992`, `#3993`);
- bounded SNBT/component nesting (`#3999`, `#4000`, `#4003`, `#4055`);
- auth URL escaping (`#3991`);
- plugin signature verification (`#3914`);
- proxy secret constant-time compare and `proxy.enabled` gate (`#3915`);
- duplicate login/max players atomicity (`#4060`);
- selector permission (`#4069`);
- Bedrock ban/whitelist (`#4061`);
- hotbar slot bounds (`#4056`);
- Rayon panic does not abort (`#3995`).

**Done:** A security checklist is signed off; each item has a test or a documented mitigation.

**Issues/PRs:** `#3392`, `#3433`, `#3911`, `#3914`, `#3915`, `#3991`, `#3992`, `#3993`, `#3995`, `#3996`, `#3999`, `#4000`, `#4001`, `#4003`, `#4055`, `#4056`, `#4060`, `#4061`, `#4069`.

### O06 — Docker and packaging `P1`

**Scope:** Docker tags, non-root user, `/data` volume, Pterodactyl egg, multi-arch, image size, SIGINT handling, `#4052`, `#1809`, `#3410`, `#3664`, `#3485`, `#2057`.

**Done:** `docker run` starts a server, persists a world, and stops cleanly on SIGTERM. Tags are documented.

**Issues/PRs:** `#4052`, `#3410`, `#3664`, `#3485`, `#1809`.

### O07 — Platform builds `P0`

**Scope:** Covered by `R04`; this task owns the build scripts and smoke tests for FreeBSD, macOS x86_64, Android armv7/x86_64, and Docker armv7.

**Done:** Each artifact is published and smoke-tested.

**Issues/PRs:** `#3628`, `#449`.

### O08 — Metrics and profiler `P2`

**Scope:** OpenTelemetry metrics, `/perf`, memory/CPU profiler, tick timing, `#3921`, `#4072`, `#2958`.

**Done:** An operator can scrape TPS, MSPT, player count, chunk count, and memory; `/perf` produces a report.

**Issues/PRs:** `#3921`, `#4072`, `#2958`.

### O09 — Logging and console `P2`

**Scope:** Structured logs, interpolation, console colors, contributor hover, `#3004`, `#3018`, `#2577`.

**Done:** Logs contain no broken placeholders; console output is readable and localized where supported.

**Issues/PRs:** `#3854`, `#2577`.

### O10 — Backups and save management `P1`

**Scope:** Document and test backups; `save-all`/`save-off`; world conversion; no partial region writes. `I02`/`I03`.

**Done:** A backup taken during play restores to a consistent world.

### O11 — Crash reporting `P1`

**Scope:** Crash report contents, backtrace, config/world summary, plugin list, upload/attachment instructions.

**Done:** A forced panic writes a report that identifies the panic site and the last tick.

### O12 — Telemetry and consent `P2`

**Scope:** Decide whether Pumpkin sends telemetry. Default off. If added, document it and make it opt-in. `#3921` is metrics, not telemetry.

**Done:** No network telemetry is sent by default; the config field exists only if implemented.

### O13 — CLI, startup, and player file formats `P1`

**Scope:** Vanilla-compatible CLI/startup behavior: `--help`, `--version`, `--port`, `--serverIp`, `--universe`, `--world`, `--nogui`, `--demo`, `--bonusChest`, `--forceUpgrade`, `--eraseCache`, `--safeMode`; `eula.txt`; `ops.json`, `whitelist.json`, `banned-players.json`, `banned-ips.json`, `usercache.json` formats; `--initSettings`. The current binary only handles the world-conversion argument.

**Done:** A script can start the server with the documented flags; an existing vanilla player file set is read without losing entries. Documented non-goals are in `T14`.

### O14 — Management protocol (MSMP) `P2`

**Scope:** Audit the management server (`net/management/`, ~2,432 lines): auth, RPC methods, DTOs, subscriptions, server-save notifications, player join/leave events, error handling, and docs. This is Pumpkin-specific, not vanilla parity, but it is part of the 1.0 server surface.

**Done:** An external client can authenticate, query players, subscribe to events, and trigger a save; the protocol is documented and has an integration test.

---

## 19. Data parity (W16)

### P01 — Advancement triggers `P0`

**Scope:** Replace the 20 hardcoded `AdvancementTrigger` variants with the full vanilla criterion trigger set and a data-driven predicate evaluator. The generated advancement data has 1,868 constants; the trigger layer only handles a small subset.

**Evidence:** `[code]` `AdvancementTrigger` has 20 variants; most advancements are checked by hand inside `trigger_advancement` (e.g. `InventoryChanged` scans a fixed item list).

**Done:** Every vanilla advancement can be earned in a bot run; a trigger coverage test lists every criterion and its trigger. `#2901`, `#4117`.

**Issues/PRs:** `#4117`.

### P02 — Advancement UI and data `P1`

**Scope:** Advancement screen, toast, progress, hidden/secret, `/advancement`, per-player visibility, Bedrock display.

**Done:** A real client sees the advancement toast and screen; a Bedrock client sees translated text. `#2901`.

**Issues/PRs:** `#2901`.

### P03 — Statistics `P1`

**Scope:** Every statistic category and custom stat increments at the vanilla point; `/stats`; persistence; scoreboard integration.

**Done:** A scripted session increments used/broken/crafted/killed/custom stats correctly.

### P04 — Status effects `P1`

**Scope:** All 41 status effects: attribute modifiers, periodic ticks, particles, icons, stacking, ambience, instant effects, milk removal, death/teleport interactions. Only 10 effects have dedicated runtime modules; verify the rest are data-driven and correct.

**Done:** An effect matrix test applies each effect and compares the player's attributes/health/particles with vanilla. `#764`, `#765`, `#2520`.

**Issues/PRs:** `#2520`.

### P05 — Enchantment effects `P1`

**Scope:** Implement every enchantment effect component used by vanilla 26.3, including the value effects and the entity effects in `pumpkin-data/src/generated/enchantment.rs`. Verify each enchantment's behavior: Sharpness, Protection, Feather Falling, Respiration, Aqua Affinity, Thorns, Frost Walker, Soul Speed, Swift Sneak, Wind Burst, Density, Breach, Mending, Unbreaking, Silk Touch, Fortune, Looting, Luck of the Sea, Fire Aspect, Flame, Infinity, Loyalty, Riptide, Channeling, Impaling, Piercing, Multishot, Quick Charge, and curses.

**Done:** An enchantment matrix test applies each enchantment at levels I-III and compares damage/drops/durability with vanilla. `#3357`, `#3166`, `#3615`.

**Issues/PRs:** `#3357`, `#3166`.

### P06 — Attributes and modifiers `P1`

**Scope:** All 41 attributes, base values per entity, equipment modifiers, effect modifiers, modifier IDs, sync to clients, `/attribute`.

**Done:** An attribute diff test covers every living entity type. `#2883`.

**Issues/PRs:** `#2883`.

### P07 — Damage types and immunities `P2`

**Scope:** All damage types, tags, immunities, death messages, damage scaling.

**Done:** A damage-type matrix test produces the correct death message and immunity.

### P08 — Enchantment data `P1`

**Scope:** Enchantment definitions, max levels, rarity, tags, anvil costs, villager trades, loot.

**Done:** The enchanting table offers the vanilla enchantments for a given seed/level; anvil costs match.

### P09 — Trade data `P1`

**Scope:** Villager profession/level trades, wandering trader trades, restock, demand, reputation, hero discount. `trade_loader.rs` exists but the data and UI need verification. `#3761`, `#3083`.

**Done:** A trade matrix test covers every profession and level; prices/demand match vanilla.

**Issues/PRs:** `#3765`.

### P10 — Loot tables `P0`

**Scope:** Covered by `E15`/`E16`; this task owns the data audit: every vanilla loot table loads, every entry resolves, every condition is supported.

**Done:** A loader test walks every table in `assets/datapack` and reports zero unsupported functions/conditions.

### P11 — Recipes `P0`

**Scope:** Covered by `E13`/`E14`; this task owns the data audit: every vanilla recipe loads and produces the correct result.

**Done:** A recipe test crafts every vanilla recipe from its ingredients.

### P12 — Sounds and particles `P1`

**Scope:** Sound registry, subtitles, category/volume/pitch, particle types and data, `#3388`, `#3062`, `#3777`, `#765`.

**Done:** A real client hears/sees the vanilla sound/particle for a scripted set of 50 actions.

### P13 — Translations `P2`

**Scope:** Covered by `W19`/`S05`; this task owns the data side: all keys, fallback, locale selection, plugin custom translations.

### P14 — Entity variants `P1`

**Scope:** Cat, wolf, frog, cow, pig, chicken, zombie nautilus, sulfur cube archetype, painting, banner pattern, instrument, jukebox song, trim material/pattern. Registry IDs and network encoding. `#3096`.

**Done:** Every variant spawns/applies with the correct texture/data and is exposed to plugins.

### P15 — Game events `P1`

**Scope:** The full `GameEvent` registry with frequencies for sculk. Today the enum has the events but there is no frequency table or listener. `D07`.

**Done:** A game-event table maps every event to its vanilla frequency; a sculk sensor reacts to each.

### P16 — Registry sync `P1`

**Scope:** Sync dynamic registries to Java clients, encode registry IDs, handle unknown entries, `#3706`.

**Done:** A client sees the same registry contents as the server; a data pack registry addition syncs.

---

## 20. QA and testing (W17)

### Q01 — GameTest framework `P0`

**Scope:** Fix the blockers in `#3867`:
- forced chunks get tickets and survive `clean_memory`;
- setup has a timeout;
- entity saver watches forced chunks;
- `clear_non_player_entities` clears disk entities;
- async block/height queries keep a ticket;
- verify the no-player despawn path (current `check_despawn` already returns early, so this bullet may be stale);
- encase failure marks the controller failed;
- `/test stop` clears barriers and prints the summary;
- `stop_game_tests` reports dropped batches;
- no `block_on` on a tokio mutex;
- per-test messages reach the log without a player;
- public step/read API;
- `test_environment` is applied;
- structure entities/fluids are placed;
- `/test` subcommands (`pos`, `create`, `runclosest`, `runthat`, `runfailed`, `clearall`, `verify`, `export`);
- `minecraft:function` tests work;
- headless harness.

**Done:** GameTest runs headless in CI, all vanilla-style tests pass, and `#3867` is closed.

**Issues/PRs:** `#3867`, `#3919`, `#2552`.

### Q02 — Headless server harness `P0`

**Scope:** Boot a `Server` in-process on a temp void world, step it tick by tick, and run tests from `cargo nextest` without sockets.

**Done:** The harness runs in CI and is used by `Q01`, `Q03`, `Q06`, `Q07`.

### Q03 — Bot parity suite `P0`

**Scope:** Mineflayer or rust-mc-bot scenarios for join, movement, combat, inventory, containers, redstone, sleep, trading, and worldgen smoke. Store scripts outside the repo except for the small suite that earns its place.

**Done:** CI runs the suite against a release candidate; failures attach logs and a replay.

### Q04 — Worldgen golden tests `P0`

**Scope:** `H01` integrated into CI. Golden hashes for overworld/nether/end, structures, features.

**Done:** A worldgen PR cannot merge if a golden hash changes without an approved update.

### Q05 — Protocol fuzzing `P0`

**Scope:** `K11` integrated into CI. Corpus from every serverbound packet.

**Done:** No panic in 24 hours of replay; a new crash adds a corpus entry.

### Q06 — Save round-trip tests `P0`

**Scope:** Vanilla world -> Pumpkin -> vanilla; Pumpkin -> vanilla; kill -9 during save; all chunk formats.

**Done:** The matrix passes for chests, furnaces, signs, entities, player data, and structures.

### Q07 — Tick and redstone timing tests `P1`

**Scope:** Deterministic tick tests for repeaters, comparators, pistons, observers, rails, hoppers, fluids, and scheduled ticks.

**Done:** A timing trace matches vanilla for 30 circuits.

### Q08 — CI matrix `P1`

**Scope:** `cargo fmt`, clippy with all targets/features, nextest, machete, typos, Android, Windows, macOS, FreeBSD, Docker, docs. `#3260`, `#2858`.

**Done:** Every supported platform builds in CI; a red job blocks merge.

**Issues/PRs:** `#3260`, `#2858`.

### Q09 — Coverage `P2`

**Scope:** Line/branch coverage for `pumpkin-core`, `pumpkin-world`, `pumpkin-protocol`, `pumpkin-inventory`. Do not chase a number; track regressions.

**Done:** A coverage report is published per release.

### Q10 — Regression corpus `P1`

**Scope:** Every fixed crash/data-loss bug gets a minimal repro in the corpus.

**Done:** The corpus runs in CI and on release candidates.

### Q11 — Real-client checklist `P0`

**Scope:** Per-PR play-test instructions, capture requirement, and a release-candidate checklist covering rendering, particles, sounds, animations, GUIs, and client prediction.

**Done:** Every player-visible PR has a capture; the release candidate has a signed checklist.

### Q12 — Benchmarks `P1`

**Scope:** Tick benchmark, mob-heavy benchmark, chunk-gen benchmark, join/leave benchmark, memory benchmark. Same machine, same seed, same bot count.

**Done:** `PF11` has a baseline and a regression threshold.

### Q13 — Load and stress testing `P1`

**Scope:** 1,000 simulated players moving and interacting for 30 minutes; join/leave churn; chunk-generation load; mob-heavy load; explosion/redstone stress. Record TPS, MSPT, memory, and errors. This is the gate `#449` claims is already done; prove it on the current fork.

**Done:** The stress run meets the `PF11` baseline with no panic, deadlock, or unbounded memory growth.

### Q14 — Compatibility matrix `P1`

**Scope:** Test against a vanilla 26.3 Java client, a Bedrock 1.26.51 client, a vanilla 26.3 world, a vanilla data pack, a Paper/Purpur world, a proxy (Velocity/Bungee), and the fixture plugins.

**Done:** The matrix is in CI or the release checklist, with a pass/fail row for each combination.

---

## 21. Performance (W18)

### PF01 — Tokio sync audit `P1`

**Scope:** Replace indiscriminate `tokio::sync` locks with `std`/`parking_lot` where the critical section is synchronous and never held across `.await`; keep async locks where required. `#3858`.

**Done:** No sync guard is held across `.await`; a lock-order audit finds no inversion. Throughput improves or stays flat.

**Issues/PRs:** `#3858`.

### PF02 — Memory per join/leave `P0`

**Scope:** `#4048` reports ~3 MB growth per join/leave. Find and fix the retention (player, client, chunk, event, plugin). Add a soak test.

**Done:** 1,000 join/leave cycles return to baseline RSS within 5%.

**Issues/PRs:** `#4048`, `#3243`.

### PF03 — Join CPU spike `P1`

**Scope:** `#3264` reports a 300-500% CPU spike for 1.5 minutes on the first join. Profile chunk loading, lighting, entity tracking, and recipe/loot loading.

**Done:** First join is within 2x of a subsequent join; a profile is attached to the PR.

**Issues/PRs:** `#3264`.

### PF04 — Chunk delivery `P1`

**Scope:** `#3116`; increase chunk send rate, prioritize visible chunks, avoid regenerating, and cancel stale sends.

**Done:** A flying player receives chunks without stalling; the send rate is configurable.

**Issues/PRs:** `#3116`.

### PF05 — Pathfinding `P1`

**Scope:** Reuse paths, share block caches, limit searches, avoid per-mob heap preallocation. `#3955`, `#4053`.

**Done:** The mob-heavy benchmark improves; behavior is unchanged.

**Issues/PRs:** `#3955`, `#4053`, `#4086`.

### PF06 — Chunk memory `P1`

**Scope:** Palette packing, chunk cache, void chunks, unload. `#3848`, `#1555`.

**Done:** A 10,000-chunk world stays under the memory budget; void chunks do not leak.

**Issues/PRs:** `#3848`, `#1555`.

### PF07 — Lighting performance `P1`

**Scope:** Flood fill, occlusion, sky cut height, queue caching. `#3238`, `#4106`.

**Done:** Lighting a chunk is within the vanilla budget; no light leaks.

**Issues/PRs:** `#3238`, `#4106`.

### PF08 — Worldgen performance `P1`

**Scope:** Noise generation, buffer allocations, structure cache, biome distance pruning. `#2506`, `#3179`, `#3335`.

**Done:** Chunk generation meets the baseline; no behavior change.

**Issues/PRs:** `#2506`, `#3179`, `#3335`.

### PF09 — Protocol buffers `P2`

**Scope:** Encrypted write buffering, typed prepared packets, compression. `#2482`, `#2792`.

**Done:** Throughput improves; no packet corruption.

**Issues/PRs:** `#2482`, `#2792`.

### PF10 — Entity tracking and nearest searches `P2`

**Scope:** Stream nearest entity searches, avoid repeated scans, cache tracking ranges. `#3425`.

**Done:** Entity-heavy benchmark improves; tracking correctness unchanged.

**Issues/PRs:** `#3425`.

### PF11 — Benchmark gate `P0`

**Scope:** Define the reference load: 100 bots, view distance 10, simulation distance 8, a fixed world, 30 minutes. Record TPS, MSPT, peak RSS, chunk count, and join time.

**Done:** The baseline is in the repo; release candidates must not regress by more than 5% without an approved note.

### PF12 — NUMA awareness `P2`

**Scope:** Evaluate `#2854`; do not add complexity unless the benchmark shows a win.

**Done:** A decision note with numbers.

### PF13 — Hoe lag `P2`

**Scope:** `#2321`; profile and fix the hoe interaction path.

**Done:** Hoe use does not spike the tick.

### PF14 — Scheduler and locks `P2`

**Scope:** `#2741` high-player-count lock issue; scheduler cancellation/completion. `#3493`.

**Done:** 100 concurrent commands do not deadlock.

**Issues/PRs:** `#3493`, `#2741`.

---

## 22. Documentation and i18n (W19)

### S01 — Codebase docs `P0`

**Scope:** Complete the `#1482` list: `pumpkin`, `pumpkin-codecs`, `pumpkin-data`, `pumpkin-gametest`, `pumpkin-plugin-runtime`, `pumpkin-plugin-wit`, `pumpkin-protocol`, `pumpkin-world`. Module-level docs, architecture, data flow, and extension points.

**Done:** `cargo doc` has no missing-doc warnings for public items in those crates; each crate has an architecture section.

**Issues/PRs:** `#1482`.

### S02 — Admin docs `P0`

**Scope:** Install, first run, config reference, world management, backups, permissions, proxies, RCON/query, performance tuning, troubleshooting.

**Done:** A new admin can run a production server from the docs.

### S03 — Plugin docs `P0`

**Scope:** Native and Wasm plugin guides, ABI versioning, event list, API reference, examples, migration from 0.x.

**Done:** `pumpkin-plugin-api` docs and the WIT docs are published; two example plugins build.

### S04 — Config reference `P0`

**Scope:** Every `pumpkin.toml` field with type, default, range, and effect.

**Done:** The reference is generated or checked against `PumpkinConfig` in CI.

### S05 — Translations `P1`

**Scope:** Server locale, log translations, command descriptions, plugin custom translations, public translation workflow, optimize `to_translated`. `#1726`.

**Done:** A non-English client sees translated UI/commands; a translator can submit a locale without touching Rust.

**Issues/PRs:** `#1726`, `#3846`.

### S06 — Command translations `P2`

**Scope:** `#960`; argument parsing and error messages.

**Done:** Command errors are localized.

### S07 — Log translations `P2`

**Scope:** `#1726`; server logs use the server locale.

**Done:** Logs are localized where a translation exists.

### S08 — Public translation platform `P2`

**Scope:** Decide on Crowdin or a Git-based workflow; document it.

**Done:** A public contribution path exists.

### S09 — Contribution and AI policy `P1`

**Scope:** The fork already has an AI-friendly policy. Ensure `CONTRIBUTING.md`, `AGENTS.md`, and the PR template agree, including the human review/play-test requirement.

**Done:** A new contributor knows exactly what is required to open a PR.

### S10 — Changelog and release notes `P0`

**Scope:** Per-release notes with breaking changes, migration steps, known issues, and captures.

**Done:** Every release candidate has notes reviewed by the release owner.

### S11 — Triage `P1`

**Scope:** Label every open issue, close stale ones, link duplicates, assign workstreams, and keep the roadmap appendix current. Triage the 313 open PRs: mark each as land/needs-rebase/superseded/close, and attach it to a task ID.

**Done:** Zero unlabeled open issues; zero un-triaged open PRs; the appendix matches GitHub.

### S12 — Website and download docs `P2`

**Scope:** Download page matches artifacts, Windows Defender note, Docker tags, install commands. `#3049`.

**Done:** The download page links only to verified artifacts.

### S13 — AGPL decision `P2`

**Scope:** `#2478`; decide with the maintainers. Do not relicense without contributor permission.

**Done:** A decision is recorded in `LICENSE`/`README`.

---

## 23. Parity audit (W20)

### T01 — Audit every green tracker entry `P0`

**Scope:** The tracker marks a block/item/command "done" when a source file exists. Audit all 402 green entries by category, starting with the highest-risk: entities (48), blocks (162), items (51), commands (91), redstone (24), combat (13), world (13).

**Done:** Each entry has one of: a test, a vanilla diff note, or a downgrade to partial. The audit result is in appendix B.

### T02 — Registry completeness `P0`

**Scope:** Compare every block, item, entity type, effect, enchantment, attribute, damage type, particle, sound, biome, structure, feature, and tag with vanilla 26.3. Generated data is the source of truth; runtime registration is the check.

**Done:** A registry diff script reports zero missing entries or lists them in `T14`.

### T03 — Block state and property coverage `P1`

**Scope:** Every block state property, default, and codec round-trips; placement and neighbor updates use the right property.

**Done:** A block-state corpus round-trips; a property diff reports zero mismatches.

### T04 — Entity NBT round-trip `P1`

**Scope:** Every entity type writes/reads its vanilla NBT keys; unknown keys are preserved.

**Done:** A vanilla entity NBT corpus loads and saves without loss.

### T05 — Command parser edge cases `P1`

**Scope:** Quoted strings, Unicode, selectors, coordinates, block states, NBT, time/duration, item predicates.

**Done:** A parser corpus matches vanilla errors and suggestions.

### T06 — Loot table load coverage `P0`

**Scope:** `P10`; every table in the vanilla data pack loads.

**Done:** Zero loader warnings for the vanilla pack.

### T07 — Recipe coverage `P0`

**Scope:** `P11`; every recipe loads and crafts.

**Done:** Zero unsupported recipe types.

### T08 — Advancement coverage `P0`

**Scope:** `P01`/`P02`; every advancement has a trigger and can be earned.

**Done:** A bot earns every advancement in a scripted run (or the advancement is documented as creative-only).

### T09 — Structure coverage `P1`

**Scope:** Every structure set, structure, template pool, processor list, and loot table loads and generates.

**Done:** `/locate` finds every structure; a generated world contains each structure at least once in a large sample.

### T10 — Biome and feature coverage `P1`

**Scope:** Every biome generates, every placed/configured feature is implemented, every biome spawn list is correct.

**Done:** A world with a large radius contains every biome; a feature audit reports zero unimplemented configured features.

### T11 — Game rule coverage `P2`

**Scope:** Every vanilla game rule exists, persists, and affects behavior.

**Done:** A game-rule matrix test toggles each rule and observes the effect.

### T12 — Sound and particle coverage `P2`

**Scope:** Every sound event and particle type used by vanilla has a registry entry and a trigger.

**Done:** A coverage script reports zero missing sound/particle IDs.

### T13 — Invalid issue triage `P2`

**Scope:** Re-test `#1220` (fog), `#2552` (test crash), and any issue labeled `invalid`/`question`. Close or convert to a task.

**Done:** No stale invalid issue remains open without a note.

### T14 — Accepted deviations `P0`

**Scope:** The only place a parity gap may live. Each row: vanilla behavior, Pumpkin behavior, reason, issue link, and whether it is permanent or planned.

**Done:** The release notes link to `T14`; no other document claims 100% parity while a deviation exists.

---

## 24. Dependency graph and waves

### Wave 0 — Triage and foundations (week 0-1)

- `R01`, `R02` design, `R03` design, `R07` baseline, `PF11` baseline.
- `T01` tracker audit starts.
- `Q02` headless harness starts.
- `I05` chunk ticket design.
- `K01` codec bounds.
- `O05` security checklist.

**Exit:** Scope frozen; CI baseline recorded; no workstream is blocked on an undefined shared interface.

### Wave 1 — Unblock the parallel work (weeks 1-4)

- `A01`-`A07` navigation, targets, move control.
- `A08`-`A11` Brain core, sensors, behaviors.
- `D01`-`D02` redstone/pistons.
- `D07` vibration dispatch.
- `E13`-`E16` recipe types, loot functions, RNG.
- `I01`-`I05` save, region writer, tickets.
- `K01`-`K03` codec, packet events, join.
- `O01`, `O05` config/security.

**Exit:** A mob can navigate, target, and use a brain; a piston and a sculk sensor work; a chest 10k blocks away saves on stop; a malformed packet cannot panic.

#### Wave 1 close-out (2026-10-20)

Owner roles here match the workstream that owns the file. "Code" means read against vanilla 26.3 at the linked lines; "live" means a boot/bot/manual check still has to be run before the row can move.

| Task | Owner | Status | Verification |
| :--- | :--- | :--- | :--- |
| `W3`/`C04` boats, chest boats and rafts (22 rows) | entity | code merged, live check pending | Port of `#4073`: boat status/float/movement/control, ejection, dismount, passengers, lily pads, chest-boat container fallback. Live checks: surface float, paddle, sink/eject, dismount, two riders, chest persistence. |
| `W2` small mob parity batch (13 rows) | entity | code merged, live check pending | Zombie baby speed/XP, Zoglin living targets, Evoker/Vindicator/Pillager revenge, Vindicator Johnny, Husk hunger/sun immunity, Wolf sit/leap, Armadillo roll, Polar Bear cub defence, Stray/Parched arrows, Endermite persistence, Wither Skeleton equipment/targets. Bot checks per row. |
| `W4` small block/redstone batch (12 rows) | block | code merged, live check pending | Note Block game event/custom head, rails dedupe and power boundary, stem support tag, Chiseled Bookshelf statistic, Bell/Jukebox events, Ender Chest piglin anger, Big Dripleaf tilt, Sugar Cane fluid support, Campfire ignition. |
| `W8` port upstream `#4029`: `/locate` must resolve structure starts with the generation height sampler | worldgen | code applied, live check pending | `structure_finder.rs` now builds `NoiseHeightSampler` and uses `generator.biome_supplier`; regression test at `structure_finder.rs:439`. Live: locate an ungenerated village and teleport. |
| `W5` Name Tag | Wave 1 verifier | done | `item/items/name_tag.rs:20-34` matches vanilla `NameTagItem` for name, persistence and decrement. Live: relog with a named mob. |
| `W4` Shulker Box | Wave 1 verifier | partial | Missing vanilla `canOpen` lid-space check and piglin anger (`ShulkerBoxBlock.java:68-82`); the tracker premise is inverted. Live check after the fix. |
| `W4` Structure Block | Wave 1 verifier | done for the stated gap | Save/load work and NBT round-trips, so "capture/save/load execution" is covered. Corner-mode scan and redstone execution remain as separate gaps; live check them before calling the block fully done. |
| `W14` `/locate` coverage | Wave 1 verifier | partial | Datapack ids, flat/custom generators and biome ring order differ from vanilla; structure/biome/POI branches exist. Live check all three branches. |
| `I01` world loading | Wave 1 verifier | done | `level.rs:150-176` canonical plus `DIM1`/`DIM-1` fallback with tests; `anvil.rs:361-420` reads `level.dat` and versions. Live: import a 26.3 world. |
| `I01` world saving, `#3902` | Wave 1 verifier | verify | `stop` now saves block entities before `level.shutdown`; `save-all flush` still does not flush and `add_block_entity_nbt` can drop unloaded chunks. Live A/B in `#3902`. |
| `T01`/`W14` parity bookkeeping | main agent | code merged, live check pending | PARITY now projects 464 / 94 / 4 = 82.6% if all Wave 1 live checks pass; the verified headline stays 415 / 143 / 4 until then. CI is the compile/test gate for this push. |

### Wave 2 — Subsystem parity (weeks 3-8)

- `W2` mob tasks in parallel by family.
- `W3` entities/vehicles.
- `W4` blocks/redstone remaining.
- `W5` items/inventory.
- `W6` combat.
- `W7` player.
- `W8` worldgen.
- `W10` world simulation.
- `W12` Bedrock.
- `W13` API surface.
- `W14` commands/datapacks.

**Exit:** Every P0 task outside release engineering is done; P1 backlog is shrinking.

### Wave 3 — Data, content, and polish (weeks 7-12)

- `W16` data parity.
- `H09` 26.3 audit.
- `W18` performance.
- `W19` docs/i18n.
- `W17` GameTest/bot suite.
- `R04` platform builds.

**Exit:** All gates except G3 and G10 are green or have a dated plan.

### Wave 4 — Release candidates and community test (weeks 10-14)

- `R05`, `R09`, `R12`, `R13`.
- Release candidate 1, 2, 3.
- Fix only P0/P1 regressions found by testers.
- Freeze, tag, publish.

**Exit:** 1.0.0.

### Dependency hotspots

| Task | Blocks | Why |
| :--- | :--- | :--- |
| `A01` | `W2` aquatic/flying, `W3` vehicles | Navigation type determines movement |
| `A05` | `W2` wolves/golems/raids | Anger/team predicate |
| `A08` | `W2` brain mobs | Brain API |
| `D07` | `W2` warden, `W16` game events | Vibration dispatch |
| `E15` | `W2` loot, `W4` blocks, `W8` structures | Loot functions |
| `E13` | `W5` recipes, `W14` recipe command | Recipe types |
| `I05` | `Q01` GameTest, `W8` locate | Chunk tickets |
| `K01` | `W11`, `W12`, `W13` packet events | Bounds/state machine |
| `R02`/`R03` | `W13` all | ABI freeze |

### Immediate queue (first 10 PRs)

1. `R01` freeze the scope in `README.md`.
2. `T01` start the tracker-green audit and publish the result table.
3. `I05` chunk tickets/forceload.
4. `K01` packet bounds and panic removal.
5. `O05` security checklist and the first fixes.
6. `A01` navigation-type hook.
7. `D07` vibration/game-event dispatch.
8. `E15` land the local `copy_components` branch and finish loot functions.
9. `E13` recipe types.
10. `Q01` GameTest blockers.

These ten unblock the most downstream work. Do not start the per-mob or per-block fan-out before `A01`, `D07`, `E15`, and `Q01` are at least at a stable interface.

---

## 25. Verification ladder

Every task uses the highest applicable rung. A lower rung does not substitute for a higher one.

1. **Compile + clippy** — `cargo clippy -p <crate> --all-targets --all-features`, no new warnings.
2. **Unit test** — codec, parser, math, state machine. Compare with known vanilla bytes/values, not a reimplementation of the same formula.
3. **GameTest** — block/entity behavior in a real world, headless.
4. **Bot test** — join, move, interact, inventory, combat, container. Mineflayer or rust-mc-bot, scripts outside the repo.
5. **Boot test** — start the server in a scratch directory, wait for "done", run the console command, read the log for panics/errors, `stop`.
6. **Real client** — rendering, particles, sounds, animations, GUI, client prediction. Capture attached to the PR.
7. **Soak/benchmark** — 30+ minutes, 100 bots, memory/TPS recorded.

### 25.1 PR handoff checklist

- [ ] Issue linked; task ID in the title or description.
- [ ] Branch based on current `master`/`fork-ai-main`.
- [ ] Only the task's files changed; no unrelated formatting/lockfile/generated churn.
- [ ] Vanilla source checked; the Java method/class is named in the description.
- [ ] Callers of changed shared functions listed.
- [ ] Removed lines accounted for.
- [ ] `cargo fmt --check`.
- [ ] `cargo clippy -p <crate> --all-targets --all-features`.
- [ ] `cargo nextest run -p <crate> --no-tests=pass`.
- [ ] `cargo machete`.
- [ ] `typos`.
- [ ] Boot test log attached (for runtime changes).
- [ ] Bot/GameTest result attached (for behavior changes).
- [ ] Real-client capture attached (for player-visible changes).
- [ ] AI disclosure and human reviewer named.

### 25.2 What not to test

Do not add tests that repeat the implementation, check constants against a copy of themselves, cover getters/setters/derives, or assert that removed code is gone. A test that passes on `master` proves nothing about a bug fix; make it fail first.

---

## 26. Release candidate checklist

- [ ] `G1`-`G11` green.
- [ ] Zero open P0 issues.
- [ ] All P1 issues either closed or in `T14`.
- [ ] Stable ABI fixtures pass.
- [ ] v0.1 and v0.2 plugins load.
- [ ] Vanilla 26.3 world import/export passes.
- [ ] All chunk formats pass.
- [ ] `stop` with a distant chest passes.
- [ ] 1,000 join/leave soak passes.
- [ ] 100-bot 30-minute benchmark meets `PF11`.
- [ ] Protocol fuzz 24 hours clean.
- [ ] Platform smoke matrix passes.
- [ ] Docker smoke passes.
- [ ] Security checklist signed.
- [ ] Docs and config reference published.
- [ ] Translations at least for the shipped locales.
- [ ] Release notes and known issues.
- [ ] Captures for every player-visible change since the last RC.
- [ ] Community test report triaged.
- [ ] Tag, sign, publish, announce.

---

## Appendix A — Open issue index (352)

Each row is one open upstream issue and its primary workstream. Cross-cutting issues are also named in the task text. This table is generated from the 2026-10-20 inventory; if an issue is closed, remove the row in the same PR that closes it.

| Issue | Title | Workstream |
| ---: | :--- | :--- |
| #4126 | Dolphins have several issues relating to their behavior. | **W2** Mob parity |
| #4125 | KeepInventory gamerule seems to apply on all dimesions | **W7** Player |
| #4124 | Villages do not appear to spawn villagers or iron golems | **W2** Mob parity |
| #4123 | Server does not appear to send the packet for the animation when a warden hits you | **W11** Java protocol |
| #4122 | Wardens do not have a sonic boom | **W2** Mob parity |
| #4121 | Mobs have a unusual collision | **W1** Mob AI foundation |
| #4109 | Server freezes on server.create_world method | **W13** Plugin API |
| #4090 | Floting Fish | **W2** Mob parity |
| #4089 | Firework Rockets All being Power 1 | **W5** Items/inventory/loot |
| #4084 | TNT explodes with delay | **W10** World simulation |
| #4079 | Nether trees can generate through solid terrain | **W8** Worldgen |
| #4078 | some dungeons are generated differently than in vanilla. | **W8** Worldgen |
| #4077 | Dripstone caves are not generating correctly. | **W8** Worldgen |
| #4076 | geodes do not generate | **W8** Worldgen |
| #4075 | Empty command in console should be ignored not error | **W15** Ops/security |
| #4070 | Parts of trees generating across a chunk border get replaced with grass | **W8** Worldgen |
| #4064 | Sleeping in a bed causes damage | **W7** Player |
| #4063 | Fish go wild on land | **W2** Mob parity |
| #4052 | Clarity Regarding Docker Image Tags | **W15** Ops/security |
| #4048 | Memory grows by about 3 MB with every player join and leave | **W18** Performance |
| #4036 | Spawn Egg problems | **W3** Entities/vehicles |
| #4035 | deep dark generates practically nothing in terms of sculk and only sculk veins | **W8** Worldgen |
| #4034 | Woodland Mansion generates buried while the vanilla counterpart does not | **W8** Worldgen |
| #4033 | ruined portal generates near bedrock while the vanilla counterpart does not | **W8** Worldgen |
| #4027 | shipwreck stairs have a incorect block state | **W8** Worldgen |
| #4021 | Support for Experimental Minecart Improvements | **W12** Bedrock |
| #4020 | Loot table generation uses non deterministic RNG instead of seed-based random sequences (1.20+) | **W5** Items/inventory/loot |
| #4011 | Extending a chest from diagonally above does not work properly | **W4** Blocks/redstone |
| #4006 | Bug: PacketSentEvent handlers are rarely or never invoked by the core event dispatcher | **W13** Plugin API |
| #4005 | Aquatic mobs bobbing instead of swimming | **W2** Mob parity |
| #3989 | Trial Vault crashes the server | **W4** Blocks/redstone |
| #3974 | fix(entity): correct hoglin and zoglin throw guards and velocity behavior | **W2** Mob parity |
| #3973 | Placing a powder snow bucket by hand consumes the bucket instead of returning an empty one | **W5** Items/inventory/loot |
| #3972 | Powder snow placed from a bucket by a dispenser is not shown to clients | **W4** Blocks/redstone |
| #3971 | Mob buckets do not keep the mob data, so a released axolotl gets a random color | **W5** Items/inventory/loot |
| #3970 | Dispensers drop filled buckets in front of replaceable blocks (short grass) instead of replacing them | **W4** Blocks/redstone |
| #3969 | Filled water and mob buckets create a water block above seagrass instead of using its water | **W5** Items/inventory/loot |
| #3933 | Catching a fish, axolotl or tadpole with a water bucket does not give the mob bucket | **W5** Items/inventory/loot |
| #3926 | Structure generation and /locate have no per-dimension structure state | **W8** Worldgen |
| #3921 | Expose OpenTelemetry server metrics | **W15** Ops/security |
| #3918 | Trident infinity damage | **W6** Combat |
| #3917 | Crafting Table Duplication Glitch | **W5** Items/inventory/loot |
| #3908 | feat(api/wit) EntityType from/to registry key helpers | **W13** Plugin API |
| #3907 | https://modrinth.com/mod/footprint | **W8** Worldgen |
| #3902 | Chest contents and structure loot tables are lost on `stop` in chunks away from spawn (only an explicit `save-all` persists them) | **W9** Persistence/chunks |
| #3900 | Piston problem | **W4** Blocks/redstone |
| #3899 | Flying in creative while flying with a elytra you move strangly | **W7** Player |
| #3888 | Taking damage/getting killed while sleeping doesn't wake the player up | **W7** Player |
| #3881 | Hopper-to-hopper cooldown compares cooldown_time instead of tickedGameTime | **W4** Blocks/redstone |
| #3880 | Hopper minecart picks up a dropped stack one item at a time | **W3** Entities/vehicles |
| #3879 | Floating snow in Snowy Taiga village | **W8** Worldgen |
| #3873 | Wind Charges Don't work | **W5** Items/inventory/loot |
| #3867 | Tracking: GameTest framework issues | **W17** QA/testing |
| #3858 | Indiscriminate use of `tokio::sync` throughout the codebase | **W18** Performance |
| #3855 | Breaking block below snow carpet bugs out in random ways | **W4** Blocks/redstone |
| #3847 | dupe with chiseled bookshelfs | **W5** Items/inventory/loot |
| #3844 | Crossbow Loading results in client kick | **W6** Combat |
| #3841 | Plugin API: Mutable (bedrock) resource pack configuration | **W13** Plugin API |
| #3838 | Button noise fires twice | **W4** Blocks/redstone |
| #3837 | Standing on one block above a magma block with snow placed on it does damage | **W4** Blocks/redstone |
| #3836 | When I teleport with /tp into not generated chunks i die in the void | **W7** Player |
| #3833 | Pistons can be powered from the head when the head is facing down. | **W4** Blocks/redstone |
| #3831 | In Bedrock edition, if try to move inside a 1 block whole you can Crawl | **W12** Bedrock |
| #3830 | Fix shearing animation | **W3** Entities/vehicles |
| #3822 | Dispensers can only shear sheep and beehives | **W4** Blocks/redstone |
| #3818 | Old Growth Spruce missing Blocks in 3rd layer | **W8** Worldgen |
| #3806 | Enderman Does not Damage when throwing water bottle | **W2** Mob parity |
| #3802 | First time world join kicked for packet spam | **W11** Java protocol |
| #3798 | Zombies burn under roofs | **W2** Mob parity |
| #3785 | Enderman does not reset anger on player death | **W2** Mob parity |
| #3784 | Zombie does not have Drowned conversion vanilla parity target | **W2** Mob parity |
| #3782 | Failed to decode packet 'clientbound/minecraft:container_set_content' | **W11** Java protocol |
| #3781 | Breaking ice blocks in creative mode replaces the ice with water | **W4** Blocks/redstone |
| #3779 | /setblock returns syntax error when adding block data | **W14** Commands/data packs |
| #3777 | Packet Decode failure for clientbound/minecraft:level_particles (Client Disconnect) | **W11** Java protocol |
| #3772 | Fire propagation from natural lava source is infinite | **W10** World simulation |
| #3767 | Item frames lack placement and survival validation | **W3** Entities/vehicles |
| #3761 | Villager trading | **W2** Mob parity |
| #3760 | /tick sprint shows completion message when starting | **W14** Commands/data packs |
| #3759 | Recursive /function calls crash the server with a stack overflow | **W14** Commands/data packs |
| #3758 | /execute store rejects valid vanilla commands | **W14** Commands/data packs |
| #3742 | Villagers move while sleeping | **W2** Mob parity |
| #3738 | Bedrock 1.26.51 cannot join via NetherNet – TLS ClientHello receives plaintext HTTP 400 | **W12** Bedrock |
| #3737 | Breaking Block Sound only plays shortly after starting to break a block | **W4** Blocks/redstone |
| #3735 | I would like to be able to support more Bedrock versions of Java in parallel, instead of just one fixed version. | **W12** Bedrock |
| #3734 | Plugin API: per-player entity visibility (show/hide entity, visible-by-default) and non-persistent entities, Paper-style | **W13** Plugin API |
| #3724 | bottom log doesnt generate | **W8** Worldgen |
| #3716 | Server panics on join when a plugin handles PacketReceivedEvent: packets are decoded regardless of connection state | **W11** Java protocol |
| #3712 | PumpkinMC does not generate some structures correctly | **W8** Worldgen |
| #3702 | TypeScript Wasm plugin fails after registering a command during `on-load` | **W13** Plugin API |
| #3701 | Building a plugin with pumpkin-plugin-utils' init fails | **W13** Plugin API |
| #3690 | End crystals damage, knockback and placement are not working as the intended way | **W6** Combat |
| #3688 | WIT: player-action-add-player.properties reuses attribute-shaped property struct | **W13** Plugin API |
| #3672 | Falling blocks don't slow with cobwebs | **W3** Entities/vehicles |
| #3659 | Fire may stay floating mid air after burning down a tree/forest | **W10** World simulation |
| #3648 | Flying high with an elytra and landing causes damage/death. | **W7** Player |
| #3647 | Removing the elytra still allows for flight. | **W7** Player |
| #3646 | Chiseled bookshelf book changes emit no block-change game event | **W4** Blocks/redstone |
| #3643 | Inserting or ejecting a jukebox record emits no block-change game event | **W4** Blocks/redstone |
| #3631 | Bells never resonate or reveal nearby raiders | **W4** Blocks/redstone |
| #3629 | Phantoms never circle or swoop-attack players | **W2** Mob parity |
| #3628 | Add support for arm/v7 | **W15** Ops/security |
| #3627 | Dispensers drop bone meal instead of applying it as fertilizer | **W4** Blocks/redstone |
| #3626 | Creepers with status effects leave no area-effect cloud when they explode | **W2** Mob parity |
| #3625 | Dispensers drop bottles o' enchanting instead of firing them | **W4** Blocks/redstone |
| #3624 | Experience orbs sink instead of floating in water | **W3** Entities/vehicles |
| #3623 | Tripwire state changes emit no game events | **W4** Blocks/redstone |
| #3622 | Broken paintings do not drop a painting item | **W3** Entities/vehicles |
| #3621 | Opening and closing containers emits no game events | **W4** Blocks/redstone |
| #3618 | Slimes cannot spawn in swamps or mangrove swamps | **W2** Mob parity |
| #3615 | Mining infested blocks with Silk Touch still spawns silverfish | **W4** Blocks/redstone |
| #3614 | Armor stands cannot equip or swap items through player interaction | **W3** Entities/vehicles |
| #3603 | Add "mob rate" options in pumpkin.toml | **W15** Ops/security |
| #3593 | Native `#[plugin_method]` futures are polled outside `GLOBAL_RUNTIME`, causing tokio reactor panics | **W13** Plugin API |
| #3581 | Players cannot use beds occupied by sleeping villagers | **W2** Mob parity |
| #3580 | Farmland and dirt paths turning into dirt can leave entities embedded | **W4** Blocks/redstone |
| #3577 | Boat placement snaps to the block center instead of the raycast hit point | **W3** Entities/vehicles |
| #3575 | Pass the used hand down to item behaviours (normal_use, normal_use_with_rotation) | **W13** Plugin API |
| #3574 | Breaking tripwire with shears does not consume durability | **W4** Blocks/redstone |
| #3567 | Waxing copper trapdoors resets their orientation and open state | **W4** Blocks/redstone |
| #3566 | Endermen place carried blocks where the block cannot survive | **W2** Mob parity |
| #3561 | Attempt to attack invalid entity when hitting ender dragon | **W6** Combat |
| #3560 | Fence gates turn farmland and dirt paths into dirt | **W4** Blocks/redstone |
| #3558 | Sugar cane cannot be planted beside water or waterlogged blocks | **W4** Blocks/redstone |
| #3557 | Lily pads cannot be placed on waterlogged blocks | **W4** Blocks/redstone |
| #3552 | Projectiles do not collapse big dripleaf leaves | **W4** Blocks/redstone |
| #3551 | Scheduled block/fluid ticks execute in chunks outside `active_chunks` (asymmetric with random ticks) | **W10** World simulation |
| #3550 | Burning projectiles cannot relight extinguished campfires | **W4** Blocks/redstone |
| #3548 | Jukebox playback state never resumes after chunk or server reload | **W4** Blocks/redstone |
| #3545 | Sitting cats do not block chests or ender chests from opening | **W2** Mob parity |
| #3542 | Curse of Binding armor can be removed in non-Creative modes | **W5** Items/inventory/loot |
| #3538 | `/gamemode` uses the wrong variables | **W14** Commands/data packs |
| #3522 | Successful fishing yields no item, experience, or rod durability cost | **W5** Items/inventory/loot |
| #3521 | Bed and respawn-anchor explosions do not create fire | **W10** World simulation |
| #3520 | Shields cannot block ordinary player melee attacks | **W6** Combat |
| #3519 | Tracking: implement Wilderness Bound gameplay and world generation from Java 26.3 | **W8** Worldgen |
| #3518 | Dispensers lose aquatic mobs from filled buckets | **W4** Blocks/redstone |
| #3516 | Crops continue growing below vanilla light level | **W4** Blocks/redstone |
| #3515 | Explosion knockback ignores Blast Protection and the resistance attribute | **W6** Combat |
| #3514 | Uncharged respawn anchors explode outside the Nether | **W10** World simulation |
| #3513 | Attack packets bypass server-side entity interaction range | **W6** Combat |
| #3512 | Older periodic player snapshots can overwrite newer disconnect saves | **W9** Persistence/chunks |
| #3511 | Player data saves truncate the last good file before the new write succeeds | **W9** Persistence/chunks |
| #3502 | Hardcore mode does not force difficulty to Hard at server startup | **W15** Ops/security |
| #3489 | Arrows dont hurt | **W6** Combat |
| #3487 | Cant Connect on Bedrock | **W12** Bedrock |
| #3484 | plugin-wit: Not all interfaces are imported | **W13** Plugin API |
| #3468 | Tracking: Mob AI | **W1** Mob AI foundation |
| #3464 | Pressing against a mob after taking knockback launches the player upwards (server keeps sending the player its own stale SetEntityMotion y) | **W6** Combat |
| #3456 | Implement the minecraft:copy_components loot function (block entity data is lost on break) | **W5** Items/inventory/loot |
| #3455 | Define authoritative scoreboard source for gameplay team membership | **W1** Mob AI foundation |
| #3448 | Select the center block for collapsed world border clamp ranges | **W10** World simulation |
| #3446 | Deduplicate Java experience synchronization packets | **W11** Java protocol |
| #3431 | Configuration load failures are completely silent: the server starts with default values and prints nothing | **W15** Ops/security |
| #3404 | Complete snow placement validation in FreezeTopLayerFeature | **W8** Worldgen |
| #3396 | Bedrock: multiple bugs (checking) | **W12** Bedrock |
| #3392 | Prevent datapack loaders from recursing into symbolic-link directories | **W14** Commands/data packs |
| #3391 | Dying deletes (or does insane durability to) armor equipped | **W7** Player |
| #3389 | Teleporting across dimensions causes visual and client bugs | **W11** Java protocol |
| #3388 | Arrows have glitchy particles | **W6** Combat |
| #3385 | Entities persist after leaving simulation distance causing them to become invalid | **W9** Persistence/chunks |
| #3383 | Entities aren't taking damage when attacked | **W6** Combat |
| #3382 | Attacking ender dragon disconnected me | **W6** Combat |
| #3379 | Sending message in chat changes name in tablist | **W7** Player |
| #3378 | Creeper explosion disconnected me from the server | **W6** Combat |
| #3377 | Bottom dripstone does not deal damage | **W4** Blocks/redstone |
| #3376 | Mobs do not aggro on player, and only sometimes do when attacked with weapon | **W1** Mob AI foundation |
| #3375 | Health bar visual bug | **W7** Player |
| #3368 | Items floating in water for an extended amount of time can't be picked up | **W3** Entities/vehicles |
| #3367 | game_rules/weather/wandering_trader read from root only, Paper-Purpur worlds silently lose them | **W9** Persistence/chunks |
| #3364 | [Feature] Implement vanilla Wandering Trader spawning and lifecycle | **W2** Mob parity |
| #3359 | Forgetting names | **W3** Entities/vehicles |
| #3357 | Wind burst enchant does not work | **W16** Data parity |
| #3356 | [Feature] Align natural spawning caps and cadence with vanilla | **W2** Mob parity |
| #3340 | Problems with the proxy server | **W12** Bedrock |
| #3319 | Decoding issue | **W11** Java protocol |
| #3293 | Composter gives you free bonemeal every time you break it | **W4** Blocks/redstone |
| #3292 | Berries duplicate when placing them and breaking them without letting them grow | **W5** Items/inventory/loot |
| #3276 | Datapack load functions execute during reload while game ticks are frozen | **W14** Commands/data packs |
| #3272 | Duplicate UUID | **W9** Persistence/chunks |
| #3264 | 300-500% CPU spike lasting ~1.5 min on first player join per server session (only ~5s on subsequent rejoins); persistent 55-60% idle floor afterward | **W18** Performance |
| #3262 | Rabbit/Bunnies movement & animation | **W2** Mob parity |
| #3250 | Leaves drop saplings when broken too often | **W8** Worldgen |
| #3246 | GUI dashboard | **W13** Plugin API |
| #3240 | Incorrect music disc drops | **W5** Items/inventory/loot |
| #3239 | Lava and water interaction | **W10** World simulation |
| #3232 | Can't multi drop with shift q | **W5** Items/inventory/loot |
| #3225 | Sheep drop wool of a random color instead of their own | **W5** Items/inventory/loot |
| #3166 | Soulspeed does not work | **W16** Data parity |
| #3163 | Plugin API: get-command-sender-type and require-with-handler-id are stubs that trap the plugin instance | **W13** Plugin API |
| #3144 | Extended piston client desync | **W4** Blocks/redstone |
| #3143 | Free affect's - Switch Exploit. | **W5** Items/inventory/loot |
| #3140 | Explosions do not damage entities | **W6** Combat |
| #3133 | Lever: on_state_replaced ignores the piston `moved` flag, and normal_use returns Consume instead of Success | **W4** Blocks/redstone |
| #3126 | Trees Got Clipped at Chunk Border | **W8** Worldgen |
| #3124 | Village Generated Under Ground | **W8** Worldgen |
| #3120 | When breaking block with torch on it in creative, torch doesnt drop | **W4** Blocks/redstone |
| #3116 | Chunk delivery to a moving client stalls at ~1 chunk/tick while generation runs far ahead | **W9** Persistence/chunks |
| #3114 | Sulfur Cube mobs do not have AI | **W2** Mob parity |
| #3113 | Non-Ponder Goat Horn item inventory disconnects | **W5** Items/inventory/loot |
| #3111 | Crops cannot be trampled | **W4** Blocks/redstone |
| #3108 | Player disconnects when picking up any painting into the inventory. | **W3** Entities/vehicles |
| #3107 | Sculk Shrieker doesnt activate | **W4** Blocks/redstone |
| #3105 | Observers doesnt work | **W4** Blocks/redstone |
| #3103 | Cant place paintings or itemframes | **W3** Entities/vehicles |
| #3100 | Swamp huts dont generate | **W8** Worldgen |
| #3099 | Magma blocks generated in the terrain dont produce bubbles underwater. | **W10** World simulation |
| #3098 | Village paths are not generating correctly. | **W8** Worldgen |
| #3096 | Tracking: API | **W13** Plugin API |
| #3092 | XP Orbs do not absorb | **W3** Entities/vehicles |
| #3089 | `player_server` no reactor running | **W13** Plugin API |
| #3086 | Http Output Plugin isssue | **W13** Plugin API |
| #3084 | When giving item with nbt data, it crash the player. | **W5** Items/inventory/loot |
| #3083 | Wandering Trader Spawns with All Trades Locked | **W2** Mob parity |
| #3082 | Chest on chest boat doesnt work | **W3** Entities/vehicles |
| #3081 | Shears doesn't work on mooshroom cow | **W5** Items/inventory/loot |
| #3073 | Can't open inventory when item has custom data | **W5** Items/inventory/loot |
| #3071 | Minecarts not going uphill | **W3** Entities/vehicles |
| #3070 | dying with an elytra makes you glide until you relog | **W7** Player |
| #3069 | Trident Drowned Only Attack in Melee Range | **W2** Mob parity |
| #3068 | Boats cannot be broken by hand | **W3** Entities/vehicles |
| #3065 | Eyeblossom state changes send malformed Trail particle data and disconnect 26.2 clients | **W11** Java protocol |
| #3064 | take damages when you get closer to a block | **W12** Bedrock |
| #3062 | Sweeping attack effect appears in the wrong position | **W6** Combat |
| #3061 | Incorrect boat placement. Boats do not drop boat item after breaking. | **W3** Entities/vehicles |
| #3060 | Mushrooms break unexpectedly | **W4** Blocks/redstone |
| #3053 | Spectator mode interactions | **W7** Player |
| #3051 | Fishing rod is broken | **W5** Items/inventory/loot |
| #3049 | Windows Defender | **W19** Docs/i18n |
| #3040 | Seagrass floating on-top of water ravines | **W8** Worldgen |
| #3039 | Tool durability reset upon dropping and picking up item | **W5** Items/inventory/loot |
| #3020 | Campfire block cannot cook and cannot be extinguished by potion | **W4** Blocks/redstone |
| #3018 | Message without coordinates: Set spawn point to %s, %s, %s [%s, %s] in %s for %s | **W19** Docs/i18n |
| #3005 | Stronghold ring placement samples biomes from the triggering chunk | **W8** Worldgen |
| #3004 | Server logs interpolation | **W19** Docs/i18n |
| #3003 | Ring bell with arrow shot | **W4** Blocks/redstone |
| #3002 | Bell animation missing | **W4** Blocks/redstone |
| #2991 | No animation | **W3** Entities/vehicles |
| #2981 | Player not kicked on server stop | **W12** Bedrock |
| #2976 | Bug: Furnace Interaction Restores Tool Durability in Survival Mode | **W5** Items/inventory/loot |
| #2923 | cant join more then 1 player | **W11** Java protocol |
| #2901 | Advancements show up in unparsed form | **W16** Data parity |
| #2885 | Incorrect Glow Lichen generation (full blocks and surface level) | **W8** Worldgen |
| #2883 | set-max-health allows values below 1 | **W13** Plugin API |
| #2879 | Mineshaft has too many fences | **W8** Worldgen |
| #2877 | Trail ruins generate with only suspicious gravel | **W8** Worldgen |
| #2872 | Enderman floating and walking in mid-air | **W2** Mob parity |
| #2871 | Banned Player Join Event | **W13** Plugin API |
| #2861 | Build fails with rustc SIGSEGV while compiling cranelift-codegen | **W15** Ops/security |
| #2854 | Numa Awareness | **W18** Performance |
| #2833 | Item pickup offhand | **W5** Items/inventory/loot |
| #2827 | Crash related to Storage blocks | **W9** Persistence/chunks |
| #2810 | Plugin API: world-editing follow-ups for WorldEdit-style plugins | **W13** Plugin API |
| #2789 | Untamed Wolves Don't Attack Sheep | **W2** Mob parity |
| #2781 | Birch tress generating next to each other | **W8** Worldgen |
| #2776 | Zombie dropped all items | **W5** Items/inventory/loot |
| #2769 | Mob drop issues | **W5** Items/inventory/loot |
| #2741 | Lock issue with high player count - concurrent command run | **W18** Performance |
| #2699 | Redstone state updates are lost / desynced when activated | **W4** Blocks/redstone |
| #2698 | Lectern Book Takes Time To Appear | **W4** Blocks/redstone |
| #2632 | Add mutable enchanting-table events to the plugin API | **W13** Plugin API |
| #2626 | Chunk writer drops block_entities, structures and PostProcessing when saving a vanilla 26.2 world | **W9** Persistence/chunks |
| #2625 | swimming freaks out | **W7** Player |
| #2601 | Redstone is freaking out | **W4** Blocks/redstone |
| #2600 | Spectator Mode / 2 Accounts | **W7** Player |
| #2599 | water propogation issue | **W10** World simulation |
| #2592 | execute if block ignores its predicate; blockstate args, forceload and CLightUpdate also broken | **W14** Commands/data packs |
| #2577 | /pumpkin command has whitespace hoverable as the contributor list | **W14** Commands/data packs |
| #2573 | Saturation effect tweaking | **W7** Player |
| #2566 | Mobs have no audio, can't crit, and have no reaction to being hit in survival | **W6** Combat |
| #2552 | Server crashes when testing | **W17** QA/testing |
| #2536 | Ricgt clicking a mob with a spawn egg doesnt spawn a baby | **W3** Entities/vehicles |
| #2535 | Taming animals does nothing | **W2** Mob parity |
| #2533 | Animals grow up instantly on rejoin | **W2** Mob parity |
| #2532 | Breeding results in 2 babies instead of 1 | **W2** Mob parity |
| #2520 | /Effect command desynchronizing, When Leaving / Joining | **W7** Player |
| #2517 | Kicks me out when i spawn in any entity | **W11** Java protocol |
| #2516 | Add functionality to resolve file-based secrets for Velocity configuration. (Example @forwarding.secret) | **W15** Ops/security |
| #2501 | Structures are generated incorrectly | **W8** Worldgen |
| #2478 | Proposal: Adopt AGPLv3 for future contributions | **W19** Docs/i18n |
| #2467 | Sand disappears after falling | **W4** Blocks/redstone |
| #2466 | Something wrong in the nether | **W8** Worldgen |
| #2433 | Server freaks out when i enchant | **W5** Items/inventory/loot |
| #2427 | Lighting not updating properly | **W10** World simulation |
| #2370 | Bedrock Connection Issues | **W12** Bedrock |
| #2362 | Incorrect terrain generation in the Jungle | **W8** Worldgen |
| #2359 | a few bugs i found | **W2** Mob parity |
| #2321 | Hoe Hoe Hoe Lag | **W18** Performance |
| #2291 | Bedrock entity spawn crashes during chunk generation on Gen-Pool threads (no Tokio runtime) | **W12** Bedrock |
| #2232 | Mobs ignore hitbox after getting hit by player + ignore bamboo | **W1** Mob AI foundation |
| #2224 | Resetting rotation when switching gamemode | **W7** Player |
| #2222 | No keyrepeat in text fields | **W7** Player |
| #2220 | Ender Dragon collision bug | **W6** Combat |
| #2211 | Block IDs table for plugin developers | **W13** Plugin API |
| #2205 | Player death animation is broken | **W7** Player |
| #2196 | Player appears to be mining with wrong hand. | **W7** Player |
| #2194 | mobs can walk in air/ground | **W1** Mob AI foundation |
| #2180 | Arrows dont update position when block is broken | **W6** Combat |
| #2174 | I see SNK wall | **W8** Worldgen |
| #2162 | Drowned Zombie burn in water when jumping | **W2** Mob parity |
| #2151 | Visible clear-cut biome borders directly at chunk boundaries in Badlands/mesa biome | **W8** Worldgen |
| #2104 | Player floating and stuck in swim state(WARN: Terible english) | **W12** Bedrock |
| #2098 | Feat: dynamic storage types | **W13** Plugin API |
| #2094 | XP points seemingly stop adding after the 32-bit signed integer limit | **W7** Player |
| #2092 | Caves generate in the ocean | **W8** Worldgen |
| #2065 | When the ender dragon fight ends the portal back to the overworld doesn't open | **W10** World simulation |
| #2057 | Panic when Docker start | **W15** Ops/security |
| #2056 | WASM Plugin re-entry/concurrency issue | **W13** Plugin API |
| #2027 | Weird terrain generation, random blocks generating in the middle of grass and underfilled trees | **W8** Worldgen |
| #1985 | /effect crashes server | **W14** Commands/data packs |
| #1960 | Blazes can't fly | **W2** Mob parity |
| #1949 | Worldgen: Incorrect ore/stone generation | **W8** Worldgen |
| #1922 | player bugs | **W7** Player |
| #1871 | cannot place Scaffolding like usual | **W4** Blocks/redstone |
| #1809 | Pterodactyl/pelican egg won't work | **W15** Ops/security |
| #1807 | Unable to return to normal state after landing with Elytra flight | **W7** Player |
| #1800 | attemping to attack an invalid entity | **W6** Combat |
| #1781 | Bot Connection Issue - Compression Protocol Incompatibility with mineflayer | **W11** Java protocol |
| #1760 | Zombies still attack player when switching from creative to survival | **W2** Mob parity |
| #1726 | Tracking: Translations | **W19** Docs/i18n |
| #1679 | Players in boats don't move | **W3** Entities/vehicles |
| #1674 | When spawning water based animals on land. The water based animals will not suffocate and despawn. | **W2** Mob parity |
| #1622 | Crits don't produce particles | **W6** Combat |
| #1621 | Armor stands produce death particles when destroyed | **W3** Entities/vehicles |
| #1583 | Dying in the Nether without setting a spawnpoint, respawns on top of nether roof. | **W7** Player |
| #1564 | Flying machines bring unpredictable behavior | **W4** Blocks/redstone |
| #1555 | Memory leak when loading void chunks | **W18** Performance |
| #1554 | Can't use an existing world | **W9** Persistence/chunks |
| #1552 | Riptide Tridents inconsistently work while it's raining | **W6** Combat |
| #1551 | Elytra State & Using Fireworks with (and without) one equipped. | **W7** Player |
| #1482 | Tracking: Codebase Docs | **W19** Docs/i18n |
| #1404 | Tracking: Combat | **W6** Combat |
| #1403 | Tracking: World | **W9** Persistence/chunks |
| #1402 | Tracking: Redstone | **W4** Blocks/redstone |
| #1336 | Zombie path block detection broken | **W1** Mob AI foundation |
| #1330 | When you Crit even tho the cooldown is fully charged (you can make a crit in normal minecraft) it makes you perform a sweep hit like if you hold space and crit same time it makes a sweep hit instead of a crit some times | **W6** Combat |
| #1302 | Witch loot incorrect | **W2** Mob parity |
| #1300 | Coral reef biome spawns broken coral patterns | **W8** Worldgen |
| #1280 | Configurable API server endpoints | **W13** Plugin API |
| #1274 | Elytra Bug , Fire work 2&3 rocket crash the server , wind charge Not working ,Potions crash the server more idk | **W7** Player |
| #1220 | A fog problem in my server | **W20** Parity audit |
| #1199 | Unable to quit/infinite loop during save after client fails to connect | **W9** Persistence/chunks |
| #1110 | HAProxy Support | **W15** Ops/security |
| #960 | Use Translations for Argument parsing | **W19** Docs/i18n |
| #877 | Naturally generated water sometimes doesn't flow. | **W10** World simulation |
| #765 | Particles don't appear when you have an effect | **W16** Data parity |
| #764 | Effects | **W16** Data parity |
| #731 | Create unified registry system to load data packs, including the vanilla data pack | **W14** Commands/data packs |
| #635 | Add Persistent Data Container | **W13** Plugin API |
| #626 | KnockbackSync, custom Knockback, legacy PVP | **W6** Combat |
| #449 | Release Pumpkin 1.0.0 | **W0** Release |
| #118 | Support for multiple World/Chunk formats | **W9** Persistence/chunks |

---

## Appendix B — Tracker gaps (143 partial + 4 planned)

The tracker is a useful inventory, not a correctness proof. `done` entries are audited in `T01`; the rows below are the entries the tracker itself flags, minus the 13 rows this fork has already closed. "Task" is the primary workstream.

| Category | Item | Status | What the tracker says is missing | Task |
| :--- | :--- | :--- | :--- | :--- |
| entities | Blaze | partial | Return towards a restricted/home area | **W2** Mob parity |
| entities | Breeze | partial | Wind-charge combat, jumping and sliding | **W2** Mob parity |
| entities | Cave Spider | partial | Leaping and armadillo avoidance | **W2** Mob parity |
| entities | Creaking | partial | The reviewed source has changed. The previous assessment needs a new review. | **W2** Mob parity |
| entities | Creeper | partial | Spawn a potion-effect cloud after exploding | **W2** Mob parity |
| entities | Drowned | partial | Trident attacks and underwater-specific movement | **W2** Mob parity |
| entities | Elder Guardian | partial | The reviewed source has changed. The previous assessment needs a new review. | **W2** Mob parity |
| entities | Enderman | partial | The reviewed source has changed. The previous assessment needs a new review. | **W2** Mob parity |
| entities | Endermite | partial | The reviewed source has changed. The previous assessment needs a new review. | **W2** Mob parity |
| entities | Evoker | partial | Avoid nearby players and retaliate against attackers | **W2** Mob parity |
| entities | Guardian | partial | The reviewed source has changed. The previous assessment needs a new review. | **W2** Mob parity |
| entities | Hoglin | partial | Repellent avoidance and retreat behaviour | **W2** Mob parity |
| entities | Husk | partial | Hunger on attacks and submerged conversion to zombie | **W2** Mob parity |
| entities | Illusioner | partial | Bow attacks, blindness and invisibility spells | **W2** Mob parity |
| entities | Parched | partial | The reviewed source has changed. The previous assessment needs a new review. | **W2** Mob parity |
| entities | Piglin | partial | The reviewed source has changed. The previous assessment needs a new review. | **W2** Mob parity |
| entities | Piglin Brute | partial | Remember and return to the bastion home location | **W2** Mob parity |
| entities | Pillager | partial | Avoid creaking entities and retaliate against attackers | **W2** Mob parity |
| entities | Ravager | partial | Shield-stun and roar state/attack | **W2** Mob parity |
| entities | Skeleton | partial | The reviewed source has changed. The previous assessment needs a new review. | **W2** Mob parity |
| entities | Spider | partial | Leaping and armadillo avoidance | **W2** Mob parity |
| entities | Stray | partial | The reviewed source has changed. The previous assessment needs a new review. | **W2** Mob parity |
| entities | Vex | partial | Flying charge movement, owner target copying and limited lifetime | **W2** Mob parity |
| entities | Vindicator | partial | Johnny targeting and revenge targeting | **W2** Mob parity |
| entities | Warden | partial | Vibration/anger-driven targeting, sniffing and sonic boom | **W2** Mob parity |
| entities | Witch | partial | Select and heal injured raiders | **W2** Mob parity |
| entities | Wither Skeleton | partial | The reviewed source has changed. The previous assessment needs a new review. | **W2** Mob parity |
| entities | Zoglin | partial | Select living targets beyond players | **W2** Mob parity |
| entities | Zombie | partial | Baby speed and experience modifiers | **W2** Mob parity |
| entities | Zombie Villager | partial | Movement through villages | **W2** Mob parity |
| entities | Zombified Piglin | partial | Spear-specific attack behaviour | **W2** Mob parity |
| entities | Ender Dragon | partial | Register hit-testable parts and apply part-based contact/knockback checks | **W2** Mob parity |
| entities | Wither | partial | Autonomous primary-head target acquisition | **W2** Mob parity |
| entities | Allay | partial | Collect and deliver matching items, note-block following and flying movement | **W2** Mob parity |
| entities | Armadillo | partial | Trigger rolling from nearby threats | **W2** Mob parity |
| entities | Axolotl | partial | The reviewed source has changed. The previous assessment needs a new review. | **W2** Mob parity |
| entities | Bee | partial | Hive entry/exit, pollination and crop growth | **W2** Mob parity |
| entities | Camel | partial | Sitting/standing transitions and pose timing | **W2** Mob parity |
| entities | Camel Husk | partial | The reviewed source has changed. The previous assessment needs a new review. | **W2** Mob parity |
| entities | Cod | partial | The reviewed source has changed. The previous assessment needs a new review. | **W2** Mob parity |
| entities | Copper Golem | partial | Chest-to-chest item sorting and oxidation ticking | **W2** Mob parity |
| entities | Dolphin | partial | The reviewed source has changed. The previous assessment needs a new review. | **W2** Mob parity |
| entities | Donkey | partial | Untamed-riding taming cycle, chest inventory and ridden movement control | **W2** Mob parity |
| entities | Fox | partial | Pouncing/prey combat, sleeping and berry foraging | **W2** Mob parity |
| entities | Frog | partial | Tongue attack, breeding/egg laying and frog-specific jumping | **W2** Mob parity |
| entities | Goat | partial | Ramming, long jumps and return a milk bucket when milked | **W2** Mob parity |
| entities | Happy Ghast | partial | The reviewed source has changed. The previous assessment needs a new review. | **W2** Mob parity |
| entities | Horse | partial | Untamed-riding taming cycle and ridden movement control | **W2** Mob parity |
| entities | Iron Golem | partial | Village-defence targeting | **W2** Mob parity |
| entities | Llama | partial | Caravan following and functional chest inventory | **W2** Mob parity |
| entities | Mule | partial | Untamed-riding taming cycle, chest inventory and ridden movement control | **W2** Mob parity |
| entities | Nautilus | partial | The reviewed source has changed. The previous assessment needs a new review. | **W2** Mob parity |
| entities | Panda | partial | Gene-driven eating, rolling, sneezing and bamboo-gated breeding | **W2** Mob parity |
| entities | Parrot | partial | Taming, owner following, shoulder landing and flying movement | **W2** Mob parity |
| entities | Polar Bear | partial | Protect nearby cubs from players | **W2** Mob parity |
| entities | Rabbit | partial | Crop raiding and rabbit hopping control | **W2** Mob parity |
| entities | Salmon | partial | The reviewed source has changed. The previous assessment needs a new review. | **W2** Mob parity |
| entities | Skeleton Horse | partial | Skeleton trap activation and horse riding control | **W2** Mob parity |
| entities | Sniffer | partial | Automatic sniff/search/dig cycle | **W2** Mob parity |
| entities | Strider | partial | Lava seeking, lava-surface walking and cold-state updates | **W2** Mob parity |
| entities | Tadpole | partial | The reviewed source has changed. The previous assessment needs a new review. | **W2** Mob parity |
| entities | Trader Llama | partial | Caravan following and functional chest inventory | **W2** Mob parity |
| entities | Tropical Fish | partial | The reviewed source has changed. The previous assessment needs a new review. | **W2** Mob parity |
| entities | Turtle | partial | Egg laying, home return and long-distance swimming | **W2** Mob parity |
| entities | Wolf | partial | Stay seated when ordered and leap attacks | **W2** Mob parity |
| entities | Zombie Horse | partial | Horse riding control and vanilla horse-specific movement | **W2** Mob parity |
| entities | Zombie Nautilus | partial | The reviewed source has changed. The previous assessment needs a new review. | **W2** Mob parity |
| entities | Mannequin | partial | The reviewed source has changed. The previous assessment needs a new review. | **W2** Mob parity |
| entities | Villager | partial | Breeding and meeting/activity schedule parity | **W2** Mob parity |
| entities | Wandering Trader | partial | The reviewed source has changed. The previous assessment needs a new review. | **W2** Mob parity |
| entities | Dragon Fireball | partial | The reviewed source has changed. The previous assessment needs a new review. | **W6** Combat |
| entities | Experience Bottle | partial | Thrown bottle projectile and impact-triggered experience | **W6** Combat |
| entities | Fishing Bobber | partial | Fishing loot tables and their luck/enchantment context | **W6** Combat |
| entities | Acacia Boat | partial | Water-status/buoyancy physics | **W3** Entities/vehicles |
| entities | Acacia Chest Boat | partial | Water-status/buoyancy physics \| Chest inventory and container interaction | **W3** Entities/vehicles |
| entities | Bamboo Chest Raft | partial | Water-status/buoyancy physics \| Chest inventory and container interaction | **W3** Entities/vehicles |
| entities | Bamboo Raft | partial | Water-status/buoyancy physics | **W3** Entities/vehicles |
| entities | Birch Boat | partial | Water-status/buoyancy physics | **W3** Entities/vehicles |
| entities | Birch Chest Boat | partial | Water-status/buoyancy physics \| Chest inventory and container interaction | **W3** Entities/vehicles |
| entities | Cherry Boat | partial | Water-status/buoyancy physics | **W3** Entities/vehicles |
| entities | Cherry Chest Boat | partial | Water-status/buoyancy physics \| Chest inventory and container interaction | **W3** Entities/vehicles |
| entities | Command Block Minecart | partial | Command storage, editing and activator-rail execution | **W3** Entities/vehicles |
| entities | Dark Oak Boat | partial | Water-status/buoyancy physics | **W3** Entities/vehicles |
| entities | Dark Oak Chest Boat | partial | Water-status/buoyancy physics \| Chest inventory and container interaction | **W3** Entities/vehicles |
| entities | Jungle Boat | partial | Water-status/buoyancy physics | **W3** Entities/vehicles |
| entities | Jungle Chest Boat | partial | Water-status/buoyancy physics \| Chest inventory and container interaction | **W3** Entities/vehicles |
| entities | Mangrove Boat | partial | Water-status/buoyancy physics | **W3** Entities/vehicles |
| entities | Mangrove Chest Boat | partial | Water-status/buoyancy physics \| Chest inventory and container interaction | **W3** Entities/vehicles |
| entities | Oak Boat | partial | Water-status/buoyancy physics | **W3** Entities/vehicles |
| entities | Oak Chest Boat | partial | Water-status/buoyancy physics \| Chest inventory and container interaction | **W3** Entities/vehicles |
| entities | Pale Oak Boat | partial | Water-status/buoyancy physics | **W3** Entities/vehicles |
| entities | Pale Oak Chest Boat | partial | Water-status/buoyancy physics \| Chest inventory and container interaction | **W3** Entities/vehicles |
| entities | Poplar Boat | partial | Water-status/buoyancy physics | **W3** Entities/vehicles |
| entities | Poplar Chest Boat | partial | Water-status/buoyancy physics \| Chest inventory and container interaction | **W3** Entities/vehicles |
| entities | Spawner Minecart | partial | Spawner state, delay and entity spawning | **W3** Entities/vehicles |
| entities | Spruce Boat | partial | Water-status/buoyancy physics | **W3** Entities/vehicles |
| entities | Spruce Chest Boat | partial | Water-status/buoyancy physics \| Chest inventory and container interaction | **W3** Entities/vehicles |
| entities | Armor Stand | partial | Equipment drops and fire-specific damage handling | **W3** Entities/vehicles |
| entities | Cushion | partial | The reviewed source has changed. The previous assessment needs a new review. | **W3** Entities/vehicles |
| entities | Painting | partial | The reviewed source has changed. The previous assessment needs a new review. | **W3** Entities/vehicles |
| entities | Ominous Item Spawner | partial | The reviewed source has changed. The previous assessment needs a new review. | **W2** Mob parity |
| blocks | Beds | partial | Wake a villager occupying the bed | **W4** Blocks/redstone |
| blocks | Beehive & Bee Nest | partial | Bee entry/release and ticking hive lifecycle | **W4** Blocks/redstone |
| blocks | Campfire | partial | Ignition from burning projectile impact | **W4** Blocks/redstone |
| blocks | Chests | partial | The reviewed source has changed. The previous assessment needs a new review. | **W4** Blocks/redstone |
| blocks | Chiseled Bookshelf | partial | Increment the used-block statistic on inserting a book | **W4** Blocks/redstone |
| blocks | Conduit | partial | The reviewed source has changed. The previous assessment needs a new review. | **W4** Blocks/redstone |
| blocks | Dirt Path | partial | Displace entities when converting and allow fence gates above | **W4** Blocks/redstone |
| blocks | Dragon Egg | partial | The reviewed source has changed. The previous assessment needs a new review. | **W4** Blocks/redstone |
| blocks | Ender Chest | partial | Piglin guarded-block reaction | **W4** Blocks/redstone |
| blocks | Farmland | partial | The reviewed source has changed. The previous assessment needs a new review. | **W4** Blocks/redstone |
| blocks | Frogspawn | planned | Placeable, but never hatches into tadpoles. | **W4** Blocks/redstone |
| blocks | Infested Blocks | partial | Spawn the specialised silverfish AI and respect Silk Touch | **W4** Blocks/redstone |
| blocks | Jukebox | partial | Block-change game events and comparator update on replacement | **W4** Blocks/redstone |
| blocks | Lodestone | planned | Compass binding is not implemented. | **W4** Blocks/redstone |
| blocks | Note Block | partial | The reviewed source has changed. The previous assessment needs a new review. | **W4** Blocks/redstone |
| blocks | Shulker Box | partial | Prevent reopening during closing animation | **W4** Blocks/redstone |
| blocks | Fire | partial | Environment-attribute driven increased fire burnout | **W4** Blocks/redstone |
| blocks | Sculk Catalyst | partial | Mob-death listener, experience absorption and sculk spreading | **W4** Blocks/redstone |
| blocks | Moving Piston | partial | Pass the breaking player through to base removal | **W4** Blocks/redstone |
| blocks | Piston Head | partial | Pass the breaking player through to base removal | **W4** Blocks/redstone |
| blocks | Pistons | partial | Complete movement checks and moving-piston state persistence | **W4** Blocks/redstone |
| blocks | Big Dripleaf | partial | Projectile-triggered tilt | **W4** Blocks/redstone |
| blocks | Lily Pad | partial | Fluid-state support and raft collision breakage | **W4** Blocks/redstone |
| blocks | Pumpkin & Melon Stems | partial | The reviewed source has changed. The previous assessment needs a new review. | **W4** Blocks/redstone |
| blocks | Sea Pickles | partial | The reviewed source has changed. The previous assessment needs a new review. | **W4** Blocks/redstone |
| blocks | Sugar Cane | partial | Fluid-aware adjacent water support | **W4** Blocks/redstone |
| blocks | Activator Rail | partial | Vanilla rail power-extension boundary | **W4** Blocks/redstone |
| blocks | Bell | partial | Activation game events | **W4** Blocks/redstone |
| blocks | Daylight Detector | partial | The reviewed source has changed. The previous assessment needs a new review. | **W4** Blocks/redstone |
| blocks | Dispenser | partial | Bonemeal, experience bottles, llama chests, armadillo brushing and bucket-contained entity spawning | **W4** Blocks/redstone |
| blocks | Powered Rail | partial | Vanilla rail power-extension boundary | **W4** Blocks/redstone |
| blocks | Tripwire | partial | Shears durability/game event and trap-avoidance filtering | **W4** Blocks/redstone |
| blocks | Tripwire Hook | partial | Activation/deactivation/attachment game events | **W4** Blocks/redstone |
| items | Fishing Rod | partial | Fishing loot tables and their luck/enchantment context | **W5** Items/inventory/loot |
| items | Mace | partial | The reviewed source has changed. The previous assessment needs a new review. | **W5** Items/inventory/loot |
| items | Elytra | partial | The reviewed source has changed. The previous assessment needs a new review. | **W5** Items/inventory/loot |
| items | Name Tag | partial | The reviewed source has changed. The previous assessment needs a new review. | **W5** Items/inventory/loot |
| commands | /jfr | planned | No dedicated implementation source identified. This does not establish that the feature is missing. | **W14** Commands/data packs |
| commands | /locate | partial | The reviewed source has changed. The previous assessment needs a new review. | **W14** Commands/data packs |
| redstone | Daylight detector | partial | The reviewed source has changed. The previous assessment needs a new review. | **W4** Blocks/redstone |
| redstone | Note block | partial | The reviewed source has changed. The previous assessment needs a new review. | **W4** Blocks/redstone |
| redstone | Pistons and block swapping | partial | Complete movement checks and moving-piston state persistence | **W4** Blocks/redstone |
| combat | 1.8 combat mode | planned | No dedicated implementation source identified. This does not establish that the feature is missing. | **W6** Combat |
| world | World loading (level.dat, DIM1, DIM-1) | partial | The reviewed source has changed. The previous assessment needs a new review. | **W9** Persistence/chunks |
| world | World saving | partial | The reviewed source has changed. The previous assessment needs a new review. | **W9** Persistence/chunks |
| world | Redstone | partial | Complete piston movement and moved-block persistence | **W10** World simulation |

---

## Appendix C — Open pull requests in flight (313)

Do not duplicate these. Check the PR before starting a task; if it is stale, comment and ask the author. This list is a snapshot and will drift.

| PR | Title | Author | Draft |
| ---: | :--- | :--- | :--- |
| #2482 | perf(protocol): buffer encrypted writes | @JamieLittle16 | no |
| #2489 | perf(protocol): use zlib-rs for Java compression | @JamieLittle16 | no |
| #2504 | fix(data): don't panic when the data directory can't be created | @Sawlyer | no |
| #2506 | perf(world): optimize chunk noise generation and buffer allocations | @JamieLittle16 | no |
| #2792 | perf(protocol): add typed prepared packet representations | @JamieLittle16 | no |
| #2793 | fix: resolve placement modifiers per position | @xRookieFight | no |
| #2795 | fix: untamed wolves attack their prey | @xRookieFight | no |
| #2828 | feat(plugin): native cdylib plugin loading via native-wit | @BjornTheProgrammer | yes |
| #2858 | feat(build): decrease incremental build times by ~1.5x | @BjornTheProgrammer | no |
| #2958 | feat(pumpkin): add /perf command | @Mcxiaocaibug | no |
| #2985 | fix(config): don't overwrite the config file when a field has the wrong type | @Lstarsky0 | no |
| #2995 | fix(piston): stop block swapping from corrupting pistons | @Mcxiaocaibug | no |
| #3017 | feat(block): add farmland trampling | @Simlesk | no |
| #3041 | fix(worldgen): generate both halves of double-height plants | @Love0118 | no |
| #3057 | feat!: Correct Piston behaviour | @zinedinp | yes |
| #3075 | feat(world): safe spawn position check for player join and respawn | @Q2297045667 | no |
| #3079 | fix(block): send trail particle options when eyeblossoms change state | @catdum200 | no |
| #3135 | fix(block): restore lever's moved guard and normal_use result | @pxperr | no |
| #3145 | fix(net): honor movement lock for vehicle movement | @Srinivasan-78 | no |
| #3149 | Fix VehicleMoveEvent not triggering and canceling | @Neol3108 | no |
| #3179 | perf(worldgen): prune biome distance checks early | @nobuildersnotools | no |
| #3216 | feat(plugin): fire BlockFormEvent for lava conversions | @pxperr | no |
| #3221 | fix(entity): play death sound on lethal hit and send Poof when removing a corpse | @xudoyberdi0410 | no |
| #3231 | chore(ci): add ferrflow for automated versioning | @BryanFRD | no |
| #3237 | fix(loot): evaluate entity property conditions (#3225)  | @BatuhanAksoyy | no |
| #3238 | feat(world): lighting engine flood, shape occlusion, and sky cut height | @zinedinp | no |
| #3243 | fix: release disconnected player ownership cycles | @nobuildersnotools | no |
| #3244 | fix: retire completed Bedrock chunk blob payloads | @nobuildersnotools | no |
| #3245 | fix: release closed and abandoned NetherNet sessions | @nobuildersnotools | no |
| #3257 | feat(java): add HAProxy PROXY v2 support | @HookWoods | no |
| #3260 | chore(ci): fix the android(aarch64) test workflow | @W9pi3cZ1 | no |
| #3275 | fix(entity): give all living entities an air supply | @CaptainParis | yes |
| #3277 | fix: prevent badland mineshafts from generating above ground | @YTBGG | no |
| #3307 | Fire PlayerDropItemEvent when dropping from GUI | @vyPal | no |
| #3320 | perf(world): avoid repeated scans when packing dense palettes | @JulesB40 | no |
| #3335 | perf(worldgen): compute concurrent structure cache misses once | @JulesB40 | no |
| #3348 | fix: preserve goat horn instrument components | @JulesB40 | no |
| #3349 | fix(entity): sync item push velocity and force position updates when pushed 🤖🤖🤖 | @IsaiahJaiden | no |
| #3351 | fix: avoid tool break effects when placing the last block | @JulesB40 | no |
| #3352 | fix(entity): panic goals only trigger on vanilla panic damage causes 🤖🤖🤖 | @IsaiahJaiden | no |
| #3353 | fix(entity): keep fish wander targets in water 🤖🤖🤖 | @IsaiahJaiden | no |
| #3354 | feat: support packet events across all connection states | @ErrorsErrors | no |
| #3355 | feat(plugin,QOL): plugin show information, reload, and tab-auto-completion | @zinedinp | no |
| #3373 | feat(config): validate listener ports and addresses before startup | @MegalithOfficial | no |
| #3394 | fix: defer datapack load functions until the next game tick | @JulesB40 | no |
| #3395 | fix: preserve player equipment on death | @JulesB40 | no |
| #3407 | fix(dragon-parts): make the ender dragon hittable and stop it despawning | @zinedinp | yes |
| #3410 | fix(egg): keep the Pterodactyl allocation port in sync on every start | @Laggy60 | no |
| #3412 | fix(brewing): follow screen slot rules and fire the start event | @Laggy60 | no |
| #3414 | fix(entity): keep armor stand name and let projectiles break it | @Laggy60 | no |
| #3415 | fix(world): drop blocks that lose support after a creative break | @Laggy60 | no |
| #3416 | fix(block): deal stalagmite damage when landing on pointed dripstone | @Laggy60 | no |
| #3417 | fix(entity): drop falling blocks as items when they can't land | @Laggy60 | no |
| #3418 | fix(loot): honour block state conditions in generated loot tables | @Laggy60 | no |
| #3420 | fix(config): make configuration load failures visible before the logger is ready 🤖🤖🤖 | @Developers13 | no |
| #3421 | fix(entity): make sheep sync color in read_nbt | @orpos | no |
| #3422 | fix(entity): make chicken sync variant in read_nbt | @orpos | no |
| #3424 | perf: avoid allocations when matching recipe ingredients | @nobuildersnotools | no |
| #3425 | perf: stream nearest entity searches | @nobuildersnotools | no |
| #3427 | perf(bedrock): Reuse indexed palettes for Bedrock chunk packets | @nobuildersnotools | no |
| #3428 | fix: project village paths onto terrain | @enderliker | no |
| #3432 | fix(player): preserve visibility and equipment across dimensions | @JulesB40 | no |
| #3433 | Prevent datapack loaders from following symlinks | @potatosips | no |
| #3438 | fix(worldgen): gate freeze-top-layer snow on face support and light | @Kurenaiiiii | no |
| #3441 | chore(deps): bump actions/github-script from 8 to 9 | @dependabot[bot] | no |
| #3442 | chore(deps): bump DeterminateSystems/magic-nix-cache-action from 14 to 15 | @dependabot[bot] | no |
| #3443 | chore(deps): bump signpath/github-action-submit-signing-request from 2 to 3 | @dependabot[bot] | no |
| #3444 | chore(deps): bump DeterminateSystems/nix-installer-action from 22 to 23 | @dependabot[bot] | no |
| #3447 | fix(world): preserve Paper overworld settings on import | @younghyun1 | no |
| #3458 | fix(entity): use the per-mob panic damage tags for wolves, pandas, armadillos and polar bears 🤖🤖🤖 | @Developers13 | no |
| #3459 | fix(elytra): vanilla-parity fixes for gliding and flight state | @darkzmaj | no |
| #3460 | fix(item-drops): apply air drag once in grounded item friction | @darkzmaj | no |
| #3461 | fix(setworldspawn): actually persist the new spawn point to level.dat | @darkzmaj | no |
| #3462 | fix(firework-rocket): render the rocket and always allow launching | @darkzmaj | no |
| #3463 | fix(protocol): fix stream desync and lost text for named items in bundles | @darkzmaj | no |
| #3465 | Guard stale block-entity property decoding (split 2/2 of #3011) | @darkzmaj | no |
| #3466 | Fix `wrong teleport id` | @Neol3108 | no |
| #3467 | perf(bedrock): Don't send all-air sub-chunks in level chunk packet for bedrock | @zinedinp | no |
| #3473 | fix: avoid duplicate experience packets | @HP-network | no |
| #3475 | fix(worldborder): select center block for collapsed clamp ranges 🤖🤖🤖 | @Thijs226 | no |
| #3476 | fix(entity): stop sending a player its own stale motion on every push 🤖🤖🤖 | @Developers13 | no |
| #3479 | Fix/falling block partial landing 3153🤖🤖🤖 | @Thijs226 | no |
| #3485 | Fix SIGINT | @Neol3108 | no |
| #3493 | feat(scheduler): add cancellation and completion tracking to scheduler | @Phoenixxo | no |
| #3497 | Add `spawn_falling_block` API | @Neol3108 | no |
| #3504 | fix(server): enforce Hard difficulty at startup for hardcore worlds | @Kurenaiiiii | no |
| #3505 | fix(plugin-wit): import all host interfaces🤖🤖🤖 | @Thijs226 | no |
| #3506 | fix(player): spectate after hardcore death🤖🤖🤖 | @Thijs226 | no |
| #3507 | fix(loot): preserve block entity components in drops | @younghyun1 | yes |
| #3508 | Fix WASM command node requirement handlers 🤖🤖🤖 | @Thijs226 | no |
| #3509 | fix(block): encode eyeblossom trail particle data 🤖🤖🤖 | @Thijs226 | no |
| #3523 | fix(block): prevent empty respawn anchor explosions🤖🤖🤖 | @Thijs226 | no |
| #3524 | Fire `EntityChangeBlockEvent` when falling block lands | @Neol3108 | no |
| #3525 | fix(entity): pass attacker position so shields can block melee attacks | @YuEfSaEDU | no |
| #3543 | fix(inventory): respect Curse of Binding on armor removal 🤖🤖🤖 | @Thijs226 | no |
| #3544 | fix(combat): pass attacker position for shield blocking 🤖🤖🤖 | @Thijs226 | no |
| #3546 | fix(block): block chests under sitting cats 🤖🤖🤖 | @Thijs226 | no |
| #3547 | fix(explosion): ignite bed and respawn anchor blasts 🤖🤖🤖 | @Thijs226 | no |
| #3549 | fix(jukebox): restore playback state after reload | @HP-network | no |
| #3553 | fix(world): pause scheduled ticks outside active chunks | @HP-network | no |
| #3554 | feat(plugin-api): expose native chunk loading 🤖🤖🤖 | @Thijs226 | no |
| #3555 | fix(block): relight campfires hit by burning projectiles | @HP-network | no |
| #3556 | fix(block): collapse big dripleaves hit by projectiles | @HP-network | no |
| #3559 | fix(block): allow lily pads on waterlogged blocks | @HP-network | no |
| #3562 | fix: preserve farmland and dirt paths under fence gates | @HP-network | no |
| #3582 | feat(villager): sleeping, breeding and food pickup | @MidnightDoes | yes |
| #3584 | fix(tripwire): damage shears when disarming tripwire | @HP-network | no |
| #3585 | fix(block): move entities when farmland turns to dirt | @HP-network | no |
| #3586 | fix(enderman): validate carried block placement | @HP-network | no |
| #3592 | fix(block/sugar_cane): allow placement next to water, waterlogged blocks | @AdmerPRO | no |
| #3594 | feat(data): validate operator configuration | @AdmerPRO | no |
| #3612 | feat(warden): add anger management | @MegalithOfficial | no |
| #3630 | feat(entity): move piglins and piglin brutes onto the brain | @wiflow | no |
| #3632 | fix(tripwire): emit state change game events 🤖🤖🤖 | @Thijs226 | no |
| #3634 | fix(world): harden player and level data saves 🤖🤖🤖 | @CocofireHD | no |
| #3636 | fix(block): respawn anchor, campfire, waxing and infested fixes 🤖🤖🤖 | @CocofireHD | no |
| #3637 | fix(entity): melee shield blocking and suspicious block breaking 🤖🤖🤖 | @CocofireHD | no |
| #3638 | fix(server): force hard difficulty on hardcore worlds 🤖🤖🤖 | @CocofireHD | no |
| #3639 | fix(world): only tick scheduled ticks in active chunks 🤖🤖🤖 | @CocofireHD | no |
| #3640 | fix(redstone): tripwire shears and dispenser bone meal 🤖🤖🤖 | @CocofireHD | no |
| #3641 | fix(inventory): lock curse of binding armor outside creative 🤖🤖🤖 | @CocofireHD | no |
| #3642 | fix(command): order gamemode feedback args per edition 🤖🤖🤖 | @CocofireHD | no |
| #3644 | fix(entity): float experience orbs in water 🤖🤖🤖 | @Thijs226 | no |
| #3645 | feat(block): dispense experience bottles as projectiles | @AdmerPRO | no |
| #3649 | fix(entity): use water-bound navigation for aquatic mobs 🤖🤖🤖 | @Thijs226 | no |
| #3655 | fix(entity): drop a painting item when a painting breaks 🤖🤖🤖 | @CocofireHD | no |
| #3656 | fix(block): emit container_open/container_close game events | @creeperkatze | no |
| #3657 | feat(elytra): port vanilla gliding | @MegalithOfficial | no |
| #3658 | fix(block): emit block_change game event on chiseled bookshelf book changes | @creeperkatze | no |
| #3660 | fix(block): emit block_change game event on jukebox insert/eject | @creeperkatze | no |
| #3661 | fix(entity): register ender dragon hitbox parts for entity id lookup | @creeperkatze | no |
| #3662 | chore(deps): bump crate-ci/typos from 1.50.1 to 1.50.2 | @dependabot[bot] | no |
| #3663 | fix(entity): spawn lingering cloud for affected creepers 🤖🤖🤖 | @Thijs226 | no |
| #3664 | fix(docker): build published images from matching release 🤖🤖🤖 | @Thijs226 | no |
| #3665 | feat(block): allow pressure plates placement on fences and fence gates | @AdmerPRO | no |
| #3666 | fix(item_stack): reject damaged items from stacking 🤖🤖🤖 | @Thijs226 | no |
| #3669 | fix(block): notify clients when fire extinguishes 🤖🤖🤖 | @Thijs226 | no |
| #3673 | fix(block): make bells reveal nearby raiders 🤖🤖🤖 | @Thijs226 | no |
| #3681 | fix(entity): sync falling block positions each tick 🤖🤖🤖 | @Thijs226 | no |
| #3682 | fix(entity): restore swamp slime spawning 🤖🤖🤖 | @Thijs226 | no |
| #3684 | fix(entity): enable armor stand item interactions 🤖🤖🤖 | @Thijs226 | no |
| #3686 | Emit game events on container open and close, fix | @cfsmi | no |
| #3687 | fix(plugin): poll native methods on global runtime 🤖🤖🤖 | @Thijs226 | no |
| #3693 | fix(block): wake villagers from occupied beds 🤖🤖🤖 | @Thijs226 | no |
| #3694 | fix: correct end crystal combat behavior 🤖🤖🤖 | @Thijs226 | no |
| #3695 | fix: make dispensers apply bone meal 🤖🤖🤖 | @Thijs226 | no |
| #3697 | feat(firework): port firework rocket flight, explosions and crossbow launching | @MegalithOfficial | no |
| #3706 | fix(protocol): reject item stack wire values that do not fit | @wiflow | no |
| #3707 | fix(world): keep the chunk tags this server does not model | @Lstarsky0 | no |
| #3708 | fix(join): send player info before entity tracking and preserve chunks when sends are cancelled | @zinedinp | no |
| #3715 | fix(entity): implement phantom circle and swoop attack AI | @Kurenaiiiii | no |
| #3717 | fix(entity): show custom names loaded from NBT to Java clients | @gitKBDL | no |
| #3718 | fix(entity): swim fish and squid like vanilla instead of like land mobs | @gitKBDL | no |
| #3719 | feat(plugin): read sections in bulk, and set and resend biomes | @gitKBDL | no |
| #3720 | feat(plugin): read, write and spawn entities from NBT | @gitKBDL | no |
| #3721 | feat(plugin): load chunks on demand | @gitKBDL | no |
| #3725 | fix(loot): fix loot table conditions | @MidnightDoes | yes |
| #3728 | fix(item): stop components with no NBT form from truncating chunk data | @KernelKraze | no |
| #3729 | fix(block): make shelf mushrooms work | @MidnightDoes | yes |
| #3739 | fix(world): scope sound events to players in range | @BryanFRD | no |
| #3741 | feat(data): Add block bounce restitution and fall distance reduction. | @MidnightDoes | no |
| #3744 | fix(entity): validate spectator camera targets like vanilla | @Q2297045667 | no |
| #3745 | fix(plugin-utils): allow wasm builds 🤖🤖🤖 | @Rennex07 | no |
| #3755 | feat(plugin): add event registration w/ handler and unregistration | @Phoenixxo | no |
| #3763 | fix(command): avoid sending tick sprint report before completion | @AdmerPRO | no |
| #3765 | fix(villager): correct trade availability and prevent trading deadlock | @AdmerPRO | no |
| #3771 | feat(server): decouple server lifecycle from the process for embedding | @zinedinp | no |
| #3774 | fix(villager): stop sleeping villagers from walking | @salvoeed | no |
| #3775 | fix(entity): send the painting variant as registry id plus one 🤖🤖🤖 | @xef5000 | no |
| #3776 | fix(entity): validate item frame placement and support 🤖🤖🤖 | @xef5000 | no |
| #3778 | feat: vanilla world types for new worlds, and decorated flat worlds | @GFoniX | no |
| #3788 | fix(server): stop the stdin console task when the server stops | @UMCEKO | no |
| #3790 | fix(entity): let blazes ascend toward higher targets | @Volubles | no |
| #3792 | fix(world): handle queued player packets before scheduled ticks | @4d1cksupmya55-source | no |
| #3793 | fix(entity): vanilla player touch area and push velocity sync | @zinedinp | no |
| #3794 | fix(entity): velocity sync and vanilla minecart velocity transfer | @zinedinp | yes |
| #3795 |   fix(entity): sulfur cube AI and cube mob size sync | @treeot | no |
| #3796 | feat(logo): add Pumpkin ANSI art | @xeidral | no |
| #3800 | fix(entity): persist leashes and destroy a leash knot on hit | @meaningg | no |
| #3804 | fix(hopper): pick up whole item stacks, including from the bowl | @4d1cksupmya55-source | no |
| #3807 | fix(command): apply /data block changes to the live block entity | @4d1cksupmya55-source | no |
| #3808 | fix(entity): prevent mobs burning under opaque blocks | @Alerp40 | no |
| #3810 | feat(item): place scaffolding like vanilla | @tom-devv | no |
| #3811 | fix(protocol): preserve custom item name formatting 🤖🤖🤖 | @zynthic-au | no |
| #3813 | fix(command): bound nested function chain depth to stop stack overflow | @Kurenaiiiii | no |
| #3815 | fix(entity): clear enderman aggression when its target dies | @AdmerPRO | no |
| #3816 | fix(entity): damage water-sensitive mobs with thrown water potions | @meaningg | no |
| #3819 | chore(multiversion): remove specific multiversion code and only have one java version with open plugin calls | @zinedinp | no |
| #3820 | fix(player): stop the Bedrock skin download from blocking the join | @BellionBastien | no |
| #3821 | feat(entity): implement the fishing bobber | @towner-10 | no |
| #3826 | fix(block): keep ice from melting when broken in creative | @luisakrivonogih | no |
| #3827 | fix(command): parse block state properties in /setblock | @luisakrivonogih | no |
| #3829 | fix(entity): add Shearable trait and fix shearing | @tom-devv | no |
| #3832 | feat(config): support file-based Velocity forwarding secrets | @Mcxiaocaibug | no |
| #3835 | fix(projectile): vanilla fireball flight, player collisions and leftOwner state machine | @LukeKappa | no |
| #3843 | fix(block): avoid double press sound on buttons | @meaningg | no |
| #3845 |  fix: entity motion, forced chunks, attributes and combat knockback | @rraattrruuee | no |
| #3846 | Slovenian translation | @Mihec708 | no |
| #3848 | perf(world): chunk memory optimisation(s) and benchmarks | @xnacly | no |
| #3849 | fix(protocol): keep item changes from block interactions | @Kron5620 | no |
| #3853 | fix(entity): apply hurt cooldown to void damage | @Kron5620 | no |
| #3854 | fix(util): stop extra %s placeholders repeating the last arg | @Zack-XS | no |
| #3859 | fix(world): remove block entity NBT when the block is removed | @ydw1904 | no |
| #3860 | feat(beacon): port vanilla beacon behaviour | @ydw1904 | no |
| #3861 | fix(spawn): use correct vanilla spawn weights for spawn entries | @ydw1904 | no |
| #3863 | fix(redstone): observers actually match vanilla now | @ydw1904 | no |
| #3865 | fix(entity): drop nether star only on death | @ydw1904 | no |
| #3868 | fix(block): remove unsupported snow during neighbor updates | @Kron5620 | no |
| #3869 | fix(block): preserve fire age on neighbor updates | @Kron5620 | no |
| #3871 | fix(entity): correct cushion placement and support removal | @Kron5620 | no |
| #3875 | fix(block): preserve block entity data when placing items | @Kron5620 | no |
| #3876 | fix(item): correct mace smash damage and effects | @Kron5620 | no |
| #3886 | feat(entity): complete the silverfish goal set | @turntabl3 | no |
| #3887 | feat: support dynamically adding registries | @EricApostal | no |
| #3894 | fix(data): parse SNBT custom model data numbers | @ToffyMTA | no |
| #3895 | fix(plugin): point block ignite events at the ignited block and carry… | @meaningg | no |
| #3896 | fix(item): give bundles vanilla's click, use and sync behaviour | @turntabl3 | no |
| #3897 | fix(codec): improve serialization and deserialization of charged projectiles | @ToffyMTA | no |
| #3901 | fix(worldgen): cache template pool max size like vanilla | @SepehrRajabi | no |
| #3903 | fix(worldgen): resolve named block state providers 🤖🤖🤖 | @Szabolcs05 | no |
| #3904 | fix(player): wake sleeping players before applying damage 🤖🤖🤖 | @Szabolcs05 | no |
| #3905 | fix(entity): restore wind charge launches and burst effects 🤖🤖🤖 | @Szabolcs05 | no |
| #3906 | fix(command): match resource tag suggestions like vanilla | @ydw1904 | no |
| #3911 | fix(protocol): bound serverbound collection lengths before allocating | @WhiteProject1 | no |
| #3913 | fix(query): derive the challenge token and handle datagrams inline | @WhiteProject1 | no |
| #3914 | fix(wasm): verify plugin signatures against the trusted key only | @WhiteProject1 | no |
| #3915 | fix(proxy): compare the forwarding secret in constant time and gate forwarding on proxy.enabled | @WhiteProject1 | no |
| #3919 | fix(gametest): Update GameTest structure version handling | @KairuDeibisu | no |
| #3920 | fix(block): spawn the stored mob when a dispenser empties a mob bucket | @BarilTomasDev | no |
| #3922 | fix(inventory): cap recipe book placement at max stack size | @Pixeluted | no |
| #3925 | fix(plugin): correct BlockFromTo positions and fluid type | @meaningg | no |
| #3927 | feat(api): add leash APIs to Wasm host v0.2 | @illyrius666 | no |
| #3931 | fix(block): allow dirt path and farmland under fence gates | @saranxzi | no |
| #3936 | feat(plugin): add world.drop-item API | @katze225 | no |
| #3940 | fix(item): give the mob bucket when catching a mob with a water bucket | @BarilTomasDev | no |
| #3951 | fix(block): place banners on walls | @AdmerPRO | no |
| #3955 | perf(pathfinder): reuse paths, share block caches, limit searches | @callowaysutton | no |
| #3961 | feat(entity): implement chest boat inventory | @xlash123 | no |
| #3962 | feat(plugin): add async v0.2 guest bindings | @Phoenixxo | yes |
| #3965 | fix(block): make hoppers respect double chests | @Mirkrog | no |
| #3975 | fix(block): make jukebox remove music disc from the player's hand if the player is not in creative mode | @Slacker201 | no |
| #3976 | fix(item): return an empty bucket after placing powder snow | @SplashGG | yes |
| #3978 | fix(entity): cap fall distance while gliding | @SplashGG | no |
| #3979 | fix(plugin): load healthy plugins when one has an unresolvable depend… | @Rennex07 | no |
| #3984 | fix(entity): feline goal parity for cats and ocelots | @Developers13 | no |
| #3986 | fix(block): fixed powder snow only being visible after rejoin when placed by dispenser | @Mirkrog | no |
| #3987 | fix(command): support execute store score | @afalinx | yes |
| #3988 | feat(entity): implement bogged mechanics and skeleton parity improvements     | @denys-shatin | no |
| #3990 | fix(block): dispenser now correctly replaces replacable block infront with bucket content | @Mirkrog | no |
| #3991 | fix(auth): escape the username in authentication URLs | @WhiteProject1 | no |
| #3992 | fix(protocol): grow the payload buffer as bytes arrive instead of reserving the declared length | @WhiteProject1 | no |
| #3993 | fix(world): cap how much a chunk may decompress to | @WhiteProject1 | no |
| #3995 | fix(server): keep a Rayon worker panic from aborting the process | @WhiteProject1 | no |
| #3996 | fix(net): validate client supplied positions in move vehicle and bedrock input | @WhiteProject1 | no |
| #3999 | fix(protocol): bound how deeply item stack templates may nest | @WhiteProject1 | yes |
| #4000 | fix(command): cut the error context on character boundaries | @WhiteProject1 | yes |
| #4001 | fix(bedrock): cap a reassembled NetherNet packet at the maximum packet size | @WhiteProject1 | yes |
| #4002 | fix(block): use vault properties when placing a vault | @RealKai295 | no |
| #4003 | fix(command): bound how deeply SNBT may nest | @WhiteProject1 | yes |
| #4007 | fix(block): pistons facing down now don't extend when powered from below | @Mirkrog | no |
| #4010 | feat(world): support scheduled tick delays longer than 255 ticks | @alastairWH | no |
| #4015 | fix(block): allow double chest extension from diagonal placement | @ethantena | no |
| #4019 | fix(block): breaking piston heads now functions correctly | @Mirkrog | no |
| #4022 | fix(block): bubble columns | @George7231 | no |
| #4026 | fix(piston): make sticky piston retracts match vanilla | @TheDarkSword | no |
| #4029 | fix(world): compute located structure starts the way chunk generation does | @WhiteProject1 | yes |
| #4030 | fix(entity): let fish swim in water instead of walking like land mobs | @WhiteProject1 | yes |
| #4031 | fix(world): keep in-place region writes from overwriting the next chunk | @WhiteProject1 | yes |
| #4039 | fix(entity): give drowned their vanilla swimming AI | @WhiteProject1 | yes |
| #4042 | fix(entity): keep the air supply of a dead player from draining before respawn | @WhiteProject1 | yes |
| #4047 | fix(piston): stop moving pistons from desyncing clients and losing blocks | @TheDarkSword | no |
| #4050 | fix(entity): add pufferfish puff and sting behavior | @ultard | no |
| #4053 | perf(entity): stop preallocating every mob's pathfinding heap | @Conman1533 | no |
| #4054 | fix(dropped-items): stack items on contact and hopper race condition and pre-merge explosion drops | @zinedinp | no |
| #4055 | fix(protocol): bound how deeply block predicates may nest | @WhiteProject1 | yes |
| #4056 | fix(bedrock): bounds-check hotbar slot ids in item stack requests | @WhiteProject1 | yes |
| #4057 | fix(inventory): refill the crafting result when it is taken | @WhiteProject1 | yes |
| #4058 | fix(net): validate finished digging before breaking the block | @WhiteProject1 | yes |
| #4059 | fix(item): write the bucket result back to the hand that used it | @WhiteProject1 | yes |
| #4060 | fix(server): make the duplicate-login and max-players checks atomic with the join | @WhiteProject1 | yes |
| #4061 | fix(bedrock): check bans and the whitelist on Bedrock login | @WhiteProject1 | no |
| #4066 | fix(command): cap /give at 100 stacks like vanilla | @WhiteProject1 | yes |
| #4067 | fix(bedrock): spawn a player only on its first completed pack response | @WhiteProject1 | yes |
| #4068 | fix(inventory): close block containers that are no longer in reach | @WhiteProject1 | yes |
| #4069 | fix(command): require permission level 2 for entity selectors | @WhiteProject1 | yes |
| #4072 | feat(profiler): add memory and CPU profiler | @MegalithOfficial | no |
| #4073 | Feat/boat water physics 26.3 | @BoredHF | no |
| #4080 | fix(entity): enforce team friendly fire in combat | @ultard | no |
| #4083 | feat(plugin): add bukkit-style config file helpers | @asimplestray | no |
| #4086 | fix(entity): hold weak self references in mob goals | @SplashGG | no |
| #4092 | refactor(entity): rework the mob AI foundation | @MegalithOfficial | no |
| #4093 | feat(entity): implement vanilla fox mechanics and AI behaviors | @junioraww | no |
| #4095 | fix(entity): port vanilla hoglin throwTarget for hoglins and zoglins | @pujitha24 | no |
| #4096 | feat(plugin): add comprehensive wasm plugin errors | @Phoenixxo | no |
| #4097 | fix(player): center sleeping position, skip in-wall damage while asleep | @SplashGG | no |
| #4098 | feat(plugin): load v0.1 plugins on a sequential engine and move to wasmtime 49.0.3 | @Phoenixxo | no |
| #4099 | fix(entity): stop the server from sending critical arrow particles | @MDZZDYXCMDZZDYXC | no |
| #4101 | fix(protocol): encode armor trims as registry references | @SplashGG | no |
| #4104 | fix(player): correctly handle off-hand item pickup | @0neRain | no |
| #4105 | fix(entity): mob_drops gamerule | @SplashGG | no |
| #4106 | perf(world): cache light level in propagation queue entry | @enderliker | no |
| #4107 | fix(Mob AI): Temperament enum, data-driven hunt/flee tables, vanilla target search | @zinedinp | yes |
| #4110 | fix(map): point the player map icon the way the player faces | @ida64 | no |
| #4111 | fix(server): ignore empty commands from the console | @gringoestrangeiro | no |
| #4112 | fix(world): apply explosion knockback resistance to explosion knockback | @TrygveDev | no |
| #4113 | fix(entity): read blaze follow range from the FOLLOW_RANGE attribute | @Rennex07 | no |
| #4114 | fix(fluid): decide what fluids replace with the washed_away_by_fluids tag | @SplashGG | yes |
| #4115 | feat(entity): implement copper golem sorting and statue lifecycle | @sluhtie | no |
| #4116 | fix(item): keep block state when waxing copper | @Kacperon | no |
| #4117 | fix(entity): restore totem of undying death protection | @ida64 | no |
| #4118 | fix(inventory): stop the throw loop when a take is empty | @ida64 | no |
| #4119 | fix(inventory): write the shift-click remainder back to the source slot (Item Dupe) | @ida64 | no |
| #4120 | fix(entity): damage armor durability when hurt | @ida64 | no |

---

## Appendix D — Tracking-issue cross-check

| Tracking issue | State in this roadmap | Notes |
| :--- | :--- | :--- |
| `#449` Release 1.0.0 | `W0` | Only ABI and community test are unchecked upstream, but the other checkboxes are stale. |
| `#1402` Redstone | `W4` | Checkbox says pistons broken; code exists but movement/persistence gaps remain. Daylight detector is marked missing; verify. |
| `#1403` World | `W8`/`W9`/`W10` | Persistence, chunks, lighting, borders, scoreboard, bossbar, spawning all have code; the 26.x sub-item is unchecked. |
| `#1404` Combat | `W6` | 1.8 combat is the only unchecked item; shields/bows/crits are marked done but open issues contradict that. |
| `#1406` Mob AI (closed) | `W1` | Closed, but its checklist is the old goal-only view. `#3468` supersedes it. |
| `#3468` Mob AI | `W1`/`W2` | Authoritative. Phase 0 done; Phases 1-4, navigation, `can_float`, move control, targeting, anger, knockback all open. |
| `#3519` Wilderness Bound | `H09` | The fork has most 26.3 content; audit and close or narrow. |
| `#3867` GameTest | `Q01` | Detailed blocker list; use it as the Q01 checklist. |
| `#1482` Codebase docs | `S01` | Eight crates still undocumented: pumpkin, codecs, data, gametest, plugin-runtime, plugin-wit, protocol, world. |
| `#1726` Translations | `S05` | Server locale, log translations, public platform, command descriptions, plugin translations, `to_translated` optimization. |
| `#3096` API | `M02` | Recipe types, registries, locator bar, advancement builder. |
| `#1407` Plugins (closed) | `W13` | Closed; native lifecycle exists, but ABI stability and event coverage do not. |

---

## Appendix E — High-risk files

These files are large and central. Changes here need an owner review and a boot test even if the diff looks small.

| File | Lines (approx.) | Why |
| :--- | ---: | :--- |
| `crates/pumpkin-core/src/world/mod.rs` | 7,650 | Ticking, saving, entities, tracking, chunks |
| `crates/pumpkin-core/src/entity/player.rs` | 8,225 | Movement, inventory, combat, sleep, stats |
| `crates/pumpkin-core/src/entity/living.rs` | large | Damage, attributes, equipment, effects |
| `crates/pumpkin-core/src/entity/ai/pathfinder/**` | large | All mob movement |
| `crates/pumpkin-world/src/level.rs` | large | Chunk cache, entity saver, clean_memory |
| `crates/pumpkin-world/src/generation/**` | large | Seed parity |
| `crates/pumpkin-protocol/src/**` | large | Every packet |
| `crates/pumpkin-core/src/plugin/**` | 283 event files | ABI and event contracts |
| `crates/pumpkin-data/src/generated/**` | very large | Never hand-edit |

---

## Appendix F — Current code facts used in this roadmap

These are the concrete audit findings that justify the P0 tags. Re-check them before starting a task; the fork moves.

| Finding | Evidence |
| :--- | :--- |
| Java supports only 26.3 | `CURRENT_MC_VERSION = V_26_3`, `LOWEST_SUPPORTED_MC_VERSION = V_26_3` |
| Bedrock supports 1.26.51 / protocol 2193 | `CURRENT_BEDROCK_MC_VERSION`, `CURRENT_BEDROCK_MC_PROTOCOL` |
| 352 open issues, 313 open PRs | GitHub inventory, 2026-10-20 |
| 562 in-scope tracker entries: 402 done, 155 partial, 5 planned | tracker data, excluding integrated-only `/publish` and `/unpublish` |
| No RCON implementation | `grep -R rcon crates` returns only translation strings |
| No spawn chunks | `grep -R spawn_chunk` returns nothing in core/world/config |
| Forced chunks do not get tickets outside GameTest | `add_force_ticket` only called in `server_test_manager.rs` |
| Sculk sensors are never triggered by game events | Fixed in `798734a3a`; `emit_game_event` now dispatches typed vibrations to sensors, shriekers, and wardens |
| Loot functions are absent | `pumpkin-util/loot_table.rs` has entries/pools/conditions but no function model |
| Loot parser supports only `set_count`, `apply_bonus`, `looting_enchant` | `loot_table_loader.rs` |
| Recipe loader supports only shaped/shapeless, cooking, brewing | `recipe_loader.rs` |
| 20 advancement trigger variants | `advancement/trigger.rs` |
| 41 status effects in data; 10 dedicated runtime effect modules | `generated/effect.rs`, `entity/effect/` |
| 20 enchantment entity-effect modules | `enchantment/effects/` |
| World generators: flat, noise, end only | `generation/generator/mod.rs` |
| 52 structure keys dispatched; 71 biome constants | `generated/structures.rs`, `generated/biome.rs` |
| Brain Phase 0 exists; no mob uses activities | `entity/ai/brain/` |
| 61 goal implementations | `entity/ai/goal/` |
| Plugin WIT v0.1 and v0.2 exist; v0.2 behind `plugin-v0_2` | `crates/pumpkin-plugin-wit`, `crates/pumpkin` feature |
| Release matrix lacks FreeBSD, macOS x86_64, Android armv7/x86_64, Docker armv7 | `.github/workflows/release.yml`, `Dockerfile` |
| 211 TODO/unimplemented markers in `pumpkin-core` | `grep` count |
| 1868 generated advancement constants | `generated/advancement.rs` |
| 68 biomes, 240 configured feature variants, 52 structures | generated data |

---

## Appendix G — Issue-to-task quick map

Use this when an issue is assigned but the workstream is unclear. The full index is Appendix A.

| Area | Tasks |
| :--- | :--- |
| Mob AI foundation | `A01`-`A17` |
| Mob behavior | `B01`-`B35` |
| Entities/vehicles | `C01`-`C15` |
| Blocks/redstone | `D01`-`D21` |
| Items/inventory/loot | `E01`-`E18` |
| Combat | `F01`-`F15` |
| Player | `G01`-`G14` |
| Worldgen | `H01`-`H13` |
| Persistence/chunks | `I01`-`I13` |
| World simulation | `J01`-`J13` |
| Java protocol | `K01`-`K12` |
| Bedrock | `L01`-`L12` |
| Plugin API | `M01`-`M13` |
| Commands/data packs | `N01`-`N10` |
| Ops/security/packaging | `O01`-`O14` |
| Data parity | `P01`-`P16` |
| QA/testing | `Q01`-`Q14` |
| Performance | `PF01`-`PF14` |
| Docs/i18n | `S01`-`S13` |
| Parity audit | `T01`-`T14` |

---

## Appendix H — Local unmerged work

These branches exist in the fork and are not in `fork-ai-main`. Check them before starting the matching task. They are based on `master`, not on `fork-ai-main`, so they need a rebase and a full verification pass.

| Branch | Commit | Work | Task |
| :--- | :--- | :--- | :--- |
| `feat/vanilla-parity-batch` | `9ec254314` | Night sky darkening for monster spawn checks | `A12` |
| `feat/vanilla-parity-batch` | `d7c08db76` | Natural spawn caps and cadence | `A12` |
| `feat/vanilla-parity-batch` | `0847909c5` | Wandering trader spawning and lifecycle | `B27` |
| `feat/vanilla-parity-batch` | `eb91eed91` | `copy_components` loot function | `E15` |
| `feat/vanilla-parity-batch` | `6a21ce72e` | Explosion knockback scaled by resistance | `F03` |
| `feat/vanilla-parity-batch` | `f67c091d3` | Reject attacks outside interaction range | `F04` |
| `fix/console-ignore-empty-command` | `457d5b63c` | Ignore empty console commands (PR `#4111`) | `N08` |

**Rule:** do not merge these branches wholesale. Each commit needs its own review, a rebase onto current `fork-ai-main`, a boot test, and the verification ladder for its task. The generated `loot_table.rs` change in `eb91eed91` is especially large; confirm it is reproducible with `cargo run --locked -p pumpkin-codegen` before landing.
