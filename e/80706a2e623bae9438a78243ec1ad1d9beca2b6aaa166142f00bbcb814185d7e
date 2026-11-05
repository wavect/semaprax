//! Diagnostic-only fixed storage. No compiler payment, work or verdict authority.
use super::*;
use std::cell::{Cell, RefCell};
#[derive(Clone, Copy, Debug)]
pub(super) struct Row {
    pub invocation: usize,
    pub generation: usize,
    pub serial: usize,
    pub old: bool,
    pub function: [u8; 256],
    pub function_len: usize,
    pub payer: &'static str,
    pub selected: bool,
    pub owner_admitted: bool,
    pub certified_bases: usize,
    pub certified_pops: usize,
    pub certified_rhs_assignments: usize,
    pub certified_views: usize,
    pub certified_heads: usize,
    pub certified_firsts: usize,
    pub pair_matches_admitted: usize,
    pub pairs: usize,
    pub rounds: usize,
    pub value_steps: usize,
    pub value_tails: usize,
    pub tails: usize,
    pub mask_checks: usize,
    pub tail_marks: usize,
    pub fused_turns: usize,
    pub tail_work: usize,
    pub pair_left_steps: usize,
    pub pair_right_steps: usize,
    pub pair_left_tails: usize,
    pub pair_right_tails: usize,
    pub pair_receivers: usize,
    legacy_final: bool,
    pub final_flat: usize,
    pub final_shared: usize,
    pub final_steps: usize,
    pub final_tails: usize,
    pub steps: usize,
    pub shared_checks: usize,
    pub shared_patterns: usize,
    pub shared_metadata: usize,
    pub shared_steps: usize,
    pub shared_empty_checks: usize,
    pub shared_links: usize,
    pub shared_queries: usize,
    pub shared_boundaries: usize,
    pub shared_runs: usize,
    pub shared_boundary_complete: usize,
    pub shared_publishes: usize,
    pub shared_turns: usize,
    pub loads: usize,
    pub identity_empty_probes: usize,
    pub empty_selectors: usize,
    pub empty_selector_skips: usize,
    pub identity_bodies: usize,
    pub identity_probes: usize,
    pub identity_detectors: usize,
    pub identity_hits: usize,
    pub identity_observations: usize,
    pub identity_work: usize,
    pub identity_captures: usize,
    pub identity_extent_captures: usize,
    pub complete: bool,
    pub ended: bool,
    pub unwound: bool,
    pub overflow: bool,
    pub truncated: bool,
}
const EMPTY: Row = Row {
    invocation: 0,
    generation: 0,
    serial: 0,
    old: false,
    function: [0; 256],
    function_len: 0,
    payer: "",
    selected: false,
    owner_admitted: false,
    certified_bases: 0,
    certified_pops: 0,
    certified_rhs_assignments: 0,
    certified_views: 0,
    certified_heads: 0,
    certified_firsts: 0,
    pair_matches_admitted: 0,
    pairs: 0,
    rounds: 0,
    value_steps: 0,
    value_tails: 0,
    tails: 0,
    mask_checks: 0,
    tail_marks: 0,
    fused_turns: 0,
    tail_work: 0,
    pair_left_steps: 0,
    pair_right_steps: 0,
    pair_left_tails: 0,
    pair_right_tails: 0,
    pair_receivers: 0,
    legacy_final: false,
    final_flat: 0,
    final_shared: 0,
    final_steps: 0,
    final_tails: 0,
    steps: 0,
    shared_checks: 0,
    shared_patterns: 0,
    shared_metadata: 0,
    shared_steps: 0,
    shared_empty_checks: 0,
    shared_links: 0,
    shared_queries: 0,
    shared_boundaries: 0,
    shared_runs: 0,
    shared_boundary_complete: 0,
    shared_publishes: 0,
    shared_turns: 0,
    loads: 0,
    identity_empty_probes: 0,
    empty_selectors: 0,
    empty_selector_skips: 0,
    identity_bodies: 0,
    identity_probes: 0,
    identity_detectors: 0,
    identity_hits: 0,
    identity_observations: 0,
    identity_work: 0,
    identity_captures: 0,
    identity_extent_captures: 0,
    complete: false,
    ended: false,
    unwound: false,
    overflow: false,
    truncated: false,
};
pub(super) fn empty_for_test() -> Row {
    EMPTY
}
struct Trace {
    enabled: bool,
    count: usize,
    generation: usize,
    serial: usize,
    rows: Vec<Row>,
}
thread_local! {
    static TRACE: RefCell<Trace> = const { RefCell::new(Trace { enabled: false, count: 0, generation: 0, serial: 0, rows: Vec::new() }) };
    static LOST: Cell<usize> = const { Cell::new(0) };
    static UNWIND_TAIL: Cell<bool> = const { Cell::new(false) };
    static UNWIND_FINAL: Cell<bool> = const { Cell::new(false) };
    static UNWIND_IDENTITY: Cell<bool> = const { Cell::new(false) };
}
fn lost() {
    LOST.with(|n| n.set(n.get().saturating_add(1)));
}
pub(super) fn unwind_next_identity_observation() {
    UNWIND_IDENTITY.with(|flag| flag.set(true));
}
pub(super) fn unwind_next_tail_work() {
    UNWIND_TAIL.with(|flag| flag.set(true));
}
pub(super) fn maybe_unwind_tail_work() {
    if UNWIND_TAIL.with(|flag| flag.replace(false)) {
        std::panic::resume_unwind(Box::new(
            "test-only unwind after an admitted paired tail work",
        ));
    }
}
pub(super) fn unwind_next_final_round() {
    UNWIND_FINAL.with(|flag| flag.set(true));
}
pub(super) fn reset() {
    super::super::frontiers::prepare();
    UNWIND_FINAL.with(|flag| flag.set(false));
    UNWIND_TAIL.with(|flag| flag.set(false));
    UNWIND_IDENTITY.with(|flag| flag.set(false));
    LOST.with(|n| n.set(0));
    TRACE.with(|t| {
        let Ok(mut t) = t.try_borrow_mut() else {
            lost();
            return;
        };
        if t.enabled {
            for row in &t.rows[..t.count] {
                if !row.ended {
                    lost();
                }
            }
        }
        t.rows.resize(128, EMPTY);
        t.count = 0;
        if let Some(generation) = t.generation.checked_add(1) {
            t.generation = generation;
            t.enabled = true;
        } else {
            t.enabled = false;
            lost();
        }
    });
}
#[derive(Clone, Copy)]
struct Identity {
    index: usize,
    generation: usize,
    serial: usize,
}
pub(in crate::cleanup_plan::replay::ordinary_plan_history) struct Guard {
    identity: Option<Identity>,
}
pub(in crate::cleanup_plan::replay::ordinary_plan_history) fn begin(
    function: &ResolvedFunction,
    budget: &ReplayBudget,
    old: bool,
) -> Guard {
    let identity = TRACE.with(|t| {
        let Ok(mut t) = t.try_borrow_mut() else {
            lost();
            return None;
        };
        if !t.enabled {
            return None;
        }
        if t.count == t.rows.len() {
            lost();
            return None;
        }
        let Some(serial) = t.serial.checked_add(1) else {
            lost();
            return None;
        };
        t.serial = serial;
        let index = t.count;
        t.count += 1;
        let mut row = EMPTY;
        row.invocation = index;
        row.generation = t.generation;
        row.serial = serial;
        row.old = old;
        let id = function.id.as_str().as_bytes();
        row.function_len = id.len().min(row.function.len());
        row.truncated = id.len() > row.function.len();
        row.function[..row.function_len].copy_from_slice(&id[..row.function_len]);
        row.payer = match budget.payer {
            AllocationPayer::LegacyUnobserved => "unobserved",
            AllocationPayer::ActiveBuilder => "active-builder",
            AllocationPayer::SelectedReservation => "selected-reservation",
            AllocationPayer::Oracle(_) => "oracle",
        };
        row.selected = budget.admission == allocation::AdmissionMode::SelectedRetained;
        t.rows[index] = row;
        Some(Identity {
            index,
            generation: t.generation,
            serial,
        })
    });
    Guard { identity }
}
impl Guard {
    fn update(&self, action: impl FnOnce(&mut Row)) {
        let Some(identity) = self.identity else {
            return;
        };
        TRACE.with(|t| {
            if let Ok(mut t) = t.try_borrow_mut() {
                if !t.enabled || t.generation != identity.generation || identity.index >= t.count {
                    lost();
                    return;
                }
                let row = &mut t.rows[identity.index];
                if row.generation != identity.generation || row.serial != identity.serial {
                    lost();
                    return;
                }
                action(row);
            } else {
                lost();
            }
        });
    }
    pub(super) fn owner_admitted(&self) {
        self.update(|r| r.owner_admitted = true);
    }
    pub(super) fn pair_left_steps(&self) {
        self.update(|r| match r.pair_left_steps.checked_add(1) {
            Some(n) => r.pair_left_steps = n,
            None => r.overflow = true,
        });
    }
    pub(super) fn pair_right_steps(&self) {
        self.update(|r| match r.pair_right_steps.checked_add(1) {
            Some(n) => r.pair_right_steps = n,
            None => r.overflow = true,
        });
    }
    pub(super) fn pair_left_tails(&self) {
        self.update(|r| match r.pair_left_tails.checked_add(1) {
            Some(n) => r.pair_left_tails = n,
            None => r.overflow = true,
        });
    }
    pub(super) fn pair_right_tails(&self) {
        self.update(|r| match r.pair_right_tails.checked_add(1) {
            Some(n) => r.pair_right_tails = n,
            None => r.overflow = true,
        });
    }
    pub(super) fn pair_receivers(&self) {
        self.update(|r| match r.pair_receivers.checked_add(1) {
            Some(n) => r.pair_receivers = n,
            None => r.overflow = true,
        });
    }
    pub(super) fn pair_match_admitted(&self) {
        self.update(|r| match r.pair_matches_admitted.checked_add(1) {
            Some(n) => r.pair_matches_admitted = n,
            None => r.overflow = true,
        });
    }
    pub(in crate::cleanup_plan::replay::ordinary_plan_history) fn pair(&self) {
        self.update(|r| match r.pairs.checked_add(1) {
            Some(n) => r.pairs = n,
            None => r.overflow = true,
        });
    }
    pub(in crate::cleanup_plan::replay::ordinary_plan_history) fn round(&self) {
        self.update(|r| match r.rounds.checked_add(1) {
            Some(n) => r.rounds = n,
            None => r.overflow = true,
        });
    }
    pub(super) fn certified_base(&self) {
        self.update(|r| match r.certified_bases.checked_add(1) {
            Some(n) => r.certified_bases = n,
            None => r.overflow = true,
        });
    }
    pub(super) fn certified_pop(&self) {
        self.update(|r| match r.certified_pops.checked_add(1) {
            Some(n) => r.certified_pops = n,
            None => r.overflow = true,
        });
    }
    pub(super) fn certified_rhs(&self) {
        self.update(|r| match r.certified_rhs_assignments.checked_add(1) {
            Some(n) => r.certified_rhs_assignments = n,
            None => r.overflow = true,
        });
    }
    pub(super) fn certified_view(&self) {
        self.update(|r| match r.certified_views.checked_add(1) {
            Some(n) => r.certified_views = n,
            None => r.overflow = true,
        });
    }
    pub(super) fn certified_head(&self) {
        self.update(|r| match r.certified_heads.checked_add(1) {
            Some(n) => r.certified_heads = n,
            None => r.overflow = true,
        });
    }
    pub(super) fn certified_first(&self) {
        self.update(|r| match r.certified_firsts.checked_add(1) {
            Some(n) => r.certified_firsts = n,
            None => r.overflow = true,
        });
    }
    pub(super) fn value_step(&self) {
        self.update(|r| match r.value_steps.checked_add(1) {
            Some(n) => r.value_steps = n,
            None => r.overflow = true,
        });
    }
    pub(super) fn value_tail(&self) {
        self.update(|r| match r.value_tails.checked_add(1) {
            Some(n) => r.value_tails = n,
            None => r.overflow = true,
        });
    }
    pub(super) fn legacy_final_flat(&self) {
        self.final_flat();
        self.update(|r| r.legacy_final = true);
    }
    pub(super) fn legacy_final_shared(&self) {
        self.final_shared();
        self.update(|r| r.legacy_final = true);
    }
    pub(super) fn final_flat(&self) {
        self.update(|r| match r.final_flat.checked_add(1) {
            Some(n) => r.final_flat = n,
            None => r.overflow = true,
        });
        if UNWIND_FINAL.with(|flag| flag.replace(false)) {
            std::panic::resume_unwind(Box::new(
                "test-only unwind inside independent PLAN/HIR comparison",
            ));
        }
    }
    pub(super) fn final_shared(&self) {
        self.update(|r| match r.final_shared.checked_add(1) {
            Some(n) => r.final_shared = n,
            None => r.overflow = true,
        });
        if UNWIND_FINAL.with(|flag| flag.replace(false)) {
            std::panic::resume_unwind(Box::new(
                "test-only unwind inside independent PLAN/HIR comparison",
            ));
        }
    }
    pub(super) fn final_step(&self) {
        self.update(|r| match r.final_steps.checked_add(1) {
            Some(n) => r.final_steps = n,
            None => r.overflow = true,
        });
    }
    pub(super) fn final_tail(&self) {
        self.update(|r| match r.final_tails.checked_add(1) {
            Some(n) => r.final_tails = n,
            None => r.overflow = true,
        });
    }
    pub(super) fn step(&self) {
        self.update(|r| {
            if r.legacy_final {
                match r.final_steps.checked_add(1) {
                    Some(n) => r.final_steps = n,
                    None => r.overflow = true,
                }
            }
        });
        self.update(|r| match r.steps.checked_add(1) {
            Some(n) => r.steps = n,
            None => r.overflow = true,
        });
    }
    pub(super) fn shared_checks(&self) {
        self.update(|r| match r.shared_checks.checked_add(1) {
            Some(n) => r.shared_checks = n,
            None => r.overflow = true,
        });
    }
    pub(super) fn shared_patterns(&self) {
        self.update(|r| match r.shared_patterns.checked_add(1) {
            Some(n) => r.shared_patterns = n,
            None => r.overflow = true,
        });
    }
    pub(super) fn shared_metadata(&self) {
        self.update(|r| match r.shared_metadata.checked_add(1) {
            Some(n) => r.shared_metadata = n,
            None => r.overflow = true,
        });
    }
    pub(super) fn shared_steps(&self) {
        self.update(|r| match r.shared_steps.checked_add(1) {
            Some(n) => r.shared_steps = n,
            None => r.overflow = true,
        });
    }
    pub(super) fn shared_empty_checks(&self) {
        self.update(|r| match r.shared_empty_checks.checked_add(1) {
            Some(n) => r.shared_empty_checks = n,
            None => r.overflow = true,
        });
    }
    pub(super) fn shared_links(&self) {
        self.update(|r| match r.shared_links.checked_add(1) {
            Some(n) => r.shared_links = n,
            None => r.overflow = true,
        });
    }
    pub(super) fn shared_queries(&self) {
        self.update(|r| match r.shared_queries.checked_add(1) {
            Some(n) => r.shared_queries = n,
            None => r.overflow = true,
        });
    }
    pub(super) fn shared_boundaries(&self) {
        self.update(|r| match r.shared_boundaries.checked_add(1) {
            Some(n) => r.shared_boundaries = n,
            None => r.overflow = true,
        });
    }
    pub(super) fn shared_publishes(&self) {
        self.update(|r| match r.shared_publishes.checked_add(1) {
            Some(n) => r.shared_publishes = n,
            None => r.overflow = true,
        });
    }
    pub(super) fn shared_boundary_complete(&self) {
        self.update(|r| match r.shared_boundary_complete.checked_add(1) {
            Some(n) => r.shared_boundary_complete = n,
            None => r.overflow = true,
        });
    }
    pub(super) fn shared_runs(&self) {
        self.update(|r| match r.shared_runs.checked_add(1) {
            Some(n) => r.shared_runs = n,
            None => r.overflow = true,
        });
    }
    pub(super) fn shared_turns(&self) {
        self.update(|r| match r.shared_turns.checked_add(1) {
            Some(n) => r.shared_turns = n,
            None => r.overflow = true,
        });
    }
    pub(super) fn load(&self) {
        self.update(|r| match r.loads.checked_add(1) {
            Some(n) => r.loads = n,
            None => r.overflow = true,
        });
    }
    pub(super) fn mask_checks(&self) {
        self.update(|r| match r.mask_checks.checked_add(1) {
            Some(n) => r.mask_checks = n,
            None => r.overflow = true,
        });
    }
    pub(super) fn tail_marks(&self) {
        self.update(|r| match r.tail_marks.checked_add(1) {
            Some(n) => r.tail_marks = n,
            None => r.overflow = true,
        });
    }
    pub(super) fn fused_turns(&self) {
        self.update(|r| match r.fused_turns.checked_add(1) {
            Some(n) => r.fused_turns = n,
            None => r.overflow = true,
        });
    }
    pub(super) fn tail_work(&self) {
        self.update(|r| match r.tail_work.checked_add(1) {
            Some(n) => r.tail_work = n,
            None => r.overflow = true,
        });
    }
    pub(super) fn empty_selector(&self) {
        self.update(|r| match r.empty_selectors.checked_add(1) {
            Some(n) => r.empty_selectors = n,
            None => r.overflow = true,
        });
    }
    pub(super) fn empty_selector_skip(&self) {
        self.update(|r| match r.empty_selector_skips.checked_add(1) {
            Some(n) => r.empty_selector_skips = n,
            None => r.overflow = true,
        });
    }
    pub(super) fn identity_empty_probe(&self) {
        self.update(|r| match r.identity_empty_probes.checked_add(1) {
            Some(n) => r.identity_empty_probes = n,
            None => r.overflow = true,
        });
    }
    pub(super) fn identity_body(&self) {
        self.update(|r| match r.identity_bodies.checked_add(1) {
            Some(n) => r.identity_bodies = n,
            None => r.overflow = true,
        });
    }
    pub(super) fn identity_probe(&self) {
        self.update(|r| match r.identity_probes.checked_add(1) {
            Some(n) => r.identity_probes = n,
            None => r.overflow = true,
        });
    }
    pub(super) fn identity_detector(&self) {
        self.update(|r| match r.identity_detectors.checked_add(1) {
            Some(n) => r.identity_detectors = n,
            None => r.overflow = true,
        });
    }
    pub(super) fn identity_hit(&self) {
        self.update(|r| match r.identity_hits.checked_add(1) {
            Some(n) => r.identity_hits = n,
            None => r.overflow = true,
        });
    }
    pub(super) fn identity_observation(&self) {
        self.update(|r| match r.identity_observations.checked_add(1) {
            Some(n) => r.identity_observations = n,
            None => r.overflow = true,
        });
        if UNWIND_IDENTITY.with(|flag| flag.replace(false)) {
            // Exercise real cleanup unwinding without running the test panic
            // reporter inside the allocation observation (which retains state).
            // The caught payload is still allocated and dropped in that window.
            std::panic::resume_unwind(Box::new(
                "test-only unwind inside live certified PLAN remainder",
            ));
        }
    }
    pub(super) fn identity_work(&self) {
        self.update(|r| match r.identity_work.checked_add(1) {
            Some(n) => r.identity_work = n,
            None => r.overflow = true,
        });
    }
    pub(super) fn identity_capture(&self) {
        self.update(|r| match r.identity_captures.checked_add(1) {
            Some(n) => r.identity_captures = n,
            None => r.overflow = true,
        });
    }
    pub(super) fn identity_extent_capture(&self) {
        self.update(|r| match r.identity_extent_captures.checked_add(1) {
            Some(n) => r.identity_extent_captures = n,
            None => r.overflow = true,
        });
    }
    pub(super) fn tail(&self) {
        self.update(|r| {
            if r.legacy_final {
                match r.final_tails.checked_add(1) {
                    Some(n) => r.final_tails = n,
                    None => r.overflow = true,
                }
            }
        });
        self.update(|r| match r.tails.checked_add(1) {
            Some(n) => r.tails = n,
            None => r.overflow = true,
        });
    }
    pub(in crate::cleanup_plan::replay::ordinary_plan_history) fn success(&self) {
        self.update(|r| r.complete = !r.truncated && !r.overflow);
    }
}
impl Drop for Guard {
    fn drop(&mut self) {
        self.update(|r| {
            r.ended = true;
            r.unwound = std::thread::panicking();
        });
    }
}
// Called only after the original result/fixture/owner Drop and observed budget.
// Allocation here cannot affect a compiler outcome or frontier.
pub(super) fn finish() -> (Vec<Row>, usize) {
    let mut rows = TRACE.with(|t| {
        let Ok(mut t) = t.try_borrow_mut() else {
            lost();
            return Vec::new();
        };
        t.enabled = false;
        let count = t.count;
        for row in &mut t.rows[..count] {
            if !row.ended || row.unwound || row.truncated || row.overflow {
                row.complete = false;
            }
        }
        t.rows[..t.count].to_vec()
    });
    let lost = LOST.with(Cell::get);
    if lost != 0 {
        for row in &mut rows {
            row.complete = false;
        }
    }
    (rows, lost)
}

#[cfg(test)]
mod controls {
    use super::*;
    fn fixture() -> ResolvedFunction {
        let mut program = super::super::super::tests::program();
        program.functions.remove(0)
    }
    #[test]
    fn nested_reset_rejects_foreign_and_restored_generation_without_touching_new_row() {
        let function = fixture();
        let budget = ReplayBudget::unobserved();
        reset();
        let outer = begin(&function, &budget, true);
        outer.pair();
        reset();
        let current = begin(&function, &budget, false);
        current.pair();
        outer.round();
        outer.success();
        drop(outer);
        current.round();
        current.success();
        drop(current);
        let (rows, lost_count) = finish();
        assert!(lost_count > 0);
        assert_eq!(rows.len(), 1);
        assert_eq!((rows[0].old, rows[0].pairs, rows[0].rounds), (false, 1, 1));
        assert!(rows[0].ended && !rows[0].complete);
        reset();
        let restored = begin(&function, &budget, true);
        restored.success();
        drop(restored);
        let (rows, lost_count) = finish();
        assert_eq!(lost_count, 0);
        assert!(rows[0].complete);
    }
    #[test]
    fn finish_while_guard_is_live_cannot_publish_complete_or_resume_same_row() {
        let function = fixture();
        let budget = ReplayBudget::unobserved();
        reset();
        let live = begin(&function, &budget, false);
        live.success();
        let (rows, lost_count) = finish();
        assert_eq!(lost_count, 0);
        assert!(!rows[0].ended && !rows[0].complete);
        live.pair();
        drop(live);
        let (rows, lost_count) = finish();
        assert!(lost_count > 0);
        assert_eq!(rows[0].pairs, 0);
        assert!(!rows[0].complete);
    }
    #[test]
    fn serial_overflow_is_explicit_loss_and_does_not_alias_a_live_invocation() {
        let function = fixture();
        let budget = ReplayBudget::unobserved();
        reset();
        let live = begin(&function, &budget, false);
        live.pair();
        TRACE.with(|t| t.borrow_mut().serial = usize::MAX);
        let refused = begin(&function, &budget, true);
        refused.success();
        drop(refused);
        live.success();
        drop(live);
        let (rows, lost_count) = finish();
        assert_eq!(lost_count, 1);
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].pairs, 1);
        assert!(!rows[0].complete);
        // Restore test-only counter state after proving the terminal overflow.
        TRACE.with(|t| t.borrow_mut().serial = 0);
    }
    #[test]
    fn generation_overflow_disables_collection_and_never_wraps_to_old_identity() {
        let function = fixture();
        let budget = ReplayBudget::unobserved();
        reset();
        let old = begin(&function, &budget, true);
        TRACE.with(|t| t.borrow_mut().generation = usize::MAX);
        reset();
        let refused = begin(&function, &budget, false);
        refused.success();
        drop(refused);
        old.success();
        drop(old);
        let (rows, lost_count) = finish();
        assert!(lost_count > 0);
        assert!(rows.is_empty());
        TRACE.with(|t| t.borrow_mut().generation = 0);
    }
    #[test]
    fn empty_selector_counter_overflow_is_incomplete_without_work_or_observation_authority() {
        for skipped in [false, true] {
            let function = fixture();
            let budget = ReplayBudget::unobserved();
            let remaining = budget.remaining;
            reset();
            let guard = begin(&function, &budget, false);
            TRACE.with(|trace| {
                let mut t = trace.borrow_mut();
                if skipped {
                    t.rows[0].empty_selector_skips = usize::MAX;
                } else {
                    t.rows[0].empty_selectors = usize::MAX;
                }
            });
            if skipped {
                guard.empty_selector_skip();
            } else {
                guard.empty_selector();
            }
            guard.success();
            drop(guard);
            let (rows, lost) = finish();
            assert_eq!(lost, 0);
            assert_eq!(rows.len(), 1);
            assert!(rows[0].overflow && rows[0].ended && !rows[0].complete);
            assert_eq!(budget.remaining, remaining);
        }
    }
    #[test]
    fn prepared_link_counter_overflow_is_incomplete_and_does_not_spend_work() {
        for load in [false, true] {
            let function = fixture();
            let budget = ReplayBudget::unobserved();
            let remaining = budget.remaining;
            reset();
            let guard = begin(&function, &budget, false);
            TRACE.with(|trace| {
                let mut trace = trace.borrow_mut();
                if load {
                    trace.rows[0].loads = usize::MAX;
                } else {
                    trace.rows[0].steps = usize::MAX;
                }
            });
            if load {
                guard.load();
            } else {
                guard.step();
            }
            guard.success();
            drop(guard);
            let (rows, lost_count) = finish();
            assert_eq!(lost_count, 0);
            assert_eq!(rows.len(), 1);
            assert!(rows[0].overflow && rows[0].ended && !rows[0].complete);
            assert_eq!(budget.remaining, remaining);
        }
    }
    #[test]
    fn shared_step_counter_overflow_is_incomplete_and_does_not_spend_work() {
        for counter in 0..4 {
            let function = fixture();
            let budget = ReplayBudget::unobserved();
            let remaining = budget.remaining;
            reset();
            let guard = begin(&function, &budget, false);
            TRACE.with(|trace| {
                let mut trace = trace.borrow_mut();
                match counter {
                    0 => trace.rows[0].shared_checks = usize::MAX,
                    1 => trace.rows[0].shared_patterns = usize::MAX,
                    2 => trace.rows[0].shared_metadata = usize::MAX,
                    _ => trace.rows[0].shared_steps = usize::MAX,
                }
            });
            match counter {
                0 => guard.shared_checks(),
                1 => guard.shared_patterns(),
                2 => guard.shared_metadata(),
                _ => guard.shared_steps(),
            }
            guard.success();
            drop(guard);
            let (rows, lost) = finish();
            assert_eq!(lost, 0);
            assert_eq!(budget.remaining, remaining);
            assert!(rows[0].overflow && rows[0].ended && !rows[0].complete);
        }
    }
    #[test]
    fn shared_boundary_counter_overflow_is_incomplete_without_spending_work() {
        for counter in 0..4 {
            let function = fixture();
            let budget = ReplayBudget::unobserved();
            let remaining = budget.remaining;
            reset();
            let guard = begin(&function, &budget, false);
            TRACE.with(|trace| {
                let mut trace = trace.borrow_mut();
                match counter {
                    0 => trace.rows[0].shared_empty_checks = usize::MAX,
                    1 => trace.rows[0].shared_links = usize::MAX,
                    2 => trace.rows[0].shared_queries = usize::MAX,
                    _ => trace.rows[0].shared_boundaries = usize::MAX,
                }
            });
            match counter {
                0 => guard.shared_empty_checks(),
                1 => guard.shared_links(),
                2 => guard.shared_queries(),
                _ => guard.shared_boundaries(),
            }
            guard.success();
            drop(guard);
            let (rows, lost) = finish();
            assert_eq!(lost, 0);
            assert_eq!(budget.remaining, remaining);
            assert!(rows[0].overflow && rows[0].ended && !rows[0].complete);
        }
    }
    #[test]
    fn shared_run_counter_overflow_is_incomplete_without_work_or_identity_aliasing() {
        for counter in 0..4 {
            let function = fixture();
            let budget = ReplayBudget::unobserved();
            let remaining = budget.remaining;
            reset();
            let guard = begin(&function, &budget, false);
            TRACE.with(|trace| {
                let mut trace = trace.borrow_mut();
                match counter {
                    0 => trace.rows[0].shared_runs = usize::MAX,
                    1 => trace.rows[0].shared_turns = usize::MAX,
                    2 => trace.rows[0].shared_publishes = usize::MAX,
                    _ => trace.rows[0].shared_boundary_complete = usize::MAX,
                }
            });
            match counter {
                0 => guard.shared_runs(),
                1 => guard.shared_turns(),
                2 => guard.shared_publishes(),
                _ => guard.shared_boundary_complete(),
            }
            guard.success();
            drop(guard);
            let (rows, lost) = finish();
            assert_eq!(lost, 0);
            assert_eq!(budget.remaining, remaining);
            assert!(rows[0].overflow && rows[0].ended && !rows[0].complete);
        }
    }
    #[test]
    fn paired_tail_counter_overflow_is_incomplete_and_does_not_spend_work() {
        for counter in 0..4 {
            let function = fixture();
            let budget = ReplayBudget::unobserved();
            let remaining = budget.remaining;
            reset();
            let guard = begin(&function, &budget, false);
            TRACE.with(|trace| {
                let mut trace = trace.borrow_mut();
                match counter {
                    0 => trace.rows[0].mask_checks = usize::MAX,
                    1 => trace.rows[0].tail_marks = usize::MAX,
                    2 => trace.rows[0].fused_turns = usize::MAX,
                    _ => trace.rows[0].tail_work = usize::MAX,
                }
            });
            match counter {
                0 => guard.mask_checks(),
                1 => guard.tail_marks(),
                2 => guard.fused_turns(),
                _ => guard.tail_work(),
            }
            guard.success();
            drop(guard);
            let (rows, lost) = finish();
            assert_eq!(lost, 0);
            assert_eq!(budget.remaining, remaining);
            assert!(rows[0].overflow && rows[0].ended && !rows[0].complete);
        }
    }
}

// Sole owning-test observation scope, outside compiler ledgers and allocation
// windows. It prints after every measured value/guard has dropped, including
// unwind, without converting an incomplete refused prefix into a full result.
pub(in crate::cleanup_plan::replay) struct Scope {
    bytes: usize,
    outcome: Cell<Option<(bool, bool)>>,
}
impl Scope {
    pub(in crate::cleanup_plan::replay) fn whole_caller(bytes: usize) -> Self {
        reset();
        Self {
            bytes,
            outcome: Cell::new(None),
        }
    }
    pub(in crate::cleanup_plan::replay) fn observed(&self, success: bool, refused: bool) {
        self.outcome.set(Some((success, refused)));
    }
}
impl Drop for Scope {
    fn drop(&mut self) {
        let (rows, lost) = finish();
        let unwind = std::thread::panicking();
        let sum = |field: fn(&Row) -> usize| {
            rows.iter()
                .try_fold(0usize, |n, row| n.checked_add(field(row)))
        };
        let totals = [
            sum(|r| r.pairs),
            sum(|r| r.rounds),
            sum(|r| r.pair_matches_admitted),
            sum(|r| r.value_steps),
            sum(|r| r.value_tails),
            sum(|r| r.steps),
            sum(|r| r.loads),
            sum(|r| r.identity_detectors),
            sum(|r| r.identity_hits),
            sum(|r| r.identity_observations),
            sum(|r| r.identity_work),
            sum(|r| r.identity_captures),
            sum(|r| r.identity_extent_captures),
            sum(|r| r.identity_probes),
            sum(|r| r.identity_bodies),
            sum(|r| r.identity_empty_probes),
            sum(|r| r.mask_checks),
            sum(|r| r.tail_marks),
            sum(|r| r.fused_turns),
            sum(|r| r.tail_work),
            sum(|r| r.pair_left_steps),
            sum(|r| r.pair_right_steps),
            sum(|r| r.pair_left_tails),
            sum(|r| r.pair_right_tails),
            sum(|r| r.pair_receivers),
            sum(|r| r.empty_selectors),
            sum(|r| r.empty_selector_skips),
        ];
        eprintln!(
            "OPT487 reached borrowed-right step={:?} tail={:?} content={:?}; partial counts only, no shipping net claim",
            sum(|r| r.pair_right_steps),
            sum(|r| r.pair_right_tails),
            sum(|r| r.pair_receivers)
        );
        let total_overflow = totals.iter().any(Option::is_none);
        let complete = self
            .outcome
            .get()
            .is_some_and(|(ok, refused)| ok && !refused)
            && !unwind
            && lost == 0
            && !total_overflow
            && rows
                .iter()
                .all(|row| row.complete && !row.old && row.owner_admitted);
        eprintln!(
            "OPT888 whole-caller current_candidate=true unqualified=true bytes={} outcome={:?} unwind={} complete={} partial_prefix={} invocations_reached={} owner_banks_admitted={} pairs_prepared={:?} rounds_reached={:?} match_banks_admitted={:?} step_banks_admitted={:?} tail_banks_admitted={:?} prepared_steps={:?} prepared_loads={:?} total_overflow={} lost={} row_capacity=128; complete-only fused component equation current+tuple*N+2Option*(STEP+TAIL)=old886+5HEADER*M+2reference*N; flat/retained-tail/prepared-link components billed separately; no whole-shipping net claim",
            self.bytes,
            self.outcome.get(),
            unwind,
            complete,
            !complete,
            rows.len(),
            rows.iter().filter(|r| r.owner_admitted).count(),
            totals[0],
            totals[1],
            totals[2],
            totals[3],
            totals[4],
            totals[5],
            totals[6],
            total_overflow,
            lost
        );
        eprintln!(
            "OPT888 typed-invoice tuple={} option_pair={} added_outer_references={} added_cold_headers={}",
            size_of::<(Option<&SkeletonObservation>, Option<&SkeletonObservation>)>(),
            2 * size_of::<Option<&SkeletonObservation>>(),
            NEW_PAIR_REFERENCES,
            paired_value::ADDITIONAL_PRIMITIVES
        );
        // These are already-collected invocation counters, not authority or
        // a net saving inferred from an incomplete refused prefix. In particular,
        // pair/round counts are not the number of identity-replayed observations.
        eprintln!(
            "OPT899 suffix whole-caller complete={} partial_prefix={} M={} G={:?} H={:?} Q={:?} probes_P={:?} admitted_bodies_B={:?} empty_probes={:?} replayed_work={:?} overflow={} lost={}; actual fresh/fresh events and reached suffixes, no segment-distribution inference or LOAD credit",
            complete,
            !complete,
            rows.iter().filter(|r| !r.old && r.owner_admitted).count(),
            totals[9],
            totals[8],
            totals[7],
            totals[13],
            totals[14],
            totals[15],
            totals[10],
            total_overflow,
            lost
        );
        eprintln!(
            "OPT899 suffix-component-only typed-invoice (excludes905 selector and900902 cold/packet additions, first-head removals, pair-link/unit-driver and paired-tail changes) owner_current={} owner_before_identity={} cold_headers={} removed_per_G={} body_per_G={} event_per_Q={} probe={}; complete-only suffix-component delta=G*(removed_per_G-body_per_G)-M*(owner_current-owner_before_identity+cold_headers)-Q*event_per_Q-probes*probe; reachable primitive-site delta separate",
            size_of::<ComparisonOwner<'static>>(),
            size_of::<before_identity_prefix::ComparisonOwner<'static>>(),
            shared_prefix::OWNER,
            PAIR_MATCH + 2 * (paired_value::STEP + paired_value::EMPTY),
            shared_prefix::REPLAY,
            shared_prefix::DETECTOR,
            shared_prefix::PROBE
        );
        for row in rows {
            let function = std::str::from_utf8(&row.function[..row.function_len])
                .unwrap_or("<truncated UTF-8>");
            eprintln!(
                "OPT888 row={} generation={} serial={} function={:?} old={} payer={} selected={} owner_admitted={} pairs={} rounds={} match_admitted={} STEP={} TAIL={} prepared_steps={} prepared_loads={} complete={} ended={} unwound={} overflow={} truncated={}",
                row.invocation,
                row.generation,
                row.serial,
                function,
                row.old,
                row.payer,
                row.selected,
                row.owner_admitted,
                row.pairs,
                row.rounds,
                row.pair_matches_admitted,
                row.value_steps,
                row.value_tails,
                row.steps,
                row.loads,
                row.complete,
                row.ended,
                row.unwound,
                row.overflow,
                row.truncated
            );
            eprintln!(
                "PLAN paired-tail row={} mask_admissions={} mark_admissions={} turn_admissions={} work_reached={} complete={} ended={} unwound={} overflow={}; reached prefix only; no whole-app gain claim",
                row.invocation,
                row.mask_checks,
                row.tail_marks,
                row.fused_turns,
                row.tail_work,
                row.complete,
                row.ended,
                row.unwound,
                row.overflow
            );
            eprintln!(
                "OPT905 row={} selector_admitted={} selector_skipped={} old_empty48_probes={} retained_nonempty_probes={} published_runs={} complete={} ended={} unwound={} overflow={}; actual admissions, prefix not net; selector1 is not old PROBE48",
                row.invocation,
                row.empty_selectors,
                row.empty_selector_skips,
                row.identity_empty_probes,
                row.identity_probes,
                row.shared_publishes,
                row.complete,
                row.ended,
                row.unwound,
                row.overflow
            );
            eprintln!(
                "OPT906 row={} boundary_body_admitted={} current_shipping_body_tariff={} right_rest_type_width={} complete={} ended={} unwound={} overflow={}; current tariff is not an old-generation invoice; component comparison requires matched complete caller, not prefix extrapolation",
                row.invocation,
                row.shared_boundaries,
                super::certified_heads::BOUNDARY_BODY,
                size_of::<&[Rc<SkeletonObservation>]>(),
                row.complete,
                row.ended,
                row.unwound,
                row.overflow
            );
            eprintln!(
                "OPT900902 row={} BASE={} POP={} VIEW_HEAD={} HEAD={} RHS={} FIRST={} Q={} G={} H={} P={} B={} complete={} ended={} unwound={} overflow={} truncated={}; reached-prefix packet admissions only",
                row.invocation,
                row.certified_bases,
                row.certified_pops,
                row.certified_views,
                row.certified_heads,
                row.certified_rhs_assignments,
                row.certified_firsts,
                row.identity_detectors,
                row.identity_observations,
                row.identity_hits,
                row.identity_probes,
                row.identity_bodies,
                row.complete,
                row.ended,
                row.unwound,
                row.overflow,
                row.truncated
            );
            eprintln!(
                "PLAN main7e-composite row={} boundary_checks={} left_empty={} right_empty={} links={} queries={} body_admitted={} boundary_complete={} run_capture={} turns={} run_publish={}; actual reached physical packets, no future work credit",
                row.invocation,
                row.shared_checks,
                row.shared_metadata,
                row.shared_empty_checks,
                row.shared_links,
                row.shared_queries,
                row.shared_boundaries,
                row.shared_boundary_complete,
                row.shared_runs,
                row.shared_turns,
                row.shared_publishes
            );
            eprintln!(
                "OPT899 row={} generation={} serial={} function={:?} old={} owner_admitted={} K={} hits={} detectors_D={} captures_C={} extent_captures_E={} replayed_work={} complete={} ended={} unwound={} overflow={} truncated={}",
                row.invocation,
                row.generation,
                row.serial,
                function,
                row.old,
                row.owner_admitted,
                row.identity_observations,
                row.identity_hits,
                row.identity_detectors,
                row.identity_captures,
                row.identity_extent_captures,
                row.identity_work,
                row.complete,
                row.ended,
                row.unwound,
                row.overflow,
                row.truncated
            );
        }
    }
}
