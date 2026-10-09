//! Replay narrowly authenticated same-owner renewal without changing canonical history.
use super::*;
use crate::cleanup_plan::CLEANUP_PLAN_SCHEMA_V17;

pub(super) fn validate_binding(
    program: &ResolvedProgram,
    function: &ResolvedFunction,
    at: &ExpressionId,
    place: &CleanupPlace,
) -> Result<(), Diagnostic> {
    if !matches!(
        function.cleanup_plan.schema,
        CLEANUP_PLAN_SCHEMA_V12
            | CLEANUP_PLAN_SCHEMA_V13
            | CLEANUP_PLAN_SCHEMA_V14
            | CLEANUP_PLAN_SCHEMA_V15
            | CLEANUP_PLAN_SCHEMA_V16
            | CLEANUP_PLAN_SCHEMA_V17
    ) || (crate::hir::vec_loop_renewal::binding(function, at).is_some()
        && !matches!(
            function.cleanup_plan.schema,
            CLEANUP_PLAN_SCHEMA_V15 | CLEANUP_PLAN_SCHEMA_V16 | CLEANUP_PLAN_SCHEMA_V17
        ))
        || (crate::string_ops::replacement::binding(function, at).is_some()
            && !matches!(
                function.cleanup_plan.schema,
                CLEANUP_PLAN_SCHEMA_V16 | CLEANUP_PLAN_SCHEMA_V17
            ))
        || (crate::byte_ops::same_owner_set_binding(function, at).is_some()
            && function.cleanup_plan.schema != CLEANUP_PLAN_SCHEMA_V17)
        || !crate::cleanup_plan::renewal_binding(program, function, at).is_some_and(|binding| {
            *place == CleanupPlace::whole(StorageId::Value(binding.id.clone()))
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
    program: &ResolvedProgram,
    function: &ResolvedFunction,
    at: &ExpressionId,
    destination: &CleanupPlace,
) -> Result<(), Diagnostic> {
    if crate::cleanup_plan::renewal_binding(program, function, at).is_some_and(|binding| {
        *destination == CleanupPlace::whole(StorageId::Value(binding.id.clone()))
    }) {
        return Err(replay_error(
            function,
            "authenticated renewal uses an ordinary transfer",
        ));
    }
    Ok(())
}
pub(super) fn reserve(
    program: &ResolvedProgram,
    function: &ResolvedFunction,
    at: &ExpressionId,
    binding: &CleanupPlace,
    state: &mut PathState,
    storage: &BTreeSet<StorageId>,
    leaves: &Leaves,
) -> Result<(), Diagnostic> {
    validate_binding(program, function, at, binding)?;
    let flags = validate_place(function, binding, storage, leaves)?;
    if flags.len() != 1
        || !state.live_order.contains(&flags[0])
        || state.renewals.contains_key(at)
        || (!matches!(
            function.cleanup_plan.schema,
            CLEANUP_PLAN_SCHEMA_V16 | CLEANUP_PLAN_SCHEMA_V17
        ) && !state.renewals.is_empty())
    {
        return Err(replay_error(
            function,
            "renewal reservation requires one live unreserved owner leaf",
        ));
    }
    state.renewals.insert(at.clone(), state.live_order.clone());
    Ok(())
}
pub(super) fn renew(
    program: &ResolvedProgram,
    function: &ResolvedFunction,
    at: &ExpressionId,
    (source, destination): (&CleanupPlace, &CleanupPlace),
    state: &mut PathState,
    storage: &BTreeSet<StorageId>,
    leaves: &Leaves,
) -> Result<(), Diagnostic> {
    validate_binding(program, function, at, destination)?;
    let history = state
        .renewals
        .remove(at)
        .ok_or_else(|| replay_error(function, "renewal has no reservation"))?;
    let flags = validate_place(function, destination, storage, leaves)?;
    if flags.len() != 1 {
        return Err(replay_error(
            function,
            "renewal destination is not one owner leaf",
        ));
    }
    if crate::string_ops::replacement::binding(function, at).is_some() {
        let source_flags = validate_place(function, source, storage, leaves)?;
        if source_flags == flags {
            return Err(replay_error(
                function,
                "String replacement aliases its new owner",
            ));
        }
        state.live_order.retain(|flag| !flags.contains(flag));
    }
    replay_transfer(function, state, source, destination, storage, leaves)?;
    let flag = flags[0];
    if crate::string_ops::replacement::binding(function, at).is_some() {
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
    state.live_order = history;
    Ok(())
}
pub(super) fn prepend_reservation(
    function: &ResolvedFunction,
    expression: &ResolvedExpr,
    paths: &mut [ExprSkeletonPath],
    work: &mut SkeletonWork<'_, '_>,
) -> Result<(), Diagnostic> {
    let binding = work
        .renewal_binding(&expression.id)
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
        path.observations
            .insert_front(SkeletonObservation::ReserveRenewal { at, binding }.into());
    }
    Ok(())
}

pub(super) fn validate_join_compatibility(
    function: &ResolvedFunction,
    existing: &BTreeSet<PathState>,
    incoming: &PathState,
    block: BlockId,
) -> Result<(), Diagnostic> {
    if existing.iter().any(|state| {
        state.renewals != incoming.renewals
            || state.pending_failure != incoming.pending_failure
            || state.selected_failure != incoming.selected_failure
            || state.published != incoming.published
    }) {
        return Err(replay_error(
            function,
            format!(
                "cleanup join at block {} has incompatible control states",
                block.0
            ),
        ));
    }

    let histories = existing
        .iter()
        .map(|state| &state.live_order)
        .chain(std::iter::once(&incoming.live_order))
        .collect::<Vec<_>>();
    let flags = histories
        .iter()
        .flat_map(|history| history.iter().copied())
        .collect::<BTreeSet<_>>();
    let mut successors = flags
        .iter()
        .map(|flag| (*flag, BTreeSet::new()))
        .collect::<BTreeMap<_, _>>();
    let mut indegree = flags
        .iter()
        .map(|flag| (*flag, 0_usize))
        .collect::<BTreeMap<_, _>>();
    for history in histories {
        for pair in history.windows(2) {
            if successors
                .get_mut(&pair[0])
                .expect("replayed live flag is indexed")
                .insert(pair[1])
            {
                indegree.entry(pair[1]).and_modify(|degree| *degree += 1);
            }
        }
    }
    let mut ready = indegree
        .iter()
        .filter_map(|(flag, degree)| (*degree == 0).then_some(*flag))
        .collect::<BTreeSet<_>>();
    let mut visited = 0_usize;
    while let Some(flag) = ready.pop_first() {
        visited += 1;
        for successor in &successors[&flag] {
            let degree = indegree
                .get_mut(successor)
                .expect("replayed successor flag is indexed");
            *degree -= 1;
            if *degree == 0 {
                ready.insert(*successor);
            }
        }
    }
    if visited != flags.len() {
        return Err(replay_error(
            function,
            format!(
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
            .saturating_add(flags.len())
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
        CleanupTransition::ReserveRenewal { at, .. } => state
            .live_order
            .len()
            .saturating_add(at.as_str().len())
            .saturating_add(1),
        CleanupTransition::Renew { .. } => {
            retained_units(state).saturating_add(state.live_order.len())
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
            .functions
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
            .functions
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
            .functions
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
        let function = &program.functions[0];
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
        let function = &program.functions[0];
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
