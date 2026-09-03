//! Authored regression evidence; deliberately unrun locally.
use semaprax::image_transport::{VNextPolicy, VNextSession};
use semaprax::project::with_authenticated_project;
use serde_json::{json, Value};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

static SERIAL: AtomicU64 = AtomicU64::new(0);
struct Fixture(PathBuf);
impl Fixture {
    fn new() -> Self {
        let root = std::env::temp_dir().join(format!(
            "spx-live-cache-{}-{}",
            std::process::id(),
            SERIAL.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir_all(root.join("src")).unwrap();
        let original = Path::new(env!("CARGO_MANIFEST_DIR")).join("examples/calculator-project");
        for path in [
            "semaprax.toml",
            "src/app.spx",
            "src/core.spx",
            "src/tests.spx",
        ] {
            std::fs::copy(original.join(path), root.join(path)).unwrap();
        }
        Self(root.canonicalize().unwrap())
    }
    fn manifest(&self) -> PathBuf {
        self.0.join("semaprax.toml")
    }
    fn cached(&self) -> VNextSession {
        VNextSession::open_with_frontend_cache(&self.manifest(), VNextPolicy::default()).unwrap()
    }
    fn revision(&self) -> String {
        with_authenticated_project(&self.manifest(), |snapshot| {
            Ok(snapshot.project_revision().to_owned())
        })
        .unwrap()
    }
    fn replace(&self, path: &str, old: &str, new: &str) {
        let source = std::fs::read_to_string(self.0.join(path)).unwrap();
        assert!(source.contains(old));
        let ast = semaprax::parse(&source.replace(old, new), path).unwrap();
        std::fs::write(self.0.join(path), semaprax::format::canonical(&ast)).unwrap();
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}
fn call(session: &mut VNextSession, method: &str, mut params: Value) -> Value {
    if method.starts_with("workspace/refresh") {
        params["image_revision"] = json!(session.image_revision());
    }
    let frame = json!({"jsonrpc":"2.0","id":1,"method":method,"params":params}).to_string();
    serde_json::from_slice(&session.handle_frame(frame.as_bytes()).unwrap()).unwrap()
}
fn payload(result: Value) -> Value {
    assert!(result.get("error").is_none(), "{result}");
    result["result"]["payload"].clone()
}
fn refresh(session: &mut VNextSession, expected: &str) -> Value {
    payload(call(
        session,
        "workspace/refresh",
        json!({"expected_new_project_revision":expected}),
    ))
}
fn work(report: &Value, parsed: usize, reused: usize) {
    assert_eq!(
        report["frontend_work"]["schema"],
        "semaprax.project-frontend-cache-work.v1"
    );
    let facts = &report["frontend_work"]["work"];
    assert_eq!(facts["modules_parsed"], parsed);
    assert_eq!(facts["canonicalizer_calls"], parsed);
    assert_eq!(facts["modules_reused"], reused);
    assert_eq!(facts["modules_resolved"], 3);
    assert_eq!(facts["checked_HIR_reused"], 0);
    assert_eq!(facts["full_cross_file_checks"], true);
    assert_eq!(facts["full_link_and_profile_admission"], true);
}

#[test]
fn warm_refresh_avoids_parsing_and_keeps_cold_identity_and_discovery() {
    let fixture = Fixture::new();
    let mut cached = fixture.cached();
    let mut cold = VNextSession::open(&fixture.manifest(), VNextPolicy::default()).unwrap();
    assert_eq!(cached.image_revision(), cold.image_revision());
    for method in ["workspace/open", "protocol/capabilities"] {
        assert_eq!(
            call(&mut cached, method, json!({})),
            call(&mut cold, method, json!({}))
        );
    }
    let expected = fixture.revision();
    let report = refresh(&mut cached, &expected);
    work(&report, 0, 3);
    assert_eq!(report["image_arc_reused"], true);
    assert!(refresh(&mut cold, &expected).get("frontend_work").is_none());
    cached.finish().unwrap();
    cold.finish().unwrap();
}

#[test]
fn preview_and_wrong_expectation_do_not_prime_cache_or_revive_drift() {
    let fixture = Fixture::new();
    let mut session = fixture.cached();
    let original = fixture.revision();
    let old_image = session.image_revision().to_owned();
    fixture.replace("src/app.spx", "multiply(6, 7)", "multiply(6, 8)");
    assert!(call(&mut session, "workspace/status", json!({}))
        .get("error")
        .is_some());
    let preview = payload(call(&mut session, "workspace/refresh-preview", json!({})));
    work(&preview, 1, 2);
    assert_eq!(session.image_revision(), old_image);
    assert!(call(&mut session, "workspace/status", json!({}))
        .get("error")
        .is_some());
    assert!(call(
        &mut session,
        "workspace/refresh",
        json!({"expected_new_project_revision":original})
    )
    .get("error")
    .is_some());
    let expected = preview["observed_project_revision"].as_str().unwrap();
    work(&refresh(&mut session, expected), 1, 2);
    let cold = VNextSession::open(&fixture.manifest(), VNextPolicy::default()).unwrap();
    assert_eq!(session.image_revision(), cold.image_revision());
    work(&refresh(&mut session, expected), 0, 3);
    session.finish().unwrap();
}

#[test]
fn provider_changes_invalidate_consumers_and_failed_semantics_preserve_cache() {
    let fixture = Fixture::new();
    let mut session = fixture.cached();
    let original = std::fs::read_to_string(fixture.0.join("src/core.spx")).unwrap();
    fixture.replace(
        "src/core.spx",
        "fn add(left: i64, right: i64)",
        "fn add(left: i64, right: i64, extra: i64)",
    );
    assert!(call(&mut session, "workspace/refresh-preview", json!({}))
        .get("error")
        .is_some());
    std::fs::write(fixture.0.join("src/core.spx"), original).unwrap();
    work(&refresh(&mut session, &fixture.revision()), 0, 3);
    fixture.replace("src/core.spx", "left + right", "left + right + 1");
    let report = refresh(&mut session, &fixture.revision());
    work(&report, 3, 0);
    assert_eq!(
        report["frontend_work"]["invalidated_sources"],
        json!(["src/app.spx", "src/core.spx", "src/tests.spx"])
    );
    let cold = VNextSession::open(&fixture.manifest(), VNextPolicy::default()).unwrap();
    assert_eq!(session.image_revision(), cold.image_revision());
}

#[test]
fn source_exact_cache_hit_still_rejects_physical_hardlink_alias() {
    let fixture = Fixture::new();
    let mut session = fixture.cached();
    let expected = fixture.revision();
    let alias = fixture.0.join("alias.spx");
    std::fs::hard_link(fixture.0.join("src/app.spx"), &alias).unwrap();
    assert!(call(&mut session, "workspace/refresh-preview", json!({}))
        .get("error")
        .is_some());
    std::fs::remove_file(alias).unwrap();
    work(&refresh(&mut session, &expected), 0, 3);
    session.finish().unwrap();
}
