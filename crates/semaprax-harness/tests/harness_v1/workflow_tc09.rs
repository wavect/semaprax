//! TC-09: reusable and scripted proposals are taken before paid routing and
//! reservation. Counting router and generator fakes with a routing provider
//! configured prove zero router/model calls and no reservation on a local hit.

use super::*;
use semaprax_harness::decision::EnablementGate;

/// Side-effecting (model-like) generator that counts its calls.
struct Gen(Cell<u32>, Vec<u8>);
struct GenRef<'a>(&'a Gen);
impl ProposalStage for GenRef<'_> {
    fn id(&self) -> String {
        "org.example/paid-model".into()
    }
    fn propose(&mut self, _r: &ProposalRequest) -> Result<Vec<u8>, StageFailure> {
        self.0 .0.set(self.0 .0.get() + 1);
        Ok(self.0 .1.clone())
    }
    fn calls(&self) -> u32 {
        self.0 .0.get()
    }
    fn side_effecting(&self) -> bool {
        true
    }
}

fn good() -> Vec<u8> {
    proposal("replace_function_body")
}

fn cfg_for(e: &Env, seed: &str) -> RunConfig {
    let task = Task {
        models: Some(two_models()),
        goal: seed.into(),
        ..Task::default()
    };
    config(e, task, None)
}

/// Run with the paid router configured (explicit mode: it is consulted
/// whenever routing happens at all).
fn exec(cfg: &RunConfig, prop: &mut dyn ProposalStage, router: &mut Router) -> Report {
    let fake = Fake::new(FIXED);
    let mut native = NativeContext::new(&fake);
    let mut view = RawCommandView;
    let mut obs = Observer::new(None, ObserverLimits::default());
    let profile = ProviderProfile {
        provider_id: "org.example/paid-route".into(),
        model_id: "m".into(),
        checkpoint: "1".into(),
        min_confidence: None,
        max_context_tokens: None,
        supported_families: None,
    };
    let gate = EnablementGate::not_evaluated("model-route/v1", &profile.provider_id);
    run(
        cfg,
        &fake,
        Stages {
            decision: Some(DecisionStage {
                invoker: router,
                profile,
                mode: ProviderMode::Explicit,
                gate,
            }),
            native: &mut native,
            external: None,
            proposer: prop,
            command: &mut view,
        },
        &mut obs,
    )
}

fn proposal_file(e: &Env, lineage: &str) -> PathBuf {
    e.cache.join(format!("{lineage}.proposal.json"))
}

#[test]
fn tc09_miss_reserves_exactly_the_dispatched_work_then_journal_hit_is_free() {
    let e = setup(LIB);
    let cfg = cfg_for(&e, "g");
    let gen = Gen(Cell::new(0), good());
    let mut router = Router(0, "strong");
    let r1 = exec(&cfg, &mut GenRef(&gen), &mut router);
    assert_eq!(r1.status, "approved-candidate-ready", "{:?}", r1.refusals);
    assert_eq!((router.0, gen.0.get()), (1, 1));
    let entries = r1.context["task_ledger"]["entries"].as_array().unwrap();
    let kinds: Vec<_> = entries
        .iter()
        .map(|x| x["kind"].as_str().unwrap())
        .collect();
    assert_eq!(
        kinds,
        ["router", "generation"],
        "one router + one generation"
    );

    // Same lineage, paid router configured: zero router, zero model, no ledger.
    let gen2 = Gen(Cell::new(0), good());
    let mut router2 = Router(0, "strong");
    let r2 = exec(&cfg, &mut GenRef(&gen2), &mut router2);
    assert_eq!(r2.status, "approved-candidate-ready", "{:?}", r2.refusals);
    assert_eq!((router2.0, gen2.0.get()), (0, 0));
    assert!(r2.context["task_ledger"].is_null(), "no new reservation");
    let acq = &r2.context["proposal_acquisition"];
    assert_eq!(acq["source"], "journal");
    assert_eq!(acq["new_reservations"], 0);
    // Historical spend stays visible, reported separately from new spend.
    assert!(
        acq["historical_incurred"]["request_bytes"]
            .as_u64()
            .unwrap()
            > 0,
        "{acq}"
    );
    assert_eq!(r2.route["router_calls"], 0);
    let j = std::fs::read_to_string(e.cache.join(format!("{}.journal.jsonl", r2.lineage))).unwrap();
    assert!(j.contains("generate.local-reuse"), "{j}");
}

#[test]
fn tc09_scripted_proposal_makes_zero_router_calls_with_paid_router_configured() {
    let e = setup(LIB);
    let cfg = cfg_for(&e, "g");
    let mut router = Router(0, "strong");
    let mut p = ScriptedProposer::from_bytes(good());
    let r = exec(&cfg, &mut p, &mut router);
    assert_eq!(r.status, "approved-candidate-ready", "{:?}", r.refusals);
    assert_eq!(router.0, 0);
    assert!(r.context["task_ledger"].is_null());
    assert_eq!(r.context["proposal_acquisition"]["source"], "scripted");
    // --proposal file form is compatible.
    let f = write(
        &e.root,
        "host/p.json",
        std::str::from_utf8(&good()).unwrap(),
    );
    let mut p = ScriptedProposer::from_file(f);
    let r = exec(&cfg, &mut p, &mut router);
    assert_eq!((r.status, router.0), ("approved-candidate-ready", 0));
    // No source at all still takes the generation path (unavailable).
    let r = exec(&cfg, &mut ScriptedProposer::empty(), &mut router);
    assert_eq!(codes(&r), ["SPX-HPD090"]);
}

#[test]
fn tc09_zero_remaining_budget_still_verifies_a_valid_completed_artifact() {
    let e = setup(LIB);
    let cfg = cfg_for(&e, "g");
    let gen = Gen(Cell::new(0), good());
    let r1 = exec(&cfg, &mut GenRef(&gen), &mut Router(0, "strong"));
    assert_eq!(r1.status, "approved-candidate-ready");
    let mut zero = cfg_for(&e, "g");
    zero.budget.policy.max_task_tokens = Some(0);
    let gen2 = Gen(Cell::new(0), good());
    let mut router = Router(0, "strong");
    let r2 = exec(&zero, &mut GenRef(&gen2), &mut router);
    assert_eq!(r2.status, "approved-candidate-ready", "{:?}", r2.refusals);
    assert_eq!((router.0, gen2.0.get()), (0, 0));
    assert!(
        r2.context["proposal_acquisition"]["historical_incurred"]["request_bytes"]
            .as_u64()
            .unwrap()
            > 0
    );
    // A genuinely fresh request under the same zero budget is refused.
    let e2 = setup(LIB);
    let mut z2 = cfg_for(&e2, "g");
    z2.budget.policy.max_task_tokens = Some(0);
    let gen3 = Gen(Cell::new(0), good());
    let r3 = exec(&z2, &mut GenRef(&gen3), &mut Router(0, "strong"));
    assert_eq!((codes(&r3), gen3.0.get()), (vec!["SPX-HPD101"], 0));
}

#[test]
fn tc09_tampered_missing_or_changed_inputs_never_give_unvalidated_reuse() {
    let e = setup(LIB);
    let cfg = cfg_for(&e, "g");
    let gen = Gen(Cell::new(0), good());
    let r1 = exec(&cfg, &mut GenRef(&gen), &mut Router(0, "strong"));
    let art = proposal_file(&e, &r1.lineage);
    let orig = std::fs::read(&art).unwrap();

    let again = |cfg: &RunConfig| {
        let g = Gen(Cell::new(0), good());
        let mut rt = Router(0, "strong");
        let r = exec(cfg, &mut GenRef(&g), &mut rt);
        (r, g.0.get(), rt.0)
    };
    // Tampered bytes: refused, no router, no model, no reuse.
    std::fs::write(&art, proposal("rewrite_file")).unwrap();
    let (r, g, rt) = again(&cfg);
    assert_eq!(
        (r.status, codes(&r), g, rt),
        ("uncertain", vec!["SPX-HPD072"], 0, 0)
    );
    // Oversized artifact.
    std::fs::write(&art, vec![b' '; 1024 * 1024 + 1]).unwrap();
    let (r, g, rt) = again(&cfg);
    assert_eq!((codes(&r), g, rt), (vec!["SPX-HPD072"], 0, 0));
    // Missing cache data.
    std::fs::remove_file(&art).unwrap();
    let (r, g, rt) = again(&cfg);
    assert_eq!((codes(&r), g, rt), (vec!["SPX-HPD072"], 0, 0));
    // Restored bytes validate again.
    std::fs::write(&art, &orig).unwrap();
    let (r, g, rt) = again(&cfg);
    assert_eq!((r.status, g, rt), ("approved-candidate-ready", 0, 0));
    // A changed task, lock (permissions) or baseline is a different lineage:
    // the old artifact is not reused, and the miss follows the generation path.
    let (r, g, _) = again(&cfg_for(&e, "other goal"));
    assert_ne!(r.lineage, r1.lineage);
    assert_eq!(g, 1);
    let mut locked = cfg_for(&e, "g");
    locked.lock_digest = "sha256:other-lock".into();
    let (r, g, _) = again(&locked);
    assert_ne!(r.lineage, r1.lineage);
    assert_eq!(g, 1);
    let e3 = setup(&LIB.replace("BUG", "BUG2"));
    let (r, g, _) = again(&cfg_for(&e3, "g"));
    assert_eq!(g, 1, "{:?}", r.refusals);
}

#[test]
fn tc09_begun_or_uncertain_request_is_not_replayed_and_does_not_route() {
    let e = setup(LIB);
    let cfg = cfg_for(&e, "g");
    let lineage = semaprax_harness::workflow::lineage::Lineage::new(
        cfg.snapshot.binding(),
        &cfg.lock_digest,
        &cfg.task.digest(),
    )
    .id;
    for state in ["begin", "uncertain"] {
        write(
            &e.cache,
            &format!("{lineage}.journal.jsonl"),
            &format!("{{\"detail\":{{}},\"seq\":1,\"state\":\"{state}\",\"step\":\"generate\"}}\n"),
        );
        let gen = Gen(Cell::new(0), good());
        let mut router = Router(0, "strong");
        let r = exec(&cfg, &mut GenRef(&gen), &mut router);
        assert_eq!(
            (r.status, codes(&r)),
            ("uncertain", vec!["SPX-HPD072"]),
            "{state}"
        );
        assert_eq!((gen.0.get(), router.0), (0, 0), "{state}");
        assert!(
            r.context["task_ledger"].is_null(),
            "{state}: nothing reserved"
        );
    }
}

#[test]
fn tc09_same_bad_proposal_is_rejected_on_local_and_generated_paths() {
    let bad: Vec<(Vec<u8>, &str)> = vec![
        (proposal("rewrite_file"), "SPX-HPD031"),
        (
            json!({"schema": "semaprax.harness-proposal.v1", "base_revision": "x", "intent": {"kind": "replace_function_body"}})
                .to_string()
                .into_bytes(),
            "SPX-HPD032",
        ),
    ];
    for (bytes, code) in bad {
        // Generated path.
        let e = setup(LIB);
        let cfg = cfg_for(&e, "g");
        let gen = Gen(Cell::new(0), bytes.clone());
        let r = exec(&cfg, &mut GenRef(&gen), &mut Router(0, "strong"));
        assert_eq!(codes(&r), [code]);
        // Scripted path.
        let r = exec(
            &cfg,
            &mut ScriptedProposer::from_bytes(bytes.clone()),
            &mut Router(0, "strong"),
        );
        assert_eq!(codes(&r), [code]);
        // Journal path: seed a completed artifact with the same bytes.
        let e = setup(LIB);
        let cfg = cfg_for(&e, "g");
        let lineage = semaprax_harness::workflow::lineage::Lineage::new(
            cfg.snapshot.binding(),
            &cfg.lock_digest,
            &cfg.task.digest(),
        )
        .id;
        std::fs::create_dir_all(&e.cache).unwrap();
        std::fs::write(proposal_file(&e, &lineage), &bytes).unwrap();
        write(
            &e.cache,
            &format!("{lineage}.journal.jsonl"),
            &format!("{{\"detail\":{{\"digest\":\"{}\"}},\"seq\":1,\"state\":\"done\",\"step\":\"generate\"}}\n", sha256_plain(&bytes)),
        );
        let gen = Gen(Cell::new(0), bytes.clone());
        let mut router = Router(0, "strong");
        let r = exec(&cfg, &mut GenRef(&gen), &mut router);
        assert_eq!(codes(&r), [code]);
        assert_eq!((gen.0.get(), router.0), (0, 0));
    }
}
