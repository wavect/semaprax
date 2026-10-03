//! Registered installed Z3 transport for exact modular summary queries.
//! No direct process execution occurs along this selected Project route.
use crate::assurance_manifest::smt_discharge::{self as smt, DischargeOutcome};
use crate::ast::Function;
use crate::project::ProjectRevision;
use crate::proof_export::installed::{InstalledProofTool, ToolKind};

use super::summary::{prove_with, ModularFailure, ModularProof};

fn discharge(
    function: &Function,
    index: usize,
    tool: &InstalledProofTool,
) -> Result<DischargeOutcome, String> {
    if tool.kind() != ToolKind::Z3 {
        return Err("Z3 capability required".into());
    }
    let encoding = smt::translate_function(function).map_err(|reason| reason.detail())?;
    if index >= encoding.ensures.len() {
        return Err("postcondition index is absent".into());
    }
    let domain_script = smt::render_domain_witness_script(&encoding, tool.proof_timeout_ms());
    let model = tool
        .smt_domain_model(&domain_script)
        .map_err(|error| error.message)?;
    smt::validate_domain_witness(function, &model)
        .map_err(|reason| format!("domain witness failed checked replay: {reason}"))?;
    let rendered = smt::render_postcondition_script(&encoding, index, tool.proof_timeout_ms());
    let script = rendered
        .strip_suffix("(get-model)\n")
        .ok_or_else(|| "unexpected SMT translator response grammar".to_owned())?;
    tool.confirm_smt(script).map_err(|error| error.message)?;
    Ok(DischargeOutcome::Proved {
        script_digest: smt::script_digest(&rendered),
        solver_identity: "z3",
        solver_version: tool.expected_version().to_owned(),
    })
}

/// Every callee, staged precondition and caller query runs under the same
/// explicit held process capability used by existing LAW-04 installed proofs.
pub fn prove_straight_line_installed(
    revision: &ProjectRevision,
    target: &str,
    tool: &InstalledProofTool,
) -> Result<ModularProof, ModularFailure> {
    if !tool.is_modular_scalar() {
        return Err(ModularFailure::CalleeProof(
            "explicit modular scalar process budget required".into(),
        ));
    }
    prove_with(revision, target, |function, index| {
        discharge(function, index, tool)
    })
}
