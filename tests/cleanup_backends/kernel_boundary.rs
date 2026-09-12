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
//! choices, so the count grows combinatorially (exponentially in `count`),
//! not additively -- see `a_ten_branch_nested_classifier_replays_within_budget`
//! below for the additive-growth control. The naive `2^count` estimate is a
//! useful lower bound for reasoning about the shape of the growth, but is
//! not the exact figure `hir::validate` enumerates: the cleanup CFG carries
//! extra per-term bookkeeping blocks beyond the two value branches, so the
//! measured count is larger than `2^count` (confirmed by
//! `fifteen_independent_scalar_comparisons_exceed_the_cleanup_replay_path_budget`
//! below, which pins the exact number this session measured rather than the
//! theoretical one). The exact crossover point below is the smallest fixture
//! this session found that reproduces `SPX-H006` from ordinary source text
//! alone, with no owned resources, no recursion, and no package dependency --
//! i.e. a kernel-sized, scalars-only program.
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
/// independent binary branch, so the terminal path count cleanup replay must
/// enumerate through this single function grows combinatorially with
/// `count` (on the order of `2^count`, though the exact measured count runs
/// higher than that naive estimate -- see the module doc comment above).
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
    validate_source(&source)
        .expect("14 independent branch terms must replay within the 65,536 path budget");
}

#[test]
fn fifteen_independent_scalar_comparisons_exceed_the_cleanup_replay_path_budget() {
    let source = source_with_independent_comparisons(15);
    let error = validate_source(&source)
        .expect_err("15 independent branch terms must exceed the 65,536-path replay budget here");
    assert_eq!(error.code, "SPX-H006");
    // 98,300 is the exact terminal-path count this session measured for this
    // source shape (not the naive `2^15 = 32,768` estimate -- see the module
    // doc comment above for why the two differ). Pinning the literal number
    // here, not just the pass/fail outcome, is deliberate: a change that
    // silently inflates or shrinks the cleanup CFG's per-term bookkeeping
    // must be visible here even if it does not move the admit/refuse
    // boundary itself.
    assert_eq!(
        error.message,
        "cleanup plan for function `app.main` failed independent replay: cleanup replay found 98300 terminal control-flow paths, exceeding the 65536 path budget: path count multiplies combinatorially (2^N) when N branch outcomes are combined independently within one function, not additively with branch count, so splitting into smaller functions only helps if it removes that combination -- restructure the branches to be mutually exclusive (a single dispatch chain, at most one branch executed per call) or combine their results across separate calls instead"
    );
}

/// Regression for the diagnostic-actionability half of issue #241: the raw
/// `SPX-H006` code and a bare "exceeds the budget" message do not tell an
/// author *why* (independent branch combinations multiply path count) or
/// *what to do about it* (splitting into smaller functions only helps if it
/// breaks the combination -- it does not help merely by existing). This
/// checks the message content directly, with substring matching robust to
/// later wording polish, so a future edit cannot silently drop the causal
/// explanation or the remedy while keeping the code and the pass/fail
/// boundary unchanged. Deleting the format!() call this session added to
/// `validate_replay_size_budget` (reverting to the old
/// "... exceeds the global path budget" wording) makes this test fail,
/// which is the point.
#[test]
fn fifteen_independent_scalar_comparisons_diagnostic_names_the_combinatorial_driver_and_remedy() {
    let source = source_with_independent_comparisons(15);
    let error = validate_source(&source).expect_err("15 terms must still exceed the budget");
    assert_eq!(error.code, "SPX-H006");
    assert!(
        error.message.contains("combined independently"),
        "diagnostic must name the actual cost driver (independent branch combination), not just the budget name: {}",
        error.message
    );
    assert!(
        error.message.contains("combinatorially"),
        "diagnostic must say the growth is combinatorial, not merely large: {}",
        error.message
    );
    assert!(
        error.message.contains("mutually exclusive") || error.message.contains("separate calls"),
        "diagnostic must name an actionable remedy, not just the cause: {}",
        error.message
    );
    assert!(
        !error
            .message
            .ends_with("cleanup replay path bound exceeds the global path budget"),
        "diagnostic must not regress to the old cause-free wording: {}",
        error.message
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
