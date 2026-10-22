use super::{CoreProviderProofs, Phase};
use crate::ast::Program;
use crate::bounded_output::{with_limit_usage, with_limit_usage_refusal};
use crate::test_allocation_observer::measure;
use crate::workspace_graph::expected_projection::{
    borrowed_imports, borrowed_imports_original, cached_exact, held_projection,
    held_projection_original, import_stub_original, private_layout, SyntheticBuilderCosts,
};
use crate::workspace_graph::{import_validation, index_authored, index_modules};

fn programs(width: usize, depth: usize) -> Vec<Program> {
    let mut source = String::from("module proof.provider;");
    for ordinal in 0..width.max(depth) {
        let body = if ordinal > 0 && ordinal < depth {
            format!("f{}(output,index,value)", ordinal - 1)
        } else {
            "bytes_set(output,index,value)".to_owned()
        };
        source.push_str(&format!(" @id(\"proof.f{ordinal}\") fn f{ordinal}(output:own Bytes,index:usize,value:u8)->Bytes {{{body}}}"));
    }
    let mut caller = String::from("module proof.app;");
    for ordinal in 0..width {
        caller.push_str(&format!(
            " use function @id(\"proof.f{ordinal}\") from proof.provider as f{ordinal};"
        ));
    }
    caller.push_str(" @id(\"proof.main\") fn main()->i64 {0}");
    vec![
        crate::parse(&caller, "caller.spx").unwrap(),
        crate::parse(&source, "provider.spx").unwrap(),
    ]
}
fn fields(cost: &SyntheticBuilderCosts) -> [usize; 6] {
    [
        cost.raw_clone_and_hir,
        cost.retained_clone_and_hir,
        cost.transient_import_clone,
        cost.runtime,
        cost.retained_hir,
        cost.synthetic_ast,
    ]
}

#[test]
fn fresh_logical_phases_reuse_only_backing_and_match_complete_original_work_and_verdicts() {
    for (width, depth) in [(1, 1), (2, 1), (17, 8), (33, 32), (1, 96)] {
        let programs = programs(width, depth);
        let authored = index_authored(&programs).unwrap();
        let forged = programs[1].functions[0].clone();
        let (_, overflowed, _) =
            with_limit_usage(crate::workspace_graph::MAX_BUILDER_BYTES, || {
                let mut core = CoreProviderProofs::new(&authored, &programs).unwrap();
                let mut previous = None;
                for _ in 0..3 {
                    let mut before =
                        import_stub_original::ByteHelperProofs::new(&authored, &programs);
                    let mut phase = Phase::new(&authored, &programs, Some(&mut core)).unwrap();
                    let initial = phase.get().unwrap().observation();
                    assert_eq!(initial.0, 0, "no truth row survives a phase");
                    if let Some((entries, work, vector, cursor)) = previous {
                        assert!(entries > 0);
                        assert!(
                            initial.1 > work,
                            "new original WORK, not a reused exhausted scope"
                        );
                        assert_eq!((initial.2, initial.3), (vector, cursor));
                    }
                    for function in &programs[1].functions {
                        let expected = import_stub_original::byte_helper_authorized(
                            function,
                            "proof.provider",
                            &mut before,
                        )
                        .unwrap();
                        let actual = super::super::byte_helper_authorized(
                            function,
                            "proof.provider",
                            phase.get().unwrap(),
                        )
                        .unwrap();
                        assert_eq!(actual, expected);
                        let current = phase.get().unwrap().observation();
                        assert_eq!(
                            (current.0, current.1),
                            import_stub_original::proof_observation(&before)
                        );
                        // The original duplicate-target work debit remains within each phase.
                        let expected = import_stub_original::byte_helper_authorized(
                            function,
                            "proof.provider",
                            &mut before,
                        )
                        .unwrap();
                        let actual = super::super::byte_helper_authorized(
                            function,
                            "proof.provider",
                            phase.get().unwrap(),
                        )
                        .unwrap();
                        assert_eq!(actual, expected);
                        let current = phase.get().unwrap().observation();
                        assert_eq!(
                            (current.0, current.1),
                            import_stub_original::proof_observation(&before)
                        );
                    }
                    let target = &programs[1].functions[0];
                    assert!(!super::super::byte_helper_authorized(
                        target,
                        "wrong.module",
                        phase.get().unwrap()
                    )
                    .unwrap());
                    assert!(!super::super::byte_helper_authorized(
                        &forged,
                        "proof.provider",
                        phase.get().unwrap()
                    )
                    .unwrap());
                    previous = Some(phase.get().unwrap().observation());
                    phase.complete().unwrap();
                    drop(phase);
                    assert!(!core.poisoned);
                    drop(before);
                }
                drop(core);
            });
        assert!(!overflowed);
    }
}

#[test]
fn core_exact_short_zero_sticky_source_payer_and_incomplete_drop_refuse_without_repair() {
    let programs = programs(17, 8);
    let authored = index_authored(&programs).unwrap();
    let target = &programs[1].functions[0];
    let operation = || -> Result<(), crate::diagnostic::Diagnostic> {
        let mut core = CoreProviderProofs::new(&authored, &programs)?;
        for _ in 0..2 {
            let mut phase = Phase::new(&authored, &programs, Some(&mut core))?;
            assert!(phase.get()?.authorized(target, "proof.provider")?);
            phase.complete()?;
        }
        drop(core);
        Ok(())
    };
    let (result, overflowed, required) =
        with_limit_usage(crate::workspace_graph::MAX_BUILDER_BYTES, operation);
    result.unwrap();
    assert!(!overflowed);
    let (result, overflowed, used) = with_limit_usage(required, operation);
    result.unwrap();
    assert!(!overflowed);
    assert_eq!(used, required);
    for limit in [0, required - 1] {
        let (result, overflowed, _, receipt) = with_limit_usage_refusal(limit, operation);
        assert_eq!(result.unwrap_err().code, "SPX-G171");
        assert!(overflowed);
        assert!(receipt.is_some());
    }
    let observe_cleanup = |retry| {
        with_limit_usage_refusal(required - 1, || {
            let mut core = CoreProviderProofs::new(&authored, &programs).unwrap();
            let verdict = {
                let mut phase = Phase::new(&authored, &programs, Some(&mut core)).unwrap();
                phase.get().unwrap().authorized(target, "proof.provider")
            };
            assert_eq!(verdict.unwrap_err().code, "SPX-G171");
            assert!(core.poisoned);
            if retry {
                assert!(Phase::new(&authored, &programs, Some(&mut core)).is_err());
            }
            drop(core);
        })
    };
    let (_, before_refused, before_used, before_receipt) = observe_cleanup(false);
    let (_, after_refused, after_used, after_receipt) = observe_cleanup(true);
    assert!(before_refused && after_refused);
    assert_eq!(before_used, after_used);
    assert_eq!(
        before_receipt, after_receipt,
        "cleanup cannot replace the first refusal or repay the scope"
    );
    assert_eq!(
        CoreProviderProofs::new(&authored, &programs)
            .err()
            .unwrap()
            .code,
        "SPX-G171"
    );
    let (_, overflowed, _) = with_limit_usage(crate::workspace_graph::MAX_BUILDER_BYTES, || {
        let mut core = CoreProviderProofs::new(&authored, &programs).unwrap();
        let mut phase = Phase::new(&authored, &programs, Some(&mut core)).unwrap();
        assert!(phase
            .get()
            .unwrap()
            .authorized(target, "proof.provider")
            .unwrap());
        // A nested payer cannot use backing acquired by this exact core payer.
        let (error, child_refused, _) =
            with_limit_usage(crate::workspace_graph::MAX_BUILDER_BYTES, || {
                phase.get().err().unwrap()
            });
        assert_eq!(error.code, "SPX-G171");
        assert!(
            !child_refused,
            "the original held payer receives the refusal"
        );
        drop(phase);
        assert!(core.poisoned);
        assert!(Phase::new(&authored, &programs, Some(&mut core)).is_err());
    });
    assert!(overflowed);
    for source_attack in 0..3 {
        let (_, overflowed, _) =
            with_limit_usage(crate::workspace_graph::MAX_BUILDER_BYTES, || {
                let mut core = CoreProviderProofs::new(&authored, &programs).unwrap();
                let changed_facts = index_authored(&programs).unwrap();
                let changed_programs = programs.clone();
                let result = match source_attack {
                    0 => Phase::new(&changed_facts, &programs, Some(&mut core)),
                    1 => Phase::new(&authored, &changed_programs, Some(&mut core)),
                    _ => Phase::new(&authored, &programs[..1], Some(&mut core)),
                };
                assert_eq!(result.err().unwrap().code, "SPX-G171");
                assert!(core.poisoned);
                assert!(Phase::new(&authored, &programs, Some(&mut core)).is_err());
            });
        assert!(
            !overflowed,
            "source rejection grants no payment or ledger rewrite"
        );
    }
    let (_, overflowed, _) = with_limit_usage(crate::workspace_graph::MAX_BUILDER_BYTES, || {
        let mut core = CoreProviderProofs::new(&authored, &programs).unwrap();
        {
            let mut phase = Phase::new(&authored, &programs, Some(&mut core)).unwrap();
            assert!(phase
                .get()
                .unwrap()
                .authorized(target, "proof.provider")
                .unwrap());
            // Early error/unwind ordinary Drop returns the backing, then poisons.
        }
        assert!(core.backing.is_some());
        assert!(core.poisoned);
        assert!(Phase::new(&authored, &programs, Some(&mut core)).is_err());
    });
    assert!(!overflowed);
    let (_, overflowed, _) = with_limit_usage(0, || {
        assert!(!crate::bounded_output::reserve_active_required(1));
        assert!(CoreProviderProofs::new(&authored, &programs).is_err());
    });
    assert!(overflowed);
    // Even the physical primitive cannot resurrect an explicitly failed scope.
    let (failed_backing, overflowed, _) = with_limit_usage(0, || {
        let mut proof = crate::loop_calls::byte_writer::SourceProofs::default();
        assert!(proof.reserve_carrier(1).is_err());
        let (backing, failed) = proof.into_backing();
        assert!(failed);
        backing
    });
    assert!(overflowed);
    let (restored, overflowed, _) =
        with_limit_usage(crate::workspace_graph::MAX_BUILDER_BYTES, || {
            crate::loop_calls::byte_writer::SourceProofs::from_backing(failed_backing)
        });
    assert_eq!(restored.err().unwrap().code, "SPX-G171");
    assert!(!overflowed, "a new budget does not repair the failed proof");
}

#[test]
fn complete_original_forecast_constructor_ids_resolution_and_drop_match_reused_core() {
    for (width, depth) in [(1, 1), (2, 1), (17, 8), (33, 32)] {
        let programs = programs(width, depth);
        let authored = index_authored(&programs).unwrap();
        let modules = index_modules(&programs).unwrap();
        let registry = private_layout::registry::LayoutRegistry::new(&programs, &authored);
        import_validation::validate_uses_with_registry(
            &programs,
            &modules,
            &authored,
            Some(&registry),
        )
        .unwrap();
        let (outcome, overflowed, consumed, first_refusal) = with_limit_usage_refusal(
            crate::workspace_graph::MAX_BUILDER_BYTES,
            || {
                std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                    let mut core = CoreProviderProofs::new(&authored, &programs).unwrap();
                    for _ in 0..2 {
                        let old_cost = held_projection_original::synthetic_builder_bytes_scoped_with_retention_with_registry(
                    &programs[0], &authored, &programs, None, true, (4, cached_exact::Retention::Parsed), Some(&registry)).unwrap();
                        let current_cost = held_projection::synthetic_builder_bytes_core(
                            &programs[0],
                            &authored,
                            &programs,
                            None,
                            true,
                            (4, cached_exact::Retention::Parsed),
                            (Some(&registry), &mut core),
                        )
                        .unwrap();
                        // The complete saved constructor still owns the historical
                        // six-field receipt. The explicit prelude-accounting v1
                        // correction supersedes only its missing retained term.
                        let historical = fields(&old_cost);
                        let maximum_identity = authored
                            .keys()
                            .map(|id| id.len())
                            .chain(crate::prelude::all_ids().into_iter().map(str::len))
                            .max()
                            .unwrap_or(0);
                        let prelude = crate::workspace_graph::expected_projection::prelude_forecast::retained_upper(
                        &crate::prelude::base_schema::BASE,
                        maximum_identity,
                    ).unwrap();
                        let corrected = [
                            historical[0].checked_add(prelude).unwrap(),
                            historical[1].checked_add(prelude).unwrap(),
                            historical[2],
                            historical[3],
                            historical[4].checked_add(prelude).unwrap(),
                            historical[5],
                        ];
                        assert_eq!(
                        fields(&current_cost),
                        corrected,
                        "all six historical fields retain only the independently derived missing prelude correction"
                    );
                        let old = borrowed_imports_original::construct(
                            &programs[0],
                            &authored,
                            &programs,
                            &registry,
                        )
                        .unwrap();
                        let current = borrowed_imports::construct_core(
                            &programs[0],
                            &authored,
                            &programs,
                            &registry,
                            &mut core,
                        )
                        .unwrap();
                        use crate::ast::program_source::ProgramSource;
                        assert_eq!(
                            old.functions().iter().collect::<Vec<_>>(),
                            current.functions().iter().collect::<Vec<_>>()
                        );
                        assert_eq!(
                            old.types().iter().collect::<Vec<_>>(),
                            current.types().iter().collect::<Vec<_>>()
                        );
                        let layout = registry.get(&programs[0], &authored, &programs).unwrap();
                        let old_hir = crate::hir::resolve_source(
                            &old,
                            layout.presentation(&programs[0], &old).unwrap(),
                        )
                        .unwrap()
                        .0;
                        let new_hir = crate::hir::resolve_source(
                            &current,
                            layout.presentation(&programs[0], &current).unwrap(),
                        )
                        .unwrap()
                        .0;
                        assert_eq!(old_hir, new_hir);
                        crate::hir::validate(&old_hir).unwrap();
                        crate::hir::validate(&new_hir).unwrap();
                        drop((old_hir, new_hir, old, current));
                    }
                    assert!(!core.poisoned);
                    drop(core);
                }))
            },
        );
        if let Err(payload) = outcome {
            // Outside the restored ledger: report its complete authoritative
            // first receipt, then resume the exact original panic. No oracle
            // is skipped and the same cumulative owner/bound remain in force.
            eprintln!(
                "core-proof-active-receipt width={width} depth={depth} consumed={consumed} refused={overflowed} first={first_refusal:?}"
            );
            std::panic::resume_unwind(payload);
        }
        assert!(!overflowed);
    }
}

#[test]
fn repeated_body_checks_have_no_second_backing_acquisition_and_complete_drop_is_observed() {
    let programs = programs(33, 32);
    let authored = index_authored(&programs).unwrap();
    let (_, overflowed, _) = with_limit_usage(crate::workspace_graph::MAX_BUILDER_BYTES, || {
        let (_, observed) = measure(|| {
            let mut core = CoreProviderProofs::new(&authored, &programs).unwrap();
            for _ in 0..3 {
                let mut phase = Phase::new(&authored, &programs, Some(&mut core)).unwrap();
                for function in &programs[1].functions {
                    phase
                        .get()
                        .unwrap()
                        .authorized(function, "proof.provider")
                        .unwrap();
                }
                phase.complete().unwrap();
            }
            drop(core);
        });
        assert!(!observed.invalid);
        assert!(observed.allocations > 0);
        assert_eq!(observed.allocated, observed.freed);
        assert_eq!(observed.live, 0);
        let (_, original) = measure(|| {
            for _ in 0..3 {
                let mut old = import_stub_original::ByteHelperProofs::new(&authored, &programs);
                for function in &programs[1].functions {
                    import_stub_original::byte_helper_authorized(
                        function,
                        "proof.provider",
                        &mut old,
                    )
                    .unwrap();
                }
                drop(old);
            }
        });
        assert!(!original.invalid);
        assert_eq!(original.allocated, original.freed);
        assert_eq!(original.live, 0);
        assert!(observed.allocated < original.allocated);
        assert!(observed.allocations < original.allocations);
        // Separately observe only the second actual phase, with backing
        // legitimately acquired before that observation. The full acquisition
        // and Drop observations above remain mandatory; no nested observer.
        let mut core = CoreProviderProofs::new(&authored, &programs).unwrap();
        {
            let mut phase = Phase::new(&authored, &programs, Some(&mut core)).unwrap();
            for function in &programs[1].functions {
                phase
                    .get()
                    .unwrap()
                    .authorized(function, "proof.provider")
                    .unwrap();
            }
            phase.complete().unwrap();
        }
        let (_, repeated) = measure(|| {
            let mut phase = Phase::new(&authored, &programs, Some(&mut core)).unwrap();
            for function in &programs[1].functions {
                phase
                    .get()
                    .unwrap()
                    .authorized(function, "proof.provider")
                    .unwrap();
            }
            phase.complete().unwrap();
        });
        assert!(!repeated.invalid);
        assert_eq!(repeated.allocations, 0);
        assert_eq!(repeated.reallocations, 0);
        assert_eq!(repeated.allocated, 0);
        drop(core);
    });
    assert!(!overflowed);
}

#[test]
fn cleared_source_rows_rerun_actual_transitive_callbacks_with_complete_original_work() {
    use crate::ast::program_source::ProgramSource;
    use crate::loop_calls::byte_writer::{source_index_original, SourceBacking, SourceProofs};
    use std::cell::Cell;
    let programs = programs(1, 8);
    let program = &programs[1];
    let target = &program.functions[7];
    let (_, overflowed, _) = with_limit_usage(crate::workspace_graph::MAX_BUILDER_BYTES, || {
        let mut backing = SourceBacking::empty();
        let mut previous = None;
        for _ in 0..3 {
            let original_calls = Cell::new(0usize);
            let current_calls = Cell::new(0usize);
            let lookup = |calls: &Cell<usize>, name: &str| {
                calls.set(calls.get() + 1);
                program
                    .functions
                    .iter()
                    .find(|function| function.name == name)
                    .map(|function| (program as &dyn ProgramSource, function))
            };
            let mut old = source_index_original::SourceProofs::default();
            let mut current = SourceProofs::from_backing(backing).unwrap();
            let expected = old
                .prove(
                    program,
                    target,
                    |_, name| lookup(&original_calls, name),
                    |_, _, _| true,
                    |_, _| true,
                    1,
                )
                .unwrap();
            let actual = current
                .prove(
                    program,
                    target,
                    |_, name| lookup(&current_calls, name),
                    |_, _, _| true,
                    |_, _| true,
                    1,
                )
                .unwrap();
            assert!(expected);
            assert_eq!(actual, expected);
            assert!(original_calls.get() > 0);
            assert_eq!(
                current_calls.get(),
                original_calls.get(),
                "every new phase repeats the actual body traversal"
            );
            let observation = current.scope_observation();
            assert_eq!((observation.0, observation.1), old.scope_observation());
            if let Some((vector, cursor)) = previous {
                assert_eq!((observation.2, observation.3), (vector, cursor));
            }
            previous = Some((observation.2, observation.3));
            let (returned, failed) = current.into_backing();
            assert!(!failed);
            backing = returned;
            drop(old);
        }
        drop(backing);
    });
    assert!(!overflowed);
}

#[test]
fn complete_original_receivers_and_all_literal_source_inverses_remain_byte_exact() {
    use sha2::{Digest, Sha256};
    let manifest: serde_json::Value = serde_json::from_str(ci_str!(
        "../../../../../docs/evidence/provider-proof-backing-714177/whole-source-inverses.json"
    ))
    .unwrap();
    let rows = manifest["whole_sources"].as_array().unwrap();
    assert_eq!(rows.len(), 17);
    for row in rows {
        let (current, original) = match row["file"].as_str().unwrap() {
            "src/loop_calls/byte_writer/source_index.rs" => (include_str!("../../../../../src/loop_calls/byte_writer/source_index.rs"), ci_str!("../../../../../docs/evidence/provider-proof-backing-714177/0.before.txt")),
            "src/loop_calls/byte_writer.rs" => (include_str!("../../../../../src/loop_calls/byte_writer.rs"), ci_str!("../../../../../docs/evidence/provider-proof-backing-714177/1.before.txt")),
            "src/workspace_graph/expected_projection/import_stub/byte_helper.rs" => (include_str!("../../../../../src/workspace_graph/expected_projection/import_stub/byte_helper.rs"), ci_str!("../../../../../docs/evidence/provider-proof-backing-714177/2.before.txt")),
            "src/workspace_graph/expected_projection/import_stub.rs" => (include_str!("../../../../../src/workspace_graph/expected_projection/import_stub.rs"), ci_str!("../../../../../docs/evidence/provider-proof-backing-714177/3.before.txt")),
            "src/workspace_graph/expected_projection/held_projection.rs" => (include_str!("../../../../../src/workspace_graph/expected_projection/held_projection.rs"), ci_str!("../../../../../docs/evidence/provider-proof-backing-714177/4.before.txt")),
            "src/workspace_graph/expected_projection/borrowed_imports.rs" => (include_str!("../../../../../src/workspace_graph/expected_projection/borrowed_imports.rs"), ci_str!("../../../../../docs/evidence/provider-proof-backing-714177/5.before.txt")),
            "src/workspace_graph/expected_projection/borrowed_leaf.rs" => (include_str!("../../../../../src/workspace_graph/expected_projection/borrowed_leaf.rs"), ci_str!("../../../../../docs/evidence/provider-proof-backing-714177/6.before.txt")),
            "src/workspace_graph/expected_projection/uncached_peak.rs" => (include_str!("../../../../../src/workspace_graph/expected_projection/uncached_peak.rs"), ci_str!("../../../../../docs/evidence/provider-proof-backing-714177/7.before.txt")),
            "src/workspace_graph/expected_projection.rs" => (include_str!("../../../../../src/workspace_graph/expected_projection.rs"), ci_str!("../../../../../docs/evidence/provider-proof-backing-714177/8.before.txt")),
            "src/workspace_graph.rs" => (include_str!("../../../../../src/workspace_graph.rs"), ci_str!("../../../../../docs/evidence/provider-proof-backing-714177/9.before.txt")),
            "tests/workspace/semantic_graph/source_inventory.rs" => (include_str!("../../../../../tests/workspace/semantic_graph/source_inventory.rs"), ci_str!("../../../../../docs/evidence/provider-proof-backing-714177/10.before.txt")),
            "crates/semaprax-native-rust-interop-builder/src/implementation/tests/ledger_capacity/source_inventory.rs" => (include_str!("../../../../../crates/semaprax-native-rust-interop-builder/src/implementation/tests/ledger_capacity/source_inventory.rs"), ci_str!("../../../../../docs/evidence/provider-proof-backing-714177/11.before.txt")),
            "src/loop_calls/byte_writer/source_index_original.rs" => (include_str!("../../../../../src/loop_calls/byte_writer/source_index_original.rs"), ci_str!("../../../../../docs/evidence/provider-proof-backing-714177/0.before.txt")),
            "src/workspace_graph/expected_projection/import_stub/byte_helper_original.rs" => (include_str!("../../../../../src/workspace_graph/expected_projection/import_stub/byte_helper_original.rs"), ci_str!("../../../../../docs/evidence/provider-proof-backing-714177/2.before.txt")),
            "src/workspace_graph/expected_projection/import_stub_original.rs" => (include_str!("../../../../../src/workspace_graph/expected_projection/import_stub_original.rs"), ci_str!("../../../../../docs/evidence/provider-proof-backing-714177/3.before.txt")),
            "src/workspace_graph/expected_projection/held_projection_original.rs" => (include_str!("../../../../../src/workspace_graph/expected_projection/held_projection_original.rs"), ci_str!("../../../../../docs/evidence/provider-proof-backing-714177/4.before.txt")),
            "src/workspace_graph/expected_projection/borrowed_imports_original.rs" => (include_str!("../../../../../src/workspace_graph/expected_projection/borrowed_imports_original.rs"), ci_str!("../../../../../docs/evidence/provider-proof-backing-714177/5.before.txt")),
            other => panic!("unregistered complete source {other}"),
        };
        let current = crate::source_reflows_merge::constructor_complete_input(
            row["file"].as_str().unwrap(), current,
        );
        let current = current.as_str();
        assert_eq!(
            original.len(),
            row["original_bytes"].as_u64().unwrap() as usize
        );
        assert_eq!(
            format!(
                "{:x}",
                crate::digest_hex::LowerHex(Sha256::digest(original.as_bytes()))
            ),
            row["original_sha256"].as_str().unwrap()
        );
        assert_eq!(
            format!(
                "{:x}",
                crate::digest_hex::LowerHex(Sha256::digest(current.as_bytes()))
            ),
            row["current_sha256"].as_str().unwrap()
        );
        let mut restored = current.to_owned();
        for hook in row["inverse"].as_array().unwrap() {
            let present = hook["current"].as_str().unwrap();
            assert_eq!(
                restored.matches(present).count(),
                1,
                "{} {}",
                row["file"],
                hook["hook"]
            );
            restored = restored.replacen(present, hook["before"].as_str().unwrap(), 1);
        }
        assert_eq!(
            restored.as_bytes(),
            original.as_bytes(),
            "complete original {}",
            row["file"]
        );
    }
    main7f8_provider::complete_incoming_receiver();
    let parent = ci_str!("../../import_stub.rs");
    assert!(
        parent.contains("#[path = \"import_stub/core_proofs.rs\"]\npub(super) mod core_proofs;")
    );
    let source = ci_str!("../../../../loop_calls/byte_writer.rs");
    assert!(source.contains("#[cfg(test)]\npub(crate) mod source_index_original;"));
}

#[path = "main7f8_provider.rs"]
mod main7f8_provider;
