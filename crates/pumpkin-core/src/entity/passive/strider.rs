use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Weak};

use pumpkin_data::Block;
use pumpkin_data::attributes::Attributes;
use pumpkin_data::entity::EntityType;
use pumpkin_data::fluid::Fluid;
use pumpkin_data::item::Item;
use pumpkin_data::item_stack::ItemStack;
use pumpkin_data::sound::{Sound, SoundCategory};
use pumpkin_data::tag::{self, Taggable};
use pumpkin_nbt::compound::NbtCompound;
use pumpkin_util::math::position::BlockPos;
use pumpkin_util::math::vector3::Vector3;
use rand::RngExt;

use crate::block::PathComputationType;
use crate::entity::ai::pathfinder::node::PathType;
use crate::entity::{
    Entity, EntityBase,
    ageable::{AgeableData, AgeableMob},
    ai::goal::{
        Controls, Goal, ParentHandle,
        breed::BreedGoal,
        escape_danger::EscapeDangerGoal,
        follow_parent::FollowParentGoal,
        look_around::RandomLookAroundGoal,
        look_at_entity::LookAtEntityGoal,
        move_to_target_pos::{MoveToTargetPos, MoveToTargetPosGoal},
        swim::SwimGoal,
        tempt::TemptGoal,
        wander_around::WanderAroundGoal,
    },
    attributes::{Modifier, ModifierOperation},
    item_steerable::{ItemBasedSteering, ItemSteerable},
    mob::{Mob, MobEntity},
    passive::animal::Animal,
    player::Player,
};
use crate::world::World;

const TEMPT_ITEMS: &[&Item] = &[&Item::WARPED_FUNGUS, &Item::WARPED_FUNGUS_ON_A_STICK];
const SUFFOCATING_MODIFIER_ID: &str = "minecraft:suffocating";
const SUFFOCATING_MODIFIER_AMOUNT: f64 = -0.34;

pub struct StriderEntity {
    pub mob_entity: MobEntity,
    pub ageable_data: AgeableData,
    pub steering: ItemBasedSteering,
    pub saddled: AtomicBool,
    pub suffocating: AtomicBool,
}

impl StriderEntity {
    pub fn new(entity: Entity) -> Arc<Self> {
        let mob_entity = MobEntity::new(entity);
        let strider = Self {
            mob_entity,
            ageable_data: AgeableData::default(),
            steering: ItemBasedSteering::default(),
            saddled: AtomicBool::new(false),
            suffocating: AtomicBool::new(false),
        };
        let mob_arc = Arc::new(strider);
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
            navigator.set_pathfinding_malus(PathType::Water, -1.0);
            navigator.set_pathfinding_malus(PathType::Lava, 0.0);
            navigator.set_pathfinding_malus(PathType::DangerFire, 0.0);
            navigator.set_pathfinding_malus(PathType::DamageFire, 0.0);
        };

        {
            let mut goal_selector = mob_arc
                .mob_entity
                .goals_selector
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);

            goal_selector.add_goal(0, Box::new(SwimGoal::default()));
            goal_selector.add_goal(1, EscapeDangerGoal::new(1.65));
            goal_selector.add_goal(2, BreedGoal::new(1.0));
            goal_selector.add_goal(3, Box::new(TemptGoal::new(1.4, TEMPT_ITEMS, false)));
            goal_selector.add_goal(4, StriderGoToLavaGoal::new());
            goal_selector.add_goal(5, Box::new(FollowParentGoal::new(1.0)));
            goal_selector.add_goal(7, Box::new(WanderAroundGoal::with_interval(1.0, 60)));
            goal_selector.add_goal(
                8,
                LookAtEntityGoal::with_default(mob_weak.clone(), &EntityType::PLAYER, 8.0),
            );
            goal_selector.add_goal(8, Box::new(RandomLookAroundGoal::default()));
            goal_selector.add_goal(
                9,
                LookAtEntityGoal::with_default(mob_weak, &EntityType::STRIDER, 8.0),
            );
        };

        mob_arc
    }

    #[must_use]
    pub fn is_suffocating(&self) -> bool {
        self.suffocating.load(Ordering::Relaxed)
    }

    pub fn set_suffocating(&self, suffocating: bool) {
        if self.suffocating.swap(suffocating, Ordering::Relaxed) == suffocating {
            return;
        }
        let entity = self.get_entity();
        entity.set_synced_data(
            pumpkin_data::tracked_data::strider::DATA_SUFFOCATING,
            suffocating,
        );
        let mut attributes = self
            .mob_entity
            .living_entity
            .attributes
            .write()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if let Some(speed) = attributes.get_mut(&Attributes::MOVEMENT_SPEED.id) {
            if suffocating {
                speed.add_or_replace_modifier(Modifier {
                    id: SUFFOCATING_MODIFIER_ID.to_string(),
                    amount: SUFFOCATING_MODIFIER_AMOUNT,
                    operation: ModifierOperation::MultiplyBase,
                });
            } else {
                speed.remove_modifier(SUFFOCATING_MODIFIER_ID);
            }
        }
    }

    fn is_warm_block(&self, pos: &BlockPos) -> bool {
        self.get_entity()
            .world
            .load()
            .get_block(pos)
            .has_tag(&tag::Block::MINECRAFT_STRIDER_WARM_BLOCKS)
    }

    fn update_suffocating(&self) {
        let entity = self.get_entity();
        let inside = entity.block_pos.load();
        let bounding_box = entity.bounding_box.load();
        let on_block = BlockPos::containing(
            entity.pos.load().x,
            bounding_box.min.y - 0.2,
            entity.pos.load().z,
        );
        let in_warm_blocks = self.is_warm_block(&inside)
            || self.is_warm_block(&on_block)
            || entity.lava_height.load() > 0.0;
        let on_warm_strider = entity.get_vehicle().is_some_and(|vehicle| {
            vehicle
                .cast_any()
                .downcast_ref::<Self>()
                .is_some_and(|strider| !strider.is_suffocating())
        });
        self.set_suffocating(!in_warm_blocks && !on_warm_strider);
    }

    fn float_strider(&self) {
        let entity = self.get_entity();
        if !entity.touching_lava.load(Ordering::Relaxed) {
            return;
        }
        let position = entity.block_pos.load();
        let world = entity.world.load();
        let fluid_above = world.get_fluid(&position.up());
        let lava_above =
            fluid_above.id == Fluid::LAVA.id || fluid_above.id == Fluid::FLOWING_LAVA.id;
        let bounding_box = entity.bounding_box.load();
        let surface = f64::from(position.0.y) + 0.5;

        if lava_above {
            let velocity = entity.velocity.load();
            entity.velocity.store(Vector3::new(
                velocity.x * 0.5,
                velocity.y * 0.5 + 0.05,
                velocity.z * 0.5,
            ));
        } else if bounding_box.min.y < surface {
            // Pumpkin has no liquid collision shapes, so stand the strider on the
            // 0.5-high lava surface `getLiquidCollisionShape` would provide.
            let mut new_position = entity.pos.load();
            new_position.y = surface;
            entity.set_pos(new_position);
            entity.on_ground.store(true, Ordering::SeqCst);
            let mut velocity = entity.velocity.load();
            velocity.y = 0.0;
            entity.velocity.store(velocity);
        } else {
            entity.on_ground.store(true, Ordering::SeqCst);
            let mut velocity = entity.velocity.load();
            if velocity.y < 0.0 {
                velocity.y = 0.0;
                entity.velocity.store(velocity);
            }
        }
    }

    fn tempt_running(&self) -> bool {
        self.mob_entity
            .goals_selector
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .is_running::<TemptGoal>()
    }

    fn panic_running(&self) -> bool {
        self.mob_entity
            .goals_selector
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .is_running::<EscapeDangerGoal>()
    }
}

/// Vanilla `Strider.StriderGoToLavaGoal`.
struct StriderGoToLavaGoal {
    inner: MoveToTargetPosGoal<Self>,
}

impl StriderGoToLavaGoal {
    #[expect(
        clippy::unnecessary_box_returns,
        reason = "GoalSelector takes boxed goals"
    )]
    fn new() -> Box<Self> {
        let mut inner = MoveToTargetPosGoal::with_default(ParentHandle::none(), 1.0, 8);
        inner.max_y_difference = 2;
        let mut this = Box::new(Self { inner });
        // SAFETY: `this` is heap allocated in a `Box` and outlives the handle.
        this.inner.move_to_target_pos = unsafe { ParentHandle::new(&*this) };
        this
    }
}

impl Goal for StriderGoToLavaGoal {
    fn can_start(&mut self, mob: &dyn Mob) -> bool {
        !mob.get_entity().touching_lava.load(Ordering::Relaxed) && self.inner.can_start(mob)
    }

    fn should_continue(&mut self, mob: &dyn Mob) -> bool {
        !mob.get_entity().touching_lava.load(Ordering::Relaxed) && self.inner.should_continue(mob)
    }

    fn start(&mut self, mob: &dyn Mob) {
        self.inner.start(mob);
    }

    fn stop(&mut self, mob: &dyn Mob) {
        self.inner.stop(mob);
    }

    fn tick(&mut self, mob: &dyn Mob) {
        self.inner.tick(mob);
        if self.inner.trying_time % 20 == 0 && !self.inner.is_reached_target() {
            self.inner.move_mob_to_block(mob);
        }
    }

    fn should_run_every_tick(&self) -> bool {
        true
    }

    fn controls(&self) -> Controls {
        self.inner.controls()
    }
}

impl MoveToTargetPos for StriderGoToLavaGoal {
    fn is_target_pos(&self, world: Arc<World>, block_pos: BlockPos) -> bool {
        if world.get_block(&block_pos).id != Block::LAVA.id {
            return false;
        }
        let (block, state) = world.get_block_and_state_id(&block_pos.up());
        world
            .block_registry
            .is_pathfindable(block, state.to_state(), PathComputationType::Land)
    }
}

impl AgeableMob for StriderEntity {
    fn get_ageable_data(&self) -> &AgeableData {
        &self.ageable_data
    }
}

impl Animal for StriderEntity {
    fn is_food(&self, item_stack: &ItemStack) -> bool {
        item_stack.item.has_tag(&tag::Item::MINECRAFT_STRIDER_FOOD)
            || item_stack.item == &Item::WARPED_FUNGUS
    }
}

impl Mob for StriderEntity {
    fn as_ageable(&self) -> Option<&dyn AgeableMob> {
        Some(self)
    }

    fn as_animal(&self) -> Option<&dyn Animal> {
        Some(self)
    }

    fn mob_write_nbt(&self, nbt: &mut NbtCompound) {
        self.write_ageable_nbt(nbt);
        nbt.put_bool("Saddle", self.is_saddled());
    }

    fn mob_read_nbt(&self, nbt: &NbtCompound) {
        self.read_ageable_nbt(nbt);
        if let Some(saddle) = nbt.get_bool("Saddle") {
            self.set_saddled(saddle);
        } else if let Some(saddle_byte) = nbt.get_byte("Saddle") {
            self.set_saddled(saddle_byte == 1);
        }
    }

    fn get_mob_entity(&self) -> &MobEntity {
        &self.mob_entity
    }

    fn get_item_steerable(&self) -> Option<&dyn ItemSteerable> {
        Some(self)
    }

    fn is_saddled(&self) -> bool {
        self.saddled.load(Ordering::Relaxed)
    }

    fn can_be_saddled(&self) -> bool {
        self.mob_entity.living_entity.entity.is_alive() && !self.is_baby()
    }

    fn set_saddled(&self, saddled: bool) {
        self.saddled.store(saddled, Ordering::Relaxed);
    }

    fn mob_tick(&self, _caller: &dyn EntityBase) {
        self.update_suffocating();
        if self.tempt_running() && rand::rng().random_range(0..140) == 0 {
            self.get_entity().world.load().play_sound(
                Sound::EntityStriderHappy,
                SoundCategory::Neutral,
                &self.get_entity().pos.load(),
            );
        } else if self.panic_running() && rand::rng().random_range(0..60) == 0 {
            self.get_entity().world.load().play_sound(
                Sound::EntityStriderRetreat,
                SoundCategory::Neutral,
                &self.get_entity().pos.load(),
            );
        }
        if self.get_entity().has_passengers() {
            self.steering.tick_boost();
        }
        self.ageable_ai_step();
    }

    fn post_tick(&self) {
        self.float_strider();
    }

    fn mob_init_data_tracker(&self) {
        let entity = self.get_entity();
        let is_baby = entity.age.load(Ordering::Relaxed) < 0;
        if is_baby {
            entity.set_synced_data(pumpkin_data::tracked_data::strider::DATA_BABY_ID, true);
        }
        entity.set_synced_data(
            pumpkin_data::tracked_data::strider::DATA_SUFFOCATING,
            self.is_suffocating(),
        );
        entity.set_synced_data(
            pumpkin_data::tracked_data::strider::DATA_BOOST_TIME,
            self.steering.boost_time_total.load(Ordering::Relaxed),
        );
    }

    fn get_ambient_sound(&self) -> Option<Sound> {
        if self.tempt_running() || self.panic_running() {
            None
        } else {
            Some(Sound::EntityStriderAmbient)
        }
    }

    fn mob_interact(&self, player: &Arc<Player>, item_stack: &mut ItemStack) -> bool {
        let item = item_stack.get_item();

        if item == &Item::SADDLE && self.can_be_saddled() && !self.is_saddled() {
            self.set_saddled(true);
            item_stack.decrement_unless_creative(player.gamemode.load(), 1);
            let entity = self.get_entity();
            let world = entity.world.load();
            let pos = entity.pos.load();
            world.play_sound(Sound::EntityStriderSaddle, SoundCategory::Neutral, &pos);
            return true;
        }

        if self.is_saddled() && !self.is_food(item_stack) && !self.get_entity().has_passengers() {
            let world = player.world();
            let ent = &self.mob_entity.living_entity.entity;
            if let Some(vehicle) = world.get_entity_by_id(ent.entity_id)
                && let Some(passenger) = world.get_player_by_id(player.entity_id())
            {
                ent.add_passenger(vehicle, passenger as Arc<dyn EntityBase>);
                return true;
            }
        }

        self.animal_interact(player, item_stack, Sound::EntityStriderEat)
    }
}

impl ItemSteerable for StriderEntity {
    fn boost(&self) -> bool {
        if !self.steering.boost() {
            return false;
        }
        self.get_entity().set_synced_data(
            pumpkin_data::tracked_data::strider::DATA_BOOST_TIME,
            self.steering.boost_time_total.load(Ordering::Relaxed),
        );
        true
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}
