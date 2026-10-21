use std::sync::{
    Arc, Mutex,
    atomic::{AtomicBool, Ordering},
};

use crate::entity::projectile::ThrownItemEntity;
use crate::entity::projectile::arrow::{ArrowEntity, ArrowPickup};
use crate::entity::projectile::egg::EggEntity;
use crate::entity::projectile::experience_bottle::ExperienceBottleEntity;
use crate::entity::projectile::lingering_potion::LingeringPotionEntity;
use crate::entity::projectile::small_fireball::SmallFireballEntity;
use crate::entity::projectile::snowball::SnowballEntity;
use crate::entity::projectile::splash_potion::SplashPotionEntity;
use crate::entity::projectile::wind_charge::{WIND_CHARGE_GRAVITY, WindChargeEntity};
use crate::entity::{Entity, EntityBase, item::ItemEntity, living::LivingEntity};
use crate::server::Server;
use pumpkin_data::damage::DamageType;
use pumpkin_data::entity::EntityType;
use pumpkin_data::game_event::GameEvent;
use pumpkin_data::item::Item;
use pumpkin_data::item_stack::ItemStack;
use pumpkin_data::sound::{Sound, SoundCategory};
use pumpkin_data::world::WorldEvent;
use pumpkin_nbt::compound::NbtCompound;
use pumpkin_protocol::codec::item_stack_seralizer::ItemStackSerializer;
use pumpkin_util::math::vector3::Vector3;
use rand::RngExt;

const SPAWN_ITEM_DELAY_MIN: i64 = 60;
const SPAWN_ITEM_DELAY_MAX: i64 = 120;
const TICKS_BEFORE_ABOUT_TO_SPAWN_SOUND: i64 = 36;
const DEFAULT_PROJECTILE_POWER: f64 = 1.1;
const DEFAULT_PROJECTILE_UNCERTAINTY: f64 = 6.0;
const POTION_PROJECTILE_POWER: f64 = 1.375;
const POTION_PROJECTILE_UNCERTAINTY: f64 = 3.0;

/// Vanilla `OminousItemSpawner`: drops an item or fires a projectile downwards
/// after a random delay. Trial spawners place it during an ominous trial.
pub struct OminousItemSpawnerEntity {
    pub entity: Entity,
    item: Mutex<ItemStack>,
    spawn_item_after_ticks: Mutex<i64>,
}

impl OminousItemSpawnerEntity {
    pub fn new(entity: Entity) -> Self {
        entity.no_physics.store(true, Ordering::Relaxed);
        Self {
            entity,
            item: Mutex::new(ItemStack::EMPTY.clone()),
            spawn_item_after_ticks: Mutex::new(
                rand::rng().random_range(SPAWN_ITEM_DELAY_MIN..=SPAWN_ITEM_DELAY_MAX),
            ),
        }
    }

    pub fn item(&self) -> ItemStack {
        self.item
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .clone()
    }

    fn set_item(&self, item: ItemStack) {
        *self
            .item
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner) = item.clone();
        self.entity.set_synced_data(
            pumpkin_data::tracked_data::ominous_item_spawner::DATA_ITEM,
            ItemStackSerializer::from(item),
        );
    }

    fn spawn_item_after_ticks(&self) -> i64 {
        *self
            .spawn_item_after_ticks
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }

    fn spawn_item(&self) {
        let item = self.item();
        if item.is_empty() {
            return;
        }

        let world = self.entity.world.load();
        let pos = self.entity.pos.load();
        let spawned: Arc<dyn EntityBase> = self.spawn_projectile(&item).map_or_else(
            || -> Arc<dyn EntityBase> {
                let entity = Entity::new(world.clone(), pos, &EntityType::ITEM);
                Arc::new(ItemEntity::new(entity, item))
            },
            |projectile| projectile,
        );

        world.spawn_entity(spawned.clone());
        world.sync_world_event(
            WorldEvent::ParticlesTrialSpawnerSpawnItem,
            self.entity.block_pos.load(),
            1,
        );
        world.emit_game_event_with_source(GameEvent::EntityPlace, pos, Some(spawned.as_ref()));
        self.set_item(ItemStack::EMPTY.clone());
    }

    fn spawn_projectile(&self, item: &ItemStack) -> Option<Arc<dyn EntityBase>> {
        let world = self.entity.world.load();
        let pos = self.entity.pos.load();
        let owner_id = self.entity.entity_id;
        let (power, uncertainty) = match item.item.id {
            id if id == Item::SPLASH_POTION.id || id == Item::LINGERING_POTION.id => {
                (POTION_PROJECTILE_POWER, POTION_PROJECTILE_UNCERTAINTY)
            }
            _ => (DEFAULT_PROJECTILE_POWER, DEFAULT_PROJECTILE_UNCERTAINTY),
        };

        let projectile: Arc<dyn EntityBase> = match item.item.id {
            id if id == Item::ARROW.id
                || id == Item::TIPPED_ARROW.id
                || id == Item::SPECTRAL_ARROW.id =>
            {
                let entity = Entity::new(
                    world.clone(),
                    pos,
                    ArrowEntity::entity_type_for_item(item.item),
                );
                let arrow =
                    ArrowEntity::new_with_item(entity, Some(owner_id), item, ArrowPickup::Allowed);
                arrow.apply_on_projectile_spawned(item);
                arrow.set_velocity(0.0, -1.0, 0.0, power, uncertainty);
                Arc::new(arrow)
            }
            id if id == Item::SNOWBALL.id => {
                let entity = Entity::new(world.clone(), pos, &EntityType::SNOWBALL);
                let mut snowball = SnowballEntity::new(entity);
                snowball.thrown.owner_id = Some(owner_id);
                snowball
                    .thrown
                    .set_velocity(0.0, -1.0, 0.0, power, uncertainty);
                Arc::new(snowball)
            }
            id if id == Item::EGG.id => {
                let entity = Entity::new(world.clone(), pos, &EntityType::EGG);
                let mut egg = EggEntity::new(entity);
                egg.thrown.owner_id = Some(owner_id);
                egg.set_item_stack(item.clone());
                egg.thrown.set_velocity(0.0, -1.0, 0.0, power, uncertainty);
                Arc::new(egg)
            }
            id if id == Item::EXPERIENCE_BOTTLE.id => {
                let entity = Entity::new(world.clone(), pos, &EntityType::EXPERIENCE_BOTTLE);
                let mut bottle = ExperienceBottleEntity::new(entity);
                bottle.thrown.owner_id = Some(owner_id);
                bottle.set_item_stack(item.clone());
                bottle
                    .thrown
                    .set_velocity(0.0, -1.0, 0.0, power, uncertainty);
                Arc::new(bottle)
            }
            id if id == Item::SPLASH_POTION.id => {
                let entity = Entity::new(world.clone(), pos, &EntityType::SPLASH_POTION);
                let mut potion = SplashPotionEntity::new(entity);
                potion.thrown.owner_id = Some(owner_id);
                potion.set_item_stack(item.clone());
                potion
                    .thrown
                    .set_velocity(0.0, -1.0, 0.0, power, uncertainty);
                Arc::new(potion)
            }
            id if id == Item::LINGERING_POTION.id => {
                let entity = Entity::new(world.clone(), pos, &EntityType::LINGERING_POTION);
                let mut potion = LingeringPotionEntity::new(entity);
                potion.thrown.owner_id = Some(owner_id);
                potion.set_item_stack(item.clone());
                potion
                    .thrown
                    .set_velocity(0.0, -1.0, 0.0, power, uncertainty);
                Arc::new(potion)
            }
            id if id == Item::FIRE_CHARGE.id => {
                let entity = Entity::new(world.clone(), pos, &EntityType::SMALL_FIREBALL);
                let mut fireball = SmallFireballEntity::new(entity);
                fireball.thrown.owner_id = Some(owner_id);
                fireball
                    .thrown
                    .set_velocity(0.0, -1.0, 0.0, power, uncertainty);
                Arc::new(fireball)
            }
            id if id == Item::WIND_CHARGE.id => {
                let entity = Entity::new(world.clone(), pos, &EntityType::WIND_CHARGE);
                let thrown = ThrownItemEntity {
                    entity,
                    owner_id: Some(owner_id),
                    collides_with_projectiles: false,
                    has_hit: AtomicBool::new(false),
                    gravity: WIND_CHARGE_GRAVITY,
                };
                thrown.set_velocity(0.0, -1.0, 0.0, power, uncertainty);
                Arc::new(WindChargeEntity::new_normal(thrown))
            }
            _ => return None,
        };

        Some(projectile)
    }
}

impl EntityBase for OminousItemSpawnerEntity {
    fn tick(&self, _caller: &dyn EntityBase, _server: &Server) {
        let age = i64::from(self.entity.age.load(Ordering::Relaxed));
        let spawn_after = self.spawn_item_after_ticks();

        if age == spawn_after - TICKS_BEFORE_ABOUT_TO_SPAWN_SOUND {
            self.entity.world.load().play_sound(
                Sound::BlockTrialSpawnerAboutToSpawnItem,
                SoundCategory::Neutral,
                &self.entity.pos.load(),
            );
        }

        if age >= spawn_after {
            self.spawn_item();
            self.entity.remove();
        }
    }

    fn init_data_tracker(&self) {
        self.set_item(self.item());
    }

    fn write_custom_nbt(&self, nbt: &mut NbtCompound) {
        let item = self.item();
        if !item.is_empty() {
            let mut item_nbt = NbtCompound::new();
            item.write_item_stack(&mut item_nbt);
            nbt.put_compound("item", item_nbt);
        }
        nbt.put_long("spawn_item_after_ticks", self.spawn_item_after_ticks());
    }

    fn read_custom_nbt(&self, nbt: &NbtCompound) {
        let item = nbt
            .get_compound("item")
            .and_then(ItemStack::read_item_stack)
            .unwrap_or_else(|| ItemStack::EMPTY.clone());
        self.set_item(item);
        *self
            .spawn_item_after_ticks
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner) =
            nbt.get_long("spawn_item_after_ticks").unwrap_or(0);
    }

    fn get_entity(&self) -> &Entity {
        &self.entity
    }

    fn get_living_entity(&self) -> Option<&LivingEntity> {
        None
    }

    fn is_pushable(&self) -> bool {
        false
    }

    fn is_pushed_by_fluids(&self) -> bool {
        false
    }

    fn can_hit(&self) -> bool {
        false
    }

    fn is_immune_to_explosion(&self) -> bool {
        true
    }

    fn damage_with_context(
        &self,
        _caller: &dyn EntityBase,
        _amount: f32,
        _damage_type: DamageType,
        _position: Option<Vector3<f64>>,
        _source: Option<&dyn EntityBase>,
        _cause: Option<&dyn EntityBase>,
    ) -> bool {
        false
    }

    fn cast_any(&self) -> &dyn std::any::Any {
        self
    }
}
