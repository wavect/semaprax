use std::collections::HashMap;
use std::path::Path;

use sha2::{Digest, Sha256};

use crate::ast::{BinaryOp, Program, UnaryOp};
use crate::diagnostic::{quote_json, Diagnostic};
use crate::graph;
use crate::hir::{
    self, DeclarationId, IdentityOrigin, ResolvedExpr, ResolvedExprKind, ResolvedProgram,
    ResolvedStatement, ResolvedType, ResolvedTypeDeclarationKind, ValueId,
};
use crate::variant_layout::{VariantLayoutCache, VariantTarget};

mod aggregate;
mod owned;
#[cfg(any(test, feature = "unstable-wit-component-harness"))]
mod result_component_v3;
#[cfg(any(test, feature = "unstable-wit-component-harness"))]
mod source_result_component_v4;

#[cfg(any(test, feature = "unstable-wit-component-harness"))]
pub(crate) use result_component_v3::{
    emit_private_result_core_v3, CANONICAL_EXPORT as RESULT_COMPONENT_CANONICAL_EXPORT_V3,
    STATUS_OUT_EXPORT as RESULT_COMPONENT_STATUS_OUT_EXPORT_V3,
};
#[cfg(any(test, feature = "unstable-wit-component-harness"))]
pub(crate) use source_result_component_v4::{
    emit_private_source_result_core_v4,
    CANONICAL_EXPORT as SOURCE_RESULT_COMPONENT_CANONICAL_EXPORT_V4,
    STATUS_OUT_EXPORT as SOURCE_RESULT_COMPONENT_STATUS_OUT_EXPORT_V4,
};

const I32: u8 = 0x7f;
const I64: u8 = 0x7e;
const SCALAR_IMPORT_COUNT: u32 = 7;

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub(super) struct Signature {
    pub(super) params: Vec<u8>,
    pub(super) results: Vec<u8>,
}

#[derive(Default)]
struct LocalLayout {
    declarations: Vec<ResolvedType>,
    lets: HashMap<ValueId, u32>,
}

pub fn emit_module(program: &Program) -> Result<Vec<u8>, Diagnostic> {
    let resolved = hir::resolve(program).map_err(|diagnostics| {
        diagnostics
            .into_iter()
            .find(|item| item.severity.is_error())
            .unwrap_or_else(|| Diagnostic::io("SPX-W100", "HIR resolution failed"))
    })?;
    emit_resolved_module(&resolved)
}

/// Emit a WebAssembly core module from verified, identity-resolved HIR.
///
/// Most callers should use [`emit_module`], which resolves and verifies parsed
/// source first. This entry point exists for semantic consumers that already
/// hold HIR and keeps all backend lowering independent of source-level names.
pub fn emit_resolved_module(program: &ResolvedProgram) -> Result<Vec<u8>, Diagnostic> {
    hir::validate(program)?;
    let concrete_variants = VariantLayoutCache::build(program, VariantTarget::Wasm32)?;
    let has_authored_aggregate = program.types.iter().any(|declaration| {
        matches!(
            &declaration.kind,
            ResolvedTypeDeclarationKind::Record { .. }
                | ResolvedTypeDeclarationKind::Variant { .. }
        ) && !program
            .declarations
            .declaration(&declaration.id)
            .is_some_and(|item| item.identity_origin == IdentityOrigin::CompilerOwned)
    });
    if has_authored_aggregate || !concrete_variants.is_empty() {
        return aggregate::emit(program);
    }
    let owned_plans = owned::plan(program)?;
    let import_count = if owned_plans.is_empty() {
        SCALAR_IMPORT_COUNT
    } else {
        SCALAR_IMPORT_COUNT + owned::IMPORT_NAMES.len() as u32
    };
    let mut types = Vec::<Signature>::new();
    let mut type_indexes = HashMap::<Signature, u32>::new();
    let binary_checked = intern_type(
        Signature {
            params: vec![I64, I64],
            results: vec![I64],
        },
        &mut types,
        &mut type_indexes,
    );
    let unary_checked = intern_type(
        Signature {
            params: vec![I64],
            results: vec![I64],
        },
        &mut types,
        &mut type_indexes,
    );
    let contract_fail = intern_type(
        Signature {
            params: vec![],
            results: vec![],
        },
        &mut types,
        &mut type_indexes,
    );

    let owned_import_types = if owned_plans.is_empty() {
        None
    } else {
        Some([
            intern_type(
                Signature {
                    params: vec![I32],
                    results: vec![I32],
                },
                &mut types,
                &mut type_indexes,
            ),
            intern_type(
                Signature {
                    params: vec![I32, I32],
                    results: vec![I32],
                },
                &mut types,
                &mut type_indexes,
            ),
            intern_type(
                Signature {
                    params: vec![I32],
                    results: vec![],
                },
                &mut types,
                &mut type_indexes,
            ),
            intern_type(
                Signature {
                    params: vec![I32],
                    results: vec![I32],
                },
                &mut types,
                &mut type_indexes,
            ),
            intern_type(
                Signature {
                    params: vec![I32],
                    results: vec![I32],
                },
                &mut types,
                &mut type_indexes,
            ),
            intern_type(
                Signature {
                    params: vec![I32, I32],
                    results: vec![],
                },
                &mut types,
                &mut type_indexes,
            ),
            intern_type(
                Signature {
                    params: vec![I32],
                    results: vec![],
                },
                &mut types,
                &mut type_indexes,
            ),
            intern_type(
                Signature {
                    params: vec![I32, I32],
                    results: vec![I32],
                },
                &mut types,
                &mut type_indexes,
            ),
            intern_type(
                Signature {
                    params: vec![I32, I32, I32],
                    results: vec![I32],
                },
                &mut types,
                &mut type_indexes,
            ),
            intern_type(
                Signature {
                    params: vec![I32],
                    results: vec![],
                },
                &mut types,
                &mut type_indexes,
            ),
            intern_type(
                Signature {
                    params: vec![I32, I32, I32],
                    results: vec![],
                },
                &mut types,
                &mut type_indexes,
            ),
        ])
    };

    let mut function_types = Vec::new();
    for function in &program.functions {
        let signature = Signature {
            params: function
                .params
                .iter()
                .map(|param| wasm_type(&param.ty))
                .collect::<Result<Vec<_>, _>>()?,
            results: vec![wasm_type(&function.return_type)?],
        };
        function_types.push(intern_type(signature, &mut types, &mut type_indexes));
    }
    let owned_function_types = owned_plans
        .iter()
        .map(|plan| {
            let (params, results) = plan.signature();
            intern_type(Signature { params, results }, &mut types, &mut type_indexes)
        })
        .collect::<Vec<_>>();

    let function_indexes: HashMap<_, _> = program
        .functions
        .iter()
        .enumerate()
        .map(|(index, function)| (function.id.clone(), import_count + index as u32))
        .collect();

    let mut module = b"\0asm\x01\0\0\0".to_vec();
    let mut type_section = Vec::new();
    write_u32(&mut type_section, types.len() as u32);
    for signature in &types {
        type_section.push(0x60);
        write_bytes(&mut type_section, &signature.params);
        write_bytes(&mut type_section, &signature.results);
    }
    section(&mut module, 1, type_section);

    let mut imports = Vec::new();
    write_u32(&mut imports, import_count);
    for name in ["spx_add", "spx_sub", "spx_mul", "spx_div", "spx_rem"] {
        function_import(&mut imports, "env", name, binary_checked);
    }
    function_import(&mut imports, "env", "spx_neg", unary_checked);
    function_import(&mut imports, "env", "spx_contract_fail", contract_fail);
    if let Some(type_indexes) = owned_import_types {
        for (name, type_index) in owned::IMPORT_NAMES.into_iter().zip(type_indexes) {
            function_import(&mut imports, "env", name, type_index);
        }
    }
    section(&mut module, 2, imports);

    let mut functions = Vec::new();
    write_u32(
        &mut functions,
        (function_types.len() + owned_function_types.len()) as u32,
    );
    for type_index in function_types {
        write_u32(&mut functions, type_index);
    }
    for type_index in owned_function_types {
        write_u32(&mut functions, type_index);
    }
    section(&mut module, 3, functions);

    if !owned_plans.is_empty() {
        let mut memories = Vec::new();
        write_u32(&mut memories, 1);
        memories.extend([0x00, 0x01]); // one-page, unbounded memory
        section(&mut module, 5, memories);
    }

    let main_index = program
        .functions
        .iter()
        .position(|function| function.id == program.entrypoint)
        .ok_or_else(|| Diagnostic::io("SPX-W101", "web target requires a main function"))?;
    let main = &program.functions[main_index];
    if !main.params.is_empty() || main.return_type != ResolvedType::I64 {
        return Err(Diagnostic::io(
            "SPX-W101",
            "resolved web entry point must have type `fn main() -> i64`",
        ));
    }
    let mut exports = Vec::new();
    write_u32(
        &mut exports,
        1 + owned_plans.len() as u32 + u32::from(!owned_plans.is_empty()),
    );
    write_name(&mut exports, "semaprax_main");
    exports.push(0x00);
    write_u32(&mut exports, import_count + main_index as u32);
    if !owned_plans.is_empty() {
        write_name(&mut exports, "memory");
        exports.push(0x02);
        write_u32(&mut exports, 0);
    }
    let adapter_base = import_count + program.functions.len() as u32;
    for (ordinal, plan) in owned_plans.iter().enumerate() {
        write_name(&mut exports, &plan.export);
        exports.push(0x00);
        write_u32(&mut exports, adapter_base + ordinal as u32);
    }
    section(&mut module, 7, exports);

    let mut code = Vec::new();
    write_u32(
        &mut code,
        (program.functions.len() + owned_plans.len()) as u32,
    );
    for function in &program.functions {
        let mut body = Vec::new();
        let result_local = function.params.len() as u32;
        let mut value_indexes: HashMap<_, _> = function
            .params
            .iter()
            .enumerate()
            .map(|(index, param)| (param.id.clone(), index as u32))
            .collect();
        let mut layout = LocalLayout {
            declarations: vec![function.return_type.clone()],
            lets: HashMap::new(),
        };
        for contract in &function.requires {
            collect_locals(contract, function.params.len() as u32, &mut layout)?;
        }
        collect_locals(&function.body, function.params.len() as u32, &mut layout)?;
        for contract in &function.ensures {
            collect_locals(contract, function.params.len() as u32, &mut layout)?;
        }
        value_indexes.extend(layout.lets.iter().map(|(id, index)| (id.clone(), *index)));
        value_indexes.insert(function.result_id.clone(), result_local);
        write_u32(&mut body, layout.declarations.len() as u32);
        for ty in &layout.declarations {
            write_u32(&mut body, 1);
            body.push(wasm_type(ty)?);
        }
        for contract in &function.requires {
            emit_expr(
                &mut body,
                contract,
                &value_indexes,
                &function_indexes,
                &layout,
                None,
            )?;
            emit_contract_guard(&mut body);
        }
        emit_expr(
            &mut body,
            &function.body,
            &value_indexes,
            &function_indexes,
            &layout,
            None,
        )?;
        body.push(0x21);
        write_u32(&mut body, result_local);
        for contract in &function.ensures {
            emit_expr(
                &mut body,
                contract,
                &value_indexes,
                &function_indexes,
                &layout,
                None,
            )?;
            emit_contract_guard(&mut body);
        }
        body.push(0x20);
        write_u32(&mut body, result_local);
        body.push(0x0b);
        write_u32(&mut code, body.len() as u32);
        code.extend(body);
    }
    for plan in &owned_plans {
        let body = plan.emit_body();
        write_u32(&mut code, body.len() as u32);
        code.extend(body);
    }
    section(&mut module, 10, code);
    Ok(module)
}

pub fn build_web(program: &Program, output: &Path) -> Result<(), Diagnostic> {
    let resolved = hir::resolve(program).map_err(|diagnostics| {
        diagnostics
            .into_iter()
            .find(|item| item.severity.is_error())
            .unwrap_or_else(|| Diagnostic::io("SPX-W100", "HIR resolution failed"))
    })?;
    let owned_plans = owned::plan(&resolved)?;
    std::fs::create_dir_all(output).map_err(|error| {
        Diagnostic::io(
            "SPX-I301",
            format!("cannot create web output {}: {error}", output.display()),
        )
    })?;
    let wasm_bytes = emit_resolved_module(&resolved)?;
    std::fs::write(output.join("app.wasm"), &wasm_bytes).map_err(|error| {
        Diagnostic::io(
            "SPX-I302",
            format!("cannot write WebAssembly module: {error}"),
        )
    })?;
    let runtime_exports = owned_plans
        .iter()
        .map(owned::OwnedPlan::runtime_json)
        .collect::<Vec<_>>()
        .join(",");
    let runtime = browser_runtime()
        .replace(
            "__SEMAPRAX_OWNED_EXPORTS__",
            &format!("Object.freeze({{{runtime_exports}}})"),
        )
        .replace(
            "__SEMAPRAX_WASM_SHA256__",
            &format!("{:x}", Sha256::digest(&wasm_bytes)),
        );
    std::fs::write(output.join("semaprax.js"), runtime).map_err(|error| {
        Diagnostic::io("SPX-I303", format!("cannot write browser runtime: {error}"))
    })?;
    std::fs::write(output.join("index.html"), browser_html()).map_err(|error| {
        Diagnostic::io("SPX-I304", format!("cannot write web entry page: {error}"))
    })?;
    std::fs::write(
        output.join("package.json"),
        "{\"private\":true,\"type\":\"module\"}\n",
    )
    .map_err(|error| {
        Diagnostic::io(
            "SPX-I306",
            format!("cannot write web package metadata: {error}"),
        )
    })?;
    let owned_manifest = owned_plans
        .iter()
        .map(|plan| plan.manifest_json(&resolved.functions[plan.function_index]))
        .collect::<Vec<_>>()
        .join(",");
    let manifest = format!(
        "{{\"schema\":\"semaprax.web.v3\",\"module\":{},\"graph_revision\":{},\"wasm\":\"app.wasm\",\"entry\":\"semaprax_main\",\"capabilities\":{},\"owned_abi\":{{\"schema\":\"semaprax.wasm-owned.v1\",\"functions\":[{}]}}}}\n",
        quote_json(&program.module),
        quote_json(&graph::revision(program)),
        json_strings(&program.permits),
        owned_manifest,
    );
    std::fs::write(output.join("semaprax.manifest.json"), manifest).map_err(|error| {
        Diagnostic::io("SPX-I305", format!("cannot write web manifest: {error}"))
    })?;
    Ok(())
}

fn collect_locals(
    expr: &ResolvedExpr,
    parameter_count: u32,
    layout: &mut LocalLayout,
) -> Result<(), Diagnostic> {
    match &expr.kind {
        ResolvedExprKind::Call { args, .. } => {
            for arg in args {
                collect_locals(arg, parameter_count, layout)?;
            }
        }
        ResolvedExprKind::Unary { value, .. } => {
            collect_locals(value, parameter_count, layout)?;
        }
        ResolvedExprKind::Try { operand, .. } => {
            collect_locals(operand, parameter_count, layout)?;
        }
        ResolvedExprKind::Binary { left, right, .. } => {
            collect_locals(left, parameter_count, layout)?;
            collect_locals(right, parameter_count, layout)?;
        }
        ResolvedExprKind::Block { statements, tail } => {
            for statement in statements {
                match statement {
                    ResolvedStatement::Let { binding, value, .. } => {
                        collect_locals(value, parameter_count, layout)?;
                        let index = parameter_count + layout.declarations.len() as u32;
                        layout.declarations.push(binding.ty.clone());
                        if layout.lets.insert(binding.id.clone(), index).is_some() {
                            return Err(Diagnostic::io(
                                "SPX-W108",
                                format!("duplicate WebAssembly local identity `{}`", binding.id),
                            ));
                        }
                    }
                }
            }
            collect_locals(tail, parameter_count, layout)?;
        }
        ResolvedExprKind::If {
            condition,
            then_branch,
            else_branch,
        } => {
            collect_locals(condition, parameter_count, layout)?;
            collect_locals(then_branch, parameter_count, layout)?;
            collect_locals(else_branch, parameter_count, layout)?;
        }
        ResolvedExprKind::ConstructRecord { fields, .. } => {
            for field in fields {
                collect_locals(&field.value, parameter_count, layout)?;
            }
        }
        ResolvedExprKind::ConstructVariant { fields, .. } => {
            for field in fields {
                collect_locals(&field.value, parameter_count, layout)?;
            }
        }
        ResolvedExprKind::Match { scrutinee, arms } => {
            collect_locals(scrutinee, parameter_count, layout)?;
            for arm in arms {
                collect_locals(&arm.value, parameter_count, layout)?;
            }
        }
        ResolvedExprKind::Project { base, .. } => {
            collect_locals(base, parameter_count, layout)?;
        }
        ResolvedExprKind::UpdateRecord { base, fields, .. } => {
            collect_locals(base, parameter_count, layout)?;
            for field in fields {
                collect_locals(&field.value, parameter_count, layout)?;
            }
        }
        ResolvedExprKind::Int(_) | ResolvedExprKind::Bool(_) | ResolvedExprKind::Place(_) => {}
    }
    Ok(())
}

fn emit_expr(
    output: &mut Vec<u8>,
    expr: &ResolvedExpr,
    value_indexes: &HashMap<ValueId, u32>,
    function_indexes: &HashMap<DeclarationId, u32>,
    layout: &LocalLayout,
    result: Option<(u32, &str)>,
) -> Result<(), Diagnostic> {
    match &expr.kind {
        ResolvedExprKind::Int(value) => {
            output.push(0x42);
            write_i64(output, *value);
        }
        ResolvedExprKind::Bool(value) => {
            output.push(0x41);
            write_i64(output, i64::from(*value));
        }
        ResolvedExprKind::Place(place) => {
            if !place.projections.is_empty() {
                return Err(Diagnostic::io(
                    "SPX-W110",
                    "aggregate place projections are not supported by the Wasm core backend",
                ));
            }
            let index = value_indexes.get(&place.root).copied().or_else(|| {
                result.and_then(|(index, result_id)| {
                    (place.root.as_str() == result_id).then_some(index)
                })
            });
            let index = index.ok_or_else(|| {
                Diagnostic::io(
                    "SPX-W103",
                    format!("unknown value identity `{}`", place.root),
                )
            })?;
            output.push(0x20);
            write_u32(output, index);
        }
        ResolvedExprKind::Call { callee, args } => {
            for arg in args {
                emit_expr(output, arg, value_indexes, function_indexes, layout, result)?;
            }
            output.push(0x10);
            write_u32(
                output,
                *function_indexes.get(callee).ok_or_else(|| {
                    Diagnostic::io("SPX-W104", format!("unknown function identity `{callee}`"))
                })?,
            );
        }
        ResolvedExprKind::Unary { op, value } => match op {
            UnaryOp::Neg => {
                emit_expr(
                    output,
                    value,
                    value_indexes,
                    function_indexes,
                    layout,
                    result,
                )?;
                output.push(0x10);
                write_u32(output, 5);
            }
            UnaryOp::Not => {
                emit_expr(
                    output,
                    value,
                    value_indexes,
                    function_indexes,
                    layout,
                    result,
                )?;
                output.push(0x45);
            }
        },
        ResolvedExprKind::Binary { op, left, right } => {
            emit_expr(
                output,
                left,
                value_indexes,
                function_indexes,
                layout,
                result,
            )?;
            if matches!(op, BinaryOp::And | BinaryOp::Or) {
                emit_short_circuit(
                    output,
                    *op,
                    right,
                    value_indexes,
                    function_indexes,
                    layout,
                    result,
                )?;
                return Ok(());
            }
            emit_expr(
                output,
                right,
                value_indexes,
                function_indexes,
                layout,
                result,
            )?;
            match op {
                BinaryOp::Add => call_import(output, 0),
                BinaryOp::Sub => call_import(output, 1),
                BinaryOp::Mul => call_import(output, 2),
                BinaryOp::Div => call_import(output, 3),
                BinaryOp::Rem => call_import(output, 4),
                BinaryOp::Eq | BinaryOp::Ne => {
                    output.push(match (&left.ty, op) {
                        (ResolvedType::I64, BinaryOp::Eq) => 0x51,
                        (ResolvedType::I64, BinaryOp::Ne) => 0x52,
                        (_, BinaryOp::Eq) => 0x46,
                        (_, BinaryOp::Ne) => 0x47,
                        _ => unreachable!(),
                    });
                }
                BinaryOp::Lt => output.push(0x53),
                BinaryOp::Gt => output.push(0x55),
                BinaryOp::Le => output.push(0x57),
                BinaryOp::Ge => output.push(0x59),
                BinaryOp::And | BinaryOp::Or => unreachable!(),
            }
        }
        ResolvedExprKind::Block { statements, tail } => {
            for statement in statements {
                match statement {
                    ResolvedStatement::Let { binding, value, .. } => {
                        emit_expr(
                            output,
                            value,
                            value_indexes,
                            function_indexes,
                            layout,
                            result,
                        )?;
                        let index = layout.lets.get(&binding.id).ok_or_else(|| {
                            Diagnostic::io(
                                "SPX-W108",
                                format!("missing WebAssembly local layout for `{}`", binding.id),
                            )
                        })?;
                        output.push(0x21);
                        write_u32(output, *index);
                    }
                }
            }
            emit_expr(
                output,
                tail,
                value_indexes,
                function_indexes,
                layout,
                result,
            )?;
        }
        ResolvedExprKind::If {
            condition,
            then_branch,
            else_branch,
        } => {
            emit_expr(
                output,
                condition,
                value_indexes,
                function_indexes,
                layout,
                result,
            )?;
            output.push(0x04);
            output.push(wasm_type(&then_branch.ty)?);
            emit_expr(
                output,
                then_branch,
                value_indexes,
                function_indexes,
                layout,
                result,
            )?;
            output.push(0x05);
            emit_expr(
                output,
                else_branch,
                value_indexes,
                function_indexes,
                layout,
                result,
            )?;
            output.push(0x0b);
        }
        ResolvedExprKind::ConstructRecord { .. }
        | ResolvedExprKind::ConstructVariant { .. }
        | ResolvedExprKind::Match { .. }
        | ResolvedExprKind::Try { .. }
        | ResolvedExprKind::Project { .. }
        | ResolvedExprKind::UpdateRecord { .. } => {
            return Err(Diagnostic::io(
                "SPX-W110",
                "aggregate expressions require WebAssembly aggregate lowering",
            ));
        }
    }
    Ok(())
}

fn emit_short_circuit(
    output: &mut Vec<u8>,
    op: BinaryOp,
    right: &ResolvedExpr,
    value_indexes: &HashMap<ValueId, u32>,
    function_indexes: &HashMap<DeclarationId, u32>,
    layout: &LocalLayout,
    result: Option<(u32, &str)>,
) -> Result<(), Diagnostic> {
    output.push(0x04);
    output.push(I32);
    if op == BinaryOp::And {
        emit_expr(
            output,
            right,
            value_indexes,
            function_indexes,
            layout,
            result,
        )?;
        output.push(0x05);
        output.extend([0x41, 0x00]);
    } else {
        output.extend([0x41, 0x01]);
        output.push(0x05);
        emit_expr(
            output,
            right,
            value_indexes,
            function_indexes,
            layout,
            result,
        )?;
    }
    output.push(0x0b);
    Ok(())
}

fn emit_contract_guard(output: &mut Vec<u8>) {
    output.push(0x45);
    output.extend([0x04, 0x40, 0x10]);
    write_u32(output, 6);
    output.push(0x00);
    output.push(0x0b);
}

fn call_import(output: &mut Vec<u8>, index: u32) {
    output.push(0x10);
    write_u32(output, index);
}

fn wasm_type(ty: &ResolvedType) -> Result<u8, Diagnostic> {
    match ty {
        ResolvedType::I64 => Ok(I64),
        ResolvedType::Bool | ResolvedType::Nominal { .. } => Ok(I32),
        ResolvedType::TypeParameter { .. } => Err(Diagnostic::io(
            "SPX-W109",
            format!(
                "unresolved generic type `{}` cannot be lowered to WebAssembly",
                ty.identity_key()
            ),
        )),
    }
}

fn intern_type(
    signature: Signature,
    types: &mut Vec<Signature>,
    indexes: &mut HashMap<Signature, u32>,
) -> u32 {
    if let Some(index) = indexes.get(&signature) {
        return *index;
    }
    let index = types.len() as u32;
    types.push(signature.clone());
    indexes.insert(signature, index);
    index
}

fn function_import(output: &mut Vec<u8>, module: &str, name: &str, type_index: u32) {
    write_name(output, module);
    write_name(output, name);
    output.push(0x00);
    write_u32(output, type_index);
}

fn section(module: &mut Vec<u8>, id: u8, contents: Vec<u8>) {
    module.push(id);
    write_u32(module, contents.len() as u32);
    module.extend(contents);
}

fn write_name(output: &mut Vec<u8>, value: &str) {
    write_bytes(output, value.as_bytes());
}

fn write_bytes(output: &mut Vec<u8>, value: &[u8]) {
    write_u32(output, value.len() as u32);
    output.extend(value);
}

fn write_u32(output: &mut Vec<u8>, mut value: u32) {
    loop {
        let mut byte = (value & 0x7f) as u8;
        value >>= 7;
        if value != 0 {
            byte |= 0x80;
        }
        output.push(byte);
        if value == 0 {
            break;
        }
    }
}

fn write_i64(output: &mut Vec<u8>, mut value: i64) {
    loop {
        let byte = (value as u8) & 0x7f;
        value >>= 7;
        let done = (value == 0 && byte & 0x40 == 0) || (value == -1 && byte & 0x40 != 0);
        output.push(if done { byte } else { byte | 0x80 });
        if done {
            break;
        }
    }
}

fn json_strings(values: &[String]) -> String {
    format!(
        "[{}]",
        values
            .iter()
            .map(|value| quote_json(value))
            .collect::<Vec<_>>()
            .join(",")
    )
}

fn browser_runtime() -> &'static str {
    r#"const SPX_MIN = -(1n << 63n);
const SPX_MAX = (1n << 63n) - 1n;
const SPX_POISON_I64 = 0x5a5a5a5a5a5a5a5an;
const SPX_POISON_HANDLE = 0x5a5a5a5a;
const SPX_MAX_RUNTIME_TAG = 0x7ff;
const SPX_MAX_SLOT = 0x3ff;
const SPX_MAX_GENERATION = 0x3ff;
const SPX_MAX_DYNAMIC_STATUS = 0x7ffffffe;
const SPX_EXHAUSTED_STATUS = 0x7fffffff;
const SPX_OWNED_EXPORTS = __SEMAPRAX_OWNED_EXPORTS__;
const SPX_WASM_SHA256 = "__SEMAPRAX_WASM_SHA256__";
const SPX_RUNTIME_TAG_ALLOCATOR_KEY = Symbol.for("semaprax.wasm-owned.runtime-tags.v1");
const spxLocalRuntimeTags = new Set();

function runtimeTagAllocator() {
  const installed = globalThis[SPX_RUNTIME_TAG_ALLOCATOR_KEY];
  if (installed !== undefined) {
    if (typeof installed !== "object" || installed === null || typeof installed.take !== "function") {
      throw new Error("SEMAPRAX runtime-tag allocator global is invalid");
    }
    return installed;
  }
  let next = 1;
  const allocator = Object.freeze({
    take() {
      if (next > SPX_MAX_RUNTIME_TAG) {
        throw new Error("SEMAPRAX owned runtime instance identity space exhausted");
      }
      return next++;
    },
  });
  Object.defineProperty(globalThis, SPX_RUNTIME_TAG_ALLOCATOR_KEY, {
    value: allocator,
    configurable: false,
    enumerable: false,
    writable: false,
  });
  return allocator;
}

async function authenticatedWasmBytes(bytes) {
  let source;
  if (bytes instanceof ArrayBuffer) {
    source = new Uint8Array(bytes);
  } else if (ArrayBuffer.isView(bytes)) {
    source = new Uint8Array(bytes.buffer, bytes.byteOffset, bytes.byteLength);
  } else {
    throw new TypeError("SEMAPRAX instantiateBytes requires an ArrayBuffer or typed-array view");
  }
  const ownedCopy = new Uint8Array(source);
  if (globalThis.crypto === undefined || globalThis.crypto.subtle === undefined) {
    throw new Error("SEMAPRAX Web Crypto SHA-256 support is required");
  }
  const digest = new Uint8Array(await globalThis.crypto.subtle.digest("SHA-256", ownedCopy));
  const actual = Array.from(digest, byte => byte.toString(16).padStart(2, "0")).join("");
  if (actual !== SPX_WASM_SHA256) {
    throw new Error("SEMAPRAX WebAssembly artifact authentication failed");
  }
  return ownedCopy;
}

function checked(value, operation) {
  if (value < SPX_MIN || value > SPX_MAX) {
    throw new RangeError(`SEMAPRAX checked arithmetic failure: ${operation}`);
  }
  return value;
}

export const imports = {
  env: {
    spx_add: (a, b) => checked(a + b, "addition overflow"),
    spx_sub: (a, b) => checked(a - b, "subtraction overflow"),
    spx_mul: (a, b) => checked(a * b, "multiplication overflow"),
    spx_div: (a, b) => {
      if (b === 0n || (a === SPX_MIN && b === -1n)) throw new RangeError("SEMAPRAX checked arithmetic failure: invalid division");
      return a / b;
    },
    spx_rem: (a, b) => {
      if (b === 0n || (a === SPX_MIN && b === -1n)) throw new RangeError("SEMAPRAX checked arithmetic failure: invalid remainder");
      return a % b;
    },
    spx_neg: value => checked(-value, "negation overflow"),
    spx_contract_fail: () => { throw new Error("SEMAPRAX contract failure"); },
  },
};

function boundedLimit(value, maximum, name) {
  if (value === undefined) return maximum;
  if (!Number.isSafeInteger(value) || value < 1 || value > maximum) {
    throw new RangeError(`invalid SEMAPRAX ${name} limit`);
  }
  return value;
}

function createOwnedRuntime(options = {}) {
  const maxSlot = boundedLimit(options.maxOwnedSlots, SPX_MAX_SLOT, "owned-slot");
  const maxDynamicStatus = boundedLimit(options.maxStatusTokens, SPX_MAX_DYNAMIC_STATUS, "status-token");
  const runtimeTag = runtimeTagAllocator().take();
  if (!Number.isInteger(runtimeTag) || runtimeTag < 1 || runtimeTag > SPX_MAX_RUNTIME_TAG
      || spxLocalRuntimeTags.has(runtimeTag)) {
    throw new Error("SEMAPRAX runtime-tag allocator returned an invalid or repeated identity");
  }
  spxLocalRuntimeTags.add(runtimeTag);
  const context = ((runtimeTag << 20) | 0x5350) | 0;
  const slots = new Map();
  const generations = new Map();
  const freeSlots = [];
  const statuses = new Map();
  statuses.set(SPX_EXHAUSTED_STATUS, Object.freeze({
    schema: "semaprax.status.v1",
    domain_id: "semaprax.wasm-adapter.v1",
    code: 5,
    class: "adapter",
    retryable: false,
  }));
  const events = [];
  const adoptionTickets = new WeakMap();
  let nextSlot = 1;
  let nextStatus = 1;
  let staging = null;
  let activeResult = null;
  let activeStatus = null;
  let semanticInvocation = null;
  let instance = null;

  const recordStatus = (domain, code, classification) => {
    if (nextStatus > maxDynamicStatus) return SPX_EXHAUSTED_STATUS;
    const token = nextStatus++;
    statuses.set(token, Object.freeze({
      schema: "semaprax.status.v1",
      domain_id: domain,
      code,
      class: classification,
      retryable: false,
    }));
    return token;
  };
  const fillStatus = (status, domain, code, classification) => {
    status.domain_id = domain;
    status.code = code;
    status.class = classification;
    Object.freeze(status);
  };
  const adapterFailure = code => {
    if (staging !== null) {
      fillStatus(staging.status, "semaprax.wasm-adapter.v1", code, "adapter");
      staging.retainStatus = true;
      return staging.statusToken;
    }
    if (activeStatus !== null) {
      fillStatus(activeStatus.status, "semaprax.wasm-adapter.v1", code, "adapter");
      const token = activeStatus.token;
      activeStatus = null;
      return token;
    }
    return recordStatus("semaprax.wasm-adapter.v1", code, "adapter");
  };
  const requireContext = candidate => candidate === context;
  const reserveSlot = (value, state) => {
    let slot;
    let generation;
    while (freeSlots.length > 0) {
      slot = freeSlots.pop();
      generation = (generations.get(slot) ?? 0) + 1;
      if (generation <= SPX_MAX_GENERATION) break;
      slot = undefined;
    }
    if (slot === undefined) {
      if (nextSlot > maxSlot) throw new Error("SEMAPRAX owned handle table exhausted");
      slot = nextSlot++;
      generation = 1;
    }
    generations.set(slot, generation);
    const handle = ((runtimeTag << 20) | (generation << 10) | slot) | 0;
    if (handle === 0 || slots.has(handle)) throw new Error("SEMAPRAX handle allocation invariant");
    const entry = { slot, generation, value, state };
    slots.set(handle, entry);
    return { handle, entry };
  };
  const allocate = value => reserveSlot(value, "owned").handle;
  const release = (handle, expected) => {
    const entry = slots.get(handle);
    if (!entry || entry.state !== expected) throw new Error("SEMAPRAX owned runtime invariant");
    slots.delete(handle);
    freeSlots.push(entry.slot);
    return entry;
  };

  const ownedImports = {
    spx_owned_begin: candidate => {
      if (!requireContext(candidate)) return adapterFailure(1);
      if (staging !== null || activeStatus !== null || activeResult !== null) return adapterFailure(2);
      if (nextStatus > maxDynamicStatus) return SPX_EXHAUSTED_STATUS;
      const statusToken = nextStatus++;
      const status = {
        schema: "semaprax.status.v1",
        domain_id: null,
        code: 0,
        class: null,
        retryable: false,
      };
      statuses.set(statusToken, status);
      staging = { handles: [], result: null, statusToken, status, retainStatus: false };
      return 0;
    },
    spx_owned_stage: (candidate, handle) => {
      if (!requireContext(candidate)) return adapterFailure(1);
      if (staging === null) return adapterFailure(2);
      const entry = slots.get(handle);
      if (!entry || entry.state !== "owned") return adapterFailure(3);
      if (staging.handles.includes(handle)) return adapterFailure(4);
      staging.handles.push(handle);
      return 0;
    },
    spx_owned_abort: candidate => {
      if (!requireContext(candidate)) throw new Error("SEMAPRAX owned abort context invariant");
      if (staging !== null && staging.result !== null) release(staging.result, "reserved");
      if (staging !== null && !staging.retainStatus) statuses.delete(staging.statusToken);
      staging = null;
    },
    spx_owned_reserve_result: candidate => {
      if (!requireContext(candidate)) return adapterFailure(1);
      if (staging === null || staging.result !== null) return adapterFailure(2);
      try {
        staging.result = reserveSlot(undefined, "reserved").handle;
      } catch (error) {
        if (error instanceof Error && error.message === "SEMAPRAX owned handle table exhausted") {
          return adapterFailure(5);
        }
        throw error;
      }
      return 0;
    },
    spx_owned_commit: candidate => {
      if (!requireContext(candidate)) return adapterFailure(1);
      if (staging === null) return adapterFailure(2);
      for (const handle of staging.handles) {
        const entry = slots.get(handle);
        if (!entry || entry.state !== "owned") return adapterFailure(3);
      }
      for (const handle of staging.handles) slots.get(handle).state = "inflight";
      activeResult = staging.result;
      activeStatus = { token: staging.statusToken, status: staging.status };
      events.push(Object.freeze({ kind: "commit", handles: Object.freeze([...staging.handles]) }));
      staging = null;
      return 0;
    },
    spx_owned_drop: (candidate, handle) => {
      if (!requireContext(candidate)) throw new Error("SEMAPRAX owned drop context invariant");
      release(handle, "inflight");
      events.push(Object.freeze({ kind: "drop", handle }));
    },
    spx_owned_cancel_result: candidate => {
      if (!requireContext(candidate)) throw new Error("SEMAPRAX owned cancel context invariant");
      if (activeResult === null) throw new Error("SEMAPRAX result reservation invariant");
      release(activeResult, "reserved");
      activeResult = null;
    },
    spx_owned_publish: (candidate, handle) => {
      if (!requireContext(candidate)) throw new Error("SEMAPRAX owned publish context invariant");
      const entry = release(handle, "inflight");
      if (activeResult === null) throw new Error("SEMAPRAX result publication reservation invariant");
      const published = activeResult;
      const reserved = slots.get(published);
      if (!reserved || reserved.state !== "reserved") throw new Error("SEMAPRAX reserved result invariant");
      reserved.value = entry.value;
      reserved.state = "owned";
      activeResult = null;
      events.push(Object.freeze({ kind: "publish", from: handle, to: published }));
      return published;
    },
    spx_status_record: (candidate, classification, code) => {
      if (!requireContext(candidate)) return adapterFailure(1);
      const target = staging ?? activeStatus;
      if (target === null) throw new Error("SEMAPRAX status reservation invariant");
      let domain;
      let statusClass;
      if (classification === 1 || classification === 2) {
        domain = "semaprax.contract.v1";
        statusClass = "contract";
      } else if (classification === 3) {
        domain = "semaprax.arithmetic.v1";
        statusClass = "arithmetic";
      } else if (classification === 4) {
        domain = "semaprax.wasm-adapter.v1";
        statusClass = "adapter";
      } else {
        throw new Error("SEMAPRAX compiler status classification invariant");
      }
      fillStatus(target.status, domain, code, statusClass);
      events.push(Object.freeze({ kind: "status", domain_id: domain, code, class: statusClass }));
      const token = target.token ?? target.statusToken;
      if (staging !== null) staging.retainStatus = true;
      else activeStatus = null;
      return token;
    },
    spx_owned_success: candidate => {
      if (!requireContext(candidate)) throw new Error("SEMAPRAX owned success context invariant");
      if (activeStatus === null || activeResult !== null) throw new Error("SEMAPRAX success reservation invariant");
      statuses.delete(activeStatus.token);
      activeStatus = null;
    },
    spx_semantic_event: (candidate, functionOrdinal, eventOrdinal) => {
      if (!requireContext(candidate)) throw new Error("SEMAPRAX semantic event context invariant");
      if (semanticInvocation === null) throw new Error("SEMAPRAX semantic event outside invocation");
      const contract = semanticInvocation.contract;
      if (functionOrdinal !== contract.function_ordinal
          || !contract.valid_ordinals.includes(eventOrdinal)
          || eventOrdinal === 0) {
        throw new Error("SEMAPRAX semantic event dictionary invariant");
      }
      semanticInvocation.ordinals.push(eventOrdinal);
    },
  };

  const facade = Object.freeze({
    prepareTrustedAdoption(value) {
      const ticket = Object.freeze(Object.create(null));
      adoptionTickets.set(ticket, { consumed: false, value });
      return ticket;
    },
    adopt(ticket) {
      const adoption = adoptionTickets.get(ticket);
      if (adoption === undefined || adoption.consumed) {
        throw new TypeError("SEMAPRAX adoption ticket is invalid or already consumed");
      }
      const handle = allocate(adoption.value);
      adoption.consumed = true;
      adoption.value = undefined;
      return handle;
    },
    dispose(handle) {
      if (!Number.isInteger(handle) || handle === 0) {
        throw new TypeError("SEMAPRAX owned handle is invalid");
      }
      release(handle, "owned");
      events.push(Object.freeze({ kind: "drop", handle }));
    },
    invoke(exportName, args, resultKind) {
      if (instance === null) throw new Error("SEMAPRAX owned runtime is not bound");
      if (typeof exportName !== "string") {
        throw new TypeError("SEMAPRAX owned export name must be a string");
      }
      if (!Object.hasOwn(SPX_OWNED_EXPORTS, exportName)) {
        throw new TypeError(`unknown SEMAPRAX owned export: ${exportName}`);
      }
      const contract = SPX_OWNED_EXPORTS[exportName];
      if (resultKind !== contract.result) {
        throw new TypeError(`SEMAPRAX owned export ${exportName} requires result kind ${contract.result}`);
      }
      if (!Array.isArray(args) || args.length !== contract.parameters.length) {
        throw new TypeError(`SEMAPRAX owned export ${exportName} argument count mismatch`);
      }
      const canonicalArgs = [];
      for (let index = 0; index < contract.parameters.length; index += 1) {
        const kind = contract.parameters[index];
        const value = args[index];
        const valid = kind === "i64" ? typeof value === "bigint" && value >= SPX_MIN && value <= SPX_MAX
          : kind === "bool" ? Number.isInteger(value) && (value === 0 || value === 1)
          : kind === "resource" ? Number.isInteger(value) && value >= 1 && value <= 0x7fffffff
          : false;
        if (!valid) throw new TypeError(`SEMAPRAX owned export ${exportName} argument ${index} kind mismatch`);
        canonicalArgs.push(value);
      }
      const fn = instance.exports[exportName];
      if (typeof fn !== "function") throw new Error(`missing SEMAPRAX owned export: ${exportName}`);
      const memory = instance.exports.memory;
      if (!(memory instanceof WebAssembly.Memory)) throw new Error("SEMAPRAX owned memory export is absent");
      const view = new DataView(memory.buffer);
      if (resultKind === "i64") view.setBigInt64(0, SPX_POISON_I64, true);
      else view.setInt32(0, SPX_POISON_HANDLE, true);
      const callArgs = [context];
      for (let index = 0; index < canonicalArgs.length; index += 1) {
        callArgs.push(canonicalArgs[index]);
      }
      callArgs.push(0);
      semanticInvocation = { contract, ordinals: [] };
      let statusToken;
      try {
        statusToken = Reflect.apply(fn, undefined, callArgs);
      } catch (error) {
        semanticInvocation = null;
        throw error;
      }
      const semantic = Object.freeze({
        schema: contract.dictionary_schema,
        function: contract.function,
        dictionary_fingerprint: contract.dictionary_fingerprint,
        ordinals: Object.freeze([...semanticInvocation.ordinals]),
      });
      semanticInvocation = null;
      if (statusToken !== 0) {
        const preserved = resultKind === "i64"
          ? view.getBigInt64(0, true) === SPX_POISON_I64
          : view.getInt32(0, true) === SPX_POISON_HANDLE;
        if (!preserved) throw new Error("SEMAPRAX failure published a poisoned result slot");
        const status = statuses.get(statusToken);
        if (!status) throw new Error("SEMAPRAX returned an unknown status token");
        return Object.freeze({ ok: false, published: false, statusToken, status, semantic });
      }
      const value = resultKind === "i64" ? view.getBigInt64(0, true) : view.getInt32(0, true);
      return Object.freeze({ ok: true, published: true, value, semantic });
    },
    resolveStatus(token) {
      return statuses.get(token) ?? null;
    },
    trace() {
      return events.map(event => ({ ...event, handles: event.handles ? [...event.handles] : undefined }));
    },
    liveHandleCount() {
      return slots.size;
    },
  });

  return Object.freeze({
    linkImports: Object.freeze({ env: Object.freeze(ownedImports) }),
    bind(wasmInstance) {
      if (instance !== null) throw new Error("SEMAPRAX owned runtime already bound");
      instance = wasmInstance;
    },
    facade,
  });
}

export async function instantiateBytes(bytes, options = {}) {
  const authenticatedBytes = await authenticatedWasmBytes(bytes);
  if (Object.keys(SPX_OWNED_EXPORTS).length === 0) {
    return Object.freeze(await WebAssembly.instantiate(authenticatedBytes, imports));
  }
  const runtime = createOwnedRuntime(options);
  const linkedImports = { env: { ...imports.env, ...runtime.linkImports.env } };
  const result = await WebAssembly.instantiate(authenticatedBytes, linkedImports);
  runtime.bind(result.instance);
  return Object.freeze({ ...result, owned: runtime.facade });
}

export async function instantiate(url = new URL("./app.wasm", import.meta.url)) {
  const response = await fetch(url);
  return instantiateBytes(await response.arrayBuffer());
}
"#
}

fn browser_html() -> &'static str {
    r##"<!doctype html>
<html lang="en">
  <head>
    <meta charset="utf-8">
    <meta name="viewport" content="width=device-width, initial-scale=1">
    <title>SEMAPRAX</title>
  </head>
  <body>
    <main>
      <h1>SEMAPRAX</h1>
      <output id="result" aria-live="polite">Loading…</output>
    </main>
    <script type="module">
      import { instantiate } from "./semaprax.js";
      const { instance } = await instantiate();
      document.querySelector("#result").value = instance.exports.semaprax_main().toString();
    </script>
  </body>
</html>
"##
}
