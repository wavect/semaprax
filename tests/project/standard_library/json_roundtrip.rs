//! A private owned-data consumer composes JSON decoding and quoting through
//! `std.io` cursors. The first staged call consumes the source Reader before
//! the second stage borrows its decoded replacement.

use std::process::Command;

use semaprax::{codegen, project, wasm};

const MANIFEST: &str = "schema = \"semaprax.manifest.v1\"\n\n[package]\nname = \"json-roundtrip-consumer\"\nversion = \"0.1.0\"\nprofile = \"owned-data-api.v1\"\n\n[modules]\nentry = \"consumer.app\"\nsources = [\"src/app.spx\", \"src/tests.spx\"]\ntests = [\"consumer.tests\"]\n\n[exports]\nweb = []\n\n[dependencies]\nstd.data.json.dec = \"=0.1.0\"\nstd.data.json.write = \"=0.1.0\"\nstd.io = \"=0.1.0\"\n";

const APP: &str = r#"module consumer.app;
use type @id("std.io.reader") from std.io as Reader;
use type @id("std.io.writer") from std.io as Writer;
use function @id("std.data.json.dec.decode-into") from std.data.json.dec as decode_into;
use function @id("std.io.reader.from-bytes") from std.io as reader_from_bytes;
use function @id("std.io.writer.finish") from std.io as writer_finish;
use function @id("std.io.writer.from-bytes") from std.io as writer_from_bytes;
use function @id("std.data.json.write.quoted-into") from std.data.json.write as quoted_into;

@id("consumer.decoded-reader")
fn decoded_reader(input: own Reader, output: own Writer) -> Reader
{
    let decoded = decode_into(input, output);
    reader_from_bytes(writer_finish(decoded))
}

@id("consumer.main")
fn main() -> i64
{
    let source = [34u8, 92u8, 117u8, 48u8, 48u8, 54u8, 49u8, 34u8];
    let input = reader_from_bytes(bytes_copy(array_as_slice(source)));
    let plain = decoded_reader(input, writer_from_bytes(bytes_zeroed(1usize)));
    let rendered = quoted_into(plain, writer_from_bytes(bytes_zeroed(3usize)));
    let written = writer_finish(rendered);
    let view = bytes_as_slice(written);
    let exact = byte_len(view) == 3usize && match byte_get(view, 0usize) { Option::Some { value } => value == 34u8, Option::None {} => false, } && match byte_get(view, 1usize) { Option::Some { value } => value == 97u8, Option::None {} => false, } && match byte_get(view, 2usize) { Option::Some { value } => value == 34u8, Option::None {} => false, };
    if exact { 0 } else { 1 }
}
"#;

const TESTS: &str = r#"module consumer.tests;
use function @id("consumer.main") from consumer.app as run_main;

@id("consumer.tests.main")
fn main() -> i64
{
    run_main()
}
"#;

#[test]
fn private_json_cursor_roundtrip_executes_across_project_backends() {
    let scratch = super::temporary("json-roundtrip");
    std::fs::create_dir_all(scratch.join("src")).unwrap();
    std::fs::write(scratch.join("semaprax.toml"), MANIFEST).unwrap();
    std::fs::write(scratch.join("src/app.spx"), APP).unwrap();
    std::fs::write(scratch.join("src/tests.spx"), TESTS).unwrap();

    project::with_authenticated_project(&scratch.join("semaprax.toml"), |snapshot| {
        snapshot.check()?;
        let options = project::ProjectExecutionOptions::default();
        assert_eq!(
            snapshot.execute_entry(&options)?.outcome(),
            &project::ProjectExecutionOutcome::Returned(0)
        );
        for _ in 0..2 {
            assert_eq!(
                snapshot.execute_test(&options)?.outcome(),
                &project::ProjectExecutionOutcome::Returned(0)
            );
        }
        let c = codegen::emit_hir_c(snapshot.test_program()).map_err(|error| vec![error])?;
        for optimization in ["-O0", "-O2"] {
            super::compile_and_run_c(&c, &scratch, optimization, "0");
        }
        let core =
            wasm::emit_resolved_module(snapshot.entry_program()).map_err(|error| vec![error])?;
        let wasm_path = scratch.join("json-roundtrip.wasm");
        std::fs::write(&wasm_path, core).unwrap();
        let script = scratch.join("json-roundtrip.mjs");
        std::fs::write(
            &script,
            wasm_runner(wasm_path.file_name().unwrap().to_str().unwrap()),
        )
        .unwrap();
        let output = Command::new("node")
            .arg(script.file_name().unwrap())
            .current_dir(&scratch)
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        Ok(())
    })
    .unwrap();
    let _ = std::fs::remove_dir_all(scratch);
}

fn wasm_runner(wasm: &str) -> String {
    format!(
        r#"import assert from "node:assert/strict";
import {{ readFile }} from "node:fs/promises";
const bytes = await readFile("./{wasm}");
const entries = new Map(); let next = 1; let linked;
const decode = carrier => {{ const word = BigInt.asUintN(64, carrier), length = Number(word & 0xffffffffn), root = Number((word >> 32n) & 0xffffffffn); return {{ word, length, root, tagged: (root & 0x80000000) !== 0, token: root & 0x7fffffff }}; }};
const read = decoded => {{ if (decoded.tagged) {{ const value = entries.get(decoded.token); if (!(value instanceof Uint8Array) || value.length !== decoded.length) throw new Error("stale byte token"); return value; }} const memory = new Uint8Array((linked.instance.exports.__spx_byte_memory ?? linked.instance.exports.memory).buffer); if (decoded.root > memory.length - decoded.length) throw new Error("byte range"); return memory.slice(decoded.root, decoded.root + decoded.length); }};
const allocate = value => {{ if (entries.size >= 2) throw new Error("owned Bytes live entry limit exceeded"); const token = next++, bytes = new Uint8Array(value); entries.set(token, bytes); return BigInt.asIntN(64, ((0x80000000n | BigInt(token)) << 32n) | BigInt(bytes.length)); }};
const imports = {{ env: {{ spx_add: (a, b) => a + b, spx_sub: (a, b) => a - b, spx_mul: (a, b) => a * b, spx_div: (a, b) => a / b, spx_rem: (a, b) => a % b, spx_neg: a => -a, spx_contract_fail: () => {{ throw new Error("contract"); }}, spx_bytes_copy: c => allocate(read(decode(c))), spx_bytes_get: (c, i) => {{ const value = read(decode(c)), index = BigInt.asUintN(64, i); return index >= BigInt(value.length) ? -1 : value[Number(index)]; }}, spx_bytes_drop: c => {{ const value = decode(c); read(value); entries.delete(value.token); }}, spx_bytes_as_slice: c => {{ const value = decode(c); read(value); return BigInt.asIntN(64, value.word); }}, spx_bytes_zeroed: count => {{ if (typeof count !== "bigint" || count < 0n || count > 65536n) throw new Error("capacity"); return allocate(new Uint8Array(Number(count))); }}, spx_bytes_set: (c, i, v) => {{ const value = decode(c), bytes = read(value); if (typeof i !== "bigint" || i < 0n || i >= BigInt(bytes.length) || !Number.isInteger(v) || v < 0 || v > 255) throw new Error("byte write"); bytes[Number(i)] = v; return BigInt.asIntN(64, value.word); }} }} }};
linked = await WebAssembly.instantiate(bytes, imports);
for (let run = 0; run < 2; ++run) {{ assert.equal(linked.instance.exports.semaprax_main(), 0n); assert.equal(entries.size, 0); }}
"#
    )
}
