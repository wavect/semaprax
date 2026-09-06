//! Validation-only authentication for compiler-owned Result `?`.

use super::*;

pub(super) fn expression_owns_exact_operand(expression: &ResolvedExpr) -> bool {
    expression.ownership == OwnershipMode::Own
        && matches!(
            &expression.ty,
            ResolvedType::Nominal {
                declaration,
                arguments,
            } if declaration.as_str() == crate::prelude::RESULT_ID
                && arguments.as_slice() == [ResolvedType::Bytes, ResolvedType::Bytes]
        )
}

pub(super) struct ValidatedResultTry<'a> {
    pub(super) operand_arguments: &'a [ResolvedType],
    pub(super) residual_arguments: &'a [ResolvedType],
}

#[allow(clippy::too_many_arguments)]
pub(super) fn validate_shape<'a>(
    expression: &ResolvedExpr,
    operand: &'a ResolvedExpr,
    result: &DeclarationId,
    ok_case: &DeclarationId,
    ok_field: &DeclarationId,
    err_case: &DeclarationId,
    err_field: &DeclarationId,
    residual_type: &'a ResolvedType,
) -> Result<ValidatedResultTry<'a>, Diagnostic> {
    if result.as_str() != crate::prelude::RESULT_ID
        || ok_case.as_str() != crate::prelude::RESULT_OK_ID
        || ok_field.as_str() != crate::prelude::RESULT_OK_VALUE_ID
        || err_case.as_str() != crate::prelude::RESULT_ERR_ID
        || err_field.as_str() != crate::prelude::RESULT_ERR_ERROR_ID
    {
        return Err(hir_error(
            "resolved `?` does not authenticate the compiler-owned Result shape",
        ));
    }
    let (
        ResolvedType::Nominal {
            declaration: operand_result,
            arguments: operand_arguments,
        },
        ResolvedType::Nominal {
            declaration: residual_result,
            arguments: residual_arguments,
        },
    ) = (&operand.ty, residual_type)
    else {
        return Err(hir_error(
            "resolved `?` operand or residual is not nominal Result",
        ));
    };
    let exact_owned = operand_arguments.as_slice() == [ResolvedType::Bytes, ResolvedType::Bytes]
        && residual_arguments.as_slice() == [ResolvedType::Bytes, ResolvedType::Bytes];
    if operand_result != result
        || residual_result != result
        || operand_arguments.len() != 2
        || residual_arguments.len() != 2
        || (!exact_owned
            && operand_arguments
                .iter()
                .chain(residual_arguments)
                .any(|argument| !matches!(argument, ResolvedType::I64 | ResolvedType::Bool)))
    {
        return Err(hir_error(
            "resolved `?` has invalid concrete Result instances",
        ));
    }
    let expected_ownership = if exact_owned {
        OwnershipMode::Own
    } else {
        OwnershipMode::Value
    };
    if expression.ownership != expected_ownership {
        return Err(hir_error(
            "resolved `?` success value has inconsistent ownership",
        ));
    }
    Ok(ValidatedResultTry {
        operand_arguments,
        residual_arguments,
    })
}
