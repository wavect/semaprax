//! Obligation identity and the record types one obligation carries.
//!
//! See [`docs/ASSURANCE-MANIFEST-V1.md`](../../docs/ASSURANCE-MANIFEST-V1.md)
//! "Obligation identity" and "Canonical envelope" for the exact wire shape.

use super::lattice::AssuranceClass;

/// Closed obligation-kind vocabulary. Every token is part of the wire schema;
/// an unrecognized token is a replay failure (`SPX-Z103`), never a silently
/// accepted extension.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum ObligationKind {
    Precondition,
    Postcondition,
    OwnershipParameter,
    /// Reserved: needs the resolved-HIR `result_ownership` helper, not
    /// derived automatically by this tranche. See "Obligation derivation".
    OwnershipResult,
    /// Reserved: not derived automatically by this tranche.
    Effect,
    /// Reserved: not derived automatically by this tranche.
    Exhaustiveness,
    /// Reserved: not derived automatically by this tranche.
    ResourceCleanup,
    /// Reserved: not derived automatically by this tranche.
    ArchitectureLaw,
    /// Reserved: not derived automatically by this tranche.
    GeneratedInterface,
}

impl ObligationKind {
    pub const ALL: [Self; 9] = [
        Self::Precondition,
        Self::Postcondition,
        Self::OwnershipParameter,
        Self::OwnershipResult,
        Self::Effect,
        Self::Exhaustiveness,
        Self::ResourceCleanup,
        Self::ArchitectureLaw,
        Self::GeneratedInterface,
    ];

    #[must_use]
    pub const fn token(self) -> &'static str {
        match self {
            Self::Precondition => "precondition",
            Self::Postcondition => "postcondition",
            Self::OwnershipParameter => "ownership_parameter",
            Self::OwnershipResult => "ownership_result",
            Self::Effect => "effect",
            Self::Exhaustiveness => "exhaustiveness",
            Self::ResourceCleanup => "resource_cleanup",
            Self::ArchitectureLaw => "architecture_law",
            Self::GeneratedInterface => "generated_interface",
        }
    }

    /// Parse one exact kind token. Unknown or case-folded names are rejected.
    #[must_use]
    pub fn from_token(token: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|kind| kind.token() == token)
    }
}

/// Derive one obligation identity from its owning declaration's persistent
/// `stable_id`, this closed `kind`, and a structural `locator`. Every
/// variable-length segment is length-prefixed so concatenation can never
/// alias two different identities (the same technique
/// `hir::ids::FunctionInstanceId::derive` uses).
#[must_use]
pub fn obligation_id(kind: ObligationKind, declaration_id: &str, locator: &str) -> String {
    let token = kind.token();
    format!(
        "semaprax.obligation.v1:{}:{token}:{}:{declaration_id}:{}:{locator}",
        token.len(),
        declaration_id.len(),
        locator.len(),
    )
}

/// One method's contribution toward assuring an obligation.
///
/// Every optional field renders as JSON `null` when absent, never an
/// omitted key, so an independent reader can index a method record
/// positionally without a presence check.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MethodRecord {
    pub class: AssuranceClass,
    pub tool: String,
    pub tool_version: String,
    pub inputs: Vec<String>,
    pub bounds: Option<String>,
    pub assumption_ids: Vec<String>,
    pub proof_ref: Option<String>,
    pub counterexample_ref: Option<String>,
    pub runtime_fallback: bool,
    pub test_refs: Vec<String>,
    pub target: Option<String>,
    pub artifact_digest: Option<String>,
    pub detail: Option<String>,
}

impl MethodRecord {
    /// A minimal method record: only `class`, `tool`, and `tool_version` are
    /// meaningfully populated; every other field starts empty/absent. Useful
    /// for external callers (tests, and future SMT/model-checking/proof-
    /// kernel producers) that only need to set a few fields.
    #[must_use]
    pub fn new(
        class: AssuranceClass,
        tool: impl Into<String>,
        tool_version: impl Into<String>,
    ) -> Self {
        Self {
            class,
            tool: tool.into(),
            tool_version: tool_version.into(),
            inputs: Vec::new(),
            bounds: None,
            assumption_ids: Vec::new(),
            proof_ref: None,
            counterexample_ref: None,
            runtime_fallback: false,
            test_refs: Vec::new(),
            target: None,
            artifact_digest: None,
            detail: None,
        }
    }
}

/// One obligation: a stable identity, its owning declaration, its closed
/// `kind`, and every method record that currently bears on it. The reported
/// `classification` is never stored here; it is always recomputed from
/// `methods` by [`super::lattice::classification_of`] so it can never drift
/// from the records that justify it.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Obligation {
    pub id: String,
    pub declaration_id: String,
    pub kind: ObligationKind,
    pub methods: Vec<MethodRecord>,
}

impl Obligation {
    #[must_use]
    pub fn new(kind: ObligationKind, declaration_id: impl Into<String>, locator: &str) -> Self {
        let declaration_id = declaration_id.into();
        let id = obligation_id(kind, &declaration_id, locator);
        Self {
            id,
            declaration_id,
            kind,
            methods: Vec::new(),
        }
    }

    #[must_use]
    pub fn with_method(mut self, method: MethodRecord) -> Self {
        self.methods.push(method);
        self
    }
}

/// One explicit, owned, rationale-bearing assumption standing in for
/// evidence. Recording one is strictly more accountable than silence, but
/// it is never rendered as proof; see the assurance lattice.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AssumptionRecord {
    pub id: String,
    pub owner: String,
    pub rationale: String,
    pub scope: String,
    /// ISO-8601 `YYYY-MM-DD`, caller-supplied. Never filled in from the
    /// current clock: see "Determinism" in the owning specification.
    pub review_by: Option<String>,
    /// Obligation `id`s that depend on this assumption.
    pub dependents: Vec<String>,
}

/// Caller-supplied obligations and assumptions merged into a generated
/// manifest without requiring any formal-method backend to exist. This is
/// how `open`, `assumed`, `test_evidenced`, `attempt_inconclusive`,
/// `smt_proved`, `model_checked`, and `theorem_proved` records reach a
/// manifest today, and how a future producer for `effect`,
/// `exhaustiveness`, `resource_cleanup`, `architecture_law`, or
/// `generated_interface` obligations can plug in before this crate grows
/// its own derivation for them.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct ExternalRecords {
    pub obligations: Vec<Obligation>,
    pub assumptions: Vec<AssumptionRecord>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn kind_tokens_round_trip_through_from_token() {
        for kind in ObligationKind::ALL {
            assert_eq!(ObligationKind::from_token(kind.token()), Some(kind));
        }
        assert_eq!(ObligationKind::from_token("bogus"), None);
    }

    #[test]
    fn obligation_id_length_prefixing_prevents_boundary_aliasing() {
        // Without length-prefixing, declaration_id="ab" + locator="c" would
        // render the same concatenation as declaration_id="a" + locator="bc".
        // Length-prefixing must keep them distinct despite the naive
        // concatenation colliding.
        let first = obligation_id(ObligationKind::Precondition, "ab", "c");
        let second = obligation_id(ObligationKind::Precondition, "a", "bc");
        assert_ne!(first, second);
    }

    #[test]
    fn obligation_id_is_stable_for_identical_inputs() {
        let first = obligation_id(ObligationKind::OwnershipParameter, "app.fetch", "param:0");
        let second = obligation_id(ObligationKind::OwnershipParameter, "app.fetch", "param:0");
        assert_eq!(first, second);
    }

    #[test]
    fn obligation_id_changes_with_locator_index() {
        let first = obligation_id(ObligationKind::Precondition, "app.fetch", "require:0");
        let second = obligation_id(ObligationKind::Precondition, "app.fetch", "require:1");
        assert_ne!(first, second);
    }
}
