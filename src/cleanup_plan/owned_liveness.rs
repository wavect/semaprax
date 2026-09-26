//! Owned-value-across-a-yield primitive, first increment (issue #296, spec
//! section 11.6).
//!
//! [`owned_locals_live_at`] answers one question: given a suspension site (an
//! [`ExpressionId`]), which entries of the function's owned-value cleanup
//! inventory ([`CleanupSlot`] in [`CleanupPlan::slots`]) are still live --
//! initialized, not yet moved, transferred, or finalized -- at that point?
//! The result is exactly the matching subset of `CleanupPlan::slots`, in that
//! vector's own order: it is never re-sorted or re-derived (see AGENTS.md's
//! cleanup-inventory-order invariant).
//!
//! Liveness is derived by replaying the exact [`CleanupTransition`]s the
//! already-built [`CleanupPlan`] carries for the prefix of the function
//! guaranteed to run before `site` -- the same facts
//! [`crate::cleanup_plan::execute`] and the replay validator
//! ([`crate::cleanup_plan::replay`]) consume to decide cleanup -- rather than
//! a fresh move/ownership analysis over the HIR. Locating that prefix reuses
//! only structural HIR traversal (which statement precedes which, and which
//! branch a nested site is under), never ownership judgment of its own.
//!
//! This is deliberately a first increment and is not wired into admission,
//! the resumable envelope, or the interpreter: nothing calls it outside its
//! own tests. Its scope is intentionally narrow:
//!
//! - `site` must be a direct `let`/assignment statement value, or the
//!   function body's own tail, reached only through `if`/`else` branches,
//!   `while` bodies, or plain block-valued slots -- exactly the placements
//!   `resumable_effects::lowering::control` admits for a real `yield`
//!   (SPX-T297's grammar). Anything else is refused.
//! - A statement that precedes the site without containing it must not carry
//!   its own `if`/`else`: joining liveness across branches that both finish
//!   before the site is deferred to a later increment, and is refused rather
//!   than approximated. A `while` statement is exempt because the admitted
//!   profile already requires its body to leave owned liveness unchanged
//!   (`Bounded While-Loops v1`), so replaying its transitions nets to the
//!   state before it regardless of iteration count.
//! - Every place this walk touches must name a whole storage (an empty
//!   [`CleanupPlace::projections`]); partial record-field liveness and
//!   conditional (variant-guarded) owned entries are out of scope and
//!   refused rather than approximated.
//!
//! A site inside a branch that the walk to `site` never enters contributes
//! nothing: its locals are never marked as having run, so they are correctly
//! absent from the result. That is the plan's own semantics falling out of
//! the replay, not a special case -- see
//! `branch_not_taken_excludes_the_untaken_local` below.

use std::collections::BTreeSet;

use crate::diagnostic::Diagnostic;
use crate::hir::{
    self, ExpressionId, ResolvedExpr, ResolvedExprKind, ResolvedFunction, ResolvedStatement,
};

use super::{CleanupPlace, CleanupPlan, CleanupSlotId, CleanupTransition, StorageId};

const CODE: &str = "SPX-H006";

fn refused(message: impl Into<String>) -> Diagnostic {
    Diagnostic::io(CODE, format!("owned_locals_live_at: {}", message.into()))
}

/// The ordered subset of `function.cleanup_plan.slots` still live at `site`.
///
/// See the module docs for the exact scope this first increment covers; a
/// site or a preceding statement outside that scope is `Err`, never silently
/// approximated.
pub(crate) fn owned_locals_live_at(
    function: &ResolvedFunction,
    site: &ExpressionId,
) -> Result<Vec<CleanupSlotId>, Diagnostic> {
    let executed = locate_predecessors(function, site)?;
    let plan = &function.cleanup_plan;
    let mut live = starting_live_set(plan)?;
    for block in &plan.blocks {
        for transition in &block.transitions {
            let Some(at) = transition_trigger(transition) else {
                continue;
            };
            if !executed.contains(at) {
                continue;
            }
            apply_transition(transition, &mut live)?;
        }
    }
    Ok(plan
        .slots
        .iter()
        .filter(|slot| live.contains(&slot.storage))
        .map(|slot| slot.id)
        .collect())
}

fn starting_live_set(plan: &CleanupPlan) -> Result<BTreeSet<StorageId>, Diagnostic> {
    if !plan.entry_state.conditional_owned_parameters.is_empty() {
        return Err(refused(
            "a conditional (variant-guarded) owned parameter is out of scope for this increment",
        ));
    }
    let mut live = BTreeSet::new();
    for place in &plan.entry_state.live_owned_parameters {
        require_whole_place(place)?;
        live.insert(place.storage.clone());
    }
    Ok(live)
}

fn require_whole_place(place: &CleanupPlace) -> Result<(), Diagnostic> {
    if !place.projections.is_empty() {
        return Err(refused(
            "partial record-field liveness is out of scope for this increment",
        ));
    }
    Ok(())
}

/// The expression whose evaluation triggers `transition`, for transitions
/// that can affect whole-storage owned liveness. `SelectFailure` (status
/// result selection) and `StageCopyResult` (Copy-aggregate staging) have no
/// such trigger relevant here: neither moves or initializes owned storage.
fn transition_trigger(transition: &CleanupTransition) -> Option<&ExpressionId> {
    match transition {
        CleanupTransition::ReserveRenewal { at, .. }
        | CleanupTransition::Renew { at, .. }
        | CleanupTransition::Initialize { at, .. }
        | CleanupTransition::InitializeVariant { at, .. }
        | CleanupTransition::Transfer { at, .. }
        | CleanupTransition::TransferVariant { at, .. }
        | CleanupTransition::AuthenticateVariantCase { at, .. } => Some(at),
        CleanupTransition::CallCommit { call, .. } => Some(call),
        CleanupTransition::SelectFailure { .. } | CleanupTransition::StageCopyResult { .. } => None,
    }
}

fn apply_transition(
    transition: &CleanupTransition,
    live: &mut BTreeSet<StorageId>,
) -> Result<(), Diagnostic> {
    match transition {
        CleanupTransition::Initialize { destination, .. }
        | CleanupTransition::InitializeVariant { destination, .. } => {
            require_whole_place(destination)?;
            live.insert(destination.storage.clone());
        }
        CleanupTransition::ReserveRenewal { .. } => {
            // A reservation alone does not publish a value; the `Renew` that
            // follows is what makes the destination live.
        }
        CleanupTransition::Renew {
            source,
            destination,
            ..
        }
        | CleanupTransition::Transfer {
            source,
            destination,
            ..
        }
        | CleanupTransition::TransferVariant {
            source,
            destination,
            ..
        } => {
            require_whole_place(source)?;
            require_whole_place(destination)?;
            live.remove(&source.storage);
            live.insert(destination.storage.clone());
        }
        CleanupTransition::AuthenticateVariantCase { .. } => {
            // Narrows which case of an already-conditional entry is active.
            // Conditional entries are refused by `starting_live_set` and
            // `require_whole_place`, so a well-formed prefix never reaches
            // this arm; treated as a no-op for whole-storage liveness if it
            // ever does.
        }
        CleanupTransition::CallCommit { arguments, .. } => {
            for argument in arguments {
                require_whole_place(&argument.source)?;
                live.remove(&argument.source.storage);
            }
        }
        CleanupTransition::SelectFailure { .. } | CleanupTransition::StageCopyResult { .. } => {}
    }
    Ok(())
}

/// The set of expression identities guaranteed to have finished evaluating
/// strictly before `site`, found by walking `function.body` through exactly
/// the nesting a real `yield` placement admits. `Err` when `site` is not
/// reachable that way, or when a preceding, non-containing statement branches
/// on its own (see the module docs).
fn locate_predecessors(
    function: &ResolvedFunction,
    site: &ExpressionId,
) -> Result<BTreeSet<ExpressionId>, Diagnostic> {
    let mut executed = BTreeSet::new();
    if find_in_block(&function.body, true, site, &mut executed)? {
        Ok(executed)
    } else {
        Err(refused(format!(
            "suspension site `{}` is not a direct let/assignment value or the function body's \
             own tail, reached only through if/else branches, while bodies, or block-valued \
             slots",
            site.as_str()
        )))
    }
}

/// `top` mirrors `resumable_effects::lowering::control::collect_sites`'s own
/// flag: only the outermost function body block's tail is ever an admitted
/// placement, matching SPX-T297 exactly.
fn find_in_block(
    expr: &ResolvedExpr,
    top: bool,
    site: &ExpressionId,
    executed: &mut BTreeSet<ExpressionId>,
) -> Result<bool, Diagnostic> {
    let ResolvedExprKind::Block { statements, tail } = &expr.kind else {
        return Err(refused(
            "expected a block-shaped suspension placement while walking toward the site",
        ));
    };
    for statement in statements {
        match statement {
            ResolvedStatement::Let { value, .. } | ResolvedStatement::Assign { value, .. } => {
                if find_in_value(value, site, executed)? {
                    return Ok(true);
                }
                reject_branch(value)?;
                mark_executed(value, executed);
            }
            ResolvedStatement::While {
                condition, body, ..
            } => {
                if find_in_block(body, false, site, executed)? {
                    return Ok(true);
                }
                mark_executed(condition, executed);
                mark_executed(body, executed);
            }
            ResolvedStatement::Unsafe { body, .. } => {
                reject_branch(body)?;
                mark_executed(body, executed);
            }
        }
    }
    if top && tail.id == *site {
        return Ok(true);
    }
    Ok(false)
}

/// A statement value admits the site directly, or nested through `if`/`else`
/// branches (each of which is itself block-shaped) or a plain nested block.
fn find_in_value(
    expr: &ResolvedExpr,
    site: &ExpressionId,
    executed: &mut BTreeSet<ExpressionId>,
) -> Result<bool, Diagnostic> {
    if expr.id == *site {
        return Ok(true);
    }
    match &expr.kind {
        ResolvedExprKind::If {
            then_branch,
            else_branch,
            ..
        } => {
            if find_in_block(then_branch, false, site, executed)? {
                return Ok(true);
            }
            find_in_block(else_branch, false, site, executed)
        }
        ResolvedExprKind::Block { .. } => find_in_block(expr, false, site, executed),
        _ => Ok(false),
    }
}

/// `Err` when `expr`'s own subtree contains an `if`: a preceding statement
/// that branches on its own would need a join across both arms, which this
/// increment refuses rather than approximates (see the module docs).
fn reject_branch(expr: &ResolvedExpr) -> Result<(), Diagnostic> {
    let mut pending = vec![expr];
    while let Some(current) = pending.pop() {
        if matches!(current.kind, ResolvedExprKind::If { .. }) {
            return Err(refused(
                "a preceding statement branches before the suspension site; joining liveness \
                 across if/else is out of scope for this increment",
            ));
        }
        hir::push_resolved_expression_children_in_authored_order(current, &mut pending);
    }
    Ok(())
}

fn mark_executed(expr: &ResolvedExpr, executed: &mut BTreeSet<ExpressionId>) {
    let mut pending = vec![expr];
    while let Some(current) = pending.pop() {
        executed.insert(current.id.clone());
        hir::push_resolved_expression_children_in_authored_order(current, &mut pending);
    }
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use super::*;
    use crate::hir::ResolvedProgram;
    use crate::parse;

    const SOURCE: &str = r#"
module test.owned_liveness;
@id("bytes.consume")
fn consume(value: own Bytes) -> usize {
    byte_len(bytes_as_slice(value))
}
@id("test.one_local")
fn one_local(data: borrow Slice<u8>) -> usize {
    let a = bytes_copy(data);
    let dummy = 0;
    consume(a)
}
@id("test.moved_before")
fn moved_before(data: borrow Slice<u8>) -> usize {
    let a = bytes_copy(data);
    let consumed = consume(a);
    let dummy = 0;
    consumed
}
@id("test.two_locals")
fn two_locals(data: borrow Slice<u8>) -> usize {
    let a = bytes_copy(data);
    let b = bytes_copy(data);
    let dummy = 0;
    consume(a) + consume(b)
}
@id("test.branch_not_taken")
fn branch_not_taken(data: borrow Slice<u8>, flag: bool) -> usize {
    let chosen = if flag {
        let a = bytes_copy(data);
        let dummy = 0;
        consume(a)
    } else {
        let c = bytes_copy(data);
        consume(c)
    };
    chosen
}
@id("test.no_owned")
fn no_owned() -> i64 {
    let dummy = 0;
    5
}
@id("app.main") fn main() -> i64 { 0 }
"#;

    fn program() -> ResolvedProgram {
        hir::resolve(&parse(SOURCE, Path::new("owned-liveness-fixture.spx")).expect("parses"))
            .expect("resolves")
    }

    fn function<'a>(program: &'a ResolvedProgram, id: &str) -> &'a ResolvedFunction {
        program
            .functions
            .iter()
            .find(|function| function.id.as_str() == id)
            .unwrap_or_else(|| panic!("fixture function `{id}` exists"))
    }

    /// Recursively finds the `let <marker> = ...;` statement's own value
    /// expression id, at any nesting depth. Test-only: unlike the primitive
    /// under test, it does not need to mirror the admitted-placement grammar,
    /// only to locate a fixture's marker unambiguously.
    fn marker_site(expr: &ResolvedExpr, marker: &str) -> Option<ExpressionId> {
        match &expr.kind {
            ResolvedExprKind::Block { statements, tail } => {
                for statement in statements {
                    let (binding, value, nested) = match statement {
                        ResolvedStatement::Let { binding, value, .. }
                        | ResolvedStatement::Assign { binding, value, .. } => {
                            (Some(binding), Some(value), None)
                        }
                        ResolvedStatement::While { body, .. } => (None, None, Some(body.as_ref())),
                        ResolvedStatement::Unsafe { body, .. } => (None, None, Some(body.as_ref())),
                    };
                    if let Some(binding) = binding {
                        if binding.name == marker {
                            return Some(value.expect("let/assign carries a value").id.clone());
                        }
                    }
                    let probe = value.or(nested);
                    if let Some(probe) = probe {
                        if let Some(found) = marker_site(probe, marker) {
                            return Some(found);
                        }
                    }
                }
                marker_site(tail, marker)
            }
            ResolvedExprKind::If {
                then_branch,
                else_branch,
                ..
            } => marker_site(then_branch, marker).or_else(|| marker_site(else_branch, marker)),
            _ => None,
        }
    }

    fn site(function: &ResolvedFunction, marker: &str) -> ExpressionId {
        marker_site(&function.body, marker)
            .unwrap_or_else(|| panic!("fixture marker `{marker}` exists"))
    }

    fn owned_slot_storages(function: &ResolvedFunction, ids: &[CleanupSlotId]) -> Vec<StorageId> {
        ids.iter()
            .map(|id| {
                function
                    .cleanup_plan
                    .slots
                    .iter()
                    .find(|slot| slot.id == *id)
                    .expect("returned id names a real slot")
                    .storage
                    .clone()
            })
            .collect()
    }

    fn binding_storage(function: &ResolvedFunction, name: &str) -> StorageId {
        fn find(expr: &ResolvedExpr, name: &str) -> Option<crate::hir::ValueId> {
            match &expr.kind {
                ResolvedExprKind::Block { statements, tail } => {
                    for statement in statements {
                        if let ResolvedStatement::Let { binding, .. }
                        | ResolvedStatement::Assign { binding, .. } = statement
                        {
                            if binding.name == name {
                                return Some(binding.id.clone());
                            }
                        }
                        let nested = match statement {
                            ResolvedStatement::Let { value, .. }
                            | ResolvedStatement::Assign { value, .. } => Some(value),
                            ResolvedStatement::While { body, .. }
                            | ResolvedStatement::Unsafe { body, .. } => Some(body.as_ref()),
                        };
                        if let Some(nested) = nested {
                            if let Some(found) = find(nested, name) {
                                return Some(found);
                            }
                        }
                    }
                    find(tail, name)
                }
                ResolvedExprKind::If {
                    then_branch,
                    else_branch,
                    ..
                } => find(then_branch, name).or_else(|| find(else_branch, name)),
                _ => None,
            }
        }
        StorageId::Value(find(&function.body, name).unwrap_or_else(|| {
            panic!("fixture binding `{name}` exists");
        }))
    }

    #[test]
    fn owned_local_created_before_the_site_and_used_after_is_live() {
        let program = program();
        let function = function(&program, "test.one_local");
        let site = site(function, "dummy");
        let live = owned_locals_live_at(function, &site).expect("site is admitted");
        let storages = owned_slot_storages(function, &live);
        assert_eq!(storages, vec![binding_storage(function, "a")]);
    }

    #[test]
    fn owned_local_moved_before_the_site_is_not_live() {
        let program = program();
        let function = function(&program, "test.moved_before");
        let site = site(function, "dummy");
        let live = owned_locals_live_at(function, &site).expect("site is admitted");
        let storages = owned_slot_storages(function, &live);
        assert!(
            !storages.contains(&binding_storage(function, "a")),
            "moved local must not be reported live; storages = {storages:?}"
        );
    }

    #[test]
    fn two_owned_locals_are_reported_in_inventory_order() {
        let program = program();
        let function = function(&program, "test.two_locals");
        let site = site(function, "dummy");
        let live = owned_locals_live_at(function, &site).expect("site is admitted");
        let storages = owned_slot_storages(function, &live);
        assert_eq!(
            storages,
            vec![
                binding_storage(function, "a"),
                binding_storage(function, "b"),
            ]
        );
    }

    #[test]
    fn branch_not_taken_excludes_the_untaken_local() {
        let program = program();
        let function = function(&program, "test.branch_not_taken");
        let site = site(function, "dummy");
        let live = owned_locals_live_at(function, &site).expect("site is admitted");
        let storages = owned_slot_storages(function, &live);
        // Documented plan semantics: the site sits inside the `then` branch,
        // so only that branch's own local (`a`) can have run. `c` lives in
        // the `else` branch, structurally unreachable on any path to this
        // site, and is correctly absent -- not because of a special case,
        // but because its `Initialize` transition's trigger expression is
        // never in the "already executed" set the walk to `site` builds.
        assert_eq!(storages, vec![binding_storage(function, "a")]);
    }

    #[test]
    fn function_with_no_owned_locals_is_empty() {
        let program = program();
        let function = function(&program, "test.no_owned");
        let site = site(function, "dummy");
        let live = owned_locals_live_at(function, &site).expect("site is admitted");
        assert!(live.is_empty());
    }

    #[test]
    fn unadmitted_site_is_refused() {
        let program = program();
        // A real `ExpressionId`, but from a different function's body: it
        // does not occur anywhere in `one_local`'s tree, so the walk to it
        // never succeeds.
        let foreign_site = site(function(&program, "test.two_locals"), "dummy");
        let target = function(&program, "test.one_local");
        let error = owned_locals_live_at(target, &foreign_site)
            .expect_err("a site absent from this function's body is refused");
        assert_eq!(error.code, "SPX-H006");
    }
}
