use crate::chunk::ChunkData;
use crate::chunk::io::Dirtiable;
use crate::chunk::palette::BlockPalette;
use crate::level::{Level, SyncChunk};
use crossbeam::queue::SegQueue;
use pumpkin_config::lighting::LightingEngineConfig;
use pumpkin_data::{Block, BlockDirection, BlockState, BlockStateId};
use pumpkin_util::math::position::BlockPos;
use pumpkin_util::math::vector3::Vector3;
use std::sync::Arc;

pub struct DynamicLightEngine {
    min_y: i32,
    max_y: i32,
    block_decrease: SegQueue<(BlockPos, u8)>,
    block_increase: SegQueue<(BlockPos, u8)>,
    sky_decrease: SegQueue<(BlockPos, u8)>,
    sky_increase: SegQueue<(BlockPos, u8)>,
}

impl DynamicLightEngine {
    #[must_use]
    pub const fn new(min_y: i32, max_y: i32) -> Self {
        Self {
            min_y,
            max_y,
            block_decrease: SegQueue::new(),
            block_increase: SegQueue::new(),
            sky_decrease: SegQueue::new(),
            sky_increase: SegQueue::new(),
        }
    }
}

impl DynamicLightEngine {
    /// Checks if there is an open sky above the given position (no opaque blocks blocking sky light).
    fn has_open_sky_above(&self, level: &Arc<Level>, pos: &BlockPos) -> bool {
        let max_y = self.max_y - 1;
        if pos.0.y >= max_y {
            return true;
        }
        let blocks_sky = |state: &BlockState| state.can_occlude() || state.opacity > 0;

        // The whole column is in one chunk, so look it up and lock its sections once
        // instead of once per block. Blocks outside the loaded sections read as void air,
        // like `Level::get_block_state`.
        let (chunk_pos, relative) = pos.chunk_and_chunk_relative_position();
        let void_air = Block::VOID_AIR.default_state;
        level
            .read_chunk_sync(&chunk_pos, |chunk| {
                let sections = chunk
                    .section
                    .block_sections
                    .read()
                    .unwrap_or_else(std::sync::PoisonError::into_inner);
                ((pos.0.y + 1)..=max_y).all(|y| {
                    let relative_y = y - chunk.section.min_y;
                    let state = usize::try_from(relative_y)
                        .ok()
                        .and_then(|relative_y| {
                            sections
                                .get(relative_y / BlockPalette::SIZE)
                                .map(|section| {
                                    section.get(
                                        relative.x as usize,
                                        relative_y % BlockPalette::SIZE,
                                        relative.z as usize,
                                    )
                                })
                        })
                        .map_or(void_air, BlockStateId::to_state);
                    !blocks_sky(state)
                })
            })
            .unwrap_or_else(|| !blocks_sky(void_air))
    }

    /// Handles all lighting updates triggered by a block change (placement/break).
    /// This updates Block Light, Sky Light, and ensures the source block is valid.
    pub fn update_lighting_at(&self, level: &Arc<Level>, pos: BlockPos) {
        // Block Light
        self.check_block_light_updates(level, pos);
        self.perform_block_light_updates(level);

        // Sky Light
        self.check_sky_light_updates(level, pos);
        self.perform_sky_light_updates(level);
    }

    pub fn queue_block_light_decrease(&self, pos: BlockPos, level: u8) {
        self.block_decrease.push((pos, level));
    }

    pub fn queue_block_light_increase(&self, pos: BlockPos, level: u8) {
        self.block_increase.push((pos, level));
    }

    pub fn queue_sky_light_decrease(&self, pos: BlockPos, level: u8) {
        self.sky_decrease.push((pos, level));
    }

    pub fn queue_sky_light_increase(&self, pos: BlockPos, level: u8) {
        self.sky_increase.push((pos, level));
    }

    pub fn perform_block_light_updates(&self, level: &Arc<Level>) -> i32 {
        let mut updates = 0;

        // Keep processing until both queues are empty
        // Light propagation queues new updates, so we need to process until convergence
        loop {
            let decrease_updates = self.perform_block_light_decrease_updates(level);
            let increase_updates = self.perform_block_light_increase_updates(level);

            updates += decrease_updates + increase_updates;

            // Stop when no more updates were processed
            if decrease_updates == 0 && increase_updates == 0 {
                break;
            }
        }

        updates
    }

    fn perform_block_light_decrease_updates(&self, level: &Arc<Level>) -> i32 {
        let mut updates = 0;

        while let Some((pos, expected_light)) = self.block_decrease.pop() {
            self.propagate_block_light_decrease(level, &pos, expected_light);
            updates += 1;
        }

        updates
    }

    fn perform_block_light_increase_updates(&self, level: &Arc<Level>) -> i32 {
        let mut updates = 0;

        while let Some((pos, expected_light)) = self.block_increase.pop() {
            self.propagate_block_light_increase(level, &pos, expected_light);
            updates += 1;
        }

        updates
    }

    fn propagate_block_light_increase(&self, level: &Arc<Level>, pos: &BlockPos, light_level: u8) {
        for dir in BlockDirection::all() {
            let neighbor_pos = pos.offset(dir.to_offset());

            if neighbor_pos.0.y < self.min_y || neighbor_pos.0.y >= self.max_y {
                continue;
            }

            Self::with_chunk(level, &neighbor_pos, |chunk, relative| {
                let Some(neighbor_light) = Self::block_light_in(chunk, relative) else {
                    return;
                };
                let neighbor_state = Self::block_state_in(chunk, relative);
                let opacity = neighbor_state.opacity.max(1);
                let new_light = light_level.saturating_sub(opacity);

                // Only propagate if new light is brighter than current light
                if new_light > neighbor_light {
                    Self::set_block_light_in(chunk, relative, new_light);
                    if new_light > 1 {
                        self.queue_block_light_increase(neighbor_pos, new_light);
                    }
                }
            });
        }
    }

    fn propagate_block_light_decrease(
        &self,
        level: &Arc<Level>,
        pos: &BlockPos,
        removed_light_level: u8,
    ) {
        // Check what the current light level actually is at this position
        let current_level = self.get_block_light_level(level, pos).unwrap_or(0);

        // Only propagate decrease if this position hasn't already been reset to 0
        // This prevents positions that were intentionally set to 0 from propagating light
        if current_level == 0 && removed_light_level > 0 {
            // This position was already darkened, so we propagate the darkness to neighbors
            for dir in BlockDirection::all() {
                let neighbor_pos = pos.offset(dir.to_offset());

                if neighbor_pos.0.y < self.min_y || neighbor_pos.0.y >= self.max_y {
                    continue;
                }

                Self::with_chunk(level, &neighbor_pos, |chunk, relative| {
                    let Some(neighbor_light) = Self::block_light_in(chunk, relative) else {
                        return;
                    };
                    if neighbor_light == 0 {
                        return; // Skip if already 0
                    }

                    let neighbor_state = Self::block_state_in(chunk, relative);
                    let opacity = neighbor_state.opacity.max(1);

                    let expected_from_removed_source = removed_light_level.saturating_sub(opacity);

                    if neighbor_light <= expected_from_removed_source {
                        let neighbor_luminance = neighbor_state.luminance;

                        if neighbor_luminance == 0 {
                            // No self-emission, darken it completely and continue propagation
                            Self::set_block_light_in(chunk, relative, 0);
                            self.queue_block_light_decrease(neighbor_pos, neighbor_light);
                        } else {
                            // Has self-emission, set to its own light and re-propagate from it
                            Self::set_block_light_in(chunk, relative, neighbor_luminance);
                            self.queue_block_light_increase(neighbor_pos, neighbor_luminance);
                        }
                    } else {
                        // This neighbor has brighter light from another source, re-propagate from it
                        self.queue_block_light_increase(neighbor_pos, neighbor_light);
                    }
                });
            }
        }
    }

    pub fn check_block_light_updates(&self, level: &Arc<Level>, pos: BlockPos) {
        match level.lighting_config {
            LightingEngineConfig::Full => {
                self.set_block_light_level(level, &pos, 15).ok();
                return;
            }
            LightingEngineConfig::Dark => {
                self.set_block_light_level(level, &pos, 0).ok();
                return;
            }
            LightingEngineConfig::Default => {}
        }

        let current_light = self.get_block_light_level(level, &pos).unwrap_or(0);
        let block_state = level.get_block_state(&pos).to_state();
        let expected_light = block_state.luminance;

        // Handle light decrease (removing light source or placing opaque block)
        if expected_light < current_light {
            // Set to expected value immediately, then queue decrease to darken neighbors
            self.set_block_light_level(level, &pos, expected_light).ok();
            self.queue_block_light_decrease(pos, current_light);
        } else if expected_light > current_light {
            // Handle light increase (placing light source)
            self.set_block_light_level(level, &pos, expected_light).ok();
            self.queue_block_light_increase(pos, expected_light);
        }

        // Only check neighbors if we didn't trigger a decrease
        // Decrease propagation handles re-validating neighbors
        if expected_light >= current_light {
            self.check_neighbors_light_updates(level, pos, expected_light);
        }
    }

    pub fn check_neighbors_light_updates(
        &self,
        level: &Arc<Level>,
        pos: BlockPos,
        current_light: u8,
    ) {
        for dir in BlockDirection::all() {
            let neighbor_pos = pos.offset(dir.to_offset());

            if neighbor_pos.0.y < self.min_y || neighbor_pos.0.y >= self.max_y {
                continue;
            }
            if let Some(neighbor_light) = self.get_block_light_level(level, &neighbor_pos)
                && neighbor_light > current_light + 1
            {
                self.queue_block_light_increase(neighbor_pos, neighbor_light);
            }
        }
    }

    pub fn perform_sky_light_updates(&self, level: &Arc<Level>) -> i32 {
        let mut updates = 0;
        loop {
            let decrease_updates = self.perform_sky_light_decrease_updates(level);
            let increase_updates = self.perform_sky_light_increase_updates(level);

            updates += decrease_updates + increase_updates;

            if decrease_updates == 0 && increase_updates == 0 {
                break;
            }
        }
        updates
    }

    fn perform_sky_light_decrease_updates(&self, level: &Arc<Level>) -> i32 {
        let mut updates = 0;
        while let Some((pos, expected_light)) = self.sky_decrease.pop() {
            self.propagate_sky_light_decrease(level, &pos, expected_light);
            updates += 1;
        }
        updates
    }

    fn perform_sky_light_increase_updates(&self, level: &Arc<Level>) -> i32 {
        let mut updates = 0;
        while let Some((pos, expected_light)) = self.sky_increase.pop() {
            self.propagate_sky_light_increase(level, &pos, expected_light);
            updates += 1;
        }
        updates
    }

    fn propagate_sky_light_increase(&self, level: &Arc<Level>, pos: &BlockPos, light_level: u8) {
        for dir in BlockDirection::all() {
            let neighbor_pos = pos.offset(dir.to_offset());

            if neighbor_pos.0.y < self.min_y || neighbor_pos.0.y >= self.max_y {
                continue;
            }

            // Never propagate into an unloaded chunk. Writes to an unloaded
            // chunk are dropped silently, so the "brighter than neighbor" check
            // below would stay true forever and keep re-queuing the same
            // position, spinning this loop indefinitely at the border between
            // loaded and unloaded chunks. `with_chunk` skips unloaded chunks.
            Self::with_chunk(level, &neighbor_pos, |chunk, relative| {
                let neighbor_light = Self::sky_light_in(chunk, relative);
                let neighbor_state = Self::block_state_in(chunk, relative);
                let opacity = if neighbor_state.can_occlude() {
                    neighbor_state.opacity.max(1)
                } else {
                    neighbor_state.opacity
                };

                // Calculate new light level for neighbor
                let new_light = if light_level == 15
                    && dir == BlockDirection::Down
                    && opacity == 0
                    && !neighbor_state.can_occlude()
                {
                    // Special case: Sky light at 15 propagates down as 15 through transparent blocks
                    15
                } else {
                    // Normal propagation: reduce by 1 for distance, then by opacity
                    light_level.saturating_sub(1).saturating_sub(opacity)
                };

                // Only propagate if new light is brighter than current light
                if new_light > neighbor_light {
                    Self::set_sky_light_in(chunk, relative, new_light);

                    if new_light > 0 {
                        self.queue_sky_light_increase(neighbor_pos, new_light);
                    }
                }
            });
        }
    }

    fn propagate_sky_light_decrease(&self, level: &Arc<Level>, pos: &BlockPos, removed_light: u8) {
        for dir in BlockDirection::all() {
            let neighbor_pos = pos.offset(dir.to_offset());

            if neighbor_pos.0.y < self.min_y || neighbor_pos.0.y >= self.max_y {
                continue;
            }

            // See `propagate_sky_light_increase`: skip unloaded chunks so sky
            // light updates never spin at loaded/unloaded chunk borders.
            Self::with_chunk(level, &neighbor_pos, |chunk, relative| {
                let neighbor_light = Self::sky_light_in(chunk, relative);
                if neighbor_light == 0 {
                    return; // Already dark
                }

                let neighbor_state = Self::block_state_in(chunk, relative);
                let opacity = if neighbor_state.can_occlude() {
                    neighbor_state.opacity.max(1)
                } else {
                    neighbor_state.opacity
                };

                // Calculate what we would have given this neighbor
                let expected = if removed_light == 15
                    && dir == BlockDirection::Down
                    && opacity == 0
                    && !neighbor_state.can_occlude()
                {
                    15
                } else {
                    removed_light.saturating_sub(1).saturating_sub(opacity)
                };

                if neighbor_light == expected || neighbor_light < removed_light {
                    // This neighbor was lit by us, darken it
                    Self::set_sky_light_in(chunk, relative, 0);
                    self.queue_sky_light_decrease(neighbor_pos, neighbor_light);
                } else if neighbor_light > removed_light {
                    // Neighbor has brighter light from another source
                    // Re-propagate from it to fill in the gap we left
                    self.queue_sky_light_increase(neighbor_pos, neighbor_light);
                }
            });
        }
    }

    pub fn check_sky_light_updates(&self, level: &Arc<Level>, pos: BlockPos) {
        match level.lighting_config {
            LightingEngineConfig::Full => {
                self.set_sky_light_level(level, &pos, 15).ok();
                return;
            }
            LightingEngineConfig::Dark => {
                self.set_sky_light_level(level, &pos, 0).ok();
                return;
            }
            LightingEngineConfig::Default => {}
        }

        let current_light = self.get_sky_light_level(level, &pos);
        let block_state = level.get_block_state(&pos).to_state();
        let opacity = if block_state.can_occlude() {
            block_state.opacity.max(1)
        } else {
            block_state.opacity
        };

        // Calculate expected sky light
        let expected_light = if opacity == 15 || block_state.is_solid_render() {
            // Fully opaque block = no light
            0
        } else {
            // Check if there's open sky above
            let has_sky = self.has_open_sky_above(level, &pos);

            if has_sky {
                // Direct sunlight, reduced by opacity
                15u8.saturating_sub(opacity)
            } else {
                // No direct sky, check neighbors for best light
                let mut best_light = 0;

                for dir in BlockDirection::all() {
                    let neighbor_pos = pos.offset(dir.to_offset());

                    if neighbor_pos.0.y < self.min_y || neighbor_pos.0.y >= self.max_y {
                        continue;
                    }

                    let neighbor_light = self.get_sky_light_level(level, &neighbor_pos);
                    // Calculate potential light from this neighbor
                    let potential = if neighbor_light == 15
                        && dir == BlockDirection::Up
                        && opacity == 0
                        && !block_state.can_occlude()
                    {
                        // Sky light at 15 from above stays 15 through transparent non-occluding blocks
                        15
                    } else {
                        // Normal decay
                        neighbor_light.saturating_sub(1)
                    };

                    best_light = best_light.max(potential);
                }

                // Apply opacity to the best incoming light
                best_light.saturating_sub(opacity)
            }
        };

        // Update if needed
        if expected_light < current_light {
            // Light decreased
            self.set_sky_light_level(level, &pos, expected_light).ok();
            self.queue_sky_light_decrease(pos, current_light);
        } else if expected_light > current_light {
            // Light increased
            self.set_sky_light_level(level, &pos, expected_light).ok();
            self.queue_sky_light_increase(pos, expected_light);
        }

        // Notify neighbors if light increased or stayed same
        if expected_light >= current_light {
            self.check_neighbors_sky_light_updates(pos, expected_light);
        }
    }

    pub fn check_neighbors_sky_light_updates(&self, pos: BlockPos, current_light: u8) {
        // When we update a position, propagate to neighbors
        if current_light > 0 {
            self.queue_sky_light_increase(pos, current_light);
        }
    }

    pub fn get_block_light_level_sync(&self, level: &Level, position: &BlockPos) -> Option<u8> {
        Self::with_chunk(level, position, Self::block_light_in)?
    }

    pub fn get_sky_light_level_sync(&self, level: &Level, position: &BlockPos) -> u8 {
        Self::with_chunk(level, position, Self::sky_light_in).unwrap_or(0)
    }

    pub fn get_block_light_level(&self, level: &Arc<Level>, position: &BlockPos) -> Option<u8> {
        Self::with_chunk(level, position, Self::block_light_in).flatten()
    }

    pub fn set_block_light_level(
        &self,
        level: &Arc<Level>,
        position: &BlockPos,
        light_level: u8,
    ) -> Result<(), String> {
        Self::with_chunk(level, position, |chunk, relative| {
            Self::set_block_light_in(chunk, relative, light_level);
        });
        Ok(())
    }

    pub fn get_sky_light_level(&self, level: &Arc<Level>, position: &BlockPos) -> u8 {
        Self::with_chunk(level, position, Self::sky_light_in).unwrap_or(0)
    }

    pub fn set_sky_light_level(
        &self,
        level: &Arc<Level>,
        position: &BlockPos,
        light_level: u8,
    ) -> Result<(), String> {
        Self::with_chunk(level, position, |chunk, relative| {
            Self::set_sky_light_in(chunk, relative, light_level);
        });
        Ok(())
    }

    /// Runs `f` on the loaded chunk holding `position`, with the position relative to that
    /// chunk. The propagation steps do all their reads and writes of one neighbour inside a
    /// single call, instead of looking the chunk up again for each of them.
    fn with_chunk<R>(
        level: &Level,
        position: &BlockPos,
        f: impl Fn(&SyncChunk, Vector3<i32>) -> R,
    ) -> Option<R> {
        let (chunk_coordinate, relative) = position.chunk_and_chunk_relative_position();
        level.read_chunk_sync(&chunk_coordinate, |chunk| f(chunk, relative))
    }

    /// Like `Level::get_block_state` for a position in `chunk`.
    fn block_state_in(chunk: &ChunkData, relative: Vector3<i32>) -> &'static BlockState {
        chunk
            .section
            .get_block_absolute_y(relative.x as usize, relative.y, relative.z as usize)
            .unwrap_or(Block::VOID_AIR.default_state.id)
            .to_state()
    }

    fn block_light_in(chunk: &SyncChunk, relative: Vector3<i32>) -> Option<u8> {
        let section_idx = (relative.y - chunk.section.min_y) as usize / 16;
        chunk
            .light_engine
            .lock()
            .ok()?
            .block_light
            .get(section_idx)
            .map(|section| {
                section.get(
                    relative.x as usize,
                    (relative.y - chunk.section.min_y) as usize % 16,
                    relative.z as usize,
                )
            })
    }

    fn sky_light_in(chunk: &SyncChunk, relative: Vector3<i32>) -> u8 {
        let section_index = (relative.y - chunk.section.min_y) as usize / BlockPalette::SIZE;

        let light_engine = chunk
            .light_engine
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        // Bounds check for section index (lock the light engine)
        if section_index >= light_engine.sky_light.len() {
            return 15;
        }

        light_engine.sky_light[section_index].get(
            relative.x as usize,
            (relative.y - chunk.section.min_y) as usize % BlockPalette::SIZE,
            relative.z as usize,
        )
    }

    fn set_block_light_in(chunk: &ChunkData, relative: Vector3<i32>, light_level: u8) {
        let section_index = (relative.y - chunk.section.min_y) as usize / BlockPalette::SIZE;
        {
            let mut light_engine = chunk
                .light_engine
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            if section_index >= light_engine.block_light.len() {
                return;
            }
            let relative_y = (relative.y - chunk.section.min_y) as usize % BlockPalette::SIZE;
            light_engine.block_light[section_index].set(
                relative.x as usize,
                relative_y,
                relative.z as usize,
                light_level,
            );
        };
        // Mark chunk as dirty so lighting changes are saved to disk
        if !chunk.is_dirty() {
            chunk.mark_dirty(true);
        }
    }

    fn set_sky_light_in(chunk: &ChunkData, relative: Vector3<i32>, light_level: u8) {
        let section_index = (relative.y - chunk.section.min_y) as usize / BlockPalette::SIZE;
        {
            let mut light_engine = chunk
                .light_engine
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            if section_index >= light_engine.sky_light.len() {
                return;
            }
            let relative_y = (relative.y - chunk.section.min_y) as usize % BlockPalette::SIZE;
            light_engine.sky_light[section_index].set(
                relative.x as usize,
                relative_y,
                relative.z as usize,
                light_level,
            );
        };
        // Mark chunk as dirty so lighting changes are saved to disk
        if !chunk.is_dirty() {
            chunk.mark_dirty(true);
        }
    }
}
