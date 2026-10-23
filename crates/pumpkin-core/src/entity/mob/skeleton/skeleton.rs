use pumpkin_data::sound::Sound;
use pumpkin_data::tracked_data;
use pumpkin_nbt::compound::NbtCompound;
use pumpkin_protocol::java::client::play::CWorldEvent;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicI32, Ordering};

use crate::entity::{
    Entity, EntityBase,
    mob::{Mob, MobEntity, equipment::RegionalDifficulty, skeleton::SkeletonEntityBase},
};
use crate::world::World;

/// Vanilla `Skeleton` `ConversionTracker` timings.
const FREEZING_TIME: i32 = 140;
const STRAY_CONVERSION_TIME: i32 = 300;
const STRAY_CONVERSION_LEVEL_EVENT: i32 = 1048;

pub struct SkeletonEntity {
    entity: Arc<SkeletonEntityBase>,
    freezing_time: AtomicI32,
    conversion_time: AtomicI32,
    converting: AtomicBool,
}

impl SkeletonEntity {
    pub fn new(entity: Entity) -> Arc<Self> {
        let entity = SkeletonEntityBase::new(entity);
        let skeleton = Self {
            entity,
            freezing_time: AtomicI32::new(0),
            conversion_time: AtomicI32::new(-1),
            converting: AtomicBool::new(false),
        };
        Arc::new(skeleton)
    }

    fn set_converting(&self, converting: bool) {
        self.converting.store(converting, Ordering::Relaxed);
        self.entity
            .mob_entity
            .living_entity
            .entity
            .set_synced_data(tracked_data::skeleton::DATA_STRAY_CONVERSION_ID, converting);
    }

    fn tick_freezing(&self) {
        let entity = &self.entity.mob_entity.living_entity.entity;
        if !entity.is_alive() || self.entity.mob_entity.is_no_ai() {
            return;
        }

        if entity.is_in_powder_snow() {
            if self.converting.load(Ordering::Relaxed) {
                let remaining = self.conversion_time.fetch_sub(1, Ordering::Relaxed) - 1;
                if remaining < 0 {
                    self.set_converting(false);
                    self.convert_to_stray();
                }
            } else {
                let frozen = self.freezing_time.fetch_add(1, Ordering::Relaxed) + 1;
                if frozen >= FREEZING_TIME {
                    self.conversion_time
                        .store(STRAY_CONVERSION_TIME, Ordering::Relaxed);
                    self.set_converting(true);
                }
            }
        } else {
            self.freezing_time.store(-1, Ordering::Relaxed);
            self.set_converting(false);
        }
    }

    fn convert_to_stray(&self) {
        let entity = &self.entity.mob_entity.living_entity.entity;
        let world = entity.world.load();
        let pos = entity.pos.load();
        let stray = crate::entity::r#type::from_type(
            &pumpkin_data::entity::EntityType::STRAY,
            pos,
            &world,
            uuid::Uuid::new_v4(),
        );

        let stray_base = stray.get_entity();
        stray_base.set_rotation(entity.yaw.load(), entity.pitch.load());
        stray_base.head_yaw.store(entity.head_yaw.load());
        stray_base.velocity.store(entity.velocity.load());

        if let Some(custom_name) = &**entity.custom_name.load() {
            stray_base.set_custom_name(custom_name.clone());
        }

        {
            let src_equip = self
                .entity
                .mob_entity
                .living_entity
                .entity_equipment
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            if let Some(living) = stray.get_living_entity() {
                let mut dst_equip = living
                    .entity_equipment
                    .lock()
                    .unwrap_or_else(std::sync::PoisonError::into_inner);
                for (slot, item) in &src_equip.equipment {
                    dst_equip.put(slot, item.clone());
                }
            }
        }

        let chunk_pos = stray_base.chunk_pos.load();
        let block_pos = stray_base.block_pos.load();
        let silent = stray_base.is_silent();
        world.spawn_entity(stray);
        if !silent {
            world.broadcast_to_chunk(
                chunk_pos,
                &CWorldEvent::new(STRAY_CONVERSION_LEVEL_EVENT, block_pos, 0, false),
            );
        }
        entity.remove();
    }
}

impl Mob for SkeletonEntity {
    fn get_mob_entity(&self) -> &MobEntity {
        &self.entity.mob_entity
    }

    fn get_ambient_sound(&self) -> Option<Sound> {
        Some(Sound::EntitySkeletonAmbient)
    }

    fn populate_default_equipment_slots(
        &self,
        world: &Arc<World>,
        difficulty: &RegionalDifficulty,
    ) {
        self.entity
            .populate_default_equipment_slots(world, difficulty);
    }

    fn populate_default_equipment_enchantments(&self, difficulty: &RegionalDifficulty) {
        self.entity
            .populate_default_equipment_enchantments(difficulty);
    }

    fn mob_tick(&self, _caller: &dyn EntityBase) {
        self.tick_freezing();
    }

    fn get_min_attack_interval(&self) -> Option<i32> {
        Some(self.entity.min_attack_interval())
    }

    fn mob_write_nbt(&self, nbt: &mut NbtCompound) {
        let freezing_time = self.freezing_time.load(Ordering::Relaxed);
        nbt.put_int(
            "FreezingTime",
            if self
                .entity
                .mob_entity
                .living_entity
                .entity
                .is_in_powder_snow()
            {
                freezing_time
            } else {
                -1
            },
        );
        nbt.put_int(
            "StrayConversionTime",
            if self.converting.load(Ordering::Relaxed) {
                self.conversion_time.load(Ordering::Relaxed)
            } else {
                -1
            },
        );
    }

    fn mob_read_nbt(&self, nbt: &NbtCompound) {
        self.freezing_time
            .store(nbt.get_int("FreezingTime").unwrap_or(0), Ordering::Relaxed);
        let conversion_time = nbt.get_int("StrayConversionTime").unwrap_or(-1);
        if conversion_time == -1 {
            self.set_converting(false);
        } else {
            self.conversion_time
                .store(conversion_time, Ordering::Relaxed);
            self.set_converting(true);
        }
    }
}
