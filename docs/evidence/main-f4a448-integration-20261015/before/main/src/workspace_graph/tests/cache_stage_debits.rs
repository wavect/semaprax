//! Exact original Project cache cases, with test-only core debit attribution.
//! Trace rendering occurs after construction, outside the watched ledger.
#[path = "../../../tests/fixtures/checked_cache_profiles_old.rs"]
mod old_profiles;
#[path = "../../../tests/fixtures/checked_cache_profiles.rs"]
mod profiles;
use crate::bounded_output::stage_debits::{take_completed, Trace};
use crate::project::{
    with_authenticated_project, ProjectFrontendCache, ProjectFrontendSource, ProjectManifest,
    ProjectRevision, ProjectSemanticImage,
};
use crate::workspace_graph::identity_resolution;
use std::path::PathBuf;
use std::sync::{
    atomic::{AtomicU64, Ordering},
    Arc,
};
static SERIAL: AtomicU64 = AtomicU64::new(0);
struct Fixture(PathBuf);
impl Fixture {
    fn new(bytes: bool) -> Self {
        let root = std::env::temp_dir().join(format!(
            "spx-cache-core-stage-{}-{}",
            std::process::id(),
            SERIAL.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir_all(root.join("src")).unwrap();
        std::fs::write(
            root.join("semaprax.toml"),
            include_str!("../../../examples/frame-payload-project/semaprax.toml"),
        )
        .unwrap();
        for (path, source) in profiles::sources(bytes) {
            let source = if path == "src/app.spx" {
                source
            } else {
                let ast = crate::parse(&source, path).unwrap();
                crate::format::canonical(&ast)
            };
            std::fs::write(root.join(path), source).unwrap();
        }
        Self(root.canonicalize().unwrap())
    }
    fn manifest(&self) -> ProjectManifest {
        ProjectManifest::parse(&std::fs::read_to_string(self.0.join("semaprax.toml")).unwrap())
            .unwrap()
    }
    fn sources(&self, manifest: &ProjectManifest) -> Vec<ProjectFrontendSource> {
        manifest
            .sources()
            .iter()
            .map(|path| {
                ProjectFrontendSource::new(
                    path,
                    &std::fs::read_to_string(self.0.join(path)).unwrap(),
                )
                .unwrap()
            })
            .collect()
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}
fn trace() -> Trace {
    let trace = take_completed().expect("actual resolved core attempt");
    assert_eq!(trace.sum(), trace.consumed);
    trace
}
fn same(
    left: &Arc<ProjectRevision>,
    right: Arc<ProjectRevision>,
    checked: Trace,
    ordinary: Trace,
    warm: Option<Trace>,
) {
    assert_eq!(left.project_revision(), right.project_revision());
    assert_eq!(left.workspace_revision(), right.workspace_revision());
    assert_eq!(left.workspace_manifest(), right.workspace_manifest());
    assert_eq!(left.sources(), right.sources());
    // Keep the original complete equality. Counter mismatches remain failures;
    // the Copy trace only supplies exact phase receipts for the next repair.
    assert_eq!(
        left.semantic_graph(),
        right.semantic_graph(),
        "checked maximum core attempt {checked:?}; ordinary maximum core attempt {ordinary:?}; warm maximum core attempt {warm:?}"
    );
    let image = |revision: Arc<ProjectRevision>| {
        let expected = revision.project_revision().to_owned();
        ProjectSemanticImage::derive(revision, &expected).unwrap()
    };
    let left = image(Arc::clone(left));
    let right = image(right);
    assert_eq!(left.image_digest(), right.image_digest());
    assert_eq!(left.to_json(), right.to_json());
}
// Success also needs its actual ledger receipts: the published graph takes
// max(canonical source bytes, actual core consumption), so graph equality alone
// can conceal different successful stage debits. Render only after all three
// builds, outside their watched ledgers and any allocation observation.
fn print_receipt(bytes: bool, route: &str, trace: Trace) {
    eprintln!(
        "cache-core-receipt bytes={bytes} route={route} prebound={} layouts={} mode={} consumed={} sum={}",
        trace.prebound, trace.layouts, trace.mode, trace.consumed, trace.sum()
    );
    // Ordinal zero is unlabelled; n+1 follows ReservationStage's fixed enum
    // order, exactly as stage_debits::debit records it. Include zero debits.
    for (ordinal, debit) in trace.debits.into_iter().enumerate() {
        eprintln!("cache-core-stage bytes={bytes} route={route} tuple=({ordinal},{debit})");
    }
}
fn reproduce(bytes: bool) {
    // Independent saved construction canonicalizes the row fixture BEFORE
    // mixed-byte edits, exactly like the original integration producer.
    let canonical = |path: &str, source: &str| {
        if path == "src/app.spx" {
            source.to_owned()
        } else {
            crate::format::canonical(&crate::parse(source, path).unwrap())
        }
    };
    let current = profiles::sources(bytes);
    let saved = old_profiles::sources(bytes, canonical);
    for ((path, source), (old_path, old_source)) in current.iter().zip(&saved) {
        assert_eq!(*path, *old_path);
        assert_eq!(canonical(path, source), canonical(old_path, old_source));
    }
    let fixture = Fixture::new(bytes);
    let manifest = fixture.manifest();
    let mut cache = ProjectFrontendCache::new_with_semantic_cache();
    take_completed();
    let acquired_before = identity_resolution::acquisitions();
    let checked = cache.build(&manifest, &fixture.sources(&manifest)).unwrap();
    let checked_trace = trace();
    // Every cold module acquired its fixed identity report in the recorded
    // resolver interval (retries may acquire again, never fewer).
    let acquired_cold = identity_resolution::acquisitions();
    assert!(acquired_cold - acquired_before >= 3);
    let report: serde_json::Value = serde_json::from_str(checked.to_json()).unwrap();
    assert_eq!(report["work"]["modules_resolved"], 3);
    assert_eq!(report["work"]["checked_HIR_reused"], 0);
    take_completed();
    let ordinary = with_authenticated_project(&fixture.0.join("semaprax.toml"), |snapshot| {
        Ok(snapshot.retain_revision())
    })
    .unwrap();
    let ordinary_trace = trace();
    take_completed();
    let acquired_ordinary = identity_resolution::acquisitions();
    let warm = cache.build(&manifest, &fixture.sources(&manifest)).unwrap();
    let warm_trace = trace();
    // Exact hits validate and charge only the stored residual: no new
    // identity report is acquired and no index is rebuilt.
    assert_eq!(identity_resolution::acquisitions(), acquired_ordinary);
    let report: serde_json::Value = serde_json::from_str(warm.to_json()).unwrap();
    assert_eq!(report["work"]["modules_resolved"], 0);
    assert_eq!(report["work"]["checked_HIR_reused"], 3);
    print_receipt(bytes, "checked-cold", checked_trace);
    print_receipt(bytes, "ordinary", ordinary_trace);
    print_receipt(bytes, "checked-warm", warm_trace);
    same(
        checked.revision(),
        ordinary,
        checked_trace,
        ordinary_trace,
        Some(warm_trace),
    );
    same(
        warm.revision(),
        Arc::clone(checked.revision()),
        warm_trace,
        checked_trace,
        None,
    );
}
#[test]
fn original_row_loan_checked_cold_and_warm_preserve_ordinary_graph_with_exact_stage_debits() {
    reproduce(false);
}
#[test]
fn original_mixed_byte_traversal_checked_cold_and_warm_preserve_ordinary_graph_with_exact_stage_debits(
) {
    reproduce(true);
}
