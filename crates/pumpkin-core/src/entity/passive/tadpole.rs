use std::sync::atomic::{AtomicBool, AtomicI32, Ordering};
use std::sync::{Arc, Weak};

use pumpkin_data::entity::EntityType;
use pumpkin_data::item::Item;
use pumpkin_data::item_stack::ItemStack;
use pumpkin_data::particle::Particle;
use pumpkin_data::sound::{Sound, SoundCategory};
use pumpkin_data::tag::{self, Taggable};
use pumpkin_nbt::compound::NbtCompound;
use pumpkin_util::math::vector3::Vector3;

use crate::entity::{
    Entity, EntityBase,
    ai::goal::{
        escape_danger::EscapeDangerGoal, look_around::RandomLookAroundGoal,
        look_at_entity::LookAtEntityGoal, tempt::TemptGoal, wander_around::WanderAroundGoal,
    },
    mob::{Mob, MobEntity},
    passive::fish,
    player::Player,
};

/// Vanilla `Tadpole.ticksToBeFrog`.
const TICKS_TO_BE_FROG: i32 = 24_000;
const TEMPT_ITEMS: &[&Item] = &[&Item::SLIME_BALL];

pub struct TadpoleEntity {
    pub mob_entity: MobEntity,
    age: AtomicI32,
    age_locked: AtomicBool,
}

impl TadpoleEntity {
    pub fn new(entity: Entity) -> Arc<Self> {
        let mob_entity = MobEntity::new(entity);
        fish::init_movement(&mob_entity);
        let tadpole = Self {
            mob_entity,
            age: AtomicI32::new(0),
            age_locked: AtomicBool::new(false),
        };
        let mob_arc = Arc::new(tadpole);
        let mob_weak: Weak<dyn Mob> = {
            let mob_arc: Arc<dyn Mob> = mob_arc.clone();
            Arc::downgrade(&mob_arc)
        };

        {
            let mut goal_selector = mob_arc
                .mob_entity
                .goals_selector
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);

            goal_selector.add_goal(1, EscapeDangerGoal::new(2.0));
            goal_selector.add_goal(2, Box::new(TemptGoal::new(1.25, TEMPT_ITEMS, false)));
            goal_selector.add_goal(3, Box::new(WanderAroundGoal::swimming(0.5, 40)));
            goal_selector.add_goal(
                4,
                LookAtEntityGoal::with_default(mob_weak, &EntityType::PLAYER, 6.0),
            );
            goal_selector.add_goal(5, Box::new(RandomLookAroundGoal::default()));
        };

        mob_arc
    }

    fn is_food(item_stack: &ItemStack) -> bool {
        item_stack.item.has_tag(&tag::Item::MINECRAFT_FROG_FOOD)
    }

    fn age(&self) -> i32 {
        self.age.load(Ordering::Relaxed)
    }

    fn set_age(&self, new_age: i32) {
        self.age.store(new_age, Ordering::Relaxed);
        if new_age >= TICKS_TO_BE_FROG {
            self.age_up();
        }
    }

    fn set_age_locked(&self, locked: bool) {
        self.age_locked.store(locked, Ordering::Relaxed);
        self.get_entity()
            .set_synced_data(pumpkin_data::tracked_data::tadpole::AGE_LOCKED, locked);
    }

    /// Vanilla `AgeableMob.getSpeedUpSecondsWhenFeeding`.
    const fn get_speed_up_seconds_when_feeding(ticks_until_adult: i32) -> i32 {
        (ticks_until_adult as f32 / 20.0 * 0.1) as i32
    }

    fn feed(&self, player: &Arc<Player>, item_stack: &mut ItemStack) {
        item_stack.decrement_unless_creative(player.gamemode.load(), 1);

        let ticks_left = (TICKS_TO_BE_FROG - self.age()).max(0);
        self.set_age(self.age() + Self::get_speed_up_seconds_when_feeding(ticks_left) * 20);

        let entity = self.get_entity();
        let world = entity.world.load();
        let pos = entity.pos.load();
        world.spawn_particle(
            pos + Vector3::new(0.0, f64::from(entity.height()), 0.0),
            Vector3::new(0.5, 0.5, 0.5),
            1.0,
            1,
            Particle::HappyVillager,
        );
    }

    fn age_up(&self) {
        let entity = self.get_entity();
        if entity.is_removed() {
            return;
        }
        let world = entity.world.load();
        let pos = entity.pos.load();
        let frog =
            crate::entity::r#type::from_type(&EntityType::FROG, pos, &world, uuid::Uuid::new_v4());

        let frog_entity = frog.get_entity();
        frog_entity.set_rotation(entity.yaw.load(), entity.pitch.load());
        frog_entity.head_yaw.store(entity.head_yaw.load());
        frog_entity.velocity.store(entity.velocity.load());
        if let Some(custom_name) = &**entity.custom_name.load() {
            frog_entity.set_custom_name(custom_name.clone());
        }
        if let Some(mob) = frog.get_mob() {
            mob.get_mob_entity()
                .persistence_required
                .store(true, Ordering::Relaxed);
        }

        world.play_sound_fine(
            Sound::EntityTadpoleGrowUp,
            SoundCategory::Neutral,
            &pos,
            0.15,
            1.0,
        );
        world.spawn_entity(frog);
        entity.remove();
    }
}

impl Mob for TadpoleEntity {
    fn get_mob_entity(&self) -> &MobEntity {
        &self.mob_entity
    }

    fn mob_write_nbt(&self, nbt: &mut NbtCompound) {
        nbt.put_int("Age", self.age());
        nbt.put_bool("AgeLocked", self.age_locked.load(Ordering::Relaxed));
    }

    fn mob_read_nbt(&self, nbt: &NbtCompound) {
        self.set_age(nbt.get_int("Age").unwrap_or(0));
        self.set_age_locked(nbt.get_bool("AgeLocked").unwrap_or(false));
    }

    fn mob_init_data_tracker(&self) {
        self.get_entity().set_synced_data(
            pumpkin_data::tracked_data::tadpole::AGE_LOCKED,
            self.age_locked.load(Ordering::Relaxed),
        );
    }

    fn mob_tick(&self, _caller: &dyn EntityBase) {
        fish::flop(self, Sound::EntityTadpoleFlop);
        if !self.age_locked.load(Ordering::Relaxed) {
            self.set_age(self.age() + 1);
        }
    }

    fn custom_travel(&self, caller: &dyn EntityBase) -> bool {
        fish::travel(self, caller)
    }

    fn mob_is_pushed_by_fluids(&self) -> bool {
        false
    }

    fn get_base_experience_reward(&self) -> u32 {
        0
    }

    fn mob_interact(&self, player: &Arc<Player>, item_stack: &mut ItemStack) -> bool {
        if Self::is_food(item_stack) && !self.age_locked.load(Ordering::Relaxed) {
            self.feed(player, item_stack);
            return true;
        }

        if item_stack.get_item() == &Item::WATER_BUCKET {
            let entity = self.get_entity();
            let world = entity.world.load();
            if let Some(server) = world.server.upgrade() {
                let mut event = crate::plugin::api::events::player::player_bucket_entity::PlayerBucketEntityEvent {
                    player: player.clone(),
                    entity_id: entity.entity_id,
                    bucket_item: "tadpole_bucket".to_string(),
                    cancelled: false,
                };
                server.plugin_manager.fire_blocking(&server, &mut event);
                if event.cancelled {
                    return true;
                }
            }
            crate::item::items::bucket::fill_mob_bucket_stack(
                item_stack,
                player,
                &Item::TADPOLE_BUCKET,
                self,
            );
            world.play_sound(
                Sound::ItemBucketFillTadpole,
                SoundCategory::Neutral,
                &entity.pos.load(),
            );
            entity.remove();
            return true;
        }

        false
    }
}
