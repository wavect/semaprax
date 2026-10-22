//! Lazy numeric closure memo. Sparse high keys still require gap directory slots.
use super::{Diagnostic, ReplayBudget, ResolvedFunction};
use crate::bounded_output::classifier_string::ClassifierLedger;
use std::mem::size_of;
const WIDTH: usize = 8;

pub(super) struct Memo<T> {
    chunks: Vec<Option<Box<[Option<T>; WIDTH]>>>,
    binding: Option<Binding>,
    failed: bool,
}
struct Binding {
    function: *const ResolvedFunction,
    budget: *const ReplayBudget,
    ledger: ClassifierLedger,
}
impl<T> Memo<T> {
    pub(super) fn new() -> Self {
        Self {
            chunks: Vec::new(),
            binding: None,
            failed: false,
        }
    }
    pub(super) fn authenticate(
        &mut self,
        function: &ResolvedFunction,
        budget: &mut ReplayBudget,
    ) -> Result<(), Diagnostic> {
        if self.failed {
            return Err(Diagnostic::io("SPX-H006", String::new()));
        }
        if let Some(binding) = &self.binding {
            // Read no foreign payer, even to construct a refusal report. The
            // exact caller keeps both borrowed receivers at stable addresses.
            let ledger_matches = binding.ledger.current();
            if binding.function != std::ptr::from_ref(function)
                || binding.budget != std::ptr::from_ref(budget)
                || !ledger_matches
            {
                self.failed = true;
                return Err(Diagnostic::io("SPX-H006", String::new()));
            }
        }
        if let Err(error) =
            budget.reserve_bytes(function, 0, "factored closure memo authentication")
        {
            self.failed = true;
            return Err(error);
        }
        if budget.pays_allocations() {
            if let Err(error) = budget.reserve_bytes(
                function,
                size_of::<Result<(), Diagnostic>>(),
                "factored closure memo authorization receipt",
            ) {
                self.failed = true;
                return Err(error);
            }
        }
        if self.binding.is_none() {
            // One live Memo receiver (including its binding slot) and the
            // separately named Option<Binding> initializer below.
            // The old Canonicalizer constructor created only allocation-free
            // inert fields; activation pays its new inline receiver once.
            if budget.pays_allocations() {
                if let Err(error) = budget.reserve_bytes(
                    function,
                    size_of::<Self>() + size_of::<Option<Binding>>(),
                    "factored closure memo controls",
                ) {
                    self.failed = true;
                    return Err(error);
                }
            }
            let binding = Some(Binding {
                function: std::ptr::from_ref(function),
                budget: std::ptr::from_ref(budget),
                ledger: ClassifierLedger::capture(),
            });
            self.binding = binding;
        }
        Ok(())
    }
    // Caller already holds the selected error; retain it unchanged while
    // invalidating views without querying any payer or allocating a report.
    pub(super) fn reject_after_error(&mut self) {
        self.failed = true;
    }
    pub(super) fn contains_key(&self, key: &usize) -> bool {
        self.get(key).is_some()
    }
    pub(super) fn get(&self, key: &usize) -> Option<&T> {
        if self.failed {
            return None;
        }
        self.chunks
            .get(*key / WIDTH)?
            .as_ref()?
            .get(*key % WIDTH)?
            .as_ref()
    }
    pub(super) fn insert(
        &mut self,
        function: &ResolvedFunction,
        key: usize,
        budget: &mut ReplayBudget,
        make: impl FnOnce() -> T,
    ) -> Result<(), Diagnostic> {
        self.authenticate(function, budget)?;
        let result = self.insert_bound(function, key, budget, make);
        if result.is_err() {
            self.failed = true;
        }
        result
    }
    fn insert_bound(
        &mut self,
        function: &ResolvedFunction,
        key: usize,
        budget: &mut ReplayBudget,
        make: impl FnOnce() -> T,
    ) -> Result<(), Diagnostic> {
        let index = key / WIDTH;
        // WIDTH > 1, so quotient + 1 cannot overflow usize.
        let required = index + 1;
        if self.chunks.len() < required {
            let additional = required - self.chunks.len();
            budget.reserve_vec(
                function,
                &mut self.chunks,
                additional,
                "factored closure memo directory",
            )?;
            self.chunks.resize_with(required, || None);
        }
        if self.chunks[index].is_none() {
            if budget.pays_allocations() {
                // Named array initializer + its simultaneously allocated boxed
                // array backing + named Box pointer. The directory's receiving
                // Option<Box> slot was already paid by reserve_vec, not again.
                budget.reserve_bytes(
                    function,
                    2 * size_of::<[Option<T>; WIDTH]>() + size_of::<Box<[Option<T>; WIDTH]>>(),
                    "factored closure memo chunk",
                )?;
            }
            let values: [Option<T>; WIDTH] = std::array::from_fn(|_| None);
            let chunk = Box::new(values);
            self.chunks[index] = Some(chunk);
        }
        self.chunks[index].as_mut().expect("paid chunk")[key % WIDTH] = Some(make());
        Ok(())
    }
}

#[cfg(test)]
#[rustfmt::skip]
mod before_index {
//! Paid immutable weighted backing for the ordinary canonical receiver.
//! Frames, open states and completed memo keys share only owned immutable
//! vectors. Label order, route weights, signature algebra and work debits keep
//! the complete original receiver in canonical_before.rs as an independent control.
use super::super::*;
use super::super::tests;

type SharedWeighted = Rc<Weighted>;

enum CanonicalFrame {
    Enter(SharedWeighted),
    Exit(SharedWeighted, u64, Vec<(Label, SharedWeighted)>),
}

// The caller has already paid the moved vector's complete allocated backing.
// Rc owns that vector without copying it; its two counters and Vec header are
// separately acquired while all preceding owners remain fully charged.
fn own_weighted(
    function: &ResolvedFunction,
    budget: &mut ReplayBudget,
    state: Weighted,
) -> Result<SharedWeighted, Diagnostic> {
    budget.reserve_bytes(
        function,
        physical_clone::rc_backing_bytes::<Weighted>(),
        "factored weighted shared header",
    )?;
    Ok(Rc::new(state))
}

// Vec's FromIterator growth policy may round 1..3 items up to four slots.
// Build this producer through its exact precharged capacity instead.
fn weighted_successor(
    function: &ResolvedFunction,
    budget: &mut ReplayBudget,
    targets: BTreeMap<usize, u64>,
) -> Result<SharedWeighted, Diagnostic> {
    budget.reserve_array::<(usize, u64)>(
        function,
        targets.len(),
        "factored weighted successor vector",
    )?;
    let mut state = Vec::with_capacity(targets.len());
    state.extend(targets);
    own_weighted(function, budget, state)
}

fn share_weighted(
    function: &ResolvedFunction,
    budget: &mut ReplayBudget,
    state: &SharedWeighted,
) -> Result<SharedWeighted, Diagnostic> {
    let work = state
        .len()
        .checked_add(1)
        .ok_or_else(|| replay_error(function, "factored weighted state work overflowed"))?;
    budget.charge(function, work, "factored weighted state copy")?;
    // Authentication remains sticky even though this clone creates no backing.
    // The receiving frame/map slot has its own full carrier reservation.
    budget.reserve_bytes(function, 0, "factored weighted state backing")?;
    Ok(Rc::clone(state))
}

/// Bottom-up hash-consing of the weighted subset construction. A determinized
/// state's id is its accepted weight together with each label and the id of
/// the state that label leads to, so two states share an id exactly when
/// they accept every observation sequence the same number of times.
pub(super) struct Canonicalizer<'a> {
    automaton: &'a Automaton,
    sink: usize,
    closures: BTreeMap<usize, Rc<BTreeMap<usize, u64>>>,
    states: BTreeMap<SharedWeighted, usize>,
    ids: BTreeMap<(u64, Vec<(Label, usize)>), usize>,
}

impl<'a> Canonicalizer<'a> {
    pub(super) fn new(automaton: &'a Automaton, sink: usize) -> Self {
        Self {
            automaton,
            sink,
            closures: BTreeMap::new(),
            states: BTreeMap::new(),
            ids: BTreeMap::new(),
        }
    }

    /// Nodes with moves (or the sink) reachable through epsilon edges, each
    /// with its number of epsilon routes.
    fn closure(
        &mut self,
        function: &ResolvedFunction,
        root: usize,
        budget: &mut ReplayBudget,
    ) -> Result<Option<Rc<BTreeMap<usize, u64>>>, Diagnostic> {
        budget.reserve_array::<(usize, bool)>(function, 1, "factored closure initial cursor")?;
        let mut stack = vec![(root, false)];
        while let Some((node, expanded)) = stack.pop() {
            budget.charge(function, 1, "factored closure cursor")?;
            if self.closures.contains_key(&node) {
                continue;
            }
            let state = &self.automaton.nodes[node];
            if !expanded {
                budget.reserve_vec(function, &mut stack, 1, "factored closure cursors")?;
                stack.push((node, true));
                for target in &state.epsilon {
                    if !self.closures.contains_key(target) {
                        budget.reserve_vec(function, &mut stack, 1, "factored closure cursors")?;
                        stack.push((*target, false));
                    }
                }
                continue;
            }
            let mut closure = BTreeMap::new();
            if !state.moves.is_empty() || node == self.sink {
                budget.reserve_btree_node::<usize, u64>(function, "factored closure node")?;
                closure.insert(node, 1_u64);
            }
            for target in &state.epsilon {
                let Some(inner) = self.closures.get(target) else {
                    return Err(replay_error(function, "cleanup CFG contains a cycle"));
                };
                for (inner_node, routes) in inner.iter() {
                    budget.charge(function, 1, "factored closure route addition")?;
                    if !closure.contains_key(inner_node) {
                        budget
                            .reserve_btree_node::<usize, u64>(function, "factored closure node")?;
                    }
                    let slot = closure.entry(*inner_node).or_insert(0_u64);
                    let Some(sum) = slot.checked_add(*routes) else {
                        return Ok(None);
                    };
                    *slot = sum;
                }
            }
            budget.charge(
                function,
                closure.len().saturating_add(1),
                "factored skeleton closure",
            )?;
            budget.reserve_bytes(
                function,
                physical_clone::rc_backing_bytes::<BTreeMap<usize, u64>>(),
                "factored closure shared header",
            )?;
            budget.reserve_btree_node::<usize, Rc<BTreeMap<usize, u64>>>(
                function,
                "factored closure memo node",
            )?;
            self.closures.insert(node, Rc::new(closure));
        }
        Ok(self.closures.get(&root).cloned())
    }

    /// The accepted weight of `state` and its weighted successor per label.
    #[allow(clippy::type_complexity)]
    fn successors(
        &mut self,
        function: &ResolvedFunction,
        state: &SharedWeighted,
        budget: &mut ReplayBudget,
    ) -> Result<Option<(u64, Vec<(Label, SharedWeighted)>)>, Diagnostic> {
        let mut closed = BTreeMap::<usize, u64>::new();
        for (node, weight) in state.iter() {
            let Some(closure) = self.closure(function, *node, budget)? else {
                return Ok(None);
            };
            for (inner, routes) in closure.iter() {
                let Some(sum) = routes.checked_mul(*weight).and_then(|product| {
                    closed.get(inner).copied().unwrap_or(0).checked_add(product)
                }) else {
                    return Ok(None);
                };
                budget.charge(function, 1, "factored weighted route addition")?;
                if !closed.contains_key(inner) {
                    budget.reserve_btree_node::<usize, u64>(
                        function,
                        "factored weighted closure node",
                    )?;
                }
                closed.insert(*inner, sum);
            }
        }
        let accepted = closed.get(&self.sink).copied().unwrap_or(0);
        let mut successors = BTreeMap::<Label, BTreeMap<usize, u64>>::new();
        for (node, weight) in &closed {
            let moves = &self.automaton.nodes[*node].moves;
            budget.charge(
                function,
                moves.len().saturating_add(1),
                "factored skeleton subset construction",
            )?;
            for (label, child) in moves {
                if !successors.contains_key(label) {
                    budget.reserve_btree_node::<Label, BTreeMap<usize, u64>>(
                        function,
                        "factored successor label node",
                    )?;
                }
                let targets = successors.entry(label.clone()).or_default();
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
        budget.reserve_array::<(Label, SharedWeighted)>(
            function,
            successors.len(),
            "factored successor vector",
        )?;
        let mut result = Vec::with_capacity(successors.len());
        for (label, targets) in successors {
            result.push((label, weighted_successor(function, budget, targets)?));
        }
        Ok(Some((accepted, result)))
    }

    pub(super) fn id(
        &mut self,
        function: &ResolvedFunction,
        root: Weighted,
        budget: &mut ReplayBudget,
    ) -> Result<Option<usize>, Diagnostic> {
        #[cfg(test)]
        tests::note_actual_entry(budget);
        budget.reserve_array::<CanonicalFrame>(function, 1, "factored canonical initial frame")?;
        let root = own_weighted(function, budget, root)?;
        let root_copy = share_weighted(function, budget, &root)?;
        let mut frames = vec![CanonicalFrame::Enter(root_copy)];
        let mut open = BTreeSet::new();
        while let Some(frame) = frames.pop() {
            budget.charge(function, 1, "factored canonical frame")?;
            match frame {
                CanonicalFrame::Enter(state) => {
                    if self.states.contains_key(&state) {
                        continue;
                    }
                    if open.contains(&state) {
                        return Err(replay_error(function, "cleanup CFG contains a cycle"));
                    }
                    budget.reserve_btree_node::<SharedWeighted, ()>(
                        function,
                        "factored open-state identity node",
                    )?;
                    open.insert(share_weighted(function, budget, &state)?);
                    let Some((accepted, successors)) = self.successors(function, &state, budget)?
                    else {
                        return Ok(None);
                    };
                    let child_count = successors.len();
                    budget.reserve_vec(
                        function,
                        &mut frames,
                        1,
                        "factored canonical exit frame",
                    )?;
                    let exit_index = frames.len();
                    frames.push(CanonicalFrame::Exit(state, accepted, successors));
                    for index in 0..child_count {
                        let CanonicalFrame::Exit(_, _, successors) = &frames[exit_index] else {
                            unreachable!("canonical exit retained below child frames");
                        };
                        let child = share_weighted(function, budget, &successors[index].1)?;
                        budget.reserve_vec(
                            function,
                            &mut frames,
                            1,
                            "factored canonical child frames",
                        )?;
                        frames.push(CanonicalFrame::Enter(child));
                    }
                }
                CanonicalFrame::Exit(state, accepted, successors) => {
                    open.remove(&state);
                    budget.charge(
                        function,
                        successors.len(),
                        "factored canonical signature construction",
                    )?;
                    budget.reserve_array::<(Label, usize)>(
                        function,
                        successors.len(),
                        "factored canonical signature vector",
                    )?;
                    let mut key = Vec::with_capacity(successors.len());
                    for (label, child) in successors {
                        let Some(id) = self.states.get(&child) else {
                            return Err(replay_error(function, "cleanup CFG contains a cycle"));
                        };
                        key.push((label, *id));
                    }
                    let key = (accepted, key);
                    let id = match self.ids.get(&key) {
                        Some(id) => *id,
                        None => {
                            budget.reserve_btree_node::<(u64, Vec<(Label, usize)>), usize>(
                                function,
                                "factored canonical signature node",
                            )?;
                            let id = self.ids.len();
                            self.ids.insert(key, id);
                            id
                        }
                    };
                    if !self.states.contains_key(&state) {
                        budget.reserve_btree_node::<SharedWeighted, usize>(
                            function,
                            "factored canonical state memo node",
                        )?;
                    }
                    self.states.insert(state, id);
                }
            }
        }
        Ok(self.states.get(&root).copied())
    }
}


}

#[cfg(test)]
#[rustfmt::skip]
mod tests {
use super::{Memo, WIDTH};
use super::super::{Diagnostic, ReplayBudget, ResolvedFunction};
use crate::bounded_output::with_limit_usage_refusal;
use crate::test_allocation_observer::{current_metrics, measure};
fn fixture() -> ResolvedFunction {
    super::super::super::super::lexical_plan_tests::fixture(super::super::super::super::lexical_plan_tests::WHILE)
        .1
}
#[test]
fn numeric_memo_sparse_overwrite_and_full_old_map_values_keep_exact_work() {
    let function = fixture();
    let (((), allocation), refused, paid, _) = with_limit_usage_refusal(usize::MAX, || {
        measure(|| {
            let mut budget = ReplayBudget::new();
            let before = budget.remaining;
            let mut memo = Memo::<usize>::new();
            let mut expected = std::collections::BTreeMap::new();
            for key in [127, 0, 7, 8, 63, 1, 7] {
                let required = key / WIDTH + 1;
                let expected_capacity = if required > memo.chunks.capacity() {
                    crate::cleanup_plan::replay::physical_storage::replay_growth_capacity(
                        memo.chunks.capacity(),
                        required,
                    )
                    .unwrap()
                } else {
                    memo.chunks.capacity()
                };
                memo.insert(&function, key, &mut budget, || key + 3)
                    .unwrap();
                assert_eq!(memo.chunks.capacity(), expected_capacity);
                expected.insert(key, key + 3);
            }
            for key in 0..=128 {
                assert_eq!(memo.get(&key).copied(), expected.get(&key).copied());
            }
            assert_eq!(budget.remaining, before);
            drop(expected);
            drop(memo);
            drop(budget);
        })
    });
    assert!(!refused && !allocation.invalid && allocation.live == 0);
    assert_eq!(allocation.allocated, allocation.freed);
    // The independent old map is observed but has no new-memo funding claim.
    assert!(paid > 0);
}
#[test]
fn numeric_memo_actual_exact_short_zero_and_partial_drop_remain_paid() {
    let function = fixture();
    let observe = |limit, count| {
        with_limit_usage_refusal(limit, || {
            measure(|| {
                let mut budget = ReplayBudget::new();
                let mut memo = Memo::<usize>::new();
                let mut success = match memo.authenticate(&function, &mut budget) {
                    Ok(()) => true,
                    Err(error) => {
                        drop(error);
                        false
                    }
                };
                if success {
                    for key in [0, 7, 8, 63, 127].into_iter().take(count) {
                        if let Err(error) = memo.insert(&function, key, &mut budget, || key) {
                            assert_eq!(error.code, "SPX-H006");
                            drop(error);
                            success = false;
                            break;
                        }
                    }
                }
                if !success {
                    let at = current_metrics().allocated;
                    assert!(
                        memo.insert(&function, 0, &mut budget, || panic!("refused publication"))
                            .is_err()
                    );
                    assert_eq!(current_metrics().allocated, at);
                }
                drop(memo);
                drop(budget);
                success
            })
        })
    };
    // Include control-only activation and each actual directory/chunk frontier.
    // Sparse high-key-first is independently exercised by the full map owner.
    for count in 0..=5 {
        let ((ok, allocation), refused, exact, first) = observe(usize::MAX, count);
        assert!(ok && !refused && first.is_none());
        assert!(
            !allocation.invalid && allocation.live == 0 && allocation.allocated == allocation.freed
        );
        assert!(allocation.allocated <= exact);
        for limit in [0, exact - 1, exact] {
            let ((ok, allocation), refused, _, first) = observe(limit, count);
            assert_eq!(ok, limit == exact);
            assert_eq!(refused, limit != exact);
            assert_eq!(first.is_none(), limit == exact);
            assert!(
                !allocation.invalid
                    && allocation.live == 0
                    && allocation.allocated == allocation.freed
            );
            if limit == 0 {
                assert_eq!(allocation.allocated, 0);
            }
        }
    }
}
#[test]
fn numeric_memo_foreign_then_restored_budget_never_reacquires() {
    let function = fixture();
    let (((), allocation), refused, _, _) = with_limit_usage_refusal(usize::MAX, || {
        measure(|| {
            let mut budget = ReplayBudget::new();
            let mut memo = Memo::<usize>::new();
            memo.insert(&function, 0, &mut budget, || 3).unwrap();
            let (_, foreign_refused, foreign_paid, _) =
                with_limit_usage_refusal(usize::MAX, || {
                    let before = current_metrics().allocated;
                    let error = memo.insert(&function, 8, &mut budget, || 9).unwrap_err();
                    drop(error);
                    assert_eq!(current_metrics().allocated, before);
                });
            assert!(!foreign_refused && foreign_paid == 0);
            let before = current_metrics().allocated;
            let error = memo.insert(&function, 8, &mut budget, || 9).unwrap_err();
            drop(error);
            assert_eq!(current_metrics().allocated, before);
            assert!(memo.failed);
            assert_eq!(memo.chunks[0].as_ref().unwrap()[0], Some(3));
            assert_eq!(memo.get(&0), None);
            assert_eq!(memo.get(&8), None);
            drop(memo);
            drop(budget);
        })
    });
    assert!(refused);
    assert!(
        !allocation.invalid && allocation.live == 0 && allocation.allocated == allocation.freed
    );
}

#[test]
fn chunked_complete_canonical_receiver_preserves_old_work_ids_and_last_drop() {
    let function = fixture();
    let (automaton, sink, roots) = super::super::tests::automaton(&function);
    let (((), allocation), refused, paid, first) = with_limit_usage_refusal(usize::MAX, || {
        measure(|| {
            let mut old = super::before_index::Canonicalizer::new(&automaton, sink);
            let mut current = super::super::Canonicalizer::new(&automaton, sink);
            let mut old_budget = ReplayBudget::new();
            let mut budget = ReplayBudget::new();
            for root in &roots[..6] {
                old_budget
                    .reserve_array::<(usize, u64)>(&function, 1, "old canonical test root")
                    .unwrap();
                budget
                    .reserve_array::<(usize, u64)>(&function, 1, "current canonical test root")
                    .unwrap();
                let a = old.id(&function, vec![(*root, 1)], &mut old_budget);
                let b = current.id(&function, vec![(*root, 1)], &mut budget);
                assert_eq!(a, b);
                assert_eq!(old_budget.remaining, budget.remaining);
                assert_eq!(old_budget.skeleton_remaining, budget.skeleton_remaining);
            }
            drop(old);
            drop(current);
            drop(old_budget);
            drop(budget);
        })
    });
    assert!(!refused && first.is_none());
    assert!(
        !allocation.invalid && allocation.live == 0 && allocation.allocated == allocation.freed
    );
    assert!(allocation.allocated <= paid);
}

#[test]
fn numeric_memo_foreign_function_budget_and_stale_ledger_fail_before_cached_reuse() {
    let function = fixture();
    let wrong = function.clone();
    let empty = Diagnostic::io("SPX-H006", String::new());
    for foreign_function in [false, true] {
        let (((), allocation), refused, paid, _) = with_limit_usage_refusal(usize::MAX, || {
            measure(|| {
                let mut budget = ReplayBudget::new();
                let mut foreign = ReplayBudget::new();
                let mut memo = Memo::<usize>::new();
                memo.insert(&function, 0, &mut budget, || 3).unwrap();
                let foreign_remaining = foreign.remaining;
                let at = current_metrics().allocated;
                let error = if foreign_function {
                    memo.authenticate(&wrong, &mut budget)
                } else {
                    memo.authenticate(&function, &mut foreign)
                }
                .unwrap_err();
                assert_eq!(error, empty);
                drop(error);
                assert!(memo.failed);
                assert!(memo.get(&0).is_none() && !memo.contains_key(&0));
                assert_eq!(memo.chunks[0].as_ref().unwrap()[0], Some(3));
                assert_eq!(foreign.remaining, foreign_remaining);
                assert!(foreign.refusal_text.is_none() && !foreign.refusal_spent);
                assert_eq!(
                    memo.insert(&function, 1, &mut budget, || panic!(
                        "no publication after refusal"
                    )),
                    Err(Diagnostic::io("SPX-H006", String::new()))
                );
                assert_eq!(current_metrics().allocated, at);
                drop(memo);
                drop(foreign);
                drop(budget);
            })
        });
        assert!(!refused);
        assert!(
            !allocation.invalid && allocation.live == 0 && allocation.allocated == allocation.freed
        );
        assert!(allocation.allocated <= paid && allocation.highwater <= paid);
    }
    // The complete ledger setup, retained scope and last Drop are observed.
    // Its test wrapper is not claimed as compiler-funded payload.
    let ((), allocation) = measure(|| {
        let ((mut memo, mut budget), refused, _, _) = with_limit_usage_refusal(usize::MAX, || {
            let mut budget = ReplayBudget::new();
            let mut memo = Memo::<usize>::new();
            memo.insert(&function, 0, &mut budget, || 3).unwrap();
            (memo, budget)
        });
        assert!(!refused);
        let at = current_metrics().allocated;
        assert_eq!(
            memo.authenticate(&function, &mut budget),
            Err(Diagnostic::io("SPX-H006", String::new()))
        );
        assert!(memo.failed && memo.get(&0).is_none());
        assert_eq!(
            memo.authenticate(&function, &mut budget),
            Err(Diagnostic::io("SPX-H006", String::new()))
        );
        assert_eq!(current_metrics().allocated, at);
        drop(memo);
        drop(budget);
    });
    assert!(
        !allocation.invalid && allocation.live == 0 && allocation.allocated == allocation.freed
    );
}

#[test]
fn numeric_memo_legacy_unobserved_payer_has_no_active_debit_or_refusal_bootstrap() {
    let function = fixture();
    let (((), allocation), refused, paid, first) = with_limit_usage_refusal(0, || {
        measure(|| {
            let mut budget = ReplayBudget::unobserved();
            let mut memo = Memo::<usize>::new();
            let before = budget.remaining;
            for key in [127, 0, 8, 7] {
                memo.insert(&function, key, &mut budget, || key).unwrap();
            }
            assert_eq!(memo.get(&127), Some(&127));
            assert_eq!(budget.remaining, before);
            assert!(budget.refusal_text.is_none() && !budget.refusal_spent);
            drop(memo);
            drop(budget);
        })
    });
    assert!(!refused && paid == 0 && first.is_none());
    assert!(
        !allocation.invalid && allocation.live == 0 && allocation.allocated == allocation.freed
    );
    // This explicit legacy control makes no selected-byte payment claim.
}

#[test]
fn chunked_complete_cached_id_authenticates_before_any_new_backing() {
    let function = fixture();
    let wrong = function.clone();
    let (automaton, sink, roots) = super::super::tests::automaton(&function);
    let (((), allocation), refused, paid, first) = with_limit_usage_refusal(usize::MAX, || {
        measure(|| {
            let mut current = super::super::Canonicalizer::new(&automaton, sink);
            let mut budget = ReplayBudget::new();
            budget
                .reserve_array::<(usize, u64)>(&function, 1, "memo cached initial input")
                .unwrap();
            let id = current
                .id(&function, vec![(roots[0], 1)], &mut budget)
                .unwrap();
            assert!(id.is_some());
            budget
                .reserve_array::<(usize, u64)>(&function, 1, "memo cached hostile input")
                .unwrap();
            let input = vec![(roots[0], 1)];
            let at = current_metrics().allocated;
            let work = budget.remaining;
            assert_eq!(
                current.id(&wrong, input, &mut budget),
                Err(Diagnostic::io("SPX-H006", String::new()))
            );
            assert_eq!(budget.remaining, work);
            assert_eq!(current_metrics().allocated, at);
            assert!(current.closures.get(&roots[0]).is_none());
            drop(current);
            drop(budget);
        })
    });
    assert!(!refused && first.is_none());
    assert!(
        !allocation.invalid && allocation.live == 0 && allocation.allocated == allocation.freed
    );
    assert!(allocation.allocated <= paid && allocation.highwater <= paid);
}

}
