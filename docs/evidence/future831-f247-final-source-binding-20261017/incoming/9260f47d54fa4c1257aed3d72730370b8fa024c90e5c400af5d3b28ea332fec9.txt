use super::*;
use crate::bounded_output::with_limit_usage_refusal;
use crate::test_allocation_observer::measure;

fn program() -> ResolvedProgram {
    crate::hir::resolve(&crate::check(
        r#"module test.constructor.intrinsics; fn list_identity(value:List<i64>)->List<i64>{value} fn iter_identity(value: own Iter<i64>)->Iter<i64>{value} @id("app.main") fn main()->i64{0}"#,
        "constructor-intrinsics.spx",
    ).unwrap()).unwrap()
}

#[test]
fn exact_intrinsic_nominal_predicates_match_complete_owned_receivers_without_acquisition() {
    use crate::hir::CompilerDeclaration as C;
    for (key, text) in [
        (C::List, crate::list_ops::LIST_ID),
        (C::ListStep, crate::list_ops::STEP_ID),
        (C::Iter, crate::iterator_ops::ITER_ID),
        (C::IterStep, crate::iterator_ops::STEP_ID),
    ] {
        assert_eq!(DeclarationId::compiler(key).as_str(), text);
        assert_eq!(DeclarationId::compiler(key), DeclarationId::new(text));
    }
    let nominal = |id: &str, arguments| ResolvedType::Nominal {
        declaration: DeclarationId::new(id),
        arguments,
    };
    let mut inputs = vec![
        ResolvedType::I64,
        ResolvedType::Never,
        crate::list_ops::resolved_list(),
        crate::list_ops::resolved_step(),
        crate::iterator_ops::resolved_iter(ResolvedType::I64),
        crate::iterator_ops::resolved_iter_step(ResolvedType::I64),
        nominal(crate::list_ops::LIST_ID, vec![]),
        nominal(crate::list_ops::LIST_ID, vec![ResolvedType::I32]),
        nominal(
            crate::list_ops::LIST_ID,
            vec![ResolvedType::I64, ResolvedType::I64],
        ),
        nominal("λ.list\0", vec![ResolvedType::I64]),
        nominal(
            crate::list_ops::STEP_ID,
            vec![ResolvedType::TypeParameter {
                owner: DeclarationId::new("foreign.λ"),
                index: 0,
            }],
        ),
    ];
    let mut nested = ResolvedType::I64;
    for _ in 0..64 {
        nested = nominal(crate::list_ops::LIST_ID, vec![nested]);
    }
    inputs.push(nested);
    for ty in inputs {
        let expected = (
            crate::list_ops::original_constructor::is_list(&ty),
            crate::list_ops::original_constructor::is_step(&ty),
            crate::iterator_ops::original_constructor::is_iter(&ty),
            crate::iterator_ops::original_constructor::is_step(&ty),
        );
        let ((actual, physical), refused, paid, first) = with_limit_usage_refusal(0, || {
            measure(|| {
                (
                    crate::list_ops::is_list(&ty),
                    crate::list_ops::is_step(&ty),
                    crate::iterator_ops::is_iter(&ty),
                    crate::iterator_ops::is_step(&ty),
                )
            })
        });
        assert_eq!(actual, expected);
        assert!(!refused && first.is_none() && paid == 0);
        assert!(
            !physical.invalid
                && physical.allocated == 0
                && physical.freed == 0
                && physical.live == 0
        );
    }
}

fn mutate(program: &mut ResolvedProgram, owner: &str, case: usize) {
    let cases = program
        .declarations
        .variant_cases
        .get_mut(&DeclarationId::new(owner))
        .unwrap();
    match case {
        0 => {}
        1 => {
            cases[1].fields[0].ty = ResolvedType::TypeParameter {
                owner: DeclarationId::new(owner),
                index: 1,
            }
        }
        2 => {
            cases[1].fields[0].ty = ResolvedType::TypeParameter {
                owner: DeclarationId::new("foreign.λ"),
                index: 0,
            }
        }
        3 => cases[1].fields[1].ty = ResolvedType::I64,
        4 => {
            let ResolvedType::Nominal { arguments, .. } = &mut cases[1].fields[1].ty else {
                panic!("canonical tail");
            };
            arguments.push(ResolvedType::I64);
        }
        5 => {
            let ResolvedType::Nominal { declaration, .. } = &mut cases[1].fields[1].ty else {
                panic!("canonical tail");
            };
            *declaration = DeclarationId::new("foreign.tail.λ");
        }
        6 => cases[0].name.push_str(".forged"),
        7 => cases[1].index = 0,
        8 => cases[1].id = DeclarationId::new("foreign.case.λ"),
        9 => {
            let field = cases[1].fields[0].clone();
            cases[0].fields.push(field);
        }
        10 => {
            let duplicate = cases[1].clone();
            cases.push(duplicate);
        }
        11 => cases[1].fields[1].index = 0,
        12 => cases[1].fields[1].name.push_str(".forged"),
        13 => cases[1].fields[1].id = DeclarationId::new("foreign.field.λ"),
        14 => cases.clear(),
        15..=17 => {
            let ResolvedType::Nominal { arguments, .. } = &mut cases[1].fields[1].ty else {
                panic!("canonical tail");
            };
            if case == 17 {
                arguments.clear();
            } else {
                arguments[0] = ResolvedType::TypeParameter {
                    owner: DeclarationId::new(if case == 15 {
                        owner
                    } else {
                        "foreign.tail.owner.λ"
                    }),
                    index: if case == 15 { 1 } else { 0 },
                };
            }
        }
        18 => cases[1].fields.swap(0, 1),
        19 => cases[1].fields[0].name.push_str(".forged"),
        20 => cases[1].fields[0].id = DeclarationId::new("foreign.head.λ"),
        21 => cases[1].fields[0].index = 1,
        22 => {
            let extra = cases[1].fields[0].clone();
            cases[1].fields.push(extra);
        }
        23 => cases[0].id = DeclarationId::new("foreign.empty.case.λ"),
        24 => cases[0].index = 1,
        _ => unreachable!(),
    }
}

#[test]
fn complete_intrinsic_case_fields_and_constructor_dispatch_keep_full_original_diagnostics() {
    let base = program();
    for owner in [crate::list_ops::STEP_ID, crate::iterator_ops::STEP_ID] {
        for case in 0..25 {
            let mut subject = base.clone();
            mutate(&mut subject, owner, case);
            let old_list = crate::list_ops::original_constructor::validate_declarations(&subject);
            let old_iter =
                crate::iterator_ops::original_constructor::validate_declarations(&subject);
            assert_eq!(crate::list_ops::validate_declarations(&subject), old_list);
            assert_eq!(
                crate::iterator_ops::validate_declarations(&subject),
                old_iter
            );
            let old = original::ConstructorSeed::authenticate(&subject, None).err();
            let current = ConstructorSeed::authenticate(&subject, None).err();
            assert_eq!(current, old);
            if case == 0 {
                assert!(current.is_none());
            } else {
                assert_eq!(current.unwrap().code, "SPX-H006");
            }
            // Earlier NUL remains first even with the later hostile intrinsic shape.
            subject.entrypoint = DeclarationId::new("bad\0entry");
            let current = ConstructorSeed::authenticate(&subject, None).err().unwrap();
            assert_eq!(
                current,
                original::ConstructorSeed::authenticate(&subject, None)
                    .err()
                    .unwrap()
            );
            assert_eq!(
                current.message,
                "resolved entry point identity contains NUL"
            );
        }
    }
    // The public iterator receiver keeps the complete original element gate,
    // including non-I64 admitted and hostile generic/arity arguments.
    for element in [
        ResolvedType::I64,
        ResolvedType::Bytes,
        ResolvedType::String,
        ResolvedType::TypeParameter {
            owner: DeclarationId::new("foreign.λ"),
            index: 0,
        },
    ] {
        let ty = crate::iterator_ops::resolved_iter_step(element);
        assert_eq!(
            crate::iterator_ops::step_shape(&base.declarations, &ty),
            crate::iterator_ops::original_constructor::step_shape(&base.declarations, &ty)
        );
    }
    // Both hostile families keep the original iterator-before-list box ordering.
    let mut subject = base.clone();
    mutate(&mut subject, crate::list_ops::STEP_ID, 6);
    mutate(&mut subject, crate::iterator_ops::STEP_ID, 6);
    let diagnostic = ConstructorSeed::authenticate(&subject, None).err().unwrap();
    assert_eq!(
        diagnostic,
        original::ConstructorSeed::authenticate(&subject, None)
            .err()
            .unwrap()
    );
    assert_eq!(
        diagnostic.message,
        "iterator prelude declaration is not canonical"
    );
}

#[test]
fn complete_intrinsic_declaration_receivers_have_zero_backing_through_drop_and_keep_sticky_caller()
{
    let subject = program();
    let ((current, physical), refused, paid, first) = with_limit_usage_refusal(0, || {
        measure(|| {
            (
                crate::list_ops::validate_declarations(&subject),
                crate::iterator_ops::validate_declarations(&subject),
            )
        })
    });
    assert!(current.0.is_ok() && current.1.is_ok());
    assert!(!refused && paid == 0 && first.is_none());
    assert!(
        !physical.invalid && physical.allocated == 0 && physical.freed == 0 && physical.live == 0
    );
    let (old, old_physical) = measure(|| {
        (
            crate::list_ops::original_constructor::validate_declarations(&subject),
            crate::iterator_ops::original_constructor::validate_declarations(&subject),
        )
    });
    assert_eq!(current, old);
    assert!(!old_physical.invalid && old_physical.allocated > 0 && old_physical.live == 0);
    assert_eq!(old_physical.allocated, old_physical.freed);
    // Borrowed truth has no receipt authority. The actual constructor still
    // authenticates its payer and cannot publish a seed on zero/sticky refusal.
    let ((ok, physical), refused, paid, first) = with_limit_usage_refusal(0, || {
        measure(|| {
            let first = ConstructorSeed::authenticate(&subject, None);
            let second = ConstructorSeed::authenticate(&subject, None);
            let ok = (first.is_ok(), second.is_ok());
            drop(first);
            drop(second);
            ok
        })
    });
    assert_eq!(ok, (false, false));
    assert!(refused && paid == 0 && first.is_some());
    assert!(!physical.invalid && physical.live == 0 && physical.allocated == physical.freed);
}

#[test]
fn complete_intrinsic_sources_and_saved_constructor_dispatch_reconstruct_every_original_byte() {
    use sha2::{Digest, Sha256};
    let proof: serde_json::Value = serde_json::from_str(ci_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/docs/evidence/constructor-intrinsic-borrowing-824/source-provenance.json"
    )))
    .unwrap();
    let currents = [
        ci_str!("../../../list_ops.rs"),
        ci_str!("../../../iterator_ops.rs"),
        ci_str!("../constructor_seed.rs"),
        ci_str!("original.rs"),
        ci_str!("../function_lookup/paid_custody.rs"),
    ];
    let originals = [
        ci_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/docs/evidence/constructor-intrinsic-borrowing-824/src__list_ops.rs.before.txt"
        )),
        ci_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/docs/evidence/constructor-intrinsic-borrowing-824/src__iterator_ops.rs.before.txt"
        )),
        ci_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/docs/evidence/constructor-intrinsic-borrowing-824/src__hir__validation__constructor_seed.rs.before.txt"
        )),
        ci_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/docs/evidence/constructor-intrinsic-borrowing-824/src__hir__validation__constructor_seed__original.rs.before.txt"
        )),
        ci_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/docs/evidence/constructor-intrinsic-borrowing-824/src__hir__validation__function_lookup__paid_custody.rs.before.txt"
        )),
    ];
    let original_digests = [
        "0abc67a67c5c22b175b81e2ef82e46206421f6a974fa772ea40de40181467dd2",
        "8541bd6ab7e58bd7a39eae3eef770e6ac29f104a6ea6923aa73287d2b6a8ed44",
        "52c886c27a00527cca2468b308102608224fe80aa2a3e36ea5e20470b7ccf855",
        "e38ef154a4b8a0e2fb00d2256f8fced29d75e6c8a847984f8387caa1019a5b1f",
        "815fb8254c64f3dc68161b56be163d9fdeb6c2503d69da11b8c237a78eb8cdf9",
    ];
    for (record, digest) in proof["files"]
        .as_array()
        .unwrap()
        .iter()
        .zip(original_digests)
    {
        assert_eq!(record["sha256"].as_str().unwrap(), digest);
    }
    let saved = [
        ci_str!("../../../list_ops/original_constructor.rs"),
        ci_str!("../../../iterator_ops/original_constructor.rs"),
        ci_str!("original_intrinsics.rs"),
    ];
    let saved_digests = [
        "9289ebd9ab42c9861644ac88641f48c7b9d30b99bbaba8a9b785f9507e3cb7a5",
        "106d9f8a281a66a26633811571f80fc81949c70202be235fa4bc67c7cda85636",
        "68f522899feb21fa7d0e0db389ce2d7cc5742dcd360b7b4f9ec57d49efc866e0",
    ];
    for (source, digest) in saved.into_iter().zip(saved_digests) {
        assert_eq!(
            format!(
                "{:x}",
                crate::digest_hex::LowerHex(Sha256::digest(source.as_bytes()))
            ),
            digest
        );
    }
    for ((record, current), original) in proof["files"]
        .as_array()
        .unwrap()
        .iter()
        .zip(currents)
        .zip(originals)
    {
        assert_eq!(
            format!(
                "{:x}",
                crate::digest_hex::LowerHex(Sha256::digest(original.as_bytes()))
            ),
            record["sha256"].as_str().unwrap()
        );
        let mut restored = current.to_owned();
        for edit in record["edits"].as_array().unwrap().iter().rev() {
            let after = edit["after"].as_str().unwrap();
            assert_eq!(restored.matches(after).count(), 1);
            restored = restored.replacen(after, edit["before"].as_str().unwrap(), 1);
        }
        assert_eq!(restored, original);
    }
}

#[test]
fn original_intrinsic_parameter_mode_fixture_keeps_complete_parser_diagnostic() {
    let original = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/docs/evidence/intrinsic-controller-repair-85286/src__hir__validation__constructor_seed__intrinsic_tests.rs.before.txt"
    ));
    let source = original
        .split_once("r#\"")
        .unwrap()
        .1
        .split_once("\"#")
        .unwrap()
        .0;
    let diagnostics = crate::check(source, "constructor-intrinsics.spx").unwrap_err();
    assert_eq!(
        diagnostics,
        vec![crate::diagnostic::Diagnostic {
            code: "SPX-P106",
            severity: crate::diagnostic::Severity::Error,
            message: "expected `:` after parameter name".into(),
            path: Some("constructor-intrinsics.spx".to_owned()),
            span: Some(crate::ast::Span {
                start: 109,
                end: 114,
                line: 1,
                column: 110
            }),
            help: None,
        }]
    );
}

#[test]
fn corrective_controllers_reconstruct_complete_original_sources() {
    use sha2::{Digest, Sha256};
    let records: serde_json::Value = serde_json::from_str(ci_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/docs/evidence/intrinsic-controller-repair-85286/exact-source-inverses.json"
    )))
    .unwrap();
    let currents = [
        ci_str!("intrinsic_tests.rs"),
        ci_str!("../function_lookup/paid_custody.rs"),
    ];
    let originals = [
        ci_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/docs/evidence/intrinsic-controller-repair-85286/src__hir__validation__constructor_seed__intrinsic_tests.rs.before.txt")),
        ci_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/docs/evidence/intrinsic-controller-repair-85286/src__hir__validation__function_lookup__paid_custody.rs.before.txt")),
    ];
    let hashes = [
        "d42039a03f7191b9c89d879f942b0cc005b51e080d36020c5b79849c2fd042a3",
        "0b61582a54b3cfa01889e9e8965e3b1fceef8340489332f02ee9cb1c059db7cb",
    ];
    assert_eq!(records.as_array().unwrap().len(), currents.len());
    for (((record, current), original), digest) in records
        .as_array()
        .unwrap()
        .iter()
        .zip(currents)
        .zip(originals)
        .zip(hashes)
    {
        assert_eq!(
            format!(
                "{:x}",
                crate::digest_hex::LowerHex(Sha256::digest(original.as_bytes()))
            ),
            digest
        );
        let mut restored = current.to_owned();
        for edit in record["edits"].as_array().unwrap().iter().rev() {
            let after = edit["after"].as_str().unwrap();
            assert_eq!(restored.matches(after).count(), 1);
            restored = restored.replacen(after, edit["before"].as_str().unwrap(), 1);
        }
        assert_eq!(restored, original);
    }
}
