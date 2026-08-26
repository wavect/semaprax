use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicUsize, Ordering};

use semaprax::{codegen, graph, parse, patch, repair, target_evidence, wasm};
use sha2::{Digest, Sha256};

static NEXT: AtomicUsize = AtomicUsize::new(0);

struct Fixture {
    directory: PathBuf,
    source: PathBuf,
    patch: PathBuf,
}

impl Fixture {
    fn rename_v1(label: &str) -> Self {
        let source = "module target.rename_v1;\n@id(\"target.helper\") fn helper()->i64{41}\n@id(\"app.main\") fn main()->i64{helper()+1}\n";
        let patch = format!(
            "base {}\nrename target.helper to answer\nrequire no-new-effects\n",
            graph::revision(&parse(source, "target.spx").unwrap())
        );
        Self::new(label, source, &patch)
    }

    fn rename(label: &str) -> Self {
        let source = "module target.rename;\n@id(\"target.helper\") fn helper()->i64{41}\n@id(\"app.main\") fn main()->i64{helper()+1}\n";
        let patch = format!(
            "schema semaprax.semantic-patch.v2\nbase {}\nrename target.helper to answer\nrequire no-new-effects\n",
            graph::revision(&parse(source, "target.spx").unwrap())
        );
        Self::new(label, source, &patch)
    }

    fn new(label: &str, source: &str, patch: &str) -> Self {
        let directory = std::env::temp_dir().join(format!(
            "semaprax-target-evidence-{}-{label}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir_all(&directory).unwrap();
        let source_path = directory.join("module.spx");
        let patch_path = directory.join("change.spatch");
        std::fs::write(&source_path, source).unwrap();
        std::fs::write(&patch_path, patch).unwrap();
        Self {
            directory,
            source: source_path,
            patch: patch_path,
        }
    }

    fn rebase_v3(label: &str) -> Self {
        let source = "module target.rebase;\nfn helper(value:i64)->i64{value+1}\n@id(\"target.caller\") fn caller(value:i64)->i64{helper(value)}\n@id(\"app.main\") fn main()->i64{caller(41)}\n";
        let fixture = Self::new(label, source, "");
        let query =
            repair::DiagnosticRepairQuery::assign_function_id("auto:target.rebase.helper").unwrap();
        let repairs: serde_json::Value =
            serde_json::from_str(&repair::query(&fixture.source, &query).unwrap()).unwrap();
        let preview: serde_json::Value = serde_json::from_str(
            &repair::instantiate(
                &fixture.source,
                repairs["repair"]["id"].as_str().unwrap(),
                &repair::PersistentDeclarationId::new("target.rebase.helper").unwrap(),
            )
            .unwrap(),
        )
        .unwrap();
        std::fs::write(&fixture.patch, preview["patch"]["source"].as_str().unwrap()).unwrap();
        fixture
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        std::fs::remove_dir_all(&self.directory).unwrap();
    }
}

fn sha256(value: &str) -> String {
    format!(
        "{:x}",
        semaprax::digest_hex::LowerHex(Sha256::digest(value.as_bytes()))
    )
}

fn domain_digest(domain: &[u8], bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(domain);
    hasher.update((bytes.len() as u64).to_le_bytes());
    hasher.update(bytes);
    format!(
        "sha256:{:x}",
        semaprax::digest_hex::LowerHex(hasher.finalize())
    )
}

#[test]
fn report_is_exact_typed_and_read_only() {
    let fixture = Fixture::rename("report");
    let before = std::fs::read(&fixture.source).unwrap();
    let report = target_evidence::preview(&fixture.source, &fixture.patch).unwrap();
    assert_eq!(
        target_evidence::preview(&fixture.source, &fixture.patch).unwrap(),
        report
    );
    assert!(!report.ends_with('\n'));
    assert_eq!(std::fs::read(&fixture.source).unwrap(), before);
    let value: serde_json::Value = serde_json::from_str(&report).unwrap();
    assert_eq!(value["schema"], "semaprax.semantic-target-evidence.v1");
    assert_eq!(value["graphs"]["classification"], "changed");
    assert_eq!(value["capabilities"]["classification"], "unchanged");
    assert_eq!(value["capabilities"]["added"], serde_json::json!([]));
    assert_eq!(value["capabilities"]["removed"], serde_json::json!([]));
    assert_eq!(value["targets"][1]["validation"], "wasmparser_structural");
    assert_eq!(value["targets"][1]["validator_version"], "0.256.0");
    assert_eq!(value["targets"][1]["validator_features"], "all");
    assert!(value["nonclaims"]
        .as_array()
        .unwrap()
        .contains(&serde_json::json!("no_project_test_discovery_or_execution")));
    assert!(value["nonclaims"]
        .as_array()
        .unwrap()
        .contains(&serde_json::json!(
            "not_target_verified_or_runtime_conformant"
        )));
    assert_eq!(value["budget"]["used_output_bytes"], report.len());
    assert_eq!(sha256(&report).len(), 64);
}

#[test]
fn cli_has_exact_arity_and_one_lf() {
    let fixture = Fixture::rename("cli");
    let output = std::process::Command::new(env!("CARGO_BIN_EXE_semaprax"))
        .args(["target-evidence"])
        .arg(&fixture.source)
        .arg(&fixture.patch)
        .output()
        .unwrap();
    assert!(output.status.success());
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(stdout.ends_with('\n'));
    assert!(!stdout[..stdout.len() - 1].contains('\n'));

    let rejected = std::process::Command::new(env!("CARGO_BIN_EXE_semaprax"))
        .arg("target-evidence")
        .output()
        .unwrap();
    assert_eq!(rejected.status.code(), Some(2));
}

#[test]
fn stale_patch_preserves_source() {
    let source = "module target.stale;\n@id(\"app.main\") fn main()->i64{1}\n";
    let fixture = Fixture::new(
        "stale",
        source,
        "base sha256:0000000000000000000000000000000000000000000000000000000000000000\nrename app.main to entry\n",
    );
    let before = std::fs::read(&fixture.source).unwrap();
    assert!(target_evidence::preview(&fixture.source, &fixture.patch).is_err());
    assert_eq!(std::fs::read(&fixture.source).unwrap(), before);
}

#[test]
fn patch_bound_and_many_front_edits_fail_closed_or_build_once() {
    let source = "module target.patch_bound;\n@id(\"app.main\") fn main()->i64{1}\n";
    let fixture = Fixture::new("patch-bound", source, &"x".repeat(4 * 1024 * 1024 + 1));
    let error = target_evidence::preview(&fixture.source, &fixture.patch).unwrap_err();
    assert_eq!(error[0].code, "SPX-G140");

    let mut source = String::from(
        "module target.many_edits;\n@id(\"target.helper\") fn helper()->i64{1}\n@id(\"app.main\") fn main()->i64{\n",
    );
    for index in 0..64 {
        source.push_str(&format!("let value{index}=helper();\n"));
    }
    source.push_str("value63}\n");
    let patch = format!(
        "schema semaprax.semantic-patch.v2\nbase {}\nrename target.helper to renamed\nrequire no-new-effects\n",
        graph::revision(&parse(&source, "many-edits.spx").unwrap())
    );
    let fixture = Fixture::new("many-edits", &source, &patch);
    let report = target_evidence::preview(&fixture.source, &fixture.patch).unwrap();
    assert_eq!(
        serde_json::from_str::<serde_json::Value>(&report).unwrap()["budget"]["used_call_sites"],
        64
    );
}

#[test]
fn source_and_operation_limits_accept_exact_and_reject_one_more() {
    let prefix = "module target.source_bound;\n@id(\"target.helper\") fn helper()->i64{1}\n@id(\"app.main\") fn main()->i64{helper()}\n";
    let mut exact_source = prefix.to_owned();
    exact_source.extend(std::iter::repeat_n(
        ' ',
        16 * 1024 * 1024 - exact_source.len(),
    ));
    let exact_patch = format!(
        "base {}\nrename target.helper to renamed\n",
        graph::revision(&parse(&exact_source, "source-bound.spx").unwrap())
    );
    let fixture = Fixture::new("source-exact", &exact_source, &exact_patch);
    let report: serde_json::Value =
        serde_json::from_str(&target_evidence::preview(&fixture.source, &fixture.patch).unwrap())
            .unwrap();
    assert_eq!(report["budget"]["used_source_bytes"], 16 * 1024 * 1024);

    exact_source.push(' ');
    let fixture = Fixture::new("source-over", &exact_source, &exact_patch);
    assert_eq!(
        target_evidence::preview(&fixture.source, &fixture.patch).unwrap_err()[0].code,
        "SPX-G140"
    );

    let source = "module target.operation_bound;\n@id(\"target.helper\") fn helper()->i64{1}\n@id(\"app.main\") fn main()->i64{helper()}\n";
    let revision = graph::revision(&parse(source, "operation-bound.spx").unwrap());
    let patch = format!(
        "base {revision}\n{}",
        "rename target.helper to renamed\n".repeat(4096)
    );
    let fixture = Fixture::new("operations-exact", source, &patch);
    let report: serde_json::Value =
        serde_json::from_str(&target_evidence::preview(&fixture.source, &fixture.patch).unwrap())
            .unwrap();
    assert_eq!(report["budget"]["used_operations"], 4096);

    let patch = format!(
        "base {revision}\n{}",
        "rename target.helper to renamed\n".repeat(4097)
    );
    let fixture = Fixture::new("operations-over", source, &patch);
    assert_eq!(
        target_evidence::preview(&fixture.source, &fixture.patch).unwrap_err()[0].code,
        "SPX-G140"
    );
}

#[test]
fn exact_candidate_artifacts_are_digest_bound_before_hosted_execution() {
    let fixture = Fixture::rename("hosted-artifacts");
    let report: serde_json::Value =
        serde_json::from_str(&target_evidence::preview(&fixture.source, &fixture.patch).unwrap())
            .unwrap();
    patch::apply(&fixture.source, &fixture.patch).unwrap();
    let candidate_source = std::fs::read_to_string(&fixture.source).unwrap();
    let candidate = parse(&candidate_source, &fixture.source).unwrap();

    let c = codegen::emit_c(&candidate).unwrap();
    assert_eq!(
        domain_digest(
            b"semaprax.semantic-target-evidence.native-c11-source-digest.v1\0",
            c.as_bytes(),
        ),
        report["targets"][0]["candidate_digest"]
    );
    for optimization in ["-O0", "-O2"] {
        let c_path = fixture
            .directory
            .join(format!("candidate-{optimization}.c"));
        let executable = fixture.directory.join(format!(
            "candidate-{optimization}{}",
            std::env::consts::EXE_SUFFIX
        ));
        std::fs::write(&c_path, &c).unwrap();
        let compiled = Command::new("clang")
            .args(["-std=c11", optimization, "-Wall", "-Wextra", "-Werror"])
            .arg(&c_path)
            .arg("-o")
            .arg(&executable)
            .output()
            .unwrap();
        assert!(
            compiled.status.success(),
            "candidate C failed at {optimization}: {}",
            String::from_utf8_lossy(&compiled.stderr)
        );
        let executed = Command::new(&executable).output().unwrap();
        assert!(executed.status.success());
        assert_eq!(String::from_utf8_lossy(&executed.stdout).trim(), "42");
    }

    let wasm_bytes = wasm::emit_module(&candidate).unwrap();
    assert_eq!(
        domain_digest(
            b"semaprax.semantic-target-evidence.wasm-core-module-digest.v1\0",
            &wasm_bytes,
        ),
        report["targets"][1]["candidate_digest"]
    );
    let web = fixture.directory.join("candidate-web");
    wasm::build_web(&candidate, &web).unwrap();
    assert_eq!(std::fs::read(web.join("app.wasm")).unwrap(), wasm_bytes);
    let executed = Command::new("node")
        .arg(Path::new(env!("CARGO_MANIFEST_DIR")).join("scripts/verify-web.mjs"))
        .arg(&web)
        .output()
        .expect("hosted Target Evidence gate requires Node");
    assert!(
        executed.status.success(),
        "candidate Wasm failed: {}",
        String::from_utf8_lossy(&executed.stderr)
    );
    assert_eq!(String::from_utf8_lossy(&executed.stdout).trim(), "42");
}

#[test]
fn absolute_paths_remain_regular_fixture_paths() {
    let fixture = Fixture::rename("paths");
    assert!(Path::new(&fixture.source).is_absolute());
}

#[test]
fn graph_v10_through_v14_target_projections_are_admitted() {
    let cases = [
        (
            "module target.schema_v10;\n@id(\"schema.target\") fn target()->i64{1}\n@id(\"app.main\") fn main()->i64{target()}\n",
            "schema.target",
            "semaprax.graph.v10",
        ),
        (
            "module target.schema_v11;\n@id(\"schema.target\") fn target(input:Option<i64>)->Option<bool>{let checked=input?;Option<bool>::Some { value: checked>0 }}\n@id(\"app.main\") fn main()->i64{0}\n",
            "schema.target",
            "semaprax.graph.v11",
        ),
        (
            include_str!("../platform-tests/component-runtime/v7.spx"),
            "component.transform-i64-bool",
            "semaprax.graph.v12",
        ),
        (
            include_str!("../platform-tests/component-runtime/v8.spx"),
            "component.pattern.preserve-phantom-i64",
            "semaprax.graph.v13",
        ),
        (
            "module target.schema_v14;\n@id(\"schema.target\") fn target<T>()->bool{true}\n@id(\"app.main\") fn main()->i64{if target<i64>(){1}else{0}}\n",
            "schema.target",
            "semaprax.graph.v14",
        ),
    ];
    for (index, (source, target, schema)) in cases.into_iter().enumerate() {
        let patch = format!(
            "base {}\nrename {target} to renamed_{index}\n",
            graph::revision(&parse(source, "schema.spx").unwrap())
        );
        let fixture = Fixture::new(&format!("schema-{index}"), source, &patch);
        let report: serde_json::Value = serde_json::from_str(
            &target_evidence::preview(&fixture.source, &fixture.patch).unwrap(),
        )
        .unwrap();
        assert_eq!(report["source_graph_schema"], schema);
        assert_eq!(report["capabilities"]["classification"], "unchanged");
    }
}

#[test]
fn whole_report_sha_kats_cover_patch_v1_v2_v3() {
    let reports = [
        Fixture::rename_v1("kat-v1"),
        Fixture::rename("kat-v2"),
        Fixture::rebase_v3("kat-v3"),
    ]
    .map(|fixture| target_evidence::preview(&fixture.source, &fixture.patch).unwrap());
    assert_eq!(
        reports.each_ref().map(|report| sha256(report)),
        [
            "85f23fa7922fc6083aa8cf1559dcf81d4657dc64cbb8931b5d08885842e3fb89".to_owned(),
            "17587113aab9af67a8b4c8ad4707db1929bbefdd695dd5ae9b85b31f61c8d5d5".to_owned(),
            "f06c0569a76de27c2b082096730106a728d5910ae3b6ae9389e1bd39f5fe6cda".to_owned(),
        ]
    );
}
