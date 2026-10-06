//! DV-02, DV-07 and DV-08: the runtime session's effect boundary, live stop
//! guards and child deadline inheritance, by deterministic fault injection.

use std::cell::Cell;
use std::rc::Rc;

use super::runtime_reroute::{accepted, policy, turn};
use super::runtime_routing::{
    choose, ctx, response, router, settling_factory, with_world, Created, MemStore, ScriptedRouter,
    World,
};
use semaprax::agent_lifecycle::{CheckpointStore, CheckpointStoreError};
use semaprax::agent_runtime::AgentCancellation;
use semaprax::live_invocation::fixture::StepClock;
use semaprax::live_invocation::InvocationClock;
use semaprax::model_budget_policy::retry::ConservativeFailureClassifier;
use semaprax::model_budget_policy::NoDelayBackoff;
use semaprax::model_routing::engine::{
    DecisionCall, DecisionInvoker, DecisionRequest, ProviderMode, TaskFamily,
};
use semaprax::model_routing::runtime::{
    ApprovedProfileSet, DelegationRequest, RoutedRunHandlers, RoutedSession, RuntimeRoutingError,
    SessionPolicy, TurnOutcome, TurnStatus, TurnVerdict,
};
use semaprax::provider_adapter_sdk::AdapterInvocationCapability;

/// A store that atomically rejects the first document containing `needle`,
/// retaining the previous generation (the declared `CheckpointStore` contract).
struct FailStore {
    needle: &'static str,
    armed: bool,
    commits: Vec<(u64, String)>,
}

impl FailStore {
    fn new(needle: &'static str) -> Self {
        Self {
            needle,
            armed: true,
            commits: Vec::new(),
        }
    }
}

impl CheckpointStore for FailStore {
    fn commit(&mut self, generation: u64, document: &str) -> Result<(), CheckpointStoreError> {
        if self.armed && document.contains(self.needle) {
            self.armed = false;
            return Err(CheckpointStoreError);
        }
        self.commits.push((generation, document.to_owned()));
        Ok(())
    }
}

/// A live clock the router can advance.
#[derive(Clone, Default)]
struct LiveClock(Rc<Cell<i64>>);
impl InvocationClock for LiveClock {
    fn now_millis(&self) -> i64 {
        self.0.get()
    }
}

/// A router that moves the live clock and records the envelope deadline.
struct Router {
    clock: LiveClock,
    advance_to: i64,
    answer: Option<serde_json::Value>,
    elapsed_ms: u64,
    calls: u32,
    deadlines: Vec<u64>,
}

impl Router {
    fn new(clock: &LiveClock, advance_to: i64, answer: Option<serde_json::Value>) -> Self {
        Self {
            clock: clock.clone(),
            advance_to,
            answer,
            elapsed_ms: 2,
            calls: 0,
            deadlines: Vec::new(),
        }
    }
}

impl DecisionInvoker for Router {
    fn evaluate(&mut self, request: &DecisionRequest) -> DecisionCall {
        self.calls += 1;
        self.deadlines.push(request.deadline_ms);
        self.clock.0.set(self.advance_to);
        match &self.answer {
            Some(result) => DecisionCall::Answered {
                result: result.clone(),
                elapsed_ms: self.elapsed_ms,
                call: None,
            },
            None => DecisionCall::Unavailable,
        }
    }
}

fn handlers_at<'h>(
    clock: &'h dyn InvocationClock,
    cancel: &'h AgentCancellation,
    factory: &'h mut dyn semaprax::model_budget_policy::ProviderAdapterFactory,
    classifier: &'h mut ConservativeFailureClassifier,
    backoff: &'h mut NoDelayBackoff,
) -> RoutedRunHandlers<'h> {
    RoutedRunHandlers {
        clock,
        cancellation: cancel,
        capability: AdapterInvocationCapability::grant("guard fixture"),
        factory,
        classifier,
        backoff,
    }
}

fn open<'a>(
    set: &'a ApprovedProfileSet,
    policy: SessionPolicy,
    store: &'a mut dyn CheckpointStore,
    ceiling: i64,
    deadline: Option<i64>,
) -> RoutedSession<'a> {
    RoutedSession::open(
        set,
        policy,
        "session.main",
        "sha256:instructions",
        "sha256:acceptance",
        ceiling,
        deadline,
        store,
    )
    .unwrap()
}

fn session_err(r: Result<TurnOutcome, RuntimeRoutingError>, needle: &str) {
    match r {
        Err(RuntimeRoutingError::Session(m)) => assert!(m.contains(needle), "{m}"),
        other => panic!("expected `{needle}`, got {:?}", other.map(|o| o.status)),
    }
}

// ---------------------------------------------------------------- DV-02

/// Runs one turn whose session store fails per `needle`; returns the
/// effect/model counters and the retained session journal at the failure.
struct EffectWorld<'w> {
    w: &'w World,
    set: ApprovedProfileSet,
    created: Created,
    effects: Cell<u32>,
}

impl<'w> EffectWorld<'w> {
    fn new(w: &'w World) -> Self {
        Self {
            w,
            set: w.set(),
            created: Rc::default(),
            effects: Cell::new(0),
        }
    }

    fn run(
        &self,
        session: &mut RoutedSession<'_>,
        turn_store: &mut MemStore,
        recovered: Option<(&str, u64)>,
        verdict: &dyn Fn() -> TurnVerdict,
    ) -> Result<TurnOutcome, RuntimeRoutingError> {
        let clock = StepClock::new(0);
        let cancel = AgentCancellation::new();
        let (mut classifier, mut backoff) = (ConservativeFailureClassifier, NoDelayBackoff);
        let mut factory = settling_factory(
            &self.set,
            response(&self.w.schema, "1"),
            self.created.clone(),
        );
        let target = self.w.target(b"t");
        session.run_turn::<dyn DecisionInvoker>(
            &turn("simple", TaskFamily::LocalizedDebug, 2),
            &ctx("s"),
            None,
            &target,
            handlers_at(&clock, &cancel, &mut factory, &mut classifier, &mut backoff),
            turn_store,
            recovered,
            &mut |_| {
                self.effects.set(self.effects.get() + 1);
                verdict()
            },
        )
    }
}

#[test]
fn a_terminal_checkpoint_failure_after_an_effect_starts_never_repeats_the_effect() {
    with_world(|w| {
        let e = EffectWorld::new(w);
        let mut journal = FailStore::new("\"kind\":\"settled\"");
        let mut turn_store = MemStore::default();
        let (doc, generation);
        {
            let mut session = open(&e.set, policy(), &mut journal, 10, None);
            let first = e.run(&mut session, &mut turn_store, None, &|| {
                accepted(b"s0", false)
            });
            assert!(matches!(first, Err(RuntimeRoutingError::Checkpoint)));
            assert_eq!((e.created.borrow().len(), e.effects.get()), (1, 1));
            (doc, generation) = (session.journal(), session.generation());
            // Live retry with the retained model result: reconciliation, no effect.
            let (env, env_gen) = turn_store.latest();
            let retry = e
                .run(
                    &mut session,
                    &mut turn_store,
                    Some((&env, env_gen)),
                    &|| accepted(b"never", false),
                )
                .unwrap();
            assert_eq!(retry.status, TurnStatus::Uncertain);
            assert_eq!((e.created.borrow().len(), e.effects.get()), (1, 1));
            assert!(session.journal().contains("\"effect\":\"effect\""));
        }
        // Restart/resume from the durable journal at the failure: same result.
        let mut store = MemStore::default();
        let mut resumed =
            RoutedSession::resume(&e.set, policy(), &doc, generation, &mut store).unwrap();
        let (env, env_gen) = turn_store.latest();
        let again = e
            .run(
                &mut resumed,
                &mut turn_store,
                Some((&env, env_gen)),
                &|| accepted(b"never", false),
            )
            .unwrap();
        assert_eq!(again.status, TurnStatus::Uncertain);
        assert_eq!((e.created.borrow().len(), e.effects.get()), (1, 1));
    });
}

#[test]
fn a_failed_uncertain_commit_stays_unreconciled_and_cannot_redispatch() {
    with_world(|w| {
        let e = EffectWorld::new(w);
        let mut journal = FailStore::new("\"kind\":\"uncertain\"");
        let mut turn_store = MemStore::default();
        let mut session = open(&e.set, policy(), &mut journal, 10, None);
        let first = e.run(&mut session, &mut turn_store, None, &|| {
            TurnVerdict::EffectUncertain
        });
        assert!(matches!(first, Err(RuntimeRoutingError::Checkpoint)));
        assert_eq!(e.effects.get(), 1);
        let (env, env_gen) = turn_store.latest();
        let retry = e
            .run(
                &mut session,
                &mut turn_store,
                Some((&env, env_gen)),
                &|| accepted(b"never", false),
            )
            .unwrap();
        assert_eq!(retry.status, TurnStatus::Uncertain);
        assert_eq!((e.created.borrow().len(), e.effects.get()), (1, 1));
    });
}

#[test]
fn recovering_model_bytes_before_any_effect_intent_allows_exactly_one_callback() {
    with_world(|w| {
        let e = EffectWorld::new(w);
        let mut journal = FailStore::new("\"kind\":\"effect_intent\"");
        let mut turn_store = MemStore::default();
        let mut session = open(&e.set, policy(), &mut journal, 10, None);
        let first = e.run(&mut session, &mut turn_store, None, &|| {
            accepted(b"s0", false)
        });
        assert!(matches!(first, Err(RuntimeRoutingError::Checkpoint)));
        assert_eq!(e.effects.get(), 0, "the intent never became durable");
        let (env, env_gen) = turn_store.latest();
        let retry = e
            .run(
                &mut session,
                &mut turn_store,
                Some((&env, env_gen)),
                &|| accepted(b"s0", false),
            )
            .unwrap();
        assert_eq!(retry.status, TurnStatus::Continue);
        assert_eq!((e.created.borrow().len(), e.effects.get()), (1, 1));
        // A completed boundary replays with no router, model or effect call.
        let replay = session.replay_turn(0).unwrap();
        assert!(replay.replayed);
        assert_eq!((e.created.borrow().len(), e.effects.get()), (1, 1));
    });
}

// ---------------------------------------------------------------- DV-07

#[test]
fn exhausted_expired_and_cancelled_turns_make_zero_router_or_generator_calls() {
    with_world(|w| {
        let set = w.set();
        let created: Created = Rc::default();
        let mut factory = settling_factory(&set, response(&w.schema, "1"), created.clone());
        let (mut classifier, mut backoff) = (ConservativeFailureClassifier, NoDelayBackoff);
        let target = w.target(b"t");
        let mut inv = ScriptedRouter {
            calls: 0,
            answer: Some(choose("strong")),
        };
        // (ceiling, deadline, clock now, cancelled, expected refusal)
        let cases: [(i64, Option<i64>, i64, bool, &str); 3] = [
            (0, None, 0, false, "parent allowance"),
            (10, Some(5), 6, false, "deadline_exceeded"),
            (10, None, 0, true, "cancelled"),
        ];
        for (ceiling, deadline, now, cancelled, needle) in cases {
            let clock = StepClock::new(now);
            let live = AgentCancellation::new();
            if cancelled {
                live.cancel();
            }
            let mut journal = MemStore::default();
            let mut session = open(&set, policy(), &mut journal, ceiling, deadline);
            for _ in 0..2 {
                let result = session.run_turn(
                    &turn("hard", TaskFamily::LocalizedDebug, 1),
                    &ctx("s"),
                    Some(&mut router(&mut inv, ProviderMode::Explicit)),
                    &target,
                    handlers_at(&clock, &live, &mut factory, &mut classifier, &mut backoff),
                    &mut MemStore::default(),
                    None,
                    &mut |_| accepted(b"never", false),
                );
                session_err(result, needle);
            }
            assert_eq!(session.committed(), 0);
        }
        assert_eq!(inv.calls, 0, "no router call for a stopped turn");
        assert!(
            created.borrow().is_empty(),
            "no generation for a stopped turn"
        );
    });
}

#[test]
fn a_router_that_crosses_the_live_deadline_cannot_lead_to_generation() {
    for answer in [Some(choose("strong")), None] {
        with_world(|w| {
            let set = w.set();
            let created: Created = Rc::default();
            let mut factory = settling_factory(&set, response(&w.schema, "1"), created.clone());
            let (mut classifier, mut backoff) = (ConservativeFailureClassifier, NoDelayBackoff);
            let cancel = AgentCancellation::new();
            let target = w.target(b"t");
            let clock = LiveClock::default();
            // `None` makes the router unavailable: the rules fallback path.
            let mut inv = Router::new(&clock, 2_000, answer.clone());
            let mut journal = MemStore::default();
            let mut session = open(&set, policy(), &mut journal, 10, Some(1_000));
            let result = session.run_turn(
                &turn("hard", TaskFamily::LocalizedDebug, 1),
                &ctx("s"),
                Some(&mut semaprax::model_routing::engine::ConfiguredProvider {
                    profile: semaprax::model_routing::engine::ProviderProfile {
                        provider_id: "fixture-router".into(),
                        model_id: "router".into(),
                        checkpoint: "c1".into(),
                        ..Default::default()
                    },
                    invoker: &mut inv,
                    mode: ProviderMode::Explicit,
                    gate: semaprax::model_routing::engine::EnablementGate::not_evaluated(
                        "model-route/v1",
                        "fixture-router",
                    ),
                }),
                &target,
                handlers_at(&clock, &cancel, &mut factory, &mut classifier, &mut backoff),
                &mut MemStore::default(),
                None,
                &mut |_| accepted(b"never", false),
            );
            session_err(result, "deadline_exceeded");
            assert_eq!(inv.calls, 1);
            assert!(created.borrow().is_empty(), "no generation after expiry");
            assert_eq!(session.committed(), 0, "nothing was reserved");
            // The router work that happened is retained, and the refused turn
            // cannot be retried into more routing work.
            assert!(session
                .journal()
                .contains("refused after routing (1 router call(s)"));
            let again = session.run_turn::<dyn DecisionInvoker>(
                &turn("hard", TaskFamily::LocalizedDebug, 1),
                &ctx("s"),
                None,
                &target,
                handlers_at(&clock, &cancel, &mut factory, &mut classifier, &mut backoff),
                &mut MemStore::default(),
                None,
                &mut |_| accepted(b"never", false),
            );
            session_err(again, "failed");
            assert_eq!(inv.calls, 1);
        });
    }
}

#[test]
fn every_router_envelope_respects_the_enclosing_deadline() {
    // (session deadline, request remaining latency, expected envelope bound)
    for (deadline, remaining, bound) in [(None, 150, 150), (Some(300), 10_000, 300)] {
        with_world(|w| {
            let set = w.set();
            let created: Created = Rc::default();
            let mut factory = settling_factory(&set, response(&w.schema, "1"), created.clone());
            let (mut classifier, mut backoff) = (ConservativeFailureClassifier, NoDelayBackoff);
            let cancel = AgentCancellation::new();
            let target = w.target(b"t");
            let clock = LiveClock::default();
            // The router reports far more elapsed time than the bound.
            let mut inv = Router::new(&clock, 0, Some(choose("strong")));
            inv.elapsed_ms = 1_000;
            let mut journal = MemStore::default();
            let mut session = open(&set, policy(), &mut journal, 10, deadline);
            let mut request = turn("hard", TaskFamily::LocalizedDebug, 1);
            request.features.remaining_latency_ms = remaining;
            let outcome = session
                .run_turn(
                    &request,
                    &ctx("s"),
                    Some(&mut semaprax::model_routing::engine::ConfiguredProvider {
                        profile: semaprax::model_routing::engine::ProviderProfile {
                            provider_id: "fixture-router".into(),
                            model_id: "router".into(),
                            checkpoint: "c1".into(),
                            ..Default::default()
                        },
                        invoker: &mut inv,
                        mode: ProviderMode::Explicit,
                        gate: semaprax::model_routing::engine::EnablementGate::not_evaluated(
                            "model-route/v1",
                            "fixture-router",
                        ),
                    }),
                    &target,
                    handlers_at(&clock, &cancel, &mut factory, &mut classifier, &mut backoff),
                    &mut MemStore::default(),
                    None,
                    &mut |_| accepted(b"s", false),
                )
                .unwrap();
            assert_eq!(inv.deadlines, [bound]);
            // The late answer is not honoured: the rules path routed it.
            assert_eq!(outcome.profile, "fast");
            assert_eq!(
                created.borrow().len(),
                1,
                "an in-deadline turn generates once"
            );
        });
    }
}

// ---------------------------------------------------------------- DV-08

#[test]
fn a_child_inherits_the_parent_deadline_through_nested_grants_and_restore() {
    with_world(|w| {
        let set = w.set();
        let created: Created = Rc::default();
        let mut factory = settling_factory(&set, response(&w.schema, "1"), created.clone());
        let (mut classifier, mut backoff) = (ConservativeFailureClassifier, NoDelayBackoff);
        let cancel = AgentCancellation::new();
        let target = w.target(b"t");
        let deep = SessionPolicy {
            max_delegation_depth: 2,
            ..policy()
        };
        let ask = |child: &str, amount: i64| DelegationRequest {
            child: child.into(),
            specialist: "reviewer".into(),
            amount,
        };
        let mut parent_journal = MemStore::default();
        let mut parent = open(&set, deep.clone(), &mut parent_journal, 10, Some(1_000));
        let grant = parent.delegate(&ask("child.a", 5), 0).unwrap();
        assert_eq!(grant.deadline, Some(1_000));
        // Delegation at or after the enclosing deadline refuses.
        assert!(parent.delegate(&ask("child.late", 1), 1_000).is_err());

        let mut child_journal = MemStore::default();
        let mut child = RoutedSession::open_child(
            &set,
            deep.clone(),
            &grant,
            "sha256:i",
            "sha256:a",
            &mut child_journal,
        )
        .unwrap();
        assert!(child.journal().contains("\"deadline\":1000"));
        // Nested grants carry the same earliest enclosing deadline.
        let nested = child.delegate(&ask("grand", 1), 0).unwrap();
        assert_eq!(nested.deadline, Some(1_000));
        assert!(child.delegate(&ask("grand.late", 1), 1_000).is_err());

        // The child can use its allowance before expiry...
        let early = StepClock::new(0);
        child
            .run_turn::<dyn DecisionInvoker>(
                &turn("hard", TaskFamily::SemanticLaw, 1),
                &ctx("c"),
                None,
                &target,
                handlers_at(&early, &cancel, &mut factory, &mut classifier, &mut backoff),
                &mut MemStore::default(),
                None,
                &mut |_| accepted(b"c0", false),
            )
            .unwrap();
        let built = created.borrow().len();
        // ...but refuses new work at the enclosing deadline.
        let late = StepClock::new(1_000);
        session_err(
            child.run_turn::<dyn DecisionInvoker>(
                &turn("hard", TaskFamily::SemanticLaw, 1),
                &ctx("c"),
                None,
                &target,
                handlers_at(&late, &cancel, &mut factory, &mut classifier, &mut backoff),
                &mut MemStore::default(),
                None,
                &mut |_| accepted(b"never", false),
            ),
            "deadline_exceeded",
        );
        assert_eq!(created.borrow().len(), built);
        let (doc, generation) = (child.journal(), child.generation());
        drop(child);

        // A child restored after the parent's deadline remains expired.
        let mut restored_store = MemStore::default();
        let mut restored =
            RoutedSession::resume(&set, deep, &doc, generation, &mut restored_store).unwrap();
        session_err(
            restored.run_turn::<dyn DecisionInvoker>(
                &turn("hard", TaskFamily::SemanticLaw, 1),
                &ctx("c"),
                None,
                &target,
                handlers_at(&late, &cancel, &mut factory, &mut classifier, &mut backoff),
                &mut MemStore::default(),
                None,
                &mut |_| accepted(b"never", false),
            ),
            "deadline_exceeded",
        );
        assert_eq!(created.borrow().len(), built);
    });
}

#[test]
fn a_root_without_a_deadline_grants_children_without_one() {
    with_world(|w| {
        let set = w.set();
        let mut journal = MemStore::default();
        let mut parent = open(&set, policy(), &mut journal, 10, None);
        let grant = parent
            .delegate(
                &DelegationRequest {
                    child: "c".into(),
                    specialist: "reviewer".into(),
                    amount: 1,
                },
                0,
            )
            .unwrap();
        assert_eq!(grant.deadline, None);
    });
}
