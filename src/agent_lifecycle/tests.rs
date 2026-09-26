//! Crate-internal gates for the lifecycle's authority invariants.
//!
//! Two of them cannot be observed from outside the crate and are therefore
//! owned here: the single mint site of the authorization value, and the
//! refusal to spend an authorization into a state or proposal it was not
//! granted against. The public terminal behaviour, the stage binding
//! rejections and the frozen Runtime v1 digests are owned by the
//! `agent_runtime_v1` harness.

use super::*;
use crate::interpreter::retained_call::evaluate_retained_call;

mod lifecycle_parity;
/// Target-private scalar carrier parity (#182). Kept separate from the
/// lifecycle authority gates because it owns an independently bound source
/// profile and runs native/Core-Wasm artifacts.
mod target_scalar_carriers;
/// Multi-turn cross-engine conversation parity (#182/#143). Kept in its own
/// submodule rather than appended here: this file is already near the
/// repository's 1500-line module cap, and every gate in it that scans a
/// module's own text by `include_str!` keeps reading exactly the file it
/// audits.
mod turn_parity;
mod wasm_literal_edges;

/// The `runtime_v1` compatibility material of the frozen fixture definition.
/// The definition compiler supplies its own schema and nonclaims.
pub(in crate::agent_lifecycle) const RUNTIME_V1: &str = concat!(
    "{\"models\":[{\"provider_id\":\"fake.local\",\"model_id\":\"fake-basic\",",
    "\"locality\":\"local\",\"quality_tier\":\"basic\",\"tokenizer_id\":\"fake.bytes-v1\",",
    "\"max_context_tokens\":4096,\"input_usd_microunits_per_million_tokens\":0,",
    "\"output_usd_microunits_per_million_tokens\":0,\"capabilities\":[\"text\"]}],",
    "\"tools\":[{\"tool_id\":\"fixture.read\",\"description\":\"Return one bounded fixture value.\",",
    "\"arguments_schema\":{\"type\":\"object\",\"fields\":[{\"name\":\"query\",\"type\":\"string\",\"required\":true,\"max_bytes\":64}],\"additional_properties\":false},",
    "\"result_schema\":{\"type\":\"object\",\"fields\":[{\"name\":\"value\",\"type\":\"string\",\"required\":true,\"max_bytes\":64}],\"additional_properties\":false},",
    "\"effects\":[\"read\"],\"required_capabilities\":[\"tool.read\"]}],",
    "\"policy\":{\"allowed_provider_ids\":[\"fake.local\"],\"allowed_model_ids\":[\"fake-basic\"],",
    "\"required_locality\":\"local_only\",\"minimum_quality_tier\":\"basic\",",
    "\"required_model_capabilities\":[\"text\"],\"granted_capabilities\":[\"tool.read\"],",
    "\"allowed_tool_ids\":[\"fixture.read\"]},",
    "\"limits\":{\"max_turns\":2,\"max_provider_attempts\":2,\"max_retries_per_turn\":1,",
    "\"max_concurrency\":1,\"max_elapsed_ms\":1000,\"max_provider_request_bytes\":65536,",
    "\"max_provider_response_bytes\":4096,\"max_stream_chunks\":64,",
    "\"max_total_provider_input_bytes\":131072,\"max_total_provider_output_bytes\":8192,",
    "\"max_reported_model_input_tokens\":131072,\"max_reported_model_output_tokens\":8192,",
    "\"max_usd_microunits\":0,\"max_tool_calls\":1,\"max_tool_arguments_bytes\":4096,",
    "\"max_tool_result_bytes\":4096,\"max_total_tool_bytes\":8192,",
    "\"max_retained_state_bytes\":131072,\"max_trace_events\":64,\"max_trace_bytes\":131072,",
    "\"max_evidence_bytes\":262144,\"max_builder_bytes\":1048576}}"
);

pub(in crate::agent_lifecycle) const DEFINITION: &str = concat!(
    "{\"schema\":\"semaprax.agent-definition.v1\",\"agent_id\":\"fixture.agent\",",
    "\"types\":[",
    "{\"role\":\"task\",\"stable_id\":\"fixture.agent.type.task\"},",
    "{\"role\":\"state\",\"stable_id\":\"fixture.agent.type.state\"},",
    "{\"role\":\"observation\",\"stable_id\":\"fixture.agent.type.observation\"},",
    "{\"role\":\"proposal\",\"stable_id\":\"fixture.agent.type.proposal\"},",
    "{\"role\":\"outcome\",\"stable_id\":\"fixture.agent.type.outcome\"},",
    "{\"role\":\"result\",\"stable_id\":\"fixture.agent.type.result\"}],",
    "\"operations\":[",
    "{\"role\":\"initialize\",\"stable_id\":\"fixture.agent.fn.initialize\",\"kind\":\"deterministic\"},",
    "{\"role\":\"observe\",\"stable_id\":\"fixture.agent.fn.observe\",\"kind\":\"deterministic\"},",
    "{\"role\":\"propose\",\"stable_id\":\"fixture.agent.fn.propose\",\"kind\":\"model\"},",
    "{\"role\":\"authorize\",\"stable_id\":\"fixture.agent.fn.authorize\",\"kind\":\"deterministic\"},",
    "{\"role\":\"execute\",\"stable_id\":\"fixture.agent.fn.execute\",\"kind\":\"effect\"},",
    "{\"role\":\"reduce\",\"stable_id\":\"fixture.agent.fn.reduce\",\"kind\":\"deterministic\"}],",
    "\"runtime_v1\":RUNTIME}\n"
);

pub(in crate::agent_lifecycle) const MODULE: &str = r#"module fixture.agent.lifecycle;

@id("fixture.agent.type.task")
record Task {
    @id("fixture.agent.type.task.objective") objective: Bytes,
    @id("fixture.agent.type.task.budget") budget: i64,
}

@id("fixture.agent.type.state")
record State {
    @id("fixture.agent.type.state.objective") objective: Bytes,
    @id("fixture.agent.type.state.budget") budget: i64,
    @id("fixture.agent.type.state.epoch") epoch: i64,
}

@id("fixture.agent.type.observation")
record Observation {
    @id("fixture.agent.type.observation.tag") tag: Bytes,
    @id("fixture.agent.type.observation.budget") budget: i64,
    @id("fixture.agent.type.observation.epoch") epoch: i64,
}

@id("fixture.agent.type.proposal")
record Proposal {
    @id("fixture.agent.type.proposal.budget") budget: i64,
    @id("fixture.agent.type.proposal.urgent") urgent: bool,
    @id("fixture.agent.type.proposal.sequence") sequence: usize,
}

@id("fixture.agent.type.decision")
variant Decision {
    @id("fixture.agent.type.decision.granted") Granted {
        @id("fixture.agent.type.decision.granted.seal") seal: Bytes,
        @id("fixture.agent.type.decision.granted.budget") budget: i64,
    },
    @id("fixture.agent.type.decision.refused") Refused {
        @id("fixture.agent.type.decision.refused.code") code: i64,
    },
}

@id("fixture.agent.type.outcome")
record Outcome {
    @id("fixture.agent.type.outcome.value") value: Bytes,
    @id("fixture.agent.type.outcome.status") status: i64,
}

@id("fixture.agent.type.result")
record Report {
    @id("fixture.agent.type.result.summary") summary: Bytes,
    @id("fixture.agent.type.result.budget") budget: i64,
    @id("fixture.agent.type.result.status") status: i64,
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
    Observation { tag: bytes_copy(array_as_slice(tag)), budget: state.budget, epoch: state.epoch }
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

struct CountingRead(usize);

impl AgentReadOperation for CountingRead {
    fn read(&mut self, _: &AuthorizedRequest) -> Option<Vec<u8>> {
        self.0 += 1;
        Some(b"observed".to_vec())
    }
}

fn lifecycle() -> CompiledAgentLifecycle {
    compile_agent_lifecycle(
        MODULE,
        "agent-lifecycle-unit.spx",
        &DEFINITION.replace("RUNTIME", RUNTIME_V1),
    )
    .expect("the unit fixture binds every stage")
}

pub(in crate::agent_lifecycle) fn proposal(
    compiled: &CompiledAgentLifecycle,
    budget: &str,
    sequence: &str,
) -> String {
    format!(
        concat!(
            "{{\"schema\":\"semaprax.agent-proposal.v1\",\"agent_id\":\"fixture.agent\",",
            "\"proposal_schema_digest\":\"{digest}\",\"value\":{{\"fields\":{{",
            "\"fixture.agent.type.proposal.budget\":\"{budget}\",",
            "\"fixture.agent.type.proposal.urgent\":false,",
            "\"fixture.agent.type.proposal.sequence\":\"{sequence}\"}}}}}}\n"
        ),
        digest = compiled.proposal_schema().schema().digest(),
        budget = budget,
        sequence = sequence
    )
}

/// Runs `initialize` and then the authorizing transition, returning the state
/// it authorized against, the exact proposal document, and the grant.
fn authorize(
    compiled: &CompiledAgentLifecycle,
    budget: i64,
) -> (RetainedValue, String, Authorized) {
    let evaluation = authorization::dispatch(
        &compiled.program,
        compiled.binding.initialize.prepared(),
        &[payload(&compiled.binding.task, b"alpha".to_vec(), budget)],
        DEFAULT_STAGE_STEPS,
    )
    .expect("initialize evaluates");
    let RetainedCallOutcome::Returned(state) = evaluation.outcome else {
        panic!("initialize did not return a state");
    };
    let document = proposal(compiled, "5", "1");
    let decoded = compiled
        .proposal_schema()
        .decode(&document)
        .expect("the scripted proposal decodes");
    let projected = compiled.project(&decoded).expect("the projection is exact");
    let mut arguments = vec![state.clone()];
    arguments.extend(projected);
    let (outcome, _) = authorization::run_authorize_stage(
        &compiled.program,
        &compiled.binding.authorize,
        &arguments,
        DEFAULT_STAGE_STEPS,
        compiled.digest(),
        &state,
        decoded.canonical_json(),
    )
    .expect("the authorizing transition evaluates");
    let authorization::AuthorizationOutcome::Granted(authorized) = outcome else {
        panic!("the authorizing transition refused a satisfiable proposal");
    };
    (state, decoded.canonical_json().to_owned(), authorized)
}

#[test]
fn the_derived_stage_graph_is_acyclic_and_its_order_is_unique() {
    let order = stages::topological_order(&stages::ALL_ROLES, &stages::stage_edges())
        .expect("the derived stage graph is acyclic");
    assert_eq!(order, stages::ALL_ROLES.to_vec());

    // A cycle has no order at all.
    let mut cyclic = stages::stage_edges();
    cyclic.push(stages::StageEdge {
        from: "reduce",
        to: "initialize",
    });
    assert!(stages::topological_order(&stages::ALL_ROLES, &cyclic).is_none());

    // So does an ambiguous graph: two independent stages would leave the
    // execution order undetermined rather than deterministic.
    let ambiguous = [stages::StageEdge {
        from: "initialize",
        to: "observe",
    }];
    assert!(stages::topological_order(&["initialize", "observe", "reduce"], &ambiguous).is_none());
}

#[test]
fn an_authorization_is_not_spendable_into_a_substituted_state_or_proposal() {
    let compiled = lifecycle();

    // The authorization the validated authorize stage granted spends exactly
    // once, into the state and proposal it was granted against.
    let (state, document, authorized) = authorize(&compiled, 12);
    let mut read = CountingRead(0);
    let value = compiled
        .spend(authorized, &state, &document, &mut read)
        .expect("the bound authorization spends");
    assert_eq!(value, b"observed".to_vec());
    assert_eq!(read.0, 1);

    // A substituted state: an authorization granted against one state does not
    // admit an effect against another, and the read operation is never reached.
    let (_, _, authorized) = authorize(&compiled, 12);
    let (other_state, _, _) = authorize(&compiled, 11);
    let mut read = CountingRead(0);
    let error = compiled
        .spend(authorized, &other_state, &document, &mut read)
        .expect_err("a substituted state is refused");
    assert_eq!(error.code, "SPX-G571");
    assert!(error
        .message
        .contains("not bound to this state and proposal"));
    assert_eq!(read.0, 0);

    // A substituted proposal is refused the same way.
    let (state, _, authorized) = authorize(&compiled, 12);
    let stale = proposal(&compiled, "4", "1");
    assert_ne!(stale, document);
    let mut read = CountingRead(0);
    let error = compiled
        .spend(authorized, &state, &stale, &mut read)
        .expect_err("a substituted proposal is refused");
    assert_eq!(error.code, "SPX-G571");
    assert_eq!(read.0, 0);
}

#[test]
fn the_authorization_binding_separates_policy_state_proposal_case_and_seal() {
    let compiled = lifecycle();
    let (state, document, authorized) = authorize(&compiled, 12);
    let case = compiled.binding.authorize.grant_case();
    let base = authorization::binding(compiled.digest(), &state, &document, case, b"AZ");
    assert_eq!(base, authorized.binding());

    let (other_state, _, _) = authorize(&compiled, 11);
    for candidate in [
        authorization::binding("sha256:0", &state, &document, case, b"AZ"),
        authorization::binding(compiled.digest(), &other_state, &document, case, b"AZ"),
        authorization::binding(
            compiled.digest(),
            &state,
            &proposal(&compiled, "4", "1"),
            case,
            b"AZ",
        ),
        authorization::binding(
            compiled.digest(),
            &state,
            &document,
            compiled.binding.authorize.refuse_case(),
            b"AZ",
        ),
        authorization::binding(compiled.digest(), &state, &document, case, b"ZA"),
    ] {
        assert_ne!(base, candidate);
    }
}

#[test]
fn the_authorization_value_has_exactly_one_mint_site_in_the_crate() {
    let authorization = include_str!("authorization.rs");
    let lifecycle = include_str!("../agent_lifecycle.rs");
    let stages = include_str!("stages.rs");
    let durable = include_str!("durable.rs");
    let checkpoint = include_str!("durable/checkpoint.rs");
    let journal = include_str!("durable/journal.rs");

    // The struct literal that builds the value exists exactly once, and its
    // fields are private to the authorization module, so no other module can
    // name them even inside this crate.
    assert_eq!(authorization.matches("\n    Authorized {").count(), 1);
    assert_eq!(lifecycle.matches("Authorized {").count(), 0);
    assert_eq!(stages.matches("Authorized {").count(), 0);
    // The durable path adds no second route: it never names the struct, never
    // names the mint, and reaches an authorization only by running the same
    // validated authorizing transition once.
    for (name, source) in [
        ("durable.rs", durable),
        ("durable/checkpoint.rs", checkpoint),
        ("durable/journal.rs", journal),
    ] {
        assert_eq!(source.matches("Authorized {").count(), 0, "{name}");
        assert_eq!(source.matches("mint(").count(), 0, "{name}");
    }
    assert_eq!(durable.matches("run_authorize_stage(").count(), 1);
    assert!(authorization.contains("pub struct Authorized {\n    binding: String,"));

    // The mint is private and is called exactly once, from the function that
    // runs the validated authorizing transition.
    assert!(authorization.contains("\nfn mint("));
    assert!(!authorization.contains("pub fn mint("));
    assert!(!authorization.contains("pub(super) fn mint("));
    assert!(!authorization.contains("pub(crate) fn mint("));
    assert_eq!(
        authorization.matches("mint(binding, budget, seal)").count(),
        1
    );
    let (_, after) = authorization
        .split_once("pub(super) fn run_authorize_stage(")
        .expect("the mint's only caller is the authorize-stage runner");
    assert!(after.contains("mint(binding, budget, seal)"));

    // The value derives nothing: it is not `Clone`, not `Copy`, and has no
    // `Default`, so one grant admits at most one effect.
    // Audit the authorization carriers, not the copyable backend selector.
    let carriers = authorization
        .split_once("/// The single mint site")
        .unwrap()
        .0;
    assert!(!carriers.contains("#[derive"));
    assert!(!authorization.contains("impl Clone for Authorized"));
    assert!(!authorization.contains("impl Default for Authorized"));

    // Nothing in the lifecycle reaches a host, a process or the environment.
    for forbidden in [
        "std::net::",
        "std::env::",
        "TcpStream",
        "Command::new",
        "fs::write",
        "fs::read",
        "File::create",
    ] {
        for (name, source) in [
            ("authorization.rs", authorization),
            ("agent_lifecycle.rs", lifecycle),
            ("stages.rs", stages),
            ("durable.rs", durable),
            ("durable/checkpoint.rs", checkpoint),
            ("durable/journal.rs", journal),
        ] {
            assert!(!source.contains(forbidden), "{name} contains {forbidden}");
        }
    }
}

#[test]
fn the_stage_executor_seam_has_exactly_three_implementations_and_one_dispatch_route() {
    let authorization = include_str!("authorization.rs");
    let native_executor = include_str!("authorization/native_executor.rs");
    let wasm_executor = include_str!("authorization/wasm_executor.rs");
    let lifecycle = include_str!("../agent_lifecycle.rs");
    let stages = include_str!("stages.rs");
    let durable = include_str!("durable.rs");
    let checkpoint = include_str!("durable/checkpoint.rs");
    let journal = include_str!("durable/journal.rs");
    let rich_stage = include_str!("rich_stage.rs");
    let driver = include_str!("iterative/driver.rs");
    let live = include_str!("iterative/driver/live.rs");

    // `StageExecutor` is implemented exactly three times in the whole tree:
    // the interpreter-backed executor here in `authorization.rs`, the
    // native C11 executor (#142) in `authorization/native_executor.rs`, and
    // the Core Wasm executor (#143) in `authorization/wasm_executor.rs`.
    // Extending this count from one to three is the deliberate, reviewed
    // outcome of admitting those two backends -- not a regression of the
    // seal. `sealed::Sealed`, defined once in a private `mod sealed` nested
    // in `authorization.rs`, is visible only to `authorization.rs` and the
    // two submodules it declares, so nothing outside this module's own tree
    // could add a fourth implementation even if it tried; the compiler, not
    // this scan, is what actually enforces that (see the `compile_fail`
    // doctest on `StageExecutor` itself).
    assert_eq!(authorization.matches("impl StageExecutor for").count(), 1);
    assert_eq!(native_executor.matches("impl StageExecutor for").count(), 1);
    assert_eq!(wasm_executor.matches("impl StageExecutor for").count(), 1);
    assert_eq!(authorization.matches("mod sealed").count(), 1);
    for (name, source) in [
        ("agent_lifecycle.rs", lifecycle),
        ("stages.rs", stages),
        ("durable.rs", durable),
        ("durable/checkpoint.rs", checkpoint),
        ("durable/journal.rs", journal),
        ("rich_stage.rs", rich_stage),
        ("iterative/driver.rs", driver),
        ("iterative/driver/live.rs", live),
    ] {
        assert_eq!(
            source.matches("impl StageExecutor for").count(),
            0,
            "{name}"
        );
        assert_eq!(source.matches("mod sealed").count(), 0, "{name}");
    }
    for (name, source) in [
        ("authorization/native_executor.rs", native_executor),
        ("authorization/wasm_executor.rs", wasm_executor),
    ] {
        assert_eq!(source.matches("mod sealed").count(), 0, "{name}");
    }

    // Every stage dispatch this crate's `src/agent_lifecycle/**` file lease
    // can reach now calls the sealed `dispatch`/`dispatch_on` instead of
    // `evaluate_retained_call` directly. The Rich Proposal binder used to
    // call it twice (authorize and reduce); it now calls it zero times. The
    // native and Wasm executors never call it at all -- they compile and
    // run the stage body through their own backend instead.
    assert_eq!(authorization.matches("evaluate_retained_call(").count(), 1);
    for (name, source) in [
        ("rich_stage.rs", rich_stage),
        ("durable.rs", durable),
        ("iterative/driver.rs", driver),
        ("iterative/driver/live.rs", live),
        ("stages.rs", stages),
        ("authorization/native_executor.rs", native_executor),
        ("authorization/wasm_executor.rs", wasm_executor),
    ] {
        assert_eq!(
            source.matches("evaluate_retained_call(").count(),
            0,
            "{name}"
        );
    }

    // The module-root `CompiledAgentLifecycle::evaluate` was the last
    // bypass: it called `evaluate_retained_call` directly, outside the
    // seam. It now routes through `authorization::dispatch` like every
    // other stage execution, so the count here is zero. If it ever becomes
    // non-zero again a second, unsealed execution route has reappeared.
    assert_eq!(lifecycle.matches("evaluate_retained_call(").count(), 0);
}

#[test]
fn the_sealed_dispatch_reaches_the_interpreter_and_matches_its_direct_evaluation() {
    let compiled = lifecycle();
    let arguments = [payload(&compiled.binding.task, b"alpha".to_vec(), 5)];

    let direct = evaluate_retained_call(
        &compiled.program,
        compiled.binding.initialize.prepared(),
        &arguments,
        DEFAULT_STAGE_STEPS,
    )
    .expect("the interpreter evaluates initialize directly");
    let sealed = authorization::dispatch(
        &compiled.program,
        compiled.binding.initialize.prepared(),
        &arguments,
        DEFAULT_STAGE_STEPS,
    )
    .expect("the sealed executor evaluates initialize");

    // The seam is not a second, drifting implementation: it reaches the same
    // interpreter and returns the identical decoded outcome.
    assert_eq!(direct.outcome, sealed.outcome);
    let RetainedCallOutcome::Returned(state) = sealed.outcome else {
        panic!("initialize did not return a state through the sealed seam");
    };
    assert!(compiled.carries(&state, "state"));
}

// ---------------------------------------------------------------------------
// #142 / #143: NativeStageExecutor and WasmStageExecutor, dispatched only
// through `authorization::dispatch_on`.
// ---------------------------------------------------------------------------

fn native_wasm_tools_available() -> bool {
    native_stage_host().is_some()
        && std::process::Command::new("node")
            .arg("--version")
            .output()
            .is_ok_and(|output| output.status.success())
}

/// Test-only host fixture for the explicitly held native compiler capability.
/// These are literal, reviewable absolute paths; native stage execution does
/// not discover `clang` through PATH. CI that installs it elsewhere can set
/// one explicit fixture path without changing production authority.
pub(in crate::agent_lifecycle) fn native_stage_host() -> Option<authorization::NativeStageHost> {
    if let Some(path) =
        std::env::var_os("SEMAPRAX_TEST_NATIVE_STAGE_CLANG").map(std::path::PathBuf::from)
    {
        return Some(
            authorization::NativeStageHost::open(&path).unwrap_or_else(|error| {
                panic!(
                    "configured native stage compiler {} was refused: {error:?}",
                    path.display()
                )
            }),
        );
    }
    [
        std::path::PathBuf::from("/usr/bin/clang"),
        std::path::PathBuf::from("/usr/local/bin/clang"),
        std::path::PathBuf::from("/opt/homebrew/opt/llvm/bin/clang"),
    ]
    .into_iter()
    .find_map(|path| authorization::NativeStageHost::open(&path).ok())
}

pub(super) fn test_wasm_stage_host() -> &'static authorization::WasmStageHost {
    static HOST: std::sync::OnceLock<authorization::WasmStageHost> = std::sync::OnceLock::new();
    HOST.get_or_init(|| {
        std::env::var_os("SEMAPRAX_TEST_WASM_STAGE_NODE")
            .map(std::path::PathBuf::from)
            .into_iter()
            .chain([
                std::path::PathBuf::from("/usr/bin/node"),
                std::path::PathBuf::from("/usr/local/bin/node"),
                std::path::PathBuf::from("/opt/homebrew/bin/node"),
            ])
            .find_map(|path| authorization::WasmStageHost::open(&path).ok())
            .expect("Core Wasm tests require an explicit absolute Node fixture path")
    })
}

pub(in crate::agent_lifecycle) fn native_backend(
    host: &authorization::NativeStageHost,
) -> authorization::StageBackend<'_> {
    authorization::StageBackend::Native { host }
}

pub(in crate::agent_lifecycle) fn native_o2_backend(
    host: &authorization::NativeStageHost,
) -> authorization::StageBackend<'_> {
    authorization::StageBackend::NativeAtOptimization {
        host,
        optimization: "-O2",
    }
}

/// Parity evidence for #142: the native C11 executor agrees with the
/// interpreter on every deterministic stage of the same lifecycle fixture
/// `authorize` above shares -- `initialize`, `observe`, both branches of
/// `authorize` (granted and refused), and `reduce` -- not merely that both
/// happen to succeed.
#[test]
fn native_executor_agrees_with_the_interpreter_on_every_deterministic_stage() {
    if !native_wasm_tools_available() {
        eprintln!("skipping native executor parity: clang or node unavailable");
        return;
    }
    let native_host = native_stage_host().expect("availability retains native host");
    let compiled = lifecycle();

    let task = payload(&compiled.binding.task, b"alpha".to_vec(), 10);
    let interpreter_initialize = authorization::dispatch_on(
        authorization::StageBackend::Interpreter,
        &compiled.program,
        compiled.binding.initialize.prepared(),
        std::slice::from_ref(&task),
        DEFAULT_STAGE_STEPS,
    )
    .expect("interpreter evaluates initialize");
    let native_initialize = authorization::dispatch_on(
        native_backend(&native_host),
        &compiled.program,
        compiled.binding.initialize.prepared(),
        std::slice::from_ref(&task),
        DEFAULT_STAGE_STEPS,
    )
    .expect("native evaluates initialize");
    assert_eq!(interpreter_initialize.outcome, native_initialize.outcome);
    let RetainedCallOutcome::Returned(state) = interpreter_initialize.outcome else {
        panic!("initialize did not return a state");
    };

    let interpreter_observe = authorization::dispatch_on(
        authorization::StageBackend::Interpreter,
        &compiled.program,
        compiled.binding.observe.prepared(),
        std::slice::from_ref(&state),
        DEFAULT_STAGE_STEPS,
    )
    .expect("interpreter evaluates observe");
    let native_observe = authorization::dispatch_on(
        native_backend(&native_host),
        &compiled.program,
        compiled.binding.observe.prepared(),
        std::slice::from_ref(&state),
        DEFAULT_STAGE_STEPS,
    )
    .expect("native evaluates observe");
    assert_eq!(interpreter_observe.outcome, native_observe.outcome);

    // `authorize`: both the grant branch and the refusal branch, so
    // agreement is never proven by a single happy path alone.
    for (budget, sequence, label) in [(5i64, 1u64, "granted"), (50i64, 1u64, "refused")] {
        let arguments = [
            state.clone(),
            RetainedValue::I64(budget),
            RetainedValue::Bool(false),
            RetainedValue::Usize(sequence),
        ];
        let interpreter = authorization::dispatch_on(
            authorization::StageBackend::Interpreter,
            &compiled.program,
            compiled.binding.authorize.stage().prepared(),
            &arguments,
            DEFAULT_STAGE_STEPS,
        )
        .unwrap_or_else(|error| panic!("interpreter evaluates authorize ({label}): {error:?}"));
        let native = authorization::dispatch_on(
            native_backend(&native_host),
            &compiled.program,
            compiled.binding.authorize.stage().prepared(),
            &arguments,
            DEFAULT_STAGE_STEPS,
        )
        .unwrap_or_else(|error| panic!("native evaluates authorize ({label}): {error:?}"));
        assert_eq!(interpreter.outcome, native.outcome, "authorize {label}");
    }

    let outcome = payload(&compiled.binding.outcome, b"observed".to_vec(), 4);
    let reduce_arguments = [
        state.clone(),
        RetainedValue::I64(3),
        RetainedValue::Bool(false),
        RetainedValue::Usize(1),
        outcome,
    ];
    let interpreter_reduce = authorization::dispatch_on(
        authorization::StageBackend::Interpreter,
        &compiled.program,
        compiled.binding.reduce.prepared(),
        &reduce_arguments,
        DEFAULT_STAGE_STEPS,
    )
    .expect("interpreter evaluates reduce");
    let native_reduce = authorization::dispatch_on(
        native_backend(&native_host),
        &compiled.program,
        compiled.binding.reduce.prepared(),
        &reduce_arguments,
        DEFAULT_STAGE_STEPS,
    )
    .expect("native evaluates reduce");
    assert_eq!(interpreter_reduce.outcome, native_reduce.outcome);
}

/// Recovery without repeated effects: every deterministic Agent stage body
/// this crate ever binds is provably pure (`stages.rs::function` refuses any
/// declared effect on one), and `NativeStageExecutor` keeps no durable state
/// of its own between calls (a fresh temporary directory and a fresh
/// out-of-process compile and run every time, removed before returning).
/// Replaying the same dispatch -- exactly what a crash-recovery retry does --
/// can therefore never repeat a side effect, because there is not one to
/// repeat, and it settles to the identical decoded value every time.
#[test]
fn native_executor_replays_the_same_dispatch_without_repeating_any_effect() {
    if !native_wasm_tools_available() {
        eprintln!("skipping native executor replay: clang or node unavailable");
        return;
    }
    let native_host = native_stage_host().expect("availability retains native host");
    let compiled = lifecycle();
    let task = payload(&compiled.binding.task, b"alpha".to_vec(), 10);

    let first = authorization::dispatch_on(
        native_backend(&native_host),
        &compiled.program,
        compiled.binding.initialize.prepared(),
        std::slice::from_ref(&task),
        DEFAULT_STAGE_STEPS,
    )
    .expect("the first native dispatch evaluates initialize");
    let second = authorization::dispatch_on(
        native_backend(&native_host),
        &compiled.program,
        compiled.binding.initialize.prepared(),
        std::slice::from_ref(&task),
        DEFAULT_STAGE_STEPS,
    )
    .expect("a replayed native dispatch evaluates initialize identically");
    assert_eq!(first.outcome, second.outcome);
}

/// Wrong ProgramRoot / wrong entry refusal, for both backends: a prepared
/// call whose named entry is absent from the program it is dispatched
/// against is refused before any compile or run is attempted, not silently
/// evaluated against a different function that happens to share a slot.
#[test]
fn native_and_interpreter_executors_refuse_a_prepared_call_whose_entry_is_absent_from_the_given_program(
) {
    if !native_wasm_tools_available() {
        eprintln!("skipping native executor wrong-entry refusal: clang or node unavailable");
        return;
    }
    let native_host = native_stage_host().expect("availability retains native host");
    let compiled = lifecycle();
    let other = hir::resolve(
        &crate::parse(
            "module test.native_executor_wrong_program;\n@id(\"app.main\") fn main() -> i64 { 0 }\n",
            std::path::Path::new("native-executor-wrong-program.spx"),
        )
        .expect("the unrelated fixture parses"),
    )
    .expect("the unrelated fixture resolves");
    let task = payload(&compiled.binding.task, b"alpha".to_vec(), 10);

    let native_error = authorization::dispatch_on(
        native_backend(&native_host),
        &other,
        compiled.binding.initialize.prepared(),
        std::slice::from_ref(&task),
        DEFAULT_STAGE_STEPS,
    )
    .expect_err("native must refuse a prepared call for an entry absent from the given program");
    assert_eq!(native_error.len(), 1);
    assert_eq!(native_error[0].code, "SPX-G570");

    let interpreter_error = authorization::dispatch_on(
        authorization::StageBackend::Interpreter,
        &other,
        compiled.binding.initialize.prepared(),
        std::slice::from_ref(&task),
        DEFAULT_STAGE_STEPS,
    )
    .expect_err(
        "the interpreter must likewise refuse the same wrong-ProgramRoot dispatch, not just native",
    );
    assert!(!interpreter_error.is_empty());
}

/// Malformed-argument settlement: an argument whose runtime shape does not
/// match its declared parameter type is refused before any C is generated
/// or compiled, not coerced or silently miscompiled.
#[test]
fn native_executor_refuses_a_malformed_argument_shape_before_any_compile() {
    if !native_wasm_tools_available() {
        eprintln!("skipping native executor malformed-argument refusal: clang or node unavailable");
        return;
    }
    let native_host = native_stage_host().expect("availability retains native host");
    let compiled = lifecycle();

    // Wrong arity.
    let arity_error = authorization::dispatch_on(
        native_backend(&native_host),
        &compiled.program,
        compiled.binding.initialize.prepared(),
        &[],
        DEFAULT_STAGE_STEPS,
    )
    .expect_err("initialize takes one argument, not zero");
    assert_eq!(arity_error[0].code, "SPX-G570");

    // Right arity, wrong runtime shape: `observe` takes one borrowed
    // `State` record, not a bare `i64`.
    let shape_error = authorization::dispatch_on(
        native_backend(&native_host),
        &compiled.program,
        compiled.binding.observe.prepared(),
        &[RetainedValue::I64(0)],
        DEFAULT_STAGE_STEPS,
    )
    .expect_err("observe's state argument must be a State record, not a scalar");
    assert_eq!(shape_error[0].code, "SPX-G570");
}

/// A result shape outside this executor's closed, reviewed vocabulary
/// (neither a record nor a variant built only from `Bytes`/`i64` leaves) is
/// refused with a diagnostic, never guessed at or partially decoded.
#[test]
fn native_executor_refuses_a_result_shape_outside_its_closed_vocabulary() {
    if !native_wasm_tools_available() {
        eprintln!("skipping native executor result-shape refusal: clang or node unavailable");
        return;
    }
    let native_host = native_stage_host().expect("availability retains native host");
    let program = hir::resolve(
        &crate::parse(
            "module test.native_executor_scalar_result;\n@id(\"scalar.fn\") fn scalar_result() -> i64 { 0 }\n@id(\"app.main\") fn main() -> i64 { 0 }\n",
            std::path::Path::new("native-executor-scalar-result.spx"),
        )
        .expect("the scalar-result fixture parses"),
    )
    .expect("the scalar-result fixture resolves");
    let prepared = crate::interpreter::retained_call::prepare_retained_call(&program, "scalar.fn")
        .expect("the scalar-result function prepares");

    let error = authorization::dispatch_on(
        native_backend(&native_host),
        &program,
        &prepared,
        &[],
        DEFAULT_STAGE_STEPS,
    )
    .expect_err("a bare scalar result is outside this executor's record/variant vocabulary");
    assert_eq!(error.len(), 1);
    assert_eq!(error[0].code, "SPX-G570");
}

/// The invariant the previous `wasm_executor_fails_closed_on_every_real_bound_stage_shape`
/// actually protected, kept executable now that the executor no longer fails
/// closed: the owned-data arena's own admission rule
/// (`src/project/public_api.rs::parameter_type`, which accepts only
/// `i64`/`bool` by value and `borrow Str`/`borrow SliceU8`) is NOT widened to
/// let an Agent stage through. Handing a real bound stage's record-taking
/// signature straight to `derive_public_api_descriptor` must still be
/// refused. The Wasm executor reaches Wasm by injecting a checked
/// zero-parameter driver whose own signature the arena already admits -- not
/// by relaxing a backend's verification.
#[test]
fn the_owned_data_arena_still_refuses_a_bound_stage_signature_directly() {
    let compiled = lifecycle();
    const FACT: &str = "sha256:1111111111111111111111111111111111111111111111111111111111111111";
    for role in [
        "fixture.agent.fn.initialize",
        "fixture.agent.fn.observe",
        "fixture.agent.fn.authorize",
        "fixture.agent.fn.reduce",
    ] {
        let subject = crate::project::PublicApiSubject {
            project_schema: crate::project::PUBLIC_OWNED_DATA_PROJECT_SCHEMA,
            project_revision: FACT,
            workspace_revision: FACT,
            project_graph_digest: FACT,
        };
        assert!(
            crate::project::derive_public_api_descriptor(
                &compiled.program,
                &[role.to_owned()],
                subject,
            )
            .is_err(),
            "{role} takes an own/borrow record and must stay outside the arena's \
             parameter vocabulary",
        );
    }
}

/// Parity evidence for #182/#143: the SAME source program, the SAME
/// arguments and the SAME decoded `RetainedCallOutcome` on four executor
/// legs -- interpreter, native C11 at `-O0`, native C11 at `-O2`, and Core
/// Wasm -- for every deterministic stage the fixture binds, including both
/// branches of `authorize` so agreement is never established by a single
/// happy path. Both native optimization levels are exercised deliberately:
/// an optimizer is exactly where backend divergence hides, and `-O0` alone
/// never exercises it.
///
/// The Core Wasm leg is a real module: the stage body is compiled into a Core
/// Wasm owned-data package and executed under a real `node`/V8 host through
/// `project::prepare_owned_data_npm_build`. It is not the interpreter wrapped
/// in a Wasm-shaped name.
///
/// What this does NOT claim: no budget, cancellation, effect-dispatch or
/// evidence loop runs on native or Wasm here, `steps_used` is not comparable
/// across engines (native and Wasm both report `0`), and this is local,
/// re-runnable evidence on macOS arm64 requiring `clang` and `node` -- not a
/// hosted run, a browser run, or a production-support claim.
#[test]
fn every_stage_executor_agrees_on_every_deterministic_stage_of_one_source_program() {
    if !native_wasm_tools_available() {
        eprintln!("skipping four-leg stage parity: clang or node unavailable");
        return;
    }
    let native_host = native_stage_host().expect("availability retains native host");
    let compiled = lifecycle();
    // The Wasm backend re-resolves exactly the module source the caller
    // supplies -- here the same `MODULE` text `lifecycle()` compiled.
    // `CompiledAgentLifecycle::source` is NOT that text: it is the rendered
    // lifecycle document, so the module source has to be threaded in
    // explicitly rather than recovered from the compiled product.
    let source = MODULE;
    let mut wasm_dispatches = 0usize;
    let mut native_o0_dispatches = 0usize;
    let mut native_o2_dispatches = 0usize;

    let mut compare = |label: &str,
                       prepared: &crate::interpreter::retained_call::PreparedRetainedCall,
                       arguments: &[RetainedValue]|
     -> RetainedCallOutcome {
        let interpreter = authorization::dispatch_on(
            authorization::StageBackend::Interpreter,
            &compiled.program,
            prepared,
            arguments,
            DEFAULT_STAGE_STEPS,
        )
        .unwrap_or_else(|error| panic!("interpreter evaluates {label}: {error:?}"));
        let native_o0 = authorization::dispatch_on(
            native_backend(&native_host),
            &compiled.program,
            prepared,
            arguments,
            DEFAULT_STAGE_STEPS,
        )
        .unwrap_or_else(|error| panic!("native -O0 evaluates {label}: {error:?}"));
        native_o0_dispatches += 1;
        let native_o2 = authorization::dispatch_on(
            native_o2_backend(&native_host),
            &compiled.program,
            prepared,
            arguments,
            DEFAULT_STAGE_STEPS,
        )
        .unwrap_or_else(|error| panic!("native -O2 evaluates {label}: {error:?}"));
        native_o2_dispatches += 1;
        let wasm = authorization::dispatch_on(
            authorization::StageBackend::Wasm { source },
            &compiled.program,
            prepared,
            arguments,
            DEFAULT_STAGE_STEPS,
        )
        .unwrap_or_else(|error| panic!("Core Wasm evaluates {label}: {error:?}"));
        wasm_dispatches += 1;
        assert_eq!(
            interpreter.outcome, native_o0.outcome,
            "{label}: native -O0"
        );
        assert_eq!(
            interpreter.outcome, native_o2.outcome,
            "{label}: native -O2"
        );
        assert_eq!(interpreter.outcome, wasm.outcome, "{label}: Core Wasm");
        interpreter.outcome
    };

    let task = payload(&compiled.binding.task, b"alpha".to_vec(), 10);
    let initialized = compare(
        "initialize",
        compiled.binding.initialize.prepared(),
        std::slice::from_ref(&task),
    );
    let RetainedCallOutcome::Returned(state) = initialized else {
        panic!("initialize did not return a state");
    };

    compare(
        "observe",
        compiled.binding.observe.prepared(),
        std::slice::from_ref(&state),
    );

    for (budget, label) in [(5i64, "authorize granted"), (50i64, "authorize refused")] {
        let arguments = [
            state.clone(),
            RetainedValue::I64(budget),
            RetainedValue::Bool(false),
            RetainedValue::Usize(1),
        ];
        let decision = compare(
            label,
            compiled.binding.authorize.stage().prepared(),
            &arguments,
        );
        let RetainedCallOutcome::Returned(RetainedValue::Variant(variant)) = decision else {
            panic!("{label} did not return a Decision variant");
        };
        // The branch actually taken is asserted, so "both engines agreed" can
        // never be satisfied by both taking the same wrong branch silently.
        let expected = if budget == 5 {
            "fixture.agent.type.decision.granted"
        } else {
            "fixture.agent.type.decision.refused"
        };
        assert_eq!(variant.case.as_str(), expected, "{label}: case");
    }

    let outcome = payload(&compiled.binding.outcome, b"observed".to_vec(), 4);
    compare(
        "reduce",
        compiled.binding.reduce.prepared(),
        &[
            state.clone(),
            RetainedValue::I64(3),
            RetainedValue::Bool(false),
            RetainedValue::Usize(1),
            outcome,
        ],
    );

    // Positive proof every leg really ran rather than being skipped or
    // vacuously matching zero cases: five stage dispatches on each of the
    // three non-interpreter legs (native -O0, native -O2, Core Wasm), each
    // of which actually compiled and executed a fresh artifact.
    assert_eq!(native_o0_dispatches, 5);
    assert_eq!(native_o2_dispatches, 5);
    assert_eq!(wasm_dispatches, 5);
    eprintln!(
        "four-leg stage parity: {native_o0_dispatches} native -O0, \
         {native_o2_dispatches} native -O2, {wasm_dispatches} Core Wasm \
         dispatches, each compared against the interpreter"
    );
}

/// Wrong ProgramRoot / wrong entry, and a malformed argument shape, are
/// refused by the Wasm executor before any source is synthesized, any module
/// is built, or any Node process is started.
#[test]
fn wasm_executor_refuses_a_wrong_entry_and_a_malformed_argument_before_building_anything() {
    let compiled = lifecycle();
    let source = MODULE;
    let other = hir::resolve(
        &crate::parse(
            "module test.wasm_executor_wrong_program;\n@id(\"app.main\") fn main() -> i64 { 0 }\n",
            std::path::Path::new("wasm-executor-wrong-program.spx"),
        )
        .expect("the unrelated fixture parses"),
    )
    .expect("the unrelated fixture resolves");
    let task = payload(&compiled.binding.task, b"alpha".to_vec(), 10);

    let absent = authorization::dispatch_on(
        authorization::StageBackend::Wasm { source },
        &other,
        compiled.binding.initialize.prepared(),
        std::slice::from_ref(&task),
        DEFAULT_STAGE_STEPS,
    )
    .expect_err("Wasm must refuse a prepared call for an entry absent from the given program");
    assert_eq!(absent.len(), 1);
    assert_eq!(absent[0].code, "SPX-G570");

    let arity = authorization::dispatch_on(
        authorization::StageBackend::Wasm { source },
        &compiled.program,
        compiled.binding.initialize.prepared(),
        &[],
        DEFAULT_STAGE_STEPS,
    )
    .expect_err("initialize takes one argument, not zero");
    assert_eq!(arity[0].code, "SPX-G570");

    let shape = authorization::dispatch_on(
        authorization::StageBackend::Wasm { source },
        &compiled.program,
        compiled.binding.observe.prepared(),
        &[RetainedValue::I64(0)],
        DEFAULT_STAGE_STEPS,
    )
    .expect_err("observe's state argument must be a State record, not a scalar");
    assert_eq!(shape[0].code, "SPX-G570");
}

/// Stale-source fail-closed: the Wasm executor re-resolves the source it is
/// handed and compares the re-derived entry against the `ResolvedFunction` it
/// was given. Source that is not the program's own text is refused rather
/// than silently executing a different body under the stage's name.
#[test]
fn wasm_executor_refuses_source_that_is_not_the_program_it_was_handed() {
    let compiled = lifecycle();
    // Same declarations, but one stage body changed: re-resolution succeeds
    // and yields a DIFFERENT `ResolvedFunction` for the same `@id`.
    let drifted = MODULE.replace("epoch: 1 }", "epoch: 2 }");
    assert_ne!(drifted, MODULE);
    let task = payload(&compiled.binding.task, b"alpha".to_vec(), 10);

    let error = authorization::dispatch_on(
        authorization::StageBackend::Wasm { source: &drifted },
        &compiled.program,
        compiled.binding.initialize.prepared(),
        std::slice::from_ref(&task),
        DEFAULT_STAGE_STEPS,
    )
    .expect_err("source drift must fail closed, not execute a different body");
    assert_eq!(error.len(), 1);
    assert_eq!(error[0].code, "SPX-G570");
}

// ---------------------------------------------------------------------------
// #143 residual gap: recovery replay, cross-instance/stale carriers, deep
// runtime-contract settlement, and the structural zero-effect guarantee.
// ---------------------------------------------------------------------------

/// #143 residual (recovery), and exactly as much of it as this backend can
/// honestly carry: replaying the same dispatch settles to the identical
/// decoded value on a FRESH module instance and a FRESH `node` process --
/// which is what every Wasm dispatch already is, since this executor builds
/// both from scratch on every call and carries nothing across.
///
/// What this does NOT prove, stated here so the name cannot be read as more
/// than it is: it is not evidence that recovery "reuses trusted observations
/// without repeating external work", because a Wasm stage body has no
/// effects to repeat and no observations to reuse. cf8ab366 scoped this
/// backend to stage bodies alone; budgets, cancellation, effect and model
/// dispatch still run on the interpreter, where
/// `native_executor_replays_the_same_dispatch_without_repeating_any_effect`
/// is the test that does carry that claim. Closing #143's fifth case for
/// Wasm needs the executor wired into the effect machinery first.
#[test]
fn wasm_executor_replays_one_dispatch_identically_on_a_fresh_module_and_process() {
    if !native_wasm_tools_available() {
        eprintln!("skipping Wasm executor replay: clang or node unavailable");
        return;
    }
    let compiled = lifecycle();
    let source = MODULE;
    let task = payload(&compiled.binding.task, b"alpha".to_vec(), 10);

    let first = authorization::dispatch_on(
        authorization::StageBackend::Wasm { source },
        &compiled.program,
        compiled.binding.initialize.prepared(),
        std::slice::from_ref(&task),
        DEFAULT_STAGE_STEPS,
    )
    .expect("the first Wasm dispatch evaluates initialize");
    let second = authorization::dispatch_on(
        authorization::StageBackend::Wasm { source },
        &compiled.program,
        compiled.binding.initialize.prepared(),
        std::slice::from_ref(&task),
        DEFAULT_STAGE_STEPS,
    )
    .expect(
        "a replayed Wasm dispatch, on a fresh module instance and a fresh node process, \
         evaluates initialize identically",
    );
    assert_eq!(first.outcome, second.outcome);
}

/// #143 residual: a carrier (a `RetainedValue` fed in as a stage argument)
/// is refused rather than silently accepted when it does not genuinely
/// belong to the dispatch it is handed to -- whether because its own field
/// shape has drifted from what the target stage declares (a STALE carrier)
/// or because it was minted by an entirely different compiled instance that
/// merely happens to reuse the same persistent `@id` for its record (a
/// CROSS-INSTANCE carrier). Neither leaks a build artifact nor corrupts the
/// executor for the next, legitimate dispatch.
#[test]
fn wasm_executor_refuses_stale_and_cross_instance_carriers_without_leakage_or_corruption() {
    if !native_wasm_tools_available() {
        eprintln!("skipping stale/cross-instance carrier refusal: clang or node unavailable");
        return;
    }
    let compiled = lifecycle();
    let source = MODULE;
    let task = payload(&compiled.binding.task, b"alpha".to_vec(), 10);
    let RetainedCallOutcome::Returned(state) = authorization::dispatch_on(
        authorization::StageBackend::Interpreter,
        &compiled.program,
        compiled.binding.initialize.prepared(),
        std::slice::from_ref(&task),
        DEFAULT_STAGE_STEPS,
    )
    .expect("initialize evaluates")
    .outcome
    else {
        panic!("initialize did not return a state");
    };

    // --- Stale carrier: the same declared record identity, but one field's
    // RUNTIME value has drifted to a type `State.epoch` no longer declares
    // (`i64`) -- exactly what a carrier minted by an incompatible, stale
    // version of the same identity would look like.
    let RetainedValue::Record(good) = state.clone() else {
        panic!("initialize did not return a record");
    };
    let epoch = hir::DeclarationId::new("fixture.agent.type.state.epoch".to_owned());
    let mut stale_fields = good.fields.clone();
    let epoch_position = stale_fields
        .iter()
        .position(|field| field.field == epoch)
        .expect("the state carrier carries an epoch field");
    stale_fields[epoch_position].value = RetainedValue::Bytes(vec![1, 2, 3]);
    let stale_state = RetainedValue::Record(RetainedRecord {
        record: good.record.clone(),
        fields: stale_fields,
    });
    let stale_error = authorization::dispatch_on(
        authorization::StageBackend::Wasm { source },
        &compiled.program,
        compiled.binding.observe.prepared(),
        std::slice::from_ref(&stale_state),
        DEFAULT_STAGE_STEPS,
    )
    .expect_err("a carrier whose field runtime type has drifted must be refused, not coerced");
    assert_eq!(stale_error[0].code, "SPX-G570");

    // --- Cross-instance carrier: a value minted by THIS program's own
    // `initialize`, handed to a dispatch bound to a wholly different,
    // independently resolved program whose `State` record reuses the same
    // `@id` but declares an extra field.
    const OTHER_INSTANCE: &str = r#"module test.wasm_executor_cross_instance;

@id("fixture.agent.type.state")
record State {
    @id("fixture.agent.type.state.objective") objective: Bytes,
    @id("fixture.agent.type.state.budget") budget: i64,
    @id("fixture.agent.type.state.epoch") epoch: i64,
    @id("test.wasm_executor_cross_instance.state.extra") extra: i64,
}

@id("fixture.agent.type.observation")
record Observation {
    @id("fixture.agent.type.observation.tag") tag: Bytes,
}

@id("fixture.agent.fn.observe")
fn observe(state: borrow State) -> Observation
{
    let tag = [79u8, 66u8];
    Observation { tag: bytes_copy(array_as_slice(tag)) }
}

@id("app.main")
fn main() -> i64 { 0 }
"#;
    let other_program = hir::resolve(
        &crate::parse(
            OTHER_INSTANCE,
            std::path::Path::new("wasm-executor-cross-instance.spx"),
        )
        .expect("the other-instance fixture parses"),
    )
    .expect("the other-instance fixture resolves");
    let other_observe = crate::interpreter::retained_call::prepare_retained_call(
        &other_program,
        "fixture.agent.fn.observe",
    )
    .expect("the other instance's observe prepares");
    let cross_instance_error = authorization::dispatch_on(
        authorization::StageBackend::Wasm {
            source: OTHER_INSTANCE,
        },
        &other_program,
        &other_observe,
        std::slice::from_ref(&state),
        DEFAULT_STAGE_STEPS,
    )
    .expect_err(
        "a carrier minted by a different compiled instance must be refused even though its \
         @id collides",
    );
    assert_eq!(cross_instance_error[0].code, "SPX-G570");

    // No leakage or corruption: the SAME original, legitimate carrier still
    // dispatches correctly against its own instance after both refusals.
    let healthy = authorization::dispatch_on(
        authorization::StageBackend::Wasm { source },
        &compiled.program,
        compiled.binding.observe.prepared(),
        std::slice::from_ref(&state),
        DEFAULT_STAGE_STEPS,
    )
    .expect("the legitimate carrier still dispatches correctly after the refusals");
    let RetainedCallOutcome::Returned(_) = healthy.outcome else {
        panic!("observe did not return an observation after recovery");
    };
}

/// #143 residual: a genuine RUNTIME contract failure inside the executed
/// Wasm module (checked division by zero, matching the interpreter's own
/// `Fault::DivisionByZero`) settles cleanly. The executor's own temporary
/// build/probe directory is removed even though the failure surfaces deep
/// inside the spawned `node` process, and the failure does not corrupt
/// accounting for a later, unrelated, healthy dispatch.
#[test]
fn wasm_executor_settles_its_probe_directory_and_stays_healthy_after_a_deep_runtime_contract_failure(
) {
    if !native_wasm_tools_available() {
        eprintln!("skipping deep stage failure settlement: clang or node unavailable");
        return;
    }
    const DIVISION_MODULE: &str = r#"module test.wasm_executor_division;

@id("test.wasm_executor_division.fn.divide")
fn divide(a: i64, b: i64) -> i64 { a / b }

@id("app.main")
fn main() -> i64 { 0 }
"#;
    let program = hir::resolve(
        &crate::parse(
            DIVISION_MODULE,
            std::path::Path::new("wasm-executor-division.spx"),
        )
        .expect("the division fixture parses"),
    )
    .expect("the division fixture resolves");
    let divide = crate::interpreter::retained_call::prepare_retained_call(
        &program,
        "test.wasm_executor_division.fn.divide",
    )
    .expect("divide prepares");

    let prefix = format!("semaprax-wasm-stage-executor-{}-", std::process::id());
    let leaked = |prefix: &str| {
        std::fs::read_dir(std::env::temp_dir())
            .into_iter()
            .flatten()
            .flatten()
            .filter(|entry| entry.file_name().to_string_lossy().starts_with(prefix))
            .count()
    };
    let before = leaked(&prefix);

    let failure = authorization::dispatch_on(
        authorization::StageBackend::Wasm {
            source: DIVISION_MODULE,
        },
        &program,
        &divide,
        &[RetainedValue::I64(10), RetainedValue::I64(0)],
        DEFAULT_STAGE_STEPS,
    )
    .expect("division by zero is a checked language outcome, not an executor failure");
    assert_eq!(
        failure.outcome,
        RetainedCallOutcome::LanguageFailure(crate::runtime_status::normalize_arithmetic(
            crate::cleanup_plan::StatusCase::DivisionByZero,
        ))
    );
    assert_eq!(
        leaked(&prefix),
        before,
        "the probe directory for the failed dispatch must not be left behind"
    );

    // Accounting is retained, not corrupted: a healthy dispatch afterward
    // still evaluates correctly.
    let healthy = authorization::dispatch_on(
        authorization::StageBackend::Wasm {
            source: DIVISION_MODULE,
        },
        &program,
        &divide,
        &[RetainedValue::I64(10), RetainedValue::I64(2)],
        DEFAULT_STAGE_STEPS,
    )
    .expect("a healthy dispatch after a failure still evaluates correctly");
    assert_eq!(
        healthy.outcome,
        RetainedCallOutcome::Returned(RetainedValue::I64(5))
    );
}

/// #143 residual: "denied authorization yields zero effects even if
/// JavaScript/provider data requests one" is not just a runtime outcome to
/// re-derive per call -- it is a STRUCTURAL fact about this file, exactly
/// like `the_authorization_value_has_exactly_one_mint_site_in_the_crate`
/// establishes for the crate as a whole. The Wasm executor runs a real
/// Node/V8 process and hands it whatever the compiled module computes, but
/// it never constructs an `Authorized`, never names `AuthorizedRequest`, and
/// never calls the crate's single `mint`. So no matter what the executed
/// JavaScript or the module's own computation "requests", there is no route
/// from this file to an effect: only `run_authorize_stage`'s own mint --
/// unreachable from here -- can ever produce an `Authorized`.
#[test]
fn the_wasm_executor_has_no_route_to_mint_an_authorization_or_reach_undocumented_process_authority()
{
    let wasm_executor = [
        include_str!("authorization/wasm_executor.rs"),
        include_str!("authorization/wasm_executor_process.rs"),
        include_str!("authorization/wasm_executor_workspace.rs"),
    ]
    .join("\n");
    assert_eq!(wasm_executor.matches("Authorized {").count(), 0);
    assert_eq!(wasm_executor.matches("AuthorizedRequest").count(), 0);
    assert_eq!(wasm_executor.matches("mint(").count(), 0);
    for forbidden in [
        "std::net::",
        "TcpStream",
        "std::env::var",
        "fs::read(",
        "fs::read_to_string(",
    ] {
        assert!(
            !wasm_executor.contains(forbidden),
            "wasm_executor.rs contains {forbidden}"
        );
    }
    // The only process this backend spawns is the one documented,
    // explicitly provisioned `node` host -- never a second, undocumented
    // executable a compromised build step could substitute.
    assert_eq!(wasm_executor.matches("Command::new(").count(), 0);
    assert!(wasm_executor.contains("HeldProcessTool::new("));
    assert!(wasm_executor.contains("WasmStageHost"));
}

#[test]
fn public_canonical_retained_value_context_is_the_existing_identity_wire() {
    let value = RetainedValue::Record(RetainedRecord {
        record: hir::DeclarationId::new("state".to_owned()),
        fields: vec![RetainedField {
            field: hir::DeclarationId::new("state.answer".to_owned()),
            value: RetainedValue::Bytes(vec![0xab]),
        }],
    });
    assert_eq!(
        canonical_retained_value_json(&value),
        "{\"record\":\"state\",\"fields\":[{\"field\":\"state.answer\",\"value\":{\"bytes\":\"ab\"}}]}"
    );
    assert_eq!(canonical_retained_value_json(&value), encode_value(&value));
}
