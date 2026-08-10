use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicUsize, Ordering};

use semaprax::{codegen, format, graph, impact, parse, patch, repair, wasm};
use sha2::{Digest, Sha256};

static NEXT_FIXTURE: AtomicUsize = AtomicUsize::new(0);

const SOURCE: &str = r#"module patch.phase_b;
fn helper(value:i64)->i64{value+1}
@id("patch.caller") fn caller(value:i64)->i64{helper(value)}
@id("app.main") fn main()->i64{caller(41)}
"#;
const TARGET: &str = "auto:patch.phase_b.helper";
const PERSISTENT: &str = "patch.phase_b.helper";
const EXPLICIT_SOURCE: &str = r#"module patch.phase_b;
@id("patch.phase_b.helper") fn helper(value:i64)->i64{value+1}
@id("patch.caller") fn caller(value:i64)->i64{helper(value)}
@id("app.main") fn main()->i64{caller(41)}
"#;

struct Fixture {
    directory: PathBuf,
    source: PathBuf,
    patch: PathBuf,
}

impl Fixture {
    fn new(label: &str, source: &str) -> Self {
        let sequence = NEXT_FIXTURE.fetch_add(1, Ordering::Relaxed);
        let directory = std::env::temp_dir().join(format!(
            "semaprax-patch-v3-{}-{label}-{sequence}",
            std::process::id()
        ));
        std::fs::create_dir_all(&directory).unwrap();
        let source_path = directory.join("module.spx");
        let patch_path = directory.join("repair.spatch");
        std::fs::write(&source_path, source).unwrap();
        Self {
            directory,
            source: source_path,
            patch: patch_path,
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

fn generated_patch(path: &Path) -> (String, serde_json::Value) {
    let request = repair::DiagnosticRepairQuery::assign_function_id(TARGET).unwrap();
    let report = repair::query(path, &request).unwrap();
    let report: serde_json::Value = serde_json::from_str(&report).unwrap();
    let preview = repair::instantiate(
        path,
        report["repair"]["id"].as_str().unwrap(),
        &repair::PersistentDeclarationId::new(PERSISTENT).unwrap(),
    )
    .unwrap();
    let preview: serde_json::Value = serde_json::from_str(&preview).unwrap();
    (
        preview["patch"]["source"].as_str().unwrap().to_owned(),
        preview,
    )
}

fn repair_id(base_revision: &str, target: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(b"semaprax.diagnostic-repair-id.v1\0");
    for value in [base_revision, "SPX-S103", "function", target] {
        hasher.update((value.len() as u64).to_le_bytes());
        hasher.update(value.as_bytes());
    }
    format!("sha256:{:x}", hasher.finalize())
}

fn assert_no_a0_artifacts(fixture: &Fixture) {
    assert_eq!(
        fixture.inventory(),
        BTreeSet::from(["module.spx".to_owned(), "repair.spatch".to_owned()])
    );
}

#[test]
fn generated_v3_patch_applies_exactly_once_through_a0() {
    let fixture = Fixture::new("apply", SOURCE);
    let (patch_source, preview) = generated_patch(&fixture.source);
    std::fs::write(&fixture.patch, &patch_source).unwrap();
    let revision = patch::apply(&fixture.source, &fixture.patch).unwrap();
    let applied = std::fs::read_to_string(&fixture.source).unwrap();
    let independently_authored = parse(EXPLICIT_SOURCE, &fixture.source).unwrap();

    assert_eq!(applied, format::canonical(&independently_authored));
    assert_eq!(revision, preview["candidate_revision"]);
    assert_eq!(revision, graph::revision(&independently_authored));
    assert_eq!(
        graph::to_json(&parse(&applied, &fixture.source).unwrap()).unwrap(),
        graph::to_json(&independently_authored).unwrap()
    );
    assert_no_a0_artifacts(&fixture);

    let applied_bytes = std::fs::read(&fixture.source).unwrap();
    let error = patch::apply(&fixture.source, &fixture.patch).unwrap_err();
    assert_eq!(error[0].code, "SPX-G409");
    assert_eq!(std::fs::read(&fixture.source).unwrap(), applied_bytes);
    assert_eq!(
        graph::revision(&parse(&applied, &fixture.source).unwrap()),
        revision
    );
    assert_no_a0_artifacts(&fixture);
}

#[test]
fn v3_authenticates_every_selector_and_preserves_source_on_failure() {
    let cases = [
        ("repair", "repair sha256:", "repair sha256:0", "SPX-R101"),
        (
            "diagnostic",
            "diagnostic SPX-S103",
            "diagnostic SPX-S104",
            "SPX-G101",
        ),
        ("target", TARGET, "auto:patch.phase_b.missing", "SPX-R101"),
        ("name", "name helper", "name other", "SPX-R101"),
        (
            "to",
            " to patch.phase_b.helper\n",
            " to auto:forbidden\n",
            "SPX-R102",
        ),
    ];
    for (label, from, to, code) in cases {
        let fixture = Fixture::new(label, SOURCE);
        let (patch_source, _) = generated_patch(&fixture.source);
        let hostile = patch_source.replacen(from, to, 1);
        std::fs::write(&fixture.patch, hostile).unwrap();
        let error = patch::apply(&fixture.source, &fixture.patch).unwrap_err();
        assert_eq!(error[0].code, code, "{label}");
        assert_eq!(std::fs::read_to_string(&fixture.source).unwrap(), SOURCE);
        assert_no_a0_artifacts(&fixture);
    }

    let fixture = Fixture::new("base", SOURCE);
    let (patch_source, _) = generated_patch(&fixture.source);
    let hostile = patch_source.replacen("base sha256:", "base sha256:0", 1);
    std::fs::write(&fixture.patch, hostile).unwrap();
    let error = patch::apply(&fixture.source, &fixture.patch).unwrap_err();
    assert_eq!(error[0].code, "SPX-G409");
    assert_eq!(std::fs::read_to_string(&fixture.source).unwrap(), SOURCE);
    assert_no_a0_artifacts(&fixture);
}

#[test]
fn v3_grammar_is_exactly_three_canonical_lf_lines_and_isolated() {
    let fixture = Fixture::new("grammar", SOURCE);
    let (patch_source, _) = generated_patch(&fixture.source);
    let malformed = [
        patch_source.trim_end().to_owned(),
        patch_source.replace('\n', "\r\n"),
        format!("{patch_source}\n"),
        format!("# comment\n{patch_source}"),
        patch_source.replace("\nbase ", "\nrequire no-new-effects\nbase "),
        patch_source.replace(
            "\nassign-function-id",
            "\nrename patch.caller to renamed\nassign-function-id",
        ),
        patch_source.replace(" repair ", "  repair "),
    ];
    for (index, hostile) in malformed.into_iter().enumerate() {
        std::fs::write(&fixture.patch, hostile).unwrap();
        let error = patch::apply(&fixture.source, &fixture.patch).unwrap_err();
        assert_eq!(error[0].code, "SPX-G101", "case {index}");
        assert_eq!(std::fs::read_to_string(&fixture.source).unwrap(), SOURCE);
        assert_no_a0_artifacts(&fixture);
    }

    let v2_confusion = patch_source.replacen(
        "schema semaprax.semantic-patch.v3",
        "schema semaprax.semantic-patch.v2",
        1,
    );
    let v1_confusion = patch_source
        .strip_prefix("schema semaprax.semantic-patch.v3\n")
        .unwrap()
        .to_owned();
    for hostile in [v2_confusion, v1_confusion] {
        std::fs::write(&fixture.patch, hostile).unwrap();
        let error = patch::apply(&fixture.source, &fixture.patch).unwrap_err();
        assert_eq!(error[0].code, "SPX-G101");
        assert_eq!(std::fs::read_to_string(&fixture.source).unwrap(), SOURCE);
        assert_no_a0_artifacts(&fixture);
    }
}

#[test]
fn v3_cannot_bypass_the_reduced_repair_domain() {
    let source = "module patch.contract;\nfn helper()->i64 requires true {1}\n@id(\"app.main\") fn main()->i64{helper()}\n";
    let target = "auto:patch.contract.helper";
    let fixture = Fixture::new("contract", source);
    let revision = graph::revision(&parse(source, &fixture.source).unwrap());
    let patch_source = format!(
        "schema semaprax.semantic-patch.v3\nbase {revision}\nassign-function-id repair {} diagnostic SPX-S103 target {target} name helper to patch.contract.helper\n",
        repair_id(&revision, target)
    );
    std::fs::write(&fixture.patch, patch_source).unwrap();
    let error = patch::apply(&fixture.source, &fixture.patch).unwrap_err();
    assert_eq!(error[0].code, "SPX-R101");
    assert_eq!(std::fs::read_to_string(&fixture.source).unwrap(), source);
    assert_no_a0_artifacts(&fixture);
}

#[test]
fn v3_function_bound_rejects_before_hir_resolution() {
    let mut source = String::from("module patch.over;\nfn selected()->i64{missing()}\n");
    for index in 0..1023 {
        source.push_str(&format!("@id(\"f{index}\") fn f{index}()->i64{{0}}\n"));
    }
    source.push_str("@id(\"app.main\") fn main()->i64{0}\n");
    let fixture = Fixture::new("function-bound", &source);
    let target = "auto:patch.over.selected";
    let revision = graph::revision(&parse(&source, &fixture.source).unwrap());
    let patch_source = format!(
        "schema semaprax.semantic-patch.v3\nbase {revision}\nassign-function-id repair {} diagnostic SPX-S103 target {target} name selected to patch.over.selected\n",
        repair_id(&revision, target)
    );
    std::fs::write(&fixture.patch, patch_source).unwrap();
    let error = patch::apply(&fixture.source, &fixture.patch).unwrap_err();
    assert_eq!(error[0].code, "SPX-R101");
    assert!(error[0].message.contains("1024 functions"));
    assert_eq!(std::fs::read_to_string(&fixture.source).unwrap(), source);
    assert_no_a0_artifacts(&fixture);
}

#[test]
fn v3_structural_call_bound_rejects_before_hir_resolution() {
    let mut source = String::from("module patch.calls;\nfn selected()->i64{\n");
    for index in 0..=65_536 {
        source.push_str(&format!("let v{index}=missing();\n"));
    }
    source.push_str("0\n}\n@id(\"app.main\") fn main()->i64{0}\n");
    let fixture = Fixture::new("call-bound", &source);
    let target = "auto:patch.calls.selected";
    let revision = graph::revision(&parse(&source, &fixture.source).unwrap());
    let patch_source = format!(
        "schema semaprax.semantic-patch.v3\nbase {revision}\nassign-function-id repair {} diagnostic SPX-S103 target {target} name selected to patch.calls.selected\n",
        repair_id(&revision, target)
    );
    std::fs::write(&fixture.patch, patch_source).unwrap();
    let error = patch::apply(&fixture.source, &fixture.patch).unwrap_err();
    assert_eq!(error[0].code, "SPX-R101");
    assert!(error[0].message.contains("65536 call sites"));
    assert_eq!(std::fs::read_to_string(&fixture.source).unwrap(), source);
    assert_no_a0_artifacts(&fixture);
}

#[test]
fn v3_rejects_collision_ineligible_target_and_impact_v1() {
    let fixture = Fixture::new("domains", SOURCE);
    let (patch_source, _) = generated_patch(&fixture.source);
    for (label, hostile, code) in [
        (
            "collision",
            patch_source.replace(&format!(" to {PERSISTENT}\n"), " to patch.caller\n"),
            "SPX-R102",
        ),
        (
            "main",
            patch_source
                .replace(TARGET, "app.main")
                .replace("name helper", "name main"),
            "SPX-R101",
        ),
        (
            "primitive",
            patch_source.replace(&format!(" to {PERSISTENT}\n"), " to i64\n"),
            "SPX-R102",
        ),
    ] {
        std::fs::write(&fixture.patch, hostile).unwrap();
        let error = patch::apply(&fixture.source, &fixture.patch).unwrap_err();
        assert_eq!(error[0].code, code, "{label}");
        assert_eq!(std::fs::read_to_string(&fixture.source).unwrap(), SOURCE);
        assert_no_a0_artifacts(&fixture);
    }

    std::fs::write(&fixture.patch, patch_source).unwrap();
    let options = impact::SemanticImpactOptions::new(1, 64 * 1024, 256).unwrap();
    let error = impact::preview(&fixture.source, &fixture.patch, &options).unwrap_err();
    assert_eq!(error[0].code, "SPX-G110");
    let unsupported_message = error[0].message.clone();
    let exact_patch = std::fs::read_to_string(&fixture.patch).unwrap();
    for hostile in [
        exact_patch.replacen("base sha256:", "base sha256:0", 1),
        exact_patch.replacen(TARGET, "auto:patch.phase_b.missing", 1),
        exact_patch.replacen(&format!(" to {PERSISTENT}\n"), " to auto:forbidden\n", 1),
    ] {
        std::fs::write(&fixture.patch, hostile).unwrap();
        let error = impact::preview(&fixture.source, &fixture.patch, &options).unwrap_err();
        assert_eq!(error[0].code, "SPX-G110");
        assert_eq!(error[0].message, unsupported_message);
    }
    assert_eq!(std::fs::read_to_string(&fixture.source).unwrap(), SOURCE);
    assert_no_a0_artifacts(&fixture);
}

#[test]
fn applied_v3_candidate_matches_handwritten_artifacts_and_backend_behavior() {
    let fixture = Fixture::new("backends", SOURCE);
    let base = parse(SOURCE, &fixture.source).unwrap();
    let (patch_source, _) = generated_patch(&fixture.source);
    std::fs::write(&fixture.patch, patch_source).unwrap();
    patch::apply(&fixture.source, &fixture.patch).unwrap();
    let applied = parse(
        &std::fs::read_to_string(&fixture.source).unwrap(),
        &fixture.source,
    )
    .unwrap();
    let handwritten = parse(EXPLICIT_SOURCE, &fixture.source).unwrap();

    assert_eq!(
        codegen::emit_c(&applied).unwrap(),
        codegen::emit_c(&handwritten).unwrap()
    );
    assert_eq!(
        wasm::emit_module(&applied).unwrap(),
        wasm::emit_module(&handwritten).unwrap()
    );

    if Command::new("clang").arg("--version").output().is_ok() {
        for optimization in ["-O0", "-O2"] {
            let mut executions = Vec::new();
            for (label, program) in [("base", &base), ("applied", &applied)] {
                let c_path = fixture.directory.join(format!("{label}-{optimization}.c"));
                let executable = fixture.directory.join(format!(
                    "{label}-{optimization}{}",
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
                    "Patch-v3 C failed at {optimization}: {}",
                    String::from_utf8_lossy(&compiled.stderr)
                );
                executions.push(Command::new(&executable).output().unwrap());
                std::fs::remove_file(c_path).unwrap();
                std::fs::remove_file(executable).unwrap();
            }
            assert_eq!(executions[0].status.code(), executions[1].status.code());
            assert_eq!(executions[0].stdout, executions[1].stdout);
            assert_eq!(executions[0].stderr, executions[1].stderr);
            assert!(executions[0].status.success());
            assert_eq!(String::from_utf8_lossy(&executions[0].stdout).trim(), "42");
        }
    }

    if Command::new("node").arg("--version").output().is_ok() {
        let script = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("scripts/verify-web.mjs");
        let mut executions = Vec::new();
        for (label, program) in [("base-web", &base), ("applied-web", &applied)] {
            let output = fixture.directory.join(label);
            wasm::build_web(program, &output).unwrap();
            executions.push(
                Command::new("node")
                    .arg(&script)
                    .arg(&output)
                    .output()
                    .unwrap(),
            );
            std::fs::remove_dir_all(output).unwrap();
        }
        assert_eq!(executions[0].status.code(), executions[1].status.code());
        assert_eq!(executions[0].stdout, executions[1].stdout);
        assert_eq!(executions[0].stderr, executions[1].stderr);
        assert!(executions[0].status.success());
        assert_eq!(String::from_utf8_lossy(&executions[0].stdout).trim(), "42");
    }
    assert_no_a0_artifacts(&fixture);
}

#[test]
fn patch_cli_applies_v3_and_reports_the_bound_candidate_revision() {
    let fixture = Fixture::new("cli", SOURCE);
    let (patch_source, preview) = generated_patch(&fixture.source);
    std::fs::write(&fixture.patch, patch_source).unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_semaprax"))
        .args([
            "patch",
            fixture.source.to_str().unwrap(),
            fixture.patch.to_str().unwrap(),
        ])
        .output()
        .unwrap();
    assert!(output.status.success());
    assert_eq!(
        String::from_utf8(output.stdout).unwrap(),
        format!(
            "applied semantic patch; graph is now {}\n",
            preview["candidate_revision"].as_str().unwrap()
        )
    );
    assert_eq!(
        std::fs::read_to_string(&fixture.source).unwrap(),
        format::canonical(&parse(EXPLICIT_SOURCE, &fixture.source).unwrap())
    );
    assert_no_a0_artifacts(&fixture);
}
