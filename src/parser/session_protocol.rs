//! Parser for declared session protocols (issue #297).
//!
//! The grammar is closed and order-fixed so the canonical formatter has one
//! spelling:
//!
//! ```text
//! session protocol "<name>" {
//!     states { S, ... }
//!     initial S;
//!     terminal S cleanup { op, ... }            // zero or more
//!     on S label: kind Payload [requires capability cap.name]
//!         [consumes resource] [via "<function-id>"] -> S | choice { label: S, ... };
//! }
//! ```
//!
//! Structural and semantic rules (duplicates, unknown states, kernel
//! validation, `via` binding, capability attribution) are verifier checks in
//! `crate::source_verify::session_protocol` (`SPX-K1xx`); this module only
//! admits the shape and enforces the declaration's capacity bounds.

use crate::ast::{
    SessionProtocolDeclaration, SessionProtocolKind, SessionProtocolName, SessionProtocolNext,
    SessionProtocolTerminal, SessionProtocolTransition,
};
use crate::diagnostic::Diagnostic;
use crate::lexer::TokenKind;

use super::Parser;

/// Maximum declared session protocols per module.
pub(crate) const MAX_SESSION_PROTOCOLS: usize = 64;
/// Maximum states, terminals, or cleanup operations in one declaration.
pub(crate) const MAX_SESSION_PROTOCOL_STATES: usize = 64;
/// Maximum transitions in one declaration.
pub(crate) const MAX_SESSION_PROTOCOL_TRANSITIONS: usize = 256;
/// Maximum branches in one `choice`.
pub(crate) const MAX_SESSION_PROTOCOL_BRANCHES: usize = 64;

impl Parser {
    pub(super) fn session_protocol(
        &mut self,
        module: &str,
        stable_id: Option<String>,
    ) -> Result<SessionProtocolDeclaration, Diagnostic> {
        let start = self.keyword("session")?.span;
        self.keyword("protocol")?;
        let name_token = self.bump().clone();
        let name = match name_token.kind {
            TokenKind::String(value) if !value.is_empty() => value,
            _ => {
                return Err(self.error_previous(
                    "SPX-P105",
                    "expected a non-empty string literal session protocol name",
                ))
            }
        };
        let name_span = name_token.span;
        let explicit_id = stable_id.is_some();
        let stable_id =
            stable_id.unwrap_or_else(|| format!("auto:session_protocol:{module}.{name}"));
        self.expect(&TokenKind::LBrace, "`{` before session protocol body")?;

        self.keyword("states")?;
        let states = self.session_name_set("state name", "session protocol states")?;
        self.keyword("initial")?;
        let initial = self.session_name("initial state name")?;
        self.expect(&TokenKind::Semicolon, "`;` after initial state")?;

        let mut terminals = Vec::new();
        while self.at_keyword("terminal") {
            if terminals.len() >= MAX_SESSION_PROTOCOL_STATES {
                return Err(self.error_here("SPX-K106", "too many session protocol terminals"));
            }
            let terminal_start = self.bump().span;
            let state = self.session_name("terminal state name")?;
            self.keyword("cleanup")?;
            let cleanup = self.session_name_set("cleanup operation name", "cleanup operations")?;
            terminals.push(SessionProtocolTerminal {
                state,
                cleanup,
                span: terminal_start.merge(self.previous_span()),
            });
        }

        let mut transitions = Vec::new();
        while self.at_keyword("on") {
            if transitions.len() >= MAX_SESSION_PROTOCOL_TRANSITIONS {
                return Err(self.error_here("SPX-K106", "too many session protocol transitions"));
            }
            transitions.push(self.session_transition()?);
        }
        if !self.at(&TokenKind::RBrace) {
            return Err(self.error_here(
                "SPX-P106",
                "expected `terminal`, `on`, or `}` in session protocol body",
            ));
        }
        let end = self.bump().span;
        Ok(SessionProtocolDeclaration {
            stable_id,
            explicit_id,
            name,
            name_span,
            states,
            initial,
            terminals,
            transitions,
            span: start.merge(end),
        })
    }

    fn session_transition(&mut self) -> Result<SessionProtocolTransition, Diagnostic> {
        let start = self.keyword("on")?.span;
        let from = self.session_name("transition source state")?;
        let label = self.session_name("transition label")?;
        self.expect(&TokenKind::Colon, "`:` after transition label")?;
        let (kind_word, kind_span) = self.ident("transition kind")?;
        let Some(kind) = SessionProtocolKind::from_keyword(&kind_word) else {
            return Err(Diagnostic::error(
                "SPX-P105",
                "expected transition kind `send`, `receive`, `call`, `return`, `cancel`, `timeout`, or `fail`",
                kind_span,
            )
            .at_path(&self.path));
        };
        let payload = self.session_name("transition payload tag")?;
        let capability = if self.at_keyword("requires") {
            self.bump();
            self.keyword("capability")?;
            let (name, span) = self.qualified_ident("capability name")?;
            Some(SessionProtocolName { name, span })
        } else {
            None
        };
        let consumes_resource = if self.at_keyword("consumes") {
            self.bump();
            self.keyword("resource")?;
            true
        } else {
            false
        };
        let via = if self.at_keyword("via") {
            self.bump();
            let token = self.bump().clone();
            match token.kind {
                TokenKind::String(value) if !value.is_empty() => Some(SessionProtocolName {
                    name: value,
                    span: token.span,
                }),
                _ => {
                    return Err(self.error_previous(
                        "SPX-P105",
                        "expected a function persistent id string after `via`",
                    ))
                }
            }
        } else {
            None
        };
        self.expect(&TokenKind::Arrow, "`->` before transition target")?;
        let next = if self.at_keyword("choice") {
            self.bump();
            self.expect(&TokenKind::LBrace, "`{` before choice branches")?;
            let mut branches = Vec::new();
            loop {
                if branches.len() >= MAX_SESSION_PROTOCOL_BRANCHES {
                    return Err(self.error_here("SPX-K106", "too many choice branches"));
                }
                let branch = self.session_name("choice label")?;
                self.expect(&TokenKind::Colon, "`:` after choice label")?;
                let target = self.session_name("choice target state")?;
                branches.push((branch, target));
                if !self.take(&TokenKind::Comma) || self.at(&TokenKind::RBrace) {
                    break;
                }
            }
            self.expect(&TokenKind::RBrace, "`}` after choice branches")?;
            SessionProtocolNext::Choice(branches)
        } else {
            SessionProtocolNext::Then(self.session_name("transition target state")?)
        };
        let end = self
            .expect(
                &TokenKind::Semicolon,
                "`;` after session protocol transition",
            )?
            .span;
        Ok(SessionProtocolTransition {
            from,
            label,
            kind,
            payload,
            capability,
            consumes_resource,
            via,
            next,
            span: start.merge(end),
        })
    }

    fn session_name(&mut self, description: &str) -> Result<SessionProtocolName, Diagnostic> {
        let (name, span) = self.ident(description)?;
        Ok(SessionProtocolName { name, span })
    }

    /// `{ name, name, ... }`, possibly empty, trailing comma admitted.
    fn session_name_set(
        &mut self,
        description: &str,
        set: &str,
    ) -> Result<Vec<SessionProtocolName>, Diagnostic> {
        self.expect(&TokenKind::LBrace, &format!("`{{` before {set}"))?;
        let mut names = Vec::new();
        while !self.at(&TokenKind::RBrace) {
            if names.len() >= MAX_SESSION_PROTOCOL_STATES {
                return Err(self.error_here("SPX-K106", format!("too many {set}")));
            }
            names.push(self.session_name(description)?);
            if !self.take(&TokenKind::Comma) {
                break;
            }
        }
        self.expect(&TokenKind::RBrace, &format!("`}}` after {set}"))?;
        Ok(names)
    }
}
