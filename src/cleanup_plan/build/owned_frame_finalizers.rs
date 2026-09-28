use crate::cleanup::LivenessFlagId;
use crate::cleanup_plan::{CleanupPlace, FinalizeAction};
use crate::hir::DeclarationId;

/// Canonical ordered leaf cleanup shared by ordinary plans and suspension proofs.
/// `live_order` is the compiler's initialized flag order, never sorted here.
pub(crate) fn canonical_finalizers_for(
    live_order: &[LivenessFlagId],
    metadata: impl Fn(LivenessFlagId) -> (CleanupPlace, DeclarationId),
    included: impl Fn(&CleanupPlace) -> bool,
) -> Vec<FinalizeAction> {
    live_order
        .iter()
        .rev()
        .filter_map(|flag| {
            let (place, lifecycle) = metadata(*flag);
            included(&place).then(|| FinalizeAction {
                source: place,
                lifecycle_id: lifecycle,
                guard_flag: *flag,
                active_case: None,
            })
        })
        .collect()
}
