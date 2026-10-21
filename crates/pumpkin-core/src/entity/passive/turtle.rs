use std::sync::{
    Arc, Weak,
    atomic::{AtomicBool, AtomicI32, Ordering},
};

use crossbeam::atomic::AtomicCell;
use pumpkin_data::damage::DamageType;
use pumpkin_data::entity::EntityType;
use pumpkin_data::item::Item;
use pumpkin_data::item_stack::ItemStack;
use pumpkin_data::sound::Sound;
use pumpkin_data::tag::{self, Taggable};
use pumpkin_nbt::compound::NbtCompound;
use pumpkin_nbt::tag::NbtTag;
use pumpkin_util::math::position::BlockPos;
use pumpkin_util::math::vector2::Vector2;
use pumpkin_util::math::vector3::Vector3;
use pumpkin_world::world::BlockFlags;
use rand::RngExt;

use crate::block::blocks::turtle_egg::TurtleEggBlock;
use crate::entity::ai::pathfinder::{Navigator, NavigatorGoal};
use crate::entity::ai::util::{default_random_pos, land_random_pos};
use crate::entity::{
    Entity, EntityBase,
    ageable::{AgeableData, AgeableMob},
    ai::goal::{
        Controls, Goal, breed::BreedGoal, escape_danger::EscapeDangerGoal,
        look_at_entity::LookAtEntityGoal, swim::SwimGoal, tempt::TemptGoal, to_goal_ticks,
        try_find_water::TryFindWaterGoal,
    },
    living::LivingEntity,
    mob::{Mob, MobEntity},
    passive::animal::Animal,
    player::Player,
};
use crate::world::World;

const TEMPT_ITEMS: &[&Item] = &[&Item::SEAGRASS];

const LAY_EGG_SEARCH_RANGE: i32 = 16;
const GO_HOME_GIVE_UP_TICKS: i32 = 600;
const GO_TO_WATER_GIVE_UP_TICKS: i32 = 1200;
const TRAVEL_HORIZONTAL_RANGE: i32 = 512;
const TRAVEL_VERTICAL_RANGE: i32 = 4;
const TRAVEL_CHUNK_RADIUS: i32 = 34;

/// Vanilla `MoveToBlockGoal.findNearestBlock`: walk outward in rings so the nearest valid
/// position wins, then the vertical offsets in the order the goal declares them.
fn find_nearest_block(
    world: &World,
    origin: BlockPos,
    vertical_offsets: &[i32],
    range: i32,
    mut valid: impl FnMut(&BlockPos) -> bool,
) -> Option<BlockPos> {
    for y in vertical_offsets {
        for r in 0..range {
            let mut x = 0;
            while x <= r {
                let mut z = if x < r && x > -r { r } else { 0 };
                while z <= r {
                    let pos = BlockPos::new(origin.0.x + x, origin.0.y + y, origin.0.z + z);
                    if valid(&pos) {
                        return Some(pos);
                    }
                    z = if z > 0 { -z } else { 1 - z };
                }
                x = if x > 0 { -x } else { 1 - x };
            }
        }
    }
    None
}

/// Vanilla `Turtle.BABY_ON_LAND_SELECTOR`: only targets that are babies on land.
///
/// The cat and ocelot prey target goals share this one predicate, like vanilla.
pub(crate) fn baby_on_land_selector(target: &LivingEntity, _world: &World) -> bool {
    target.entity.age.load(Ordering::Relaxed) < 0 && !target.is_in_water()
}

pub struct TurtleEntity {
    pub mob_entity: MobEntity,
    pub ageable_data: AgeableData,
    pub has_egg: AtomicBool,
    pub laying_egg: AtomicBool,
    pub home_pos: AtomicCell<BlockPos>,
    pub travel_pos: AtomicCell<Option<BlockPos>>,
    pub going_home: AtomicBool,
    pub lay_egg_counter: AtomicI32,
}

impl TurtleEntity {
    pub fn new(entity: Entity) -> Arc<Self> {
        let mob_entity = MobEntity::new(entity);
        let home_pos = AtomicCell::new(mob_entity.living_entity.entity.block_pos.load());
        *mob_entity
            .navigator
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner) = Navigator::amphibious(false);
        let turtle = Self {
            mob_entity,
            ageable_data: AgeableData::default(),
            has_egg: AtomicBool::new(false),
            laying_egg: AtomicBool::new(false),
            home_pos,
            travel_pos: AtomicCell::new(None),
            going_home: AtomicBool::new(false),
            lay_egg_counter: AtomicI32::new(0),
        };
        let mob_arc = Arc::new(turtle);
        let mob_weak: Weak<dyn Mob> = {
            let mob_arc: Arc<dyn Mob> = mob_arc.clone();
            Arc::downgrade(&mob_arc)
        };
        let turtle_weak = Arc::downgrade(&mob_arc);

        {
            let mut goal_selector = mob_arc
                .mob_entity
                .goals_selector
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);

            goal_selector.add_goal(0, EscapeDangerGoal::new(1.2));
            goal_selector.add_goal(0, Box::new(SwimGoal::default()));
            goal_selector.add_goal(1, BreedGoal::new(1.0));
            goal_selector.add_goal(1, Box::new(TurtleLayEggGoal::new(turtle_weak.clone(), 1.0)));
            goal_selector.add_goal(2, Box::new(TemptGoal::new(1.1, TEMPT_ITEMS, false)));
            goal_selector.add_goal(
                3,
                Box::new(TurtleGoToWaterGoal::new(turtle_weak.clone(), 1.0)),
            );
            goal_selector.add_goal(4, Box::new(TurtleGoHomeGoal::new(turtle_weak.clone(), 1.0)));
            goal_selector.add_goal(7, Box::new(TurtleTravelGoal::new(turtle_weak.clone(), 1.0)));
            goal_selector.add_goal(
                8,
                LookAtEntityGoal::with_default(mob_weak, &EntityType::PLAYER, 8.0),
            );
            goal_selector.add_goal(9, Box::new(TurtleRandomStrollGoal::new(turtle_weak, 1.0)));
        };

        mob_arc
    }

    #[must_use]
    pub fn has_egg(&self) -> bool {
        self.has_egg.load(Ordering::Relaxed)
    }

    pub fn set_has_egg(&self, has_egg: bool) {
        self.has_egg.store(has_egg, Ordering::Relaxed);
        let entity = self.get_entity();
        entity.set_synced_data(pumpkin_data::tracked_data::turtle::HAS_EGG, has_egg);
    }

    #[must_use]
    pub fn is_laying_egg(&self) -> bool {
        self.laying_egg.load(Ordering::Relaxed)
    }

    pub fn set_laying_egg(&self, laying_egg: bool) {
        self.laying_egg.store(laying_egg, Ordering::Relaxed);
        self.lay_egg_counter
            .store(if laying_egg { 1 } else { 0 }, Ordering::Relaxed);
        let entity = self.get_entity();
        entity.set_synced_data(pumpkin_data::tracked_data::turtle::LAYING_EGG, laying_egg);
    }

    #[must_use]
    pub fn get_home_pos(&self) -> BlockPos {
        self.home_pos.load()
    }

    pub fn set_home_pos(&self, pos: BlockPos) {
        self.home_pos.store(pos);
    }

    #[must_use]
    pub fn is_near_home(&self, distance: f64) -> bool {
        let home = self.home_pos.load();
        let pos = self.get_entity().pos.load();
        let dx = f64::from(home.0.x) + 0.5 - pos.x;
        let dy = f64::from(home.0.y) + 0.5 - pos.y;
        let dz = f64::from(home.0.z) + 0.5 - pos.z;
        dx * dx + dy * dy + dz * dz < distance * distance
    }

    fn home_vector(&self) -> Vector3<f64> {
        let home = self.home_pos.load();
        Vector3::new(
            f64::from(home.0.x) + 0.5,
            f64::from(home.0.y),
            f64::from(home.0.z) + 0.5,
        )
    }

    fn is_going_home(&self) -> bool {
        self.going_home.load(Ordering::Relaxed)
    }

    fn set_going_home(&self, going_home: bool) {
        self.going_home.store(going_home, Ordering::Relaxed);
    }

    /// Vanilla `Turtle.TurtleLayEggGoal` tick, split out so the goal only owns the search.
    fn tick_lay_egg(&self, world: &Arc<World>, egg_pos: BlockPos) {
        let turtle_pos = self.get_entity().block_pos.load();
        if self.mob_entity.living_entity.is_in_water() {
            return;
        }

        if self.lay_egg_counter.load(Ordering::Relaxed) < 1 {
            self.set_laying_egg(true);
        } else if self.lay_egg_counter.load(Ordering::Relaxed) > to_goal_ticks(200) {
            world.play_sound_fine(
                Sound::EntityTurtleLayEgg,
                pumpkin_data::sound::SoundCategory::Blocks,
                &turtle_pos.to_f64(),
                0.3,
                0.9 + rand::random::<f32>() * 0.2,
            );

            let mut properties = pumpkin_data::block_properties::TurtleEggLikeProperties::default(
                &pumpkin_data::Block::TURTLE_EGG,
            );
            properties.eggs = rand::random_range(1..=4);
            world.set_block_state(
                &egg_pos,
                properties.to_state_id(&pumpkin_data::Block::TURTLE_EGG),
                BlockFlags::NOTIFY_ALL,
            );
            self.set_has_egg(false);
            self.set_laying_egg(false);
            self.mob_entity
                .set_love_ticks(600, self.mob_entity.breeder.load());
        }

        if self.is_laying_egg() {
            self.lay_egg_counter.fetch_add(1, Ordering::Relaxed);
        }
    }
}

impl AgeableMob for TurtleEntity {
    fn get_ageable_data(&self) -> &AgeableData {
        &self.ageable_data
    }

    fn age_boundary_reached(&self) {
        if self.is_baby() {
            return;
        }
        let entity = self.get_entity();
        let world = entity.world.load();
        if !world.level_info.load().game_rules.mob_drops {
            return;
        }
        if let Some(loot_table) = world.get_loot_table("minecraft:gameplay/turtle_grow") {
            for stack in loot_table.generate_loot(rand::random::<i64>()) {
                entity.spawn_at_location(stack);
            }
        }
    }
}

impl Animal for TurtleEntity {
    fn is_food(&self, item_stack: &ItemStack) -> bool {
        item_stack.item.has_tag(&tag::Item::MINECRAFT_TURTLE_FOOD)
            || item_stack.item == &Item::SEAGRASS
    }

    fn can_breed_with(&self, _partner: &dyn EntityBase) -> bool {
        !self.has_egg()
    }

    /// Vanilla `TurtleBreedGoal.breed`: sets the egg flag instead of spawning a child.
    fn spawn_child_from_breeding(&self, partner: &dyn EntityBase) {
        self.set_has_egg(true);
        self.finalize_spawn_child_from_breeding(partner);
    }

    fn animal_walk_target_value(&self, pos: &BlockPos) -> f32 {
        let world = self.get_entity().world.load_full();
        if !self.is_going_home() && TryFindWaterGoal::is_water(&world, pos) {
            return 10.0;
        }
        if TurtleEggBlock::on_sand(world.as_ref(), pos) {
            return 10.0;
        }
        world.get_light_level_dependent_magic_value(pos) - 0.5
    }
}

impl Mob for TurtleEntity {
    fn as_ageable(&self) -> Option<&dyn AgeableMob> {
        Some(self)
    }

    fn as_animal(&self) -> Option<&dyn Animal> {
        Some(self)
    }

    fn mob_write_nbt(&self, nbt: &mut NbtCompound) {
        self.write_ageable_nbt(nbt);
        let home = self.home_pos.load();
        nbt.put(
            "home_pos",
            NbtTag::IntArray(vec![home.0.x, home.0.y, home.0.z]),
        );
        nbt.put_bool("has_egg", self.has_egg());
    }

    fn mob_read_nbt(&self, nbt: &NbtCompound) {
        self.read_ageable_nbt(nbt);
        if let Some(home) = nbt.get_int_array("home_pos")
            && home.len() == 3
        {
            self.set_home_pos(BlockPos::new(home[0], home[1], home[2]));
        } else {
            self.set_home_pos(self.get_entity().block_pos.load());
        }
        if let Some(has_egg) = nbt.get_bool("has_egg") {
            self.set_has_egg(has_egg);
        }
    }

    fn get_mob_entity(&self) -> &MobEntity {
        &self.mob_entity
    }

    fn finalize_spawn(
        &self,
        _world: &Arc<World>,
        group_data: Option<crate::entity::mob::spawn::SpawnGroupData>,
    ) -> Option<crate::entity::mob::spawn::SpawnGroupData> {
        self.mob_entity.finalize_spawn_base();
        self.set_home_pos(self.get_entity().block_pos.load());
        group_data
    }

    fn mob_tick(&self, _caller: &dyn EntityBase) {
        self.ageable_ai_step();
    }

    fn get_ambient_sound(&self) -> Option<Sound> {
        let entity = self.get_entity();
        (!self.mob_entity.living_entity.is_in_water()
            && entity.on_ground.load(Ordering::Relaxed)
            && !self.is_baby())
        .then_some(Sound::EntityTurtleAmbientLand)
    }

    fn get_ambient_sound_interval(&self) -> i32 {
        200
    }

    fn mob_is_pushed_by_fluids(&self) -> bool {
        false
    }

    fn mob_on_lightning_strike(
        &self,
        caller: &dyn EntityBase,
        _lightning: &crate::entity::lightning::LightningBoltEntity,
    ) {
        self.damage_with_context(
            caller,
            f32::MAX,
            DamageType::LIGHTNING_BOLT,
            None,
            None,
            None,
        );
    }

    fn mob_init_data_tracker(&self) {
        let entity = self.get_entity();
        let is_baby = entity.age.load(Ordering::Relaxed) < 0;
        if is_baby {
            entity.set_synced_data(pumpkin_data::tracked_data::turtle::DATA_BABY_ID, true);
        }
        entity.set_synced_data(pumpkin_data::tracked_data::turtle::HAS_EGG, self.has_egg());
        entity.set_synced_data(
            pumpkin_data::tracked_data::turtle::LAYING_EGG,
            self.is_laying_egg(),
        );
    }

    fn mob_interact(&self, player: &Arc<Player>, item_stack: &mut ItemStack) -> bool {
        let item = item_stack.get_item();
        // Vanilla `Turtle.canBeLeashed` is false; an already leashed turtle can still be
        // released by `MobEntity::mob_interact`.
        if item == &Item::LEAD && !self.get_entity().is_leashed() {
            return false;
        }
        if self.is_food(item_stack) && (self.is_baby() || !self.has_egg()) {
            return self.animal_interact(player, item_stack, Sound::EntityTurtleAmbientLand);
        }
        self.mob_entity.mob_interact(player, item_stack)
    }
}

/// Vanilla `Turtle.TurtleGoHomeGoal`.
struct TurtleGoHomeGoal {
    turtle: Weak<TurtleEntity>,
    speed: f64,
    stuck: bool,
    close_to_home_try_ticks: i32,
}

impl TurtleGoHomeGoal {
    const fn new(turtle: Weak<TurtleEntity>, speed: f64) -> Self {
        Self {
            turtle,
            speed,
            stuck: false,
            close_to_home_try_ticks: 0,
        }
    }
}

impl Goal for TurtleGoHomeGoal {
    fn can_start(&mut self, _mob: &dyn Mob) -> bool {
        let Some(turtle) = self.turtle.upgrade() else {
            return false;
        };
        if turtle.is_baby() {
            return false;
        }
        if turtle.has_egg() {
            return true;
        }
        turtle.get_random().random_range(0..to_goal_ticks(700)) == 0 && !turtle.is_near_home(64.0)
    }

    fn should_continue(&mut self, _mob: &dyn Mob) -> bool {
        let Some(turtle) = self.turtle.upgrade() else {
            return false;
        };
        !turtle.is_near_home(7.0)
            && !self.stuck
            && self.close_to_home_try_ticks <= self.get_tick_count(GO_HOME_GIVE_UP_TICKS)
    }

    fn start(&mut self, _mob: &dyn Mob) {
        if let Some(turtle) = self.turtle.upgrade() {
            turtle.set_going_home(true);
            self.stuck = false;
            self.close_to_home_try_ticks = 0;
        }
    }

    fn stop(&mut self, _mob: &dyn Mob) {
        if let Some(turtle) = self.turtle.upgrade() {
            turtle.set_going_home(false);
        }
    }

    fn tick(&mut self, mob: &dyn Mob) {
        let Some(turtle) = self.turtle.upgrade() else {
            return;
        };
        let entity = turtle.get_entity();
        let world = entity.world.load_full();
        let close_to_home = turtle.is_near_home(16.0);
        if close_to_home {
            self.close_to_home_try_ticks += 1;
        }

        let idle = mob.is_navigator_idle();
        if !idle {
            return;
        }

        let home = turtle.home_vector();
        let mut next =
            default_random_pos::get_pos_towards(mob, 16, 3, home, std::f64::consts::PI / 10.0);
        if next.is_none() {
            next =
                default_random_pos::get_pos_towards(mob, 8, 7, home, std::f64::consts::FRAC_PI_2);
        }
        if let Some(position) = next
            && !close_to_home
            && world.get_block(&BlockPos::floored(position.x, position.y, position.z))
                != &pumpkin_data::Block::WATER
        {
            next =
                default_random_pos::get_pos_towards(mob, 16, 5, home, std::f64::consts::FRAC_PI_2);
        }

        let Some(position) = next else {
            self.stuck = true;
            return;
        };
        let mob_pos = entity.pos.load();
        mob.get_mob_entity()
            .navigator
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .set_progress(NavigatorGoal::new(mob_pos, position, self.speed));
    }

    fn controls(&self) -> Controls {
        Controls::MOVE
    }
}

/// Vanilla `Turtle.TurtleGoToWaterGoal`, a `MoveToBlockGoal` for water blocks.
struct TurtleGoToWaterGoal {
    turtle: Weak<TurtleEntity>,
    speed: f64,
    next_start_tick: i32,
    try_ticks: i32,
    block_pos: BlockPos,
    reached_target: bool,
}

impl TurtleGoToWaterGoal {
    const fn new(turtle: Weak<TurtleEntity>, speed: f64) -> Self {
        Self {
            turtle,
            speed,
            next_start_tick: 0,
            try_ticks: 0,
            block_pos: BlockPos::ZERO,
            reached_target: false,
        }
    }

    fn find_water(&self, mob: &dyn Mob) -> Option<BlockPos> {
        let world = mob.get_entity().world.load();
        let origin = mob.get_entity().block_pos.load();
        for dx in -24..=24 {
            for dz in -24..=24 {
                let pos = BlockPos::new(origin.0.x + dx, origin.0.y - 1, origin.0.z + dz);
                if world.get_block(&pos) == &pumpkin_data::Block::WATER {
                    return Some(pos);
                }
            }
        }
        None
    }
}

impl Goal for TurtleGoToWaterGoal {
    fn can_start(&mut self, mob: &dyn Mob) -> bool {
        if self.next_start_tick > 0 {
            self.next_start_tick -= 1;
            return false;
        }
        self.next_start_tick = self.get_tick_count(200 + mob.get_random().random_range(0..200));

        let Some(turtle) = self.turtle.upgrade() else {
            return false;
        };
        let entity = turtle.get_entity();
        let in_water = turtle.mob_entity.living_entity.is_in_water();
        let can_look = if turtle.is_baby() {
            !in_water
        } else {
            !turtle.is_going_home() && !in_water && !turtle.has_egg()
        };
        if !can_look {
            return false;
        }
        if let Some(pos) = self.find_water(mob) {
            self.block_pos = pos;
            return true;
        }
        false
    }

    fn should_continue(&mut self, mob: &dyn Mob) -> bool {
        let Some(turtle) = self.turtle.upgrade() else {
            return false;
        };
        !turtle.mob_entity.living_entity.is_in_water()
            && self.try_ticks <= GO_TO_WATER_GIVE_UP_TICKS
            && mob.get_entity().world.load().get_block(&self.block_pos)
                == &pumpkin_data::Block::WATER
    }

    fn start(&mut self, mob: &dyn Mob) {
        self.try_ticks = 0;
        self.reached_target = false;
        let target = self.block_pos.up();
        let mob_pos = mob.get_entity().pos.load();
        let destination = Vector3::new(
            f64::from(target.0.x) + 0.5,
            f64::from(target.0.y),
            f64::from(target.0.z) + 0.5,
        );
        let speed = self.turtle.upgrade().map_or(self.speed, |turtle| {
            if turtle.is_baby() { 2.0 } else { self.speed }
        });
        mob.get_mob_entity()
            .navigator
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .set_progress(NavigatorGoal::new(mob_pos, destination, speed));
    }

    fn tick(&mut self, mob: &dyn Mob) {
        let target = self.block_pos.up();
        let mob_pos = mob.get_entity().pos.load();
        let target_center = Vector3::new(
            f64::from(target.0.x) + 0.5,
            f64::from(target.0.y),
            f64::from(target.0.z) + 0.5,
        );
        if mob_pos.squared_distance_to_vec(&target_center) > 1.0 {
            self.reached_target = false;
            self.try_ticks += 1;
            if self.try_ticks % 160 == 0 {
                mob.get_mob_entity()
                    .navigator
                    .lock()
                    .unwrap_or_else(std::sync::PoisonError::into_inner)
                    .set_progress(NavigatorGoal::new(mob_pos, target_center, self.speed));
            }
        } else {
            self.reached_target = true;
            self.try_ticks -= 1;
        }
    }

    fn controls(&self) -> Controls {
        Controls::MOVE | Controls::JUMP
    }
}

/// Vanilla `Turtle.TurtleLayEggGoal`, a `MoveToBlockGoal` for sand near the home position.
struct TurtleLayEggGoal {
    turtle: Weak<TurtleEntity>,
    speed: f64,
    next_start_tick: i32,
    try_ticks: i32,
    block_pos: BlockPos,
    reached_target: bool,
}

impl TurtleLayEggGoal {
    const fn new(turtle: Weak<TurtleEntity>, speed: f64) -> Self {
        Self {
            turtle,
            speed,
            next_start_tick: 0,
            try_ticks: 0,
            block_pos: BlockPos::ZERO,
            reached_target: false,
        }
    }

    fn find_sand(&self, mob: &dyn Mob) -> Option<BlockPos> {
        let world = mob.get_entity().world.load_full();
        let origin = mob.get_entity().block_pos.load();
        for y in [0, 1, -1] {
            for dx in -LAY_EGG_SEARCH_RANGE..=LAY_EGG_SEARCH_RANGE {
                for dz in -LAY_EGG_SEARCH_RANGE..=LAY_EGG_SEARCH_RANGE {
                    let pos = BlockPos::new(origin.0.x + dx, origin.0.y + y, origin.0.z + dz);
                    if world.get_block(&pos.up()).is_air()
                        && TurtleEggBlock::is_sand(world.as_ref(), &pos)
                    {
                        return Some(pos);
                    }
                }
            }
        }
        None
    }
}

impl Goal for TurtleLayEggGoal {
    fn can_start(&mut self, mob: &dyn Mob) -> bool {
        if self.next_start_tick > 0 {
            self.next_start_tick -= 1;
            return false;
        }
        self.next_start_tick = self.get_tick_count(200 + mob.get_random().random_range(0..200));

        let Some(turtle) = self.turtle.upgrade() else {
            return false;
        };
        if !turtle.has_egg() || !turtle.is_near_home(9.0) {
            return false;
        }
        if let Some(pos) = self.find_sand(mob) {
            self.block_pos = pos;
            return true;
        }
        false
    }

    fn should_continue(&mut self, mob: &dyn Mob) -> bool {
        let Some(turtle) = self.turtle.upgrade() else {
            return false;
        };
        let world = mob.get_entity().world.load_full();
        self.try_ticks >= -2400
            && self.try_ticks <= GO_TO_WATER_GIVE_UP_TICKS
            && turtle.has_egg()
            && turtle.is_near_home(9.0)
            && world.get_block(&self.block_pos.up()).is_air()
            && TurtleEggBlock::is_sand(world.as_ref(), &self.block_pos)
    }

    fn start(&mut self, mob: &dyn Mob) {
        self.try_ticks = 0;
        self.reached_target = false;
        let target = self.block_pos.up();
        let mob_pos = mob.get_entity().pos.load();
        let destination = Vector3::new(
            f64::from(target.0.x) + 0.5,
            f64::from(target.0.y),
            f64::from(target.0.z) + 0.5,
        );
        mob.get_mob_entity()
            .navigator
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .set_progress(NavigatorGoal::new(mob_pos, destination, self.speed));
    }

    fn tick(&mut self, mob: &dyn Mob) {
        let target = self.block_pos.up();
        let mob_pos = mob.get_entity().pos.load();
        let target_center = Vector3::new(
            f64::from(target.0.x) + 0.5,
            f64::from(target.0.y),
            f64::from(target.0.z) + 0.5,
        );
        if mob_pos.squared_distance_to_vec(&target_center) > 1.0 {
            self.reached_target = false;
            self.try_ticks += 1;
            if self.try_ticks % 40 == 0
                && let Some(turtle) = self.turtle.upgrade()
            {
                turtle
                    .mob_entity
                    .navigator
                    .lock()
                    .unwrap_or_else(std::sync::PoisonError::into_inner)
                    .set_progress(NavigatorGoal::new(mob_pos, target_center, self.speed));
            }
        } else {
            self.reached_target = true;
            self.try_ticks -= 1;
        }

        if self.reached_target
            && let Some(turtle) = self.turtle.upgrade()
        {
            let world = turtle.get_entity().world.load_full();
            if !turtle.mob_entity.living_entity.is_in_water() {
                turtle.tick_lay_egg(&world, self.block_pos.up());
            }
        }
    }

    fn controls(&self) -> Controls {
        Controls::MOVE | Controls::JUMP
    }
}

/// Vanilla `Turtle.TurtleTravelGoal`: a long random swim while in water.
struct TurtleTravelGoal {
    turtle: Weak<TurtleEntity>,
    speed: f64,
    stuck: bool,
}

impl TurtleTravelGoal {
    const fn new(turtle: Weak<TurtleEntity>, speed: f64) -> Self {
        Self {
            turtle,
            speed,
            stuck: false,
        }
    }

    fn has_chunks_around(world: &World, x: i32, z: i32) -> bool {
        let min_x = (x - TRAVEL_CHUNK_RADIUS) >> 4;
        let max_x = (x + TRAVEL_CHUNK_RADIUS) >> 4;
        let min_z = (z - TRAVEL_CHUNK_RADIUS) >> 4;
        let max_z = (z + TRAVEL_CHUNK_RADIUS) >> 4;
        for chunk_x in min_x..=max_x {
            for chunk_z in min_z..=max_z {
                if !world.level.is_chunk_loaded(&Vector2::new(chunk_x, chunk_z)) {
                    return false;
                }
            }
        }
        true
    }
}

impl Goal for TurtleTravelGoal {
    fn can_start(&mut self, _mob: &dyn Mob) -> bool {
        let Some(turtle) = self.turtle.upgrade() else {
            return false;
        };
        !turtle.is_going_home()
            && !turtle.has_egg()
            && turtle.mob_entity.living_entity.is_in_water()
    }

    fn should_continue(&mut self, mob: &dyn Mob) -> bool {
        let Some(turtle) = self.turtle.upgrade() else {
            return false;
        };
        !mob.is_navigator_idle()
            && !self.stuck
            && !turtle.is_going_home()
            && !turtle.is_in_love()
            && !turtle.has_egg()
    }

    fn start(&mut self, _mob: &dyn Mob) {
        let Some(turtle) = self.turtle.upgrade() else {
            return;
        };
        let entity = turtle.get_entity();
        let pos = entity.pos.load();
        let world = entity.world.load();
        let mut y = rand::random_range(-TRAVEL_VERTICAL_RANGE..=TRAVEL_VERTICAL_RANGE);
        if f64::from(y) + pos.y > f64::from(world.sea_level - 1) {
            y = 0;
        }
        let x = rand::random_range(-TRAVEL_HORIZONTAL_RANGE..=TRAVEL_HORIZONTAL_RANGE);
        let z = rand::random_range(-TRAVEL_HORIZONTAL_RANGE..=TRAVEL_HORIZONTAL_RANGE);
        turtle.travel_pos.store(Some(BlockPos::floored(
            pos.x + f64::from(x),
            pos.y + f64::from(y),
            pos.z + f64::from(z),
        )));
        self.stuck = false;
    }

    fn tick(&mut self, mob: &dyn Mob) {
        let Some(turtle) = self.turtle.upgrade() else {
            return;
        };
        let Some(travel_pos) = turtle.travel_pos.load() else {
            self.stuck = true;
            return;
        };
        if !mob.is_navigator_idle() {
            return;
        }

        let target = Vector3::new(
            f64::from(travel_pos.0.x) + 0.5,
            f64::from(travel_pos.0.y),
            f64::from(travel_pos.0.z) + 0.5,
        );
        let mut next =
            default_random_pos::get_pos_towards(mob, 16, 3, target, std::f64::consts::PI / 10.0);
        if next.is_none() {
            next =
                default_random_pos::get_pos_towards(mob, 8, 7, target, std::f64::consts::FRAC_PI_2);
        }
        if let Some(position) = next {
            let entity = turtle.get_entity();
            let world = entity.world.load_full();
            if !Self::has_chunks_around(
                &world,
                position.x.floor() as i32,
                position.z.floor() as i32,
            ) {
                next = None;
            }
        }
        let Some(position) = next else {
            self.stuck = true;
            return;
        };
        let mob_pos = turtle.get_entity().pos.load();
        mob.get_mob_entity()
            .navigator
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .set_progress(NavigatorGoal::new(mob_pos, position, self.speed));
    }

    fn stop(&mut self, _mob: &dyn Mob) {
        if let Some(turtle) = self.turtle.upgrade() {
            turtle.travel_pos.store(None);
        }
    }

    fn controls(&self) -> Controls {
        Controls::MOVE
    }
}

/// Vanilla `Turtle.TurtleRandomStrollGoal`: only while on land and not going home or carrying
/// an egg.
struct TurtleRandomStrollGoal {
    turtle: Weak<TurtleEntity>,
    speed: f64,
    interval: i32,
    target: Option<Vector3<f64>>,
}

impl TurtleRandomStrollGoal {
    const fn new(turtle: Weak<TurtleEntity>, speed: f64) -> Self {
        Self {
            turtle,
            speed,
            interval: 100,
            target: None,
        }
    }
}

impl Goal for TurtleRandomStrollGoal {
    fn can_start(&mut self, mob: &dyn Mob) -> bool {
        let Some(turtle) = self.turtle.upgrade() else {
            return false;
        };
        if turtle.mob_entity.living_entity.is_in_water()
            || turtle.is_going_home()
            || turtle.has_egg()
        {
            return false;
        }
        if mob
            .get_random()
            .random_range(0..to_goal_ticks(self.interval))
            != 0
        {
            return false;
        }
        self.target = land_random_pos::get_pos(mob, 10, 7);
        self.target.is_some()
    }

    fn should_continue(&mut self, mob: &dyn Mob) -> bool {
        !mob.is_navigator_idle()
    }

    fn start(&mut self, mob: &dyn Mob) {
        if let Some(target) = self.target {
            let mob_pos = mob.get_entity().pos.load();
            mob.get_mob_entity()
                .navigator
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .set_progress(NavigatorGoal::new(mob_pos, target, self.speed));
        }
    }

    fn stop(&mut self, _mob: &dyn Mob) {
        self.target = None;
    }

    fn controls(&self) -> Controls {
        Controls::MOVE
    }
}
