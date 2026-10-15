//! Exact projection, complete replay and physical lifetime receivers.
use super::*;
use crate::bounded_output::with_limit_usage_refusal;
use crate::cleanup_plan::replay::*;
use crate::test_allocation_observer::measure;
mod old_frame;

fn program(source: &str) -> ResolvedProgram {
    crate::hir::resolve(&crate::parse(source, "param-view.spx").unwrap()).unwrap()
}
fn arity_program(arity: usize) -> ResolvedProgram {
    let signature = (0..arity)
        .map(|i| format!("p{i}:i64"))
        .collect::<Vec<_>>()
        .join(",");
    program(&format!("module param.view; @id(\"target\") fn target({signature})->i64 {{0}} @id(\"main\") fn main()->i64 {{0}}"))
}
fn assert_projection(old: &[ResolvedParam], view: &Params<'_>) {
    assert_eq!(old.len(), view.len());
    for (old, current) in old.iter().zip(view.iter()) {
        assert_eq!(&old.ty, current.ty);
        assert_eq!(old.ownership, current.ownership);
    }
    assert!(view.get(old.len()).is_none());
}
#[test]
fn authored_arity_zero_one_two_seventeen_borrows_actual_params_without_any_request() {
    for arity in [0, 1, 2, 17] {
        let program = arity_program(arity);
        let target = &program.functions[0];
        let main = &program.functions[1];
        let old = resolved_call_params(&program, main, &main.body, &target.id, None, &[]).unwrap();
        assert_eq!(
            old, target.params,
            "complete old IDs/names/spans/types/ownership projection"
        );
        let view =
            resolved_call_param_view(&program, main, &main.body, &target.id, None, &[]).unwrap();
        assert_projection(&old, &view);
        let Params::Borrowed(actual) = &view else {
            panic!("authored params must be borrowed")
        };
        assert_eq!(actual.as_ptr(), target.params.as_ptr());
        for (index, parameter) in view.iter().enumerate() {
            assert!(std::ptr::eq(parameter.ty, &target.params[index].ty));
        }
        drop(view);
        let (_, before) = measure(|| {
            drop(resolved_call_params(&program, main, &main.body, &target.id, None, &[]).unwrap())
        });
        let (_, after) = measure(|| {
            let view = resolved_call_param_view(&program, main, &main.body, &target.id, None, &[])
                .unwrap();
            let Params::Borrowed(actual) = &view else {
                panic!("borrowed receiver")
            };
            assert_eq!(actual.as_ptr(), target.params.as_ptr());
            drop(view);
        });
        let removed = arity * std::mem::size_of::<ResolvedParam>()
            + target.params.iter().map(|p| p.name.len()).sum::<usize>();
        assert_eq!(before.allocated, after.allocated + removed);
        assert_eq!(
            before.allocations,
            after.allocations + if arity == 0 { 0 } else { arity + 1 }
        );
        for observed in [before, after] {
            assert!(!observed.invalid && observed.live == 0);
            assert_eq!(observed.allocated, observed.freed);
        }
    }
}

#[test]
fn generic_instances_borrow_exact_target_and_hostile_instance_errors_match_legacy() {
    let program = program("module param.generic; @id(\"keep\") fn keep<T>(value:T)->T {value} @id(\"main\") fn main()->i64 {keep<i64>(7)}");
    let main = program
        .functions
        .iter()
        .find(|f| f.id.as_str() == "main")
        .unwrap();
    let call = replay_expression_child(&main.body, 0).unwrap_or(&main.body);
    let call = if matches!(call.kind, ResolvedExprKind::Call { .. }) {
        call
    } else {
        &main.body
    };
    let ResolvedExprKind::Call {
        callee,
        instance,
        type_arguments,
        ..
    } = &call.kind
    else {
        panic!("generic call fixture")
    };
    let target =
        crate::cleanup::ReplayContext::resolve_call_target(&program, callee, instance.as_ref())
            .unwrap();
    let old = resolved_call_params(
        &program,
        main,
        call,
        callee,
        instance.as_ref(),
        type_arguments,
    )
    .unwrap();
    assert_eq!(old, target.params);
    let view = resolved_call_param_view(
        &program,
        main,
        call,
        callee,
        instance.as_ref(),
        type_arguments,
    )
    .unwrap();
    assert_projection(&old, &view);
    let Params::Borrowed(actual) = view else {
        panic!("instance params must be borrowed")
    };
    assert_eq!(actual.as_ptr(), target.params.as_ptr());
    let wrong = DeclarationId::new("unknown.target");
    let before = resolved_call_params(
        &program,
        main,
        call,
        &wrong,
        instance.as_ref(),
        type_arguments,
    )
    .unwrap_err();
    let after = resolved_call_param_view(
        &program,
        main,
        call,
        &wrong,
        instance.as_ref(),
        type_arguments,
    )
    .err()
    .unwrap();
    assert_eq!(before, after);
}

#[test]
fn every_closed_intrinsic_keeps_legacy_projection_and_ignored_type_argument_order() {
    let program = arity_program(0);
    let function = &program.functions[1];
    let mut views = Vec::new();
    views.extend(crate::byte_ops::ByteOp::ALL.into_iter().map(Params::Byte));
    use crate::string_ops::StringOp;
    views.extend(
        StringOp::ALL
            .into_iter()
            .chain(StringOp::TEXT_TOOLKIT)
            .chain(StringOp::COLLECTIONS)
            .chain(StringOp::CONVERSIONS)
            .chain([
                StringOp::Take,
                StringOp::FromUtf8,
                StringOp::I64FromU8,
                StringOp::I64FromI32,
                StringOp::UsizeFromU8,
                StringOp::U8FromI64,
                StringOp::CharFromU8,
                StringOp::CharFromI64,
            ])
            .map(Params::String),
    );
    use crate::str_ops::StrOp;
    views.extend(
        [
            StrOp::LenBytes,
            StrOp::IsEmpty,
            StrOp::StartsWith,
            StrOp::Contains,
            StrOp::ByteAt,
        ]
        .into_iter()
        .map(Params::Str),
    );
    for view in views {
        let (id, names) = match &view {
            Params::Byte(op) => (op.id(), op.param_names()),
            Params::String(op) => (op.id(), op.param_names()),
            Params::Str(op) => (op.id(), op.param_names()),
            _ => unreachable!(),
        };
        let callee = DeclarationId::new(id);
        let old =
            resolved_call_params(&program, function, &function.body, &callee, None, &[]).unwrap();
        assert_projection(&old, &view);
        for (index, parameter) in old.iter().enumerate() {
            assert_eq!(parameter.name, names[index]);
            assert_eq!(
                parameter.id,
                crate::hir::ValueId::intrinsic_parameter(id, index)
            );
            assert_eq!(parameter.span, crate::ast::Span::default());
        }
        for args in [&[][..], &[ResolvedType::I64][..]] {
            let old = resolved_call_params(&program, function, &function.body, &callee, None, args)
                .unwrap();
            let view =
                resolved_call_param_view(&program, function, &function.body, &callee, None, args)
                    .unwrap();
            assert_projection(&old, &view);
        }
    }
}

#[test]
fn excluded_collection_closure_native_and_dedicated_provider_errors_remain_owned() {
    let program = program("module param.native; @id(\"host\") interface Host permits {} { @id(\"host.combine\") import rust fn combine(left:i64,selected:bool)->i64 effects {} failure status \"param.test\"; } @id(\"main\") fn main()->i64 {combine(2,true)}");
    let function = &program.functions[0];
    let native = DeclarationId::new("host.combine");
    let old = resolved_call_params(&program, function, &function.body, &native, None, &[]).unwrap();
    let view =
        resolved_call_param_view(&program, function, &function.body, &native, None, &[]).unwrap();
    assert_projection(&old, &view);
    let Params::Owned(actual) = view else {
        panic!("native provider remains owned")
    };
    assert_eq!(old, actual);
    for (callee, arguments) in [
        (DeclarationId::new(crate::vec_field::ID), vec![]),
        (DeclarationId::new(crate::literal_format::ID), vec![]),
        (
            DeclarationId::new(crate::hir::closure::once::INVOKE_ID),
            vec![ResolvedType::I64],
        ),
        (
            (*crate::hir::function_value::INVOKE_ID).clone(),
            vec![ResolvedType::I64],
        ),
        (DeclarationId::new(crate::vec_ops::VecOp::Len.id()), vec![]),
    ] {
        let old = resolved_call_params(
            &program,
            function,
            &function.body,
            &callee,
            None,
            &arguments,
        );
        let new = resolved_call_param_view(
            &program,
            function,
            &function.body,
            &callee,
            None,
            &arguments,
        );
        match (old, new) {
            (Err(old), Err(new)) => assert_eq!(old, new),
            (Ok(old), Ok(Params::Owned(new))) => assert_eq!(old, new),
            _ => panic!("excluded provider changed its validation/fallback"),
        }
    }
}

const REPLAY: &str = "module param.replay; @id(\"consume\") fn consume(a:string,b:string)->i64 {string_len(a)+string_len(b)} @id(\"main\") fn main()->i64 {consume(string_concat(\"a\",\"b\"),string_concat(\"c\",\"d\"))}";
fn paths(program: &ResolvedProgram, owned: bool) -> Vec<Vec<ExprSkeletonPath>> {
    let operation = || {
        let mut budget = ReplayBudget::unobserved();
        reserve_program_skeleton_work(program, program.functions.iter(), &mut budget).unwrap();
        program
            .functions
            .iter()
            .map(|function| {
                let mut work = SkeletonWork::new(program, function, &mut budget);
                expression_skeleton(program, function, &function.body, &mut work).unwrap()
            })
            .collect()
    };
    if owned {
        with_owned_oracle(operation)
    } else {
        operation()
    }
}
#[test]
fn full_old_constructor_replay_and_atomic_commit_order_are_identical() {
    let program = program(REPLAY);
    let old = paths(&program, true);
    let new = paths(&program, false);
    assert_eq!(old.len(), new.len());
    for (old, new) in old.iter().zip(&new) {
        assert_eq!(old.len(), new.len());
        for (old, new) in old.iter().zip(new) {
            assert_eq!(old.observations, new.observations);
            assert_eq!(old.owned_source, new.owned_source);
            assert_eq!(
                (old.failed, old.residual, &old.continuation),
                (new.failed, new.residual, &new.continuation)
            );
        }
    }
    with_owned_oracle(|| validate_program(&program)).unwrap();
    validate_program(&program).unwrap();
    let mut corrupt = program.clone();
    let main = corrupt
        .functions
        .iter_mut()
        .find(|f| f.id.as_str() == "main")
        .unwrap();
    let commit = main
        .cleanup_plan
        .blocks
        .iter_mut()
        .flat_map(|b| &mut b.transitions)
        .find_map(|t| match t {
            crate::cleanup_plan::CleanupTransition::CallCommit { arguments, .. }
                if arguments.len() == 2 =>
            {
                Some(arguments)
            }
            _ => None,
        })
        .unwrap();
    commit.swap(0, 1);
    let old = with_owned_oracle(|| validate_program(&corrupt));
    let new = validate_program(&corrupt);
    assert!(old.is_err());
    assert_eq!(old, new);
}
fn intrinsic_bytes(id: &str, names: &[&str]) -> usize {
    let align = std::mem::align_of::<usize>();
    names.len() * std::mem::size_of::<ResolvedParam>()
        + names
            .iter()
            .enumerate()
            .map(|(index, name)| {
                let len = id.len() + 7 + index.to_string().len();
                let arc = (len + 2 * std::mem::size_of::<usize>()).div_ceil(align) * align;
                name.len() + len + arc
            })
            .sum::<usize>()
}
#[test]
fn whole_ordinary_receiver_literal_removed_requests_and_no_frame_growth_exact_short() {
    let program = program("module param.storage; @id(\"main\") fn main()->i64 {string_len(string_concat(\"a\",\"b\"))}");
    let function = &program.functions[0];
    assert_eq!(
        old_frame::bytes(),
        std::mem::size_of::<continuation_storage::Frame<'_>>(),
        "the complete old max variant must bound the new carrier too"
    );
    let observe = |owned, limit| {
        let operation = || {
            with_limit_usage_refusal(limit, || {
                measure(|| {
                    let mut budget = ReplayBudget::with_skeleton_limit(MAX_REPLAY_WORK_UNITS);
                    let mut work = SkeletonWork::new(&program, function, &mut budget);
                    let result = expression_skeleton(&program, function, &function.body, &mut work);
                    let accepted = result.is_ok();
                    if !accepted {
                        drop(
                            work.budget
                                .reserve_bytes(function, 0, "sticky parameter view")
                                .unwrap_err(),
                        );
                    }
                    drop(result);
                    drop(work);
                    drop(budget);
                    accepted
                })
            })
        };
        if owned {
            with_owned_oracle(operation)
        } else {
            operation()
        }
    };
    let ((old_ok, old), old_refused, old_charge, _) = observe(true, usize::MAX);
    let ((new_ok, new), new_refused, charge, _) = observe(false, usize::MAX);
    assert!(old_ok && new_ok && !old_refused && !new_refused);
    use crate::string_ops::StringOp;
    let removed = intrinsic_bytes(StringOp::Len.id(), StringOp::Len.param_names())
        + intrinsic_bytes(StringOp::Concat.id(), StringOp::Concat.param_names());
    assert_eq!(old.allocated, new.allocated + removed);
    assert_eq!(old.allocations, new.allocations + 11);
    assert_eq!(old_charge, charge + removed);
    for metrics in [old, new] {
        assert!(!metrics.invalid && metrics.live == 0);
        assert_eq!(metrics.allocated, metrics.freed);
    }
    let ((ok, exact), refused, actual, _) = observe(false, charge);
    assert!(ok && !refused && actual == charge && !exact.invalid && exact.live == 0);
    let ((ok, short), refused, _, receipt) = observe(false, charge - 1);
    assert!(!ok && refused && receipt.is_some() && !short.invalid && short.live == 0);
}

#[test]
fn whole_authored_receiver_arity_seventeen_removes_only_owned_parameter_payload() {
    let signature = (0..17)
        .map(|i| format!("p{i}:i64"))
        .collect::<Vec<_>>()
        .join(",");
    let args = (0..17).map(|i| i.to_string()).collect::<Vec<_>>().join(",");
    let program = program(&format!("module param.wide; @id(\"target\") fn target({signature})->i64 {{0}} @id(\"main\") fn main()->i64 {{target({args})}}"));
    let target = &program.functions[0];
    let function = &program.functions[1];
    let observe = |owned, limit| {
        let operation = || {
            with_limit_usage_refusal(limit, || {
                measure(|| {
                    let mut budget = ReplayBudget::with_skeleton_limit(MAX_REPLAY_WORK_UNITS);
                    let mut work = SkeletonWork::new(&program, function, &mut budget);
                    let result = expression_skeleton(&program, function, &function.body, &mut work);
                    let accepted = result.is_ok();
                    drop(result);
                    drop(work);
                    drop(budget);
                    accepted
                })
            })
        };
        if owned {
            with_owned_oracle(operation)
        } else {
            operation()
        }
    };
    let ((old_ok, old), old_refused, old_charge, _) = observe(true, usize::MAX);
    let ((new_ok, new), refused, charge, _) = observe(false, usize::MAX);
    let removed = 17 * std::mem::size_of::<ResolvedParam>()
        + target.params.iter().map(|p| p.name.len()).sum::<usize>();
    assert!(old_ok && new_ok && !old_refused && !refused);
    assert_eq!(old.allocated, new.allocated + removed);
    assert_eq!(old.allocations, new.allocations + 18);
    assert_eq!(old_charge, charge + removed);
    for observed in [old, new] {
        assert!(!observed.invalid && observed.live == 0);
        assert_eq!(observed.allocated, observed.freed);
    }
    let ((ok, exact), refused, actual, _) = observe(false, charge);
    assert!(ok && !refused && actual == charge && !exact.invalid && exact.live == 0);
    let ((ok, short), refused, _, receipt) = observe(false, charge - 1);
    assert!(!ok && refused && receipt.is_some() && !short.invalid && short.live == 0);
}

#[test]
fn saved_constructor_work_refusals_and_hostile_argument_arity_type_are_equal() {
    let program = program(REPLAY);
    let function = program
        .functions
        .iter()
        .find(|f| f.id.as_str() == "main")
        .unwrap();
    for units in [0, 1, 3, 8, 17, 31, 63, 127, MAX_REPLAY_WORK_UNITS] {
        let execute = || {
            let mut budget = ReplayBudget::with_skeleton_limit(units);
            let mut work = SkeletonWork::new(&program, function, &mut budget);
            let result = expression_skeleton(&program, function, &function.body, &mut work);
            (result, work.budget.skeleton_spent)
        };
        let (old, old_spent) = with_owned_oracle(execute);
        let (new, new_spent) = execute();
        assert_eq!(old_spent, new_spent);
        match (old, new) {
            (Ok(old), Ok(new)) => {
                assert_eq!(old.len(), new.len());
                for (a, b) in old.iter().zip(&new) {
                    assert_eq!(a.observations, b.observations);
                }
            }
            (Err(old), Err(new)) => assert_eq!(old, new),
            _ => panic!("parameter views changed the first work refusal"),
        }
    }
    for wrong_type in [false, true] {
        let mut corrupt = program.clone();
        let main = corrupt
            .functions
            .iter_mut()
            .find(|f| f.id.as_str() == "main")
            .unwrap();
        fn call(expression: &mut crate::hir::ResolvedExpr) -> &mut Vec<crate::hir::ResolvedExpr> {
            match &mut expression.kind {
                ResolvedExprKind::Block { tail, .. } => call(tail),
                ResolvedExprKind::Call { args, .. } => args,
                _ => panic!("authored call fixture"),
            }
        }
        let args = call(&mut main.body);
        if wrong_type {
            args[0].ty = ResolvedType::I64;
        } else {
            args.pop();
        }
        let old = with_owned_oracle(|| validate_program(&corrupt));
        let new = validate_program(&corrupt);
        assert!(old.is_err());
        assert_eq!(old, new);
    }
}

#[test]
fn ordinary_vec_fallback_keeps_original_owned_provider_and_payment_with_old_selector() {
    let program = program("module param.vector; @id(\"probe\") fn probe()->usize {let mut values=vec_with_capacity<i64>(2usize); values=vec_push<i64>(values,7); vec_len<i64>(values)} @id(\"main\") fn main()->i64 {0}");
    let function = &program.functions[0];
    // This is an actual ordinary Vec expression, outside selected admission.
    let callee = DeclarationId::new(crate::vec_ops::VecOp::Len.id());
    let old = resolved_call_params(
        &program,
        function,
        &function.body,
        &callee,
        None,
        &[ResolvedType::I64],
    )
    .unwrap();
    let view = resolved_call_param_view(
        &program,
        function,
        &function.body,
        &callee,
        None,
        &[ResolvedType::I64],
    )
    .unwrap();
    let Params::Owned(current) = view else {
        panic!("Vec remains on its owned fallback")
    };
    assert_eq!(old, current);
    let observe = |owned| {
        let operation = || {
            with_limit_usage_refusal(usize::MAX, || {
                measure(|| {
                    let mut budget = ReplayBudget::with_skeleton_limit(MAX_REPLAY_WORK_UNITS);
                    let mut work = SkeletonWork::new(&program, function, &mut budget);
                    assert!(!work.budget.selected());
                    let result = expression_skeleton(&program, function, &function.body, &mut work);
                    let result = result.unwrap();
                    let spent = work.budget.skeleton_spent;
                    drop(result);
                    drop(work);
                    drop(budget);
                    spent
                })
            })
        };
        if owned {
            with_owned_oracle(operation)
        } else {
            operation()
        }
    };
    let ((old_spent, old), old_refused, old_charge, _) = observe(true);
    let ((current_spent, current), current_refused, current_charge, _) = observe(false);
    assert!(!old_refused && !current_refused);
    assert_eq!(old_spent, current_spent);
    assert_eq!(
        old_charge, current_charge,
        "original ordinary Vec reservations stay unchanged"
    );
    assert_eq!(old.allocated, current.allocated);
    assert_eq!(old.allocations, current.allocations);
    for observed in [old, current] {
        assert!(!observed.invalid && observed.live == 0);
        assert_eq!(observed.allocated, observed.freed);
    }
    let old = paths(&program, true);
    let current = paths(&program, false);
    for (old, current) in old.iter().zip(&current) {
        assert_eq!(old.len(), current.len());
        for (old, current) in old.iter().zip(current) {
            assert_eq!(old.observations, current.observations);
            assert_eq!(old.owned_source, current.owned_source);
            assert_eq!(
                (old.failed, old.residual, &old.continuation),
                (current.failed, current.residual, &current.continuation)
            );
        }
    }
    with_owned_oracle(|| validate_program(&program)).unwrap();
    validate_program(&program).unwrap();
}

fn ordinal_transfers(
    params: &[ResolvedParam],
    ordinals: &[u32],
) -> Vec<crate::cleanup_plan::CallArgumentTransfer> {
    ordinals
        .iter()
        .map(
            |&parameter_index| crate::cleanup_plan::CallArgumentTransfer {
                parameter_index,
                source: CleanupPlace {
                    storage: StorageId::Value(params[0].id.clone()),
                    projections: Vec::new(),
                },
            },
        )
        .collect()
}
#[test]
fn borrowed_commit_ordinals_preserve_full_classifier_priority_and_hostile_order() {
    let mut program = arity_program(17);
    for (index, parameter) in program.functions[0].params.iter_mut().enumerate() {
        parameter.ownership = if index % 2 == 0 {
            OwnershipMode::Own
        } else {
            OwnershipMode::Value
        };
        parameter.ty = ResolvedType::String;
    }
    let target = &program.functions[0];
    let function = &program.functions[1];
    let params = Params::Borrowed(&target.params);
    let canonical = (0..17).step_by(2).collect::<Vec<u32>>();
    let mut reordered = canonical.clone();
    reordered.swap(0, 1);
    let mut duplicated = canonical.clone();
    duplicated[1] = duplicated[0];
    let mut extra = canonical.clone();
    extra.push(u32::MAX);
    for ordinals in [
        canonical.clone(),
        vec![],
        vec![u32::MAX],
        canonical[1..].to_vec(),
        reordered,
        duplicated,
        extra,
    ] {
        let arguments = ordinal_transfers(&target.params, &ordinals);
        for fail_at in [None, Some(1), Some(8)] {
            let compare = |saved| {
                let mut visits = 0;
                let classifier = |_: &ResolvedType| {
                    let ordinal = visits;
                    visits += 1;
                    if Some(ordinal) == fail_at {
                        Err(replay_error(function, "later droppable type failure"))
                    } else {
                        Ok(true)
                    }
                };
                let result = if saved {
                    saved_owned_parameter_indices_match(&params, &arguments, function, classifier)
                } else {
                    owned_parameter_indices_match(&params, &arguments, function, classifier)
                };
                (result, visits)
            };
            let old = compare(true);
            let current = compare(false);
            assert_eq!(old, current);
            assert_eq!(current.1, fail_at.map_or(9, |ordinal| ordinal + 1));
            if fail_at.is_some() {
                assert!(current.0.is_err());
            } else {
                assert_eq!(current.0.unwrap(), ordinals == canonical);
            }
        }
    }
}
#[test]
fn early_commit_ordinal_mismatch_keeps_later_current_type_diagnostic() {
    let mut program = arity_program(2);
    for parameter in &mut program.functions[0].params {
        parameter.ownership = OwnershipMode::Own;
        parameter.ty = ResolvedType::String;
    }
    program.functions[0].params[1].ty = ResolvedType::Nominal {
        declaration: DeclarationId::new("missing.current.cleanup.type"),
        arguments: Vec::new(),
    };
    let target = &program.functions[0];
    let function = &program.functions[1];
    let params = Params::Borrowed(&target.params);
    for ordinals in [vec![], vec![u32::MAX], vec![1, 0], vec![0, 0]] {
        let arguments = ordinal_transfers(&target.params, &ordinals);
        let old = saved_owned_parameter_indices_match(&params, &arguments, function, |ty| {
            type_needs_drop(&program, function, ty)
        });
        let current = owned_parameter_indices_match(&params, &arguments, function, |ty| {
            type_needs_drop(&program, function, ty)
        });
        assert!(old.is_err());
        assert_eq!(old, current);
    }
}

#[test]
fn borrowed_commit_ordinals_remove_two_literal_scratch_requests_through_drop() {
    let mut program = arity_program(2);
    for parameter in &mut program.functions[0].params {
        parameter.ownership = OwnershipMode::Own;
        parameter.ty = ResolvedType::String;
    }
    let target = &program.functions[0];
    let function = &program.functions[1];
    let params = Params::Borrowed(&target.params);
    let arguments = ordinal_transfers(&target.params, &[0, 1]);
    let observe = |saved| {
        with_limit_usage_refusal(0, || {
            measure(|| {
                let result = if saved {
                    saved_owned_parameter_indices_match(&params, &arguments, function, |ty| {
                        type_needs_drop(&program, function, ty)
                    })
                } else {
                    owned_parameter_indices_match(&params, &arguments, function, |ty| {
                        type_needs_drop(&program, function, ty)
                    })
                };
                result.unwrap()
            })
        })
    };
    let ((old_ok, old), old_refused, old_charge, _) = observe(true);
    let ((current_ok, current), refused, charge, _) = observe(false);
    assert!(old_ok && current_ok && !old_refused && !refused);
    // Original push-grown u32 Vec minimum capacity 4, and exact collected
    // actual ordinal Vec capacity 2. Neither scratch vector ever had a payer.
    assert_eq!(
        old.allocated,
        current.allocated + (4 + 2) * std::mem::size_of::<u32>()
    );
    assert_eq!(old.allocations, current.allocations + 2);
    assert_eq!((old_charge, charge), (0, 0));
    for observed in [old, current] {
        assert!(!observed.invalid && observed.live == 0);
        assert_eq!(observed.allocated, observed.freed);
    }
}
#[test]
fn whole_commit_receiver_preserves_ordinary_verdicts_debit_and_exact_short() {
    let program = program(REPLAY);
    assert_eq!(
        with_saved_commit_indices(|| validate_program(&program)),
        validate_program(&program)
    );
    for ordinals in [
        vec![],
        vec![0],
        vec![1, 0],
        vec![0, 0],
        vec![0, 1, u32::MAX],
        vec![u32::MAX, 1],
    ] {
        let mut forged = program.clone();
        let main = forged
            .functions
            .iter_mut()
            .find(|function| function.id.as_str() == "main")
            .unwrap();
        let arguments = main
            .cleanup_plan
            .blocks
            .iter_mut()
            .flat_map(|block| &mut block.transitions)
            .find_map(|transition| match transition {
                crate::cleanup_plan::CleanupTransition::CallCommit { arguments, .. }
                    if arguments.len() == 2 =>
                {
                    Some(arguments)
                }
                _ => None,
            })
            .unwrap();
        let first = arguments[0].clone();
        *arguments = ordinals
            .iter()
            .map(|&parameter_index| {
                let mut argument = first.clone();
                argument.parameter_index = parameter_index;
                argument
            })
            .collect();
        let old = with_saved_commit_indices(|| validate_program(&forged));
        let current = validate_program(&forged);
        assert!(old.is_err());
        assert_eq!(old, current);
    }
    let observe = |saved, limit| {
        let operation = || {
            with_limit_usage_refusal(limit, || {
                measure(|| {
                    let result = validate_program(&program);
                    let accepted = result.is_ok();
                    drop(result);
                    accepted
                })
            })
        };
        if saved {
            with_saved_commit_indices(operation)
        } else {
            operation()
        }
    };
    let ((old_ok, old), old_refused, old_charge, _) = observe(true, usize::MAX);
    let ((current_ok, current), refused, charge, _) = observe(false, usize::MAX);
    assert!(old_ok && current_ok && !old_refused && !refused && charge > 0);
    assert_eq!(old_charge, charge, "scratch removal changes no paid debit");
    assert!(old.allocated > current.allocated && old.allocations > current.allocations);
    for observed in [old, current] {
        assert!(!observed.invalid && observed.live == 0);
        assert_eq!(observed.allocated, observed.freed);
    }
    for saved in [true, false] {
        let ((ok, exact), refused, actual, _) = observe(saved, charge);
        assert!(ok && !refused && actual == charge && !exact.invalid && exact.live == 0);
        let ((ok, short), refused, _, receipt) = observe(saved, charge - 1);
        assert!(!ok && refused && receipt.is_some() && !short.invalid && short.live == 0);
    }
}

#[test]
fn valid_eight_scalar_generic_closures_keep_full_owned_fallback_projection_receiver_and_errors() {
    // Read the unchanged original application fixture, not a smaller mirrored
    // closure. Its eight scalar instantiations/captures remain authoritative.
    let corpus = include_str!("../../../../../tests/language/function_values_generic.rs");
    let source = corpus
        .split_once("const GENERIC_TEMPLATE_CLOSURE: &str = r#\"")
        .unwrap()
        .1
        .split_once("\"#;")
        .unwrap()
        .0;
    let ast = crate::check(source, "param-generic-closures.spx").unwrap();
    let program = crate::hir::resolve(&ast).unwrap();
    assert_eq!(program.function_instances.len(), 8);
    assert_eq!(crate::hir::closure::inventory(&program).len(), 8);

    fn projections(
        program: &ResolvedProgram,
        function: &ResolvedFunction,
        expression: &crate::hir::ResolvedExpr,
    ) -> usize {
        let mut invocations = 0;
        if let ResolvedExprKind::Invoke { callable, args } = &expression.kind {
            let callee: &DeclarationId = &crate::hir::function_value::INVOKE_ID;
            let arguments = [callable.ty.clone()];
            let old = resolved_call_params(program, function, expression, callee, None, &arguments)
                .unwrap();
            let current =
                resolved_call_param_view(program, function, expression, callee, None, &arguments)
                    .unwrap();
            assert_projection(&old, &current);
            let Params::Owned(current) = current else {
                panic!("valid closure invocation must retain its owned provider")
            };
            assert_eq!(old, current);
            // The actual Invoke receiver uses cleanup_call, not ParamView.
            // Its complete params differ only by the documented source span.
            let (actual_callee, actual_args, actual) =
                crate::hir::function_value::cleanup_call(expression).unwrap();
            assert_eq!(actual_callee, callee);
            assert_eq!(actual_args, args.as_slice());
            let mut expected = old;
            for parameter in &mut expected {
                parameter.span = expression.span;
            }
            assert_eq!(actual, expected);
            assert_eq!(actual.len(), 1);
            invocations += 1;
        } else if let ResolvedExprKind::Call {
            callee,
            instance,
            type_arguments,
            ..
        } = &expression.kind
        {
            let old = resolved_call_params(
                program,
                function,
                expression,
                callee,
                instance.as_ref(),
                type_arguments,
            )
            .unwrap();
            let current = resolved_call_param_view(
                program,
                function,
                expression,
                callee,
                instance.as_ref(),
                type_arguments,
            )
            .unwrap();
            assert_projection(&old, &current);
            match current {
                Params::Owned(actual) => assert_eq!(old, actual),
                Params::Borrowed(actual) => assert_eq!(old.as_slice(), actual),
                _ => {}
            }
        }
        for index in 0.. {
            let Some(child) = replay_expression_child(expression, index) else {
                break;
            };
            invocations += projections(program, function, child);
        }
        invocations
    }
    let mut invocations = 0;
    for function in program.functions.iter().chain(
        program
            .function_instances
            .iter()
            .map(|instance| &instance.function),
    ) {
        invocations += projections(&program, function, &function.body);
    }
    assert_eq!(invocations, 8);
    // Both complete current source/HIR and ordinary independent cleanup replay
    // run with the full saved owning parameter constructor and the new route.
    for old in [false, true] {
        let run = || {
            crate::hir::validate(&program).unwrap();
            validate_program(&program).unwrap();
        };
        if old {
            with_owned_oracle(run);
        } else {
            run();
        }
    }

    // Traverse exactly the original fixture's block/while/collection-call
    // structure to corrupt a genuine invocation, without manufacturing a new
    // callable receiver or replacing any of the eight source instances.
    fn corrupt(expression: &mut crate::hir::ResolvedExpr, mutation: usize) -> bool {
        match &mut expression.kind {
            ResolvedExprKind::Invoke { callable, args } => {
                match mutation {
                    0 => args.clear(),
                    1 => {
                        args[0].ty = if args[0].ty == ResolvedType::Bool {
                            ResolvedType::I64
                        } else {
                            ResolvedType::Bool
                        };
                    }
                    _ => callable.ty = ResolvedType::I64,
                }
                true
            }
            ResolvedExprKind::Call { args, .. } => {
                args.iter_mut().any(|argument| corrupt(argument, mutation))
            }
            ResolvedExprKind::Block { statements, tail }
            | ResolvedExprKind::VecConstructor {
                statements, tail, ..
            } => {
                for statement in statements {
                    match statement {
                        crate::hir::ResolvedStatement::While {
                            condition, body, ..
                        }
                        | crate::hir::ResolvedStatement::LexicalWhile {
                            condition, body, ..
                        } => {
                            if corrupt(condition, mutation) || corrupt(body, mutation) {
                                return true;
                            }
                        }
                        _ => {
                            if corrupt(statement.value_mut(), mutation) {
                                return true;
                            }
                        }
                    }
                }
                corrupt(tail, mutation)
            }
            _ => false,
        }
    }
    for mutation in 0..3 {
        let mut hostile = program.clone();
        assert!(corrupt(
            &mut hostile.function_instances[0].function.body,
            mutation
        ));
        let old = with_owned_oracle(|| crate::hir::validate(&hostile));
        let current = crate::hir::validate(&hostile);
        assert!(old.is_err());
        assert_eq!(old, current);
        let old = with_owned_oracle(|| validate_program(&hostile));
        let current = validate_program(&hostile);
        assert!(old.is_err());
        assert_eq!(old, current);
    }

    // Hostile provider signatures still select the complete original error,
    // independently of the actual receiver's callable/argument validation.
    let function = &program.function_instances[0].function;
    for arguments in [vec![], vec![ResolvedType::I64], vec![ResolvedType::Bool]] {
        let old = resolved_call_params(
            &program,
            function,
            &function.body,
            &crate::hir::function_value::INVOKE_ID,
            None,
            &arguments,
        );
        let current = resolved_call_param_view(
            &program,
            function,
            &function.body,
            &crate::hir::function_value::INVOKE_ID,
            None,
            &arguments,
        );
        assert!(old.is_err());
        assert_eq!(old.unwrap_err(), current.err().unwrap());
    }
}
