//! Fixed-message custody only; the compatibility error Vec remains observed.
use super::{default_expr, original, Type, TypeDeclaration, TypeDeclarationKind};
use crate::bounded_output::with_limit_usage_refusal;
use crate::test_allocation_observer::measure;
const LIMIT: usize = 8 * 1024 * 1024;
fn row() -> TypeDeclaration {
    let mut program = crate::parse("module errors.defaults; @id(\"row\") record Row { @id(\"field.a\") a:i64, @id(\"field.z\") z:i64, } @id(\"main\") fn main()->i64 {0}", "default-errors.spx").unwrap();
    program.types.remove(0)
}
fn named(name: &str, arguments: Vec<Type>) -> Type {
    Type::Named {
        name: name.to_owned(),
        arguments,
    }
}
#[test]
fn closed_errors_keep_full_old_diagnostics_and_remove_only_message_backing_through_drop() {
    let mut fixtures = vec![
        (Type::Never, None),
        (Type::Str, None),
        (Type::SliceU8, None),
        (Type::StringMap, None),
        (Type::ArrayU8(4), None),
        (Type::Bytes, None),
        (Type::OnceFunction, None),
        (Type::OnceFunctionI64, None),
        (Type::OnceFunctionI64Pair, None),
        (Type::MutFunctionI64, None),
        (
            Type::Function {
                parameters: vec![Type::I64],
                result: Box::new(Type::I64),
            },
            None,
        ),
        (named("Absent.λ\0🙂", vec![]), None),
        (named("Row", vec![Type::I64]), None),
    ];
    let mut empty_variant = row();
    empty_variant.kind = TypeDeclarationKind::Variant { cases: vec![] };
    fixtures.push((named("Row", vec![]), Some(empty_variant)));
    let mut resource = row();
    resource.kind = TypeDeclarationKind::Resource { lifecycles: vec![] };
    fixtures.push((named("Row", vec![]), Some(resource)));
    let mut late = row();
    if let TypeDeclarationKind::Record { fields } = &mut late.kind {
        fields[0].name = "earlier.λ\0🙂".to_owned();
        fields[1].ty = Type::Str;
    }
    fixtures.push((named("Row", vec![]), Some(late)));
    for (ty, declaration) in fixtures {
        let declarations = declaration
            .as_ref()
            .map(|d| vec![("Row", d)])
            .unwrap_or_default();
        let (old, refused, _, first) =
            with_limit_usage_refusal(LIMIT, || original::default_expr(&ty, &declarations, false));
        assert!(!refused && first.is_none());
        let old = old.unwrap_err();
        assert_eq!(old.len(), 1);
        assert!(old[0].message_owned_capacity() > 0);
        let (((), actual), refused, _, first) = with_limit_usage_refusal(LIMIT, || {
            measure(|| {
                let current = default_expr(&ty, &declarations, false).unwrap_err();
                assert_eq!(current, old);
                assert_eq!(current[0].message_owned_capacity(), 0);
                drop(current);
            })
        });
        assert!(!refused && first.is_none());
        assert!(!actual.invalid && actual.live == 0 && actual.allocated == actual.freed);
        let (((), old_actual), refused, _, first) = with_limit_usage_refusal(LIMIT, || {
            measure(|| {
                let result = original::default_expr(&ty, &declarations, false).unwrap_err();
                assert_eq!(result, old);
                drop(result);
            })
        });
        assert!(!refused && first.is_none());
        assert!(
            !old_actual.invalid && old_actual.live == 0 && old_actual.allocated == old_actual.freed
        );
        assert!(actual.allocated < old_actual.allocated);
        let current = default_expr(&ty, &declarations, false).unwrap_err();
        assert_eq!(current[0].json(), old[0].json());
    }
}
#[test]
fn exact_short_zero_and_sticky_preserve_full_primary_and_expose_legacy_vector_residual() {
    let ty = Type::Str;
    let observe = |limit, original_route| {
        with_limit_usage_refusal(limit, || {
            measure(|| {
                let result = if original_route {
                    original::default_expr(&ty, &[], false)
                } else {
                    default_expr(&ty, &[], false)
                };
                let code = result.as_ref().unwrap_err()[0].code;
                drop(result);
                code
            })
        })
    };
    let ((code, actual), refused, paid, first) = observe(LIMIT, false);
    assert_eq!(code, "SPX-G173");
    assert!(!refused && first.is_none());
    assert!(!actual.invalid && actual.live == 0 && actual.allocated == actual.freed);
    let ((code, actual), refused, used, first) = observe(paid, false);
    assert_eq!(code, "SPX-G173");
    assert!(!refused && used == paid && first.is_none());
    assert!(!actual.invalid && actual.live == 0 && actual.allocated == actual.freed);
    for limit in [0, paid - 1] {
        let ((code, actual), refused, used, first) = observe(limit, false);
        let ((old_code, old_actual), old_refused, old_used, old_first) = observe(limit, true);
        assert_eq!(code, old_code);
        assert_eq!(code, "SPX-G171");
        assert!(refused && old_refused);
        assert_eq!(used, old_used);
        assert_eq!(first, old_first);
        assert!(!actual.invalid && actual.live == 0 && actual.allocated == actual.freed);
        assert!(
            !old_actual.invalid && old_actual.live == 0 && old_actual.allocated == old_actual.freed
        );
        // This API still returns a real owned nonempty Vec; no offset or zero-output claim.
        assert!(actual.allocated > 0);
    }
    let (((), actual), refused, used, first) = with_limit_usage_refusal(0, || {
        measure(|| {
            let first_error = default_expr(&ty, &[], false).unwrap_err();
            let retry = default_expr(&Type::I64, &[], false).unwrap_err();
            assert_eq!(retry, first_error);
            drop(retry);
            drop(first_error);
        })
    });
    assert!(refused && used == 0 && first.is_some());
    assert!(!actual.invalid && actual.live == 0 && actual.allocated == actual.freed);
}

#[test]
fn complete_static_error_source_and_independent_old_dispatch_preserve_all_original_bytes() {
    use sha2::{Digest, Sha256};
    let record: serde_json::Value = serde_json::from_str(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/docs/evidence/default-static-error-custody-825/exact-source-inverse.json"
    )))
    .unwrap();
    let original = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/docs/evidence/default-static-error-custody-825/defaults.before.txt"
    ));
    assert_eq!(
        format!(
            "{:x}",
            crate::digest_hex::LowerHex(Sha256::digest(original.as_bytes()))
        ),
        record["before_sha256"].as_str().unwrap()
    );
    let mut current = include_str!("../defaults.rs").to_owned();
    let edits = record["edits"].as_array().unwrap();
    // Preserve the twelve original adapters and identify the integrated whole
    // source preinverse explicitly; it is applied first by the reverse walk.
    assert_eq!(
        edits
            .iter()
            .map(|edit| edit["name"].as_str().unwrap())
            .collect::<Vec<_>>(),
        [
            "closed-static-default-error-0",
            "closed-static-default-error-1",
            "closed-static-default-error-2",
            "closed-static-default-error-3",
            "closed-static-default-error-4",
            "closed-static-default-error-5",
            "closed-static-default-error-6",
            "closed-static-default-error-7",
            "closed-default-error-custody-controls",
            "explicit-default-module-path-1",
            "explicit-default-module-path-2",
            "explicit-default-module-path-3",
            "ordered-large-integrated-whole-source-preinverse",
        ]
    );
    let integrated = &edits[12];
    assert_eq!(integrated["count"].as_u64(), Some(1));
    for (field, expected) in [
        (
            "after",
            "2485211074d2424a39b29dcf1ffaee1c48807c7a0368a211395c367162b8cedb",
        ),
        (
            "before",
            "e97250b91fc9c8bb8f60e6ea1ef7d90a332dcb9dcf3fffdbd66348a9f3e2a366",
        ),
    ] {
        assert_eq!(
            format!(
                "{:x}",
                crate::digest_hex::LowerHex(Sha256::digest(
                    integrated[field].as_str().unwrap().as_bytes()
                ))
            ),
            expected
        );
    }
    for edit in record["edits"].as_array().unwrap().iter().rev() {
        let after = edit["after"].as_str().unwrap();
        assert_eq!(current.matches(after).count(), 1);
        current = current.replacen(after, edit["before"].as_str().unwrap(), 1);
    }
    assert_eq!(current, original);
    // Exact lexical import adapter only; compare the complete original file,
    // including its original trailing newline, rather than slicing any body.
    let saved = include_str!("original.rs");
    let adapter: serde_json::Value = serde_json::from_str(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/docs/evidence/composed-next-source-unions-a1edab/default-original-import-inverse.json"
    )))
    .unwrap();
    let mut restored = saved.to_owned();
    for hook in adapter["inverse"].as_array().unwrap() {
        let current = hook["current"].as_str().unwrap();
        assert_eq!(restored.matches(current).count(), 1);
        restored = restored.replacen(current, hook["before"].as_str().unwrap(), 1);
    }
    assert_eq!(
        format!(
            "{:x}",
            crate::digest_hex::LowerHex(Sha256::digest(restored.as_bytes()))
        ),
        "0b1c97abf52ee327690f629fdbf406cd34ab9e5f21a8d07f1c457e5fe85db07e"
    );
}
