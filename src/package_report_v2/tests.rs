use std::path::Path;

use sha2::{Digest as _, Sha256};

use super::{
    enforce_function_limit, enforce_output_limit, enforce_source_limit, generate, verify_envelope,
    PackageReportV2Options, MAX_CONTRACT_DEPTH, MAX_CONTRACT_NODES, MAX_FUNCTIONS,
    MAX_OUTPUT_BYTES, MAX_REACHABLE_TYPES, MAX_SOURCE_BYTES, TARGET_PROJECTION_MAX_BYTES,
};

fn remint_payload(payload: &str) -> String {
    super::wire::render_envelope(payload)
}

fn envelope_payload(envelope: &str) -> &str {
    let marker = "\"payload\":";
    let offset = envelope.find(marker).expect("payload") + marker.len();
    &envelope[offset..envelope.len() - 1]
}

#[test]
fn self_contained_report_replays_from_authenticated_source() {
    let envelope = generate(
        Path::new("examples/meaning.spx"),
        &PackageReportV2Options::default(),
    )
    .expect("v2 report");
    let receipt = verify_envelope(&envelope).expect("source-bound replay");
    assert_eq!(receipt.package, "examples.meaning");
    assert!(envelope.contains("\"schema\":\"semaprax.canonical-source.v1\""));
    assert!(envelope.contains("\"status\":\"available\""));
}

#[test]
fn self_consistent_semantic_remint_is_rejected() {
    let envelope = generate(
        Path::new("examples/meaning.spx"),
        &PackageReportV2Options::default(),
    )
    .expect("v2 report");
    let payload = envelope_payload(&envelope);
    let tampered_payload = payload.replacen("\"ownership\":\"value\"", "\"ownership\":\"own\"", 1);
    assert_ne!(tampered_payload, payload);
    let reminted = remint_payload(&tampered_payload);
    // Replay derives ownership from embedded verified source even after an
    // attacker re-mints every outer integrity field.
    assert!(verify_envelope(&reminted).is_err());
}

#[test]
fn source_only_mutation_with_full_outer_remint_is_rejected() {
    let envelope = generate(
        Path::new("examples/meaning.spx"),
        &PackageReportV2Options::default(),
    )
    .expect("v2 report");
    let payload = envelope_payload(&envelope);
    let changed = payload.replacen("add(19, 23)", "add(19, 24)", 1);
    assert_ne!(changed, payload);
    assert!(verify_envelope(&remint_payload(&changed)).is_err());
}

#[test]
fn contract_normalization_ignores_display_renames() {
    fn facts(source: &str) -> (Vec<String>, Vec<String>) {
        let program = crate::parse(source, Path::new("rename.spx")).expect("parse");
        let diagnostics = crate::verify::verify(&program);
        assert!(diagnostics.is_empty(), "{diagnostics:?}");
        let resolved = crate::hir::resolve(&program).expect("HIR");
        super::contract::normalize(&resolved.functions[0]).expect("stable contract")
    }
    let before = "module rename;\n@id(\"stable.f\")\nfn before(value: i64) -> i64 requires value >= 0 ensures result == value { value }\n@id(\"app.main\") fn main() -> i64 { 0 }\n";
    let after = "module rename;\n@id(\"stable.f\")\nfn after(renamed: i64) -> i64 requires renamed >= 0 ensures result == renamed { renamed }\n@id(\"app.main\") fn main() -> i64 { 0 }\n";
    assert_eq!(facts(before), facts(after));
}

#[test]
fn valid_contract_requiring_pattern_identity_is_explicitly_unproven() {
    let source = r#"module contract.unproven;
@id("contract.source")
fn source(flag: bool) -> Result<i64, bool> {
    if flag { Result<i64, bool>::Err { error: true } }
    else { Result<i64, bool>::Ok { value: 42 } }
}
@id("contract.subject")
fn subject(flag: bool) -> Result<bool, bool>
    ensures match result {
        Result::Ok { value } => value,
        Result::Err { error } => error,
    }
{
    Result<bool, bool>::Ok { value: flag }
}
@id("app.main") fn main() -> i64 { 0 }
"#;
    let program = crate::parse(source, Path::new("contract-unproven.spx")).expect("parse");
    let diagnostics = crate::verify::verify(&program);
    assert!(diagnostics.is_empty(), "{diagnostics:?}");
    let canonical = crate::format::canonical(&program);
    let envelope =
        super::build_from_canonical_source(&canonical, &PackageReportV2Options::default())
            .expect("v2 report");
    assert!(envelope.contains(
        "\"stable_id\":\"contract.subject\",\"name\":\"subject\",\"reason\":\"contract_identity_unproven\""
    ));
}

#[test]
fn target_proof_states_are_closed_and_fail_closed() {
    use super::model::{render_target_fact, target_projection, TargetProof};

    let unavailable = target_projection("x", "p", false, || panic!("must not project"));
    assert!(matches!(unavailable, TargetProof::Unavailable { .. }));
    assert_eq!(
        render_target_fact(&unavailable),
        "{\"target\":\"x\",\"status\":\"unavailable\",\"proof\":\"closed_source_export_inventory\",\"reason\":\"no_explicit_monomorphic_export\",\"execution\":\"unproven\"}"
    );
    let available = target_projection("x", "p", true, || Ok(()));
    assert!(matches!(available, TargetProof::Available { .. }));
    assert_eq!(
        render_target_fact(&available),
        "{\"target\":\"x\",\"status\":\"available\",\"proof\":\"p\",\"reason\":\"none\",\"execution\":\"unproven\"}"
    );
    let rejected = target_projection("x", "p", true, || {
        Err(crate::diagnostic::Diagnostic::io("SPX-T", "rejected"))
    });
    assert!(matches!(rejected, TargetProof::Unproven { .. }));
    assert_eq!(
        render_target_fact(&rejected),
        "{\"target\":\"x\",\"status\":\"unproven\",\"proof\":\"none\",\"reason\":\"projection_rejected\",\"execution\":\"unproven\"}"
    );
    let overflow = target_projection("x", "p", true, || {
        assert!(!crate::bounded_output::reserve_active(
            TARGET_PROJECTION_MAX_BYTES + 1
        ));
        Ok(())
    });
    assert!(matches!(overflow, TargetProof::Unproven { .. }));
}

#[test]
fn frozen_limit_helpers_accept_exact_and_reject_first_overflow() {
    assert!(enforce_source_limit(MAX_SOURCE_BYTES).is_ok());
    assert!(enforce_source_limit(MAX_SOURCE_BYTES + 1).is_err());
    assert!(enforce_function_limit(MAX_FUNCTIONS).is_ok());
    assert!(enforce_function_limit(MAX_FUNCTIONS + 1).is_err());
    assert!(enforce_output_limit(MAX_OUTPUT_BYTES, MAX_OUTPUT_BYTES).is_ok());
    assert!(enforce_output_limit(MAX_OUTPUT_BYTES + 1, MAX_OUTPUT_BYTES).is_err());

    let mut nodes = 0;
    assert!(
        super::model::admit_contract_shape(&mut nodes, MAX_CONTRACT_NODES, MAX_CONTRACT_DEPTH)
            .expect("exact")
    );
    assert!(
        super::model::admit_contract_shape(&mut 0, 1, MAX_CONTRACT_DEPTH + 1)
            .is_ok_and(|admitted| !admitted)
    );
    assert!(super::model::admit_contract_shape(&mut nodes, 1, 1).is_err());
    assert!(super::model::admit_reachable_type_count(MAX_REACHABLE_TYPES).is_ok());
    assert!(super::model::admit_reachable_type_count(MAX_REACHABLE_TYPES + 1).is_err());
}

#[test]
fn directly_constructed_invalid_options_reject_before_subject_work() {
    let below = PackageReportV2Options {
        max_bytes: crate::graph::MIN_AGENT_CONTEXT_BYTES - 1,
    };
    let above = PackageReportV2Options {
        max_bytes: MAX_OUTPUT_BYTES + 1,
    };
    assert_eq!(
        super::build_from_canonical_source("not parsed", &below).unwrap_err()[0].code,
        "SPX-P401"
    );
    assert_eq!(
        super::build_from_canonical_source("not parsed", &above).unwrap_err()[0].code,
        "SPX-P401"
    );
    assert_eq!(
        generate(Path::new("does-not-exist.spx"), &below).unwrap_err()[0].code,
        "SPX-P401"
    );
}

#[test]
fn malformed_duplicate_extra_and_noncanonical_subjects_fail_closed() {
    let envelope = generate(
        Path::new("examples/meaning.spx"),
        &PackageReportV2Options::default(),
    )
    .expect("v2 report");
    assert!(verify_envelope(&envelope.replacen('{', "{\"extra\":0,", 1)).is_err());
    assert!(verify_envelope(&envelope.replacen(
        "\"schema\":",
        "\"schema\":\"duplicate\",\"schema\":",
        1
    ))
    .is_err());
    let canonical = include_str!("../../examples/meaning.spx");
    let noncanonical = canonical.replace("fn add", "fn  add");
    assert!(
        super::build_from_canonical_source(&noncanonical, &PackageReportV2Options::default())
            .is_err()
    );
}

#[test]
fn legacy_v1_golden_bytes_remain_frozen() {
    let envelope = crate::package_report::generate(
        Path::new("examples/meaning.spx"),
        &crate::package_report::PackageReportOptions::default(),
    )
    .expect("v1 report");
    let mut hasher = Sha256::new();
    hasher.update(envelope.as_bytes());
    assert_eq!(
        format!(
            "sha256:{:x}",
            crate::digest_hex::LowerHex(hasher.finalize())
        ),
        "sha256:97bcde287804d9311f343157058926fb0648e66282461ede138e98824aac06f2"
    );
}
