use std::path::Path;
use std::process::Command;

use semaprax::hir::{self, DeclarationId, ResolvedExprKind};
use semaprax::{codegen, parse};

fn resolve(source: &str) -> semaprax::hir::ResolvedProgram {
    let program = parse(source, Path::new("hir-native.spx")).unwrap();
    hir::resolve(&program).unwrap()
}

#[test]
fn native_symbols_and_calls_follow_stable_declaration_ids() {
    let before = resolve(
        r#"
module test.hir_native;
@id("math.increment")
fn increment(value: i64) -> i64 { value + 1 }
@id("app.main")
fn main() -> i64 { increment(41) }
"#,
    );
    let mut after = resolve(
        r#"
module test.hir_native;
@id("math.increment")
fn renamed(value: i64) -> i64 { value + 1 }
@id("app.main")
fn main() -> i64 { renamed(41) }
"#,
    );
    let entrypoint = after.entrypoint.clone();
    after
        .functions
        .iter_mut()
        .find(|function| function.id == entrypoint)
        .unwrap()
        .name = "entry_display_metadata".to_owned();

    let before_c = codegen::emit_hir_c(&before).unwrap();
    let after_c = codegen::emit_hir_c(&after).unwrap();
    assert_eq!(before_c, after_c);
    assert!(before_c.contains("spx_decl_6d6174682e696e6372656d656e74"));
    assert!(!before_c.contains("increment"));
}

#[test]
fn native_hir_lowering_rejects_a_missing_entrypoint_identity() {
    let mut program = resolve(
        r#"
module test.hir_native_entry;
@id("app.main")
fn main() -> i64 { 42 }
"#,
    );
    program.entrypoint = DeclarationId::new("missing.entrypoint");

    assert_eq!(codegen::emit_hir_c(&program).unwrap_err().code, "SPX-H006");
}

#[test]
fn native_hir_lowering_rejects_an_unindexed_callee_without_panicking() {
    let mut program = resolve(
        r#"
module test.hir_native_invalid;
@id("math.answer")
fn answer() -> i64 { 42 }
@id("app.main")
fn main() -> i64 { answer() }
"#,
    );
    let main = program
        .functions
        .iter_mut()
        .find(|function| function.name == "main")
        .unwrap();
    let ResolvedExprKind::Block { tail, .. } = &mut main.body.kind else {
        panic!("expected function body block");
    };
    let ResolvedExprKind::Call { callee, .. } = &mut tail.kind else {
        panic!("expected direct call in body tail");
    };
    *callee = DeclarationId::new("missing.function");

    let diagnostic = codegen::emit_hir_c(&program).unwrap_err();
    assert_eq!(diagnostic.code, "SPX-H006");
    assert!(diagnostic.message.contains("missing.function"));
}

#[test]
fn native_hir_contract_labels_escape_control_characters() {
    let program = resolve(
        r#"
module test.hir_native_escape;
@id("app.\nmain")
fn main() -> i64 ensures result == 42 { 42 }
"#,
    );
    let output = codegen::emit_hir_c(&program).unwrap();

    assert!(output.contains(r"app.\nmain"));
    assert!(!output.contains("app.\nmain"));

    if Command::new("clang").arg("--version").output().is_err() {
        return;
    }
    let stem = format!("semaprax-hir-escape-{}", std::process::id());
    let source = std::env::temp_dir().join(format!("{stem}.c"));
    let executable = std::env::temp_dir().join(format!("{stem}{}", std::env::consts::EXE_SUFFIX));
    std::fs::write(&source, output).unwrap();
    let compiled = Command::new("clang")
        .args(["-std=c11", "-Wall", "-Wextra", "-Werror"])
        .arg(&source)
        .arg("-o")
        .arg(&executable)
        .output()
        .unwrap();
    let _ = std::fs::remove_file(&source);
    let _ = std::fs::remove_file(&executable);
    assert!(
        compiled.status.success(),
        "generated C did not compile: {}",
        String::from_utf8_lossy(&compiled.stderr)
    );
}
