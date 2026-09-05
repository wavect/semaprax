//! MCP CLI policy and source-authority evidence.
use semaprax::image_transport::{VNextPolicy, VNextSession};
use semaprax::project::{with_authenticated_project, ProjectCandidate, SemanticChange};
use serde_json::{json, Value};
use std::collections::BTreeSet;
use std::io::{BufRead, BufReader, Read, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Output, Stdio};
use std::sync::{
    atomic::{AtomicU64, Ordering},
    mpsc::{self, Receiver},
};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};
const MAX_STDIO_LINE_BYTES: usize = 8 * 1024 * 1024 + 1;
const MAX_STDERR_BYTES: usize = 64 * 1024;
const MAX_CATALOGUE_PAGES: usize = 64;
const MAX_CATALOGUE_TOOLS: usize = 512;
static SERIAL: AtomicU64 = AtomicU64::new(0);
struct Fixture(PathBuf);
impl Fixture {
    fn new() -> Self {
        let root = std::env::temp_dir().join(format!(
            "spx-mcp-cli-{}-{}",
            std::process::id(),
            SERIAL.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir_all(root.join("src")).unwrap();
        let sample = Path::new(env!("CARGO_MANIFEST_DIR")).join("examples/calculator-project");
        for file in [
            "semaprax.toml",
            "src/app.spx",
            "src/core.spx",
            "src/tests.spx",
        ] {
            std::fs::copy(sample.join(file), root.join(file)).unwrap();
        }
        Self(root.canonicalize().unwrap())
    }
    fn run(&self, command: &str, policy: &Value, input: &str) -> Output {
        let policy_path = self.0.join("host.json");
        std::fs::write(&policy_path, policy.to_string()).unwrap();
        let mut child = Command::new(env!("CARGO_BIN_EXE_semaprax"))
            .arg(command)
            .arg(self.0.join("semaprax.toml"))
            .arg(policy_path)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .unwrap();
        let mut stdin = child.stdin.take().unwrap();
        let _ = stdin.write_all(input.as_bytes());
        drop(stdin);
        child.wait_with_output().unwrap()
    }
    fn sources(&self) -> Vec<Vec<u8>> {
        [
            "semaprax.toml",
            "src/app.spx",
            "src/core.spx",
            "src/tests.spx",
        ]
        .iter()
        .map(|file| std::fs::read(self.0.join(file)).unwrap())
        .collect()
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}
fn policy(version: usize, prepare: bool) -> Value {
    let mut policy = json!({"schema":format!("semaprax.workspace-host-policy.v{version}"),"candidate_prepare":prepare,"diagnostics":false,"build_enabled":false,"test_policy":null,"git_commit":null});
    if version >= 2 {
        policy["frontend_cache"] = json!(false);
    }
    if version >= 3 {
        policy["candidate_archives"] = json!([]);
    }
    if version >= 4 {
        policy["semantic_cache"] = json!(false);
    }
    if version >= 5 {
        policy["semantic_cache_entry"] = Value::Null;
    }
    if version >= 6 {
        policy["draft_archives"] = json!([]);
    }
    policy
}
fn request(id: Value, method: &str, params: Value) -> Value {
    json!({"jsonrpc":"2.0","id":id,"method":method,"params":params})
}
fn tool(id: Value, name: &str, args: Value) -> Value {
    request(id, "tools/call", json!({"name":name,"arguments":args}))
}
fn input(calls: Vec<Value>) -> String {
    let mut frames = vec![
        request(
            json!("initialize"),
            "initialize",
            json!({"protocolVersion":"2025-11-25","capabilities":{},"clientInfo":{"name":"cli-evidence","version":"1"}}),
        ),
        json!({"jsonrpc":"2.0","method":"notifications/initialized"}),
    ];
    frames.extend(calls);
    frames
        .into_iter()
        .map(|frame| format!("{frame}\n"))
        .collect()
}
fn rows(output: Output) -> Vec<Value> {
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(output.stderr.is_empty());
    String::from_utf8(output.stdout)
        .unwrap()
        .lines()
        .map(|line| serde_json::from_str(line).unwrap())
        .collect()
}
fn inner(row: &Value) -> Value {
    serde_json::from_str(row["result"]["content"][0]["text"].as_str().unwrap()).unwrap()
}
fn failed(row: &Value) -> bool {
    row.get("error").is_some() || row["result"]["isError"] == true
}

fn bounded_line(reader: &mut impl BufRead) -> Result<Option<String>, String> {
    let mut line = Vec::new();
    loop {
        let available = reader.fill_buf().map_err(|error| error.to_string())?;
        if available.is_empty() {
            return if line.is_empty() {
                Ok(None)
            } else {
                Err("MCP stdout ended inside a frame".into())
            };
        }
        let end = available
            .iter()
            .position(|byte| *byte == b'\n')
            .map_or(available.len(), |index| index + 1);
        if line.len().saturating_add(end) > MAX_STDIO_LINE_BYTES {
            return Err("MCP stdout frame exceeds its retained byte bound".into());
        }
        let terminated = available[end - 1] == b'\n';
        line.extend_from_slice(&available[..end]);
        reader.consume(end);
        if terminated {
            return String::from_utf8(line)
                .map(Some)
                .map_err(|_| "MCP stdout frame is not UTF-8".into());
        }
    }
}

struct LiveMcp {
    child: Child,
    input: Option<std::process::ChildStdin>,
    output: Receiver<Result<String, String>>,
    output_done: Receiver<()>,
    error_result: Receiver<Result<(Vec<u8>, bool), String>>,
    output_thread: Option<JoinHandle<()>>,
    error_thread: Option<JoinHandle<()>>,
    reaped: bool,
}
impl LiveMcp {
    fn start(fixture: &Fixture, policy: &Value) -> Self {
        let policy_path = fixture.0.join("host.json");
        std::fs::write(&policy_path, policy.to_string()).unwrap();
        let mut child = Command::new(env!("CARGO_BIN_EXE_semaprax"))
            .arg("serve-workspace-mcp")
            .arg(fixture.0.join("semaprax.toml"))
            .arg(policy_path)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .unwrap();
        let input = child.stdin.take().unwrap();
        let output = child.stdout.take().unwrap();
        let error = child.stderr.take().unwrap();
        let (sender, receiver) = mpsc::channel();
        let (output_done_sender, output_done) = mpsc::channel();
        let output_thread = thread::spawn(move || {
            let mut reader = BufReader::new(output);
            loop {
                match bounded_line(&mut reader) {
                    Ok(None) => break,
                    Ok(Some(line)) => {
                        if sender.send(Ok(line)).is_err() {
                            break;
                        }
                    }
                    Err(error) => {
                        let _ = sender.send(Err(error.to_string()));
                        break;
                    }
                }
            }
            let _ = output_done_sender.send(());
        });
        let (error_result_sender, error_result) = mpsc::channel();
        let error_thread = thread::spawn(move || {
            let mut reader = BufReader::new(error);
            let mut retained = Vec::new();
            let mut overflow = false;
            let result = loop {
                let mut chunk = [0_u8; 8192];
                match reader.read(&mut chunk) {
                    Ok(0) => break Ok((retained, overflow)),
                    Ok(count) => {
                        let remaining = MAX_STDERR_BYTES.saturating_sub(retained.len());
                        retained.extend_from_slice(&chunk[..count.min(remaining)]);
                        overflow |= count > remaining;
                    }
                    Err(error) => break Err(error.to_string()),
                }
            };
            let _ = error_result_sender.send(result);
        });
        Self {
            child,
            input: Some(input),
            output: receiver,
            output_done,
            error_result,
            output_thread: Some(output_thread),
            error_thread: Some(error_thread),
            reaped: false,
        }
    }
    fn send(&mut self, frame: Value) {
        let input = self.input.as_mut().unwrap();
        writeln!(input, "{frame}").unwrap();
        input.flush().unwrap();
    }
    fn receive(&self) -> Value {
        let line = self
            .output
            .recv_timeout(Duration::from_secs(10))
            .unwrap_or_else(|error| panic!("timed out waiting for MCP response: {error}"))
            .unwrap();
        assert!(line.ends_with('\n'));
        assert!(!line.contains('\r'));
        serde_json::from_str(line.strip_suffix('\n').unwrap()).unwrap()
    }
    fn finish(mut self) {
        self.input.take();
        let deadline = Instant::now() + Duration::from_secs(10);
        let status = loop {
            if let Some(status) = self.child.try_wait().unwrap() {
                break status;
            }
            if Instant::now() >= deadline {
                self.child.kill().unwrap();
                let _ = self.child.wait();
                self.reaped = true;
                panic!("MCP process did not exit after stdin EOF");
            }
            thread::sleep(Duration::from_millis(10));
        };
        self.reaped = true;
        self.output_done
            .recv_timeout(Duration::from_secs(2))
            .expect("MCP stdout reader did not finish after process exit");
        let (stderr, stderr_overflow) = self
            .error_result
            .recv_timeout(Duration::from_secs(2))
            .expect("MCP stderr reader did not finish after process exit")
            .expect("cannot read MCP stderr");
        self.output_thread.take();
        self.error_thread.take();
        assert!(status.success(), "{}", String::from_utf8_lossy(&stderr));
        assert!(
            !stderr_overflow,
            "MCP stderr exceeds its retained byte bound"
        );
        assert!(stderr.is_empty());
        assert!(self.output.try_recv().is_err(), "unexpected MCP response");
    }
}
impl Drop for LiveMcp {
    fn drop(&mut self) {
        if !self.reaped {
            let _ = self.child.kill();
            let _ = self.child.wait();
        }
        let _ = self.output_done.recv_timeout(Duration::from_secs(2));
        let _ = self.error_result.recv_timeout(Duration::from_secs(2));
        self.output_thread.take();
        self.error_thread.take();
    }
}

#[test]
fn help_pins_the_optional_mcp_command_without_replacing_v5() {
    // The bare invocation prints the guided page; the catalog is `help all`.
    let output = Command::new(env!("CARGO_BIN_EXE_semaprax"))
        .args(["help", "all"])
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(0));
    let help = String::from_utf8(output.stdout).unwrap();
    let help_lines = help.lines().collect::<Vec<_>>();
    for line in [
        "semaprax serve-workspace-mcp <manifest> <host-policy.json>",
        "semaprax serve-workspace <manifest> <host-policy.json>",
    ] {
        assert_eq!(
            help_lines
                .iter()
                .filter(|candidate| **candidate == line)
                .count(),
            1
        );
    }
}

#[test]
fn real_stdio_catalogue_paging_and_notification_nonexecution_are_explicit() {
    let fixture = Fixture::new();
    let disk = fixture.sources();
    let base = with_authenticated_project(&fixture.0.join("semaprax.toml"), |snapshot| {
        ProjectCandidate::open(snapshot.retain_revision(), snapshot.project_revision())
    })
    .unwrap();
    let mut host = VNextSession::open(
        &fixture.0.join("semaprax.toml"),
        VNextPolicy {
            candidate_prepare: true,
            ..Default::default()
        },
    )
    .unwrap();
    let image = host.image_revision().to_owned();
    host.finish().unwrap();

    let mut process = LiveMcp::start(&fixture, &policy(6, true));
    process.send(request(
        json!("initialize"),
        "initialize",
        json!({"protocolVersion":"2025-11-25","capabilities":{},"clientInfo":{"name":"real-stdio-evidence","version":"1"}}),
    ));
    let initialized = process.receive();
    assert_eq!(initialized["id"], "initialize");
    assert_eq!(initialized["result"]["protocolVersion"], "2025-11-25");
    process.send(json!({"jsonrpc":"2.0","method":"notifications/initialized"}));

    let mut names = Vec::new();
    let mut cursors = BTreeSet::new();
    let mut cursor = None;
    let mut terminal_cursor = false;
    for _ in 0..MAX_CATALOGUE_PAGES {
        let params = cursor
            .as_ref()
            .map_or_else(|| json!({}), |cursor| json!({"cursor":cursor}));
        let id = format!("catalogue-{}", names.len());
        process.send(request(json!(id), "tools/list", params));
        let page = process.receive();
        assert_eq!(page["id"], id);
        assert!(page.get("error").is_none(), "{page}");
        let tools = page["result"]["tools"].as_array().unwrap();
        assert!(!tools.is_empty());
        assert!(tools.len() <= 8);
        assert!(
            names.len().saturating_add(tools.len()) <= MAX_CATALOGUE_TOOLS,
            "MCP catalogue exceeds its test inventory bound"
        );
        names.extend(
            tools
                .iter()
                .map(|tool| tool["name"].as_str().unwrap().to_owned()),
        );
        cursor = page["result"]["nextCursor"].as_str().map(str::to_owned);
        match cursor.as_ref() {
            Some(cursor) => assert!(cursors.insert(cursor.clone())),
            None => {
                terminal_cursor = true;
                break;
            }
        }
    }
    assert!(
        terminal_cursor,
        "MCP catalogue has no bounded terminal cursor"
    );
    assert!(names.windows(2).all(|pair| pair[0] < pair[1]));
    for available in ["workspace__open", "candidate__open", "candidate__query"] {
        assert!(names.iter().any(|name| name == available), "{available}");
    }
    for unavailable in ["candidate__build", "candidate__test", "candidate__commit"] {
        assert!(
            !names.iter().any(|name| name == unavailable),
            "{unavailable}"
        );
    }

    process.send(tool(json!("open"), "workspace__open", json!({})));
    let opened = process.receive();
    assert_eq!(opened["id"], "open");
    assert!(!failed(&opened), "{opened}");
    assert_eq!(inner(&opened)["result"]["payload"]["image_revision"], image);

    process.send(tool(json!("unknown"), "not_a_tool", json!({})));
    let unknown = process.receive();
    assert_eq!(unknown["id"], "unknown");
    assert_eq!(unknown["error"]["code"], -32602);

    process.send(json!({"jsonrpc":"2.0","method":"tools/call","params":{"name":"candidate__open","arguments":{"image_revision":image}}}));
    process.send(tool(
        json!("notification-probe"),
        "candidate__query",
        json!({"image_revision":image,"candidate_revision":base.candidate_digest()}),
    ));
    let probe = process.receive();
    assert_eq!(probe["id"], "notification-probe");
    assert!(failed(&probe), "{probe}");
    let probe = inner(&probe);
    assert_eq!(probe["error"]["code"], -32000);
    assert_eq!(
        probe["error"]["message"],
        "SPX-G224: candidate handle is stale, discarded, or unknown"
    );

    process.finish();
    assert_eq!(fixture.sources(), disk);
}

#[test]
fn all_six_fixed_host_policies_preserve_readonly_grants_and_exact_v5_read_bytes() {
    let fixture = Fixture::new();
    let disk = fixture.sources();
    for version in 1..=6 {
        let policy = policy(version, false);
        let direct = fixture.run(
            "serve-workspace",
            &policy,
            &format!("{}\n", request(json!(0), "workspace/open", json!({}))),
        );
        assert!(
            direct.status.success(),
            "{}",
            String::from_utf8_lossy(&direct.stderr)
        );
        let direct_text = String::from_utf8(direct.stdout).unwrap();
        let direct_value: Value = serde_json::from_str(&direct_text).unwrap();
        let calls = vec![
            tool(json!(-2), "workspace__open", json!({})),
            tool(
                json!(3),
                "candidate__open",
                json!({"image_revision":"untrusted"}),
            ),
            tool(json!(4), "candidate__build", json!({})),
            tool(json!(5), "candidate__test", json!({})),
            tool(json!(6), "candidate__commit", json!({"approved":true})),
        ];
        let result = rows(fixture.run("serve-workspace-mcp", &policy, &input(calls)));
        assert_eq!(result.len(), 6);
        assert_eq!(result[0]["result"]["protocolVersion"], "2025-11-25");
        assert_eq!(result[1]["id"], -2);
        assert_eq!(inner(&result[1]), direct_value);
        assert_eq!(
            result[1]["result"]["content"][0]["text"].as_str().unwrap(),
            direct_text.strip_suffix('\n').unwrap()
        );
        assert_eq!(result[1]["result"]["isError"], false);
        for denied in &result[2..] {
            assert!(failed(denied), "{denied}");
        }
        assert_eq!(fixture.sources(), disk);
    }
}

#[test]
fn candidate_enabled_cli_replays_semantic_edits_without_source_or_execution_authority() {
    let fixture = Fixture::new();
    let disk = fixture.sources();
    let base = with_authenticated_project(&fixture.0.join("semaprax.toml"), |snapshot| {
        ProjectCandidate::open(snapshot.retain_revision(), snapshot.project_revision())
    })
    .unwrap();
    let change = SemanticChange::new(
        base.revision().project_revision(),
        &json!({"kind":"rename_declaration","target":"calculator.add","name":"addition"}),
    )
    .unwrap();
    let changed = base.apply(base.candidate_digest(), &change).unwrap();
    let mut host = VNextSession::open(
        &fixture.0.join("semaprax.toml"),
        VNextPolicy {
            candidate_prepare: true,
            ..Default::default()
        },
    )
    .unwrap();
    let image = host.image_revision().to_owned();
    host.finish().unwrap();
    let calls = vec![
        tool(json!(1), "candidate__open", json!({"image_revision":image})),
        tool(
            json!(2),
            "candidate__apply-intent",
            json!({"image_revision":image,"candidate_revision":base.candidate_digest(),"intent":{"kind":"rename_declaration","target":"calculator.add","name":"addition"}}),
        ),
        tool(
            json!(3),
            "candidate__query",
            json!({"image_revision":image,"candidate_revision":base.candidate_digest(),"chunk_bytes":1024}),
        ),
        tool(
            json!(4),
            "candidate__build",
            json!({"image_revision":image,"candidate_revision":changed.candidate_digest()}),
        ),
        tool(
            json!(5),
            "candidate__test",
            json!({"image_revision":image,"candidate_revision":changed.candidate_digest()}),
        ),
        tool(
            json!(6),
            "candidate__commit",
            json!({"image_revision":image,"candidate_revision":changed.candidate_digest(),"approved":true}),
        ),
    ];
    let result = rows(fixture.run("serve-workspace-mcp", &policy(6, true), &input(calls)));
    assert_eq!(result.len(), 7);
    assert_eq!(
        inner(&result[1])["result"]["payload"]["candidate_revision"],
        base.candidate_digest()
    );
    assert_eq!(
        inner(&result[2])["result"]["payload"]["candidate_revision"],
        changed.candidate_digest()
    );
    assert_eq!(
        inner(&result[2])["result"]["payload"]["source_authority"],
        false
    );
    for row in &result[1..4] {
        assert!(!failed(row), "{row}");
        assert_eq!(inner(row)["id"], 0);
    }
    for row in &result[4..] {
        assert!(failed(row), "{row}");
    }
    assert_eq!(fixture.sources(), disk);
}

#[test]
fn malformed_host_policy_fails_before_handshake_and_v5_frames_are_not_mcp_calls() {
    let fixture = Fixture::new();
    for mut policy in [policy(1, false), policy(6, true)] {
        policy["approval_via_request"] = json!(true);
        let result = fixture.run("serve-workspace-mcp", &policy, &input(vec![]));
        assert!(!result.status.success());
        assert!(result.stdout.is_empty());
        assert!(String::from_utf8_lossy(&result.stderr).contains("SPX-G280"));
    }
    let result = rows(fixture.run(
        "serve-workspace-mcp",
        &policy(1, false),
        &format!("{}\n", request(json!(0), "workspace/open", json!({}))),
    ));
    assert_eq!(result.len(), 1);
    assert_eq!(result[0]["error"]["code"], -32601);
}
