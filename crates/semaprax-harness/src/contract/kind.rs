//! First-wave capability kinds and their versioned operation sets.

/// A capability kind the host implements. New kinds need a host
/// implementation and a versioned contract; new providers do not.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum CapabilityKind {
    ContextRepository,
    CommandView,
    DecisionEvaluate,
    ModelGenerate,
    SkillCatalog,
}

/// A kind at one contract version.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CapabilityRef {
    pub kind: CapabilityKind,
    pub version: u32,
}

impl CapabilityKind {
    pub const ALL: [CapabilityKind; 5] = [
        CapabilityKind::ContextRepository,
        CapabilityKind::CommandView,
        CapabilityKind::DecisionEvaluate,
        CapabilityKind::ModelGenerate,
        CapabilityKind::SkillCatalog,
    ];

    pub fn as_str(&self) -> &'static str {
        match self {
            Self::ContextRepository => "context.repository",
            Self::CommandView => "command.view",
            Self::DecisionEvaluate => "decision.evaluate",
            Self::ModelGenerate => "model.generate",
            Self::SkillCatalog => "skill.catalog",
        }
    }

    pub fn parse(s: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|k| k.as_str() == s)
    }

    /// Contract versions the host implements for this kind.
    pub fn supported_versions(&self) -> &'static [u32] {
        &[SUPPORTED_VERSION]
    }

    /// Closed operation vocabulary of this kind at v1.
    pub fn operations(&self) -> &'static [&'static str] {
        match self {
            Self::ContextRepository => &["orient", "search", "skeleton", "references"],
            Self::CommandView => &["view", "wrap", "plan"],
            Self::DecisionEvaluate => &["evaluate"],
            Self::ModelGenerate => &["generate"],
            Self::SkillCatalog => &["list", "load"],
        }
    }
}

/// Current (only) contract version of every first-wave kind.
pub const SUPPORTED_VERSION: u32 = 1;

/// `skill.evolve/v1` (HN-15): experimental, host-invoked evolution capability.
///
/// It is deliberately not a member of [`CapabilityKind::ALL`]: it is never
/// bound by profile resolution or negotiated as a default provider. The host
/// calls it only for an explicit, isolated evolution experiment
/// (`docs/HARNESS-EVOLUTION-V1.md`). Payloads are validated by
/// `contract::payload::evolve`.
pub struct EvolveCapability;

impl EvolveCapability {
    pub const KIND: &'static str = "skill.evolve";
    pub const VERSION: u32 = 1;
    /// `evolve`: ingest traces, consolidate the wiki, propose one skill.
    /// `solve`: run one held-out task with or without a skill (the host grades).
    pub const OPERATIONS: [&'static str; 2] = ["evolve", "solve"];
}
