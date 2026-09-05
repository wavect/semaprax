use std::sync::Arc;

use semaprax::agent_observation::compile_source_agent_observation_schema;
use semaprax::ast::{
    AgentDeclaration, AgentOperationDeclaration, AgentOperationKind, AgentOperationRole,
    AgentTypeRole, AgentTypeRoleDeclaration, ModuleUse, ModuleUseKind, Program, Span,
};
use semaprax::diagnostic::Diagnostic;
use semaprax::project::{
    compile_source_agent_declaration, compile_source_agent_proposal_schema,
    compile_source_program_agents, compile_source_project_agents, render_project_lock,
    with_authenticated_project, AgentDefinitionsQuery, ExactProgramContext, ImageArtifactKind,
    InterfaceArtifactFacts, ProgramRootV2, SemanticWorkspaceService,
    AGENT_INTERACTION_CONTRACT_FACTS_SCHEMA, MAX_IMAGE_ARTIFACT_BUILD_BYTES,
};

use super::{agent_definition_v1::definition, profile};

fn runtime_v1(profile: &str) -> String {
    let profile = profile.strip_suffix('\n').unwrap();
    let members = profile
        .strip_prefix(
            "{\"schema\":\"semaprax.agent-runtime-profile.v1\",\"agent_id\":\"fixture.agent\",",
        )
        .unwrap();
    let (runtime, _) = members.split_once(",\"nonclaims\":").unwrap();
    format!("{{{runtime}}}")
}

fn declaration(agent_id: &str) -> AgentDeclaration {
    let type_role = |role, suffix: &str| AgentTypeRoleDeclaration {
        role,
        stable_id: format!("{agent_id}.type.{suffix}"),
        span: Span::default(),
    };
    let operation = |role, kind, suffix: &str| AgentOperationDeclaration {
        role,
        kind,
        stable_id: format!("{agent_id}.fn.{suffix}"),
        span: Span::default(),
    };
    AgentDeclaration {
        stable_id: agent_id.to_owned(),
        name: "FixtureAgent".to_owned(),
        name_span: Span::default(),
        types: vec![
            type_role(AgentTypeRole::Task, "task"),
            type_role(AgentTypeRole::State, "state"),
            type_role(AgentTypeRole::Observation, "observation"),
            type_role(AgentTypeRole::Proposal, "proposal"),
            type_role(AgentTypeRole::Outcome, "outcome"),
            type_role(AgentTypeRole::Result, "result"),
        ],
        operations: vec![
            operation(
                AgentOperationRole::Initialize,
                AgentOperationKind::Deterministic,
                "initialize",
            ),
            operation(
                AgentOperationRole::Observe,
                AgentOperationKind::Deterministic,
                "observe",
            ),
            operation(
                AgentOperationRole::Propose,
                AgentOperationKind::Model,
                "propose",
            ),
            operation(
                AgentOperationRole::Authorize,
                AgentOperationKind::Deterministic,
                "authorize",
            ),
            operation(
                AgentOperationRole::Execute,
                AgentOperationKind::Effect,
                "execute",
            ),
            operation(
                AgentOperationRole::Reduce,
                AgentOperationKind::Deterministic,
                "reduce",
            ),
        ],
        runtime_v1_json: runtime_v1(&profile()),
        span: Span::default(),
    }
}

fn program(agents: Vec<AgentDeclaration>) -> Program {
    Program {
        path: "agent.spx".to_owned(),
        module: "fixture".to_owned(),
        module_uses: Vec::new(),
        permits: Vec::new(),
        types: Vec::new(),
        interfaces: Vec::new(),
        protocols: Vec::new(),
        implementations: Vec::new(),
        agents,
        functions: Vec::new(),
    }
}

fn assert_code<T>(result: Result<T, Vec<Diagnostic>>, code: &str) {
    let errors = result.err().unwrap_or_else(|| panic!("expected {code}"));
    assert!(errors.iter().any(|error| error.code == code), "{errors:?}");
}

#[test]
fn source_agent_reproduces_frozen_definition_graph_and_profile_bytes() {
    let profile = profile();
    let source = definition(&profile);
    let declaration = declaration("fixture.agent");
    let compiled = compile_source_agent_declaration(&declaration).unwrap();
    assert_eq!(compiled.definition().canonical_source(), source);
    assert_eq!(compiled.runtime_v1_profile(), profile);
    assert_eq!(
        compiled.definition().digest(),
        "sha256:82ab9abbeca5e209c36224d9cab3b7b6a7cdffc3b2fce5db73123fa7425965a0"
    );
    assert_eq!(
        compiled.graph().digest(),
        "sha256:0dc7ce1d50d43077042577cf6ac3dcfb5d2a744fb3acd2ca6cea12a6e296ff61"
    );

    let mut renamed = declaration.clone();
    renamed.name = "RenamedDisplayOnly".to_owned();
    let renamed = compile_source_agent_declaration(&renamed).unwrap();
    assert_eq!(renamed.definition().canonical_source(), source);
    assert_eq!(
        renamed.graph().canonical_json(),
        compiled.graph().canonical_json()
    );

    let one = program(vec![declaration]);
    let project = compile_source_program_agents(&one).unwrap();
    assert_eq!(project.definitions().len(), 1);
    assert_eq!(project.agents().len(), 1);
    assert_eq!(project.agents()[0].stable_id.as_str(), "fixture.agent");
    assert_eq!(project.agents()[0].types.len(), 6);
    assert_eq!(project.agents()[0].operations.len(), 6);
    assert_eq!(
        project.definitions()[0].definition().canonical_source(),
        source
    );

    let mut resolved_input = semaprax::parse(
        "module fixture;\n@id(\"app.main\") fn main() -> i64 { 0 }\n",
        "agent-hir.spx",
    )
    .unwrap();
    resolved_input.agents = one.agents.clone();
    let resolved = semaprax::hir::resolve(&resolved_input).unwrap();
    assert_eq!(resolved.agents.len(), 1);
    assert_eq!(resolved.agents[0], project.agents()[0]);
    semaprax::hir::validate(&resolved).unwrap();
}

#[test]
fn source_agent_role_and_project_identity_errors_fail_closed() {
    let mut missing = declaration("fixture.agent");
    missing.types.pop();
    assert_code(compile_source_agent_declaration(&missing), "SPX-G559");

    let mut duplicate_role = declaration("fixture.agent");
    duplicate_role.types[1].role = AgentTypeRole::Task;
    assert_code(
        compile_source_agent_declaration(&duplicate_role),
        "SPX-G559",
    );

    let mut wrong_kind = declaration("fixture.agent");
    wrong_kind.operations[2].kind = AgentOperationKind::Deterministic;
    assert_code(compile_source_agent_declaration(&wrong_kind), "SPX-G559");

    let mut duplicate_id = declaration("fixture.agent");
    duplicate_id.operations[0].stable_id = duplicate_id.types[0].stable_id.clone();
    assert_code(compile_source_agent_declaration(&duplicate_id), "SPX-G559");

    let first = program(vec![declaration("z.agent")]);
    let second = program(vec![declaration("a.agent")]);
    let ordered = compile_source_project_agents(&[&first, &second]).unwrap();
    assert_eq!(ordered.definitions()[0].definition().agent_id(), "a.agent");
    assert_eq!(ordered.definitions()[1].definition().agent_id(), "z.agent");

    let collision = program(vec![declaration("a.agent")]);
    assert_code(
        compile_source_project_agents(&[&second, &collision]),
        "SPX-G559",
    );

    let shared_first = declaration("shared.a");
    let mut shared_second = declaration("shared.b");
    shared_second.types = shared_first.types.clone();
    shared_second.operations = shared_first.operations.clone();
    let shared = program(vec![shared_first, shared_second]);
    assert_eq!(
        compile_source_program_agents(&shared)
            .unwrap()
            .definitions()
            .len(),
        2,
        "multiple Agents may bind the same ordinary declarations"
    );

    let mut declaration_collision = program(vec![declaration("external.agent")]);
    declaration_collision.module_uses.push(ModuleUse {
        kind: ModuleUseKind::Type,
        persistent_id: "external.agent".to_owned(),
        target_module: "dependency".to_owned(),
        alias: "Task".to_owned(),
        span: Span::default(),
    });
    assert_code(
        compile_source_program_agents(&declaration_collision),
        "SPX-G559",
    );
}

#[test]
fn source_agent_populates_project_program_root_query_and_service_without_changing_outputs() {
    let root = std::env::temp_dir().join(format!(
        "semaprax-source-agent-project-{}",
        std::process::id()
    ));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(root.join("src")).unwrap();
    let runtime = serde_json::to_string(&runtime_v1(&profile())).unwrap();
    let agent_source = format!(
        r#"module fixture;

@id("fixture.agent.type.observation")
record Observation {{
    @id("fixture.agent.type.observation.ready")
    ready: bool,
}}

@id("fixture.agent.type.proposal")
record Proposal {{
    @id("fixture.agent.type.proposal.action")
    action: bool,
}}

@id("fixture.payload")
record Payload {{
    @id("fixture.payload.bytes")
    bytes: Bytes,
}}

@id("fixture.agent")
agent FixtureAgent {{
    types {{
        @id("fixture.agent.type.task")
        type task;
        @id("fixture.agent.type.state")
        type state;
        @id("fixture.agent.type.observation")
        type observation;
        @id("fixture.agent.type.proposal")
        type proposal;
        @id("fixture.agent.type.outcome")
        type outcome;
        @id("fixture.agent.type.result")
        type result;
    }}
    operations {{
        @id("fixture.agent.fn.initialize")
        fn initialize;
        @id("fixture.agent.fn.observe")
        fn observe;
        @id("fixture.agent.fn.propose")
        model fn propose;
        @id("fixture.agent.fn.authorize")
        fn authorize;
        @id("fixture.agent.fn.execute")
        effect fn execute;
        @id("fixture.agent.fn.reduce")
        fn reduce;
    }}
    runtime_v1 {{
        canonical_json {runtime};
    }}
}}

@id("fixture.agent.module-marker")
fn main() -> i64 {{ 0 }}

@id("fixture.build")
fn build(input: borrow Slice<u8>) -> Payload {{
    Payload {{ bytes: bytes_copy(input) }}
}}
"#,
    );
    let agent_source =
        semaprax::format::canonical(&semaprax::parse(&agent_source, "src/app.spx").unwrap());
    std::fs::write(root.join("src/app.spx"), &agent_source).unwrap();
    std::fs::write(
        root.join("src/tests.spx"),
        "module fixture.tests;\n\n@id(\"fixture.tests.main\")\nfn main() -> i64\n{\n    0\n}\n",
    )
    .unwrap();
    std::fs::write(
        root.join("semaprax.toml"),
        "schema = \"semaprax.project.v11\"\nname = \"fixture\"\nversion = \"1.0.0\"\nprofile = \"nested-owned-record-api.v1\"\nentry = \"fixture\"\nsources = [\"src/app.spx\", \"src/tests.spx\"]\nweb_exports = [\"fixture.build\"]\ntests = [\"fixture.tests\"]\n",
    )
    .unwrap();
    let manifest = root.join("semaprax.toml").canonicalize().unwrap();

    with_authenticated_project(&manifest, |snapshot| {
        let revision = snapshot.retain_revision();
        assert_eq!(revision.source_agents().len(), 1);
        assert_eq!(revision.agent_definitions().len(), 1);
        let retained = &revision.agent_definitions()[0];
        let direct = compile_source_agent_declaration(&declaration("fixture.agent"))?;
        assert_eq!(
            retained.definition().canonical_source(),
            direct.definition().canonical_source()
        );
        assert_eq!(
            retained.graph().canonical_json(),
            direct.graph().canonical_json()
        );
        assert_eq!(retained.runtime_v1_profile(), direct.runtime_v1_profile());
        let contracts = revision.agent_interaction_contract_facts().unwrap();
        assert_eq!(contracts.facts().len(), 1);
        assert!(contracts
            .to_json()
            .contains(AGENT_INTERACTION_CONTRACT_FACTS_SCHEMA));
        assert!(contracts.to_json().contains("\"authority\":false"));
        let contract = &contracts.facts()[0];
        let proposal =
            compile_source_agent_proposal_schema(&agent_source, "src/app.spx", "fixture.agent")?;
        let observation =
            compile_source_agent_observation_schema(&agent_source, "src/app.spx", "fixture.agent")?;
        assert_eq!(
            contract.proposal_schema(),
            proposal.schema().canonical_json()
        );
        assert_eq!(
            contract.proposal_schema_digest(),
            proposal.schema().digest()
        );
        assert_eq!(
            contract.observation_schema(),
            observation.schema().canonical_json()
        );
        assert_eq!(
            contract.observation_schema_digest(),
            observation.schema().digest()
        );

        let workspace = revision.canonical_workspace_revision()?;
        let root = workspace.program_root()?;
        let agents =
            serde_json::from_str::<serde_json::Value>(workspace.agent_definitions().to_json())
                .unwrap();
        assert_eq!(
            agents["payload"]["integration"],
            "source_owned_spx_agent_declarations"
        );
        assert_eq!(
            agents["payload"]["definitions"][0]["agent_id"],
            "fixture.agent"
        );
        assert_eq!(
            agents["payload"]["definitions"][0]["interaction_contract"]["proposal"]["schema"],
            proposal.schema().canonical_json()
        );
        assert_eq!(
            agents["payload"]["definitions"][0]["interaction_contract"]["observation"]["schema"],
            observation.schema().canonical_json()
        );
        assert_eq!(
            root.segment("agent_definitions").unwrap().node_digest(),
            workspace.agent_definitions().digest()
        );

        let service = SemanticWorkspaceService::open(Arc::clone(&revision))?;
        let query = AgentDefinitionsQuery::new(workspace.workspace_revision())?;
        let result = service.query_agent_definitions(&query)?;
        assert_eq!(result.workspace_revision(), workspace.workspace_revision());
        assert_eq!(result.program_root(), &root);
        assert_eq!(result.agent_definitions(), workspace.agent_definitions());
        assert_eq!(result.agent_interaction_contract_facts(), Some(contracts));
        assert_eq!(
            service
                .active_generation()
                .agent_interaction_contract_facts(),
            Some(contracts)
        );
        assert_eq!(
            service.active_generation().source_agents(),
            revision.source_agents()
        );
        assert_eq!(
            service.active_generation().compiled_agent_definitions()[0]
                .definition()
                .canonical_source(),
            direct.definition().canonical_source()
        );

        let lock = render_project_lock(snapshot)?;
        let association =
            root.associate_dependency_lock(snapshot, root.program_root_digest(), &lock)?;
        let facts = InterfaceArtifactFacts::derive(
            revision.clone(),
            revision.project_revision(),
            &[ImageArtifactKind::Npm],
            MAX_IMAGE_ARTIFACT_BUILD_BYTES,
        )?;
        let v2 = ProgramRootV2::derive(&workspace, &root, &facts, &association)?;
        let context = ExactProgramContext::assemble(
            revision,
            snapshot.project_revision(),
            workspace.clone(),
            workspace.workspace_revision(),
            facts,
            association,
        )?;
        assert_eq!(context.program_root_v2(), &v2);
        assert_eq!(
            context
                .program_root_v2()
                .segment("agent_definitions")
                .unwrap()
                .node_digest(),
            workspace.agent_definitions().digest()
        );
        Ok(())
    })
    .unwrap();
    std::fs::remove_dir_all(root).unwrap();
}
