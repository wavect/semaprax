//! Native C11 callback ABI coverage for Filesystem I/O v1.

use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicU64, Ordering};

use semaprax::{codegen, hir, parse, verify};

static NEXT_ID: AtomicU64 = AtomicU64::new(0);

const SOURCE: &str = r#"
module test.filesystem_native;
permit { fs.read, fs.write }
@id("filesystem-native.run")
fn run() -> bool uses { fs.read, fs.write } {
    let input = [105u8, 110u8];
    let bytes = file_read(array_as_slice(input), 2usize, 4usize);
    let output = [111u8, 117u8, 116u8];
    let view = bytes_as_slice(bytes);
    file_write_new(array_as_slice(output), 3usize, view, byte_len(view)) == 4usize
}
@id("main") fn main() -> i64 { 0 }
"#;

const INVALID_PATH_SOURCE: &str = r#"
module test.filesystem_native_invalid_path;
permit { fs.read }
@id("filesystem-native-invalid.run")
fn run() -> bool uses { fs.read } {
    let path = [47u8];
    let bytes = file_read(array_as_slice(path), 1usize, 1usize);
    byte_len(bytes_as_slice(bytes)) == 0usize
}
@id("main") fn main() -> i64 { 0 }
"#;

const HARNESS: &str = r#"
static uint32_t fixture_read(void *context, spx_slice_u8_v1 path, uint64_t length,
    uint8_t *destination, uint64_t capacity, uint64_t *written) {
    uint64_t *calls = (uint64_t *)context; *calls += UINT64_C(1);
    if (length != UINT64_C(2) || capacity != UINT64_C(4) || destination == NULL || written == NULL ||
        path.ptr[0] != UINT8_C(105) || path.ptr[1] != UINT8_C(110)) return UINT32_C(5);
    destination[0] = UINT8_C(80); destination[1] = UINT8_C(73);
    destination[2] = UINT8_C(78); destination[3] = UINT8_C(71); *written = UINT64_C(4); return 0;
}
static uint32_t fixture_write(void *context, spx_slice_u8_v1 path, uint64_t length,
    spx_slice_u8_v1 data, uint64_t data_length, uint64_t *written) {
    uint64_t *calls = (uint64_t *)context; *calls += UINT64_C(1);
    if (length != UINT64_C(3) || data_length != UINT64_C(4) || written == NULL ||
        path.ptr[0] != UINT8_C(111) || path.ptr[1] != UINT8_C(117) || path.ptr[2] != UINT8_C(116) ||
        data.ptr[0] != UINT8_C(80) || data.ptr[1] != UINT8_C(73) || data.ptr[2] != UINT8_C(78) || data.ptr[3] != UINT8_C(71)) return UINT32_C(5);
    *written = data_length; return 0;
}
static void fixture_settle(void *context) { *(uint64_t *)context += UINT64_C(1); }
int main(void) {
    uint64_t calls = UINT64_C(0);
    struct spx_filesystem_callbacks_v1 callbacks = { .context = &calls, .read = fixture_read, .write_new = fixture_write, .settle = fixture_settle };
    struct spx_filesystem_command_result_v1 result;
    int invoked = spx_run_filesystem_command_v1(&callbacks, &result);
    return invoked == 1 && result.semantic_success && result.matched && calls == UINT64_C(3) ? 0 : 1;
}
"#;

const FAILURE_HARNESS: &str = r#"
static uint32_t refused_read(void *context, spx_slice_u8_v1 path, uint64_t length,
    uint8_t *destination, uint64_t capacity, uint64_t *written) {
    (void)path; (void)length; (void)destination; (void)capacity; (void)written;
    *(uint64_t *)context += UINT64_C(1); return UINT32_C(2);
}
static uint32_t unexpected_write(void *context, spx_slice_u8_v1 path, uint64_t length,
    spx_slice_u8_v1 data, uint64_t data_length, uint64_t *written) {
    (void)path; (void)length; (void)data; (void)data_length; (void)written;
    *(uint64_t *)context += UINT64_C(100); return UINT32_C(5);
}
static void refused_settle(void *context) { *(uint64_t *)context += UINT64_C(1); }
int main(void) {
    uint64_t calls = UINT64_C(0);
    struct spx_filesystem_callbacks_v1 callbacks = { .context = &calls, .read = refused_read, .write_new = unexpected_write, .settle = refused_settle };
    struct spx_filesystem_command_result_v1 result;
    int invoked = spx_run_filesystem_command_v1(&callbacks, &result);
    return invoked == 1 && !result.semantic_success && result.status_code == UINT32_C(2) &&
        calls == UINT64_C(2) ? 0 : 1;
}
"#;

const INVALID_COUNT_HARNESS: &str = r#"
static uint32_t invalid_count_read(void *context, spx_slice_u8_v1 path, uint64_t length,
    uint8_t *destination, uint64_t capacity, uint64_t *written) {
    (void)path; (void)length; (void)destination; *(uint64_t *)context += UINT64_C(1);
    *written = capacity + UINT64_C(1); return UINT32_C(0);
}
static void invalid_count_settle(void *context) { *(uint64_t *)context += UINT64_C(1); }
int main(void) {
    uint64_t calls = UINT64_C(0);
    struct spx_filesystem_callbacks_v1 callbacks = { .context = &calls, .read = invalid_count_read, .write_new = NULL, .settle = invalid_count_settle };
    struct spx_filesystem_command_result_v1 result;
    int invoked = spx_run_filesystem_command_v1(&callbacks, &result);
    return invoked == 1 && !result.semantic_success && result.status_code == UINT32_C(4) &&
        calls == UINT64_C(2) ? 0 : 1;
}
"#;

const INVALID_PATH_HARNESS: &str = r#"
static uint32_t must_not_read(void *context, spx_slice_u8_v1 path, uint64_t length,
    uint8_t *destination, uint64_t capacity, uint64_t *written) {
    (void)path; (void)length; (void)destination; (void)capacity; (void)written;
    *(uint64_t *)context += UINT64_C(100); return UINT32_C(5);
}
static void invalid_path_settle(void *context) { *(uint64_t *)context += UINT64_C(1); }
int main(void) {
    uint64_t calls = UINT64_C(0);
    struct spx_filesystem_callbacks_v1 callbacks = { .context = &calls, .read = must_not_read, .write_new = NULL, .settle = invalid_path_settle };
    struct spx_filesystem_command_result_v1 result;
    int invoked = spx_run_filesystem_command_v1(&callbacks, &result);
    return invoked == 1 && !result.semantic_success && result.status_code == UINT32_C(1) && calls == UINT64_C(1) ? 0 : 1;
}
"#;

const CUMULATIVE_HARNESS: &str = r#"
static uint32_t counted_write(void *context, spx_slice_u8_v1 path, uint64_t path_length,
    spx_slice_u8_v1 data, uint64_t data_length, uint64_t *written) {
    (void)path; (void)path_length; (void)data; *(uint64_t *)context += UINT64_C(1);
    *written = data_length; return UINT32_C(0);
}
int main(void) {
    static const uint8_t path_bytes[] = { UINT8_C(102) };
    static const uint8_t data_bytes[65536] = { UINT8_C(0) };
    uint64_t calls = UINT64_C(0), result = UINT64_C(0);
    struct spx_filesystem_callbacks_v1 callbacks = { .context = &calls, .read = NULL, .write_new = counted_write, .settle = NULL };
    struct spx_filesystem_command_state_v1 state = { .callbacks = &callbacks };
    struct spx_status_entry entries[UINT32_C(1)]; struct spx_context context = {0};
    if (!spx_context_init(&context, UINT64_C(1), entries, UINT32_C(1), NULL, NULL, &state)) return 1;
    spx_slice_u8_v1 path = { .ptr = path_bytes, .len = UINT64_C(1) };
    spx_slice_u8_v1 data = { .ptr = data_bytes, .len = UINT64_C(65536) };
    for (uint64_t index = UINT64_C(0); index < UINT64_C(16); ++index) {
        if (spx_host_file_write_new_v1(&context, path, UINT64_C(1), data, UINT64_C(65536), &result) != SPX_STATUS_SUCCESS || result != UINT64_C(65536)) return 1;
    }
    spx_status_token failed = spx_host_file_write_new_v1(&context, path, UINT64_C(1), data, UINT64_C(65536), &result);
    const struct spx_normalized_status *status = spx_status_resolve(&context, failed);
    return calls == UINT64_C(16) && status != NULL && status->code == UINT32_C(4) ? 0 : 1;
}
"#;

fn resolved() -> hir::ResolvedProgram {
    let ast = parse(SOURCE, Path::new("filesystem-native.spx")).unwrap();
    let diagnostics = verify::verify(&ast);
    assert!(diagnostics.is_empty(), "{diagnostics:?}");
    hir::resolve(&ast).unwrap()
}

fn resolved_invalid_path() -> hir::ResolvedProgram {
    let ast = parse(
        INVALID_PATH_SOURCE,
        Path::new("filesystem-native-invalid-path.spx"),
    )
    .unwrap();
    let diagnostics = verify::verify(&ast);
    assert!(diagnostics.is_empty(), "{diagnostics:?}");
    hir::resolve(&ast).unwrap()
}

fn clang_available() -> bool {
    Command::new("clang").arg("--version").output().is_ok()
}

struct Compiled {
    source: PathBuf,
    executable: PathBuf,
}

impl Compiled {
    fn build(source: &str) -> Self {
        let id = NEXT_ID.fetch_add(1, Ordering::Relaxed);
        let stem = format!("semaprax-filesystem-native-{}-{id}", std::process::id());
        let source_path = std::env::temp_dir().join(format!("{stem}.c"));
        let executable =
            std::env::temp_dir().join(format!("{stem}{}", std::env::consts::EXE_SUFFIX));
        std::fs::write(&source_path, source).unwrap();
        let output = Command::new("clang")
            .args(["-std=c11", "-Wall", "-Wextra", "-Werror"])
            .arg(&source_path)
            .arg("-o")
            .arg(&executable)
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        Self {
            source: source_path,
            executable,
        }
    }
}

impl Drop for Compiled {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.source);
        let _ = std::fs::remove_file(&self.executable);
    }
}

#[test]
fn filesystem_profile_is_callback_only_and_runs_read_then_write() {
    if !clang_available() {
        return;
    }
    let generated =
        codegen::emit_hir_c_with_filesystem_io(&resolved(), "filesystem-native.run").unwrap();
    for required in [
        "spx_run_filesystem_command_v1",
        "struct spx_filesystem_callbacks_v1",
        "spx_host_file_read_v1",
        "spx_host_file_write_new_v1",
        "spx_filesystem_charge_v1",
        "SPX_FILESYSTEM_STATUS_DOMAIN_V1 \"semaprax.filesystem.v1\"",
    ] {
        assert!(generated.contains(required), "missing {required}");
    }
    for forbidden in ["open(", "openat(", "fopen(", "getcwd(", "chdir("] {
        assert!(
            !generated.contains(forbidden),
            "ambient filesystem text: {forbidden}"
        );
    }
    let compiled = Compiled::build(&format!("{generated}\n{HARNESS}"));
    assert!(Command::new(&compiled.executable)
        .status()
        .unwrap()
        .success());
}

#[test]
fn filesystem_callback_failures_select_closed_status_and_settle_once() {
    if !clang_available() {
        return;
    }
    let generated =
        codegen::emit_hir_c_with_filesystem_io(&resolved(), "filesystem-native.run").unwrap();
    for harness in [FAILURE_HARNESS, INVALID_COUNT_HARNESS] {
        let compiled = Compiled::build(&format!("{generated}\n{harness}"));
        assert!(Command::new(&compiled.executable)
            .status()
            .unwrap()
            .success());
    }
}

#[test]
fn filesystem_invalid_path_never_reaches_callback_and_cumulative_writes_stop() {
    if !clang_available() {
        return;
    }
    let invalid_path = codegen::emit_hir_c_with_filesystem_io(
        &resolved_invalid_path(),
        "filesystem-native-invalid.run",
    )
    .unwrap();
    let compiled = Compiled::build(&format!("{invalid_path}\n{INVALID_PATH_HARNESS}"));
    assert!(Command::new(&compiled.executable)
        .status()
        .unwrap()
        .success());

    let generated =
        codegen::emit_hir_c_with_filesystem_io(&resolved(), "filesystem-native.run").unwrap();
    let compiled = Compiled::build(&format!("{generated}\n{CUMULATIVE_HARNESS}"));
    assert!(Command::new(&compiled.executable)
        .status()
        .unwrap()
        .success());
}

#[test]
fn filesystem_profile_rejects_wrong_selected_shape() {
    let error = codegen::emit_hir_c_with_filesystem_io(&resolved(), "main").unwrap_err();
    assert_eq!(error.code, "SPX-B103");
}

#[test]
fn filesystem_native_failure_priority_matches_interpreter() {
    if !clang_available() {
        return;
    }
    for (max, expected) in [(65537, 1), (1048577, 4)] {
        let source = SOURCE
            .replace("[105u8, 110u8]", "[0u8, 0u8]")
            .replace("2usize, 4usize", &format!("2usize, {max}usize"));
        let ast = parse(&source, Path::new("filesystem-priority.spx")).unwrap();
        let program = hir::resolve(&ast).unwrap();
        let generated =
            codegen::emit_hir_c_with_filesystem_io(&program, "filesystem-native.run").unwrap();
        let harness = INVALID_PATH_HARNESS.replace(
            "result.status_code == UINT32_C(1)",
            &format!("result.status_code == UINT32_C({expected})"),
        );
        let compiled = Compiled::build(&format!("{generated}\n{harness}"));
        assert!(Command::new(&compiled.executable)
            .status()
            .unwrap()
            .success());
    }
}
