//! Measure the current replay receiver against its ordinary detached ledger.
use super::*;
use crate::cleanup_plan::validate::scratch_ledger::MAX_REPLAY_SCRATCH_BYTES;

pub(in crate::cleanup_plan) fn replay(
    program: &ResolvedProgram,
    indexed: bool,
) -> (Result<(usize, usize), Diagnostic>, bool, usize) {
    crate::bounded_output::detached::with_detached_limit(MAX_REPLAY_SCRATCH_BYTES, || {
        fact_preflight::phase_measurement::replay(program, indexed)
    })
}
