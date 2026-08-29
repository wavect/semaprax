use std::path::Path;

use semaprax::hosted_interpreter::{execute_language_command, HostedCommandInput};
use semaprax::interpreter::CommandEvaluationOutcome;
use semaprax::{hir, parse, verify};

const APP_SOURCE: &str = include_str!("../examples/spxgrep-lines-project/src/app.spx");
const FILTER_SOURCE: &str = include_str!("../examples/spxgrep-lines-project/src/filter.spx");

fn linked_source() -> String {
    let app = APP_SOURCE.replace(
        "use function @id(\"spxgrep-lines.contains\") from spxgrep_lines.filter as contains;\n",
        "",
    );
    let filter = FILTER_SOURCE
        .replace("module spxgrep_lines.filter;\n", "")
        .replace(
            "@id(\"spxgrep-lines.filter.main\")\nfn main() -> i64 { 0 }\n",
            "",
        );
    format!("{app}\n{filter}")
}

fn resolved() -> hir::ResolvedProgram {
    let source = linked_source();
    let ast = parse(&source, Path::new("spxgrep-lines-app.spx")).unwrap();
    let diagnostics = verify::verify(&ast);
    assert!(diagnostics.is_empty(), "{diagnostics:?}");
    hir::resolve(&ast).unwrap()
}

fn run(needle: &str, stdin: &[u8]) -> (CommandEvaluationOutcome, Vec<u8>, Vec<u8>) {
    let result = execute_language_command(
        &resolved(),
        "spxgrep-lines.run",
        &HostedCommandInput {
            arguments: vec![needle.to_owned()],
            stdin: stdin.to_vec(),
        },
        200_000,
    )
    .unwrap();
    (result.evaluation.outcome, result.stdout, result.stderr)
}

#[test]
fn line_filter_preserves_lf_and_the_final_unterminated_line_without_a_phantom() {
    let (outcome, stdout, stderr) = run("hit", b"miss\nhit one\nhit final");
    assert_eq!(outcome, CommandEvaluationOutcome::ReturnedBool(true));
    assert_eq!(stdout, b"hit one\nhit final");
    assert!(stderr.is_empty());

    let (outcome, stdout, _) = run("hit", b"hit\n");
    assert_eq!(outcome, CommandEvaluationOutcome::ReturnedBool(true));
    assert_eq!(stdout, b"hit\n");
}

#[test]
fn line_filter_treats_cr_and_nul_as_bytes_and_empty_needle_matches_physical_lines() {
    let (_, stdout, _) = run("x", b"a\r\n\0x\r\nlast\0x");
    assert_eq!(stdout, b"\0x\r\nlast\0x");

    let (outcome, stdout, _) = run("", b"a\n\nlast");
    assert_eq!(outcome, CommandEvaluationOutcome::ReturnedBool(true));
    assert_eq!(stdout, b"a\n\nlast");

    let (outcome, stdout, _) = run("", b"");
    assert_eq!(outcome, CommandEvaluationOutcome::ReturnedBool(false));
    assert!(stdout.is_empty());
}

#[test]
fn line_filter_false_and_usage_paths_are_exact() {
    let (outcome, stdout, stderr) = run("absent", b"one\ntwo\n");
    assert_eq!(outcome, CommandEvaluationOutcome::ReturnedBool(false));
    assert!(stdout.is_empty());
    assert!(stderr.is_empty());

    let result = execute_language_command(
        &resolved(),
        "spxgrep-lines.run",
        &HostedCommandInput::default(),
        200_000,
    )
    .unwrap();
    assert_eq!(
        result.evaluation.outcome,
        CommandEvaluationOutcome::ReturnedBool(false)
    );
    assert_eq!(result.stderr, b"usage: spxgrep-lines <needle>\n");
    assert!(result.stdout.is_empty());
}

#[test]
fn range_and_append_are_explicit_hir_not_replayed_as_generic_calls() {
    let program = resolved();
    let source = linked_source();
    let graph =
        semaprax::graph::to_json(&parse(&source, Path::new("spxgrep-lines-app.spx")).unwrap())
            .unwrap();
    assert!(graph.contains("\"schema\":\"semaprax.graph.v23\""));
    assert!(graph.contains("\"kind\":\"loan_plan\",\"schema\":\"semaprax.loan-plan.v1\""));
    assert!(graph.contains("\"kind\":\"byte_range\""));
    assert!(graph.contains("\"operation\":\"core.host.stdout-append\""));
    assert!(program
        .functions
        .iter()
        .any(|function| function.id.as_str() == "spxgrep-lines.run"));
}
