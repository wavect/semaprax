//! Exact original inputs and repeated compiler output are separate inventories.
use super::*;
fn entries() -> Vec<project::JsonCodecPlanEntry> {
    vec![project::JsonCodecPlanEntry {
        source_path: "src/schema.spx".into(),
        record_id: "application.patient".into(),
        profile: project::JsonCodecProfile::FlatScalars,
    }]
}
fn digest(bytes: &[u8]) -> String {
    use std::fmt::Write;
    let mut encoded = String::with_capacity(7 + 64);
    encoded.push_str("sha256:");
    for byte in Sha256::digest(bytes) {
        write!(&mut encoded, "{byte:02x}").unwrap();
    }
    encoded
}
#[test]
fn retained_receipt_contains_complete_originals_resolved_closure_and_two_actual_equal_derivations()
{
    let root = fixture("codec-retained-receipt", SCHEMA);
    let manifest = root.join("semaprax.toml");
    let original = std::fs::read_to_string(&manifest).unwrap();
    let commented = format!("# retain this exact original comment\n{original}");
    // The original manifest-comment positive was inadmissible: the owning
    // Project grammar requires schema first and exact ordered assignments.
    // Keep both complete original hostile byte strings as six-field negatives.
    for malformed in [
        &commented,
        &format!("{commented}# changed raw input, same parsed manifest\n"),
    ] {
        let diagnostics = project::ProjectManifest::parse(malformed).unwrap_err();
        assert_eq!(diagnostics.len(), 1);
        let diagnostic = &diagnostics[0];
        assert_eq!(diagnostic.code, "SPX-J100");
        assert_eq!(diagnostic.severity, semaprax::diagnostic::Severity::Error);
        assert_eq!(
            diagnostic.message,
            "Project v1 manifest expected canonical `schema` string assignment"
        );
        assert_eq!(diagnostic.path, None);
        assert_eq!(diagnostic.span, None);
        assert_eq!(diagnostic.help, None);
    }
    let source_path = root.join("src/schema.spx");
    let source = std::fs::read_to_string(&source_path).unwrap();
    let source_original = format!("// retain this exact original source comment\n{source}");
    std::fs::write(&source_path, &source_original).unwrap();
    std::fs::write(&manifest, &original).unwrap();
    let receipt = project::with_authenticated_json_codec_receipt(&manifest, &entries()).unwrap();
    assert_eq!(receipt.schema, "semaprax.json-codec-retained-receipt.v1");
    assert_eq!(receipt.compiler_package_version, env!("CARGO_PKG_VERSION"));
    assert_eq!(receipt.compiler_binary_sha256, None);
    assert!(!receipt.independent_execution);
    assert_eq!(
        receipt.repeat_kind,
        "same-process.same-retained-revision.v1"
    );
    assert_eq!(receipt.held_inputs.len(), 4);
    assert_eq!(receipt.held_inputs[0].source, original);
    for input in &receipt.held_inputs {
        let exact = std::fs::read(root.join(&input.path)).unwrap();
        assert_eq!(input.source.as_bytes(), exact);
        assert_eq!(input.bytes, exact.len());
        assert_eq!(input.sha256, digest(&exact));
    }
    let expected = project::with_authenticated_project(&manifest, |snapshot| {
        project::derive_json_codec_plan(&snapshot.retain_revision(), &entries())
    })
    .unwrap();
    assert_eq!(receipt.bundle, expected);
    assert!(receipt
        .bundle
        .authored_sources
        .iter()
        .any(|source| source.path.starts_with("dependencies/std.data.json")));
    assert!(receipt
        .bundle
        .authored_sources
        .iter()
        .all(|source| source.sha256 == digest(source.source.as_bytes())));
    let bytes = serde_json::to_vec(&expected).unwrap();
    assert_eq!(receipt.derivations.len(), 2);
    for (index, repeat) in receipt.derivations.iter().enumerate() {
        assert_eq!(repeat.ordinal, index + 1);
        assert_eq!(repeat.bundle_sha256, digest(&bytes));
        assert_eq!(repeat.bundle_bytes, bytes.len());
        assert_eq!(
            repeat.project_revision,
            receipt.bundle.authored_project_revision
        );
    }
    project::verify_authenticated_json_codec_receipt(&manifest, &entries(), &receipt).unwrap();
    let encoded = serde_json::to_vec(&receipt).unwrap();
    assert_eq!(
        serde_json::from_slice::<project::JsonCodecRetainedReceipt>(&encoded).unwrap(),
        receipt
    );
    let mut unknown = serde_json::to_value(&receipt).unwrap();
    unknown
        .as_object_mut()
        .unwrap()
        .insert("authority".into(), serde_json::json!(true));
    assert!(serde_json::from_value::<project::JsonCodecRetainedReceipt>(unknown).is_err());
    // An admitted source-comment change keeps source meaning while changing
    // complete retained bytes. Manifest comments above remain grammar refusals.
    std::fs::write(
        &source_path,
        format!("{source_original}// changed raw input, same parsed source\n"),
    )
    .unwrap();
    assert_eq!(
        project::verify_authenticated_json_codec_receipt(&manifest, &entries(), &receipt)
            .unwrap_err()[0]
            .code,
        "SPX-J180"
    );
    std::fs::write(&source_path, &source_original).unwrap();
    std::fs::write(&manifest, &original).unwrap();
    assert_eq!(std::fs::read_to_string(&manifest).unwrap(), original);
    std::fs::remove_dir_all(root).unwrap();
}
#[test]
fn retained_receipt_replay_refuses_reminted_originals_outputs_profiles_repeat_and_binary_claims() {
    let root = fixture("codec-retained-forgery", SCHEMA);
    let manifest = root.join("semaprax.toml");
    let receipt = project::with_authenticated_json_codec_receipt(&manifest, &entries()).unwrap();
    for mutation in 0..10 {
        let mut changed = receipt.clone();
        match mutation {
            0 => {
                changed.held_inputs[0].source.push(' ');
                changed.held_inputs[0].bytes += 1;
                changed.held_inputs[0].sha256 = digest(changed.held_inputs[0].source.as_bytes());
            }
            1 => {
                changed.held_inputs.pop();
            }
            2 => {
                changed.held_inputs[1].path = "src/unrelated.spx".into();
            }
            3 => {
                changed.bundle.candidate_sources[0].source.push(' ');
                changed.bundle.candidate_sources[0].sha256 =
                    digest(changed.bundle.candidate_sources[0].source.as_bytes());
            }
            4 => {
                changed.bundle.entries[0].profile = "identifier-views.v1".into();
            }
            5 => {
                changed.derivations.pop();
            }
            6 => {
                changed.derivations[1].project_revision.push('0');
            }
            7 => {
                changed.independent_execution = true;
            }
            8 => {
                changed.compiler_binary_sha256 = Some(format!("sha256:{}", "0".repeat(64)));
            }
            9 => {
                changed.derivations[0].bundle_sha256 = "sha256:reminted".into();
            }
            _ => unreachable!(),
        }
        assert_eq!(
            project::verify_authenticated_json_codec_receipt(&manifest, &entries(), &changed)
                .unwrap_err()[0]
                .code,
            "SPX-J180",
            "mutation={mutation}"
        );
    }
    let mut invalid = entries();
    invalid[0].record_id = "missing.record".into();
    let expected = project::with_authenticated_project(&manifest, |snapshot| {
        project::derive_json_codec_plan(&snapshot.retain_revision(), &invalid)
    })
    .unwrap_err();
    assert_eq!(
        project::with_authenticated_json_codec_receipt(&manifest, &invalid)
            .unwrap_err()
            .iter()
            .map(|diagnostic| diagnostic.json())
            .collect::<Vec<_>>(),
        expected
            .iter()
            .map(|diagnostic| diagnostic.json())
            .collect::<Vec<_>>(),
        "complete original six-field ordinary refusal"
    );
    std::fs::remove_dir_all(root).unwrap();
}
#[test]
fn retained_receipt_cli_exclusively_publishes_one_artifact_and_preserves_old_source_mode() {
    let root = fixture("codec-retained-cli", SCHEMA);
    let args = [
        "json-codec",
        ".",
        "--source",
        "src/schema.spx",
        "--type",
        "application.patient",
        "--output",
        "receipt.json",
        "--retained-receipt",
    ];
    let result = Command::new(env!("CARGO_BIN_EXE_semaprax"))
        .current_dir(&root)
        .args(args)
        .output()
        .unwrap();
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    let bytes = std::fs::read(root.join("receipt.json")).unwrap();
    let receipt: project::JsonCodecRetainedReceipt = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(receipt.derivations.len(), 2);
    let refused = Command::new(env!("CARGO_BIN_EXE_semaprax"))
        .current_dir(&root)
        .args(args)
        .output()
        .unwrap();
    assert!(!refused.status.success());
    assert_eq!(std::fs::read(root.join("receipt.json")).unwrap(), bytes);
    let ordinary = Command::new(env!("CARGO_BIN_EXE_semaprax"))
        .current_dir(&root)
        .args([
            "json-codec",
            ".",
            "--source",
            "src/schema.spx",
            "--type",
            "application.patient",
            "--output",
            "ordinary.spx",
        ])
        .output()
        .unwrap();
    assert!(
        ordinary.status.success(),
        "{}",
        String::from_utf8_lossy(&ordinary.stderr)
    );
    let output = receipt
        .bundle
        .candidate_sources
        .iter()
        .find(|source| source.path == "src/schema.spx")
        .unwrap();
    assert_eq!(
        std::fs::read_to_string(root.join("ordinary.spx")).unwrap(),
        output.source
    );
    let mut bad = args;
    bad[5] = "missing.record";
    bad[7] = "absent.json";
    assert!(!Command::new(env!("CARGO_BIN_EXE_semaprax"))
        .current_dir(&root)
        .args(bad)
        .output()
        .unwrap()
        .status
        .success());
    assert!(!root.join("absent.json").exists());
    assert!(std::fs::read_dir(&root).unwrap().all(|entry| {
        !entry
            .unwrap()
            .file_name()
            .to_string_lossy()
            .starts_with(".receipt.json.json-codec-")
    }));
    std::fs::remove_dir_all(root).unwrap();
}

#[path = "retained_receipt/stdout_seal.rs"]
mod stdout_seal;
