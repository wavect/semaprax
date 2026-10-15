//! Full saved-map function chain comparisons and physical lifetime controls.
use super::*;
use crate::test_allocation_observer::measure;
const LIMIT: usize = 64 * 1024 * 1024;
const CONTRACT_SOURCE: &str = r#"module contract.hir;
@id("inspect") fn inspect(input:own Bytes)->bool {byte_len(bytes_as_slice(input))==2usize}
@id("factory") fn factory(n:usize)->Bytes {bytes_zeroed(2usize)}
@id("probe") fn probe(input:own Bytes)->Bytes
 requires {let local=bytes_zeroed(2usize);inspect(local)}
 ensures {let checked=bytes_zeroed(2usize);byte_len(bytes_as_slice(checked))==2usize}
 {bytes_zeroed(1usize)}
@id("main") fn main()->i64{0}
"#;
const PAYLOAD_SOURCE: &str = r#"module function.payload;
@id("payload") fn payload(input:own Box<i64>)->Box<i64>
 requires true ensures true {box_new<i64>(box_into_inner<i64>(input))}
@id("main") fn main()->i64 {let value=payload(box_new<i64>(3));box_into_inner<i64>(value)}
"#;
fn program(source: &str) -> ResolvedProgram {
    let ast = crate::check(source, "function-scope.spx").unwrap();
    crate::hir::resolve(&ast).unwrap()
}
fn compare(program: &ResolvedProgram, function: &ResolvedFunction) -> Result<(), Diagnostic> {
    let execution = FunctionExecutionId::Monomorphic(function.id.clone());
    let mut current = HirValidator::new(program).unwrap();
    let mut old = current.clone();
    let expected = old.validate_function_owned_reference(function, &execution);
    let actual = current.validate_function(function, &execution);
    HirValidator::assert_validation_oracle(
        &actual,
        &expected,
        &current,
        &old,
        &BTreeMap::new(),
        &BTreeMap::new(),
        function.id.as_str(),
    );
    HirValidator::assert_scope_auxiliary_oracle(&current, &old);
    actual
}
#[test]
fn production_chain_matches_complete_saved_map_function_and_full_iterative_corpus() {
    for source in [
        crate::hir::iterative_validator_tests::SOURCE,
        CONTRACT_SOURCE,
        PAYLOAD_SOURCE,
    ] {
        let program = program(source);
        for function in &program.functions {
            compare(&program, function).unwrap();
        }
    }
}
#[test]
fn production_chain_preserves_parameter_requires_body_result_and_ensures_first_errors() {
    let program = program(CONTRACT_SOURCE);
    let original = program
        .functions
        .iter()
        .find(|f| f.id.as_str() == "probe")
        .unwrap();
    for phase in [
        "parameter",
        "requires",
        "body",
        "result",
        "ensures",
        "requires-owner",
        "ensures-owner",
    ] {
        let mut function = original.clone();
        match phase {
            "parameter" => function.params[0].id = ValueId::new("bad.parameter".to_owned()),
            "requires" => function.requires[0].ty = ResolvedType::I64,
            "body" => function.body.ownership = OwnershipMode::Borrow,
            "result" => function.result_id = ValueId::new("bad.result".to_owned()),
            "ensures" => function.ensures[0].ty = ResolvedType::I64,
            "requires-owner" | "ensures-owner" => {
                let ensures = phase == "ensures-owner";
                let root = if ensures {
                    function.result_id.clone()
                } else {
                    function.params[0].id.clone()
                };
                let clause = if ensures {
                    &mut function.ensures[0]
                } else {
                    &mut function.requires[0]
                };
                let ResolvedExprKind::Block { statements, .. } = &mut clause.kind else {
                    panic!("contract block")
                };
                let ResolvedStatement::Let { value, .. } = &mut statements[0] else {
                    panic!("fresh local")
                };
                value.kind = ResolvedExprKind::Place(Place {
                    root,
                    projections: Vec::new(),
                });
            }
            _ => unreachable!(),
        }
        compare(&program, &function).unwrap_err();
    }
}
fn lifetime(
    program: &ResolvedProgram,
    function: &ResolvedFunction,
    limit: usize,
    old: bool,
) -> (crate::test_allocation_observer::Metrics, bool, usize, bool) {
    // Constructor acquisition is held outside the function-chain observation.
    // All new function validation backing and its complete Drop are inside it.
    let mut validator = HirValidator::new(program).unwrap();
    let execution = FunctionExecutionId::Monomorphic(function.id.clone());
    let (((), metrics), refused, paid, receipt) =
        crate::bounded_output::with_limit_usage_refusal(limit, || {
            measure(|| {
                let result = if old {
                    validator.validate_function_owned_reference(function, &execution)
                } else {
                    validator.validate_function(function, &execution)
                };
                if result.is_err() {
                    assert!(!crate::bounded_output::reserve_active_required(0));
                } else {
                    result.as_ref().unwrap();
                }
                drop(result);
                drop(validator);
            })
        });
    (metrics, refused, paid, receipt.is_some())
}
#[test]
fn production_chain_observes_actual_backing_debits_complete_drop_and_exact_short_sticky() {
    for source in [
        CONTRACT_SOURCE,
        crate::hir::iterative_validator_tests::SOURCE,
        PAYLOAD_SOURCE,
    ] {
        let program = program(source);
        for function in &program.functions {
            let (actual, refused, paid, receipt) = lifetime(&program, function, LIMIT, false);
            let (old, old_refused, _, old_receipt) = lifetime(&program, function, LIMIT, true);
            assert!(
                !refused
                    && !old_refused
                    && !receipt
                    && !old_receipt
                    && !actual.invalid
                    && !old.invalid
            );
            assert_eq!(actual.live, 0);
            assert_eq!(old.live, 0);
            assert!(actual.allocated > 0 && paid > 0);
            assert!(
                actual.allocated <= paid && actual.highwater <= paid,
                "function {}: actual {actual:?}, paid {paid}",
                function.id
            );
            let (exact, refused, exact_paid, exact_receipt) =
                lifetime(&program, function, paid, false);
            assert!(!refused && !exact_receipt && !exact.invalid);
            assert_eq!(exact.live, 0);
            assert_eq!(exact_paid, paid);
            assert_eq!(exact.allocated, actual.allocated);
            let (short, refused, short_paid, short_receipt) =
                lifetime(&program, function, paid - 1, false);
            assert!(refused && short_receipt && !short.invalid);
            assert_eq!(short.live, 0);
            assert!(short_paid < paid);
        }
    }
}
#[test]
fn production_chain_validator_drop_is_complete_on_postvalidation_unwind() {
    let program = program(CONTRACT_SOURCE);
    for function in &program.functions {
        let mut validator = HirValidator::new(&program).unwrap();
        let execution = FunctionExecutionId::Monomorphic(function.id.clone());
        let (((), observed), refused, _) = crate::bounded_output::with_limit_usage(LIMIT, || {
            measure(|| {
                let caught = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                    validator.validate_function(function, &execution).unwrap();
                    // Ownership of the validator enters the unwinding closure.
                    let _validator = validator;
                    // Own the real message payload inside the observation. resume_unwind
                    // skips the harness panic hook, whose output capture outlives it.
                    std::panic::resume_unwind(Box::new(String::from(
                        "injected after complete production function chain",
                    )));
                }));
                assert!(caught.is_err());
                assert_eq!(
                    caught
                        .as_ref()
                        .err()
                        .unwrap()
                        .downcast_ref::<String>()
                        .map(String::as_str),
                    Some("injected after complete production function chain"),
                );
                drop(caught);
            })
        });
        assert!(!refused && !observed.invalid);
        assert_eq!(observed.live, 0);
    }
}

#[test]
fn persistent_requires_and_ensures_keep_hostile_flags_and_full_outer_metadata_isolated() {
    let program = program(CONTRACT_SOURCE);
    let original = program
        .functions
        .iter()
        .find(|f| f.id.as_str() == "probe")
        .unwrap();
    for ensures in [false, true] {
        for hostile in [false, true] {
            for admitted in [false, true] {
                let mut function = original.clone();
                let execution = FunctionExecutionId::Monomorphic(function.id.clone());
                let mut outer = BTreeMap::new();
                for (id, ty, ownership) in function
                    .params
                    .iter()
                    .map(|p| (&p.id, &p.ty, p.ownership))
                    .chain(std::iter::once((
                        &function.result_id,
                        &function.return_type,
                        OwnershipMode::Own,
                    )))
                {
                    let path = vec![PlaceProjection::Field(DeclarationId::new("scope.field")); 3];
                    outer.insert(
                        id.clone(),
                        ValidationBinding {
                            contract_local: true,
                            mutable: true,
                            ty: ty.clone(),
                            ownership,
                            availability: Availability::Available,
                            active_loans: BTreeSet::from([LoanId(17)]),
                            moved_places: BTreeMap::from([(
                                path.clone(),
                                Availability::MaybeMoved,
                            )]),
                            definitely_partial: BTreeSet::from([path]),
                        },
                    );
                }
                // Retain the original rich fixture verbatim as the first case.
                // Its partial metadata selects a read diagnostic before transfer.
                // The second case admits the read and isolates ownership transfer.
                if admitted {
                    for binding in outer.values_mut() {
                        binding.active_loans.clear();
                        binding.moved_places.clear();
                        binding.definitely_partial.clear();
                    }
                }
                let before = outer.clone();
                if hostile {
                    let root = if ensures {
                        function.result_id.clone()
                    } else {
                        function.params[0].id.clone()
                    };
                    let clause = if ensures {
                        &mut function.ensures[0]
                    } else {
                        &mut function.requires[0]
                    };
                    let ResolvedExprKind::Block { statements, .. } = &mut clause.kind else {
                        panic!("contract block")
                    };
                    let ResolvedStatement::Let { value, .. } = &mut statements[0] else {
                        panic!("local")
                    };
                    value.kind = ResolvedExprKind::Place(Place {
                        root,
                        projections: Vec::new(),
                    });
                }
                let clauses = if ensures {
                    &function.ensures
                } else {
                    &function.requires
                };
                let phase = if ensures { "ensures" } else { "requires" };
                let mut current = HirValidator::new(&program).unwrap();
                let mut old = current.clone();
                scope_checkpoint::Scope::prepare(outer.len()).unwrap();
                let persistent = scope_checkpoint::Scope::from_prepaid(outer.clone());
                let expected =
                    old.validate_contracts_owned_reference(&execution, clauses, &outer, phase);
                let actual =
                    current.validate_contracts_persistent(&execution, clauses, &persistent, phase);
                HirValidator::assert_validation_oracle(
                    &actual, &expected, &current, &old, &outer, &before, phase,
                );
                HirValidator::assert_scope_auxiliary_oracle(&current, &old);
                assert!(persistent.iter().eq(before.iter()));
                assert!(persistent.values().all(|value| value.contract_local));
                assert_eq!(
                    actual.as_ref().err().map(Diagnostic::json),
                    expected.as_ref().err().map(Diagnostic::json)
                );
                assert_eq!(actual.is_err(), hostile);
                if hostile && admitted {
                    assert!(actual
                        .unwrap_err()
                        .message
                        .contains("contract cannot transfer ownership into a local binding"));
                }
            }
        }
    }
}

#[test]
fn persistent_body_receiver_preserves_complete_saved_iterative_scope_on_every_corpus_function() {
    let program = program(crate::hir::iterative_validator_tests::SOURCE);
    for function in &program.functions {
        let execution = FunctionExecutionId::Monomorphic(function.id.clone());
        let mut old_scope = function
            .params
            .iter()
            .map(|param| {
                (
                    param.id.clone(),
                    ValidationBinding {
                        contract_local: false,
                        mutable: false,
                        ty: param.ty.clone(),
                        ownership: param.ownership,
                        availability: Availability::Available,
                        active_loans: BTreeSet::new(),
                        moved_places: BTreeMap::new(),
                        definitely_partial: BTreeSet::new(),
                    },
                )
            })
            .collect::<BTreeMap<_, _>>();
        scope_checkpoint::Scope::prepare(old_scope.len()).unwrap();
        let mut persistent = scope_checkpoint::Scope::from_prepaid(old_scope.clone());
        let mut current = HirValidator::new(&program).unwrap();
        let mut old = current.clone();
        let effects = function
            .effects
            .iter()
            .map(String::as_str)
            .collect::<BTreeSet<_>>();
        let expected = old.validate_expr_iterative_owned_reference(
            &execution,
            &function.body,
            &mut old_scope,
            "body",
            true,
            Some(effect_keys::Allowed::Borrowed(&effects)),
        );
        let actual = current.validate_expr_persistent(
            &execution,
            &function.body,
            &mut persistent,
            "body",
            true,
            Some(effect_keys::Allowed::Borrowed(&effects)),
        );
        let current_scope = persistent
            .iter()
            .map(|(id, binding)| (id.clone(), binding.clone()))
            .collect();
        HirValidator::assert_validation_oracle(
            &actual,
            &expected,
            &current,
            &old,
            &current_scope,
            &old_scope,
            function.id.as_str(),
        );
        HirValidator::assert_scope_auxiliary_oracle(&current, &old);
        actual.unwrap();
    }
}
