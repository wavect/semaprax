//! Callee-scoped admission for the checked native owner family.
//! This decision comes from HIR import/type facts, never an attached plan.
use crate::hir::{
    OwnershipMode, ResolvedExpr, ResolvedExprKind, ResolvedImportResultKind, ResolvedProgram,
};

pub(super) const STATUS_DOMAIN: &str = "semaprax.native-rust-owner-admission.v1";
pub(super) const REFUSED: u32 = 7;

pub(super) fn required(program: &ResolvedProgram, expression: &ResolvedExpr) -> bool {
    let Some(call) = super::native_rust::parts(expression) else {
        return false;
    };
    if matches!(expression.kind, ResolvedExprKind::Call { .. })
        && program
            .resolve_call_target(call.callee, call.instance)
            .is_none()
    {
        return false;
    }
    call.args.iter().any(|argument| {
        argument.ownership == OwnershipMode::Own
            && program
                .interfaces
                .iter()
                .flat_map(|i| &i.imports)
                .any(|import| {
                    import.native_rust
                        && matches!(
                            import.result.kind,
                            ResolvedImportResultKind::OwnedResource { .. }
                                | ResolvedImportResultKind::OwnedString
                                | ResolvedImportResultKind::OwnedOptionString
                                | ResolvedImportResultKind::OwnedResultStringI64
                                | ResolvedImportResultKind::OwnedResultStringOptionI64
                        )
                        && import
                            .result
                            .kind
                            .value_type(&program.declarations)
                            .ok()
                            .as_ref()
                            == Some(&argument.ty)
                })
    })
}
