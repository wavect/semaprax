use super::*;
use std::fs::OpenOptions;
use std::io::Write as _;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering as AtomicOrdering};

static SERIAL: AtomicU64 = AtomicU64::new(0);

struct Fixture {
    root: PathBuf,
}

impl Fixture {
    fn new() -> Self {
        let serial = SERIAL.fetch_add(1, AtomicOrdering::Relaxed);
        let root = std::env::temp_dir().join(format!(
            "semaprax-workspace-analysis-private-{}-{serial}",
            std::process::id()
        ));
        std::fs::create_dir(&root).unwrap();
        for (path, source) in [
            (
                "a/provider.spx",
                r#"
module collision.same;
permit { collision.same }

@id("collision.point")
record Point { @id("collision.point.value") value: i64, }

@id("collision.same")
fn work(value: Point) -> i64 uses { collision.same } { value.value }

@id("collision.a") fn a() -> i64 { 1 }
@id("collision.b") fn b() -> i64 { 2 }
@id("collision.leaf") fn leaf() -> i64 { 3 }
fn helper() -> i64 { 4 }
@id("collision.provider.main") fn main() -> i64 { 0 }
"#,
            ),
            (
                "z/entry.spx",
                r#"
module entry.app;
use type @id("collision.point") from collision.same as Point;
use function @id("collision.same") from collision.same as work;
permit { collision.same }

@id("entry.main")
fn main() -> i64 uses { collision.same } {
    work(Point { value: 1 })
}
"#,
            ),
        ] {
            let destination = root.join(path);
            std::fs::create_dir_all(destination.parent().unwrap()).unwrap();
            let program = crate::parse(source, Path::new(path)).unwrap();
            std::fs::write(destination, crate::format::canonical(&program)).unwrap();
        }
        let paths = root.join("paths.json");
        std::fs::write(
                &paths,
                "{\"schema\":\"semaprax.workspace-semantic-path-set.v1\",\"files\":[{\"path\":\"a/provider.spx\"},{\"path\":\"z/entry.spx\"}]}\n",
            )
            .unwrap();
        crate::semantic_workspace::initialize(&root, &paths).unwrap();
        Self { root }
    }

    fn analysis(&self) -> WorkspaceAnalysis {
        crate::workspace_graph::build_authenticated_analysis_for_test(&self.root, "entry.app")
            .unwrap()
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.root);
    }
}

fn code<T>(result: Result<T, Vec<Diagnostic>>) -> &'static str {
    result.err().expect("hostile case must fail")[0].code
}

fn declaration(id: &str) -> WorkspaceAnalysisNode {
    WorkspaceAnalysisNode::Declaration(id.to_owned())
}

fn capability(id: &str) -> WorkspaceAnalysisNode {
    WorkspaceAnalysisNode::Capability(id.to_owned())
}

fn add_typed_edge(
    analysis: &mut WorkspaceAnalysis,
    source: WorkspaceAnalysisNode,
    target: WorkspaceAnalysisNode,
    family: WorkspaceAnalysisEdgeFamily,
) -> usize {
    let index = analysis.typed_edges.len();
    let (caller_path, caller) = match &source {
        WorkspaceAnalysisNode::Module { path, module } => (path.clone(), module.clone()),
        WorkspaceAnalysisNode::Declaration(id) => (
            analysis.declarations[id].path.clone().unwrap_or_default(),
            id.clone(),
        ),
        WorkspaceAnalysisNode::Capability(name) => (String::new(), name.clone()),
    };
    let (target_path, target_name) = match &target {
        WorkspaceAnalysisNode::Module { path, module } => (path.clone(), module.clone()),
        WorkspaceAnalysisNode::Declaration(id) => (
            analysis.declarations[id].path.clone().unwrap_or_default(),
            id.clone(),
        ),
        WorkspaceAnalysisNode::Capability(name) => (caller_path.clone(), name.clone()),
    };
    analysis.projection.push_analysis_test_edge(
        caller_path,
        caller,
        target_path,
        target_name,
        family.name(),
    );
    analysis.typed_edges.push(WorkspaceAnalysisTypedEdge {
        source: clone_node(&source),
        target: clone_node(&target),
        family,
    });
    analysis.forward.entry(source).or_default().push(index);
    analysis.reverse.entry(target).or_default().push(index);
    index
}

#[test]
fn typed_selectors_keep_module_declaration_and_capability_namespaces_distinct() {
    let analysis = Fixture::new().analysis();
    let declaration_target = WorkspaceAnalysisTarget::declaration("collision.same").unwrap();
    let capability_target = WorkspaceAnalysisTarget::capability("collision.same").unwrap();
    assert_eq!(
        analysis.select(&declaration_target).unwrap(),
        declaration("collision.same")
    );
    assert_eq!(
        analysis.select(&capability_target).unwrap(),
        capability("collision.same")
    );
    assert!(analysis.modules.keys().any(|node| matches!(
        node,
        WorkspaceAnalysisNode::Module { module, .. } if module == "collision.same"
    )));

    let automatic = analysis
        .declarations
        .iter()
        .find(|(_, fact)| fact.origin == IdentityOrigin::Automatic)
        .map(|(id, _)| id.clone())
        .unwrap();
    let facts = analysis
        .context(
            WorkspaceAnalysisTarget::declaration(&automatic).unwrap(),
            WorkspaceAnalysisDirection::Forward,
            0,
            1,
        )
        .unwrap();
    assert_eq!(
        facts.nodes[0].identity_origin,
        Some(IdentityOrigin::Automatic)
    );

    let compiler = analysis
        .declarations
        .iter()
        .find(|(_, fact)| fact.origin == IdentityOrigin::CompilerOwned)
        .map(|(id, _)| id.clone())
        .unwrap();
    assert_eq!(
        code(analysis.context(
            WorkspaceAnalysisTarget::declaration(&compiler).unwrap(),
            WorkspaceAnalysisDirection::Forward,
            0,
            1,
        )),
        "SPX-G177"
    );
    for target in [
        WorkspaceAnalysisTarget::declaration("absent").unwrap(),
        WorkspaceAnalysisTarget::capability("absent").unwrap(),
    ] {
        assert_eq!(
            code(analysis.context(target, WorkspaceAnalysisDirection::Forward, 0, 1)),
            "SPX-G177"
        );
    }
}

#[test]
fn six_family_indexes_and_typed_endpoint_replay_are_exact_and_ordered() {
    let analysis = Fixture::new().analysis();
    assert_eq!(
        EDGE_FAMILIES.map(WorkspaceAnalysisEdgeFamily::name),
        [
            "function_import",
            "type_import",
            "call",
            "type_reference",
            "effect_requirement",
            "capability_authority",
        ]
    );
    for family in EDGE_FAMILIES {
        assert!(analysis.family_edges(family).next().is_some(), "{family:?}");
    }
    validate_adjacency_replay(
        &analysis.typed_edges,
        &analysis.families,
        &analysis.forward,
        &analysis.reverse,
    )
    .unwrap();
    for (raw, typed) in analysis.edges().iter().zip(&analysis.typed_edges) {
        replay_typed_edge(
            raw,
            typed,
            &analysis.modules,
            &analysis.declarations,
            &analysis.capabilities,
        )
        .unwrap();
    }

    let mut substituted = analysis.typed_edges.clone();
    substituted[0].family = WorkspaceAnalysisEdgeFamily::Call;
    assert_eq!(
        code(replay_typed_edge(
            &analysis.edges()[0],
            &substituted[0],
            &analysis.modules,
            &analysis.declarations,
            &analysis.capabilities,
        )),
        "SPX-G179"
    );
    let mut duplicate = analysis.forward.clone();
    duplicate.values_mut().next().unwrap().push(0);
    assert_eq!(
        code(validate_adjacency_replay(
            &analysis.typed_edges,
            &analysis.families,
            &duplicate,
            &analysis.reverse,
        )),
        "SPX-G179"
    );
    let mut orphan = analysis.reverse.clone();
    let key = orphan
        .iter()
        .find(|(_, indexes)| !indexes.is_empty())
        .map(|(key, _)| key.clone())
        .unwrap();
    orphan.get_mut(&key).unwrap().pop();
    assert_eq!(
        code(validate_adjacency_replay(
            &analysis.typed_edges,
            &analysis.families,
            &analysis.forward,
            &orphan,
        )),
        "SPX-G179"
    );
}

#[test]
fn context_and_impact_preserve_minimum_depth_ties_and_exact_path_edges() {
    let mut analysis = Fixture::new().analysis();
    let root = declaration("collision.same");
    let a = declaration("collision.a");
    let b = declaration("collision.b");
    let leaf = declaration("collision.leaf");
    add_typed_edge(
        &mut analysis,
        root.clone(),
        a.clone(),
        WorkspaceAnalysisEdgeFamily::Call,
    );
    add_typed_edge(
        &mut analysis,
        root.clone(),
        b.clone(),
        WorkspaceAnalysisEdgeFamily::Call,
    );
    add_typed_edge(
        &mut analysis,
        a.clone(),
        leaf.clone(),
        WorkspaceAnalysisEdgeFamily::Call,
    );
    add_typed_edge(
        &mut analysis,
        b.clone(),
        leaf.clone(),
        WorkspaceAnalysisEdgeFamily::Call,
    );
    add_typed_edge(
        &mut analysis,
        root.clone(),
        leaf.clone(),
        WorkspaceAnalysisEdgeFamily::Call,
    );
    add_typed_edge(
        &mut analysis,
        leaf.clone(),
        root.clone(),
        WorkspaceAnalysisEdgeFamily::Call,
    );

    let both = analysis
        .context(
            WorkspaceAnalysisTarget::declaration("collision.same").unwrap(),
            WorkspaceAnalysisDirection::Both,
            3,
            32,
        )
        .unwrap();
    let leaf_fact = both.nodes.iter().find(|fact| fact.node == leaf).unwrap();
    assert_eq!(leaf_fact.depth, 1);
    assert_eq!(
        leaf_fact.reached_by,
        [
            WorkspaceAnalysisReachedBy::Forward,
            WorkspaceAnalysisReachedBy::Reverse,
        ]
    );
    assert!(both.path_edges.iter().any(|edge| {
        edge.reached_by == WorkspaceAnalysisReachedBy::Forward
            && analysis.typed_edges[edge.edge_index].source == root
    }));
    assert!(both.path_edges.iter().any(|edge| {
        edge.reached_by == WorkspaceAnalysisReachedBy::Reverse
            && analysis.typed_edges[edge.edge_index].target == root
    }));

    let impact = analysis
        .impact(
            WorkspaceAnalysisTarget::declaration("collision.leaf").unwrap(),
            3,
            32,
        )
        .unwrap();
    assert!(impact.path_edges.iter().all(|edge| {
        edge.reached_by == WorkspaceAnalysisReachedBy::Reverse
            && edge.depth
                == impact
                    .nodes
                    .iter()
                    .find(|fact| fact.node.node == analysis.typed_edges[edge.edge_index].source)
                    .unwrap()
                    .node
                    .depth
    }));
    assert!(impact.nodes.iter().any(|fact| {
        matches!(fact.node.node, WorkspaceAnalysisNode::Module { .. })
            && fact.role == WorkspaceImpactRole::ModuleConsumer
    }));
}

#[test]
fn truncation_uses_bfs_depth_before_kind_and_deduplicates_deferred_nodes() {
    let mut analysis = Fixture::new().analysis();
    let root = declaration("collision.same");
    let near_cap = capability("near.cap");
    let far_module = analysis.modules.keys().next().unwrap().clone();
    analysis.capabilities.insert("near.cap".to_owned());
    add_typed_edge(
        &mut analysis,
        root.clone(),
        near_cap.clone(),
        WorkspaceAnalysisEdgeFamily::EffectRequirement,
    );
    add_typed_edge(
        &mut analysis,
        near_cap.clone(),
        far_module,
        WorkspaceAnalysisEdgeFamily::EffectRequirement,
    );
    let limited = analysis
        .context(
            WorkspaceAnalysisTarget::declaration("collision.same").unwrap(),
            WorkspaceAnalysisDirection::Forward,
            2,
            2,
        )
        .unwrap();
    assert_eq!(limited.nodes[1].node, near_cap);
    assert!(!limited.truncation.frontier.is_empty());
    assert!(limited.truncation.omitted_known_nodes >= 1);

    let deferred = capability("deferred.same");
    analysis.capabilities.insert("deferred.same".to_owned());
    for _ in 0..2 {
        add_typed_edge(
            &mut analysis,
            root.clone(),
            deferred.clone(),
            WorkspaceAnalysisEdgeFamily::EffectRequirement,
        );
    }
    let context = analysis
        .context(
            WorkspaceAnalysisTarget::declaration("collision.same").unwrap(),
            WorkspaceAnalysisDirection::Both,
            MAX_TRAVERSAL_DEPTH,
            MAX_TRAVERSAL_NODES,
        )
        .unwrap();
    let unique_edges = context
        .path_edges
        .iter()
        .map(|edge| edge.edge_index)
        .collect::<BTreeSet<_>>();
    assert_eq!(unique_edges.len(), context.path_edges.len());
}

#[test]
fn target_depth_node_and_discovery_boundaries_are_exact() {
    assert!(WorkspaceAnalysisTarget::declaration(&"x".repeat(MAX_TARGET_BYTES)).is_ok());
    let over_target = format!("private-sentinel-{}", "x".repeat(MAX_TARGET_BYTES));
    let error = WorkspaceAnalysisTarget::declaration(&over_target).unwrap_err();
    assert_eq!(error[0].code, "SPX-G178");
    assert!(!error[0].message.contains("private-sentinel"));
    assert_eq!(
        code(WorkspaceAnalysisTarget::declaration("bad\0target")),
        "SPX-G176"
    );
    validate_traversal_limits(MAX_TRAVERSAL_DEPTH, MAX_TRAVERSAL_NODES).unwrap();
    assert_eq!(
        code(validate_traversal_limits(
            MAX_TRAVERSAL_DEPTH + 1,
            MAX_TRAVERSAL_NODES,
        )),
        "SPX-G178"
    );
    assert_eq!(
        code(validate_traversal_limits(
            MAX_TRAVERSAL_DEPTH,
            MAX_TRAVERSAL_NODES + 1,
        )),
        "SPX-G178"
    );

    let mut analysis = Fixture::new().analysis();
    let root = declaration("collision.same");
    for ordinal in 0..MAX_TRAVERSAL_NODES {
        let name = format!("bulk.{ordinal:04}");
        analysis.capabilities.insert(name.clone());
        add_typed_edge(
            &mut analysis,
            root.clone(),
            capability(&name),
            WorkspaceAnalysisEdgeFamily::EffectRequirement,
        );
    }
    add_typed_edge(
        &mut analysis,
        root.clone(),
        capability(&format!("bulk.{:04}", MAX_TRAVERSAL_NODES - 1)),
        WorkspaceAnalysisEdgeFamily::EffectRequirement,
    );
    let facts = analysis
        .context(
            WorkspaceAnalysisTarget::declaration("collision.same").unwrap(),
            WorkspaceAnalysisDirection::Forward,
            1,
            MAX_TRAVERSAL_NODES,
        )
        .unwrap();
    assert_eq!(facts.nodes.len(), MAX_TRAVERSAL_NODES);
    assert_eq!(facts.truncation.omitted_known_nodes, 0);
    assert_eq!(facts.truncation.deferred_known_nodes, 1);
    assert!(facts.truncation.frontier.is_empty());
}

fn document_sha(document: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(document.as_bytes());
    format!(
        "sha256:{:x}",
        crate::digest_hex::LowerHex(hasher.finalize())
    )
}

fn assert_fragments_in_order(document: &str, fragments: &[&str]) {
    let mut remainder = document;
    for fragment in fragments {
        let offset = remainder
            .find(fragment)
            .unwrap_or_else(|| panic!("missing wire fragment `{fragment}`"));
        remainder = &remainder[offset + fragment.len()..];
    }
}

#[test]
fn context_impact_and_review_documents_have_frozen_kats_and_exact_digest_replay() {
    let analysis = Fixture::new().analysis();
    let declaration_target = WorkspaceAnalysisTarget::declaration("collision.same").unwrap();
    let capability_target = WorkspaceAnalysisTarget::capability("collision.same").unwrap();
    let contexts = [
        analysis
            .render_context(
                declaration_target.clone(),
                WorkspaceAnalysisDirection::Forward,
                8,
                MAX_OUTPUT_BYTES,
                64,
            )
            .unwrap(),
        analysis
            .render_context(
                declaration_target.clone(),
                WorkspaceAnalysisDirection::Reverse,
                8,
                MAX_OUTPUT_BYTES,
                64,
            )
            .unwrap(),
        analysis
            .render_context(
                declaration_target.clone(),
                WorkspaceAnalysisDirection::Both,
                8,
                MAX_OUTPUT_BYTES,
                64,
            )
            .unwrap(),
        analysis
            .render_context(
                capability_target.clone(),
                WorkspaceAnalysisDirection::Reverse,
                8,
                MAX_OUTPUT_BYTES,
                64,
            )
            .unwrap(),
    ];
    assert_eq!(
        contexts
            .each_ref()
            .map(|artifact| document_sha(&artifact.json)),
        [
            "sha256:034357caffca26d6ffd07ae880aaa2afa1c28061d857a5a677293eb33dd885e9",
            "sha256:872ab2a3c6a1d066c464c6e30fd155a8da3606b2934e0fcc406623cdefd99881",
            "sha256:343129156efde7075cbee6aedab4e0795f31d110bcd161acba82cbc7e99c9259",
            "sha256:940fbcbf25ba83e20030e879076ccff88fa0ddc6420728b4b704e3bd9b140126"
        ]
    );
    for artifact in &contexts {
        let wire: serde_json::Value = serde_json::from_str(&artifact.json).unwrap();
        assert_eq!(
            wire["budget"]["analysis"]["used_output_bytes"],
            artifact.json.len()
        );
        let payload = render_context_json(
            &analysis,
            &artifact.facts,
            wire["query"]["depth"].as_u64().unwrap() as usize,
            wire["query"]["max_bytes"].as_u64().unwrap() as usize,
            wire["query"]["max_nodes"].as_u64().unwrap() as usize,
            None,
            artifact.json.len(),
        );
        assert_eq!(
            artifact.digest,
            artifact_digest(CONTEXT_DIGEST_DOMAIN, payload.as_bytes())
        );
        assert_fragments_in_order(
            &artifact.json,
            &[
                "\"schema\":",
                "\"workspace_manifest_schema\":",
                "\"workspace_revision\":",
                "\"workspace_graph_digest\":",
                "\"artifact_digest\":",
                "\"entry\":",
                "\"target\":",
                "\"query\":",
                "\"limits\":",
                "\"budget\":",
                "\"truncation\":",
                "\"frontier\":",
                "\"nodes\":",
                "\"edges\":",
                "\"nonclaims\":",
            ],
        );
    }

    let impacts = [
        analysis
            .render_impact(declaration_target.clone(), 8, MAX_OUTPUT_BYTES, 64)
            .unwrap(),
        analysis
            .render_impact(capability_target.clone(), 8, MAX_OUTPUT_BYTES, 64)
            .unwrap(),
    ];
    assert_eq!(
        impacts
            .each_ref()
            .map(|artifact| document_sha(&artifact.json)),
        [
            "sha256:c3196a4f38a64565f437eb01b3b21fa7d784ebeb286fd4aa9315e68af6dd8cfc",
            "sha256:49a5f12b2afa110c9f3076894e6e6b259ac159e850ca7946f8539f796a92f4a3",
        ]
    );
    let declaration_impact: serde_json::Value = serde_json::from_str(&impacts[0].json).unwrap();
    assert_eq!(
        declaration_impact["affected"],
        serde_json::json!([
            {
                "kind": "declaration",
                "declaration_kind": "function",
                "identity_origin": "explicit",
                "id": "collision.same",
                "path": "a/provider.spx",
                "module": "collision.same",
                "minimum_depth": 0,
                "impact_role": "target",
                "reasons": [],
            },
            {
                "kind": "module",
                "declaration_kind": null,
                "identity_origin": null,
                "id": "entry.app",
                "path": "z/entry.spx",
                "module": "entry.app",
                "minimum_depth": 1,
                "impact_role": "module_consumer",
                "reasons": ["function_import"],
            },
            {
                "kind": "declaration",
                "declaration_kind": "function",
                "identity_origin": "explicit",
                "id": "entry.main",
                "path": "z/entry.spx",
                "module": "entry.app",
                "minimum_depth": 1,
                "impact_role": "consumer",
                "reasons": ["call"],
            },
        ])
    );
    let capability_impact: serde_json::Value = serde_json::from_str(&impacts[1].json).unwrap();
    assert_eq!(
        capability_impact["affected"],
        serde_json::json!([
            {
                "kind": "capability",
                "declaration_kind": null,
                "identity_origin": null,
                "id": "collision.same",
                "path": null,
                "module": null,
                "minimum_depth": 0,
                "impact_role": "target",
                "reasons": [],
            },
            {
                "kind": "module",
                "declaration_kind": null,
                "identity_origin": null,
                "id": "collision.same",
                "path": "a/provider.spx",
                "module": "collision.same",
                "minimum_depth": 1,
                "impact_role": "module_consumer",
                "reasons": ["capability_authority"],
            },
            {
                "kind": "module",
                "declaration_kind": null,
                "identity_origin": null,
                "id": "entry.app",
                "path": "z/entry.spx",
                "module": "entry.app",
                "minimum_depth": 1,
                "impact_role": "module_consumer",
                "reasons": ["capability_authority"],
            },
            {
                "kind": "declaration",
                "declaration_kind": "function",
                "identity_origin": "explicit",
                "id": "entry.main",
                "path": "z/entry.spx",
                "module": "entry.app",
                "minimum_depth": 1,
                "impact_role": "consumer",
                "reasons": ["effect_requirement"],
            },
        ])
    );
    for artifact in &impacts {
        let wire: serde_json::Value = serde_json::from_str(&artifact.json).unwrap();
        assert_eq!(
            wire["budget"]["analysis"]["used_output_bytes"],
            artifact.json.len()
        );
        assert!(artifact
            .facts
            .path_edges
            .windows(2)
            .all(|pair| pair[0].edge_index < pair[1].edge_index));
        assert_eq!(
            wire["dependency_edges"]
                .as_array()
                .unwrap()
                .iter()
                .map(|edge| edge["kind"].as_str().unwrap())
                .collect::<Vec<_>>(),
            artifact
                .facts
                .path_edges
                .iter()
                .map(|edge| analysis.edges()[edge.edge_index].kind())
                .collect::<Vec<_>>()
        );
        let payload = render_impact_json(
            &analysis,
            &artifact.facts,
            wire["query"]["depth"].as_u64().unwrap() as usize,
            wire["query"]["max_bytes"].as_u64().unwrap() as usize,
            wire["query"]["max_nodes"].as_u64().unwrap() as usize,
            None,
            artifact.json.len(),
        );
        assert_eq!(
            artifact.digest,
            artifact_digest(IMPACT_DIGEST_DOMAIN, payload.as_bytes())
        );
    }

    let review = analysis.render_review(declaration_target.clone()).unwrap();
    assert_eq!(
        document_sha(&review.json),
        "sha256:4c9c8216822550837fa48efc9f33ee3932cb0cf5a45d84a203e9850b32a5d005"
    );
    let direct_context = analysis
        .render_context(
            declaration_target.clone(),
            WorkspaceAnalysisDirection::Both,
            MAX_TRAVERSAL_DEPTH,
            MAX_OUTPUT_BYTES,
            MAX_TRAVERSAL_NODES,
        )
        .unwrap();
    let direct_impact = analysis
        .render_impact(
            declaration_target,
            MAX_TRAVERSAL_DEPTH,
            MAX_OUTPUT_BYTES,
            MAX_TRAVERSAL_NODES,
        )
        .unwrap();
    assert!(review.json.contains(&format!(
        "\"context\":{},\"impact\":{}",
        direct_context.json, direct_impact.json
    )));
    let review_wire: serde_json::Value = serde_json::from_str(&review.json).unwrap();
    assert_eq!(review_wire["schema"], REVIEW_SCHEMA);
    assert_eq!(review_wire["context"]["schema"], CONTEXT_SCHEMA);
    assert_eq!(review_wire["impact"]["schema"], IMPACT_SCHEMA);
    assert_eq!(
        review_wire["budget"]["analysis"]["used_output_bytes"],
        review.json.len()
    );
    for findings in review_wire["sections"].as_object().unwrap().values() {
        for finding in findings.as_array().unwrap() {
            for reference in finding["evidence"].as_array().unwrap() {
                let index = reference["index"].as_u64().unwrap() as usize;
                let bound = match (
                    reference["artifact"].as_str().unwrap(),
                    reference["relation"].as_str().unwrap(),
                ) {
                    ("context", "edges") => {
                        review_wire["context"]["edges"].as_array().unwrap().len()
                    }
                    ("impact", "dependency_edges") => review_wire["impact"]["dependency_edges"]
                        .as_array()
                        .unwrap()
                        .len(),
                    ("impact", "affected") => {
                        review_wire["impact"]["affected"].as_array().unwrap().len()
                    }
                    relation => panic!("unexpected Review evidence relation {relation:?}"),
                };
                assert!(index < bound);
            }
        }
    }
    let cumulative_impact = analysis
        .render_impact_with_output_limit_and_builder_start(
            direct_context.facts.target.clone(),
            MAX_TRAVERSAL_DEPTH,
            MAX_OUTPUT_BYTES,
            MAX_TRAVERSAL_NODES,
            MAX_OUTPUT_BYTES,
            direct_context.facts.aggregate_builder_bytes,
        )
        .unwrap();
    assert_eq!(cumulative_impact.json, direct_impact.json);
    let payload = render_review_json(
        &analysis,
        &direct_context,
        &cumulative_impact,
        None,
        review.json.len(),
    );
    assert_eq!(
        review.digest,
        artifact_digest(REVIEW_DIGEST_DOMAIN, payload.as_bytes())
    );
    assert_ne!(
        artifact_digest(REVIEW_DIGEST_DOMAIN, format!("{payload}x").as_bytes()),
        review.digest
    );
    assert_fragments_in_order(
        &review.json,
        &[
            "\"context\":",
            "\"impact\":",
            "\"sections\":{\"behavior\":",
            "\"api_identity\":",
            "\"security_authority\":",
            "\"memory_ownership\":",
            "\"target_artifact\":",
            "\"migration\":",
            "\"unsafe\":",
            "\"limits\":",
            "\"budget\":",
            "\"nonclaims\":",
        ],
    );
    let mut escaped = crate::bounded_output::CappedString::new();
    push_json_string(&mut escaped, "quote\" slash\\ line\n tab\t\u{1}");
    assert_eq!(
        escaped.into_string(),
        "\"quote\\\" slash\\\\ line\\n tab\\t\\u0001\""
    );
}

#[test]
fn output_caps_are_exact_and_truncated_documents_have_no_dangling_edges() {
    let analysis = Fixture::new().analysis();
    let target = WorkspaceAnalysisTarget::declaration("collision.same").unwrap();
    let full = analysis
        .render_context(
            target.clone(),
            WorkspaceAnalysisDirection::Both,
            8,
            MAX_OUTPUT_BYTES,
            64,
        )
        .unwrap();
    let mut low = MIN_OUTPUT_BYTES;
    let mut high = full.json.len();
    while low < high {
        let middle = low + (high - low) / 2;
        if analysis
            .render_context(
                target.clone(),
                WorkspaceAnalysisDirection::Both,
                8,
                middle,
                64,
            )
            .is_ok()
        {
            high = middle;
        } else {
            low = middle + 1;
        }
    }
    let exact = analysis
        .render_context(target.clone(), WorkspaceAnalysisDirection::Both, 8, low, 64)
        .unwrap();
    assert!(exact.json.len() <= low);
    if low > MIN_OUTPUT_BYTES {
        assert_eq!(
            code(analysis.render_context(
                target.clone(),
                WorkspaceAnalysisDirection::Both,
                8,
                low - 1,
                64,
            )),
            "SPX-G178"
        );
    }

    let full_impact = analysis
        .render_impact(target.clone(), 8, MAX_OUTPUT_BYTES, 64)
        .unwrap();
    let mut impact_low = MIN_OUTPUT_BYTES;
    let mut impact_high = full_impact.json.len();
    while impact_low < impact_high {
        let middle = impact_low + (impact_high - impact_low) / 2;
        if analysis
            .render_impact(target.clone(), 8, middle, 64)
            .is_ok()
        {
            impact_high = middle;
        } else {
            impact_low = middle + 1;
        }
    }
    analysis
        .render_impact(target.clone(), 8, impact_low, 64)
        .unwrap();
    if impact_low > MIN_OUTPUT_BYTES {
        assert_eq!(
            code(analysis.render_impact(target.clone(), 8, impact_low - 1, 64)),
            "SPX-G178"
        );
    }

    let truncated = analysis
        .render_context(
            target.clone(),
            WorkspaceAnalysisDirection::Both,
            8,
            MAX_OUTPUT_BYTES,
            2,
        )
        .unwrap();
    let wire: serde_json::Value = serde_json::from_str(&truncated.json).unwrap();
    assert_eq!(wire["truncation"]["truncated"], true);
    assert!(wire["truncation"]["reasons"]
        .as_array()
        .unwrap()
        .iter()
        .any(|reason| reason == "max_nodes"));
    let node_keys = truncated
        .facts
        .nodes
        .iter()
        .map(|fact| fact.node.clone())
        .collect::<BTreeSet<_>>();
    for path_edge in &truncated.facts.path_edges {
        let edge = &analysis.typed_edges[path_edge.edge_index];
        assert!(node_keys.contains(&edge.source));
        assert!(node_keys.contains(&edge.target));
    }
    let frontier_depths = truncated
        .facts
        .truncation
        .frontier
        .iter()
        .map(|fact| fact.depth)
        .collect::<BTreeSet<_>>();
    assert!(frontier_depths.len() <= 1);

    let truncated_impact = analysis
        .render_impact(target.clone(), 8, MAX_OUTPUT_BYTES, 2)
        .unwrap();
    let impact_nodes = truncated_impact
        .facts
        .nodes
        .iter()
        .map(|fact| fact.node.node.clone())
        .collect::<BTreeSet<_>>();
    for path_edge in &truncated_impact.facts.path_edges {
        let edge = &analysis.typed_edges[path_edge.edge_index];
        assert!(impact_nodes.contains(&edge.source));
        assert!(impact_nodes.contains(&edge.target));
    }

    let mut review_low = MIN_OUTPUT_BYTES;
    let mut review_high = MAX_REVIEW_OUTPUT_BYTES;
    while review_low < review_high {
        let middle = review_low + (review_high - review_low) / 2;
        if analysis
            .render_review_with_limit(target.clone(), middle)
            .is_ok()
        {
            review_high = middle;
        } else {
            review_low = middle + 1;
        }
    }
    analysis
        .render_review_with_limit(target.clone(), review_low)
        .unwrap();
    if review_low > MIN_OUTPUT_BYTES {
        assert_eq!(
            code(analysis.render_review_with_limit(target, review_low - 1)),
            "SPX-G180"
        );
    }
}

#[test]
fn wide_depth_prefix_fit_is_maximal_with_a_late_tiny_builder_remainder() {
    let mut analysis = Fixture::new().analysis();
    let root = declaration("collision.same");
    let bridge = declaration("collision.a");
    add_typed_edge(
        &mut analysis,
        root.clone(),
        bridge.clone(),
        WorkspaceAnalysisEdgeFamily::Call,
    );
    for ordinal in 0..2048 {
        let name = format!("wide.capability.{ordinal:04}");
        analysis.capabilities.insert(name.clone());
        add_typed_edge(
            &mut analysis,
            bridge.clone(),
            capability(&name),
            WorkspaceAnalysisEdgeFamily::EffectRequirement,
        );
    }

    let target = WorkspaceAnalysisTarget::declaration("collision.same").unwrap();
    let facts = analysis
        .context(
            target,
            WorkspaceAnalysisDirection::Forward,
            2,
            MAX_TRAVERSAL_NODES,
        )
        .unwrap();
    let full = render_context_artifact(
        &analysis,
        facts.clone(),
        2,
        MAX_OUTPUT_BYTES,
        MAX_TRAVERSAL_NODES,
        MAX_OUTPUT_BYTES,
    )
    .unwrap();
    let output_limit = (full.json.len() / 2).max(MIN_OUTPUT_BYTES);
    let prefix = render_context_artifact(
        &analysis,
        facts.clone(),
        2,
        output_limit,
        MAX_TRAVERSAL_NODES,
        output_limit,
    )
    .unwrap();
    let retained = prefix.facts.nodes.len();
    assert!(retained > 1 && retained < facts.nodes.len());
    assert_eq!(prefix.facts.nodes, facts.nodes[..retained]);

    let mut measured_facts = facts.clone();
    let remaining_builder_bytes = MAX_BUILDER_BYTES
        .checked_sub(measured_facts.aggregate_builder_bytes)
        .unwrap();
    let (ranks, rank_builder_bytes) = retained_node_ranks(
        measured_facts.nodes.iter().map(|fact| &fact.node),
        remaining_builder_bytes,
    )
    .unwrap();
    measured_facts.used_builder_bytes =
        checked_builder_sum(measured_facts.used_builder_bytes, rank_builder_bytes).unwrap();
    measured_facts.aggregate_builder_bytes =
        checked_builder_sum(measured_facts.aggregate_builder_bytes, rank_builder_bytes).unwrap();
    let selected_builder_bytes = checked_builder_sum(
        measured_facts.used_builder_bytes,
        context_finalization_debit(&measured_facts, retained).unwrap(),
    )
    .unwrap();
    assert!(measure_artifact(output_limit, |used_output_bytes| {
        render_context_json_prefix(
            &analysis,
            &measured_facts,
            2,
            output_limit,
            MAX_TRAVERSAL_NODES,
            Some(DIGEST_PLACEHOLDER),
            used_output_bytes,
            retained,
            Some(&ranks),
            selected_builder_bytes,
        )
    })
    .unwrap()
    .is_some());
    let next = next_context_builder_feasible_prefix(
        &measured_facts,
        retained + 1,
        measured_facts.used_builder_bytes,
        measured_facts.aggregate_builder_bytes,
    )
    .unwrap();
    assert!(next <= measured_facts.nodes.len());
    let next_builder_bytes = checked_builder_sum(
        measured_facts.used_builder_bytes,
        context_finalization_debit(&measured_facts, next).unwrap(),
    )
    .unwrap();
    assert!(measure_artifact(output_limit, |used_output_bytes| {
        render_context_json_prefix(
            &analysis,
            &measured_facts,
            2,
            output_limit,
            MAX_TRAVERSAL_NODES,
            Some(DIGEST_PLACEHOLDER),
            used_output_bytes,
            next,
            Some(&ranks),
            next_builder_bytes,
        )
    })
    .unwrap()
    .is_none());

    let depth_two_start = measured_facts.nodes.partition_point(|fact| fact.depth < 2);
    let depth_two_end = measured_facts.nodes.partition_point(|fact| fact.depth <= 2);
    assert!(depth_two_end - depth_two_start >= 2048);
    let node_bytes = std::mem::size_of::<WorkspaceAnalysisNodeFact>();
    let late_prefix = next_context_builder_feasible_prefix(
        &measured_facts,
        depth_two_start,
        MAX_BUILDER_BYTES - 2 * node_bytes,
        MAX_BUILDER_BYTES - 2 * node_bytes,
    )
    .unwrap();
    assert_eq!(late_prefix, depth_two_end - 2);
    assert_eq!(
        context_finalization_debit(&measured_facts, late_prefix).unwrap(),
        2 * node_bytes
    );
    assert_eq!(
        context_finalization_debit(&measured_facts, late_prefix - 1).unwrap(),
        3 * node_bytes
    );
}

#[test]
fn review_rejects_any_child_discovery_truncation_as_g180() {
    let mut analysis = Fixture::new().analysis();
    let root = declaration("collision.same");
    for ordinal in 0..MAX_TRAVERSAL_NODES {
        let name = format!("review.bulk.{ordinal:04}");
        analysis.capabilities.insert(name.clone());
        add_typed_edge(
            &mut analysis,
            root.clone(),
            capability(&name),
            WorkspaceAnalysisEdgeFamily::EffectRequirement,
        );
    }
    let error = analysis
        .render_review(WorkspaceAnalysisTarget::declaration("collision.same").unwrap())
        .err()
        .expect("Review must return no artifact when a child is truncated");
    assert_eq!(error.len(), 1);
    assert_eq!(error[0].code, "SPX-G180");
}

fn mutate_active_and_assert_immediate_reacquire(root: &Path) {
    let active = root.join(".semaprax-workspace/ACTIVE");
    OpenOptions::new()
        .append(true)
        .open(active)
        .unwrap()
        .write_all(b"x")
        .unwrap();
}

fn assert_immediate_exclusive_reacquire(root: &Path) {
    let lock = OpenOptions::new()
        .read(true)
        .write(true)
        .open(root.join(".semaprax-workspace/LOCK"))
        .unwrap();
    fs2::FileExt::try_lock_exclusive(&lock).unwrap();
    fs2::FileExt::unlock(&lock).unwrap();
}

#[test]
fn context_impact_and_review_discard_after_render_drift_and_release_authority() {
    let context_fixture = Fixture::new();
    let context_called = std::cell::Cell::new(false);
    let context = crate::workspace_graph::build_authenticated_context_artifact_with_hook(
        &context_fixture.root,
        "entry.app",
        WorkspaceAnalysisTarget::declaration("collision.same").unwrap(),
        WorkspaceAnalysisDirection::Both,
        4,
        1024 * 1024,
        1024,
        |artifact| {
            context_called.set(true);
            assert!(!artifact.json.is_empty());
            mutate_active_and_assert_immediate_reacquire(&context_fixture.root);
        },
    );
    assert!(context_called.get());
    let error = context
        .err()
        .expect("final drift must return no Context artifact");
    assert_eq!(error.len(), 1);
    assert_eq!(error[0].code, "SPX-G153");
    assert_eq!(
        error[0].message,
        "workspace object changed during authentication"
    );
    assert_immediate_exclusive_reacquire(&context_fixture.root);

    let impact_fixture = Fixture::new();
    let impact_called = std::cell::Cell::new(false);
    let impact = crate::workspace_graph::build_authenticated_impact_artifact_with_hook(
        &impact_fixture.root,
        "entry.app",
        WorkspaceAnalysisTarget::declaration("collision.same").unwrap(),
        16,
        1024 * 1024,
        1024,
        |artifact| {
            impact_called.set(true);
            assert!(!artifact.json.is_empty());
            mutate_active_and_assert_immediate_reacquire(&impact_fixture.root);
        },
    );
    assert!(impact_called.get());
    let error = impact
        .err()
        .expect("final drift must return no Impact artifact");
    assert_eq!(error.len(), 1);
    assert_eq!(error[0].code, "SPX-G153");
    assert_eq!(
        error[0].message,
        "workspace object changed during authentication"
    );
    assert_immediate_exclusive_reacquire(&impact_fixture.root);

    let review_fixture = Fixture::new();
    let review_called = std::cell::Cell::new(false);
    let review = crate::workspace_graph::build_authenticated_review_artifact_with_hook(
        &review_fixture.root,
        "entry.app",
        WorkspaceAnalysisTarget::declaration("collision.same").unwrap(),
        |artifact| {
            review_called.set(true);
            assert!(!artifact.json.is_empty());
            mutate_active_and_assert_immediate_reacquire(&review_fixture.root);
        },
    );
    assert!(review_called.get());
    let error = review
        .err()
        .expect("final drift must return no Review artifact");
    assert_eq!(error.len(), 1);
    assert_eq!(error[0].code, "SPX-G153");
    assert_eq!(
        error[0].message,
        "workspace object changed during authentication"
    );
    assert_immediate_exclusive_reacquire(&review_fixture.root);
}
