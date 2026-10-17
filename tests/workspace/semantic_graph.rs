const SEMANTIC_GRAPH_PRIVATE_LAYOUT_ROOT: &str =
    "src/workspace_graph/expected_projection/private_layout.rs";
include!("semantic_graph/source_inventory.rs");
#[path = "semantic_graph/source_inventory_controls.rs"]
mod source_inventory_controls;
use std::fs::{File, OpenOptions};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicU64, Ordering};

use semaprax::semantic_workspace;
use semaprax::workspace_analysis::{
    self, WorkspaceAnalysisDirection, WorkspaceAnalysisTargetKind, WorkspaceContextOptions,
    WorkspaceImpactOptions,
};
use semaprax::workspace_graph::{
    self, WorkspaceSemanticGraph, WorkspaceSemanticGraphBudget, WorkspaceSemanticGraphDeclaration,
    WorkspaceSemanticGraphEdge, WorkspaceSemanticGraphEntry, WorkspaceSemanticGraphLimits,
    WorkspaceSemanticGraphModule,
};
use semaprax::{format, parse, workspace};
use sha2::{Digest, Sha256};
static SERIAL: AtomicU64 = AtomicU64::new(0);

struct ManagedFixture {
    root: PathBuf,
    path_set: PathBuf,
    revision: String,
}

impl ManagedFixture {
    fn stage(label: &str) -> (PathBuf, PathBuf) {
        let serial = SERIAL.fetch_add(1, Ordering::Relaxed);
        let root = std::env::temp_dir().join(format!(
            "semaprax-workspace-semantic-graph-v1-{label}-{}-{serial}",
            std::process::id()
        ));
        std::fs::create_dir(&root).unwrap();
        let sources = [
            (
                "z/app.spx",
                r#"
module public.app;
use type @id("public.point") from public.provider as Point;
use function @id("public.work") from public.provider as work;
permit { audit.write }

fn helper() -> i64 { 7 }

@id("public.main")
fn main() -> i64 uses { audit.write } {
    let local = helper();
    local + work(Point { value: 1 })
}
"#,
            ),
            (
                "a/provider.spx",
                r#"
module public.provider;
permit { audit.write }

@id("public.point")
record Point { @id("public.point.value") value: i64, }

@id("public.work")
fn work(value: Point) -> i64 uses { audit.write } { value.value }

@id("public.provider.main")
fn main() -> i64 { 0 }
"#,
            ),
        ];
        for (path, source) in sources {
            let destination = root.join(path);
            std::fs::create_dir_all(destination.parent().unwrap()).unwrap();
            let program = parse(source, path).unwrap();
            std::fs::write(destination, format::canonical(&program)).unwrap();
        }
        let path_set = root.join("paths.json");
        std::fs::write(
            &path_set,
            "{\"schema\":\"semaprax.workspace-semantic-path-set.v1\",\"files\":[{\"path\":\"a/provider.spx\"},{\"path\":\"z/app.spx\"}]}\n",
        )
        .unwrap();
        (root, path_set)
    }

    fn new(label: &str) -> Self {
        let (root, path_set) = Self::stage(label);
        let revision = semantic_workspace::initialize(&root, &path_set).unwrap();
        Self {
            root,
            path_set,
            revision,
        }
    }

    fn new_via_cli(label: &str) -> (Self, Vec<u8>) {
        let (root, path_set) = Self::stage(label);
        let output = Command::new(env!("CARGO_BIN_EXE_semaprax"))
            .arg("semantic-workspace-init")
            .arg(&root)
            .arg(&path_set)
            .output()
            .unwrap();
        assert!(output.status.success());
        assert!(output.stderr.is_empty());
        let stdout = String::from_utf8(output.stdout.clone()).unwrap();
        let revision = stdout
            .strip_prefix("initialized semantic graph workspace; workspace is ")
            .and_then(|value| value.strip_suffix('\n'))
            .expect("semantic initializer stdout must have exact framing")
            .to_owned();
        (
            Self {
                root,
                path_set,
                revision,
            },
            output.stdout,
        )
    }

    fn lock(&self) -> File {
        OpenOptions::new()
            .read(true)
            .write(true)
            .open(self.root.join(".semaprax-workspace/LOCK"))
            .unwrap()
    }
}

impl Drop for ManagedFixture {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.root);
    }
}

fn inventory(root: &Path) -> Vec<(String, &'static str, Vec<u8>)> {
    fn visit(root: &Path, current: &Path, out: &mut Vec<(String, &'static str, Vec<u8>)>) {
        let mut entries = std::fs::read_dir(current)
            .unwrap()
            .map(|entry| entry.unwrap().path())
            .collect::<Vec<_>>();
        entries.sort();
        for path in entries {
            let relative = path
                .strip_prefix(root)
                .unwrap()
                .to_string_lossy()
                .into_owned();
            let metadata = std::fs::symlink_metadata(&path).unwrap();
            if metadata.is_dir() {
                out.push((relative, "directory", Vec::new()));
                visit(root, &path, out);
            } else if metadata.is_file() {
                out.push((relative, "file", std::fs::read(&path).unwrap()));
            } else {
                out.push((relative, "other", Vec::new()));
            }
        }
    }

    let mut result = Vec::new();
    visit(root, root, &mut result);
    result
}

/// Optional evidence sink for the actual owning producers. Existing assertions
/// still execute, and an existing evidence file is never replaced.
fn capture_pin_candidate(name: &str, bytes: &[u8]) {
    let Some(directory) = std::env::var_os("SEMAPRAX_PRELUDE_PIN_CAPTURE_DIR") else {
        return;
    };
    use std::io::Write as _;
    let path = std::path::Path::new(&directory).join(name);
    let mut file = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)
        .expect("fresh pin evidence file");
    file.write_all(bytes).expect("capture complete actual wire");
}

fn document_digest(bytes: &[u8]) -> String {
    format!(
        "sha256:{:x}",
        semaprax::digest_hex::LowerHex(Sha256::digest(bytes))
    )
}

#[allow(dead_code)]
fn assert_public_edge_getter_surface(edge: &WorkspaceSemanticGraphEdge) {
    let _: (&str, &str, &str, &str, &str, &str, &str, &str, &str, usize) = (
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
    );
}

fn assert_public_getters_match_wire(graph: &WorkspaceSemanticGraph) {
    let wire: serde_json::Value = serde_json::from_str(graph.to_json()).unwrap();
    assert_eq!(graph.schema(), wire["schema"].as_str().unwrap());
    assert_eq!(
        graph.workspace_manifest_schema(),
        wire["workspace_manifest_schema"].as_str().unwrap()
    );
    assert_eq!(graph.workspace_revision(), wire["workspace_revision"]);
    assert_eq!(graph.graph_digest(), wire["graph_digest"]);

    let entry: &WorkspaceSemanticGraphEntry = graph.entry();
    assert_eq!(entry.module(), wire["entry"]["module"]);
    assert_eq!(entry.path(), wire["entry"]["path"]);

    let modules = wire["modules"].as_array().unwrap();
    for (module, encoded) in graph.modules().iter().zip(modules) {
        let _: &WorkspaceSemanticGraphModule = module;
        assert_eq!(module.path(), encoded["path"]);
        assert_eq!(module.module(), encoded["module"]);
        assert_eq!(module.source_graph_schema(), encoded["source_graph_schema"]);
        assert_eq!(module.source_revision(), encoded["source_revision"]);
        assert_eq!(module.source_digest(), encoded["source_digest"]);
        assert_eq!(
            module.dependency_depth() as u64,
            encoded["dependency_depth"].as_u64().unwrap()
        );
        assert_eq!(
            module.permits(),
            encoded["permits"]
                .as_array()
                .unwrap()
                .iter()
                .map(|value| value.as_str().unwrap().to_owned())
                .collect::<Vec<_>>()
        );
    }

    let declarations = wire["declarations"].as_array().unwrap();
    for (declaration, encoded) in graph.declarations().iter().zip(declarations) {
        let _: &WorkspaceSemanticGraphDeclaration = declaration;
        assert_eq!(declaration.id(), encoded["id"]);
        assert_eq!(declaration.kind(), encoded["kind"]);
        assert_eq!(declaration.identity_origin(), encoded["identity_origin"]);
        assert_eq!(declaration.owner(), encoded["owner"].as_str());
        assert_eq!(declaration.path(), encoded["path"].as_str());
        assert_eq!(declaration.module(), encoded["module"].as_str());
    }
    assert!(graph.declarations().iter().any(|declaration| {
        declaration.id() == "auto:public.app.helper"
            && declaration.kind() == "function"
            && declaration.identity_origin() == "automatic"
    }));
    assert!(graph.declarations().iter().any(|declaration| {
        declaration.id() == "public.point"
            && declaration.kind() == "record"
            && declaration.identity_origin() == "explicit"
    }));
    assert!(graph.declarations().iter().all(|declaration| matches!(
        declaration.identity_origin(),
        "automatic" | "explicit" | "compiler_owned"
    )));

    let edges = wire["edges"].as_array().unwrap();
    assert_eq!(graph.edges().len(), edges.len());
    for (edge, encoded) in graph.edges().iter().zip(edges) {
        let _: &WorkspaceSemanticGraphEdge = edge;
        assert_eq!(edge.caller_path(), encoded["caller_path"]);
        assert_eq!(edge.caller(), encoded["caller"]);
        assert_eq!(edge.target_path(), encoded["target_path"]);
        assert_eq!(edge.target(), encoded["target"]);
        assert_eq!(edge.kind(), encoded["kind"]);
        assert_eq!(edge.site(), encoded["site"]);
        assert_eq!(edge.expression(), encoded["expression"]);
        assert_eq!(edge.ast_path(), encoded["ast_path"]);
        assert_eq!(edge.alias(), encoded["alias"]);
        assert_eq!(edge.ordinal() as u64, encoded["ordinal"].as_u64().unwrap());
    }

    let limits: WorkspaceSemanticGraphLimits = graph.limits();
    for (key, actual) in [
        ("max_managed_files", limits.max_managed_files()),
        ("max_reachable_modules", limits.max_reachable_modules()),
        ("max_entry_module_bytes", limits.max_entry_module_bytes()),
        ("max_total_source_bytes", limits.max_total_source_bytes()),
        ("max_declarations", limits.max_declarations()),
        ("max_callables", limits.max_callables()),
        ("max_call_sites", limits.max_call_sites()),
        ("max_uses", limits.max_uses()),
        (
            "max_resolved_cross_file_edges",
            limits.max_resolved_cross_file_edges(),
        ),
        ("max_dependency_depth", limits.max_dependency_depth()),
        ("max_builder_bytes", limits.max_builder_bytes()),
        ("max_manifest_bytes", limits.max_manifest_bytes()),
        ("max_output_bytes", limits.max_output_bytes()),
        (
            "max_retained_generations",
            limits.max_retained_generations(),
        ),
        ("max_staging_attempts", limits.max_staging_attempts()),
        (
            "max_unexpected_inventory_entries",
            limits.max_unexpected_inventory_entries(),
        ),
    ] {
        assert_eq!(wire["limits"][key].as_u64(), Some(actual as u64), "{key}");
    }

    let budget: WorkspaceSemanticGraphBudget = graph.budget();
    for (key, actual) in [
        ("used_managed_files", budget.used_managed_files()),
        ("used_reachable_modules", budget.used_reachable_modules()),
        ("used_entry_module_bytes", budget.used_entry_module_bytes()),
        ("used_total_source_bytes", budget.used_total_source_bytes()),
        ("used_declarations", budget.used_declarations()),
        ("used_callables", budget.used_callables()),
        ("used_call_sites", budget.used_call_sites()),
        ("used_uses", budget.used_uses()),
        (
            "used_resolved_cross_file_edges",
            budget.used_resolved_cross_file_edges(),
        ),
        ("used_dependency_depth", budget.used_dependency_depth()),
        ("used_builder_bytes", budget.used_builder_bytes()),
        ("used_manifest_bytes", budget.used_manifest_bytes()),
        ("used_output_bytes", budget.used_output_bytes()),
        (
            "used_retained_generations",
            budget.used_retained_generations(),
        ),
        ("used_staging_attempts", budget.used_staging_attempts()),
        (
            "used_unexpected_inventory_entries",
            budget.used_unexpected_inventory_entries(),
        ),
    ] {
        assert_eq!(wire["budget"][key].as_u64(), Some(actual as u64), "{key}");
    }
    assert_eq!(budget.used_entry_module_bytes(), "public.app".len());
    assert_eq!(budget.used_output_bytes(), graph.to_json().len());
    assert_eq!(
        graph.nonclaims(),
        wire["nonclaims"]
            .as_array()
            .unwrap()
            .iter()
            .map(|value| value.as_str().unwrap())
            .collect::<Vec<_>>()
    );
}

#[test]
fn expected_projection_source_boundary_is_pure_and_keeps_shared_helpers_in_root() {
    // The boundary this contract locks is "the workspace graph module keeps its
    // shared helpers", so the read has to follow the module, not just its root
    // file: a helper relocated into a sibling submodule must still count as
    // present in the root and absent from the projection.
    assert_eq!(audit_source_join!("", "α", "\0", "\r\n", "β"), "α\0\r\nβ");
    let root = semantic_graph_root_source_inventory!();
    let root = root.as_str();
    let projection_root = semantic_graph_projection_root_source_inventory!();
    let projection_local_identity =
        semantic_graph_projection_child_source_inventory!(local_identity);
    let projection_call_identity = semantic_graph_projection_child_source_inventory!(call_identity);
    let projection_call_sites = semantic_graph_projection_child_source_inventory!(call_sites);
    let projection_cost = semantic_graph_projection_child_source_inventory!(cost);
    let projection_declaration_cost =
        semantic_graph_projection_child_source_inventory!(declaration_cost);
    let projection_identity_slots =
        semantic_graph_projection_child_source_inventory!(identity_slots);
    let projection_statement_segment =
        semantic_graph_projection_child_source_inventory!(statement_segment);
    let projection_defaults = semantic_graph_projection_child_source_inventory!(defaults);
    let projection_import_stub = semantic_graph_projection_child_source_inventory!(import_stub);
    let projection_uncached_peak = semantic_graph_projection_child_source_inventory!(uncached_peak);
    let projection_type_rewrite = semantic_graph_projection_child_source_inventory!(type_rewrite);
    let projection_private_layout = semantic_graph_private_layout_owned_source_inventory!();
    let projection = format!(
        "{projection_private_layout}\n{projection_type_rewrite}\n{projection_import_stub}\n{projection_uncached_peak}\n{projection_root}\n{projection_local_identity}\n{projection_call_identity}\n{projection_call_sites}\n{projection_cost}\n{projection_declaration_cost}\n{projection_identity_slots}\n{projection_statement_segment}\n{projection_defaults}"
    );
    let production_projection = production_source_inventory::production(
        &projection,
        include_str!("../../docs/evidence/a3-e47-final-custody-20261016/projection.json"),
    );
    assert!(root.contains("mod expected_projection;"));
    assert!(!production_projection.contains("use super::*;"));
    assert!(projection_root.contains("pub(super) use uncached_peak::{\n    initial_core_prebound, next_retention_prebound_with_uncached_peak,\n    uncached_output_peak_prebound,\n};"));
    let exact_facade = [
        // These helpers are deliberately visible only inside workspace_graph;
        // the exact list prevents the analysis facade from growing silently.
        "pub(super) mod cached_exact;",
        "pub(super) mod cost;",
        "pub(super) mod private_layout;",
        "pub(super) mod statement_segment;",
        "pub(super) mod type_rewrite;",
        "pub(super) mod uncached_peak;",
        "pub(super) fn byte_writer_import_authorized<'a>(",
        "pub(super) use uncached_peak::{",
        "pub(super) struct SyntheticBuilderCosts {",
        "pub(super) raw_clone_and_hir: usize,",
        "pub(super) runtime: usize,",
        "pub(super) fn synthetic_builder_bytes(",
        "pub(super) fn synthetic_builder_bytes_scoped(",
        "pub(super) fn checked_retention_prebound(",
        "pub(super) fn checked_retention_prebound_with_uncached_peak(",
        "pub(super) fn retention_prebound_mode(",
        "pub(super) fn next_retention_prebound(",
        "pub(super) fn checked_builder_sum(left: usize, right: usize) -> Result<usize, Vec<Diagnostic>> {",
        "pub(super) fn rewrite_type_runtime_cost(",
        "pub(super) fn validate_dependency_dag(",
        "pub(super) fn dependency_depths<'a>(",
        "pub(super) fn synthetic_program(",
        "pub(super) fn rewrite_type(",
        "pub(super) fn collect_expected_edges(",
        "pub(super) fn verify_resolved_call_edges(",
        // Exact child-module facades include the field-uniqueness receiver.
        // The complete source union above keeps every child inside this audit.
        "pub(super) fn verify(",
        // The borrowed base-prelude forecast is private to expected_projection.
        "pub(super) fn retained_upper(",
        // These exact child-only helpers grant no workspace_graph facade:
        // checked_key::materialize is callable only by borrowed_leaf, and
        // old::construct is a cfg(test) oracle shared only by its test siblings.
        // Keep both in the full union and reject any other added helper.
        "pub(super) fn materialize(view: SyntheticProgramView<'_>) -> Result<Program, Vec<Diagnostic>> {",
        "pub(super) fn materialize_receiving(",
        "pub(super) fn type_payload(original: &TypeDeclaration) -> TypeDeclaration {",
        "pub(super) fn retained_function(original: &Function, alias: &str) -> Function {",
        "pub(super) fn signature_function(original: &Function, alias: &str) -> Function {",
        "pub(super) fn construct(",
        // Complete cfg(test) prelude initialization receiver stays in this
        // joined inventory; it grants no production facade or ambient authority.
        "pub(super) fn complete_original_receiver_before_measurement(programs: &[Program]) {",
        // These two constructors are private to ast::program_source; the joined
        // source keeps their exact facades without granting workspace authority.
        "pub(super) fn append(authored: &'a [T], generated: &'a [T]) -> Option<Self> {",
        "pub(super) fn selected(",
        "pub(super) fn collect_edges(",
        "pub(super) fn verify(",
        "pub(super) fn verify(",
        "pub(super) fn verify(",
    ];
    // The complete actual projection above remains the purity input. This
    // independent historical view retains the original facade assertion too.
    let current_projection_root = projection_root;
    current344_input::refuses_drift(
        "expected_projection::root_join",
        current_projection_root.as_str(),
    );
    let facade_input = current344_input::project(
        "expected_projection::root_join",
        current_projection_root.as_str(),
    );
    let projection_root = historical_projection_root_facade(&facade_input);
    assert_eq!(
        projection_root
            .lines()
            .filter_map(|line| {
                let trimmed = line.trim();
                trimmed.contains("pub(super)").then_some(trimmed)
            })
            .collect::<Vec<_>>(),
        exact_facade,
    );
    let mut current_facade = exact_facade.to_vec();
    let child = current_facade
        .iter()
        .position(|line| *line == "pub(super) fn materialize_receiving(")
        .unwrap()
        + 1;
    current_facade.splice(child..child, [
        "pub(super) fn pay_type(declaration: &TypeDeclaration, owner: &mut Receiver) -> Result<(), Setup> {",
        "pub(super) fn pay_signature(function: &Function, owner: &mut Receiver) -> Result<(), Setup> {",
        "pub(super) fn teardown_type(",
        "pub(super) fn teardown_function(",
        "pub(super) fn pay_default_teardown(",
        "pub(super) fn pay_teardown_minimum(owner: &mut Receiver) -> Result<(), Setup> {",
    ]);
    current_facade.push("pub(super) fn verify(");
    let current_facade_input = current_projection_facade_input(&facade_input);
    assert_eq!(
        current_facade_input
            .lines()
            .filter_map(|line| {
                let trimmed = line.trim();
                trimmed.contains("pub(super)").then_some(trimmed)
            })
            .collect::<Vec<_>>(),
        current_facade
    );

    assert_eq!(
        projection_call_sites
            .lines()
            .filter_map(|line| {
                let trimmed = line.trim();
                trimmed.contains("pub(super)").then_some(trimmed)
            })
            .collect::<Vec<_>>(),
        ["pub(super) fn imported_call_sites<'a>("],
    );

    for helper in [
        "fn reserve_builder_structure(",
        "fn push_edge(",
        "fn budgeted_edge_clone(",
        "fn visit_ast_call_sites(",
    ] {
        assert!(root.contains(helper), "root lost shared helper `{helper}`");
        assert!(
            !projection.contains(helper),
            "projection duplicated shared helper `{helper}`"
        );
    }
    for moved in [
        "struct SyntheticBuilderCosts",
        "fn synthetic_builder_bytes(",
        "fn validate_dependency_dag(",
        "fn dependency_depths<'a>(",
        "fn synthetic_program(",
        "fn collect_expected_edges(",
        "fn verify_resolved_call_edges(",
    ] {
        assert_eq!(
            root.matches(moved).count() + projection.matches(moved).count(),
            1,
            "expected one definition of `{moved}`"
        );
    }

    // Keep the complete child union for structural audits. The native/Wasm
    // fixture driver has host effects only behind its exact test-only module.
    let private_root = semantic_graph_private_layout_source_inventory!();
    assert!(private_root.contains(
        "#[cfg(test)]\n#[path = \"private_layout/function_runtime.rs\"]\nmod function_runtime;"
    ));
    assert_eq!(private_root.matches("mod function_runtime;").count(), 1);
    let runtime_fixture = include_str!(
        "../../src/workspace_graph/expected_projection/private_layout/function_runtime.rs"
    );
    assert_eq!(projection.matches(runtime_fixture).count(), 1);
    // Scoped builder-limit mutation belongs only to this exact test child;
    // its complete bytes still participate in every structural audit above.
    let cost_root = include_str!("../../src/workspace_graph/expected_projection/cost.rs");
    assert!(cost_root.contains(
        "#[cfg(test)]\n#[path = \"uncached_absence_tests.rs\"]\nmod uncached_absence_tests;"
    ));
    assert_eq!(cost_root.matches("mod uncached_absence_tests;").count(), 1);
    let constructor_cost =
        include_str!("../../src/workspace_graph/expected_projection/cost/constructor.rs");
    assert_eq!(projection.matches(constructor_cost).count(), 1);
    let absence_fixture =
        include_str!("../../src/workspace_graph/expected_projection/uncached_absence_tests.rs");
    assert_eq!(projection.matches(absence_fixture).count(), 1);
    // Keep every byte and all six original test-budget TLS references in the
    // raw audit. Its exact cfg(test) registration closes the test authority;
    // production source remains unmodified for every forbidden-word scan.
    let prelude_cost_root =
        include_str!("../../src/workspace_graph/expected_projection/prelude_forecast.rs");
    assert!(prelude_cost_root
        .contains("#[cfg(test)]\n#[path = \"prelude_forecast_tests.rs\"]\nmod tests;"));
    assert_eq!(prelude_cost_root.matches("mod tests;").count(), 1);
    let prelude_fixture =
        include_str!("../../src/workspace_graph/expected_projection/prelude_forecast_tests.rs");
    assert_eq!(projection.matches(prelude_fixture).count(), 1);
    let budget_tls = "crate::workspace_graph::ACTIVE_BUILDER_LIMIT";
    assert_eq!(prelude_fixture.matches(budget_tls).count(), 6);
    assert_eq!(prelude_fixture.matches("ACTIVE").count(), 6);
    for (offset, _) in prelude_fixture.match_indices(budget_tls) {
        assert!(prelude_fixture[offset + budget_tls.len()..].starts_with(".with("));
    }
    // The raw custody union above stays complete, including these exact test
    // children. Authority checks consume the independently closed production
    // partition; no source text is stripped or rewritten for this scan.
    let authority_projection = production_projection.as_str();
    for forbidden in [
        "workspace::",
        "std::fs",
        "std::process",
        "snapshot(",
        "build_authenticated",
        "render_graph_json",
        "write_file",
        "create_dir",
        "remove_file",
        "ACTIVE",
    ] {
        assert!(
            !authority_projection.contains(forbidden),
            "pure expected projection admitted authority or effects through `{forbidden}`"
        );
    }

    let costs = projection.find("struct SyntheticBuilderCosts").unwrap();
    let bytes = projection.find("fn synthetic_builder_bytes(").unwrap();
    let dependency = projection.find("fn validate_dependency_dag(").unwrap();
    let synthetic = projection.find("fn synthetic_program(").unwrap();
    let collect = projection.find("fn collect_expected_edges(").unwrap();
    let verify = projection.find("fn verify_resolved_call_edges(").unwrap();
    assert!(costs < bytes && bytes < dependency && dependency < synthetic);
    assert!(synthetic < collect && collect < verify);

    let collect_source = &projection[collect..verify];
    // Import-free modules return after their capability edges; the imported
    // path reaches the terminal sort. Each path sorts the complete projection
    // exactly once, and the fast path must not admit intervening work.
    assert_eq!(collect_source.matches("edges.sort();").count(), 2);
    assert!(collect_source.contains(
        "if function_uses.is_empty() && type_uses.is_empty() {\n        edges.sort();\n        return Ok(());\n    }"
    ));
    assert!(collect_source.contains("    edges.sort();\n    Ok(())\n}"));
    assert!(!collect_source.contains("workspace_call_sites"));
    let verify_source = &projection[verify..];
    assert!(verify_source.contains("expected.sort();"));
    assert!(verify_source.contains("actual.sort();"));
    assert!(!verify_source.contains("collect_expected_edges("));

    let push = root.find("fn push_edge(").unwrap();
    let clone = root.find("fn budgeted_edge_clone(").unwrap();
    let push_source = &root[push..clone];
    let limit = push_source
        .find("if edges.len() == MAX_CROSS_FILE_EDGES")
        .unwrap();
    let reserve = push_source.find("reserve_builder_structure(").unwrap();
    let append = push_source.find("edges.push(edge)").unwrap();
    assert!(limit < reserve && reserve < append);
}

#[test]
fn public_api_cli_bytes_getters_and_read_only_locking_are_exact() {
    let fixture = ManagedFixture::new("public-success");
    let before = inventory(&fixture.root);
    let shared = fixture.lock();
    fs2::FileExt::try_lock_shared(&shared).unwrap();

    let graph = workspace_graph::snapshot(&fixture.root, "public.app").unwrap();
    capture_pin_candidate("public-graph.json", graph.to_json().as_bytes());
    assert_eq!(graph.workspace_revision(), fixture.revision);
    assert_eq!(
        graph.workspace_manifest_schema(),
        "semaprax.workspace-semantic-manifest.v1"
    );
    assert!(!graph.to_json().ends_with('\n'));
    assert_public_getters_match_wire(&graph);
    assert_eq!(
        graph
            .edges()
            .iter()
            .map(|edge| edge.kind())
            .collect::<std::collections::BTreeSet<_>>(),
        std::collections::BTreeSet::from([
            "call",
            "capability_authority",
            "effect_requirement",
            "function_import",
            "type_import",
            "type_reference",
        ])
    );
    // Imported Point metadata now selects graph.v10; the manifest revision and
    // analysis digest chain follow. The separate, explicitly derived history
    // proof restores this semantic transition before unchanged budget checks.
    // See docs/evidence/managed67b-semantic-history-20261016/README.md.
    assert_eq!(graph.budget().used_builder_bytes(), 973_339);
    assert_eq!(
        document_digest(graph.to_json().as_bytes()),
        "sha256:cda634c1cfeb7fd5a31be9f8332bc5cee0610310379be7510fee23cdc1e669c1"
    );

    let output = Command::new(env!("CARGO_BIN_EXE_semaprax"))
        .arg("workspace-graph")
        .arg(&fixture.root)
        .arg("public.app")
        .output()
        .unwrap();
    assert!(output.status.success());
    assert!(output.stderr.is_empty());
    let mut expected_stdout = graph.to_json().as_bytes().to_vec();
    expected_stdout.push(b'\n');
    assert_eq!(output.stdout, expected_stdout);

    let contender = fixture.lock();
    assert!(fs2::FileExt::try_lock_exclusive(&contender).is_err());
    fs2::FileExt::unlock(&shared).unwrap();
    fs2::FileExt::try_lock_exclusive(&contender).unwrap();
    fs2::FileExt::unlock(&contender).unwrap();
    assert_eq!(inventory(&fixture.root), before);

    let (cli_fixture, init_stdout) = ManagedFixture::new_via_cli("cli-init-success");
    assert_eq!(
        init_stdout,
        format!(
            "initialized semantic graph workspace; workspace is {}\n",
            cli_fixture.revision
        )
        .into_bytes()
    );
    let cli_graph = workspace_graph::snapshot(&cli_fixture.root, "public.app").unwrap();
    assert_eq!(cli_graph.to_json(), graph.to_json());
    assert_eq!(
        std::fs::read_to_string(&cli_fixture.path_set).unwrap(),
        "{\"schema\":\"semaprax.workspace-semantic-path-set.v1\",\"files\":[{\"path\":\"a/provider.spx\"},{\"path\":\"z/app.spx\"}]}\n"
    );
}

#[test]
fn public_api_and_cli_reject_entry_and_arity_errors_exactly() {
    let fixture = ManagedFixture::new("public-errors");
    for (entry, code, message) in [
        (
            "bad-entry",
            "SPX-G170",
            "Workspace Semantic Graph entry module `bad-entry` is not canonical",
        ),
        (
            "missing.module",
            "SPX-G172",
            "Workspace Semantic Graph entry module `missing.module` is absent",
        ),
    ] {
        let error = workspace_graph::snapshot(&fixture.root, entry)
            .err()
            .expect("invalid public entry must return no graph");
        assert_eq!(error.len(), 1);
        assert_eq!(error[0].code, code);
        assert_eq!(error[0].message, message);

        let output = Command::new(env!("CARGO_BIN_EXE_semaprax"))
            .arg("workspace-graph")
            .arg(&fixture.root)
            .arg(entry)
            .output()
            .unwrap();
        assert_eq!(output.status.code(), Some(1));
        assert!(output.stdout.is_empty());
        assert_eq!(
            String::from_utf8(output.stderr).unwrap(),
            format!("error[{code}]: {message}\n")
        );
    }

    let oversized = format!("-private-sentinel-{}", "x".repeat(16 * 1024 * 1024));
    let error = workspace_graph::snapshot(&fixture.root, &oversized)
        .err()
        .expect("oversize public entry must return no graph");
    assert_eq!(error.len(), 1);
    assert_eq!(error[0].code, "SPX-G171");
    assert_eq!(
        error[0].message,
        "Workspace Semantic Graph `entry_module_bytes` exceeds 16777216"
    );
    assert!(!error[0].message.contains("private-sentinel"));

    for arguments in [
        vec!["workspace-graph"],
        vec!["workspace-graph", "root", "entry", "extra"],
    ] {
        let output = Command::new(env!("CARGO_BIN_EXE_semaprax"))
            .args(arguments)
            .output()
            .unwrap();
        assert_eq!(output.status.code(), Some(2));
        assert!(output.stdout.is_empty());
        assert_eq!(
            String::from_utf8(output.stderr).unwrap(),
            "workspace-graph requires exactly <root> <entry-module>\nhint: run `semaprax workspace-graph --help` for usage\n"
        );
    }

    for arguments in [
        vec!["semantic-workspace-init"],
        vec!["semantic-workspace-init", "root", "paths", "extra"],
    ] {
        let output = Command::new(env!("CARGO_BIN_EXE_semaprax"))
            .args(arguments)
            .output()
            .unwrap();
        assert_eq!(output.status.code(), Some(2));
        assert!(output.stdout.is_empty());
        assert_eq!(
            String::from_utf8(output.stderr).unwrap(),
            "semantic-workspace-init requires exactly <root> <path-set.json>\nhint: run `semaprax semantic-workspace-init --help` for usage\n"
        );
    }
}

#[test]
fn semantic_and_ordinary_public_modes_remain_exactly_separate() {
    let (root, path_set) = ManagedFixture::stage("ordinary-import-rejection");
    std::fs::write(
        &path_set,
        "{\"schema\":\"semaprax.workspace-path-set.v1\",\"files\":[{\"path\":\"a/provider.spx\"},{\"path\":\"z/app.spx\"}]}\n",
    )
    .unwrap();
    let error = workspace::initialize(&root, &path_set).unwrap_err();
    assert_eq!(error.len(), 1);
    assert_eq!(error[0].code, "SPX-G172");
    assert_eq!(
        error[0].message,
        "source module imports require Workspace Semantic Graph resolution"
    );
    assert!(!root.join(".semaprax-workspace").exists());
    std::fs::remove_dir_all(&root).unwrap();

    let (root, path_set) = ManagedFixture::stage("semantic-schema-rejection");
    std::fs::write(
        &path_set,
        "{\"schema\":\"semaprax.workspace-path-set.v1\",\"files\":[{\"path\":\"a/provider.spx\"},{\"path\":\"z/app.spx\"}]}\n",
    )
    .unwrap();
    let error = semantic_workspace::initialize(&root, &path_set).unwrap_err();
    assert_eq!(error.len(), 1);
    assert_eq!(error[0].code, "SPX-G174");
    assert_eq!(
        error[0].message,
        "semantic workspace path set is not canonical semaprax.workspace-semantic-path-set.v1"
    );
    assert!(!root.join(".semaprax-workspace").exists());
    std::fs::remove_dir_all(&root).unwrap();

    let (ordinary_root, ordinary_path_set) = ManagedFixture::stage("ordinary-preservation");
    for (path, source) in [
        (
            "a/provider.spx",
            "module ordinary.provider; @id(\"ordinary.provider.main\") fn main()->i64{0}",
        ),
        (
            "z/app.spx",
            "module ordinary.app; @id(\"ordinary.app.main\") fn main()->i64{1}",
        ),
    ] {
        let program = parse(source, path).unwrap();
        std::fs::write(ordinary_root.join(path), format::canonical(&program)).unwrap();
    }
    std::fs::write(
        &ordinary_path_set,
        "{\"schema\":\"semaprax.workspace-path-set.v1\",\"files\":[{\"path\":\"a/provider.spx\"},{\"path\":\"z/app.spx\"}]}\n",
    )
    .unwrap();
    let revision = workspace::initialize(&ordinary_root, &ordinary_path_set).unwrap();
    assert_eq!(
        workspace::snapshot(&ordinary_root)
            .unwrap()
            .workspace_revision(),
        revision
    );
    let error = workspace_graph::snapshot(&ordinary_root, "ordinary.app")
        .err()
        .expect("ordinary Workspace v1 root must not be accepted as semantic");
    assert_eq!(error.len(), 1);
    assert_eq!(error[0].code, "SPX-G174");
    assert_eq!(
        error[0].message,
        "managed workspace is not a semaprax.workspace-semantic-root.v1 workspace"
    );
    std::fs::remove_dir_all(&ordinary_root).unwrap();
}

fn assert_cli_artifact(arguments: &[&str], expected: &str) {
    let output = Command::new(env!("CARGO_BIN_EXE_semaprax"))
        .args(arguments)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(output.stderr.is_empty());
    let mut expected_stdout = expected.as_bytes().to_vec();
    expected_stdout.push(b'\n');
    assert_eq!(output.stdout, expected_stdout);
}

#[test]
fn public_workspace_analysis_api_cli_kats_and_locking_are_exact() {
    let fixture = ManagedFixture::new("public-analysis-success");
    let before = inventory(&fixture.root);
    let shared = fixture.lock();
    fs2::FileExt::try_lock_shared(&shared).unwrap();

    let context_options = WorkspaceContextOptions::default();
    let context = workspace_analysis::context(
        &fixture.root,
        "public.app",
        WorkspaceAnalysisTargetKind::Declaration,
        "public.main",
        context_options,
    )
    .unwrap();
    assert!(!context.ends_with('\n'));
    let context_wire: serde_json::Value = serde_json::from_str(&context).unwrap();
    assert_eq!(context_wire["query"]["direction"], "both");
    assert_eq!(context_wire["query"]["depth"], 4);
    assert_eq!(context_wire["query"]["max_bytes"], 1024 * 1024);
    assert_eq!(context_wire["query"]["max_nodes"], 1024);
    assert_cli_artifact(
        &[
            "workspace-context",
            fixture.root.to_str().unwrap(),
            "public.app",
            "declaration",
            "public.main",
        ],
        &context,
    );

    let forward_options =
        WorkspaceContextOptions::new(WorkspaceAnalysisDirection::Forward, 2, 1024 * 1024, 64)
            .unwrap();
    let forward = workspace_analysis::context(
        &fixture.root,
        "public.app",
        WorkspaceAnalysisTargetKind::Declaration,
        "public.main",
        forward_options,
    )
    .unwrap();
    assert_cli_artifact(
        &[
            "workspace-context",
            fixture.root.to_str().unwrap(),
            "public.app",
            "declaration",
            "public.main",
            "--max-nodes",
            "64",
            "--direction",
            "forward",
            "--max-bytes",
            "1048576",
            "--depth",
            "2",
        ],
        &forward,
    );

    let capability_context = workspace_analysis::context(
        &fixture.root,
        "public.app",
        WorkspaceAnalysisTargetKind::Capability,
        "audit.write",
        WorkspaceContextOptions::default(),
    )
    .unwrap();
    assert_eq!(
        serde_json::from_str::<serde_json::Value>(&capability_context).unwrap()["target"]["kind"],
        "capability"
    );
    let type_context = workspace_analysis::context(
        &fixture.root,
        "public.app",
        WorkspaceAnalysisTargetKind::Declaration,
        "public.point",
        WorkspaceContextOptions::default(),
    )
    .unwrap();

    let impact_options = WorkspaceImpactOptions::default();
    let impact = workspace_analysis::impact(
        &fixture.root,
        "public.app",
        WorkspaceAnalysisTargetKind::Declaration,
        "public.work",
        impact_options,
    )
    .unwrap();
    assert!(!impact.ends_with('\n'));
    let impact_wire: serde_json::Value = serde_json::from_str(&impact).unwrap();
    assert_eq!(impact_wire["query"]["direction"], "reverse");
    assert_eq!(impact_wire["query"]["depth"], 16);
    assert_eq!(impact_wire["query"]["max_bytes"], 1024 * 1024);
    assert_eq!(impact_wire["query"]["max_nodes"], 1024);
    let mut edge_kinds = std::collections::BTreeSet::new();
    for (document, relation) in [
        (&context, "edges"),
        (&capability_context, "edges"),
        (&type_context, "edges"),
        (&impact, "dependency_edges"),
    ] {
        let wire: serde_json::Value = serde_json::from_str(document).unwrap();
        edge_kinds.extend(
            wire[relation]
                .as_array()
                .unwrap()
                .iter()
                .map(|edge| edge["kind"].as_str().unwrap().to_owned()),
        );
    }
    assert_eq!(
        edge_kinds,
        std::collections::BTreeSet::from(
            [
                "call",
                "capability_authority",
                "effect_requirement",
                "function_import",
                "type_import",
                "type_reference",
            ]
            .map(str::to_owned)
        )
    );
    assert_cli_artifact(
        &[
            "workspace-impact",
            fixture.root.to_str().unwrap(),
            "public.app",
            "declaration",
            "public.work",
        ],
        &impact,
    );
    let custom_impact = workspace_analysis::impact(
        &fixture.root,
        "public.app",
        WorkspaceAnalysisTargetKind::Capability,
        "audit.write",
        WorkspaceImpactOptions::new(3, 1024 * 1024, 64).unwrap(),
    )
    .unwrap();
    assert_cli_artifact(
        &[
            "workspace-impact",
            fixture.root.to_str().unwrap(),
            "public.app",
            "capability",
            "audit.write",
            "--max-nodes",
            "64",
            "--max-bytes",
            "1048576",
            "--depth",
            "3",
        ],
        &custom_impact,
    );

    let review = workspace_analysis::review(
        &fixture.root,
        "public.app",
        WorkspaceAnalysisTargetKind::Declaration,
        "public.work",
    )
    .unwrap();
    assert!(!review.ends_with('\n'));
    assert_cli_artifact(
        &[
            "workspace-review",
            fixture.root.to_str().unwrap(),
            "public.app",
            "declaration",
            "public.work",
        ],
        &review,
    );
    let capability_review = workspace_analysis::review(
        &fixture.root,
        "public.app",
        WorkspaceAnalysisTargetKind::Capability,
        "audit.write",
    )
    .unwrap();
    assert_eq!(
        serde_json::from_str::<serde_json::Value>(&capability_review).unwrap()["target"]["kind"],
        "capability"
    );

    for (name, wire) in [
        ("context.json", &context),
        ("forward.json", &forward),
        ("capability-context.json", &capability_context),
        ("type-context.json", &type_context),
        ("impact.json", &impact),
        ("custom-impact.json", &custom_impact),
        ("review.json", &review),
        ("capability-review.json", &capability_review),
    ] {
        capture_pin_candidate(name, wire.as_bytes());
    }

    // Full current reports reproduce all eight original hashes by restoring
    // only the measured workspace debit and authenticated digest bindings.
    assert_eq!(
        [
            document_digest(context.as_bytes()),
            document_digest(forward.as_bytes()),
            document_digest(capability_context.as_bytes()),
            document_digest(type_context.as_bytes()),
            document_digest(impact.as_bytes()),
            document_digest(custom_impact.as_bytes()),
            document_digest(review.as_bytes()),
            document_digest(capability_review.as_bytes()),
        ],
        [
            "sha256:26c450e0185cbe5de20633e074f7d38455ed04ceb68651adbd2459e0e496b2a8",
            "sha256:7c7ae2cedc6ee83ba9814c28b1a6b0a96fb8b85cfb7bd65cca628fe9242222af",
            "sha256:3be3b04b2cbc5f8abef6c584deed7f854ec22189134a4b67db71cfd416d6dfb1",
            "sha256:bb4991566a276f76d7970f49def1184a9047ecf4af0a166b9e642a5598583a73",
            "sha256:d6e4f39c11c6c2d6ed7ec3e5138d787583961a3669a648daa9085236b3e3e7ee",
            "sha256:720704c7baf6b69411b6357041512416c02ece3305fa0ff034d580ddc93a87a7",
            "sha256:93cd7596ab762e34654581f0530368afbe6896e0a395149609711fc9e2b89b3f",
            "sha256:64807a35fbe94bfa66201dd3db1468712b93f28b49d98ccc70eeb33460c1e2a0",
        ]
    );

    std::thread::scope(|scope| {
        let context_task = scope.spawn(|| {
            workspace_analysis::context(
                &fixture.root,
                "public.app",
                WorkspaceAnalysisTargetKind::Declaration,
                "public.main",
                WorkspaceContextOptions::default(),
            )
            .unwrap()
        });
        let impact_task = scope.spawn(|| {
            workspace_analysis::impact(
                &fixture.root,
                "public.app",
                WorkspaceAnalysisTargetKind::Declaration,
                "public.work",
                WorkspaceImpactOptions::default(),
            )
            .unwrap()
        });
        let review_task = scope.spawn(|| {
            workspace_analysis::review(
                &fixture.root,
                "public.app",
                WorkspaceAnalysisTargetKind::Declaration,
                "public.work",
            )
            .unwrap()
        });
        assert_eq!(context_task.join().unwrap(), context);
        assert_eq!(impact_task.join().unwrap(), impact);
        assert_eq!(review_task.join().unwrap(), review);
    });

    let contender = fixture.lock();
    assert!(fs2::FileExt::try_lock_exclusive(&contender).is_err());
    fs2::FileExt::unlock(&shared).unwrap();
    fs2::FileExt::try_lock_exclusive(&contender).unwrap();
    fs2::FileExt::unlock(&contender).unwrap();
    assert_eq!(inventory(&fixture.root), before);
}

#[test]
fn public_workspace_analysis_cli_hostiles_and_mode_separation_are_exact() {
    let fixture = ManagedFixture::new("public-analysis-hostile");
    let root = fixture.root.to_str().unwrap();
    for (arguments, expected) in [
        (
            vec!["workspace-context"],
            "workspace-context requires <root> <entry-module> <declaration|capability> <target> [--direction forward|reverse|both] [--depth N] [--max-bytes N] [--max-nodes N]\nhint: run `semaprax workspace-context --help` for usage\n",
        ),
        (
            vec![
                "workspace-context",
                root,
                "public.app",
                "other",
                "public.work",
            ],
            "workspace-context target kind must be `declaration` or `capability`\nhint: run `semaprax workspace-context --help` for usage\n",
        ),
        (
            vec![
                "workspace-context",
                root,
                "public.app",
                "declaration",
                "public.work",
                "--unknown",
                "1",
            ],
            "unknown workspace-context option `--unknown`\nhint: run `semaprax workspace-context --help` for usage\n",
        ),
        (
            vec![
                "workspace-context",
                root,
                "public.app",
                "declaration",
                "public.work",
                "--depth",
                "1",
                "--depth",
                "2",
            ],
            "duplicate workspace-context option `--depth`\nhint: run `semaprax workspace-context --help` for usage\n",
        ),
        (
            vec![
                "workspace-context",
                root,
                "public.app",
                "declaration",
                "public.work",
                "--depth",
            ],
            "workspace-context option `--depth` requires a value\nhint: run `semaprax workspace-context --help` for usage\n",
        ),
        (
            vec![
                "workspace-context",
                root,
                "public.app",
                "declaration",
                "public.work",
                "--direction",
                "sideways",
            ],
            "unknown workspace-context direction `sideways`\nhint: run `semaprax workspace-context --help` for usage\n",
        ),
        (
            vec![
                "workspace-context",
                root,
                "public.app",
                "declaration",
                "public.work",
                "--max-nodes",
                "01",
            ],
            "workspace-context option `--max-nodes` requires a canonical nonnegative integer\nhint: run `semaprax workspace-context --help` for usage\n",
        ),
        (
            vec!["workspace-impact"],
            "workspace-impact requires <root> <entry-module> <declaration|capability> <target> [--depth N] [--max-bytes N] [--max-nodes N]\nhint: run `semaprax workspace-impact --help` for usage\n",
        ),
        (
            vec![
                "workspace-impact",
                root,
                "public.app",
                "declaration",
                "public.work",
                "--unknown",
                "1",
            ],
            "unknown workspace-impact option `--unknown`\nhint: run `semaprax workspace-impact --help` for usage\n",
        ),
        (
            vec![
                "workspace-impact",
                root,
                "public.app",
                "declaration",
                "public.work",
                "--depth",
                "1",
                "--depth",
                "2",
            ],
            "duplicate workspace-impact option `--depth`\nhint: run `semaprax workspace-impact --help` for usage\n",
        ),
        (
            vec![
                "workspace-impact",
                root,
                "public.app",
                "declaration",
                "public.work",
                "--max-bytes",
            ],
            "workspace-impact option `--max-bytes` requires a value\nhint: run `semaprax workspace-impact --help` for usage\n",
        ),
        (
            vec![
                "workspace-review",
                root,
                "public.app",
                "declaration",
                "public.work",
                "extra",
            ],
            "workspace-review requires exactly <root> <entry-module> <declaration|capability> <target>\nhint: run `semaprax workspace-review --help` for usage\n",
        ),
    ] {
        let output = Command::new(env!("CARGO_BIN_EXE_semaprax"))
            .args(arguments)
            .output()
            .unwrap();
        assert_eq!(output.status.code(), Some(2));
        assert!(output.stdout.is_empty());
        assert_eq!(String::from_utf8(output.stderr).unwrap(), expected);
    }

    let (ordinary_root, ordinary_path_set) = ManagedFixture::stage("analysis-ordinary-mode");
    for (path, source) in [
        (
            "a/provider.spx",
            "module ordinary.provider; @id(\"ordinary.provider.main\") fn main()->i64{0}",
        ),
        (
            "z/app.spx",
            "module ordinary.app; @id(\"ordinary.app.main\") fn main()->i64{1}",
        ),
    ] {
        let program = parse(source, path).unwrap();
        std::fs::write(ordinary_root.join(path), format::canonical(&program)).unwrap();
    }
    std::fs::write(
        &ordinary_path_set,
        "{\"schema\":\"semaprax.workspace-path-set.v1\",\"files\":[{\"path\":\"a/provider.spx\"},{\"path\":\"z/app.spx\"}]}\n",
    )
    .unwrap();
    workspace::initialize(&ordinary_root, &ordinary_path_set).unwrap();
    let expected = "managed workspace is not a semaprax.workspace-semantic-root.v1 workspace";
    for result in [
        workspace_analysis::context(
            &ordinary_root,
            "ordinary.app",
            WorkspaceAnalysisTargetKind::Declaration,
            "ordinary.app.main",
            WorkspaceContextOptions::default(),
        ),
        workspace_analysis::impact(
            &ordinary_root,
            "ordinary.app",
            WorkspaceAnalysisTargetKind::Declaration,
            "ordinary.app.main",
            WorkspaceImpactOptions::default(),
        ),
        workspace_analysis::review(
            &ordinary_root,
            "ordinary.app",
            WorkspaceAnalysisTargetKind::Declaration,
            "ordinary.app.main",
        ),
    ] {
        let error = result.expect_err("ordinary mode must return no artifact");
        assert_eq!(error.len(), 1);
        assert_eq!(error[0].code, "SPX-G174");
        assert_eq!(error[0].message, expected);
    }
    std::fs::remove_dir_all(&ordinary_root).unwrap();
}

// The historical equality remains complete and rejects any raw join drift.
fn historical_projection_root_facade(current: &str) -> String {
    use sha2::{Digest, Sha256};
    let row: serde_json::Value = serde_json::from_str(include_str!(
        "../../docs/evidence/projection-facade-frontier-8bb-20261016/root-input.json"
    ))
    .unwrap();
    assert_eq!(
        format!(
            "{:x}",
            semaprax::digest_hex::LowerHex(Sha256::digest(current.as_bytes()))
        ),
        row["current_sha256"]
    );
    let mut previous = current.to_owned();
    for edit in row["edits"].as_array().unwrap() {
        let new = edit["new"].as_str().unwrap();
        assert_eq!(
            previous.matches(new).count(),
            1,
            "complete unique facade input inverse"
        );
        previous = previous.replacen(new, edit["old"].as_str().unwrap(), 1);
    }
    assert_eq!(
        format!(
            "{:x}",
            semaprax::digest_hex::LowerHex(Sha256::digest(previous.as_bytes()))
        ),
        row["original_sha256"]
    );
    historical_projection_root_facade_before(&previous)
}

fn historical_projection_root_facade_before(current: &str) -> String {
    use sha2::{Digest, Sha256};
    let row: serde_json::Value = serde_json::from_str(include_str!(
        "../../docs/evidence/ae91-source-consumer-corrections-20261015/facade.json"
    ))
    .unwrap();
    assert_eq!(
        format!(
            "{:x}",
            semaprax::digest_hex::LowerHex(Sha256::digest(current.as_bytes()))
        ),
        row["current_sha256"]
    );
    assert_eq!(current, row["edits"][0]["new"].as_str().unwrap());
    let original = row["edits"][0]["old"].as_str().unwrap();
    assert_eq!(
        format!(
            "{:x}",
            semaprax::digest_hex::LowerHex(Sha256::digest(original.as_bytes()))
        ),
        row["original_sha256"]
    );
    original.to_owned()
}

#[cfg(test)]
#[path = "../support/production_source_inventory.rs"]
mod production_source_inventory;

// Full pre847 declaration-source view for the unchanged historical facade list.
// Current production purity above and the entire current projected method stay audited.
fn current_projection_facade_input(current: &str) -> String {
    use sha2::{Digest, Sha256};
    let row: serde_json::Value = serde_json::from_str(include_str!(
        "../../docs/evidence/actual50-fixture-and-facade-corrections-20261016/current-facade-input.json"
    )).unwrap();
    let actual = include_str!("../../src/ast/program_source/decl_slices.rs");
    assert_eq!(
        format!(
            "{:x}",
            semaprax::digest_hex::LowerHex(Sha256::digest(actual.as_bytes()))
        ),
        row["current_decl_sha256"]
    );
    assert_eq!(
        current.matches(actual).count(),
        1,
        "complete actual declaration owner in raw facade union"
    );
    assert!(actual.contains("impl<'a, T> DeclSlices<'a, T> {"));
    let method = row["actual_projected_method"].as_str().unwrap();
    assert!(method.starts_with("    pub(super) fn projected("));
    assert_eq!(
        actual.matches(method).count(),
        1,
        "complete current parent-private projected method"
    );
    assert_eq!(
        format!(
            "{:x}",
            semaprax::digest_hex::LowerHex(Sha256::digest(current.as_bytes()))
        ),
        row["current_sha256"]
    );
    let mut previous = current.to_owned();
    for edit in row["edits"].as_array().unwrap() {
        let new = edit["new"].as_str().unwrap();
        assert_eq!(new, actual, "complete current declaration source input");
        assert_eq!(previous.matches(new).count(), 1);
        previous = previous.replacen(new, edit["old"].as_str().unwrap(), 1);
    }
    assert_eq!(
        format!(
            "{:x}",
            semaprax::digest_hex::LowerHex(Sha256::digest(previous.as_bytes()))
        ),
        row["original_sha256"]
    );
    previous
}

// The purity gate above still consumes the complete actual join. Only its
// independent historical facade equality receives this authenticated bridge.
#[path = "../../src/source_reflows_merge/opt858_sequence_input.rs"]
mod opt858_sequence_input;
// Keep the shared source reader and its authenticated outer adapter together.
#[path = "../../src/source_reflows_merge/main67107_inputs.rs"]
mod main67107_inputs;
#[path = "../../src/source_reflows_merge/ordinary_plan_inputs.rs"]
mod ordinary_plan_inputs;
#[path = "../../src/source_reflows_merge/final_ready_inputs.rs"]
mod final_ready_inputs;
mod current344_input {
    include!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/src/source_reflows_merge/current344.rs"
    ));
}

#[test]
fn private_layout_inventory_retains_every_full_input_and_every_current_owned_module() {
    source_inventory_controls::verify(
        Path::new(env!("CARGO_MANIFEST_DIR")),
        SEMANTIC_GRAPH_PRIVATE_LAYOUT_ROOT,
        &semantic_graph_private_layout_source_inventory!(),
        &source_inventory_controls::current_owned_source_inventory(),
        include_str!("../../docs/evidence/workspace21-legacy-source-repair-20261017/current-projection-inputs.tsv"),
    );
}
