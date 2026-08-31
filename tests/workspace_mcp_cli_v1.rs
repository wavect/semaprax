//! MCP CLI policy and source-authority evidence, authored and intentionally unrun.
use semaprax::image_transport::{VNextPolicy, VNextSession};
use semaprax::project::{with_authenticated_project, ProjectCandidate, SemanticChange};
use serde_json::{json, Value};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};
use std::sync::atomic::{AtomicU64, Ordering};
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
        child
            .stdin
            .take()
            .unwrap()
            .write_all(input.as_bytes())
            .unwrap();
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

#[test]
fn help_pins_the_optional_mcp_command_without_replacing_v5() {
    let output = Command::new(env!("CARGO_BIN_EXE_semaprax"))
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(2));
    let help = String::from_utf8(output.stdout).unwrap();
    for line in [
        "semaprax serve-workspace-mcp <manifest> <host-policy.json>\n",
        "semaprax serve-workspace <manifest> <host-policy.json>\n",
    ] {
        assert_eq!(help.matches(line).count(), 1);
    }
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
