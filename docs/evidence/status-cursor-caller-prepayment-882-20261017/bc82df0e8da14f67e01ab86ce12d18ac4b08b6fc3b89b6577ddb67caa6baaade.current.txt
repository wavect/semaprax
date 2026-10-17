//! Actual independent pre01527 layouts/fees, beside the funded shipping cursor.
use super::*;
use crate::bounded_output::{ReservationRefusal, with_limit_usage_refusal};
use crate::test_allocation_observer::{Metrics, current_metrics, measure};
use std::fmt::Write as _;

#[derive(Debug, PartialEq, Eq)]
struct Failure {
    bytes: [u8; 4096],
    used: usize,
}
impl std::fmt::Write for Failure {
    fn write_str(&mut self, text: &str) -> std::fmt::Result {
        let end = self.used.checked_add(text.len()).ok_or(std::fmt::Error)?;
        if end > self.bytes.len() {
            return Err(std::fmt::Error);
        }
        self.bytes[self.used..end].copy_from_slice(text.as_bytes());
        self.used = end;
        Ok(())
    }
}
fn failure(error: &Diagnostic) -> Failure {
    let mut result = Failure {
        bytes: [0; 4096],
        used: 0,
    };
    // Derived Diagnostic Debug includes all six fields, without heap copying.
    write!(&mut result, "{error:?}").unwrap();
    result
}
fn function() -> ResolvedFunction {
    let mut program = crate::hir::resolve(&crate::check(
        "module prefix.status; @id(\"status.f\") fn f(x:i64)->i64{x+1} @id(\"status.main\") fn main()->i64{0}",
        "status-prefix.spx").unwrap()).unwrap();
    let index = program
        .functions
        .iter()
        .position(|f| f.id.as_str() == "status.f")
        .unwrap();
    let mut function = program.functions.swap_remove(index);
    function.cleanup_plan.status_sources = (0..512)
        .map(|i| StatusSource {
            id: StatusSourceId {
                expression: ExpressionId::from_owned(format!("opaque.λ\0.{:04}", i * 197 % 512)),
                lane: StatusLane::OperationFailure,
            },
            producer: StatusProducer::ContractFalse {
                phase: crate::cleanup_plan::ContractPhase::Requires,
                ordinal: 0,
            },
        })
        .collect();
    function
}

#[test]
fn complete_pre01527_layouts_and_fees_preserve_every_status_key_and_tree_link() {
    let function = function();
    let ((), refused, _, first) = with_limit_usage_refusal(usize::MAX, || {
        let mut old_budget = ReplayBudget::new();
        let mut new_budget = ReplayBudget::new();
        let mut old =
            Directory::new(&function, &mut old_budget, "status cursor genuine baseline").unwrap();
        let mut current = super::super::Directory::new(
            &function,
            &mut new_budget,
            "status cursor genuine baseline",
        )
        .unwrap();
        let mut before = old.held_prefix(&function, &mut old_budget).unwrap();
        let mut after = current.held_prefix(&function, &mut new_budget).unwrap();
        for source in &function.cleanup_plan.status_sources {
            assert_eq!(
                before.insert(&function, &source.id, &mut old_budget),
                after.insert(&function, &source.id, &mut new_budget)
            );
        }
        drop(before);
        drop(after);
        assert_eq!(
            (old.root, old.first, old.len()),
            (current.root, current.first, current.len())
        );
        assert!(old.balanced() && current.balanced());
        assert!(old.keys().eq(current.keys()));
        for (a, b) in old.nodes.iter().zip(&current.nodes) {
            assert!(std::ptr::eq(a.key, b.key));
            assert_eq!(
                (a.left, a.right, a.parent, a.next, a.height),
                (b.left, b.right, b.parent, b.next, b.height)
            );
        }
        eprintln!(
            "status-cursor REAL-pre01527 work old={} current={}",
            MAX_REPLAY_WORK_UNITS - old_budget.remaining,
            MAX_REPLAY_WORK_UNITS - new_budget.remaining
        );
    });
    assert!(!refused && first.is_none());
}

type Observed = (
    (Result<(), Failure>, usize, Metrics),
    bool,
    usize,
    Option<ReservationRefusal>,
);
fn observe(
    function: &ResolvedFunction,
    old: bool,
    available: usize,
    floor: usize,
    work: usize,
) -> Observed {
    let (((result, spent_work), metrics), refused, paid, first) =
        with_limit_usage_refusal(available + floor, || {
            assert!(crate::bounded_output::set_active_floor(floor));
            measure(|| {
                let mut budget = ReplayBudget::new();
                budget.remaining = work;
                let result = (|| {
                    if old {
                        // These complete old types have no cursor field and never
                        // allocate-and-remove a new cursor to inflate a baseline.
                        let mut directory =
                            Directory::new(function, &mut budget, "status cursor real receipt")?;
                        let mut held = directory.held_prefix(function, &mut budget)?;
                        for source in &function.cleanup_plan.status_sources {
                            assert!(held.insert(function, &source.id, &mut budget)?);
                        }
                        drop(held);
                        assert_eq!(directory.len(), 512);
                        assert!(directory.balanced());
                    } else {
                        let mut directory = super::super::Directory::new(
                            function,
                            &mut budget,
                            "status cursor real receipt",
                        )?;
                        let mut held = directory.held_prefix(function, &mut budget)?;
                        for source in &function.cleanup_plan.status_sources {
                            assert!(held.insert(function, &source.id, &mut budget)?);
                        }
                        drop(held);
                        assert_eq!(directory.len(), 512);
                        assert!(directory.balanced());
                    }
                    Ok::<_, Diagnostic>(())
                })();
                let snapshot = result.as_ref().map(|_| ()).map_err(failure);
                drop(result);
                if snapshot.is_err() {
                    let allocated = current_metrics().allocated;
                    let remaining = budget.remaining;
                    let retry = budget
                        .reserve_bytes(function, 0, "sticky cursor actual caller")
                        .unwrap_err();
                    assert_eq!(retry, Diagnostic::io("SPX-H006", String::new()));
                    drop(retry);
                    assert_eq!(current_metrics().allocated, allocated);
                    assert_eq!(budget.remaining, remaining);
                }
                let spent_work = work - budget.remaining;
                drop(budget);
                (snapshot, spent_work)
            })
        });
    ((result, spent_work, metrics), refused, paid, first)
}
fn balanced(metrics: &Metrics, paid: usize) {
    assert!(!metrics.invalid && metrics.live == 0 && metrics.allocated == metrics.freed);
    assert!(metrics.allocated <= paid && metrics.highwater <= paid);
}

#[test]
fn actual_caller_prepaid_cursor_and_complete_old_producer_keep_receipts_floors_and_whole_drop() {
    let function = function();
    for old in [true, false] {
        let ((result, work, metrics), refused, exact, first) =
            observe(&function, old, usize::MAX, 0, MAX_REPLAY_WORK_UNITS);
        assert!(result.is_ok() && !refused && first.is_none());
        balanced(&metrics, exact);
        eprintln!(
            "status-cursor REAL-pre01527 old={old} paid={exact} work={work} allocated={} peak={} freed={} live={}",
            metrics.allocated, metrics.highwater, metrics.freed, metrics.live
        );
        for floor in [0, 37] {
            for available in [0, exact - 1, exact] {
                let ((result, _, metrics), refused, spent, first) =
                    observe(&function, old, available, floor, MAX_REPLAY_WORK_UNITS);
                assert_eq!(result.is_ok(), available == exact);
                assert_eq!(refused, available != exact);
                assert_eq!(first.is_some(), refused);
                if let Some(first) = first {
                    assert_eq!(first.floor, floor);
                }
                balanced(&metrics, spent);
                if available == 0 {
                    assert_eq!(metrics.allocated, 0);
                }
            }
        }
        // Follow actual refused receiver boundaries, not a guessed constructor
        // price. Each bank amount comes from the live reservation receipt.
        let mut end = 0;
        for _ in 0..6 {
            let ((result, _, metrics), refused, spent, first) =
                observe(&function, old, end, 0, MAX_REPLAY_WORK_UNITS);
            assert!(result.is_err() && refused);
            balanced(&metrics, spent);
            let first = first.unwrap();
            assert!(first.requested > first.remaining);
            let next = end.checked_add(first.requested - first.remaining).unwrap();
            assert!(next > end);
            end = next;
        }
        let exact_work = work;
        for work in [0, 1, exact_work - 1, exact_work] {
            let ((result, spent_work, metrics), _, paid, _) =
                observe(&function, old, usize::MAX, 0, work);
            assert_eq!(result.is_ok(), work == exact_work);
            assert!(spent_work <= work);
            balanced(&metrics, paid);
        }
    }
}

#[test]
fn foreign_function_refuses_before_new_cursor_dispatch_and_keeps_all_six_fields() {
    let function = function();
    let foreign = function.clone();
    let mut signatures = Vec::new();
    for old in [true, false] {
        let ((signature, metrics), refused, _, first) =
            with_limit_usage_refusal(usize::MAX, || {
                measure(|| {
                    let mut budget = ReplayBudget::new();
                    let phase = "status cursor foreign caller";
                    let key = &function.cleanup_plan.status_sources[0].id;
                    let error = if old {
                        let mut directory = Directory::new(&function, &mut budget, phase).unwrap();
                        let mut held = directory.held_prefix(&function, &mut budget).unwrap();
                        let remaining = budget.remaining;
                        let error = held.insert(&foreign, key, &mut budget).unwrap_err();
                        assert_eq!(budget.remaining, remaining);
                        error
                    } else {
                        let mut directory =
                            super::super::Directory::new(&function, &mut budget, phase).unwrap();
                        let mut held = directory.held_prefix(&function, &mut budget).unwrap();
                        let remaining = budget.remaining;
                        let error = held.insert(&foreign, key, &mut budget).unwrap_err();
                        assert_eq!(budget.remaining, remaining);
                        error
                    };
                    let signature = failure(&error);
                    drop(error);
                    drop(budget);
                    signature
                })
            });
        assert!(!refused && first.is_none());
        assert!(!metrics.invalid && metrics.live == 0 && metrics.allocated == metrics.freed);
        signatures.push(signature);
    }
    assert_eq!(signatures[0], signatures[1]);
}
