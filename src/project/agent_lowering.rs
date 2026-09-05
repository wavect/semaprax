//! Semantic lowering for language-native `.spx` Agent declarations.
//!
//! The existing AgentDefinition v1 compiler remains the compatibility oracle:
//! lowering renders its exact canonical input, recompiles it, and returns the
//! unchanged AgentDefinition, AgentGraph, and Runtime Profile products.

use std::collections::BTreeSet;
use std::fmt::Write;

use crate::agent_definition::{compile_agent_definition, CompiledAgentDefinition};
use crate::ast::{
    AgentDeclaration, AgentOperationKind, AgentOperationRole, AgentTypeRole, Program,
    TypeDeclarationKind,
};
use crate::diagnostic::{quote_json, Diagnostic};

use super::{ProjectRevision, SemanticWorkspaceRevision};

pub const SOURCE_AGENT_LOWERING_SCHEMA: &str = "semaprax.source-agent-lowering.v1";
pub const MAX_SOURCE_AGENTS_PER_PROJECT: usize = 64;

const TYPE_ROLES: [AgentTypeRole; 6] = [
    AgentTypeRole::Task,
    AgentTypeRole::State,
    AgentTypeRole::Observation,
    AgentTypeRole::Proposal,
    AgentTypeRole::Outcome,
    AgentTypeRole::Result,
];
const OPERATION_ROLES: [(AgentOperationRole, AgentOperationKind); 6] = [
    (
        AgentOperationRole::Initialize,
        AgentOperationKind::Deterministic,
    ),
    (
        AgentOperationRole::Observe,
        AgentOperationKind::Deterministic,
    ),
    (AgentOperationRole::Propose, AgentOperationKind::Model),
    (
        AgentOperationRole::Authorize,
        AgentOperationKind::Deterministic,
    ),
    (AgentOperationRole::Execute, AgentOperationKind::Effect),
    (
        AgentOperationRole::Reduce,
        AgentOperationKind::Deterministic,
    ),
];

type Result<T> = std::result::Result<T, Vec<Diagnostic>>;

/// Complete compiler products for a Project's admitted source Agents, ordered
/// by stable Agent identity for canonical workspace association.
pub struct CompiledSourceAgents {
    agents: Vec<ResolvedSourceAgent>,
    definitions: Vec<CompiledAgentDefinition>,
    semantic_ids: BTreeSet<String>,
}

pub type ResolvedSourceAgent = crate::hir::ResolvedAgentDeclaration;

impl CompiledSourceAgents {
    pub fn definitions(&self) -> &[CompiledAgentDefinition] {
        &self.definitions
    }

    pub fn is_empty(&self) -> bool {
        self.definitions.is_empty()
    }

    pub fn agents(&self) -> &[ResolvedSourceAgent] {
        &self.agents
    }

    pub fn into_definitions(self) -> Vec<CompiledAgentDefinition> {
        self.definitions
    }

    pub fn into_parts(self) -> (Vec<ResolvedSourceAgent>, Vec<CompiledAgentDefinition>) {
        (self.agents, self.definitions)
    }

    /// Populate the existing AgentDefinitions node from compiler products.
    /// The Project owner must supply the exact revision whose admitted ASTs
    /// produced this value; the revision precondition fails closed.
    pub fn derive_workspace(
        &self,
        revision: &ProjectRevision,
        expected_project_revision: &str,
    ) -> Result<SemanticWorkspaceRevision> {
        if expected_project_revision != revision.project_revision() {
            return Err(invariant(
                "source Agent workspace association selected a stale Project revision",
            ));
        }
        validate_project_identity_separation(revision, &self.semantic_ids)?;
        if self.definitions.is_empty() {
            return SemanticWorkspaceRevision::derive(revision);
        }
        let definitions = self.definitions.iter().collect::<Vec<_>>();
        SemanticWorkspaceRevision::derive_with_agent_definitions(
            revision,
            expected_project_revision,
            &definitions,
        )
    }
}

/// Lower one frontend Agent declaration through the frozen JSON compiler.
pub fn compile_source_agent_declaration(
    declaration: &AgentDeclaration,
) -> Result<CompiledAgentDefinition> {
    validate_declaration_shape(declaration)?;
    let source = render_definition_v1(declaration)?;
    let compiled = compile_agent_definition(&source)?;
    if compiled.definition().agent_id() != declaration.stable_id
        || compiled.definition().canonical_source() != source
    {
        return Err(invariant(
            "source Agent did not reproduce the exact AgentDefinition v1 identity and bytes",
        ));
    }
    Ok(compiled)
}

/// Lower the Agent declarations in one verified frontend Program.
pub fn compile_source_program_agents(program: &Program) -> Result<CompiledSourceAgents> {
    compile_source_project_agents(&[program])
}

/// Lower an exact Project-owned Program set, reject cross-module stable-ID
/// collisions, and place complete compiler products in stable Agent-ID order.
pub fn compile_source_project_agents(programs: &[&Program]) -> Result<CompiledSourceAgents> {
    let count = programs
        .iter()
        .try_fold(0usize, |count, program| {
            count.checked_add(program.agents.len())
        })
        .ok_or_else(|| invariant("source Agent declaration count overflowed"))?;
    if count > MAX_SOURCE_AGENTS_PER_PROJECT {
        return Err(invariant(
            "source Project exceeds the sixty-four Agent declaration bound",
        ));
    }

    let mut occupied_ids = BTreeSet::new();
    for program in programs {
        collect_existing_program_ids(program, &mut occupied_ids);
    }
    let mut agent_ids = BTreeSet::new();
    let mut declarations = Vec::with_capacity(count);
    for program in programs {
        for declaration in &program.agents {
            for stable_id in declaration_ids(declaration) {
                if !agent_ids.insert(stable_id.to_owned())
                    || !occupied_ids.insert(stable_id.to_owned())
                {
                    return Err(invariant(
                        "source Project Agent identities must be unique and disjoint from existing declarations",
                    ));
                }
            }
            declarations.push(declaration);
        }
    }
    declarations.sort_by(|left, right| left.stable_id.as_bytes().cmp(right.stable_id.as_bytes()));
    let mut agents = Vec::with_capacity(declarations.len());
    let mut definitions = Vec::with_capacity(declarations.len());
    for declaration in declarations {
        let compiled = compile_source_agent_declaration(declaration)?;
        agents.push(resolve_source_agent(declaration));
        definitions.push(compiled);
    }
    Ok(CompiledSourceAgents {
        agents,
        definitions,
        semantic_ids: agent_ids,
    })
}

fn resolve_source_agent(declaration: &AgentDeclaration) -> ResolvedSourceAgent {
    crate::hir::ResolvedAgentDeclaration {
        stable_id: crate::hir::DeclarationId::new(declaration.stable_id.clone()),
        name: declaration.name.clone(),
        types: declaration
            .types
            .iter()
            .map(|role| crate::hir::ResolvedAgentTypeRole {
                role: match role.role {
                    AgentTypeRole::Task => crate::hir::ResolvedAgentTypeRoleKind::Task,
                    AgentTypeRole::State => crate::hir::ResolvedAgentTypeRoleKind::State,
                    AgentTypeRole::Observation => {
                        crate::hir::ResolvedAgentTypeRoleKind::Observation
                    }
                    AgentTypeRole::Proposal => crate::hir::ResolvedAgentTypeRoleKind::Proposal,
                    AgentTypeRole::Outcome => crate::hir::ResolvedAgentTypeRoleKind::Outcome,
                    AgentTypeRole::Result => crate::hir::ResolvedAgentTypeRoleKind::Result,
                },
                stable_id: crate::hir::DeclarationId::new(role.stable_id.clone()),
            })
            .collect(),
        operations: declaration
            .operations
            .iter()
            .map(|operation| crate::hir::ResolvedAgentOperationRole {
                role: match operation.role {
                    AgentOperationRole::Initialize => {
                        crate::hir::ResolvedAgentOperationRoleKind::Initialize
                    }
                    AgentOperationRole::Observe => {
                        crate::hir::ResolvedAgentOperationRoleKind::Observe
                    }
                    AgentOperationRole::Propose => {
                        crate::hir::ResolvedAgentOperationRoleKind::Propose
                    }
                    AgentOperationRole::Authorize => {
                        crate::hir::ResolvedAgentOperationRoleKind::Authorize
                    }
                    AgentOperationRole::Execute => {
                        crate::hir::ResolvedAgentOperationRoleKind::Execute
                    }
                    AgentOperationRole::Reduce => {
                        crate::hir::ResolvedAgentOperationRoleKind::Reduce
                    }
                },
                kind: match operation.kind {
                    AgentOperationKind::Deterministic => {
                        crate::hir::ResolvedAgentOperationKind::Deterministic
                    }
                    AgentOperationKind::Model => crate::hir::ResolvedAgentOperationKind::Model,
                    AgentOperationKind::Effect => crate::hir::ResolvedAgentOperationKind::Effect,
                },
                stable_id: crate::hir::DeclarationId::new(operation.stable_id.clone()),
            })
            .collect(),
        runtime_v1_json: declaration.runtime_v1_json.clone(),
    }
}

fn collect_existing_program_ids(program: &Program, ids: &mut BTreeSet<String>) {
    for module_use in &program.module_uses {
        ids.insert(module_use.persistent_id.clone());
    }
    for declaration in &program.types {
        ids.insert(declaration.stable_id.clone());
        match &declaration.kind {
            TypeDeclarationKind::Resource { lifecycles } => {
                for lifecycle in lifecycles {
                    if let Some(id) = &lifecycle.stable_id {
                        ids.insert(id.clone());
                    }
                }
            }
            TypeDeclarationKind::Record { fields } => {
                ids.extend(fields.iter().map(|field| field.stable_id.clone()));
            }
            TypeDeclarationKind::Variant { cases } => {
                for case in cases {
                    ids.insert(case.stable_id.clone());
                    ids.extend(case.fields.iter().map(|field| field.stable_id.clone()));
                }
            }
            TypeDeclarationKind::Class { fields, methods } => {
                ids.extend(fields.iter().map(|field| field.stable_id.clone()));
                ids.extend(methods.iter().map(|method| method.stable_id.clone()));
            }
        }
    }
    for interface in &program.interfaces {
        ids.insert(interface.stable_id.clone());
        ids.extend(
            interface
                .imports
                .iter()
                .map(|import| import.stable_id.clone()),
        );
    }
    for protocol in &program.protocols {
        ids.insert(protocol.stable_id.clone());
        ids.extend(
            protocol
                .methods
                .iter()
                .map(|method| method.stable_id.clone()),
        );
    }
    ids.extend(
        program
            .implementations
            .iter()
            .map(|implementation| implementation.stable_id.clone()),
    );
    ids.extend(
        program
            .functions
            .iter()
            .map(|function| function.stable_id.clone()),
    );
}

fn validate_project_identity_separation(
    revision: &ProjectRevision,
    agent_ids: &BTreeSet<String>,
) -> Result<()> {
    fn visit(value: &serde_json::Value, retained_ids: &mut BTreeSet<String>) {
        match value {
            serde_json::Value::Object(object) => {
                for (key, value) in object {
                    if matches!(key.as_str(), "id" | "stable_id" | "declaration_id") {
                        if let Some(id) = value.as_str() {
                            retained_ids.insert(id.to_owned());
                        }
                    }
                    visit(value, retained_ids);
                }
            }
            serde_json::Value::Array(values) => {
                for value in values {
                    visit(value, retained_ids);
                }
            }
            _ => {}
        }
    }
    let graph: serde_json::Value = serde_json::from_str(revision.semantic_graph())
        .map_err(|_| invariant("retained Project graph is not valid JSON"))?;
    let mut retained_ids = BTreeSet::new();
    visit(&graph, &mut retained_ids);
    if agent_ids.iter().any(|id| retained_ids.contains(id)) {
        return Err(invariant(
            "source Agent identity collides with an existing Project declaration identity",
        ));
    }
    Ok(())
}

fn validate_declaration_shape(declaration: &AgentDeclaration) -> Result<()> {
    if declaration.types.len() != TYPE_ROLES.len() {
        return Err(invariant(
            "source Agent requires exactly task, state, observation, proposal, outcome, and result type roles",
        ));
    }
    for (actual, expected) in declaration.types.iter().zip(TYPE_ROLES) {
        if actual.role != expected {
            return Err(invariant(
                "source Agent type roles are missing, duplicated, or out of canonical order",
            ));
        }
    }
    if declaration.operations.len() != OPERATION_ROLES.len() {
        return Err(invariant(
            "source Agent requires exactly initialize, observe, propose, authorize, execute, and reduce operation roles",
        ));
    }
    for (actual, (expected_role, expected_kind)) in
        declaration.operations.iter().zip(OPERATION_ROLES)
    {
        if actual.role != expected_role || actual.kind != expected_kind {
            return Err(invariant(
                "source Agent operation roles or deterministic/model/effect kinds are invalid",
            ));
        }
    }
    let mut identities = BTreeSet::new();
    if !identities.insert(declaration.stable_id.as_str())
        || declaration
            .types
            .iter()
            .map(|role| role.stable_id.as_str())
            .chain(
                declaration
                    .operations
                    .iter()
                    .map(|role| role.stable_id.as_str()),
            )
            .any(|stable_id| !identities.insert(stable_id))
    {
        return Err(invariant(
            "source Agent type, operation, and Agent identities must be unique",
        ));
    }
    let runtime: serde_json::Value = serde_json::from_str(&declaration.runtime_v1_json)
        .map_err(|_| malformed("source Agent runtime_v1 is not valid JSON"))?;
    if !runtime.is_object()
        || declaration.runtime_v1_json.contains('\n')
        || declaration.runtime_v1_json.contains('\r')
    {
        return Err(malformed(
            "source Agent runtime_v1 must be one canonical JSON object",
        ));
    }
    Ok(())
}

fn declaration_ids(declaration: &AgentDeclaration) -> impl Iterator<Item = &str> {
    std::iter::once(declaration.stable_id.as_str())
        .chain(declaration.types.iter().map(|role| role.stable_id.as_str()))
        .chain(
            declaration
                .operations
                .iter()
                .map(|role| role.stable_id.as_str()),
        )
}

fn render_definition_v1(declaration: &AgentDeclaration) -> Result<String> {
    let mut source = String::new();
    write!(
        source,
        "{{\"schema\":\"semaprax.agent-definition.v1\",\"agent_id\":{},\"types\":[",
        quote_json(&declaration.stable_id),
    )
    .expect("String writes do not fail");
    for (index, role) in declaration.types.iter().enumerate() {
        if index != 0 {
            source.push(',');
        }
        write!(
            source,
            "{{\"role\":{},\"stable_id\":{}}}",
            quote_json(role.role.source_name()),
            quote_json(&role.stable_id),
        )
        .expect("String writes do not fail");
    }
    source.push_str("],\"operations\":[");
    for (index, operation) in declaration.operations.iter().enumerate() {
        if index != 0 {
            source.push(',');
        }
        write!(
            source,
            "{{\"role\":{},\"stable_id\":{},\"kind\":{}}}",
            quote_json(operation.role.source_name()),
            quote_json(&operation.stable_id),
            quote_json(operation_kind_name(operation.kind)),
        )
        .expect("String writes do not fail");
    }
    writeln!(source, "],\"runtime_v1\":{}}}", declaration.runtime_v1_json,)
        .expect("String writes do not fail");
    Ok(source)
}

const fn operation_kind_name(kind: AgentOperationKind) -> &'static str {
    match kind {
        AgentOperationKind::Deterministic => "deterministic",
        AgentOperationKind::Model => "model",
        AgentOperationKind::Effect => "effect",
    }
}

fn malformed(message: impl Into<String>) -> Vec<Diagnostic> {
    vec![Diagnostic::io("SPX-G558", message)]
}

fn invariant(message: impl Into<String>) -> Vec<Diagnostic> {
    vec![Diagnostic::io("SPX-G559", message)]
}
