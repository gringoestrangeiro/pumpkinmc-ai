# Parity Tracking

**Last updated:** 2026-10-20
**Fork commit:** `3ee95ba18`
**Target:** Java Edition 26.3. Bedrock 1.26.51 is a separate workstream.

## Headline

The public PumpkinMC tracker had **564 entries: 402 done, 155 partial, 7 planned**. That gives the familiar **71.3%** completion rate.

This fork now has **405 done, 152 partial, 7 planned**, or **71.8%**.

| Metric | Upstream | This fork | Delta |
| :--- | ---: | ---: | ---: |
| Tracker entries | 564 | 564 | 0 |
| Done | 402 | 405 | +3 |
| Partial | 155 | 152 | -3 |
| Planned | 7 | 7 | 0 |
| Strict completion, done only | 71.3% | 71.8% | +0.5 pp |
| Weighted completion, partial = 0.5 | 85.0% | 85.3% | +0.3 pp |

## What the percentage means

This is a **tracker completion rate**, not a measured behavioral parity score.

The tracker's green entries mean "an implementation source exists". They do not prove vanilla behavior. The 1.0 roadmap treats every green entry as unverified until the `T01` audit, and two entries touched by this batch were green while the behavior did not work:

- `Sculk Shrieker` was green, but shriekers did not respond to vibrations.
- `Sculk sensors, shriekers and calibrated sensors` was green, but `SculkSensorBlock::trigger` had no callers.

Commit `798734a3a` repairs both paths. They stay counted as done, but they now have code, game rules, and dispatch tests behind them.

The **71.8% is therefore an upper bound** until the green entries are audited. The honest verified number is lower. What is known for certain is that **159 tracker rows are still open** (152 partial and 7 planned).

## Per-category status

| Category | Entries | Done | Partial | Planned | Done % |
| :--- | ---: | ---: | ---: | ---: | ---: |
| Blocks | 196 | 163 | 31 | 2 | 83.2% |
| Entities | 160 | 50 | 110 | 0 | 31.2% |
| Commands | 96 | 91 | 1 | 4 | 94.8% |
| Items | 55 | 51 | 4 | 0 | 92.7% |
| Redstone | 27 | 24 | 3 | 0 | 88.9% |
| World | 16 | 13 | 3 | 0 | 81.2% |
| Combat | 14 | 13 | 0 | 1 | 92.9% |
| **Total** | **564** | **405** | **152** | **7** | **71.8%** |

Entities are the dominant gap. They are 110 of the 159 open rows, which is why mob AI work moves the tracker percentage much more than block edge cases do.

## Rows closed or repaired by this fork

| Tracker row | Before | Work | Evidence |
| :--- | :--- | :--- | :--- |
| Entities / Passive / Bat | partial | Bat AI parity pass | `ba3743a2e`, `cargo nextest -p pumpkin-core` |
| Blocks / Blocks / Structure Block | partial | Structure block save and load | `3ee95ba18`, template capture/placement tests |
| Entities / Hostile / Sulfur Cube | partial | Sulfur cube AI and synced size | `fb850f755` |
| Blocks / Fire & Sculk / Sculk Shrieker | done, unverified | Vibration dispatch, shrieker response | `798734a3a` |
| Redstone / Power & Sensors / Sculk sensors, shriekers and calibrated sensors | done, unverified | Frequency table, listener radius, calibrated filter, step path | `798734a3a` |

The Warden row remains partial. `798734a3a` adds vibration hearing and a minimal tendril-click response, but anger, sniffing, and the sonic boom are still missing.

## How to update this file

1. Use the tracker snapshot that matches the date in this file. The current baseline is the 564-row snapshot collected on 2026-10-20.
2. Change a row from `partial` or `planned` to `done` only when the matching roadmap task's **Done** line is met and verified. A source file appearing is not enough.
3. If a green row is found to be false, record it in this file and either fix it or move it back to partial in the tracker. Do not let green rows hide gaps.
4. Recompute the counts and percentages. Keep the date and fork commit current.
5. Keep the weighted number as a secondary signal. The primary number is done-only, because that is what the public 71% figure uses.

## Open gaps by workstream

The 152 partial and 7 planned rows are mapped to roadmap tasks in Appendix B. The largest clusters are:

- **Entities:** 110 partial rows, mostly mob AI, pathfinding, and missing interactions.
- **Blocks:** 31 partial and 2 planned rows, mostly block-specific edge cases, redstone details, and world interaction.
- **Redstone:** 3 partial rows.
- **World:** 3 partial rows.
- **Items:** 4 partial rows.
- **Commands:** 1 partial and 4 planned rows.

Closing block and entity rows is the fastest way to move the strict percentage. Closing an entity row is worth about 0.18 pp; closing the whole entity gap is worth about 19.5 pp.
