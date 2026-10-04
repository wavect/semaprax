use std::fs;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

use super::checkpoint::bounded_read;
use super::options::{Command, SessionConfig};

static NEXT: AtomicU64 = AtomicU64::new(0);

struct Fixture(PathBuf);
impl Fixture {
    fn new() -> Self {
        let parent = std::env::temp_dir().canonicalize().unwrap();
        let path = parent.join(format!(
            "spx-source-live-cli-test-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&path).unwrap();
        Self(path)
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

#[test]
fn command_grammar_requires_explicit_absolute_operands() {
    let args = |values: &[&str]| {
        values
            .iter()
            .map(|value| (*value).into())
            .collect::<Vec<_>>()
    };
    assert!(Command::parse(&args(&[])).is_err());
    assert!(Command::parse(&args(&[
        "resume",
        "relative.json",
        "/checkpoint",
        "--opencode",
        "/opencode",
        "--scratch",
        "/scratch"
    ]))
    .is_err());
    assert!(Command::parse(&args(&[
        "run",
        "/config.json",
        "/checkpoint",
        "--opencode",
        "/opencode",
        "--scratch",
        "/scratch"
    ]))
    .is_ok());
    assert!(Command::parse(&args(&[
        "migrate",
        "/old.json",
        "/old",
        "/new.json",
        "/new",
        "migration.id",
        "3",
        "--opencode",
        "/opencode",
        "--scratch",
        "/scratch"
    ]))
    .is_ok());
    assert!(Command::parse(&args(&[
        "migrate",
        "/old.json",
        "/old",
        "/new.json",
        "/new",
        "migration.id",
        "0",
        "--opencode",
        "/opencode",
        "--scratch",
        "/scratch"
    ]))
    .is_err());
}

fn valid_config() -> serde_json::Value {
    serde_json::json!({
        "schema": "semaprax.source-live-cli.config.v1",
        "manifest": "/physical/project/semaprax.toml",
        "source_path": "src/app.spx",
        "agent_id": "fixture.agent",
        "step_id": "fixture.agent.type.step",
        "task_path": "/physical/task.txt",
        "task_budget": 1,
        "read_path": "/physical/read.txt",
        "deadline_millis": 2000000000000i64,
        "ceiling": 3,
        "reservation_units": 1,
        "max_iterations": 2,
        "max_stages": 16,
        "max_steps_per_stage": 100,
        "max_total_steps": 5000,
        "response_limit": 4096
    })
}

#[test]
fn exact_config_refuses_duplicate_unknown_negative_and_oversized_capacity() {
    let fixture = Fixture::new();
    let path = fixture.0.join("config.json");
    let value = valid_config();
    fs::write(&path, serde_json::to_vec(&value).unwrap()).unwrap();
    assert!(SessionConfig::load(&path).is_ok());

    let duplicate = serde_json::to_string(&value).unwrap().replacen(
        "\"ceiling\":3",
        "\"ceiling\":3,\"ceiling\":9",
        1,
    );
    assert_ne!(duplicate, serde_json::to_string(&value).unwrap());
    fs::write(&path, duplicate).unwrap();
    assert!(SessionConfig::load(&path).is_err());

    for (key, replacement) in [
        ("reservation_units", serde_json::json!(-1)),
        ("deadline_millis", serde_json::json!(0)),
        ("response_limit", serde_json::json!(65_537)),
        ("source_path", serde_json::json!("../outside.spx")),
    ] {
        let mut wrong = value.clone();
        wrong[key] = replacement;
        fs::write(&path, serde_json::to_vec(&wrong).unwrap()).unwrap();
        assert!(SessionConfig::load(&path).is_err(), "{key}");
    }
    let mut unknown = value;
    unknown["provider_override"] = serde_json::json!("paid");
    fs::write(&path, serde_json::to_vec(&unknown).unwrap()).unwrap();
    assert!(SessionConfig::load(&path).is_err());
}

#[cfg(unix)]
#[test]
fn held_checkpoint_fd_survives_rename_and_second_writer_is_refused() {
    use semaprax::agent_lifecycle::CheckpointStore;

    use super::checkpoint::CheckpointDir;

    let fixture = Fixture::new();
    let project = fixture.0.join("project");
    fs::create_dir(&project).unwrap();
    let original = fixture.0.join("checkpoint");
    let moved = fixture.0.join("checkpoint-moved");
    let mut first = CheckpointDir::fresh(&original, &project).unwrap();
    assert!(CheckpointDir::existing(&original, &project).is_err());
    fs::rename(&original, &moved).unwrap();
    first.commit(1, "held-journal\n").unwrap();
    assert_eq!(
        fs::read_to_string(moved.join("checkpoint.json")).unwrap(),
        "held-journal\n"
    );
    assert!(!original.exists());
    drop(first);
    assert!(CheckpointDir::existing(&moved, &project).is_ok());
}

#[cfg(unix)]
#[test]
fn failed_checkpoint_rename_poisoned_writer_requires_fresh_latest_recovery() {
    use semaprax::agent_lifecycle::CheckpointStore;

    use super::checkpoint::CheckpointDir;

    let fixture = Fixture::new();
    let project = fixture.0.join("project");
    fs::create_dir(&project).unwrap();
    let checkpoint = fixture.0.join("checkpoint");
    let mut writer = CheckpointDir::fresh(&checkpoint, &project).unwrap();
    fs::create_dir(checkpoint.join("checkpoint.json")).unwrap();
    assert!(writer.commit(1, "candidate\n").is_err());
    fs::remove_dir(checkpoint.join("checkpoint.json")).unwrap();
    assert!(writer.commit(1, "retry-without-reload\n").is_err());
    drop(writer);
    let recovered = CheckpointDir::existing(&checkpoint, &project).unwrap();
    assert!(recovered.latest().unwrap().is_none());
}

#[cfg(unix)]
#[test]
fn symlinked_latest_checkpoint_is_refused_not_treated_as_absent() {
    use std::os::unix::fs::symlink;

    use super::checkpoint::CheckpointDir;

    let fixture = Fixture::new();
    let project = fixture.0.join("project");
    fs::create_dir(&project).unwrap();
    let checkpoint = fixture.0.join("checkpoint");
    let writer = CheckpointDir::fresh(&checkpoint, &project).unwrap();
    let outside = fixture.0.join("outside");
    fs::write(&outside, b"forged").unwrap();
    symlink(&outside, checkpoint.join("checkpoint.json")).unwrap();
    assert!(writer.latest().is_err());
}

#[cfg(unix)]
#[test]
fn bounded_input_rejects_symlink_and_fifo_without_blocking() {
    use std::os::unix::fs::symlink;

    let fixture = Fixture::new();
    let ordinary = fixture.0.join("ordinary");
    fs::write(&ordinary, b"ok").unwrap();
    assert_eq!(bounded_read(&ordinary, 2).unwrap(), b"ok");
    let link = fixture.0.join("link");
    symlink(&ordinary, &link).unwrap();
    assert!(bounded_read(&link, 2).is_err());
    let fifo = fixture.0.join("fifo");
    assert!(std::process::Command::new("mkfifo")
        .arg(&fifo)
        .status()
        .unwrap()
        .success());
    assert!(bounded_read(&fifo, 2).is_err());
}

#[path = "../../examples/fixtures/opencode_source_fixture.rs"]
mod source_fixture;

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use crate::opencode_host::{OpenCodeHostConfig, OpenCodeRunner, OpenCodeRunnerFailure};

struct RecordedRunner {
    answer: String,
    prompts: Rc<RefCell<Vec<String>>>,
    calls: Rc<Cell<usize>>,
}

impl OpenCodeRunner for RecordedRunner {
    fn run(
        &mut self,
        _: &OpenCodeHostConfig,
        prompt: &str,
    ) -> Result<Vec<u8>, OpenCodeRunnerFailure> {
        self.calls.set(self.calls.get() + 1);
        self.prompts.borrow_mut().push(prompt.to_owned());
        Ok(recorded_transport(prompt, &self.answer).0)
    }

    fn export(
        &mut self,
        _: &OpenCodeHostConfig,
        _: &str,
    ) -> Result<Vec<u8>, OpenCodeRunnerFailure> {
        let prompts = self.prompts.borrow();
        Ok(recorded_transport(prompts.last().expect("run before export"), &self.answer).1)
    }
}

fn recorded_transport(prompt: &str, answer: &str) -> (Vec<u8>, Vec<u8>) {
    let mut export: serde_json::Value = serde_json::from_str(include_str!(
        "../../../../scripts/fixtures/opencode-provider-smoke-v1/session.json"
    ))
    .unwrap();
    let presented = if prompt.contains(' ') {
        format!("\"{}\"", prompt.replace('"', "\\\""))
    } else {
        prompt.to_owned()
    };
    export["messages"][0]["parts"][0]["text"] = serde_json::json!(presented);
    export["messages"][1]["parts"][2]["text"] = serde_json::json!(answer);
    let parts = export["messages"][1]["parts"].as_array().unwrap();
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
    (events, serde_json::to_vec(&export).unwrap())
}

fn source_agent_block(state_role: &str) -> String {
    // AgentDefinition v1 preserves this member's canonical field order.
    // Re-encoding through serde_json::Value would sort keys and alter the
    // frozen definition bytes even though the parsed object has equal values.
    let runtime_json = source_fixture::DEFINITION
        .trim_end()
        .split_once("\"runtime_v1\":")
        .unwrap()
        .1
        .strip_suffix('}')
        .unwrap();
    let runtime = serde_json::to_string(&runtime_json).unwrap();
    format!(
        r#"
@id("fixture.agent")
agent FixtureAgent {{
    types {{
        @id("fixture.agent.type.task") type task;
        @id("{state_role}") type state;
        @id("fixture.agent.type.observation") type observation;
        @id("fixture.agent.type.proposal") type proposal;
        @id("fixture.agent.type.outcome") type outcome;
        @id("fixture.agent.type.result") type result;
    }}
    operations {{
        @id("fixture.agent.fn.initialize") fn initialize;
        @id("fixture.agent.fn.observe") fn observe;
        @id("fixture.agent.fn.propose") model fn propose;
        @id("fixture.agent.fn.authorize") fn authorize;
        @id("fixture.agent.fn.execute") effect fn execute;
        @id("fixture.agent.fn.reduce") fn reduce;
    }}
    runtime_v1 {{ canonical_json {runtime}; }}
}}
"#
    )
}

fn source_project(root: &std::path::Path, source: &str, state_role: &str) -> PathBuf {
    fs::create_dir_all(root.join("src")).unwrap();
    // The Project-linked observation schema admits scalar fields. The frozen
    // standalone smoke source has a Bytes tag; remove only that unused tag.
    let source = source
        .replace(
            "    @id(\"fixture.agent.type.observation.tag\") tag: Bytes,\n",
            "",
        )
        .replace("    let tag = [79u8, 66u8];\n", "")
        .replace("tag: bytes_copy(array_as_slice(tag)), ", "");
    let source = format!(
        "{source}\n{}\n@id(\"fixture.export.payload\")\nrecord ExportPayload {{ @id(\"fixture.export.payload.bytes\") bytes: Bytes, }}\n@id(\"fixture.export.build\")\nfn build(input: borrow Slice<u8>) -> ExportPayload {{ ExportPayload {{ bytes: bytes_copy(input) }} }}\n",
        source_agent_block(state_role)
    );
    let canonical = semaprax::format::canonical(&semaprax::parse(&source, "src/app.spx").unwrap());
    fs::write(root.join("src/app.spx"), canonical).unwrap();
    let tests = "module fixture.tests;\n@id(\"fixture.tests.main\")\nfn main() -> i64 { 0 }\n";
    fs::write(
        root.join("src/tests.spx"),
        semaprax::format::canonical(&semaprax::parse(tests, "src/tests.spx").unwrap()),
    )
    .unwrap();
    fs::write(root.join("semaprax.toml"), "schema = \"semaprax.project.v11\"\nname = \"fixture\"\nversion = \"1.0.0\"\nprofile = \"nested-owned-record-api.v1\"\nentry = \"fixture.agent.lifecycle\"\nsources = [\"src/app.spx\", \"src/tests.spx\"]\nweb_exports = [\"fixture.export.build\"]\ntests = [\"fixture.tests\"]\n").unwrap();
    root.join("semaprax.toml")
}

fn source_config(fixture: &Fixture, manifest: &std::path::Path) -> PathBuf {
    let task = fixture.0.join("task.txt");
    let read = fixture.0.join("read.txt");
    fs::write(&task, b"alpha").unwrap();
    fs::write(&read, b"observed").unwrap();
    let mut config = valid_config();
    config["manifest"] = serde_json::json!(manifest);
    config["task_path"] = serde_json::json!(task);
    config["read_path"] = serde_json::json!(read);
    let path = fixture.0.join("config.json");
    fs::write(&path, serde_json::to_vec(&config).unwrap()).unwrap();
    path
}

fn recorded_answer(manifest: &std::path::Path) -> String {
    use semaprax::agent_lifecycle::iterative::compile_project_agent_lifecycle_v2;
    use semaprax::project::with_authenticated_project;

    let project =
        with_authenticated_project(manifest, |snapshot| Ok(snapshot.retain_revision())).unwrap();
    let compiled = compile_project_agent_lifecycle_v2(
        &project,
        "src/app.spx",
        "fixture.agent",
        "fixture.agent.type.step",
    )
    .unwrap();
    format!(
        "{{\"schema\":\"semaprax.agent-proposal.v1\",\"agent_id\":\"fixture.agent\",\"proposal_schema_digest\":{:?},\"value\":{{\"fields\":{{\"fixture.agent.type.proposal.budget\":\"1\",\"fixture.agent.type.proposal.urgent\":false,\"fixture.agent.type.proposal.sequence\":\"1\"}}}}}}\n",
        compiled.proposal_schema().schema().digest(),
    )
}

fn run_command(
    verb: &str,
    config: &std::path::Path,
    checkpoint: &std::path::Path,
    scratch: &std::path::Path,
) -> Command {
    fs::create_dir(scratch).unwrap();
    Command::parse(&[
        verb.into(),
        config.display().to_string(),
        checkpoint.display().to_string(),
        "--opencode".into(),
        "/usr/bin/true".into(),
        "--scratch".into(),
        scratch.display().to_string(),
    ])
    .unwrap()
}

fn runner(answer: String, calls: &Rc<Cell<usize>>) -> RecordedRunner {
    RecordedRunner {
        answer,
        prompts: Rc::new(RefCell::new(Vec::new())),
        calls: calls.clone(),
    }
}

#[cfg(unix)]
#[test]
fn retained_project_run_and_terminal_resume_reject_changed_inputs_without_dispatch() {
    let fixture = Fixture::new();
    let manifest = source_project(
        &fixture.0.join("project"),
        source_fixture::SOURCE,
        "fixture.agent.type.state",
    );
    let config = source_config(&fixture, &manifest);
    let answer = recorded_answer(&manifest);
    let checkpoint = fixture.0.join("checkpoint");
    let source_before = fs::read(manifest.parent().unwrap().join("src/app.spx")).unwrap();
    let calls = Rc::new(Cell::new(0));
    let first = super::run::execute_with_runner(
        run_command("run", &config, &checkpoint, &fixture.0.join("scratch-1")),
        runner(answer.clone(), &calls),
    )
    .unwrap();
    let receipt: serde_json::Value = serde_json::from_str(&first).unwrap();
    assert_eq!(receipt["status"], "complete");
    assert_eq!(receipt["model_dispatches"], 1);
    assert_eq!(receipt["effect_dispatches"], 1);
    assert_eq!(calls.get(), 1);
    // A dispatching attempt carries the compiled reducer's own revision-bound
    // evidence document (stage rows, authorization bindings, terminal value
    // digest), not just the checkpoint's bookkeeping counters.
    assert_eq!(
        receipt["iterative_evidence"]["schema"],
        "semaprax.agent-iterative-evidence.v2"
    );
    assert_eq!(receipt["iterative_evidence"]["status"], "Complete");
    assert_eq!(receipt["iterative_evidence"]["effects"], 1);
    assert!(receipt["iterative_evidence"]["stages"]
        .as_array()
        .is_some_and(|stages| !stages.is_empty()));
    assert_eq!(
        fs::read(manifest.parent().unwrap().join("src/app.spx")).unwrap(),
        source_before
    );
    let journal_before = fs::read(checkpoint.join("checkpoint.json")).unwrap();

    let resumed = super::run::execute_with_runner(
        run_command("resume", &config, &checkpoint, &fixture.0.join("scratch-2")),
        runner(answer.clone(), &calls),
    )
    .unwrap();
    let replay: serde_json::Value = serde_json::from_str(&resumed).unwrap();
    assert_eq!(replay["status"], "complete");
    assert_eq!(replay["model_dispatches"], 0);
    assert_eq!(replay["effect_dispatches"], 0);
    assert_eq!(replay["committed_model_units"], 1);
    assert_eq!(calls.get(), 1);
    // A pure terminal-checkpoint replay redispatches nothing, so it has no
    // fresh reducer evidence to carry; it must not fabricate one.
    assert_eq!(replay["iterative_evidence"], serde_json::Value::Null);
    assert_eq!(
        fs::read(checkpoint.join("checkpoint.json")).unwrap(),
        journal_before
    );

    let read_path = fixture.0.join("read.txt");
    fs::write(&read_path, b"changed observation").unwrap();
    assert!(super::run::execute_with_runner(
        run_command("resume", &config, &checkpoint, &fixture.0.join("scratch-3")),
        runner(answer.clone(), &calls),
    )
    .is_err());
    fs::write(&read_path, b"observed").unwrap();
    let task_path = fixture.0.join("task.txt");
    fs::write(&task_path, b"changed task").unwrap();
    assert!(super::run::execute_with_runner(
        run_command("resume", &config, &checkpoint, &fixture.0.join("scratch-4")),
        runner(answer.clone(), &calls),
    )
    .is_err());
    fs::write(&task_path, b"alpha").unwrap();
    let mut changed = valid_config();
    changed["manifest"] = serde_json::json!(manifest);
    changed["task_path"] = serde_json::json!(task_path);
    changed["read_path"] = serde_json::json!(read_path);
    changed["max_total_steps"] = serde_json::json!(4999);
    fs::write(&config, serde_json::to_vec(&changed).unwrap()).unwrap();
    assert!(super::run::execute_with_runner(
        run_command("resume", &config, &checkpoint, &fixture.0.join("scratch-5")),
        runner(answer, &calls),
    )
    .is_err());
    assert_eq!(calls.get(), 1);
}

#[cfg(unix)]
#[test]
fn empty_fresh_store_refuses_run_and_resume_with_zero_dispatch() {
    let fixture = Fixture::new();
    let manifest = source_project(
        &fixture.0.join("project"),
        source_fixture::SOURCE,
        "fixture.agent.type.state",
    );
    let config = source_config(&fixture, &manifest);
    let answer = recorded_answer(&manifest);
    let checkpoint = fixture.0.join("checkpoint");
    let store =
        super::checkpoint::CheckpointDir::fresh(&checkpoint, manifest.parent().unwrap()).unwrap();
    drop(store);
    let calls = Rc::new(Cell::new(0));
    for verb in ["run", "resume"] {
        assert!(super::run::execute_with_runner(
            run_command(
                verb,
                &config,
                &checkpoint,
                &fixture.0.join(format!("scratch-{verb}"))
            ),
            runner(answer.clone(), &calls),
        )
        .is_err());
    }
    assert_eq!(calls.get(), 0);
}

fn successor_source() -> String {
    let source = source_fixture::SOURCE
        .replace(
            "fn initialize(task: own Task) -> State\n{\n    State",
            "fn initialize(task: own Task) -> StateB\n{\n    StateB",
        )
        .replace("epoch: 1 }", "epoch: 1, marker: 7 }")
        .replace(
            "fn observe(state: borrow State)",
            "fn observe(state: borrow StateB)",
        )
        .replace(
            "fn authorize(state: borrow State,",
            "fn authorize(state: borrow StateB,",
        )
        .replace(
            "fn reduce(state: own State,",
            "fn reduce(state: own StateB,",
        )
        .replace(
            "@id(\"fixture.agent.step.continue.epoch\") epoch: i64,",
            "@id(\"fixture.agent.step.continue.epoch\") epoch: i64, @id(\"fixture.agent.step.continue.marker\") marker: i64,",
        )
        .replace(
            "@id(\"fixture.agent.step.suspend.epoch\") epoch: i64,",
            "@id(\"fixture.agent.step.suspend.epoch\") epoch: i64, @id(\"fixture.agent.step.suspend.marker\") marker: i64,",
        );
    assert!(source.contains("fn initialize(task: own Task) -> StateB"));
    format!(
        r#"{source}
@id("fixture.agent.type.state_b")
record StateB {{
    @id("fixture.agent.type.state_b.objective") objective: Bytes,
    @id("fixture.agent.type.state_b.budget") budget: i64,
    @id("fixture.agent.type.state_b.epoch") epoch: i64,
    @id("fixture.agent.type.state_b.marker") marker: i64,
}}
@id("fixture.agent.fn.migrate_b")
fn migrate_b(old: own State) -> StateB {{
    StateB {{ objective: old.objective, budget: old.budget, epoch: old.epoch, marker: 7 }}
}}
"#
    )
}

fn migrate_command(
    previous_config: &std::path::Path,
    previous_checkpoint: &std::path::Path,
    destination_config: &std::path::Path,
    destination_checkpoint: &std::path::Path,
    scratch: &std::path::Path,
) -> Command {
    fs::create_dir(scratch).unwrap();
    Command::parse(&[
        "migrate".into(),
        previous_config.display().to_string(),
        previous_checkpoint.display().to_string(),
        destination_config.display().to_string(),
        destination_checkpoint.display().to_string(),
        "fixture.agent.fn.migrate_b".into(),
        "1000".into(),
        "--opencode".into(),
        "/usr/bin/true".into(),
        "--scratch".into(),
        scratch.display().to_string(),
    ])
    .unwrap()
}

#[cfg(unix)]
#[test]
fn suspended_retained_project_migrates_once_and_recovers_destination_without_dispatch() {
    let fixture = Fixture::new();
    let suspend_source = source_fixture::SOURCE.replace(
        "Step::Complete { summary: state.objective, budget: state.budget, status: state.epoch }",
        "Step::Suspend { objective: state.objective, budget: state.budget, epoch: state.epoch }",
    );
    assert_ne!(suspend_source, source_fixture::SOURCE);
    let old_manifest = source_project(
        &fixture.0.join("project-a"),
        &suspend_source,
        "fixture.agent.type.state",
    );
    let old_config = source_config(&fixture, &old_manifest);
    let renamed_old_config = fixture.0.join("config-a.json");
    fs::rename(&old_config, &renamed_old_config).unwrap();
    let old_checkpoint = fixture.0.join("checkpoint-a");
    let old_answer = recorded_answer(&old_manifest);
    let old_calls = Rc::new(Cell::new(0));
    let suspended = super::run::execute_with_runner(
        run_command(
            "run",
            &renamed_old_config,
            &old_checkpoint,
            &fixture.0.join("scratch-a"),
        ),
        runner(old_answer, &old_calls),
    )
    .unwrap();
    let first: serde_json::Value = serde_json::from_str(&suspended).unwrap();
    assert_eq!(first["status"], "suspend");
    assert_eq!(old_calls.get(), 1);

    let new_manifest = source_project(
        &fixture.0.join("project-b"),
        &successor_source(),
        "fixture.agent.type.state_b",
    );
    let new_config = source_config(&fixture, &new_manifest);
    let new_answer = recorded_answer(&new_manifest);
    let new_checkpoint = fixture.0.join("checkpoint-b");
    let new_calls = Rc::new(Cell::new(0));
    let migrated = super::run::execute_with_runner(
        migrate_command(
            &renamed_old_config,
            &old_checkpoint,
            &new_config,
            &new_checkpoint,
            &fixture.0.join("scratch-b"),
        ),
        runner(new_answer.clone(), &new_calls),
    )
    .unwrap();
    let second: serde_json::Value = serde_json::from_str(&migrated).unwrap();
    assert_eq!(second["status"], "complete");
    assert_eq!(second["model_dispatches"], 1);
    assert_eq!(new_calls.get(), 1);
    assert_eq!(second["committed_model_units"], 2);

    let journal = fs::read(new_checkpoint.join("checkpoint.json")).unwrap();
    let recovered = super::run::execute_with_runner(
        migrate_command(
            &renamed_old_config,
            &old_checkpoint,
            &new_config,
            &new_checkpoint,
            &fixture.0.join("scratch-c"),
        ),
        runner(new_answer.clone(), &new_calls),
    )
    .unwrap();
    let replay: serde_json::Value = serde_json::from_str(&recovered).unwrap();
    assert_eq!(replay["status"], "complete");
    assert_eq!(replay["model_dispatches"], 0);
    assert_eq!(replay["effect_dispatches"], 0);
    assert_eq!(new_calls.get(), 1);
    assert_eq!(
        fs::read(new_checkpoint.join("checkpoint.json")).unwrap(),
        journal
    );

    let other_destination = fixture.0.join("checkpoint-c");
    assert!(super::run::execute_with_runner(
        migrate_command(
            &renamed_old_config,
            &old_checkpoint,
            &new_config,
            &other_destination,
            &fixture.0.join("scratch-d")
        ),
        runner(new_answer, &new_calls),
    )
    .is_err());
    assert_eq!(new_calls.get(), 1);
}

#[cfg(unix)]
#[test]
fn headless_hot_reload_agent_bridge_migrates_retained_a_to_b_with_recorded_provider() {
    use semaprax::project::{
        with_authenticated_project, HotReloadDecision, HotReloadSession,
        PreparedProjectInterpreterOptions,
    };

    let fixture = Fixture::new();
    let suspended_source = source_fixture::SOURCE.replace(
        "Step::Complete { summary: state.objective, budget: state.budget, status: state.epoch }",
        "Step::Suspend { objective: state.objective, budget: state.budget, epoch: state.epoch }",
    );
    let old_manifest = source_project(
        &fixture.0.join("project-a"),
        &suspended_source,
        "fixture.agent.type.state",
    );
    let old_config = source_config(&fixture, &old_manifest);
    let saved_old_config = fixture.0.join("config-a.json");
    fs::rename(&old_config, &saved_old_config).unwrap();
    let old_checkpoint = fixture.0.join("checkpoint-a");
    let old_calls = Rc::new(Cell::new(0));
    let old_answer = recorded_answer(&old_manifest);
    let first = super::run::execute_with_runner(
        run_command(
            "run",
            &saved_old_config,
            &old_checkpoint,
            &fixture.0.join("scratch-a"),
        ),
        runner(old_answer, &old_calls),
    )
    .unwrap();
    assert_eq!(
        serde_json::from_str::<serde_json::Value>(&first).unwrap()["status"],
        "suspend"
    );
    assert_eq!(old_calls.get(), 1);
    let old_project =
        with_authenticated_project(&old_manifest, |snapshot| Ok(snapshot.retain_revision()))
            .unwrap();

    // Rewrite the same admitted Project after A has checkpointed. The
    // supervisor retains A while destination admission reads saved B.
    let new_manifest = source_project(
        old_manifest.parent().unwrap(),
        &successor_source(),
        "fixture.agent.type.state_b",
    );
    let new_config = source_config(&fixture, &new_manifest);
    let new_project =
        with_authenticated_project(&new_manifest, |snapshot| Ok(snapshot.retain_revision()))
            .unwrap();
    let mut supervisor =
        HotReloadSession::new(old_project, PreparedProjectInterpreterOptions::default()).unwrap();
    supervisor.admit_candidate(new_project.clone()).unwrap();
    let plan = supervisor.plan().unwrap();
    assert_eq!(
        plan.decision(),
        HotReloadDecision::EligibleSourceAgentCheckpointHandoff
    );

    let new_checkpoint = fixture.0.join("checkpoint-b");
    let scratch = fixture.0.join("scratch-b");
    fs::create_dir(&scratch).unwrap();
    let arguments = vec![
        "migrate".into(),
        saved_old_config.display().to_string(),
        old_checkpoint.display().to_string(),
        new_config.display().to_string(),
        new_checkpoint.display().to_string(),
        "fixture.agent.fn.migrate_b".into(),
        "1000".into(),
        "--opencode".into(),
        "/usr/bin/true".into(),
        "--scratch".into(),
        scratch.display().to_string(),
    ];
    let new_calls = Rc::new(Cell::new(0));
    super::run::execute_hot_reload_migration_with_runner(
        &mut supervisor,
        plan,
        &arguments,
        runner(recorded_answer(&new_manifest), &new_calls),
    )
    .unwrap();
    assert_eq!(new_calls.get(), 1);
    assert_eq!(supervisor.generation(), 1);
    assert_eq!(
        supervisor.active_project_revision(),
        new_project.project_revision()
    );
    assert!(new_checkpoint.join("checkpoint.json").is_file());
}

#[cfg(unix)]
#[path = "priced_tests.rs"]
mod priced_tests;

#[cfg(unix)]
#[path = "priced_migration_tests.rs"]
mod priced_migration_tests;

#[cfg(unix)]
#[path = "priced_adapter_boundaries.rs"]
mod priced_adapter_boundaries;

#[cfg(unix)]
#[path = "io_tests.rs"]
mod io_tests;
