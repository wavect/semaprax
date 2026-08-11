//! Bounded, read-only Context, Impact, and Review over one authenticated
//! Workspace Semantic Graph.
//!
//! Each operation authenticates the managed semantic workspace under its
//! shared lock, builds one typed graph/index view, renders the canonical
//! bounded artifact, then performs the final workspace recheck and checked
//! unlock. This module has no parser, verifier, patch, stage, publish, apply,
//! commit, backend, runtime, or reusable authorization authority.

use std::cmp::Ordering;
use std::collections::{BTreeMap, BTreeSet, VecDeque};
use std::fmt::Write as _;
use std::path::Path;

use crate::diagnostic::Diagnostic;
use crate::hir::{DeclarationKind, IdentityOrigin};
use crate::workspace_graph::{
    WorkspaceEdge, WorkspaceGraphProjection, WorkspaceGraphProjectionDeclaration,
    WorkspaceGraphProjectionModule, WorkspaceGraphProjectionUsage,
};
use sha2::{Digest, Sha256};

pub(crate) const MAX_TARGET_BYTES: usize = 4096;
pub(crate) const MAX_TRAVERSAL_DEPTH: usize = 1024;
pub(crate) const MAX_TRAVERSAL_NODES: usize = 8208;
pub(crate) const MAX_BUILDER_BYTES: usize = 16 * 1024 * 1024;
pub(crate) const MIN_OUTPUT_BYTES: usize = 4096;
pub(crate) const MAX_OUTPUT_BYTES: usize = 16 * 1024 * 1024;
pub(crate) const MAX_REVIEW_OUTPUT_BYTES: usize = 32 * 1024 * 1024;

const CONTEXT_SCHEMA: &str = "semaprax.workspace-semantic-context.v1";
const IMPACT_SCHEMA: &str = "semaprax.workspace-semantic-impact.v1";
const REVIEW_SCHEMA: &str = "semaprax.workspace-semantic-review.v1";
const WORKSPACE_MANIFEST_SCHEMA: &str = "semaprax.workspace-semantic-manifest.v1";
const CONTEXT_DIGEST_DOMAIN: &[u8] = b"semaprax.workspace-semantic-context.artifact-digest.v1\0";
const IMPACT_DIGEST_DOMAIN: &[u8] = b"semaprax.workspace-semantic-impact.artifact-digest.v1\0";
const REVIEW_DIGEST_DOMAIN: &[u8] = b"semaprax.workspace-semantic-review.artifact-digest.v1\0";
const DIGEST_PLACEHOLDER: &str =
    "sha256:0000000000000000000000000000000000000000000000000000000000000000";
const COMMON_NONCLAIMS: [&str; 11] = [
    "no_generic_cross_file_composition",
    "automatic_target_identity_is_revision_scoped_not_persistent_patch_address",
    "no_cross_file_resource_interface_ownership_borrowing_or_lifetime_composition",
    "no_reexport_wildcard_implicit_or_ambiguous_imports",
    "no_target_codegen_artifact_project_test_or_execution",
    "no_exclusive_lock_stage_publish_apply_or_commit_authority",
    "not_proof_signature_provenance_approval_or_reusable_authorization",
    "no_raw_working_tree_git_editor_or_unmanaged_file_analysis",
    "no_incremental_cache_persistence_or_repository_index",
    "no_recovery_rollback_cleanup_gc_or_durability_guarantee",
    "no_external_consumer_compatibility",
];
const CONTEXT_NONCLAIMS: [&str; 15] = prepend_nonclaims(
    [
        "no_patch_candidate_change_or_semantic_delta",
        "no_impact_or_review_claim",
        "only_six_workspace_graph_edge_families",
        "no_embedding_search_ranking_or_answer_quality",
    ],
    COMMON_NONCLAIMS,
);
const IMPACT_NONCLAIMS: [&str; 15] = prepend_nonclaims(
    [
        "potential_structural_dependency_impact_not_patch_candidate_or_behavioral_delta",
        "no_source_consumer_span_or_authored_operation_provenance",
        "only_reverse_closure_over_six_workspace_graph_edge_families",
        "no_repair_review_ranking_or_commit_authority",
    ],
    COMMON_NONCLAIMS,
);
const REVIEW_NONCLAIMS: [&str; 15] = prepend_nonclaims(
    [
        "dependency_review_not_patch_change_or_general_semantic_review",
        "not_human_approval_policy_or_security_audit",
        "context_and_impact_are_current_state_read_only_projections",
        "memory_ownership_target_artifact_and_unsafe_sections_are_not_analyzed",
    ],
    COMMON_NONCLAIMS,
);

const fn prepend_nonclaims(
    first: [&'static str; 4],
    rest: [&'static str; 11],
) -> [&'static str; 15] {
    [
        first[0], first[1], first[2], first[3], rest[0], rest[1], rest[2], rest[3], rest[4],
        rest[5], rest[6], rest[7], rest[8], rest[9], rest[10],
    ]
}

const EDGE_FAMILIES: [WorkspaceAnalysisEdgeFamily; 6] = [
    WorkspaceAnalysisEdgeFamily::FunctionImport,
    WorkspaceAnalysisEdgeFamily::TypeImport,
    WorkspaceAnalysisEdgeFamily::Call,
    WorkspaceAnalysisEdgeFamily::TypeReference,
    WorkspaceAnalysisEdgeFamily::EffectRequirement,
    WorkspaceAnalysisEdgeFamily::CapabilityAuthority,
];

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum WorkspaceAnalysisArtifactKind {
    Context,
    Impact,
    Review,
}

impl WorkspaceAnalysisArtifactKind {
    const fn name(self) -> &'static str {
        match self {
            Self::Context => "Context",
            Self::Impact => "Impact",
            Self::Review => "Review",
        }
    }

    const fn invariant_message(self) -> &'static str {
        match self {
            Self::Context => "Workspace Semantic Context replay or digest binding disagrees",
            Self::Impact => "Workspace Semantic Impact replay or digest binding disagrees",
            Self::Review => "Workspace Semantic Review replay or digest binding disagrees",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub(crate) enum WorkspaceAnalysisEdgeFamily {
    FunctionImport,
    TypeImport,
    Call,
    TypeReference,
    EffectRequirement,
    CapabilityAuthority,
}

impl WorkspaceAnalysisEdgeFamily {
    pub(crate) const fn name(self) -> &'static str {
        match self {
            Self::FunctionImport => "function_import",
            Self::TypeImport => "type_import",
            Self::Call => "call",
            Self::TypeReference => "type_reference",
            Self::EffectRequirement => "effect_requirement",
            Self::CapabilityAuthority => "capability_authority",
        }
    }

    fn from_name(name: &str) -> Option<Self> {
        EDGE_FAMILIES
            .into_iter()
            .find(|candidate| candidate.name() == name)
    }
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum WorkspaceAnalysisDirection {
    Forward,
    Reverse,
    Both,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum WorkspaceAnalysisTargetKind {
    Declaration,
    Capability,
}

/// Validated bounds for one Workspace Semantic Context query.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct WorkspaceContextOptions {
    direction: WorkspaceAnalysisDirection,
    depth: usize,
    max_bytes: usize,
    max_nodes: usize,
}

impl WorkspaceContextOptions {
    pub fn new(
        direction: WorkspaceAnalysisDirection,
        depth: usize,
        max_bytes: usize,
        max_nodes: usize,
    ) -> Result<Self, Diagnostic> {
        validate_public_options(
            WorkspaceAnalysisArtifactKind::Context,
            depth,
            max_bytes,
            max_nodes,
        )?;
        Ok(Self {
            direction,
            depth,
            max_bytes,
            max_nodes,
        })
    }
}

impl Default for WorkspaceContextOptions {
    fn default() -> Self {
        Self {
            direction: WorkspaceAnalysisDirection::Both,
            depth: 4,
            max_bytes: 1024 * 1024,
            max_nodes: 1024,
        }
    }
}

/// Validated bounds for one Workspace Semantic Impact query.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct WorkspaceImpactOptions {
    depth: usize,
    max_bytes: usize,
    max_nodes: usize,
}

impl WorkspaceImpactOptions {
    pub fn new(depth: usize, max_bytes: usize, max_nodes: usize) -> Result<Self, Diagnostic> {
        validate_public_options(
            WorkspaceAnalysisArtifactKind::Impact,
            depth,
            max_bytes,
            max_nodes,
        )?;
        Ok(Self {
            depth,
            max_bytes,
            max_nodes,
        })
    }
}

impl Default for WorkspaceImpactOptions {
    fn default() -> Self {
        Self {
            depth: 16,
            max_bytes: 1024 * 1024,
            max_nodes: 1024,
        }
    }
}

/// Render one canonical Workspace Semantic Context artifact while the
/// authenticated semantic workspace authority remains held.
pub fn context(
    root: &Path,
    entry_module: &str,
    target_kind: WorkspaceAnalysisTargetKind,
    target: &str,
    options: WorkspaceContextOptions,
) -> Result<String, Vec<Diagnostic>> {
    crate::workspace_graph::validate_entry_module(entry_module)?;
    let target = WorkspaceAnalysisTarget::new(target_kind, target)?;
    crate::workspace_graph::build_authenticated_context_artifact(
        root,
        entry_module,
        target,
        options.direction,
        options.depth,
        options.max_bytes,
        options.max_nodes,
    )
    .map(|artifact| artifact.json)
}

/// Render one canonical Workspace Semantic Impact artifact while the
/// authenticated semantic workspace authority remains held.
pub fn impact(
    root: &Path,
    entry_module: &str,
    target_kind: WorkspaceAnalysisTargetKind,
    target: &str,
    options: WorkspaceImpactOptions,
) -> Result<String, Vec<Diagnostic>> {
    crate::workspace_graph::validate_entry_module(entry_module)?;
    let target = WorkspaceAnalysisTarget::new(target_kind, target)?;
    crate::workspace_graph::build_authenticated_impact_artifact(
        root,
        entry_module,
        target,
        options.depth,
        options.max_bytes,
        options.max_nodes,
    )
    .map(|artifact| artifact.json)
}

/// Render one canonical complete Workspace Semantic Review artifact while the
/// authenticated semantic workspace authority remains held.
pub fn review(
    root: &Path,
    entry_module: &str,
    target_kind: WorkspaceAnalysisTargetKind,
    target: &str,
) -> Result<String, Vec<Diagnostic>> {
    crate::workspace_graph::validate_entry_module(entry_module)?;
    let target = WorkspaceAnalysisTarget::new(target_kind, target)?;
    crate::workspace_graph::build_authenticated_review_artifact(root, entry_module, target)
        .map(|artifact| artifact.json)
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct WorkspaceAnalysisTarget {
    kind: WorkspaceAnalysisTargetKind,
    value: String,
}

impl WorkspaceAnalysisTarget {
    #[cfg(test)]
    pub(crate) fn declaration(id: &str) -> Result<Self, Vec<Diagnostic>> {
        Self::new(WorkspaceAnalysisTargetKind::Declaration, id)
    }

    #[cfg(test)]
    pub(crate) fn capability(name: &str) -> Result<Self, Vec<Diagnostic>> {
        Self::new(WorkspaceAnalysisTargetKind::Capability, name)
    }

    fn new(kind: WorkspaceAnalysisTargetKind, value: &str) -> Result<Self, Vec<Diagnostic>> {
        validate_target_text(value)?;
        Ok(Self {
            kind,
            value: value.to_owned(),
        })
    }

    pub(crate) const fn kind(&self) -> WorkspaceAnalysisTargetKind {
        self.kind
    }

    pub(crate) fn value(&self) -> &str {
        &self.value
    }
}

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub(crate) enum WorkspaceAnalysisNode {
    Module { path: String, module: String },
    Declaration(String),
    Capability(String),
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub(crate) enum WorkspaceAnalysisReachedBy {
    Root,
    Forward,
    Reverse,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct WorkspaceAnalysisNodeFact {
    node: WorkspaceAnalysisNode,
    depth: usize,
    reached_by: Vec<WorkspaceAnalysisReachedBy>,
    declaration_kind: Option<DeclarationKind>,
    identity_origin: Option<IdentityOrigin>,
    owner: Option<String>,
    path: Option<String>,
    module: Option<String>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct WorkspaceAnalysisPathEdge {
    edge_index: usize,
    reached_by: WorkspaceAnalysisReachedBy,
    depth: usize,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct WorkspaceAnalysisTruncationFacts {
    frontier: Vec<WorkspaceAnalysisNodeFact>,
    omitted_known_nodes: usize,
    deferred_known_nodes: usize,
    byte_truncated: bool,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct WorkspaceContextFacts {
    target: WorkspaceAnalysisTarget,
    direction: WorkspaceAnalysisDirection,
    nodes: Vec<WorkspaceAnalysisNodeFact>,
    path_edges: Vec<WorkspaceAnalysisPathEdge>,
    truncation: WorkspaceAnalysisTruncationFacts,
    used_builder_bytes: usize,
    aggregate_builder_bytes: usize,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum WorkspaceImpactRole {
    Root,
    DeclarationConsumer,
    ModuleConsumer,
    CapabilityConsumer,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct WorkspaceImpactNodeFact {
    node: WorkspaceAnalysisNodeFact,
    role: WorkspaceImpactRole,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct WorkspaceImpactFacts {
    target: WorkspaceAnalysisTarget,
    nodes: Vec<WorkspaceImpactNodeFact>,
    path_edges: Vec<WorkspaceAnalysisPathEdge>,
    truncation: WorkspaceAnalysisTruncationFacts,
    used_builder_bytes: usize,
    aggregate_builder_bytes: usize,
}

pub(crate) struct WorkspaceContextArtifact {
    json: String,
    #[cfg_attr(
        not(test),
        allow(dead_code, reason = "digest is bound into canonical JSON")
    )]
    digest: String,
    facts: WorkspaceContextFacts,
}

pub(crate) struct WorkspaceImpactArtifact {
    json: String,
    #[cfg_attr(
        not(test),
        allow(dead_code, reason = "digest is bound into canonical JSON")
    )]
    digest: String,
    facts: WorkspaceImpactFacts,
}

pub(crate) struct WorkspaceReviewArtifact {
    json: String,
    #[cfg_attr(
        not(test),
        allow(dead_code, reason = "digest is bound into canonical JSON")
    )]
    digest: String,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum WorkspaceAnalysisCallableKind {
    Function,
    Template,
    Instance,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct WorkspaceAnalysisCallableLocation {
    module_index: usize,
    callable_index: usize,
    kind: WorkspaceAnalysisCallableKind,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct WorkspaceAnalysisDeclarationFact {
    kind: DeclarationKind,
    origin: IdentityOrigin,
    owner: Option<String>,
    path: Option<String>,
    module: Option<String>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct WorkspaceAnalysisTypedEdge {
    source: WorkspaceAnalysisNode,
    target: WorkspaceAnalysisNode,
    family: WorkspaceAnalysisEdgeFamily,
}

pub(crate) struct WorkspaceAnalysis {
    projection: WorkspaceGraphProjection,
    workspace_graph_digest: String,
    workspace_graph_output_bytes: usize,
    builder_bytes: usize,
    modules: BTreeMap<WorkspaceAnalysisNode, usize>,
    declarations: BTreeMap<String, WorkspaceAnalysisDeclarationFact>,
    capabilities: BTreeSet<String>,
    #[allow(dead_code, reason = "retained authenticated callable index proof")]
    callables: BTreeMap<String, WorkspaceAnalysisCallableLocation>,
    #[allow(dead_code, reason = "retained authenticated shared-prelude proof")]
    prelude: BTreeSet<String>,
    #[allow(dead_code, reason = "retained authenticated permit-authority proof")]
    permits: BTreeMap<WorkspaceAnalysisNode, BTreeSet<String>>,
    typed_edges: Vec<WorkspaceAnalysisTypedEdge>,
    #[allow(dead_code, reason = "retained authenticated edge-family replay index")]
    families: BTreeMap<WorkspaceAnalysisEdgeFamily, Vec<usize>>,
    forward: BTreeMap<WorkspaceAnalysisNode, Vec<usize>>,
    reverse: BTreeMap<WorkspaceAnalysisNode, Vec<usize>>,
}

impl WorkspaceAnalysis {
    pub(crate) fn build(projection: WorkspaceGraphProjection) -> Result<Self, Vec<Diagnostic>> {
        let (workspace_graph_digest, workspace_graph_output_bytes) =
            crate::workspace_graph::projection_graph_binding(&projection)?;
        let (result, overflowed, builder_bytes) =
            crate::bounded_output::with_limit_usage(MAX_BUILDER_BYTES, || {
                Self::build_bounded(projection)
            });
        if overflowed {
            return Err(vec![limit_error("builder_bytes", MAX_BUILDER_BYTES)]);
        }
        result.map(|mut analysis| {
            analysis.workspace_graph_digest = workspace_graph_digest;
            analysis.workspace_graph_output_bytes = workspace_graph_output_bytes;
            analysis.builder_bytes = builder_bytes;
            analysis
        })
    }

    fn build_bounded(projection: WorkspaceGraphProjection) -> Result<Self, Vec<Diagnostic>> {
        let mut modules = BTreeMap::new();
        let mut callables = BTreeMap::new();
        let mut permits = BTreeMap::new();
        for (module_index, module) in projection.modules().iter().enumerate() {
            let node = module_node(module);
            reserve_entry::<(WorkspaceAnalysisNode, usize)>()?;
            if modules.insert(clone_node(&node), module_index).is_some() {
                return Err(vec![invariant_error(
                    "workspace analysis module index contains a duplicate typed module node",
                )]);
            }
            let mut module_permits = BTreeSet::new();
            for permit in module.permits() {
                reserve_entry::<String>()?;
                if !module_permits.insert(budgeted_clone(permit)) {
                    return Err(vec![invariant_error(
                        "workspace analysis module permits contain a duplicate capability",
                    )]);
                }
            }
            reserve_entry::<(WorkspaceAnalysisNode, BTreeSet<String>)>()?;
            permits.insert(node, module_permits);
            index_callables(module, module_index, &mut callables)?;
        }

        let mut prelude = BTreeSet::new();
        for id in projection.shared_prelude_ids() {
            reserve_entry::<String>()?;
            prelude.insert(budgeted_clone(id));
        }

        let mut declarations = BTreeMap::new();
        for declaration in projection.declarations() {
            reserve_entry::<(String, WorkspaceAnalysisDeclarationFact)>()?;
            let id = budgeted_clone(declaration.id());
            let fact = declaration_fact(declaration);
            if declarations.insert(id, fact).is_some() {
                return Err(vec![invariant_error(
                    "workspace analysis declaration index contains a duplicate identity",
                )]);
            }
        }

        let mut typed_edges = Vec::new();
        let mut capabilities = BTreeSet::new();
        for edge in projection.edges() {
            reserve_entry::<WorkspaceAnalysisTypedEdge>()?;
            let typed = typed_edge(edge)?;
            if let WorkspaceAnalysisNode::Capability(capability) = &typed.target {
                reserve_entry::<String>()?;
                capabilities.insert(budgeted_clone(capability));
            }
            typed_edges.push(typed);
        }

        validate_typed_endpoints(
            projection.modules(),
            &modules,
            &declarations,
            &capabilities,
            projection.edges(),
            &typed_edges,
        )?;

        let mut families = BTreeMap::<WorkspaceAnalysisEdgeFamily, Vec<usize>>::new();
        let mut forward = BTreeMap::<WorkspaceAnalysisNode, Vec<usize>>::new();
        let mut reverse = BTreeMap::<WorkspaceAnalysisNode, Vec<usize>>::new();
        for (index, (raw, typed)) in projection.edges().iter().zip(&typed_edges).enumerate() {
            replay_typed_edge(raw, typed, &modules, &declarations, &capabilities)?;
            push_index(&mut families, typed.family, index)?;
            push_index(&mut forward, clone_node(&typed.source), index)?;
            push_index(&mut reverse, clone_node(&typed.target), index)?;
        }
        for family in EDGE_FAMILIES {
            families.entry(family).or_default();
        }
        validate_adjacency_replay(&typed_edges, &families, &forward, &reverse)?;

        Ok(Self {
            projection,
            workspace_graph_digest: String::new(),
            workspace_graph_output_bytes: 0,
            builder_bytes: 0,
            modules,
            declarations,
            capabilities,
            callables,
            prelude,
            permits,
            typed_edges,
            families,
            forward,
            reverse,
        })
    }

    pub(crate) fn workspace_revision(&self) -> &str {
        self.projection.workspace_revision()
    }

    pub(crate) fn entry_module(&self) -> &str {
        self.projection.entry_module()
    }

    pub(crate) fn modules(&self) -> &[WorkspaceGraphProjectionModule] {
        self.projection.modules()
    }

    pub(crate) fn edges(&self) -> &[WorkspaceEdge] {
        self.projection.edges()
    }

    pub(crate) fn usage(&self) -> WorkspaceGraphProjectionUsage {
        self.projection.usage()
    }

    #[cfg(test)]
    pub(crate) fn family_edges(
        &self,
        family: WorkspaceAnalysisEdgeFamily,
    ) -> impl Iterator<Item = &WorkspaceEdge> {
        self.families[&family]
            .iter()
            .map(|index| &self.projection.edges()[*index])
    }

    pub(crate) fn context(
        &self,
        target: WorkspaceAnalysisTarget,
        direction: WorkspaceAnalysisDirection,
        depth: usize,
        max_nodes: usize,
    ) -> Result<WorkspaceContextFacts, Vec<Diagnostic>> {
        self.context_with_builder_start(target, direction, depth, max_nodes, self.builder_bytes)
    }

    fn context_with_builder_start(
        &self,
        target: WorkspaceAnalysisTarget,
        direction: WorkspaceAnalysisDirection,
        depth: usize,
        max_nodes: usize,
        builder_start: usize,
    ) -> Result<WorkspaceContextFacts, Vec<Diagnostic>> {
        let remaining_builder_bytes = MAX_BUILDER_BYTES
            .checked_sub(builder_start)
            .ok_or_else(|| vec![limit_error("builder_bytes", MAX_BUILDER_BYTES)])?;
        let (result, overflowed, builder_bytes) =
            crate::bounded_output::with_limit_usage(remaining_builder_bytes, || {
                validate_traversal_limits(depth, max_nodes)?;
                let root = self.select(&target)?;
                let traversal = self.traverse(&root, direction, depth, max_nodes)?;
                Ok(WorkspaceContextFacts {
                    target,
                    direction,
                    nodes: self.materialize_nodes(&traversal)?,
                    path_edges: self.context_edges(&traversal, direction)?,
                    truncation: self.materialize_truncation(&traversal)?,
                    used_builder_bytes: 0,
                    aggregate_builder_bytes: 0,
                })
            });
        if overflowed {
            return Err(vec![limit_error("builder_bytes", MAX_BUILDER_BYTES)]);
        }
        result.and_then(|mut facts| {
            facts.used_builder_bytes = checked_builder_sum(self.builder_bytes, builder_bytes)?;
            facts.aggregate_builder_bytes = checked_builder_sum(builder_start, builder_bytes)?;
            Ok(facts)
        })
    }

    pub(crate) fn impact(
        &self,
        target: WorkspaceAnalysisTarget,
        depth: usize,
        max_nodes: usize,
    ) -> Result<WorkspaceImpactFacts, Vec<Diagnostic>> {
        self.impact_with_builder_start(target, depth, max_nodes, self.builder_bytes)
    }

    fn impact_with_builder_start(
        &self,
        target: WorkspaceAnalysisTarget,
        depth: usize,
        max_nodes: usize,
        builder_start: usize,
    ) -> Result<WorkspaceImpactFacts, Vec<Diagnostic>> {
        let remaining_builder_bytes = MAX_BUILDER_BYTES
            .checked_sub(builder_start)
            .ok_or_else(|| vec![limit_error("builder_bytes", MAX_BUILDER_BYTES)])?;
        let (result, overflowed, builder_bytes) =
            crate::bounded_output::with_limit_usage(remaining_builder_bytes, || {
                validate_traversal_limits(depth, max_nodes)?;
                let root = self.select(&target)?;
                let traversal =
                    self.traverse(&root, WorkspaceAnalysisDirection::Reverse, depth, max_nodes)?;
                let mut nodes = Vec::new();
                for node in self.materialize_nodes(&traversal)? {
                    reserve_entry::<WorkspaceImpactNodeFact>()?;
                    let role = if node.depth == 0 {
                        WorkspaceImpactRole::Root
                    } else {
                        match node.node {
                            WorkspaceAnalysisNode::Module { .. } => {
                                WorkspaceImpactRole::ModuleConsumer
                            }
                            WorkspaceAnalysisNode::Declaration(_) => {
                                WorkspaceImpactRole::DeclarationConsumer
                            }
                            WorkspaceAnalysisNode::Capability(_) => {
                                WorkspaceImpactRole::CapabilityConsumer
                            }
                        }
                    };
                    nodes.push(WorkspaceImpactNodeFact { node, role });
                }
                Ok(WorkspaceImpactFacts {
                    target,
                    nodes,
                    path_edges: self.impact_path_edges(&traversal)?,
                    truncation: self.materialize_truncation(&traversal)?,
                    used_builder_bytes: 0,
                    aggregate_builder_bytes: 0,
                })
            });
        if overflowed {
            return Err(vec![limit_error("builder_bytes", MAX_BUILDER_BYTES)]);
        }
        result.and_then(|mut facts| {
            facts.used_builder_bytes = checked_builder_sum(self.builder_bytes, builder_bytes)?;
            facts.aggregate_builder_bytes = checked_builder_sum(builder_start, builder_bytes)?;
            Ok(facts)
        })
    }

    pub(crate) fn render_context(
        &self,
        target: WorkspaceAnalysisTarget,
        direction: WorkspaceAnalysisDirection,
        depth: usize,
        max_bytes: usize,
        max_nodes: usize,
    ) -> Result<WorkspaceContextArtifact, Vec<Diagnostic>> {
        let result = (|| {
            validate_output_limit(max_bytes)?;
            self.render_context_with_output_limit(
                target, direction, depth, max_bytes, max_nodes, max_bytes,
            )
        })();
        map_artifact_result(WorkspaceAnalysisArtifactKind::Context, result)
    }

    fn render_context_with_output_limit(
        &self,
        target: WorkspaceAnalysisTarget,
        direction: WorkspaceAnalysisDirection,
        depth: usize,
        query_max_bytes: usize,
        max_nodes: usize,
        output_limit: usize,
    ) -> Result<WorkspaceContextArtifact, Vec<Diagnostic>> {
        self.render_context_with_output_limit_and_builder_start(
            target,
            direction,
            depth,
            query_max_bytes,
            max_nodes,
            output_limit,
            self.builder_bytes,
        )
    }

    #[allow(
        clippy::too_many_arguments,
        reason = "closed Context budget composition"
    )]
    fn render_context_with_output_limit_and_builder_start(
        &self,
        target: WorkspaceAnalysisTarget,
        direction: WorkspaceAnalysisDirection,
        depth: usize,
        query_max_bytes: usize,
        max_nodes: usize,
        output_limit: usize,
        builder_start: usize,
    ) -> Result<WorkspaceContextArtifact, Vec<Diagnostic>> {
        let facts =
            self.context_with_builder_start(target, direction, depth, max_nodes, builder_start)?;
        render_context_artifact(self, facts, depth, query_max_bytes, max_nodes, output_limit)
    }

    pub(crate) fn render_impact(
        &self,
        target: WorkspaceAnalysisTarget,
        depth: usize,
        max_bytes: usize,
        max_nodes: usize,
    ) -> Result<WorkspaceImpactArtifact, Vec<Diagnostic>> {
        let result = (|| {
            validate_output_limit(max_bytes)?;
            self.render_impact_with_output_limit(target, depth, max_bytes, max_nodes, max_bytes)
        })();
        map_artifact_result(WorkspaceAnalysisArtifactKind::Impact, result)
    }

    fn render_impact_with_output_limit(
        &self,
        target: WorkspaceAnalysisTarget,
        depth: usize,
        query_max_bytes: usize,
        max_nodes: usize,
        output_limit: usize,
    ) -> Result<WorkspaceImpactArtifact, Vec<Diagnostic>> {
        self.render_impact_with_output_limit_and_builder_start(
            target,
            depth,
            query_max_bytes,
            max_nodes,
            output_limit,
            self.builder_bytes,
        )
    }

    fn render_impact_with_output_limit_and_builder_start(
        &self,
        target: WorkspaceAnalysisTarget,
        depth: usize,
        query_max_bytes: usize,
        max_nodes: usize,
        output_limit: usize,
        builder_start: usize,
    ) -> Result<WorkspaceImpactArtifact, Vec<Diagnostic>> {
        let facts = self.impact_with_builder_start(target, depth, max_nodes, builder_start)?;
        render_impact_artifact(self, facts, depth, query_max_bytes, max_nodes, output_limit)
    }

    pub(crate) fn render_review(
        &self,
        target: WorkspaceAnalysisTarget,
    ) -> Result<WorkspaceReviewArtifact, Vec<Diagnostic>> {
        self.render_review_with_limit(target, MAX_REVIEW_OUTPUT_BYTES)
    }

    pub(crate) fn render_review_with_limit(
        &self,
        target: WorkspaceAnalysisTarget,
        output_limit: usize,
    ) -> Result<WorkspaceReviewArtifact, Vec<Diagnostic>> {
        let result = self.render_review_with_limit_inner(target, output_limit);
        map_artifact_result(WorkspaceAnalysisArtifactKind::Review, result)
    }

    fn render_review_with_limit_inner(
        &self,
        target: WorkspaceAnalysisTarget,
        output_limit: usize,
    ) -> Result<WorkspaceReviewArtifact, Vec<Diagnostic>> {
        if output_limit > MAX_REVIEW_OUTPUT_BYTES {
            return Err(vec![limit_error("output_bytes", MAX_REVIEW_OUTPUT_BYTES)]);
        }
        self.select(&target)?;
        let mandatory = mandatory_review_reservation(self, &target, output_limit)?;
        let Some(mut remaining) = output_limit.checked_sub(mandatory) else {
            return Err(vec![limit_error("output_bytes", output_limit)]);
        };
        let context_limit = MAX_OUTPUT_BYTES.min(remaining);
        if context_limit < MIN_OUTPUT_BYTES {
            return Err(vec![limit_error("output_bytes", output_limit)]);
        }
        let context = self.render_context_with_output_limit_and_builder_start(
            target.clone(),
            WorkspaceAnalysisDirection::Both,
            MAX_TRAVERSAL_DEPTH,
            MAX_OUTPUT_BYTES,
            MAX_TRAVERSAL_NODES,
            context_limit,
            self.builder_bytes,
        )?;
        if is_truncated(&context.facts.truncation) {
            return Err(vec![Diagnostic::io(
                "SPX-G180",
                "Workspace Semantic Review requires complete Context and Impact evidence",
            )]);
        }
        remaining = remaining.checked_sub(context.json.len()).ok_or_else(|| {
            vec![invariant_error(
                "Workspace Semantic Review replay or digest binding disagrees",
            )]
        })?;
        let impact_limit = MAX_OUTPUT_BYTES.min(remaining);
        if impact_limit < MIN_OUTPUT_BYTES {
            return Err(vec![Diagnostic::io(
                "SPX-G180",
                "Workspace Semantic Review requires complete Context and Impact evidence",
            )]);
        }
        let impact = self.render_impact_with_output_limit_and_builder_start(
            target,
            MAX_TRAVERSAL_DEPTH,
            MAX_OUTPUT_BYTES,
            MAX_TRAVERSAL_NODES,
            impact_limit,
            context.facts.aggregate_builder_bytes,
        )?;
        if is_truncated(&context.facts.truncation) || is_truncated(&impact.facts.truncation) {
            return Err(vec![Diagnostic::io(
                "SPX-G180",
                "Workspace Semantic Review requires complete Context and Impact evidence",
            )]);
        }
        render_review_artifact(self, context, impact, output_limit)
    }

    fn select(
        &self,
        target: &WorkspaceAnalysisTarget,
    ) -> Result<WorkspaceAnalysisNode, Vec<Diagnostic>> {
        validate_target_text(target.value())?;
        match target.kind() {
            WorkspaceAnalysisTargetKind::Declaration => {
                let id = target.value();
                let Some(fact) = self.declarations.get(id) else {
                    return Err(vec![target_domain_error(
                        "Workspace Semantic Analysis target is not in the authenticated entry closure",
                    )]);
                };
                if fact.origin == IdentityOrigin::CompilerOwned {
                    return Err(vec![target_domain_error(
                        "Workspace Semantic Analysis compiler-owned declaration targets are unsupported",
                    )]);
                }
                Ok(WorkspaceAnalysisNode::Declaration(budgeted_clone(id)))
            }
            WorkspaceAnalysisTargetKind::Capability => {
                let capability = target.value();
                if !self.capabilities.contains(capability) {
                    return Err(vec![target_domain_error(
                        "Workspace Semantic Analysis target is not in the authenticated entry closure",
                    )]);
                }
                Ok(WorkspaceAnalysisNode::Capability(budgeted_clone(
                    capability,
                )))
            }
        }
    }

    fn traverse(
        &self,
        root: &WorkspaceAnalysisNode,
        direction: WorkspaceAnalysisDirection,
        max_depth: usize,
        max_nodes: usize,
    ) -> Result<Traversal, Vec<Diagnostic>> {
        let requested: &[WorkspaceAnalysisReachedBy] = match direction {
            WorkspaceAnalysisDirection::Forward => &[WorkspaceAnalysisReachedBy::Forward],
            WorkspaceAnalysisDirection::Reverse => &[WorkspaceAnalysisReachedBy::Reverse],
            WorkspaceAnalysisDirection::Both => &[
                WorkspaceAnalysisReachedBy::Forward,
                WorkspaceAnalysisReachedBy::Reverse,
            ],
        };
        let mut state_depths =
            BTreeMap::<(WorkspaceAnalysisNode, WorkspaceAnalysisReachedBy), usize>::new();
        let mut queue = VecDeque::new();
        reserve_entry::<(WorkspaceAnalysisNode, usize)>()?;
        let mut node_depths = BTreeMap::from([(clone_node(root), 0usize)]);
        for reached_by in requested.iter().copied() {
            reserve_entry::<((WorkspaceAnalysisNode, WorkspaceAnalysisReachedBy), usize)>()?;
            state_depths.insert((clone_node(root), reached_by), 0);
            reserve_entry::<(WorkspaceAnalysisNode, WorkspaceAnalysisReachedBy)>()?;
            queue.push_back((clone_node(root), reached_by));
        }

        let mut deferred =
            BTreeMap::<WorkspaceAnalysisNode, (usize, BTreeSet<WorkspaceAnalysisReachedBy>)>::new();
        while let Some((node, reached_by)) = queue.pop_front() {
            let depth = state_depths[&(clone_node(&node), reached_by)];
            if depth > max_depth {
                continue;
            }
            let adjacency = match reached_by {
                WorkspaceAnalysisReachedBy::Forward => self.forward.get(&node),
                WorkspaceAnalysisReachedBy::Reverse => self.reverse.get(&node),
                WorkspaceAnalysisReachedBy::Root => None,
            };
            let Some(adjacency) = adjacency else {
                continue;
            };
            for edge_index in adjacency {
                let edge = &self.typed_edges[*edge_index];
                let neighbor = match reached_by {
                    WorkspaceAnalysisReachedBy::Forward => &edge.target,
                    WorkspaceAnalysisReachedBy::Reverse => &edge.source,
                    WorkspaceAnalysisReachedBy::Root => unreachable!(),
                };
                let next_depth = depth
                    .checked_add(1)
                    .ok_or_else(|| vec![limit_error("traversal_depth", MAX_TRAVERSAL_DEPTH)])?;
                let state_key = (clone_node(neighbor), reached_by);
                if state_depths.contains_key(&state_key) {
                    continue;
                }
                if !node_depths.contains_key(neighbor) && node_depths.len() == MAX_TRAVERSAL_NODES {
                    if !deferred.contains_key(neighbor) {
                        reserve_entry::<(
                            WorkspaceAnalysisNode,
                            (usize, BTreeSet<WorkspaceAnalysisReachedBy>),
                        )>()?;
                        deferred.insert(clone_node(neighbor), (next_depth, BTreeSet::new()));
                    }
                    let deferred_entry = deferred.get_mut(neighbor).ok_or_else(|| {
                        vec![invariant_error(
                            "workspace analysis deferred-node replay disagrees",
                        )]
                    })?;
                    deferred_entry.0 = deferred_entry.0.min(next_depth);
                    reserve_entry::<WorkspaceAnalysisReachedBy>()?;
                    deferred_entry.1.insert(reached_by);
                    continue;
                }
                reserve_entry::<((WorkspaceAnalysisNode, WorkspaceAnalysisReachedBy), usize)>()?;
                state_depths.insert(state_key, next_depth);
                node_depths
                    .entry(clone_node(neighbor))
                    .and_modify(|known| *known = (*known).min(next_depth))
                    .or_insert(next_depth);
                if next_depth <= max_depth {
                    reserve_entry::<(WorkspaceAnalysisNode, WorkspaceAnalysisReachedBy)>()?;
                    queue.push_back((clone_node(neighbor), reached_by));
                }
            }
        }

        let mut reached_by =
            BTreeMap::<WorkspaceAnalysisNode, BTreeSet<WorkspaceAnalysisReachedBy>>::new();
        reserve_entry::<WorkspaceAnalysisReachedBy>()?;
        let mut root_reached = BTreeSet::new();
        root_reached.insert(WorkspaceAnalysisReachedBy::Root);
        reached_by.insert(clone_node(root), root_reached);
        for ((node, direction), depth) in &state_depths {
            if node != root && node_depths[node] == *depth {
                reserve_entry::<WorkspaceAnalysisReachedBy>()?;
                reached_by
                    .entry(clone_node(node))
                    .or_default()
                    .insert(*direction);
            }
        }

        let mut candidates = Vec::new();
        for (node, depth) in &node_depths {
            if *depth <= max_depth {
                reserve_entry::<WorkspaceAnalysisNode>()?;
                candidates.push(clone_node(node));
            }
        }
        candidates.sort_by(|left, right| {
            node_depths[left]
                .cmp(&node_depths[right])
                .then_with(|| compare_nodes(left, right, &self.declarations))
        });
        let mut emitted = BTreeSet::new();
        for node in candidates.into_iter().take(max_nodes) {
            reserve_entry::<WorkspaceAnalysisNode>()?;
            emitted.insert(node);
        }
        let omitted_known_nodes = node_depths
            .keys()
            .filter(|node| !emitted.contains(*node))
            .count();
        let first_omitted_depth = node_depths
            .iter()
            .filter(|(node, _)| !emitted.contains(*node))
            .map(|(_, depth)| *depth)
            .min();
        let mut frontier = Vec::new();
        if let Some(first_omitted_depth) = first_omitted_depth {
            for (node, depth) in &node_depths {
                if !emitted.contains(node) && *depth == first_omitted_depth {
                    reserve_entry::<WorkspaceAnalysisNode>()?;
                    frontier.push(clone_node(node));
                }
            }
            frontier.sort_by(|left, right| compare_nodes(left, right, &self.declarations));
        }
        Ok(Traversal {
            node_depths,
            state_depths,
            reached_by,
            emitted,
            frontier,
            omitted_known_nodes,
            deferred,
        })
    }

    fn context_edges(
        &self,
        traversal: &Traversal,
        direction: WorkspaceAnalysisDirection,
    ) -> Result<Vec<WorkspaceAnalysisPathEdge>, Vec<Diagnostic>> {
        let mut selected = BTreeMap::<usize, (usize, WorkspaceAnalysisReachedBy)>::new();
        for (edge_index, edge) in self.typed_edges.iter().enumerate() {
            if !traversal.emitted.contains(&edge.source)
                || !traversal.emitted.contains(&edge.target)
            {
                continue;
            }
            for reached_by in [
                WorkspaceAnalysisReachedBy::Forward,
                WorkspaceAnalysisReachedBy::Reverse,
            ] {
                let enabled = matches!(
                    (direction, reached_by),
                    (
                        WorkspaceAnalysisDirection::Forward | WorkspaceAnalysisDirection::Both,
                        WorkspaceAnalysisReachedBy::Forward
                    ) | (
                        WorkspaceAnalysisDirection::Reverse | WorkspaceAnalysisDirection::Both,
                        WorkspaceAnalysisReachedBy::Reverse
                    )
                );
                if !enabled {
                    continue;
                }
                let (source, target) = match reached_by {
                    WorkspaceAnalysisReachedBy::Forward => (&edge.source, &edge.target),
                    WorkspaceAnalysisReachedBy::Reverse => (&edge.target, &edge.source),
                    WorkspaceAnalysisReachedBy::Root => unreachable!(),
                };
                let Some(source_depth) = traversal
                    .state_depths
                    .get(&(clone_node(source), reached_by))
                else {
                    continue;
                };
                let Some(target_depth) = traversal
                    .state_depths
                    .get(&(clone_node(target), reached_by))
                else {
                    continue;
                };
                let depth = (*source_depth).max(*target_depth);
                match selected.entry(edge_index) {
                    std::collections::btree_map::Entry::Vacant(entry) => {
                        reserve_entry::<(usize, (usize, WorkspaceAnalysisReachedBy))>()?;
                        entry.insert((depth, reached_by));
                    }
                    std::collections::btree_map::Entry::Occupied(mut entry) => {
                        let (known_depth, known_direction) = entry.get_mut();
                        *known_depth = (*known_depth).min(depth);
                        *known_direction = (*known_direction).min(reached_by);
                    }
                }
            }
        }
        let mut edges = Vec::new();
        for (edge_index, (depth, reached_by)) in selected {
            reserve_entry::<WorkspaceAnalysisPathEdge>()?;
            edges.push(WorkspaceAnalysisPathEdge {
                edge_index,
                reached_by,
                depth,
            });
        }
        edges.sort_by_key(|edge| edge.edge_index);
        Ok(edges)
    }

    fn impact_path_edges(
        &self,
        traversal: &Traversal,
    ) -> Result<Vec<WorkspaceAnalysisPathEdge>, Vec<Diagnostic>> {
        let reached_by = WorkspaceAnalysisReachedBy::Reverse;
        let mut edges = Vec::new();
        for (edge_index, edge) in self.typed_edges.iter().enumerate() {
            if !traversal.emitted.contains(&edge.source)
                || !traversal.emitted.contains(&edge.target)
            {
                continue;
            }
            let Some(target_depth) = traversal
                .state_depths
                .get(&(clone_node(&edge.target), reached_by))
            else {
                continue;
            };
            let Some(source_depth) = traversal
                .state_depths
                .get(&(clone_node(&edge.source), reached_by))
            else {
                continue;
            };
            if source_depth == &target_depth.saturating_add(1)
                && traversal.node_depths.get(&edge.source) == Some(source_depth)
            {
                reserve_entry::<WorkspaceAnalysisPathEdge>()?;
                edges.push(WorkspaceAnalysisPathEdge {
                    edge_index,
                    reached_by,
                    depth: *source_depth,
                });
            }
        }
        edges.sort_by_key(|edge| edge.edge_index);
        Ok(edges)
    }

    fn materialize_nodes(
        &self,
        traversal: &Traversal,
    ) -> Result<Vec<WorkspaceAnalysisNodeFact>, Vec<Diagnostic>> {
        let mut nodes = Vec::new();
        for node in &traversal.emitted {
            let depth = traversal.node_depths[node];
            nodes.push(self.materialize_node(
                node,
                depth,
                collect_reached_by(&traversal.reached_by[node])?,
            )?);
        }
        nodes.sort_by(|left, right| {
            left.depth
                .cmp(&right.depth)
                .then_with(|| compare_node_facts(left, right))
        });
        Ok(nodes)
    }

    fn materialize_truncation(
        &self,
        traversal: &Traversal,
    ) -> Result<WorkspaceAnalysisTruncationFacts, Vec<Diagnostic>> {
        let mut frontier = Vec::new();
        for node in &traversal.frontier {
            let depth = traversal.node_depths[node];
            frontier.push(self.materialize_node(
                node,
                depth,
                collect_reached_by(&traversal.reached_by[node])?,
            )?);
        }
        frontier.sort_by(compare_node_facts);
        Ok(WorkspaceAnalysisTruncationFacts {
            frontier,
            omitted_known_nodes: traversal.omitted_known_nodes,
            deferred_known_nodes: traversal.deferred.len(),
            byte_truncated: false,
        })
    }

    fn materialize_node(
        &self,
        node: &WorkspaceAnalysisNode,
        depth: usize,
        reached_by: Vec<WorkspaceAnalysisReachedBy>,
    ) -> Result<WorkspaceAnalysisNodeFact, Vec<Diagnostic>> {
        reserve_entry::<WorkspaceAnalysisNodeFact>()?;
        let (kind, origin, owner, path, module) = match node {
            WorkspaceAnalysisNode::Module { path, module } => {
                if !self.modules.contains_key(node) {
                    return Err(vec![invariant_error(
                        "workspace analysis traversal reached an unauthenticated module node",
                    )]);
                }
                (
                    None,
                    None,
                    None,
                    Some(budgeted_clone(path)),
                    Some(budgeted_clone(module)),
                )
            }
            WorkspaceAnalysisNode::Declaration(id) => {
                let fact = self.declarations.get(id).ok_or_else(|| {
                    vec![invariant_error(
                        "workspace analysis traversal reached an unknown declaration node",
                    )]
                })?;
                (
                    Some(fact.kind),
                    Some(fact.origin),
                    fact.owner.as_deref().map(budgeted_clone),
                    fact.path.as_deref().map(budgeted_clone),
                    fact.module.as_deref().map(budgeted_clone),
                )
            }
            WorkspaceAnalysisNode::Capability(capability) => {
                if !self.capabilities.contains(capability) {
                    return Err(vec![invariant_error(
                        "workspace analysis traversal reached an unknown capability node",
                    )]);
                }
                (None, None, None, None, None)
            }
        };
        Ok(WorkspaceAnalysisNodeFact {
            node: clone_node(node),
            depth,
            reached_by,
            declaration_kind: kind,
            identity_origin: origin,
            owner,
            path,
            module,
        })
    }
}

struct Traversal {
    node_depths: BTreeMap<WorkspaceAnalysisNode, usize>,
    state_depths: BTreeMap<(WorkspaceAnalysisNode, WorkspaceAnalysisReachedBy), usize>,
    reached_by: BTreeMap<WorkspaceAnalysisNode, BTreeSet<WorkspaceAnalysisReachedBy>>,
    emitted: BTreeSet<WorkspaceAnalysisNode>,
    frontier: Vec<WorkspaceAnalysisNode>,
    omitted_known_nodes: usize,
    deferred: BTreeMap<WorkspaceAnalysisNode, (usize, BTreeSet<WorkspaceAnalysisReachedBy>)>,
}

fn module_node(module: &WorkspaceGraphProjectionModule) -> WorkspaceAnalysisNode {
    WorkspaceAnalysisNode::Module {
        path: budgeted_clone(module.path()),
        module: budgeted_clone(module.module()),
    }
}

fn declaration_fact(
    declaration: &WorkspaceGraphProjectionDeclaration,
) -> WorkspaceAnalysisDeclarationFact {
    WorkspaceAnalysisDeclarationFact {
        kind: declaration.kind(),
        origin: declaration.origin(),
        owner: declaration.owner().map(budgeted_clone),
        path: declaration.path().map(budgeted_clone),
        module: declaration.module().map(budgeted_clone),
    }
}

fn index_callables(
    module: &WorkspaceGraphProjectionModule,
    module_index: usize,
    callables: &mut BTreeMap<String, WorkspaceAnalysisCallableLocation>,
) -> Result<(), Vec<Diagnostic>> {
    for (callable_index, function) in module.functions().iter().enumerate() {
        insert_callable(
            callables,
            function.id.as_str(),
            WorkspaceAnalysisCallableLocation {
                module_index,
                callable_index,
                kind: WorkspaceAnalysisCallableKind::Function,
            },
        )?;
    }
    for (callable_index, template) in module.function_templates().iter().enumerate() {
        insert_callable(
            callables,
            template.id.as_str(),
            WorkspaceAnalysisCallableLocation {
                module_index,
                callable_index,
                kind: WorkspaceAnalysisCallableKind::Template,
            },
        )?;
    }
    for (callable_index, instance) in module.function_instances().iter().enumerate() {
        insert_callable(
            callables,
            instance.id.as_str(),
            WorkspaceAnalysisCallableLocation {
                module_index,
                callable_index,
                kind: WorkspaceAnalysisCallableKind::Instance,
            },
        )?;
    }
    Ok(())
}

fn insert_callable(
    callables: &mut BTreeMap<String, WorkspaceAnalysisCallableLocation>,
    id: &str,
    location: WorkspaceAnalysisCallableLocation,
) -> Result<(), Vec<Diagnostic>> {
    reserve_entry::<(String, WorkspaceAnalysisCallableLocation)>()?;
    if callables.insert(budgeted_clone(id), location).is_some() {
        return Err(vec![invariant_error(
            "workspace analysis callable index contains a duplicate identity",
        )]);
    }
    Ok(())
}

fn typed_edge(edge: &WorkspaceEdge) -> Result<WorkspaceAnalysisTypedEdge, Vec<Diagnostic>> {
    let family = WorkspaceAnalysisEdgeFamily::from_name(edge.kind()).ok_or_else(|| {
        vec![invariant_error(
            "workspace analysis encountered an unknown authenticated edge family",
        )]
    })?;
    let source = match family {
        WorkspaceAnalysisEdgeFamily::FunctionImport
        | WorkspaceAnalysisEdgeFamily::TypeImport
        | WorkspaceAnalysisEdgeFamily::CapabilityAuthority => WorkspaceAnalysisNode::Module {
            path: budgeted_clone(edge.caller_path()),
            module: budgeted_clone(edge.caller()),
        },
        WorkspaceAnalysisEdgeFamily::Call
        | WorkspaceAnalysisEdgeFamily::TypeReference
        | WorkspaceAnalysisEdgeFamily::EffectRequirement => {
            WorkspaceAnalysisNode::Declaration(budgeted_clone(edge.caller()))
        }
    };
    let target = match family {
        WorkspaceAnalysisEdgeFamily::FunctionImport
        | WorkspaceAnalysisEdgeFamily::TypeImport
        | WorkspaceAnalysisEdgeFamily::Call
        | WorkspaceAnalysisEdgeFamily::TypeReference => {
            WorkspaceAnalysisNode::Declaration(budgeted_clone(edge.target()))
        }
        WorkspaceAnalysisEdgeFamily::EffectRequirement
        | WorkspaceAnalysisEdgeFamily::CapabilityAuthority => {
            WorkspaceAnalysisNode::Capability(budgeted_clone(edge.target()))
        }
    };
    Ok(WorkspaceAnalysisTypedEdge {
        source,
        target,
        family,
    })
}

fn replay_typed_edge(
    raw: &WorkspaceEdge,
    typed: &WorkspaceAnalysisTypedEdge,
    modules: &BTreeMap<WorkspaceAnalysisNode, usize>,
    declarations: &BTreeMap<String, WorkspaceAnalysisDeclarationFact>,
    capabilities: &BTreeSet<String>,
) -> Result<(), Vec<Diagnostic>> {
    if WorkspaceAnalysisEdgeFamily::from_name(raw.kind()) != Some(typed.family) {
        return Err(vec![invariant_error(
            "workspace analysis typed edge family replay disagrees",
        )]);
    }
    match (&typed.source, typed.family) {
        (
            WorkspaceAnalysisNode::Module { path, module },
            WorkspaceAnalysisEdgeFamily::FunctionImport
            | WorkspaceAnalysisEdgeFamily::TypeImport
            | WorkspaceAnalysisEdgeFamily::CapabilityAuthority,
        ) if path == raw.caller_path()
            && module == raw.caller()
            && modules.contains_key(&typed.source) => {}
        (
            WorkspaceAnalysisNode::Declaration(id),
            WorkspaceAnalysisEdgeFamily::Call
            | WorkspaceAnalysisEdgeFamily::TypeReference
            | WorkspaceAnalysisEdgeFamily::EffectRequirement,
        ) if id == raw.caller()
            && declarations.get(id).and_then(|fact| fact.path.as_deref())
                == Some(raw.caller_path()) => {}
        _ => {
            return Err(vec![invariant_error(
                "workspace analysis typed edge source replay disagrees",
            )])
        }
    }
    match (&typed.target, typed.family) {
        (
            WorkspaceAnalysisNode::Declaration(id),
            WorkspaceAnalysisEdgeFamily::FunctionImport
            | WorkspaceAnalysisEdgeFamily::TypeImport
            | WorkspaceAnalysisEdgeFamily::Call
            | WorkspaceAnalysisEdgeFamily::TypeReference,
        ) if id == raw.target()
            && declarations.get(id).and_then(|fact| fact.path.as_deref())
                == Some(raw.target_path()) => {}
        (
            WorkspaceAnalysisNode::Capability(capability),
            WorkspaceAnalysisEdgeFamily::EffectRequirement
            | WorkspaceAnalysisEdgeFamily::CapabilityAuthority,
        ) if capability == raw.target() && capabilities.contains(capability) => {}
        _ => {
            return Err(vec![invariant_error(
                "workspace analysis typed edge target replay disagrees",
            )])
        }
    }
    Ok(())
}

fn validate_typed_endpoints(
    projection_modules: &[WorkspaceGraphProjectionModule],
    modules: &BTreeMap<WorkspaceAnalysisNode, usize>,
    declarations: &BTreeMap<String, WorkspaceAnalysisDeclarationFact>,
    capabilities: &BTreeSet<String>,
    raw_edges: &[WorkspaceEdge],
    typed_edges: &[WorkspaceAnalysisTypedEdge],
) -> Result<(), Vec<Diagnostic>> {
    if raw_edges.len() != typed_edges.len() || modules.len() != projection_modules.len() {
        return Err(vec![invariant_error(
            "workspace analysis typed endpoint cardinality disagrees with authenticated facts",
        )]);
    }
    for typed in typed_edges {
        match &typed.source {
            WorkspaceAnalysisNode::Module { .. } if modules.contains_key(&typed.source) => {}
            WorkspaceAnalysisNode::Declaration(id) if declarations.contains_key(id) => {}
            WorkspaceAnalysisNode::Capability(_) => {
                return Err(vec![invariant_error(
                    "workspace analysis capability cannot be an authenticated edge source",
                )])
            }
            _ => {
                return Err(vec![invariant_error(
                    "workspace analysis edge source is absent from its typed namespace",
                )])
            }
        }
        match &typed.target {
            WorkspaceAnalysisNode::Declaration(id) if declarations.contains_key(id) => {}
            WorkspaceAnalysisNode::Capability(name) if capabilities.contains(name) => {}
            WorkspaceAnalysisNode::Module { .. } => {
                return Err(vec![invariant_error(
                    "workspace analysis module cannot be an authenticated edge target",
                )])
            }
            _ => {
                return Err(vec![invariant_error(
                    "workspace analysis edge target is absent from its typed namespace",
                )])
            }
        }
    }
    Ok(())
}

fn validate_adjacency_replay(
    typed_edges: &[WorkspaceAnalysisTypedEdge],
    families: &BTreeMap<WorkspaceAnalysisEdgeFamily, Vec<usize>>,
    forward: &BTreeMap<WorkspaceAnalysisNode, Vec<usize>>,
    reverse: &BTreeMap<WorkspaceAnalysisNode, Vec<usize>>,
) -> Result<(), Vec<Diagnostic>> {
    reserve_bytes(typed_edges.len())?;
    let mut seen = vec![0_u8; typed_edges.len()];
    for family in EDGE_FAMILIES {
        for edge_index in &families[&family] {
            if typed_edges.get(*edge_index).map(|edge| edge.family) != Some(family) {
                return Err(vec![invariant_error(
                    "workspace analysis family index disagrees with typed edge replay",
                )]);
            }
            mark_seen(&mut seen, *edge_index)?;
        }
    }
    if seen.iter().any(|count| *count != 1) {
        return Err(vec![invariant_error(
            "workspace analysis family index omits or duplicates an authenticated edge",
        )]);
    }
    seen.fill(0);
    for (node, edge_indexes) in forward {
        for edge_index in edge_indexes {
            if typed_edges.get(*edge_index).map(|edge| &edge.source) != Some(node) {
                return Err(vec![invariant_error(
                    "workspace analysis forward adjacency disagrees with typed edge replay",
                )]);
            }
            mark_seen(&mut seen, *edge_index)?;
        }
    }
    if seen.iter().any(|count| *count != 1) {
        return Err(vec![invariant_error(
            "workspace analysis forward adjacency omits or duplicates an authenticated edge",
        )]);
    }
    seen.fill(0);
    for (node, edge_indexes) in reverse {
        for edge_index in edge_indexes {
            if typed_edges.get(*edge_index).map(|edge| &edge.target) != Some(node) {
                return Err(vec![invariant_error(
                    "workspace analysis reverse adjacency disagrees with typed edge replay",
                )]);
            }
            mark_seen(&mut seen, *edge_index)?;
        }
    }
    if seen.iter().any(|count| *count != 1) {
        return Err(vec![invariant_error(
            "workspace analysis reverse adjacency omits or duplicates an authenticated edge",
        )]);
    }
    Ok(())
}

fn mark_seen(seen: &mut [u8], edge_index: usize) -> Result<(), Vec<Diagnostic>> {
    let Some(count) = seen.get_mut(edge_index) else {
        return Err(vec![invariant_error(
            "workspace analysis adjacency index is outside its authenticated edge vector",
        )]);
    };
    *count = count.checked_add(1).ok_or_else(|| {
        vec![invariant_error(
            "workspace analysis adjacency multiplicity overflowed",
        )]
    })?;
    Ok(())
}

fn validate_traversal_limits(depth: usize, max_nodes: usize) -> Result<(), Vec<Diagnostic>> {
    if depth > MAX_TRAVERSAL_DEPTH {
        return Err(vec![limit_error("traversal_depth", MAX_TRAVERSAL_DEPTH)]);
    }
    if max_nodes == 0 {
        return Err(vec![target_error(
            "Workspace Analysis traversal_nodes must be at least 1",
        )]);
    }
    if max_nodes > MAX_TRAVERSAL_NODES {
        return Err(vec![limit_error("traversal_nodes", MAX_TRAVERSAL_NODES)]);
    }
    Ok(())
}

fn compare_nodes(
    left: &WorkspaceAnalysisNode,
    right: &WorkspaceAnalysisNode,
    declarations: &BTreeMap<String, WorkspaceAnalysisDeclarationFact>,
) -> Ordering {
    match (left, right) {
        (
            WorkspaceAnalysisNode::Module {
                path: left_path,
                module: left_module,
            },
            WorkspaceAnalysisNode::Module {
                path: right_path,
                module: right_module,
            },
        ) => left_path
            .cmp(right_path)
            .then(left_module.cmp(right_module)),
        (WorkspaceAnalysisNode::Module { .. }, _) => Ordering::Less,
        (_, WorkspaceAnalysisNode::Module { .. }) => Ordering::Greater,
        (
            WorkspaceAnalysisNode::Declaration(left_id),
            WorkspaceAnalysisNode::Declaration(right_id),
        ) => declaration_kind_rank(declarations[left_id].kind)
            .cmp(&declaration_kind_rank(declarations[right_id].kind))
            .then(left_id.cmp(right_id)),
        (WorkspaceAnalysisNode::Declaration(_), WorkspaceAnalysisNode::Capability(_)) => {
            Ordering::Less
        }
        (WorkspaceAnalysisNode::Capability(_), WorkspaceAnalysisNode::Declaration(_)) => {
            Ordering::Greater
        }
        (
            WorkspaceAnalysisNode::Capability(left_name),
            WorkspaceAnalysisNode::Capability(right_name),
        ) => left_name.cmp(right_name),
    }
}

fn compare_node_facts(
    left: &WorkspaceAnalysisNodeFact,
    right: &WorkspaceAnalysisNodeFact,
) -> Ordering {
    match (&left.node, &right.node) {
        (
            WorkspaceAnalysisNode::Module {
                path: left_path,
                module: left_module,
            },
            WorkspaceAnalysisNode::Module {
                path: right_path,
                module: right_module,
            },
        ) => left_path
            .cmp(right_path)
            .then(left_module.cmp(right_module)),
        (WorkspaceAnalysisNode::Module { .. }, _) => Ordering::Less,
        (_, WorkspaceAnalysisNode::Module { .. }) => Ordering::Greater,
        (
            WorkspaceAnalysisNode::Declaration(left_id),
            WorkspaceAnalysisNode::Declaration(right_id),
        ) => declaration_kind_rank(left.declaration_kind.expect("declaration fact kind"))
            .cmp(&declaration_kind_rank(
                right.declaration_kind.expect("declaration fact kind"),
            ))
            .then(left_id.cmp(right_id)),
        (WorkspaceAnalysisNode::Declaration(_), WorkspaceAnalysisNode::Capability(_)) => {
            Ordering::Less
        }
        (WorkspaceAnalysisNode::Capability(_), WorkspaceAnalysisNode::Declaration(_)) => {
            Ordering::Greater
        }
        (
            WorkspaceAnalysisNode::Capability(left_name),
            WorkspaceAnalysisNode::Capability(right_name),
        ) => left_name.cmp(right_name),
    }
}

const fn declaration_kind_rank(kind: DeclarationKind) -> usize {
    match kind {
        DeclarationKind::Resource => 0,
        DeclarationKind::ResourceDrop => 1,
        DeclarationKind::Record => 2,
        DeclarationKind::Field => 3,
        DeclarationKind::Variant => 4,
        DeclarationKind::VariantCase => 5,
        DeclarationKind::CaseField => 6,
        DeclarationKind::Interface => 7,
        DeclarationKind::Import => 8,
        DeclarationKind::Function => 9,
    }
}

fn validate_target_text(value: &str) -> Result<(), Vec<Diagnostic>> {
    if value.len() > MAX_TARGET_BYTES {
        return Err(vec![limit_error("target_bytes", MAX_TARGET_BYTES)]);
    }
    if value.is_empty() || value.as_bytes().contains(&0) {
        return Err(vec![target_error(
            "Workspace Analysis target must contain 1..4096 non-NUL UTF-8 bytes",
        )]);
    }
    Ok(())
}

fn collect_reached_by(
    source: &BTreeSet<WorkspaceAnalysisReachedBy>,
) -> Result<Vec<WorkspaceAnalysisReachedBy>, Vec<Diagnostic>> {
    let mut reached_by = Vec::new();
    for value in source {
        reserve_entry::<WorkspaceAnalysisReachedBy>()?;
        reached_by.push(*value);
    }
    Ok(reached_by)
}

fn push_index<K: Ord>(
    index: &mut BTreeMap<K, Vec<usize>>,
    key: K,
    edge_index: usize,
) -> Result<(), Vec<Diagnostic>> {
    reserve_entry::<usize>()?;
    index.entry(key).or_default().push(edge_index);
    Ok(())
}

fn reserve_entry<T>() -> Result<(), Vec<Diagnostic>> {
    reserve_bytes(std::mem::size_of::<T>())
}

fn reserve_bytes(bytes: usize) -> Result<(), Vec<Diagnostic>> {
    if crate::bounded_output::reserve_active(bytes) {
        Ok(())
    } else {
        Err(vec![limit_error("builder_bytes", MAX_BUILDER_BYTES)])
    }
}

fn budgeted_clone(value: &str) -> String {
    crate::bounded_output::budgeted_clone(value)
}

fn clone_node(node: &WorkspaceAnalysisNode) -> WorkspaceAnalysisNode {
    match node {
        WorkspaceAnalysisNode::Module { path, module } => WorkspaceAnalysisNode::Module {
            path: budgeted_clone(path),
            module: budgeted_clone(module),
        },
        WorkspaceAnalysisNode::Declaration(id) => {
            WorkspaceAnalysisNode::Declaration(budgeted_clone(id))
        }
        WorkspaceAnalysisNode::Capability(capability) => {
            WorkspaceAnalysisNode::Capability(budgeted_clone(capability))
        }
    }
}

fn target_error(message: &'static str) -> Diagnostic {
    Diagnostic::io("SPX-G176", message)
}

fn target_domain_error(message: &'static str) -> Diagnostic {
    Diagnostic::io("SPX-G177", message)
}

fn invariant_error(message: &'static str) -> Diagnostic {
    Diagnostic::io("SPX-G179", message)
}

pub(crate) fn map_artifact_diagnostics(
    artifact: WorkspaceAnalysisArtifactKind,
    diagnostics: Vec<Diagnostic>,
) -> Vec<Diagnostic> {
    diagnostics
        .into_iter()
        .map(|diagnostic| match diagnostic.code {
            "SPX-G176"
                if diagnostic.message
                    == "Workspace Analysis traversal_nodes must be at least 1" =>
            {
                artifact_option_error(artifact, "max_nodes")
            }
            "SPX-G176"
                if diagnostic.message
                    == "Workspace Semantic Analysis max_bytes must be at least 4096" =>
            {
                artifact_option_error(artifact, "max_bytes")
            }
            "SPX-G178" => parse_limit_binding(&diagnostic.message)
                .map(|(field, maximum)| artifact_limit_error(artifact, field, maximum))
                .unwrap_or(diagnostic),
            "SPX-G179" if diagnostic.message != artifact.invariant_message() => {
                invariant_error(artifact.invariant_message())
            }
            _ => diagnostic,
        })
        .collect()
}

fn map_artifact_result<T>(
    artifact: WorkspaceAnalysisArtifactKind,
    result: Result<T, Vec<Diagnostic>>,
) -> Result<T, Vec<Diagnostic>> {
    result.map_err(|diagnostics| map_artifact_diagnostics(artifact, diagnostics))
}

fn parse_limit_binding(message: &str) -> Option<(&str, usize)> {
    let field_start = message.find('`')?.checked_add(1)?;
    let remainder = message.get(field_start..)?;
    let (field, maximum) = remainder.split_once("` exceeds ")?;
    Some((field, maximum.parse().ok()?))
}

fn artifact_option_error(artifact: WorkspaceAnalysisArtifactKind, name: &str) -> Diagnostic {
    Diagnostic::io(
        "SPX-G176",
        crate::bounded_output::budgeted_format(format_args!(
            "Workspace Semantic {} option `{name}` is not canonical",
            artifact.name()
        )),
    )
}

fn artifact_limit_error(
    artifact: WorkspaceAnalysisArtifactKind,
    field: &str,
    maximum: usize,
) -> Diagnostic {
    Diagnostic::io(
        "SPX-G178",
        crate::bounded_output::budgeted_format(format_args!(
            "Workspace Semantic {} `{field}` exceeds {maximum}",
            artifact.name()
        )),
    )
}

fn limit_error(field: &'static str, maximum: usize) -> Diagnostic {
    Diagnostic::io(
        "SPX-G178",
        crate::bounded_output::budgeted_format(format_args!(
            "Workspace Analysis `{field}` exceeds {maximum}"
        )),
    )
}

fn checked_builder_sum(current: usize, additional: usize) -> Result<usize, Vec<Diagnostic>> {
    let used = current
        .checked_add(additional)
        .ok_or_else(|| vec![limit_error("builder_bytes", MAX_BUILDER_BYTES)])?;
    if used > MAX_BUILDER_BYTES {
        return Err(vec![limit_error("builder_bytes", MAX_BUILDER_BYTES)]);
    }
    Ok(used)
}

fn validate_output_limit(max_bytes: usize) -> Result<(), Vec<Diagnostic>> {
    if max_bytes < MIN_OUTPUT_BYTES {
        return Err(vec![target_error(
            "Workspace Semantic Analysis max_bytes must be at least 4096",
        )]);
    }
    if max_bytes > MAX_OUTPUT_BYTES {
        return Err(vec![limit_error("output_bytes", MAX_OUTPUT_BYTES)]);
    }
    Ok(())
}

fn validate_public_options(
    artifact: WorkspaceAnalysisArtifactKind,
    depth: usize,
    max_bytes: usize,
    max_nodes: usize,
) -> Result<(), Diagnostic> {
    if depth > MAX_TRAVERSAL_DEPTH {
        return Err(artifact_limit_error(
            artifact,
            "traversal_depth",
            MAX_TRAVERSAL_DEPTH,
        ));
    }
    if max_bytes < MIN_OUTPUT_BYTES {
        return Err(artifact_option_error(artifact, "max_bytes"));
    }
    if max_bytes > MAX_OUTPUT_BYTES {
        return Err(artifact_limit_error(
            artifact,
            "output_bytes",
            MAX_OUTPUT_BYTES,
        ));
    }
    if max_nodes == 0 {
        return Err(artifact_option_error(artifact, "max_nodes"));
    }
    if max_nodes > MAX_TRAVERSAL_NODES {
        return Err(artifact_limit_error(
            artifact,
            "traversal_nodes",
            MAX_TRAVERSAL_NODES,
        ));
    }
    Ok(())
}

fn artifact_digest(domain: &[u8], payload: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(domain);
    hasher.update((payload.len() as u64).to_le_bytes());
    hasher.update(payload);
    format!("sha256:{:x}", hasher.finalize())
}

fn bind_artifact(
    output_limit: usize,
    domain: &[u8],
    mismatch: &'static str,
    mut render: impl FnMut(Option<&str>, usize) -> String,
) -> Result<(String, String), Vec<Diagnostic>> {
    let bounded = |digest,
                   used_output_bytes,
                   render: &mut dyn FnMut(Option<&str>, usize) -> String| {
        let (json, overflowed) =
            crate::bounded_output::with_limit(output_limit, || render(digest, used_output_bytes));
        if overflowed || json.len() > output_limit {
            Err(vec![limit_error("output_bytes", output_limit)])
        } else {
            Ok(json)
        }
    };
    let mut used_output_bytes = 0usize;
    for _ in 0..20 {
        let placeholder = bounded(Some(DIGEST_PLACEHOLDER), used_output_bytes, &mut render)?;
        if placeholder.len() == used_output_bytes {
            let payload = bounded(None, used_output_bytes, &mut render)?;
            let digest = artifact_digest(domain, payload.as_bytes());
            let json = bounded(Some(&digest), used_output_bytes, &mut render)?;
            if json.len() != used_output_bytes {
                return Err(vec![invariant_error(mismatch)]);
            }
            return Ok((json, digest));
        }
        used_output_bytes = placeholder.len();
    }
    Err(vec![invariant_error(mismatch)])
}

fn push_json_string(output: &mut crate::bounded_output::CappedString, value: &str) {
    output.push('"');
    for character in value.chars() {
        match character {
            '"' => output.push_str("\\\""),
            '\\' => output.push_str("\\\\"),
            '\n' => output.push_str("\\n"),
            '\r' => output.push_str("\\r"),
            '\t' => output.push_str("\\t"),
            character if character.is_control() => {
                write!(output, "\\u{:04x}", character as u32)
                    .expect("writing to a string cannot fail");
            }
            character => output.push(character),
        }
    }
    output.push('"');
}

fn push_optional_json_string(
    output: &mut crate::bounded_output::CappedString,
    value: Option<&str>,
) {
    if let Some(value) = value {
        push_json_string(output, value);
    } else {
        output.push_str("null");
    }
}

fn push_string_array(output: &mut crate::bounded_output::CappedString, values: &[&str]) {
    output.push('[');
    for (index, value) in values.iter().enumerate() {
        if index > 0 {
            output.push(',');
        }
        push_json_string(output, value);
    }
    output.push(']');
}

fn direction_text(direction: WorkspaceAnalysisDirection) -> &'static str {
    match direction {
        WorkspaceAnalysisDirection::Forward => "forward",
        WorkspaceAnalysisDirection::Reverse => "reverse",
        WorkspaceAnalysisDirection::Both => "both",
    }
}

fn reached_by_text(reached_by: WorkspaceAnalysisReachedBy) -> &'static str {
    match reached_by {
        WorkspaceAnalysisReachedBy::Root => "root",
        WorkspaceAnalysisReachedBy::Forward => "forward",
        WorkspaceAnalysisReachedBy::Reverse => "reverse",
    }
}

fn declaration_kind_text(kind: DeclarationKind) -> &'static str {
    match kind {
        DeclarationKind::Resource => "resource",
        DeclarationKind::ResourceDrop => "resource_drop",
        DeclarationKind::Record => "record",
        DeclarationKind::Field => "field",
        DeclarationKind::Variant => "variant",
        DeclarationKind::VariantCase => "variant_case",
        DeclarationKind::CaseField => "case_field",
        DeclarationKind::Interface => "interface",
        DeclarationKind::Import => "import",
        DeclarationKind::Function => "function",
    }
}

struct WireNodeFields<'a> {
    kind: &'static str,
    id: &'a str,
    declaration_kind: Option<&'static str>,
    identity_origin: Option<&'static str>,
    path: Option<&'a str>,
    module: Option<&'a str>,
}

fn target_fields<'a>(
    analysis: &'a WorkspaceAnalysis,
    target: &'a WorkspaceAnalysisTarget,
) -> WireNodeFields<'a> {
    match target.kind() {
        WorkspaceAnalysisTargetKind::Declaration => {
            let fact = &analysis.declarations[target.value()];
            WireNodeFields {
                kind: "declaration",
                id: target.value(),
                declaration_kind: Some(declaration_kind_text(fact.kind)),
                identity_origin: Some(fact.origin.text()),
                path: fact.path.as_deref(),
                module: fact.module.as_deref(),
            }
        }
        WorkspaceAnalysisTargetKind::Capability => WireNodeFields {
            kind: "capability",
            id: target.value(),
            declaration_kind: None,
            identity_origin: None,
            path: None,
            module: None,
        },
    }
}

fn push_target(
    output: &mut crate::bounded_output::CappedString,
    analysis: &WorkspaceAnalysis,
    target: &WorkspaceAnalysisTarget,
) {
    let fields = target_fields(analysis, target);
    output.push_str("{\"kind\":");
    push_json_string(output, fields.kind);
    output.push_str(",\"id\":");
    push_json_string(output, fields.id);
    output.push_str(",\"declaration_kind\":");
    push_optional_json_string(output, fields.declaration_kind);
    output.push_str(",\"identity_origin\":");
    push_optional_json_string(output, fields.identity_origin);
    output.push_str(",\"path\":");
    push_optional_json_string(output, fields.path);
    output.push_str(",\"module\":");
    push_optional_json_string(output, fields.module);
    output.push('}');
}

fn node_wire_fields(fact: &WorkspaceAnalysisNodeFact) -> WireNodeFields<'_> {
    match &fact.node {
        WorkspaceAnalysisNode::Module { path, module } => WireNodeFields {
            kind: "module",
            id: module,
            declaration_kind: None,
            identity_origin: None,
            path: Some(path),
            module: Some(module),
        },
        WorkspaceAnalysisNode::Declaration(id) => WireNodeFields {
            kind: "declaration",
            id,
            declaration_kind: fact.declaration_kind.map(declaration_kind_text),
            identity_origin: fact.identity_origin.map(IdentityOrigin::text),
            path: fact.path.as_deref(),
            module: fact.module.as_deref(),
        },
        WorkspaceAnalysisNode::Capability(id) => WireNodeFields {
            kind: "capability",
            id,
            declaration_kind: None,
            identity_origin: None,
            path: None,
            module: None,
        },
    }
}

fn push_reached_by(
    output: &mut crate::bounded_output::CappedString,
    reached_by: &[WorkspaceAnalysisReachedBy],
) {
    output.push('[');
    for (index, value) in reached_by.iter().enumerate() {
        if index > 0 {
            output.push(',');
        }
        push_json_string(output, reached_by_text(*value));
    }
    output.push(']');
}

fn push_node(output: &mut crate::bounded_output::CappedString, fact: &WorkspaceAnalysisNodeFact) {
    output.push('{');
    push_node_members(output, fact);
    output.push('}');
}

fn push_node_members(
    output: &mut crate::bounded_output::CappedString,
    fact: &WorkspaceAnalysisNodeFact,
) {
    let fields = node_wire_fields(fact);
    output.push_str("\"kind\":");
    push_json_string(output, fields.kind);
    output.push_str(",\"declaration_kind\":");
    push_optional_json_string(output, fields.declaration_kind);
    output.push_str(",\"identity_origin\":");
    push_optional_json_string(output, fields.identity_origin);
    output.push_str(",\"id\":");
    push_json_string(output, fields.id);
    output.push_str(",\"path\":");
    push_optional_json_string(output, fields.path);
    output.push_str(",\"module\":");
    push_optional_json_string(output, fields.module);
    write!(output, ",\"minimum_depth\":{},\"reached_by\":", fact.depth)
        .expect("writing to a string cannot fail");
    push_reached_by(output, &fact.reached_by);
}

fn push_affected(
    output: &mut crate::bounded_output::CappedString,
    analysis: &WorkspaceAnalysis,
    facts: &WorkspaceImpactFacts,
    affected: &WorkspaceImpactNodeFact,
    retained_ranks: Option<&BTreeMap<WorkspaceAnalysisNode, usize>>,
    retained_nodes: usize,
) {
    let fields = node_wire_fields(&affected.node);
    output.push_str("{\"kind\":");
    push_json_string(output, fields.kind);
    output.push_str(",\"declaration_kind\":");
    push_optional_json_string(output, fields.declaration_kind);
    output.push_str(",\"identity_origin\":");
    push_optional_json_string(output, fields.identity_origin);
    output.push_str(",\"id\":");
    push_json_string(output, fields.id);
    output.push_str(",\"path\":");
    push_optional_json_string(output, fields.path);
    output.push_str(",\"module\":");
    push_optional_json_string(output, fields.module);
    write!(
        output,
        ",\"minimum_depth\":{},\"impact_role\":",
        affected.node.depth
    )
    .expect("writing to a string cannot fail");
    push_json_string(
        output,
        match affected.role {
            WorkspaceImpactRole::Root => "target",
            WorkspaceImpactRole::DeclarationConsumer => "consumer",
            WorkspaceImpactRole::ModuleConsumer => "module_consumer",
            WorkspaceImpactRole::CapabilityConsumer => "dependency",
        },
    );
    output.push_str(",\"reasons\":[");
    let mut reasons = [false; EDGE_FAMILIES.len()];
    if affected.role != WorkspaceImpactRole::Root {
        for path_edge in &facts.path_edges {
            let typed = &analysis.typed_edges[path_edge.edge_index];
            if typed.source == affected.node.node
                && edge_is_visible(typed, retained_ranks, retained_nodes)
            {
                let rank = EDGE_FAMILIES
                    .iter()
                    .position(|family| *family == typed.family)
                    .expect("typed edge family is from the closed workspace family set");
                reasons[rank] = true;
            }
        }
    }
    let mut first = true;
    for (rank, family) in EDGE_FAMILIES.iter().enumerate() {
        if reasons[rank] {
            if !first {
                output.push(',');
            }
            push_json_string(output, family.name());
            first = false;
        }
    }
    output.push_str("]}");
}

fn push_frontier_fact(
    output: &mut crate::bounded_output::CappedString,
    fact: &WorkspaceAnalysisNodeFact,
) {
    let fields = node_wire_fields(fact);
    output.push_str("{\"kind\":");
    push_json_string(output, fields.kind);
    output.push_str(",\"id\":");
    push_json_string(output, fields.id);
    write!(output, ",\"minimum_depth\":{},\"reached_by\":", fact.depth)
        .expect("writing to a string cannot fail");
    push_reached_by(output, &fact.reached_by);
    output.push('}');
}

fn edge_is_visible(
    edge: &WorkspaceAnalysisTypedEdge,
    retained_ranks: Option<&BTreeMap<WorkspaceAnalysisNode, usize>>,
    retained_nodes: usize,
) -> bool {
    retained_ranks.is_none_or(|ranks| {
        ranks
            .get(&edge.source)
            .is_some_and(|rank| *rank < retained_nodes)
            && ranks
                .get(&edge.target)
                .is_some_and(|rank| *rank < retained_nodes)
    })
}

fn push_edge(output: &mut crate::bounded_output::CappedString, edge: &WorkspaceEdge) {
    output.push_str("{\"caller_path\":");
    push_json_string(output, edge.caller_path());
    output.push_str(",\"caller\":");
    push_json_string(output, edge.caller());
    output.push_str(",\"target_path\":");
    push_json_string(output, edge.target_path());
    output.push_str(",\"target\":");
    push_json_string(output, edge.target());
    output.push_str(",\"kind\":");
    push_json_string(output, edge.kind());
    output.push_str(",\"site\":");
    push_json_string(output, edge.site());
    output.push_str(",\"expression\":");
    push_json_string(output, edge.expression());
    output.push_str(",\"ast_path\":");
    push_json_string(output, edge.ast_path());
    output.push_str(",\"alias\":");
    push_json_string(output, edge.alias());
    write!(output, ",\"ordinal\":{}}}", edge.ordinal()).expect("writing to a string cannot fail");
}

fn entry_path(analysis: &WorkspaceAnalysis) -> &str {
    analysis
        .modules()
        .iter()
        .find(|module| module.module() == analysis.entry_module())
        .expect("validated analysis retains entry module")
        .path()
}

fn push_entry(output: &mut crate::bounded_output::CappedString, analysis: &WorkspaceAnalysis) {
    output.push_str("{\"module\":");
    push_json_string(output, analysis.entry_module());
    output.push_str(",\"path\":");
    push_json_string(output, entry_path(analysis));
    output.push('}');
}

fn push_workspace_limits(output: &mut crate::bounded_output::CappedString) {
    output.push_str("{\"max_managed_files\":16,\"max_reachable_modules\":16,\"max_entry_module_bytes\":16777216,\"max_total_source_bytes\":16777216,\"max_declarations\":4096,\"max_callables\":1024,\"max_call_sites\":65536,\"max_uses\":4096,\"max_resolved_cross_file_edges\":65536,\"max_dependency_depth\":16,\"max_builder_bytes\":16777216,\"max_manifest_bytes\":1048576,\"max_output_bytes\":16777216,\"max_retained_generations\":32,\"max_staging_attempts\":32,\"max_unexpected_inventory_entries\":0}");
}

fn push_workspace_budget(
    output: &mut crate::bounded_output::CappedString,
    analysis: &WorkspaceAnalysis,
) {
    let usage = analysis.usage();
    write!(
        output,
        "{{\"used_managed_files\":{},\"used_reachable_modules\":{},\"used_entry_module_bytes\":{},\"used_total_source_bytes\":{},\"used_declarations\":{},\"used_callables\":{},\"used_call_sites\":{},\"used_uses\":{},\"used_resolved_cross_file_edges\":{},\"used_dependency_depth\":{},\"used_builder_bytes\":{},\"used_manifest_bytes\":{},\"used_output_bytes\":{},\"used_retained_generations\":{},\"used_staging_attempts\":{},\"used_unexpected_inventory_entries\":{}}}",
        usage.used_managed_files(),
        usage.used_reachable_modules(),
        usage.used_entry_module_bytes(),
        usage.used_total_source_bytes(),
        usage.used_declarations(),
        usage.used_callables(),
        usage.used_call_sites(),
        usage.used_uses(),
        usage.used_resolved_cross_file_edges(),
        usage.used_dependency_depth(),
        usage.used_builder_bytes(),
        usage.used_manifest_bytes(),
        analysis.workspace_graph_output_bytes,
        usage.used_retained_generations(),
        usage.used_staging_attempts(),
        usage.used_unexpected_inventory_entries(),
    )
    .expect("writing to a string cannot fail");
}

fn push_edge_kinds(output: &mut crate::bounded_output::CappedString) {
    output.push('[');
    for (index, family) in EDGE_FAMILIES.iter().enumerate() {
        if index > 0 {
            output.push(',');
        }
        push_json_string(output, family.name());
    }
    output.push(']');
}

fn truncation_reasons(
    truncation: &WorkspaceAnalysisTruncationFacts,
    query_depth: usize,
) -> Vec<&'static str> {
    let mut reasons = Vec::new();
    if truncation
        .frontier
        .first()
        .is_some_and(|fact| fact.depth > query_depth)
    {
        reasons.push("max_depth");
    }
    if truncation.deferred_known_nodes > 0
        || (truncation.omitted_known_nodes > 0
            && truncation
                .frontier
                .first()
                .is_some_and(|fact| fact.depth <= query_depth))
    {
        reasons.push("max_nodes");
    }
    if truncation.byte_truncated {
        reasons.push("max_bytes");
    }
    reasons
}

fn is_truncated(truncation: &WorkspaceAnalysisTruncationFacts) -> bool {
    truncation.omitted_known_nodes > 0
        || truncation.deferred_known_nodes > 0
        || truncation.byte_truncated
}

fn push_prefix_truncation(
    output: &mut crate::bounded_output::CappedString,
    truncation: &WorkspaceAnalysisTruncationFacts,
    query_depth: usize,
    removed_nodes: usize,
) {
    let original_reasons = truncation_reasons(truncation, query_depth);
    let max_depth = original_reasons.contains(&"max_depth");
    let max_nodes = original_reasons.contains(&"max_nodes");
    let max_bytes = truncation.byte_truncated || removed_nodes > 0;
    output.push_str("{\"truncated\":");
    output.push_str(if max_depth || max_nodes || max_bytes {
        "true"
    } else {
        "false"
    });
    output.push_str(",\"reasons\":[");
    let mut first = true;
    for (present, reason) in [
        (max_depth, "max_depth"),
        (max_nodes, "max_nodes"),
        (max_bytes, "max_bytes"),
    ] {
        if present {
            if !first {
                output.push(',');
            }
            push_json_string(output, reason);
            first = false;
        }
    }
    let omitted = truncation
        .omitted_known_nodes
        .saturating_add(removed_nodes);
    write!(
        output,
        "],\"omitted_known_nodes\":{omitted},\"deferred_known_nodes\":{}}}",
        truncation.deferred_known_nodes,
    )
    .expect("writing to a string cannot fail");
}

fn push_context_prefix_frontier(
    output: &mut crate::bounded_output::CappedString,
    facts: &WorkspaceContextFacts,
    retained_nodes: usize,
) {
    output.push('[');
    let removed_depth = facts.nodes.get(retained_nodes).map(|fact| fact.depth);
    let mut first = true;
    if let Some(depth) = removed_depth {
        let mut removed = facts.nodes[retained_nodes..]
            .iter()
            .take_while(|fact| fact.depth == depth)
            .peekable();
        let mut prior = facts
            .truncation
            .frontier
            .iter()
            .filter(|fact| fact.depth == depth)
            .peekable();
        while removed.peek().is_some() || prior.peek().is_some() {
            let fact = match (removed.peek(), prior.peek()) {
                (Some(left), Some(right)) if compare_node_facts(left, right).is_le() => {
                    removed.next().expect("peeked removed frontier fact")
                }
                (Some(_), Some(_)) => prior.next().expect("peeked prior frontier fact"),
                (Some(_), None) => removed.next().expect("peeked removed frontier fact"),
                (None, Some(_)) => prior.next().expect("peeked prior frontier fact"),
                (None, None) => unreachable!(),
            };
            if !first {
                output.push(',');
            }
            push_frontier_fact(output, fact);
            first = false;
        }
    } else {
        for fact in &facts.truncation.frontier {
            if !first {
                output.push(',');
            }
            push_frontier_fact(output, fact);
            first = false;
        }
    }
    output.push(']');
}

fn push_impact_prefix_frontier(
    output: &mut crate::bounded_output::CappedString,
    facts: &WorkspaceImpactFacts,
    retained_nodes: usize,
) {
    output.push('[');
    let removed_depth = facts.nodes.get(retained_nodes).map(|fact| fact.node.depth);
    let mut first = true;
    if let Some(depth) = removed_depth {
        let mut removed = facts.nodes[retained_nodes..]
            .iter()
            .map(|fact| &fact.node)
            .take_while(|fact| fact.depth == depth)
            .peekable();
        let mut prior = facts
            .truncation
            .frontier
            .iter()
            .filter(|fact| fact.depth == depth)
            .peekable();
        while removed.peek().is_some() || prior.peek().is_some() {
            let fact = match (removed.peek(), prior.peek()) {
                (Some(left), Some(right)) if compare_node_facts(left, right).is_le() => {
                    removed.next().expect("peeked removed frontier fact")
                }
                (Some(_), Some(_)) => prior.next().expect("peeked prior frontier fact"),
                (Some(_), None) => removed.next().expect("peeked removed frontier fact"),
                (None, Some(_)) => prior.next().expect("peeked prior frontier fact"),
                (None, None) => unreachable!(),
            };
            if !first {
                output.push(',');
            }
            push_frontier_fact(output, fact);
            first = false;
        }
    } else {
        for fact in &facts.truncation.frontier {
            if !first {
                output.push(',');
            }
            push_frontier_fact(output, fact);
            first = false;
        }
    }
    output.push(']');
}

fn used_depth(nodes: &[WorkspaceAnalysisNodeFact]) -> usize {
    nodes.iter().map(|node| node.depth).max().unwrap_or(0)
}

fn push_common_limits(output: &mut crate::bounded_output::CappedString, review: bool) {
    output.push_str("{\"workspace\":");
    push_workspace_limits(output);
    output.push_str(",\"analysis\":{\"max_target_bytes\":4096,\"max_traversal_depth\":1024,\"max_traversal_nodes\":8208,\"max_analysis_builder_bytes\":16777216");
    if review {
        output.push_str(",\"max_context_bytes\":16777216,\"max_impact_bytes\":16777216,\"max_output_bytes\":33554432}}");
    } else {
        output.push_str(",\"max_output_bytes\":16777216}}");
    }
}

struct AnalysisBudgetFields {
    target_bytes: usize,
    depth: usize,
    nodes: usize,
    builder_bytes: usize,
    context_bytes: Option<usize>,
    impact_bytes: Option<usize>,
    output_bytes: usize,
}

fn push_analysis_budget(
    output: &mut crate::bounded_output::CappedString,
    analysis: &WorkspaceAnalysis,
    fields: AnalysisBudgetFields,
) {
    output.push_str("{\"workspace\":");
    push_workspace_budget(output, analysis);
    output.push_str(",\"analysis\":{");
    write!(
        output,
        "\"used_target_bytes\":{},\"used_traversal_depth\":{},\"used_traversal_nodes\":{},\"used_analysis_builder_bytes\":{}",
        fields.target_bytes,
        fields.depth,
        fields.nodes,
        fields.builder_bytes,
    )
    .expect("writing to a string cannot fail");
    if let (Some(context_bytes), Some(impact_bytes)) = (fields.context_bytes, fields.impact_bytes) {
        write!(
            output,
            ",\"used_context_bytes\":{context_bytes},\"used_impact_bytes\":{impact_bytes}"
        )
        .expect("writing to a string cannot fail");
    }
    write!(output, ",\"used_output_bytes\":{}}}}}", fields.output_bytes)
        .expect("writing to a string cannot fail");
}

fn push_nonclaims(output: &mut crate::bounded_output::CappedString, nonclaims: &[&str]) {
    push_string_array(output, nonclaims);
}

fn render_context_json(
    analysis: &WorkspaceAnalysis,
    facts: &WorkspaceContextFacts,
    query_depth: usize,
    query_max_bytes: usize,
    query_max_nodes: usize,
    digest: Option<&str>,
    used_output_bytes: usize,
) -> String {
    render_context_json_prefix(
        analysis,
        facts,
        query_depth,
        query_max_bytes,
        query_max_nodes,
        digest,
        used_output_bytes,
        facts.nodes.len(),
        None,
        facts.used_builder_bytes,
    )
}

#[allow(
    clippy::too_many_arguments,
    reason = "closed canonical Context renderer binding"
)]
fn render_context_json_prefix(
    analysis: &WorkspaceAnalysis,
    facts: &WorkspaceContextFacts,
    query_depth: usize,
    query_max_bytes: usize,
    query_max_nodes: usize,
    digest: Option<&str>,
    used_output_bytes: usize,
    retained_nodes: usize,
    retained_ranks: Option<&BTreeMap<WorkspaceAnalysisNode, usize>>,
    used_builder_bytes: usize,
) -> String {
    let mut output = crate::bounded_output::CappedString::new();
    output.push_str("{\"schema\":");
    push_json_string(&mut output, CONTEXT_SCHEMA);
    output.push_str(",\"workspace_manifest_schema\":");
    push_json_string(&mut output, WORKSPACE_MANIFEST_SCHEMA);
    output.push_str(",\"workspace_revision\":");
    push_json_string(&mut output, analysis.workspace_revision());
    output.push_str(",\"workspace_graph_digest\":");
    push_json_string(&mut output, &analysis.workspace_graph_digest);
    if let Some(digest) = digest {
        output.push_str(",\"artifact_digest\":");
        push_json_string(&mut output, digest);
    }
    output.push_str(",\"entry\":");
    push_entry(&mut output, analysis);
    output.push_str(",\"target\":");
    push_target(&mut output, analysis, &facts.target);
    output.push_str(",\"query\":{\"direction\":");
    push_json_string(&mut output, direction_text(facts.direction));
    write!(
        output,
        ",\"depth\":{query_depth},\"max_bytes\":{query_max_bytes},\"max_nodes\":{query_max_nodes},\"edge_kinds\":"
    )
    .expect("writing to a string cannot fail");
    push_edge_kinds(&mut output);
    output.push_str("},\"limits\":");
    push_common_limits(&mut output, false);
    output.push_str(",\"budget\":");
    push_analysis_budget(
        &mut output,
        analysis,
        AnalysisBudgetFields {
            target_bytes: facts.target.value().len(),
            depth: used_depth(&facts.nodes[..retained_nodes]),
            nodes: retained_nodes,
            builder_bytes: used_builder_bytes,
            context_bytes: None,
            impact_bytes: None,
            output_bytes: used_output_bytes,
        },
    );
    output.push_str(",\"truncation\":");
    push_prefix_truncation(
        &mut output,
        &facts.truncation,
        query_depth,
        facts.nodes.len() - retained_nodes,
    );
    output.push_str(",\"frontier\":");
    push_context_prefix_frontier(&mut output, facts, retained_nodes);
    output.push_str(",\"nodes\":[");
    for (index, node) in facts.nodes[..retained_nodes].iter().enumerate() {
        if index > 0 {
            output.push(',');
        }
        push_node(&mut output, node);
    }
    output.push_str("],\"edges\":[");
    let mut first_edge = true;
    for path_edge in &facts.path_edges {
        let typed = &analysis.typed_edges[path_edge.edge_index];
        if !edge_is_visible(typed, retained_ranks, retained_nodes) {
            continue;
        }
        if !first_edge {
            output.push(',');
        }
        push_edge(&mut output, &analysis.edges()[path_edge.edge_index]);
        first_edge = false;
    }
    output.push_str("],\"nonclaims\":");
    push_nonclaims(&mut output, &CONTEXT_NONCLAIMS);
    output.push('}');
    output.into_string()
}

fn render_impact_json(
    analysis: &WorkspaceAnalysis,
    facts: &WorkspaceImpactFacts,
    query_depth: usize,
    query_max_bytes: usize,
    query_max_nodes: usize,
    digest: Option<&str>,
    used_output_bytes: usize,
) -> String {
    render_impact_json_prefix(
        analysis,
        facts,
        query_depth,
        query_max_bytes,
        query_max_nodes,
        digest,
        used_output_bytes,
        facts.nodes.len(),
        None,
        facts.used_builder_bytes,
    )
}

#[allow(
    clippy::too_many_arguments,
    reason = "closed canonical Impact renderer binding"
)]
fn render_impact_json_prefix(
    analysis: &WorkspaceAnalysis,
    facts: &WorkspaceImpactFacts,
    query_depth: usize,
    query_max_bytes: usize,
    query_max_nodes: usize,
    digest: Option<&str>,
    used_output_bytes: usize,
    retained_nodes: usize,
    retained_ranks: Option<&BTreeMap<WorkspaceAnalysisNode, usize>>,
    used_builder_bytes: usize,
) -> String {
    let mut output = crate::bounded_output::CappedString::new();
    output.push_str("{\"schema\":");
    push_json_string(&mut output, IMPACT_SCHEMA);
    output.push_str(",\"workspace_manifest_schema\":");
    push_json_string(&mut output, WORKSPACE_MANIFEST_SCHEMA);
    output.push_str(",\"workspace_revision\":");
    push_json_string(&mut output, analysis.workspace_revision());
    output.push_str(",\"workspace_graph_digest\":");
    push_json_string(&mut output, &analysis.workspace_graph_digest);
    if let Some(digest) = digest {
        output.push_str(",\"artifact_digest\":");
        push_json_string(&mut output, digest);
    }
    output.push_str(",\"entry\":");
    push_entry(&mut output, analysis);
    output.push_str(",\"target\":");
    push_target(&mut output, analysis, &facts.target);
    write!(
        output,
        ",\"query\":{{\"direction\":\"reverse\",\"depth\":{query_depth},\"max_bytes\":{query_max_bytes},\"max_nodes\":{query_max_nodes},\"edge_kinds\":"
    )
    .expect("writing to a string cannot fail");
    push_edge_kinds(&mut output);
    output.push_str("},\"limits\":");
    push_common_limits(&mut output, false);
    output.push_str(",\"budget\":");
    push_analysis_budget(
        &mut output,
        analysis,
        AnalysisBudgetFields {
            target_bytes: facts.target.value().len(),
            depth: facts
                .nodes
                .get(..retained_nodes)
                .unwrap_or_default()
                .iter()
                .map(|affected| affected.node.depth)
                .max()
                .unwrap_or(0),
            nodes: retained_nodes,
            builder_bytes: used_builder_bytes,
            context_bytes: None,
            impact_bytes: None,
            output_bytes: used_output_bytes,
        },
    );
    output.push_str(",\"truncation\":");
    push_prefix_truncation(
        &mut output,
        &facts.truncation,
        query_depth,
        facts.nodes.len() - retained_nodes,
    );
    output.push_str(",\"frontier\":");
    push_impact_prefix_frontier(&mut output, facts, retained_nodes);
    output.push_str(",\"affected\":[");
    for (index, affected) in facts.nodes[..retained_nodes].iter().enumerate() {
        if index > 0 {
            output.push(',');
        }
        push_affected(
            &mut output,
            analysis,
            facts,
            affected,
            retained_ranks,
            retained_nodes,
        );
    }
    output.push_str("],\"dependency_edges\":[");
    let mut first_edge = true;
    for path_edge in &facts.path_edges {
        let typed = &analysis.typed_edges[path_edge.edge_index];
        if !edge_is_visible(typed, retained_ranks, retained_nodes) {
            continue;
        }
        if !first_edge {
            output.push(',');
        }
        push_edge(&mut output, &analysis.edges()[path_edge.edge_index]);
        first_edge = false;
    }
    output.push_str("],\"nonclaims\":");
    push_nonclaims(&mut output, &IMPACT_NONCLAIMS);
    output.push('}');
    output.into_string()
}

fn render_context_artifact(
    analysis: &WorkspaceAnalysis,
    mut facts: WorkspaceContextFacts,
    query_depth: usize,
    query_max_bytes: usize,
    query_max_nodes: usize,
    output_limit: usize,
) -> Result<WorkspaceContextArtifact, Vec<Diagnostic>> {
    let remaining_builder_bytes = MAX_BUILDER_BYTES
        .checked_sub(facts.aggregate_builder_bytes)
        .ok_or_else(|| vec![limit_error("builder_bytes", MAX_BUILDER_BYTES)])?;
    let (ranks, rank_builder_bytes) = retained_node_ranks(
        facts.nodes.iter().map(|fact| &fact.node),
        remaining_builder_bytes,
    )?;
    facts.used_builder_bytes = checked_builder_sum(facts.used_builder_bytes, rank_builder_bytes)?;
    facts.aggregate_builder_bytes =
        checked_builder_sum(facts.aggregate_builder_bytes, rank_builder_bytes)?;
    let prefix_builder_bytes = facts.used_builder_bytes;
    let prefix_aggregate_builder_bytes = facts.aggregate_builder_bytes;
    let mut low = 1usize;
    let mut high = facts.nodes.len();
    let mut retained_nodes = None;
    while low <= high {
        let raw_candidate = low + (high - low) / 2;
        let candidate = next_context_builder_feasible_prefix(
            &facts,
            raw_candidate,
            prefix_builder_bytes,
            prefix_aggregate_builder_bytes,
        )
        .ok_or_else(|| vec![limit_error("builder_bytes", MAX_BUILDER_BYTES)])?;
        if candidate > high {
            if raw_candidate == 1 {
                break;
            }
            high = raw_candidate - 1;
            continue;
        }
        let candidate_builder_bytes = checked_builder_sum(
            prefix_builder_bytes,
            context_finalization_debit(&facts, candidate)?,
        )?;
        let measured = measure_artifact(output_limit, |used_output_bytes| {
            render_context_json_prefix(
                analysis,
                &facts,
                query_depth,
                query_max_bytes,
                query_max_nodes,
                Some(DIGEST_PLACEHOLDER),
                used_output_bytes,
                candidate,
                Some(&ranks),
                candidate_builder_bytes,
            )
        })?;
        if measured.is_some() {
            retained_nodes = Some(candidate);
            low = candidate.saturating_add(1);
        } else if candidate == 1 {
            break;
        } else {
            high = candidate - 1;
        }
    }
    let retained_nodes =
        retained_nodes.ok_or_else(|| vec![limit_error("output_bytes", output_limit)])?;
    let finalization_debit = context_finalization_debit(&facts, retained_nodes)?;
    facts.used_builder_bytes = checked_builder_sum(prefix_builder_bytes, finalization_debit)?;
    facts.aggregate_builder_bytes =
        checked_builder_sum(prefix_aggregate_builder_bytes, finalization_debit)?;
    let final_builder_bytes = finalize_context_prefix(
        analysis,
        &mut facts,
        retained_nodes,
        &ranks,
        finalization_debit,
    )?;
    if final_builder_bytes != finalization_debit {
        return Err(vec![invariant_error(
            "Workspace Semantic Context replay or digest binding disagrees",
        )]);
    }
    let (json, digest) = bind_artifact(
        output_limit,
        CONTEXT_DIGEST_DOMAIN,
        "Workspace Semantic Context replay or digest binding disagrees",
        |digest, used_output_bytes| {
            render_context_json(
                analysis,
                &facts,
                query_depth,
                query_max_bytes,
                query_max_nodes,
                digest,
                used_output_bytes,
            )
        },
    )?;
    Ok(WorkspaceContextArtifact {
        json,
        digest,
        facts,
    })
}

fn render_impact_artifact(
    analysis: &WorkspaceAnalysis,
    mut facts: WorkspaceImpactFacts,
    query_depth: usize,
    query_max_bytes: usize,
    query_max_nodes: usize,
    output_limit: usize,
) -> Result<WorkspaceImpactArtifact, Vec<Diagnostic>> {
    let remaining_builder_bytes = MAX_BUILDER_BYTES
        .checked_sub(facts.aggregate_builder_bytes)
        .ok_or_else(|| vec![limit_error("builder_bytes", MAX_BUILDER_BYTES)])?;
    let (ranks, rank_builder_bytes) = retained_node_ranks(
        facts.nodes.iter().map(|fact| &fact.node.node),
        remaining_builder_bytes,
    )?;
    facts.used_builder_bytes = checked_builder_sum(facts.used_builder_bytes, rank_builder_bytes)?;
    facts.aggregate_builder_bytes =
        checked_builder_sum(facts.aggregate_builder_bytes, rank_builder_bytes)?;
    let prefix_builder_bytes = facts.used_builder_bytes;
    let prefix_aggregate_builder_bytes = facts.aggregate_builder_bytes;
    let mut low = 1usize;
    let mut high = facts.nodes.len();
    let mut retained_nodes = None;
    while low <= high {
        let raw_candidate = low + (high - low) / 2;
        let candidate = next_impact_builder_feasible_prefix(
            &facts,
            raw_candidate,
            prefix_builder_bytes,
            prefix_aggregate_builder_bytes,
        )
        .ok_or_else(|| vec![limit_error("builder_bytes", MAX_BUILDER_BYTES)])?;
        if candidate > high {
            if raw_candidate == 1 {
                break;
            }
            high = raw_candidate - 1;
            continue;
        }
        let candidate_builder_bytes = checked_builder_sum(
            prefix_builder_bytes,
            impact_finalization_debit(&facts, candidate)?,
        )?;
        let measured = measure_artifact(output_limit, |used_output_bytes| {
            render_impact_json_prefix(
                analysis,
                &facts,
                query_depth,
                query_max_bytes,
                query_max_nodes,
                Some(DIGEST_PLACEHOLDER),
                used_output_bytes,
                candidate,
                Some(&ranks),
                candidate_builder_bytes,
            )
        })?;
        if measured.is_some() {
            retained_nodes = Some(candidate);
            low = candidate.saturating_add(1);
        } else if candidate == 1 {
            break;
        } else {
            high = candidate - 1;
        }
    }
    let retained_nodes =
        retained_nodes.ok_or_else(|| vec![limit_error("output_bytes", output_limit)])?;
    let finalization_debit = impact_finalization_debit(&facts, retained_nodes)?;
    facts.used_builder_bytes = checked_builder_sum(prefix_builder_bytes, finalization_debit)?;
    facts.aggregate_builder_bytes =
        checked_builder_sum(prefix_aggregate_builder_bytes, finalization_debit)?;
    let final_builder_bytes = finalize_impact_prefix(
        analysis,
        &mut facts,
        retained_nodes,
        &ranks,
        finalization_debit,
    )?;
    if final_builder_bytes != finalization_debit {
        return Err(vec![invariant_error(
            "Workspace Semantic Impact replay or digest binding disagrees",
        )]);
    }
    let (json, digest) = bind_artifact(
        output_limit,
        IMPACT_DIGEST_DOMAIN,
        "Workspace Semantic Impact replay or digest binding disagrees",
        |digest, used_output_bytes| {
            render_impact_json(
                analysis,
                &facts,
                query_depth,
                query_max_bytes,
                query_max_nodes,
                digest,
                used_output_bytes,
            )
        },
    )?;
    Ok(WorkspaceImpactArtifact {
        json,
        digest,
        facts,
    })
}

fn measure_artifact(
    output_limit: usize,
    mut render: impl FnMut(usize) -> String,
) -> Result<Option<usize>, Vec<Diagnostic>> {
    let mut used_output_bytes = 0usize;
    for _ in 0..20 {
        let (json, overflowed) =
            crate::bounded_output::with_limit(output_limit, || render(used_output_bytes));
        if overflowed || json.len() > output_limit {
            return Ok(None);
        }
        if json.len() == used_output_bytes {
            return Ok(Some(used_output_bytes));
        }
        used_output_bytes = json.len();
    }
    Err(vec![invariant_error(
        "workspace analysis output length fixed point disagrees",
    )])
}

fn retained_node_ranks<'a>(
    nodes: impl Iterator<Item = &'a WorkspaceAnalysisNode>,
    remaining_builder_bytes: usize,
) -> Result<(BTreeMap<WorkspaceAnalysisNode, usize>, usize), Vec<Diagnostic>> {
    let (result, overflowed, consumed) =
        crate::bounded_output::with_limit_usage(remaining_builder_bytes, || {
            let mut ranks = BTreeMap::new();
            for (rank, node) in nodes.enumerate() {
                reserve_entry::<(WorkspaceAnalysisNode, usize)>()?;
                if ranks.insert(clone_node(node), rank).is_some() {
                    return Err(vec![invariant_error(
                        "workspace analysis byte-prefix node index disagrees",
                    )]);
                }
            }
            Ok(ranks)
        });
    if overflowed {
        return Err(vec![limit_error("builder_bytes", MAX_BUILDER_BYTES)]);
    }
    result.map(|ranks| (ranks, consumed))
}

fn context_finalization_debit(
    facts: &WorkspaceContextFacts,
    retained_nodes: usize,
) -> Result<usize, Vec<Diagnostic>> {
    let Some(first) = facts.nodes.get(retained_nodes) else {
        return Ok(0);
    };
    facts
        .nodes
        .partition_point(|fact| fact.depth <= first.depth)
        .checked_sub(retained_nodes)
        .ok_or_else(|| {
            vec![invariant_error(
                "workspace analysis Context byte-prefix depth accounting disagrees",
            )]
        })?
        .checked_mul(std::mem::size_of::<WorkspaceAnalysisNodeFact>())
        .ok_or_else(|| vec![limit_error("builder_bytes", MAX_BUILDER_BYTES)])
}

fn impact_finalization_debit(
    facts: &WorkspaceImpactFacts,
    retained_nodes: usize,
) -> Result<usize, Vec<Diagnostic>> {
    let Some(first) = facts.nodes.get(retained_nodes) else {
        return Ok(0);
    };
    facts
        .nodes
        .partition_point(|fact| fact.node.depth <= first.node.depth)
        .checked_sub(retained_nodes)
        .ok_or_else(|| {
            vec![invariant_error(
                "workspace analysis Impact byte-prefix depth accounting disagrees",
            )]
        })?
        .checked_mul(std::mem::size_of::<WorkspaceAnalysisNodeFact>())
        .ok_or_else(|| vec![limit_error("builder_bytes", MAX_BUILDER_BYTES)])
}

fn next_context_builder_feasible_prefix(
    facts: &WorkspaceContextFacts,
    candidate: usize,
    local_builder_bytes: usize,
    aggregate_builder_bytes: usize,
) -> Option<usize> {
    let remaining = MAX_BUILDER_BYTES
        .checked_sub(local_builder_bytes)?
        .min(MAX_BUILDER_BYTES.checked_sub(aggregate_builder_bytes)?);
    let affordable_frontier = remaining / std::mem::size_of::<WorkspaceAnalysisNodeFact>();
    let mut retained = candidate;
    while retained < facts.nodes.len() {
        let depth = facts.nodes[retained].depth;
        let group_end = facts.nodes.partition_point(|fact| fact.depth <= depth);
        let threshold = group_end.saturating_sub(affordable_frontier);
        if threshold < group_end {
            return Some(retained.max(threshold));
        }
        retained = group_end;
    }
    Some(facts.nodes.len())
}

fn next_impact_builder_feasible_prefix(
    facts: &WorkspaceImpactFacts,
    candidate: usize,
    local_builder_bytes: usize,
    aggregate_builder_bytes: usize,
) -> Option<usize> {
    let remaining = MAX_BUILDER_BYTES
        .checked_sub(local_builder_bytes)?
        .min(MAX_BUILDER_BYTES.checked_sub(aggregate_builder_bytes)?);
    let affordable_frontier = remaining / std::mem::size_of::<WorkspaceAnalysisNodeFact>();
    let mut retained = candidate;
    while retained < facts.nodes.len() {
        let depth = facts.nodes[retained].node.depth;
        let group_end = facts.nodes.partition_point(|fact| fact.node.depth <= depth);
        let threshold = group_end.saturating_sub(affordable_frontier);
        if threshold < group_end {
            return Some(retained.max(threshold));
        }
        retained = group_end;
    }
    Some(facts.nodes.len())
}

fn finalize_context_prefix(
    analysis: &WorkspaceAnalysis,
    facts: &mut WorkspaceContextFacts,
    retained_nodes: usize,
    ranks: &BTreeMap<WorkspaceAnalysisNode, usize>,
    finalization_limit: usize,
) -> Result<usize, Vec<Diagnostic>> {
    if retained_nodes == facts.nodes.len() {
        return Ok(0);
    }
    facts.path_edges.retain(|path_edge| {
        edge_is_visible(
            &analysis.typed_edges[path_edge.edge_index],
            Some(ranks),
            retained_nodes,
        )
    });
    let removed_count = facts.nodes.len() - retained_nodes;
    let removed_depth = facts.nodes[retained_nodes].depth;
    let (result, overflowed, consumed) =
        crate::bounded_output::with_limit_usage(finalization_limit, || {
            facts
                .truncation
                .frontier
                .retain(|fact| fact.depth == removed_depth);
            for fact in facts.nodes.drain(retained_nodes..) {
                if fact.depth == removed_depth {
                    reserve_entry::<WorkspaceAnalysisNodeFact>()?;
                    facts.truncation.frontier.push(fact);
                }
            }
            facts.truncation.frontier.sort_by(compare_node_facts);
            facts.truncation.omitted_known_nodes = facts
                .truncation
                .omitted_known_nodes
                .checked_add(removed_count)
                .ok_or_else(|| {
                    vec![invariant_error(
                        "workspace analysis Context omitted-node accounting disagrees",
                    )]
                })?;
            facts.truncation.byte_truncated = true;
            Ok(())
        });
    if overflowed {
        return Err(vec![limit_error("builder_bytes", MAX_BUILDER_BYTES)]);
    }
    result.map(|()| consumed)
}

fn finalize_impact_prefix(
    analysis: &WorkspaceAnalysis,
    facts: &mut WorkspaceImpactFacts,
    retained_nodes: usize,
    ranks: &BTreeMap<WorkspaceAnalysisNode, usize>,
    finalization_limit: usize,
) -> Result<usize, Vec<Diagnostic>> {
    if retained_nodes == facts.nodes.len() {
        return Ok(0);
    }
    facts.path_edges.retain(|path_edge| {
        edge_is_visible(
            &analysis.typed_edges[path_edge.edge_index],
            Some(ranks),
            retained_nodes,
        )
    });
    let removed_count = facts.nodes.len() - retained_nodes;
    let removed_depth = facts.nodes[retained_nodes].node.depth;
    let (result, overflowed, consumed) =
        crate::bounded_output::with_limit_usage(finalization_limit, || {
            facts
                .truncation
                .frontier
                .retain(|fact| fact.depth == removed_depth);
            for fact in facts.nodes.drain(retained_nodes..) {
                if fact.node.depth == removed_depth {
                    reserve_entry::<WorkspaceAnalysisNodeFact>()?;
                    facts.truncation.frontier.push(fact.node);
                }
            }
            facts.truncation.frontier.sort_by(compare_node_facts);
            facts.truncation.omitted_known_nodes = facts
                .truncation
                .omitted_known_nodes
                .checked_add(removed_count)
                .ok_or_else(|| {
                    vec![invariant_error(
                        "workspace analysis Impact omitted-node accounting disagrees",
                    )]
                })?;
            facts.truncation.byte_truncated = true;
            Ok(())
        });
    if overflowed {
        return Err(vec![limit_error("builder_bytes", MAX_BUILDER_BYTES)]);
    }
    result.map(|()| consumed)
}

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
struct ReviewEvidenceRef {
    artifact: &'static str,
    relation: &'static str,
    index: usize,
}

fn render_review_artifact(
    analysis: &WorkspaceAnalysis,
    context: WorkspaceContextArtifact,
    impact: WorkspaceImpactArtifact,
    output_limit: usize,
) -> Result<WorkspaceReviewArtifact, Vec<Diagnostic>> {
    let (json, digest) = bind_artifact(
        output_limit,
        REVIEW_DIGEST_DOMAIN,
        "Workspace Semantic Review replay or digest binding disagrees",
        |digest, used_output_bytes| {
            render_review_json(analysis, &context, &impact, digest, used_output_bytes)
        },
    )?;
    Ok(WorkspaceReviewArtifact { json, digest })
}

fn mandatory_review_reservation(
    analysis: &WorkspaceAnalysis,
    target: &WorkspaceAnalysisTarget,
    output_limit: usize,
) -> Result<usize, Vec<Diagnostic>> {
    let mut used_output_bytes = 0usize;
    for _ in 0..20 {
        let (envelope, overflowed) = crate::bounded_output::with_limit(output_limit, || {
            render_review_envelope(analysis, target, used_output_bytes)
        });
        if overflowed || envelope.len() > output_limit {
            return Err(vec![limit_error("output_bytes", output_limit)]);
        }
        let evidence = maximum_review_evidence_bytes(analysis.edges().len())?;
        let reserved = envelope.len().checked_add(evidence).ok_or_else(|| {
            vec![invariant_error(
                "Workspace Semantic Review replay or digest binding disagrees",
            )]
        })?;
        if reserved == used_output_bytes {
            return Ok(reserved);
        }
        used_output_bytes = reserved;
    }
    Err(vec![invariant_error(
        "Workspace Semantic Review replay or digest binding disagrees",
    )])
}

fn render_review_envelope(
    analysis: &WorkspaceAnalysis,
    target: &WorkspaceAnalysisTarget,
    used_output_bytes: usize,
) -> String {
    let mut output = crate::bounded_output::CappedString::new();
    output.push_str("{\"schema\":");
    push_json_string(&mut output, REVIEW_SCHEMA);
    output.push_str(",\"workspace_manifest_schema\":");
    push_json_string(&mut output, WORKSPACE_MANIFEST_SCHEMA);
    output.push_str(",\"workspace_revision\":");
    push_json_string(&mut output, analysis.workspace_revision());
    output.push_str(",\"workspace_graph_digest\":");
    push_json_string(&mut output, &analysis.workspace_graph_digest);
    output.push_str(",\"artifact_digest\":");
    push_json_string(&mut output, DIGEST_PLACEHOLDER);
    output.push_str(",\"entry\":");
    push_entry(&mut output, analysis);
    output.push_str(",\"target\":");
    push_target(&mut output, analysis, target);
    // Child bytes are deliberately absent. Their exact final lengths are
    // debited from the remaining review budget before each child build.
    output.push_str(",\"context\":,\"impact\":,\"sections\":{");
    push_reserved_review_sections(&mut output);
    output.push_str("},\"limits\":");
    push_common_limits(&mut output, true);
    output.push_str(",\"budget\":");
    push_analysis_budget(
        &mut output,
        analysis,
        AnalysisBudgetFields {
            target_bytes: target.value().len(),
            depth: MAX_TRAVERSAL_DEPTH,
            nodes: MAX_TRAVERSAL_NODES,
            builder_bytes: MAX_BUILDER_BYTES,
            context_bytes: Some(MAX_OUTPUT_BYTES),
            impact_bytes: Some(MAX_OUTPUT_BYTES),
            output_bytes: used_output_bytes,
        },
    );
    output.push_str(",\"nonclaims\":");
    push_nonclaims(&mut output, &REVIEW_NONCLAIMS);
    output.push('}');
    output.into_string()
}

fn push_reserved_review_sections(output: &mut crate::bounded_output::CappedString) {
    output.push_str("\"behavior\":[");
    push_review_finding(
        output,
        "workspace_behavior_dependencies",
        &[],
        "Authenticated workspace call dependencies require review.",
        "No authenticated workspace call dependencies are present in the selected closure.",
    );
    output.push_str("],\"api_identity\":[");
    push_review_finding(
        output,
        "workspace_api_identity_dependencies",
        &[],
        "Authenticated workspace API identity dependencies require review.",
        "No authenticated workspace API identity dependencies are present in the selected closure.",
    );
    output.push_str("],\"security_authority\":[");
    push_review_finding(
        output,
        "workspace_security_authority_dependencies",
        &[],
        "Authenticated workspace security-authority dependencies require review.",
        "No authenticated workspace security-authority dependencies are present in the selected closure.",
    );
    output.push_str("],\"memory_ownership\":[");
    push_not_analyzed_finding(
        output,
        "workspace_memory_ownership_not_analyzed",
        "Workspace memory-ownership effects are not analyzed by this version.",
    );
    output.push_str("],\"target_artifact\":[");
    push_not_analyzed_finding(
        output,
        "workspace_target_artifact_not_analyzed",
        "Workspace target-artifact effects are not analyzed by this version.",
    );
    output.push_str("],\"migration\":[");
    push_review_finding(
        output,
        "workspace_migration_dependencies",
        &[],
        "Authenticated workspace consumer dependencies require migration review.",
        "No authenticated workspace consumer dependencies are present in the selected impact closure.",
    );
    output.push_str("],\"unsafe\":[");
    push_not_analyzed_finding(
        output,
        "workspace_unsafe_not_analyzed",
        "Workspace unsafe-code effects are not analyzed by this version.",
    );
    output.push(']');
}

fn maximum_review_evidence_bytes(edge_count: usize) -> Result<usize, Vec<Diagnostic>> {
    let context_constant = "{\"artifact\":\"context\",\"relation\":\"edges\",\"index\":".len() + 1;
    let impact_edge_constant =
        "{\"artifact\":\"impact\",\"relation\":\"dependency_edges\",\"index\":".len() + 1;
    let affected_constant =
        "{\"artifact\":\"impact\",\"relation\":\"affected\",\"index\":".len() + 1;
    // Four analyzable findings can replace `informational` with the two-byte
    // longer `review_required` disposition.
    let mut bytes = 8usize;
    for index in 0..edge_count {
        let digits = decimal_digits(index);
        bytes = bytes
            .checked_add(context_constant)
            .and_then(|value| value.checked_add(digits))
            .and_then(|value| value.checked_add(impact_edge_constant))
            .and_then(|value| value.checked_add(digits))
            .and_then(|value| value.checked_add(2))
            .ok_or_else(|| {
                vec![invariant_error(
                    "Workspace Semantic Review replay or digest binding disagrees",
                )]
            })?;
    }
    for index in 0..MAX_TRAVERSAL_NODES.saturating_sub(1) {
        bytes = bytes
            .checked_add(affected_constant)
            .and_then(|value| value.checked_add(decimal_digits(index)))
            .and_then(|value| value.checked_add(1))
            .ok_or_else(|| {
                vec![invariant_error(
                    "Workspace Semantic Review replay or digest binding disagrees",
                )]
            })?;
    }
    Ok(bytes)
}

const fn decimal_digits(mut value: usize) -> usize {
    let mut digits = 1usize;
    while value >= 10 {
        value /= 10;
        digits += 1;
    }
    digits
}

fn render_review_json(
    analysis: &WorkspaceAnalysis,
    context: &WorkspaceContextArtifact,
    impact: &WorkspaceImpactArtifact,
    digest: Option<&str>,
    used_output_bytes: usize,
) -> String {
    let mut output = crate::bounded_output::CappedString::new();
    output.push_str("{\"schema\":");
    push_json_string(&mut output, REVIEW_SCHEMA);
    output.push_str(",\"workspace_manifest_schema\":");
    push_json_string(&mut output, WORKSPACE_MANIFEST_SCHEMA);
    output.push_str(",\"workspace_revision\":");
    push_json_string(&mut output, analysis.workspace_revision());
    output.push_str(",\"workspace_graph_digest\":");
    push_json_string(&mut output, &analysis.workspace_graph_digest);
    if let Some(digest) = digest {
        output.push_str(",\"artifact_digest\":");
        push_json_string(&mut output, digest);
    }
    output.push_str(",\"entry\":");
    push_entry(&mut output, analysis);
    output.push_str(",\"target\":");
    push_target(&mut output, analysis, &context.facts.target);
    output.push_str(",\"context\":");
    output.push_str(&context.json);
    output.push_str(",\"impact\":");
    output.push_str(&impact.json);
    output.push_str(",\"sections\":{");
    push_review_sections(&mut output, analysis, context, impact);
    output.push_str("},\"limits\":");
    push_common_limits(&mut output, true);
    output.push_str(",\"budget\":");
    let context_depth = used_depth(&context.facts.nodes);
    let impact_depth = impact
        .facts
        .nodes
        .iter()
        .map(|fact| fact.node.depth)
        .max()
        .unwrap_or(0);
    push_analysis_budget(
        &mut output,
        analysis,
        AnalysisBudgetFields {
            target_bytes: context.facts.target.value().len(),
            depth: context_depth.max(impact_depth),
            nodes: context.facts.nodes.len().max(impact.facts.nodes.len()),
            builder_bytes: impact.facts.aggregate_builder_bytes,
            context_bytes: Some(context.json.len()),
            impact_bytes: Some(impact.json.len()),
            output_bytes: used_output_bytes,
        },
    );
    output.push_str(",\"nonclaims\":");
    push_nonclaims(&mut output, &REVIEW_NONCLAIMS);
    output.push('}');
    output.into_string()
}

fn push_review_sections(
    output: &mut crate::bounded_output::CappedString,
    analysis: &WorkspaceAnalysis,
    context: &WorkspaceContextArtifact,
    impact: &WorkspaceImpactArtifact,
) {
    let behavior = dependency_evidence(analysis, context, impact, &["call"], false);
    let api_identity = dependency_evidence(
        analysis,
        context,
        impact,
        &["function_import", "type_import", "type_reference"],
        false,
    );
    let security = dependency_evidence(
        analysis,
        context,
        impact,
        &["effect_requirement", "capability_authority"],
        false,
    );
    let migration = dependency_evidence(analysis, context, impact, &[], true);

    output.push_str("\"behavior\":[");
    push_review_finding(
        output,
        "workspace_behavior_dependencies",
        &behavior,
        "Authenticated workspace call dependencies require review.",
        "No authenticated workspace call dependencies are present in the selected closure.",
    );
    output.push_str("],\"api_identity\":[");
    push_review_finding(
        output,
        "workspace_api_identity_dependencies",
        &api_identity,
        "Authenticated workspace API identity dependencies require review.",
        "No authenticated workspace API identity dependencies are present in the selected closure.",
    );
    output.push_str("],\"security_authority\":[");
    push_review_finding(
        output,
        "workspace_security_authority_dependencies",
        &security,
        "Authenticated workspace security-authority dependencies require review.",
        "No authenticated workspace security-authority dependencies are present in the selected closure.",
    );
    output.push_str("],\"memory_ownership\":[");
    push_not_analyzed_finding(
        output,
        "workspace_memory_ownership_not_analyzed",
        "Workspace memory-ownership effects are not analyzed by this version.",
    );
    output.push_str("],\"target_artifact\":[");
    push_not_analyzed_finding(
        output,
        "workspace_target_artifact_not_analyzed",
        "Workspace target-artifact effects are not analyzed by this version.",
    );
    output.push_str("],\"migration\":[");
    push_review_finding(
        output,
        "workspace_migration_dependencies",
        &migration,
        "Authenticated workspace consumer dependencies require migration review.",
        "No authenticated workspace consumer dependencies are present in the selected impact closure.",
    );
    output.push_str("],\"unsafe\":[");
    push_not_analyzed_finding(
        output,
        "workspace_unsafe_not_analyzed",
        "Workspace unsafe-code effects are not analyzed by this version.",
    );
    output.push(']');
}

fn dependency_evidence(
    analysis: &WorkspaceAnalysis,
    context: &WorkspaceContextArtifact,
    impact: &WorkspaceImpactArtifact,
    kinds: &[&str],
    migration: bool,
) -> Vec<ReviewEvidenceRef> {
    let mut evidence = Vec::new();
    if !migration {
        for (index, path_edge) in context.facts.path_edges.iter().enumerate() {
            let kind = analysis.edges()[path_edge.edge_index].kind();
            if kinds.contains(&kind) {
                push_review_evidence(
                    &mut evidence,
                    ReviewEvidenceRef {
                        artifact: "context",
                        relation: "edges",
                        index,
                    },
                );
            }
        }
    }
    if migration {
        for (index, affected) in impact.facts.nodes.iter().enumerate() {
            if affected.role != WorkspaceImpactRole::Root {
                push_review_evidence(
                    &mut evidence,
                    ReviewEvidenceRef {
                        artifact: "impact",
                        relation: "affected",
                        index,
                    },
                );
            }
        }
    } else {
        for (index, path_edge) in impact.facts.path_edges.iter().enumerate() {
            let kind = analysis.edges()[path_edge.edge_index].kind();
            if kinds.contains(&kind) {
                push_review_evidence(
                    &mut evidence,
                    ReviewEvidenceRef {
                        artifact: "impact",
                        relation: "dependency_edges",
                        index,
                    },
                );
            }
        }
    }
    evidence.sort();
    evidence.dedup();
    evidence
}

fn push_review_evidence(evidence: &mut Vec<ReviewEvidenceRef>, item: ReviewEvidenceRef) {
    if crate::bounded_output::reserve_active(std::mem::size_of::<ReviewEvidenceRef>()) {
        evidence.push(item);
    }
}

fn push_review_finding(
    output: &mut crate::bounded_output::CappedString,
    code: &str,
    evidence: &[ReviewEvidenceRef],
    nonempty_statement: &str,
    empty_statement: &str,
) {
    output.push_str("{\"code\":");
    push_json_string(output, code);
    output.push_str(",\"disposition\":");
    push_json_string(
        output,
        if evidence.is_empty() {
            "informational"
        } else {
            "review_required"
        },
    );
    output.push_str(",\"statement\":");
    push_json_string(
        output,
        if evidence.is_empty() {
            empty_statement
        } else {
            nonempty_statement
        },
    );
    output.push_str(",\"evidence\":");
    push_evidence(output, evidence);
    output.push('}');
}

fn push_not_analyzed_finding(
    output: &mut crate::bounded_output::CappedString,
    code: &str,
    statement: &str,
) {
    output.push_str("{\"code\":");
    push_json_string(output, code);
    output.push_str(",\"disposition\":\"not_analyzed\",\"statement\":");
    push_json_string(output, statement);
    output.push_str(",\"evidence\":[]}");
}

fn push_evidence(output: &mut crate::bounded_output::CappedString, evidence: &[ReviewEvidenceRef]) {
    output.push('[');
    for (item_index, evidence) in evidence.iter().enumerate() {
        if item_index > 0 {
            output.push(',');
        }
        output.push_str("{\"artifact\":");
        push_json_string(output, evidence.artifact);
        output.push_str(",\"relation\":");
        push_json_string(output, evidence.relation);
        write!(output, ",\"index\":{}}}", evidence.index).expect("writing to a string cannot fail");
    }
    output.push(']');
}

#[cfg(test)]
mod tests {
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
        format!("sha256:{:x}", hasher.finalize())
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
                "sha256:35f39e8220a9fcd2e952e361ed70c8e47c290eda55422d7b772348c97d97668a",
                "sha256:c93bfff8347892750ad1f0a3e87ed5dff32ede26f3793acfb904d703996becc2",
                "sha256:8d9a68b005d8f6e147c954e32d8437cc2e7a86c771cded64f2a2e2db58d3d1f7",
                "sha256:991a5a4f3e4339bd801918d50f15ffd40acb7d683162bd69d291b47b5808702c"
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
                "sha256:70259a820e24e9110874b645ec96d3c1350dcd0441d58c3562298937bc871af5",
                "sha256:20c4f1d72f10d75852580da4ad5a1e43e9c69677e26d88c9c8212bf57531727e",
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
            "sha256:ff8dd7f60be9c8fc0ff06a9216c864e502ec5cca6d577ee460f338b6e6a12cf9"
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
            checked_builder_sum(measured_facts.aggregate_builder_bytes, rank_builder_bytes)
                .unwrap();
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
}
