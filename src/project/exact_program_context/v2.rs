//! Additive exact context binding ProgramRoot v3 and contract/test facts.
//!
//! V2 independently replays the retained v1 context and both new typed
//! products. It stores descriptors only and grants no authority.

use std::sync::Arc;

use serde_json::{json, Map, Value};
use sha2::{Digest, Sha256};

use crate::diagnostic::Diagnostic;

use super::ExactProgramContext;
use crate::project::{
    ContractsAndTestsFacts, ProgramRoot, ProgramRootV2, ProgramRootV3, ProjectRevision,
};

pub const EXACT_PROGRAM_CONTEXT_V2_SCHEMA: &str = "semaprax.exact-program-context.v2";
pub const MAX_EXACT_PROGRAM_CONTEXT_V2_BYTES: usize = 96 * 1024;

const CONTEXT_V2_DOMAIN: &[u8] = b"semaprax.exact-program-context.digest.v2\0";
const NONCLAIMS: [&str; 5] = [
    "additive_successor_of_exact_program_context_v1",
    "retained_products_are_descriptors_not_embedded_payloads",
    "contracts_and_tests_facts_are_not_coverage_proof_or_execution_results",
    "program_root_v1_v2_v3_and_context_v1_bytes_are_unchanged",
    "no_filesystem_network_process_execution_deployment_commit_or_publication_authority",
];

type Result<T> = std::result::Result<T, Vec<Diagnostic>>;

pub struct ExactProgramContextV2 {
    context_v1: Arc<ExactProgramContext>,
    contracts_and_tests_facts: ContractsAndTestsFacts,
    program_root_v3: ProgramRootV3,
    context_v2_digest: String,
    json: String,
}

impl ExactProgramContextV2 {
    /// Freshly derive the additive facts and root from an exact v1 context,
    /// then independently replay every retained product.
    pub fn assemble(context_v1: Arc<ExactProgramContext>) -> Result<Self> {
        let facts = ContractsAndTestsFacts::derive(
            Arc::clone(context_v1.revision()),
            context_v1.project_revision(),
        )?;
        let v3 = ProgramRootV3::derive(
            context_v1.semantic_workspace(),
            context_v1.base_project_root(),
            context_v1.interface_artifact_facts(),
            context_v1.dependency_lock_association(),
            &facts,
        )?;
        let workspace = context_v1
            .semantic_workspace()
            .workspace_revision()
            .to_owned();
        let digest = v3.program_root_v3_digest().to_owned();
        Self::derive(context_v1, facts, v3, &workspace, &digest)
    }

    /// Admit a candidate exact generation only after independently replaying
    /// both the retained current context and the caller-supplied candidate
    /// context. The candidate context's Project must be byte-identical to the
    /// separately admitted candidate revision. Its retained external facts are
    /// freshly supplied and replayed, never implicitly copied from the current
    /// generation. Their selection policy may change and is bound by the new
    /// context identity.
    ///
    /// This operation retains no snapshot, path, lock, cache, or publication
    /// authority. A host that needs fresh Project Lock association material
    /// must assemble `candidate_context` through its ordinary authenticated
    /// snapshot boundary before calling this method.
    pub fn refresh_candidate(
        current: &Arc<Self>,
        candidate_revision: &Arc<ProjectRevision>,
        candidate_context: Arc<Self>,
    ) -> Result<Arc<Self>> {
        replay_retained_context(current)?;
        let replayed_candidate = replay_retained_context(&candidate_context)?;
        if !same_project_revision(
            candidate_revision,
            replayed_candidate.exact_program_context_v1().revision(),
        ) {
            return Err(stale(
                "candidate exact context does not match the separately admitted Project revision",
            ));
        }
        Ok(Arc::new(replayed_candidate))
    }

    /// Construct v2 only after exact selector checks and independent replay of
    /// v1 context, facts, and ProgramRoot v3.
    pub fn derive(
        context_v1: Arc<ExactProgramContext>,
        contracts_and_tests_facts: ContractsAndTestsFacts,
        program_root_v3: ProgramRootV3,
        expected_workspace_revision: &str,
        expected_program_root_v3_digest: &str,
    ) -> Result<Self> {
        validate_selectors(
            &context_v1,
            &program_root_v3,
            expected_workspace_revision,
            expected_program_root_v3_digest,
        )?;
        let replayed_v1 = replay_v1(&context_v1)?;
        if replayed_v1.context_digest() != context_v1.context_digest()
            || replayed_v1.to_json() != context_v1.to_json()
        {
            return Err(stale(
                "exact context v2 retained v1 context failed exact replay",
            ));
        }
        let replayed_facts = ContractsAndTestsFacts::replay(
            Arc::clone(replayed_v1.revision()),
            replayed_v1.project_revision(),
            contracts_and_tests_facts.facts_digest(),
            contracts_and_tests_facts.to_json().as_bytes(),
        )?;
        if replayed_facts != contracts_and_tests_facts {
            return Err(stale(
                "exact context v2 contract/test facts failed exact replay",
            ));
        }
        let replayed_v3 = ProgramRootV3::replay(
            replayed_v1.semantic_workspace(),
            replayed_v1.base_project_root(),
            replayed_v1.interface_artifact_facts(),
            replayed_v1.dependency_lock_association(),
            &replayed_facts,
            expected_program_root_v3_digest,
            program_root_v3.to_json().as_bytes(),
        )?;
        if replayed_v3 != program_root_v3 {
            return Err(stale("exact context v2 ProgramRoot v3 failed exact replay"));
        }

        let payload = json!({
            "contracts_and_tests_facts_digest": replayed_facts.facts_digest(),
            "exact_program_context_v1_digest": replayed_v1.context_digest(),
            "limits": {"max_context_v2_bytes": MAX_EXACT_PROGRAM_CONTEXT_V2_BYTES},
            "nonclaims": NONCLAIMS,
            "program_root_v1_digest": replayed_v1.semantic_workspace_root().program_root_digest(),
            "program_root_v2_digest": replayed_v1.program_root_v2().program_root_v2_digest(),
            "program_root_v3_digest": replayed_v3.program_root_v3_digest(),
            "project_revision": replayed_v1.project_revision(),
            "schema": EXACT_PROGRAM_CONTEXT_V2_SCHEMA,
            "semantic_workspace_revision": replayed_v1.semantic_workspace().workspace_revision(),
        });
        let identity = canonical_json(payload.clone())?;
        let context_v2_digest = framed_digest(CONTEXT_V2_DOMAIN, identity.as_bytes());
        let mut value = payload;
        value
            .as_object_mut()
            .expect("context v2 payload is object")
            .insert("context_v2_digest".to_owned(), json!(context_v2_digest));
        let json = canonical_json(value)?;
        if json.len() > MAX_EXACT_PROGRAM_CONTEXT_V2_BYTES {
            return Err(invalid("exact program context v2 exceeds its byte limit"));
        }
        Ok(Self {
            context_v1: Arc::new(replayed_v1),
            contracts_and_tests_facts: replayed_facts,
            program_root_v3: replayed_v3,
            context_v2_digest,
            json,
        })
    }

    /// Replay submitted v2 descriptor bytes after all typed selectors and
    /// retained products have independently replayed.
    #[allow(clippy::too_many_arguments)]
    pub fn replay(
        context_v1: Arc<ExactProgramContext>,
        contracts_and_tests_facts: ContractsAndTestsFacts,
        program_root_v3: ProgramRootV3,
        expected_workspace_revision: &str,
        expected_program_root_v3_digest: &str,
        expected_context_v2_digest: &str,
        bytes: &[u8],
    ) -> Result<Self> {
        validate_selectors(
            &context_v1,
            &program_root_v3,
            expected_workspace_revision,
            expected_program_root_v3_digest,
        )?;
        validate_digest(expected_context_v2_digest)?;
        if bytes.len() > MAX_EXACT_PROGRAM_CONTEXT_V2_BYTES {
            return Err(invalid(
                "submitted exact program context v2 exceeds its byte limit",
            ));
        }
        let source = std::str::from_utf8(bytes)
            .map_err(|_| invalid("submitted exact program context v2 is not UTF-8"))?;
        let value: Value = serde_json::from_str(source)
            .map_err(|_| invalid("submitted exact program context v2 is not JSON"))?;
        if canonical_json(value.clone())?.as_bytes() != bytes {
            return Err(invalid(
                "submitted exact program context v2 is not canonical JSON",
            ));
        }
        validate_wire_shape(&value)?;
        let derived = Self::derive(
            context_v1,
            contracts_and_tests_facts,
            program_root_v3,
            expected_workspace_revision,
            expected_program_root_v3_digest,
        )?;
        if expected_context_v2_digest != derived.context_v2_digest()
            || bytes != derived.to_json().as_bytes()
        {
            return Err(stale("exact program context v2 failed exact replay"));
        }
        Ok(derived)
    }

    pub fn select(
        &self,
        expected_workspace_revision: &str,
        expected_program_root_v3_digest: &str,
    ) -> Result<&ProgramRootV3> {
        validate_selectors(
            &self.context_v1,
            &self.program_root_v3,
            expected_workspace_revision,
            expected_program_root_v3_digest,
        )?;
        Ok(&self.program_root_v3)
    }

    pub fn exact_program_context_v1(&self) -> &ExactProgramContext {
        &self.context_v1
    }
    pub fn exact_program_context_v1_arc(&self) -> &Arc<ExactProgramContext> {
        &self.context_v1
    }
    pub fn base_program_root_v1(&self) -> &ProgramRoot {
        self.context_v1.base_project_root()
    }
    pub fn semantic_workspace_program_root_v1(&self) -> &ProgramRoot {
        self.context_v1.semantic_workspace_root()
    }
    pub fn program_root_v2(&self) -> &ProgramRootV2 {
        self.context_v1.program_root_v2()
    }
    pub fn contracts_and_tests_facts(&self) -> &ContractsAndTestsFacts {
        &self.contracts_and_tests_facts
    }
    pub fn program_root_v3(&self) -> &ProgramRootV3 {
        &self.program_root_v3
    }
    pub fn context_v2_digest(&self) -> &str {
        &self.context_v2_digest
    }
    pub fn to_json(&self) -> &str {
        &self.json
    }
}

fn replay_retained_context(context: &ExactProgramContextV2) -> Result<ExactProgramContextV2> {
    ExactProgramContextV2::replay(
        Arc::clone(context.exact_program_context_v1_arc()),
        context.contracts_and_tests_facts().clone(),
        context.program_root_v3().clone(),
        context
            .exact_program_context_v1()
            .semantic_workspace()
            .workspace_revision(),
        context.program_root_v3().program_root_v3_digest(),
        context.context_v2_digest(),
        context.to_json().as_bytes(),
    )
}

fn same_project_revision(left: &ProjectRevision, right: &ProjectRevision) -> bool {
    left.project_revision() == right.project_revision()
        && left.workspace_revision() == right.workspace_revision()
        && left.manifest().to_canonical_toml() == right.manifest().to_canonical_toml()
        && left.workspace_manifest() == right.workspace_manifest()
        && left.semantic_graph() == right.semantic_graph()
        && left.sources().len() == right.sources().len()
        && left
            .sources()
            .iter()
            .zip(right.sources())
            .all(|(left, right)| {
                left.path() == right.path()
                    && left.source() == right.source()
                    && left.source_revision() == right.source_revision()
                    && left.source_digest() == right.source_digest()
            })
}

fn replay_v1(context: &ExactProgramContext) -> Result<ExactProgramContext> {
    ExactProgramContext::derive(
        Arc::clone(context.revision()),
        context.project_revision(),
        context.semantic_workspace().clone(),
        context.semantic_workspace().workspace_revision(),
        context.interface_artifact_facts().clone(),
        context.dependency_lock_association().clone(),
        context.program_root_v2().clone(),
        context.program_root_v2().program_root_v2_digest(),
    )
}

fn validate_selectors(
    context: &ExactProgramContext,
    v3: &ProgramRootV3,
    workspace: &str,
    v3_digest: &str,
) -> Result<()> {
    validate_digest(workspace)?;
    validate_digest(v3_digest)?;
    if workspace != context.semantic_workspace().workspace_revision()
        || workspace != v3.semantic_workspace_revision()
        || v3_digest != v3.program_root_v3_digest()
        || v3.program_root_v2_digest() != context.program_root_v2().program_root_v2_digest()
    {
        return Err(stale(
            "exact context v2 requires matching workspace and ProgramRoot v3 selectors",
        ));
    }
    Ok(())
}

fn validate_wire_shape(value: &Value) -> Result<()> {
    let object = value
        .as_object()
        .ok_or_else(|| invalid("exact program context v2 must be an object"))?;
    exact_fields(
        object,
        &[
            "context_v2_digest",
            "contracts_and_tests_facts_digest",
            "exact_program_context_v1_digest",
            "limits",
            "nonclaims",
            "program_root_v1_digest",
            "program_root_v2_digest",
            "program_root_v3_digest",
            "project_revision",
            "schema",
            "semantic_workspace_revision",
        ],
    )?;
    if value["schema"] != EXACT_PROGRAM_CONTEXT_V2_SCHEMA
        || value["limits"] != json!({"max_context_v2_bytes": MAX_EXACT_PROGRAM_CONTEXT_V2_BYTES})
        || value["nonclaims"] != json!(NONCLAIMS)
    {
        return Err(invalid("exact program context v2 fixed fields are invalid"));
    }
    for field in [
        "context_v2_digest",
        "contracts_and_tests_facts_digest",
        "exact_program_context_v1_digest",
        "program_root_v1_digest",
        "program_root_v2_digest",
        "program_root_v3_digest",
        "project_revision",
        "semantic_workspace_revision",
    ] {
        validate_digest(
            value[field]
                .as_str()
                .ok_or_else(|| invalid("exact program context v2 digest field is invalid"))?,
        )?;
    }
    let mut identity = value.clone();
    identity
        .as_object_mut()
        .expect("checked object")
        .remove("context_v2_digest");
    if value["context_v2_digest"]
        != framed_digest(CONTEXT_V2_DOMAIN, canonical_json(identity)?.as_bytes())
    {
        return Err(invalid(
            "exact program context v2 identity is internally inconsistent",
        ));
    }
    Ok(())
}

fn exact_fields(object: &Map<String, Value>, fields: &[&str]) -> Result<()> {
    if object.len() != fields.len() || !fields.iter().all(|field| object.contains_key(*field)) {
        return Err(invalid(
            "exact program context v2 contains missing or unknown fields",
        ));
    }
    Ok(())
}

fn canonical_json(mut value: Value) -> Result<String> {
    value.sort_all_objects();
    let mut output = serde_json::to_string(&value)
        .map_err(|_| invalid("exact program context v2 could not be serialized"))?;
    output.push('\n');
    Ok(output)
}

fn validate_digest(value: &str) -> Result<()> {
    if value.len() != 71
        || !value.starts_with("sha256:")
        || !value.as_bytes()[7..]
            .iter()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(byte))
    {
        return Err(invalid(
            "exact program context v2 requires a lowercase sha256 digest",
        ));
    }
    Ok(())
}

fn framed_digest(domain: &[u8], bytes: &[u8]) -> String {
    let mut digest = Sha256::new();
    digest.update(domain);
    digest.update((bytes.len() as u64).to_le_bytes());
    digest.update(bytes);
    format!(
        "sha256:{:x}",
        crate::digest_hex::LowerHex(digest.finalize())
    )
}

fn invalid(message: impl Into<String>) -> Vec<Diagnostic> {
    vec![Diagnostic::io("SPX-G576", message)]
}
fn stale(message: impl Into<String>) -> Vec<Diagnostic> {
    vec![Diagnostic::io("SPX-G577", message)]
}
