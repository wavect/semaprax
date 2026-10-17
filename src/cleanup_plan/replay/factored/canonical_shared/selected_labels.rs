//! Selected-only owned inline outer labels; no cache or authority is stored.
//! Expanded in the original authenticated successor receiver, after its closed
//! set and before its original subset loop. Legacy/Oracle/Active do not enter.
use super::*;
use std::mem::size_of;

pub(super) enum Groups {
    Empty,
    One { label: Label, targets: paid_many::Targets },
    Many(BTreeMap<Label, paid_many::Targets>),
}
type ByteCall<'a> = (&'a mut ReplayBudget, &'a ResolvedFunction, usize, &'a str);
type NodeCall<'a> = (&'a mut ReplayBudget, &'a ResolvedFunction, &'a str);
const BYTE_ABI: usize = 2 * size_of::<ByteCall<'static>>()
    + 2 * size_of::<Result<(), Diagnostic>>();
const NODE_ABI: usize = 2 * size_of::<NodeCall<'static>>()
    + 2 * size_of::<Result<(), Diagnostic>>();
// Each distinct new lexical payment call is paid before its formation. The
// entry replaces the old label-controls call at the same receiving boundary;
// its unchanged call/Result transport remains owned by that boundary.
// Per-movement, first, promotion, promoted-first node, promoted-second node,
// later node, later initializer, consuming transport, and Many iterator.
pub(super) const ENTRY: usize = 2 * size_of::<Groups>()
    + 6 * BYTE_ABI + 3 * NODE_ABI;
// One actual mutable Label field binding, then the two operands' actual and
// formal immutable references for full Label::eq. These are distinct roles.
const ONE_LABEL_BINDING: usize = size_of::<&mut Label>();
const LABEL_EQUALITY_RECEIVERS: usize = 2 * size_of::<(&Label, &Label)>();
// Many(many) binds a field reference before insert/get_mut forms its own
// actual/formal receiver pair. The field is not credited to that pair.
const MANY_FIELD_BINDING: usize = size_of::<&mut BTreeMap<Label, paid_many::Targets>>();
const MAP_MUTABLE_CALL_RECEIVERS: usize = 2 * size_of::<&mut BTreeMap<Label, paid_many::Targets>>();
pub(super) const MOVEMENT: usize = 2 * size_of::<&mut Groups>()
    + ONE_LABEL_BINDING + LABEL_EQUALITY_RECEIVERS
    + 2 * size_of::<bool>()
    + 2 * size_of::<&mut paid_many::Targets>()
    + 2 * size_of::<&mut usize>() + 2 * size_of::<&mut u64>()
    + size_of::<Option<u64>>()
    + MANY_FIELD_BINDING + MAP_MUTABLE_CALL_RECEIVERS
    + 2 * size_of::<&BTreeMap<Label, paid_many::Targets>>()
    + 2 * size_of::<Option<&mut paid_many::Targets>>();
// Label::clone copies a terminal or one Rc only: no payload clone or type
// cursor. Both its borrowed receiver and returned/receiving owned Label count.
pub(super) const FIRST: usize = 2 * size_of::<Groups>()
    + 2 * size_of::<Label>() + 2 * size_of::<&Label>()
    + 2 * size_of::<paid_many::Targets>()
    + size_of::<Option<u64>>() + 2 * size_of::<u64>()
    + BYTE_ABI;
pub(super) const PROMOTION: usize = 6 * size_of::<Groups>()
    + 2 * size_of::<&mut Groups>()
    + 2 * size_of::<BTreeMap<Label, paid_many::Targets>>()
    + 4 * size_of::<&mut BTreeMap<Label, paid_many::Targets>>()
    + 4 * size_of::<(Label, paid_many::Targets)>()
    + 4 * size_of::<Option<paid_many::Targets>>()
    + 2 * size_of::<&Label>() + 2 * size_of::<Label>()
    + 2 * size_of::<paid_many::Targets>() + size_of::<Option<u64>>()
    + 2 * size_of::<u64>() + BYTE_ABI;
pub(super) const LATER: usize = 2 * size_of::<Label>()
    + 2 * size_of::<&Label>() + 2 * size_of::<paid_many::Targets>()
    + size_of::<Option<paid_many::Targets>>() + size_of::<Option<u64>>()
    + 2 * size_of::<u64>() + BYTE_ABI;
pub(super) const TRANSPORT: usize = 2 * size_of::<Groups>()
    + size_of::<Label>() + size_of::<paid_many::Targets>()
    + size_of::<BTreeMap<Label, paid_many::Targets>>()
    + 2 * size_of::<&Groups>() + 2 * size_of::<usize>();
pub(super) const ITERATOR: usize =
    size_of::<std::collections::btree_map::IntoIter<Label, paid_many::Targets>>()
    + size_of::<Option<(Label, paid_many::Targets)>>()
    + size_of::<(Label, paid_many::Targets)>();

macro_rules! selected_labels_successors {
    ($function:ident, $budget:ident, $automaton:expr, $closed:ident, $accepted:ident, $unit:expr) => {{
        // No function call or borrowed-key lifetime change. Memo preflight and
        // ClosureReceiving::valid have already authenticated the exact function,
        // budget and live ledger before any closed view in the enclosing method.
        $budget.reserve_bytes($function, selected_labels::ENTRY,
            "factored selected inline label receiving controls")?;
        let mut labels = selected_labels::Groups::Empty;
        macro_rules! add {
            ($targets:ident, $child:ident, $weight:ident) => {{
                        match $targets {
                            paid_many::Targets::One {
                                child: first,
                                weight: total,
                            } if *first == *$child => {
                                // Same repeated-child checked_add before any later
                                // target acquisition or error, as in the old map.
                                let Some(sum) = total.checked_add(*$weight) else {
                                    return Ok(None);
                                };
                                *total = sum;
                            }
                            paid_many::Targets::One {
                                child: first,
                                weight: total,
                            } => {
                                if  $budget.pays_allocations() {
                                     $budget.reserve_bytes( $function, paid_many::PROMOTE_RECEIVING,
                                        "factored composed promotion receivers")?;
                                }
                                #[cfg(test)]
                                if  $budget.pays_allocations() { paid_many::note_branch($unit); }
                                *$targets = paid_many::promote( $function,  $budget, *first, *total, *$child, *$weight)?;
                            }
                            paid_many::Targets::PaidMany(_) => {
                                 $budget.reserve_bytes( $function, paid_many::ADD_RECEIVING,
                                    "factored composed flat add receivers")?;
                                let paid_many::Targets::PaidMany(flat) = $targets else { unreachable!("paid flat arm") };
                                if !paid_many::add( $function,  $budget, flat, *$child, *$weight)? { return Ok(None); }
                            }
                            paid_many::Targets::Many(targets) => {
                                if !targets.contains_key($child) {
                                     $budget.reserve_btree_node::<usize, u64>(
                                         $function,
                                        "factored successor target node",
                                    )?;
                                }
                                let slot = targets.entry(*$child).or_insert(0);
                                let Some(sum) = slot.checked_add(*$weight) else {
                                    return Ok(None);
                                };
                                *slot = sum;
                            }
                        }
            }};
        }
        for (node, weight) in $closed.iter() {
            let moves = ($automaton).nodes[*node].moves();
            $budget.charge($function, moves.len().saturating_add(1),
                "factored skeleton subset construction")?;
            for (label, child) in moves {
                $budget.reserve_bytes($function, selected_labels::MOVEMENT,
                    "factored selected inline movement receivers")?;
                match &mut labels {
                    selected_labels::Groups::Empty => {
                        $budget.reserve_bytes($function, selected_labels::FIRST,
                            "factored selected first label receivers")?;
                        $budget.reserve_bytes($function, 0,
                            "factored singleton target authentication")?;
                        let weight = 0_u64.checked_add(*weight)
                            .expect("zero plus u64 cannot overflow");
                        labels = selected_labels::Groups::One {
                            label: label.clone(),
                            targets: paid_many::Targets::One { child: *child, weight },
                        };
                    }
                    selected_labels::Groups::One { label: first, targets }
                        if &*first == label => { add!(targets, child, weight); }
                    selected_labels::Groups::One { .. } => {
                        // Complete fallible acquisition precedes moving One.
                        $budget.reserve_bytes($function, selected_labels::PROMOTION,
                            "factored selected label promotion receivers")?;
                        $budget.reserve_btree_node::<Label, paid_many::Targets>($function,
                            "factored selected first label node")?;
                        $budget.reserve_btree_node::<Label, paid_many::Targets>($function,
                            "factored selected second label node")?;
                        $budget.reserve_bytes($function, 0,
                            "factored singleton target authentication")?;
                        let weight = 0_u64.checked_add(*weight)
                            .expect("zero plus u64 cannot overflow");
                        let mut many = BTreeMap::new();
                        let selected_labels::Groups::One { label: first, targets } =
                            std::mem::replace(&mut labels, selected_labels::Groups::Empty)
                        else { unreachable!("occupied inline label") };
                        many.insert(first, targets);
                        many.insert(label.clone(), paid_many::Targets::One { child: *child, weight });
                        labels = selected_labels::Groups::Many(many);
                    }
                    selected_labels::Groups::Many(many) => {
                        if !many.contains_key(label) {
                            $budget.reserve_btree_node::<Label, paid_many::Targets>($function,
                                "factored successor label node")?;
                            $budget.reserve_bytes($function, selected_labels::LATER,
                                "factored selected later label receivers")?;
                            $budget.reserve_bytes($function, 0,
                                "factored singleton target authentication")?;
                            let weight = 0_u64.checked_add(*weight)
                                .expect("zero plus u64 cannot overflow");
                            many.insert(label.clone(), paid_many::Targets::One { child: *child, weight });
                        } else {
                            let targets = many.get_mut(label).expect("existing label");
                            add!(targets, child, weight);
                        }
                    }
                }
            }
        }
        $budget.reserve_bytes($function, selected_labels::TRANSPORT,
            "factored selected consuming label receivers")?;
        let count = match &labels {
            selected_labels::Groups::Empty => 0,
            selected_labels::Groups::One { .. } => 1,
            selected_labels::Groups::Many(many) => many.len(),
        };
        $budget.reserve_array::<(Label, SharedWeighted)>($function, count,
            "factored successor vector")?;
        let mut result = Vec::with_capacity(count);
        macro_rules! publish {
            ($label:ident, $targets:ident) => {{
                    if  $budget.pays_allocations() {
                         $budget.reserve_bytes(
                             $function,
                            2 * std::mem::size_of::<SharedWeighted>()
                                + std::mem::size_of::<(usize, u64)>(),
                            "factored inline successor finish receiving",
                        )?;
                    }
                     $budget.reserve_bytes(
                         $function,
                        0,
                        "factored successor target finish authentication",
                    )?;
                    let state = match $targets {
                        paid_many::Targets::One { child, weight } => {
                            if  $budget.pays_allocations() {
                                 $budget.reserve_bytes(
                                     $function,
                                    2 * std::mem::size_of::<SmallWeighted>()
                                        + std::mem::size_of::<[(usize, u64); 1]>()
                                        + std::mem::size_of::<Result<(), Diagnostic>>(),
                                    "factored inline weighted enum and array receiving",
                                )?;
                            }
                             $budget.reserve_bytes(
                                 $function,
                                physical_clone::rc_backing_bytes::<SmallWeighted>(),
                                "factored weighted shared header",
                            )?;
                            let singleton = SmallWeighted::One([(child, weight)]);
                            Rc::new(singleton)
                        }
                        paid_many::Targets::PaidMany(_) => match  $budget {
                            ReplayBudget { payer: AllocationPayer::LegacyUnobserved, .. } => {
                                 $budget.reserve_bytes( $function, paid_many::FINISH_RECEIVING,
                                    "factored composed flat finish receivers")?;
                                let paid_many::Targets::PaidMany(flat) = $targets else { unreachable!("paid flat finish") };
                                own_weighted( $function,  $budget, flat)?
                            }
                            _ => paid_many::finish!( $function,  $budget, $targets),
                        },
                        paid_many::Targets::Many(targets) => {
                            // The remaining fallible weighted producer still has
                            // its real map argument and Result receiving carrier.
                            if  $budget.pays_allocations() {
                                 $budget.reserve_bytes(
                                     $function,
                                    std::mem::size_of::<BTreeMap<usize, u64>>()
                                        + std::mem::size_of::<Vec<(usize, u64)>>()
                                        + std::mem::size_of::<Result<SharedWeighted, Diagnostic>>(),
                                    "factored many weighted producer receiving",
                                )?;
                            }
                            weighted_successor( $function,  $budget, targets)?
                        }
                    };
                    result.push(($label, state));
            }};
        }
        match labels {
            selected_labels::Groups::Empty => {}
            selected_labels::Groups::One { label, targets } => { publish!(label, targets); }
            selected_labels::Groups::Many(many) => {
                $budget.reserve_bytes($function, selected_labels::ITERATOR,
                    "factored successor target iterator receiving")?;
                for (label, targets) in many { publish!(label, targets); }
            }
        }
        Ok(Some(($accepted, result)))
    }};
}

#[cfg(test)]
mod tests;
