//! Resealed negative controls target each separately named trust link. The
//! fixture kernel is synthetic; these checks exercise binding refusal only.

use std::path::Path;

use serde_json::Value;

use crate::diagnostic::Diagnostic;
use crate::proof_export::{self, KernelRun, LeanKernel};

use super::render_trust_chain_view;

const SOURCE: &str = "module app.t;\n\
@id(\"app.t.shifted\") fn shifted(a: i64, b: i64) -> i64\n\
requires a >= 0 requires a <= 1000 requires b >= 0 requires b <= 1000\n\
ensures result >= a { let total = a + b; total }\n\
@id(\"app.main\") fn main() -> i64 { 0 }\n";

struct FixtureKernel;

impl LeanKernel for FixtureKernel {
    fn check(&self, lean_source: &str) -> Result<KernelRun, Diagnostic> {
        let mut output = String::from("info: [1/1] Building Export\n");
        for line in lean_source.lines() {
            if let Some(name) = line.strip_prefix("#print axioms ") {
                output.push_str(&format!(
                    "info: Export.lean:1:0: '{name}' depends on axioms: [propext, Classical.choice, Quot.sound]\n"
                ));
            }
        }
        Ok(KernelRun {
            toolchain: proof_export::PINNED_TOOLCHAIN.to_owned(),
            output,
        })
    }
}

fn reseal(payload: &Value) -> String {
    let body = serde_json::to_string(payload).unwrap();
    format!(
        "{{\"schema\":\"{}\",\"digest\":\"{}\",\"bytes\":{},\"payload\":{body}}}",
        proof_export::CERTIFICATE_SCHEMA,
        proof_export::certificate::payload_digest(body.as_bytes()),
        body.len(),
    )
}

#[test]
fn source_statement_compiler_profile_target_and_artifact_ladder_refuses() {
    let root =
        std::env::temp_dir().join(format!("semaprax-law-trust-ladder-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(&root).unwrap();
    let path = root.join("case.spx");
    std::fs::write(&path, SOURCE).unwrap();
    let certificate =
        proof_export::export_obligation_certificate(&path, "app.t.shifted", 0, &FixtureKernel)
            .unwrap();
    let program = crate::parse(SOURCE, Path::new(&path)).unwrap();
    let artifact =
        crate::wasm::emit_resolved_module(&crate::hir::resolve(&program).unwrap()).unwrap();
    render_trust_chain_view(&certificate, &path, &artifact, None).unwrap();

    for (needle, replacement) in [
        ("let total = a + b", "let total = a - b"),
        ("ensures result >= a", "ensures result >= b"),
    ] {
        let changed = SOURCE.replace(needle, replacement);
        assert_ne!(changed, SOURCE);
        std::fs::write(&path, changed).unwrap();
        assert_eq!(
            render_trust_chain_view(&certificate, &path, &artifact, None)
                .unwrap_err()
                .code,
            "SPX-Z112",
        );
    }
    std::fs::write(&path, SOURCE).unwrap();

    let original: Value = serde_json::from_str(&certificate).unwrap();
    let mut statement = original["payload"].clone();
    let lean = statement["lean_source"].as_str().unwrap();
    let weakened = lean.replace("(result ≥ v_a)", "(result ≥ result)");
    assert_ne!(weakened, lean);
    statement["lean_source"] = Value::String(weakened.clone());
    statement["lean_source_sha256"] =
        Value::String(proof_export::certificate::lean_digest(&weakened));
    assert_eq!(
        render_trust_chain_view(&reseal(&statement), &path, &artifact, None)
            .unwrap_err()
            .code,
        "SPX-Z112",
    );

    for (field, value, expected_code) in [
        ("compiler_version", "0.0.0-wrong", "SPX-Z112"),
        ("profile", "wrong-profile", "SPX-Z111"),
    ] {
        let mut payload = original["payload"].clone();
        payload[field] = Value::String(value.to_owned());
        assert_eq!(
            render_trust_chain_view(&reseal(&payload), &path, &artifact, None)
                .unwrap_err()
                .code,
            expected_code,
        );
    }
    let mut target = original["payload"].clone();
    target["artifact"]["target"] = Value::String("native-v1".to_owned());
    assert_eq!(
        render_trust_chain_view(&reseal(&target), &path, &artifact, None)
            .unwrap_err()
            .code,
        "SPX-Z111",
    );
    let mut wrong_artifact = artifact;
    wrong_artifact.push(0);
    assert_eq!(
        render_trust_chain_view(&certificate, &path, &wrong_artifact, None)
            .unwrap_err()
            .code,
        "SPX-Z112",
    );
    let _ = std::fs::remove_dir_all(root);
}
