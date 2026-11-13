//! Replay narrowly authenticated same-owner renewal without changing canonical history.
use super::*;
use crate::cleanup_plan::{
    CLEANUP_PLAN_SCHEMA_V17, CLEANUP_PLAN_SCHEMA_V18, CLEANUP_PLAN_SCHEMA_V19,
};

pub(super) fn validate_binding(
    program: &dyn crate::cleanup::ReplayContext,
    function: &ResolvedFunction,
    at: &ExpressionId,
    place: &CleanupPlace,
    helpers: Option<&crate::loop_calls::byte_writer::ModuleIndex<'_>>,
    budget: &mut ReplayBudget,
) -> Result<(), Diagnostic> {
    if let Some(sites) = program.primitive_renewals(function) {
        return validate_binding_selected(function, at, place, sites);
    }
    use crate::cleanup_plan::renewal_profile::ordinary::Family;
    let found = ordinary_renewals::lookup(program, function, at, helpers, budget)?;
    let families = found.map_or(0, |(_, families)| families);
    let place_matches = match found {
        Some((binding, _)) => ordinary_renewals::whole(function, place, binding, budget)?,
        None => false,
    };
    if !matches!(
        function.cleanup_plan.schema,
        CLEANUP_PLAN_SCHEMA_V12
            | CLEANUP_PLAN_SCHEMA_V13
            | CLEANUP_PLAN_SCHEMA_V14
            | CLEANUP_PLAN_SCHEMA_V15
            | CLEANUP_PLAN_SCHEMA_V16
            | CLEANUP_PLAN_SCHEMA_V17
            | CLEANUP_PLAN_SCHEMA_V18
            | CLEANUP_PLAN_SCHEMA_V19
    ) || (ordinary_renewals::has(families, Family::Vec)
        && !matches!(
            function.cleanup_plan.schema,
            CLEANUP_PLAN_SCHEMA_V15
                | CLEANUP_PLAN_SCHEMA_V16
                | CLEANUP_PLAN_SCHEMA_V17
                | CLEANUP_PLAN_SCHEMA_V18
                | CLEANUP_PLAN_SCHEMA_V19
        ))
        || (ordinary_renewals::has(families, Family::String)
            && !matches!(
                function.cleanup_plan.schema,
                CLEANUP_PLAN_SCHEMA_V16
                    | CLEANUP_PLAN_SCHEMA_V17
                    | CLEANUP_PLAN_SCHEMA_V18
                    | CLEANUP_PLAN_SCHEMA_V19
            ))
        || (ordinary_renewals::has(families, Family::ByteSet)
            && !matches!(
                function.cleanup_plan.schema,
                CLEANUP_PLAN_SCHEMA_V17 | CLEANUP_PLAN_SCHEMA_V18 | CLEANUP_PLAN_SCHEMA_V19
            ))
        || (ordinary_renewals::has(families, Family::Helper)
            && function.cleanup_plan.schema != CLEANUP_PLAN_SCHEMA_V19)
        || !place_matches
    {
        return Err(replay_error(
            function,
            "renewal is outside an exact authenticated same-owner assignment",
        ));
    }
    Ok(())
}
pub(super) fn validate_binding_selected(
    function: &ResolvedFunction,
    at: &ExpressionId,
    place: &CleanupPlace,
    sites: &crate::cleanup_plan::ReplayRenewalBindings<'_>,
) -> Result<(), Diagnostic> {
    if !matches!(
        function.cleanup_plan.schema,
        CLEANUP_PLAN_SCHEMA_V18 | CLEANUP_PLAN_SCHEMA_V19
    ) || sites.is_helper(at) && function.cleanup_plan.schema != CLEANUP_PLAN_SCHEMA_V19
        || !sites.get(at).is_some_and(|binding| {
            matches!(&place.storage, StorageId::Value(id) if id == &binding.id)
                && place.projections.is_empty()
        })
    {
        return Err(replay_error(
            function,
            "renewal is outside an exact authenticated same-owner assignment",
        ));
    }
    Ok(())
}

pub(super) fn reject_unmarked_finish(
    program: &dyn crate::cleanup::ReplayContext,
    function: &ResolvedFunction,
    at: &ExpressionId,
    destination: &CleanupPlace,
    helpers: Option<&crate::loop_calls::byte_writer::ModuleIndex<'_>>,
    budget: &mut ReplayBudget,
) -> Result<(), Diagnostic> {
    let found = ordinary_renewals::lookup(program, function, at, helpers, budget)?;
    let matches = match found {
        Some((binding, _)) => ordinary_renewals::whole(function, destination, binding, budget)?,
        None => false,
    };
    if matches {
        return Err(replay_error(
            function,
            "authenticated renewal uses an ordinary transfer",
        ));
    }
    Ok(())
}
#[allow(clippy::too_many_arguments)] // Keep the independently held phase index explicit.
pub(super) fn reserve(
    program: &dyn crate::cleanup::ReplayContext,
    function: &ResolvedFunction,
    at: &ExpressionId,
    binding: &CleanupPlace,
    state: &mut PathState,
    storage: &BTreeSet<StorageId>,
    leaves: &Leaves,
    helpers: Option<&crate::loop_calls::byte_writer::ModuleIndex<'_>>,
    budget: &mut ReplayBudget,
) -> Result<(), Diagnostic> {
    validate_binding(program, function, at, binding, helpers, budget)?;
    let flags = path_join::storage::validate_place(function, binding, storage, leaves, budget)?;
    if flags.len() != 1
        || !state.live_order.contains(&flags[0])
        || state.renewals.contains_key(at)
        || (!matches!(
            function.cleanup_plan.schema,
            CLEANUP_PLAN_SCHEMA_V16
                | CLEANUP_PLAN_SCHEMA_V17
                | CLEANUP_PLAN_SCHEMA_V18
                | CLEANUP_PLAN_SCHEMA_V19
        ) && !state.renewals.is_empty())
    {
        return Err(replay_error(
            function,
            "renewal reservation requires one live unreserved owner leaf",
        ));
    }
    let protected = find_resolved_expression(function, at)
        .is_some_and(super::super::renewal_profile::owned_rhs_provenance)
        .then_some(flags[0]);
    budget.reserve_array::<LivenessFlagId>(
        function,
        state.live_order.len(),
        "renewal original history",
    )?;
    if protected.is_some() {
        budget.reserve_array::<(LivenessFlagId, LivenessFlagId)>(
            function,
            state.live_order.len(),
            "renewal staged epochs",
        )?;
    }
    budget.reserve_btree_node::<ExpressionId, super::super::renewal_history::RenewalHistory>(
        function,
        "renewal reservation node",
    )?;
    state.renewals.insert(
        at.clone(),
        super::super::renewal_history::RenewalHistory::new(state.live_order.clone(), protected),
    );
    Ok(())
}
#[allow(clippy::too_many_arguments)] // Keep the independently held phase index explicit.
pub(super) fn renew(
    program: &dyn crate::cleanup::ReplayContext,
    function: &ResolvedFunction,
    at: &ExpressionId,
    (source, destination): (&CleanupPlace, &CleanupPlace),
    state: &mut PathState,
    storage: &BTreeSet<StorageId>,
    leaves: &Leaves,
    helpers: Option<&crate::loop_calls::byte_writer::ModuleIndex<'_>>,
    budget: &mut ReplayBudget,
) -> Result<(), Diagnostic> {
    validate_binding(program, function, at, destination, helpers, budget)?;
    let history = state
        .renewals
        .remove(at)
        .ok_or_else(|| replay_error(function, "renewal has no reservation"))?;
    let flags = path_join::storage::validate_place(function, destination, storage, leaves, budget)?;
    if flags.len() != 1 {
        return Err(replay_error(
            function,
            "renewal destination is not one owner leaf",
        ));
    }
    if !budget.selected() && crate::string_ops::replacement::binding(function, at).is_some() {
        let source_flags =
            path_join::storage::validate_place(function, source, storage, leaves, budget)?;
        if source_flags == flags {
            return Err(replay_error(
                function,
                "String replacement aliases its new owner",
            ));
        }
        state.live_order.retain(|flag| !flags.contains(flag));
    }
    replay_transfer(
        function,
        state,
        source,
        destination,
        storage,
        leaves,
        budget,
    )?;
    let flag = flags[0];
    if !budget.selected() && crate::string_ops::replacement::binding(function, at).is_some() {
        return string_replacement::finish(function, flag, &history, state);
    }
    if !history.contains(&flag)
        || history
            .iter()
            .filter(|candidate| **candidate != flag)
            .ne(state
                .live_order
                .iter()
                .filter(|candidate| **candidate != flag))
    {
        return Err(replay_error(
            function,
            "renewal changed unrelated cleanup initialization history",
        ));
    }
    state.live_order = history.history;
    Ok(())
}
pub(super) fn prepend_reservation(
    function: &ResolvedFunction,
    expression: &ResolvedExpr,
    paths: &mut [ExprSkeletonPath],
    work: &mut SkeletonWork<'_, '_>,
) -> Result<(), Diagnostic> {
    let binding = work
        .renewal_binding(&expression.id)?
        .ok_or_else(|| replay_error(function, "renewal HIR binding disappeared"))?;
    for path in paths {
        let at = work.clone_owned(&expression.id, "renewal expression clone")?;
        let binding = CleanupPlace::whole(StorageId::Value(
            work.clone_owned(&binding.id, "renewal binding clone")?,
        ));
        work.charge(
            path.observations.len().saturating_add(1),
            "renewal prefix insertion",
        )?;
        note_skeleton_materialization();
        work.insert_front_observation(
            &mut path.observations,
            SkeletonObservation::ReserveRenewal { at, binding },
            "renewal prefix storage",
        )?;
    }
    Ok(())
}

pub(super) fn validate_join_compatibility<'a>(
    function: &ResolvedFunction,
    existing: impl Iterator<Item = &'a PathState> + Clone,
    incoming: &PathState,
    block: BlockId,
    budget: &mut ReplayBudget,
) -> Result<(), Diagnostic> {
    if existing.clone().any(|state| {
        state.renewals != incoming.renewals
            || state.pending_failure != incoming.pending_failure
            || state.selected_failure != incoming.selected_failure
            || state.published != incoming.published
            || state.returned != incoming.returned
    }) {
        return Err(replay_error(
            function,
            format_args!(
                "cleanup join at block {} has incompatible control states",
                block.0
            ),
        ));
    }

    let count = existing.clone().count().saturating_add(1);
    budget.reserve_array::<&Vec<LivenessFlagId>>(function, count, "join history references")?;
    let mut histories = Vec::with_capacity(count);
    histories.extend(existing.map(|state| &state.live_order));
    histories.push(&incoming.live_order);
    let mut flags = BTreeSet::new();
    for flag in histories.iter().flat_map(|history| history.iter().copied()) {
        path_join::storage::insert_set(function, &mut flags, flag, budget, "join union flags")?;
    }
    let mut successors = BTreeMap::new();
    let mut indegree = BTreeMap::new();
    for flag in &flags {
        path_join::storage::insert_map(
            function,
            &mut successors,
            *flag,
            BTreeSet::new(),
            budget,
            "join successor roots",
        )?;
        path_join::storage::insert_map(
            function,
            &mut indegree,
            *flag,
            0usize,
            budget,
            "join indegrees",
        )?;
    }
    for history in histories {
        for pair in history.windows(2) {
            if path_join::storage::insert_set(
                function,
                successors
                    .get_mut(&pair[0])
                    .expect("replayed live flag is indexed"),
                pair[1],
                budget,
                "join history edges",
            )? {
                indegree.entry(pair[1]).and_modify(|degree| *degree += 1);
            }
        }
    }
    let mut ready = BTreeSet::new();
    for (flag, degree) in &indegree {
        if *degree == 0 {
            path_join::storage::insert_set(
                function,
                &mut ready,
                *flag,
                budget,
                "join ready flags",
            )?;
        }
    }
    let mut visited = 0_usize;
    while let Some(flag) = ready.pop_first() {
        visited += 1;
        for successor in &successors[&flag] {
            let degree = indegree
                .get_mut(successor)
                .expect("replayed successor flag is indexed");
            *degree -= 1;
            if *degree == 0 {
                path_join::storage::insert_set(
                    function,
                    &mut ready,
                    *successor,
                    budget,
                    "join ready flags",
                )?;
            }
        }
    }
    if visited != flags.len() {
        return Err(replay_error(
            function,
            format_args!(
                "cleanup join at block {} has conflicting initialization histories",
                block.0
            ),
        ));
    }
    Ok(())
}

#[allow(clippy::items_after_test_module)]
pub(super) fn retained_units(state: &PathState) -> usize {
    state.renewals.iter().fold(0usize, |total, (at, flags)| {
        total
            .saturating_add(at.as_str().len())
            .saturating_add(flags.retained_units())
            .saturating_add(1)
    })
}
#[allow(clippy::items_after_test_module)]
pub(super) fn charge_transition(
    function: &ResolvedFunction,
    transition: &CleanupTransition,
    state: &PathState,
    budget: &mut ReplayBudget,
) -> Result<(), Diagnostic> {
    let units = match transition {
        CleanupTransition::ReserveRenewal { at, .. } => {
            let owned_rhs = find_resolved_expression(function, at)
                .is_some_and(super::super::renewal_profile::owned_rhs_provenance);
            state
                .live_order
                .len()
                .saturating_mul(if owned_rhs { 3 } else { 1 })
                .saturating_add(usize::from(owned_rhs))
                .saturating_add(at.as_str().len())
                .saturating_add(1)
        }
        CleanupTransition::Renew { .. } => {
            retained_units(state).saturating_add(state.live_order.len())
        }
        CleanupTransition::Transfer { .. } | CleanupTransition::CallCommit { .. } => {
            state.renewals.values().fold(0usize, |units, history| {
                units.saturating_add(history.provenance_units())
            })
        }
        _ => 0,
    };
    budget.charge(function, units, "renewal history materialization")
}

#[cfg(test)]
mod tests {
    use super::*;
    const SOURCE: &str = r#"
module test.iterator_renewal;
@id("app.main") fn main()->i64 {
    let input = vec_push<i64>(vec_push<i64>(vec_with_capacity<i64>(2usize), -1), 2);
    let mut output = vec_with_capacity<i64>(2usize);
    for own item in vec_into_iter<i64>(input) {
        if item > 0 { output = vec_push<i64>(output, item); 0 } else { 0 }
    }
    if vec_len<i64>(output) == 1usize { 1 } else { 0 }
}
"#;
    #[test]
    fn iterator_conditional_renewal_requires_exact_reservation_and_schema() {
        let source = crate::check(SOURCE, std::path::Path::new("renewal.spx"))
            .expect("renewal source checks");
        let program = crate::hir::resolve(&source).expect("conditional renewal resolves");
        let function = program
            .functions()
            .iter()
            .find(|function| function.id.as_str() == "app.main")
            .expect("main");
        assert_eq!(function.cleanup_plan.schema, CLEANUP_PLAN_SCHEMA_V12);
        assert!(function
            .cleanup_plan
            .blocks
            .iter()
            .flat_map(|block| &block.transitions)
            .any(|transition| matches!(transition, CleanupTransition::ReserveRenewal { .. })));
        assert!(function
            .cleanup_plan
            .blocks
            .iter()
            .flat_map(|block| &block.transitions)
            .any(|transition| matches!(transition, CleanupTransition::Renew { .. })));
        validate_structure(&program, function).expect("independent renewal replay");
        let mut missing = function.clone();
        for block in &mut missing.cleanup_plan.blocks {
            block.transitions.retain(|transition| {
                !matches!(transition, CleanupTransition::ReserveRenewal { .. })
            });
        }
        assert!(validate_structure(&program, &missing).is_err());
        let mut unmarked = function.clone();
        for transition in unmarked
            .cleanup_plan
            .blocks
            .iter_mut()
            .flat_map(|block| &mut block.transitions)
        {
            if let CleanupTransition::Renew {
                at,
                source,
                destination,
            } = transition
            {
                *transition = CleanupTransition::Transfer {
                    at: at.clone(),
                    source: source.clone(),
                    destination: destination.clone(),
                };
            }
        }
        assert!(validate_structure(&program, &unmarked).is_err());
        let mut downgraded = function.clone();
        downgraded.cleanup_plan.schema = CLEANUP_PLAN_SCHEMA_V11;
        assert!(validate_structure(&program, &downgraded).is_err());
    }
    #[test]
    fn record_renewal_replay_rejects_forged_binding_projection_and_transfer() {
        let source = format!(
            "{}\n{}",
            include_str!("../../../std/io/src/io.spx"),
            r#"
@id("app.main")
fn main() -> i64
{
    let mut reader = reader_from_bytes(bytes_zeroed(2usize));
    while reader_remaining(reader) > 0usize {
        reader = reader_advance(reader, 1usize);
        reader_remaining(reader) > 0usize
    }
    let retained = reader_finish(reader);
    if byte_len(bytes_as_slice(retained)) == 2usize { 0 } else { 1 }
}
"#,
        );
        let source = crate::check(&source, std::path::Path::new("record-renewal.spx"))
            .expect("record renewal source checks");
        let program = crate::hir::resolve(&source).expect("record renewal resolves");
        let function = program
            .functions()
            .iter()
            .find(|function| function.id.as_str() == "app.main")
            .expect("main");
        assert_eq!(function.cleanup_plan.schema, CLEANUP_PLAN_SCHEMA_V12);
        validate_structure(&program, function).expect("record renewal independently replays");

        let mut forged = function.clone();
        let binding = forged
            .cleanup_plan
            .blocks
            .iter_mut()
            .flat_map(|block| &mut block.transitions)
            .find_map(|transition| match transition {
                CleanupTransition::ReserveRenewal { binding, .. } => Some(binding),
                _ => None,
            })
            .expect("reservation");
        binding
            .projections
            .push(DeclarationId::new("std.io.reader.forged"));
        assert!(validate_structure(&program, &forged).is_err());

        let mut duplicate_reservation = function.clone();
        let block = duplicate_reservation
            .cleanup_plan
            .blocks
            .iter_mut()
            .find(|block| {
                block.transitions.iter().any(|transition| {
                    matches!(transition, CleanupTransition::ReserveRenewal { .. })
                })
            })
            .expect("reservation block");
        let index = block
            .transitions
            .iter()
            .position(|transition| matches!(transition, CleanupTransition::ReserveRenewal { .. }))
            .expect("reservation");
        block
            .transitions
            .insert(index, block.transitions[index].clone());
        assert!(validate_structure(&program, &duplicate_reservation).is_err());

        let mut duplicate_renewal = function.clone();
        let block = duplicate_renewal
            .cleanup_plan
            .blocks
            .iter_mut()
            .find(|block| {
                block
                    .transitions
                    .iter()
                    .any(|transition| matches!(transition, CleanupTransition::Renew { .. }))
            })
            .expect("renewal block");
        let index = block
            .transitions
            .iter()
            .position(|transition| matches!(transition, CleanupTransition::Renew { .. }))
            .expect("renewal");
        block
            .transitions
            .insert(index, block.transitions[index].clone());
        assert!(validate_structure(&program, &duplicate_renewal).is_err());

        let mut forged_destination = function.clone();
        let destination = forged_destination
            .cleanup_plan
            .blocks
            .iter_mut()
            .flat_map(|block| &mut block.transitions)
            .find_map(|transition| match transition {
                CleanupTransition::Renew { destination, .. } => Some(destination),
                _ => None,
            })
            .expect("renewal destination");
        destination
            .projections
            .push(DeclarationId::new("std.io.writer.position"));
        assert!(validate_structure(&program, &forged_destination).is_err());

        let mut unmarked = function.clone();
        let transition = unmarked
            .cleanup_plan
            .blocks
            .iter_mut()
            .flat_map(|block| &mut block.transitions)
            .find(|transition| matches!(transition, CleanupTransition::Renew { .. }))
            .expect("renewal");
        let CleanupTransition::Renew {
            at,
            source,
            destination,
        } = transition
        else {
            unreachable!()
        };
        *transition = CleanupTransition::Transfer {
            at: at.clone(),
            source: source.clone(),
            destination: destination.clone(),
        };
        assert!(validate_structure(&program, &unmarked).is_err());
    }

    #[test]
    fn record_renewal_named_views_reject_forged_operands_and_owner_binding() {
        let source = crate::check(
            include_str!("../../../tests/language/while_loops/record_borrow_renewal.spx"),
            "record-renewal-views.spx",
        )
        .unwrap();
        let program = crate::hir::resolve(&source).unwrap();
        let function = program
            .functions()
            .iter()
            .find(|f| f.id.as_str() == "matcher.run")
            .unwrap();
        validate_structure(&program, function).unwrap();
        for mode in 0..4 {
            let mut forged = function.clone();
            let ResolvedExprKind::Block { statements, .. } = &mut forged.body.kind else {
                panic!("run block")
            };
            let other = statements
                .iter()
                .find_map(|statement| match statement {
                    ResolvedStatement::Let { binding, .. } if binding.name == "source" => {
                        Some(binding.id.clone())
                    }
                    _ => None,
                })
                .unwrap();
            let body = statements
                .iter_mut()
                .find_map(|statement| match statement {
                    ResolvedStatement::While { body, .. } => Some(body),
                    _ => None,
                })
                .unwrap();
            let ResolvedExprKind::Block { statements, .. } = &mut body.kind else {
                panic!("loop block")
            };
            let call = statements
                .iter_mut()
                .find_map(|statement| match statement {
                    ResolvedStatement::Assign { value, .. } => Some(value),
                    _ => None,
                })
                .unwrap();
            let ResolvedExprKind::Call { args, .. } = &mut call.kind else {
                panic!("renewal call")
            };
            match mode {
                0 => {
                    let ResolvedExprKind::Place(place) = &mut args[1].kind else {
                        panic!("view place")
                    };
                    place
                        .projections
                        .push(crate::hir::PlaceProjection::Field(DeclarationId::new(
                            "matcher.storage",
                        )));
                }
                1 => {
                    args[1].kind = ResolvedExprKind::Block {
                        statements: Vec::new(),
                        tail: Box::new(args[1].clone()),
                    }
                }
                2 => args[1].ownership = OwnershipMode::Own,
                3 => {
                    let ResolvedExprKind::Place(place) = &mut args[0].kind else {
                        panic!("owner place")
                    };
                    place.root = other;
                }
                _ => unreachable!(),
            }
            assert!(
                validate_structure(&program, &forged).is_err(),
                "forged renewal mode {mode}"
            );
        }
    }

    #[test]
    fn ordinary_vec_renewal_rejects_missing_forged_and_downgraded_proofs() {
        // The independently authenticated Copy-record extension must retain
        // the same reservation and reject cached field-type drift.
        let copy = crate::hir::resolve(
            &crate::check(
                r#"module copy_renewal;
@id("row") record Row { @id("row.n") n:i64, }
@id("app.main") fn main()->i64 {
 let mut values=vec_with_capacity<Row>(2usize); let mut i=0;
 while i<2 { values=vec_push<Row>(values,Row{n:i}); i=i+1; 0 }
 i64_from_usize(vec_len<Row>(values))
}
"#,
                "copy-renewal.spx",
            )
            .unwrap(),
        )
        .unwrap();
        let function = &copy.functions[0];
        assert!(crate::hir::vec_loop_renewal::requires_in(&copy, function));
        assert_eq!(function.cleanup_plan.schema, CLEANUP_PLAN_SCHEMA_V15);
        validate_structure(&copy, function).unwrap();
        let mut drift = copy.clone();
        crate::hir::vec_loop_renewal::forge_record_field_for_test(
            &mut drift.declarations,
            &crate::hir::DeclarationId::new("row"),
            crate::hir::ResolvedType::String,
        );
        assert!(!crate::hir::vec_loop_renewal::requires_in(&drift, function));
        assert!(validate_structure(&drift, function).is_err());
        let source = crate::check(
            r#"module test.ordinary_vec_renewal;
@id("app.main") fn main()->i64 {
 let mut values=vec_with_capacity<i64>(2usize);
 let mut untouched=vec_with_capacity<i64>(1usize);
 let mut i=0;
 while i<2 { if i==0 { values=vec_push<i64>(values,i); 0 } else {0} i=i+1; 0 }
 if vec_len<i64>(values)==1usize && vec_len<i64>(untouched)==0usize {7}else{0}
}
"#,
            "ordinary-renewal.spx",
        )
        .unwrap();
        let program = crate::hir::resolve(&source).unwrap();
        let function = program.functions().get(0).expect("current replay ordinal");
        assert_eq!(function.cleanup_plan.schema, CLEANUP_PLAN_SCHEMA_V15);
        validate_structure(&program, function).unwrap();
        for mode in 0..6 {
            let mut forged = function.clone();
            for block in &mut forged.cleanup_plan.blocks {
                if mode == 0 {
                    block
                        .transitions
                        .retain(|t| !matches!(t, CleanupTransition::ReserveRenewal { .. }));
                }
                for transition in &mut block.transitions {
                    match transition {
                        CleanupTransition::ReserveRenewal { binding, .. } if mode == 1 => {
                            *binding =
                                CleanupPlace::whole(StorageId::Value(function.result_id.clone()));
                        }
                        CleanupTransition::Renew {
                            at,
                            source,
                            destination,
                        } if mode == 2 => {
                            *transition = CleanupTransition::Transfer {
                                at: at.clone(),
                                source: source.clone(),
                                destination: destination.clone(),
                            };
                        }
                        CleanupTransition::Renew {
                            source,
                            destination,
                            ..
                        } if mode == 3 => {
                            *destination = source.clone();
                        }
                        _ => {}
                    }
                }
            }
            if mode == 4 {
                forged.cleanup_plan.schema = CLEANUP_PLAN_SCHEMA_V12;
            }
            if mode == 5 {
                let crate::hir::ResolvedExprKind::Block { statements, .. } = &mut forged.body.kind
                else {
                    panic!("function block")
                };
                let crate::hir::ResolvedStatement::Let { mutable, .. } = &mut statements[0] else {
                    panic!("mutable Vec binding")
                };
                *mutable = false;
                assert!(!crate::hir::vec_loop_renewal::requires(&forged));
            }

            let error = validate_structure(&program, &forged).unwrap_err();
            assert_eq!(error.code, "SPX-H006", "mode {mode}: {error:?}");
        }
    }

    #[test]
    fn byte_renewal_rejects_missing_forged_downgraded_and_unauthenticated_proofs() {
        let source = crate::check(
            r#"module test.byte_renewal;
@id("app.main") fn main()->i64 {
 let mut buffer=bytes_zeroed(2usize);
 let untouched=bytes_zeroed(1usize);
 let mut index=0usize;
 while index<2usize {
  if index==0usize {buffer=bytes_set(buffer,index,65u8);0}else{0}
  index=index+1usize;
  0
 }
 if byte_len(bytes_as_slice(buffer))==2usize && byte_len(bytes_as_slice(untouched))==1usize {7}else{0}
}
"#,
            "byte-renewal.spx",
        )
        .unwrap();
        let program = crate::hir::resolve(&source).unwrap();
        let function = program.functions().get(0).expect("current replay ordinal");
        assert_eq!(function.cleanup_plan.schema, CLEANUP_PLAN_SCHEMA_V17);
        validate_structure(&program, function).unwrap();
        for mode in 0..6 {
            let mut forged = function.clone();
            for block in &mut forged.cleanup_plan.blocks {
                if mode == 0 {
                    block.transitions.retain(|transition| {
                        !matches!(transition, CleanupTransition::ReserveRenewal { .. })
                    });
                }
                for transition in &mut block.transitions {
                    match transition {
                        CleanupTransition::ReserveRenewal { binding, .. } if mode == 1 => {
                            *binding =
                                CleanupPlace::whole(StorageId::Value(function.result_id.clone()));
                        }
                        CleanupTransition::Renew {
                            at,
                            source,
                            destination,
                        } if mode == 2 => {
                            *transition = CleanupTransition::Transfer {
                                at: at.clone(),
                                source: source.clone(),
                                destination: destination.clone(),
                            };
                        }
                        CleanupTransition::Renew {
                            source,
                            destination,
                            ..
                        } if mode == 3 => {
                            *destination = source.clone();
                        }
                        _ => {}
                    }
                }
            }
            if mode == 4 {
                forged.cleanup_plan.schema = CLEANUP_PLAN_SCHEMA_V16;
            }
            if mode == 5 {
                let ResolvedExprKind::Block { statements, .. } = &mut forged.body.kind else {
                    panic!("function block")
                };
                let ResolvedStatement::Let { mutable, .. } = &mut statements[0] else {
                    panic!("mutable Bytes binding")
                };
                *mutable = false;
                assert!(!crate::byte_ops::requires_same_owner_set(&forged));
            }
            let error = validate_structure(&program, &forged).unwrap_err();
            assert_eq!(error.code, "SPX-H006", "mode {mode}: {error:?}");
        }
    }
}

#[cfg(test)]
#[path = "renewal/owned_commit.rs"]
mod owned_commit;
