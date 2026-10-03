//! LAW-13: execute an exact scalar law fixture through the existing
//! interpreter/Core-Wasm differential lanes. This is sampled translation
//! evidence, never a theorem about lowering.
use super::backends;
use super::observe::{self, Case, LaneReport, LaneStatus, Observation, CLASS_VALUE_DISAGREEMENT};
use super::temporary_root;
use semaprax::parse;
use std::collections::BTreeMap;

const SOURCE: &str = r#"module law.runtime;
@id("app.main") fn main() -> i64 { 0 }
@id("law.safe") fn safe() -> i64 ensures result == 42 { 40 + 2 }
@id("law.overflow") fn overflow() -> i64 { 9223372036854775807 + 1 }
@id("law.minimum") fn minimum() -> i64 { -9223372036854775807 - 1 }
@id("law.divide") fn divide(a: i64, b: i64) -> i64 { a / b }
@id("law.lazy") fn lazy() -> i64 { if false && (divide(1, 0) > 0) { 1 } else { 7 } }
@id("law.requires") fn positive(a: i64) -> i64 requires a > 0 { a }
@id("law.guard") fn guard() -> i64 { positive(0) }
@id("law.post") fn post() -> i64 ensures result > 0 { -1 }
"#;

fn cases() -> Vec<Case> {
    [
        "law.safe",
        "law.overflow",
        "law.minimum",
        "law.lazy",
        "law.guard",
        "law.post",
    ]
    .into_iter()
    .map(|id| (id.to_owned(), super::grammar::Type::I64))
    .collect()
}

fn reference() -> LaneReport {
    let values = BTreeMap::from([
        (
            "law.safe".into(),
            Observation::Returned {
                scalar: "i64",
                value: "42".into(),
            },
        ),
        (
            "law.overflow".into(),
            Observation::Failed {
                domain: "semaprax.arithmetic.v1".into(),
                code: 1,
            },
        ),
        (
            "law.minimum".into(),
            Observation::Returned {
                scalar: "i64",
                value: i64::MIN.to_string(),
            },
        ),
        (
            "law.lazy".into(),
            Observation::Returned {
                scalar: "i64",
                value: "7".into(),
            },
        ),
        (
            "law.guard".into(),
            Observation::Failed {
                domain: "semaprax.contract.v1".into(),
                code: 1,
            },
        ),
        (
            "law.post".into(),
            Observation::Failed {
                domain: "semaprax.contract.v1".into(),
                code: 2,
            },
        ),
    ]);
    LaneReport {
        lane: observe::Lane::Interpreter,
        status: LaneStatus::Observed(values),
        commands: vec!["handwritten exact checked-i64 reference cases".into()],
    }
}

#[test]
fn law_runtime_seeded_wrong_wasm_value_is_translation_disagreement() {
    let root = temporary_root("law-runtime-mutant");
    let mutant = SOURCE.replace("else { 7 }", "else { 8 }");
    assert_ne!(mutant, SOURCE);
    let source_path = root.join("mutant.spx");
    std::fs::write(&source_path, &mutant).unwrap();
    let frontend = observe::observe_frontend(&mutant, &source_path);
    assert!(
        frontend.findings.is_empty(),
        "mutant frontend: {:?}",
        frontend.findings
    );
    let program = parse(&mutant, &source_path).unwrap();
    let wasm = backends::observe_core_wasm(&cases(), &program, &root);
    assert!(
        wasm.observations().is_some(),
        "mutant Wasm was not executed: {wasm:?}"
    );
    let comparison = observe::compare(&reference(), &[wasm]);
    assert!(
        comparison
            .findings
            .iter()
            .any(|finding| finding.class == CLASS_VALUE_DISAGREEMENT
                && finding.case.as_deref() == Some("law.lazy")),
        "seeded runtime discrepancy: {comparison:?}"
    );
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn law_runtime_scalar_reference_interpreter_and_emitted_wasm_agree() {
    let root = temporary_root("law-runtime-chain");
    let source_path = root.join("law-runtime.spx");
    std::fs::write(&source_path, SOURCE).unwrap();
    let frontend = observe::observe_frontend(SOURCE, &source_path);
    assert!(
        frontend.findings.is_empty(),
        "frontend: {:?}",
        frontend.findings
    );
    let program = parse(SOURCE, &source_path).unwrap();
    let cases = cases();
    let (interpreter, _) = observe::observe_interpreter(&cases, &source_path, super::MAX_STEPS);
    let wasm = backends::observe_core_wasm(&cases, &program, &root);
    assert!(
        interpreter.observations().is_some(),
        "interpreter: {interpreter:?}"
    );
    assert!(
        wasm.observations().is_some(),
        "emitted Wasm was not executed: {wasm:?}"
    );
    let comparison = observe::compare(&reference(), &[interpreter, wasm]);
    assert!(
        comparison.agrees(),
        "law runtime disagreement: {comparison:?}"
    );
    let _ = std::fs::remove_dir_all(root);
}
