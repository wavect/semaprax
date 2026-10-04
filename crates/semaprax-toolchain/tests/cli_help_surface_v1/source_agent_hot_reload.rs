use std::fs;
use std::io::{BufRead, BufReader, Write};
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicU64, Ordering};

use serde_json::{json, Value};

use crate::source_fixture;

static SERIAL: AtomicU64 = AtomicU64::new(0);

struct Fixture(PathBuf);

impl Fixture {
    fn new() -> Self {
        let root = std::env::temp_dir().canonicalize().unwrap().join(format!(
            "semaprax-full-source-agent-hot-reload-{}-{}",
            std::process::id(),
            SERIAL.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir_all(root.join("project/src")).unwrap();
        fs::create_dir(root.join("scratch-a")).unwrap();
        fs::create_dir(root.join("scratch-b")).unwrap();
        fs::create_dir(root.join("xdg-data")).unwrap();
        Self(root.canonicalize().unwrap())
    }

    fn project(&self) -> PathBuf {
        self.0.join("project")
    }

    fn source_live(&self, arguments: &[String]) -> std::process::Output {
        Command::new(env!("CARGO_BIN_EXE_semaprax-full"))
            .arg("source-live")
            .args(arguments)
            .env("XDG_DATA_HOME", self.0.join("xdg-data"))
            .output()
            .unwrap()
    }

    fn child(&self, arguments: &[String]) -> std::process::Child {
        Command::new(env!("CARGO_BIN_EXE_semaprax-full"))
            .arg("dev")
            .arg(self.project().join("semaprax.toml"))
            .arg("--jsonl")
            .arg("--source-agent")
            .args(arguments)
            .env("XDG_DATA_HOME", self.0.join("xdg-data"))
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .unwrap()
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn source_agent_block(state_role: &str) -> String {
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

fn write_project(root: &Path, source: &str, state_role: &str) -> PathBuf {
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
    fs::write(
        root.join("semaprax.toml"),
        "schema = \"semaprax.project.v11\"\nname = \"fixture\"\nversion = \"1.0.0\"\nprofile = \"nested-owned-record-api.v1\"\nentry = \"fixture.agent.lifecycle\"\nsources = [\"src/app.spx\", \"src/tests.spx\"]\nweb_exports = [\"fixture.export.build\"]\ntests = [\"fixture.tests\"]\n",
    )
    .unwrap();
    root.join("semaprax.toml")
}

fn successor_source() -> String {
    let source = source_fixture::SOURCE
        .replace(
            "fn initialize(task: own Task) -> State\n{\n    State",
            "fn initialize(task: own Task) -> StateB\n{\n    StateB",
        )
        .replace("epoch: 1 }", "epoch: 1, marker: 7 }")
        .replace("fn observe(state: borrow State)", "fn observe(state: borrow StateB)")
        .replace(
            "fn authorize(state: borrow State,",
            "fn authorize(state: borrow StateB,",
        )
        .replace("fn reduce(state: own State,", "fn reduce(state: own StateB,")
        .replace(
            "@id(\"fixture.agent.step.continue.epoch\") epoch: i64,",
            "@id(\"fixture.agent.step.continue.epoch\") epoch: i64, @id(\"fixture.agent.step.continue.marker\") marker: i64,",
        )
        .replace(
            "@id(\"fixture.agent.step.suspend.epoch\") epoch: i64,",
            "@id(\"fixture.agent.step.suspend.epoch\") epoch: i64, @id(\"fixture.agent.step.suspend.marker\") marker: i64,",
        );
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

fn config(fixture: &Fixture, name: &str, manifest: &Path) -> PathBuf {
    let task = fixture.0.join("task.txt");
    let read = fixture.0.join("read.txt");
    fs::write(&task, b"alpha").unwrap();
    fs::write(&read, b"observed").unwrap();
    let value = json!({
        "schema": "semaprax.source-live-cli.config.v1",
        "manifest": manifest,
        "source_path": "src/app.spx",
        "agent_id": "fixture.agent",
        "step_id": "fixture.agent.type.step",
        "task_path": task,
        "task_budget": 1,
        "read_path": read,
        "deadline_millis": 2_000_000_000_000i64,
        "ceiling": 3,
        "reservation_units": 1,
        "max_iterations": 2,
        "max_stages": 16,
        "max_steps_per_stage": 100,
        "max_total_steps": 5000,
        "response_limit": 4096
    });
    let path = fixture.0.join(name);
    fs::write(&path, serde_json::to_vec(&value).unwrap()).unwrap();
    path
}

fn stub(fixture: &Fixture) -> PathBuf {
    let executable = fixture.0.join("opencode-stub.rb");
    let log = fixture.0.join("opencode-calls.log");
    let script = format!(
        r##"#!/usr/bin/ruby
require "json"
log = {log:?}
session = "ses_fixture"
message = "msg_fixture"
parts = [
  {{"id" => "prt_start", "messageID" => message, "sessionID" => session, "type" => "step-start"}},
  {{"id" => "prt_text", "messageID" => message, "sessionID" => session, "type" => "text"}},
  {{"id" => "prt_finish", "messageID" => message, "sessionID" => session, "type" => "step-finish", "reason" => "stop"}}
]
case ARGV[0]
when "run"
  abort "bad run arguments" unless ARGV[1, 9] == ["--pure", "--agent", "semaprax-live", "--model", "opencode/muse-spark-1.3-contributor-free", "--format", "json", "--dir", ARGV[9]]
  prompt = ARGV[-1]
  digest = prompt.match(/\nproposal_schema_digest=([^\n]+)/)[1]
  answer = JSON.generate({{"schema" => "semaprax.agent-proposal.v1", "agent_id" => "fixture.agent", "proposal_schema_digest" => digest, "value" => {{"fields" => {{"fixture.agent.type.proposal.budget" => "1", "fixture.agent.type.proposal.urgent" => false, "fixture.agent.type.proposal.sequence" => "1"}}}}}}) + "\n"
  parts[1]["text"] = answer
  File.write(".opencode-host-private/state/prompt", prompt)
  File.open(log, "a") {{ |file| file.puts("run") }}
  ["step_start", "text", "step_finish"].zip(parts).each {{ |kind, part| puts(JSON.generate({{"type" => kind, "sessionID" => session, "part" => part}})) }}
when "export"
  abort "bad export arguments" unless ARGV == ["export", session, "--pure"]
  prompt = File.read(".opencode-host-private/state/prompt")
  presented = prompt.include?(" ") ? '"' + prompt.gsub('"', '\\"') + '"' : prompt
  answer = parts[1]["text"] = JSON.generate({{"schema" => "semaprax.agent-proposal.v1", "agent_id" => "fixture.agent", "proposal_schema_digest" => prompt.match(/\nproposal_schema_digest=([^\n]+)/)[1], "value" => {{"fields" => {{"fixture.agent.type.proposal.budget" => "1", "fixture.agent.type.proposal.urgent" => false, "fixture.agent.type.proposal.sequence" => "1"}}}}}}) + "\n"
  export = {{
    "info" => {{"id" => session, "model" => {{"id" => "muse-spark-1.3-contributor-free", "providerID" => "opencode"}}}},
    "messages" => [
      {{"info" => {{"role" => "user", "id" => "msg_user", "sessionID" => session}}, "parts" => [{{"id" => "prt_user", "messageID" => "msg_user", "sessionID" => session, "type" => "text", "text" => presented}}]}},
      {{"info" => {{"role" => "assistant", "id" => message, "sessionID" => session, "parentID" => "msg_user", "modelID" => "muse-spark-1.3-contributor-free", "providerID" => "opencode", "finish" => "stop", "cost" => 0}}, "parts" => parts}}
    ]
  }}
  File.open(log, "a") {{ |file| file.puts("export") }}
  puts(JSON.generate(export))
else
  abort "unexpected opencode command"
end
"##
    );
    fs::write(&executable, script).unwrap();
    fs::set_permissions(&executable, fs::Permissions::from_mode(0o700)).unwrap();
    executable
}

fn request(id: u64, op: &str) -> String {
    format!("{{\"schema\":\"semaprax.hot-reload-control.v1\",\"id\":{id},\"op\":\"{op}\"}}\n")
}

fn send(input: &mut impl Write, id: u64, op: &str) {
    input.write_all(request(id, op).as_bytes()).unwrap();
    input.flush().unwrap();
}

fn receive(output: &mut impl BufRead) -> Value {
    let mut line = String::new();
    output.read_line(&mut line).unwrap();
    assert!(line.len() <= 8192, "response exceeds the control bound");
    serde_json::from_str(&line).unwrap()
}

#[test]
fn full_dev_source_agent_migrates_real_journal_a_to_b_with_local_opencode_stub() {
    let fixture = Fixture::new();
    let suspended = source_fixture::SOURCE.replace(
        "Step::Complete { summary: state.objective, budget: state.budget, status: state.epoch }",
        "Step::Suspend { objective: state.objective, budget: state.budget, epoch: state.epoch }",
    );
    let manifest = write_project(&fixture.project(), &suspended, "fixture.agent.type.state");
    let config_a = config(&fixture, "config-a.json", &manifest);
    let checkpoint_a = fixture.0.join("checkpoint-a");
    let executable = stub(&fixture);
    let initial = fixture.source_live(&[
        "run".into(),
        config_a.display().to_string(),
        checkpoint_a.display().to_string(),
        "--opencode".into(),
        executable.display().to_string(),
        "--scratch".into(),
        fixture.0.join("scratch-a").display().to_string(),
    ]);
    assert!(
        initial.status.success(),
        "{}",
        String::from_utf8_lossy(&initial.stderr)
    );
    assert_eq!(
        serde_json::from_slice::<Value>(&initial.stdout).unwrap()["status"],
        "suspend"
    );
    assert!(checkpoint_a.join("checkpoint.json").is_file());

    let config_b = config(&fixture, "config-b.json", &manifest);
    let checkpoint_b = fixture.0.join("checkpoint-b");
    let operands = vec![
        "migrate".into(),
        config_a.display().to_string(),
        checkpoint_a.display().to_string(),
        config_b.display().to_string(),
        checkpoint_b.display().to_string(),
        "fixture.agent.fn.migrate_b".into(),
        "1000".into(),
        "--opencode".into(),
        executable.display().to_string(),
        "--scratch".into(),
        fixture.0.join("scratch-b").display().to_string(),
    ];
    let mut child = fixture.child(&operands);
    let mut input = child.stdin.take().unwrap();
    let mut output = BufReader::new(child.stdout.take().unwrap());

    send(&mut input, 1, "start");
    assert_eq!(receive(&mut output)["event"], "started");
    write_project(
        &fixture.project(),
        &successor_source(),
        "fixture.agent.type.state_b",
    );
    send(&mut input, 2, "plan");
    let plan = receive(&mut output);
    assert_eq!(plan["event"], "candidate_admitted");
    assert_eq!(
        plan["plan"]["decision"],
        "eligible_source_agent_checkpoint_handoff"
    );
    send(&mut input, 3, "activate");
    let activated = receive(&mut output);
    assert_eq!(activated["event"], "activated");
    assert_eq!(activated["generation"], 1);
    assert_eq!(activated["terminal_uncertainty"], false);
    assert!(checkpoint_b.join("checkpoint.json").is_file());

    send(&mut input, 4, "invoke");
    let invoke = receive(&mut output);
    assert_eq!(invoke["event"], "rejected");
    assert_eq!(
        invoke["message"],
        "source-Agent execution remains owned by the authenticated source-live session"
    );
    send(&mut input, 5, "stop");
    assert_eq!(receive(&mut output)["event"], "stopped");
    drop(input);
    let status = child.wait().unwrap();
    assert!(status.success());
    assert_eq!(
        fs::read_to_string(fixture.0.join("opencode-calls.log")).unwrap(),
        "run\nexport\nrun\nexport\n"
    );
}
