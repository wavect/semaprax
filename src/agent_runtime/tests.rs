use std::cell::Cell;
use std::fs;
use std::path::PathBuf;
use std::rc::Rc;
use std::time::{SystemTime, UNIX_EPOCH};

use sha2::{Digest, Sha256};

use super::private::{
    new_agent, parse_profile, parse_task, preflight_terminal_for_test, replay_evidence,
    replay_trace, terminal_diagnostics_for_test,
};
use super::*;

#[derive(Clone)]
struct FakeProbe {
    cancelled: Rc<Cell<bool>>,
    elapsed: Rc<Cell<u64>>,
    epoch: Rc<Cell<u64>>,
}

impl AgentBoundaryProbe for FakeProbe {
    fn policy_epoch(&self) -> u64 {
        self.epoch.get()
    }
    fn elapsed_ms(&self) -> u64 {
        self.elapsed.get()
    }
}

struct FakeHost {
    probe: FakeProbe,
    attempt: usize,
    tool_calls: usize,
}

impl FakeHost {
    fn fixture() -> Self {
        Self {
            probe: FakeProbe {
                cancelled: Rc::new(Cell::new(false)),
                elapsed: Rc::new(Cell::new(0)),
                epoch: Rc::new(Cell::new(7)),
            },
            attempt: 0,
            tool_calls: 0,
        }
    }
}

impl AgentHost for FakeHost {
    fn policy_epoch(&self) -> u64 {
        self.probe.policy_epoch()
    }
    fn elapsed_ms(&self) -> u64 {
        self.probe.elapsed_ms()
    }
    fn boundary_probe(&self) -> Box<dyn AgentBoundaryProbe> {
        Box::new(self.probe.clone())
    }
    fn tokenize(&mut self, _: &str, request: &str) -> Option<u64> {
        Some(request.len() as u64)
    }
    fn attempt_provider(
        &mut self,
        _: &str,
        _: &str,
        request: &str,
        _: u64,
        sink: &mut ProviderSink,
    ) -> ProviderAttempt {
        let response = if self.attempt == 0 {
            "{\"schema\":\"semaprax.agent-runtime-action.v1\",\"kind\":\"tool\",\"tool_id\":\"fixture.read\",\"arguments\":{\"query\":\"alpha\"}}\n"
        } else {
            "{\"schema\":\"semaprax.agent-runtime-action.v1\",\"kind\":\"final\",\"message\":\"done\"}\n"
        };
        self.attempt += 1;
        assert!(sink.push(&response.as_bytes()[..response.len() / 2]));
        assert!(sink.push(&response.as_bytes()[response.len() / 2..]));
        ProviderAttempt {
            disposition: ProviderDisposition::Succeeded,
            usage: ProviderUsage {
                input_tokens: request.len() as u64,
                output_tokens: response.len() as u64,
                usd_microunits: 0,
            },
        }
    }
    fn invoke_tool(&mut self, _: &str, tool_id: &str, _: &str, sink: &mut ToolResultSink) -> bool {
        assert_eq!(tool_id, "fixture.read");
        self.tool_calls += 1;
        assert!(sink.push(b"{\"value\":\"alpha\"}"));
        true
    }
}

fn fixture_profile() -> String {
    let field = |name: &str| SchemaField {
        name: name.to_owned(),
        kind: ScalarKind::String,
        required: true,
        max_bytes: 64,
    };
    let profile = Profile {
        agent_id: "fixture.agent".to_owned(),
        models: vec![Model {
            provider_id: "fake.local".to_owned(),
            model_id: "fake-basic".to_owned(),
            locality: Locality::Local,
            quality_tier: QualityTier::Basic,
            tokenizer_id: "fake.bytes-v1".to_owned(),
            max_context_tokens: 4096,
            input_price: 0,
            output_price: 0,
            capabilities: vec!["text".to_owned()],
        }],
        tools: vec![Tool {
            tool_id: "fixture.read".to_owned(),
            description: "Return one bounded fixture value.".to_owned(),
            arguments_schema: ClosedSchema {
                fields: vec![field("query")],
            },
            result_schema: ClosedSchema {
                fields: vec![field("value")],
            },
            required_capabilities: vec!["tool.read".to_owned()],
        }],
        policy: Policy {
            allowed_provider_ids: vec!["fake.local".to_owned()],
            allowed_model_ids: vec!["fake-basic".to_owned()],
            required_locality: RequiredLocality::LocalOnly,
            minimum_quality_tier: QualityTier::Basic,
            required_model_capabilities: vec!["text".to_owned()],
            granted_capabilities: vec!["tool.read".to_owned()],
            allowed_tool_ids: vec!["fixture.read".to_owned()],
        },
        limits: EffectiveLimits {
            max_turns: 2,
            max_provider_attempts: 2,
            max_retries_per_turn: 1,
            max_concurrency: 1,
            max_elapsed_ms: 1000,
            max_provider_request_bytes: 65536,
            max_provider_response_bytes: 4096,
            max_stream_chunks: 64,
            max_total_provider_input_bytes: 131072,
            max_total_provider_output_bytes: 8192,
            max_reported_model_input_tokens: 131072,
            max_reported_model_output_tokens: 8192,
            max_usd_microunits: 0,
            max_tool_calls: 1,
            max_tool_arguments_bytes: 4096,
            max_tool_result_bytes: 4096,
            max_total_tool_bytes: 8192,
            max_retained_state_bytes: 131072,
            max_trace_events: 64,
            max_trace_bytes: 131072,
            max_evidence_bytes: 262144,
            max_builder_bytes: 1048576,
        },
        source: String::new(),
        digest: String::new(),
    };
    render_profile(&profile)
}

fn fixture_task() -> String {
    let task = Task {
        nonce: "0".repeat(64),
        objective: "Return one bounded answer.".to_owned(),
        context: vec![ContextItem {
            label: "input".to_owned(),
            provenance: Provenance::CallerUntrusted,
            content: "alpha".to_owned(),
        }],
        source: String::new(),
        digest: String::new(),
    };
    super::private::render_task(&task)
}

#[test]
fn private_fixture_runs_one_tool_then_completes_with_replayable_evidence() {
    let profile_source = fixture_profile();
    let task_source = fixture_task();
    assert_eq!(
        parse_profile(&profile_source).unwrap().source,
        profile_source
    );
    assert_eq!(parse_task(&task_source).unwrap().source, task_source);
    let artifact = new_agent(&profile_source, FakeHost::fixture())
        .run(&task_source)
        .unwrap();
    assert!(artifact.status == RunStatus::Completed);
    replay_trace(&artifact.trace).unwrap();
    assert_eq!(
        artifact.trace_digest,
        digest(TRACE_DOMAIN, artifact.trace.as_bytes())
    );
    assert_eq!(
        artifact.evidence_digest,
        digest(EVIDENCE_DOMAIN, artifact.evidence.as_bytes())
    );
    assert!(!artifact.trace.contains("alpha"));
    assert!(!artifact.evidence.contains("alpha"));
}

#[derive(Clone, Copy)]
enum BoundaryFault {
    None,
    Cancel,
    Deadline,
    Revoke,
}

struct ScriptHost {
    probe: FakeProbe,
    attempts: Vec<(ProviderDisposition, Vec<Vec<u8>>, ProviderUsage)>,
    tool_result: Vec<Vec<u8>>,
    tool_failure: bool,
    provider_fault: BoundaryFault,
    tool_fault: BoundaryFault,
    revoke_after_admission: bool,
    provider_calls: Rc<Cell<usize>>,
    tool_calls: Rc<Cell<usize>>,
    selected: Vec<(String, String)>,
    requests: Vec<String>,
    call_ids: Vec<String>,
    private_secret: String,
}

impl ScriptHost {
    fn final_only(message: &str) -> Self {
        let response = format!(
            "{{\"schema\":\"{ACTION_SCHEMA}\",\"kind\":\"final\",\"message\":{}}}\n",
            quote_json(message)
        );
        Self {
            probe: FakeHost::fixture().probe,
            attempts: vec![(
                ProviderDisposition::Succeeded,
                vec![response.into_bytes()],
                ProviderUsage::default(),
            )],
            tool_result: vec![b"{\"value\":\"alpha\"}".to_vec()],
            tool_failure: false,
            provider_fault: BoundaryFault::None,
            tool_fault: BoundaryFault::None,
            revoke_after_admission: false,
            provider_calls: Rc::new(Cell::new(0)),
            tool_calls: Rc::new(Cell::new(0)),
            selected: Vec::new(),
            requests: Vec::new(),
            call_ids: Vec::new(),
            private_secret: "HOST-CREDENTIAL-SENTINEL".to_owned(),
        }
    }

    fn apply_fault(&self, fault: BoundaryFault) {
        match fault {
            BoundaryFault::None => {}
            BoundaryFault::Cancel => self.probe.cancelled.set(true),
            BoundaryFault::Deadline => self.probe.elapsed.set(1_001),
            BoundaryFault::Revoke => self.probe.epoch.set(8),
        }
    }
}

impl AgentHost for ScriptHost {
    fn policy_epoch(&self) -> u64 {
        let epoch = self.probe.policy_epoch();
        if self.revoke_after_admission {
            self.probe.epoch.set(epoch + 1);
        }
        epoch
    }

    fn elapsed_ms(&self) -> u64 {
        self.probe.elapsed_ms()
    }

    fn boundary_probe(&self) -> Box<dyn AgentBoundaryProbe> {
        Box::new(self.probe.clone())
    }

    fn tokenize(&mut self, _: &str, request: &str) -> Option<u64> {
        assert!(!request.contains(&self.private_secret));
        Some(request.len() as u64)
    }

    fn attempt_provider(
        &mut self,
        provider_id: &str,
        model_id: &str,
        request: &str,
        _: u64,
        sink: &mut ProviderSink,
    ) -> ProviderAttempt {
        self.selected
            .push((provider_id.to_owned(), model_id.to_owned()));
        self.requests.push(request.to_owned());
        let index = self.provider_calls.get();
        self.provider_calls.set(index + 1);
        let (disposition, chunks, usage) = self
            .attempts
            .get(index)
            .unwrap_or_else(|| self.attempts.last().expect("script has an attempt"));
        self.apply_fault(self.provider_fault);
        for chunk in chunks {
            if !sink.push(chunk) {
                break;
            }
        }
        let mut usage = *usage;
        if matches!(disposition, ProviderDisposition::Succeeded) {
            usage.input_tokens = request.len() as u64;
        }
        ProviderAttempt {
            disposition: match disposition {
                ProviderDisposition::Succeeded => ProviderDisposition::Succeeded,
                ProviderDisposition::DefinitelyNotStarted => {
                    ProviderDisposition::DefinitelyNotStarted
                }
                ProviderDisposition::FailedUncertain => ProviderDisposition::FailedUncertain,
            },
            usage,
        }
    }

    fn invoke_tool(&mut self, call_id: &str, _: &str, _: &str, sink: &mut ToolResultSink) -> bool {
        self.tool_calls.set(self.tool_calls.get() + 1);
        self.call_ids.push(call_id.to_owned());
        self.apply_fault(self.tool_fault);
        for chunk in &self.tool_result {
            if !sink.push(chunk) {
                break;
            }
        }
        !self.tool_failure
    }
}

fn raw_sha(source: &str) -> String {
    format!(
        "sha256:{:x}",
        crate::digest_hex::LowerHex(Sha256::digest(source.as_bytes()))
    )
}

fn diagnostic(error: Vec<Diagnostic>) -> (&'static str, String) {
    assert_eq!(error.len(), 1);
    (error[0].code, error[0].message.clone())
}

fn parsed_profile() -> Profile {
    parse_profile(&fixture_profile()).unwrap()
}

fn profile_error(source: &str) -> Diagnostic {
    match parse_profile(source) {
        Ok(_) => panic!("profile unexpectedly parsed"),
        Err(error) => error,
    }
}

fn task_error(source: &str) -> Diagnostic {
    match parse_task(source) {
        Ok(_) => panic!("task unexpectedly parsed"),
        Err(error) => error,
    }
}

fn run_error<H: AgentHost>(profile: &str, host: H, task: &str) -> Vec<Diagnostic> {
    match new_agent(profile, host).run(task) {
        Ok(_) => panic!("run unexpectedly succeeded"),
        Err(error) => error,
    }
}

fn final_action(message: &str) -> Vec<u8> {
    format!(
        "{{\"schema\":\"{ACTION_SCHEMA}\",\"kind\":\"final\",\"message\":{}}}\n",
        quote_json(message)
    )
    .into_bytes()
}

fn tool_action(tool: &str, arguments: &str) -> Vec<u8> {
    format!(
        "{{\"schema\":\"{ACTION_SCHEMA}\",\"kind\":\"tool\",\"tool_id\":{},\"arguments\":{arguments}}}\n",
        quote_json(tool)
    )
    .into_bytes()
}

#[test]
fn canonical_profile_and_task_parsers_reject_wire_and_invariant_mutations() {
    let profile = fixture_profile();
    let task = fixture_task();
    let tool_action =
        String::from_utf8(tool_action("fixture.read", "{\"query\":\"alpha\"}")).unwrap();
    let final_action = String::from_utf8(final_action("done")).unwrap();
    assert_eq!(
        raw_sha(&profile),
        "sha256:14981ee99af965dcea311121a90cacfb9891a00d6365e7ad00cab8cefe69c01a"
    );
    assert_eq!(
        raw_sha(&task),
        "sha256:b6be370dea6708b7b3f7c6bd46299061c8f146a684fdf9895c32dc7e50c9a425"
    );
    assert_eq!(
        raw_sha(&tool_action),
        "sha256:a7142d92a8d33130892472cfeafee44519fe7bbc9c52a12319638089583a5286"
    );
    assert_eq!(
        raw_sha(&final_action),
        "sha256:2b44a98bfc80bb89339c4a76c6d43637f3a65c5b0a65a9a5571d507289f6681a"
    );
    for hostile in [
        profile.trim_end().to_owned(),
        profile.replace('\n', "\r\n"),
        format!("\u{feff}{profile}"),
        profile.replacen("\"agent_id\":", "\"extra\":0,\"agent_id\":", 1),
        profile.replacen("\"schema\":", "\"schema_copy\":\"x\",\"schema\":", 1),
        profile.replacen("\"schema\":", "\"schema\":\"x\",\"schema\":", 1),
        profile.replacen("\"models\":[", "\"models\":{\"bad\":", 1),
        profile.replacen("\"provider_id\":", "\"extra\":0,\"provider_id\":", 1),
        profile.replacen("\"tools\":[", "\"tools\":{\"bad\":", 1),
        profile.replacen("\"tool_id\":", "\"extra\":0,\"tool_id\":", 1),
        profile.replacen(
            "\"allowed_provider_ids\":",
            "\"extra\":0,\"allowed_provider_ids\":",
            1,
        ),
        profile.replacen("\"max_turns\":", "\"extra\":0,\"max_turns\":", 1),
    ] {
        assert_eq!(profile_error(&hostile).code, "SPX-G204");
    }
    for hostile in [
        task.trim_end().to_owned(),
        task.replace('\n', "\r\n"),
        format!("\u{feff}{task}"),
        task.replacen("\"objective\":", "\"extra\":0,\"objective\":", 1),
        task.replacen("\"nonce\":", "\"nonce\":\"bad\",\"nonce_copy\":", 1),
        task.replacen("\"context\":[", "\"context\":{\"bad\":", 1),
        task.replacen("\"label\":", "\"extra\":0,\"label\":", 1),
        task.replacen("\"provenance\":", "\"provenance\":0,\"old\":", 1),
    ] {
        assert_eq!(task_error(&hostile).code, "SPX-G204");
    }

    let mut profile = parsed_profile();
    profile.models.push(profile.models[0].clone());
    assert_eq!(profile_error(&render_profile(&profile)).code, "SPX-G205");
    let mut profile = parsed_profile();
    profile.policy.granted_capabilities = vec!["*".to_owned()];
    assert_eq!(profile_error(&render_profile(&profile)).code, "SPX-G205");
    let mut profile = parsed_profile();
    profile.limits.max_concurrency = 2;
    assert_eq!(profile_error(&render_profile(&profile)).code, "SPX-G208");
    let mut profile = parsed_profile();
    profile.limits.max_concurrency = 0;
    assert_eq!(profile_error(&render_profile(&profile)).code, "SPX-G205");
    let mut profile = parsed_profile();
    profile.limits.max_turns = MAX_TURNS + 1;
    let error = profile_error(&render_profile(&profile));
    assert_eq!(error.code, "SPX-G208");
    assert_eq!(error.message, format!("max_turns exceeds {MAX_TURNS}"));

    let exact_profile_bytes = "x".repeat(MAX_PROFILE_BYTES);
    assert_eq!(profile_error(&exact_profile_bytes).code, "SPX-G204");
    let over_profile_bytes = "x".repeat(MAX_PROFILE_BYTES + 1);
    let error = profile_error(&over_profile_bytes);
    assert_eq!(error.code, "SPX-G208");
    assert_eq!(
        error.message,
        format!("profile_bytes exceeds {MAX_PROFILE_BYTES}")
    );

    let nested_task = |arrays: usize| {
        format!(
            "{{\"schema\":\"{TASK_SCHEMA}\",\"nonce\":\"{}\",\"objective\":\"\",\"context\":{}{}}}\n",
            "0".repeat(64),
            "[".repeat(arrays),
            "]".repeat(arrays)
        )
    };
    assert_eq!(
        task_error(&nested_task(MAX_JSON_DEPTH - 1)).code,
        "SPX-G204"
    );
    let error = task_error(&nested_task(MAX_JSON_DEPTH));
    assert_eq!(error.code, "SPX-G208");
    assert_eq!(
        error.message,
        format!("json_depth exceeds {MAX_JSON_DEPTH}")
    );
}

struct NoWriteFixture {
    root: PathBuf,
    expected: Vec<(String, Vec<u8>)>,
}

impl NoWriteFixture {
    fn new() -> Self {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let root = std::env::temp_dir().join(format!(
            "semaprax-agent-runtime-no-write-{}-{unique}",
            std::process::id()
        ));
        fs::create_dir(&root).unwrap();
        fs::write(root.join("sentinel.bin"), b"unchanged\0bytes").unwrap();
        fs::create_dir(root.join("nested")).unwrap();
        fs::write(root.join("nested/value.txt"), b"still unchanged\n").unwrap();
        Self {
            root,
            expected: vec![
                ("nested/value.txt".to_owned(), b"still unchanged\n".to_vec()),
                ("sentinel.bin".to_owned(), b"unchanged\0bytes".to_vec()),
            ],
        }
    }

    fn assert_unchanged(&self) {
        let mut actual = Vec::new();
        for relative in ["nested/value.txt", "sentinel.bin"] {
            actual.push((
                relative.to_owned(),
                fs::read(self.root.join(relative)).unwrap(),
            ));
        }
        assert_eq!(actual, self.expected);
        let mut root_entries = fs::read_dir(&self.root)
            .unwrap()
            .map(|entry| entry.unwrap().file_name())
            .collect::<Vec<_>>();
        root_entries.sort();
        assert_eq!(root_entries.len(), 2);
    }
}

impl Drop for NoWriteFixture {
    fn drop(&mut self) {
        fs::remove_dir_all(&self.root).unwrap();
    }
}

#[test]
fn private_runtime_has_no_ambient_write_authority() {
    let fixture = NoWriteFixture::new();
    let artifact = new_agent(&fixture_profile(), ScriptHost::final_only("done"))
        .run(&fixture_task())
        .unwrap();
    assert!(artifact.status == RunStatus::Completed);
    fixture.assert_unchanged();
    assert!(artifact
        .evidence
        .contains("no_ambient_network_filesystem_process_home_or_environment_authority"));
    assert!(artifact
        .evidence
        .contains("no_write_apply_mutation_or_target_execution_tool_authority"));
}

#[test]
fn router_is_cost_then_identity_ordered_and_permutation_independent() {
    let mut profile = parsed_profile();
    let mut cheaper = profile.models[0].clone();
    cheaper.provider_id = "a.local".to_owned();
    cheaper.model_id = "cheap".to_owned();
    cheaper.input_price = 0;
    cheaper.output_price = 0;
    let mut expensive = cheaper.clone();
    expensive.provider_id = "z.local".to_owned();
    expensive.model_id = "expensive".to_owned();
    expensive.input_price = 1_000_000;
    expensive.output_price = 1_000_000;
    profile.models = vec![cheaper.clone(), expensive];
    profile.policy.allowed_provider_ids = vec!["a.local".to_owned(), "z.local".to_owned()];
    profile.policy.allowed_model_ids = vec!["cheap".to_owned(), "expensive".to_owned()];
    profile.limits.max_usd_microunits = MAX_USD_MICROUNITS;
    let source = render_profile(&profile);
    let artifact = new_agent(&source, ScriptHost::final_only("done"))
        .run(&fixture_task())
        .unwrap();
    assert!(artifact.trace.contains("\"provider_id\":\"a.local\""));
    assert!(!artifact.trace.contains("\"provider_id\":\"z.local\""));

    let mut tied = cheaper;
    tied.provider_id = "b.local".to_owned();
    tied.model_id = "same".to_owned();
    profile.models = vec![
        Model {
            model_id: "same".to_owned(),
            ..profile.models[0].clone()
        },
        tied,
    ];
    profile.policy.allowed_provider_ids = vec!["a.local".to_owned(), "b.local".to_owned()];
    profile.policy.allowed_model_ids = vec!["same".to_owned()];
    let source = render_profile(&profile);
    let first = new_agent(&source, ScriptHost::final_only("done"))
        .run(&fixture_task())
        .unwrap();
    let second = new_agent(&source, ScriptHost::final_only("done"))
        .run(&fixture_task())
        .unwrap();
    assert_eq!(first.trace, second.trace);
    assert_eq!(first.evidence, second.evidence);
    assert!(first.trace.contains("\"provider_id\":\"a.local\""));
}

#[test]
fn provider_streaming_retry_and_uncertainty_are_exact() {
    let response = final_action("fragmented");
    let mut single = ScriptHost::final_only("fragmented");
    single.attempts[0].1 = vec![response.clone()];
    let mut fragmented = ScriptHost::final_only("fragmented");
    fragmented.attempts[0].1 = vec![
        response[..response.len() / 3].to_vec(),
        response[response.len() / 3..].to_vec(),
    ];
    let one = new_agent(&fixture_profile(), single)
        .run(&fixture_task())
        .unwrap();
    let many = new_agent(&fixture_profile(), fragmented)
        .run(&fixture_task())
        .unwrap();
    assert_eq!(one.trace, many.trace);
    assert_eq!(one.evidence, many.evidence);

    let mut retry = ScriptHost::final_only("retried");
    retry.attempts.insert(
        0,
        (
            ProviderDisposition::DefinitelyNotStarted,
            Vec::new(),
            ProviderUsage::default(),
        ),
    );
    let artifact = new_agent(&fixture_profile(), retry)
        .run(&fixture_task())
        .unwrap();
    assert!(artifact.status == RunStatus::Completed);
    assert!(artifact
        .trace
        .contains("\"status\":\"definitely_not_started\""));
    assert!(artifact.trace.contains("\"provider_attempts\":2"));

    let mut uncertain = ScriptHost::final_only("must-not-run");
    uncertain.attempts[0] = (
        ProviderDisposition::FailedUncertain,
        vec![b"partial".to_vec()],
        ProviderUsage::default(),
    );
    let artifact = new_agent(&fixture_profile(), uncertain)
        .run(&fixture_task())
        .unwrap();
    assert!(artifact.status == RunStatus::ProviderFailed);
    assert!(artifact.trace.contains("\"provider_attempts\":1"));
    assert!(!artifact.trace.contains("must-not-run"));

    let mut malformed = ScriptHost::final_only("unused");
    malformed.attempts[0].1 = vec![vec![0xff]];
    let artifact = new_agent(&fixture_profile(), malformed)
        .run(&fixture_task())
        .unwrap();
    assert!(artifact.status == RunStatus::ProviderFailed);
    assert!(artifact.evidence.contains("SPX-I218"));
}

#[test]
fn cancellation_deadline_and_policy_revocation_close_provider_and_tool_sinks() {
    for (fault, status, code) in [
        (BoundaryFault::Cancel, RunStatus::Cancelled, "SPX-I220"),
        (
            BoundaryFault::Deadline,
            RunStatus::DeadlineExceeded,
            "SPX-I221",
        ),
        (BoundaryFault::Revoke, RunStatus::PolicyRejected, "SPX-G207"),
    ] {
        let mut host = ScriptHost::final_only("unreachable");
        host.revoke_after_admission = matches!(fault, BoundaryFault::Revoke);
        let probe = host.probe.clone();
        let cancellation = AgentCancellation::new();
        let provider_calls = Rc::clone(&host.provider_calls);
        let tool_calls = Rc::clone(&host.tool_calls);
        let profile_source = fixture_profile();
        let mut agent = Agent::new(&profile_source, host, cancellation.clone()).unwrap();
        match fault {
            BoundaryFault::Cancel => cancellation.cancel(),
            BoundaryFault::Deadline => probe.elapsed.set(1_001),
            BoundaryFault::Revoke => {}
            BoundaryFault::None => unreachable!(),
        }
        if matches!(fault, BoundaryFault::Cancel) {
            let diagnostics = match agent.run(&fixture_task()) {
                Ok(_) => panic!("pre-effect cancellation produced an artifact"),
                Err(diagnostics) => diagnostics,
            };
            assert_eq!(diagnostics.len(), 1);
            assert_eq!(diagnostics[0].code, code);
            assert_eq!(diagnostics[0].message, "Agent Runtime run was cancelled");
        } else {
            let artifact = agent.run(&fixture_task()).unwrap();
            assert!(artifact.status == status);
            assert!(artifact.evidence.contains(code));
        }
        assert_eq!(provider_calls.get(), 0);
        assert_eq!(tool_calls.get(), 0);

        let mut host = ScriptHost::final_only("unreachable");
        host.provider_fault = fault;
        let cancellation = AgentCancellation::new();
        if matches!(fault, BoundaryFault::Cancel) {
            host.provider_fault = BoundaryFault::None;
        }
        let mut agent = Agent::new(&fixture_profile(), host, cancellation.clone()).unwrap();
        if matches!(fault, BoundaryFault::Cancel) {
            cancellation.cancel();
        }
        if matches!(fault, BoundaryFault::Cancel) {
            let diagnostics = match agent.run(&fixture_task()) {
                Ok(_) => panic!("pre-effect cancellation produced an artifact"),
                Err(diagnostics) => diagnostics,
            };
            assert_eq!(diagnostics.len(), 1);
            assert_eq!(diagnostics[0].code, code);
        } else {
            let artifact = agent.run(&fixture_task()).unwrap();
            assert!(artifact.status == status);
            assert!(artifact.evidence.contains(code));
            assert!(!artifact.trace.contains("unreachable"));
        }

        let mut host = ScriptHost::final_only("done");
        host.attempts = vec![
            (
                ProviderDisposition::Succeeded,
                vec![tool_action("fixture.read", "{\"query\":\"alpha\"}")],
                ProviderUsage::default(),
            ),
            (
                ProviderDisposition::Succeeded,
                vec![final_action("done")],
                ProviderUsage::default(),
            ),
        ];
        host.tool_fault = fault;
        let cancellation = AgentCancellation::new();
        if matches!(fault, BoundaryFault::Cancel) {
            host.tool_fault = BoundaryFault::None;
        }
        let mut agent = Agent::new(&fixture_profile(), host, cancellation.clone()).unwrap();
        let artifact = if matches!(fault, BoundaryFault::Cancel) {
            // Existing private fault injection cannot access the runtime-owned bit;
            // the public integration corpus exercises cancellation during tool push.
            cancellation.cancel();
            let diagnostics = match agent.run(&fixture_task()) {
                Ok(_) => panic!("pre-effect cancellation produced an artifact"),
                Err(diagnostics) => diagnostics,
            };
            assert_eq!(diagnostics.len(), 1);
            assert_eq!(diagnostics[0].code, code);
            continue;
        } else {
            agent.run(&fixture_task()).unwrap()
        };
        assert!(artifact.status == status);
        assert!(artifact.evidence.contains(code));
        assert!(!artifact.trace.contains("\"status\":\"succeeded\",\"usage\":{\"provider_input_bytes\":0,\"provider_output_bytes\":0,\"reported_model_input_tokens\":0,\"reported_model_output_tokens\":0,\"usd_microunits\":0,\"tool_argument_bytes\":0,\"tool_result_bytes\":"));
    }
}

#[test]
fn tool_authority_schema_and_preinvoke_budgets_fail_without_a_call() {
    let cases = [
        ("other.read", "{\"query\":\"alpha\"}", "unknown tool"),
        (
            "fixture.read",
            "{\"extra\":\"alpha\"}",
            "arguments schema mismatch",
        ),
        ("fixture.read", "{\"query\":7}", "arguments schema mismatch"),
    ];
    for (tool, arguments, reason) in cases {
        let mut host = ScriptHost::final_only("unused");
        host.attempts[0].1 = vec![tool_action(tool, arguments)];
        let tool_calls = Rc::clone(&host.tool_calls);
        let artifact = new_agent(&fixture_profile(), host)
            .run(&fixture_task())
            .unwrap();
        assert!(artifact.status == RunStatus::PolicyRejected);
        assert!(artifact.evidence.contains(reason));
        assert!(artifact.trace.contains("\"tool_calls\":0"));
        assert_eq!(tool_calls.get(), 0);
    }

    let mut profile = parsed_profile();
    profile.policy.granted_capabilities.clear();
    let mut host = ScriptHost::final_only("unused");
    host.attempts[0].1 = vec![tool_action("fixture.read", "{\"query\":\"alpha\"}")];
    let artifact = new_agent(&render_profile(&profile), host)
        .run(&fixture_task())
        .unwrap();
    assert!(artifact.status == RunStatus::PolicyRejected);
    assert!(artifact.evidence.contains("required capability missing"));
    assert!(artifact.trace.contains("\"tool_calls\":0"));

    for (label, mutate) in [
        (
            "tool_calls exceeds 0",
            (|limits: &mut EffectiveLimits| limits.max_tool_calls = 0) as fn(&mut EffectiveLimits),
        ),
        (
            "tool_arguments_bytes exceeds 1",
            (|limits: &mut EffectiveLimits| limits.max_tool_arguments_bytes = 1)
                as fn(&mut EffectiveLimits),
        ),
        (
            "total_tool_bytes exceeds 1",
            (|limits: &mut EffectiveLimits| limits.max_total_tool_bytes = 1)
                as fn(&mut EffectiveLimits),
        ),
    ] {
        let mut profile = parsed_profile();
        mutate(&mut profile.limits);
        let mut host = ScriptHost::final_only("unused");
        host.attempts[0].1 = vec![tool_action("fixture.read", "{\"query\":\"alpha\"}")];
        let tool_calls = Rc::clone(&host.tool_calls);
        let artifact = new_agent(&render_profile(&profile), host)
            .run(&fixture_task())
            .unwrap();
        assert!(
            artifact.status == RunStatus::BudgetExhausted,
            "{label} status"
        );
        assert!(artifact.evidence.contains(label), "{label} evidence");
        assert!(artifact.trace.contains("\"tool_calls\":0"));
        assert_eq!(tool_calls.get(), 0);
    }
}

#[test]
fn tool_result_failures_are_terminal_and_call_ids_are_deterministic() {
    let mut failed = ScriptHost::final_only("unused");
    failed.attempts[0].1 = vec![tool_action("fixture.read", "{\"query\":\"alpha\"}")];
    failed.tool_failure = true;
    let tool_calls = Rc::clone(&failed.tool_calls);
    let artifact = new_agent(&fixture_profile(), failed)
        .run(&fixture_task())
        .unwrap();
    assert!(artifact.status == RunStatus::ToolFailed);
    assert!(artifact.evidence.contains("SPX-I219"));
    assert_eq!(tool_calls.get(), 1);

    let mut profile = parsed_profile();
    profile.limits.max_tool_result_bytes = 1;
    let mut over = ScriptHost::final_only("unused");
    over.attempts[0].1 = vec![tool_action("fixture.read", "{\"query\":\"alpha\"}")];
    let tool_calls = Rc::clone(&over.tool_calls);
    let artifact = new_agent(&render_profile(&profile), over)
        .run(&fixture_task())
        .unwrap();
    assert!(artifact.status == RunStatus::BudgetExhausted);
    assert!(artifact.evidence.contains("tool_result_bytes exceeds 1"));
    assert_eq!(tool_calls.get(), 0);

    for (bytes, status, code, message) in [
        (
            b"{".to_vec(),
            RunStatus::ToolFailed,
            "SPX-I219",
            "result invalid",
        ),
        (
            b"{\"wrong\":true}".to_vec(),
            RunStatus::PolicyRejected,
            "SPX-G207",
            "result schema mismatch",
        ),
    ] {
        let mut host = ScriptHost::final_only("unused");
        host.attempts[0].1 = vec![tool_action("fixture.read", "{\"query\":\"alpha\"}")];
        host.tool_result = vec![bytes.clone()];
        let provider_calls = Rc::clone(&host.provider_calls);
        let tool_calls = Rc::clone(&host.tool_calls);
        let artifact = new_agent(&fixture_profile(), host)
            .run(&fixture_task())
            .unwrap();
        assert!(artifact.status == status);
        assert!(artifact.evidence.contains(code));
        assert!(artifact.evidence.contains(message));
        assert_eq!(provider_calls.get(), 1);
        assert_eq!(tool_calls.get(), 1);

        let trace: Value = serde_json::from_str(artifact.trace.trim_end()).unwrap();
        let events = trace["events"].as_array().unwrap();
        let tool_finished = events
            .iter()
            .find(|event| event["kind"] == "tool_finished")
            .unwrap();
        assert_eq!(tool_finished["status"], "failed");
        assert!(tool_finished["output_digest"].is_null());
        assert_eq!(tool_finished["usage"]["tool_result_bytes"], bytes.len());
        assert_eq!(trace["usage"]["tool_result_bytes"], bytes.len());
        assert_eq!(trace["usage"]["provider_attempts"], 1);
        assert_eq!(trace["usage"]["tool_calls"], 1);
        assert_eq!(
            events
                .iter()
                .filter(|event| event["kind"] == "provider_attempt_started")
                .count(),
            1
        );
        assert_eq!(
            events
                .iter()
                .filter(|event| event["kind"] == "tool_finished")
                .count(),
            1
        );
    }

    let mut invalid = ScriptHost::final_only("unused");
    invalid.attempts[0].1 = vec![tool_action("fixture.read", "{\"query\":\"alpha\"}")];
    invalid.tool_result = vec![b"{\"wrong\":true}".to_vec()];
    let artifact = new_agent(&fixture_profile(), invalid)
        .run(&fixture_task())
        .unwrap();
    assert!(artifact.status == RunStatus::PolicyRejected);
    assert!(artifact.evidence.contains("result schema mismatch"));
    assert!(artifact.trace.contains("\"tool_calls\":1"));

    let run = || {
        let mut host = ScriptHost::final_only("done");
        host.attempts = vec![
            (
                ProviderDisposition::Succeeded,
                vec![tool_action("fixture.read", "{\"query\":\"alpha\"}")],
                ProviderUsage::default(),
            ),
            (
                ProviderDisposition::Succeeded,
                vec![final_action("done")],
                ProviderUsage::default(),
            ),
        ];
        new_agent(&fixture_profile(), host)
            .run(&fixture_task())
            .unwrap()
    };
    let first = run();
    let second = run();
    assert_eq!(first.trace, second.trace);
    assert_eq!(first.evidence, second.evidence);
    assert!(first.status == RunStatus::Completed);
    assert!(first.trace.contains("\"tool_calls\":1"));
}

#[test]
fn trace_and_evidence_known_answers_and_replay_mutations_are_exact() {
    let artifact = new_agent(&fixture_profile(), FakeHost::fixture())
        .run(&fixture_task())
        .unwrap();
    assert_eq!(
        raw_sha(&artifact.trace),
        "sha256:b418408ff16de76251e0b40eb2c7b68dd408bbae66b96e734138ad64f6f70eab"
    );
    assert_eq!(
        raw_sha(&artifact.evidence),
        "sha256:45da26349aa89514ca3066a0f14076d4220cb03560589b9f959f97e9564bd6ad"
    );
    for (index, hostile) in [
        artifact.trace.trim_end().to_owned(),
        artifact.trace.replacen("\"index\":0", "\"index\":1", 1),
        artifact
            .trace
            .replacen("\"run_started\"", "\"unknown_evt\"", 1),
        artifact
            .trace
            .replacen("\"nonclaims\":", "\"extra\":0,\"nonclaims\":", 1),
        artifact.trace.replacen("\"completed\"", "\"cancelled\"", 1),
        artifact
            .trace
            .replacen("\"provider_attempts\":2", "\"provider_attempts\":3", 1),
        artifact.trace.replacen(
            "\"profile_digest\":\"sha256:",
            "\"profile_digest\":\"sha256:0",
            1,
        ),
        artifact.trace.replacen(
            "\"termination\":{\"status\":\"completed\",\"code\":null",
            "\"termination\":{\"status\":\"completed\",\"code\":\"SPX-G207\"",
            1,
        ),
    ]
    .into_iter()
    .enumerate()
    {
        assert!(
            replay_trace(&hostile).is_err(),
            "trace hostile {index} passed"
        );
    }
    let profile = parsed_profile();
    let task = parse_task(&fixture_task()).unwrap();
    for (index, hostile) in [
        artifact.evidence.trim_end().to_owned(),
        artifact.evidence.replacen(
            &artifact.trace_digest,
            &format!("sha256:{}", "0".repeat(64)),
            1,
        ),
        artifact
            .evidence
            .replacen("\"used_turns\":2", "\"used_turns\":3", 1),
        artifact
            .evidence
            .replacen("no_model_output_authority", "no_model_output_authoritx", 1),
        artifact
            .evidence
            .replacen("\"result\":", "\"extra\":0,\"result\":", 1),
        artifact
            .evidence
            .replacen(&profile.digest, &format!("sha256:{}", "0".repeat(64)), 1),
        artifact
            .evidence
            .replacen(&task.digest, &format!("sha256:{}", "1".repeat(64)), 1),
        artifact.evidence.replacen(
            "\"result\":{\"status\":\"completed\"",
            "\"result\":{\"status\":\"cancelled\"",
            1,
        ),
        artifact.evidence.replacen(
            &format!("\"max_profile_bytes\":{MAX_PROFILE_BYTES}"),
            &format!("\"max_profile_bytes\":{}", MAX_PROFILE_BYTES - 1),
            1,
        ),
        artifact
            .evidence
            .replacen("\"used_models\":1", "\"used_models\":2", 1),
    ]
    .into_iter()
    .enumerate()
    {
        assert_ne!(hostile, artifact.evidence, "evidence hostile {index} no-op");
        let error = replay_evidence(&hostile, &fixture_profile(), &artifact).unwrap_err();
        assert!(matches!(error.code, "SPX-G204" | "SPX-G209"));
    }
}

#[test]
fn secret_isolation_and_builder_limits_are_fail_closed() {
    let sentinel = "HOST-CREDENTIAL-SENTINEL";
    let artifact = new_agent(&fixture_profile(), ScriptHost::final_only("done"))
        .run(&fixture_task())
        .unwrap();
    assert!(!artifact.trace.contains(sentinel));
    assert!(!artifact.evidence.contains(sentinel));

    let mut profile = parsed_profile();
    profile.limits.max_builder_bytes = 1;
    let error = run_error(
        &render_profile(&profile),
        ScriptHost::final_only("done"),
        &fixture_task(),
    );
    let (code, message) = diagnostic(error);
    assert_eq!(code, "SPX-G208");
    assert_eq!(message, "builder_bytes exceeds 1");

    let mut profile = parsed_profile();
    profile.limits.max_provider_response_bytes = 1;
    let artifact = new_agent(&render_profile(&profile), ScriptHost::final_only("done"))
        .run(&fixture_task())
        .unwrap();
    assert!(artifact.status == RunStatus::BudgetExhausted);
    assert!(artifact.evidence.contains("SPX-G208"));
}

#[test]
fn profile_cardinality_and_task_byte_boundaries_are_exact() {
    let mut profile = parsed_profile();
    profile.models = (0..MAX_MODELS)
        .map(|index| Model {
            model_id: format!("model-{index:02}"),
            ..profile.models[0].clone()
        })
        .collect();
    profile.policy.allowed_model_ids = profile
        .models
        .iter()
        .map(|model| model.model_id.clone())
        .collect();
    let exact = render_profile(&profile);
    assert_eq!(parse_profile(&exact).unwrap().models.len(), MAX_MODELS);
    profile.models.push(Model {
        model_id: "model-32".to_owned(),
        ..profile.models[0].clone()
    });
    profile.policy.allowed_model_ids.push("model-32".to_owned());
    assert_eq!(profile_error(&render_profile(&profile)).code, "SPX-G205");

    let mut profile = parsed_profile();
    profile.tools = (0..MAX_TOOLS)
        .map(|index| Tool {
            tool_id: format!("fixture.read-{index:02}"),
            ..profile.tools[0].clone()
        })
        .collect();
    profile.policy.allowed_tool_ids = profile
        .tools
        .iter()
        .map(|tool| tool.tool_id.clone())
        .collect();
    let exact = render_profile(&profile);
    assert_eq!(parse_profile(&exact).unwrap().tools.len(), MAX_TOOLS);
    profile.tools.push(Tool {
        tool_id: "fixture.read-32".to_owned(),
        ..profile.tools[0].clone()
    });
    profile
        .policy
        .allowed_tool_ids
        .push("fixture.read-32".to_owned());
    assert_eq!(profile_error(&render_profile(&profile)).code, "SPX-G205");

    let mut profile = parsed_profile();
    profile.models[0].capabilities = (0..MAX_CAPABILITIES)
        .map(|index| format!("cap-{index:02}"))
        .collect();
    profile.policy.required_model_capabilities = profile.models[0].capabilities.clone();
    assert_eq!(
        parse_profile(&render_profile(&profile)).unwrap().models[0]
            .capabilities
            .len(),
        MAX_CAPABILITIES
    );
    profile.models[0].capabilities.push("cap-64".to_owned());
    profile
        .policy
        .required_model_capabilities
        .push("cap-64".to_owned());
    assert_eq!(profile_error(&render_profile(&profile)).code, "SPX-G205");

    let mut task = parse_task(&fixture_task()).unwrap();
    let empty_length = {
        task.objective.clear();
        super::private::render_task(&task).len()
    };
    task.objective = "x".repeat(MAX_TASK_BYTES - empty_length);
    let exact = super::private::render_task(&task);
    assert_eq!(exact.len(), MAX_TASK_BYTES);
    assert_eq!(
        parse_task(&exact).unwrap().objective.len(),
        MAX_TASK_BYTES - empty_length
    );
    task.objective.push('x');
    let over = super::private::render_task(&task);
    assert_eq!(over.len(), MAX_TASK_BYTES + 1);
    let error = task_error(&over);
    assert_eq!(error.code, "SPX-G208");
    assert_eq!(
        error.message,
        format!("task_bytes exceeds {MAX_TASK_BYTES}")
    );
}

#[test]
fn action_stream_and_usage_hostility_fails_closed() {
    for response in [
        String::from_utf8(final_action("done"))
            .unwrap()
            .trim_end()
            .to_owned(),
        String::from_utf8(final_action("done"))
            .unwrap()
            .replace('\n', "\r\n"),
        String::from_utf8(final_action("done")).unwrap().replacen(
            "\"message\":",
            "\"extra\":0,\"message\":",
            1,
        ),
        String::from_utf8(final_action("done")).unwrap().replacen(
            "\"message\":",
            "\"message\":\"x\",\"message_copy\":",
            1,
        ),
        format!("{{\"schema\":\"{ACTION_SCHEMA}\",\"kind\":\"other\"}}\n"),
    ] {
        let mut host = ScriptHost::final_only("unused");
        host.attempts[0].1 = vec![response.into_bytes()];
        let artifact = new_agent(&fixture_profile(), host)
            .run(&fixture_task())
            .unwrap();
        assert!(artifact.status == RunStatus::ProviderFailed);
        assert!(artifact.evidence.contains("SPX-I218"));
    }

    let response = final_action("done");
    let mut exact_profile = parsed_profile();
    exact_profile.limits.max_stream_chunks = 2;
    exact_profile.limits.max_provider_response_bytes = response.len() as u64;
    let mut exact = ScriptHost::final_only("done");
    exact.attempts[0].1 = vec![response[..1].to_vec(), response[1..].to_vec()];
    let artifact = new_agent(&render_profile(&exact_profile), exact)
        .run(&fixture_task())
        .unwrap();
    assert!(artifact.status == RunStatus::Completed);

    let mut chunks_over = ScriptHost::final_only("done");
    chunks_over.attempts[0].1 = vec![
        response[..1].to_vec(),
        response[1..2].to_vec(),
        response[2..].to_vec(),
    ];
    let artifact = new_agent(&render_profile(&exact_profile), chunks_over)
        .run(&fixture_task())
        .unwrap();
    assert!(artifact.status == RunStatus::BudgetExhausted);
    assert!(artifact.evidence.contains("stream_chunks exceeds 2"));

    let mut usage = ScriptHost::final_only("done");
    usage.attempts[0].2.output_tokens = exact_profile.limits.max_reported_model_output_tokens + 1;
    let artifact = new_agent(&render_profile(&exact_profile), usage)
        .run(&fixture_task())
        .unwrap();
    assert!(artifact.status == RunStatus::ProviderFailed);
    assert!(artifact.evidence.contains("usage invalid"));
}

fn minimum_successful_limit(
    field: fn(&mut EffectiveLimits, u64),
    maximum: u64,
    label: &str,
) -> u64 {
    let succeeds = |limit| {
        let mut profile = parsed_profile();
        field(&mut profile.limits, limit);
        new_agent(&render_profile(&profile), ScriptHost::final_only("done"))
            .run(&fixture_task())
            .is_ok()
    };
    assert!(succeeds(maximum), "{label} upper bound {maximum}");
    let mut low = 0;
    let mut high = maximum;
    while low < high {
        let middle = low + (high - low) / 2;
        if succeeds(middle) {
            high = middle;
        } else {
            low = middle + 1;
        }
    }
    low
}

fn long_provider_profile() -> Profile {
    let mut profile = parsed_profile();
    profile.models[0].provider_id = "p".repeat(MAX_IDENTIFIER_BYTES);
    profile.models[0].model_id = "m".repeat(MAX_IDENTIFIER_BYTES);
    profile.policy.allowed_provider_ids = vec![profile.models[0].provider_id.clone()];
    profile.policy.allowed_model_ids = vec![profile.models[0].model_id.clone()];
    profile
}

fn escaped_task() -> String {
    let mut task = parse_task(&fixture_task()).unwrap();
    task.objective = "quote=\" slash=\\ newline=\n tab=\t".to_owned();
    task.context[0].content = "untrusted \"quoted\" \\ path\nnext\tcell".to_owned();
    super::private::render_task(&task)
}

fn provider_boundary_calls(
    profile: &Profile,
    task: &str,
    field: fn(&mut EffectiveLimits, u64),
    limit: u64,
) -> (Result<AgentRuntimeEvidence, Vec<Diagnostic>>, usize, usize) {
    let mut profile = profile.clone();
    field(&mut profile.limits, limit);
    let host = ScriptHost::final_only("escaped \"done\" \\ value");
    let provider_calls = Rc::clone(&host.provider_calls);
    let tool_calls = Rc::clone(&host.tool_calls);
    let result = new_agent(&render_profile(&profile), host).run(task);
    (result, provider_calls.get(), tool_calls.get())
}

fn minimum_provider_boundary_limit(
    profile: &Profile,
    task: &str,
    field: fn(&mut EffectiveLimits, u64),
    maximum: u64,
) -> u64 {
    assert!(provider_boundary_calls(profile, task, field, maximum).1 > 0);
    let mut low = 0;
    let mut high = maximum;
    while low < high {
        let middle = low + (high - low) / 2;
        if provider_boundary_calls(profile, task, field, middle).1 > 0 {
            high = middle;
        } else {
            low = middle + 1;
        }
    }
    low
}

fn long_tool_host(tool_id: &str) -> ScriptHost {
    let mut host = ScriptHost::final_only("done");
    host.attempts = vec![(
        ProviderDisposition::Succeeded,
        vec![tool_action(tool_id, "{\"query\":\"q\\\"\\\\\\n\\t\"}")],
        ProviderUsage::default(),
    )];
    host.tool_failure = true;
    host
}

fn tool_boundary_calls(
    profile: &Profile,
    field: fn(&mut EffectiveLimits, u64),
    limit: u64,
) -> (Result<AgentRuntimeEvidence, Vec<Diagnostic>>, usize, usize) {
    let mut profile = profile.clone();
    field(&mut profile.limits, limit);
    let host = long_tool_host(&profile.tools[0].tool_id);
    let provider_calls = Rc::clone(&host.provider_calls);
    let tool_calls = Rc::clone(&host.tool_calls);
    let result = new_agent(&render_profile(&profile), host).run(&fixture_task());
    (result, provider_calls.get(), tool_calls.get())
}

fn minimum_tool_boundary_limit(
    profile: &Profile,
    field: fn(&mut EffectiveLimits, u64),
    maximum: u64,
) -> u64 {
    assert!(tool_boundary_calls(profile, field, maximum).2 > 0);
    let mut low = 0;
    let mut high = maximum;
    while low < high {
        let middle = low + (high - low) / 2;
        if tool_boundary_calls(profile, field, middle).2 > 0 {
            high = middle;
        } else {
            low = middle + 1;
        }
    }
    low
}

#[test]
fn long_identifiers_and_escaping_are_counted_before_external_boundaries() {
    let profile = long_provider_profile();
    let task = escaped_task();
    for (field, maximum, label) in [
        (
            (|limits: &mut EffectiveLimits, value| limits.max_trace_bytes = value)
                as fn(&mut EffectiveLimits, u64),
            profile.limits.max_trace_bytes,
            "trace_bytes",
        ),
        (
            (|limits: &mut EffectiveLimits, value| limits.max_evidence_bytes = value)
                as fn(&mut EffectiveLimits, u64),
            profile.limits.max_evidence_bytes,
            "evidence_bytes",
        ),
    ] {
        let high = provider_boundary_calls(&profile, &task, field, maximum)
            .0
            .unwrap();
        let actual_bytes = if label == "trace_bytes" {
            high.trace.len()
        } else {
            high.evidence.len()
        };
        let minimum = minimum_provider_boundary_limit(&profile, &task, field, maximum);
        let (exact, provider_calls, tool_calls) =
            provider_boundary_calls(&profile, &task, field, minimum);
        assert!(
            exact.is_ok(),
            "{label} exact {minimum}, actual {actual_bytes}"
        );
        assert_eq!(provider_calls, 1);
        assert_eq!(tool_calls, 0);
        let (over, provider_calls, tool_calls) =
            provider_boundary_calls(&profile, &task, field, minimum - 1);
        let error = diagnostic(match over {
            Ok(_) => panic!("{label} minimum-minus-one unexpectedly succeeded"),
            Err(error) => error,
        });
        assert_eq!(error.0, "SPX-G208");
        assert_eq!(error.1, format!("{label} exceeds {}", minimum - 1));
        assert_eq!(provider_calls, 0);
        assert_eq!(tool_calls, 0);
    }

    let mut profile = parsed_profile();
    profile.tools[0].tool_id = "t".repeat(MAX_IDENTIFIER_BYTES);
    profile.policy.allowed_tool_ids = vec![profile.tools[0].tool_id.clone()];
    for (field, maximum, label) in [
        (
            (|limits: &mut EffectiveLimits, value| limits.max_trace_bytes = value)
                as fn(&mut EffectiveLimits, u64),
            profile.limits.max_trace_bytes,
            "trace_bytes",
        ),
        (
            (|limits: &mut EffectiveLimits, value| limits.max_evidence_bytes = value)
                as fn(&mut EffectiveLimits, u64),
            profile.limits.max_evidence_bytes,
            "evidence_bytes",
        ),
    ] {
        let high = tool_boundary_calls(&profile, field, maximum).0.unwrap();
        let actual_bytes = if label == "trace_bytes" {
            high.trace.len()
        } else {
            high.evidence.len()
        };
        let minimum = minimum_tool_boundary_limit(&profile, field, maximum);
        let (exact, provider_calls, tool_calls) = tool_boundary_calls(&profile, field, minimum);
        assert!(
            exact.is_ok(),
            "{label} exact {minimum}, actual {actual_bytes}"
        );
        assert_eq!(provider_calls, 1);
        assert_eq!(tool_calls, 1);
        let (over, provider_calls, tool_calls) = tool_boundary_calls(&profile, field, minimum - 1);
        let artifact = over.unwrap();
        assert!(artifact.status == RunStatus::BudgetExhausted);
        assert!(artifact
            .evidence
            .contains(&format!("{label} exceeds {}", minimum - 1)));
        assert_eq!(provider_calls, 1);
        assert_eq!(tool_calls, 0);
    }
}

fn history_stress_profile(payload_bytes: usize, retained_limit: u64) -> String {
    let mut profile = parsed_profile();
    profile.limits.max_trace_bytes = MAX_TRACE_BYTES;
    profile.limits.max_evidence_bytes = MAX_EVIDENCE_BYTES;
    profile.limits.max_builder_bytes = MAX_BUILDER_BYTES as u64;
    profile.limits.max_provider_request_bytes = MAX_PROVIDER_REQUEST_BYTES;
    profile.limits.max_total_provider_input_bytes = MAX_TOTAL_PROVIDER_INPUT_BYTES;
    profile.limits.max_reported_model_input_tokens = MAX_REPORTED_MODEL_INPUT_TOKENS;
    profile.models[0].max_context_tokens = MAX_REPORTED_MODEL_INPUT_TOKENS;
    profile.limits.max_tool_result_bytes = payload_bytes as u64 + 512;
    profile.limits.max_total_tool_bytes = MAX_TOTAL_TOOL_BYTES;
    profile.limits.max_retained_state_bytes = retained_limit;
    profile.tools[0].result_schema.fields[0].max_bytes = payload_bytes as u64;
    render_profile(&profile)
}

fn history_stress_run(
    payload_bytes: usize,
    retained_limit: u64,
) -> (Result<AgentRuntimeEvidence, Vec<Diagnostic>>, usize, usize) {
    let profile_source = history_stress_profile(payload_bytes, retained_limit);
    let mut host = ScriptHost::final_only("done");
    host.attempts = vec![
        (
            ProviderDisposition::Succeeded,
            vec![tool_action("fixture.read", "{\"query\":\"alpha\"}")],
            ProviderUsage::default(),
        ),
        (
            ProviderDisposition::Succeeded,
            vec![final_action("done")],
            ProviderUsage::default(),
        ),
    ];
    host.tool_result =
        vec![format!("{{\"value\":\"{}\"}}", "x".repeat(payload_bytes)).into_bytes()];
    let provider_calls = Rc::clone(&host.provider_calls);
    let tool_calls = Rc::clone(&host.tool_calls);
    let result = new_agent(&profile_source, host).run(&fixture_task());
    (result, provider_calls.get(), tool_calls.get())
}

#[test]
fn cumulative_builder_and_retained_history_boundaries_are_exact() {
    let succeeds = |payload| {
        history_stress_run(payload, MAX_RETAINED_STATE_BYTES)
            .0
            .is_ok_and(|artifact| artifact.status == RunStatus::Completed)
    };
    assert!(succeeds(1));
    let mut low = 1usize;
    let mut high = 500_000usize;
    assert!(!succeeds(high));
    while low + 1 < high {
        let middle = low + (high - low) / 2;
        if succeeds(middle) {
            low = middle;
        } else {
            high = middle;
        }
    }
    let (exact, provider_calls, tool_calls) = history_stress_run(low, MAX_RETAINED_STATE_BYTES);
    assert!(exact.unwrap().status == RunStatus::Completed);
    assert_eq!(provider_calls, 2);
    assert_eq!(tool_calls, 1);
    let (over, provider_calls, tool_calls) = history_stress_run(high, MAX_RETAINED_STATE_BYTES);
    let over = match over {
        Ok(artifact) => artifact,
        Err(error) => panic!("payload={high} post-effect boundary returned top-level error with provider_calls={provider_calls}, tool_calls={tool_calls}: {error:?}"),
    };
    assert!(over.status == RunStatus::BudgetExhausted);
    assert!(over.evidence.contains("builder_bytes exceeds 67108864"));
    assert_eq!(provider_calls, 1);
    assert_eq!(tool_calls, 1);

    let payload = 65_536;
    let (probe, provider_calls, tool_calls) = history_stress_run(payload, MAX_RETAINED_STATE_BYTES);
    let probe = probe.unwrap();
    assert!(probe.status == RunStatus::Completed);
    assert_eq!(provider_calls, 2);
    assert_eq!(tool_calls, 1);
    let trace: Value = serde_json::from_str(probe.trace.trim_end()).unwrap();
    let retained = trace["usage"]["retained_state_bytes"].as_u64().unwrap();
    assert!(retained > payload as u64);

    let (exact, provider_calls, tool_calls) = history_stress_run(payload, retained);
    assert!(exact.unwrap().status == RunStatus::Completed);
    assert_eq!(provider_calls, 2);
    assert_eq!(tool_calls, 1);
    let (over, provider_calls, tool_calls) = history_stress_run(payload, retained - 1);
    let over = over.unwrap();
    assert!(over.status == RunStatus::BudgetExhausted);
    assert!(over
        .evidence
        .contains(&format!("retained_state_bytes exceeds {}", retained - 1)));
    assert_eq!(provider_calls, 1);
    assert_eq!(tool_calls, 1);
    let trace: Value = serde_json::from_str(over.trace.trim_end()).unwrap();
    let events = trace["events"].as_array().unwrap();
    let tool_finished = events
        .iter()
        .find(|event| event["kind"] == "tool_finished")
        .unwrap();
    assert_eq!(tool_finished["status"], "failed");
    assert!(tool_finished["output_digest"].is_null());
    assert!(
        tool_finished["usage"]["tool_result_bytes"]
            .as_u64()
            .unwrap()
            > payload as u64
    );
    assert_eq!(trace["usage"]["retained_state_bytes"], 0);
    assert_eq!(trace["usage"]["provider_attempts"], 1);
    assert_eq!(trace["usage"]["tool_calls"], 1);
}

#[test]
fn terminal_preflight_counts_the_longest_g204_after_nonempty_history_exactly() {
    const G204_MESSAGE: &str =
        "Agent Runtime provider request is not canonical semaprax.agent-runtime-provider-request.v1 JSON";
    assert!(terminal_diagnostics_for_test().contains(&(
        "policy_rejected",
        "SPX-G204",
        G204_MESSAGE,
    )));

    let profile_source = history_stress_profile(1, MAX_RETAINED_STATE_BYTES);
    let (artifact, provider_calls, tool_calls) = history_stress_run(1, MAX_RETAINED_STATE_BYTES);
    let artifact = artifact.unwrap();
    assert!(artifact.status == RunStatus::Completed);
    assert_eq!(provider_calls, 2);
    assert_eq!(tool_calls, 1);
    let trace: Value = serde_json::from_str(artifact.trace.trim_end()).unwrap();
    let events = trace["events"].as_array().unwrap();
    assert_eq!(
        events
            .iter()
            .filter(|event| event["kind"] == "provider_attempt_started")
            .count(),
        2
    );
    assert!(events
        .iter()
        .any(|event| event["kind"] == "tool_finished" && event["status"] == "succeeded"));

    let minimum = |trace_limit: bool| {
        let mut rejected = 0u64;
        let mut admitted = if trace_limit {
            MAX_TRACE_BYTES
        } else {
            MAX_EVIDENCE_BYTES
        };
        assert!(preflight_terminal_for_test(
            &profile_source,
            &artifact,
            if trace_limit {
                admitted
            } else {
                MAX_TRACE_BYTES
            },
            if trace_limit {
                MAX_EVIDENCE_BYTES
            } else {
                admitted
            },
        )
        .is_ok());
        while rejected + 1 < admitted {
            let middle = rejected + (admitted - rejected) / 2;
            if preflight_terminal_for_test(
                &profile_source,
                &artifact,
                if trace_limit { middle } else { MAX_TRACE_BYTES },
                if trace_limit {
                    MAX_EVIDENCE_BYTES
                } else {
                    middle
                },
            )
            .is_ok()
            {
                admitted = middle;
            } else {
                rejected = middle;
            }
        }
        admitted
    };

    let trace_minimum = minimum(true);
    assert!(preflight_terminal_for_test(
        &profile_source,
        &artifact,
        trace_minimum,
        MAX_EVIDENCE_BYTES,
    )
    .is_ok());
    let error = preflight_terminal_for_test(
        &profile_source,
        &artifact,
        trace_minimum - 1,
        MAX_EVIDENCE_BYTES,
    )
    .unwrap_err();
    assert_eq!(error.code, "SPX-G208");
    assert_eq!(
        error.message,
        format!("trace_bytes exceeds {}", trace_minimum - 1)
    );

    let evidence_minimum = minimum(false);
    assert!(preflight_terminal_for_test(
        &profile_source,
        &artifact,
        MAX_TRACE_BYTES,
        evidence_minimum,
    )
    .is_ok());
    let error = preflight_terminal_for_test(
        &profile_source,
        &artifact,
        MAX_TRACE_BYTES,
        evidence_minimum - 1,
    )
    .unwrap_err();
    assert_eq!(error.code, "SPX-G208");
    assert_eq!(
        error.message,
        format!("evidence_bytes exceeds {}", evidence_minimum - 1)
    );
}

#[test]
fn trace_evidence_and_builder_caps_have_exact_minimum_boundaries() {
    let fixture_limits = parsed_profile().limits;
    for (field, maximum, label) in [
        (
            (|limits: &mut EffectiveLimits, value| limits.max_trace_bytes = value)
                as fn(&mut EffectiveLimits, u64),
            fixture_limits.max_trace_bytes,
            "trace_bytes",
        ),
        (
            (|limits: &mut EffectiveLimits, value| limits.max_evidence_bytes = value)
                as fn(&mut EffectiveLimits, u64),
            fixture_limits.max_evidence_bytes,
            "evidence_bytes",
        ),
        (
            (|limits: &mut EffectiveLimits, value| limits.max_builder_bytes = value)
                as fn(&mut EffectiveLimits, u64),
            fixture_limits.max_builder_bytes,
            "builder_bytes",
        ),
    ] {
        let minimum = minimum_successful_limit(field, maximum, label);
        assert!(minimum > 0);
        let mut exact = parsed_profile();
        field(&mut exact.limits, minimum);
        let exact_host = ScriptHost::final_only("done");
        let exact_provider_calls = Rc::clone(&exact_host.provider_calls);
        assert!(new_agent(&render_profile(&exact), exact_host)
            .run(&fixture_task())
            .is_ok());
        assert_eq!(exact_provider_calls.get(), 1);
        let mut over = parsed_profile();
        field(&mut over.limits, minimum - 1);
        let over_host = ScriptHost::final_only("done");
        let over_provider_calls = Rc::clone(&over_host.provider_calls);
        let over_tool_calls = Rc::clone(&over_host.tool_calls);
        let result = new_agent(&render_profile(&over), over_host).run(&fixture_task());
        match result {
            Ok(artifact) => {
                assert_eq!(label, "builder_bytes");
                assert!(artifact.status == RunStatus::BudgetExhausted);
                assert!(artifact
                    .evidence
                    .contains(&format!("{label} exceeds {}", minimum - 1)));
            }
            Err(error) => {
                let error = diagnostic(error);
                assert_eq!(error.0, "SPX-G208", "{label} minimum {minimum}");
                assert!(error.1.starts_with(label));
                if label != "builder_bytes" {
                    assert_eq!(
                        over_provider_calls.get(),
                        0,
                        "{label} provider-call boundary"
                    );
                }
            }
        }
        assert_eq!(over_tool_calls.get(), 0, "{label} crossed tool");
    }
}
