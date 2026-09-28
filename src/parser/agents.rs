use std::collections::BTreeSet;

use crate::ast::{
    AgentDeclaration, AgentOperationDeclaration, AgentOperationKind, AgentOperationRole,
    AgentTypeRole, AgentTypeRoleDeclaration,
};
use crate::diagnostic::Diagnostic;
use crate::lexer::TokenKind;

use super::Parser;

mod embedded;

const MAX_RUNTIME_V1_JSON_BYTES: usize = 1_310_720;
const TYPE_ROLES: [(&str, AgentTypeRole); 6] = [
    ("task", AgentTypeRole::Task),
    ("state", AgentTypeRole::State),
    ("observation", AgentTypeRole::Observation),
    ("proposal", AgentTypeRole::Proposal),
    ("outcome", AgentTypeRole::Outcome),
    ("result", AgentTypeRole::Result),
];
const OPERATIONS: [(&str, AgentOperationRole, AgentOperationKind); 6] = [
    (
        "initialize",
        AgentOperationRole::Initialize,
        AgentOperationKind::Deterministic,
    ),
    (
        "observe",
        AgentOperationRole::Observe,
        AgentOperationKind::Deterministic,
    ),
    (
        "propose",
        AgentOperationRole::Propose,
        AgentOperationKind::Model,
    ),
    (
        "authorize",
        AgentOperationRole::Authorize,
        AgentOperationKind::Deterministic,
    ),
    (
        "execute",
        AgentOperationRole::Execute,
        AgentOperationKind::Effect,
    ),
    (
        "reduce",
        AgentOperationRole::Reduce,
        AgentOperationKind::Deterministic,
    ),
];

impl Parser {
    pub(super) fn agent(
        &mut self,
        module: &str,
        functions: &mut Vec<crate::ast::Function>,
        stable_id: Option<String>,
    ) -> Result<AgentDeclaration, Diagnostic> {
        let start = self.keyword("agent")?.span;
        let stable_id = stable_id.ok_or_else(|| {
            self.error_here(
                "SPX-P124",
                "agent declarations require an explicit @id identity",
            )
        })?;
        let (name, name_span) = self.ident("agent name")?;
        self.expect(&TokenKind::LBrace, "`{` before agent declaration")?;
        self.keyword("types")?;
        self.expect(&TokenKind::LBrace, "`{` before agent type roles")?;
        let mut identities = BTreeSet::new();
        identities.insert(stable_id.clone());
        let mut types = Vec::with_capacity(TYPE_ROLES.len());
        for (expected_name, role) in TYPE_ROLES {
            let role_id = self.required_role_id("agent type role")?;
            if !identities.insert(role_id.clone()) {
                return Err(
                    self.error_previous("SPX-P124", "agent identities must be locally unique")
                );
            }
            let role_start = self.keyword("type")?.span;
            let (actual_name, _) = self.ident("agent type role")?;
            if actual_name != expected_name {
                return Err(self.error_previous(
                    "SPX-P124",
                    format!("expected agent type role `{expected_name}`"),
                ));
            }
            let end = self
                .expect(&TokenKind::Semicolon, "`;` after agent type role")?
                .span;
            types.push(AgentTypeRoleDeclaration {
                role,
                stable_id: role_id,
                span: role_start.merge(end),
            });
        }
        self.expect(&TokenKind::RBrace, "`}` after agent type roles")?;

        self.keyword("operations")?;
        self.expect(&TokenKind::LBrace, "`{` before agent operations")?;
        let mut operations = Vec::with_capacity(OPERATIONS.len());
        for (expected_name, role, kind) in OPERATIONS {
            let operation_id = self.required_role_id("agent operation")?;
            if !identities.insert(operation_id.clone()) {
                return Err(
                    self.error_previous("SPX-P124", "agent identities must be locally unique")
                );
            }
            operations.push(self.agent_operation(
                module,
                functions,
                expected_name,
                role,
                kind,
                operation_id,
            )?);
        }
        self.expect(&TokenKind::RBrace, "`}` after agent operations")?;

        let model_wait = self.agent_model_wait()?;
        self.keyword("runtime_v1")?;
        self.expect(&TokenKind::LBrace, "`{` before runtime_v1 compatibility")?;
        self.keyword("canonical_json")?;
        let runtime_v1_json = match self.bump().kind.clone() {
            TokenKind::String(value) => value,
            _ => {
                return Err(self.error_previous(
                    "SPX-P124",
                    "runtime_v1 canonical_json expects a string literal",
                ));
            }
        };
        if runtime_v1_json.len() > MAX_RUNTIME_V1_JSON_BYTES {
            return Err(self.error_previous(
                "SPX-P124",
                "runtime_v1 canonical_json exceeds the AgentDefinition v1 byte bound",
            ));
        }
        self.expect(&TokenKind::Semicolon, "`;` after runtime_v1 canonical_json")?;
        self.expect(&TokenKind::RBrace, "`}` after runtime_v1 compatibility")?;
        let end = self
            .expect(&TokenKind::RBrace, "`}` after agent declaration")?
            .span;
        Ok(AgentDeclaration {
            stable_id,
            name,
            name_span,
            types,
            operations,
            runtime_v1_json,
            span: start.merge(end),
            model_wait,
        })
    }

    fn required_role_id(&mut self, subject: &'static str) -> Result<String, Diagnostic> {
        self.stable_id_attribute()?.ok_or_else(|| {
            self.error_here(
                "SPX-P124",
                format!("{subject} requires an explicit @id identity"),
            )
        })
    }
}
