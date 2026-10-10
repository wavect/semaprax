//! Issue #103's finite feature-composition matrix inventory.
//!
//! Runtime owners below retain their independent backend observations.  This
//! inventory prevents a quieter regression: a corpus document that still
//! claims a row after its owner or module registration has disappeared.  It
//! deliberately binds declarations and registrations only; source scans never
//! substitute for the named execution gates.

const CORPUS_DOCUMENT: &str = include_str!("../../../docs/FEATURE-COMPOSITION-CORPUS-V1.md");

const SCALAR_HARNESS: &str = include_str!("../../scalar_status_backend_equivalence.rs");
const SCALAR_DIFFERENTIAL: &str =
    include_str!("../../scalar_status_backend_equivalence/differential.rs");
const VIEW_OWNERSHIP: &str =
    include_str!("../../scalar_status_backend_equivalence/view_ownership_composition.rs");
const VIEW_CALL_BOUNDARY: &str =
    include_str!("../../scalar_status_backend_equivalence/view_call_boundary_composition.rs");

const PROJECT_HARNESS: &str = concat!(
    include_str!("../standard_library.rs"),
    include_str!("wasm_host.rs"),
    include_str!("backend_conformance.rs"),
    include_str!("pattern_conformance.rs")
);
const IMPORTED_VIEW: &str = include_str!("imported_view_composition.rs");
const OWNED_FAILURE: &str = include_str!("owned_failure_composition.rs");

const GENERIC_HARNESS: &str = include_str!("../../public_generic_native_adapter_v1.rs");
const GENERIC_SETTLEMENT: &str =
    include_str!("../../public_generic_native_adapter_v1/settlement_corpus.rs");
const GENERIC_SETTLEMENT_CHILD: &str = include_str!(
    "../../public_generic_native_adapter_v1/settlement_corpus/divergence_7_compounding_cleanup.rs"
);
const ALLOCATION_REJECTION: &str = include_str!(
    "../../public_generic_native_adapter_v1/settlement_corpus/allocation_rejection_order.rs"
);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Registration {
    source: &'static str,
    declaration: &'static str,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct CorpusRow {
    id: &'static str,
    source: &'static str,
    owner: &'static str,
    registrations: &'static [Registration],
    required_markers: &'static [&'static str],
}

const SCALAR_DIFFERENTIAL_REGISTRATION: [Registration; 1] = [Registration {
    source: SCALAR_HARNESS,
    declaration:
        "#[path = \"scalar_status_backend_equivalence/differential.rs\"]\nmod differential;",
}];
const VIEW_OWNERSHIP_REGISTRATION: [Registration; 1] = [Registration {
    source: SCALAR_HARNESS,
    declaration: "#[path = \"scalar_status_backend_equivalence/view_ownership_composition.rs\"]\nmod view_ownership_composition;",
}];
const VIEW_CALL_REGISTRATION: [Registration; 1] = [Registration {
    source: SCALAR_HARNESS,
    declaration: "#[path = \"scalar_status_backend_equivalence/view_call_boundary_composition.rs\"]\nmod view_call_boundary_composition;",
}];
const IMPORTED_VIEW_REGISTRATION: [Registration; 1] = [Registration {
    source: PROJECT_HARNESS,
    declaration: "#[path = \"standard_library/imported_view_composition.rs\"]\nmod imported_view_composition;",
}];
const OWNED_FAILURE_REGISTRATION: [Registration; 1] = [Registration {
    source: PROJECT_HARNESS,
    declaration: "#[path = \"standard_library/owned_failure_composition.rs\"]\nmod owned_failure_composition;",
}];
const GENERIC_SETTLEMENT_REGISTRATION: [Registration; 1] = [Registration {
    source: GENERIC_HARNESS,
    declaration: "#[path = \"public_generic_native_adapter_v1/settlement_corpus.rs\"]\nmod settlement_corpus;",
}];
const ALLOCATION_REJECTION_REGISTRATIONS: [Registration; 3] = [
    Registration {
        source: GENERIC_HARNESS,
        declaration: "#[path = \"public_generic_native_adapter_v1/settlement_corpus.rs\"]\nmod settlement_corpus;",
    },
    Registration {
        source: GENERIC_SETTLEMENT,
        declaration: "#[path = \"settlement_corpus/divergence_7_compounding_cleanup.rs\"]\nmod divergence_7_compounding_cleanup;",
    },
    Registration {
        source: GENERIC_SETTLEMENT_CHILD,
        declaration: "#[path = \"allocation_rejection_order.rs\"]\nmod allocation_rejection_order;",
    },
];

const CORPUS_ROWS: [CorpusRow; 10] = [
    CorpusRow {
        id: "scalar.fixed-status-v1",
        source: SCALAR_HARNESS,
        owner: "native_o0_o2_and_core_wasm_share_exact_scalar_status_results",
        registrations: &[],
        required_markers: &["EXPORT_IDS", "EXPECTED_TRANSCRIPT"],
    },
    CorpusRow {
        id: "scalar.view-origin-context-v1",
        source: VIEW_OWNERSHIP,
        owner: "view_ownership_composition_agrees_across_interpreter_native_and_core_wasm",
        registrations: &VIEW_OWNERSHIP_REGISTRATION,
        required_markers: &["CASES", "decode_case_values"],
    },
    CorpusRow {
        id: "scalar.view-ownership-route-v1",
        source: VIEW_OWNERSHIP,
        owner: "ownership_route_agrees_across_interpreter_and_native",
        registrations: &VIEW_OWNERSHIP_REGISTRATION,
        required_markers: &[
            "OWNERSHIP_CASES",
            "scalar_export_profile_refuses_ownership_transfer_with_spx_w115",
        ],
    },
    CorpusRow {
        id: "scalar.view-call-boundary-v1",
        source: VIEW_CALL_BOUNDARY,
        owner: "borrowed_view_call_matrix_agrees_across_interpreter_native_and_core_wasm",
        registrations: &VIEW_CALL_REGISTRATION,
        required_markers: &[
            "CASES",
            "borrowed_view_return_is_rejected_with_stable_diagnostic",
        ],
    },
    CorpusRow {
        id: "scalar.lazy-checked-failure-v1",
        source: SCALAR_DIFFERENTIAL,
        owner: "lazy_short_circuit_skips_checked_failure_across_every_available_lane",
        registrations: &SCALAR_DIFFERENTIAL_REGISTRATION,
        required_markers: &["lazy_checked_failure_module", "BinaryOp::Div"],
    },
    CorpusRow {
        id: "scalar.seeded-differential-v1",
        source: SCALAR_DIFFERENTIAL,
        owner: "fixed_seeds_agree_across_every_available_lane",
        registrations: &SCALAR_DIFFERENTIAL_REGISTRATION,
        required_markers: &[
            "PR_SEEDS",
            "the_frontend_and_reference_interpreter_agree_on_a_wider_seed_sweep",
            "bounded_campaign_agrees_across_every_available_lane",
        ],
    },
    CorpusRow {
        id: "project.imported-offset-view-v1",
        source: IMPORTED_VIEW,
        owner: "imported_std_bytes_view_composition_agrees_across_project_backends",
        registrations: &IMPORTED_VIEW_REGISTRATION,
        required_markers: &[
            "std.bytes.field_start",
            "byte_range",
            "assert_backend_value",
        ],
    },
    CorpusRow {
        id: "project.owned-cursor-contract-v1",
        source: OWNED_FAILURE,
        owner: "owned_cursor_chain_settles_before_sticky_contract_failure_on_project_backends",
        registrations: &OWNED_FAILURE_REGISTRATION,
        required_markers: &[
            "CONTRACT_REQUIRES_FALSE_CODE",
            "EXPECTED_DROPS",
            "run_wasm_settlement_oracle",
        ],
    },
    CorpusRow {
        id: "generic.result-allocation-rejection-v1",
        source: ALLOCATION_REJECTION,
        owner: "result_allocation_rejection_preserves_ordered_input_release",
        registrations: &ALLOCATION_REJECTION_REGISTRATIONS,
        required_markers: &["AllocationFailure", "assert_reverse_release_order"],
    },
    CorpusRow {
        id: "generic.boundary-input-v1",
        source: GENERIC_SETTLEMENT,
        owner:
            "native_o0_and_o2_agree_with_interpreter_and_wasm_across_the_shared_settlement_corpus",
        registrations: &GENERIC_SETTLEMENT_REGISTRATION,
        required_markers: &["fn corpus()", "compare_case("],
    },
];

#[derive(Debug, PartialEq, Eq)]
enum InventoryError {
    DuplicateRow(&'static str),
    MissingRow(&'static str),
    MissingDocumentRow(&'static str),
    MissingModuleRegistration(&'static str),
    MissingOwner(&'static str),
    MissingSemanticMarker {
        row: &'static str,
        marker: &'static str,
    },
}

fn documented_rows(document: &str) -> Vec<&str> {
    document
        .lines()
        .filter_map(|line| line.strip_prefix("| `"))
        .filter_map(|line| line.split_once('`'))
        .map(|(id, _)| id)
        .filter(|id| id.ends_with("-v1"))
        .collect()
}

fn validate_inventory(
    expected: &[&'static str],
    rows: &[CorpusRow],
    document: &str,
) -> Result<(), InventoryError> {
    let mut seen = std::collections::BTreeSet::new();
    for row in rows {
        if !seen.insert(row.id) {
            return Err(InventoryError::DuplicateRow(row.id));
        }
        if !documented_rows(document).contains(&row.id) {
            return Err(InventoryError::MissingDocumentRow(row.id));
        }
        if !row.source.contains(&format!("fn {}(", row.owner)) {
            return Err(InventoryError::MissingOwner(row.id));
        }
        for registration in row.registrations {
            if !registration.source.contains(registration.declaration) {
                return Err(InventoryError::MissingModuleRegistration(row.id));
            }
        }
        for marker in row.required_markers {
            if !row.source.contains(marker) {
                return Err(InventoryError::MissingSemanticMarker {
                    row: row.id,
                    marker,
                });
            }
        }
    }
    for id in expected {
        if !seen.contains(id) {
            return Err(InventoryError::MissingRow(id));
        }
    }
    Ok(())
}

#[test]
fn feature_composition_rows_are_registered_and_documented() {
    assert!(CORPUS_DOCUMENT.contains("semaprax.feature-composition-corpus.v1"));
    let expected = CORPUS_ROWS.map(|row| row.id);
    assert_eq!(
        documented_rows(CORPUS_DOCUMENT).as_slice(),
        expected.as_slice()
    );
    validate_inventory(&expected, &CORPUS_ROWS, CORPUS_DOCUMENT).unwrap();
}

#[test]
fn feature_composition_inventory_rejects_missing_and_duplicate_rows() {
    let expected = CORPUS_ROWS.map(|row| row.id);
    let missing = &CORPUS_ROWS[..CORPUS_ROWS.len() - 1];
    assert_eq!(
        validate_inventory(&expected, missing, CORPUS_DOCUMENT),
        Err(InventoryError::MissingRow("generic.boundary-input-v1"))
    );
    let duplicate = [CORPUS_ROWS[0], CORPUS_ROWS[0]];
    assert_eq!(
        validate_inventory(&[CORPUS_ROWS[0].id], &duplicate, CORPUS_DOCUMENT),
        Err(InventoryError::DuplicateRow("scalar.fixed-status-v1"))
    );
}
