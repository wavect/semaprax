use super::*;
use crate::bounded_output::cfg_reservation::{CfgReservation, Refused};
use crate::test_allocation_observer::measure;
use std::mem::{align_of, size_of};
use std::panic::{catch_unwind, AssertUnwindSafe};

#[test]
fn canonical_stage_preserves_actual_budget_receipt_guard_and_paid_carrier_layouts() {
    // Frozen pre-change field types, independent of the current declarations.
    // The old and new stage enums both fit the same one-byte/niche layout.
    #[derive(Clone, Copy)]
    enum BeforeStage {
        ResolvedCore,
        CompactValidation,
        UncachedSyntheticPeak,
        AuthoredModuleValidation,
        FrontendLookup,
        SelectiveResolution,
        FrontendRetention,
        ImportedVecInstances,
        CorePrebound,
        ExpectedEdges,
        ResolvedCallEdges,
        StubSignatures,
        DeclarationFacts,
        RetainedFacts,
        CoreDirectories,
        EdgeProof,
        SyntheticModule,
        CachedValidation,
        FreshResolution,
        HirLowering,
        HirLoanPlans,
        HirInstanceLoanPlans,
        HirCleanupInventories,
        HirInstanceCleanupInventories,
        HirCleanupPlans,
        HirYieldProfiles,
        HirValidation,
        RetainedModule,
        LoanIdentityInventory,
        RetainedLoanScratch,
        RetainedLoanSidecars,
    }
    let _all_old_stages = [
        BeforeStage::ResolvedCore,
        BeforeStage::CompactValidation,
        BeforeStage::UncachedSyntheticPeak,
        BeforeStage::AuthoredModuleValidation,
        BeforeStage::FrontendLookup,
        BeforeStage::SelectiveResolution,
        BeforeStage::FrontendRetention,
        BeforeStage::ImportedVecInstances,
        BeforeStage::CorePrebound,
        BeforeStage::ExpectedEdges,
        BeforeStage::ResolvedCallEdges,
        BeforeStage::StubSignatures,
        BeforeStage::DeclarationFacts,
        BeforeStage::RetainedFacts,
        BeforeStage::CoreDirectories,
        BeforeStage::EdgeProof,
        BeforeStage::SyntheticModule,
        BeforeStage::CachedValidation,
        BeforeStage::FreshResolution,
        BeforeStage::HirLowering,
        BeforeStage::HirLoanPlans,
        BeforeStage::HirInstanceLoanPlans,
        BeforeStage::HirCleanupInventories,
        BeforeStage::HirInstanceCleanupInventories,
        BeforeStage::HirCleanupPlans,
        BeforeStage::HirYieldProfiles,
        BeforeStage::HirValidation,
        BeforeStage::RetainedModule,
        BeforeStage::LoanIdentityInventory,
        BeforeStage::RetainedLoanScratch,
        BeforeStage::RetainedLoanSidecars,
    ];
    struct BeforeBudget {
        _initial: usize,
        _remaining: Cell<usize>,
        _floor: Cell<usize>,
        _overflowed: Cell<bool>,
        _first_refusal: Cell<Option<BeforeReceipt>>,
        _stage: Cell<Option<BeforeStage>>,
    }
    #[derive(Clone, Copy)]
    struct BeforeReceipt {
        _requested: usize,
        _remaining: usize,
        _floor: usize,
        _stage: Option<BeforeStage>,
    }
    struct BeforeGuard {
        _held: Option<(Rc<BeforeBudget>, Option<BeforeStage>)>,
    }
    #[derive(Clone, Copy)]
    enum BeforeRefused {
        NoOwner,
        Budget { _receipt: BeforeReceipt },
        Allocation,
        Capacity,
        Detached,
        Arithmetic,
    }
    let _all_old_reasons = [
        BeforeRefused::NoOwner,
        BeforeRefused::Budget {
            _receipt: BeforeReceipt {
                _requested: 0,
                _remaining: 0,
                _floor: 0,
                _stage: None,
            },
        },
        BeforeRefused::Allocation,
        BeforeRefused::Capacity,
        BeforeRefused::Detached,
        BeforeRefused::Arithmetic,
    ];
    struct BeforePaidCarrier {
        _budget: Rc<BeforeBudget>,
        _text: String,
        _first: Cell<Option<BeforeRefused>>,
    }
    assert_eq!(size_of::<ReservationStage>(), 1);
    assert_eq!(size_of::<Option<ReservationStage>>(), 1);
    macro_rules! same_layout {
        ($actual:ty, $before:ty) => {
            assert_eq!(size_of::<$actual>(), size_of::<$before>());
            assert_eq!(align_of::<$actual>(), align_of::<$before>());
        };
    }
    same_layout!(ReservationStage, BeforeStage);
    same_layout!(Option<ReservationStage>, Option<BeforeStage>);
    same_layout!(Refused, BeforeRefused);
    same_layout!(Option<Refused>, Option<BeforeRefused>);
    // The independent call-site diagnostic added one pointer after this
    // stage's original control. Retain the entire old budget comparison.
    #[allow(
        dead_code,
        reason = "independent full old budget plus the added call-site pointer"
    )]
    struct BudgetWithSite {
        before: BeforeBudget,
        site: Cell<Option<&'static std::panic::Location<'static>>>,
    }
    same_layout!(HistoricalBudget, BeforeBudget);
    same_layout!(Budget, BudgetWithSite);
    same_layout!(ReservationRefusal, BeforeReceipt);
    same_layout!(ReservationStageGuard, BeforeGuard);
    same_layout!(CfgReservation, BeforePaidCarrier);
}

#[test]
fn canonical_stage_entry_exit_has_no_allocation_or_debit_and_keeps_outer_identity() {
    let (((), metrics), overflow, used, first) = with_limit_usage_refusal(3, || {
        measure(|| {
            let budget = active().unwrap();
            let count = Rc::strong_count(&budget);
            let _outer = reservation_stage_guard(ReservationStage::FreshResolution);
            {
                let _inner = reservation_stage_guard(ReservationStage::CanonicalLoanStorage);
                assert!(Rc::ptr_eq(&active().unwrap(), &budget));
                assert_eq!(Rc::strong_count(&budget), count + 2);
                assert_eq!(
                    budget.stage.get(),
                    Some(ReservationStage::CanonicalLoanStorage)
                );
                assert!(reserve_active_required(3));
            }
            assert_eq!(Rc::strong_count(&budget), count + 1);
            assert_eq!(budget.stage.get(), Some(ReservationStage::FreshResolution));
        })
    });
    assert_eq!((overflow, used, first), (false, 3, None));
    assert!(!metrics.invalid);
    assert_eq!(metrics.allocations, 0);
    assert_eq!(metrics.reallocations, 0);
    assert_eq!(
        (
            metrics.allocated,
            metrics.freed,
            metrics.highwater,
            metrics.live
        ),
        (0, 0, 0, 0)
    );
    let (_, metrics) = measure(|| {
        drop(reservation_stage_guard(
            ReservationStage::CanonicalLoanStorage,
        ))
    });
    assert!(!metrics.invalid);
    assert_eq!(metrics.allocations, 0);
    assert!(active().is_none());
}

#[test]
fn canonical_stage_nested_foreign_detached_unwind_floor_and_sticky_receipts_stay_local() {
    let (escaped, overflow, used, first) = with_limit_usage_refusal(5, || {
        let _outer = reservation_stage_guard(ReservationStage::FreshResolution);
        let parent = active().unwrap();
        let escaped = reservation_stage_guard(ReservationStage::CanonicalLoanStorage);
        let child = with_limit_usage_refusal(2, || {
            assert_eq!(active().unwrap().stage.get(), None);
            let _child = reservation_stage_guard(ReservationStage::HirValidation);
            assert!(reserve_active_required(1));
            // Parent guard belongs to parent even while the child is active.
            drop(escaped);
            assert_eq!(
                active().unwrap().stage.get(),
                Some(ReservationStage::HirValidation)
            );
            assert_eq!(parent.stage.get(), Some(ReservationStage::FreshResolution));
        });
        assert_eq!((child.1, child.2, child.3), (false, 1, None));
        let caught = catch_unwind(AssertUnwindSafe(|| {
            let _stage = reservation_stage_guard(ReservationStage::CanonicalLoanStorage);
            panic!("stage restoration witness");
        }));
        assert!(caught.is_err());
        assert_eq!(parent.stage.get(), Some(ReservationStage::FreshResolution));
        parent.floor.set(2);
        let caught = catch_unwind(AssertUnwindSafe(|| {
            let _stage = reservation_stage_guard(ReservationStage::CanonicalLoanStorage);
            assert!(!reserve_active_required(3));
            panic!("first sticky stage survives unwind");
        }));
        assert!(caught.is_err());
        assert!(!reserve_active_required(0));
        assert!(!reserve_active_required(1));
        reservation_stage_guard(ReservationStage::CanonicalLoanStorage)
    });
    assert!(overflow);
    assert_eq!(used, 1);
    assert_eq!(
        first,
        Some(ReservationRefusal {
            requested: 3,
            remaining: 4,
            floor: 2,
            stage: Some(ReservationStage::CanonicalLoanStorage),
        })
    );
    // Dropping a detached guard while another budget is current restores only
    // its original owner, never the foreign active budget or its first receipt.
    let (_, overflow, used, first) = with_limit_usage_refusal(1, || {
        let _foreign = reservation_stage_guard(ReservationStage::HirValidation);
        drop(escaped);
        assert_eq!(
            active().unwrap().stage.get(),
            Some(ReservationStage::HirValidation)
        );
        assert!(!reserve_active_required(2));
    });
    assert!(overflow);
    assert_eq!(used, 0);
    assert_eq!(first.unwrap().stage, Some(ReservationStage::HirValidation));
    assert!(active().is_none());
}

// Compiled complete pre-site subject, projected from the actual budget below.
#[allow(dead_code, reason = "exact historical layout subject")]
pub(super) struct HistoricalBudget {
    initial: usize,
    remaining: Cell<usize>,
    floor: Cell<usize>,
    overflowed: Cell<bool>,
    first_refusal: Cell<Option<ReservationRefusal>>,
    stage: Cell<Option<ReservationStage>>,
}

#[test]
fn reservation_site_layout_binds_both_complete_original_controls_and_the_full_current_budget() {
    use sha2::{Digest, Sha256};
    let record: serde_json::Value = serde_json::from_str(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/docs/evidence/reservation-site-layout-v1/subjects.json"
    )))
    .unwrap();
    for (name, expected) in [
        (
            "budget_before_site",
            "0c5bb18e9f5e2860ccf6aaaf997df73790ceb5c9fb6c226ad2777c298d6bd1cf",
        ),
        (
            "budget_with_site",
            "9fd668330c1496e9b97fc36bba33faf1b8e23bd90fa78440ded3ff7e61f939e1",
        ),
        (
            "canonical_stage_tests.rs",
            "44a01dc211f3c29f82abcbad190de40ed682e7d44ffb3798db2eb3654bab60ea",
        ),
        (
            "loan_inner_stage_tests.rs",
            "2ff543efc4371754cbb456c79a93e3378471a1a1db482b9c4f81ed43b3add425",
        ),
    ] {
        let body = record[name]["body"].as_str().unwrap();
        assert_eq!(record[name]["sha256"], expected);
        assert_eq!(
            format!(
                "{:x}",
                crate::digest_hex::LowerHex(Sha256::digest(body.as_bytes()))
            ),
            expected
        );
    }
    let before = record["budget_before_site"]["body"].as_str().unwrap();
    let current = record["budget_with_site"]["body"].as_str().unwrap();
    let field = "    first_site: Cell<Option<ReservationSite>>,\n";
    assert_eq!(current.matches(field).count(), 1);
    assert_eq!(current.replacen(field, "", 1), before);
    census_before(
        "src/bounded_output/request_reservation.rs",
        include_bytes!("request_reservation.rs"),
    );
    let restored = census_before(
        "src/bounded_output.rs",
        include_bytes!("../bounded_output.rs"),
    );
    let live = std::str::from_utf8(&restored).unwrap();
    assert_eq!(live.matches(current).count(), 1);
    assert_eq!(
        live.matches("pub(crate) type ReservationSite = &'static std::panic::Location<'static>;")
            .count(),
        1
    );
    let historical = before.replacen("struct Budget {", "pub(super) struct HistoricalBudget {", 1);
    assert_eq!(
        include_str!("canonical_stage_tests.rs")
            .matches(&historical)
            .count(),
        1
    );
    {
        let original = record["canonical_stage_tests.rs"]["body"].as_str().unwrap();
        let old = "    same_layout!(Budget, BeforeBudget);";
        let bridge = "    // The independent call-site diagnostic added one pointer after this\n    // stage's original control. Retain the entire old budget comparison.\n    #[allow(\n        dead_code,\n        reason = \"independent full old budget plus the added call-site pointer\"\n    )]\n    struct BudgetWithSite {\n        before: BeforeBudget,\n        site: Cell<Option<&'static std::panic::Location<'static>>>,\n    }\n    same_layout!(HistoricalBudget, BeforeBudget);\n    same_layout!(Budget, BudgetWithSite);";
        assert_eq!(original.matches(old).count(), 1);
        let current = original.replacen(old, bridge, 1);
        assert_eq!(
            include_str!("canonical_stage_tests.rs")
                .matches(&current)
                .count(),
            1
        );
    }
    {
        let original = record["loan_inner_stage_tests.rs"]["body"]
            .as_str()
            .unwrap();
        let old = "    same!(Budget, BeforeBudget);";
        let bridge = "    // The independent call-site diagnostic added one pointer after this\n    // stage's original control. Retain the entire old budget comparison.\n    #[allow(\n        dead_code,\n        reason = \"independent full old budget plus the added call-site pointer\"\n    )]\n    struct BudgetWithSite {\n        before: BeforeBudget,\n        site: Cell<Option<&'static std::panic::Location<'static>>>,\n    }\n    same!(super::canonical_stage_tests::HistoricalBudget, BeforeBudget);\n    same!(Budget, BudgetWithSite);";
        assert_eq!(original.matches(old).count(), 1);
        let current = original.replacen(old, bridge, 1);
        assert_eq!(
            include_str!("loan_inner_stage_tests.rs")
                .matches(&current)
                .count(),
            1
        );
    }
}

#[path = "reservation_site_e47.rs"]
mod reservation_site_e47;

// Finite feature-only source transport, before any earlier immutable inverse.
// Raw/default and compiled channels must agree; no caller accepts this as proof.
pub(crate) fn census_original_before(path: &str, current: &[u8]) -> Vec<u8> {
    use sha2::{Digest, Sha256};
    let digest = |bytes: &[u8]| format!("{:x}", crate::digest_hex::LowerHex(Sha256::digest(bytes)));
    let encoded = include_str!("../cleanup_plan/replay/path_join/path_census_inverse.json");
    let rows: serde_json::Value = serde_json::from_str(encoded).unwrap();
    let Some(row) = rows["rows"]
        .as_array()
        .unwrap()
        .iter()
        .find(|row| row["path"] == path)
    else {
        return current.to_vec();
    };
    let compiled: &[u8] = match path {
        "crates/semaprax-native-rust-interop-builder/src/implementation/tests/ledger_capacity/source_inventory.rs" => include_bytes!(concat!(env!("CARGO_MANIFEST_DIR"), "/crates/semaprax-native-rust-interop-builder/src/implementation/tests/ledger_capacity/source_inventory.rs")).as_slice(),
        "crates/semaprax-native-rust-interop-builder/src/implementation/tests/ledger_capacity/source_inventory_leaf_views.rs" => include_bytes!(concat!(env!("CARGO_MANIFEST_DIR"), "/crates/semaprax-native-rust-interop-builder/src/implementation/tests/ledger_capacity/source_inventory_leaf_views.rs")).as_slice(),
        "scripts/diagnose-resolved-core.py" => include_bytes!(concat!(env!("CARGO_MANIFEST_DIR"), "/scripts/diagnose-resolved-core.py")).as_slice(),
        "scripts/diagnose-resolved-core.rs" => include_bytes!(concat!(env!("CARGO_MANIFEST_DIR"), "/scripts/diagnose-resolved-core.rs")).as_slice(),
        "src/bounded_output.rs" => include_bytes!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/bounded_output.rs")).as_slice(),
        "src/bounded_output/canonical_stage_tests.rs" => include_bytes!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/bounded_output/canonical_stage_tests.rs")).as_slice(),
        "src/bounded_output/exclusive_debits.rs" => include_bytes!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/bounded_output/exclusive_debits.rs")).as_slice(),
        "src/cleanup_plan/replay/lexical_replay.rs" => include_bytes!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/cleanup_plan/replay/lexical_replay.rs")).as_slice(),
        "src/cleanup_plan/replay/path_join.rs" => include_bytes!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/cleanup_plan/replay/path_join.rs")).as_slice(),
        "src/cleanup_plan/replay/path_join/frontier.rs" => include_bytes!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/cleanup_plan/replay/path_join/frontier.rs")).as_slice(),
        "src/cleanup_plan/replay/path_join/frontier_tests.rs" => include_bytes!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/cleanup_plan/replay/path_join/frontier_tests.rs")).as_slice(),
        "src/cleanup_plan/replay/path_join/frontier_tests/custody.rs" => include_bytes!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/cleanup_plan/replay/path_join/frontier_tests/custody.rs")).as_slice(),
        "src/cleanup_plan/replay/path_join/selected/frontier.rs" => include_bytes!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/cleanup_plan/replay/path_join/selected/frontier.rs")).as_slice(),
        "src/cleanup_plan/replay/path_join/selected/walk.rs" => include_bytes!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/cleanup_plan/replay/path_join/selected/walk.rs")).as_slice(),
        "src/cleanup_plan/replay/scalar_replay.rs" => include_bytes!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/cleanup_plan/replay/scalar_replay.rs")).as_slice(),
        "src/source_reflows_merge/constructor_stage.rs" => include_bytes!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/source_reflows_merge/constructor_stage.rs")).as_slice(),
        "src/workflow_profile/plan_comparison.rs" => include_bytes!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/workflow_profile/plan_comparison.rs")).as_slice(),
        "src/workspace_graph/shape_identity/source_controls.rs" => include_bytes!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/workspace_graph/shape_identity/source_controls.rs")).as_slice(),
        "tests/hir_module_boundaries.rs" => include_bytes!(concat!(env!("CARGO_MANIFEST_DIR"), "/tests/hir_module_boundaries.rs")).as_slice(),
        "tests/workspace/semantic_graph/source_inventory.rs" => include_bytes!(concat!(env!("CARGO_MANIFEST_DIR"), "/tests/workspace/semantic_graph/source_inventory.rs")).as_slice(),
        "src/bounded_output/request_reservation.rs" => include_bytes!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/bounded_output/request_reservation.rs")).as_slice(),
        _ => panic!("undeclared census source"),
    };
    assert_eq!(current, compiled);
    let normalized = census_stage(path, current, false);
    let current = normalized.as_deref().unwrap_or(current);
    assert_eq!(current.len() as u64, row["length"].as_u64().unwrap());
    assert_eq!(digest(current), row["sha256"].as_str().unwrap());
    let mut bytes = current.to_vec();
    for edit in row["edits"].as_array().unwrap().iter().rev() {
        let start = edit[0].as_u64().unwrap() as usize;
        let after = edit[1].as_str().unwrap().as_bytes();
        assert_eq!(bytes.get(start..start + after.len()), Some(after));
        bytes.splice(
            start..start + after.len(),
            edit[2].as_str().unwrap().bytes(),
        );
    }
    assert_eq!(bytes.len() as u64, row["before_length"].as_u64().unwrap());
    assert_eq!(digest(&bytes), row["before_sha256"].as_str().unwrap());
    if path == "src/bounded_output/request_reservation.rs" {
        assert_eq!(row["before_owner"], "src/bounded_output.rs");
        let parent = census_original_before(
            "src/bounded_output.rs",
            include_bytes!("../bounded_output.rs"),
        );
        let start = row["before_start"].as_u64().unwrap() as usize;
        let end = row["before_end"].as_u64().unwrap() as usize;
        assert_eq!(parent.get(start..end), Some(bytes.as_slice()));
    }
    bytes
}

// Separate source acquisition composition: never feed the old82b5 image to a
// later fixed shared-insertions inverse that expects the complete bd8 source.
pub(crate) fn census_before(path: &str, current: &[u8]) -> Vec<u8> {
    let original = census_original_before(path, current);
    census_stage(path, current, true).unwrap_or(original)
}
fn census_stage(path: &str, current: &[u8], predecessor: bool) -> Option<Vec<u8>> {
    use sha2::{Digest, Sha256};
    let digest = |bytes: &[u8]| format!("{:x}", crate::digest_hex::LowerHex(Sha256::digest(bytes)));
    let data: serde_json::Value = serde_json::from_str(include_str!(
        "../cleanup_plan/replay/path_join/census_transport.json"
    ))
    .unwrap();
    assert_eq!(data["schema"], "semaprax.path-census-source-composition.v1");
    assert_eq!(data["e912"], "e9124c411f445ed80200460f2beb40cbdaa625df");
    assert_eq!(data["predecessor"], "bd8a94fac");
    let row = data["rows"]
        .as_array()
        .unwrap()
        .iter()
        .find(|r| r["path"] == path)?;
    let kind = if predecessor { "predecessor" } else { "e912" };
    let target = &row[kind];
    if target.is_null() {
        return None;
    }
    let compiled: &[u8] = match path {
        "crates/semaprax-native-rust-interop-builder/src/implementation/tests/ledger_capacity/source_inventory.rs" => include_bytes!(concat!(env!("CARGO_MANIFEST_DIR"), "/crates/semaprax-native-rust-interop-builder/src/implementation/tests/ledger_capacity/source_inventory.rs")),
        "src/bounded_output.rs" => include_bytes!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/bounded_output.rs")),
        "src/bounded_output/canonical_stage_tests.rs" => include_bytes!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/bounded_output/canonical_stage_tests.rs")),
        "src/cleanup_plan/replay/path_join.rs" => include_bytes!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/cleanup_plan/replay/path_join.rs")),
        "src/cleanup_plan/replay/path_join/frontier.rs" => include_bytes!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/cleanup_plan/replay/path_join/frontier.rs")),
        "src/cleanup_plan/replay/path_join/frontier_tests/custody.rs" => include_bytes!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/cleanup_plan/replay/path_join/frontier_tests/custody.rs")),
        "src/cleanup_plan/replay/path_join/held_edge_tests.rs" => include_bytes!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/cleanup_plan/replay/path_join/held_edge_tests.rs")),
        "src/cleanup_plan/replay/path_join/shared_insertions_tests.rs" => include_bytes!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/cleanup_plan/replay/path_join/shared_insertions_tests.rs")),
        "tests/hir_module_boundaries.rs" => include_bytes!(concat!(env!("CARGO_MANIFEST_DIR"), "/tests/hir_module_boundaries.rs")),
        "tests/workspace/semantic_graph/source_inventory.rs" => include_bytes!(concat!(env!("CARGO_MANIFEST_DIR"), "/tests/workspace/semantic_graph/source_inventory.rs")),
        _ => panic!("undeclared census composition source"),
    };
    assert_eq!(current, compiled);
    assert_eq!(current.len() as u64, row["length"].as_u64().unwrap());
    assert_eq!(digest(current), row["sha256"].as_str().unwrap());
    let (file, length, sha, fixture): (&str, usize, &str, &[u8]) = match (path, predecessor) {
        ("crates/semaprax-native-rust-interop-builder/src/implementation/tests/ledger_capacity/source_inventory.rs", false) => ("e912_00.txt", 227742, "a89396ad310b42c99c219473d6744eee8efb7e498f1275c460673b38427fc2ca", include_bytes!("../cleanup_plan/replay/path_join/census_transport/e912_00.txt")),
        ("src/bounded_output.rs", false) => ("e912_01.txt", 52350, "b964e4a5ddffbf9d8e61d37eeb2cbf819b96c2dd204c91bca0755dc08c8a9664", include_bytes!("../cleanup_plan/replay/path_join/census_transport/e912_01.txt")),
        ("src/bounded_output/canonical_stage_tests.rs", false) => ("e912_02.txt", 19983, "2119b3931a91be3727ca3962c2c01e76f73e7f371b15cdc6635f46b3a206a8bb", include_bytes!("../cleanup_plan/replay/path_join/census_transport/e912_02.txt")),
        ("src/cleanup_plan/replay/path_join.rs", false) => ("e912_03.txt", 53255, "e66afaf7a9241c178c4800d325fdb20a7ff6a6715bf016c839fdb031148b83c9", include_bytes!("../cleanup_plan/replay/path_join/census_transport/e912_03.txt")),
        ("src/cleanup_plan/replay/path_join.rs", true) => ("predecessor_00.txt", 52645, "63b056cf5042a971c20eb8eca2cd6934446362c9b6e140b09bf4cb52e34720a6", include_bytes!("../cleanup_plan/replay/path_join/census_transport/predecessor_00.txt")),
        ("src/cleanup_plan/replay/path_join/frontier.rs", false) => ("e912_04.txt", 14589, "2a6bb7f4c53fd303d7b8ae581e0d210b8a9ecdfc8e0b36e8334cd681681b667d", include_bytes!("../cleanup_plan/replay/path_join/census_transport/e912_04.txt")),
        ("src/cleanup_plan/replay/path_join/frontier.rs", true) => ("predecessor_01.txt", 15531, "0f9c07072519396de6d9aecd059e095c8cbe853a2269ed404dccb00838f91570", include_bytes!("../cleanup_plan/replay/path_join/census_transport/predecessor_01.txt")),
        ("src/cleanup_plan/replay/path_join/frontier_tests/custody.rs", false) => ("e912_05.txt", 13248, "bc0f17e4753b893a398e71c9739d1e165236c3bfcc14f0622e2cd2e6174d27a6", include_bytes!("../cleanup_plan/replay/path_join/census_transport/e912_05.txt")),
        ("src/cleanup_plan/replay/path_join/frontier_tests/custody.rs", true) => ("predecessor_02.txt", 15127, "bdcd62df062f799a561dbaa937d42a0d790ed6b3216648b711da07a263440318", include_bytes!("../cleanup_plan/replay/path_join/census_transport/predecessor_02.txt")),
        ("src/cleanup_plan/replay/path_join/held_edge_tests.rs", true) => ("predecessor_03.txt", 6878, "5b7af349b0c28fb3dbe9fd4adbd58343e45aaf4511f0a8f5ca3a3e2dbce06dfa", include_bytes!("../cleanup_plan/replay/path_join/census_transport/predecessor_03.txt")),
        ("src/cleanup_plan/replay/path_join/shared_insertions_tests.rs", true) => ("predecessor_04.txt", 19690, "acec70169bd33476ef5386646a977ba5a2c098a2c9bb26824cd6e0f3c8da5224", include_bytes!("../cleanup_plan/replay/path_join/census_transport/predecessor_04.txt")),
        ("tests/hir_module_boundaries.rs", false) => ("e912_06.txt", 153194, "29a37b470efcab74c13779e9dcd1b2fac6cd548d7325d2cd1a9a4081afb3af96", include_bytes!("../cleanup_plan/replay/path_join/census_transport/e912_06.txt")),
        ("tests/workspace/semantic_graph/source_inventory.rs", false) => ("e912_07.txt", 181075, "36ae99b2e4319a83f688797315600c1c7f6dcfacbebcc0f5d21637c26155ff9e", include_bytes!("../cleanup_plan/replay/path_join/census_transport/e912_07.txt")),
        _ => panic!("undeclared census composition target"),
    };
    assert_eq!(target["file"], file);
    assert_eq!(target["length"], length);
    assert_eq!(target["sha256"], sha);
    assert_eq!(fixture.len(), length);
    assert_eq!(digest(fixture), sha);
    assert_eq!(
        std::fs::read(
            std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("src/cleanup_plan/replay/path_join/census_transport")
                .join(file)
        )
        .unwrap(),
        fixture
    );
    let mut bytes = current.to_vec();
    for edit in target["edits"].as_array().unwrap().iter().rev() {
        let at = edit[0].as_u64().unwrap() as usize;
        let after = edit[1].as_str().unwrap().as_bytes();
        assert_eq!(bytes.get(at..at + after.len()), Some(after));
        bytes.splice(at..at + after.len(), edit[2].as_str().unwrap().bytes());
    }
    assert_eq!(
        bytes, fixture,
        "complete independently bound census predecessor"
    );
    Some(bytes)
}
