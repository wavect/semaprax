use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicU64, Ordering};

use semaprax::project::{
    with_authenticated_project, ProjectFrontendSource, ProjectManifest, SemanticQuery,
    SemanticServiceIndexQuery, SemanticTransaction, SemanticTransactionRenameDisplayName,
    SemanticWorkspaceServiceHistoryQuery,
};
use semaprax::semantic_service_mcp::{
    SemanticWorkspaceMcpSession, SEMANTIC_SERVICE_MCP_PROTOCOL_VERSION, SEMANTIC_SERVICE_MCP_SCHEMA,
};
use serde_json::{json, Value};

static SERIAL: AtomicU64 = AtomicU64::new(0);

struct Fixture(PathBuf);

impl Fixture {
    fn new() -> Self {
        let root = std::env::temp_dir().join(format!(
            "spx-semantic-service-mcp-v1-{}-{}",
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

    fn manifest_and_sources(&self) -> (ProjectManifest, Vec<ProjectFrontendSource>) {
        let manifest =
            ProjectManifest::parse(&std::fs::read_to_string(self.0.join("semaprax.toml")).unwrap())
                .unwrap();
        let sources = ["src/app.spx", "src/core.spx", "src/tests.spx"]
            .iter()
            .map(|path| {
                ProjectFrontendSource::new(
                    path,
                    &std::fs::read_to_string(self.0.join(path)).unwrap(),
                )
                .unwrap()
            })
            .collect();
        (manifest, sources)
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn request(id: Value, method: &str, params: Value) -> Vec<u8> {
    json!({"jsonrpc":"2.0","id":id,"method":method,"params":params})
        .to_string()
        .into_bytes()
}

fn response(session: &mut SemanticWorkspaceMcpSession, value: Vec<u8>) -> Value {
    serde_json::from_slice(&session.handle_frame(&value).unwrap()).unwrap()
}

fn initialize(session: &mut SemanticWorkspaceMcpSession) {
    let initialized = response(
        session,
        request(
            json!(1),
            "initialize",
            json!({"protocolVersion":"unknown","capabilities":{},"clientInfo":{"name":"test","version":"1"}}),
        ),
    );
    assert_eq!(
        initialized["result"]["protocolVersion"],
        SEMANTIC_SERVICE_MCP_PROTOCOL_VERSION
    );
    assert!(session
        .handle_frame(br#"{"jsonrpc":"2.0","method":"notifications/initialized"}"#)
        .is_none());
}

fn tool(session: &mut SemanticWorkspaceMcpSession, id: u64, name: &str, arguments: Value) -> Value {
    response(
        session,
        request(
            id.into(),
            "tools/call",
            json!({"name":name,"arguments":arguments}),
        ),
    )
}

fn inner(response: &Value) -> Value {
    serde_json::from_str(response["result"]["content"][0]["text"].as_str().unwrap()).unwrap()
}

#[test]
fn lifecycle_catalogue_and_tools_share_the_retained_authority_free_generation() {
    let fixture = Fixture::new();
    let revision = with_authenticated_project(&fixture.0.join("semaprax.toml"), |snapshot| {
        Ok(snapshot.retain_revision())
    })
    .unwrap();
    let mut session = SemanticWorkspaceMcpSession::open(revision).unwrap();
    let before = session
        .service()
        .active_generation()
        .workspace_revision()
        .to_owned();

    let early = response(&mut session, request(json!(1), "tools/list", json!({})));
    assert_eq!(early["error"]["code"], -32000);
    initialize(&mut session);
    let listed = response(&mut session, request(json!(2), "tools/list", json!({})));
    let names = listed["result"]["tools"]
        .as_array()
        .unwrap()
        .iter()
        .map(|tool| tool["name"].as_str().unwrap())
        .collect::<Vec<_>>();
    assert_eq!(
        names,
        [
            "service__protocol",
            "workspace__status",
            "workspace__query",
            "workspace__index_query",
            "workspace__history_query",
            "workspace__validate_transaction",
            "workspace__refresh",
        ]
    );
    assert_eq!(
        SEMANTIC_SERVICE_MCP_SCHEMA,
        "semaprax.semantic-workspace-service-mcp.v1"
    );

    let protocol = inner(&tool(&mut session, 3, "service__protocol", json!({})));
    assert_eq!(protocol["result"]["authority"], false);
    let status = inner(&tool(&mut session, 4, "workspace__status", json!({})));
    assert_eq!(status["result"]["workspace_revision"], before);
    assert_eq!(status["result"]["payload"]["opened"], true);

    let query = SemanticQuery::symbol(&before, "calculator.add").unwrap();
    let queried = tool(
        &mut session,
        5,
        "workspace__query",
        json!({"query":query.to_json()}),
    );
    assert_eq!(queried["result"]["isError"], false);
    assert_eq!(inner(&queried)["result"]["workspace_revision"], before);

    let index_query =
        SemanticServiceIndexQuery::tests_covering_declaration(&before, "calculator.add").unwrap();
    let indexed = tool(
        &mut session,
        51,
        "workspace__index_query",
        json!({"query":index_query.to_json()}),
    );
    assert_eq!(indexed["result"]["isError"], false);
    assert_eq!(inner(&indexed)["result"]["workspace_revision"], before);

    let transaction = SemanticTransaction::rename_display_name(
        &before,
        SemanticTransactionRenameDisplayName::new("calculator.add", "add", "sum"),
    )
    .unwrap();
    let validated = tool(
        &mut session,
        52,
        "workspace__validate_transaction",
        json!({"transaction":transaction.to_json()}),
    );
    assert_eq!(validated["result"]["isError"], false);
    assert_eq!(inner(&validated)["result"]["workspace_revision"], before);

    let history_query = SemanticWorkspaceServiceHistoryQuery::new(&before, 0, 64).unwrap();
    let history = tool(
        &mut session,
        521,
        "workspace__history_query",
        json!({"query":history_query.to_json()}),
    );
    assert_eq!(history["result"]["isError"], false);
    assert_eq!(
        inner(&history)["result"]["payload"]["value"]["items"]
            .as_array()
            .unwrap()
            .len(),
        1
    );

    let (manifest, sources) = fixture.manifest_and_sources();
    let refreshed = tool(
        &mut session,
        53,
        "workspace__refresh",
        json!({
            "expected_workspace_revision":before,
            "manifest":manifest.to_canonical_toml(),
            "sources":sources.iter().map(|source| json!({"path":source.path(),"source":source.source()})).collect::<Vec<_>>()
        }),
    );
    assert_eq!(refreshed["result"]["isError"], false);
    assert_eq!(inner(&refreshed)["result"]["workspace_revision"], before);
    assert_eq!(
        inner(&refreshed)["result"]["payload"]["generation_reused"],
        true
    );
    assert_eq!(
        session.service().active_generation().workspace_revision(),
        before
    );

    let unknown = tool(&mut session, 6, "workspace__open", json!({}));
    assert_eq!(unknown["error"]["code"], -32602);
}

#[test]
fn real_service_mcp_cli_uses_ndjson_and_performs_no_post_startup_path_reads() {
    let fixture = Fixture::new();
    let mut child = Command::new(env!("CARGO_BIN_EXE_semaprax"))
        .arg("service")
        .arg(fixture.0.join("semaprax.toml"))
        .arg("--mcp")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    let mut input = child.stdin.take().unwrap();
    let mut output = BufReader::new(child.stdout.take().unwrap());
    let initialize = request(
        json!(1),
        "initialize",
        json!({"protocolVersion":"2025-11-25","capabilities":{},"clientInfo":{"name":"cli","version":"1"}}),
    );
    input.write_all(&initialize).unwrap();
    input.write_all(b"\n").unwrap();
    input.flush().unwrap();
    let mut first = String::new();
    output.read_line(&mut first).unwrap();
    assert_eq!(serde_json::from_str::<Value>(&first).unwrap()["id"], 1);

    // Startup admission has ended: neither later calls nor EOF may reopen the
    // original source paths.
    std::fs::rename(fixture.0.join("src"), fixture.0.join("moved-src")).unwrap();
    for frame in [
        br#"{"jsonrpc":"2.0","method":"notifications/initialized"}"#.to_vec(),
        request(
            json!(2),
            "tools/call",
            json!({"name":"workspace__status","arguments":{}}),
        ),
    ] {
        input.write_all(&frame).unwrap();
        input.write_all(b"\n").unwrap();
    }
    input.flush().unwrap();
    let mut second = String::new();
    output.read_line(&mut second).unwrap();
    let status: Value = serde_json::from_str(&second).unwrap();
    assert_eq!(status["id"], 2);
    assert_eq!(inner(&status)["result"]["authority"], false);
    drop(input);
    let completed = child.wait_with_output().unwrap();
    assert!(
        completed.status.success(),
        "{}",
        String::from_utf8_lossy(&completed.stderr)
    );
    assert!(completed.stderr.is_empty());
}
