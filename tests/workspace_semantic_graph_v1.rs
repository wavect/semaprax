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
fn public_api_cli_bytes_getters_and_read_only_locking_are_exact() {
    let fixture = ManagedFixture::new("public-success");
    let before = inventory(&fixture.root);
    let shared = fixture.lock();
    fs2::FileExt::try_lock_shared(&shared).unwrap();

    let graph = workspace_graph::snapshot(&fixture.root, "public.app").unwrap();
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
    assert_eq!(
        document_digest(graph.to_json().as_bytes()),
        "sha256:64dddc0c2046766640ec93b7a7249214d099f683a2b6f26f43cdc22073764a6c"
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
            "workspace-graph requires exactly <root> <entry-module>\n"
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
            "semantic-workspace-init requires exactly <root> <path-set.json>\n"
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
            "sha256:f3f1ffbf87bd0c491623a409ce3d2994cadce0f3e07fe72bffe2901a0ff8856b",
            "sha256:403b087739868747ac1e46f61d867e835127de5f17780006fe7c002b01b0cbca",
            "sha256:fb4ac64fd18c56b3320c0f36a0af93e59194577846a09b11bd1266cdee6ee9c2",
            "sha256:c2c35eb2e7c1646a6df84e65d46fd314734c59a934682675ed15ac15378102ef",
            "sha256:bf30e56b98f00624c190842f071064e4ce58c940970b25b9d7642cc06ff624af",
            "sha256:dd81f94016d25d1a12647b88128d8ac122c6fe646eb677e1d602b42d5b24bd49",
            "sha256:e356fc8b41ebd7163f7d406dcb06055d7ba265edb2ea8677972f2b3f0ebc31c2",
            "sha256:cdef6e5618e0013d38a2b058ecdb6c0efad7a1ab8270cbc3399cf31779ebfc63",
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
            "workspace-context requires <root> <entry-module> <declaration|capability> <target> [--direction forward|reverse|both] [--depth N] [--max-bytes N] [--max-nodes N]\n",
        ),
        (
            vec!["workspace-context", root, "public.app", "other", "public.work"],
            "workspace-context target kind must be `declaration` or `capability`\n",
        ),
        (
            vec!["workspace-context", root, "public.app", "declaration", "public.work", "--unknown", "1"],
            "unknown workspace-context option `--unknown`\n",
        ),
        (
            vec!["workspace-context", root, "public.app", "declaration", "public.work", "--depth", "1", "--depth", "2"],
            "duplicate workspace-context option `--depth`\n",
        ),
        (
            vec!["workspace-context", root, "public.app", "declaration", "public.work", "--depth"],
            "workspace-context option `--depth` requires a value\n",
        ),
        (
            vec!["workspace-context", root, "public.app", "declaration", "public.work", "--direction", "sideways"],
            "unknown workspace-context direction `sideways`\n",
        ),
        (
            vec!["workspace-context", root, "public.app", "declaration", "public.work", "--max-nodes", "01"],
            "workspace-context option `--max-nodes` requires a canonical nonnegative integer\n",
        ),
        (
            vec!["workspace-impact"],
            "workspace-impact requires <root> <entry-module> <declaration|capability> <target> [--depth N] [--max-bytes N] [--max-nodes N]\n",
        ),
        (
            vec!["workspace-impact", root, "public.app", "declaration", "public.work", "--unknown", "1"],
            "unknown workspace-impact option `--unknown`\n",
        ),
        (
            vec!["workspace-impact", root, "public.app", "declaration", "public.work", "--depth", "1", "--depth", "2"],
            "duplicate workspace-impact option `--depth`\n",
        ),
        (
            vec!["workspace-impact", root, "public.app", "declaration", "public.work", "--max-bytes"],
            "workspace-impact option `--max-bytes` requires a value\n",
        ),
        (
            vec!["workspace-review", root, "public.app", "declaration", "public.work", "extra"],
            "workspace-review requires exactly <root> <entry-module> <declaration|capability> <target>\n",
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
