//! Bounded lazy fused map/filter/fold composition (SPX-AI-022, issue #121).
//!
//! These helpers compose the existing consuming iterator protocol (`Iter<T>`,
//! `IterStep<T>`, `iter_next`), the existing generic map/filter/fold profile
//! (`GENERIC-ITERATOR-OPERATIONS-V1.md`), and scalar snapshot closures
//! (`CLOSURES-V2.md`) into single-pass fused pipelines that build no
//! intermediate `Vec`, plus a one-pull `first_if` adapter that advances the
//! source iterator exactly once and lets the caller drop the untouched
//! remainder without ever reaching exhaustion. No new compiler surface is
//! introduced: every admitted shape below is ordinary authored `.spx` source
//! over the already `HOSTED GREEN` profiles it composes, exactly as
//! `map`/`filter`/`fold` themselves are (see
//! `examples/iterator-operations.spx`). A genuinely multi-pull, caller-driven
//! step-wise filter adapter (skipping more than one rejected element inside a
//! single call) is not constructible against the current tree: both a
//! `match` inside a `while` body (`SPX-T252`) and a generic function
//! participating in a recursive call cycle (`SPX-T226`) are refused; see
//! `docs/BOUNDED-LAZY-ITERATOR-ADAPTERS-V1.md`'s "What remains" for the
//! exact diagnostics this session hit designing that shape.
use super::collections;

const EXAMPLE: &str = include_str!("../../../examples/lazy-iterator-adapters.spx");

fn helpers() -> &'static str {
    EXAMPLE.split("@id(\"app.main\")").next().unwrap()
}

#[test]
fn lazy_iterator_adapters_example_executes_on_every_engine() {
    collections::run_source_value(EXAMPLE, 1);
}

/// Empty and exhausted iterators invoke no extra callback: a `keep`/`combine`
/// callback that carries a failing `requires` would fail the whole program if
/// it were ever invoked, so a successful run over zero elements is itself the
/// proof that construction and traversal over an empty/exhausted source make
/// no element callback.
#[test]
fn lazy_iterator_adapters_empty_and_exhausted_invoke_no_callback() {
    let source = format!(
        r#"{}
@id("poison.keep") fn poison_keep(value: i64) -> bool requires false {{true}}
@id("poison.transform") fn poison_transform(value: i64) -> i64 requires false {{value}}
@id("poison.combine") fn poison_combine(accumulator: i64, value: i64) -> i64 requires false {{accumulator}}
@id("app.main") fn main() -> i64 {{
 let empty_filter_fold = filter_fold<i64, i64>(vec_into_iter<i64>(vec_with_capacity<i64>(0usize)), poison_keep, 9, poison_combine);
 let empty_map_fold = map_fold<i64, i64>(vec_into_iter<i64>(vec_with_capacity<i64>(0usize)), poison_transform, 9, poison_combine);
 let empty_map_filter = map_filter<i64, i64>(vec_into_iter<i64>(vec_with_capacity<i64>(0usize)), 0usize, poison_transform, poison_keep);
 let exhausted_first_if = first_if<i64>(vec_into_iter<i64>(vec_with_capacity<i64>(0usize)), poison_keep) == false;
 if empty_filter_fold == 9 && empty_map_fold == 9 && vec_len<i64>(empty_map_filter) == 0usize && exhausted_first_if {{1}} else {{0}}
}}
"#,
        helpers()
    );
    collections::run_source_value(&source, 1);
}

/// Filtered-out owned items are dropped exactly once and accepted items
/// preserve source order, in a single fused pass with no intermediate `Vec`.
#[test]
fn lazy_iterator_adapters_rejected_items_dropped_once_accepted_preserve_order() {
    let source = format!(
        r#"{}
@id("keep.even") fn keep_even(value: i64) -> bool {{value % 2 == 0}}
@id("double") fn double(value: i64) -> i64 {{value * 2}}
@id("sum") fn sum(accumulator: i64, value: i64) -> i64 {{accumulator + value}}
@id("app.main") fn main() -> i64 {{
 let input = vec_push<i64>(vec_push<i64>(vec_push<i64>(vec_push<i64>(vec_with_capacity<i64>(4usize), 1), 2), 3), 4);
 let mapped = map_filter<i64, i64>(vec_into_iter<i64>(input), 4usize, double, keep_even);
 let ordered = vec_len<i64>(mapped) == 4usize && vec_get<i64>(mapped, 0usize) == 2 && vec_get<i64>(mapped, 1usize) == 4 && vec_get<i64>(mapped, 2usize) == 6 && vec_get<i64>(mapped, 3usize) == 8;
 let input2 = vec_push<i64>(vec_push<i64>(vec_push<i64>(vec_push<i64>(vec_with_capacity<i64>(4usize), 1), 2), 3), 4);
 let kept_sum = filter_fold<i64, i64>(vec_into_iter<i64>(input2), keep_even, 0, sum);
 if ordered && kept_sum == 6 {{1}}else{{0}}
}}
"#,
        helpers()
    );
    collections::run_source_value(&source, 1);
}

/// A single `first_if` call pulls at most one element from the source
/// iterator and checks it; the two remaining, still-unconsumed elements
/// (held by the pulled step's own `rest: Iter<T>` remainder) are dropped at
/// the end of the match arm without the call ever reaching exhaustion or
/// pulling a second element. Settlement of that abandoned remainder is
/// checked across every engine's owner-settlement machinery via the shared
/// `collections` harness (interpreter, native C11 O0/O2, and Core Wasm all
/// require zero live/stale owners after the call returns).
#[test]
fn lazy_iterator_adapters_partial_consumption_then_drop_settles_once() {
    let source = format!(
        r#"{}
@id("keep.positive") fn keep_positive(value: i64) -> bool {{value > 0}}
@id("app.main") fn main() -> i64 {{
 let input = vec_push<i64>(vec_push<i64>(vec_push<i64>(vec_with_capacity<i64>(3usize), -1), 2), 3);
 if first_if<i64>(vec_into_iter<i64>(input), keep_positive) == false {{2}} else {{0}}
}}
"#,
        helpers()
    );
    collections::run_source_value(&source, 2);
}

/// A callback failure partway through a fused traversal retains the selected
/// status (sticky failure) regardless of where in the traversal it occurs,
/// and settles every staged owner (the remaining iterator tail and the
/// accumulator) rather than reviving or partially publishing a result.
#[test]
fn lazy_iterator_adapters_callback_failure_is_sticky_and_settles_owners() {
    let source = format!(
        r#"{}
@id("guard") fn guard(value: i64) -> bool requires value < 2 {{true}}
@id("sum") fn sum(accumulator: i64, value: i64) -> i64 {{accumulator + value}}
@id("app.main") fn main() -> i64 {{
 let input = vec_push<i64>(vec_push<i64>(vec_with_capacity<i64>(2usize), 1), 2);
 let folded = filter_fold<i64, i64>(vec_into_iter<i64>(input), guard, 0, sum);
 0
}}
"#,
        helpers()
    );
    collections::run_source(&source, 1);
}

/// The bound is explicit and its exhaustion is reported, not silently
/// truncated: an under-provisioned `map_filter` capacity fails with the
/// existing bounded-Vec capacity diagnostic instead of dropping the
/// overflowing element.
#[test]
fn lazy_iterator_adapters_bounded_capacity_exhaustion_is_reported() {
    let source = format!(
        r#"{}
@id("keep.all") fn keep_all(value: i64) -> bool {{true}}
@id("identity") fn identity(value: i64) -> i64 {{value}}
@id("app.main") fn main() -> i64 {{
 let input = vec_push<i64>(vec_push<i64>(vec_with_capacity<i64>(2usize), 1), 2);
 let output = map_filter<i64, i64>(vec_into_iter<i64>(input), 1usize, identity, keep_all);
 0
}}
"#,
        helpers()
    );
    collections::run_source(&source, 2);
}

/// An owned value transferred into a fused adapter call cannot be reused
/// afterward: the iterator's ownership epoch is consumed at the call's
/// commit boundary exactly like any other owned call, so a use-after-move of
/// the same source binding is a compile-time diagnostic, never a backend
/// accident. This is the achievable analogue of "repeated calls through a
/// one-shot closure are refused": the language admits no owning-capture
/// closure yet (SPX-AI-021/issue #120 is not implemented and is not a
/// prerequisite of this issue), so every closure here is the existing Copy
/// scalar-snapshot kind and is never one-shot; the iterator argument is the
/// one genuinely one-shot owned value this profile transfers, and reuse of
/// it after transfer is what is checked here.
#[test]
fn lazy_iterator_adapters_reuse_of_transferred_iterator_is_refused() {
    let source = format!(
        r#"{}
@id("keep.all") fn keep_all(value: i64) -> bool {{true}}
@id("sum") fn sum(accumulator: i64, value: i64) -> i64 {{accumulator + value}}
@id("app.main") fn main() -> i64 {{
 let input = vec_push<i64>(vec_with_capacity<i64>(1usize), 1);
 let source = vec_into_iter<i64>(input);
 let first = filter_fold<i64, i64>(source, keep_all, 0, sum);
 let second = iter_next<i64>(source);
 0
}}
"#,
        helpers()
    );
    let diagnostics = semaprax::check(&source, "lazy-iterator-adapters-reuse.spx").unwrap_err();
    assert!(
        diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code == "SPX-O101"),
        "{diagnostics:?}"
    );
}
