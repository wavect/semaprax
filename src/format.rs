use std::fmt::Write;

use crate::ast::{
    Expr, ExprKind, ImportFailure, Program, ResourceLifecycleKind, Statement, TypeDeclarationKind,
    UnaryOp,
};

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
            TypeDeclarationKind::Resource { lifecycles } => {
                if lifecycles.is_empty() {
                    writeln!(output, "resource {};", declaration.name).unwrap();
                    continue;
                }
                writeln!(output, "resource {} {{", declaration.name).unwrap();
                for lifecycle in lifecycles {
                    if let Some(stable_id) = &lifecycle.stable_id {
                        writeln!(output, "    @id(\"{}\")", escape_string(stable_id)).unwrap();
                    }
                    match &lifecycle.kind {
                        ResourceLifecycleKind::Trivial => {
                            writeln!(output, "    drop trivial;").unwrap();
                        }
                        ResourceLifecycleKind::Imported { import_key } => {
                            writeln!(output, "    drop import \"{}\";", escape_string(import_key))
                                .unwrap();
                        }
                    }
                }
                writeln!(output, "}}").unwrap();
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
    for interface in &program.interfaces {
        writeln!(output).unwrap();
        if interface.explicit_id {
            writeln!(output, "@id(\"{}\")", escape_string(&interface.stable_id)).unwrap();
        }
        writeln!(output, "interface {}", interface.name).unwrap();
        writeln!(output, "    permits {{ {} }}", interface.permits.join(", ")).unwrap();
        writeln!(output, "{{").unwrap();
        for import in &interface.imports {
            if import.explicit_id {
                writeln!(output, "    @id(\"{}\")", escape_string(&import.stable_id)).unwrap();
            }
            write!(output, "    import fn {}(", import.name).unwrap();
            for (index, param) in import.params.iter().enumerate() {
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
            writeln!(output, ") -> unit").unwrap();
            writeln!(
                output,
                "        effects {{ {} }}",
                import.effects.join(", ")
            )
            .unwrap();
            match &import.failure {
                ImportFailure::Infallible => {
                    writeln!(output, "        failure infallible").unwrap();
                }
                ImportFailure::Status { domain_id } => {
                    writeln!(
                        output,
                        "        failure status \"{}\"",
                        escape_string(domain_id)
                    )
                    .unwrap();
                }
            }
            writeln!(output, "        consumes {} always;", import.consumes).unwrap();
        }
        writeln!(output, "}}").unwrap();
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
            writeln!(
                output,
                "    requires {}",
                record_literal_delimited_expr(contract)
            )
            .unwrap();
        }
        for contract in &function.ensures {
            writeln!(
                output,
                "    ensures {}",
                record_literal_delimited_expr(contract)
            )
            .unwrap();
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
            record_literal_delimited_expr(condition),
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

fn record_literal_delimited_expr(value: &Expr) -> String {
    let rendered = expr(value, 0);
    if contains_record_construction(value) {
        format!("({rendered})")
    } else {
        rendered
    }
}

fn contains_record_construction(value: &Expr) -> bool {
    match &value.kind {
        ExprKind::ConstructRecord { .. } => true,
        ExprKind::Call { args, .. } => args.iter().any(contains_record_construction),
        ExprKind::Unary { value, .. } | ExprKind::Project { base: value, .. } => {
            contains_record_construction(value)
        }
        ExprKind::Binary { left, right, .. } => {
            contains_record_construction(left) || contains_record_construction(right)
        }
        ExprKind::Block { statements, tail } => {
            statements.iter().any(|statement| match statement {
                Statement::Let { value, .. } => contains_record_construction(value),
            }) || contains_record_construction(tail)
        }
        ExprKind::If {
            condition,
            then_branch,
            else_branch,
        } => {
            contains_record_construction(condition)
                || contains_record_construction(then_branch)
                || contains_record_construction(else_branch)
        }
        ExprKind::Int(_) | ExprKind::Bool(_) | ExprKind::Var(_) => false,
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
