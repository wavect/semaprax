//! Retained ActiveBuilder successor sites in the canonical frame activation.
use super::*;
use std::mem::size_of;
type Successors = (u64, Vec<(Label, SharedWeighted)>);
type InlineSuccessors<'a> = (u64, &'a Label, SharedWeighted);
// Exact same producer-output and caller-destructure role as old Successors:
// accepted:u64 survives, the old Vec<Row> output is replaced by borrowed Label/owned Rc.
// No old output/Vec is live on this branch. The single Option receiver and
// both complete Result compatibility slots in ORIGINAL_RECEIVING stay paid
// at their original frontier and map individually, never to an unrelated bank.
const _: () = assert!(size_of::<InlineSuccessors<'static>>() <= size_of::<Successors>());
const _: () =
    assert!(size_of::<Option<InlineSuccessors<'static>>>() <= size_of::<Option<Successors>>());
const _: () = assert!(
    size_of::<Result<Option<InlineSuccessors<'static>>, Diagnostic>>()
        <= size_of::<Result<Option<Successors>, Diagnostic>>()
);

pub(super) enum OnePassLabels<'a> {
    Empty,
    One(&'a Label, paid_many::Targets),
    Many(BTreeMap<&'a Label, paid_many::Targets>),
}
// Actual call/formal/return, two initializer/receiver values, and the original
// traversal's iterator/next/tuple slots. Per-movement storage/query roles below
// are acquired before the branch that forms them, rather than priced by width.
pub(super) const ORIGINAL_RECEIVING: usize = 2 * size_of::<(&ResolvedFunction, &mut ReplayBudget, &Automaton, &ordered_weights::OrderedWeights, u64)>()
        + 2 * size_of::<Result<Option<Successors>, Diagnostic>>()
        + size_of::<Option<Successors>>()
        + 2 * size_of::<OnePassLabels<'static>>()
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
        + ONE_PASS_TREE_PUBLISH_PAYMENTS;
// Each constant names ONE real lexical call, with actual and formal argument
// tuples and returned/received Result slots. No bank is credited twice.
type OnePassByteCall<'a> = (&'a mut ReplayBudget, &'a ResolvedFunction, usize, &'a str);
type OnePassVectorCall<'a> = (&'a mut ReplayBudget, &'a ResolvedFunction, usize, &'a str);
pub(super) const ONE_PASS_ZERO_BYTES_CALL: usize =
    2 * size_of::<OnePassByteCall<'static>>() + 2 * size_of::<Result<(), Diagnostic>>();
pub(super) const ONE_PASS_ZERO_WORK_CALL: usize =
    2 * size_of::<OnePassByteCall<'static>>() + 2 * size_of::<Result<(), Diagnostic>>();
// The subset charge is the same original one-pass call/arguments, not a new
// selector. Its Result is consumed by ? before any next node or recursive call;
// this nonrecursive producer retains no charge receiver between iterations.
pub(super) const ONE_PASS_SUBSET_CALL: usize =
    2 * size_of::<OnePassByteCall<'static>>() + 2 * size_of::<Result<(), Diagnostic>>();
pub(super) const ONE_PASS_VECTOR_CALL: usize =
    2 * size_of::<OnePassVectorCall<'static>>() + 2 * size_of::<Result<(), Diagnostic>>();
type OnePassNodeCall<'a> = (&'a mut ReplayBudget, &'a ResolvedFunction, &'a str);
pub(super) const ONE_PASS_UNIT_ABI: usize =
    2 * size_of::<OnePassByteCall<'static>>() + 2 * size_of::<Result<(), Diagnostic>>();
pub(super) const ONE_PASS_NODE_ABI: usize =
    2 * size_of::<OnePassNodeCall<'static>>() + 2 * size_of::<Result<(), Diagnostic>>();
pub(super) const ONE_PASS_MOVEMENT_PAYMENT: usize = ONE_PASS_UNIT_ABI;
pub(super) const ONE_PASS_PROMOTION_PAYMENT: usize = ONE_PASS_UNIT_ABI;
pub(super) const ONE_PASS_FIRST_LABEL_NODE: usize = ONE_PASS_NODE_ABI;
pub(super) const ONE_PASS_SECOND_LABEL_NODE: usize = ONE_PASS_NODE_ABI;
pub(super) const ONE_PASS_PROMOTION_AUTH: usize = ONE_PASS_UNIT_ABI;
pub(super) const ONE_PASS_LATER_LABEL_NODE: usize = ONE_PASS_NODE_ABI;
pub(super) const ONE_PASS_LATER_LABEL_AUTH: usize = ONE_PASS_UNIT_ABI;
pub(super) const ONE_PASS_EMPTY_LABEL_AUTH: usize = ONE_PASS_UNIT_ABI;
pub(super) const ONE_PASS_PUBLICATION_PAYMENT: usize = ONE_PASS_UNIT_ABI;
pub(super) const ONE_PASS_FINAL_TRANSPORT_PAYMENT: usize = ONE_PASS_UNIT_ABI;
pub(super) const ONE_PASS_ITERATOR_PAYMENT: usize = ONE_PASS_UNIT_ABI;
// add!: reserve PROMOTE_RECEIVING, reserve ADD_RECEIVING, reserve target node.
pub(super) const ONE_PASS_INLINE_TARGET_PAYMENTS: usize =
    ONE_PASS_UNIT_ABI + ONE_PASS_UNIT_ABI + ONE_PASS_NODE_ABI;
pub(super) const ONE_PASS_TREE_TARGET_PAYMENTS: usize =
    ONE_PASS_UNIT_ABI + ONE_PASS_UNIT_ABI + ONE_PASS_NODE_ABI;
// publish!: row bank, inline enum bank, Rc backing, paidMany finish bank.
// The legacy weighted helper has its own actual/formal/Result transport here.
pub(super) const ONE_PASS_WEIGHTED_FINISH: usize =
    2 * size_of::<(&ResolvedFunction, &mut ReplayBudget, BTreeMap<usize, u64>)>()
        + 2 * size_of::<Result<SharedWeighted, Diagnostic>>();
pub(super) const ONE_PASS_INLINE_PUBLISH_PAYMENTS: usize = ONE_PASS_UNIT_ABI
    + ONE_PASS_UNIT_ABI
    + ONE_PASS_UNIT_ABI
    + ONE_PASS_UNIT_ABI
    + ONE_PASS_WEIGHTED_FINISH;
pub(super) const ONE_PASS_TREE_PUBLISH_PAYMENTS: usize = ONE_PASS_UNIT_ABI
    + ONE_PASS_UNIT_ABI
    + ONE_PASS_UNIT_ABI
    + ONE_PASS_UNIT_ABI
    + ONE_PASS_WEIGHTED_FINISH;
pub(super) const ONE_PASS_MOVEMENT: usize = 2 * size_of::<&mut OnePassLabels<'static>>()
    + 2 * size_of::<OnePassLabels<'static>>() // fresh inline One initializer/receiver
    + 2 * size_of::<&Label>() + 2 * size_of::<&usize>()
    + 2 * size_of::<&mut paid_many::Targets>()
    // add! has two One patterns (same child / distinct child), each binding
    // its actual mutable child and weight fields before checked_add/promote.
    + 2 * size_of::<&mut usize>() + 2 * size_of::<&mut u64>()
    // Real contains_key/get_mut receivers on the borrowed-label tree.
    + 2 * size_of::<&BTreeMap<&Label, paid_many::Targets>>()
    + 2 * size_of::<&mut BTreeMap<&Label, paid_many::Targets>>()
    + 2 * size_of::<&Label>()
    + 2 * size_of::<Option<&mut paid_many::Targets>>()
    + 2 * size_of::<Option<u64>>() + 4 * size_of::<u64>()
    + 4 * size_of::<bool>()
    + 2 * size_of::<paid_many::Targets>()
    + 2 * size_of::<Option<paid_many::Targets>>();
pub(super) const ONE_PASS_PROMOTION: usize = 2 * size_of::<BTreeMap<&Label, paid_many::Targets>>()
    // mem::replace: actual/formal Empty + returned/bound former One.
    + 4 * size_of::<OnePassLabels<'static>>()
    + 2 * size_of::<&mut OnePassLabels<'static>>()
    // Two actual/formal insert tuples and returned/received insertion Options.
    + 4 * size_of::<(&Label, paid_many::Targets)>()
    + 4 * size_of::<&mut BTreeMap<&Label, paid_many::Targets>>()
    + 2 * size_of::<paid_many::Targets>()
    + 4 * size_of::<Option<paid_many::Targets>>();
pub(super) const ONE_PASS_PUBLICATION: usize = 2 * size_of::<usize>() + inline_exit::SELECT;
pub(super) const ONE_PASS_PUBLISHED_ROW: usize = 2 * size_of::<paid_many::Targets>()
    + 2 * size_of::<SharedWeighted>()
    + size_of::<(Label, SharedWeighted)>();
pub(super) const ONE_PASS_FINAL_TRANSPORT: usize = 2 * size_of::<OnePassLabels<'static>>()
    + 2 * size_of::<BTreeMap<&Label, paid_many::Targets>>()
    + 2 * size_of::<(&Label, paid_many::Targets)>()
    + 2 * size_of::<paid_many::Targets>()
    + 2 * size_of::<&Label>();
pub(super) const ONE_PASS_OWNING_ITERATOR: usize =
    size_of::<std::collections::btree_map::IntoIter<&'static Label, paid_many::Targets>>()
        + 2 * size_of::<Option<(&Label, paid_many::Targets)>>()
        + 2 * size_of::<(&Label, paid_many::Targets)>();

// 25 byte/work/vector primitive sites + five node sites. BOTH owned weighted
// finish ABIs are compound payloads and stay outside this retained bank.
pub(super) const PRIMITIVE_FRAMES: usize = 25 * ONE_PASS_UNIT_ABI + 5 * ONE_PASS_NODE_ABI;
// Resident flag test/store and selected scalar operand remain conservatively
// priced on every expansion. No fee helper/callback/runtime arithmetic exists.
pub(super) const PER_USE_SELECTION: usize = 4 * size_of::<bool>() + 2 * size_of::<usize>();
pub(super) const WARM_RECEIVING: usize = ORIGINAL_RECEIVING - PRIMITIVE_FRAMES + PER_USE_SELECTION;
pub(super) const COLD_RECEIVING: usize = ORIGINAL_RECEIVING + PER_USE_SELECTION;
const _: () = assert!(PER_USE_SELECTION < PRIMITIVE_FRAMES);

macro_rules! lexical_one_pass_successors {
    ($function:ident, $budget:ident, $automaton:expr, $closed:ident, $accepted:ident, $receiving:ident, $attempt:ident, $closed_owner:ident, $closed_type:ty, $unit:expr, $vector:ident, $frames:lifetime) => {{
        'one_pass: {
            // The actual canonical caller has already authenticated its Memo and held
            // receiver. Keep zero-byte sticky authentication even for no movements.
            $attempt!($budget.reserve_bytes(
                $function,
                0,
                "factored one-pass successor authentication"
            ));
            $attempt!($budget.charge($function, 0, "factored one-pass successor authentication"));
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
                            $attempt!($budget.reserve_bytes(
                                $function,
                                paid_many::PROMOTE_RECEIVING,
                                "factored one-pass distinct-target receivers"
                            ));
                            #[cfg(test)]
                            if $budget.pays_allocations() {
                                paid_many::note_branch(true);
                            }
                            *$targets = $attempt!(paid_many::promote(
                                $function, $budget, *first, *total, *$child, *$weight
                            ));
                        }
                        paid_many::Targets::PaidMany(flat) => {
                            $attempt!($budget.reserve_bytes(
                                $function,
                                paid_many::ADD_RECEIVING,
                                "factored one-pass flat-target receivers"
                            ));
                            if !$attempt!(paid_many::add(
                                $function, $budget, flat, *$child, *$weight
                            )) {
                                $receiving.finish_overflow();
                                return;
                            }
                        }
                        paid_many::Targets::Many(targets) => {
                            if !targets.contains_key($child) {
                                $attempt!($budget.reserve_btree_node::<usize, u64>(
                                    $function,
                                    "factored successor target node"
                                ));
                            }
                            let slot = targets.entry(*$child).or_insert(0);
                            let Some(sum) = slot.checked_add(*$weight) else {
                                $receiving.finish_overflow();
                                return;
                            };
                            *slot = sum;
                        }
                    }
                };
            }
            for (node, weight) in $closed.iter() {
                let moves = ($automaton).nodes[*node].moves();
                // This is the complete before_index subset request, at the same point.
                // No label census, search-charge prepass or four-op selector is run.
                $attempt!($budget.charge(
                    $function,
                    moves.len().saturating_add(1),
                    "factored skeleton subset construction"
                ));
                for (label, child) in moves {
                    $attempt!($budget.reserve_bytes(
                        $function,
                        lexical_successors::ONE_PASS_MOVEMENT,
                        "factored one-pass movement receivers"
                    ));
                    match &mut labels {
                        lexical_successors::OnePassLabels::Empty => {
                            $attempt!($budget.reserve_bytes(
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
                            $attempt!($budget.reserve_bytes(
                                $function,
                                lexical_successors::ONE_PASS_PROMOTION,
                                "factored one-pass label promotion receivers"
                            ));
                            $attempt!($budget.reserve_btree_node::<&Label, paid_many::Targets>(
                                $function,
                                "factored one-pass first label node"
                            ));
                            $attempt!($budget.reserve_btree_node::<&Label, paid_many::Targets>(
                                $function,
                                "factored one-pass second label node"
                            ));
                            $attempt!($budget.reserve_bytes(
                                $function,
                                0,
                                "factored singleton target authentication"
                            ));
                            let weight = 0_u64
                                .checked_add(*weight)
                                .expect("zero plus u64 cannot overflow");
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
                                $attempt!($budget
                                    .reserve_btree_node::<&Label, paid_many::Targets>(
                                        $function,
                                        "factored one-pass label node"
                                    ));
                                $attempt!($budget.reserve_bytes(
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
            $attempt!($budget.reserve_bytes(
                $function,
                lexical_successors::ONE_PASS_PUBLICATION,
                "factored one-pass publication receivers"
            ));
            let count = match &labels {
                lexical_successors::OnePassLabels::Empty => 0,
                lexical_successors::OnePassLabels::One(_, _) => 1,
                lexical_successors::OnePassLabels::Many(many) => many.len(),
            };
            $attempt!($budget.reserve_array::<(Label, SharedWeighted)>(
                $function,
                if count == 1 { 0 } else { count },
                "factored successor vector"
            ));
            // The completed One publishes only owned fields. Its outer result
            // stays inside the old complete Option/Result layout ceiling.
            #[cfg(test)]
            inline_exit::census::arity(count, $function, $budget);
            if let lexical_successors::OnePassLabels::One(..) = &labels {
                $attempt!($budget.reserve_bytes(
                    $function,
                    lexical_successors::ONE_PASS_FINAL_TRANSPORT + inline_exit::FIELDS,
                    "factored one-pass consuming publication transport"
                ));
                let lexical_successors::OnePassLabels::One(label, targets) = labels else {
                    unreachable!("selected singleton")
                };
                $attempt!($budget.reserve_bytes(
                    $function,
                    lexical_successors::ONE_PASS_PUBLISHED_ROW,
                    "factored one-pass published row receivers"
                ));
                let state = match targets {
                    paid_many::Targets::One { child, weight } => {
                        $attempt!($budget.reserve_bytes(
                            $function,
                            2 * size_of::<SmallWeighted>() + size_of::<[(usize, u64); 1]>(),
                            "factored inline weighted enum and array receiving"
                        ));
                        $attempt!($budget.reserve_bytes(
                            $function,
                            physical_clone::rc_backing_bytes::<SmallWeighted>(),
                            "factored weighted shared header"
                        ));
                        Rc::new(SmallWeighted::One([(child, weight)]))
                    }
                    paid_many::Targets::PaidMany(flat) => {
                        $attempt!($budget.reserve_bytes(
                            $function,
                            paid_many::FINISH_RECEIVING,
                            "factored one-pass flat finish receivers"
                        ));
                        $attempt!(own_weighted($function, $budget, flat))
                    }
                    paid_many::Targets::Many(targets) => {
                        $attempt!(weighted_successor($function, $budget, targets))
                    }
                };
                break 'one_pass Some(($accepted, label, state));
            }
            let mut result = Vec::with_capacity(count);
            macro_rules! publish {
                ($label:ident, $targets:ident) => {{
                    $attempt!($budget.reserve_bytes(
                        $function,
                        lexical_successors::ONE_PASS_PUBLISHED_ROW,
                        "factored one-pass published row receivers"
                    ));
                    let state = match $targets {
                        paid_many::Targets::One { child, weight } => {
                            $attempt!($budget.reserve_bytes(
                                $function,
                                2 * size_of::<SmallWeighted>() + size_of::<[(usize, u64); 1]>(),
                                "factored inline weighted enum and array receiving"
                            ));
                            $attempt!($budget.reserve_bytes(
                                $function,
                                physical_clone::rc_backing_bytes::<SmallWeighted>(),
                                "factored weighted shared header"
                            ));
                            Rc::new(SmallWeighted::One([(child, weight)]))
                        }
                        paid_many::Targets::PaidMany(flat) => {
                            $attempt!($budget.reserve_bytes(
                                $function,
                                paid_many::FINISH_RECEIVING,
                                "factored one-pass flat finish receivers"
                            ));
                            $attempt!(own_weighted($function, $budget, flat))
                        }
                        paid_many::Targets::Many(targets) => {
                            $attempt!(weighted_successor($function, $budget, targets))
                        }
                    };
                    result.push(($label.clone(), state));
                }};
            }
            // This is a distinct consuming publication, not either initial Empty
            // construction. Pay enum transport and both possible owned pattern bindings
            // before the match can move One's tuple/Targets or Many's map out of labels.
            $attempt!($budget.reserve_bytes(
                $function,
                lexical_successors::ONE_PASS_FINAL_TRANSPORT + 2 * size_of::<$closed_type>(),
                "factored one-pass consuming publication transport"
            ));
            match labels {
                lexical_successors::OnePassLabels::Empty => {}
                lexical_successors::OnePassLabels::One(..) => {
                    unreachable!("singleton returned without a vector")
                }
                lexical_successors::OnePassLabels::Many(many) => {
                    $attempt!($budget.reserve_bytes(
                        $function,
                        lexical_successors::ONE_PASS_OWNING_ITERATOR,
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

macro_rules! lexical_successors {
    ($this:ident, $function:ident, $state:ident, $budget:ident, $receiving:ident, $attempt:ident, $vector:ident, $frames:lifetime) => {{
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
                    $attempt!($budget.reserve_array::<(Label, SharedWeighted)>(
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
                    $attempt!($budget.reserve_bytes(
                        $function,
                        2 * std::mem::size_of::<Rc<ordered_weights::OrderedWeights>>()
                            + 2 * std::mem::size_of::<&ordered_weights::OrderedWeights>(),
                        "factored borrowed closed receiving controls",
                    ));
                }
                let closure = Rc::clone($this.closures.get(node).expect("computed shared closure"));
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
                        lexical_successors::WARM_RECEIVING
                    } else {
                        lexical_successors::COLD_RECEIVING
                    },
                );
                $attempt!($budget.reserve_bytes(
                    $function,
                    if $this.successor_unit_paid {
                        lexical_successors::WARM_RECEIVING
                    } else {
                        lexical_successors::COLD_RECEIVING
                    },
                    "factored one-pass label call receivers"
                ));
                $this.successor_unit_paid = true;
                #[cfg(test)]
                lexical_successors::census::acquired();
                break 'lexical_successor lexical_one_pass_successors!(
                    $function,
                    $budget,
                    $this.automaton,
                    closed,
                    accepted,
                    $receiving,
                    $attempt,
                    borrowed,
                    Rc<ordered_weights::OrderedWeights>,
                    true,
                    $vector,
                    $frames
                );
            }
            if $budget.pays_allocations() {
                $attempt!($budget.reserve_bytes(
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
                    lexical_successors::WARM_RECEIVING
                } else {
                    lexical_successors::COLD_RECEIVING
                },
            );
            $attempt!($budget.reserve_bytes(
                $function,
                if $this.successor_general_paid {
                    lexical_successors::WARM_RECEIVING
                } else {
                    lexical_successors::COLD_RECEIVING
                },
                "factored one-pass label call receivers"
            ));
            $this.successor_general_paid = true;
            #[cfg(test)]
            lexical_successors::census::acquired();
            break 'lexical_successor lexical_one_pass_successors!(
                $function,
                $budget,
                $this.automaton,
                closed,
                accepted,
                $receiving,
                $attempt,
                owned,
                ordered_weights::OrderedWeights,
                false,
                $vector,
                $frames
            );
        }
    }};
}

#[cfg(test)]
pub(super) use super::super::lexical_successors::census;
pub(super) mod frame_loop;
