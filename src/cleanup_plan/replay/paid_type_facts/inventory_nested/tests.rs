use super::*;
use crate::test_allocation_observer::{measure, Metrics};
const SOURCE: &str = include_str!("../../../../../tests/fixtures/owned-nested-outcome.spx");
fn fixture() -> ResolvedProgram {
    crate::hir::resolve(&crate::parse(SOURCE, "nested-paid.spx").unwrap()).unwrap()
}
fn nominal(id: &str, arguments: Vec<ResolvedType>) -> ResolvedType {
    ResolvedType::Nominal {
        declaration: DeclarationId::new(id),
        arguments,
    }
}
fn run(
    limit: usize,
    work: usize,
    program: &ResolvedProgram,
    ty: &ResolvedType,
    expected: Option<bool>,
) -> ((bool, bool, usize), Metrics, bool, usize) {
    let ((outcome, observed), refused, paid) =
        crate::bounded_output::with_limit_usage(limit, || {
            measure(|| {
                let function = &program.functions[0];
                let mut budget = ReplayBudget::new();
                let result = budget
                    .charge(function, 0, "nested paid receiver setup")
                    .and_then(|()| {
                        budget.remaining = work;
                        admitted(
                            &program.declarations,
                            ty,
                            &mut super::super::tests::payer(function, &mut budget),
                        )
                    });
                let matched =
                    expected.is_none_or(|old| result.as_ref().is_ok_and(|actual| *actual == old));
                let failed = result.is_err();
                let used_work = work.saturating_sub(budget.remaining);
                if crate::bounded_output::active_refused() {
                    let before = crate::test_allocation_observer::current_metrics();
                    let retry = admitted(
                        &program.declarations,
                        ty,
                        &mut super::super::tests::payer(function, &mut budget),
                    );
                    assert!(retry.is_err());
                    assert_eq!(
                        before.allocated,
                        crate::test_allocation_observer::current_metrics().allocated
                    );
                    drop(retry);
                }
                drop(result);
                drop(budget);
                (matched, failed, used_work)
            })
        });
    (outcome, observed, refused, paid)
}
#[test]
fn full_nested_receiver_arguments_case_order_exact_work_backing_and_receiving_drop() {
    let program = fixture();
    let mut roots = vec![
        nominal("n.Outcome", vec![]),
        nominal("missing", vec![]),
        ResolvedType::Bytes,
    ];
    for count in [1, 17] {
        roots.push(nominal(
            "n.Outcome",
            vec![nominal("n.Payload", vec![ResolvedType::Bytes]); count],
        ));
    }
    for root in roots {
        let expected =
            crate::hir::collection_outcome::nested::admitted(&program.declarations, &root);
        let (outcome, observed, refused, paid) = run(
            usize::MAX,
            MAX_REPLAY_WORK_UNITS,
            &program,
            &root,
            Some(expected),
        );
        assert!(outcome.0 && !outcome.1 && !refused && !observed.invalid);
        assert!(observed.allocated <= paid);
        assert_eq!(observed.live, 0);
        assert_eq!(observed.allocated, observed.freed);
        let (exact, observed, refused, used) =
            run(paid, outcome.2, &program, &root, Some(expected));
        assert!(exact.0 && !exact.1 && !refused && !observed.invalid);
        assert_eq!(used, paid);
        assert_eq!(observed.live, 0);
        for (bytes, work) in [
            (0, MAX_REPLAY_WORK_UNITS),
            (paid - 1, MAX_REPLAY_WORK_UNITS),
            (paid, 0),
            (paid, outcome.2 - 1),
        ] {
            let (short, observed, _, used) = run(bytes, work, &program, &root, None);
            assert!(short.1 && !observed.invalid);
            assert!(observed.allocated <= used);
            assert_eq!(observed.live, 0);
            assert_eq!(observed.allocated, observed.freed);
        }
    }
}
#[test]
fn borrowed_full_payload_field_identity_and_type_match_complete_original() {
    let program = fixture();
    let function = &program.functions[0];
    let container = nominal("n.Outcome", vec![]);
    let payload = nominal("n.Payload", vec![]);
    for (case, field) in [
        ("n.Outcome.ready", "n.Outcome.value"),
        ("n.Outcome.error", "n.Outcome.value"),
        ("foreign", "n.Outcome.value"),
        ("n.Outcome.ready", "n.Payload.items"),
    ] {
        let case = DeclarationId::new(case);
        let field = DeclarationId::new(field);
        for ty in [
            &payload,
            &nominal("n.Payload", vec![ResolvedType::Bytes]),
            &ResolvedType::Bytes,
        ] {
            let expected = crate::hir::collection_outcome::nested::record_field(
                &program.declarations,
                &container,
                &case,
                &field,
                ty,
            );
            crate::bounded_output::with_limit(usize::MAX, || {
                let mut budget = ReplayBudget::new();
                budget.charge(function, 0, "nested field receiver").unwrap();
                assert_eq!(
                    record_field(
                        &program.declarations,
                        &container,
                        &case,
                        &field,
                        ty,
                        &mut super::super::tests::payer(function, &mut budget)
                    )
                    .unwrap(),
                    expected
                );
            });
        }
    }
}
#[test]
fn held_metadata_root_suffix_zero_one_short_and_sticky_work_keep_original_queries() {
    let program = fixture();
    let index = &program.declarations;
    let function = &program.functions[0];
    for text in [
        "n.Outcome",
        "n.Config",
        "Config",
        "Outcome",
        "missing",
        "é\0🦀suffix",
    ] {
        let key = DeclarationId::new(text);
        // Key construction is outside this query-only allocation observation;
        // the production caller must use the genuine paid complete constructor.
        let full_key = nominal(text, vec![]).identity_key();
        let expected = (
            index.declaration(&key),
            index.has_record_invariant(&key),
            index.cached_zero_argument_facts(&key),
            index.type_id(text),
            index.cached_zero_argument_facts(&key),
        );
        let (work, refused) = crate::bounded_output::with_limit(usize::MAX, || {
            let mut budget = ReplayBudget::new();
            budget.charge(function, 0, "metadata receiver").unwrap();
            let before = budget.remaining;
            let mut payer = super::super::tests::payer(function, &mut budget);
            let value = metadata::declared(index, &key, &mut payer).unwrap();
            let present = metadata::has_invariant(index, &key, &mut payer).unwrap();
            let cached = metadata::cached_identity(index, &full_key, &mut payer).unwrap();
            let named = metadata::named_type(index, text, &mut payer).unwrap();
            let borrowed = metadata::cached_zero_argument(index, &key, &mut payer).unwrap();
            assert_eq!((value, present, cached, named, borrowed), expected);
            before - payer.budget.remaining
        });
        assert!(!refused);
        for limit in [work, work - 1, 0] {
            // Each finite control starts a distinct held budget; never revive
            // a refused payer or transfer an unused work allowance to it.
            let ((failed, observed), _, paid) =
                crate::bounded_output::with_limit_usage(usize::MAX, || {
                    measure(|| {
                        let mut budget = ReplayBudget::new();
                        budget
                            .charge(function, 0, "metadata finite receiver")
                            .unwrap();
                        budget.remaining = limit;
                        let mut payer = super::super::tests::payer(function, &mut budget);
                        let result =
                            metadata::declared(index, &key, &mut payer).and_then(|value| {
                                metadata::has_invariant(index, &key, &mut payer).and_then(
                                    |present| {
                                        metadata::cached_identity(index, &full_key, &mut payer)
                                            .and_then(|cached| {
                                                metadata::named_type(index, text, &mut payer)
                                                    .and_then(|named| {
                                                        metadata::cached_zero_argument(
                                                            index, &key, &mut payer,
                                                        )
                                                        .map(|borrowed| {
                                                            (
                                                                value, present, cached, named,
                                                                borrowed,
                                                            )
                                                        })
                                                    })
                                            })
                                    },
                                )
                            });
                        let failed = result.is_err();
                        if limit == work {
                            assert_eq!(result.as_ref().unwrap(), &expected);
                        } else {
                            assert!(failed);
                            let before = crate::test_allocation_observer::current_metrics();
                            let retry = metadata::declared(index, &key, &mut payer);
                            assert!(retry.is_err());
                            assert_eq!(
                                before.allocated,
                                crate::test_allocation_observer::current_metrics().allocated
                            );
                            drop(retry);
                        }
                        drop(result);
                        drop(budget);
                        failed
                    })
                });
            assert_eq!(failed, limit < work);
            assert!(!observed.invalid && observed.allocated <= paid);
            assert_eq!(observed.live, 0);
            assert_eq!(observed.allocated, observed.freed);
        }
    }
}
#[test]
fn complete_original_nested_and_index_sources_stay_independent_and_byte_exact() {
    assert_eq!(
        ci_str!("../../../../hir/collection_outcome/nested.rs"),
        ci_str!("../../../../../docs/evidence/schema-summary/nested.rs.before")
    );
    let nominal: serde_json::Value = serde_json::from_str(ci_str!(
        "../../../../../docs/evidence/nominal-payload-827/exact-source-inverses-activation-v3.json"
    ))
    .unwrap();
    let row = nominal
        .as_array()
        .unwrap()
        .iter()
        .find(|row| row["source"] == "src/hir/declaration_index.rs")
        .unwrap();
    let mut projected = ci_str!("../../../../hir/declaration_index.rs").to_owned();
    // Later paid construction modules and test-only legacy insertion helpers
    // are exact named additions to the v3 nominal activation projection.
    for registration in [
        "mod nominal_construction;\nmod nominal_paid_values;\nmod nominal_paid_inheritance;\n",
        "    #[cfg(test)]\n    fn insert_field(",
        "    #[cfg(test)]\n    fn insert_case_field(",
    ] {
        assert_eq!(projected.matches(registration).count(), 1);
        let prior = registration.replace("    #[cfg(test)]\n", "");
        let prior = if registration.starts_with("mod ") {
            ""
        } else {
            &prior
        };
        projected = projected.replacen(registration, prior, 1);
    }
    for edit in row["edits"].as_array().unwrap() {
        let new = edit["new"].as_str().unwrap();
        assert_eq!(
            projected.matches(new).count(),
            1,
            "complete current nominal index source"
        );
        projected = projected.replacen(new, edit["before"].as_str().unwrap(), 1);
    }
    assert_eq!(projected,ci_str!("../../../../../docs/evidence/nominal-payload-827/original/src/hir/declaration_index.rs"));
    assert_eq!(
        projected
            .replacen("pub(crate) use flags::{inventory_identity, inventory_substitute, InventoryPayer};\n", "", 1)
            .replacen(
            "mod inventory_lookup;\n",
            "",
            1
        ),
        ci_str!("../../../../../docs/evidence/schema-summary/declaration_index.rs.before")
    );
}

#[test]
fn full_forged_cycle_missing_arity_and_case_metadata_keep_original_refusal_order() {
    let original = fixture();
    let root = nominal("n.Outcome", vec![]);
    for index in original.declarations.inventory_nested_forgeries_for_test() {
        let expected = crate::hir::collection_outcome::nested::admitted(&index, &root);
        assert!(!expected);
        crate::bounded_output::with_limit(usize::MAX, || {
            let function = &original.functions[0];
            let mut budget = ReplayBudget::new();
            budget
                .charge(function, 0, "forged nested receiver setup")
                .unwrap();
            assert_eq!(
                admitted(
                    &index,
                    &root,
                    &mut super::super::tests::payer(function, &mut budget)
                )
                .unwrap(),
                expected
            );
        });
    }
}

#[test]
fn full_native_authored_admission_first_lookup_order_and_forged_metadata_keep_paid_drop() {
    let program = fixture();
    let function = &program.functions[0];
    let indexes = std::iter::once(program.declarations.clone())
        .chain(program.declarations.inventory_nested_forgeries_for_test());
    for index in indexes {
        let mut roots = vec![
            ResolvedType::Bytes,
            ResolvedType::I64,
            nominal("n.Outcome", vec![]),
            nominal("n.Config", vec![]),
        ];
        for width in [0, 1, 17] {
            roots.push(nominal(
                "n.Outcome",
                vec![
                    ResolvedType::Function {
                        parameters: vec![ResolvedType::Bytes; width],
                        result: Box::new(ResolvedType::I64),
                    };
                    width
                ],
            ));
        }
        if let Some(id) = index.type_id("Option") {
            roots.push(ResolvedType::Nominal {
                declaration: id.clone(),
                arguments: vec![ResolvedType::String],
            });
        }
        if let Some(id) = index.type_id("Result") {
            roots.push(ResolvedType::Nominal {
                declaration: id.clone(),
                arguments: vec![ResolvedType::String, ResolvedType::I64],
            });
            roots.push(ResolvedType::Nominal {
                declaration: id.clone(),
                arguments: vec![ResolvedType::Bytes, ResolvedType::Bytes],
            });
        }
        for ty in roots {
            let expected = index.classifier_admission_for_test(&ty);
            let query = |payer: &mut Payer<'_>| {
                let native = metadata::admission::native_owned(&index, &ty, payer)?;
                metadata::admission::authored_byte_variant(&index, &ty, payer)
                    .map(|authored| (native, authored))
            };
            let (work, refused) = crate::bounded_output::with_limit(usize::MAX, || {
                let mut budget = ReplayBudget::new();
                budget
                    .charge(function, 0, "admission full receiver setup")
                    .unwrap();
                let before = budget.remaining;
                assert_eq!(
                    query(&mut super::super::tests::payer(function, &mut budget)).unwrap(),
                    expected
                );
                before - budget.remaining
            });
            assert!(!refused && work > 0);
            for limit in [work, work - 1, 0] {
                let ((failed, observed), _, paid) =
                    crate::bounded_output::with_limit_usage(usize::MAX, || {
                        measure(|| {
                            let mut budget = ReplayBudget::new();
                            budget
                                .charge(function, 0, "admission finite receiver setup")
                                .unwrap();
                            budget.remaining = limit;
                            let result =
                                query(&mut super::super::tests::payer(function, &mut budget));
                            let failed = result.is_err();
                            if limit == work {
                                assert_eq!(result.as_ref().unwrap(), &expected);
                            } else {
                                assert!(failed);
                                let before = crate::test_allocation_observer::current_metrics();
                                let retry =
                                    query(&mut super::super::tests::payer(function, &mut budget));
                                assert!(retry.is_err());
                                assert_eq!(
                                    before.allocated,
                                    crate::test_allocation_observer::current_metrics().allocated
                                );
                                drop(retry);
                            }
                            drop(result);
                            drop(budget);
                            failed
                        })
                    });
                assert_eq!(failed, limit < work);
                assert!(!observed.invalid && observed.allocated <= paid);
                assert_eq!(observed.live, 0);
                assert_eq!(observed.allocated, observed.freed);
            }
        }
    }
    assert_eq!(
        include_str!("../../../../hir/declaration_index/flags/paid/admission.rs"),
        include_str!("../../../../../docs/evidence/schema-summary/flags-paid-admission.rs.before")
    );
}
