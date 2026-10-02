//! Compiler-owned explorer projection regressions.
use semaprax::image_transport::{VNextPolicy, VNextSession};
use semaprax::project::{
    with_authenticated_project, ExplorerMode, ExplorerPageOptions, ExplorerQuery, ExplorerSide,
    ExplorerView, ProjectCandidate, ProjectSemanticImage, SemanticChange,
};
use serde_json::{json, Value};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

static SERIAL: AtomicU64 = AtomicU64::new(0);
const FILES: [&str; 4] = [
    "semaprax.toml",
    "src/app.spx",
    "src/core.spx",
    "src/tests.spx",
];
struct Fixture(PathBuf);
impl Fixture {
    fn new() -> Self {
        let root = std::env::temp_dir().join(format!(
            "spx-explorer-{}-{}",
            std::process::id(),
            SERIAL.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir_all(root.join("src")).unwrap();
        let sample = Path::new(env!("CARGO_MANIFEST_DIR")).join("examples/calculator-project");
        for path in FILES {
            std::fs::copy(sample.join(path), root.join(path)).unwrap();
        }
        Self(root.canonicalize().unwrap())
    }
    fn manifest(&self) -> PathBuf {
        self.0.join("semaprax.toml")
    }
    fn image(&self) -> ProjectSemanticImage {
        with_authenticated_project(&self.manifest(), |snapshot| {
            ProjectSemanticImage::derive(snapshot.retain_revision(), snapshot.project_revision())
        })
        .unwrap()
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}
fn call(session: &mut VNextSession, method: &str, params: Value) -> Value {
    let request = json!({"jsonrpc":"2.0","id":1,"method":method,"params":params});
    serde_json::from_slice(
        &session
            .handle_frame(request.to_string().as_bytes())
            .unwrap(),
    )
    .unwrap()
}
fn payload(value: Value) -> Value {
    assert!(value.get("error").is_none(), "{value}");
    value["result"]["payload"].clone()
}
fn inventory<'a>(summary: &'a Value, view: &str) -> &'a Value {
    summary["inventories"]
        .as_array()
        .unwrap()
        .iter()
        .find(|row| row["view"] == view)
        .unwrap()
}

#[test]
fn image_overview_pages_match_the_held_library_subject() {
    let fixture = Fixture::new();
    let image = fixture.image();
    let expected: Value = serde_json::from_str(
        &image
            .explorer_summary(
                image.image_digest(),
                ExplorerMode::Overview,
                None,
                ExplorerQuery::default(),
            )
            .unwrap(),
    )
    .unwrap();
    let mut session = VNextSession::open(&fixture.manifest(), VNextPolicy::default()).unwrap();
    let image_revision = session.image_revision().to_owned();
    let actual = payload(call(
        &mut session,
        "image/explorer-summary",
        json!({"image_revision":image_revision,"mode":"overview"}),
    ));
    assert_eq!(actual, expected);
    assert_eq!(actual["source_authority"], false);
    for view in ExplorerView::ALL {
        let handle = actual["inventories"]
            .as_array()
            .unwrap()
            .iter()
            .find(|row| row["view"] == view.name())
            .unwrap()["handle"]
            .as_str()
            .unwrap();
        let page = payload(call(
            &mut session,
            "image/explorer-page",
            json!({"image_revision":image_revision,"mode":"overview","view":view.name(),"handle":handle}),
        ));
        let expected: Value = serde_json::from_str(
            &image
                .explorer_page(
                    image.image_digest(),
                    ExplorerMode::Overview,
                    None,
                    ExplorerQuery::default(),
                    view,
                    handle,
                    None,
                    ExplorerPageOptions::default(),
                )
                .unwrap(),
        )
        .unwrap();
        assert_eq!(page, expected);
        if view == ExplorerView::Declarations {
            let declaration = page["items"]
                .as_array()
                .unwrap()
                .iter()
                .find(|row| row["id"] == "calculator.add")
                .unwrap();
            let source = &declaration["source_reference"];
            assert_eq!(source["path"], "src/core.spx");
            assert!(source["source_revision"]
                .as_str()
                .unwrap()
                .starts_with("sha256:"));
            assert!(source["source_digest"]
                .as_str()
                .unwrap()
                .starts_with("sha256:"));
            assert!(source["span"]["start"].as_u64().is_some());
            let start = source["span"]["start"].as_u64().unwrap();
            let end = source["span"]["end"].as_u64().unwrap();
            assert!(end > start);
        }
    }
    session.finish().unwrap();
}

#[test]
fn explorer_rejects_wrong_handles_and_overview_targets() {
    let fixture = Fixture::new();
    let mut session = VNextSession::open(&fixture.manifest(), VNextPolicy::default()).unwrap();
    let image_revision = session.image_revision().to_owned();
    let summary = payload(call(
        &mut session,
        "image/explorer-summary",
        json!({"image_revision":image_revision,"mode":"overview"}),
    ));
    let handle = summary["inventories"][0]["handle"].as_str().unwrap();
    let wrong = call(
        &mut session,
        "image/explorer-page",
        json!({"image_revision":image_revision,"mode":"overview","view":"modules","handle":format!("{handle}x")}),
    );
    assert!(wrong.get("error").is_some());
    let target = call(
        &mut session,
        "image/explorer-summary",
        json!({"image_revision":image_revision,"mode":"overview","target":"calculator.add"}),
    );
    assert!(target.get("error").is_some());
    session.finish().unwrap();
}

#[test]
fn candidate_explorer_binds_side_handle_cursor_and_rejects_source_drift() {
    let fixture = Fixture::new();
    let disk = FILES
        .iter()
        .map(|path| {
            (
                path.to_string(),
                std::fs::read(fixture.0.join(path)).unwrap(),
            )
        })
        .collect::<Vec<_>>();
    let mut session = VNextSession::open(
        &fixture.manifest(),
        VNextPolicy {
            candidate_prepare: true,
            ..VNextPolicy::default()
        },
    )
    .unwrap();
    let image_revision = session.image_revision().to_owned();
    let root = payload(call(
        &mut session,
        "candidate/open",
        json!({"image_revision":image_revision}),
    ));
    let intent = json!({
        "kind":"replace_function_body",
        "target":"calculator.add",
        "body":{"kind":"place","name":"left"}
    });
    let candidate = payload(call(
        &mut session,
        "candidate/apply-intent",
        json!({"image_revision":image_revision,"candidate_revision":root["candidate_revision"],"intent":intent}),
    ));
    let candidate_revision = candidate["candidate_revision"].as_str().unwrap();
    let query = json!({
        "image_revision":image_revision,
        "candidate_revision":candidate_revision,
        "mode":"context",
        "target":"calculator.add",
        "direction":"both",
        "depth":1,
        "max_nodes":256,
        "analysis_max_bytes":262144
    });
    let mut base_params = query.clone();
    base_params["side"] = json!("base");
    let base_summary = payload(call(
        &mut session,
        "candidate/explorer-summary",
        base_params,
    ));
    let mut candidate_params = query.clone();
    candidate_params["side"] = json!("candidate");
    let candidate_summary = payload(call(
        &mut session,
        "candidate/explorer-summary",
        candidate_params,
    ));
    assert_eq!(base_summary["subject"]["side"], "base");
    assert_eq!(candidate_summary["subject"]["side"], "candidate");
    let overview = json!({
        "image_revision":image_revision,
        "candidate_revision":candidate_revision,
        "side":"base",
        "mode":"overview",
        "direction":"both",
        "depth":1,
        "max_nodes":256,
        "analysis_max_bytes":262144
    });
    let overview_summary = payload(call(&mut session, "candidate/explorer-summary", overview));
    let inventory = overview_summary["inventories"]
        .as_array()
        .unwrap()
        .iter()
        .find(|row| row["view"] == "declarations")
        .unwrap();
    let handle = inventory["handle"].as_str().unwrap();
    let page_params = json!({
        "image_revision":image_revision,
        "candidate_revision":candidate_revision,
        "side":"base",
        "mode":"overview",
        "direction":"both",
        "depth":1,
        "max_nodes":256,
        "analysis_max_bytes":262144,
        "view":"declarations",
        "handle":handle,
        "page_size":1,
        "max_bytes":65536
    });
    let first = payload(call(
        &mut session,
        "candidate/explorer-page",
        page_params.clone(),
    ));
    let cursor = first["next_cursor"]
        .as_str()
        .expect("overview page has a continuation");
    let mut wrong_side = page_params.clone();
    wrong_side["side"] = json!("candidate");
    assert!(call(&mut session, "candidate/explorer-page", wrong_side)
        .get("error")
        .is_some());
    let mut wrong_cursor = page_params;
    wrong_cursor["cursor"] = json!(format!("{cursor}x"));
    assert!(call(&mut session, "candidate/explorer-page", wrong_cursor)
        .get("error")
        .is_some());

    let source = fixture.0.join("src/core.spx");
    let text = std::fs::read(&source).unwrap();
    let mut changed = text.clone();
    changed.extend_from_slice(b"\n// external editor change\n");
    std::fs::write(&source, changed).unwrap();
    let drifted = std::fs::read(&source).unwrap();
    let mut stale_params = query;
    stale_params["side"] = json!("base");
    let stale = call(&mut session, "candidate/explorer-summary", stale_params);
    assert!(stale.get("error").is_some());
    let still_stale = call(
        &mut session,
        "image/explorer-summary",
        json!({
            "image_revision":image_revision,"mode":"overview"
        }),
    );
    assert!(still_stale.get("error").is_some());
    assert_eq!(session.image_revision(), image_revision);
    assert!(session.finish().is_err());
    assert_eq!(std::fs::read(&source).unwrap(), drifted);
    for (path, bytes) in disk {
        if path != "src/core.spx" {
            assert_eq!(std::fs::read(fixture.0.join(path)).unwrap(), bytes);
        }
    }
}

#[test]
fn deleted_declaration_keeps_base_context_and_impact_but_is_absent_on_candidate_side() {
    let fixture = Fixture::new();
    let source = fixture.0.join("src/core.spx");
    let mut bytes = std::fs::read(&source).unwrap();
    let original = b"    left + right\n";
    let at = bytes
        .windows(original.len())
        .position(|part| part == original)
        .unwrap();
    bytes.splice(
        at..at + original.len(),
        b"    explorer_unused(left) + right\n".iter().copied(),
    );
    let insertion = b"@id(\"calculator.explorer-unused\")\nfn explorer_unused(value: i64) -> i64\n{\n    value\n}\n\n";
    let before_add = b"@id(\"calculator.add\")";
    let at = bytes
        .windows(before_add.len())
        .position(|part| part == before_add)
        .unwrap();
    bytes.splice(at..at, insertion.iter().copied());
    std::fs::write(&source, bytes).unwrap();
    let mut session = VNextSession::open(
        &fixture.manifest(),
        VNextPolicy {
            candidate_prepare: true,
            ..VNextPolicy::default()
        },
    )
    .unwrap();
    let image_revision = session.image_revision().to_owned();
    let root = payload(call(
        &mut session,
        "candidate/open",
        json!({"image_revision":image_revision}),
    ));
    let still_consumed = call(
        &mut session,
        "candidate/apply-intent",
        json!({
            "image_revision":image_revision,
            "candidate_revision":root["candidate_revision"],
            "intent":{"kind":"delete_declaration","target":"calculator.explorer-unused"}
        }),
    );
    assert_eq!(
        still_consumed["error"]["data"]["diagnostics"][0]["code"], "SPX-T203",
        "{still_consumed}"
    );
    let severed = payload(call(
        &mut session,
        "candidate/apply-intent",
        json!({
            "image_revision":image_revision,
            "candidate_revision":root["candidate_revision"],
            "intent":{"kind":"replace_function_body","target":"calculator.add","body":{"kind":"place","name":"left"}}
        }),
    ));
    let delete = json!({"kind":"delete_declaration","target":"calculator.explorer-unused"});
    let deleted = payload(call(
        &mut session,
        "candidate/apply-intent",
        json!({
            "image_revision":image_revision,
            "candidate_revision":severed["candidate_revision"],
            "intent":delete
        }),
    ));
    let candidate_revision = deleted["candidate_revision"].as_str().unwrap();
    for mode in ["context", "impact"] {
        let base = payload(call(
            &mut session,
            "candidate/explorer-summary",
            json!({
                "image_revision":image_revision,
                "candidate_revision":candidate_revision,
                "side":"base","mode":mode,"target":"calculator.explorer-unused"
            }),
        ));
        assert_eq!(base["subject"]["side"], "base");
        assert_eq!(base["coverage"]["complete_within_query"], true);
        let declarations = base["inventories"]
            .as_array()
            .unwrap()
            .iter()
            .find(|row| row["view"] == "declarations")
            .unwrap();
        assert!(declarations["total_items"].as_u64().unwrap() > 0);
        let candidate = call(
            &mut session,
            "candidate/explorer-summary",
            json!({
                "image_revision":image_revision,
                "candidate_revision":candidate_revision,
                "side":"candidate","mode":mode,"target":"calculator.explorer-unused"
            }),
        );
        assert_eq!(
            candidate["error"]["data"]["diagnostics"][0]["code"], "SPX-G177",
            "{candidate}"
        );
    }
    session.finish().unwrap();
}

#[test]
fn deletion_rejects_exported_declaration_without_changing_candidate() {
    let fixture = Fixture::new();
    let mut session = VNextSession::open(
        &fixture.manifest(),
        VNextPolicy {
            candidate_prepare: true,
            ..VNextPolicy::default()
        },
    )
    .unwrap();
    let image_revision = session.image_revision().to_owned();
    let root = payload(call(
        &mut session,
        "candidate/open",
        json!({"image_revision":image_revision}),
    ));
    let rejected = call(
        &mut session,
        "candidate/apply-intent",
        json!({
            "image_revision":image_revision,
            "candidate_revision":root["candidate_revision"],
            "intent":{"kind":"delete_declaration","target":"calculator.add"}
        }),
    );
    assert!(rejected.get("error").is_some());
    assert_eq!(
        rejected["error"]["data"]["diagnostics"][0]["code"], "SPX-G225",
        "{rejected}"
    );
    session.finish().unwrap();
}

#[test]
fn candidate_context_and_impact_pages_equal_the_exact_core_subject() {
    let fixture = Fixture::new();
    let image = fixture.image();
    let opened = ProjectCandidate::open(
        Arc::clone(image.revision()),
        image.revision().project_revision(),
    )
    .unwrap();
    let change = SemanticChange::new(opened.revision().project_revision(), &json!({
        "kind":"replace_function_body","target":"calculator.add","body":{"kind":"place","name":"left"}
    })).unwrap();
    let candidate = opened.apply(opened.candidate_digest(), &change).unwrap();
    let mut session = VNextSession::open(
        &fixture.manifest(),
        VNextPolicy {
            candidate_prepare: true,
            ..VNextPolicy::default()
        },
    )
    .unwrap();
    let image_revision = session.image_revision().to_owned();
    let root = payload(call(
        &mut session,
        "candidate/open",
        json!({"image_revision":image_revision}),
    ));
    let applied = payload(call(
        &mut session,
        "candidate/apply-intent",
        json!({
            "image_revision":image_revision,"candidate_revision":root["candidate_revision"],
            "intent":{"kind":"replace_function_body","target":"calculator.add","body":{"kind":"place","name":"left"}}
        }),
    ));
    assert_eq!(applied["candidate_revision"], candidate.candidate_digest());
    for (side, side_name) in [
        (ExplorerSide::Base, "base"),
        (ExplorerSide::Candidate, "candidate"),
    ] {
        for (mode, mode_name) in [
            (ExplorerMode::Context, "context"),
            (ExplorerMode::Impact, "impact"),
        ] {
            let params = json!({"image_revision":image_revision,"candidate_revision":candidate.candidate_digest(),
                "side":side_name,"mode":mode_name,"target":"calculator.add"});
            let actual = payload(call(
                &mut session,
                "candidate/explorer-summary",
                params.clone(),
            ));
            let expected: Value = serde_json::from_str(
                &candidate
                    .explorer_summary(
                        candidate.candidate_digest(),
                        side,
                        mode,
                        Some("calculator.add"),
                        ExplorerQuery::default(),
                    )
                    .unwrap(),
            )
            .unwrap();
            assert_eq!(actual, expected);
            for view in ExplorerView::ALL {
                let handle = inventory(&actual, view.name())["handle"].as_str().unwrap();
                let page_params = json!({"image_revision":image_revision,"candidate_revision":candidate.candidate_digest(),
                    "side":side_name,"mode":mode_name,"target":"calculator.add","view":view.name(),"handle":handle});
                let actual_page =
                    payload(call(&mut session, "candidate/explorer-page", page_params));
                let expected_page: Value = serde_json::from_str(
                    &candidate
                        .explorer_page(
                            candidate.candidate_digest(),
                            side,
                            mode,
                            Some("calculator.add"),
                            ExplorerQuery::default(),
                            view,
                            handle,
                            None,
                            ExplorerPageOptions::default(),
                        )
                        .unwrap(),
                )
                .unwrap();
                assert_eq!(actual_page, expected_page);
            }
        }
    }
    session.finish().unwrap();
}

#[test]
fn readonly_explorer_discovery_and_schema_do_not_grant_candidate_authority() {
    let fixture = Fixture::new();
    let mut readonly = VNextSession::open(&fixture.manifest(), VNextPolicy::default()).unwrap();
    let image_revision = readonly.image_revision().to_owned();
    let capabilities = payload(call(&mut readonly, "protocol/capabilities", json!({})));
    let methods = capabilities["methods"].as_array().unwrap();
    for name in ["image/explorer-summary", "image/explorer-page"] {
        assert!(methods.contains(&json!(name)));
    }
    for name in [
        "candidate/explorer-summary",
        "candidate/explorer-page",
        "candidate/open",
        "candidate/test",
        "candidate/commit",
    ] {
        assert!(!methods.contains(&json!(name)));
        assert_eq!(
            call(
                &mut readonly,
                name,
                json!({"image_revision":image_revision})
            )["error"]["code"],
            -32601
        );
    }
    let schemas = payload(call(&mut readonly, "protocol/schemas", json!({})));
    let descriptors = schemas["methods"].as_array().unwrap();
    for name in ["image/explorer-summary", "image/explorer-page"] {
        let method = descriptors
            .iter()
            .find(|row| row["method"] == name)
            .unwrap();
        assert_eq!(method["capability"], "semantic_read");
        assert_eq!(method["query"], true);
        assert_eq!(
            method["request_schema"]["properties"]["params"]["additionalProperties"],
            false
        );
        assert_eq!(
            method["success_response_schema"]["properties"]["result"]["properties"]["payload"]
                ["$ref"],
            "urn:semaprax.explorer-view.v1"
        );
    }
    for name in ["candidate/explorer-summary", "candidate/explorer-page"] {
        assert!(!descriptors.iter().any(|row| row["method"] == name));
    }
    let overview = payload(call(
        &mut readonly,
        "image/explorer-summary",
        json!({
            "image_revision":image_revision,"mode":"overview"
        }),
    ));
    assert_eq!(overview["schema"], "semaprax.explorer-view.v1");
    assert_eq!(overview["subject"]["side"], "current");
    assert_eq!(overview["source_authority"], false);
    assert_eq!(overview["execution"], false);
    assert_eq!(overview["publication_authority"], false);
    readonly.finish().unwrap();
}

#[test]
fn explorer_pages_reconstruct_inventory_and_bind_view_query_options_and_subject() {
    let fixture = Fixture::new();
    let mut session = VNextSession::open(&fixture.manifest(), VNextPolicy::default()).unwrap();
    let image_revision = session.image_revision().to_owned();
    let summary = payload(call(
        &mut session,
        "image/explorer-summary",
        json!({
            "image_revision":image_revision,"mode":"overview"
        }),
    ));
    let handle = inventory(&summary, "declarations")["handle"]
        .as_str()
        .unwrap();
    let total = inventory(&summary, "declarations")["total_items"]
        .as_u64()
        .unwrap() as usize;
    assert!(total > 1);
    let base = json!({"image_revision":image_revision,"mode":"overview", "view":"declarations",
        "handle":handle,"page_size":1,"max_bytes":65536});
    let mut cursor: Option<String> = None;
    let mut rows = Vec::new();
    let mut first_cursor = None;
    loop {
        let mut params = base.clone();
        if let Some(value) = &cursor {
            params["cursor"] = json!(value);
        }
        let page = payload(call(&mut session, "image/explorer-page", params));
        assert_eq!(page["offset"].as_u64().unwrap() as usize, rows.len());
        assert_eq!(page["total_items"].as_u64().unwrap() as usize, total);
        rows.extend(page["items"].as_array().unwrap().iter().cloned());
        let next = page["next_cursor"].as_str().map(str::to_owned);
        if first_cursor.is_none() {
            first_cursor = next.clone();
        }
        match next {
            Some(value) => cursor = Some(value),
            None => break,
        }
    }
    assert_eq!(rows.len(), total);
    let full = payload(call(
        &mut session,
        "image/explorer-page",
        json!({
            "image_revision":image_revision,"mode":"overview","view":"declarations", "handle":handle,
            "page_size":128,"max_bytes":524288
        }),
    ));
    assert_eq!(full["items"], json!(rows));
    let mut wrong_view = base.clone();
    wrong_view["view"] = json!("relations");
    assert_eq!(
        call(&mut session, "image/explorer-page", wrong_view)["error"]["data"]["diagnostics"][0]
            ["code"],
        "SPX-G327"
    );
    let mut wrong_query = base.clone();
    wrong_query["depth"] = json!(2);
    assert_eq!(
        call(&mut session, "image/explorer-page", wrong_query)["error"]["data"]["diagnostics"][0]
            ["code"],
        "SPX-G327"
    );
    let cursor = first_cursor.unwrap();
    let mut altered_options = base.clone();
    altered_options["cursor"] = json!(cursor.clone());
    altered_options["page_size"] = json!(2);
    assert_eq!(
        call(&mut session, "image/explorer-page", altered_options)["error"]["data"]["diagnostics"]
            [0]["code"],
        "SPX-G327"
    );
    let mut out_of_range = base.clone();
    out_of_range["cursor"] = json!(format!(
        "{}:{}",
        total + 1,
        cursor.split_once(':').unwrap().1
    ));
    assert_eq!(
        call(&mut session, "image/explorer-page", out_of_range)["error"]["data"]["diagnostics"][0]
            ["code"],
        "SPX-G327"
    );
    let foreign = Fixture::new();
    let foreign_source = foreign.0.join("src/core.spx");
    let mut bytes = std::fs::read(&foreign_source).unwrap();
    let original = b"    left + right\n";
    let at = bytes
        .windows(original.len())
        .position(|part| part == original)
        .unwrap();
    bytes.splice(
        at..at + original.len(),
        b"    left - right\n".iter().copied(),
    );
    std::fs::write(&foreign_source, bytes).unwrap();
    let mut other = VNextSession::open(&foreign.manifest(), VNextPolicy::default()).unwrap();
    let other_revision = other.image_revision().to_owned();
    let foreign_result = call(
        &mut other,
        "image/explorer-page",
        json!({
            "image_revision":other_revision,"mode":"overview","view":"declarations","handle":handle
        }),
    );
    assert_eq!(
        foreign_result["error"]["data"]["diagnostics"][0]["code"],
        "SPX-G327"
    );
    other.finish().unwrap();
    session.finish().unwrap();
}

#[test]
fn overview_keeps_same_display_names_distinct_by_explicit_identity() {
    let fixture = Fixture::new();
    for (path, id) in [
        ("src/core.spx", "calculator.core.same"),
        ("src/tests.spx", "calculator.tests.same"),
    ] {
        let source = fixture.0.join(path);
        let mut bytes = std::fs::read(&source).unwrap();
        bytes.extend_from_slice(
            format!("\n@id(\"{id}\")\nfn same(value: i64) -> i64\n{{\n    value\n}}\n").as_bytes(),
        );
        std::fs::write(source, bytes).unwrap();
    }
    let mut session = VNextSession::open(&fixture.manifest(), VNextPolicy::default()).unwrap();
    let image_revision = session.image_revision().to_owned();
    let summary = payload(call(
        &mut session,
        "image/explorer-summary",
        json!({
            "image_revision":image_revision,"mode":"overview"
        }),
    ));
    let handle = inventory(&summary, "declarations")["handle"]
        .as_str()
        .unwrap();
    let page = payload(call(
        &mut session,
        "image/explorer-page",
        json!({
            "image_revision":image_revision,"mode":"overview","view":"declarations","handle":handle,
            "page_size":128,"max_bytes":524288
        }),
    ));
    let same = page["items"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|row| row["display_name"] == "same")
        .collect::<Vec<_>>();
    assert_eq!(same.len(), 2);
    assert_eq!(same[0]["identity_origin"], "explicit");
    assert_eq!(same[1]["identity_origin"], "explicit");
    assert_ne!(same[0]["id"], same[1]["id"]);
    assert_ne!(same[0]["node_key"], same[1]["node_key"]);
    assert_ne!(same[0]["module"], same[1]["module"]);
    session.finish().unwrap();
}
