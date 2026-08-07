use std::collections::HashMap;
use std::fmt::Write;
use std::path::Path;
use std::process::Command;
use std::sync::atomic::{AtomicU64, Ordering};

use crate::ast::{BinaryOp, Expr, ExprKind, Program, Statement, Type, UnaryOp};
use crate::diagnostic::Diagnostic;

pub fn emit_c(program: &Program) -> Result<String, Diagnostic> {
    if let Some(error) = crate::verify::verify(program)
        .into_iter()
        .find(|item| item.severity.is_error())
    {
        return Err(error);
    }
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
            write!(output, "{}", c_type(&param.ty)).unwrap();
        }
        if function.params.is_empty() {
            output.push_str("void");
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
            write!(output, "{} spx_param_{}", c_type(&param.ty), param.name).unwrap();
        }
        if function.params.is_empty() {
            output.push_str("void");
        }
        output.push_str(") {\n");
        let returns = program
            .functions
            .iter()
            .map(|item| (item.name.clone(), item.return_type.clone()))
            .collect();
        let variables = function
            .params
            .iter()
            .map(|param| {
                (
                    param.name.clone(),
                    CBinding {
                        name: format!("spx_param_{}", param.name),
                        ty: param.ty.clone(),
                    },
                )
            })
            .collect();
        let mut emitter = CEmitter::new(&mut output, variables, returns);
        for param in &function.params {
            emitter.line(&format!("(void)spx_param_{};", param.name));
        }
        for contract in &function.requires {
            let condition = emitter.emit_expr(contract);
            emitter.line(&format!(
                "if (!({})) spx_contract_fail(\"requires\", \"{}\", \"{}\");",
                condition.code,
                function.name,
                c_string(&crate::format::expr(contract, 0))
            ));
        }
        let body = emitter.emit_expr(&function.body);
        emitter.line(&format!(
            "{} spx_result = {};",
            c_type(&function.return_type),
            body.code
        ));
        emitter.variables.insert(
            "result".to_owned(),
            CBinding {
                name: "spx_result".to_owned(),
                ty: function.return_type.clone(),
            },
        );
        for contract in &function.ensures {
            let condition = emitter.emit_expr(contract);
            emitter.line(&format!(
                "if (!({})) spx_contract_fail(\"ensures\", \"{}\", \"{}\");",
                condition.code,
                function.name,
                c_string(&crate::format::expr(contract, 0))
            ));
        }
        emitter.line("return spx_result;");
        drop(emitter);
        output.push_str("}\n\n");
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
    Ok(output)
}

pub fn build(program: &Program, output: &Path) -> Result<(), Diagnostic> {
    static BUILD_ID: AtomicU64 = AtomicU64::new(0);
    let build_id = BUILD_ID.fetch_add(1, Ordering::Relaxed);
    let c_path = std::env::temp_dir().join(format!(
        "semaprax-codegen-{}-{build_id}.c",
        std::process::id()
    ));
    std::fs::write(&c_path, emit_c(program)?).map_err(|error| {
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

#[derive(Clone)]
struct CBinding {
    name: String,
    ty: Type,
}

struct CValue {
    code: String,
    ty: Type,
}

struct CEmitter<'a> {
    output: &'a mut String,
    variables: HashMap<String, CBinding>,
    returns: HashMap<String, Type>,
    next_local: usize,
    indent: usize,
}

impl<'a> CEmitter<'a> {
    fn new(
        output: &'a mut String,
        variables: HashMap<String, CBinding>,
        returns: HashMap<String, Type>,
    ) -> Self {
        Self {
            output,
            variables,
            returns,
            next_local: 0,
            indent: 1,
        }
    }

    fn line(&mut self, value: &str) {
        for _ in 0..self.indent {
            self.output.push_str("    ");
        }
        writeln!(self.output, "{value}").unwrap();
    }

    fn temporary(&mut self, ty: &Type) -> String {
        let name = format!("spx_internal_{}", self.next_local);
        self.next_local += 1;
        self.line(&format!("{} {name};", c_type(ty)));
        name
    }

    fn emit_expr(&mut self, expr: &Expr) -> CValue {
        match &expr.kind {
            ExprKind::Int(value) => CValue {
                code: format!("INT64_C({value})"),
                ty: Type::I64,
            },
            ExprKind::Bool(value) => CValue {
                code: value.to_string(),
                ty: Type::Bool,
            },
            ExprKind::Var(name) => {
                let binding = self
                    .variables
                    .get(name)
                    .unwrap_or_else(|| panic!("verified C local `{name}` missing"));
                CValue {
                    code: binding.name.clone(),
                    ty: binding.ty.clone(),
                }
            }
            ExprKind::Call { name, args } => {
                let args = args
                    .iter()
                    .map(|arg| self.emit_expr(arg).code)
                    .collect::<Vec<_>>();
                let ty = self.returns[name].clone();
                let temporary = self.temporary(&ty);
                self.line(&format!(
                    "{temporary} = spx_user_{name}({});",
                    args.join(", ")
                ));
                CValue {
                    code: temporary,
                    ty,
                }
            }
            ExprKind::Unary { op, value } => {
                let value = self.emit_expr(value);
                let ty = match op {
                    UnaryOp::Neg => Type::I64,
                    UnaryOp::Not => Type::Bool,
                };
                let temporary = self.temporary(&ty);
                let operation = match op {
                    UnaryOp::Neg => format!("spx_rt_neg({})", value.code),
                    UnaryOp::Not => format!("(!{})", value.code),
                };
                self.line(&format!("{temporary} = {operation};"));
                CValue {
                    code: temporary,
                    ty,
                }
            }
            ExprKind::Binary { op, left, right } => self.emit_binary(*op, left, right),
            ExprKind::Block { statements, tail } => {
                let saved = self.variables.clone();
                for statement in statements {
                    match statement {
                        Statement::Let { name, value, .. } => {
                            let value = self.emit_expr(value);
                            let local = format!("spx_local_{}", self.next_local);
                            self.next_local += 1;
                            self.line(&format!("{} {local} = {};", c_type(&value.ty), value.code));
                            self.variables.insert(
                                name.clone(),
                                CBinding {
                                    name: local,
                                    ty: value.ty,
                                },
                            );
                        }
                    }
                }
                let value = self.emit_expr(tail);
                self.variables = saved;
                value
            }
            ExprKind::If {
                condition,
                then_branch,
                else_branch,
            } => {
                let condition = self.emit_expr(condition);
                let ty = self.infer_type(then_branch);
                let temporary = self.temporary(&ty);
                self.line(&format!("if ({}) {{", condition.code));
                self.indent += 1;
                let then_value = self.emit_expr(then_branch);
                self.line(&format!("{temporary} = {};", then_value.code));
                self.indent -= 1;
                self.line("} else {");
                self.indent += 1;
                let else_value = self.emit_expr(else_branch);
                self.line(&format!("{temporary} = {};", else_value.code));
                self.indent -= 1;
                self.line("}");
                CValue {
                    code: temporary,
                    ty,
                }
            }
        }
    }

    fn emit_binary(&mut self, op: BinaryOp, left: &Expr, right: &Expr) -> CValue {
        let left = self.emit_expr(left);
        if matches!(op, BinaryOp::And | BinaryOp::Or) {
            let temporary = self.temporary(&Type::Bool);
            if op == BinaryOp::And {
                self.line(&format!("if ({}) {{", left.code));
                self.indent += 1;
                let right = self.emit_expr(right);
                self.line(&format!("{temporary} = {};", right.code));
                self.indent -= 1;
                self.line("} else {");
                self.indent += 1;
                self.line(&format!("{temporary} = false;"));
            } else {
                self.line(&format!("if ({}) {{", left.code));
                self.indent += 1;
                self.line(&format!("{temporary} = true;"));
                self.indent -= 1;
                self.line("} else {");
                self.indent += 1;
                let right = self.emit_expr(right);
                self.line(&format!("{temporary} = {};", right.code));
            }
            self.indent -= 1;
            self.line("}");
            return CValue {
                code: temporary,
                ty: Type::Bool,
            };
        }
        let right = self.emit_expr(right);
        let ty = match op {
            BinaryOp::Add | BinaryOp::Sub | BinaryOp::Mul | BinaryOp::Div | BinaryOp::Rem => {
                Type::I64
            }
            _ => Type::Bool,
        };
        let temporary = self.temporary(&ty);
        let operation = match op {
            BinaryOp::Add => format!("spx_rt_add({}, {})", left.code, right.code),
            BinaryOp::Sub => format!("spx_rt_sub({}, {})", left.code, right.code),
            BinaryOp::Mul => format!("spx_rt_mul({}, {})", left.code, right.code),
            BinaryOp::Div => format!("spx_rt_div({}, {})", left.code, right.code),
            BinaryOp::Rem => format!("spx_rt_rem({}, {})", left.code, right.code),
            _ => format!("({} {} {})", left.code, op.text(), right.code),
        };
        self.line(&format!("{temporary} = {operation};"));
        CValue {
            code: temporary,
            ty,
        }
    }

    fn infer_type(&self, expr: &Expr) -> Type {
        infer_type(expr, &self.variables, &self.returns)
    }
}

fn infer_type(
    expr: &Expr,
    variables: &HashMap<String, CBinding>,
    returns: &HashMap<String, Type>,
) -> Type {
    match &expr.kind {
        ExprKind::Int(_) => Type::I64,
        ExprKind::Bool(_) => Type::Bool,
        ExprKind::Var(name) => variables[name].ty.clone(),
        ExprKind::Call { name, .. } => returns[name].clone(),
        ExprKind::Unary { op, .. } => match op {
            UnaryOp::Neg => Type::I64,
            UnaryOp::Not => Type::Bool,
        },
        ExprKind::Binary { op, .. } => match op {
            BinaryOp::Add | BinaryOp::Sub | BinaryOp::Mul | BinaryOp::Div | BinaryOp::Rem => {
                Type::I64
            }
            _ => Type::Bool,
        },
        ExprKind::Block { statements, tail } => {
            let mut scope = variables.clone();
            for statement in statements {
                match statement {
                    Statement::Let { name, value, .. } => {
                        let ty = infer_type(value, &scope, returns);
                        scope.insert(
                            name.clone(),
                            CBinding {
                                name: String::new(),
                                ty,
                            },
                        );
                    }
                }
            }
            infer_type(tail, &scope, returns)
        }
        ExprKind::If { then_branch, .. } => infer_type(then_branch, variables, returns),
    }
}

fn c_string(value: &str) -> String {
    value.replace('\\', "\\\\").replace('"', "\\\"")
}
