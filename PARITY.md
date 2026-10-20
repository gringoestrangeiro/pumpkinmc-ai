# Parity Tracking

**Last updated:** 2026-10-20
**Fork code commit:** `ad8a17bcb`
**Target:** Java Edition 26.3. Bedrock 1.26.51 is a separate workstream.

## Headline

The public PumpkinMC tracker had **564 entries: 402 done, 155 partial, 7 planned**. Two of those planned rows are `/publish` and `/unpublish`, which vanilla registers only for the integrated server. They are excluded from the dedicated-server target, so the in-scope baseline is **562 entries: 402 done, 155 partial, 5 planned**, or **71.5%**.

This fork now has **406 done, 152 partial, 4 planned**, or **72.2%**.

| Metric | Upstream | This fork | Delta |
| :--- | ---: | ---: | ---: |
| In-scope tracker entries | 562 | 562 | 0 |
| Done | 402 | 406 | +4 |
| Partial | 155 | 152 | -3 |
| Planned | 5 | 4 | -1 |
| Strict completion, done only | 71.5% | 72.2% | +0.7 pp |
| Weighted completion, partial = 0.5 | 85.3% | 85.8% | +0.5 pp |

## What the percentage means

This is a **tracker completion rate**, not a measured behavioral parity score.

The tracker's green entries mean "an implementation source exists". They do not prove vanilla behavior. The 1.0 roadmap treats every green entry as unverified until the `T01` audit, and two entries touched by this batch were green while the behavior did not work:

- `Sculk Shrieker` was green, but shriekers did not respond to vibrations.
- `Sculk sensors, shriekers and calibrated sensors` was green, but `SculkSensorBlock::trigger` had no callers.

Commit `798734a3a` repairs both paths. They stay counted as done, but they now have code, game rules, and dispatch tests behind them.

The **72.2% is therefore an upper bound** until the green entries are audited. The honest verified number is lower. What is known for certain is that **156 tracker rows are still open** (152 partial and 4 planned).

## Per-category status

| Category | Entries | Done | Partial | Planned | Done % |
| :--- | ---: | ---: | ---: | ---: | ---: |
| Blocks | 196 | 163 | 31 | 2 | 83.2% |
| Entities | 160 | 50 | 110 | 0 | 31.2% |
| Commands | 94 | 92 | 1 | 1 | 97.9% |
| Items | 55 | 51 | 4 | 0 | 92.7% |
| Redstone | 27 | 24 | 3 | 0 | 88.9% |
| World | 16 | 13 | 3 | 0 | 81.2% |
| Combat | 14 | 13 | 0 | 1 | 92.9% |
| **Total** | **562** | **406** | **152** | **4** | **72.2%** |

Entities are the dominant gap. They are 110 of the 156 open rows, which is why mob AI work moves the tracker percentage much more than block edge cases do.

## Rows closed or repaired by this fork

| Tracker row | Before | Work | Evidence |
| :--- | :--- | :--- | :--- |
| Entities / Passive / Bat | partial | Bat AI parity pass | `ba3743a2e`, `cargo nextest -p pumpkin-core` |
| Blocks / Blocks / Structure Block | partial | Structure block save and load | `3ee95ba18`, template capture/placement tests |
| Entities / Hostile / Sulfur Cube | partial | Sulfur cube AI and synced size | `fb850f755` |
| Commands / Vanilla / `/perf` | planned | `/perf start` and `/perf stop` ported from PR `#2958` | `ad8a17bcb` |
| Blocks / Fire & Sculk / Sculk Shrieker | done, unverified | Vibration dispatch, shrieker response | `798734a3a` |
| Redstone / Power & Sensors / Sculk sensors, shriekers and calibrated sensors | done, unverified | Frequency table, listener radius, calibrated filter, step path | `798734a3a` |

The Warden row remains partial. `798734a3a` adds vibration hearing and a minimal tendril-click response, but anger, sniffing, and the sonic boom are still missing.

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
| `/jfr start`, `/jfr stop`, `/jfr dump` | missing | JVM Flight Recorder tooling, not gameplay. Recommend a documented non-goal; operators can use JVM flags or `jcmd`. |

`/locate` is marked partial by the tracker, but the current code implements structure, biome, and POI searches. It needs a live verification before the row is moved to done.

### Excluded from the parity target

`/publish` and `/unpublish` are not part of this count. Vanilla registers them only for the integrated server (`CommandSelection.includeIntegrated`), and a dedicated server must not expose them. They were removed from the denominator instead of being treated as missing commands.

Dev-only commands gated by `SharedConstants.DEBUG_DEV_COMMANDS`, `DEBUG_CHASE_COMMAND`, or running in the IDE are also out of scope: `/chase`, `/debugpath`, `/debugmobspawning`, `/wardenspawntracker`, `/spawnarmortrims`, `/serverpack`, `/debugconfig`. `/emote` is not a vanilla command; `/me` is, and Pumpkin implements it.

Pumpkin extras that are not parity gaps: `/pumpkin`, `/plugin`, `/plugins`, `/tps`, and `/raid`. Vanilla only registers `/raid` in dev mode, so it can stay as a Pumpkin debug extra or be gated later.

### Effect on the tracker

The raw tracker had 564 rows. Removing `/publish` and `/unpublish` gives the 562-row in-scope baseline. Porting `/perf` moves one planned command row to done. The current total is **406/562 = 72.2%**. Verifying `/locate` would make it **407/562 = 72.4%**, and treating `/jfr` as a non-goal would give **407/561 = 72.5%**.

## How to update this file

1. Use the tracker snapshot that matches the date in this file. The current baseline is the 562-row in-scope snapshot derived from the 564-row public snapshot collected on 2026-10-20.
2. Change a row from `partial` or `planned` to `done` only when the matching roadmap task's **Done** line is met and verified. A source file appearing is not enough.
3. If a green row is found to be false, record it in this file and either fix it or move it back to partial in the tracker. Do not let green rows hide gaps.
4. Recompute the counts and percentages. Keep the date and fork commit current.
5. Keep the weighted number as a secondary signal. The primary number is done-only, because that is what the public 71% figure uses.

## Open gaps by workstream

The 152 partial and 4 planned rows are mapped to roadmap tasks in Appendix B. The largest clusters are:

- **Entities:** 110 partial rows, mostly mob AI, pathfinding, and missing interactions.
- **Blocks:** 31 partial and 2 planned rows, mostly block-specific edge cases, redstone details, and world interaction.
- **Redstone:** 3 partial rows.
- **World:** 3 partial rows.
- **Items:** 4 partial rows.
- **Commands:** 1 partial (`/locate`) and 1 planned (`/jfr`) row. `/jfr` is a recommended non-goal, so `/locate` is the only remaining command verification.

Closing block and entity rows is the fastest way to move the strict percentage. Closing an entity row is worth about 0.18 pp; closing the whole entity gap is worth about 19.5 pp.
