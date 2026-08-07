use std::fmt::Write;

use crate::ast::{Expr, ExprKind, Program, UnaryOp};

pub fn canonical(program: &Program) -> String {
    let mut output = String::new();
    writeln!(output, "module {};", program.module).unwrap();
    if !program.permits.is_empty() {
        writeln!(output, "\npermit {{ {} }}", program.permits.join(", ")).unwrap();
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
            write!(output, "{}: {}", param.name, param.ty).unwrap();
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
        writeln!(output, "{{").unwrap();
        writeln!(output, "    {}", expr(&function.body, 0)).unwrap();
        writeln!(output, "}}").unwrap();
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
    }
}

fn escape_string(value: &str) -> String {
    value.replace('\\', "\\\\").replace('"', "\\\"")
}
