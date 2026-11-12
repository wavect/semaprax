//! Parser fix hints for habits carried over from other languages.
//!
//! Each case is the first thing a newcomer or a coding agent tends to write.
//! Rejected habits retain stable diagnostics and fixes. Lexical exits are
//! explicitly admitted now; their original inputs remain executable positive
//! controls rather than obsolete grammar refusals.

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

fn admitted_lexical(source: &str, expected: &str, stem: &str) {
    let program = semaprax::check(source, "habit.spx").unwrap();
    let canonical = semaprax::format::canonical(&program);
    let roundtrip = semaprax::check(&canonical, "habit.spx").unwrap();
    assert_eq!(canonical, semaprax::format::canonical(&roundtrip));
    assert_eq!(
        semaprax::graph::to_json(&program).unwrap(),
        semaprax::graph::to_json(&roundtrip).unwrap()
    );
    semaprax::hir::validate(&semaprax::hir::resolve(&program).unwrap()).unwrap();
    let directory = std::env::temp_dir().join(format!(
        "semaprax-foreign-habit-{}-{stem}",
        std::process::id()
    ));
    std::fs::create_dir_all(&directory).unwrap();
    let path = directory.join("main.spx");
    std::fs::write(&path, source).unwrap();
    let result = semaprax::interpreter::internal_strings::interpret(
        &path,
        "main",
        &[],
        &semaprax::interpreter::InterpreterOptions::default(),
    )
    .unwrap();
    let envelope: serde_json::Value = serde_json::from_str(&result.envelope).unwrap();
    assert_eq!(envelope["payload"]["outcome"]["kind"], "returned");
    assert_eq!(envelope["payload"]["outcome"]["value"], expected);
    std::fs::remove_dir_all(directory).unwrap();
}

#[test]
fn loop_return_is_checked_and_value_boundary_crossings_remain_closed() {
    admitted_lexical(
        r#"
module habit.ret;
@id("app.main")
fn main() -> i64
{
    while true { return 42; }
    0
}
"#,
        "42",
        "loop-return",
    );
    for body in [
        "let value={while false {0} return 42;};value",
        "let call=fn()->i64 {while false {0} return 42;};0",
    ] {
        let source = main_body(body);
        let program = parse(&source, Path::new("habit-crossing.spx")).unwrap();
        let diagnostics = semaprax::verify::verify(&program);
        let diagnostic = diagnostics
            .iter()
            .find(|d| d.code == "SPX-T312")
            .expect("value boundary stays closed");
        assert!(help(diagnostic).contains(
            "initializer, argument, aggregate, return-operand, yield and closure crossings"
        ));
    }
}

#[test]
fn for_and_loop_point_at_while() {
    let range = rejection(
        "module habit.loops;\n@id(\"app.main\")\nfn main() -> i64\n{\n    let mut t = 0;\n    for i in 0..3 { t = t + i; }\n    t\n}\n",
    );
    assert_eq!(range.code, "SPX-P105");
    assert!(
        range.message.contains("range") || range.message.contains("`for`"),
        "{range}"
    );
    assert!(
        help(&range).contains("while") || help(&range).contains("range"),
        "{range}"
    );

    let loope = rejection(
        "module habit.loops;\n@id(\"app.main\")\nfn main() -> i64\n{\n    loop { 1 }\n}\n",
    );
    assert_eq!(loope.code, "SPX-P106");
    assert!(loope.message.contains("`while`"), "{loope}");
    assert!(
        help(&loope).contains("condition controls repetition"),
        "{loope}"
    );
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
    admitted_lexical(
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
        "0",
        "scalar-expression-statement",
    );
    let diagnostic = rejection(&main_body("1;"));
    assert_eq!(diagnostic.code, "SPX-P106");
    assert_eq!(diagnostic.message, "expected `}` after block");
    // The habitual `;` after a bare exit guard is now admitted as the same
    // statement `if` as the unterminated spelling, with identical canonical text.
    let terminated = main_body("let mut i=0;while i<2 {i=i+1;if i==1 {continue;} else {0};0} i");
    admitted_lexical(&terminated, "2", "terminated-exit-guard");
    let unterminated = terminated.replace("else {0};0", "else {0} 0");
    admitted_lexical(&unterminated, "2", "valued-exit-guard");
    let canonical =
        |source: &str| semaprax::format::canonical(&semaprax::check(source, "habit.spx").unwrap());
    assert_eq!(canonical(&terminated), canonical(&unterminated));
    // Value boundaries keep their refusals: an initializer and an operator
    // suffix are generic discarded initializers, and a final `;` has no item.
    for closed in [
        terminated.replace(
            "if i==1 {continue;} else {0};",
            "let _=if i==1 {continue;} else {0};",
        ),
        terminated.replace("else {0};0", "else {0} + 0;0"),
    ] {
        let diagnostics = semaprax::check(&closed, "habit.spx").unwrap_err();
        assert!(
            diagnostics.iter().any(|d| d.code == "SPX-T312"),
            "{closed}: {diagnostics:?}"
        );
    }
    assert_eq!(
        rejection(&terminated.replace("else {0};0}", "else {0};}")).code,
        "SPX-P106"
    );
}

#[test]
fn keyword_and_unknown_identifier_statements_point_at_the_offending_token() {
    for word in ["module", "struct", "frobnicate"] {
        let source = format!(
            "module habit.bad_statement;\n@id(\"app.main\")\nfn main() -> i64\n{{\n    {word};\n    42\n}}\n"
        );
        let diagnostics = semaprax::check(&source, "habit.spx").unwrap_err();
        let diagnostic = diagnostics
            .iter()
            .find(|d| d.code == "SPX-T202")
            .expect("unknown operand remains rejected before discard typing");
        assert_eq!(
            diagnostic.span.map(|span| (span.line, span.column)),
            Some((5, 5)),
            "{diagnostic}"
        );
    }
}

#[test]
fn utf8_bom_has_a_targeted_removal_diagnostic() {
    let diagnostic =
        rejection("\u{feff}module habit.bom;\n@id(\"app.main\")\nfn main() -> i64 { 0 }\n");
    assert_eq!(diagnostic.code, "SPX-P001");
    assert!(
        diagnostic.message.contains("byte-order mark"),
        "{diagnostic}"
    );
    assert!(help(&diagnostic).contains("remove"), "{diagnostic}");
    assert_eq!(
        diagnostic.span.map(|span| (span.line, span.column)),
        Some((1, 1))
    );
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
    let diagnostic =
        rejection("module habit.if_tail; @id(\"app.main\") fn main()->i64 {if true {1} else {2};}");
    assert_eq!(diagnostic.code, "SPX-P106");
    assert_eq!(diagnostic.message, "expected `}` after block");
    assert!(help(&diagnostic).contains("a final value expression has no trailing `;`"));
    assert!(help(&diagnostic).contains("intermediate Copy scalar `if ... else ...` may use `;`"));
    assert!(help(&diagnostic).contains("never in a `let` initializer"));
    let program = semaprax::check(
        "module habit.if_statement; @id(\"app.main\") fn main()->i64 {if true {1} else {2}; 3}",
        "habit.spx",
    )
    .unwrap();
    let canonical = semaprax::format::canonical(&program);
    assert_eq!(
        canonical,
        semaprax::format::canonical(&parse(&canonical, "habit.spx").unwrap())
    );
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
        help(&diagnostic).contains("value is discarded"),
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
}
"#,
    );
    assert_eq!(diagnostic.code, "SPX-P203");
    assert!(
        help(&diagnostic).contains("statement `if` yields no value")
            && help(&diagnostic).contains("final expression after it")
            && help(&diagnostic).contains("add an `else` branch"),
        "{diagnostic}"
    );
}

#[test]
fn empty_function_body_names_the_rule() {
    let diagnostic = rejection("module habit.empty;\n@id(\"app.main\")\nfn main() -> i64\n{\n}\n");
    assert_eq!(diagnostic.code, "SPX-P203");
    assert!(help(&diagnostic).contains("return value;"), "{diagnostic}");
    assert!(
        help(&diagnostic).contains("terminal exhaustive `match`")
            && help(&diagnostic).contains("explicitly return on every path")
            && help(&diagnostic).contains("loop and closure returns remain refused"),
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
fn else_if_is_sugar_for_the_nested_spelling() {
    let chained = "module habit.elif;\n@id(\"app.main\")\nfn main() -> i64\n{\n    let x = 2;\n    if x == 0 { 0 } else if x == 1 { 1 } else { 2 }\n}\n";
    let nested = chained.replace(
        "else if x == 1 { 1 } else { 2 }",
        "else { if x == 1 { 1 } else { 2 } }",
    );
    let chained = parse(chained, Path::new("elif.spx")).unwrap();
    let nested = parse(&nested, Path::new("elif.spx")).unwrap();
    assert!(semaprax::verify::verify(&chained).is_empty());
    assert_eq!(
        semaprax::format::canonical(&chained),
        semaprax::format::canonical(&nested)
    );
    assert_eq!(
        semaprax::graph::revision(&chained),
        semaprax::graph::revision(&nested)
    );
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
    // The last comma before `}` is optional in declarations; `fmt` writes it.
    for (declaration, canonical) in [
        ("record P {\n    x: i64\n}\n", "    x: i64,\n"),
        ("record P { x: i64, y: bool }\n", "    y: bool,\n"),
        ("variant V { A, B }\n", "    B,\n"),
        ("variant V { A { n: i64 }, B }\n", "        n: i64,\n"),
    ] {
        let source = format!(
            "module habit.comma;\n{declaration}@id(\"app.main\")\nfn main() -> i64\n{{\n    0\n}}\n"
        );
        let program = parse(&source, Path::new("comma.spx"))
            .unwrap_or_else(|diagnostic| panic!("{declaration}: {diagnostic}"));
        let formatted = semaprax::format::canonical(&program);
        assert!(formatted.contains(canonical), "{formatted}");
    }

    // Between two fields the comma is still required.
    let record = rejection(
        "module habit.comma;\n@id(\"m.p\")\nrecord P {\n    @id(\"m.p.x\")\n    x: i64\n    y: i64,\n}\n@id(\"app.main\")\nfn main() -> i64\n{\n    0\n}\n",
    );
    assert_eq!(record.code, "SPX-P106");
    assert_eq!(record.message, "expected `,` after record field");
    assert!(
        help(&record).contains("every record field ends with `,`"),
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

#[test]
fn range_for_hint_preserves_vector_traversal_and_other_projection_errors() {
    parse(
        "module habit.vector;\n@id(\"app.main\") fn main() -> i64 { for item in values { item } 0 }\n",
        Path::new("vector-for.spx"),
    ).expect("ordinary for traversal still parses");
    let diagnostic = rejection(
        "module habit.projection;\n@id(\"app.main\") fn main() -> i64 { for item in values.3 { item } 0 }\n",
    );
    assert_eq!(diagnostic.code, "SPX-P105");
    assert!(!diagnostic.message.contains("range"));
}

fn main_body(body: &str) -> String {
    format!("module habit.op;\n@id(\"app.main\")\nfn main() -> i64\n{{\n{body}\n}}\n")
}

#[test]
fn operators_and_statements_from_other_languages_name_the_admitted_form() {
    admitted_lexical(
        &main_body("    let mut i = 0;\n    while true { i = i + 1; break; }\n    i"),
        "1",
        "loop-break",
    );
    admitted_lexical(
        &main_body("    let mut i = 0;\n    while i < 2 { i = i + 1; continue; }\n    i"),
        "2",
        "loop-continue",
    );
    for exit in ["break", "continue"] {
        let diagnostic = rejection(&main_body(&format!("while false {{ {exit} }}\n0")));
        assert_eq!(diagnostic.code, "SPX-P106");
        assert_eq!(diagnostic.message, format!("expected `;` after `{exit}`"));
        assert!(help(&diagnostic).contains(&format!("`{exit};`")));
    }
    let cases = [
        (
            "    let x = 1;\n    x > 0 ? 1 : 2",
            "SPX-P106",
            "`if <condition> { a } else { b }`",
        ),
        (
            "    let x = 1i32;\n    x as i64",
            "SPX-P106",
            "convert with `f64_from_i64(x)`, `i64_from_f64(x)`",
        ),
        (
            "    let a = true and false;\n    0",
            "SPX-P106",
            "logical and is `&&`",
        ),
        (
            "    let a = true or false;\n    0",
            "SPX-P106",
            "logical or is `||`",
        ),
        (
            "    let f = |x: i64| x + 1;\n    0",
            "SPX-P201",
            "`fn(x: i64) -> i64 { x + 1 }`",
        ),
        ("    let a = [1, 2, 3];\n    0", "SPX-T262", "`Vec`"),
        (
            "    let o = Option<i64>::Some { value: 1 };\n    if let Option::Some { value: v } = o { v } else { 0 }",
            "SPX-P106",
            "there is no `if let`",
        ),
        (
            "    let x = 2;\n    match x { 0..=5 => 1, _ => 0, }",
            "SPX-P106",
            "guard the arm",
        ),
        (
            "    let s = format!(\"{}\", 1);\n    0",
            "SPX-P106",
            "there are no macros",
        ),
        (
            "    println!(\"hi\");\n    0",
            "SPX-P106",
            "there are no macros",
        ),
        (
            "    let mut i = 0;\n    i++;\n    i",
            "SPX-P201",
            "`i = i + 1;`",
        ),
        (
            "    let mut i = 0;\n    i--;\n    i",
            "SPX-P201",
            "`i = i + 1;`",
        ),
        (
            "    let x = 2;\n    match x { 1 => 10 _ => 0, }",
            "SPX-P106",
            "every match arm ends with `,`",
        ),
        (
            "    println(\"hi\");\n    0",
            "SPX-T203",
            "stdout_write(str_as_bytes(view))",
        ),
        (
            "    let v = vec_with_capacity<i64>(1usize);\n    for x in v { let y = x; }\n    0",
            "SPX-P203",
            "end the `for` body",
        ),
    ];
    for (body, code, expected_help) in cases {
        let diagnostic = if code == "SPX-T203" {
            semaprax::check(&main_body(body), "habit.spx")
                .unwrap_err()
                .into_iter()
                .find(|d| d.code == code)
                .unwrap()
        } else {
            rejection(&main_body(body))
        };
        assert_eq!(diagnostic.code, code, "{body}: {diagnostic}");
        assert!(
            help(&diagnostic).contains(expected_help),
            "{body}: {diagnostic}"
        );
    }
}

#[test]
fn path_imports_show_the_declaration_import_form() {
    let diagnostic = rejection(
        "module habit.use;\nuse std::io;\n@id(\"app.main\")\nfn main() -> i64\n{\n    0\n}\n",
    );
    assert_eq!(diagnostic.code, "SPX-G170");
    assert!(
        help(&diagnostic).contains("use function @id("),
        "{diagnostic}"
    );
}

#[test]
fn tuple_variant_cases_show_the_named_field_form() {
    let diagnostic = rejection(
        "module habit.variant;\n@id(\"habit.shape\")\nvariant Shape {\n    @id(\"habit.shape.circle\")\n    Circle(i64),\n}\n@id(\"app.main\")\nfn main() -> i64\n{\n    0\n}\n",
    );
    assert_eq!(diagnostic.code, "SPX-P106");
    assert!(
        help(&diagnostic).contains("radius: i64, },"),
        "{diagnostic}"
    );
}

#[test]
fn mutable_parameters_show_the_local_copy() {
    let diagnostic = rejection(
        "module habit.p;\n@id(\"habit.f\")\nfn f(mut x: i64) -> i64\n{\n    x\n}\n@id(\"app.main\")\nfn main() -> i64\n{\n    f(1)\n}\n",
    );
    assert_eq!(diagnostic.code, "SPX-U103");
    assert!(
        help(&diagnostic).contains("`let mut current = <parameter>;`"),
        "{diagnostic}"
    );
}
