//! Two finite sessions bootstrap revisions from the packaged daemon itself.
use super::{calculator, command, Release};
use semaprax::project::{verify_execution_envelope, with_authenticated_project};
use serde_json::{json, Value};
use std::path::Path;
use std::process::Command;
use std::time::Duration;

fn session(release: &Release, project: &Path, captures: &Path, requests: &[Value]) -> Vec<Value> {
    let mut input = Vec::new();
    for request in requests {
        input.extend(serde_json::to_vec(request).unwrap());
        input.push(b'\n');
    }
    assert!(input.len() < 8192);
    let output = command::run(
        Command::new(&release.daemon)
            .args(["--stdio", "--manifest-path"])
            .arg(project.join("semaprax.toml"))
            .args([
                "--max-request-bytes",
                "65536",
                "--max-response-bytes",
                "1048576",
            ])
            .current_dir(project),
        &input,
        captures,
        Duration::from_secs(30),
        8 * 1024 * 1024,
        65536,
    );
    assert!(output.status.success(), "{output:?}");
    assert!(output.stderr.is_empty(), "{output:?}");
    assert!(output.stdout.ends_with(b"\n"));
    assert!(!output.stdout.contains(&b'\r'));
    let lines = output.stdout[..output.stdout.len() - 1]
        .split(|byte| *byte == b'\n')
        .collect::<Vec<_>>();
    let rows = lines
        .iter()
        .map(|line| {
            assert!(!line.is_empty() && line.len() < 1048576);
            serde_json::from_slice::<Value>(line).unwrap()
        })
        .collect::<Vec<_>>();
    assert_eq!(rows.len(), requests.len());
    for ((response, request), line) in rows.iter().zip(requests).zip(lines) {
        assert_eq!(response["jsonrpc"], "2.0");
        assert_eq!(response["id"], request["id"]);
        assert!(response.get("result").is_some() ^ response.get("error").is_some());
        assert_eq!(response.as_object().unwrap().len(), 3);
        if request["method"] == "test" {
            // Preserve the original canonical envelope bytes; serde Value
            // reserialization would reorder its keys before verification.
            let prefix = format!("{{\"jsonrpc\":\"2.0\",\"id\":{},\"result\":{{\"command_succeeded\":true,\"execution\":", request["id"]);
            let line = std::str::from_utf8(line).unwrap();
            let envelope = line
                .strip_prefix(&prefix)
                .unwrap()
                .strip_suffix("}}")
                .unwrap();
            verify_execution_envelope(envelope).unwrap();
        }
    }
    rows
}

fn request(id: u64, method: &str) -> Value {
    json!({"jsonrpc":"2.0","id":id,"method":method})
}

pub(super) fn run(release: &Release, project: &Path, captures: &Path) {
    let before = calculator::inventory(project);
    let expected = with_authenticated_project(&project.join("semaprax.toml"), |snapshot| {
        Ok(snapshot.retain_revision())
    })
    .unwrap();
    assert_eq!(
        expected
            .sources()
            .iter()
            .map(|source| source.path())
            .collect::<Vec<_>>(),
        ["src/app.spx", "src/tests.spx"]
    );
    for source in expected.sources() {
        assert_eq!(source.source().as_bytes(), before[source.path()]);
    }
    let bootstrap = session(
        release,
        project,
        &captures.join("daemon-bootstrap"),
        &[
            request(1, "protocol"),
            request(2, "workspace/open"),
            request(3, "shutdown"),
        ],
    );
    assert_eq!(
        bootstrap[0]["result"]["protocol"],
        "semaprax.agent-transport.v2"
    );
    assert_eq!(bootstrap[0]["result"]["state"], "configured");
    assert_eq!(bootstrap[0]["result"]["version"], env!("CARGO_PKG_VERSION"));
    assert_eq!(
        bootstrap[0]["result"]["limits"],
        json!({"max_request_bytes":65536,"max_response_bytes":1048576})
    );
    assert_eq!(
        bootstrap[0]["result"]["bound_manifest"],
        json!({"path":project.join("semaprax.toml").display().to_string(),"project_schema":"semaprax.project.v1"})
    );
    assert_eq!(
        bootstrap[0]["result"]["methods"],
        json!([
            "check",
            "context",
            "graph",
            "ping",
            "protocol",
            "shutdown",
            "test",
            "workspace/open",
            "workspace/snapshot",
            "workspace/status"
        ])
    );
    assert_eq!(bootstrap[1]["result"]["opened"], true);
    assert_eq!(bootstrap[2]["result"], json!({"ok":true}));
    let open = bootstrap[1]["result"].clone();
    assert_eq!(
        open,
        json!({"opened":true,"project_revision":expected.project_revision(),"workspace_revision":expected.workspace_revision()})
    );
    let params = json!({"project_revision":open["project_revision"], "workspace_revision":open["workspace_revision"]});
    for name in ["project_revision", "workspace_revision"] {
        let value = params[name].as_str().unwrap();
        assert!(value.starts_with("sha256:") && value.len() == 71);
        assert!(value[7..]
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte)));
    }
    assert_eq!(calculator::inventory(project), before);
    let mut requests = vec![request(10, "workspace/open")];
    for (id, method) in [
        (11, "workspace/snapshot"),
        (12, "check"),
        (13, "graph"),
        (14, "test"),
    ] {
        let mut row = request(id, method);
        row["params"] = params.clone();
        requests.push(row);
    }
    let mut stale = request(15, "check");
    stale["params"] = params.clone();
    let mut changed = params["project_revision"].as_str().unwrap().to_owned();
    changed.replace_range(7..8, if &changed[7..8] == "0" { "1" } else { "0" });
    stale["params"]["project_revision"] = Value::String(changed);
    requests.push(stale);
    let mut healthy = request(16, "check");
    healthy["params"] = params;
    requests.push(healthy);
    requests.push(request(17, "shutdown"));
    let rows = session(release, project, &captures.join("daemon-read"), &requests);
    assert_eq!(rows[0]["result"], open);
    let sources = expected
        .sources()
        .iter()
        .map(|source| {
            json!({
                "path":source.path(), "source_graph_schema":source.source_graph_schema(),
                "source_revision":source.source_revision(), "source_digest":source.source_digest(),
                "bytes":source.source().len()
            })
        })
        .collect::<Vec<_>>();
    assert_eq!(
        rows[1]["result"],
        json!({
            "schema":"semaprax.project-snapshot.v1", "project_schema":"semaprax.project.v1",
            "name":"archive-calculator", "entry":"archive_calculator.app",
            "test_module":"archive_calculator.tests", "project_revision":expected.project_revision(),
            "workspace_revision":expected.workspace_revision(),
            "manifest_bytes":expected.manifest().to_canonical_toml().len(), "sources":sources
        })
    );
    assert_eq!(rows[2]["result"], json!({"ok":true}));
    assert_eq!(
        rows[3]["result"]["graph"]["schema"],
        "semaprax.project-semantic-graph.v1"
    );
    let graph = &rows[3]["result"]["graph"];
    assert_eq!(
        *graph,
        serde_json::from_str::<Value>(expected.semantic_graph()).unwrap()
    );
    // The complete projection includes the compiler-owned prelude exactly
    // once, even when this calculator does not use Option or Result.
    assert_eq!(
        graph["declarations"]
            .as_array()
            .unwrap()
            .iter()
            .map(|row| row["id"].as_str().unwrap())
            .collect::<Vec<_>>(),
        [
            "archive-calculator.add",
            "archive-calculator.app.main",
            "archive-calculator.tests.main",
            "core.option",
            "core.option.none",
            "core.option.some",
            "core.option.some.value",
            "core.result",
            "core.result.err",
            "core.result.err.error",
            "core.result.ok",
            "core.result.ok.value"
        ]
    );
    assert_eq!(rows[4]["result"]["command_succeeded"], true);
    assert_eq!(
        rows[4]["result"]["execution"]["schema"],
        "semaprax.project-execution.v1"
    );
    let execution = &rows[4]["result"]["execution"];
    assert_eq!(execution["project_revision"], expected.project_revision());
    assert_eq!(
        execution["workspace_revision"],
        expected.workspace_revision()
    );
    assert_eq!(execution["project"], "archive-calculator");
    assert_eq!(execution["role"], "test");
    assert_eq!(execution["module"], "archive_calculator.tests");
    assert_eq!(execution["stable_id"], "archive-calculator.tests.main");
    assert_eq!(
        execution["outcome"],
        json!({"kind":"returned","type":"i64","value":"0"})
    );
    assert_eq!(rows[5]["error"]["code"], -32602);
    assert_eq!(rows[6]["result"], json!({"ok":true}));
    assert_eq!(rows[7]["result"], json!({"ok":true}));
    assert_eq!(calculator::inventory(project), before);
}
