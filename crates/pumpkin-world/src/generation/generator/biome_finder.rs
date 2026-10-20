use pumpkin_data::chunk::{Biome, BiomeTree, NETHER_BIOME_SOURCE, OVERWORLD_BIOME_SOURCE};
use pumpkin_data::dimension::Dimension;
use pumpkin_util::math::position::BlockPos;
use rustc_hash::FxHashSet;

use crate::biome::{BiomeSupplier, MultiNoiseBiomeSupplier, end::TheEndBiomeSupplier};
use crate::generation::biome_coords;
use crate::generation::noise::router::multi_noise_sampler::MultiNoiseSampler;

use super::{VanillaGenerator, WorldGenerator};

/// Finds the closest position whose sampled biome id is contained in
/// `targets`, mirroring vanilla's `BiomeSource.findClosestBiome3d`.
///
/// The search walks vanilla's square spiral from `origin` in steps of
/// `horizontal_step` blocks up to `horizontal_radius`, probing each column at
/// y levels spreading out from the origin y in steps of `vertical_step`
/// (staying within the dimension's build height). The first match wins, which
/// approximates the nearest one just like vanilla.
///
/// Returns the block position of the sample point plus the concrete biome
/// found there.
#[must_use]
#[expect(clippy::implicit_hasher)]
pub fn find_closest_biome_3d(
    world_gen: &WorldGenerator,
    origin: BlockPos,
    targets: &FxHashSet<u8>,
    horizontal_radius: i32,
    horizontal_step: i32,
    vertical_step: i32,
) -> Option<(BlockPos, &'static Biome)> {
    match world_gen {
        WorldGenerator::Flat(flat) => {
            // Superflat worlds use a single fixed biome everywhere, matching
            // the resolution in `FlatGenerator::step_to_biomes`.
            let biome = flat.biome();
            targets.contains(&biome.id).then_some((origin, biome))
        }
        WorldGenerator::Noise(generator) => find_in_noise_world(
            generator,
            origin,
            targets,
            horizontal_radius,
            horizontal_step,
            vertical_step,
        ),
        // Plugin generators only expose biomes by generating a whole chunk, so
        // there is no sampler to search against.
        WorldGenerator::Custom(_) => None,
    }
}

fn find_in_noise_world(
    generator: &VanillaGenerator,
    origin: BlockPos,
    targets: &FxHashSet<u8>,
    horizontal_radius: i32,
    horizontal_step: i32,
    vertical_step: i32,
) -> Option<(BlockPos, &'static Biome)> {
    // Vanilla intersects the request with `BiomeSource#possibleBiomes` first,
    // so asking for a biome that cannot generate in this dimension returns
    // immediately instead of scanning the whole search radius.
    if targets.is_disjoint(&possible_biomes(&generator.dimension)) {
        return None;
    }

    let supplier: &dyn BiomeSupplier = if generator.dimension == Dimension::THE_END {
        &TheEndBiomeSupplier
    } else if generator.dimension == Dimension::THE_NETHER {
        &MultiNoiseBiomeSupplier::NETHER
    } else {
        &MultiNoiseBiomeSupplier::OVERWORLD
    };

    let mut sampler = MultiNoiseSampler::generate(&generator.base_router.multi_noise);

    // `level.getMinY() + 1` to `level.getMaxY() + 1` in vanilla.
    let min_y = i32::from(generator.settings.shape.min_y) + 1;
    let max_y =
        i32::from(generator.settings.shape.min_y) + i32::from(generator.settings.shape.height);
    let ys = out_from_origin(origin.0.y, min_y, max_y, vertical_step);

    let check_column = |x: i32, z: i32, sampler: &mut MultiNoiseSampler| {
        let biome_x = biome_coords::from_block(x);
        let biome_z = biome_coords::from_block(z);
        for &y in &ys {
            let biome = supplier.biome(biome_x, biome_coords::from_block(y), biome_z, sampler);
            if targets.contains(&biome.id) {
                return Some((BlockPos::new(x, y, z), biome));
            }
        }
        None
    };

    // Vanilla walks a square spiral from the origin instead of one ring at a
    // time. The first match therefore differs from a ring-major scan.
    let rings = horizontal_radius / horizontal_step;
    for (dx, dz) in SpiralAround::new(rings) {
        let x = origin.0.x + dx * horizontal_step;
        let z = origin.0.z + dz * horizontal_step;
        if let Some(found) = check_column(x, z, &mut sampler) {
            return Some(found);
        }
    }

    None
}

/// Offsets in the order of vanilla's
/// `BlockPos.spiralAround(ZERO, radius, EAST, SOUTH)`: every position of the
/// `(2 * radius + 1)` square around the origin, starting at the origin and
/// walking one leg per side.
struct SpiralAround {
    x: i32,
    z: i32,
    leg: i32,
    leg_size: i32,
    leg_index: i32,
    legs: i32,
    first: bool,
}

impl SpiralAround {
    const fn new(radius: i32) -> Self {
        Self {
            x: 0,
            z: 1,
            leg: -1,
            leg_size: 0,
            leg_index: 0,
            legs: 4 * radius,
            first: true,
        }
    }
}

impl Iterator for SpiralAround {
    type Item = (i32, i32);

    fn next(&mut self) -> Option<Self::Item> {
        if self.first {
            self.first = false;
            // Vanilla's cursor starts one step along the second direction.
            self.x = 0;
            self.z = 0;
        } else {
            let (dx, dz) = match self.leg.rem_euclid(4) {
                0 => (1, 0),
                1 => (0, 1),
                2 => (-1, 0),
                _ => (0, -1),
            };
            self.x += dx;
            self.z += dz;
        }

        if self.leg_index >= self.leg_size {
            if self.leg >= self.legs {
                return None;
            }
            self.leg += 1;
            self.leg_index = 0;
            self.leg_size = self.leg / 2 + 1;
        }
        self.leg_index += 1;
        Some((self.x, self.z))
    }
}

/// The ids of every biome the given dimension's biome source can produce.
#[must_use]
pub fn possible_biomes(dimension: &Dimension) -> FxHashSet<u8> {
    let mut out = FxHashSet::default();
    if *dimension == Dimension::THE_END {
        // Matches the fixed set in `TheEndBiomeSupplier`.
        for biome in [
            &Biome::THE_END,
            &Biome::END_HIGHLANDS,
            &Biome::END_MIDLANDS,
            &Biome::SMALL_END_ISLANDS,
            &Biome::END_BARRENS,
        ] {
            out.insert(biome.id);
        }
    } else if *dimension == Dimension::THE_NETHER {
        collect_tree_biomes(&NETHER_BIOME_SOURCE, &mut out);
    } else {
        collect_tree_biomes(&OVERWORLD_BIOME_SOURCE, &mut out);
    }
    out
}

fn collect_tree_biomes(tree: &'static BiomeTree, out: &mut FxHashSet<u8>) {
    match tree {
        BiomeTree::Leaf { biome, .. } => {
            out.insert(biome.id);
        }
        BiomeTree::Branch { nodes, .. } => {
            for node in *nodes {
                collect_tree_biomes(node, out);
            }
        }
    }
}

/// Y levels to probe, ordered outwards from `origin` (upwards first) and
/// clamped to `[min, max]`, mirroring vanilla's `Mth.outFromOrigin`.
fn out_from_origin(origin: i32, min: i32, max: i32, step: i32) -> Vec<i32> {
    let start = origin.clamp(min, max);
    let mut ys = vec![start];
    let mut distance = step;
    loop {
        let up = start + distance;
        let down = start - distance;
        if up > max && down < min {
            break;
        }
        if up <= max {
            ys.push(up);
        }
        if down >= min {
            ys.push(down);
        }
        distance += step;
    }
    ys
}

#[cfg(test)]
mod test {
    use pumpkin_data::chunk::Biome;
    use pumpkin_data::dimension::Dimension;
    use pumpkin_util::math::position::BlockPos;
    use pumpkin_util::world_seed::Seed;
    use rustc_hash::FxHashSet;

    use super::super::flat::FlatGenerator;
    use super::super::{GeneratorInit, VanillaGenerator, WorldGenerator};
    use super::{SpiralAround, find_closest_biome_3d, out_from_origin};

    fn targets(biomes: &[&Biome]) -> FxHashSet<u8> {
        biomes.iter().map(|biome| biome.id).collect()
    }

    #[test]
    fn y_levels_spread_outwards() {
        assert_eq!(
            out_from_origin(64, -63, 320, 64),
            vec![64, 128, 0, 192, 256, 320]
        );
        // Origin outside the bounds gets clamped first.
        assert_eq!(out_from_origin(-500, -63, 320, 200), vec![-63, 137]);
    }

    /// The first match of `/locate biome` depends on this order, so it has to
    /// be the order of vanilla's `BlockPos.spiralAround`.
    #[test]
    fn spiral_matches_vanilla_order() {
        assert_eq!(SpiralAround::new(0).collect::<Vec<_>>(), vec![(0, 0)]);
        assert_eq!(
            SpiralAround::new(1).collect::<Vec<_>>(),
            vec![
                (0, 0),
                (1, 0),
                (1, 1),
                (0, 1),
                (-1, 1),
                (-1, 0),
                (-1, -1),
                (0, -1),
                (1, -1),
            ]
        );
    }

    #[test]
    fn finds_known_biome() {
        // Seed 13579 has a desert around block (-96, 4, 32); see
        // `biome::test::biome_desert`.
        let world_gen = WorldGenerator::Noise(Box::new(VanillaGenerator::new(
            Seed(13579),
            Dimension::OVERWORLD,
        )));

        let (pos, biome) = find_closest_biome_3d(
            &world_gen,
            BlockPos::new(-96, 4, 32),
            &targets(&[&Biome::DESERT]),
            6400,
            32,
            64,
        )
        .expect("a desert should be within range");
        assert_eq!(biome.id, Biome::DESERT.id);
        assert_eq!(pos, BlockPos::new(-96, 4, 32));
    }

    #[test]
    fn short_circuits_impossible_dimension_biomes() {
        let world_gen = WorldGenerator::Noise(Box::new(VanillaGenerator::new(
            Seed(13579),
            Dimension::OVERWORLD,
        )));

        // A nether biome can never generate in the overworld; this must
        // return without scanning the whole search radius.
        assert!(
            find_closest_biome_3d(
                &world_gen,
                BlockPos::new(0, 64, 0),
                &targets(&[&Biome::CRIMSON_FOREST]),
                6400,
                32,
                64,
            )
            .is_none()
        );
    }

    #[test]
    fn flat_world_has_a_single_fixed_biome() {
        let world_gen = WorldGenerator::Flat(Box::new(FlatGenerator::new(
            Seed(0),
            Dimension::OVERWORLD,
            Vec::new(),
            "minecraft:plains".to_string(),
        )));

        let origin = BlockPos::new(17, 64, -3);
        let (pos, biome) = find_closest_biome_3d(
            &world_gen,
            origin,
            &targets(&[&Biome::PLAINS]),
            6400,
            32,
            64,
        )
        .expect("the flat biome is everywhere");
        assert_eq!(biome.id, Biome::PLAINS.id);
        assert_eq!(pos, origin);

        assert!(
            find_closest_biome_3d(
                &world_gen,
                origin,
                &targets(&[&Biome::DESERT]),
                6400,
                32,
                64
            )
            .is_none()
        );
    }
}
