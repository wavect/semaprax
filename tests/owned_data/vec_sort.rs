use semaprax::{codegen, graph, hir, interpreter, parse, verify, wasm};
use std::process::Command;

fn source() -> String {
    let mut source = String::from("module sort.test; @id(\"sort.main\") fn main()->i64 { ");
    let mut checks = Vec::new();
    for (index, (ty, values, first, last)) in [
        ("i64", "3,-7,3,0", "-7", "3"),
        ("i32", "3i32,-7i32,3i32,0i32", "-7i32", "3i32"),
        ("u8", "255u8,0u8,17u8,17u8", "0u8", "255u8"),
        (
            "usize",
            "99usize,0usize,17usize,17usize",
            "0usize",
            "99usize",
        ),
        ("char", "'🦀','a','é','a'", "'a'", "'🦀'"),
        ("f32", "3.5f32,-7.5f32,0.0f32,3.5f32", "-7.5f32", "3.5f32"),
        ("f64", "3.5,-7.5,0.0,3.5", "-7.5", "3.5"),
        ("bool", "true,false,true,false", "false", "true"),
    ]
    .into_iter()
    .enumerate()
    {
        source.push_str(&format!(
            "let mut v{index}=vec_with_capacity<{ty}>(4usize);"
        ));
        for value in values.split(',') {
            source.push_str(&format!("v{index}=vec_push<{ty}>(v{index},{value});"));
        }
        source.push_str(&format!("v{index}=vec_sort<{ty}>(v{index});"));
        checks.push(format!("vec_get<{ty}>(v{index},0usize)=={first} && vec_get<{ty}>(v{index},3usize)=={last} && vec_len<{ty}>(v{index})==4usize && vec_capacity<{ty}>(v{index})==4usize"));
    }
    source.push_str("let mut empty=vec_with_capacity<i64>(0usize); empty=vec_sort<i64>(empty); let mut i=0; while i<2 { empty=vec_sort<i64>(empty); i=i+1; 0 } ");
    source.push_str(&format!(
        "if {} && vec_len<i64>(empty)==0usize {{42}} else {{1}} }}",
        checks.join(" && ")
    ));
    source
}

#[test]
fn all_copy_scalar_sorts_preserve_owners_and_agree_across_engines() {
    let text = source();
    let ast = parse(&text, "sort.spx").unwrap();
    assert!(verify::verify(&ast).is_empty());
    let canonical = semaprax::format::canonical(&ast);
    let round = parse(&canonical, "sort.spx").unwrap();
    assert_eq!(canonical, semaprax::format::canonical(&round));
    let resolved = hir::resolve(&ast).unwrap();
    hir::validate(&resolved).unwrap();
    let meaning = graph::to_json(&ast).unwrap();
    assert!(meaning.contains("semaprax.prelude.v12"));
    assert!(meaning.contains("core.vec.sort"));
    let root = std::env::temp_dir().join(format!("semaprax-vec-sort-{}", std::process::id()));
    std::fs::create_dir_all(&root).unwrap();
    let path = root.join("sort.spx");
    std::fs::write(&path, &text).unwrap();
    let result = interpreter::interpret(
        &path,
        "sort.main",
        &[],
        &interpreter::InterpreterOptions::default(),
    )
    .unwrap();
    assert!(
        result.envelope.contains("\"value\":\"42\""),
        "{}",
        result.envelope
    );
    let c = root.join("sort.c");
    std::fs::write(&c, codegen::emit_c(&ast).unwrap()).unwrap();
    for optimization in ["-O0", "-O2"] {
        let binary = root.join(format!("sort{optimization}"));
        let output = Command::new("clang")
            .args(["-std=c11", "-Wall", "-Wextra", "-Werror", optimization])
            .arg(&c)
            .arg("-o")
            .arg(&binary)
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        let result = Command::new(binary).output().unwrap();
        assert!(result.status.success());
        assert_eq!(String::from_utf8_lossy(&result.stdout).trim(), "42");
    }
    let wasm = wasm::emit_module(&ast).unwrap();
    wasmparser::Validator::new().validate_all(&wasm).unwrap();
    let module = root.join("sort.wasm");
    std::fs::write(&module, wasm).unwrap();
    let script = include_str!("vec_sort_host.js");
    let output = Command::new("node")
        .arg("-e")
        .arg(script)
        .arg(module)
        .output()
        .unwrap();
    std::fs::remove_dir_all(root).unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn sort_rejects_owned_payloads_wrong_arity_and_reuse_after_move() {
    for (text, code) in [
        (
            "module t; fn main()->i64 { let v=vec_with_capacity<Bytes>(0usize); let x=vec_sort<Bytes>(v); 0 }",
            "SPX-T281",
        ),
        (
            "module t; fn main()->i64 { let v=vec_with_capacity<i64>(0usize); let x=vec_sort<i64>(v,1); 0 }",
            "SPX-T281",
        ),
        (
            "module t; fn main()->i64 { let v=vec_with_capacity<i64>(0usize); let x=vec_sort<i64>(v); let n=vec_len<i64>(v); 0 }",
            "SPX-O101",
        ),
    ] {
        let program = parse(text, "sort-negative.spx").unwrap();
        let diagnostics = verify::verify(&program);
        assert!(
            diagnostics.iter().any(|error| error.code == code),
            "{text}: {diagnostics:?}"
        );
    }
}
