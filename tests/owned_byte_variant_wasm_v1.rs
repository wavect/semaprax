use std::path::Path;
use std::process::Command;

use semaprax::{parse, wasm};

const SOURCE: &str = include_str!("owned_byte_variant_v1_fixture.spx");

fn run_node(source: &str, stem: &str, script: &str) {
    if Command::new("node").arg("--version").output().is_err() {
        return;
    }
    let program = parse(source, Path::new("owned-byte-variant-wasm-v1.spx")).unwrap();
    let root = std::env::temp_dir().join(format!(
        "semaprax-owned-byte-variant-wasm-{}-{stem}",
        std::process::id(),
    ));
    std::fs::create_dir_all(&root).unwrap();
    wasm::build_web(&program, &root).unwrap();
    std::fs::write(root.join("package.json"), "{\"type\":\"module\"}\n").unwrap();
    std::fs::write(root.join("probe.mjs"), script).unwrap();
    let output = Command::new("node")
        .arg("probe.mjs")
        .current_dir(&root)
        .output()
        .unwrap();
    let _ = std::fs::remove_dir_all(root);
    assert!(
        output.status.success(),
        "stdout={} stderr={}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

fn failure_source(entry: &str) -> String {
    let success = SOURCE.replacen(
        "@id(\"app.main\")\nfn main()",
        "@id(\"sum.success-main\")\nfn success_main()",
        1,
    );
    format!("{success}\n@id(\"app.main\")\nfn main() -> i64 {{ {entry}() }}\n")
}

#[test]
fn owned_byte_variants_execute_repeatedly_with_tight_token_capacity() {
    run_node(
        SOURCE,
        "success",
        r#"import {readFile} from 'node:fs/promises';
import {instantiateBytes} from './semaprax.js';
const {instance}=await instantiateBytes(await readFile('./app.wasm'),{maxOwnedByteEntries:3});
for(let i=0;i<6;i+=1){const value=instance.exports.semaprax_main();if(value!==132n)throw Error(`semantic-or-settlement:${value}`);}
console.log('owned-byte-variant-wasm-v1-ok');
"#,
    );
}

#[test]
fn owned_byte_variant_failures_settle_after_call_commit_and_inside_owned_match() {
    for (entry, stem) in [
        ("trigger_call_failure", "post-commit"),
        ("trigger_match_failure", "match-own"),
    ] {
        let source = failure_source(entry);
        run_node(
            &source,
            stem,
            r#"import {readFile} from 'node:fs/promises';
import {instantiateBytes} from './semaprax.js';
const {instance}=await instantiateBytes(await readFile('./app.wasm'),{maxOwnedByteEntries:1});
for(let i=0;i<6;i+=1){let failed=false;try{instance.exports.semaprax_main()}catch(error){if(error.message!=='SEMAPRAX checked arithmetic failure: addition overflow')throw error;failed=true}if(!failed)throw Error('missing-owned-variant-failure');}
console.log('owned-byte-variant-wasm-v1-failure-ok');
"#,
        );
    }
}
