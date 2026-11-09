//! Lexical pure queries add no helper receiver or forwarding result ABI.
macro_rules! exact_join {
    ($budget:ident,$function:ident,$lexical:ident,$block:ident) => {{
        $budget.reserve_array::<(&lexical_cert::LexicalAuth, bool)>(
            $function,
            1,
            "selected path exact join query receiving",
        )?;
        $budget.charge($function, 2, "selected path exact join query receiving")?;
        match $lexical {
            Some(auth) => auth.exact_joins.contains(&$block),
            None => false,
        }
    }};
}
pub(super) use exact_join;
macro_rules! lexical_lookup {
    ($budget:ident,$function:ident,$lexical:ident,$key:ident,$field:ident,$value:ty) => {{
        $budget.reserve_array::<(&lexical_cert::LexicalAuth,Option<&$value>)>(
            $function,1,"selected path lexical lookup result receiving")?;
        $budget.charge($function,2,"selected path lexical lookup result receiving")?;
        match $lexical {
            Some(auth)=>dynamic_get!($budget,$function,auth.$field,
                &BTreeMap<crate::cleanup_plan::ExitTargetId,$value>,$key,
                &crate::cleanup_plan::ExitTargetId,Option<&$value>,(&$value),4),
            None=>None,
        }
    }};
}
pub(super) use lexical_lookup;

// Lexical expansion only: iterator/result storage precedes formation, each
// reached next (including None) and predicate result precedes evaluation.
// No closure, helper activation, speculative scan or precredited old fee.
macro_rules! scan {
    ($budget:ident,$function:ident,$producer:expr,$iterator:ty,$item:ty,
        $pattern:pat,($($binding:ty),*),$fields:expr,$predicate:expr,$initial:expr,$decisive:expr) => {{
        receive!($budget,$function,($iterator,bool),2,
            "selected path scan iterator and result receiving",());
        let mut scan_result = $initial;
        let mut scan_iterator = $producer;
        while let Some($pattern) = dynamic_next!($budget,$function,scan_iterator,
            $iterator,$item,($($binding),*),$fields) {
            if receive!($budget,$function,bool,1,
                "selected path scan predicate receiving",$predicate) == $decisive {
                scan_result = $decisive;
                break;
            }
        }
        scan_result
    }};
}
pub(super) use scan;
macro_rules! find {
    ($budget:ident,$function:ident,$producer:expr,$iterator:ty,$item:ty,
        $pattern:ident,($($binding:ty),*),$fields:expr,$predicate:expr) => {{
        receive!($budget,$function,($iterator,Option<$item>),2,
            "selected path find iterator and result receiving",());
        let mut find_result = None;
        let mut find_iterator = $producer;
        while let Some($pattern) = dynamic_next!($budget,$function,find_iterator,
            $iterator,$item,($($binding),*),$fields) {
            if receive!($budget,$function,bool,1,
                "selected path find predicate receiving",$predicate) {
                find_result = Some($pattern);
                break;
            }
        }
        find_result
    }};
}
pub(super) use find;
macro_rules! flag_any {
    ($budget:ident,$function:ident,$flags:expr,$flag:ident,$predicate:expr) => {
        scan!(
            $budget,
            $function,
            $flags.iter(),
            std::slice::Iter<'_, LivenessFlagId>,
            &LivenessFlagId,
            $flag,
            (&LivenessFlagId),
            3,
            $predicate,
            false,
            true
        )
    };
}
pub(super) use flag_any;
macro_rules! flag_all {
    ($budget:ident,$function:ident,$flags:expr,$flag:ident,$predicate:expr) => {
        scan!(
            $budget,
            $function,
            $flags.iter(),
            std::slice::Iter<'_, LivenessFlagId>,
            &LivenessFlagId,
            $flag,
            (&LivenessFlagId),
            3,
            $predicate,
            true,
            false
        )
    };
}
pub(super) use flag_all;
macro_rules! variant_any {
    ($budget:ident,$function:ident,$variants:expr,$variant:ident,$predicate:expr) => {
        scan!(
            $budget,
            $function,
            $variants.iter(),
            std::slice::Iter<'_, ReplayConditionalVariant>,
            &ReplayConditionalVariant,
            $variant,
            (&ReplayConditionalVariant),
            3,
            $predicate,
            false,
            true
        )
    };
}
pub(super) use variant_any;
// Stable compaction for the Copy flag payload has the same retained order and
// capacity as Vec::retain. Fresh get/result, predicate and write roles are
// admitted at their actual iteration; there is no allocating collection.
macro_rules! retain_flags {
    ($budget:ident,$function:ident,$flags:expr,$flag:ident,$predicate:expr) => {{
        receive!(
            $budget,
            $function,
            (usize, usize, usize),
            3,
            "selected path flag compaction indices receiving",
            ()
        );
        let mut read = 0usize;
        let mut write = 0usize;
        let length = $flags.len();
        while read < length {
            receive!(
                $budget,
                $function,
                (LivenessFlagId, &LivenessFlagId, bool),
                3,
                "selected path flag compaction predicate receiving",
                ()
            );
            let value = *dynamic_get!(
                $budget,
                $function,
                $flags,
                &Vec<LivenessFlagId>,
                read,
                usize,
                Option<&LivenessFlagId>,
                (&LivenessFlagId),
                4
            )
            .expect("retained flag index is in bounds");
            let $flag = &value;
            if $predicate {
                *receive!(
                    $budget,
                    $function,
                    (
                        &mut Vec<LivenessFlagId>,
                        usize,
                        Option<&mut LivenessFlagId>,
                        &mut LivenessFlagId
                    ),
                    4,
                    "selected path flag compaction write receiving",
                    $flags.get_mut(write)
                )
                .expect("compacted flag index is in bounds") = value;
                write += 1;
            }
            read += 1;
        }
        $flags.truncate(write);
    }};
}
pub(super) use retain_flags;
macro_rules! variant_position {
    ($budget:ident,$function:ident,$variants:expr,$variant:ident,$predicate:expr) => {{
        receive!(
            $budget,
            $function,
            (
                std::slice::Iter<'_, ReplayConditionalVariant>,
                usize,
                Option<usize>
            ),
            3,
            "selected path variant position receiving",
            ()
        );
        let mut position = None;
        let mut index = 0usize;
        let mut variants = $variants.iter();
        while let Some($variant) = dynamic_next!(
            $budget,
            $function,
            variants,
            std::slice::Iter<'_, ReplayConditionalVariant>,
            &ReplayConditionalVariant,
            (&ReplayConditionalVariant),
            3
        ) {
            if receive!(
                $budget,
                $function,
                bool,
                1,
                "selected path variant position predicate receiving",
                $predicate
            ) {
                position = Some(index);
                break;
            }
            index += 1;
        }
        position
    }};
}
pub(super) use variant_position;
macro_rules! filtered_flag_next {
    ($budget:ident,$function:ident,$iterator:ident,$flag:ident,$predicate:expr) => {{
        receive!(
            $budget,
            $function,
            Option<&LivenessFlagId>,
            1,
            "selected path filtered flag result receiving",
            ()
        );
        let mut filtered = None;
        while let Some($flag) = dynamic_next!(
            $budget,
            $function,
            $iterator,
            std::slice::Iter<'_, LivenessFlagId>,
            &LivenessFlagId,
            (&LivenessFlagId),
            3
        ) {
            if receive!(
                $budget,
                $function,
                bool,
                1,
                "selected path flag filter predicate receiving",
                $predicate
            ) {
                filtered = Some($flag);
                break;
            }
        }
        filtered
    }};
}
pub(super) use filtered_flag_next;
macro_rules! equal_without_flag {
    ($budget:ident,$function:ident,$left:expr,$right:expr,$excluded:ident) => {{
        receive!(
            $budget,
            $function,
            (
                std::slice::Iter<'_, LivenessFlagId>,
                std::slice::Iter<'_, LivenessFlagId>,
                bool
            ),
            3,
            "selected path filtered equality iterators receiving",
            ()
        );
        let mut left = $left.iter();
        let mut right = $right.iter();
        let mut equal = true;
        loop {
            let left_flag =
                filtered_flag_next!($budget, $function, left, candidate, *candidate != $excluded);
            let right_flag = filtered_flag_next!(
                $budget,
                $function,
                right,
                candidate,
                *candidate != $excluded
            );
            if receive!(
                $budget,
                $function,
                bool,
                1,
                "selected path filtered equality comparison receiving",
                left_flag != right_flag
            ) {
                equal = false;
                break;
            }
            if left_flag.is_none() {
                break;
            }
        }
        equal
    }};
}
pub(super) use equal_without_flag;
macro_rules! mutable_get {
    ($budget:ident,$function:ident,$receiver:expr,$key:expr,$key_type:ty,$value:ty) => {
        receive!(
            $budget,
            $function,
            (
                &mut BTreeMap<$key_type, $value>,
                &$key_type,
                Option<&mut $value>,
                &mut $value
            ),
            4,
            "selected path mutable map lookup receiving",
            $receiver.get_mut($key)
        )
    };
}
pub(super) use mutable_get;
macro_rules! leaf_at {
    ($budget:ident,$function:ident,$leaves:ident,$flag:expr) => {
        receive!(
            $budget,
            $function,
            (
                &Leaves,
                &BTreeMap<LivenessFlagId, Leaf>,
                &LivenessFlagId,
                &Leaf
            ),
            4,
            "selected path leaf index receiving",
            &$leaves[$flag]
        )
    };
}
pub(super) use leaf_at;
macro_rules! map_index {
    ($budget:ident,$function:ident,$map:expr,$key:expr,$key_type:ty,$value:ty) => {
        receive!(
            $budget,
            $function,
            (&BTreeMap<$key_type, $value>, &$key_type, &$value),
            3,
            "selected path map index receiving",
            &$map[$key]
        )
    };
}
pub(super) use map_index;
macro_rules! index {
    ($budget:ident,$function:ident,$values:expr,$index:expr,$item:ty) => {
        receive!(
            $budget,
            $function,
            (&Vec<$item>, usize, &$item),
            3,
            "selected path index receiving",
            &$values[$index]
        )
    };
}
pub(super) use index;
macro_rules! index_mut {
    ($budget:ident,$function:ident,$values:expr,$index:expr,$item:ty) => {
        receive!(
            $budget,
            $function,
            (&mut Vec<$item>, usize, &mut $item),
            3,
            "selected path mutable index receiving",
            &mut $values[$index]
        )
    };
}
pub(super) use index_mut;
macro_rules! flag_at {
    ($budget:ident,$function:ident,$values:expr,$index:expr) => {
        receive!(
            $budget,
            $function,
            LivenessFlagId,
            1,
            "selected path indexed flag copy receiving",
            *index!($budget, $function, $values, $index, LivenessFlagId)
        )
    };
}
pub(super) use flag_at;
macro_rules! plan_at {
    ($budget:ident,$function:ident,$plan:ident,blocks,$index:expr) => {
        index!(
            $budget,
            $function,
            $plan.blocks,
            $index,
            crate::cleanup_plan::CleanupBlock
        )
    };
    ($budget:ident,$function:ident,$plan:ident,edges,$index:expr) => {
        index!(
            $budget,
            $function,
            $plan.edges,
            $index,
            crate::cleanup_plan::CleanupEdge
        )
    };
    ($budget:ident,$function:ident,$plan:ident,exits,$index:expr) => {
        index!($budget, $function, $plan.exits, $index, ExitTarget)
    };
}
pub(super) use plan_at;
macro_rules! incoming_at {
    ($budget:ident,$function:ident,$incoming:ident,$index:expr) => {
        index_mut!(
            $budget,
            $function,
            $incoming,
            $index,
            BTreeSet<Box<PathState>>
        )
    };
}
pub(super) use incoming_at;
macro_rules! counter_at {
    ($budget:ident,$function:ident,$values:ident,$index:expr) => {
        index_mut!($budget, $function, $values, $index, usize)
    };
}
pub(super) use counter_at;
macro_rules! selected_renewals {
    ($budget:ident,$function:ident,$program:ident) => {
        receive!(
            $budget,
            $function,
            (
                &dyn ReplayContext,
                &ResolvedFunction,
                Option<&crate::cleanup_plan::ReplayRenewalBindings<'_>>,
                &crate::cleanup_plan::ReplayRenewalBindings<'_>
            ),
            4,
            "selected path current renewal lease receiving",
            $program.primitive_renewals($function)
        )
    };
}
pub(super) use selected_renewals;
macro_rules! clone_value {
    ($budget:ident,$function:ident,$value:expr,$item:ty) => {
        receive!(
            $budget,
            $function,
            (&$item, $item),
            2,
            "selected path clone receiver and value receiving",
            $value.clone()
        )
    };
}
pub(super) use clone_value;
macro_rules! map_insert {
    ($budget:ident,$function:ident,$map:expr,$key:expr,$value:expr,$key_type:ty,$value_type:ty) => {
        receive!(
            $budget,
            $function,
            (
                &mut BTreeMap<$key_type, $value_type>,
                $key_type,
                $value_type,
                Option<$value_type>
            ),
            4,
            "selected path map insertion result receiving",
            $map.insert($key, $value)
        )
    };
}
pub(super) use map_insert;
macro_rules! group_at {
    ($budget:ident,$function:ident,$groups:ident,$key:ident) => {
        receive!(
            $budget,
            $function,
            (
                &mut BTreeMap<ControlKey, BTreeSet<Box<PathState>>>,
                ControlKey,
                std::collections::btree_map::Entry<'_, ControlKey, BTreeSet<Box<PathState>>>,
                &mut BTreeSet<Box<PathState>>
            ),
            4,
            "selected path control group entry receiving",
            $groups.entry($key).or_default()
        )
    };
}
pub(super) use group_at;
macro_rules! clone_state {
    ($budget:ident,$function:ident,$state:ident) => {
        clone_value!($budget, $function, $state, PathState)
    };
}
pub(super) use clone_state;
macro_rules! control_key {
    ($budget:ident,$function:ident,$state:ident) => {
        receive!(
            $budget,
            $function,
            ControlKey,
            1,
            "selected path fresh control key receiving",
            (
                clone_value!(
                    $budget,
                    $function,
                    $state.pending_failure,
                    Option<StatusSourceId>
                ),
                clone_value!(
                    $budget,
                    $function,
                    $state.selected_failure,
                    Option<StatusSourceId>
                ),
                clone_value!(
                    $budget,
                    $function,
                    $state.staged_copy_result,
                    Option<StagedCopyResultSource>
                ),
                $state.published,
                clone_value!($budget, $function, $state.returned, Option<ReturnedSource>)
            )
        )
    };
}
pub(super) use control_key;
macro_rules! place_flags {
    ($budget:ident,$function:ident,$place:ident,$storage:ident,$leaves:ident) => {
        path_child!(
            $budget,
            $function,
            PlaceCall,
            6,
            structural_control::selected::helpers::validate_place(
                $function, $place, $storage, $leaves, $budget
            )
        )
    };
}
pub(super) use place_flags;

macro_rules! owned_provenance {
    ($budget:ident,$function:ident,$expression:ident) => {
        receive!(
            $budget,
            $function,
            (&crate::hir::ResolvedExpr, bool),
            2,
            "selected renewal provenance query receiving",
            {
                receive!(
                    $budget,
                    $function,
                    (&DeclarationId, &Vec<crate::hir::ResolvedExpr>),
                    2,
                    "selected renewal provenance call operands receiving",
                    ()
                );
                match &$expression.kind {
                    crate::hir::ResolvedExprKind::Call { callee, args, .. } => {
                        match receive!(
                            $budget,
                            $function,
                            (&str, Option<crate::vec_ops::VecOp>, crate::vec_ops::VecOp),
                            3,
                            "selected renewal provenance operation lookup receiving",
                            crate::vec_ops::by_id(callee.as_str())
                        ) {
                            Some(op) => {
                                receive!(
                                    $budget,
                                    $function,
                                    (crate::vec_ops::VecOp, bool),
                                    2,
                                    "selected renewal provenance operation query receiving",
                                    op.reopens_same_owner()
                                ) && scan!(
                                    $budget,
                                    $function,
                                    args.iter().skip(1),
                                    std::iter::Skip<std::slice::Iter<'_, crate::hir::ResolvedExpr>>,
                                    &crate::hir::ResolvedExpr,
                                    arg,
                                    (&crate::hir::ResolvedExpr),
                                    3,
                                    arg.ownership == crate::hir::OwnershipMode::Own,
                                    false,
                                    true
                                )
                            }
                            None => false,
                        }
                    }
                    _ => false,
                }
            }
        )
    };
}
pub(super) use owned_provenance;
macro_rules! retained_units {
    ($budget:ident,$function:ident,$state:expr) => {{
        receive!(
            $budget,
            $function,
            (
                &PathState,
                std::collections::btree_map::Iter<
                    '_,
                    ExpressionId,
                    crate::cleanup_plan::renewal_history::RenewalHistory,
                >,
                usize,
                usize
            ),
            4,
            "selected renewal retained total receiving",
            ()
        );
        let held: &PathState = $state;
        let mut total = 0usize;
        let mut entries = held.renewals.iter();
        while let Some((at, history)) = dynamic_next!(
            $budget,
            $function,
            entries,
            std::collections::btree_map::Iter<
                '_,
                ExpressionId,
                crate::cleanup_plan::renewal_history::RenewalHistory,
            >,
            (
                &ExpressionId,
                &crate::cleanup_plan::renewal_history::RenewalHistory
            ),
            (
                &ExpressionId,
                &crate::cleanup_plan::renewal_history::RenewalHistory
            ),
            4
        ) {
            total = total
                .saturating_add(at.as_str().len())
                .saturating_add(receive!(
                    $budget,
                    $function,
                    (&crate::cleanup_plan::renewal_history::RenewalHistory, usize),
                    2,
                    "selected renewal retained scalar query receiving",
                    history.retained_units()
                ))
                .saturating_add(1);
        }
        total
    }};
}
pub(super) use retained_units;

// The retained query mutates the same budget as its final work debit. Finish
// the query before borrowing that receiver; acquire the new usize first.
// Lexical expansion adds no forwarding activation or Result transport.
macro_rules! charge_retained_units {
    ($budget:ident,$function:ident,$state:expr,$phase:expr) => {{
        receive!(
            $budget,
            $function,
            usize,
            1,
            "selected retained work local receiving",
            ()
        );
        let retained_work = retained_units!($budget, $function, $state);
        $budget.charge($function, retained_work, $phase)
    }};
}
pub(super) use charge_retained_units;

macro_rules! slice_index {
    ($budget:ident,$function:ident,$values:expr,$index:expr,$item:ty) => {
        receive!(
            $budget,
            $function,
            (&[$item], usize, &$item),
            3,
            "selected path slice index receiving",
            &$values[$index]
        )
    };
}
pub(super) use slice_index;
macro_rules! pair_at {
    ($budget:ident,$function:ident,$values:ident,$index:expr) => {
        receive!(
            $budget,
            $function,
            LivenessFlagId,
            1,
            "selected path indexed pair flag copy receiving",
            *slice_index!($budget, $function, $values, $index, LivenessFlagId)
        )
    };
}
pub(super) use pair_at;
