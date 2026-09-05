use std::path::Path;
use std::sync::Arc;

use semaprax::ast::{
    AgentDeclaration, AgentOperationDeclaration, AgentOperationKind, AgentOperationRole,
    AgentTypeRole, AgentTypeRoleDeclaration, ModuleUse, ModuleUseKind, Program, Span,
};
use semaprax::diagnostic::Diagnostic;
use semaprax::project::{
    compile_source_agent_declaration, compile_source_program_agents, compile_source_project_agents,
    render_project_lock, with_authenticated_project, AgentDefinitionsQuery, ExactProgramContext,
    ImageArtifactKind, InterfaceArtifactFacts, ProgramRootV2, SemanticWorkspaceService,
    MAX_IMAGE_ARTIFACT_BUILD_BYTES,
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

    let mut declaration_collision = program(vec![declaration("external.agent")]);
    declaration_collision.module_uses.push(ModuleUse {
        kind: ModuleUseKind::Type,
        persistent_id: "external.agent.type.task".to_owned(),
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
    let sample = Path::new(env!("CARGO_MANIFEST_DIR")).join("examples/calculator-project");
    for file in ["src/app.spx", "src/core.spx", "src/tests.spx"] {
        std::fs::copy(sample.join(file), root.join(file)).unwrap();
    }
    let mut agent_program = program(vec![declaration("fixture.agent")]);
    agent_program.path = "src/agent.spx".to_owned();
    let mut agent_source = semaprax::format::canonical(&agent_program);
    agent_source.push_str(
        "\n@id(\"fixture.agent.module-marker\")\nfn module_marker() -> i64\n{\n    0\n}\n",
    );
    std::fs::write(root.join("src/agent.spx"), agent_source).unwrap();
    let manifest = std::fs::read_to_string(sample.join("semaprax.toml"))
        .unwrap()
        .replace(
            "sources = [\"src/app.spx\", \"src/core.spx\", \"src/tests.spx\"]",
            "sources = [\"src/agent.spx\", \"src/app.spx\", \"src/core.spx\", \"src/tests.spx\"]",
        );
    std::fs::write(root.join("semaprax.toml"), manifest).unwrap();
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
            root.segment("agent_definitions").unwrap().node_digest(),
            workspace.agent_definitions().digest()
        );

        let service = SemanticWorkspaceService::open(Arc::clone(&revision))?;
        let query = AgentDefinitionsQuery::new(workspace.workspace_revision())?;
        let result = service.query_agent_definitions(&query)?;
        assert_eq!(result.workspace_revision(), workspace.workspace_revision());
        assert_eq!(result.program_root(), &root);
        assert_eq!(result.agent_definitions(), workspace.agent_definitions());
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
            &[ImageArtifactKind::Web],
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
