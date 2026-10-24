use pumpkin_util::{
    HeightMap,
    math::{block_box::BlockBox, vector3::Axis},
};

use crate::{
    ProtoChunk,
    generation::structure::{piece::StructurePieceType, structures::StructurePiece},
};

pub const DEFAULT_H_POS: i32 = -1;

/// The averaging step of vanilla `ScatteredFeaturePiece.updateAverageGroundHeight`.
///
/// Returns the mean `MOTION_BLOCKING_NO_LEAVES` height of the footprint columns that lie in
/// `chunk_box`, or `None` when the footprint misses the chunk. Vanilla tests each column at
/// Y 64, which is always inside the chunk box, so only X and Z are compared here.
#[must_use]
pub fn average_ground_height(
    chunk: &ProtoChunk,
    bounding_box: &BlockBox,
    chunk_box: &BlockBox,
) -> Option<i32> {
    let mut total = 0;
    let mut count = 0;
    for z in bounding_box.min.z..=bounding_box.max.z {
        for x in bounding_box.min.x..=bounding_box.max.x {
            if (chunk_box.min.x..=chunk_box.max.x).contains(&x)
                && (chunk_box.min.z..=chunk_box.max.z).contains(&z)
            {
                total += chunk.get_top_y(&HeightMap::MotionBlockingNoLeaves, x, z);
                count += 1;
            }
        }
    }
    (count != 0).then(|| total / count)
}

pub struct ShiftableStructurePiece {
    pub piece: StructurePiece,
    h_pos: i32,
    width: i32,
    height: i32,
}

impl ShiftableStructurePiece {
    #[expect(clippy::too_many_arguments)]
    #[must_use]
    pub fn new(
        r#type: StructurePieceType,
        x: i32,
        y: i32,
        z: i32,
        width: i32,
        height: i32,
        depth: i32,
        axis: Axis,
    ) -> Self {
        Self {
            piece: StructurePiece::new(
                r#type,
                BlockBox::create_box(x, y, z, axis, width, height, depth),
                0,
            ),
            h_pos: DEFAULT_H_POS,
            width,
            height,
        }
    }

    /// Vanilla `ScatteredFeaturePiece.updateAverageGroundHeight`.
    pub fn adjust_to_average_height(
        &mut self,
        chunk: &ProtoChunk,
        chunk_box: &BlockBox,
        offset: i32,
    ) -> bool {
        if self.h_pos >= 0 {
            return true;
        }
        let Some(height) = average_ground_height(chunk, &self.piece.bounding_box, chunk_box) else {
            return false;
        };
        self.h_pos = height;
        let shift_y = height - self.piece.bounding_box.min.y + offset;
        self.piece.bounding_box.move_pos(0, shift_y, 0);
        true
    }

    pub fn adjust_to_min_height(&mut self, chunk: &ProtoChunk, y_offset: i32) -> bool {
        let bounding_box = self.piece.bounding_box;

        if self.h_pos >= 0 {
            return true;
        }

        let mut min_y = i32::MAX;
        let mut found_any = false;

        // Iterate over the X/Z footprint of the bounding box
        for z in bounding_box.min.z..=bounding_box.max.z {
            for x in bounding_box.min.x..=bounding_box.max.x {
                // Vanilla uses MOTION_BLOCKING_NO_LEAVES.
                // In Pumpkin/Standard generation, OceanFloorWg is usually the closest equivalent for terrain height.
                let y = chunk.get_top_y(&HeightMap::OceanFloorWg, x, z);

                if y < min_y {
                    min_y = y;
                }
                found_any = true;
            }
        }

        // Should only return false if the bounding box has no area (width or depth is 0)
        if !found_any {
            return false;
        }

        self.h_pos = min_y;

        // Calculate shift: (Target Height - Current Height) + Offset
        let current_y = bounding_box.min.y;
        let shift_y = (self.h_pos - current_y) + y_offset;

        self.piece.bounding_box.min.y += shift_y;
        self.piece.bounding_box.max.y += shift_y;
        true
    }
}
