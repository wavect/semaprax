//! External input charging must not reclassify an internal owned Bytes view.
use semaprax::{codegen, hir, interpreter};
use std::process::Command;
use std::sync::atomic::{AtomicU64, Ordering};

static NEXT: AtomicU64 = AtomicU64::new(0);
const SOURCE: &str = r#"module test.internal_owned_slice;
@id("probe.inspect") fn inspect(value: borrow Slice<u8>) -> i64 {
    if byte_len(value) == 131072usize { 9 } else { 3 }
}
@id("probe.pair") fn pair(text: borrow str, value: borrow Slice<u8>) -> i64 {
    if str_len_bytes(text) == 512usize && byte_len(value) == 65024usize { 5 } else { 0 }
}
@id("app.main") fn main() -> i64 {
    let buffer = bytes_zeroed(131072usize);
    let view = bytes_as_slice(buffer);
    inspect(view)
}
"#;

#[test]
fn internal_owned_views_preserve_capacity_and_external_root_charging() {
    let ast = semaprax::check(SOURCE, "internal-owned-slice.spx").unwrap();
    let resolved = hir::resolve(&ast).unwrap();
    hir::validate(&resolved).unwrap();
    let root = std::env::temp_dir().join(format!(
        "internal-owned-slice-{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    ));
    std::fs::create_dir(&root).unwrap();
    let source = root.join("case.spx");
    std::fs::write(&source, SOURCE).unwrap();
    let outcome = interpreter::interpret(
        &source,
        "app.main",
        &[],
        &interpreter::InterpreterOptions::default(),
    )
    .unwrap();
    interpreter::verify_envelope(&outcome.envelope).unwrap();
    let envelope: serde_json::Value = serde_json::from_str(&outcome.envelope).unwrap();
    assert!(outcome.returned, "{}", outcome.envelope);
    assert_eq!(envelope["payload"]["outcome"]["value"], "9");

    let generated = codegen::emit_hir_c(&resolved).unwrap();
    let probe = r#"
int main(int argc, char **argv) {
    struct spx_status_entry entries[32]; struct spx_context context = {0};
    if (!spx_context_init(&context,UINT64_C(401),entries,32,NULL,NULL,NULL)) return 10;
    int64_t result = 0;
    if (argc == 1) {
        for (unsigned i=0;i<3;++i) {
            if (spx_decl_6170702e6d61696e(&context,&result)!=SPX_STATUS_SUCCESS || result!=9) return 11;
            if (context.call_depth || context.borrowed_str_depth || context.status_arena.length) return 12;
        }
        return 0;
    }
    unsigned char input[65537] = {0}; unsigned char text[512]; memset(text,'x',sizeof text);
    if (!strcmp(argv[1],"slice-exact") || !strcmp(argv[1],"slice-plus-one")) {
        spx_slice_u8_v1 view = {.ptr=input,.len=!strcmp(argv[1],"slice-exact")?65536:65537};
        if (spx_decl_70726f62652e696e7370656374(&context,view,&result)!=SPX_STATUS_SUCCESS || result!=3) return 13;
    } else {
        spx_str_v1 string = {.data=text,.len=512};
        spx_slice_u8_v1 view = {.ptr=input,.len=!strcmp(argv[1],"pair-exact")?65024:65025};
        if (spx_decl_70726f62652e70616972(&context,string,view,&result)!=SPX_STATUS_SUCCESS || result!=5) return 14;
    }
    return context.call_depth || context.borrowed_str_depth ? 15 : 0;
}
"#;
    let c = root.join("case.c");
    std::fs::write(&c, format!("{generated}\n{probe}")).unwrap();
    for optimization in ["-O0", "-O2"] {
        let binary = root.join(format!("case{optimization}"));
        let built = Command::new("clang")
            .args([
                "-std=c11",
                optimization,
                "-Wall",
                "-Wextra",
                "-Werror",
                "-DSPX_NO_ENTRY_WRAPPER",
            ])
            .arg(&c)
            .arg("-o")
            .arg(&binary)
            .output()
            .unwrap();
        assert!(
            built.status.success(),
            "{}",
            String::from_utf8_lossy(&built.stderr)
        );
        for argument in [None, Some("slice-exact"), Some("pair-exact")] {
            let mut command = Command::new(&binary);
            if let Some(argument) = argument {
                command.arg(argument);
            }
            let output = command.output().unwrap();
            assert!(output.status.success(), "{argument:?}: {output:?}");
        }
        for argument in ["slice-plus-one", "pair-plus-one"] {
            let output = Command::new(&binary).arg(argument).output().unwrap();
            assert!(!output.status.success(), "foreign root +1 admitted");
            assert!(
                String::from_utf8_lossy(&output.stderr)
                    .contains("borrowed byte invocation exceeds the cumulative root bound"),
                "{output:?}"
            );
        }
    }
    let bytes = semaprax::wasm::emit_module(&ast).unwrap();
    wasmparser::Validator::new().validate_all(&bytes).unwrap();
    let module = root.join("case.wasm");
    std::fs::write(&module, bytes).unwrap();
    let output = Command::new("node")
        .arg(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/tests/owned_data/owned_leaf_vec/host.js"
        ))
        .arg(&module)
        .arg("0")
        .arg("9")
        .output()
        .unwrap();
    assert!(output.status.success(), "{output:?}");
    std::fs::remove_dir_all(root).unwrap();
}
