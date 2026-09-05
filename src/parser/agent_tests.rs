use std::path::Path;

use crate::ast::{AgentOperationKind, AgentOperationRole, AgentTypeRole};

const CANONICAL: &str = r#"module agent.example;

@id("example.agent")
agent Example {
    types {
        @id("example.agent.type.task")
        type task;
        @id("example.agent.type.state")
        type state;
        @id("example.agent.type.observation")
        type observation;
        @id("example.agent.type.proposal")
        type proposal;
        @id("example.agent.type.outcome")
        type outcome;
        @id("example.agent.type.result")
        type result;
    }
    operations {
        @id("example.agent.fn.initialize")
        fn initialize;
        @id("example.agent.fn.observe")
        fn observe;
        @id("example.agent.fn.propose")
        model fn propose;
        @id("example.agent.fn.authorize")
        fn authorize;
        @id("example.agent.fn.execute")
        effect fn execute;
        @id("example.agent.fn.reduce")
        fn reduce;
    }
    runtime_v1 {
        canonical_json "{\"limits\":{},\"models\":[],\"policy\":{},\"tools\":[]}";
    }
}

@id("app.main")
fn main() -> i64
{
    0
}
"#;

#[test]
fn agent_declaration_has_one_exact_canonical_projection() {
    let program = crate::parse(CANONICAL, Path::new("agent.spx")).unwrap();
    assert_eq!(program.agents.len(), 1);
    let agent = &program.agents[0];
    assert_eq!(agent.stable_id, "example.agent");
    assert_eq!(agent.name, "Example");
    assert_eq!(
        agent.types.iter().map(|row| row.role).collect::<Vec<_>>(),
        vec![
            AgentTypeRole::Task,
            AgentTypeRole::State,
            AgentTypeRole::Observation,
            AgentTypeRole::Proposal,
            AgentTypeRole::Outcome,
            AgentTypeRole::Result,
        ]
    );
    assert_eq!(agent.operations[2].role, AgentOperationRole::Propose);
    assert_eq!(agent.operations[2].kind, AgentOperationKind::Model);
    assert_eq!(agent.operations[4].role, AgentOperationRole::Execute);
    assert_eq!(agent.operations[4].kind, AgentOperationKind::Effect);
    assert_eq!(crate::format::canonical(&program), CANONICAL);
    let repeated =
        crate::parse(&crate::format::canonical(&program), Path::new("agent.spx")).unwrap();
    assert_eq!(repeated, program);
}

#[test]
fn agent_roles_are_closed_ordered_and_explicitly_identified() {
    let missing_id = CANONICAL.replacen("@id(\"example.agent.type.task\")\n        ", "", 1);
    let error = crate::parse(&missing_id, Path::new("agent.spx")).unwrap_err();
    assert_eq!(error.code, "SPX-P124");

    let reordered = CANONICAL
        .replace("type task;", "type wrong;")
        .replace("model fn propose;", "fn propose;");
    let error = crate::parse(&reordered, Path::new("agent.spx")).unwrap_err();
    assert!(matches!(error.code, "SPX-P104" | "SPX-P124"));
}
