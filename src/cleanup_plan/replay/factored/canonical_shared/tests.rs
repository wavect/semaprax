//! Independent full-old algebra and actual allocation/drop controls.
use super::Canonicalizer;
use super::before_inline_weighted::{own_weighted, share_weighted, weighted_successor};
use super::super::{
    Automaton, BTreeMap, Diagnostic, Label, MAX_REPLAY_WORK_UNITS, Rc, ReplayBudget,
    ResolvedFunction, SkeletonTerminal, Weighted, canonical_before, physical_clone,
};

fn fixture() -> ResolvedFunction {
    super::super::lexical_plan_tests::fixture(super::super::lexical_plan_tests::WHILE).1
}

pub(super) fn automaton(function: &ResolvedFunction) -> (Automaton, usize, Vec<usize>) {
    let mut budget = ReplayBudget::new();
    let mut automaton = Automaton::default();
    let sink = automaton.node(function, &mut budget).unwrap();
    let mut roots = Vec::new();
    for terminal in [
        SkeletonTerminal::Success,
        SkeletonTerminal::Failure,
        SkeletonTerminal::LoopLatch,
    ] {
        roots.push(
            automaton
                .end(function, &mut budget, sink, terminal)
                .unwrap(),
        );
    }
    let duplicate = automaton.node(function, &mut budget).unwrap();
    for _ in 0..2 {
        automaton
            .epsilon(function, &mut budget, duplicate, roots[0])
            .unwrap();
    }
    roots.push(duplicate);
    for targets in [[roots[0], roots[1]], [roots[1], roots[0]]] {
        let root = automaton.node(function, &mut budget).unwrap();
        for target in targets {
            automaton
                .epsilon(function, &mut budget, root, target)
                .unwrap();
        }
        roots.push(root);
    }
    let cycle = automaton.node(function, &mut budget).unwrap();
    automaton
        .movement(
            function,
            &mut budget,
            cycle,
            Label::End(SkeletonTerminal::Success),
            cycle,
        )
        .unwrap();
    roots.push(cycle);
    (automaton, sink, roots)
}

#[test]
fn full_old_receiver_preserves_ids_signatures_work_duplicates_overflow_and_cycle() {
    let function = fixture();
    let (automaton, sink, roots) = automaton(&function);
    let mut old = canonical_before::Receiver::new(&automaton, sink);
    let mut current = Canonicalizer::new(&automaton, sink);
    let mut old_budget = ReplayBudget::new();
    let mut budget = ReplayBudget::new();
    let mut ids = Vec::new();
    for root in &roots[..6] {
        let expected = old
            .id(&function, vec![(*root, 1)], &mut old_budget)
            .unwrap();
        let actual = current
            .id(&function, vec![(*root, 1)], &mut budget)
            .unwrap();
        assert_eq!(actual, expected);
        assert_eq!(budget.remaining, old_budget.remaining);
        assert_eq!(budget.skeleton_remaining, old_budget.skeleton_remaining);
        assert_eq!(&current.ids, old.signatures());
        assert_eq!(current.states.len(), old.states().len());
        for ((actual, actual_id), (state, id)) in current.states.iter().zip(old.states()) {
            assert_eq!(actual.as_slice(), state.as_slice());
            assert_eq!(actual_id, id);
        }
        ids.push(actual);
    }
    assert_ne!(ids[0], ids[3]);
    assert_ne!(ids[0], ids[1]);
    assert_ne!(ids[1], ids[2]);
    assert_eq!(ids[4], ids[5]);
    let expected = old
        .id(&function, vec![(roots[3], u64::MAX)], &mut old_budget)
        .unwrap();
    assert_eq!(
        current
            .id(&function, vec![(roots[3], u64::MAX)], &mut budget)
            .unwrap(),
        expected
    );
    assert_eq!(expected, None);
    assert_eq!(budget.remaining, old_budget.remaining);
    let expected = old
        .id(&function, vec![(roots[6], 1)], &mut old_budget)
        .unwrap_err();
    let actual = current
        .id(&function, vec![(roots[6], 1)], &mut budget)
        .unwrap_err();
    assert_eq!(actual, expected);
    assert_eq!(budget.remaining, old_budget.remaining);
}

fn probe(
    function: &ResolvedFunction,
    automaton: &Automaton,
    sink: usize,
    roots: &[usize],
    limit: usize,
) -> (Result<usize, ()>, crate::test_allocation_observer::Metrics) {
    let mut budget = ReplayBudget::with_physical(function, limit).unwrap();
    let before = budget.physical.as_ref().unwrap().charged();
    crate::test_allocation_observer::measure(|| {
        let mut current = Canonicalizer::new(automaton, sink);
        let result = (|| {
            for root in roots {
                budget.reserve_array::<(usize, u64)>(function, 1, "shared test root")?;
                current.id(function, vec![(*root, 1)], &mut budget)?;
            }
            Ok::<_, Diagnostic>(budget.physical.as_ref().unwrap().charged() - before)
        })();
        drop(current);
        // Drop refusal storage inside observation as well as every memo/frame.
        result.map_err(|error| {
            assert_eq!(error.code, "SPX-H006");
            drop(error);
        })
    })
}

#[test]
fn shared_full_backing_exact_short_refusal_and_error_drop_are_physical() {
    let function = fixture();
    let (automaton, sink, roots) = automaton(&function);
    let sequence = [roots[4], roots[5], roots[3], roots[4]];
    let (result, observed) = probe(&function, &automaton, sink, &sequence, 1 << 24);
    let charged = result.unwrap();
    assert!(!observed.invalid && observed.allocated <= charged && observed.highwater <= charged);
    assert_eq!(observed.live, 0);
    assert_eq!(observed.allocated, observed.freed);
    let baseline = ReplayBudget::with_physical(&function, 1 << 24).unwrap();
    let admission = baseline.physical.as_ref().unwrap().charged();
    let (exact, observed) = probe(&function, &automaton, sink, &sequence, admission + charged);
    assert_eq!(exact.unwrap(), charged);
    assert!(!observed.invalid && observed.allocated <= charged && observed.highwater <= charged);
    assert_eq!(observed.live, 0);
    for limit in [admission, admission + charged - 1] {
        let (result, observed) = probe(&function, &automaton, sink, &sequence, limit);
        assert!(result.is_err());
        assert!(!observed.invalid);
        assert_eq!(observed.live, 0);
        assert_eq!(observed.allocated, observed.freed);
    }
    let (cycle, observed) = probe(&function, &automaton, sink, &roots[6..], 1 << 24);
    assert!(cycle.is_err());
    assert!(!observed.invalid && observed.live == 0);
    assert_eq!(observed.allocated, observed.freed);
}

#[test]
fn immutable_shares_preserve_copy_work_and_zero_requests_cannot_clear_refusal() {
    let function = fixture();
    let mut budget = ReplayBudget::with_physical(&function, 1 << 20).unwrap();
    budget
        .reserve_array::<(usize, u64)>(&function, 2, "share test backing")
        .unwrap();
    let state = own_weighted(&function, &mut budget, vec![(0, 1), (1, 2)]).unwrap();
    let before = budget.physical.as_ref().unwrap().charged();
    let work = budget.remaining;
    let shared = share_weighted(&function, &mut budget, &state).unwrap();
    assert!(Rc::ptr_eq(&state, &shared));
    assert_eq!(budget.remaining, work - state.len() - 1);
    assert_eq!(budget.physical.as_ref().unwrap().charged(), before);
    budget.physical.as_mut().unwrap().reject();
    assert!(share_weighted(&function, &mut budget, &state).is_err());
    assert!(budget.reserve_bytes(&function, 0, "refused zero").is_err());
    assert_eq!(budget.physical.as_ref().unwrap().charged(), before);
}

thread_local! {
    static ACTUAL_ENTRIES: std::cell::Cell<Option<(usize, usize)>> = const { std::cell::Cell::new(None) };
}
struct ActualEntries(Option<(usize, usize)>);
impl ActualEntries {
    fn open(limit: usize) -> Self {
        Self(ACTUAL_ENTRIES.with(|state| state.replace(Some((limit, 0)))))
    }
    fn calls(&self) -> usize {
        ACTUAL_ENTRIES.with(|state| state.get().unwrap().1)
    }
}
impl Drop for ActualEntries {
    fn drop(&mut self) {
        ACTUAL_ENTRIES.with(|state| state.set(self.0));
    }
}
pub(super) fn note_actual_entry(budget: &mut ReplayBudget) {
    ACTUAL_ENTRIES.with(|state| {
        if let Some((limit, calls)) = state.get() {
            if calls == 0 {
                budget.remaining = limit;
            }
            state.set(Some((limit, calls + 1)));
        }
    });
}

#[test]
fn exact_short_and_zero_work_match_the_complete_old_receiver() {
    let function = fixture();
    let (automaton, sink, roots) = automaton(&function);
    let mut full = ReplayBudget::unobserved();
    Canonicalizer::new(&automaton, sink)
        .id(&function, vec![(roots[4], 1)], &mut full)
        .unwrap();
    let spent = MAX_REPLAY_WORK_UNITS - full.remaining;
    for limit in [spent, spent - 1, 0] {
        let mut old_budget = ReplayBudget::unobserved();
        old_budget.remaining = limit;
        let mut budget = ReplayBudget::unobserved();
        budget.remaining = limit;
        let old = canonical_before::Receiver::new(&automaton, sink).id(
            &function,
            vec![(roots[4], 1)],
            &mut old_budget,
        );
        let current =
            Canonicalizer::new(&automaton, sink).id(&function, vec![(roots[4], 1)], &mut budget);
        assert_eq!(current, old);
        assert_eq!(budget.remaining, old_budget.remaining);
        if limit == spent {
            assert!(current.unwrap().is_some());
            assert_eq!(budget.remaining, 0);
        } else {
            assert!(current.is_err());
            // These deliberately detached LegacyUnobserved ledgers retain
            // the complete old counter policy: Err selects the receiver's
            // result, but does not poison a raw zero-unit counter probe.
            let remaining = budget.remaining;
            let old_zero = old_budget.charge(&function, 0, "legacy work-state probe");
            let zero = budget.charge(&function, 0, "legacy work-state probe");
            assert_eq!(zero, old_zero);
            assert!(zero.is_ok());
            assert_eq!(budget.remaining, remaining);
            assert_eq!(budget.remaining, old_budget.remaining);
            assert_eq!(budget.refusal_spent, old_budget.refusal_spent);
            assert!(!budget.refusal_spent);
        }
    }

    // Exercise the actual ordinary comparison, including its real root
    // reservations and first id() `?` boundary. The observer gives only that
    // first real canonical entry a short test allowance; production ceilings
    // and prior automaton-construction work are not changed.
    let (program, function) =
        super::super::lexical_plan_tests::fixture(super::super::lexical_plan_tests::WHILE);
    for limit in [0, 1] {
        let observation = ActualEntries::open(limit);
        let mut budget = ReplayBudget::unobserved();
        let result = super::super::compare(&program, &function, &mut budget);
        assert!(result.is_err());
        assert_eq!(
            observation.calls(),
            1,
            "actual comparison must propagate the first canonical Err before the plan root"
        );
    }
}

#[test]
fn overflow_returns_no_proof_and_drops_all_paid_backing() {
    let function = fixture();
    let (automaton, sink, roots) = automaton(&function);
    let mut budget = ReplayBudget::with_physical(&function, 1 << 24).unwrap();
    let before = budget.physical.as_ref().unwrap().charged();
    let (charged, observed) = crate::test_allocation_observer::measure(|| {
        let mut current = Canonicalizer::new(&automaton, sink);
        budget
            .reserve_array::<(usize, u64)>(&function, 1, "overflow test root")
            .unwrap();
        assert_eq!(
            current
                .id(&function, vec![(roots[3], u64::MAX)], &mut budget)
                .unwrap(),
            None
        );
        drop(current);
        budget.physical.as_ref().unwrap().charged() - before
    });
    assert!(!observed.invalid && observed.allocated <= charged && observed.highwater <= charged);
    assert_eq!(observed.live, 0);
    assert_eq!(observed.allocated, observed.freed);
}

#[test]
fn exact_small_weighted_successor_capacity_and_requested_backing_are_paid_before_allocation() {
    let function = fixture();
    let admission = ReplayBudget::with_physical(&function, 1 << 20)
        .unwrap()
        .physical
        .as_ref()
        .unwrap()
        .charged();
    for width in 0..=4 {
        let vector = width * std::mem::size_of::<(usize, u64)>();
        let header = physical_clone::rc_backing_bytes::<Weighted>();
        let expected = vector + header;
        let make_targets = || {
            (0..width)
                .map(|n| (n, n as u64 + 1))
                .collect::<BTreeMap<_, _>>()
        };
        let mut budget = ReplayBudget::with_physical(&function, admission + expected).unwrap();
        let targets = make_targets();
        let ((capacity, length, work), observed) = crate::test_allocation_observer::measure(|| {
            let work = budget.remaining;
            let state = weighted_successor(&function, &mut budget, targets).unwrap();
            let capacity = state.capacity();
            let length = state.len();
            for (n, entry) in state.iter().enumerate() {
                assert_eq!(*entry, (n, n as u64 + 1));
            }
            drop(state);
            (capacity, length, work)
        });
        assert_eq!((capacity, length), (width, width));
        assert_eq!(
            budget.remaining, work,
            "producer adds no copy-work discount or debit"
        );
        assert_eq!(
            budget.physical.as_ref().unwrap().charged(),
            admission + expected
        );
        assert!(!observed.invalid);
        assert_eq!(
            observed.allocated, expected,
            "actual Vec and Rc requested Layouts"
        );
        assert_eq!(observed.highwater, expected);
        assert_eq!(observed.live, 0);
        assert_eq!(observed.allocated, observed.freed);
        assert_eq!(observed.reallocations, 0);

        // Refusal at the vector acquisition must precede both vector and Rc.
        // Width zero has no vector backing, so its first allocation is Rc.
        let short = if width == 0 { header - 1 } else { vector - 1 };
        for available in [0, short] {
            let mut budget = ReplayBudget::with_physical(&function, admission + available).unwrap();
            let targets = make_targets();
            let ((), observed) = crate::test_allocation_observer::measure(|| {
                let error = weighted_successor(&function, &mut budget, targets).unwrap_err();
                assert_eq!(error.code, "SPX-H006");
                drop(error);
                // Even an empty successor cannot clear the selected refusal.
                let error =
                    weighted_successor(&function, &mut budget, BTreeMap::new()).unwrap_err();
                assert_eq!(error.code, "SPX-H006");
                drop(error);
            });
            assert!(!observed.invalid);
            assert_eq!(observed.allocated, 0, "refused backing was never acquired");
            assert_eq!(observed.live, 0);
            assert_eq!(budget.physical.as_ref().unwrap().charged(), admission);
        }

        // A one-byte-short Rc acquisition may retain only the already funded
        // exact vector until error Drop; the refused Rc must not allocate.
        let mut budget = ReplayBudget::with_physical(&function, admission + expected - 1).unwrap();
        let targets = make_targets();
        let ((), observed) = crate::test_allocation_observer::measure(|| {
            drop(weighted_successor(&function, &mut budget, targets).unwrap_err());
        });
        assert!(!observed.invalid && observed.live == 0);
        assert_eq!(observed.allocated, vector);
        assert_eq!(observed.freed, vector);
        assert_eq!(
            budget.physical.as_ref().unwrap().charged(),
            admission + vector
        );
    }
}

#[test]
fn exact_small_successor_producer_keeps_complete_old_weighted_algebra_and_work() {
    let function = fixture();
    for width in 0..=4 {
        let mut builder = ReplayBudget::new();
        let mut automaton = Automaton::default();
        let sink = automaton.node(&function, &mut builder).unwrap();
        let root = automaton.node(&function, &mut builder).unwrap();
        for _ in 0..width {
            let target = automaton
                .end(&function, &mut builder, sink, SkeletonTerminal::Failure)
                .unwrap();
            automaton
                .movement(
                    &function,
                    &mut builder,
                    root,
                    Label::End(SkeletonTerminal::Success),
                    target,
                )
                .unwrap();
        }
        let mut current = Canonicalizer::new(&automaton, sink);
        let mut old = canonical_before::Receiver::new(&automaton, sink);
        let mut budget = ReplayBudget::new();
        let mut old_budget = ReplayBudget::new();
        for weight in [1, 2, u64::MAX] {
            assert_eq!(
                current.id(&function, vec![(root, weight)], &mut budget),
                old.id(&function, vec![(root, weight)], &mut old_budget)
            );
            assert_eq!(budget.remaining, old_budget.remaining);
            assert_eq!(budget.skeleton_remaining, old_budget.skeleton_remaining);
            assert_eq!(&current.ids, old.signatures());
        }
    }
}
