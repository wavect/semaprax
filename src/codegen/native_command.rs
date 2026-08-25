//! Fixed native process adapter for Project Useful Data Command v2.
//!
//! The semantic runner is deliberately separate from the process adapter. It
//! accepts two already bounded borrowed slices, stages output in invocation-
//! local memory, and publishes the transcript only after semantic success.
//! The process adapter owns argv/stdin decoding and the one physical flush.

use super::COutput;

pub(super) const RUN_SYMBOL: &str = "spx_native_command_run_v1";

pub(super) fn emit_runner(output: &mut impl COutput, command_symbol: &str) {
    writeln!(
        output,
        r#"struct spx_native_command_result_v1 {{
    bool matched;
    uint64_t transcript_length;
    uint8_t transcript[SPX_STDOUT_TRANSCRIPT_CAPACITY_V1];
}};

int {RUN_SYMBOL}(
    spx_slice_u8_v1 input,
    spx_slice_u8_v1 needle,
    struct spx_native_command_result_v1 *result_out
) {{
    if (result_out == NULL) return 0;
    memset(result_out, 0, sizeof(*result_out));
    if (input.len > SPX_STDOUT_TRANSCRIPT_CAPACITY_V1 ||
        needle.len > SPX_STDOUT_TRANSCRIPT_CAPACITY_V1 ||
        input.len > SPX_STDOUT_TRANSCRIPT_CAPACITY_V1 - needle.len ||
        (input.len == UINT64_C(0) ? input.ptr != NULL : input.ptr == NULL) ||
        (needle.len == UINT64_C(0) ? needle.ptr != NULL : needle.ptr == NULL)) {{
        return 0;
    }}

    struct spx_stdout_staging_v1 staging = {{0}};
    struct spx_status_entry spx_status_entries[UINT32_C(1)];
    struct spx_context spx_ctx = {{0}};
    if (!spx_context_init(
        &spx_ctx,
        UINT64_C(1),
        spx_status_entries,
        UINT32_C(1),
        NULL,
        NULL,
        &staging
    )) return 0;

    bool matched = false;
    spx_status_token status = {command_symbol}(&spx_ctx, input, needle, &matched);
    if (status != SPX_STATUS_SUCCESS) {{
        (void)spx_status_resolve(&spx_ctx, status);
        (void)spx_status_resolve_detail(&spx_ctx, status);
        memset(&staging, 0, sizeof(staging));
        return 0;
    }}
    if (staging.length > SPX_STDOUT_TRANSCRIPT_CAPACITY_V1) {{
        memset(&staging, 0, sizeof(staging));
        return 0;
    }}

    if (!matched) {{
        memset(&staging, 0, sizeof(staging));
        return 1;
    }}

    result_out->matched = true;
    if (staging.length != UINT64_C(0)) {{
        memcpy(result_out->transcript, staging.bytes, (size_t)staging.length);
    }}
    result_out->transcript_length = staging.length;
    memset(&staging, 0, sizeof(staging));
    return 1;
}}
"#
    )
    .expect("writing native command runner cannot fail");
}

pub(super) fn emit_process_adapter(output: &mut impl COutput) {
    output.push_str(
        r#"#if defined(_WIN32)
#include <fcntl.h>
#include <io.h>
#include <limits.h>
#include <windows.h>
#include <wchar.h>
#else
#include <signal.h>
#endif

static int spx_native_command_fail_v1(void) {
    static const char message[] = "SEMAPRAX native command failed\n";
    (void)fwrite(message, sizeof(char), sizeof(message) - 1u, stderr);
    return 2;
}

static __attribute__((unused)) bool spx_native_command_utf8_v1(
    const uint8_t *bytes,
    uint64_t length
) {
    uint64_t offset = UINT64_C(0);
    while (offset < length) {
        uint8_t first = bytes[offset];
        uint64_t width;
        if (first <= UINT8_C(0x7f)) {
            width = UINT64_C(1);
        } else if (first >= UINT8_C(0xc2) && first <= UINT8_C(0xdf)) {
            width = UINT64_C(2);
        } else if (first >= UINT8_C(0xe0) && first <= UINT8_C(0xef)) {
            width = UINT64_C(3);
        } else if (first >= UINT8_C(0xf0) && first <= UINT8_C(0xf4)) {
            width = UINT64_C(4);
        } else {
            return false;
        }
        if (width > length - offset) return false;
        if (width >= UINT64_C(2)) {
            uint8_t second = bytes[offset + UINT64_C(1)];
            if ((second & UINT8_C(0xc0)) != UINT8_C(0x80) ||
                (first == UINT8_C(0xe0) && second < UINT8_C(0xa0)) ||
                (first == UINT8_C(0xed) && second > UINT8_C(0x9f)) ||
                (first == UINT8_C(0xf0) && second < UINT8_C(0x90)) ||
                (first == UINT8_C(0xf4) && second > UINT8_C(0x8f))) {
                return false;
            }
        }
        for (uint64_t tail = UINT64_C(2); tail < width; ++tail) {
            if ((bytes[offset + tail] & UINT8_C(0xc0)) != UINT8_C(0x80)) {
                return false;
            }
        }
        offset += width;
    }
    return true;
}

static bool spx_native_command_read_stdin_v1(
    uint8_t *bytes,
    uint64_t capacity,
    uint64_t *length_out
) {
    if (bytes == NULL || length_out == NULL || capacity > UINT64_C(65536)) {
        return false;
    }
    uint64_t length = UINT64_C(0);
    while (length < capacity) {
        size_t count = fread(
            bytes + (size_t)length,
            sizeof(uint8_t),
            (size_t)(capacity - length),
            stdin
        );
        if (count != 0u) {
            length += (uint64_t)count;
            continue;
        }
        if (feof(stdin)) {
            *length_out = length;
            return true;
        }
        return false;
    }
    int probe = fgetc(stdin);
    if (probe != EOF) return false;
    if (ferror(stdin)) return false;
    *length_out = length;
    return true;
}

static int spx_native_command_execute_v1(
    uint8_t *arena,
    uint64_t needle_length
) {
    if (arena == NULL || needle_length > UINT64_C(65536)) {
        return spx_native_command_fail_v1();
    }
    uint64_t input_length = UINT64_C(0);
    if (!spx_native_command_read_stdin_v1(
        arena + (size_t)needle_length,
        UINT64_C(65536) - needle_length,
        &input_length
    )) {
        return spx_native_command_fail_v1();
    }
    spx_slice_u8_v1 input = {
        .ptr = input_length == UINT64_C(0) ? NULL : arena + (size_t)needle_length,
        .len = input_length
    };
    spx_slice_u8_v1 needle = {
        .ptr = needle_length == UINT64_C(0) ? NULL : arena,
        .len = needle_length
    };
    struct spx_native_command_result_v1 result;
    if (!spx_native_command_run_v1(input, needle, &result)) {
        return spx_native_command_fail_v1();
    }
    if (result.transcript_length != UINT64_C(0) &&
        fwrite(
            result.transcript,
            sizeof(uint8_t),
            (size_t)result.transcript_length,
            stdout
        ) != (size_t)result.transcript_length) {
        memset(&result, 0, sizeof(result));
        return spx_native_command_fail_v1();
    }
    if (fflush(stdout) != 0) {
        memset(&result, 0, sizeof(result));
        return spx_native_command_fail_v1();
    }
    int exit_code = result.matched ? 0 : 1;
    memset(&result, 0, sizeof(result));
    return exit_code;
}

#if defined(_WIN32)
int wmain(int argc, wchar_t **argv) {
    uint8_t arena[UINT32_C(65536)];
    if (_setmode(_fileno(stderr), _O_BINARY) == -1) return 2;
    if (argc != 2 || argv == NULL || argv[1] == NULL ||
        _setmode(_fileno(stdin), _O_BINARY) == -1 ||
        _setmode(_fileno(stdout), _O_BINARY) == -1) {
        return spx_native_command_fail_v1();
    }
    size_t wide_length = UINT32_C(0);
    while (wide_length <= UINT32_C(65536) && argv[1][wide_length] != L'\0') {
        ++wide_length;
    }
    if (wide_length > UINT32_C(65536) || wide_length > (size_t)INT_MAX) {
        return spx_native_command_fail_v1();
    }
    int needle_length = 0;
    if (wide_length != 0u) {
        needle_length = WideCharToMultiByte(
            CP_UTF8,
            WC_ERR_INVALID_CHARS,
            argv[1],
            (int)wide_length,
            (char *)arena,
            65536,
            NULL,
            NULL
        );
        if (needle_length <= 0) return spx_native_command_fail_v1();
    }
    return spx_native_command_execute_v1(arena, (uint64_t)needle_length);
}
#else
int main(int argc, char **argv) {
    uint8_t arena[UINT32_C(65536)];
    if (signal(SIGPIPE, SIG_IGN) == SIG_ERR ||
        argc != 2 || argv == NULL || argv[1] == NULL) {
        return spx_native_command_fail_v1();
    }
    uint64_t needle_length = UINT64_C(0);
    while (needle_length <= UINT64_C(65536) && argv[1][needle_length] != '\0') {
        ++needle_length;
    }
    if (needle_length > UINT64_C(65536) ||
        !spx_native_command_utf8_v1((const uint8_t *)argv[1], needle_length)) {
        return spx_native_command_fail_v1();
    }
    if (needle_length != UINT64_C(0)) {
        memcpy(arena, argv[1], (size_t)needle_length);
    }
    return spx_native_command_execute_v1(arena, needle_length);
}
#endif
"#,
    );
}

#[cfg(test)]
mod tests {
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
        let executable =
            std::env::temp_dir().join(format!("{stem}{}", std::env::consts::EXE_SUFFIX));
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
        let diagnostic = super::super::emit_hir_c_with_native_command(
            &resolved(&wrong_signature),
            "test.command",
        )
        .unwrap_err();
        assert_eq!(diagnostic.code, "SPX-W121");

        let diagnostic =
            super::super::emit_hir_c_with_native_command(&resolved(SOURCE), "test.absent")
                .unwrap_err();
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
        let output = run(&executable, invalid, b"payload");
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
}
