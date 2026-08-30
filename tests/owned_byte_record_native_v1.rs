use std::path::Path;
use std::process::Command;
use std::sync::atomic::{AtomicU64, Ordering};

use semaprax::{codegen, hir, parse};

static SERIAL: AtomicU64 = AtomicU64::new(0);

const SOURCE: &str = r#"
module test.owned_byte_record_native;

@id("native.packet")
record Packet {
    @id("native.packet.left") left: Bytes,
    @id("native.packet.marker") marker: i64,
    @id("native.packet.right") right: Bytes,
}

@id("native.make")
fn make(left: borrow Slice<u8>, right: borrow Slice<u8>) -> Packet {
    Packet { left: bytes_copy(left), marker: 9, right: bytes_copy(right) }
}

@id("native.borrow_count")
fn borrow_count(value: borrow Packet) -> i64 {
    match borrow value {
        Packet { left, marker: _, right } => {
            if byte_len(bytes_as_slice(left)) == 1usize && byte_len(bytes_as_slice(right)) == 1usize { 1 } else { 0 }
        },
    }
}

@id("native.borrow_forward")
fn borrow_forward(value: borrow Packet) -> i64 {
    let scratch_len = match borrow value {
        Packet { left, marker: _, right: _ } => {
            let scratch = bytes_copy(bytes_as_slice(left));
            if byte_len(bytes_as_slice(scratch)) == 1usize { 1 } else { 0 }
        },
    };
    borrow_count(value)
}

@id("native.inspect")
fn inspect(value: own Packet) -> Packet {
    let measured = borrow_forward(value);
    value
}

@id("native.consume")
fn consume(value: own Packet) -> i64 {
    match own value {
        Packet { left, marker: _, right } => {
            match byte_get(bytes_as_slice(left), 0usize) {
                Option::Some { value: left_byte } => match byte_get(bytes_as_slice(right), 0usize) {
                    Option::Some { value: right_byte } => {
                        if left_byte == 11u8 && right_byte == 22u8 { 33 } else { 2 }
                    },
                    Option::None {} => 3,
                },
                Option::None {} => 4,
            }
        },
    }
}

@id("native.reject")
fn reject(value: own Packet) -> i64 requires false { 0 }

@id("native.failure")
fn failure(data: borrow Slice<u8>) -> i64 {
    reject(make(data, data))
}

@id("native.round_trip")
fn round_trip(data: borrow Slice<u8>) -> i64 {
    consume(inspect(make(data, data)))
}

@id("app.main")
fn main() -> i64 {
    let left = [11u8];
    let right = [22u8];
    consume(inspect(make(array_as_slice(left), array_as_slice(right))))
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

fn compile_and_run(source: &str, probe: &str, optimization: &str) {
    let serial = SERIAL.fetch_add(1, Ordering::Relaxed);
    let stem = format!(
        "semaprax-owned-record-native-{}-{serial}",
        std::process::id()
    );
    let c_path = std::env::temp_dir().join(format!("{stem}.c"));
    let executable = std::env::temp_dir().join(format!("{stem}{}", std::env::consts::EXE_SUFFIX));
    std::fs::write(&c_path, format!("{source}\n{probe}")).unwrap();
    let compiled = Command::new("clang")
        .args([
            "-std=c11",
            optimization,
            "-Wall",
            "-Wextra",
            "-Werror",
            "-DSPX_NO_ENTRY_WRAPPER",
        ])
        .arg(&c_path)
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
    let _ = std::fs::remove_file(c_path);
    let _ = std::fs::remove_file(executable);
    assert!(executed.status.success(), "probe failed: {executed:?}");
}

#[test]
fn flat_owned_byte_records_execute_and_settle_exactly_at_o0_and_o2() {
    assert_native_record_corpus(SOURCE);
}

#[test]
fn record_field_places_branches_and_blocks_follow_exact_cleanup_transfers() {
    let original = "Packet { left: bytes_copy(left), marker: 9, right: bytes_copy(right) }";
    assert_eq!(SOURCE.matches(original).count(), 1);
    for replacement in [
        "let first = bytes_copy(left); let second = bytes_copy(right); Packet { left: first, marker: 9, right: second }",
        "Packet { left: if true { bytes_copy(left) } else { bytes_copy(left) }, marker: 9, right: if false { bytes_copy(right) } else { bytes_copy(right) } }",
        "Packet { left: { let first = bytes_copy(left); first }, marker: 9, right: { let second = bytes_copy(right); second } }",
    ] {
        assert_native_record_corpus(&SOURCE.replace(original, replacement));
    }
}

fn assert_native_record_corpus(source: &str) {
    if Command::new("clang").arg("--version").output().is_err() {
        return;
    }
    let program = parse(source, Path::new("owned-byte-record-native-v1.spx")).unwrap();
    let generated = codegen::emit_c(&program).unwrap();
    assert_eq!(generated, codegen::emit_c(&program).unwrap());
    assert!(!generated.contains("memcpy(&"));
    assert!(!generated.contains("memcpy(spx_bytes_slot_"));
    assert!(generated.contains("spx_bytes_move"));
    assert!(generated.contains("dead owned record field"));
    assert!(generated.contains("const spx_bytes_v1 *spx_param_0_borrow_spx_field_"));

    let success_probe = format!(
        r#"
int main(void) {{
    struct spx_status_entry entries[UINT32_C(16)];
    struct spx_context context = {{0}};
    if (!spx_context_init(&context, UINT64_C(91), entries, UINT32_C(16), NULL, NULL, NULL)) return 10;
    int64_t result = INT64_C(0);
    if ({main}(&context, &result) != SPX_STATUS_SUCCESS) return 11;
    return result == INT64_C(33) ? 0 : 12;
}}
"#,
        main = symbol("app.main"),
    );

    let tracked = format!(
        "#include <stddef.h>\nstatic void *spx_test_malloc(size_t);\nstatic void spx_test_free(void *);\n{}",
        generated
            .replace(
            "malloc((size_t)value.len)",
            "spx_test_malloc((size_t)value.len)",
        )
            .replace("free(value->ptr)", "spx_test_free(value->ptr)")
    );
    let failure_probe = format!(
        r#"
static void *spx_allocations[UINT32_C(12)];
static uint32_t spx_allocation_count = UINT32_C(0);
static uint32_t spx_free_count = UINT32_C(0);
static void *spx_test_malloc(size_t size) {{
    void *value = malloc(size);
    if (value == NULL || spx_allocation_count >= UINT32_C(12)) abort();
    spx_allocations[spx_allocation_count++] = value;
    return value;
}}
static void spx_test_free(void *value) {{
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
    struct spx_status_entry entries[UINT32_C(16)];
    struct spx_context context = {{0}};
    if (!spx_context_init(&context, UINT64_C(92), entries, UINT32_C(16), NULL, NULL, NULL)) return 20;
    const uint8_t payload[] = {{UINT8_C(7)}};
    spx_slice_u8_v1 view = {{payload, UINT64_C(1)}};
    int64_t ignored = INT64_C(0);
    for (uint32_t index = UINT32_C(0); index < UINT32_C(2); ++index) {{
        spx_status_token success = {round_trip}(&context, view, &ignored);
        if (success != SPX_STATUS_SUCCESS) return 21;
    }}
    for (uint32_t index = UINT32_C(0); index < UINT32_C(2); ++index) {{
        spx_status_token status = {failure}(&context, view, &ignored);
        const struct spx_normalized_status *normalized = spx_status_resolve(&context, status);
        if (normalized == NULL || normalized->code != SPX_STATUS_CONTRACT_REQUIRES_FALSE) return 22;
    }}
    return spx_allocation_count == UINT32_C(10) && spx_free_count == UINT32_C(10) ? 0 : 23;
}}
"#,
        failure = symbol("native.failure"),
        round_trip = symbol("native.round_trip"),
    );

    for optimization in ["-O0", "-O2"] {
        compile_and_run(&generated, &success_probe, optimization);
        compile_and_run(&tracked, &failure_probe, optimization);
    }
}

#[test]
fn hostile_projected_plan_field_identity_drift_is_rejected() {
    let program = parse(SOURCE, Path::new("owned-byte-record-hostile.spx")).unwrap();
    let mut resolved = hir::resolve(&program).unwrap();
    let function = resolved
        .functions
        .iter_mut()
        .find(|function| function.id.as_str() == "native.make")
        .unwrap();
    let destination = function
        .cleanup_plan
        .blocks
        .iter_mut()
        .flat_map(|block| &mut block.transitions)
        .find_map(|transition| match transition {
            semaprax::cleanup_plan::CleanupTransition::Transfer { destination, .. }
                if !destination.projections.is_empty() =>
            {
                Some(destination)
            }
            _ => None,
        })
        .unwrap();
    destination.projections[0] = hir::DeclarationId::new("hostile.wrong-field");
    let diagnostic = codegen::emit_hir_c(&resolved).unwrap_err();
    assert_eq!(diagnostic.code, "SPX-H006");
}
