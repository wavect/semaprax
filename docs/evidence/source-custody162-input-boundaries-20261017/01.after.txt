//! MAIN borrowed sorted outer labels composed with current inlineOne/paidMany.
//! Labels borrow only immutable automaton cells. Legacy outer trees remain in
//! both actual successor branches; this directory is selected only for payers.
use super::*;
use crate::cleanup_plan::replay::physical_storage;
use std::mem::size_of;

type Successors = (u64, Vec<(Label, SharedWeighted)>);
pub(super) trait Closed: 'static {
    type Iter<'a>: Iterator<Item = (&'a usize, &'a u64)> where Self: 'a;
    fn ordered(&self) -> Self::Iter<'_>;
}
impl Closed for ordered_weights::OrderedWeights {
    type Iter<'a> = ordered_weights::Iter<'a>;
    fn ordered(&self) -> Self::Iter<'_> { self.iter() }
}
#[cfg(test)]
impl Closed for BTreeMap<usize, u64> {
    type Iter<'a> = std::collections::btree_map::Iter<'a, usize, u64>;
    fn ordered(&self) -> Self::Iter<'_> { self.iter() }
}
// Actual forwarded call/return carriers and both real closed iterators. The
// actual unit/general callers acquire these before forming this new call.
pub(super) fn receiving<C: Closed>() -> usize {
    2 * size_of::<(&ResolvedFunction, &mut ReplayBudget, &Automaton, &C, u64)>()
        + 2 * size_of::<Result<Option<Successors>, Diagnostic>>()
        + size_of::<Option<Successors>>()
        + 2 * size_of::<C::Iter<'static>>()
        + 2 * size_of::<std::slice::Iter<'static, (Label, usize)>>()
        + 2 * size_of::<(&usize, &u64)>()
        + 2 * size_of::<(&Label, &usize)>()
        + 4 * size_of::<Result<(), Diagnostic>>()
}
struct Directory<'a> {
    keys: Vec<&'a Label>,
    values: Vec<Option<paid_many::Targets>>,
}
impl<'a> Directory<'a> {
    fn new(function: &ResolvedFunction, budget: &mut ReplayBudget) -> Result<Self, Diagnostic> {
        budget.reserve_bytes(function,
            2 * size_of::<Self>() + 2 * size_of::<Result<Self, Diagnostic>>(),
            "factored label directory headers and receiving")?;
        Ok(Self { keys: Vec::new(), values: Vec::new() })
    }
    fn key(&mut self, function: &ResolvedFunction, budget: &mut ReplayBudget, label: &'a Label) -> Result<(), Diagnostic> {
        budget.reserve_bytes(function, 0, "factored label directory authentication")?;
        let comparisons = (usize::BITS - self.keys.len().leading_zeros()) as usize + 1;
        budget.reserve_bytes(function,
            2 * size_of::<(&mut Self, &ResolvedFunction, &mut ReplayBudget, &Label)>()
                + 2 * size_of::<Result<usize, usize>>()
                + comparisons * (size_of::<&&Label>() + size_of::<std::cmp::Ordering>())
                + 3 * size_of::<usize>() + size_of::<u32>(),
            "factored borrowed label census receivers")?;
        budget.charge(function, comparisons, "factored successor label census search")?;
        if let Err(index) = self.keys.binary_search_by(|key| (*key).cmp(label)) {
            budget.charge(function, self.keys.len() - index + 1, "factored successor label census shifts")?;
            budget.reserve_vec(function, &mut self.keys, 1, "factored successor borrowed labels")?;
            self.keys.insert(index, label);
        }
        Ok(())
    }
    fn prepare(&mut self, function: &ResolvedFunction, budget: &mut ReplayBudget) -> Result<(), Diagnostic> {
        budget.reserve_bytes(function,
            2 * size_of::<(&mut Self, &ResolvedFunction, &mut ReplayBudget)>()
                + size_of::<Option<paid_many::Targets>>()
                + size_of::<std::slice::Iter<'_, &'a Label>>()
                + 2 * size_of::<Result<(), Diagnostic>>(),
            "factored successor target table receivers")?;
        budget.charge(function, self.keys.len(), "factored successor target headers")?;
        budget.reserve_vec(function, &mut self.values, self.keys.len(), "factored successor target table")?;
        for _ in &self.keys { self.values.push(None); }
        Ok(())
    }
    fn add(&mut self, function: &ResolvedFunction, budget: &mut ReplayBudget, label: &Label, child: usize, weight: u64) -> Result<bool, Diagnostic> {
        budget.reserve_bytes(function, 0, "factored label target authentication")?;
        let comparisons = (usize::BITS - self.keys.len().leading_zeros()) as usize + 1;
        budget.reserve_bytes(function,
            2 * size_of::<(&mut Self, &ResolvedFunction, &mut ReplayBudget, &Label, usize, u64)>()
                + 2 * size_of::<Result<usize, usize>>()
                + comparisons * (size_of::<&&Label>() + size_of::<std::cmp::Ordering>())
                + 3 * size_of::<usize>() + size_of::<u32>()
                + size_of::<&mut Option<paid_many::Targets>>()
                + 2 * size_of::<Option<u64>>()
                + size_of::<Option<paid_many::Targets>>() + size_of::<paid_many::Targets>(),
            "factored borrowed label target receivers")?;
        budget.charge(function, comparisons, "factored successor label search")?;
        let index = self.keys.binary_search_by(|key| (*key).cmp(label)).map_err(|_| {
            budget.budget_refusal(function, "immutable successor label census changed", "")
        })?;
        let slot = &mut self.values[index];
        match slot {
            None => {
                budget.reserve_bytes(function, 0, "factored singleton target authentication")?;
                let weight = 0_u64.checked_add(weight).expect("zero plus u64 cannot overflow");
                *slot = Some(paid_many::Targets::One { child, weight });
            }
            Some(paid_many::Targets::One { child: first, weight: total }) if *first == child => {
                // Keep repeated-child overflow before any promotion receiver.
                let Some(sum) = total.checked_add(weight) else { return Ok(false); };
                *total = sum;
            }
            Some(paid_many::Targets::One { child: first, weight: total }) => {
                budget.reserve_bytes(function, paid_many::PROMOTE_RECEIVING, "factored composed promotion receivers")?;
                #[cfg(test)]
                if budget.pays_allocations() { paid_many::note_branch(true); }
                *slot = Some(paid_many::promote(function, budget, *first, *total, child, weight)?);
            }
            Some(paid_many::Targets::PaidMany(flat)) => {
                budget.reserve_bytes(function, paid_many::ADD_RECEIVING, "factored composed flat add receivers")?;
                if !paid_many::add(function, budget, flat, child, weight)? { return Ok(false); }
            }
            Some(paid_many::Targets::Many(targets)) => {
                // Reachable only in direct unobserved compatibility controls.
                if !targets.contains_key(&child) {
                    budget.reserve_btree_node::<usize, u64>(function, "factored successor target node")?;
                }
                let target = targets.entry(child).or_insert(0);
                let Some(sum) = target.checked_add(weight) else { return Ok(false); };
                *target = sum;
            }
        }
        Ok(true)
    }
    fn finish(self, function: &ResolvedFunction, budget: &mut ReplayBudget) -> Result<Vec<(Label, SharedWeighted)>, Diagnostic> {
        budget.reserve_bytes(function,
            2 * size_of::<Self>()
                + 2 * size_of::<Result<Vec<(Label, SharedWeighted)>, Diagnostic>>()
                + size_of::<std::iter::Zip<std::vec::IntoIter<&'a Label>, std::vec::IntoIter<Option<paid_many::Targets>>>>()
                + size_of::<Vec<(Label, SharedWeighted)>>()
                + size_of::<(&Label, Option<paid_many::Targets>)>()
                + 2 * size_of::<SharedWeighted>() + size_of::<(Label, SharedWeighted)>(),
            "factored borrowed label publication receivers")?;
        let mut result = Vec::new();
        budget.reserve_vec(function, &mut result, self.keys.len(), "factored successor vector")?;
        for (label, targets) in self.keys.into_iter().zip(self.values) {
            let state = match targets {
                Some(paid_many::Targets::One { child, weight }) => {
                    budget.reserve_bytes(function,
                        2 * size_of::<SmallWeighted>() + size_of::<[(usize, u64); 1]>()
                            + size_of::<Result<(), Diagnostic>>(),
                        "factored inline weighted enum and array receiving")?;
                    budget.reserve_bytes(function, physical_clone::rc_backing_bytes::<SmallWeighted>(), "factored weighted shared header")?;
                    Rc::new(SmallWeighted::One([(child, weight)]))
                }
                Some(paid_many::Targets::PaidMany(flat)) => {
                    budget.reserve_bytes(function, paid_many::FINISH_RECEIVING, "factored composed flat finish receivers")?;
                    own_weighted(function, budget, flat)?
                }
                Some(paid_many::Targets::Many(targets)) => weighted_successor(function, budget, targets)?,
                None => return Err(budget.budget_refusal(function, "immutable successor label census changed", "")),
            };
            // Only an Rc observation or Copy terminal: no label payload clone.
            result.push((label.clone(), state));
        }
        Ok(result)
    }
}
pub(super) fn successors<C: Closed>(function: &ResolvedFunction, budget: &mut ReplayBudget, automaton: &Automaton, closed: &C, accepted: u64) -> Result<Option<Successors>, Diagnostic> {
    let mut labels = Directory::new(function, budget)?;
    for (node, _) in closed.ordered() {
        let moves = automaton.nodes[*node].moves();
        budget.charge(function, moves.len().saturating_add(1), "factored successor label census")?;
        for (label, _) in moves { labels.key(function, budget, label)?; }
    }
    labels.prepare(function, budget)?;
    for (node, weight) in closed.ordered() {
        let moves = automaton.nodes[*node].moves();
        budget.charge(function, moves.len().saturating_add(1), "factored skeleton subset construction")?;
        for (label, child) in moves {
            if !labels.add(function, budget, label, *child, *weight)? { return Ok(None); }
        }
    }
    Ok(Some((accepted, labels.finish(function, budget)?)))
}

#[test]
fn joined_outer_directory_keeps_inline_repetition_and_distinct_paid_promotion() {
    let function = super::super::lexical_plan_tests::fixture(super::super::lexical_plan_tests::WHILE).1;
    crate::bounded_output::with_limit_usage_refusal(usize::MAX, || {
        let mut budget = ReplayBudget::new();
        let label = Label::End(super::super::SkeletonTerminal::Success);
        let mut directory = Directory::new(&function, &mut budget).unwrap();
        directory.key(&function, &mut budget, &label).unwrap();
        directory.prepare(&function, &mut budget).unwrap();
        assert!(directory.add(&function, &mut budget, &label, 5, 0).unwrap());
        assert!(matches!(directory.values[0], Some(paid_many::Targets::One { child: 5, weight: 0 })));
        assert!(directory.add(&function, &mut budget, &label, 5, 2).unwrap());
        assert!(matches!(directory.values[0], Some(paid_many::Targets::One { child: 5, weight: 2 })));
        assert!(directory.add(&function, &mut budget, &label, 1, 3).unwrap());
        assert!(matches!(directory.values[0], Some(paid_many::Targets::PaidMany(_))));
        let result = directory.finish(&function, &mut budget).unwrap();
        assert_eq!(result[0].1.as_ref().as_slice(), &[(1, 3), (5, 2)]);
        let mut directory = Directory::new(&function, &mut budget).unwrap();
        directory.key(&function, &mut budget, &label).unwrap();
        directory.prepare(&function, &mut budget).unwrap();
        assert!(directory.add(&function, &mut budget, &label, 9, u64::MAX).unwrap());
        assert!(!directory.add(&function, &mut budget, &label, 9, 1).unwrap());
        assert!(matches!(directory.values[0], Some(paid_many::Targets::One { child: 9, weight: u64::MAX })));
    });
}
#[test]
fn joined_outer_headers_and_target_slots_use_actual_layouts() {
    struct PreviousDirectory<'a> { keys: Vec<&'a Label>, values: Vec<Weighted> }
    let previous = PreviousDirectory { keys: Vec::new(), values: Vec::new() };
    assert!(previous.keys.is_empty() && previous.values.is_empty());
    assert_eq!(size_of::<Directory<'_>>(), size_of::<PreviousDirectory<'_>>());
    assert_eq!(size_of::<&Label>(), size_of::<usize>());
    // Slots may widen relative to MAIN's eager Vec headers. prepare reserves
    // the actual Option<Targets> physical table; no old-header credit is used.
    assert!(size_of::<Option<paid_many::Targets>>() >= size_of::<paid_many::Targets>());
    assert!(receiving::<ordered_weights::OrderedWeights>() >= 2 * size_of::<ordered_weights::Iter<'_>>());
}


// The existing pays_allocations query is replaced by a borrowed payer tag.
// No payload copy or wider authority-query frame is introduced.
const _: () = assert!(size_of::<&AllocationPayer>() + size_of::<bool>()
    <= size_of::<&ReplayBudget>() + size_of::<bool>());
// The old Directory producer above remains an independent, unchanged receiver.
// All actual arguments, branch/iterator slots and returned/received headers
// below are paid by the actual caller before this function is entered.
pub(super) fn ordinary_receiving() -> usize {
    crate::bounded_output::classifier_string::ClassifierLedger::current_receiving_controls()
        // The held-current mismatch may record the first terminal receipt.
        // These complete existing inventories include its receipt/site/stage
        // constructors; no old generic producer allowance funds this branch.
        + crate::bounded_output::classifier_string::ClassifierLedger::insertion_terminal_controls()
        + crate::bounded_output::classifier_string::ClassifierLedger::storage_terminal_receipt_controls()
        // Two nested callback captures: caller and receiving function borrow.
        + 4 * size_of::<&ResolvedFunction>()
        // Mutable selected-ledger lookup, expect transport, and sticky reject.
        + 2 * size_of::<&mut Option<Box<physical_storage::PhysicalReservation>>>()
        + 2 * size_of::<Option<&mut physical_storage::PhysicalReservation>>()
        + 2 * size_of::<&mut physical_storage::PhysicalReservation>()
        + 2 * size_of::<&str>()
        + 2 * size_of::<Option<physical_storage::PhysicalRefusal>>()
        + size_of::<physical_storage::PhysicalRefusal>()
        + 2 * size_of::<&Option<crate::cleanup_plan::replay::SelectedDirectAnchor>>()
        + 2 * size_of::<Option<&crate::cleanup_plan::replay::SelectedDirectAnchor>>()
        + 2 * size_of::<&Option<Box<physical_storage::PhysicalReservation>>>()
        + 2 * size_of::<Option<&physical_storage::PhysicalReservation>>()
        + 2 * size_of::<&physical_storage::PhysicalReservation>()
        + 2 * size_of::<&crate::cleanup_plan::replay::SelectedDirectAnchor>()
        + 2 * size_of::<*const ResolvedFunction>()
        + 2 * size_of::<&AllocationPayer>()
        + 2 * size_of::<&ResolvedFunction>()
        + 2 * size_of::<&crate::bounded_output::classifier_string::ClassifierLedger>()
        + 2 * size_of::<&str>()
        + 2 * size_of::<String>()
        + 2 * size_of::<Diagnostic>()
        + 12 * size_of::<bool>()
        + 2 * size_of::<(&ResolvedFunction, &mut ReplayBudget, &Automaton, &ordered_weights::OrderedWeights, u64)>()
        + 2 * size_of::<Result<Option<Successors>, Diagnostic>>()
        + size_of::<Option<Successors>>()
        + 2 * size_of::<ordered_weights::Iter<'static>>()
        + size_of::<&ordered_weights::OrderedWeights>()
        + size_of::<&mut ordered_weights::Iter<'static>>()
        + 2 * size_of::<Option<(&usize, &u64)>>()
        + 2 * size_of::<(&usize, &u64)>() + 2 * size_of::<&str>()
        + 2 * size_of::<&[(Label, usize)]>()
        + size_of::<&(Label, usize)>() + size_of::<(&Label, &usize)>()
        + 2 * size_of::<Vec<(Label, SharedWeighted)>>()
        + 2 * size_of::<SharedWeighted>() + size_of::<(Label, SharedWeighted)>()
        + 2 * size_of::<Option<u64>>() + 4 * size_of::<u64>()
        + 3 * size_of::<usize>() + 2 * size_of::<bool>()
        + 4 * size_of::<Result<(), Diagnostic>>()
        + 4 * size_of::<(&ResolvedFunction, &mut ReplayBudget, usize, &str)>()
}

pub(super) fn ordinary_successors(
    function: &ResolvedFunction, budget: &mut ReplayBudget, automaton: &Automaton,
    closed: &ordered_weights::OrderedWeights, accepted: u64,
) -> Result<Option<Successors>, Diagnostic> {
    // The general replay budget intentionally permits multiple functions.
    // This direct selected compatibility receiver instead authenticates the
    // exact setup anchor before querying its payer or spending logical work.
    if matches!(&budget.payer, AllocationPayer::SelectedReservation)
        && (budget.refusal_spent || budget.physical.as_deref().is_none_or(|physical| {
            physical.direct_anchor.as_ref().is_none_or(|anchor| {
                !anchor.ledger.current() || anchor.function != std::ptr::from_ref(function)
            })
        })) {
            budget.refusal_spent = true;
            budget.physical.as_deref_mut().expect("selected reservation payer").reject();
            return Err(Diagnostic::io("SPX-H006", String::new()));
        }
    // These zero calls retain sticky/function/ledger authentication even for a
    // singleton sink. A paid refusal is final and never selects the old route.
    budget.reserve_bytes(function, 0, "factored direct singleton authentication")?;
    budget.charge(function, 0, "factored direct singleton authentication")?;
    budget.charge(function, 4, "factored direct singleton selection")?;
    if closed.len() == 1 {
        let (node, weight) = closed.iter().next().expect("singleton closed node");
        let moves = automaton.nodes[*node].moves();
        if moves.len() <= 1 {
            // Replay the original requests in their original order. The old
            // directory/search objects themselves are never formed or paid.
            budget.charge(function, moves.len() + 1, "factored successor label census")?;
            if !moves.is_empty() {
                budget.charge(function, 1, "factored successor label census search")?;
                budget.charge(function, 1, "factored successor label census shifts")?;
            }
            budget.charge(function, moves.len(), "factored successor target headers")?;
            budget.charge(function, moves.len() + 1, "factored skeleton subset construction")?;
            if !moves.is_empty() {
                budget.charge(function, 2, "factored successor label search")?;
            }
            let mut result = Vec::new();
            budget.reserve_vec(function, &mut result, moves.len(), "factored successor vector")?;
            if let [(label, child)] = moves {
                // Zero + the actual closure weight has the same non-overflow
                // rule as Directory::add. No assumed unit weight is substituted.
                let weight = 0_u64.checked_add(*weight).expect("zero plus u64 cannot overflow");
                budget.reserve_bytes(function,
                    2 * size_of::<SmallWeighted>() + size_of::<[(usize, u64); 1]>()
                        + size_of::<Result<(), Diagnostic>>(),
                    "factored inline weighted enum and array receiving")?;
                budget.reserve_bytes(function, physical_clone::rc_backing_bytes::<SmallWeighted>(),
                    "factored weighted shared header")?;
                let state = Rc::new(SmallWeighted::One([(*child, weight)]));
                result.push((label.clone(), state));
            }
            return Ok(Some((accepted, result)));
        }
    }
    // Multi-node/multi-movement closures retain the complete old algorithm.
    // Its additional call/receivers are acquired before forming that call.
    budget.reserve_bytes(function, receiving::<ordered_weights::OrderedWeights>(),
        "factored borrowed label fallback call receivers")?;
    lexical_finish::successors(function, budget, automaton, closed, accepted)
}

#[cfg(test)]
pub(super) fn ordinary_successors_before_selected_anchor(
    function: &ResolvedFunction, budget: &mut ReplayBudget, automaton: &Automaton,
    closed: &ordered_weights::OrderedWeights, accepted: u64,
) -> Result<Option<Successors>, Diagnostic> {
    // These zero calls retain sticky/function/ledger authentication even for a
    // singleton sink. A paid refusal is final and never selects the old route.
    budget.reserve_bytes(function, 0, "factored direct singleton authentication")?;
    budget.charge(function, 0, "factored direct singleton authentication")?;
    budget.charge(function, 4, "factored direct singleton selection")?;
    if closed.len() == 1 {
        let (node, weight) = closed.iter().next().expect("singleton closed node");
        let moves = automaton.nodes[*node].moves();
        if moves.len() <= 1 {
            // Replay the original requests in their original order. The old
            // directory/search objects themselves are never formed or paid.
            budget.charge(function, moves.len() + 1, "factored successor label census")?;
            if !moves.is_empty() {
                budget.charge(function, 1, "factored successor label census search")?;
                budget.charge(function, 1, "factored successor label census shifts")?;
            }
            budget.charge(function, moves.len(), "factored successor target headers")?;
            budget.charge(function, moves.len() + 1, "factored skeleton subset construction")?;
            if !moves.is_empty() {
                budget.charge(function, 2, "factored successor label search")?;
            }
            let mut result = Vec::new();
            budget.reserve_vec(function, &mut result, moves.len(), "factored successor vector")?;
            if let [(label, child)] = moves {
                // Zero + the actual closure weight has the same non-overflow
                // rule as Directory::add. No assumed unit weight is substituted.
                let weight = 0_u64.checked_add(*weight).expect("zero plus u64 cannot overflow");
                budget.reserve_bytes(function,
                    2 * size_of::<SmallWeighted>() + size_of::<[(usize, u64); 1]>()
                        + size_of::<Result<(), Diagnostic>>(),
                    "factored inline weighted enum and array receiving")?;
                budget.reserve_bytes(function, physical_clone::rc_backing_bytes::<SmallWeighted>(),
                    "factored weighted shared header")?;
                let state = Rc::new(SmallWeighted::One([(*child, weight)]));
                result.push((label.clone(), state));
            }
            return Ok(Some((accepted, result)));
        }
    }
    // Multi-node/multi-movement closures retain the complete old algorithm.
    // Its additional call/receivers are acquired before forming that call.
    budget.reserve_bytes(function, receiving::<ordered_weights::OrderedWeights>(),
        "factored borrowed label fallback call receivers")?;
    lexical_finish::successors(function, budget, automaton, closed, accepted)
}


#[cfg(test)]
pub(super) fn ordinary_receiving_before_selected_anchor() -> usize {
    2 * size_of::<(&ResolvedFunction, &mut ReplayBudget, &Automaton, &ordered_weights::OrderedWeights, u64)>()
        + 2 * size_of::<Result<Option<Successors>, Diagnostic>>()
        + size_of::<Option<Successors>>()
        + 2 * size_of::<ordered_weights::Iter<'static>>()
        + size_of::<&ordered_weights::OrderedWeights>()
        + size_of::<&mut ordered_weights::Iter<'static>>()
        + 2 * size_of::<Option<(&usize, &u64)>>()
        + 2 * size_of::<(&usize, &u64)>() + 2 * size_of::<&str>()
        + 2 * size_of::<&[(Label, usize)]>()
        + size_of::<&(Label, usize)>() + size_of::<(&Label, &usize)>()
        + 2 * size_of::<Vec<(Label, SharedWeighted)>>()
        + 2 * size_of::<SharedWeighted>() + size_of::<(Label, SharedWeighted)>()
        + 2 * size_of::<Option<u64>>() + 4 * size_of::<u64>()
        + 3 * size_of::<usize>() + 2 * size_of::<bool>()
        + 4 * size_of::<Result<(), Diagnostic>>()
        + 4 * size_of::<(&ResolvedFunction, &mut ReplayBudget, usize, &str)>()
}

#[cfg(test)]
fn direct_fixture(width: usize, nodes: usize, weight: u64) -> (ResolvedFunction, Automaton, ordered_weights::OrderedWeights) {
    direct_fixture_label(width,nodes,weight,&Label::End(super::super::SkeletonTerminal::Success))
}
#[cfg(test)]
fn direct_fixture_label(width: usize, nodes: usize, weight: u64, label: &Label) -> (ResolvedFunction, Automaton, ordered_weights::OrderedWeights) {
    let function = super::super::lexical_plan_tests::fixture(super::super::lexical_plan_tests::WHILE).1;
    let mut budget = ReplayBudget::unobserved();
    let mut graph = Automaton::default();
    let sink = graph.node(&function, &mut budget).unwrap();
    let root = graph.node(&function, &mut budget).unwrap();
    for _ in 0..width {
        graph.movement(&function, &mut budget, root,
            label.clone(), sink).unwrap();
    }
    let mut closed = ordered_weights::OrderedWeights::new();
    closed.insert_prepaid(if width == 0 { sink } else { root }, weight);
    if nodes > 1 {
        closed.grow(&function, &mut budget).unwrap();
        closed.insert_prepaid(if width == 0 { root } else { sink }, weight);
    }
    (function, graph, closed)
}

#[cfg(test)]
fn same_successors(left: &Result<Option<Successors>, Diagnostic>, right: &Result<Option<Successors>, Diagnostic>) {
    match (left, right) {
        (Ok(Some((a, x))), Ok(Some((b, y)))) => {
            assert_eq!(a,b);assert_eq!(x.len(),y.len());
            for ((la,sa),(lb,sb)) in x.iter().zip(y) {
                assert_eq!(la,lb);assert_eq!(sa.as_ref().as_slice(),sb.as_ref().as_slice());
            }
        }
        (Ok(None),Ok(None)) => {}
        (Err(a),Err(b)) => {
            assert_eq!((&a.code,&a.severity,&a.message,&a.span,&a.path,&a.help),
                (&b.code,&b.severity,&b.message,&b.span,&b.path,&b.help));
        }
        _ => panic!("complete successor result changed"),
    }
}

#[test]
fn direct_singleton_complete_weights_labels_work_cuts_and_fallback_keep_old_receiver() {
    use crate::bounded_output::with_limit_usage_refusal;
    for width in [0,1,2,4] {
        for nodes in [1,2] {
            for weight in [0,1,u64::MAX] {
                let (function, graph, closed) = direct_fixture(width,nodes,weight);
                let full = if nodes==1 && width<=1 { if width==0 {2} else {9} } else {128};
                for work in 0..=full {
                    let ((expected, old_remaining),_,_,_) = with_limit_usage_refusal(1<<25, || {
                        let mut old=ReplayBudget::new();old.remaining=work;
                        old.reserve_bytes(&function,receiving::<ordered_weights::OrderedWeights>(),"direct test old caller receiving").unwrap();
                        let result=successors(&function,&mut old,&graph,&closed,weight);
                        (result,old.remaining)
                    });
                    with_limit_usage_refusal(1<<25, || {
                        let mut budget=ReplayBudget::new();budget.remaining=work+4;
                        budget.reserve_bytes(&function,ordinary_receiving(),"direct test actual caller receiving").unwrap();
                        let actual=ordinary_successors(&function,&mut budget,&graph,&closed,weight);
                        same_successors(&actual,&expected);
                        assert_eq!(budget.remaining,old_remaining);
                        if actual.is_err() {
                            let remaining=budget.remaining;
                            let bytes=crate::bounded_output::active_remaining();
                            let retry=ordinary_successors(&function,&mut budget,&graph,&closed,weight);
                            assert!(retry.is_err());assert_eq!(budget.remaining,remaining);
                            assert_eq!(crate::bounded_output::active_remaining(),bytes);drop(retry);
                        }
                        drop(actual);
                    });
                }
            }
        }
    }
}

#[test]
fn direct_singleton_real_paid_allocated_peak_exact_short_zero_and_full_drop() {
    use crate::bounded_output::with_limit_usage_refusal;
    use crate::test_allocation_observer::measure;
    for width in [0,1,2,4] {
        for nodes in [1,2] {
            let (function,graph,closed)=direct_fixture(width,nodes,7);
            let mut old=ReplayBudget::unobserved();
            let expected=successors(&function,&mut old,&graph,&closed,11);
            let observe=|direct:bool, limit:usize| {
                let ((success, allocation),refused,debit,_) = with_limit_usage_refusal(limit, || measure(|| {
                    let mut budget=ReplayBudget::new();
                    let result=(|| {
                        budget.reserve_bytes(&function,if direct {ordinary_receiving()} else {receiving::<ordered_weights::OrderedWeights>()},"direct test actual caller receiving")?;
                        if direct { ordinary_successors(&function,&mut budget,&graph,&closed,11) }
                        else { successors(&function,&mut budget,&graph,&closed,11) }
                    })();
                    let success=result.is_ok();
                    if success {same_successors(&result,&expected);}
                    else {assert_eq!(result.as_ref().unwrap_err().code,"SPX-H006");}
                    drop(result);drop(budget);success
                }));
                assert!(!allocation.invalid);assert_eq!(allocation.live,0);
                assert_eq!(allocation.allocated,allocation.freed);
                (success,refused,debit,allocation)
            };
            let before=observe(false,1<<25);let current=observe(true,1<<25);
            assert!(before.0 && current.0 && !before.1 && !current.1);
            assert!(before.3.allocated<=before.2 && current.3.allocated<=current.2);
            assert!(before.3.highwater<=before.2 && current.3.highwater<=current.2);
            if nodes==1 && width==1 {assert!(current.3.allocations<before.3.allocations);}
            for limit in 0..=current.2 {
                let result=observe(true,limit);assert_eq!(result.0,limit==current.2);
                assert_eq!(result.1,limit!=current.2);
            }
            eprintln!("direct singleton nodes={nodes} moves={width} old/current={before:?}/{current:?}");
        }
    }
}


#[test]
fn direct_singleton_zero_authentication_foreign_owner_and_sticky_refusal_are_final() {
    let (function,graph,closed)=direct_fixture(1,1,3);
    let mut foreign=function.clone();foreign.id=crate::hir::DeclarationId::new("direct.foreign");
    let mut budget=ReplayBudget::with_physical(&function,1<<25).unwrap();
    // Acquire both actual call boundaries under the held owner before refusal.
    budget.reserve_bytes(&function, 2 * ordinary_receiving(), "selected direct two-call receiving controls").unwrap();
    let remaining=budget.remaining;
    let error=ordinary_successors(&foreign,&mut budget,&graph,&closed,5).unwrap_err();
    assert_eq!(error.code,"SPX-H006");assert_eq!(budget.remaining,remaining);
    let after=budget.physical.as_ref().unwrap().charged();
    let error2=ordinary_successors(&function,&mut budget,&graph,&closed,5).unwrap_err();
    assert_eq!(error2.code,"SPX-H006");assert_eq!(budget.remaining,remaining);
    assert_eq!(budget.physical.as_ref().unwrap().charged(),after);
}


#[test]
fn direct_singleton_actual_canonical_calls_keep_complete_ids_signatures_and_states() {
    for width in [0,1,2,4] {
        let (function,graph,closed)=direct_fixture(width,1,1);
        let root=*closed.iter().next().unwrap().0;
        for weight in [0,1,7,u64::MAX] {
            let mut old=Canonicalizer::new(&graph,0);
            let mut old_budget=ReplayBudget::unobserved();
            let expected=old.id(&function,vec![(root,weight)],&mut old_budget);
            crate::bounded_output::with_limit_usage_refusal(1<<26, || {
                let mut current=Canonicalizer::new(&graph,0);
                let mut budget=ReplayBudget::new();
                let actual=current.id(&function,vec![(root,weight)],&mut budget);
                assert_eq!(actual,expected);
                assert!(current.ids.iter().eq(old.ids.iter()));
                assert!(current.states.iter().eq(old.states.iter()));
                drop(actual);drop(current);drop(budget);
            });
        }
    }
}


#[test]
fn direct_singleton_complete_directory_preimage_remains_byte_exact() {
    let before=include_str!(concat!(env!("CARGO_MANIFEST_DIR"),"/docs/evidence/ordinary-singleton-successors-20261017/before/src/cleanup_plan/replay/factored/canonical_shared/borrowed_labels.rs"));
    let current=include_str!("borrowed_labels.rs");
    let acquired=crate::source_reflows_merge::main67107_inputs::semantic_batch_previous(
        "src/cleanup_plan/replay/factored/canonical_shared/borrowed_labels.rs",current,"base851");
    let current=acquired.as_str();
    assert!(current.starts_with(before));
    assert_eq!(current.matches(concat!("pub(super) fn ", "successors<C: Closed>")).count(),1);
    assert_eq!(current.matches(concat!("struct ", "Directory<'a>")).count(),1);
}


#[test]
fn direct_singleton_every_terminal_and_immutable_observation_keep_complete_labels() {
    let observation=Rc::new(super::super::SkeletonObservation::Boolean {
        expression:crate::hir::ExpressionId::from_owned("direct.singleton.é".into()),value:true,
    });
    for label in [Label::End(super::super::SkeletonTerminal::Success),
        Label::End(super::super::SkeletonTerminal::Failure),
        Label::End(super::super::SkeletonTerminal::LoopLatch),Label::Observation(observation.clone())] {
        for weight in [0,1,u64::MAX] {
            let (function,graph,closed)=direct_fixture_label(1,1,weight,&label);
            let mut old=ReplayBudget::unobserved();
            let expected=successors(&function,&mut old,&graph,&closed,weight);
            crate::bounded_output::with_limit_usage_refusal(1<<25, || {
                let mut budget=ReplayBudget::new();
                budget.reserve_bytes(&function,ordinary_receiving(),"direct test actual caller receiving").unwrap();
                let actual=ordinary_successors(&function,&mut budget,&graph,&closed,weight);
                same_successors(&actual,&expected);
                if let Label::Observation(input)=&label {
                    let Label::Observation(output)=&actual.as_ref().unwrap().as_ref().unwrap().1[0].0 else {panic!("label kind changed")};
                    assert!(Rc::ptr_eq(input,output));
                }
                drop(actual);
            });
        }
    }
}


#[test]
fn direct_singleton_new_selection_zero_through_three_keeps_six_fields_and_sticky_cut() {
    let (function,graph,closed)=direct_fixture(1,1,1);
    for work in 0..4 {
        crate::bounded_output::with_limit_usage_refusal(1<<25, || {
            let mut budget=ReplayBudget::new();budget.remaining=work;
            budget.reserve_bytes(&function,ordinary_receiving(),"direct test actual caller receiving").unwrap();
            let actual=ordinary_successors(&function,&mut budget,&graph,&closed,1);
            let expected=Err(Diagnostic::io("SPX-H006",format!(
                "cleanup plan for function `{}` failed independent replay: cleanup replay work budget exhausted during factored direct singleton selection",function.id)));
            same_successors(&actual,&expected);
            assert!(budget.refusal_spent);assert_eq!(budget.remaining,0);
            let bytes=crate::bounded_output::active_remaining();
            let retry=ordinary_successors(&function,&mut budget,&graph,&closed,1);
            assert!(retry.is_err());assert_eq!(budget.remaining,0);
            assert_eq!(crate::bounded_output::active_remaining(),bytes);
            // Keep the originally selected six-field value live across retry.
            same_successors(&actual,&expected);drop(retry);drop(actual);
        });
    }
}

// The complete original Directory and generic successors above stay executable.
// Only the ordinary paid fallback selects the lexical finalizer.
mod lexical_finish;

#[test]
fn direct_selected_anchor_rejects_same_id_foreign_pointer_and_foreign_scope_before_work() {
    use crate::bounded_output::with_limit_usage_refusal;
    let (function, graph, closed) = direct_fixture(1, 1, 3);
    let same_id_foreign = function.clone();
    assert_eq!(same_id_foreign.id, function.id);
    for foreign_scope in [false, true] {
        let (_, parent_refused, _, _) = with_limit_usage_refusal(usize::MAX, || {
            let mut budget = ReplayBudget::with_physical(&function, 1 << 25).unwrap();
            // Both calls are funded in the original scope before entering any
            // foreign scope or setting sticky refusal; retry queries no payer.
            budget.reserve_bytes(&function, 2 * ordinary_receiving(), "selected direct two-call receiving controls").unwrap();
            let remaining = budget.remaining;
            let charged = budget.physical.as_ref().unwrap().charged();
            let error = if foreign_scope {
                let (result, child_refused, _, _) = with_limit_usage_refusal(1 << 25, || {
                    ordinary_successors(&function, &mut budget, &graph, &closed, 5)
                });
                assert!(!child_refused, "foreign enclosing payer was not queried");
                result.unwrap_err()
            } else {
                ordinary_successors(&same_id_foreign, &mut budget, &graph, &closed, 5).unwrap_err()
            };
            assert_eq!(error, Diagnostic::io("SPX-H006", String::new()));
            assert_eq!(budget.remaining, remaining);
            assert_eq!(budget.physical.as_ref().unwrap().charged(), charged);
            assert!(budget.refusal_spent);
            let restored = ordinary_successors(&function, &mut budget, &graph, &closed, 5).unwrap_err();
            assert_eq!(restored, error);
            assert_eq!(budget.remaining, remaining);
            assert_eq!(budget.physical.as_ref().unwrap().charged(), charged);
        });
        assert_eq!(parent_refused, foreign_scope, "only a held-scope mismatch poisons its scope");
    }
}

#[test]
fn complete_before_direct_producer_records_foreign_first_acceptance_as_negative() {
    let (function, graph, closed) = direct_fixture(1, 1, 3);
    let mut foreign = function.clone();
    foreign.id = crate::hir::DeclarationId::new("direct.foreign");
    let mut budget = ReplayBudget::with_physical(&function, 1 << 25).unwrap();
    let result = ordinary_successors_before_selected_anchor(&foreign, &mut budget, &graph, &closed, 5)
        .unwrap().unwrap();
    assert_eq!(result.0, 5);
    assert_eq!(result.1.len(), 1);
    assert_eq!(result.1[0].1.as_ref().as_slice(), &[(0, 3)]);
}
