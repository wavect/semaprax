#[path = "support/project_product.rs"]
mod support;

use semaprax::{codegen, project, wasm};
use serde_json::{json, Value};
use support::{
    native_rust_sdk_required, run_core_wasm, run_native_c, run_project_rust_sdk, run_web_carrier,
    subject, validate_web_carrier, Daemon, ProjectFixture, BUILD_MAX_BYTES,
};

#[test]
fn returned_web_carrier_inventory_is_authenticated_before_materialization() {
    let paths = [
        "app.wasm",
        "semaprax.js",
        "semaprax.bindings.js",
        "semaprax.bindings.d.ts",
        "semaprax.scalar-exports.json",
        "package.json",
        "index.html",
    ];
    let valid = json!({
        "artifacts": paths.map(|path| json!({"path":path,"content_hex":""}))
    });
    assert!(validate_web_carrier(&valid).is_ok());

    for hostile in ["/tmp/app.wasm", "../app.wasm", "nested/app.wasm"] {
        let mut carrier = valid.clone();
        carrier["artifacts"][0]["path"] = json!(hostile);
        assert!(validate_web_carrier(&carrier).is_err());
    }
    let mut duplicate = valid.clone();
    duplicate["artifacts"][0]["path"] = duplicate["artifacts"][1]["path"].clone();
    assert!(validate_web_carrier(&duplicate).is_err());
    let mut unknown = valid;
    unknown["artifacts"][0]["path"] = json!("foreign.wasm");
    assert!(validate_web_carrier(&unknown).is_err());
}

#[test]
fn calculator_project_survives_the_complete_agent_change_and_consumer_cycle() {
    let fixture = ProjectFixture::calculator("unified");
    let baseline = project::with_authenticated_project(&fixture.manifest(), |snapshot| {
        snapshot.check()?;
        let entry = snapshot.execute_entry(&project::ProjectExecutionOptions::default())?;
        assert_eq!(
            entry.outcome(),
            &project::ProjectExecutionOutcome::Returned(42)
        );
        let tests = snapshot.execute_test(&project::ProjectExecutionOptions::default())?;
        assert_eq!(
            tests.outcome(),
            &project::ProjectExecutionOutcome::Returned(0)
        );
        let entry_c =
            codegen::emit_hir_c(snapshot.entry_program()).map_err(|diagnostic| vec![diagnostic])?;
        run_native_c(&entry_c, "baseline-entry", "42", &["-O0", "-O2"]);
        let entry_wasm =
            wasm::emit_resolved_module(snapshot.entry_program()).map_err(|error| vec![error])?;
        run_core_wasm(&entry_wasm, "baseline-entry", "42");
        let test_c =
            codegen::emit_hir_c(snapshot.test_program()).map_err(|diagnostic| vec![diagnostic])?;
        run_native_c(&test_c, "baseline-tests", "0", &["-O0", "-O2"]);
        run_core_wasm(&snapshot.test_wasm_module()?, "baseline-tests", "0");
        let carrier = snapshot.build_web_inline(BUILD_MAX_BYTES)?;
        carrier.verify().map_err(|diagnostic| vec![diagnostic])?;
        Ok((
            snapshot.project_revision().to_owned(),
            snapshot.workspace_revision().to_owned(),
            serde_json::from_str::<Value>(carrier.envelope()).unwrap(),
        ))
    })
    .unwrap();
    let baseline_web = run_web_carrier(&baseline.2, "baseline-direct");
    let baseline_rust =
        native_rust_sdk_required().then(|| run_project_rust_sdk(&fixture, "baseline"));

    let mut daemon = Daemon::workflow(&fixture);
    let protocol = daemon.call(1, "protocol", None);
    assert_eq!(
        protocol["result"]["protocol"],
        "semaprax.agent-transport.v4"
    );
    for method in [
        "workspace/open",
        "graph",
        "context",
        "rename/derive",
        "change/preview",
        "impact",
        "review",
        "change/apply",
        "build",
        "test",
    ] {
        assert!(
            protocol["result"]["methods"]
                .as_array()
                .unwrap()
                .contains(&json!(method)),
            "v4 protocol omitted {method}"
        );
    }

    let opened = daemon.call(2, "workspace/open", None);
    let base_project = opened["result"]["project_revision"]
        .as_str()
        .unwrap()
        .to_owned();
    let base_workspace = opened["result"]["workspace_revision"]
        .as_str()
        .unwrap()
        .to_owned();
    assert_eq!(base_project, baseline.0);
    assert_eq!(base_workspace, baseline.1);

    let graph = daemon.call(3, "graph", Some(subject(&base_project, &base_workspace)));
    assert_eq!(graph["result"]["graph"]["project_revision"], base_project);
    assert!(graph["result"]["graph"]["declarations"]
        .as_array()
        .unwrap()
        .iter()
        .any(|declaration| declaration["id"] == "calculator.add"));
    let mut context_params = subject(&base_project, &base_workspace);
    context_params.as_object_mut().unwrap().extend([
        ("target_kind".to_owned(), json!("declaration")),
        ("target".to_owned(), json!("calculator.add")),
    ]);
    let context = daemon.call(4, "context", Some(context_params));
    assert_eq!(
        context["result"]["context"]["schema"],
        "semaprax.project-semantic-context.v1"
    );

    let baseline_build = daemon.call(
        5,
        "build",
        Some(json!({
            "project_revision":base_project,
            "workspace_revision":base_workspace,
            "target":"web",
            "max_bytes":BUILD_MAX_BYTES
        })),
    );
    assert_eq!(baseline_build["result"]["build"], baseline.2);
    run_web_carrier(&baseline_build["result"]["build"], "baseline-daemon");

    let derivation = daemon.call(
        6,
        "rename/derive",
        Some(json!({
            "project_revision":base_project,
            "workspace_revision":base_workspace,
            "target_id":"calculator.add",
            "from":"add",
            "to":"sum"
        })),
    );
    assert_eq!(
        derivation["result"]["derivation"]["schema"],
        "semaprax.project-rename-derivation.v1"
    );
    let derivation_digest = derivation["result"]["derivation"]["artifact_digest"]
        .as_str()
        .unwrap()
        .to_owned();
    let preview = daemon.call(
        7,
        "change/preview",
        Some(json!({
            "project_revision":base_project,
            "workspace_revision":base_workspace,
            "derivation_digest":derivation_digest
        })),
    );
    let change = &preview["result"]["change"];
    assert_eq!(change["schema"], "semaprax.project-change-preview.v1");
    assert_eq!(
        change["impact"]["conclusions"]["stable_identity_preserved"],
        true
    );
    assert_eq!(
        change["rename_preview"]["target"]["stable_id"],
        "calculator.add"
    );
    let change_digest = change["artifact_digest"].as_str().unwrap().to_owned();
    let candidate_project = change["rename_preview"]["candidate_project_revision"]
        .as_str()
        .unwrap()
        .to_owned();
    let candidate_workspace = change["rename_preview"]["candidate_workspace_revision"]
        .as_str()
        .unwrap()
        .to_owned();
    let candidate_source = change["rename_preview"]["candidate_source"]["source_revision"]
        .as_str()
        .unwrap()
        .to_owned();
    assert_ne!(candidate_project, base_project);
    assert_ne!(candidate_workspace, base_workspace);

    for (id, method, schema) in [
        (8, "impact", "semaprax.project-change-impact.v1"),
        (9, "review", "semaprax.project-change-review.v1"),
    ] {
        let response = daemon.call(
            id,
            method,
            Some(json!({
                "project_revision":base_project,
                "workspace_revision":base_workspace,
                "change_preview_digest":change_digest
            })),
        );
        assert_eq!(response["result"][method]["schema"], schema);
    }

    let applied = daemon.call(
        10,
        "change/apply",
        Some(json!({
            "project_revision":base_project,
            "workspace_revision":base_workspace,
            "change_preview_digest":change_digest
        })),
    );
    assert_eq!(applied["result"]["applied"], true);
    assert_eq!(
        applied["result"]["candidate_project_revision"],
        candidate_project
    );
    assert_eq!(
        applied["result"]["candidate_workspace_revision"],
        candidate_workspace
    );
    assert_eq!(
        applied["result"]["candidate_source_revision"],
        candidate_source
    );
    assert!(fixture
        .core_source()
        .contains("@id(\"calculator.add\")\nfn sum("));

    let renamed_graph = daemon.call(
        11,
        "graph",
        Some(subject(&candidate_project, &candidate_workspace)),
    );
    assert!(renamed_graph["result"]["graph"]["declarations"]
        .as_array()
        .unwrap()
        .iter()
        .any(|declaration| declaration["id"] == "calculator.add"));
    let renamed_build = daemon.call(
        12,
        "build",
        Some(json!({
            "project_revision":candidate_project,
            "workspace_revision":candidate_workspace,
            "target":"web",
            "max_bytes":BUILD_MAX_BYTES
        })),
    );
    let renamed_carrier = &renamed_build["result"]["build"];
    assert_eq!(renamed_carrier["schema"], project::PROJECT_WEB_BUILD_SCHEMA);
    assert_eq!(renamed_carrier["project_revision"], candidate_project);
    let renamed_web = run_web_carrier(renamed_carrier, "renamed-daemon");
    for stable_artifact in [
        "app.wasm",
        "semaprax.js",
        "semaprax.bindings.js",
        "semaprax.bindings.d.ts",
    ] {
        assert_eq!(
            baseline_web[stable_artifact], renamed_web[stable_artifact],
            "display rename changed stable-ID Web artifact {stable_artifact}"
        );
    }
    let baseline_manifest: Value =
        serde_json::from_slice(&baseline_web["semaprax.scalar-exports.json"]).unwrap();
    let renamed_manifest: Value =
        serde_json::from_slice(&renamed_web["semaprax.scalar-exports.json"]).unwrap();
    assert_ne!(
        baseline_manifest["project_revision"],
        renamed_manifest["project_revision"]
    );
    assert_ne!(
        baseline_manifest["workspace_revision"],
        renamed_manifest["workspace_revision"]
    );
    assert_eq!(
        baseline_manifest["scalar_abi"], renamed_manifest["scalar_abi"],
        "display rename changed the stable-ID scalar ABI"
    );
    let tests = daemon.call(
        13,
        "test",
        Some(subject(&candidate_project, &candidate_workspace)),
    );
    assert_eq!(tests["result"]["command_succeeded"], true);
    daemon.finish();

    project::with_authenticated_project(&fixture.manifest(), |snapshot| {
        assert_eq!(snapshot.project_revision(), candidate_project);
        assert_eq!(snapshot.workspace_revision(), candidate_workspace);
        let entry = snapshot.execute_entry(&project::ProjectExecutionOptions::default())?;
        assert_eq!(
            entry.outcome(),
            &project::ProjectExecutionOutcome::Returned(42)
        );
        let tests = snapshot.execute_test(&project::ProjectExecutionOptions::default())?;
        assert_eq!(
            tests.outcome(),
            &project::ProjectExecutionOutcome::Returned(0)
        );
        let entry_c =
            codegen::emit_hir_c(snapshot.entry_program()).map_err(|diagnostic| vec![diagnostic])?;
        run_native_c(&entry_c, "renamed-entry", "42", &["-O0", "-O2"]);
        let entry_wasm =
            wasm::emit_resolved_module(snapshot.entry_program()).map_err(|error| vec![error])?;
        run_core_wasm(&entry_wasm, "renamed-entry", "42");
        let test_c =
            codegen::emit_hir_c(snapshot.test_program()).map_err(|diagnostic| vec![diagnostic])?;
        run_native_c(&test_c, "renamed-tests", "0", &["-O0", "-O2"]);
        run_core_wasm(&snapshot.test_wasm_module()?, "renamed-tests", "0");
        Ok(())
    })
    .unwrap();

    if let Some(baseline_rust) = baseline_rust {
        assert_eq!(baseline_rust.project_revision, base_project);
        assert_eq!(baseline_rust.workspace_revision, base_workspace);
        let renamed_rust = run_project_rust_sdk(&fixture, "renamed");
        assert_eq!(renamed_rust.project_revision, candidate_project);
        assert_eq!(renamed_rust.workspace_revision, candidate_workspace);
        assert_ne!(baseline_rust.subject_digest, renamed_rust.subject_digest);
        assert_ne!(
            baseline_rust.source_revisions,
            renamed_rust.source_revisions
        );
        assert!(renamed_rust.source_revisions.contains(&candidate_source));
        assert_eq!(
            baseline_rust.manifest_exports, renamed_rust.manifest_exports,
            "display rename changed the stable-ID Project Rust SDK export inventory"
        );
    }
}
