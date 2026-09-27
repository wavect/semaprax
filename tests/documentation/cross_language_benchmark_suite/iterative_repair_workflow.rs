//! Real-toolchain regression coverage for `iterative-repair-workflow-v1`
//! (issue #211's "Agent workflow" category; issue #298).
//!
//! This task's candidate module carries two interacting, sequentially-
//! masked defects (a withdrawal fee applied unconditionally, then, once
//! fixed, applied in the wrong order relative to clamping) plus a second,
//! independent, already-correct `tier_label` classifier left by a "prior
//! session". These cases prove the committed correct candidate and three
//! independent wrong candidates through the Rust, TypeScript, and SEMAPRAX
//! ports, following `cold_chain_release_gate.rs`'s established pattern for
//! this suite. See
//! `benchmarks/cross-language-v1/tasks/iterative-repair-workflow-v1/EQUIVALENCE.md`
//! for the task's full iterative-repair narrative and hand-verified
//! wrong-candidate divergence this module now exercises automatically
//! instead of by hand.

use super::*;

const TASK: &str = "iterative-repair-workflow-v1";

fn task_source() -> PathBuf {
    root().join(SUITE).join("tasks").join(TASK)
}

fn task_inventory(directory: &Path, id: &str) -> PathBuf {
    let tasks = serde_json::json!({
        "schema": "benchmark.cross_language.tasks.v1",
        "tasks": [{
            "id": id,
            "category": "repair",
            "split": "held_out",
            "summary": "iterative-repair-workflow candidate regression control",
            "equivalence": "task/EQUIVALENCE.md",
            "languages": {
                "rust": {"public": "task/public/rust", "hidden": "task/hidden/rust"},
                "typescript": {"public": "task/public/typescript", "hidden": "task/hidden/typescript"},
                "semaprax-project": {"public": "task/public/semaprax", "hidden": "task/hidden/semaprax"}
            }
        }]
    });
    let path = directory.join("tasks.json");
    write_json(&path, &tasks);
    path
}

fn copy_task(directory: &Path) {
    let source = task_source();
    let task = directory.join("task");
    for language in ["rust", "typescript", "semaprax"] {
        copy_fixture_tree(
            &source.join(format!("public/{language}")),
            &task.join(format!("public/{language}")),
        );
        copy_fixture_tree(
            &source.join(format!("hidden/{language}")),
            &task.join(format!("hidden/{language}")),
        );
    }
    std::fs::copy(source.join("EQUIVALENCE.md"), task.join("EQUIVALENCE.md")).unwrap();
}

fn run_all_ports(directory: &Path, tasks: &Path, id: &str) -> (std::process::Output, PathBuf) {
    let output = directory.join("result.json");
    let result = runner()
        .arg("--root")
        .arg(directory)
        .arg("--tasks")
        .arg(tasks)
        .arg("--adapters")
        .arg(root().join(SUITE).join("adapters.json"))
        .arg("--semaprax")
        .arg(env!("CARGO_BIN_EXE_semaprax"))
        .arg("--only")
        .arg(id)
        .arg("--language")
        .arg("rust")
        .arg("--language")
        .arg("typescript")
        .arg("--language")
        .arg("semaprax-project")
        .arg("--output")
        .arg(&output)
        .output()
        .unwrap();
    (result, output)
}

#[test]
#[cfg_attr(target_os = "windows", ignore = "requires Unix toolchain")]
fn iterative_repair_workflow_v1_correct_candidates_pass_public_and_hidden_in_all_ports() {
    let directory = scratch("iterative-repair-correct");
    copy_task(&directory);
    let tasks = task_inventory(&directory, "iterative-repair-correct");
    let (result, output) = run_all_ports(&directory, &tasks, "iterative-repair-correct");
    assert!(
        result.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&result.stderr)
    );
    let report = document(&output);
    for language in ["rust", "typescript", "semaprax-project"] {
        let record = result_for(&report, &format!("iterative-repair-correct::{language}"));
        assert_eq!(record["status"], "ok", "{record}");
        assert_eq!(record["public"]["passed"], true, "{record}");
        assert_eq!(record["hidden"]["passed"], true, "{record}");
        assert_eq!(record["leak_check"], "ok", "{record}");
    }
}

#[test]
#[cfg_attr(target_os = "windows", ignore = "requires Unix toolchain")]
fn attempt_zero_unconditional_fee_fails_public_in_all_ports() {
    // The narrative's attempt_0: charges the withdrawal fee on every
    // adjustment regardless of sign. This is the surface-level defect the
    // public suite alone already catches -- included as a sanity control
    // proving the public suite's own discriminating power, not a hidden-test
    // proof.
    let directory = scratch("iterative-repair-attempt-zero");
    copy_task(&directory);
    let task = directory.join("task");
    for (path, wrong_candidate) in [
        (
            task.join("public/rust/candidate.rs"),
            r#"fn clamp(value: i64) -> i64 {
    if value < 0 {
        0
    } else if value > 500 {
        500
    } else {
        value
    }
}

fn apply_step(balance: i64, adjustment: i64) -> i64 {
    clamp(balance + adjustment - 3)
}

pub fn process_batch(b0: i64, a1: i64, a2: i64, a3: i64, a4: i64, a5: i64) -> i64 {
    apply_step(
        apply_step(apply_step(apply_step(apply_step(b0, a1), a2), a3), a4),
        a5,
    )
}

pub fn tier_label(balance: i64) -> i64 {
    if balance < 100 {
        0
    } else if balance < 300 {
        1
    } else {
        2
    }
}
"#,
        ),
        (
            task.join("public/typescript/candidate.ts"),
            r#"function clamp(value: number): number {
  if (value < 0) return 0;
  if (value > 500) return 500;
  return value;
}

function applyStep(balance: number, adjustment: number): number {
  return clamp(balance + adjustment - 3);
}

export function processBatch(
  b0: number,
  a1: number,
  a2: number,
  a3: number,
  a4: number,
  a5: number,
): number {
  return applyStep(
    applyStep(applyStep(applyStep(applyStep(b0, a1), a2), a3), a4),
    a5,
  );
}

export function tierLabel(balance: number): number {
  if (balance < 100) return 0;
  if (balance < 300) return 1;
  return 2;
}
"#,
        ),
        (
            task.join("public/semaprax/src/candidate.spx"),
            r#"module bench.ledger.candidate;

@id("bench.ledger.clamp")
fn clamp(value: i64) -> i64
{
    if value < 0 { 0 } else { if value > 500 { 500 } else { value } }
}

@id("bench.ledger.apply_step")
fn apply_step(balance: i64, adjustment: i64) -> i64
{
    clamp(balance + adjustment - 3)
}

@id("bench.ledger.process_batch")
fn process_batch(b0: i64, a1: i64, a2: i64, a3: i64, a4: i64, a5: i64) -> i64
{
    apply_step(apply_step(apply_step(apply_step(apply_step(b0, a1), a2), a3), a4), a5)
}

@id("bench.ledger.tier_label")
fn tier_label(balance: i64) -> i64
{
    if balance < 100 { 0 } else { if balance < 300 { 1 } else { 2 } }
}
"#,
        ),
    ] {
        let candidate = if path.extension().is_some_and(|extension| extension == "spx") {
            semaprax::parse_canonical(wrong_candidate, &path).unwrap().1
        } else {
            wrong_candidate.to_owned()
        };
        std::fs::write(path, candidate).unwrap();
    }

    let tasks = task_inventory(&directory, "iterative-repair-attempt-zero");
    let (result, output) = run_all_ports(&directory, &tasks, "iterative-repair-attempt-zero");
    assert_eq!(result.status.code(), Some(1), "{result:?}");
    let report = document(&output);
    for language in ["rust", "typescript", "semaprax-project"] {
        let record = result_for(
            &report,
            &format!("iterative-repair-attempt-zero::{language}"),
        );
        assert_eq!(record["public"]["passed"], false, "{language}: {record}");
        assert_eq!(record["status"], "failed", "{language}: {record}");
    }
}

#[test]
#[cfg_attr(target_os = "windows", ignore = "requires Unix toolchain")]
fn attempt_one_fee_order_swap_passes_public_but_fails_hidden_in_all_ports() {
    // The narrative's attempt_1: the withdrawal fee is now correctly
    // conditioned on sign, but it is subtracted *after* clamping instead of
    // before. Every public vector avoids letting a withdrawal's fee
    // interact with the floor, so this passes public and only the hidden
    // floor-interaction vectors catch it -- the task-defining negative
    // control for the "read the redacted grader feedback" half of this
    // task's narrative.
    let directory = scratch("iterative-repair-attempt-one");
    copy_task(&directory);
    let task = directory.join("task");
    for (path, wrong_candidate) in [
        (
            task.join("public/rust/candidate.rs"),
            r#"fn clamp(value: i64) -> i64 {
    if value < 0 {
        0
    } else if value > 500 {
        500
    } else {
        value
    }
}

fn fee(adjustment: i64) -> i64 {
    if adjustment < 0 {
        3
    } else {
        0
    }
}

fn apply_step(balance: i64, adjustment: i64) -> i64 {
    clamp(balance + adjustment) - fee(adjustment)
}

pub fn process_batch(b0: i64, a1: i64, a2: i64, a3: i64, a4: i64, a5: i64) -> i64 {
    apply_step(
        apply_step(apply_step(apply_step(apply_step(b0, a1), a2), a3), a4),
        a5,
    )
}

pub fn tier_label(balance: i64) -> i64 {
    if balance < 100 {
        0
    } else if balance < 300 {
        1
    } else {
        2
    }
}
"#,
        ),
        (
            task.join("public/typescript/candidate.ts"),
            r#"function clamp(value: number): number {
  if (value < 0) return 0;
  if (value > 500) return 500;
  return value;
}

function fee(adjustment: number): number {
  return adjustment < 0 ? 3 : 0;
}

function applyStep(balance: number, adjustment: number): number {
  return clamp(balance + adjustment) - fee(adjustment);
}

export function processBatch(
  b0: number,
  a1: number,
  a2: number,
  a3: number,
  a4: number,
  a5: number,
): number {
  return applyStep(
    applyStep(applyStep(applyStep(applyStep(b0, a1), a2), a3), a4),
    a5,
  );
}

export function tierLabel(balance: number): number {
  if (balance < 100) return 0;
  if (balance < 300) return 1;
  return 2;
}
"#,
        ),
        (
            task.join("public/semaprax/src/candidate.spx"),
            r#"module bench.ledger.candidate;

@id("bench.ledger.clamp")
fn clamp(value: i64) -> i64
{
    if value < 0 { 0 } else { if value > 500 { 500 } else { value } }
}

@id("bench.ledger.fee")
fn fee(adjustment: i64) -> i64
{
    if adjustment < 0 { 3 } else { 0 }
}

@id("bench.ledger.apply_step")
fn apply_step(balance: i64, adjustment: i64) -> i64
{
    clamp(balance + adjustment) - fee(adjustment)
}

@id("bench.ledger.process_batch")
fn process_batch(b0: i64, a1: i64, a2: i64, a3: i64, a4: i64, a5: i64) -> i64
{
    apply_step(apply_step(apply_step(apply_step(apply_step(b0, a1), a2), a3), a4), a5)
}

@id("bench.ledger.tier_label")
fn tier_label(balance: i64) -> i64
{
    if balance < 100 { 0 } else { if balance < 300 { 1 } else { 2 } }
}
"#,
        ),
    ] {
        let candidate = if path.extension().is_some_and(|extension| extension == "spx") {
            semaprax::parse_canonical(wrong_candidate, &path).unwrap().1
        } else {
            wrong_candidate.to_owned()
        };
        std::fs::write(path, candidate).unwrap();
    }

    let tasks = task_inventory(&directory, "iterative-repair-attempt-one");
    let (result, output) = run_all_ports(&directory, &tasks, "iterative-repair-attempt-one");
    assert_eq!(result.status.code(), Some(1), "{result:?}");
    let report = document(&output);
    for language in ["rust", "typescript", "semaprax-project"] {
        let record = result_for(&report, &format!("iterative-repair-attempt-one::{language}"));
        assert_eq!(record["public"]["passed"], true, "{language}: {record}");
        assert_eq!(record["hidden"]["passed"], false, "{language}: {record}");
        assert_eq!(record["leak_check"], "ok", "{language}: {record}");
        assert_eq!(record["status"], "failed", "{language}: {record}");
    }
}

#[test]
#[cfg_attr(target_os = "windows", ignore = "requires Unix toolchain")]
fn a_candidate_that_breaks_the_preexisting_tier_classifier_passes_public_but_fails_hidden_in_all_ports(
) {
    // The task-defining sibling-preservation control: `process_batch`'s
    // repair is fully correct (both narrative defects fixed), but the
    // unrelated, already-completed `tier_label` classifier has its branch
    // order swapped, as an over-aggressive "cleanup" edit might do. No
    // public vector calls it, so this passes public; only the hidden
    // vectors that call it directly catch the loss.
    let directory = scratch("iterative-repair-sibling-broken");
    copy_task(&directory);
    let task = directory.join("task");
    for (path, wrong_candidate) in [
        (
            task.join("public/rust/candidate.rs"),
            r#"fn clamp(value: i64) -> i64 {
    if value < 0 {
        0
    } else if value > 500 {
        500
    } else {
        value
    }
}

fn fee(adjustment: i64) -> i64 {
    if adjustment < 0 {
        3
    } else {
        0
    }
}

fn apply_step(balance: i64, adjustment: i64) -> i64 {
    clamp(balance + adjustment - fee(adjustment))
}

pub fn process_batch(b0: i64, a1: i64, a2: i64, a3: i64, a4: i64, a5: i64) -> i64 {
    apply_step(
        apply_step(apply_step(apply_step(apply_step(b0, a1), a2), a3), a4),
        a5,
    )
}

pub fn tier_label(balance: i64) -> i64 {
    if balance < 100 {
        2
    } else if balance < 300 {
        1
    } else {
        0
    }
}
"#,
        ),
        (
            task.join("public/typescript/candidate.ts"),
            r#"function clamp(value: number): number {
  if (value < 0) return 0;
  if (value > 500) return 500;
  return value;
}

function fee(adjustment: number): number {
  return adjustment < 0 ? 3 : 0;
}

function applyStep(balance: number, adjustment: number): number {
  return clamp(balance + adjustment - fee(adjustment));
}

export function processBatch(
  b0: number,
  a1: number,
  a2: number,
  a3: number,
  a4: number,
  a5: number,
): number {
  return applyStep(
    applyStep(applyStep(applyStep(applyStep(b0, a1), a2), a3), a4),
    a5,
  );
}

export function tierLabel(balance: number): number {
  if (balance < 100) return 2;
  if (balance < 300) return 1;
  return 0;
}
"#,
        ),
        (
            task.join("public/semaprax/src/candidate.spx"),
            r#"module bench.ledger.candidate;

@id("bench.ledger.clamp")
fn clamp(value: i64) -> i64
{
    if value < 0 { 0 } else { if value > 500 { 500 } else { value } }
}

@id("bench.ledger.fee")
fn fee(adjustment: i64) -> i64
{
    if adjustment < 0 { 3 } else { 0 }
}

@id("bench.ledger.apply_step")
fn apply_step(balance: i64, adjustment: i64) -> i64
{
    clamp(balance + adjustment - fee(adjustment))
}

@id("bench.ledger.process_batch")
fn process_batch(b0: i64, a1: i64, a2: i64, a3: i64, a4: i64, a5: i64) -> i64
{
    apply_step(apply_step(apply_step(apply_step(apply_step(b0, a1), a2), a3), a4), a5)
}

@id("bench.ledger.tier_label")
fn tier_label(balance: i64) -> i64
{
    if balance < 100 { 2 } else { if balance < 300 { 1 } else { 0 } }
}
"#,
        ),
    ] {
        let candidate = if path.extension().is_some_and(|extension| extension == "spx") {
            semaprax::parse_canonical(wrong_candidate, &path).unwrap().1
        } else {
            wrong_candidate.to_owned()
        };
        std::fs::write(path, candidate).unwrap();
    }

    let tasks = task_inventory(&directory, "iterative-repair-sibling-broken");
    let (result, output) = run_all_ports(&directory, &tasks, "iterative-repair-sibling-broken");
    assert_eq!(result.status.code(), Some(1), "{result:?}");
    let report = document(&output);
    for language in ["rust", "typescript", "semaprax-project"] {
        let record = result_for(
            &report,
            &format!("iterative-repair-sibling-broken::{language}"),
        );
        assert_eq!(record["public"]["passed"], true, "{language}: {record}");
        assert_eq!(record["hidden"]["passed"], false, "{language}: {record}");
        assert_eq!(record["leak_check"], "ok", "{language}: {record}");
        assert_eq!(record["status"], "failed", "{language}: {record}");
    }
}
