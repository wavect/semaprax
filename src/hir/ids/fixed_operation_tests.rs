//! Closed-operation identities carry the same bytes, without acquiring backing.
//! The argument builder supplies typed receiver inputs, not a verified HIR program.
use super::*;
use crate::bounded_output::{reserve_active_required, with_limit_usage, with_limit_usage_refusal};
use crate::hir::{ResolvedExpr, ResolvedExprKind, ResolvedParam};
use crate::test_allocation_observer::measure;

pub(in crate::hir) fn owner() -> FunctionExecutionId {
    FunctionExecutionId::Monomorphic(DeclarationId::new("owner.λ\0full"))
}
pub(in crate::hir) fn arguments(
    owner: &FunctionExecutionId,
    params: &[ResolvedParam],
) -> Vec<ResolvedExpr> {
    params
        .iter()
        .enumerate()
        .map(|(ordinal, parameter)| ResolvedExpr {
            id: ExpressionId::new(owner, &format!("argument.{ordinal}")),
            ty: parameter.ty.clone(),
            ownership: parameter.ownership,
            kind: ResolvedExprKind::Int(ordinal as i64),
            span: parameter.span,
        })
        .collect()
}

#[test]
fn closed_operation_identities_keep_owned_wire_traits_and_zero_sticky_complete_drop() {
    let cases = crate::map_ops::MapOp::ALL
        .into_iter()
        .map(|op| (CompilerDeclaration::MapOperation(op), op.id()))
        .chain(
            crate::list_ops::ALL
                .into_iter()
                .map(|op| (CompilerDeclaration::ListOperation(op), op.id())),
        )
        .chain(
            crate::iterator_ops::ALL
                .into_iter()
                .map(|op| (CompilerDeclaration::IteratorOperation(op), op.id())),
        );
    for (operation, text) in cases {
        let oracle = DeclarationId::new(text);
        let static_id = DeclarationId::compiler(operation);
        assert_eq!(
            crate::cache_codec::encode(&static_id).unwrap(),
            crate::cache_codec::encode(&oracle).unwrap()
        );
        assert_eq!(format!("{static_id:?}"), format!("{oracle:?}"));
        assert_eq!(static_id.cmp(&oracle), std::cmp::Ordering::Equal);
        assert_eq!(hash(&static_id), hash(&oracle));
        let (((), observed), refused, paid) = with_limit_usage(0, || {
            measure(|| {
                let id = DeclarationId::compiler(operation);
                assert_eq!(id, oracle);
                assert_eq!(id.as_str().as_bytes(), text.as_bytes());
                assert!(id.is_compiler_static());
                assert_eq!(id.owned_allocation_bytes(), 0);
                drop([id.clone(), id.clone(), id]);
            })
        });
        assert!(!refused && !observed.invalid);
        assert_eq!(
            (
                observed.allocations,
                observed.allocated,
                observed.freed,
                observed.live,
                paid
            ),
            (0, 0, 0, 0, 0)
        );
        let (((first, retry), observed), refused, paid, receipt) =
            with_limit_usage_refusal(0, || {
                measure(|| {
                    let first = reserve_active_required(1);
                    let id = DeclarationId::compiler(operation);
                    assert_eq!(id, oracle);
                    drop(id);
                    (first, reserve_active_required(0))
                })
            });
        assert!(!first && !retry && refused && !observed.invalid);
        assert_eq!(
            (observed.allocated, observed.freed, observed.live, paid),
            (0, 0, 0, 0)
        );
        assert_eq!(receipt.unwrap().requested, 1);
        let ((), observed) = measure(|| drop(DeclarationId::new(text)));
        assert!(!observed.invalid);
        assert_eq!(
            observed.allocated,
            text.len() + DeclarationId::shared_backing_bytes(text.len()).unwrap()
        );
        assert_eq!(observed.allocated, observed.freed);
        assert_eq!(observed.live, 0);
    }
}

fn hash(value: &DeclarationId) -> u64 {
    use std::hash::{Hash, Hasher};
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    value.hash(&mut hasher);
    hasher.finish()
}

#[test]
fn full_original_operation_sources_and_receivers_have_only_declared_identity_hooks() {
    let original = ci_str!(
        "../../../docs/evidence/fixed-operation-identities-785/src__iterator_ops.rs.before.txt"
    );
    assert_eq!(
        format!(
            "{:x}",
            crate::digest_hex::LowerHex(<sha2::Sha256 as sha2::Digest>::digest(
                original.as_bytes()
            ))
        ),
        "8541bd6ab7e58bd7a39eae3eef770e6ac29f104a6ea6923aa73287d2b6a8ed44"
    );
    let current = ci_str!("../../../src/iterator_ops.rs");
    let start = current.find("pub(crate) fn step_shape(").unwrap();
    let end = current.find("pub(crate) fn is_step_rest_field(").unwrap();
    // The later borrowed canonical-Step receiver is a separately qualified
    // optimization. Bind its complete current body and keep both earlier
    // constructors independently executable and byte-bound to this archive.
    assert_eq!(
        format!(
            "{:x}",
            crate::digest_hex::LowerHex(<sha2::Sha256 as sha2::Digest>::digest(
                current[start..end].as_bytes()
            ))
        ),
        "ef04cf361ae1d56eb44b0e308786371d14d93239341b6fd6c322d4710a416fa4"
    );
    let saved = ci_str!("../../../src/iterator_ops/original_constructor.rs");
    let saved_start = saved.find("pub(crate) fn step_shape(").unwrap();
    let saved_end = saved.find("pub(crate) fn element(").unwrap();
    let saved = saved[saved_start..saved_end].replacen(
        "}\n\npub(crate) fn validate_declarations(",
        "}\npub(crate) fn validate_declarations(",
        1,
    );
    let mut projected = format!("{}{}{}", &current[..start], saved, &current[end..]);
    let registration = "\n#[cfg(test)]\npub(crate) mod original_constructor;\n";
    assert_eq!(projected.matches(registration).count(), 1);
    projected = projected.replacen(registration, "", 1);
    assert_eq!(projected, original);
    assert_eq!(original.matches("pub(crate) const fn id(self)").count(), 1);

    let original = ci_str!(
        "../../../docs/evidence/fixed-operation-identities-785/src__list_ops.rs.before.txt"
    );
    assert_eq!(
        format!(
            "{:x}",
            crate::digest_hex::LowerHex(<sha2::Sha256 as sha2::Digest>::digest(
                original.as_bytes()
            ))
        ),
        "0abc67a67c5c22b175b81e2ef82e46206421f6a974fa772ea40de40181467dd2"
    );
    assert_eq!(ci_str!("../../../src/list_ops.rs"), original);
    assert_eq!(original.matches("pub(crate) const fn id(self)").count(), 1);

    let old = ci_str!(
        "../../../docs/evidence/fixed-operation-identities-785/src__hir__ids.rs.before.txt"
    );
    assert_eq!(
        format!(
            "{:x}",
            crate::digest_hex::LowerHex(<sha2::Sha256 as sha2::Digest>::digest(old.as_bytes()))
        ),
        "4c4f9dd06f00371bb571d1c7122e292fccabd7608b55160469c13350407555fd"
    );
    let current = ci_str!("../../../src/hir/ids.rs");
    assert_eq!(
        current
            .matches("\n#[cfg(test)]\npub(in crate::hir) mod fixed_operation_tests;\n")
            .count(),
        1
    );
    let inverse = current.replace(
        "\n#[cfg(test)]\npub(in crate::hir) mod fixed_operation_tests;\n",
        "",
    );
    assert_eq!(inverse, old);
    let old = ci_str!("../../../docs/evidence/fixed-operation-identities-785/src__hir__ids__compiler.rs.before.txt");
    assert_eq!(
        format!(
            "{:x}",
            crate::digest_hex::LowerHex(<sha2::Sha256 as sha2::Digest>::digest(old.as_bytes()))
        ),
        "9a34241c9eb97cc65b4c9330b372bc6e507599ab6ddf683d9c32567ddaf2ebe1"
    );
    let current = ci_str!("../../../src/hir/ids/compiler.rs");
    assert_eq!(current.matches("    MapOperation(crate::map_ops::MapOp),\n    ListOperation(crate::list_ops::ListOp),\n    IteratorOperation(crate::iterator_ops::IteratorOp),\n").count(), 1);
    assert_eq!(current.matches("            Self::MapOperation(op) => op.id(),\n            Self::ListOperation(op) => op.id(),\n            Self::IteratorOperation(op) => op.id(),\n").count(), 1);
    let inverse = current.replace("    MapOperation(crate::map_ops::MapOp),\n    ListOperation(crate::list_ops::ListOp),\n    IteratorOperation(crate::iterator_ops::IteratorOp),\n", "").replace("            Self::MapOperation(op) => op.id(),\n            Self::ListOperation(op) => op.id(),\n            Self::IteratorOperation(op) => op.id(),\n", "");
    assert_eq!(inverse, old);
    let old =
        ci_str!("../../../docs/evidence/fixed-operation-identities-785/src__map_ops.rs.before.txt");
    assert_eq!(
        format!(
            "{:x}",
            crate::digest_hex::LowerHex(<sha2::Sha256 as sha2::Digest>::digest(old.as_bytes()))
        ),
        "b24e08d4fb5ca49986917f97b61c72eb796a6dd079db5fc95cd0291a2bf06852"
    );
    let current = ci_str!("../../../src/map_ops.rs");
    assert_eq!(current.matches("pub(crate) const fn id(self)").count(), 1);
    let inverse = current.replace("pub(crate) const fn id(self)", "pub(crate) fn id(self)");
    assert_eq!(inverse, old);
    let old = ci_str!("../../../docs/evidence/fixed-operation-identities-785/src__hir__resolve_box_call__map.rs.before.txt");
    assert_eq!(
        format!(
            "{:x}",
            crate::digest_hex::LowerHex(<sha2::Sha256 as sha2::Digest>::digest(old.as_bytes()))
        ),
        "fb4ddcbbb38a9b81bd2f11d3781532729f869eaa890e1ac718806dc3d2d592a8"
    );
    let current = ci_str!("../../../src/hir/resolve_box_call/map.rs");
    assert_eq!(
        current
            .matches("\n#[cfg(test)]\n#[path = \"map/identity_tests.rs\"]\nmod identity_tests;\n")
            .count(),
        1
    );
    assert_eq!(
        current
            .matches("DeclarationId::compiler(crate::hir::CompilerDeclaration::MapOperation(op))")
            .count(),
        1
    );
    let inverse = current
        .replace(
            "\n#[cfg(test)]\n#[path = \"map/identity_tests.rs\"]\nmod identity_tests;\n",
            "",
        )
        .replace(
            "DeclarationId::compiler(crate::hir::CompilerDeclaration::MapOperation(op))",
            "DeclarationId::new(op.id())",
        );
    let saved = ci_str!("../resolve_box_call/map/identity_tests.rs");
    let original = old
        .split("pub(super) fn finish")
        .nth(1)
        .unwrap()
        .split("#[cfg(test)]")
        .next()
        .unwrap()
        .trim();
    let independent = saved
        .split("fn saved_finish")
        .nth(1)
        .unwrap()
        .split("#[test]")
        .next()
        .unwrap()
        .trim();
    assert_eq!(independent, original);
    assert_eq!(inverse, old);
    let old = ci_str!("../../../docs/evidence/fixed-operation-identities-785/src__hir__resolve_box_call__list.rs.before.txt");
    assert_eq!(
        format!(
            "{:x}",
            crate::digest_hex::LowerHex(<sha2::Sha256 as sha2::Digest>::digest(old.as_bytes()))
        ),
        "4dcfc2ca08c79396747d598370a743fb91bcd670bdb0f0820036626d8e78070b"
    );
    let current = ci_str!("../../../src/hir/resolve_box_call/list.rs");
    assert_eq!(
        current
            .matches("\n#[cfg(test)]\n#[path = \"list/identity_tests.rs\"]\nmod identity_tests;\n")
            .count(),
        1
    );
    assert_eq!(
        current
            .matches("DeclarationId::compiler(crate::hir::CompilerDeclaration::ListOperation(op))")
            .count(),
        1
    );
    let inverse = current
        .replace(
            "\n#[cfg(test)]\n#[path = \"list/identity_tests.rs\"]\nmod identity_tests;\n",
            "",
        )
        .replace(
            "DeclarationId::compiler(crate::hir::CompilerDeclaration::ListOperation(op))",
            "DeclarationId::new(op.id())",
        );
    let saved = ci_str!("../resolve_box_call/list/identity_tests.rs");
    let original = old
        .split("pub(super) fn finish")
        .nth(1)
        .unwrap()
        .split("#[cfg(test)]")
        .next()
        .unwrap()
        .trim();
    let independent = saved
        .split("fn saved_finish")
        .nth(1)
        .unwrap()
        .split("#[test]")
        .next()
        .unwrap()
        .trim();
    assert_eq!(independent, original);
    assert_eq!(inverse, old);
    let old = ci_str!("../../../docs/evidence/fixed-operation-identities-785/src__hir__resolve_box_call__iterator.rs.before.txt");
    assert_eq!(
        format!(
            "{:x}",
            crate::digest_hex::LowerHex(<sha2::Sha256 as sha2::Digest>::digest(old.as_bytes()))
        ),
        "6badd599deed511632f2cfc396a09ef8b04dbace494c1496a75a6bfdef8f200d"
    );
    let current = ci_str!("../../../src/hir/resolve_box_call/iterator.rs");
    assert_eq!(
        current
            .matches(
                "\n#[cfg(test)]\n#[path = \"iterator/identity_tests.rs\"]\nmod identity_tests;\n"
            )
            .count(),
        1
    );
    assert_eq!(
        current
            .matches(
                "DeclarationId::compiler(crate::hir::CompilerDeclaration::IteratorOperation(op))"
            )
            .count(),
        1
    );
    let inverse = current
        .replace(
            "\n#[cfg(test)]\n#[path = \"iterator/identity_tests.rs\"]\nmod identity_tests;\n",
            "",
        )
        .replace(
            "DeclarationId::compiler(crate::hir::CompilerDeclaration::IteratorOperation(op))",
            "DeclarationId::new(op.id())",
        );
    let saved = ci_str!("../resolve_box_call/iterator/identity_tests.rs");
    let original = old
        .split("pub(super) fn finish")
        .nth(1)
        .unwrap()
        .split("#[cfg(test)]")
        .next()
        .unwrap()
        .trim();
    let independent = saved
        .split("fn saved_finish")
        .nth(1)
        .unwrap()
        .split("#[test]")
        .next()
        .unwrap()
        .trim();
    assert_eq!(independent, original);
    assert_eq!(inverse, old);
}

/// Both closures own their independently prepared inputs before observation.
/// This compares full receiving Drop, without claiming its other acquisitions paid.
pub(in crate::hir) fn compare_finish_drop(
    old: impl FnOnce() -> Result<ResolvedExpr, crate::diagnostic::Diagnostic>,
    current: impl FnOnce() -> Result<ResolvedExpr, crate::diagnostic::Diagnostic>,
    text: &str,
) {
    let ((), before) = measure(|| drop(old().unwrap()));
    let ((), after) = measure(|| drop(current().unwrap()));
    assert!(!before.invalid && !after.invalid);
    assert_eq!((before.live, after.live), (0, 0));
    assert_eq!(before.allocated, before.freed);
    assert_eq!(after.allocated, after.freed);
    assert_eq!(before.allocations, after.allocations + 2);
    assert_eq!(
        before.allocated,
        after.allocated + text.len() + DeclarationId::shared_backing_bytes(text.len()).unwrap()
    );
}
