//! Complete multi-codec candidates stay bound to one authored Project revision.
use super::*;

pub(super) fn paired_fixture(label: &str) -> std::path::PathBuf {
    let root = fixture(label, SCHEMA);
    let manifest = MANIFEST.replace(
        "\"src/schema.spx\",",
        "\"src/schema.spx\", \"src/second.spx\",",
    );
    std::fs::write(root.join("semaprax.toml"), manifest).unwrap();
    let second = SCHEMA
        .replace("consumer.schema", "consumer.second")
        .replace("application.patient", "application.other")
        .replace("Patient", "Other");
    std::fs::write(root.join("src/second.spx"), canonical(&second)).unwrap();
    root
}

fn entries() -> Vec<project::JsonCodecPlanEntry> {
    [
        ("src/schema.spx", "application.patient"),
        ("src/second.spx", "application.other"),
    ]
    .map(|(path, record)| project::JsonCodecPlanEntry {
        source_path: path.to_owned(),
        record_id: record.to_owned(),
        profile: project::JsonCodecProfile::FlatScalars,
    })
    .to_vec()
}

// Bundled dependency paths are semantic workspace identities, not files below
// the fixture root. Independently read the exact compiler-bundled file mapping
// (standard_dependencies.rs PACKAGES), before deriving any receipt.
pub(super) fn paired_originals(
    root: &std::path::Path,
) -> std::collections::BTreeMap<String, String> {
    let mut originals = std::collections::BTreeMap::new();
    for path in [
        "src/schema.spx",
        "src/second.spx",
        "src/app.spx",
        "src/tests.spx",
    ] {
        assert!(originals
            .insert(
                path.to_owned(),
                std::fs::read_to_string(root.join(path)).unwrap()
            )
            .is_none());
    }
    let repository = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    for (package, directory, file) in [
        ("std.data.json", "data-json", "json"),
        ("std.data.json.digits", "data-json-digits", "digits"),
        ("std.data.json.doc", "data-json-doc", "doc"),
        ("std.data.json.query", "data-json-query", "query"),
        ("std.data.json.scan", "data-json-scan", "scan"),
        ("std.data.json.token", "data-json-token", "token"),
        ("std.data.json.utf8", "data-json-utf8", "utf8"),
        ("std.data.json.write", "data-json-write", "write"),
        ("std.io", "io", "io"),
    ] {
        let path = format!("dependencies/{package}/0.1.0/{file}.spx");
        let physical = repository.join(format!("std/{directory}/src/{file}.spx"));
        assert!(originals
            .insert(path, std::fs::read_to_string(physical).unwrap())
            .is_none());
    }
    originals
}

#[test]
fn paired_plan_is_one_checked_revision_with_order_independent_exact_replay() {
    let root = paired_fixture("codec-plan-pair");
    let original_files = paired_originals(&root);
    let bundle = project::with_authenticated_project(&root.join("semaprax.toml"), |snapshot| {
        let revision = snapshot.retain_revision();
        let plan = entries();
        let bundle = project::derive_json_codec_plan(&revision, &plan)?;
        let mut reordered = plan.clone();
        reordered.reverse();
        assert_eq!(
            bundle,
            project::derive_json_codec_plan(&revision, &reordered)?
        );
        project::verify_json_codec_plan(&revision, &reordered, &bundle)?;
        assert_eq!(
            bundle.authored_project_revision,
            revision.project_revision()
        );
        assert_ne!(
            bundle.candidate_project_revision,
            bundle.authored_project_revision
        );
        assert_eq!(bundle.entries.len(), 2);
        assert_eq!(bundle.candidate_sources.len(), revision.sources().len());
        let mut covered = std::collections::BTreeSet::new();
        for source in &bundle.candidate_sources {
            assert!(
                covered.insert(source.path.clone()),
                "duplicate candidate path"
            );
            let original = bundle
                .authored_sources
                .iter()
                .find(|original| original.path == source.path)
                .expect("every candidate has its exact authored source");
            let projected = canonical(&source.source);
            assert_eq!(canonical(&projected), projected);
            if bundle
                .entries
                .iter()
                .any(|entry| entry.source_path == source.path)
            {
                assert_eq!(projected, source.source);
            } else {
                // Dependency sources are preserved bytes, not formatter output.
                assert_eq!(source.source, original.source);
                assert_eq!(source.sha256, original.sha256);
            }
            assert_eq!(
                source.sha256,
                format!(
                    "sha256:{:x}",
                    semaprax::digest_hex::LowerHex(Sha256::digest(source.source.as_bytes()))
                )
            );
        }
        assert_eq!(covered.len(), bundle.authored_sources.len());
        assert_eq!(bundle.authored_sources.len(), original_files.len());
        for original in &bundle.authored_sources {
            let physical_original = original_files
                .get(&original.path)
                .expect("every authored path has an independent physical source");
            assert_eq!(&original.source, physical_original);
            assert_eq!(
                original.sha256,
                format!(
                    "sha256:{:x}",
                    semaprax::digest_hex::LowerHex(Sha256::digest(physical_original.as_bytes()))
                )
            );
        }
        Ok(bundle)
    })
    .unwrap();
    assert_eq!(paired_originals(&root), original_files);
    for source in &bundle.candidate_sources {
        if bundle
            .entries
            .iter()
            .any(|entry| entry.source_path == source.path)
        {
            std::fs::write(root.join(&source.path), &source.source).unwrap();
        } else {
            assert_eq!(
                &source.source,
                original_files
                    .get(&source.path)
                    .expect("unchanged dependency is inventoried")
            );
        }
    }
    project::with_authenticated_project(&root.join("semaprax.toml"), |snapshot| {
        let revision = snapshot.retain_revision();
        assert_eq!(
            revision.project_revision(),
            bundle.candidate_project_revision
        );
        assert_eq!(
            revision.workspace_revision(),
            bundle.candidate_workspace_revision
        );
        let graph = revision.semantic_graph();
        assert!(graph.contains("application.patient.json.decode"));
        assert!(graph.contains("application.other.json.encode"));
        Ok(())
    })
    .unwrap();
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn paired_plan_refuses_overlap_invalid_members_bounds_and_receipt_drift() {
    let root = paired_fixture("codec-plan-hostile");
    let bundle = project::with_authenticated_project(&root.join("semaprax.toml"), |snapshot| {
        let revision = snapshot.retain_revision();
        let plan = entries();
        let bundle = project::derive_json_codec_plan(&revision, &plan)?;
        for invalid in [
            vec![],
            vec![plan[0].clone(), plan[0].clone()],
            vec![plan[0].clone(); 9],
        ] {
            assert_eq!(
                project::derive_json_codec_plan(&revision, &invalid).unwrap_err()[0].code,
                "SPX-J180"
            );
        }
        let mut overlap = plan.clone();
        overlap[1].source_path = overlap[0].source_path.clone();
        assert_eq!(
            project::derive_json_codec_plan(&revision, &overlap).unwrap_err()[0].code,
            "SPX-J180"
        );
        overlap = plan.clone();
        overlap[1].record_id = overlap[0].record_id.clone();
        assert_eq!(
            project::derive_json_codec_plan(&revision, &overlap).unwrap_err()[0].code,
            "SPX-J180"
        );
        let mut invalid = plan.clone();
        invalid[1].record_id = "missing.record".to_owned();
        assert_eq!(
            project::derive_json_codec_plan(&revision, &invalid).unwrap_err()[0].code,
            "SPX-J180"
        );
        let mut changed = bundle.clone();
        changed.entries[0].max_string_bytes = Some(16);
        assert_eq!(
            project::verify_json_codec_plan(&revision, &plan, &changed).unwrap_err()[0].code,
            "SPX-J180"
        );
        changed = bundle.clone();
        changed.candidate_sources[0].source.push('\n');
        assert_eq!(
            project::verify_json_codec_plan(&revision, &plan, &changed).unwrap_err()[0].code,
            "SPX-J180"
        );
        invalid = plan.clone();
        invalid[1].profile = project::JsonCodecProfile::NestedResponse {
            max_string_bytes: 65,
            max_array_items: 256,
        };
        assert_eq!(
            project::derive_json_codec_plan(&revision, &invalid).unwrap_err()[0].code,
            "SPX-J180"
        );
        Ok(bundle)
    })
    .unwrap();
    let path = root.join("src/schema.spx");
    let changed = std::fs::read_to_string(&path)
        .unwrap()
        .replace("n: i64", "n: u8");
    std::fs::write(path, changed).unwrap();
    project::with_authenticated_project(&root.join("semaprax.toml"), |snapshot| {
        assert_eq!(
            project::verify_json_codec_plan(&snapshot.retain_revision(), &entries(), &bundle)
                .unwrap_err()[0]
                .code,
            "SPX-J180"
        );
        Ok(())
    })
    .unwrap();
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn paired_cli_publishes_only_one_complete_new_bundle_and_invalid_plan_leaves_none() {
    let root = paired_fixture("codec-plan-cli");
    let plan = serde_json::json!({"schema":"semaprax.json-codec-plan.v1","entries":[
        {"source":"src/schema.spx","record":"application.patient","profile":"flat-scalars.v1"},
        {"source":"src/second.spx","record":"application.other","profile":"flat-scalars.v1"}
    ]});
    std::fs::write(root.join("plan.json"), serde_json::to_vec(&plan).unwrap()).unwrap();
    let run = |output: &str| {
        Command::new(env!("CARGO_BIN_EXE_semaprax"))
            .current_dir(&root)
            .args(["json-codec", ".", "--plan", "plan.json", "--output", output])
            .output()
            .unwrap()
    };
    let result = run("candidate.json");
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    let bytes = std::fs::read(root.join("candidate.json")).unwrap();
    let bundle: project::JsonCodecBundle = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(bundle.entries.len(), 2);
    assert!(!run("candidate.json").status.success());
    assert_eq!(std::fs::read(root.join("candidate.json")).unwrap(), bytes);
    let mut stale = plan.clone();
    stale["expected_project_revision"] = format!("sha256:{}", "0".repeat(64)).into();
    std::fs::write(root.join("plan.json"), serde_json::to_vec(&stale).unwrap()).unwrap();
    assert!(!run("stale.json").status.success());
    assert!(!root.join("stale.json").exists());
    let mut invalid = plan;
    invalid["entries"][1]["record"] = "missing".into();
    std::fs::write(
        root.join("plan.json"),
        serde_json::to_vec(&invalid).unwrap(),
    )
    .unwrap();
    assert!(!run("absent.json").status.success());
    assert!(!root.join("absent.json").exists());
    assert!(!std::fs::read_dir(&root).unwrap().any(|entry| entry
        .unwrap()
        .file_name()
        .to_string_lossy()
        .contains(".json-codec-")));
    assert_eq!(
        std::fs::read_to_string(root.join("src/schema.spx")).unwrap(),
        canonical(SCHEMA)
    );
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn paired_plan_rejects_generated_identity_collision_in_another_authored_module() {
    let root = paired_fixture("codec-plan-cross-identity");
    let path = root.join("src/second.spx");
    let mut text = std::fs::read_to_string(&path).unwrap();
    text.push_str(
        "\n@id(\"application.patient.json.encode\") fn reserved_codec_identity()->i64 { 0 }\n",
    );
    std::fs::write(path, canonical(&text)).unwrap();
    project::with_authenticated_project(&root.join("semaprax.toml"), |snapshot| {
        let revision = snapshot.retain_revision();
        let error = project::derive_json_codec_plan(&revision, &entries()).unwrap_err();
        assert_eq!(error[0].code, "SPX-G172");
        assert!(error[0].message.contains("application.patient.json.encode"));
        Ok(())
    })
    .unwrap();
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn paired_plan_live_input_drift_refuses_before_artifact_handoff() {
    let root = paired_fixture("codec-plan-live-drift");
    let result = project::with_authenticated_project(&root.join("semaprax.toml"), |snapshot| {
        let bundle = project::derive_json_codec_plan(&snapshot.retain_revision(), &entries())?;
        let path = root.join("src/schema.spx");
        let mut text = std::fs::read_to_string(&path).unwrap();
        text.push('\n');
        std::fs::write(path, text).unwrap();
        Ok(bundle)
    });
    assert_eq!(result.unwrap_err()[0].code, "SPX-J102");
    assert!(!root.join("candidate.json").exists());
    std::fs::remove_dir_all(root).unwrap();
}
