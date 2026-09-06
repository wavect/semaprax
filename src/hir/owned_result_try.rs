//! Resolution helpers for compiler-owned Result `?`.

use super::{DeclarationId, OwnershipMode, ResolvedExpr, ResolvedExprKind, ResolvedType};

pub(super) fn is_exact_owned_instance(operand: &ResolvedType, residual: &ResolvedType) -> bool {
    matches!(
        operand,
        ResolvedType::Nominal {
            declaration,
            arguments,
        } if declaration.as_str() == crate::prelude::RESULT_ID
            && matches!(arguments.as_slice(), [ResolvedType::Bytes,
                ResolvedType::Bytes | ResolvedType::I64 | ResolvedType::I32 | ResolvedType::U8
                    | ResolvedType::Usize | ResolvedType::Char | ResolvedType::F32
                    | ResolvedType::F64 | ResolvedType::Bool
                    | ResolvedType::TypeParameter { .. }])
            && operand == residual
    )
}

pub(super) fn ownership_for(operand: &ResolvedType, residual: &ResolvedType) -> OwnershipMode {
    if is_exact_owned_instance(operand, residual) {
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
