use std::collections::HashMap;
use std::path::Path;

use crate::ast::{BinaryOp, Program, UnaryOp};
use crate::diagnostic::{quote_json, Diagnostic};
use crate::graph;
use crate::hir::{
    self, DeclarationId, ResolvedExpr, ResolvedExprKind, ResolvedProgram, ResolvedStatement,
    ResolvedType, ResolvedTypeDeclarationKind, ValueId,
};

const I32: u8 = 0x7f;
const I64: u8 = 0x7e;
const IMPORT_COUNT: u32 = 7;

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
struct Signature {
    params: Vec<u8>,
    results: Vec<u8>,
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
    if program.types.iter().any(|declaration| {
        matches!(
            &declaration.kind,
            ResolvedTypeDeclarationKind::Record { .. }
        )
    }) {
        return Err(Diagnostic::io(
            "SPX-W110",
            "WebAssembly record lowering is gated on linear-memory cleanup and layout support",
        ));
    }
    if program
        .types
        .iter()
        .any(|declaration| matches!(&declaration.kind, ResolvedTypeDeclarationKind::Resource))
    {
        return Err(Diagnostic::io(
            "SPX-W111",
            "WebAssembly resource lowering requires lifecycle declarations and the verified cleanup ABI",
        ));
    }
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

    let function_indexes: HashMap<_, _> = program
        .functions
        .iter()
        .enumerate()
        .map(|(index, function)| (function.id.clone(), IMPORT_COUNT + index as u32))
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
    write_u32(&mut imports, IMPORT_COUNT);
    for name in ["spx_add", "spx_sub", "spx_mul", "spx_div", "spx_rem"] {
        function_import(&mut imports, "env", name, binary_checked);
    }
    function_import(&mut imports, "env", "spx_neg", unary_checked);
    function_import(&mut imports, "env", "spx_contract_fail", contract_fail);
    section(&mut module, 2, imports);

    let mut functions = Vec::new();
    write_u32(&mut functions, function_types.len() as u32);
    for type_index in function_types {
        write_u32(&mut functions, type_index);
    }
    section(&mut module, 3, functions);

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
    write_u32(&mut exports, 1);
    write_name(&mut exports, "semaprax_main");
    exports.push(0x00);
    write_u32(&mut exports, IMPORT_COUNT + main_index as u32);
    section(&mut module, 7, exports);

    let mut code = Vec::new();
    write_u32(&mut code, program.functions.len() as u32);
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
    section(&mut module, 10, code);
    Ok(module)
}

pub fn build_web(program: &Program, output: &Path) -> Result<(), Diagnostic> {
    std::fs::create_dir_all(output).map_err(|error| {
        Diagnostic::io(
            "SPX-I301",
            format!("cannot create web output {}: {error}", output.display()),
        )
    })?;
    std::fs::write(output.join("app.wasm"), emit_module(program)?).map_err(|error| {
        Diagnostic::io(
            "SPX-I302",
            format!("cannot write WebAssembly module: {error}"),
        )
    })?;
    std::fs::write(output.join("semaprax.js"), browser_runtime()).map_err(|error| {
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
    let manifest = format!(
        "{{\"schema\":\"semaprax.web.v2\",\"module\":{},\"graph_revision\":{},\"wasm\":\"app.wasm\",\"entry\":\"semaprax_main\",\"capabilities\":{}}}\n",
        quote_json(&program.module),
        quote_json(&graph::revision(program)),
        json_strings(&program.permits)
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
        ResolvedExprKind::Project { base, .. } => {
            collect_locals(base, parameter_count, layout)?;
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
        ResolvedExprKind::ConstructRecord { .. } | ResolvedExprKind::Project { .. } => {
            return Err(Diagnostic::io(
                "SPX-W110",
                "record expressions require WebAssembly aggregate lowering",
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

export async function instantiate(url = new URL("./app.wasm", import.meta.url)) {
  const response = await fetch(url);
  const bytes = await response.arrayBuffer();
  return WebAssembly.instantiate(bytes, imports);
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
