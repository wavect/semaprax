//! Multi-line statement projection for canonical formatting.

use super::*;

/// Renders one statement of a multi-line block body. Unsafe boundary
/// statements open their own indented braces; their ordinary block bodies
/// recurse through this same helper.
pub(super) fn write_block_statement(
    output: &mut impl std::fmt::Write,
    statement: &Statement,
    depth: usize,
    placement: &comments::Placement,
) {
    match statement {
        Statement::Let {
            name,
            mutable,
            declared,
            value,
            ..
        } => {
            write_indent(output, depth);
            if *mutable {
                write!(output, "let mut {name}").unwrap();
            } else {
                write!(output, "let {name}").unwrap();
            }
            if let Some(ty) = declared {
                write!(output, ": ").unwrap();
                write_type(output, ty);
            }
            write!(output, " = ").unwrap();
            write_expr(output, value, 0);
            writeln!(output, ";").unwrap();
        }
        Statement::Assign {
            name, field, value, ..
        } => {
            write_indent(output, depth);
            match field {
                Some(field) => write!(output, "{name}.{} = ", field.name).unwrap(),
                None => write!(output, "{name} = ").unwrap(),
            }
            write_expr(output, value, 0);
            writeln!(output, ";").unwrap();
        }
        Statement::Unsafe { audit, body, .. } => {
            write_indent(output, depth);
            write!(output, "@audit(\"").unwrap();
            write_escaped(output, audit);
            writeln!(output, "\") unsafe {{").unwrap();
            let ExprKind::Block { statements, tail } = &body.kind else {
                unreachable!("unsafe bodies always parse as blocks");
            };
            write_block_items(output, statements, tail, depth + 1, placement);
            placement.closing(output, body.span.end.saturating_sub(1), depth + 1);
            write_indent(output, depth);
            writeln!(output, "}}").unwrap();
        }
        Statement::While {
            condition, body, ..
        } => {
            write_indent(output, depth);
            write!(output, "while ").unwrap();
            write_expr(output, condition, 0);
            writeln!(output, " {{").unwrap();
            let ExprKind::Block { statements, tail } = &body.kind else {
                unreachable!("while bodies always parse as blocks");
            };
            write_block_items(output, statements, tail, depth + 1, placement);
            placement.closing(output, body.span.end.saturating_sub(1), depth + 1);
            write_indent(output, depth);
            writeln!(output, "}}").unwrap();
        }
        Statement::For {
            item, values, body, ..
        }
        | Statement::ForOwn {
            item, values, body, ..
        } => {
            write_indent(output, depth);
            let prefix = if matches!(statement, Statement::ForOwn { .. }) {
                "for own"
            } else {
                "for"
            };
            write!(output, "{prefix} {item} in ").unwrap();
            write_expr(output, values, 0);
            writeln!(output, " {{").unwrap();
            let ExprKind::Block { statements, tail } = &body.kind else {
                unreachable!("for bodies always parse as blocks");
            };
            write_block_items(output, statements, tail, depth + 1, placement);
            placement.closing(output, body.span.end.saturating_sub(1), depth + 1);
            write_indent(output, depth);
            writeln!(output, "}}").unwrap();
        }
    }
}
