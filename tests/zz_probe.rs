use semaprax::{hir, parse};
#[test]
fn end_to_end_probe() {
    let source = r#"
module test.probe;
@id("p.classify")
fn classify(x: i64) -> i64 {
    match x {
        0 => 100,
        -1 | -2 => 200,
        7 if x > 3 => 300,
        n => n + 1,
    }
}
@id("main") fn main() -> i64 { classify(0) + classify(-1) + classify(9) + classify(5) }
"#;
    let program = parse(source, std::path::Path::new("probe.spx")).unwrap();
    let diagnostics = semaprax::verify::verify(&program);
    assert!(diagnostics.is_empty(), "{diagnostics:?}");
    let resolved = hir::resolve(&program).unwrap();
    let function = resolved.functions.iter().find(|f| f.id.as_str() == "p.classify").unwrap();
    assert!(function.cleanup.slots.is_empty());
    assert!(function.cleanup_plan.slots.is_empty());
    let _c = semaprax::codegen::emit_c(&program).unwrap();
    let _w = semaprax::wasm::emit_module(&program).unwrap();
    println!("E2E-OK");
}
