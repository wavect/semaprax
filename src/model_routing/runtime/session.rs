//! MR-10: route runtime turns and child-agent handoffs at safe lifecycle
//! boundaries. A session is a sequence of MR-09 routed invocations; a route
//! may change only at a [`RerouteBoundary`], after the previous turn settled
//! durably, the host accepted its state and the session journaled the
//! `continue` transition. Each turn is a new properly bound invocation over a
//! bounded [`Handoff`]; nothing reorders a previous binding or frozen plan.

mod handoff;
mod journal;

use std::collections::{BTreeMap, BTreeSet};

use serde_json::json;

pub use handoff::{Handoff, HANDOFF_SCHEMA, MAX_HANDOFF_REFS, MAX_HANDOFF_STATE_BYTES};
pub use journal::{RouteReason, SESSION_SCHEMA};

use super::error::RuntimeRoutingError;
use super::features::RuntimeFeatures;
use super::invoke::{
    bind_routed_invocation, resume_routed_invocation, run_routed_invocation, InvocationTarget,
    RoutedRun, RoutedRunHandlers,
};
use super::profiles::ApprovedProfileSet;
use super::select::route_among;
use journal::{Entry, MAX_ENTRIES};

use crate::agent_lifecycle::{CheckpointStore, LifecycleTask};
use crate::live_invocation::{
    CumulativeBudgetLedger, DurablePolicyRun, InvocationClock, ModelInvocationRequest,
};
use crate::model_routing::engine::json;
use crate::model_routing::engine::{
    Confidentiality, ConfiguredProvider, DecisionInvoker, RouteContext,
};

const MAX_SESSION_ID_BYTES: usize = 128;

/// An authorized specialist: a named profile a turn or child may use.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SpecialistGrant {
    pub id: String,
    pub profile: String,
    /// Whether child-agent work may be delegated to it.
    pub delegable: bool,
}

/// A bounded, configured escalation rule keyed on host-observed progress,
/// never on model self-confidence.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct EscalationRule {
    pub after_no_progress_turns: u32,
    pub escalate_to: BTreeSet<String>,
    pub max_escalations: u32,
}

/// Deployment-authorized routing allowlists for one session.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SessionPolicy {
    /// Declared role/task profile -> profile ids it may route among.
    pub role_profiles: BTreeMap<String, BTreeSet<String>>,
    pub specialists: Vec<SpecialistGrant>,
    pub escalation: Option<EscalationRule>,
    pub max_delegation_depth: u32,
    pub max_turns: u32,
    /// Session confidentiality; no turn or handoff may lower it.
    pub confidentiality: Confidentiality,
}

impl SessionPolicy {
    fn digest(&self) -> String {
        json::digest(
            "semaprax.runtime-session-policy.v1",
            &json!({
                "roles": self.role_profiles,
                "specialists": self.specialists.iter().map(|s| json!([s.id, s.profile, s.delegable])).collect::<Vec<_>>(),
                "escalation": self.escalation.as_ref().map(|e| json!([e.after_no_progress_turns, e.escalate_to, e.max_escalations])),
                "max_delegation_depth": self.max_delegation_depth,
                "max_turns": self.max_turns,
                "confidentiality": self.confidentiality.as_str(),
            }),
        )
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ProgressCounters {
    pub turns_completed: u32,
    pub no_progress_turns: u32,
    pub escalations: u32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LastOutcome {
    None,
    Accepted,
    AcceptedWithoutProgress,
}

/// Structured turn features, derived from the session journal plus the
/// host's declared role and next-stage needs. Recorded by digest in the
/// handoff.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TurnFeatures {
    pub role: String,
    pub last_outcome: LastOutcome,
    pub remaining_parent_budget: i64,
    pub deadline_millis: Option<i64>,
    pub next_stage_capabilities: Vec<String>,
    pub progress: ProgressCounters,
}

impl TurnFeatures {
    pub fn digest(&self) -> String {
        json::digest(
            "semaprax.runtime-turn-features.v1",
            &json!({
                "role": self.role,
                "last_outcome": format!("{:?}", self.last_outcome),
                "remaining_parent_budget": self.remaining_parent_budget,
                "deadline_millis": self.deadline_millis,
                "next_stage_capabilities": self.next_stage_capabilities,
                "progress": [self.progress.turns_completed, self.progress.no_progress_turns, self.progress.escalations],
            }),
        )
    }
}

/// The only point a route may change: before turn `turn`, after the previous
/// turn's durable accepted `continue` terminal (or before the first turn).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RerouteBoundary {
    pub turn: u32,
    pub previous_profile: Option<String>,
}

/// The host's request for the next turn.
#[derive(Clone, Debug)]
pub struct TurnRequest {
    pub role: String,
    pub specialist: Option<String>,
    pub next_stage_capabilities: Vec<String>,
    /// Base routing features; confidentiality must equal the session's and
    /// any operator pin is replaced by the session's own allowlist rules.
    pub features: RuntimeFeatures,
    /// Units reserved from the parent allowance before dispatch.
    pub reservation: i64,
}

/// The host's verdict on a settled response. Effects (tool calls) belong to
/// the host here and are journaled; a replayed turn never calls this again.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum TurnVerdict {
    Accepted {
        committed_state: Vec<u8>,
        progressed: bool,
        complete: bool,
        tool_results: Vec<String>,
    },
    Rejected(String),
    /// An external effect timed out with unknown status: reconciliation only.
    EffectUncertain,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TurnStatus {
    Continue,
    Complete,
    Rejected,
    Uncertain,
    Failed,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TurnOutcome {
    pub turn: u32,
    pub profile: String,
    pub reason: RouteReason,
    pub router_calls: u32,
    /// The turn's boundary was already complete in the journal: no route,
    /// model or effect call was made.
    pub replayed: bool,
    pub status: TurnStatus,
    pub run: Option<RoutedRun>,
}

/// A child-agent delegation request.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DelegationRequest {
    pub child: String,
    pub specialist: String,
    pub amount: i64,
}

/// A reserved child allowance. The child session is opened from it and can
/// never exceed it, widen capabilities or re-enter a router on its lineage.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ChildGrant {
    pub child: String,
    pub caller: String,
    pub profile: String,
    pub depth: u32,
    pub max_depth: u32,
    pub allowance: i64,
    /// The enclosing session's absolute deadline (clock units of the
    /// session). A child inherits it and can never broaden it.
    pub deadline: Option<i64>,
    pub router_lineage: Vec<String>,
}

/// A fixed instant for the existing cumulative ledger.
struct At(i64);
impl InvocationClock for At {
    fn now_millis(&self) -> i64 {
        self.0
    }
}

pub struct RoutedSession<'a> {
    set: &'a ApprovedProfileSet,
    policy: SessionPolicy,
    entries: Vec<Entry>,
    store: &'a mut dyn CheckpointStore,
    generation: u64,
}

fn refuse<T>(why: impl Into<String>) -> Result<T, RuntimeRoutingError> {
    Err(RuntimeRoutingError::session(why))
}

impl<'a> RoutedSession<'a> {
    /// Opens a root session and commits its first journal generation.
    #[allow(clippy::too_many_arguments)]
    pub fn open(
        set: &'a ApprovedProfileSet,
        policy: SessionPolicy,
        session: &str,
        instructions_digest: &str,
        acceptance_digest: &str,
        ceiling: i64,
        deadline_millis: Option<i64>,
        store: &'a mut dyn CheckpointStore,
    ) -> Result<Self, RuntimeRoutingError> {
        Self::open_inner(
            set,
            policy,
            session,
            None,
            0,
            None,
            [instructions_digest, acceptance_digest],
            ceiling,
            deadline_millis,
            Vec::new(),
            None,
            store,
        )
    }

    /// Opens a child session from a reserved grant. Its allowance, depth,
    /// router lineage and single authorized profile come from the grant.
    pub fn open_child(
        set: &'a ApprovedProfileSet,
        policy: SessionPolicy,
        grant: &ChildGrant,
        instructions_digest: &str,
        acceptance_digest: &str,
        store: &'a mut dyn CheckpointStore,
    ) -> Result<Self, RuntimeRoutingError> {
        Self::open_inner(
            set,
            policy,
            &grant.child,
            Some(grant.caller.clone()),
            grant.depth,
            Some(grant.max_depth),
            [instructions_digest, acceptance_digest],
            grant.allowance,
            grant.deadline,
            grant.router_lineage.clone(),
            Some(vec![grant.profile.clone()]),
            store,
        )
    }

    #[allow(clippy::too_many_arguments)]
    fn open_inner(
        set: &'a ApprovedProfileSet,
        policy: SessionPolicy,
        session: &str,
        caller: Option<String>,
        depth: u32,
        max_depth: Option<u32>,
        digests: [&str; 2],
        ceiling: i64,
        deadline: Option<i64>,
        lineage: Vec<String>,
        allowed: Option<Vec<String>>,
        store: &'a mut dyn CheckpointStore,
    ) -> Result<Self, RuntimeRoutingError> {
        if session.is_empty() || session.len() > MAX_SESSION_ID_BYTES || !session.is_ascii() {
            return refuse("session id must be bounded ASCII");
        }
        if ceiling < 0 {
            return refuse("negative session allowance");
        }
        let max_depth = max_depth.map_or(policy.max_delegation_depth, |m| {
            m.min(policy.max_delegation_depth)
        });
        let opened = Entry::Opened {
            session: session.to_owned(),
            caller,
            depth,
            max_depth,
            ceiling,
            deadline,
            profile_set: set.digest().to_owned(),
            policy: policy.digest(),
            instructions: digests[0].to_owned(),
            acceptance: digests[1].to_owned(),
            lineage,
            allowed,
        };
        let mut s = Self {
            set,
            policy,
            entries: Vec::new(),
            store,
            generation: 0,
        };
        s.append(opened)?;
        Ok(s)
    }

    /// Resumes from the latest committed session journal. Completed turn
    /// boundaries replay from it; the current policy and approved set govern
    /// only work that has not started (revocation stops it, never rewrites).
    pub fn resume(
        set: &'a ApprovedProfileSet,
        policy: SessionPolicy,
        document: &str,
        generation: u64,
        store: &'a mut dyn CheckpointStore,
    ) -> Result<Self, RuntimeRoutingError> {
        Ok(Self {
            set,
            policy,
            entries: journal::parse(document)?,
            store,
            generation,
        })
    }

    /// The canonical session journal bytes (also what each commit writes).
    pub fn journal(&self) -> String {
        journal::render(&self.entries)
    }
    pub fn generation(&self) -> u64 {
        self.generation
    }

    fn append(&mut self, entry: Entry) -> Result<(), RuntimeRoutingError> {
        if self.entries.len() >= MAX_ENTRIES {
            return refuse("route session journal is full");
        }
        self.entries.push(entry);
        let next = self.generation + 1;
        if self
            .store
            .commit(next, &journal::render(&self.entries))
            .is_err()
        {
            self.entries.pop();
            return Err(RuntimeRoutingError::Checkpoint);
        }
        self.generation = next;
        Ok(())
    }

    fn opened(&self) -> &Entry {
        &self.entries[0]
    }
    fn session_id(&self) -> &str {
        match self.opened() {
            Entry::Opened { session, .. } => session,
            _ => unreachable!("journal starts with opened"),
        }
    }

    /// Nonrefundable spend so far: every turn and child reservation.
    pub fn committed(&self) -> i64 {
        self.entries
            .iter()
            .map(|e| match e {
                Entry::Routed { reserved, .. } => *reserved,
                Entry::ChildReserved { amount, .. } => *amount,
                _ => 0,
            })
            .fold(0i64, i64::saturating_add)
    }

    /// Reserves `amount` through the existing cumulative ledger, seeded with
    /// everything already committed: ceiling, deadline and nonrefundability
    /// are the ledger's own rules.
    fn reserve(&self, amount: i64, now: i64) -> Result<(), RuntimeRoutingError> {
        let Entry::Opened {
            ceiling, deadline, ..
        } = self.opened()
        else {
            unreachable!()
        };
        let mut at = At(now);
        let mut ledger =
            CumulativeBudgetLedger::migrated(*ceiling, *deadline, self.committed(), &mut at);
        let probe = ModelInvocationRequest {
            turn: 0,
            task: Vec::new(),
            observation: Vec::new(),
            proposal_grammar_digest: String::new(),
            deployment_binding: String::new(),
            max_response_bytes: 0,
            effective_budget: amount,
        };
        crate::live_invocation::InvocationBudgetHook::reserve(&mut ledger, &probe)
            .map(|_| ())
            .map_err(|refusal| {
                RuntimeRoutingError::session(format!("parent allowance: {}", refusal.0))
            })
    }

    /// Admits a turn at the live instant: cancellation, deadline and parent
    /// allowance, through the existing cumulative ledger.
    fn admit(
        &self,
        amount: i64,
        handlers: &RoutedRunHandlers<'_>,
    ) -> Result<(), RuntimeRoutingError> {
        if handlers.cancellation.is_cancelled() {
            return refuse("turn was cancelled before dispatch");
        }
        self.reserve(amount, handlers.clock.now_millis())
    }

    /// Whether `turn` has an effect intent but no terminal entry yet.
    fn effect_intent_open(&self, turn: u32) -> bool {
        let mut open = false;
        for e in &self.entries {
            match e {
                Entry::EffectIntent { turn: t } if *t == turn => open = true,
                Entry::Settled { turn: t, .. }
                | Entry::Unaccepted { turn: t, .. }
                | Entry::Uncertain { turn: t, .. }
                | Entry::Failed { turn: t, .. }
                    if *t == turn =>
                {
                    open = false
                }
                _ => {}
            }
        }
        open
    }

    fn halted(&self) -> Option<String> {
        self.entries.iter().rev().find_map(|e| match e {
            Entry::Unaccepted { turn, .. } => Some(format!("turn {turn} was not accepted")),
            Entry::Uncertain { turn, kind } => Some(format!(
                "turn {turn} has an uncertain {kind}; reconcile it, do not re-route"
            )),
            Entry::Failed { turn, .. } => Some(format!("turn {turn} failed")),
            Entry::Settled { complete: true, .. } => Some("session is complete".into()),
            _ => None,
        })
    }

    fn settled(&self) -> Vec<(u32, &Vec<u8>, bool, &Vec<String>, &String)> {
        self.entries
            .iter()
            .filter_map(|e| match e {
                Entry::Settled {
                    turn,
                    state,
                    progressed,
                    tool_results,
                    response,
                    ..
                } => Some((*turn, state, *progressed, tool_results, response)),
                _ => None,
            })
            .collect()
    }

    fn routed(&self, turn: u32) -> Option<&Entry> {
        self.entries
            .iter()
            .find(|e| matches!(e, Entry::Routed { turn: t, .. } if *t == turn))
    }

    fn progress(&self) -> ProgressCounters {
        let mut p = ProgressCounters::default();
        for e in &self.entries {
            match e {
                Entry::Settled { progressed, .. } => {
                    p.turns_completed += 1;
                    p.no_progress_turns = if *progressed {
                        0
                    } else {
                        p.no_progress_turns + 1
                    };
                }
                Entry::Routed {
                    reason: RouteReason::Escalation,
                    ..
                } => p.escalations += 1,
                _ => {}
            }
        }
        p
    }

    fn last_profile(&self) -> Option<String> {
        self.entries.iter().rev().find_map(|e| match e {
            Entry::Routed { to, .. } => Some(to.clone()),
            _ => None,
        })
    }

    /// The next re-route boundary, if one exists: before the first turn, or
    /// after a durable accepted `continue` terminal. `None` while a turn is
    /// in flight, after an uncertain/failed/rejected turn, or when complete.
    pub fn boundary(&self) -> Option<RerouteBoundary> {
        if self.halted().is_some() {
            return None;
        }
        let turn = self.settled().len() as u32;
        if self.routed(turn).is_some() {
            return None;
        }
        Some(RerouteBoundary {
            turn,
            previous_profile: self.last_profile(),
        })
    }

    fn turn_features(&self, request: &TurnRequest) -> TurnFeatures {
        let Entry::Opened {
            ceiling, deadline, ..
        } = self.opened()
        else {
            unreachable!()
        };
        let progress = self.progress();
        let last_outcome = match self.settled().last() {
            None => LastOutcome::None,
            Some((_, _, true, _, _)) => LastOutcome::Accepted,
            Some(_) => LastOutcome::AcceptedWithoutProgress,
        };
        TurnFeatures {
            role: request.role.clone(),
            last_outcome,
            remaining_parent_budget: ceiling.saturating_sub(self.committed()).max(0),
            deadline_millis: *deadline,
            next_stage_capabilities: request.next_stage_capabilities.clone(),
            progress,
        }
    }

    fn handoff(
        &self,
        turn: u32,
        from: Option<String>,
        to: &str,
        features: &TurnFeatures,
    ) -> Handoff {
        let Entry::Opened {
            instructions,
            acceptance,
            ..
        } = self.opened()
        else {
            unreachable!()
        };
        let settled = self.settled();
        let state = settled
            .last()
            .map(|(_, s, ..)| (*s).clone())
            .unwrap_or_default();
        let outputs: Vec<String> = settled
            .iter()
            .rev()
            .take(MAX_HANDOFF_REFS)
            .rev()
            .map(|(.., r)| (*r).clone())
            .collect();
        let tools: Vec<String> = settled
            .iter()
            .flat_map(|(_, _, _, t, _)| t.iter().cloned())
            .rev()
            .take(MAX_HANDOFF_REFS)
            .collect::<Vec<_>>()
            .into_iter()
            .rev()
            .collect();
        let caps = self
            .set
            .profile(to)
            .map(|p| p.deployment().granted_capabilities().to_vec())
            .unwrap_or_default();
        Handoff {
            session: self.session_id().to_owned(),
            turn,
            from_profile: from,
            to_profile: to.to_owned(),
            confidentiality: self.policy.confidentiality,
            instructions_digest: instructions.clone(),
            acceptance_digest: acceptance.clone(),
            tool_capabilities: caps,
            turn_features_digest: features.digest(),
            committed_state: state,
            accepted_outputs: outputs,
            tool_results: tools,
        }
    }

    fn lineage(&self) -> Vec<String> {
        let Entry::Opened { lineage, .. } = self.opened() else {
            unreachable!()
        };
        let mut all = lineage.clone();
        for e in &self.entries {
            if let Entry::Routed { record, .. } = e {
                if record.router_calls() > 0 && !all.iter().any(|x| x == record.decision_provider())
                {
                    all.push(record.decision_provider().to_owned());
                }
            }
        }
        all
    }

    /// Which profiles this turn may route among, and why.
    fn allowlist(
        &self,
        request: &TurnRequest,
    ) -> Result<(BTreeSet<String>, RouteReason), RuntimeRoutingError> {
        let Entry::Opened { allowed, .. } = self.opened() else {
            unreachable!()
        };
        let mut set: BTreeSet<String> = match &request.specialist {
            Some(id) => {
                let grant = self
                    .policy
                    .specialists
                    .iter()
                    .find(|s| s.id == *id)
                    .ok_or_else(|| {
                        RuntimeRoutingError::session(format!("specialist `{id}` is not authorized"))
                    })?;
                [grant.profile.clone()].into()
            }
            None => self
                .policy
                .role_profiles
                .get(&request.role)
                .cloned()
                .ok_or_else(|| {
                    RuntimeRoutingError::session(format!(
                        "role `{}` is not authorized",
                        request.role
                    ))
                })?,
        };
        if let Some(only) = allowed {
            set.retain(|p| only.contains(p));
        }
        set.retain(|id| {
            self.set.profile(id).is_some_and(|p| {
                request
                    .next_stage_capabilities
                    .iter()
                    .all(|c| p.deployment().granted_capabilities().contains(c))
            })
        });
        let mut reason = if request.specialist.is_some() {
            RouteReason::Specialist
        } else if self.settled().is_empty() {
            RouteReason::Initial
        } else {
            RouteReason::Reroute
        };
        let progress = self.progress();
        if let Some(rule) = &self.policy.escalation {
            if request.specialist.is_none()
                && progress.turns_completed > 0
                && progress.no_progress_turns >= rule.after_no_progress_turns
            {
                if progress.escalations >= rule.max_escalations {
                    return refuse("no-progress escalation budget is exhausted");
                }
                set.retain(|p| rule.escalate_to.contains(p));
                reason = RouteReason::Escalation;
            }
        }
        if set.is_empty() {
            return refuse("no deployment-authorized profile covers this turn");
        }
        Ok((set, reason))
    }

    /// Runs (or replays) the next turn. A completed boundary replays from
    /// the journal with no route, model or effect call; an in-flight turn
    /// resumes its recorded route with zero router calls; a new turn routes
    /// only at a [`RerouteBoundary`].
    #[allow(clippy::too_many_arguments)]
    pub fn run_turn<I: ?Sized + DecisionInvoker>(
        &mut self,
        request: &TurnRequest,
        ctx: &RouteContext,
        provider: Option<&mut ConfiguredProvider<'_, I>>,
        target: &InvocationTarget<'_>,
        handlers: RoutedRunHandlers<'_>,
        turn_store: &mut dyn CheckpointStore,
        recovered: Option<(&str, u64)>,
        accept: &mut dyn FnMut(&[u8]) -> TurnVerdict,
    ) -> Result<TurnOutcome, RuntimeRoutingError> {
        if let Some(why) = self.halted() {
            return refuse(why);
        }
        let turn = self.settled().len() as u32;
        let (bound, reason, router_calls) = if let Some(Entry::Routed {
            reason,
            to,
            record,
            handoff,
            reserved,
            ..
        }) = self.routed(turn).cloned()
        {
            // An acceptance callback that may already have run is never
            // re-entered: record the effect as uncertain, make no model call.
            if self.effect_intent_open(turn) {
                self.append(Entry::Uncertain {
                    turn,
                    kind: "effect".into(),
                })?;
                return Ok(TurnOutcome {
                    turn,
                    profile: to,
                    reason,
                    router_calls: 0,
                    replayed: false,
                    status: TurnStatus::Uncertain,
                    run: None,
                });
            }
            // In flight: reuse the recorded route and the retained task bytes.
            let mut target = target.clone();
            let bytes = self.retained_handoff(turn, &to, &handoff)?;
            target.task = LifecycleTask {
                objective: bytes,
                budget: reserved,
            };
            target.effective_budget = reserved;
            target.turn = turn;
            let run = match recovered {
                Some((doc, generation)) => resume_routed_invocation(
                    self.set, doc, generation, &target, handlers, turn_store,
                )?,
                None => {
                    let bound = bind_routed_invocation(self.set, record, &target)?;
                    run_routed_invocation(&bound, target.schema, handlers, turn_store)?
                }
            };
            (run, reason, 0)
        } else {
            let Some(boundary) = self.boundary() else {
                return refuse("no re-route boundary is open");
            };
            if turn >= self.policy.max_turns {
                return refuse("session turn limit reached");
            }
            if request.features.confidentiality != self.policy.confidentiality {
                return refuse("a turn may not change the session confidentiality");
            }
            if request.reservation < 0 {
                return refuse("negative turn reservation");
            }
            // Only routers inherited from callers count: a session may keep
            // using its own router across its turns, but a child may never
            // re-enter a router already on its caller's decision lineage.
            let Entry::Opened {
                lineage: inherited, ..
            } = self.opened()
            else {
                unreachable!()
            };
            let lineage = inherited.clone();
            if let Some(p) = provider.as_ref() {
                if lineage.iter().any(|id| *id == p.profile.provider_id) {
                    return refuse(format!(
                        "router recursion: `{}` is already on this decision lineage",
                        p.profile.provider_id
                    ));
                }
            }
            let (allowed, reason) = self.allowlist(request)?;
            // Live stop conditions are checked before any provider dispatch.
            self.admit(request.reservation, &handlers)?;
            let mut features = request.features.clone();
            if let Entry::Opened {
                deadline: Some(deadline),
                ..
            } = self.opened()
            {
                let left = u64::try_from(deadline.saturating_sub(handlers.clock.now_millis()))
                    .unwrap_or(0);
                features.remaining_latency_ms = features.remaining_latency_ms.min(left);
            }
            features.operator_pin = request
                .specialist
                .as_ref()
                .and_then(|_| allowed.iter().next().cloned());
            let mut ctx = ctx.clone();
            ctx.lineage_id = format!("{}/turn/{turn}", self.session_id());
            ctx.router_lineage = lineage;
            let routed = route_among(self.set, Some(&allowed), &features, &ctx, provider)?;
            let to = routed.profile().id().to_owned();
            let record = routed.into_record();
            // Routing may have consumed the remaining time: re-read the live
            // clock before admitting generation, on every route path. Router
            // work that happened is retained in the journal.
            if let Err(refusal) = self.admit(request.reservation, &handlers) {
                self.append(Entry::Failed {
                    turn,
                    why: format!(
                        "refused after routing ({} router call(s) via `{}`): {refusal}",
                        record.router_calls(),
                        record.decision_provider()
                    ),
                })?;
                return Err(refusal);
            }
            let turn_features = self.turn_features(request);
            let handoff =
                self.handoff(turn, boundary.previous_profile.clone(), &to, &turn_features);
            let bytes = handoff.bytes();
            let mut target = target.clone();
            target.task = LifecycleTask {
                objective: bytes,
                budget: request.reservation,
            };
            target.effective_budget = request.reservation;
            target.turn = turn;
            let bound = bind_routed_invocation(self.set, record.clone(), &target)?;
            let calls = record.router_calls();
            self.append(Entry::Routed {
                turn,
                reason,
                from: boundary.previous_profile,
                to,
                record,
                handoff: json::canonical(&handoff.to_json()),
                reserved: request.reservation,
            })?;
            let run = run_routed_invocation(&bound, target.schema, handlers, turn_store)?;
            (run, reason, calls)
        };
        let profile = bound.record.profile().to_owned();
        let status = match &bound.run {
            DurablePolicyRun::Settled(bytes) | DurablePolicyRun::Replayed(bytes) => {
                // Persist the effect boundary first: from here on a lost
                // terminal write can never authorize a second callback.
                self.append(Entry::EffectIntent { turn })?;
                match accept(bytes) {
                    TurnVerdict::Accepted {
                        committed_state,
                        progressed,
                        complete,
                        tool_results,
                    } => {
                        if committed_state.len() > MAX_HANDOFF_STATE_BYTES
                            || tool_results.len() > MAX_HANDOFF_REFS
                        {
                            self.append(Entry::Unaccepted {
                                turn,
                                why: "committed state exceeds the handoff bound".into(),
                            })?;
                            TurnStatus::Rejected
                        } else {
                            self.append(Entry::Settled {
                                turn,
                                response: json::sha256_labeled(
                                    "semaprax.runtime-turn-response.v1",
                                    bytes,
                                ),
                                state: committed_state,
                                progressed,
                                complete,
                                tool_results,
                            })?;
                            if complete {
                                TurnStatus::Complete
                            } else {
                                TurnStatus::Continue
                            }
                        }
                    }
                    TurnVerdict::Rejected(why) => {
                        self.append(Entry::Unaccepted { turn, why })?;
                        TurnStatus::Rejected
                    }
                    TurnVerdict::EffectUncertain => {
                        self.append(Entry::Uncertain {
                            turn,
                            kind: "effect".into(),
                        })?;
                        TurnStatus::Uncertain
                    }
                }
            }
            DurablePolicyRun::Uncertain => {
                self.append(Entry::Uncertain {
                    turn,
                    kind: "model_dispatch".into(),
                })?;
                TurnStatus::Uncertain
            }
            DurablePolicyRun::Refused(why) => {
                self.append(Entry::Failed {
                    turn,
                    why: format!("{why:?}"),
                })?;
                TurnStatus::Failed
            }
        };
        Ok(TurnOutcome {
            turn,
            profile,
            reason,
            router_calls,
            replayed: false,
            status,
            run: Some(bound),
        })
    }

    /// The exact retained handoff bytes for an in-flight turn.
    fn retained_handoff(
        &self,
        turn: u32,
        to: &str,
        retained: &str,
    ) -> Result<Vec<u8>, RuntimeRoutingError> {
        let value: serde_json::Value = serde_json::from_str(retained)
            .map_err(|_| RuntimeRoutingError::RecordMismatch("handoff JSON".into()))?;
        if value["schema"] != HANDOFF_SCHEMA || value["turn"] != turn || value["to_profile"] != to {
            return Err(RuntimeRoutingError::RecordMismatch(
                "retained handoff".into(),
            ));
        }
        Ok(retained.as_bytes().to_vec())
    }

    /// Replays a completed turn boundary from the journal: no route, model or
    /// effect call. `None` when `turn` has not completed.
    pub fn replay_turn(&self, turn: u32) -> Option<TurnOutcome> {
        let Entry::Routed { reason, to, .. } = self.routed(turn)? else {
            return None;
        };
        let status = self.entries.iter().find_map(|e| match e {
            Entry::Settled {
                turn: t, complete, ..
            } if *t == turn => Some(if *complete {
                TurnStatus::Complete
            } else {
                TurnStatus::Continue
            }),
            Entry::Unaccepted { turn: t, .. } if *t == turn => Some(TurnStatus::Rejected),
            Entry::Uncertain { turn: t, .. } if *t == turn => Some(TurnStatus::Uncertain),
            Entry::Failed { turn: t, .. } if *t == turn => Some(TurnStatus::Failed),
            _ => None,
        })?;
        Some(TurnOutcome {
            turn,
            profile: to.clone(),
            reason: *reason,
            router_calls: 0,
            replayed: true,
            status,
            run: None,
        })
    }

    /// The handoff digest journaled for `turn`.
    pub fn handoff_digest(&self, turn: u32) -> Option<String> {
        match self.routed(turn)? {
            Entry::Routed { handoff, .. } => Some(json::sha256_labeled(
                "semaprax.runtime-handoff.digest.v1",
                handoff.as_bytes(),
            )),
            _ => None,
        }
    }

    /// Reserves child-agent work from this session's allowance before
    /// dispatch. Depth, specialist authorization and capability containment
    /// are checked first; the reservation is journaled and nonrefundable.
    pub fn delegate(
        &mut self,
        request: &DelegationRequest,
        now_millis: i64,
    ) -> Result<ChildGrant, RuntimeRoutingError> {
        if let Some(why) = self.halted() {
            return refuse(why);
        }
        let Entry::Opened {
            depth,
            max_depth,
            deadline,
            ..
        } = self.opened().clone()
        else {
            unreachable!()
        };
        if depth >= max_depth {
            return refuse(format!(
                "delegation depth {} exceeds the bound {max_depth}",
                depth + 1
            ));
        }
        if request.child.is_empty()
            || request.child.len() > MAX_SESSION_ID_BYTES
            || !request.child.is_ascii()
            || self
                .entries
                .iter()
                .any(|e| matches!(e, Entry::ChildReserved { child, .. } if *child == request.child))
        {
            return refuse("child id must be bounded, ASCII and unique");
        }
        if request.amount <= 0 {
            return refuse("a child reservation must be positive");
        }
        let grant = self
            .policy
            .specialists
            .iter()
            .find(|s| s.id == request.specialist && s.delegable)
            .cloned()
            .ok_or_else(|| {
                RuntimeRoutingError::session(format!(
                    "specialist `{}` is not authorized for delegation",
                    request.specialist
                ))
            })?;
        let callee = self
            .set
            .profile(&grant.profile)
            .ok_or_else(|| RuntimeRoutingError::session("specialist profile is not approved"))?;
        if let Some(current) = self.last_profile().and_then(|id| self.set.profile(&id)) {
            let have = current.deployment().granted_capabilities();
            if callee
                .deployment()
                .granted_capabilities()
                .iter()
                .any(|c| !have.contains(c))
            {
                return refuse("a child may not widen the caller's tools or effects");
            }
        }
        self.reserve(request.amount, now_millis)?;
        self.append(Entry::ChildReserved {
            child: request.child.clone(),
            specialist: grant.id.clone(),
            profile: grant.profile.clone(),
            depth: depth + 1,
            amount: request.amount,
        })?;
        Ok(ChildGrant {
            child: request.child.clone(),
            caller: self.session_id().to_owned(),
            profile: grant.profile,
            depth: depth + 1,
            max_depth,
            allowance: request.amount,
            deadline,
            router_lineage: self.lineage(),
        })
    }

    /// Reconciles a child's completion: spend never exceeds the reservation
    /// and nothing is refunded.
    pub fn settle_child(&mut self, child: &str, spent: i64) -> Result<(), RuntimeRoutingError> {
        let amount = self
            .entries
            .iter()
            .find_map(|e| match e {
                Entry::ChildReserved {
                    child: c, amount, ..
                } if c == child => Some(*amount),
                _ => None,
            })
            .ok_or_else(|| RuntimeRoutingError::session("unknown child"))?;
        if self
            .entries
            .iter()
            .any(|e| matches!(e, Entry::ChildSettled { child: c, .. } if c == child))
        {
            return refuse("child already settled");
        }
        if !(0..=amount).contains(&spent) {
            return refuse("child spend exceeds its reservation");
        }
        self.append(Entry::ChildSettled {
            child: child.to_owned(),
            spent,
        })
    }
}
