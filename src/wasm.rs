use std::collections::HashMap;
use std::path::Path;

use crate::ast::{BinaryOp, Expr, ExprKind, Program, Statement, Type, UnaryOp};
use crate::diagnostic::{quote_json, Diagnostic};
use crate::graph;

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
    declarations: Vec<Type>,
    lets: HashMap<usize, (u32, Type)>,
}

pub fn emit_module(program: &Program) -> Result<Vec<u8>, Diagnostic> {
    if let Some(error) = crate::verify::verify(program)
        .into_iter()
        .find(|item| item.severity.is_error())
    {
        return Err(error);
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
                .collect(),
            results: vec![wasm_type(&function.return_type)],
        };
        function_types.push(intern_type(signature, &mut types, &mut type_indexes));
    }

    let function_indexes: HashMap<_, _> = program
        .functions
        .iter()
        .enumerate()
        .map(|(index, function)| (function.name.as_str(), IMPORT_COUNT + index as u32))
        .collect();
    let function_returns: HashMap<_, _> = program
        .functions
        .iter()
        .map(|item| (item.name.to_owned(), item.return_type.clone()))
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
        .position(|function| function.name == "main")
        .ok_or_else(|| Diagnostic::io("SPX-W101", "web target requires a main function"))?;
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
        let variables: HashMap<_, _> = function
            .params
            .iter()
            .enumerate()
            .map(|(index, param)| (param.name.clone(), (index as u32, param.ty.clone())))
            .collect();
        let mut layout = LocalLayout {
            declarations: vec![function.return_type.clone()],
            lets: HashMap::new(),
        };
        for contract in &function.requires {
            collect_locals(
                contract,
                &variables,
                &function_returns,
                function.params.len() as u32,
                &mut layout,
            )?;
        }
        collect_locals(
            &function.body,
            &variables,
            &function_returns,
            function.params.len() as u32,
            &mut layout,
        )?;
        let mut ensure_variables = variables.clone();
        ensure_variables.insert(
            "result".to_owned(),
            (result_local, function.return_type.clone()),
        );
        for contract in &function.ensures {
            collect_locals(
                contract,
                &ensure_variables,
                &function_returns,
                function.params.len() as u32,
                &mut layout,
            )?;
        }
        write_u32(&mut body, layout.declarations.len() as u32);
        for ty in &layout.declarations {
            write_u32(&mut body, 1);
            body.push(wasm_type(ty));
        }
        let mut variables = variables;
        for contract in &function.requires {
            emit_expr(
                &mut body,
                contract,
                &mut variables,
                &function_indexes,
                &function_returns,
                &layout,
                None,
            )?;
            emit_contract_guard(&mut body);
        }
        emit_expr(
            &mut body,
            &function.body,
            &mut variables,
            &function_indexes,
            &function_returns,
            &layout,
            None,
        )?;
        body.push(0x21);
        write_u32(&mut body, result_local);
        for contract in &function.ensures {
            emit_expr(
                &mut body,
                contract,
                &mut variables,
                &function_indexes,
                &function_returns,
                &layout,
                Some((result_local, function.return_type.clone())),
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
        "{{\"schema\":\"semaprax.web.v1\",\"module\":{},\"graph_revision\":{},\"wasm\":\"app.wasm\",\"entry\":\"semaprax_main\",\"capabilities\":{}}}\n",
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
    expr: &Expr,
    variables: &HashMap<String, (u32, Type)>,
    function_returns: &HashMap<String, Type>,
    parameter_count: u32,
    layout: &mut LocalLayout,
) -> Result<(), Diagnostic> {
    match &expr.kind {
        ExprKind::Call { args, .. } => {
            for arg in args {
                collect_locals(arg, variables, function_returns, parameter_count, layout)?;
            }
        }
        ExprKind::Unary { value, .. } => {
            collect_locals(value, variables, function_returns, parameter_count, layout)?;
        }
        ExprKind::Binary { left, right, .. } => {
            collect_locals(left, variables, function_returns, parameter_count, layout)?;
            collect_locals(right, variables, function_returns, parameter_count, layout)?;
        }
        ExprKind::Block { statements, tail } => {
            let mut scope = variables.clone();
            for statement in statements {
                match statement {
                    Statement::Let {
                        name, value, span, ..
                    } => {
                        collect_locals(value, &scope, function_returns, parameter_count, layout)?;
                        let ty = expr_type(value, &scope, function_returns, None)?;
                        let index = parameter_count + layout.declarations.len() as u32;
                        layout.declarations.push(ty.clone());
                        layout.lets.insert(span.start, (index, ty.clone()));
                        scope.insert(name.clone(), (index, ty));
                    }
                }
            }
            collect_locals(tail, &scope, function_returns, parameter_count, layout)?;
        }
        ExprKind::If {
            condition,
            then_branch,
            else_branch,
        } => {
            collect_locals(
                condition,
                variables,
                function_returns,
                parameter_count,
                layout,
            )?;
            collect_locals(
                then_branch,
                &variables.clone(),
                function_returns,
                parameter_count,
                layout,
            )?;
            collect_locals(
                else_branch,
                &variables.clone(),
                function_returns,
                parameter_count,
                layout,
            )?;
        }
        ExprKind::Int(_) | ExprKind::Bool(_) | ExprKind::Var(_) => {}
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn emit_expr(
    output: &mut Vec<u8>,
    expr: &Expr,
    variables: &mut HashMap<String, (u32, Type)>,
    function_indexes: &HashMap<&str, u32>,
    function_returns: &HashMap<String, Type>,
    layout: &LocalLayout,
    result: Option<(u32, Type)>,
) -> Result<(), Diagnostic> {
    match &expr.kind {
        ExprKind::Int(value) => {
            output.push(0x42);
            write_i64(output, *value);
        }
        ExprKind::Bool(value) => {
            output.push(0x41);
            write_i64(output, i64::from(*value));
        }
        ExprKind::Var(name) if name == "result" => {
            let (index, _) = result
                .as_ref()
                .ok_or_else(|| Diagnostic::io("SPX-W102", "result used outside postcondition"))?;
            output.push(0x20);
            write_u32(output, *index);
        }
        ExprKind::Var(name) => {
            let (index, _) = variables
                .get(name.as_str())
                .ok_or_else(|| Diagnostic::io("SPX-W103", format!("unknown local `{name}`")))?;
            output.push(0x20);
            write_u32(output, *index);
        }
        ExprKind::Call { name, args } => {
            for arg in args {
                emit_expr(
                    output,
                    arg,
                    variables,
                    function_indexes,
                    function_returns,
                    layout,
                    result.clone(),
                )?;
            }
            output.push(0x10);
            write_u32(
                output,
                *function_indexes.get(name.as_str()).ok_or_else(|| {
                    Diagnostic::io("SPX-W104", format!("unknown function `{name}`"))
                })?,
            );
        }
        ExprKind::Unary { op, value } => match op {
            UnaryOp::Neg => {
                emit_expr(
                    output,
                    value,
                    variables,
                    function_indexes,
                    function_returns,
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
                    variables,
                    function_indexes,
                    function_returns,
                    layout,
                    result,
                )?;
                output.push(0x45);
            }
        },
        ExprKind::Binary { op, left, right } => {
            emit_expr(
                output,
                left,
                variables,
                function_indexes,
                function_returns,
                layout,
                result.clone(),
            )?;
            if matches!(op, BinaryOp::And | BinaryOp::Or) {
                emit_short_circuit(
                    output,
                    *op,
                    right,
                    variables,
                    function_indexes,
                    function_returns,
                    layout,
                    result,
                )?;
                return Ok(());
            }
            emit_expr(
                output,
                right,
                variables,
                function_indexes,
                function_returns,
                layout,
                result.clone(),
            )?;
            match op {
                BinaryOp::Add => call_import(output, 0),
                BinaryOp::Sub => call_import(output, 1),
                BinaryOp::Mul => call_import(output, 2),
                BinaryOp::Div => call_import(output, 3),
                BinaryOp::Rem => call_import(output, 4),
                BinaryOp::Eq | BinaryOp::Ne => {
                    let ty = expr_type(left, variables, function_returns, result.as_ref())?;
                    output.push(match (ty, op) {
                        (Type::I64, BinaryOp::Eq) => 0x51,
                        (Type::I64, BinaryOp::Ne) => 0x52,
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
        ExprKind::Block { statements, tail } => {
            let saved = variables.clone();
            for statement in statements {
                match statement {
                    Statement::Let {
                        name, value, span, ..
                    } => {
                        emit_expr(
                            output,
                            value,
                            variables,
                            function_indexes,
                            function_returns,
                            layout,
                            result.clone(),
                        )?;
                        let (index, ty) = layout.lets.get(&span.start).ok_or_else(|| {
                            Diagnostic::io(
                                "SPX-W108",
                                format!("missing WebAssembly local layout for `{name}`"),
                            )
                        })?;
                        output.push(0x21);
                        write_u32(output, *index);
                        variables.insert(name.clone(), (*index, ty.clone()));
                    }
                }
            }
            emit_expr(
                output,
                tail,
                variables,
                function_indexes,
                function_returns,
                layout,
                result,
            )?;
            *variables = saved;
        }
        ExprKind::If {
            condition,
            then_branch,
            else_branch,
        } => {
            emit_expr(
                output,
                condition,
                variables,
                function_indexes,
                function_returns,
                layout,
                result.clone(),
            )?;
            let ty = expr_type(then_branch, variables, function_returns, result.as_ref())?;
            output.push(0x04);
            output.push(wasm_type(&ty));
            let mut then_variables = variables.clone();
            emit_expr(
                output,
                then_branch,
                &mut then_variables,
                function_indexes,
                function_returns,
                layout,
                result.clone(),
            )?;
            output.push(0x05);
            let mut else_variables = variables.clone();
            emit_expr(
                output,
                else_branch,
                &mut else_variables,
                function_indexes,
                function_returns,
                layout,
                result,
            )?;
            output.push(0x0b);
        }
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn emit_short_circuit(
    output: &mut Vec<u8>,
    op: BinaryOp,
    right: &Expr,
    variables: &mut HashMap<String, (u32, Type)>,
    function_indexes: &HashMap<&str, u32>,
    function_returns: &HashMap<String, Type>,
    layout: &LocalLayout,
    result: Option<(u32, Type)>,
) -> Result<(), Diagnostic> {
    output.push(0x04);
    output.push(I32);
    if op == BinaryOp::And {
        emit_expr(
            output,
            right,
            variables,
            function_indexes,
            function_returns,
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
            variables,
            function_indexes,
            function_returns,
            layout,
            result,
        )?;
    }
    output.push(0x0b);
    Ok(())
}

fn expr_type(
    expr: &Expr,
    variables: &HashMap<String, (u32, Type)>,
    functions: &HashMap<String, Type>,
    result: Option<&(u32, Type)>,
) -> Result<Type, Diagnostic> {
    Ok(match &expr.kind {
        ExprKind::Int(_) => Type::I64,
        ExprKind::Bool(_) => Type::Bool,
        ExprKind::Var(name) if name == "result" => result
            .map(|(_, ty)| ty.clone())
            .or_else(|| variables.get(name).map(|(_, ty)| ty.clone()))
            .ok_or_else(|| Diagnostic::io("SPX-W105", "missing result type"))?,
        ExprKind::Var(name) => variables
            .get(name.as_str())
            .map(|(_, ty)| ty.clone())
            .ok_or_else(|| Diagnostic::io("SPX-W106", format!("unknown local `{name}`")))?,
        ExprKind::Call { name, .. } => functions
            .get(name.as_str())
            .cloned()
            .ok_or_else(|| Diagnostic::io("SPX-W107", format!("unknown function `{name}`")))?,
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
                        let ty = expr_type(value, &scope, functions, result)?;
                        scope.insert(name.clone(), (u32::MAX, ty));
                    }
                }
            }
            expr_type(tail, &scope, functions, result)?
        }
        ExprKind::If { then_branch, .. } => expr_type(then_branch, variables, functions, result)?,
    })
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

fn wasm_type(ty: &Type) -> u8 {
    match ty {
        Type::I64 => I64,
        Type::Bool | Type::Resource(_) => I32,
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
