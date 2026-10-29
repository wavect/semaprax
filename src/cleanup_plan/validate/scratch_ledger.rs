//! Cleanup-replay scratch is metered on its own per-program ledger.
//!
//! Independent replay is verification scratch: every carrier it reserves is
//! released when the program's replay returns, and none of it is retained by
//! the resolved program, graph or artifact. Under an active builder it keeps
//! the exact ordinary `ActiveBuilder` payer and every per-allocation charge,
//! but those charges are debited from a fresh detached ledger instead of being
//! settled cumulatively into the enclosing builder ledger. The detached ledger
//! has the Workspace Semantic Graph builder limit: a replay may consume what a
//! builder ledger could admit, but no longer shares it with graph
//! construction. It is independent of the enclosing ledger's remaining or
//! nested limit, since a stage budget is sized for its own retained output.
//! Exhaustion is the replay's existing `SPX-H006` refusal. Without an active
//! ledger replay stays unobserved, and an already refused enclosing ledger
//! keeps its route.
use crate::diagnostic::Diagnostic;
use crate::hir::ResolvedProgram;

/// Per-program cleanup-replay scratch ceiling.
const MAX_REPLAY_SCRATCH_BYTES: usize = crate::workspace_graph::MAX_BUILDER_BYTES;

/// Complete independent replay of a resolved program on the detached ledger.
pub(in crate::cleanup_plan) fn validate_program(
    program: &ResolvedProgram,
) -> Result<(), Diagnostic> {
    detached(|| crate::cleanup_plan::replay::validate_program(program))
}

/// Complete independent replay of a projected context on the detached ledger.
pub(in crate::cleanup_plan) fn validate_program_context(
    program: &dyn crate::cleanup::ReplayContext,
) -> Result<(), Diagnostic> {
    detached(|| crate::cleanup_plan::replay::validate_program_context(program))
}

fn detached(replay: impl FnOnce() -> Result<(), Diagnostic>) -> Result<(), Diagnostic> {
    if crate::bounded_output::active_limit().is_none() || crate::bounded_output::active_refused() {
        return replay();
    }
    let (result, refused, _) =
        crate::bounded_output::detached::with_detached_limit(MAX_REPLAY_SCRATCH_BYTES, replay);
    match result {
        Err(error) => Err(error),
        Ok(()) if refused => Err(Diagnostic::io(
            "SPX-H006",
            "cleanup replay scratch exceeds its per-program physical reservation",
        )),
        Ok(()) => Ok(()),
    }
}
