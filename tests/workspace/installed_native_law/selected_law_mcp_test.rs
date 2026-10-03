use super::*;
use semaprax::project::install_host_strict_law_policy;
use serde_json::{json, Value};
use std::io::{BufRead, Write};
use std::process::{Command, Stdio};

fn call(
    input: &mut impl Write,
    output: &mut impl BufRead,
    id: u64,
    method: &str,
    params: Value,
) -> Value {
    let request = json!({"jsonrpc":"2.0","id":id,"method":method,"params":params});
    writeln!(input, "{request}").unwrap();
    input.flush().unwrap();
    let mut line = String::new();
    assert!(
        output.read_line(&mut line).unwrap() > 0,
        "MCP daemon exited"
    );
    serde_json::from_str(&line).unwrap()
}

fn tool_call(
    input: &mut impl Write,
    output: &mut impl BufRead,
    id: u64,
    name: &str,
    arguments: Value,
) -> (Value, Value) {
    let outer = call(
        input,
        output,
        id,
        "tools/call",
        json!({"name":name,"arguments":arguments}),
    );
    let inner: Value =
        serde_json::from_str(outer["result"]["content"][0]["text"].as_str().unwrap()).unwrap();
    (outer, inner)
}

#[test]
#[ignore = "requires explicitly provisioned installed Z3"]
fn mcp_selected_law_rechecks_current_candidate_without_new_authority() {
    let project = native_project("law12-mcp-repair", "n + 0 == n");
    let law_source = "module fresh.laws;\n@id(\"fresh.law.seventeen\")\nlaw contract \"fresh.seventeen\" ensures (a: i64, result: i64)\n result == a + 17\n evidence smt_proved;\n";
    let law = semaprax::native_law_source::canonical(
        &semaprax::native_law_source::parse(law_source, "src/contracts.spx").unwrap(),
    );
    std::fs::write(project.root.join("src/contracts.spx"), law).unwrap();
    let bad = semaprax::format::canonical(&semaprax::parse(
        "module app.fresh; @id(\"fresh.seventeen\") fn seventeen(a: i64) -> i64 requires a >= 0 requires a <= 100 ensures result == a + 17 { a + 16 } @id(\"fresh.main\") fn main() -> i64 { seventeen(0) }",
        "src/app.spx",
    ).unwrap());
    std::fs::write(project.root.join("src/app.spx"), &bad).unwrap();
    let revision = project.revision();
    let laws = LawSet::derive(&revision, "law12-mcp-v1", revision.law_modules().to_vec()).unwrap();
    let tool = provisioned(&project, ToolKind::Z3);
    let policy = StrictLawPolicy::new(
        laws,
        BTreeMap::from([(
            "fresh.law.seventeen".into(),
            RequiredLawEvidence::PinnedSmtSource {
                toolchain: tool.expected_version().into(),
                accepted_translation: semaprax::assurance_manifest::smt_discharge::BOUNDS_V1.into(),
            },
        )]),
    )
    .unwrap();
    let manifest = project.root.join("semaprax.toml");
    install_host_strict_law_policy(&manifest, &policy, vec!["fresh.seventeen".into()]).unwrap();
    let mut daemon = Command::new(env!("CARGO_BIN_EXE_semapraxd"))
        .args([
            "--stdio",
            "--mcp",
            "--allow-project-law-workflow",
            "--manifest-path",
        ])
        .arg(&manifest)
        .args(["--law-tool", "z3", "--law-executable"])
        .arg(std::env::var("SEMAPRAX_LAW_Z3").unwrap())
        .arg("--law-version-line")
        .arg(std::env::var("SEMAPRAX_LAW_Z3_VERSION").unwrap())
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    let mut input = daemon.stdin.take().unwrap();
    let mut output = std::io::BufReader::new(daemon.stdout.take().unwrap());
    let too_early = call(&mut input, &mut output, 1, "tools/list", json!({}));
    assert_eq!(too_early["error"]["code"], -32000);
    let init = call(
        &mut input,
        &mut output,
        2,
        "initialize",
        json!({
            "protocolVersion":"2025-11-25","capabilities":{},
            "clientInfo":{"name":"law12-gate","version":"1"}
        }),
    );
    assert_eq!(init["result"]["protocolVersion"], "2025-11-25");
    writeln!(
        input,
        "{}",
        json!({"jsonrpc":"2.0","method":"notifications/initialized"})
    )
    .unwrap();
    input.flush().unwrap();
    let catalog = call(&mut input, &mut output, 3, "tools/list", json!({}));
    assert_eq!(catalog["result"]["tools"].as_array().unwrap().len(), 2);
    assert_eq!(catalog["result"]["tools"][0]["name"], "law__status");
    assert_eq!(catalog["result"]["tools"][1]["name"], "law__check");
    let (outer, status) = tool_call(&mut input, &mut output, 4, "law__status", json!({}));
    assert_eq!(outer["result"]["isError"], false);
    let first_revision = status["result"]["candidate_revision"].as_str().unwrap();
    let params = |revision: &str| {
        json!({
            "candidate_revision":revision,"law_id":"fresh.law.seventeen","view":"detail",
            "source":"src/app.spx","declaration":"fresh.seventeen","ensures_index":0
        })
    };
    let (outer, failed) = tool_call(
        &mut input,
        &mut output,
        5,
        "law__check",
        params(first_revision),
    );
    assert_eq!(outer["result"]["isError"], false);
    assert_eq!(
        failed["result"]["proof_attempt"]["outcome"],
        "disproved_concrete"
    );
    assert_eq!(failed["result"]["view"]["accepted"], false);
    assert_eq!(
        failed["result"]["proof_attempt"]["counterexample"]["redacted"],
        true
    );
    let hostile = tool_call(
        &mut input,
        &mut output,
        6,
        "law__check",
        json!({
            "candidate_revision":first_revision,"law_id":"fresh.law.seventeen","view":"detail",
            "executable":"/other/z3"
        }),
    );
    assert_eq!(hostile.0["result"]["isError"], true);
    assert_eq!(hostile.1["error"]["code"], -32602);

    std::fs::write(
        project.root.join("src/app.spx"),
        bad.replace("a + 16", "a + 17"),
    )
    .unwrap();
    let (_, stale) = tool_call(
        &mut input,
        &mut output,
        7,
        "law__check",
        params(first_revision),
    );
    assert_eq!(stale["result"]["proof_attempt"]["outcome"], "stale");
    assert_eq!(stale["result"]["view"]["counts"]["required"], 1);
    let (_, status) = tool_call(&mut input, &mut output, 8, "law__status", json!({}));
    let current_revision = status["result"]["candidate_revision"].as_str().unwrap();
    assert_ne!(current_revision, first_revision);
    let (_, fixed) = tool_call(
        &mut input,
        &mut output,
        9,
        "law__check",
        params(current_revision),
    );
    assert_eq!(fixed["result"]["proof_attempt"]["outcome"], "proved");
    assert_eq!(fixed["result"]["view"]["accepted"], true);
    assert_eq!(fixed["result"]["view"]["counts"]["required"], 1);

    let weaker = semaprax::native_law_source::canonical(
        &semaprax::native_law_source::parse(
            &law_source.replace("evidence smt_proved", "evidence runtime_guarded"),
            "src/contracts.spx",
        )
        .unwrap(),
    );
    std::fs::write(project.root.join("src/contracts.spx"), weaker).unwrap();
    let (outer, refused) = tool_call(&mut input, &mut output, 10, "law__status", json!({}));
    assert_eq!(outer["result"]["isError"], true);
    assert!(refused["error"]["message"]
        .as_str()
        .unwrap()
        .contains("SPX-LW120"));
    drop(input);
    assert!(daemon.wait().unwrap().success());
}
