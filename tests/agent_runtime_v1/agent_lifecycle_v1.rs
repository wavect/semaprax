//! Agent Lifecycle v1: deterministic stage identities bound to verified
//! `.spx` functions, one acyclic lifecycle executed through the retained
//! interpreter seam, and the opaque one-use authorization value only the
//! validated authorize stage can construct.

use semaprax::agent_definition::compile_agent_definition;
use semaprax::agent_lifecycle::{
    compile_agent_lifecycle, verify_agent_lifecycle_bundle, AgentReadOperation, AuthorizedRequest,
    LifecycleBudget, LifecycleStatus, LifecycleTask,
};
use semaprax::agent_runtime::AgentCancellation;

use super::agent_definition_v1::definition;
use super::{profile, raw_sha};

pub(super) const MODULE_PATH: &str = "fixture-agent-lifecycle.spx";

/// One acyclic lifecycle over the six role types of the frozen fixture
/// AgentDefinition. Every declaration carries an explicit stable identity, the
/// four deterministic stages declare no effect, and every stage value lives
/// inside the retained seam's closed vocabulary: scalars, owned `Bytes`, and
/// bounded records and owned-byte variants over those.
pub(super) const MODULE: &str = r#"module fixture.agent.lifecycle;

@id("fixture.agent.type.task")
record Task {
    @id("fixture.agent.type.task.objective")
    objective: Bytes,
    @id("fixture.agent.type.task.budget")
    budget: i64,
}

@id("fixture.agent.type.state")
record State {
    @id("fixture.agent.type.state.objective")
    objective: Bytes,
    @id("fixture.agent.type.state.budget")
    budget: i64,
    @id("fixture.agent.type.state.epoch")
    epoch: i64,
}

@id("fixture.agent.type.observation")
record Observation {
    @id("fixture.agent.type.observation.tag")
    tag: Bytes,
    @id("fixture.agent.type.observation.budget")
    budget: i64,
    @id("fixture.agent.type.observation.epoch")
    epoch: i64,
}

@id("fixture.agent.type.proposal")
record Proposal {
    @id("fixture.agent.type.proposal.budget")
    budget: i64,
    @id("fixture.agent.type.proposal.urgent")
    urgent: bool,
    @id("fixture.agent.type.proposal.sequence")
    sequence: usize,
}

@id("fixture.agent.type.decision")
variant Decision {
    @id("fixture.agent.type.decision.granted")
    Granted {
        @id("fixture.agent.type.decision.granted.seal")
        seal: Bytes,
        @id("fixture.agent.type.decision.granted.budget")
        budget: i64,
    },
    @id("fixture.agent.type.decision.refused")
    Refused {
        @id("fixture.agent.type.decision.refused.code")
        code: i64,
    },
}

@id("fixture.agent.type.outcome")
record Outcome {
    @id("fixture.agent.type.outcome.value")
    value: Bytes,
    @id("fixture.agent.type.outcome.status")
    status: i64,
}

@id("fixture.agent.type.result")
record Report {
    @id("fixture.agent.type.result.summary")
    summary: Bytes,
    @id("fixture.agent.type.result.budget")
    budget: i64,
    @id("fixture.agent.type.result.status")
    status: i64,
}

@id("fixture.agent.fn.initialize")
fn initialize(task: own Task) -> State
{
    State { objective: task.objective, budget: task.budget, epoch: 1 }
}

@id("fixture.agent.fn.observe")
fn observe(state: borrow State) -> Observation
{
    let tag = [79u8, 66u8];
    Observation {
        tag: bytes_copy(array_as_slice(tag)),
        budget: state.budget,
        epoch: state.epoch,
    }
}

@id("fixture.agent.fn.authorize")
fn authorize(state: borrow State, budget: i64, urgent: bool, sequence: usize) -> Decision
{
    let seal = [65u8, 90u8];
    if budget <= state.budget && sequence > 0usize {
        Decision::Granted { seal: bytes_copy(array_as_slice(seal)), budget: budget }
    } else {
        Decision::Refused { code: if urgent { 2 } else { 1 } }
    }
}

@id("fixture.agent.fn.reduce")
fn reduce(state: own State, budget: i64, urgent: bool, sequence: usize, outcome: own Outcome) -> Report
{
    let stamp = [82u8, 80u8];
    Report {
        summary: bytes_copy(array_as_slice(stamp)),
        budget: state.budget - budget,
        status: outcome.status + state.epoch,
    }
}

@id("app.main")
fn main() -> i64 { 0 }
"#;

struct Read {
    value: Vec<u8>,
    fails: bool,
    calls: usize,
    bindings: Vec<String>,
}

impl Read {
    fn new() -> Self {
        Self {
            value: b"observed".to_vec(),
            fails: false,
            calls: 0,
            bindings: Vec::new(),
        }
    }

    fn failing() -> Self {
        Self {
            fails: true,
            ..Self::new()
        }
    }
}

impl AgentReadOperation for Read {
    fn read(&mut self, request: &AuthorizedRequest) -> Option<Vec<u8>> {
        self.calls += 1;
        self.bindings.push(request.binding().to_owned());
        // The authorize stage's own granted budget, never the proposal's.
        assert!(request.budget() >= 0);
        assert_eq!(request.seal(), b"AZ");
        (!self.fails).then(|| self.value.clone())
    }
}

pub(super) fn proposal(schema_digest: &str, budget: &str, urgent: bool, sequence: &str) -> String {
    format!(
        concat!(
            "{{\"schema\":\"semaprax.agent-proposal.v1\",\"agent_id\":\"fixture.agent\",",
            "\"proposal_schema_digest\":\"{digest}\",\"value\":{{\"fields\":{{",
            "\"fixture.agent.type.proposal.budget\":\"{budget}\",",
            "\"fixture.agent.type.proposal.urgent\":{urgent},",
            "\"fixture.agent.type.proposal.sequence\":\"{sequence}\"}}}}}}\n"
        ),
        digest = schema_digest,
        budget = budget,
        urgent = urgent,
        sequence = sequence
    )
}

fn task() -> LifecycleTask {
    LifecycleTask {
        objective: b"alpha".to_vec(),
        budget: 12,
    }
}

#[test]
fn lifecycle_binds_four_verified_stages_and_runs_one_acyclic_pass() {
    let source = definition(&profile());
    let first = compile_agent_lifecycle(MODULE, MODULE_PATH, &source).unwrap();
    let second = compile_agent_lifecycle(MODULE, MODULE_PATH, &source).unwrap();
    assert_eq!(first.canonical_json(), second.canonical_json());
    assert_eq!(first.digest(), second.digest());
    assert!(first.canonical_json().ends_with('\n'));
    assert_eq!(first.agent_id(), "fixture.agent");

    // The stage identities resolve to actual functions, not to descriptions.
    for (role, function_id) in [
        ("initialize", "fixture.agent.fn.initialize"),
        ("observe", "fixture.agent.fn.observe"),
        ("authorize", "fixture.agent.fn.authorize"),
        ("reduce", "fixture.agent.fn.reduce"),
    ] {
        assert_eq!(first.stage_function_id(role), Some(function_id));
    }
    assert_eq!(first.stage_function_id("propose"), None);
    assert_eq!(first.stage_function_id("execute"), None);
    assert_eq!(
        first.stage_order(),
        [
            "initialize",
            "observe",
            "propose",
            "authorize",
            "execute",
            "reduce"
        ]
    );

    // Ownership modes are published from the same HIR the interpreter runs.
    let document = first.canonical_json();
    assert!(document.contains("\"ownership\":\"consumes\",\"type\":\"fixture.agent.type.task\""));
    assert!(document.contains("\"ownership\":\"borrows\",\"type\":\"fixture.agent.type.state\""));
    assert!(document.contains("\"fixture.agent.type.decision.granted\""));
    assert!(document.contains("\"no_reusable_authorization_value_one_grant_admits_one_effect\""));

    let mut read = Read::new();
    let run = first
        .run(
            &task(),
            &proposal(first.proposal_schema().schema().digest(), "5", false, "1"),
            &mut read,
            LifecycleBudget::default(),
            &AgentCancellation::new(),
        )
        .unwrap();
    assert_eq!(run.status(), LifecycleStatus::Completed);
    assert_eq!(run.reason(), "reduce_published_a_result");
    assert_eq!(read.calls, 1);
    assert!(run.result().is_some());
    assert!(run.authorization_spent());
    assert_eq!(run.authorization_binding(), Some(read.bindings[0].as_str()));
    assert_eq!(run.refusal_code(), None);

    // Every deterministic stage ran, in order, and settled its owned leaves.
    let roles = run
        .stages()
        .iter()
        .map(|stage| stage.role())
        .collect::<Vec<_>>();
    assert_eq!(roles, ["initialize", "observe", "authorize", "reduce"]);
    for stage in run.stages() {
        assert_eq!(stage.outcome(), "returned");
        assert!(stage.cleanup_events() >= 1, "{}", stage.function_id());
    }

    // The same inputs replay to the same evidence bytes and the same digest.
    let mut replay = Read::new();
    let again = first
        .run(
            &task(),
            &proposal(first.proposal_schema().schema().digest(), "5", false, "1"),
            &mut replay,
            LifecycleBudget::default(),
            &AgentCancellation::new(),
        )
        .unwrap();
    assert_eq!(again.evidence(), run.evidence());
    assert_eq!(again.evidence_digest(), run.evidence_digest());
    assert!(!run.evidence().contains("alpha"));
    assert!(!run.evidence().contains("observed"));

    verify_agent_lifecycle_bundle(MODULE, MODULE_PATH, &source, first.canonical_json()).unwrap();
    let tampered =
        first
            .canonical_json()
            .replacen("\"kind\":\"deterministic\"", "\"kind\":\"model\"", 1);
    let error = verify_agent_lifecycle_bundle(MODULE, MODULE_PATH, &source, &tampered)
        .err()
        .unwrap();
    assert_eq!(error[0].code, "SPX-G572");
}

#[test]
fn the_authorization_is_bound_to_the_exact_policy_state_and_proposal() {
    let source = definition(&profile());
    let compiled = compile_agent_lifecycle(MODULE, MODULE_PATH, &source).unwrap();
    let digest = compiled.proposal_schema().schema().digest().to_owned();
    let binding = |task: LifecycleTask, budget: &str| {
        let mut read = Read::new();
        let run = compiled
            .run(
                &task,
                &proposal(&digest, budget, false, "1"),
                &mut read,
                LifecycleBudget::default(),
                &AgentCancellation::new(),
            )
            .unwrap();
        assert_eq!(run.status(), LifecycleStatus::Completed);
        run.authorization_binding().unwrap().to_owned()
    };

    let base = binding(task(), "5");
    assert!(base.starts_with("sha256:"));
    assert_eq!(base, binding(task(), "5"));
    // A different state and a different proposal each produce a different
    // authorization, so one grant never covers another state or proposal.
    assert_ne!(
        base,
        binding(
            LifecycleTask {
                objective: b"alpha".to_vec(),
                budget: 11,
            },
            "5"
        )
    );
    assert_ne!(
        base,
        binding(
            LifecycleTask {
                objective: b"beta".to_vec(),
                budget: 12,
            },
            "5"
        )
    );
    assert_ne!(base, binding(task(), "4"));

    // A different policy — one changed stage identity — restages every
    // authorization even though the state and proposal are identical.
    // A display rename alone is not a different policy: the document names
    // identities, never display names.
    let renamed = MODULE.replace("fn reduce(state: own State", "fn reduced(state: own State");
    let same = compile_agent_lifecycle(&renamed, MODULE_PATH, &source).unwrap();
    assert_eq!(same.digest(), compiled.digest());

    // A changed grant-seal identity is a different policy, so an otherwise
    // identical state and proposal authorize to a different value.
    let restaged = MODULE.replace(
        "fixture.agent.type.decision.granted.seal",
        "fixture.agent.type.decision.granted.witness",
    );
    let other = compile_agent_lifecycle(&restaged, MODULE_PATH, &source).unwrap();
    assert_ne!(other.digest(), compiled.digest());
    let mut read = Read::new();
    let run = other
        .run(
            &task(),
            &proposal(other.proposal_schema().schema().digest(), "5", false, "1"),
            &mut read,
            LifecycleBudget::default(),
            &AgentCancellation::new(),
        )
        .unwrap();
    assert_eq!(run.status(), LifecycleStatus::Completed);
    assert_ne!(run.authorization_binding().unwrap(), base);
}

#[test]
fn refusal_stale_proposals_and_effect_failure_are_distinct_terminals() {
    let source = definition(&profile());
    let compiled = compile_agent_lifecycle(MODULE, MODULE_PATH, &source).unwrap();
    let digest = compiled.proposal_schema().schema().digest().to_owned();

    // The authorize stage refuses: no authorization is minted and the injected
    // read operation is never reached.
    let mut read = Read::new();
    let run = compiled
        .run(
            &task(),
            &proposal(&digest, "5", true, "0"),
            &mut read,
            LifecycleBudget::default(),
            &AgentCancellation::new(),
        )
        .unwrap();
    assert_eq!(run.status(), LifecycleStatus::Rejected);
    assert_eq!(run.reason(), "authorize_refused");
    assert_eq!(run.refusal_code(), Some(2));
    assert_eq!(run.authorization_binding(), None);
    assert!(!run.authorization_spent());
    assert_eq!(read.calls, 0);

    // A proposal over the state's budget is refused for the other code.
    let mut read = Read::new();
    let run = compiled
        .run(
            &task(),
            &proposal(&digest, "99", false, "1"),
            &mut read,
            LifecycleBudget::default(),
            &AgentCancellation::new(),
        )
        .unwrap();
    assert_eq!(run.status(), LifecycleStatus::Rejected);
    assert_eq!(run.refusal_code(), Some(1));
    assert_eq!(read.calls, 0);

    // A stale grammar digest, a cross-agent proposal, a reordered document and
    // an out-of-range integer are model failures, all before any host work.
    let valid = proposal(&digest, "5", false, "1");
    for stale in [
        proposal(&"sha256:".to_owned().repeat(1), "5", false, "1"),
        valid.replacen("\"fixture.agent\"", "\"other.agent\"", 1),
        valid.replacen(
            "\"fixture.agent.type.proposal.budget\":\"5\",\"fixture.agent.type.proposal.urgent\":false",
            "\"fixture.agent.type.proposal.urgent\":false,\"fixture.agent.type.proposal.budget\":\"5\"",
            1,
        ),
        valid.replacen(
            "\"fixture.agent.type.proposal.sequence\":\"1\"",
            "\"fixture.agent.type.proposal.sequence\":\"-1\"",
            1,
        ),
        valid.trim_end().to_owned(),
    ] {
        let mut read = Read::new();
        let run = compiled
            .run(
                &task(),
                &stale,
                &mut read,
                LifecycleBudget::default(),
                &AgentCancellation::new(),
            )
            .unwrap();
        assert_eq!(run.status(), LifecycleStatus::ModelFailed, "{stale}");
        assert_eq!(run.authorization_binding(), None);
        assert_eq!(read.calls, 0);
        // The model failure happens after the two deterministic stages that
        // precede `propose`, and before the authorizing transition.
        assert_eq!(run.stages().len(), 2);
    }

    // The injected read operation fails: the authorization was spent and no
    // Result is published.
    let mut read = Read::failing();
    let run = compiled
        .run(
            &task(),
            &valid,
            &mut read,
            LifecycleBudget::default(),
            &AgentCancellation::new(),
        )
        .unwrap();
    assert_eq!(run.status(), LifecycleStatus::EffectFailed);
    assert_eq!(run.reason(), "execute_failed");
    assert_eq!(read.calls, 1);
    assert!(run.authorization_spent());
    assert!(run.result().is_none());
    assert_eq!(run.stages().len(), 3);
}

#[test]
fn cancellation_before_initialize_and_budget_exhaustion_are_fail_closed() {
    let source = definition(&profile());
    let compiled = compile_agent_lifecycle(MODULE, MODULE_PATH, &source).unwrap();
    let digest = compiled.proposal_schema().schema().digest().to_owned();
    let valid = proposal(&digest, "5", false, "1");

    // Cancellation is checked before `initialize`, before `observe`, before
    // `authorize` and before `execute`. Only the first of those four is
    // deterministically reachable from a caller that supplies the whole run at
    // once, so it is the one this gate executes; the other three are
    // implemented and unexercised.
    let cancellation = AgentCancellation::new();
    cancellation.cancel();
    let mut read = Read::new();
    let run = compiled
        .run(
            &task(),
            &valid,
            &mut read,
            LifecycleBudget::default(),
            &cancellation,
        )
        .unwrap();
    assert_eq!(run.status(), LifecycleStatus::Cancelled);
    assert_eq!(run.reason(), "cancelled_before_initialize");
    assert!(run.stages().is_empty());
    assert_eq!(read.calls, 0);
    assert!(run.evidence().contains("\"status\":\"cancelled\""));

    // Fuel is charged per stage, so the smallest budget stops the first stage
    // rather than producing a partial state.
    let mut read = Read::new();
    let run = compiled
        .run(
            &task(),
            &valid,
            &mut read,
            LifecycleBudget {
                max_steps_per_stage: 1,
            },
            &AgentCancellation::new(),
        )
        .unwrap();
    assert_eq!(run.status(), LifecycleStatus::BudgetExhausted);
    assert_eq!(run.stages().len(), 1);
    assert_eq!(run.stages()[0].outcome(), "fuel_exhausted");
    assert_eq!(read.calls, 0);
    assert_eq!(run.authorization_binding(), None);

    // A budget that admits `initialize` but not `observe` stops one stage
    // later, and still performs no host work.
    let mut stopped_later = None;
    for steps in 2..400 {
        let mut read = Read::new();
        let run = compiled
            .run(
                &task(),
                &valid,
                &mut read,
                LifecycleBudget {
                    max_steps_per_stage: steps,
                },
                &AgentCancellation::new(),
            )
            .unwrap();
        if run.status() == LifecycleStatus::BudgetExhausted && run.stages().len() == 2 {
            assert_eq!(read.calls, 0);
            stopped_later = Some(steps);
            break;
        }
    }
    assert!(stopped_later.is_some(), "no budget stops at `observe`");
}

#[test]
fn unresolved_incompatible_and_effectful_stages_are_rejected_before_any_run() {
    let source = definition(&profile());
    let cases: [(String, &str); 7] = [
        // An unresolved stage identity.
        (
            MODULE.replace(
                "@id(\"fixture.agent.fn.observe\")",
                "@id(\"fixture.agent.fn.observed\")",
            ),
            "observe.unresolved",
        ),
        // An incorrect ownership mode: the graph says `observe` borrows.
        (
            MODULE.replace("fn observe(state: borrow State)", "fn observe(state: own State)"),
            "observe.ownership",
        ),
        // An incorrect ownership mode: the graph says `reduce` consumes.
        (
            MODULE.replace("fn reduce(state: own State", "fn reduce(state: borrow State"),
            "reduce.ownership",
        ),
        // An incompatible result type.
        (
            MODULE.replace(
                concat!(
                    "fn initialize(task: own Task) -> State\n{\n",
                    "    State { objective: task.objective, budget: task.budget, epoch: 1 }\n}"
                ),
                concat!(
                    "fn initialize(task: own Task) -> Observation\n{\n",
                    "    let tag = [79u8, 66u8];\n",
                    "    Observation { tag: bytes_copy(array_as_slice(tag)), budget: task.budget, epoch: 1 }\n}"
                ),
            ),
            "initialize.result",
        ),
        // A deterministic stage that declares an effect.
        (
            MODULE
                .replace(
                    "module fixture.agent.lifecycle;",
                    "module fixture.agent.lifecycle;\n\npermit { process.stdout.write }",
                )
                .replace(
                    "fn observe(state: borrow State) -> Observation\n{",
                    "fn observe(state: borrow State) -> Observation\n    uses { process.stdout.write }\n{",
                ),
            "observe.effects",
        ),
        // A proposal field outside the stage projection's exact vocabulary.
        (
            MODULE.replace(
                "    @id(\"fixture.agent.type.proposal.sequence\")\n    sequence: usize,",
                "    @id(\"fixture.agent.type.proposal.sequence\")\n    sequence: string,",
            ),
            "proposal_type.field.representation",
        ),
        // A decision variant that is not a two-case grant/refusal.
        (
            MODULE.replace(
                "    @id(\"fixture.agent.type.decision.refused\")\n    Refused {",
                concat!(
                    "    @id(\"fixture.agent.type.decision.deferred\")\n",
                    "    Deferred {\n",
                    "        @id(\"fixture.agent.type.decision.deferred.at\")\n",
                    "        at: i64,\n",
                    "    },\n",
                    "    @id(\"fixture.agent.type.decision.refused\")\n    Refused {"
                ),
            ),
            "authorize.decision.cases",
        ),
    ];
    for (module, field) in cases {
        assert_ne!(module, MODULE, "the `{field}` fixture edit did not apply");
        let errors = match compile_agent_lifecycle(&module, MODULE_PATH, &source) {
            Ok(_) => panic!("`{field}` was admitted"),
            Err(errors) => errors,
        };
        assert!(
            errors.iter().any(|error| error.code == "SPX-G570"
                && error.message
                    == format!("AgentLifecycle stage binding invariant failed: {field}")),
            "{field}: {errors:?}"
        );
    }
}

#[test]
fn the_lifecycle_changes_no_frozen_agent_definition_byte() {
    let profile = profile();
    let source = definition(&profile);
    let compiled = compile_agent_definition(&source).unwrap();
    assert_eq!(
        compiled.definition().digest(),
        "sha256:82ab9abbeca5e209c36224d9cab3b7b6a7cdffc3b2fce5db73123fa7425965a0"
    );
    assert_eq!(
        compiled.graph().digest(),
        "sha256:0dc7ce1d50d43077042577cf6ac3dcfb5d2a744fb3acd2ca6cea12a6e296ff61"
    );
    assert_eq!(
        raw_sha(compiled.runtime_v1_profile()),
        "sha256:14981ee99af965dcea311121a90cacfb9891a00d6365e7ad00cab8cefe69c01a"
    );

    // Compiling and running a lifecycle over the same definition leaves every
    // one of those bytes alone.
    let lifecycle = compile_agent_lifecycle(MODULE, MODULE_PATH, &source).unwrap();
    let mut read = Read::new();
    lifecycle
        .run(
            &task(),
            &proposal(
                lifecycle.proposal_schema().schema().digest(),
                "5",
                false,
                "1",
            ),
            &mut read,
            LifecycleBudget::default(),
            &AgentCancellation::new(),
        )
        .unwrap();
    let after = compile_agent_definition(&source).unwrap();
    assert_eq!(after.definition().canonical_source(), source);
    assert_eq!(after.definition().digest(), compiled.definition().digest());
    assert_eq!(after.graph().digest(), compiled.graph().digest());
    assert_eq!(after.runtime_v1_profile(), profile);
    assert_eq!(
        lifecycle.definition_digest(),
        compiled.definition().digest()
    );
}
