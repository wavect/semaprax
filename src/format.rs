use std::fmt::Write;

use crate::ast::{Expr, ExprKind, Program, Statement, TypeDeclarationKind, UnaryOp};

pub fn canonical(program: &Program) -> String {
    let mut output = String::new();
    writeln!(output, "module {};", program.module).unwrap();
    if !program.permits.is_empty() {
        writeln!(output, "\npermit {{ {} }}", program.permits.join(", ")).unwrap();
    }
    for declaration in &program.types {
        writeln!(output).unwrap();
        if declaration.explicit_id {
            writeln!(output, "@id(\"{}\")", escape_string(&declaration.stable_id)).unwrap();
        }
        match &declaration.kind {
            TypeDeclarationKind::Resource => {
                writeln!(output, "resource {};", declaration.name).unwrap();
            }
            TypeDeclarationKind::Record { fields } => {
                writeln!(output, "record {} {{", declaration.name).unwrap();
                for field in fields {
                    if field.explicit_id {
                        writeln!(output, "    @id(\"{}\")", escape_string(&field.stable_id))
                            .unwrap();
                    }
                    writeln!(output, "    {}: {},", field.name, field.ty).unwrap();
                }
                writeln!(output, "}}").unwrap();
            }
        }
    }
    for function in &program.functions {
        writeln!(output).unwrap();
        if function.explicit_id {
            writeln!(output, "@id(\"{}\")", escape_string(&function.stable_id)).unwrap();
        }
        write!(output, "fn {}(", function.name).unwrap();
        for (index, param) in function.params.iter().enumerate() {
            if index > 0 {
                output.push_str(", ");
            }
            write!(
                output,
                "{}: {}{}",
                param.name,
                param.mode.source_prefix(),
                param.ty
            )
            .unwrap();
        }
        writeln!(output, ") -> {}", function.return_type).unwrap();
        if !function.effects.is_empty() {
            writeln!(output, "    uses {{ {} }}", function.effects.join(", ")).unwrap();
        }
        for contract in &function.requires {
            writeln!(output, "    requires {}", expr(contract, 0)).unwrap();
        }
        for contract in &function.ensures {
            writeln!(output, "    ensures {}", expr(contract, 0)).unwrap();
        }
        write_function_body(&mut output, &function.body);
    }
    output
}

pub fn expr(value: &Expr, parent_precedence: u8) -> String {
    match &value.kind {
        ExprKind::Int(number) => number.to_string(),
        ExprKind::Bool(value) => value.to_string(),
        ExprKind::Var(name) => name.clone(),
        ExprKind::Call { name, args } => format!(
            "{}({})",
            name,
            args.iter()
                .map(|arg| expr(arg, 0))
                .collect::<Vec<_>>()
                .join(", ")
        ),
        ExprKind::Unary { op, value } => {
            let operator = match op {
                UnaryOp::Neg => "-",
                UnaryOp::Not => "!",
            };
            format!("{operator}{}", expr(value, 7))
        }
        ExprKind::Binary { op, left, right } => {
            let precedence = op.precedence();
            let rendered = format!(
                "{} {} {}",
                expr(left, precedence),
                op.text(),
                expr(right, precedence + 1)
            );
            if precedence < parent_precedence {
                format!("({rendered})")
            } else {
                rendered
            }
        }
        ExprKind::Block { statements, tail } => {
            let mut parts = statements
                .iter()
                .map(|statement| match statement {
                    Statement::Let { name, value, .. } => {
                        format!("let {name} = {};", expr(value, 0))
                    }
                })
                .collect::<Vec<_>>();
            parts.push(expr(tail, 0));
            format!("{{ {} }}", parts.join(" "))
        }
        ExprKind::If {
            condition,
            then_branch,
            else_branch,
        } => format!(
            "if {} {} else {}",
            expr(condition, 0),
            expr(then_branch, 0),
            expr(else_branch, 0)
        ),
        ExprKind::ConstructRecord {
            type_name, fields, ..
        } => {
            if fields.is_empty() {
                format!("{type_name} {{}}")
            } else {
                format!(
                    "{type_name} {{ {} }}",
                    fields
                        .iter()
                        .map(|field| format!("{}: {}", field.name, expr(&field.value, 0)))
                        .collect::<Vec<_>>()
                        .join(", ")
                )
            }
        }
        ExprKind::Project { base, field, .. } => {
            let base = match &base.kind {
                ExprKind::Binary { .. } | ExprKind::If { .. } | ExprKind::Block { .. } => {
                    format!("({})", expr(base, 0))
                }
                _ => expr(base, 8),
            };
            format!("{base}.{field}")
        }
    }
}

fn write_function_body(output: &mut String, body: &Expr) {
    writeln!(output, "{{").unwrap();
    if let ExprKind::Block { statements, tail } = &body.kind {
        for statement in statements {
            match statement {
                Statement::Let { name, value, .. } => {
                    writeln!(output, "    let {name} = {};", expr(value, 0)).unwrap();
                }
            }
        }
        writeln!(output, "    {}", expr(tail, 0)).unwrap();
    } else {
        writeln!(output, "    {}", expr(body, 0)).unwrap();
    }
    writeln!(output, "}}").unwrap();
}

fn escape_string(value: &str) -> String {
    value.replace('\\', "\\\\").replace('"', "\\\"")
}
