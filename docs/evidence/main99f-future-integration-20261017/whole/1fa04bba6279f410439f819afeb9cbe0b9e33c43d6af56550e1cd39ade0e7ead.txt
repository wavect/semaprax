//! Complete values, actual allocation through Drop, and explicit outward residual.
use super::{inline, original, Type};
use crate::bounded_output::with_limit_usage_refusal;
use crate::test_allocation_observer::measure;
use crate::workspace_graph::constructor_failure::Failure;
const LIMIT: usize = 8 * 1024 * 1024;

#[test]
fn complete_default_and_stub_diagnostics_move_inline_and_outward_conversion_stays_observed() {
    let mut program = crate::parse("module inline.defaults; @id(\"row\") record Row { @id(\"field.a\") a:i64, @id(\"field.z\") z:i64, } @id(\"main\") fn main()->i64 {0}", "inline-defaults.spx").unwrap();
    let mut declaration = program.types.remove(0);
    let mut types = vec![
        Type::Never,
        Type::Str,
        Type::SliceU8,
        Type::StringMap,
        Type::Bytes,
        Type::ArrayU8(4),
        Type::OnceFunction,
        Type::OnceFunctionI64,
        Type::OnceFunctionI64Pair,
        Type::MutFunctionI64,
        Type::Function {
            parameters: vec![Type::I64],
            result: Box::new(Type::I64),
        },
        Type::Named {
            name: "Absent.λ\0🙂".to_owned(),
            arguments: vec![],
        },
        Type::Named {
            name: "Row".to_owned(),
            arguments: vec![Type::I64],
        },
    ];
    for ty in types.drain(..) {
        let old = original::default_expr(&ty, &[], false).unwrap_err();
        let (((), physical), refused, _, first) = with_limit_usage_refusal(LIMIT, || {
            measure(|| {
                let error = inline::default_expr(&ty, &[], false).unwrap_err();
                assert!(matches!(error, Failure::One(_)));
                assert_eq!(error.diagnostics(), old.as_slice());
                drop(error);
            })
        });
        assert!(!refused && first.is_none());
        assert!(
            !physical.invalid
                && physical.allocated == 0
                && physical.freed == 0
                && physical.live == 0
        );
        // The legacy Vec and all serialization stay INSIDE the observer.
        let (((), outward), refused, _, first) = with_limit_usage_refusal(LIMIT, || {
            measure(|| {
                let error = inline::default_expr(&ty, &[], false)
                    .unwrap_err()
                    .into_legacy_unfunded();
                assert_eq!(error, old);
                assert_eq!(error[0].json(), old[0].json());
                drop(error);
            })
        });
        assert!(!refused && first.is_none());
        assert!(
            !outward.invalid
                && outward.allocated > 0
                && outward.allocated == outward.freed
                && outward.live == 0
        );
        let mut function = program.functions[0].clone();
        function.return_type = ty;
        let expected =
            super::super::import_stub_original::default_expr(&function, &[], false, false)
                .unwrap_err();
        let actual = super::super::import_stub::default_expr_inline(&function, &[], false, false)
            .unwrap_err();
        assert_eq!(actual.diagnostics(), expected.as_slice());
    }
    let nominal = Type::Named {
        name: "Row".to_owned(),
        arguments: vec![],
    };
    for case in 0..4 {
        if case == 1 {
            if let crate::ast::TypeDeclarationKind::Record { fields } = &mut declaration.kind {
                fields[0].name = "earlier.λ\0🙂".to_owned();
                fields[1].ty = Type::Str;
            }
        } else if case == 2 {
            declaration.kind = crate::ast::TypeDeclarationKind::Variant { cases: vec![] };
        } else if case == 3 {
            declaration.kind = crate::ast::TypeDeclarationKind::Resource { lifecycles: vec![] };
        }
        let declarations = [("Row", &declaration)];
        let old = original::default_expr(&nominal, &declarations, false);
        let (((), physical), refused, _, first) = with_limit_usage_refusal(LIMIT, || {
            measure(|| {
                let actual = inline::default_expr(&nominal, &declarations, false);
                match (&actual, &old) {
                    (Ok(actual), Ok(old)) => assert_eq!(actual, old),
                    (Err(actual), Err(old)) => assert_eq!(actual.diagnostics(), old.as_slice()),
                    _ => panic!("full default result differs"),
                }
                drop(actual);
            })
        });
        assert!(!refused && first.is_none());
        assert!(!physical.invalid && physical.allocated == physical.freed && physical.live == 0);
    }
}

#[test]
fn many_preserves_every_field_order_and_existing_backing_without_reclassification() {
    let diagnostics = vec![
        crate::diagnostic::Diagnostic::error(
            "SPX-G173",
            "first.λ\0🙂",
            crate::ast::Span::default(),
        )
        .at_path("first.spx")
        .with_help("help.λ"),
        crate::diagnostic::Diagnostic::io("SPX-G172", "second.🙂"),
    ];
    let expected = diagnostics;
    let ((), physical) = measure(|| {
        let diagnostics = expected.clone();
        let pointer = diagnostics.as_ptr();
        let capacity = diagnostics.capacity();
        let error = Failure::from(diagnostics);
        assert!(matches!(error, Failure::Many(_)));
        assert_eq!(error.diagnostics(), expected.as_slice());
        let outward = error.into_legacy_unfunded();
        assert_eq!(outward.as_ptr(), pointer);
        assert_eq!(outward.capacity(), capacity);
        assert_eq!(outward, expected);
        drop(outward);
    });
    assert!(
        !physical.invalid
            && physical.allocated > 0
            && physical.allocated == physical.freed
            && physical.live == 0
    );
}

#[test]
fn full_carrier_exact_short_zero_and_sticky_keep_first_receipt_without_legacy_offset() {
    let observe = |limit, old_route| {
        with_limit_usage_refusal(limit, || {
            measure(|| {
                if old_route {
                    let result = original::default_expr(&Type::Str, &[], false);
                    let code = result.as_ref().unwrap_err()[0].code;
                    drop(result);
                    code
                } else {
                    let result = inline::default_expr(&Type::Str, &[], false);
                    let code = result.as_ref().unwrap_err().diagnostics()[0].code;
                    drop(result);
                    code
                }
            })
        })
    };
    let ((code, physical), refused, paid, first) = observe(LIMIT, false);
    assert_eq!(code, "SPX-G173");
    assert!(!refused && first.is_none());
    assert!(!physical.invalid && physical.allocated == 0 && physical.live == 0);
    let ((old_code, old_physical), old_refused, old_paid, old_first) = observe(LIMIT, true);
    assert_eq!(old_code, code);
    assert!(!old_refused && old_first.is_none());
    assert!(
        !old_physical.invalid
            && old_physical.allocated > 0
            && old_physical.allocated == old_physical.freed
            && old_physical.live == 0
    );
    assert_eq!(
        paid,
        old_paid + std::mem::size_of::<Result<crate::ast::Expr, Failure>>()
    );
    for (route, exact) in [(false, paid), (true, old_paid)] {
        let ((code, physical), refused, used, first) = observe(exact, route);
        assert_eq!(code, "SPX-G173");
        assert!(!refused && first.is_none() && used == exact);
        assert!(!physical.invalid && physical.allocated == physical.freed && physical.live == 0);
        for limit in [0, exact - 1] {
            let ((code, physical), refused, _, first) = observe(limit, route);
            assert_eq!(code, "SPX-G171");
            assert!(refused && first.is_some());
            assert!(
                !physical.invalid && physical.allocated == physical.freed && physical.live == 0
            );
        }
    }
    let (((), physical), refused, paid, first) = with_limit_usage_refusal(0, || {
        measure(|| {
            let error = inline::default_expr(&Type::Str, &[], false).unwrap_err();
            let retry = inline::default_expr(&Type::I64, &[], false).unwrap_err();
            assert_eq!(error.diagnostics(), retry.diagnostics());
            drop(retry);
            drop(error);
        })
    });
    assert!(refused && paid == 0 && first.is_some());
    assert!(!physical.invalid && physical.allocated == physical.freed && physical.live == 0);
}

#[test]
fn live_core_constructor_chain_preserves_complete_old_overlay_and_full_drop() {
    use super::super::{borrowed_imports_original, private_layout, CoreProviderProofs};
    use crate::ast::program_source::ProgramSource;
    use crate::workspace_graph::{constructor_failure, index_authored};
    let sources = [
        "module inline.app; use function @id(\"helper\") from inline.provider as increment; @id(\"app.main\") fn main()->i64 {increment(41)}",
        "module inline.provider; @id(\"helper\") fn helper(value:i64)->i64 {value+1}",
    ];
    let mut programs = sources
        .iter()
        .map(|s| crate::parse(s, "inline-core.spx").unwrap())
        .collect::<Vec<_>>();
    for late_failure in [false, true] {
        if late_failure {
            programs[1].functions[0].return_type = Type::Str;
        }
        let authored = index_authored(&programs).unwrap();
        let registry = private_layout::registry::LayoutRegistry::new(&programs, &authored);
        let old =
            borrowed_imports_original::construct(&programs[0], &authored, &programs, &registry);
        let (((), old_physical), refused, _, first) = with_limit_usage_refusal(LIMIT, || {
            measure(|| {
                let result = borrowed_imports_original::construct(
                    &programs[0],
                    &authored,
                    &programs,
                    &registry,
                );
                match (&result, &old) {
                    (Ok(actual), Ok(expected)) => {
                        for (actual, expected) in
                            actual.functions().iter().zip(expected.functions().iter())
                        {
                            assert_eq!(actual, expected);
                        }
                        assert_eq!(actual.functions().len(), expected.functions().len());
                    }
                    (Err(actual), Err(expected)) => assert_eq!(actual, expected),
                    _ => panic!("independent old constructor differs"),
                }
                drop(result);
            })
        });
        assert!(!refused && first.is_none());
        assert!(
            !old_physical.invalid
                && old_physical.allocated == old_physical.freed
                && old_physical.live == 0
        );
        let (((), physical), refused, _, first) = with_limit_usage_refusal(LIMIT, || {
            measure(|| {
                let mut core = CoreProviderProofs::new(&authored, &programs).unwrap();
                let current = constructor_failure::construct_transient_core(
                    &programs[0],
                    &authored,
                    &programs,
                    &registry,
                    &mut core,
                );
                match (&current, &old) {
                    (Ok(current), Ok(old)) => {
                        assert!(current.borrowed().is_some());
                        let current = current.source();
                        assert_eq!(current.functions().len(), old.functions().len());
                        for (current, old) in current.functions().iter().zip(old.functions().iter())
                        {
                            assert_eq!(current, old);
                        }
                        assert_eq!(current.types().len(), old.types().len());
                        for (current, old) in current.types().iter().zip(old.types().iter()) {
                            assert_eq!(current, old);
                        }
                    }
                    (Err(current), Err(old)) => {
                        assert!(matches!(current, Failure::One(_)));
                        assert_eq!(current.diagnostics(), old.as_slice());
                    }
                    _ => panic!("complete core constructor result differs"),
                }
                drop(current);
                drop(core);
            })
        });
        assert!(!refused && first.is_none());
        assert!(!physical.invalid && physical.allocated == physical.freed && physical.live == 0);
    }
}

#[test]
fn complete_inline_sources_and_original_owners_have_exact_whole_file_inverses() {
    use sha2::{Digest, Sha256};
    let proof: serde_json::Value = serde_json::from_str(ci_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/docs/evidence/constructor-inline-failure-825/exact-source-inverses.json"
    )))
    .unwrap();
    let owners = [
        include_str!("../../../workspace_graph.rs"),
        include_str!("../defaults.rs"),
        include_str!("../import_stub.rs"),
        include_str!("../borrowed_imports.rs"),
        include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/tests/workspace/semantic_graph/source_inventory.rs"
        )),
    ];
    let originals = [
        ci_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/docs/evidence/constructor-inline-failure-825/0.before.txt"
        )),
        ci_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/docs/evidence/constructor-inline-failure-825/1.before.txt"
        )),
        ci_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/docs/evidence/constructor-inline-failure-825/2.before.txt"
        )),
        ci_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/docs/evidence/constructor-inline-failure-825/3.before.txt"
        )),
        ci_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/docs/evidence/constructor-inline-failure-825/4.before.txt"
        )),
    ];
    let records = proof["owners"].as_array().unwrap();
    assert_eq!(records.len(), owners.len());
    for ((record, current), original) in records.iter().zip(owners).zip(originals) {
        let current = crate::source_reflows_merge::constructor_complete_input(
            record["source"].as_str().unwrap(), current,
        );
        let current = current.as_str();
        let mut restored = current.to_owned();
        for edit in record["edits"].as_array().unwrap().iter().rev() {
            let after = edit["after"].as_str().unwrap();
            assert_eq!(restored.matches(after).count(), 1);
            restored = restored.replacen(after, edit["before"].as_str().unwrap(), 1);
        }
        assert_eq!(restored, original);
        assert_eq!(
            format!(
                "{:x}",
                crate::digest_hex::LowerHex(Sha256::digest(original.as_bytes()))
            ),
            record["before_sha256"].as_str().unwrap()
        );
    }
    let children = [
        include_str!("../../constructor_failure.rs"),
        include_str!("inline.rs"),
        include_str!("inline_fields.rs"),
        include_str!("../borrowed_imports/inline.rs"),
        include_str!("inline_tests.rs"),
    ];
    for (record, source) in proof["children"].as_array().unwrap().iter().zip(children) {
        let source = crate::source_reflows_merge::constructor_complete_input(
            record["source"].as_str().unwrap(), source,
        );
        assert_eq!(
            format!(
                "{:x}",
                crate::digest_hex::LowerHex(Sha256::digest(source.as_bytes()))
            ),
            record["sha256"].as_str().unwrap()
        );
    }
}
