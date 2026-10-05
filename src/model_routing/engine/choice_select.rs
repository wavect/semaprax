//! `choice-select/v1` decision flow and its typed outcome (MR-11).
//!
//! [`select_choice`] screens the caller's candidate set, takes the zero/one
//! option paths without inference, and otherwise consults an attached provider
//! only when the adapter negotiated the task ([`CHOICE_WIRE_VERSION`]) and, in
//! `Auto` mode, holds a passed gate for exactly `choice-select/v1`. The answer
//! is validated like a `model-route/v2` result and mapped back to the caller's
//! own stable id. Every failure is an explicit [`ChoiceOutcome::Abstained`] or
//! [`ChoiceOutcome::Refused`]; there is no default action.
//!
//! A [`ChoiceSelection`] has no public constructor and names an admitted
//! option only. It is advisory: the caller's authorize/execute stage rechecks
//! it ([`ChoiceSelection::recheck`]) before any effect.

use super::call::{AbstentionReason, ResultV2, ScoreKind};
use super::choice::{
    screen, screen_option, ChoiceInputs, ChoiceOption, DestinationKind, PreparedChoice, Rejection,
    SingleOption, CHOICE_TASK, CHOICE_WIRE_VERSION,
};
use super::diag::{DecisionResult, Diagnostic};
use super::provider::{ConfiguredProvider, DecisionCall, DecisionInvoker, ProviderMode};
use super::request::DecisionRequest;
use super::route_v2::Modality;
use super::router::{RouteContext, WireInfo};
use super::wire::{self, check_against_request, Direction};

/// Why no option was selected. Each is explicit; none becomes a default.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ChoiceAbstain {
    /// The provider abstained natively.
    Native,
    /// The chosen option's mass is below the profile threshold.
    HostThreshold,
    /// Two or more admitted options and no provider attached.
    NoProvider,
    /// The adapter did not negotiate `choice-select/v1` (decision.evaluate v3).
    UnsupportedAdapter,
    /// The request exceeds the adapter's declared profile (options, state).
    ProfileLimits,
    /// `Auto` mode without a passed gate for `choice-select/v1`.
    NotQualified,
    /// The known remaining budget cannot cover the provider call reserve.
    BudgetExhausted,
    /// One admitted option and the policy asks for explicit abstention.
    SingleOptionPolicy,
    Unavailable,
    Timeout,
    InvalidResult,
    /// The provider named something that is not an admitted selection id.
    RejectedChoice,
    CallCapExhausted,
    LatencyExhausted,
    RecursionBlocked,
    IdentityMismatch,
}

impl ChoiceAbstain {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Native => "native",
            Self::HostThreshold => "host_threshold",
            Self::NoProvider => "no_provider",
            Self::UnsupportedAdapter => "unsupported_adapter",
            Self::ProfileLimits => "profile_limits",
            Self::NotQualified => "not_qualified",
            Self::BudgetExhausted => "budget_exhausted",
            Self::SingleOptionPolicy => "single_option_policy",
            Self::Unavailable => "unavailable",
            Self::Timeout => "timeout",
            Self::InvalidResult => "invalid_result",
            Self::RejectedChoice => "rejected_choice",
            Self::CallCapExhausted => "call_cap_exhausted",
            Self::LatencyExhausted => "latency_exhausted",
            Self::RecursionBlocked => "recursion_blocked",
            Self::IdentityMismatch => "identity_mismatch",
        }
    }
}

/// How a selection was made.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ChoiceSource {
    /// Exactly one admitted option: zero-model path, no inference.
    SingleAdmitted,
    /// A provider's validated answer.
    Provider,
}

/// What the decision saw: admitted and rejected options, wire facts and the
/// digests to journal. Present on every outcome.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ChoiceReport {
    pub admitted: Vec<String>,
    pub rejected: Vec<(String, Rejection)>,
    pub option_set_digest: String,
    /// Digest of the prepared payload, when one was prepared.
    pub request_digest: Option<String>,
    pub router_calls: u32,
    pub router_ms: u64,
    /// `version` is [`CHOICE_WIRE_VERSION`] once a call was attempted.
    pub wire: WireInfo,
    pub provider_id: Option<String>,
    pub provider_status: Option<&'static str>,
}

/// The selected allowed case. No public constructor.
#[derive(Clone, Debug, PartialEq)]
pub struct ChoiceSelection {
    option: ChoiceOption,
    source: ChoiceSource,
    scores: Option<(ScoreKind, f64)>,
}

impl ChoiceSelection {
    /// The caller's own stable id of the selected option.
    pub fn id(&self) -> &str {
        &self.option.id
    }

    pub fn kind(&self) -> DestinationKind {
        self.option.kind
    }

    pub fn source(&self) -> ChoiceSource {
        self.source
    }

    /// The chosen option's score and its kind (never a success probability).
    pub fn score(&self) -> Option<(ScoreKind, f64)> {
        self.scores
    }

    /// The caller's own object for the selection; `None` when the caller no
    /// longer holds it (it must then refuse, never trust the selection).
    pub fn resolve<'h, T>(&self, held: &'h [T], id_of: impl Fn(&T) -> &str) -> Option<&'h T> {
        held.iter().find(|t| id_of(t) == self.id())
    }

    /// Re-screen the selection against the live inputs at dispatch
    /// (`SPX-HPJ024`): the option must still be supplied with the same
    /// declaration and still pass every screen.
    pub fn recheck<'i>(&self, live: &'i ChoiceInputs) -> DecisionResult<&'i ChoiceOption> {
        let bad = |why: &str| {
            Diagnostic::new(
                "SPX-HPJ024",
                format!("selected destination `{}` {why}", self.id()),
            )
        };
        let o = live
            .options
            .iter()
            .find(|o| o.id == self.option.id)
            .ok_or_else(|| bad("is no longer supplied"))?;
        if *o != self.option {
            return Err(bad("changed its declaration since the decision"));
        }
        screen_option(&live.question, o)
            .map_err(|r| bad(&format!("no longer passes screening ({})", r.as_str())))?;
        Ok(o)
    }
}

/// The typed result an application dispatches on exhaustively.
#[derive(Clone, Debug, PartialEq)]
pub enum ChoiceOutcome {
    Selected {
        selection: ChoiceSelection,
        report: ChoiceReport,
    },
    Abstained {
        reason: ChoiceAbstain,
        report: ChoiceReport,
    },
    Refused {
        diagnostic: Diagnostic,
        report: ChoiceReport,
    },
}

impl ChoiceOutcome {
    pub fn report(&self) -> &ChoiceReport {
        match self {
            Self::Selected { report, .. }
            | Self::Abstained { report, .. }
            | Self::Refused { report, .. } => report,
        }
    }

    /// The selection, if any.
    pub fn selection(&self) -> Option<&ChoiceSelection> {
        match self {
            Self::Selected { selection, .. } => Some(selection),
            _ => None,
        }
    }
}

/// Decide one finite choice over caller-supplied options. `provider` is the
/// attached decision adapter, if any; with none, two or more admitted options
/// abstain (`NoProvider`) rather than pick one.
pub fn select_choice<I: ?Sized + DecisionInvoker>(
    inputs: &ChoiceInputs,
    ctx: &RouteContext,
    provider: Option<&mut ConfiguredProvider<'_, I>>,
) -> ChoiceOutcome {
    let scr = match screen(inputs) {
        Ok(s) => s,
        Err(diagnostic) => {
            return ChoiceOutcome::Refused {
                diagnostic,
                report: ChoiceReport::default(),
            }
        }
    };
    let mut report = ChoiceReport {
        admitted: scr.admitted.iter().map(|o| o.id.clone()).collect(),
        rejected: scr.rejected.clone(),
        option_set_digest: scr.option_set_digest(),
        ..ChoiceReport::default()
    };
    let abstain = |reason, report| ChoiceOutcome::Abstained { reason, report };
    match scr.admitted.len() {
        0 => {
            let why: Vec<String> = scr
                .rejected
                .iter()
                .map(|(id, r)| format!("{id}: {}", r.as_str()))
                .collect();
            return ChoiceOutcome::Refused {
                diagnostic: Diagnostic::new(
                    "SPX-HPJ022",
                    format!(
                        "no admissible destination for `{}` ({})",
                        inputs.question.schema,
                        if why.is_empty() {
                            "none supplied".to_string()
                        } else {
                            why.join(", ")
                        }
                    ),
                ),
                report,
            };
        }
        1 => {
            return match inputs.policy.single_option {
                SingleOption::Select => ChoiceOutcome::Selected {
                    selection: ChoiceSelection {
                        option: scr.admitted[0].clone(),
                        source: ChoiceSource::SingleAdmitted,
                        scores: None,
                    },
                    report,
                },
                SingleOption::Abstain => abstain(ChoiceAbstain::SingleOptionPolicy, report),
            };
        }
        _ => {}
    }
    let prepared = match PreparedChoice::prepare(inputs, &scr.admitted) {
        Ok(p) => p,
        Err(diagnostic) => return ChoiceOutcome::Refused { diagnostic, report },
    };
    report.request_digest = Some(prepared.request_digest());
    report.wire.rendered_digest = Some(prepared.rendered.digest.clone());
    report.wire.max_wire_bytes = Some(prepared.max_wire_bytes);
    report.wire.note = prepared.disclosure_note.clone();
    let Some(p) = provider else {
        return abstain(ChoiceAbstain::NoProvider, report);
    };
    report.provider_id = Some(p.profile.provider_id.clone());
    report.provider_status = Some(p.status(CHOICE_TASK));
    match consult(inputs, ctx, p, &prepared, &mut report) {
        Ok(selection) => ChoiceOutcome::Selected {
            selection: ChoiceSelection {
                option: scr
                    .admitted
                    .iter()
                    .find(|o| o.id == selection.0)
                    .cloned()
                    .expect("mapped selection is admitted"),
                source: ChoiceSource::Provider,
                scores: selection.1,
            },
            report,
        },
        Err(reason) => abstain(reason, report),
    }
}

type Picked = (String, Option<(ScoreKind, f64)>);

fn consult<I: ?Sized + DecisionInvoker>(
    inputs: &ChoiceInputs,
    ctx: &RouteContext,
    p: &mut ConfiguredProvider<'_, I>,
    pr: &PreparedChoice,
    report: &mut ChoiceReport,
) -> Result<Picked, ChoiceAbstain> {
    use ChoiceAbstain as A;
    // Negotiated capability, never inferred from a name.
    if !p.invoker.decision_versions().contains(&CHOICE_WIRE_VERSION) {
        report.wire.note = Some("adapter did not negotiate choice-select/v1".into());
        return Err(A::UnsupportedAdapter);
    }
    // Qualification is per task: a model-route gate never enables this.
    if !p.enabled(CHOICE_TASK) {
        return Err(A::NotQualified);
    }
    let pid = p.profile.provider_id.clone();
    if ctx.router_lineage.contains(&pid) {
        return Err(A::RecursionBlocked);
    }
    if let Err(why) = p.profile.admits_request(
        pr.selection.len(),
        pr.rendered.state.len(),
        &[Modality::Text].into(),
    ) {
        report.wire.note = Some(format!("provider capability preflight refused: {why}"));
        return Err(A::ProfileLimits);
    }
    if inputs
        .question
        .remaining_budget_micros
        .is_some_and(|left| left < inputs.policy.router_reserve_micros)
    {
        return Err(A::BudgetExhausted);
    }
    let remaining = inputs
        .policy
        .max_router_calls
        .saturating_sub(ctx.router_calls_used);
    if remaining == 0 {
        return Err(A::CallCapExhausted);
    }
    let latency_left = inputs
        .policy
        .max_router_latency_ms
        .saturating_sub(ctx.router_ms_used);
    if latency_left == 0 {
        return Err(A::LatencyExhausted);
    }
    let mut lineage = ctx.router_lineage.clone();
    lineage.push(pid);
    let env = DecisionRequest {
        invocation_id: ctx.invocation_id.clone(),
        project: ctx.project.clone(),
        lock_digest: ctx.lock_digest.clone(),
        version: CHOICE_WIRE_VERSION,
        deadline_ms: latency_left.clamp(1, 600_000),
        max_result_bytes: 65_536,
        remaining_calls: remaining,
        lineage,
        payload: pr.payload.clone(),
    };
    env.validate().map_err(|_| A::InvalidResult)?;
    report.wire.version = CHOICE_WIRE_VERSION;
    report.router_calls += 1;
    let (result, elapsed, call) = match p.invoker.evaluate(&env) {
        DecisionCall::Unavailable => return Err(A::Unavailable),
        DecisionCall::Timeout => return Err(A::Timeout),
        DecisionCall::Answered {
            result,
            elapsed_ms,
            call,
        } => (result, elapsed_ms, call),
    };
    report.wire.call = call.clone();
    report.router_ms += elapsed;
    if elapsed > latency_left {
        return Err(A::Timeout);
    }
    wire::validate(Direction::Result, &result).map_err(|_| A::InvalidResult)?;
    check_against_request(&env.payload, &result).map_err(|e| {
        if e.code == "SPX-HPA043" {
            A::RejectedChoice
        } else {
            A::InvalidResult
        }
    })?;
    let r = ResultV2::from_json(&result).map_err(|_| A::InvalidResult)?;
    if call.as_ref().is_some_and(|c| *c != r.call) {
        return Err(A::InvalidResult);
    }
    report.wire.call = Some(r.call.clone());
    report.wire.score_kind = Some(r.score_kind);
    report.wire.native_confidence = r.native_confidence.zip(r.native_confidence_kind.clone());
    if !p.profile.admits_score_kind(r.score_kind) {
        return Err(A::InvalidResult);
    }
    let Some(sel) = r.choice.as_deref() else {
        report.wire.abstention = Some(r.abstention_reason);
        return Err(A::Native);
    };
    let id = pr.stable_id(sel).ok_or(A::RejectedChoice)?.to_string();
    let chosen = r.scores.as_ref().and_then(|s| s.get(sel).copied());
    let below = |min: f64| !chosen.is_some_and(|x| x >= min);
    if p.profile
        .min_option_mass
        .is_some_and(|m| r.score_kind == ScoreKind::OptionDistribution && below(m))
        || p.profile.min_confidence.is_some_and(below)
    {
        report.wire.abstention = Some(AbstentionReason::HostThreshold);
        return Err(A::HostThreshold);
    }
    if p.mode == ProviderMode::Auto {
        if let Err(why) = p.profile.verify_identity(&r.call) {
            report.wire.note = Some(format!("qualified choice not used: {why}"));
            return Err(A::IdentityMismatch);
        }
    }
    Ok((id, chosen.map(|x| (r.score_kind, x))))
}

#[cfg(test)]
#[path = "choice_tests.rs"]
mod tests;
