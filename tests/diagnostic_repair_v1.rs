use std::collections::BTreeSet;
use std::path::PathBuf;
use std::process::Command;
use std::sync::atomic::{AtomicUsize, Ordering};

use semaprax::{codegen, format, graph, parse, repair, wasm};
use sha2::{Digest, Sha256};

static NEXT_FIXTURE: AtomicUsize = AtomicUsize::new(0);

const SOURCE: &str = r#"module repair.phase_a;
fn helper(value:i64)->i64{let adjusted=value+1;if adjusted>1{adjusted}else{1}}
@id("repair.caller") fn caller(value:i64)->i64{helper(value)}
@id("app.main") fn main()->i64{caller(1)}
"#;
const TARGET: &str = "auto:repair.phase_a.helper";
const EXPLICIT_SOURCE: &str = r#"module repair.phase_a;
@id("repair.phase_a.helper") fn helper(value:i64)->i64{let adjusted=value+1;if adjusted>1{adjusted}else{1}}
@id("repair.caller") fn caller(value:i64)->i64{helper(value)}
@id("app.main") fn main()->i64{caller(1)}
"#;

struct Fixture {
    directory: PathBuf,
    source: PathBuf,
}

impl Fixture {
    fn new(label: &str, source: &str) -> Self {
        let sequence = NEXT_FIXTURE.fetch_add(1, Ordering::Relaxed);
        let directory = std::env::temp_dir().join(format!(
            "semaprax-diagnostic-repair-{}-{label}-{sequence}",
            std::process::id()
        ));
        std::fs::create_dir_all(&directory).unwrap();
        let source_path = directory.join("module.spx");
        std::fs::write(&source_path, source).unwrap();
        Self {
            directory,
            source: source_path,
        }
    }

    fn inventory(&self) -> BTreeSet<String> {
        std::fs::read_dir(&self.directory)
            .unwrap()
            .map(|entry| entry.unwrap().file_name().to_string_lossy().into_owned())
            .collect()
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        std::fs::remove_dir_all(&self.directory).unwrap();
    }
}

fn query() -> repair::DiagnosticRepairQuery {
    repair::DiagnosticRepairQuery::assign_function_id(TARGET).unwrap()
}

fn report(fixture: &Fixture) -> String {
    repair::query(&fixture.source, &query()).unwrap()
}

fn independently_derived_repair_id(base_revision: &str, target: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(b"semaprax.diagnostic-repair-id.v1\0");
    for value in [base_revision, "SPX-S103", "function", target] {
        hasher.update((value.len() as u64).to_le_bytes());
        hasher.update(value.as_bytes());
    }
    format!("sha256:{:x}", hasher.finalize())
}

fn domain_digest(domain: &[u8], bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(domain);
    hasher.update((bytes.len() as u64).to_le_bytes());
    hasher.update(bytes);
    format!("sha256:{:x}", hasher.finalize())
}

#[test]
fn targeted_report_is_canonical_exact_and_read_only() {
    let fixture = Fixture::new("report", SOURCE);
    let before_inventory = fixture.inventory();
    let output = report(&fixture);
    let parsed: serde_json::Value = serde_json::from_str(&output).unwrap();
    let revision = graph::revision(&parse(SOURCE, &fixture.source).unwrap());

    assert_eq!(fixture.inventory(), before_inventory);
    assert_eq!(std::fs::read_to_string(&fixture.source).unwrap(), SOURCE);
    assert_eq!(parsed["schema"], "semaprax.diagnostic-repair.v1");
    assert_eq!(parsed["source_graph_schema"], "semaprax.graph.v10");
    assert_eq!(parsed["base_revision"], revision);
    assert_eq!(
        parsed["source"]["digest"],
        domain_digest(
            b"semaprax.diagnostic-repair.source-digest.v1\0",
            SOURCE.as_bytes()
        )
    );
    assert_eq!(parsed["query"]["kind"], "assign_function_id");
    assert_eq!(parsed["query"]["target"], TARGET);
    assert_eq!(parsed["diagnostic"]["code"], "SPX-S103");
    assert_eq!(
        parsed["repair"]["classification"],
        "breaking_identity_rebase"
    );
    assert_eq!(
        parsed["repair"]["input"]["type"],
        "persistent_declaration_id"
    );
    assert_eq!(
        parsed["repair"]["input"]["constraints"]["forbidden_values"],
        serde_json::json!(["bool", "i64"])
    );
    assert_eq!(
        parsed["repair"]["id"],
        independently_derived_repair_id(&revision, TARGET)
    );
    assert_eq!(
        parsed["diagnostic"]["path"],
        fixture.source.display().to_string()
    );
    assert_eq!(parsed["limits"]["max_functions"], 1024);
    assert_eq!(parsed["limits"]["max_output_bytes"], 32 * 1024 * 1024);
    assert_eq!(parsed["budget"]["used_source_bytes"], SOURCE.len());
    assert_eq!(parsed["budget"]["used_functions"], 3);
    assert_eq!(parsed["budget"]["used_call_sites"], 2);
    assert_eq!(parsed["budget"]["used_output_bytes"], output.len());
}

#[test]
fn instantiation_is_exact_read_only_and_runs_the_one_edit_rebase_gate() {
    let fixture = Fixture::new("instantiate", SOURCE);
    let report: serde_json::Value = serde_json::from_str(&report(&fixture)).unwrap();
    let repair_id = report["repair"]["id"].as_str().unwrap();
    let persistent = repair::PersistentDeclarationId::new("repair.phase_a.helper").unwrap();
    let before_inventory = fixture.inventory();
    let preview = repair::instantiate(&fixture.source, repair_id, &persistent).unwrap();
    let parsed: serde_json::Value = serde_json::from_str(&preview).unwrap();
    let expected_patch = format!(
        "schema semaprax.semantic-patch.v3\nbase {}\nassign-function-id repair {repair_id} diagnostic SPX-S103 target {TARGET} name helper to repair.phase_a.helper\n",
        report["base_revision"].as_str().unwrap()
    );

    assert_eq!(parsed["schema"], "semaprax.diagnostic-repair-preview.v1");
    assert_eq!(parsed["budget"]["used_output_bytes"], preview.len());
    assert_eq!(parsed["patch"]["schema"], "semaprax.semantic-patch.v3");
    assert_eq!(parsed["patch"]["source"], expected_patch);
    assert_eq!(
        parsed["patch"]["digest"],
        domain_digest(
            b"semaprax.diagnostic-repair.patch-digest.v1\0",
            expected_patch.as_bytes()
        )
    );
    assert_ne!(parsed["candidate_revision"], parsed["base_revision"]);
    let candidate_text =
        SOURCE.replacen("fn helper", "@id(\"repair.phase_a.helper\")\nfn helper", 1);
    let candidate = parse(&candidate_text, &fixture.source).unwrap();
    let candidate_source = format::canonical(&candidate);
    assert_eq!(
        parsed["candidate_source"]["digest"],
        domain_digest(
            b"semaprax.diagnostic-repair.source-digest.v1\0",
            candidate_source.as_bytes()
        )
    );
    assert_eq!(parsed["identity_rebase"]["before_id"], TARGET);
    assert_eq!(
        parsed["identity_rebase"]["after_id"],
        "repair.phase_a.helper"
    );
    assert_eq!(
        parsed["identity_rebase"]["direct_callers"]
            .as_array()
            .unwrap()
            .len(),
        1
    );
    assert_eq!(
        parsed["identity_rebase"]["direct_callers"][0]["id"],
        "repair.caller"
    );
    assert_eq!(
        parsed["identity_rebase"]["direct_callers"][0]["site_count"],
        1
    );
    assert!(
        parsed["identity_rebase"]["derived_id_count"]
            .as_u64()
            .unwrap()
            > 0
    );
    assert!(parsed["identity_rebase"]["derived_id_digest"]
        .as_str()
        .unwrap()
        .starts_with("sha256:"));
    assert_eq!(fixture.inventory(), before_inventory);
    assert_eq!(std::fs::read_to_string(&fixture.source).unwrap(), SOURCE);
}

#[test]
fn persistent_id_type_is_closed_and_source_collisions_fail() {
    for value in [
        "",
        "_leading",
        "with space",
        "ümlaut",
        "slash/value",
        "auto:repair.helper",
        "auto:repair.helper",
        "core.value",
        "semaprax.value",
        "declaration:value",
        "function-execution:value",
        "parameter:value",
        "nominal:value",
        "bool",
        "i64",
    ] {
        assert_eq!(
            repair::PersistentDeclarationId::new(value)
                .unwrap_err()
                .code,
            "SPX-R102"
        );
    }
    assert_eq!(
        repair::PersistentDeclarationId::new("a".repeat(256))
            .unwrap_err()
            .code,
        "SPX-R102"
    );
    for value in ["a", "A0", "repair.helper", "repair:helper-1_ok"] {
        assert_eq!(
            repair::PersistentDeclarationId::new(value)
                .unwrap()
                .as_str(),
            value
        );
    }

    let fixture = Fixture::new("collision", SOURCE);
    let parsed: serde_json::Value = serde_json::from_str(&report(&fixture)).unwrap();
    let error = repair::instantiate(
        &fixture.source,
        parsed["repair"]["id"].as_str().unwrap(),
        &repair::PersistentDeclarationId::new("repair.caller").unwrap(),
    )
    .unwrap_err();
    assert_eq!(error[0].code, "SPX-R102");
}

#[test]
fn supplied_ids_cannot_confuse_graph_names_or_cleanup_enums() {
    for persistent_id in ["helper", "operation_failure"] {
        let fixture = Fixture::new(persistent_id, SOURCE);
        let report: serde_json::Value = serde_json::from_str(&report(&fixture)).unwrap();
        let preview = repair::instantiate(
            &fixture.source,
            report["repair"]["id"].as_str().unwrap(),
            &repair::PersistentDeclarationId::new(persistent_id).unwrap(),
        )
        .unwrap();
        let preview: serde_json::Value = serde_json::from_str(&preview).unwrap();
        assert_eq!(preview["identity_rebase"]["after_id"], persistent_id);
    }
}

#[test]
fn admitted_scalar_shapes_and_automatic_caller_are_exact() {
    let cases = [
        (
            "zero",
            "module repair.zero;\nfn helper()->i64{1}\n@id(\"app.main\") fn main()->i64{helper()}\n",
            "auto:repair.zero.helper",
            "repair.zero.helper",
        ),
        (
            "two",
            "module repair.two;\nfn helper(left:i64,right:i64)->i64{left+right}\n@id(\"app.main\") fn main()->i64{helper(20,22)}\n",
            "auto:repair.two.helper",
            "repair.two.helper",
        ),
        (
            "bool",
            "module repair.boolean;\nfn helper(value:bool)->bool{value}\n@id(\"app.main\") fn main()->i64{if helper(true){1}else{0}}\n",
            "auto:repair.boolean.helper",
            "repair.boolean.helper",
        ),
    ];
    for (label, source, target, persistent) in cases {
        let fixture = Fixture::new(label, source);
        let request = repair::DiagnosticRepairQuery::assign_function_id(target).unwrap();
        let report = repair::query(&fixture.source, &request).unwrap();
        let report: serde_json::Value = serde_json::from_str(&report).unwrap();
        repair::instantiate(
            &fixture.source,
            report["repair"]["id"].as_str().unwrap(),
            &repair::PersistentDeclarationId::new(persistent).unwrap(),
        )
        .unwrap();
    }

    let source = "module repair.auto_caller;\nfn helper()->i64{1}\nfn caller()->i64{helper()}\n@id(\"app.main\") fn main()->i64{caller()}\n";
    let fixture = Fixture::new("auto-caller", source);
    let request =
        repair::DiagnosticRepairQuery::assign_function_id("auto:repair.auto_caller.helper")
            .unwrap();
    let report = repair::query(&fixture.source, &request).unwrap();
    let report: serde_json::Value = serde_json::from_str(&report).unwrap();
    let preview = repair::instantiate(
        &fixture.source,
        report["repair"]["id"].as_str().unwrap(),
        &repair::PersistentDeclarationId::new("repair.auto_caller.helper").unwrap(),
    )
    .unwrap();
    let preview: serde_json::Value = serde_json::from_str(&preview).unwrap();
    assert_eq!(
        preview["identity_rebase"]["direct_callers"],
        serde_json::json!([{
            "id": "auto:repair.auto_caller.caller",
            "identity_origin": "automatic",
            "site_count": 1
        }])
    );
}

#[test]
fn query_and_instantiation_fail_closed_for_stale_or_wrong_targets() {
    let fixture = Fixture::new("hostile", SOURCE);
    let error = repair::query(
        &fixture.source,
        &repair::DiagnosticRepairQuery::assign_function_id("repair.caller").unwrap(),
    )
    .unwrap_err();
    assert_eq!(error[0].code, "SPX-R101");
    let error = repair::query(
        &fixture.source,
        &repair::DiagnosticRepairQuery::assign_function_id("app.main").unwrap(),
    )
    .unwrap_err();
    assert_eq!(error[0].code, "SPX-R101");
    let error = repair::instantiate(
        &fixture.source,
        "sha256:0000000000000000000000000000000000000000000000000000000000000000",
        &repair::PersistentDeclarationId::new("repair.helper").unwrap(),
    )
    .unwrap_err();
    assert_eq!(error[0].code, "SPX-R101");
}

#[test]
fn excluded_language_domains_and_cycles_are_not_repairable() {
    let cases = [
        (
            "recursive",
            "module repair.recursive;\nfn helper()->i64{helper()}\n@id(\"app.main\") fn main()->i64{0}\n",
            "auto:repair.recursive.helper",
        ),
        (
            "contract",
            "module repair.contract;\nfn helper()->i64 requires true {1}\n@id(\"app.main\") fn main()->i64{helper()}\n",
            "auto:repair.contract.helper",
        ),
        (
            "generic",
            "module repair.generic;\nfn helper<T>()->bool{true}\n@id(\"app.main\") fn main()->i64{if helper<i64>(){1}else{0}}\n",
            "auto:repair.generic.helper",
        ),
        (
            "aggregate",
            "module repair.aggregate;\n@id(\"repair.box\") record Box { @id(\"repair.box.value\") value:i64, }\nfn helper()->i64{1}\n@id(\"app.main\") fn main()->i64{helper()}\n",
            "auto:repair.aggregate.helper",
        ),
    ];
    for (label, source, target) in cases {
        let fixture = Fixture::new(label, source);
        let error = repair::query(
            &fixture.source,
            &repair::DiagnosticRepairQuery::assign_function_id(target).unwrap(),
        )
        .unwrap_err();
        assert_eq!(error[0].code, "SPX-R101", "{label}");
        assert_eq!(std::fs::read_to_string(&fixture.source).unwrap(), source);
    }
}

#[test]
fn targeted_query_is_deterministic_at_high_cardinality() {
    let mut source = String::from("module repair.large;\nfn selected()->i64{1}\n");
    for index in 0..1022 {
        source.push_str(&format!(
            "@id(\"f{index}\") fn f{index}()->i64{{selected()}}\n"
        ));
    }
    source.push_str("@id(\"app.main\") fn main()->i64{selected()}\n");
    let fixture = Fixture::new("large", &source);
    let query =
        repair::DiagnosticRepairQuery::assign_function_id("auto:repair.large.selected").unwrap();
    let first = repair::query(&fixture.source, &query).unwrap();
    let second = repair::query(&fixture.source, &query).unwrap();
    assert_eq!(first, second);
    assert_eq!(
        serde_json::from_str::<serde_json::Value>(&first).unwrap()["query"]["target"],
        "auto:repair.large.selected"
    );
    let parsed: serde_json::Value = serde_json::from_str(&first).unwrap();
    let preview = repair::instantiate(
        &fixture.source,
        parsed["repair"]["id"].as_str().unwrap(),
        &repair::PersistentDeclarationId::new("repair.large.selected").unwrap(),
    )
    .unwrap();
    let preview: serde_json::Value = serde_json::from_str(&preview).unwrap();
    assert_eq!(
        preview["identity_rebase"]["direct_callers"]
            .as_array()
            .unwrap()
            .len(),
        1023
    );
    assert_eq!(preview["budget"]["used_functions"], 1024);
    assert_eq!(preview["budget"]["used_call_sites"], 1023);
    assert_eq!(
        preview["budget"]["used_output_bytes"],
        serde_json::to_string(&preview).unwrap().len()
    );
}

#[test]
fn function_work_bound_rejects_the_next_node() {
    let mut source = String::from("module repair.over;\nfn selected()->i64{missing()}\n");
    for index in 0..1023 {
        source.push_str(&format!("@id(\"f{index}\") fn f{index}()->i64{{0}}\n"));
    }
    source.push_str("@id(\"app.main\") fn main()->i64{selected()}\n");
    let fixture = Fixture::new("over", &source);
    let error = repair::query(
        &fixture.source,
        &repair::DiagnosticRepairQuery::assign_function_id("auto:repair.over.selected").unwrap(),
    )
    .unwrap_err();
    assert_eq!(error[0].code, "SPX-R101");
    assert!(error[0].message.contains("1024 functions"));
}

#[test]
fn structural_call_bound_rejects_before_hir_resolution() {
    let mut source = String::from("module repair.calls;\nfn selected()->i64{\n");
    for index in 0..=65_536 {
        source.push_str(&format!("let v{index}=missing();\n"));
    }
    source.push_str("0\n}\n@id(\"app.main\") fn main()->i64{0}\n");
    let fixture = Fixture::new("calls", &source);
    let error = repair::query(
        &fixture.source,
        &repair::DiagnosticRepairQuery::assign_function_id("auto:repair.calls.selected").unwrap(),
    )
    .unwrap_err();
    assert_eq!(error[0].code, "SPX-R101");
    assert!(error[0].message.contains("65536 call sites"));
}

#[test]
fn checked_in_report_and_preview_are_literal_sha_kats() {
    let path = PathBuf::from("tests/fixtures/diagnostic_repair_v1.spx");
    let query = repair::DiagnosticRepairQuery::assign_function_id(TARGET).unwrap();
    let report = repair::query(&path, &query).unwrap();
    let report_json: serde_json::Value = serde_json::from_str(&report).unwrap();
    let preview = repair::instantiate(
        &path,
        report_json["repair"]["id"].as_str().unwrap(),
        &repair::PersistentDeclarationId::new("repair.phase_a.helper").unwrap(),
    )
    .unwrap();
    assert_eq!(
        format!("{:x}", Sha256::digest(report.as_bytes())),
        "ef689fed2c742dea6cedb0b8ec3d449e5facd8748dd00cb8a8f2e6115be82075"
    );
    assert_eq!(
        format!("{:x}", Sha256::digest(preview.as_bytes())),
        "ae779749b252e5d9661172dfebcd3317211b97310eed57a0a6b7a692be1053e4"
    );
}

#[test]
fn independent_candidate_graph_and_backends_preserve_behavior() {
    let fixture = Fixture::new("backends", SOURCE);
    let base = parse(SOURCE, &fixture.source).unwrap();
    let independently_authored = parse(EXPLICIT_SOURCE, &fixture.source).unwrap();
    let inserted_source =
        SOURCE.replacen("fn helper", "@id(\"repair.phase_a.helper\")\nfn helper", 1);
    let instantiated_candidate = parse(&inserted_source, &fixture.source).unwrap();
    let independent_graph = graph::to_json(&independently_authored).unwrap();

    assert_eq!(
        graph::revision(&instantiated_candidate),
        graph::revision(&independently_authored)
    );
    assert_eq!(
        graph::to_json(&instantiated_candidate).unwrap(),
        independent_graph
    );
    assert_eq!(
        format!("{:x}", Sha256::digest(independent_graph.as_bytes())),
        "d255c0e88ff497436ca0737ffd139cf47c2c142cf1b4f2da071514c0515ad2b3"
    );
    assert_eq!(
        codegen::emit_c(&instantiated_candidate).unwrap(),
        codegen::emit_c(&independently_authored).unwrap()
    );
    assert_eq!(
        wasm::emit_module(&instantiated_candidate).unwrap(),
        wasm::emit_module(&independently_authored).unwrap()
    );

    let report: serde_json::Value = serde_json::from_str(&report(&fixture)).unwrap();
    let preview = repair::instantiate(
        &fixture.source,
        report["repair"]["id"].as_str().unwrap(),
        &repair::PersistentDeclarationId::new("repair.phase_a.helper").unwrap(),
    )
    .unwrap();
    let preview: serde_json::Value = serde_json::from_str(&preview).unwrap();
    assert_eq!(
        preview["candidate_revision"],
        graph::revision(&independently_authored)
    );
    let canonical_candidate = format::canonical(&independently_authored);
    assert_eq!(
        preview["candidate_source"]["digest"],
        domain_digest(
            b"semaprax.diagnostic-repair.source-digest.v1\0",
            canonical_candidate.as_bytes()
        )
    );

    if Command::new("clang").arg("--version").output().is_ok() {
        for optimization in ["-O0", "-O2"] {
            let mut executions = Vec::new();
            for (label, program) in [("base", &base), ("candidate", &independently_authored)] {
                let c_path = fixture
                    .directory
                    .join(format!("{label}-{}.c", &optimization[2..]));
                let executable = fixture.directory.join(format!(
                    "{label}-{}{}",
                    &optimization[2..],
                    std::env::consts::EXE_SUFFIX
                ));
                std::fs::write(&c_path, codegen::emit_c(program).unwrap()).unwrap();
                let compiled = Command::new("clang")
                    .args(["-std=c11", optimization, "-Wall", "-Wextra", "-Werror"])
                    .arg(&c_path)
                    .arg("-o")
                    .arg(&executable)
                    .output()
                    .unwrap();
                assert!(
                    compiled.status.success(),
                    "candidate repair C failed at {optimization}: {}",
                    String::from_utf8_lossy(&compiled.stderr)
                );
                executions.push(Command::new(&executable).output().unwrap());
            }
            assert_eq!(executions[0].status.code(), executions[1].status.code());
            assert_eq!(executions[0].stdout, executions[1].stdout);
            assert_eq!(executions[0].stderr, executions[1].stderr);
            assert!(executions[0].status.success());
            assert_eq!(String::from_utf8_lossy(&executions[0].stdout).trim(), "2");
        }
    }

    if Command::new("node").arg("--version").output().is_ok() {
        let script = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("scripts/verify-web.mjs");
        let mut executions = Vec::new();
        for (label, program) in [
            ("base-web", &base),
            ("candidate-web", &independently_authored),
        ] {
            let output = fixture.directory.join(label);
            wasm::build_web(program, &output).unwrap();
            executions.push(
                Command::new("node")
                    .arg(&script)
                    .arg(&output)
                    .arg("2")
                    .output()
                    .unwrap(),
            );
        }
        assert_eq!(executions[0].status.code(), executions[1].status.code());
        assert_eq!(executions[0].stdout, executions[1].stdout);
        assert_eq!(executions[0].stderr, executions[1].stderr);
        assert!(executions[0].status.success());
        assert_eq!(String::from_utf8_lossy(&executions[0].stdout).trim(), "2");
    }
}

#[test]
fn cli_is_exact_and_rejects_confused_forms_before_semantic_output() {
    let fixture = Fixture::new("cli", SOURCE);
    let binary = env!("CARGO_BIN_EXE_semaprax");
    let query_output = Command::new(binary)
        .args([
            "repairs",
            fixture.source.to_str().unwrap(),
            "assign-function-id",
            TARGET,
        ])
        .output()
        .unwrap();
    assert!(query_output.status.success());
    let query_stdout = String::from_utf8(query_output.stdout).unwrap();
    assert_eq!(query_stdout.trim_end(), report(&fixture));
    let parsed: serde_json::Value = serde_json::from_str(query_stdout.trim_end()).unwrap();
    let repair_id = parsed["repair"]["id"].as_str().unwrap();
    let repair_output = Command::new(binary)
        .args([
            "repair",
            fixture.source.to_str().unwrap(),
            repair_id,
            "--persistent-id",
            "repair.phase_a.helper",
        ])
        .output()
        .unwrap();
    assert!(repair_output.status.success());
    assert_eq!(repair_output.stdout.last(), Some(&b'\n'));

    for args in [
        vec!["repairs"],
        vec!["repairs", "missing.spx", "unknown", TARGET],
        vec!["repair", "missing.spx", repair_id],
        vec![
            "repair",
            fixture.source.to_str().unwrap(),
            repair_id,
            "--unknown",
            "repair.helper",
        ],
    ] {
        let output = Command::new(binary).args(args).output().unwrap();
        assert_eq!(output.status.code(), Some(2));
        assert!(output.stdout.is_empty());
    }
}
