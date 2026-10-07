//! Resolution helpers for compiler-owned Result `?`.

use super::{DeclarationId, OwnershipMode, ResolvedExpr, ResolvedExprKind, ResolvedType};

pub(super) fn is_exact_owned_instance(operand: &ResolvedType, residual: &ResolvedType) -> bool {
    let ResolvedType::Nominal {
        declaration,
        arguments,
    } = operand
    else {
        return false;
    };
    let payload = |ty: &ResolvedType| {
        *ty == ResolvedType::Bytes
            || crate::hir::is_scalar_resolved_type(ty)
            || matches!(ty, ResolvedType::TypeParameter { .. })
    };
    let ResolvedType::Nominal {
        declaration: residual_declaration,
        arguments: residual_arguments,
    } = residual
    else {
        return false;
    };
    declaration.as_str() == crate::prelude::RESULT_ID
        && residual_declaration == declaration
        && matches!((arguments.as_slice(), residual_arguments.as_slice()),
            ([ok, error], [residual_ok, residual_error])
                if error == residual_error
                    && (*ok == ResolvedType::Bytes || *error == ResolvedType::Bytes)
                    && (*residual_ok == ResolvedType::Bytes || *residual_error == ResolvedType::Bytes)
                    && payload(ok) && payload(error) && payload(residual_ok))
}

pub(super) fn ownership_for(operand: &ResolvedType, residual: &ResolvedType) -> OwnershipMode {
    if is_exact_owned_instance(operand, residual)
        && matches!(operand, ResolvedType::Nominal { arguments, .. } if arguments[0] == ResolvedType::Bytes)
    {
        OwnershipMode::Own
    } else {
        OwnershipMode::Value
    }
}

pub(super) fn resolve_result(
    operand: ResolvedExpr,
    residual_type: ResolvedType,
    ok_type: &ResolvedType,
) -> (ResolvedExprKind, ResolvedType, OwnershipMode) {
    let ownership = ownership_for(&operand.ty, &residual_type);
    (
        ResolvedExprKind::Try {
            operand: Box::new(operand),
            result: DeclarationId::new(crate::prelude::RESULT_ID),
            ok_case: DeclarationId::new(crate::prelude::RESULT_OK_ID),
            ok_field: DeclarationId::new(crate::prelude::RESULT_OK_VALUE_ID),
            err_case: DeclarationId::new(crate::prelude::RESULT_ERR_ID),
            err_field: DeclarationId::new(crate::prelude::RESULT_ERR_ERROR_ID),
            residual_type,
        },
        ok_type.clone(),
        ownership,
    )
}
