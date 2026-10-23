//! Chunk-column index of a world's non-player entities, so bounding box queries only visit the
//! entities near the box instead of every entity in the world.
//!
//! An item is filed under the column holding the minimum corner of its bounding box. Boxes wider
//! than [`MAX_COLUMN_ITEM_WIDTH`] go to a list every query scans, so a query only has to look
//! that far past its own minimum corner. Moving and removing items happen under the write lock,
//! so a query never sees an item twice or misses one that is mid-move.

use std::sync::atomic::{AtomicU64, Ordering::Relaxed};
use std::sync::{PoisonError, RwLock, RwLockReadGuard, RwLockWriteGuard, Weak};

use pumpkin_util::math::boundingbox::BoundingBox;
use pumpkin_util::math::get_section_cord;
use pumpkin_util::math::vector2::Vector2;
use rustc_hash::FxHashMap;

/// Widest box (along x or z) filed under a column; wider boxes go to the wide list.
const MAX_COLUMN_ITEM_WIDTH: f64 = 4.0;
/// Absorbs float rounding when a query box is widened by [`MAX_COLUMN_ITEM_WIDTH`].
const QUERY_SLACK: f64 = 1.0;
/// Queries spanning more columns than this are left to the caller's full scan.
const MAX_QUERY_COLUMNS: i64 = 64;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum IndexKey {
    Column(Vector2<i32>),
    Wide,
}

impl IndexKey {
    #[must_use]
    pub fn of(aabb: &BoundingBox) -> Self {
        let width_x = aabb.max.x - aabb.min.x;
        let width_z = aabb.max.z - aabb.min.z;
        // Negated so NaN widths land in the wide list too.
        if !(width_x <= MAX_COLUMN_ITEM_WIDTH && width_z <= MAX_COLUMN_ITEM_WIDTH) {
            return Self::Wide;
        }
        Self::Column(Vector2::new(
            get_section_cord(aabb.min.x.floor() as i32),
            get_section_cord(aabb.min.z.floor() as i32),
        ))
    }
}

/// Where an item sits in a [`SectionIndex`]. The item keeps it to move or remove its entry.
pub struct IndexSlot<T> {
    pub index: Weak<SectionIndex<T>>,
    pub key: IndexKey,
    pub seq: u64,
}

struct Buckets<T> {
    columns: FxHashMap<Vector2<i32>, Vec<(u64, T)>>,
    wide: Vec<(u64, T)>,
}

impl<T> Buckets<T> {
    fn bucket_mut(&mut self, key: IndexKey) -> &mut Vec<(u64, T)> {
        match key {
            IndexKey::Column(column) => self.columns.entry(column).or_default(),
            IndexKey::Wide => &mut self.wide,
        }
    }

    fn take(&mut self, key: IndexKey, seq: u64) -> Option<(u64, T)> {
        match key {
            IndexKey::Column(column) => {
                let bucket = self.columns.get_mut(&column)?;
                let index = bucket.iter().position(|(s, _)| *s == seq)?;
                let entry = bucket.swap_remove(index);
                if bucket.is_empty() {
                    self.columns.remove(&column);
                }
                Some(entry)
            }
            IndexKey::Wide => {
                let index = self.wide.iter().position(|(s, _)| *s == seq)?;
                Some(self.wide.swap_remove(index))
            }
        }
    }
}

pub struct SectionIndex<T> {
    buckets: RwLock<Buckets<T>>,
    next_seq: AtomicU64,
}

impl<T> Default for SectionIndex<T> {
    fn default() -> Self {
        Self {
            buckets: RwLock::new(Buckets {
                columns: FxHashMap::default(),
                wide: Vec::new(),
            }),
            next_seq: AtomicU64::new(0),
        }
    }
}

impl<T: Clone> SectionIndex<T> {
    fn read(&self) -> RwLockReadGuard<'_, Buckets<T>> {
        self.buckets.read().unwrap_or_else(PoisonError::into_inner)
    }

    fn write(&self) -> RwLockWriteGuard<'_, Buckets<T>> {
        self.buckets.write().unwrap_or_else(PoisonError::into_inner)
    }

    /// Files `item` under `aabb` and returns its key and insertion sequence number.
    pub fn insert(&self, item: T, aabb: &BoundingBox) -> (IndexKey, u64) {
        let key = IndexKey::of(aabb);
        let seq = self.next_seq.fetch_add(1, Relaxed);
        self.write().bucket_mut(key).push((seq, item));
        (key, seq)
    }

    pub fn relocate(&self, seq: u64, from: IndexKey, to: IndexKey) {
        if from == to {
            return;
        }
        let mut buckets = self.write();
        if let Some(entry) = buckets.take(from, seq) {
            buckets.bucket_mut(to).push(entry);
        }
    }

    pub fn remove(&self, seq: u64, key: IndexKey) -> Option<T> {
        self.write().take(key, seq).map(|(_, item)| item)
    }

    /// Items whose box, read through `aabb_of`, intersects `aabb`, in insertion order.
    ///
    /// Returns `None` when `aabb` spans too many columns for the index to beat a full scan.
    pub fn query(&self, aabb: &BoundingBox, aabb_of: impl Fn(&T) -> BoundingBox) -> Option<Vec<T>> {
        let reach = MAX_COLUMN_ITEM_WIDTH + QUERY_SLACK;
        let min_x = get_section_cord((aabb.min.x - reach).floor() as i32);
        let max_x = get_section_cord(aabb.max.x.floor() as i32);
        let min_z = get_section_cord((aabb.min.z - reach).floor() as i32);
        let max_z = get_section_cord(aabb.max.z.floor() as i32);
        let span_x = (i64::from(max_x) - i64::from(min_x) + 1).max(0);
        let span_z = (i64::from(max_z) - i64::from(min_z) + 1).max(0);
        if span_x * span_z > MAX_QUERY_COLUMNS {
            return None;
        }

        let mut found = Vec::new();
        let mut collect = |bucket: &[(u64, T)]| {
            for (seq, item) in bucket {
                if aabb_of(item).intersects(aabb) {
                    found.push((*seq, item.clone()));
                }
            }
        };
        let buckets = self.read();
        if !buckets.columns.is_empty() {
            for x in min_x..=max_x {
                for z in min_z..=max_z {
                    if let Some(bucket) = buckets.columns.get(&Vector2::new(x, z)) {
                        collect(bucket);
                    }
                }
            }
        }
        collect(&buckets.wide);
        drop(buckets);
        found.sort_unstable_by_key(|(seq, _)| *seq);
        Some(found.into_iter().map(|(_, item)| item).collect())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use pumpkin_util::math::vector3::Vector3;
    use std::sync::Arc;

    fn aabb(x: f64, z: f64, width: f64) -> BoundingBox {
        BoundingBox::new(
            Vector3::new(x, 0.0, z),
            Vector3::new(x + width, 1.0, z + width),
        )
    }

    /// Small xorshift so the test does not depend on a rand version.
    fn next(state: &mut u64) -> f64 {
        *state ^= *state << 13;
        *state ^= *state >> 7;
        *state ^= *state << 17;
        (*state % 1_000_000) as f64 / 1_000_000.0
    }

    #[test]
    fn query_matches_full_scan_across_moves() {
        let mut state = 0x9E37_79B9_7F4A_7C15;
        let index = SectionIndex::<usize>::default();
        let mut boxes = Vec::new();
        let mut slots = Vec::new();
        for id in 0..400 {
            let width = if id % 50 == 0 {
                9.0
            } else {
                0.1 + next(&mut state) * 3.9
            };
            let bb = aabb(
                next(&mut state).mul_add(160.0, -80.0),
                next(&mut state).mul_add(160.0, -80.0),
                width,
            );
            slots.push(index.insert(id, &bb));
            boxes.push(bb);
        }

        for round in 0..200 {
            for id in (round % 7..boxes.len()).step_by(7) {
                let (key, seq) = slots[id];
                let old = boxes[id];
                let bb = aabb(
                    old.min.x + next(&mut state).mul_add(40.0, -20.0),
                    old.min.z + next(&mut state).mul_add(40.0, -20.0),
                    old.max.x - old.min.x,
                );
                let new_key = IndexKey::of(&bb);
                index.relocate(seq, key, new_key);
                slots[id] = (new_key, seq);
                boxes[id] = bb;
            }

            let query = aabb(
                next(&mut state).mul_add(160.0, -80.0),
                next(&mut state).mul_add(160.0, -80.0),
                next(&mut state) * 20.0,
            );
            let expected: Vec<usize> = (0..boxes.len())
                .filter(|id| boxes[*id].intersects(&query))
                .collect();
            let found = index.query(&query, |id| boxes[*id]);
            assert_eq!(found, Some(expected), "round {round}");
        }
    }

    #[test]
    fn boxes_on_column_edges_are_found() {
        let index = SectionIndex::<usize>::default();
        let boxes = [
            aabb(15.999, 15.999, 0.6),
            aabb(-0.3, -0.3, 0.6),
            aabb(-16.0, 31.9, MAX_COLUMN_ITEM_WIDTH),
        ];
        for (id, bb) in boxes.iter().enumerate() {
            index.insert(id, bb);
        }
        for (id, bb) in boxes.iter().enumerate() {
            let probe = aabb(bb.max.x - 0.01, bb.max.z - 0.01, 0.005);
            let found = index.query(&probe, |i| boxes[*i]);
            assert_eq!(found, Some(vec![id]));
        }
    }

    #[test]
    fn removed_items_are_gone_and_huge_queries_fall_back() {
        let index = SectionIndex::<Arc<str>>::default();
        let bb = aabb(1.0, 1.0, 0.6);
        let (key, seq) = index.insert(Arc::from("a"), &bb);
        assert!(index.remove(seq, key).is_some());
        assert_eq!(index.query(&bb, |_| bb), Some(Vec::new()));
        assert!(index.query(&aabb(0.0, 0.0, 1000.0), |_| bb).is_none());
    }
}
