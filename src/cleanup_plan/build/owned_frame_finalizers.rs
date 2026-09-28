use crate::cleanup::LivenessFlagId;
use crate::cleanup_plan::{CleanupPlace, FinalizeAction, VariantCaseGuard};
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

/// The ordinary builder's conditional append order, shared without sorting
/// case or flag inventory. The caller supplies variants in reverse live order.
pub(crate) fn canonical_conditional_finalizers_for(
    root: &CleanupPlace,
    variant: &DeclarationId,
    cases: &[(DeclarationId, Vec<LivenessFlagId>)],
    metadata: impl Fn(LivenessFlagId) -> (CleanupPlace, DeclarationId),
    included: impl Fn(&CleanupPlace) -> bool,
) -> Vec<FinalizeAction> {
    let mut actions = Vec::new();
    for (case, flags) in cases.iter().rev() {
        for flag in flags.iter().rev() {
            let (place, lifecycle) = metadata(*flag);
            if included(&place) {
                actions.push(FinalizeAction {
                    source: place,
                    lifecycle_id: lifecycle,
                    guard_flag: *flag,
                    active_case: Some(VariantCaseGuard {
                        storage: root.storage.clone(),
                        variant: variant.clone(),
                        case: case.clone(),
                    }),
                });
            }
        }
    }
    actions
}
