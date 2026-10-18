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
    pub pairs: usize,
    pub rounds: usize,
    pub tails: usize,
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
    pub shared_turns: usize,
    pub loads: usize,
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
    pairs: 0,
    rounds: 0,
    tails: 0,
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
    shared_turns: 0,
    loads: 0,
    complete: false,
    ended: false,
    unwound: false,
    overflow: false,
    truncated: false,
};
struct Trace {
    enabled: bool,
    count: usize,
    generation: usize,
    serial: usize,
    rows: [Row; 128],
}
thread_local! {
    static TRACE: RefCell<Trace> = const { RefCell::new(Trace { enabled: false, count: 0, generation: 0, serial: 0, rows: [EMPTY; 128] }) };
    static LOST: Cell<usize> = const { Cell::new(0) };
}
fn lost() {
    LOST.with(|n| n.set(n.get().saturating_add(1)));
}
pub(super) fn reset() {
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
pub(super) struct Guard {
    identity: Option<Identity>,
}
pub(super) fn begin(function: &ResolvedFunction, budget: &ReplayBudget, old: bool) -> Guard {
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
    pub(super) fn pair(&self) {
        self.update(|r| match r.pairs.checked_add(1) {
            Some(n) => r.pairs = n,
            None => r.overflow = true,
        });
    }
    pub(super) fn round(&self) {
        self.update(|r| match r.rounds.checked_add(1) {
            Some(n) => r.rounds = n,
            None => r.overflow = true,
        });
    }
    pub(super) fn step(&self) {
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
    pub(super) fn tail(&self) {
        self.update(|r| match r.tails.checked_add(1) {
            Some(n) => r.tails = n,
            None => r.overflow = true,
        });
    }
    pub(super) fn success(&self) {
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
        for turn in [false, true] {
            let function = fixture();
            let budget = ReplayBudget::unobserved();
            let remaining = budget.remaining;
            reset();
            let guard = begin(&function, &budget, false);
            TRACE.with(|trace| {
                let mut trace = trace.borrow_mut();
                if turn {
                    trace.rows[0].shared_turns = usize::MAX;
                } else {
                    trace.rows[0].shared_runs = usize::MAX;
                }
            });
            if turn {
                guard.shared_turns();
            } else {
                guard.shared_runs();
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
