//! Declared session protocols (issue #297): the `.spx` surface for the
//! message-order model `crate::session_protocol` checks at runtime.
//!
//! A `session protocol` declaration is checked and erased. The verifier lowers
//! it to a [`crate::session_protocol::spec::ProtocolSpec`] and runs the
//! kernel's static validation and bounded graph check; no backend lowers it,
//! and it grants no effect, capability, or resource authority. See
//! `docs/SESSION-PROTOCOL-TYPES-V1.md`.

use super::Span;

/// One identifier-shaped name with its source span (a state, label, payload
/// tag, capability, or cleanup operation).
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SessionProtocolName {
    pub name: String,
    pub span: Span,
}

/// `session protocol "<name>" { ... }`.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SessionProtocolDeclaration {
    pub stable_id: String,
    pub explicit_id: bool,
    /// The protocol's versioned name, a string literal (for example
    /// `"database-transaction-v1"`).
    pub name: String,
    pub name_span: Span,
    /// Declared state set, in source order.
    pub states: Vec<SessionProtocolName>,
    pub initial: SessionProtocolName,
    /// Terminal states with their canonical cleanup inventories, in source
    /// order. Cleanup order is runtime order and is never sorted.
    pub terminals: Vec<SessionProtocolTerminal>,
    /// Declared transitions, in source order.
    pub transitions: Vec<SessionProtocolTransition>,
    pub span: Span,
}

/// `terminal <State> cleanup { op, ... }`.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SessionProtocolTerminal {
    pub state: SessionProtocolName,
    pub cleanup: Vec<SessionProtocolName>,
    pub span: Span,
}

/// Closed transition kind vocabulary, mirroring
/// [`crate::session_protocol::spec::Kind`].
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SessionProtocolKind {
    Send,
    Receive,
    Call,
    Return,
    Cancel,
    Timeout,
    Fail,
}

impl SessionProtocolKind {
    pub const ALL: [Self; 7] = [
        Self::Send,
        Self::Receive,
        Self::Call,
        Self::Return,
        Self::Cancel,
        Self::Timeout,
        Self::Fail,
    ];

    pub fn keyword(self) -> &'static str {
        match self {
            Self::Send => "send",
            Self::Receive => "receive",
            Self::Call => "call",
            Self::Return => "return",
            Self::Cancel => "cancel",
            Self::Timeout => "timeout",
            Self::Fail => "fail",
        }
    }

    pub fn from_keyword(keyword: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|kind| kind.keyword() == keyword)
    }
}

/// `-> <State>` or `-> choice { label: State, ... }`.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum SessionProtocolNext {
    Then(SessionProtocolName),
    Choice(Vec<(SessionProtocolName, SessionProtocolName)>),
}

/// `on <From> <label>: <kind> <Payload> [requires capability <cap>]
/// [consumes resource] [via "<function-id>"] -> <next>;`
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SessionProtocolTransition {
    pub from: SessionProtocolName,
    pub label: SessionProtocolName,
    pub kind: SessionProtocolKind,
    pub payload: SessionProtocolName,
    /// Ordering metadata naming authority the caller must separately hold.
    /// It never grants that authority.
    pub capability: Option<SessionProtocolName>,
    pub consumes_resource: bool,
    /// Persistent id of the ordinary function that realizes this transition
    /// in checked source, when one exists.
    pub via: Option<SessionProtocolName>,
    pub next: SessionProtocolNext,
    pub span: Span,
}
