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
    let cached_ordinary = identity_resolution::cached_prelude_acquisitions();
    let warm = cache.build(&manifest, &fixture.sources(&manifest)).unwrap();
    let warm_trace = trace();
    // Exact hits still acquire their actual cached-prelude report receivers.
    // Excluding only that independently classified acquisition preserves the
    // original no-fresh-resolution/index predicate; the total stays truthful.
    let cached_warm = identity_resolution::cached_prelude_acquisitions();
    assert_eq!(cached_warm - cached_ordinary, 3);
    assert_eq!(
        identity_resolution::acquisitions() - cached_warm,
        acquired_ordinary - cached_ordinary
    );
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

#[test]
fn no_possible_wrapper_routes_are_paid_symmetric_and_keep_complete_results_and_drop() {
    use crate::test_allocation_observer::measure;
    use crate::workspace_graph::constructor_failure::typed::{
        may_borrow_wrapper, with_original_alias_dispatch,
    };
    // This is an allocation observation of the raw predicate, not a funded
    // receiver claim. The production selector pays before calling it.
    let (_, predicate) = measure(|| {
        for id in [
            "",
            "loan.weight",
            "frame.status",
            "std.collections.vec",
            "std.mem.box",
        ] {
            assert!(!may_borrow_wrapper(id));
        }
        for op in crate::vec_ops::ALL {
            if let Some(id) = crate::vec_ops::wrapper_id(op) {
                assert!(may_borrow_wrapper(id));
            }
        }
        for op in crate::box_ops::ALL {
            assert!(may_borrow_wrapper(crate::box_ops::wrapper_id(op)));
        }
        // False positives confer no wrapper or provider authority.
        assert!(may_borrow_wrapper("std.collections.vec.not-a-wrapper"));
    });
    assert_eq!(predicate.allocated, 0);
    assert_eq!(predicate.freed, 0);
    assert_eq!(predicate.live, 0);
    assert!(!predicate.invalid);
    for bytes in [false, true] {
        let fixture = Fixture::new(bytes);
        let manifest = fixture.manifest();
        // Common process-static initialization and saved reference roots are
        // outside both complete construction-through-Drop observations.
        let expected = with_original_alias_dispatch(|| {
            with_authenticated_project(&fixture.0.join("semaprax.toml"), |snapshot| {
                Ok(snapshot.retain_revision())
            })
            .unwrap()
        });
        let image = |revision: Arc<ProjectRevision>| {
            let revision_id = revision.project_revision().to_owned();
            ProjectSemanticImage::derive(revision, &revision_id).unwrap()
        };
        let expected_image = image(Arc::clone(&expected));
        let observe = |saved: bool| {
            let run = || {
                measure(|| {
                    take_completed();
                    let direct =
                        with_authenticated_project(&fixture.0.join("semaprax.toml"), |snapshot| {
                            Ok(snapshot.retain_revision())
                        })
                        .unwrap();
                    let direct_trace = trace();
                    let mut cache = ProjectFrontendCache::new_with_semantic_cache();
                    let cold = cache.build(&manifest, &fixture.sources(&manifest)).unwrap();
                    let cold_trace = trace();
                    let warm = cache.build(&manifest, &fixture.sources(&manifest)).unwrap();
                    let warm_trace = trace();
                    let cold_work: serde_json::Value =
                        serde_json::from_str(cold.to_json()).unwrap();
                    let warm_work: serde_json::Value =
                        serde_json::from_str(warm.to_json()).unwrap();
                    assert_eq!(cold_work["work"]["modules_resolved"], 3);
                    assert_eq!(cold_work["work"]["checked_HIR_reused"], 0);
                    assert_eq!(warm_work["work"]["modules_resolved"], 0);
                    assert_eq!(warm_work["work"]["checked_HIR_reused"], 3);
                    for revision in [&direct, cold.revision(), warm.revision()] {
                        assert_eq!(revision.sources(), expected.sources());
                        assert_eq!(revision.workspace_manifest(), expected.workspace_manifest());
                        assert_eq!(revision.workspace_revision(), expected.workspace_revision());
                        assert_eq!(revision.project_revision(), expected.project_revision());
                        assert_eq!(revision.entry_program(), expected.entry_program());
                        assert_eq!(revision.public_api_program(), expected.public_api_program());
                        assert_eq!(revision.test_program(), expected.test_program());
                    }
                    let direct_image = image(Arc::clone(&direct));
                    let cold_image = image(Arc::clone(cold.revision()));
                    let warm_image = image(Arc::clone(warm.revision()));
                    let equal = direct.semantic_graph() == cold.revision().semantic_graph()
                        && direct.semantic_graph() == warm.revision().semantic_graph()
                        && direct_image.to_json() == cold_image.to_json()
                        && direct_image.to_json() == warm_image.to_json();
                    if !saved {
                        // Whole current wire equality remains strict. Both measured
                        // arms construct the same three complete Images through Drop.
                        assert_eq!(direct.semantic_graph(), cold.revision().semantic_graph());
                        assert_eq!(direct.semantic_graph(), warm.revision().semantic_graph());
                        assert_eq!(direct_image.to_json(), cold_image.to_json());
                        assert_eq!(direct_image.to_json(), warm_image.to_json());
                        assert_eq!(direct_image.image_digest(), cold_image.image_digest());
                        assert_eq!(direct_image.image_digest(), warm_image.image_digest());
                    }
                    let result = (
                        equal,
                        direct_trace.consumed,
                        cold_trace.consumed,
                        warm_trace.consumed,
                    );
                    drop((
                        cold_work,
                        warm_work,
                        direct_image,
                        cold_image,
                        warm_image,
                        direct,
                        cold,
                        warm,
                        cache,
                    ));
                    result
                })
            };
            if saved {
                with_original_alias_dispatch(run)
            } else {
                run()
            }
        };
        let old = observe(true);
        let current = observe(false);
        assert!(current.0 .0);
        for metrics in [old.1, current.1] {
            assert!(!metrics.invalid);
            assert_eq!(metrics.live, 0);
            assert_eq!(metrics.allocated, metrics.freed);
        }
        // All old counters and unfavorable controls remain visible. Removing
        // one directory is not a claim about total Project allocator savings.
        eprintln!("no-wrapper whole-project bytes={bytes} old={old:?} current={current:?}");
        drop((expected_image, expected));
        // A reserved-looking foreign identity must still take the full ordinary
        // semantic checks; the prefix predicate cannot admit a malformed wrapper.
        let tests_path = fixture.0.join("src/tests.spx");
        let provider_path = fixture.0.join("src/frame.spx");
        let tests = std::fs::read_to_string(&tests_path)
            .unwrap()
            .replace("loan.weight", "std.collections.vec.push");
        let provider = std::fs::read_to_string(&provider_path)
            .unwrap()
            .replace("loan.weight", "std.collections.vec.push");
        std::fs::write(&tests_path, tests).unwrap();
        std::fs::write(&provider_path, provider).unwrap();
        let errors = |saved: bool, checked: bool| {
            let run = || {
                if checked {
                    ProjectFrontendCache::new_with_semantic_cache()
                        .build(&manifest, &fixture.sources(&manifest))
                        .err()
                        .unwrap()
                } else {
                    with_authenticated_project(&fixture.0.join("semaprax.toml"), |snapshot| {
                        Ok(snapshot.retain_revision())
                    })
                    .err()
                    .unwrap()
                }
            };
            if saved {
                with_original_alias_dispatch(run)
            } else {
                run()
            }
        };
        let old = errors(true, false);
        assert!(!old.is_empty());
        assert_eq!(errors(false, false), old);
        assert_eq!(errors(false, true), old);
    }
}
