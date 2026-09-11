use std::collections::BTreeSet;
use std::path::Path;

use serde_json::Value;

use super::*;

/// The pinned, committed bundle this compiler ships. Regenerating this file
/// with `regenerate_committed_bundle` (below) and re-running
/// `committed_bundle_is_pinned_and_regenerates_byte_identical` is the drift
/// gate: a version bump, a public-workflow edit, or catalog drift that is not
/// reflected here fails the test closed rather than silently shipping a
/// stale bundle.
const COMMITTED_BUNDLE: &str = include_str!("../../docs/AGENT-SKILL-BUNDLE-V1.json");

const REQUIRED_VERBS: &[&str] = &[
    "apply", "context", "impact", "inspect", "propose", "publish", "rebase", "repair", "review",
    "test",
];

#[test]
fn generation_is_byte_identical_on_repetition() {
    let first = generate_agent_skill_bundle().expect("bundle generates");
    let second = generate_agent_skill_bundle().expect("bundle generates");
    assert_eq!(first, second, "repeated generation must be byte-identical");
}

#[test]
fn committed_bundle_is_pinned_and_regenerates_byte_identical() {
    let generated = generate_agent_skill_bundle().expect("bundle generates");
    assert_eq!(
        COMMITTED_BUNDLE, generated,
        "docs/AGENT-SKILL-BUNDLE-V1.json is stale; regenerate it with the \
         ignored `regenerate_committed_bundle` test"
    );
}

#[test]
#[ignore = "regenerates the pinned agent skill bundle; run after an intentional bundle change"]
fn regenerate_committed_bundle() {
    let generated = generate_agent_skill_bundle().expect("bundle generates");
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("docs/AGENT-SKILL-BUNDLE-V1.json");
    std::fs::write(path, generated).expect("bundle writes");
}

/// Extract the exact `payload` substring the same way
/// `semantic_discovery::verify_discovery_manifest_against_source` does: a
/// `serde_json::Value` round-trip re-serializes object keys in `BTreeMap`
/// order, which would silently reorder this envelope's deliberately
/// hand-ordered top-level payload keys and produce a byte sequence the
/// generator itself never emitted.
fn exact_payload_text(envelope: &str) -> &str {
    const PAYLOAD_KEY: &str = "\"payload\":";
    let offset = envelope.find(PAYLOAD_KEY).expect("envelope has a payload");
    assert!(envelope.ends_with('}'));
    &envelope[offset + PAYLOAD_KEY.len()..envelope.len() - 1]
}

#[test]
fn envelope_is_well_formed_and_bounded() {
    let envelope = generate_agent_skill_bundle().expect("bundle generates");
    assert!(envelope.len() <= MAX_AGENT_SKILL_BUNDLE_BYTES);
    let value: Value = serde_json::from_str(&envelope).expect("valid JSON");
    assert_eq!(value["schema"], AGENT_SKILL_SCHEMA);
    let payload_text = exact_payload_text(&envelope);
    assert_eq!(value["bytes"], payload_text.len());
    assert_eq!(
        value["digest"],
        domain_digest(AGENT_SKILL_DOMAIN, payload_text.as_bytes())
    );
    let payload = &value["payload"];
    assert_eq!(payload["schema"], AGENT_SKILL_SCHEMA);
    assert_eq!(payload["authority"], false);
    assert_eq!(payload["compiler"]["package"], env!("CARGO_PKG_NAME"));
    assert_eq!(payload["compiler"]["version"], env!("CARGO_PKG_VERSION"));
    assert_eq!(payload["compiler"]["binary_identity_claimed"], false);
    assert_eq!(
        payload["discovery"]["schema"],
        semantic_discovery::DISCOVERY_SCHEMA
    );
}

#[test]
fn public_workflow_is_sorted_by_verb_and_covers_the_required_ten_verbs() {
    let verbs: Vec<&str> = PUBLIC_WORKFLOW.iter().map(|entry| entry.verb).collect();
    let mut sorted = verbs.clone();
    sorted.sort_unstable();
    assert_eq!(
        verbs, sorted,
        "PUBLIC_WORKFLOW must be sorted by verb bytes"
    );

    let mut required = REQUIRED_VERBS.to_vec();
    required.sort_unstable();
    assert_eq!(
        verbs, required,
        "the public workflow must be exactly the ten verbs issue #196 requires, no more, no fewer"
    );
}

#[test]
fn authority_classes_cover_exactly_the_used_set_and_are_sorted() {
    let mut sorted = AUTHORITY_CLASSES.to_vec();
    sorted.sort_unstable();
    sorted.dedup();
    assert_eq!(
        AUTHORITY_CLASSES.to_vec(),
        sorted,
        "AUTHORITY_CLASSES must be sorted and duplicate-free"
    );
    let used: BTreeSet<&str> = PUBLIC_WORKFLOW
        .iter()
        .map(|entry| entry.authority_class)
        .collect();
    let closed: BTreeSet<&str> = AUTHORITY_CLASSES.iter().copied().collect();
    assert_eq!(
        used, closed,
        "every declared authority class must be used, and every used class must be declared"
    );
}

#[test]
fn target_profiles_are_sorted_and_closed() {
    let mut sorted = TARGET_PROFILES.to_vec();
    sorted.sort_unstable();
    assert_eq!(TARGET_PROFILES.to_vec(), sorted);
    assert_eq!(TARGET_PROFILES, &["core-wasm", "interpreter", "native-c11"]);
}

#[test]
fn every_public_workflow_command_names_an_existing_top_level_cli_surface() {
    // The exact set of top-level canonical command names this compiler
    // admits is only visible from the binary (`cli::help::COMMANDS`), so the
    // authoritative cross-check lives there:
    // `cli::agent::tests::public_workflow_commands_are_all_catalogued`. This
    // lib-side test only pins the small, closed set of distinct commands the
    // table currently names, so an edit here is visible in review.
    let commands: BTreeSet<&str> = PUBLIC_WORKFLOW
        .iter()
        .map(|entry| entry.cli_command)
        .collect();
    assert_eq!(
        commands,
        BTreeSet::from([
            "apply-semantic-workspace-change-evidence",
            "change",
            "context",
            "graph",
            "impact",
            "project-candidate-git-publish",
            "repair",
            "review",
            "test",
        ])
    );
}

#[test]
fn package_status_matches_the_bundled_standard_library_catalog() {
    let status = package_status().expect("package status computes");
    let catalog: Value = serde_json::from_str(STDLIB_CATALOG).unwrap();
    assert_eq!(status["schema"], catalog["schema"]);
    assert_eq!(
        status["module_count"],
        catalog["modules"].as_array().unwrap().len()
    );
    assert_eq!(
        status["digest"],
        domain_digest(STDLIB_CATALOG_DOMAIN, STDLIB_CATALOG.as_bytes())
    );
}

#[test]
fn negotiate_agent_skill_schema_accepts_exact_match_and_rejects_drift() {
    negotiate_agent_skill_schema(AGENT_SKILL_SCHEMA).expect("exact schema must negotiate");
    let error =
        negotiate_agent_skill_schema("semaprax.agent-skill.v2").expect_err("must not fall back");
    assert_eq!(error.code, "SPX-G587");
    let error = negotiate_agent_skill_schema("").expect_err("must not accept an empty schema");
    assert_eq!(error.code, "SPX-G587");
}

#[test]
fn discovery_operations_embed_the_full_live_catalog_including_this_bundle_s_own_entry() {
    let envelope = generate_agent_skill_bundle().expect("bundle generates");
    let value: Value = serde_json::from_str(&envelope).unwrap();
    let operations = value["payload"]["discovery"]["operations"]
        .as_array()
        .expect("operations array");
    let names: Vec<&str> = operations
        .iter()
        .map(|entry| entry["name"].as_str().unwrap())
        .collect();
    assert!(
        names.contains(&"agent_skill"),
        "the discovery catalog embedded here must list this bundle's own entry"
    );
    let mut sorted = names.clone();
    sorted.sort_unstable();
    assert_eq!(names, sorted);
}
