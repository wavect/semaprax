use std::collections::HashMap;
use std::fmt::Write;
use std::path::Path;
use std::process::Command;
use std::sync::atomic::{AtomicU64, Ordering};

use crate::ast::{BinaryOp, Program, UnaryOp};
use crate::diagnostic::Diagnostic;
use crate::hir::{
    self, DeclarationId, DeclarationKind, ExpressionId, ResolvedExpr, ResolvedExprKind,
    ResolvedFunction, ResolvedProgram, ResolvedStatement, ResolvedType,
    ResolvedTypeDeclarationKind, ValueId,
};

/// Resolve a parsed program fail-closed, then emit its checked native bootstrap IR.
pub fn emit_c(program: &Program) -> Result<String, Diagnostic> {
    let resolved = hir::resolve(program).map_err(first_backend_diagnostic)?;
    let labels = contract_labels(program, &resolved);
    emit_hir_c_with_labels(&resolved, &labels)
}

/// Emit C11 from resolved HIR.
///
/// This entry point exists so backend tests and future compiler stages can prove
/// that code generation consumes semantic identities and centralized type facts,
/// rather than reconstructing either from source names.
pub fn emit_hir_c(program: &ResolvedProgram) -> Result<String, Diagnostic> {
    emit_hir_c_with_labels(program, &HashMap::new())
}

fn first_backend_diagnostic(diagnostics: Vec<Diagnostic>) -> Diagnostic {
    diagnostics
        .iter()
        .find(|diagnostic| diagnostic.severity.is_error())
        .cloned()
        .or_else(|| diagnostics.into_iter().next())
        .unwrap_or_else(|| backend_error("HIR resolution failed without a diagnostic"))
}

fn contract_labels(program: &Program, resolved: &ResolvedProgram) -> HashMap<ExpressionId, String> {
    let mut labels = HashMap::new();
    for function in &resolved.functions {
        let Some(source) = program
            .functions
            .iter()
            .find(|candidate| candidate.stable_id == function.id.as_str())
        else {
            continue;
        };
        for (expression, source) in function.requires.iter().zip(&source.requires) {
            labels.insert(expression.id.clone(), crate::format::expr(source, 0));
        }
        for (expression, source) in function.ensures.iter().zip(&source.ensures) {
            labels.insert(expression.id.clone(), crate::format::expr(source, 0));
        }
    }
    labels
}

fn emit_hir_c_with_labels(
    program: &ResolvedProgram,
    contract_labels: &HashMap<ExpressionId, String>,
) -> Result<String, Diagnostic> {
    hir::validate(program)?;
    if program.types.iter().any(|declaration| {
        matches!(
            &declaration.kind,
            ResolvedTypeDeclarationKind::Record { .. }
        )
    }) {
        return Err(backend_error(
            "native record lowering is gated on aggregate cleanup and layout support",
        ));
    }
    if program.types.iter().any(|declaration| {
        matches!(
            &declaration.kind,
            ResolvedTypeDeclarationKind::Resource { .. }
        )
    }) {
        return Err(Diagnostic::io(
            "SPX-B104",
            "native resource lowering requires lifecycle declarations and the verified cleanup ABI",
        ));
    }
    let functions = function_index(program)?;
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
        let metadata = functions
            .get(&function.id)
            .ok_or_else(|| backend_error(format!("function `{}` is not indexed", function.id)))?;
        write!(
            output,
            "static __attribute__((unused)) {} {}(",
            c_type(program, &function.return_type)?,
            metadata.symbol
        )
        .expect("writing to a string cannot fail");
        for (index, param) in function.params.iter().enumerate() {
            if index > 0 {
                output.push_str(", ");
            }
            output.push_str(c_type(program, &param.ty)?);
        }
        if function.params.is_empty() {
            output.push_str("void");
        }
        output.push_str(");\n");
    }
    output.push('\n');

    for function in &program.functions {
        emit_function(&mut output, program, &functions, function, contract_labels)?;
    }

    let main = program
        .functions
        .iter()
        .find(|function| function.id == program.entrypoint)
        .ok_or_else(|| backend_error("resolved native entry point is not indexed"))?;
    if !main.params.is_empty() || main.return_type != ResolvedType::I64 {
        return Err(backend_error(
            "resolved native entry point must have type `fn main() -> i64`",
        ));
    }
    let symbol = &functions
        .get(&main.id)
        .ok_or_else(|| backend_error("native entry point is not indexed"))?
        .symbol;
    write!(
        output,
        "int main(void) {{\n    int64_t result = {symbol}();\n    printf(\"%lld\\n\", (long long)result);\n    return 0;\n}}\n"
    )
    .expect("writing to a string cannot fail");
    Ok(output)
}

fn emit_function(
    output: &mut String,
    program: &ResolvedProgram,
    functions: &HashMap<DeclarationId, CFunction>,
    function: &ResolvedFunction,
    contract_labels: &HashMap<ExpressionId, String>,
) -> Result<(), Diagnostic> {
    let metadata = functions
        .get(&function.id)
        .ok_or_else(|| backend_error(format!("function `{}` is not indexed", function.id)))?;
    write!(
        output,
        "static __attribute__((unused)) {} {}(",
        c_type(program, &function.return_type)?,
        metadata.symbol
    )
    .expect("writing to a string cannot fail");
    for (index, param) in function.params.iter().enumerate() {
        if index > 0 {
            output.push_str(", ");
        }
        write!(output, "{} spx_param_{index}", c_type(program, &param.ty)?)
            .expect("writing to a string cannot fail");
    }
    if function.params.is_empty() {
        output.push_str("void");
    }
    output.push_str(") {\n");

    let variables = function
        .params
        .iter()
        .enumerate()
        .map(|(index, param)| {
            (
                param.id.clone(),
                CBinding {
                    name: format!("spx_param_{index}"),
                    ty: param.ty.clone(),
                },
            )
        })
        .collect();
    let mut emitter = CEmitter::new(output, program, variables, functions);
    for index in 0..function.params.len() {
        emitter.line(&format!("(void)spx_param_{index};"));
    }
    for contract in &function.requires {
        let condition = emitter.emit_expr(contract)?;
        emitter.require_type(&condition.ty, &ResolvedType::Bool, "precondition")?;
        emitter.line(&format!(
            "if (!({})) spx_contract_fail(\"requires\", \"{}\", \"{}\");",
            condition.code,
            c_string(&function.name),
            c_string(contract_label(contract, contract_labels))
        ));
    }
    let body = emitter.emit_expr(&function.body)?;
    emitter.require_type(&body.ty, &function.return_type, "function body")?;
    emitter.line(&format!(
        "{} spx_result = {};",
        c_type(program, &function.return_type)?,
        body.code
    ));

    emitter.variables.insert(
        function.result_id.clone(),
        CBinding {
            name: "spx_result".to_owned(),
            ty: function.return_type.clone(),
        },
    );
    for contract in &function.ensures {
        let condition = emitter.emit_expr(contract)?;
        emitter.require_type(&condition.ty, &ResolvedType::Bool, "postcondition")?;
        emitter.line(&format!(
            "if (!({})) spx_contract_fail(\"ensures\", \"{}\", \"{}\");",
            condition.code,
            c_string(&function.name),
            c_string(contract_label(contract, contract_labels))
        ));
    }
    emitter.line("return spx_result;");
    drop(emitter);
    output.push_str("}\n\n");
    Ok(())
}

fn contract_label<'a>(
    expression: &'a ResolvedExpr,
    labels: &'a HashMap<ExpressionId, String>,
) -> &'a str {
    labels
        .get(&expression.id)
        .map_or_else(|| expression.id.as_str(), String::as_str)
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

#[derive(Clone)]
struct CFunction {
    symbol: String,
    params: Vec<ResolvedType>,
    return_type: ResolvedType,
}

fn function_index(
    program: &ResolvedProgram,
) -> Result<HashMap<DeclarationId, CFunction>, Diagnostic> {
    let mut functions = HashMap::new();
    for function in &program.functions {
        let declaration = program
            .declarations
            .declaration(&function.id)
            .ok_or_else(|| {
                backend_error(format!(
                    "resolved function `{}` has no declaration",
                    function.id
                ))
            })?;
        if declaration.kind != DeclarationKind::Function {
            return Err(backend_error(format!(
                "resolved callable `{}` does not refer to a function declaration",
                function.id
            )));
        }
        let metadata = CFunction {
            symbol: c_function_symbol(&function.id),
            params: function
                .params
                .iter()
                .map(|param| param.ty.clone())
                .collect(),
            return_type: function.return_type.clone(),
        };
        if functions.insert(function.id.clone(), metadata).is_some() {
            return Err(backend_error(format!(
                "duplicate resolved function identity `{}`",
                function.id
            )));
        }
    }
    Ok(functions)
}

fn c_function_symbol(id: &DeclarationId) -> String {
    let mut symbol = String::from("spx_decl_");
    for byte in id.as_str().bytes() {
        write!(symbol, "{byte:02x}").expect("writing to a string cannot fail");
    }
    symbol
}

fn c_type<'a>(program: &ResolvedProgram, ty: &ResolvedType) -> Result<&'a str, Diagnostic> {
    match ty {
        ResolvedType::I64 => Ok("int64_t"),
        ResolvedType::Bool => Ok("bool"),
        ResolvedType::Nominal { .. } => {
            let facts = program.declarations.type_facts(ty).ok_or_else(|| {
                backend_error(format!(
                    "semantic facts are unavailable for `{}`",
                    ty.identity_key()
                ))
            })?;
            if facts.contains_resource && facts.sized {
                Ok("void *")
            } else {
                Err(backend_error(format!(
                    "native representation is unavailable for `{}`",
                    ty.identity_key()
                )))
            }
        }
        ResolvedType::TypeParameter { .. } => Err(backend_error(format!(
            "native representation is unavailable for `{}`",
            ty.identity_key()
        ))),
    }
}

#[derive(Clone)]
struct CBinding {
    name: String,
    ty: ResolvedType,
}

struct CValue {
    code: String,
    ty: ResolvedType,
}

struct CEmitter<'a> {
    output: &'a mut String,
    program: &'a ResolvedProgram,
    variables: HashMap<ValueId, CBinding>,
    functions: &'a HashMap<DeclarationId, CFunction>,
    next_local: usize,
    indent: usize,
}

impl<'a> CEmitter<'a> {
    fn new(
        output: &'a mut String,
        program: &'a ResolvedProgram,
        variables: HashMap<ValueId, CBinding>,
        functions: &'a HashMap<DeclarationId, CFunction>,
    ) -> Self {
        Self {
            output,
            program,
            variables,
            functions,
            next_local: 0,
            indent: 1,
        }
    }

    fn line(&mut self, value: &str) {
        for _ in 0..self.indent {
            self.output.push_str("    ");
        }
        writeln!(self.output, "{value}").expect("writing to a string cannot fail");
    }

    fn temporary(&mut self, ty: &ResolvedType) -> Result<String, Diagnostic> {
        let name = format!("spx_internal_{}", self.next_local);
        self.next_local += 1;
        self.line(&format!("{} {name};", c_type(self.program, ty)?));
        Ok(name)
    }

    fn require_type(
        &self,
        actual: &ResolvedType,
        expected: &ResolvedType,
        context: &str,
    ) -> Result<(), Diagnostic> {
        if actual == expected {
            Ok(())
        } else {
            Err(backend_error(format!(
                "inconsistent HIR type for {context}: expected `{}`, found `{}`",
                expected.identity_key(),
                actual.identity_key()
            )))
        }
    }

    fn emit_expr(&mut self, expr: &ResolvedExpr) -> Result<CValue, Diagnostic> {
        let value = match &expr.kind {
            ResolvedExprKind::Int(value) => {
                self.require_type(&expr.ty, &ResolvedType::I64, "integer literal")?;
                CValue {
                    code: format!("INT64_C({value})"),
                    ty: ResolvedType::I64,
                }
            }
            ResolvedExprKind::Bool(value) => {
                self.require_type(&expr.ty, &ResolvedType::Bool, "boolean literal")?;
                CValue {
                    code: value.to_string(),
                    ty: ResolvedType::Bool,
                }
            }
            ResolvedExprKind::Place(place) => {
                if !place.projections.is_empty() {
                    return Err(backend_error(
                        "native aggregate place projections are not implemented",
                    ));
                }
                let binding = self.variables.get(&place.root).ok_or_else(|| {
                    backend_error(format!("resolved value `{}` is not in scope", place.root))
                })?;
                self.require_type(&expr.ty, &binding.ty, "place expression")?;
                CValue {
                    code: binding.name.clone(),
                    ty: binding.ty.clone(),
                }
            }
            ResolvedExprKind::Call { callee, args } => {
                let target = self.functions.get(callee).ok_or_else(|| {
                    backend_error(format!("resolved callee `{callee}` is not indexed"))
                })?;
                if args.len() != target.params.len() {
                    return Err(backend_error(format!(
                        "resolved call to `{callee}` has {} arguments; expected {}",
                        args.len(),
                        target.params.len()
                    )));
                }
                let target = target.clone();
                let mut arguments = Vec::with_capacity(args.len());
                for (index, (arg, expected)) in args.iter().zip(&target.params).enumerate() {
                    let argument = self.emit_expr(arg)?;
                    self.require_type(&argument.ty, expected, &format!("call argument {index}"))?;
                    arguments.push(argument.code);
                }
                self.require_type(&expr.ty, &target.return_type, "call result")?;
                let temporary = self.temporary(&target.return_type)?;
                self.line(&format!(
                    "{temporary} = {}({});",
                    target.symbol,
                    arguments.join(", ")
                ));
                CValue {
                    code: temporary,
                    ty: target.return_type,
                }
            }
            ResolvedExprKind::Unary { op, value } => {
                let value = self.emit_expr(value)?;
                let (ty, operand_type) = match op {
                    UnaryOp::Neg => (ResolvedType::I64, ResolvedType::I64),
                    UnaryOp::Not => (ResolvedType::Bool, ResolvedType::Bool),
                };
                self.require_type(&value.ty, &operand_type, "unary operand")?;
                self.require_type(&expr.ty, &ty, "unary result")?;
                let temporary = self.temporary(&ty)?;
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
            ResolvedExprKind::Binary { op, left, right } => {
                return self.emit_binary(*op, left, right, &expr.ty);
            }
            ResolvedExprKind::Block { statements, tail } => {
                let saved = self.variables.clone();
                for statement in statements {
                    match statement {
                        ResolvedStatement::Let { binding, value, .. } => {
                            let value = self.emit_expr(value)?;
                            self.require_type(&value.ty, &binding.ty, "local binding")?;
                            let local = format!("spx_local_{}", self.next_local);
                            self.next_local += 1;
                            self.line(&format!(
                                "{} {local} = {};",
                                c_type(self.program, &binding.ty)?,
                                value.code
                            ));
                            if self
                                .variables
                                .insert(
                                    binding.id.clone(),
                                    CBinding {
                                        name: local,
                                        ty: binding.ty.clone(),
                                    },
                                )
                                .is_some()
                            {
                                return Err(backend_error(format!(
                                    "duplicate resolved local identity `{}`",
                                    binding.id
                                )));
                            }
                        }
                    }
                }
                let tail = self.emit_expr(tail)?;
                self.require_type(&tail.ty, &expr.ty, "block result")?;
                self.variables = saved;
                tail
            }
            ResolvedExprKind::If {
                condition,
                then_branch,
                else_branch,
            } => {
                let condition = self.emit_expr(condition)?;
                self.require_type(&condition.ty, &ResolvedType::Bool, "if condition")?;
                let temporary = self.temporary(&expr.ty)?;
                self.line(&format!("if ({}) {{", condition.code));
                self.indent += 1;
                let then_value = self.emit_expr(then_branch)?;
                self.require_type(&then_value.ty, &expr.ty, "then branch")?;
                self.line(&format!("{temporary} = {};", then_value.code));
                self.indent -= 1;
                self.line("} else {");
                self.indent += 1;
                let else_value = self.emit_expr(else_branch)?;
                self.require_type(&else_value.ty, &expr.ty, "else branch")?;
                self.line(&format!("{temporary} = {};", else_value.code));
                self.indent -= 1;
                self.line("}");
                CValue {
                    code: temporary,
                    ty: expr.ty.clone(),
                }
            }
            ResolvedExprKind::ConstructRecord { .. } | ResolvedExprKind::Project { .. } => {
                return Err(backend_error(
                    "native record expressions require aggregate lowering",
                ));
            }
        };
        self.require_type(&value.ty, &expr.ty, "expression")?;
        Ok(value)
    }

    fn emit_binary(
        &mut self,
        op: BinaryOp,
        left: &ResolvedExpr,
        right: &ResolvedExpr,
        result_type: &ResolvedType,
    ) -> Result<CValue, Diagnostic> {
        let left = self.emit_expr(left)?;
        let operand_type = match op {
            BinaryOp::And | BinaryOp::Or => ResolvedType::Bool,
            BinaryOp::Eq | BinaryOp::Ne => left.ty.clone(),
            _ => ResolvedType::I64,
        };
        self.require_type(&left.ty, &operand_type, "binary left operand")?;
        let expected_result = match op {
            BinaryOp::Add | BinaryOp::Sub | BinaryOp::Mul | BinaryOp::Div | BinaryOp::Rem => {
                ResolvedType::I64
            }
            _ => ResolvedType::Bool,
        };
        self.require_type(result_type, &expected_result, "binary result")?;
        if matches!(op, BinaryOp::And | BinaryOp::Or) {
            let temporary = self.temporary(&ResolvedType::Bool)?;
            if op == BinaryOp::And {
                self.line(&format!("if ({}) {{", left.code));
                self.indent += 1;
                let right = self.emit_expr(right)?;
                self.require_type(&right.ty, &ResolvedType::Bool, "lazy right operand")?;
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
                let right = self.emit_expr(right)?;
                self.require_type(&right.ty, &ResolvedType::Bool, "lazy right operand")?;
                self.line(&format!("{temporary} = {};", right.code));
            }
            self.indent -= 1;
            self.line("}");
            return Ok(CValue {
                code: temporary,
                ty: ResolvedType::Bool,
            });
        }
        let right = self.emit_expr(right)?;
        self.require_type(&right.ty, &operand_type, "binary right operand")?;
        let temporary = self.temporary(&expected_result)?;
        let operation = match op {
            BinaryOp::Add => format!("spx_rt_add({}, {})", left.code, right.code),
            BinaryOp::Sub => format!("spx_rt_sub({}, {})", left.code, right.code),
            BinaryOp::Mul => format!("spx_rt_mul({}, {})", left.code, right.code),
            BinaryOp::Div => format!("spx_rt_div({}, {})", left.code, right.code),
            BinaryOp::Rem => format!("spx_rt_rem({}, {})", left.code, right.code),
            _ => format!("({} {} {})", left.code, op.text(), right.code),
        };
        self.line(&format!("{temporary} = {operation};"));
        Ok(CValue {
            code: temporary,
            ty: expected_result,
        })
    }
}

fn backend_error(message: impl Into<String>) -> Diagnostic {
    Diagnostic::io("SPX-B103", message)
}

fn c_string(value: &str) -> String {
    let mut escaped = String::with_capacity(value.len());
    for character in value.chars() {
        match character {
            '\\' => escaped.push_str("\\\\"),
            '"' => escaped.push_str("\\\""),
            '\n' => escaped.push_str("\\n"),
            '\r' => escaped.push_str("\\r"),
            '\t' => escaped.push_str("\\t"),
            value if value.is_control() => {
                let mut bytes = [0; 4];
                for byte in value.encode_utf8(&mut bytes).bytes() {
                    write!(escaped, "\\{byte:03o}").expect("writing to a string cannot fail");
                }
            }
            value => escaped.push(value),
        }
    }
    escaped
}
