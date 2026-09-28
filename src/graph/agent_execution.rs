//! Graph v50: checked Agent execution associations over the exact dynamic base.
use crate::ast::{AgentOperationKind, AgentOperationRole, Program};
use crate::bounded_output::BudgetedJoin as _;
use crate::diagnostic::{quote_json, Diagnostic};

pub(crate) const GRAPH_SCHEMA: &str = "semaprax.graph.v50";

pub(super) fn has_agent_execution(program: &Program) -> bool {
    program
        .agents
        .iter()
        .any(|agent| agent.has_execution_metadata())
}

/// Canonical tagged rows reused unchanged by authenticated project projections.
pub(crate) fn facts(program: &Program) -> Result<Vec<String>, Diagnostic> {
    let mut agents = program
        .agents
        .iter()
        .filter(|agent| agent.has_execution_metadata())
        .collect::<Vec<_>>();
    agents.sort_by(|a, b| a.stable_id.as_bytes().cmp(b.stable_id.as_bytes()));
    agents
        .into_iter()
        .map(|agent| {
            agent
                .validate_execution_metadata(program)
                .map_err(|message| Diagnostic::io("SPX-G559", message))?;
            let expected = [
                AgentOperationRole::Initialize,
                AgentOperationRole::Observe,
                AgentOperationRole::Authorize,
                AgentOperationRole::Reduce,
            ];
            if !agent
                .operations
                .iter()
                .filter(|operation| operation.kind == AgentOperationKind::Deterministic)
                .map(|operation| operation.role)
                .eq(expected)
            {
                return Err(Diagnostic::io(
                    "SPX-G559",
                    "checked Agent deterministic roles are not canonical",
                ));
            }
            let mut operation_rows = agent
                .operations
                .iter()
                .filter(|operation| operation.kind == AgentOperationKind::Deterministic)
                .map(|operation| {
                    format!(
                        "{{\"role\":{},\"operation_id\":{},\"function_id\":{},\"origin\":{}}}",
                        quote_json(operation.role.source_name()),
                        quote_json(&operation.stable_id),
                        quote_json(&operation.stable_id),
                        quote_json(if operation.embedded_function_index.is_some() {
                            "embedded"
                        } else {
                            "reference"
                        })
                    )
                });
            // Validation fixes this carrier at four deterministic roles; retain
            // it on the stack rather than allocating an uncharged Vec.
            let mut next_row = || {
                operation_rows.next().ok_or_else(|| {
                    Diagnostic::io("SPX-G559", "checked Agent deterministic role is missing")
                })
            };
            let operation_rows = [next_row()?, next_row()?, next_row()?, next_row()?];
            let operations = operation_rows.budgeted_join(",");
            let wait = if let Some(binding) = &agent.model_wait {
                let model = agent
                    .operations
                    .iter()
                    .find(|operation| {
                        operation.role == AgentOperationRole::Propose
                            && operation.kind == AgentOperationKind::Model
                    })
                    .ok_or_else(|| {
                        Diagnostic::io("SPX-G559", "checked Agent model role is missing")
                    })?;
                format!(
                    ",\"model_wait\":{{\"model_operation_id\":{},\"helper_id\":{}}}",
                    quote_json(&model.stable_id),
                    quote_json(&binding.helper_id)
                )
            } else {
                String::new()
            };
            Ok(format!(
                "{{\"agent\":{},\"operations\":[{}]{}}}",
                quote_json(&agent.stable_id),
                operations,
                wait
            ))
        })
        .collect()
}

pub(super) fn attach(program: &Program, mut graph: String) -> Result<String, Diagnostic> {
    if !has_agent_execution(program) {
        return Ok(graph);
    }
    let rows = facts(program)?.budgeted_join(",");
    let base = serde_json::from_str::<serde_json::Value>(&graph)
        .map_err(|_| Diagnostic::io("SPX-G411", "invalid checked graph payload"))?["schema"]
        .as_str()
        .ok_or_else(|| Diagnostic::io("SPX-G411", "missing checked schema"))?
        .to_owned();
    let prefix = format!("{{\"schema\":{}", quote_json(&base));
    if !graph.starts_with(&prefix) || !graph.ends_with('}') {
        return Err(Diagnostic::io(
            "SPX-G411",
            "checked graph header is not canonical",
        ));
    }
    graph.replace_range(..prefix.len(), &format!("{{\"schema\":\"{GRAPH_SCHEMA}\""));
    graph.pop();
    graph.push_str(&format!(
        ",\"agent_execution\":{{\"base_schema\":{},\"agents\":[{}]}}}}",
        quote_json(&base),
        rows
    ));
    Ok(graph)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn combined() -> crate::ast::Program {
        let agent = crate::parser::agent_embedded_tests::source();
        let agent = agent.split_once("\n").unwrap().1;
        let agent = agent.split("@id(\"main\")").next().unwrap();
        let source = format!("{}\n{agent}\n@id(\"loan.consume\") fn consume(value:own Bytes)->i64 {{7}}\n@id(\"loan.observe\") fn borrowed()->i64 {{ let owned=bytes_zeroed(2usize); let view=bytes_as_slice(owned); let size=byte_len(view); consume(owned) + if size==2usize {{1}} else {{0}} }}", include_str!("../session_protocol/tests/fixtures/follows.spx"));
        crate::check(&source, "combined.spx").unwrap()
    }

    #[test]
    fn facts_refuse_missing_extra_or_reordered_deterministic_roles() {
        let program = combined();
        for mutation in 0..3 {
            let mut forged = program.clone();
            let agent = &mut forged.agents[0];
            let index = agent
                .operations
                .iter()
                .position(|operation| operation.role == AgentOperationRole::Observe)
                .unwrap();
            match mutation {
                0 => {
                    agent.operations.remove(index);
                }
                1 => {
                    let extra = agent.operations[index].clone();
                    agent.operations.push(extra);
                }
                _ => {
                    agent.operations.swap(index, 0);
                }
            }
            assert_eq!(facts(&forged).unwrap_err().code, "SPX-G559");
        }
    }

    #[test]
    fn v50_preserves_the_exact_dynamic_protocol_cleanup_and_loan_base() {
        let program = combined();
        let mut base = program.clone();
        for agent in &mut base.agents {
            agent.model_wait = None;
            for operation in &mut agent.operations {
                operation.embedded_function_index = None;
            }
        }
        let resolved = crate::hir::resolve(&base).unwrap();
        let owner = resolved
            .functions
            .iter()
            .find(|f| f.id.as_str() == "loan.observe")
            .unwrap();
        assert!(!owner.cleanup_plan.slots.is_empty());
        assert!(!owner.loan_plan.loans.is_empty());
        let base_json = crate::graph::to_json(&base).unwrap();
        let base_value: serde_json::Value = serde_json::from_str(&base_json).unwrap();
        assert_eq!(base_value["schema"], "semaprax.graph.v49");
        assert_eq!(
            base_value["session_protocol_follows"]["base_schema"],
            "semaprax.graph.v48"
        );
        assert!(base_value["session_protocols"].is_object());
        assert!(base_json.contains("core.bytes.drop"));
        assert!(base_json.contains("semaprax.loan-plan.v1"));
        let combined = attach(&program, base_json).unwrap();
        let mut value: serde_json::Value = serde_json::from_str(&combined).unwrap();
        assert_eq!(value["schema"], GRAPH_SCHEMA);
        let facts = value
            .as_object_mut()
            .unwrap()
            .remove("agent_execution")
            .unwrap();
        assert_eq!(facts["base_schema"], "semaprax.graph.v49");
        assert_eq!(facts["agents"].as_array().unwrap().len(), 1);
        assert_eq!(
            facts["agents"][0]["operations"].as_array().unwrap().len(),
            4
        );
        value["schema"] = base_value["schema"].clone();
        assert_eq!(
            value, base_value,
            "the complete selected base must survive byte-preserving wrapping"
        );
    }

    #[test]
    fn graph_replay_rejects_dropped_or_reminted_association_edges_and_downgrade() {
        let program = combined();
        let canonical = crate::format::canonical(&program);
        let repeated = crate::check(&canonical, "combined.spx").unwrap();
        assert_eq!(crate::format::canonical(&repeated), canonical);
        let graph = crate::graph::to_json(&repeated).unwrap();
        crate::graph::verify_json(&repeated, &graph).unwrap();
        let original: serde_json::Value = serde_json::from_str(&graph).unwrap();
        assert_eq!(original["schema"], GRAPH_SCHEMA);
        for selector in 0..3 {
            let mut forged = original.clone();
            match selector {
                0 => {
                    forged.as_object_mut().unwrap().remove("agent_execution");
                }
                1 => {
                    forged["agent_execution"]["agents"][0]["operations"][0]["function_id"] =
                        "unrelated".into()
                }
                _ => forged["schema"] = "semaprax.graph.v49".into(),
            }
            assert_eq!(
                crate::graph::verify_json(&repeated, &forged.to_string()).unwrap_err()[0].code,
                "SPX-G411"
            );
        }
    }
}
