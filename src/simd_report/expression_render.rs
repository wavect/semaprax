//! Canonical expression display for SIMD eligibility reports.
use super::*;

pub(super) fn render_expr(
    walker: &Walker<'_>,
    expr: &ResolvedExpr,
    parent_precedence: u8,
    output: &mut String,
) {
    match &expr.kind {
        ResolvedExprKind::Int(number) => {
            output.push_str(&number.to_string());
        }
        ResolvedExprKind::Int32(value) => {
            output.push_str(&value.to_string());
            output.push_str("i32");
        }
        ResolvedExprKind::Uint8(value) => {
            output.push_str(&value.to_string());
            output.push_str("u8");
        }
        ResolvedExprKind::Usize(value) => {
            output.push_str(&value.to_string());
            output.push_str("usize");
        }
        ResolvedExprKind::Float32(bits) => {
            output.push_str(&format::canonical_f32_bits(*bits));
            output.push_str("f32");
        }
        ResolvedExprKind::Float64(bits) => {
            output.push_str(&format::canonical_f64_bits(*bits));
        }
        ResolvedExprKind::Bool(value) => {
            output.push_str(if *value { "true" } else { "false" });
        }
        ResolvedExprKind::String(value) => {
            output.push_str(&crate::format::canonical_string(value));
        }
        ResolvedExprKind::ArrayU8(values) => {
            output.push('[');
            for (index, value) in values.iter().enumerate() {
                if index != 0 {
                    output.push_str(", ");
                }
                output.push_str(&format!("{value}u8"));
            }
            output.push(']');
        }
        ResolvedExprKind::RepeatArrayU8 { value, count } => {
            output.push_str(&format!("[{value}u8; {count}]"));
        }
        ResolvedExprKind::Char(value) => {
            output.push_str(&format!("char({value})"));
        }
        ResolvedExprKind::Place(place) => {
            output.push_str(&walker.place_name(&place.root, &place.projections));
        }
        ResolvedExprKind::BorrowPlace { operation, place } => {
            output.push_str(&walker.declaration_name(operation));
            output.push('(');
            output.push_str(&walker.place_name(&place.root, &place.projections));
            output.push(')');
        }
        ResolvedExprKind::ByteRange {
            source, start, end, ..
        } => {
            output.push_str("byte_range(");
            output.push_str(&render_child(walker, source, 0));
            output.push_str(", ");
            output.push_str(&render_child(walker, start, 0));
            output.push_str(", ");
            output.push_str(&render_child(walker, end, 0));
            output.push(')');
        }
        ResolvedExprKind::FunctionReference { target } => {
            output.push_str(&walker.declaration_name(target))
        }
        ResolvedExprKind::Invoke { callable, args } => {
            output.push_str(&render_child(walker, callable, 0));
            render_args(walker, args, output);
        }
        ResolvedExprKind::Call { callee, args, .. } => {
            let name = walker.declaration_name(callee);
            output.push_str(&name);
            render_args(walker, args, output);
        }
        ResolvedExprKind::NativeRustImportCall(call) => {
            let name = walker.declaration_name(&call.import);
            output.push_str(&name);
            output.push_str("(<native-rust-import>)");
        }
        ResolvedExprKind::HostCommandCall(call) => {
            output.push_str(crate::command_io_ops::name(call.operation));
            render_args(walker, &call.args, output);
        }
        ResolvedExprKind::Unary { op, value } => {
            let precedence = 7u8;
            output.push_str(match op {
                UnaryOp::Neg => "-",
                UnaryOp::Not => "!",
            });
            let inner = render_child(walker, value, precedence);
            output.push_str(&inner);
        }
        ResolvedExprKind::Binary { op, left, right } => {
            let precedence = op.precedence();
            let delimited = precedence < parent_precedence;
            if delimited {
                output.push('(');
            }
            output.push_str(&render_child(walker, left, precedence));
            output.push(' ');
            output.push_str(op.text());
            output.push(' ');
            output.push_str(&render_child(walker, right, precedence));
            if delimited {
                output.push(')');
            }
        }
        ResolvedExprKind::Block { statements, tail } => {
            output.push_str("{ ");
            for statement in statements {
                match statement {
                    ResolvedStatement::Let { binding, value, .. } => {
                        output.push_str("let ");
                        output.push_str(&binding.name);
                        output.push_str(" = ");
                        output.push_str(&render_child(walker, value, 0));
                        output.push_str("; ");
                    }
                    ResolvedStatement::Assign {
                        binding,
                        field,
                        value,
                        ..
                    } => {
                        output.push_str(&binding.name);
                        if let Some(field) = field {
                            output.push('.');
                            output.push_str(&walker.declaration_name(field));
                        }
                        output.push_str(" = ");
                        output.push_str(&render_child(walker, value, 0));
                        output.push_str("; ");
                    }
                    ResolvedStatement::Unsafe { body, .. } => {
                        output.push_str("unsafe ");
                        output.push_str(&render_child(walker, body, 0));
                        output.push_str("; ");
                    }
                    ResolvedStatement::While {
                        condition, body, ..
                    } => {
                        output.push_str("while ");
                        output.push_str(&render_child(walker, condition, 0));
                        output.push(' ');
                        output.push_str(&render_child(walker, body, 0));
                        output.push_str("; ");
                    }
                }
            }
            output.push_str(&render_child(walker, tail, 0));
            output.push_str(" }");
        }
        ResolvedExprKind::If {
            condition,
            then_branch,
            else_branch,
        } => {
            output.push_str("if(");
            output.push_str(&render_child(walker, condition, 0));
            output.push(',');
            output.push_str(&render_child(walker, then_branch, 0));
            output.push(',');
            output.push_str(&render_child(walker, else_branch, 0));
            output.push(')');
        }
        ResolvedExprKind::ConstructRecord { record, fields } => {
            output.push_str(&walker.declaration_name(record));
            render_fields(walker, fields, output);
        }
        ResolvedExprKind::ConstructVariant {
            variant,
            case,
            fields,
        } => {
            output.push_str(&walker.declaration_name(variant));
            output.push('.');
            output.push_str(&walker.declaration_name(case));
            render_fields(walker, fields, output);
        }
        ResolvedExprKind::UpdateRecord { base, fields, .. } => {
            output.push_str("update(");
            output.push_str(&render_child(walker, base, 0));
            output.push(',');
            render_fields(walker, fields, output);
            output.push(')');
        }
        ResolvedExprKind::Match {
            scrutinee, arms, ..
        } => {
            output.push_str("match(");
            output.push_str(&render_child(walker, scrutinee, 0));
            output.push_str("){");
            for (index, arm) in arms.iter().enumerate() {
                if index != 0 {
                    output.push('|');
                }
                output.push_str(pattern_tag(walker, &arm.pattern).as_str());
                if let Some(guard) = &arm.guard {
                    output.push_str(" if ");
                    output.push_str(&render_child(walker, guard, 0));
                }
                output.push_str("=>");
                output.push_str(&render_child(walker, &arm.value, 0));
            }
            output.push('}');
        }
        ResolvedExprKind::Try { operand, .. } => {
            output.push_str("try(");
            output.push_str(&render_child(walker, operand, 0));
            output.push(')');
        }
        ResolvedExprKind::TryOption { operand, .. } => {
            output.push_str("try_option(");
            output.push_str(&render_child(walker, operand, 0));
            output.push(')');
        }
        ResolvedExprKind::Project { base, field } => {
            output.push_str(&render_child(walker, base, 7));
            output.push('.');
            output.push_str(&walker.declaration_name(field));
        }
        ResolvedExprKind::Upcast { source } => {
            output.push_str(&render_child(walker, source, 7));
        }
    }
}

pub(super) fn render_child(
    walker: &Walker<'_>,
    expr: &ResolvedExpr,
    parent_precedence: u8,
) -> String {
    let mut text = String::new();
    render_expr(walker, expr, parent_precedence, &mut text);
    text
}

fn render_args(walker: &Walker<'_>, args: &[ResolvedExpr], output: &mut String) {
    output.push('(');
    for (index, argument) in args.iter().enumerate() {
        if index != 0 {
            output.push_str(", ");
        }
        output.push_str(&render_child(walker, argument, 0));
    }
    output.push(')');
}

fn render_fields(
    walker: &Walker<'_>,
    fields: &[hir::ResolvedFieldInitializer],
    output: &mut String,
) {
    output.push('{');
    for (index, field) in fields.iter().enumerate() {
        if index != 0 {
            output.push(',');
        }
        output.push_str(&walker.declaration_name(&field.field));
        output.push(':');
        output.push_str(&render_child(walker, &field.value, 0));
    }
    output.push('}');
}

pub(super) fn pattern_tag(walker: &Walker<'_>, pattern: &hir::ResolvedMatchPattern) -> String {
    match pattern {
        hir::ResolvedMatchPattern::Variant { variant, case, .. } => format!(
            "{}.{}",
            walker.declaration_name(variant),
            walker.declaration_name(case)
        ),
        hir::ResolvedMatchPattern::Record { record, .. } => walker.declaration_name(record),
        hir::ResolvedMatchPattern::Wildcard => "_".to_owned(),
        // Refutable Match v1: literal/or patterns tag with their canonical
        // value text; a binding arm tags with its name.
        hir::ResolvedMatchPattern::Literal(value) => pattern_value_text(*value),
        hir::ResolvedMatchPattern::Or(alternatives) => alternatives
            .iter()
            .map(|alternative| pattern_tag(walker, alternative))
            .collect::<Vec<_>>()
            .join("|"),
        hir::ResolvedMatchPattern::Binding(binding) => binding.name.clone(),
    }
}

pub(super) fn pattern_value_text(value: hir::PatternValue) -> String {
    match value {
        hir::PatternValue::Int(value) => value.to_string(),
        hir::PatternValue::Int32(value) => format!("{value}i32"),
        hir::PatternValue::Uint8(value) => format!("{value}u8"),
        hir::PatternValue::Usize(value) => format!("{value}usize"),
        hir::PatternValue::Char(value) => crate::format::canonical_char(value),
        hir::PatternValue::Bool(value) => value.to_string(),
    }
}
