use semaprax::{codegen, format, graph, hir, interpreter, parse, verify};
use std::process::Command;
const SOURCE: &str = r#"module ordering;
@id("ordering.main") fn main()->i64 {
 let a="a";let b="ab";let nul="a\u{0}b";let longer="a\u{0}c";let bmp="\u{e000}";let supplementary="\u{10000}";
 if a<b && a<=a && b>a && b>=b && nul<longer && bmp<supplementary && !(b<a) {42} else {1}
}
"#;
#[test]
fn utf8_ordering_handles_prefix_nul_and_supplementary_scalars() {
    let ast = parse(SOURCE, "ordering.spx").unwrap();
    assert!(verify::verify(&ast).is_empty());
    let canonical = format::canonical(&ast);
    let round = parse(&canonical, "ordering.spx").unwrap();
    assert_eq!(canonical, format::canonical(&round));
    hir::validate(&hir::resolve(&ast).unwrap()).unwrap();
    graph::verify_json(&ast, &graph::to_json(&ast).unwrap()).unwrap();
    let dir = std::env::temp_dir().join(format!("semaprax-string-ordering-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("ordering.spx");
    std::fs::write(&path, SOURCE).unwrap();
    let result = interpreter::interpret(
        &path,
        "ordering.main",
        &[],
        &interpreter::InterpreterOptions::default(),
    )
    .unwrap();
    assert!(
        result.envelope.contains("\"value\":\"42\""),
        "{}",
        result.envelope
    );
    let c = dir.join("ordering.c");
    std::fs::write(&c, codegen::emit_c(&ast).unwrap()).unwrap();
    for optimization in ["-O0", "-O2"] {
        let binary = dir.join(format!("ordering{optimization}"));
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
        let output = Command::new(binary).output().unwrap();
        assert!(output.status.success());
        assert_eq!(String::from_utf8_lossy(&output.stdout).trim(), "42");
    }
    std::fs::remove_dir_all(dir).unwrap();
}
#[test]
fn string_ordering_requires_matching_operand_types_and_keeps_arithmetic_closed() {
    for (expression, code) in [("\"a\"<1", "SPX-T208"), ("\"a\"+\"b\"", "SPX-T250")] {
        let source = format!("module negative; fn main()->i64 {{ let ignored={expression};0 }}");
        let program = parse(&source, "ordering-negative.spx").unwrap();
        let diagnostics = verify::verify(&program);
        assert!(
            diagnostics.iter().any(|e| e.code == code),
            "{diagnostics:?}"
        );
    }
}
