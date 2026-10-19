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
    let actual = include_str!("../../../src/iterator_ops.rs");
    let current = crate::source_reflows_merge::main_e177_final_inputs::value(
        "src/iterator_ops.rs",
        "6a9bd6dfb76df8707e5cd21ddd4e0ec85b0248cfc1834645c14824d11d775b16",
        actual,
    );
    let expected = include_str!("../../../docs/evidence/actual2e92-fixed-operation-source-stage-20261018/iterator_ops.b00-parent.whole.rs.txt");
    assert_eq!(
        current, expected,
        "complete genuine pre-provider-view source"
    );
    assert_eq!(
        format!(
            "{:x}",
            crate::digest_hex::LowerHex(<sha2::Sha256 as sha2::Digest>::digest(
                expected.as_bytes()
            ))
        ),
        "6a9bd6dfb76df8707e5cd21ddd4e0ec85b0248cfc1834645c14824d11d775b16",
    );
    // Acquisition above authenticates the complete current compiled channel
    // before selecting b00's whole pre-provider view. Only current contains
    // the declared payload accessor: bind that exact reversible relationship,
    // then run the original constructor/hook inverse on its genuine generation.
    let retained_hook = "program\n        .functions()\n        .iter()\n        .any(|function| uses_function(function.payload()))";
    // The current retained-source iterator adds a profiling adapter. Normalize
    // that one declared hook only after the complete compiled acquisition above;
    // the original whole accessor and constructor inverses below stay unchanged.
    let profiled_hook = "program\n        .functions()\n        .profile_iter()\n        .any(|function| uses_function(function.payload()))";
    assert_eq!(actual.matches(profiled_hook).count(), 1);
    assert_eq!(actual.matches(retained_hook).count(), 0);
    let retained_current = actual.replacen(profiled_hook, retained_hook, 1);
    let actual = retained_current.as_str();
    assert_eq!(actual.matches(retained_hook).count(), 1);
    assert_eq!(
        actual.replacen(retained_hook, "program.functions().iter().any(uses_function)", 1),
        current,
        "whole current source differs from the authenticated b00 view only by the declared accessor hook"
    );
    let current = actual;
    let start = current.find("pub(crate) fn step_shape(").unwrap();
    let end = current.find("pub(crate) fn is_step_rest_field(").unwrap();
    // The later borrowed canonical-Step receiver is a separately qualified
    // optimization. Bind its complete current body and keep both earlier
    // constructors independently executable and byte-bound to this archive.
    assert_eq!(
        format!(
            "{:x}",
            crate::digest_hex::LowerHex(<sha2::Sha256 as sha2::Digest>::digest(
                &current.as_bytes()[start..end]
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
    // Retained ProgramSource functions expose the same AST through payload().
    // Account for that exact accessor hook; keep every original iterator byte.
    let retained_functions = "program\n        .functions()\n        .iter()\n        .any(|function| uses_function(function.payload()))";
    assert_eq!(projected.matches(retained_functions).count(), 1);
    projected = projected.replacen(
        retained_functions,
        "program.functions().iter().any(uses_function)",
        1,
    );
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
    let current = ci_str!("../../../src/list_ops.rs");
    let start = current.find("pub(crate) fn is_list(").unwrap();
    let end = current
        .find("pub(crate) fn validate_declarations(")
        .unwrap();
    // Bind the complete later borrowed predicate receiver, and reconstruct
    // every archived byte from the independently compiled old predicates.
    assert_eq!(
        format!(
            "{:x}",
            crate::digest_hex::LowerHex(<sha2::Sha256 as sha2::Digest>::digest(
                &current.as_bytes()[start..end]
            ))
        ),
        "c91741020e653b86582b9e811353438c67c934a29746dedfdec632ef81ece8a9"
    );
    let saved = ci_str!("../../../src/list_ops/original_constructor.rs");
    let saved_start = saved.find("pub(crate) fn is_list(").unwrap();
    let saved_end = saved.find("pub(crate) fn validate_declarations(").unwrap();
    let mut projected = format!(
        "{}{}{}",
        &current[..start],
        &saved[saved_start..saved_end],
        &current[end..]
    );
    let later_call = "&& canonical_step_shape(&program.declarations)";
    assert_eq!(projected.matches(later_call).count(), 1);
    projected = projected.replacen(
        later_call,
        "&& step_shape(&program.declarations, &resolved_step())",
        1,
    );
    let registration = "\n#[cfg(test)]\npub(crate) mod original_constructor;\n";
    assert_eq!(projected.matches(registration).count(), 1);
    projected = projected.replacen(registration, "", 1);
    assert_eq!(projected, original);
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
    // Restore the entire later lowering stage before the unchanged tag inverse.
    let ids_before_lowering = super::lowering_identity_tests::prior_source(
        "src/hir/ids.rs", ci_str!("../../../src/hir/ids.rs"),
    );
    // Reuse the complete closed-tag inverse and its exact current/prior pins.
    let acquired = crate::source_reflows_merge::main_e177_final_inputs::value(
        "src/hir/ids.rs",
        "5e56cda454fa3f14f61e2db5f410eea13ac7795d0887e680348eefdc64e80e64",
        &ids_before_lowering,
    );
    let current = super::declaration_tag::old_source("src/hir/ids.rs", &acquired).unwrap();
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
    // The separately owned streaming instance comparator adds one registration.
    let registration =
        "\n#[path = \"ids/instance_match.rs\"]\npub(in crate::hir) mod instance_match;\n";
    assert_eq!(inverse.matches(registration).count(), 1);
    let inverse = inverse.replacen(registration, "", 1);
    assert_eq!(inverse, old);
    let old = ci_str!("../../../docs/evidence/fixed-operation-identities-785/src__hir__ids__compiler.rs.before.txt");
    assert_eq!(
        format!(
            "{:x}",
            crate::digest_hex::LowerHex(<sha2::Sha256 as sha2::Digest>::digest(old.as_bytes()))
        ),
        "9a34241c9eb97cc65b4c9330b372bc6e507599ab6ddf683d9c32567ddaf2ebe1"
    );
    let current = super::lowering_identity_tests::prior_source(
        "src/hir/ids/compiler.rs",
        ci_str!("../../../src/hir/ids/compiler.rs"),
    );
    assert_eq!(current.matches("    MapOperation(crate::map_ops::MapOp),\n    ListOperation(crate::list_ops::ListOp),\n    IteratorOperation(crate::iterator_ops::IteratorOp),\n").count(), 1);
    assert_eq!(current.matches("            Self::MapOperation(op) => op.id(),\n            Self::ListOperation(op) => op.id(),\n            Self::IteratorOperation(op) => op.id(),\n").count(), 1);
    let mut inverse = current.replace("    MapOperation(crate::map_ops::MapOp),\n    ListOperation(crate::list_ops::ListOp),\n    IteratorOperation(crate::iterator_ops::IteratorOp),\n", "").replace("            Self::MapOperation(op) => op.id(),\n            Self::ListOperation(op) => op.id(),\n            Self::IteratorOperation(op) => op.id(),\n", "");
    // Later closed Vec/Box/stdin operation identities have exact additive hooks.
    for addition in [
        "    VecOperation(crate::vec_ops::VecOp),\n    BoxOperation(crate::box_ops::BoxOp),\n    StdinInspection(crate::stdin_stream_ops::PureOp),\n",
        "            Self::VecOperation(op) => op.id(),\n            Self::BoxOperation(op) => op.id(),\n            Self::StdinInspection(op) => op.id(),\n",
        "\n// These existing owned-call sites formerly received an &str into new(). The\n// closed enum replaces that receiver; it introduces no larger call carrier.\nconst _: () = assert!(std::mem::size_of::<CompilerDeclaration>() <= std::mem::size_of::<&str>());\n",
    ] {
        assert_eq!(inverse.matches(addition).count(), 1);
        inverse = inverse.replacen(addition, "", 1);
    }
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
    // The later retained resolver separates its two input lifetimes.
    assert_eq!(current.matches("&Resolver<'_, '_>").count(), 2);
    let current = current.replace("&Resolver<'_, '_>", "&Resolver<'_>");
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
    // The later retained resolver separates its two input lifetimes.
    assert_eq!(current.matches("&Resolver<'_, '_>").count(), 2);
    let current = current.replace("&Resolver<'_, '_>", "&Resolver<'_>");
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
    // The later retained resolver separates its two input lifetimes.
    assert_eq!(current.matches("&Resolver<'_, '_>").count(), 3);
    let current = current.replace("&Resolver<'_, '_>", "&Resolver<'_>");
    // The retained source-function receiver keeps the same generic lookup body.
    let retained_lookup = "if let Some(candidate) = resolver.source_functions().find(|candidate| {\n            candidate.payload().stable_id == owner.as_str()\n                && crate::source_verify::generic_collection_profile(candidate.payload())\n        }) {\n            return resolver.resolve_function_type_ref(candidate, ty, span);\n        }";
    assert_eq!(current.matches(retained_lookup).count(), 1);
    let current = current.replacen(
        retained_lookup,
        "if let Some(candidate) = resolver.program.functions().iter().find(|candidate| {\n            candidate.stable_id == owner.as_str()\n                && crate::source_verify::generic_collection_profile(candidate)\n        }) {\n            return resolver.resolve_function_type(candidate, ty, span);\n        }",
        1,
    );
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

// Original finish algorithms compiled independently; only their private names
// change. No production dispatch or fake source/authentication context exists.
use crate::ast::Span;
use crate::diagnostic::Diagnostic;
use crate::hir::{ExprPath, OwnershipMode, ResolvedType, Resolver};

fn saved_vec_finish<'p>(
    resolver: &Resolver<'_, '_>,
    function: &FunctionExecutionId,
    path: impl Into<ExprPath<'p>>,
    span: Span,
    op: crate::vec_ops::VecOp,
    element: ResolvedType,
    args: Vec<ResolvedExpr>,
) -> Result<ResolvedExpr, Diagnostic> {
    for (index, argument) in args.iter().enumerate() {
        if !op.accepts_resolved(index, &argument.ty, &element) {
            return Err(resolver.error(
                "SPX-H006",
                format!(
                    "vector operation `{}` argument {index} has the wrong type",
                    op.name()
                ),
                argument.span,
            ));
        }
    }
    let ty = op.resolved_return_type(&element);
    if op == crate::vec_ops::VecOp::StrAt && !crate::vec_ops::string_slot::arguments(&args) {
        return Err(resolver.error("SPX-H006", crate::vec_ops::string_slot::SHAPE, span));
    }
    if op == crate::vec_ops::VecOp::UniqueBy
        && !crate::vec_ops::field_unique::arguments(&args, &element)
    {
        return Err(resolver.error("SPX-H006", crate::vec_ops::field_unique::SHAPE, span));
    }
    let ownership = if op == crate::vec_ops::VecOp::StrAt {
        OwnershipMode::Borrow
    } else if op.returns_owner() {
        OwnershipMode::Own
    } else {
        OwnershipMode::Value
    };
    Ok(ResolvedExpr {
        id: path.into().expression_id(function),
        ty,
        ownership,
        kind: ResolvedExprKind::Call {
            callee: DeclarationId::new(op.id()),
            type_arguments: vec![element],
            instance: None,
            args,
        },
        span,
    })
}

fn saved_box_finish<'p>(
    function: &FunctionExecutionId,
    path: impl Into<ExprPath<'p>>,
    span: Span,
    op: crate::box_ops::BoxOp,
    element: ResolvedType,
    args: Vec<ResolvedExpr>,
) -> Result<ResolvedExpr, Diagnostic> {
    let Some(argument) = args.first() else {
        return Err(Diagnostic::io(
            "SPX-H006",
            "box operation argument is missing",
        ));
    };
    if argument.ty != op.resolved_param_type(&element) {
        return Err(Diagnostic::io(
            "SPX-H006",
            "box operation argument has the wrong type",
        ));
    }
    let ty = op.resolved_return_type(&element);
    Ok(ResolvedExpr {
        id: path.into().expression_id(function),
        ownership: if op == crate::box_ops::BoxOp::New || element == ResolvedType::Bytes {
            OwnershipMode::Own
        } else {
            OwnershipMode::Value
        },
        ty,
        kind: ResolvedExprKind::Call {
            callee: DeclarationId::new(op.id()),
            type_arguments: vec![element],
            instance: None,
            args,
        },
        span,
    })
}

#[test]
fn closed_vec_box_and_stdin_identities_keep_full_owned_traits_and_zero_sticky_drop() {
    let cases = crate::vec_ops::ALL
        .into_iter()
        .map(|op| (CompilerDeclaration::VecOperation(op), op.id()))
        .chain(
            crate::box_ops::ALL
                .into_iter()
                .map(|op| (CompilerDeclaration::BoxOperation(op), op.id())),
        )
        .chain(
            [
                crate::stdin_stream_ops::PureOp::Chunk,
                crate::stdin_stream_ops::PureOp::Eof,
            ]
            .into_iter()
            .map(|op| (CompilerDeclaration::StdinInspection(op), op.id())),
        );
    for (operation, text) in cases {
        let old = DeclarationId::new(text);
        let current = DeclarationId::compiler(operation);
        assert_eq!(current, old);
        assert_eq!(current.cmp(&old), std::cmp::Ordering::Equal);
        assert_eq!(hash(&current), hash(&old));
        assert_eq!(format!("{current:?}"), format!("{old:?}"));
        assert_eq!(
            crate::cache_codec::encode(&current).unwrap(),
            crate::cache_codec::encode(&old).unwrap()
        );
        let (((), observed), refused, spent, first) = with_limit_usage_refusal(0, || {
            measure(|| {
                let identity = DeclarationId::compiler(operation);
                assert_eq!(identity.as_str().as_bytes(), text.as_bytes());
                assert!(identity.is_compiler_static());
                assert_eq!(identity.owned_allocation_bytes(), 0);
                drop([identity.clone(), identity]);
            })
        });
        assert!(!refused && first.is_none() && !observed.invalid);
        assert_eq!(
            (
                spent,
                observed.allocations,
                observed.reallocations,
                observed.allocated,
                observed.freed,
                observed.live
            ),
            (0, 0, 0, 0, 0, 0)
        );
        let (((), observed), refused, spent, first) = with_limit_usage_refusal(0, || {
            measure(|| {
                assert!(!reserve_active_required(1));
                let identity = DeclarationId::compiler(operation);
                assert_eq!(identity, old);
                drop(identity);
                assert!(!reserve_active_required(0));
            })
        });
        assert!(refused && !observed.invalid);
        assert_eq!(first.unwrap().requested, 1);
        assert_eq!(
            (spent, observed.allocated, observed.freed, observed.live),
            (0, 0, 0, 0)
        );
    }
}

#[test]
fn vec_and_box_actual_finish_keep_complete_owned_oracle_diagnostics_and_drop() {
    // Fixture setup precedes observation; these are typed finish receivers,
    // not a claim that arbitrary integer AST payloads are source-valid calls.
    let program = crate::check(
        "module operation.identities; @id(\"operation.main\") fn main()->i64 {0}",
        "operation-identities.spx",
    )
    .unwrap();
    let resolver = Resolver {
        function_cells: std::borrow::Cow::Owned(crate::source_verify::function_cells(&program)),
        program: &program,
        byte_helpers: crate::loop_calls::byte_writer::SourceIndex::new(&program),
        declarations: crate::hir::DeclarationIndex::from_verified(&program).unwrap(),
        reuse: None,
        function_work: crate::hir::FunctionResolutionWork::default(),
        fused_producer: None,
        partition_profile: false,
    };
    let owner = owner();
    let mut accepted_vec_receivers = 0usize;
    for op in crate::vec_ops::ALL {
        for element in [ResolvedType::I64, ResolvedType::String] {
            let args = arguments(&owner, &crate::vec_ops::resolved_params(op, &element));
            // Operations with additional Place/source-profile requirements may
            // reject this direct typed fixture; equality covers all six fields.
            let old = saved_vec_finish(
                &resolver,
                &owner,
                "λ.call\0tail",
                Span::default(),
                op,
                element.clone(),
                args.clone(),
            );
            let current = crate::hir::resolve_vec_call::finish(
                &resolver,
                &owner,
                "λ.call\0tail",
                Span::default(),
                op,
                element.clone(),
                args.clone(),
            );
            assert_eq!(current, old, "{op:?}");
            if current.is_ok() {
                accepted_vec_receivers += 1;
                let old_args = args.clone();
                let current_args = args.clone();
                let old_element = element.clone();
                let current_element = element.clone();
                compare_finish_drop(
                    || {
                        saved_vec_finish(
                            &resolver,
                            &owner,
                            "full.drop",
                            Span::default(),
                            op,
                            old_element,
                            old_args,
                        )
                    },
                    || {
                        crate::hir::resolve_vec_call::finish(
                            &resolver,
                            &owner,
                            "full.drop",
                            Span::default(),
                            op,
                            current_element,
                            current_args,
                        )
                    },
                    op.id(),
                );
            }
            for at in 0..args.len() {
                let mut changed = args.clone();
                changed[at].ty = ResolvedType::Bool;
                assert_eq!(
                    crate::hir::resolve_vec_call::finish(
                        &resolver,
                        &owner,
                        "hostile",
                        Span::default(),
                        op,
                        element.clone(),
                        changed.clone()
                    ),
                    saved_vec_finish(
                        &resolver,
                        &owner,
                        "hostile",
                        Span::default(),
                        op,
                        element.clone(),
                        changed
                    )
                );
            }
        }
    }
    assert!(
        accepted_vec_receivers > 0,
        "actual Vec finish must reach success and physical Drop comparison"
    );
    for op in crate::box_ops::ALL {
        for element in [ResolvedType::I64, ResolvedType::Bytes] {
            let args = arguments(&owner, &crate::box_ops::resolved_params(op, &element));
            let old_args = args.clone();
            let current_args = args.clone();
            let old_element = element.clone();
            let current_element = element.clone();
            compare_finish_drop(
                || {
                    saved_box_finish(
                        &owner,
                        "full.drop",
                        Span::default(),
                        op,
                        old_element,
                        old_args,
                    )
                },
                || {
                    crate::hir::resolve_box_call::finish(
                        &owner,
                        "full.drop",
                        Span::default(),
                        op,
                        current_element,
                        current_args,
                    )
                },
                op.id(),
            );
            let mut changed = args;
            changed.clear();
            assert_eq!(
                crate::hir::resolve_box_call::finish(
                    &owner,
                    "hostile",
                    Span::default(),
                    op,
                    element.clone(),
                    changed.clone()
                ),
                saved_box_finish(&owner, "hostile", Span::default(), op, element, changed)
            );
        }
    }
}

use crate::ast::{Expr, ExprKind};
use crate::hir::resolve_expr::path_arena::Seg;
use crate::hir::{Binding, Place};
use std::collections::BTreeMap;

impl Resolver<'_, '_> {
    fn saved_stdin_inspection<'p>(
        &self,
        execution: &FunctionExecutionId,
        expression: &Expr,
        bindings: &BTreeMap<String, Binding>,
        path: impl Into<ExprPath<'p>>,
    ) -> Result<Option<ResolvedExpr>, Diagnostic> {
        let ExprKind::Call {
            name,
            type_arguments,
            args,
        } = &expression.kind
        else {
            return Ok(None);
        };
        let Some(op) = crate::stdin_stream_ops::pure_by_name(name) else {
            return Ok(None);
        };
        let [argument] = args.as_slice() else {
            return Err(self.error(
                "SPX-T270",
                "streaming stdin inspection requires one reader",
                expression.span,
            ));
        };
        let ExprKind::Var(name) = &argument.kind else {
            return Err(self.error(
                "SPX-T270",
                "streaming stdin inspection requires an exact named reader",
                argument.span,
            ));
        };
        let binding = bindings.get(name).ok_or_else(|| {
            self.error(
                "SPX-H002",
                "streaming stdin reader is out of scope",
                argument.span,
            )
        })?;
        if !type_arguments.is_empty()
            || !crate::stdin_stream_ops::is_reader(&binding.ty)
            || !matches!(
                binding.ownership,
                OwnershipMode::Own | OwnershipMode::Borrow
            )
        {
            return Err(self.error(
                "SPX-T270",
                "invalid streaming stdin inspection operand",
                argument.span,
            ));
        }
        let path = path.into();
        let place = Place {
            root: binding.id.clone(),
            projections: Vec::new(),
        };
        let (kind, ownership) = match op {
            crate::stdin_stream_ops::PureOp::Chunk => (
                ResolvedExprKind::BorrowPlace {
                    operation: DeclarationId::new(op.id()),
                    place,
                },
                OwnershipMode::Borrow,
            ),
            crate::stdin_stream_ops::PureOp::Eof => (
                ResolvedExprKind::Call {
                    callee: DeclarationId::new(op.id()),
                    type_arguments: Vec::new(),
                    instance: None,
                    args: vec![ResolvedExpr {
                        id: path.expression_id_with(execution, &[Seg::ArgZero.into()]),
                        ty: binding.ty.clone(),
                        ownership: binding.ownership,
                        kind: ResolvedExprKind::Place(place),
                        span: argument.span,
                    }],
                },
                OwnershipMode::Value,
            ),
        };
        Ok(Some(ResolvedExpr {
            id: path.expression_id(execution),
            ty: op.result(),
            ownership,
            kind,
            span: expression.span,
        }))
    }
}

#[test]
fn stdin_actual_inspections_keep_full_owned_oracle_scope_refusals_and_drop() {
    let program = crate::check(
        "module stream.identities; @id(\"stream.main\") fn main()->i64 {0}",
        "stream-identities.spx",
    )
    .unwrap();
    let resolver = Resolver {
        function_cells: std::borrow::Cow::Owned(crate::source_verify::function_cells(&program)),
        program: &program,
        byte_helpers: crate::loop_calls::byte_writer::SourceIndex::new(&program),
        declarations: crate::hir::DeclarationIndex::from_verified(&program).unwrap(),
        reuse: None,
        function_work: crate::hir::FunctionResolutionWork::default(),
        fused_producer: None,
        partition_profile: false,
    };
    let owner = owner();
    for (name, text) in [
        ("stdin_stream_chunk", crate::stdin_stream_ops::CHUNK_ID),
        ("stdin_stream_eof", crate::stdin_stream_ops::EOF_ID),
    ] {
        let expression = Expr {
            kind: ExprKind::Call {
                name: name.to_owned(),
                type_arguments: Vec::new(),
                args: vec![Expr {
                    kind: ExprKind::Var("reader".to_owned()),
                    span: Span::default(),
                }],
            },
            span: Span::default(),
        };
        let mut bindings = BTreeMap::new();
        bindings.insert(
            "reader".to_owned(),
            Binding {
                traversal_row: None,
                id: ValueId::parameter(&owner, 0),
                ty: crate::stdin_stream_ops::reader(),
                ownership: OwnershipMode::Borrow,
                mutable: false,
            },
        );
        let old = resolver.saved_stdin_inspection(&owner, &expression, &bindings, "λ.stream\0tail");
        let current =
            resolver.resolve_stdin_inspection(&owner, &expression, &bindings, "λ.stream\0tail");
        assert_eq!(current, old);
        assert!(current.unwrap().is_some());
        compare_finish_drop(
            || {
                resolver
                    .saved_stdin_inspection(&owner, &expression, &bindings, "full.drop")
                    .map(|value| value.unwrap())
            },
            || {
                resolver
                    .resolve_stdin_inspection(&owner, &expression, &bindings, "full.drop")
                    .map(|value| value.unwrap())
            },
            text,
        );
        for mutation in 0..6 {
            let mut changed = expression.clone();
            let mut changed_bindings = bindings.clone();
            match mutation {
                0 => {
                    changed_bindings.clear();
                }
                1 => {
                    changed_bindings.get_mut("reader").unwrap().ty = ResolvedType::I64;
                }
                2 => {
                    changed_bindings.get_mut("reader").unwrap().ownership = OwnershipMode::Value;
                }
                3 => {
                    if let ExprKind::Call { args, .. } = &mut changed.kind {
                        args.clear();
                    }
                }
                4 => {
                    if let ExprKind::Call { type_arguments, .. } = &mut changed.kind {
                        type_arguments.push(crate::ast::Type::I64);
                    }
                }
                _ => {
                    if let ExprKind::Call { args, .. } = &mut changed.kind {
                        args[0].kind = ExprKind::Int(0);
                    }
                }
            }
            let old =
                resolver.saved_stdin_inspection(&owner, &changed, &changed_bindings, "hostile");
            let current =
                resolver.resolve_stdin_inspection(&owner, &changed, &changed_bindings, "hostile");
            assert_eq!(current, old, "{name}, mutation {mutation}");
            assert!(current.is_err());
        }
    }
}
