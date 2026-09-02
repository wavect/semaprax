//! Test-only observation counters and injected failure points for the
//! private lane, with inert `#[cfg(not(test))]` counterparts.

#[cfg(test)]
use super::*;

#[cfg(test)]
thread_local! {
    pub(super) static CANONICAL_FORMAT_PASS_COUNT: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
    pub(super) static HIR_RESOLVE_PASS_COUNT: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
    pub(super) static HIR_POST_RESOLVE_PHASE_COUNT: std::cell::Cell<[usize; 4]> = const { std::cell::Cell::new([0; 4]) };
    pub(super) static HIR_POST_RESOLVE_CAPACITY_HIGH_WATER: std::cell::Cell<[usize; 3]> = const { std::cell::Cell::new([0; 3]) };
    pub(super) static POST_HIR_FACTS_ENTRY_COUNT: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
    pub(super) static POST_HIR_FACTS_CAPACITY_HIGH_WATER: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
    pub(super) static POST_HIR_FACTS_SCRATCH_HIGH_WATER: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
    pub(super) static POST_HIR_AUTHORITY_TRANSFER_TERMS: std::cell::Cell<[usize; 5]> = const { std::cell::Cell::new([0; 5]) };
    pub(super) static POST_HIR_RENDER_CAPACITY_HIGH_WATER: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
    pub(super) static POST_HIR_REPLAY_CAPACITY_HIGH_WATER: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
    pub(super) static EXACT_ARTIFACT_OUTPUT_ALLOCATION_COUNT: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
    pub(super) static CLOSURE_CAPACITY_HIGH_WATER: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
    pub(super) static RESOLVED_DISPOSE_HIGH_WATER: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
    pub(super) static RESOLVED_DISPOSE_COMPLETIONS: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
    pub(super) static RESOLVED_DISPOSE_CAPACITIES: std::cell::Cell<[usize; 2]> = const { std::cell::Cell::new([0; 2]) };
    pub(super) static PREPARE_FAILURE_INJECTION: std::cell::Cell<Option<PrepareFailurePoint>> = const { std::cell::Cell::new(None) };
    pub(super) static CREATE_AUTH_DISAGREEMENT: std::cell::Cell<Option<CreateAuthDisagreement>> = const { std::cell::Cell::new(None) };
    pub(super) static CREATE_AUTH_DISCARD_ATTEMPTS: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
    pub(super) static PHASE_B_EFFECT_STARTED: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
    pub(super) static PHASE_B_POST_EFFECT_ERROR_MATERIALIZATIONS: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
    pub(super) static PHASE_B_NATIVE_STAGE_ARENA_ALLOCATIONS: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
    pub(super) static PHASE_B_NATIVE_STAGE_ARENA_SETS: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
    pub(super) static PHASE_B_NATIVE_STAGE_ARENA_CONSUMPTIONS: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
    pub(super) static PHASE_B_PREPARED_CARRIER_IDENTITIES: std::cell::Cell<[usize; 7]> = const { std::cell::Cell::new([0; 7]) };
    pub(super) static PHASE_B_LOCAL_FAILURE_INJECTION: std::cell::Cell<Option<PhaseBLocalError>> = const { std::cell::Cell::new(None) };
    pub(super) static PHASE_B_DISCARD_FAILURE_AFTER_DELETE: std::cell::Cell<Option<usize>> = const { std::cell::Cell::new(None) };
    pub(super) static PHASE_B_DISCARD_ATTEMPTS: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
    pub(super) static PHASE_B_OVERSIZE_MANIFEST_INJECTION: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
    pub(super) static PHASE_B_OUTPUT_PROBES: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
    pub(super) static PHASE_B_TOOL_HOLDS: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
    pub(super) static PHASE_B_TOOL_PROCESSES: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
    pub(super) static PHASE_B_PROCESS_ARENA_DROPS: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
    pub(super) static PHASE_B_PROCESS_ARENA_BUDGET_DROPS: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
    pub(super) static PHASE_B_PROCESS_ARENA_DROP_ORDER: std::cell::Cell<[u8; 2]> = const { std::cell::Cell::new([0; 2]) };
    pub(super) static PHASE_B_PROCESS_ARENA_DROP_ORDER_LENGTH: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
    pub(super) static PHASE_B_INVALID_TOOL_ENV_INJECTION: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
    pub(super) static PHASE_B_DIRECT_SYSROOT_MISMATCH_INJECTION: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
    pub(super) static PHASE_B_BUILD_INVOCATION_PLANS: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
    pub(super) static PHASE_B_BUILD_INVOCATION_CONSUMPTIONS: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
    pub(super) static PHASE_B_LINK_COPY_PLANS: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
    pub(super) static PHASE_B_LINK_COPY_CONSUMPTIONS: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
    pub(super) static PHASE_B_LINK_COPY_FAIL_BEFORE_AUTHENTICATION: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
    pub(super) static PHASE_B_INVENTORY_EXACT_PLANS: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
    pub(super) static PHASE_B_INVENTORY_EXACT_SCANS: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
    pub(super) static PHASE_B_PUBLISH_PLANS: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
    pub(super) static PHASE_B_PUBLISH_CONSUMPTIONS: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
    pub(super) static PHASE_B_PUBLISH_FAILURE: std::cell::Cell<u8> = const { std::cell::Cell::new(0) };
    pub(super) static PHASE_B_OBJECT_AUTHORITY_TRANSFERS: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
    pub(super) static PHASE_B_OBJECT_AUTHORITY_DROPS: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
    pub(super) static PHASE_B_OBJECT_AUTHORITY_LIVE: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
    pub(super) static PHASE_B_OBJECT_AUTHORITY_MANIFEST_OBSERVATIONS: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
    pub(super) static PHASE_B_OBJECT_AUTHORITY_PUBLISH_OBSERVATIONS: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
    pub(super) static PHASE_B_OBJECT_BYTES_DROPS: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
    pub(super) static PHASE_B_OBJECT_DROP_ORDER: std::cell::Cell<[u8; 2]> = const { std::cell::Cell::new([0; 2]) };
    pub(super) static PHASE_B_OBJECT_DROP_ORDER_LENGTH: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
    pub(super) static PHASE_B_MANIFEST_PLAN_CAPACITY: std::cell::Cell<usize> = const { std::cell::Cell::new(MAX_MANIFEST_BYTES) };
    pub(super) static PHASE_B_MANIFEST_ARENA_ALLOCATIONS: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
    pub(super) static PHASE_B_MANIFEST_ARENA_GROWTHS: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
    pub(super) static PHASE_B_MANIFEST_AUTHORITY_TRANSFERS: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
    pub(super) static PHASE_B_MANIFEST_AUTHORITY_DROPS: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
    pub(super) static PHASE_B_MANIFEST_AUTHORITY_LIVE: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
    pub(super) static PHASE_B_MANIFEST_BYTES_DROPS: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
    pub(super) static PHASE_B_MANIFEST_DROP_ORDER: std::cell::Cell<[u8; 2]> = const { std::cell::Cell::new([0; 2]) };
    pub(super) static PHASE_B_MANIFEST_DROP_ORDER_LENGTH: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}

#[cfg(test)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum PrepareFailurePoint {
    Closure,
    Facts,
    Render,
    Replay,
}

#[cfg(test)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum CreateAuthDisagreement {
    Clean,
    Substituted,
}

#[cfg(test)]
pub(super) fn inject_prepare_failure(point: PrepareFailurePoint) -> Result<(), Diagnostic> {
    if PREPARE_FAILURE_INJECTION.with(std::cell::Cell::get) == Some(point) {
        Err(b107("injected private preparation failure"))
    } else {
        Ok(())
    }
}

#[cfg(test)]
pub(super) fn note_canonical_format_pass() {
    CANONICAL_FORMAT_PASS_COUNT.with(|count| count.set(count.get() + 1));
}

#[cfg(test)]
pub(super) fn note_hir_resolve_pass() {
    HIR_RESOLVE_PASS_COUNT.with(|count| count.set(count.get() + 1));
}

#[cfg(test)]
pub(super) fn note_hir_post_resolve_phase(index: usize) {
    HIR_POST_RESOLVE_PHASE_COUNT.with(|counts| {
        let mut values = counts.get();
        values[index] += 1;
        counts.set(values);
    });
}

#[cfg(test)]
pub(super) fn note_hir_post_resolve_capacity(index: usize, bytes: usize) {
    HIR_POST_RESOLVE_CAPACITY_HIGH_WATER.with(|water| {
        let mut values = water.get();
        values[index] = values[index].max(bytes);
        water.set(values);
    });
}

#[cfg(test)]
pub(super) fn note_post_hir_facts_entry() {
    POST_HIR_FACTS_ENTRY_COUNT.with(|count| count.set(count.get() + 1));
}

#[cfg(test)]
pub(super) fn note_post_hir_facts_capacity(bytes: usize) {
    POST_HIR_FACTS_CAPACITY_HIGH_WATER.with(|water| water.set(water.get().max(bytes)));
}

#[cfg(test)]
pub(super) fn note_post_hir_facts_scratch(bytes: usize) {
    POST_HIR_FACTS_SCRATCH_HIGH_WATER.with(|water| water.set(water.get().max(bytes)));
}

#[cfg(test)]
pub(super) fn note_post_hir_render_capacity(bytes: usize) {
    POST_HIR_RENDER_CAPACITY_HIGH_WATER.with(|water| water.set(water.get().max(bytes)));
}

#[cfg(test)]
pub(super) fn note_post_hir_replay_capacity(bytes: usize) {
    POST_HIR_REPLAY_CAPACITY_HIGH_WATER.with(|water| water.set(water.get().max(bytes)));
}

#[cfg(not(test))]
pub(super) fn note_hir_post_resolve_phase(_index: usize) {}

#[cfg(not(test))]
pub(super) fn note_hir_post_resolve_capacity(_index: usize, _bytes: usize) {}

#[cfg(not(test))]
pub(super) fn note_hir_resolve_pass() {}

#[cfg(test)]
pub(super) fn reset_closure_capacity_high_water() {
    CLOSURE_CAPACITY_HIGH_WATER.with(|water| water.set(0));
}

#[cfg(test)]
pub(super) fn closure_capacity_high_water() -> usize {
    CLOSURE_CAPACITY_HIGH_WATER.with(std::cell::Cell::get)
}

#[cfg(test)]
pub(super) fn note_closure_capacity_high_water(bytes: usize) {
    CLOSURE_CAPACITY_HIGH_WATER.with(|water| water.set(water.get().max(bytes)));
}

#[cfg(test)]
pub(super) fn note_resolved_dispose_high_water(len: usize) {
    RESOLVED_DISPOSE_HIGH_WATER.with(|water| water.set(water.get().max(len)));
}

#[cfg(not(test))]
pub(super) fn note_resolved_dispose_high_water(_len: usize) {}

#[cfg(test)]
pub(super) fn note_resolved_dispose_completion() {
    RESOLVED_DISPOSE_COMPLETIONS.with(|count| count.set(count.get() + 1));
}

#[cfg(test)]
pub(super) fn note_resolved_dispose_capacity(index: usize, capacity: usize) {
    RESOLVED_DISPOSE_CAPACITIES.with(|capacities| {
        let mut values = capacities.get();
        values[index] = capacity;
        capacities.set(values);
    });
}

#[cfg(not(test))]
pub(super) fn note_resolved_dispose_capacity(_index: usize, _capacity: usize) {}

#[cfg(not(test))]
pub(super) fn note_resolved_dispose_completion() {}

#[cfg(not(test))]
pub(super) fn note_canonical_format_pass() {}
