mod native_adapter_abi;
#[cfg(not(any(target_arch = "wasm32", target_arch = "wasm64")))]
mod native_capability_authority;
mod native_capability_token;
mod native_cleanup;
mod native_cleanup_emit;
#[cfg(test)]
mod native_conformance;
#[cfg(test)]
mod native_conformance_materialize;
#[cfg(test)]
mod native_conformance_wire;
mod native_host_contract;
mod native_resource;
mod native_runtime;
mod native_trace;
mod native_trace_runtime;
mod native_value;

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
    let resource_abi = native_resource::build_resource_abi(program)?;
    let functions = function_index(program)?;
    if !resource_abi.resources.is_empty() {
        let _preflight =
            preflight_resource_lowering(program, &functions, &resource_abi, contract_labels);
        return Err(resource_lowering_gate());
    }
    let mut output = String::new();
    emit_native_prelude(&mut output, &resource_abi);
    emit_function_prototypes(&mut output, program, &functions, &resource_abi)?;

    for function in &program.functions {
        emit_function(
            &mut output,
            program,
            &resource_abi,
            &functions,
            function,
            contract_labels,
        )?;
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
        "#ifndef SPX_NO_ENTRY_WRAPPER\n\
         int main(void) {{\n\
             struct spx_status_entry spx_status_entries[UINT32_C(1)];\n\
             struct spx_context spx_ctx = {{0}};\n\
             if (!spx_context_init(&spx_ctx, UINT64_C(1), spx_status_entries, UINT32_C(1), NULL, NULL, NULL)) {{\n\
                 fputs(\"SEMAPRAX native runtime invariant failure: context initialization\\n\", stderr);\n\
                 return 72;\n\
             }}\n\
             int64_t result;\n\
             spx_status_token status = {symbol}(&spx_ctx, &result);\n\
             if (status != SPX_STATUS_SUCCESS) return spx_public_failure(&spx_ctx, status);\n\
             printf(\"%lld\\n\", (long long)result);\n\
             return 0;\n\
         }}\n\
         #endif\n"
    )
    .expect("writing to a string cannot fail");
    Ok(output)
}

fn emit_native_prelude(output: &mut String, resource_abi: &native_resource::NativeResourceAbi) {
    native_runtime::emit_status_runtime(output);
    output.push_str(&resource_abi.declarations);
    output.push_str("#include <stdio.h>\n\n");
    output.push_str(NATIVE_SCALAR_RUNTIME_C);
}

fn emit_function_prototypes(
    output: &mut String,
    program: &ResolvedProgram,
    functions: &HashMap<DeclarationId, CFunction>,
    resource_abi: &native_resource::NativeResourceAbi,
) -> Result<(), Diagnostic> {
    for function in &program.functions {
        let metadata = functions
            .get(&function.id)
            .ok_or_else(|| backend_error(format!("function `{}` is not indexed", function.id)))?;
        write!(
            output,
            "static __attribute__((unused)) spx_status_token {}(struct spx_context *spx_ctx",
            metadata.symbol,
        )
        .expect("writing to a string cannot fail");
        for param in &function.params {
            write!(output, ", {}", resource_abi.c_type(program, &param.ty)?)
                .expect("writing to a string cannot fail");
        }
        writeln!(
            output,
            ", {} *spx_result_out);",
            resource_abi.c_type(program, &function.return_type)?
        )
        .expect("writing to a string cannot fail");
    }
    output.push('\n');
    Ok(())
}

fn preflight_resource_lowering(
    program: &ResolvedProgram,
    functions: &HashMap<DeclarationId, CFunction>,
    resource_abi: &native_resource::NativeResourceAbi,
    contract_labels: &HashMap<ExpressionId, String>,
) -> Result<(), Diagnostic> {
    let mut first_failure = None;
    for function in &program.functions {
        match native_cleanup::classify(program, function) {
            Ok(cleanup) => {
                match native_value::plan(program, function, &cleanup, resource_abi, contract_labels)
                {
                    Ok(values) => {
                        match native_host_contract::derive_from_admitted(
                            program,
                            &function.id,
                            resource_abi,
                            &cleanup,
                            &values,
                        ) {
                            Ok(host_template) => {
                                match native_adapter_abi::derive(&host_template) {
                                    Ok(descriptor) => {
                                        let _descriptor_header =
                                            native_adapter_abi::emit_header(&descriptor);
                                        if let Err(diagnostic) = native_adapter_abi::emit_source(
                                            &descriptor,
                                            "semaprax_adapter_descriptor.h",
                                        ) {
                                            first_failure.get_or_insert(diagnostic);
                                        }
                                    }
                                    Err(diagnostic) => {
                                        first_failure.get_or_insert(diagnostic);
                                    }
                                }
                                let _declarations = native_value::emit_declarations(&values);
                                match native_cleanup_emit::emit_with_block_prologues(
                                    &cleanup,
                                    &values.cleanup_bindings,
                                    |block, output| {
                                        output.push_str(&native_value::emit_block_prologue(
                                            &values, block,
                                        ));
                                        Ok(())
                                    },
                                ) {
                                    Ok(_cleanup_body) => {}
                                    Err(diagnostic) => {
                                        first_failure.get_or_insert(diagnostic);
                                    }
                                }
                            }
                            Err(diagnostic) => {
                                first_failure.get_or_insert(diagnostic);
                            }
                        }
                    }
                    Err(diagnostic) => {
                        first_failure.get_or_insert(diagnostic);
                    }
                }
            }
            Err(diagnostic) => {
                first_failure.get_or_insert(diagnostic);
            }
        }
        if let Err(diagnostic) = native_trace::required_event_capacity(program, function) {
            first_failure.get_or_insert(diagnostic);
        }
    }

    // Exercise the same ABI declaration/type/prototype order that the eventual
    // resource emitter will use, then discard it. The loop above separately
    // constructs the gated value/cleanup bodies; the exact conformance harness
    // composes those inside strict C functions. No resource artifact may escape
    // until a public host ownership boundary is defined and proven.
    let mut staged_output = String::new();
    emit_native_prelude(&mut staged_output, resource_abi);
    if let Err(diagnostic) =
        emit_function_prototypes(&mut staged_output, program, functions, resource_abi)
    {
        first_failure.get_or_insert(diagnostic);
    }
    first_failure.map_or(Ok(()), Err)
}

fn resource_lowering_gate() -> Diagnostic {
    Diagnostic::io(
        "SPX-B104",
        "native resource lowering requires lifecycle declarations and the verified cleanup ABI",
    )
}

const NATIVE_SCALAR_RUNTIME_C: &str = r#"#include <stdlib.h>

static __attribute__((noreturn, unused)) void spx_runtime_invariant_failure(
    const char *message
) {
    fprintf(stderr, "SEMAPRAX native runtime invariant failure: %s\n", message);
    abort();
}

static __attribute__((unused)) spx_status_token spx_rt_arithmetic_failure(
    struct spx_context *spx_ctx,
    uint32_t code,
    const char *operation
) {
    spx_status_token token = SPX_STATUS_SUCCESS;
    if (!spx_status_record_arithmetic(spx_ctx, code, &token)) {
        spx_runtime_invariant_failure("status arena exhaustion");
    }
    struct spx_status_detail detail = {NULL, NULL, NULL, operation};
    if (!spx_status_attach_detail(spx_ctx, token, detail)) {
        spx_runtime_invariant_failure("arithmetic status detail attachment");
    }
    return token;
}

static __attribute__((unused)) spx_status_token spx_rt_contract(
    struct spx_context *spx_ctx,
    uint32_t code,
    const char *kind,
    const char *function,
    const char *expression
) {
    spx_status_token token = SPX_STATUS_SUCCESS;
    bool recorded = code == SPX_STATUS_CONTRACT_REQUIRES_FALSE
        ? spx_status_record_requires_false(spx_ctx, &token)
        : code == SPX_STATUS_CONTRACT_ENSURES_FALSE
            ? spx_status_record_ensures_false(spx_ctx, &token)
            : false;
    if (!recorded) spx_runtime_invariant_failure("status arena exhaustion");
    struct spx_status_detail detail = {kind, function, expression, NULL};
    if (!spx_status_attach_detail(spx_ctx, token, detail)) {
        spx_runtime_invariant_failure("contract status detail attachment");
    }
    return token;
}

static __attribute__((unused)) spx_status_token spx_rt_add(
    struct spx_context *spx_ctx, int64_t a, int64_t b, int64_t *result_out
) {
    int64_t result;
    if (__builtin_add_overflow(a, b, &result)) {
        return spx_rt_arithmetic_failure(
            spx_ctx, SPX_STATUS_ARITHMETIC_ADD_OVERFLOW, "addition overflow"
        );
    }
    *result_out = result;
    return SPX_STATUS_SUCCESS;
}

static __attribute__((unused)) spx_status_token spx_rt_sub(
    struct spx_context *spx_ctx, int64_t a, int64_t b, int64_t *result_out
) {
    int64_t result;
    if (__builtin_sub_overflow(a, b, &result)) {
        return spx_rt_arithmetic_failure(
            spx_ctx, SPX_STATUS_ARITHMETIC_SUB_OVERFLOW, "subtraction overflow"
        );
    }
    *result_out = result;
    return SPX_STATUS_SUCCESS;
}

static __attribute__((unused)) spx_status_token spx_rt_mul(
    struct spx_context *spx_ctx, int64_t a, int64_t b, int64_t *result_out
) {
    int64_t result;
    if (__builtin_mul_overflow(a, b, &result)) {
        return spx_rt_arithmetic_failure(
            spx_ctx, SPX_STATUS_ARITHMETIC_MUL_OVERFLOW, "multiplication overflow"
        );
    }
    *result_out = result;
    return SPX_STATUS_SUCCESS;
}

static __attribute__((unused)) spx_status_token spx_rt_div(
    struct spx_context *spx_ctx, int64_t a, int64_t b, int64_t *result_out
) {
    if (b == 0) {
        return spx_rt_arithmetic_failure(
            spx_ctx, SPX_STATUS_ARITHMETIC_DIVISION_BY_ZERO, "invalid division"
        );
    }
    if (a == INT64_MIN && b == -1) {
        return spx_rt_arithmetic_failure(
            spx_ctx, SPX_STATUS_ARITHMETIC_DIVISION_OVERFLOW, "invalid division"
        );
    }
    *result_out = a / b;
    return SPX_STATUS_SUCCESS;
}

static __attribute__((unused)) spx_status_token spx_rt_rem(
    struct spx_context *spx_ctx, int64_t a, int64_t b, int64_t *result_out
) {
    if (b == 0) {
        return spx_rt_arithmetic_failure(
            spx_ctx, SPX_STATUS_ARITHMETIC_REMAINDER_BY_ZERO, "invalid remainder"
        );
    }
    if (a == INT64_MIN && b == -1) {
        return spx_rt_arithmetic_failure(
            spx_ctx, SPX_STATUS_ARITHMETIC_REMAINDER_OVERFLOW, "invalid remainder"
        );
    }
    *result_out = a % b;
    return SPX_STATUS_SUCCESS;
}

static __attribute__((unused)) spx_status_token spx_rt_neg(
    struct spx_context *spx_ctx, int64_t value, int64_t *result_out
) {
    if (value == INT64_MIN) {
        return spx_rt_arithmetic_failure(
            spx_ctx, SPX_STATUS_ARITHMETIC_NEGATION_OVERFLOW, "negation overflow"
        );
    }
    *result_out = -value;
    return SPX_STATUS_SUCCESS;
}

static __attribute__((unused)) int spx_public_failure(
    const struct spx_context *spx_ctx,
    spx_status_token token
) {
    const struct spx_normalized_status *status = spx_status_resolve(spx_ctx, token);
    if (status == NULL) {
        fputs("SEMAPRAX native runtime invariant failure: invalid status token\n", stderr);
        return 72;
    }
    const struct spx_status_detail *detail = spx_status_resolve_detail(spx_ctx, token);
    if (status->status_class == SPX_STATUS_CLASS_CONTRACT) {
        if (detail == NULL || detail->failure_kind == NULL ||
            detail->failure_function == NULL || detail->failure_expression == NULL) {
            fputs("SEMAPRAX native runtime invariant failure: missing contract detail\n", stderr);
            return 72;
        }
        fprintf(
            stderr,
            "SEMAPRAX contract failure: %s in %s: %s\n",
            detail->failure_kind,
            detail->failure_function,
            detail->failure_expression
        );
        return 70;
    }
    if (status->status_class == SPX_STATUS_CLASS_ARITHMETIC) {
        if (detail == NULL || detail->failure_operation == NULL) {
            fputs("SEMAPRAX native runtime invariant failure: missing arithmetic detail\n", stderr);
            return 72;
        }
        fprintf(
            stderr,
            "SEMAPRAX checked arithmetic failure: %s\n",
            detail->failure_operation
        );
        return 71;
    }
    fprintf(
        stderr,
        "SEMAPRAX operation failure: %s/%u\n",
        status->domain_id,
        status->code
    );
    return 73;
}

"#;

fn emit_function(
    output: &mut String,
    program: &ResolvedProgram,
    resource_abi: &native_resource::NativeResourceAbi,
    functions: &HashMap<DeclarationId, CFunction>,
    function: &ResolvedFunction,
    contract_labels: &HashMap<ExpressionId, String>,
) -> Result<(), Diagnostic> {
    let metadata = functions
        .get(&function.id)
        .ok_or_else(|| backend_error(format!("function `{}` is not indexed", function.id)))?;
    write!(
        output,
        "static __attribute__((unused)) spx_status_token {}(struct spx_context *spx_ctx",
        metadata.symbol
    )
    .expect("writing to a string cannot fail");
    for (index, param) in function.params.iter().enumerate() {
        write!(
            output,
            ", {} spx_param_{index}",
            resource_abi.c_type(program, &param.ty)?
        )
        .expect("writing to a string cannot fail");
    }
    writeln!(
        output,
        ", {} *spx_result_out) {{",
        resource_abi.c_type(program, &function.return_type)?
    )
    .expect("writing to a string cannot fail");

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
    let mut emitter = CEmitter::new(output, program, resource_abi, variables, functions);
    emitter.line("spx_status_token spx_status = SPX_STATUS_SUCCESS;");
    emitter.line("(void)spx_ctx;");
    emitter.line(&format!(
        "{} spx_result = {{0}};",
        resource_abi.c_type(program, &function.return_type)?
    ));
    for index in 0..function.params.len() {
        emitter.line(&format!("(void)spx_param_{index};"));
    }
    for contract in &function.requires {
        let condition = emitter.emit_expr(contract)?;
        emitter.require_type(&condition.ty, &ResolvedType::Bool, "precondition")?;
        emitter.line(&format!("if (!({})) {{", condition.code));
        emitter.indent += 1;
        emitter.line(&format!(
            "spx_status = spx_rt_contract(spx_ctx, SPX_STATUS_CONTRACT_REQUIRES_FALSE, \"requires\", \"{}\", \"{}\");",
            c_string(&function.name),
            c_string(contract_label(contract, contract_labels))
        ));
        emitter.line("goto spx_epilogue;");
        emitter.indent -= 1;
        emitter.line("}");
    }
    let body = emitter.emit_expr(&function.body)?;
    emitter.require_type(&body.ty, &function.return_type, "function body")?;
    emitter.line(&format!("spx_result = {};", body.code));

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
        emitter.line(&format!("if (!({})) {{", condition.code));
        emitter.indent += 1;
        emitter.line(&format!(
            "spx_status = spx_rt_contract(spx_ctx, SPX_STATUS_CONTRACT_ENSURES_FALSE, \"ensures\", \"{}\", \"{}\");",
            c_string(&function.name),
            c_string(contract_label(contract, contract_labels))
        ));
        emitter.line("goto spx_epilogue;");
        emitter.indent -= 1;
        emitter.line("}");
    }
    emitter.line("goto spx_epilogue;");
    drop(emitter);
    output.push_str("spx_epilogue:\n");
    output.push_str("    if (spx_status != SPX_STATUS_SUCCESS) return spx_status;\n");
    output.push_str("    *spx_result_out = spx_result;\n");
    output.push_str("    return SPX_STATUS_SUCCESS;\n");
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

fn c_i64(value: i64) -> String {
    if value == i64::MIN {
        "(-INT64_C(9223372036854775807) - INT64_C(1))".to_owned()
    } else if value < 0 {
        format!("-INT64_C({})", value.unsigned_abs())
    } else {
        format!("INT64_C({value})")
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
    resource_abi: &'a native_resource::NativeResourceAbi,
    variables: HashMap<ValueId, CBinding>,
    functions: &'a HashMap<DeclarationId, CFunction>,
    next_local: usize,
    indent: usize,
}

impl<'a> CEmitter<'a> {
    fn new(
        output: &'a mut String,
        program: &'a ResolvedProgram,
        resource_abi: &'a native_resource::NativeResourceAbi,
        variables: HashMap<ValueId, CBinding>,
        functions: &'a HashMap<DeclarationId, CFunction>,
    ) -> Self {
        Self {
            output,
            program,
            resource_abi,
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
        self.line(&format!(
            "{} {name};",
            self.resource_abi.c_type(self.program, ty)?
        ));
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
                    code: c_i64(*value),
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
                    "spx_status = {}(spx_ctx{}{}, &{temporary});",
                    target.symbol,
                    if arguments.is_empty() { "" } else { ", " },
                    arguments.join(", ")
                ));
                self.line("if (spx_status != SPX_STATUS_SUCCESS) goto spx_epilogue;");
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
                match op {
                    UnaryOp::Neg => {
                        self.line(&format!(
                            "spx_status = spx_rt_neg(spx_ctx, {}, &{temporary});",
                            value.code
                        ));
                        self.line("if (spx_status != SPX_STATUS_SUCCESS) goto spx_epilogue;");
                    }
                    UnaryOp::Not => self.line(&format!("{temporary} = (!{});", value.code)),
                }
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
                                self.resource_abi.c_type(self.program, &binding.ty)?,
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
        if matches!(
            op,
            BinaryOp::Add | BinaryOp::Sub | BinaryOp::Mul | BinaryOp::Div | BinaryOp::Rem
        ) {
            let helper = match op {
                BinaryOp::Add => "spx_rt_add",
                BinaryOp::Sub => "spx_rt_sub",
                BinaryOp::Mul => "spx_rt_mul",
                BinaryOp::Div => "spx_rt_div",
                BinaryOp::Rem => "spx_rt_rem",
                _ => unreachable!("checked arithmetic operation was matched above"),
            };
            self.line(&format!(
                "spx_status = {helper}(spx_ctx, {}, {}, &{temporary});",
                left.code, right.code
            ));
            self.line("if (spx_status != SPX_STATUS_SUCCESS) goto spx_epilogue;");
        } else {
            self.line(&format!(
                "{temporary} = ({} {} {});",
                left.code,
                op.text(),
                right.code
            ));
        }
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
    for byte in value.as_bytes() {
        match *byte {
            b'\\' => escaped.push_str("\\\\"),
            b'"' => escaped.push_str("\\\""),
            b'\n' => escaped.push_str("\\n"),
            b'\r' => escaped.push_str("\\r"),
            b'\t' => escaped.push_str("\\t"),
            b'?' | 0x00..=0x1f | 0x7f..=0xff => {
                write!(escaped, "\\{byte:03o}").expect("writing to a string cannot fail");
            }
            value => escaped.push(char::from(value)),
        }
    }
    escaped
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use crate::{hir, parse};
    use sha2::{Digest, Sha256};

    use super::*;

    const RESOURCE_SOURCE: &str = r#"module test.native_resource_types;

@id("token.type")
resource Token {
    @id("token.drop")
    drop trivial;
}

@id("token.identity")
fn identity(value: own Token) -> Token { value }

@id("app.main")
fn main() -> i64 { 0 }
"#;

    fn resolved_resource_program() -> ResolvedProgram {
        let parsed = parse(
            RESOURCE_SOURCE,
            Path::new("native-resource-type-selection.spx"),
        )
        .unwrap();
        hir::resolve(&parsed).unwrap()
    }

    #[test]
    fn scalar_c_output_matches_the_committed_pre_resource_template_baseline() {
        let parsed = parse(
            r#"module test.native_scalar_baseline;

@id("math.increment")
fn increment(value: i64) -> i64 { value + 1 }

@id("app.main")
fn main() -> i64 { increment(41) }
"#,
            Path::new("native-scalar-baseline.spx"),
        )
        .unwrap();
        let generated = emit_c(&parsed).unwrap();
        let digest = format!("{:x}", Sha256::digest(generated.as_bytes()));
        assert_eq!(
            digest,
            "c29ee66e0acd1bb4a936c5307c54172e6617432a279d359c244b5632557ccd2f"
        );
    }

    #[test]
    fn direct_resource_parameters_and_results_use_the_stable_wrapper_type() {
        let program = resolved_resource_program();
        let resource_abi = native_resource::build_resource_abi(&program).unwrap();
        let functions = function_index(&program).unwrap();
        let wrapper = &resource_abi.resources[0].c_type;
        let mut output = String::new();
        emit_native_prelude(&mut output, &resource_abi);
        emit_function_prototypes(&mut output, &program, &functions, &resource_abi).unwrap();

        let identity_symbol = c_function_symbol(&DeclarationId::new("token.identity"));
        let prototype = output
            .lines()
            .find(|line| line.contains(&identity_symbol))
            .unwrap();
        assert!(prototype.contains(&format!(
            "{identity_symbol}(struct spx_context *spx_ctx, {wrapper}, {wrapper} *spx_result_out);"
        )));
        assert!(!prototype.contains("void *"));
        assert!(
            output
                .find("/* semaprax.native-resource-abi.v1 */")
                .unwrap()
                < output.find(&identity_symbol).unwrap()
        );

        let mut second = String::new();
        emit_native_prelude(&mut second, &resource_abi);
        emit_function_prototypes(&mut second, &program, &functions, &resource_abi).unwrap();
        assert_eq!(output, second);
    }

    #[test]
    fn public_resource_emission_runs_preflight_but_remains_b104_gated() {
        let parsed = parse(
            RESOURCE_SOURCE,
            Path::new("native-resource-public-gate.spx"),
        )
        .unwrap();
        let program = hir::resolve(&parsed).unwrap();
        let resource_abi = native_resource::build_resource_abi(&program).unwrap();
        let functions = function_index(&program).unwrap();
        preflight_resource_lowering(&program, &functions, &resource_abi, &HashMap::new()).unwrap();

        let diagnostic = emit_c(&parsed).unwrap_err();
        assert_eq!(diagnostic.code, "SPX-B104");
        assert_eq!(
            diagnostic.message,
            "native resource lowering requires lifecycle declarations and the verified cleanup ABI"
        );
    }

    #[test]
    fn resource_value_preflight_rejects_unstaged_borrow_without_changing_public_gate() {
        let source = RESOURCE_SOURCE.replace(
            "@id(\"token.identity\")\nfn identity(value: own Token) -> Token { value }",
            "@id(\"token.observe\")\nfn observe(value: borrow Token) -> i64 { 0 }",
        );
        let parsed = parse(
            &source,
            Path::new("native-resource-borrow-value-preflight.spx"),
        )
        .unwrap();
        let program = hir::resolve(&parsed).unwrap();
        let resource_abi = native_resource::build_resource_abi(&program).unwrap();
        let functions = function_index(&program).unwrap();
        let preflight =
            preflight_resource_lowering(&program, &functions, &resource_abi, &HashMap::new())
                .unwrap_err();
        assert_eq!(preflight.code, "SPX-B104");
        assert!(preflight.message.contains("resource parameter"));

        let public = emit_c(&parsed).unwrap_err();
        let gate = resource_lowering_gate();
        assert_eq!(public.code, gate.code);
        assert_eq!(public.message, gate.message);
    }

    #[test]
    fn c_literals_preserve_utf8_bytes_without_exposing_trigraphs() {
        static NEXT_FIXTURE: AtomicU64 = AtomicU64::new(0);

        if Command::new("clang").arg("--version").output().is_err() {
            return;
        }

        let escaped = c_string("??/λ\n\r\t\\\"\u{7f}");
        assert_eq!(escaped, "\\077\\077/\\316\\273\\n\\r\\t\\\\\\\"\\177");
        assert!(!escaped.contains("??"));
        assert!(escaped.is_ascii());
        assert_eq!(
            c_i64(i64::MIN),
            "(-INT64_C(9223372036854775807) - INT64_C(1))"
        );
        assert_eq!(c_i64(-42), "-INT64_C(42)");
        assert_eq!(c_i64(42), "INT64_C(42)");

        let source = format!(
            "#include <stddef.h>\n\
             #include <stdint.h>\n\
             static const unsigned char value[] = \"{escaped}\";\n\
             static const unsigned char expected[] = {{0x3f, 0x3f, 0x2f, 0xce, 0xbb, 0x0a, 0x0d, 0x09, 0x5c, 0x22, 0x7f, 0x00}};\n\
             static const int64_t minimum = {};\n\
             static const int64_t negative = {};\n\
             int main(void) {{\n\
                 if (sizeof(value) != sizeof(expected)) return 1;\n\
                 for (size_t index = 0; index < sizeof(expected); ++index) {{\n\
                     if (value[index] != expected[index]) return 2;\n\
                 }}\n\
                 if (minimum != INT64_MIN || negative != -INT64_C(42)) return 3;\n\
                 return 0;\n\
             }}\n",
            c_i64(i64::MIN),
            c_i64(-42),
        );
        assert!(!source.contains("??"));

        let suffix = NEXT_FIXTURE.fetch_add(1, Ordering::Relaxed);
        let c_path = std::env::temp_dir().join(format!(
            "semaprax-c-string-{}-{suffix}.c",
            std::process::id()
        ));
        let binary = std::env::temp_dir().join(format!(
            "semaprax-c-string-{}-{suffix}{}",
            std::process::id(),
            std::env::consts::EXE_SUFFIX
        ));
        std::fs::write(&c_path, source).expect("write strict C string fixture");
        let compiled = Command::new("clang")
            .args([
                "-std=c11",
                "-O2",
                "-Wall",
                "-Wextra",
                "-Werror",
                "-Wtrigraphs",
                "-Wimplicitly-unsigned-literal",
            ])
            .arg(&c_path)
            .arg("-o")
            .arg(&binary)
            .output()
            .expect("clang was available during the version probe");
        let _ = std::fs::remove_file(&c_path);
        assert!(
            compiled.status.success(),
            "strict C11 compilation failed:\n{}",
            String::from_utf8_lossy(&compiled.stderr)
        );
        let run = Command::new(&binary)
            .output()
            .expect("run C string fixture");
        let _ = std::fs::remove_file(&binary);
        assert!(
            run.status.success(),
            "C string fixture exited {}",
            run.status
        );
    }
}
