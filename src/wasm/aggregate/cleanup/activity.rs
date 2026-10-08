//! Runtime ownership references are distinct from structural slot inventory.
use crate::cleanup_plan::{CleanupPlan, CleanupTransition, StorageId};
use std::collections::BTreeSet;

pub(in crate::wasm::aggregate) fn runtime_storages(plan: &CleanupPlan) -> BTreeSet<StorageId> {
    let mut active = BTreeSet::new();
    for place in &plan.entry_state.live_owned_parameters {
        active.insert(place.storage.clone());
    }
    for entry in &plan.entry_state.conditional_owned_parameters {
        active.insert(entry.storage.clone());
    }
    for transition in plan.blocks.iter().flat_map(|block| &block.transitions) {
        match transition {
            CleanupTransition::Initialize { destination, .. }
            | CleanupTransition::InitializeVariant { destination, .. } => {
                active.insert(destination.storage.clone());
            }
            CleanupTransition::Transfer {
                source,
                destination,
                ..
            }
            | CleanupTransition::Renew {
                source,
                destination,
                ..
            }
            | CleanupTransition::TransferVariant {
                source,
                destination,
                ..
            } => {
                active.insert(source.storage.clone());
                active.insert(destination.storage.clone());
            }
            CleanupTransition::ReserveRenewal { binding, .. } => {
                active.insert(binding.storage.clone());
            }
            CleanupTransition::CallCommit { arguments, .. } => {
                active.extend(
                    arguments
                        .iter()
                        .map(|argument| argument.source.storage.clone()),
                );
            }
            CleanupTransition::AuthenticateVariantCase { .. }
            | CleanupTransition::SelectFailure { .. }
            | CleanupTransition::StageCopyResult { .. } => {}
        }
    }
    for action in plan.exits.iter().flat_map(|exit| &exit.finalize_in_order) {
        active.insert(action.source.storage.clone());
    }
    active
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn runtime_census_distinguishes_named_reads_from_real_string_temporary_siblings() {
        let source = "module test.wasm_runtime_anchors; @id(\"read\") fn read(text:string)->i64{let mut i=0; while i<1 && string_len(text)>0 && string_len(\"x\")>0{i=i+1;0} i} @id(\"main\") fn main()->i64{0}";
        let parsed =
            crate::parse(source, std::path::Path::new("wasm-runtime-anchors.spx")).unwrap();
        let resolved = crate::hir::resolve(&parsed).unwrap();
        crate::hir::validate(&resolved).unwrap();
        let function = resolved
            .functions
            .iter()
            .find(|f| f.id.as_str() == "read")
            .unwrap();
        let active = runtime_storages(&function.cleanup_plan);
        let mut strings = function
            .cleanup_plan
            .slots
            .iter()
            .filter(|slot| {
                slot.ty == crate::hir::ResolvedType::String
                    && matches!(slot.storage, StorageId::Temporary(_))
            })
            .map(|slot| active.contains(&slot.storage))
            .collect::<Vec<_>>();
        strings.sort();
        assert_eq!(
            strings,
            [false, true],
            "the named read stays dormant beside the real literal"
        );
    }
}
