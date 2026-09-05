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
fn missing_module_header_shows_the_first_line() {
    let diagnostic = rejection("@id(\"app.main\")\nfn main() -> i64\n{\n    42\n}\n");
    assert_eq!(diagnostic.code, "SPX-P104");
    assert_eq!(diagnostic.message, "expected `module`");
    assert!(
        help(&diagnostic).contains("`module dotted.name;`"),
        "{diagnostic}"
    );
    // The header keyword is still required and still parsed the same way.
    assert_eq!(
        rejection("modul app.x;\n@id(\"app.main\")\nfn main() -> i64\n{\n    42\n}\n").code,
        "SPX-P104"
    );
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
fn trailing_expression_semicolon_is_never_silently_rewritten() {
    for tail in ["42;", "side(1);", "let value = 1; value;"] {
        let source = format!(
            "module habit.trailing_semicolon;\n\
             @id(\"habit.side\")\nfn side(value: i64) -> i64 {{ value }}\n\
             @id(\"app.main\")\nfn main() -> i64\n{{\n    {tail}\n}}\n"
        );
        let diagnostic = rejection(&source);
        assert_eq!(diagnostic.code, "SPX-P106", "{diagnostic}");
        assert_eq!(diagnostic.message, "expected `}` after block");
        assert!(
            help(&diagnostic).contains("final value expression"),
            "{diagnostic}"
        );
    }
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
        help(&diagnostic).contains("cannot stand as a statement")
            && help(&diagnostic).contains("add an `else` branch")
            && help(&diagnostic).contains("let _ = if"),
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

#[test]
fn foreign_declaration_keywords_name_the_local_form() {
    let cases = [
        (
            "struct P {\n    @id(\"m.p.x\")\n    x: i64,\n}\n",
            "record Name",
        ),
        ("enum E {\n    @id(\"m.e.a\")\n    A,\n}\n", "variant Name"),
        ("pub fn f() -> i64\n{\n    1\n}\n", "no visibility keyword"),
        ("const LIMIT: i64 = 10;\n", "fn name() -> i64 { value }"),
    ];
    for (declaration, expected_help) in cases {
        let source = format!(
            "module habit.decl;\n@id(\"m.d\")\n{declaration}@id(\"app.main\")\nfn main() -> i64\n{{\n    0\n}}\n"
        );
        let diagnostic = rejection(&source);
        assert_eq!(diagnostic.code, "SPX-P104", "{diagnostic}");
        assert_eq!(diagnostic.message, "expected `fn`", "{diagnostic}");
        assert!(help(&diagnostic).contains(expected_help), "{diagnostic}");
    }
}

#[test]
fn missing_trailing_comma_names_the_rule_for_fields_and_arms() {
    let record = rejection(
        "module habit.comma;\n@id(\"m.p\")\nrecord P {\n    @id(\"m.p.x\")\n    x: i64\n}\n@id(\"app.main\")\nfn main() -> i64\n{\n    0\n}\n",
    );
    assert_eq!(record.code, "SPX-P106");
    assert_eq!(record.message, "expected `,` after record field");
    assert!(
        help(&record).contains("every record field ends with `,`, including the last"),
        "{record}"
    );

    let arm = rejection(
        "module habit.comma;\n@id(\"app.main\")\nfn main() -> i64\n{\n    let x = 1;\n    match x { 0 => 0, _ => 1 }\n}\n",
    );
    assert_eq!(arm.code, "SPX-P106");
    assert_eq!(arm.message, "expected `,` after match arm");
    assert!(
        help(&arm).contains("every match arm ends with `,`, including the last"),
        "{arm}"
    );
}

#[test]
fn compound_assignment_shows_the_plain_form() {
    let diagnostic = rejection(
        "module habit.compound;\n@id(\"app.main\")\nfn main() -> i64\n{\n    let mut x = 1;\n    x += 1;\n    x\n}\n",
    );
    assert_eq!(diagnostic.code, "SPX-P201");
    assert_eq!(diagnostic.message, "compound assignment is not admitted");
    assert!(help(&diagnostic).contains("x = x + …;"), "{diagnostic}");
}

#[test]
fn missing_return_type_and_unit_type_name_the_result_rule() {
    let missing = rejection(
        "module habit.ret;\n@id(\"m.f\")\nfn f()\n{\n    1\n}\n@id(\"app.main\")\nfn main() -> i64\n{\n    0\n}\n",
    );
    assert_eq!(missing.code, "SPX-P106");
    assert_eq!(missing.message, "expected `->` before return type");
    assert!(
        help(&missing).contains("return `i64` or `bool`"),
        "{missing}"
    );

    let unit = rejection(
        "module habit.unit;\n@id(\"m.f\")\nfn f() -> ()\n{\n    1\n}\n@id(\"app.main\")\nfn main() -> i64\n{\n    0\n}\n",
    );
    assert_eq!(unit.code, "SPX-P105");
    assert_eq!(unit.message, "expected type");
    assert!(help(&unit).contains("no unit type"), "{unit}");
}

#[test]
fn uninitialised_let_and_assignment_condition_name_their_rules() {
    let uninitialised = rejection(
        "module habit.let;\n@id(\"app.main\")\nfn main() -> i64\n{\n    let x: i64;\n    0\n}\n",
    );
    assert_eq!(uninitialised.code, "SPX-P106");
    assert!(
        help(&uninitialised).contains("no uninitialised binding"),
        "{uninitialised}"
    );

    let condition = rejection(
        "module habit.cond;\n@id(\"app.main\")\nfn main() -> i64\n{\n    let x = 1;\n    if x = 1 { 1 } else { 0 }\n}\n",
    );
    assert_eq!(condition.code, "SPX-P106");
    assert!(
        help(&condition).contains("comparison is `==`"),
        "{condition}"
    );
}

#[test]
fn indexing_syntax_points_at_byte_get() {
    let diagnostic = rejection(
        "module habit.index;\n@id(\"app.main\")\nfn main() -> i64\n{\n    let a = [1u8, 2u8];\n    if a[0] == 1u8 { 0 } else { 1 }\n}\n",
    );
    assert_eq!(diagnostic.code, "SPX-P106");
    assert!(
        help(&diagnostic).contains("byte_get(view, index)"),
        "{diagnostic}"
    );
}
