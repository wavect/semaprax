use std::io::Write;
use std::path::Path;
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicU64, Ordering};

use semaprax::hosted_interpreter::{execute_language_command, HostedCommandInput};
use semaprax::interpreter::CommandEvaluationOutcome;
use semaprax::{codegen, hir, parse, verify};

static NEXT_ID: AtomicU64 = AtomicU64::new(0);

const SOURCE: &str = r#"
module test.language_command;

permit { process.args.read, process.stderr.write, process.stdin.read, process.stdout.write }

@id("command.run")
fn run() -> bool
    uses { process.args.read, process.stderr.write, process.stdin.read, process.stdout.write }
{
    if args_len() == 1usize {
        let argument = arg_utf8(0usize);
        let argument_bytes = str_as_bytes(argument);
        let input = stdin_read();
        let input_bytes = bytes_as_slice(input);
        let stderr_count = stderr_write(argument_bytes);
        let stdout_count = stdout_write(input_bytes);
        stderr_count == byte_len(argument_bytes) && stdout_count == byte_len(input_bytes)
    } else {
        let usage = [117u8, 115u8, 97u8, 103u8, 101u8, 10u8];
        let usage_bytes = array_as_slice(usage);
        stderr_write(usage_bytes) == byte_len(usage_bytes) && false
    }
}

@id("main")
fn main() -> i64 { 0 }
"#;

fn resolved(source: &str) -> hir::ResolvedProgram {
    let ast = parse(source, Path::new("language-command.spx")).unwrap();
    let diagnostics = verify::verify(&ast);
    assert!(diagnostics.is_empty(), "{diagnostics:?}");
    hir::resolve(&ast).unwrap()
}

#[test]
fn hosted_input_is_exact_and_false_still_seals_both_channels() {
    let program = resolved(SOURCE);
    let input = HostedCommandInput {
        arguments: vec!["needle".to_owned()],
        stdin: vec![0, 1, 2, 255],
    };
    let result = execute_language_command(&program, "command.run", &input, 10_000).unwrap();
    assert_eq!(
        result.evaluation.outcome,
        CommandEvaluationOutcome::ReturnedBool(true)
    );
    assert_eq!(result.stderr, b"needle");
    assert_eq!(result.stdout, [0, 1, 2, 255]);

    let false_result = execute_language_command(
        &program,
        "command.run",
        &HostedCommandInput::default(),
        10_000,
    )
    .unwrap();
    assert_eq!(
        false_result.evaluation.outcome,
        CommandEvaluationOutcome::ReturnedBool(false)
    );
    assert!(false_result.stdout.is_empty());
    assert_eq!(false_result.stderr, b"usage\n");
}

#[test]
fn normalized_input_failure_discards_both_transcripts() {
    let failed = SOURCE.replace("arg_utf8(0usize)", "arg_utf8(1usize)");
    let result = execute_language_command(
        &resolved(&failed),
        "command.run",
        &HostedCommandInput {
            arguments: vec!["present".to_owned()],
            stdin: b"must-not-publish".to_vec(),
        },
        10_000,
    )
    .unwrap();
    let CommandEvaluationOutcome::LanguageFailure(status) = result.evaluation.outcome else {
        panic!("expected normalized command-input failure");
    };
    assert_eq!(status.domain_id(), "semaprax.command-input.v1");
    assert_eq!(status.code(), 1);
    assert!(result.stdout.is_empty());
    assert!(result.stderr.is_empty());
}

#[test]
fn hosted_snapshot_limits_fail_before_entry() {
    let program = resolved(SOURCE);
    let too_many = HostedCommandInput {
        arguments: vec![String::new(); 17],
        stdin: Vec::new(),
    };
    assert!(execute_language_command(&program, "command.run", &too_many, 10_000).is_err());
    let too_large = HostedCommandInput {
        arguments: vec!["x".to_owned()],
        stdin: vec![0; 65_536],
    };
    assert!(execute_language_command(&program, "command.run", &too_large, 10_000).is_err());
}

#[test]
fn native_command_selection_requires_an_explicit_stable_identity() {
    let automatic = SOURCE.replacen("@id(\"command.run\")\n", "", 1);
    let ast = parse(&automatic, Path::new("automatic-language-command.spx")).unwrap();
    let program = hir::resolve(&ast).unwrap();
    let command_id = program
        .functions
        .iter()
        .find(|function| function.name == "run")
        .unwrap()
        .id
        .as_str()
        .to_owned();
    let error = codegen::emit_hir_c_with_language_command_io(&program, &command_id).unwrap_err();
    assert!(error.message.contains("explicit stable-ID"), "{error:?}");
}

#[test]
fn native_o0_o2_process_adapter_round_trips_exact_binary_input() {
    if Command::new("clang").arg("--version").output().is_err() {
        return;
    }
    let generated =
        codegen::emit_hir_c_with_language_command_io(&resolved(SOURCE), "command.run").unwrap();
    assert!(generated.contains("spx_language_command_run_v1"));
    assert!(generated.contains("int wmain(int argc, wchar_t **argv)"));
    assert!(generated.contains("int main(int argc, char **argv)"));
    assert!(!generated.contains("getenv("));

    for optimization in ["-O0", "-O2"] {
        let id = NEXT_ID.fetch_add(1, Ordering::Relaxed);
        let stem = format!(
            "semaprax-language-command-{}-{id}-{optimization}",
            std::process::id()
        );
        let source = std::env::temp_dir().join(format!("{stem}.c"));
        let executable =
            std::env::temp_dir().join(format!("{stem}{}", std::env::consts::EXE_SUFFIX));
        std::fs::write(&source, &generated).unwrap();
        let compiled = Command::new("clang")
            .args(["-std=c11", optimization, "-Wall", "-Wextra", "-Werror"])
            .arg(&source)
            .arg("-o")
            .arg(&executable)
            .output()
            .unwrap();
        assert!(
            compiled.status.success(),
            "{}",
            String::from_utf8_lossy(&compiled.stderr)
        );

        let mut child = Command::new(&executable)
            .arg("needle")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .unwrap();
        child
            .stdin
            .take()
            .unwrap()
            .write_all(&[0, 1, 2, 255])
            .unwrap();
        let output = child.wait_with_output().unwrap();
        assert_eq!(output.status.code(), Some(0));
        assert_eq!(output.stdout, [0, 1, 2, 255]);
        assert_eq!(output.stderr, b"needle");

        let false_output = Command::new(&executable).output().unwrap();
        assert_eq!(false_output.status.code(), Some(1));
        assert!(false_output.stdout.is_empty());
        assert_eq!(false_output.stderr, b"usage\n");

        let mut oversized = Command::new(&executable)
            .arg("needle")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .unwrap();
        oversized
            .stdin
            .take()
            .unwrap()
            .write_all(&vec![0; 65_537])
            .unwrap();
        let oversized = oversized.wait_with_output().unwrap();
        assert_eq!(oversized.status.code(), Some(2));
        assert!(oversized.stdout.is_empty());
        assert_eq!(oversized.stderr, b"SEMAPRAX language command failed\n");

        let _ = std::fs::remove_file(source);
        let _ = std::fs::remove_file(executable);
    }
}
