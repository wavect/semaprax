use super::*;
use crate::bounded_output::with_limit_usage_refusal_site;
use crate::test_allocation_observer::measure;
const PROVIDER: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/std/collections/src/collections.spx"
));
const CALLER: &str = r#"module alias.current;
use function @id("std.collections.vec.with-capacity") from std.collections as with_capacity;
use function @id("std.collections.vec.push") from std.collections as push;
use function @id("std.collections.vec.get") from std.collections as get;
@id("alias.current.main") fn main()->i64 {let values=with_capacity<i64>(2usize);let appended=push<i64>(values,42);get<i64>(appended,0usize)}"#;
#[test]
fn actual_uncached_core_old_and_current_include_new_entry_controls_and_all_drop() {
    let programs = [
        crate::parse(CALLER, "caller.spx").unwrap(),
        crate::parse(PROVIDER, "provider.spx").unwrap(),
    ];
    let authored = index_authored(&programs).unwrap();
    let module_paths = index_modules(&programs).unwrap();
    let depths = expected_projection::validate_dependency_dag(&programs).unwrap();
    let order = [0, 1];
    let observe = |current, limit| {
        with_limit_usage_refusal_site(limit, || {
            measure(|| {
                let layouts = expected_projection::private_layout::registry::LayoutRegistry::new(
                    &programs, &authored,
                );
                let result = if current {
                    crate::workspace_graph::build_resolved_core_current(
                        &programs,
                        &module_paths,
                        &depths,
                        &authored,
                        None,
                        None,
                        &layouts,
                        Some(&order),
                    )
                } else {
                    super::build_resolved_core_current(
                        &programs,
                        &module_paths,
                        &depths,
                        &authored,
                        None,
                        None,
                        &layouts,
                        Some(&order),
                    )
                };
                let success = result.is_ok();
                drop(result);
                drop(layouts);
                success
            })
        })
    };
    // Common process prelude initialization uses the complete immediate old
    // core, not a shadow schema; both paired observations then share this state.
    assert!(observe(false, MAX_BUILDER_BYTES).0 .0);
    let old = observe(false, MAX_BUILDER_BYTES);
    let current = observe(true, MAX_BUILDER_BYTES);
    assert!(old.0 .0 && current.0 .0 && !old.1 && !current.1);
    for result in [&old, &current] {
        let m = &result.0 .1;
        assert!(!m.invalid && m.live == 0 && m.allocated == m.freed);
    }
    eprintln!(
        "847 complete uncached core old paid={} physical={:?}; current paid={} physical={:?}",
        old.2, old.0 .1, current.2, current.0 .1
    );
    // No inferred 19,422-byte saving, universal improvement or application fit.
    for limit in [0, current.2 - 1, current.2] {
        let result = observe(true, limit);
        eprintln!(
            "847 outer probe limit={limit} internal_ok={} overflowed={} paid={} first={:?} site={:?}",
            result.0 .0, result.1, result.2, result.3, result.4
        );
        // The public caller rejects an overflowing phase even when its last
        // budgeted clone leaves an internal Ok containing an empty string.
        let accepted = result.0 .0 && !result.1;
        assert_eq!(accepted, limit == current.2);
        if limit == current.2 - 1 {
            assert!(result.1, "the one-short invocation must stay refused");
        }
        if limit == current.2 {
            assert!(!result.1 && result.3.is_none() && result.4.is_none());
        }
        // Preserve optional receipt/site evidence as observed. A formatter
        // error can mark overflow without selecting a reservation receipt.
        let m = result.0 .1;
        assert!(!m.invalid && m.live == 0 && m.allocated == m.freed);
    }
}

#[test]
fn actual_borrowed_alias_project_keeps_complete_sources_graph_and_image_of_old_core() {
    use crate::project::{ProjectManifest, ProjectSemanticImage};
    use crate::semantic_workspace::SemanticWorkspaceSource;
    use std::sync::Arc;
    const MANIFEST: &str = r#"schema = "semaprax.manifest.v1"

[package]
name = "alias-parity"
version = "1.0.0"

[modules]
entry = "alias.app"
sources = ["src/app.spx", "src/provider.spx", "src/tests.spx"]
tests = ["alias.tests"]

[exports]
web = ["alias.current.value"]

[dependencies]
std.collections = "=0.1.0"
"#;
    // Entry and test roots import a shared provider that declares no main,
    // matching the admitted three-module calculator Project shape.
    const APP: &str = r#"module alias.app;
use function @id("alias.current.value") from alias.current as current;
@id("alias.app.main") fn main()->i64 {current()}
"#;
    const APP_PROVIDER: &str = r#"module alias.current;
use function @id("std.collections.vec.with-capacity") from std.collections as with_capacity;
use function @id("std.collections.vec.push") from std.collections as push;
use function @id("std.collections.vec.get") from std.collections as get;
@id("alias.current.value") fn value()->i64 {let values=with_capacity<i64>(2usize);let appended=push<i64>(values,42);get<i64>(appended,0usize)}"#;
    const TESTS: &str = r#"module alias.tests;
use function @id("alias.current.value") from alias.current as current;
@id("alias.tests.main") fn main()->i64 {if current()==42 {0} else {1}}
"#;
    let build = || {
        let canonical =
            |text: &str, path: &str| crate::format::canonical(&crate::parse(text, path).unwrap());
        // The payload instantiates Vec<T>; select its existing owned-data
        // Project admission rather than the intentionally scalar-only default.
        let owned_manifest = MANIFEST.replacen(
            "version = \"1.0.0\"\n",
            "version = \"1.0.0\"\nprofile = \"owned-data-api.v1\"\n",
            1,
        );
        let manifest = ProjectManifest::parse(&owned_manifest).unwrap();
        let sources = vec![
            SemanticWorkspaceSource::new("src/app.spx".to_owned(), canonical(APP, "src/app.spx")),
            SemanticWorkspaceSource::new(
                "src/provider.spx".to_owned(),
                canonical(APP_PROVIDER, "src/provider.spx"),
            ),
            SemanticWorkspaceSource::new(
                "src/tests.spx".to_owned(),
                canonical(TESTS, "src/tests.spx"),
            ),
        ];
        Arc::new(crate::project::rebuild_owned_revision(manifest, sources).unwrap())
    };
    // No preparatory build/cache warming: each side performs its actual full
    // Project preflight, core selection, source retention, graph and admission.
    let ((old, old_borrowed), old_components) =
        super::component_run(|| super::parity_run(true, build));
    let ((current, current_borrowed), current_components) =
        super::component_run(|| super::parity_run(false, build));
    assert_eq!(old_borrowed, 0);
    assert!(
        current_borrowed > 0,
        "fallback is not evidence of the optimized borrowed-alias route"
    );
    assert_eq!(
        old.manifest().to_canonical_toml(),
        current.manifest().to_canonical_toml()
    );
    assert_eq!(old.workspace_manifest(), current.workspace_manifest());
    assert_eq!(old.workspace_revision(), current.workspace_revision());
    assert_eq!(old.project_revision(), current.project_revision());
    let facts = |revision: &crate::project::ProjectRevision| {
        revision
            .sources()
            .iter()
            .map(|source| {
                (
                    source.path().to_owned(),
                    source.source().to_owned(),
                    source.source_graph_schema().to_owned(),
                    source.source_revision().to_owned(),
                    source.source_digest().to_owned(),
                )
            })
            .collect::<Vec<_>>()
    };
    assert_eq!(facts(&old), facts(&current));
    // The graph owns its real resource usage. Calibrate the complete changed
    // constructor/retention component independently of that published total;
    // every debit outside it must remain exactly equal. No field is discarded.
    assert_eq!(old_components.len, current_components.len);
    let mut old_core = 0usize;
    let mut current_core = 0usize;
    for (before, after) in old_components.rows[..old_components.len]
        .iter()
        .zip(&current_components.rows[..current_components.len])
    {
        assert_eq!(before.inputs, 4, "all four real authenticated modules");
        assert_eq!(
            (before.inputs, before.identity, before.compact),
            (after.inputs, after.identity, after.compact)
        );
        assert!(before.finished && after.finished);
        old_core = old_core.checked_add(before.bytes).unwrap();
        current_core = current_core.checked_add(after.bytes).unwrap();
    }
    let old_graph: serde_json::Value = serde_json::from_str(old.semantic_graph()).unwrap();
    let current_graph: serde_json::Value = serde_json::from_str(current.semantic_graph()).unwrap();
    let old_used =
        usize::try_from(old_graph["budget"]["used_builder_bytes"].as_u64().unwrap()).unwrap();
    let current_used = usize::try_from(
        current_graph["budget"]["used_builder_bytes"]
            .as_u64()
            .unwrap(),
    )
    .unwrap();
    let outside = old_used.checked_sub(old_core).unwrap();
    assert_eq!(current_used.checked_sub(current_core), Some(outside));
    assert!(
        current_core < old_core,
        "the real borrowed-alias fixture must save complete component debits"
    );
    let expected_used = outside.checked_add(current_core).unwrap();
    let expected_graph = calibrated_graph(
        old.semantic_graph(),
        old.semantic_graph_digest(),
        old_used,
        expected_used,
    );
    assert_eq!(expected_graph.0, current.semantic_graph());
    assert_eq!(expected_graph.1, current.semantic_graph_digest());
    let roundtrip = calibrated_graph(
        current.semantic_graph(),
        current.semantic_graph_digest(),
        current_used,
        old_used,
    );
    assert_eq!(roundtrip.0, old.semantic_graph());
    assert_eq!(roundtrip.1, old.semantic_graph_digest());
    let old_image = ProjectSemanticImage::derive(Arc::clone(&old), old.project_revision()).unwrap();
    let current_image =
        ProjectSemanticImage::derive(Arc::clone(&current), current.project_revision()).unwrap();
    let expected_image =
        calibrated_image(old_image.to_json(), &expected_graph.0, &expected_graph.1);
    assert_eq!(expected_image.0, current_image.to_json());
    assert_eq!(expected_image.1, current_image.image_digest());
    let old_roundtrip = calibrated_image(
        current_image.to_json(),
        old.semantic_graph(),
        old.semantic_graph_digest(),
    );
    assert_eq!(old_roundtrip.0, old_image.to_json());
    assert_eq!(old_roundtrip.1, old_image.image_digest());
    eprintln!("847 independent full-core component old/current={old_core}/{current_core} unchanged_outside={outside} published_old/current={old_used}/{current_used}; measured component equation, no application-fit or shipping-allocation claim");
    eprintln!("847 actual borrowed Project parity entries={current_borrowed} graph={} image={} graph_bytes={} image_bytes={}",
        current.semantic_graph_digest(), current_image.image_digest(), current.semantic_graph().len(), current_image.to_json().len());
    drop((old_image, current_image, old, current));

    // Replay the complete original scalar fixture only AFTER both measured
    // positive builds. This cannot warm their actual Project acquisition path.
    let canonical =
        |text: &str, path: &str| crate::format::canonical(&crate::parse(text, path).unwrap());
    let scalar_manifest = ProjectManifest::parse(MANIFEST).unwrap();
    let scalar_sources = vec![
        SemanticWorkspaceSource::new("src/app.spx".to_owned(), canonical(APP, "src/app.spx")),
        SemanticWorkspaceSource::new(
            "src/provider.spx".to_owned(),
            canonical(APP_PROVIDER, "src/provider.spx"),
        ),
        SemanticWorkspaceSource::new(
            "src/tests.spx".to_owned(),
            canonical(TESTS, "src/tests.spx"),
        ),
    ];
    let errors = crate::project::rebuild_owned_revision(scalar_manifest, scalar_sources)
        .err()
        .expect("the original scalar profile must keep rejecting generic templates");
    assert_eq!(errors.len(), 1);
    let expected = crate::diagnostic::Diagnostic::io(
        "SPX-W115",
        "Public Scalar Export Profile v1 does not admit generic function templates or instances",
    );
    assert_eq!(errors[0].code, expected.code);
    assert_eq!(errors[0].severity, expected.severity);
    assert_eq!(errors[0].message, expected.message);
    assert_eq!(errors[0].path, expected.path);
    assert_eq!(errors[0].span, expected.span);
    assert_eq!(errors[0].help, expected.help);
}

// The exact compiler graph ordering is retained, including every budget field.
// Only the independently calibrated total changes; the ordinary digest domain
// rebinds the complete resulting payload. No semantic field is normalized.
fn calibrated_graph(graph: &str, digest: &str, old: usize, new: usize) -> (String, String) {
    let identity = format!(",\"graph_digest\":\"{digest}\"");
    assert_eq!(graph.matches(&identity).count(), 1);
    let payload = graph.replacen(&identity, "", 1);
    assert_eq!(
        project_artifact_digest(PROJECT_GRAPH_DIGEST_DOMAIN, payload.as_bytes()),
        digest
    );
    let before = format!("\"used_builder_bytes\":{old},");
    let after = format!("\"used_builder_bytes\":{new},");
    assert_eq!(payload.matches(&before).count(), 1);
    let expected_payload = payload.replacen(&before, &after, 1);
    let expected_digest =
        project_artifact_digest(PROJECT_GRAPH_DIGEST_DOMAIN, expected_payload.as_bytes());
    let expected = graph.replacen(&before, &after, 1).replacen(
        &identity,
        &format!(",\"graph_digest\":\"{expected_digest}\""),
        1,
    );
    (expected, expected_digest)
}
fn calibrated_image(image: &str, graph: &str, digest: &str) -> (String, String) {
    let mut expected: serde_json::Value = serde_json::from_str(image).unwrap();
    expected["project_graph"] = serde_json::from_str(graph).unwrap();
    expected["project_graph_digest"] = digest.into();
    // Match the existing image renderer's deterministic complete object order.
    expected.sort_all_objects();
    let expected = serde_json::to_string(&expected).unwrap() + "\n";
    let identity = project_artifact_digest(
        b"semaprax.semantic-workspace-image.digest.v1\0",
        expected.as_bytes(),
    );
    (expected, identity)
}
