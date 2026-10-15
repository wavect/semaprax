//! Constructor-local payment for the validator's owning identity B-trees.
//! Shared immutable identities clone without a payload request. The admitted
//! B-tree node upper is a conservative acquisition, not observed node bytes.
use super::super::*;
pub(in crate::hir::validation) fn insert<K: Ord + Clone>(
    values: &mut BTreeSet<K>,
    id: &K,
) -> Result<bool, Diagnostic> {
    // Preserve the original duplicate diagnostic priority and avoid a constructor.
    if values.contains(id) {
        return Ok(false);
    }
    scope_checkpoint::charge::reserve(crate::cleanup_plan::replay_btree_node_upper::<K, ()>())?;
    Ok(values.insert(id.clone()))
}
pub(super) fn expression(
    values: &mut BTreeSet<ExpressionId>,
    id: &ExpressionId,
) -> Result<bool, Diagnostic> {
    insert(values, id)
}
pub(super) fn value(values: &mut BTreeSet<ValueId>, id: &ValueId) -> Result<bool, Diagnostic> {
    insert(values, id)
}
