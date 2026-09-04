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
fn distant_or_ambiguous_names_get_no_suggestion() {
    let diagnostic = only(
        "module habit.far;\n@id(\"app.main\")\nfn main() -> i64\n{\n    frobnicate(1)\n}\n",
        "SPX-T203",
    );
    assert!(diagnostic.help.is_none(), "{diagnostic}");
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
fn mismatched_non_literal_operands_get_no_literal_hint() {
    let diagnostic = only(
        "module habit.nolit;\n@id(\"app.main\")\nfn main() -> i64\n{\n    let n = 3usize;\n    let m = 4;\n    if n < m { 1 } else { 0 }\n}\n",
        "SPX-T208",
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
