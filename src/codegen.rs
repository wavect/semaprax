use std::fmt::Write;
use std::path::Path;
use std::process::Command;
use std::sync::atomic::{AtomicU64, Ordering};

use crate::ast::{BinaryOp, Expr, ExprKind, Program, Type, UnaryOp};
use crate::diagnostic::Diagnostic;

pub fn emit_c(program: &Program) -> String {
    let mut output = String::from(
        "#include <stdbool.h>\n#include <stdint.h>\n#include <stdio.h>\n#include <stdlib.h>\n\n",
    );
    output.push_str(
        "static __attribute__((unused)) void spx_contract_fail(const char *kind, const char *function, const char *expression) {\n\
         fprintf(stderr, \"SEMAPRAX contract failure: %s in %s: %s\\n\", kind, function, expression);\n\
         exit(70);\n}\n\n",
    );
    output.push_str(
        "static __attribute__((unused)) void spx_arithmetic_fail(const char *operation) {\n\
         fprintf(stderr, \"SEMAPRAX checked arithmetic failure: %s\\n\", operation);\n\
         exit(71);\n}\n\n\
         static __attribute__((unused)) int64_t spx_rt_add(int64_t a, int64_t b) { int64_t r; if (__builtin_add_overflow(a, b, &r)) spx_arithmetic_fail(\"addition overflow\"); return r; }\n\
         static __attribute__((unused)) int64_t spx_rt_sub(int64_t a, int64_t b) { int64_t r; if (__builtin_sub_overflow(a, b, &r)) spx_arithmetic_fail(\"subtraction overflow\"); return r; }\n\
         static __attribute__((unused)) int64_t spx_rt_mul(int64_t a, int64_t b) { int64_t r; if (__builtin_mul_overflow(a, b, &r)) spx_arithmetic_fail(\"multiplication overflow\"); return r; }\n\
         static __attribute__((unused)) int64_t spx_rt_div(int64_t a, int64_t b) { if (b == 0 || (a == INT64_MIN && b == -1)) spx_arithmetic_fail(\"invalid division\"); return a / b; }\n\
         static __attribute__((unused)) int64_t spx_rt_rem(int64_t a, int64_t b) { if (b == 0 || (a == INT64_MIN && b == -1)) spx_arithmetic_fail(\"invalid remainder\"); return a % b; }\n\
         static __attribute__((unused)) int64_t spx_rt_neg(int64_t value) { if (value == INT64_MIN) spx_arithmetic_fail(\"negation overflow\"); return -value; }\n\n",
    );
    for function in &program.functions {
        write!(
            output,
            "static __attribute__((unused)) {} spx_user_{}(",
            c_type(&function.return_type),
            function.name
        )
        .unwrap();
        for (index, param) in function.params.iter().enumerate() {
            if index > 0 {
                output.push_str(", ");
            }
            write!(output, "{} {}", c_type(&param.ty), param.name).unwrap();
        }
        output.push_str(");\n");
    }
    output.push('\n');
    for function in &program.functions {
        write!(
            output,
            "static __attribute__((unused)) {} spx_user_{}(",
            c_type(&function.return_type),
            function.name
        )
        .unwrap();
        for (index, param) in function.params.iter().enumerate() {
            if index > 0 {
                output.push_str(", ");
            }
            write!(output, "{} {}", c_type(&param.ty), param.name).unwrap();
        }
        output.push_str(") {\n");
        for param in &function.params {
            writeln!(output, "    (void){};", param.name).unwrap();
        }
        for contract in &function.requires {
            writeln!(
                output,
                "    if (!({})) spx_contract_fail(\"requires\", \"{}\", \"{}\");",
                c_expr(contract),
                function.name,
                c_string(&crate::format::expr(contract, 0))
            )
            .unwrap();
        }
        writeln!(
            output,
            "    {} result = {};",
            c_type(&function.return_type),
            c_expr(&function.body)
        )
        .unwrap();
        for contract in &function.ensures {
            writeln!(
                output,
                "    if (!({})) spx_contract_fail(\"ensures\", \"{}\", \"{}\");",
                c_expr(contract),
                function.name,
                c_string(&crate::format::expr(contract, 0))
            )
            .unwrap();
        }
        output.push_str("    return result;\n}\n\n");
    }
    if program
        .functions
        .iter()
        .any(|function| function.name == "main")
    {
        output.push_str(
            "int main(void) {\n\
             int64_t result = spx_user_main();\n\
             printf(\"%lld\\n\", (long long)result);\n\
             return 0;\n}\n",
        );
    }
    output
}

pub fn build(program: &Program, output: &Path) -> Result<(), Diagnostic> {
    static BUILD_ID: AtomicU64 = AtomicU64::new(0);
    let build_id = BUILD_ID.fetch_add(1, Ordering::Relaxed);
    let c_path = std::env::temp_dir().join(format!(
        "semaprax-codegen-{}-{build_id}.c",
        std::process::id()
    ));
    std::fs::write(&c_path, emit_c(program)).map_err(|error| {
        Diagnostic::io(
            "SPX-I101",
            format!(
                "cannot write temporary C source {}: {error}",
                c_path.display()
            ),
        )
    })?;
    let result = Command::new("clang")
        .args(["-std=c11", "-O2", "-Wall", "-Wextra", "-Werror"])
        .arg(&c_path)
        .arg("-o")
        .arg(output)
        .output()
        .map_err(|error| {
            Diagnostic::io(
                "SPX-B101",
                format!("failed to start clang; install a C11 toolchain: {error}"),
            )
        })?;
    let _ = std::fs::remove_file(&c_path);
    if !result.status.success() {
        return Err(Diagnostic::io(
            "SPX-B102",
            format!(
                "native backend failed:\n{}",
                String::from_utf8_lossy(&result.stderr)
            ),
        ));
    }
    Ok(())
}

fn c_type(ty: &Type) -> &'static str {
    match ty {
        Type::I64 => "int64_t",
        Type::Bool => "bool",
        Type::Resource(_) => "void *",
    }
}

fn c_expr(expr: &Expr) -> String {
    match &expr.kind {
        ExprKind::Int(value) => format!("INT64_C({value})"),
        ExprKind::Bool(value) => value.to_string(),
        ExprKind::Var(name) => name.clone(),
        ExprKind::Call { name, args } => format!(
            "spx_user_{}({})",
            name,
            args.iter().map(c_expr).collect::<Vec<_>>().join(", ")
        ),
        ExprKind::Unary {
            op: UnaryOp::Neg,
            value,
        } => format!("spx_rt_neg({})", c_expr(value)),
        ExprKind::Unary {
            op: UnaryOp::Not,
            value,
        } => format!("(!{})", c_expr(value)),
        ExprKind::Binary { op, left, right } => {
            let left = c_expr(left);
            let right = c_expr(right);
            match op {
                BinaryOp::Add => format!("spx_rt_add({left}, {right})"),
                BinaryOp::Sub => format!("spx_rt_sub({left}, {right})"),
                BinaryOp::Mul => format!("spx_rt_mul({left}, {right})"),
                BinaryOp::Div => format!("spx_rt_div({left}, {right})"),
                BinaryOp::Rem => format!("spx_rt_rem({left}, {right})"),
                BinaryOp::And => format!("({left} && {right})"),
                BinaryOp::Or => format!("({left} || {right})"),
                _ => format!("({left} {} {right})", op.text()),
            }
        }
    }
}

fn c_string(value: &str) -> String {
    value.replace('\\', "\\\\").replace('"', "\\\"")
}
