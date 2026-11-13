//! Actual requested layouts and FIFO contents, independent of growth arithmetic.
use super::*;
use crate::test_allocation_observer;

#[test]
fn first_refusal_names_the_requesting_receiver_through_reservation_wrappers() {
    let function = function();
    for operation in 0..6 {
        let mut budget = ReplayBudget::new();
        budget.payer = AllocationPayer::ActiveBuilder;
        budget.prepare_builder(&function).unwrap();
        budget.prepare_clone_cursor(&function).unwrap();
        let payload = vec![7u64];
        let mut vector = Vec::<u64>::new();
        let mut queue = VecDeque::<u64>::new();
        let ((begin, end), refused, used, receipt, site) =
            crate::bounded_output::with_limit_usage_refusal_site(0, || {
                let begin = line!();
                let result = match operation {
                    0 => budget.reserve_bytes(&function, 8, "direct caller"),
                    1 => budget.reserve_array::<u64>(&function, 1, "array caller"),
                    2 => budget.reserve_clone(&function, &payload, "clone caller"),
                    3 => budget.reserve_btree_node::<u64, ()>(&function, "tree caller"),
                    4 => budget.reserve_vec(&function, &mut vector, 1, "vector caller"),
                    5 => budget.reserve_deque(&function, &mut queue, 1, "queue caller"),
                    _ => unreachable!(),
                };
                let end = line!();
                assert_eq!(result.unwrap_err().code, "SPX-H006");
                assert!(budget.reserve_bytes(&function, 99, "later caller").is_err());
                (begin, end)
            });
        assert!(refused);
        assert_eq!(used, 0);
        assert!(receipt.unwrap().requested > 0);
        assert_eq!(receipt.unwrap().remaining, 0);
        let site = site.unwrap();
        assert_eq!(site.file(), file!());
        assert!(site.line() > begin && site.line() < end);
        assert_eq!(vector.capacity(), 0);
        assert_eq!(queue.capacity(), 0);
    }
}

const EXPECTED: usize = 255 * std::mem::size_of::<u64>();
fn function() -> ResolvedFunction {
    let parsed = crate::check(
        "module queue.growth;\n@id(\"main\") fn main()->i64 {0}",
        "queue-growth.spx",
    )
    .unwrap();
    crate::hir::resolve(&parsed)
        .unwrap()
        .functions
        .into_iter()
        .find(|function| function.id.as_str() == "main")
        .unwrap()
}
fn budget(bytes: usize) -> ReplayBudget {
    let mut budget = ReplayBudget::new();
    budget.admission = AdmissionMode::SelectedRetained;
    budget.payer = AllocationPayer::SelectedReservation;
    budget.physical = Some(Box::new(physical_storage::PhysicalReservation::new(bytes)));
    budget
}
#[test]
fn geometric_queue_growth_observes_full_replacements_and_wrapped_fifo_spare() {
    let function = function();
    let mut budget = budget(EXPECTED);
    let ((charged, observed), refused, used) =
        crate::bounded_output::with_limit_usage(EXPECTED, || {
            test_allocation_observer::measure(|| {
                let mut queue = VecDeque::<u64>::new();
                for value in 0..100 {
                    budget
                        .reserve_deque(&function, &mut queue, 1, "queue witness")
                        .unwrap();
                    queue.push_back(value);
                }
                assert_eq!(queue.capacity(), 128);
                let charged = budget.physical.as_ref().unwrap().charged();
                for value in 0..60 {
                    assert_eq!(queue.pop_front(), Some(value));
                }
                for value in 100..160 {
                    budget
                        .reserve_deque(&function, &mut queue, 1, "wrapped spare")
                        .unwrap();
                    queue.push_back(value);
                }
                assert!(
                    !queue.as_slices().1.is_empty(),
                    "witness must use wrapped storage"
                );
                assert_eq!(budget.physical.as_ref().unwrap().charged(), charged);
                assert_eq!(queue.len(), 100);
                for value in 60..160 {
                    assert_eq!(queue.pop_front(), Some(value));
                }
                assert!(queue.is_empty());
                charged
            })
        });
    assert!(!refused && !observed.invalid);
    assert_eq!(charged, EXPECTED);
    assert_eq!(used, EXPECTED);
    assert_eq!(observed.allocated, EXPECTED);
    // The observer physically keeps the old 64-slot allocation alive while
    // allocating its 128-slot replacement; this is not ledger-derived peak data.
    assert_eq!(observed.highwater, 192 * std::mem::size_of::<u64>());
    assert_eq!(observed.freed, EXPECTED);
    assert_eq!(observed.live, 0);
}
#[test]
fn queue_one_short_refuses_before_growth_and_work_reset_cannot_restore_spare() {
    let function = function();
    let mut budget = budget(EXPECTED - 1);
    let ((capacity, observed), refused, _) =
        crate::bounded_output::with_limit_usage(1024 * 1024, || {
            test_allocation_observer::measure(|| {
                let mut queue = VecDeque::<u64>::new();
                for value in 0..64 {
                    budget
                        .reserve_deque(&function, &mut queue, 1, "before refusal")
                        .unwrap();
                    queue.push_back(value);
                }
                let charged = budget.physical.as_ref().unwrap().charged();
                assert_eq!(
                    budget
                        .reserve_deque(&function, &mut queue, 1, "one-short replacement")
                        .unwrap_err()
                        .code,
                    "SPX-H006"
                );
                assert_eq!(queue.capacity(), 64);
                assert_eq!(queue.len(), 64);
                assert_eq!(budget.physical.as_ref().unwrap().charged(), charged);
                budget.remaining = MAX_REPLAY_WORK_UNITS;
                budget.skeleton_remaining = MAX_REPLAY_WORK_UNITS;
                assert_eq!(queue.pop_front(), Some(0));
                // There is now spare space, but the earlier physical refusal is sticky.
                assert_eq!(
                    budget
                        .reserve_deque(&function, &mut queue, 1, "after work reset")
                        .unwrap_err()
                        .code,
                    "SPX-H006"
                );
                assert_eq!(budget.physical.as_ref().unwrap().charged(), charged);
                for value in 1..64 {
                    assert_eq!(queue.pop_front(), Some(value));
                }
                queue.capacity()
            })
        });
    assert!(!refused && !observed.invalid);
    assert_eq!(capacity, 64);
    // Diagnostics allocate too: retain them in observed totals rather than calling
    // the failed path fully charged by the successful queue-storage bound.
    assert!(observed.allocated >= 127 * std::mem::size_of::<u64>());
    assert!(observed.highwater <= observed.allocated);
    assert_eq!(observed.live, 0);
}
#[test]
fn ordinary_queue_reservation_keeps_legacy_allocator_route_unchanged() {
    let function = function();
    let mut budget = ReplayBudget::new();
    let mut queue = VecDeque::<u64>::new();
    let (result, observed) = test_allocation_observer::measure(|| {
        budget.reserve_deque(&function, &mut queue, usize::MAX, "ordinary unchanged")
    });
    result.unwrap();
    assert_eq!(queue.capacity(), 0);
    assert_eq!(observed.allocated, 0);
    assert!(!observed.invalid);
    assert!(budget.physical.is_none());
}
