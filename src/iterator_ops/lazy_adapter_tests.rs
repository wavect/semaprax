//! Bounded lazy fused iterator adapters (SPX-AI-022, issue #121): source/HIR
//! evidence that composing the existing consuming iterator protocol into
//! fused map/filter/fold pipelines needs no new cleanup shape, and that the
//! canonical cleanup order these pipelines derive is structural metadata a
//! hostile mutation cannot silently reorder.
//!
//! `docs/BOUNDED-LAZY-ITERATOR-ADAPTERS-V1.md` is the owning specification.
const SOURCE: &str = r#"module test.lazy_iterator_adapters;
@id("lazy.filter-fold")
fn filter_fold<T, A>(input: own Iter<T>, keep: fn(T) -> bool, initial: A, combine: fn(A, T) -> A) -> A {
    let mut accumulator = initial;
    for own item in input {
        if keep(item) { accumulator = combine(accumulator, item); 0 } else { 0 }
    }
    accumulator
}
@id("keep.positive") fn keep_positive(value: i64) -> bool { value > 0 }
@id("sum") fn sum(accumulator: i64, value: i64) -> i64 { accumulator + value }
@id("it.main") fn main() -> i64 {
    let input = vec_push<i64>(vec_push<i64>(vec_with_capacity<i64>(2usize), -1), 2);
    filter_fold<i64, i64>(vec_into_iter<i64>(input), keep_positive, 0, sum)
}"#;

#[test]
fn fused_filter_fold_runs_at_the_existing_iterator_and_cleanup_schema() {
    let program = crate::check(SOURCE, "lazy-iterator-adapters.spx").unwrap();
    let resolved = crate::hir::resolve(&program).unwrap();
    crate::hir::validate(&resolved).unwrap();
    let instance = resolved
        .function_instances
        .iter()
        .find(|instance| instance.template.as_str() == "lazy.filter-fold")
        .unwrap();
    assert_eq!(
        instance.type_arguments,
        vec![crate::hir::ResolvedType::I64, crate::hir::ResolvedType::I64]
    );
    // No new cleanup schema is introduced: the fused function is bound by
    // the same CleanupPlan schema Owning Iterator Loops v1 already uses.
    assert_eq!(
        instance.function.cleanup_plan.schema,
        crate::cleanup_plan::CLEANUP_PLAN_SCHEMA_V11
    );
    let graph = crate::graph::to_json(&program).unwrap();
    crate::graph::verify_json(&program, &graph).unwrap();

    // Determinism (AGENTS.md: "source formatting, graph JSON, Wasm bytes,
    // diagnostics, semantic patches, and contracted generated artifacts are
    // deterministic"): an independent recompile of the identical source
    // produces byte-identical graph JSON, including the cleanup plan it
    // carries. A lazy adapter that reordered or deferred cleanup between
    // otherwise-identical compiles would fail this assertion.
    let program_again = crate::check(SOURCE, "lazy-iterator-adapters-again.spx").unwrap();
    let graph_again = crate::graph::to_json(&program_again).unwrap();
    assert_eq!(graph, graph_again);
}

/// AGENTS.md: "Cleanup inventory order is structural metadata. Cleanup-plan
/// vectors are canonical runtime order and must never be sorted or repaired
/// downstream." The fused function's own `for own item in input` loop
/// lowers to per-block `transitions` that stage the hidden `IterStep<T>`
/// slot and transfer the accumulator's call argument, in the exact order
/// [`CallArgumentTransfer`, `InitializeVariant`, `TransferVariant`] (observed
/// directly from this function's own resolved cleanup plan, not assumed). A
/// hostile reordering of that block's transitions must be rejected by
/// ordinary HIR validation (independent replay), exactly like the existing
/// precedent in
/// `src/cleanup_plan/replay_tests.rs::assert_independent_replay_rejects`
/// (same diagnostic code, reached here through the public `hir::validate`
/// entry point rather than the private `cleanup_plan::replay` internals that
/// module owns).
#[test]
fn fused_filter_fold_rejects_a_hostile_transition_order_reorder() {
    let program = crate::check(SOURCE, "lazy-iterator-adapters-hostile.spx").unwrap();
    let resolved = crate::hir::resolve(&program).unwrap();
    crate::hir::validate(&resolved).unwrap();
    let mut forged = resolved.clone();
    let instance = forged
        .function_instances
        .iter_mut()
        .find(|instance| instance.template.as_str() == "lazy.filter-fold")
        .unwrap();
    let reorderable = instance
        .function
        .cleanup_plan
        .blocks
        .iter_mut()
        .find(|block| block.transitions.len() >= 2);
    let Some(block) = reorderable else {
        // If this fused shape ever settles with at most one transition per
        // block, there is nothing to reorder and this specific hostility is
        // not constructible against it; the determinism check above still
        // pins the canonical order in that case. Recorded rather than
        // silently skipped so a future shape change is visible here.
        panic!(
            "expected at least one block with >= 2 transitions to reorder; \
             cleanup plan shape changed, update this test"
        );
    };
    block.transitions.swap(0, 1);
    let diagnostic = crate::hir::validate(&forged).unwrap_err();
    assert_eq!(diagnostic.code, "SPX-H006");
}

/// The achievable analogue of "repeated calls through a one-shot closure are
/// refused": this profile admits no owning-capture closure (SPX-AI-021,
/// issue #120, is not implemented and is not a prerequisite of this
/// profile), so every closure here is the existing Copy scalar-snapshot kind
/// and is never one-shot. The one genuinely one-shot owned value a fused
/// call transfers is the iterator argument itself; reusing the same source
/// binding after it has been transferred into the call's commit boundary is
/// refused exactly like any other owned call (AGENTS.md: "an owned call
/// stages arguments left to right and transfers them together at its
/// declared commit boundary").
#[test]
fn fused_filter_fold_rejects_reuse_of_the_transferred_iterator() {
    let reuse = SOURCE.replace(
        r#"filter_fold<i64, i64>(vec_into_iter<i64>(input), keep_positive, 0, sum)"#,
        r#"let source = vec_into_iter<i64>(input);
    let first = filter_fold<i64, i64>(source, keep_positive, 0, sum);
    let second = iter_next<i64>(source);
    first"#,
    );
    let diagnostics = crate::check(&reuse, "lazy-iterator-adapters-reuse.spx").unwrap_err();
    assert!(
        diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code == "SPX-O101"),
        "{diagnostics:?}"
    );
}
