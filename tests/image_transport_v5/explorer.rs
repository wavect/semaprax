//! Compiler-owned explorer projection regressions.
use semaprax::image_transport::{VNextPolicy, VNextSession};
use semaprax::project::{
    with_authenticated_project, ExplorerMode, ExplorerPageOptions, ExplorerQuery, ExplorerView,
    ProjectSemanticImage,
};
use serde_json::{json, Value};
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
