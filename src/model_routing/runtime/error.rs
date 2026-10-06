//! Closed runtime-routing refusals. Every variant is decided locally before a
//! generation adapter is constructed; none carries provider output.

use crate::diagnostic::Diagnostic;
use crate::model_budget_policy::DurablePolicyBindingRefusal;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RuntimeRoutingError {
    /// The host routing configuration is malformed (empty, duplicate id,
    /// unbounded text, ...).
    InvalidConfig(String),
    /// A profile's deployment does not bind to the route's semantic
    /// definition: unknown/wrong definition digest, capability or tool
    /// expansion, model incompatibility or widened limits (`SPX-G55x`).
    Deployment {
        profile: String,
        code: String,
        message: String,
    },
    /// A profile binds a different semantic definition than the route.
    SemanticMismatch { profile: String },
    /// A profile's ordered provider policy or limits fail the durable checks.
    PolicyBinding {
        profile: String,
        refusal: DurablePolicyBindingRefusal,
    },
    /// The runtime feature projection is out of bounds.
    InvalidFeatures(String),
    /// An operator pin names no approved profile.
    UnknownPin(String),
    /// An operator pin names an approved profile the screen excludes. Pins
    /// never fall back to another profile.
    InadmissiblePin { profile: String, reason: String },
    /// No approved profile passes the screen; refused before generation.
    NoAdmissibleProfile { excluded: Vec<(String, String)> },
    /// The decision core refused (for example a `refuse` fallback policy).
    Router { code: String, message: String },
    /// A retained route names a deployment the current set no longer
    /// approves: further work stops, history is not rewritten or rebound.
    ProfileNotApproved { deployment: String },
    /// Execution roots could not be bound for the selected deployment.
    Execution { code: String, message: String },
    /// A retained route record or checkpoint envelope does not match.
    RecordMismatch(String),
    /// The route record checkpoint could not be committed.
    Checkpoint,
    /// MR-10: a turn or delegation was refused at the session boundary.
    Session(String),
    /// MR-11: a runtime choice selection was refused at the authorize stage
    /// (`SPX-HPJ024` recheck, `SPX-HPJ026` grant or argument mismatch).
    Choice { code: String, message: String },
}

impl RuntimeRoutingError {
    pub(crate) fn from_diagnostics(profile: &str, diagnostics: &[Diagnostic]) -> Self {
        let first = diagnostics.first();
        Self::Deployment {
            profile: profile.to_owned(),
            code: first.map_or("", |d| d.code).to_owned(),
            message: first.map_or_else(String::new, |d| d.message.clone()),
        }
    }

    pub(crate) fn execution(diagnostics: &[Diagnostic]) -> Self {
        let first = diagnostics.first();
        Self::Execution {
            code: first.map_or("", |d| d.code).to_owned(),
            message: first.map_or_else(String::new, |d| d.message.clone()),
        }
    }

    pub(crate) fn session(why: impl Into<String>) -> Self {
        Self::Session(why.into())
    }
}

impl std::fmt::Display for RuntimeRoutingError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "runtime routing refused: {self:?}")
    }
}

impl std::error::Error for RuntimeRoutingError {}
