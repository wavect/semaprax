//! Real public resolution compared with the full independent Owned constructor.
use super::*;
fn current_after_validation(
    program: &Program,
) -> Result<(ResolvedProgram, FunctionResolutionWork), Diagnostic> {
    // Full frozen receiver with the independently saved complete index. No
    // shared constructor, payload oracle, schema substitute, or partial view.
    (Resolver {
        program,
        byte_helpers: crate::loop_calls::byte_writer::SourceIndex::new(program),
        declarations: DeclarationIndex::from_verified_with_prelude(
            program,
            crate::prelude::declarations_for_program(program),
        )?,
        reuse: None,
        function_work: FunctionResolutionWork::default(),
        fused_producer: None,
        partition_profile: false,
    })
    .resolve_agent_identity_original()
}
#[test]
fn public_legacy_nominal_postpass_keeps_full_hir_graph_cache_and_isolated_mutation() {
    let source = SOURCE
        .replace("variant Choice {", "variant Choice<T> {")
        .replace("@id(\"choice.left.x\") x:i64", "@id(\"choice.left.x\") x:T");
    let program = crate::check(&source, "legacy-nominal.spx").unwrap();
    let (original, old_work) = old(&program).unwrap();
    let (_, current_work) = current_after_validation(&program).unwrap();
    assert_eq!(old_work.reused(), current_work.reused());
    assert_eq!(old_work.static_text(), current_work.static_text());
    assert_eq!(old_work.costs(), current_work.costs());
    let mut current = crate::hir::resolve(&program).unwrap();
    assert_eq!(current, original);
    assert_eq!(
        crate::cache_codec::encode(&current).unwrap(),
        crate::cache_codec::encode(&original).unwrap()
    );
    let revision = crate::graph::revision(&program);
    assert_eq!(
        crate::graph::to_hir_json(&current, &revision).unwrap(),
        crate::graph::to_hir_json(&original, &revision).unwrap()
    );
    let bytes = crate::cache_codec::encode(&current).unwrap();
    let decoded: ResolvedProgram = crate::cache_codec::decode(&bytes).unwrap();
    assert_eq!(decoded, original);
    for ty in &decoded.types {
        assert!(!ty.type_parameters.has_shared_owner());
    }
    let mut sibling = current.clone();
    for declaration in &current.types {
        assert!(!declaration.type_parameters.is_shared());
        if !declaration.type_parameters.is_empty() {
            assert!(declaration.type_parameters.same_owner(
                current
                    .declarations
                    .parameter_payload(&declaration.id)
                    .unwrap()
            ));
        }
        match &declaration.kind {
            ResolvedTypeDeclarationKind::Record { fields }
            | ResolvedTypeDeclarationKind::Class { fields, .. } => {
                if !fields.is_empty() {
                    assert!(fields.same_owner(
                        current
                            .declarations
                            .record_payload(&declaration.id)
                            .unwrap()
                    ));
                }
            }
            ResolvedTypeDeclarationKind::Variant { cases } => {
                if !cases.is_empty() {
                    assert!(cases
                        .same_owner(current.declarations.case_payload(&declaration.id).unwrap()));
                }
                for case in cases {
                    assert_eq!(
                        case.fields.as_ptr(),
                        current.declarations.case_fields(&case.id).unwrap().as_ptr()
                    );
                }
            }
            _ => {}
        }
    }
    let row_id = DeclarationId::new("row.type");
    let row = current.types.iter_mut().find(|d| d.id == row_id).unwrap();
    let ResolvedTypeDeclarationKind::Record { fields } = &mut row.kind else {
        panic!("record")
    };
    fields[0].name.push_str(".mutated");
    assert_eq!(
        current.declarations.record_fields(&row_id).unwrap()[0].name,
        "x"
    );
    let sibling_row = sibling.types.iter().find(|d| d.id == row_id).unwrap();
    let ResolvedTypeDeclarationKind::Record { fields } = &sibling_row.kind else {
        panic!("record")
    };
    assert_eq!(fields[0].name, "x");
    current.declarations.record_fields.get_mut(&row_id).unwrap()[0]
        .name
        .push_str(".index");
    let row = current.types.iter().find(|d| d.id == row_id).unwrap();
    let ResolvedTypeDeclarationKind::Record { fields } = &row.kind else {
        panic!("record")
    };
    assert_eq!(fields[0].name, "x.mutated");
    assert_eq!(
        current.declarations.record_fields(&row_id).unwrap()[0].name,
        "x.index"
    );
    let choice_id = DeclarationId::new("choice.type");
    let choice = sibling
        .types
        .iter_mut()
        .find(|d| d.id == choice_id)
        .unwrap();
    for p in &mut choice.type_parameters {
        p.name.push_str(".changed");
    }
    let ResolvedTypeDeclarationKind::Variant { cases } = &mut choice.kind else {
        panic!("variant")
    };
    for case in cases {
        for field in &mut case.fields {
            field.name.push_str(".changed");
        }
    }
    let case_id = DeclarationId::new("choice.left");
    assert_eq!(
        sibling.declarations.case_fields(&case_id).unwrap()[0].name,
        "x"
    );
    // Index-side legacy case/record mutation also detaches from both HIR owners.
    current.declarations.case_fields.get_mut(&case_id).unwrap()[0]
        .name
        .push_str(".index");
    assert_eq!(
        sibling.declarations.case_fields(&case_id).unwrap()[0].name,
        "x"
    );
    assert_eq!(
        original.declarations.case_fields(&case_id).unwrap()[0].name,
        "x"
    );
}
#[test]
fn legacy_postpass_keeps_full_original_diagnostics_before_any_canonicalization() {
    for source in [
        SOURCE.replace("Child : Base", "Child : Missing"),
        SOURCE.replace("class Base {", "class Base : Child {"),
        SOURCE.replace("@id(\"child.read\")", "@id(\"base.read\")"),
        "module legacy.recursive; @id(\"r\") record R {@id(\"r.x\") x:R,}".to_owned(),
    ] {
        let program = crate::parse(&source, "legacy-invalid.spx").unwrap();
        let prelude = crate::prelude::declarations_for_program(&program);
        let old = DeclarationIndex::nominal_original_with_prelude(&program, prelude).unwrap_err();
        let current = DeclarationIndex::from_verified_with_prelude(&program, prelude).unwrap_err();
        assert_eq!(current, old, "all six fields and original first error");
    }
}
#[test]
fn complete_owned_baseline_and_public_legacy_sharing_report_real_allocation_and_drop() {
    let program = crate::check(SOURCE, "legacy-cost.spx").unwrap();
    for current in [false, true] {
        for first in [false, true] {
            let ((clone_allocated, clone_peak, before_allocated), observed) = measure(|| {
                let (
                    (clone_allocated, clone_peak, before_allocated),
                    refused,
                    reserved,
                    first_refusal,
                ) = crate::bounded_output::with_limit_usage_refusal(usize::MAX, || {
                    // The saved original builds Owned independently and bypasses
                    // the postpass. Its existing Clone population is unchanged.
                    let resolved = if current {
                        current_after_validation(&program).unwrap().0
                    } else {
                        old(&program).unwrap().0
                    };
                    let before = current_metrics();
                    let sibling = resolved.clone();
                    let after = current_metrics();
                    if first {
                        drop(resolved);
                        drop(sibling);
                    } else {
                        drop(sibling);
                        drop(resolved);
                    }
                    (
                        after.allocated - before.allocated,
                        after.highwater,
                        before.allocated,
                    )
                });
                assert!(!refused && first_refusal.is_none());
                (clone_allocated, clone_peak, (before_allocated, reserved))
            });
            eprintln!("legacy_nominal current={current} first={first} clone_allocated={clone_allocated} clone_peak={clone_peak} before_and_reserved={before_allocated:?} full={observed:?}");
            assert!(!observed.invalid);
            assert_eq!(observed.live, 0);
            assert_eq!(observed.allocated, observed.freed);
        }
    }
}
