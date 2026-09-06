//! Closed interpreter lowering for owned `Result<Bytes, E>` postfix `?`.

use super::*;

pub(super) const ESCAPED_RESIDUAL_GUARD: &str =
    "owned postfix `?` residual escaped its function frame";

pub(super) fn scan_is_admitted(
    declarations: &hir::DeclarationIndex,
    expression: &ResolvedExpr,
) -> bool {
    let ResolvedExprKind::Try {
        operand,
        result,
        ok_case,
        ok_field,
        err_case,
        err_field,
        residual_type,
    } = &expression.kind
    else {
        return false;
    };
    expression.ownership == hir::OwnershipMode::Own
        && expression.ty == ResolvedType::Bytes
        && operand.ty == *residual_type
        && is_admitted_owned_byte_variant(declarations, &operand.ty)
        && result.as_str() == crate::prelude::RESULT_ID
        && ok_case.as_str() == crate::prelude::RESULT_OK_ID
        && ok_field.as_str() == crate::prelude::RESULT_OK_VALUE_ID
        && err_case.as_str() == crate::prelude::RESULT_ERR_ID
        && err_field.as_str() == crate::prelude::RESULT_ERR_ERROR_ID
}

impl Evaluator<'_> {
    pub(super) fn evaluate_owned_try(
        &mut self,
        expression: &ResolvedExpr,
        environment: &mut Environment,
        depth: usize,
    ) -> Result<Value, Flow> {
        let ResolvedExprKind::Try {
            operand,
            result,
            ok_case,
            ok_field,
            err_case,
            err_field,
            residual_type,
        } = &expression.kind
        else {
            return Err(Flow::Guard(
                "owned postfix `?` helper received a non-Try expression",
            ));
        };
        let ResolvedType::Nominal {
            declaration: source_result,
            arguments: source_arguments,
        } = &operand.ty
        else {
            return Err(Flow::Guard("owned postfix `?` operand is not Result"));
        };
        let ResolvedType::Nominal {
            declaration: target_result,
            arguments: target_arguments,
        } = residual_type
        else {
            return Err(Flow::Guard("owned postfix `?` residual is not Result"));
        };
        if result.as_str() != crate::prelude::RESULT_ID
            || source_result != result
            || target_result != result
            || !matches!(
                source_arguments.as_slice(),
                [
                    ResolvedType::Bytes,
                    ResolvedType::Bytes
                        | ResolvedType::I64
                        | ResolvedType::I32
                        | ResolvedType::U8
                        | ResolvedType::Usize
                        | ResolvedType::Char
                        | ResolvedType::F32
                        | ResolvedType::F64
                        | ResolvedType::Bool
                ]
            )
            || source_arguments != target_arguments
            || expression.ty != ResolvedType::Bytes
            || ok_case.as_str() != crate::prelude::RESULT_OK_ID
            || ok_field.as_str() != crate::prelude::RESULT_OK_VALUE_ID
            || err_case.as_str() != crate::prelude::RESULT_ERR_ID
            || err_field.as_str() != crate::prelude::RESULT_ERR_ERROR_ID
        {
            return Err(Flow::Guard(
                "owned postfix `?` metadata is outside its authenticated Result profile",
            ));
        }
        let Value::Variant(carrier) = self.evaluate(operand, environment, depth)? else {
            return Err(Flow::Guard(
                "owned postfix `?` operand is not a variant carrier",
            ));
        };
        if carrier.ty != operand.ty || carrier.variant != *result {
            return Err(Flow::Guard(
                "owned postfix `?` carrier identity disagrees with its operand",
            ));
        }
        let mut carrier = Arc::try_unwrap(carrier)
            .map_err(|_| Flow::Guard("owned postfix `?` carrier still has a live alias"))?;
        let selected_field = if carrier.case == *ok_case {
            ok_field
        } else if carrier.case == *err_case {
            err_field
        } else {
            return Err(Flow::Guard("owned postfix `?` carrier has an invalid tag"));
        };
        if carrier.fields.len() != 1 {
            return Err(Flow::Guard(
                "owned postfix `?` carrier payload inventory is inconsistent",
            ));
        }
        let payload = carrier.fields.remove(selected_field).ok_or(Flow::Guard(
            "owned postfix `?` carrier omits its selected payload",
        ))?;
        let payload_type = if carrier.case == *ok_case {
            &source_arguments[0]
        } else {
            &source_arguments[1]
        };
        let payload_matches = matches!(
            (payload_type, &payload),
            (ResolvedType::Bytes, Value::Bytes(_))
                | (ResolvedType::I64, Value::Int(_))
                | (ResolvedType::Bool, Value::Bool(_))
                | (ResolvedType::I32, Value::Int32(_))
                | (ResolvedType::U8, Value::Uint8(_))
                | (ResolvedType::Usize, Value::Usize(_))
                | (ResolvedType::Char, Value::Char(_))
                | (ResolvedType::F32, Value::Float32(_))
                | (ResolvedType::F64, Value::Float64(_))
        );
        if !payload_matches || !carrier.fields.is_empty() {
            return Err(Flow::Guard(
                "owned postfix `?` selected payload does not match its authenticated case type",
            ));
        }
        if carrier.case == *ok_case {
            Ok(payload)
        } else {
            let mut fields = BTreeMap::new();
            fields.insert(err_field.clone(), payload);
            Err(Flow::Residual(Value::Variant(Arc::new(
                OwnedVariantValue {
                    ty: residual_type.clone(),
                    variant: result.clone(),
                    case: err_case.clone(),
                    fields,
                },
            ))))
        }
    }
}
