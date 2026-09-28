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
//! The branching-predecessor refusal above only ever fires for a function
//! whose cleanup plan carries at least one owned slot: `owned_locals_live_at`
//! returns an empty result immediately, without walking toward `site` at all,
//! when [`CleanupPlan::slots`] is empty (issue #296, bug #296 in R20). A
//! purely scalar control-dependent function -- every `carried_locals_at` call
//! sees, whether or not it ever carries an owned local -- is never refused
//! merely because an earlier top-level statement happens to branch: there is
//! nothing for a join across that branch to lose track of.
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
    ResolvedType, ValueId,
};

use super::{CleanupPlace, CleanupPlan, CleanupSlotId, CleanupTransition, StorageId};
use crate::cleanup::FieldLivenessShape;

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
    // A function whose cleanup plan carries no owned slot at all has nothing
    // for any join to join: the result is vacuously empty regardless of how
    // `site` is reached, so a purely scalar control-dependent function (an
    // `if`/`else` preceding the site, say) is never refused merely for
    // branching on its own. `admit_owned_bytes_profile` already short-circuits
    // this same way before ever calling this function for its whole-function
    // admission; `carried_locals_at` (issue #296 R20, bug #296) is the second,
    // narrower caller this increment's own scope refusal must not reach when
    // there is no owned local to lose track of. A function with a non-empty
    // plan but nothing live at this particular `site` still goes through the
    // ordinary replay below and can still be refused if the walk to `site`
    // itself is out of the admitted grammar.
    if function.cleanup_plan.slots.is_empty() {
        return Ok(Vec::new());
    }
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

/// Every `yield` expression's identity anywhere in `function`, in no
/// particular order: a plain structural walk, not the admitted-placement
/// grammar `locate_predecessors` enforces. `parser::yields`/`SPX-T297` has
/// already refused every other placement, so this is exhaustive over the
/// sites a real suspension can occur at.
pub(crate) fn direct_yield_sites(function: &ResolvedFunction) -> Vec<ExpressionId> {
    let mut sites = Vec::new();
    let mut pending = vec![&function.body];
    while let Some(expr) = pending.pop() {
        if matches!(expr.kind, ResolvedExprKind::Yield { .. }) {
            sites.push(expr.id.clone());
        }
        hir::push_resolved_expression_children_in_authored_order(expr, &mut pending);
    }
    sites
}

/// Second increment (issue #296, spec section 11.6): the whole-function
/// admission check that lets an owned `Bytes` local -- and only that type,
/// only a whole `let`-bound storage, never a temporary, call-argument, or
/// partially live field -- leave the Copy-scalar profile that
/// `hir::resolve_yield::check_scalar` otherwise enforces unconditionally.
///
/// `check_scalar` already defers exactly one case: an unborrowed value of
/// type `Bytes`. Every other owned or aggregate value it still refuses
/// outright with `SPX-T303` (and every borrow/resource keeps `SPX-T305`/
/// `SPX-T306`), so by the time this runs -- after `function.cleanup_plan` is
/// built, which `check_scalar` itself runs before -- every slot in the
/// built plan is already known to be an unborrowed `Bytes` value. This
/// function is the second, narrower half of that admission: for every slot
/// [`owned_locals_live_at`] reports live at some real suspension site (a
/// value that must survive a suspension as a live local, not merely exist
/// somewhere in the function), it proves that slot is a whole `let`-bound
/// storage, and refuses the function (`SPX-T303`) otherwise. A slot no site
/// ever reports live -- a call's own transient provisional-result or staged
/// call-argument storage, or a genuinely dead local -- resolves entirely
/// within one non-suspended segment and needs no such proof; a site the
/// query itself refuses (a preceding statement branching on its own, say)
/// still refuses the whole function, since a slot it would have reported is
/// then unaccounted for either way.
pub(crate) fn admit_owned_bytes_profile(function: &ResolvedFunction) -> Result<(), Diagnostic> {
    let plan = &function.cleanup_plan;
    if plan.slots.is_empty() {
        return Ok(());
    }
    // The cleanup plan's own inventory is not only "the function's named
    // owned locals": a call's provisional result, or an argument staged for
    // an atomic commit, is its own slot too, transient storage that never
    // itself needs to survive a suspension even when the value it carries
    // does (a whole-storage `Transfer`/`Initialize` moves the live value on
    // to its named binding's own slot before any later site, and this
    // increment's admission is only ever about a value that must survive
    // *as* a suspended local). This increment's admission therefore checks
    // only the slots [`owned_locals_live_at`] actually reports live at some
    // real site; every other slot resolves entirely within one non-suspended
    // segment and is this admission's concern only in that it must still be
    // provable one way or the other -- a site the query itself refuses (a
    // preceding statement branching on its own, say) still refuses the whole
    // function, exactly as a value the query never reaches alive would.
    let sites = direct_yield_sites(function);
    let mut live_anywhere: BTreeSet<CleanupSlotId> = BTreeSet::new();
    for site in &sites {
        let live = owned_locals_live_at(function, site).map_err(|_| {
            not_admitted(format!(
                "function `{}` carries an owned value whose liveness this increment's narrower \
                 query cannot prove at every suspension site",
                function.name
            ))
        })?;
        // Issue #296, spec section 11.6: the interpreter's carrying
        // substitution (`interpreter::resumable::Resumption::Replay::carried`)
        // is a flat map keyed by the static `let` binding, consumed on the
        // *first* dynamic occurrence it is asked to substitute during one
        // resume's replay. A site reached through a `while` body can
        // suspend more than once per invocation (the loop's own bound is
        // `MAX_CONTROL_SUSPENSIONS`, not one), but that substitution is sound
        // exactly when the carried local's own storage is never touched
        // (defined, renewed, or transferred) *inside* the `while` body: such
        // a local's own binding then reaches exactly one dynamic occurrence
        // per invocation regardless of how many times the loop-embedded site
        // itself suspends, so its one recorded value is the right one for
        // every occurrence. A local whose own storage *is* touched inside
        // the loop can legitimately hold a different value at each dynamic
        // occurrence of its own binding, which the flat map cannot represent
        // (only the first would ever be consulted): that shape stays
        // refused. `slot_touched_inside_while` decides this per live slot,
        // rather than blanket-refusing every live slot merely because the
        // *site* sits inside a `while` body.
        if !live.is_empty() && site_is_loop_embedded(function, site) {
            for slot_id in &live {
                let Some(slot) = plan.slots.iter().find(|slot| slot.id == *slot_id) else {
                    continue;
                };
                if slot_touched_inside_while(function, &slot.storage) {
                    return Err(not_admitted(format!(
                        "function `{}` carries an owned `Bytes` value defined or reassigned \
                         inside a `while` loop body, live across a suspension site reached \
                         through that same loop; only a value defined -- and never reassigned \
                         -- outside every enclosing loop admits a carried value in this \
                         increment",
                        function.name
                    )));
                }
            }
        }
        live_anywhere.extend(live);
    }
    for slot in &plan.slots {
        if !live_anywhere.contains(&slot.id) {
            continue;
        }
        if slot.ty != ResolvedType::Bytes {
            return Err(not_admitted(format!(
                "function `{}` carries a non-`Bytes` owned value live across a suspension site, \
                 which this increment does not admit",
                function.name
            )));
        }
        if !matches!(slot.storage, StorageId::Value(_)) {
            return Err(not_admitted(format!(
                "function `{}` carries an owned `Bytes` value that is not a whole `let`-bound \
                 local live across a suspension site",
                function.name
            )));
        }
        if !matches!(slot.field_liveness_shape, FieldLivenessShape::Leaf { .. }) {
            return Err(not_admitted(format!(
                "function `{}` carries an owned `Bytes` value with a non-leaf field liveness \
                 shape live across a suspension site",
                function.name
            )));
        }
    }
    Ok(())
}

/// Admission for the direct sequential aggregate-`Bytes` channel.  The
/// request expression itself is evaluated after this prefix query, so its
/// temporary owned leaf is deliberately absent. Any named owned local already
/// live before a yield would need durable carrying, which this lane does not
/// implement.
pub(crate) fn admit_sequential_aggregate_bytes_profile(
    function: &ResolvedFunction,
) -> Result<(), Diagnostic> {
    for site in direct_yield_sites(function) {
        let live = owned_locals_live_at(function, &site).map_err(|_| {
            not_admitted(format!(
                "function `{}` has owned liveness the sequential aggregate-Bytes channel cannot prove before a suspension",
                function.name
            ))
        })?;
        if !live.is_empty() {
            return Err(not_admitted(format!(
                "function `{}` has an owned local live before a sequential aggregate-Bytes yield; only an inline request is admitted",
                function.name
            )));
        }
    }
    Ok(())
}

/// The stable refusal an owned `Bytes` local that this increment does not
/// admit keeps: the same `SPX-T303` `hir::resolve_yield::check_scalar`
/// would have raised immediately, had it not deferred exactly this one type
/// to be decided here, once the cleanup plan exists. Distinct from this
/// module's own `SPX-H006`, which stays reserved for
/// [`owned_locals_live_at`]'s own scope-limitation refusals.
fn not_admitted(message: impl Into<String>) -> Diagnostic {
    Diagnostic::io("SPX-T303", message)
}

/// True when `site` is reached only by passing through at least one `while`
/// loop's body or condition (a loop-embedded site). Unlike
/// `locate_predecessors`'s own admitted-placement grammar, this walk is
/// exhaustive over every expression and statement kind: `site` can be a
/// transition's trigger expression anywhere in the function, not only a
/// direct `let`/assignment value or tail reached through `if`/`else`/`while`/
/// block-valued nesting. `false` when `site` sits entirely outside any
/// `while` body/condition; also `false` (fail toward the caller's own
/// separate refusal, never silently toward "safe") when `site` is not found
/// at all, which does not occur for a trigger expression genuinely present
/// in `function.body`.
fn site_is_loop_embedded(function: &ResolvedFunction, site: &ExpressionId) -> bool {
    // `(expression, already inside a while)`. Every statement kind
    // (`Let`/`Assign`/`Unsafe`/`While`) and every expression kind reachable
    // from `function.body` is covered: `While`'s own two children (condition,
    // then body, per `ResolvedStatement::child_count`/`child`) are pushed
    // `in_while = true`; every other statement's children, and every
    // expression's own children via
    // `hir::push_resolved_expression_children_in_authored_order` (an
    // exhaustive match over `ResolvedExprKind`, so a new expression kind
    // fails this to compile rather than silently skipping it), are pushed
    // with the enclosing `in_while` unchanged.
    let mut pending: Vec<(&ResolvedExpr, bool)> = vec![(&function.body, false)];
    while let Some((expr, in_while)) = pending.pop() {
        if expr.id == *site {
            return in_while;
        }
        if let ResolvedExprKind::Block { statements, tail } = &expr.kind {
            pending.push((tail, in_while));
            for statement in statements.iter().rev() {
                let statement_in_while =
                    in_while || matches!(statement, ResolvedStatement::While { .. });
                for index in (0..statement.child_count()).rev() {
                    if let Some(child) = statement.child(index) {
                        pending.push((child, statement_in_while));
                    }
                }
            }
        } else {
            let mut children = Vec::new();
            hir::push_resolved_expression_children_in_authored_order(expr, &mut children);
            pending.extend(children.into_iter().map(|child| (child, in_while)));
        }
    }
    false
}

/// True when some transition touching `storage` -- as its `Initialize`/
/// `InitializeVariant` destination, its `Renew`/`Transfer`/`TransferVariant`
/// source or destination, its `ReserveRenewal` reservation, or a
/// `CallCommit` argument it stages -- has a trigger expression reached only
/// through a `while` body or condition (`site_is_loop_embedded`). This is
/// the admission question for a loop-embedded carried site: a slot untouched
/// inside every `while` body of the function is defined (and, if ever
/// reassigned, only ever reassigned) *outside* any loop, so its own
/// `let`/assignment reaches exactly one dynamic occurrence per invocation
/// regardless of how many times a loop-embedded site downstream suspends --
/// the shape `interpreter::resumable`'s carrying substitution proves sound.
/// A slot touched inside a `while` body is the genuinely unsound shape this
/// increment still refuses: reused rather than approximated.
pub(crate) fn slot_touched_inside_while(function: &ResolvedFunction, storage: &StorageId) -> bool {
    function
        .cleanup_plan
        .blocks
        .iter()
        .flat_map(|block| &block.transitions)
        .filter_map(|transition| transition_touch(transition, storage))
        .any(|at| site_is_loop_embedded(function, at))
}

/// The trigger expression of `transition` when it touches `storage` as a
/// source, destination, reservation, or committed call argument; `None` when
/// it does not touch `storage` at all, or (`AuthenticateVariantCase`) only
/// narrows an already-conditional entry that whole-storage admission never
/// reaches.
fn transition_touch<'t>(
    transition: &'t CleanupTransition,
    storage: &StorageId,
) -> Option<&'t ExpressionId> {
    match transition {
        CleanupTransition::ReserveRenewal { at, binding } => {
            (binding.storage == *storage).then_some(at)
        }
        CleanupTransition::Initialize { at, destination }
        | CleanupTransition::InitializeVariant {
            at, destination, ..
        } => (destination.storage == *storage).then_some(at),
        CleanupTransition::Renew {
            at,
            source,
            destination,
        }
        | CleanupTransition::Transfer {
            at,
            source,
            destination,
        }
        | CleanupTransition::TransferVariant {
            at,
            source,
            destination,
            ..
        } => (source.storage == *storage || destination.storage == *storage).then_some(at),
        CleanupTransition::AuthenticateVariantCase { .. } => None,
        CleanupTransition::CallCommit { call, arguments } => arguments
            .iter()
            .any(|argument| argument.source.storage == *storage)
            .then_some(call),
        CleanupTransition::SelectFailure { .. } | CleanupTransition::StageCopyResult { .. } => None,
    }
}

/// The ordered `ValueId`s of the owned `Bytes` locals live at `site`, exactly
/// [`owned_locals_live_at`]'s result mapped from cleanup-slot identity to the
/// storage identity the interpreter's environment is keyed by. Only ever
/// called for a function [`admit_owned_bytes_profile`] has already admitted,
/// so every returned slot is a whole `StorageId::Value` storage by
/// construction; a caller that violates that precondition gets `Err` rather
/// than a silently wrong mapping.
pub(crate) fn carried_locals_at(
    function: &ResolvedFunction,
    site: &ExpressionId,
) -> Result<Vec<ValueId>, Diagnostic> {
    let live = owned_locals_live_at(function, site)?;
    live.iter()
        .map(|id| {
            let slot = function
                .cleanup_plan
                .slots
                .iter()
                .find(|slot| slot.id == *id)
                .ok_or_else(|| refused("live slot id is absent from the cleanup plan"))?;
            match &slot.storage {
                StorageId::Value(value) => Ok(value.clone()),
                _ => Err(refused(
                    "live owned Bytes slot is not a whole let-bound local",
                )),
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use super::*;
    use crate::ast::Span;
    use crate::cleanup_plan::{
        BlockId, CleanupBlock, CleanupRegionId, CleanupTerminator, ExitTargetId,
    };
    use crate::hir::{
        FunctionExecutionId, OwnershipMode, ResolvedMatchArm, ResolvedMatchMode,
        ResolvedMatchPattern, ResolvedProgram,
    };
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
@id("test.branch_join")
fn branch_join(data: borrow Slice<u8>, flag: bool) -> i64 {
    let a = bytes_copy(data);
    let branched = if flag { 1 } else { 2 };
    let dummy = branched;
    let _ = consume(a);
    dummy
}
@id("test.scalar_branch_join")
fn scalar_branch_join(flag: bool) -> i64 {
    let branched = if flag { 1 } else { 2 };
    let dummy = branched;
    dummy
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

    /// Issue #296 R20 bug #296: a function with an empty cleanup plan (no
    /// owned `Bytes` local anywhere) is never refused merely because an
    /// earlier top-level statement happens to branch on its own -- there is
    /// no owned local for a join across that branch to lose track of, so
    /// the vacuous short-circuit at the top of `owned_locals_live_at` fires
    /// before `locate_predecessors`'s own branching-predecessor refusal ever
    /// runs.
    #[test]
    fn scalar_function_with_no_owned_locals_past_a_branching_predecessor_is_not_refused() {
        let program = program();
        let function = function(&program, "test.scalar_branch_join");
        assert!(
            function.cleanup_plan.slots.is_empty(),
            "fixture must carry no owned slot to exercise the vacuous short-circuit"
        );
        let site = site(function, "dummy");
        let live = owned_locals_live_at(function, &site)
            .expect("an empty cleanup plan is vacuously admitted regardless of branching");
        assert!(live.is_empty());
    }

    /// Issue #296 R20 bug #296: the empty-plan short-circuit above must not
    /// weaken the genuine refusal. `branch_join` carries a real owned
    /// `Bytes` local (`a`, a non-empty cleanup plan), so a later top-level
    /// site reached only past a preceding statement that branches on its own
    /// (`branched = if flag {..} else {..}`) still refuses `SPX-H006`,
    /// exactly as it did before the fix.
    #[test]
    fn owned_local_past_a_branching_predecessor_still_refuses_h006() {
        let program = program();
        let function = function(&program, "test.branch_join");
        assert!(
            !function.cleanup_plan.slots.is_empty(),
            "fixture must carry a real owned slot for this to be the genuine case"
        );
        let site = site(function, "dummy");
        let error = owned_locals_live_at(function, &site).expect_err(
            "a branching predecessor before a genuinely owned-Bytes-carrying site is refused",
        );
        assert_eq!(error.code, "SPX-H006");
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

    fn leaf(owner: &FunctionExecutionId, path: &str, span: Span) -> ResolvedExpr {
        ResolvedExpr {
            id: ExpressionId::new(owner, path),
            ty: ResolvedType::I64,
            ownership: OwnershipMode::Value,
            kind: ResolvedExprKind::Int(0),
            span,
        }
    }

    /// Issue #296, spec section 11.6, third increment: independent review
    /// found `site_is_loop_embedded`'s previous structural descent (only
    /// `If`/`Block` among expressions, `Let`/`Assign`/`While` among
    /// statements) missed a transition trigger reached through a `match` arm
    /// or an `unsafe` boundary nested inside a `while` body -- both are
    /// refused at the surface today (`hir::resolve_statement`'s while-body
    /// admission, `SPX-T252`), so this hand-builds the resolved HIR shape
    /// directly ("bypassing surface parsing") to prove the *walker* itself,
    /// not merely today's surface grammar, classifies them: a transition
    /// whose trigger sits inside either nesting must be reported touched.
    #[test]
    fn slot_touched_inside_while_finds_a_trigger_nested_in_match_or_unsafe() {
        let program = program();
        let mut function = function(&program, "test.no_owned").clone();
        let owner = FunctionExecutionId::Monomorphic(function.id.clone());
        let span = Span {
            start: 0,
            end: 0,
            line: 1,
            column: 1,
        };
        let marker_in_unsafe = leaf(&owner, "test.marker_in_unsafe", span);
        let marker_in_match = leaf(&owner, "test.marker_in_match", span);
        let scrutinee = leaf(&owner, "test.scrutinee", span);
        let condition = ResolvedExpr {
            kind: ResolvedExprKind::Bool(true),
            ty: ResolvedType::Bool,
            ..leaf(&owner, "test.condition", span)
        };
        let match_tail = ResolvedExpr {
            id: ExpressionId::new(&owner, "test.while_tail"),
            kind: ResolvedExprKind::Match {
                mode: ResolvedMatchMode::Value,
                scrutinee: Box::new(scrutinee),
                arms: vec![ResolvedMatchArm {
                    pattern: ResolvedMatchPattern::Wildcard,
                    guard: None,
                    value: marker_in_match.clone(),
                    span,
                }],
            },
            ..leaf(&owner, "test.while_tail_leaf", span)
        };
        let while_body = ResolvedExpr {
            id: ExpressionId::new(&owner, "test.while_body"),
            kind: ResolvedExprKind::Block {
                statements: vec![ResolvedStatement::Unsafe {
                    audit: "test".to_owned(),
                    body: Box::new(marker_in_unsafe.clone()),
                    span,
                }],
                tail: Box::new(match_tail),
            },
            ..leaf(&owner, "test.while_body_leaf", span)
        };
        let while_statement = ResolvedStatement::While {
            condition: Box::new(condition),
            body: Box::new(while_body),
            span,
        };
        let outer_tail = leaf(&owner, "test.outer_tail", span);
        function.body = ResolvedExpr {
            id: ExpressionId::new(&owner, "test.outer_body"),
            kind: ResolvedExprKind::Block {
                statements: vec![while_statement],
                tail: Box::new(outer_tail),
            },
            ..leaf(&owner, "test.outer_body_leaf", span)
        };

        let unsafe_value = ValueId::intrinsic_parameter("test.slot_touched_unsafe", 0);
        let match_value = ValueId::intrinsic_parameter("test.slot_touched_match", 0);
        function.cleanup_plan.blocks = vec![CleanupBlock {
            id: BlockId(0),
            region: CleanupRegionId(0),
            transitions: vec![
                CleanupTransition::Initialize {
                    at: marker_in_unsafe.id.clone(),
                    destination: CleanupPlace::whole(StorageId::Value(unsafe_value.clone())),
                },
                CleanupTransition::Initialize {
                    at: marker_in_match.id.clone(),
                    destination: CleanupPlace::whole(StorageId::Value(match_value.clone())),
                },
            ],
            terminator: CleanupTerminator::Exit(ExitTargetId(0)),
        }];

        assert!(
            slot_touched_inside_while(&function, &StorageId::Value(unsafe_value.clone())),
            "a transition triggered inside an `unsafe` boundary nested in a `while` body must \
             be reported touched"
        );
        assert!(
            slot_touched_inside_while(&function, &StorageId::Value(match_value.clone())),
            "a transition triggered inside a `match` arm nested in a `while` body must be \
             reported touched"
        );

        // A storage no transition touches at all is correctly untouched.
        let absent = ValueId::intrinsic_parameter("test.slot_touched_absent", 0);
        assert!(!slot_touched_inside_while(
            &function,
            &StorageId::Value(absent)
        ));

        // The same two markers, moved (by hand) to sit directly in the
        // function's own top-level body -- outside any `while` -- are
        // correctly reported untouched: this is the same walker, not a
        // special case for the positive result above.
        function.body = ResolvedExpr {
            id: ExpressionId::new(&owner, "test.outer_body_no_while"),
            kind: ResolvedExprKind::Block {
                statements: vec![ResolvedStatement::Unsafe {
                    audit: "test".to_owned(),
                    body: Box::new(marker_in_unsafe.clone()),
                    span,
                }],
                tail: Box::new(marker_in_match.clone()),
            },
            ..leaf(&owner, "test.outer_body_no_while_leaf", span)
        };
        assert!(!slot_touched_inside_while(
            &function,
            &StorageId::Value(unsafe_value)
        ));
        assert!(!slot_touched_inside_while(
            &function,
            &StorageId::Value(match_value)
        ));
    }
}

mod owned_frame;
pub(crate) use owned_frame::{
    owned_frame_body, owned_frame_copy_expression, owned_frame_liveness, owned_frame_parameter,
    OwnedFrameLiveness,
};

mod owned_frame_v2;
pub(crate) use owned_frame_v2::{
    owned_frame_v2_body, owned_frame_v2_liveness, owned_frame_v2_parameter,
};
