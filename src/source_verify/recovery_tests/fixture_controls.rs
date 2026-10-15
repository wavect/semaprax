//! Original invalid recovery fixtures kept as independently executed negative
//! controls, each beside a corrected admitted counterpart.
//!
//! Original source strings are byte-exact with the fixtures that failed at
//! runtime: a missing final match-arm comma, a local named `result`, a `main`
//! with a non-`i64` result or any parameter, an explicit `own string`
//! parameter and a Boolean `match` without a final catch-all. Invariant diagnostics are authored as
//! complete six-field objects at named literal sites or parsed declaration
//! spans; no expectation is borrowed from an actual diagnostic list and no
//! receiver output is normalized.
use super::refusal_dependency::CALLS;
use super::{codes, receive, source};
use crate::ast::Span;
use crate::diagnostic::Diagnostic;
use crate::test_allocation_observer::measure;
use std::path::Path;

const PATH: &str = "fixture-controls.spx";
const RECEIVER_PATH: &str = "scoped-recovery.spx";
const COMMA_HELP: &str = "every match arm ends with `,`, including the last one before `}`";
const PARAMS_HELP: &str = "`main` takes no parameters; read command-line arguments with `args_len()` and `arg_utf8(index)`";
const RESULT_HELP: &str =
    "`main` returns `i64`, the process exit status: `0` conventionally means success";
const RESULT_NAME_HELP: &str = "`result` names the function's return value only in `ensures`; rename a local binding to `outcome` and update its uses";
const OWNER_HELP: &str = "use `owner: string` to transfer ownership; for a read-only helper, use `owner: borrow str` and pass `string_as_str(text)`";

const M101: &str = "SPX-M101";
const O002: &str = "SPX-O002";
const O101: &str = "SPX-O101";
const O102: &str = "SPX-O102";
const O104: &str = "SPX-O104";
const T103: &str = "SPX-T103";
const T104: &str = "SPX-T104";
const T205: &str = "SPX-T205";
const T232: &str = "SPX-T232";
const T256: &str = "SPX-T256";
const T265: &str = "SPX-T265";

const RESULT_ORIGINAL: &str =
    "let bad: bool = 1; let result = bool_id(bad); let independent = f_string(result); 0";
const OUTCOME: &str =
    "let bad: bool = 1; let outcome = bool_id(bad); let independent = f_string(outcome); 0";
const ACCEPTED_OUTCOME: &str =
    "let flag: bool = true; let outcome = bool_id(flag); let independent = f_bool(outcome); 0";
const BOOL_MATCH: &str = "match true { true => 1, false => 0, }";

const RETURN_VIEW: &str =
    r#"module test.return_view; @id("app.main") fn main() -> bool { let bad: bool = 1; bad }"#;
const RETURN_VIEW_HELPER: &str = r#"module test.return_view; @id("app.probe") fn probe() -> bool { let bad: bool = 1; bad } @id("app.main") fn main() -> i64 { 0 }"#;
const INDEPENDENT_RETURN: &str = r#"module test.independent_return; @id("app.main") fn main() -> string { let bad: bool = 1; bad }"#;
const INDEPENDENT_RETURN_HELPER: &str = r#"module test.independent_return; @id("app.probe") fn probe() -> string { let bad: bool = 1; bad } @id("app.main") fn main() -> i64 { 0 }"#;
const BORROWED_RETURN: &str = r#"module test.borrowed_return; @id("app.main") fn main(held: borrow Bytes) -> Bytes { let alias: string = held; alias }"#;
const BORROWED_RETURN_HELPER: &str = r#"module test.borrowed_return; @id("app.probe") fn probe(held: borrow Bytes) -> Bytes { let alias: string = held; alias } @id("app.main") fn main() -> i64 { 0 }"#;
const LEXICAL_RETURN: &str = r#"module test.lexical_borrowed_return; @id("app.main") fn main(held: borrow Bytes) -> string { let alias: string = held; return alias }"#;
const LEXICAL_RETURN_HELPER: &str = r#"module test.lexical_borrowed_return; @id("app.probe") fn probe(held: borrow Bytes) -> string { let alias: string = held; return alias; } @id("app.main") fn main() -> i64 { 0 }"#;
const VARIANT: &str =
    r#"@id("choice") variant Choice { @id("choice.one") One, @id("choice.two") Two, }"#;

fn owned_view_original() -> String {
    format!(
        r#"module test.owned_view; {CALLS} @id("app.main") fn main(owner: own string) -> i64 {{ let alias: Bytes = owner; let consumed = consume_bytes(alias); let independent = string_len(owner); 0 }}"#
    )
}

fn owned_view_corrected() -> String {
    format!(
        r#"module test.owned_view; {CALLS} @id("app.probe") fn probe(owner: string) -> i64 {{ let alias: Bytes = owner; let consumed = consume_bytes(alias); let independent = string_len(owner); 0 }} @id("app.main") fn main() -> i64 {{ 0 }}"#
    )
}

fn borrowed_view_original() -> String {
    format!(
        r#"module test.borrowed_view; {CALLS} @id("app.main") fn main(held: borrow Bytes) -> i64 {{ let alias: string = held; let consumed = f_string(alias); 0 }}"#
    )
}

fn borrowed_view_corrected() -> String {
    format!(
        r#"module test.borrowed_view; {CALLS} @id("app.probe") fn probe(held: borrow Bytes) -> i64 {{ let alias: string = held; let consumed = f_string(alias); 0 }} @id("app.main") fn main() -> i64 {{ 0 }}"#
    )
}

fn loan_original() -> String {
    format!(
        r#"module test.loan_view; {CALLS} @id("app.main") fn main(owner: own Bytes) -> i64 {{ let view: bool = bytes_as_slice(owner); let consumed = consume_bytes(owner); let still_live = byte_len(view); 0 }}"#
    )
}

fn loan_corrected() -> String {
    format!(
        r#"module test.loan_view; {CALLS} @id("app.probe") fn probe(owner: own Bytes) -> i64 {{ let view: bool = bytes_as_slice(owner); let consumed = consume_bytes(owner); let still_live = byte_len(view); 0 }} @id("app.main") fn main() -> i64 {{ 0 }}"#
    )
}

fn contract_original() -> String {
    format!(
        r#"module test.contract_borrow; {CALLS} @id("app.main") fn main(held: borrow Bytes) -> i64 requires {{ let alias: string = held; let used = f_string(alias); true }} {{ 0 }}"#
    )
}

fn contract_corrected() -> String {
    format!(
        r#"module test.contract_borrow; {CALLS} @id("app.probe") fn probe(held: borrow Bytes) -> i64 requires {{ let alias: string = held; let used = f_string(alias); true }} {{ 0 }} @id("app.main") fn main() -> i64 {{ 0 }}"#
    )
}

fn guard_original(initializer: &str, arms: &str) -> String {
    format!(
        r#"module test.guard_view; {VARIANT} @id("app.main") fn main(selected: Choice) -> i64 {{ let bad: bool = {initializer}; match selected {{ {arms} }} }}"#
    )
}

fn guard_corrected(initializer: &str, arms: &str) -> String {
    format!(
        r#"module test.guard_view; {VARIANT} @id("app.probe") fn probe(selected: Choice) -> i64 {{ let bad: bool = {initializer}; match selected {{ {arms} }} }} @id("app.main") fn main() -> i64 {{ 0 }}"#
    )
}

fn module(body: &str) -> String {
    format!(
        r#"module test.refusal_dependency; {CALLS} @id("app.main") fn main() -> i64 {{ {body} }}"#
    )
}

/// A span derived from one unique named literal site in `source`.
fn span_at(source: &str, literal: &str, offset: usize, text: &str) -> Span {
    assert_eq!(
        source.matches(literal).count(),
        1,
        "named literal site {literal} must be unique in {source}"
    );
    let start = source.find(literal).unwrap() + offset;
    let end = start + text.len();
    assert_eq!(&source[start..end], text, "named literal site text");
    let newline = char::from(10u8);
    let line_start = source[..start].rfind(newline).map_or(0, |index| index + 1);
    let line = 1 + source[..start].matches(newline).count();
    Span {
        start,
        end,
        line,
        column: start - line_start + 1,
    }
}

fn authored(code: &'static str, message: &str, span: Span, help: Option<&str>) -> Diagnostic {
    let value = Diagnostic::error(code, message, span).at_path(PATH);
    match help {
        Some(help) => value.with_help(help),
        None => value,
    }
}

fn full(source: &str, old: bool) -> Vec<Diagnostic> {
    let program = crate::parse(source, Path::new(PATH)).unwrap();
    if old {
        crate::source_verify::declaration::old::verify(&program)
    } else {
        crate::source_verify::verify(&program)
    }
}

/// The span of the parsed `main` declaration (original AST site).
fn main_span(source: &str) -> Span {
    let program = crate::parse(source, Path::new(PATH)).unwrap();
    program
        .functions
        .iter()
        .find(|function| function.name == "main")
        .unwrap()
        .span
}

fn entry_signature(source: &str, help: &str) -> Diagnostic {
    authored(
        T104,
        "entry function must be monomorphic with signature `fn main() -> i64`",
        main_span(source),
        Some(help),
    )
}

fn assert_roundtrip(source: &str) {
    let original = crate::parse(source, Path::new(PATH)).unwrap();
    let canonical = crate::format::canonical(&original);
    let roundtrip = crate::parse(&canonical, Path::new(PATH)).unwrap();
    assert_eq!(crate::format::canonical(&roundtrip), canonical);
    assert_eq!(
        crate::graph::revision(&original),
        crate::graph::revision(&roundtrip)
    );
    assert!(crate::source_verify::verify(&roundtrip).is_empty());
}

/// Exact per-case code lists for both complete receivers, the original root,
/// ordered exact object subsequence, and each independently authored invariant
/// diagnostic exactly once in both sequences.
fn assert_case(
    source: &str,
    old_codes: &[&str],
    current_codes: &[&str],
    invariants: &[Diagnostic],
) {
    let old = full(source, true);
    let current = full(source, false);
    assert_eq!(codes(&old), old_codes, "old: {source}\n{old:?}");
    assert_eq!(
        codes(&current),
        current_codes,
        "current: {source}\n{current:?}"
    );
    assert_eq!(current.first(), old.first(), "original root: {source}");
    let mut remaining = old.iter();
    for diagnostic in &current {
        assert!(
            remaining.any(|original| original == diagnostic),
            "lost exact original diagnostic provenance: {source}: {diagnostic:?}"
        );
    }
    for invariant in invariants {
        for (label, found) in [("old", &old), ("current", &current)] {
            assert_eq!(
                found
                    .iter()
                    .filter(|value| value.code == invariant.code)
                    .count(),
                1,
                "{label} {}: {source}",
                invariant.code
            );
            assert!(
                found.contains(invariant),
                "{label} lost invariant {invariant:?}: {source}\n{found:?}"
            );
        }
    }
}

struct Pair<'a> {
    original: &'a str,
    help: &'a str,
    extra: Vec<Diagnostic>,
    old: &'a [&'a str],
    current: &'a [&'a str],
    corrected: &'a str,
    corrected_old: &'a [&'a str],
    corrected_current: &'a [&'a str],
}

/// The original keeps `SPX-T104` (plus any extra invariant) as a negative; the
/// helper counterpart carries the same dependency meaning behind a valid main.
fn assert_pair(pair: Pair<'_>) {
    let mut invariants = vec![entry_signature(pair.original, pair.help)];
    invariants.extend(pair.extra);
    assert_case(pair.original, pair.old, pair.current, &invariants);
    assert_case(
        pair.corrected,
        pair.corrected_old,
        pair.corrected_current,
        &[],
    );
    for found in [full(pair.corrected, true), full(pair.corrected, false)].concat() {
        assert!(
            found.code != T104 && found.code != O002,
            "corrected counterpart kept an entry/ownership invariant: {found:?}"
        );
    }
}

#[test]
fn original_missing_arm_comma_bodies_stay_independent_parser_refusals() {
    for body in [
        "match 1 { 0 => missing, n => n }",
        "match 1 { 0 => 0, n => n }",
    ] {
        let text = source(body);
        let expected = authored(
            "SPX-P106",
            "expected `,` after match arm",
            span_at(&text, "n => n }", 7, "}"),
            Some(COMMA_HELP),
        );
        let refused = crate::parse(&text, Path::new(PATH))
            .err()
            .unwrap_or_else(|| panic!("original body must be refused by the parser"));
        assert_eq!(refused, expected, "{text}");
        assert_eq!(refused.json(), expected.json(), "{text}");
    }
}

#[test]
fn corrected_arm_comma_bodies_keep_original_full_receiver_and_declaration_parity() {
    let missing = source("match 1 { 0 => missing, n => n, }");
    let site = span_at(&missing, "0 => missing,", 5, "missing");
    let unknown = |path: &str| {
        Diagnostic::error("SPX-T202", "unknown value `missing` in `main`", site).at_path(path)
    };
    let old = receive(&missing, true);
    assert_eq!(receive(&missing, false), old);
    assert_eq!(old.1.first(), Some(&unknown(RECEIVER_PATH)));
    let declared = full(&missing, true);
    assert_eq!(full(&missing, false), declared);
    assert!(declared.contains(&unknown(PATH)), "{declared:?}");

    let admitted = source("match 1 { 0 => 0, n => n, }");
    assert_eq!(receive(&admitted, false), receive(&admitted, true));
    assert!(receive(&admitted, false).1.is_empty());
    assert!(full(&admitted, true).is_empty());
    assert_eq!(full(&admitted, false), full(&admitted, true));
    assert_roundtrip(&admitted);
}

#[test]
fn original_reserved_local_result_keeps_full_old_and_current_sequences() {
    let text = module(RESULT_ORIGINAL);
    let root = authored(
        T232,
        "declared binding type `bool` does not accept value type `i64`",
        span_at(&text, "let bad: bool = 1;", 16, "1"),
        None,
    );
    let reserved = authored(
        "SPX-S109",
        "`result` is reserved and cannot name a local binding",
        span_at(&text, "let result = ", 4, "result"),
        Some(RESULT_NAME_HELP),
    );
    let argument = authored(
        T205,
        "argument `value` to `bool_id` expects bool, received i64",
        span_at(&text, "bool_id(bad)", 8, "bad"),
        None,
    );
    let postcondition_only = authored(
        "SPX-T201",
        "`result` is only available in postconditions",
        span_at(&text, "f_string(result)", 9, "result"),
        Some(RESULT_NAME_HELP),
    );
    assert_eq!(
        full(&text, true),
        [
            root.clone(),
            reserved.clone(),
            argument,
            postcondition_only.clone()
        ]
    );
    assert_eq!(full(&text, false), [root, reserved, postcondition_only]);
}

#[test]
fn valid_outcome_name_keeps_fixed_result_reset_with_full_sequences() {
    let text = module(OUTCOME);
    let root = authored(
        T232,
        "declared binding type `bool` does not accept value type `i64`",
        span_at(&text, "let bad: bool = 1;", 16, "1"),
        None,
    );
    let argument = authored(
        T205,
        "argument `value` to `bool_id` expects bool, received i64",
        span_at(&text, "bool_id(bad)", 8, "bad"),
        None,
    );
    let independent = authored(
        T205,
        "argument `value` to `f_string` expects string, received bool",
        span_at(&text, "f_string(outcome)", 9, "outcome"),
        None,
    );
    assert_eq!(
        full(&text, true),
        [root.clone(), argument, independent.clone()]
    );
    assert_eq!(full(&text, false), [root, independent]);
}

#[test]
fn accepted_outcome_name_keeps_receiver_canonical_and_graph_results() {
    let text = module(ACCEPTED_OUTCOME);
    assert!(full(&text, true).is_empty());
    assert_eq!(full(&text, false), full(&text, true));
    assert_eq!(receive(&text, false), receive(&text, true));
    assert_roundtrip(&text);
}

#[test]
fn original_non_i64_main_results_keep_exact_t104_and_helper_counterparts() {
    assert_pair(Pair {
        original: RETURN_VIEW,
        help: RESULT_HELP,
        extra: Vec::new(),
        old: &[T232, T103, T104],
        current: &[T232, T104],
        corrected: RETURN_VIEW_HELPER,
        corrected_old: &[T232, T103],
        corrected_current: &[T232],
    });
    assert_pair(Pair {
        original: INDEPENDENT_RETURN,
        help: RESULT_HELP,
        extra: Vec::new(),
        old: &[T232, T103, O104, T104],
        current: &[T232, T103, T104],
        corrected: INDEPENDENT_RETURN_HELPER,
        corrected_old: &[T232, T103, O104],
        corrected_current: &[T232, T103],
    });
}

#[test]
fn original_parameterized_mains_keep_exact_t104_o002_and_helper_counterparts() {
    let owned = owned_view_original();
    let owner = authored(
        O002,
        "string parameters own their input implicitly; an explicit ownership mode is not admitted",
        span_at(&owned, "owner: own string", 0, "owner"),
        Some(OWNER_HELP),
    );
    assert_pair(Pair {
        original: &owned,
        help: PARAMS_HELP,
        extra: vec![owner],
        old: &[O002, T232, T205, O101, T104],
        current: &[O002, T232, O101, T104],
        corrected: &owned_view_corrected(),
        corrected_old: &[T232, T205, O101],
        corrected_current: &[T232, O101],
    });
    assert_pair(Pair {
        original: &borrowed_view_original(),
        help: PARAMS_HELP,
        extra: Vec::new(),
        old: &[T232, T205, O102, T104],
        current: &[T232, O102, T104],
        corrected: &borrowed_view_corrected(),
        corrected_old: &[T232, T205, O102],
        corrected_current: &[T232, O102],
    });
    assert_pair(Pair {
        original: BORROWED_RETURN,
        help: PARAMS_HELP,
        extra: Vec::new(),
        old: &[T232, O104, T104],
        current: &[T232, O104, T104],
        corrected: BORROWED_RETURN_HELPER,
        corrected_old: &[T232, O104],
        corrected_current: &[T232, O104],
    });
    let loan = loan_original();
    let helper = loan_corrected();
    assert_pair(Pair {
        original: &loan,
        help: PARAMS_HELP,
        extra: Vec::new(),
        old: &[T232, T265, T104],
        current: &[T232, T265, T104],
        corrected: &helper,
        corrected_old: &[T232, T265],
        corrected_current: &[T232, T265],
    });
    assert_eq!(full(&loan, false), full(&loan, true));
    assert_eq!(full(&helper, false), full(&helper, true));
    assert_pair(Pair {
        original: &contract_original(),
        help: PARAMS_HELP,
        extra: Vec::new(),
        old: &[T232, T205, O102, T104],
        current: &[T232, O102, T104],
        corrected: &contract_corrected(),
        corrected_old: &[T232, T205, O102],
        corrected_current: &[T232, O102],
    });
}

#[test]
fn original_lexical_return_fixture_pins_its_parse_boundary_and_keeps_helper_control() {
    // ACYCLIC-GUARD-RETURNS-V1 requires a terminating semicolon. The
    // original source is executed as a complete parser-refusal control.
    let expected = authored(
        "SPX-P106",
        "expected `;` after return value",
        span_at(LEXICAL_RETURN, "return alias }", 13, "}"),
        None,
    );
    let original = crate::parse(LEXICAL_RETURN, Path::new(PATH)).unwrap_err();
    assert_eq!(original, expected);
    assert_eq!(original.json(), expected.json());
    // Acyclic `return alias;` normalizes to the original body tail. The real
    // Bytes carrier differs from string, so the current declaration receiver
    // neither repeats the explained type error nor fabricates owned string.
    assert_case(LEXICAL_RETURN_HELPER, &[T232, T103, O104], &[T232], &[]);
}

#[test]
fn original_guard_mains_keep_exact_t104_and_helper_counterparts() {
    let complete = "Choice::One {} if bad => 1, Choice::One {} => 0, Choice::Two {} => 0,";
    let partial = "Choice::One {} if bad => 1, Choice::One {} => 0,";
    assert_pair(Pair {
        original: &guard_original("1", complete),
        help: PARAMS_HELP,
        extra: Vec::new(),
        old: &[T232, T256, T104],
        current: &[T232, T104],
        corrected: &guard_corrected("1", complete),
        corrected_old: &[T232, T256],
        corrected_current: &[T232],
    });
    assert_pair(Pair {
        original: &guard_original("1", partial),
        help: PARAMS_HELP,
        extra: Vec::new(),
        old: &[T232, T256, M101, T104],
        current: &[T232, M101, T104],
        corrected: &guard_corrected("1", partial),
        corrected_old: &[T232, T256, M101],
        corrected_current: &[T232, M101],
    });
    let owned = guard_original(r#""x""#, complete);
    let helper = guard_corrected(r#""x""#, complete);
    assert_pair(Pair {
        original: &owned,
        help: PARAMS_HELP,
        extra: Vec::new(),
        old: &[T232, T256, T104],
        current: &[T232, T256, T104],
        corrected: &helper,
        corrected_old: &[T232, T256],
        corrected_current: &[T232, T256],
    });
    assert_eq!(full(&owned, false), full(&owned, true));
    assert_eq!(full(&helper, false), full(&helper, true));
}

#[test]
fn original_boolean_match_keeps_exact_catch_all_refusal_and_admitted_controls() {
    // REFUTABLE-MATCH-V1 requires a trailing unguarded catch-all, including
    // Boolean matches whose literal arms cover both values.
    let original = module(BOOL_MATCH);
    let expected = authored(
        "SPX-T257",
        "refutable match requires a trailing irrefutable catch-all arm (`_` or a binding) without a guard",
        span_at(&original, "false => 0,", 0, "false => 0"),
        None,
    );
    assert_eq!(
        full(&original, true).as_slice(),
        std::slice::from_ref(&expected)
    );
    assert_eq!(
        full(&original, false).as_slice(),
        std::slice::from_ref(&expected)
    );
    assert_eq!(full(&original, false)[0].json(), expected.json());
    for body in [
        "match true { true => 1, false => 0, _ => 0, }",
        "match 1 { 0 => 1, n => n, }",
    ] {
        let admitted = module(body);
        assert!(full(&admitted, true).is_empty());
        assert_eq!(full(&admitted, false), full(&admitted, true));
        assert_eq!(receive(&admitted, false), receive(&admitted, true));
        assert_roundtrip(&admitted);
    }
}

#[test]
fn negative_and_counterpart_full_receivers_release_actual_storage_through_drop() {
    // Hold only the immutable prelude table; the verifier is never warmed.
    let held = crate::prelude::declarations();
    let identity = (held.as_ptr(), held.len());
    for text in [
        module(RESULT_ORIGINAL),
        module(OUTCOME),
        owned_view_original(),
        owned_view_corrected(),
    ] {
        for old in [true, false] {
            let (count, metrics) = measure(|| full(&text, old).len());
            assert!(count > 0, "old={old}: {text}");
            assert!(
                !metrics.invalid,
                "old={old}: invalid observation with prelude held: {metrics:?}"
            );
            assert!(metrics.allocations > 0, "old={old}: {metrics:?}");
            assert_eq!(
                metrics.live, 0,
                "old={old}: live bytes after Drop with prelude held: {metrics:?}"
            );
            assert_eq!(
                metrics.allocated, metrics.freed,
                "old={old}: allocated/freed mismatch with prelude held: {metrics:?}"
            );
            let after = crate::prelude::declarations();
            assert_eq!(identity, (after.as_ptr(), after.len()), "old={old}");
            assert!(std::ptr::eq(held, after), "old={old}");
        }
    }
}

#[test]
fn admitted_helpers_and_valid_main_keep_full_success_canonical_graph_and_receiver_parity() {
    for text in [
        r#"module test.valid_bool_helper; @id("app.probe") fn probe() -> bool { let flag: bool = true; flag } @id("app.main") fn main() -> i64 { if probe() { 0 } else { 1 } }"#,
        r#"module test.valid_string_helper; @id("app.probe") fn probe(owner: string) -> i64 { string_len(owner) } @id("app.main") fn main() -> i64 { probe("x") }"#,
        r#"module test.valid_borrow_helper; @id("app.probe") fn probe(held: borrow Bytes) -> i64 { 0 } @id("app.main") fn main() -> i64 { 0 }"#,
    ] {
        assert!(
            full(text, true).is_empty(),
            "complete original success: {text}"
        );
        assert_eq!(full(text, false), full(text, true));
        assert_eq!(receive(text, false), receive(text, true));
        assert_roundtrip(text);
    }
}
