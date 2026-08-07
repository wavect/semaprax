use std::collections::{BTreeSet, HashMap, VecDeque};
use std::fmt::Write;

use crate::ast::{Expr, ExprKind, Program, Statement};
use crate::diagnostic::quote_json;
use crate::format;

pub fn revision(program: &Program) -> String {
    let source = format::canonical(program);
    let mut hash = 0xcbf29ce484222325_u64;
    for byte in source.as_bytes() {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x100000001b3);
    }
    format!("fnv1a64:{hash:016x}")
}

pub fn to_json(program: &Program) -> String {
    let selected: BTreeSet<_> = program
        .functions
        .iter()
        .map(|function| function.name.clone())
        .collect();
    graph_json(program, &selected)
}

pub fn context_json(program: &Program, symbol: &str, depth: usize) -> Option<String> {
    let by_name: HashMap<_, _> = program
        .functions
        .iter()
        .map(|function| (function.name.as_str(), function))
        .collect();
    let root = program
        .functions
        .iter()
        .find(|function| function.name == symbol || function.stable_id == symbol)?;
    let mut selected = BTreeSet::from([root.name.clone()]);
    let mut queue = VecDeque::from([(root.name.clone(), 0_usize)]);
    while let Some((name, current_depth)) = queue.pop_front() {
        if current_depth >= depth {
            continue;
        }
        if let Some(function) = by_name.get(name.as_str()) {
            visit_function_calls(function, &mut |callee, _| {
                if by_name.contains_key(callee) && selected.insert(callee.to_owned()) {
                    queue.push_back((callee.to_owned(), current_depth + 1));
                }
            });
        }
    }
    Some(graph_json(program, &selected))
}

fn graph_json(program: &Program, selected: &BTreeSet<String>) -> String {
    let mut output = String::new();
    write!(
        output,
        "{{\"schema\":\"semaprax.graph.v2\",\"revision\":{},\"identity\":{{\"declarations\":\"persistent\",\"expressions\":\"revision-scoped\"}},\"module\":{},\"permits\":{},\"nodes\":[",
        quote_json(&revision(program)),
        quote_json(&program.module),
        string_array(&program.permits)
    )
    .unwrap();
    let mut first = true;
    for resource in &program.resources {
        if !first {
            output.push(',');
        }
        first = false;
        write!(
            output,
            "{{\"id\":{},\"kind\":\"resource\",\"name\":{},\"memory\":\"unique\"}}",
            quote_json(&resource.stable_id),
            quote_json(&resource.name)
        )
        .unwrap();
    }
    for function in &program.functions {
        if !selected.contains(function.name.as_str()) {
            continue;
        }
        if !first {
            output.push(',');
        }
        first = false;
        let mut calls = Vec::new();
        visit_function_calls(function, &mut |name, _| calls.push(name.to_owned()));
        let params = function
            .params
            .iter()
            .map(|param| {
                format!(
                    "{{\"name\":{},\"type\":{},\"ownership\":{}}}",
                    quote_json(&param.name),
                    quote_json(&param.ty.to_string()),
                    quote_json(param.mode.text())
                )
            })
            .collect::<Vec<_>>()
            .join(",");
        let requires = function
            .requires
            .iter()
            .map(|value| format::expr(value, 0))
            .collect::<Vec<_>>();
        let ensures = function
            .ensures
            .iter()
            .map(|value| format::expr(value, 0))
            .collect::<Vec<_>>();
        let requires_graph = function
            .requires
            .iter()
            .enumerate()
            .map(|(index, value)| {
                expr_json(value, &format!("{}#requires{index}", function.stable_id))
            })
            .collect::<Vec<_>>()
            .join(",");
        let ensures_graph = function
            .ensures
            .iter()
            .enumerate()
            .map(|(index, value)| {
                expr_json(value, &format!("{}#ensures{index}", function.stable_id))
            })
            .collect::<Vec<_>>()
            .join(",");
        let body = expr_json(&function.body, &format!("{}#body", function.stable_id));
        write!(
            output,
            "{{\"id\":{},\"kind\":\"function\",\"name\":{},\"params\":[{}],\"returns\":{},\"effects\":{},\"requires\":{},\"requires_graph\":[{}],\"ensures\":{},\"ensures_graph\":[{}],\"calls\":{},\"body\":{}}}",
            quote_json(&function.stable_id),
            quote_json(&function.name),
            params,
            quote_json(&function.return_type.to_string()),
            string_array(&function.effects),
            string_array(&requires),
            requires_graph,
            string_array(&ensures),
            ensures_graph,
            string_array(&calls),
            body
        )
        .unwrap();
    }
    output.push_str("]}");
    output
}

fn visit_function_calls(
    function: &crate::ast::Function,
    visit: &mut impl FnMut(&str, crate::ast::Span),
) {
    for contract in &function.requires {
        contract.visit_calls(visit);
    }
    function.body.visit_calls(visit);
    for contract in &function.ensures {
        contract.visit_calls(visit);
    }
}

fn expr_json(expr: &Expr, id: &str) -> String {
    match &expr.kind {
        ExprKind::Int(value) => format!(
            "{{\"id\":{},\"kind\":\"int\",\"value\":{value}}}",
            quote_json(id)
        ),
        ExprKind::Bool(value) => format!(
            "{{\"id\":{},\"kind\":\"bool\",\"value\":{value}}}",
            quote_json(id)
        ),
        ExprKind::Var(name) => format!(
            "{{\"id\":{},\"kind\":\"value_ref\",\"name\":{}}}",
            quote_json(id),
            quote_json(name)
        ),
        ExprKind::Call { name, args } => format!(
            "{{\"id\":{},\"kind\":\"call\",\"callee\":{},\"args\":[{}]}}",
            quote_json(id),
            quote_json(name),
            args.iter()
                .enumerate()
                .map(|(index, arg)| expr_json(arg, &format!("{id}.arg{index}")))
                .collect::<Vec<_>>()
                .join(",")
        ),
        ExprKind::Unary { op, value } => format!(
            "{{\"id\":{},\"kind\":\"unary\",\"op\":{},\"value\":{}}}",
            quote_json(id),
            quote_json(match op {
                crate::ast::UnaryOp::Neg => "-",
                crate::ast::UnaryOp::Not => "!",
            }),
            expr_json(value, &format!("{id}.value"))
        ),
        ExprKind::Binary { op, left, right } => format!(
            "{{\"id\":{},\"kind\":\"binary\",\"op\":{},\"left\":{},\"right\":{}}}",
            quote_json(id),
            quote_json(op.text()),
            expr_json(left, &format!("{id}.left")),
            expr_json(right, &format!("{id}.right"))
        ),
        ExprKind::Block { statements, tail } => format!(
            "{{\"id\":{},\"kind\":\"block\",\"statements\":[{}],\"tail\":{}}}",
            quote_json(id),
            statements
                .iter()
                .enumerate()
                .map(|(index, statement)| statement_json(statement, &format!("{id}.s{index}")))
                .collect::<Vec<_>>()
                .join(","),
            expr_json(tail, &format!("{id}.tail"))
        ),
        ExprKind::If {
            condition,
            then_branch,
            else_branch,
        } => format!(
            "{{\"id\":{},\"kind\":\"if\",\"condition\":{},\"then\":{},\"else\":{}}}",
            quote_json(id),
            expr_json(condition, &format!("{id}.condition")),
            expr_json(then_branch, &format!("{id}.then")),
            expr_json(else_branch, &format!("{id}.else"))
        ),
    }
}

fn statement_json(statement: &Statement, id: &str) -> String {
    match statement {
        Statement::Let { name, value, .. } => format!(
            "{{\"id\":{},\"kind\":\"let\",\"name\":{},\"value\":{}}}",
            quote_json(id),
            quote_json(name),
            expr_json(value, &format!("{id}.value"))
        ),
    }
}

fn string_array(values: &[String]) -> String {
    format!(
        "[{}]",
        values
            .iter()
            .map(|value| quote_json(value))
            .collect::<Vec<_>>()
            .join(",")
    )
}
