//! Complete old receivers and actual caller-owned construction/receiving/Drop.
use super::*;
use crate::bounded_output::{active_remaining, reserve_active_required, with_limit_usage_refusal};
use crate::test_allocation_observer::{current_metrics, measure};

fn literal(file: &'static str) -> &'static str {
    file.split_once("const SOURCE: &str = r#\"")
        .unwrap()
        .1
        .split_once("\"#;")
        .unwrap()
        .0
}
pub(in crate::cleanup_plan::replay) fn fixtures() -> [ResolvedProgram; 3] {
    let replay = include_str!("../../replay_tests.rs");
    let try_source = replay
        .split_once("const TRY_SOURCE: &str = r#\"")
        .unwrap()
        .1
        .split_once("\"#;")
        .unwrap()
        .0;
    [
        literal(include_str!("../../nested_record_destructure_tests.rs")),
        literal(replay),
        try_source,
    ]
    .map(|source| crate::hir::resolve(&crate::parse(source, "classifier.spx").unwrap()).unwrap())
}

pub(super) fn payer<'a>(function: &'a ResolvedFunction, budget: &'a mut ReplayBudget) -> Payer<'a> {
    Payer {
        function,
        budget,
        ledger: ClassifierLedger::capture(),
    }
}
fn complete(
    program: &ResolvedProgram,
    function: &ResolvedFunction,
    saved: bool,
) -> (Result<(), Diagnostic>, usize, usize, usize) {
    let mut budget = ReplayBudget::new();
    let run = |budget: &mut ReplayBudget| {
        reserve_program_skeleton_work(program, std::iter::once(function), budget)?;
        validate_structure_with_budget(program, function, budget, None)
    };
    let result = if saved {
        with_saved_classifier(|| run(&mut budget))
    } else {
        run(&mut budget)
    };
    (
        result,
        budget.remaining,
        budget.skeleton_remaining,
        budget.skeleton_spent,
    )
}
#[test]
fn complete_original_nested_resource_receivers_preserve_diagnostics_and_work() {
    for program in fixtures() {
        for function in &program.functions {
            assert_eq!(
                complete(&program, function, true),
                complete(&program, function, false)
            );
            let mut changed = function.clone();
            if let Some(slot) = changed.cleanup_plan.slots.first_mut() {
                slot.storage_index = slot.storage_index.wrapping_add(1);
                assert_eq!(
                    complete(&program, &changed, true),
                    complete(&program, &changed, false)
                );
            }
            let mut changed = function.clone();
            changed.cleanup_plan.slots.pop();
            assert_eq!(
                complete(&program, &changed, true),
                complete(&program, &changed, false)
            );
        }
    }
}
#[test]
fn full_uncached_owned_classifier_layouts_match_complete_original_classifier() {
    for program in fixtures() {
        let index = program.declarations.classifier_uncached_for_test();
        let function = &program.functions[0];
        for item in &program.types {
            let ty = ResolvedType::Nominal {
                declaration: item.id.clone(),
                arguments: Vec::new(),
            };
            let expected = index.classifier_old_facts_for_test(&ty);
            let mut budget = ReplayBudget::new();
            assert_eq!(
                index
                    .classifier_compute_for_test(&ty, payer(function, &mut budget))
                    .unwrap(),
                expected
            );
            assert_eq!(
                flags(&index, function, &ty, &mut budget).unwrap(),
                index.type_flags(&ty)
            );
        }
        // Original generic declaration and every concrete function boundary
        // remain complete source receivers, including owned variant instances,
        // wrong arity and unsupported argument/substitution frontiers.
        let mut roots = Vec::new();
        for function in &program.functions {
            roots.push(function.return_type.clone());
            roots.extend(function.params.iter().map(|parameter| parameter.ty.clone()));
        }
        if let Some(declaration) = index.type_id("GenericChoice") {
            for arguments in [
                vec![ResolvedType::Bytes],
                vec![ResolvedType::I64],
                vec![ResolvedType::String],
                vec![ResolvedType::Bytes, ResolvedType::Bytes],
                vec![ResolvedType::TypeParameter {
                    owner: function.id.clone(),
                    index: 0,
                }],
            ] {
                roots.push(ResolvedType::Nominal {
                    declaration: declaration.clone(),
                    arguments,
                });
            }
        }
        for ty in roots {
            let expected = index.classifier_old_facts_for_test(&ty);
            let mut budget = ReplayBudget::new();
            assert_eq!(
                index
                    .classifier_compute_for_test(&ty, payer(function, &mut budget))
                    .unwrap(),
                expected
            );
        }
        for ty in [
            ResolvedType::ArrayU8(17),
            ResolvedType::Function {
                parameters: vec![ResolvedType::I64, ResolvedType::Bytes],
                result: Box::new(ResolvedType::I64),
            },
            ResolvedType::TypeParameter {
                owner: function.id.clone(),
                index: 0,
            },
        ] {
            let expected = index.classifier_old_facts_for_test(&ty);
            let mut budget = ReplayBudget::new();
            assert_eq!(
                index
                    .classifier_compute_for_test(&ty, payer(function, &mut budget))
                    .unwrap(),
                expected
            );
        }
    }
}
#[test]
fn hostile_cached_facts_and_declined_compound_census_keep_original_precedence() {
    let program = fixtures().into_iter().next().unwrap();
    let function = &program.functions[0];
    let mut index = program.declarations.classifier_uncached_for_test();
    let mut types = vec![ResolvedType::Nominal {
        declaration: program.types[0].id.clone(),
        arguments: Vec::new(),
    }];
    // Full owned-key fallback after the old bounded borrowed census declines.
    types.push(ResolvedType::Function {
        parameters: vec![ResolvedType::I64; 128],
        result: Box::new(ResolvedType::Never),
    });
    for ty in types {
        index.classifier_cache_for_test(
            &ty,
            crate::hir::TypeFacts {
                copy: false,
                contains_resource: true,
                sized: false,
                needs_drop: true,
                layout_key: "forged-layout-retains-priority".to_owned(),
            },
        );
        let expected = index.type_flags(&ty);
        let mut budget = ReplayBudget::new();
        assert_eq!(flags(&index, function, &ty, &mut budget).unwrap(), expected);
    }
}
#[test]
fn whole_classifier_exact_short_zero_and_sticky_receiving_drop_use_one_payer() {
    let program = fixtures().into_iter().next().unwrap();
    let index = program.declarations.classifier_uncached_for_test();
    let function = &program.functions[0];
    let ty = ResolvedType::Nominal {
        declaration: program.types[2].id.clone(),
        arguments: Vec::new(),
    };
    let expected = index.classifier_old_facts_for_test(&ty);
    let observe = |limit| {
        with_limit_usage_refusal(limit, || {
            measure(|| {
                let mut budget = ReplayBudget::new();
                let result = index.classifier_compute_for_test(&ty, payer(function, &mut budget));
                if crate::bounded_output::active_refused() {
                    assert!(result.is_err());
                    let before = active_remaining();
                    assert!(!reserve_active_required(0));
                    let start = current_metrics();
                    drop(index.classifier_compute_for_test(&ty, payer(function, &mut budget)));
                    let end = current_metrics();
                    assert_eq!(start.allocated, end.allocated);
                    assert_eq!(start.allocations, end.allocations);
                    assert_eq!(before, active_remaining());
                } else {
                    assert_eq!(result.as_ref().unwrap(), &expected);
                }
                drop(result);
                drop(budget);
            })
        })
    };
    let (((), wide), refused, used, _) = observe(usize::MAX);
    assert!(!refused && used > 0 && !wide.invalid && wide.live == 0);
    assert_eq!(wide.allocated, wide.freed);
    assert!(wide.allocated <= used && wide.highwater <= used);
    let (((), exact), refused, exact_used, _) = observe(used);
    assert!(!refused && exact_used == used && !exact.invalid && exact.live == 0);
    assert_eq!(exact.allocated, exact.freed);
    let (((), short), refused, _, first) = observe(used - 1);
    assert!(refused && first.is_some() && !short.invalid && short.live == 0);
    assert_eq!(short.allocated, short.freed);
    let (((), zero), refused, charged, first) = observe(0);
    assert!(refused && first.is_some() && charged == 0 && !zero.invalid && zero.live == 0);
    assert_eq!(zero.allocated, 0);
}
#[test]
fn classifier_current_invocation_binding_refuses_cross_invocation_before_allocation() {
    let program = fixtures().into_iter().next().unwrap();
    let function = &program.functions[0];
    let (((), observed), refused, used, _) = with_limit_usage_refusal(usize::MAX, || {
        measure(|| {
            let mut budget = ReplayBudget::new();
            budget.prepare_builder(function).unwrap();
            let mut held = payer(function, &mut budget);
            // Each invocation's TLS setup is outside the refusal observation. This
            // control proves the payer cannot migrate to that new invocation; it
            // does not claim that the TLS budget factory is funded by this payer.
            let ((), _, _, _) = with_limit_usage_refusal(0, || {
                let before = current_metrics();
                let result = held.bytes(1);
                assert!(result.is_err());
                drop(result);
                let after = current_metrics();
                assert_eq!(before.allocated, after.allocated);
                assert_eq!(before.allocations, after.allocations);
            });
            assert!(!reserve_active_required(0));
            drop(held);
            drop(budget);
        })
    });
    assert!(refused && used > 0 && !observed.invalid && observed.live == 0);
    assert_eq!(observed.allocated, observed.freed);
}
#[test]
fn ordinary_droppability_callback_preserves_full_old_semantic_errors_and_stops_on_physical_error() {
    for program in fixtures() {
        for item in &program.types {
            let ty = ResolvedType::Nominal {
                declaration: item.id.clone(),
                arguments: Vec::new(),
            };
            let old = crate::cleanup::type_needs_resource_cleanup(&program, &ty);
            let current = droppability::needs_drop(&program, &ty, |ty| {
                Ok(program.declarations.type_flags(ty))
            })
            .map_err(|failure| match failure {
                droppability::Failure::Semantic(text) => text,
                droppability::Failure::Physical(_) => panic!("semantic-only callback"),
            });
            assert_eq!(old, current);
            let function = &program.functions[0];
            let mut budget = ReplayBudget::new();
            budget.prepare_builder(function).unwrap();
            let mut calls = 0;
            let result = droppability::needs_drop(&program, &ty, |_| {
                calls += 1;
                Err(budget.budget_refusal(function, "classifier physical callback refusal", ""))
            });
            assert_eq!(calls, 1);
            assert!(matches!(result, Err(droppability::Failure::Physical(_))));
        }
    }
}

#[test]
fn full_owned_identity_substitution_and_admission_oracles_keep_every_payload() {
    for program in fixtures() {
        let index = &program.declarations;
        let function = &program.functions[0];
        let owner = &function.id;
        let mut types: Vec<_> = program
            .types
            .iter()
            .map(|item| ResolvedType::Nominal {
                declaration: item.id.clone(),
                arguments: Vec::new(),
            })
            .collect();
        if let Some(option) = index.type_id("Option") {
            types.push(ResolvedType::Nominal {
                declaration: option.clone(),
                arguments: vec![ResolvedType::String],
            });
        }
        if let Some(result) = index.type_id("Result") {
            types.push(ResolvedType::Nominal {
                declaration: result.clone(),
                arguments: vec![ResolvedType::String, ResolvedType::I64],
            });
        }
        types.push(ResolvedType::Function {
            parameters: vec![ResolvedType::I64; 128],
            result: Box::new(ResolvedType::I64),
        });
        types.push(ResolvedType::Function {
            parameters: vec![ResolvedType::TypeParameter {
                owner: owner.clone(),
                index: 0,
            }],
            result: Box::new(ResolvedType::TypeParameter {
                owner: owner.clone(),
                index: 1,
            }),
        });
        for ty in types {
            let mut budget = ReplayBudget::new();
            assert_eq!(
                index
                    .classifier_key_for_test(&ty, payer(function, &mut budget))
                    .unwrap(),
                Some(ty.identity_key())
            );
            for arguments in [
                vec![],
                vec![ResolvedType::Bytes],
                vec![ResolvedType::Bytes, ResolvedType::I64],
            ] {
                assert_eq!(
                    index
                        .classifier_substitute_for_test(
                            &ty,
                            owner,
                            &arguments,
                            payer(function, &mut budget)
                        )
                        .unwrap(),
                    crate::hir::substitute_type(&ty, owner, &arguments).ok()
                );
            }
            assert_eq!(
                index.classifier_admission_for_test(&ty),
                (
                    crate::hir::ResolvedImportResultKind::is_owned_container_type(&ty, index),
                    crate::hir::is_admitted_concrete_owned_byte_variant(index, &ty)
                )
            );
        }
    }
}

#[test]
fn complete_classifier_source_join_keeps_the_existing_payer_and_full_receiver_inventory() {
    let production = concat!(
        include_str!("../../../bounded_output/classifier_string.rs"),
        include_str!("../paid_type_facts.rs"),
        include_str!("droppability.rs"),
        include_str!("droppability/paid.rs"),
        include_str!("droppability/saved_paid.rs"),
        include_str!("droppability/key.rs"),
        include_str!("droppability/text.rs"),
        include_str!("../../../hir/declaration_index/flags/paid.rs"),
        include_str!("../../../hir/declaration_index/flags/paid/admission.rs"),
        include_str!("../../../hir/declaration_index/flags/paid/builtin.rs"),
        include_str!("../../../hir/declaration_index/flags/paid/compute.rs"),
        include_str!("../../../hir/declaration_index/flags/paid/key.rs"),
        include_str!("../../../hir/declaration_index/flags/paid/storage.rs"),
        include_str!("../../../hir/declaration_index/flags/paid/substitute.rs")
    );
    // Read every production child, not just the facade. Classifier construction
    // must use the existing explicit payer rather than mint a phase allowance.
    for forbidden in ["ReplayBudget::new(", "with_limit_usage(", "with_limit("] {
        assert!(
            !production.contains(forbidden),
            "new classifier authority: {forbidden}"
        );
    }
    let complete = [
        production,
        include_str!("../../../bounded_output/classifier_string/tests.rs"),
        include_str!("tests.rs"),
        include_str!("droppability/tests.rs"),
        include_str!("droppability/helper_tests.rs"),
        include_str!("droppability/root_tests.rs"),
        include_str!("droppability/key_tests.rs"),
        include_str!("zero_tests.rs"),
        include_str!("../physical_budget.rs"),
        include_str!("../../../cleanup.rs"),
        include_str!("../../../hir/declaration_index/flags.rs"),
        include_str!("../../../hir/nodes/import_result_type.rs"),
        include_str!("../../../hir/nodes/import_result_type/tests.rs"),
    ];
    for source in complete {
        assert!(!source.is_empty());
    }
    let routes = concat!(
        include_str!("../../replay.rs"),
        include_str!("../nested_shape.rs")
    );
    assert!(routes.contains("paid_type_facts::flags("));
    assert!(routes.contains("super::paid_type_facts::needs_drop("));
}

#[test]
fn paid_flag_fast_receivers_omit_only_unconstructed_storage_and_keep_actual_payer_sticky() {
    for program in fixtures() {
        let function = &program.functions[0];
        let mut types = vec![
            ResolvedType::I64,
            ResolvedType::Bool,
            ResolvedType::Bytes,
            ResolvedType::String,
            ResolvedType::Never,
        ];
        types.extend(
            program
                .types
                .iter()
                .filter(|item| item.type_parameters.is_empty())
                .map(|item| ResolvedType::Nominal {
                    declaration: item.id.clone(),
                    arguments: vec![],
                }),
        );
        for ty in &types {
            let mut original_budget = ReplayBudget::new();
            let mut current_budget = ReplayBudget::new();
            let original = program
                .declarations
                .type_flags_paid_original_receiver(ty, payer(function, &mut original_budget));
            let current = program
                .declarations
                .type_flags_paid(ty, payer(function, &mut current_budget));
            assert_eq!(current, original);
        }
        for ty in [ResolvedType::I64, ResolvedType::Bytes] {
            let (_, overflow, used, _) = with_limit_usage_refusal(usize::MAX, || {
                let mut budget = ReplayBudget::new();
                program
                    .declarations
                    .type_flags_paid(&ty, payer(function, &mut budget))
                    .unwrap()
            });
            assert!(!overflow);
            assert_eq!(used, 0);
            let (_, overflow, original_used, _) = with_limit_usage_refusal(usize::MAX, || {
                let mut budget = ReplayBudget::new();
                program
                    .declarations
                    .type_flags_paid_original_receiver(&ty, payer(function, &mut budget))
                    .unwrap()
            });
            assert!(!overflow);
            assert!(original_used > 0);
            let (_, overflow, _, _) = with_limit_usage_refusal(0, || {
                assert!(!reserve_active_required(1));
                let mut budget = ReplayBudget::new();
                let first = program
                    .declarations
                    .type_flags_paid(&ty, payer(function, &mut budget))
                    .unwrap_err();
                let retry = program
                    .declarations
                    .type_flags_paid(&ty, payer(function, &mut budget))
                    .unwrap_err();
                assert_eq!(first, retry);
                assert!(!reserve_active_required(0));
            });
            assert!(overflow);
            let (_, actual) = measure(|| {
                for _ in 0..256 {
                    let mut budget = ReplayBudget::new();
                    std::hint::black_box(
                        program
                            .declarations
                            .type_flags_paid(&ty, payer(function, &mut budget))
                            .unwrap(),
                    );
                }
            });
            assert!(!actual.invalid);
            assert_eq!(actual.live, 0);
            assert_eq!(actual.allocated, actual.freed);
        }
    }
}

#[test]
fn paid_flag_miss_keeps_complete_original_constructor_exact_short_and_drop() {
    let program = fixtures().into_iter().next().unwrap();
    let function = &program.functions[0];
    let ty = ResolvedType::Nominal {
        declaration: crate::hir::DeclarationId::new("unknown.classifier"),
        arguments: vec![],
    };
    let run = |old: bool| {
        let mut budget = ReplayBudget::new();
        if old {
            program
                .declarations
                .type_flags_paid_original_receiver(&ty, payer(function, &mut budget))
        } else {
            program
                .declarations
                .type_flags_paid(&ty, payer(function, &mut budget))
        }
    };
    let (actual, overflow, used, _) = with_limit_usage_refusal(usize::MAX, || run(false));
    assert!(!overflow && used > 0);
    let (original, overflow, original_used, _) = with_limit_usage_refusal(usize::MAX, || run(true));
    assert!(!overflow);
    assert_eq!(actual, original);
    assert_eq!(used, original_used);
    for limit in [0, used - 1, used] {
        let (actual, refused, used, receipt) = with_limit_usage_refusal(limit, || run(false));
        let (old, old_refused, old_used, old_receipt) =
            with_limit_usage_refusal(limit, || run(true));
        assert_eq!(actual, old);
        assert_eq!(
            (refused, used, receipt),
            (old_refused, old_used, old_receipt)
        );
    }
    let (_, actual) = measure(|| {
        for _ in 0..32 {
            std::hint::black_box(run(false).unwrap());
        }
    });
    assert!(!actual.invalid && actual.allocated > 0);
    assert_eq!(actual.live, 0);
    assert_eq!(actual.allocated, actual.freed);
}

/// The complete prior paid bool receiver, before the ordinary inventory fold.
/// It authenticates the same live account and only its original zero-work
/// fast paths can succeed on the literal zero-byte/zero-work owner.
pub(in crate::cleanup_plan::replay) fn original_fast_probe(
    program: &dyn crate::cleanup::ReplayContext,
    function: &ResolvedFunction,
    ty: &ResolvedType,
) -> Result<bool, Diagnostic> {
    let lease = allocation::authority(program, function)
        .map_err(|_| Diagnostic::io_static("SPX-H006", "old probe invocation refused"))?
        .ok_or_else(|| Diagnostic::io_static("SPX-H006", "old probe requires its live account"))?;
    let mut budget = ReplayBudget::with_skeleton_limit_oracle(MAX_REPLAY_WORK_UNITS, lease);
    let mut payer = Payer {
        function,
        budget: &mut budget,
        ledger: ClassifierLedger::oracle_marker(),
    };
    match droppability::needs_drop_paid(program, ty, &mut payer) {
        Ok(value) => Ok(value),
        Err(droppability::Failure::Physical(error)) => Err(error),
        Err(droppability::Failure::Semantic(message)) => Err(droppability::replay_diagnostic(
            function, &message, &mut payer,
        )?),
    }
}

#[test]
fn recursive_flags_match_complete_uncached_owned_oracles_and_exact_short_sticky_payer() {
    for program in fixtures() {
        let function = &program.functions[0];
        let index = program.declarations.classifier_uncached_for_test();
        let mut types = vec![
            ResolvedType::Never,
            ResolvedType::Unit,
            ResolvedType::I64,
            ResolvedType::Bytes,
            ResolvedType::String,
            ResolvedType::ArrayU8(1),
            ResolvedType::Function {
                parameters: vec![ResolvedType::I64],
                result: Box::new(ResolvedType::I64),
            },
            ResolvedType::Function {
                parameters: vec![ResolvedType::Never],
                result: Box::new(ResolvedType::I64),
            },
        ];
        for item in &program.types {
            for arguments in [
                vec![],
                vec![ResolvedType::I64],
                vec![ResolvedType::String],
                vec![ResolvedType::U8],
                vec![ResolvedType::Never],
                vec![ResolvedType::I64, ResolvedType::Bool],
            ] {
                types.push(ResolvedType::Nominal {
                    declaration: item.id.clone(),
                    arguments,
                });
            }
        }
        for ty in types {
            let expected = index
                .classifier_old_facts_for_test(&ty)
                .as_ref()
                .map(|facts| {
                    (
                        facts.copy,
                        facts.contains_resource,
                        facts.sized,
                        facts.needs_drop,
                    )
                });
            let project = |flags: Option<crate::hir::TypeFlags>| {
                flags.map(|f| (f.copy, f.contains_resource, f.sized, f.needs_drop))
            };
            assert_eq!(project(index.type_flags(&ty)), expected, "unpaid {ty:?}");
            let run = || {
                let mut budget = ReplayBudget::new();
                index.type_flags_paid(&ty, payer(function, &mut budget))
            };
            let (actual, refused, used, _) = with_limit_usage_refusal(usize::MAX, run);
            assert!(!refused);
            assert_eq!(project(actual.unwrap()), expected, "paid {ty:?}");
            let mut old_budget = ReplayBudget::new();
            assert_eq!(
                project(
                    index
                        .type_flags_paid_original_receiver(&ty, payer(function, &mut old_budget))
                        .unwrap()
                ),
                expected,
                "old paid {ty:?}"
            );
            if used > 0 {
                let (exact, refused, exact_used, _) = with_limit_usage_refusal(used, run);
                assert!(!refused);
                assert_eq!(exact_used, used);
                assert_eq!(project(exact.unwrap()), expected);
                // Invalid types keep their independent semantic None above.
                // Budget refusal is checked on admitted complete values.
                if expected.is_some() {
                    for limit in [0, used - 1] {
                        let (_, refused, _, receipt) = with_limit_usage_refusal(limit, || {
                            let mut budget = ReplayBudget::new();
                            let first = index.type_flags_paid(&ty, payer(function, &mut budget));
                            assert!(
                                matches!(first, Err(_) | Ok(None)),
                                "short payer cannot produce admitted flags for {ty:?}"
                            );
                            let held = crate::bounded_output::active_remaining();
                            assert!(crate::bounded_output::active_refused());
                            let retry = index
                                .type_flags_paid(&ty, payer(function, &mut budget))
                                .unwrap_err();
                            assert_eq!(retry.code, "SPX-H006");
                            assert_eq!(
                                crate::bounded_output::active_remaining(),
                                held,
                                "retry cannot consume more bytes or replace the first refusal"
                            );
                        });
                        assert!(refused);
                        assert!(receipt.is_some());
                    }
                }
            }
        }
    }
}
