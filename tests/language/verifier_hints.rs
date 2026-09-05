//! Verifier fix hints for the type-level habits an agent brings from other
//! languages: a misspelled or foreign function name, a generic call or generic
//! variant without explicit type arguments, an unsuffixed integer literal
//! against a narrower operand, and an owned `string` handed to a byte or host
//! operation. Every case keeps its stable code; only the `help` line is new.

use std::path::Path;

use semaprax::diagnostic::Diagnostic;
use semaprax::{parse, verify};

fn diagnostics(source: &str) -> Vec<Diagnostic> {
    let program = parse(source, Path::new("habit.spx")).expect("the grammar accepts this input");
    verify::verify(&program)
}

fn only(source: &str, code: &str) -> Diagnostic {
    let found = diagnostics(source);
    let mut matching = found.iter().filter(|diagnostic| diagnostic.code == code);
    let first = matching
        .next()
        .unwrap_or_else(|| panic!("expected {code}, found {found:?}"))
        .clone();
    first
}

fn help(diagnostic: &Diagnostic) -> &str {
    diagnostic
        .help
        .as_deref()
        .unwrap_or_else(|| panic!("{diagnostic} carries no fix hint"))
}

#[test]
fn one_project_module_checked_alone_names_the_project_command() {
    let imports = only(
        "module habit.app;\nuse function @id(\"habit.core.add\") from habit.core as add;\n@id(\"app.main\")\nfn main() -> i64\n{\n    add(1, 2)\n}\n",
        "SPX-G172",
    );
    assert_eq!(
        imports.message,
        "source module imports require Workspace Semantic Graph resolution"
    );
    assert!(
        help(&imports).contains("semaprax check <project-dir>"),
        "{imports}"
    );

    let library = only(
        "module habit.core;\n@id(\"habit.core.add\")\nfn add(left: i64, right: i64) -> i64\n{\n    left + right\n}\n",
        "SPX-T105",
    );
    assert_eq!(
        library.message,
        "executable module must define `fn main() -> i64`"
    );
    assert!(help(&library).contains("library module"), "{library}");
    assert!(
        help(&library).contains("semaprax check <project-dir>"),
        "{library}"
    );
}

#[test]
fn misspelled_builtin_suggests_the_nearest_reserved_name() {
    let diagnostic = only(
        "module habit.name;\n@id(\"app.main\")\nfn main() -> i64\n{\n    let s = \"abc\";\n    string_length(s)\n}\n",
        "SPX-T203",
    );
    assert_eq!(diagnostic.message, "unknown function `string_length`");
    assert_eq!(help(&diagnostic), "did you mean `string_len`?");
}

#[test]
fn misspelled_declared_function_suggests_the_declaration() {
    let diagnostic = only(
        "module habit.decl;\n@id(\"habit.compute_total\")\nfn compute_total(v: i64) -> i64\n{\n    v\n}\n@id(\"app.main\")\nfn main() -> i64\n{\n    compute_totals(1)\n}\n",
        "SPX-T203",
    );
    assert_eq!(help(&diagnostic), "did you mean `compute_total`?");
}

#[test]
fn print_family_points_at_stdout_write() {
    for name in ["print", "println", "printf", "puts", "console_log", "echo"] {
        let diagnostic = only(
            &format!(
                "module habit.print;\n@id(\"app.main\")\nfn main() -> i64\n{{\n    let n = {name}(\"hi\");\n    0\n}}\n"
            ),
            "SPX-T203",
        );
        assert!(
            help(&diagnostic).contains("stdout_write(str_as_bytes(view))"),
            "{name}: {diagnostic}"
        );
    }
}

#[test]
fn distant_names_get_the_declare_or_import_hint_instead_of_a_guess() {
    let diagnostic = only(
        "module habit.far;\n@id(\"app.main\")\nfn main() -> i64\n{\n    frobnicate(1)\n}\n",
        "SPX-T203",
    );
    assert!(
        help(&diagnostic).contains("from other.module as frobnicate;"),
        "no near name, so the declare-or-import hint applies: {diagnostic}"
    );
}

#[test]
fn bundled_standard_library_function_names_the_dependency_route() {
    let diagnostic = only(
        "module habit.std;\n@id(\"app.main\")\nfn main() -> i64\n{\n    abs(-3)\n}\n",
        "SPX-T203",
    );
    assert_eq!(diagnostic.message, "unknown function `abs`");
    assert!(help(&diagnostic).contains("`std.num`"), "{diagnostic}");
    assert!(
        help(&diagnostic).contains("[dependencies] std.num = \"^0.1.0\""),
        "{diagnostic}"
    );
    assert!(help(&diagnostic).contains("help library"), "{diagnostic}");
}

#[test]
fn immutable_parameter_names_the_mutable_copy_repair() {
    let diagnostic = only(
        "module habit.param_mut;\n@id(\"habit.bump\")\nfn bump(value: i64) -> i64\n{\n    value = value + 1;\n    value\n}\n@id(\"app.main\")\nfn main() -> i64\n{\n    bump(1)\n}\n",
        "SPX-U101",
    );
    assert!(
        help(&diagnostic).contains("parameters are immutable"),
        "{diagnostic}"
    );
    assert!(
        help(&diagnostic).contains("let mut value = value;"),
        "{diagnostic}"
    );
}

#[test]
fn generic_call_without_type_arguments_shows_the_call_shape() {
    let diagnostic = only(
        "module habit.generic;\n@id(\"habit.id\")\nfn id<T>(v: T) -> T\n{\n    v\n}\n@id(\"app.main\")\nfn main() -> i64\n{\n    id(4)\n}\n",
        "SPX-T225",
    );
    assert!(help(&diagnostic).contains("id<i64>(…)"), "{diagnostic}");
}

#[test]
fn generic_variant_without_type_arguments_shows_the_constructor_shape() {
    let diagnostic = only(
        "module habit.variant;\n@id(\"habit.f\")\nfn f() -> Option<i64>\n{\n    Option::Some { value: 1 }\n}\n@id(\"app.main\")\nfn main() -> i64\n{\n    0\n}\n",
        "SPX-T221",
    );
    assert!(
        help(&diagnostic).contains("Option<i64>::Some { value: … }"),
        "{diagnostic}"
    );
}

#[test]
fn unsuffixed_literal_against_a_narrower_operand_names_the_suffix() {
    let diagnostic = only(
        "module habit.suffix;\n@id(\"app.main\")\nfn main() -> i64\n{\n    let n = 3usize;\n    if n < 5 { 1 } else { 0 }\n}\n",
        "SPX-T208",
    );
    assert_eq!(diagnostic.message, "operator `<` expects usize operands");
    assert!(help(&diagnostic).contains("5usize"), "{diagnostic}");
}

#[test]
fn unsuffixed_literal_in_narrow_integer_arithmetic_is_rejected_at_check() {
    for (declaration, expression, expected_help) in [
        ("let n = 3usize;", "n + 1", "1usize"),
        ("let n = 3u8;", "n * 1", "1u8"),
        ("let n = 3i32;", "n - 1", "1i32"),
    ] {
        let source = format!(
            "module habit.arithmetic_suffix;\n\
             @id(\"app.main\")\nfn main() -> i64\n{{\n    {declaration}\n    let bad = {expression};\n    0\n}}\n"
        );
        let diagnostic = only(&source, "SPX-T208");
        assert!(help(&diagnostic).contains(expected_help), "{diagnostic}");
    }
}

#[test]
fn mismatched_non_literal_operands_get_no_literal_hint() {
    let diagnostic = only(
        "module habit.nolit;\n@id(\"app.main\")\nfn main() -> i64\n{\n    let n = 3usize;\n    let m = 4;\n    if n < m { 1 } else { 0 }\n}\n",
        "SPX-T208",
    );
    assert!(diagnostic.help.is_none(), "{diagnostic}");
}

#[test]
fn owned_string_into_a_borrow_str_parameter_names_the_conversion() {
    let diagnostic = only(
        "module habit.param;\n@id(\"habit.f\")\nfn f(s: borrow str) -> i64\n{\n    if str_is_empty(s) { 0 } else { 1 }\n}\n@id(\"app.main\")\nfn main() -> i64\n{\n    let owned = \"abc\";\n    f(owned)\n}\n",
        "SPX-T205",
    );
    assert_eq!(
        diagnostic.message,
        "argument `s` to `f` expects str, received string"
    );
    assert!(
        help(&diagnostic).contains("f(string_as_str(binding))"),
        "{diagnostic}"
    );
}

#[test]
fn owned_bytes_into_a_slice_parameter_names_the_views() {
    let diagnostic = only(
        "module habit.slice;\n@id(\"habit.f\")\nfn f(v: borrow Slice<u8>) -> usize\n{\n    byte_len(v)\n}\n@id(\"app.main\")\nfn main() -> i64\n{\n    let sample = [1u8, 2u8];\n    let n = f(sample);\n    0\n}\n",
        "SPX-T205",
    );
    assert!(
        help(&diagnostic).contains("array_as_slice(array)"),
        "{diagnostic}"
    );
}

#[test]
fn scalar_argument_mismatch_gets_no_view_hint() {
    let diagnostic = only(
        "module habit.scalar;\n@id(\"habit.f\")\nfn f(v: i64) -> i64\n{\n    v\n}\n@id(\"app.main\")\nfn main() -> i64\n{\n    f(true)\n}\n",
        "SPX-T205",
    );
    assert!(diagnostic.help.is_none(), "{diagnostic}");
}

#[test]
fn owned_string_into_a_byte_view_names_the_conversion() {
    let diagnostic = only(
        "module habit.view;\npermit { process.stdout.write }\n@id(\"app.main\")\nfn main() -> i64\n    uses { process.stdout.write }\n{\n    let text = \"hi\";\n    let n = stdout_write(str_as_bytes(text));\n    0\n}\n",
        "SPX-T263",
    );
    assert!(help(&diagnostic).contains("string_as_str"), "{diagnostic}");
}

#[test]
fn owned_string_into_stdout_write_names_the_conversion() {
    let diagnostic = only(
        "module habit.host;\npermit { process.stdout.write }\n@id(\"app.main\")\nfn main() -> i64\n    uses { process.stdout.write }\n{\n    let n = stdout_write(\"hi\");\n    0\n}\n",
        "SPX-T269",
    );
    assert!(help(&diagnostic).contains("str_as_bytes"), "{diagnostic}");
}

#[test]
fn variant_shorthand_constructors_show_the_typed_spelling() {
    let some = only(
        "module habit.some;\n@id(\"habit.f\")\nfn f(flag: bool) -> Option<i64>\n{\n    if flag { Some(1) } else { Option<i64>::None {} }\n}\n@id(\"app.main\")\nfn main() -> i64\n{\n    0\n}\n",
        "SPX-T203",
    );
    assert_eq!(some.message, "unknown function `Some`");
    assert!(
        help(&some).contains("Option<i64>::Some { value: 1 }"),
        "{some}"
    );

    let none = only(
        "module habit.none;\n@id(\"habit.f\")\nfn f() -> Option<i64>\n{\n    None\n}\n@id(\"app.main\")\nfn main() -> i64\n{\n    0\n}\n",
        "SPX-T202",
    );
    assert!(help(&none).contains("Option<i64>::None {}"), "{none}");
}

#[test]
fn method_on_a_string_names_the_compiler_owned_function() {
    let diagnostic = only(
        "module habit.method;\n@id(\"app.main\")\nfn main() -> i64\n{\n    let s = \"abc\";\n    s.len()\n}\n",
        "SPX-T203",
    );
    assert_eq!(
        diagnostic.message,
        "method `len` requires a class receiver, found `string`"
    );
    assert!(help(&diagnostic).contains("string_len(s)"), "{diagnostic}");
}

#[test]
fn method_on_a_record_explains_that_records_have_no_methods() {
    let diagnostic = only(
        "module habit.rec;\n@id(\"m.p\")\nrecord P {\n    @id(\"m.p.x\")\n    x: i64,\n}\n@id(\"app.main\")\nfn main() -> i64\n{\n    let p = P { x: 1 };\n    p.total()\n}\n",
        "SPX-T203",
    );
    assert!(
        help(&diagnostic).contains("records have no methods"),
        "{diagnostic}"
    );
}

#[test]
fn foreign_type_names_point_at_the_admitted_types() {
    let cases = [
        ("String", "owned text is `string`"),
        ("int", "`i64` (the literal default)"),
        ("double", "`f64` and `f32`"),
        ("boolean", "spelled `bool`"),
        ("Vec", "no general collection type"),
    ];
    for (name, expected_help) in cases {
        let diagnostic = only(
            &format!(
                "module habit.ty;\n@id(\"habit.f\")\nfn f(v: {name}) -> i64\n{{\n    1\n}}\n@id(\"app.main\")\nfn main() -> i64\n{{\n    0\n}}\n"
            ),
            "SPX-T001",
        );
        assert!(
            help(&diagnostic).contains(expected_help),
            "{name}: {diagnostic}"
        );
    }
}

#[test]
fn a_genuinely_unknown_type_keeps_the_resource_message_without_a_hint() {
    let diagnostic = only(
        "module habit.unk;\n@id(\"habit.f\")\nfn f(v: Widget) -> i64\n{\n    1\n}\n@id(\"app.main\")\nfn main() -> i64\n{\n    0\n}\n",
        "SPX-T001",
    );
    assert_eq!(
        diagnostic.message,
        "unknown type `Widget`; declare it with `resource Widget;`"
    );
    assert!(diagnostic.help.is_none(), "{diagnostic}");
}

#[test]
fn borrowed_view_of_a_literal_names_the_binding_step() {
    let diagnostic = only(
        "module habit.view;\npermit { process.stdout.write }\n@id(\"app.main\")\nfn main() -> i64\n    uses { process.stdout.write }\n{\n    let n = stdout_write(str_as_bytes(\"hi\"));\n    0\n}\n",
        "SPX-T266",
    );
    assert_eq!(
        diagnostic.message,
        "borrowed view `str_as_bytes` requires an exact admitted storage place"
    );
    assert!(
        help(&diagnostic).contains("str_as_bytes(string_as_str(text))"),
        "{diagnostic}"
    );
}

#[test]
fn borrowed_view_of_an_array_literal_names_the_binding_step() {
    let diagnostic = only(
        "module habit.arr;\n@id(\"app.main\")\nfn main() -> i64\n{\n    let n = byte_len(array_as_slice([1u8, 2u8]));\n    0\n}\n",
        "SPX-T266",
    );
    assert!(
        help(&diagnostic).contains("array_as_slice(bytes)"),
        "{diagnostic}"
    );
}
