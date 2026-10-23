use pumpkin_data::{
    BlockState,
    material_rule::{ConditionMaterialRule, MaterialRule, SequenceMaterialRule},
};

use super::MaterialRuleContext;
use crate::{
    ProtoChunk,
    generation::{
        noise::router::surface_height_sampler::SurfaceHeightEstimateSampler,
        surface::test_condition,
    },
};

pub fn try_apply_material_rule(
    rule: &MaterialRule,
    chunk: &mut ProtoChunk,
    context: &mut MaterialRuleContext,
    surface_height_estimate_sampler: &mut SurfaceHeightEstimateSampler,
) -> Option<&'static BlockState> {
    match rule {
        MaterialRule::Badlands(_badlands) => Some(BadLandsMaterialRule::try_apply(context)),
        MaterialRule::Block(block) => Some(BlockMaterialRule::try_apply(block.result_state)),
        MaterialRule::Sequence(sequence) => {
            try_apply_sequence(sequence, chunk, context, surface_height_estimate_sampler)
        }
        MaterialRule::Condition(condition) => {
            try_apply_condition(condition, chunk, context, surface_height_estimate_sampler)
        }
        MaterialRule::OreVein(ore_vein) => context.try_apply_ore_vein(ore_vein),
    }
}

/// The block Y range `[min, max)` that the rule's ore vein rules can place in, if it has any.
#[must_use]
pub fn ore_vein_y_range(rule: &MaterialRule) -> Option<(i32, i32)> {
    match rule {
        MaterialRule::OreVein(ore_vein) => Some((ore_vein.min_y, ore_vein.max_y)),
        MaterialRule::Sequence(sequence) => sequence
            .sequence
            .iter()
            .filter_map(ore_vein_y_range)
            .reduce(|(min_a, max_a), (min_b, max_b)| (min_a.min(min_b), max_a.max(max_b))),
        MaterialRule::Condition(condition) => ore_vein_y_range(condition.then_run),
        MaterialRule::Badlands(_) | MaterialRule::Block(_) => None,
    }
}

pub struct BadLandsMaterialRule;

impl BadLandsMaterialRule {
    pub fn try_apply(context: &mut MaterialRuleContext) -> &'static BlockState {
        context.terrain_builder.get_terracotta_block(
            context.block_pos_x,
            context.block_pos_y,
            context.block_pos_z,
        )
    }
}

pub struct BlockMaterialRule;

impl BlockMaterialRule {
    pub const fn try_apply(state: &'static BlockState) -> &'static BlockState {
        state
    }
}

pub fn try_apply_sequence(
    rule: &SequenceMaterialRule,
    chunk: &mut ProtoChunk,
    context: &mut MaterialRuleContext,
    surface_height_estimate_sampler: &mut SurfaceHeightEstimateSampler,
) -> Option<&'static BlockState> {
    for seq in rule.sequence {
        if let Some(state) =
            try_apply_material_rule(seq, chunk, context, surface_height_estimate_sampler)
        {
            return Some(state);
        }
    }
    None
}

pub fn try_apply_condition(
    rule: &ConditionMaterialRule,
    chunk: &mut ProtoChunk,
    context: &mut MaterialRuleContext,
    surface_height_estimate_sampler: &mut SurfaceHeightEstimateSampler,
) -> Option<&'static BlockState> {
    if test_condition(
        &rule.if_true,
        chunk,
        context,
        surface_height_estimate_sampler,
    ) {
        return try_apply_material_rule(
            rule.then_run,
            chunk,
            context,
            surface_height_estimate_sampler,
        );
    }
    None
}
