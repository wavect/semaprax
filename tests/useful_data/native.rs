use semaprax::{codegen, parse};
use std::path::Path;
use std::process::Command;
use std::sync::atomic::{AtomicU64, Ordering};

static NEXT_ID: AtomicU64 = AtomicU64::new(0);

const SOURCE: &str = r#"module test.useful_data_native;

@id("bytes.zero_frame")
record ZeroFrame {
    @id("bytes.zero_frame.empty")
    empty: [u8; 0],
    @id("bytes.zero_frame.marker")
    marker: u8,
}

@id("bytes.zero_only")
record ZeroOnly {
    @id("bytes.zero_only.empty")
    empty: [u8; 0],
}

@id("bytes.zero_array_identity")
fn zero_array_identity(value: [u8; 0]) -> [u8; 0] { value }

@id("bytes.zero_array_round_trip")
fn zero_array_round_trip() -> i64 {
    let value = zero_array_identity([]);
    let view = array_as_slice(value);
    if byte_len(view) == 0usize { 1 } else { 0 }
}

@id("bytes.zero_record_identity")
fn zero_record_identity(value: ZeroOnly) -> ZeroOnly { value }

@id("bytes.all_zero_record")
fn all_zero_record() -> i64 {
    let frame = zero_record_identity(ZeroOnly { empty: [] });
    let empty = frame.empty;
    let view = array_as_slice(empty);
    if byte_len(view) == 0usize { 1 } else { 0 }
}

@id("bytes.forward")
fn forward(value: own Bytes) -> Bytes { value }

@id("bytes.inspect")
fn inspect() -> i64 {
    let source = [0u8, 255u8, 128u8, 0u8];
    let view = array_as_slice(source);
    let owned = bytes_copy(view);
    let forwarded = forward(owned);
    let copied_view = bytes_as_slice(forwarded);
    match byte_get(copied_view, 1usize) {
        Option::Some { value: byte } => if byte == 255u8 { 41 } else { 2 },
        Option::None {} => 3,
    }
}

@id("bytes.choose")
fn choose(flag: bool) -> Bytes {
    if flag {
        let source = [0u8, 255u8];
        let view = array_as_slice(source);
        bytes_copy(view)
    } else {
        let source = [128u8, 0u8];
        let view = array_as_slice(source);
        bytes_copy(view)
    }
}

@id("bytes.branch")
fn branch() -> i64 {
    let selected = choose(true);
    let view = bytes_as_slice(selected);
    match byte_get(view, 1usize) {
        Option::Some { value: byte } => if byte == 255u8 { 1 } else { 0 },
        Option::None {} => 0,
    }
}

@id("bytes.mixed_roots")
fn mixed_roots(text: borrow str, data: borrow Slice<u8>) -> usize {
    byte_len(str_as_bytes(text)) + byte_len(data)
}

@id("bytes.reject")
fn reject(value: own Bytes) -> i64 requires false { 0 }

@id("bytes.failure_cleanup")
fn failure_cleanup(data: borrow Slice<u8>) -> i64 {
    let owned = bytes_copy(data);
    reject(owned)
}

@id("bytes.scope_cleanup")
fn scope_cleanup(data: borrow Slice<u8>) -> i64 {
    let marker = {
        let inner = bytes_copy(data);
        7
    };
    let outer = bytes_copy(data);
    marker
}

@id("bytes.empty")
fn empty() -> i64 {
    let source = [];
    let view = array_as_slice(source);
    let owned = bytes_copy(view);
    let copied_view = bytes_as_slice(owned);
    if byte_len(copied_view) == 0usize { 1 } else { 0 }
}

@id("bytes.zero_record")
fn zero_record() -> i64 {
    let frame = ZeroFrame { empty: [], marker: 7u8 };
    let empty = frame.empty;
    let view = array_as_slice(empty);
    if byte_len(view) == 0usize && frame.marker == 7u8 { 1 } else { 0 }
}

@id("app.main")
fn main() -> i64 {
    let repeated = [9u8; 4];
    let repeated_view = array_as_slice(repeated);
    if byte_len(repeated_view) == 4usize {
        inspect() + empty() + branch() + zero_record() + zero_array_round_trip() + all_zero_record()
    } else {
        0
    }
}
"#;

fn symbol(id: &str) -> String {
    use std::fmt::Write as _;
    let mut hex = String::with_capacity(id.len() * 2);
    for byte in id.bytes() {
        write!(hex, "{byte:02x}").unwrap();
    }
    format!("spx_decl_{hex}")
}

#[test]
fn native_arrays_and_owned_bytes_are_exact_at_o0_and_o2() {
    if Command::new("clang").arg("--version").output().is_err() {
        return;
    }
    let program = parse(SOURCE, Path::new("useful-data-native.spx")).unwrap();
    let generated = codegen::emit_c(&program).unwrap();
    assert_eq!(generated, codegen::emit_c(&program).unwrap());
    assert!(!generated.contains("struct spx_array_u8_0"));
    assert!(generated.contains("struct spx_array_u8_4"));
    assert!(generated.contains("uint8_t spx_zero_sized_record_carrier;"));
    assert!(generated.contains("SEMAPRAX zero-sized native aggregate carrier size"));
    assert!(generated.contains(&format!(
        "{}(struct spx_context *spx_ctx, uint8_t spx_param_0, uint8_t *spx_result_out)",
        symbol("bytes.zero_array_identity")
    )));
    assert!(generated.contains("SEMAPRAX native aggregate field offset"));
    assert!(generated.contains("spx_bytes_copy"));
    assert!(generated.contains("spx_bytes_move"));
    assert!(generated.contains("spx_bytes_drop"));
    assert!(generated.contains("spx_borrowed_root_bytes"));
    assert!(!generated.contains("strlen(value.ptr)"));
    let (freestanding, entry_wrapper) = generated
        .split_once("#ifndef SPX_NO_ENTRY_WRAPPER")
        .expect("ordinary native output must contain one process entry wrapper");
    assert!(!freestanding.contains("#include <fcntl.h>"));
    assert!(entry_wrapper.contains("#include <fcntl.h>"));
    assert!(entry_wrapper.contains("#include <io.h>"));
    let binary_mode = entry_wrapper
        .find("_setmode(_fileno(stdout), _O_BINARY)")
        .expect("Windows stdout must be made byte-exact");
    let stdout_write = entry_wrapper
        .find("printf(\"%lld\\n\"")
        .expect("ordinary native result write must remain present");
    assert!(binary_mode < stdout_write);

    let mixed_probe = format!(
        r#"
int main(int argc, char **argv) {{
    (void)argv;
    struct spx_status_entry entries[UINT32_C(4)];
    struct spx_context context = {{0}};
    if (!spx_context_init(&context, UINT64_C(8), entries, UINT32_C(4), NULL, NULL, NULL)) return 10;
    static uint8_t text_bytes[UINT64_C(65535)];
    static uint8_t slice_bytes[UINT64_C(2)];
    memset(text_bytes, 'a', sizeof(text_bytes));
    spx_str_v1 text = {{text_bytes, UINT64_C(65535)}};
    spx_slice_u8_v1 slice = {{slice_bytes, argc > 1 ? UINT64_C(2) : UINT64_C(1)}};
    uint64_t result = UINT64_C(0);
    if ({mixed}(&context, text, slice, &result) != SPX_STATUS_SUCCESS) return 11;
    return result == UINT64_C(65536) ? 0 : 12;
}}
"#,
        mixed = symbol("bytes.mixed_roots"),
    );
    let failure_runtime = generated
        .replace(
            "malloc((size_t)value.len)",
            "spx_test_malloc((size_t)value.len)",
        )
        .replace("free(value->ptr)", "spx_test_free(value->ptr)");
    let failure_probe = format!(
        r#"
static void *spx_allocations[UINT32_C(8)];
static uint32_t spx_allocation_count = UINT32_C(0);
static uint32_t spx_free_count = UINT32_C(0);
static void *spx_test_malloc(size_t size) {{
    if (spx_allocation_count != spx_free_count) abort();
    void *value = malloc(size);
    if (value == NULL || spx_allocation_count >= UINT32_C(8)) abort();
    spx_allocations[spx_allocation_count++] = value;
    return value;
}}
static void spx_test_free(void *value) {{
    if (value == NULL) abort();
    bool found = false;
    for (uint32_t index = UINT32_C(0); index < spx_allocation_count; ++index) {{
        if (spx_allocations[index] == value) {{
            if (found) abort();
            found = true;
            spx_allocations[index] = NULL;
        }}
    }}
    if (!found) abort();
    ++spx_free_count;
    free(value);
}}
int main(void) {{
    struct spx_status_entry entries[UINT32_C(8)];
    struct spx_context context = {{0}};
    if (!spx_context_init(&context, UINT64_C(11), entries, UINT32_C(8), NULL, NULL, NULL)) return 10;
    const uint8_t payload[] = {{UINT8_C(0), UINT8_C(255), UINT8_C(128)}};
    spx_slice_u8_v1 view = {{payload, UINT64_C(3)}};
    int64_t ignored = INT64_C(0);
    spx_status_token scope_status = {scope}(&context, view, &ignored);
    if (scope_status != SPX_STATUS_SUCCESS || ignored != INT64_C(7)) return 11;
    for (uint32_t index = UINT32_C(0); index < UINT32_C(2); ++index) {{
        spx_status_token status = {failure}(&context, view, &ignored);
        const struct spx_normalized_status *normalized = spx_status_resolve(&context, status);
        if (normalized == NULL || normalized->code != SPX_STATUS_CONTRACT_REQUIRES_FALSE) return 12;
    }}
    if (spx_allocation_count != UINT32_C(4) || spx_free_count != UINT32_C(4)) return 13;
    return 0;
}}
"#,
        failure = symbol("bytes.failure_cleanup"),
        scope = symbol("bytes.scope_cleanup"),
    );

    for optimization in ["-O0", "-O2"] {
        let id = NEXT_ID.fetch_add(1, Ordering::Relaxed);
        let stem = format!("semaprax-useful-data-native-{}-{id}", std::process::id());
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
        let executed = Command::new(&executable).output().unwrap();
        let _ = std::fs::remove_file(source);
        let _ = std::fs::remove_file(executable);
        assert!(
            executed.status.success(),
            "native probe failed: {executed:?}"
        );
        assert_eq!(executed.stdout, b"46\n");

        let probe_source = std::env::temp_dir().join(format!("{stem}-mixed.c"));
        let probe_executable =
            std::env::temp_dir().join(format!("{stem}-mixed{}", std::env::consts::EXE_SUFFIX));
        std::fs::write(&probe_source, format!("{generated}\n{mixed_probe}")).unwrap();
        let compiled = Command::new("clang")
            .args([
                "-std=c11",
                optimization,
                "-Wall",
                "-Wextra",
                "-Werror",
                "-DSPX_NO_ENTRY_WRAPPER",
            ])
            .arg(&probe_source)
            .arg("-o")
            .arg(&probe_executable)
            .output()
            .unwrap();
        assert!(
            compiled.status.success(),
            "{}",
            String::from_utf8_lossy(&compiled.stderr)
        );
        assert!(Command::new(&probe_executable).status().unwrap().success());
        assert!(!Command::new(&probe_executable)
            .arg("overflow")
            .status()
            .unwrap()
            .success());
        let _ = std::fs::remove_file(probe_source);
        let _ = std::fs::remove_file(probe_executable);

        let failure_source = std::env::temp_dir().join(format!("{stem}-failure.c"));
        let failure_executable =
            std::env::temp_dir().join(format!("{stem}-failure{}", std::env::consts::EXE_SUFFIX));
        let instrumented = format!(
            "#include <stddef.h>\n#include <stdlib.h>\nstatic void *spx_test_malloc(size_t);\nstatic void spx_test_free(void *);\n{failure_runtime}\n{failure_probe}"
        );
        std::fs::write(&failure_source, instrumented).unwrap();
        let compiled = Command::new("clang")
            .args([
                "-std=c11",
                optimization,
                "-Wall",
                "-Wextra",
                "-Werror",
                "-DSPX_NO_ENTRY_WRAPPER",
            ])
            .arg(&failure_source)
            .arg("-o")
            .arg(&failure_executable)
            .output()
            .unwrap();
        assert!(
            compiled.status.success(),
            "{}",
            String::from_utf8_lossy(&compiled.stderr)
        );
        assert!(Command::new(&failure_executable)
            .status()
            .unwrap()
            .success());
        let _ = std::fs::remove_file(failure_source);
        let _ = std::fs::remove_file(failure_executable);
    }
}
