use std::path::Path;
use std::process::Command;

use semaprax::{parse, verify, wasm};

const PROGRAM: &str = r#"
module test.web;
@id("math.add")
fn add(left: i64, right: i64) -> i64
    requires left >= 0
    ensures result == left + right
{
    left + right
}
@id("app.main")
fn main() -> i64
    ensures result == 42 && true
{
    add(19, 23)
}
"#;

#[test]
fn emits_a_valid_browser_webassembly_package() {
    let program = parse(PROGRAM, Path::new("web.spx")).unwrap();
    assert!(verify::verify(&program).is_empty());
    let module = wasm::emit_module(&program).unwrap();
    assert_eq!(&module[..8], b"\0asm\x01\0\0\0");

    let output = std::env::temp_dir().join(format!("semaprax-web-{}", std::process::id()));
    wasm::build_web(&program, &output).unwrap();
    assert!(output.join("app.wasm").is_file());
    assert!(output.join("semaprax.js").is_file());
    assert!(output.join("index.html").is_file());
    assert!(output.join("package.json").is_file());
    assert!(output.join("semaprax.manifest.json").is_file());

    if Command::new("node").arg("--version").output().is_ok() {
        let script = Path::new(env!("CARGO_MANIFEST_DIR")).join("scripts/verify-web.mjs");
        let result = Command::new("node")
            .arg(script)
            .arg(&output)
            .output()
            .unwrap();
        assert!(
            result.status.success(),
            "node failed: {}",
            String::from_utf8_lossy(&result.stderr)
        );
        assert_eq!(String::from_utf8_lossy(&result.stdout).trim(), "42");
    }

    let _ = std::fs::remove_dir_all(output);
}
