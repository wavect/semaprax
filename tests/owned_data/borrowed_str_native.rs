use semaprax::{codegen, parse};
use std::path::Path;
use std::process::Command;
use std::sync::atomic::{AtomicU64, Ordering};

static NEXT_ID: AtomicU64 = AtomicU64::new(0);

const SOURCE: &str = r#"module text.native;

@id("text.contains")
fn contains(value: borrow str, needle: borrow str) -> bool {
    str_contains(value, needle)
}

@id("text.starts")
fn starts(value: borrow str, prefix: borrow str) -> bool {
    str_starts_with(value, prefix)
}

@id("text.len")
fn text_byte_len(value: borrow str) -> i64 {
    str_len_bytes(value)
}

@id("text.same")
fn same(value: borrow str) -> bool {
    str_starts_with(value, value) && str_contains(value, value)
}

@id("main")
fn main() -> i64 { 0 }
"#;

fn command_available(command: &str) -> bool {
    Command::new(command).arg("--version").output().is_ok()
}

fn symbol(id: &str) -> String {
    use std::fmt::Write as _;
    let mut hex = String::with_capacity(id.len() * 2);
    for byte in id.bytes() {
        write!(hex, "{byte:02x}").unwrap();
    }
    format!("spx_decl_{hex}")
}

#[test]
fn native_borrowed_str_is_length_aware_utf8_and_cleanup_inert_at_o0_o2() {
    if !command_available("clang") {
        return;
    }
    let program = parse(SOURCE, Path::new("borrowed-str-native.spx")).unwrap();
    let generated = codegen::emit_c(&program).unwrap();
    assert_eq!(generated, codegen::emit_c(&program).unwrap());
    assert!(generated
        .contains("typedef struct {\n    const uint8_t *data;\n    uint64_t len;\n} spx_str_v1;"));
    assert!(!generated.contains("spx_string_clone"));
    assert!(!generated.contains("spx_string_drop"));
    assert!(!generated.contains("strstr("));
    assert!(generated.contains("Fixed-capacity KMP"));
    assert!(generated.contains("uint16_t prefix[SPX_BORROWED_STR_MAX_BYTES]"));
    assert!(!generated.contains("memcmp(value.data + offset"));

    let probe = format!(
        r#"
int main(void) {{
    struct spx_status_entry entries[UINT32_C(16)];
    struct spx_context context = {{0}};
    if (!spx_context_init(&context, UINT64_C(9), entries, UINT32_C(16), NULL, NULL, NULL)) return 10;
    const uint8_t value_bytes[] = {{'a', UINT8_C(0), 'b', UINT8_C(0xe2), UINT8_C(0x82), UINT8_C(0xac)}};
    const uint8_t nul_b[] = {{UINT8_C(0), 'b'}};
    const uint8_t euro[] = {{UINT8_C(0xe2), UINT8_C(0x82), UINT8_C(0xac)}};
    spx_str_v1 value = {{value_bytes, UINT64_C(6)}};
    spx_str_v1 embedded = {{nul_b, UINT64_C(2)}};
    spx_str_v1 suffix = {{euro, UINT64_C(3)}};
    spx_str_v1 empty = {{NULL, UINT64_C(0)}};
    bool observed = false;
    if ({contains}(&context, value, embedded, &observed) != SPX_STATUS_SUCCESS || !observed) return 11;
    if ({contains}(&context, value, suffix, &observed) != SPX_STATUS_SUCCESS || !observed) return 12;
    if ({starts}(&context, value, empty, &observed) != SPX_STATUS_SUCCESS || !observed) return 13;
    int64_t length = INT64_C(-1);
    if ({len}(&context, value, &length) != SPX_STATUS_SUCCESS || length != INT64_C(6)) return 14;

    static uint8_t periodic_value[UINT64_C(49152)];
    static uint8_t periodic_needle[UINT64_C(16384)];
    memset(periodic_value, 'a', sizeof(periodic_value));
    memset(periodic_needle, 'a', sizeof(periodic_needle));
    periodic_needle[sizeof(periodic_needle) - 1u] = 'b';
    periodic_value[sizeof(periodic_value) - 1u] = 'b';
    spx_str_v1 periodic_value_view = {{periodic_value, UINT64_C(49152)}};
    spx_str_v1 periodic_needle_view = {{periodic_needle, UINT64_C(16384)}};
    if ({contains}(&context, periodic_value_view, periodic_needle_view, &observed)
        != SPX_STATUS_SUCCESS || !observed) return 15;
    periodic_value[sizeof(periodic_value) - 1u] = 'a';
    if ({contains}(&context, periodic_value_view, periodic_needle_view, &observed)
        != SPX_STATUS_SUCCESS || observed) return 16;

    static uint8_t exact_alias[SPX_BORROWED_STR_MAX_BYTES];
    memset(exact_alias, 'a', sizeof(exact_alias));
    spx_str_v1 exact_alias_view = {{exact_alias, SPX_BORROWED_STR_MAX_BYTES}};
    if ({same}(&context, exact_alias_view, &observed)
        != SPX_STATUS_SUCCESS || !observed) return 17;
    return 0;
}}
"#,
        contains = symbol("text.contains"),
        starts = symbol("text.starts"),
        len = symbol("text.len"),
        same = symbol("text.same"),
    );
    let overflow_probe = format!(
        r#"
int main(void) {{
    struct spx_status_entry entries[UINT32_C(4)];
    struct spx_context context = {{0}};
    if (!spx_context_init(&context, UINT64_C(10), entries, UINT32_C(4), NULL, NULL, NULL)) return 10;
    static uint8_t first[UINT64_C(40000)];
    static uint8_t second[UINT64_C(40000)];
    memset(first, 'a', sizeof(first)); memset(second, 'a', sizeof(second));
    spx_str_v1 first_view = {{first, UINT64_C(40000)}};
    spx_str_v1 second_view = {{second, UINT64_C(40000)}};
    bool observed = false;
    (void){contains}(&context, first_view, second_view, &observed);
    return 0;
}}
"#,
        contains = symbol("text.contains"),
    );

    for optimization in ["-O0", "-O2"] {
        let id = NEXT_ID.fetch_add(1, Ordering::Relaxed);
        let stem = format!("semaprax-borrowed-str-native-{}-{id}", std::process::id());
        let source = std::env::temp_dir().join(format!("{stem}.c"));
        let executable =
            std::env::temp_dir().join(format!("{stem}{}", std::env::consts::EXE_SUFFIX));
        std::fs::write(&source, format!("{generated}\n{probe}")).unwrap();
        let compiled = Command::new("clang")
            .args([
                "-std=c11",
                optimization,
                "-Wall",
                "-Wextra",
                "-Werror",
                "-DSPX_NO_ENTRY_WRAPPER",
            ])
            .arg(&source)
            .arg("-o")
            .arg(&executable)
            .output()
            .unwrap();
        assert!(
            compiled.status.success(),
            "borrowed Str C failed at {optimization}: {}",
            String::from_utf8_lossy(&compiled.stderr)
        );
        let executed = Command::new(&executable).output().unwrap();
        let _ = std::fs::remove_file(&source);
        let _ = std::fs::remove_file(&executable);
        assert!(
            executed.status.success(),
            "borrowed Str native probe failed at {optimization}: {:?}",
            executed.status.code()
        );

        let overflow_source = std::env::temp_dir().join(format!("{stem}-overflow.c"));
        let overflow_executable =
            std::env::temp_dir().join(format!("{stem}-overflow{}", std::env::consts::EXE_SUFFIX));
        std::fs::write(&overflow_source, format!("{generated}\n{overflow_probe}")).unwrap();
        let compiled = Command::new("clang")
            .args([
                "-std=c11",
                optimization,
                "-Wall",
                "-Wextra",
                "-Werror",
                "-DSPX_NO_ENTRY_WRAPPER",
            ])
            .arg(&overflow_source)
            .arg("-o")
            .arg(&overflow_executable)
            .output()
            .unwrap();
        assert!(
            compiled.status.success(),
            "borrowed Str overflow C failed at {optimization}: {}",
            String::from_utf8_lossy(&compiled.stderr)
        );
        let overflow = Command::new(&overflow_executable).output().unwrap();
        let _ = std::fs::remove_file(overflow_source);
        let _ = std::fs::remove_file(overflow_executable);
        assert!(
            !overflow.status.success(),
            "native external invocation widened the cumulative byte budget at {optimization}"
        );
    }
}
