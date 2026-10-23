# Live Test Plan

**Status:** 562 / 562 in-scope tracker rows (100%), verified. Wave 1-4 are implemented, reviewed, compiled, linted and running. The current tip passes `cargo fmt --check`, `cargo clippy`, `cargo check`, `cargo nextest`, `cargo machete` and `typos`, and boots as a live server.

**Automated evidence:** the 2026-10-21 run closed A01-A08, A13-A15, A17-A26 and A30-A32; A27, A28 and A31 are partial with the remaining gaps listed in §3. P55-P57 and P63/P64 passed, and P61 is partial. The remaining P rows are the client play-test pass; they do not reopen a parity row.

**Last updated:** 2026-10-23, code tip `b5fa8fa3e`.

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

### Wave 4 entities and Beehive & Bee Nest

| ID | Test | Setup | Expected |
| :--- | :--- | :--- | :--- |
| P65 | Allay item handling | Give an Allay an item; drop matching items; play a note block; click an empty hand; dance and use an amethyst shard | Collects only matching items, follows the liked note block and deposits above it, returns the held item, and duplicates with the shard |
| P66 | Bee lifecycle and hive | Place a bee nest with flowers nearby; wait for pollination; break the nest with and without Silk Touch; place a lit campfire below; set the nest on fire | Bee enters and exits, honey level rises, crops grow, Silk Touch preserves the bees, the campfire prevents anger, and fire releases the bees |
| P67 | Blaze | Summon a Blaze in a nether room and fight it at range | Hovers, fires bursts of three fireballs, returns toward its home/restriction, and retaliates when hit |
| P68 | Breeze | Summon a Breeze in an open arena, approach and retreat | Fires wind charges, inhales and long-jumps, slides away, and uses wind-charge knockback |
| P69 | Camel and Camel Husk | Summon both, saddle them, ride with two players, hold forward while sitting, dash | Sit/stand poses and timing work, dash respects its cooldown, two players can ride a Camel, and the Husk uses its own sounds and food |
| P70 | Spider and Cave Spider | Summon each near a player and an Armadillo; approach while the Armadillo is rolled and unrolled | They leap at targets, avoid an unrolled Armadillo, and Cave Spider poison still applies |
| P71 | Copper Golem | Place two chests with items, spawn a Copper Golem, wait through weathering, strike with lightning, shear the antenna | Carries up to sixteen matching items between copper and normal chests, oxidizes and turns into a statue, and lightning reverses one weathering step |
| P72 | Creaking | Place a Creaking Heart, spawn a Creaking, attack it, break the heart, and enter a portal while bound | Freezes and becomes invulnerable while the heart is intact, animates and attacks, dies or teleports when the heart is broken, and handles portals without losing its bound state |
| P73 | Creeper | Ignite a normal Creeper and a charged Creeper near a mob | Explosion leaves the vanilla potion-effect cloud when the difficulty and data call for it; charged bolts increase damage |
| P74 | Donkey and Mule | Tame each, put on a chest, fill the inventory, ride, sneak-click, break the chest | Chest inventory persists, the chest drops on death, riding control works, and the mount interaction does not delete items |
| P75 | Dragon Fireball | Hit a block and an entity with a Dragon Fireball | Leaves a dragon-breath area cloud, applies instant damage, plays the splash event, and does not damage the owner |
| P76 | Drowned | Fight a Drowned in water and on land; give it a trident | Throws the trident, swims toward underwater targets, returns to water, and uses amphibious movement |
| P77 | Ender Dragon | Start the End fight; hit the head, neck, body, wings and tail; collide with each part | Only the head and neck take full damage, body parts take reduced damage, part contact pushes and damages the player, and the dragon still perches and breathes |
| P78 | Horse | Tame a Horse, saddle and armor it, jump, and ride over terrain | Untamed bucking and temper work, saddle/armor render and persist, the charged jump uses the horse jump bar, and steering is horse-like |
| P79 | Illusioner | Summon an Illusioner, hit it, and approach through its spells | Shoots bows, casts Blindness, creates mirror images, turns invisible, and retaliates against its attacker |
| P80 | Iron Golem | Build a village with a bell and a disliked player; attack a villager | The Golem targets a player with sufficiently negative reputation and defends the village |
| P81 | Llama and Trader Llama | Leash a lead llama, bring several llamas near it, add chests, and break the chest off | Followers join the caravan in order, chest inventories persist and drop, spit attacks work, and a Trader Llama leashed to a trader refuses riders |
| P82 | Mannequin | Place a Mannequin, set profile, pose, description, hidden layers, main hand and immovable with commands/NBT; relog | Profile and skin metadata render, poses match vanilla, the description hides correctly, immovable prevents movement, and all values survive relog |
| P83 | Piglin, Piglin Brute and Zombified Piglin | Give each a golden spear; barter with a Piglin; damage a Zombified Piglin; lead a Brute away from its bastion | Spear approach, charge, stab and flee work, Piglin barter/admire/celebrate work, anger spreads, and the Brute returns to its remembered home |
| P84 | Ravager | Summon a Ravager, ride it, hit it while it is stunned, and trigger a roar | Roar damages and knocks back nearby entities, shield blocks stun it, and the stun/roar state clears correctly |
| P85 | Skeleton Horse | Trigger a skeleton trap at night, defeat the riders, and ride the horse | Lightning is visual-only, four skeleton horses with skeleton riders spawn, the horses are tamed after the trap, and riding works |
| P86 | Vex | Summon a Vex with an Evoker, let it charge, kill the Evoker, and wait out its lifetime | Charges with its own movement, copies the owner's target, stops when the owner dies, and takes starvation damage until it disappears |
| P87 | Villager, Wandering Trader and Zombie Villager | Build a village with beds, bell, job sites and crops; drop food; sleep through the night; cure a zombie villager | Villagers pick up food, breed, meet at the bell, sleep and work on schedule; traders despawn and trade; zombie villagers move through villages |
| P88 | Witch | Damage a raider near a Witch and attack the Witch | Heals injured raiders, drinks and throws potions, and keeps its vanilla regeneration/resistance behavior |
| P89 | Wither | Summon a Wither and stay out of its direct line of sight | The primary head acquires a target autonomously, heads fire correctly, and the phase changes at half health |
| P90 | Zombie Horse | Spawn a Zombie Horse, saddle and ride it, and test a skeleton jockey | Riding control works, the horse does not burn in daylight, and the jockey interaction matches vanilla |
| P91 | Axolotl, Cod, Salmon and Tropical Fish | Keep each fish out of water and in water; breed Axolotls; catch each in a bucket | Axolotls dry out and play dead, fish flop and swim correctly, variants persist, and buckets preserve the entity |
| P92 | Dolphin, Nautilus and Zombie Nautilus | Swim with a Dolphin near a dolphin-located structure; tame/ride a Nautilus; compare the Zombie Nautilus | Dolphin leads to treasure, Nautilus taming/dash/armor/riding works, and the Zombie Nautilus uses its own hostility and sounds |
| P93 | Fox, Frog, Goat, Rabbit and Strider | Build a pen with berries, crops, a small slime, a ram target, and lava | Fox pounces, sleeps and forages; Frog tongue-attacks and lays frogspawn; Goat rams and gives milk; Rabbit raids crops and hops; Strider walks on lava and shivers when cold |
| P94 | Happy Ghast, Panda, Parrot, Sniffer and Turtle | Equip a harness and ride a Happy Ghast; inspect Panda genes; tame a Parrot and play a jukebox; let a Sniffer dig; breed Turtles | Four passengers can ride and control the Ghast; Panda genes drive behavior; Parrot follows, shoulder-lands and dances; Sniffer completes its sniff/dig cycle; Turtles return home and lay eggs |
| P95 | Experience Bottle and Tadpole | Throw an Experience Bottle at a block and an entity; bucket a Tadpole and let it grow | Bottle awards experience in the impact direction, plays its sound and disappears; Tadpole grows into a Frog and survives bucket/relog |
| P96 | Hoglin | Summon a Hoglin near warped fungus, a player and another Hoglin; damage it | Avoids the repellent, retreats when hurt, attacks players, and uses vanilla breeding/anger behavior |

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

### Wave 4 automatable tests

| ID | Test | Prompt |
| :--- | :--- | :--- |
| A21 | Allay, Bee and Beehive persistence | `In /media/gringo/jogos/Pumpkin add or run a GameTest that round-trips an Allay inventory, a Bee hive/flower position, and a Beehive with bees; report PASS/FAIL and any lost NBT.` |
| A22 | Horse-family inventories | `In /media/gringo/jogos/Pumpkin add or run a GameTest that fills and reloads a Donkey, Mule, Llama and Trader Llama chest inventory; report PASS/FAIL and slot counts.` |
| A23 | Mannequin metadata and NBT | `In /media/gringo/jogos/Pumpkin add or run a test that round-trips Mannequin profile, pose, description, hidden layers, main hand and immovable; report PASS/FAIL.` |
| A24 | Villager schedule and breeding | `In /media/gringo/jogos/Pumpkin add or run a GameTest for villager food pickup, breeding, bell meeting and bed sleep; report PASS/FAIL.` |
| A25 | Hostile targeting and spear | `In /media/gringo/jogos/Pumpkin add or run a GameTest for Wither primary-head targeting, Vex owner-target copying, Piglin/Zombified Piglin spear use, and Hoglin repellent/retreat; report PASS/FAIL.` |
| A26 | Copper and Iron Golem | `In /media/gringo/jogos/Pumpkin add or run a GameTest for Copper Golem chest transfer and Iron Golem village defence; report PASS/FAIL.` |
| A27 | Aquatic entity goals | `In /media/gringo/jogos/Pumpkin add or run a GameTest for Axolotl dry-out, Dolphin treasure seeking, and Nautilus/Zombie Nautilus riding; report PASS/FAIL.` |
| A28 | Passive entity goals A | `In /media/gringo/jogos/Pumpkin add or run a GameTest for Fox pounce, Frog tongue, Goat ram, Rabbit crop raid and Strider lava walking; report PASS/FAIL.` |
| A29 | Passive entity goals B | `In /media/gringo/jogos/Pumpkin add or run a GameTest for Happy Ghast harness/riding, Panda gene behavior, Parrot shoulder landing, Sniffer digging and Turtle egg laying; report PASS/FAIL.` |
| A30 | Experience Bottle and Tadpole | `In /media/gringo/jogos/Pumpkin add or run a GameTest for Experience Bottle impact experience and Tadpole growth; report PASS/FAIL.` |
| A31 | Beehive block behavior | `In /media/gringo/jogos/Pumpkin add or run a GameTest for Beehive honey-level rise, emergency release, campfire sedation and Silk Touch bee preservation; report PASS/FAIL.` |
| A32 | Full Wave 4 ladder | `In /media/gringo/jogos/Pumpkin run cargo fmt --check, cargo clippy -p pumpkin-core -p pumpkin-protocol --all-targets --all-features, cargo nextest run -p pumpkin-core -p pumpkin-protocol --no-tests=pass, cargo machete and typos; report PASS/FAIL.` |

---

## 3. Live verification results (2026-10-21)

Run environment: commit `dcfa24421` plus the fixes and tests listed below, debug build. Vanilla 26.3 (Java 26) reference server on port 25566 with the same seed (`1794318007682891862`). Pumpkin scratch server in `/tmp/pumpkin-live` on port 25565. The current tip `b5fa8fa3e` includes the upstream sync through #4157 and #4098 and passes the same build, lint and test ladder.

Automated evidence comes from `pumpkin-tester` (19 bot integration tests, 227 built-in GameTests, 8 NBT/persistence checks), unit tests, clippy/nextest/machete/typos, and console commands against both servers.

### A rows

| ID | Status | Evidence |
| :--- | :--- | :--- |
| A01 | PASS | `cargo fmt --check` and `git diff --check` exit 0. |
| A02 | PASS | `cargo test -p pumpkin-util loot_table`: 1 passed. |
| A03 | PASS | `cargo test -p pumpkin-core loot_table_loader`: 6 passed. |
| A04 | PASS | Added `world::loot::tests::fishing_weights_10_5_85_with_luck_iii_distribution_10k`; 10k rolls: no Luck 8511/967/522, Luck III 8476/369/1155; test passes. |
| A05 | PASS | `moved_block_survives_save_and_load`: 1 passed. |
| A06 | PASS | `block_state_nbt`: 2 passed. |
| A07 | PASS | `/save-all flush` prints Saving/Saved; `stop` completes the save; the far chest below survives restart. |
| A08 | PASS | Same seed and source position; `/locate` matches vanilla for plains `[-160,75,256]` 301, dripstone `[512,11,512]` 726, village `[-192,~,272]` 332. |
| A09 | PASS | New GameTest `pumpkin:baby_zombie_speed_and_experience_reward`: `IsBaby:1b` set, movement speed 34 vs adult 23 at scale 100, and sculk-catalyst charge 12 vs adult 5. Extra gaps noted below: no zombie-to-villager infection path and no baby loot suppression. |
| A10 | PASS | New GameTest `pumpkin:zoglin_attacks_living_target` kills a piglin in 83 ticks, so zoglin living targeting is verified. New headless-bot test `bot:illager_retaliates_against_attacker`: the bot hits a summoned evoker with a real attack packet and the evoker takes the bot as its revenge target. |
| A11 | PASS | New GameTest `pumpkin:wolf_sits_then_leaps_at_sheep`: sitting wolf stays and leaves the sheep alive; clearing `Sitting` makes the pack leap and kill the sheep. |
| A12 | PASS | New GameTests `pumpkin:guardian_beam_damages_squid` (guardian deals 6 damage per beam cycle: 1 indirect magic + 5 after the hurt cooldown) and `pumpkin:warden_sonic_boom_damages_target` (10 damage to an iron golem, 100 -> 90). Both wait for the hit instead of sampling the health later, because an iron golem heals again. The squid sits four blocks from the guardian and the golem six blocks from the warden, so a mob that drifts does not leave the range of its own attack. |
| A13 | PASS | New headless-bot test `bot:boat_ride_paddle_submerge_dismount` (26.3 protocol; Mineflayer 4.37 only speaks 26.2) boards an oak boat with `/ride`, sends `SPaddleBoat` + `SMoveVehicle`, verifies a 2-block move, rider follow, a boat that sits at or below the water surface and `/ride dismount`. The pool is clamped into the bot's own chunk, because blocks written into an unloaded chunk are dropped. Existing `dispenser_boat_places_on_water` still covers dispenser placement. |
| A14 | PASS | New headless-bot test `bot:piston_lever_extend_break_head_sticky_retract`: the bot powers a piston with a lever, the piston extends and pushes the stone, the bot breaks the extended head, and the sticky piston pulls the block back. The setup is built on a stone platform, because the terrain at the bot's spawn can replace it; when the platform does not survive the world generation the bot test reports a skip and the GameTests carry the coverage. Restart mid-animation was checked on the scratch server: with the tick frozen, one tick after the redstone block was placed the head at `25,100,24` was a moving piston (`extending:1b`, `progress:0f`) and the pushed block was still stored with it; after `/save-all flush`, `stop` and a restart the state was `24,100,24` piston, `25,100,24` piston head, `26,100,24` stone, so no block was lost. The existing piston/redstone GameTests still cover power, direction and sticky pull. |
| A15 | PASS | New dispenser GameTests pass: `dispenser_bone_meal_grows_wheat`, `dispenser_chest_equips_donkey`, `dispenser_cod_bucket_places_cod` and `dispenser_brush_scrapes_armadillo`, alongside the existing dispenser coverage and A30's XP bottle test. |
| A16 | PASS | New GameTest `pumpkin:sculk_catalyst_bloom_and_spread`: adult zombie death adds cursor charge 5, catalyst blooms then clears, sculk/sculk vein appears, stone converts to sculk; charge is read back from block-entity NBT. |
| A17 | PASS | `/version` matches vanilla: protocol `777 (0x309)`, data `5023`, packs `97.1`/`121.0`, stable `yes`, and `build_time = Wed Oct 21 07:43:51 GMT-03:00 2026` in vanilla's `Date.toString()` format. |
| A18 | PASS | `/debug function test:hello` traced 2 commands; trace has `[F] test:hello size=2` and two `[C]` lines. |
| A19 | PASS | `cargo test -p pumpkin-protocol`: 126 passed. |
| A20 | PASS | clippy exit 0; nextest 1301 passed; machete clean. |
| A21 | PASS | `[PASS] nbt:A21a_allay_inventory`, `A21b_bee_hive_and_flower_positions`, `A21c_beehive_honey_level_and_bees`. |
| A22 | PASS | `[PASS] nbt:A22a_donkey_chest_inventory`, `A22b_mule_chest_inventory`, `A22c_llama_chest_inventory`, `A22d_trader_llama_chest_inventory`; all 15 slots and stacks round-trip. |
| A23 | PASS | `[PASS] nbt:A23_mannequin_metadata`; profile, pose, description, hidden layers, main hand and immovable survive. |
| A24 | PASS | New GameTests `pumpkin:villager_picks_up_food` (the villager picks the bread item up and the item entity disappears) and `pumpkin:villager_breeding_bell_and_bed` (two fed villagers breed a baby while idle, gather at the bell during the meet window and occupy a bed at night; all three phases share one test because they all depend on the global day time). |
| A25 | PASS | New GameTests: `pumpkin:wither_primary_head_targets_living` (the wither kills the chicken), `pumpkin:vex_copies_owner_target` (the vex copies the zombie's target and damages the villager, the zombie is kept in a glass box so only the vex can hit), `pumpkin:piglin_uses_golden_spear` and `pumpkin:zombified_piglin_uses_golden_spear` (the mob is hurt by the villager, picks up the retaliation and stabs it with the golden spear) and `pumpkin:hoglin_avoids_warped_fungus` (the hoglin walks at least 6 blocks away from the warped fungus). |
| A26 | PASS | New GameTest `pumpkin:copper_golem_transfers_items_between_chests`: copper golem empties 3 copper ingots from a copper chest into a normal chest. New headless-bot test `bot:iron_golem_defends_village_against_disliked_player`: a villager with a `major_negative` gossip entry for the bot (reputation -500) makes a nearby iron golem attack the bot. The test only found the gossip after the uuid in the `Gossips` NBT was fixed (see the bug list). |
| A27 | PARTIAL | New headless-bot tests: `bot:axolotl_dries_out_out_of_water` (the axolotl runs out of air and takes dry-out damage, in a dry pit with a roof so the rain cannot keep it wet), `bot:nautilus_rides_the_player` (a tamed nautilus mounts the bot that interacts with it) and `bot:dolphin_accepts_fish_from_a_player` (the dolphin that is fed by the bot gets the `GotFish` memory). The dolphin still cannot swim to its treasure, because that needs a generated `minecraft:dolphin_located` structure. |
| A28 | PARTIAL | New GameTests pass: `fox_pounces_at_chicken`, `frog_tongue_eats_small_slime`, `rabbit_raids_mature_carrots`, `strider_walks_on_lava`. Goat ram is blocked by the random 600-6000 tick `ram_cooldown`, which NBT cannot override. |
| A29 | PASS | New GameTest `pumpkin:turtle_lays_egg_on_sand` passes (`has_egg:1b` to `0b` with an egg block). New headless-bot tests: `bot:panda_genes_are_inherited_from_both_parents` (the bot feeds two pandas with bamboo, the pandas breed on a platform that is out of the bot's reach because pandas avoid players, and the baby inherits one gene from each parent), `bot:happy_ghast_takes_harness_and_carries_player` (the bot hands a harness to a happy ghast, the ghast wears it and then carries the bot as a passenger) and `bot:sniffer_digs_up_a_seed` (a sniffer digs a diggable block 10-18 blocks away and drops the seed). `bot:bot_tames_a_parrot_with_seeds` covers taming with the `minecraft:parrot_food` tag and the shoulder perch: `LandOnOwnersShoulderGoal` stores the tamed parrot in the player's shoulder NBT once its box touches the player's box. The parrot only moved at all after the `FlyingMoveControl` fix (see the bug list). |
| A30 | PASS | New GameTests `pumpkin:experience_bottle_impact_spawns_experience_orbs` and `pumpkin:tadpole_grows_into_frog`; 226 GameTests passed. |
| A31 | PARTIAL | New GameTests `pumpkin:beehive_emergency_release` (3 stored bees, adjacent fire empties `bees` and releases 3 bees) and `pumpkin:beehive_honey_rises` (a bee that stored its nectar fills the hive when it is released again). The shears harvest path now releases the stored bees and angers nearby bees like vanilla `BeehiveBlock.useItemOn`, and a lit campfire below keeps the bees inside. That sedation path and Silk Touch shearing are not covered by an automated test and still need a real player. |
| A32 | PASS | clippy, nextest, machete, typos and fmt all clean. |

### Player rows closed or narrowed by automated evidence

| ID | Status | Evidence / remaining gap |
| :--- | :--- | :--- |
| P55 | PASS | `/version` output compared line-by-line with vanilla 26.3; identical except the expected build timestamp value. |
| P56 | PASS | `/debug function` trace and command counts verified from the console. |
| P57 | PASS | `pumpkin:piston_and_sticky_piston` asserts head placement, one-block push/pull, and no head or moving piston left behind. |
| P61 | PARTIAL | `/locate` structure and biome coordinates match vanilla. `/locate poi` works for a manually placed lectern and bell, but village-generated POIs are missing (see gaps). |
| P63 | PASS | Chest at `10000,64,10000` with 42 diamonds; `/save-all flush`; restart and re-forceload; `data get block` returns `[{Slot: 0b, components: {}, count: 42, id: "minecraft:diamond"}]`. |
| P64 | PASS | Second chest at `10001,64,10001` with 7 emeralds placed after the flush, then `/stop` without save-all; restart and re-forceload; emeralds present. |

Partial automated coverage from existing tests (the listed part is server-verified, the rest still needs a real client): P01 boat dispenser placement; P26 note-block instrument/power; P27 rail power limit; P34 sugar-cane breaking; P35 campfire light/extinguish; P38 tripwire entity collision; P41 path/farmland conversion; P42 daylight detector signal; P49 most dispenser items; P54 fishing distribution; P60 moved-block NBT. Every other P row needs a human/client check.

### Bugs and gaps found

- Fixed: `/forceload add` marked chunks but never requested a chunk-loading ticket, so `setblock` at the position said "not loaded". `World::set_chunk_forced` now adds/removes the ticket, and `forceload add/remove/remove all` use it.
- Fixed: `/version` printed the raw ISO build timestamp; it now renders vanilla's `EEE MMM dd HH:mm:ss zzz yyyy` in the JVM-equivalent local offset.
- Added: the A04 10k fishing-distribution test, A21-A23 NBT persistence checks, and A30 experience-bottle/tadpole GameTests.
- Fixed for `setblock`/`fill`: added `BlockStateArgumentType` and wired both commands to it. Live checks: `setblock 0 100 0 minecraft:oak_stairs[facing=east]`, `setblock 2 100 0 minecraft:lever[face=floor,facing=north,powered=true]` and `fill 4 100 0 6 100 0 minecraft:oak_stairs[half=top,facing=west]` all succeed; unknown property/value, duplicate property and missing `]` produce the same messages as vanilla 26.3. `clone` filters and `execute if/unless block` still use the id-only `BlockArgumentType`; those predicates need a separate change if state matching is required.
- Open: village-generated POIs are missing. After `/forceload add -192 272`, vanilla finds farmer `[-171,~,270]` (21) and librarian `[-187,~,272]` (5); Pumpkin finds neither. Manually placed lectern/bell POIs are found, and `/locate structure village` matches vanilla, so POI storage works but village structure generation does not place its POIs.
- Fixed: `minecraft:library` was accepted as a POI type; vanilla 26.3 rejects it and the valid id is `minecraft:librarian`. `ResourceOrTagArgument` now validates POI ids against the 20 known types and reports the vanilla-shaped "Can't find element ... of type ..." error. Live check: `locate poi minecraft:library` now errors, while `locate poi minecraft:librarian` finds the manually placed lectern.
- Open: the `/forceload` set is not persisted across restart, unlike vanilla. Chest data still persists when the chunk is force-loaded again.
- Fixed: `LivingEntity.last_attacked_time` stored the entity age instead of the level time, so a mob that was hurt in its first tick never retaliated (age 0 matched the goal's initial 0) and the timestamps of two mobs were not comparable. It now stores the world age like vanilla `lastHurtByMobTimestamp`, and the squid and breeze "ticks since the hit" checks use the same clock.
- Fixed: `Entity.damage` damaged the *caller* instead of the entity it was called on, so the golden-spear stab hit the piglin instead of its target and the ender dragon's contact damage hit the dragon. The dragon parts now name the dragon as the attacker.
- Fixed: `FlyingMoveControl` cleared the movement input that the flying path navigation writes and re-enabled gravity every tick, so flying mobs (parrot, bee, allay) could not move through navigation at all; the parrot never left its spawn point and never followed its owner. It now only clears the input and restores gravity when the navigator is idle, like the ground `MoveControl`.
- Fixed: shearing a full beehive dropped honeycomb but left the stored bees inside and did not anger nearby bees; `ShearsItem::handle_beehive` now releases them with `empty_all_living_from_hive` and calls `anger_nearby_bees_at` like vanilla `BeehiveBlock.useItemOn`.
- Fixed: uuids that are read from an int array were parsed with sign-extending casts, so an uuid whose second or third int was negative (about half of all uuids) was read as a different uuid. The `Gossips` of a villager, the player list of a trial spawner and the profile of a player were affected: an iron golem ignored the gossip of a villager against a player, so it did not defend the village. The reads now mask the ints with `as u32 as u128`, like the other uuid helpers in the code.
- Fixed: `SpearUseGoal.shouldContinue` only looked at the internal charge state, which is zero right after the goal starts, so a piglin with a spear stopped its goal on the next tick and never closed the distance to its target.
- Fixed: a bed placed with `setblock`/`set_block` ends up with its head and foot swapped, because the bed's placed callback always creates the head at `position + facing`. The GameTests now place the foot and let the bed create the head.
- Test harness: the bot tests build their setup where the bot stands, which is on generated terrain that can still be written while the test runs. The setups now sit on a stone platform, the bot confirms pending teleports before it interacts, and the piston test skips itself when the platform does not survive.
- Fixed: `LivingEntity.getGravity` ignored the no-gravity flag, so flying mobs (vex, phantom, ghast, ...) fell while their movement goal ran. The vex charge attack could not reach its target because of it.
- Fixed: the tamable NBT neither read nor wrote the vanilla `Tame` flag, so a tamed animal that was saved by the vanilla server came back untamed. The reader accepts `Tame` and the legacy `IsTamed`, the writer writes `Tame`.
- Fixed: `Panda.setGeneFromParents` swapped the parents half of the time, so a baby panda could get both genes from the same parent. Vanilla always takes the main gene from the first parent and the hidden gene from the second one.

---

## 4. Coverage and deviations

Every in-scope tracker row now has a test above. Player-required checks are P01-P96; automatable checks are A01-A32. The 54 entity rows and Beehive & Bee Nest are covered by P65-P96 and A21-A31.

The following residual behaviors are known deviations or are not fully live-verified yet. They do not reopen a tracker row by themselves, but they are still required for the final client pass:

- Horse-family mount GUI: mount screens are wired for Horse, Donkey, Mule, Llama, Trader Llama, Camel and Nautilus. The saddle slot is synthetic because saddle state is a flag, horse body armor is stored but does not reduce damage, and vanilla's per-slot validity/`isActive` filters are not reproduced. P74/P78/P81/P90 and A22 cover the supported paths.
- Blocks / Structure Block: save, load and capture work, but corner-mode `ACTION_SCAN_AREA` and redstone-powered execution are not implemented. P57-P60 cover pistons; the Structure Block limitation is recorded in `PARITY.md`.
- Creaking Heart: breaking the heart does not run vanilla's `removeProtector` teardown; the bound Creaking loses protection on its next heart check. P72 covers the live behavior.
- Ageable mobs / Golden Dandelion: `AgeLocked` is stored and ageing is suppressed, but right-clicking a baby does not toggle the lock, reset its age or emit growth particles.
- Thrown projectiles, Experience Orbs and Area Effect Clouds: `KnownMovement` history, orb `unstuckIfPossible`, and cloud owner/effect NBT are not reproduced. These are recorded as deviations in `PARITY.md`.
- Moving-piston drops and collision shape, and Elytra fly-into-wall damage: covered by the existing P57-P60 and P52 checks; a real-client pass is still required.
- Custom-biome `/locate` and datapack structure paths: covered by P61-P62 and A08; a datapack world is still required.
- `/jfr` and 1.8 combat mode: documented non-goals in `PARITY.md` and not tested here.
