//! Host-batched immutable read regressions, authored and deliberately unrun.
use semaprax::image_transport::{VNextPolicy, VNextSession, MAX_REQUEST_BYTES};
use serde_json::{json, Value};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

static SERIAL: AtomicU64 = AtomicU64::new(0);
struct Fixture(PathBuf);
impl Fixture {
    fn new() -> Self {
        let root = std::env::temp_dir().join(format!(
            "spx-parallel-reads-{}-{}",
            std::process::id(),
            SERIAL.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir_all(root.join("src")).unwrap();
        let example = Path::new(env!("CARGO_MANIFEST_DIR")).join("examples/calculator-project");
        for path in [
            "semaprax.toml",
            "src/app.spx",
            "src/core.spx",
            "src/tests.spx",
        ] {
            std::fs::copy(example.join(path), root.join(path)).unwrap();
        }
        Self(root.canonicalize().unwrap())
    }
    fn session(&self) -> VNextSession {
        VNextSession::open(
            &self.0.join("semaprax.toml"),
            VNextPolicy {
                candidate_prepare: true,
                diagnostics: true,
                ..Default::default()
            },
        )
        .unwrap()
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}
fn frame(id: u64, method: &str, params: Value) -> Vec<u8> {
    json!({"jsonrpc":"2.0", "id":id, "method":method, "params":params})
        .to_string()
        .into_bytes()
}
fn decoded(row: &Option<Vec<u8>>) -> Value {
    serde_json::from_slice(row.as_ref().unwrap()).unwrap()
}

#[test]
fn parallel_image_reads_equal_sequential_bytes_in_request_order() {
    let fixture = Fixture::new();
    let mut sequential = fixture.session();
    let mut parallel = fixture.session();
    let revision = parallel.image_revision();
    let requests = vec![
        frame(8, "workspace/open", json!({})),
        frame(
            3,
            "image/symbol",
            json!({"image_revision":revision,"stable_id":"calculator.add"}),
        ),
        frame(
            1,
            "image/function-summary",
            json!({"image_revision":revision,"target":"calculator.add"}),
        ),
        frame(
            7,
            "image/context",
            json!({"image_revision":revision,"target_kind":"declaration","target":"calculator.add","depth":2}),
        ),
        frame(
            2,
            "image/impact",
            json!({"image_revision":revision,"target_kind":"declaration","target":"calculator.add"}),
        ),
        frame(6, "protocol/capabilities", json!({})),
        frame(4, "protocol/instructions", json!({})),
        frame(
            5,
            "image/symbol",
            json!({"image_revision":revision,"stable_id":"unknown"}),
        ),
    ];
    let refs = requests.iter().map(Vec::as_slice).collect::<Vec<_>>();
    let expected = requests
        .iter()
        .map(|request| sequential.handle_frame(request))
        .collect::<Vec<_>>();
    for workers in [1, 2, 4] {
        let actual = parallel.handle_read_batch(&refs, workers).unwrap();
        assert_eq!(actual, expected);
    }
    assert!(parallel.parallel_read_methods().contains(&"image/facet"));
    assert!(!parallel
        .parallel_read_methods()
        .contains(&"workspace/refresh-preview"));
    parallel.finish().unwrap();
}

#[test]
fn batch_excludes_mutation_and_execution_even_when_session_has_candidate_grants() {
    let fixture = Fixture::new();
    let mut session = fixture.session();
    let revision = session.image_revision();
    let requests = [
        frame(1, "candidate/open", json!({"image_revision":revision})),
        frame(
            2,
            "workspace/refresh-preview",
            json!({"image_revision":revision}),
        ),
        frame(
            3,
            "workspace/refresh",
            json!({"image_revision":revision,"expected_new_project_revision":"ignored"}),
        ),
        frame(4, "candidate/test", json!({})),
        frame(5, "candidate/commit", json!({})),
        frame(6, "source-commit/status", json!({})),
    ];
    let rows = session
        .handle_read_batch(&requests.iter().map(Vec::as_slice).collect::<Vec<_>>(), 4)
        .unwrap();
    assert_eq!(rows.len(), requests.len());
    for row in rows {
        assert_eq!(decoded(&row)["error"]["code"], -32601);
    }
    // This is a method-specific host entry point, not a revocation or elevation
    // of the independently configured ordinary session.
    let ordinary = frame(
        7,
        "candidate/open",
        json!({"image_revision":session.image_revision()}),
    );
    let row = session.handle_frame(&ordinary);
    assert!(decoded(&row).get("result").is_some());
}

#[test]
fn drift_discards_entire_batch_and_remains_absorbing() {
    let fixture = Fixture::new();
    let mut session = fixture.session();
    let path = fixture.0.join("src/app.spx");
    let source = std::fs::read(&path).unwrap();
    std::fs::write(&path, b"invalid manual edit\n").unwrap();
    let request = frame(1, "workspace/status", json!({}));
    assert!(session
        .handle_read_batch(&[request.as_slice(), request.as_slice()], 2)
        .is_err());
    std::fs::write(&path, &source).unwrap();
    assert!(session.handle_read_batch(&[request.as_slice()], 1).is_err());
    assert!(session.finish().is_err());
}

#[test]
fn batch_bounds_notifications_and_startup_approval_boundary() {
    let fixture = Fixture::new();
    let mut session = fixture.session();
    let request = frame(1, "workspace/status", json!({}));
    for workers in [0, 5] {
        assert_eq!(
            session
                .handle_read_batch(&[request.as_slice()], workers)
                .unwrap_err()[0]
                .code,
            "SPX-G294"
        );
    }
    assert!(session.handle_read_batch(&[], 1).is_err());
    assert!(session
        .handle_read_batch(&vec![request.as_slice(); 17], 1)
        .is_err());
    let oversized = vec![b' '; MAX_REQUEST_BYTES + 1];
    assert_eq!(
        session
            .handle_read_batch(&[oversized.as_slice()], 1)
            .unwrap_err()[0]
            .code,
        "SPX-G294"
    );
    let notification = br#"{"jsonrpc":"2.0","method":"candidate/open","params":{}}"#;
    let rows = session.handle_read_batch(&[notification, b""], 2).unwrap();
    assert_eq!(rows, vec![None, None]);
    let errors = session.approve_git_commit("not-an-approval").unwrap_err();
    assert!(errors[0].message.contains("precede the first frame"));
    let malformed = session.handle_read_batch(&[b"{"], 1).unwrap();
    assert_eq!(decoded(&malformed[0])["error"]["code"], -32700);
    assert!(session.handle_frame(&request).is_some());
}
