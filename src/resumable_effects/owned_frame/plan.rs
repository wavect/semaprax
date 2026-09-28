use crate::cleanup_plan::OwnedFrameLiveness;
use crate::diagnostic::Diagnostic;
use crate::hir::{self, DeclarationId, ResolvedProgram, ResolvedType};
use sha2::{Digest, Sha256};
use std::sync::Arc;

pub(crate) const PROFILE: &str = "semaprax.source-owned-frame.v1";
/// Immutable compiler proof. No constructor accepts host-authored plan metadata.
#[derive(Clone)]
pub(crate) struct CheckedOwnedFramePlan {
    pub(in crate::resumable_effects) program: Arc<ResolvedProgram>,
    pub(in crate::resumable_effects) function: DeclarationId,
    pub(in crate::resumable_effects) binding: String,
    pub(in crate::resumable_effects) liveness: OwnedFrameLiveness,
}
impl CheckedOwnedFramePlan {
    pub(crate) fn binding(&self) -> &str {
        &self.binding
    }
    pub(crate) fn program(&self) -> &ResolvedProgram {
        &self.program
    }
    pub(crate) fn function(&self) -> &hir::ResolvedFunction {
        self.program
            .functions
            .iter()
            .find(|f| f.id == self.function)
            .expect("sealed checked function")
    }
    pub(crate) fn liveness(&self) -> &OwnedFrameLiveness {
        &self.liveness
    }
}
pub(crate) fn compile_owned_frame_plan(
    program: &ResolvedProgram,
    function: &DeclarationId,
) -> Result<CheckedOwnedFramePlan, Diagnostic> {
    hir::validate(program)?;
    let declaration = program
        .declarations
        .declaration(function)
        .ok_or_else(|| Diagnostic::io("SPX-T303", "owned frame function missing"))?;
    if declaration.identity_origin != hir::IdentityOrigin::Explicit || declaration.owner.is_some() {
        return Err(Diagnostic::io(
            "SPX-T303",
            "owned frame requires persistent function identity",
        ));
    }
    let entry = program
        .functions
        .iter()
        .find(|f| f.id == *function)
        .ok_or_else(|| {
            Diagnostic::io("SPX-T303", "owned frame requires an ordinary free function")
        })?;
    let liveness = crate::cleanup_plan::owned_frame_liveness(&program.declarations, entry)?;
    let ResolvedType::Nominal {
        declaration: nominal,
        ..
    } = &entry.params[0].ty
    else {
        unreachable!("checked owned record")
    };
    let fields = program
        .declarations
        .record_fields(nominal)
        .expect("checked fields");
    if [
        function.as_str(),
        nominal.as_str(),
        entry.params[0].id.as_str(),
        liveness.site.as_str(),
    ]
    .into_iter()
    .any(|id| id.len() > 256)
        || fields.iter().any(|field| field.id.as_str().len() > 256)
        || !matches!(
            entry.cleanup_plan.schema,
            "semaprax.cleanup-plan.v2"
                | "semaprax.cleanup-plan.v3"
                | "semaprax.cleanup-plan.v4"
                | "semaprax.cleanup-plan.v5"
                | "semaprax.cleanup-plan.v6"
                | "semaprax.cleanup-plan.v7"
                | "semaprax.cleanup-plan.v8"
                | "semaprax.cleanup-plan.v9"
                | "semaprax.cleanup-plan.v10"
                | "semaprax.cleanup-plan.v11"
                | "semaprax.cleanup-plan.v12"
                | "semaprax.cleanup-plan.v13"
        )
    {
        return Err(Diagnostic::io(
            "SPX-T303",
            "owned frame identity bounds or cleanup schema outside pinned profile",
        ));
    }
    let graph = crate::graph::to_hir_json(program, "semaprax.source-owned-frame-plan.v1")?;
    let mut hash = Sha256::new();
    hash.update(b"semaprax.source-owned-frame-plan.v1\0");
    hash.update(PROFILE.as_bytes());
    hash.update(function.as_str().as_bytes());
    hash.update(graph.as_bytes());
    hash.update(crate::graph_cleanup::cleanup_plan_json(&entry.cleanup_plan).as_bytes());
    let mut binding = String::from("sha256:");
    for byte in hash.finalize() {
        use std::fmt::Write;
        write!(binding, "{byte:02x}").unwrap();
    }
    Ok(CheckedOwnedFramePlan {
        program: Arc::new(program.clone()),
        function: function.clone(),
        binding,
        liveness,
    })
}

#[cfg(test)]
mod tests;
