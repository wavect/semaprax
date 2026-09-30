//! Inert helper proof only. Agent association/binding identity belongs to the
//! checked frontend successor; this does not invent its v2 wire digest.
use crate::cleanup_plan::OwnedFrameLiveness;
use crate::diagnostic::Diagnostic;
use crate::hir::{self, DeclarationId, ResolvedFunction, ResolvedProgram};
use std::sync::Arc;
#[derive(Clone)]
pub(crate) struct CheckedOwnedFrameHelperV2 {
    program: Arc<ResolvedProgram>,
    function: DeclarationId,
    liveness: OwnedFrameLiveness,
}
impl CheckedOwnedFrameHelperV2 {
    pub(crate) fn same_helper(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.program, &other.program) && self.function == other.function
    }
    pub(crate) fn program(&self) -> &ResolvedProgram {
        &self.program
    }
    pub(crate) fn function(&self) -> &ResolvedFunction {
        self.program
            .functions
            .iter()
            .find(|f| f.id == self.function)
            .expect("sealed helper")
    }
    pub(crate) fn liveness(&self) -> &OwnedFrameLiveness {
        &self.liveness
    }
}
pub(crate) fn compile_owned_frame_helper_v2(
    program: &ResolvedProgram,
    function: &DeclarationId,
) -> Result<CheckedOwnedFrameHelperV2, Diagnostic> {
    hir::validate(program)?;
    let declaration = program
        .declarations
        .declaration(function)
        .ok_or_else(|| Diagnostic::io("SPX-T303", "owned v2 helper missing"))?;
    if declaration.identity_origin != hir::IdentityOrigin::Explicit || declaration.owner.is_some() {
        return Err(Diagnostic::io(
            "SPX-T303",
            "owned v2 requires persistent free helper",
        ));
    }
    let entry = program
        .functions
        .iter()
        .find(|f| f.id == *function)
        .ok_or_else(|| Diagnostic::io("SPX-T303", "owned v2 requires ordinary helper"))?;
    let liveness = crate::cleanup_plan::owned_frame_v2_liveness(&program.declarations, entry)?;
    if function.as_str().len() > 256
        || entry.params.iter().any(|p| p.id.as_str().len() > 256)
        || liveness.site.as_str().len() > 256
    {
        return Err(Diagnostic::io(
            "SPX-T303",
            "owned v2 helper identity bounds",
        ));
    }
    for ty in [
        &entry.params[0].ty,
        &entry.params[1].ty,
        &entry.yields.as_ref().expect("checked yields").response_type,
    ] {
        let hir::ResolvedType::Nominal { declaration, .. } = ty else {
            unreachable!("checked records")
        };
        if declaration.as_str().len() > 256
            || program
                .declarations
                .record_fields(declaration)
                .expect("checked fields")
                .iter()
                .any(|f| f.id.as_str().len() > 256)
        {
            return Err(Diagnostic::io(
                "SPX-T303",
                "owned v2 record identity bounds",
            ));
        }
    }
    Ok(CheckedOwnedFrameHelperV2 {
        program: Arc::new(program.clone()),
        function: function.clone(),
        liveness,
    })
}
