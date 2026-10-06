//! Terminal-path regressions for every consumer of the shared `FrameReader`:
//! an oversized request is rejected with one bounded error and the session
//! ends without another read, so no LF or EOF from the client is needed.

use std::collections::BTreeMap;
use std::ffi::OsString;
use std::io::{self, BufRead, Read};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use serde_json::Value;

use super::config::ServerConfig;
use super::{selected_law, selected_law_mcp, session};

const CAP: usize = 64;
static SERIAL: AtomicU64 = AtomicU64::new(0);

/// One cap-plus-one chunk with a trailing executable request, then a
/// sentinel error on every further read, standing in for an input pipe the
/// malformed client keeps open.
struct OpenPipe {
    chunk: Vec<u8>,
    offset: usize,
    reads: usize,
}

impl OpenPipe {
    fn new() -> Self {
        let mut chunk = vec![b'x'; CAP + 1];
        chunk.extend_from_slice(b"\n{\"jsonrpc\":\"2.0\",\"id\":7,\"method\":\"shutdown\"}\n");
        Self {
            chunk,
            offset: 0,
            reads: 0,
        }
    }
}

impl Read for OpenPipe {
    fn read(&mut self, buffer: &mut [u8]) -> io::Result<usize> {
        let available = self.fill_buf()?;
        let count = available.len().min(buffer.len());
        buffer[..count].copy_from_slice(&available[..count]);
        self.consume(count);
        Ok(count)
    }
}

impl BufRead for OpenPipe {
    fn fill_buf(&mut self) -> io::Result<&[u8]> {
        self.reads += 1;
        if self.reads > 1 {
            return Err(io::Error::other("sentinel: blocked on rejected input"));
        }
        Ok(&self.chunk[self.offset..])
    }

    fn consume(&mut self, amount: usize) {
        self.offset += amount;
    }
}

fn parse(arguments: &[&str]) -> ServerConfig {
    ServerConfig::parse(arguments.iter().map(OsString::from)).unwrap()
}

/// Exactly one bounded parse error; the trailing request never executed.
fn assert_single_rejection(output: &[u8], message: &str, pipe: &OpenPipe) {
    assert_eq!(pipe.reads, 1, "rejection waited for LF or EOF");
    assert!(pipe.offset <= CAP + 1, "consumed past the delivered frame");
    let text = std::str::from_utf8(output).unwrap();
    let lines = text.split_terminator('\n').collect::<Vec<_>>();
    assert_eq!(lines.len(), 1, "{text}");
    let response: Value = serde_json::from_str(lines[0]).unwrap();
    assert_eq!(response["id"], Value::Null);
    assert_eq!(response["error"]["code"], -32700);
    assert_eq!(response["error"]["message"], message);
}

struct Fixture(PathBuf);

impl Fixture {
    fn new() -> Self {
        let root = std::env::temp_dir().join(format!(
            "semaprax-project-transport-oversize-{}-{}",
            std::process::id(),
            SERIAL.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir_all(root.join("src")).unwrap();
        let example = Path::new(env!("CARGO_MANIFEST_DIR")).join("examples/calculator-project");
        for path in [
            "semaprax.toml",
            "src/app.spx",
            "src/core.spx",
            "src/tests.spx",
        ] {
            std::fs::copy(example.join(path), root.join(path)).unwrap();
        }
        Self(root.canonicalize().unwrap())
    }

    fn manifest(&self) -> String {
        self.0.join("semaprax.toml").to_str().unwrap().to_owned()
    }

    fn inventory(&self) -> BTreeMap<PathBuf, Vec<u8>> {
        fn visit(path: &Path, facts: &mut BTreeMap<PathBuf, Vec<u8>>) {
            for entry in std::fs::read_dir(path).unwrap() {
                let entry = entry.unwrap();
                if entry.file_type().unwrap().is_dir() {
                    visit(&entry.path(), facts);
                } else {
                    facts.insert(entry.path(), std::fs::read(entry.path()).unwrap());
                }
            }
        }
        let mut facts = BTreeMap::new();
        visit(&self.0, &mut facts);
        facts
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

#[test]
fn project_session_rejects_oversize_without_lf_or_eof_and_finishes_authority() {
    let fixture = Fixture::new();
    let manifest = fixture.manifest();
    for profile in [
        None,
        Some("--allow-project-rename"),
        Some("--allow-project-workflow"),
    ] {
        let before = fixture.inventory();
        let mut arguments = vec!["semapraxd", "--stdio", "--manifest-path", manifest.as_str()];
        arguments.extend(["--max-request-bytes", "64"]);
        arguments.extend(profile);
        let mut pipe = OpenPipe::new();
        let mut output = Vec::new();
        // `Ok` means the loop ended and `finish_authority` accepted the
        // bound Project after the rejection.
        session::serve(&mut pipe, &mut output, parse(&arguments)).unwrap();
        assert_single_rejection(&output, "request exceeds configured byte limit", &pipe);
        assert_eq!(
            fixture.inventory(),
            before,
            "{profile:?} changed the Project"
        );
    }
}

#[test]
fn selected_law_profiles_reject_oversize_without_lf_or_eof() {
    // The workflow requires absolute paths; `/nonexistent/...` is not absolute
    // on Windows, so build never-created paths under the platform temp root.
    let missing = std::env::temp_dir().join("semaprax-oversize-test-nonexistent");
    let law_executable = missing.join("z3");
    let manifest_path = missing.join("semaprax.toml");
    let arguments = [
        "semapraxd",
        "--stdio",
        "--allow-project-law-workflow",
        "--law-tool",
        "z3",
        "--law-executable",
        law_executable.to_str().unwrap(),
        "--law-version-line",
        "Z3 version 0.0.0",
        "--manifest-path",
        manifest_path.to_str().unwrap(),
        "--max-request-bytes",
        "64",
    ];
    let config = parse(&arguments);
    let mut pipe = OpenPipe::new();
    let mut output = Vec::new();
    selected_law::serve_authenticated(
        &mut pipe,
        &mut output,
        &config,
        config.manifest_path(),
        config.law_tool().unwrap(),
    )
    .unwrap();
    assert_single_rejection(&output, "request exceeds configured byte limit", &pipe);

    let mut mcp_arguments = arguments.to_vec();
    mcp_arguments.push("--mcp");
    let config = parse(&mcp_arguments);
    let mut pipe = OpenPipe::new();
    let mut output = Vec::new();
    selected_law_mcp::serve_authenticated(&mut pipe, &mut output, &config).unwrap();
    assert_single_rejection(&output, "MCP request exceeds configured byte limit", &pipe);
}
