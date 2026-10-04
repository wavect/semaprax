//! TC-12: the campaign's cost accounting over a real `workflow::pipeline::run`
//! report. A resumed, eligible cached proposal reserves and bills nothing new;
//! a fresh run's router and generator attempts stay in the totals with their
//! spend unknown (no receipt) rather than zero.

use super::*;
use semaprax_harness::bench::apptask::profile_campaign::{attempts_from_report, spend_of};
use semaprax_harness::decision::EnablementGate;

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

#[test]
fn tc12_resume_of_an_eligible_cached_proposal_adds_no_attempts_and_a_fresh_run_keeps_unknown_spend()
{
    let e = setup(LIB);
    let task = Task {
        models: Some(two_models()),
        goal: "g".into(),
        ..Task::default()
    };
    let cfg = config(&e, task, None);
    let gen = Gen(Cell::new(0), proposal("replace_function_body"));
    let mut router = Router(0, "strong");
    let r1 = exec(&cfg, &mut GenRef(&gen), &mut router);
    assert_eq!(r1.status, "approved-candidate-ready", "{:?}", r1.refusals);
    let first = attempts_from_report(&r1.context);
    let roles: Vec<&str> = first.iter().map(|a| a.role).collect();
    assert_eq!(
        roles,
        ["router", "generator"],
        "every dispatched attempt is recorded"
    );
    let s1 = spend_of(&first);
    assert!(
        s1.micros.is_none() && !s1.complete && s1.dispatched == 2,
        "spend with no receipt stays unknown, not zero"
    );

    // Resume: same lineage, paid router configured. Nothing new is dispatched.
    let gen2 = Gen(Cell::new(0), proposal("replace_function_body"));
    let mut router2 = Router(0, "strong");
    let r2 = exec(&cfg, &mut GenRef(&gen2), &mut router2);
    assert_eq!(r2.status, "approved-candidate-ready", "{:?}", r2.refusals);
    assert_eq!((router2.0, gen2.0.get()), (0, 0));
    let second = attempts_from_report(&r2.context);
    assert!(
        second.is_empty(),
        "no newly billed router or generator work"
    );
    let s2 = spend_of(&second);
    assert_eq!(s2.dispatched, 0);
    assert!(
        s2.micros.is_none(),
        "not dispatched is reported as such, never as a billed zero"
    );
    assert_eq!(r2.context["proposal_acquisition"]["new_reservations"], 0);
}
