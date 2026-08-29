use serde_json::Value;

use super::support::*;

#[test]
fn canonical_manifest_evidence_and_receipt_are_exactly_bound() {
    let fixture = fixture();
    let receipt = package_build::verify(
        &fixture.build,
        &fixture.resolution,
        &fixture.input,
        &ResolutionOptions::default(),
        &fixture.options,
    )
    .expect("independent package build replay");

    assert_eq!(receipt.root_package, ROOT);
    assert_eq!(receipt.packages, vec![coordinate(ROOT, "1.0.0")]);
    assert_eq!(receipt.artifact_bytes, artifact_bytes(&fixture.build));
    assert_eq!(receipt.wasm_sha256.len(), 71);
    assert!(receipt.wasm_sha256.starts_with("sha256:"));
    assert!(
        receipt.wasm_sha256.as_bytes()[7..]
            .iter()
            .all(|byte| byte.is_ascii_digit() || matches!(byte, b'a'..=b'f'))
    );
    assert!(!fixture.build.manifest_json.ends_with('\n'));
    assert!(!fixture.build.evidence_json.ends_with('\n'));

    assert_ordered(
        &fixture.build.manifest_json,
        &[
            "\"schema\":",
            "\"profile\":",
            "\"root\":",
            "\"packages\":",
            "\"exports\":",
            "\"runtime_imports\":",
            "\"module\":",
            "\"compiler\":",
            "\"limits\":",
            "\"nonclaims\":",
        ],
    );
    assert_ordered(
        &fixture.build.evidence_json,
        &["\"schema\":", "\"digest\":", "\"bytes\":", "\"payload\":"],
    );
    assert_ordered(
        payload(&fixture.build.evidence_json),
        &[
            "\"schema\":",
            "\"resolution_digest\":",
            "\"resolution_bytes\":",
            "\"lock_digest\":",
            "\"lock_bytes\":",
            "\"subjects\":",
            "\"root\":",
            "\"exports\":",
            "\"package_source_set_digest\":",
            "\"package_link_digest\":",
            "\"manifest_digest\":",
            "\"manifest_bytes\":",
            "\"wasm_digest\":",
            "\"wasm_bytes\":",
            "\"limits\":",
            "\"budget\":",
            "\"nonclaims\":",
        ],
    );
    assert_eq!(
        serde_json::from_str::<Value>(&fixture.build.manifest_json).unwrap()["schema"],
        BUILD_SCHEMA
    );
    assert_eq!(
        serde_json::from_str::<Value>(&fixture.build.evidence_json).unwrap()["schema"],
        EVIDENCE_SCHEMA
    );
}

#[test]
fn noncanonical_or_foreign_wire_is_rejected_before_semantic_receipt() {
    let fixture = fixture();

    let mut whitespace_manifest = copied_build(&fixture.build);
    whitespace_manifest.manifest_json.insert(1, ' ');
    assert_eq!(verify_error(&whitespace_manifest, &fixture), "SPX-PB506");

    let mut terminal_lf = copied_build(&fixture.build);
    terminal_lf.evidence_json.push('\n');
    assert_eq!(verify_error(&terminal_lf, &fixture), "SPX-PB506");

    let mut duplicate = copied_build(&fixture.build);
    duplicate.evidence_json = duplicate.evidence_json.replacen(
        "{\"schema\":",
        "{\"schema\":\"foreign\",\"schema\":",
        1,
    );
    assert_eq!(verify_error(&duplicate, &fixture), "SPX-PB506");

    let mut foreign_manifest_member = copied_build(&fixture.build);
    foreign_manifest_member.manifest_json = foreign_manifest_member
        .manifest_json
        .replacen("{\"schema\":", "{\"foreign\":0,\"schema\":", 1);
    assert_eq!(
        verify_error(&foreign_manifest_member, &fixture),
        "SPX-PB506"
    );

    let mut reordered = copied_build(&fixture.build);
    let value: Value = serde_json::from_str(&reordered.evidence_json).unwrap();
    let exact_payload = payload(&reordered.evidence_json);
    reordered.evidence_json = format!(
        "{{\"digest\":{},\"schema\":{},\"bytes\":{},\"payload\":{}}}",
        serde_json::to_string(&value["digest"]).unwrap(),
        serde_json::to_string(&value["schema"]).unwrap(),
        value["bytes"],
        exact_payload,
    );
    assert_eq!(verify_error(&reordered, &fixture), "SPX-PB506");
}

#[test]
fn correctly_reminted_semantic_mutation_still_fails_exact_replay() {
    let fixture = fixture();
    let mutated_payload = mutate_decimal_member(payload(&fixture.build.evidence_json), "wasm_bytes");
    let mut reminted = copied_build(&fixture.build);
    reminted.evidence_json = remint_evidence(&mutated_payload);
    assert_eq!(verify_error(&reminted, &fixture), "SPX-PB507");

    let mut digest_flip = copied_build(&fixture.build);
    let digest = digest_flip
        .evidence_json
        .find("sha256:")
        .expect("evidence digest");
    let index = digest + "sha256:".len();
    let replacement = if digest_flip.evidence_json.as_bytes()[index] == b'f' {
        "e"
    } else {
        "f"
    };
    digest_flip
        .evidence_json
        .replace_range(index..index + 1, replacement);
    assert_eq!(verify_error(&digest_flip, &fixture), "SPX-PB507");
}
