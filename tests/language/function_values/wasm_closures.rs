//! Actual closure carrier lifetime and left-to-right invocation evidence.
use std::process::Command;
use std::sync::atomic::{AtomicU64, Ordering};
static NEXT: AtomicU64 = AtomicU64::new(0);

const SOURCE: &str = r#"
module test.wasm_closure_order;
@id("closure.make") fn make(offset:i64)->fn(i64)->i64 { fn(input:i64)->i64 { offset+input } }
@id("app.main") fn main()->i64 {
    let mut captured=7;
    let callback=fn(first:i64,second:i64)->i64 { captured+first+second };
    let observed=callback(captured,{ captured=99; let other=make(100); other(1)-100 });
    observed+captured
}
"#;

#[test]
fn wasm_closures_snapshot_callable_before_later_argument_mutation_and_nested_frames() {
    let (program, _) = super::checked(SOURCE);
    let module = semaprax::wasm::emit_module(&program).unwrap();
    assert_eq!(module, semaprax::wasm::emit_module(&program).unwrap());
    let available = Command::new("node").arg("--version").output().is_ok();
    assert!(available || std::env::var_os("SPX_REQUIRE_NODE").is_none());
    if !available {
        return;
    }
    let root = std::env::temp_dir().join(format!(
        "semaprax-wasm-closure-order-{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    ));
    semaprax::wasm::build_web(&program, &root).unwrap();
    std::fs::write(root.join("package.json"), "{\"type\":\"module\"}\n").unwrap();
    std::fs::write(root.join("probe.mjs"), r#"
import {readFile} from 'node:fs/promises';
import {instantiateBytes} from './semaprax.js';
const {instance}=await instantiateBytes(await readFile('./app.wasm'));
for(let run=0;run<3;run++) if(instance.exports.semaprax_main()!==114n) throw Error('closure order or frame lifetime changed');
"#).unwrap();
    let output = Command::new("node")
        .arg("probe.mjs")
        .current_dir(&root)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn wasm_closures_keep_function_reassignment_outside_mutation_v1() {
    let source = r#"
module test.wasm_closure_assignment;
@id("app.main") fn main()->i64 {
    let offset=1;
    let mut callback=fn(value:i64)->i64 { offset+value };
    callback=fn(value:i64)->i64 { offset-value };
    callback(2)
}
"#;
    let errors = semaprax::check(source, "closure-assignment.spx").unwrap_err();
    assert!(
        errors.iter().any(|error| error.code == "SPX-U105"),
        "{errors:?}"
    );
}
