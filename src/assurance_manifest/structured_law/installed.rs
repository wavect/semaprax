//! Source-bound LAW-07 proof and replay for one retained Project postcondition.
//! A certificate is inert JSON. Only a fresh installed-tool confirmation can
//! create the opaque proof accepted by Project and strict-law reports.

use serde::{Deserialize, Serialize};

use crate::assurance_manifest::{
    modular_law::cache::{self, ProofTaskCache, WorkMetrics},
    smt_discharge as smt, AssuranceClass, MethodRecord, VerifiedProjectProof,
};
use crate::diagnostic::Diagnostic;
use crate::project::ProjectRevision;
use crate::proof_export::{
    installed::{InstalledProofTool, ToolKind},
    kernel_report, lean, LeanKernel,
};

use super::{lower, lower_aggregate_clause, Lowered, Refusal, PROFILE};

pub const CERTIFICATE_SCHEMA: &str = "semaprax.structured-law-project-certificate.v1";
pub const SMT_PROFILE: &str = "semaprax.structured-law-smt-v1: finite immutable aggregate scalarization plus checked SMT-LIB QF_LIA; exact retained Project and installed Z3";
pub const LEAN_PROFILE: &str = "semaprax.structured-law-lean-v1: finite immutable aggregate scalarization plus pinned Lean simp_all/omega; exact retained Project and installed Lean";
const CERTIFICATE_DOMAIN: &[u8] = b"semaprax.structured-law-project-certificate.v1\0";
const QUERY_DOMAIN: &[u8] = b"semaprax.structured-law-query.v1\0";

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FieldBinding {
    pub parameter: String,
    pub declaration_id: Option<String>,
    pub field_path: Vec<String>,
    pub scalar_type: String,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Backend {
    Z3,
    Lean,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Certificate {
    pub schema: String,
    pub compiler_version: String,
    pub project_revision: String,
    pub program_root: String,
    pub source_path: String,
    pub source_revision: String,
    pub source_digest: String,
    pub declaration_id: String,
    pub ensures_index: usize,
    pub obligation_id: String,
    pub scalar_revision: String,
    pub field_bindings: Vec<FieldBinding>,
    pub dependency_ids: Vec<String>,
    pub backend: Backend,
    pub toolchain: String,
    pub profile: String,
    pub query_digest: String,
    pub domain_query_digest: Option<String>,
    pub domain_witness: String,
    pub trust_boundary: String,
    pub backend_coverage: Vec<String>,
    pub nonclaims: Vec<String>,
}

fn refused(detail: impl Into<String>) -> Vec<Diagnostic> {
    vec![Diagnostic::io(
        "SPX-LW140",
        format!("structured law refused: {}", detail.into()),
    )]
}

fn subject_refusal(reason: Refusal) -> Vec<Diagnostic> {
    refused(format!("{}: {reason:?}", reason.code()))
}

fn digest(bytes: &[u8]) -> String {
    crate::proof_export::certificate::domain_digest(QUERY_DOMAIN, bytes)
}

struct Subject {
    lowered: Lowered,
    certificate: Certificate,
    query: String,
    domain_query: Option<String>,
    lean_theorems: Vec<String>,
}

fn subject(
    revision: &ProjectRevision,
    source_path: &str,
    declaration: &str,
    index: usize,
    tool: &InstalledProofTool,
) -> Result<Subject, Vec<Diagnostic>> {
    let selected = [
        revision.entry_program(),
        revision.public_api_program(),
        revision.test_program(),
    ]
    .iter()
    .any(|program| {
        program
            .functions
            .iter()
            .any(|function| function.id.as_str() == declaration)
    });
    if !selected {
        return Err(refused("declaration is outside exact selected Project HIR"));
    }
    let source = revision
        .sources()
        .iter()
        .find(|row| row.path() == source_path)
        .ok_or_else(|| refused("source is absent from exact retained Project"))?;
    if source.source_graph_schema() == "semaprax.native-law.v1" {
        return Err(refused(
            "native relational law is not a source postcondition",
        ));
    }
    let program = crate::parse(source.source(), source_path).map_err(|error| vec![error])?;
    let function = program
        .functions
        .iter()
        .find(|function| function.stable_id == declaration)
        .ok_or_else(|| refused("declaration is absent from exact selected source"))?;
    if index >= function.ensures.len() {
        return Err(refused("postcondition index is absent"));
    }
    let lowered = if matches!(
        &function.return_type,
        crate::ast::Type::I64
            | crate::ast::Type::I32
            | crate::ast::Type::U8
            | crate::ast::Type::Usize
            | crate::ast::Type::Bool
    ) {
        lower(&program, function)
    } else {
        lower_aggregate_clause(&program, function, index)
    }
    .map_err(subject_refusal)?;
    let lowered_index = lowered.source_ensures_index.map_or(index, |_| 0);
    let mut scalar = program.clone();
    scalar.module_uses.clear();
    scalar.types.clear();
    scalar.interfaces.clear();
    scalar.protocols.clear();
    scalar.implementations.clear();
    scalar.session_protocols.clear();
    scalar.agents.clear();
    scalar.functions = vec![lowered.scalar.clone()];
    let scalar_revision = crate::graph::revision(&scalar);
    let (backend, query, domain_query, lean_theorems, profile, coverage) = match tool.kind() {
        ToolKind::Z3 => {
            let encoding = smt::translate_function(&lowered.scalar)
                .map_err(|reason| refused(format!("scalar SMT subset: {}", reason.code())))?;
            let rendered =
                smt::render_postcondition_script(&encoding, lowered_index, tool.proof_timeout_ms());
            let script = rendered
                .strip_suffix("(get-model)\n")
                .ok_or_else(|| refused("unexpected SMT translator response grammar"))?;
            (
                Backend::Z3,
                script.to_owned(),
                Some(smt::render_domain_witness_script(
                    &encoding,
                    tool.proof_timeout_ms(),
                )),
                Vec::new(),
                SMT_PROFILE,
                vec![
                    "structured_postcondition".into(),
                    "checked_arithmetic".into(),
                    "satisfiable_requires_domain".into(),
                ],
            )
        }
        ToolKind::Lean => {
            let export = lean::export_structured_module(&scalar, &scalar_revision);
            if !export.unsupported.is_empty() || export.exported.len() != 1 {
                return Err(refused(
                    "scalarized declaration is outside pinned Lean structured profile",
                ));
            }
            (
                Backend::Lean,
                export.lean_source.clone(),
                None,
                export.theorem_names(),
                LEAN_PROFILE,
                vec![
                    "structured_postcondition".into(),
                    "all_declaration_range_obligations".into(),
                    "bounded_checked_requires_witness".into(),
                ],
            )
        }
    };
    let root = revision.program_root()?;
    let binding = lowered
        .leaves
        .iter()
        .map(|leaf| FieldBinding {
            parameter: leaf.parameter.clone(),
            declaration_id: leaf.declaration_id.clone(),
            field_path: leaf.field_path.clone(),
            scalar_type: leaf.ty.to_string(),
        })
        .collect();
    let certificate = Certificate {
        schema: CERTIFICATE_SCHEMA.into(),
        compiler_version: env!("CARGO_PKG_VERSION").into(),
        project_revision: revision.project_revision().into(),
        program_root: root.program_root().into(),
        source_path: source.path().into(),
        source_revision: source.source_revision().into(),
        source_digest: source.source_digest().into(),
        declaration_id: declaration.into(),
        ensures_index: index,
        obligation_id: smt::postcondition_obligation_id(declaration, index),
        scalar_revision,
        field_bindings: binding,
        dependency_ids: lowered.declaration_ids.clone(),
        backend,
        toolchain: tool.expected_version().into(),
        profile: profile.into(),
        query_digest: digest(query.as_bytes()),
        domain_query_digest: domain_query.as_ref().map(|query| digest(query.as_bytes())),
        domain_witness: match tool.kind() {
            ToolKind::Z3 => "installed_z3_model_checked_against_scalar_requires",
            ToolKind::Lean => "bounded_checked_scalar_requires_witness",
        }
        .into(),
        trust_boundary: "trusted_local_installed_tool_and_unverified_compiler_translation".into(),
        backend_coverage: coverage,
        nonclaims: vec![
            "no_backend_lowering_proof".into(),
            "no_execution_or_publication_authority".into(),
            "no_database_atomicity_claim".into(),
        ],
    };
    Ok(Subject {
        lowered,
        certificate,
        query,
        domain_query,
        lean_theorems,
    })
}

fn confirm(subject: &Subject, tool: &InstalledProofTool) -> Result<(), Vec<Diagnostic>> {
    match subject.certificate.backend {
        Backend::Z3 => {
            let model = tool
                .smt_domain_model(subject.domain_query.as_deref().expect("Z3 domain query"))
                .map_err(|error| vec![error])?;
            smt::validate_domain_witness(&subject.lowered.scalar, &model).map_err(|reason| {
                refused(format!("domain model failed checked replay: {reason}"))
            })?;
            tool.confirm_smt(&subject.query)
                .map_err(|error| vec![error])
        }
        Backend::Lean => {
            let witness =
                smt::bounded_domain_witness(&subject.lowered.scalar, 256).ok_or_else(|| {
                    refused("precondition domain is unknown under bounded checked replay")
                })?;
            smt::validate_domain_witness(&subject.lowered.scalar, &witness)
                .map_err(|reason| refused(format!("bounded domain witness failed: {reason}")))?;
            let run = tool.check(&subject.query).map_err(|error| vec![error])?;
            if !matches!(
                kernel_report::parse(&subject.lean_theorems, &run.toolchain, &run.output),
                kernel_report::KernelVerdict::Checked { .. }
            ) {
                return Err(refused(
                    "pinned Lean kernel did not confirm all structured obligations",
                ));
            }
            Ok(())
        }
    }
}

fn attached(subject: &Subject, certificate: &str) -> VerifiedProjectProof {
    let row = &subject.certificate;
    let proof_ref =
        crate::proof_export::certificate::domain_digest(CERTIFICATE_DOMAIN, certificate.as_bytes());
    let mut method = MethodRecord::new(
        match row.backend {
            Backend::Z3 => AssuranceClass::SmtProved,
            Backend::Lean => AssuranceClass::TheoremProved,
        },
        match row.backend {
            Backend::Z3 => "z3",
            Backend::Lean => crate::proof_export::KERNEL_IDENTITY,
        },
        match row.backend {
            Backend::Z3 => row.toolchain.clone(),
            Backend::Lean => crate::proof_export::PINNED_TOOLCHAIN.into(),
        },
    );
    method.proof_ref = Some(proof_ref.clone());
    method.bounds = Some(row.profile.clone());
    if row.backend == Backend::Lean {
        method.assumption_ids = crate::proof_export::ASSUMPTIONS
            .iter()
            .map(|(id, _)| (*id).into())
            .collect();
    }
    method.inputs = vec![
        row.program_root.clone(),
        row.source_digest.clone(),
        row.scalar_revision.clone(),
        row.query_digest.clone(),
        row.toolchain.clone(),
    ];
    method.detail = Some(format!("LAW-07 exact retained source and field/case identities; {}; coverage={}; no proved lowering, execution or publication authority", row.trust_boundary, row.backend_coverage.join(",")));
    VerifiedProjectProof::kernel_confirmed(
        row.obligation_id.clone(),
        row.declaration_id.clone(),
        method,
        row.project_revision.clone(),
        row.program_root.clone(),
        row.source_path.clone(),
        row.source_revision.clone(),
        row.source_digest.clone(),
        proof_ref,
    )
}

/// Execute the exact installed backend and return inert replayable proof data
/// plus an opaque exact-Project attachment for this invocation only.
pub fn prove_installed_project(
    revision: &ProjectRevision,
    source_path: &str,
    declaration: &str,
    index: usize,
    tool: &InstalledProofTool,
) -> Result<(String, VerifiedProjectProof), Vec<Diagnostic>> {
    let subject = subject(revision, source_path, declaration, index, tool)?;
    confirm(&subject, tool)?;
    let certificate = serde_json::to_string(&subject.certificate)
        .map_err(|_| refused("certificate serialization failed"))?;
    let proof = attached(&subject, &certificate);
    Ok((certificate, proof))
}

fn checked_logical_subject(subject: &Subject) -> Result<(String, String), Vec<Diagnostic>> {
    let row = &subject.certificate;
    let bindings = serde_json::to_string(&row.field_bindings)
        .map_err(|_| refused("field dependency encoding failed"))?;
    let dependencies = serde_json::to_string(&row.dependency_ids)
        .map_err(|_| refused("declaration dependency encoding failed"))?;
    let theorem_names = serde_json::to_string(&subject.lean_theorems)
        .map_err(|_| refused("theorem inventory encoding failed"))?;
    let coverage = serde_json::to_string(&row.backend_coverage)
        .map_err(|_| refused("backend coverage encoding failed"))?;
    let nonclaims = serde_json::to_string(&row.nonclaims)
        .map_err(|_| refused("proof boundary encoding failed"))?;
    let axioms = match row.backend {
        Backend::Z3 => "none".into(),
        Backend::Lean => serde_json::to_string(&lean::ASSUMPTIONS)
            .map_err(|_| refused("Lean assumption inventory encoding failed"))?,
    };
    let index = row.ensures_index.to_string();
    let logical = cache::logical_subject_digest(&[
        "structured-law",
        &row.declaration_id,
        &index,
        &row.scalar_revision,
        &bindings,
        &dependencies,
        &row.query_digest,
        row.domain_query_digest.as_deref().unwrap_or("none"),
        &row.domain_witness,
        &theorem_names,
        &coverage,
        &nonclaims,
    ]);
    Ok((logical, axioms))
}

/// Rebuild the exact current source-bound certificate and Project attachment,
/// while reusing a previously checked logical LAW-07 query only when the
/// lowered subject, every field/case dependency, backend profile, kernel
/// assumptions, installed tool and process bounds still match.
pub fn prove_installed_project_cached(
    project_root: &std::path::Path,
    revision: &ProjectRevision,
    source_path: &str,
    declaration: &str,
    index: usize,
    tool: &InstalledProofTool,
    cache: &mut ProofTaskCache,
) -> Result<(String, VerifiedProjectProof, WorkMetrics), Vec<Diagnostic>> {
    let subject = subject(revision, source_path, declaration, index, tool)?;
    let (logical, axioms) = checked_logical_subject(&subject)?;
    let role = match subject.certificate.backend {
        Backend::Z3 => "structured-z3",
        Backend::Lean => "structured-lean",
    };
    let owner = format!("{declaration}:{index}");
    let work = cache::check_bound_task(
        cache,
        project_root,
        tool,
        role,
        &owner,
        &logical,
        &subject.certificate.query_digest,
        &subject.certificate.profile,
        &axioms,
        || confirm(&subject, tool),
    )?;
    let certificate = serde_json::to_string(&subject.certificate)
        .map_err(|_| refused("certificate serialization failed"))?;
    let proof = attached(&subject, &certificate);
    cache.record_event(revision, role, &owner, work);
    Ok((certificate, proof, work))
}

/// Reparse exact retained source, recompute every field/case/query identity,
/// and rerun the installed backend. A structurally valid certificate alone
/// can never create `VerifiedProjectProof`.
pub fn replay_installed_project(
    document: &str,
    revision: &ProjectRevision,
    tool: &InstalledProofTool,
) -> Result<VerifiedProjectProof, Vec<Diagnostic>> {
    if document.len() > 262_144 {
        return Err(refused("certificate exceeds byte bound"));
    }
    let certificate: Certificate =
        serde_json::from_str(document).map_err(|_| refused("certificate schema is invalid"))?;
    if certificate.schema != CERTIFICATE_SCHEMA
        || serde_json::to_string(&certificate).ok().as_deref() != Some(document)
    {
        return Err(refused("certificate is noncanonical or has wrong schema"));
    }
    let subject = subject(
        revision,
        &certificate.source_path,
        &certificate.declaration_id,
        certificate.ensures_index,
        tool,
    )?;
    if certificate != subject.certificate {
        return Err(refused(
            "certificate source, Project, field/case, query or backend identity drifted",
        ));
    }
    confirm(&subject, tool)?;
    Ok(attached(&subject, document))
}
