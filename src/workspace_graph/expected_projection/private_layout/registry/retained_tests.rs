//! Distinct cold-success storage, including the complete independent old route.
use super::super::{LayoutClosure, WORK};
use super::{before_retained, LayoutRegistry};
use crate::bounded_output::{
    active_remaining, clear_active_floor, set_active_floor, with_limit_usage_refusal,
};
use crate::test_allocation_observer::{current_metrics, measure};
use crate::workspace_graph::index_authored;
const CALLER: &str = include_str!("../function_caller.spx");
const PROVIDER: &str = include_str!("../function_provider.spx");
fn sources(count: usize) -> Vec<crate::ast::Program> {
    let mut programs = (0..count)
        .map(|n| {
            let source = CALLER
                .replace("function_layout.app", &format!("function_layout.app{n}"))
                .replace("app.main", &format!("app{n}.main"))
                .replace("report", &format!("_spx_layout_{}", n * 97 + 7));
            crate::parse(&source, format!("caller{n}.spx")).unwrap()
        })
        .collect::<Vec<_>>();
    programs.push(crate::parse(PROVIDER, "provider.spx").unwrap());
    programs
}
fn same(old: &LayoutClosure<'_>, new: &LayoutClosure<'_>) {
    assert_eq!(old.entries.len(), new.entries.len());
    for (a, b) in old.entries.iter().zip(&new.entries) {
        assert_eq!(
            (a.module, a.declaration, &a.alias),
            (b.module, b.declaration, &b.alias)
        );
    }
}
#[test]
fn distinct_cold_storage_keeps_complete_old_layout_work_sites_and_empty_reference_stacks() {
    let programs = sources(4);
    let authored = index_authored(&programs).unwrap();
    let registry = LayoutRegistry::new(&programs, &authored);
    let old = before_retained::LayoutRegistry::new(&programs, &authored);
    let mut state = None;
    for i in [0, 1, 0, 2, 3, 1] {
        let actual = registry.get(&programs[i], &authored, &programs).unwrap();
        let expected = old.get(&programs[i], &authored, &programs).unwrap();
        same(&expected, &actual);
        let target = authored.get("make").unwrap();
        let ty = &target.function.unwrap().return_type;
        assert_eq!(
            actual
                .signature_owner(&programs[i], target, ty, &authored, &programs)
                .unwrap(),
            expected
                .signature_owner(&programs[i], target, ty, &authored, &programs)
                .unwrap()
        );
        assert_eq!(registry.work_remaining(), old.work_remaining());
        let now = registry.retained.borrow().as_ref().unwrap().state();
        assert!(
            now.3,
            "all borrowed child references cleared before next caller"
        );
        if let Some(prior) = state {
            assert_eq!(now, prior, "all six backing pointers stable");
        }
        state = Some(now);
    }
    assert_eq!(registry.derivations.get(), 4);
}
#[test]
fn retained_storage_whole_constructor_to_drop_reports_old_current_bytes_and_real_distinct_savings()
{
    for count in [0, 1, 4] {
        let programs = sources(count);
        let authored = index_authored(&programs).unwrap();
        let old = with_limit_usage_refusal(1 << 26, || {
            measure(|| {
                let registry = before_retained::LayoutRegistry::new(&programs, &authored);
                for caller in &programs[..count] {
                    drop(registry.get(caller, &authored, &programs).unwrap());
                }
                if count == 0 {
                    assert!(registry
                        .get(&programs[0], &authored, &programs)
                        .unwrap()
                        .is_empty());
                }
                let receipt = registry.receipt();
                drop(registry);
                receipt
            })
        });
        let new = with_limit_usage_refusal(1 << 26, || {
            measure(|| {
                let registry = LayoutRegistry::new(&programs, &authored);
                for caller in &programs[..count] {
                    drop(registry.get(caller, &authored, &programs).unwrap());
                }
                if count == 0 {
                    assert!(registry
                        .get(&programs[0], &authored, &programs)
                        .unwrap()
                        .is_empty());
                }
                let receipt = registry.receipt();
                drop(registry);
                receipt
            })
        });
        assert!(!old.1 && !new.1 && old.3.is_none() && new.3.is_none());
        assert_eq!(old.0 .0, old.2);
        assert_eq!(new.0 .0, new.2);
        for metrics in [&old.0 .1, &new.0 .1] {
            assert!(!metrics.invalid && metrics.live == 0 && metrics.allocated == metrics.freed);
        }
        if count == 4 {
            assert!(
                new.2 < old.2,
                "actual reservation improvement across distinct successes"
            );
            assert!(
                new.0 .1.allocated < old.0 .1.allocated,
                "actual acquired bytes, not pointer reuse alone"
            );
            assert!(new.0 .1.allocations < old.0 .1.allocations);
        }
        eprintln!("833 retained cold={count} complete_old={old:?} current={new:?}");
    }
}
#[test]
fn retained_storage_release_late_cold_and_registry_moves_keep_real_generation_custody() {
    let programs = sources(3);
    let authored = index_authored(&programs).unwrap();
    let registry = LayoutRegistry::new(&programs, &authored);
    drop(registry.get(&programs[0], &authored, &programs).unwrap());
    let first = registry.receipt();
    let state = registry.retained.borrow().as_ref().unwrap().state();
    // Move the registry after acquisition: stack address is deliberately not identity.
    let moved = registry;
    drop(moved.get(&programs[1], &authored, &programs).unwrap());
    assert_eq!(state, moved.retained.borrow().as_ref().unwrap().state());
    let before = moved.receipt();
    assert!(before > first);
    moved.release_scratch();
    assert!(moved.retained.borrow().is_none());
    assert_eq!(moved.receipt(), before);
    drop(moved.get(&programs[0], &authored, &programs).unwrap());
    assert!(
        moved.retained.borrow().is_none(),
        "held hit grants no reacquisition"
    );
    drop(moved.get(&programs[2], &authored, &programs).unwrap());
    assert!(moved.retained.borrow().is_some());
    assert!(moved.receipt() > before);
}
#[test]
fn retained_storage_exact_short_zero_bytes_and_work_keep_terminal_receipts() {
    let programs = sources(2);
    let authored = index_authored(&programs).unwrap();
    let run = |bytes, work| {
        let registry = LayoutRegistry::new(&programs, &authored);
        registry.remaining.set(work);
        let (answer, overflowed, used, first) = with_limit_usage_refusal(bytes, || {
            for caller in &programs[..2] {
                drop(registry.get(caller, &authored, &programs)?);
            }
            Ok::<_, Vec<crate::diagnostic::Diagnostic>>(())
        });
        let work_used = work - registry.work_remaining();
        if answer.is_err() {
            assert!(registry.is_failed());
            assert!(registry.retained.borrow().is_none());
            let receipt = registry.receipt();
            registry.set_remaining(WORK);
            assert!(registry.get(&programs[0], &authored, &programs).is_err());
            assert_eq!(registry.receipt(), receipt);
        }
        assert_eq!(registry.receipt(), used);
        (answer, overflowed, used, first, work_used)
    };
    let ample = run(1 << 26, WORK);
    assert!(ample.0.is_ok());
    assert!(run(ample.2, ample.4).0.is_ok());
    for (bytes, work) in [
        (0, WORK),
        (ample.2 - 1, WORK),
        (1 << 26, 0),
        (1 << 26, ample.4 - 1),
    ] {
        assert!(run(bytes, work).0.is_err());
    }
}
#[test]
fn retained_storage_cold_parent_refusal_is_preowned_cache_path_unchanged_and_restoration_sticky() {
    let programs = sources(2);
    let authored = index_authored(&programs).unwrap();
    let expected = super::super::refusal();
    let (((), physical), overflowed, _, first) = with_limit_usage_refusal(1 << 26, || {
        measure(|| {
            let registry = LayoutRegistry::new(&programs, &authored);
            drop(registry.get(&programs[0], &authored, &programs).unwrap());
            let before = registry.receipt();
            // A cached directory uses the complete unchanged source/work path:
            // it neither touches storage nor consumes the next-cold entry bank.
            let cached =
                with_limit_usage_refusal(0, || registry.get(&programs[0], &authored, &programs));
            assert!(cached.0.is_ok() && !cached.1 && cached.2 == 0 && cached.3.is_none());
            drop(cached);
            let (result, foreign_failed, spent, foreign_first) =
                with_limit_usage_refusal(0, || {
                    let before = current_metrics().allocated;
                    let result = registry.get(&programs[1], &authored, &programs);
                    assert_eq!(
                        current_metrics().allocated,
                        before,
                        "new cold-parent refusal moves its paid G172, never allocates"
                    );
                    result
                });
            assert_eq!(
                result.as_ref().err().unwrap(),
                &expected,
                "all six original G172 fields, including static message bytes"
            );
            drop(result);
            assert!(!foreign_failed && spent == 0 && foreign_first.is_none());
            assert!(registry.is_failed() && registry.retained.borrow().is_none());
            assert!(registry.get(&programs[1], &authored, &programs).is_err());
            assert_eq!(registry.receipt(), before);
            drop(registry);
        })
    });
    assert!(overflowed);
    assert_eq!(first.unwrap().requested, 0);
    assert!(!physical.invalid && physical.live == 0 && physical.allocated == physical.freed);
    let registry = LayoutRegistry::new(&programs, &authored);
    drop(registry.get(&programs[0], &authored, &programs).unwrap());
    let copies = programs.clone();
    let copied = index_authored(&copies).unwrap();
    assert!(registry.get(&copies[1], &copied, &copies).is_err());
    assert!(registry.is_failed() && registry.retained.borrow().is_none());
    assert!(registry.get(&programs[1], &authored, &programs).is_err());
}
#[test]
fn retained_storage_nested_floor_and_malformed_diagnostic_match_complete_old() {
    let mut programs = sources(2);
    programs[1].module = "_spx_layout_18446744073709551615".to_owned();
    let authored = index_authored(&programs).unwrap();
    let registry = LayoutRegistry::new(&programs, &authored);
    let old = before_retained::LayoutRegistry::new(&programs, &authored);
    drop(registry.get(&programs[0], &authored, &programs).unwrap());
    drop(old.get(&programs[0], &authored, &programs).unwrap());
    assert_eq!(
        registry
            .get(&programs[1], &authored, &programs)
            .err()
            .unwrap(),
        old.get(&programs[1], &authored, &programs).err().unwrap()
    );
    assert!(registry.retained.borrow().is_none());
    let programs = sources(2);
    let authored = index_authored(&programs).unwrap();
    let result = with_limit_usage_refusal(1 << 26, || {
        assert!(set_active_floor(128));
        let registry = LayoutRegistry::new(&programs, &authored);
        drop(registry.get(&programs[0], &authored, &programs).unwrap());
        drop(registry.get(&programs[1], &authored, &programs).unwrap());
        assert!(active_remaining().unwrap() >= 128);
        clear_active_floor();
    });
    assert!(!result.1 && result.3.is_none());
}

#[test]
fn complete_immediate_sources_and_old_owning_bodies_remain_independent() {
    use crate::bounded_output::{
        classifier_string::ClassifierLedger, ReservationRefusal, ReservationStage,
    };
    use sha2::{Digest, Sha256};
    assert_eq!(
        ClassifierLedger::storage_terminal_receipt_controls(),
        2 * (3 * std::mem::size_of::<Option<ReservationRefusal>>()
            + std::mem::size_of::<ReservationRefusal>()
            + std::mem::size_of::<Option<ReservationStage>>())
    );
    let rows: serde_json::Value = serde_json::from_str(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/docs/evidence/retained-registry-storage-833/current-to-0e.json"
    )))
    .unwrap();
    for row in rows.as_array().unwrap() {
        let (current, before) = match row["source"].as_str().unwrap() {
            "crates/semaprax-native-rust-interop-builder/src/implementation/tests/ledger_capacity/source_inventory.rs" => (include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/crates/semaprax-native-rust-interop-builder/src/implementation/tests/ledger_capacity/source_inventory.rs")), include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/docs/evidence/retained-registry-storage-833/before/crates/semaprax-native-rust-interop-builder/src/implementation/tests/ledger_capacity/source_inventory.rs"))),
            "docs/PRIVATE-NOMINAL-LAYOUT-CLOSURE-V1.md" => (include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/docs/PRIVATE-NOMINAL-LAYOUT-CLOSURE-V1.md")), include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/docs/evidence/retained-registry-storage-833/before/docs/PRIVATE-NOMINAL-LAYOUT-CLOSURE-V1.md"))),
            "src/bounded_output/classifier_string.rs" => (include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/bounded_output/classifier_string.rs")), include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/docs/evidence/retained-registry-storage-833/before/src/bounded_output/classifier_string.rs"))),
            "src/source_reflows_merge.rs" => (include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/source_reflows_merge.rs")), include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/docs/evidence/retained-registry-storage-833/before/src/source_reflows_merge.rs"))),
            "src/workspace_graph/expected_projection/private_layout/names.rs" => (include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/workspace_graph/expected_projection/private_layout/names.rs")), include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/docs/evidence/retained-registry-storage-833/before/src/workspace_graph/expected_projection/private_layout/names.rs"))),
            "src/workspace_graph/expected_projection/private_layout/owner_index.rs" => (include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/workspace_graph/expected_projection/private_layout/owner_index.rs")), include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/docs/evidence/retained-registry-storage-833/before/src/workspace_graph/expected_projection/private_layout/owner_index.rs"))),
            "src/workspace_graph/expected_projection/private_layout/registry/scratch_tests.rs" => (include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/workspace_graph/expected_projection/private_layout/registry/scratch_tests.rs")), include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/docs/evidence/retained-registry-storage-833/before/src/workspace_graph/expected_projection/private_layout/registry/scratch_tests.rs"))),
            "src/workspace_graph/expected_projection/private_layout/registry.rs" => (include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/workspace_graph/expected_projection/private_layout/registry.rs")), include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/docs/evidence/retained-registry-storage-833/before/src/workspace_graph/expected_projection/private_layout/registry.rs"))),
            "src/workspace_graph/expected_projection/private_layout.rs" => (include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/workspace_graph/expected_projection/private_layout.rs")), include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/docs/evidence/retained-registry-storage-833/before/src/workspace_graph/expected_projection/private_layout.rs"))),
            "tests/hir_module_boundaries.rs" => (include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/tests/hir_module_boundaries.rs")), include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/docs/evidence/retained-registry-storage-833/before/tests/hir_module_boundaries.rs"))),
            "tests/workspace/semantic_graph/source_inventory.rs" => (include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/tests/workspace/semantic_graph/source_inventory.rs")), include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/docs/evidence/retained-registry-storage-833/before/tests/workspace/semantic_graph/source_inventory.rs"))),
            _ => panic!("unexpected complete source owner"),
        };
        let projected = crate::source_reflows_merge::retained833_input(
            row["source"].as_str().unwrap(),
            current,
        );
        let current = projected.as_str();
        assert_eq!(
            format!(
                "{:x}",
                crate::digest_hex::LowerHex(Sha256::digest(current.as_bytes()))
            ),
            row["current_sha256"]
        );
        let mut restored = current.to_owned();
        for edit in row["edits"].as_array().unwrap() {
            let new = edit["new"].as_str().unwrap();
            assert_eq!(restored.matches(new).count(), 1);
            restored = restored.replacen(new, edit["old"].as_str().unwrap(), 1);
        }
        assert_eq!(restored, before);
        assert_eq!(
            format!(
                "{:x}",
                crate::digest_hex::LowerHex(Sha256::digest(restored.as_bytes()))
            ),
            row["original_sha256"]
        );
    }
    let adapter: serde_json::Value = serde_json::from_str(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/docs/evidence/retained-registry-storage-833/old-registry-adaptation.json"
    )))
    .unwrap();
    let mut compiled = crate::source_reflows_merge::retained833_input(
        "src/workspace_graph/expected_projection/private_layout/registry/before_retained.rs",
        include_str!("before_retained.rs"),
    );
    assert_eq!(
        format!(
            "{:x}",
            crate::digest_hex::LowerHex(Sha256::digest(compiled.as_bytes()))
        ),
        adapter["compiled_sha256"]
    );
    for edit in adapter["edits"].as_array().unwrap() {
        let new = edit["new"].as_str().unwrap();
        assert_eq!(compiled.matches(new).count(), 1);
        compiled = compiled.replacen(new, edit["old"].as_str().unwrap(), 1);
    }
    assert_eq!(compiled, include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/docs/evidence/retained-registry-storage-833/before/src/workspace_graph/expected_projection/private_layout/registry.rs")));
    let bindings: serde_json::Value = serde_json::from_str(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/docs/evidence/retained-registry-storage-833/new-owners.json"
    )))
    .unwrap();
    for row in bindings.as_array().unwrap() {
        let source = match row["source"].as_str().unwrap() {
            "storage.rs" => include_str!("storage.rs"),
            "before_retained.rs" => include_str!("before_retained.rs"),
            "retained_tests.rs" => include_str!("retained_tests.rs"),
            _ => panic!("unknown new owner"),
        };
        let name = row["source"].as_str().unwrap();
        let path =
            format!("src/workspace_graph/expected_projection/private_layout/registry/{name}");
        let source = crate::source_reflows_merge::retained833_input(&path, source);
        assert_eq!(
            format!(
                "{:x}",
                crate::digest_hex::LowerHex(Sha256::digest(source.as_bytes()))
            ),
            row["sha256"]
        );
    }
}
