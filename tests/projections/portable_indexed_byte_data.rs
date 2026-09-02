use std::fmt::Write as _;
use std::path::Path;
use std::process::Command;
use std::sync::atomic::{AtomicU64, Ordering};

use semaprax::hir::{self, ByteSliceExtent, ByteSliceRootKind, ResolvedType};
use semaprax::interpreter::{self, ArgumentValue, InterpreterOptions};
use semaprax::{codegen, format, graph, parse, verify};
use sha2::{Digest as _, Sha256};

static NEXT_ID: AtomicU64 = AtomicU64::new(0);

const SOURCE: &str = r#"
module test.portable_indexed_bytes;

@id("bytes.first")
fn first(value: borrow Slice<u8>) -> u8 {
    match byte_get(value, 0usize) {
        Option::Some { value: byte } => byte,
        Option::None {} => 0u8,
    }
}

@id("bytes.forwarded")
fn forwarded(value: borrow Slice<u8>) -> u8 {
    let alias = value;
    first(alias)
}

@id("bytes.summary")
fn summary(left: borrow Slice<u8>, right: borrow Slice<u8>) -> usize {
    byte_len(left) + byte_len(right)
}

@id("app.main")
fn main() -> i64 { 0 }
"#;

fn symbol(id: &str) -> String {
    let mut hex = String::with_capacity(id.len() * 2);
    for byte in id.bytes() {
        write!(hex, "{byte:02x}").unwrap();
    }
    format!("spx_decl_{hex}")
}

fn write_source() -> std::path::PathBuf {
    let id = NEXT_ID.fetch_add(1, Ordering::Relaxed);
    let path = std::env::temp_dir().join(format!(
        "semaprax-portable-indexed-bytes-{}-{id}.spx",
        std::process::id()
    ));
    std::fs::write(&path, SOURCE).unwrap();
    path
}

#[test]
fn slice_source_round_trips_and_rejects_every_escaping_shape() {
    let program = parse(SOURCE, Path::new("portable-indexed-bytes.spx")).unwrap();
    assert!(verify::verify(&program).is_empty());
    let canonical = format::canonical(&program);
    assert!(canonical.contains("borrow Slice<u8>"));
    assert_eq!(
        format::canonical(&parse(&canonical, "roundtrip.spx").unwrap()),
        canonical
    );

    let cases = [
        (
            SOURCE.replace("value: borrow Slice<u8>", "value: Slice<u8>"),
            "SPX-T263",
        ),
        (
            SOURCE.replace(
                "fn first(value: borrow Slice<u8>) -> u8",
                "fn first(value: borrow Slice<u8>) -> Slice<u8>",
            ),
            "SPX-T264",
        ),
        (SOURCE.replace("Slice<u8>", "Slice<i64>"), "SPX-T268"),
        (SOURCE.replace("fn first", "fn byte_len"), "SPX-S113"),
    ];
    for (source, expected) in cases {
        match parse(&source, "rejected-slice.spx") {
            Err(diagnostic) => assert_eq!(diagnostic.code, expected),
            Ok(program) => {
                let diagnostics = verify::verify(&program);
                assert_eq!(diagnostics.first().map(|item| item.code), Some(expected));
            }
        }
    }
}

#[test]
fn hir_and_graph_preserve_symbolic_parameter_roots_and_aliases() {
    let program = parse(SOURCE, "slice-provenance.spx").unwrap();
    let resolved = hir::resolve(&program).unwrap();
    hir::validate(&resolved).unwrap();

    let roots = resolved
        .declarations
        .byte_slice_provenances()
        .collect::<Vec<_>>();
    assert!(
        roots.len() >= 5,
        "three formal roots plus the forwarded alias"
    );
    for (_, provenance) in roots {
        assert_eq!(provenance.root_kind, ByteSliceRootKind::FunctionParameter);
        assert_eq!(provenance.root_length, ByteSliceExtent::ParameterLength);
        assert_eq!(provenance.offset, ByteSliceExtent::Constant(0));
        assert_eq!(provenance.length, ByteSliceExtent::ParameterLength);
    }

    let graph = graph::to_json(&program).unwrap();
    assert!(graph.contains("\"schema\":\"semaprax.graph.v17\""));
    assert!(graph.contains("\"root_kind\":\"function_parameter\""));
    assert!(graph.contains("\"byte_slice_provenance\""));
    assert!(graph.contains("\"max_external_root_bytes\":65536"));
    assert!(graph.contains("\"callee\":\"core.bytes.get\""));

    let legacy = parse(
        "module legacy;\n@id(\"app.main\") fn main() -> i64 { 0 }\n",
        "legacy.spx",
    )
    .unwrap();
    let legacy_graph = graph::to_json(&legacy).unwrap();
    assert!(legacy_graph.contains("\"schema\":\"semaprax.graph.v10\""));
    assert!(!legacy_graph.contains("portable_indexed_byte_data"));
}

#[test]
fn graph_v17_bytes_remain_frozen_without_stdout_v18_facts() {
    let program = parse(SOURCE, "graph-v17-frozen.spx").unwrap();
    let graph = graph::to_json(&program).unwrap();
    assert!(graph.contains("\"schema\":\"semaprax.graph.v17\""));
    assert!(!graph.contains("stdout_write_sites"));
    assert!(!graph.contains("bounded_stdout_transcript"));
    assert_eq!(
        format!(
            "{:x}",
            semaprax::digest_hex::LowerHex(Sha256::digest(graph.as_bytes()))
        ),
        "35e44f3d697abb3d406955d76bfd2395eb7d0cd1ccecf05699057712d571209b"
    );
}

#[test]
fn interpreter_is_nul_safe_total_and_enforces_the_cumulative_root_bound() {
    assert_eq!(
        interpreter::parse_argument("[0,255,1]").unwrap(),
        ArgumentValue::BorrowedSlice(vec![0, 255, 1])
    );
    assert!(interpreter::parse_argument("[01]").is_err());
    assert!(interpreter::parse_argument("[256]").is_err());

    let path = write_source();
    let options = InterpreterOptions::default();
    let first = interpreter::interpret(
        &path,
        "bytes.forwarded",
        &["[255,0,7]".to_owned()],
        &options,
    )
    .unwrap();
    assert!(first.envelope.contains("\"type\":\"u8\""));
    assert!(first.envelope.contains("\"value\":\"255u8\""));

    let empty =
        interpreter::interpret(&path, "bytes.forwarded", &["[]".to_owned()], &options).unwrap();
    assert!(empty.envelope.contains("\"value\":\"0u8\""));

    let forty_kib = serde_json::to_string(&vec![1u8; 40_000]).unwrap();
    let overflow = interpreter::interpret(
        &path,
        "bytes.summary",
        &[forty_kib.clone(), forty_kib],
        &options,
    )
    .unwrap_err();
    assert_eq!(overflow.first().map(|item| item.code), Some("SPX-F105"));
    let _ = std::fs::remove_file(path);
}

#[test]
fn native_slice_carrier_and_total_get_are_exact_at_o0_and_o2() {
    if Command::new("clang").arg("--version").output().is_err() {
        return;
    }
    let program = parse(SOURCE, "slice-native.spx").unwrap();
    let generated = codegen::emit_c(&program).unwrap();
    assert!(generated.contains("spx_slice_u8_v1"));
    assert!(generated.contains("spx_byte_len"));
    assert!(!generated.contains("strlen("));

    let forwarded = symbol("bytes.forwarded");
    let summary = symbol("bytes.summary");
    let probe = format!(
        r#"
int main(void) {{
    struct spx_status_entry entries[UINT32_C(16)];
    struct spx_context context = {{0}};
    if (!spx_context_init(&context, UINT64_C(19), entries, UINT32_C(16), NULL, NULL, NULL)) return 10;
    const uint8_t bytes[] = {{UINT8_C(0), UINT8_C(255), UINT8_C(7)}};
    spx_slice_u8_v1 view = {{bytes, UINT64_C(3)}};
    spx_slice_u8_v1 empty = {{NULL, UINT64_C(0)}};
    uint8_t observed = UINT8_C(0);
    if ({forwarded}(&context, view, &observed) != SPX_STATUS_SUCCESS || observed != UINT8_C(0)) return 11;
    if ({forwarded}(&context, empty, &observed) != SPX_STATUS_SUCCESS || observed != UINT8_C(0)) return 12;
    uint64_t length = UINT64_C(0);
    if ({summary}(&context, view, empty, &length) != SPX_STATUS_SUCCESS || length != UINT64_C(3)) return 13;
    static uint8_t boundary_left[UINT64_C(32768)];
    static uint8_t boundary_right[UINT64_C(32768)];
    spx_slice_u8_v1 left = {{boundary_left, UINT64_C(32768)}};
    spx_slice_u8_v1 right = {{boundary_right, UINT64_C(32768)}};
    if ({summary}(&context, left, right, &length) != SPX_STATUS_SUCCESS || length != UINT64_C(65536)) return 14;
    return 0;
}}
"#
    );
    let rejection_probe = format!(
        r#"
int main(int argc, char **argv) {{
    struct spx_status_entry entries[UINT32_C(16)];
    struct spx_context context = {{0}};
    if (!spx_context_init(&context, UINT64_C(23), entries, UINT32_C(16), NULL, NULL, NULL)) return 10;
    static uint8_t first[UINT64_C(40000)];
    static uint8_t second[UINT64_C(40000)];
    spx_slice_u8_v1 left = {{first, UINT64_C(40000)}};
    spx_slice_u8_v1 right = {{second, UINT64_C(40000)}};
    if (argc > 1) {{
        spx_slice_u8_v1 malformed_empty = {{first, UINT64_C(0)}};
        uint8_t observed = UINT8_C(0);
        (void){forwarded}(&context, malformed_empty, &observed);
    }} else {{
        uint64_t length = UINT64_C(0);
        (void){summary}(&context, left, right, &length);
    }}
    (void)argv;
    return 0;
}}
"#
    );
    for optimization in ["-O0", "-O2"] {
        let id = NEXT_ID.fetch_add(1, Ordering::Relaxed);
        let stem = format!("semaprax-slice-native-{}-{id}", std::process::id());
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

        let rejection_source = std::env::temp_dir().join(format!("{stem}-reject.c"));
        let rejection_executable =
            std::env::temp_dir().join(format!("{stem}-reject{}", std::env::consts::EXE_SUFFIX));
        std::fs::write(&rejection_source, format!("{generated}\n{rejection_probe}")).unwrap();
        let compiled = Command::new("clang")
            .args([
                "-std=c11",
                optimization,
                "-Wall",
                "-Wextra",
                "-Werror",
                "-DSPX_NO_ENTRY_WRAPPER",
            ])
            .arg(&rejection_source)
            .arg("-o")
            .arg(&rejection_executable)
            .output()
            .unwrap();
        assert!(
            compiled.status.success(),
            "{}",
            String::from_utf8_lossy(&compiled.stderr)
        );
        let oversized = Command::new(&rejection_executable).output().unwrap();
        let malformed = Command::new(&rejection_executable)
            .arg("malformed-empty")
            .output()
            .unwrap();
        let _ = std::fs::remove_file(rejection_source);
        let _ = std::fs::remove_file(rejection_executable);
        assert!(!oversized.status.success(), "oversized roots were admitted");
        assert!(
            !malformed.status.success(),
            "non-normalized empty carrier was admitted"
        );
    }
}

#[test]
fn slice_is_cleanup_inert_and_not_an_aggregate_layout_value() {
    let program = parse(SOURCE, "slice-cleanup.spx").unwrap();
    let resolved = hir::resolve(&program).unwrap();
    let forwarded = resolved
        .functions
        .iter()
        .find(|function| function.id.as_str() == "bytes.forwarded")
        .unwrap();
    assert_eq!(forwarded.params[0].ty, ResolvedType::SliceU8);
    assert!(forwarded
        .cleanup
        .slots
        .iter()
        .all(|slot| slot.ty != ResolvedType::SliceU8));
    assert!(forwarded
        .cleanup_plan
        .slots
        .iter()
        .all(|slot| slot.ty != ResolvedType::SliceU8));
}
