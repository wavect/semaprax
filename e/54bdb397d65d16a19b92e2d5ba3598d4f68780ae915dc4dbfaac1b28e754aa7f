//! Whole constructor/forecast controls. These observations grant no admission.
use super::super::before903::{self, before_local_projection as original};
use super::*;
use crate::bounded_output::with_limit_usage_refusal;
use crate::test_allocation_observer::{measure_with_capacity, Metrics};

fn inputs(case: usize) -> Vec<Program> {
    if case < 2 {
        return vec![crate::parse(&source(if case == 0 { 1 } else { 64 }), "shared.spx").unwrap()];
    }
    vec![
        crate::parse("module shared.local; use function @id(\"provider.remote\") from provider as remote; @id(\"shared.helper\") fn helper(x:i64)->i64{x} @id(\"shared.main\") fn main()->i64{helper(1)+remote(2)}", "shared.spx").unwrap(),
        crate::parse("module provider; @id(\"provider.remote\") fn remote(x:i64)->i64{x}", "provider.spx").unwrap(),
    ]
}

#[test]
fn frozen_full_projection_preserves_every_mode_import_and_proof_fee() {
    assert_eq!(before903::OLD_PROOF_BYTES, 2 * size_of::<usize>());
    assert_eq!(before903::OLD_COST_BYTES, 6 * size_of::<usize>());
    for case in 0..3 {
        let programs = inputs(case);
        let authored = crate::workspace_graph::index_authored(&programs).unwrap();
        let program = &programs[0];
        let maximum = dependency_identity_max(program, &authored, &programs).unwrap();
        let proof = costs(Context {
            program,
            authored: &authored,
            programs: &programs,
        })
        .unwrap();
        for imports in [false, true] {
            let (old_roots, old_fee) =
                before903::before_local_carrier_costs(program, &authored, imports);
            let new_carrier =
                super::super::super::call_carriers::count(program, &authored, imports);
            assert_eq!(new_carrier.roots, old_roots);
            for mode in 0..=7 {
                for retention in [
                    cached_exact::Retention::Parsed,
                    cached_exact::Retention::Semantic,
                ] {
                    let old =
                        original::synthetic_builder_bytes_scoped_with_retention_with_registry(
                            program,
                            &authored,
                            &programs,
                            Some(maximum),
                            imports,
                            (mode, retention),
                            None,
                        )
                        .unwrap();
                    let current = held_projection::synthetic_builder_bytes_scoped_with_retention_with_registry(
                        program, &authored, &programs, Some(maximum), imports, (mode, retention), None).unwrap();
                    let eligible = mode == 6 && retention == cached_exact::Retention::Parsed;
                    let removed = if eligible {
                        proof.slots * maximum * super::super::super::HIR_IDENTITY_COPY_FACTOR
                    } else {
                        0
                    };
                    let new_fee = if eligible { proof.bytes } else { 0 };
                    let (old_census, current_census) = if mode >= 5 {
                        (old_fee, new_carrier.bytes)
                    } else {
                        (0, 0)
                    };
                    // The genuine old ABI is never silently repriced by current Proof/Costs.
                    assert_eq!(
                        current.retained_hir + removed + old_census,
                        old.retained_hir + new_fee + current_census
                    );
                    assert_eq!(
                        current.retained_clone_and_hir + removed + old_census,
                        old.retained_clone_and_hir + new_fee + current_census
                    );
                    assert_eq!(
                        current.raw_clone_and_hir + removed + old_census,
                        old.raw_clone_and_hir + new_fee + current_census
                    );
                    assert_eq!(
                        (
                            current.runtime,
                            current.synthetic_ast,
                            current.transient_import_clone
                        ),
                        (old.runtime, old.synthetic_ast, old.transient_import_clone)
                    );
                }
            }
        }
    }
}

fn constructors(resolved: &ResolvedProgram, expected: usize) {
    let declaration = resolved
        .functions
        .iter()
        .find(|f| f.id.as_str() == "shared.helper")
        .unwrap();
    let key = declaration.id.shared_allocation_key();
    assert!(key.is_some());
    let indexed = resolved.declarations.function_id("helper").unwrap();
    assert_eq!(key, indexed.shared_allocation_key());
    assert_eq!(
        key,
        resolved
            .declarations
            .declaration(indexed)
            .unwrap()
            .id
            .shared_allocation_key()
    );
    // A function's callee is not a nominal type declaration. No per-call type,
    // field, case, default or fresh ExprID is subtracted by this certificate.
    assert!(resolved.declarations.type_id("helper").is_none());
    assert!(resolved.types.iter().all(|ty| ty.id != declaration.id));
    let actual: usize = resolved
        .functions
        .iter()
        .filter(|f| f.id.as_str().starts_with("shared."))
        .map(|f| shared_calls(&f.body, resolved))
        .sum();
    assert_eq!(actual, expected);
    crate::hir::validate(resolved).unwrap();
    let calls = crate::call_index::PersistentCallIndex::build(resolved).unwrap();
    let mut persistent = 0;
    for targets in calls.calls_by_owner().values() {
        for callee in targets {
            if callee == &declaration.id {
                assert_eq!(key, callee.shared_allocation_key());
                persistent += 1;
            }
        }
    }
    assert!(persistent > 0);
    let reverse = calls
        .callers_by_callee()
        .keys()
        .find(|callee| **callee == declaration.id)
        .unwrap();
    assert_eq!(key, reverse.shared_allocation_key());
    let edges = crate::hir::workspace_call_edges(resolved);
    let mut retained = 0;
    for (_, callee) in &edges {
        if callee == &declaration.id {
            assert_eq!(key, callee.shared_allocation_key());
            retained += 1;
        }
    }
    assert!(retained > 0);
    let mut propagated = 0;
    for function in &resolved.functions {
        // The real resolver already constructed both inventories. A callee is
        // retained by PropagatedCall, while storage origins remain independent.
        // These fixtures have only scalar parameters. Check the complete
        // entry states independently: inventory storage IDs and plan places
        // deliberately have different representations.
        assert!(function
            .cleanup
            .entry_state
            .live_owned_parameters
            .is_empty());
        assert!(function
            .cleanup
            .entry_state
            .conditional_owned_parameters
            .is_empty());
        assert!(function
            .cleanup_plan
            .entry_state
            .live_owned_parameters
            .is_empty());
        assert!(function
            .cleanup_plan
            .entry_state
            .conditional_owned_parameters
            .is_empty());
        if function.id.as_str().starts_with("shared.") {
            // These real scalar-only constructors create no owned storage.
            // The inventory therefore adds no independent callee identity;
            // the call status below retains the shared identity separately.
            assert!(function.cleanup.slots.is_empty());
            assert!(function.cleanup.flags.is_empty());
            assert!(function
                .cleanup
                .entry_state
                .live_owned_parameters
                .is_empty());
            assert!(function
                .cleanup
                .entry_state
                .conditional_owned_parameters
                .is_empty());
        }
        for source in &function.cleanup_plan.status_sources {
            if let crate::cleanup_plan::StatusProducer::PropagatedCall { callee } = &source.producer
            {
                if callee == &declaration.id {
                    assert_eq!(key, callee.shared_allocation_key());
                    assert_ne!(source.id.expression.as_str(), callee.as_str());
                    propagated += 1;
                }
            }
        }
    }
    assert!(propagated >= expected);
    drop((calls, edges));
}

#[test]
fn eight_consumer_constructor_frontiers_keep_shared_and_independent_storage() {
    for count in [1, 64] {
        let source = source(count);
        let checked = crate::check(&source, "shared.spx").unwrap();
        let ((), allocations) = measure_with_capacity(1 << 16, || {
            let resolved = crate::hir::resolve(&checked).unwrap();
            constructors(&resolved, count);
            drop(resolved);
        });
        settled(allocations);
        eprintln!("LOCAL_CALLEE_EIGHT_CONSTRUCTORS calls={count} whole_after_drop={allocations:?}; requested allocator backing, not RSS or application admission");
    }
    // All eight source families are joined completely. These checks distinguish
    // the actual constructors; they do not replace the backing checks above.
    let families = [
        (
            "HIR",
            include_str!("../../../hir/resolve_expr.rs"),
            "Frame::FinishCall",
        ),
        (
            "declaration index",
            include_str!("../../../hir/declaration_index.rs"),
            "functions_by_name: BTreeMap<String, DeclarationId>",
        ),
        (
            "type index",
            include_str!("../../../hir/declaration_index.rs"),
            "types_by_name: BTreeMap<String, DeclarationId>",
        ),
        (
            "persistent calls",
            include_str!("../../../call_index.rs"),
            "callee: callee.clone()",
        ),
        (
            "validation",
            include_str!("../../../hir/validation/function_lookup.rs"),
            "Directory",
        ),
        (
            "cleanup inventory",
            include_str!("../../../cleanup/inventory_builder.rs"),
            "ResolvedExprKind::Call { args, .. }",
        ),
        (
            "cleanup plan",
            include_str!("../../../cleanup_plan/build/finish_call.rs"),
            "callee: callee.clone()",
        ),
        (
            "retained call edges",
            include_str!("../../../hir/inspection.rs"),
            "edges.insert((function.id.clone(), callee.clone()))",
        ),
    ];
    assert_eq!(families.len(), 8);
    for (family, whole, constructor) in families {
        assert!(whole.contains(constructor), "{family}");
    }
    assert!(include_str!("../../../graph.rs").contains("calls.insert(callee.as_str().to_owned())"));
    assert!(include_str!("../../source_callables.rs")
        .contains("synthetic_builder_bytes(program, &authored, &programs)"));
}

fn settled(metrics: Metrics) {
    assert!(!metrics.invalid);
    assert_eq!(metrics.live, 0);
    assert_eq!(metrics.allocated, metrics.freed);
}

fn whole(programs: &[Program], old: bool, calls: usize) -> ((usize, usize, usize), Metrics, usize) {
    let authored = crate::workspace_graph::index_authored(programs).unwrap();
    let program = &programs[0];
    let maximum = dependency_identity_max(program, &authored, programs).unwrap();
    // Establish the process-wide prelude outside BOTH measured constructors.
    let warm = crate::hir::resolve(program).unwrap();
    drop(warm);
    let ((fields, metrics), refused, charged, first) = with_limit_usage_refusal(usize::MAX, || {
        measure_with_capacity(1 << 16, || {
            // Branch directly to the original full signature. There is no
            // forwarding forecast frame or padded matched-current old ABI.
            let fields = if old {
                let estimate =
                    original::synthetic_builder_bytes_scoped_with_retention_with_registry(
                        program,
                        &authored,
                        programs,
                        Some(maximum),
                        true,
                        (6, cached_exact::Retention::Parsed),
                        None,
                    )
                    .unwrap();
                (
                    estimate.retained_hir,
                    estimate.retained_clone_and_hir,
                    estimate.raw_clone_and_hir,
                )
            } else {
                let estimate =
                    held_projection::synthetic_builder_bytes_scoped_with_retention_with_registry(
                        program,
                        &authored,
                        programs,
                        Some(maximum),
                        true,
                        (6, cached_exact::Retention::Parsed),
                        None,
                    )
                    .unwrap();
                (
                    estimate.retained_hir,
                    estimate.retained_clone_and_hir,
                    estimate.raw_clone_and_hir,
                )
            };
            let synthetic = held_projection::synthetic_program_with_registry(
                program, &authored, programs, None,
            )
            .unwrap();
            let resolved = crate::hir::resolve(&synthetic).unwrap();
            constructors(&resolved, calls);
            drop(resolved);
            drop(synthetic);
            fields
        })
    });
    assert!(!refused && first.is_none());
    settled(metrics);
    (fields, metrics, charged)
}

#[test]
fn complete_old_current_projection_and_hir_callers_are_observed_through_drop() {
    for (case, calls) in [(0, 1), (1, 64)] {
        let programs = inputs(case);
        let original = whole(&programs, true, calls);
        let current = whole(&programs, false, calls);
        eprintln!("LOCAL_CALLEE_TRUE_OLD_WHOLE calls={calls} old_forecast={:?} current_forecast={:?} old_paid={} current_paid={} old_after_drop={:?} current_after_drop={:?}; forecast, cumulative reservation and requested backing are distinct; this is not the final workspace/application qualification", original.0, current.0, original.2, current.2, original.1, current.1);
    }
}

#[test]
fn independent_projection_source_reconstructs_the_entire_pre_certificate_owner() {
    for (current, original, inverse_text, target_sha) in [
        (
            include_bytes!("before903/before_local_projection.rs").as_slice(),
            include_bytes!("before903/before_local_projection.rs.txt").as_slice(),
            include_str!("before903/before_local_projection.inverse.json"),
            "3e0d38a5c01f87a8b05815d9831f80b2604df5e37c15d0dcbf0cf0055656e942",
        ),
        (
            include_bytes!("before903/before_local_uncached.rs").as_slice(),
            include_bytes!("before903/before_local_uncached.rs.txt").as_slice(),
            include_str!("before903/before_local_uncached.inverse.json"),
            "4206e2a1a96eb2d4c89634aa193999383d0deb28a22a5a67b30647f9590d67f8",
        ),
        (
            include_bytes!("before903/call_carriers.rs").as_slice(),
            include_bytes!("before903/call_carriers.before_local.rs.txt").as_slice(),
            include_str!("before903/call_carriers.before_local.inverse.json"),
            "d0f832f517a6037e1c3f8c7f5b1cca79af990ed2837b8c29b38903cb4baa6033",
        ),
        (
            include_bytes!("before903/call_carriers_directory.rs").as_slice(),
            include_bytes!("before903/call_carriers_directory.before_local.rs.txt").as_slice(),
            include_str!("before903/call_carriers_directory.before_local.inverse.json"),
            "c49b0a20bbb6681d67dfbf177fe3fd41a64c57079afa7d7713aaa78a42d6ab95",
        ),
    ] {
        let inverse: serde_json::Value = serde_json::from_str(inverse_text).unwrap();
        assert_eq!(
            inverse["git_before"],
            "21574297e02afb6f286901171f6c179bf2ccea0a"
        );
        assert_eq!(inverse["target"]["sha256"], target_sha);
        for (bytes, key) in [(current, "current"), (original, "target")] {
            assert_eq!(bytes.len() as u64, inverse[key]["length"].as_u64().unwrap());
            assert_eq!(
                format!("{:x}", crate::digest_hex::LowerHex(Sha256::digest(bytes))),
                inverse[key]["sha256"].as_str().unwrap()
            );
        }
        let mut restored = Vec::new();
        let mut cursor = 0;
        for edit in inverse["edits"].as_array().unwrap() {
            let start = edit["start"].as_u64().unwrap() as usize;
            let end = edit["end"].as_u64().unwrap() as usize;
            assert!(cursor <= start && start <= end && end <= current.len());
            assert_eq!(
                &current[start..end],
                edit["after"].as_str().unwrap().as_bytes()
            );
            restored.extend_from_slice(&current[cursor..start]);
            restored.extend_from_slice(edit["before"].as_str().unwrap().as_bytes());
            cursor = end;
        }
        restored.extend_from_slice(&current[cursor..]);
        assert_eq!(restored, original);
    }
    assert_eq!(before903::OLD_PROOF_BYTES, 2 * size_of::<usize>());
    assert_eq!(before903::OLD_COST_BYTES, 6 * size_of::<usize>());
}

#[test]
fn complete_uncached_old_current_forecasts_keep_their_actual_invoices() {
    // The process-wide OnceLock belongs to neither whole-caller observation.
    // Match whole() above: fresh per-call constructors still run under measure.
    let warm = crate::hir::resolve(&inputs(0)[0]).unwrap();
    drop(warm);
    for case in 0..3 {
        let programs = inputs(case);
        let authored = crate::workspace_graph::index_authored(&programs).unwrap();
        let run = |old| {
            let ((forecast, metrics), refused, paid, first) =
                with_limit_usage_refusal(usize::MAX, || {
                    measure_with_capacity(1 << 16, || {
                        // Replace the same complete caller signature; no extra
                        // forwarding frame, returned proof padding or refund.
                        if old {
                            before903::before_local_uncached::uncached_output_layout(
                                &programs, &authored,
                            )
                            .unwrap()
                            .forecast
                        } else {
                            super::super::super::uncached_peak::uncached_output_layout(
                                &programs, &authored,
                            )
                            .unwrap()
                            .forecast
                        }
                    })
                });
            assert!(!refused && first.is_none());
            settled(metrics);
            (forecast, paid, metrics)
        };
        let old = run(true);
        let current = run(false);
        eprintln!("LOCAL_CALLEE_TRUE_OLD_UNCACHED case={case} old_forecast={} current_forecast={} complete_forecast_delta={} old_paid={} current_paid={} old_after_drop={:?} current_after_drop={:?}; complete old mode-five caller versus current mode-six caller including all composed corrections, not exclusive local-ID attribution or application admission", old.0, current.0, current.0 as i128-old.0 as i128, old.1, current.1, old.2, current.2);
    }
    let current = include_str!("../uncached_peak.rs");
    let old = include_str!("before903/before_local_uncached.rs");
    assert!(old.contains("program, authored, programs, maximum, 5, true, registry"));
    assert!(current.contains("program, authored, programs, maximum, 6, true, registry"));
}
