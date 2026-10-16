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
entry = "alias.current"
sources = ["src/app.spx", "src/tests.spx"]
tests = ["alias.tests"]

[exports]
web = ["alias.current.main"]

[dependencies]
std.collections = "=0.1.0"
"#;
    const TESTS: &str = r#"module alias.tests;
use function @id("alias.current.main") from alias.current as current;
@id("alias.tests.main") fn main()->i64 {if current()==42 {0} else {1}}
"#;
    let build = || {
        let canonical =
            |text: &str, path: &str| crate::format::canonical(&crate::parse(text, path).unwrap());
        let manifest = ProjectManifest::parse(MANIFEST).unwrap();
        let sources = vec![
            SemanticWorkspaceSource::new(
                "src/app.spx".to_owned(),
                canonical(CALLER, "src/app.spx"),
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
    let (old, old_borrowed) = super::parity_run(true, build);
    let (current, current_borrowed) = super::parity_run(false, build);
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
    assert_eq!(old.semantic_graph(), current.semantic_graph());
    assert_eq!(old.semantic_graph_digest(), current.semantic_graph_digest());
    let old_image = ProjectSemanticImage::derive(Arc::clone(&old), old.project_revision()).unwrap();
    let current_image =
        ProjectSemanticImage::derive(Arc::clone(&current), current.project_revision()).unwrap();
    assert_eq!(old_image.to_json(), current_image.to_json());
    assert_eq!(old_image.image_digest(), current_image.image_digest());
    eprintln!("847 actual borrowed Project parity entries={current_borrowed} graph={} image={} graph_bytes={} image_bytes={}",
        current.semantic_graph_digest(), current_image.image_digest(), current.semantic_graph().len(), current_image.to_json().len());
    drop((old_image, current_image, old, current));
}
