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

mod targets {
    use super::super::targets::Targets;
    use super::*;

    fn function() -> ResolvedFunction {
        super::super::super::lexical_plan_tests::fixture(
            super::super::super::lexical_plan_tests::WHILE,
        )
        .1
    }

    #[test]
    fn sorted_targets_preserve_zero_duplicate_maximum_and_overflow_weights() {
        let function = function();
        for entries in [
            vec![],
            vec![(7, 0), (2, 3), (7, 4), (0, 1), (2, 5)],
            vec![(usize::MAX, u64::MAX), (0, 0)],
            vec![(1, u64::MAX), (1, 1)],
        ] {
            let run = |paid| {
                let mut budget = ReplayBudget::new();
                let mut targets = Targets::new(paid);
                let mut verdicts = Vec::new();
                for &(node, weight) in &entries {
                    let admitted = targets.add(&function, &mut budget, node, weight).unwrap();
                    verdicts.push(admitted);
                    if !admitted {
                        break;
                    }
                }
                let weights = targets.share(&function, &mut budget).unwrap();
                (verdicts, weights)
            };
            assert_eq!(run(true), run(false));
        }
    }

    #[test]
    fn paid_targets_move_backing_and_keep_exact_short_zero_and_sticky_limits() {
        let function = function();
        let observe = |paid, limit, floor| {
            crate::bounded_output::with_limit_usage_refusal(limit + floor, || {
                assert!(crate::bounded_output::set_active_floor(floor));
                crate::test_allocation_observer::measure(|| {
                    let mut budget = ReplayBudget::new();
                    let result = (|| {
                        let mut targets = Targets::new(paid);
                        for node in (0..64).rev() {
                            assert!(targets.add(&function, &mut budget, node, 2)?);
                            assert!(targets.add(&function, &mut budget, node, 3)?);
                        }
                        let pointer = match &targets {
                            Targets::Paid(weights) => Some(weights.as_ptr()),
                            Targets::Legacy(_) => None,
                        };
                        let shared = targets.share(&function, &mut budget)?;
                        if let Some(pointer) = pointer {
                            assert_eq!(pointer, shared.as_ptr(), "backing moves without copying");
                        }
                        assert_eq!(shared.len(), 64);
                        for (expected, &(node, weight)) in shared.iter().enumerate() {
                            assert_eq!((node, weight), (expected, 5));
                        }
                        drop(shared);
                        Ok::<_, Diagnostic>(())
                    })();
                    let success = result.is_ok();
                    if !success {
                        assert!(budget
                            .reserve_bytes(&function, 0, "sticky targets")
                            .is_err());
                    }
                    drop(result);
                    drop(budget);
                    success
                })
            })
        };
        let ((ok, old), refused, before, first) = observe(false, usize::MAX, 0);
        assert!(ok && !refused && first.is_none());
        let ((ok, new), refused, exact, first) = observe(true, usize::MAX, 0);
        assert!(ok && !refused && first.is_none());
        assert!(exact < before && new.allocated < old.allocated);
        for allocation in [old, new] {
            assert!(!allocation.invalid && allocation.live == 0);
            assert_eq!(allocation.allocated, allocation.freed);
        }
        assert!(new.allocated <= exact && old.allocated <= before);
        for (limit, floor) in [0, 37]
            .into_iter()
            .flat_map(|floor| [0, exact - 1, exact].map(|limit| (limit, floor)))
        {
            let ((ok, allocation), refused, spent, first) = observe(true, limit, floor);
            assert_eq!(ok, limit == exact);
            assert_eq!(refused, limit != exact);
            assert_eq!(first.is_some(), limit != exact);
            if let Some(receipt) = first {
                assert_eq!(receipt.floor, floor);
            }
            assert!(spent <= limit);
            assert!(!allocation.invalid && allocation.live == 0);
            assert_eq!(allocation.allocated, allocation.freed);
            if ok {
                assert_eq!(spent, exact);
            }
        }
    }

    #[test]
    fn target_work_refusal_precedes_growth_and_preserves_existing_weights() {
        let function = function();
        let mut budget = ReplayBudget::new();
        let mut targets = Targets::new(true);
        assert!(targets.add(&function, &mut budget, 3, 7).unwrap());
        budget.remaining = 0;
        assert!(targets.add(&function, &mut budget, 1, 2).is_err());
        let Targets::Paid(weights) = targets else {
            unreachable!()
        };
        assert_eq!(weights, vec![(3, 7)]);
    }

    // Whole predecessors remain byte-bound before every historical inverse.
    use sha2::{Digest, Sha256};

    #[test]
    fn complete_predecessors_and_new_target_sources_are_bound() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
        let evidence = root.join("docs/evidence/factored-flat-targets-v1");
        let proof: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(evidence.join("sources.json")).unwrap())
                .unwrap();
        let digest = |text: &str| {
            format!(
                "{:x}",
                crate::digest_hex::LowerHex(Sha256::digest(text.as_bytes()))
            )
        };
        for record in proof["predecessors"].as_array().unwrap() {
            let before =
                std::fs::read_to_string(evidence.join(record["archive"].as_str().unwrap()))
                    .unwrap();
            let current =
                std::fs::read_to_string(root.join(record["path"].as_str().unwrap())).unwrap();
            let current = crate::source_reflows_merge::main67107_inputs::compile35f_input(
                record["path"].as_str().unwrap(), &current,
            );
            let current = super::super::targets::incoming_input(record["path"].as_str().unwrap(), &current);
            assert_eq!(digest(&before), record["before_sha256"]);
            assert_eq!(digest(&current), record["after_sha256"]);
            let restore = |input: &str| -> Option<String> {
                let mut output = input.to_owned();
                for edit in record["inverse"].as_array().unwrap() {
                    let after = edit["after"].as_str().unwrap();
                    if output.matches(after).count() != 1 {
                        return None;
                    }
                    output = output.replacen(after, edit["before"].as_str().unwrap(), 1);
                }
                Some(output)
            };
            assert_eq!(restore(&current).as_deref(), Some(before.as_str()));
            assert_ne!(
                restore(&(current.clone() + "\n")).as_deref(),
                Some(before.as_str())
            );
            let mut changed = current.clone();
            changed.replace_range(..1, "!");
            assert_ne!(restore(&changed).as_deref(), Some(before.as_str()));
        }
        for record in proof["new_files"].as_array().unwrap() {
            let current =
                std::fs::read_to_string(root.join(record["path"].as_str().unwrap())).unwrap();
            let current = crate::source_reflows_merge::main67107_inputs::compile35f_input(
                record["path"].as_str().unwrap(), &current,
            );
            assert_eq!(digest(&current), record["sha256"]);
            assert_ne!(digest(&(current + "\n")), record["sha256"]);
        }
    }
}

mod successor_labels {
    use super::super::{targets, borrowed_labels as labels};
    use crate::cleanup_plan::replay::SkeletonObservation;
    use crate::hir::ExpressionId;
    use super::super::super::compact_nodes::{Node, Many};
    use super::*;

    fn function() -> ResolvedFunction {
        fixture()
    }

    fn subject(width: usize, overflow: bool) -> (Automaton, BTreeMap<usize, u64>) {
        let mut keys = (0..width)
            .map(|index| {
                Label::Observation(Rc::new(SkeletonObservation::Boolean {
                    expression: ExpressionId::from_owned(format!("labels.é.{index}")),
                    value: index % 2 == 0,
                }))
            })
            .collect::<Vec<_>>();
        if width != 0 {
            for terminal in [
                SkeletonTerminal::Success,
                SkeletonTerminal::Failure,
                SkeletonTerminal::LoopLatch,
            ] {
                keys.push(Label::End(terminal));
            }
        }
        let nodes = (0..4)
            .map(|node| Node::Many(Box::new(Many {
                moves: keys
                    .iter()
                    .rev()
                    .enumerate()
                    .map(|(index, key)| {
                        (key.clone(), if overflow { 0 } else { (index + node) % 5 })
                    })
                    .collect(),
                epsilon: Vec::new(),
            })))
            .collect();
        let weights = [
            (0, if overflow { u64::MAX } else { 0 }),
            (1, 2),
            (2, 3),
            (3, 5),
        ]
        .into_iter()
        .collect();
        (Automaton { nodes }, weights)
    }

    // Complete prior outer producer, with only self.automaton made a parameter.
    #[allow(clippy::type_complexity)]
    fn before_labels(
        function: &ResolvedFunction,
        budget: &mut ReplayBudget,
        automaton: &Automaton,
        closed: &BTreeMap<usize, u64>,
        accepted: u64,
    ) -> Result<Option<(u64, Vec<(Label, Rc<Weighted>)>)>, Diagnostic> {
        let mut successors = BTreeMap::<Label, targets::Targets>::new();
        for (node, weight) in closed {
            let moves = automaton.nodes[*node].moves();
            budget.charge(
                function,
                moves.len().saturating_add(1),
                "factored skeleton subset construction",
            )?;
            for (label, child) in moves {
                if !successors.contains_key(label) {
                    budget.reserve_btree_node::<Label, targets::Targets>(
                        function,
                        "factored successor label node",
                    )?;
                }
                let targets = successors
                    .entry(label.clone())
                    .or_insert_with(|| targets::Targets::new(budget.pays_allocations()));
                if !targets.add(function, budget, *child, *weight)? {
                    return Ok(None);
                }
            }
        }
        budget.reserve_array::<(Label, Rc<Weighted>)>(
            function,
            successors.len(),
            "factored successor vector",
        )?;
        let mut result = Vec::with_capacity(successors.len());
        for (label, targets) in successors {
            result.push((label, targets.share(function, budget)?));
        }
        Ok(Some((accepted, result)))
    }

    #[test]
    fn complete_outer_labels_preserve_order_weights_zero_empty_and_overflow() {
        let function = function();
        for width in [0, 1, 2, 8, 64] {
            for overflow in [false, true] {
                let (automaton, closed) = subject(width, overflow);
                let mut old = ReplayBudget::new();
                let mut new = ReplayBudget::new();
                let expected = before_labels(&function, &mut old, &automaton, &closed, 57);
                let actual = labels::successors(&function, &mut new, &automaton, &closed, 57);
                super::super::inline_weighted::assert_successors(&actual, &expected);
                if let Some((accepted, labels)) = actual.unwrap() {
                    assert_eq!(accepted, 57);
                    assert_eq!(labels.len(), if width == 0 { 0 } else { width + 3 });
                    assert!(labels.windows(2).all(|pair| pair[0].0 < pair[1].0));
                } else {
                    assert!(overflow && width != 0);
                }
            }
        }
    }

    #[test]
    fn complete_paid_outer_storage_keeps_last_drop_and_physical_boundaries() {
        let function = function();
        let (automaton, closed) = subject(64, false);
        fn inspect<T: std::ops::Deref<Target = [(usize, u64)]>>(
            result: Result<Option<(u64, Vec<(Label, Rc<T>)>)>, Diagnostic>,
        ) -> bool {
            let success = result.is_ok();
            if let Ok(Some((accepted, labels))) = &result {
                assert_eq!(*accepted, 57);
                assert_eq!(labels.len(), 67);
                for (_, weights) in labels {
                    assert_eq!(weights.iter().map(|(_, weight)| weight).sum::<u64>(), 10);
                }
            }
            drop(result);
            success
        }
        let observe = |old, available, floor| {
            crate::bounded_output::with_limit_usage_refusal(available + floor, || {
                assert!(crate::bounded_output::set_active_floor(floor));
                crate::test_allocation_observer::measure(|| {
                    let mut budget = ReplayBudget::new();
                    let success = if old {
                        inspect(before_labels(&function, &mut budget, &automaton, &closed, 57))
                    } else {
                        inspect(labels::successors(&function, &mut budget, &automaton, &closed, 57))
                    };
                    if !success {
                        assert!(budget.reserve_bytes(&function, 0, "sticky labels").is_err());
                    }
                    drop(budget);
                    success
                })
            })
        };
        let ((old_ok, old), refused, before, receipt) = observe(true, usize::MAX, 0);
        assert!(old_ok && !refused && receipt.is_none());
        let ((new_ok, new), refused, exact, receipt) = observe(false, usize::MAX, 0);
        assert!(new_ok && !refused && receipt.is_none());
        assert!(exact < before && new.allocated < old.allocated);
        for allocation in [old, new] {
            assert!(!allocation.invalid && allocation.live == 0);
            assert_eq!(allocation.allocated, allocation.freed);
        }
        assert!(new.allocated <= exact && old.allocated <= before);
        for floor in [0, 37] {
            for available in [0, exact - 1, exact] {
                let ((ok, allocation), refused, spent, receipt) = observe(false, available, floor);
                assert_eq!(ok, available == exact);
                assert_eq!(refused, !ok);
                assert_eq!(receipt.is_some(), !ok);
                if let Some(receipt) = receipt {
                    assert_eq!(receipt.floor, floor);
                }
                assert!(spent <= available && allocation.allocated <= spent);
                assert!(!allocation.invalid && allocation.live == 0);
                assert_eq!(allocation.allocated, allocation.freed);
                if available == 0 {
                    assert_eq!(allocation.allocated, 0);
                }
            }
        }
    }

    #[test]
    fn outer_label_work_refusal_precedes_publication() {
        let function = function();
        let (automaton, closed) = subject(8, false);
        let mut budget = ReplayBudget::new();
        budget.remaining = 0;
        assert!(labels::successors(&function, &mut budget, &automaton, &closed, 57).is_err());
    }

    #[test]
    fn complete_outer_label_sources_and_original_producer_remain_exact() {
        use sha2::{Digest, Sha256};
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
        let evidence = root.join("docs/evidence/factored-flat-labels-v1");
        let proof: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(evidence.join("sources.json")).unwrap())
                .unwrap();
        let digest = |text: &str| {
            format!(
                "{:x}",
                crate::digest_hex::LowerHex(Sha256::digest(text.as_bytes()))
            )
        };
        assert_eq!(proof["predecessors"].as_array().unwrap().len(), 4);
        for record in proof["predecessors"].as_array().unwrap() {
            let before =
                std::fs::read_to_string(evidence.join(record["archive"].as_str().unwrap()))
                    .unwrap();
            let current =
                std::fs::read_to_string(root.join(record["path"].as_str().unwrap())).unwrap();
            let current = crate::source_reflows_merge::main67107_inputs::compile35f_input(
                record["path"].as_str().unwrap(), &current,
            );
            assert_eq!(digest(&before), record["before_sha256"]);
            assert_eq!(digest(&current), record["after_sha256"]);
            let restore = |input: &str| -> Option<String> {
                let mut output = input.to_owned();
                for edit in record["inverse"].as_array().unwrap() {
                    let after = edit["after"].as_str().unwrap();
                    if output.matches(after).count() != 1 {
                        return None;
                    }
                    output = output.replacen(after, edit["before"].as_str().unwrap(), 1);
                }
                Some(output)
            };
            assert_eq!(restore(&current).as_deref(), Some(before.as_str()));
            assert_ne!(
                restore(&(current.clone() + "\n")).as_deref(),
                Some(before.as_str())
            );
            let mut changed = current.clone();
            changed.replace_range(..1, "!");
            assert_ne!(restore(&changed).as_deref(), Some(before.as_str()));
        }
        for record in proof["new_files"].as_array().unwrap() {
            let current =
                std::fs::read_to_string(root.join(record["path"].as_str().unwrap())).unwrap();
            let current = crate::source_reflows_merge::main67107_inputs::compile35f_input(
                record["path"].as_str().unwrap(), &current,
            );
            assert_eq!(digest(&current), record["sha256"]);
            assert_ne!(digest(&(current + "\n")), record["sha256"]);
        }
        let before = std::fs::read_to_string(
            evidence.join("src__cleanup_plan__replay__factored__canonical_shared.rs.before.txt"),
        )
        .unwrap();
        let start = before
            .find("        let mut successors = BTreeMap::<Label, targets::Targets>::new();\n")
            .unwrap();
        let end = start
            + before[start..]
                .find("\n    }\n\n    pub(super) fn id(")
                .unwrap();
        let old_body = before[start..end].replace("self.automaton", "automaton");
        assert_eq!(crate::source_reflows_merge::main67107_inputs::project("testsMAIN", include_str!("tests.rs")).matches(&old_body).count(), 1);
    }
}
