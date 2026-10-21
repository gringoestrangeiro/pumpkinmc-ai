use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Weak};

use pumpkin_data::attributes::Attributes;
use pumpkin_data::data_component_impl::EquipmentSlot;
use pumpkin_data::entity::EntityType;
use pumpkin_data::item::Item;
use pumpkin_data::item_stack::ItemStack;
use pumpkin_data::sound::{Sound, SoundCategory};
use pumpkin_nbt::compound::NbtCompound;
use pumpkin_util::Difficulty;
use pumpkin_util::math::position::BlockPos;
use pumpkin_util::math::vector3::Vector3;
use rand::RngExt;

use crate::entity::{
    Entity, EntityBase,
    ai::goal::{
        Controls, Goal, ParentHandle,
        active_target::ActiveTargetGoal,
        move_to_target_pos::{MoveToTargetPos, MoveToTargetPosGoal},
        ranged_attack::RangedAttackGoal,
        swim::SwimGoal,
        zombie_attack::ZombieAttackGoal,
    },
    ai::pathfinder::node::PathType,
    ai::pathfinder::{Navigator, NavigatorGoal},
    ai::util::default_random_pos,
    living::LivingEntity,
    mob::{Mob, MobEntity, RangedAttackMob, zombie::ZombieEntityBase},
    passive::turtle::baby_on_land_selector,
    projectile::{arrow::ArrowPickup, trident::TridentEntity},
};
use crate::world::World;

pub struct DrownedEntity {
    entity: Arc<ZombieEntityBase>,
    searching_for_land: AtomicBool,
}

impl DrownedEntity {
    pub fn new(entity: Entity) -> Arc<Self> {
        Self::with_base(ZombieEntityBase::new(entity))
    }

    #[must_use]
    pub fn with_can_break_doors(entity: Entity, can_break_doors: bool) -> Arc<Self> {
        Self::with_base(ZombieEntityBase::with_can_break_doors(
            entity,
            can_break_doors,
        ))
    }

    #[allow(clippy::too_many_lines)]
    fn with_base(entity: Arc<ZombieEntityBase>) -> Arc<Self> {
        let drowned = Self {
            entity,
            searching_for_land: AtomicBool::new(false),
        };
        let mob_arc = Arc::new(drowned);
        let drowned_weak: Weak<Self> = Arc::downgrade(&mob_arc);
        let ranged_weak: Weak<dyn RangedAttackMob> = {
            let ranged_arc: Arc<dyn RangedAttackMob> = mob_arc.clone();
            Arc::downgrade(&ranged_arc)
        };

        {
            let mut navigator = mob_arc
                .entity
                .mob_entity
                .navigator
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            *navigator = Navigator::amphibious(false);
            navigator.set_pathfinding_malus(PathType::Water, 0.0);
        };

        mob_arc.entity.mob_entity.living_entity.update_attribute(
            &Attributes::STEP_HEIGHT,
            |step_height| {
                step_height.base_value = 1.0;
                step_height.dirty.store(true, Ordering::Relaxed);
            },
        );

        {
            let mut goal_selector = mob_arc
                .entity
                .mob_entity
                .goals_selector
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            let mut target_selector = mob_arc
                .entity
                .mob_entity
                .target_selector
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);

            let _ = goal_selector.remove_goals::<ZombieAttackGoal>();
            let _ = goal_selector.remove_goals::<SwimGoal>();

            goal_selector.add_goal(1, Box::new(DrownedGoToWaterGoal::new(1.0)));
            goal_selector.add_goal(2, Box::new(DrownedTridentAttackGoal::new(ranged_weak)));
            goal_selector.add_goal(2, DrownedAttackGoal::new(drowned_weak.clone()));
            goal_selector.add_goal(5, DrownedGoToBeachGoal::new(1.0, drowned_weak.clone()));
            goal_selector.add_goal(6, Box::new(DrownedSwimUpGoal::new(drowned_weak)));

            let _ = target_selector.remove_goals::<ActiveTargetGoal>();
            target_selector.add_goal(
                2,
                Box::new(ActiveTargetGoal::new(
                    &mob_arc.entity.mob_entity,
                    &EntityType::PLAYER,
                    10,
                    true,
                    false,
                    Some(|target: &LivingEntity, world: &World| {
                        !world.is_bright_outside() || target.is_in_water()
                    }),
                )),
            );
            target_selector.add_goal(
                3,
                ActiveTargetGoal::with_default(
                    &mob_arc.entity.mob_entity,
                    &EntityType::VILLAGER,
                    false,
                ),
            );
            target_selector.add_goal(
                3,
                ActiveTargetGoal::with_default(
                    &mob_arc.entity.mob_entity,
                    &EntityType::IRON_GOLEM,
                    true,
                ),
            );
            target_selector.add_goal(
                3,
                ActiveTargetGoal::with_default(
                    &mob_arc.entity.mob_entity,
                    &EntityType::AXOLOTL,
                    true,
                ),
            );
            target_selector.add_goal(
                5,
                Box::new(ActiveTargetGoal::new(
                    &mob_arc.entity.mob_entity,
                    &EntityType::TURTLE,
                    10,
                    true,
                    false,
                    Some(baby_on_land_selector),
                )),
            );
        };

        mob_arc
    }

    /// Vanilla `Drowned.okTarget`.
    #[must_use]
    pub fn ok_target(&self, target: Option<&Arc<dyn EntityBase>>) -> bool {
        let Some(target) = target else {
            return false;
        };
        let entity = &self.entity.mob_entity.living_entity.entity;
        !entity.world.load().is_bright_outside() || target.get_entity().is_in_water()
    }

    /// Vanilla `Drowned.wantsToSwim`.
    #[must_use]
    pub fn wants_to_swim(&self) -> bool {
        if self.searching_for_land.load(Ordering::Relaxed) {
            return true;
        }
        self.entity
            .mob_entity
            .get_target()
            .is_some_and(|target| target.get_entity().is_in_water())
    }

    pub fn set_searching_for_land(&self, searching: bool) {
        self.searching_for_land.store(searching, Ordering::Relaxed);
    }

    fn main_hand_item(&self) -> ItemStack {
        self.entity
            .mob_entity
            .living_entity
            .entity_equipment
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .get(&EquipmentSlot::MAIN_HAND)
    }

    /// Vanilla `Drowned.closeToNextPos`.
    fn close_to_next_pos(mob: &dyn Mob) -> bool {
        let navigator = mob
            .get_mob_entity()
            .navigator
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let Some(path) = navigator.get_path() else {
            return false;
        };
        let target = path.get_target();
        let target_pos = Vector3::new(
            f64::from(target.0.x),
            f64::from(target.0.y),
            f64::from(target.0.z),
        );
        mob.get_entity()
            .pos
            .load()
            .squared_distance_to_vec(&target_pos)
            < 4.0
    }
}

impl Mob for DrownedEntity {
    fn get_mob_entity(&self) -> &MobEntity {
        &self.entity.mob_entity
    }

    fn spawn_as_baby(&self) -> bool {
        self.entity.spawn_as_baby()
    }

    fn get_base_experience_reward(&self) -> u32 {
        self.entity.get_base_experience_reward()
    }

    fn mob_write_nbt(&self, nbt: &mut NbtCompound) {
        self.entity.mob_write_nbt(nbt);
    }

    fn mob_read_nbt(&self, nbt: &NbtCompound) {
        self.entity.mob_read_nbt(nbt);
    }

    fn custom_server_ai_step(&self, _caller: &dyn EntityBase) {
        let entity = &self.entity.mob_entity.living_entity.entity;
        // Vanilla `Drowned.updateSwimming`, called from `aiStep`.
        entity.set_swimming(entity.is_under_water() && self.wants_to_swim());
    }

    fn custom_travel(&self, caller: &dyn EntityBase) -> bool {
        let living = &self.entity.mob_entity.living_entity;
        let entity = &living.entity;
        if !entity.is_under_water() || !self.wants_to_swim() {
            return false;
        }

        // Vanilla `Drowned.travelInWater` while swimming underwater.
        entity.update_velocity_from_input(living.movement_input.load(), 0.01);
        living.make_move(caller);
        entity.velocity.store(entity.velocity.load() * 0.9);
        true
    }

    fn mob_is_pushed_by_fluids(&self) -> bool {
        !self.entity.mob_entity.living_entity.entity.is_swimming()
    }
}

impl RangedAttackMob for DrownedEntity {
    fn perform_ranged_attack(&self, target: &Arc<dyn EntityBase>, _power: f32) {
        let entity = &self.entity.mob_entity.living_entity.entity;
        let world = entity.world.load_full();

        let main_hand = self.main_hand_item();
        let stack = if main_hand.item.id == Item::TRIDENT.id {
            main_hand
        } else {
            ItemStack::new(1, &Item::TRIDENT)
        };
        let spawned_stack = stack.clone();

        let trident_entity = Entity::new(world.clone(), entity.pos.load(), &EntityType::TRIDENT);
        let trident =
            TridentEntity::new_shot(trident_entity, entity, stack, ArrowPickup::Disallowed);

        let target_entity = target.get_entity();
        let target_pos = target_entity.pos.load();
        let trident_pos = trident.get_entity().pos.load();
        let dx = target_pos.x - entity.pos.load().x;
        let dy = (target_pos.y + f64::from(target_entity.entity_dimension.load().height) / 3.0)
            - trident_pos.y;
        let dz = target_pos.z - entity.pos.load().z;
        let horizontal_distance = dx.hypot(dz);

        trident.set_velocity(
            dx,
            horizontal_distance.mul_add(0.2, dy),
            dz,
            1.6,
            f64::from(ranged_attack_uncertainty(
                world.level_info.load().difficulty,
            )),
        );
        trident.apply_on_projectile_spawned(&spawned_stack);

        if !entity.silent.load(Ordering::Relaxed) {
            let pitch = 1.0 / rand::random_range(0.8f32..1.2f32);
            world.play_sound_fine(
                Sound::EntityDrownedShoot,
                SoundCategory::Hostile,
                &entity.pos.load(),
                1.0,
                pitch,
            );
        }

        let trident: Arc<dyn EntityBase> = Arc::new(trident);
        world.spawn_entity(trident);
    }
}

const fn ranged_attack_uncertainty(difficulty: Difficulty) -> i32 {
    match difficulty {
        Difficulty::Peaceful => 14,
        Difficulty::Easy => 10,
        Difficulty::Normal => 6,
        Difficulty::Hard => 2,
    }
}

struct DrownedAttackGoal {
    inner: Box<ZombieAttackGoal>,
    drowned: Weak<DrownedEntity>,
}

impl DrownedAttackGoal {
    #[expect(
        clippy::unnecessary_box_returns,
        reason = "GoalSelector takes boxed goals"
    )]
    fn new(drowned: Weak<DrownedEntity>) -> Box<Self> {
        Box::new(Self {
            inner: ZombieAttackGoal::new(1.0, false),
            drowned,
        })
    }

    fn ok_target(&self, mob: &dyn Mob) -> bool {
        self.drowned
            .upgrade()
            .is_some_and(|drowned| drowned.ok_target(mob.get_mob_entity().get_target().as_ref()))
    }
}

impl Goal for DrownedAttackGoal {
    fn can_start(&mut self, mob: &dyn Mob) -> bool {
        self.ok_target(mob) && self.inner.can_start(mob)
    }

    fn should_continue(&mut self, mob: &dyn Mob) -> bool {
        self.ok_target(mob) && self.inner.should_continue(mob)
    }

    fn start(&mut self, mob: &dyn Mob) {
        self.inner.start(mob);
    }

    fn stop(&mut self, mob: &dyn Mob) {
        self.inner.stop(mob);
    }

    fn tick(&mut self, mob: &dyn Mob) {
        self.inner.tick(mob);
    }

    fn should_run_every_tick(&self) -> bool {
        self.inner.should_run_every_tick()
    }

    fn controls(&self) -> Controls {
        self.inner.controls()
    }
}

struct DrownedTridentAttackGoal {
    inner: RangedAttackGoal,
}

impl DrownedTridentAttackGoal {
    const fn new(ranged: Weak<dyn RangedAttackMob>) -> Self {
        Self {
            inner: RangedAttackGoal::new(ranged, 1.0, 40, 10.0),
        }
    }

    fn holding_trident(mob: &dyn Mob) -> bool {
        mob.get_mob_entity()
            .living_entity
            .entity_equipment
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .get(&EquipmentSlot::MAIN_HAND)
            .item
            .id
            == Item::TRIDENT.id
    }
}

impl Goal for DrownedTridentAttackGoal {
    fn can_start(&mut self, mob: &dyn Mob) -> bool {
        Self::holding_trident(mob) && self.inner.can_start(mob)
    }

    fn should_continue(&mut self, mob: &dyn Mob) -> bool {
        Self::holding_trident(mob) && self.inner.should_continue(mob)
    }

    fn start(&mut self, mob: &dyn Mob) {
        self.inner.start(mob);
        mob.get_mob_entity().set_attacking(true);
    }

    fn stop(&mut self, mob: &dyn Mob) {
        self.inner.stop(mob);
        mob.get_mob_entity().set_attacking(false);
    }

    fn tick(&mut self, mob: &dyn Mob) {
        self.inner.tick(mob);
    }

    fn should_run_every_tick(&self) -> bool {
        self.inner.should_run_every_tick()
    }

    fn controls(&self) -> Controls {
        self.inner.controls()
    }
}

/// Vanilla `Drowned.DrownedGoToWaterGoal`.
struct DrownedGoToWaterGoal {
    speed: f64,
    wanted: Option<Vector3<f64>>,
}

impl DrownedGoToWaterGoal {
    const fn new(speed: f64) -> Self {
        Self {
            speed,
            wanted: None,
        }
    }

    fn find_water_pos(mob: &dyn Mob) -> Option<Vector3<f64>> {
        let entity = mob.get_entity();
        let world = entity.world.load();
        let pos = entity.block_pos.load();

        for _ in 0..10 {
            let random_pos = BlockPos::new(
                pos.0.x + mob.get_random().random_range(-10..10),
                pos.0.y + 2 - mob.get_random().random_range(0..8),
                pos.0.z + mob.get_random().random_range(-10..10),
            );
            if world.get_block(&random_pos).id == pumpkin_data::Block::WATER.id {
                return Some(Vector3::new(
                    f64::from(random_pos.0.x) + 0.5,
                    f64::from(random_pos.0.y),
                    f64::from(random_pos.0.z) + 0.5,
                ));
            }
        }
        None
    }
}

impl Goal for DrownedGoToWaterGoal {
    fn can_start(&mut self, mob: &dyn Mob) -> bool {
        let entity = mob.get_entity();
        if !entity.world.load().is_bright_outside() || entity.is_in_water() {
            return false;
        }
        self.wanted = Self::find_water_pos(mob);
        self.wanted.is_some()
    }

    fn should_continue(&mut self, mob: &dyn Mob) -> bool {
        !mob.is_navigator_idle()
    }

    fn start(&mut self, mob: &dyn Mob) {
        if let Some(wanted) = self.wanted {
            mob.get_mob_entity()
                .navigator
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .set_progress(NavigatorGoal::new(
                    mob.get_entity().pos.load(),
                    wanted,
                    self.speed,
                ));
        }
    }

    fn controls(&self) -> Controls {
        Controls::MOVE
    }
}

/// Vanilla `Drowned.DrownedGoToBeachGoal`.
struct DrownedGoToBeachGoal {
    inner: MoveToTargetPosGoal<Self>,
    drowned: Weak<DrownedEntity>,
}

impl DrownedGoToBeachGoal {
    #[expect(
        clippy::unnecessary_box_returns,
        reason = "GoalSelector takes boxed goals"
    )]
    fn new(speed: f64, drowned: Weak<DrownedEntity>) -> Box<Self> {
        let mut this = Box::new(Self {
            inner: MoveToTargetPosGoal::new(ParentHandle::none(), speed, 8, 2),
            drowned,
        });
        // SAFETY: `this` is heap allocated in a `Box` and outlives the handle.
        this.inner.move_to_target_pos = unsafe { ParentHandle::new(&*this) };
        this
    }
}

impl Goal for DrownedGoToBeachGoal {
    fn can_start(&mut self, mob: &dyn Mob) -> bool {
        let entity = mob.get_entity();
        let world = entity.world.load();
        !world.is_bright_outside()
            && entity.is_in_water()
            && entity.pos.load().y >= f64::from(world.sea_level - 3)
            && self.inner.can_start(mob)
    }

    fn should_continue(&mut self, mob: &dyn Mob) -> bool {
        self.inner.should_continue(mob)
    }

    fn start(&mut self, mob: &dyn Mob) {
        if let Some(drowned) = self.drowned.upgrade() {
            drowned.set_searching_for_land(false);
        }
        self.inner.start(mob);
    }

    fn stop(&mut self, mob: &dyn Mob) {
        self.inner.stop(mob);
    }

    fn tick(&mut self, mob: &dyn Mob) {
        self.inner.tick(mob);
    }

    fn should_run_every_tick(&self) -> bool {
        self.inner.should_run_every_tick()
    }

    fn controls(&self) -> Controls {
        self.inner.controls()
    }
}

impl MoveToTargetPos for DrownedGoToBeachGoal {
    fn is_target_pos(&self, world: Arc<World>, block_pos: BlockPos) -> bool {
        let above = block_pos.up();
        world.get_block_state(&above).is_air()
            && world.get_block_state(&above.up()).is_air()
            && world.get_block_state(&block_pos).is_solid_block()
    }
}

/// Vanilla `Drowned.DrownedSwimUpGoal`.
struct DrownedSwimUpGoal {
    drowned: Weak<DrownedEntity>,
    speed: f64,
    stuck: bool,
}

impl DrownedSwimUpGoal {
    const fn new(drowned: Weak<DrownedEntity>) -> Self {
        Self {
            drowned,
            speed: 1.0,
            stuck: false,
        }
    }
}

impl Goal for DrownedSwimUpGoal {
    fn can_start(&mut self, mob: &dyn Mob) -> bool {
        let entity = mob.get_entity();
        let world = entity.world.load();
        !world.is_bright_outside()
            && entity.is_in_water()
            && entity.pos.load().y < f64::from(world.sea_level - 2)
    }

    fn should_continue(&mut self, mob: &dyn Mob) -> bool {
        !self.stuck && self.can_start(mob)
    }

    fn start(&mut self, _mob: &dyn Mob) {
        if let Some(drowned) = self.drowned.upgrade() {
            drowned.set_searching_for_land(true);
        }
        self.stuck = false;
    }

    fn stop(&mut self, _mob: &dyn Mob) {
        if let Some(drowned) = self.drowned.upgrade() {
            drowned.set_searching_for_land(false);
        }
    }

    fn tick(&mut self, mob: &dyn Mob) {
        let entity = mob.get_entity();
        let world = entity.world.load();
        let sea_level = world.sea_level;
        let pos = entity.pos.load();

        if pos.y < f64::from(sea_level - 1)
            && (mob.is_navigator_idle() || DrownedEntity::close_to_next_pos(mob))
        {
            let towards_pos = Vector3::new(pos.x, f64::from(sea_level - 1), pos.z);
            let Some(next_pos) = default_random_pos::get_pos_towards(
                mob,
                4,
                8,
                towards_pos,
                std::f64::consts::FRAC_PI_2,
            ) else {
                self.stuck = true;
                return;
            };
            mob.get_mob_entity()
                .navigator
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .set_progress(NavigatorGoal::new(pos, next_pos, self.speed));
        }
    }
}
