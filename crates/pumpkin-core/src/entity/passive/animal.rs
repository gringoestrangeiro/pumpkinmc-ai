use std::sync::Arc;

use pumpkin_data::entity::EntityStatus;
use pumpkin_data::item_stack::ItemStack;
use pumpkin_data::particle::Particle;
use pumpkin_data::sound::{Sound, SoundCategory};
use uuid::Uuid;

use crate::entity::ageable::{AgeableMob, BABY_START_AGE};
use crate::entity::{EntityBase, mob::Mob, player::Player};
use crate::entity::{experience_orb::ExperienceOrbEntity, r#type::from_type};
use pumpkin_protocol::bedrock::server::actor_event::ActorEventID;
use pumpkin_util::math::vector3::Vector3;
use rand::RngExt;

pub trait Animal: Mob {
    fn is_food(&self, item_stack: &ItemStack) -> bool;

    /// Vanilla `Animal.canMate` restrictions beyond both animals being in love.
    fn can_breed_with(&self, _partner: &dyn EntityBase) -> bool {
        true
    }

    /// Vanilla `Animal.spawnChildFromBreeding`. Overrides replace the live child with an
    /// egg or set the mother's egg flag.
    fn spawn_child_from_breeding(&self, partner: &dyn EntityBase) {
        let entity = &self.get_mob_entity().living_entity.entity;
        let world = entity.world.load_full();
        let baby = from_type(
            entity.entity_type,
            entity.pos.load(),
            &world,
            Uuid::new_v4(),
        );
        baby.get_entity().set_age(
            self.as_ageable()
                .map_or(BABY_START_AGE, AgeableMob::get_baby_start_age),
        );
        self.baby_spawned_from_breeding(baby.as_ref(), partner);
        world.spawn_entity(baby);
        self.finalize_spawn_child_from_breeding(partner);
    }

    /// Vanilla `Animal.getBreedOffspring` post-processing, used by pandas for gene inheritance.
    fn baby_spawned_from_breeding(&self, _baby: &dyn EntityBase, _partner: &dyn EntityBase) {}

    /// Vanilla `Animal.finalizeSpawnChildFromBreeding`: stats, love reset and the XP orb.
    fn finalize_spawn_child_from_breeding(&self, partner: &dyn EntityBase) {
        let mob_entity = self.get_mob_entity();
        let entity = &mob_entity.living_entity.entity;
        let world = entity.world.load_full();
        let love_cause = mob_entity.breeder.load().or_else(|| {
            partner
                .get_mob()
                .and_then(|mob| mob.get_mob_entity().breeder.load())
        });
        if let Some(player) = love_cause.and_then(|uuid| world.get_player_by_uuid(uuid)) {
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
                &format!("minecraft:{}", entity.entity_type.resource_name),
            );
        }

        mob_entity.reset_love_ticks();
        mob_entity
            .breeding_cooldown
            .store(6000, std::sync::atomic::Ordering::Relaxed);
        partner.reset_love();
        partner.set_breeding_cooldown(6000);

        world.send_entity_status(entity, EntityStatus::InLoveHearts, None);
        if world.level_info.load().game_rules.mob_drops {
            ExperienceOrbEntity::spawn(
                &world,
                entity.pos.load(),
                self.get_random().random_range(1..8),
            );
        }
    }

    /// Animals prefer grass, then bright spots.
    fn animal_walk_target_value(&self, pos: &pumpkin_util::math::position::BlockPos) -> f32 {
        let world = self.get_mob_entity().living_entity.entity.world.load();
        if world.get_block(&pos.down()).id == pumpkin_data::Block::GRASS_BLOCK.id {
            return 10.0;
        }
        world.get_light_level_dependent_magic_value(pos) - 0.5
    }

    fn play_eating_sound(&self, sound: Sound) {
        let mob_entity = self.get_mob_entity();
        let entity = &mob_entity.living_entity.entity;
        let world = entity.world.load();
        world.play_sound(sound, SoundCategory::Neutral, &entity.pos.load());
    }

    fn write_animal_nbt(&self, nbt: &mut pumpkin_nbt::compound::NbtCompound) {
        let mob_entity = self.get_mob_entity();
        let in_love = mob_entity
            .love_ticks
            .load(std::sync::atomic::Ordering::Relaxed);
        nbt.put_int("InLove", in_love);
        if let Some(uuid) = mob_entity.breeder.load() {
            nbt.put_uuid("LoveCause", uuid);
        }
    }

    fn read_animal_nbt(&self, nbt: &pumpkin_nbt::compound::NbtCompound) {
        let mob_entity = self.get_mob_entity();
        let in_love = nbt.get_int("InLove").unwrap_or(0);
        let love_cause = nbt.get_uuid("LoveCause");
        mob_entity.set_love_ticks(in_love, love_cause);
    }

    fn animal_interact(
        &self,
        player: &Arc<Player>,
        item_stack: &mut ItemStack,
        ambient_sound: Sound,
    ) -> bool {
        let mob_entity = self.get_mob_entity();
        if self.is_food(item_stack) {
            let age = mob_entity
                .living_entity
                .entity
                .age
                .load(std::sync::atomic::Ordering::Relaxed);

            if age >= 0 && mob_entity.is_breeding_ready() && !mob_entity.is_in_love() {
                item_stack.decrement_unless_creative(player.gamemode.load(), 1);

                mob_entity.set_love_ticks(600, Some(player.gameprofile.id));
                let entity = &mob_entity.living_entity.entity;
                let world = entity.world.load();

                // Vanilla `Animal.setInLove` broadcasts entity event 18 only; clients render
                // the heart burst themselves (`Animal.handleEntityEvent`).
                world.send_entity_status(
                    entity,
                    pumpkin_data::entity::EntityStatus::InLoveHearts,
                    Some(ActorEventID::InLoveHearts),
                );

                world.play_sound(ambient_sound, SoundCategory::Neutral, &entity.pos.load());
                return true;
            }

            if age < 0 {
                item_stack.decrement_unless_creative(player.gamemode.load(), 1);
                let speedup = (-age / 10).max(1);
                mob_entity
                    .living_entity
                    .entity
                    .age
                    .fetch_add(speedup, std::sync::atomic::Ordering::Relaxed);

                let entity = &mob_entity.living_entity.entity;
                let world = entity.world.load();
                let pos = entity.pos.load();

                world.spawn_particle(
                    pos + Vector3::new(0.0, f64::from(entity.height()), 0.0),
                    Vector3::new(0.5, 0.5, 0.5),
                    1.0,
                    7,
                    Particle::HappyVillager,
                );
                self.play_eating_sound(ambient_sound);
                return true;
            }
        }

        mob_entity.mob_interact(player, item_stack)
    }
}

/// Vanilla `ItemUtils.createFilledResult` for the bucket of water an animal
/// leaves behind when it eats a fish bucket.
pub fn give_bucket_food_remainder(player: &Player, item_stack: &mut ItemStack) {
    let mut water_bucket = ItemStack::new(1, &pumpkin_data::item::Item::WATER_BUCKET);
    if player.gamemode.load() == pumpkin_util::GameMode::Creative {
        let added = player.inventory.insert_stack_anywhere(&mut water_bucket);
        if !added && !water_bucket.is_empty() {
            player
                .world()
                .drop_stack(&player.get_entity().block_pos.load(), water_bucket);
        }
    } else {
        *item_stack = water_bucket;
    }
}

#[must_use]
pub fn get_dye_color_from_item(item: &pumpkin_data::item::Item) -> Option<u8> {
    use pumpkin_data::data_component::DataComponent;
    use pumpkin_data::data_component_impl::{DyeImpl, get};

    for (id, component) in item.components {
        if *id == DataComponent::Dye {
            return Some(get::<DyeImpl>(*component).color.id());
        }
    }
    None
}

#[must_use]
pub const fn get_wool_item_for_color(color: u8) -> &'static pumpkin_data::item::Item {
    match color {
        0 => &pumpkin_data::item::Item::WHITE_WOOL,
        1 => &pumpkin_data::item::Item::ORANGE_WOOL,
        2 => &pumpkin_data::item::Item::MAGENTA_WOOL,
        3 => &pumpkin_data::item::Item::LIGHT_BLUE_WOOL,
        4 => &pumpkin_data::item::Item::YELLOW_WOOL,
        5 => &pumpkin_data::item::Item::LIME_WOOL,
        6 => &pumpkin_data::item::Item::PINK_WOOL,
        7 => &pumpkin_data::item::Item::GRAY_WOOL,
        8 => &pumpkin_data::item::Item::LIGHT_GRAY_WOOL,
        9 => &pumpkin_data::item::Item::CYAN_WOOL,
        10 => &pumpkin_data::item::Item::PURPLE_WOOL,
        11 => &pumpkin_data::item::Item::BLUE_WOOL,
        12 => &pumpkin_data::item::Item::BROWN_WOOL,
        13 => &pumpkin_data::item::Item::GREEN_WOOL,
        14 => &pumpkin_data::item::Item::RED_WOOL,
        _ => &pumpkin_data::item::Item::BLACK_WOOL,
    }
}

#[must_use]
pub fn get_carpet_color_from_item(item: &pumpkin_data::item::Item) -> Option<u8> {
    match item.registry_key {
        "white_carpet" => Some(0),
        "orange_carpet" => Some(1),
        "magenta_carpet" => Some(2),
        "light_blue_carpet" => Some(3),
        "yellow_carpet" => Some(4),
        "lime_carpet" => Some(5),
        "pink_carpet" => Some(6),
        "gray_carpet" => Some(7),
        "light_gray_carpet" => Some(8),
        "cyan_carpet" => Some(9),
        "purple_carpet" => Some(10),
        "blue_carpet" => Some(11),
        "brown_carpet" => Some(12),
        "green_carpet" => Some(13),
        "red_carpet" => Some(14),
        "black_carpet" => Some(15),
        _ => None,
    }
}
