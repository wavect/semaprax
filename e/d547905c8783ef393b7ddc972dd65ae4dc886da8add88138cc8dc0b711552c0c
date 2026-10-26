//! Complete immediate-old forecast and real shared-backing controls.
use super::super::{
    before_shared_local_projection as before, cached_exact, dependency_identity_max,
    held_projection,
};
use super::*;
use crate::hir::{ResolvedExpr, ResolvedExprKind, ResolvedProgram};
use sha2::{Digest, Sha256};

#[path = "controls_tests.rs"]
mod controls;

fn source(calls: usize) -> String {
    let body = std::iter::repeat_n("helper(1)", calls)
        .collect::<Vec<_>>()
        .join(" + ");
    format!("module shared.local; @id(\"shared.helper\") fn helper(x:i64)->i64{{x}} @id(\"shared.main\") fn main()->i64{{{body}}}")
}

#[test]
fn full_immediate_old_projection_keeps_all_modes_and_exact_new_invoice() {
    for count in [1, 64] {
        let source = source(count);
        let programs = vec![crate::parse(&source, "shared.spx").unwrap()];
        let authored = crate::workspace_graph::index_authored(&programs).unwrap();
        let program = &programs[0];
        let maximum = dependency_identity_max(program, &authored, &programs).unwrap();
        let proof = costs(Context {
            program,
            authored: &authored,
            programs: &programs,
        })
        .unwrap();
        assert_eq!(proof.slots, count);
        assert!(proof.bytes > 0);
        for mode in 0..=7 {
            for retention in [
                cached_exact::Retention::Parsed,
                cached_exact::Retention::Semantic,
            ] {
                for imports in [false, true] {
                    let old = before::synthetic_builder_bytes_scoped_with_retention_with_registry(
                        program,
                        &authored,
                        &programs,
                        Some(maximum),
                        imports,
                        (mode, retention),
                        None,
                    )
                    .unwrap();
                    let now = held_projection::synthetic_builder_bytes_scoped_with_retention_with_registry(
                        program, &authored, &programs, Some(maximum), imports, (mode, retention), None).unwrap();
                    let (fee, removed) =
                        if mode == 6 && retention == cached_exact::Retention::Parsed {
                            (
                                proof.bytes,
                                proof.slots * maximum * super::super::HIR_IDENTITY_COPY_FACTOR,
                            )
                        } else {
                            (0, 0)
                        };
                    assert_eq!(now.retained_hir + removed, old.retained_hir + fee);
                    assert_eq!(
                        now.retained_clone_and_hir + removed,
                        old.retained_clone_and_hir + fee
                    );
                }
            }
        }
    }
}

fn shared_calls(expression: &ResolvedExpr, program: &ResolvedProgram) -> usize {
    let mut count = 0;
    match &expression.kind {
        ResolvedExprKind::Call {
            callee,
            type_arguments,
            instance,
            args,
            ..
        } => {
            assert!(type_arguments.is_empty());
            assert!(instance.is_none());
            let declaration = program
                .functions
                .iter()
                .find(|function| function.id == *callee)
                .unwrap();
            let held = program.declarations.function_id(&declaration.name).unwrap();
            assert!(callee.shared_allocation_key().is_some());
            assert_eq!(callee.shared_allocation_key(), held.shared_allocation_key());
            assert_eq!(
                callee.shared_allocation_key(),
                declaration.id.shared_allocation_key()
            );
            assert_eq!(
                callee.owned_allocation_bytes(),
                held.owned_allocation_bytes()
            );
            count += 1;
            for argument in args {
                count += shared_calls(argument, program);
            }
        }
        ResolvedExprKind::Block { statements, tail } => {
            assert!(statements.is_empty());
            count += shared_calls(tail, program);
        }
        ResolvedExprKind::Binary { left, right, .. } => {
            count += shared_calls(left, program);
            count += shared_calls(right, program);
        }
        ResolvedExprKind::Int(_) | ResolvedExprKind::Place(_) => {}
        other => panic!("closed fixture added unowned HIR shape: {other:?}"),
    }
    count
}

#[test]
fn tiny_repeated_proof_and_whole_resolver_backing_are_measured_through_drop() {
    for count in [1, 64] {
        let source = source(count);
        let programs = vec![crate::parse(&source, "shared.spx").unwrap()];
        let authored = crate::workspace_graph::index_authored(&programs).unwrap();
        let context = Context {
            program: &programs[0],
            authored: &authored,
            programs: &programs,
        };
        // Budget harness setup precedes observation; the complete proof,
        // captured ledger loan, header nodes and their Drop remain inside.
        let ((proof, proof_alloc), refused, charged, first) =
            crate::bounded_output::with_limit_usage_refusal(usize::MAX, || {
                crate::test_allocation_observer::measure(|| costs(context).unwrap())
            });
        assert!(!refused);
        assert!(first.is_none());
        assert_eq!(proof.slots, count);
        assert_eq!(proof.bytes, charged);
        assert!(!proof_alloc.invalid);
        assert_eq!(proof_alloc.live, 0);
        assert_eq!(proof_alloc.allocated, proof_alloc.freed);
        assert!(proof_alloc.highwater <= charged);
        let checked = crate::check(&source, "shared.spx").unwrap();
        let ((actual, after_resolve, after_validate), allocation) =
            crate::test_allocation_observer::measure_with_capacity(1 << 16, || {
                let resolved = crate::hir::resolve(&checked).unwrap();
                let after_resolve = crate::test_allocation_observer::current_metrics();
                crate::hir::validate(&resolved).unwrap();
                let after_validate = crate::test_allocation_observer::current_metrics();
                let actual = resolved
                    .functions
                    .iter()
                    .filter(|function| function.id.as_str().starts_with("shared."))
                    .map(|function| shared_calls(&function.body, &resolved))
                    .sum::<usize>();
                drop(resolved);
                (actual, after_resolve, after_validate)
            });
        assert_eq!(actual, count);
        assert!(!allocation.invalid);
        assert_eq!(allocation.live, 0);
        assert_eq!(allocation.allocated, allocation.freed);
        let maximum = Some(dependency_identity_max(context.program, &authored, &programs).unwrap());
        let old = before::synthetic_builder_bytes_scoped_with_retention_with_registry(
            context.program,
            &authored,
            &programs,
            maximum,
            true,
            (6, cached_exact::Retention::Parsed),
            None,
        )
        .unwrap();
        let now = held_projection::synthetic_builder_bytes_scoped_with_retention_with_registry(
            context.program,
            &authored,
            &programs,
            maximum,
            true,
            (6, cached_exact::Retention::Parsed),
            None,
        )
        .unwrap();
        assert!(allocation.highwater <= now.retained_hir);
        eprintln!("LOCAL_CALLEE_SHARED_BACKING calls={count} proof_slots={} proof_charged={charged} proof_allocation={proof_alloc:?} before_forecast={} current_forecast={} after_resolve={after_resolve:?} after_validate={after_validate:?} whole_after_drop={allocation:?}; requested layouts and conservative reservations are distinct; no RSS or application admission claim", proof.slots, old.retained_hir, now.retained_hir);
    }
}

#[test]
fn immediate_old_bytes_and_serialized_phase_boundary_remain_explicit() {
    let old903 = crate::source_reflows::next_custody_before("constructor903", "src/workspace_graph/expected_projection/shared_local_callees/before_projection.rs", include_str!("before_projection.rs"));
    let old = old903.as_str();
    assert_eq!(
        format!(
            "{:x}",
            crate::digest_hex::LowerHex(Sha256::digest(old.as_bytes()))
        ),
        "3e0d38a5c01f87a8b05815d9831f80b2604df5e37c15d0dcbf0cf0055656e942"
    );
    let current = include_str!("../held_projection.rs");
    let uncached = include_str!("../uncached_peak.rs");
    let source_callables = include_str!("../../source_callables.rs");
    let core = include_str!("../../../workspace_graph.rs");
    assert!(current.contains("layout_mode == 6 && retention == cached_exact::Retention::Parsed"));
    assert!(current.contains("checked_sub(shared_callees.slots)"));
    assert!(current.contains("checked_mul(HIR_IDENTITY_COPY_FACTOR)"));
    assert!(uncached.contains("program, authored, programs, maximum, 6, true, registry"));
    assert!(
        source_callables
            .find("synthetic_builder_bytes(program, &authored, &programs)")
            .unwrap()
            < source_callables
                .find("graph::to_hir_json(&resolved, defining_revision)")
                .unwrap()
    );
    assert!(core.contains("retain_workspace_module("));
    assert!(!core.contains("graph::to_hir_json("));
    // All physical helper banks stay joined to the source owner.
    for child in [
        include_str!("../shared_local_callees.rs"),
        include_str!("invoice.rs"),
        include_str!("walk.rs"),
    ] {
        assert!(!child.is_empty());
    }
}

#[path = "constructor_tests.rs"]
mod constructors;
