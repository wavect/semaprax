//! Failure injection: proof that the differential checker actually notices.
//!
//! A parity harness that never fails is indistinguishable from one that cannot
//! fail. These tests deliberately produce each kind of wrongness the checker
//! claims to catch and assert the exact class it reports.
//!
//! Two injection depths are used on purpose.
//!
//! *Source-level* injection is end to end: a mutant module is compiled and
//! executed by a real lane, and its report is presented under another lane's
//! name. Generation, execution, envelope decoding, normalization, and
//! comparison all take part, so a bug anywhere in that chain shows up here.
//! It covers a wrong backend value and an incorrect failure selection.
//!
//! *Observation-level* injection covers the two outcomes no source can
//! produce: a lane that dies, and a lane that never ran because its tool is
//! absent. The abort case is additionally proved against a real process that
//! exits non-zero, and against a real native link failure when `clang` is
//! provisioned, so the mapping from a dead process to `Aborted` is exercised
//! rather than assumed.

use std::path::Path;
use std::process::Command;

use super::grammar::Type;
use super::observe::{
    self, Case, Lane, LaneReport, LaneStatus, Observation, CLASS_ADMISSION_DISAGREEMENT,
    CLASS_FAILURE_SELECTION, CLASS_LANE_ABORT, CLASS_VALUE_DISAGREEMENT,
};
use super::{backends, program_of, temporary_root, MAX_STEPS};

/// The baseline fixture. `inject.value` returns 42, `inject.divide` selects the
/// divide-by-zero arithmetic status, and `inject.contract` selects the
/// precondition-failure contract status.
const BASELINE: &str = r#"module test.differential.injection;

@id("inject.helper")
fn helper(p0: i64) -> i64
    requires p0 >= 0
{
    p0 + 1
}

@id("inject.value")
fn value() -> i64
{
    helper(41)
}

@id("inject.divide")
fn divide() -> i64
{
    7 / 0
}

@id("inject.contract")
fn contract() -> i64
{
    helper(-1)
}

@id("app.main")
fn main() -> i64
{
    value()
}
"#;

fn cases() -> Vec<Case> {
    vec![
        ("inject.value".to_owned(), Type::I64),
        ("inject.divide".to_owned(), Type::I64),
        ("inject.contract".to_owned(), Type::I64),
    ]
}

/// Execute one source through the reference lane and relabel the report as if
/// another lane had produced it. This is how a mutant becomes a stand-in for a
/// miscompiling backend.
fn observe_as(source: &str, lane: Lane, root: &Path, name: &str) -> LaneReport {
    let path = root.join(format!("{name}.spx"));
    std::fs::write(&path, source).expect("the fixture is writable");
    let findings = observe::observe_frontend(source, &path);
    assert!(
        findings.findings.is_empty(),
        "the {name} fixture must verify: {:?}",
        findings.findings
    );
    let (report, _) = observe::observe_interpreter(&cases(), &path, MAX_STEPS);
    LaneReport {
        lane,
        status: report.status,
        commands: report.commands,
    }
}

fn baseline(root: &Path) -> LaneReport {
    observe_as(BASELINE, Lane::Interpreter, root, "baseline")
}

fn classes(findings: &[observe::Finding]) -> Vec<&'static str> {
    let mut found = findings
        .iter()
        .map(|finding| finding.class)
        .collect::<Vec<_>>();
    found.sort_unstable();
    found.dedup();
    found
}

#[test]
fn the_baseline_fixture_agrees_with_itself() {
    // The control. If this ever fails, every injection below proves nothing.
    let root = temporary_root("injection-control");
    let reference = baseline(&root);
    let candidate = observe_as(BASELINE, Lane::NativeO0, &root, "control");
    let comparison = observe::compare(&reference, &[candidate]);
    assert!(
        comparison.agrees(),
        "the unmodified fixture disagreed with itself: {:?}",
        comparison.findings
    );
    assert_eq!(comparison.compared, vec![Lane::NativeO0]);
    assert!(comparison.unavailable.is_empty());
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn an_injected_wrong_backend_value_is_detected() {
    let root = temporary_root("injection-value");
    let reference = baseline(&root);
    // The mutant returns 43 where the reference returns 42.
    let mutant = BASELINE.replace("helper(41)", "helper(42)");
    assert_ne!(mutant, BASELINE, "the value mutation must apply");
    let candidate = observe_as(&mutant, Lane::NativeO0, &root, "wrong-value");
    let comparison = observe::compare(&reference, &[candidate]);
    assert_eq!(
        classes(&comparison.findings),
        vec![CLASS_VALUE_DISAGREEMENT],
        "findings: {:?}",
        comparison.findings
    );
    let finding = &comparison.findings[0];
    assert_eq!(finding.case.as_deref(), Some("inject.value"));
    assert_eq!(finding.expected, "returned i64 42");
    assert_eq!(finding.observed, "returned i64 43");
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn an_injected_incorrect_failure_selection_is_detected() {
    let root = temporary_root("injection-selection");
    let reference = baseline(&root);
    // Two independent wrong selections in one mutant: a different arithmetic
    // status for the same operand shape, and a satisfied precondition where the
    // reference selects a contract failure.
    let mutant = BASELINE
        .replace("7 / 0", "7 % 0")
        .replace("helper(-1)", "helper(0)");
    assert_ne!(mutant, BASELINE, "the selection mutation must apply");
    let candidate = observe_as(&mutant, Lane::CoreWasm, &root, "wrong-selection");
    let comparison = observe::compare(&reference, &[candidate]);
    assert_eq!(
        classes(&comparison.findings),
        vec![CLASS_FAILURE_SELECTION],
        "findings: {:?}",
        comparison.findings
    );
    let mut observed = comparison
        .findings
        .iter()
        .map(|finding| {
            (
                finding.case.clone().unwrap_or_default(),
                finding.expected.clone(),
                finding.observed.clone(),
            )
        })
        .collect::<Vec<_>>();
    observed.sort();
    assert_eq!(
        observed,
        vec![
            (
                "inject.contract".to_owned(),
                "failed semaprax.contract.v1#1".to_owned(),
                "returned i64 1".to_owned(),
            ),
            (
                "inject.divide".to_owned(),
                "failed semaprax.arithmetic.v1#4".to_owned(),
                "failed semaprax.arithmetic.v1#6".to_owned(),
            ),
        ]
    );
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn an_injected_lane_abort_is_detected() {
    let root = temporary_root("injection-abort");
    let reference = baseline(&root);
    let aborted = LaneReport {
        lane: Lane::NativeO2,
        status: LaneStatus::Aborted {
            detail: "the observer died with SIGABRT".to_owned(),
        },
        commands: vec!["native-O2 probe".to_owned()],
    };
    let comparison = observe::compare(&reference, &[aborted]);
    assert_eq!(classes(&comparison.findings), vec![CLASS_LANE_ABORT]);
    assert!(
        comparison.compared.is_empty(),
        "an aborted lane is not a compared lane"
    );
    assert!(
        comparison.unavailable.is_empty(),
        "an abort is a finding, not an unavailability"
    );

    // A per-case abort inside a lane that otherwise answered is also caught.
    let mut partial = reference
        .observations()
        .cloned()
        .expect("baseline observed");
    partial.insert(
        "inject.value".to_owned(),
        Observation::Aborted {
            detail: "no line for this case".to_owned(),
        },
    );
    let partial_lane = LaneReport {
        lane: Lane::NativeO0,
        status: LaneStatus::Observed(partial),
        commands: Vec::new(),
    };
    let comparison = observe::compare(&reference, &[partial_lane]);
    assert_eq!(classes(&comparison.findings), vec![CLASS_LANE_ABORT]);
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn a_real_process_that_exits_non_zero_becomes_an_abort() {
    // The mapping from a dead observer process to `Aborted` is exercised, not
    // assumed. `--test-threads=0` is rejected by the test harness itself, so
    // this launches a real process that really exits non-zero.
    let executable = std::env::current_exe().expect("the test binary knows its own path");
    let output = Command::new(executable).arg("--test-threads=0").output();
    let status = backends::transcript_status(output, &cases());
    let lane = LaneReport {
        lane: Lane::NativeO0,
        status,
        commands: Vec::new(),
    };
    let root = temporary_root("injection-process");
    let reference = baseline(&root);
    let comparison = observe::compare(&reference, &[lane]);
    assert_eq!(
        classes(&comparison.findings),
        vec![CLASS_LANE_ABORT],
        "a dead observer must not be parity: {:?}",
        comparison.findings
    );
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn a_native_lane_that_cannot_link_aborts_rather_than_passing() {
    // A real native lane failure when `clang` is provisioned: the probe names a
    // declaration the module does not export, so the link fails. When `clang`
    // is absent the lane must instead say so explicitly.
    let root = temporary_root("injection-link");
    let path = root.join("baseline.spx");
    std::fs::write(&path, BASELINE).expect("the fixture is writable");
    let program = program_of(BASELINE, &path);
    let absent = vec![("inject.absent".to_owned(), Type::I64)];
    let report = backends::observe_native(&absent, &program, &root, "-O0");
    match &report.status {
        LaneStatus::Aborted { detail } => {
            assert!(
                !detail.is_empty(),
                "an aborted lane must carry the reason it died"
            );
        }
        LaneStatus::Unavailable { reason } => {
            assert!(
                backends::tool_identity("clang").is_none(),
                "clang is provisioned, so the lane must not report unavailable: {reason}"
            );
        }
        LaneStatus::Observed(_) => {
            panic!("a probe that cannot link must never produce observations")
        }
    }
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn an_absent_tool_is_an_explicit_outcome_and_never_a_parity_pass() {
    let root = temporary_root("injection-unavailable");
    let reference = baseline(&root);
    let missing =
        LaneReport::unavailable(Lane::CoreWasm, "node is not provisioned on this machine");
    let comparison = observe::compare(&reference, &[missing]);
    // No finding, because nothing disagreed — but also no agreement, because
    // the lane contributed nothing to compare.
    assert!(comparison.findings.is_empty());
    assert!(
        !comparison.compared.contains(&Lane::CoreWasm),
        "an unavailable lane must never enter the compared set"
    );
    assert_eq!(
        comparison.unavailable,
        vec![(
            Lane::CoreWasm,
            "node is not provisioned on this machine".to_owned()
        )],
        "an unavailable lane must be recorded with its exact reason"
    );

    // And a run that requires a lane must fail when that lane did not run.
    let required = [Lane::NativeO0, Lane::NativeO2, Lane::CoreWasm];
    let satisfied = required
        .iter()
        .all(|lane| comparison.compared.contains(lane));
    assert!(
        !satisfied,
        "requiring every lane must not be satisfiable by an unavailable lane"
    );
    let _ = std::fs::remove_dir_all(root);
}

/// The module from issue #75, reduced there: `check` verifies it, the native
/// and web backends execute it, and the interpreter refuses to admit the record
/// construction. Records are outside this tranche's scalar grammar, so this is
/// a demonstration that the checker detects a real, currently open divergence,
/// not a gate on it. It stays `#[ignore]`d so that fixing #75 does not turn a
/// green tree red.
const ISSUE_75: &str = r#"module audit.minrec;

@id("audit.minrec.point")
record Point {
    @id("audit.minrec.point.x")
    x: i64,
    @id("audit.minrec.point.y")
    y: i64,
}

@id("app.main")
fn main() -> i64
{
    let p = Point { x: 40, y: 2, };
    p.x + p.y
}
"#;

#[test]
#[ignore = "demonstration against open issue #75; not a gate"]
fn the_checker_detects_the_open_record_construction_divergence() {
    let root = temporary_root("issue-75");
    let path = root.join("minrec.spx");
    std::fs::write(&path, ISSUE_75).expect("the fixture is writable");
    let frontend = observe::observe_frontend(ISSUE_75, &path);
    assert!(
        frontend.findings.is_empty(),
        "issue #75's module verifies; the frontend lanes must agree: {:?}",
        frontend.findings
    );
    let cases = vec![("app.main".to_owned(), Type::I64)];
    let (reference, _) = observe::observe_interpreter(&cases, &path, MAX_STEPS);
    let program = program_of(ISSUE_75, &path);
    let native = backends::observe_native(&cases, &program, &root, "-O0");
    let wasm = backends::observe_core_wasm(&cases, &program, &root);
    let comparison = observe::compare(&reference, &[native, wasm]);
    println!("interpreter: {:?}", reference.status);
    println!("findings: {:#?}", comparison.findings);
    println!("unavailable: {:?}", comparison.unavailable);
    assert!(
        comparison
            .findings
            .iter()
            .any(|finding| finding.class == CLASS_ADMISSION_DISAGREEMENT),
        "the checker must classify issue #75 as an admission disagreement: {:?}",
        comparison.findings
    );
    let _ = std::fs::remove_dir_all(root);
}
