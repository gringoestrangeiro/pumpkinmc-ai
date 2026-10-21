use super::BlockEntity;
use crate::entity::ageable::AgeableMob;
use crate::entity::mob::Mob;
use crate::entity::passive::bee::{BeeEntity, drop_off_nectar};
use crate::entity::{EntityBase, r#type::from_type};
use crate::world::World;
use pumpkin_data::block_properties::{BeeNestLikeProperties, CampfireLikeProperties};
use pumpkin_data::entity::EntityType;
use pumpkin_data::game_event::GameEvent;
use pumpkin_data::sound::{Sound, SoundCategory};
use pumpkin_data::tag::{self, Taggable};
use pumpkin_data::{Block, BlockId, HorizontalFacingExt};
use pumpkin_nbt::compound::NbtCompound;
use pumpkin_nbt::tag::NbtTag;
use pumpkin_util::math::position::BlockPos;
use pumpkin_world::world::BlockFlags;
use std::sync::{Arc, Mutex};
use uuid::Uuid;

pub const MAX_OCCUPANTS: usize = 3;
const MIN_TICKS_BEFORE_REENTERING_HIVE: i32 = 400;
const MIN_OCCUPATION_TICKS_NECTAR: i32 = 2400;
pub const MIN_OCCUPATION_TICKS_NECTARLESS: i32 = 600;
const EMERGENCY_RELEASE_DISTANCE_SQ: f64 = 16.0;

/// Vanilla `BeehiveBlockEntity.IGNORED_BEE_TAGS`.
const IGNORED_BEE_TAGS: [&str; 25] = [
    "Air",
    "drop_chances",
    "equipment",
    "Brain",
    "CanPickUpLoot",
    "DeathTime",
    "fall_distance",
    "FallFlying",
    "Fire",
    "HurtTime",
    "LeftHanded",
    "Motion",
    "NoGravity",
    "OnGround",
    "PortalCooldown",
    "Pos",
    "Rotation",
    "sleeping_pos",
    "CannotEnterHiveTicks",
    "TicksSincePollination",
    "CropsGrownSincePollination",
    "hive_pos",
    "Passengers",
    "leash",
    "UUID",
];

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum BeeReleaseStatus {
    HoneyDelivered,
    BeeReleased,
    Emergency,
}

pub struct BeehiveBlockEntity {
    pub position: BlockPos,
    pub bees: Mutex<Option<Vec<NbtTag>>>,
    pub flower_pos: Mutex<Option<BlockPos>>,
}

impl BlockEntity for BeehiveBlockEntity {
    fn resource_location(&self) -> &'static str {
        Self::ID
    }

    fn get_position(&self) -> BlockPos {
        self.position
    }

    fn from_nbt(nbt: &pumpkin_nbt::compound::NbtCompound, position: BlockPos) -> Self
    where
        Self: Sized,
    {
        let bees = nbt
            .get_list("bees")
            .or_else(|| nbt.get_list("Bees"))
            .map(<[_]>::to_vec);
        let flower_pos = nbt
            .get_int_array("flower_pos")
            .and_then(|arr| (arr.len() == 3).then(|| BlockPos::new(arr[0], arr[1], arr[2])))
            .or_else(|| {
                nbt.get_compound("flower_pos")
                    .or_else(|| nbt.get_compound("FlowerPos"))
                    .map(|c| {
                        BlockPos::new(
                            c.get_int("X").or_else(|| c.get_int("x")).unwrap_or(0),
                            c.get_int("Y").or_else(|| c.get_int("y")).unwrap_or(0),
                            c.get_int("Z").or_else(|| c.get_int("z")).unwrap_or(0),
                        )
                    })
            });
        Self {
            position,
            bees: Mutex::new(bees),
            flower_pos: Mutex::new(flower_pos),
        }
    }

    fn write_nbt(&self, nbt: &mut NbtCompound) {
        if let Ok(b) = self.bees.lock()
            && let Some(b) = b.as_ref()
        {
            nbt.put_list("bees", b.clone());
        }
        if let Ok(fp) = self.flower_pos.lock()
            && let Some(fp) = fp.as_ref()
        {
            nbt.put(
                "flower_pos",
                pumpkin_nbt::tag::NbtTag::IntArray(vec![fp.0.x, fp.0.y, fp.0.z]),
            );
        }
    }

    fn chunk_data_nbt(&self) -> Option<NbtCompound> {
        let mut nbt = NbtCompound::new();
        if let Ok(bees) = self.bees.try_lock()
            && let Some(ref b) = *bees
        {
            nbt.put_list("bees", b.clone());
        }
        if let Ok(flower_pos) = self.flower_pos.try_lock()
            && let Some(ref fp) = *flower_pos
        {
            nbt.put(
                "flower_pos",
                pumpkin_nbt::tag::NbtTag::IntArray(vec![fp.0.x, fp.0.y, fp.0.z]),
            );
        }
        Some(nbt)
    }

    fn collect_item_components(&self, stack: &mut pumpkin_data::item_stack::ItemStack) {
        if let Ok(bees) = self.bees.lock()
            && let Some(bees) = bees.as_ref()
            && !bees.is_empty()
        {
            stack.set_data_component(pumpkin_data::data_component_impl::BeesImpl {
                bees: bees.clone(),
            });
        }
    }

    fn apply_item_components(&self, stack: &pumpkin_data::item_stack::ItemStack) {
        if let Some(bees) =
            stack.get_data_component::<pumpkin_data::data_component_impl::BeesImpl>()
            && !bees.bees.is_empty()
        {
            *self
                .bees
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner) = Some(bees.bees.clone());
        }
    }

    fn tick(&self, world: &Arc<World>) {
        let state = world.get_block_state(&self.position);
        if !self.is_empty() && self.is_fire_nearby(world) {
            self.empty_all_living_from_hive(world, None, state.id, BeeReleaseStatus::Emergency);
        }

        self.tick_occupants(world, state.id);
        if !self.is_empty() && rand::random::<f64>() < 0.005 {
            world.play_sound(
                Sound::BlockBeehiveWork,
                SoundCategory::Blocks,
                &self.position.to_centered_f64(),
            );
        }
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}

impl BeehiveBlockEntity {
    pub const ID: &'static str = "minecraft:beehive";
    #[must_use]
    pub const fn new(position: BlockPos) -> Self {
        Self {
            position,
            bees: Mutex::new(None),
            flower_pos: Mutex::new(None),
        }
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.bees
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .as_ref()
            .is_none_or(Vec::is_empty)
    }

    #[must_use]
    pub fn is_full(&self) -> bool {
        self.occupant_count() == MAX_OCCUPANTS
    }

    #[must_use]
    pub fn occupant_count(&self) -> usize {
        self.bees
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .as_ref()
            .map_or(0, Vec::len)
    }

    #[must_use]
    pub fn is_fire_nearby(&self, world: &World) -> bool {
        for dx in -1..=1 {
            for dy in -1..=1 {
                for dz in -1..=1 {
                    let pos = BlockPos::new(
                        self.position.0.x + dx,
                        self.position.0.y + dy,
                        self.position.0.z + dz,
                    );
                    if world.get_block(&pos).has_tag(&tag::Block::MINECRAFT_FIRE) {
                        return true;
                    }
                }
            }
        }
        false
    }

    pub fn add_occupant(&self, entity: &dyn EntityBase, world: &Arc<World>) {
        if self.occupant_count() >= MAX_OCCUPANTS {
            return;
        }

        let bee_entity = entity.cast_any().downcast_ref::<BeeEntity>();
        let has_nectar = bee_entity.is_some_and(BeeEntity::has_nectar);
        let saved_flower = bee_entity.and_then(BeeEntity::get_saved_flower_pos);

        let mut entity_data = NbtCompound::new();
        entity.write_nbt(&mut entity_data);
        for ignored in IGNORED_BEE_TAGS {
            entity_data.child_tags.remove(ignored);
        }

        let mut occupant = NbtCompound::new();
        occupant.put_compound("entity_data", entity_data);
        occupant.put_int("ticks_in_hive", 0);
        occupant.put_int(
            "min_ticks_in_hive",
            if has_nectar {
                MIN_OCCUPATION_TICKS_NECTAR
            } else {
                MIN_OCCUPATION_TICKS_NECTARLESS
            },
        );

        {
            let mut bees = self
                .bees
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            let stored = bees.get_or_insert_with(Vec::new);
            if stored.len() >= MAX_OCCUPANTS {
                return;
            }
            stored.push(NbtTag::Compound(occupant));
        }

        if let Some(flower) = saved_flower {
            let mut stored = self
                .flower_pos
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            if stored.is_none() || rand::random::<bool>() {
                *stored = Some(flower);
            }
        }

        entity.get_entity().unleash();
        world.play_sound(
            Sound::BlockBeehiveEnter,
            SoundCategory::Blocks,
            &self.position.to_centered_f64(),
        );
        world.emit_game_event_with_source(
            GameEvent::BlockChange,
            self.position.to_centered_f64(),
            Some(entity),
        );
        entity.get_entity().remove();
    }

    pub fn empty_all_living_from_hive(
        &self,
        world: &Arc<World>,
        player: Option<&Arc<crate::entity::player::Player>>,
        state_id: pumpkin_data::BlockStateId,
        release_status: BeeReleaseStatus,
    ) {
        let released = self.release_all_occupants(world, state_id, release_status);
        let Some(player) = player else {
            return;
        };
        let player_pos = player.get_entity().pos.load();
        let sedated = is_sedated(world, &self.position);
        for entity in released {
            if !entity
                .get_entity()
                .pos
                .load()
                .squared_distance_to_vec(&player_pos)
                <= EMERGENCY_RELEASE_DISTANCE_SQ
            {
                continue;
            }
            let Some(bee) = entity.cast_any().downcast_ref::<BeeEntity>() else {
                continue;
            };
            if sedated {
                bee.cannot_enter_hive_ticks.store(
                    MIN_TICKS_BEFORE_REENTERING_HIVE,
                    std::sync::atomic::Ordering::Relaxed,
                );
            } else {
                let target: Arc<dyn EntityBase> = player.clone();
                bee.set_mob_target(Some(target));
            }
        }
    }

    fn release_all_occupants(
        &self,
        world: &Arc<World>,
        state_id: pumpkin_data::BlockStateId,
        release_status: BeeReleaseStatus,
    ) -> Vec<Arc<dyn EntityBase>> {
        let mut released = Vec::new();
        let saved_flower = *self
            .flower_pos
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let position = self.position;

        {
            let mut bees = self
                .bees
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            let Some(stored) = bees.as_mut() else {
                return released;
            };
            stored.retain(|occupant| {
                !release_occupant(
                    world,
                    &position,
                    state_id,
                    occupant,
                    &mut released,
                    release_status,
                    saved_flower,
                )
            });
        }

        released
    }

    fn tick_occupants(&self, world: &Arc<World>, state_id: pumpkin_data::BlockStateId) {
        let saved_flower = *self
            .flower_pos
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let position = self.position;
        let mut bees = self
            .bees
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let Some(stored) = bees.as_mut() else {
            return;
        };

        let mut index = 0;
        while index < stored.len() {
            let (ticks, min_ticks, has_nectar) = match &stored[index] {
                NbtTag::Compound(occupant) => {
                    let ticks = occupant.get_int("ticks_in_hive").unwrap_or(0) + 1;
                    let min_ticks = occupant
                        .get_int("min_ticks_in_hive")
                        .unwrap_or(MIN_OCCUPATION_TICKS_NECTARLESS);
                    let has_nectar = occupant_entity_data(occupant)
                        .and_then(|data| data.get_bool("HasNectar"))
                        .unwrap_or(false);
                    (ticks, min_ticks, has_nectar)
                }
                _ => {
                    index += 1;
                    continue;
                }
            };

            if let NbtTag::Compound(occupant) = &mut stored[index] {
                occupant.put_int("ticks_in_hive", ticks);
            }
            if ticks <= min_ticks {
                index += 1;
                continue;
            }

            let status = if has_nectar {
                BeeReleaseStatus::HoneyDelivered
            } else {
                BeeReleaseStatus::BeeReleased
            };
            let occupant = stored[index].clone();
            let mut released = Vec::new();
            if release_occupant(
                world,
                &position,
                state_id,
                &occupant,
                &mut released,
                status,
                saved_flower,
            ) {
                stored.remove(index);
            } else {
                index += 1;
            }
        }
    }
}

fn occupant_entity_data(occupant: &NbtCompound) -> Option<&NbtCompound> {
    occupant
        .get_compound("entity_data")
        .or_else(|| occupant.has("id").then_some(occupant))
}

#[expect(clippy::too_many_lines)]
fn release_occupant(
    world: &Arc<World>,
    block_pos: &BlockPos,
    state_id: pumpkin_data::BlockStateId,
    occupant: &NbtTag,
    spawned: &mut Vec<Arc<dyn EntityBase>>,
    release_status: BeeReleaseStatus,
    saved_flower_pos: Option<BlockPos>,
) -> bool {
    if world.bees_stay_in_hive(block_pos) && release_status != BeeReleaseStatus::Emergency {
        return false;
    }

    let props = BeeNestLikeProperties::from_state_id(state_id);
    let facing = props.facing.to_block_direction();
    let facing_pos = block_pos.offset(facing.to_offset());
    let front_blocked = !world
        .get_block_state(&facing_pos)
        .collision_shapes
        .is_empty();
    if front_blocked && release_status != BeeReleaseStatus::Emergency {
        return false;
    }

    let Some(occupant) = occupant.extract_compound() else {
        return false;
    };
    let Some(entity_data) = occupant_entity_data(occupant) else {
        return false;
    };
    let Some(type_name) = entity_data.get_string("id") else {
        return false;
    };
    let type_name = type_name.strip_prefix("minecraft:").unwrap_or(type_name);
    let Some(entity_type) = EntityType::from_name(type_name) else {
        return false;
    };
    if entity_type != &EntityType::BEE {
        return false;
    }

    let mut tag = entity_data.clone();
    tag.child_tags.remove("id");
    for ignored in IGNORED_BEE_TAGS {
        tag.child_tags.remove(ignored);
    }

    let entity = from_type(
        entity_type,
        block_pos.to_centered_f64(),
        world,
        Uuid::new_v4(),
    );
    entity.read_nbt_non_mut(&tag);
    let Some(bee) = entity.cast_any().downcast_ref::<BeeEntity>() else {
        return false;
    };

    let ticks_in_hive = occupant.get_int("ticks_in_hive").unwrap_or(0);
    if let Some(ageable) = bee.as_ageable() {
        if !ageable.is_age_locked() {
            let age = ageable.get_age();
            if age < 0 {
                ageable.set_age(age.saturating_add(ticks_in_hive).min(0));
            } else if age > 0 {
                ageable.set_age(age.saturating_sub(ticks_in_hive).max(0));
            }
        }
    }
    bee.mob_entity.love_ticks.store(
        bee.mob_entity
            .love_ticks
            .load(std::sync::atomic::Ordering::Relaxed)
            .saturating_sub(ticks_in_hive)
            .max(0),
        std::sync::atomic::Ordering::Relaxed,
    );
    bee.set_hive_pos(Some(*block_pos));

    if let Some(flower) = saved_flower_pos
        && !bee.has_saved_flower_pos()
        && rand::random::<f32>() < 0.9
    {
        bee.set_saved_flower_pos(Some(flower));
    }

    if release_status == BeeReleaseStatus::HoneyDelivered {
        drop_off_nectar(entity.as_ref());
        let block = Block::from_state_id(state_id);
        if block.id == BlockId::BEEHIVE || block.id == BlockId::BEE_NEST {
            let honey_level = props.honey_level;
            if honey_level < 5 {
                let mut level_increase = if rand::random_range(0..100) == 0 {
                    2
                } else {
                    1
                };
                if honey_level + level_increase > 5 {
                    level_increase -= 1;
                }
                let mut grown = props;
                grown.honey_level += level_increase;
                world.set_block_state(block_pos, grown.to_state_id(block), BlockFlags::NOTIFY_ALL);
            }
        }
    }

    let width = f64::from(entity_type.dimension[0]);
    let height = f64::from(entity_type.dimension[1]);
    let delta = if front_blocked {
        0.0
    } else {
        0.55 + width / 2.0
    };
    let spawn_x = f64::from(block_pos.0.x) + 0.5 + delta * f64::from(facing.to_offset().x);
    let spawn_y = f64::from(block_pos.0.y) + 0.5 - height / 2.0;
    let spawn_z = f64::from(block_pos.0.z) + 0.5 + delta * f64::from(facing.to_offset().z);
    let entity_ref = entity.get_entity();
    entity_ref.set_pos(pumpkin_util::math::vector3::Vector3::new(
        spawn_x, spawn_y, spawn_z,
    ));
    entity_ref.set_rotation(entity_ref.yaw.load(), entity_ref.pitch.load());
    entity_ref.set_has_no_gravity(true);

    world.play_sound(
        Sound::BlockBeehiveExit,
        SoundCategory::Blocks,
        &block_pos.to_centered_f64(),
    );
    world.emit_game_event_with_source(
        GameEvent::BlockChange,
        block_pos.to_centered_f64(),
        Some(entity.as_ref()),
    );
    spawned.push(entity.clone());
    world.spawn_entity(entity)
}

/// Vanilla `CampfireBlock.isSmokeyPos`: a lit campfire in the five blocks below
/// the hive keeps its bees calm.
fn is_sedated(world: &World, pos: &BlockPos) -> bool {
    for height in 1..=5 {
        let check = pos.down_height(height);
        let (block, state) = world.get_block_and_state_id(&check);
        if (block.id == BlockId::CAMPFIRE || block.id == BlockId::SOUL_CAMPFIRE)
            && CampfireLikeProperties::from_state_id(state).lit
        {
            return true;
        }
    }
    false
}
