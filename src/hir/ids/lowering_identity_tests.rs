//! Closed lowering identities against immediate owning receivers and full old text resolver.
use super::*;
use crate::bounded_output::{reserve_active_required, with_limit_usage, with_limit_usage_refusal};
use crate::hir::{self, OwnershipMode, ResolvedExpr, ResolvedExprKind, ResolvedType};
use crate::test_allocation_observer::measure;
use sha2::{Digest, Sha256};

// Compile-time actual whole receiver envelopes, not a field-sum forecast or a rebate.
const _: () = {
    assert!(std::mem::size_of::<hir::resolve_for::Profile>() <= std::mem::size_of::<hir::resolve_for::identity_before::Profile>());
    assert!(std::mem::align_of::<hir::resolve_for::Profile>() <= std::mem::align_of::<hir::resolve_for::identity_before::Profile>());
};

fn sha(bytes: &[u8]) -> String {
    format!("{:x}", crate::digest_hex::LowerHex(Sha256::digest(bytes)))
}

pub(super) fn prior_source(source: &str, current: &str) -> String {
    // Exact current control repairs precede the immutable complete lowering
    // inverse. Every archived baseline and original old-body check stays intact.
    let repaired = match source {
        "src/hir/ids/fixed_operation_tests.rs" => {
            assert_eq!(current.len(), 40750);
            assert_eq!(sha(current.as_bytes()), "f5ca1f734cd8ae4c3ecd9c73a65da5ff263d01cd412ffc97b4a00a326dc0f42d");
            let mut before = current.to_owned();
            assert_eq!(before.matches("    // The current retained-source iterator adds a profiling adapter. Normalize\n    // that one declared hook only after the complete compiled acquisition above;\n    // the original whole accessor and constructor inverses below stay unchanged.\n    let profiled_hook = \"program\\n        .functions()\\n        .profile_iter()\\n        .any(|function| uses_function(function.payload()))\";\n    assert_eq!(actual.matches(profiled_hook).count(), 1);\n    assert_eq!(actual.matches(retained_hook).count(), 0);\n    let retained_current = actual.replacen(profiled_hook, retained_hook, 1);\n    let actual = retained_current.as_str();\n").count(), 1);
            before = before.replacen("    // The current retained-source iterator adds a profiling adapter. Normalize\n    // that one declared hook only after the complete compiled acquisition above;\n    // the original whole accessor and constructor inverses below stay unchanged.\n    let profiled_hook = \"program\\n        .functions()\\n        .profile_iter()\\n        .any(|function| uses_function(function.payload()))\";\n    assert_eq!(actual.matches(profiled_hook).count(), 1);\n    assert_eq!(actual.matches(retained_hook).count(), 0);\n    let retained_current = actual.replacen(profiled_hook, retained_hook, 1);\n    let actual = retained_current.as_str();\n", "", 1);
            assert_eq!(before.matches("    // Restore the entire later lowering stage before the unchanged tag inverse.\n    let ids_before_lowering = super::lowering_identity_tests::prior_source(\n        \"src/hir/ids.rs\", ci_str!(\"../../../src/hir/ids.rs\"),\n    );\n").count(), 1);
            before = before.replacen("    // Restore the entire later lowering stage before the unchanged tag inverse.\n    let ids_before_lowering = super::lowering_identity_tests::prior_source(\n        \"src/hir/ids.rs\", ci_str!(\"../../../src/hir/ids.rs\"),\n    );\n", "", 1);
            assert_eq!(before.matches("        &ids_before_lowering,\n").count(), 1);
            before = before.replacen("        &ids_before_lowering,\n", "        ci_str!(\"../../../src/hir/ids.rs\"),\n", 1);
            Some(before)
        }
        "src/hir/ids/declaration_tag.rs" => {
            assert_eq!(current.len(), 29891);
            assert_eq!(sha(current.as_bytes()), "58a3bbec53e96e2f429cebfe3bfe66105293a15ffc8862afe3503e293fd26ba9");
            let mut before = current.to_owned();
            assert_eq!(before.matches("    let ids_before_lowering = super::lowering_identity_tests::prior_source(\n        \"src/hir/ids.rs\", include_str!(\"../ids.rs\"),\n    );\n").count(), 1);
            before = before.replacen("    let ids_before_lowering = super::lowering_identity_tests::prior_source(\n        \"src/hir/ids.rs\", include_str!(\"../ids.rs\"),\n    );\n", "", 1);
            assert_eq!(before.matches("        (\"src/hir/ids.rs\",ids_before_lowering.as_str()),\n").count(), 1);
            before = before.replacen("        (\"src/hir/ids.rs\",ids_before_lowering.as_str()),\n", "        (\"src/hir/ids.rs\",include_str!(\"../ids.rs\")),\n", 1);
            Some(before)
        }
        _ => None,
    };
    let current = repaired.as_deref().unwrap_or(current);
    let proof: serde_json::Value = serde_json::from_str(include_str!("lowering_identity_inverse.json")).unwrap();
    let row = proof["rows"].as_array().unwrap().iter().find(|row| row["source"] == source).unwrap();
    assert_eq!(current.len() as u64, row["current_len"].as_u64().unwrap());
    assert_eq!(sha(current.as_bytes()), row["current_sha256"].as_str().unwrap());
    let mut old = current.to_owned();
    for edit in row["edits"].as_array().unwrap().iter().rev() {
        let start = edit["start"].as_u64().unwrap() as usize;
        let new = edit["new"].as_str().unwrap();
        assert_eq!(old.get(start..start + new.len()), Some(new));
        old.replace_range(start..start + new.len(), edit["old"].as_str().unwrap());
    }
    assert_eq!(old.len() as u64, row["before_len"].as_u64().unwrap());
    assert_eq!(sha(old.as_bytes()), row["before_sha256"].as_str().unwrap());
    old
}

pub(super) fn cases() -> Vec<(CompilerDeclaration, &'static str)> {
    use CompilerDeclaration as C;
    use crate::str_ops::StrOp as S;
    let mut result = vec![
        (C::StrOperation(S::LenBytes), crate::str_ops::LEN_BYTES_ID),
        (C::StrOperation(S::IsEmpty), crate::str_ops::IS_EMPTY_ID),
        (C::StrOperation(S::StartsWith), crate::str_ops::STARTS_WITH_ID),
        (C::StrOperation(S::Contains), crate::str_ops::CONTAINS_ID),
        (C::StrOperation(S::ByteAt), crate::str_ops::BYTE_AT_ID),
        (C::HostIoOperation(crate::host_io_ops::HostIoOp::StdoutWrite), crate::host_io_ops::HostIoOp::StdoutWrite.id()),
        (C::IterStep, crate::iterator_ops::STEP_ID),
        (C::IterYield, crate::iterator_ops::YIELD_ID),
        (C::IterDone, crate::iterator_ops::DONE_ID),
        (C::IterItem, crate::iterator_ops::ITEM_ID),
        (C::IterRest, crate::iterator_ops::REST_ID),
        (C::IteratorOperation(crate::iterator_ops::IteratorOp::Next), crate::iterator_ops::NEXT_ID),
        (C::VecOperation(crate::vec_ops::VecOp::Len), crate::vec_ops::LEN_ID),
        (C::VecOperation(crate::vec_ops::VecOp::Get), crate::vec_ops::GET_ID),
        (C::Option, crate::prelude::OPTION_ID),
        (C::OptionSome, crate::prelude::OPTION_SOME_ID),
        (C::OptionSomeValue, crate::prelude::OPTION_SOME_VALUE_ID),
    ];
    result.extend(crate::byte_ops::ByteOp::ALL.into_iter().chain([crate::byte_ops::ByteOp::Partition]).map(|op| (C::ByteOperation(op), op.id())));
    result
}

#[test]
fn every_closed_lowering_identity_keeps_owned_wire_traits_zero_allocation_and_sticky_floor() {
    use std::collections::hash_map::DefaultHasher;
    for (tag, bytes) in cases() {
        let old = DeclarationId::new(bytes);
        let current = DeclarationId::compiler(tag);
        assert_eq!(current, old);
        assert_eq!(current.cmp(&old), std::cmp::Ordering::Equal);
        let hash = |id: &DeclarationId| { let mut h = DefaultHasher::new(); id.hash(&mut h); h.finish() };
        assert_eq!(hash(&current), hash(&old));
        assert_eq!(format!("{current:?}"), format!("{old:?}"));
        assert_eq!(crate::cache_codec::encode(&current).unwrap(), crate::cache_codec::encode(&old).unwrap());
        let (((), metrics), refused, paid) = with_limit_usage(0, || measure(|| {
            let id = DeclarationId::compiler(tag);
            assert!(id.is_compiler_static());
            assert_eq!(id.as_str().as_bytes(), bytes.as_bytes());
            drop([id.clone(), id]);
        }));
        assert!(!refused && !metrics.invalid);
        assert_eq!((metrics.allocated, metrics.freed, metrics.live, paid), (0, 0, 0, 0));
        let (((), metrics), refused, paid, receipt) = with_limit_usage_refusal(0, || measure(|| {
            assert!(!reserve_active_required(1));
            drop(DeclarationId::compiler(tag));
            assert!(!reserve_active_required(0));
        }));
        assert!(refused && !metrics.invalid);
        assert_eq!(receipt.unwrap().requested, 1);
        assert_eq!((metrics.allocated, metrics.freed, metrics.live, paid), (0, 0, 0, 0));
    }
    // These replace existing &str arguments; no larger identity or profile role is introduced.
    assert!(std::mem::size_of::<CompilerDeclaration>() <= std::mem::size_of::<&str>());
    assert!(std::mem::align_of::<CompilerDeclaration>() <= std::mem::align_of::<&str>());
    assert!(std::mem::size_of::<hir::resolve_for::Profile>() <= std::mem::size_of::<hir::resolve_for::identity_before::Profile>());
    assert!(std::mem::align_of::<hir::resolve_for::Profile>() <= std::mem::align_of::<hir::resolve_for::identity_before::Profile>());
    assert!(std::mem::size_of::<DeclarationId>() <= std::mem::size_of::<String>());
}

fn expr(owner: &FunctionExecutionId, path: &str, ty: ResolvedType, ownership: OwnershipMode) -> ResolvedExpr {
    ResolvedExpr { id: ExpressionId::new(owner, path), ty, ownership, kind: ResolvedExprKind::Int(0), span: crate::ast::Span::default() }
}

#[test]
fn both_complete_traversal_lowerings_match_owning_predecessors_and_drop_every_receiver() {
    let owner = FunctionExecutionId::Monomorphic(DeclarationId::new("lowering.λ\0owner"));
    let span = crate::ast::Span::default();
    for element in [ResolvedType::I64, ResolvedType::I32, ResolvedType::U8, ResolvedType::Usize, ResolvedType::Bool, ResolvedType::Char, ResolvedType::F32, ResolvedType::F64] {
        let binding = hir::ResolvedBinding { id: ValueId::local(&owner, "source"), name: "source".into(), ty: crate::vec_ops::resolved_vec(element.clone()), ownership: OwnershipMode::Borrow, span };
        let user = expr(&owner, "user", ResolvedType::I64, OwnershipMode::Value);
        let current = || hir::resolve_for::lower(&owner, "stmt.λ\0", "item", span, &binding, element.clone(), user.clone(), span);
        let old = || hir::resolve_for::identity_before::lower(&owner, "stmt.λ\0", "item", span, &binding, element.clone(), user.clone(), span);
        assert_eq!(current(), old(), "entire Copy traversal statement");
        let ((), new_metrics) = measure(|| drop(current()));
        let ((), old_metrics) = measure(|| drop(old()));
        assert!(!new_metrics.invalid && !old_metrics.invalid);
        assert_eq!(new_metrics.live, 0);
        assert_eq!(old_metrics.live, 0);
        assert_eq!(new_metrics.allocated, new_metrics.freed);
        assert_eq!(old_metrics.allocated, old_metrics.freed);
        assert!(new_metrics.allocated < old_metrics.allocated);
        let source = expr(&owner, "iterator", crate::iterator_ops::resolved_iter(element.clone()), OwnershipMode::Own);
        let current = || hir::resolve_for_own::lower(&owner, "own.λ\0", "item", span, source.clone(), element.clone(), user.clone(), span);
        let old = || hir::resolve_for_own::identity_before::lower(&owner, "own.λ\0", "item", span, source.clone(), element.clone(), user.clone(), span);
        assert_eq!(current(), old(), "entire consuming traversal statement");
        let ((), new_metrics) = measure(|| drop(current()));
        let ((), old_metrics) = measure(|| drop(old()));
        assert!(!new_metrics.invalid && !old_metrics.invalid);
        assert_eq!((new_metrics.live, old_metrics.live), (0, 0));
        assert_eq!(new_metrics.allocated, new_metrics.freed);
        assert_eq!(old_metrics.allocated, old_metrics.freed);
        assert!(new_metrics.allocated < old_metrics.allocated);
    }
}

fn old_program(program: &crate::Program) -> Result<hir::ResolvedProgram, Vec<crate::diagnostic::Diagnostic>> {
    let diagnostics = crate::source_verify::verify(program);
    if diagnostics.iter().any(|diagnostic| diagnostic.severity.is_error()) { return Err(diagnostics); }
    let (result, count) = hir::resolve_statement::text_reference::with(|| hir::resolve_program::agent_identity_original::resolve_verified_original(program, None).map(|(resolved, _)| resolved).map_err(|error| vec![error]));
    assert!(count > 0);
    assert!(!hir::resolve_statement::text_reference::active());
    result
}

#[test]
fn real_expression_routes_keep_complete_hir_wire_graph_plans_and_diagnostics_against_recursive_oracle() {
    // Preserve the complete admitted fixture and expose the frozen resolver's
    // arithmetic limitation instead of changing that independent oracle.
    let traversal = include_str!("../../../tests/fixtures/byte-slice-traversal.spx");
    let traversal_ast = crate::check(traversal, "lowering-identity.spx").unwrap();
    let traversal_current = hir::resolve(&traversal_ast).unwrap();
    hir::validate(&traversal_current).unwrap();
    crate::loan_plan::validate_program(&traversal_current).unwrap();
    let traversal_again = hir::resolve(&traversal_ast).unwrap();
    assert_eq!(traversal_current, traversal_again);
    assert_eq!(crate::cache_codec::encode(&traversal_current).unwrap(), crate::cache_codec::encode(&traversal_again).unwrap());
    let traversal_revision = crate::graph::revision(&traversal_ast);
    assert_eq!(crate::graph::to_hir_json(&traversal_current, &traversal_revision).unwrap(), crate::graph::to_hir_json(&traversal_again, &traversal_revision).unwrap());
    assert_eq!(old_program(&traversal_ast).unwrap_err(), vec![crate::diagnostic::Diagnostic {
        code: "SPX-U102",
        severity: crate::diagnostic::Severity::Error,
        message: "assigned value type `i64` does not exactly match binding type `usize`".into(),
        path: Some("lowering-identity.spx".into()),
        span: Some(crate::ast::Span { start: 659, end: 673, line: 26, column: 17 }),
        help: None,
    }]);
    let increment = "index = index + 1usize;";
    let compatible_increment = "index = usize_from_i64(i64_from_usize(index) + 1);";
    assert_eq!(traversal.matches(increment).count(), 2);
    assert_eq!(traversal.matches(compatible_increment).count(), 0);
    let compatible_traversal = traversal.replace(increment, compatible_increment);
    assert_eq!(compatible_traversal.replace(compatible_increment, increment), traversal);
    // Only these two exact increment expressions differ. The full fixture,
    // including every traversal/control-flow body, enters the paired oracle.
    let owned = include_str!("../../../tests/fixtures/owned-leaf-collections.spx");
    let owned_ast = crate::check(owned, "lowering-identity.spx").unwrap();
    let owned_current = hir::resolve(&owned_ast).unwrap();
    hir::validate(&owned_current).unwrap();
    crate::loan_plan::validate_program(&owned_current).unwrap();
    assert_eq!(old_program(&owned_ast).unwrap_err(), vec![crate::diagnostic::Diagnostic {
        code: "SPX-H006",
        severity: crate::diagnostic::Severity::Error,
        message: "expression `declaration:15:owned.leaf.main:expression:34:body.s11.value.s0.value.arg.0.arg0` has a non-canonical identity".into(),
        path: None,
        span: None,
        help: None,
    }]);
    use hir::resolve_statement::text_reference as reference;
    reference::assert_canonical_iterator_reference_source();
    assert!(!reference::canonical_iterator_path_active());
    reference::with_canonical_iterator_path(|| {
        assert!(reference::canonical_iterator_path_active());
        reference::with_canonical_iterator_path(|| assert!(reference::canonical_iterator_path_active()));
        assert!(reference::canonical_iterator_path_active());
    });
    assert!(!reference::canonical_iterator_path_active());
    assert!(std::panic::catch_unwind(|| reference::with_canonical_iterator_path(|| panic!("canonical reference unwind"))).is_err());
    assert!(!reference::canonical_iterator_path_active());
    // Only the owned fixture uses the separately labelled corrected reference.
    // Its whole frozen original rejection above remains an independent result;
    // this corrected-reference parity makes no old allocation/cost claim.
    for text in [
        include_str!("../../../tests/fixtures/projected-string-views.spx"),
        compatible_traversal.as_str(),
        include_str!("../../../tests/fixtures/owned-leaf-collections.spx"),
        r#"module test.lowering_effects;
permit { process.stdout.write }
@id("lowering.write") fn write(view: borrow Slice<u8>) -> usize uses { process.stdout.write } { stdout_write(view) }
@id("lowering.option") fn option(value: Option<i64>) -> Option<bool> { let checked = value?; Option<bool>::Some { value: checked > 0 } }
@id("lowering.main") fn main() -> i64 { 0 }
"#,
    ] {
        let ast = crate::check(text, "lowering-identity.spx").unwrap();
        let current = hir::resolve(&ast).unwrap();
        let old = if text == owned {
            reference::with_canonical_iterator_path(|| old_program(&ast)).unwrap()
        } else {
            old_program(&ast).unwrap()
        };
        assert!(!reference::canonical_iterator_path_active());
        hir::validate(&current).unwrap();
        hir::validate(&old).unwrap();
        crate::loan_plan::validate_program(&current).unwrap();
        crate::loan_plan::validate_program(&old).unwrap();
        assert_eq!(current, old);
        assert_eq!(crate::cache_codec::encode(&current).unwrap(), crate::cache_codec::encode(&old).unwrap());
        let revision = crate::graph::revision(&ast);
        assert_eq!(crate::graph::to_hir_json(&current, &revision).unwrap(), crate::graph::to_hir_json(&old, &revision).unwrap());
    }
    let original = include_str!("../../../tests/fixtures/projected-string-views.spx");
    for replacement in ["str_len_bytes(0)", "str_len_bytes(text, text)"] {
        let changed = original.replace("str_len_bytes(text)", replacement);
        assert_ne!(changed, original);
        let ast = crate::parse(&changed, std::path::Path::new("lowering-invalid.spx")).unwrap();
        let current = hir::resolve(&ast).unwrap_err();
        let old = old_program(&ast).unwrap_err();
        let corrected = reference::with_canonical_iterator_path(|| old_program(&ast)).unwrap_err();
        assert!(!reference::canonical_iterator_path_active());
        assert_eq!(corrected, old, "corrected iterator spelling cannot change foreign call refusal");
        assert_eq!(current, old, "every original diagnostic field and refusal priority");
    }
}

#[test]
fn borrowed_invariant_query_preserves_complete_admission_and_foreign_precedence() {
    let text = include_str!("../../../tests/fixtures/nested-collection-records.spx");
    let ast = crate::check(text, "lowering-record.spx").unwrap();
    let program = hir::resolve(&ast).unwrap();
    for name in ["collection.report", "foreign.λ\0", "", "collection.report#invariant"] {
        let ty = ResolvedType::Nominal { declaration: DeclarationId::new(name), arguments: vec![] };
        assert_eq!(hir::owned_collection_record::admitted(&ty, &program.declarations), hir::owned_collection_record::identity_before::admitted(&ty, &program.declarations));
    }
    let ty = ResolvedType::Nominal { declaration: DeclarationId::new("collection.report"), arguments: vec![] };
    assert!(hir::owned_collection_record::admitted(&ty, &program.declarations));
    let (current, current_metrics) = measure(|| hir::owned_collection_record::admitted(&ty, &program.declarations));
    let (old, old_metrics) = measure(|| hir::owned_collection_record::identity_before::admitted(&ty, &program.declarations));
    assert_eq!(current, old);
    assert!(!current_metrics.invalid && !old_metrics.invalid);
    assert_eq!((current_metrics.live, old_metrics.live), (0, 0));
    assert_eq!(current_metrics.allocated, current_metrics.freed);
    assert_eq!(old_metrics.allocated, old_metrics.freed);
    assert!(current_metrics.allocated < old_metrics.allocated);
    for kind in [hir::DeclarationKind::Resource, hir::DeclarationKind::Record] {
        let mut forged = program.declarations.clone();
        let id = DeclarationId::new("collection.report#invariant");
        forged.declarations.insert(id.clone(), hir::Declaration { id, name: "foreign reserved suffix".into(), kind, identity_origin: hir::IdentityOrigin::Explicit, owner: None });
        assert!(!hir::owned_collection_record::admitted(&ty, &forged));
        assert_eq!(hir::owned_collection_record::admitted(&ty, &forged), hir::owned_collection_record::identity_before::admitted(&ty, &forged));
    }
}

#[test]
fn complete_source_inverses_and_independent_old_lowering_bodies_remain_exact() {
    hir::resolve_statement::text_reference::assert_canonical_iterator_reference_source();
    let sources = [
        ("src/hir/ids/compiler.rs", include_str!("compiler.rs")),
        ("src/hir/resolve_expr.rs", include_str!("../resolve_expr.rs")),
        ("src/hir/resolve_for.rs", include_str!("../resolve_for.rs")),
        ("src/hir/resolve_for/bytes.rs", include_str!("../resolve_for/bytes.rs")),
        ("src/hir/resolve_for_own.rs", include_str!("../resolve_for_own.rs")),
        ("src/hir/owned_collection_record.rs", include_str!("../owned_collection_record.rs")),
        ("src/hir/ids.rs", include_str!("../ids.rs")),
        ("src/hir/ids/fixed_operation_tests.rs", include_str!("fixed_operation_tests.rs")),
        ("src/hir/ids/declaration_tag.rs", include_str!("declaration_tag.rs")),
    ];
    let originals = sources.map(|(path, source)| (path, prior_source(path, source)));
    let old = &originals[2].1;
    let old = &old[old.find("fn place(").unwrap()..];
    let old = old.replace("pub(super) fn lower(", "pub(in crate::hir) fn lower(").replace("pub(super) fn lower_profile(", "pub(in crate::hir) fn lower_profile(").replace("pub(super) struct Profile", "pub(in crate::hir) struct Profile").replace("pub(super) length_", "pub(in crate::hir) length_").replace("pub(super) wrapper:", "pub(in crate::hir) wrapper:").replace("pub(super) length:", "pub(in crate::hir) length:").replace("pub(super) item:", "pub(in crate::hir) item:").replace("pub(super) kind:", "pub(in crate::hir) kind:").replace("super::SourceForKind", "crate::hir::SourceForKind").replace("super::resolve_expr::", "crate::hir::resolve_expr::").replace("super::LoopLatchKind::", "crate::hir::LoopLatchKind::");
    assert_eq!(include_str!("../resolve_for/identity_before.rs").strip_prefix("//! Complete immediate old traversal helpers; namespace-only access changes.\nuse super::*;\n\n").unwrap(), old);
    let old = &originals[4].1;
    let old = &old[old.find("#[allow(clippy::too_many_arguments)]\nfn lower(").unwrap()..old.find("\n#[cfg(test)]\n#[allow(clippy::too_many_arguments)]\npub(super) fn resolve_reference").unwrap()];
    assert_eq!(include_str!("../resolve_for_own/identity_before.rs").strip_prefix("//! Entire immediate old consuming traversal receiver; only visibility differs.\nuse super::*;\n\n").unwrap(), old.replacen("fn lower(", "pub(in crate::hir) fn lower(", 1).replace("super::resolve_expr::", "crate::hir::resolve_expr::").replace("super::LoopLatchKind::", "crate::hir::LoopLatchKind::"));
    let old = &originals[5].1;
    let old = &old[old.find("pub(crate) fn admitted(").unwrap()..old.find("\n/// Negative frozen-profile guard").unwrap()];
    assert_eq!(include_str!("../owned_collection_record/identity_before.rs").strip_prefix("//! Complete original admission predicate, including owning invariant query.\nuse super::*;\n\n").unwrap(), old);
    let proof: serde_json::Value = serde_json::from_str(include_str!("lowering_identity_inverse.json")).unwrap();
    assert_eq!(sha(include_str!("../resolve_expr_reference.rs").as_bytes()), proof["unchanged_recursive_sha256"].as_str().unwrap());
    for (path, source) in sources {
        if !matches!(path, "src/hir/ids.rs" | "src/hir/ids/declaration_tag.rs" | "src/hir/ids/fixed_operation_tests.rs") { assert!(!source.contains("DeclarationId::new("), "unclosed constructor in {path}"); }
    }
    for hostile in [format!("{} ", sources[0].1), sources[0].1[..sources[0].1.len() - 1].to_owned(), originals[0].1.clone()] {
        assert!(std::panic::catch_unwind(|| prior_source(sources[0].0, &hostile)).is_err());
    }
}
