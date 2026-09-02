use std::io::Write as _;
use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};
use std::sync::atomic::{AtomicU64, Ordering};

use crate::{hir, parse, verify};

static NEXT_ID: AtomicU64 = AtomicU64::new(0);

const SOURCE: &str = r#"
module test.native_command;

permit { process.stdout.write }

@id("test.command")
fn command(input: borrow Slice<u8>, needle: borrow Slice<u8>) -> bool
uses { process.stdout.write }
{
let found = if byte_len(needle) == 0usize {
    true
} else {
    match byte_get(needle, 0usize) {
        Option::Some { value: needle_byte } => match byte_get(input, 0usize) {
            Option::Some { value: input_byte } => input_byte == needle_byte,
            Option::None {} => false,
        },
        Option::None {} => false,
    }
};
if found {
    let written = stdout_write(input);
    written == byte_len(input)
} else {
    false
}
}

@id("main")
fn main() -> i64 { 0 }
"#;

fn resolved(source: &str) -> hir::ResolvedProgram {
    let ast = parse(source, Path::new("native-command.spx")).unwrap();
    let diagnostics = verify::verify(&ast);
    assert!(diagnostics.is_empty(), "{diagnostics:?}");
    hir::resolve(&ast).unwrap()
}

fn generated(source: &str) -> String {
    super::super::emit_hir_c_with_native_command(&resolved(source), "test.command").unwrap()
}

fn compile(source: &str, optimization: &str) -> Option<PathBuf> {
    if Command::new("clang").arg("--version").output().is_err() {
        return None;
    }
    let id = NEXT_ID.fetch_add(1, Ordering::Relaxed);
    let stem = format!(
        "semaprax-native-command-{}-{id}-{optimization}",
        std::process::id()
    );
    let c_path = std::env::temp_dir().join(format!("{stem}.c"));
    let executable = std::env::temp_dir().join(format!("{stem}{}", std::env::consts::EXE_SUFFIX));
    std::fs::write(&c_path, source).unwrap();
    let mut compiler = Command::new("clang");
    compiler.args([
        "-std=c11",
        "-pedantic",
        "-Wall",
        "-Wextra",
        "-Werror",
        optimization,
    ]);
    #[cfg(all(windows, target_env = "gnu"))]
    compiler.arg("-municode");
    let compiled = compiler
        .arg(&c_path)
        .arg("-o")
        .arg(&executable)
        .output()
        .unwrap();
    let _ = std::fs::remove_file(c_path);
    assert!(
        compiled.status.success(),
        "{}",
        String::from_utf8_lossy(&compiled.stderr)
    );
    Some(executable)
}

fn run(executable: &Path, needle: &std::ffi::OsStr, input: &[u8]) -> Output {
    let mut child = Command::new(executable)
        .arg(needle)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    child.stdin.take().unwrap().write_all(input).unwrap();
    child.wait_with_output().unwrap()
}

#[test]
fn shared_plan_rejects_wrong_signature_selection_contract_and_authority() {
    let wrong_signature = SOURCE
        .replace(
            "input: borrow Slice<u8>, needle: borrow Slice<u8>",
            "input: borrow Slice<u8>",
        )
        .replace("byte_len(needle) == 0usize", "byte_len(input) == 0usize")
        .replace("byte_get(needle, 0usize)", "byte_get(input, 0usize)");
    let diagnostic =
        super::super::emit_hir_c_with_native_command(&resolved(&wrong_signature), "test.command")
            .unwrap_err();
    assert_eq!(diagnostic.code, "SPX-W121");

    let diagnostic =
        super::super::emit_hir_c_with_native_command(&resolved(SOURCE), "test.absent").unwrap_err();
    assert_eq!(diagnostic.code, "SPX-W121");

    let with_contract = SOURCE.replace(
        "uses { process.stdout.write }",
        "uses { process.stdout.write }\n    requires true",
    );
    let diagnostic =
        super::super::emit_hir_c_with_native_command(&resolved(&with_contract), "test.command")
            .unwrap_err();
    assert_eq!(diagnostic.code, "SPX-W121");

    let widened = SOURCE.replace(
        "permit { process.stdout.write }",
        "permit { process.stdout.write, process.network }",
    );
    let diagnostic =
        super::super::emit_hir_c_with_native_command(&resolved(&widened), "test.command")
            .unwrap_err();
    assert_eq!(diagnostic.code, "SPX-T269");
}

#[test]
fn projection_has_one_fixed_process_entry_and_no_legacy_failure_path() {
    let c = generated(SOURCE);
    assert!(c.contains("int spx_native_command_run_v1("));
    assert!(c.contains("int wmain(int argc, wchar_t **argv)"));
    assert!(c.contains("int main(int argc, char **argv)"));
    assert!(c.contains("WC_ERR_INVALID_CHARS"));
    assert!(c.contains("_setmode(_fileno(stdin), _O_BINARY)"));
    assert!(c.contains("_setmode(_fileno(stderr), _O_BINARY)"));
    assert!(c.contains("int probe = fgetc(stdin);"));
    assert_eq!(c.matches("SEMAPRAX native command failed\\n").count(), 1);
    assert!(!c.contains("spx_public_failure"));
    assert!(!c.contains("int main(void)"));
    assert!(!c.contains("printf(\"%lld\\n\""));
    let unix_main = c
        .split_once("#else\nint main(int argc, char **argv) {")
        .and_then(|(_, tail)| tail.split_once("\n}\n#endif"))
        .map(|(body, _)| body)
        .expect("generated Unix native-command main");
    let argv_validation = unix_main
        .find("!spx_native_command_utf8_v1")
        .expect("Unix argv UTF-8 validation");
    let semantic_execute = unix_main
        .find("return spx_native_command_execute_v1")
        .expect("Unix semantic execution handoff");
    assert!(argv_validation < semantic_execute);
    assert!(!unix_main.contains("spx_native_command_read_stdin_v1"));
}

#[test]
fn unix_o0_o2_process_adapter_seals_output_and_enforces_exact_boundaries() {
    for optimization in ["-O0", "-O2"] {
        let Some(executable) = compile(&generated(SOURCE), optimization) else {
            return;
        };

        let matched = run(&executable, std::ffi::OsStr::new("a"), b"a\0b");
        assert_eq!(matched.status.code(), Some(0));
        assert_eq!(matched.stdout, b"a\0b");
        assert!(matched.stderr.is_empty());

        let absent = run(&executable, std::ffi::OsStr::new("z"), b"a\0b");
        assert_eq!(absent.status.code(), Some(1));
        assert!(absent.stdout.is_empty());
        assert!(absent.stderr.is_empty());

        let exact_input = vec![b'a'; 65_535];
        let exact = run(&executable, std::ffi::OsStr::new("a"), &exact_input);
        assert_eq!(exact.status.code(), Some(0));
        assert_eq!(exact.stdout, exact_input);
        assert!(exact.stderr.is_empty());

        let overflow_input = vec![b'a'; 65_536];
        let overflow = run(&executable, std::ffi::OsStr::new("a"), &overflow_input);
        assert_eq!(overflow.status.code(), Some(2));
        assert!(overflow.stdout.is_empty());
        assert_eq!(overflow.stderr, b"SEMAPRAX native command failed\n");

        let mut broken_pipe = Command::new(&executable)
            .arg("a")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .unwrap();
        drop(broken_pipe.stdout.take());
        broken_pipe
            .stdin
            .take()
            .unwrap()
            .write_all(&[b'a'; 4_096])
            .unwrap();
        let broken_pipe = broken_pipe.wait_with_output().unwrap();
        assert_eq!(broken_pipe.status.code(), Some(2));
        assert_eq!(broken_pipe.stderr, b"SEMAPRAX native command failed\n");

        let _ = std::fs::remove_file(executable);
    }
}

#[cfg(unix)]
#[test]
fn unix_rejects_non_utf8_needle_before_semantic_execution() {
    use std::os::unix::ffi::OsStrExt as _;

    let Some(executable) = compile(&generated(SOURCE), "-O0") else {
        return;
    };
    let invalid = std::ffi::OsStr::from_bytes(&[0xff]);
    let output = Command::new(&executable)
        .arg(invalid)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(2));
    assert!(output.stdout.is_empty());
    assert_eq!(output.stderr, b"SEMAPRAX native command failed\n");
    let _ = std::fs::remove_file(executable);
}

#[test]
fn semantic_failure_after_write_discards_staged_transcript() {
    let failed_source = SOURCE.replace(
        "written == byte_len(input)",
        "{ let impossible = 1 / 0; written == byte_len(input) && impossible == 0 }",
    );
    let Some(executable) = compile(&generated(&failed_source), "-O0") else {
        return;
    };
    let output = run(&executable, std::ffi::OsStr::new("a"), b"abc");
    assert_eq!(output.status.code(), Some(2));
    assert!(output.stdout.is_empty());
    assert_eq!(output.stderr, b"SEMAPRAX native command failed\n");
    let _ = std::fs::remove_file(executable);
}

#[test]
fn semantic_false_after_write_publishes_no_transcript_and_exits_one() {
    let false_source = SOURCE.replace("written == byte_len(input)", "written == 0usize");
    for optimization in ["-O0", "-O2"] {
        let Some(executable) = compile(&generated(&false_source), optimization) else {
            return;
        };
        let output = run(&executable, std::ffi::OsStr::new("a"), b"abc");
        assert_eq!(output.status.code(), Some(1));
        assert!(output.stdout.is_empty());
        assert!(output.stderr.is_empty());
        let _ = std::fs::remove_file(executable);
    }
}
