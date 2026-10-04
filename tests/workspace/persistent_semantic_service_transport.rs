use std::collections::BTreeMap;
use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, ChildStdin, ChildStdout, Command, Stdio};
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

use semaprax::project::{
    ProjectCandidate, ProjectFrontendSource, ProjectManifest, ProjectRevision, SemanticQuery,
    SemanticServiceIndexQuery, SemanticTransaction, SemanticTransactionRenameDisplayName,
    SemanticTransactionReplaceExpression, SemanticTransactionV2, SemanticWorkspaceService,
    SemanticWorkspaceServiceHistoryQuery, with_authenticated_project,
};
use semaprax::semantic_service_transport::{
    MAX_SEMANTIC_SERVICE_REQUEST_BYTES, MAX_SEMANTIC_SERVICE_RESPONSE_BYTES,
    SEMANTIC_SERVICE_TRANSPORT_ERROR_SCHEMA, SEMANTIC_SERVICE_TRANSPORT_RESULT_SCHEMA,
    SEMANTIC_SERVICE_TRANSPORT_SCHEMA, SemanticWorkspaceStdioSession,
};
use serde_json::{Value, json};

static SERIAL: AtomicU64 = AtomicU64::new(0);
const PATHS: [&str; 3] = ["src/app.spx", "src/core.spx", "src/tests.spx"];

struct Fixture(PathBuf);

impl Fixture {
    fn new() -> Self {
        let root = std::env::temp_dir().join(format!(
            "spx-persistent-semantic-transport-v1-{}-{}",
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

    fn revision(&self) -> Arc<ProjectRevision> {
        with_authenticated_project(&self.0.join("semaprax.toml"), |snapshot| {
            Ok(snapshot.retain_revision())
        })
        .unwrap()
    }

    fn manifest(&self) -> ProjectManifest {
        ProjectManifest::parse(&std::fs::read_to_string(self.0.join("semaprax.toml")).unwrap())
            .unwrap()
    }

    fn sources(&self) -> Vec<ProjectFrontendSource> {
        PATHS
            .iter()
            .map(|path| {
                ProjectFrontendSource::new(
                    path,
                    &std::fs::read_to_string(self.0.join(path)).unwrap(),
                )
                .unwrap()
            })
            .collect()
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn inventory(root: &Path) -> BTreeMap<PathBuf, Vec<u8>> {
    fn visit(root: &Path, current: &Path, result: &mut BTreeMap<PathBuf, Vec<u8>>) {
        let mut paths = std::fs::read_dir(current)
            .unwrap()
            .map(|entry| entry.unwrap().path())
            .collect::<Vec<_>>();
        paths.sort();
        for path in paths {
            let relative = path.strip_prefix(root).unwrap().to_owned();
            if path.is_dir() {
                result.insert(relative, Vec::new());
                visit(root, &path, result);
            } else {
                result.insert(relative, std::fs::read(path).unwrap());
            }
        }
    }
    let mut result = BTreeMap::new();
    visit(root, root, &mut result);
    result
}

fn request(id: Value, method: &str, params: Value) -> Value {
    json!({"jsonrpc":"2.0","id":id,"method":method,"params":params})
}

fn call(
    session: &mut SemanticWorkspaceStdioSession,
    id: Value,
    method: &str,
    params: Value,
) -> Value {
    let bytes = session
        .handle_frame(request(id, method, params).to_string().as_bytes())
        .unwrap();
    assert!(bytes.len() <= MAX_SEMANTIC_SERVICE_RESPONSE_BYTES);
    serde_json::from_slice(&bytes).unwrap()
}

fn result(response: &Value) -> &Value {
    assert_eq!(response["jsonrpc"], "2.0");
    assert!(response.get("error").is_none(), "{response}");
    &response["result"]
}

fn error_code(response: &Value) -> &str {
    assert_eq!(
        response["error"]["data"]["schema"],
        SEMANTIC_SERVICE_TRANSPORT_ERROR_SCHEMA
    );
    response["error"]["data"]["diagnostics"][0]["code"]
        .as_str()
        .unwrap()
}

fn source_values(sources: &[ProjectFrontendSource]) -> Value {
    Value::Array(
        sources
            .iter()
            .map(|source| json!({"path":source.path(),"source":source.source()}))
            .collect(),
    )
}

fn changed_sources(sources: &[ProjectFrontendSource]) -> Vec<ProjectFrontendSource> {
    sources
        .iter()
        .map(|source| {
            let text = if source.path() == "src/app.spx" {
                let changed = source.source().replace("multiply(6, 7)", "multiply(6, 8)");
                let program = semaprax::parse(&changed, source.path()).unwrap();
                semaprax::format::canonical(&program)
            } else {
                source.source().to_owned()
            };
            ProjectFrontendSource::new(source.path(), &text).unwrap()
        })
        .collect()
}

fn query(revision: &str) -> SemanticQuery {
    SemanticQuery::symbol(revision, "calculator.add").unwrap()
}

fn transaction(revision: &str) -> SemanticTransaction {
    SemanticTransaction::rename_display_name(
        revision,
        SemanticTransactionRenameDisplayName::new("calculator.add", "add", "sum"),
    )
    .unwrap()
}

#[test]
fn one_session_retains_one_generation_and_delegates_exact_query_and_transaction_results() {
    let fixture = Fixture::new();
    let before = inventory(&fixture.0);
    let revision = fixture.revision();
    let direct = SemanticWorkspaceService::open(Arc::clone(&revision)).unwrap();
    let workspace = direct.active_generation().workspace_revision().to_owned();
    let mut session = SemanticWorkspaceStdioSession::open(Arc::clone(&revision)).unwrap();

    let protocol = result(&call(&mut session, json!(1), "service/protocol", json!({}))).clone();
    assert_eq!(protocol["schema"], SEMANTIC_SERVICE_TRANSPORT_SCHEMA);
    assert_eq!(protocol["authority"], false);
    assert_eq!(protocol["host_grants"], json!([]));
    assert_eq!(
        protocol["methods"],
        json!([
            "service/protocol",
            "workspace/open",
            "workspace/status",
            "workspace/query",
            "workspace/index-query",
            "workspace/history-query",
            "workspace/validate-transaction",
            "workspace/validate-transaction-v2",
            "workspace/validate-transaction-v2-workflow",
            "workspace/patch-receipt",
            "workspace/verify-patch-receipt",
            "workspace/patch-receipt-refusal",
            "workspace/verify-patch-receipt-refusal",
            "workspace/patch-receipt-evidence-summary",
            "workspace/patch-receipt-evidence-page",
            "workspace/compare-patch-receipts",
            "workspace/compare-patch-receipt-set",
            "workspace/compact-projection",
            "workspace/refresh",
            "shutdown"
        ])
    );
    assert_eq!(
        protocol["limits"],
        json!({"max_diagnostics":64,"max_request_bytes":MAX_SEMANTIC_SERVICE_REQUEST_BYTES,"max_response_bytes":MAX_SEMANTIC_SERVICE_RESPONSE_BYTES})
    );

    let opened = result(&call(&mut session, json!(2), "workspace/open", json!({}))).clone();
    assert_eq!(opened["schema"], SEMANTIC_SERVICE_TRANSPORT_RESULT_SCHEMA);
    assert_eq!(opened["workspace_revision"], workspace);
    assert_eq!(
        opened["payload"]["value"],
        serde_json::from_str::<Value>(direct.open_work().to_json()).unwrap()
    );
    let status = result(&call(&mut session, json!(3), "workspace/status", json!({}))).clone();
    assert_eq!(status["workspace_revision"], workspace);
    assert_eq!(status["payload"], json!({"opened":true,"state":"ready"}));

    let query = query(&workspace);
    let direct_query = direct.query(query.to_json().as_bytes()).unwrap();
    let queried = result(&call(
        &mut session,
        json!(4),
        "workspace/query",
        json!({"query":query.to_json()}),
    ))
    .clone();
    assert_eq!(queried["workspace_revision"], workspace);
    assert_eq!(
        queried["payload"]["value"],
        serde_json::from_str::<Value>(direct_query.to_json()).unwrap()
    );
    assert_eq!(
        queried["payload"]["result_digest"],
        direct_query.result_digest()
    );

    let index_query =
        SemanticServiceIndexQuery::tests_covering_declaration(&workspace, "calculator.add")
            .unwrap();
    let direct_index = direct
        .index_query(index_query.to_json().as_bytes())
        .unwrap();
    let indexed = result(&call(
        &mut session,
        json!(41),
        "workspace/index-query",
        json!({"query":index_query.to_json()}),
    ))
    .clone();
    assert_eq!(indexed["workspace_revision"], workspace);
    assert_eq!(
        indexed["payload"]["value"],
        serde_json::from_str::<Value>(direct_index.to_json()).unwrap()
    );
    assert_eq!(
        indexed["payload"]["result_digest"],
        direct_index.result_digest()
    );

    let transaction = transaction(&workspace);
    let direct_artifacts = direct
        .validate_transaction(transaction.to_json().as_bytes())
        .unwrap();
    let validated = result(&call(
        &mut session,
        json!(5),
        "workspace/validate-transaction",
        json!({"transaction":transaction.to_json()}),
    ))
    .clone();
    assert_eq!(validated["workspace_revision"], workspace);
    assert_eq!(
        validated["payload"]["result"],
        serde_json::from_str::<Value>(direct_artifacts.result()).unwrap()
    );
    assert_eq!(
        validated["payload"]["evidence"],
        serde_json::from_str::<Value>(direct_artifacts.evidence()).unwrap()
    );
    let history_query = SemanticWorkspaceServiceHistoryQuery::new(&workspace, 0, 64).unwrap();
    let history = result(&call(
        &mut session,
        json!(51),
        "workspace/history-query",
        json!({"query":history_query.to_json()}),
    ))
    .clone();
    assert_eq!(history["workspace_revision"], workspace);
    assert_eq!(
        history["payload"]["value"]["items"]
            .as_array()
            .unwrap()
            .len(),
        1
    );
    assert_eq!(
        history["payload"]["value"]["items"][0]["value"]["kind"],
        "transaction_validation"
    );

    let candidate =
        ProjectCandidate::open(Arc::clone(&revision), revision.project_revision()).unwrap();
    let catalog: Value =
        serde_json::from_str(&candidate.expression_catalog("calculator.add").unwrap()).unwrap();
    let source_path = catalog["source"]["path"].as_str().unwrap();
    let source = revision
        .sources()
        .iter()
        .find(|source| source.path() == source_path)
        .unwrap()
        .source();
    let entry = catalog["expressions"]
        .as_array()
        .unwrap()
        .iter()
        .find(|entry| {
            let start = entry["source_span"]["start"].as_u64().unwrap() as usize;
            let end = entry["source_span"]["end"].as_u64().unwrap() as usize;
            source.get(start..end) == Some("left + right")
        })
        .unwrap();
    let expression_id = entry["expression_id"].as_str().unwrap();
    let workflow_step = SemanticTransactionV2::replace_expression(
        &workspace,
        SemanticTransactionReplaceExpression::new(
            "calculator.add",
            expression_id,
            "left + right",
            json!({
                "kind": "binary", "op": "-",
                "left": {"kind": "place", "name": "left"},
                "right": {"kind": "place", "name": "right"},
            }),
        ),
    )
    .unwrap();
    let direct_v2 = direct
        .validate_transaction_v2(workflow_step.to_json().as_bytes())
        .unwrap();
    let validated_v2 = result(&call(
        &mut session,
        json!(52),
        "workspace/validate-transaction-v2",
        json!({"transaction":workflow_step.to_json()}),
    ))
    .clone();
    assert_eq!(validated_v2["workspace_revision"], workspace);
    for (field, value) in [
        (
            "candidate_revision",
            Value::String(
                direct_v2
                    .candidate()
                    .revision()
                    .project_revision()
                    .to_owned(),
            ),
        ),
        (
            "evidence",
            serde_json::from_str(direct_v2.evidence()).unwrap(),
        ),
        ("impact", serde_json::from_str(direct_v2.impact()).unwrap()),
        ("result", serde_json::from_str(direct_v2.result()).unwrap()),
        ("review", serde_json::from_str(direct_v2.review()).unwrap()),
    ] {
        assert_eq!(
            validated_v2["payload"][field], value,
            "v2 {field} must be exact"
        );
    }
    let workflowed = result(&call(
        &mut session,
        json!(53),
        "workspace/validate-transaction-v2-workflow",
        json!({"steps":[workflow_step.to_json()]}),
    ))
    .clone();
    assert_eq!(workflowed["workspace_revision"], workspace);
    assert_eq!(
        workflowed["payload"]["value"]["step_count"].as_u64(),
        Some(1)
    );
    assert_eq!(
        workflowed["payload"]["candidate_revision"],
        workflowed["payload"]["value"]["result"]["project_revision"]
    );

    assert_eq!(
        session.service().active_generation().workspace_revision(),
        workspace
    );
    assert_eq!(inventory(&fixture.0), before);
}

#[test]
fn refresh_is_atomic_reusable_cold_equivalent_and_rolls_back_stale_or_failed_inputs() {
    let fixture = Fixture::new();
    let before = inventory(&fixture.0);
    let manifest = fixture.manifest();
    let manifest_text = manifest.to_canonical_toml();
    let sources = fixture.sources();
    let changed = changed_sources(&sources);
    let mut session = SemanticWorkspaceStdioSession::open(fixture.revision()).unwrap();
    result(&call(&mut session, json!(1), "workspace/open", json!({})));
    let initial = session
        .service()
        .active_generation()
        .workspace_revision()
        .to_owned();

    let unchanged = result(&call(
        &mut session,
        json!(2),
        "workspace/refresh",
        json!({"expected_workspace_revision":&initial,"manifest":&manifest_text,"sources":source_values(&sources)}),
    ))
    .clone();
    assert_eq!(unchanged["workspace_revision"], initial);
    assert_eq!(unchanged["payload"]["generation_reused"], true);

    let refreshed = result(&call(
        &mut session,
        json!(3),
        "workspace/refresh",
        json!({"expected_workspace_revision":&initial,"manifest":&manifest_text,"sources":source_values(&changed)}),
    ))
    .clone();
    let current = refreshed["workspace_revision"].as_str().unwrap().to_owned();
    assert_ne!(current, initial);
    assert_eq!(refreshed["payload"]["generation_reused"], false);
    let mut cold_cache = semaprax::project::ProjectFrontendCache::new_with_semantic_cache();
    let cold = cold_cache
        .build(&manifest, &changed)
        .unwrap()
        .into_revision();
    assert_eq!(
        session
            .service()
            .active_generation()
            .revision()
            .project_revision(),
        cold.project_revision()
    );

    let stale = call(
        &mut session,
        json!(4),
        "workspace/refresh",
        json!({"expected_workspace_revision":&initial,"manifest":&manifest_text,"sources":source_values(&sources)}),
    );
    assert_eq!(error_code(&stale), "SPX-G530");
    assert_eq!(
        session.service().active_generation().workspace_revision(),
        current
    );
    let invalid = call(
        &mut session,
        json!(5),
        "workspace/refresh",
        json!({"expected_workspace_revision":&current,"manifest":&manifest_text,"sources":[{"path":"src/app.spx","source":"invalid"}]}),
    );
    assert!(invalid.get("error").is_some());
    assert_eq!(
        session.service().active_generation().workspace_revision(),
        current
    );
    let old_query = query(&initial);
    assert_eq!(
        error_code(&call(
            &mut session,
            json!(6),
            "workspace/query",
            json!({"query":old_query.to_json()}),
        )),
        "SPX-G530"
    );
    let status = result(&call(&mut session, json!(7), "workspace/status", json!({}))).clone();
    assert_eq!(status["workspace_revision"], current);
    assert_eq!(inventory(&fixture.0), before);
}

#[test]
fn malformed_unknown_oversized_and_lifecycle_inputs_fail_closed_without_mutation() {
    let fixture = Fixture::new();
    let before = inventory(&fixture.0);
    let mut session = SemanticWorkspaceStdioSession::open(fixture.revision()).unwrap();
    let initial = session
        .service()
        .active_generation()
        .workspace_revision()
        .to_owned();
    let query = query(&initial);
    assert_eq!(
        error_code(&call(
            &mut session,
            json!(1),
            "workspace/query",
            json!({"query":query.to_json()}),
        )),
        "SPX-G548"
    );
    let malformed: Value = serde_json::from_slice(
        &session
            .handle_frame(br#"{"jsonrpc":"2.0","id":2,"method":7,"params":{}}"#)
            .unwrap(),
    )
    .unwrap();
    assert_eq!(malformed["error"]["code"], -32600);
    let unknown = call(&mut session, json!(3), "workspace/unknown", json!({}));
    assert_eq!(error_code(&unknown), "SPX-G548");
    let oversized = vec![b' '; MAX_SEMANTIC_SERVICE_REQUEST_BYTES + 1];
    let response: Value =
        serde_json::from_slice(&session.handle_frame(&oversized).unwrap()).unwrap();
    assert_eq!(response["error"]["code"], -32001);
    assert!(response.to_string().len() <= MAX_SEMANTIC_SERVICE_RESPONSE_BYTES);
    assert_eq!(
        session.service().active_generation().workspace_revision(),
        initial
    );

    result(&call(&mut session, json!(4), "workspace/open", json!({})));
    let closed = call(&mut session, json!(5), "shutdown", json!({}));
    assert_eq!(result(&closed)["payload"]["shutdown"], true);
    assert!(session.is_terminal());
    assert!(
        session
            .handle_frame(br#"{"jsonrpc":"2.0","method":"shutdown","params":{}}"#)
            .is_none()
    );
    assert_eq!(inventory(&fixture.0), before);
}

struct LiveService {
    child: Child,
    input: ChildStdin,
    output: BufReader<ChildStdout>,
}

impl LiveService {
    fn start(fixture: &Fixture) -> Self {
        let mut child = Command::new(env!("CARGO_BIN_EXE_semaprax"))
            .arg("service")
            .arg(fixture.0.join("semaprax.toml"))
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .unwrap();
        let input = child.stdin.take().unwrap();
        let output = BufReader::new(child.stdout.take().unwrap());
        Self {
            child,
            input,
            output,
        }
    }

    fn call(&mut self, value: Value) -> Value {
        let frame = value.to_string();
        assert!(frame.len() <= MAX_SEMANTIC_SERVICE_REQUEST_BYTES);
        self.input.write_all(frame.as_bytes()).unwrap();
        self.input.write_all(b"\n").unwrap();
        self.input.flush().unwrap();
        let mut line = String::new();
        self.output.read_line(&mut line).unwrap();
        assert!(!line.is_empty());
        assert!(line.len() <= MAX_SEMANTIC_SERVICE_RESPONSE_BYTES + 1);
        serde_json::from_str(&line).unwrap()
    }

    fn finish(mut self) {
        let response = self.call(request(json!(99), "shutdown", json!({})));
        assert_eq!(result(&response)["payload"]["shutdown"], true);
        drop(self.input);
        let output = self.child.wait_with_output().unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert!(output.stderr.is_empty());
    }
}

#[test]
fn real_service_cli_retains_the_same_process_generation_across_multiple_requests() {
    let fixture = Fixture::new();
    let before = inventory(&fixture.0);
    let revision = fixture.revision();
    let direct = SemanticWorkspaceService::open(Arc::clone(&revision)).unwrap();
    let workspace = direct.active_generation().workspace_revision().to_owned();
    let mut live = LiveService::start(&fixture);
    let protocol = live.call(request(json!(1), "service/protocol", json!({})));
    assert_eq!(
        result(&protocol)["schema"],
        SEMANTIC_SERVICE_TRANSPORT_SCHEMA
    );
    let opened = live.call(request(json!(2), "workspace/open", json!({})));
    assert_eq!(result(&opened)["workspace_revision"], workspace);
    let query = query(&workspace);
    let queried = live.call(request(
        json!(3),
        "workspace/query",
        json!({"query":query.to_json()}),
    ));
    assert_eq!(result(&queried)["workspace_revision"], workspace);
    let status = live.call(request(json!(4), "workspace/status", json!({})));
    assert_eq!(result(&status)["workspace_revision"], workspace);
    assert_eq!(
        result(&status)["payload"],
        json!({"opened":true,"state":"ready"})
    );
    live.finish();
    assert_eq!(inventory(&fixture.0), before);

    // The new protocol is additive and does not reuse or replace frozen v5.
    assert_eq!(
        semaprax::image_transport::VNEXT_PROTOCOL_SCHEMA,
        "semaprax.image-agent-protocol.v5"
    );
}

#[test]
fn patch_receipt_adapters_replay_retained_candidates_without_history_or_path_inputs() {
    let fixture = Fixture::new();
    let revision = fixture.revision();
    let direct = SemanticWorkspaceService::open(Arc::clone(&revision)).unwrap();
    let workspace = direct.active_generation().workspace_revision().to_owned();
    let transaction = transaction(&workspace).to_json().to_owned();
    let candidate = direct
        .validate_transaction(transaction.as_bytes())
        .unwrap()
        .candidate()
        .candidate_digest()
        .to_owned();
    let mut session = SemanticWorkspaceStdioSession::open(revision).unwrap();
    result(&call(&mut session, json!(1), "workspace/open", json!({})));

    let receipt = result(&call(
        &mut session,
        json!(2),
        "workspace/patch-receipt",
        json!({"transaction":transaction,"candidate_digest":candidate}),
    ))["payload"]["value"]
        .clone();
    assert_eq!(receipt["schema"], "semaprax.patch-receipt.v1");
    let receipt_text = format!("{}\n", receipt);
    let verified_response = call(
        &mut session,
        json!(3),
        "workspace/verify-patch-receipt",
        json!({"transaction":transaction,"candidate_digest":candidate,"receipt":receipt_text}),
    );
    let verified = result(&verified_response);
    assert_eq!(
        verified["payload"]["value"]["result"],
        "exact_recomputation"
    );

    let summary = result(&call(
        &mut session,
        json!(4),
        "workspace/patch-receipt-evidence-summary",
        json!({"transaction":transaction,"candidate_digest":candidate}),
    ))["payload"]["value"]
        .clone();
    let catalog = summary["evidence"]
        .as_array()
        .unwrap()
        .iter()
        .find(|entry| entry["id"] == "declaration_catalog")
        .unwrap();
    let page_response = call(
        &mut session,
        json!(5),
        "workspace/patch-receipt-evidence-page",
        json!({"transaction":transaction,"candidate_digest":candidate,"evidence_id":"declaration_catalog","handle":catalog["handle"],"cursor":null,"page_size":1,"max_bytes":65536}),
    );
    let page = result(&page_response);
    assert_eq!(
        page["payload"]["value"]["schema"],
        "semaprax.patch-receipt-evidence-page.v1"
    );

    let stale = "sha256:1111111111111111111111111111111111111111111111111111111111111111";
    let refusal = result(&call(
        &mut session,
        json!(6),
        "workspace/patch-receipt-refusal",
        json!({"transaction":transaction,"requested_candidate_digest":stale}),
    ))["payload"]["value"]
        .clone();
    assert_eq!(
        refusal["content"]["attempt"]["status"],
        "refused_stale_candidate_selector"
    );
    let refusal_text = format!("{}\n", refusal);
    assert_eq!(
        result(&call(
            &mut session,
            json!(7),
            "workspace/verify-patch-receipt-refusal",
            json!({"transaction":transaction,"requested_candidate_digest":stale,"receipt":refusal_text}),
        ))["payload"]["value"]["result"],
        "exact_refusal_recomputation"
    );
    assert_eq!(
        result(&call(
            &mut session,
            json!(8),
            "workspace/compare-patch-receipts",
            json!({"left_transaction":transaction,"left_candidate_digest":candidate,"left_receipt":receipt_text,"right_transaction":transaction,"right_candidate_digest":candidate,"right_receipt":receipt_text}),
        ))["payload"]["value"]["result"],
        "comparable"
    );
    assert_eq!(
        result(&call(
            &mut session,
            json!(9),
            "workspace/compare-patch-receipt-set",
            json!({"receipts":[
                {"transaction":transaction,"candidate_digest":candidate,"receipt":receipt_text},
                {"transaction":transaction,"candidate_digest":candidate,"receipt":receipt_text},
                {"transaction":transaction,"candidate_digest":candidate,"receipt":receipt_text}
            ]}),
        ))["payload"]["value"]["result"],
        "comparable"
    );

    let history = SemanticWorkspaceServiceHistoryQuery::new(&workspace, 0, 64).unwrap();
    assert!(
        result(&call(
            &mut session,
            json!(10),
            "workspace/history-query",
            json!({"query":history.to_json()}),
        ))["payload"]["value"]["items"]
            .as_array()
            .unwrap()
            .is_empty()
    );
    let stale_transaction = transaction.replace(
        &workspace,
        "sha256:1111111111111111111111111111111111111111111111111111111111111111",
    );
    assert_eq!(
        error_code(&call(
            &mut session,
            json!(10),
            "workspace/patch-receipt",
            json!({"transaction":stale_transaction,"candidate_digest":candidate}),
        )),
        "SPX-G530"
    );
}
