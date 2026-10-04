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
            Some("invoked"),
            Some("stopped")
        ]
    );
    assert_eq!(rows[2]["invocation"]["outcome"]["kind"], "returned");

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
