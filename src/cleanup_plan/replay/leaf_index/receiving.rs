//! Same original ordered leaf backing, with sealed per-call budget receiving.
use super::{BTreeMap, Leaf, Leaves, LivenessFlagId, ReplayBudget, ResolvedFunction, StorageId};
use crate::cleanup_plan::replay::selected_context::child::receiving::receive;
use crate::cleanup_plan::replay::selected_context::child::{
    CloneReceiving, NodeReceiving, Permit, VecReceiving,
};
type Error = crate::cleanup_plan::replay::expression_facts::errors::EngineError;
impl Leaves {
    pub(in crate::cleanup_plan::replay) fn new_with_receiving(
        function: &ResolvedFunction,
        by_flag: BTreeMap<LivenessFlagId, Leaf>,
        budget: &mut ReplayBudget,
    ) -> Result<Self, Error> {
        let permit = Permit::profile::<Self>(function, budget)?;
        let mut by_storage = BTreeMap::<StorageId, Vec<LivenessFlagId>>::new();
        for (flag, leaf) in &by_flag {
            if !by_storage.contains_key(&leaf.place.storage) {
                receive!(
                    function,
                    budget,
                    permit,
                    NodeReceiving<'_>,
                    8,
                    budget.child_reserve_btree_node::<StorageId, Vec<LivenessFlagId>>(
                        function,
                        "leaf storage index",
                    )
                );
                receive!(
                    function,
                    budget,
                    permit,
                    CloneReceiving<'_, StorageId>,
                    9,
                    budget.child_reserve_clone(function, &leaf.place.storage, "leaf storage key")
                );
                by_storage.insert(leaf.place.storage.clone(), Vec::new());
            }
            let flags = by_storage
                .get_mut(&leaf.place.storage)
                .expect("storage key inserted");
            receive!(
                function,
                budget,
                permit,
                VecReceiving<'_, LivenessFlagId>,
                10,
                budget.child_reserve_vec(function, flags, 1, "leaf storage flag vector")
            );
            flags.push(*flag);
        }
        Ok(Self {
            by_flag,
            by_storage,
        })
    }
}
