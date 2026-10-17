//! Independent complete immediate canonical receiver and actual arena owners.
use super::{
    before_arena, Automaton, Canonicalizer, Diagnostic, Label, ReplayBudget, ResolvedFunction,
};
use crate::test_allocation_observer::{current_metrics, measure, Metrics};

// Actual signature dictionary branch observations. Const TLS has no heap
// backing, carries no payer, and is reset before each measured whole owner.
thread_local! {
    static SIGNATURE_LOOKUPS: std::cell::Cell<(usize, usize)> = const {
        std::cell::Cell::new((0, 0))
    };
}
pub(super) fn signature_lookup(hit: bool) {
    SIGNATURE_LOOKUPS.with(|value| {
        let (hits, misses) = value.get();
        value.set((hits + usize::from(hit), misses + usize::from(!hit)));
    });
}
fn lookups() -> (usize, usize) {
    SIGNATURE_LOOKUPS.with(std::cell::Cell::get)
}

fn fixture() -> ResolvedFunction {
    super::super::lexical_plan_tests::fixture(super::super::lexical_plan_tests::WHILE).1
}

#[test]
fn ordered_interning_preserves_complete_ids_signatures_work_overflow_and_cycle() {
    let function = fixture();
    let (old_graph, old_sink, old_roots) = super::tests::automaton(&function);
    let (graph, sink, roots) = super::tests::automaton(&function);
    assert_eq!((old_sink, &old_roots), (sink, &roots));
    assert!(!std::ptr::eq(&old_graph, &graph));
    for root in roots.into_iter().chain([sink]) {
        for weight in [0, 1, 2, u64::MAX] {
            for work in [0, 1, 2, 4, 16, 128, 1024] {
                let mut old = before_arena::Canonicalizer::new(&old_graph, old_sink);
                let mut current = Canonicalizer::new(&graph, sink);
                let mut a = ReplayBudget::unobserved();
                let mut b = ReplayBudget::unobserved();
                a.remaining = work;
                b.remaining = work;
                let expected = old.id(&function, vec![(root, weight)], &mut a);
                let actual = current.id(&function, vec![(root, weight)], &mut b);
                assert_eq!(
                    actual, expected,
                    "six fields root={root} weight={weight} work={work}"
                );
                assert_eq!(
                    (a.remaining, a.skeleton_remaining),
                    (b.remaining, b.skeleton_remaining)
                );
                assert_eq!(&current.ids, old.signatures());
                assert_eq!(&current.states, old.states());
                assert!(
                    current.cursor.is_empty()
                        && current.frames.is_empty()
                        && current.open.is_empty()
                );
            }
        }
    }
    for shape in 0..3 {
        let (old_graph, old_sink, old_roots) = corpus(&function, shape);
        let (graph, sink, roots) = corpus(&function, shape);
        assert_eq!((old_sink, &old_roots), (sink, &roots));
        assert!(!std::ptr::eq(&old_graph, &graph));
        let mut old = before_arena::Canonicalizer::new(&old_graph, old_sink);
        let mut current = Canonicalizer::new(&graph, sink);
        let mut a = ReplayBudget::unobserved();
        let mut b = ReplayBudget::unobserved();
        for (old_root, root) in old_roots.into_iter().zip(roots) {
            assert_eq!(
                current.id(&function, vec![(root, 1)], &mut b),
                old.id(&function, vec![(old_root, 1)], &mut a),
            );
            assert_eq!(&current.ids, old.signatures());
            assert_eq!(&current.states, old.states());
            assert_eq!(
                (a.remaining, a.skeleton_remaining),
                (b.remaining, b.skeleton_remaining)
            );
        }
    }
    for old in [true, false] {
        failures_keep_exact_owner_and_first_receipt(old);
    }
}

// Ordinary immutable input construction is outside the canonical owner window.
// Duplicate signatures retain distinct weighted-state keys; the wide case has
// distinct real observation labels and signatures instead of padded old fees.
fn corpus(function: &ResolvedFunction, shape: usize) -> (Automaton, usize, Vec<usize>) {
    let mut budget = ReplayBudget::unobserved();
    let mut graph = Automaton::default();
    let sink = graph.node(function, &mut budget).unwrap();
    if shape == 2 {
        return (graph, sink, vec![sink]);
    }
    let mut roots = Vec::new();
    for row in 0..32 {
        let root = graph.node(function, &mut budget).unwrap();
        for column in 0..if shape == 0 { 128 } else { 24 } {
            // Duplicate corpus repeats the same full labels; unique-wide uses
            // distinct labels on each row. Both retain distinct state keys.
            let identity_row = if shape == 0 { 0 } else { row };
            let label = Label::Observation(super::Rc::new(
                super::super::super::SkeletonObservation::Boolean {
                    expression: crate::hir::ExpressionId::from_owned(format!(
                        "arena.row.{identity_row}.column.{column}"
                    )),
                    value: column % 2 == 0,
                },
            ));
            graph
                .movement(function, &mut budget, root, label, sink)
                .unwrap();
        }
        roots.push(root);
    }
    (graph, sink, roots)
}

fn observe(
    function: &ResolvedFunction,
    graph: &Automaton,
    sink: usize,
    roots: &[usize],
    limit: usize,
    old: bool,
) -> (Result<(usize, usize, u64, [usize; 4]), ()>, Metrics) {
    SIGNATURE_LOOKUPS.with(|value| value.set((0, 0)));
    measure(|| {
        let mut budget = match ReplayBudget::with_physical(function, limit) {
            Ok(value) => value,
            Err(error) => {
                assert_eq!(error.code, "SPX-H006");
                drop(error);
                return Err(());
            }
        };
        let result = (|| {
            let mut checksum = 0u64;
            let counts;
            if old {
                let mut owner = before_arena::Canonicalizer::new(graph, sink);
                for root in roots {
                    budget.reserve_array::<(usize, u64)>(function, 1, "arena corpus root")?;
                    let id = owner.id(function, vec![(*root, 1)], &mut budget)?.unwrap();
                    checksum = checksum.wrapping_mul(31).wrapping_add(id as u64);
                }
                let (hits, misses) = lookups();
                counts = [hits, misses, owner.signatures().len(), owner.states().len()];
            } else {
                // The same explicit shipping acquisition precedes the actual
                // new Canonicalizer/arena header constructors.
                budget.reserve_bytes(
                    function,
                    Canonicalizer::CONSTRUCTOR_CONTROLS,
                    "canonical ordered dictionary owner receiving",
                )?;
                let mut owner = Canonicalizer::new(graph, sink);
                for root in roots {
                    budget.reserve_array::<(usize, u64)>(function, 1, "arena corpus root")?;
                    let id = owner.id(function, vec![(*root, 1)], &mut budget)?.unwrap();
                    checksum = checksum.wrapping_mul(31).wrapping_add(id as u64);
                }
                let (hits, misses) = lookups();
                counts = [hits, misses, owner.ids.len(), owner.states.len()];
            }
            Ok::<_, Diagnostic>((
                budget.physical.as_ref().unwrap().charged(),
                budget.remaining,
                checksum,
                counts,
            ))
        })();
        assert!(current_metrics().allocated <= budget.physical.as_ref().unwrap().charged());
        let result = result.map_err(|error| {
            assert_eq!(error.code, "SPX-H006");
            drop(error);
        });
        drop(budget);
        result
    })
}

#[test]
fn whole_arena_owner_reports_duplicate_unique_wide_costs_growth_refusal_and_complete_drop() {
    let function = fixture();
    for shape in 0..3 {
        let mut observations = [(0, 0, 0, [0; 4]); 2];
        for (index, old) in [true, false].into_iter().enumerate() {
            // Separate construction for each whole receiver; no graph/Rc roots
            // or label allocation is shared between the old/current arms.
            let (graph, sink, roots) = corpus(&function, shape);
            let (result, metrics) = observe(&function, &graph, sink, &roots, 1 << 26, old);
            observations[index] = result.unwrap();
            assert!(!metrics.invalid && metrics.live == 0);
            assert_eq!(metrics.allocated, metrics.freed);
            eprintln!(
                "arena shape={shape} old={old} reserved={} hits={} misses={} signature_nodes={} state_nodes={} actual={metrics:?}",
                observations[index].0, observations[index].3[0], observations[index].3[1],
                observations[index].3[2], observations[index].3[3]
            );
            let paid = observations[index].0;
            for limit in [0, paid / 2, paid - 1, paid] {
                let (result, metrics) = observe(&function, &graph, sink, &roots, limit, old);
                assert_eq!(result.is_ok(), limit == paid);
                assert!(!metrics.invalid && metrics.live == 0);
                assert_eq!(metrics.allocated, metrics.freed);
            }
        }
        assert_eq!(observations[0].1, observations[1].1, "exact original work");
        assert_eq!(observations[0].2, observations[1].2, "insertion ordinals");
        assert_eq!(
            observations[0].3, observations[1].3,
            "actual lookup and node counts"
        );
        let [hits, misses, signatures, states] = observations[1].3;
        assert_eq!(
            misses, signatures,
            "one actual signature insertion per miss"
        );
        assert_eq!(
            hits + misses,
            states,
            "one actual signature lookup per published state"
        );
        if shape == 0 {
            assert!(hits > 0 && misses > 0 && states > signatures);
            assert!(
                observations[1].0 < observations[0].0,
                "whole duplicate-signature corpus, unchanged old receiver"
            );
        }
        if shape == 1 {
            assert_eq!(
                hits, 0,
                "all-unique wide signatures must not fabricate hits"
            );
            assert!(misses > 1 && states == signatures);
        }
        // Unique-wide and tiny cases are reported even if new controls/spare
        // backing are unfavorable. No universal inequality is asserted.
    }
}

// Exercise the actual dictionary owner after a real completed insertion. The
// rejected calls must not read either dictionary or touch a foreign payer, and
// restoring the original function/ledger cannot revive a poisoned owner.
fn failures_keep_exact_owner_and_first_receipt(old: bool) {
    use crate::bounded_output::{
        active_remaining, reserve_active_required, with_limit_usage_refusal,
        with_limit_usage_refusal_site,
    };
    let function = fixture();
    let wrong = function.clone();
    for attack in 0..4 {
        let (graph, sink, roots) = corpus(&function, 1);
        let ((expected, allocation), refused, _, first, site) =
            with_limit_usage_refusal_site(1 << 26, || {
                measure(|| {
                    let mut budget = ReplayBudget::new();
                    let mut foreign = ReplayBudget::new();
                    if !old {
                        budget
                            .reserve_bytes(
                                &function,
                                Canonicalizer::CONSTRUCTOR_CONTROLS,
                                "canonical ordered dictionary owner receiving",
                            )
                            .unwrap();
                    }
                    macro_rules! exercise {
                        ($owner:ident = $constructor:expr; $counts:expr) => {{
                            let mut $owner = $constructor;
                            budget
                                .reserve_array::<(usize, u64)>(&function, 1, "arena bound root")
                                .unwrap();
                            assert!($owner
                                .id(&function, vec![(roots[0], 1)], &mut budget)
                                .unwrap()
                                .is_some());
                            // Both call inputs are real prepaid owners before a
                            // deliberately failed/stale authority is selected.
                            budget
                                .reserve_array::<(usize, u64)>(&function, 1, "arena rejected root")
                                .unwrap();
                            let input = vec![(roots[1], 1)];
                            budget
                                .reserve_array::<(usize, u64)>(&function, 1, "arena restored root")
                                .unwrap();
                            let retry = vec![(roots[1], 1)];
                            let counts = $counts;
                            assert!(counts.0 > 0 && counts.1 > 0);
                            let queries = lookups();
                            let work = budget.remaining;
                            let mut expected = None;
                            if attack == 0 {
                                let remaining = active_remaining().unwrap();
                                let requested = remaining.checked_add(1).unwrap();
                                assert!(!reserve_active_required(requested));
                                expected = Some((requested, remaining));
                            }
                            let paid = active_remaining();
                            let allocated = current_metrics().allocated;
                            let error = if attack == 1 {
                                $owner.id(&wrong, input, &mut budget)
                            } else if attack == 2 {
                                $owner.id(&function, input, &mut foreign)
                            } else if attack == 3 {
                                let (result, child_refused, child_paid, child_first) =
                                    with_limit_usage_refusal(1 << 26, || {
                                        let at = current_metrics().allocated;
                                        let result = $owner.id(&function, input, &mut budget);
                                        assert_eq!(current_metrics().allocated, at);
                                        result
                                    });
                                assert!(!child_refused && child_paid == 0 && child_first.is_none());
                                result
                            } else {
                                $owner.id(&function, input, &mut budget)
                            };
                            assert_eq!(error, Err(Diagnostic::io("SPX-H006", String::new())));
                            assert_eq!(active_remaining(), paid);
                            if attack != 3 {
                                assert_eq!(current_metrics().allocated, allocated);
                            }
                            // The nested scope itself owns one ordinary TLS
                            // budget allocation; the rejected call inside it
                            // was checked separately above.
                            let allocated = current_metrics().allocated;
                            assert_eq!(budget.remaining, work);
                            assert_eq!($counts, counts);
                            assert_eq!(lookups(), queries);
                            assert!(foreign.refusal_text.is_none() && !foreign.refusal_spent);
                            assert_eq!(
                                $owner.id(&function, retry, &mut budget),
                                Err(Diagnostic::io("SPX-H006", String::new())),
                            );
                            assert_eq!(active_remaining(), paid);
                            assert_eq!(current_metrics().allocated, allocated);
                            assert_eq!(budget.remaining, work);
                            assert_eq!($counts, counts);
                            assert_eq!(lookups(), queries);
                            drop($owner);
                            expected
                        }};
                    }
                    let expected = if old {
                        exercise!(owner = before_arena::Canonicalizer::new(&graph, sink);
                            (owner.signatures().len(), owner.states().len()))
                    } else {
                        exercise!(owner = Canonicalizer::new(&graph, sink);
                            (owner.ids.len(), owner.states.len()))
                    };
                    drop(foreign);
                    drop(budget);
                    expected
                })
            });
        if let Some((requested, remaining)) = expected {
            assert!(refused && site.is_some());
            let first = first.unwrap();
            assert_eq!((first.requested, first.remaining), (requested, remaining));
            assert_eq!(first.floor, 0);
            assert_eq!(first.stage, None);
        }
        assert!(!allocation.invalid && allocation.live == 0);
        assert_eq!(allocation.allocated, allocation.freed);
    }
}

#[test]
fn vec_borrowed_queries_and_zipped_misses_keep_full_old_key_order() {
    use super::intern_arena::{Query, SignatureQuery};
    use std::mem::size_of;
    struct PreviousQuery<'a> {
        accepted: u64,
        successors: &'a [(Label, super::SharedWeighted)],
        children: &'a [usize],
    }
    let previous_controls = 2 * size_of::<PreviousQuery<'_>>()
        + 2 * size_of::<&Vec<(Label, super::SharedWeighted)>>()
        + 2 * size_of::<&Vec<usize>>()
        + size_of::<&[(Label, super::SharedWeighted)]>()
        + size_of::<&[usize]>();
    assert!(SignatureQuery::CONTROLS < previous_controls);
    let function = fixture();
    for shape in 0..3 {
        let (graph, sink, roots) = corpus(&function, shape);
        let mut owner = Canonicalizer::new(&graph, sink);
        let mut budget = ReplayBudget::unobserved();
        for root in roots {
            owner.id(&function, vec![(root, 1)], &mut budget).unwrap();
        }
        for (key, _) in owner.ids.iter() {
            let successors: Vec<_> = key.1.iter().map(|(label, _)| {
                (label.clone(), super::Rc::new(super::SmallWeighted::One([(sink, 1)])))
            }).collect();
            let children: Vec<_> = key.1.iter().map(|(_, id)| *id).collect();
            let previous = PreviousQuery {
                accepted: key.0, successors: &successors, children: &children,
            };
            assert_eq!(previous.accepted, key.0);
            assert_eq!(previous.successors.len(), previous.children.len());
            let query = SignatureQuery { accepted: key.0, successors: &successors, children: &children };
            for (stored, _) in owner.ids.iter() {
                assert_eq!(query.compare(stored), key.cmp(stored));
            }
            let expected: Vec<_> = successors.iter().enumerate()
                .map(|(index, (label, _))| (label.clone(), children[index])).collect();
            let actual: Vec<_> = successors.into_iter().zip(&children)
                .map(|((label, _), id)| (label, *id)).collect();
            assert_eq!(actual, expected);
            assert_eq!(actual, key.1);
        }
    }
}

// Complete immediate f6ae receiver: independent methods, never delegates to new successors.
use super::*;
impl Canonicalizer<'_> {
    fn previous_selected_successors(
        &mut self,
        function: &ResolvedFunction,
        state: &SharedWeighted,
        budget: &mut ReplayBudget,
    ) -> Result<Option<(u64, Vec<(Label, SharedWeighted)>)>, Diagnostic> {
        if budget.pays_allocations() {
            budget.reserve_bytes(
                function,
                closure_receiving::ClosureReceiving::DIRECT_CONTROLS,
                "factored held closure invocation controls",
            )?;
        }
        let mut receiving = closure_receiving::ClosureReceiving::from_prepaid(function, budget);
        self.previous_selected_successors_with_receiving(function, state, budget, &mut receiving)
    }
    fn previous_selected_successors_with_receiving(
        &mut self,
        function: &ResolvedFunction,
        state: &SharedWeighted,
        budget: &mut ReplayBudget,
        receiving: &mut closure_receiving::ClosureReceiving,
    ) -> Result<Option<(u64, Vec<(Label, SharedWeighted)>)>, Diagnostic> {
        if let [(node, 1)] = state.as_slice() {
            self.closure_query(function, *node, budget, receiving);
            receiving.ready = false;
            if receiving.refusal.is_some() {
                return Err(receiving.refusal.take().expect("published refusal"));
            }
            if receiving.overflow {
                return Ok(None);
            }
            if receiving.empty {
                // No closed map, Rc, label-group map, or iterator is constructed.
                // The original surviving empty vector and outer result remain.
                budget.reserve_array::<(Label, SharedWeighted)>(
                    function,
                    0,
                    "factored successor vector",
                )?;
                let accepted = 0;
                let result = Vec::with_capacity(0);
                return Ok(Some((accepted, result)));
            }
            if budget.pays_allocations() {
                budget.reserve_bytes(
                    function,
                    2 * std::mem::size_of::<Rc<ordered_weights::OrderedWeights>>()
                        + 2 * std::mem::size_of::<&ordered_weights::OrderedWeights>(),
                    "factored borrowed closed receiving controls",
                )?;
            }
            let closure = Rc::clone(self.closures.get(node).expect("computed shared closure"));
            for (_, routes) in closure.iter() {
                let Some(_sum) = routes
                    .checked_mul(1)
                    .and_then(|product| 0_u64.checked_add(product))
                else {
                    return Ok(None);
                };
                budget.charge(function, 1, "factored weighted route addition")?;
            }
            let borrowed = closure;
            let closed = &*borrowed;
            let accepted = closed.get(&self.sink).copied().unwrap_or(0);
            match &budget.payer {
                AllocationPayer::ActiveBuilder => {
                    budget.reserve_bytes(function, borrowed_labels::ordinary_receiving(), "factored direct singleton call receivers")?;
                    return borrowed_labels::ordinary_successors(function, budget, self.automaton, closed, accepted);
                }
                AllocationPayer::LegacyUnobserved => {}
                _ => {
                    budget.reserve_bytes(function, borrowed_labels::receiving::<ordered_weights::OrderedWeights>(), "factored borrowed label call receivers")?;
                    return borrowed_labels::successors(function, budget, self.automaton, closed, accepted);
                }
            }
            if budget.pays_allocations() {
                budget.reserve_bytes(
                    function,
                    std::mem::size_of::<BTreeMap<Label, paid_many::Targets>>()
                        + std::mem::size_of::<Option<&mut paid_many::Targets>>(),
                    "factored successor label receiving controls",
                )?;
            }
            let mut successors = BTreeMap::<Label, paid_many::Targets>::new();
            for (node, weight) in closed.iter() {
                let moves = self.automaton.nodes[*node].moves();
                budget.charge(
                    function,
                    moves.len().saturating_add(1),
                    "factored skeleton subset construction",
                )?;
                for (label, child) in moves {
                    if !successors.contains_key(label) {
                        budget.reserve_btree_node::<Label, paid_many::Targets>(
                            function,
                            "factored successor label node",
                        )?;
                        if budget.pays_allocations() {
                            budget.reserve_bytes(
                                function,
                                2 * std::mem::size_of::<paid_many::Targets>()
                                    + std::mem::size_of::<Option<u64>>()
                                    + std::mem::size_of::<Option<paid_many::Targets>>(),
                                "factored inline singleton target initializer and receiving",
                            )?;
                        }
                        budget.reserve_bytes(
                            function,
                            0,
                            "factored singleton target authentication",
                        )?;
                        let weight = 0_u64
                            .checked_add(*weight)
                            .expect("zero plus u64 cannot overflow");
                        successors.insert(
                            label.clone(),
                            paid_many::Targets::One {
                                child: *child,
                                weight,
                            },
                        );
                    } else {
                        let targets = successors.get_mut(label).expect("existing label");
                        match targets {
                            paid_many::Targets::One {
                                child: first,
                                weight: total,
                            } if *first == *child => {
                                // Same repeated-child checked_add before any later
                                // target acquisition or error, as in the old map.
                                let Some(sum) = total.checked_add(*weight) else {
                                    return Ok(None);
                                };
                                *total = sum;
                            }
                            paid_many::Targets::One {
                                child: first,
                                weight: total,
                            } => {
                                if budget.pays_allocations() {
                                    budget.reserve_bytes(function, paid_many::PROMOTE_RECEIVING,
                                        "factored composed promotion receivers")?;
                                }
                                #[cfg(test)]
                                if budget.pays_allocations() { paid_many::note_branch(true); }
                                *targets = paid_many::promote(function, budget, *first, *total, *child, *weight)?;
                            }
                            paid_many::Targets::PaidMany(_) => {
                                budget.reserve_bytes(function, paid_many::ADD_RECEIVING,
                                    "factored composed flat add receivers")?;
                                let paid_many::Targets::PaidMany(flat) = targets else { unreachable!("paid flat arm") };
                                if !paid_many::add(function, budget, flat, *child, *weight)? { return Ok(None); }
                            }
                            paid_many::Targets::Many(targets) => {
                                if !targets.contains_key(child) {
                                    budget.reserve_btree_node::<usize, u64>(
                                        function,
                                        "factored successor target node",
                                    )?;
                                }
                                let slot = targets.entry(*child).or_insert(0);
                                let Some(sum) = slot.checked_add(*weight) else {
                                    return Ok(None);
                                };
                                *slot = sum;
                            }
                        }
                    }
                }
            }
            budget.reserve_array::<(Label, SharedWeighted)>(
                function,
                successors.len(),
                "factored successor vector",
            )?;
            let mut result = Vec::with_capacity(successors.len());
            // Do not initialize an owning map iterator or its receiving tuple on
            // the empty route. Zero-byte vector authentication above still runs.
            if !successors.is_empty() {
                if budget.pays_allocations() {
                    budget.reserve_bytes(
                        function,
                        std::mem::size_of::<
                            std::collections::btree_map::IntoIter<
                                Label,
                                paid_many::Targets,
                            >,
                        >() + std::mem::size_of::<Option<(Label, paid_many::Targets)>>()
                            + std::mem::size_of::<(Label, paid_many::Targets)>(),
                        "factored successor target iterator receiving",
                    )?;
                }
                for (label, targets) in successors {
                    if budget.pays_allocations() {
                        budget.reserve_bytes(
                            function,
                            2 * std::mem::size_of::<SharedWeighted>()
                                + std::mem::size_of::<(usize, u64)>(),
                            "factored inline successor finish receiving",
                        )?;
                    }
                    budget.reserve_bytes(
                        function,
                        0,
                        "factored successor target finish authentication",
                    )?;
                    let state = match targets {
                        paid_many::Targets::One { child, weight } => {
                            if budget.pays_allocations() {
                                budget.reserve_bytes(
                                    function,
                                    2 * std::mem::size_of::<SmallWeighted>()
                                        + std::mem::size_of::<[(usize, u64); 1]>()
                                        + std::mem::size_of::<Result<(), Diagnostic>>(),
                                    "factored inline weighted enum and array receiving",
                                )?;
                            }
                            budget.reserve_bytes(
                                function,
                                physical_clone::rc_backing_bytes::<SmallWeighted>(),
                                "factored weighted shared header",
                            )?;
                            let singleton = SmallWeighted::One([(child, weight)]);
                            Rc::new(singleton)
                        }
                        paid_many::Targets::PaidMany(_) => {
                            budget.reserve_bytes(function, paid_many::FINISH_RECEIVING,
                                "factored composed flat finish receivers")?;
                            let paid_many::Targets::PaidMany(flat) = targets else { unreachable!("paid flat finish") };
                            own_weighted(function, budget, flat)?
                        }
                        paid_many::Targets::Many(targets) => {
                            // The remaining fallible weighted producer still has
                            // its real map argument and Result receiving carrier.
                            if budget.pays_allocations() {
                                budget.reserve_bytes(
                                    function,
                                    std::mem::size_of::<BTreeMap<usize, u64>>()
                                        + std::mem::size_of::<Vec<(usize, u64)>>()
                                        + std::mem::size_of::<Result<SharedWeighted, Diagnostic>>(),
                                    "factored many weighted producer receiving",
                                )?;
                            }
                            weighted_successor(function, budget, targets)?
                        }
                    };
                    result.push((label, state));
                }
            }
            return Ok(Some((accepted, result)));
        }
        if budget.pays_allocations() {
            budget.reserve_bytes(
                function,
                ordered_weights::OrderedWeights::CONTROLS
                    + 2 * std::mem::size_of::<&ordered_weights::OrderedWeights>(),
                "factored ordered accumulator controls",
            )?;
        }
        let mut owned = ordered_weights::OrderedWeights::new();
        for (node, weight) in state.iter() {
            self.closure_query(function, *node, budget, receiving);
            receiving.ready = false;
            if receiving.refusal.is_some() {
                return Err(receiving.refusal.take().expect("published refusal"));
            }
            if receiving.overflow {
                return Ok(None);
            }
            let Some(closure) = self.closures.get(node) else {
                continue;
            };
            for (inner, routes) in closure.iter() {
                let Some(sum) = routes.checked_mul(*weight).and_then(|product| {
                    owned.get(inner).copied().unwrap_or(0).checked_add(product)
                }) else {
                    return Ok(None);
                };
                budget.charge(function, 1, "factored weighted route addition")?;
                if owned.get(inner).is_none() {
                    if owned.needs_capacity() {
                        owned.grow(function, budget)?;
                    }
                    owned.insert_prepaid(*inner, 0_u64);
                }
                *owned.get_mut(inner).expect("inserted ordered weight") = sum;
            }
        }
        let closed = &owned;
        let accepted = closed.get(&self.sink).copied().unwrap_or(0);
        match &budget.payer {
            AllocationPayer::ActiveBuilder => {
                budget.reserve_bytes(function, borrowed_labels::ordinary_receiving(), "factored direct singleton call receivers")?;
                return borrowed_labels::ordinary_successors(function, budget, self.automaton, closed, accepted);
            }
            AllocationPayer::LegacyUnobserved => {}
            _ => {
                budget.reserve_bytes(function, borrowed_labels::receiving::<ordered_weights::OrderedWeights>(), "factored borrowed label call receivers")?;
                return borrowed_labels::successors(function, budget, self.automaton, closed, accepted);
            }
        }
        if budget.pays_allocations() {
            budget.reserve_bytes(
                function,
                std::mem::size_of::<BTreeMap<Label, paid_many::Targets>>()
                    + std::mem::size_of::<Option<&mut paid_many::Targets>>(),
                "factored successor label receiving controls",
            )?;
        }
        let mut successors = BTreeMap::<Label, paid_many::Targets>::new();
        for (node, weight) in closed.iter() {
            let moves = self.automaton.nodes[*node].moves();
            budget.charge(
                function,
                moves.len().saturating_add(1),
                "factored skeleton subset construction",
            )?;
            for (label, child) in moves {
                if !successors.contains_key(label) {
                    budget.reserve_btree_node::<Label, paid_many::Targets>(
                        function,
                        "factored successor label node",
                    )?;
                    if budget.pays_allocations() {
                        budget.reserve_bytes(
                            function,
                            2 * std::mem::size_of::<paid_many::Targets>()
                                + std::mem::size_of::<Option<u64>>()
                                + std::mem::size_of::<Option<paid_many::Targets>>(),
                            "factored inline singleton target initializer and receiving",
                        )?;
                    }
                    budget.reserve_bytes(
                        function,
                        0,
                        "factored singleton target authentication",
                    )?;
                    let weight = 0_u64
                        .checked_add(*weight)
                        .expect("zero plus u64 cannot overflow");
                    successors.insert(
                        label.clone(),
                        paid_many::Targets::One {
                            child: *child,
                            weight,
                        },
                    );
                } else {
                    let targets = successors.get_mut(label).expect("existing label");
                    match targets {
                        paid_many::Targets::One {
                            child: first,
                            weight: total,
                        } if *first == *child => {
                            // Same repeated-child checked_add before any later
                            // target acquisition or error, as in the old map.
                            let Some(sum) = total.checked_add(*weight) else {
                                return Ok(None);
                            };
                            *total = sum;
                        }
                        paid_many::Targets::One {
                            child: first,
                            weight: total,
                        } => {
                            if budget.pays_allocations() {
                                budget.reserve_bytes(function, paid_many::PROMOTE_RECEIVING,
                                    "factored composed promotion receivers")?;
                            }
                            #[cfg(test)]
                            if budget.pays_allocations() { paid_many::note_branch(false); }
                            *targets = paid_many::promote(function, budget, *first, *total, *child, *weight)?;
                        }
                        paid_many::Targets::PaidMany(_) => {
                            budget.reserve_bytes(function, paid_many::ADD_RECEIVING,
                                "factored composed flat add receivers")?;
                            let paid_many::Targets::PaidMany(flat) = targets else { unreachable!("paid flat arm") };
                            if !paid_many::add(function, budget, flat, *child, *weight)? { return Ok(None); }
                        }
                        paid_many::Targets::Many(targets) => {
                            if !targets.contains_key(child) {
                                budget.reserve_btree_node::<usize, u64>(
                                    function,
                                    "factored successor target node",
                                )?;
                            }
                            let slot = targets.entry(*child).or_insert(0);
                            let Some(sum) = slot.checked_add(*weight) else {
                                return Ok(None);
                            };
                            *slot = sum;
                        }
                    }
                }
            }
        }
        budget.reserve_array::<(Label, SharedWeighted)>(
            function,
            successors.len(),
            "factored successor vector",
        )?;
        let mut result = Vec::with_capacity(successors.len());
        // Do not initialize an owning map iterator or its receiving tuple on
        // the empty route. Zero-byte vector authentication above still runs.
        if !successors.is_empty() {
            if budget.pays_allocations() {
                budget.reserve_bytes(
                    function,
                    std::mem::size_of::<
                        std::collections::btree_map::IntoIter<Label, paid_many::Targets>,
                    >() + std::mem::size_of::<Option<(Label, paid_many::Targets)>>()
                        + std::mem::size_of::<(Label, paid_many::Targets)>(),
                    "factored successor target iterator receiving",
                )?;
            }
            for (label, targets) in successors {
                if budget.pays_allocations() {
                    budget.reserve_bytes(
                        function,
                        2 * std::mem::size_of::<SharedWeighted>()
                            + std::mem::size_of::<(usize, u64)>(),
                        "factored inline successor finish receiving",
                    )?;
                }
                budget.reserve_bytes(
                    function,
                    0,
                    "factored successor target finish authentication",
                )?;
                let state = match targets {
                    paid_many::Targets::One { child, weight } => {
                        if budget.pays_allocations() {
                            budget.reserve_bytes(
                                function,
                                2 * std::mem::size_of::<SmallWeighted>()
                                    + std::mem::size_of::<[(usize, u64); 1]>()
                                    + std::mem::size_of::<Result<(), Diagnostic>>(),
                                "factored inline weighted enum and array receiving",
                            )?;
                        }
                        budget.reserve_bytes(
                            function,
                            physical_clone::rc_backing_bytes::<SmallWeighted>(),
                            "factored weighted shared header",
                        )?;
                        let singleton = SmallWeighted::One([(child, weight)]);
                        Rc::new(singleton)
                    }
                    paid_many::Targets::PaidMany(_) => {
                        budget.reserve_bytes(function, paid_many::FINISH_RECEIVING,
                            "factored composed flat finish receivers")?;
                        let paid_many::Targets::PaidMany(flat) = targets else { unreachable!("paid flat finish") };
                        own_weighted(function, budget, flat)?
                    }
                    paid_many::Targets::Many(targets) => {
                        // The remaining fallible weighted producer still has
                        // its real map argument and Result receiving carrier.
                        if budget.pays_allocations() {
                            budget.reserve_bytes(
                                function,
                                std::mem::size_of::<BTreeMap<usize, u64>>()
                                    + std::mem::size_of::<Vec<(usize, u64)>>()
                                    + std::mem::size_of::<Result<SharedWeighted, Diagnostic>>(),
                                "factored many weighted producer receiving",
                            )?;
                        }
                        weighted_successor(function, budget, targets)?
                    }
                };
                result.push((label, state));
            }
        }
        Ok(Some((accepted, result)))
    }

}

// This separately scoped generation changes SelectedReservation only. These
// controls call the complete immediate predecessor above, never a shipping flag.
#[derive(Debug, Eq, PartialEq)]
struct SelectedFailure {
    code: &'static str,
    severity: crate::diagnostic::Severity,
    message: ([u8; 1024], usize),
    span: Option<crate::ast::Span>,
    path: Option<([u8; 1024], usize)>,
    help: Option<([u8; 1024], usize)>,
}
fn selected_text(text: &str) -> ([u8; 1024], usize) {
    assert!(text.len() <= 1024);
    let mut bytes = [0; 1024];
    bytes[..text.len()].copy_from_slice(text.as_bytes());
    (bytes, text.len())
}
fn selected_failure(error: &Diagnostic) -> SelectedFailure {
    SelectedFailure {
        code: error.code, severity: error.severity,
        message: selected_text(&error.message), span: error.span,
        path: error.path.as_deref().map(selected_text),
        help: error.help.as_deref().map(selected_text),
    }
}
fn selected_state(values: &[(usize, u64)]) -> SharedWeighted {
    Rc::new(SmallWeighted::Many(values.to_vec()))
}
fn selected_successors(
    old: bool, function: &ResolvedFunction, graph: &Automaton, sink: usize,
    input: &SharedWeighted, work: usize,
) -> (Result<Option<(u64, Vec<(Label, SharedWeighted)>)>, Diagnostic>, usize) {
    let mut budget = ReplayBudget::with_physical(function, 1 << 26).unwrap();
    budget.remaining = work;
    budget.reserve_bytes(function, Canonicalizer::CONSTRUCTOR_CONTROLS,
        "selected successor owner receiving").unwrap();
    let mut owner = Canonicalizer::new(graph, sink);
    let result = if old {
        owner.previous_selected_successors(function, input, &mut budget)
    } else { owner.successors(function, input, &mut budget) };
    (result, budget.remaining)
}
#[test]
fn selected_generation_complete_prior_directory_and_new_tree_keep_weights_overflow_and_both_sites() {
    use crate::bounded_output::with_limit_usage_refusal;
    let function = fixture();
    let (graph, sink, roots) = super::tests::automaton(&function);
    with_limit_usage_refusal(usize::MAX, || {
        for root in roots.iter().copied().chain([sink]) {
            for weight in [0, 1, 2, u64::MAX] {
                // One unit root reaches the borrowed/unit call site. A nonunit
                // root or two entries reaches the ordered/general call site.
                for values in [vec![(root, weight)], vec![(root, weight), (sink, 1)]] {
                    let state = selected_state(&values);
                    let (expected, _) = selected_successors(true, &function, &graph, sink, &state, 1 << 20);
                    let (actual, _) = selected_successors(false, &function, &graph, sink, &state, 1 << 20);
                    assert_eq!(actual, expected, "complete labels and weights {values:?}");
                    // Every low-work cut is independently repeatable, including
                    // all six fields. Directory's removed census is NOT offset.
                    for old in [false, true] {
                        for work in 0..=32 {
                            let first = selected_successors(old, &function, &graph, sink, &state, work);
                            let again = selected_successors(old, &function, &graph, sink, &state, work);
                            assert_eq!(first, again, "own generation work cut {work}");
                        }
                    }
                }
            }
        }
        // Same-label repeated child overflow and real distinct-child promotion
        // are separate from closure multiplication overflow in the corpus.
        for distinct in [false,true] {
            let mut setup=ReplayBudget::unobserved();
            let mut graph=Automaton::default();
            let sink=graph.node(&function,&mut setup).unwrap();
            let child=graph.node(&function,&mut setup).unwrap();
            let root=graph.node(&function,&mut setup).unwrap();
            let label=Label::End(super::super::super::SkeletonTerminal::Success);
            graph.movement(&function,&mut setup,root,label.clone(),sink).unwrap();
            graph.movement(&function,&mut setup,root,label,if distinct {child}else{sink}).unwrap();
            for weight in [0,2,u64::MAX] {
                let state=selected_state(&[(root,weight)]);
                let (expected,_)=selected_successors(true,&function,&graph,sink,&state,1<<20);
                let (actual,_)=selected_successors(false,&function,&graph,sink,&state,1<<20);
                assert_eq!(actual,expected);
                if !distinct && weight==u64::MAX {assert!(actual.unwrap().is_none());}
                else {assert!(actual.unwrap().is_some());}
            }
        }
    });
}

#[derive(Debug, Eq, PartialEq)]
struct SelectedObservation {
    result: Result<Option<(u64, usize)>, SelectedFailure>,
    charged: usize,
    work: usize,
}
fn selected_observe(old: bool, function: &ResolvedFunction, graph: &Automaton,
    sink: usize, input: &SharedWeighted, limit: usize, floor: usize,
) -> (SelectedObservation, Metrics) {
    measure(|| {
        assert!(crate::bounded_output::set_active_floor(floor));
        let mut budget = match ReplayBudget::with_physical(function, limit) {
            Ok(value) => value,
            Err(error) => {
                let result = Err(selected_failure(&error)); drop(error);
                crate::bounded_output::clear_active_floor();
                return SelectedObservation { result, charged: 0, work: 0 };
            }
        };
        let result = (|| {
            budget.reserve_bytes(function, Canonicalizer::CONSTRUCTOR_CONTROLS,
                "selected successor owner receiving")?;
            let mut owner = Canonicalizer::new(graph, sink);
            if old { owner.previous_selected_successors(function, input, &mut budget) }
            else { owner.successors(function, input, &mut budget) }
        })();
        let result = match result {
            Ok(value) => Ok(value.map(|(accepted, rows)| (accepted, rows.len()))),
            Err(error) => {
                let first = selected_failure(&error);
                let charged = budget.physical.as_ref().unwrap().charged();
                let remaining = budget.remaining;
                assert!(budget.reserve_bytes(function, 0, "selected refused retry").is_err());
                assert_eq!(budget.physical.as_ref().unwrap().charged(), charged);
                assert_eq!(budget.remaining, remaining);
                assert_eq!(selected_failure(&error), first);
                drop(error); Err(first)
            }
        };
        let charged = budget.physical.as_ref().unwrap().charged();
        let work = budget.remaining;
        drop(budget);
        crate::bounded_output::clear_active_floor();
        SelectedObservation { result, charged, work }
    })
}
#[test]
fn selected_generation_actual_local_and_enclosing_cuts_keep_six_fields_sticky_and_whole_drop() {
    use crate::bounded_output::with_limit_usage_refusal;
    let function = fixture();
    let (graph, sink, roots) = super::tests::automaton(&function);
    for state in [selected_state(&[]), selected_state(&[(sink,1)]),
        selected_state(&[(roots[0],1)]), selected_state(&[(roots[0],2),(roots[1],1)])] {
        for old in [true,false] {
            let ((success, allocation), refused, paid, _) = with_limit_usage_refusal(usize::MAX,
                || selected_observe(old,&function,&graph,sink,&state,1<<26,0));
            assert!(success.result.is_ok() && !refused);
            assert!(!allocation.invalid && allocation.live==0 && allocation.allocated==allocation.freed);
            assert!(allocation.allocated <= paid);
            let exact=success.charged;
            assert!(exact>0);
            for limit in [0,exact/2,exact-1,exact] {
                let observe=|| with_limit_usage_refusal(usize::MAX,
                    || selected_observe(old,&function,&graph,sink,&state,limit,0));
                let ((first, allocation), refused, paid, receipt)=observe();
                assert!(!allocation.invalid && allocation.live==0 && allocation.allocated==allocation.freed);
                if limit==exact { assert!(first.result.is_ok()); } else { assert!(first.result.is_err()); }
                let ((again, allocation), again_refused, again_paid, again_receipt)=observe();
                assert_eq!(again,first); assert_eq!((again_refused,again_paid,again_receipt),(refused,paid,receipt));
                assert!(!allocation.invalid && allocation.live==0 && allocation.allocated==allocation.freed);
            }
            // Same exact positive local allowance, but an enclosing held floor
            // refuses the actual receiver before publication, not a guessed offset.
            let ((failed,allocation),refused,_,receipt)=with_limit_usage_refusal(exact,
                || selected_observe(old,&function,&graph,sink,&state,1<<26,1));
            assert!(failed.result.is_err() && refused && receipt.is_some());
            assert!(!allocation.invalid && allocation.live==0 && allocation.allocated==allocation.freed);
        }
    }
}

#[test]
fn selected_generation_foreign_function_budget_and_scope_never_revive_bound_receiver() {
    use crate::bounded_output::with_limit_usage_refusal;
    let function=fixture(); let wrong=function.clone();
    let (graph,sink,roots)=super::tests::automaton(&function);
    let state=selected_state(&[(roots[0],1)]);
    for old in [true,false] { for attack in 0..3 {
        with_limit_usage_refusal(usize::MAX,|| {
            let mut budget=ReplayBudget::with_physical(&function,1<<26).unwrap();
            let mut foreign=ReplayBudget::with_physical(&function,1<<26).unwrap();
            budget.reserve_bytes(&function,Canonicalizer::CONSTRUCTOR_CONTROLS,
                "selected foreign owner receiving").unwrap();
            let mut owner=Canonicalizer::new(&graph,sink);
            let initial=if old {owner.previous_selected_successors(&function,&state,&mut budget)}
                else {owner.successors(&function,&state,&mut budget)};
            assert!(initial.unwrap().is_some());
            let result=if attack==0 {
                if old {owner.previous_selected_successors(&wrong,&state,&mut budget)}
                else {owner.successors(&wrong,&state,&mut budget)}
            } else if attack==1 {
                if old {owner.previous_selected_successors(&function,&state,&mut foreign)}
                else {owner.successors(&function,&state,&mut foreign)}
            } else {
                with_limit_usage_refusal(1<<26,|| {
                    if old {owner.previous_selected_successors(&function,&state,&mut budget)}
                    else {owner.successors(&function,&state,&mut budget)}
                }).0
            };
            let error=result.unwrap_err();
            let frozen=selected_failure(&error);
            assert_eq!(frozen,selected_failure(&Diagnostic::io("SPX-H006",String::new())));
            let restored=if old {owner.previous_selected_successors(&function,&state,&mut budget)}
                else {owner.successors(&function,&state,&mut budget)};
            assert_eq!(selected_failure(&restored.unwrap_err()),frozen);
            assert_eq!(selected_failure(&error),frozen);
            assert!(!foreign.refusal_spent);
        });
    }}
}

#[test]
fn selected_generation_retains_complete_immediate_methods_and_original_arena_gate() {
    let original=include_str!("../../../../../docs/evidence/selected-successor-generation-20261017/before/src/cleanup_plan/replay/factored/canonical_shared.rs");
    let current=include_str!("../canonical_shared.rs");
    let acquired=crate::source_reflows_merge::main67107_inputs::canonical_import_input("src/cleanup_plan/replay/factored/canonical_shared.rs",current);
    let current=acquired.as_str();
    let restored=current.replace("                AllocationPayer::SelectedReservation => {}\n","")
        .replace("            AllocationPayer::SelectedReservation => {}\n","");
    assert_eq!(restored,original);
    let tests=include_str!("arena_tests.rs");
    let oldtests=include_str!("../../../../../docs/evidence/selected-successor-generation-20261017/before/src/cleanup_plan/replay/factored/canonical_shared/arena_tests.rs");
    assert!(tests.starts_with(oldtests));
    for (start,end) in [("    fn successors(\n","    #[cfg(test)]\n    fn closure("),
        ("    fn successors_with_receiving(\n","    pub(super) fn id_with_receiving(")] {
        let offset=original.find(start).unwrap();
        let body=&original[offset..offset+original[offset..].find(end).unwrap()];
        let renamed=body.replace("fn successors(","fn previous_selected_successors(")
            .replace("fn successors_with_receiving(","fn previous_selected_successors_with_receiving(")
            .replace("self.successors_with_receiving(","self.previous_selected_successors_with_receiving(");
        assert_eq!(tests.matches(&renamed).count(),1);
    }
}
