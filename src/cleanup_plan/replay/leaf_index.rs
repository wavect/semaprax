//! Droppable leaves indexed by storage.
//!
//! `validate_place` resolves the leaves under one place for every transition
//! and every exit finalizer. Scanning every leaf of the function made exit
//! validation cubic in the number of owned values (exits times finalizers
//! times leaves); the storage index keeps each lookup to that storage's own
//! leaves, still in liveness-flag order.
use super::*;

pub(super) struct Leaves {
    by_flag: BTreeMap<LivenessFlagId, Leaf>,
    by_storage: BTreeMap<StorageId, Vec<LivenessFlagId>>,
}

impl Leaves {
    pub(super) fn new(by_flag: BTreeMap<LivenessFlagId, Leaf>) -> Self {
        let mut by_storage = BTreeMap::<StorageId, Vec<LivenessFlagId>>::new();
        for (flag, leaf) in &by_flag {
            by_storage
                .entry(leaf.place.storage.clone())
                .or_default()
                .push(*flag);
        }
        Self {
            by_flag,
            by_storage,
        }
    }

    /// The leaves at or below `place`, in liveness-flag order.
    pub(super) fn under(&self, place: &CleanupPlace) -> Vec<LivenessFlagId> {
        self.by_storage
            .get(&place.storage)
            .into_iter()
            .flatten()
            .filter(|flag| {
                self.by_flag[*flag]
                    .place
                    .projections
                    .starts_with(&place.projections)
            })
            .copied()
            .collect()
    }
}

impl std::ops::Deref for Leaves {
    type Target = BTreeMap<LivenessFlagId, Leaf>;

    fn deref(&self) -> &Self::Target {
        &self.by_flag
    }
}
