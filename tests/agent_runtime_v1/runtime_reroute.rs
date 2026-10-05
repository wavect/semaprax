//! MR-10: runtime turns and agent handoffs re-route only at safe lifecycle
//! boundaries, over the real MR-09 routed-invocation path.

use std::cell::Cell;
use std::collections::{BTreeMap, BTreeSet};
use std::rc::Rc;

use super::runtime_routing::{
    choose, ctx, features, handlers, host, response, router, settling_factory, spec, with_world,
    Created, MemStore, ScriptedRouter, World,
};
use semaprax::model_routing::engine::{
    Confidentiality, DecisionInvoker, ProviderMode, RoutePolicy, TaskFamily,
};
use semaprax::model_routing::runtime::{
    ApprovedProfileSet, DelegationRequest, RerouteBoundary, RouteReason, RoutedSession,
    RuntimeRoutingError, SessionPolicy, SpecialistGrant, TurnRequest, TurnStatus, TurnVerdict,
};

fn policy() -> SessionPolicy {
    SessionPolicy {
        role_profiles: BTreeMap::from([
            ("simple".to_owned(), BTreeSet::from(["fast".to_owned()])),
            (
                "hard".to_owned(),
                BTreeSet::from(["fast".to_owned(), "strong".to_owned()]),
            ),
        ]),
        specialists: vec![SpecialistGrant {
            id: "reviewer".into(),
            profile: "strong".into(),
            delegable: true,
        }],
        escalation: None,
        max_delegation_depth: 1,
        max_turns: 4,
        confidentiality: Confidentiality::Project,
    }
}

fn turn(role: &str, family: TaskFamily, reservation: i64) -> TurnRequest {
    TurnRequest {
        role: role.into(),
        specialist: None,
        next_stage_capabilities: Vec::new(),
        features: features(family),
        reservation,
    }
}

fn accepted(state: &[u8], complete: bool) -> TurnVerdict {
    TurnVerdict::Accepted {
        committed_state: state.to_vec(),
        progressed: true,
        complete,
        tool_results: vec!["sha256:tool-result".into()],
    }
}

fn open<'a>(
    w: &World,
    set: &'a ApprovedProfileSet,
    store: &'a mut MemStore,
    ceiling: i64,
) -> RoutedSession<'a> {
    let _ = w;
    RoutedSession::open(
        set,
        policy(),
        "session.main",
        "sha256:instructions",
        "sha256:acceptance",
        ceiling,
        None,
        store,
    )
    .unwrap()
}

#[test]
fn a_two_turn_agent_changes_profile_only_between_durable_terminal_turns() {
    with_world(|w| {
        let set = w.set();
        host!(clock, cancel, classifier, backoff);
        let created: Created = Rc::default();
        let mut factory = settling_factory(&set, response(&w.schema, "1"), created.clone());
        let mut journal = MemStore::default();
        let mut session = open(w, &set, &mut journal, 10);
        assert_eq!(
            session.boundary(),
            Some(RerouteBoundary {
                turn: 0,
                previous_profile: None
            })
        );
        let target = w.target(b"unused: the session supplies the handoff");
        let mut outcomes = Vec::new();
        for (index, request) in [
            turn("simple", TaskFamily::LocalizedDebug, 3),
            turn("hard", TaskFamily::SemanticLaw, 3),
        ]
        .iter()
        .enumerate()
        {
            let mut turn_store = MemStore::default();
            let outcome = session
                .run_turn::<dyn DecisionInvoker>(
                    request,
                    &ctx("session"),
                    None,
                    &target,
                    handlers(&clock, &cancel, &mut factory, &mut classifier, &mut backoff),
                    &mut turn_store,
                    None,
                    &mut |_| accepted(format!("state-{index}").as_bytes(), false),
                )
                .unwrap();
            assert_eq!(outcome.status, TurnStatus::Continue);
            let run = outcome.run.clone().unwrap();
            let profile = set.profile(&outcome.profile).unwrap();
            assert_eq!(run.record.deployment(), profile.deployment_digest());
            // Each turn's envelope is its own invocation's journal.
            assert!(turn_store.commits[0].1.contains(&run.invocation));
            outcomes.push(outcome);
        }
        assert_eq!(outcomes[0].profile, "fast");
        assert_eq!(outcomes[0].reason, RouteReason::Initial);
        assert_eq!(outcomes[1].profile, "strong");
        assert_eq!(outcomes[1].reason, RouteReason::Reroute);
        let (a, b) = (
            outcomes[0].run.as_ref().unwrap(),
            outcomes[1].run.as_ref().unwrap(),
        );
        assert_ne!(a.deployment_root, b.deployment_root);
        assert_ne!(a.instance_root, b.instance_root);
        assert_ne!(a.invocation, b.invocation);
        assert_eq!(
            session.boundary(),
            Some(RerouteBoundary {
                turn: 2,
                previous_profile: Some("strong".into())
            })
        );
        // Route transitions and handoff digests are journaled; the second
        // handoff carries turn 0's committed state, not a transcript.
        let doc = session.journal();
        assert_eq!(doc.matches("\"kind\":\"routed\"").count(), 2);
        assert!(
            doc.contains("73746174652d30"),
            "turn 0 committed state is journaled"
        );
        assert!(session.handoff_digest(1).unwrap().starts_with("sha256:"));
        assert_ne!(session.handoff_digest(0), session.handoff_digest(1));
        assert_eq!(session.committed(), 6);
        assert_eq!(created.borrow().as_slice(), ["fake.local", "other.local"]);
    });
}

#[test]
fn child_reservations_count_against_the_parent_allowance() {
    with_world(|w| {
        let set = w.set();
        host!(clock, cancel, classifier, backoff);
        let created: Created = Rc::default();
        let mut factory = settling_factory(&set, response(&w.schema, "1"), created.clone());
        let mut journal = MemStore::default();
        let mut session = open(w, &set, &mut journal, 10);
        let target = w.target(b"t");
        let mut turn_store = MemStore::default();
        session
            .run_turn::<dyn DecisionInvoker>(
                &turn("hard", TaskFamily::SemanticLaw, 3),
                &ctx("s"),
                None,
                &target,
                handlers(&clock, &cancel, &mut factory, &mut classifier, &mut backoff),
                &mut turn_store,
                None,
                &mut |_| accepted(b"s0", false),
            )
            .unwrap();
        let delegate = |s: &mut RoutedSession<'_>, child: &str, amount: i64| {
            s.delegate(
                &DelegationRequest {
                    child: child.into(),
                    specialist: "reviewer".into(),
                    amount,
                },
                0,
            )
        };
        let first = delegate(&mut session, "child.a", 4).unwrap();
        assert_eq!(
            (first.depth, first.allowance, first.caller.as_str()),
            (1, 4, "session.main")
        );
        // Repeated admissions cannot overspend: 3 + 4 + 4 > 10.
        assert!(
            matches!(delegate(&mut session, "child.b", 4), Err(RuntimeRoutingError::Session(m)) if m.contains("parent allowance"))
        );
        let second = delegate(&mut session, "child.c", 3).unwrap();
        assert_eq!(session.committed(), 10);
        assert!(delegate(&mut session, "child.d", 1).is_err());
        // The parent's own next turn is now refused before dispatch.
        let before = created.borrow().len();
        let refused = session.run_turn::<dyn DecisionInvoker>(
            &turn("simple", TaskFamily::LocalizedDebug, 1),
            &ctx("s"),
            None,
            &target,
            handlers(&clock, &cancel, &mut factory, &mut classifier, &mut backoff),
            &mut MemStore::default(),
            None,
            &mut |_| accepted(b"never", false),
        );
        assert!(
            matches!(refused, Err(RuntimeRoutingError::Session(m)) if m.contains("parent allowance"))
        );
        assert_eq!(created.borrow().len(), before);
        // Settlement reconciles without refund or overspend.
        assert!(session.settle_child("child.a", 5).is_err());
        session.settle_child("child.a", 2).unwrap();
        assert!(session.settle_child("child.a", 1).is_err());
        assert_eq!(session.committed(), 10);

        // A child cannot reset its budget: its ceiling is the grant.
        let mut child_journal = MemStore::default();
        let mut child = RoutedSession::open_child(
            &set,
            policy(),
            &second,
            "sha256:child-instructions",
            "sha256:acceptance",
            &mut child_journal,
        )
        .unwrap();
        let over = child.run_turn::<dyn DecisionInvoker>(
            &turn("hard", TaskFamily::SemanticLaw, 4),
            &ctx("c"),
            None,
            &target,
            handlers(&clock, &cancel, &mut factory, &mut classifier, &mut backoff),
            &mut MemStore::default(),
            None,
            &mut |_| accepted(b"never", false),
        );
        assert!(
            matches!(over, Err(RuntimeRoutingError::Session(m)) if m.contains("parent allowance"))
        );
        assert_eq!(created.borrow().len(), before);
    });
}

#[test]
fn a_timed_out_tool_with_unknown_effect_is_not_replayed_by_a_stronger_model() {
    with_world(|w| {
        let set = w.set();
        host!(clock, cancel, classifier, backoff);
        let created: Created = Rc::default();
        let mut factory = settling_factory(&set, response(&w.schema, "1"), created.clone());
        let mut journal = MemStore::default();
        let mut session = open(w, &set, &mut journal, 10);
        let target = w.target(b"t");
        let outcome = session
            .run_turn::<dyn DecisionInvoker>(
                &turn("simple", TaskFamily::LocalizedDebug, 2),
                &ctx("s"),
                None,
                &target,
                handlers(&clock, &cancel, &mut factory, &mut classifier, &mut backoff),
                &mut MemStore::default(),
                None,
                &mut |_| TurnVerdict::EffectUncertain,
            )
            .unwrap();
        assert_eq!(outcome.status, TurnStatus::Uncertain);
        assert_eq!(
            session.boundary(),
            None,
            "no re-route boundary after an uncertain effect"
        );
        let mut stronger = turn("hard", TaskFamily::SemanticLaw, 2);
        stronger.specialist = Some("reviewer".into());
        let refused = session.run_turn::<dyn DecisionInvoker>(
            &stronger,
            &ctx("s"),
            None,
            &target,
            handlers(&clock, &cancel, &mut factory, &mut classifier, &mut backoff),
            &mut MemStore::default(),
            None,
            &mut |_| accepted(b"never", false),
        );
        assert!(
            matches!(refused, Err(RuntimeRoutingError::Session(m)) if m.contains("uncertain effect"))
        );
        assert!(session
            .delegate(
                &DelegationRequest {
                    child: "child".into(),
                    specialist: "reviewer".into(),
                    amount: 1
                },
                0
            )
            .is_err());
        assert_eq!(
            created.borrow().as_slice(),
            ["fake.local"],
            "the specialist was never constructed"
        );
    });
}

#[test]
fn a_resumed_session_makes_no_duplicate_route_model_or_effect_call() {
    with_world(|w| {
        let set = w.set();
        host!(clock, cancel, classifier, backoff);
        let created: Created = Rc::default();
        let mut factory = settling_factory(&set, response(&w.schema, "1"), created.clone());
        let mut inv = ScriptedRouter {
            calls: 0,
            answer: Some(choose("strong")),
        };
        let effects = Cell::new(0);
        let mut journal = MemStore::default();
        let mut turn_stores = vec![MemStore::default(), MemStore::default()];
        {
            let mut session = open(w, &set, &mut journal, 10);
            let target = w.target(b"t");
            for (i, store) in turn_stores.iter_mut().enumerate() {
                let request = turn("hard", TaskFamily::LocalizedDebug, 2);
                session
                    .run_turn(
                        &request,
                        &ctx("s"),
                        Some(&mut router(&mut inv, ProviderMode::Explicit)),
                        &target,
                        handlers(&clock, &cancel, &mut factory, &mut classifier, &mut backoff),
                        store,
                        None,
                        &mut |_| {
                            effects.set(effects.get() + 1);
                            accepted(format!("s{i}").as_bytes(), false)
                        },
                    )
                    .unwrap();
            }
        }
        let (calls, built, applied) = (inv.calls, created.borrow().len(), effects.get());
        assert_eq!((calls, built, applied), (2, 2, 2));

        // Full resume: both completed boundaries replay from the journal.
        let (doc, generation) = journal.latest();
        let mut resumed_store = MemStore::default();
        let resumed =
            RoutedSession::resume(&set, policy(), &doc, generation, &mut resumed_store).unwrap();
        for t in 0..2 {
            let replay = resumed.replay_turn(t).unwrap();
            assert!(replay.replayed);
            assert_eq!(replay.profile, "strong");
            assert_eq!(replay.status, TurnStatus::Continue);
        }
        assert_eq!(resumed.boundary().unwrap().turn, 2);

        // Crash after turn 1 was routed but before its settlement: resume
        // reuses the recorded route and the retained invocation journal.
        let routed_at = journal
            .commits
            .iter()
            .position(|(_, d)| d.matches("\"kind\":\"routed\"").count() == 2)
            .unwrap();
        let (mid_gen, mid_doc) = journal.commits[routed_at].clone();
        assert_eq!(mid_doc.matches("\"kind\":\"settled\"").count(), 1);
        let mut crash_store = MemStore::default();
        let mut in_flight =
            RoutedSession::resume(&set, policy(), &mid_doc, mid_gen, &mut crash_store).unwrap();
        assert_eq!(in_flight.boundary(), None, "a turn is in flight");
        let (envelope, env_gen) = turn_stores[1].latest();
        let target = w.target(b"t");
        let outcome = in_flight
            .run_turn(
                &turn("hard", TaskFamily::LocalizedDebug, 2),
                &ctx("s"),
                Some(&mut router(&mut inv, ProviderMode::Explicit)),
                &target,
                handlers(&clock, &cancel, &mut factory, &mut classifier, &mut backoff),
                &mut MemStore::default(),
                Some((&envelope, env_gen)),
                &mut |_| accepted(b"s1", false),
            )
            .unwrap();
        assert_eq!(outcome.router_calls, 0);
        assert_eq!(outcome.profile, "strong");
        assert!(matches!(
            outcome.run.unwrap().run,
            semaprax::live_invocation::DurablePolicyRun::Replayed(_)
        ));
        assert_eq!(inv.calls, calls, "no duplicate route call");
        assert_eq!(created.borrow().len(), built, "no duplicate model call");
        assert_eq!(effects.get(), applied, "no duplicate external effect");
    });
}

#[test]
fn unauthorized_specialist_confidentiality_depth_and_router_recursion_are_rejected() {
    with_world(|w| {
        let set = w.set();
        host!(clock, cancel, classifier, backoff);
        let created: Created = Rc::default();
        let mut factory = settling_factory(&set, response(&w.schema, "1"), created.clone());
        let mut journal = MemStore::default();
        let mut session = open(w, &set, &mut journal, 10);
        let target = w.target(b"t");
        let mut run =
            |s: &mut RoutedSession<'_>, request: &TurnRequest, inv: Option<&mut ScriptedRouter>| {
                let mut provider = inv.map(|i| router(i, ProviderMode::Explicit));
                s.run_turn(
                    request,
                    &ctx("s"),
                    provider.as_mut(),
                    &target,
                    handlers(&clock, &cancel, &mut factory, &mut classifier, &mut backoff),
                    &mut MemStore::default(),
                    None,
                    &mut |_| accepted(b"s", false),
                )
            };
        let session_err = |r: Result<_, RuntimeRoutingError>, needle: &str| match r {
            Err(RuntimeRoutingError::Session(m)) => assert!(m.contains(needle), "{m}"),
            other => panic!(
                "expected `{needle}`, got {:?}",
                other.map(|o: semaprax::model_routing::runtime::TurnOutcome| o.status)
            ),
        };
        let mut ghost = turn("hard", TaskFamily::LocalizedDebug, 1);
        ghost.specialist = Some("ghost".into());
        session_err(
            run(&mut session, &ghost, None),
            "specialist `ghost` is not authorized",
        );
        session_err(
            run(
                &mut session,
                &turn("admin", TaskFamily::LocalizedDebug, 1),
                None,
            ),
            "role `admin`",
        );
        let mut public = turn("hard", TaskFamily::LocalizedDebug, 1);
        public.features.confidentiality = Confidentiality::Public;
        session_err(run(&mut session, &public, None), "confidentiality");
        assert!(created.borrow().is_empty());

        // A turn routed by the decision provider puts it on the lineage.
        let mut inv = ScriptedRouter {
            calls: 0,
            answer: Some(choose("strong")),
        };
        let routed = run(
            &mut session,
            &turn("hard", TaskFamily::LocalizedDebug, 1),
            Some(&mut inv),
        )
        .unwrap();
        assert_eq!(routed.router_calls, 1);
        let grant = session
            .delegate(
                &DelegationRequest {
                    child: "child".into(),
                    specialist: "reviewer".into(),
                    amount: 3,
                },
                0,
            )
            .unwrap();
        assert!(grant.router_lineage.contains(&"fixture-router".to_owned()));
        let mut child_journal = MemStore::default();
        let mut child = RoutedSession::open_child(
            &set,
            policy(),
            &grant,
            "sha256:i",
            "sha256:a",
            &mut child_journal,
        )
        .unwrap();
        session_err(
            child
                .delegate(
                    &DelegationRequest {
                        child: "grandchild".into(),
                        specialist: "reviewer".into(),
                        amount: 1,
                    },
                    0,
                )
                .map(|_| unreachable_outcome()),
            "delegation depth 2 exceeds the bound 1",
        );
        let mut child_inv = ScriptedRouter {
            calls: 0,
            answer: Some(choose("strong")),
        };
        session_err(
            run(
                &mut child,
                &turn("hard", TaskFamily::LocalizedDebug, 1),
                Some(&mut child_inv),
            ),
            "router recursion",
        );
        assert_eq!(child_inv.calls, 0);
    });
}

fn unreachable_outcome() -> semaprax::model_routing::runtime::TurnOutcome {
    unreachable!("delegation is refused")
}

#[test]
fn single_turn_and_static_model_apps_show_no_added_lifecycle_or_routing_calls() {
    with_world(|w| {
        let single = ApprovedProfileSet::approve(
            &w.semantic,
            vec![spec("only", &w.fast, 10, 1)],
            RoutePolicy::default(),
        )
        .unwrap();
        host!(clock, cancel, classifier, backoff);
        let created: Created = Rc::default();
        let mut factory = settling_factory(&single, response(&w.schema, "1"), created.clone());
        let mut inv = ScriptedRouter {
            calls: 0,
            answer: Some(choose("only")),
        };
        let mut journal = MemStore::default();
        let mut session = RoutedSession::open(
            &single,
            SessionPolicy {
                role_profiles: BTreeMap::from([(
                    "main".to_owned(),
                    BTreeSet::from(["only".to_owned()]),
                )]),
                ..policy()
            },
            "single",
            "sha256:i",
            "sha256:a",
            5,
            None,
            &mut journal,
        )
        .unwrap();
        let target = w.target(b"t");
        let mut turn_store = MemStore::default();
        let outcome = session
            .run_turn(
                &turn("main", TaskFamily::LocalizedDebug, 1),
                &ctx("single"),
                Some(&mut router(&mut inv, ProviderMode::Explicit)),
                &target,
                handlers(&clock, &cancel, &mut factory, &mut classifier, &mut backoff),
                &mut turn_store,
                None,
                &mut |_| accepted(b"done", true),
            )
            .unwrap();
        assert_eq!(outcome.status, TurnStatus::Complete);
        assert_eq!(outcome.router_calls, 0);
        assert_eq!(inv.calls, 0);
        assert_eq!(
            created.borrow().len(),
            1,
            "exactly one adapter, no fallback or extra turn"
        );
        drop(session);
        // opened, routed, settled: nothing else is journaled.
        assert_eq!(journal.commits.len(), 3);
        // The invocation journal is the unchanged kernel's (route + intent + settle).
        assert_eq!(turn_store.commits.len(), 3);
    });
}
