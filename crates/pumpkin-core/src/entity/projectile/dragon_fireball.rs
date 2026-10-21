use std::sync::atomic::{AtomicBool, Ordering};

use pumpkin_data::effect::StatusEffect;
use pumpkin_data::entity::EntityType;
use pumpkin_data::item::Item;
use pumpkin_data::item_stack::ItemStack;
use pumpkin_data::world::WorldEvent;
use pumpkin_util::math::boundingbox::BoundingBox;
use pumpkin_util::math::position::BlockPos;
use pumpkin_util::math::vector3::Vector3;

use crate::entity::{
    Entity, EntityBase,
    area_effect_cloud::AreaEffectCloudEntity,
    projectile::{
        ProjectileHit, ThrownItemEntity,
        fireball::{AIR_INERTIA, INITIAL_ACCELERATION_POWER, WATER_INERTIA},
    },
};
use crate::server::Server;

/// Vanilla `DragonFireball`: a gravity-free projectile that leaves a dragon
/// breath cloud with instant damage where it lands.
pub struct DragonFireballEntity {
    pub thrown: ThrownItemEntity,
}

impl DragonFireballEntity {
    const SPLASH_RANGE: f64 = 4.0;
    const CLOUD_RADIUS: f32 = 3.0;
    const CLOUD_DURATION: i32 = 600;
    const MAX_RADIUS: f32 = 7.0;

    #[must_use]
    pub const fn new(entity: Entity) -> Self {
        Self {
            thrown: ThrownItemEntity {
                entity,
                owner_id: None,
                collides_with_projectiles: false,
                has_hit: AtomicBool::new(false),
                gravity: 0.0,
            },
        }
    }

    #[must_use]
    pub fn new_shot(entity: Entity, shooter: &Entity, direction: Vector3<f64>) -> Self {
        let thrown = ThrownItemEntity::new(entity, shooter, 0.0);
        thrown.entity.velocity.store(direction.normalize().multiply(
            INITIAL_ACCELERATION_POWER,
            INITIAL_ACCELERATION_POWER,
            INITIAL_ACCELERATION_POWER,
        ));
        Self { thrown }
    }

    fn spawn_cloud(&self, hit: &ProjectileHit) {
        let entity = self.get_entity();
        let world = entity.world.load();
        let hit_pos = hit.hit_pos();
        let block_pos = BlockPos::floored(hit_pos.x, hit_pos.y, hit_pos.z);

        let search_box = BoundingBox::new(
            Vector3::new(
                hit_pos.x - Self::SPLASH_RANGE,
                hit_pos.y - 2.0,
                hit_pos.z - Self::SPLASH_RANGE,
            ),
            Vector3::new(
                hit_pos.x + Self::SPLASH_RANGE,
                hit_pos.y + 2.0,
                hit_pos.z + Self::SPLASH_RANGE,
            ),
        );
        let cloud_pos = world
            .get_entities_and_players_at_box(&search_box)
            .into_iter()
            .find(|target| {
                target.get_living_entity().is_some()
                    && target
                        .get_entity()
                        .pos
                        .load()
                        .squared_distance_to_vec(&hit_pos)
                        < Self::SPLASH_RANGE * Self::SPLASH_RANGE
            })
            .map_or(hit_pos, |target| target.get_entity().pos.load());

        let cloud_entity = Entity::new(world.clone(), cloud_pos, &EntityType::AREA_EFFECT_CLOUD);
        let cloud = AreaEffectCloudEntity::create(
            cloud_entity,
            ItemStack::new(0, &Item::DRAGON_BREATH),
            vec![(&StatusEffect::INSTANT_DAMAGE, 1, 1, false, true, true)],
            Self::CLOUD_DURATION,
            Self::CLOUD_RADIUS,
            20,
            20,
            0.0,
            0,
        );
        if let Some(cloud) = cloud.cast_any().downcast_ref::<AreaEffectCloudEntity>() {
            *cloud
                .radius_on_tick
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner) =
                (Self::MAX_RADIUS - Self::CLOUD_RADIUS) / Self::CLOUD_DURATION as f32;
            // Vanilla `PowerParticleOption.create(ParticleTypes.DRAGON_BREATH, 1.0F)`.
            cloud.set_custom_particle(
                pumpkin_data::particle::Particle::DragonBreath as i32,
                Box::new(1.0f32.to_be_bytes()),
            );
        }
        world.sync_world_event(
            WorldEvent::ParticlesDragonFireballSplash,
            block_pos,
            if entity.silent.load(Ordering::Relaxed) {
                -1
            } else {
                1
            },
        );
        world.spawn_entity(cloud);
    }
}

impl EntityBase for DragonFireballEntity {
    fn get_owner_id(&self) -> Option<i32> {
        self.thrown.owner_id
    }

    fn tick(&self, caller: &dyn EntityBase, _server: &Server) {
        // Vanilla `AbstractHurtingProjectile.applyInertia`, as ported for `Fireball`.
        let entity = self.get_entity();
        let mut velocity = entity.velocity.load();
        let inertia = if entity.touching_water.load(Ordering::Relaxed) {
            WATER_INERTIA
        } else {
            AIR_INERTIA
        };
        let speed = velocity.length();
        if speed > 1e-6 {
            velocity = velocity
                .normalize()
                .multiply(
                    INITIAL_ACCELERATION_POWER,
                    INITIAL_ACCELERATION_POWER,
                    INITIAL_ACCELERATION_POWER,
                )
                .add(&velocity)
                .multiply(inertia, inertia, inertia);
            entity.velocity.store(velocity);
        }
        self.thrown.process_tick(caller);
    }

    fn get_entity(&self) -> &Entity {
        self.thrown.get_entity()
    }

    fn get_living_entity(&self) -> Option<&crate::entity::living::LivingEntity> {
        None
    }

    fn on_hit(&self, hit: ProjectileHit) {
        if let ProjectileHit::Entity { entity, .. } = &hit
            && Some(entity.get_entity().entity_id) == self.thrown.owner_id
        {
            return;
        }
        self.spawn_cloud(&hit);
    }

    fn cast_any(&self) -> &dyn std::any::Any {
        self
    }
}
