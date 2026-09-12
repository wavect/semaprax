//! Boundary fixtures for issue #188 (minimal semantic kernel definition) and
//! issue #241 (`SPX-G171`/`SPX-H006` capacity ceilings): exact source shapes
//! that sit on either side of the `SPX-H006` cleanup-replay path budget, built
//! from real source text through the same `parse` -> `hir::resolve` ->
//! `hir::validate` path the compiler itself uses, not a synthesized
//! `CleanupPlan` mutation.
//!
//! `MAX_REPLAY_PATHS` (`src/cleanup_plan/replay.rs`) bounds how many distinct
//! control-flow paths cleanup replay may enumerate for one function. A
//! function whose body is a chain of independent, Copy-scalar `if`/`else`
//! terms combined by `+` produces one CFG path per combination of branch
//! choices: `2^count` terminal paths for `count` such terms. The exact
//! crossover point below is the smallest fixture this session found that
//! reproduces `SPX-H006` from ordinary source text alone, with no owned
//! resources, no recursion, and no package dependency -- i.e. a kernel-sized,
//! scalars-only program.
//!
//! This is a narrower, sharper repro than issue #241's own finding ("roughly
//! ten sequential branches"): a *nested* if/else-if classification chain of
//! ten or more branches (each execution takes exactly one path, so path count
//! grows additively, not combinatorially) does not by itself hit this budget
//! -- see `docs/SEMANTIC-KERNEL-V1.md` for the nested-chain control fixture
//! and the distinction this implies for issue #241's follow-up.

use semaprax::hir;

/// One `main` function summing `count` independent
/// `(if value < i { 1 } else { 0 })` terms. Each term contributes one
/// independent binary branch, so replay must enumerate `2^count` terminal
/// paths through this single function.
fn source_with_independent_comparisons(count: usize) -> String {
    let terms = (0..count)
        .map(|i| format!("(if value < {i} {{ 1 }} else {{ 0 }})"))
        .collect::<Vec<_>>()
        .join(" + ");
    format!(
        "module test.kernel_boundary;\n\n\
         @id(\"app.main\")\n\
         fn main() -> i64\n\
         {{\n\
         \x20   let value = 5;\n\
         \x20   {terms}\n\
         }}\n"
    )
}

/// The cleanup-replay path budget is already enforced while resolving to HIR
/// (`hir::resolve`), not only in the later independent `hir::validate` replay,
/// so this reports whichever stage rejects the source first.
fn validate_source(source: &str) -> Result<(), Box<semaprax::diagnostic::Diagnostic>> {
    let program = semaprax::parse(source, "kernel-boundary.spx").expect("source must parse");
    let resolved = hir::resolve(&program).map_err(|mut diagnostics| {
        Box::new(
            diagnostics
                .pop()
                .expect("resolve error path always carries at least one diagnostic"),
        )
    })?;
    hir::validate(&resolved).map_err(Box::new)
}

#[test]
fn fourteen_independent_scalar_comparisons_replay_within_budget() {
    let source = source_with_independent_comparisons(14);
    validate_source(&source).expect("2^14 = 16384 terminal paths must replay within budget");
}

#[test]
fn fifteen_independent_scalar_comparisons_exceed_the_cleanup_replay_path_budget() {
    let source = source_with_independent_comparisons(15);
    let error = validate_source(&source)
        .expect_err("2^15 = 32768 terminal paths must exceed the replay path budget here");
    assert_eq!(error.code, "SPX-H006");
    assert_eq!(
        error.message,
        "cleanup plan for function `app.main` failed independent replay: cleanup replay path bound exceeds the global path budget"
    );
}

/// Control fixture: a *nested* if/else-if classification chain (the shape
/// issue #241 calls "a per-record status dispatcher") of the same branch
/// count does not exhaust the replay budget by itself, because exactly one
/// branch executes per call -- the terminal path count grows with the branch
/// count, not with its power set. This isolates "branch count" from
/// "independent branch combinations" as the actual `SPX-H006` cost driver.
#[test]
fn a_ten_branch_nested_classifier_replays_within_budget() {
    let mut chain = String::from("10");
    for i in (0..10).rev() {
        chain = format!("if value < {i} {{ {i} }} else {{ {chain} }}");
    }
    let source = format!(
        "module test.kernel_boundary_control;\n\n\
         @id(\"app.main\")\n\
         fn main() -> i64\n\
         {{\n\
         \x20   let value = 5;\n\
         \x20   {chain}\n\
         }}\n"
    );
    validate_source(&source).expect("a linear, mutually-exclusive branch chain stays cheap");
}
