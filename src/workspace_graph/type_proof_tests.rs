use super::*;

fn canonical_source(path: &str, source: &str) -> WorkspaceSource {
    let program = parse(source, Path::new(path)).expect("type-proof fixture must parse");
    WorkspaceSource {
        path: path.to_owned(),
        source: format::canonical(&program),
    }
}

fn point_provider() -> WorkspaceSource {
    canonical_source(
        "lib/core.spx",
        r#"
module lib.core;

@id("lib.point")
record Point {
    @id("lib.point.x")
    x: i64,
}

@id("lib.local")
fn local(value: Point) -> Point { value }
"#,
    )
}

#[cfg(test)]
mod projection_tests {
    use super::*;
    use std::fs::OpenOptions;
    use std::sync::atomic::{AtomicU64, Ordering};

    static SERIAL: AtomicU64 = AtomicU64::new(0);

    struct ManagedFixture {
        root: std::path::PathBuf,
        revision: String,
    }

    impl ManagedFixture {
        fn new(label: &str) -> Self {
            let serial = SERIAL.fetch_add(1, Ordering::Relaxed);
            let root = std::env::temp_dir().join(format!(
                "semaprax-workspace-graph-projection-{label}-{}-{serial}",
                std::process::id()
            ));
            std::fs::create_dir(&root).unwrap();
            for (path, module, id) in [
                ("alpha.spx", "workspace.alpha", "workspace.alpha.run"),
                ("beta.spx", "workspace.beta", "workspace.beta.run"),
            ] {
                let source = canonical_source(
                    path,
                    &format!(
                        "module {module}; @id(\"{id}\") fn run()->i64{{0}} fn main()->i64{{run()}}"
                    ),
                );
                std::fs::write(root.join(path), source.source).unwrap();
            }
            let path_set = root.join("paths.json");
            std::fs::write(
                    &path_set,
                    "{\"schema\":\"semaprax.workspace-semantic-path-set.v1\",\"files\":[{\"path\":\"alpha.spx\"},{\"path\":\"beta.spx\"}]}\n",
                )
                .unwrap();
            let revision =
                crate::semantic_workspace::initialize_from_preflight(&root, &path_set).unwrap();
            Self { root, revision }
        }

        fn control(&self) -> std::path::PathBuf {
            self.root.join(".semaprax-workspace")
        }

        fn generation(&self) -> std::path::PathBuf {
            self.control().join("generations").join(
                self.revision
                    .strip_prefix("sha256:")
                    .expect("workspace revision must be a canonical digest"),
            )
        }

        fn reacquire_exclusive(&self) {
            let lock = OpenOptions::new()
                .read(true)
                .write(true)
                .open(self.control().join("LOCK"))
                .unwrap();
            fs2::FileExt::try_lock_exclusive(&lock).unwrap();
            fs2::FileExt::unlock(&lock).unwrap();
        }
    }

    impl Drop for ManagedFixture {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.root);
        }
    }

    fn canonical_source(path: &str, source: &str) -> WorkspaceSource {
        let program = parse(source, Path::new(path)).expect("projection source must parse");
        WorkspaceSource {
            path: path.to_owned(),
            source: format::canonical(&program),
        }
    }

    fn projection_sources() -> Vec<WorkspaceSource> {
        vec![
            canonical_source(
                "z/entry.spx",
                r#"
module app.entry;
use function @id("provider.run") from provider.api as provider_run;

@id("app.entry.run")
fn run() -> i64 { provider_run() }
"#,
            ),
            canonical_source(
                "m/provider.spx",
                r#"
module provider.api;
use type @id("types.point") from types.core as Point;

@id("provider.run")
fn run() -> i64 { 7 }
"#,
            ),
            canonical_source(
                "a/types.spx",
                r#"
module types.core;

@id("types.point")
record Point { @id("types.point.x") x: i64, }

@id("types.local")
fn local() -> i64 { 0 }
"#,
            ),
            canonical_source(
                "q/disconnected.spx",
                r#"
module disconnected.reverse;
use function @id("app.entry.run") from app.entry as entry_run;

@id("disconnected.run")
fn run() -> i64 { entry_run() }
"#,
            ),
        ]
    }

    fn authenticated_build() -> AuthenticatedWorkspaceGraphBuild {
        let sources = projection_sources();
        authenticated_build_from(sources)
    }

    fn authenticated_build_from(sources: Vec<WorkspaceSource>) -> AuthenticatedWorkspaceGraphBuild {
        let source_facts = sources
            .iter()
            .map(|source| {
                (
                    source.path.clone(),
                    AuthenticatedSourceFact {
                        path: source.path.clone(),
                        source_graph_schema: "semaprax.semantic-graph.v14".to_owned(),
                        source_revision: format!("revision:{}", source.path),
                        source_digest: format!("sha256:{:064x}", source.source.len()),
                    },
                )
            })
            .collect();
        AuthenticatedWorkspaceGraphBuild {
            workspace_revision: "sha256:workspace".to_owned(),
            sources: source_facts,
            storage: AuthenticatedWorkspaceStorageUsage {
                manifest_bytes: 711,
                retained_generations: 3,
                staging_attempts: 2,
                unexpected_inventory_entries: 0,
            },
            graph: build_owned(sources).expect("full workspace must validate before projection"),
        }
    }

    fn diamond_sources() -> Vec<WorkspaceSource> {
        vec![
            canonical_source(
                "z/entry.spx",
                r#"
module diamond.entry;
use function @id("diamond.left") from diamond.left as left;
use function @id("diamond.right") from diamond.right as right;

@id("diamond.main")
fn main() -> i64 { left() + right() }
"#,
            ),
            canonical_source(
                "b/left.spx",
                r#"
module diamond.left;
use function @id("diamond.leaf") from diamond.leaf as leaf;

@id("diamond.left")
fn left() -> i64 { leaf() }
"#,
            ),
            canonical_source(
                "c/right.spx",
                r#"
module diamond.right;
use function @id("diamond.leaf") from diamond.leaf as leaf;

@id("diamond.right")
fn right() -> i64 { leaf() }
"#,
            ),
            canonical_source(
                "a/leaf.spx",
                r#"
module diamond.leaf;

@id("diamond.leaf")
fn leaf() -> i64 { 1 }
"#,
            ),
            canonical_source(
                "y/reverse.spx",
                r#"
module diamond.reverse;
use function @id("diamond.main") from diamond.entry as entry;

@id("diamond.reverse")
fn reverse() -> i64 { entry() }
"#,
            ),
            canonical_source(
                "x/disconnected.spx",
                r#"
module diamond.disconnected;

@id("diamond.disconnected")
fn disconnected() -> i64 { 0 }
"#,
            ),
        ]
    }

    fn rendered_entry() -> WorkspaceSemanticGraph {
        render_semantic_graph(authenticated_build().project("app.entry").unwrap())
            .expect("authenticated projection must render")
    }

    fn assert_ordered_fragments(document: &str, fragments: &[&str]) {
        let mut remainder = document;
        for fragment in fragments {
            let offset = remainder
                .find(fragment)
                .unwrap_or_else(|| panic!("missing ordered wire fragment `{fragment}`"));
            remainder = &remainder[offset + fragment.len()..];
        }
    }

    #[test]
    fn entry_projection_is_forward_provider_only_sorted_and_authenticated() {
        let projection = authenticated_build().project("app.entry").unwrap();
        assert_eq!(projection.workspace_revision(), "sha256:workspace");
        assert_eq!(projection.entry_module(), "app.entry");
        assert_eq!(
            projection
                .modules()
                .iter()
                .map(WorkspaceGraphProjectionModule::path)
                .collect::<Vec<_>>(),
            ["a/types.spx", "m/provider.spx", "z/entry.spx"]
        );
        assert!(projection
            .modules()
            .iter()
            .all(|module| module.source_graph_schema() == "semaprax.semantic-graph.v14"));
        assert!(projection
            .modules()
            .iter()
            .all(|module| module.source_revision().starts_with("revision:")));
        assert!(projection
            .modules()
            .iter()
            .all(|module| module.source_digest().starts_with("sha256:")));
        assert!(projection
            .declarations()
            .windows(2)
            .all(|pair| pair[0].id() < pair[1].id()));
        assert!(projection
            .declarations()
            .iter()
            .all(|fact| fact.path() != Some("q/disconnected.spx")));
        assert!(projection
            .shared_prelude_ids()
            .contains(&prelude::OPTION_ID));
        assert!(projection
            .edges()
            .iter()
            .all(|edge| edge.caller_path() != "q/disconnected.spx"));
        assert!(projection.edges().windows(2).all(|pair| pair[0] < pair[1]));

        let usage = projection.usage();
        assert_eq!(usage.used_managed_files(), 4);
        assert_eq!(usage.used_reachable_modules(), 3);
        assert_eq!(usage.used_manifest_bytes(), 711);
        assert_eq!(usage.used_retained_generations(), 3);
        assert_eq!(usage.used_staging_attempts(), 2);
        assert_eq!(usage.used_unexpected_inventory_entries(), 0);
        assert!(usage.used_total_source_bytes() > 0);
        assert!(usage.used_declarations() > 0);
        assert!(usage.used_callables() > 0);
        assert!(usage.used_call_sites() > 0);
        assert_eq!(usage.used_uses(), 3);
        assert!(usage.used_resolved_cross_file_edges() >= 3);
        assert!(usage.used_dependency_depth() >= 2);
        assert!(usage.used_builder_bytes() >= usage.used_total_source_bytes());
    }

    #[test]
    fn entry_need_not_define_main_and_reverse_consumers_are_excluded() {
        let projection = authenticated_build().project("types.core").unwrap();
        assert_eq!(projection.modules().len(), 1);
        assert_eq!(projection.modules()[0].module(), "types.core");
        assert!(projection.modules()[0]
            .functions()
            .iter()
            .all(|function| function.name != "main"));
        assert!(projection
            .declarations()
            .iter()
            .any(|fact| fact.id() == "types.point"));
        assert!(!projection
            .declarations()
            .iter()
            .any(|fact| fact.id() == "provider.run"));
    }

    #[test]
    fn entry_spelling_and_absence_have_frozen_diagnostics() {
        for entry in ["", ".app", "app.", "app-main", "App.µ"] {
            let error = authenticated_build()
                .project(entry)
                .err()
                .expect("noncanonical entry must fail");
            assert_eq!(error[0].code, "SPX-G170");
            assert_eq!(
                error[0].message,
                format!("Workspace Semantic Graph entry module `{entry}` is not canonical")
            );
        }
        let error = authenticated_build()
            .project("missing.module")
            .err()
            .expect("absent canonical entry must fail");
        assert_eq!(error[0].code, "SPX-G172");
        assert_eq!(
            error[0].message,
            "Workspace Semantic Graph entry module `missing.module` is absent"
        );
    }

    #[test]
    fn diamond_projection_has_exact_provider_closure_order_and_full_usage() {
        let projection = authenticated_build_from(diamond_sources())
            .project("diamond.entry")
            .unwrap();
        assert_eq!(
            projection
                .modules()
                .iter()
                .map(|module| (module.path(), module.dependency_depth()))
                .collect::<Vec<_>>(),
            [
                ("a/leaf.spx", 1),
                ("b/left.spx", 2),
                ("c/right.spx", 2),
                ("z/entry.spx", 3),
            ]
        );
        assert_eq!(
            projection
                .declarations()
                .iter()
                .filter(|fact| fact.origin() != hir::IdentityOrigin::CompilerOwned)
                .map(WorkspaceGraphProjectionDeclaration::id)
                .collect::<Vec<_>>(),
            [
                "diamond.leaf",
                "diamond.left",
                "diamond.main",
                "diamond.right",
            ]
        );
        assert_eq!(
            projection
                .edges()
                .iter()
                .filter(|edge| edge.kind() == "function_import")
                .map(|edge| (
                    edge.caller_path(),
                    edge.caller(),
                    edge.target_path(),
                    edge.target(),
                    edge.kind(),
                    edge.site(),
                    edge.expression(),
                    edge.ast_path(),
                    edge.alias(),
                    edge.ordinal(),
                ))
                .collect::<Vec<_>>(),
            [
                (
                    "b/left.spx",
                    "diamond.left",
                    "a/leaf.spx",
                    "diamond.leaf",
                    "function_import",
                    "module",
                    "use.0",
                    "use.0",
                    "leaf",
                    0,
                ),
                (
                    "c/right.spx",
                    "diamond.right",
                    "a/leaf.spx",
                    "diamond.leaf",
                    "function_import",
                    "module",
                    "use.0",
                    "use.0",
                    "leaf",
                    0,
                ),
                (
                    "z/entry.spx",
                    "diamond.entry",
                    "b/left.spx",
                    "diamond.left",
                    "function_import",
                    "module",
                    "use.0",
                    "use.0",
                    "left",
                    0,
                ),
                (
                    "z/entry.spx",
                    "diamond.entry",
                    "c/right.spx",
                    "diamond.right",
                    "function_import",
                    "module",
                    "use.1",
                    "use.1",
                    "right",
                    1,
                ),
            ]
        );
        assert!(projection.edges().windows(2).all(|pair| pair[0] < pair[1]));
        assert!(projection.modules().iter().any(|module| {
            module.module() == "diamond.entry"
                && module
                    .functions()
                    .iter()
                    .any(|function| function.name == "main")
        }));
        assert!(projection.modules().iter().all(|module| {
            module.functions().iter().all(|function| {
                !function
                    .id
                    .as_str()
                    .starts_with("workspace.synthetic.main.")
            })
        }));

        let leaf = authenticated_build_from(diamond_sources())
            .project("diamond.leaf")
            .unwrap();
        assert_eq!(leaf.modules().len(), 1);
        assert_eq!(leaf.modules()[0].path(), "a/leaf.spx");
        assert!(projection
            .modules()
            .iter()
            .all(|module| !matches!(module.module(), "diamond.reverse" | "diamond.disconnected")));
        assert!(projection
            .declarations()
            .iter()
            .all(|fact| !matches!(fact.id(), "diamond.reverse" | "diamond.disconnected")));

        let mut full_usage = projection.usage();
        let mut leaf_usage = leaf.usage();
        assert_eq!(full_usage.used_managed_files(), 6);
        assert_eq!(full_usage.used_reachable_modules(), 4);
        assert_eq!(leaf_usage.used_reachable_modules(), 1);
        assert_eq!(full_usage.used_entry_module_bytes(), "diamond.entry".len());
        assert_eq!(leaf_usage.used_entry_module_bytes(), "diamond.leaf".len());
        full_usage.used_reachable_modules = 0;
        leaf_usage.used_reachable_modules = 0;
        full_usage.used_entry_module_bytes = 0;
        leaf_usage.used_entry_module_bytes = 0;
        assert_eq!(full_usage, leaf_usage);
    }

    #[test]
    fn projection_preserves_identity_ownership_once_without_stubs_or_synthetic_main() {
        let sources = vec![
            canonical_source(
                "app/identity.spx",
                r#"
module projection.identity;
use function @id("projection.answer") from projection.provider as answer;

@id("projection.record")
record Record {
    automatic: i64,
    @id("projection.record.explicit")
    explicit: i64,
}

fn helper() -> i64 { answer() }

@id("projection.main")
fn main() -> i64 { helper() }
"#,
            ),
            canonical_source(
                "lib/provider.spx",
                r#"
module projection.provider;

@id("projection.answer")
fn answer() -> i64 { 42 }
"#,
            ),
        ];
        let projection = authenticated_build_from(sources)
            .project("projection.identity")
            .unwrap();
        let assert_fact = |id: &str,
                           kind: hir::DeclarationKind,
                           origin: hir::IdentityOrigin,
                           owner: Option<&str>,
                           path: &str,
                           module: &str| {
            let fact = projection
                .declarations()
                .iter()
                .find(|fact| fact.id() == id)
                .unwrap_or_else(|| panic!("missing projected fact `{id}`"));
            assert_eq!(fact.kind(), kind, "kind for `{id}`");
            assert_eq!(fact.origin(), origin, "origin for `{id}`");
            assert_eq!(fact.owner(), owner, "owner for `{id}`");
            assert_eq!(fact.path(), Some(path), "path for `{id}`");
            assert_eq!(fact.module(), Some(module), "module for `{id}`");
        };
        assert_fact(
            "projection.record",
            hir::DeclarationKind::Record,
            hir::IdentityOrigin::Explicit,
            None,
            "app/identity.spx",
            "projection.identity",
        );
        assert_fact(
            "auto:field:projection.record.automatic",
            hir::DeclarationKind::Field,
            hir::IdentityOrigin::Automatic,
            Some("projection.record"),
            "app/identity.spx",
            "projection.identity",
        );
        assert_fact(
            "projection.record.explicit",
            hir::DeclarationKind::Field,
            hir::IdentityOrigin::Explicit,
            Some("projection.record"),
            "app/identity.spx",
            "projection.identity",
        );
        assert_fact(
            "auto:projection.identity.helper",
            hir::DeclarationKind::Function,
            hir::IdentityOrigin::Automatic,
            None,
            "app/identity.spx",
            "projection.identity",
        );
        assert_fact(
            "projection.main",
            hir::DeclarationKind::Function,
            hir::IdentityOrigin::Explicit,
            None,
            "app/identity.spx",
            "projection.identity",
        );
        assert_fact(
            "projection.answer",
            hir::DeclarationKind::Function,
            hir::IdentityOrigin::Explicit,
            None,
            "lib/provider.spx",
            "projection.provider",
        );

        let expected_prelude = prelude::all_ids().into_iter().collect::<BTreeSet<_>>();
        let projected_prelude = projection
            .shared_prelude_ids()
            .iter()
            .copied()
            .collect::<BTreeSet<_>>();
        assert_eq!(
            projection.shared_prelude_ids().len(),
            expected_prelude.len()
        );
        assert_eq!(projected_prelude, expected_prelude);
        let compiler_facts = projection
            .declarations()
            .iter()
            .filter(|fact| fact.origin() == hir::IdentityOrigin::CompilerOwned)
            .map(WorkspaceGraphProjectionDeclaration::id)
            .collect::<BTreeSet<_>>();
        assert_eq!(compiler_facts, projected_prelude);

        let functions = projection
            .modules()
            .iter()
            .map(|module| {
                (
                    module.module(),
                    module
                        .functions()
                        .iter()
                        .map(|function| function.id.as_str())
                        .collect::<BTreeSet<_>>(),
                )
            })
            .collect::<BTreeMap<_, _>>();
        assert_eq!(
            functions["projection.identity"],
            BTreeSet::from(["auto:projection.identity.helper", "projection.main"])
        );
        assert_eq!(
            functions["projection.provider"],
            BTreeSet::from(["projection.answer"])
        );
        assert!(projection
            .declarations()
            .iter()
            .all(|fact| !fact.id().starts_with("workspace.synthetic.main.")));
    }

    #[test]
    fn mixed_graph_v10_through_v14_metadata_is_path_ordered_and_exact() {
        let sources = vec![
            canonical_source(
                "e/v10.spx",
                r#"
module graph.v10;
use function @id("graph.v11.run") from graph.v11 as next;
@id("graph.v10.run") fn run() -> i64 { next() }
"#,
            ),
            canonical_source(
                "d/v11.spx",
                r#"
module graph.v11;
use function @id("graph.v12.run") from graph.v12 as next;
@id("graph.v11.run") fn run() -> i64 { next() }
"#,
            ),
            canonical_source(
                "c/v12.spx",
                r#"
module graph.v12;
use function @id("graph.v13.run") from graph.v13 as next;
@id("graph.v12.run") fn run() -> i64 { next() }
"#,
            ),
            canonical_source(
                "b/v13.spx",
                r#"
module graph.v13;
use function @id("graph.v14.run") from graph.v14 as next;
@id("graph.v13.run") fn run() -> i64 { next() }
"#,
            ),
            canonical_source(
                "a/v14.spx",
                r#"
module graph.v14;
@id("graph.v14.run") fn run() -> i64 { 14 }
"#,
            ),
        ];
        let mut authenticated = authenticated_build_from(sources);
        for (path, schema) in [
            ("e/v10.spx", "semaprax.semantic-graph.v10"),
            ("d/v11.spx", "semaprax.semantic-graph.v11"),
            ("c/v12.spx", "semaprax.semantic-graph.v12"),
            ("b/v13.spx", "semaprax.semantic-graph.v13"),
            ("a/v14.spx", "semaprax.semantic-graph.v14"),
        ] {
            let fact = authenticated.sources.get_mut(path).unwrap();
            fact.source_graph_schema = schema.to_owned();
            fact.source_revision = format!("revision:{schema}");
            fact.source_digest = format!("digest:{schema}");
        }
        let projection = authenticated.project("graph.v10").unwrap();
        assert_eq!(
            projection
                .modules()
                .iter()
                .map(|module| (
                    module.path(),
                    module.source_graph_schema(),
                    module.source_revision(),
                    module.source_digest(),
                ))
                .collect::<Vec<_>>(),
            [
                (
                    "a/v14.spx",
                    "semaprax.semantic-graph.v14",
                    "revision:semaprax.semantic-graph.v14",
                    "digest:semaprax.semantic-graph.v14",
                ),
                (
                    "b/v13.spx",
                    "semaprax.semantic-graph.v13",
                    "revision:semaprax.semantic-graph.v13",
                    "digest:semaprax.semantic-graph.v13",
                ),
                (
                    "c/v12.spx",
                    "semaprax.semantic-graph.v12",
                    "revision:semaprax.semantic-graph.v12",
                    "digest:semaprax.semantic-graph.v12",
                ),
                (
                    "d/v11.spx",
                    "semaprax.semantic-graph.v11",
                    "revision:semaprax.semantic-graph.v11",
                    "digest:semaprax.semantic-graph.v11",
                ),
                (
                    "e/v10.spx",
                    "semaprax.semantic-graph.v10",
                    "revision:semaprax.semantic-graph.v10",
                    "digest:semaprax.semantic-graph.v10",
                ),
            ]
        );
        assert_eq!(projection.usage().used_reachable_modules(), 5);
    }

    #[test]
    fn rendered_document_has_literal_sha_exact_wire_order_and_digest_binding() {
        let graph = rendered_entry();
        let json = graph.to_json();
        let document_sha = format!(
            "sha256:{:x}",
            crate::digest_hex::LowerHex(Sha256::digest(json.as_bytes()))
        );
        assert_eq!(
            document_sha,
            "sha256:e4a78dabb26de46141a5f3b920183279317a409aaa07c0cc222f58df3ad82636"
        );
        assert!(json.starts_with(
                "{\"schema\":\"semaprax.workspace-semantic-graph.v1\",\"workspace_manifest_schema\":\"semaprax.workspace-semantic-manifest.v1\",\"workspace_revision\":\"sha256:workspace\",\"graph_digest\":\"sha256:"
            ));
        assert!(json.ends_with("\"]}"));
        assert_ordered_fragments(
            json,
            &[
                "\"schema\":",
                "\"workspace_manifest_schema\":",
                "\"workspace_revision\":",
                "\"graph_digest\":",
                "\"entry\":",
                "\"modules\":",
                "\"declarations\":",
                "\"edges\":",
                "\"limits\":",
                "\"budget\":",
                "\"nonclaims\":",
            ],
        );
        let types = graph
            .modules()
            .iter()
            .find(|module| module.path() == "a/types.spx")
            .unwrap();
        assert!(json.contains(&format!(
                "{{\"path\":\"a/types.spx\",\"module\":\"types.core\",\"source_graph_schema\":\"semaprax.semantic-graph.v14\",\"source_revision\":\"revision:a/types.spx\",\"source_digest\":\"{}\",\"dependency_depth\":1,\"permits\":[]}}",
                types.source_digest()
            )));
        assert!(json.contains(
                "{\"id\":\"types.point\",\"kind\":\"record\",\"identity_origin\":\"explicit\",\"owner\":null,\"path\":\"a/types.spx\",\"module\":\"types.core\"}"
            ));
        assert!(json.contains(
                "{\"caller_path\":\"m/provider.spx\",\"caller\":\"provider.api\",\"target_path\":\"a/types.spx\",\"target\":\"types.point\",\"kind\":\"type_import\",\"site\":\"module\",\"expression\":\"use.0\",\"ast_path\":\"use.0\",\"alias\":\"Point\",\"ordinal\":0}"
            ));
        assert!(json.contains(
                "\"max_managed_files\":16,\"max_reachable_modules\":16,\"max_entry_module_bytes\":16777216,\"max_total_source_bytes\":16777216"
            ));
        assert!(json.contains(
                "\"used_managed_files\":4,\"used_reachable_modules\":3,\"used_entry_module_bytes\":9,\"used_total_source_bytes\":"
            ));

        let digest_field = format!(",\"graph_digest\":\"{}\"", graph.graph_digest());
        assert_eq!(json.matches(&digest_field).count(), 1);
        let payload = json.replacen(&digest_field, "", 1);
        assert_eq!(artifact_digest(payload.as_bytes()), graph.graph_digest());
        assert_eq!(graph.budget().used_output_bytes(), json.len());
    }

    #[test]
    fn rendering_replays_input_order_and_digest_binds_payload_mutation() {
        let sources = diamond_sources();
        let first = render_semantic_graph(
            authenticated_build_from(sources.clone())
                .project("diamond.entry")
                .unwrap(),
        )
        .unwrap();
        let mut reversed = sources;
        reversed.reverse();
        let second = render_semantic_graph(
            authenticated_build_from(reversed)
                .project("diamond.entry")
                .unwrap(),
        )
        .unwrap();
        assert_eq!(first.graph_digest(), second.graph_digest());
        assert_eq!(first.to_json(), second.to_json());

        let json = first.to_json();
        let digest_field = format!(",\"graph_digest\":\"{}\"", first.graph_digest());
        let payload = json.replacen(&digest_field, "", 1);
        let mut mutated = payload.into_bytes();
        let offset = mutated
            .windows(b"diamond.entry".len())
            .position(|window| window == b"diamond.entry")
            .expect("entry module must occur in digest payload");
        mutated[offset] = b'D';
        assert_ne!(artifact_digest(&mutated), first.graph_digest());
    }

    #[test]
    fn rendering_output_cap_is_exact_and_returns_no_partial_artifact() {
        let used = rendered_entry().to_json().len();
        let exact = render_semantic_graph_with_output_limit(
            authenticated_build().project("app.entry").unwrap(),
            used,
        )
        .expect("the exact measured output limit must succeed");
        assert_eq!(exact.to_json().len(), used);
        assert_eq!(exact.budget().used_output_bytes(), used);

        let error = render_semantic_graph_with_output_limit(
            authenticated_build().project("app.entry").unwrap(),
            used - 1,
        )
        .err()
        .expect("one byte below the exact output limit must return no artifact");
        assert_eq!(error.len(), 1);
        assert_eq!(error[0].code, "SPX-G171");
        assert_eq!(
            error[0].message,
            format!(
                "Workspace Semantic Graph `output_bytes` exceeds {}",
                used - 1
            )
        );
    }

    #[test]
    fn rendered_wire_carries_exact_projection_closure_and_full_counters() {
        let graph = render_semantic_graph(
            authenticated_build_from(diamond_sources())
                .project("diamond.entry")
                .unwrap(),
        )
        .unwrap();
        let json = graph.to_json();
        let value: serde_json::Value = serde_json::from_str(json).unwrap();
        assert_eq!(value["entry"]["module"], "diamond.entry");
        assert_eq!(value["entry"]["path"], "z/entry.spx");
        assert_eq!(
            value["modules"]
                .as_array()
                .unwrap()
                .iter()
                .map(|module| module["path"].as_str().unwrap())
                .collect::<Vec<_>>(),
            ["a/leaf.spx", "b/left.spx", "c/right.spx", "z/entry.spx"]
        );
        assert!(value["modules"].as_array().unwrap().iter().all(|module| {
            !matches!(
                module["module"].as_str().unwrap(),
                "diamond.reverse" | "diamond.disconnected"
            )
        }));

        let usage = graph.budget();
        let budget = &value["budget"];
        for (key, expected) in [
            ("used_managed_files", usage.used_managed_files()),
            ("used_reachable_modules", usage.used_reachable_modules()),
            ("used_entry_module_bytes", usage.used_entry_module_bytes()),
            ("used_total_source_bytes", usage.used_total_source_bytes()),
            ("used_declarations", usage.used_declarations()),
            ("used_callables", usage.used_callables()),
            ("used_call_sites", usage.used_call_sites()),
            ("used_uses", usage.used_uses()),
            (
                "used_resolved_cross_file_edges",
                usage.used_resolved_cross_file_edges(),
            ),
            ("used_dependency_depth", usage.used_dependency_depth()),
            ("used_builder_bytes", usage.used_builder_bytes()),
            ("used_manifest_bytes", usage.used_manifest_bytes()),
            ("used_output_bytes", usage.used_output_bytes()),
            (
                "used_retained_generations",
                usage.used_retained_generations(),
            ),
            ("used_staging_attempts", usage.used_staging_attempts()),
            (
                "used_unexpected_inventory_entries",
                usage.used_unexpected_inventory_entries(),
            ),
        ] {
            assert_eq!(budget[key].as_u64(), Some(expected as u64), "{key}");
        }
        assert_eq!(usage.used_managed_files(), 6);
        assert_eq!(usage.used_reachable_modules(), 4);
        assert_eq!(usage.used_entry_module_bytes(), "diamond.entry".len());
        assert_eq!(
            value["nonclaims"].as_array().unwrap().len(),
            NONCLAIMS.len()
        );
    }

    #[test]
    fn entry_module_byte_limit_precedes_grammar_and_never_echoes_oversize_input() {
        let exact = "a".repeat(MAX_ENTRY_MODULE_BYTES);
        validate_entry_module(&exact).expect("the exact entry-module byte limit is admitted");

        for oversized in [
            format!("{}x", exact),
            format!("-private-sentinel-{}", "x".repeat(MAX_ENTRY_MODULE_BYTES)),
        ] {
            let error = validate_entry_module(&oversized)
                .expect_err("one-over entry modules must fail before grammar or lookup");
            assert_eq!(error.len(), 1);
            assert_eq!(error[0].code, "SPX-G171");
            assert_eq!(
                error[0].message,
                "Workspace Semantic Graph `entry_module_bytes` exceeds 16777216"
            );
            assert!(!error[0].message.contains("private-sentinel"));
            assert!(!error[0].message.contains(&oversized));
        }
    }

    #[test]
    fn renderer_escapes_authenticated_strings_canonically() {
        let escaped = "revision:\"quoted\"\\slash\nline\rreturn\ttab\u{0001}";
        let mut authenticated = authenticated_build();
        authenticated.workspace_revision = escaped.to_owned();
        authenticated
            .sources
            .get_mut("z/entry.spx")
            .unwrap()
            .source_revision = escaped.to_owned();
        let graph = render_semantic_graph(authenticated.project("app.entry").unwrap()).unwrap();
        let json = graph.to_json();
        assert!(json.contains(
                "\"workspace_revision\":\"revision:\\\"quoted\\\"\\\\slash\\nline\\rreturn\\ttab\\u0001\""
            ));
        assert!(json.contains(
            "\"source_revision\":\"revision:\\\"quoted\\\"\\\\slash\\nline\\rreturn\\ttab\\u0001\""
        ));
        let value: serde_json::Value = serde_json::from_str(json).unwrap();
        assert_eq!(value["workspace_revision"], escaped);
        assert_eq!(value["modules"][2]["source_revision"], escaped);
    }

    #[test]
    fn measured_builder_usage_is_the_exact_minimum_successful_limit() {
        let sources = projection_sources();
        let build = build_owned(sources.clone()).unwrap();
        let used = build.usage.builder_bytes;
        let exact = build_owned_with_builder_limit(sources.clone(), used).unwrap();
        assert_eq!(exact.usage.builder_bytes, used);
        let error = build_owned_with_builder_limit(sources, used - 1)
            .err()
            .expect("one byte below measured sequential peak must fail");
        assert_eq!(error[0].code, "SPX-G171");
        assert_eq!(
            error[0].message,
            format!(
                "Workspace Semantic Graph `builder_bytes` exceeds {}",
                used - 1
            )
        );
    }

    #[test]
    fn real_managed_renderer_matches_the_production_shared_hook_path() {
        let fixture = ManagedFixture::new("semantic-graph-success");
        let production = snapshot(&fixture.root, "workspace.alpha")
            .expect("production managed rendering must succeed");
        let hook_called = Cell::new(false);
        let through_hook = build_authenticated_semantic_graph_with_hook(
            &fixture.root,
            "workspace.alpha",
            |rendered| {
                hook_called.set(true);
                assert_eq!(rendered.schema(), WORKSPACE_GRAPH_SCHEMA);
                assert_eq!(rendered.workspace_revision(), fixture.revision);
                assert_eq!(rendered.entry().module(), "workspace.alpha");
                assert_eq!(rendered.entry().path(), "alpha.spx");
                assert_eq!(rendered.budget().used_managed_files(), 2);
                assert_eq!(rendered.budget().used_reachable_modules(), 1);
            },
        )
        .expect("unchanged shared-hook rendering must succeed");
        assert!(hook_called.get());
        assert_eq!(through_hook.graph_digest(), production.graph_digest());
        assert_eq!(through_hook.to_json(), production.to_json());
        assert_eq!(
            through_hook
                .modules()
                .iter()
                .map(WorkspaceSemanticGraphModule::path)
                .collect::<Vec<_>>(),
            ["alpha.spx"]
        );
        assert!(through_hook
            .declarations()
            .iter()
            .all(|declaration| declaration.path() != Some("beta.spx")));
        fixture.reacquire_exclusive();
    }

    #[test]
    fn after_render_mutation_discards_graph_rechecks_and_releases_lock() {
        let fixture = ManagedFixture::new("semantic-graph-final-recheck");
        let hook_called = Cell::new(false);
        let result = build_authenticated_semantic_graph_with_hook(
            &fixture.root,
            "workspace.alpha",
            |rendered| {
                hook_called.set(true);
                assert!(!rendered.to_json().is_empty());
                append_byte(&fixture.control().join("ACTIVE"));
            },
        );
        assert!(
            hook_called.get(),
            "mutation must occur only after full rendering"
        );
        let error = result
            .err()
            .expect("the final recheck must return no semantic graph artifact");
        assert_eq!(error.len(), 1);
        assert_eq!(error[0].code, "SPX-G153");
        assert_eq!(
            error[0].message,
            "workspace object changed during authentication"
        );
        fixture.reacquire_exclusive();
    }

    #[test]
    fn final_projection_boundary_rechecks_owned_workspace_and_releases_lock() {
        for mutation in ["active", "manifest", "source", "inventory"] {
            let fixture = ManagedFixture::new(mutation);
            let error =
                build_authenticated_projection_with_hook(&fixture.root, "workspace.alpha", || {
                    match mutation {
                        "active" => append_byte(&fixture.control().join("ACTIVE")),
                        "manifest" => append_byte(&fixture.generation().join("manifest.json")),
                        "source" => append_byte(&fixture.generation().join("files/alpha.spx")),
                        "inventory" => {
                            std::fs::create_dir(fixture.control().join("staging/0")).unwrap();
                        }
                        _ => unreachable!(),
                    }
                })
                .err()
                .expect("final authenticated boundary must reject mutation");
            assert!(
                matches!(error[0].code, "SPX-G152" | "SPX-I209" | "SPX-G153"),
                "unexpected underlying Workspace diagnostic: {:?}",
                error[0]
            );
            fixture.reacquire_exclusive();
        }
    }

    #[cfg(unix)]
    #[test]
    fn final_projection_boundary_rejects_generation_identity_replacement() {
        let fixture = ManagedFixture::new("generation");
        let generation = fixture.generation();
        let displaced = fixture.control().join("generations/displaced");
        let error =
            build_authenticated_projection_with_hook(&fixture.root, "workspace.alpha", || {
                std::fs::rename(&generation, &displaced).unwrap();
                std::fs::create_dir(&generation).unwrap();
            })
            .err()
            .expect("generation identity replacement must fail");
        assert!(matches!(error[0].code, "SPX-I209" | "SPX-G153"));
        fixture.reacquire_exclusive();
    }

    fn append_byte(path: &Path) {
        use std::io::Write as _;

        let mut file = OpenOptions::new().append(true).open(path).unwrap();
        file.write_all(b" ").unwrap();
        file.sync_all().unwrap();
    }
}

#[test]
fn imported_type_classification_is_per_consumer_module() {
    let app = canonical_source(
        "app/main.spx",
        r#"
module app.main;
use type @id("lib.point") from lib.core as RemotePoint;

@id("app.main")
fn main() -> i64 { 0 }
"#,
    );
    let build = build_owned(vec![app, point_provider()]).unwrap();
    assert!(build.edges.iter().all(|edge| {
        edge.kind != "type_reference"
            || edge.caller_path != "lib/core.spx"
            || edge.target != "lib.point"
    }));
}

#[test]
fn explicit_body_pattern_and_nested_type_carrier_sites_replay() {
    let app = canonical_source(
        "app/main.spx",
        r#"
module app.main;
use type @id("lib.point") from lib.core as RemotePoint;

@id("app.wrapper")
record Wrapper {
    @id("app.wrapper.point")
    point: RemotePoint,
}

@id("app.read")
fn read(value: Wrapper) -> i64 {
    match value { Wrapper { point: RemotePoint { x } } => x, }
}

@id("app.main")
fn main() -> i64 {
    let value = Wrapper { point: RemotePoint { x: 7 } };
    read(value)
}
"#,
    );
    let build = build_owned(vec![app, point_provider()]).unwrap();
    let paths = build
        .edges
        .iter()
        .filter(|edge| edge.kind == "type_reference" && edge.target == "lib.point")
        .map(|edge| (edge.caller.as_str(), edge.ast_path.as_str()))
        .collect::<BTreeSet<_>>();
    assert!(paths.contains(&("app.wrapper", "type.app.wrapper.field.0")));
    assert!(paths.contains(&("app.read", "body.tail.arm.0.pattern.field.0.pattern")));
    assert!(paths.contains(&("app.main", "body.s0.value.field.0.value.type")));
}

#[test]
fn transitive_exposed_type_requires_direct_consumer_import() {
    let leaf = canonical_source(
        "a/point.spx",
        r#"
module a.point;

@id("a.point")
record Point { @id("a.point.x") x: i64, }

@id("a.local")
fn local() -> i64 { 0 }
"#,
    );
    let wrapper = canonical_source(
        "b/wrapper.spx",
        r#"
module b.wrapper;
use type @id("a.point") from a.point as InnerPoint;

@id("b.wrapper")
record Wrapper { @id("b.wrapper.value") value: InnerPoint, }

@id("b.local")
fn local() -> i64 { 0 }
"#,
    );
    let missing = canonical_source(
        "c/main.spx",
        r#"
module c.main;
use type @id("b.wrapper") from b.wrapper as Wrapper;

@id("c.main")
fn main() -> i64 { 0 }
"#,
    );
    let error = build_owned(vec![leaf.clone(), wrapper.clone(), missing])
        .err()
        .expect("transitive type authority must be direct");
    assert_eq!(error[0].code, "SPX-G172");

    let present = canonical_source(
        "c/main.spx",
        r#"
module c.main;
use type @id("b.wrapper") from b.wrapper as Wrapper;
use type @id("a.point") from a.point as DirectPoint;

@id("c.main")
fn main() -> i64 { 0 }
"#,
    );
    build_owned(vec![leaf, wrapper, present]).unwrap();
}

#[test]
fn interface_import_carriers_reject_workspace_type_aliases_pre_hir() {
    let app = canonical_source(
        "app/main.spx",
        r#"
module app.main;
use type @id("lib.point") from lib.core as RemotePoint;

@id("app.host")
interface Host permits {} {
    @id("app.host.consume")
    import fn consume(value: own RemotePoint) -> unit
        effects {}
        failure infallible
        consumes value always;
}

@id("app.main")
fn main() -> i64 { 0 }
"#,
    );
    let error = build_owned(vec![app, point_provider()])
        .err()
        .expect("interface carrier must reject workspace alias");
    assert_eq!(error[0].code, "SPX-G172");
    assert_eq!(
        error[0].message,
        "workspace type aliases are not admitted in interface/import parameter carriers"
    );
}

#[test]
fn shared_loans_cannot_mask_an_owned_variant_v22_workspace_base_schema() {
    let app = canonical_source(
        "app/main.spx",
        r#"
module app.main;

@id("app.choice")
variant Choice {
    @id("app.choice.none") None,
    @id("app.choice.data") Data {
        @id("app.choice.data.payload") payload: Bytes,
    },
}

@id("app.consume")
fn consume(value: own Choice) -> i64 {
    match own value {
        Choice::None {} => 0,
        Choice::Data { payload } => 1,
    }
}

@id("app.view")
fn view(input: borrow Slice<u8>) -> usize {
    let owned = bytes_copy(input);
    let bytes = bytes_as_slice(owned);
    byte_len(bytes)
}

@id("app.main")
fn main() -> i64 { 0 }
"#,
    );
    let support = canonical_source(
        "lib/support.spx",
        r#"
module lib.support;

@id("lib.support.ready")
fn ready() -> i64 { 0 }
"#,
    );
    let sources = vec![app, support];
    let build = build_owned(sources.clone()).unwrap();
    let error = build
        .source_graph_schemas()
        .expect_err("Graph v23 must not hide an unsupported v22 base schema");
    assert_eq!(error[0].code, "SPX-G410");
    assert!(error[0].message.contains("cannot mask"));

    let (change, _) =
        build_owned_retaining_sources_for_change(sources, MAX_CHANGE_BUILDER_BYTES).unwrap();
    let error = match change.into_change_view() {
        Err(error) => error,
        Ok(_) => panic!("change view must share the same complete schema gate"),
    };
    assert_eq!(error[0].code, "SPX-G410");
}
