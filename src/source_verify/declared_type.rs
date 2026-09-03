//! Declaration-level checks reached from `verify`: declared-type admission,
//! record layout recursion, generic function substitution, ownership-mode
//! rules, and record pattern checking.

use super::binding::{Availability, Binding};
use super::diagnostics::{error, source_identifier};
use super::type_table::{
    effective_record_fields, owned_byte_prelude_instance_is_admitted, TypeTable,
};
use crate::ast::{
    Expr, ExprKind, FieldDeclaration, Function, MatchMode, Param, ParamMode, Program,
    RecordMatchFieldPattern, RecordMatchPatternField, Span, Type, TypeDeclarationKind,
};
use crate::diagnostic::Diagnostic;
use std::collections::{BTreeSet, HashMap, HashSet};

pub(super) fn native_rust_status_domain(value: &str) -> bool {
    let bytes = value.as_bytes();
    (2..=128).contains(&bytes.len())
        && (bytes[0].is_ascii_lowercase() || bytes[0].is_ascii_digit())
        && (bytes[bytes.len() - 1].is_ascii_lowercase() || bytes[bytes.len() - 1].is_ascii_digit())
        && bytes.iter().all(|byte| {
            byte.is_ascii_lowercase() || byte.is_ascii_digit() || matches!(byte, b'.' | b'-')
        })
}

pub(super) fn record_layout_is_recursive(
    name: &str,
    types: &TypeTable<'_>,
    visiting: &mut HashSet<String>,
    checked: &mut HashSet<String>,
) -> bool {
    enum Frame<'a> {
        Enter(&'a str),
        Fields {
            name: &'a str,
            fields: &'a [FieldDeclaration],
            parameters: HashSet<&'a str>,
            index: usize,
        },
    }

    let mut frames = vec![Frame::Enter(name)];
    let mut results = Vec::new();
    while let Some(frame) = frames.pop() {
        match frame {
            Frame::Enter(name) => {
                if checked.contains(name) {
                    results.push(false);
                    continue;
                }
                if !visiting.insert(name.to_owned()) {
                    results.push(true);
                    continue;
                }
                let Some(declaration) = types.declaration(name) else {
                    visiting.remove(name);
                    checked.insert(name.to_owned());
                    results.push(false);
                    continue;
                };
                let (TypeDeclarationKind::Record { fields }
                | TypeDeclarationKind::Class { fields, .. }) = &declaration.kind
                else {
                    visiting.remove(name);
                    checked.insert(name.to_owned());
                    results.push(false);
                    continue;
                };
                let parameters = declaration
                    .type_parameters
                    .iter()
                    .map(|parameter| parameter.name.as_str())
                    .collect::<HashSet<_>>();
                frames.push(Frame::Fields {
                    name,
                    fields,
                    parameters,
                    index: 0,
                });
            }
            Frame::Fields {
                name,
                fields,
                parameters,
                mut index,
            } => {
                if results.pop().unwrap_or(false) {
                    visiting.remove(name);
                    results.push(true);
                    continue;
                }
                let mut child = None;
                while let Some(field) = fields.get(index) {
                    index += 1;
                    let Type::Named {
                        name: field_type,
                        arguments,
                    } = &field.ty
                    else {
                        continue;
                    };
                    if arguments.is_empty() && parameters.contains(field_type.as_str()) {
                        continue;
                    }
                    if matches!(
                        types.declaration(field_type).map(|item| &item.kind),
                        Some(
                            TypeDeclarationKind::Record { .. } | TypeDeclarationKind::Class { .. }
                        )
                    ) {
                        child = Some(field_type.as_str());
                        break;
                    }
                }
                if let Some(child) = child {
                    frames.push(Frame::Fields {
                        name,
                        fields,
                        parameters,
                        index,
                    });
                    frames.push(Frame::Enter(child));
                } else {
                    visiting.remove(name);
                    checked.insert(name.to_owned());
                    results.push(false);
                }
            }
        }
    }
    results.pop().unwrap_or(false)
}

pub(super) fn check_declared_type(
    program: &Program,
    ty: &Type,
    span: Span,
    types: &TypeTable<'_>,
    parameters: &HashSet<&str>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let mut pending = vec![ty];
    while let Some(ty) = pending.pop() {
        let Type::Named { name, arguments } = ty else {
            continue;
        };
        if parameters.contains(name.as_str()) {
            if !arguments.is_empty() {
                diagnostics.push(error(
                    program,
                    "SPX-T220",
                    format!("type parameter `{name}` cannot take type arguments"),
                    span,
                ));
            }
            continue;
        }
        let Some(declaration) = types.declaration(name) else {
            let (code, message) = if parameters.is_empty() {
                (
                    "SPX-T001",
                    format!("unknown type `{name}`; declare it with `resource {name};`"),
                )
            } else {
                (
                    "SPX-T220",
                    format!("`{name}` is not an in-scope type parameter"),
                )
            };
            diagnostics.push(error(program, code, message, span));
            continue;
        };
        if arguments.len() != declaration.type_parameters.len() {
            diagnostics.push(error(
                program,
                "SPX-T221",
                format!(
                    "type `{name}` expects {} type arguments, received {}",
                    declaration.type_parameters.len(),
                    arguments.len()
                ),
                span,
            ));
        }
        if arguments
            .iter()
            .any(|argument| matches!(argument, Type::ArrayU8(_)))
            || (arguments.contains(&Type::Bytes)
                && !owned_byte_prelude_instance_is_admitted(name, arguments))
        {
            diagnostics.push(error(
                program,
                "SPX-T268",
                "fixed arrays and non-admitted `Bytes` carriers are not admitted as generic arguments",
                span,
            ));
            pending.extend(arguments.iter().rev());
            continue;
        }
        if !arguments.is_empty()
            && !owned_byte_prelude_instance_is_admitted(name, arguments)
            && (!matches!(
                declaration.kind,
                TypeDeclarationKind::Record { .. }
                    | TypeDeclarationKind::Class { .. }
                    | TypeDeclarationKind::Variant { .. }
            ) || arguments
                .iter()
                .any(|argument| !matches!(argument, Type::I64 | Type::Bool)))
        {
            diagnostics.push(error(
                program,
                "SPX-T223",
                format!("generic copy type `{name}` accepts only direct `i64` or `bool` arguments"),
                span,
            ));
        }
        pending.extend(arguments.iter().rev());
    }
}

pub(super) fn direct_function_type_argument(ty: &Type) -> bool {
    matches!(ty, Type::I64 | Type::Bool)
}

pub(super) fn generic_function_signature_slot(ty: &Type, parameters: &HashSet<&str>) -> bool {
    match ty {
        Type::I64 | Type::Bool | Type::String => true,
        Type::I32
        | Type::Char
        | Type::U8
        | Type::Usize
        | Type::ArrayU8(_)
        | Type::F32
        | Type::F64
        | Type::Bytes
        | Type::Str
        | Type::SliceU8 => false,
        Type::Named { name, arguments } => {
            arguments.is_empty() && parameters.contains(name.as_str())
        }
    }
}

pub(super) fn substitute_function_type(
    function: &Function,
    arguments: &[Type],
    template: &Type,
) -> Option<Type> {
    enum Frame<'a> {
        Enter(&'a Type),
        Finish(&'a str, usize),
    }
    let mut frames = vec![Frame::Enter(template)];
    let mut resolved = Vec::new();
    while let Some(frame) = frames.pop() {
        match frame {
            Frame::Enter(template) => match template {
                Type::I64 => resolved.push(Type::I64),
                Type::I32 => resolved.push(Type::I32),
                Type::Char => resolved.push(Type::Char),
                Type::U8 => resolved.push(Type::U8),
                Type::Usize => resolved.push(Type::Usize),
                Type::ArrayU8(length) => resolved.push(Type::ArrayU8(*length)),
                Type::F32 => resolved.push(Type::F32),
                Type::F64 => resolved.push(Type::F64),
                Type::Bool => resolved.push(Type::Bool),
                Type::String => resolved.push(Type::String),
                Type::Bytes => resolved.push(Type::Bytes),
                Type::Str => resolved.push(Type::Str),
                Type::SliceU8 => resolved.push(Type::SliceU8),
                Type::Named {
                    name,
                    arguments: nested,
                } => {
                    if nested.is_empty() {
                        if let Some(index) = function
                            .type_parameters
                            .iter()
                            .position(|parameter| parameter.name == *name)
                        {
                            resolved.push(arguments.get(index)?.clone());
                            continue;
                        }
                    }
                    frames.push(Frame::Finish(name, nested.len()));
                    frames.extend(nested.iter().rev().map(Frame::Enter));
                }
            },
            Frame::Finish(name, count) => {
                let split = resolved.len().checked_sub(count)?;
                let arguments = resolved.drain(split..).collect();
                resolved.push(Type::Named {
                    name: name.to_owned(),
                    arguments,
                });
            }
        }
    }
    (resolved.len() == 1).then(|| resolved.pop().expect("type count checked above"))
}

pub(super) fn scalar_function_substitutions(parameter_count: usize) -> Vec<Vec<Type>> {
    let count = 1_usize << parameter_count;
    (0..count)
        .map(|bits| {
            (0..parameter_count)
                .map(|index| {
                    if bits & (1 << index) == 0 {
                        Type::I64
                    } else {
                        Type::Bool
                    }
                })
                .collect()
        })
        .collect()
}

pub(super) fn generic_function_expression_is_direct_scalar(expression: &Expr) -> bool {
    let mut pending = vec![expression];
    while let Some(expression) = pending.pop() {
        match &expression.kind {
            ExprKind::Int(_)
            | ExprKind::Int32(_)
            | ExprKind::Char(_)
            | ExprKind::Uint8(_)
            | ExprKind::Usize(_)
            | ExprKind::Float32(_)
            | ExprKind::Float64(_)
            | ExprKind::Bool(_)
            | ExprKind::String(_)
            | ExprKind::Var(_) => {}
            ExprKind::ArrayU8(_) | ExprKind::RepeatArrayU8 { .. } => return false,
            ExprKind::Call { args, .. } => pending.extend(args.iter().rev()),
            ExprKind::Unary { value, .. } => pending.push(value),
            ExprKind::Binary { left, right, .. } => {
                pending.push(right);
                pending.push(left);
            }
            ExprKind::Block { statements, tail } => {
                pending.push(tail);
                for statement in statements.iter().rev() {
                    for index in (0..statement.child_count()).rev() {
                        if let Some(child) = statement.child(index) {
                            pending.push(child);
                        }
                    }
                }
            }
            ExprKind::If {
                condition,
                then_branch,
                else_branch,
            } => {
                pending.push(else_branch);
                pending.push(then_branch);
                pending.push(condition);
            }
            ExprKind::ConstructRecord { .. }
            | ExprKind::ConstructVariant { .. }
            | ExprKind::Match { .. }
            | ExprKind::Try { .. }
            | ExprKind::UpdateRecord { .. }
            | ExprKind::Project { .. }
            | ExprKind::MethodCall { .. }
            | ExprKind::SuperMethod { .. } => return false,
        }
    }
    true
}

pub(super) fn function_reaches(
    graph: &HashMap<String, Vec<String>>,
    current: &str,
    target: &str,
    visited: &mut HashSet<String>,
) -> bool {
    let mut pending = vec![current];
    while let Some(current) = pending.pop() {
        if current == target {
            return true;
        }
        if !visited.insert(current.to_owned()) {
            continue;
        }
        if let Some(callees) = graph.get(current) {
            pending.extend(callees.iter().rev().map(String::as_str));
        }
    }
    false
}

pub(super) fn function_reaches_any(
    graph: &HashMap<String, Vec<String>>,
    current: &str,
    targets: &HashSet<&str>,
    visited: &mut HashSet<String>,
) -> bool {
    let mut pending = vec![current];
    while let Some(current) = pending.pop() {
        if targets.contains(current) {
            return true;
        }
        if !visited.insert(current.to_owned()) {
            continue;
        }
        if let Some(callees) = graph.get(current) {
            pending.extend(callees.iter().rev().map(String::as_str));
        }
    }
    false
}

pub(super) fn validation_specialize_function(
    function: &Function,
    arguments: &[Type],
) -> Option<Function> {
    let mut specialized = function.clone();
    for param in &mut specialized.params {
        param.ty = substitute_function_type(function, arguments, &param.ty)?;
    }
    specialized.return_type = substitute_function_type(function, arguments, &function.return_type)?;
    Some(specialized)
}

pub(super) fn validation_specialize_signature(
    function: &Function,
    arguments: &[Type],
) -> Option<(Vec<Param>, Type)> {
    let mut params = Vec::with_capacity(function.params.len());
    for parameter in &function.params {
        let mut specialized = parameter.clone();
        specialized.ty = substitute_function_type(function, arguments, &parameter.ty)?;
        params.push(specialized);
    }
    let return_type = substitute_function_type(function, arguments, &function.return_type)?;
    Some((params, return_type))
}

pub(super) fn check_ownership_mode(
    program: &Program,
    function: &Function,
    param: &crate::ast::Param,
    types: &TypeTable<'_>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    if param.ty == Type::Str {
        if param.mode != ParamMode::Borrow {
            diagnostics.push(
                error(
                    program,
                    "SPX-O115",
                    format!(
                        "borrowed text parameter `{}.{}` must use `borrow str`",
                        function.name, param.name
                    ),
                    param.span,
                )
                .with_help(format!("use `{}: borrow str`", param.name)),
            );
        }
        return;
    }
    if param.ty == Type::SliceU8 {
        if param.mode != ParamMode::Borrow {
            diagnostics.push(
                error(
                    program,
                    "SPX-T263",
                    format!(
                        "byte-slice parameter `{}.{}` must use `borrow Slice<u8>`",
                        function.name, param.name
                    ),
                    param.span,
                )
                .with_help(format!("use `{}: borrow Slice<u8>`", param.name)),
            );
        }
        return;
    }
    if param.ty == Type::Bytes {
        if param.mode == ParamMode::Borrow && !function.type_parameters.is_empty() {
            diagnostics.push(error(
                program,
                "SPX-T263",
                format!(
                    "borrowed Bytes parameter `{}.{}` requires a monomorphic function",
                    function.name, param.name
                ),
                param.span,
            ));
        } else if !matches!(param.mode, ParamMode::Own | ParamMode::Borrow) {
            diagnostics.push(error(
                program,
                "SPX-T263",
                format!(
                    "byte parameter `{}.{}` must use `own Bytes` or `borrow Bytes`",
                    function.name, param.name
                ),
                param.span,
            ));
        }
        return;
    }
    let requires_explicit_mode =
        types.contains_resource(&param.ty) || types.contains_owned_bytes(&param.ty);
    match (requires_explicit_mode, param.mode) {
        (true, ParamMode::Value) => diagnostics.push(
            error(
                program,
                "SPX-O001",
                format!(
                    "resource parameter `{}.{}` needs `own`, `borrow`, or `shared`",
                    function.name, param.name
                ),
                param.span,
            )
            .with_help(format!(
                "use `{}: own {}` to transfer ownership",
                param.name, param.ty
            )),
        ),
        (false, mode) if mode != ParamMode::Value => diagnostics.push(error(
            program,
            "SPX-O002",
            format!(
                "ownership mode `{}` is only valid for resource types; `{}` is a value type",
                mode.text(),
                param.ty
            ),
            param.span,
        )),
        _ => {}
    }
}

pub(super) fn ordinary_result_arguments(ty: &Type) -> Option<(&Type, &Type)> {
    let Type::Named { name, arguments } = ty else {
        return None;
    };
    if name != "Result" || arguments.len() != 2 {
        return None;
    }
    Some((&arguments[0], &arguments[1]))
}

pub(super) fn ordinary_option_argument(ty: &Type) -> Option<&Type> {
    let Type::Named { name, arguments } = ty else {
        return None;
    };
    if name != "Option" || arguments.len() != 1 {
        return None;
    }
    Some(&arguments[0])
}

#[allow(clippy::too_many_arguments)]
pub(super) fn check_record_pattern(
    program: &Program,
    pattern_type: &str,
    fields: &[RecordMatchPatternField],
    expected: &Type,
    variables: &mut HashMap<String, Binding>,
    types: &TypeTable<'_>,
    diagnostics: &mut Vec<Diagnostic>,
    span: Span,
    mode: MatchMode,
) {
    let exact_recursive = mode != MatchMode::Value
        && types.is_nested_owned_byte_record(expected)
        && !types.is_flat_owned_byte_record(expected);
    enum Frame<'a, 't> {
        Enter {
            pattern_type: &'a str,
            fields: &'a [RecordMatchPatternField],
            expected: Type,
            span: Span,
        },
        Fields {
            pattern_type: &'a str,
            fields: &'a [RecordMatchPatternField],
            expected: Type,
            declared_fields: &'t [FieldDeclaration],
            index: usize,
            supplied: HashSet<&'a str>,
            span: Span,
        },
    }

    let mut frames = vec![Frame::Enter {
        pattern_type,
        fields,
        expected: expected.clone(),
        span,
    }];
    while let Some(frame) = frames.pop() {
        match frame {
            Frame::Enter {
                pattern_type,
                fields,
                expected,
                span,
            } => {
                let compatible = matches!(
                    &expected,
                    Type::Named { name, .. } if name == pattern_type
                );
                let declared_fields = effective_record_fields(types, &expected);
                if !compatible
                    || declared_fields.is_none()
                    || (mode == MatchMode::Value && types.needs_drop(&expected))
                {
                    diagnostics.push(error(
                        program,
                        "SPX-M103",
                        format!(
                            "record pattern `{pattern_type}` is incompatible with `{expected}`"
                        ),
                        span,
                    ));
                    continue;
                }
                frames.push(Frame::Fields {
                    pattern_type,
                    fields,
                    expected,
                    declared_fields: declared_fields.expect("checked above"),
                    index: 0,
                    supplied: HashSet::new(),
                    span,
                });
            }
            Frame::Fields {
                pattern_type,
                fields,
                expected,
                declared_fields,
                index,
                mut supplied,
                span,
            } => {
                let Some(field) = fields.get(index) else {
                    for declared in declared_fields {
                        if !supplied.contains(declared.name.as_str()) {
                            diagnostics.push(error(
                                program,
                                "SPX-M104",
                                format!(
                                    "record pattern `{pattern_type}` is missing field `{}`",
                                    declared.name
                                ),
                                span,
                            ));
                        }
                    }
                    continue;
                };
                let declared = declared_fields
                    .iter()
                    .find(|candidate| candidate.name == field.name);
                if !supplied.insert(field.name.as_str()) || declared.is_none() {
                    diagnostics.push(error(
                        program,
                        "SPX-M104",
                        format!(
                            "unknown or duplicate record pattern field `{}.{}`",
                            pattern_type, field.name
                        ),
                        field.span,
                    ));
                    frames.push(Frame::Fields {
                        pattern_type,
                        fields,
                        expected,
                        declared_fields,
                        index: index + 1,
                        supplied,
                        span,
                    });
                    continue;
                }
                let declared = declared.expect("checked above");
                let field_ty = types
                    .record_field_type(&expected, declared)
                    .unwrap_or_else(|| declared.ty.clone());
                frames.push(Frame::Fields {
                    pattern_type,
                    fields,
                    expected,
                    declared_fields,
                    index: index + 1,
                    supplied,
                    span,
                });
                match &field.pattern {
                    RecordMatchFieldPattern::Binding { name, span } => {
                        if exact_recursive && types.record_fields(&field_ty).is_some() {
                            diagnostics.push(error(
                                program,
                                "SPX-O117",
                                "nested owned-record fields require recursive record patterns",
                                *span,
                            ));
                        } else if !source_identifier(name) || variables.contains_key(name) {
                            diagnostics.push(error(
                                program,
                                "SPX-M104",
                                format!("invalid or duplicate record pattern binding `{name}`"),
                                *span,
                            ));
                        } else {
                            variables.insert(
                                name.clone(),
                                Binding {
                                    mode: if types.needs_drop(&field_ty) {
                                        match mode {
                                            MatchMode::Own => ParamMode::Own,
                                            MatchMode::Borrow => ParamMode::Borrow,
                                            MatchMode::Value => ParamMode::Value,
                                        }
                                    } else {
                                        ParamMode::Value
                                    },
                                    ty: field_ty,
                                    availability: Availability::Available,
                                    moved_places: HashMap::new(),
                                    definitely_partial: HashSet::new(),
                                    native_unit_discard: false,
                                    mutable: false,
                                    active_loans: BTreeSet::new(),
                                    borrow_origin: None,
                                },
                            );
                        }
                    }
                    RecordMatchFieldPattern::Wildcard { span } => {
                        if (mode == MatchMode::Own || exact_recursive)
                            && types.needs_drop(&field_ty)
                        {
                            let message = if exact_recursive {
                                "exact owned-record patterns cannot wildcard an owned field"
                            } else {
                                "`match own` must bind every owned record field in this tranche"
                            };
                            diagnostics.push(error(program, "SPX-O117", message, *span));
                        }
                    }
                    RecordMatchFieldPattern::Record {
                        type_name,
                        fields,
                        span,
                        ..
                    } => frames.push(Frame::Enter {
                        pattern_type: type_name,
                        fields,
                        expected: field_ty,
                        span: *span,
                    }),
                }
            }
        }
    }
}
