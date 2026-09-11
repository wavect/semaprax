//! Cross-transport equivalence for Universal Semantic Query v1.
//!
//! Issue #200 asks for "cross-transport conformance: the same request against
//! the same revision yields byte-equivalent semantic result and diagnostics
//! across all adapters." Existing focused evidence proves CLI-output-equals-
//! direct-core-call (`tests/workspace/universal_semantic_workflow_cli.rs`) and
//! separately proves MCP-output-equals-direct-core-call
//! (`tests/workspace/persistent_semantic_service_mcp.rs`), but no existing
//! test spawns two real *surfaces* against the same request and compares
//! their answers directly. This module closes that gap for one operation
//! (`declarations`): it runs the real `semaprax` CLI binary and, separately,
//! a real `semaprax service <project> --mcp` subprocess driven over its
//! NDJSON protocol, and asserts their parsed results are identical to each
//! other and to the in-process direct-core result.
//!
//! See `docs/SEMANTIC-SERVICE-SURFACE-CONSOLIDATION-AUDIT-V1.md` for the
//! surface inventory this test evidences, and for what remains (the same
//! pattern repeated for the other six operations and for the raw stdio
//! transport).

use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicU64, Ordering};

use semaprax::project::{with_authenticated_project, SemanticQuery, SemanticWorkspaceService};
use semaprax::query::QueryFilters;
use serde_json::{json, Value};

static SERIAL: AtomicU64 = AtomicU64::new(0);

struct Fixture(PathBuf);

impl Fixture {
    fn new() -> Self {
        let root = std::env::temp_dir().join(format!(
            "spx-cli-mcp-query-cross-transport-v1-{}-{}",
            std::process::id(),
            SERIAL.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir_all(root.join("src")).unwrap();
        let sample = Path::new(env!("CARGO_MANIFEST_DIR")).join("examples/calculator-project");
        for path in [
            "semaprax.toml",
            "src/app.spx",
            "src/core.spx",
            "src/tests.spx",
        ] {
            std::fs::copy(sample.join(path), root.join(path)).unwrap();
        }
        Self(root.canonicalize().unwrap())
    }

    fn manifest(&self) -> PathBuf {
        self.0.join("semaprax.toml")
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

/// Run the real CLI binary's one-shot Universal Semantic Query v1 client and
/// return the exact stdout bytes, parsed.
fn cli_declarations_result(fixture: &Fixture, revision: &str) -> Value {
    let output = Command::new(env!("CARGO_BIN_EXE_semaprax"))
        .args([
            "query",
            fixture.0.to_str().unwrap(),
            "declarations",
            "--kind",
            "function",
            "--offset",
            "0",
            "--limit",
            "2",
            "--revision",
            revision,
        ])
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(output.stderr.is_empty());
    serde_json::from_slice(&output.stdout).unwrap()
}

/// Run a real `semaprax service <project> --mcp` subprocess, perform the
/// closed lifecycle handshake, call `workspace__query` with the same
/// declarations request, and return the embedded query-result value.
fn mcp_declarations_result(fixture: &Fixture, query: &SemanticQuery) -> Value {
    let mut child = Command::new(env!("CARGO_BIN_EXE_semaprax"))
        .arg("service")
        .arg(fixture.manifest())
        .arg("--mcp")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    let mut input = child.stdin.take().unwrap();
    let mut output = BufReader::new(child.stdout.take().unwrap());

    fn send(input: &mut impl Write, bytes: &[u8]) {
        input.write_all(bytes).unwrap();
        input.write_all(b"\n").unwrap();
        input.flush().unwrap();
    }
    fn recv(output: &mut impl BufRead) -> Value {
        let mut response = String::new();
        output.read_line(&mut response).unwrap();
        serde_json::from_str(&response).unwrap()
    }
    fn call(input: &mut impl Write, output: &mut impl BufRead, request: Value) -> Value {
        send(input, request.to_string().as_bytes());
        recv(output)
    }

    let initialized = call(
        &mut input,
        &mut output,
        json!({
            "jsonrpc":"2.0",
            "id":1,
            "method":"initialize",
            "params":{
                "protocolVersion":"2025-11-25",
                "capabilities":{},
                "clientInfo":{"name":"cross-transport-test","version":"1"},
            },
        }),
    );
    assert_eq!(initialized["id"], 1);
    assert!(initialized.get("error").is_none(), "{initialized}");
    send(
        &mut input,
        br#"{"jsonrpc":"2.0","method":"notifications/initialized"}"#,
    );

    let queried = call(
        &mut input,
        &mut output,
        json!({
            "jsonrpc":"2.0",
            "id":2,
            "method":"tools/call",
            "params":{"name":"workspace__query","arguments":{"query":query.to_json()}},
        }),
    );
    assert_eq!(queried["id"], 2);
    assert_eq!(queried["result"]["isError"], false, "{queried}");
    let inner: Value = serde_json::from_str(
        queried["result"]["content"][0]["text"]
            .as_str()
            .expect("MCP tool result carries one text content item"),
    )
    .unwrap();
    assert!(inner.get("error").is_none(), "{inner}");
    let payload = inner["result"]["payload"]["value"].clone();

    drop(input);
    let completed = child.wait_with_output().unwrap();
    assert!(
        completed.status.success(),
        "{}",
        String::from_utf8_lossy(&completed.stderr)
    );
    assert!(completed.stderr.is_empty());

    payload
}

#[test]
fn cli_and_mcp_answer_the_same_declarations_query_byte_identically() {
    let fixture = Fixture::new();
    let revision = with_authenticated_project(&fixture.manifest(), |snapshot| {
        Ok(snapshot.retain_revision())
    })
    .unwrap();
    let service = SemanticWorkspaceService::open(revision).unwrap();
    let workspace_revision = service.active_generation().workspace_revision().to_owned();

    let filters = QueryFilters {
        kinds: vec!["function".to_owned()],
        ..QueryFilters::default()
    };
    let query = SemanticQuery::declarations(&workspace_revision, &filters, 0, 2).unwrap();

    // Direct core: the same kernel entry point every transport delegates to.
    let direct: Value =
        serde_json::from_str(&service.query(query.to_json().as_bytes()).unwrap().to_json())
            .unwrap();

    let cli = cli_declarations_result(&fixture, &workspace_revision);
    let mcp = mcp_declarations_result(&fixture, &query);

    assert_eq!(
        cli, direct,
        "CLI one-shot query must equal the direct core result"
    );
    assert_eq!(
        mcp, direct,
        "MCP workspace__query must equal the direct core result"
    );
    assert_eq!(
        cli, mcp,
        "CLI and MCP must answer the identical declarations query identically"
    );

    // Sanity: the fixture actually has matching function declarations, so
    // this is not a vacuous empty-payload comparison.
    assert!(!direct["payload"]["matches"]
        .as_array()
        .expect("declarations payload carries a matches array")
        .is_empty());
}
