//! Frontend-only evidence for explicit ownership modes on `match`.
//!
//! Semantic ownership validation and lowering are intentionally exercised by
//! later phases. These tests freeze the contextual grammar, canonical source,
//! malformed-mode diagnostic, and legacy `own`/`borrow` identifier behavior.

use std::path::Path;

use semaprax::ast::{ExprKind, MatchMode, Program};
use semaprax::{format, parse};

fn tail_match_mode(program: &Program, function_index: usize) -> MatchMode {
    let ExprKind::Block { tail, .. } = &program.functions[function_index].body.kind else {
        panic!("function body must be a block");
    };
    let ExprKind::Match { mode, .. } = &tail.kind else {
        panic!("function tail must be a match");
    };
    *mode
}

#[test]
fn explicit_own_and_borrow_modes_round_trip_canonically() {
    let source = r#"
module syntax.match_modes;

fn owned(value: i64) -> i64 {
    match own value { _ => 1, }
}

fn borrowed(value: i64) -> i64 {
    match borrow value { _ => 2, }
}
"#;
    let parsed = parse(source, Path::new("match-modes.spx")).unwrap();
    assert_eq!(tail_match_mode(&parsed, 0), MatchMode::Own);
    assert_eq!(tail_match_mode(&parsed, 1), MatchMode::Borrow);

    let canonical = format::canonical(&parsed);
    assert!(canonical.contains("match own value { _ => 1, }"));
    assert!(canonical.contains("match borrow value { _ => 2, }"));
    let reparsed = parse(&canonical, Path::new("match-modes-canonical.spx")).unwrap();
    assert_eq!(canonical, format::canonical(&reparsed));
    assert_eq!(tail_match_mode(&reparsed, 0), MatchMode::Own);
    assert_eq!(tail_match_mode(&reparsed, 1), MatchMode::Borrow);
}

#[test]
fn own_and_borrow_identifiers_keep_legacy_match_grammar_and_bytes() {
    let source = r#"
module syntax.match_mode_ambiguity;

fn own(value: i64) -> i64 { value }
fn borrow(value: i64) -> i64 { value }

fn own_name(own: i64) -> i64 {
    match own { _ => 1, }
}

fn borrow_name(borrow: i64) -> i64 {
    match borrow { _ => 2, }
}

fn own_call(value: i64) -> i64 {
    match own(value) { _ => 3, }
}

fn borrow_call(value: i64) -> i64 {
    match borrow(value) { _ => 4, }
}
"#;
    let parsed = parse(source, Path::new("match-mode-legacy.spx")).unwrap();
    for index in 2..6 {
        assert_eq!(tail_match_mode(&parsed, index), MatchMode::Value);
    }

    let canonical = format::canonical(&parsed);
    assert!(canonical.contains("match own { _ => 1, }"));
    assert!(canonical.contains("match borrow { _ => 2, }"));
    assert!(canonical.contains("match own(value) { _ => 3, }"));
    assert!(canonical.contains("match borrow(value) { _ => 4, }"));
    assert!(!canonical.contains("match own own"));
    assert!(!canonical.contains("match borrow borrow"));
    let reparsed = parse(&canonical, Path::new("match-mode-legacy-canonical.spx")).unwrap();
    assert_eq!(canonical, format::canonical(&reparsed));
}

#[test]
fn own_and_borrow_identifier_record_updates_remain_legacy_scrutinees() {
    let source = r#"
module syntax.match_mode_update_ambiguity;

record Pair { value: i64, }

fn own_update(own: Pair, value: i64) -> i64 {
    match own with { value: value } { _ => 1, }
}

fn borrow_update(borrow: Pair, value: i64) -> i64 {
    match borrow with { value: value } { _ => 2, }
}
"#;
    let parsed = parse(source, Path::new("match-mode-update-legacy.spx")).unwrap();
    assert_eq!(tail_match_mode(&parsed, 0), MatchMode::Value);
    assert_eq!(tail_match_mode(&parsed, 1), MatchMode::Value);

    let canonical = format::canonical(&parsed);
    assert!(
        canonical.contains("match own with { value: value } { _ => 1, }"),
        "{canonical}"
    );
    assert!(
        canonical.contains("match borrow with { value: value } { _ => 2, }"),
        "{canonical}"
    );
    let reparsed = parse(
        &canonical,
        Path::new("match-mode-update-legacy-canonical.spx"),
    )
    .unwrap();
    assert_eq!(canonical, format::canonical(&reparsed));
}

#[test]
fn repeated_or_conflicting_match_modes_have_a_stable_diagnostic() {
    for source in [
        "module bad; fn f(value:i64)->i64 { match own borrow value { _ => 0, } }",
        "module bad; fn f(value:i64)->i64 { match borrow own value { _ => 0, } }",
        "module bad; fn f(value:i64)->i64 { match own own value { _ => 0, } }",
        "module bad; fn f(value:i64)->i64 { match borrow borrow value { _ => 0, } }",
    ] {
        let error = parse(source, Path::new("match-mode-malformed.spx")).unwrap_err();
        assert_eq!(error.code, "SPX-P207", "unexpected diagnostic: {error}");
        assert!(
            error.message.contains("exactly one ownership mode"),
            "unexpected diagnostic: {error}"
        );
    }
}

#[test]
fn unambiguous_unary_scrutinees_keep_the_authored_mode() {
    let source = r#"
module syntax.match_mode_expression_starts;

fn unary(value: i64) -> i64 {
    match borrow -value { _ => 2, }
}
"#;
    let parsed = parse(source, Path::new("match-mode-expression-starts.spx")).unwrap();
    assert_eq!(tail_match_mode(&parsed, 0), MatchMode::Borrow);
    let canonical = format::canonical(&parsed);
    assert!(canonical.contains("match borrow -value { _ => 2, }"));
}
