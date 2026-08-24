use std::path::Path;
use std::process::Command;

use semaprax::{parse, verify, wasm};
use wasmparser::{ExternalKind, Parser, Payload, TypeRef, Validator};

const SOURCE: &str = r#"module text.web;

@id("text.contains")
fn contains(value: borrow str, needle: borrow str) -> bool {
    str_contains(value, needle)
}

@id("text.starts")
fn starts(value: borrow str, prefix: borrow str) -> bool {
    str_starts_with(value, prefix)
}

@id("text.len")
fn byte_len(value: borrow str) -> i64 {
    str_len_bytes(value)
}

@id("text.empty")
fn empty(value: borrow str) -> bool {
    str_is_empty(value)
}

@id("text.probe")
fn probe(value: borrow str) -> i64 {
    str_len_bytes(value) / 0
}

@id("text.same")
fn same(value: borrow str) -> bool {
    str_starts_with(value, value) && str_contains(value, value)
}

@id("main")
fn main() -> i64 { 0 }
"#;

fn program() -> semaprax::ast::Program {
    let program = parse(SOURCE, Path::new("wasm-text-v1.spx")).unwrap();
    let diagnostics = verify::verify(&program);
    assert!(diagnostics.is_empty(), "{diagnostics:#?}");
    program
}

fn selected() -> Vec<String> {
    [
        "text.contains",
        "text.empty",
        "text.len",
        "text.probe",
        "text.same",
        "text.starts",
    ]
    .map(str::to_owned)
    .to_vec()
}

fn raw_symbol(id: &str) -> String {
    let mut symbol = String::from("spx_text_");
    for byte in id.bytes() {
        symbol.push_str(&format!("{byte:02x}"));
    }
    symbol
}

#[test]
fn borrowed_text_profile_has_closed_raw_abi_and_fixed_memory() {
    let bytes = wasm::emit_module_with_text_exports(&program(), &selected()).unwrap();
    Validator::new().validate_all(&bytes).unwrap();

    let mut imports = Vec::new();
    let mut exports = Vec::new();
    let mut memory = None;
    for payload in Parser::new(0).parse_all(&bytes) {
        match payload.unwrap() {
            Payload::ImportSection(section) => {
                for import in section.into_imports() {
                    let import = import.unwrap();
                    let TypeRef::Func(_) = import.ty else {
                        panic!("text profile imported non-function authority")
                    };
                    imports.push((import.module.to_owned(), import.name.to_owned()));
                }
            }
            Payload::MemorySection(section) => {
                let memories = section.into_iter().map(Result::unwrap).collect::<Vec<_>>();
                assert_eq!(memories.len(), 1);
                memory = Some(memories[0]);
            }
            Payload::ExportSection(section) => {
                exports.extend(section.into_iter().map(|item| {
                    let item = item.unwrap();
                    (item.name.to_owned(), item.kind)
                }));
            }
            _ => {}
        }
    }
    assert!(imports.iter().all(|(_, name)| !name.contains("string")));
    let memory = memory.unwrap();
    assert_eq!(memory.initial, 3);
    assert_eq!(memory.maximum, Some(3));
    assert!(!memory.memory64);
    for global in [
        "__spx_text_status_v1",
        "__spx_text_scratch_base_v1",
        "__spx_text_scratch_capacity_v1",
    ] {
        assert!(exports.contains(&(global.to_owned(), ExternalKind::Global)));
    }
    assert!(exports.contains(&("memory".to_owned(), ExternalKind::Memory)));
    for id in selected() {
        assert!(exports.contains(&(raw_symbol(&id), ExternalKind::Func)));
    }
    assert!(!exports.iter().any(|(name, _)| name == "semaprax_main"));
}

#[test]
fn raw_boundary_rejects_oob_and_noncanonical_utf8_before_target_entry() {
    if Command::new("node").arg("--version").output().is_err() {
        return;
    }
    let bytes = wasm::emit_module_with_text_exports(&program(), &selected()).unwrap();
    let encoded = base64(&bytes);
    let script = format!(
        r#"import assert from "node:assert/strict";
const bytes = Buffer.from("{encoded}", "base64");
let targetEntries = 0;
const checked = (f) => (a, b) => f(a, b);
const imports = {{ env: {{
  spx_add: checked((a,b) => a+b), spx_sub: checked((a,b) => a-b),
  spx_mul: checked((a,b) => a*b), spx_div: (_a,_b) => {{ targetEntries++; return 0n; }},
  spx_rem: checked((a,b) => a%b), spx_neg: (a) => -a,
  spx_contract_fail: () => {{ throw new Error("contract"); }},
}} }};
const {{ instance }} = await WebAssembly.instantiate(bytes, imports);
const e = instance.exports;
assert.equal(e.__spx_text_scratch_base_v1.value, 0);
assert.equal(e.__spx_text_scratch_capacity_v1.value, 65536);
assert.throws(() => e.memory.grow(1), RangeError);
const u8 = new Uint8Array(e.memory.buffer);
const contains = e[{contains:?}], starts = e[{starts:?}], len = e[{len:?}], empty = e[{empty:?}], probe = e[{probe:?}], same = e[{same:?}];
const value = Uint8Array.from([0x61,0x62,0x00,0xe2,0x82,0xac,0x63,0x64]);
u8.set(value, 16); u8.set([0x00,0xe2,0x82,0xac], 64); u8.set([0x61,0x62], 96);
assert.equal(len(19,3), 3n, `euro status=${{e.__spx_text_status_v1.value}}`);
assert.equal(len(64,4), 4n, `nul-euro status=${{e.__spx_text_status_v1.value}}`);
assert.equal(contains(16,8,64,4), 1, `contains status=${{e.__spx_text_status_v1.value}}`); assert.equal(e.__spx_text_status_v1.value, 0);
assert.equal(starts(16,8,96,2), 1, `starts status=${{e.__spx_text_status_v1.value}}`); assert.equal(len(16,8), 8n);
assert.equal(empty(16,0), 1); assert.equal(empty(16,8), 0);

// Adversarial KMP shape: the near-periodic mismatch is linear and uses the
// reserved fixed table after the public 64-KiB scratch region.
u8.fill(0x61, 0, 65536);
u8[49151] = 0x62; u8[65535] = 0x62;
assert.equal(contains(0,49152,49152,16384), 1);
u8[49151] = 0x61;
assert.equal(contains(0,49152,49152,16384), 0);
assert.equal(e.__spx_text_status_v1.value, 0);

// Exported memory makes the reserved KMP pages caller-visible. Every call
// resets the index-zero sentinel before it can affect fallback control flow.
u8[65536] = 1; u8[65537] = 0;
u8[0] = 0x61; u8[1] = 0x62;
assert.equal(contains(0,2,0,2), 1);
assert.equal(u8[65536], 0); assert.equal(u8[65537], 0);

// One external view is charged once when the target aliases it internally.
u8.fill(0x61, 0, 65536);
assert.equal(same(0,65536), 1); assert.equal(e.__spx_text_status_v1.value, 0);

// The exact cumulative budget is checked before the selected target runs.
assert.equal(probe(0,65536), 0n); assert.equal(e.__spx_text_status_v1.value, 0);
assert.equal(contains(0,32769,32767,32768), 0); assert.equal(e.__spx_text_status_v1.value, 1);
targetEntries = 0;

// Range failure is settled before any load, including unsigned wraparound.
assert.equal(probe(65535,2), 0n); assert.equal(e.__spx_text_status_v1.value, 1); assert.equal(targetEntries, 0);
assert.equal(probe(-1,1), 0n); assert.equal(e.__spx_text_status_v1.value, 1); assert.equal(targetEntries, 0);

for (const malformed of [[0xc0,0x80],[0xe0,0x80,0x80],[0xed,0xa0,0x80],[0xf0,0x80,0x80,0x80],[0xf4,0x90,0x80,0x80],[0x80]]) {{
  u8.set(malformed, 128);
  assert.equal(probe(128, malformed.length), 0n);
  assert.equal(e.__spx_text_status_v1.value, 2);
  assert.equal(targetEntries, 0);
}}
for (const valid of [[0],[0xc2,0x80],[0xe0,0xa0,0x80],[0xed,0x9f,0xbf],[0xf0,0x90,0x80,0x80],[0xf4,0x8f,0xbf,0xbf]]) {{
  u8.set(valid, 256);
  probe(256, valid.length);
  assert.equal(e.__spx_text_status_v1.value, 0);
}}
assert.equal(targetEntries, 6);
"#,
        contains = raw_symbol("text.contains"),
        starts = raw_symbol("text.starts"),
        len = raw_symbol("text.len"),
        empty = raw_symbol("text.empty"),
        probe = raw_symbol("text.probe"),
        same = raw_symbol("text.same"),
    );
    let output = Command::new("node")
        .args(["--input-type=module", "--eval", &script])
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "node text ABI proof failed:\n{}",
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn profile_rejects_unreachable_owned_string_and_loop_inventory() {
    let owned = parse(
        r#"module text.hostile;
@id("text.len") fn byte_len(value: borrow str) -> i64 { str_len_bytes(value) }
@id("text.unused") fn unused(value: string) -> i64 { string_len(value) }
@id("main") fn main() -> i64 { 0 }
"#,
        Path::new("text-owned-hostile.spx"),
    )
    .unwrap();
    let error = wasm::emit_module_with_text_exports(&owned, &["text.len".to_owned()]).unwrap_err();
    assert_eq!(error.code, "SPX-W119");
    assert!(error.message.contains("unsupported parameter"));

    for recursive in [
        r#"module text.recursive;
@id("text.f") fn f(value: borrow str) -> i64 { f(value) }
@id("main") fn main() -> i64 { 0 }
"#,
        r#"module text.recursive;
@id("text.f") fn f(value: borrow str) -> i64 { g(value) }
@id("text.g") fn g(value: borrow str) -> i64 { f(value) }
@id("main") fn main() -> i64 { 0 }
"#,
    ] {
        let program = parse(recursive, Path::new("text-recursive-hostile.spx")).unwrap();
        let error =
            wasm::emit_module_with_text_exports(&program, &["text.f".to_owned()]).unwrap_err();
        assert_eq!(error.code, "SPX-W119");
        assert!(error.message.contains("recursive call cycle"));
    }

    let looped = parse(
        r#"module text.looped;
@id("text.len") fn byte_len(value: borrow str) -> i64 { str_len_bytes(value) }
@id("text.unused") fn unused(value: i64) -> i64 {
    let mut n = value;
    while n > 0 { n = n - 1; 0 }
    n
}
@id("main") fn main() -> i64 { 0 }
"#,
        Path::new("text-loop-hostile.spx"),
    )
    .unwrap();
    let error = wasm::emit_module_with_text_exports(&looped, &["text.len".to_owned()]).unwrap_err();
    assert_eq!(error.code, "SPX-W119");
    assert!(error.message.contains("reaches a loop"));
}

fn base64(bytes: &[u8]) -> String {
    const TABLE: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::with_capacity(bytes.len().div_ceil(3) * 4);
    for chunk in bytes.chunks(3) {
        let word = (u32::from(chunk[0]) << 16)
            | (u32::from(*chunk.get(1).unwrap_or(&0)) << 8)
            | u32::from(*chunk.get(2).unwrap_or(&0));
        out.push(TABLE[((word >> 18) & 63) as usize] as char);
        out.push(TABLE[((word >> 12) & 63) as usize] as char);
        out.push(if chunk.len() > 1 {
            TABLE[((word >> 6) & 63) as usize] as char
        } else {
            '='
        });
        out.push(if chunk.len() > 2 {
            TABLE[(word & 63) as usize] as char
        } else {
            '='
        });
    }
    out
}
