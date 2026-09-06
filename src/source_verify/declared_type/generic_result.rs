//! Bounded generic Result ownership, independently checked before HIR.
use super::*;
use crate::ast::Statement;

pub(in crate::source_verify) fn slot(function: &Function, ty: &Type) -> bool {
    let [parameter] = function.type_parameters.as_slice() else {
        return false;
    };
    matches!(ty, Type::Named { name, arguments }
        if name == "Result" && matches!(arguments.as_slice(), [Type::Bytes, Type::Named { name, arguments }] | [Type::Named { name, arguments }, Type::Bytes]
            if arguments.is_empty() && name == &parameter.name))
}

pub(in crate::source_verify) fn profile(function: &Function) -> bool {
    slot(function, &function.return_type)
        && function
            .params
            .iter()
            .filter(|p| p.mode == ParamMode::Own)
            .count()
            == 1
        && function.params.iter().all(|p| {
            (p.mode == ParamMode::Own && p.ty == function.return_type)
                || (p.mode == ParamMode::Value && matches!(p.ty, Type::I64 | Type::Bool))
        })
}

pub(in crate::source_verify) fn arguments(function: &Function, arguments: &[Type]) -> bool {
    if copy_success(function) && arguments == [Type::Bytes] {
        return false;
    }
    matches!(
        arguments,
        [Type::I64
            | Type::I32
            | Type::Char
            | Type::U8
            | Type::Usize
            | Type::F32
            | Type::F64
            | Type::Bool
            | Type::Bytes]
    )
}

pub(in crate::source_verify) fn body(function: &Function, expression: &Expr) -> bool {
    if !profile(function) {
        return false;
    }
    if generic_function_expression_is_direct_scalar(expression) {
        return true;
    }
    match &expression.kind {
        ExprKind::Block { statements, tail } => statements.iter().all(
            |statement| matches!(statement, Statement::Let { value, .. } if body(function, value)),
        ) && body(function, tail),
        ExprKind::Try { operand } => generic_function_expression_is_direct_scalar(operand),
        ExprKind::ConstructVariant {
            type_name,
            type_arguments,
            case_name,
            fields,
            ..
        } => {
            type_name == "Result"
                && case_name == "Ok"
                && fields.len() == 1
                && fields[0].name == "value"
                && matches!(&function.return_type, Type::Named { arguments, .. } if arguments == type_arguments)
                && (generic_function_expression_is_direct_scalar(&fields[0].value)
                    || matches!(&fields[0].value.kind, ExprKind::Try { operand } if generic_function_expression_is_direct_scalar(operand)))
        }
        _ => false,
    }
}

pub(in crate::source_verify) fn copy_success(function: &Function) -> bool {
    matches!(&function.return_type, Type::Named { arguments, .. } if arguments.first().is_some_and(|ty| *ty != Type::Bytes))
}

pub(in crate::source_verify) fn substitutions(function: &Function) -> Vec<Vec<Type>> {
    let mut substitutions = owned_record_function_substitutions(1);
    if !copy_success(function) {
        substitutions.push(vec![Type::Bytes]);
    }
    substitutions
}

pub(in crate::source_verify) fn concrete_try(operand: &Type, result: &Type) -> bool {
    operand == result
        && matches!(ordinary_result_arguments(operand), Some((ok, error))
        if (*ok == Type::Bytes || *error == Type::Bytes)
            && owned_byte_prelude_instance_is_admitted("Result", &[ok.clone(), error.clone()]))
}
