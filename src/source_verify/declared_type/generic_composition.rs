//! Closed source grammar for flat generic owned-record expression composition.

use crate::ast::{
    Expr, ExprKind, Function, MatchMode, MatchPattern, Param, ParamMode, RecordMatchFieldPattern,
    Statement, Type,
};
use crate::source_verify::type_table::TypeTable;
use std::collections::HashSet;

use super::{
    generic_function_expression_is_direct_scalar,
    generic_function_has_exact_nested_owned_record_relay,
};

pub(super) fn is_admitted(function: &Function, types: &TypeTable<'_>, expression: &Expr) -> bool {
    let Some(owner) = exact_flat_owner(function, types) else {
        return false;
    };
    let mut record_roots = HashSet::from([owner.name.clone()]);
    match &expression.kind {
        ExprKind::Block { statements, tail } => {
            statements.iter().all(|statement| match statement {
                Statement::Let { name, value, .. } => {
                    let admitted = admitted_root(function, types, owner, &record_roots, value);
                    if admitted && matches!(value.kind, ExprKind::UpdateRecord { .. }) {
                        record_roots.insert(name.clone());
                    }
                    admitted
                }
                _ => false,
            }) && admitted_root(function, types, owner, &record_roots, tail)
        }
        _ => admitted_root(function, types, owner, &record_roots, expression),
    }
}

fn exact_flat_owner<'a>(function: &'a Function, types: &TypeTable<'_>) -> Option<&'a Param> {
    if !generic_function_has_exact_nested_owned_record_relay(function, types) {
        return None;
    }
    let owner = function
        .params
        .iter()
        .find(|parameter| parameter.mode == ParamMode::Own)?;
    let parameters = function
        .type_parameters
        .iter()
        .map(|parameter| parameter.name.as_str())
        .collect();
    types
        .is_flat_owned_byte_record_template(&owner.ty, &parameters)
        .then_some(owner)
}

fn admitted_root(
    function: &Function,
    types: &TypeTable<'_>,
    owner: &Param,
    record_roots: &HashSet<String>,
    expression: &Expr,
) -> bool {
    if generic_function_expression_is_direct_scalar(expression) {
        return true;
    }
    match &expression.kind {
        ExprKind::Project { base, field, .. } => {
            matches!(&base.kind, ExprKind::Var(name) if name == &owner.name)
                && copy_field(function, types, &owner.ty, field)
        }
        ExprKind::UpdateRecord { base, fields } => {
            matches!(&base.kind, ExprKind::Var(name) if name == &owner.name)
                && !fields.is_empty()
                && fields.iter().enumerate().all(|(index, field)| {
                    fields[..index]
                        .iter()
                        .all(|earlier| earlier.name != field.name)
                        && copy_field(function, types, &owner.ty, &field.name)
                        && generic_function_expression_is_direct_scalar(&field.value)
                })
        }
        ExprKind::Match {
            mode: MatchMode::Borrow,
            scrutinee,
            arms,
        } => borrow_copy_match(function, types, owner, scrutinee, arms),
        ExprKind::Match {
            mode: MatchMode::Own,
            scrutinee,
            arms,
        } => own_reconstruction(function, types, owner, record_roots, scrutinee, arms),
        _ => false,
    }
}

fn copy_field(function: &Function, types: &TypeTable<'_>, record: &Type, name: &str) -> bool {
    types
        .record_fields(record)
        .and_then(|fields| fields.iter().find(|field| field.name == name))
        .and_then(|field| types.record_field_type(record, field))
        .is_some_and(|field| match &field {
            Type::I64
            | Type::I32
            | Type::Char
            | Type::U8
            | Type::Usize
            | Type::F32
            | Type::F64
            | Type::Bool => true,
            Type::Named { name, arguments } => {
                arguments.is_empty()
                    && function
                        .type_parameters
                        .iter()
                        .any(|parameter| parameter.name == *name)
            }
            _ => false,
        })
}

fn borrow_copy_match(
    function: &Function,
    types: &TypeTable<'_>,
    owner: &Param,
    scrutinee: &Expr,
    arms: &[crate::ast::MatchArm],
) -> bool {
    let [arm] = arms else { return false };
    if arm.guard.is_some() || !matches!(&scrutinee.kind, ExprKind::Var(name) if name == &owner.name)
    {
        return false;
    }
    let MatchPattern::Record {
        type_name, fields, ..
    } = &arm.pattern
    else {
        return false;
    };
    let ExprKind::Var(result) = &arm.value.kind else {
        return false;
    };
    if record_name(&owner.ty) != Some(type_name.as_str())
        || !exact_pattern_fields(types, &owner.ty, fields)
        || fields
            .iter()
            .any(|field| matches!(&field.pattern, RecordMatchFieldPattern::Record { .. }))
    {
        return false;
    }
    let mut bindings = fields.iter().filter_map(|field| match &field.pattern {
        RecordMatchFieldPattern::Binding { name, .. } => Some((field, name)),
        RecordMatchFieldPattern::Wildcard { .. } | RecordMatchFieldPattern::Record { .. } => None,
    });
    let Some((field, name)) = bindings.next() else {
        return false;
    };
    bindings.next().is_none()
        && name == result
        && copy_field(function, types, &owner.ty, &field.name)
}

fn own_reconstruction(
    function: &Function,
    types: &TypeTable<'_>,
    owner: &Param,
    record_roots: &HashSet<String>,
    scrutinee: &Expr,
    arms: &[crate::ast::MatchArm],
) -> bool {
    let [arm] = arms else { return false };
    if arm.guard.is_some()
        || !matches!(&scrutinee.kind, ExprKind::Var(name) if record_roots.contains(name))
    {
        return false;
    }
    let MatchPattern::Record {
        type_name, fields, ..
    } = &arm.pattern
    else {
        return false;
    };
    let ExprKind::ConstructRecord {
        type_name: built,
        type_arguments,
        fields: initializers,
        ..
    } = &arm.value.kind
    else {
        return false;
    };
    record_name(&owner.ty) == Some(type_name.as_str())
        && built == type_name
        && matches!(&owner.ty, Type::Named { arguments, .. } if arguments == type_arguments)
        && exact_pattern_fields(types, &owner.ty, fields)
        && exact_initializer_fields(types, &owner.ty, initializers)
        && initializers.iter().all(|initializer| {
            let Some(pattern) = fields.iter().find(|field| field.name == initializer.name) else {
                return false;
            };
            let RecordMatchFieldPattern::Binding { name: binding, .. } = &pattern.pattern else {
                return false;
            };
            if copy_field(function, types, &owner.ty, &initializer.name) {
                generic_function_expression_is_direct_scalar(&initializer.value)
                    && !references_owned_field_binding(
                        function,
                        types,
                        &owner.ty,
                        fields,
                        &initializer.value,
                    )
            } else {
                matches!(&initializer.value.kind, ExprKind::Var(value) if value == binding)
            }
        })
}

fn references_owned_field_binding(
    function: &Function,
    types: &TypeTable<'_>,
    record: &Type,
    fields: &[crate::ast::RecordMatchPatternField],
    expression: &Expr,
) -> bool {
    let mut pending = vec![expression];
    while let Some(expression) = pending.pop() {
        if let ExprKind::Var(name) = &expression.kind {
            if fields.iter().any(|field| {
                !copy_field(function, types, record, &field.name)
                    && matches!(&field.pattern,
                        RecordMatchFieldPattern::Binding { name: binding, .. }
                            if binding == name)
            }) {
                return true;
            }
        }
        let mut index = 0;
        while let Some(child) = expression.child(index) {
            pending.push(child);
            index += 1;
        }
    }
    false
}

fn exact_initializer_fields(
    types: &TypeTable<'_>,
    record: &Type,
    fields: &[crate::ast::FieldInitializer],
) -> bool {
    let Some(declared) = types.record_fields(record) else {
        return false;
    };
    fields.len() == declared.len()
        && declared.iter().all(|item| {
            fields
                .iter()
                .filter(|field| field.name == item.name)
                .count()
                == 1
        })
}

fn exact_pattern_fields(
    types: &TypeTable<'_>,
    record: &Type,
    fields: &[crate::ast::RecordMatchPatternField],
) -> bool {
    let Some(declared) = types.record_fields(record) else {
        return false;
    };
    fields.len() == declared.len()
        && declared.iter().all(|item| {
            fields
                .iter()
                .filter(|field| field.name == item.name)
                .count()
                == 1
        })
}

fn record_name(ty: &Type) -> Option<&str> {
    match ty {
        Type::Named { name, .. } => Some(name),
        _ => None,
    }
}
