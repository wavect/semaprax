//! Owning controls: complete old values, real name work and refusal custody.
use super::*;
use crate::ast::{Span, TypeParameterDeclaration, VariantCaseDeclaration};
use crate::bounded_output::{active_refused, with_limit_usage_refusal};
use crate::test_allocation_observer::{current_metrics, measure};
use crate::workspace_graph::constructor_failure::receiving::Setup;

fn named(name: &str) -> Type {
    Type::Named {
        name: name.into(),
        arguments: Vec::new(),
    }
}
fn fixture(kind: usize, missing: bool) -> TypeDeclaration {
    let mut declaration = super::super::borrowed_tests::record();
    let TypeDeclarationKind::Record { mut fields } = declaration.kind else {
        unreachable!()
    };
    let mut second = fields[0].clone();
    fields[0].ty = named("T");
    second.ty = named("Target");
    fields.push(second.clone());
    second.ty = if missing {
        named("Missing")
    } else {
        Type::Bool
    };
    fields.push(second);
    declaration.kind = match kind {
        0 => TypeDeclarationKind::Record { fields },
        1 => {
            let mut method = crate::parse(
                "module method.fixture; @id(\"method\") fn method(x:i64)->i64{x}",
                "method.spx",
            )
            .unwrap()
            .functions
            .remove(0);
            method.params[0].ty = named("Target");
            method.return_type = if missing {
                named("Missing")
            } else {
                named("T")
            };
            TypeDeclarationKind::Class {
                fields,
                methods: vec![method],
            }
        }
        2 => TypeDeclarationKind::Variant {
            cases: vec![VariantCaseDeclaration {
                stable_id: "case".into(),
                explicit_id: true,
                name: "Case".into(),
                name_span: declaration.name_span,
                fields,
                span: declaration.span,
            }],
        },
        _ => unreachable!(),
    };
    declaration
}
#[test]
fn typed_declaration_rewrite_preserves_complete_old_record_class_variant_order_and_six_fields() {
    let (caller, programs) = super::super::borrowed_tests::contexts();
    for kind in 0..3 {
        for missing in [false, true] {
            let original = fixture(kind, missing);
            let mut old = original.clone();
            let expected = super::super::original::rewrite_declaration_with_layout(
                &mut old,
                "fixture.provider",
                &caller,
                &programs,
                None,
            );
            let ((), refused, _, first) = with_limit_usage_refusal(1 << 24, || {
                let ((), whole) = measure(|| {
                    let mut current = original.clone();
                    let mut owner = Receiver::acquire().unwrap();
                    let result = rewrite_declaration_with_layout(
                        &mut current,
                        "fixture.provider",
                        &caller,
                        &programs,
                        None,
                        &mut owner,
                    );
                    assert_eq!(current, old);
                    match (result, &expected) {
                        (Ok(()), Ok(())) => {}
                        (Err(Failure::One(actual)), Err(errors)) => {
                            assert_eq!(errors.len(), 1);
                            assert_eq!(actual, errors[0]);
                            assert_eq!(actual.json(), errors[0].json());
                        }
                        pair => panic!("old/current declaration selection differs: {pair:?}"),
                    }
                    drop(current);
                    drop(owner);
                });
                assert!(!whole.invalid && whole.live == 0);
                assert_eq!(whole.allocated, whole.freed);
            });
            assert!(!refused && first.is_none());
        }
    }
}
#[test]
fn typed_declaration_parameter_spill_keeps_exact_old_sort_lookup_work_and_original_borrows() {
    for count in [0, 1, 16, 17, 129, 4096] {
        let parameters: Vec<_> = (0..count)
            .rev()
            .map(|index| TypeParameterDeclaration {
                name: format!("P{index:06}"),
                span: Span::default(),
            })
            .collect();
        let old = parameter_names::BorrowedNames::new(&parameters).unwrap();
        // The old baseline has exactly the same input and lookup sequence.
        let (_, refused, _, first) = with_limit_usage_refusal(1 << 24, || {
            let mut owner = Receiver::acquire().unwrap();
            let current = parameter_names::receiving::new(&parameters, &mut owner).unwrap();
            for name in ["P000000", "P999999", "", "雪"] {
                assert_eq!(current.contains(name), old.contains(name));
            }
            let actual = parameter_names::receiving::observation(&current);
            let expected = parameter_names::receiving::observation(&old);
            assert_eq!(actual.0, parameters.as_ptr());
            assert_eq!(actual, expected);
            drop(current);
            drop(owner);
        });
        assert!(!refused && first.is_none());
    }
}
#[test]
fn typed_declaration_every_byte_frontier_stops_acquisition_and_preserves_original_first_setup() {
    let (caller, programs) = super::super::borrowed_tests::contexts();
    let original = fixture(0, false);
    let run = |extra| {
        with_limit_usage_refusal(Receiver::setup_bytes().unwrap() + extra, || {
            let ((), whole) = measure(|| {
                let mut current = original.clone();
                let mut owner = Receiver::acquire().unwrap();
                let result = rewrite_declaration_with_layout(
                    &mut current,
                    "fixture.provider",
                    &caller,
                    &programs,
                    None,
                    &mut owner,
                );
                if active_refused() {
                    let stopped = current_metrics().allocated;
                    let first = owner.reserve(0).unwrap_err();
                    let retry = rewrite_declaration_with_layout(
                        &mut current,
                        "fixture.provider",
                        &caller,
                        &programs,
                        None,
                        &mut owner,
                    );
                    assert!(matches!(retry, Err(Failure::Setup(setup)) if setup == first));
                    assert_eq!(current_metrics().allocated, stopped);
                    assert!(matches!(result, Err(Failure::Setup(Setup::Budget { .. }))));
                    drop(retry);
                } else {
                    result.unwrap();
                }
                drop(current);
                drop(owner);
            });
            assert!(!whole.invalid && whole.live == 0);
            assert_eq!(whole.allocated, whole.freed);
        })
    };
    let (_, refused, complete, first) = run(1 << 20);
    assert!(!refused && first.is_none());
    let subject = complete - Receiver::setup_bytes().unwrap();
    for extra in 0..=subject {
        let (_, refused, _, first) = run(extra);
        assert_eq!(refused, extra < subject);
        assert_eq!(first.is_some(), refused);
    }
}
#[test]
fn typed_declaration_foreign_restored_owner_keeps_unchanged_ast_and_sticky_detachment() {
    let (caller, programs) = super::super::borrowed_tests::contexts();
    let original = fixture(0, false);
    let (_, refused, _, first) = with_limit_usage_refusal(1 << 24, || {
        let mut current = original.clone();
        let mut owner = Receiver::acquire().unwrap();
        let (_, child_refused, used, child_first) = with_limit_usage_refusal(1 << 20, || {
            let (result, observed) = measure(|| {
                rewrite_declaration_with_layout(
                    &mut current,
                    "fixture.provider",
                    &caller,
                    &programs,
                    None,
                    &mut owner,
                )
            });
            assert!(matches!(result, Err(Failure::Setup(Setup::Detached))));
            assert_eq!(observed.allocated, 0);
        });
        assert!(!child_refused && child_first.is_none() && used == 0);
        let (result, observed) = measure(|| {
            rewrite_declaration_with_layout(
                &mut current,
                "fixture.provider",
                &caller,
                &programs,
                None,
                &mut owner,
            )
        });
        assert!(matches!(result, Err(Failure::Setup(Setup::Detached))));
        assert_eq!(observed.allocated, 0);
        assert_eq!(current, original);
        drop(owner);
    });
    assert!(!refused && first.is_none());
}

#[test]
fn typed_declaration_complete_old_receivers_actual_callers_and_full_source_inverses_stay_bound() {
    use sha2::{Digest, Sha256};
    let rows: serde_json::Value = serde_json::from_str(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/docs/evidence/constructor-declaration-rewrite-825/source-inverses.json"
    )))
    .unwrap();
    let sources = [
        include_str!("../../generic_type_import.rs"),
        include_str!("../parameter_names.rs"),
        include_str!("../parameter_names/receiving.rs"),
        include_str!("../receiving.rs"),
        include_str!("tests.rs"),
        include_str!("../../constructor_failure/receiving/typed.rs"),
        include_str!("../../expected_projection/borrowed_imports/typed.rs"),
        include_str!("../../../../tests/workspace/semantic_graph/source_inventory.rs"),
    ];
    let manifest = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    assert_eq!(rows.as_array().unwrap().len(), sources.len());
    for (row, source) in rows.as_array().unwrap().iter().zip(sources) {
        let projected = crate::source_reflows_merge::consumer_input("decl825", row["source"].as_str().unwrap(), source);
        let source = projected.as_str();
        assert_eq!(
            format!(
                "{:x}",
                crate::digest_hex::LowerHex(Sha256::digest(source.as_bytes()))
            ),
            row["current_sha256"].as_str().unwrap()
        );
        let mut restored = source.to_owned();
        for edit in row["edits"].as_array().unwrap() {
            let new = edit["new"].as_str().unwrap();
            assert_eq!(restored.matches(new).count(), 1);
            restored = restored.replacen(new, edit["before"].as_str().unwrap(), 1);
        }
        let original = std::fs::read(manifest.join(row["archive"].as_str().unwrap())).unwrap();
        assert_eq!(restored.as_bytes(), original);
        assert_eq!(
            format!(
                "{:x}",
                crate::digest_hex::LowerHex(Sha256::digest(&original))
            ),
            row["original_sha256"].as_str().unwrap()
        );
    }
    let old_generic = include_str!("../original.rs");
    assert!(old_generic.contains("pub(super) fn rewrite_declaration_with_layout("));
    assert!(old_generic.contains("for ty in declaration_types_mut(declaration)"));
    assert!(
        sources[6]
            .matches("generic_type_import::receiving::rewrite_declaration_with_layout(")
            .count()
            == 2
    );
    for forbidden in [
        "with_limit_usage_refusal",
        "ClassifierLedger::capture",
        "vec![",
        "graph_error(",
    ] {
        assert!(!sources[3].contains(forbidden));
    }
}
