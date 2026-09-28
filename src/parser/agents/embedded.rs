//! Ordinary function parsing for embedded deterministic Agent operations.
use super::*;
use crate::ast::{AgentModelWaitBinding, Function};

impl Parser {
    pub(super) fn agent_operation(
        &mut self,
        module: &str,
        functions: &mut Vec<Function>,
        expected_name: &str,
        role: AgentOperationRole,
        kind: AgentOperationKind,
        stable_id: String,
    ) -> Result<AgentOperationDeclaration, Diagnostic> {
        let start = self.current().span;
        let full_function = kind == AgentOperationKind::Deterministic
            && self
                .tokens
                .get(self.cursor + 2)
                .is_some_and(|token| matches!(token.kind, TokenKind::LParen | TokenKind::Lt));
        if full_function {
            let function = self.function(module, Some(stable_id.clone()))?;
            if function.name != expected_name {
                return Err(self.error_previous(
                    "SPX-P124",
                    "embedded Agent operation has the wrong role name",
                ));
            }
            let span = function.span;
            let embedded_function_index = Some(functions.len());
            functions.push(function);
            return Ok(AgentOperationDeclaration {
                role,
                kind,
                stable_id,
                span,
                embedded_function_index,
            });
        }
        match kind {
            AgentOperationKind::Deterministic => {}
            AgentOperationKind::Model => {
                self.keyword("model")?;
            }
            AgentOperationKind::Effect => {
                self.keyword("effect")?;
            }
        }
        self.keyword("fn")?;
        let (name, _) = self.ident("agent operation role")?;
        if name != expected_name {
            return Err(self.error_previous(
                "SPX-P124",
                format!("expected agent operation role `{expected_name}`"),
            ));
        }
        if !self.at(&TokenKind::Semicolon) && kind != AgentOperationKind::Deterministic {
            return Err(self.error_here(
                "SPX-P124",
                "model and effect Agent operations remain external declarations",
            ));
        }
        let end = self
            .expect(&TokenKind::Semicolon, "`;` after agent operation")?
            .span;
        Ok(AgentOperationDeclaration {
            role,
            kind,
            stable_id,
            span: start.merge(end),
            embedded_function_index: None,
        })
    }

    pub(super) fn agent_model_wait(
        &mut self,
    ) -> Result<Option<Box<AgentModelWaitBinding>>, Diagnostic> {
        if !self.at_keyword("model_wait_v1") {
            if matches!(&self.current().kind, TokenKind::Ident(value) if value.starts_with("model_wait"))
            {
                return Err(self.error_here("SPX-P124", "unknown model wait binding version"));
            }
            return Ok(None);
        }
        let start = self.bump().span;
        self.expect(&TokenKind::LBrace, "`{` before model_wait_v1")?;
        let (role, _) = self.ident("model wait role")?;
        if role != "propose" {
            return Err(self.error_previous("SPX-P124", "model_wait_v1 binds only propose"));
        }
        self.expect(&TokenKind::Eq, "`=` before model wait helper identity")?;
        let helper_id = match self.bump().kind.clone() {
            TokenKind::String(value) if crate::agent_definition::canonical_identifier(&value) => {
                value
            }
            _ => {
                return Err(self.error_previous(
                    "SPX-P124",
                    "model wait helper requires a canonical explicit identity of at most 240 bytes",
                ))
            }
        };
        self.expect(&TokenKind::Semicolon, "`;` after model wait helper")?;
        if !self.at(&TokenKind::RBrace) && !self.at(&TokenKind::Eof) {
            return Err(
                self.error_here("SPX-P124", "model_wait_v1 has exactly one propose binding")
            );
        }
        let end = self
            .expect(&TokenKind::RBrace, "`}` after model_wait_v1")?
            .span;
        if matches!(&self.current().kind, TokenKind::Ident(value) if value.starts_with("model_wait"))
        {
            return Err(self.error_here("SPX-P124", "model_wait_v1 may occur only once"));
        }
        Ok(Some(Box::new(AgentModelWaitBinding {
            helper_id,
            span: start.merge(end),
        })))
    }
}
