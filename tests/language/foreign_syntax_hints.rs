//! Parser fix hints for habits carried over from other languages.
//!
//! Each case is the first thing a newcomer or a coding agent tends to write.
//! The grammar already rejected every one of them; these regressions pin the
//! stable code the rejection keeps, the fix the diagnostic now carries, and
//! the invariant that no hint admits new syntax.

use std::path::Path;

use semaprax::diagnostic::Diagnostic;
use semaprax::parse;

fn rejection(source: &str) -> Diagnostic {
    parse(source, Path::new("habit.spx")).expect_err("the grammar must still reject this input")
}

fn help(diagnostic: &Diagnostic) -> &str {
    diagnostic
        .help
        .as_deref()
        .unwrap_or_else(|| panic!("{diagnostic} carries no fix hint"))
}

#[test]
fn return_statement_names_the_tail_expression_rule() {
    let diagnostic = rejection(
        r#"
module habit.ret;
@id("app.main")
fn main() -> i64
{
    return 42;
}
"#,
    );
    assert_eq!(diagnostic.code, "SPX-P106");
    assert!(diagnostic.message.contains("`return`"), "{diagnostic}");
    assert!(
        help(&diagnostic).contains("last expression"),
        "{diagnostic}"
    );
    assert_eq!(
        diagnostic.span.map(|span| (span.line, span.column)),
        Some((6, 5))
    );
}

#[test]
fn for_and_loop_point_at_while() {
    for source in [
        "module habit.loops;\n@id(\"app.main\")\nfn main() -> i64\n{\n    let mut t = 0;\n    for i in 0..3 { t = t + i; }\n    t\n}\n",
        "module habit.loops;\n@id(\"app.main\")\nfn main() -> i64\n{\n    loop { 1 }\n}\n",
    ] {
        let diagnostic = rejection(source);
        assert_eq!(diagnostic.code, "SPX-P106");
        assert!(diagnostic.message.contains("`while`"), "{diagnostic}");
        assert!(help(&diagnostic).contains("decides whether to loop again"), "{diagnostic}");
    }
}

#[test]
fn a_binding_named_return_still_parses() {
    let program = parse(
        "module habit.ident;\n@id(\"app.main\")\nfn main() -> i64\n{\n    let return = 41;\n    return + 1\n}\n",
        Path::new("habit.spx"),
    )
    .expect("`return` is an ordinary identifier when nothing follows it as an operand");
    assert_eq!(program.functions.len(), 1);
}

#[test]
fn expression_statement_explains_the_block_shape() {
    let diagnostic = rejection(
        r#"
module habit.stmt;
@id("habit.side")
fn side(value: i64) -> i64
{
    value
}
@id("app.main")
fn main() -> i64
{
    side(1);
    0
}
"#,
    );
    assert_eq!(diagnostic.code, "SPX-P106");
    assert_eq!(diagnostic.message, "expected `}` after block");
    assert!(help(&diagnostic).contains("let _ = …;"), "{diagnostic}");
}

#[test]
fn while_body_without_a_continuation_names_the_rule() {
    let diagnostic = rejection(
        r#"
module habit.wh;
@id("app.main")
fn main() -> i64
{
    let mut i = 0;
    while i < 3 {
        i = i + 1;
    }
    i
}
"#,
    );
    assert_eq!(diagnostic.code, "SPX-P203");
    assert!(
        help(&diagnostic).contains("loop condition repeated"),
        "{diagnostic}"
    );
}

#[test]
fn if_branch_without_a_value_names_the_rule() {
    let diagnostic = rejection(
        r#"
module habit.branch;
@id("app.main")
fn main() -> i64
{
    let mut x = 0;
    if x == 0 { x = 1; }
    x
}
"#,
    );
    assert_eq!(diagnostic.code, "SPX-P203");
    assert!(
        help(&diagnostic).contains("`if` is an expression"),
        "{diagnostic}"
    );
}

#[test]
fn empty_function_body_names_the_rule() {
    let diagnostic = rejection("module habit.empty;\n@id(\"app.main\")\nfn main() -> i64\n{\n}\n");
    assert_eq!(diagnostic.code, "SPX-P203");
    assert!(
        help(&diagnostic).contains("there is no `return`"),
        "{diagnostic}"
    );
}

#[test]
fn inner_block_help_wins_over_the_enclosing_body() {
    let diagnostic = rejection(
        "module habit.nested;\n@id(\"app.main\")\nfn main() -> i64\n{\n    let mut i = 0;\n    while i < 1 { i = 1; }\n    i\n}\n",
    );
    assert_eq!(diagnostic.code, "SPX-P203");
    assert!(help(&diagnostic).contains("`while` body"), "{diagnostic}");
}

#[test]
fn else_if_shows_the_nested_spelling() {
    let diagnostic = rejection(
        "module habit.elif;\n@id(\"app.main\")\nfn main() -> i64\n{\n    let x = 2;\n    if x == 0 { 0 } else if x == 1 { 1 } else { 2 }\n}\n",
    );
    assert_eq!(diagnostic.code, "SPX-P106");
    assert!(diagnostic.message.contains("`else if`"), "{diagnostic}");
    assert!(help(&diagnostic).contains("else { if"), "{diagnostic}");
}

#[test]
fn missing_else_names_the_expression_rule() {
    let diagnostic = rejection(
        "module habit.noelse;\n@id(\"app.main\")\nfn main() -> i64\n{\n    if true { 42 }\n}\n",
    );
    assert_eq!(diagnostic.code, "SPX-P104");
    assert_eq!(diagnostic.message, "expected `else`");
    assert!(
        help(&diagnostic).contains("always has an `else`"),
        "{diagnostic}"
    );
}

#[test]
fn call_shaped_pattern_shows_the_field_spelling() {
    let diagnostic = rejection(
        "module habit.pat;\n@id(\"app.main\")\nfn main() -> i64\n{\n    let sample = [1u8];\n    let view = array_as_slice(sample);\n    match byte_get(view, 0usize) { Some(b) => 1, None => 0, }\n}\n",
    );
    assert_eq!(diagnostic.code, "SPX-P106");
    assert_eq!(diagnostic.message, "expected `=>` after match pattern");
    assert!(
        help(&diagnostic).contains("Option::Some { value: v }"),
        "{diagnostic}"
    );
}

#[test]
fn tuple_literal_points_at_records() {
    let diagnostic = rejection(
        "module habit.tuple;\n@id(\"app.main\")\nfn main() -> i64\n{\n    let pair = (1, 2);\n    1\n}\n",
    );
    assert_eq!(diagnostic.code, "SPX-P106");
    assert_eq!(diagnostic.message, "expected `)` after expression");
    assert!(help(&diagnostic).contains("record"), "{diagnostic}");
}

#[test]
fn unrelated_parser_errors_carry_no_hint() {
    let diagnostic = rejection(
        "module habit.plain;\n@id(\"app.main\")\nfn main() -> i64\n{\n    let answer 42;\n    answer\n}\n",
    );
    assert_eq!(diagnostic.code, "SPX-P106");
    assert!(diagnostic.help.is_none(), "{diagnostic}");
}
