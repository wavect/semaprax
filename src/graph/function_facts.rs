//! Per-function agent-context fact rendering, split out of `graph.rs` to
//! keep that file at its recorded module-size budget (see
//! `tests/module-size-budget.tsv`; AGENTS.md, "Repository navigation").
//!
//! A submodule of `graph`, so `use super::*` brings in everything `graph.rs`
//! itself imports plus its private helpers (`agent_reference_index_json`,
//! `agent_contract_expr_json`, `result_ownership`, `ownership_text`, ...) --
//! descendant modules see an ancestor's private items in Rust.

use super::*;

/// One standalone legacy function fact. Queries render facts through
/// `agent_function_json_for_schema` with their already selected schema and
/// query-local call set instead.
#[cfg(test)]
pub(super) fn agent_function_json(
    program: &ResolvedProgram,
    function: &ResolvedFunction,
    filters: &BTreeSet<AgentContextFilter>,
) -> Result<String, Diagnostic> {
    let schema = nested_owned::legacy_graph_schema(program)?;
    let calls = super::agent_query::AgentCallables::new(program).function_calls(function);
    agent_function_json_for_schema(program, function, filters, schema, &calls)
}

/// Render one function fact for the schema and legacy `calls` set the
/// caller already selected for this query (REF-09).
pub(super) fn agent_function_json_for_schema(
    program: &ResolvedProgram,
    function: &ResolvedFunction,
    filters: &BTreeSet<AgentContextFilter>,
    schema: &str,
    calls: &BTreeSet<DeclarationId>,
) -> Result<String, Diagnostic> {
    let mut propagations = Vec::new();
    collect_result_propagations(&function.body, &mut propagations);
    let mut output = format!(
        "{{\"id\":{},\"kind\":\"function\",\"name\":{},\"calls\":{},\"reference_index\":{}",
        quote_json(function.id.as_str()),
        quote_json(&function.name),
        string_array(
            &calls
                .iter()
                .map(|id| id.as_str().to_owned())
                .collect::<Vec<_>>(),
        ),
        agent_reference_index_json(program, function)?
    );
    if schema == "semaprax.graph.v14" || graph_schema_includes_modern_composite_facts(schema) {
        write!(
            output,
            ",\"call_instances\":[{}],\"body\":{}",
            agent_call_instances_json(function),
            agent_contract_expr_json(&function.body)?,
        )
        .expect("writing to a string cannot fail");
    }
    if !propagations.is_empty() {
        write!(
            output,
            ",\"result_propagations\":[{}]",
            propagations
                .into_iter()
                .map(result_propagation_json)
                .collect::<Vec<_>>()
                .budgeted_join(",")
        )
        .expect("writing to a string cannot fail");
    }
    if filters.contains(&AgentContextFilter::Contracts) {
        write!(
            output,
            ",\"contracts\":{{\"requires\":[{}],\"ensures\":[{}]}}",
            function
                .requires
                .iter()
                .map(agent_contract_expr_json)
                .collect::<Result<Vec<_>, _>>()?
                .budgeted_join(","),
            function
                .ensures
                .iter()
                .map(agent_contract_expr_json)
                .collect::<Result<Vec<_>, _>>()?
                .budgeted_join(",")
        )
        .expect("writing to a string cannot fail");
    }
    if filters.contains(&AgentContextFilter::Ownership) {
        let result = result_ownership(program, &function.return_type)?;
        write!(
            output,
            ",\"ownership\":{{\"parameters\":[{}],\"result\":{}}}",
            function
                .params
                .iter()
                .map(|parameter| format!(
                    "{{\"id\":{},\"mode\":{}}}",
                    quote_json(parameter.id.as_str()),
                    quote_json(ownership_text(parameter.ownership))
                ))
                .collect::<Vec<_>>()
                .budgeted_join(","),
            quote_json(ownership_text(result))
        )
        .expect("writing to a string cannot fail");
    }
    if filters.contains(&AgentContextFilter::Effects) {
        write!(output, ",\"effects\":{}", string_array(&function.effects))
            .expect("writing to a string cannot fail");
    }
    if filters.contains(&AgentContextFilter::Types) {
        let mut selected_types = BTreeSet::new();
        collect_function_type_declarations(function, &mut selected_types);
        close_type_declarations(program, &mut selected_types)?;
        let selected_functions = BTreeSet::from([function.id.clone()]);
        write!(
            output,
            ",\"types\":{{\"parameters\":[{}],\"result\":{},\"facts\":[{}],\"declarations\":[{}]}}",
            function
                .params
                .iter()
                .map(|parameter| format!(
                    "{{\"id\":{},\"type_id\":{}}}",
                    quote_json(parameter.id.as_str()),
                    quote_json(&parameter.ty.identity_key())
                ))
                .collect::<Vec<_>>()
                .budgeted_join(","),
            quote_json(&function.return_type.identity_key()),
            type_facts_array(program, &selected_functions, &selected_types)?,
            agent_type_declarations_json(program, &selected_types)?
        )
        .expect("writing to a string cannot fail");
    }
    if graph_schema_includes_loans(schema) {
        write!(
            output,
            ",\"loans\":{}",
            crate::graph_loan::loan_plan_json(&function.loan_plan)
        )
        .expect("writing to a string cannot fail");
    }
    output.push('}');
    Ok(output)
}
