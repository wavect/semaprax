//! Active-builder canonical frame activation. The complete general entry stays
//! compiled and callable; this activation has no legacy successor-call arm.
use super::payments as legacy_payments;
use super::*;
use std::mem::size_of;
#[macro_use]
mod payments {
    //! Complete active one-pass producer, with the flat insertion physically inline.
    macro_rules! active_one_pass_successors {
    ($function:ident, $budget:ident, $automaton:expr, $closed:ident, $accepted:ident, $receiving:ident, $attempt:ident, $compact_attempt:ident, $closed_owner:ident, $closed_type:ty, $unit:expr, $vector:ident, $frames:lifetime) => {{
        'one_pass: {
            // The actual canonical caller has already authenticated its Memo and held
            // receiver. Keep zero-byte sticky authentication even for no movements.
            $compact_attempt!($budget.reserve_compact_bytes(
                $function,
                0,
                "factored one-pass successor authentication"
            ));
            $compact_attempt!($budget.charge_compact(
                $function,
                0,
                "factored one-pass successor authentication"
            ));
            // This activation is selected only for ActiveBuilder. Fresh values
            // are One, and authenticated promotion returns PaidMany. No target
            // map enters from a caller or a memo, so legacy trees cannot occur.
            let mut labels = lexical_successors::OnePassLabels::Empty;
            macro_rules! add {
                ($targets:ident, $child:ident, $weight:ident) => {
                    match $targets {
                        paid_many::Targets::One {
                            child: first,
                            weight: total,
                        } if *first == *$child => {
                            let Some(sum) = total.checked_add(*$weight) else {
                                $receiving.finish_overflow();
                                return;
                            };
                            *total = sum;
                        }
                        paid_many::Targets::One {
                            child: first,
                            weight: total,
                        } => {
                            $compact_attempt!($budget.reserve_compact_bytes(
                                $function,
                                compact_targets::PROMOTE_RECEIVING,
                                "factored one-pass distinct-target receivers"
                            ));
                            #[cfg(test)]
                            if $budget.pays_allocations() {
                                paid_many::note_branch(true);
                            }
                            *$targets = $compact_attempt!(compact_targets::promote(
                                $function, $budget, *first, *total, *$child, *$weight
                            ));
                        }
                        paid_many::Targets::PaidMany(flat) => {
                            $compact_attempt!($budget.reserve_compact_bytes(
                                $function,
                                active_targets::FlatCursor::CONTROLS,
                                "factored one-pass flat-target receivers"
                            ));
                            active_targets::add_flat!(
                                $function,
                                $budget,
                                flat,
                                *$child,
                                *$weight,
                                $compact_attempt,
                                {
                                    $receiving.finish_overflow();
                                    return;
                                }
                            );
                        }
                        paid_many::Targets::Many(_) => {
                            unreachable!(
                                "active-only target construction never produces legacy trees"
                            )
                        }
                    }
                };
            }
            for (node, weight) in $closed.iter() {
                let moves = ($automaton).nodes[*node].moves();
                // This is the complete before_index subset request, at the same point.
                // No label census, search-charge prepass or four-op selector is run.
                $compact_attempt!($budget.charge_compact(
                    $function,
                    moves.len().saturating_add(1),
                    "factored skeleton subset construction"
                ));
                for (label, child) in moves {
                    $compact_attempt!($budget.reserve_compact_bytes(
                        $function,
                        compact_successors::ONE_PASS_MOVEMENT,
                        "factored one-pass movement receivers"
                    ));
                    match &mut labels {
                        lexical_successors::OnePassLabels::Empty => {
                            $compact_attempt!($budget.reserve_compact_bytes(
                                $function,
                                0,
                                "factored singleton target authentication"
                            ));
                            let weight = 0_u64
                                .checked_add(*weight)
                                .expect("zero plus u64 cannot overflow");
                            labels = lexical_successors::OnePassLabels::One(
                                label,
                                paid_many::Targets::One {
                                    child: *child,
                                    weight,
                                },
                            );
                        }
                        lexical_successors::OnePassLabels::One(first, targets)
                            if *first == label =>
                        {
                            add!(targets, child, weight);
                        }
                        lexical_successors::OnePassLabels::One(_, _) => {
                            // All receiving and both new label nodes precede moving
                            // the existing inline entry. A refusal retains that entry.
                            $compact_attempt!($budget.reserve_compact_bytes(
                                $function,
                                compact_successors::ONE_PASS_PROMOTION,
                                "factored one-pass label promotion receivers"
                            ));
                            $compact_attempt!($budget
                                .reserve_compact_btree_node::<&Label, paid_many::Targets>(
                                    $function,
                                    "factored one-pass first label node"
                                ));
                            $compact_attempt!($budget.reserve_compact_bytes(
                                $function,
                                0,
                                "factored one-pass second label node"
                            ));
                            $compact_attempt!($budget.reserve_compact_bytes(
                                $function,
                                0,
                                "factored singleton target authentication"
                            ));
                            let weight = 0_u64
                                .checked_add(*weight)
                                .expect("zero plus u64 cannot overflow");
                            #[cfg(test)]
                            active_targets::AVOIDED_LABEL_NODES
                                .with(|count| count.set(count.get() + 1));
                            let mut many = BTreeMap::new();
                            let lexical_successors::OnePassLabels::One(first, targets) =
                                std::mem::replace(
                                    &mut labels,
                                    lexical_successors::OnePassLabels::Empty,
                                )
                            else {
                                unreachable!("occupied inline label")
                            };
                            many.insert(first, targets);
                            many.insert(
                                label,
                                paid_many::Targets::One {
                                    child: *child,
                                    weight,
                                },
                            );
                            labels = lexical_successors::OnePassLabels::Many(many);
                        }
                        lexical_successors::OnePassLabels::Many(many) => {
                            if !many.contains_key(label) {
                                $compact_attempt!(active_targets::append_label_node(
                                    $function,
                                    $budget,
                                    many.len(),
                                    "factored one-pass label node"
                                ));
                                $compact_attempt!($budget.reserve_compact_bytes(
                                    $function,
                                    0,
                                    "factored singleton target authentication"
                                ));
                                let weight = 0_u64
                                    .checked_add(*weight)
                                    .expect("zero plus u64 cannot overflow");
                                many.insert(
                                    label,
                                    paid_many::Targets::One {
                                        child: *child,
                                        weight,
                                    },
                                );
                            } else {
                                let targets = many.get_mut(label).expect("present immutable label");
                                add!(targets, child, weight);
                            }
                        }
                    }
                }
            }
            // Publication owns its actual iterator/enum transport. This new fee is not
            // an old map/Vec-header credit and may regress many-label charged totals.
            $compact_attempt!($budget.reserve_compact_bytes(
                $function,
                compact_successors::ONE_PASS_PUBLICATION,
                "factored one-pass publication receivers"
            ));
            let count = match &labels {
                lexical_successors::OnePassLabels::Empty => 0,
                lexical_successors::OnePassLabels::One(_, _) => 1,
                lexical_successors::OnePassLabels::Many(many) => many.len(),
            };
            $compact_attempt!($budget.reserve_compact_array::<(Label, SharedWeighted)>(
                $function,
                if count == 1 { 0 } else { count },
                "factored successor vector"
            ));
            // The completed One publishes only owned fields. Its outer result
            // stays inside the old complete Option/Result layout ceiling.
            #[cfg(test)]
            inline_exit::census::arity(count, $function, $budget);
            if let lexical_successors::OnePassLabels::One(..) = &labels {
                $compact_attempt!($budget.reserve_compact_bytes(
                    $function,
                    compact_successors::ONE_PASS_FINAL_TRANSPORT + inline_exit::FIELDS,
                    "factored one-pass consuming publication transport"
                ));
                let lexical_successors::OnePassLabels::One(label, targets) = labels else {
                    unreachable!("selected singleton")
                };
                $compact_attempt!($budget.reserve_compact_bytes(
                    $function,
                    compact_successors::ONE_PASS_PUBLISHED_ROW,
                    "factored one-pass published row receivers"
                ));
                let state = match targets {
                    paid_many::Targets::One { child, weight } => {
                        $compact_attempt!($budget.reserve_compact_bytes(
                            $function,
                            2 * size_of::<SmallWeighted>() + size_of::<[(usize, u64); 1]>(),
                            "factored inline weighted enum and array receiving"
                        ));
                        $compact_attempt!($budget.reserve_compact_bytes(
                            $function,
                            physical_clone::rc_backing_bytes::<SmallWeighted>(),
                            "factored weighted shared header"
                        ));
                        Rc::new(SmallWeighted::One([(child, weight)]))
                    }
                    paid_many::Targets::PaidMany(flat) => {
                        $compact_attempt!($budget.reserve_compact_bytes(
                            $function,
                            active_targets::FINISH_RECEIVING,
                            "factored one-pass flat finish receivers"
                        ));
                        $compact_attempt!(active_targets::finish_weighted($function, $budget, flat))
                    }
                    paid_many::Targets::Many(_) => {
                        unreachable!("active-only target construction never produces legacy trees")
                    }
                };
                break 'one_pass Some(($accepted, label, state));
            }
            let mut result = Vec::with_capacity(count);
            macro_rules! publish {
                ($label:ident, $targets:ident) => {{
                    $compact_attempt!($budget.reserve_compact_bytes(
                        $function,
                        compact_successors::ONE_PASS_PUBLISHED_ROW,
                        "factored one-pass published row receivers"
                    ));
                    let state = match $targets {
                        paid_many::Targets::One { child, weight } => {
                            $compact_attempt!($budget.reserve_compact_bytes(
                                $function,
                                2 * size_of::<SmallWeighted>() + size_of::<[(usize, u64); 1]>(),
                                "factored inline weighted enum and array receiving"
                            ));
                            $compact_attempt!($budget.reserve_compact_bytes(
                                $function,
                                physical_clone::rc_backing_bytes::<SmallWeighted>(),
                                "factored weighted shared header"
                            ));
                            Rc::new(SmallWeighted::One([(child, weight)]))
                        }
                        paid_many::Targets::PaidMany(flat) => {
                            $compact_attempt!($budget.reserve_compact_bytes(
                                $function,
                                active_targets::FINISH_RECEIVING,
                                "factored one-pass flat finish receivers"
                            ));
                            $compact_attempt!(active_targets::finish_weighted(
                                $function, $budget, flat
                            ))
                        }
                        paid_many::Targets::Many(_) => {
                            unreachable!(
                                "active-only target construction never produces legacy trees"
                            )
                        }
                    };
                    result.push(($label.clone(), state));
                }};
            }
            // This is a distinct consuming publication, not either initial Empty
            // construction. Pay enum transport and both possible owned pattern bindings
            // before the match can move One's tuple/Targets or Many's map out of labels.
            $compact_attempt!($budget.reserve_compact_bytes(
                $function,
                compact_successors::ONE_PASS_FINAL_TRANSPORT + 2 * size_of::<$closed_type>(),
                "factored one-pass consuming publication transport"
            ));
            match labels {
                lexical_successors::OnePassLabels::Empty => {}
                lexical_successors::OnePassLabels::One(..) => {
                    unreachable!("singleton returned without a vector")
                }
                lexical_successors::OnePassLabels::Many(many) => {
                    $compact_attempt!($budget.reserve_compact_bytes(
                        $function,
                        compact_successors::ONE_PASS_OWNING_ITERATOR,
                        "factored one-pass owning label iterator receivers"
                    ));
                    for (label, targets) in many {
                        publish!(label, targets);
                    }
                }
            }
            // The explicit drop has its complete new actual/formal owner packet.
            // It preserves the old return-scope Drop before caller frame growth.
            #[cfg(test)]
            inline_exit::census::vector_drop($unit, $function, $budget);
            drop($closed_owner);
            $vector!($accepted, result);
        }
    }};
}

    macro_rules! active_successors {
        ($this:ident, $function:ident, $state:ident, $budget:ident, $receiving:ident, $attempt:ident, $compact_attempt:ident, $vector:ident, $frames:lifetime) => {{
            'lexical_successor: {
                if let [(node, 1)] = $state.as_slice() {
                    $this.closure_query($function, *node, $budget, $receiving);
                    $receiving.ready = false;
                    if $receiving.refusal.is_some() {
                        $receiving.ready = true;
                        return;
                    }
                    if $receiving.overflow {
                        $receiving.finish_overflow();
                        return;
                    }
                    if $receiving.empty {
                        // No closed map, Rc, label-group map, or iterator is constructed.
                        // The original surviving empty vector and outer result remain.
                        $compact_attempt!($budget
                            .reserve_compact_array::<(Label, SharedWeighted)>(
                                $function,
                                0,
                                "factored successor vector",
                            ));
                        #[cfg(test)]
                        inline_exit::census::arity(0, $function, $budget);
                        let accepted = 0;
                        let result = Vec::with_capacity(0);
                        $vector!(accepted, result);
                    }
                    if $budget.pays_allocations() {
                        $compact_attempt!($budget.reserve_compact_bytes(
                            $function,
                            2 * std::mem::size_of::<Rc<ordered_weights::OrderedWeights>>()
                                + 2 * std::mem::size_of::<&ordered_weights::OrderedWeights>(),
                            "factored borrowed closed receiving controls",
                        ));
                    }
                    let closure =
                        Rc::clone($this.closures.get(node).expect("computed shared closure"));
                    for (_, routes) in closure.iter() {
                        let Some(_sum) = routes
                            .checked_mul(1)
                            .and_then(|product| 0_u64.checked_add(product))
                        else {
                            $receiving.finish_overflow();
                            return;
                        };
                        $attempt!($budget.charge($function, 1, "factored weighted route addition"));
                    }
                    let borrowed = closure;
                    let closed = &*borrowed;
                    let accepted = closed.get(&$this.sink).copied().unwrap_or(0);
                    #[cfg(test)]
                    lexical_successors::census::request_inline(
                        true,
                        $this.successor_unit_paid,
                        $function,
                        $budget,
                        if $this.successor_unit_paid {
                            compact_successors::WARM_RECEIVING
                        } else {
                            compact_successors::COLD_RECEIVING
                        },
                    );
                    $compact_attempt!($budget.reserve_compact_bytes(
                        $function,
                        if $this.successor_unit_paid {
                            compact_successors::WARM_RECEIVING
                        } else {
                            compact_successors::COLD_RECEIVING
                        },
                        "factored one-pass label call receivers"
                    ));
                    $this.successor_unit_paid = true;
                    #[cfg(test)]
                    lexical_successors::census::acquired();
                    break 'lexical_successor active_one_pass_successors!(
                        $function,
                        $budget,
                        $this.automaton,
                        closed,
                        accepted,
                        $receiving,
                        $attempt,
                        $compact_attempt,
                        borrowed,
                        Rc<ordered_weights::OrderedWeights>,
                        true,
                        $vector,
                        $frames
                    );
                }
                if $budget.pays_allocations() {
                    $compact_attempt!($budget.reserve_compact_bytes(
                        $function,
                        ordered_weights::OrderedWeights::CONTROLS
                            + 2 * std::mem::size_of::<&ordered_weights::OrderedWeights>(),
                        "factored ordered accumulator controls",
                    ));
                }
                let mut owned = ordered_weights::OrderedWeights::new();
                for (node, weight) in $state.iter() {
                    $this.closure_query($function, *node, $budget, $receiving);
                    $receiving.ready = false;
                    if $receiving.refusal.is_some() {
                        $receiving.ready = true;
                        return;
                    }
                    if $receiving.overflow {
                        $receiving.finish_overflow();
                        return;
                    }
                    let Some(closure) = $this.closures.get(node) else {
                        continue;
                    };
                    for (inner, routes) in closure.iter() {
                        let Some(sum) = routes.checked_mul(*weight).and_then(|product| {
                            owned.get(inner).copied().unwrap_or(0).checked_add(product)
                        }) else {
                            $receiving.finish_overflow();
                            return;
                        };
                        $attempt!($budget.charge($function, 1, "factored weighted route addition"));
                        if owned.get(inner).is_none() {
                            if owned.needs_capacity() {
                                $attempt!(owned.grow($function, $budget));
                            }
                            owned.insert_prepaid(*inner, 0_u64);
                        }
                        *owned.get_mut(inner).expect("inserted ordered weight") = sum;
                    }
                }
                let closed = &owned;
                let accepted = closed.get(&$this.sink).copied().unwrap_or(0);
                #[cfg(test)]
                lexical_successors::census::request_inline(
                    false,
                    $this.successor_general_paid,
                    $function,
                    $budget,
                    if $this.successor_general_paid {
                        compact_successors::WARM_RECEIVING
                    } else {
                        compact_successors::COLD_RECEIVING
                    },
                );
                $compact_attempt!($budget.reserve_compact_bytes(
                    $function,
                    if $this.successor_general_paid {
                        compact_successors::WARM_RECEIVING
                    } else {
                        compact_successors::COLD_RECEIVING
                    },
                    "factored one-pass label call receivers"
                ));
                $this.successor_general_paid = true;
                #[cfg(test)]
                lexical_successors::census::acquired();
                break 'lexical_successor active_one_pass_successors!(
                    $function,
                    $budget,
                    $this.automaton,
                    closed,
                    accepted,
                    $receiving,
                    $attempt,
                    $compact_attempt,
                    owned,
                    ordered_weights::OrderedWeights,
                    false,
                    $vector,
                    $frames
                );
            }
        }};
    }
}
mod active_targets {
    //! Actual retained receivers for the active producer's inlined flat insertion.
    use super::*;
    use crate::cleanup_plan::replay::physical_budget::ByteFailure;

    pub(super) struct FlatCursor<'a> {
        pub(super) targets: &'a mut Weighted,
        pub(super) child: usize,
        pub(super) weight: u64,
        pub(super) low: usize,
        pub(super) high: usize,
        pub(super) middle: usize,
        pub(super) key: usize,
        pub(super) comparisons: usize,
        pub(super) found: bool,
        pub(super) sum: Option<u64>,
    }
    impl FlatCursor<'_> {
        // The real cursor initializer/receiver, pattern borrow, integer operands,
        // conditions, checked sum binding, row insertion and index receivers.
        // The four primitive calls below have separate retained actual ABIs.
        pub(super) const CONTROLS: usize = 2 * size_of::<FlatCursor<'static>>()
            + 2 * size_of::<&mut Weighted>()
            + 4 * size_of::<usize>()
            + 2 * size_of::<u32>()
            + 6 * size_of::<bool>()
            + 2 * size_of::<u64>()
            + 2 * size_of::<(usize, u64)>()
            + 2 * size_of::<&(usize, u64)>()
            + 2 * size_of::<&mut u64>();
    }
    type UnitCall<'a> = (&'a mut ReplayBudget, &'a ResolvedFunction, usize, &'a str);
    type GrowthCall<'a> = (
        &'a mut ReplayBudget,
        &'a ResolvedFunction,
        &'a mut Weighted,
        usize,
        &'a str,
    );
    // Authentication, comparison work, insertion work, and fallible vector growth.
    // There are two lexical add! expansions in each active one-pass activation.
    pub(super) const PRIMITIVE_FRAMES: usize = 3
        * (2 * size_of::<UnitCall<'static>>() + 2 * size_of::<Result<(), ByteFailure>>())
        + 2 * size_of::<GrowthCall<'static>>()
        + 2 * size_of::<Result<(), ByteFailure>>();

    // Only the fresh append-only label map in active_one_pass_successors calls
    // this helper. It has no removal, clear, replacement of a key, or retained empty
    // root. The existing pinned B=6 model gives every non-root node >=5 keys and a
    // nonempty root >=1: nodes <= 1 + (entries-1)/5. Insertion allocates nodes but
    // never deallocates them, so the same bound covers cumulative node allocation
    // and every intermediate split. The first pair is financed directly by one
    // node plus zero-byte authentication before insertion. This helper serves only
    // later entries and reads the real map.len() supplied by its private caller.
    type AppendLabelCall<'a> = (&'a mut ReplayBudget, &'a ResolvedFunction, usize, &'a str);
    #[allow(
        clippy::manual_bits,
        reason = "Eight separate usize receiving roles are charged in bytes, not a bit-width conversion."
    )]
    pub(super) const APPEND_LABEL_FRAMES: usize = 2 * size_of::<AppendLabelCall<'static>>()
    + 3 * size_of::<Result<(), ByteFailure>>() // wrapper return/receive + named zero-auth result
    + 2 * size_of::<&BTreeMap<&Label, paid_many::Targets>>() // actual/formal len receiver
    + 8 * size_of::<usize>() // len result, entry copy, modulo/equality operands and result
    + 8 * size_of::<bool>() // both comparisons, method return/receive, negation and short-circuit
    // The old node ABI remains funded for the positive branch. The new zero
    // authentication call has its own complete primitive packet, never credit.
    + 2 * size_of::<UnitCall<'static>>() + 2 * size_of::<Result<(), ByteFailure>>();
    #[cfg(test)]
    thread_local! {pub(super) static AVOIDED_LABEL_NODES:std::cell::Cell<usize>=const{std::cell::Cell::new(0)};}
    pub(super) fn append_label_node(
        function: &ResolvedFunction,
        budget: &mut ReplayBudget,
        existing_entries: usize,
        phase: &str,
    ) -> Result<(), ByteFailure> {
        if existing_entries != 0 && !existing_entries.is_multiple_of(5) {
            let result = budget.reserve_compact_bytes(function, 0, phase);
            #[cfg(test)]
            if result.is_ok() && budget.pays_allocations() {
                AVOIDED_LABEL_NODES.with(|count| count.set(count.get() + 1));
            }
            return result;
        }
        budget.reserve_compact_btree_node::<&Label, paid_many::Targets>(function, phase)
    }

    // Complete owned-vector producer; only primitive and outer error transport
    // narrow. The old own_weighted function remains compiled and callable.
    pub(super) const FINISH_RECEIVING: usize = 2 * size_of::<&ResolvedFunction>()
        + 2 * size_of::<&mut ReplayBudget>()
        + 2 * size_of::<Weighted>()
        + size_of::<Result<SharedWeighted, ByteFailure>>()
        + size_of::<Result<(), ByteFailure>>();
    const FINISH_BODY_CONTROLS: usize = 2 * size_of::<SmallWeighted>()
        + size_of::<Result<SharedWeighted, ByteFailure>>()
        + size_of::<Result<(), ByteFailure>>();
    #[cfg(test)]
    thread_local! {pub(super) static FINISH_CALLS: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };}
    pub(super) fn finish_weighted(
        function: &ResolvedFunction,
        budget: &mut ReplayBudget,
        state: Weighted,
    ) -> Result<SharedWeighted, ByteFailure> {
        #[cfg(test)]
        FINISH_CALLS.with(|calls| calls.set(calls.get() + 1));
        if budget.pays_allocations() {
            budget.reserve_compact_bytes(
                function,
                FINISH_BODY_CONTROLS,
                "factored moved many weighted enum receiving",
            )?;
        }
        budget.reserve_compact_bytes(
            function,
            physical_clone::rc_backing_bytes::<SmallWeighted>(),
            "factored weighted shared header",
        )?;
        let shared = SmallWeighted::Many(state);
        Ok(Rc::new(shared))
    }

    macro_rules! add_flat {
        ($function:ident, $budget:ident, $targets:ident, $child:expr, $weight:expr,
     $attempt:ident, $overflow:block) => {{
            $attempt!($budget.reserve_compact_bytes(
                $function,
                0,
                "factored flat target authentication"
            ));
            let mut cursor = active_targets::FlatCursor {
                targets: $targets,
                child: $child,
                weight: $weight,
                low: 0,
                high: 0,
                middle: 0,
                key: 0,
                comparisons: 0,
                found: false,
                sum: None,
            };
            cursor.high = cursor.targets.len();
            cursor.comparisons = (usize::BITS - cursor.high.leading_zeros()) as usize + 1;
            // Preserve the complete predecessor's conservative work bound. Only
            // the removed callback and helper transport stop forming byte slots.
            $attempt!($budget.charge_compact(
                $function,
                cursor.comparisons,
                "factored target search"
            ));
            while cursor.low < cursor.high {
                cursor.middle = cursor.low + (cursor.high - cursor.low) / 2;
                cursor.key = cursor.targets[cursor.middle].0;
                if cursor.key == cursor.child {
                    cursor.low = cursor.middle;
                    cursor.found = true;
                    break;
                }
                if cursor.key < cursor.child {
                    cursor.low = cursor.middle + 1;
                } else {
                    cursor.high = cursor.middle;
                }
            }
            if cursor.found {
                cursor.sum = cursor.targets[cursor.low].1.checked_add(cursor.weight);
                let Some(total) = cursor.sum else { $overflow };
                cursor.targets[cursor.low].1 = total;
            } else {
                $attempt!($budget.charge_compact(
                    $function,
                    cursor.targets.len() - cursor.low + 1,
                    "factored target insertion and shifts"
                ));
                $attempt!($budget.reserve_compact_vec(
                    $function,
                    cursor.targets,
                    1,
                    "factored target weights"
                ));
                cursor
                    .targets
                    .insert(cursor.low, (cursor.child, cursor.weight));
            }
        }};
    }
    pub(super) use add_flat;

    #[cfg(test)]
    mod tests {
        include!("active_tests.rs");
    }
}

mod receiving {
    use super::super::super::super::OnePassLabels;
    pub(super) use super::super::payments::*;
    use super::*;
    // Every current iterator, owned output and payment role remains. This
    // method has no general successor actual/formal tuple, Result or Option.
    const ACTIVE_RECEIVING: usize = 2 * size_of::<OnePassLabels<'static>>()
        + 2 * size_of::<ordered_weights::Iter<'static>>()
        + 2 * size_of::<Option<(&usize, &u64)>>()
        + 2 * size_of::<(&usize, &u64)>()
        + 2 * size_of::<&[(Label, usize)]>()
        + 2 * size_of::<std::slice::Iter<'static, (Label, usize)>>()
        + 2 * size_of::<Option<(&Label, &usize)>>()
        + 2 * size_of::<(&Label, &usize)>()
        + 2 * size_of::<usize>()
        + 2 * size_of::<Vec<(Label, SharedWeighted)>>()
        + ONE_PASS_ZERO_BYTES_CALL
        + ONE_PASS_ZERO_WORK_CALL
        + ONE_PASS_SUBSET_CALL
        + ONE_PASS_VECTOR_CALL
        // Each new payment's own call/Result is held before that payment.
        + ONE_PASS_MOVEMENT_PAYMENT
        + ONE_PASS_PROMOTION_PAYMENT
        + ONE_PASS_FIRST_LABEL_NODE
        + ONE_PASS_SECOND_LABEL_NODE
        + ONE_PASS_PROMOTION_AUTH
        + ONE_PASS_LATER_LABEL_NODE
        + ONE_PASS_LATER_LABEL_AUTH
        + ONE_PASS_EMPTY_LABEL_AUTH
        + ONE_PASS_PUBLICATION_PAYMENT
        + ONE_PASS_FINAL_TRANSPORT_PAYMENT
        + ONE_PASS_UNIT_ABI
        + ONE_PASS_ITERATOR_PAYMENT
        // Two actual macro expansions: inline outer label / tree outer label.
        + ONE_PASS_INLINE_TARGET_PAYMENTS
        + ONE_PASS_TREE_TARGET_PAYMENTS
        + ONE_PASS_INLINE_PUBLISH_PAYMENTS
        + ONE_PASS_TREE_PUBLISH_PAYMENTS
        - Canonicalizer::ACTIVE_REMOVED_LABEL_CALLS
        - Canonicalizer::ACTIVE_REMOVED_TARGET_NODE_FRAMES;
    const ACTIVE_PRIMITIVE_FRAMES: usize =
        PRIMITIVE_FRAMES - Canonicalizer::ACTIVE_REMOVED_TARGET_NODE_FRAMES;
    pub(super) const COLD_RECEIVING: usize = ACTIVE_RECEIVING
        + PER_USE_SELECTION
        + Canonicalizer::ACTIVE_TARGET_FRAMES
        + Canonicalizer::ACTIVE_APPEND_LABEL_FRAMES;
    pub(super) const WARM_RECEIVING: usize =
        ACTIVE_RECEIVING - ACTIVE_PRIMITIVE_FRAMES + PER_USE_SELECTION;
}
use receiving as compact_successors;
type Dispatch<'hir> = fn(
    &mut Canonicalizer<'hir>,
    &ResolvedFunction,
    Weighted,
    &mut ReplayBudget,
    &mut closure_receiving::ClosureReceiving,
) -> Result<Option<usize>, Diagnostic>;
impl<'hir> Canonicalizer<'hir> {
    pub(in crate::cleanup_plan::replay::factored::canonical_shared) const ACTIVE_APPEND_LABEL_FRAMES: usize = active_targets::APPEND_LABEL_FRAMES + legacy_payments::ONE_PASS_UNIT_ABI - legacy_payments::ONE_PASS_NODE_ABI;
    #[cfg(test)]
    pub(in crate::cleanup_plan::replay::factored::canonical_shared) fn active_avoided_label_nodes(
        reset: bool,
    ) -> usize {
        active_targets::AVOIDED_LABEL_NODES
            .with(|count| if reset { count.replace(0) } else { count.get() })
    }
    #[cfg(test)]
    pub(in crate::cleanup_plan::replay::factored::canonical_shared) fn active_finish_calls(
        reset: bool,
    ) -> usize {
        active_targets::FINISH_CALLS
            .with(|calls| if reset { calls.replace(0) } else { calls.get() })
    }
    #[cfg(test)]
    pub(in crate::cleanup_plan::replay::factored::canonical_shared) const ACTIVE_REMOVED_WEIGHTED_TRANSPORT: usize =
        2 * (size_of::<Result<SharedWeighted, Diagnostic>>() - size_of::<Result<SharedWeighted, crate::cleanup_plan::replay::physical_budget::ByteFailure>>())
        + 2 * (size_of::<Result<(), Diagnostic>>() - size_of::<Result<(), crate::cleanup_plan::replay::physical_budget::ByteFailure>>());
    pub(in crate::cleanup_plan::replay::factored::canonical_shared) const ACTIVE_REMOVED_LABEL_CALLS: usize =
        2 * legacy_payments::ONE_PASS_WEIGHTED_FINISH;
    pub(in crate::cleanup_plan::replay::factored::canonical_shared) const ACTIVE_REMOVED_TARGET_NODE_FRAMES: usize =
        2 * legacy_payments::ONE_PASS_NODE_ABI;
    pub(in crate::cleanup_plan::replay::factored::canonical_shared) const ACTIVE_TARGET_FRAMES:
        usize = 2 * active_targets::PRIMITIVE_FRAMES;
    // The selected method-pointer initializer/receiver/invocation places, the
    // payer borrow and conditional result are acquired on the caller's existing
    // held-slot frontier, before selection. Both weighted roots reuse them.
    pub(in crate::cleanup_plan::replay::factored) const ACTIVE_DISPATCH_CONTROLS: usize = 2
        * size_of::<(&Canonicalizer<'static>, &AllocationPayer)>()
        + 4 * size_of::<Dispatch<'static>>()
        + 2 * size_of::<&AllocationPayer>()
        + 2 * size_of::<bool>();

    pub(in crate::cleanup_plan::replay::factored) fn canonical_dispatch(
        &self,
        payer: &AllocationPayer,
    ) -> Dispatch<'hir> {
        match payer {
            AllocationPayer::ActiveBuilder => Self::id_with_active_receiving,
            _ => Self::id_with_compact_receiving,
        }
    }
}

impl<'hir> Canonicalizer<'hir> {
    #[allow(
        clippy::iter_next_slice,
        clippy::single_match,
        reason = "The iterator receiver is explicitly prepaid and compared with the complete predecessor. The retained arity dispatch and its exact paid receiving controls are intentionally unchanged."
    )]
    pub(in crate::cleanup_plan::replay::factored::canonical_shared) fn id_inner_into_active(
        &mut self,
        function: &ResolvedFunction,
        root: Weighted,
        budget: &mut ReplayBudget,
        receiving: &mut closure_receiving::ClosureReceiving,
    ) {
        macro_rules! attempt {
            ($operation:expr) => {
                match $operation {
                    Ok(value) => value,
                    Err(error) => {
                        receiving.refusal = Some(error);
                        receiving.ready = true;
                        return;
                    }
                }
            };
        }
        macro_rules! compact_attempt {
            ($operation:expr) => {
                match $operation {
                    Ok(value) => value,
                    Err(error) => {
                        receiving.refusal = Some(error.into_diagnostic());
                        receiving.ready = true;
                        return;
                    }
                }
            };
        }
        macro_rules! schedule_vector {
            ($state:ident, $accepted:ident, $successors:ident) => {{
                let child_count = $successors.len();
                attempt!(budget.reserve_vec(
                    function,
                    &mut self.frames,
                    1,
                    "factored canonical exit frame",
                ));
                let exit_index = self.frames.len();
                self.frames
                    .push(CanonicalFrame::Exit($state, $accepted, $successors));
                for index in 0..child_count {
                    let CanonicalFrame::Exit(_, _, $successors) = &self.frames[exit_index] else {
                        unreachable!("canonical exit retained below child frames");
                    };
                    let child = attempt!(share_weighted(function, budget, &$successors[index].1));
                    attempt!(budget.reserve_vec(
                        function,
                        &mut self.frames,
                        1,
                        "factored canonical child frames",
                    ));
                    self.frames.push(CanonicalFrame::Enter(child));
                }
            }};
        }
        self.successor_unit_paid = false;
        self.successor_general_paid = false;
        receiving.overflow = false;
        attempt!(budget.reserve_vec(
            function,
            &mut self.frames,
            1,
            "factored canonical initial frame",
        ));
        // This direct root construction has no Diagnostic-bearing helper Result.
        // Both enum values, actual Rc initializer/receiving values and payment
        // Result remain real and prepaid; the general successor helper is unchanged.
        if budget.pays_allocations() {
            attempt!(budget.reserve_bytes(
                function,
                2 * std::mem::size_of::<SmallWeighted>()
                    + 2 * std::mem::size_of::<SharedWeighted>()
                    + std::mem::size_of::<Result<(), Diagnostic>>()
                    + borrowed_single::LOCAL_CONTROLS
                    + inline_exit::PRIMITIVES,
                "factored direct root enum receiving",
            ));
        }
        attempt!(budget.reserve_bytes(
            function,
            physical_clone::rc_backing_bytes::<SmallWeighted>(),
            "factored weighted shared header",
        ));
        let shared = SmallWeighted::Many(root);
        let root = Rc::new(shared);
        let root_copy = attempt!(share_weighted(function, budget, &root));
        self.frames.push(CanonicalFrame::Enter(root_copy));
        // The original direct-root admission above prepaid both bool values.
        // This one primitive frame belongs to this nonrecursive activation.
        let mut exit_admission_paid = false;
        // No initializer value is formed. This stable receiving place retains
        // the ORIGINAL Vec::len result used by the signature-work call.
        let mut exit_arity: usize;
        'canonical_frames: while let Some(frame) = self.frames.pop() {
            attempt!(budget.charge(function, 1, "factored canonical frame"));
            match frame {
                CanonicalFrame::Enter(state) => {
                    if self.states.contains_key(&state) {
                        continue;
                    }
                    if self.open.contains(&state) {
                        receiving.refusal =
                            Some(replay_error(function, "cleanup CFG contains a cycle"));
                        receiving.ready = true;
                        return;
                    }
                    attempt!(budget.reserve_vec(
                        function,
                        &mut self.open,
                        1,
                        "factored retained open-state identity slots",
                    ));
                    self.open
                        .push(attempt!(share_weighted(function, budget, &state)));
                    // Exact old caller receiving places: no payload is formed
                    // on One. Active output moves the original local result
                    // directly here. Legacy destructures the original Some
                    // payload with field-place assignment, constructing no
                    // additional tuple, Vec alias, Option, callback or flag.
                    // Every original output/Result/Vec invoice stays paid.
                    let vector_accepted: u64;
                    let vector_successors: Vec<(Label, SharedWeighted)>;
                    'vector_continuation: {
                        macro_rules! publish_vector {
                            ($a:expr, $rows:ident) => {{
                                vector_accepted = $a;
                                vector_successors = $rows;
                                break 'vector_continuation;
                            }};
                        }

                        let Some((accepted, label, child)) = active_successors!(self, function, state, budget, receiving, attempt, compact_attempt, publish_vector, 'canonical_frames)
                        else {
                            receiving.finish_overflow();
                            return;
                        };
                        attempt!(budget.reserve_vec(
                            function,
                            &mut self.frames,
                            1,
                            "factored canonical exit frame"
                        ));
                        let exit_index = self.frames.len();
                        self.frames
                            .push(CanonicalFrame::ExitOne(state, accepted, (label, child)));
                        let CanonicalFrame::ExitOne(_, _, (_, child)) = &self.frames[exit_index]
                        else {
                            unreachable!("inline exit retained below child")
                        };
                        let child = attempt!(share_weighted(function, budget, child));
                        attempt!(budget.reserve_vec(
                            function,
                            &mut self.frames,
                            1,
                            "factored canonical child frames"
                        ));
                        self.frames.push(CanonicalFrame::Enter(child));
                        continue 'canonical_frames;
                    }
                    // One real vector scheduling site for all four source
                    // branches; no callback or initialized Option carrier.
                    schedule_vector!(state, vector_accepted, vector_successors);
                }
                CanonicalFrame::ExitOne(state, accepted, row) => {
                    self.open.retain(|entry| entry != &state);
                    attempt!(budget.charge(
                        function,
                        1,
                        "factored canonical signature construction"
                    ));
                    attempt!(budget.reserve_bytes(
                        function,
                        inline_exit::RECEIVING,
                        "canonical inline exit receiving"
                    ));
                    let Some(child_id) = self.states.get(&row.1) else {
                        receiving.refusal =
                            Some(replay_error(function, "cleanup CFG contains a cycle"));
                        receiving.ready = true;
                        return;
                    };
                    let query = borrowed_single::OneQuery {
                        accepted,
                        label: row.0,
                        child: child_id,
                    };
                    let id = match self.ids.get(&query) {
                        Some(id) => {
                            #[cfg(test)]
                            arena_tests::signature_lookup(true);
                            #[cfg(test)]
                            inline_exit::census::signature(true, function, budget);
                            *id
                        }
                        None => {
                            #[cfg(test)]
                            arena_tests::signature_lookup(false);
                            #[cfg(test)]
                            inline_exit::census::signature(false, function, budget);
                            attempt!(budget.reserve_array::<(Label, usize)>(
                                function,
                                1,
                                "factored canonical signature vector"
                            ));
                            let mut key = Vec::with_capacity(1);
                            {
                                let (label, _child) = row;
                                key.push((Label::clone(label), *child_id));
                            }
                            let key = (accepted, key);
                            intern_arena::prepare_insert!(
                                self.ids,
                                intern_arena::Signature,
                                function,
                                budget
                            );
                            let id = self.ids.len();
                            self.ids.insert_prepaid(key, id);
                            id
                        }
                    };
                    self.child_ids.clear();
                    if !self.states.contains_key(&state) {
                        intern_arena::prepare_insert!(
                            self.states,
                            SharedWeighted,
                            function,
                            budget
                        );
                    }
                    self.states.insert_prepaid(state, id);
                    continue;
                }
                CanonicalFrame::Exit(state, accepted, mut successors) => {
                    self.open.retain(|entry| entry != &state);
                    exit_arity = successors.len();
                    attempt!(budget.charge(
                        function,
                        exit_arity,
                        "factored canonical signature construction",
                    ));
                    // The pattern reads the retained place: no extra len(),
                    // moved scrutinee local, query or helper return exists.
                    // This paid place-pattern keeps the independent typed oracle; an if adds a boolean operand.
                    #[allow(clippy::single_match)]
                    match exit_arity {
                        1 => {
                            #[cfg(test)]
                            borrowed_single::note(budget, exit_admission_paid);
                            attempt!(budget.reserve_bytes(
                                function,
                                if exit_admission_paid {
                                    borrowed_single::WARM
                                } else {
                                    borrowed_single::COLD
                                },
                                "canonical lexical exit receiving",
                            ));
                            exit_admission_paid = true;
                            // One original iterator.next, without an Index or slice
                            // helper. Its temporary iterator ends before remove(0).
                            // Retain the funded Iterator/Option representation; first() has a different receiver shape.
                            #[allow(clippy::iter_next_slice)]
                            let Some((label, child)) = successors.iter().next() else {
                                unreachable!("one successor was matched");
                            };
                            let Some(child_id) = self.states.get(child) else {
                                receiving.refusal =
                                    Some(replay_error(function, "cleanup CFG contains a cycle"));
                                receiving.ready = true;
                                return;
                            };
                            let query = borrowed_single::OneQuery {
                                accepted,
                                label,
                                child: child_id,
                            };
                            let id = match self.ids.get(&query) {
                                Some(id) => {
                                    #[cfg(test)]
                                    arena_tests::signature_lookup(true);
                                    #[cfg(test)]
                                    inline_exit::census::signature(true, function, budget);
                                    *id
                                }
                                None => {
                                    #[cfg(test)]
                                    arena_tests::signature_lookup(false);
                                    #[cfg(test)]
                                    inline_exit::census::signature(false, function, budget);
                                    attempt!(budget.reserve_array::<(Label, usize)>(
                                        function,
                                        1,
                                        "factored canonical signature vector",
                                    ));
                                    let mut key = Vec::with_capacity(1);
                                    {
                                        let (label, _child) = successors.remove(0);
                                        key.push((label, *child_id));
                                    }
                                    // Same owned Vec input slots as the original
                                    // into_iter hand-off; release its buffer before
                                    // either arena can acquire backing, as before.
                                    drop(successors);
                                    let key = (accepted, key);
                                    intern_arena::prepare_insert!(
                                        self.ids,
                                        intern_arena::Signature,
                                        function,
                                        budget
                                    );
                                    let id = self.ids.len();
                                    self.ids.insert_prepaid(key, id);
                                    id
                                }
                            };
                            self.child_ids.clear();
                            if !self.states.contains_key(&state) {
                                intern_arena::prepare_insert!(
                                    self.states,
                                    SharedWeighted,
                                    function,
                                    budget
                                );
                            }
                            self.states.insert_prepaid(state, id);
                            continue;
                        }
                        _ => {}
                    }
                    // Empty/general routes retain their complete original
                    // admission, predicate, query/scratch/miss and Drop order.
                    if budget.pays_allocations() {
                        attempt!(budget.reserve_bytes(
                            function,
                            empty_signature::BRANCH_CONTROLS,
                            "canonical empty signature branch receiving",
                        ));
                    }
                    if successors.is_empty() {
                        if budget.pays_allocations() {
                            attempt!(budget.reserve_bytes(
                                function,
                                empty_signature::EMPTY_CONTROLS,
                                "canonical empty signature receiving",
                            ));
                        }
                        self.child_ids.clear();
                        // Retain the original key-vector zero authentication;
                        // no child directory, borrowed query or Zip exists here.
                        attempt!(budget.reserve_array::<(Label, usize)>(
                            function,
                            0,
                            "factored canonical signature vector",
                        ));
                        let key = (accepted, Vec::new());
                        let id = match self.ids.get(&key) {
                            Some(id) => {
                                #[cfg(test)]
                                arena_tests::signature_lookup(true);
                                #[cfg(test)]
                                inline_exit::census::signature(true, function, budget);
                                *id
                            }
                            None => {
                                #[cfg(test)]
                                arena_tests::signature_lookup(false);
                                #[cfg(test)]
                                inline_exit::census::signature(false, function, budget);
                                intern_arena::prepare_insert!(
                                    self.ids,
                                    intern_arena::Signature,
                                    function,
                                    budget
                                );
                                let id = self.ids.len();
                                self.ids.insert_prepaid(key, id);
                                id
                            }
                        };
                        self.child_ids.clear();
                        if !self.states.contains_key(&state) {
                            intern_arena::prepare_insert!(
                                self.states,
                                SharedWeighted,
                                function,
                                budget
                            );
                        }
                        self.states.insert_prepaid(state, id);
                        continue;
                    }
                    attempt!(budget.reserve_bytes(
                        function,
                        intern_arena::SignatureQuery::LEXICAL_EXIT_CONTROLS,
                        "canonical borrowed signature receiving",
                    ));
                    // Retained scalar scratch contains only the child IDs for
                    // this frame; the epsilon cursor keeps its original shape.
                    self.child_ids.clear();
                    lexical_child_scratch::prepare!(self, successors, function, budget, receiving);
                    attempt!(budget.reserve_array::<[usize; 2]>(
                        function,
                        successors.len(),
                        "canonical child identity receiving",
                    ));
                    // Resolve every ID before signature comparison can stop at
                    // its first mismatch. This preserves the original failure order.
                    for (_, child) in &successors {
                        let Some(id) = self.states.get(child) else {
                            receiving.refusal =
                                Some(replay_error(function, "cleanup CFG contains a cycle"));
                            receiving.ready = true;
                            return;
                        };
                        self.child_ids.push(*id);
                    }
                    let query = intern_arena::SignatureQuery {
                        accepted,
                        successors: &successors,
                        children: &self.child_ids,
                    };
                    let id = match self.ids.get(&query) {
                        Some(id) => {
                            #[cfg(test)]
                            arena_tests::signature_lookup(true);
                            #[cfg(test)]
                            inline_exit::census::signature(true, function, budget);
                            *id
                        }
                        None => {
                            #[cfg(test)]
                            arena_tests::signature_lookup(false);
                            #[cfg(test)]
                            inline_exit::census::signature(false, function, budget);
                            attempt!(budget.reserve_array::<(Label, usize)>(
                                function,
                                successors.len(),
                                "factored canonical signature vector",
                            ));
                            // Every ID is already resolved. Move each original
                            // label with its borrowed ID, without an owned
                            // scalar cursor or repeated index/increment operands.
                            attempt!(budget.reserve_bytes(
                                function,
                                intern_arena::SignatureQuery::MISS_CONTROLS,
                                "canonical signature miss iterator receiving",
                            ));
                            attempt!(budget.reserve_array::<[&usize; 2]>(
                                function,
                                successors.len(),
                                "canonical signature miss borrowed IDs",
                            ));
                            let mut key = Vec::with_capacity(successors.len());
                            for ((label, _child), id) in successors.into_iter().zip(&self.child_ids)
                            {
                                key.push((label, *id));
                            }
                            let key = (accepted, key);
                            intern_arena::prepare_insert!(
                                self.ids,
                                intern_arena::Signature,
                                function,
                                budget
                            );
                            let id = self.ids.len();
                            self.ids.insert_prepaid(key, id);
                            id
                        }
                    };
                    self.child_ids.clear();
                    if !self.states.contains_key(&state) {
                        intern_arena::prepare_insert!(
                            self.states,
                            SharedWeighted,
                            function,
                            budget
                        );
                    }
                    self.states.insert_prepaid(state, id);
                }
            }
        }
        match self.states.get(&root) {
            Some(id) => receiving.id = *id,
            None => receiving.overflow = true,
        }
        receiving.ready = true;
    }
}

impl Canonicalizer<'_> {
    pub(in crate::cleanup_plan::replay::factored) fn id_with_active_receiving(
        &mut self,
        function: &ResolvedFunction,
        root: Weighted,
        budget: &mut ReplayBudget,
        receiving: &mut closure_receiving::ClosureReceiving,
    ) -> Result<Option<usize>, Diagnostic> {
        #[cfg(test)]
        let _successor_invocation = lexical_successors::census::invocation(false, function, budget);
        #[cfg(test)]
        let _operation = crate::bounded_output::stage_debits::operation_guard(
            crate::bounded_output::stage_debits::Operation::ReplayFactoredCanonical,
        );
        #[cfg(feature = "unstable-workflow-profiling")]
        let _exclusive_operation = crate::bounded_output::exclusive_debits::operation_guard(
            crate::bounded_output::exclusive_debits::Operation::ReplayFactoredCanonical,
        );
        #[cfg(test)]
        crate::cleanup_plan::replay::factored::canonical_shared::tests::note_actual_entry(budget);
        // Authenticate even when states already contains the requested root.
        // One equivalent() call holds this function and budget borrowed across
        // both roots; no closure/cache view leaves that invocation.
        // Validate the held destination before any ordinary payer query.
        if !receiving.valid(function, budget) {
            self.closures.reject_after_error();
            return Err(Diagnostic::io("SPX-H006", String::new()));
        }
        authenticate_closure!(self, function, budget, receiving);
        if receiving.refusal.is_some() {
            return Err(receiving.refusal.take().expect("published refusal"));
        }
        if budget.pays_allocations() {
            budget.reserve_bytes(
                function,
                std::mem::size_of::<Weighted>(),
                "factored canonical moved root",
            )?;
        }
        // Keep first-use acquisition at its original lexical position, but
        // construct no forwarding prepare_scratch Result or argument receivers.
        if !self.scratch_paid {
            if budget.pays_allocations() {
                budget.reserve_bytes(
                    function,
                    std::mem::size_of::<Vec<(usize, bool)>>()
                        + std::mem::size_of::<Vec<CanonicalFrame<'_>>>()
                        + std::mem::size_of::<Vec<SharedWeighted>>()
                        + std::mem::size_of::<bool>(),
                    "factored retained canonical scratch controls",
                )?;
            }
            self.scratch_paid = true;
        }
        self.id_inner_into_active(function, root, budget, receiving);
        // Retain capacity, never owned references, across either root and all
        // success, arithmetic, work, physical and semantic failure exits.
        self.cursor.clear();
        self.child_ids.clear();
        self.frames.clear();
        self.open.clear();
        receiving.ready = false;
        if receiving.refusal.is_some() {
            return Err(receiving.refusal.take().expect("published refusal"));
        }
        if receiving.overflow {
            return Ok(None);
        }
        Ok(Some(receiving.id))
    }
}
