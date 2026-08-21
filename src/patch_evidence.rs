//! Deterministic Semantic Patch Evidence v1 generation and exact replay.

use std::collections::BTreeSet;
use std::fs::File;
use std::io::Read;
use std::path::Path;

use sha2::{Digest, Sha256};

use crate::bounded_output::BudgetedJoin as _;
use crate::diagnostic::{quote_json, Diagnostic};
use crate::{patch, review, target_evidence};

pub(crate) const EVIDENCE_SCHEMA: &str = "semaprax.semantic-patch-evidence.v1";
const VERIFICATION_SCHEMA: &str = "semaprax.semantic-patch-evidence-verification.v1";
const REVIEW_SCHEMA: &str = "semaprax.semantic-review.v1";
const REVIEW_DIGEST_DOMAIN: &[u8] = b"semaprax.semantic-patch-evidence.review-digest.v1\0";
const ARTIFACT_DIGEST_DOMAIN: &[u8] = b"semaprax.semantic-patch-evidence.artifact-digest.v1\0";
const EVIDENCE_SCHEMA_V2: &str = "semaprax.semantic-patch-evidence.v2";
const VERIFICATION_SCHEMA_V2: &str = "semaprax.semantic-patch-evidence-verification.v2";
const ARTIFACT_DIGEST_DOMAIN_V2: &[u8] = b"semaprax.semantic-patch-evidence.artifact-digest.v2\0";
const MAX_EVIDENCE_BYTES: usize = 65_536;
const MAX_RECEIPT_BYTES: usize = 65_536;
const MAX_JSON_NESTING_DEPTH: usize = 8;
const ASSESSMENT_KEYS: [&str; 7] = [
    "behavior",
    "api_identity",
    "security_authority",
    "memory_ownership",
    "target_artifact",
    "migration",
    "unsafe",
];
const ASSESSMENT_VALUES: [&str; 5] = [
    "change_proven",
    "unchanged_within_admitted_domain",
    "mixed",
    "unknown",
    "not_applicable",
];
const NONCLAIMS: [&str; 13] = [
    "not_signature_or_authenticated_provenance",
    "not_human_approval_or_policy",
    "not_safe_compatible_or_target_verified",
    "no_commit_authority",
    "no_reusable_authorization_token",
    "no_test_or_target_execution",
    "no_agent_context_or_repository_analysis",
    "no_multi_file_transaction",
    "no_general_proof_system",
    "no_semantic_impact_v3",
    "no_persistence_or_incrementality",
    "no_external_consumer_compatibility",
    "no_new_patch_repair_graph_cleanup_or_runtime_semantics",
];
const NONCLAIMS_V2: [&str; 15] = [
    "not_signature_or_authenticated_provenance",
    "not_human_approval_or_policy",
    "not_safe_compatible_or_abi_verified",
    "no_commit_authority",
    "no_reusable_authorization_token",
    "no_project_test_discovery_or_execution",
    "no_native_toolchain_or_runtime_execution",
    "native_evidence_is_deterministic_c11_source_only",
    "wasm_evidence_is_deterministic_core_module_only",
    "no_agent_context_or_repository_analysis",
    "no_multi_file_transaction",
    "no_general_proof_system_or_capability_flow_theorem",
    "no_persistence_or_incrementality",
    "no_external_consumer_compatibility",
    "no_new_patch_repair_graph_cleanup_or_runtime_semantics",
];

#[derive(Clone, Copy)]
pub(crate) struct EvidenceUsage {
    source_bytes: usize,
    patch_bytes: usize,
    operations: usize,
    declarations: usize,
    callables: usize,
    call_sites: usize,
    impact_depth: usize,
    impact_nodes: usize,
    impact_bytes: usize,
    review_bytes: usize,
}

#[derive(Clone)]
pub(crate) struct PatchEvidenceFacts {
    source_graph_schema: String,
    base_revision: String,
    candidate_revision: String,
    source_digest: String,
    patch_schema: String,
    patch_digest: String,
    review_digest: String,
    assessments: [String; 7],
    supporting_kind: String,
    supporting_schema: String,
    supporting_digest: String,
    usage: EvidenceUsage,
}

#[allow(
    dead_code,
    reason = "shared by the private Workspace Evidence Phase A build"
)]
impl PatchEvidenceFacts {
    pub(crate) fn source_graph_schema(&self) -> &str {
        &self.source_graph_schema
    }

    pub(crate) fn base_revision(&self) -> &str {
        &self.base_revision
    }

    pub(crate) fn candidate_revision(&self) -> &str {
        &self.candidate_revision
    }

    pub(crate) fn source_digest(&self) -> &str {
        &self.source_digest
    }

    pub(crate) fn patch_schema(&self) -> &str {
        &self.patch_schema
    }

    pub(crate) fn patch_digest(&self) -> &str {
        &self.patch_digest
    }

    pub(crate) fn review_digest(&self) -> &str {
        &self.review_digest
    }

    pub(crate) fn assessments(&self) -> &[String; 7] {
        &self.assessments
    }

    pub(crate) fn supporting_kind(&self) -> &str {
        &self.supporting_kind
    }

    pub(crate) fn supporting_schema(&self) -> &str {
        &self.supporting_schema
    }

    pub(crate) fn supporting_digest(&self) -> &str {
        &self.supporting_digest
    }

    pub(crate) fn usage(&self) -> EvidenceUsage {
        self.usage
    }
}

#[allow(
    dead_code,
    reason = "shared by the private Workspace Evidence Phase A build"
)]
impl EvidenceUsage {
    pub(crate) fn source_bytes(self) -> usize {
        self.source_bytes
    }

    pub(crate) fn patch_bytes(self) -> usize {
        self.patch_bytes
    }

    pub(crate) fn operations(self) -> usize {
        self.operations
    }

    pub(crate) fn declarations(self) -> usize {
        self.declarations
    }

    pub(crate) fn callables(self) -> usize {
        self.callables
    }

    pub(crate) fn call_sites(self) -> usize {
        self.call_sites
    }

    pub(crate) fn impact_depth(self) -> usize {
        self.impact_depth
    }

    pub(crate) fn impact_nodes(self) -> usize {
        self.impact_nodes
    }

    pub(crate) fn impact_bytes(self) -> usize {
        self.impact_bytes
    }

    pub(crate) fn review_bytes(self) -> usize {
        self.review_bytes
    }
}

#[allow(dead_code, reason = "digest is shared by Workspace Evidence Phase A")]
pub(crate) struct RenderedPatchEvidence {
    artifact: String,
    digest: String,
}

#[allow(
    dead_code,
    reason = "shared by the private Workspace Evidence Phase A build"
)]
impl RenderedPatchEvidence {
    pub(crate) fn artifact(&self) -> &str {
        &self.artifact
    }

    pub(crate) fn digest(&self) -> &str {
        &self.digest
    }

    pub(crate) fn into_parts(self) -> (String, String) {
        (self.artifact, self.digest)
    }
}

struct CapsuleV2Facts {
    review: PatchEvidenceFacts,
    target_digest: String,
    target_report_bytes: usize,
    target_usage: [usize; 6],
}

pub fn generate(source_path: &Path, patch_path: &Path) -> Result<String, Vec<Diagnostic>> {
    generate_with_hook(source_path, patch_path, |_, _, _| Ok(()))
}

pub fn verify(
    source_path: &Path,
    patch_path: &Path,
    evidence_path: &Path,
) -> Result<String, Vec<Diagnostic>> {
    verify_with_hook(source_path, patch_path, evidence_path, |_, _, _| Ok(()))
}

pub fn apply(
    source_path: &Path,
    patch_path: &Path,
    evidence_path: &Path,
) -> Result<String, Vec<Diagnostic>> {
    apply_with_hook(source_path, patch_path, evidence_path, |_, _, _| Ok(()))
}

/// Generate one target-bound Semantic Patch Evidence v2 capsule.
pub fn generate_v2(source_path: &Path, patch_path: &Path) -> Result<String, Vec<Diagnostic>> {
    generate_v2_with_hook(source_path, patch_path, |_, _, _| Ok(()))
}

fn generate_v2_with_hook(
    source_path: &Path,
    patch_path: &Path,
    mut hook: impl FnMut(ReadPhase, &Path, &Path) -> std::io::Result<()>,
) -> Result<String, Vec<Diagnostic>> {
    let canonical_source_path = patch::canonical_source_path(source_path)?;
    let snapshot = patch::read_source_snapshot_bounded(
        &canonical_source_path,
        review::MAX_SOURCE_BYTES,
        "SPX-G131",
    )?;
    let patch_source = read_patch_bounded(patch_path)?;
    hook(ReadPhase::PatchRead, patch_path, source_path).map_err(|error| {
        vec![Diagnostic::io(
            "SPX-I202",
            format!("Semantic Patch Evidence v2 patch-read hook failed: {error}"),
        )]
    })?;
    let build = review::build_target_owned(
        snapshot.source().to_owned(),
        patch_source,
        source_path.to_path_buf(),
        target_evidence::MAX_NATIVE_C11_BYTES,
    )
    .map_err(map_review_diagnostics)?;
    let facts = facts_v2_from_review(&build)?;
    let capsule = render_capsule_v2_bounded(&facts)?;
    hook(ReadPhase::FinalCheck, &canonical_source_path, source_path).map_err(|error| {
        vec![Diagnostic::io(
            "SPX-I207",
            format!("Semantic Patch Evidence v2 final-check hook failed: {error}"),
        )]
    })?;
    patch::validate_source_unchanged_bounded(
        &canonical_source_path,
        source_path,
        &snapshot,
        build.base_revision(),
        review::MAX_SOURCE_BYTES,
    )?;
    Ok(capsule)
}

/// Independently replay and verify one target-bound evidence capsule.
pub fn verify_v2(
    source_path: &Path,
    patch_path: &Path,
    evidence_path: &Path,
) -> Result<String, Vec<Diagnostic>> {
    verify_v2_with_hook(source_path, patch_path, evidence_path, |_, _, _| Ok(()))
}

fn verify_v2_with_hook(
    source_path: &Path,
    patch_path: &Path,
    evidence_path: &Path,
    mut hook: impl FnMut(ReadPhase, &Path, &Path) -> std::io::Result<()>,
) -> Result<String, Vec<Diagnostic>> {
    let submitted = read_evidence_bounded(evidence_path)?;
    hook(ReadPhase::EvidenceRead, evidence_path, source_path).map_err(|error| {
        vec![Diagnostic::io(
            "SPX-I208",
            format!("Semantic Patch Evidence v2 evidence-read hook failed: {error}"),
        )]
    })?;
    let submitted_facts = parse_canonical_capsule_v2(&submitted)?;
    let canonical_source_path = patch::canonical_source_path(source_path)?;
    let snapshot = patch::read_source_snapshot_bounded(
        &canonical_source_path,
        review::MAX_SOURCE_BYTES,
        "SPX-G131",
    )?;
    let patch_source = read_patch_bounded(patch_path)?;
    hook(ReadPhase::PatchRead, patch_path, source_path).map_err(|error| {
        vec![Diagnostic::io(
            "SPX-I202",
            format!("Semantic Patch Evidence v2 patch-read hook failed: {error}"),
        )]
    })?;
    let build = review::build_target_owned(
        snapshot.source().to_owned(),
        patch_source,
        source_path.to_path_buf(),
        target_evidence::MAX_NATIVE_C11_BYTES,
    )
    .map_err(map_review_diagnostics)?;
    let expected_facts = facts_v2_from_review(&build)?;
    let expected = render_capsule_v2_bounded(&expected_facts)?;
    if submitted != expected || !same_v2_bindings(&submitted_facts, &expected_facts) {
        return Err(vec![mismatch_error(
            "submitted Semantic Patch Evidence v2 differs from independent canonical replay",
        )]);
    }
    let artifact_digest = domain_digest(ARTIFACT_DIGEST_DOMAIN_V2, submitted.as_bytes());
    let receipt = render_receipt_v2_bounded(&expected_facts, &artifact_digest, submitted.len())?;
    hook(ReadPhase::FinalCheck, &canonical_source_path, source_path).map_err(|error| {
        vec![Diagnostic::io(
            "SPX-I207",
            format!("Semantic Patch Evidence v2 final-check hook failed: {error}"),
        )]
    })?;
    patch::validate_source_unchanged_bounded(
        &canonical_source_path,
        source_path,
        &snapshot,
        build.base_revision(),
        review::MAX_SOURCE_BYTES,
    )?;
    Ok(receipt)
}

/// Apply an already replayed target-bound capsule through the unchanged A0 boundary.
pub fn apply_v2(
    source_path: &Path,
    patch_path: &Path,
    evidence_path: &Path,
) -> Result<String, Vec<Diagnostic>> {
    apply_v2_with_hook(source_path, patch_path, evidence_path, |_, _, _| Ok(()))
}

fn apply_v2_with_hook(
    source_path: &Path,
    patch_path: &Path,
    evidence_path: &Path,
    mut hook: impl FnMut(ApplyPhase, &Path, &Path) -> std::io::Result<()>,
) -> Result<String, Vec<Diagnostic>> {
    let guard = patch::acquire_a0_commit_guard(source_path)?;
    let patch_source = read_patch_bounded(patch_path)?;
    hook(ApplyPhase::PatchRead, patch_path, source_path).map_err(|error| {
        vec![Diagnostic::io(
            "SPX-I202",
            format!("Semantic Patch Evidence v2 apply patch-read hook failed: {error}"),
        )]
    })?;
    let submitted = read_evidence_bounded(evidence_path)?;
    hook(ApplyPhase::EvidenceRead, evidence_path, source_path).map_err(|error| {
        vec![Diagnostic::io(
            "SPX-I208",
            format!("Semantic Patch Evidence v2 apply evidence-read hook failed: {error}"),
        )]
    })?;
    let submitted_facts = parse_canonical_capsule_v2(&submitted)?;
    let authenticated =
        patch::authenticate_a0_source(&guard, Some((review::MAX_SOURCE_BYTES, "SPX-G131")))?;
    let build = review::build_target_owned(
        authenticated.source().to_owned(),
        patch_source,
        source_path.to_path_buf(),
        target_evidence::MAX_NATIVE_C11_BYTES,
    )
    .map_err(map_review_diagnostics)?;
    let expected_facts = facts_v2_from_review(&build)?;
    let expected = render_capsule_v2_bounded(&expected_facts)?;
    if submitted != expected || !same_v2_bindings(&submitted_facts, &expected_facts) {
        return Err(vec![mismatch_error(
            "submitted Semantic Patch Evidence v2 differs from independent canonical replay",
        )]);
    }
    hook(
        ApplyPhase::BeforeStage,
        guard.canonical_source_path(),
        source_path,
    )
    .map_err(|error| {
        vec![Diagnostic::io(
            "SPX-I203",
            format!("Semantic Patch Evidence v2 pre-stage hook failed: {error}"),
        )]
    })?;
    let prepared = patch::prepare_a0_commit(&authenticated, build.preflight())?;
    patch::commit_prepared_a0(prepared, |phase, source, staging| {
        hook(
            match phase {
                patch::CommitPhase::BeforeFinalCheck => ApplyPhase::BeforeFinalCheck,
                patch::CommitPhase::BeforeRename => ApplyPhase::BeforeRename,
            },
            source,
            staging,
        )
    })
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum ReadPhase {
    EvidenceRead,
    PatchRead,
    FinalCheck,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum ApplyPhase {
    PatchRead,
    EvidenceRead,
    BeforeStage,
    BeforeFinalCheck,
    BeforeRename,
}

fn apply_with_hook(
    source_path: &Path,
    patch_path: &Path,
    evidence_path: &Path,
    mut hook: impl FnMut(ApplyPhase, &Path, &Path) -> std::io::Result<()>,
) -> Result<String, Vec<Diagnostic>> {
    let guard = patch::acquire_a0_commit_guard(source_path)?;
    let patch_source = read_patch_bounded(patch_path)?;
    hook(ApplyPhase::PatchRead, patch_path, source_path).map_err(|error| {
        vec![Diagnostic::io(
            "SPX-I202",
            format!("semantic patch evidence apply patch-read hook failed: {error}"),
        )]
    })?;
    let submitted = read_evidence_bounded(evidence_path)?;
    hook(ApplyPhase::EvidenceRead, evidence_path, source_path).map_err(|error| {
        vec![Diagnostic::io(
            "SPX-I208",
            format!("semantic patch evidence apply read hook failed: {error}"),
        )]
    })?;
    let submitted_facts = parse_canonical_capsule(&submitted)?;
    let authenticated =
        patch::authenticate_a0_source(&guard, Some((review::MAX_SOURCE_BYTES, "SPX-G131")))?;
    let build = review::build_owned(
        authenticated.source().to_owned(),
        patch_source,
        source_path.to_path_buf(),
    )
    .map_err(map_review_diagnostics)?;
    let expected_facts = facts_from_review(&build)?;
    let expected = render_from_facts(&expected_facts)?;
    if submitted != expected.artifact || !same_bindings(&submitted_facts, &expected_facts) {
        return Err(vec![mismatch_error(
            "submitted Semantic Patch Evidence differs from independent canonical replay",
        )]);
    }
    hook(
        ApplyPhase::BeforeStage,
        guard.canonical_source_path(),
        source_path,
    )
    .map_err(|error| {
        vec![Diagnostic::io(
            "SPX-I203",
            format!("semantic patch evidence pre-stage hook failed: {error}"),
        )]
    })?;
    let prepared = patch::prepare_a0_commit(&authenticated, build.preflight())?;
    patch::commit_prepared_a0(prepared, |phase, source, staging| {
        hook(
            match phase {
                patch::CommitPhase::BeforeFinalCheck => ApplyPhase::BeforeFinalCheck,
                patch::CommitPhase::BeforeRename => ApplyPhase::BeforeRename,
            },
            source,
            staging,
        )
    })
}

fn generate_with_hook(
    source_path: &Path,
    patch_path: &Path,
    mut hook: impl FnMut(ReadPhase, &Path, &Path) -> std::io::Result<()>,
) -> Result<String, Vec<Diagnostic>> {
    let canonical_source_path = patch::canonical_source_path(source_path)?;
    let snapshot = patch::read_source_snapshot_bounded(
        &canonical_source_path,
        review::MAX_SOURCE_BYTES,
        "SPX-G131",
    )?;
    let patch_source = read_patch_bounded(patch_path)?;
    hook(ReadPhase::PatchRead, patch_path, source_path).map_err(|error| {
        vec![Diagnostic::io(
            "SPX-I202",
            format!("semantic patch evidence patch-read hook failed: {error}"),
        )]
    })?;
    let build = review::build_owned(
        snapshot.source().to_owned(),
        patch_source,
        source_path.to_path_buf(),
    )
    .map_err(map_review_diagnostics)?;
    let facts = facts_from_review(&build)?;
    let capsule = render_from_facts(&facts)?.artifact;
    hook(ReadPhase::FinalCheck, &canonical_source_path, source_path).map_err(|error| {
        vec![Diagnostic::io(
            "SPX-I207",
            format!("semantic patch evidence final-check hook failed: {error}"),
        )]
    })?;
    patch::validate_source_unchanged_bounded(
        &canonical_source_path,
        source_path,
        &snapshot,
        build.base_revision(),
        review::MAX_SOURCE_BYTES,
    )?;
    Ok(capsule)
}

fn verify_with_hook(
    source_path: &Path,
    patch_path: &Path,
    evidence_path: &Path,
    mut hook: impl FnMut(ReadPhase, &Path, &Path) -> std::io::Result<()>,
) -> Result<String, Vec<Diagnostic>> {
    let submitted = read_evidence_bounded(evidence_path)?;
    hook(ReadPhase::EvidenceRead, evidence_path, source_path).map_err(|error| {
        vec![Diagnostic::io(
            "SPX-I208",
            format!("semantic patch evidence read hook failed: {error}"),
        )]
    })?;
    let submitted_facts = parse_canonical_capsule(&submitted)?;

    let canonical_source_path = patch::canonical_source_path(source_path)?;
    let snapshot = patch::read_source_snapshot_bounded(
        &canonical_source_path,
        review::MAX_SOURCE_BYTES,
        "SPX-G131",
    )?;
    let patch_source = read_patch_bounded(patch_path)?;
    hook(ReadPhase::PatchRead, patch_path, source_path).map_err(|error| {
        vec![Diagnostic::io(
            "SPX-I202",
            format!("semantic patch evidence patch-read hook failed: {error}"),
        )]
    })?;
    let build = review::build_owned(
        snapshot.source().to_owned(),
        patch_source,
        source_path.to_path_buf(),
    )
    .map_err(map_review_diagnostics)?;
    let expected_facts = facts_from_review(&build)?;
    let expected = render_from_facts(&expected_facts)?;
    if submitted != expected.artifact || !same_bindings(&submitted_facts, &expected_facts) {
        return Err(vec![mismatch_error(
            "submitted Semantic Patch Evidence differs from independent canonical replay",
        )]);
    }
    let artifact_digest = domain_digest(ARTIFACT_DIGEST_DOMAIN, submitted.as_bytes());
    let receipt = render_receipt_bounded(&expected_facts, &artifact_digest, submitted.len())?;
    hook(ReadPhase::FinalCheck, &canonical_source_path, source_path).map_err(|error| {
        vec![Diagnostic::io(
            "SPX-I207",
            format!("semantic patch evidence verification final-check hook failed: {error}"),
        )]
    })?;
    patch::validate_source_unchanged_bounded(
        &canonical_source_path,
        source_path,
        &snapshot,
        build.base_revision(),
        review::MAX_SOURCE_BYTES,
    )?;
    Ok(receipt)
}

/// Extracts the immutable Patch Evidence v1 facts directly from the typed
/// Review build. Consumers must not reconstruct these bindings by parsing the
/// rendered Review JSON.
pub(crate) fn facts_from_review(
    build: &review::ReviewBuild,
) -> Result<PatchEvidenceFacts, Vec<Diagnostic>> {
    if build.report().len() > review::MAX_OUTPUT_BYTES {
        return Err(vec![bound_error(format!(
            "semantic review evidence exceeds {} bytes",
            review::MAX_OUTPUT_BYTES
        ))]);
    }
    let assessments = validated_assessments(
        build
            .assessments()
            .iter()
            .map(|assessment| (assessment.key(), assessment.value())),
    )
    .map_err(|error| vec![error])?;
    let supporting = build.supporting_evidence();
    validate_supporting(supporting.kind(), supporting.schema())?;
    let usage = build.usage();
    Ok(PatchEvidenceFacts {
        source_graph_schema: build.source_graph_schema().to_owned(),
        base_revision: build.base_revision().to_owned(),
        candidate_revision: build.candidate_revision().to_owned(),
        source_digest: build.source_digest().to_owned(),
        patch_schema: build.patch_schema().to_owned(),
        patch_digest: build.patch_digest().to_owned(),
        review_digest: domain_digest(REVIEW_DIGEST_DOMAIN, build.report().as_bytes()),
        assessments,
        supporting_kind: supporting.kind().to_owned(),
        supporting_schema: supporting.schema().to_owned(),
        supporting_digest: supporting.digest().to_owned(),
        usage: EvidenceUsage {
            source_bytes: usage.source_bytes(),
            patch_bytes: usage.patch_bytes(),
            operations: usage.operations(),
            declarations: usage.declarations(),
            callables: usage.callables(),
            call_sites: usage.call_sites(),
            impact_depth: usage.impact_depth(),
            impact_nodes: usage.impact_nodes(),
            impact_bytes: usage.impact_bytes(),
            review_bytes: build.report().len(),
        },
    })
}

/// Renders the exact canonical child Patch Evidence v1 artifact, including its
/// terminal LF, and binds those literal bytes with the existing v1 artifact
/// digest domain.
pub(crate) fn render_from_facts(
    facts: &PatchEvidenceFacts,
) -> Result<RenderedPatchEvidence, Vec<Diagnostic>> {
    render_from_facts_with_limit(facts, MAX_EVIDENCE_BYTES)
}

pub(crate) fn render_from_facts_with_limit(
    facts: &PatchEvidenceFacts,
    max_evidence_bytes: usize,
) -> Result<RenderedPatchEvidence, Vec<Diagnostic>> {
    let artifact = render_capsule_bounded_with_limit(facts, max_evidence_bytes)?;
    let digest = domain_digest(ARTIFACT_DIGEST_DOMAIN, artifact.as_bytes());
    Ok(RenderedPatchEvidence { artifact, digest })
}

fn facts_v2_from_review(build: &review::ReviewBuild) -> Result<CapsuleV2Facts, Vec<Diagnostic>> {
    let mut review = facts_from_review(build)?;
    let target = target_evidence::build_from_review(build).map_err(map_review_diagnostics)?;
    if !target.capability_unchanged() {
        return Err(vec![invariant_error(
            "typed target capability evidence is not an unchanged delta",
        )]);
    }
    review.assessments[2] = "unchanged_within_admitted_domain".to_owned();
    review.assessments[4] = if target.target_changed() {
        "change_proven"
    } else {
        "unchanged_within_admitted_domain"
    }
    .to_owned();
    let usage = target.usage();
    Ok(CapsuleV2Facts {
        review,
        target_digest: target.digest().to_owned(),
        target_report_bytes: target.report().len(),
        target_usage: usage,
    })
}

fn render_capsule_v2_bounded(facts: &CapsuleV2Facts) -> Result<String, Vec<Diagnostic>> {
    let mut used_evidence_bytes = 0usize;
    for _ in 0..4 {
        let mut output = render_capsule_v2(facts, used_evidence_bytes);
        output.push('\n');
        if output.len() == used_evidence_bytes {
            if output.len() > MAX_EVIDENCE_BYTES {
                return Err(vec![bound_error(format!(
                    "Semantic Patch Evidence v2 exceeds {MAX_EVIDENCE_BYTES} bytes"
                ))]);
            }
            return Ok(output);
        }
        used_evidence_bytes = output.len();
    }
    Err(vec![invariant_error(
        "Semantic Patch Evidence v2 byte accounting did not converge",
    )])
}

fn render_capsule_v2(facts: &CapsuleV2Facts, used_evidence_bytes: usize) -> String {
    let base = &facts.review;
    let target = facts.target_usage;
    format!(
        "{{\"schema\":\"{EVIDENCE_SCHEMA_V2}\",\"source_graph_schema\":{},\"base_revision\":{},\"candidate_revision\":{},\"source\":{{\"digest\":{}}},\"patch\":{{\"schema\":{},\"digest\":{}}},\"review\":{{\"schema\":\"{REVIEW_SCHEMA}\",\"digest\":{}}},\"assessments\":{},\"supporting_evidence\":{{\"id\":\"evidence:0\",\"kind\":{},\"schema\":{},\"digest\":{}}},\"target_evidence\":{{\"id\":\"evidence:1\",\"kind\":\"semantic_target_evidence_v1\",\"schema\":\"semaprax.semantic-target-evidence.v1\",\"digest\":{}}},\"limits\":{},\"budget\":{{\"used_source_bytes\":{},\"used_patch_bytes\":{},\"used_operations\":{},\"used_declarations\":{},\"used_callables\":{},\"used_call_sites\":{},\"used_impact_depth\":{},\"used_impact_nodes\":{},\"used_impact_bytes\":{},\"used_review_bytes\":{},\"used_target_evidence_bytes\":{},\"used_base_graph_bytes\":{},\"used_candidate_graph_bytes\":{},\"used_base_native_c11_bytes\":{},\"used_candidate_native_c11_bytes\":{},\"used_base_wasm_core_bytes\":{},\"used_candidate_wasm_core_bytes\":{},\"used_evidence_bytes\":{used_evidence_bytes}}},\"nonclaims\":{}}}",
        quote_json(&base.source_graph_schema), quote_json(&base.base_revision),
        quote_json(&base.candidate_revision), quote_json(&base.source_digest),
        quote_json(&base.patch_schema), quote_json(&base.patch_digest),
        quote_json(&base.review_digest), assessments_json(&base.assessments),
        quote_json(&base.supporting_kind), quote_json(&base.supporting_schema),
        quote_json(&base.supporting_digest), quote_json(&facts.target_digest),
        limits_v2_json(), base.usage.source_bytes, base.usage.patch_bytes,
        base.usage.operations, base.usage.declarations, base.usage.callables,
        base.usage.call_sites, base.usage.impact_depth, base.usage.impact_nodes,
        base.usage.impact_bytes, base.usage.review_bytes, facts.target_report_bytes,
        target[0], target[1], target[2], target[3], target[4], target[5],
        nonclaims_v2_json(),
    )
}

fn render_receipt_v2_bounded(
    facts: &CapsuleV2Facts,
    artifact_digest: &str,
    used_evidence_bytes: usize,
) -> Result<String, Vec<Diagnostic>> {
    let mut used_receipt_bytes = 0usize;
    for _ in 0..4 {
        let mut output = render_receipt_v2(
            facts,
            artifact_digest,
            used_evidence_bytes,
            used_receipt_bytes,
        );
        output.push('\n');
        if output.len() == used_receipt_bytes {
            if output.len() > MAX_RECEIPT_BYTES {
                return Err(vec![bound_error(format!(
                    "Semantic Patch Evidence v2 receipt exceeds {MAX_RECEIPT_BYTES} bytes"
                ))]);
            }
            return Ok(output);
        }
        used_receipt_bytes = output.len();
    }
    Err(vec![invariant_error(
        "Semantic Patch Evidence v2 receipt byte accounting did not converge",
    )])
}

fn render_receipt_v2(
    facts: &CapsuleV2Facts,
    artifact_digest: &str,
    used_evidence_bytes: usize,
    used_receipt_bytes: usize,
) -> String {
    let base = &facts.review;
    let target = facts.target_usage;
    format!(
        "{{\"schema\":\"{VERIFICATION_SCHEMA_V2}\",\"result\":\"exact_replay\",\"source_graph_schema\":{},\"base_revision\":{},\"candidate_revision\":{},\"source\":{{\"digest\":{}}},\"patch\":{{\"schema\":{},\"digest\":{}}},\"patch_evidence\":{{\"schema\":\"{EVIDENCE_SCHEMA_V2}\",\"digest\":{}}},\"review\":{{\"schema\":\"{REVIEW_SCHEMA}\",\"digest\":{}}},\"assessments\":{},\"supporting_evidence\":{{\"id\":\"evidence:0\",\"kind\":{},\"schema\":{},\"digest\":{}}},\"target_evidence\":{{\"id\":\"evidence:1\",\"kind\":\"semantic_target_evidence_v1\",\"schema\":\"semaprax.semantic-target-evidence.v1\",\"digest\":{}}},\"limits\":{},\"budget\":{{\"used_source_bytes\":{},\"used_patch_bytes\":{},\"used_evidence_bytes\":{used_evidence_bytes},\"used_operations\":{},\"used_declarations\":{},\"used_callables\":{},\"used_call_sites\":{},\"used_impact_depth\":{},\"used_impact_nodes\":{},\"used_impact_bytes\":{},\"used_review_bytes\":{},\"used_target_evidence_bytes\":{},\"used_base_graph_bytes\":{},\"used_candidate_graph_bytes\":{},\"used_base_native_c11_bytes\":{},\"used_candidate_native_c11_bytes\":{},\"used_base_wasm_core_bytes\":{},\"used_candidate_wasm_core_bytes\":{},\"used_receipt_bytes\":{used_receipt_bytes}}},\"nonclaims\":{}}}",
        quote_json(&base.source_graph_schema), quote_json(&base.base_revision),
        quote_json(&base.candidate_revision), quote_json(&base.source_digest),
        quote_json(&base.patch_schema), quote_json(&base.patch_digest),
        quote_json(artifact_digest), quote_json(&base.review_digest),
        assessments_json(&base.assessments), quote_json(&base.supporting_kind),
        quote_json(&base.supporting_schema), quote_json(&base.supporting_digest),
        quote_json(&facts.target_digest), limits_v2_json(), base.usage.source_bytes,
        base.usage.patch_bytes, base.usage.operations, base.usage.declarations,
        base.usage.callables, base.usage.call_sites, base.usage.impact_depth,
        base.usage.impact_nodes, base.usage.impact_bytes, base.usage.review_bytes,
        facts.target_report_bytes, target[0], target[1], target[2], target[3], target[4],
        target[5], nonclaims_v2_json(),
    )
}

fn limits_v2_json() -> String {
    format!(
        "{{\"max_source_bytes\":{},\"max_patch_bytes\":{},\"max_evidence_bytes\":{MAX_EVIDENCE_BYTES},\"max_operations\":{},\"max_declarations\":{},\"max_callables\":{},\"max_call_sites\":{},\"max_impact_depth\":{},\"max_impact_nodes\":{},\"max_impact_bytes\":{},\"max_review_bytes\":{},\"max_target_evidence_bytes\":{},\"max_graph_bytes\":{},\"max_native_c11_bytes\":{},\"max_wasm_core_bytes\":{},\"max_receipt_bytes\":{MAX_RECEIPT_BYTES}}}",
        review::MAX_SOURCE_BYTES, review::MAX_PATCH_BYTES, review::MAX_OPERATIONS,
        review::MAX_DECLARATIONS, review::MAX_CALLABLES, review::MAX_CALL_SITES,
        review::MAX_IMPACT_DEPTH, review::MAX_IMPACT_NODES, review::MAX_IMPACT_BYTES,
        review::MAX_OUTPUT_BYTES, target_evidence::MAX_OUTPUT_BYTES,
        target_evidence::MAX_GRAPH_BYTES, target_evidence::MAX_NATIVE_C11_BYTES,
        target_evidence::MAX_WASM_CORE_BYTES,
    )
}

fn nonclaims_v2_json() -> String {
    format!(
        "[{}]",
        NONCLAIMS_V2
            .iter()
            .map(|value| quote_json(value))
            .collect::<Vec<_>>()
            .join(",")
    )
}

fn validated_assessments<'a>(
    assessments: impl IntoIterator<Item = (&'a str, &'a str)>,
) -> Result<[String; 7], Diagnostic> {
    let assessments = assessments.into_iter().collect::<Vec<_>>();
    if assessments.len() != ASSESSMENT_KEYS.len() {
        return Err(invariant_error(
            "typed Review assessment count is outside the evidence schema",
        ));
    }
    let values = assessments
        .into_iter()
        .enumerate()
        .map(|(index, (key, value))| {
            if key != ASSESSMENT_KEYS[index] || !ASSESSMENT_VALUES.contains(&value) {
                return Err(invariant_error(
                    "typed Review assessment order or value is outside the evidence schema",
                ));
            }
            Ok(value.to_owned())
        })
        .collect::<Result<Vec<_>, _>>()?;
    values.try_into().map_err(|_| {
        invariant_error("typed Review assessment count is outside the evidence schema")
    })
}

fn render_capsule_bounded_with_limit(
    facts: &PatchEvidenceFacts,
    max_evidence_bytes: usize,
) -> Result<String, Vec<Diagnostic>> {
    let mut used_evidence_bytes = 0usize;
    for _ in 0..4 {
        let limit = MAX_EVIDENCE_BYTES.min(max_evidence_bytes);
        if limit == 0 {
            return Err(vec![bound_error(
                "Semantic Patch Evidence exceeds its available evidence budget",
            )]);
        }
        let (mut output, overflowed) = crate::bounded_output::with_limit(limit - 1, || {
            render_capsule(facts, used_evidence_bytes)
        });
        output.push('\n');
        if overflowed {
            return Err(vec![bound_error(
                "Semantic Patch Evidence exceeds its available evidence budget",
            )]);
        }
        if output.len() == used_evidence_bytes {
            if output.len() > limit {
                return Err(vec![bound_error(
                    "Semantic Patch Evidence exceeds its available evidence budget",
                )]);
            }
            return Ok(output);
        }
        used_evidence_bytes = output.len();
    }
    Err(vec![invariant_error(
        "Semantic Patch Evidence byte accounting did not converge",
    )])
}

fn render_capsule(facts: &PatchEvidenceFacts, used_evidence_bytes: usize) -> String {
    crate::bounded_output::budgeted_format(format_args!(
        "{{\"schema\":\"{EVIDENCE_SCHEMA}\",\"source_graph_schema\":{},\"base_revision\":{},\"candidate_revision\":{},\"source\":{{\"digest\":{}}},\"patch\":{{\"schema\":{},\"digest\":{}}},\"review\":{{\"schema\":\"{REVIEW_SCHEMA}\",\"digest\":{}}},\"assessments\":{},\"supporting_evidence\":{{\"id\":\"evidence:0\",\"kind\":{},\"schema\":{},\"digest\":{}}},\"limits\":{},\"budget\":{{\"used_source_bytes\":{},\"used_patch_bytes\":{},\"used_operations\":{},\"used_declarations\":{},\"used_callables\":{},\"used_call_sites\":{},\"used_impact_depth\":{},\"used_impact_nodes\":{},\"used_impact_bytes\":{},\"used_review_bytes\":{},\"used_evidence_bytes\":{used_evidence_bytes}}},\"nonclaims\":{}}}",
        quote_json(&facts.source_graph_schema),
        quote_json(&facts.base_revision),
        quote_json(&facts.candidate_revision),
        quote_json(&facts.source_digest),
        quote_json(&facts.patch_schema),
        quote_json(&facts.patch_digest),
        quote_json(&facts.review_digest),
        assessments_json(&facts.assessments),
        quote_json(&facts.supporting_kind),
        quote_json(&facts.supporting_schema),
        quote_json(&facts.supporting_digest),
        limits_json(),
        facts.usage.source_bytes,
        facts.usage.patch_bytes,
        facts.usage.operations,
        facts.usage.declarations,
        facts.usage.callables,
        facts.usage.call_sites,
        facts.usage.impact_depth,
        facts.usage.impact_nodes,
        facts.usage.impact_bytes,
        facts.usage.review_bytes,
        nonclaims_json(),
    ))
}

fn render_receipt_bounded(
    facts: &PatchEvidenceFacts,
    artifact_digest: &str,
    used_evidence_bytes: usize,
) -> Result<String, Vec<Diagnostic>> {
    let mut used_receipt_bytes = 0usize;
    for _ in 0..4 {
        let mut output = render_receipt(
            facts,
            artifact_digest,
            used_evidence_bytes,
            used_receipt_bytes,
        );
        output.push('\n');
        if output.len() == used_receipt_bytes {
            if output.len() > MAX_RECEIPT_BYTES {
                return Err(vec![bound_error(format!(
                    "Semantic Patch Evidence verification receipt exceeds {MAX_RECEIPT_BYTES} bytes"
                ))]);
            }
            return Ok(output);
        }
        used_receipt_bytes = output.len();
    }
    Err(vec![invariant_error(
        "Semantic Patch Evidence verification receipt byte accounting did not converge",
    )])
}

fn render_receipt(
    facts: &PatchEvidenceFacts,
    artifact_digest: &str,
    used_evidence_bytes: usize,
    used_receipt_bytes: usize,
) -> String {
    format!(
        "{{\"schema\":\"{VERIFICATION_SCHEMA}\",\"result\":\"exact_replay\",\"source_graph_schema\":{},\"base_revision\":{},\"candidate_revision\":{},\"source\":{{\"digest\":{}}},\"patch\":{{\"schema\":{},\"digest\":{}}},\"patch_evidence\":{{\"schema\":\"{EVIDENCE_SCHEMA}\",\"digest\":{}}},\"review\":{{\"schema\":\"{REVIEW_SCHEMA}\",\"digest\":{}}},\"assessments\":{},\"supporting_evidence\":{{\"id\":\"evidence:0\",\"kind\":{},\"schema\":{},\"digest\":{}}},\"limits\":{},\"budget\":{{\"used_source_bytes\":{},\"used_patch_bytes\":{},\"used_evidence_bytes\":{used_evidence_bytes},\"used_operations\":{},\"used_declarations\":{},\"used_callables\":{},\"used_call_sites\":{},\"used_impact_depth\":{},\"used_impact_nodes\":{},\"used_impact_bytes\":{},\"used_review_bytes\":{},\"used_receipt_bytes\":{used_receipt_bytes}}},\"nonclaims\":{}}}",
        quote_json(&facts.source_graph_schema),
        quote_json(&facts.base_revision),
        quote_json(&facts.candidate_revision),
        quote_json(&facts.source_digest),
        quote_json(&facts.patch_schema),
        quote_json(&facts.patch_digest),
        quote_json(artifact_digest),
        quote_json(&facts.review_digest),
        assessments_json(&facts.assessments),
        quote_json(&facts.supporting_kind),
        quote_json(&facts.supporting_schema),
        quote_json(&facts.supporting_digest),
        limits_json(),
        facts.usage.source_bytes,
        facts.usage.patch_bytes,
        facts.usage.operations,
        facts.usage.declarations,
        facts.usage.callables,
        facts.usage.call_sites,
        facts.usage.impact_depth,
        facts.usage.impact_nodes,
        facts.usage.impact_bytes,
        facts.usage.review_bytes,
        nonclaims_json(),
    )
}

fn limits_json() -> String {
    crate::bounded_output::budgeted_format(format_args!(
        "{{\"max_source_bytes\":{},\"max_patch_bytes\":{},\"max_evidence_bytes\":{MAX_EVIDENCE_BYTES},\"max_operations\":{},\"max_declarations\":{},\"max_callables\":{},\"max_call_sites\":{},\"max_impact_depth\":{},\"max_impact_nodes\":{},\"max_impact_bytes\":{},\"max_review_bytes\":{},\"max_receipt_bytes\":{MAX_RECEIPT_BYTES}}}",
        review::MAX_SOURCE_BYTES,
        review::MAX_PATCH_BYTES,
        review::MAX_OPERATIONS,
        review::MAX_DECLARATIONS,
        review::MAX_CALLABLES,
        review::MAX_CALL_SITES,
        review::MAX_IMPACT_DEPTH,
        review::MAX_IMPACT_NODES,
        review::MAX_IMPACT_BYTES,
        review::MAX_OUTPUT_BYTES,
    ))
}

fn assessments_json(assessments: &[String; 7]) -> String {
    let entries = ASSESSMENT_KEYS
        .iter()
        .zip(assessments)
        .map(|(key, value)| {
            crate::bounded_output::budgeted_format(format_args!(
                "{}:{}",
                quote_json(key),
                quote_json(value)
            ))
        })
        .collect::<Vec<_>>()
        .as_slice()
        .budgeted_join(",");
    crate::bounded_output::budgeted_format(format_args!("{{{entries}}}"))
}

fn nonclaims_json() -> String {
    crate::bounded_output::budgeted_format(format_args!(
        "[{}]",
        NONCLAIMS
            .iter()
            .map(|value| quote_json(value))
            .collect::<Vec<_>>()
            .as_slice()
            .budgeted_join(",")
    ))
}

fn parse_canonical_capsule(source: &str) -> Result<PatchEvidenceFacts, Vec<Diagnostic>> {
    if source.as_bytes().first() == Some(&0xef)
        || source.contains('\r')
        || !source.ends_with('\n')
        || source[..source.len() - 1].contains('\n')
    {
        return Err(vec![format_error(
            "Semantic Patch Evidence must be one canonical JSON line with one terminal LF",
        )]);
    }
    let body = &source[..source.len() - 1];
    validate_json_structure(body)?;
    reject_duplicate_json_keys(body)?;
    let value: serde_json::Value = serde_json::from_str(body).map_err(|_| {
        vec![format_error(
            "Semantic Patch Evidence is not canonical UTF-8 JSON",
        )]
    })?;
    let top = exact_object(
        &value,
        &[
            "schema",
            "source_graph_schema",
            "base_revision",
            "candidate_revision",
            "source",
            "patch",
            "review",
            "assessments",
            "supporting_evidence",
            "limits",
            "budget",
            "nonclaims",
        ],
        "Semantic Patch Evidence",
    )?;
    require_text(top, "schema", EVIDENCE_SCHEMA)?;
    let source_graph_schema = text(top, "source_graph_schema")?;
    if !matches!(
        source_graph_schema.as_str(),
        "semaprax.graph.v10"
            | "semaprax.graph.v11"
            | "semaprax.graph.v12"
            | "semaprax.graph.v13"
            | "semaprax.graph.v14"
    ) {
        return Err(vec![format_error(
            "Semantic Patch Evidence has an unsupported Graph schema",
        )]);
    }
    let base_revision = digest_text(top, "base_revision")?;
    let candidate_revision = digest_text(top, "candidate_revision")?;
    let source_object = exact_object(&top["source"], &["digest"], "source")?;
    let source_digest = digest_text(source_object, "digest")?;
    let patch_object = exact_object(&top["patch"], &["schema", "digest"], "patch")?;
    let patch_schema = text(patch_object, "schema")?;
    if !matches!(
        patch_schema.as_str(),
        "semaprax.semantic-patch.v1" | "semaprax.semantic-patch.v2" | "semaprax.semantic-patch.v3"
    ) {
        return Err(vec![format_error(
            "Semantic Patch Evidence has an unsupported Patch schema",
        )]);
    }
    let patch_digest = digest_text(patch_object, "digest")?;
    let review_object = exact_object(&top["review"], &["schema", "digest"], "review")?;
    require_text(review_object, "schema", REVIEW_SCHEMA)?;
    let review_digest = digest_text(review_object, "digest")?;
    let assessment_object = exact_object(&top["assessments"], &ASSESSMENT_KEYS, "assessments")?;
    let assessments = ASSESSMENT_KEYS
        .iter()
        .map(|key| {
            let value = text(assessment_object, key)?;
            if !ASSESSMENT_VALUES.contains(&value.as_str()) {
                return Err(vec![format_error(
                    "Semantic Patch Evidence has an unknown assessment value",
                )]);
            }
            Ok(value)
        })
        .collect::<Result<Vec<_>, Vec<Diagnostic>>>()?
        .try_into()
        .map_err(|_| {
            vec![format_error(
                "Semantic Patch Evidence assessment count is noncanonical",
            )]
        })?;
    let supporting = exact_object(
        &top["supporting_evidence"],
        &["id", "kind", "schema", "digest"],
        "supporting_evidence",
    )?;
    require_text(supporting, "id", "evidence:0")?;
    let supporting_kind = text(supporting, "kind")?;
    let supporting_schema = text(supporting, "schema")?;
    validate_supporting(&supporting_kind, &supporting_schema)?;
    let supporting_digest = digest_text(supporting, "digest")?;
    validate_limits(&top["limits"])?;
    let budget = exact_object(
        &top["budget"],
        &[
            "used_source_bytes",
            "used_patch_bytes",
            "used_operations",
            "used_declarations",
            "used_callables",
            "used_call_sites",
            "used_impact_depth",
            "used_impact_nodes",
            "used_impact_bytes",
            "used_review_bytes",
            "used_evidence_bytes",
        ],
        "budget",
    )?;
    let used_evidence_bytes = number(budget, "used_evidence_bytes")?;
    if used_evidence_bytes != source.len() {
        return Err(vec![format_error(
            "Semantic Patch Evidence byte accounting is not exact",
        )]);
    }
    validate_nonclaims(&top["nonclaims"])?;
    let facts = PatchEvidenceFacts {
        source_graph_schema,
        base_revision,
        candidate_revision,
        source_digest,
        patch_schema,
        patch_digest,
        review_digest,
        assessments,
        supporting_kind,
        supporting_schema,
        supporting_digest,
        usage: EvidenceUsage {
            source_bytes: number(budget, "used_source_bytes")?,
            patch_bytes: number(budget, "used_patch_bytes")?,
            operations: number(budget, "used_operations")?,
            declarations: number(budget, "used_declarations")?,
            callables: number(budget, "used_callables")?,
            call_sites: number(budget, "used_call_sites")?,
            impact_depth: number(budget, "used_impact_depth")?,
            impact_nodes: number(budget, "used_impact_nodes")?,
            impact_bytes: number(budget, "used_impact_bytes")?,
            review_bytes: number(budget, "used_review_bytes")?,
        },
    };
    let canonical = render_capsule(&facts, used_evidence_bytes);
    if canonical != body {
        return Err(vec![format_error(
            "Semantic Patch Evidence key order or JSON spelling is noncanonical",
        )]);
    }
    Ok(facts)
}

fn parse_canonical_capsule_v2(source: &str) -> Result<CapsuleV2Facts, Vec<Diagnostic>> {
    if source.as_bytes().first() == Some(&0xef)
        || source.contains('\r')
        || !source.ends_with('\n')
        || source[..source.len() - 1].contains('\n')
    {
        return Err(vec![format_error(
            "Semantic Patch Evidence v2 must be one canonical JSON line with one terminal LF",
        )]);
    }
    let body = &source[..source.len() - 1];
    validate_json_structure(body)?;
    reject_duplicate_json_keys(body)?;
    let value: serde_json::Value = serde_json::from_str(body).map_err(|_| {
        vec![format_error(
            "Semantic Patch Evidence v2 is not canonical UTF-8 JSON",
        )]
    })?;
    let top = exact_object(
        &value,
        &[
            "schema",
            "source_graph_schema",
            "base_revision",
            "candidate_revision",
            "source",
            "patch",
            "review",
            "assessments",
            "supporting_evidence",
            "target_evidence",
            "limits",
            "budget",
            "nonclaims",
        ],
        "Semantic Patch Evidence v2",
    )?;
    require_text(top, "schema", EVIDENCE_SCHEMA_V2)?;
    let source_graph_schema = text(top, "source_graph_schema")?;
    if !matches!(
        source_graph_schema.as_str(),
        "semaprax.graph.v10"
            | "semaprax.graph.v11"
            | "semaprax.graph.v12"
            | "semaprax.graph.v13"
            | "semaprax.graph.v14"
    ) {
        return Err(vec![format_error(
            "Semantic Patch Evidence v2 has an unsupported Graph schema",
        )]);
    }
    let base_revision = digest_text(top, "base_revision")?;
    let candidate_revision = digest_text(top, "candidate_revision")?;
    let source_object = exact_object(&top["source"], &["digest"], "source")?;
    let source_digest = digest_text(source_object, "digest")?;
    let patch_object = exact_object(&top["patch"], &["schema", "digest"], "patch")?;
    let patch_schema = text(patch_object, "schema")?;
    if !matches!(
        patch_schema.as_str(),
        "semaprax.semantic-patch.v1" | "semaprax.semantic-patch.v2" | "semaprax.semantic-patch.v3"
    ) {
        return Err(vec![format_error(
            "Semantic Patch Evidence v2 has an unsupported Patch schema",
        )]);
    }
    let patch_digest = digest_text(patch_object, "digest")?;
    let review_object = exact_object(&top["review"], &["schema", "digest"], "review")?;
    require_text(review_object, "schema", REVIEW_SCHEMA)?;
    let review_digest = digest_text(review_object, "digest")?;
    let assessment_object = exact_object(&top["assessments"], &ASSESSMENT_KEYS, "assessments")?;
    let assessments = ASSESSMENT_KEYS
        .iter()
        .map(|key| {
            let value = text(assessment_object, key)?;
            if !ASSESSMENT_VALUES.contains(&value.as_str()) {
                return Err(vec![format_error(
                    "Semantic Patch Evidence v2 has an unknown assessment value",
                )]);
            }
            Ok(value)
        })
        .collect::<Result<Vec<_>, Vec<Diagnostic>>>()?
        .try_into()
        .map_err(|_| {
            vec![format_error(
                "Semantic Patch Evidence v2 assessment count is noncanonical",
            )]
        })?;
    let supporting = exact_object(
        &top["supporting_evidence"],
        &["id", "kind", "schema", "digest"],
        "supporting_evidence",
    )?;
    require_text(supporting, "id", "evidence:0")?;
    let supporting_kind = text(supporting, "kind")?;
    let supporting_schema = text(supporting, "schema")?;
    validate_supporting(&supporting_kind, &supporting_schema)?;
    let supporting_digest = digest_text(supporting, "digest")?;
    let target = exact_object(
        &top["target_evidence"],
        &["id", "kind", "schema", "digest"],
        "target_evidence",
    )?;
    require_text(target, "id", "evidence:1")?;
    require_text(target, "kind", "semantic_target_evidence_v1")?;
    require_text(target, "schema", "semaprax.semantic-target-evidence.v1")?;
    let target_digest = digest_text(target, "digest")?;
    validate_limits_v2(&top["limits"])?;
    let budget = exact_object(
        &top["budget"],
        &[
            "used_source_bytes",
            "used_patch_bytes",
            "used_operations",
            "used_declarations",
            "used_callables",
            "used_call_sites",
            "used_impact_depth",
            "used_impact_nodes",
            "used_impact_bytes",
            "used_review_bytes",
            "used_target_evidence_bytes",
            "used_base_graph_bytes",
            "used_candidate_graph_bytes",
            "used_base_native_c11_bytes",
            "used_candidate_native_c11_bytes",
            "used_base_wasm_core_bytes",
            "used_candidate_wasm_core_bytes",
            "used_evidence_bytes",
        ],
        "budget",
    )?;
    let used_evidence_bytes = number(budget, "used_evidence_bytes")?;
    if used_evidence_bytes != source.len() {
        return Err(vec![format_error(
            "Semantic Patch Evidence v2 byte accounting is not exact",
        )]);
    }
    validate_nonclaims_v2(&top["nonclaims"])?;
    let facts = CapsuleV2Facts {
        review: PatchEvidenceFacts {
            source_graph_schema,
            base_revision,
            candidate_revision,
            source_digest,
            patch_schema,
            patch_digest,
            review_digest,
            assessments,
            supporting_kind,
            supporting_schema,
            supporting_digest,
            usage: EvidenceUsage {
                source_bytes: number(budget, "used_source_bytes")?,
                patch_bytes: number(budget, "used_patch_bytes")?,
                operations: number(budget, "used_operations")?,
                declarations: number(budget, "used_declarations")?,
                callables: number(budget, "used_callables")?,
                call_sites: number(budget, "used_call_sites")?,
                impact_depth: number(budget, "used_impact_depth")?,
                impact_nodes: number(budget, "used_impact_nodes")?,
                impact_bytes: number(budget, "used_impact_bytes")?,
                review_bytes: number(budget, "used_review_bytes")?,
            },
        },
        target_digest,
        target_report_bytes: number(budget, "used_target_evidence_bytes")?,
        target_usage: [
            number(budget, "used_base_graph_bytes")?,
            number(budget, "used_candidate_graph_bytes")?,
            number(budget, "used_base_native_c11_bytes")?,
            number(budget, "used_candidate_native_c11_bytes")?,
            number(budget, "used_base_wasm_core_bytes")?,
            number(budget, "used_candidate_wasm_core_bytes")?,
        ],
    };
    if render_capsule_v2(&facts, used_evidence_bytes) != body {
        return Err(vec![format_error(
            "Semantic Patch Evidence v2 key order or JSON spelling is noncanonical",
        )]);
    }
    Ok(facts)
}

fn validate_limits_v2(value: &serde_json::Value) -> Result<(), Vec<Diagnostic>> {
    let limits = exact_object(
        value,
        &[
            "max_source_bytes",
            "max_patch_bytes",
            "max_evidence_bytes",
            "max_operations",
            "max_declarations",
            "max_callables",
            "max_call_sites",
            "max_impact_depth",
            "max_impact_nodes",
            "max_impact_bytes",
            "max_review_bytes",
            "max_target_evidence_bytes",
            "max_graph_bytes",
            "max_native_c11_bytes",
            "max_wasm_core_bytes",
            "max_receipt_bytes",
        ],
        "limits",
    )?;
    let expected = [
        ("max_source_bytes", review::MAX_SOURCE_BYTES),
        ("max_patch_bytes", review::MAX_PATCH_BYTES),
        ("max_evidence_bytes", MAX_EVIDENCE_BYTES),
        ("max_operations", review::MAX_OPERATIONS),
        ("max_declarations", review::MAX_DECLARATIONS),
        ("max_callables", review::MAX_CALLABLES),
        ("max_call_sites", review::MAX_CALL_SITES),
        ("max_impact_depth", review::MAX_IMPACT_DEPTH),
        ("max_impact_nodes", review::MAX_IMPACT_NODES),
        ("max_impact_bytes", review::MAX_IMPACT_BYTES),
        ("max_review_bytes", review::MAX_OUTPUT_BYTES),
        (
            "max_target_evidence_bytes",
            target_evidence::MAX_OUTPUT_BYTES,
        ),
        ("max_graph_bytes", target_evidence::MAX_GRAPH_BYTES),
        (
            "max_native_c11_bytes",
            target_evidence::MAX_NATIVE_C11_BYTES,
        ),
        ("max_wasm_core_bytes", target_evidence::MAX_WASM_CORE_BYTES),
        ("max_receipt_bytes", MAX_RECEIPT_BYTES),
    ];
    for (key, expected) in expected {
        if number(limits, key)? != expected {
            return Err(vec![format_error(
                "Semantic Patch Evidence v2 carries noncanonical limits",
            )]);
        }
    }
    Ok(())
}

fn validate_nonclaims_v2(value: &serde_json::Value) -> Result<(), Vec<Diagnostic>> {
    let array = value.as_array().ok_or_else(|| {
        vec![format_error(
            "Semantic Patch Evidence v2 nonclaims must be an array",
        )]
    })?;
    if array.len() != NONCLAIMS_V2.len()
        || array
            .iter()
            .zip(NONCLAIMS_V2)
            .any(|(actual, expected)| actual.as_str() != Some(expected))
    {
        return Err(vec![format_error(
            "Semantic Patch Evidence v2 nonclaims are noncanonical",
        )]);
    }
    Ok(())
}

fn same_v2_bindings(left: &CapsuleV2Facts, right: &CapsuleV2Facts) -> bool {
    same_bindings(&left.review, &right.review)
        && left.target_digest == right.target_digest
        && left.target_report_bytes == right.target_report_bytes
        && left.target_usage == right.target_usage
}

fn validate_limits(value: &serde_json::Value) -> Result<(), Vec<Diagnostic>> {
    let limits = exact_object(
        value,
        &[
            "max_source_bytes",
            "max_patch_bytes",
            "max_evidence_bytes",
            "max_operations",
            "max_declarations",
            "max_callables",
            "max_call_sites",
            "max_impact_depth",
            "max_impact_nodes",
            "max_impact_bytes",
            "max_review_bytes",
            "max_receipt_bytes",
        ],
        "limits",
    )?;
    let expected = [
        ("max_source_bytes", review::MAX_SOURCE_BYTES),
        ("max_patch_bytes", review::MAX_PATCH_BYTES),
        ("max_evidence_bytes", MAX_EVIDENCE_BYTES),
        ("max_operations", review::MAX_OPERATIONS),
        ("max_declarations", review::MAX_DECLARATIONS),
        ("max_callables", review::MAX_CALLABLES),
        ("max_call_sites", review::MAX_CALL_SITES),
        ("max_impact_depth", review::MAX_IMPACT_DEPTH),
        ("max_impact_nodes", review::MAX_IMPACT_NODES),
        ("max_impact_bytes", review::MAX_IMPACT_BYTES),
        ("max_review_bytes", review::MAX_OUTPUT_BYTES),
        ("max_receipt_bytes", MAX_RECEIPT_BYTES),
    ];
    for (key, expected) in expected {
        if number(limits, key)? != expected {
            return Err(vec![format_error(
                "Semantic Patch Evidence carries noncanonical limits",
            )]);
        }
    }
    Ok(())
}

fn validate_nonclaims(value: &serde_json::Value) -> Result<(), Vec<Diagnostic>> {
    let array = value.as_array().ok_or_else(|| {
        vec![format_error(
            "Semantic Patch Evidence nonclaims must be an array",
        )]
    })?;
    if array.len() != NONCLAIMS.len()
        || array
            .iter()
            .zip(NONCLAIMS)
            .any(|(actual, expected)| actual.as_str() != Some(expected))
    {
        return Err(vec![format_error(
            "Semantic Patch Evidence nonclaims are noncanonical",
        )]);
    }
    Ok(())
}

fn exact_object<'a>(
    value: &'a serde_json::Value,
    keys: &[&str],
    label: &str,
) -> Result<&'a serde_json::Map<String, serde_json::Value>, Vec<Diagnostic>> {
    let object = value.as_object().ok_or_else(|| {
        vec![format_error(format!(
            "Semantic Patch Evidence {label} must be an object"
        ))]
    })?;
    if object.len() != keys.len() || keys.iter().any(|key| !object.contains_key(*key)) {
        return Err(vec![format_error(format!(
            "Semantic Patch Evidence {label} has missing or extra fields"
        ))]);
    }
    Ok(object)
}

fn text(
    object: &serde_json::Map<String, serde_json::Value>,
    key: &str,
) -> Result<String, Vec<Diagnostic>> {
    object[key].as_str().map(str::to_owned).ok_or_else(|| {
        vec![format_error(format!(
            "Semantic Patch Evidence field `{key}` must be text"
        ))]
    })
}

fn require_text(
    object: &serde_json::Map<String, serde_json::Value>,
    key: &str,
    expected: &str,
) -> Result<(), Vec<Diagnostic>> {
    if object[key].as_str() != Some(expected) {
        return Err(vec![format_error(format!(
            "Semantic Patch Evidence field `{key}` has the wrong value"
        ))]);
    }
    Ok(())
}

fn digest_text(
    object: &serde_json::Map<String, serde_json::Value>,
    key: &str,
) -> Result<String, Vec<Diagnostic>> {
    let value = text(object, key)?;
    if !valid_digest(&value) {
        return Err(vec![format_error(format!(
            "Semantic Patch Evidence field `{key}` is not a canonical SHA-256 digest"
        ))]);
    }
    Ok(value)
}

fn number(
    object: &serde_json::Map<String, serde_json::Value>,
    key: &str,
) -> Result<usize, Vec<Diagnostic>> {
    object[key]
        .as_u64()
        .and_then(|value| usize::try_from(value).ok())
        .ok_or_else(|| {
            vec![format_error(format!(
                "Semantic Patch Evidence field `{key}` must be a bounded integer"
            ))]
        })
}

fn valid_digest(value: &str) -> bool {
    value.len() == 71
        && value.starts_with("sha256:")
        && value[7..]
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

fn validate_supporting(kind: &str, schema: &str) -> Result<(), Vec<Diagnostic>> {
    if !matches!(
        (kind, schema),
        ("semantic_impact_v1", "semaprax.semantic-impact.v1")
            | ("identity_rebase_v1", "semaprax.identity-rebase.v1")
    ) {
        return Err(vec![format_error(
            "Semantic Patch Evidence supporting evidence kind and schema disagree",
        )]);
    }
    Ok(())
}

fn same_bindings(left: &PatchEvidenceFacts, right: &PatchEvidenceFacts) -> bool {
    left.source_graph_schema == right.source_graph_schema
        && left.base_revision == right.base_revision
        && left.candidate_revision == right.candidate_revision
        && left.source_digest == right.source_digest
        && left.patch_schema == right.patch_schema
        && left.patch_digest == right.patch_digest
        && left.review_digest == right.review_digest
        && left.assessments == right.assessments
        && left.supporting_kind == right.supporting_kind
        && left.supporting_schema == right.supporting_schema
        && left.supporting_digest == right.supporting_digest
}

fn reject_duplicate_json_keys(source: &str) -> Result<(), Vec<Diagnostic>> {
    let bytes = source.as_bytes();
    let mut position = 0usize;
    scan_json_value(bytes, &mut position)?;
    skip_json_whitespace(bytes, &mut position);
    if position != bytes.len() {
        return Err(vec![format_error(
            "Semantic Patch Evidence has trailing JSON data",
        )]);
    }
    Ok(())
}

fn validate_json_structure(source: &str) -> Result<(), Vec<Diagnostic>> {
    let mut stack = Vec::with_capacity(MAX_JSON_NESTING_DEPTH);
    let mut in_string = false;
    let mut escaped = false;
    for byte in source.bytes() {
        if in_string {
            if escaped {
                escaped = false;
            } else if byte == b'\\' {
                escaped = true;
            } else if byte == b'"' {
                in_string = false;
            }
            continue;
        }
        match byte {
            b'"' => in_string = true,
            b'{' | b'[' => {
                if stack.len() == MAX_JSON_NESTING_DEPTH {
                    return Err(vec![format_error(format!(
                        "Semantic Patch Evidence JSON exceeds nesting depth {MAX_JSON_NESTING_DEPTH}"
                    ))]);
                }
                stack.push(byte);
            }
            b'}' if stack.pop() != Some(b'{') => {
                return Err(vec![format_error(
                    "Semantic Patch Evidence JSON structure is unbalanced",
                )]);
            }
            b']' if stack.pop() != Some(b'[') => {
                return Err(vec![format_error(
                    "Semantic Patch Evidence JSON structure is unbalanced",
                )]);
            }
            b'}' | b']' => {}
            _ => {}
        }
    }
    if in_string || escaped || !stack.is_empty() {
        return Err(vec![format_error(
            "Semantic Patch Evidence JSON structure is unbalanced",
        )]);
    }
    Ok(())
}

fn scan_json_value(bytes: &[u8], position: &mut usize) -> Result<(), Vec<Diagnostic>> {
    skip_json_whitespace(bytes, position);
    match bytes.get(*position).copied() {
        Some(b'{') => scan_json_object(bytes, position),
        Some(b'[') => scan_json_array(bytes, position),
        Some(b'"') => {
            scan_json_string(bytes, position)?;
            Ok(())
        }
        Some(_) => {
            let start = *position;
            while bytes.get(*position).is_some_and(|byte| {
                !matches!(byte, b',' | b']' | b'}' | b' ' | b'\n' | b'\r' | b'\t')
            }) {
                *position += 1;
            }
            if *position == start {
                return Err(vec![format_error("invalid Semantic Patch Evidence JSON")]);
            }
            Ok(())
        }
        None => Err(vec![format_error("truncated Semantic Patch Evidence JSON")]),
    }
}

fn scan_json_object(bytes: &[u8], position: &mut usize) -> Result<(), Vec<Diagnostic>> {
    *position += 1;
    let mut keys = BTreeSet::new();
    skip_json_whitespace(bytes, position);
    if bytes.get(*position) == Some(&b'}') {
        *position += 1;
        return Ok(());
    }
    loop {
        skip_json_whitespace(bytes, position);
        let start = *position;
        scan_json_string(bytes, position)?;
        let key: String = serde_json::from_slice(&bytes[start..*position])
            .map_err(|_| vec![format_error("invalid Semantic Patch Evidence JSON key")])?;
        if !keys.insert(key) {
            return Err(vec![format_error(
                "Semantic Patch Evidence contains a duplicate JSON key",
            )]);
        }
        skip_json_whitespace(bytes, position);
        if bytes.get(*position) != Some(&b':') {
            return Err(vec![format_error(
                "invalid Semantic Patch Evidence JSON object",
            )]);
        }
        *position += 1;
        scan_json_value(bytes, position)?;
        skip_json_whitespace(bytes, position);
        match bytes.get(*position) {
            Some(b',') => *position += 1,
            Some(b'}') => {
                *position += 1;
                return Ok(());
            }
            _ => {
                return Err(vec![format_error(
                    "invalid Semantic Patch Evidence JSON object",
                )])
            }
        }
    }
}

fn scan_json_array(bytes: &[u8], position: &mut usize) -> Result<(), Vec<Diagnostic>> {
    *position += 1;
    skip_json_whitespace(bytes, position);
    if bytes.get(*position) == Some(&b']') {
        *position += 1;
        return Ok(());
    }
    loop {
        scan_json_value(bytes, position)?;
        skip_json_whitespace(bytes, position);
        match bytes.get(*position) {
            Some(b',') => *position += 1,
            Some(b']') => {
                *position += 1;
                return Ok(());
            }
            _ => {
                return Err(vec![format_error(
                    "invalid Semantic Patch Evidence JSON array",
                )])
            }
        }
    }
}

fn scan_json_string(bytes: &[u8], position: &mut usize) -> Result<(), Vec<Diagnostic>> {
    if bytes.get(*position) != Some(&b'"') {
        return Err(vec![format_error(
            "invalid Semantic Patch Evidence JSON string",
        )]);
    }
    *position += 1;
    while let Some(byte) = bytes.get(*position).copied() {
        *position += 1;
        match byte {
            b'"' => return Ok(()),
            b'\\' => {
                if bytes.get(*position).is_none() {
                    break;
                }
                *position += 1;
            }
            0x00..=0x1f => break,
            _ => {}
        }
    }
    Err(vec![format_error(
        "invalid Semantic Patch Evidence JSON string",
    )])
}

fn skip_json_whitespace(bytes: &[u8], position: &mut usize) {
    while bytes
        .get(*position)
        .is_some_and(|byte| matches!(byte, b' ' | b'\n' | b'\r' | b'\t'))
    {
        *position += 1;
    }
}

fn read_patch_bounded(path: &Path) -> Result<String, Vec<Diagnostic>> {
    read_text_bounded(path, review::MAX_PATCH_BYTES, "SPX-I202", false)
}

fn read_evidence_bounded(path: &Path) -> Result<String, Vec<Diagnostic>> {
    read_text_bounded(path, MAX_EVIDENCE_BYTES, "SPX-I208", true)
}

fn read_text_bounded(
    path: &Path,
    max_bytes: usize,
    io_code: &'static str,
    evidence: bool,
) -> Result<String, Vec<Diagnostic>> {
    let file = File::open(path).map_err(|error| {
        vec![Diagnostic::io(
            io_code,
            format!("cannot read {}: {error}", path.display()),
        )]
    })?;
    let metadata = file.metadata().map_err(|error| {
        vec![Diagnostic::io(
            io_code,
            format!("cannot inspect {}: {error}", path.display()),
        )]
    })?;
    if metadata.len() > max_bytes as u64 {
        return Err(vec![bound_error(format!(
            "{} exceeds {max_bytes} bytes",
            if evidence {
                "Semantic Patch Evidence"
            } else {
                "semantic patch"
            }
        ))]);
    }
    let mut bytes = Vec::with_capacity(metadata.len() as usize);
    file.take((max_bytes + 1) as u64)
        .read_to_end(&mut bytes)
        .map_err(|error| {
            vec![Diagnostic::io(
                io_code,
                format!("cannot read {}: {error}", path.display()),
            )]
        })?;
    if bytes.len() > max_bytes {
        return Err(vec![bound_error(format!(
            "{} exceeds {max_bytes} bytes",
            if evidence {
                "Semantic Patch Evidence"
            } else {
                "semantic patch"
            }
        ))]);
    }
    String::from_utf8(bytes).map_err(|_| {
        vec![if evidence {
            format_error("Semantic Patch Evidence is not UTF-8")
        } else {
            Diagnostic::io(
                "SPX-I202",
                format!("semantic patch {} is not UTF-8", path.display()),
            )
        }]
    })
}

fn map_review_diagnostics(mut diagnostics: Vec<Diagnostic>) -> Vec<Diagnostic> {
    for diagnostic in &mut diagnostics {
        diagnostic.code = match diagnostic.code {
            "SPX-G120" | "SPX-G140" => "SPX-G131",
            "SPX-G121" | "SPX-G141" => "SPX-G133",
            code => code,
        };
    }
    diagnostics
}

fn domain_digest(domain: &[u8], bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(domain);
    hasher.update((bytes.len() as u64).to_le_bytes());
    hasher.update(bytes);
    format!(
        "sha256:{:x}",
        crate::digest_hex::LowerHex(hasher.finalize())
    )
}

fn format_error(message: impl Into<String>) -> Diagnostic {
    Diagnostic::io("SPX-G130", message)
}

fn bound_error(message: impl Into<String>) -> Diagnostic {
    Diagnostic::io("SPX-G131", message)
}

fn mismatch_error(message: impl Into<String>) -> Diagnostic {
    Diagnostic::io("SPX-G132", message)
}

fn invariant_error(message: impl Into<String>) -> Diagnostic {
    Diagnostic::io("SPX-G133", message)
}

#[cfg(test)]
mod tests {
    use std::sync::atomic::{AtomicUsize, Ordering};

    use super::*;
    use crate::{graph, parse};

    static NEXT_FIXTURE: AtomicUsize = AtomicUsize::new(0);

    fn fixture(
        source: &str,
        patch_source: &str,
    ) -> (
        std::path::PathBuf,
        std::path::PathBuf,
        std::path::PathBuf,
        std::path::PathBuf,
    ) {
        let sequence = NEXT_FIXTURE.fetch_add(1, Ordering::Relaxed);
        let directory = std::env::temp_dir().join(format!(
            "semaprax-patch-evidence-unit-{}-{sequence}",
            std::process::id()
        ));
        std::fs::create_dir_all(&directory).unwrap();
        let source_path = directory.join("module.spx");
        let patch_path = directory.join("change.spatch");
        let evidence_path = directory.join("evidence.json");
        std::fs::write(&source_path, source).unwrap();
        std::fs::write(&patch_path, patch_source).unwrap();
        (directory, source_path, patch_path, evidence_path)
    }

    fn assert_no_a0_artifacts(source_path: &Path) {
        assert!(std::fs::read_dir(source_path.parent().unwrap())
            .unwrap()
            .map(|entry| entry.unwrap().file_name().to_string_lossy().into_owned())
            .all(|name| {
                !(name.ends_with(".semaprax-patch.lock")
                    || name.contains(".semaprax-stage.") && name.ends_with(".tmp"))
            }));
    }

    #[test]
    fn verification_rejects_oversize_before_source_semantics() {
        let (directory, source, patch, evidence) = fixture("not source", "not patch");
        std::fs::write(&evidence, vec![b'x'; MAX_EVIDENCE_BYTES + 1]).unwrap();
        let error = verify(&source, &patch, &evidence).unwrap_err();
        assert_eq!(error[0].code, "SPX-G131");
        std::fs::remove_dir_all(directory).unwrap();
    }

    #[test]
    fn typed_assessment_count_is_an_invariant_not_an_indexing_panic() {
        let assessments = std::iter::repeat_n(("behavior", "unknown"), 8);
        let error = validated_assessments(assessments).unwrap_err();
        assert_eq!(error.code, "SPX-G133");
    }

    #[test]
    fn workspace_child_renderer_respects_tiny_remaining_budget() {
        let source = "module evidence.child_bound;\n@id(\"evidence.helper\") fn helper()->i64{1}\n@id(\"app.main\") fn main()->i64{helper()}\n";
        let program = parse(source, Path::new("child-bound.spx")).unwrap();
        let patch_source = format!(
            "base {}\nrename evidence.helper to renamed\n",
            graph::revision(&program)
        );
        let preflight = crate::patch::preflight_review_owned(
            source.to_owned(),
            patch_source,
            Path::new("child-bound.spx").to_path_buf(),
            review::MAX_OPERATIONS,
        )
        .unwrap();
        let build = review::build_from_preflight(preflight).unwrap();
        let facts = facts_from_review(&build).unwrap();
        let exact = render_from_facts(&facts).unwrap();
        assert_eq!(
            render_from_facts_with_limit(&facts, MAX_EVIDENCE_BYTES)
                .unwrap()
                .artifact(),
            exact.artifact()
        );
        let Err(error) = render_from_facts_with_limit(&facts, 1) else {
            panic!("child rendering must stop at the remaining aggregate budget")
        };
        assert_eq!(error[0].code, "SPX-G131");
    }

    #[test]
    fn evidence_v2_translates_target_bounds_and_invariants() {
        let translated = map_review_diagnostics(vec![
            Diagnostic::io("SPX-G140", "target bound"),
            Diagnostic::io("SPX-G141", "target invariant"),
        ]);
        assert_eq!(translated[0].code, "SPX-G131");
        assert_eq!(translated[1].code, "SPX-G133");
    }

    #[test]
    fn parsed_ast_call_boundary_accepts_exact_and_rejects_limit_plus_one() {
        let source_with_calls = |count: usize| {
            let mut source =
                String::from("module evidence.call_bound;\n@id(\"app.main\") fn main()->i64{\n");
            for index in 0..count {
                source.push_str(&format!("let value{index}=missing();\n"));
            }
            source.push_str("0}\n");
            source
        };
        let exact_source = source_with_calls(review::MAX_CALL_SITES);
        let exact = parse(&exact_source, Path::new("exact.spx")).unwrap();
        assert_eq!(
            review::precheck_counts_for_test(&exact).unwrap().2,
            review::MAX_CALL_SITES
        );
        let over_source = source_with_calls(review::MAX_CALL_SITES + 1);
        let over = parse(&over_source, Path::new("over.spx")).unwrap();
        let error = review::precheck_counts_for_test(&over).unwrap_err();
        assert_eq!(error[0].code, "SPX-G120");
    }

    #[test]
    fn parsed_ast_declaration_and_callable_boundaries_are_exact() {
        let declarations = |fields: usize| {
            let mut source = String::from("module evidence.declarations;\nrecord Row {\n");
            for index in 0..fields {
                source.push_str(&format!("field{index}: i64,\n"));
            }
            source.push_str("}\nfn main()->i64{0}\n");
            source
        };
        let exact = parse(
            &declarations(review::MAX_DECLARATIONS - 2),
            Path::new("declarations-exact.spx"),
        )
        .unwrap();
        assert_eq!(
            review::precheck_counts_for_test(&exact).unwrap().0,
            review::MAX_DECLARATIONS
        );
        let over = parse(
            &declarations(review::MAX_DECLARATIONS - 1),
            Path::new("declarations-over.spx"),
        )
        .unwrap();
        assert_eq!(
            review::precheck_counts_for_test(&over).unwrap_err()[0].code,
            "SPX-G120"
        );

        let callables = |count: usize| {
            let mut source = String::from("module evidence.callables;\n");
            for index in 0..count {
                source.push_str(&format!("fn callable{index}()->i64{{0}}\n"));
            }
            source
        };
        let exact = parse(
            &callables(review::MAX_CALLABLES),
            Path::new("callables-exact.spx"),
        )
        .unwrap();
        assert_eq!(
            review::precheck_counts_for_test(&exact).unwrap().1,
            review::MAX_CALLABLES
        );
        let over = parse(
            &callables(review::MAX_CALLABLES + 1),
            Path::new("callables-over.spx"),
        )
        .unwrap();
        assert_eq!(
            review::precheck_counts_for_test(&over).unwrap_err()[0].code,
            "SPX-G120"
        );
    }

    #[test]
    fn owned_text_reads_accept_exact_limits_and_reject_one_more_byte() {
        let (directory, source, patch, evidence) = fixture("source", "patch");
        for (path, limit, evidence_input) in [
            (&source, review::MAX_SOURCE_BYTES, false),
            (&patch, review::MAX_PATCH_BYTES, false),
            (&evidence, MAX_EVIDENCE_BYTES, true),
        ] {
            std::fs::write(path, vec![b'x'; limit]).unwrap();
            assert_eq!(
                read_text_bounded(path, limit, "SPX-I208", evidence_input)
                    .unwrap()
                    .len(),
                limit
            );
            std::fs::write(path, vec![b'x'; limit + 1]).unwrap();
            assert_eq!(
                read_text_bounded(path, limit, "SPX-I208", evidence_input).unwrap_err()[0].code,
                "SPX-G131"
            );
        }
        std::fs::remove_dir_all(directory).unwrap();
    }

    #[test]
    fn generation_and_verification_reject_final_source_drift() {
        let source = "module evidence.unit;\n@id(\"evidence.helper\") fn helper()->i64{1}\n@id(\"app.main\") fn main()->i64{helper()}\n";
        let program = parse(source, Path::new("evidence.spx")).unwrap();
        let patch_source = format!(
            "base {}\nrename evidence.helper to renamed\n",
            graph::revision(&program)
        );
        let (directory, source_path, patch_path, evidence_path) = fixture(source, &patch_source);
        let error = generate_with_hook(&source_path, &patch_path, |phase, canonical, _| {
            if phase == ReadPhase::FinalCheck {
                std::fs::write(canonical, source.replace("{1}", "{2}"))?;
            }
            Ok(())
        })
        .unwrap_err();
        assert_eq!(error[0].code, "SPX-I207");
        std::fs::write(&source_path, source).unwrap();
        let capsule = generate(&source_path, &patch_path).unwrap();
        std::fs::write(&evidence_path, capsule).unwrap();
        let error = verify_with_hook(
            &source_path,
            &patch_path,
            &evidence_path,
            |phase, canonical, _| {
                if phase == ReadPhase::FinalCheck {
                    std::fs::write(canonical, source.replace("{1}", "{2}"))?;
                }
                Ok(())
            },
        )
        .unwrap_err();
        assert_eq!(error[0].code, "SPX-I207");
        std::fs::remove_dir_all(directory).unwrap();
    }

    #[test]
    fn owned_patch_and_evidence_bytes_are_never_reread() {
        let source = "module evidence.owned;\n@id(\"evidence.helper\") fn helper()->i64{1}\n@id(\"app.main\") fn main()->i64{helper()}\n";
        let patch_source = format!(
            "base {}\nrename evidence.helper to renamed\n",
            graph::revision(&parse(source, Path::new("evidence.spx")).unwrap())
        );
        let (directory, source_path, patch_path, evidence_path) = fixture(source, &patch_source);
        let expected_capsule = generate(&source_path, &patch_path).unwrap();
        let actual_capsule = generate_with_hook(&source_path, &patch_path, |phase, path, _| {
            if phase == ReadPhase::PatchRead {
                std::fs::write(path, "mutated after read\n")?;
            }
            Ok(())
        })
        .unwrap();
        assert_eq!(actual_capsule, expected_capsule);

        std::fs::write(&patch_path, &patch_source).unwrap();
        std::fs::write(&evidence_path, &expected_capsule).unwrap();
        let expected_receipt = verify(&source_path, &patch_path, &evidence_path).unwrap();
        let actual_receipt = verify_with_hook(
            &source_path,
            &patch_path,
            &evidence_path,
            |phase, path, _| {
                if phase == ReadPhase::EvidenceRead {
                    std::fs::write(path, "mutated after read\n")?;
                }
                Ok(())
            },
        )
        .unwrap();
        assert_eq!(actual_receipt, expected_receipt);

        std::fs::write(&evidence_path, &expected_capsule).unwrap();
        let actual_receipt = verify_with_hook(
            &source_path,
            &patch_path,
            &evidence_path,
            |phase, path, _| {
                if phase == ReadPhase::PatchRead {
                    std::fs::write(path, "mutated after read\n")?;
                }
                Ok(())
            },
        )
        .unwrap();
        assert_eq!(actual_receipt, expected_receipt);
        std::fs::remove_dir_all(directory).unwrap();
    }

    #[test]
    fn evidence_apply_uses_owned_patch_and_evidence_bytes_exactly_once() {
        let source = "module evidence.apply_owned;\n@id(\"evidence.helper\") fn helper()->i64{1}\n@id(\"app.main\") fn main()->i64{helper()}\n";
        let patch_source = format!(
            "base {}\nrename evidence.helper to renamed\n",
            graph::revision(&parse(source, Path::new("evidence.spx")).unwrap())
        );
        let (directory, source_path, patch_path, evidence_path) = fixture(source, &patch_source);
        let capsule = generate(&source_path, &patch_path).unwrap();
        let candidate = serde_json::from_str::<serde_json::Value>(&capsule).unwrap()
            ["candidate_revision"]
            .as_str()
            .unwrap()
            .to_owned();
        std::fs::write(&evidence_path, &capsule).unwrap();
        let revision = apply_with_hook(
            &source_path,
            &patch_path,
            &evidence_path,
            |phase, path, _| {
                if phase == ApplyPhase::PatchRead || phase == ApplyPhase::EvidenceRead {
                    std::fs::write(path, "mutated after owned read\n")?;
                }
                Ok(())
            },
        )
        .unwrap();
        assert_eq!(revision, candidate);
        assert!(std::fs::read_to_string(&source_path)
            .unwrap()
            .contains("fn renamed"));
        assert_no_a0_artifacts(&source_path);
        std::fs::remove_dir_all(directory).unwrap();
    }

    #[test]
    fn evidence_apply_rechecks_source_at_every_a0_boundary() {
        let source = "module evidence.apply_drift;\n@id(\"evidence.helper\") fn helper()->i64{1}\n@id(\"app.main\") fn main()->i64{helper()}\n";
        for (label, selected) in [
            ("before-stage", ApplyPhase::BeforeStage),
            ("first-final", ApplyPhase::BeforeFinalCheck),
            ("second-final", ApplyPhase::BeforeRename),
        ] {
            let patch_source = format!(
                "base {}\nrename evidence.helper to renamed\n",
                graph::revision(&parse(source, Path::new("evidence.spx")).unwrap())
            );
            let (directory, source_path, patch_path, evidence_path) =
                fixture(source, &patch_source);
            let capsule = generate(&source_path, &patch_path).unwrap();
            std::fs::write(&evidence_path, capsule).unwrap();
            let changed = source.replace("{1}", "{2}");
            let error = apply_with_hook(
                &source_path,
                &patch_path,
                &evidence_path,
                |phase, path, _| {
                    if phase == selected {
                        std::fs::write(path, &changed)?;
                    }
                    Ok(())
                },
            )
            .unwrap_err();
            assert_eq!(error[0].code, "SPX-I207", "{label}");
            assert_eq!(std::fs::read_to_string(&source_path).unwrap(), changed);
            assert_no_a0_artifacts(&source_path);
            std::fs::remove_dir_all(directory).unwrap();
        }
    }

    #[test]
    fn evidence_apply_rejects_same_bytes_with_replaced_source_identity() {
        let source = "module evidence.apply_identity;\n@id(\"evidence.helper\") fn helper()->i64{1}\n@id(\"app.main\") fn main()->i64{helper()}\n";
        let patch_source = format!(
            "base {}\nrename evidence.helper to renamed\n",
            graph::revision(&parse(source, Path::new("evidence.spx")).unwrap())
        );
        let (directory, source_path, patch_path, evidence_path) = fixture(source, &patch_source);
        let capsule = generate(&source_path, &patch_path).unwrap();
        std::fs::write(&evidence_path, capsule).unwrap();
        let backup = source_path.with_extension("original.spx");
        let error = apply_with_hook(
            &source_path,
            &patch_path,
            &evidence_path,
            |phase, path, _| {
                if phase == ApplyPhase::BeforeRename {
                    std::fs::rename(path, &backup)?;
                    std::fs::write(path, source)?;
                }
                Ok(())
            },
        )
        .unwrap_err();
        assert_eq!(error[0].code, "SPX-I207");
        assert_eq!(std::fs::read_to_string(&source_path).unwrap(), source);
        assert_eq!(std::fs::read_to_string(&backup).unwrap(), source);
        assert_no_a0_artifacts(&source_path);
        std::fs::remove_dir_all(directory).unwrap();
    }

    #[test]
    fn evidence_apply_bounds_both_final_source_reads_and_cleans_stage() {
        let source = "module evidence.apply_growth;\n@id(\"evidence.helper\") fn helper()->i64{1}\n@id(\"app.main\") fn main()->i64{helper()}\n";
        for selected in [ApplyPhase::BeforeFinalCheck, ApplyPhase::BeforeRename] {
            let patch_source = format!(
                "base {}\nrename evidence.helper to renamed\n",
                graph::revision(&parse(source, Path::new("evidence.spx")).unwrap())
            );
            let (directory, source_path, patch_path, evidence_path) =
                fixture(source, &patch_source);
            let capsule = generate(&source_path, &patch_path).unwrap();
            std::fs::write(&evidence_path, capsule).unwrap();
            let oversized = vec![b'x'; review::MAX_SOURCE_BYTES + 1];
            let error = apply_with_hook(
                &source_path,
                &patch_path,
                &evidence_path,
                |phase, path, _| {
                    if phase == selected {
                        std::fs::write(path, &oversized)?;
                    }
                    Ok(())
                },
            )
            .unwrap_err();
            assert_eq!(error[0].code, "SPX-I207");
            assert_eq!(
                std::fs::metadata(&source_path).unwrap().len(),
                (review::MAX_SOURCE_BYTES + 1) as u64
            );
            assert_no_a0_artifacts(&source_path);
            std::fs::remove_dir_all(directory).unwrap();
        }
    }

    #[test]
    fn evidence_apply_rejects_stage_mutation_and_injected_rename_failure() {
        let source = "module evidence.apply_stage;\n@id(\"evidence.helper\") fn helper()->i64{1}\n@id(\"app.main\") fn main()->i64{helper()}\n";
        for (label, selected, expected) in [
            ("stage", ApplyPhase::BeforeFinalCheck, "SPX-I203"),
            ("rename", ApplyPhase::BeforeRename, "SPX-I204"),
        ] {
            let patch_source = format!(
                "base {}\nrename evidence.helper to renamed\n",
                graph::revision(&parse(source, Path::new("evidence.spx")).unwrap())
            );
            let (directory, source_path, patch_path, evidence_path) =
                fixture(source, &patch_source);
            let capsule = generate(&source_path, &patch_path).unwrap();
            std::fs::write(&evidence_path, capsule).unwrap();
            let error = apply_with_hook(
                &source_path,
                &patch_path,
                &evidence_path,
                |phase, _, staging| {
                    if phase == selected {
                        if selected == ApplyPhase::BeforeFinalCheck {
                            std::fs::write(staging, "mutated stage")?;
                        } else {
                            return Err(std::io::Error::new(
                                std::io::ErrorKind::PermissionDenied,
                                "injected rename rejection",
                            ));
                        }
                    }
                    Ok(())
                },
            )
            .unwrap_err();
            assert_eq!(error[0].code, expected, "{label}");
            assert_eq!(std::fs::read_to_string(&source_path).unwrap(), source);
            assert_no_a0_artifacts(&source_path);
            std::fs::remove_dir_all(directory).unwrap();
        }
    }

    #[test]
    fn evidence_apply_never_deletes_a_foreign_stage_path_replacement() {
        let source = "module evidence.apply_stage_identity;\n@id(\"evidence.helper\") fn helper()->i64{1}\n@id(\"app.main\") fn main()->i64{helper()}\n";
        let patch_source = format!(
            "base {}\nrename evidence.helper to renamed\n",
            graph::revision(&parse(source, Path::new("evidence.spx")).unwrap())
        );
        let (directory, source_path, patch_path, evidence_path) = fixture(source, &patch_source);
        let capsule = generate(&source_path, &patch_path).unwrap();
        std::fs::write(&evidence_path, capsule).unwrap();
        let displaced = source_path.with_extension("owned-stage");
        let mut foreign = None;
        let error = apply_with_hook(
            &source_path,
            &patch_path,
            &evidence_path,
            |phase, _, staging| {
                if phase == ApplyPhase::BeforeRename {
                    std::fs::rename(staging, &displaced)?;
                    std::fs::write(staging, "foreign path object")?;
                    foreign = Some(staging.to_path_buf());
                }
                Ok(())
            },
        )
        .unwrap_err();
        assert_eq!(error[0].code, "SPX-I203");
        let foreign = foreign.unwrap();
        assert_eq!(
            std::fs::read_to_string(&foreign).unwrap(),
            "foreign path object"
        );
        assert!(displaced.exists());
        assert_eq!(std::fs::read_to_string(&source_path).unwrap(), source);
        assert!(std::fs::read_dir(&directory)
            .unwrap()
            .map(|entry| entry.unwrap().file_name().to_string_lossy().into_owned())
            .all(|name| !name.ends_with(".semaprax-patch.lock")));
        std::fs::remove_file(foreign).unwrap();
        std::fs::remove_file(displaced).unwrap();
        assert_no_a0_artifacts(&source_path);
        std::fs::remove_dir_all(directory).unwrap();
    }

    #[test]
    fn evidence_v2_apply_owns_inputs_and_replays_every_a0_boundary() {
        let source = "module evidence.v2_hooks;\n@id(\"evidence.helper\") fn helper()->i64{1}\n@id(\"app.main\") fn main()->i64{helper()}\n";
        let patch_source = format!(
            "schema semaprax.semantic-patch.v2\nbase {}\nrename evidence.helper to renamed\nrequire no-new-effects\n",
            graph::revision(&parse(source, Path::new("evidence.spx")).unwrap())
        );
        let (directory, source_path, patch_path, evidence_path) = fixture(source, &patch_source);
        let capsule = generate_v2(&source_path, &patch_path).unwrap();
        std::fs::write(&evidence_path, &capsule).unwrap();
        let revision = apply_v2_with_hook(
            &source_path,
            &patch_path,
            &evidence_path,
            |phase, path, _| {
                if matches!(phase, ApplyPhase::PatchRead | ApplyPhase::EvidenceRead) {
                    std::fs::write(path, "mutated after owned read\n")?;
                }
                Ok(())
            },
        )
        .unwrap();
        assert!(revision.starts_with("sha256:"));
        assert!(std::fs::read_to_string(&source_path)
            .unwrap()
            .contains("fn renamed"));
        assert_no_a0_artifacts(&source_path);
        std::fs::remove_dir_all(directory).unwrap();

        for (label, selected, expected) in [
            ("before-stage", ApplyPhase::BeforeStage, "SPX-I207"),
            ("first-final", ApplyPhase::BeforeFinalCheck, "SPX-I207"),
            ("second-final", ApplyPhase::BeforeRename, "SPX-I207"),
        ] {
            let (directory, source_path, patch_path, evidence_path) =
                fixture(source, &patch_source);
            let capsule = generate_v2(&source_path, &patch_path).unwrap();
            std::fs::write(&evidence_path, capsule).unwrap();
            let changed = source.replace("{1}", "{2}");
            let error = apply_v2_with_hook(
                &source_path,
                &patch_path,
                &evidence_path,
                |phase, path, _| {
                    if phase == selected {
                        std::fs::write(path, &changed)?;
                    }
                    Ok(())
                },
            )
            .unwrap_err();
            assert_eq!(error[0].code, expected, "{label}");
            assert_eq!(std::fs::read_to_string(&source_path).unwrap(), changed);
            assert_no_a0_artifacts(&source_path);
            std::fs::remove_dir_all(directory).unwrap();
        }
    }

    #[test]
    fn evidence_v2_read_only_routes_own_inputs_and_reject_final_drift() {
        let source = "module evidence.v2_readonly;\n@id(\"evidence.helper\") fn helper()->i64{1}\n@id(\"app.main\") fn main()->i64{helper()}\n";
        let patch_source = format!(
            "schema semaprax.semantic-patch.v2\nbase {}\nrename evidence.helper to renamed\nrequire no-new-effects\n",
            graph::revision(&parse(source, Path::new("evidence.spx")).unwrap())
        );
        let (directory, source_path, patch_path, evidence_path) = fixture(source, &patch_source);
        let expected_capsule = generate_v2(&source_path, &patch_path).unwrap();
        let capsule = generate_v2_with_hook(&source_path, &patch_path, |phase, path, _| {
            if phase == ReadPhase::PatchRead {
                std::fs::write(path, "mutated after read\n")?;
            }
            Ok(())
        })
        .unwrap();
        assert_eq!(capsule, expected_capsule);

        std::fs::write(&patch_path, &patch_source).unwrap();
        std::fs::write(&evidence_path, &expected_capsule).unwrap();
        let expected_receipt = verify_v2(&source_path, &patch_path, &evidence_path).unwrap();
        let receipt = verify_v2_with_hook(
            &source_path,
            &patch_path,
            &evidence_path,
            |phase, path, _| {
                if phase == ReadPhase::EvidenceRead {
                    std::fs::write(path, "mutated after read\n")?;
                }
                Ok(())
            },
        )
        .unwrap();
        assert_eq!(receipt, expected_receipt);

        std::fs::write(&evidence_path, &expected_capsule).unwrap();
        let receipt = verify_v2_with_hook(
            &source_path,
            &patch_path,
            &evidence_path,
            |phase, path, _| {
                if phase == ReadPhase::PatchRead {
                    std::fs::write(path, "mutated after read\n")?;
                }
                Ok(())
            },
        )
        .unwrap();
        assert_eq!(receipt, expected_receipt);

        std::fs::write(&patch_path, &patch_source).unwrap();
        std::fs::write(&evidence_path, &expected_capsule).unwrap();
        let error = generate_v2_with_hook(&source_path, &patch_path, |phase, path, _| {
            if phase == ReadPhase::FinalCheck {
                std::fs::write(path, source.replace("{1}", "{2}"))?;
            }
            Ok(())
        })
        .unwrap_err();
        assert_eq!(error[0].code, "SPX-I207");

        std::fs::write(&source_path, source).unwrap();
        let backup = source_path.with_extension("readonly-original.spx");
        let error = verify_v2_with_hook(
            &source_path,
            &patch_path,
            &evidence_path,
            |phase, path, _| {
                if phase == ReadPhase::FinalCheck {
                    std::fs::rename(path, &backup)?;
                    std::fs::write(path, source)?;
                }
                Ok(())
            },
        )
        .unwrap_err();
        assert_eq!(error[0].code, "SPX-I207");
        assert_eq!(std::fs::read_to_string(&source_path).unwrap(), source);
        assert_eq!(std::fs::read_to_string(&backup).unwrap(), source);
        std::fs::remove_dir_all(directory).unwrap();
    }

    #[test]
    fn evidence_v2_apply_rejects_stage_replacement_and_rename_failure() {
        let source = "module evidence.v2_stage;\n@id(\"evidence.helper\") fn helper()->i64{1}\n@id(\"app.main\") fn main()->i64{helper()}\n";
        let patch_source = format!(
            "schema semaprax.semantic-patch.v2\nbase {}\nrename evidence.helper to renamed\nrequire no-new-effects\n",
            graph::revision(&parse(source, Path::new("evidence.spx")).unwrap())
        );
        for (selected, expected) in [
            (ApplyPhase::BeforeFinalCheck, "SPX-I203"),
            (ApplyPhase::BeforeRename, "SPX-I204"),
        ] {
            let (directory, source_path, patch_path, evidence_path) =
                fixture(source, &patch_source);
            let capsule = generate_v2(&source_path, &patch_path).unwrap();
            std::fs::write(&evidence_path, capsule).unwrap();
            let error = apply_v2_with_hook(
                &source_path,
                &patch_path,
                &evidence_path,
                |phase, _, staging| {
                    if phase == selected {
                        if selected == ApplyPhase::BeforeFinalCheck {
                            std::fs::write(staging, "mutated stage")?;
                        } else {
                            return Err(std::io::Error::new(
                                std::io::ErrorKind::PermissionDenied,
                                "injected rename failure",
                            ));
                        }
                    }
                    Ok(())
                },
            )
            .unwrap_err();
            assert_eq!(error[0].code, expected);
            assert_eq!(std::fs::read_to_string(&source_path).unwrap(), source);
            assert_no_a0_artifacts(&source_path);
            std::fs::remove_dir_all(directory).unwrap();
        }
    }

    #[test]
    fn evidence_v2_apply_rejects_same_bytes_with_replaced_source_identity() {
        let source = "module evidence.v2_identity;\n@id(\"evidence.helper\") fn helper()->i64{1}\n@id(\"app.main\") fn main()->i64{helper()}\n";
        let patch_source = format!(
            "schema semaprax.semantic-patch.v2\nbase {}\nrename evidence.helper to renamed\nrequire no-new-effects\n",
            graph::revision(&parse(source, Path::new("evidence.spx")).unwrap())
        );
        let (directory, source_path, patch_path, evidence_path) = fixture(source, &patch_source);
        let capsule = generate_v2(&source_path, &patch_path).unwrap();
        std::fs::write(&evidence_path, capsule).unwrap();
        let backup = source_path.with_extension("original.spx");
        let error = apply_v2_with_hook(
            &source_path,
            &patch_path,
            &evidence_path,
            |phase, path, _| {
                if phase == ApplyPhase::BeforeRename {
                    std::fs::rename(path, &backup)?;
                    std::fs::write(path, source)?;
                }
                Ok(())
            },
        )
        .unwrap_err();
        assert_eq!(error[0].code, "SPX-I207");
        assert_eq!(std::fs::read_to_string(&source_path).unwrap(), source);
        assert_eq!(std::fs::read_to_string(&backup).unwrap(), source);
        assert_no_a0_artifacts(&source_path);
        std::fs::remove_dir_all(directory).unwrap();
    }

    #[test]
    fn evidence_v2_apply_bounds_both_final_reads_and_preserves_foreign_stage() {
        let source = "module evidence.v2_growth;\n@id(\"evidence.helper\") fn helper()->i64{1}\n@id(\"app.main\") fn main()->i64{helper()}\n";
        let patch_source = format!(
            "schema semaprax.semantic-patch.v2\nbase {}\nrename evidence.helper to renamed\nrequire no-new-effects\n",
            graph::revision(&parse(source, Path::new("evidence.spx")).unwrap())
        );
        for selected in [ApplyPhase::BeforeFinalCheck, ApplyPhase::BeforeRename] {
            let (directory, source_path, patch_path, evidence_path) =
                fixture(source, &patch_source);
            let capsule = generate_v2(&source_path, &patch_path).unwrap();
            std::fs::write(&evidence_path, capsule).unwrap();
            let oversized = vec![b'x'; review::MAX_SOURCE_BYTES + 1];
            let error = apply_v2_with_hook(
                &source_path,
                &patch_path,
                &evidence_path,
                |phase, path, _| {
                    if phase == selected {
                        std::fs::write(path, &oversized)?;
                    }
                    Ok(())
                },
            )
            .unwrap_err();
            assert_eq!(error[0].code, "SPX-I207");
            assert_eq!(
                std::fs::metadata(&source_path).unwrap().len(),
                (review::MAX_SOURCE_BYTES + 1) as u64
            );
            assert_no_a0_artifacts(&source_path);
            std::fs::remove_dir_all(directory).unwrap();
        }

        let (directory, source_path, patch_path, evidence_path) = fixture(source, &patch_source);
        let capsule = generate_v2(&source_path, &patch_path).unwrap();
        std::fs::write(&evidence_path, capsule).unwrap();
        let displaced = source_path.with_extension("owned-stage");
        let mut foreign = None;
        let error = apply_v2_with_hook(
            &source_path,
            &patch_path,
            &evidence_path,
            |phase, _, staging| {
                if phase == ApplyPhase::BeforeRename {
                    std::fs::rename(staging, &displaced)?;
                    std::fs::write(staging, "foreign v2 stage")?;
                    foreign = Some(staging.to_path_buf());
                }
                Ok(())
            },
        )
        .unwrap_err();
        assert_eq!(error[0].code, "SPX-I203");
        let foreign = foreign.unwrap();
        assert_eq!(
            std::fs::read_to_string(&foreign).unwrap(),
            "foreign v2 stage"
        );
        assert!(displaced.exists());
        assert_eq!(std::fs::read_to_string(&source_path).unwrap(), source);
        std::fs::remove_file(foreign).unwrap();
        std::fs::remove_file(displaced).unwrap();
        assert_no_a0_artifacts(&source_path);
        std::fs::remove_dir_all(directory).unwrap();
    }

    #[test]
    fn evidence_v2_apply_acquires_lock_before_owned_reads() {
        let source = "module evidence.v2_lock;\n@id(\"evidence.helper\") fn helper()->i64{1}\n@id(\"app.main\") fn main()->i64{helper()}\n";
        let patch_source = format!(
            "schema semaprax.semantic-patch.v2\nbase {}\nrename evidence.helper to renamed\nrequire no-new-effects\n",
            graph::revision(&parse(source, Path::new("evidence.spx")).unwrap())
        );
        let (directory, source_path, patch_path, evidence_path) = fixture(source, &patch_source);
        let capsule = generate_v2(&source_path, &patch_path).unwrap();
        std::fs::write(&evidence_path, capsule).unwrap();
        apply_v2_with_hook(
            &source_path,
            &patch_path,
            &evidence_path,
            |phase, path, _| {
                if phase == ApplyPhase::PatchRead {
                    let names = std::fs::read_dir(path.parent().unwrap())?
                        .map(|entry| entry.unwrap().file_name().to_string_lossy().into_owned())
                        .collect::<Vec<_>>();
                    assert!(names
                        .iter()
                        .any(|name| name.ends_with(".semaprax-patch.lock")));
                }
                Ok(())
            },
        )
        .unwrap();
        assert_no_a0_artifacts(&source_path);
        std::fs::remove_dir_all(directory).unwrap();
    }

    #[cfg(unix)]
    #[test]
    fn evidence_v2_apply_preserves_source_permissions() {
        use std::os::unix::fs::{MetadataExt, PermissionsExt};

        let source = "module evidence.v2_permissions;\n@id(\"evidence.helper\") fn helper()->i64{1}\n@id(\"app.main\") fn main()->i64{helper()}\n";
        let patch_source = format!(
            "schema semaprax.semantic-patch.v2\nbase {}\nrename evidence.helper to renamed\nrequire no-new-effects\n",
            graph::revision(&parse(source, Path::new("evidence.spx")).unwrap())
        );
        let (directory, source_path, patch_path, evidence_path) = fixture(source, &patch_source);
        std::fs::set_permissions(&source_path, std::fs::Permissions::from_mode(0o640)).unwrap();
        let before = std::fs::metadata(&source_path).unwrap();
        let capsule = generate_v2(&source_path, &patch_path).unwrap();
        std::fs::write(&evidence_path, capsule).unwrap();
        apply_v2(&source_path, &patch_path, &evidence_path).unwrap();
        let after = std::fs::metadata(&source_path).unwrap();
        assert_eq!(after.mode() & 0o777, before.mode() & 0o777);
        assert_eq!(after.uid(), before.uid());
        assert_eq!(after.gid(), before.gid());
        assert_no_a0_artifacts(&source_path);
        std::fs::remove_dir_all(directory).unwrap();
    }
}
