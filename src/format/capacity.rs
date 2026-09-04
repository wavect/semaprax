//! Capacity accounting for the canonical formatter: exact rendered lengths and
//! the legacy temporary-byte estimates the bounded-output reservations use.
//! Moved verbatim out of `format.rs`; the writers it measures stay there.

use super::*;

fn rendered_expr_len(value: &Expr, parent_precedence: u8) -> usize {
    struct Counter(usize);
    impl std::fmt::Write for Counter {
        fn write_str(&mut self, value: &str) -> std::fmt::Result {
            self.0 = self.0.saturating_add(value.len());
            Ok(())
        }
    }
    let mut counter = Counter(0);
    write_expr(&mut counter, value, parent_precedence);
    counter.0
}

fn display_len(value: &impl std::fmt::Display) -> usize {
    struct Counter(usize);
    impl std::fmt::Write for Counter {
        fn write_str(&mut self, value: &str) -> std::fmt::Result {
            self.0 = self.0.saturating_add(value.len());
            Ok(())
        }
    }
    let mut counter = Counter(0);
    std::fmt::write(&mut counter, format_args!("{value}")).unwrap();
    counter.0
}

fn joined_len(lengths: impl IntoIterator<Item = usize>, count: usize, separator: usize) -> usize {
    lengths
        .into_iter()
        .fold(0usize, usize::saturating_add)
        .saturating_add(separator.saturating_mul(count.saturating_sub(1)))
}

fn escaped_len(value: &str) -> usize {
    value.bytes().fold(0usize, |length, byte| {
        length.saturating_add(if matches!(byte, b'\\' | b'"') { 2 } else { 1 })
    })
}

fn legacy_type_parameter_bytes(parameters: &[crate::ast::TypeParameterDeclaration]) -> usize {
    if parameters.is_empty() {
        return 0;
    }
    let names = parameters.iter().map(|parameter| parameter.name.len());
    let cloned = names.clone().fold(0usize, usize::saturating_add);
    let joined = joined_len(names, parameters.len(), 2);
    cloned
        .saturating_add(joined)
        .saturating_add(joined.saturating_add(2))
}

fn legacy_string_join_bytes(values: &[String]) -> usize {
    let cloned = values
        .iter()
        .map(String::len)
        .fold(0usize, usize::saturating_add);
    cloned.saturating_add(joined_len(values.iter().map(String::len), values.len(), 2))
}

pub(super) fn legacy_canonical_temporary_bytes(program: &Program) -> usize {
    let mut total = 0usize;
    for protocol in &program.protocols {
        if protocol.explicit_id {
            total = total.saturating_add(escaped_len(&protocol.stable_id));
        }
        for method in &protocol.methods {
            if method.explicit_id {
                total = total.saturating_add(escaped_len(&method.stable_id));
            }
        }
    }
    for implementation in &program.implementations {
        if implementation.explicit_id {
            total = total.saturating_add(escaped_len(&implementation.stable_id));
        }
        total = total
            .saturating_add(escaped_len(&implementation.protocol_id))
            .saturating_add(escaped_len(&implementation.receiver_id));
        for member in &implementation.members {
            total = total
                .saturating_add(escaped_len(&member.method_id))
                .saturating_add(escaped_len(&member.function_id));
        }
    }
    for module_use in &program.module_uses {
        total = total.saturating_add(escaped_len(&module_use.persistent_id));
    }
    if !program.permits.is_empty() {
        total = total.saturating_add(legacy_string_join_bytes(&program.permits));
    }
    for declaration in &program.types {
        if declaration.explicit_id {
            total = total.saturating_add(escaped_len(&declaration.stable_id));
        }
        total = total.saturating_add(legacy_type_parameter_bytes(&declaration.type_parameters));
        match &declaration.kind {
            TypeDeclarationKind::Resource { lifecycles } => {
                for lifecycle in lifecycles {
                    if let Some(stable_id) = &lifecycle.stable_id {
                        total = total.saturating_add(escaped_len(stable_id));
                    }
                    if let ResourceLifecycleKind::Imported { import_key } = &lifecycle.kind {
                        total = total.saturating_add(escaped_len(import_key));
                    }
                }
            }
            TypeDeclarationKind::Record { fields } => {
                for field in fields {
                    if field.explicit_id {
                        total = total.saturating_add(escaped_len(&field.stable_id));
                    }
                }
            }
            TypeDeclarationKind::Class { fields, methods } => {
                for field in fields {
                    if field.explicit_id {
                        total = total.saturating_add(escaped_len(&field.stable_id));
                    }
                }
                for method in methods {
                    if method.explicit_id {
                        total = total.saturating_add(escaped_len(&method.stable_id));
                    }
                    total = total
                        .saturating_add(legacy_type_parameter_bytes(&method.type_parameters))
                        .saturating_add(method.name.len())
                        .saturating_add(legacy_string_join_bytes(&method.effects));
                    for param in &method.params {
                        total = total.saturating_add(param.name.len());
                    }
                    total = total.saturating_add(
                        legacy_expr_temporary_bytes(&method.body, 0).saturating_mul(2),
                    );
                }
            }
            TypeDeclarationKind::Variant { cases } => {
                for case in cases {
                    if case.explicit_id {
                        total = total.saturating_add(escaped_len(&case.stable_id));
                    }
                    for field in &case.fields {
                        if field.explicit_id {
                            total = total.saturating_add(escaped_len(&field.stable_id));
                        }
                    }
                }
            }
        }
    }
    for interface in &program.interfaces {
        if interface.explicit_id {
            total = total.saturating_add(escaped_len(&interface.stable_id));
        }
        total = total.saturating_add(legacy_string_join_bytes(&interface.permits));
        for import in &interface.imports {
            if import.explicit_id {
                total = total.saturating_add(escaped_len(&import.stable_id));
            }
            total = total.saturating_add(legacy_string_join_bytes(&import.effects));
            if let ImportFailure::Status { domain_id } = &import.failure {
                total = total.saturating_add(escaped_len(domain_id));
            }
        }
    }
    for function in &program.functions {
        if function.explicit_id {
            total = total.saturating_add(escaped_len(&function.stable_id));
        }
        total = total.saturating_add(legacy_type_parameter_bytes(&function.type_parameters));
        if !function.effects.is_empty() {
            total = total.saturating_add(legacy_string_join_bytes(&function.effects));
        }
        for contract in function.requires.iter().chain(&function.ensures) {
            total = total.saturating_add(legacy_expr_temporary_bytes(contract, 0));
            if contains_record_construction(contract) {
                total = total.saturating_add(rendered_expr_len(contract, 0).saturating_add(2));
            }
        }
        if let ExprKind::Block { statements, tail } = &function.body.kind {
            for statement in statements {
                let child_count = statement.child_count();
                for child_index in 0..child_count {
                    if let Some(child) = statement.child(child_index) {
                        total = total.saturating_add(legacy_expr_temporary_bytes(child, 0));
                    }
                }
            }
            total = total.saturating_add(legacy_expr_temporary_bytes(tail, 0));
        } else {
            total = total.saturating_add(legacy_expr_temporary_bytes(&function.body, 0));
        }
    }
    total
}

pub(super) fn legacy_expr_temporary_bytes(root: &Expr, root_precedence: u8) -> usize {
    let mut total = 0usize;
    let mut stack = vec![(root, root_precedence)];
    while let Some((value, parent_precedence)) = stack.pop() {
        let rendered = rendered_expr_len(value, parent_precedence);
        match &value.kind {
            ExprKind::Int(_)
            | ExprKind::Int32(_)
            | ExprKind::Char(_)
            | ExprKind::Uint8(_)
            | ExprKind::Usize(_)
            | ExprKind::ArrayU8(_)
            | ExprKind::RepeatArrayU8 { .. }
            | ExprKind::Float32(_)
            | ExprKind::Float64(_)
            | ExprKind::Bool(_)
            | ExprKind::String(_) => {}
            ExprKind::Var(name) => total = total.saturating_add(name.len()),
            ExprKind::MethodCall { receiver, args, .. } => {
                let joined = joined_len(
                    args.iter().map(|argument| rendered_expr_len(argument, 0)),
                    args.len(),
                    2,
                );
                total = total.saturating_add(joined).saturating_add(rendered);
                stack.push((receiver, 8));
                stack.extend(args.iter().rev().map(|argument| (argument, 0)));
            }
            ExprKind::Call {
                type_arguments,
                args,
                ..
            } => {
                if !type_arguments.is_empty() {
                    let argument_lengths = type_arguments.iter().map(display_len);
                    let arguments = argument_lengths.clone().fold(0usize, usize::saturating_add);
                    let joined = joined_len(argument_lengths, type_arguments.len(), 2);
                    total = total
                        .saturating_add(arguments)
                        .saturating_add(joined)
                        .saturating_add(joined.saturating_add(2));
                }
                let joined = joined_len(
                    args.iter().map(|argument| rendered_expr_len(argument, 0)),
                    args.len(),
                    2,
                );
                total = total.saturating_add(joined).saturating_add(rendered);
                stack.extend(args.iter().rev().map(|argument| (argument, 0)));
            }
            ExprKind::Unary { value, .. } => {
                total = total.saturating_add(rendered);
                stack.push((value, 7));
            }
            ExprKind::Binary {
                op, left, right, ..
            } => {
                let inner = rendered_expr_len(left, op.precedence())
                    .saturating_add(rendered_expr_len(right, op.precedence() + 1))
                    .saturating_add(op.text().len())
                    .saturating_add(2);
                total = total.saturating_add(inner);
                if op.precedence() < parent_precedence {
                    total = total.saturating_add(inner.saturating_add(2));
                }
                stack.push((right, op.precedence() + 1));
                stack.push((left, op.precedence()));
            }
            ExprKind::Block { statements, tail } => {
                let mut parts = Vec::with_capacity(statements.len() + 1);
                for statement in statements {
                    let part = match statement {
                        Statement::Let {
                            name,
                            mutable,
                            value,
                            ..
                        } => name
                            .len()
                            .saturating_add(rendered_expr_len(value, 0))
                            .saturating_add(if *mutable { 12 } else { 8 }),
                        Statement::Assign {
                            name, field, value, ..
                        } => name
                            .len()
                            .saturating_add(
                                field
                                    .as_ref()
                                    .map(|field| field.name.len().saturating_add(1))
                                    .unwrap_or(0),
                            )
                            .saturating_add(rendered_expr_len(value, 0))
                            .saturating_add(4),
                        Statement::Unsafe { audit, .. } => escaped_len(audit).saturating_add(18),
                        // `while ` + condition + one separator space; the body
                        // block renders through the shared expression budget
                        // below.
                        Statement::While { condition, .. } => {
                            rendered_expr_len(condition, 0).saturating_add(7)
                        }
                    };
                    total = total.saturating_add(part);
                    parts.push(part);
                    let child_count = statement.child_count();
                    for child_index in 0..child_count {
                        if let Some(child) = statement.child(child_index) {
                            stack.push((child, 0));
                        }
                    }
                }
                parts.push(rendered_expr_len(tail, 0));
                let joined = joined_len(parts, statements.len() + 1, 1);
                total = total.saturating_add(joined).saturating_add(rendered);
                stack.push((tail, 0));
            }
            ExprKind::If {
                condition,
                then_branch,
                else_branch,
            } => {
                if contains_record_construction(condition) {
                    total = total.saturating_add(rendered_expr_len(condition, 0).saturating_add(2));
                }
                total = total.saturating_add(rendered);
                stack.push((else_branch, 0));
                stack.push((then_branch, 0));
                stack.push((condition, 0));
            }
            ExprKind::ConstructRecord {
                type_name,
                type_arguments,
                fields,
                ..
            }
            | ExprKind::ConstructVariant {
                type_name,
                type_arguments,
                fields,
                ..
            } => {
                if type_arguments.is_empty() {
                    total = total.saturating_add(type_name.len());
                } else {
                    let argument_lengths = type_arguments.iter().map(display_len);
                    let arguments = argument_lengths.clone().fold(0usize, usize::saturating_add);
                    let joined = joined_len(argument_lengths, type_arguments.len(), 2);
                    total = total
                        .saturating_add(arguments)
                        .saturating_add(joined)
                        .saturating_add(type_name.len().saturating_add(joined).saturating_add(2));
                }
                let mut parts = Vec::with_capacity(fields.len());
                for field in fields {
                    let part = field
                        .name
                        .len()
                        .saturating_add(rendered_expr_len(&field.value, 0))
                        .saturating_add(2);
                    total = total.saturating_add(part);
                    parts.push(part);
                    stack.push((&field.value, 0));
                }
                if !fields.is_empty() {
                    total = total.saturating_add(joined_len(parts, fields.len(), 2));
                }
                total = total.saturating_add(rendered);
            }
            ExprKind::Match {
                mode,
                scrutinee,
                arms,
            } => {
                total = total.saturating_add(mode.source_prefix().len());
                if contains_record_construction(scrutinee) {
                    total = total.saturating_add(rendered_expr_len(scrutinee, 0).saturating_add(2));
                }
                let mut parts = Vec::with_capacity(arms.len());
                for arm in arms {
                    total = total.saturating_add(legacy_match_pattern_bytes(&arm.pattern));
                    // Refutable Match v1: guarded arms render their guard
                    // between the pattern and `=>`.
                    let guard_len = arm
                        .guard
                        .as_ref()
                        .map_or(0, |guard| rendered_expr_len(guard, 0).saturating_add(4));
                    let part = rendered_match_pattern_len(&arm.pattern)
                        .saturating_add(rendered_expr_len(&arm.value, 0))
                        .saturating_add(5)
                        .saturating_add(guard_len);
                    total = total.saturating_add(part);
                    parts.push(part);
                    if let Some(guard) = &arm.guard {
                        stack.push((guard.as_ref(), 0));
                    }
                    stack.push((&arm.value, 0));
                }
                total = total
                    .saturating_add(joined_len(parts, arms.len(), 1))
                    .saturating_add(rendered);
                stack.push((scrutinee, 0));
            }
            ExprKind::Try { operand } => {
                let delimited = matches!(
                    operand.kind,
                    ExprKind::Binary { .. } | ExprKind::If { .. } | ExprKind::Block { .. }
                );
                if delimited {
                    total = total.saturating_add(rendered_expr_len(operand, 0).saturating_add(2));
                }
                total = total.saturating_add(rendered);
                stack.push((operand, if delimited { 0 } else { 8 }));
            }
            ExprKind::UpdateRecord { base, fields } => {
                let delimited = matches!(
                    base.kind,
                    ExprKind::Binary { .. } | ExprKind::If { .. } | ExprKind::Block { .. }
                );
                if delimited {
                    total = total.saturating_add(rendered_expr_len(base, 0).saturating_add(2));
                }
                let mut parts = Vec::with_capacity(fields.len());
                for field in fields {
                    let part = field
                        .name
                        .len()
                        .saturating_add(rendered_expr_len(&field.value, 0))
                        .saturating_add(2);
                    total = total.saturating_add(part);
                    parts.push(part);
                    stack.push((&field.value, 0));
                }
                if !fields.is_empty() {
                    total = total.saturating_add(joined_len(parts, fields.len(), 2));
                }
                total = total.saturating_add(rendered);
                stack.push((base, if delimited { 0 } else { 8 }));
            }
            ExprKind::Project { base, .. } => {
                let delimited = matches!(
                    base.kind,
                    ExprKind::Binary { .. } | ExprKind::If { .. } | ExprKind::Block { .. }
                );
                if delimited {
                    total = total.saturating_add(rendered_expr_len(base, 0).saturating_add(2));
                }
                total = total.saturating_add(rendered);
                stack.push((base, if delimited { 0 } else { 8 }));
            }
            ExprKind::SuperMethod { method, args, .. } => {
                let joined = joined_len(
                    args.iter().map(|argument| rendered_expr_len(argument, 0)),
                    args.len(),
                    2,
                );
                total = total
                    .saturating_add(method.len())
                    .saturating_add(joined)
                    .saturating_add(rendered);
                stack.extend(args.iter().rev().map(|argument| (argument, 0)));
            }
        }
    }
    total
}

fn rendered_match_pattern_len(pattern: &MatchPattern) -> usize {
    struct Counter(usize);
    impl std::fmt::Write for Counter {
        fn write_str(&mut self, value: &str) -> std::fmt::Result {
            self.0 = self.0.saturating_add(value.len());
            Ok(())
        }
    }
    let mut counter = Counter(0);
    write_match_pattern(&mut counter, pattern);
    counter.0
}

fn legacy_match_pattern_bytes(pattern: &MatchPattern) -> usize {
    match pattern {
        MatchPattern::Wildcard { .. } => 0,
        // Refutable Match v1 patterns have no legacy byte accounting; their
        // exact canonical rendering is already counted by the caller.
        MatchPattern::Literal { .. } | MatchPattern::Or { .. } | MatchPattern::Binding { .. } => 0,
        MatchPattern::Variant {
            type_name,
            case_name,
            fields,
            ..
        } => {
            let parts = fields.iter().map(|field| {
                if field.name == field.binding {
                    field.name.len()
                } else {
                    field
                        .name
                        .len()
                        .saturating_add(field.binding.len())
                        .saturating_add(2)
                }
            });
            let part_total = parts.clone().fold(0usize, usize::saturating_add);
            let joined = joined_len(parts, fields.len(), 2);
            let outer = type_name
                .len()
                .saturating_add(case_name.len())
                .saturating_add(if fields.is_empty() {
                    5
                } else {
                    joined.saturating_add(8)
                });
            part_total.saturating_add(joined).saturating_add(outer)
        }
        MatchPattern::Record {
            type_name, fields, ..
        } => legacy_record_pattern_bytes(type_name, fields),
    }
}

fn legacy_record_pattern_bytes(
    root_name: &str,
    root_fields: &[crate::ast::RecordMatchPatternField],
) -> usize {
    use crate::ast::RecordMatchFieldPattern;

    let mut total = 0usize;
    let mut stack = vec![(root_name, root_fields)];
    while let Some((type_name, fields)) = stack.pop() {
        let mut part_lengths = Vec::with_capacity(fields.len());
        for field in fields {
            let part = match &field.pattern {
                RecordMatchFieldPattern::Binding { name, .. } if name == &field.name => {
                    total = total.saturating_add(field.name.len());
                    field.name.len()
                }
                RecordMatchFieldPattern::Binding { name, .. } => field
                    .name
                    .len()
                    .saturating_add(name.len())
                    .saturating_add(2),
                RecordMatchFieldPattern::Wildcard { .. } => field.name.len().saturating_add(3),
                RecordMatchFieldPattern::Record {
                    type_name,
                    fields: nested,
                    ..
                } => {
                    let length = field
                        .name
                        .len()
                        .saturating_add(rendered_record_pattern_len(type_name, nested))
                        .saturating_add(2);
                    stack.push((type_name, nested));
                    length
                }
            };
            total = total.saturating_add(part);
            part_lengths.push(part);
        }
        let joined = joined_len(part_lengths, fields.len(), 2);
        total = total
            .saturating_add(joined)
            .saturating_add(type_name.len().saturating_add(if fields.is_empty() {
                3
            } else {
                joined.saturating_add(4)
            }));
    }
    total
}

fn rendered_record_pattern_len(
    type_name: &str,
    fields: &[crate::ast::RecordMatchPatternField],
) -> usize {
    struct Counter(usize);
    impl std::fmt::Write for Counter {
        fn write_str(&mut self, value: &str) -> std::fmt::Result {
            self.0 = self.0.saturating_add(value.len());
            Ok(())
        }
    }
    let mut counter = Counter(0);
    write_record_match_pattern(&mut counter, type_name, fields);
    counter.0
}
