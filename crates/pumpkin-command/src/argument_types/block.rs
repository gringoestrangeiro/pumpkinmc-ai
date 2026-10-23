use crate::{
    argument_types::argument_type::{ArgumentType, JavaClientArgumentType},
    context::command_context::CommandContext,
    errors::command_syntax_error::CommandSyntaxError,
    errors::error_types::CommandErrorType,
    string_reader::StringReader,
    suggestion::suggestions::{Suggestions, SuggestionsBuilder},
};
use std::collections::HashSet;

use pumpkin_data::{Block, BlockStateId, Property, translation};
use pumpkin_util::text::TextComponent;

pub const INVALID_BLOCK_ERROR_TYPE: CommandErrorType<1> = CommandErrorType::new(
    translation::java::ARGUMENT_BLOCK_ID_INVALID,
    translation::java::ARGUMENT_BLOCK_ID_INVALID,
);

pub const UNKNOWN_BLOCK_PROPERTY_ERROR_TYPE: CommandErrorType<2> = CommandErrorType::new(
    translation::java::ARGUMENT_BLOCK_PROPERTY_UNKNOWN,
    translation::java::ARGUMENT_BLOCK_PROPERTY_UNKNOWN,
);

pub const DUPLICATE_BLOCK_PROPERTY_ERROR_TYPE: CommandErrorType<2> = CommandErrorType::new(
    translation::java::ARGUMENT_BLOCK_PROPERTY_DUPLICATE,
    translation::java::ARGUMENT_BLOCK_PROPERTY_DUPLICATE,
);

pub const INVALID_BLOCK_PROPERTY_VALUE_ERROR_TYPE: CommandErrorType<3> = CommandErrorType::new(
    translation::java::ARGUMENT_BLOCK_PROPERTY_INVALID,
    translation::java::ARGUMENT_BLOCK_PROPERTY_INVALID,
);

pub const MISSING_BLOCK_PROPERTY_VALUE_ERROR_TYPE: CommandErrorType<2> = CommandErrorType::new(
    translation::java::ARGUMENT_BLOCK_PROPERTY_NOVALUE,
    translation::java::ARGUMENT_BLOCK_PROPERTY_NOVALUE,
);

pub const UNCLOSED_BLOCK_PROPERTIES_ERROR_TYPE: CommandErrorType<0> = CommandErrorType::new(
    translation::java::ARGUMENT_BLOCK_PROPERTY_UNCLOSED,
    translation::java::ARGUMENT_BLOCK_PROPERTY_UNCLOSED,
);

#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash)]
pub struct BlockArgumentType;

impl<S: crate::source::CommandSource> ArgumentType<S> for BlockArgumentType {
    type Item = &'static Block;

    fn parse(&self, reader: &mut StringReader) -> Result<Self::Item, CommandSyntaxError> {
        let start = reader.cursor();
        while let Some(c) = reader.peek() {
            if c.is_alphanumeric() || c == '_' || c == ':' || c == '/' || c == '.' || c == '-' {
                reader.skip();
            } else {
                break;
            }
        }
        let block_name = &reader.string()[start..reader.cursor()];
        let normalized = if block_name.contains(':') {
            block_name.to_string()
        } else {
            format!("minecraft:{block_name}")
        };

        Block::from_name(&normalized)
            .ok_or_else(|| INVALID_BLOCK_ERROR_TYPE.create(reader, TextComponent::text(normalized)))
    }

    fn client_side_parser(&'_ self) -> JavaClientArgumentType {
        JavaClientArgumentType::BlockState
    }

    fn list_suggestions(
        &self,
        _context: &CommandContext<S>,
        builder: SuggestionsBuilder,
    ) -> Suggestions {
        builder.build()
    }
}

impl BlockArgumentType {
    pub fn get<S: crate::source::CommandSource>(
        context: &CommandContext<S>,
        name: &str,
    ) -> Result<&'static Block, CommandSyntaxError> {
        context.get_argument::<&'static Block>(name).copied()
    }
}

/// A block together with the state parsed from a block state argument
/// such as `minecraft:oak_stairs[facing=east]`.
#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash)]
pub struct BlockStateInput {
    pub block: &'static Block,
    pub state_id: BlockStateId,
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash)]
pub struct BlockStateArgumentType;

impl<S: crate::source::CommandSource> ArgumentType<S> for BlockStateArgumentType {
    type Item = BlockStateInput;

    fn parse(&self, reader: &mut StringReader) -> Result<Self::Item, CommandSyntaxError> {
        let block = <BlockArgumentType as ArgumentType<S>>::parse(&BlockArgumentType, reader)?;
        let state_id = if reader.peek() == Some('[') {
            parse_block_state(reader, block)?
        } else {
            block.default_state.id
        };

        Ok(BlockStateInput { block, state_id })
    }

    fn client_side_parser(&'_ self) -> JavaClientArgumentType {
        JavaClientArgumentType::BlockState
    }

    fn list_suggestions(
        &self,
        _context: &CommandContext<S>,
        builder: SuggestionsBuilder,
    ) -> Suggestions {
        builder.build()
    }
}

impl_copy_get!(BlockStateArgumentType, BlockStateInput);

fn block_name(block: &Block) -> TextComponent {
    TextComponent::text(format!("minecraft:{}", block.name))
}

fn parse_block_state(
    reader: &mut StringReader,
    block: &'static Block,
) -> Result<BlockStateId, CommandSyntaxError> {
    reader.skip(); // '['

    let mut properties = block
        .properties(block.default_state.id)
        .map(|properties| properties.to_props())
        .unwrap_or_default();
    let mut seen = HashSet::new();
    let mut state = block.default_state;

    loop {
        reader.skip_whitespace();
        match reader.peek() {
            Some(']') => {
                reader.skip();
                break;
            }
            None => return Err(UNCLOSED_BLOCK_PROPERTIES_ERROR_TYPE.create(reader)),
            Some(_) => {}
        }

        let key = reader.read_string()?;
        let Some(index) = properties
            .iter()
            .position(|property| property.key_str() == key)
        else {
            return Err(UNKNOWN_BLOCK_PROPERTY_ERROR_TYPE.create(
                reader,
                block_name(block),
                TextComponent::text(key),
            ));
        };
        if !seen.insert(properties[index].key()) {
            return Err(DUPLICATE_BLOCK_PROPERTY_ERROR_TYPE.create(
                reader,
                TextComponent::text(key),
                block_name(block),
            ));
        }

        reader.skip_whitespace();
        if reader.peek() != Some('=') {
            return Err(MISSING_BLOCK_PROPERTY_VALUE_ERROR_TYPE.create(
                reader,
                TextComponent::text(key),
                block_name(block),
            ));
        }
        reader.skip();
        reader.skip_whitespace();

        let value = reader.read_string()?;
        let Some(property) = Property::from_str_pair(&key, &value) else {
            return Err(INVALID_BLOCK_PROPERTY_VALUE_ERROR_TYPE.create(
                reader,
                block_name(block),
                TextComponent::text(value),
                TextComponent::text(key),
            ));
        };
        properties[index] = property;

        let Some(new_state) = block.state_from_properties(&properties) else {
            return Err(INVALID_BLOCK_PROPERTY_VALUE_ERROR_TYPE.create(
                reader,
                block_name(block),
                TextComponent::text(value),
                TextComponent::text(key),
            ));
        };
        state = new_state;

        reader.skip_whitespace();
        if reader.peek() == Some(',') {
            reader.skip();
        } else if reader.peek() != Some(']') {
            return Err(UNCLOSED_BLOCK_PROPERTIES_ERROR_TYPE.create(reader));
        }
    }

    Ok(state.id)
}

#[cfg(test)]
mod test {
    use pumpkin_data::{
        Block, Property,
        block_properties::{Facing, Half},
    };

    use crate::{
        argument_types::block::{
            BlockStateArgumentType, BlockStateInput, DUPLICATE_BLOCK_PROPERTY_ERROR_TYPE,
            INVALID_BLOCK_ERROR_TYPE, INVALID_BLOCK_PROPERTY_VALUE_ERROR_TYPE,
            UNCLOSED_BLOCK_PROPERTIES_ERROR_TYPE, UNKNOWN_BLOCK_PROPERTY_ERROR_TYPE,
        },
        string_reader::StringReader,
    };

    #[test]
    fn parse_default_state() {
        let mut reader = StringReader::new("minecraft:oak_stairs");
        assert_parse_ok_reset!(
            &mut reader,
            BlockStateArgumentType,
            BlockStateInput {
                block: &Block::OAK_STAIRS,
                state_id: Block::OAK_STAIRS.default_state.id,
            }
        );
    }

    #[test]
    fn parse_single_property() {
        let mut reader = StringReader::new("minecraft:oak_stairs[facing=east]");
        let expected = Block::OAK_STAIRS
            .from_properties(&[Property::Facing(Facing::East)])
            .to_state_id(&Block::OAK_STAIRS);
        assert_parse_ok_reset!(
            &mut reader,
            BlockStateArgumentType,
            BlockStateInput {
                block: &Block::OAK_STAIRS,
                state_id: expected,
            }
        );
    }

    #[test]
    fn parse_multiple_properties() {
        let mut reader = StringReader::new("minecraft:oak_stairs[half=top,facing=west]");
        let expected = Block::OAK_STAIRS
            .from_properties(&[Property::Half(Half::Top), Property::Facing(Facing::West)])
            .to_state_id(&Block::OAK_STAIRS);
        assert_parse_ok_reset!(
            &mut reader,
            BlockStateArgumentType,
            BlockStateInput {
                block: &Block::OAK_STAIRS,
                state_id: expected,
            }
        );
    }

    #[test]
    fn reject_unknown_property() {
        let mut reader = StringReader::new("minecraft:oak_stairs[foo=bar]");
        assert_parse_err_reset!(
            &mut reader,
            BlockStateArgumentType,
            &UNKNOWN_BLOCK_PROPERTY_ERROR_TYPE
        );
    }

    #[test]
    fn reject_invalid_property_value() {
        let mut reader = StringReader::new("minecraft:oak_stairs[facing=sideways]");
        assert_parse_err_reset!(
            &mut reader,
            BlockStateArgumentType,
            &INVALID_BLOCK_PROPERTY_VALUE_ERROR_TYPE
        );

        // Parses as a facing, but stairs only accept horizontal facings.
        let mut reader = StringReader::new("minecraft:oak_stairs[facing=up]");
        assert_parse_err_reset!(
            &mut reader,
            BlockStateArgumentType,
            &INVALID_BLOCK_PROPERTY_VALUE_ERROR_TYPE
        );
    }

    #[test]
    fn reject_duplicate_property() {
        let mut reader = StringReader::new("minecraft:oak_stairs[facing=east,facing=west]");
        assert_parse_err_reset!(
            &mut reader,
            BlockStateArgumentType,
            &DUPLICATE_BLOCK_PROPERTY_ERROR_TYPE
        );
    }

    #[test]
    fn reject_unclosed_properties() {
        let mut reader = StringReader::new("minecraft:oak_stairs[facing=east");
        assert_parse_err_reset!(
            &mut reader,
            BlockStateArgumentType,
            &UNCLOSED_BLOCK_PROPERTIES_ERROR_TYPE
        );
    }

    #[test]
    fn reject_unknown_block() {
        let mut reader = StringReader::new("minecraft:not_a_block");
        assert_parse_err_reset!(
            &mut reader,
            BlockStateArgumentType,
            &INVALID_BLOCK_ERROR_TYPE
        );
    }
}
