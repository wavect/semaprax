use super::*;
use std::sync::atomic::{AtomicU64, Ordering};
static NEXT: AtomicU64 = AtomicU64::new(0);
fn fixture() -> std::path::PathBuf {
    let root = std::env::temp_dir().join(format!(
        "codec-input-receipt-{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    ));
    std::fs::create_dir_all(root.join("src")).unwrap();
    std::fs::write(
        root.join("semaprax.toml"),
        include_str!("../../../../examples/calculator-project/semaprax.toml"),
    )
    .unwrap();
    for (path, text) in [
        (
            "app.spx",
            include_str!("../../../../examples/calculator-project/src/app.spx"),
        ),
        (
            "core.spx",
            include_str!("../../../../examples/calculator-project/src/core.spx"),
        ),
        (
            "tests.spx",
            include_str!("../../../../examples/calculator-project/src/tests.spx"),
        ),
    ] {
        std::fs::write(root.join("src").join(path), text).unwrap();
    }
    root
}
#[test]
fn retained_handles_preserve_entire_original_manifest_and_sources_without_path_reopening() {
    let root = fixture();
    let manifest = root.join("semaprax.toml");
    let original = std::fs::read_to_string(&manifest).unwrap();
    // A harmless noncanonical manifest comment is retained verbatim, never
    // reconstructed from its canonical parsed projection.
    std::fs::write(
        &manifest,
        format!("# original retained comment\n{original}"),
    )
    .unwrap();
    let inputs =
        crate::project::with_authenticated_project(&manifest, |snapshot| retain(snapshot)).unwrap();
    assert_eq!(inputs.len(), 4);
    for input in inputs {
        let exact = std::fs::read_to_string(root.join(&input.path)).unwrap();
        assert_eq!(input.source, exact);
        assert_eq!(input.bytes, exact.len());
        assert_eq!(input.sha256, digest(exact.as_bytes()));
    }
    std::fs::remove_dir_all(root).unwrap();
}
#[test]
fn retained_handle_bytes_and_identity_drift_refuse_instead_of_exporting_a_revision_claim() {
    for replace in [false, true] {
        let root = fixture();
        let manifest = root.join("semaprax.toml");
        let errors = crate::project::with_authenticated_project(&manifest, |snapshot| {
            let path = root.join("src/core.spx");
            let original = std::fs::read(&path).unwrap();
            if replace {
                std::fs::rename(&path, root.join("old-core.spx")).unwrap();
                std::fs::write(&path, &original).unwrap();
            } else {
                let mut changed = original;
                changed.push(b' ');
                std::fs::write(&path, changed).unwrap();
            }
            retain(snapshot)
        })
        .unwrap_err();
        assert!(errors.iter().any(|error| error.code == "SPX-J102"));
        std::fs::remove_dir_all(root).unwrap();
    }
}

fn mock_bundle() -> JsonCodecBundle {
    // Only a private workflow test double; never admitted as a Project receipt.
    JsonCodecBundle {
        schema: "test-only.invalid".into(),
        authored_project_revision: "test-revision".into(),
        candidate_project_revision: String::new(),
        candidate_workspace_revision: String::new(),
        manifest: String::new(),
        authored_sources: vec![],
        candidate_sources: vec![],
        entries: vec![],
    }
}
#[test]
fn second_derivation_failure_and_full_output_disagreement_produce_no_receipt() {
    let mut calls = 0;
    let selected = vec![
        Diagnostic::io("SPX-J180", "selected second derivation refusal")
            .with_help("retain every diagnostic field"),
    ];
    let result = repeat_bundle(|| {
        calls += 1;
        if calls == 1 {
            Ok(mock_bundle())
        } else {
            Err(selected.clone())
        }
    });
    assert_eq!(calls, 2);
    assert_eq!(result.unwrap_err(), selected);
    let mut calls = 0;
    let result = repeat_bundle(|| {
        calls += 1;
        let mut bundle = mock_bundle();
        if calls == 2 {
            bundle.manifest.push(' ');
        }
        Ok(bundle)
    });
    assert_eq!(calls, 2);
    assert_eq!(
        result.unwrap_err(),
        refusal("codec repeated derivation differs for the same retained revision")
    );
}
