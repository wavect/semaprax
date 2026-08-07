use std::collections::{BTreeSet, HashMap, VecDeque};
use std::fmt::Write;

use crate::ast::Program;
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
            function.body.visit_calls(&mut |callee, _| {
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
        "{{\"schema\":\"semaprax.graph.v1\",\"revision\":{},\"module\":{},\"permits\":{},\"nodes\":[",
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
        function
            .body
            .visit_calls(&mut |name, _| calls.push(name.to_owned()));
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
        write!(
            output,
            "{{\"id\":{},\"kind\":\"function\",\"name\":{},\"params\":[{}],\"returns\":{},\"effects\":{},\"requires\":{},\"ensures\":{},\"calls\":{}}}",
            quote_json(&function.stable_id),
            quote_json(&function.name),
            params,
            quote_json(&function.return_type.to_string()),
            string_array(&function.effects),
            string_array(&requires),
            string_array(&ensures),
            string_array(&calls)
        )
        .unwrap();
    }
    output.push_str("]}");
    output
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
