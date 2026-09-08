//! Source regressions for immutable Copy-scalar closure snapshots.

use semaprax::hir::{self, ResolvedExprKind, ResolvedStatement};
use semaprax::interpreter::{self, InterpreterOptions};
use std::sync::atomic::{AtomicU64, Ordering};

static NEXT_FIXTURE: AtomicU64 = AtomicU64::new(0);

const SNAPSHOT_SOURCE: &str = r#"
module test.function_value_closures;
@id("closure.snapshot") fn snapshot() -> i64 {
    let mut captured = 7;
    let callback = fn(value: i64) -> i64 { captured + value };
    captured = 100;
    callback(5)
}
@id("closure.shadow") fn shadow(captured: i64) -> i64 {
    let callback = fn(captured: i64) -> i64 { captured + 1 };
    callback(41)
}
@id("closure.return") fn return_closure(offset: i64) -> fn(i64) -> i64 {
    fn(value: i64) -> i64 { offset + value }
}
@id("app.main") fn main() -> i64 {
    let escaped = return_closure(40);
    snapshot() + shadow(99) + escaped(2)
}
"#;

fn interpret_main(source: &str) -> String {
    let sequence = NEXT_FIXTURE.fetch_add(1, Ordering::Relaxed);
    let path = std::env::temp_dir().join(format!(
        "semaprax-function-value-closure-{}-{sequence}.spx",
        std::process::id()
    ));
    let (program, _) = super::checked(source);
    std::fs::write(&path, semaprax::format::canonical(&program)).unwrap();
    let result =
        interpreter::interpret(&path, "app.main", &[], &InterpreterOptions::default()).unwrap();
    let _ = std::fs::remove_file(&path);
    result.envelope
}

#[test]
fn closures_snapshot_mutable_copy_values_shadow_parameters_and_escape_privately() {
    let (program, resolved) = super::checked(SNAPSHOT_SOURCE);
    let snapshot = resolved
        .functions
        .iter()
        .find(|function| function.id.as_str() == "closure.snapshot")
        .unwrap();
    assert_eq!(closure_initializer_capture_count(snapshot), 1);
    let shadow = resolved
        .functions
        .iter()
        .find(|function| function.id.as_str() == "closure.shadow")
        .unwrap();
    assert_eq!(closure_initializer_capture_count(shadow), 0);
    for _ in 0..2 {
        assert!(
            interpret_main(SNAPSHOT_SOURCE).contains("\"value\":\"96\""),
            "a closure captures the pre-mutation Copy value and may escape a private function"
        );
    }
    let graph = semaprax::graph::to_json(&program).unwrap();
    assert!(graph.contains("semaprax.graph.v37"), "{graph}");
    assert!(graph.contains("\"closure_definitions\""), "{graph}");
    semaprax::graph::verify_json(&program, &graph).unwrap();
    assert_eq!(graph, semaprax::graph::to_json(&program).unwrap());
    let forged_kind = graph.replacen("\"kind\":\"closure\"", "\"kind\":\"function_reference\"", 1);
    assert_ne!(forged_kind, graph);
    assert!(semaprax::graph::verify_json(&program, &forged_kind).is_err());
    let forged_capture = graph.replacen("\"name\":\"captured\"", "\"name\":\"forged\"", 1);
    assert_ne!(forged_capture, graph);
    assert!(semaprax::graph::verify_json(&program, &forged_capture).is_err());
}

fn closure_initializer_capture_count(function: &hir::ResolvedFunction) -> usize {
    let ResolvedExprKind::Block { statements, .. } = &function.body.kind else {
        panic!("expected a function block");
    };
    statements
        .iter()
        .find_map(|statement| {
            let ResolvedStatement::Let { value, .. } = statement else {
                return None;
            };
            let ResolvedExprKind::Closure { captures, .. } = &value.kind else {
                return None;
            };
            Some(captures.len())
        })
        .expect("expected the closure initializer")
}

#[test]
fn closures_snapshot_every_copy_scalar_kind() {
    for (ty, literal) in [
        ("i64", "7"),
        ("i32", "7i32"),
        ("u8", "7u8"),
        ("usize", "7usize"),
        ("char", "'x'"),
        ("f32", "7.0f32"),
        ("f64", "7.0f64"),
        ("bool", "true"),
    ] {
        super::checked(&format!(
            "module test.function_value_closure_scalars;\n@id(\"closure.capture\") fn capture(value: {ty}) -> {ty} {{ let callback = fn() -> {ty} {{ value }}; callback() }}\n@id(\"app.main\") fn main() -> i64 {{ let observed = capture({literal}); 0 }}\n"
        ));
    }
}

#[test]
fn closures_reject_owned_captures_and_fixed_parameter_capture_bounds() {
    let owned = r#"
module test.function_value_closure_owned;
@id("closure.bad") fn bad(value: own Bytes) -> fn(i64) -> i64 {
    fn(input: i64) -> i64 { if true { value } else { value } }
}
@id("app.main") fn main() -> i64 { 0 }
"#;
    let parameter_bound = r#"
module test.function_value_closure_parameters;
@id("closure.bad") fn bad() -> i64 {
    let callback = fn(a:i64,b:i64,c:i64,d:i64,e:i64,f:i64,g:i64,h:i64,i:i64) -> i64 { 0 };
    0
}
@id("app.main") fn main() -> i64 { 0 }
"#;
    let capture_bound = r#"
module test.function_value_closure_captures;
@id("closure.bad") fn bad() -> i64 {
    let a=1; let b=2; let c=3; let d=4; let e=5;
    let f=6; let g=7; let h=8; let i=9;
    let callback=fn() -> i64 { a+b+c+d+e+f+g+h+i };
    callback()
}
@id("app.main") fn main() -> i64 { 0 }
"#;
    for source in [owned, parameter_bound, capture_bound] {
        let diagnostics = semaprax::check(source, "bad-function-value-closure.spx").unwrap_err();
        assert!(
            diagnostics
                .iter()
                .any(|diagnostic| diagnostic.code == "SPX-T288"),
            "{diagnostics:?}"
        );
    }
}
