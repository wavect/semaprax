//! Last-use release before either independent typed-control comparison.
use super::{case_proof::Attempt, StatusCases, StatusSources};
use crate::cleanup_plan::replay::{expression_facts::Facts, leaf_index::Leaves, ReplayBudget};
use crate::cleanup_plan::StorageId;
use crate::diagnostic::Diagnostic;
use crate::hir::ResolvedFunction;
use std::collections::BTreeSet;
use std::mem::size_of;

// The tuple owns the actual five moved values. Tuple fields drop in the same
// reverse-declaration order as the old lexical exit. Every referenced Program,
// Function, helper and continuation owner remains outside this scratch tuple.
type Scratch<'hir> = (
    StatusSources<'hir>,
    Leaves,
    BTreeSet<StorageId>,
    StatusCases<'hir>,
    Facts<'hir>,
);
const PAYMENT: usize = 2 * size_of::<(&mut ReplayBudget, &ResolvedFunction, usize, &str)>()
    + 2 * size_of::<Result<(), Diagnostic>>();
// Initializer and drop argument/formal are separate moved packet owners; none
// of the earlier constructors' backing or receiver payments is credited here.
pub(super) const RELEASE: usize = 2 * size_of::<Scratch<'static>>() + PAYMENT;
pub(super) const ATTEMPT: usize = 2 * size_of::<Attempt<'static, 'static>>() + PAYMENT;

macro_rules! finish_structure {
    ($program:ident, $function:ident, $budget:ident, $helpers:ident, $continuations:ident;
     $statuses:ident, $leaves:ident, $storage:ident, $cases:ident, $facts:ident, $attempt:ident) => {{
        $budget.reserve_bytes($function, super::last_use::ATTEMPT,
            "completed status attempt release transport")?;
        // complete() already fixed the affine token's successful state. Drop
        // neither queries a payer nor changes the selected diagnostic here.
        drop($attempt);
        finish_structure!($program, $function, $budget, $helpers, $continuations;
            $statuses, $leaves, $storage, $cases, $facts)
    }};
    ($program:ident, $function:ident, $budget:ident, $helpers:ident, $continuations:ident;
     $statuses:ident, $leaves:ident, $storage:ident, $cases:ident, $facts:ident) => {{
        $budget.reserve_bytes($function, super::last_use::RELEASE,
            "ordinary structure scratch release transport")?;
        drop(($statuses, $leaves, $storage, $cases, $facts));
        validate_typed_control_skeleton_with_index(
            $program, $function, $budget, $helpers, $continuations)
    }};
}

#[cfg(test)]
mod tests;
