use std::path::Path;
use std::process::Command;

use semaprax::{hir, parse, wasm};

const SUCCESS_SOURCE: &str = r#"
module test.owned_byte_record_wasm;

@id("owned.packet")
record Packet {
    @id("owned.packet.left") left: Bytes,
    @id("owned.packet.right") right: Bytes,
    @id("owned.packet.marker") marker: i64,
}

@id("app.main")
fn main() -> i64 {
    let left_source = [1u8, 2u8];
    let right_source = [3u8, 4u8, 5u8];
    let packet = Packet {
        left: bytes_copy(array_as_slice(left_source)),
        right: bytes_copy(array_as_slice(right_source)),
        marker: 37,
    };
    let borrowed = inspect(packet);
    consume(identity(packet)) + borrowed
}

@id("owned.inspect")
fn inspect(packet: borrow Packet) -> i64 {
    match borrow packet {
        Packet { left, right, marker: _ } =>
            if byte_len(bytes_as_slice(left)) == 2usize
                && byte_len(bytes_as_slice(right)) == 3usize { 5 } else { 0 },
    }
}

@id("owned.identity")
fn identity(packet: own Packet) -> Packet { packet }

@id("owned.consume")
fn consume(packet: own Packet) -> i64 {
    match own packet {
        Packet { left, right, marker } =>
            if byte_len(bytes_as_slice(left)) == 2usize
                && byte_len(bytes_as_slice(right)) == 3usize {
                marker
            } else { 0 },
    }
}
"#;

const FAILURE_SOURCE: &str = r#"
module test.owned_byte_record_wasm_failure;

@id("owned.packet")
record Packet {
    @id("owned.packet.left") left: Bytes,
    @id("owned.packet.right") right: Bytes,
}

@id("fail.contract")
fn reject(packet: own Packet) -> i64 requires false { 0 }

@id("app.main")
fn main() -> i64 {
    let left_source = [1u8];
    let right_source = [2u8];
    reject(Packet {
        left: bytes_copy(array_as_slice(left_source)),
        right: bytes_copy(array_as_slice(right_source)),
    })
}
"#;

const PRECOMMIT_FAILURE_SOURCE: &str = r#"
module test.owned_byte_record_wasm_precommit_failure;

@id("owned.packet")
record Packet {
    @id("owned.packet.left") left: Bytes,
    @id("owned.packet.right") right: Bytes,
}

@id("fail.before-commit")
fn fail_before_commit() -> i64 requires false { 0 }

@id("owned.consume-after")
fn consume_after(packet: own Packet, ignored: i64) -> i64 {
    match own packet { Packet { left, right } => ignored, }
}

@id("app.main")
fn main() -> i64 {
    let left_source = [1u8];
    let right_source = [2u8];
    consume_after(Packet {
        left: bytes_copy(array_as_slice(left_source)),
        right: bytes_copy(array_as_slice(right_source)),
    }, fail_before_commit())
}
"#;

fn run_node(source: &str, stem: &str, script: &str) {
    if Command::new("node").arg("--version").output().is_err() {
        return;
    }
    let program = parse(source, Path::new(stem)).unwrap();
    let root = std::env::temp_dir().join(format!("semaprax-{stem}-{}", std::process::id()));
    wasm::build_web(&program, &root).unwrap();
    std::fs::write(root.join("package.json"), "{\"type\":\"module\"}\n").unwrap();
    std::fs::write(root.join("probe.mjs"), script).unwrap();
    let output = Command::new("node")
        .arg("probe.mjs")
        .current_dir(&root)
        .output()
        .unwrap();
    let _ = std::fs::remove_dir_all(&root);
    assert!(
        output.status.success(),
        "stdout={} stderr={}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn wasm_moves_two_owned_fields_and_settles_borrow_before_repeated_own_match() {
    run_node(
        SUCCESS_SOURCE,
        "owned-byte-record-wasm-v1.spx",
        r#"import {readFile} from 'node:fs/promises';
import {instantiateBytes} from './semaprax.js';
const {instance}=await instantiateBytes(await readFile('./app.wasm'),{maxOwnedByteEntries:2});
for(let i=0;i<4;i+=1){const value=instance.exports.semaprax_main();if(value!==42n)throw Error(`semantic-or-settlement:${value}`);}
console.log('owned-byte-record-wasm-v1-ok');
"#,
    );
}

#[test]
fn wasm_failure_cleans_both_projected_tokens_before_reentry() {
    run_node(
        FAILURE_SOURCE,
        "owned-byte-record-wasm-failure-v1.spx",
        r#"import {readFile} from 'node:fs/promises';
import {instantiateBytes} from './semaprax.js';
const {instance}=await instantiateBytes(await readFile('./app.wasm'),{maxOwnedByteEntries:2});
for(let i=0;i<3;i+=1){let failed=false;try{instance.exports.semaprax_main()}catch(error){if(error.message!=='SEMAPRAX contract failure')throw error;failed=true}if(!failed)throw Error('missing-failure');}
console.log('owned-byte-record-wasm-failure-v1-ok');
"#,
    );

    run_node(
        PRECOMMIT_FAILURE_SOURCE,
        "owned-byte-record-wasm-precommit-failure-v1.spx",
        r#"import {readFile} from 'node:fs/promises';
import {instantiateBytes} from './semaprax.js';
const {instance}=await instantiateBytes(await readFile('./app.wasm'),{maxOwnedByteEntries:2});
for(let i=0;i<3;i+=1){let failed=false;try{instance.exports.semaprax_main()}catch(error){if(error.message!=='SEMAPRAX contract failure')throw error;failed=true}if(!failed)throw Error('missing-precommit-failure');}
console.log('owned-byte-record-wasm-precommit-failure-v1-ok');
"#,
    );
}

#[test]
fn hostile_projected_call_epoch_field_identity_drift_is_rejected() {
    let program = parse(
        SUCCESS_SOURCE,
        Path::new("owned-byte-record-wasm-hostile-v1.spx"),
    )
    .unwrap();
    let mut resolved = hir::resolve(&program).unwrap();
    let function = resolved
        .functions
        .iter_mut()
        .find(|function| function.id.as_str() == "app.main")
        .unwrap();
    let shape = function
        .cleanup_plan
        .slots
        .iter_mut()
        .find(|slot| {
            matches!(
                slot.storage,
                semaprax::cleanup_plan::StorageId::CallArgument { .. }
            )
        })
        .map(|slot| &mut slot.field_liveness_shape)
        .unwrap();
    let semaprax::cleanup::FieldLivenessShape::Record { fields, .. } = shape else {
        panic!("call epoch is not projected record storage")
    };
    fields[0].field = hir::DeclarationId::new("hostile.wrong-field");
    assert_eq!(
        wasm::emit_resolved_module(&resolved).unwrap_err().code,
        "SPX-H006"
    );
}
