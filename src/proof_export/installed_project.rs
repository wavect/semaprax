//! Real installed proofs for exact retained Project postconditions.
use super::installed::{InstalledProofTool, ToolKind};
use crate::assurance_manifest::{
    smt_discharge as smt, AssuranceClass, MethodRecord, VerifiedProjectProof,
};
use crate::{diagnostic::Diagnostic, project::ProjectRevision};

/// The returned evidence cannot be retargeted, serialized into authority or
/// attached to another source/Project. No application entry point is executed.
pub fn prove_postcondition(
    revision: &ProjectRevision,
    source_path: &str,
    declaration: &str,
    index: usize,
    tool: &InstalledProofTool,
) -> Result<VerifiedProjectProof, Vec<Diagnostic>> {
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
        return Err(error(
            "proof declaration is outside the exact selected Project HIR",
        ));
    }
    let source = revision
        .sources()
        .iter()
        .find(|source| source.path() == source_path)
        .ok_or_else(|| error("proof source is absent from exact retained Project"))?;
    if source.source_graph_schema() == "semaprax.native-law.v1" {
        return Err(error(
            "native relational law attachment is not a source postcondition",
        ));
    }
    let program = crate::parse(source.source(), source_path).map_err(|error| vec![error])?;
    crate::hir::resolve(&program)?;
    if tool.kind() == ToolKind::Lean {
        let certificate = super::source_certificate(
            source.source(),
            std::path::Path::new(source_path),
            declaration,
            index,
            tool,
        )?;
        let binding = super::bind_certificate_to_program_root(&certificate, revision, source_path)
            .map_err(|error| vec![error])?;
        return super::assurance_method_attachment(&certificate, &binding, revision, tool)
            .map_err(|error| vec![error]);
    }
    let function = program
        .functions
        .iter()
        .find(|function| function.stable_id == declaration)
        .ok_or_else(|| error("exact selected declaration is absent"))?;
    let encoding = smt::translate_function(function)
        .map_err(|_| error("declaration is outside the admitted SMT source profile"))?;
    if index >= encoding.ensures.len() {
        return Err(error("exact selected postcondition is absent"));
    }
    let domain_script = smt::render_domain_witness_script(&encoding, tool.proof_timeout_ms());
    let domain_model = tool
        .smt_domain_model(&domain_script)
        .map_err(|error| vec![error])?;
    smt::validate_domain_witness(function, &domain_model)
        .map_err(|reason| error(&format!("domain witness failed checked replay: {reason}")))?;
    let domain_witness = domain_model
        .iter()
        .map(|(name, value)| {
            let value = match value {
                smt::ModelValue::Int(raw) => format!("int:{raw}"),
                smt::ModelValue::Bool(raw) => format!("bool:{raw}"),
            };
            (name.clone(), value)
        })
        .collect::<std::collections::BTreeMap<_, _>>();
    // Same existing checked-arithmetic translation. Model retrieval is a
    // separate command and unnecessary for strict success-only proof checking.
    let rendered = smt::render_postcondition_script(&encoding, index, tool.proof_timeout_ms());
    let script = rendered
        .strip_suffix("(get-model)\n")
        .ok_or_else(|| error("unexpected SMT translator response grammar"))?;
    tool.confirm_smt(script).map_err(|error| vec![error])?;
    let version = tool.expected_version();
    let root = revision.program_root()?;
    let obligation = smt::postcondition_obligation_id(declaration, index);
    let receipt = serde_json::json!({
        "schema":"semaprax.installed-smt-project-proof.v1", "project_revision":revision.project_revision(),
        "program_root":root.program_root(), "source_path":source.path(),
        "source_revision":source.source_revision(), "source_digest":source.source_digest(),
        "declaration_id":declaration, "obligation_id":obligation,
        "script":script, "domain_script":domain_script, "domain_witness":domain_witness,
        "toolchain":version, "profile":smt::BOUNDS_V1,
        "host_profile":"trusted_local", "proved_lowering":false
    });
    let digest = super::certificate::domain_digest(
        b"semaprax.installed-smt-project-proof.v1\0",
        receipt.to_string().as_bytes(),
    );
    let mut method = MethodRecord::new(AssuranceClass::SmtProved, "z3", version);
    method.proof_ref = Some(digest.clone());
    method.bounds = Some(smt::BOUNDS_V1.into());
    method.inputs = vec![root.program_root().into(), source.source_digest().into()];
    method.detail = Some("Exact retained source checked by explicitly authorized installed Z3; trusted translation and local host, no proved lowering or execution authority".into());
    Ok(VerifiedProjectProof::kernel_confirmed(
        obligation,
        declaration.into(),
        method,
        revision.project_revision().into(),
        root.program_root().into(),
        source.path().into(),
        source.source_revision().into(),
        source.source_digest().into(),
        digest,
    ))
}

fn error(message: &str) -> Vec<Diagnostic> {
    vec![Diagnostic::io("SPX-LW140", message)]
}
