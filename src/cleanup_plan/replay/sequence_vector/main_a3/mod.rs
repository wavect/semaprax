//! Complete independent Maina3 vector helper and sequencing receiver.
use super::*;
use crate::cleanup_plan::replay::path_array_main8a as path_array;
use crate::cleanup_plan::replay::sequence_prefix_main8a::helper as sequence_prefix;
use current::sequence_skeleton_paths;
use helper as sequence_vector;
#[path = "current.rs"]
mod current;
#[path = "helper.rs"]
mod helper;

thread_local! { static ACTIVE: std::cell::Cell<bool> = const { std::cell::Cell::new(false) }; }
pub(in crate::cleanup_plan::replay) fn selected() -> bool {
    ACTIVE.with(std::cell::Cell::get)
}
pub(in crate::cleanup_plan::replay) use current::sequence_skeleton_paths as current_sequence;
// Namespace-only whole-HIR adapter: every reached sequence operation is the
// complete Maina3 receiver, including its original flag-controlled old arm.
fn hir_skeleton_paths<'hir>(
    program: &'hir dyn crate::cleanup::ReplayContext,
    function: &'hir ResolvedFunction,
    budget: &mut ReplayBudget,
) -> Result<Vec<SkeletonPath>, Diagnostic> {
    struct Restore(bool);
    impl Drop for Restore {
        fn drop(&mut self) {
            ACTIVE.with(|slot| slot.set(self.0));
        }
    }
    let _restore = Restore(ACTIVE.with(|slot| slot.replace(true)));
    crate::cleanup_plan::replay::hir_skeleton_paths(program, function, budget)
}

#[path = "path_array_source.rs"]
mod path_array_source;
