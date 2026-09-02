use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicUsize, Ordering};

use semaprax::{graph, parse, patch, patch_evidence, repair};
use sha2::{Digest, Sha256};

static NEXT_FIXTURE: AtomicUsize = AtomicUsize::new(0);

struct Fixture {
    directory: PathBuf,
    source: PathBuf,
    patch: PathBuf,
    evidence: PathBuf,
}

impl Fixture {
    fn new(label: &str, source: &str, patch: &str) -> Self {
        let sequence = NEXT_FIXTURE.fetch_add(1, Ordering::Relaxed);
        let directory = std::env::temp_dir().join(format!(
            "semaprax-patch-evidence-{}-{label}-{sequence}",
            std::process::id()
        ));
        std::fs::create_dir_all(&directory).unwrap();
        let source_path = directory.join("module.spx");
        let patch_path = directory.join("change.spatch");
        let evidence_path = directory.join("evidence.json");
        std::fs::write(&source_path, source).unwrap();
        std::fs::write(&patch_path, patch).unwrap();
        Self {
            directory,
            source: source_path,
            patch: patch_path,
            evidence: evidence_path,
        }
    }

    fn inventory(&self) -> BTreeSet<String> {
        std::fs::read_dir(&self.directory)
            .unwrap()
            .map(|entry| entry.unwrap().file_name().to_string_lossy().into_owned())
            .collect()
    }

    fn assert_no_a0_artifacts(&self) {
        assert!(self.inventory().iter().all(|name| {
            !(name.ends_with(".semaprax-patch.lock")
                || name.contains(".semaprax-stage.") && name.ends_with(".tmp"))
        }));
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        std::fs::remove_dir_all(&self.directory).unwrap();
    }
}

fn revision(source: &str) -> String {
    graph::revision(&parse(source, Path::new("evidence.spx")).unwrap())
}

fn sha256(value: &str) -> String {
    format!(
        "{:x}",
        semaprax::digest_hex::LowerHex(Sha256::digest(value.as_bytes()))
    )
}

fn artifact_digest(value: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(b"semaprax.semantic-patch-evidence.artifact-digest.v1\0");
    hasher.update((value.len() as u64).to_le_bytes());
    hasher.update(value.as_bytes());
    format!(
        "sha256:{:x}",
        semaprax::digest_hex::LowerHex(hasher.finalize())
    )
}

fn replace_and_reaccount(capsule: &str, needle: &str, replacement: &str) -> String {
    assert_eq!(capsule.matches(needle).count(), 1, "{needle}");
    let original: serde_json::Value = serde_json::from_str(capsule).unwrap();
    let original_size = original["budget"]["used_evidence_bytes"].as_u64().unwrap();
    let mut changed = capsule.replacen(needle, replacement, 1);
    for _ in 0..4 {
        let marker = format!("\"used_evidence_bytes\":{original_size}");
        if changed.contains(&marker) {
            changed = changed.replacen(
                &marker,
                &format!("\"used_evidence_bytes\":{}", changed.len()),
                1,
            );
        } else {
            let start = changed.find("\"used_evidence_bytes\":").unwrap()
                + "\"used_evidence_bytes\":".len();
            let end = start
                + changed[start..]
                    .bytes()
                    .take_while(u8::is_ascii_digit)
                    .count();
            let length = changed.len();
            changed.replace_range(start..end, &length.to_string());
        }
        let parsed: serde_json::Value = serde_json::from_str(&changed).unwrap();
        if parsed["budget"]["used_evidence_bytes"] == changed.len() {
            return changed;
        }
    }
    panic!("evidence length accounting did not converge")
}

fn changed_digest(value: &str) -> String {
    let mut changed = value.to_owned();
    let replacement = if changed.ends_with('0') { '1' } else { '0' };
    changed.pop();
    changed.push(replacement);
    changed
}

fn v1_fixture(label: &str) -> Fixture {
    let source = "module evidence.rename;\n@id(\"evidence.helper\") fn helper()->i64{41}\n@id(\"app.main\") fn main()->i64{helper()+1}\n";
    let patch = format!(
        "# exact patch bytes\nbase {}\nrename evidence.helper to answer\nrequire no-new-effects\n",
        revision(source)
    );
    Fixture::new(label, source, &patch)
}

fn v2_fixture(label: &str) -> Fixture {
    let source = "module evidence.rename_v2;\n@id(\"evidence.helper\") fn helper()->i64{41}\n@id(\"app.main\") fn main()->i64{helper()+1}\n";
    let patch = format!(
        "schema semaprax.semantic-patch.v2\nbase {}\nrename evidence.helper to answer\nrequire no-new-effects\n",
        revision(source)
    );
    Fixture::new(label, source, &patch)
}

fn v3_fixture(label: &str) -> Fixture {
    let source = "module evidence.rebase;\nfn helper(value:i64)->i64{value+1}\n@id(\"evidence.caller\") fn caller(value:i64)->i64{helper(value)}\n@id(\"app.main\") fn main()->i64{caller(41)}\n";
    let fixture = Fixture::new(label, source, "");
    let query =
        repair::DiagnosticRepairQuery::assign_function_id("auto:evidence.rebase.helper").unwrap();
    let repairs: serde_json::Value =
        serde_json::from_str(&repair::query(&fixture.source, &query).unwrap()).unwrap();
    let preview: serde_json::Value = serde_json::from_str(
        &repair::instantiate(
            &fixture.source,
            repairs["repair"]["id"].as_str().unwrap(),
            &repair::PersistentDeclarationId::new("evidence.rebase.helper").unwrap(),
        )
        .unwrap(),
    )
    .unwrap();
    std::fs::write(&fixture.patch, preview["patch"]["source"].as_str().unwrap()).unwrap();
    fixture
}

fn assert_capsule(fixture: &Fixture, patch_schema: &str, kind: &str, schema: &str) -> String {
    let inventory = fixture.inventory();
    let capsule = patch_evidence::generate(&fixture.source, &fixture.patch).unwrap();
    assert_eq!(
        patch_evidence::generate(&fixture.source, &fixture.patch).unwrap(),
        capsule
    );
    assert!(capsule.ends_with('\n'));
    assert!(!capsule[..capsule.len() - 1].contains('\n'));
    assert_eq!(fixture.inventory(), inventory);
    let value: serde_json::Value = serde_json::from_str(&capsule).unwrap();
    assert_eq!(value["schema"], "semaprax.semantic-patch-evidence.v1");
    assert_eq!(value["patch"]["schema"], patch_schema);
    assert_eq!(value["supporting_evidence"]["id"], "evidence:0");
    assert_eq!(value["supporting_evidence"]["kind"], kind);
    assert_eq!(value["supporting_evidence"]["schema"], schema);
    assert_eq!(value["budget"]["used_evidence_bytes"], capsule.len());
    assert_eq!(
        value["budget"]["used_source_bytes"],
        std::fs::metadata(&fixture.source).unwrap().len()
    );
    assert_eq!(
        value["budget"]["used_patch_bytes"],
        std::fs::metadata(&fixture.patch).unwrap().len()
    );
    assert_eq!(
        value["nonclaims"],
        serde_json::json!([
            "not_signature_or_authenticated_provenance",
            "not_human_approval_or_policy",
            "not_safe_compatible_or_target_verified",
            "no_commit_authority",
            "no_reusable_authorization_token",
            "no_test_or_target_execution",
            "no_agent_context_or_repository_analysis",
            "no_multi_file_transaction",
            "no_general_proof_system",
            "no_semantic_impact_v3",
            "no_persistence_or_incrementality",
            "no_external_consumer_compatibility",
            "no_new_patch_repair_graph_cleanup_or_runtime_semantics"
        ])
    );
    capsule
}

#[test]
fn capsule_v1_v2_v3_whole_document_kats_and_replay() {
    let cases = [
        (
            v1_fixture("kat-v1"),
            "semaprax.semantic-patch.v1",
            "semantic_impact_v1",
            "semaprax.semantic-impact.v1",
            "03befad24157620b56138e84d4495b1973d141275ee728493d5fbe4f0f6f09aa",
        ),
        (
            v2_fixture("kat-v2"),
            "semaprax.semantic-patch.v2",
            "semantic_impact_v1",
            "semaprax.semantic-impact.v1",
            "23742f9b8a323003237106d7a800cc8fb98f53a68bd72f5e0961cf47c63f7bba",
        ),
        (
            v3_fixture("kat-v3"),
            "semaprax.semantic-patch.v3",
            "identity_rebase_v1",
            "semaprax.identity-rebase.v1",
            "d682e08b125451af3ed49dce03a0814e83ca5e665224fc3bc7ab7b314827f62c",
        ),
    ];
    for (fixture, patch_schema, kind, schema, expected_sha) in cases {
        let capsule = assert_capsule(&fixture, patch_schema, kind, schema);
        assert_eq!(sha256(&capsule), expected_sha, "{patch_schema}");
        std::fs::write(&fixture.evidence, &capsule).unwrap();
        let receipt =
            patch_evidence::verify(&fixture.source, &fixture.patch, &fixture.evidence).unwrap();
        assert_eq!(
            patch_evidence::verify(&fixture.source, &fixture.patch, &fixture.evidence).unwrap(),
            receipt
        );
        let receipt_value: serde_json::Value = serde_json::from_str(&receipt).unwrap();
        let receipt_offsets = [
            "\"schema\":",
            "\"result\":",
            "\"source_graph_schema\":",
            "\"base_revision\":",
            "\"candidate_revision\":",
            "\"source\":",
            "\"patch\":",
            "\"patch_evidence\":",
            "\"review\":",
            "\"assessments\":",
            "\"supporting_evidence\":",
            "\"limits\":",
            "\"budget\":",
            "\"nonclaims\":",
        ]
        .map(|key| receipt.find(key).unwrap());
        assert!(receipt_offsets.windows(2).all(|pair| pair[0] < pair[1]));
        assert_eq!(
            receipt_value["schema"],
            "semaprax.semantic-patch-evidence-verification.v1"
        );
        assert_eq!(receipt_value["result"], "exact_replay");
        assert_eq!(
            receipt_value["patch_evidence"]["digest"],
            artifact_digest(&capsule)
        );
        assert_eq!(
            receipt_value["budget"]["used_evidence_bytes"],
            capsule.len()
        );
        assert_eq!(receipt_value["budget"]["used_receipt_bytes"], receipt.len());
        let capsule_value: serde_json::Value = serde_json::from_str(&capsule).unwrap();
        assert_eq!(receipt_value["assessments"], capsule_value["assessments"]);
        assert_eq!(
            receipt_value["supporting_evidence"],
            capsule_value["supporting_evidence"]
        );
        let expected_receipt_sha = match patch_schema {
            "semaprax.semantic-patch.v1" => {
                "1f2733743aaf2f9d2b9ad6bf2709a6867f169f596be01a9d53e92daecb8730a1"
            }
            "semaprax.semantic-patch.v2" => {
                "6d8b13b3f54277e66a1ee501e1e71d6fe959a2ebcdbaa158a7ece20dde054e48"
            }
            "semaprax.semantic-patch.v3" => {
                "13a99674a4c014d9f7f315d8108c3e5c870dcac2c5950ff3035ca1a1c155361b"
            }
            _ => unreachable!(),
        };
        assert_eq!(
            sha256(&receipt),
            expected_receipt_sha,
            "{patch_schema} receipt"
        );
    }
}

#[test]
fn exact_key_order_assessments_and_digest_layers_are_frozen() {
    let fixture = v1_fixture("wire");
    let capsule = patch_evidence::generate(&fixture.source, &fixture.patch).unwrap();
    let ordered = [
        "\"schema\":",
        "\"source_graph_schema\":",
        "\"base_revision\":",
        "\"candidate_revision\":",
        "\"source\":",
        "\"patch\":",
        "\"review\":",
        "\"assessments\":",
        "\"supporting_evidence\":",
        "\"limits\":",
        "\"budget\":",
        "\"nonclaims\":",
    ]
    .map(|key| capsule.find(key).unwrap());
    assert!(ordered.windows(2).all(|pair| pair[0] < pair[1]));
    let value: serde_json::Value = serde_json::from_str(&capsule).unwrap();
    assert_eq!(
        value["assessments"]
            .as_object()
            .unwrap()
            .keys()
            .map(String::as_str)
            .collect::<BTreeSet<_>>(),
        [
            "behavior",
            "api_identity",
            "security_authority",
            "memory_ownership",
            "target_artifact",
            "migration",
            "unsafe"
        ]
        .into_iter()
        .collect::<BTreeSet<_>>()
    );
    let review = semaprax::review::preview(&fixture.source, &fixture.patch).unwrap();
    let review_value: serde_json::Value = serde_json::from_str(&review).unwrap();
    for key in [
        "behavior",
        "api_identity",
        "security_authority",
        "memory_ownership",
        "target_artifact",
        "migration",
        "unsafe",
    ] {
        assert_eq!(
            value["assessments"][key],
            review_value["sections"][key]["assessment"]
        );
    }
    assert_eq!(
        value["supporting_evidence"]["kind"],
        review_value["evidence"]["kind"]
    );
    assert_eq!(
        value["supporting_evidence"]["schema"],
        review_value["evidence"]["schema"]
    );
    assert_eq!(
        value["supporting_evidence"]["digest"],
        review_value["evidence"]["digest"]
    );
    let mut hasher = Sha256::new();
    hasher.update(b"semaprax.semantic-patch-evidence.review-digest.v1\0");
    hasher.update((review.len() as u64).to_le_bytes());
    hasher.update(review.as_bytes());
    assert_eq!(
        value["review"]["digest"],
        format!(
            "sha256:{:x}",
            semaprax::digest_hex::LowerHex(hasher.finalize())
        )
    );
}

#[test]
fn graph_v10_through_v14_are_preserved_without_new_graph_semantics() {
    let cases = [
        (
            "module evidence.schema_v10;\n@id(\"schema.target\") fn target()->i64{1}\n@id(\"app.main\") fn main()->i64{target()}\n",
            "schema.target",
            "renamed_v10",
            "semaprax.graph.v10",
        ),
        (
            "module evidence.schema_v11;\n@id(\"schema.target\") fn target(input:Option<i64>)->Option<bool>{let checked=input?;Option<bool>::Some { value: checked>0 }}\n@id(\"app.main\") fn main()->i64{0}\n",
            "schema.target",
            "renamed_v11",
            "semaprax.graph.v11",
        ),
        (
            include_str!("../../platform-tests/component-runtime/v7.spx"),
            "component.transform-i64-bool",
            "renamed_v12",
            "semaprax.graph.v12",
        ),
        (
            include_str!("../../platform-tests/component-runtime/v8.spx"),
            "component.pattern.preserve-phantom-i64",
            "renamed_v13",
            "semaprax.graph.v13",
        ),
        (
            "module evidence.schema_v14;\n@id(\"schema.target\") fn target<T>()->bool{true}\n@id(\"app.main\") fn main()->i64{if target<i64>(){1}else{0}}\n",
            "schema.target",
            "renamed_v14",
            "semaprax.graph.v14",
        ),
    ];
    for (index, (source, target, renamed, schema)) in cases.into_iter().enumerate() {
        let patch = format!("base {}\nrename {target} to {renamed}\n", revision(source));
        let fixture = Fixture::new(&format!("schema-{index}"), source, &patch);
        let capsule = patch_evidence::generate(&fixture.source, &fixture.patch).unwrap();
        let value: serde_json::Value = serde_json::from_str(&capsule).unwrap();
        assert_eq!(value["source_graph_schema"], schema);
        std::fs::write(&fixture.evidence, capsule).unwrap();
        assert!(patch_evidence::verify(&fixture.source, &fixture.patch, &fixture.evidence).is_ok());
    }
}

#[test]
fn cli_is_fixed_arity_and_prints_exact_api_bytes() {
    let fixture = v1_fixture("cli");
    let capsule = patch_evidence::generate(&fixture.source, &fixture.patch).unwrap();
    let binary = env!("CARGO_BIN_EXE_semaprax");
    let generated = Command::new(binary)
        .arg("patch-evidence")
        .arg(&fixture.source)
        .arg(&fixture.patch)
        .output()
        .unwrap();
    assert!(generated.status.success());
    assert_eq!(generated.stdout, capsule.as_bytes());
    std::fs::write(&fixture.evidence, &capsule).unwrap();
    let receipt =
        patch_evidence::verify(&fixture.source, &fixture.patch, &fixture.evidence).unwrap();
    let verified = Command::new(binary)
        .arg("verify-patch-evidence")
        .arg(&fixture.source)
        .arg(&fixture.patch)
        .arg(&fixture.evidence)
        .output()
        .unwrap();
    assert!(verified.status.success());
    assert_eq!(verified.stdout, receipt.as_bytes());
    let rejected = Command::new(binary)
        .arg("patch-evidence")
        .arg(&fixture.source)
        .arg(&fixture.patch)
        .arg("--extra")
        .output()
        .unwrap();
    assert_eq!(rejected.status.code(), Some(2));
}

#[test]
fn verifier_rejects_noncanonical_and_deep_json_before_source_semantics() {
    let fixture = Fixture::new("syntax", "not source", "not patch");
    let hostile = [
        "{}",
        "{}\n\n",
        "\u{feff}{}\n",
        "{}\r\n",
        " { }\n",
        "{\"schema\":\"semaprax.semantic-patch-evidence-verification.v1\"}\n",
        "[[[[[[[[[0]]]]]]]]]\n",
        "{\"a\":{\"b\":1},\"a\":{\"b\":2}}\n",
        "{]\n",
    ];
    for evidence in hostile {
        std::fs::write(&fixture.evidence, evidence).unwrap();
        let error =
            patch_evidence::verify(&fixture.source, &fixture.patch, &fixture.evidence).unwrap_err();
        assert_eq!(error[0].code, "SPX-G130", "{evidence:?}");
    }
    std::fs::write(&fixture.evidence, [0xff, b'\n']).unwrap();
    let error =
        patch_evidence::verify(&fixture.source, &fixture.patch, &fixture.evidence).unwrap_err();
    assert_eq!(error[0].code, "SPX-G130");
}

#[test]
fn canonical_but_changed_capsule_is_not_a_reusable_authorization() {
    let fixture = v1_fixture("mismatch");
    let capsule = patch_evidence::generate(&fixture.source, &fixture.patch).unwrap();
    let value: serde_json::Value = serde_json::from_str(&capsule).unwrap();
    let digest = value["review"]["digest"].as_str().unwrap();
    let replacement = if digest.ends_with('0') { '1' } else { '0' };
    let mut changed_digest = digest.to_owned();
    changed_digest.pop();
    changed_digest.push(replacement);
    let changed = capsule.replacen(digest, &changed_digest, 1);
    assert_eq!(changed.len(), capsule.len());
    std::fs::write(&fixture.evidence, changed).unwrap();
    let inventory = fixture.inventory();
    let error =
        patch_evidence::verify(&fixture.source, &fixture.patch, &fixture.evidence).unwrap_err();
    assert_eq!(error[0].code, "SPX-G132");
    assert_eq!(fixture.inventory(), inventory);
}

#[test]
fn every_capsule_layer_is_closed_and_exact_replay_bound() {
    let fixture = v1_fixture("closed-envelope");
    let capsule = patch_evidence::generate(&fixture.source, &fixture.patch).unwrap();
    let value: serde_json::Value = serde_json::from_str(&capsule).unwrap();
    let mut mutations = Vec::new();
    let mut add = |needle: String, replacement: String, expected| {
        mutations.push((
            replace_and_reaccount(&capsule, &needle, &replacement),
            expected,
        ));
    };

    add(
        format!(
            "\"schema\":{}",
            serde_json::to_string(value["schema"].as_str().unwrap()).unwrap()
        ),
        "\"schema\":\"semaprax.semantic-patch-evidence.v2\"".to_owned(),
        "SPX-G130",
    );
    add(
        "\"source_graph_schema\":\"semaprax.graph.v10\"".to_owned(),
        "\"source_graph_schema\":\"semaprax.graph.v99\"".to_owned(),
        "SPX-G130",
    );
    for (path, key) in [
        ("base_revision", "base_revision"),
        ("candidate_revision", "candidate_revision"),
    ] {
        let digest = value[path].as_str().unwrap();
        add(
            format!("\"{key}\":\"{digest}\""),
            format!("\"{key}\":\"{}\"", changed_digest(digest)),
            "SPX-G132",
        );
    }
    for (object, prefix) in [
        ("source", "\"source\":{\"digest\":".to_owned()),
        (
            "patch",
            "\"patch\":{\"schema\":\"semaprax.semantic-patch.v1\",\"digest\":".to_owned(),
        ),
        (
            "review",
            "\"review\":{\"schema\":\"semaprax.semantic-review.v1\",\"digest\":".to_owned(),
        ),
        (
            "supporting_evidence",
            "\"supporting_evidence\":{\"id\":\"evidence:0\",\"kind\":\"semantic_impact_v1\",\"schema\":\"semaprax.semantic-impact.v1\",\"digest\":".to_owned(),
        ),
    ] {
        let digest = value[object]["digest"].as_str().unwrap();
        add(
            format!("{prefix}\"{digest}\""),
            format!("{prefix}\"{}\"", changed_digest(digest)),
            "SPX-G132",
        );
    }
    add(
        "\"patch\":{\"schema\":\"semaprax.semantic-patch.v1\"".to_owned(),
        "\"patch\":{\"schema\":\"semaprax.semantic-patch.v2\"".to_owned(),
        "SPX-G132",
    );
    add(
        "\"review\":{\"schema\":\"semaprax.semantic-review.v1\"".to_owned(),
        "\"review\":{\"schema\":\"semaprax.semantic-review.v2\"".to_owned(),
        "SPX-G130",
    );
    for key in [
        "behavior",
        "api_identity",
        "security_authority",
        "memory_ownership",
        "target_artifact",
        "migration",
        "unsafe",
    ] {
        let actual = value["assessments"][key].as_str().unwrap();
        let replacement = if actual == "unknown" {
            "mixed"
        } else {
            "unknown"
        };
        add(
            format!("\"{key}\":\"{actual}\""),
            format!("\"{key}\":\"{replacement}\""),
            "SPX-G132",
        );
    }
    add(
        "\"supporting_evidence\":{\"id\":\"evidence:0\"".to_owned(),
        "\"supporting_evidence\":{\"id\":\"evidence:1\"".to_owned(),
        "SPX-G130",
    );
    add(
        "\"kind\":\"semantic_impact_v1\"".to_owned(),
        "\"kind\":\"identity_rebase_v1\"".to_owned(),
        "SPX-G130",
    );
    add(
        "\"schema\":\"semaprax.semantic-impact.v1\"".to_owned(),
        "\"schema\":\"semaprax.semantic-impact.v2\"".to_owned(),
        "SPX-G130",
    );
    for key in [
        "max_source_bytes",
        "max_patch_bytes",
        "max_evidence_bytes",
        "max_operations",
        "max_declarations",
        "max_callables",
        "max_call_sites",
        "max_impact_depth",
        "max_impact_nodes",
        "max_impact_bytes",
        "max_review_bytes",
        "max_receipt_bytes",
    ] {
        let actual = value["limits"][key].as_u64().unwrap();
        add(
            format!("\"{key}\":{actual}"),
            format!("\"{key}\":{}", actual + 1),
            "SPX-G130",
        );
    }
    for key in [
        "used_source_bytes",
        "used_patch_bytes",
        "used_operations",
        "used_declarations",
        "used_callables",
        "used_call_sites",
        "used_impact_depth",
        "used_impact_nodes",
        "used_impact_bytes",
        "used_review_bytes",
    ] {
        let actual = value["budget"][key].as_u64().unwrap();
        add(
            format!("\"{key}\":{actual}"),
            format!("\"{key}\":{}", actual + 1),
            "SPX-G132",
        );
    }
    add(
        "not_signature_or_authenticated_provenance".to_owned(),
        "not_signature_or_authenticated_provenancf".to_owned(),
        "SPX-G130",
    );

    for (index, (mutation, expected)) in mutations.into_iter().enumerate() {
        std::fs::write(&fixture.evidence, mutation).unwrap();
        let error =
            patch_evidence::verify(&fixture.source, &fixture.patch, &fixture.evidence).unwrap_err();
        assert_eq!(error[0].code, expected, "mutation {index}");
    }

    let mut raw_hostiles = vec![
        capsule.replacen('{', "{ ", 1),
        capsule.replacen(
            "{\"schema\":",
            "{\"extra\":0,\"schema\":",
            1,
        ),
        capsule.replacen(
            "{\"schema\":",
            "{\"schema\":\"duplicate\",\"schema\":",
            1,
        ),
        capsule.replacen("\"source\":{\"digest\":", "\"source\":{\"extra\":0,\"digest\":", 1),
        capsule.replacen("\"source\":{\"digest\":", "\"source\":{\"digest\":\"sha256:0000000000000000000000000000000000000000000000000000000000000000\",\"digest\":", 1),
        capsule.replacen("\"source\":{\"digest\":", "\"source\":{\"removed\":", 1),
    ];
    let graph_schema = value["source_graph_schema"].as_str().unwrap();
    raw_hostiles.push(capsule.replacen("{\"schema\":", "{\"removed\":", 1));
    raw_hostiles.push(capsule.replacen(
        &format!(
            "{{\"schema\":\"semaprax.semantic-patch-evidence.v1\",\"source_graph_schema\":\"{graph_schema}\""
        ),
        &format!(
            "{{\"source_graph_schema\":\"{graph_schema}\",\"schema\":\"semaprax.semantic-patch-evidence.v1\""
        ),
        1,
    ));
    let patch_digest = value["patch"]["digest"].as_str().unwrap();
    raw_hostiles.push(capsule.replacen(
        &format!(
            "\"patch\":{{\"schema\":\"semaprax.semantic-patch.v1\",\"digest\":\"{patch_digest}\"}}"
        ),
        &format!(
            "\"patch\":{{\"digest\":\"{patch_digest}\",\"schema\":\"semaprax.semantic-patch.v1\"}}"
        ),
        1,
    ));
    for opening in [
        "\"source\":{",
        "\"patch\":{",
        "\"review\":{",
        "\"assessments\":{",
        "\"supporting_evidence\":{",
        "\"limits\":{",
        "\"budget\":{",
    ] {
        raw_hostiles.push(capsule.replacen(opening, &format!("{opening}\"extra\":0,"), 1));
        raw_hostiles.push(capsule.replacen(
            opening,
            &format!("{opening}\"duplicate\":0,\"duplicate\":1,"),
            1,
        ));
    }
    for (needle, replacement) in [
        ("\"source\":{\"digest\":", "\"source\":{\"removed\":"),
        ("\"patch\":{\"schema\":", "\"patch\":{\"removed\":"),
        ("\"review\":{\"schema\":", "\"review\":{\"removed\":"),
        (
            "\"assessments\":{\"behavior\":",
            "\"assessments\":{\"removed\":",
        ),
        (
            "\"supporting_evidence\":{\"id\":",
            "\"supporting_evidence\":{\"removed\":",
        ),
        (
            "\"limits\":{\"max_source_bytes\":",
            "\"limits\":{\"removed\":",
        ),
        (
            "\"budget\":{\"used_source_bytes\":",
            "\"budget\":{\"removed\":",
        ),
    ] {
        raw_hostiles.push(capsule.replacen(needle, replacement, 1));
    }
    let used_evidence = value["budget"]["used_evidence_bytes"].as_u64().unwrap();
    raw_hostiles.push(capsule.replacen(
        &format!("\"used_evidence_bytes\":{used_evidence}"),
        &format!("\"used_evidence_bytes\":{}", used_evidence + 1),
        1,
    ));
    for hostile in raw_hostiles {
        std::fs::write(&fixture.evidence, hostile).unwrap();
        let error =
            patch_evidence::verify(&fixture.source, &fixture.patch, &fixture.evidence).unwrap_err();
        assert_eq!(error[0].code, "SPX-G130");
    }

    std::fs::write(&fixture.evidence, &capsule).unwrap();
    let receipt =
        patch_evidence::verify(&fixture.source, &fixture.patch, &fixture.evidence).unwrap();
    std::fs::write(&fixture.evidence, receipt).unwrap();
    let error =
        patch_evidence::verify(&fixture.source, &fixture.patch, &fixture.evidence).unwrap_err();
    assert_eq!(error[0].code, "SPX-G130");
}

#[test]
fn capsules_cannot_be_substituted_across_source_patch_or_schema_family() {
    let first = v1_fixture("substitution-first");
    let first_capsule = patch_evidence::generate(&first.source, &first.patch).unwrap();
    let second_source = "module evidence.other;\n@id(\"evidence.other\") fn other()->i64{2}\n@id(\"app.main\") fn main()->i64{other()}\n";
    let second_patch = format!(
        "base {}\nrename evidence.other to renamed\n",
        revision(second_source)
    );
    let second = Fixture::new("substitution-second", second_source, &second_patch);
    let second_capsule = patch_evidence::generate(&second.source, &second.patch).unwrap();
    std::fs::write(&first.evidence, second_capsule).unwrap();
    let error = patch_evidence::verify(&first.source, &first.patch, &first.evidence).unwrap_err();
    assert_eq!(error[0].code, "SPX-G132");

    let v2 = v2_fixture("substitution-v2");
    let v2_capsule = patch_evidence::generate(&v2.source, &v2.patch).unwrap();
    std::fs::write(&first.evidence, v2_capsule).unwrap();
    let error = patch_evidence::verify(&first.source, &first.patch, &first.evidence).unwrap_err();
    assert_eq!(error[0].code, "SPX-G132");

    let parsed: serde_json::Value = serde_json::from_str(&first_capsule).unwrap();
    let old = parsed["source"]["digest"].as_str().unwrap();
    let other_digest = serde_json::from_str::<serde_json::Value>(
        &patch_evidence::generate(&second.source, &second.patch).unwrap(),
    )
    .unwrap()["source"]["digest"]
        .as_str()
        .unwrap()
        .to_owned();
    let rehashed = replace_and_reaccount(
        &first_capsule,
        &format!("\"source\":{{\"digest\":\"{old}\""),
        &format!("\"source\":{{\"digest\":\"{other_digest}\""),
    );
    std::fs::write(&first.evidence, rehashed).unwrap();
    let error = patch_evidence::verify(&first.source, &first.patch, &first.evidence).unwrap_err();
    assert_eq!(error[0].code, "SPX-G132");
}

#[test]
fn review_work_bounds_are_preserved_as_evidence_bounds() {
    let source = "module evidence.limit;\n@id(\"evidence.helper\") fn helper()->i64{missing()}\n@id(\"app.main\") fn main()->i64{helper()}\n";
    let mut patch = format!("base {}\n", revision(source));
    for _ in 0..=4096 {
        patch.push_str("rename evidence.helper to renamed\n");
    }
    let fixture = Fixture::new("operation-limit", source, &patch);
    let error = patch_evidence::generate(&fixture.source, &fixture.patch).unwrap_err();
    assert_eq!(error[0].code, "SPX-G131");

    let mut many = String::from("module evidence.callable_limit;\n");
    for index in 0..1025 {
        many.push_str(&format!(
            "@id(\"evidence.function{index}\") fn function{index}()->i64{{missing()}}\n"
        ));
    }
    let fixture = Fixture::new("callable-limit", &many, "base irrelevant\n");
    let error = patch_evidence::generate(&fixture.source, &fixture.patch).unwrap_err();
    assert_eq!(error[0].code, "SPX-G131");

    let source = "module evidence.operation_exact;\n@id(\"evidence.helper\") fn helper()->i64{1}\n@id(\"app.main\") fn main()->i64{helper()}\n";
    let mut patch = format!("base {}\n", revision(source));
    for _ in 0..4096 {
        patch.push_str("rename evidence.helper to renamed\n");
    }
    let fixture = Fixture::new("operation-exact", source, &patch);
    let capsule = patch_evidence::generate(&fixture.source, &fixture.patch).unwrap();
    let value: serde_json::Value = serde_json::from_str(&capsule).unwrap();
    assert_eq!(value["budget"]["used_operations"], 4096);

    let mut exact_callables = String::from("module evidence.callable_exact;\n");
    for index in 0..1023 {
        exact_callables.push_str(&format!(
            "@id(\"evidence.function{index}\") fn function{index}()->i64{{{index}}}\n"
        ));
    }
    exact_callables.push_str("@id(\"app.main\") fn main()->i64{function0()}\n");
    let patch = format!(
        "base {}\nrename evidence.function0 to renamed\n",
        revision(&exact_callables)
    );
    let fixture = Fixture::new("callable-exact", &exact_callables, &patch);
    let capsule = patch_evidence::generate(&fixture.source, &fixture.patch).unwrap();
    let value: serde_json::Value = serde_json::from_str(&capsule).unwrap();
    assert_eq!(value["budget"]["used_callables"], 1024);
}

#[test]
fn source_patch_and_evidence_byte_boundaries_are_exact() {
    const MAX_SOURCE: usize = 16 * 1024 * 1024;
    const MAX_PATCH: usize = 4 * 1024 * 1024;
    const MAX_EVIDENCE: usize = 65_536;

    let base_source = "module evidence.source_bytes;\n@id(\"evidence.helper\") fn helper()->i64{1}\n@id(\"app.main\") fn main()->i64{helper()}\n";
    let mut exact_source = base_source.to_owned();
    exact_source.push_str(&" ".repeat(MAX_SOURCE - exact_source.len()));
    let patch = format!(
        "base {}\nrename evidence.helper to renamed\n",
        revision(&exact_source)
    );
    let exact = Fixture::new("source-exact", &exact_source, &patch);
    let capsule = patch_evidence::generate(&exact.source, &exact.patch).unwrap();
    let value: serde_json::Value = serde_json::from_str(&capsule).unwrap();
    assert_eq!(value["budget"]["used_source_bytes"], MAX_SOURCE);

    let over = Fixture::new("source-over", base_source, &patch);
    std::fs::OpenOptions::new()
        .write(true)
        .open(&over.source)
        .unwrap()
        .set_len((MAX_SOURCE + 1) as u64)
        .unwrap();
    let error = patch_evidence::generate(&over.source, &over.patch).unwrap_err();
    assert_eq!(error[0].code, "SPX-G131");

    let source = "module evidence.patch_bytes;\n@id(\"evidence.helper\") fn helper()->i64{1}\n@id(\"app.main\") fn main()->i64{helper()}\n";
    let mut exact_patch = format!(
        "base {}\nrename evidence.helper to renamed\n#",
        revision(source)
    );
    exact_patch.push_str(&"x".repeat(MAX_PATCH - exact_patch.len()));
    let exact = Fixture::new("patch-exact", source, &exact_patch);
    let capsule = patch_evidence::generate(&exact.source, &exact.patch).unwrap();
    let value: serde_json::Value = serde_json::from_str(&capsule).unwrap();
    assert_eq!(value["budget"]["used_patch_bytes"], MAX_PATCH);

    let over = Fixture::new("patch-over", source, "");
    std::fs::OpenOptions::new()
        .write(true)
        .open(&over.patch)
        .unwrap()
        .set_len((MAX_PATCH + 1) as u64)
        .unwrap();
    let error = patch_evidence::generate(&over.source, &over.patch).unwrap_err();
    assert_eq!(error[0].code, "SPX-G131");

    let ordinary = v1_fixture("evidence-boundary");
    std::fs::write(&ordinary.evidence, vec![b' '; MAX_EVIDENCE]).unwrap();
    let error =
        patch_evidence::verify(&ordinary.source, &ordinary.patch, &ordinary.evidence).unwrap_err();
    assert_eq!(error[0].code, "SPX-G130");
    std::fs::write(&ordinary.evidence, vec![b' '; MAX_EVIDENCE + 1]).unwrap();
    let error =
        patch_evidence::verify(&ordinary.source, &ordinary.patch, &ordinary.evidence).unwrap_err();
    assert_eq!(error[0].code, "SPX-G131");
}

#[test]
fn declaration_and_call_site_boundaries_are_checked_before_hir() {
    let mut exact_declarations = String::from(
        "module evidence.declaration_exact;\n@id(\"evidence.large\") record Large {\n",
    );
    for index in 0..4093 {
        exact_declarations.push_str(&format!(
            "@id(\"evidence.large.field{index}\") field{index}: i64,\n"
        ));
    }
    exact_declarations.push_str(
        "}\n@id(\"evidence.target\") fn target()->i64{1}\n@id(\"app.main\") fn main()->i64{target()}\n",
    );
    let patch = format!(
        "base {}\nrename evidence.target to renamed\n",
        revision(&exact_declarations)
    );
    let exact = Fixture::new("declarations-exact", &exact_declarations, &patch);
    let capsule = patch_evidence::generate(&exact.source, &exact.patch).unwrap();
    let value: serde_json::Value = serde_json::from_str(&capsule).unwrap();
    assert_eq!(value["budget"]["used_declarations"], 4096);

    let mut over_declarations =
        String::from("module evidence.declaration_over;\n@id(\"evidence.large\") record Large {\n");
    for index in 0..4094 {
        over_declarations.push_str(&format!(
            "@id(\"evidence.large.field{index}\") field{index}: i64,\n"
        ));
    }
    over_declarations.push_str(
        "}\n@id(\"evidence.target\") fn target()->i64{missing()}\n@id(\"app.main\") fn main()->i64{missing()}\n",
    );
    let over = Fixture::new("declarations-over", &over_declarations, "base irrelevant\n");
    let error = patch_evidence::generate(&over.source, &over.patch).unwrap_err();
    assert_eq!(error[0].code, "SPX-G131");

    let mut over_calls =
        String::from("module evidence.calls_over;\n@id(\"app.main\") fn main()->i64{\n");
    for index in 0..65_537 {
        over_calls.push_str(&format!("let value{index}=missing();\n"));
    }
    over_calls.push_str("0}\n");
    let over = Fixture::new("calls-over", &over_calls, "base irrelevant\n");
    let error = patch_evidence::generate(&over.source, &over.patch).unwrap_err();
    assert_eq!(error[0].code, "SPX-G131");
}

#[test]
fn evidence_apply_matches_a0_for_patch_v1_v2_and_v3_then_rejects_replay() {
    let evidence_fixtures = [
        v1_fixture("apply-v1-evidence"),
        v2_fixture("apply-v2-evidence"),
        v3_fixture("apply-v3-evidence"),
    ];
    for (index, evidence_fixture) in evidence_fixtures.into_iter().enumerate() {
        let source = std::fs::read_to_string(&evidence_fixture.source).unwrap();
        let patch_source = std::fs::read_to_string(&evidence_fixture.patch).unwrap();
        let plain_fixture = Fixture::new(&format!("apply-plain-{index}"), &source, &patch_source);
        let capsule =
            patch_evidence::generate(&evidence_fixture.source, &evidence_fixture.patch).unwrap();
        std::fs::write(&evidence_fixture.evidence, capsule).unwrap();
        let evidence_revision = patch_evidence::apply(
            &evidence_fixture.source,
            &evidence_fixture.patch,
            &evidence_fixture.evidence,
        )
        .unwrap();
        let plain_revision = patch::apply(&plain_fixture.source, &plain_fixture.patch).unwrap();
        assert_eq!(evidence_revision, plain_revision);
        assert_eq!(
            std::fs::read(&evidence_fixture.source).unwrap(),
            std::fs::read(&plain_fixture.source).unwrap()
        );
        let committed = std::fs::read(&evidence_fixture.source).unwrap();
        let error = patch_evidence::apply(
            &evidence_fixture.source,
            &evidence_fixture.patch,
            &evidence_fixture.evidence,
        )
        .unwrap_err();
        assert_eq!(error[0].code, "SPX-G409");
        assert_eq!(std::fs::read(&evidence_fixture.source).unwrap(), committed);
        evidence_fixture.assert_no_a0_artifacts();
        plain_fixture.assert_no_a0_artifacts();
    }
}

#[test]
fn mismatch_and_receipt_confusion_fail_before_staging() {
    let fixture = v1_fixture("apply-mismatch");
    let source = std::fs::read(&fixture.source).unwrap();
    let capsule = patch_evidence::generate(&fixture.source, &fixture.patch).unwrap();
    let parsed: serde_json::Value = serde_json::from_str(&capsule).unwrap();
    let digest = parsed["review"]["digest"].as_str().unwrap();
    let changed = replace_and_reaccount(
        &capsule,
        &format!("\"review\":{{\"schema\":\"semaprax.semantic-review.v1\",\"digest\":\"{digest}\""),
        &format!(
            "\"review\":{{\"schema\":\"semaprax.semantic-review.v1\",\"digest\":\"{}\"",
            changed_digest(digest)
        ),
    );
    std::fs::write(&fixture.evidence, changed).unwrap();
    let inventory = fixture.inventory();
    let error =
        patch_evidence::apply(&fixture.source, &fixture.patch, &fixture.evidence).unwrap_err();
    assert_eq!(error[0].code, "SPX-G132");
    assert_eq!(fixture.inventory(), inventory);
    assert_eq!(std::fs::read(&fixture.source).unwrap(), source);
    fixture.assert_no_a0_artifacts();

    std::fs::write(&fixture.evidence, &capsule).unwrap();
    let receipt =
        patch_evidence::verify(&fixture.source, &fixture.patch, &fixture.evidence).unwrap();
    std::fs::write(&fixture.evidence, receipt).unwrap();
    let inventory = fixture.inventory();
    let error =
        patch_evidence::apply(&fixture.source, &fixture.patch, &fixture.evidence).unwrap_err();
    assert_eq!(error[0].code, "SPX-G130");
    assert_eq!(fixture.inventory(), inventory);
    assert_eq!(std::fs::read(&fixture.source).unwrap(), source);
    fixture.assert_no_a0_artifacts();
}

#[test]
fn a0_lock_is_acquired_before_patch_or_evidence_authority_work() {
    let fixture = Fixture::new("apply-lock-first", "not source", "not patch");
    let lock = fixture.directory.join(format!(
        ".{}.semaprax-patch.lock",
        fixture.source.file_name().unwrap().to_string_lossy()
    ));
    std::fs::write(&lock, "held").unwrap();
    let error =
        patch_evidence::apply(&fixture.source, &fixture.patch, &fixture.evidence).unwrap_err();
    assert_eq!(error[0].code, "SPX-I205");
    assert_eq!(std::fs::read_to_string(&lock).unwrap(), "held");
    std::fs::remove_file(lock).unwrap();
    fixture.assert_no_a0_artifacts();
}

#[test]
fn patch_with_evidence_cli_has_exact_arity_and_success_output() {
    let fixture = v1_fixture("apply-cli");
    let capsule = patch_evidence::generate(&fixture.source, &fixture.patch).unwrap();
    let candidate = serde_json::from_str::<serde_json::Value>(&capsule).unwrap()
        ["candidate_revision"]
        .as_str()
        .unwrap()
        .to_owned();
    std::fs::write(&fixture.evidence, capsule).unwrap();
    let binary = env!("CARGO_BIN_EXE_semaprax");
    let output = Command::new(binary)
        .arg("patch-with-evidence")
        .arg(&fixture.source)
        .arg(&fixture.patch)
        .arg(&fixture.evidence)
        .output()
        .unwrap();
    assert!(output.status.success());
    assert_eq!(
        output.stdout,
        format!("applied semantic patch with exact evidence replay; graph is now {candidate}\n")
            .as_bytes()
    );
    fixture.assert_no_a0_artifacts();

    let rejected = Command::new(binary)
        .arg("patch-with-evidence")
        .arg(&fixture.source)
        .arg(&fixture.patch)
        .arg(&fixture.evidence)
        .arg("--extra")
        .output()
        .unwrap();
    assert_eq!(rejected.status.code(), Some(2));
}

#[cfg(unix)]
#[test]
fn evidence_apply_preserves_source_permissions() {
    use std::os::unix::fs::PermissionsExt;

    let fixture = v1_fixture("apply-permissions");
    let mut permissions = std::fs::metadata(&fixture.source).unwrap().permissions();
    permissions.set_mode(0o640);
    std::fs::set_permissions(&fixture.source, permissions).unwrap();
    let capsule = patch_evidence::generate(&fixture.source, &fixture.patch).unwrap();
    std::fs::write(&fixture.evidence, capsule).unwrap();
    patch_evidence::apply(&fixture.source, &fixture.patch, &fixture.evidence).unwrap();
    assert_eq!(
        std::fs::metadata(&fixture.source)
            .unwrap()
            .permissions()
            .mode()
            & 0o777,
        0o640
    );
    fixture.assert_no_a0_artifacts();
}
