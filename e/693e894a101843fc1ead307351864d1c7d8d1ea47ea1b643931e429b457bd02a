//! Exact inert-root driver parity and real payer/allocation controls.
use super::*;
use crate::bounded_output::with_limit_usage_refusal;
use crate::test_allocation_observer::measure;
use sha2::{Digest, Sha256};
const LIMIT: usize = 64 * 1024 * 1024;

fn program() -> ResolvedProgram {
    crate::hir::resolve(
        &crate::parse(
            "module move.root; @id(\"main\") fn main()->i64 {0}",
            std::path::Path::new("move-root.spx"),
        )
        .unwrap(),
    )
    .unwrap()
}
fn expression(program: &ResolvedProgram, kind: ResolvedExprKind) -> ResolvedExpr {
    let mut value = program.functions[0].body.clone();
    value.kind = kind;
    value
}
fn seed() -> scope_checkpoint::Scope {
    let mut scope = scope_checkpoint::Scope::default();
    scope
        .insert(
            ValueId::new("move.owner".to_owned()),
            ValidationBinding {
                contract_local: false,
                mutable: true,
                ty: ResolvedType::String,
                ownership: OwnershipMode::Own,
                availability: Availability::Available,
                active_loans: BTreeSet::new(),
                moved_places: BTreeMap::new(),
                definitely_partial: BTreeSet::new(),
            },
        )
        .unwrap();
    scope
}
fn call(
    validator: &HirValidator<'_>,
    expression: &ResolvedExpr,
    scope: &mut dyn ScopeWrite,
    old: bool,
) -> Result<(), Diagnostic> {
    if old {
        validator.mark_value_sources_moved_before_inert_root(expression, scope)
    } else {
        validator.mark_value_sources_moved(expression, scope)
    }
}
fn same(left: &Result<(), Diagnostic>, right: &Result<(), Diagnostic>) {
    // Diagnostic derives full-field equality only for tests.
    assert_eq!(left, right);
}
fn inert_roots(program: &ResolvedProgram) -> Vec<ResolvedExpr> {
    let child = expression(
        program,
        ResolvedExprKind::Place(Place {
            root: ValueId::new("move.owner".to_owned()),
            projections: Vec::new(),
        }),
    );
    [
        ResolvedExprKind::Int(1),
        ResolvedExprKind::Int32(1),
        ResolvedExprKind::Char(0x3bb),
        ResolvedExprKind::Uint8(255),
        ResolvedExprKind::Usize(1),
        ResolvedExprKind::Float32(0x7fc00001),
        ResolvedExprKind::Float64(0x7ff8000000000001),
        ResolvedExprKind::Bool(true),
        ResolvedExprKind::String("owned".to_owned()),
        ResolvedExprKind::ArrayU8(vec![0, 255]),
        ResolvedExprKind::RepeatArrayU8 { value: 3, count: 2 },
        ResolvedExprKind::FunctionReference {
            target: DeclarationId::new("missing"),
        },
        ResolvedExprKind::Closure {
            parameters: Vec::new(),
            captures: Vec::new(),
            body: Box::new(child.clone()),
        },
        // Forged payloads are deliberate: the complete old source-move pass
        // never interprets constructor children or grants validity to them.
        ResolvedExprKind::ConstructRecord {
            record: DeclarationId::new("missing"),
            fields: Vec::new(),
        },
        ResolvedExprKind::ConstructVariant {
            variant: DeclarationId::new("missing"),
            case: DeclarationId::new("missing.case"),
            fields: Vec::new(),
        },
    ]
    .into_iter()
    .map(|kind| expression(program, kind))
    .collect()
}

#[test]
fn complete_predecessor_and_exact_inert_shape_set_remain_independent() {
    let before = include_str!("before.rs")
        .replacen("use super::super::super::*;", "use super::super::*;", 1)
        .replacen(
            "fn mark_value_sources_moved_before_inert_root(",
            "fn mark_value_sources_moved(",
            1,
        );
    assert_eq!(
        format!(
            "{:x}",
            crate::digest_hex::LowerHex(Sha256::digest(before.as_bytes()))
        ),
        "08b3d1a43078fcc73d6ba8514a093d5bdd4b7a5b00378bfd20a8a9a67b75e464"
    );
    let current = super::direct_tests::before_source();
    let start = current
        .find("        // The original walker does nothing")
        .unwrap();
    let end = current[start..].find("        enum Frame<'a> {").unwrap() + start;
    let mut restored = current.to_owned();
    restored.replace_range(start..end, "");
    restored = restored.replacen("        #[cfg(test)]\n        if tests::before_selected() {\n            return self.mark_value_sources_moved_before_inert_root(expression, scope);\n        }\n", "", 1);
    let tail = restored.find("\n// One real primitive site,").unwrap();
    restored.truncate(tail);
    assert_eq!(restored, before, "every original driver byte survives");
    let original = before
        .split_once("                    ResolvedExprKind::Int(_)")
        .unwrap()
        .1
        .split_once(" => {}")
        .unwrap()
        .0;
    let selected = current[start..end]
        .split_once("            | ResolvedExprKind::Int(_)")
        .unwrap()
        .1
        .split_once(" => return Ok(())")
        .unwrap()
        .0;
    let compact = |text: &str| text.split_whitespace().collect::<String>();
    assert_eq!(compact(original), compact(selected));
    assert!(current[start..end].contains(
        "ResolvedExprKind::Closure { .. }\n            | ResolvedExprKind::FunctionReference { .. }"
    ));
}

#[test]
fn inert_roots_keep_exact_scope_and_legacy_owned_route_including_forged_payloads() {
    let program = program();
    let validator = HirValidator::new(&program).unwrap();
    for mut expression in inert_roots(&program) {
        for hostile in [false, true] {
            if hostile {
                expression.ty = ResolvedType::Never;
                expression.ownership = OwnershipMode::Borrow;
                expression.id = ExpressionId::from_owned("forged\0expression".to_owned());
            }
            let mut current = seed();
            let mut old = current.clone();
            let snapshot = current.clone();
            same(
                &call(&validator, &expression, &mut current, false),
                &call(&validator, &expression, &mut old, true),
            );
            assert_eq!(current, snapshot);
            assert_eq!(old, snapshot);
            let mut legacy = snapshot
                .iter()
                .map(|(id, value)| (id.clone(), value.clone()))
                .collect::<BTreeMap<_, _>>();
            let prior = legacy.clone();
            let (outcome, refused, used, _) =
                with_limit_usage_refusal(0, || call(&validator, &expression, &mut legacy, false));
            outcome.unwrap();
            assert!(!refused);
            assert_eq!(used, 0, "legacy oracle takes its original route");
            assert_eq!(legacy, prior);
        }
    }
}

#[test]
fn propagating_place_block_if_invoke_and_missing_projection_keep_old_results_and_scope() {
    let program = program();
    let validator = HirValidator::new(&program).unwrap();
    let place = expression(
        &program,
        ResolvedExprKind::Place(Place {
            root: ValueId::new("move.owner".to_owned()),
            projections: Vec::new(),
        }),
    );
    let scalar = expression(&program, ResolvedExprKind::Int(0));
    let roots = [
        place.clone(),
        expression(
            &program,
            ResolvedExprKind::Block {
                statements: Vec::new(),
                tail: Box::new(place.clone()),
            },
        ),
        expression(
            &program,
            ResolvedExprKind::If {
                condition: Box::new(scalar.clone()),
                then_branch: Box::new(place.clone()),
                else_branch: Box::new(scalar.clone()),
            },
        ),
        expression(
            &program,
            ResolvedExprKind::Invoke {
                callable: Box::new(scalar),
                args: vec![place.clone()],
            },
        ),
        expression(
            &program,
            ResolvedExprKind::Project {
                base: Box::new(place.clone()),
                field: DeclarationId::new("unused-by-move-pass"),
            },
        ),
        expression(
            &program,
            ResolvedExprKind::Upcast {
                source: Box::new(place.clone()),
            },
        ),
        expression(
            &program,
            ResolvedExprKind::VecConstructor {
                element: ResolvedType::I64,
                count: 0,
                statements: Vec::new(),
                tail: Box::new(place.clone()),
            },
        ),
        expression(
            &program,
            ResolvedExprKind::LexicalExit {
                kind: crate::ast::LexicalExitKind::Return,
                target: crate::hir::lexical_exit::ControlTarget::Function(
                    FunctionExecutionId::Monomorphic(program.functions[0].id.clone()),
                ),
                value: Some(Box::new(place.clone())),
            },
        ),
        expression(
            &program,
            ResolvedExprKind::Match {
                mode: ResolvedMatchMode::Value,
                scrutinee: Box::new(place.clone()),
                arms: vec![ResolvedMatchArm {
                    pattern: ResolvedMatchPattern::Wildcard,
                    guard: None,
                    span: place.span,
                    value: place.clone(),
                }],
            },
        ),
        expression(
            &program,
            ResolvedExprKind::Place(Place {
                root: ValueId::new("move.owner".to_owned()),
                projections: vec![PlaceProjection::Field(DeclarationId::new("missing.field"))],
            }),
        ),
    ];
    for expression in roots {
        let mut current = seed();
        let mut old = current.clone();
        let expected = call(&validator, &expression, &mut old, true);
        let actual = call(&validator, &expression, &mut current, false);
        same(&actual, &expected);
        assert_eq!(current, old);
    }
}

#[test]
fn inert_exact_short_zero_floor_and_sticky_refusal_never_take_the_scope() {
    let program = program();
    let validator = HirValidator::new(&program).unwrap();
    let expression = expression(&program, ResolvedExprKind::String("new".to_owned()));
    for (limit, floor, success) in [
        (INERT_ROOT_CONTROLS, 0, true),
        (INERT_ROOT_CONTROLS - 1, 0, false),
        (0, 0, false),
        (INERT_ROOT_CONTROLS, 1, false),
    ] {
        let mut scope = seed();
        let snapshot = scope.clone();
        let ((outcome, allocations), refused, used, first) =
            with_limit_usage_refusal(limit, || {
                if floor != 0 {
                    assert!(crate::bounded_output::set_active_floor(floor));
                }
                measure(|| call(&validator, &expression, &mut scope, false))
            });
        assert_eq!(outcome.is_ok(), success);
        assert_eq!(refused, !success);
        assert_eq!(first.is_none(), success);
        if let Some(first) = first {
            assert_eq!(first.requested, INERT_ROOT_CONTROLS);
        }
        assert_eq!(used, if success { INERT_ROOT_CONTROLS } else { 0 });
        assert_eq!(scope, snapshot);
        assert_eq!(allocations.allocations, 0);
        assert_eq!(allocations.live, 0);
        assert!(!allocations.invalid);
        if !success {
            same(
                &outcome,
                &Err(hir_error_static(
                    "ownership scope checkpoint exceeds the active validation budget",
                )),
            );
        }
    }
    let mut scope = seed();
    let snapshot = scope.clone();
    let ((first, retry), refused, used, receipt) =
        with_limit_usage_refusal(INERT_ROOT_CONTROLS, || {
            assert!(!crate::bounded_output::reserve_active_required(
                INERT_ROOT_CONTROLS + 1
            ));
            let first = call(&validator, &expression, &mut scope, false);
            let retry = call(&validator, &expression, &mut scope, false);
            (first, retry)
        });
    assert!(refused);
    assert_eq!(used, 0);
    assert!(first.is_err());
    assert_eq!(receipt.unwrap().requested, INERT_ROOT_CONTROLS + 1);
    same(&first, &retry);
    assert_eq!(scope, snapshot);
}

#[test]
fn tiny_and_repeated_actual_driver_payers_and_allocation_intervals_include_complete_drop() {
    let program = program();
    let validator = HirValidator::new(&program).unwrap();
    let expression = expression(&program, ResolvedExprKind::String("new".to_owned()));
    for count in [1, 64, 1024] {
        for old in [true, false] {
            let ((used, refused), allocations) = measure(|| {
                let mut scope = seed();
                let snapshot = scope.clone();
                let (outcome, refused, used, _) = with_limit_usage_refusal(LIMIT, || {
                    for _ in 0..count {
                        call(&validator, &expression, &mut scope, old)?;
                    }
                    Ok::<(), Diagnostic>(())
                });
                outcome.unwrap();
                assert_eq!(scope, snapshot);
                drop(snapshot);
                drop(scope);
                (used, refused)
            });
            assert!(!refused);
            assert!(!allocations.invalid);
            assert_eq!(allocations.live, 0);
            if !old {
                assert_eq!(used, count * INERT_ROOT_CONTROLS);
            }
            eprintln!(
                "OPT913 root-driver old={old} count={count} paid={used} allocations={allocations:?} complete_drop=true; no application or RSS claim"
            );
        }
    }
}

#[test]
fn live_overlapping_loan_keeps_complete_original_refusal_and_error_publication() {
    let program = program();
    let mut validator = HirValidator::new(&program).unwrap();
    let place = Place {
        root: ValueId::new("move.owner".to_owned()),
        projections: Vec::new(),
    };
    let expression = expression(&program, ResolvedExprKind::Place(place.clone()));
    // The helper's complete immediate oracle sees the same canonical lookup.
    // This isolated fixture does not assert admission of an attached loan plan.
    validator.canonical_loans = canonical_loans::Lookup::Legacy(canonical_loans::Inventory {
        ids: BTreeMap::new(),
        liveness: BTreeMap::from([(
            (expression.id.clone(), LoanPointPhase::Before, LoanId(0)),
            0,
        )]),
        origins: vec![place.clone()],
    });
    let mut current = seed();
    current
        .get_mut(&place.root)
        .unwrap()
        .unwrap()
        .active_loans
        .insert(LoanId(0));
    let mut old = current.clone();
    let expected = call(&validator, &expression, &mut old, true);
    let actual = call(&validator, &expression, &mut current, false);
    same(&actual, &expected);
    assert!(actual.unwrap_err().message.contains("SPX-T265"));
    assert_eq!(current, old);
    assert_eq!(
        current.len(),
        0,
        "original failed-driver publication remains unchanged"
    );
}

#[rustfmt::skip]
#[test]
fn propagating_driver_keeps_each_original_payer_cut_after_the_new_selector() {
    super::direct_tests::before(|| {
    let program = program();
    let validator = HirValidator::new(&program).unwrap();
    let expression = expression(
        &program,
        ResolvedExprKind::Place(Place {
            root: ValueId::new("absent".to_owned()),
            projections: Vec::new(),
        }),
    );
    let mut scope = seed();
    let (result, refused, old_paid, _) =
        with_limit_usage_refusal(LIMIT, || call(&validator, &expression, &mut scope, true));
    result.unwrap();
    assert!(!refused);
    for cut in [0, 1, old_paid / 2, old_paid - 1, old_paid] {
        let mut current = seed();
        let mut old = current.clone();
        let (expected, old_refused, old_used, _) =
            with_limit_usage_refusal(cut, || call(&validator, &expression, &mut old, true));
        let (actual, refused, used, _) =
            with_limit_usage_refusal(cut + INERT_ROOT_CONTROLS, || {
                call(&validator, &expression, &mut current, false)
            });
        same(&actual, &expected);
        assert_eq!(refused, old_refused);
        assert_eq!(used, old_used + INERT_ROOT_CONTROLS);
        assert_eq!(current, old);
    }
    });
}

thread_local! {
    static BEFORE: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}
pub(super) fn before_selected() -> bool {
    BEFORE.with(std::cell::Cell::get)
}
fn with_before<T>(old: bool, run: impl FnOnce() -> T) -> T {
    struct Restore(bool);
    impl Drop for Restore {
        fn drop(&mut self) {
            BEFORE.with(|value| value.set(self.0));
        }
    }
    let _restore = Restore(BEFORE.with(|value| value.replace(old)));
    run()
}

#[test]
fn ordinary_function_callers_report_complete_old_current_bills_and_drop_without_gain_assumption() {
    // The unmodified library remains the whole workload for both validators;
    // executable resolution additionally requires this ordinary checked entry.
    let decimal = format!(
        "{}\n@id(\"move.decimal.main\") fn main()->i64 {{let value=add(\"1\",\"2\");string_len(value)}}\n",
        include_str!("../../../../../std/int-decimal/src/decimal.spx")
    );
    for source in [
        "module move.callers; @id(\"relay\") fn relay(s:string)->string {s} @id(\"main\") fn main()->i64 {let x=relay(\"owned\"); string_len(x)}",
        decimal.as_str(),
    ] {
        let program = crate::hir::resolve(
            &crate::parse(source, std::path::Path::new("move-callers.spx")).unwrap(),
        )
        .unwrap();
        let mut results = Vec::new();
        for old in [true, false] {
            let ((outcome, refused, paid, _), allocations) = measure(|| {
                with_limit_usage_refusal(LIMIT, || {
                    with_before(old, || {
                        let mut validator = HirValidator::new(&program)?;
                        for function in &program.functions {
                            validator.validate_function(
                                function,
                                &FunctionExecutionId::Monomorphic(function.id.clone()),
                            )?;
                        }
                        Ok::<(), Diagnostic>(())
                    })
                })
            });
            assert!(!allocations.invalid);
            assert_eq!(allocations.live, 0);
            eprintln!(
                "OPT913 complete_function_chain old={old} source_bytes={} paid={paid} refused={refused} allocations={allocations:?}; no presumed net or CLI49 admission",
                source.len()
            );
            results.push((outcome, refused));
        }
        assert_eq!(results[0], results[1]);
        results[0].0.as_ref().unwrap();
    }
    let caught = std::panic::catch_unwind(|| with_before(true, || panic!("test selection unwind")));
    assert!(caught.is_err());
    assert!(!before_selected());
}

#[test]
fn unchanged_propagating_take_unwind_releases_the_complete_owned_scope() {
    struct PanicTake(scope_checkpoint::Scope);
    impl ScopeRead for PanicTake {
        fn get(&self, id: &ValueId) -> Option<&ValidationBinding> {
            self.0.get(id)
        }
        fn len(&self) -> usize {
            self.0.len()
        }
        fn iter(&self) -> scope_checkpoint::views::Entries<'_> {
            self.0.iter()
        }
    }
    impl ScopeWrite for PanicTake {
        fn publication_target(&mut self) -> scope_checkpoint::views::Target<'_> {
            self.0.publication_target()
        }
        fn get_mut(&mut self, id: &ValueId) -> Result<Option<&mut ValidationBinding>, Diagnostic> {
            self.0.get_mut(id)
        }
        fn insert(&mut self, id: ValueId, value: ValidationBinding) -> Result<(), Diagnostic> {
            self.0.insert(id, value)
        }
        fn take_owned(&mut self) -> scope_checkpoint::Owned {
            let _taken = self.0.take_owned();
            // Inject an owned unwind payload without the process panic hook retaining backing.
            std::panic::resume_unwind(Box::new("owning take unwind"));
        }
        fn replace_owned(&mut self, value: scope_checkpoint::Owned) {
            self.0.replace_owned(value);
        }
    }
    let program = program();
    let validator = HirValidator::new(&program).unwrap();
    let expression = expression(
        &program,
        ResolvedExprKind::Place(Place {
            root: ValueId::new("move.owner".to_owned()),
            projections: Vec::new(),
        }),
    );
    for old in [true, false] {
        let ((), allocations) = measure(|| {
            let mut scope = PanicTake(seed());
            let caught = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                call(&validator, &expression, &mut scope, old)
            }));
            assert!(caught.is_err());
            assert_eq!(
                caught.as_ref().unwrap_err().downcast_ref::<&str>(),
                Some(&"owning take unwind")
            );
            assert_eq!(scope.len(), 0);
            drop(caught);
            drop(scope);
        });
        assert!(!allocations.invalid);
        assert_eq!(allocations.live, 0);
    }
}
