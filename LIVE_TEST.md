# Live Test Plan

**Status:** 562 / 562 theoretical tracker rows (100%). All 54 entity rows and Beehive & Bee Nest are implemented and reviewed in the theoretical count. No row is marked live-verified by this plan.

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

## 3. Coverage and deviations

Every in-scope tracker row now has a test above. Player-required checks are P01-P96; automatable checks are A01-A32. The 54 entity rows and Beehive & Bee Nest are covered by P65-P96 and A21-A31.

The following residual behaviors are known deviations or are deliberately not live-verified in this pass. They do not reopen a tracker row by themselves, but they are still required for the final live-verification pass:

- Horse-family mount GUI: mount screens are wired for Horse, Donkey, Mule, Llama, Trader Llama, Camel and Nautilus. The saddle slot is synthetic because saddle state is a flag, horse body armor is stored but does not reduce damage, and vanilla's per-slot validity/`isActive` filters are not reproduced. P74/P78/P81/P90 and A22 cover the supported paths.
- Blocks / Structure Block: save, load and capture work, but corner-mode `ACTION_SCAN_AREA` and redstone-powered execution are not implemented. P57-P60 cover pistons; the Structure Block limitation is recorded in `PARITY.md`.
- Creaking Heart: breaking the heart does not run vanilla's `removeProtector` teardown; the bound Creaking loses protection on its next heart check. P72 covers the live behavior.
- Ageable mobs / Golden Dandelion: `AgeLocked` is stored and ageing is suppressed, but right-clicking a baby does not toggle the lock, reset its age or emit growth particles.
- Thrown projectiles, Experience Orbs and Area Effect Clouds: `KnownMovement` history, orb `unstuckIfPossible`, and cloud owner/effect NBT are not reproduced. These are recorded as deviations in `PARITY.md`.
- Moving-piston drops and collision shape, and Elytra fly-into-wall damage: covered by the existing P57-P60 and P52 checks; a real-client pass is still required.
- Custom-biome `/locate` and datapack structure paths: covered by P61-P62 and A08; a datapack world is still required.
- `/jfr` and 1.8 combat mode: documented non-goals in `PARITY.md` and not tested here.
