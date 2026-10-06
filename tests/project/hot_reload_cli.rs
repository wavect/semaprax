use std::fs;
use std::io::{BufRead, BufReader, Write};
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicU64, Ordering};

use serde_json::Value;

static SERIAL: AtomicU64 = AtomicU64::new(0);

struct Fixture(PathBuf);

impl Fixture {
    fn new() -> Self {
        let root = std::env::temp_dir().canonicalize().unwrap().join(format!(
            "semaprax-hot-reload-cli-{}-{}",
            std::process::id(),
            SERIAL.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir_all(root.join("src")).unwrap();
        let source = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("examples/calculator-project");
        for relative in [
            "semaprax.toml",
            "src/app.spx",
            "src/core.spx",
            "src/tests.spx",
        ] {
            fs::copy(source.join(relative), root.join(relative)).unwrap();
        }
        Self(root)
    }
    fn child(&self) -> std::process::Child {
        Command::new(env!("CARGO_BIN_EXE_semaprax"))
            .arg("dev")
            .arg(self.0.join("semaprax.toml"))
            .arg("--jsonl")
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

fn rewrite(path: &std::path::Path, old: &str, new: &str) {
    let source = fs::read_to_string(path).unwrap();
    assert!(source.contains(old), "fixture source is unexpectedly stale");
    fs::write(path, source.replacen(old, new, 1)).unwrap();
}

#[test]
fn dev_jsonl_child_handles_split_coalesced_eof_stop_and_bounded_output() {
    let fixture = Fixture::new();
    let mut child = fixture.child();
    let mut stdin = child.stdin.take().unwrap();
    let start = request(1, "start");
    stdin.write_all(&start.as_bytes()[..11]).unwrap();
    stdin.flush().unwrap();
    stdin.write_all(&start.as_bytes()[11..]).unwrap();
    let mut input = request(2, "status");
    input.push_str(&request(2, "status"));
    input.push_str(&request(3, "invoke"));
    input.push_str(&request(4, "stop"));
    stdin.write_all(input.as_bytes()).unwrap();
    drop(stdin);
    let output = child.wait_with_output().unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(output.stderr.is_empty());
    let rows = BufReader::new(output.stdout.as_slice())
        .lines()
        .map(|line| {
            let line = line.unwrap();
            assert!(line.len() <= 8192);
            serde_json::from_str::<Value>(&line).unwrap()
        })
        .collect::<Vec<_>>();
    assert_eq!(
        rows.iter()
            .map(|row| row["event"].as_str())
            .collect::<Vec<_>>(),
        vec![
            Some("started"),
            Some("status"),
            Some("rejected"),
            Some("invoked"),
            Some("stopped")
        ]
    );
    assert_eq!(rows[2]["message"], "request id is stale");
    assert_eq!(rows[3]["invocation"]["outcome"]["kind"], "returned");

    let mut eof = fixture.child();
    eof.stdin
        .take()
        .unwrap()
        .write_all(request(1, "start").as_bytes())
        .unwrap();
    let eof_output = eof.wait_with_output().unwrap();
    assert!(eof_output.status.success());
    assert_eq!(
        String::from_utf8(eof_output.stdout)
            .unwrap()
            .lines()
            .count(),
        1
    );
}

#[test]
fn dev_jsonl_child_rejects_malformed_and_oversized_frames() {
    let fixture = Fixture::new();
    let mut child = fixture.child();
    let mut stdin = child.stdin.take().unwrap();
    stdin.write_all(b"{bad}\n").unwrap();
    stdin.write_all(&vec![b'x'; 4097]).unwrap();
    stdin.write_all(b"\n").unwrap();
    drop(stdin);
    let output = child.wait_with_output().unwrap();
    assert_eq!(output.status.code(), Some(2));
    assert!(!output.stderr.is_empty());
    let rows = String::from_utf8(output.stdout)
        .unwrap()
        .lines()
        .map(|line| serde_json::from_str::<Value>(line).unwrap())
        .collect::<Vec<_>>();
    assert_eq!(rows[0]["event"], "rejected");
    assert_eq!(rows[1]["message"], "control frame exceeds its bound");
}

#[test]
fn dev_jsonl_child_keeps_b_active_when_invalid_c_is_rejected() {
    let fixture = Fixture::new();
    let mut child = fixture.child();
    let mut input = child.stdin.take().unwrap();
    let mut output = BufReader::new(child.stdout.take().unwrap());

    send(&mut input, 1, "start");
    let a = receive(&mut output);
    assert_eq!(a["event"], "started");
    assert_eq!(a["generation"], 0);

    rewrite(
        &fixture.0.join("src/app.spx"),
        "multiply(6, 7)",
        "multiply(6, 8)",
    );
    send(&mut input, 2, "plan");
    let b_plan = receive(&mut output);
    assert_eq!(b_plan["event"], "candidate_admitted");
    assert_eq!(b_plan["plan"]["decision"], "eligible_code_replacement");

    send(&mut input, 3, "activate");
    let b = receive(&mut output);
    assert_eq!(b["event"], "activated");
    assert_eq!(b["generation"], 1);

    rewrite(
        &fixture.0.join("src/app.spx"),
        "multiply(6, 8)",
        "multiply(",
    );
    send(&mut input, 4, "plan");
    let invalid_c = receive(&mut output);
    assert_eq!(invalid_c["event"], "candidate_rejected");
    assert_eq!(invalid_c["generation"], 1);

    send(&mut input, 5, "invoke");
    let b_after_c = receive(&mut output);
    assert_eq!(b_after_c["event"], "invoked");
    assert_eq!(b_after_c["generation"], 1);
    assert_eq!(
        b_after_c["invocation"]["outcome"],
        serde_json::json!({"kind":"returned","value":48})
    );

    send(&mut input, 6, "stop");
    assert_eq!(receive(&mut output)["event"], "stopped");
    drop(input);
    let status = child.wait().unwrap();
    assert!(status.success());
}

#[test]
fn dev_jsonl_child_discards_a_pending_candidate_when_source_returns_to_the_active_revision() {
    let fixture = Fixture::new();
    let app = fixture.0.join("src/app.spx");
    let original = fs::read(&app).unwrap();
    let mut child = fixture.child();
    let mut input = child.stdin.take().unwrap();
    let mut output = BufReader::new(child.stdout.take().unwrap());

    send(&mut input, 1, "start");
    let a = receive(&mut output);
    assert_eq!(a["event"], "started");

    rewrite(&app, "multiply(6, 7)", "multiply(6, 8)");
    send(&mut input, 2, "plan");
    let b = receive(&mut output);
    assert_eq!(b["event"], "candidate_admitted");
    assert_ne!(
        b["plan"]["candidate_project_revision"],
        a["active_project_revision"]
    );

    // Exact A bytes again: unchanged, and no stale plan for B is offered.
    fs::write(&app, &original).unwrap();
    send(&mut input, 3, "plan");
    let reverted = receive(&mut output);
    assert_eq!(reverted["event"], "unchanged");
    assert!(reverted.get("plan").is_none(), "{reverted}");
    assert_eq!(
        reverted["active_project_revision"],
        a["active_project_revision"]
    );

    send(&mut input, 4, "activate");
    let refused = receive(&mut output);
    assert_eq!(refused["event"], "rejected");
    assert_eq!(refused["message"], "no retained activation plan");

    // A subsequent valid change still plans and activates normally.
    rewrite(&app, "multiply(6, 7)", "multiply(6, 9)");
    send(&mut input, 5, "plan");
    assert_eq!(receive(&mut output)["event"], "candidate_admitted");
    send(&mut input, 6, "activate");
    let activated = receive(&mut output);
    assert_eq!(activated["event"], "activated");
    assert_eq!(activated["generation"], 1);

    send(&mut input, 7, "stop");
    assert_eq!(receive(&mut output)["event"], "stopped");
    drop(input);
    assert!(child.wait().unwrap().success());
}
