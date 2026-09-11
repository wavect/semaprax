//! What one checked lifecycle stage, or one rich effect operation's
//! argument/result slot, is bound to accept.
//!
//! A [`StageBinding`] is captured once from a real
//! [`CompiledInteractionSchema`], at the exact root type and schema
//! revision that schema derived. [`StageBinding::admit`] then refuses,
//! before any dispatch, every one of the checked admission failures this
//! carrier is required to catch: a wrong nominal record, a wrong variant
//! case, and a stale structural schema (the revision digest changes on any
//! structural edit — an added/removed/retyped field, a changed case, a
//! changed nested reference — but never on a display rename, so a binding
//! built from a renamed-only schema still admits values decoded under the
//! original schema, and a binding built from a structurally different
//! schema refuses them).
//!
//! Field-order mutation is already refused one layer down, by
//! `agent_interaction_schema::decode`'s byte-exact canonical-replay check
//! (a reordered document fails to re-render byte-identical to itself and is
//! refused before a `DecodedInteractionValue` ever exists) — this module
//! does not re-implement that check, only relies on never accepting a value
//! except through that decoder.

use crate::agent_interaction_schema::{CompiledInteractionSchema, DecodedInteractionValue};
use crate::diagnostic::Diagnostic;

use super::refusal;

/// The four deterministic stage roles of one Agent lifecycle, mirroring
/// `agent_lifecycle::stages::DETERMINISTIC_ROLES`'s normative order. This is
/// an independent, additive naming: it does not read or alter that
/// module's own (private) role list, and a future direct integration is
/// free to bind these one-for-one.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LifecycleStageRole {
    Initialize,
    Observe,
    Authorize,
    Reduce,
    /// A rich effect operation's argument or result slot, not one of the
    /// four deterministic stages.
    Effect,
}

impl LifecycleStageRole {
    #[must_use]
    pub fn name(self) -> &'static str {
        match self {
            Self::Initialize => "initialize",
            Self::Observe => "observe",
            Self::Authorize => "authorize",
            Self::Reduce => "reduce",
            Self::Effect => "effect",
        }
    }
}

/// The exact nominal type, schema revision, and (for a variant root) the
/// optional exact case one checked stage or operation slot requires.
///
/// Built once from a real derived [`CompiledInteractionSchema`]; a value is
/// never admitted against a hand-built or caller-supplied binding.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StageBinding {
    role: &'static str,
    root_type_id: String,
    schema_digest: String,
    expected_case: Option<String>,
}

impl StageBinding {
    /// Binds `role` to exactly the root type and schema revision
    /// `schema` was derived at.
    #[must_use]
    pub fn new(role: LifecycleStageRole, schema: &CompiledInteractionSchema) -> Self {
        Self {
            role: role.name(),
            root_type_id: schema.schema().root_type_id().to_owned(),
            schema_digest: schema.schema().digest().to_owned(),
            expected_case: None,
        }
    }

    /// Narrows this binding to admit only one exact variant case. The root
    /// type must be a variant for any value to ever satisfy this; a record
    /// root can never admit under a case-narrowed binding.
    #[must_use]
    pub fn expect_case(mut self, case: impl Into<String>) -> Self {
        self.expected_case = Some(case.into());
        self
    }

    #[must_use]
    pub fn role(&self) -> &str {
        self.role
    }

    #[must_use]
    pub fn root_type_id(&self) -> &str {
        &self.root_type_id
    }

    #[must_use]
    pub fn schema_digest(&self) -> &str {
        &self.schema_digest
    }

    #[must_use]
    pub fn expected_case(&self) -> Option<&str> {
        self.expected_case.as_deref()
    }

    /// Admits one decoded value against this exact binding, before any
    /// dispatch. Consumes `value`: an admitted value is transferred to the
    /// caller exactly once, never re-admitted.
    ///
    /// Refuses (`SPX-Z210`):
    /// - `stage.wrong_nominal_type` — `value`'s root type is not this
    ///   binding's root type.
    /// - `stage.schema_mismatch` — `value`'s schema revision is not the
    ///   exact revision this binding was built from (covers a genuinely
    ///   stale structural schema; never covers a display-only rename,
    ///   since the revision digest is rename-invariant).
    /// - `stage.wrong_variant` — this binding names an exact case and
    ///   `value`'s case disagrees, or `value` is not a variant at all.
    pub fn admit(
        &self,
        value: DecodedInteractionValue,
    ) -> Result<DecodedInteractionValue, Diagnostic> {
        if value.root_type_id() != self.root_type_id {
            return Err(refusal("SPX-Z210", "stage.wrong_nominal_type"));
        }
        if value.schema_digest() != self.schema_digest {
            return Err(refusal("SPX-Z210", "stage.schema_mismatch"));
        }
        if let Some(expected) = &self.expected_case {
            match value.value().case() {
                Some(case) if case == expected => {}
                _ => return Err(refusal("SPX-Z210", "stage.wrong_variant")),
            }
        }
        Ok(value)
    }
}
