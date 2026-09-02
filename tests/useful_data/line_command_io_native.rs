use std::io::Write;
use std::path::Path;
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicU64, Ordering};

use semaprax::hosted_interpreter::{execute_language_command, HostedCommandInput};
use semaprax::interpreter::CommandEvaluationOutcome;
use semaprax::{codegen, hir, parse, verify};

static NEXT_ID: AtomicU64 = AtomicU64::new(0);

const SOURCE: &str = r#"
module test.line_command;

permit { process.args.read, process.stderr.write, process.stdin.read, process.stdout.write }

@id("line.run")
fn run() -> bool
    uses { process.stdin.read, process.stdout.write }
{
    let owned = stdin_read();
    let input = bytes_as_slice(owned);
    let view = byte_range(input, 0usize, byte_len(input));
    let first = stdout_append(view);
    let second = stdout_append(view);
    first == byte_len(view) && second == byte_len(view)
}

@id("main")
fn main() -> i64 { 0 }
"#;

fn resolved(source: &str) -> hir::ResolvedProgram {
    let ast = parse(source, Path::new("line-command.spx")).unwrap();
    let diagnostics = verify::verify(&ast);
    assert!(diagnostics.is_empty(), "{diagnostics:?}");
    hir::resolve(&ast).unwrap()
}

#[test]
fn interpreter_range_and_cumulative_append_have_exact_failure_domains() {
    let program = resolved(SOURCE);
    let exact_input = vec![0x5a; 32_768];
    let exact = execute_language_command(
        &program,
        "line.run",
        &HostedCommandInput {
            arguments: Vec::new(),
            stdin: exact_input.clone(),
        },
        50_000,
    )
    .unwrap();
    assert_eq!(
        exact.evaluation.outcome,
        CommandEvaluationOutcome::ReturnedBool(true)
    );
    assert_eq!(exact.stdout.len(), 65_536);
    assert_eq!(&exact.stdout[..32_768], exact_input);
    assert_eq!(&exact.stdout[32_768..], exact_input);
    assert!(exact.stderr.is_empty());

    let overflow = execute_language_command(
        &program,
        "line.run",
        &HostedCommandInput {
            arguments: Vec::new(),
            stdin: vec![0x5a; 32_769],
        },
        50_000,
    )
    .unwrap();
    let CommandEvaluationOutcome::LanguageFailure(status) = overflow.evaluation.outcome else {
        panic!("expected cumulative output failure");
    };
    assert_eq!(status.domain_id(), "semaprax.command-output.v1");
    assert_eq!(status.code(), 1);
    assert!(overflow.stdout.is_empty());
    assert!(overflow.stderr.is_empty());

    let out_of_bounds = SOURCE.replacen(
        "byte_range(input, 0usize, byte_len(input))",
        "byte_range(input, 0usize, byte_len(input) + 1usize)",
        1,
    );
    let failed = execute_language_command(
        &resolved(&out_of_bounds),
        "line.run",
        &HostedCommandInput {
            arguments: Vec::new(),
            stdin: vec![1, 2, 3],
        },
        50_000,
    )
    .unwrap();
    let CommandEvaluationOutcome::LanguageFailure(status) = failed.evaluation.outcome else {
        panic!("expected byte-range failure");
    };
    assert_eq!(status.domain_id(), "semaprax.byte-range.v1");
    assert_eq!(status.code(), 2);
    assert!(failed.stdout.is_empty());
}

#[test]
fn native_o0_o2_accept_exact_output_capacity_and_discard_overflow() {
    if Command::new("clang").arg("--version").output().is_err() {
        return;
    }
    let generated = codegen::emit_hir_c_with_line_command_io(&resolved(SOURCE), "line.run")
        .expect("line command must lower to native C11");
    assert!(generated.contains("spx_byte_range_v1"));
    assert!(generated.contains("spx_host_command_stdout_append_v1"));
    assert!(generated.contains("semaprax.command-output.v1"));

    for optimization in ["-O0", "-O2"] {
        let id = NEXT_ID.fetch_add(1, Ordering::Relaxed);
        let stem = format!(
            "semaprax-line-command-{}-{id}-{optimization}",
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

        let mut exact = Command::new(&executable)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .unwrap();
        exact
            .stdin
            .take()
            .unwrap()
            .write_all(&vec![0x5a; 32_768])
            .unwrap();
        let exact = exact.wait_with_output().unwrap();
        assert_eq!(exact.status.code(), Some(0));
        assert_eq!(exact.stdout, vec![0x5a; 65_536]);
        assert!(exact.stderr.is_empty());

        let mut overflow = Command::new(&executable)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .unwrap();
        overflow
            .stdin
            .take()
            .unwrap()
            .write_all(&vec![0x5a; 32_769])
            .unwrap();
        let overflow = overflow.wait_with_output().unwrap();
        assert_eq!(overflow.status.code(), Some(2));
        assert!(overflow.stdout.is_empty());
        assert_eq!(overflow.stderr, b"SEMAPRAX language command failed\n");

        let _ = std::fs::remove_file(source);
        let _ = std::fs::remove_file(executable);
    }
}
