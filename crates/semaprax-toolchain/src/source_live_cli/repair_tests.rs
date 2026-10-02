use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use super::*;
use crate::opencode_host::{OpenCodeHostConfig, OpenCodeRunner, OpenCodeRunnerFailure};

#[path = "repair_tests/fixed_candidate_test.rs"]
mod fixed_candidate_test;

static NEXT: AtomicU64 = AtomicU64::new(0);

struct Fixture(PathBuf);
impl Fixture {
    fn new() -> Self {
        let parent = std::env::temp_dir().canonicalize().unwrap();
        let path = parent.join(format!(
            "spx-source-live-repair-test-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir_all(path.join("project/src")).unwrap();
        Self(path)
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

#[path = "repair_tests/windows.rs"]
#[cfg(windows)]
mod windows;

macro_rules! unix_checkpoint_host {
    () => {
        #[cfg(windows)]
        {
            windows::assert_checkpoint_host_refusal();
            return;
        }
    };
}

const MANIFEST: &str = include_str!("../../../../examples/offline-repair-project/semaprax.toml");
const APP: &str = include_str!("../../../../examples/offline-repair-project/src/app.spx");
const TESTS_SPX: &str = include_str!("../../../../examples/offline-repair-project/src/tests.spx");

/// Writes a fresh, host-selected copy of the checked-in offline-repair-demo
/// Project into the fixture, with `app_source` as its one mutable module.
/// This proves the candidate-preview machinery below is no longer hardwired
/// to a single compiled-in manifest path: it is driven entirely off the
/// operand config, same as any other host-selected `.spx` Project.
fn write_project(fixture: &Fixture, app_source: &str) -> PathBuf {
    let root = fixture.0.join("project");
    fs::create_dir_all(root.join("src")).unwrap();
    fs::write(root.join("semaprax.toml"), MANIFEST).unwrap();
    fs::write(root.join("src/app.spx"), app_source).unwrap();
    fs::write(root.join("src/tests.spx"), TESTS_SPX).unwrap();
    root.join("semaprax.toml")
}

fn project_tree(root: &Path) -> Vec<(PathBuf, Vec<u8>)> {
    fn visit(root: &Path, path: &Path, files: &mut Vec<(PathBuf, Vec<u8>)>) {
        let mut entries = fs::read_dir(path)
            .unwrap()
            .map(|entry| entry.unwrap())
            .collect::<Vec<_>>();
        entries.sort_by_key(|entry| entry.path());
        for entry in entries {
            let path = entry.path();
            if entry.file_type().unwrap().is_dir() {
                visit(root, &path, files);
            } else {
                files.push((
                    path.strip_prefix(root).unwrap().to_owned(),
                    fs::read(path).unwrap(),
                ));
            }
        }
    }
    let mut files = Vec::new();
    visit(root, root, &mut files);
    files
}

fn schema_digest(app_source: &str) -> String {
    let compiled = compile_source_agent_lifecycle_v2(
        app_source,
        "src/app.spx",
        "fixture.agent",
        "fixture.agent.type.step",
    )
    .unwrap();
    compiled.proposal_schema().schema().digest().to_owned()
}

fn proposal(schema_digest: &str, budget: &str, sequence: &str) -> String {
    format!(
        concat!(
            "{{\"schema\":\"semaprax.agent-proposal.v1\",\"agent_id\":\"fixture.agent\",",
            "\"proposal_schema_digest\":\"{schema_digest}\",\"value\":{{\"fields\":{{",
            "\"fixture.agent.type.proposal.budget\":\"{budget}\",",
            "\"fixture.agent.type.proposal.urgent\":false,",
            "\"fixture.agent.type.proposal.sequence\":\"{sequence}\"}}}}}}\n"
        ),
        schema_digest = schema_digest,
        budget = budget,
        sequence = sequence,
    )
}

fn decode_hex(text: &str) -> Vec<u8> {
    assert_eq!(text.len() % 2, 0, "feedback hex has whole bytes");
    text.as_bytes()
        .chunks_exact(2)
        .map(|pair| {
            std::str::from_utf8(pair)
                .ok()
                .and_then(|pair| u8::from_str_radix(pair, 16).ok())
                .expect("feedback is lowercase hexadecimal")
        })
        .collect()
}

#[allow(clippy::too_many_arguments)]
fn repair_config_value(
    manifest: &Path,
    task_path: &Path,
    schema_digest: &str,
    migration_id: &str,
) -> serde_json::Value {
    serde_json::json!({
        "schema": "semaprax.source-live-cli.repair-config.v1",
        "manifest": manifest.to_str().unwrap(),
        "source_path": "src/app.spx",
        "agent_id": "fixture.agent",
        "step_id": "fixture.agent.type.step",
        "selector_field_id": "fixture.agent.type.proposal.sequence",
        "deployment_migration_id": migration_id,
        "target": "fixture.repair.value",
        "malformed_operation_id": "fixture.read",
        "corrected_operation_id": "fixture.read.second",
        "effect_id": "read",
        "argument_id": "query",
        "proposal_field_id": "fixture.agent.type.proposal.budget",
        "result_id": "value",
        "task_path": task_path.to_str().unwrap(),
        "task_budget": 12,
        "deadline_millis": 1000,
        "ceiling": 2,
        "reservation_units": 1,
        "max_total_steps": 2_000_000,
        "effect_budget": {
            "max_calls": 2,
            "max_argument_bytes": 4096,
            "max_result_bytes": 4096,
            "max_total_bytes": 8192,
        },
        "malformed_replacement": 0,
        "malformed_bool_literal": true,
        "turns": [
            {"document": proposal(schema_digest, "0", "0"), "requires_prior_feedback": false},
            {"document": proposal(schema_digest, "7", "1"), "requires_prior_feedback": true},
        ],
    })
}

fn write_config(fixture: &Fixture, value: &serde_json::Value) -> PathBuf {
    let path = fixture.0.join("config.json");
    fs::write(&path, serde_json::to_vec(value).unwrap()).unwrap();
    path
}

fn run_repair(verb: &str, config: &Path, checkpoint: &Path) -> Result<String, CliError> {
    run(&[
        verb.to_owned(),
        config.to_str().unwrap().to_owned(),
        checkpoint.to_str().unwrap().to_owned(),
    ])
}

struct RecordedOpenCodeRunner {
    answers: VecDeque<String>,
    last_answer: Option<String>,
    prompts: Rc<RefCell<Vec<String>>>,
    calls: Rc<Cell<usize>>,
}

#[derive(Clone)]
struct CandidateTestSubjectFact {
    capability: String,
    candidate_revision: String,
    base_project_revision: String,
    source_revision: String,
    candidate_source: String,
}

enum CandidateTestReply {
    Canonical {
        status: &'static str,
        detail: &'static str,
    },
    CanonicalOverride {
        field: &'static str,
        value: &'static str,
    },
    ExtraField,
    DuplicateStatus,
    ExtraNewline,
    Raw(Vec<u8>),
}

struct RecordedCandidateTestObserver {
    reply: CandidateTestReply,
    calls: Rc<Cell<usize>>,
    subjects: Rc<RefCell<Vec<CandidateTestSubjectFact>>>,
}

impl CandidateTestObserver for RecordedCandidateTestObserver {
    fn observe(
        &mut self,
        _: &CandidateTestCapability,
        subject: &CandidateTestSubject,
    ) -> Result<CandidateTestObservation, CandidateTestObservationError> {
        self.calls.set(self.calls.get() + 1);
        let candidate_source = subject
            .candidate()
            .revision()
            .sources()
            .iter()
            .find(|source| source.path() == "src/app.spx")
            .expect("candidate test receives the exact selected source")
            .source()
            .to_owned();
        self.subjects.borrow_mut().push(CandidateTestSubjectFact {
            capability: subject.capability().to_owned(),
            candidate_revision: subject.candidate_revision().to_owned(),
            base_project_revision: subject.base_project_revision().to_owned(),
            source_revision: subject.source_revision().to_owned(),
            candidate_source,
        });
        match &self.reply {
            CandidateTestReply::Canonical { status, detail } => {
                CandidateTestObservation::try_from_bytes(
                    &serde_json::to_vec(&serde_json::json!({
                        "schema": CANDIDATE_TEST_SCHEMA,
                        "capability": subject.capability(),
                        "candidate_revision": subject.candidate_revision(),
                        "base_project_revision": subject.base_project_revision(),
                        "source_revision": subject.source_revision(),
                        "status": status,
                        "detail": detail,
                    }))
                    .unwrap(),
                )
                .map_err(|_| CandidateTestObservationError)
            }
            CandidateTestReply::CanonicalOverride {
                field,
                value: override_value,
            } => {
                let mut document = serde_json::json!({
                    "schema": CANDIDATE_TEST_SCHEMA,
                    "capability": subject.capability(),
                    "candidate_revision": subject.candidate_revision(),
                    "base_project_revision": subject.base_project_revision(),
                    "source_revision": subject.source_revision(),
                    "status": "passed",
                    "detail": "candidate test passed",
                });
                document[*field] = serde_json::json!(override_value);
                CandidateTestObservation::try_from_bytes(&serde_json::to_vec(&document).unwrap())
            }
            CandidateTestReply::ExtraField => {
                let mut value = serde_json::json!({
                    "schema": CANDIDATE_TEST_SCHEMA,
                    "capability": subject.capability(),
                    "candidate_revision": subject.candidate_revision(),
                    "base_project_revision": subject.base_project_revision(),
                    "source_revision": subject.source_revision(),
                    "status": "passed",
                    "detail": "candidate test passed",
                });
                value["extra"] = serde_json::json!("unexpected");
                CandidateTestObservation::try_from_bytes(&serde_json::to_vec(&value).unwrap())
            }
            CandidateTestReply::DuplicateStatus => {
                let value = serde_json::to_string(&serde_json::json!({
                    "schema": CANDIDATE_TEST_SCHEMA,
                    "capability": subject.capability(),
                    "candidate_revision": subject.candidate_revision(),
                    "base_project_revision": subject.base_project_revision(),
                    "source_revision": subject.source_revision(),
                    "status": "passed",
                    "detail": "candidate test passed",
                }))
                .unwrap()
                .replacen(
                    "\"status\":\"passed\"",
                    "\"status\":\"passed\",\"status\":\"passed\"",
                    1,
                );
                CandidateTestObservation::try_from_bytes(value.as_bytes())
            }
            CandidateTestReply::ExtraNewline => {
                let mut value = serde_json::to_vec(&serde_json::json!({
                    "schema": CANDIDATE_TEST_SCHEMA,
                    "capability": subject.capability(),
                    "candidate_revision": subject.candidate_revision(),
                    "base_project_revision": subject.base_project_revision(),
                    "source_revision": subject.source_revision(),
                    "status": "passed",
                    "detail": "candidate test passed",
                }))
                .unwrap();
                value.extend_from_slice(b"\n\n");
                CandidateTestObservation::try_from_bytes(&value)
            }
            CandidateTestReply::Raw(bytes) => CandidateTestObservation::try_from_bytes(bytes),
        }
    }
}

impl OpenCodeRunner for RecordedOpenCodeRunner {
    fn run(
        &mut self,
        _: &OpenCodeHostConfig,
        prompt: &str,
    ) -> Result<Vec<u8>, OpenCodeRunnerFailure> {
        let answer = self
            .answers
            .pop_front()
            .expect("recorded OpenCode answer is available");
        self.calls.set(self.calls.get() + 1);
        self.prompts.borrow_mut().push(prompt.to_owned());
        self.last_answer = Some(answer.clone());
        Ok(recorded_transport(prompt, &answer).0)
    }

    fn export(
        &mut self,
        _: &OpenCodeHostConfig,
        _: &str,
    ) -> Result<Vec<u8>, OpenCodeRunnerFailure> {
        let prompt = self
            .prompts
            .borrow()
            .last()
            .expect("OpenCode export follows run")
            .clone();
        Ok(recorded_transport(
            &prompt,
            self.last_answer
                .as_deref()
                .expect("OpenCode answer was retained"),
        )
        .1)
    }
}

/// Deterministically models an external source edit after the durable route
/// has received a real provider response. The production post-run snapshot
/// recheck, not this runner, must select the refusal.
struct SourceMutatingOpenCodeRunner {
    inner: RecordedOpenCodeRunner,
    source_path: PathBuf,
    replacement: Vec<u8>,
    mutated: bool,
}

impl OpenCodeRunner for SourceMutatingOpenCodeRunner {
    fn run(
        &mut self,
        config: &OpenCodeHostConfig,
        prompt: &str,
    ) -> Result<Vec<u8>, OpenCodeRunnerFailure> {
        let transport = self.inner.run(config, prompt)?;
        if !self.mutated {
            fs::write(&self.source_path, &self.replacement)
                .expect("the test source mutation must be writable");
            self.mutated = true;
        }
        Ok(transport)
    }

    fn export(
        &mut self,
        config: &OpenCodeHostConfig,
        prompt: &str,
    ) -> Result<Vec<u8>, OpenCodeRunnerFailure> {
        self.inner.export(config, prompt)
    }
}

fn recorded_transport(prompt: &str, answer: &str) -> (Vec<u8>, Vec<u8>) {
    let mut export: serde_json::Value = serde_json::from_str(include_str!(
        "../../../../scripts/fixtures/opencode-provider-smoke-v1/session.json"
    ))
    .expect("checked OpenCode transport fixture");
    let presented = if prompt.contains(' ') {
        format!("\"{}\"", prompt.replace('"', "\\\""))
    } else {
        prompt.to_owned()
    };
    export["messages"][0]["parts"][0]["text"] = serde_json::json!(presented);
    export["messages"][1]["parts"][2]["text"] = serde_json::json!(answer);
    let parts = export["messages"][1]["parts"]
        .as_array()
        .expect("checked OpenCode transport parts");
    let events = [
        ("step_start", &parts[0]),
        ("text", &parts[2]),
        ("step_finish", &parts[3]),
    ]
    .iter()
    .map(|(kind, part)| {
        serde_json::json!({"type": kind, "sessionID": "ses_fixture", "part": part}).to_string()
    })
    .collect::<Vec<_>>()
    .join("\n")
    .into_bytes();
    (
        events,
        serde_json::to_vec(&export).expect("canonical OpenCode export"),
    )
}

fn v2_command(verb: &str, config: PathBuf, checkpoint: PathBuf, scratch: PathBuf) -> Command {
    let executable = scratch
        .parent()
        .expect("fixture scratch has a parent")
        .join("opencode-fixture-executable");
    if !executable.exists() {
        fs::write(&executable, b"fixed injected-runner executable identity").unwrap();
    }
    let provider = OpenCodeOperands {
        executable,
        scratch,
    };
    match verb {
        "run" => Command::Run {
            config,
            checkpoint,
            provider: Some(provider),
        },
        "resume" => Command::Resume {
            config,
            checkpoint,
            provider: Some(provider),
        },
        _ => panic!("test helper only accepts run or resume"),
    }
}

fn v2_config(fixture: &Fixture, config: &Path) -> PathBuf {
    let mut value: serde_json::Value = serde_json::from_slice(&fs::read(config).unwrap()).unwrap();
    value["schema"] = serde_json::json!(CONFIG_SCHEMA_V2);
    value.as_object_mut().unwrap().remove("turns");
    let deadline = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("Unix epoch clock")
        .checked_add(Duration::from_secs(10 * 60))
        .and_then(|duration| i64::try_from(duration.as_millis()).ok())
        .expect("bounded V2 deadline");
    value["deadline_millis"] = serde_json::json!(deadline);
    let path = fixture.0.join("config-v2.json");
    fs::write(&path, serde_json::to_vec(&value).unwrap()).unwrap();
    path
}

fn opencode_app_source() -> String {
    const OPENCODE_SOURCE_MODEL: &str = "muse-spark-1.3-contributor-free";
    let source = APP
        .replace("fake.local", "opencode")
        .replace("fake-basic", OPENCODE_SOURCE_MODEL);
    assert!(
        !source.contains("fake.local") && !source.contains("fake-basic"),
        "the V2 fixture must admit only the exact OpenCode model pair"
    );
    assert!(
        source.contains(&format!(r#"\"model_id\":\"{OPENCODE_SOURCE_MODEL}\""#))
            && source.contains(&format!(
                r#"\"allowed_model_ids\":[\"{OPENCODE_SOURCE_MODEL}\"]"#
            )),
        "the V2 fixture must bind the adapter's full provider/model identity"
    );
    source
}

#[test]
fn opencode_source_binding_uses_the_unprefixed_checked_model_identity() {
    let raw = source_model_identity(Path::new("/bin/true"), Path::new("/scratch"));
    let adapter_identity = raw.adapter_identity.clone();
    let bound = source_deployment_identity(raw).unwrap();
    assert_eq!(bound.provider_id, "opencode");
    assert_eq!(bound.model_id, "muse-spark-1.3-contributor-free");
    assert_eq!(bound.adapter_identity, adapter_identity);

    let invalid = SourceModelAdapterIdentity {
        provider_id: "opencode".into(),
        model_id: "muse-spark-1.3-contributor-free".into(),
        adapter_identity: "fixture".into(),
        adapter_version: "1".into(),
        provider_profile: "fixture".into(),
    };
    assert!(source_deployment_identity(invalid).is_err());
}

fn setup_v2(fixture: &Fixture, migration_id: &str) -> (PathBuf, PathBuf, String) {
    let app_source = opencode_app_source();
    let manifest = write_project(fixture, &app_source).canonicalize().unwrap();
    let digest = schema_digest(&app_source);
    let task_path = fixture.0.join("task.txt");
    fs::write(&task_path, b"repair the checked candidate").unwrap();
    let v1_config = write_config(
        fixture,
        &repair_config_value(&manifest, &task_path, &digest, migration_id),
    );
    let config = v2_config(fixture, &v1_config);
    let checkpoint = fixture.0.join("checkpoint");
    (config, checkpoint, digest)
}

fn setup_v2_with_feedback_turn(
    fixture: &Fixture,
    migration_id: &str,
) -> (PathBuf, PathBuf, String) {
    let app_source = opencode_app_source()
        .replacen("sequence <= 1usize", "sequence <= 2usize", 1)
        .replacen("state.epoch < 2", "state.epoch < 3", 1)
        .replacen(r#"\"max_tool_calls\":2"#, r#"\"max_tool_calls\":3"#, 1);
    assert!(app_source.contains("sequence <= 2usize"));
    assert!(app_source.contains("state.epoch < 3"));
    assert!(app_source.contains(r#"\"max_tool_calls\":3"#));
    let manifest = write_project(fixture, &app_source).canonicalize().unwrap();
    let digest = schema_digest(&app_source);
    let task_path = fixture.0.join("task.txt");
    fs::write(&task_path, b"repair the checked candidate").unwrap();
    let v1_config = write_config(
        fixture,
        &repair_config_value(&manifest, &task_path, &digest, migration_id),
    );
    let config = v2_config(fixture, &v1_config);
    let mut value: serde_json::Value = serde_json::from_slice(&fs::read(&config).unwrap()).unwrap();
    value["ceiling"] = serde_json::json!(3);
    value["effect_budget"]["max_calls"] = serde_json::json!(3);
    fs::write(&config, serde_json::to_vec(&value).unwrap()).unwrap();
    let checkpoint = fixture.0.join("checkpoint");
    (config, checkpoint, digest)
}

fn setup(fixture: &Fixture, migration_id: &str) -> (PathBuf, PathBuf) {
    let manifest = write_project(fixture, APP).canonicalize().unwrap();
    let digest = schema_digest(APP);
    let task_path = fixture.0.join("task.txt");
    fs::write(&task_path, b"repair the checked candidate").unwrap();
    let config_value = repair_config_value(&manifest, &task_path, &digest, migration_id);
    let config = write_config(fixture, &config_value);
    let checkpoint = fixture.0.join("checkpoint");
    (config, checkpoint)
}

// CheckpointDir deliberately refuses on Windows. Each durable run/replay test
// verifies that refusal there and exercises the full success path on Unix.
/// Generalization: the exact candidate-preview/source-diff/semantic-impact
/// evidence the fixed `offline-repair` demo produces is reachable for a
/// host-selected Project and target through operand config alone, with a
/// durable (resumable) checkpoint rather than an in-memory-only journal.
#[test]
fn repair_run_generalizes_candidate_preview_for_a_host_selected_project() {
    unix_checkpoint_host!();
    let fixture = Fixture::new();
    let (config, checkpoint) = setup(&fixture, "test.repair.run.v1");

    let rendered = run_repair("run", &config, &checkpoint).unwrap();
    let report: serde_json::Value = serde_json::from_str(&rendered).unwrap();
    assert_eq!(report["schema"], RECEIPT_SCHEMA_V1);
    assert_eq!(report["target"], "fixture.repair.value");
    assert_eq!(report["status"], "complete");
    assert_eq!(report["model_dispatches"], 2);
    assert_eq!(report["effect_dispatches"], 2);
    assert_eq!(report["rejected_candidates"], 1);
    assert_eq!(report["source_mutation"], false);
    assert_eq!(report["publication_authority"], false);
    assert!(report["candidate_digest"].is_string());
    assert!(report["source_review"].is_object());
    assert!(report["semantic_delta"].is_object());
    assert!(report["impact_summary"].is_object());
    let mut keys = report
        .as_object()
        .unwrap()
        .keys()
        .cloned()
        .collect::<Vec<_>>();
    keys.sort();
    assert_eq!(
        keys,
        [
            "candidate_digest",
            "effect_dispatches",
            "generation",
            "impact_summary",
            "model_dispatches",
            "publication_authority",
            "rejected_candidates",
            "schema",
            "semantic_delta",
            "source_mutation",
            "source_review",
            "status",
            "target",
        ],
        "the V1 scripted receipt remains a frozen projection; new repair evidence belongs only in V2",
    );
}

/// Ordering property: replay happens before staging or candidate creation.
/// A resumed terminal checkpoint redispatches no model or effect call and
/// fabricates no fresh candidate evidence; only the exact live invocation
/// that produced it holds that evidence.
#[test]
fn repair_resume_replays_terminal_checkpoint_without_redispatch_or_refabricated_evidence() {
    unix_checkpoint_host!();
    let fixture = Fixture::new();
    let (config, checkpoint) = setup(&fixture, "test.repair.resume.v1");

    let first = run_repair("run", &config, &checkpoint).unwrap();
    let first_report: serde_json::Value = serde_json::from_str(&first).unwrap();
    assert_eq!(first_report["model_dispatches"], 2);
    assert!(first_report["candidate_digest"].is_string());

    let checkpoint_document = checkpoint.join("checkpoint.json");
    let bytes_before = fs::read(&checkpoint_document).unwrap();

    let resumed = run_repair("resume", &config, &checkpoint).unwrap();
    let resumed_report: serde_json::Value = serde_json::from_str(&resumed).unwrap();
    assert_eq!(resumed_report["status"], "complete");
    assert_eq!(resumed_report["model_dispatches"], 0);
    assert_eq!(resumed_report["effect_dispatches"], 0);
    assert_eq!(resumed_report["candidate_digest"], serde_json::Value::Null);
    assert_eq!(resumed_report["generation"], first_report["generation"]);
    assert_eq!(fs::read(&checkpoint_document).unwrap(), bytes_before);
}

/// V1 fixture fields only describe the test transport. Once a terminal source
/// journal exists, recovery must validate that journal before it derives any
/// fixture diagnostic or candidate preview. Otherwise an irrelevant fixture
/// edit could turn a read-only replay into a new pre-replay candidate action.
#[test]
fn repair_v1_terminal_resume_does_not_prederive_fixture_diagnostics() {
    unix_checkpoint_host!();
    let fixture = Fixture::new();
    let (config, checkpoint) = setup(&fixture, "test.repair.fixture-replay.v1");
    run_repair("run", &config, &checkpoint).unwrap();
    let checkpoint_document = checkpoint.join("checkpoint.json");
    let bytes_before = fs::read(&checkpoint_document).unwrap();

    let mut value: serde_json::Value = serde_json::from_slice(&fs::read(&config).unwrap()).unwrap();
    // These fields are consumed by the fixed fixture only. `7, false` is the
    // corrected candidate shape, so the former eager `envelope.preview` would
    // reject before it ever recovered the terminal journal.
    value["malformed_replacement"] = serde_json::json!(7);
    value["malformed_bool_literal"] = serde_json::json!(false);
    fs::write(&config, serde_json::to_vec(&value).unwrap()).unwrap();

    let resumed: serde_json::Value =
        serde_json::from_str(&run_repair("resume", &config, &checkpoint).unwrap()).unwrap();
    assert_eq!(resumed["status"], "complete");
    assert_eq!(resumed["model_dispatches"], 0);
    assert_eq!(resumed["effect_dispatches"], 0);
    assert_eq!(resumed["candidate_digest"], serde_json::Value::Null);
    assert_eq!(fs::read(&checkpoint_document).unwrap(), bytes_before);
}

/// The V2 route receives exactly the same settled OpenCode event/export wire
/// as production, through an injected credential-free runner. A terminal
/// replay does not start that runner, fabricate fresh candidate evidence, or
/// mutate authoritative source or the checkpoint.
#[test]
fn repair_v2_settled_wire_terminal_resume_is_zero_dispatch_and_nonpublishing() {
    unix_checkpoint_host!();
    let fixture = Fixture::new();
    let (config, checkpoint, digest) = setup_v2(&fixture, "test.repair.v2-terminal.v1");
    let scratch = fixture.0.join("scratch");
    let source_path = fixture.0.join("project/src/app.spx");
    let source_before = fs::read(&source_path).unwrap();
    fs::create_dir(&scratch).unwrap();
    let calls = Rc::new(Cell::new(0));
    let prompts = Rc::new(RefCell::new(Vec::new()));
    let first = execute_with_runner(
        v2_command("run", config.clone(), checkpoint.clone(), scratch.clone()),
        RecordedOpenCodeRunner {
            answers: VecDeque::from([proposal(&digest, "0", "0"), proposal(&digest, "7", "1")]),
            last_answer: None,
            prompts: Rc::clone(&prompts),
            calls: Rc::clone(&calls),
        },
    )
    .unwrap();
    let first: serde_json::Value = serde_json::from_str(&first).unwrap();
    assert_eq!(first["schema"], RECEIPT_SCHEMA_V2);
    assert_eq!(first["status"], "complete");
    assert_eq!(first["model_dispatches"], 2);
    assert_eq!(first["effect_dispatches"], 2);
    assert_eq!(first["candidate_test_execution"]["status"], "not_run");
    assert_eq!(first["source_mutation"], false);
    assert_eq!(first["publication_authority"], false);
    assert_eq!(first["selected_profile"]["config_schema"], CONFIG_SCHEMA_V2);
    assert_eq!(first["selected_profile"]["provider_id"], "opencode");
    assert_eq!(
        first["selected_profile"]["model_id"],
        "muse-spark-1.3-contributor-free"
    );
    assert_eq!(first["selected_profile"]["adapter_version"], "1.0.0");
    assert_eq!(
        first["selected_profile"]["provider_profile"],
        "opencode-free"
    );
    assert!(first["selected_profile"]["adapter_identity"]
        .as_str()
        .is_some_and(|identity| identity.starts_with("opencode-repair-adapter:sha256:")));
    assert!(first["checked_prerequisites"]["program_root"]
        .as_str()
        .is_some_and(|digest| digest.starts_with("sha256:")));
    assert!(first["checked_prerequisites"]["source_revision"]
        .as_str()
        .is_some_and(|digest| digest.starts_with("sha256:")));
    assert_eq!(
        first["checked_prerequisites"]["proposal_schema_digest"],
        digest
    );
    assert!(first["checked_prerequisites"]["deployment_binding"]
        .as_str()
        .is_some_and(|binding| binding.starts_with("sha256:")));
    let attempts = first["model_attempts"]
        .as_array()
        .expect("V2 exports bounded per-attempt journal projections");
    assert_eq!(attempts.len(), 2);
    for (turn, attempt) in attempts.iter().enumerate() {
        assert_eq!(attempt["turn"].as_u64(), Some(turn as u64));
        assert_eq!(attempt["attempt"].as_u64(), Some(0));
        assert_eq!(attempt["stage"], "decoded");
        assert!(attempt["request_digest"].is_string());
        assert!(attempt["response_digest"].is_string());
        assert!(attempt["response_bytes"]
            .as_u64()
            .is_some_and(|bytes| bytes > 0));
        assert_eq!(
            attempt["provider_reported"]["total"],
            serde_json::Value::Null
        );
        assert_eq!(
            attempt["provider_reported"]["input"],
            serde_json::Value::Null
        );
        assert_eq!(
            attempt["provider_reported"]["output"],
            serde_json::Value::Null
        );
    }
    assert!(first["journal_binding"]["invocation"].is_string());
    assert_eq!(calls.get(), 2);
    assert_eq!(prompts.borrow().len(), 2);
    assert_eq!(
        fs::read(&source_path).unwrap(),
        source_before,
        "the settled V2 run must not write authoritative source"
    );
    let checkpoint_document = checkpoint.join("checkpoint.json");
    let bytes_before = fs::read(&checkpoint_document).unwrap();

    let resumed = execute_with_runner(
        v2_command("resume", config, checkpoint.clone(), scratch),
        RecordedOpenCodeRunner {
            answers: VecDeque::new(),
            last_answer: None,
            prompts,
            calls: Rc::clone(&calls),
        },
    )
    .unwrap();
    let resumed: serde_json::Value = serde_json::from_str(&resumed).unwrap();
    assert_eq!(resumed["schema"], RECEIPT_SCHEMA_V2);
    assert_eq!(resumed["status"], "complete");
    assert_eq!(resumed["model_dispatches"], 0);
    assert_eq!(resumed["effect_dispatches"], 0);
    assert_eq!(resumed["candidate_digest"], serde_json::Value::Null);
    assert_eq!(resumed["analysis"]["coverage"]["source_review"], false);
    assert_eq!(resumed["source_mutation"], false);
    assert_eq!(resumed["publication_authority"], false);
    assert_eq!(resumed["selected_profile"], first["selected_profile"]);
    assert_eq!(
        resumed["checked_prerequisites"],
        first["checked_prerequisites"]
    );
    assert_eq!(resumed["model_attempts"], first["model_attempts"]);
    assert_eq!(calls.get(), 2, "terminal replay must not start OpenCode");
    assert_eq!(
        fs::read(&source_path).unwrap(),
        source_before,
        "the terminal V2 replay must not write authoritative source"
    );
    assert_eq!(fs::read(&checkpoint_document).unwrap(), bytes_before);
}

/// A settled-provider response is journal-authenticated. A hostile edit to
/// its retained bytes must fail recovery before an OpenCode invocation, effect
/// execution, candidate preview, or checkpoint write can occur.
#[test]
fn repair_v2_tampered_settled_wire_refuses_resume_without_redispatch() {
    unix_checkpoint_host!();
    let fixture = Fixture::new();
    let (config, checkpoint, digest) = setup_v2(&fixture, "test.repair.v2-tamper.v1");
    let scratch = fixture.0.join("scratch");
    let source_path = fixture.0.join("project/src/app.spx");
    let source_before = fs::read(&source_path).unwrap();
    fs::create_dir(&scratch).unwrap();
    let calls = Rc::new(Cell::new(0));
    let prompts = Rc::new(RefCell::new(Vec::new()));
    execute_with_runner(
        v2_command("run", config.clone(), checkpoint.clone(), scratch.clone()),
        RecordedOpenCodeRunner {
            answers: VecDeque::from([proposal(&digest, "0", "0"), proposal(&digest, "7", "1")]),
            last_answer: None,
            prompts: Rc::clone(&prompts),
            calls: Rc::clone(&calls),
        },
    )
    .unwrap();
    assert_eq!(calls.get(), 2);
    assert_eq!(
        fs::read(&source_path).unwrap(),
        source_before,
        "the settled V2 run must not write authoritative source"
    );
    let checkpoint_document = checkpoint.join("checkpoint.json");
    let mut journal: serde_json::Value =
        serde_json::from_slice(&fs::read(&checkpoint_document).unwrap()).unwrap();
    let settled = journal["entries"]
        .as_array_mut()
        .unwrap()
        .iter_mut()
        .find(|entry| entry["kind"] == "attempt_settled")
        .expect("recorded V2 execution has a settled provider response");
    let response = settled["response"]
        .as_str()
        .expect("settled response is hex")
        .to_owned();
    let replacement = if response.starts_with('0') { '1' } else { '0' };
    settled["response"] = serde_json::json!(format!("{replacement}{}", &response[1..]));
    let tampered = serde_json::to_vec(&journal).unwrap();
    fs::write(&checkpoint_document, &tampered).unwrap();

    let error = execute_with_runner(
        v2_command("resume", config, checkpoint.clone(), scratch),
        RecordedOpenCodeRunner {
            answers: VecDeque::new(),
            last_answer: None,
            prompts,
            calls: Rc::clone(&calls),
        },
    )
    .expect_err("tampered settled response must fail closed");
    assert_eq!(error.reason, "repair V2 retained checkpoint is malformed");
    assert_eq!(calls.get(), 2, "hostile recovery must not start OpenCode");
    assert_eq!(
        fs::read(&source_path).unwrap(),
        source_before,
        "hostile recovery must not write authoritative source"
    );
    assert_eq!(fs::read(&checkpoint_document).unwrap(), tampered);
}
#[path = "repair_tests/recovery.rs"]
mod recovery;
#[test]
fn v2_failed_candidate_test_is_bound_feedback_and_terminal_resume_never_redispatches_it() {
    unix_checkpoint_host!();
    let fixture = Fixture::new();
    let (config, checkpoint, digest) = setup_v2(&fixture, "test.repair.v2-test-failure.v1");
    let scratch = fixture.0.join("scratch");
    fs::create_dir(&scratch).unwrap();
    let calls = Rc::new(Cell::new(0));
    let prompts = Rc::new(RefCell::new(Vec::new()));
    let test_calls = Rc::new(Cell::new(0));
    let subjects = Rc::new(RefCell::new(Vec::new()));
    let mut observer = RecordedCandidateTestObserver {
        reply: CandidateTestReply::Canonical {
            status: "failed",
            detail: "the checked candidate test failed",
        },
        calls: Rc::clone(&test_calls),
        subjects: Rc::clone(&subjects),
    };
    let capability = CandidateTestCapability::host_selected("test.candidate.v1").unwrap();
    let mut host = CandidateTestHost::new(capability, &mut observer);
    let source_path = fixture.0.join("project/src/app.spx");
    let source_before = fs::read(&source_path).unwrap();
    let project_before = project_tree(&fixture.0.join("project"));
    let first = execute_with_runner_and_candidate_test(
        v2_command("run", config.clone(), checkpoint.clone(), scratch.clone()),
        RecordedOpenCodeRunner {
            answers: VecDeque::from([proposal(&digest, "0", "0"), proposal(&digest, "7", "1")]),
            last_answer: None,
            prompts: Rc::clone(&prompts),
            calls: Rc::clone(&calls),
        },
        Some(&mut host),
    )
    .unwrap();
    let first: serde_json::Value = serde_json::from_str(&first).unwrap();
    assert_eq!(first["candidate_test_execution"]["status"], "failed");
    assert!(first["selected_profile"]["adapter_identity"]
        .as_str()
        .is_some_and(|identity| identity.contains(":candidate-test:sha256:")));
    assert_eq!(
        first["analysis"]["coverage"]["candidate_test_execution"],
        true
    );
    assert!(first["candidate_test_execution"]["feedback_code"]
        .as_i64()
        .is_some_and(|code| code < 0));
    assert_eq!(test_calls.get(), 1);
    assert_eq!(subjects.borrow().len(), 1);
    let subject = &subjects.borrow()[0];
    assert_eq!(subject.capability, "test.candidate.v1");
    assert_eq!(
        subject.candidate_revision,
        first["candidate_digest"].as_str().unwrap()
    );
    assert!(subject.base_project_revision.starts_with("sha256:"));
    assert!(subject.source_revision.starts_with("sha256:"));
    assert!(subject.candidate_source.contains("\n    7\n"));
    assert_eq!(fs::read(&source_path).unwrap(), source_before);
    assert_eq!(project_tree(&fixture.0.join("project")), project_before);
    let checkpoint_before = fs::read(checkpoint.join("checkpoint.json")).unwrap();

    let resumed = execute_with_runner_and_candidate_test(
        v2_command("resume", config, checkpoint.clone(), scratch),
        RecordedOpenCodeRunner {
            answers: VecDeque::new(),
            last_answer: None,
            prompts,
            calls: Rc::clone(&calls),
        },
        Some(&mut host),
    )
    .unwrap();
    let resumed: serde_json::Value = serde_json::from_str(&resumed).unwrap();
    assert_eq!(resumed["model_dispatches"], 0);
    assert_eq!(resumed["effect_dispatches"], 0);
    assert_eq!(resumed["candidate_test_execution"]["status"], "failed");
    assert_eq!(resumed["candidate_test_execution"]["replayed"], true);
    assert_eq!(
        resumed["candidate_test_execution"]["observation"],
        serde_json::Value::Null
    );
    assert_eq!(
        resumed["analysis"]["coverage"]["candidate_test_execution"],
        true
    );
    assert_eq!(calls.get(), 2);
    assert_eq!(test_calls.get(), 1);
    assert_eq!(
        fs::read(&checkpoint.join("checkpoint.json")).unwrap(),
        checkpoint_before
    );
    assert_eq!(fs::read(&source_path).unwrap(), source_before);
    assert_eq!(project_tree(&fixture.0.join("project")), project_before);
}
#[test]
fn v2_failed_candidate_test_feedback_reaches_the_next_real_provider_prompt() {
    unix_checkpoint_host!();
    let fixture = Fixture::new();
    let (config, checkpoint, digest) =
        setup_v2_with_feedback_turn(&fixture, "test.repair.v2-test-feedback.v1");
    let project_before = project_tree(&fixture.0.join("project"));
    let scratch = fixture.0.join("scratch");
    fs::create_dir(&scratch).unwrap();
    let calls = Rc::new(Cell::new(0));
    let prompts = Rc::new(RefCell::new(Vec::new()));
    let test_calls = Rc::new(Cell::new(0));
    let subjects = Rc::new(RefCell::new(Vec::new()));
    let mut observer = RecordedCandidateTestObserver {
        reply: CandidateTestReply::Canonical {
            status: "failed",
            detail: "the candidate test must be repaired",
        },
        calls: Rc::clone(&test_calls),
        subjects,
    };
    let capability = CandidateTestCapability::host_selected("test.feedback.v1").unwrap();
    let mut host = CandidateTestHost::new(capability, &mut observer);
    execute_with_runner_and_candidate_test(
        v2_command("run", config, checkpoint, scratch),
        RecordedOpenCodeRunner {
            answers: VecDeque::from([
                proposal(&digest, "0", "0"),
                proposal(&digest, "7", "1"),
                proposal(&digest, "0", "0"),
            ]),
            last_answer: None,
            prompts: Rc::clone(&prompts),
            calls: Rc::clone(&calls),
        },
        Some(&mut host),
    )
    .unwrap();
    assert_eq!(test_calls.get(), 1);
    assert_eq!(calls.get(), 3, "the third request must reach the provider");
    let third: serde_json::Value = serde_json::from_str(&prompts.borrow()[2]).unwrap();
    let feedback = third["previous_effect_hex"]
        .as_str()
        .expect("the next provider request carries the settled effect feedback");
    let bytes = decode_hex(feedback);
    let feedback: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    let code = feedback["fields"][0][1]
        .as_str()
        .and_then(|value| value.parse::<i64>().ok())
        .expect("candidate-test feedback remains a canonically encoded typed i64 result");
    assert!(
        code < 0,
        "the actual failed test must feed a negative result"
    );
    assert_eq!(project_tree(&fixture.0.join("project")), project_before);
}
#[test]
fn v2_candidate_test_observation_refuses_malformed_oversized_and_withheld_output() {
    unix_checkpoint_host!();
    for raw in [
        Vec::new(),
        b"{not canonical}".to_vec(),
        vec![b'x'; MAX_CANDIDATE_TEST_OBSERVATION_BYTES + 1],
    ] {
        let fixture = Fixture::new();
        let (config, checkpoint, digest) = setup_v2(&fixture, "test.repair.v2-test-hostile.v1");
        let scratch = fixture.0.join("scratch");
        fs::create_dir(&scratch).unwrap();
        let calls = Rc::new(Cell::new(0));
        let prompts = Rc::new(RefCell::new(Vec::new()));
        let test_calls = Rc::new(Cell::new(0));
        let subjects = Rc::new(RefCell::new(Vec::new()));
        let mut observer = RecordedCandidateTestObserver {
            reply: CandidateTestReply::Raw(raw),
            calls: Rc::clone(&test_calls),
            subjects,
        };
        let capability = CandidateTestCapability::host_selected("test.hostile.v1").unwrap();
        let mut host = CandidateTestHost::new(capability, &mut observer);
        let source_path = fixture.0.join("project/src/app.spx");
        let source_before = fs::read(&source_path).unwrap();
        let project_before = project_tree(&fixture.0.join("project"));
        assert!(execute_with_runner_and_candidate_test(
            v2_command("run", config.clone(), checkpoint.clone(), scratch.clone()),
            RecordedOpenCodeRunner {
                answers: VecDeque::from([proposal(&digest, "0", "0"), proposal(&digest, "7", "1")]),
                last_answer: None,
                prompts,
                calls: Rc::clone(&calls),
            },
            Some(&mut host),
        )
        .is_err());
        assert_eq!(calls.get(), 2);
        assert_eq!(test_calls.get(), 1);
        assert_eq!(fs::read(&source_path).unwrap(), source_before);
        assert_eq!(project_tree(&fixture.0.join("project")), project_before);
        let resumed = execute_with_runner_and_candidate_test(
            v2_command("resume", config, checkpoint, scratch),
            RecordedOpenCodeRunner {
                answers: VecDeque::new(),
                last_answer: None,
                prompts: Rc::new(RefCell::new(Vec::new())),
                calls: Rc::clone(&calls),
            },
            Some(&mut host),
        )
        .unwrap();
        let resumed: serde_json::Value = serde_json::from_str(&resumed).unwrap();
        assert_eq!(resumed["candidate_test_execution"]["status"], "refused");
        assert_eq!(resumed["candidate_test_execution"]["replayed"], true);
        assert_eq!(
            calls.get(),
            2,
            "uncertain observer work must not redispatch"
        );
        assert_eq!(test_calls.get(), 1);
        assert_eq!(fs::read(&source_path).unwrap(), source_before);
        assert_eq!(project_tree(&fixture.0.join("project")), project_before);
    }
}
#[test]
fn v2_candidate_test_observation_refuses_foreign_bindings_and_noncanonical_fields() {
    unix_checkpoint_host!();
    let replies = [
        CandidateTestReply::CanonicalOverride {
            field: "candidate_revision",
            value: "sha256:foreign-candidate",
        },
        CandidateTestReply::CanonicalOverride {
            field: "base_project_revision",
            value: "sha256:foreign-base",
        },
        CandidateTestReply::CanonicalOverride {
            field: "source_revision",
            value: "sha256:foreign-source",
        },
        CandidateTestReply::CanonicalOverride {
            field: "capability",
            value: "foreign-capability",
        },
        CandidateTestReply::CanonicalOverride {
            field: "status",
            value: "unknown",
        },
        CandidateTestReply::CanonicalOverride {
            field: "detail",
            value: "",
        },
        CandidateTestReply::CanonicalOverride {
            field: "detail",
            value: "\u{0001}",
        },
        CandidateTestReply::ExtraField,
        CandidateTestReply::DuplicateStatus,
        CandidateTestReply::ExtraNewline,
    ];
    for reply in replies {
        let fixture = Fixture::new();
        let (config, checkpoint, digest) =
            setup_v2(&fixture, "test.repair.v2-test-binding-fields.v1");
        let project_before = project_tree(&fixture.0.join("project"));
        let scratch = fixture.0.join("scratch");
        fs::create_dir(&scratch).unwrap();
        let calls = Rc::new(Cell::new(0));
        let prompts = Rc::new(RefCell::new(Vec::new()));
        let test_calls = Rc::new(Cell::new(0));
        let subjects = Rc::new(RefCell::new(Vec::new()));
        let mut observer = RecordedCandidateTestObserver {
            reply,
            calls: Rc::clone(&test_calls),
            subjects,
        };
        let mut host = CandidateTestHost::new(
            CandidateTestCapability::host_selected("test.hostile-binding.v1").unwrap(),
            &mut observer,
        );
        assert!(execute_with_runner_and_candidate_test(
            v2_command("run", config, checkpoint, scratch),
            RecordedOpenCodeRunner {
                answers: VecDeque::from([proposal(&digest, "0", "0"), proposal(&digest, "7", "1")]),
                last_answer: None,
                prompts,
                calls,
            },
            Some(&mut host),
        )
        .is_err());
        assert_eq!(test_calls.get(), 1);
        assert_eq!(project_tree(&fixture.0.join("project")), project_before);
    }
}
#[test]
fn v2_candidate_test_capability_drift_refuses_resume_before_provider_or_test_dispatch() {
    unix_checkpoint_host!();
    let fixture = Fixture::new();
    let (config, checkpoint, digest) = setup_v2(&fixture, "test.repair.v2-test-binding.v1");
    let scratch = fixture.0.join("scratch");
    fs::create_dir(&scratch).unwrap();
    let calls = Rc::new(Cell::new(0));
    let prompts = Rc::new(RefCell::new(Vec::new()));
    let test_calls = Rc::new(Cell::new(0));
    let subjects = Rc::new(RefCell::new(Vec::new()));
    let mut first_observer = RecordedCandidateTestObserver {
        reply: CandidateTestReply::Canonical {
            status: "passed",
            detail: "candidate test passed",
        },
        calls: Rc::clone(&test_calls),
        subjects: Rc::clone(&subjects),
    };
    let mut first_host = CandidateTestHost::new(
        CandidateTestCapability::host_selected("test.binding.a.v1").unwrap(),
        &mut first_observer,
    );
    execute_with_runner_and_candidate_test(
        v2_command("run", config.clone(), checkpoint.clone(), scratch.clone()),
        RecordedOpenCodeRunner {
            answers: VecDeque::from([proposal(&digest, "0", "0"), proposal(&digest, "7", "1")]),
            last_answer: None,
            prompts: Rc::clone(&prompts),
            calls: Rc::clone(&calls),
        },
        Some(&mut first_host),
    )
    .unwrap();
    let checkpoint_before = fs::read(checkpoint.join("checkpoint.json")).unwrap();
    let mut second_observer = RecordedCandidateTestObserver {
        reply: CandidateTestReply::Canonical {
            status: "passed",
            detail: "a different host must not be accepted",
        },
        calls: Rc::clone(&test_calls),
        subjects,
    };
    let mut second_host = CandidateTestHost::new(
        CandidateTestCapability::host_selected("test.binding.b.v1").unwrap(),
        &mut second_observer,
    );
    assert!(execute_with_runner_and_candidate_test(
        v2_command("resume", config, checkpoint.clone(), scratch),
        RecordedOpenCodeRunner {
            answers: VecDeque::new(),
            last_answer: None,
            prompts,
            calls: Rc::clone(&calls),
        },
        Some(&mut second_host),
    )
    .is_err());
    assert_eq!(calls.get(), 2);
    assert_eq!(test_calls.get(), 1);
    assert_eq!(
        fs::read(checkpoint.join("checkpoint.json")).unwrap(),
        checkpoint_before
    );
}
/// Ordering property: the ordinary Project lock/authority is acquired before
/// any checkpoint store is touched. An unauthenticatable manifest refuses
/// before a checkpoint directory is ever created, so no replay, staging or
/// candidate creation can be reached without it.
#[test]
fn repair_run_acquires_project_authority_before_creating_the_checkpoint_store() {
    let fixture = Fixture::new();
    let manifest = fixture.0.join("missing-project/semaprax.toml");
    let task_path = fixture.0.join("task.txt");
    fs::write(&task_path, b"objective").unwrap();
    let digest = schema_digest(APP);
    let config_value = repair_config_value(&manifest, &task_path, &digest, "test.repair.auth.v1");
    let config = write_config(&fixture, &config_value);
    let checkpoint = fixture.0.join("checkpoint");

    assert!(run_repair("run", &config, &checkpoint).is_err());
    assert!(!checkpoint.exists());
}

/// This reaches the production entry recheck after `with_authenticated_project`
/// has retained the exact source snapshot but before checkpoint or provider
/// construction. It must reject the externally changed raw path and leave no
/// checkpoint behind.
#[test]
fn repair_run_refuses_source_changed_after_project_authentication() {
    let fixture = Fixture::new();
    let (config, checkpoint) = setup(&fixture, "test.repair.entry-source-drift.v1");
    let source_path = fixture.0.join("project/src/app.spx");
    let changed = format!("{APP}\n").into_bytes();
    set_test_source_snapshot_hook(move || fs::write(source_path, changed).unwrap());

    let error = run_repair("run", &config, &checkpoint)
        .expect_err("source drift after Project authentication must be refused");
    assert_eq!(
        error.reason,
        "repair source cannot be read within checked snapshot bounds"
    );
    assert!(
        !checkpoint.exists(),
        "the entry recheck must refuse before checkpoint creation"
    );
}

/// The existing recorded OpenCode runner is the smallest deterministic live
/// boundary for an external edit during a real durable run. The completed
/// journal remains available for review, but the CLI must refuse its receipt
/// rather than associate it with source bytes it did not authenticate.
#[test]
fn repair_v2_refuses_source_changed_during_provider_run() {
    unix_checkpoint_host!();
    let fixture = Fixture::new();
    let (config, checkpoint, digest) = setup_v2(&fixture, "test.repair.exit-source-drift.v1");
    let scratch = fixture.0.join("scratch");
    fs::create_dir(&scratch).unwrap();
    let source_path = fixture.0.join("project/src/app.spx");
    let changed = opencode_app_source()
        .replacen("\n    0\n", "\n    9\n", 1)
        .into_bytes();
    let calls = Rc::new(Cell::new(0));
    let prompts = Rc::new(RefCell::new(Vec::new()));

    let error = execute_with_runner(
        v2_command("run", config, checkpoint.clone(), scratch),
        SourceMutatingOpenCodeRunner {
            inner: RecordedOpenCodeRunner {
                answers: VecDeque::from([proposal(&digest, "0", "0"), proposal(&digest, "7", "1")]),
                last_answer: None,
                prompts,
                calls: Rc::clone(&calls),
            },
            source_path: source_path.clone(),
            replacement: changed.clone(),
            mutated: false,
        },
    )
    .expect_err("source changed during provider work must be refused");
    assert_eq!(
        error.reason,
        "repair source differs from checked Project snapshot"
    );
    assert_eq!(
        calls.get(),
        2,
        "the completed durable run reached both provider turns"
    );
    assert_eq!(
        fs::read(&source_path).unwrap(),
        changed,
        "the CLI must not overwrite the externally changed authoritative source"
    );
    assert!(
        checkpoint.join("checkpoint.json").is_file(),
        "the acknowledged completed journal remains reviewable after the refusal"
    );
}

/// The hostile, load-bearing case: source drift between the checked preview
/// and a later resume fails closed rather than silently replaying stale
/// evidence against a Project that no longer matches it. The refused resume
/// leaves the checkpoint exactly as it was; it advances nothing.
#[test]
fn repair_resume_refuses_when_source_drifts_between_preview_and_resume() {
    unix_checkpoint_host!();
    let fixture = Fixture::new();
    let (config, checkpoint) = setup(&fixture, "test.repair.drift.v1");

    let first = run_repair("run", &config, &checkpoint).unwrap();
    let first_report: serde_json::Value = serde_json::from_str(&first).unwrap();
    assert_eq!(first_report["status"], "complete");

    let checkpoint_document = checkpoint.join("checkpoint.json");
    let bytes_before = fs::read(&checkpoint_document).unwrap();

    // Mutate the exact checked source that produced the retained preview,
    // after that preview was produced and the terminal checkpoint committed,
    // but before any later session reads it again.
    let manifest = PathBuf::from(config_manifest(&config));
    let source_path = manifest.parent().unwrap().join("src/app.spx");
    let mutated = APP.replacen(
        "@id(\"fixture.repair.value\")\nfn repair_value() -> i64\n{\n    0\n}",
        "@id(\"fixture.repair.value\")\nfn repair_value() -> i64\n{\n    9\n}",
        1,
    );
    assert_ne!(
        mutated, APP,
        "the drift edit must actually change the source"
    );
    fs::write(&source_path, &mutated).unwrap();

    let resumed = run_repair("resume", &config, &checkpoint);
    assert!(
        resumed.is_err(),
        "a resume against drifted source must be refused, not silently replayed"
    );
    assert_eq!(
        fs::read(&checkpoint_document).unwrap(),
        bytes_before,
        "a refused drifted resume must not advance or corrupt the checkpoint"
    );
}
/// Required failure case from issue #116: "The fixture agent repairs using
/// actual feedback and fails when the required observation is withheld."
/// This flips `requires_prior_feedback` onto the *first* scripted turn, which
/// runs before any effect has ever been dispatched, so the real per-request
/// prompt genuinely carries no `previous_effect_hex` yet. The guard must
/// inspect the actual prompt bytes the runtime built (not a hand-fed one) and
/// refuse rather than let the fixture agent proceed on a fabricated
/// observation. `FeedbackGuardedAdapter::start` is the sole production
/// enforcement point (`repair.rs`); this test drives it end to end through
/// the real `run_live_bound_model_durable` path rather than calling it
/// directly.
#[test]
fn repair_run_fails_when_the_required_prior_feedback_observation_is_withheld() {
    unix_checkpoint_host!();
    let fixture = Fixture::new();
    let manifest = write_project(&fixture, APP).canonicalize().unwrap();
    let digest = schema_digest(APP);
    let task_path = fixture.0.join("task.txt");
    fs::write(&task_path, b"repair the checked candidate").unwrap();
    let mut config_value =
        repair_config_value(&manifest, &task_path, &digest, "test.repair.withheld.v1");
    config_value["turns"][0]["requires_prior_feedback"] = serde_json::json!(true);
    let config = write_config(&fixture, &config_value);
    let checkpoint = fixture.0.join("checkpoint");

    let result = run_repair("run", &config, &checkpoint);
    let error = result.expect_err(
        "a turn withholding its required prior-feedback observation must fail, not silently proceed",
    );
    assert!(
        error
            .reason
            .contains("repair checked source execution refused"),
        "unexpected refusal reason: {}",
        error.reason
    );
}

#[test]
fn repair_feedback_guard_rejects_a_forged_nonempty_prior_effect() {
    let expected_feedback = Rc::new(RefCell::new(Some(canonical_effect_hex(&[(
        "value".to_owned(),
        semaprax::interpreter::retained_call::RetainedValue::I64(583),
    )]))));
    let starts = Rc::new(Cell::new(0));
    let mut guarded = FeedbackGuardedAdapter {
        inner: ScriptedStreamingAdapter::new(Vec::new(), Vec::new(), usage(1, 1, 0), true),
        starts: Rc::clone(&starts),
        requires_prior_feedback: true,
        expected_feedback,
        refuse_start: false,
    };
    let request = AdapterRequest {
        request_bytes: br#"{"previous_effect_hex":"00"}"#.to_vec(),
        max_response_bytes: 4096,
    };
    let error = guarded
        .start(
            &AdapterInvocationCapability::grant("repair feedback guard test"),
            &request,
        )
        .expect_err("a nonempty effect observation must still match the actual handler result");
    assert_eq!(
        error.0,
        "repair correction omitted checked diagnostic feedback"
    );
    assert_eq!(
        starts.get(),
        1,
        "the guard records the rejected attempted adapter start"
    );
}

fn config_manifest(config: &Path) -> String {
    let value: serde_json::Value = serde_json::from_slice(&fs::read(config).unwrap()).unwrap();
    value["manifest"].as_str().unwrap().to_owned()
}

#[test]
fn command_grammar_requires_run_or_resume_and_absolute_operands() {
    let args = |values: &[&str]| {
        values
            .iter()
            .map(|value| (*value).into())
            .collect::<Vec<_>>()
    };
    assert!(Command::parse(&args(&[])).is_err());
    assert!(Command::parse(&args(&["run", "relative.json", "/checkpoint"])).is_err());
    assert!(Command::parse(&args(&["migrate", "/config.json", "/checkpoint"])).is_err());
    assert!(Command::parse(&args(&[
        "run",
        "/config.json",
        "/checkpoint",
        "--opencode",
        "/bin/opencode",
        "--scratch",
        "/scratch"
    ]))
    .is_ok());
    assert!(Command::parse(&args(&[
        "run",
        "/config.json",
        "/checkpoint",
        "--scratch",
        "/scratch",
        "--opencode",
        "/bin/opencode"
    ]))
    .is_err());
    assert!(Command::parse(&args(&["run", "/config.json", "/checkpoint"])).is_ok());
    assert!(Command::parse(&args(&["resume", "/config.json", "/checkpoint"])).is_ok());
}

#[test]
fn opencode_repair_configuration_requires_explicit_host_provider_operands() {
    let fixture = Fixture::new();
    let (config, checkpoint) = setup(&fixture, "test.repair.opencode-denial.v1");
    let mut value: serde_json::Value = serde_json::from_slice(&fs::read(&config).unwrap()).unwrap();
    value["schema"] = serde_json::json!("semaprax.source-live-cli.repair-config.v2");
    value.as_object_mut().unwrap().remove("turns");
    fs::write(&config, serde_json::to_vec(&value).unwrap()).unwrap();

    let error = run_repair("run", &config, &checkpoint)
        .expect_err("the production repair configuration must not select an implicit provider");
    assert!(error.reason.contains("requires --opencode"));
    assert!(
        !checkpoint.exists(),
        "provider authority refusal must happen before a checkpoint exists"
    );
}
