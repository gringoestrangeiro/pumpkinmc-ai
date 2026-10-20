use std::sync::{
    Arc,
    atomic::{AtomicBool, AtomicU8, AtomicU32, Ordering},
};

use pumpkin_data::Block;
use pumpkin_data::damage::DamageType;
use pumpkin_data::item::Item;
use pumpkin_data::item_stack::ItemStack;
use pumpkin_data::sound::{Sound, SoundCategory};
use pumpkin_data::tag::{self, Taggable};
use pumpkin_nbt::compound::NbtCompound;
use pumpkin_protocol::codec::var_int::VarInt;
use pumpkin_protocol::java::client::play::Metadata;
use pumpkin_util::math::boundingbox::BoundingBox;
use pumpkin_util::math::position::BlockPos;
use pumpkin_util::math::vector3::Vector3;
use pumpkin_util::version::JavaMinecraftVersion;

use crate::entity::lightning::LightningBoltEntity;
use crate::entity::player::Player;
use crate::entity::{Entity, EntityBase, living::LivingEntity};
use crate::server::Server;
use crate::world::World;

pub struct CushionEntity {
    pub entity: Entity,
    color: AtomicU8,
    ticks_since_last_check: AtomicU32,
    had_passenger: AtomicBool,
}

impl CushionEntity {
    const ANCHOR_DEPTH: f64 = 0.015_625;

    /// Vanilla `Cushion.canBePlacedAt`.
    #[must_use]
    pub fn can_be_placed_at(world: &World, bounding_box: &BoundingBox) -> bool {
        Self::would_survive_at(world, bounding_box) && !Self::is_anchor_buried(world, bounding_box)
    }

    /// Vanilla `Cushion.wouldSurviveAt`.
    #[must_use]
    pub fn would_survive_at(world: &World, bounding_box: &BoundingBox) -> bool {
        Self::has_anchor_below(world, bounding_box)
            && !Self::is_covered_by_suffocating_blocks(world, bounding_box)
    }

    fn has_anchor_below(world: &World, bounding_box: &BoundingBox) -> bool {
        let anchor_box = BoundingBox::new(
            Vector3::new(
                bounding_box.min.x,
                bounding_box.min.y - Self::ANCHOR_DEPTH,
                bounding_box.min.z,
            ),
            Vector3::new(
                bounding_box.max.x.next_down(),
                bounding_box.min.y,
                bounding_box.max.z.next_down(),
            ),
        );
        let search_box = BoundingBox::new(
            Vector3::new(anchor_box.min.x, anchor_box.min.y - 0.125, anchor_box.min.z),
            anchor_box.max,
        );

        BlockPos::iterate(search_box.min_block_pos(), search_box.max_block_pos()).any(|pos| {
            let state = world.get_block_state(&pos);
            state
                .get_block_collision_shapes_at(&pos)
                .any(|shape| shape.at_pos(pos).intersects(&anchor_box))
        })
    }

    fn is_anchor_buried(world: &World, bounding_box: &BoundingBox) -> bool {
        let mut resting_slice = *bounding_box;
        resting_slice.max.y = resting_slice.min.y + Self::ANCHOR_DEPTH;
        let resting_slice = resting_slice.contract_all(1.0e-7);

        let mut exposed_surface = vec![resting_slice];
        for pos in BlockPos::iterate(resting_slice.min_block_pos(), resting_slice.max_block_pos()) {
            let state = world.get_block_state(&pos);
            for collider in state.get_block_collision_shapes_at(&pos) {
                let collider = collider.at_pos(pos);
                exposed_surface = exposed_surface
                    .into_iter()
                    .flat_map(|part| subtract_box(part, &collider))
                    .collect();
                if exposed_surface.is_empty() {
                    return true;
                }
            }
        }
        false
    }

    fn is_covered_by_suffocating_blocks(world: &World, bounding_box: &BoundingBox) -> bool {
        let inner = bounding_box.contract_all(1.0e-7);
        BlockPos::iterate(inner.min_block_pos(), inner.max_block_pos())
            .all(|pos| world.get_block_state(&pos).is_suffocating())
    }

    pub const fn new(entity: Entity, color: u8) -> Self {
        Self {
            entity,
            color: AtomicU8::new(color),
            ticks_since_last_check: AtomicU32::new(0),
            had_passenger: AtomicBool::new(false),
        }
    }

    pub fn color(&self) -> u8 {
        self.color.load(Ordering::Relaxed)
    }

    pub fn set_color(&self, color: u8) {
        self.color.store(color, Ordering::Relaxed);
        self.sync_color();
    }

    fn sync_color(&self) {
        self.entity.set_synced_data(
            pumpkin_data::tracked_data::cushion::COLOR,
            VarInt(i32::from(self.color())),
        );
    }

    #[must_use]
    pub const fn item_for_color(color: u8) -> &'static Item {
        match color {
            0 => &Item::WHITE_CUSHION,
            1 => &Item::ORANGE_CUSHION,
            2 => &Item::MAGENTA_CUSHION,
            3 => &Item::LIGHT_BLUE_CUSHION,
            4 => &Item::YELLOW_CUSHION,
            5 => &Item::LIME_CUSHION,
            6 => &Item::PINK_CUSHION,
            7 => &Item::GRAY_CUSHION,
            8 => &Item::LIGHT_GRAY_CUSHION,
            9 => &Item::CYAN_CUSHION,
            10 => &Item::PURPLE_CUSHION,
            11 => &Item::BLUE_CUSHION,
            12 => &Item::BROWN_CUSHION,
            13 => &Item::GREEN_CUSHION,
            14 => &Item::RED_CUSHION,
            _ => &Item::BLACK_CUSHION,
        }
    }

    #[must_use]
    pub fn color_from_item(item: &Item) -> u8 {
        for (id, comp) in item.components {
            if *id == pumpkin_data::data_component::DataComponent::CushionColor {
                return pumpkin_data::data_component_impl::get::<
                    pumpkin_data::data_component_impl::CushionColorImpl,
                >(*comp)
                .color
                .id();
            }
        }
        0
    }

    #[must_use]
    pub fn color_from_stack(stack: &ItemStack) -> u8 {
        stack
            .get_data_component::<pumpkin_data::data_component_impl::CushionColorImpl>()
            .map_or_else(|| Self::color_from_item(stack.get_item()), |c| c.color.id())
    }

    fn item_stack(&self) -> ItemStack {
        let mut stack = ItemStack::new(1, Self::item_for_color(self.color()));
        if let Some(custom_name) = self.entity.custom_name.load().as_ref().clone() {
            stack.set_custom_name(custom_name.to_pretty_console());
        }
        stack
    }

    /// Vanilla `Cushion.dropItem`.
    fn drop_item(&self, caused_by: Option<&dyn EntityBase>) {
        let world = self.entity.world.load();
        if !world.level_info.load().game_rules.entity_drops {
            return;
        }

        world.play_sound(
            Sound::EntityCushionBreak,
            SoundCategory::Blocks,
            &self.entity.pos.load(),
        );

        let creative = caused_by
            .and_then(|entity| entity.get_player())
            .is_some_and(Player::is_creative);
        if !creative {
            world.drop_stack(&self.entity.block_pos.load(), self.item_stack());
        }
    }

    fn drop_and_remove(&self, caused_by: Option<&dyn EntityBase>) {
        let passengers = self
            .entity
            .passengers
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .clone();
        for passenger in passengers {
            self.entity
                .remove_passenger(passenger.get_entity().entity_id);
        }
        // A cancelled dismount must not leave a rider attached to a removed cushion.
        if self.entity.has_passengers() {
            return;
        }

        self.drop_item(caused_by);
        self.entity.remove();
    }

    /// Vanilla `Cushion.destroyIfInFire`.
    fn destroy_if_in_fire(&self, world: &World) {
        let area = self.entity.bounding_box.load().contract_all(1.0e-7);
        for pos in BlockPos::iterate(area.min_block_pos(), area.max_block_pos()) {
            let state = world.get_block_state(&pos);
            if Block::from_state_id(state.id).has_tag(&tag::Block::MINECRAFT_FIRE) {
                self.drop_and_remove(None);
                return;
            }
        }
    }
}

/// The parts of `part` that `other` does not cover.
fn subtract_box(part: BoundingBox, other: &BoundingBox) -> Vec<BoundingBox> {
    if !part.intersects(other) {
        return vec![part];
    }
    let mut pieces = Vec::new();
    let mut rest = part;
    if rest.min.x < other.min.x {
        pieces.push(BoundingBox::new(
            rest.min,
            Vector3::new(other.min.x, rest.max.y, rest.max.z),
        ));
        rest.min.x = other.min.x;
    }
    if rest.max.x > other.max.x {
        pieces.push(BoundingBox::new(
            Vector3::new(other.max.x, rest.min.y, rest.min.z),
            rest.max,
        ));
        rest.max.x = other.max.x;
    }
    if rest.min.y < other.min.y {
        pieces.push(BoundingBox::new(
            rest.min,
            Vector3::new(rest.max.x, other.min.y, rest.max.z),
        ));
        rest.min.y = other.min.y;
    }
    if rest.max.y > other.max.y {
        pieces.push(BoundingBox::new(
            Vector3::new(rest.min.x, other.max.y, rest.min.z),
            rest.max,
        ));
        rest.max.y = other.max.y;
    }
    if rest.min.z < other.min.z {
        pieces.push(BoundingBox::new(
            rest.min,
            Vector3::new(rest.max.x, rest.max.y, other.min.z),
        ));
        rest.min.z = other.min.z;
    }
    if rest.max.z > other.max.z {
        pieces.push(BoundingBox::new(
            Vector3::new(rest.min.x, rest.min.y, other.max.z),
            rest.max,
        ));
    }
    pieces
}

impl EntityBase for CushionEntity {
    fn get_entity(&self) -> &Entity {
        &self.entity
    }

    fn get_living_entity(&self) -> Option<&LivingEntity> {
        None
    }

    fn init_data_tracker(&self) {
        self.sync_color();
    }

    /// The colour has to ride along with the spawn packet; a client seeing the
    /// cushion for the first time never got the tracked data update.
    fn java_spawn_metadata(&self, version: JavaMinecraftVersion) -> Option<Box<[u8]>> {
        let mut metadata = Vec::new();
        Metadata::new(
            pumpkin_data::tracked_data::cushion::COLOR,
            VarInt(i32::from(self.color())),
        )
        .write(&mut metadata, &version)
        .ok()?;
        metadata.push(255);
        Some(metadata.into_boxed_slice())
    }

    fn tick(&self, _caller: &dyn EntityBase, _server: &Server) {
        let has_passenger = self.entity.has_passengers();
        let had_passenger = self.had_passenger.swap(has_passenger, Ordering::Relaxed);
        if had_passenger && !has_passenger && !self.entity.is_removed() {
            self.entity.world.load().play_sound(
                Sound::EntityCushionGetUp,
                SoundCategory::Blocks,
                &self.entity.pos.load(),
            );
        }

        if !self.entity.is_alive()
            || self.ticks_since_last_check.fetch_add(1, Ordering::Relaxed) < 100
        {
            return;
        }
        self.ticks_since_last_check.store(0, Ordering::Relaxed);

        let world = self.entity.world.load();
        self.destroy_if_in_fire(&world);
        if !self.entity.is_removed()
            && !Self::would_survive_at(&world, &self.entity.bounding_box.load())
        {
            self.drop_and_remove(None);
        }
    }

    fn on_lightning_strike(&self, _caller: &dyn EntityBase, lightning: &LightningBoltEntity) {
        if !self.entity.is_removed() {
            self.drop_and_remove(Some(lightning));
        }
    }

    fn can_hit(&self) -> bool {
        self.entity.is_alive()
    }

    fn is_collidable(&self, _entity: Option<Box<dyn EntityBase>>) -> bool {
        true
    }

    fn write_custom_nbt(&self, nbt: &mut NbtCompound) {
        nbt.put_byte("Color", self.color() as i8);
    }

    fn read_custom_nbt(&self, nbt: &NbtCompound) {
        if let Some(color) = nbt.get_byte("Color") {
            self.set_color(color as u8);
        }
    }

    fn interact(&self, player: &Arc<Player>, _item_stack: &mut ItemStack) -> bool {
        if player.get_entity().is_sneaking() {
            return false;
        }
        if !self
            .entity
            .passengers
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .is_empty()
        {
            return false;
        }

        let world = self.entity.world.load();
        let Some(vehicle) = world.get_entity_by_id(self.entity.entity_id) else {
            return false;
        };
        let Some(passenger) = world.get_player_by_id(player.entity_id()) else {
            return false;
        };

        world.play_sound(
            Sound::EntityCushionSit,
            SoundCategory::Blocks,
            &self.entity.pos.load(),
        );
        self.entity
            .add_passenger(vehicle, passenger as Arc<dyn EntityBase>);
        true
    }

    fn damage_with_context(
        &self,
        _caller: &dyn EntityBase,
        _amount: f32,
        _damage_type: DamageType,
        _position: Option<Vector3<f64>>,
        source: Option<&dyn EntityBase>,
        cause: Option<&dyn EntityBase>,
    ) -> bool {
        if !self.entity.is_alive() {
            return false;
        }

        let world = self.entity.world.load();
        if !world.level_info.load().game_rules.mob_griefing
            && cause.is_some_and(|cause| cause.get_mob().is_some())
        {
            return false;
        }

        let caused_by = cause.or(source);
        if caused_by
            .and_then(|entity| entity.get_player())
            .is_some_and(|player| {
                !player
                    .abilities
                    .try_lock()
                    .is_ok_and(|abilities| abilities.allow_modify_world)
            })
        {
            return false;
        }

        self.drop_and_remove(caused_by);
        true
    }

    fn cast_any(&self) -> &dyn std::any::Any {
        self
    }
}
