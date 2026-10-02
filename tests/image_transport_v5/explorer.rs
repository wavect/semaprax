//! Compiler-owned explorer projection regressions.
use semaprax::image_transport::{VNextPolicy, VNextSession};
use semaprax::project::{
    ExplorerMode, ExplorerPageOptions, ExplorerQuery, ExplorerView, ProjectSemanticImage,
    with_authenticated_project,
};
use serde_json::{Value, json};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

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
    let root = payload(call(&mut session, "candidate/open", json!({"image_revision":image_revision})));
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
    assert!(
        call(&mut session, "candidate/explorer-page", wrong_side)
            .get("error")
            .is_some()
    );
    let mut wrong_cursor = page_params;
    wrong_cursor["cursor"] = json!(format!("{cursor}x"));
    assert!(
        call(&mut session, "candidate/explorer-page", wrong_cursor)
            .get("error")
            .is_some()
    );

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
    assert!(session.finish().is_err());
    assert_eq!(std::fs::read(&source).unwrap(), drifted);
    for (path, bytes) in disk {
        if path != "src/core.spx" {
            assert_eq!(std::fs::read(fixture.0.join(path)).unwrap(), bytes);
        }
    }
}
