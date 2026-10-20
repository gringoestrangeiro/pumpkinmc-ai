# Live Test Plan

**Status:** 507 / 562 theoretical tracker rows (90.2%). 55 rows remain: 54 entities and Beehive & Bee Nest. This file covers the 507 implemented rows. The remaining entity tests will be added when those rows land.

**Verified baseline:** 415 / 143 / 4 (73.8%). Nothing below has been run live yet.

## How to run

Use a scratch working directory so the server does not write into the repository:

```bash
mkdir -p /tmp/pumpkin-live && cd /tmp/pumpkin-live
cargo run --manifest-path /media/gringo/jogos/Pumpkin/Cargo.toml -p pumpkin
```

Stop with `stop` on stdin. For bot tests, connect a headless client after the `Done` line.

**P = needs a real player/client.** These cover animation, sound, rendering, prediction, or GUI behavior.
**A = automatable.** These can be driven by a GameTest, unit test, or bot.

---

## 1. Player-required tests

### Boats and rafts

| ID | Test | Setup | Expected |
| :--- | :--- | :--- | :--- |
| P01 | Water placement and float | Use a boat on water and on a block | Boat lands on the surface; no sinking or jitter; block placement keeps the item if blocked |
| P02 | Paddle movement | Board an empty boat and paddle forward/back/turn | Smooth movement, no rubber-banding or “Invalid move vehicle” kick |
| P03 | Ledge entry | Drive a boat off a ledge into water | One smooth snap to the waterline; no bounce loop |
| P04 | Underwater ejection | Submerge a boat for ~3 seconds | Rider is ejected; boat resets |
| P05 | Dismount | Sneak-dismount on land and over water | Valid floor beside the boat on land; on top of the boat over water |
| P06 | Two riders | Two players board one boat; a third tries | Two seats at 0.2/-0.6; third refused |
| P07 | Chest boat | Open a chest boat, fill it, relog, break it | 27 slots; contents persist; break drops contents; one seat only |
| P08 | Bamboo raft | Ride a bamboo raft | Seat height differs from a wooden boat; no clipping |
| P09 | Lily pads | Drive a boat and a raft over lily pads | Pads break; boat is not slowed or stopped |
| P10 | Boat collisions | Ram a boat, player and mob | No pass-through; entities are pushed |

### Mobs

| ID | Test | Setup | Expected |
| :--- | :--- | :--- | :--- |
| P11 | Zombie baby | Summon adult and baby zombies; compare walk speed; kill both | Baby ~50% faster; baby drops 2.5× XP |
| P12 | Zoglin targets | Zoglin near cow, creeper, zoglin, player | Attacks cow/player; ignores creeper/zoglin |
| P13 | Illager revenge | Shoot evoker, vindicator, pillager; shoot a raider near them | They retaliate against the attacker; ignore fellow raiders |
| P14 | Vindicator Johnny | Name a vindicator `Johnny`, spawn a cow | Johnny attacks the cow and other living mobs |
| P15 | Husk | Stand in sun; punch with empty hand and with sword | No sunlight burn; Hunger only on empty-hand hit |
| P16 | Wolf | Tame, order to sit, walk away; send at a sheep | Stays seated; leaps at the target |
| P17 | Armadillo | Sprint past; approach with an undead | Rolls up on threat; unrolls after danger |
| P18 | Polar bear | Adult + cub near a player | Adult charges only while the cub is nearby |
| P19 | Stray / Parched arrows | Let each shoot; check effects; splash Weakness on Parched | Slowness 600 / Weakness 600; Parched is Weakness-immune |
| P20 | Endermite | Set `PersistenceRequired:1b`; hit it | Never despawns; retaliates |
| P21 | Wither skeleton | Inspect equipment; attack a piglin; hit with Wither | Stone sword, no enchant glint, targets piglins, Wither-immune |
| P22 | Warden | Idle; then stand 10 blocks away with anger | Sniff pose/sound every ~6s; charge sound then sonic boom (~10 damage + knockback) |
| P23 | Guardian / Elder | Swim 4+ blocks away; break line of sight; hit while moving and while still | Beam after ~80 ticks; cancels on LOS loss; thorns only when stationary; Elder applies Mining Fatigue on its timer |
| P24 | Skeleton conversion | Stand in powder snow; leave and return; relog during conversion | Shaking after ~7s; converts to Stray after ~15s; leaving resets; timer persists |
| P25 | Enderman | Open-sky daytime; kill while carrying a block | Random daylight teleport; carried block drops with silk-touch loot; scream only while aggroed |

### Blocks and redstone

| ID | Test | Setup | Expected |
| :--- | :--- | :--- | :--- |
| P26 | Note block | Right-click, redstone pulse, custom head above | Note event; custom head sound; no UI click |
| P27 | Rails | 10 powered + 10 activator rails with a redstone source | First 8 power; 9th does not; types do not cross-power; removing source de-powers |
| P28 | Stems | Grow pumpkin/melon fruit onto podzol, moss, mud, farmland | Fruit accepted; sand/stone rejected |
| P29 | Chiseled bookshelf | Insert normal and enchanted books | Insert sounds; Used statistic increments |
| P30 | Bell | Ring near a sculk sensor | `block_change` vibration |
| P31 | Jukebox | Insert/eject disc; break with comparator attached | Comparator updates; stop-play event |
| P32 | Ender chest | Open near a seeing piglin and behind a wall | Seeing piglin targets player; blocked piglin stays idle |
| P33 | Big dripleaf | Shoot any projectile | Full tilt and tilt-down sound |
| P34 | Sugar cane | Place beside water only; remove water | Survives with water or tagged block; breaks when neither remains |
| P35 | Campfire | Extinguish; shoot burning arrow, then normal arrow | Burning arrow relights; normal arrow does not; waterlogged stays unlit |
| P36 | Frogspawn | Place on water; accelerate ticks | 2–5 tadpoles after the 3600–12000 tick delay |
| P37 | Lodestone | Bind compass; break lodestone; unload chunk and return | Tracker clears; name reverts; unloaded target is not cleared |
| P38 | Tripwire | Walk through; shear; use a bat/marker armor stand | Events fire; shears damage and disarm; ignored entities do not trigger |
| P39 | Beds | Summon a sleeping villager; right-click the bed | Villager wakes, bed frees both halves |
| P40 | Shulker box | Solid block in lid space; open normally; open near piglin | Refuses open when lid blocked; opens otherwise; piglin anger |
| P41 | Dirt path / farmland | Convert under an entity; fence gate above | Entity lifts 1/16; gate blocks conversion |
| P42 | Daylight detector | Right-click near sculk sensor | `block_change` with player source |
| P43 | Chests | Sit a tamed cat on the lid; open | Chest refuses to open |
| P44 | Infested blocks | Break with and without Silk Touch | Silk Touch drops the block and no silverfish; normal break spawns silverfish |
| P45 | Conduit | Activate frame; damage a hostile nearby | Conduit Power and attack beam work |
| P46 | Dragon egg | Hit and right-click; let it fall | Teleports; falls correctly |
| P47 | Sea pickles | Bonemeal on coral; place/stack | Spreads; light and waterlogging correct |
| P48 | Fire | Burn in swamp/jungle vs plains | Faster burnout in the eight increased-burnout biomes |
| P49 | Dispenser | Fire bone meal, XP bottle, chest item, brush, mob bucket | Each case performs its vanilla effect |
| P50 | Sculk catalyst | Kill a mob within 8 blocks; relog | Bloom, XP absorbed, sculk spreads; cursors persist |

### Items, combat, pistons, commands, world

| ID | Test | Setup | Expected |
| :--- | :--- | :--- | :--- |
| P51 | Mace | Hit from 1.6/3/8/12 blocks of fall; land near mobs | Damage follows the piecewise curve; nearby mobs are knocked back; no smash while gliding |
| P52 | Elytra | Jump and glide; land; break elytra; toggle creative flight/water/vehicle; relog | Glide pose/flag visible; 1 durability per 20 ticks; flight stops correctly |
| P53 | Name tag | Name a mob; relog; use in survival/creative | Name/persistence survive; consumed in survival only |
| P54 | Fishing | Fish in open water and a cramped pond; with/without Luck of the Sea | Correct junk/treasure/fish distribution; treasure only in open water |
| P55 | `/version` | Run `/version` | Vanilla-style version, data, series, protocol, build time, pack versions, stable |
| P56 | `/debug function` | Run `/debug function <name>` | Trace file with `[F]`/`[C]` lines and correct counts |
| P57 | Piston basics | Power/unpower piston and sticky piston | Head appears; one block push/pull; no stray moving piston |
| P58 | Piston head/base break | Break head and base in creative and survival | Breaking player passes through; correct drops; no orphaned head |
| P59 | Quick toggle | 1-tick pulse on sticky piston | Block drops instead of being dragged |
| P60 | Save mid-animation | Save/stop/restart during the 2-tick extension | Moved block lands; no air-only moving piston |
| P61 | `/locate` noise | Locate structure/biome/POI in a noise world | Coordinates match vanilla for the same seed |
| P62 | `/locate` flat | Flat world with village/stronghold overrides | Village/stronghold located in the fixed biome |
| P63 | Save flush | Chest 10k blocks out; `/save-all flush`; restart | Feedback after writes; chest contents survive |
| P64 | `#3902` | Same chest; `/stop` without save-all; restart | Chest contents survive |

---

## 2. Automatable tests

Each prompt is intentionally short. Send it to an AI with build permission in the repository.

| ID | Test | Prompt |
| :--- | :--- | :--- |
| A01 | Formatting and whitespace | `In /media/gringo/jogos/Pumpkin run cargo fmt --check and git diff --check; report PASS/FAIL. Do not edit files.` |
| A02 | Loot weight/quality | `In /media/gringo/jogos/Pumpkin run cargo test -p pumpkin-util loot_table; report PASS/FAIL and failing assertions.` |
| A03 | Loot loader nested tables | `In /media/gringo/jogos/Pumpkin run cargo test -p pumpkin-core loot_table_loader; report PASS/FAIL.` |
| A04 | Fishing distribution | `In /media/gringo/jogos/Pumpkin run the fishing loot tests; if absent, add a 10k-roll test for weights 10/5/85 and Luck III, then run it; report the histogram.` |
| A05 | Piston moved-state NBT | `In /media/gringo/jogos/Pumpkin run cargo test -p pumpkin-core moved_block_survives_save_and_load; report PASS/FAIL.` |
| A06 | Block-state codec | `In /media/gringo/jogos/Pumpkin run cargo test -p pumpkin-world block_state_nbt; report PASS/FAIL.` |
| A07 | Save/flush round trip | `In /media/gringo/jogos/Pumpkin run the world save round-trip tests; then a scratch-server boot with /save-all flush and a restart; report PASS/FAIL.` |
| A08 | `/locate` coordinate fixture | `In /media/gringo/jogos/Pumpkin run the locate tests; compare one seed's structure/biome coordinates against vanilla 26.3; report PASS/FAIL.` |
| A09 | Zombie baby speed/XP | `In /media/gringo/jogos/Pumpkin add or run a GameTest for baby zombie speed and XP; report PASS/FAIL.` |
| A10 | Zoglin/illager targeting | `In /media/gringo/jogos/Pumpkin add or run a GameTest for zoglin living targets and illager revenge; report PASS/FAIL.` |
| A11 | Wolf sit/leap | `In /media/gringo/jogos/Pumpkin add or run a GameTest for wolf sit and leap; report PASS/FAIL.` |
| A12 | Guardian/Warden combat | `In /media/gringo/jogos/Pumpkin add or run a bot test for guardian beam and warden sonic boom damage; report PASS/FAIL and damage values.` |
| A13 | Boat physics bot | `In /media/gringo/jogos/Pumpkin start a scratch server, connect a Mineflayer bot, place/board/paddle a boat, submerge it, dismount; report PASS/FAIL with logs.` |
| A14 | Redstone/piston bot | `In /media/gringo/jogos/Pumpkin start a scratch server, use a bot to power a piston and a sticky piston, break the head, and restart mid-animation; report PASS/FAIL.` |
| A15 | Dispenser bot | `In /media/gringo/jogos/Pumpkin start a scratch server, fire bone meal, XP bottle, chest, brush and mob bucket from dispensers; report PASS/FAIL.` |
| A16 | Sculk catalyst bot | `In /media/gringo/jogos/Pumpkin start a scratch server, kill a mob near a catalyst, verify bloom/spread/XP and cursor persistence; report PASS/FAIL.` |
| A17 | `/version` output | `In /media/gringo/jogos/Pumpkin boot a scratch server, run /version from the console, compare the lines with vanilla 26.3; report PASS/FAIL.` |
| A18 | `/debug function` trace | `In /media/gringo/jogos/Pumpkin boot a scratch server, run /debug function on a test function, inspect the trace file and counts; report PASS/FAIL.` |
| A19 | Protocol/fuzz suite | `In /media/gringo/jogos/Pumpkin run the protocol codec and fuzz tests; report PASS/FAIL and any panic.` |
| A20 | Full CI-equivalent | `In /media/gringo/jogos/Pumpkin run cargo clippy -p pumpkin-core -p pumpkin-world -p pumpkin-util -p pumpkin-protocol --all-targets --all-features and cargo nextest run -p pumpkin-core -p pumpkin-world -p pumpkin-util --no-tests=pass; report PASS/FAIL.` |

---

## 3. Remaining rows

These 55 rows are not covered by this plan yet:

- **54 entities:** Allay, Axolotl, Bee, Blaze, Breeze, Camel, Camel Husk, Cave Spider, Cod, Copper Golem, Creaking, Creeper, Dolphin, Donkey, Dragon Fireball, Drowned, Ender Dragon, Experience Bottle, Fox, Frog, Goat, Happy Ghast, Hoglin, Horse, Illusioner, Iron Golem, Llama, Mannequin, Mule, Nautilus, Panda, Parrot, Piglin, Piglin Brute, Rabbit, Ravager, Salmon, Skeleton Horse, Sniffer, Spider, Strider, Tadpole, Trader Llama, Tropical Fish, Turtle, Vex, Villager, Wandering Trader, Witch, Wither, Zombie Horse, Zombie Nautilus, Zombie Villager, Zombified Piglin.
- **1 block:** Beehive & Bee Nest.

The entity waves will add their player-required and bot tests here before those rows are counted as verified.
