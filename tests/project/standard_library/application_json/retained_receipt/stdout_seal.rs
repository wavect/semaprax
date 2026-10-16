//! Full CLI composition: seal exact bytes only after the ordinary receiver succeeds.
use super::*;

fn invoke(
    root: &std::path::Path,
    output: &str,
    record: &str,
    extra: &[&str],
) -> std::process::Output {
    Command::new(env!("CARGO_BIN_EXE_semaprax"))
        .current_dir(root)
        .args([
            "json-codec",
            ".",
            "--source",
            "src/schema.spx",
            "--type",
            record,
            "--output",
            output,
        ])
        .args(extra)
        .output()
        .unwrap()
}

#[test]
fn cli_seal_matches_exact_legacy_receipt_bytes_and_no_overwrite_is_silent() {
    let root = fixture("codec-stdout-seal", SCHEMA);
    let source = std::fs::read(root.join("src/schema.spx")).unwrap();
    let old = invoke(
        &root,
        "old.json",
        "application.patient",
        &["--retained-receipt"],
    );
    assert!(
        old.status.success(),
        "{}",
        String::from_utf8_lossy(&old.stderr)
    );
    assert!(old.stdout.is_empty());
    let bytes = std::fs::read(root.join("old.json")).unwrap();
    for (output, nonce) in [("sealed-a.json", "runner-1"), ("sealed-b.json", "runner-2")] {
        let result = invoke(
            &root,
            output,
            "application.patient",
            &["--retained-receipt", "--receipt-stdout-seal", nonce],
        );
        assert!(
            result.status.success(),
            "{}",
            String::from_utf8_lossy(&result.stderr)
        );
        assert_eq!(std::fs::read(root.join(output)).unwrap(), bytes);
        assert_eq!(result.stdout.last(), Some(&b'\n'));
        assert_eq!(result.stdout.iter().filter(|b| **b == b'\n').count(), 1);
        assert!(result.stdout.len() <= 512);
        let seal: serde_json::Value = serde_json::from_slice(&result.stdout).unwrap();
        assert_eq!(
            seal,
            serde_json::json!({"schema": "semaprax.json-codec-publication-seal.v1",
            "nonce": nonce, "receipt_bytes": bytes.len(), "receipt_sha256": digest(&bytes)})
        );
        let refused = invoke(
            &root,
            output,
            "application.patient",
            &["--retained-receipt", "--receipt-stdout-seal", nonce],
        );
        assert!(!refused.status.success());
        assert!(refused.stdout.is_empty());
        assert_eq!(std::fs::read(root.join(output)).unwrap(), bytes);
    }
    let receipt: project::JsonCodecRetainedReceipt = serde_json::from_slice(&bytes).unwrap();
    assert!(!receipt.independent_execution);
    assert_eq!(receipt.compiler_binary_sha256, None);
    assert_eq!(std::fs::read(root.join("src/schema.spx")).unwrap(), source);
    assert!(std::fs::read_dir(&root).unwrap().all(|entry| !entry
        .unwrap()
        .file_name()
        .to_string_lossy()
        .contains(".json-codec-")));
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn malformed_or_failed_sealed_derivation_publishes_neither_artifact_nor_stdout() {
    let root = fixture("codec-stdout-seal-refusal", SCHEMA);
    let original = std::fs::read(root.join("src/schema.spx")).unwrap();
    for extra in [
        vec!["--receipt-stdout-seal", "n"],
        vec!["--retained-receipt", "--receipt-stdout-seal"],
        vec!["--retained-receipt", "--receipt-stdout-seal", ""],
        vec![
            "--retained-receipt",
            "--receipt-stdout-seal",
            "n",
            "--receipt-stdout-seal",
            "m",
        ],
        vec![
            "--retained-receipt",
            "--receipt-stdout-seal",
            "n",
            "--plan",
            "p.json",
        ],
        vec![
            "--retained-receipt",
            "--receipt-stdout-seal",
            "n",
            "--explain",
        ],
    ] {
        let result = invoke(&root, "absent.json", "application.patient", &extra);
        assert_eq!(result.status.code(), Some(2));
        assert!(result.stdout.is_empty());
        assert!(!root.join("absent.json").exists());
    }
    let ordinary_failure = invoke(
        &root,
        "absent.json",
        "missing.record",
        &["--retained-receipt"],
    );
    let sealed_failure = invoke(
        &root,
        "absent.json",
        "missing.record",
        &["--retained-receipt", "--receipt-stdout-seal", "n"],
    );
    assert!(!sealed_failure.status.success());
    assert_eq!(sealed_failure.status.code(), ordinary_failure.status.code());
    assert_eq!(sealed_failure.stderr, ordinary_failure.stderr);
    assert!(sealed_failure.stdout.is_empty());
    assert!(!root.join("absent.json").exists());
    assert_eq!(
        std::fs::read(root.join("src/schema.spx")).unwrap(),
        original
    );
    std::fs::remove_dir_all(root).unwrap();
}
