#[allow(clippy::wildcard_imports)]
use super::*;
use crate::block::entities::structure_block::StructureBlockBlockEntity;
use crate::entity::r#type::from_type;
use crate::world::block_placer::WorldBlockPlacer;
use pumpkin_data::block_properties::{StructureBlockLikeProperties, StructureblockMode};
use pumpkin_data::entity::EntityType;
use pumpkin_data::{Mirror, Rotation};
use pumpkin_nbt::tag::NbtTag;
use pumpkin_protocol::java::server::play::SSetStructureBlock;
use pumpkin_world::generation::structure::template::{
    StructureTemplate, global_cache, place_structure_template,
};
use uuid::Uuid;

/// Vanilla `StructureBlockEntity.MAX_OFFSET_PER_AXIS`.
const MAX_OFFSET_PER_AXIS: i32 = 48;
/// Vanilla `StructureBlockEntity.MAX_SIZE_PER_AXIS`.
const MAX_SIZE_PER_AXIS: i32 = 48;
/// Vanilla reads the metadata field with `readUtf(128)`.
const MAX_METADATA_LENGTH: usize = 128;
/// Vanilla implicit namespace.
const DEFAULT_NAMESPACE: &str = "minecraft";
/// Vanilla `StructureTemplateManager.STRUCTURE_DIRECTORY_NAME` in 26.3.
const STRUCTURE_DIRECTORY: &str = "structure";
const GENERATED_DIRECTORY: &str = "generated";

impl JavaClient {
    #[allow(clippy::too_many_lines)]
    pub fn handle_set_structure_block(&self, player: &Player, packet: &SSetStructureBlock<'_>) {
        if player.permission_lvl.load() < PermissionLvl::Two {
            return;
        }

        let world = player.world();
        let position = packet.location;
        let Some(block_entity) = world.get_block_entity(&position) else {
            return;
        };
        if block_entity.resource_location() != StructureBlockBlockEntity::ID {
            return;
        }
        let Some(structure_block) = block_entity
            .as_any()
            .downcast_ref::<StructureBlockBlockEntity>()
        else {
            return;
        };

        let Some((mode_state, mode_name)) = structure_mode(packet.mode.0) else {
            return;
        };
        let mirror = mirror_from_index(packet.mirror.0);
        let rotation = rotation_from_index(packet.rotation.0);
        let offset = BlockPos::new(
            i32::from(packet.offset_x).clamp(-MAX_OFFSET_PER_AXIS, MAX_OFFSET_PER_AXIS),
            i32::from(packet.offset_y).clamp(-MAX_OFFSET_PER_AXIS, MAX_OFFSET_PER_AXIS),
            i32::from(packet.offset_z).clamp(-MAX_OFFSET_PER_AXIS, MAX_OFFSET_PER_AXIS),
        );
        let size = Vector3::new(
            i32::from(packet.size_x).clamp(0, MAX_SIZE_PER_AXIS),
            i32::from(packet.size_y).clamp(0, MAX_SIZE_PER_AXIS),
            i32::from(packet.size_z).clamp(0, MAX_SIZE_PER_AXIS),
        );
        let structure_name = parse_structure_identifier(packet.name);

        *structure_block
            .name
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner) =
            structure_name.clone().unwrap_or_default();
        *structure_block
            .mode
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner) = mode_name.to_string();
        *structure_block
            .pos_x
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner) = offset.0.x;
        *structure_block
            .pos_y
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner) = offset.0.y;
        *structure_block
            .pos_z
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner) = offset.0.z;
        *structure_block
            .size_x
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner) = size.x;
        *structure_block
            .size_y
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner) = size.y;
        *structure_block
            .size_z
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner) = size.z;
        *structure_block
            .rotation
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner) =
            rotation_name(rotation).to_string();
        *structure_block
            .mirror
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner) = mirror_name(mirror).to_string();
        *structure_block
            .metadata
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner) =
            packet.metadata.chars().take(MAX_METADATA_LENGTH).collect();
        *structure_block
            .ignore_entities
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner) = packet.ignore_entities();
        *structure_block
            .strict
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner) = packet.is_strict();
        *structure_block
            .show_air
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner) = packet.show_air();
        *structure_block
            .show_bounding_box
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner) = packet.show_bounding_box();
        *structure_block
            .integrity
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner) = packet.integrity.clamp(0.0, 1.0);
        *structure_block
            .seed
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner) = packet.seed.0;

        update_block_mode(&world, &position, mode_state);

        let Some(name) = structure_name else {
            world.update_block_entity(&block_entity);
            return;
        };

        if packet.action.0 == SSetStructureBlock::ACTION_SAVE_AREA
            && mode_state == StructureblockMode::Save
        {
            save_structure(
                player,
                &world,
                position.offset(offset.0),
                size,
                structure_block,
                &name,
            );
        } else if packet.action.0 == SSetStructureBlock::ACTION_LOAD_AREA
            && mode_state == StructureblockMode::Load
        {
            load_structure(
                &world,
                position.offset(offset.0),
                size,
                structure_block,
                &name,
                rotation,
                mirror,
                packet.seed.0,
                packet.ignore_entities(),
            );
        }

        world.update_block_entity(&block_entity);
    }
}

/// Writes a structure block's range into a template and saves it under `generated`.
fn save_structure(
    player: &Player,
    world: &Arc<World>,
    position: BlockPos,
    size: Vector3<i32>,
    structure_block: &StructureBlockBlockEntity,
    name: &str,
) {
    let Some((namespace, path)) = split_structure_identifier(name) else {
        return;
    };
    let ignore_entities = *structure_block
        .ignore_entities
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);

    let mut template = StructureTemplate::default();
    template.fill_from_world(
        position.0,
        size,
        !ignore_entities,
        &["minecraft:structure_void"],
        world.as_ref(),
    );
    template.set_author(player.gameprofile.name.clone());

    let Ok(bytes) = pumpkin_nbt::nbt_compress::write_gzip_compound_tag_to_bytes(template.save())
    else {
        warn!("Failed to serialize structure '{name}'");
        return;
    };

    let file = world
        .level
        .level_folder
        .root_folder
        .join(GENERATED_DIRECTORY)
        .join(&namespace)
        .join(STRUCTURE_DIRECTORY)
        .join(format!("{path}.nbt"));
    if let Some(parent) = file.parent()
        && let Err(error) = std::fs::create_dir_all(parent)
    {
        warn!(
            "Failed to create structure directory '{}': {error}",
            parent.display()
        );
    } else if let Err(error) = std::fs::write(&file, &bytes) {
        warn!(
            "Failed to save structure '{name}' to '{}': {error}",
            file.display()
        );
    }

    // Vanilla keeps the template in memory even when writing it to disk fails.
    global_cache().register_template(name, Arc::from(bytes.into_boxed_slice()));
}

/// Vanilla `StructureBlockEntity.placeStructureIfSameSize`.
#[allow(clippy::too_many_arguments)]
fn load_structure(
    world: &Arc<World>,
    position: BlockPos,
    size: Vector3<i32>,
    structure_block: &StructureBlockBlockEntity,
    name: &str,
    rotation: Rotation,
    mirror: Mirror,
    seed: i64,
    ignore_entities: bool,
) {
    let Some(template) = global_cache().get(name) else {
        return;
    };

    load_structure_info(structure_block, &template);
    if template.size != size {
        return;
    }

    let integrity = *structure_block
        .integrity
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    let mut placer = WorldBlockPlacer::new(world);
    if !place_structure_template(
        &mut placer,
        &template,
        position.0,
        rotation,
        mirror,
        integrity,
        seed,
    ) {
        return;
    }

    placer.remove_overwritten_block_entities();
    // Send the block changes before the block entity data so clients already have the
    // new blocks when the data packets arrive.
    world.queue_block_updates(&placer.changed_positions);
    world.flush_block_updates();
    placer.finalize();
    if !ignore_entities {
        place_structure_entities(world, &template, position, rotation, mirror);
    }
}

/// Vanilla `StructureBlockEntity.loadStructureInfo`.
fn load_structure_info(structure_block: &StructureBlockBlockEntity, template: &StructureTemplate) {
    (*structure_block
        .author
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner))
    .clone_from(&template.author);
    *structure_block
        .size_x
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner) = template.size.x;
    *structure_block
        .size_y
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner) = template.size.y;
    *structure_block
        .size_z
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner) = template.size.z;
}

/// Vanilla `StructureTemplate.placeEntities`.
fn place_structure_entities(
    world: &Arc<World>,
    template: &StructureTemplate,
    position: BlockPos,
    rotation: Rotation,
    mirror: Mirror,
) {
    for entity_info in template.entity_info_list() {
        let Some(id) = entity_info.nbt.get_string("id") else {
            continue;
        };
        let Some(entity_type) = EntityType::from_name(id.strip_prefix("minecraft:").unwrap_or(id))
        else {
            continue;
        };

        let relative = StructureTemplate::transform_vec3(
            entity_info.pos,
            mirror,
            rotation,
            Vector3::new(0, 0, 0),
        );
        let spawn_pos = Vector3::new(
            f64::from(position.0.x) + relative.x,
            f64::from(position.0.y) + relative.y,
            f64::from(position.0.z) + relative.z,
        );

        let mut nbt = entity_info.nbt.clone();
        nbt.child_tags.remove("UUID");
        nbt.put(
            "Pos",
            NbtTag::List(vec![
                spawn_pos.x.into(),
                spawn_pos.y.into(),
                spawn_pos.z.into(),
            ]),
        );

        let entity = from_type(entity_type, spawn_pos, world, Uuid::new_v4());
        entity.read_nbt_non_mut(&nbt);
        world.spawn_entity(entity);
    }
}

/// Updates the structure block's `mode` block state property.
fn update_block_mode(world: &Arc<World>, position: &BlockPos, mode: StructureblockMode) {
    let state_id = world.get_block_state_id(position);
    let block = Block::from_state_id(state_id);
    if block.id != Block::STRUCTURE_BLOCK.id {
        return;
    }

    let mut props = StructureBlockLikeProperties::from_state_id(state_id);
    props.r#mode = mode;
    world.set_block_state(
        position,
        props.to_state_id(block),
        BlockFlags::NOTIFY_LISTENERS | BlockFlags::UPDATE_KNOWN_SHAPE,
    );
}

const fn structure_mode(index: i32) -> Option<(StructureblockMode, &'static str)> {
    match index {
        SSetStructureBlock::MODE_SAVE => Some((StructureblockMode::Save, "SAVE")),
        SSetStructureBlock::MODE_LOAD => Some((StructureblockMode::Load, "LOAD")),
        SSetStructureBlock::MODE_CORNER => Some((StructureblockMode::Corner, "CORNER")),
        SSetStructureBlock::MODE_DATA => Some((StructureblockMode::Data, "DATA")),
        _ => None,
    }
}

/// Vanilla `StructureBlockEntity` stores mirror and rotation with their enum names.
const fn mirror_from_index(index: i32) -> Mirror {
    match index {
        1 => Mirror::LeftRight,
        2 => Mirror::FrontBack,
        _ => Mirror::None,
    }
}

const fn rotation_from_index(index: i32) -> Rotation {
    match index {
        1 => Rotation::Clockwise90,
        2 => Rotation::Rotate180,
        3 => Rotation::CounterClockwise90,
        _ => Rotation::None,
    }
}

const fn mirror_name(mirror: Mirror) -> &'static str {
    match mirror {
        Mirror::None => "NONE",
        Mirror::LeftRight => "LEFT_RIGHT",
        Mirror::FrontBack => "FRONT_BACK",
    }
}

const fn rotation_name(rotation: Rotation) -> &'static str {
    match rotation {
        Rotation::None => "NONE",
        Rotation::Clockwise90 => "CLOCKWISE_90",
        Rotation::Rotate180 => "CLOCKWISE_180",
        Rotation::CounterClockwise90 => "COUNTERCLOCKWISE_90",
    }
}

/// Validates a structure id like vanilla `Identifier.tryParse`, returning its canonical form.
fn parse_structure_identifier(name: &str) -> Option<String> {
    let (namespace, path) = match name.split_once(':') {
        Some((namespace, path)) => (namespace, path),
        None => (DEFAULT_NAMESPACE, name),
    };

    if !valid_namespace(namespace) || !valid_path(path) {
        return None;
    }
    Some(format!("{namespace}:{path}"))
}

fn split_structure_identifier(name: &str) -> Option<(String, String)> {
    let (namespace, path) = name.split_once(':')?;
    Some((namespace.to_string(), path.to_string()))
}

fn valid_namespace(namespace: &str) -> bool {
    !namespace.is_empty()
        && namespace != "."
        && namespace != ".."
        && namespace
            .chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || matches!(c, '_' | '-' | '.'))
}

fn valid_path(path: &str) -> bool {
    !path.is_empty()
        && path
            .chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || matches!(c, '_' | '-' | '.' | '/'))
        // Reject empty segments and `.`/`..`, which would escape the structures directory.
        && !path
            .split('/')
            .any(|segment| segment.is_empty() || segment == "." || segment == "..")
}
