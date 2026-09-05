use std::path::Path;
use std::process::Command;
use std::sync::atomic::{AtomicU64, Ordering};

use super::{
    borrow_place_shape_is_admitted, emit_owned_data_exports, emit_profile, function_import,
    hex_identity, intern_type, lower_selected_function_instances, section, write_bytes, write_i64,
    write_name, write_u32, Signature, I32, RANGE_DESCRIPTOR_ADDRESS_LIMIT,
    RANGE_DESCRIPTOR_POINTER_MASK, SHADOW_STACK_TOP,
};
use crate::codegen::native_aggregate::{
    resource_harness_scenario, wasm_address, HarnessAction, ResourceHarnessScenario,
};
use crate::hir::{self, DeclarationId, FunctionInstanceId, ResolvedType};
use crate::parse;

static NEXT_ID: AtomicU64 = AtomicU64::new(0);

#[test]
fn projected_borrow_shape_gate_rejects_wrong_operation_and_depth() {
    let field = hir::PlaceProjection::Field(DeclarationId::new("packet.payload"));
    let exact = hir::Place {
        root: hir::ValueId::intrinsic_parameter("packet", 0),
        projections: vec![field.clone()],
    };
    assert!(borrow_place_shape_is_admitted(
        &DeclarationId::new(crate::byte_ops::BYTES_AS_SLICE_ID),
        &exact,
    ));
    assert!(!borrow_place_shape_is_admitted(
        &DeclarationId::new(crate::byte_ops::STR_AS_BYTES_ID),
        &exact,
    ));
    let deeper = hir::Place {
        root: exact.root,
        projections: vec![field.clone(), field],
    };
    assert!(!borrow_place_shape_is_admitted(
        &DeclarationId::new(crate::byte_ops::BYTES_AS_SLICE_ID),
        &deeper,
    ));
}

#[test]
fn projected_borrowed_bytes_forwards_one_token_and_drops_only_the_owner() {
    if Command::new("node").arg("--version").output().is_err() {
        return;
    }
    let source = r#"
module test.wasm_projected_borrowed_bytes;
@id("packet") record Packet {
    @id("packet.payload") payload: Bytes,
    @id("packet.marker") marker: i64,
}
@id("bytes.inspect")
fn inspect(value: borrow Bytes) -> usize {
    byte_len(bytes_as_slice(value))
}
@id("bytes.projected")
fn projected(packet: own Packet) -> usize {
    let first = inspect(packet.payload);
    let second = inspect(packet.payload);
    first + second
}
@id("app.main") fn main() -> i64 { 0 }
"#;
    let resolved =
        hir::resolve(&parse(source, Path::new("wasm-projected-borrowed-bytes.spx")).unwrap())
            .unwrap();
    let bytes = emit_profile(&resolved, true, false).unwrap();
    let id = NEXT_ID.fetch_add(1, Ordering::Relaxed);
    let stem = format!(
        "semaprax-projected-borrowed-bytes-wasm-{}-{id}",
        std::process::id()
    );
    let wasm_path = std::env::temp_dir().join(format!("{stem}.wasm"));
    let script_path = std::env::temp_dir().join(format!("{stem}.mjs"));
    std::fs::write(&wasm_path, bytes).unwrap();
    let projected = format!(
        "__spx_test_{}",
        hex_identity(&DeclarationId::new("bytes.projected"))
    );
    let script = format!(
        r#"import {{ readFile }} from "node:fs/promises";
const bytes = await readFile(process.argv[2]);
let copies = 0, drops = 0, views = 0;
const fail = name => () => {{ throw new Error(`unexpected host import ${{name}}`); }};
const {{instance}} = await WebAssembly.instantiate(bytes, {{env: {{
  spx_add: fail("spx_add"), spx_sub: fail("spx_sub"), spx_mul: fail("spx_mul"),
  spx_div: fail("spx_div"), spx_rem: fail("spx_rem"), spx_neg: fail("spx_neg"),
  spx_contract_fail: fail("spx_contract_fail"),
  spx_bytes_copy: value => {{ copies += 1; return value; }},
  spx_bytes_drop: () => {{ drops += 1; }},
  spx_bytes_as_slice: value => {{ views += 1; return value; }},
  spx_bytes_get: fail("spx_bytes_get"),
}}}});
const view = new DataView(instance.exports.__spx_test_memory.buffer);
const input = 1024, output = 2048, token = 5n;
for (let iteration = 1; iteration <= 3; iteration++) {{
  view.setBigUint64(input, token, true);
  view.setBigInt64(input + 8, 7n, true);
  if (instance.exports["{projected}"](input, output) !== 0) throw new Error("status");
  if (view.getBigUint64(output, true) !== 10n) throw new Error("borrowed token changed");
  if (copies !== 0) throw new Error("borrow minted an owner");
  if (drops !== iteration) throw new Error("borrow scheduled an extra drop");
}}
if (views !== 12) throw new Error("borrowed carrier was not forwarded unchanged");
"#
    );
    std::fs::write(&script_path, script).unwrap();
    let output = Command::new("node")
        .arg(&script_path)
        .arg(&wasm_path)
        .output()
        .unwrap();
    let _ = std::fs::remove_file(script_path);
    let _ = std::fs::remove_file(wasm_path);
    assert!(
        output.status.success(),
        "stdout={} stderr={}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn owned_data_scalar_wrappers_publish_exact_width_and_preserve_foreign_sentinels() {
    let node = Command::new("node").arg("--version").output().unwrap();
    assert!(
        node.status.success(),
        "Node is required for owned-data evidence"
    );
    let source = r#"
module test.owned_data_scalars;
@id("scalar.signed") fn signed() -> i64 { -7 }
@id("scalar.flag") fn flag() -> bool { true }
@id("scalar.maximum") fn maximum() -> usize { 18446744073709551615usize }
@id("scalar.main") fn main() -> i64 { 0 }
"#;
    let resolved =
        hir::resolve(&parse(source, Path::new("owned-data-scalars.spx")).unwrap()).unwrap();
    let plans = [
        super::super::owned_data_exports::OwnedDataExportPlan {
            stable_id: "scalar.flag".to_owned(),
            wasm_export: "flag".to_owned(),
            function_id: DeclarationId::new("scalar.flag"),
            parameters: Vec::new(),
            result: super::super::owned_data_exports::ResultLayout::Bool,
        },
        super::super::owned_data_exports::OwnedDataExportPlan {
            stable_id: "scalar.maximum".to_owned(),
            wasm_export: "maximum".to_owned(),
            function_id: DeclarationId::new("scalar.maximum"),
            parameters: Vec::new(),
            result: super::super::owned_data_exports::ResultLayout::Usize,
        },
        super::super::owned_data_exports::OwnedDataExportPlan {
            stable_id: "scalar.signed".to_owned(),
            wasm_export: "signed".to_owned(),
            function_id: DeclarationId::new("scalar.signed"),
            parameters: Vec::new(),
            result: super::super::owned_data_exports::ResultLayout::I64,
        },
    ];
    let bytes = emit_owned_data_exports(&resolved, &plans).unwrap();
    let id = NEXT_ID.fetch_add(1, Ordering::Relaxed);
    let stem = format!("semaprax-owned-data-scalars-{}-{id}", std::process::id());
    let wasm_path = std::env::temp_dir().join(format!("{stem}.wasm"));
    let script_path = std::env::temp_dir().join(format!("{stem}.mjs"));
    std::fs::write(&wasm_path, bytes).unwrap();
    let script = r#"import { readFile } from "node:fs/promises";
const bytes = await readFile(process.argv[2]);
const env = Object.freeze({
  spx_add: (a,b) => a+b, spx_sub: (a,b) => a-b, spx_mul: (a,b) => a*b,
  spx_div: (a,b) => a/b, spx_rem: (a,b) => a%b, spx_neg: a => -a,
  spx_contract_fail: () => { throw new Error("contract"); },
  spx_bytes_copy: () => { throw new Error("bytes_copy"); },
  spx_bytes_get: () => { throw new Error("bytes_get"); },
  spx_bytes_drop: () => { throw new Error("bytes_drop"); },
  spx_bytes_as_slice: () => { throw new Error("bytes_as_slice"); },
  spx_owned_utf8_validate_v1: () => { throw new Error("utf8_validate"); },
});
const { instance } = await WebAssembly.instantiate(bytes, { env });
const output = 65536, memory = new Uint8Array(instance.exports.memory.buffer);
const view = new DataView(memory.buffer), sentinel = 0x5a;
memory.fill(sentinel, output, output + 16);
if (instance.exports.flag(output) !== 0 || view.getUint32(output, true) !== 1) throw new Error("bool result");
for (let index = output + 4; index < output + 16; index++) if (memory[index] !== sentinel) throw new Error("bool overwrote foreign bytes");
memory.fill(sentinel, output, output + 16);
if (instance.exports.signed(output) !== 0 || view.getBigInt64(output, true) !== -7n) throw new Error("i64 result");
for (let index = output + 8; index < output + 16; index++) if (memory[index] !== sentinel) throw new Error("i64 overwrote foreign bytes");
memory.fill(sentinel, output, output + 16);
if (instance.exports.maximum(output) !== 0 || view.getBigUint64(output, true) !== 18446744073709551615n) throw new Error("usize result");
for (let index = output + 8; index < output + 16; index++) if (memory[index] !== sentinel) throw new Error("usize overwrote foreign bytes");
"#;
    std::fs::write(&script_path, script).unwrap();
    let output = Command::new("node")
        .arg(&script_path)
        .arg(&wasm_path)
        .output()
        .unwrap();
    let _ = std::fs::remove_file(script_path);
    let _ = std::fs::remove_file(wasm_path);
    assert!(
        output.status.success(),
        "stdout={} stderr={}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn range_descriptor_carrier_covers_the_entire_private_shadow_stack() {
    assert_eq!(
        RANGE_DESCRIPTOR_ADDRESS_LIMIT,
        (RANGE_DESCRIPTOR_POINTER_MASK + 1) * 8
    );
    let stack_top = std::hint::black_box(SHADOW_STACK_TOP);
    assert!(stack_top <= RANGE_DESCRIPTOR_ADDRESS_LIMIT);
}

const BYTE_BOUNDARY_SOURCE: &str = r#"
module test.byte_boundary;
@id("bytes.total")
fn total(left: borrow Slice<u8>, right: borrow Slice<u8>) -> usize {
    byte_len(left) + byte_len(right)
}
@id("bytes.forward")
fn forward(left: borrow Slice<u8>, right: borrow Slice<u8>) -> usize {
    total(left, right)
}
@id("bytes.mixed")
fn mixed(text: borrow str, bytes: borrow Slice<u8>) -> usize {
    byte_len(str_as_bytes(text)) + byte_len(bytes)
}
@id("bytes.nul")
fn nul(text: borrow str) -> bool {
    match byte_get(str_as_bytes(text), 1usize) {
        Option::Some { value: byte } => byte == 0u8,
        Option::None {} => false,
    }
}
@id("bytes.at")
fn at(value: borrow Slice<u8>, index: usize) -> u8 {
    match byte_get(value, index) {
        Option::Some { value: byte } => byte,
        Option::None {} => 0u8,
    }
}
@id("bytes.range-at")
fn range_at(value: borrow Slice<u8>, start: usize, end: usize, index: usize) -> u8 {
    let selected = byte_range(value, start, end);
    match byte_get(selected, index) {
        Option::Some { value: byte } => byte,
        Option::None {} => 0u8,
    }
}
@id("usize.add.failure")
fn usize_add(left: usize, right: usize) -> usize { left + right }
@id("usize.sub.failure")
fn usize_sub(left: usize, right: usize) -> usize { left - right }
@id("usize.mul.failure")
fn usize_mul(left: usize, right: usize) -> usize { left * right }
@id("usize.mul.nested")
fn usize_mul_nested(left: usize, right: usize) -> usize {
    if right == 0usize { left * right } else { left * right }
}
@id("usize.div.failure")
fn usize_div(left: usize, right: usize) -> usize { left / right }
@id("usize.rem.failure")
fn usize_rem(left: usize, right: usize) -> usize { left % right }
@id("app.main")
fn main() -> i64 { 0 }
"#;

#[test]
fn node_rejects_invalid_and_cumulatively_oversized_external_byte_roots() {
    if Command::new("node").arg("--version").output().is_err() {
        return;
    }
    let program = parse(BYTE_BOUNDARY_SOURCE, Path::new("byte-boundary-wasm.spx")).unwrap();
    let resolved = hir::resolve(&program).unwrap();
    let bytes = emit_profile(&resolved, true, false).unwrap();
    let id = NEXT_ID.fetch_add(1, Ordering::Relaxed);
    let stem = format!("semaprax-byte-boundary-wasm-{}-{id}", std::process::id());
    let wasm_path = std::env::temp_dir().join(format!("{stem}.wasm"));
    let script_path = std::env::temp_dir().join(format!("{stem}.mjs"));
    std::fs::write(&wasm_path, bytes).unwrap();
    let export = |id: &str| format!("__spx_test_{}", hex_identity(&DeclarationId::new(id)));
    let script = format!(
        r#"import {{ readFile }} from "node:fs/promises";
const fail = (name) => () => {{ throw new Error(`unexpected host import ${{name}}`); }};
const bytes = await readFile(process.argv[2]);
let wasmInstance;
const result = await WebAssembly.instantiate(bytes, {{ env: {{
  spx_add: (a, b) => a + b, spx_sub: fail("spx_sub"), spx_mul: fail("spx_mul"),
  spx_div: fail("spx_div"), spx_rem: fail("spx_rem"), spx_neg: fail("spx_neg"),
  spx_contract_fail: fail("spx_contract_fail"),
  spx_bytes_copy: fail("spx_bytes_copy"), spx_bytes_drop: fail("spx_bytes_drop"),
  spx_bytes_as_slice: value => value,
  spx_bytes_get: (carrier, index) => {{
    const at=BigInt.asUintN(64,index),memory=wasmInstance.exports.__spx_test_memory.buffer;
    const get=(value,n)=>{{const word=BigInt.asUintN(64,value),length=Number(word&0xffffffffn),root=Number((word>>32n)&0xffffffffn);if(n>=BigInt(length))return -1;if(((root&0xc0000000)>>>0)===0x40000000){{const p=(root&0xffff)*8,view=new DataView(memory),ultimate=view.getBigInt64(p+8,true),offset=view.getBigUint64(p+16,true);return get(ultimate,offset+n)}}return new Uint8Array(memory)[root+Number(n)]}};
    return get(carrier,at);
  }},
}} }});
wasmInstance=result.instance;
const {{ instance }} = result;
const view = new DataView(instance.exports.__spx_test_memory.buffer);
const output = 65536;
const pack = (offset, length) => (BigInt(offset) << 32n) | BigInt(length);
const forward = instance.exports["{forward}"];
const mixed = instance.exports["{mixed}"];
const nul = instance.exports["{nul}"];
const at = instance.exports["{at}"];
const rangeAt = instance.exports["{range_at}"];
const usizeAdd = instance.exports["{usize_add}"];
const usizeSub = instance.exports["{usize_sub}"];
const usizeMul = instance.exports["{usize_mul}"];
const usizeMulNested = instance.exports["{usize_mul_nested}"];
const usizeDiv = instance.exports["{usize_div}"];
const usizeRem = instance.exports["{usize_rem}"];
if (forward(pack(0, 32768), pack(32768, 32768), output) !== 0) throw new Error("valid boundary status");
if (view.getBigUint64(output, true) !== 65536n) throw new Error("internal forwarding recharged roots");
view.setUint8(10, 0); view.setUint8(11, 255); view.setUint8(12, 7);
if (at(pack(10, 3), 1n, output) !== 0 || view.getUint8(output) !== 255) throw new Error("total indexed hit");
if (at(pack(10, 3), 3n, output) !== 0 || view.getUint8(output) !== 0) throw new Error("total indexed miss");
if (at(pack(10, 3), 0xffffffffffffffffn, output) !== 0 || view.getUint8(output) !== 0) throw new Error("total indexed max miss");
if (rangeAt(pack(10, 3), 1n, 3n, 0n, output) !== 0 || view.getUint8(output) !== 255) throw new Error("general aggregate byte range");
view.setUint8(20, 65); view.setUint8(21, 0); view.setUint8(22, 66);
if (nul(pack(20, 3), output) !== 0 || view.getUint8(output) !== 1) throw new Error("embedded NUL str view");
if (mixed(pack(20, 32768), pack(32768, 32768), output) !== 0 || view.getBigUint64(output,true)!==65536n) throw new Error("mixed root budget");
if (usizeAdd(-1n, 1n, output) !== 1) throw new Error("usize add overflow status");
if (usizeSub(0n, 1n, output) !== 2) throw new Error("usize sub overflow status");
if (usizeMul(-1n, 2n, output) !== 3) throw new Error("usize mul overflow status");
const maximum = 18446744073709551615n;
const productCases = [
  [0n, 0n, 0n], [maximum, 0n, 0n], [0n, maximum, 0n],
  [maximum, 1n, maximum], [1n, maximum, maximum],
  [maximum / 2n, 2n, maximum - 1n], [maximum / 3n, 3n, maximum],
];
const resultBytes = new Uint8Array(view.buffer, output, 16);
for (const multiply of [usizeMul, usizeMulNested]) {{
  for (const [left, right, expected] of productCases) {{
    resultBytes.fill(0xa5);
    if (multiply(left, right, output) !== 0) throw new Error("usize product success status");
    if (view.getBigUint64(output, true) !== expected) throw new Error("usize product value");
    if (!resultBytes.subarray(8).every(byte => byte === 0xa5)) throw new Error("usize product output extent");
  }}
  for (const [left, right] of [[maximum, 2n], [maximum / 2n + 1n, 2n], [2n, maximum]]) {{
    resultBytes.fill(0xa5);
    if (multiply(left, right, output) !== 3) throw new Error("usize product checked overflow");
    if (!resultBytes.every(byte => byte === 0xa5)) throw new Error("usize overflow modified output");
    if (multiply(maximum, 0n, output) !== 0 || view.getBigUint64(output, true) !== 0n) throw new Error("usize zero recovery");
  }}
}}
if (usizeDiv(1n, 0n, output) !== 4) throw new Error("usize division by zero status");
if (usizeRem(1n, 0n, output) !== 6) throw new Error("usize remainder by zero status");
let invalidRange = false;
try {{ forward(pack(65000, 1000), pack(0, 0), output); }} catch {{ invalidRange = true; }}
if (!invalidRange) throw new Error("invalid packed range was admitted");
let cumulative = false;
try {{ forward(pack(0, 40000), pack(0, 40000), output); }} catch {{ cumulative = true; }}
if (!cumulative) throw new Error("cumulative external roots were admitted");
let mixedCumulative = false;
try {{ mixed(pack(0, 40000), pack(0, 40000), output); }} catch {{ mixedCumulative = true; }}
if (!mixedCumulative) throw new Error("mixed external roots were admitted");
"#,
        forward = export("bytes.forward"),
        mixed = export("bytes.mixed"),
        nul = export("bytes.nul"),
        at = export("bytes.at"),
        range_at = export("bytes.range-at"),
        usize_add = export("usize.add.failure"),
        usize_sub = export("usize.sub.failure"),
        usize_mul = export("usize.mul.failure"),
        usize_mul_nested = export("usize.mul.nested"),
        usize_div = export("usize.div.failure"),
        usize_rem = export("usize.rem.failure")
    );
    std::fs::write(&script_path, script).unwrap();
    let output = Command::new("node")
        .arg(&script_path)
        .arg(&wasm_path)
        .output()
        .unwrap();
    let _ = std::fs::remove_file(&script_path);
    let _ = std::fs::remove_file(&wasm_path);
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
}

const GENERIC_INSTANCE_SOURCE: &str =
    include_str!("../../../platform-tests/component-runtime/v9.spx");

#[test]
fn selected_generic_lowering_authenticates_exact_instance_sequence_and_identity() {
    let program = parse(
        GENERIC_INSTANCE_SOURCE,
        Path::new("selected-generic-instances.spx"),
    )
    .unwrap();
    let resolved = hir::resolve(&program).unwrap();
    let ordered = resolved
        .function_instances
        .iter()
        .map(|instance| instance.id.clone())
        .collect::<Vec<_>>();
    assert_eq!(ordered.len(), 6);

    let first = lower_selected_function_instances(&resolved, &ordered, &ordered[0]).unwrap();
    assert_eq!(first.selected_index, 0);
    for (index, selected) in ordered.iter().enumerate().skip(1) {
        let lowering = lower_selected_function_instances(&resolved, &ordered, selected).unwrap();
        assert_eq!(lowering.types, first.types);
        assert_eq!(lowering.function_type_indexes, first.function_type_indexes);
        assert_eq!(lowering.bodies, first.bodies);
        assert_eq!(lowering.selected_index, u32::try_from(index).unwrap());
    }

    assert!(lower_selected_function_instances(&resolved, &[], &ordered[0]).is_err());

    let mut missing = ordered.clone();
    missing.pop();
    assert!(lower_selected_function_instances(&resolved, &missing, &ordered[0]).is_err());

    let mut duplicate = ordered.clone();
    duplicate[5] = duplicate[0].clone();
    assert!(lower_selected_function_instances(&resolved, &duplicate, &ordered[0]).is_err());

    let mut reordered = ordered.clone();
    reordered.swap(0, 1);
    assert!(lower_selected_function_instances(&resolved, &reordered, &ordered[0]).is_err());

    let monomorphic_confusion = FunctionInstanceId::derive(
        &DeclarationId::new("generic.materialize"),
        &[ResolvedType::I64],
    );
    assert!(
        lower_selected_function_instances(&resolved, &ordered, &monomorphic_confusion,).is_err()
    );

    let mut inconsistent = resolved.clone();
    inconsistent.function_instances[0].type_arguments[0] = ResolvedType::Bool;
    assert!(lower_selected_function_instances(&inconsistent, &ordered, &ordered[0]).is_err());
}

const SOURCE: &str = r#"
module test.aggregate_wasm;
@id("inner.type")
record Inner {
    @id("inner.value") value: i64,
    @id("inner.flag") flag: bool,
}
@id("outer.type")
record Outer {
    @id("outer.inner") inner: Inner,
    @id("outer.other") other: i64,
}
@id("case.ok")
fn ok(base: Outer) -> Outer {
    base with { inner: base.inner with { value: base.inner.value + 2 } }
}
@id("case.fail.base")
fn fail_base() -> Outer requires false {
    Outer { inner: Inner { value: 1, flag: true }, other: 2 }
}
@id("case.base.first")
fn base_first() -> Outer {
    fail_base() with { other: 9223372036854775807 + 1 }
}
@id("case.replacements")
fn replacements(base: Outer) -> Outer {
    base with {
        inner: base.inner with { value: 9223372036854775807 + 1 },
        other: 1 / 0,
    }
}
@id("case.post")
fn post(base: Outer) -> Outer ensures false { ok(base) }
@id("app.main")
fn main() -> i64 {
    let value = Outer {
        inner: Inner { value: 18, flag: true },
        other: 22,
    };
    let changed = ok(value);
    if changed.inner.flag { changed.inner.value + changed.other } else { 0 }
}
"#;

const VARIANT_SOURCE: &str = r#"
module test.variant_wasm;
@id("choice.type")
variant Choice {
    @id("choice.none") None,
    @id("choice.number") Number {
        @id("choice.number.value") value: i64,
    },
    @id("choice.flag") Flag {
        @id("choice.flag.enabled") enabled: bool,
    },
    @id("choice.pair") Pair {
        @id("choice.pair.first") first: i64,
        @id("choice.pair.second") second: i64,
    },
}
@id("choice.make")
fn make(value: i64) -> Choice { Choice::Number { value: value } }
@id("choice.select")
fn select(choice: Choice) -> i64 {
    match choice {
        Choice::None {} => 0,
        Choice::Number { value: number } => number,
        Choice::Flag { enabled: flag } => if flag { 1 } else { 2 },
        Choice::Pair { first: left, second: right } => left + right,
    }
}
@id("choice.as_bool")
fn as_bool(choice: Choice) -> bool {
    match choice {
        Choice::None {} => false,
        Choice::Number { value: number } => number == 42,
        Choice::Flag { enabled: flag } => flag,
        Choice::Pair { first: left, second: right } => left == right,
    }
}
@id("choice.selected")
fn selected_only() -> i64 {
    match Choice::Number { value: 42 } {
        Choice::None {} => 9223372036854775807 + 1,
        Choice::Number { value: number } => number,
        Choice::Flag { enabled: flag } => 1 / 0,
        Choice::Pair { first: left, second: right } => left + right,
    }
}
@id("choice.selected_failure")
fn selected_failure() -> i64 {
    match Choice::Flag { enabled: true } {
        Choice::None {} => 1 / 0,
        Choice::Number { value: number } => number,
        Choice::Flag { enabled: flag } => 9223372036854775807 + 1,
        Choice::Pair { first: left, second: right } => left + right,
    }
}
@id("choice.construct_order")
fn construct_order() -> i64 {
    match Choice::Pair { second: 1 / 0, first: 9223372036854775807 + 1 } {
        Choice::None {} => 0,
        Choice::Number { value: number } => number,
        Choice::Flag { enabled: flag } => if flag { 1 } else { 2 },
        Choice::Pair { first: left, second: right } => left + right,
    }
}
@id("choice.scrutinee")
fn failing_scrutinee() -> Choice requires false { Choice::None {} }
@id("choice.aggregate_failure")
fn aggregate_failure() -> Choice {
    Choice::Number { value: 9223372036854775807 + 1 }
}
@id("choice.scrutinee_first")
fn scrutinee_first() -> i64 {
    match failing_scrutinee() {
        Choice::None {} => 9223372036854775807 + 1,
        Choice::Number { value: number } => number,
        Choice::Flag { enabled: flag } => if flag { 1 } else { 2 },
        Choice::Pair { first: left, second: right } => left + right,
    }
}
@id("choice.post")
fn post(choice: Choice) -> i64 ensures false { select(choice) }
@id("app.main")
fn main() -> i64 { select(make(42)) }
"#;

const GENERIC_VARIANT_SOURCE: &str = r#"
module test.generic_variant_wasm;
@id("choice.generic")
variant Choice<T> {
    @id("choice.generic.none") None,
    @id("choice.generic.value") Value {
        @id("choice.generic.value.value") value: T,
    },
}
@id("choice.i64")
fn choice_i64() -> Choice<i64> { Choice<i64>::Value { value: 40 } }
@id("choice.bool")
fn choice_bool() -> Choice<bool> { Choice<bool>::Value { value: true } }
@id("choice.read_i64")
fn read_choice_i64(value: Choice<i64>) -> i64 {
    match value {
        Choice::None {} => 0,
        Choice::Value { value: inner } => inner,
    }
}
@id("choice.read_bool")
fn read_choice_bool(value: Choice<bool>) -> i64 {
    match value {
        Choice::None {} => 0,
        Choice::Value { value: inner } => if inner { 1 } else { 0 },
    }
}
@id("option.some")
fn option_some() -> Option<i64> { Option<i64>::Some { value: 1 } }
@id("option.read")
fn read_option(value: Option<i64>) -> i64 {
    match value {
        Option::None {} => 0,
        Option::Some { value: inner } => inner,
    }
}
@id("result.err")
fn result_err() -> Result<i64, bool> { Result<i64, bool>::Err { error: true } }
@id("result.read")
fn read_result(value: Result<i64, bool>) -> i64 {
    match value {
        Result::Ok { value: success } => success,
        Result::Err { error } => if error { 1 } else { 0 },
    }
}
@id("result.failure")
fn result_failure() -> Result<i64, bool> {
    Result<i64, bool>::Ok { value: 9223372036854775807 + 1 }
}
@id("app.main")
fn main() -> i64 {
    read_choice_i64(choice_i64()) + read_choice_bool(choice_bool()) +
        read_option(option_some()) + read_result(result_err())
}
"#;

const RESULT_TRY_SOURCE: &str = r#"
module test.result_try_wasm;
@id("try.source_i64")
fn source_i64(residual: bool, value: i64) -> Result<i64, bool> {
    if residual {
        Result<i64, bool>::Err { error: true }
    } else {
        Result<i64, bool>::Ok { value: value }
    }
}
@id("try.source_bool")
fn source_bool(residual: bool, value: bool) -> Result<bool, bool> {
    if residual {
        Result<bool, bool>::Err { error: true }
    } else {
        Result<bool, bool>::Ok { value: value }
    }
}
@id("try.large_to_small")
fn large_to_small(residual: bool, value: i64) -> Result<bool, bool>
    ensures match result {
        Result::Ok { value: success } => success,
        Result::Err { error: failure } => failure,
    }
{
    let number = source_i64(residual, value)?;
    Result<bool, bool>::Ok { value: number > 0 }
}
@id("try.small_to_large")
fn small_to_large(residual: bool, value: bool) -> Result<i64, bool>
    ensures match result {
        Result::Ok { value: success } => success == 0 || success == 1,
        Result::Err { error: failure } => failure,
    }
{
    let flag = source_bool(residual, value)?;
    Result<i64, bool>::Ok { value: if flag { 1 } else { 0 } }
}
@id("try.post_err")
fn post_err() -> Result<bool, bool> ensures false {
    let number = source_i64(true, 7)?;
    Result<bool, bool>::Ok { value: number > 0 }
}
@id("try.physical")
fn physical() -> Result<i64, bool> requires false {
    Result<i64, bool>::Err { error: true }
}
@id("try.physical_then_post")
fn physical_then_post() -> Result<bool, bool> ensures false {
    let number = physical()?;
    Result<bool, bool>::Ok { value: number > 0 }
}
@id("try.err_skips_later")
fn err_skips_later() -> Result<bool, bool> {
    let number = source_i64(true, 7)?;
    Result<bool, bool>::Ok { value: number + 9223372036854775807 > 0 }
}
@id("try.from_input")
fn from_input(value: Result<i64, bool>) -> Result<bool, bool> {
    let number = value?;
    Result<bool, bool>::Ok { value: number > 0 }
}
@id("app.main")
fn main() -> i64 {
    let large = large_to_small(false, 42);
    let small = small_to_large(true, true);
    let left = match large {
        Result::Ok { value: success } => if success { 40 } else { 0 },
        Result::Err { error: failure } => if failure { 1 } else { 0 },
    };
    let right = match small {
        Result::Ok { value: success } => success,
        Result::Err { error: failure } => if failure { 2 } else { 0 },
    };
    left + right
}
"#;

#[test]
fn node_executes_aggregate_status_out_poison_order_and_shadow_stack_reentry() {
    if Command::new("node").arg("--version").output().is_err() {
        return;
    }
    let program = parse(SOURCE, Path::new("aggregate-wasm.spx")).unwrap();
    let resolved = hir::resolve(&program).unwrap();
    let bytes = emit_profile(&resolved, true, false).unwrap();
    let id = NEXT_ID.fetch_add(1, Ordering::Relaxed);
    let stem = format!("semaprax-aggregate-wasm-{}-{id}", std::process::id());
    let wasm_path = std::env::temp_dir().join(format!("{stem}.wasm"));
    let script_path = std::env::temp_dir().join(format!("{stem}.mjs"));
    std::fs::write(&wasm_path, bytes).unwrap();

    let export = |id: &str| format!("__spx_test_{}", hex_identity(&DeclarationId::new(id)));
    let script = format!(
        r#"import {{ readFile }} from "node:fs/promises";
const fail = (name) => () => {{ throw new Error(`unexpected host import ${{name}}`); }};
const bytes = await readFile(process.argv[2]);
const {{ instance }} = await WebAssembly.instantiate(bytes, {{ env: {{
  spx_add: fail("spx_add"), spx_sub: fail("spx_sub"), spx_mul: fail("spx_mul"),
  spx_div: fail("spx_div"), spx_rem: fail("spx_rem"), spx_neg: fail("spx_neg"),
  spx_contract_fail: fail("spx_contract_fail"),
}} }});
const memory = instance.exports.__spx_test_memory;
const stack = instance.exports.__spx_test_shadow_stack;
const view = new DataView(memory.buffer);
const input = 1024;
const output = 2048;
const poison = 0xa5;
const poisonOutput = () => new Uint8Array(memory.buffer, output, 24).fill(poison);
const assertPoison = () => {{
  for (const byte of new Uint8Array(memory.buffer, output, 24)) if (byte !== poison) throw new Error("aggregate failure published output");
}};
view.setBigInt64(input, 18n, true);
view.setInt32(input + 8, 1, true);
view.setBigInt64(input + 16, 22n, true);
poisonOutput();
if (instance.exports["{ok}"](input, output) !== 0) throw new Error("success status");
if (view.getBigInt64(output, true) !== 20n || view.getInt32(output + 8, true) !== 1 || view.getBigInt64(output + 16, true) !== 22n) throw new Error("success aggregate");
if (stack.value !== {stack_top}) throw new Error("success stack restore");
for (let index = 0; index < 4096; index += 1) {{
  poisonOutput();
  if (instance.exports["{base_first}"](output) !== 9) throw new Error("base-first status");
  assertPoison();
  if (stack.value !== {stack_top}) throw new Error("base-first stack restore");
  if (instance.exports["{replacements}"](input, output) !== 1) throw new Error("replacement-order status");
  assertPoison();
  if (stack.value !== {stack_top}) throw new Error("replacement stack restore");
  if (instance.exports["{post}"](input, output) !== 10) throw new Error("postcondition status");
  assertPoison();
  if (stack.value !== {stack_top}) throw new Error("postcondition stack restore");
}}
if (instance.exports.semaprax_main() !== 42n) throw new Error("public aggregate result");
if (stack.value !== {stack_top}) throw new Error("public stack restore");
console.log("aggregate-wasm-v1-ok");
"#,
        ok = export("case.ok"),
        base_first = export("case.base.first"),
        replacements = export("case.replacements"),
        post = export("case.post"),
        stack_top = SHADOW_STACK_TOP,
    );
    std::fs::write(&script_path, script).unwrap();
    let output = Command::new("node")
        .arg(&script_path)
        .arg(&wasm_path)
        .output()
        .unwrap();
    let _ = std::fs::remove_file(&script_path);
    let _ = std::fs::remove_file(&wasm_path);
    assert!(
        output.status.success(),
        "Node aggregate runtime failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(
        String::from_utf8_lossy(&output.stdout).trim(),
        "aggregate-wasm-v1-ok"
    );
}

#[test]
fn node_executes_copy_variants_selected_arms_invalid_tags_and_reentry() {
    if Command::new("node").arg("--version").output().is_err() {
        return;
    }
    let program = parse(VARIANT_SOURCE, Path::new("variant-wasm.spx")).unwrap();
    let resolved = hir::resolve(&program).unwrap();
    let bytes = emit_profile(&resolved, true, false).unwrap();
    assert_eq!(bytes, emit_profile(&resolved, true, false).unwrap());
    let id = NEXT_ID.fetch_add(1, Ordering::Relaxed);
    let stem = format!("semaprax-variant-wasm-{}-{id}", std::process::id());
    let wasm_path = std::env::temp_dir().join(format!("{stem}.wasm"));
    let script_path = std::env::temp_dir().join(format!("{stem}.mjs"));
    std::fs::write(&wasm_path, bytes).unwrap();

    let export = |id: &str| format!("__spx_test_{}", hex_identity(&DeclarationId::new(id)));
    let script = format!(
        r#"import {{ readFile }} from "node:fs/promises";
const fail = (name) => () => {{ throw new Error(`unexpected host import ${{name}}`); }};
const bytes = await readFile(process.argv[2]);
const {{ instance }} = await WebAssembly.instantiate(bytes, {{ env: {{
  spx_add: fail("spx_add"), spx_sub: fail("spx_sub"), spx_mul: fail("spx_mul"),
  spx_div: fail("spx_div"), spx_rem: fail("spx_rem"), spx_neg: fail("spx_neg"),
  spx_contract_fail: fail("spx_contract_fail"),
}} }});
const memory = instance.exports.__spx_test_memory;
const stack = instance.exports.__spx_test_shadow_stack;
const view = new DataView(memory.buffer);
const input = 1024;
const output = 2048;
const poison = 0xa5;
const poisonOutput = (length) => new Uint8Array(memory.buffer, output, length).fill(poison);
const assertPoison = (length) => {{
  for (const byte of new Uint8Array(memory.buffer, output, length)) if (byte !== poison) throw new Error("variant failure published output");
}};
const assertStack = (label) => {{ if (stack.value !== {stack_top}) throw new Error(`${{label}} stack restore`); }};
view.setUint32(input, 1, true);
view.setBigInt64(input + 8, 42n, true);
poisonOutput(8);
if (instance.exports["{select}"](input, output) !== 0 || view.getBigInt64(output, true) !== 42n) throw new Error("number match");
assertStack("number");
poisonOutput(4);
if (instance.exports["{as_bool}"](input, output) !== 0 || view.getInt32(output, true) !== 1) throw new Error("bool match");
assertStack("bool");
poisonOutput(24);
if (instance.exports["{failing_scrutinee}"](output) !== 9) throw new Error("aggregate failure status");
assertPoison(24);
assertStack("aggregate failure");
poisonOutput(24);
if (instance.exports["{aggregate_failure}"](output) !== 1) throw new Error("aggregate arithmetic failure status");
assertPoison(24);
assertStack("aggregate arithmetic failure");
poisonOutput(24);
if (instance.exports["{make}"](42n, output) !== 0) throw new Error("construct status");
if (view.getUint32(output, true) !== 1 || view.getBigInt64(output + 8, true) !== 42n) throw new Error("construct payload");
for (const offset of [4, 5, 6, 7, 16, 17, 18, 19, 20, 21, 22, 23]) if (view.getUint8(output + offset) !== 0) throw new Error("variant padding not zero");
assertStack("construct");
for (let index = 0; index < 4096; index += 1) {{
  poisonOutput(8);
  if (instance.exports["{selected}"](output) !== 0 || view.getBigInt64(output, true) !== 42n) throw new Error("selected arm");
  assertStack("selected");
  poisonOutput(8);
  if (instance.exports["{selected_failure}"](output) !== 1) throw new Error("selected failure status");
  assertPoison(8);
  assertStack("selected failure");
  if (instance.exports["{construct_order}"](output) !== 4) throw new Error("constructor source order status");
  assertPoison(8);
  assertStack("constructor order");
  if (instance.exports["{scrutinee_first}"](output) !== 9) throw new Error("scrutinee-first status");
  assertPoison(8);
  assertStack("scrutinee first");
  if (instance.exports["{post}"](input, output) !== 10) throw new Error("postcondition status");
  assertPoison(8);
  assertStack("postcondition");
}}
view.setUint32(input, 0xffffffff, true);
poisonOutput(8);
if (instance.exports["{select}"](input, output) !== -1) throw new Error("invalid tag did not fail out-of-band");
assertPoison(8);
assertStack("invalid tag");
if (instance.exports.semaprax_main() !== 42n) throw new Error("public variant result");
assertStack("public");
console.log("variant-wasm-v1-ok");
"#,
        select = export("choice.select"),
        as_bool = export("choice.as_bool"),
        make = export("choice.make"),
        selected = export("choice.selected"),
        selected_failure = export("choice.selected_failure"),
        construct_order = export("choice.construct_order"),
        scrutinee_first = export("choice.scrutinee_first"),
        post = export("choice.post"),
        failing_scrutinee = export("choice.scrutinee"),
        aggregate_failure = export("choice.aggregate_failure"),
        stack_top = SHADOW_STACK_TOP,
    );
    std::fs::write(&script_path, script).unwrap();
    let output = Command::new("node")
        .arg(&script_path)
        .arg(&wasm_path)
        .output()
        .unwrap();
    let _ = std::fs::remove_file(&script_path);
    let _ = std::fs::remove_file(&wasm_path);
    assert!(
        output.status.success(),
        "Node variant runtime failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(
        String::from_utf8_lossy(&output.stdout).trim(),
        "variant-wasm-v1-ok"
    );
}

const OWNED_VARIANT_INVALID_CARRIER_SOURCE: &str = r#"
module test.owned_variant_invalid_carrier;
@id("invalid.choice")
variant Choice {
  @id("invalid.choice.none") None,
  @id("invalid.choice.data") Data {
    @id("invalid.choice.data.payload") payload: Bytes,
  },
}
@id("invalid.consume")
fn consume(value: own Choice) -> i64 {
  match own value {
    Choice::None {} => 0,
    Choice::Data { payload } =>
      if byte_len(bytes_as_slice(payload)) == 0usize { 1 } else { 2 },
  }
}
@id("app.main") fn main() -> i64 { 0 }
"#;

#[test]
fn owned_variant_invalid_carrier_traps_before_cleanup_or_publication() {
    if Command::new("node").arg("--version").output().is_err() {
        return;
    }
    let program = parse(
        OWNED_VARIANT_INVALID_CARRIER_SOURCE,
        Path::new("owned-variant-invalid-carrier-wasm.spx"),
    )
    .unwrap();
    let resolved = hir::resolve(&program).unwrap();
    let bytes = emit_profile(&resolved, true, false).unwrap();
    let id = NEXT_ID.fetch_add(1, Ordering::Relaxed);
    let stem = format!(
        "semaprax-owned-variant-invalid-carrier-wasm-{}-{id}",
        std::process::id()
    );
    let wasm_path = std::env::temp_dir().join(format!("{stem}.wasm"));
    let script_path = std::env::temp_dir().join(format!("{stem}.mjs"));
    std::fs::write(&wasm_path, bytes).unwrap();
    let consume = format!(
        "__spx_test_{}",
        hex_identity(&DeclarationId::new("invalid.consume"))
    );
    let script = format!(
        r#"import {{ readFile }} from "node:fs/promises";
const bytes = await readFile(process.argv[2]);
let drops = 0;
const fail = name => () => {{ throw new Error(`unexpected host import ${{name}}`); }};
const {{instance}} = await WebAssembly.instantiate(bytes, {{env: {{
  spx_add: fail("spx_add"), spx_sub: fail("spx_sub"), spx_mul: fail("spx_mul"),
  spx_div: fail("spx_div"), spx_rem: fail("spx_rem"), spx_neg: fail("spx_neg"),
  spx_contract_fail: fail("spx_contract_fail"),
  spx_bytes_copy: fail("spx_bytes_copy"),
  spx_bytes_drop: () => {{ drops += 1; }},
  spx_bytes_as_slice: fail("spx_bytes_as_slice"),
  spx_bytes_get: fail("spx_bytes_get"),
}}}});
const memory = instance.exports.__spx_test_memory;
const view = new DataView(memory.buffer);
const input = 1024, output = 2048, poison = 0xa5;
view.setUint32(input, 0xffffffff, true);
new Uint8Array(memory.buffer, output, 8).fill(poison);
let trapped = false;
try {{ instance.exports["{consume}"](input, output); }}
catch (error) {{ if (!(error instanceof WebAssembly.RuntimeError)) throw error; trapped = true; }}
if (!trapped) throw new Error("invalid owned variant carrier returned a status");
if (drops !== 0) throw new Error("invalid owned variant carrier ran cleanup");
for (const byte of new Uint8Array(memory.buffer, output, 8)) if (byte !== poison) throw new Error("invalid owned variant carrier published output");
console.log("owned-variant-invalid-carrier-trap-ok");
"#
    );
    std::fs::write(&script_path, script).unwrap();
    let output = Command::new("node")
        .arg(&script_path)
        .arg(&wasm_path)
        .output()
        .unwrap();
    let _ = std::fs::remove_file(script_path);
    let _ = std::fs::remove_file(wasm_path);
    assert!(
        output.status.success(),
        "stdout={} stderr={}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn node_executes_generic_option_result_and_preserves_full_failure_poison() {
    if Command::new("node").arg("--version").output().is_err() {
        return;
    }
    let program = parse(
        GENERIC_VARIANT_SOURCE,
        Path::new("generic-variant-wasm.spx"),
    )
    .unwrap();
    let resolved = hir::resolve(&program).unwrap();
    let bytes = emit_profile(&resolved, true, false).unwrap();
    assert_eq!(bytes, emit_profile(&resolved, true, false).unwrap());
    let id = NEXT_ID.fetch_add(1, Ordering::Relaxed);
    let stem = format!("semaprax-generic-variant-wasm-{}-{id}", std::process::id());
    let wasm_path = std::env::temp_dir().join(format!("{stem}.wasm"));
    let script_path = std::env::temp_dir().join(format!("{stem}.mjs"));
    std::fs::write(&wasm_path, bytes).unwrap();

    let export = |id: &str| format!("__spx_test_{}", hex_identity(&DeclarationId::new(id)));
    let script = format!(
        r#"import {{ readFile }} from "node:fs/promises";
const fail = (name) => () => {{ throw new Error(`unexpected host import ${{name}}`); }};
const bytes = await readFile(process.argv[2]);
const {{ instance }} = await WebAssembly.instantiate(bytes, {{ env: {{
  spx_add: fail("spx_add"), spx_sub: fail("spx_sub"), spx_mul: fail("spx_mul"),
  spx_div: fail("spx_div"), spx_rem: fail("spx_rem"), spx_neg: fail("spx_neg"),
  spx_contract_fail: fail("spx_contract_fail"),
}} }});
const memory = instance.exports.__spx_test_memory;
const stack = instance.exports.__spx_test_shadow_stack;
const view = new DataView(memory.buffer);
const input = 1024;
const output = 2048;
const poison = 0xa5;
const poisonOutput = (length) => new Uint8Array(memory.buffer, output, length).fill(poison);
const assertPoison = (length) => {{
  for (const byte of new Uint8Array(memory.buffer, output, length)) if (byte !== poison) throw new Error("generic failure published output");
}};
const assertStack = (label) => {{ if (stack.value !== {stack_top}) throw new Error(`${{label}} stack restore`); }};
poisonOutput(16);
if (instance.exports["{result_err}"](output) !== 0) throw new Error("Result Err status");
if (view.getUint32(output, true) !== 1 || view.getInt32(output + 8, true) !== 1) throw new Error("Result Err publication");
for (const offset of [4, 5, 6, 7, 12, 13, 14, 15]) if (view.getUint8(output + offset) !== 0) throw new Error("Result padding not zero");
assertStack("Result Err");
for (let index = 0; index < 4096; index += 1) {{
  poisonOutput(16);
  if (instance.exports["{result_failure}"](output) !== 1) throw new Error("Result failure status");
  assertPoison(16);
  assertStack("Result failure");
}}
view.setUint32(input, 0xffffffff, true);
poisonOutput(8);
if (instance.exports["{read_result}"](input, output) !== -1) throw new Error("invalid generic Result tag");
assertPoison(8);
assertStack("invalid Result tag");
if (instance.exports.semaprax_main() !== 43n) throw new Error("generic/prelude public result");
assertStack("public generic result");
console.log("generic-variant-wasm-v2-ok");
"#,
        result_err = export("result.err"),
        result_failure = export("result.failure"),
        read_result = export("result.read"),
        stack_top = SHADOW_STACK_TOP,
    );
    std::fs::write(&script_path, script).unwrap();
    let output = Command::new("node")
        .arg(&script_path)
        .arg(&wasm_path)
        .output()
        .unwrap();
    let _ = std::fs::remove_file(&script_path);
    let _ = std::fs::remove_file(&wasm_path);
    assert!(
        output.status.success(),
        "Node generic/prelude variant runtime failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(
        String::from_utf8_lossy(&output.stdout).trim(),
        "generic-variant-wasm-v2-ok"
    );
}

#[test]
fn node_executes_result_try_reconstruction_status_poison_and_reentry() {
    if Command::new("node").arg("--version").output().is_err() {
        return;
    }
    let program = parse(RESULT_TRY_SOURCE, Path::new("result-try-wasm.spx")).unwrap();
    let resolved = hir::resolve(&program).unwrap();
    let bytes = emit_profile(&resolved, true, false).unwrap();
    assert_eq!(bytes, emit_profile(&resolved, true, false).unwrap());
    let id = NEXT_ID.fetch_add(1, Ordering::Relaxed);
    let stem = format!("semaprax-result-try-wasm-{}-{id}", std::process::id());
    let wasm_path = std::env::temp_dir().join(format!("{stem}.wasm"));
    let script_path = std::env::temp_dir().join(format!("{stem}.mjs"));
    std::fs::write(&wasm_path, bytes).unwrap();

    let export = |id: &str| format!("__spx_test_{}", hex_identity(&DeclarationId::new(id)));
    let script = format!(
        r#"import {{ readFile }} from "node:fs/promises";
const fail = (name) => () => {{ throw new Error(`unexpected host import ${{name}}`); }};
const bytes = await readFile(process.argv[2]);
const {{ instance }} = await WebAssembly.instantiate(bytes, {{ env: {{
  spx_add: fail("spx_add"), spx_sub: fail("spx_sub"), spx_mul: fail("spx_mul"),
  spx_div: fail("spx_div"), spx_rem: fail("spx_rem"), spx_neg: fail("spx_neg"),
  spx_contract_fail: fail("spx_contract_fail"),
}} }});
const memory = instance.exports.__spx_test_memory;
const stack = instance.exports.__spx_test_shadow_stack;
const view = new DataView(memory.buffer);
const input = 1024;
const output = 2048;
const poison = 0xa5;
const poisonOutput = (length) => new Uint8Array(memory.buffer, output, length).fill(poison);
const assertPoison = (length, label) => {{
  for (const byte of new Uint8Array(memory.buffer, output, length)) if (byte !== poison) throw new Error(`${{label}} published output`);
}};
const assertStack = (label) => {{ if (stack.value !== {stack_top}) throw new Error(`${{label}} stack restore`); }};
const assertSmall = (tag, payload, label) => {{
  if (view.getUint32(output, true) !== tag || view.getInt32(output + 4, true) !== payload) throw new Error(`${{label}} value`);
}};
const assertLarge = (tag, payload, label) => {{
  if (view.getUint32(output, true) !== tag) throw new Error(`${{label}} tag`);
  if (tag === 0 && view.getBigInt64(output + 8, true) !== payload) throw new Error(`${{label}} Ok payload`);
  if (tag === 1 && view.getInt32(output + 8, true) !== Number(payload)) throw new Error(`${{label}} Err payload`);
  for (const offset of [4, 5, 6, 7]) if (view.getUint8(output + offset) !== 0) throw new Error(`${{label}} tag padding`);
  if (tag === 1) for (let offset = 9; offset < 16; offset += 1) if (view.getUint8(output + offset) !== 0) throw new Error(`${{label}} payload padding`);
}};

for (let index = 0; index < 4096; index += 1) {{
  poisonOutput(8);
  if (instance.exports["{large_to_small}"](0, 42n, output) !== 0) throw new Error("large-to-small Ok status");
  assertSmall(0, 1, "large-to-small Ok");
  assertStack("large-to-small Ok");

  poisonOutput(8);
  if (instance.exports["{large_to_small}"](1, 42n, output) !== 0) throw new Error("large-to-small Err status");
  assertSmall(1, 1, "large-to-small Err");
  assertStack("large-to-small Err");

  poisonOutput(16);
  if (instance.exports["{small_to_large}"](0, 1, output) !== 0) throw new Error("small-to-large Ok status");
  assertLarge(0, 1n, "small-to-large Ok");
  assertStack("small-to-large Ok");

  poisonOutput(16);
  if (instance.exports["{small_to_large}"](1, 1, output) !== 0) throw new Error("small-to-large Err status");
  assertLarge(1, 1n, "small-to-large Err");
  assertStack("small-to-large Err");

  poisonOutput(8);
  if (instance.exports["{post_err}"](output) !== 10) throw new Error("Err did not run ensures");
  assertPoison(8, "postcondition failure");
  assertStack("postcondition failure");

  poisonOutput(8);
  if (instance.exports["{physical_then_post}"](output) !== 9) throw new Error("physical status was replaced");
  assertPoison(8, "physical failure");
  assertStack("physical failure");

  poisonOutput(8);
  if (instance.exports["{err_skips_later}"](output) !== 0) throw new Error("Err residual status");
  assertSmall(1, 1, "Err skips later body");
  assertStack("Err skips later body");
}}

new Uint8Array(memory.buffer, input, 16).fill(0);
view.setUint32(input, 0xffffffff, true);
poisonOutput(8);
if (instance.exports["{from_input}"](input, output) !== -1) throw new Error("invalid Result tag did not fail out-of-band");
assertPoison(8, "invalid tag");
assertStack("invalid tag");
if (instance.exports.semaprax_main() !== 42n) throw new Error("typed ? public result");
assertStack("public typed ?");
console.log("result-try-wasm-v1-ok");
"#,
        large_to_small = export("try.large_to_small"),
        small_to_large = export("try.small_to_large"),
        post_err = export("try.post_err"),
        physical_then_post = export("try.physical_then_post"),
        err_skips_later = export("try.err_skips_later"),
        from_input = export("try.from_input"),
        stack_top = SHADOW_STACK_TOP,
    );
    std::fs::write(&script_path, script).unwrap();
    let output = Command::new("node")
        .arg(&script_path)
        .arg(&wasm_path)
        .output()
        .unwrap();
    let _ = std::fs::remove_file(&script_path);
    let _ = std::fs::remove_file(&wasm_path);
    assert!(
        output.status.success(),
        "Node typed ? runtime failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(
        String::from_utf8_lossy(&output.stdout).trim(),
        "result-try-wasm-v1-ok"
    );
}

#[test]
fn private_node_resource_records_follow_plan_order_and_finish_with_zero_liveness() {
    if Command::new("node").arg("--version").output().is_err() {
        return;
    }
    let scenario = resource_harness_scenario();
    let bytes = private_resource_harness_wasm(&scenario);
    let id = NEXT_ID.fetch_add(1, Ordering::Relaxed);
    let stem = format!(
        "semaprax-resource-aggregate-wasm-{}-{id}",
        std::process::id()
    );
    let wasm_path = std::env::temp_dir().join(format!("{stem}.wasm"));
    let script_path = std::env::temp_dir().join(format!("{stem}.mjs"));
    std::fs::write(&wasm_path, bytes).unwrap();
    let expected = scenario
        .expected_trace
        .iter()
        .map(u32::to_string)
        .collect::<Vec<_>>()
        .join(",");
    let live = scenario
        .actions
        .iter()
        .filter_map(|action| match action {
            HarnessAction::Store(_, value) => Some(value.to_string()),
            _ => None,
        })
        .collect::<Vec<_>>()
        .join(",");
    let script = format!(
        r#"import {{ readFile }} from "node:fs/promises";
const expected = [{expected}];
const live = new Set([{live}]);
const log = [];
const bytes = await readFile(process.argv[2]);
const {{ instance }} = await WebAssembly.instantiate(bytes, {{ env: {{
  finalize(handle) {{
    if (!live.delete(handle)) throw new Error(`duplicate/unknown finalizer ${{handle}}`);
    log.push(handle);
  }},
}} }});
if (instance.exports.run() !== 1) throw new Error("resource aggregate poison check failed");
if (live.size !== 0) throw new Error(`resource aggregate liveness ${{[...live]}}`);
if (log.join(",") !== expected.join(",")) throw new Error(`resource aggregate order ${{log}}`);
console.log("aggregate-resource-wasm-v1-ok");
"#
    );
    std::fs::write(&script_path, script).unwrap();
    let output = Command::new("node")
        .arg(&script_path)
        .arg(&wasm_path)
        .output()
        .unwrap();
    let _ = std::fs::remove_file(&script_path);
    let _ = std::fs::remove_file(&wasm_path);
    assert!(
        output.status.success(),
        "Node private resource aggregate failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(
        String::from_utf8_lossy(&output.stdout).trim(),
        "aggregate-resource-wasm-v1-ok"
    );
}

fn private_resource_harness_wasm(scenario: &ResourceHarnessScenario) -> Vec<u8> {
    let mut types = Vec::new();
    let mut indexes = std::collections::HashMap::new();
    let finalize_type = intern_type(
        Signature {
            params: vec![I32],
            results: vec![],
        },
        &mut types,
        &mut indexes,
    );
    let run_type = intern_type(
        Signature {
            params: vec![],
            results: vec![I32],
        },
        &mut types,
        &mut indexes,
    );
    let mut module = b"\0asm\x01\0\0\0".to_vec();
    let mut type_section = Vec::new();
    write_u32(&mut type_section, types.len() as u32);
    for signature in &types {
        type_section.push(0x60);
        write_bytes(&mut type_section, &signature.params);
        write_bytes(&mut type_section, &signature.results);
    }
    section(&mut module, 1, type_section);
    let mut imports = Vec::new();
    write_u32(&mut imports, 1);
    function_import(&mut imports, "env", "finalize", finalize_type);
    section(&mut module, 2, imports);
    let mut functions = Vec::new();
    write_u32(&mut functions, 1);
    write_u32(&mut functions, run_type);
    section(&mut module, 3, functions);
    let mut memory = Vec::new();
    write_u32(&mut memory, 1);
    memory.extend([0x00, 0x01]);
    section(&mut module, 5, memory);
    let mut exports = Vec::new();
    write_u32(&mut exports, 1);
    write_name(&mut exports, "run");
    exports.push(0x00);
    write_u32(&mut exports, 1);
    section(&mut module, 7, exports);

    let mut body = vec![0x00];
    for action in &scenario.actions {
        match *action {
            HarnessAction::Store(slot, value) => store(&mut body, wasm_address(slot), value as i32),
            HarnessAction::Transfer(source, destination) => {
                transfer(&mut body, wasm_address(source), wasm_address(destination))
            }
            HarnessAction::Finalize(slot) => finalize(&mut body, wasm_address(slot)),
            HarnessAction::PoisonPartialResult => {
                // Wasm32 Pair is exactly two four-byte resource leaves;
                // poison the entire caller result slot, not just field 0.
                store(&mut body, 2048, 0x7f7f_7f7f);
                store(&mut body, 2052, 0x7f7f_7f7f);
            }
        }
    }
    load(&mut body, 2048);
    body.push(0x41);
    write_i64(&mut body, 0x7f7f_7f7f);
    body.push(0x46);
    load(&mut body, 2052);
    body.push(0x41);
    write_i64(&mut body, 0x7f7f_7f7f);
    body.extend([0x46, 0x71, 0x0b]);
    let mut code = Vec::new();
    write_u32(&mut code, 1);
    write_u32(&mut code, body.len() as u32);
    code.extend(body);
    section(&mut module, 10, code);
    module
}

fn store(body: &mut Vec<u8>, address: i32, value: i32) {
    body.push(0x41);
    write_i64(body, i64::from(address));
    body.push(0x41);
    write_i64(body, i64::from(value));
    body.extend([0x36, 0x02, 0x00]);
}

fn load(body: &mut Vec<u8>, address: i32) {
    body.push(0x41);
    write_i64(body, i64::from(address));
    body.extend([0x28, 0x02, 0x00]);
}

fn transfer(body: &mut Vec<u8>, source: i32, destination: i32) {
    body.push(0x41);
    write_i64(body, i64::from(destination));
    load(body, source);
    body.extend([0x36, 0x02, 0x00]);
    store(body, source, 0);
}

fn finalize(body: &mut Vec<u8>, address: i32) {
    load(body, address);
    body.push(0x10);
    write_u32(body, 0);
    store(body, address, 0);
}
