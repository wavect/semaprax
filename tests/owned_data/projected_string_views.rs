//! Same-source rooted String loans, no hidden clone, and failure settlement.
use semaprax::{codegen, interpreter, wasm};
use std::process::Command;
use std::sync::atomic::{AtomicU64, Ordering};
static NEXT: AtomicU64 = AtomicU64::new(0);
const SOURCE: &str = include_str!("../fixtures/projected-string-views.spx");

fn directory(label: &str) -> std::path::PathBuf {
    let path = std::env::temp_dir().join(format!(
        "projected-string-{}-{label}-{}",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    ));
    std::fs::create_dir_all(&path).unwrap();
    path
}
fn strict_wasm(ast: &semaprax::ast::Program, root: &std::path::Path, status: &str, expected: &str) {
    let bytes = wasm::emit_module(ast).unwrap();
    wasmparser::Validator::new().validate_all(&bytes).unwrap();
    let module = root.join("views.wasm");
    std::fs::write(&module, bytes).unwrap();
    let original = include_str!("owned_leaf_vec/host.js");
    let host=original.replace("const alloc = carrier => {", "const alloc = carrier => {\n  if(split(carrier).origin & 0x80000000)throw Error('implicit owning clone during projected view');");
    assert_ne!(
        host, original,
        "clone guard must join its exact host allocation seam"
    );
    let output = Command::new("node")
        .arg("-e")
        .arg(host)
        // The shared file-mode host reads the Wasm path at argv[2].
        .arg("semaprax-owned-host")
        .arg(module)
        .args([status, expected, "projected-view"])
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn own_and_borrowed_nested_roots_repeated_views_do_not_clone_and_settle() {
    let direct = SOURCE
        .replace(
            "@id(\"view.outer.inner\") inner:Inner,",
            "@id(\"view.outer.text\") text:string,@id(\"view.outer.sibling\") sibling:string,",
        )
        .replace(
            r#"inner:Inner{text:"A\u{0}é",sibling:"tail"}"#,
            r#"text:"A\u{0}é",sibling:"tail""#,
        )
        .replace("value.inner.text", "value.text");
    assert_ne!(direct, SOURCE);
    // No standalone Str expression or bound Slice may accidentally select the
    // native helpers needed solely by this inline fused view.
    let inline = format!(
        "{}\n@id(\"view.inline\") fn inspect(value:borrow Outer)->i64 {{i64_from_usize(byte_len(str_as_bytes(string_as_str(value.inner.text))))}}\n@id(\"app.main\") fn main()->i64 {{let value=make();if inspect(value)==4 {{42}}else{{0}}}}",
        SOURCE.split("@id(\"view.measure\")").next().unwrap()
    );
    for (label, source) in [
        ("nested", SOURCE),
        ("direct", direct.as_str()),
        ("inline", inline.as_str()),
    ] {
        let root = directory(label);
        let path = root.join("app.spx");
        std::fs::write(&path, source).unwrap();
        let ast = semaprax::check(source, &path).unwrap();
        for _ in 0..3 {
            let result = interpreter::interpret(
                &path,
                "app.main",
                &[],
                &interpreter::InterpreterOptions::default(),
            )
            .unwrap();
            interpreter::verify_envelope(&result.envelope).unwrap();
            let envelope: serde_json::Value = serde_json::from_str(&result.envelope).unwrap();
            assert!(result.returned, "{}", result.envelope);
            assert_eq!(envelope["payload"]["outcome"]["value"], "42");
        }
        let original = codegen::emit_c(&ast).unwrap();
        let generated=original.replace("char *spx_string_clone(const char *spx_source) {", "char *spx_string_clone(const char *spx_source) {\n    abort(); /* any implicit view clone fails this physical gate */");
        assert_ne!(
            generated, original,
            "clone guard must join the exact native helper"
        );
        super::owned_collection_outcome::run_native_generated(generated, &root);
        strict_wasm(&ast, &root, "0", "42");
        std::fs::remove_dir_all(root).unwrap();
    }
}

#[test]
fn projected_view_then_owned_staging_or_callee_failure_keeps_first_status_and_settles() {
    let prefix = SOURCE.split("@id(\"app.main\")").next().unwrap();
    for (label,body) in [
        ("staging","let value=make();let view=string_as_str(value.inner.text);let count=str_len_bytes(view);sink(value,boom())"),
        ("callee","let value=make();fail(value)"),
        ("range","let value=make();let bytes=str_as_bytes(string_as_str(value.inner.text));let invalid=byte_range(bytes,0usize,5usize);0"),
    ] {
        let source=format!("{prefix}\n@id(\"view.boom\") fn boom()->i64 {{9223372036854775807+1}}\n@id(\"view.fail\") fn fail(value:own Outer)->i64 {{let view=string_as_str(value.inner.text);let count=str_len_bytes(view);boom()}}\n@id(\"app.main\") fn main()->i64 {{{body}}}");
        let root=directory(label);let path=root.join("app.spx");std::fs::write(&path,&source).unwrap();
        let ast=semaprax::check(&source,&path).unwrap();
        let (domain,code,status)=if label=="range" {("semaprax.byte-range.v1",2,"12")}else{("semaprax.arithmetic.v1",1,"1")};
        for _ in 0..3 {
            let result=interpreter::interpret(&path,"app.main",&[],&interpreter::InterpreterOptions::default()).unwrap();
            assert!(!result.returned,"{}",result.envelope);
            interpreter::verify_envelope(&result.envelope).unwrap();
            assert!(result.envelope.contains(&format!("\"domain_id\":\"{domain}\"")));
            assert!(result.envelope.contains(&format!("\"code\":{code}")));
        }
        super::nested_collection_record::failure::native_failure(&ast,&root,domain,code);
        strict_wasm(&ast,&root,status,"0");std::fs::remove_dir_all(root).unwrap();
    }
}
