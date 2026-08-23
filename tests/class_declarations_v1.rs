use std::path::Path;
use std::process::Command;

use semaprax::{codegen, format, graph, parse, verify, wasm};

const CLASSES: &str = r#"module test.classes;

@id("demo.point")
class Point {
    @id("demo.point.x")
    x: i64,
    @id("demo.point.tag")
    tag: bool,

    @id("demo.point.get")
    fn get(self: Point) -> i64
{
        self.x
    }

    @id("demo.point.combine")
    fn combine(self: Point, other: Point) -> i64
{
        if self.tag && other.tag { self.x + other.x } else { self.x - other.x }
    }
}

@id("app.main")
fn main() -> i64
{
    let left = Point { x: 20, tag: true };
    let right = Point { x: 22, tag: true };
    let mixed = Point { x: 50, tag: false };
    if left.get() == 20 && left.combine(right) == 42 && left.combine(mixed) == -30 { 42 } else { 1 }
}
"#;

const INHERITANCE: &str = r#"module test.inheritance;

@id("demo.a")
class A { @id("demo.a.x") x: i64, }

@id("demo.b")
class B : A { @id("demo.b.y") y: i64, }

@id("app.main")
fn main() -> i64 { 0 }
"#;

fn parse_ok(source: &str) -> semaprax::ast::Program {
    parse(source, Path::new("classes.spx")).expect("class program must parse")
}

#[test]
fn class_programs_round_trip_canonically() {
    let program = parse_ok(CLASSES);
    assert!(verify::verify(&program).is_empty());
    let canonical = format::canonical(&program);
    assert_eq!(canonical, CLASSES);
    let reparsed = parse(&canonical, Path::new("classes-canonical.spx")).unwrap();
    assert_eq!(format::canonical(&reparsed), canonical);
}

#[test]
fn class_graph_json_exposes_deterministic_class_nodes() {
    let program = parse_ok(CLASSES);
    let json = graph::to_json(&program).unwrap();
    assert!(json.contains("\"kind\":\"class\""), "{json}");
    assert!(json.contains("\"id\":\"demo.point\""));
    assert!(json.contains("\"id\":\"demo.point.get\""));
    assert!(json.contains("\"id\":\"demo.point.x\""));
    // Non-class graphs stay untouched by the additive class projection.
    let scalar = parse(
        "module t;\n@id(\"t.main\") fn main() -> i64 { 0 }\n",
        Path::new("scalar.spx"),
    )
    .unwrap();
    let scalar_json = graph::to_json(&scalar).unwrap();
    assert!(!scalar_json.contains("\"kind\":\"class\""));
}

#[test]
fn native_class_methods_execute_identically_at_o0_and_o2() {
    if !command_available("clang") {
        return;
    }
    let program = parse_ok(CLASSES);
    let generated = codegen::emit_c(&program).unwrap();
    use std::fmt::Write as _;
    let mut hex = String::new();
    for byte in b"app.main" {
        write!(hex, "{byte:02x}").unwrap();
    }
    let main_symbol = format!("spx_decl_{hex}");
    let probe = format!(
        r#"
int main(void) {{
    struct spx_status_entry entries[UINT32_C(32)];
    struct spx_context context = {{0}};
    if (!spx_context_init(&context, UINT64_C(64), entries, UINT32_C(32), NULL, NULL, NULL)) return 10;
    int64_t result = 0;
    if ({main}(&context, &result) != SPX_STATUS_SUCCESS) return 11;
    return (int)result;
}}
"#,
        main = main_symbol,
    );
    for optimization in ["-O0", "-O2"] {
        let id = unique_id();
        let stem = format!("semaprax-class-native-{id}");
        let source = std::env::temp_dir().join(format!("{stem}.c"));
        std::fs::write(&source, format!("{generated}\n{probe}")).unwrap();
        let executable =
            std::env::temp_dir().join(format!("{stem}{}", std::env::consts::EXE_SUFFIX));
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
            "class C failed at {optimization}: {}",
            String::from_utf8_lossy(&compiled.stderr)
        );
        let executed = Command::new(&executable).output().unwrap();
        let _ = std::fs::remove_file(&source);
        let _ = std::fs::remove_file(&executable);
        assert_eq!(
            executed.status.code(),
            Some(42),
            "class program exited unexpectedly at {optimization}: {}",
            String::from_utf8_lossy(&executed.stderr)
        );
    }
}

#[test]
fn wasm_class_programs_match_native_results_in_node() {
    if !command_available("node") {
        return;
    }
    let program = parse_ok(CLASSES);
    let id = unique_id();
    let output = std::env::temp_dir().join(format!("semaprax-class-web-{id}"));
    wasm::build_web(&program, &output).unwrap();
    let script = Path::new(env!("CARGO_MANIFEST_DIR")).join("scripts/verify-web.mjs");
    let result = Command::new("node").arg(script).arg(&output).output();
    let _ = std::fs::remove_dir_all(&output);
    let result = result.unwrap();
    assert!(
        result.status.success(),
        "node failed: {}",
        String::from_utf8_lossy(&result.stderr)
    );
    assert_eq!(String::from_utf8_lossy(&result.stdout).trim(), "42");
}

#[test]
fn inheritance_syntax_is_rejected_with_a_stable_diagnostic() {
    let error = parse(INHERITANCE, Path::new("inheritance.spx")).unwrap_err();
    assert_eq!(error.code, "SPX-P106");
    assert!(error.message.contains("inheritance"));
}

fn command_available(name: &str) -> bool {
    Command::new(name)
        .arg("--version")
        .output()
        .map(|output| output.status.success())
        .unwrap_or(false)
}

fn unique_id() -> u64 {
    use std::sync::atomic::{AtomicU64, Ordering};
    static NEXT: AtomicU64 = AtomicU64::new(0);
    let id = NEXT.fetch_add(1, Ordering::Relaxed);
    (std::process::id() as u64) << 8 | id
}
