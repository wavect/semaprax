//! Executable native evidence for the internal synchronous `borrow Bytes`
//! boundary. The same generated C is compiled at O0 and O2 so a textual
//! pointer-shape assertion cannot hide an addressability or ownership defect.

use semaprax::{codegen, hir, parse, verify};
use std::fmt::Write as _;
use std::path::Path;
use std::process::Command;
use std::sync::atomic::{AtomicU64, Ordering};

static SERIAL: AtomicU64 = AtomicU64::new(0);

const SOURCE: &str = r#"
module test.projected_owned_bytes_borrowed_call_native;

@id("borrow.packet")
record Packet {
    @id("borrow.packet.payload") payload: Bytes,
    @id("borrow.packet.marker") marker: i64,
}

@id("borrow.inspect")
fn inspect(value: borrow Bytes) -> usize {
    byte_len(bytes_as_slice(value))
}

@id("borrow.exercise")
fn exercise() -> i64 {
    let source = [1u8, 2u8, 3u8, 4u8, 5u8, 6u8, 7u8];
    let view = array_as_slice(source);
    let packet = Packet {
        payload: bytes_copy(view),
        marker: 28,
    };
    let first = inspect(packet.payload);
    let second = inspect(packet.payload);
    let retained = bytes_as_slice(packet.payload);
    if first == 7usize
        && second == first
        && byte_len(retained) == 7usize
    {
        packet.marker + 14
    } else {
        0
    }
}

@id("app.main") fn main() -> i64 { exercise() }
"#;

fn symbol(id: &str) -> String {
    let mut hex = String::with_capacity(id.len() * 2);
    for byte in id.bytes() {
        write!(hex, "{byte:02x}").unwrap();
    }
    format!("spx_decl_{hex}")
}

#[test]
fn native_o0_and_o2_preserve_projected_borrow_and_settle_only_the_owner() {
    if Command::new("clang").arg("--version").output().is_err() {
        return;
    }
    let parsed = parse(
        SOURCE,
        Path::new("projected-owned-bytes-borrowed-call-native-v1.spx"),
    )
    .expect("fixture parses");
    assert!(verify::verify(&parsed).is_empty());
    let resolved = hir::resolve(&parsed).expect("fixture resolves");
    hir::validate(&resolved).expect("fixture validates");
    let generated = codegen::emit_hir_c(&resolved).expect("native C emits");
    let inspect = symbol("borrow.inspect");
    assert!(generated.contains(&format!(
        "{inspect}(struct spx_context *spx_ctx, const spx_bytes_v1 *spx_param_0"
    )));
    for line in generated.lines().filter(|line| line.contains(&inspect)) {
        assert!(!line.contains("spx_bytes_move"), "{line}");
    }

    let probe = format!(
        r#"
int main(void) {{
    struct spx_status_entry entries[UINT32_C(16)];
    struct spx_context context = {{0}};
    if (!spx_context_init(&context, UINT64_C(401), entries, UINT32_C(16), NULL, NULL, NULL)) return 10;
    int64_t result = INT64_C(0);
    if ({main}(&context, &result) != SPX_STATUS_SUCCESS) return 11;
    return result == INT64_C(42) ? 0 : 12;
}}
"#,
        main = symbol("app.main"),
    );

    for optimization in ["-O0", "-O2"] {
        let serial = SERIAL.fetch_add(1, Ordering::Relaxed);
        let stem = format!(
            "semaprax-projected-owned-bytes-borrowed-call-{}-{serial}",
            std::process::id()
        );
        let c_path = std::env::temp_dir().join(format!("{stem}.c"));
        let executable =
            std::env::temp_dir().join(format!("{stem}{}", std::env::consts::EXE_SUFFIX));
        std::fs::write(&c_path, format!("{generated}\n{probe}")).unwrap();
        let built = Command::new("clang")
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
            built.status.success(),
            "{}",
            String::from_utf8_lossy(&built.stderr)
        );
        let executed = Command::new(&executable).output().unwrap();
        let _ = std::fs::remove_file(c_path);
        let _ = std::fs::remove_file(executable);
        assert!(executed.status.success(), "probe failed: {executed:?}");
    }
}
