use std::sync::{
    Arc, Weak,
    atomic::{AtomicBool, AtomicI32, Ordering},
};

use pumpkin_data::Block;
use pumpkin_data::entity::{EntityPose, EntityStatus, EntityType};
use pumpkin_data::fluid::Fluid;
use pumpkin_data::frog_variant::FrogVariant;
use pumpkin_data::game_event::GameEvent;
use pumpkin_data::item::Item;
use pumpkin_data::item_stack::ItemStack;
use pumpkin_data::sound::{Sound, SoundCategory};
use pumpkin_data::tag::{self, Taggable};
use pumpkin_nbt::compound::NbtCompound;
use pumpkin_protocol::codec::optional_int::OptionalInt;
use pumpkin_protocol::codec::var_int::VarInt;
use pumpkin_util::math::position::BlockPos;
use pumpkin_world::world::BlockFlags;
use rand::RngExt;

use crate::entity::{
    Entity, EntityBase,
    ageable::{AgeableData, AgeableMob},
    ai::goal::{
        Controls, Goal,
        active_target::ActiveTargetGoal,
        long_jump::{LongJumpGoal, frog_acceptable_landing},
        look_around::RandomLookAroundGoal,
        look_at_entity::LookAtEntityGoal,
        swim::SwimGoal,
        tempt::TemptGoal,
        wander_around::WanderAroundGoal,
    },
    ai::pathfinder::node::PathType,
    experience_orb::ExperienceOrbEntity,
    living::LivingEntity,
    mob::{
        Mob, MobEntity,
        spawn::{SpawnGroupData, finalize_ageable_spawn},
    },
    passive::animal::Animal,
    player::Player,
};
use crate::world::World;

pub const FROG_FOOD: &[&Item] = &[&Item::SLIME_BALL];

/// Vanilla `VariantUtils.selectVariantToSpawn` for `frog_variant`: the cold and
/// warm entries are biome-conditioned at priority 1, temperate is the default.
fn variant_for_biome(biome: &pumpkin_data::biome::Biome) -> FrogVariant {
    let name = biome
        .registry_id
        .strip_prefix("minecraft:")
        .unwrap_or(biome.registry_id);
    if tag::WorldgenBiome::MINECRAFT_SPAWNS_COLD_VARIANT_FROGS
        .0
        .contains(&name)
    {
        FrogVariant::Cold
    } else if tag::WorldgenBiome::MINECRAFT_SPAWNS_WARM_VARIANT_FROGS
        .0
        .contains(&name)
    {
        FrogVariant::Warm
    } else {
        FrogVariant::Temperate
    }
}

const TONGUE_RANGE: f64 = 10.0;
const TONGUE_REACH: f64 = 1.75;
const TONGUE_MOVE_TIMEOUT: i32 = 100;
const TONGUE_SPEED: f64 = 2.0;

/// Represents a Frog, an amphibious mob that can eat small slimes and magma cubes.
///
/// Wiki: <https://minecraft.wiki/w/Frog>
pub struct FrogEntity {
    pub mob_entity: MobEntity,
    pub ageable_data: AgeableData,
    pub variant: AtomicI32,
    pub tongue_target_id: AtomicI32,
    pregnant: AtomicBool,
}

impl FrogEntity {
    pub fn new(entity: Entity) -> Arc<Self> {
        let world = entity.world.load_full();
        let variant = variant_for_biome(world.get_biome(&entity.block_pos.load()));
        let mob_entity = MobEntity::new(entity);
        let frog = Self {
            mob_entity,
            ageable_data: AgeableData::default(),
            variant: AtomicI32::new(variant.id() as i32),
            tongue_target_id: AtomicI32::new(-1),
            pregnant: AtomicBool::new(false),
        };
        let mob_arc = Arc::new(frog);
        let mob_weak: Weak<dyn Mob> = {
            let mob_arc: Arc<dyn Mob> = mob_arc.clone();
            Arc::downgrade(&mob_arc)
        };

        {
            let mut navigator = mob_arc
                .mob_entity
                .navigator
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            navigator.set_pathfinding_malus(PathType::Water, 4.0);
            navigator.set_pathfinding_malus(PathType::Trapdoor, -1.0);
            navigator.set_can_float(true);
        };

        {
            let mut goal_selector = mob_arc
                .mob_entity
                .goals_selector
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);

            goal_selector.add_goal(0, Box::new(SwimGoal::default()));
            goal_selector.add_goal(1, Box::new(TemptGoal::new(1.0, FROG_FOOD, false)));
            goal_selector.add_goal(2, FrogBreedGoal::new(&mob_arc));
            goal_selector.add_goal(2, TongueGoal::new(&mob_arc));
            goal_selector.add_goal(2, LayFrogSpawnGoal::new(&mob_arc));
            goal_selector.add_goal(
                2,
                Box::new(LongJumpGoal::new(
                    2,
                    4,
                    3.571_428_8,
                    100,
                    140,
                    frog_jump_sound,
                    Sound::EntityFrogStep,
                    frog_acceptable_landing,
                )),
            );
            goal_selector.add_goal(2, Box::new(WanderAroundGoal::new(1.0)));
            goal_selector.add_goal(
                3,
                LookAtEntityGoal::with_default(mob_weak, &EntityType::PLAYER, 6.0),
            );
            goal_selector.add_goal(4, Box::new(RandomLookAroundGoal::default()));
        };

        {
            let mut target_selector = mob_arc
                .mob_entity
                .target_selector
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            target_selector.add_goal(
                2,
                Box::new(ActiveTargetGoal::new(
                    &mob_arc.mob_entity,
                    &EntityType::SLIME,
                    10,
                    true,
                    false,
                    Some(is_small_cube),
                )),
            );
            target_selector.add_goal(
                2,
                Box::new(ActiveTargetGoal::new(
                    &mob_arc.mob_entity,
                    &EntityType::MAGMA_CUBE,
                    10,
                    true,
                    false,
                    Some(is_small_cube),
                )),
            );
        };

        mob_arc
    }

    #[must_use]
    pub fn get_variant(&self) -> FrogVariant {
        FrogVariant::from_id(self.variant.load(Ordering::Relaxed) as u32)
    }

    pub fn set_variant(&self, variant: FrogVariant) {
        self.variant.store(variant.id() as i32, Ordering::Relaxed);
        let entity = self.get_entity();
        entity.set_synced_data(
            pumpkin_data::tracked_data::frog::VARIANT,
            VarInt(variant.id() as i32),
        );
    }

    fn set_tongue_target(&self, target: &dyn EntityBase) {
        let id = target.get_entity().entity_id;
        self.tongue_target_id.store(id, Ordering::Relaxed);
        self.get_entity().set_synced_data(
            pumpkin_data::tracked_data::frog::TONGUE_TARGET_ID,
            OptionalInt(Some(id)),
        );
    }

    fn erase_tongue_target(&self) {
        self.tongue_target_id.store(-1, Ordering::Relaxed);
        self.get_entity().set_synced_data(
            pumpkin_data::tracked_data::frog::TONGUE_TARGET_ID,
            OptionalInt(None),
        );
    }

    #[must_use]
    pub fn is_pregnant(&self) -> bool {
        self.pregnant.load(Ordering::Relaxed)
    }

    fn set_pregnant(&self, pregnant: bool) {
        self.pregnant.store(pregnant, Ordering::Relaxed);
    }
}

fn is_small_cube(living: &LivingEntity, _world: &World) -> bool {
    living.entity.data.load(Ordering::Relaxed) == 1
}

/// Vanilla `Frog.canEat`.
fn can_eat(target: &dyn EntityBase) -> bool {
    let entity = target.get_entity();
    if entity.entity_type != &EntityType::SLIME && entity.entity_type != &EntityType::MAGMA_CUBE {
        return false;
    }
    entity.data.load(Ordering::Relaxed) == 1
}

fn frog_jump_sound(_mob: &dyn Mob) -> Sound {
    Sound::EntityFrogLongJump
}

/// Vanilla `ShootTongue`.
#[derive(Clone, Copy, PartialEq, Eq)]
enum TongueState {
    MoveToTarget,
    CatchAnimation,
    EatAnimation,
    Done,
}

struct TongueGoal {
    frog: Weak<FrogEntity>,
    target: Option<Arc<dyn EntityBase>>,
    state: TongueState,
    eat_timer: i32,
    calculate_path_counter: i32,
    move_ticks: i32,
}

impl TongueGoal {
    #[expect(
        clippy::unnecessary_box_returns,
        reason = "GoalSelector takes boxed goals"
    )]
    fn new(frog: &Arc<FrogEntity>) -> Box<Self> {
        Box::new(Self {
            frog: Arc::downgrade(frog),
            target: None,
            state: TongueState::Done,
            eat_timer: 0,
            calculate_path_counter: 0,
            move_ticks: 0,
        })
    }

    fn eat_entity(&self, mob: &dyn Mob) {
        let entity = mob.get_entity();
        entity.world.load().play_sound_fine(
            Sound::EntityFrogEat,
            SoundCategory::Neutral,
            &entity.pos.load(),
            2.0,
            1.0,
        );
        let Some(target) = &self.target else {
            return;
        };
        if target.get_entity().is_alive() {
            mob.get_mob_entity().try_attack(entity, target.as_ref());
        }
    }
}

impl Goal for TongueGoal {
    fn can_start(&mut self, mob: &dyn Mob) -> bool {
        let Some(target) = mob.get_mob_entity().get_target() else {
            return false;
        };
        if !target.get_entity().is_alive() || !can_eat(target.as_ref()) {
            return false;
        }
        let distance = mob
            .get_entity()
            .pos
            .load()
            .squared_distance_to_vec(&target.get_entity().pos.load());
        if distance > TONGUE_RANGE * TONGUE_RANGE {
            return false;
        }
        if mob.get_entity().pose.load() == EntityPose::Croaking {
            return false;
        }
        self.target = Some(target);
        true
    }

    fn should_continue(&mut self, _mob: &dyn Mob) -> bool {
        self.state != TongueState::Done
            && self
                .target
                .as_ref()
                .is_some_and(|target| target.get_entity().is_alive() && can_eat(target.as_ref()))
    }

    fn start(&mut self, mob: &dyn Mob) {
        let Some(target) = &self.target else {
            return;
        };
        let Some(frog) = self.frog.upgrade() else {
            return;
        };
        frog.set_tongue_target(target.as_ref());
        let entity = mob.get_entity();
        mob.get_mob_entity()
            .look_control
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .look_at_entity(mob, target);
        mob.get_mob_entity()
            .navigator
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .set_progress(crate::entity::ai::pathfinder::NavigatorGoal::new(
                entity.pos.load(),
                target.get_entity().pos.load(),
                TONGUE_SPEED,
            ));
        self.calculate_path_counter = 10;
        self.move_ticks = 0;
        self.state = TongueState::MoveToTarget;
    }

    fn stop(&mut self, mob: &dyn Mob) {
        if let Some(frog) = self.frog.upgrade() {
            frog.erase_tongue_target();
        }
        mob.set_mob_target(None);
        mob.get_entity().set_pose(EntityPose::Standing);
        mob.get_mob_entity()
            .navigator
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .stop();
        self.target = None;
        self.state = TongueState::Done;
        self.eat_timer = 0;
        self.move_ticks = 0;
    }

    fn tick(&mut self, mob: &dyn Mob) {
        let Some(target) = self.target.clone() else {
            return;
        };
        let Some(frog) = self.frog.upgrade() else {
            return;
        };
        frog.set_tongue_target(target.as_ref());
        let entity = mob.get_entity();

        match self.state {
            TongueState::MoveToTarget => {
                let distance = entity
                    .pos
                    .load()
                    .squared_distance_to_vec(&target.get_entity().pos.load());
                self.move_ticks += 1;
                if distance < TONGUE_REACH * TONGUE_REACH {
                    entity.world.load().play_sound_fine(
                        Sound::EntityFrogTongue,
                        SoundCategory::Neutral,
                        &entity.pos.load(),
                        2.0,
                        1.0,
                    );
                    entity.set_pose(EntityPose::UsingTongue);
                    let pull = entity.pos.load() - target.get_entity().pos.load();
                    if pull.length_squared() > 1.0e-7 {
                        target.get_entity().set_velocity(pull.normalize() * 0.75);
                    }
                    self.eat_timer = 0;
                    self.state = TongueState::CatchAnimation;
                } else if self.calculate_path_counter <= 0 {
                    mob.get_mob_entity()
                        .navigator
                        .lock()
                        .unwrap_or_else(std::sync::PoisonError::into_inner)
                        .set_progress(crate::entity::ai::pathfinder::NavigatorGoal::new(
                            entity.pos.load(),
                            target.get_entity().pos.load(),
                            TONGUE_SPEED,
                        ));
                    self.calculate_path_counter = 10;
                } else {
                    self.calculate_path_counter -= 1;
                }

                if self.move_ticks > TONGUE_MOVE_TIMEOUT && distance >= TONGUE_REACH * TONGUE_REACH
                {
                    self.state = TongueState::Done;
                }
            }
            TongueState::CatchAnimation => {
                self.eat_timer += 1;
                if self.eat_timer >= 6 {
                    self.state = TongueState::EatAnimation;
                    self.eat_entity(mob);
                }
            }
            TongueState::EatAnimation => {
                if self.eat_timer >= 10 {
                    self.state = TongueState::Done;
                } else {
                    self.eat_timer += 1;
                }
            }
            TongueState::Done => {}
        }
    }

    fn should_run_every_tick(&self) -> bool {
        true
    }

    fn controls(&self) -> Controls {
        Controls::MOVE | Controls::LOOK
    }
}

/// Vanilla `Frog.spawnChildFromBreeding`: no baby, the frog becomes pregnant and
/// lays frogspawn later.
struct FrogBreedGoal {
    frog: Weak<FrogEntity>,
    mate: Option<Arc<dyn EntityBase>>,
    timer: i32,
}

impl FrogBreedGoal {
    #[expect(
        clippy::unnecessary_box_returns,
        reason = "GoalSelector takes boxed goals"
    )]
    fn new(frog: &Arc<FrogEntity>) -> Box<Self> {
        Box::new(Self {
            frog: Arc::downgrade(frog),
            mate: None,
            timer: 0,
        })
    }

    fn find_mate(mob: &dyn Mob) -> Option<Arc<dyn EntityBase>> {
        let entity = mob.get_entity();
        let pos = entity.pos.load();
        let world = entity.world.load();
        let my_uuid = entity.entity_uuid;

        world
            .get_nearby_entities(pos, 8.0)
            .values()
            .filter(|candidate| {
                let candidate_entity = candidate.get_entity();
                candidate_entity.entity_uuid != my_uuid
                    && candidate_entity.entity_type == entity.entity_type
                    && candidate.is_in_love()
                    && candidate.is_breeding_ready()
                    && !candidate.is_panicking()
            })
            .min_by(|a, b| {
                let a = pos.squared_distance_to_vec(&a.get_entity().pos.load());
                let b = pos.squared_distance_to_vec(&b.get_entity().pos.load());
                a.total_cmp(&b)
            })
            .cloned()
    }

    fn breed(mob: &dyn Mob, mate: &dyn EntityBase, frog: &FrogEntity) {
        let mob_entity = mob.get_mob_entity();
        let entity = mob.get_entity();
        let world = entity.world.load();

        if let Some(player) = mob_entity
            .breeder
            .load()
            .and_then(|uuid| world.get_player_by_uuid(uuid))
        {
            let entity_type_name = entity.entity_type.resource_name;
            player.increment_stat(
                pumpkin_data::statistic::StatisticCategory::Custom,
                pumpkin_data::statistic::CustomStatistic::AnimalsBred as i32,
                1,
            );
            player.trigger_advancement_criterion(
                pumpkin_data::advancement::Advancement::HUSBANDRY_BREED_AN_ANIMAL,
                "bred",
            );
            player.trigger_advancement_criterion(
                pumpkin_data::advancement::Advancement::HUSBANDRY_BRED_ALL_ANIMALS,
                &format!("minecraft:{entity_type_name}"),
            );
        }

        mob_entity.reset_love_ticks();
        mob_entity.breeding_cooldown.store(6000, Ordering::Relaxed);
        mate.reset_love();
        mate.set_breeding_cooldown(6000);
        frog.set_pregnant(true);
        world.send_entity_status(entity, EntityStatus::InLoveHearts, None);
        ExperienceOrbEntity::spawn(&world, entity.pos.load(), rand::rng().random_range(1..8));
    }
}

impl Goal for FrogBreedGoal {
    fn can_start(&mut self, mob: &dyn Mob) -> bool {
        let Some(frog) = self.frog.upgrade() else {
            return false;
        };
        if frog.is_pregnant() || !mob.get_mob_entity().is_breeding_ready() || !mob.is_in_love() {
            return false;
        }
        self.mate = Self::find_mate(mob);
        self.mate.is_some()
    }

    fn should_continue(&mut self, _mob: &dyn Mob) -> bool {
        self.mate.as_ref().is_some_and(|mate| {
            mate.get_entity().is_alive()
                && !mate.is_panicking()
                && mate.is_in_love()
                && self.timer < 60
        })
    }

    fn start(&mut self, _mob: &dyn Mob) {
        self.timer = 0;
    }

    fn stop(&mut self, mob: &dyn Mob) {
        self.mate = None;
        self.timer = 0;
        mob.get_mob_entity()
            .navigator
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .stop();
    }

    fn tick(&mut self, mob: &dyn Mob) {
        let Some(mate) = &self.mate else {
            return;
        };
        let mob_entity = mob.get_mob_entity();
        let mate_pos = mate.get_entity().pos.load();
        let my_pos = mob.get_entity().pos.load();

        mob_entity
            .look_control
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .look_at_entity(mob, mate);
        mob_entity
            .navigator
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .set_progress(crate::entity::ai::pathfinder::NavigatorGoal::new(
                my_pos,
                mate_pos,
                BREED_SPEED,
            ));

        self.timer += 1;
        if self.timer >= self.get_tick_count(60)
            && my_pos.squared_distance_to_vec(&mate_pos) < 9.0
            && let Some(frog) = self.frog.upgrade()
        {
            Self::breed(mob, mate.as_ref(), &frog);
        }
    }

    fn should_run_every_tick(&self) -> bool {
        true
    }

    fn controls(&self) -> Controls {
        Controls::MOVE | Controls::LOOK
    }
}

const BREED_SPEED: f64 = 1.0;
const LAY_SPAWN_SEARCH: i32 = 8;
const LAY_SPAWN_SCAN_COOLDOWN: i32 = 20;
const LAY_SPAWN_REACH: f64 = 2.5;

/// Vanilla `TryFindLandNearLiquid` and `TryLaySpawnOnFluidNearLand`.
struct LayFrogSpawnGoal {
    frog: Weak<FrogEntity>,
    target: Option<BlockPos>,
    scan_cooldown: i32,
}

impl LayFrogSpawnGoal {
    #[expect(
        clippy::unnecessary_box_returns,
        reason = "GoalSelector takes boxed goals"
    )]
    fn new(frog: &Arc<FrogEntity>) -> Box<Self> {
        Box::new(Self {
            frog: Arc::downgrade(frog),
            target: None,
            scan_cooldown: 0,
        })
    }

    fn find_spawn(world: &World, origin: &BlockPos) -> Option<BlockPos> {
        let mut best: Option<(f64, BlockPos)> = None;

        for dx in -LAY_SPAWN_SEARCH..=LAY_SPAWN_SEARCH {
            for dy in -2..=2 {
                for dz in -LAY_SPAWN_SEARCH..=LAY_SPAWN_SEARCH {
                    let support = BlockPos::new(origin.0.x + dx, origin.0.y + dy, origin.0.z + dz);
                    let (block, _) = world.get_block_and_state(&support);
                    let fluid = world.get_fluid(&support);
                    if fluid.id != Fluid::EMPTY.id
                        && !fluid.has_tag(&tag::Fluid::MINECRAFT_SUPPORTS_FROGSPAWN)
                    {
                        continue;
                    }
                    if !fluid.has_tag(&tag::Fluid::MINECRAFT_SUPPORTS_FROGSPAWN)
                        && !block.has_tag(&tag::Block::MINECRAFT_SUPPORTS_FROGSPAWN)
                    {
                        continue;
                    }
                    if world.get_block_state(&support).is_solid() {
                        continue;
                    }
                    let spawn = support.up();
                    if !world.get_block_state(&spawn).is_air()
                        || world.get_block_state(&spawn.up()).is_solid()
                    {
                        continue;
                    }

                    let distance = spawn
                        .to_centered_f64()
                        .squared_distance_to_vec(&origin.to_centered_f64());
                    if best
                        .as_ref()
                        .is_none_or(|(best_distance, _)| distance < *best_distance)
                    {
                        best = Some((distance, spawn));
                    }
                }
            }
        }

        best.map(|(_, spawn)| spawn)
    }
}

impl Goal for LayFrogSpawnGoal {
    fn can_start(&mut self, mob: &dyn Mob) -> bool {
        let Some(frog) = self.frog.upgrade() else {
            return false;
        };
        if !frog.is_pregnant() {
            return false;
        }
        if self.scan_cooldown > 0 {
            self.scan_cooldown -= 1;
            return false;
        }
        self.scan_cooldown = LAY_SPAWN_SCAN_COOLDOWN;

        let entity = mob.get_entity();
        if entity.is_in_water() || !entity.on_ground.load(Ordering::Relaxed) {
            return false;
        }
        let world = entity.world.load();
        self.target = Self::find_spawn(&world, &entity.block_pos.load());
        self.target.is_some()
    }

    fn should_continue(&mut self, _mob: &dyn Mob) -> bool {
        self.target.is_some() && self.frog.upgrade().is_some_and(|frog| frog.is_pregnant())
    }

    fn start(&mut self, mob: &dyn Mob) {
        let Some(target) = self.target else {
            return;
        };
        let entity = mob.get_entity();
        mob.get_mob_entity()
            .navigator
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .set_progress(crate::entity::ai::pathfinder::NavigatorGoal::new(
                entity.pos.load(),
                target.to_centered_f64(),
                1.0,
            ));
    }

    fn stop(&mut self, mob: &dyn Mob) {
        self.target = None;
        mob.get_mob_entity()
            .navigator
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .stop();
    }

    fn tick(&mut self, mob: &dyn Mob) {
        let Some(target) = self.target else {
            return;
        };
        let Some(frog) = self.frog.upgrade() else {
            return;
        };
        let entity = mob.get_entity();
        let distance = target
            .to_centered_f64()
            .squared_distance_to_vec(&entity.pos.load());

        if distance > LAY_SPAWN_REACH * LAY_SPAWN_REACH {
            mob.get_mob_entity()
                .navigator
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .set_progress(crate::entity::ai::pathfinder::NavigatorGoal::new(
                    entity.pos.load(),
                    target.to_centered_f64(),
                    1.0,
                ));
            return;
        }

        let world = entity.world.load_full();
        if !world.get_block_state(&target).is_air() {
            self.target = None;
            return;
        }
        world.set_block_state(
            &target,
            Block::FROGSPAWN.default_state.id,
            BlockFlags::NOTIFY_ALL,
        );
        world.emit_game_event(GameEvent::BlockPlace, target.to_centered_f64());
        world.play_sound_fine(
            Sound::EntityFrogLaySpawn,
            SoundCategory::Blocks,
            &entity.pos.load(),
            1.0,
            1.0,
        );
        frog.set_pregnant(false);
        self.target = None;
    }

    fn should_run_every_tick(&self) -> bool {
        true
    }

    fn controls(&self) -> Controls {
        Controls::MOVE | Controls::LOOK
    }
}

impl AgeableMob for FrogEntity {
    fn get_ageable_data(&self) -> &AgeableData {
        &self.ageable_data
    }

    fn can_be_a_baby(&self) -> bool {
        false
    }
}

impl Animal for FrogEntity {
    fn is_food(&self, item_stack: &ItemStack) -> bool {
        item_stack.item.has_tag(&tag::Item::MINECRAFT_FROG_FOOD)
            || FROG_FOOD.iter().any(|i| i.id == item_stack.item.id)
    }
}

impl Mob for FrogEntity {
    fn as_ageable(&self) -> Option<&dyn AgeableMob> {
        Some(self)
    }

    fn as_animal(&self) -> Option<&dyn Animal> {
        Some(self)
    }

    fn mob_write_nbt(&self, nbt: &mut NbtCompound) {
        nbt.put_string("variant", self.get_variant().asset_id().to_string());
    }

    fn mob_read_nbt(&self, nbt: &NbtCompound) {
        if let Some(variant_str) = nbt.get_string("variant") {
            self.set_variant(FrogVariant::from_name(variant_str).unwrap_or_default());
        }
    }

    fn get_mob_entity(&self) -> &MobEntity {
        &self.mob_entity
    }

    fn mob_set_variant_name(&self, name: &str) {
        self.set_variant(FrogVariant::from_name(name).unwrap_or_default());
    }

    fn mob_tick(&self, _caller: &dyn EntityBase) {
        self.ageable_ai_step();
    }

    fn finalize_spawn(
        &self,
        world: &Arc<World>,
        group_data: Option<SpawnGroupData>,
    ) -> Option<SpawnGroupData> {
        let biome = world.get_biome(&self.get_entity().block_pos.load());
        self.set_variant(
            if biome.has_tag(&tag::WorldgenBiome::MINECRAFT_SPAWNS_COLD_VARIANT_FROGS) {
                FrogVariant::Cold
            } else if biome.has_tag(&tag::WorldgenBiome::MINECRAFT_SPAWNS_WARM_VARIANT_FROGS) {
                FrogVariant::Warm
            } else {
                FrogVariant::Temperate
            },
        );
        finalize_ageable_spawn(self, group_data)
    }

    fn mob_init_data_tracker(&self) {
        let entity = self.get_entity();
        let is_baby = entity.age.load(Ordering::Relaxed) < 0;
        if is_baby {
            entity.set_synced_data(pumpkin_data::tracked_data::frog::BABY_ID, true);
        }
        entity.set_synced_data(
            pumpkin_data::tracked_data::frog::VARIANT,
            VarInt(self.get_variant().id() as i32),
        );
        entity.set_synced_data(
            pumpkin_data::tracked_data::frog::TONGUE_TARGET_ID,
            OptionalInt(None),
        );
    }

    fn get_ambient_sound(&self) -> Option<Sound> {
        Some(Sound::EntityFrogAmbient)
    }

    fn mob_interact(&self, player: &Arc<Player>, item_stack: &mut ItemStack) -> bool {
        self.animal_interact(player, item_stack, Sound::EntityFrogEat)
    }
}
