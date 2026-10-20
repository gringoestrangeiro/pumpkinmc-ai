use std::collections::HashMap;

use pumpkin_data::{Block, BlockState, BlockStateId, Property};
use pumpkin_nbt::compound::NbtCompound;
use pumpkin_nbt::tag::NbtTag;
use serde::{Deserialize, Deserializer, Serialize, Serializer};

#[derive(Serialize, Deserialize, Clone)]
#[serde(rename_all = "PascalCase")]
pub struct BlockStateCodec {
    /// Block name
    #[serde(
        deserialize_with = "parse_block_name",
        serialize_with = "block_to_string"
    )]
    pub name: &'static Block,
    /// Key-value pairs of properties
    #[serde(skip_serializing_if = "Option::is_none")]
    pub properties: Option<HashMap<String, String>>,
}

fn parse_block_name<'de, D: Deserializer<'de>>(
    deserializer: D,
) -> Result<&'static Block, D::Error> {
    let s = String::deserialize(deserializer)?;
    let block =
        Block::from_name(s.as_str()).ok_or(serde::de::Error::custom("Invalid block name"))?;
    Ok(block)
}

fn block_to_string<S: Serializer>(block: &'static Block, serializer: S) -> Result<S::Ok, S::Error> {
    serializer.serialize_str(block.name)
}

impl BlockStateCodec {
    /// Vanilla `BlockState.CODEC` NBT form: `{Name, Properties}`, with `Properties`
    /// omitted for blocks that have none.
    #[must_use]
    pub fn to_nbt(state_id: BlockStateId) -> NbtCompound {
        let block = Block::from_state_id(state_id);
        let mut nbt = NbtCompound::new();
        nbt.put_string("Name", format!("minecraft:{}", block.name));

        if let Some(properties) = block.properties(state_id) {
            let properties = properties.to_props();
            if !properties.is_empty() {
                let mut compound = NbtCompound::new();
                for property in properties {
                    compound.put_string(property.key_str(), property.value_str().to_string());
                }
                nbt.put_compound("Properties", compound);
            }
        }
        nbt
    }

    /// Inverse of [`Self::to_nbt`]. `None` for a missing or unknown `Name`; unknown or
    /// absent properties fall back to the block's defaults.
    #[must_use]
    pub fn from_nbt(nbt: &NbtCompound) -> Option<BlockStateId> {
        let block = Block::from_name(nbt.get_string("Name")?)?;
        let Some(properties) = nbt.get_compound("Properties") else {
            return Some(block.default_state.id);
        };

        let properties: Vec<Property> = properties
            .child_tags
            .iter()
            .filter_map(|(key, tag)| match tag {
                NbtTag::String(value) => Property::from_str_pair(key, value),
                _ => None,
            })
            .collect();
        Some(block.from_properties(&properties).to_state_id(block))
    }

    #[must_use]
    pub fn get_state(&self) -> &'static BlockState {
        let state_id = self.get_state_id();
        BlockState::from_id(state_id)
    }

    #[must_use]
    pub const fn get_block(&self) -> &'static Block {
        self.name
    }

    /// Prefer this over `get_state` when the only the state ID is needed
    #[must_use]
    pub fn get_state_id(&self) -> BlockStateId {
        let block = self.name;

        let Some(properties_map) = &self.properties else {
            return block.default_state.id;
        };

        let props_iter = properties_map
            .iter()
            .filter_map(|(k, v)| Property::from_str_pair(k, v))
            .collect::<Vec<Property>>();

        let block_properties = block.from_properties(&props_iter);
        block_properties.to_state_id(block)
    }
}

#[cfg(test)]
mod test {
    use pumpkin_data::{Block, BlockStateId};
    use pumpkin_nbt::compound::NbtCompound;

    use super::BlockStateCodec;
    use crate::chunk::palette::BLOCK_NETWORK_MAX_BITS;

    #[test]
    fn block_state_nbt_round_trips_name_and_properties() {
        let wet_stairs = Block::OAK_STAIRS
            .set_waterlogged(Block::OAK_STAIRS.default_state.id, true)
            .expect("oak stairs support waterlogging");
        let nbt = BlockStateCodec::to_nbt(wet_stairs);
        assert_eq!(nbt.get_string("Name"), Some("minecraft:oak_stairs"));
        let properties = nbt
            .get_compound("Properties")
            .expect("waterlogged state writes properties");
        assert_eq!(properties.get_string("waterlogged"), Some("true"));
        assert_eq!(BlockStateCodec::from_nbt(&nbt), Some(wet_stairs));

        let stone = BlockStateCodec::to_nbt(Block::STONE.default_state.id);
        assert!(stone.get_compound("Properties").is_none());
        assert_eq!(
            BlockStateCodec::from_nbt(&stone),
            Some(Block::STONE.default_state.id)
        );
    }

    #[test]
    fn block_state_nbt_rejects_missing_or_unknown_name() {
        let mut nbt = NbtCompound::new();
        nbt.put_string("Name", "minecraft:not_a_real_block".to_string());
        assert_eq!(BlockStateCodec::from_nbt(&nbt), None);
        assert_eq!(BlockStateCodec::from_nbt(&NbtCompound::new()), None);
    }

    #[test]
    fn proper_network_bits_per_entry() {
        let addressable = 1u32 << BLOCK_NETWORK_MAX_BITS;
        assert!(
            u32::from(BlockStateId::COUNT) <= addressable,
            "We need to update our constants! {} states do not fit in {BLOCK_NETWORK_MAX_BITS} bits",
            BlockStateId::COUNT
        );
    }
}
