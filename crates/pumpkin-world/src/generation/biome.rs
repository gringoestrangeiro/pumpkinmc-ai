use pumpkin_util::math::{square_f64, vector3::Vector3};

use super::biome_coords;

// This blends biome boundaries, returning which biome to populate the surface on based on the seed
pub fn get_biome_blend(
    bottom_y: i8,
    height: u16,
    seed: i64,
    x: i32,
    y: i32,
    z: i32,
) -> Vector3<i32> {
    blend_with_offsets(bottom_y, height, x, y, z, |corner_x, corner_y, corner_z| {
        fiddle_offsets(seed, corner_x, corner_y, corner_z)
    })
}

/// [`get_biome_blend`] for many blocks of one area: each biome corner's offsets are hashed once
/// instead of once for every block that reads them.
pub struct BiomeFiddleCache {
    seed: i64,
    min_x: i32,
    min_y: i32,
    min_z: i32,
    size_x: usize,
    size_y: usize,
    size_z: usize,
    offsets: Box<[[f64; 3]]>,
}

impl BiomeFiddleCache {
    /// Precomputes the offsets of every biome corner that blending a block in the inclusive
    /// block box `min..=max` reads.
    #[must_use]
    pub fn new(seed: i64, min: Vector3<i32>, max: Vector3<i32>) -> Self {
        let min_x = biome_coords::from_block(min.x - 2);
        let min_y = biome_coords::from_block(min.y - 2);
        let min_z = biome_coords::from_block(min.z - 2);
        let size_x = (biome_coords::from_block(max.x - 2) + 2 - min_x).max(0) as usize;
        let size_y = (biome_coords::from_block(max.y - 2) + 2 - min_y).max(0) as usize;
        let size_z = (biome_coords::from_block(max.z - 2) + 2 - min_z).max(0) as usize;
        let mut offsets = Vec::with_capacity(size_x * size_y * size_z);
        for corner_x in min_x..min_x + size_x as i32 {
            for corner_z in min_z..min_z + size_z as i32 {
                for corner_y in min_y..min_y + size_y as i32 {
                    offsets.push(fiddle_offsets(seed, corner_x, corner_y, corner_z));
                }
            }
        }
        Self {
            seed,
            min_x,
            min_y,
            min_z,
            size_x,
            size_y,
            size_z,
            offsets: offsets.into_boxed_slice(),
        }
    }

    fn offsets(&self, corner_x: i32, corner_y: i32, corner_z: i32) -> [f64; 3] {
        let local_x = corner_x.wrapping_sub(self.min_x) as usize;
        let local_y = corner_y.wrapping_sub(self.min_y) as usize;
        let local_z = corner_z.wrapping_sub(self.min_z) as usize;
        if local_x < self.size_x && local_y < self.size_y && local_z < self.size_z {
            self.offsets[(local_x * self.size_z + local_z) * self.size_y + local_y]
        } else {
            fiddle_offsets(self.seed, corner_x, corner_y, corner_z)
        }
    }

    /// Same result as [`get_biome_blend`] with this cache's seed.
    #[must_use]
    pub fn get_biome_blend(
        &self,
        bottom_y: i8,
        height: u16,
        x: i32,
        y: i32,
        z: i32,
    ) -> Vector3<i32> {
        blend_with_offsets(bottom_y, height, x, y, z, |corner_x, corner_y, corner_z| {
            self.offsets(corner_x, corner_y, corner_z)
        })
    }
}

#[inline]
fn blend_with_offsets(
    bottom_y: i8,
    height: u16,
    x: i32,
    y: i32,
    z: i32,
    offsets: impl Fn(i32, i32, i32) -> [f64; 3],
) -> Vector3<i32> {
    // This is the "left" side of the biome boundary
    let offset_x = x - 2;
    let offset_y = y - 2;
    let offset_z = z - 2;
    let biome_x = biome_coords::from_block(offset_x);
    let biome_y = biome_coords::from_block(offset_y);
    let biome_z = biome_coords::from_block(offset_z);
    // &'ing 3 gives values of 0-3, it is also the data we removed when converting to biome coords
    // This is effectively "quarters" into the biome
    // Original was "/ 4.0" but we use "* 0.25" as multiplication can be faster
    let quarters_x = (offset_x & 0b11) as f64 * 0.25;
    let quarters_y = (offset_y & 0b11) as f64 * 0.25;
    let quarters_z = (offset_z & 0b11) as f64 * 0.25;

    let mut best_permutation = 0;
    let mut best_score = f64::INFINITY;
    for permutation in 0..8 {
        let should_maintain_x = (permutation & 0b100) == 0;
        let should_maintain_y = (permutation & 0b010) == 0;
        let should_maintain_z = (permutation & 0b001) == 0;

        // If we are shifting, add 1 to the biome coords
        let shifted_biome_x = if should_maintain_x {
            biome_x
        } else {
            biome_x + 1
        };
        let shifted_biome_y = if should_maintain_y {
            biome_y
        } else {
            biome_y + 1
        };
        let shifted_biome_z = if should_maintain_z {
            biome_z
        } else {
            biome_z + 1
        };

        // And reflect the "quarters" across the shift
        let shifted_quarters_x = if should_maintain_x {
            quarters_x
        } else {
            quarters_x - 1.0
        };
        let shifted_quarters_y = if should_maintain_y {
            quarters_y
        } else {
            quarters_y - 1.0
        };
        let shifted_quarters_z = if should_maintain_z {
            quarters_z
        } else {
            quarters_z - 1.0
        };

        let permutation_score = score_offsets(
            offsets(shifted_biome_x, shifted_biome_y, shifted_biome_z),
            shifted_quarters_x,
            shifted_quarters_y,
            shifted_quarters_z,
        );

        if best_score > permutation_score {
            best_score = permutation_score;
            best_permutation = permutation;
        }
    }

    // Now check if we want to use the "left" side or the "right" side
    let biome_x = if (best_permutation & 0b100) == 0 {
        biome_x
    } else {
        biome_x + 1
    };
    let biome_y = if (best_permutation & 0b010) == 0 {
        biome_y
    } else {
        biome_y + 1
    };
    let biome_z = if (best_permutation & 0b001) == 0 {
        biome_z
    } else {
        biome_z + 1
    };

    // Java's `getBiomeForNoiseGen`
    let bottom_y = bottom_y as i32;
    let biome_bottom = biome_coords::from_block(bottom_y);
    let biome_top = biome_bottom + biome_coords::from_block(height as i32) - 1;
    let biome_y = biome_y.clamp(biome_bottom, biome_top);

    Vector3::new(biome_x, biome_y, biome_z)
}

// This is effectively getting a random offset (+/- 0.0-0.8ish) to our biome position quarters and
// returning a hypotenuse squared of the parts + the offset
#[cfg(test)]
const fn score_permutation(
    seed: i64,
    x: i32,
    y: i32,
    z: i32,
    x_part: f64,
    y_part: f64,
    z_part: f64,
) -> f64 {
    score_offsets(fiddle_offsets(seed, x, y, z), x_part, y_part, z_part)
}

/// The random offset vanilla `BiomeManager.getFiddledDistance` adds to a biome corner.
const fn fiddle_offsets(seed: i64, x: i32, y: i32, z: i32) -> [f64; 3] {
    let mix = salt_mix(seed, x as i64);
    let mix = salt_mix(mix, y as i64);
    let mix = salt_mix(mix, z as i64);
    let mix = salt_mix(mix, x as i64);
    let mix = salt_mix(mix, y as i64);
    let mix = salt_mix(mix, z as i64);
    let offset_x = scale_mix(mix);
    let mix = salt_mix(mix, seed);
    let offset_y = scale_mix(mix);
    let mix = salt_mix(mix, seed);
    let offset_z = scale_mix(mix);
    [offset_x, offset_y, offset_z]
}

const fn score_offsets(offsets: [f64; 3], x_part: f64, y_part: f64, z_part: f64) -> f64 {
    let [offset_x, offset_y, offset_z] = offsets;
    square_f64(z_part + offset_z) + square_f64(y_part + offset_y) + square_f64(x_part + offset_x)
}

#[inline]
pub const fn scale_mix(l: i64) -> f64 {
    // Shifting and then masking with 1023 (1024 - 1)
    // This is mathematically identical to floor_mod(l >> 24, 1024)
    // but executes in a single CPU cycle.
    let d = ((l >> 24) & 1023) as f64 / 1024.0;

    (d - 0.5) * 0.9
}

#[inline]
const fn salt_mix(seed: i64, salt: i64) -> i64 {
    let mixed_seed = seed.wrapping_mul(
        seed.wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407),
    );
    mixed_seed.wrapping_add(salt)
}

#[cfg(test)]
mod test {
    use pumpkin_util::{math::vector3::Vector3, read_data_from_file};

    use crate::{
        biome::hash_seed,
        generation::biome::{BiomeFiddleCache, get_biome_blend, scale_mix, score_permutation},
    };

    use super::salt_mix;

    #[test]
    fn mix_seed() {
        let seed = salt_mix(12345678, 12345678);
        assert_eq!(seed, 2937271135939595220);
    }

    #[test]
    fn permutation() {
        let seed = hash_seed(0);
        let score = score_permutation(seed, 123, 456, 456, 0.25, 0.5, 0.75);
        assert_eq!(score, 1.276986312866211);
    }

    #[test]
    fn biome_blend() {
        let biome_pos = get_biome_blend(-64, 384, 1234567890, 123, 123, 123);
        assert_eq!(biome_pos, Vector3::new(31, 30, 30));
    }

    #[test]
    fn fiddle_cache_matches_blend() {
        let seed = hash_seed(13579);
        let cache = BiomeFiddleCache::new(
            seed,
            Vector3::new(-48, -64, 96),
            Vector3::new(-33, 319, 111),
        );
        for x in -48..=-33 {
            for z in 96..=111 {
                for y in -64..=319 {
                    assert_eq!(
                        cache.get_biome_blend(-64, 384, x, y, z),
                        get_biome_blend(-64, 384, seed, x, y, z)
                    );
                }
            }
        }
        // Blocks outside the cached box fall back to hashing their corners directly.
        assert_eq!(
            cache.get_biome_blend(-64, 384, 500, 40, -9),
            get_biome_blend(-64, 384, seed, 500, 40, -9)
        );
    }

    #[test]
    fn scale() {
        let seed = scale_mix(12345678);
        assert_eq!(seed, -0.45);
    }

    #[test]
    fn chunk_wide_blend() {
        let data: Vec<(i32, i32, i32, i32, i32, i32)> =
            read_data_from_file!("../../../../assets/tests/biome_mixer.json");

        let seed = hash_seed((-777i64) as u64);
        for (i, (x, y, z, result_x, result_y, result_z)) in data.into_iter().enumerate() {
            let result = get_biome_blend(i8::MIN, u16::MAX, seed, x, y, z);
            let expected = Vector3::new(result_x, result_y, result_z);
            assert_eq!(
                result, expected,
                "Expected: {expected:?}, was: {result:?} ({i})"
            );
        }
    }
}
