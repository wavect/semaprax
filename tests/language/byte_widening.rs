//! Byte Widening v1: exact scalar conversion plus existing borrowed byte views.
use std::path::Path;
use std::process::Command;

use super::owned_string_loops_v1::support::Fixture;
use semaprax::interpreter::{self, InterpreterOptions};
use semaprax::{codegen, format, graph, hir, parse, verify, wasm};

const SCALARS: &str = r#"module test.byte_widening;

@id("byte.widen")
fn widen(value: u8) -> i64
    ensures result >= 0 && result <= 255 && result == i64_from_u8(value)
{
    i64_from_u8(value)
}

@id("app.main")
fn main() -> i64
{
    let mut byte = 0u8;
    let mut total = 0;
    while i64_from_u8(byte) < 255 {
        total = total + widen(byte);
        byte = byte + 1u8;
        0
    }
    let lazy = true || i64_from_u8(255u8 + 1u8) > 0;
    if lazy && i64_from_u8(0u8) == 0 && i64_from_u8(128u8) == 128 {
        total + widen(byte)
    } else {
        -1
    }
}
"#;

// Byte offsets deliberately split a multi-byte scalar: reads preserve the
// UTF-8 bytes and embedded NUL, not character semantics. Every large index
// returns None before a physical address/index is narrowed.
const BORROWED: &str = r#"module test.borrowed_byte_widening;

@id("byte.read")
fn read(text: borrow str, index: usize) -> i64
{
    let view = str_as_bytes(text);
    match byte_get(view, index) { Option::Some { value: byte } => i64_from_u8(byte), Option::None {} => -1, }
}

@id("app.main")
fn main() -> i64
{
    let text = "é\u{0}";
    let raw = string_as_str(text);
    let empty = "";
    let raw_empty = string_as_str(empty);
    read(raw, 0usize) + read(raw, 1usize) + read(raw, 2usize) + read(raw, 3usize) + read(raw, 4294967296usize) + read(raw, 18446744073709551615usize) + read(raw_empty, 0usize)
}
"#;

fn available(command: &str) -> bool {
    Command::new(command).arg("--version").output().is_ok()
}

#[test]
fn byte_widening_source_graph_and_hostile_hir() {
    for source in [SCALARS, BORROWED] {
        let ast = parse(source, Path::new("byte-widening.spx")).unwrap();
        assert!(verify::verify(&ast).is_empty());
        let canonical = format::canonical(&ast);
        let roundtrip = parse(&canonical, Path::new("canonical.spx")).unwrap();
        assert_eq!(graph::revision(&ast), graph::revision(&roundtrip));
        assert_eq!(
            graph::to_json(&ast).unwrap(),
            graph::to_json(&roundtrip).unwrap()
        );
        assert!(graph::to_json(&ast)
            .unwrap()
            .contains("\"callee\":\"core.num.i64_from_u8\""));
        hir::validate(&hir::resolve(&ast).unwrap()).unwrap();
    }
    let ast = parse(SCALARS, Path::new("hostile.spx")).unwrap();
    for generic in [false, true] {
        let mut program = hir::resolve(&ast).unwrap();
        let function = program
            .functions
            .iter_mut()
            .find(|function| function.id.as_str() == "byte.widen")
            .unwrap();
        let hir::ResolvedExprKind::Block { tail, .. } = &mut function.body.kind else {
            panic!("block")
        };
        let hir::ResolvedExprKind::Call {
            args,
            type_arguments,
            ..
        } = &mut tail.kind
        else {
            panic!("intrinsic call")
        };
        if generic {
            type_arguments.push(hir::ResolvedType::U8);
        } else {
            args[0].ty = hir::ResolvedType::I64;
        }
        assert_eq!(hir::validate(&program).unwrap_err().code, "SPX-H006");
    }
}

#[test]
fn byte_widening_preserves_exact_source_diagnostics() {
    for (body, code) in [
        ("i64_from_u8(1)", "SPX-T205"),
        ("i64_from_u8(1usize)", "SPX-T205"),
        ("i64_from_u8(1u8, 2u8)", "SPX-T204"),
        ("i64_from_u8()", "SPX-T204"),
    ] {
        let source = format!("module t; @id(\"app.main\") fn main()->i64 {{ {body} }}");
        let ast = parse(&source, Path::new("invalid.spx")).unwrap();
        assert!(verify::verify(&ast)
            .iter()
            .any(|diagnostic| diagnostic.code == code));
        assert!(hir::resolve(&ast)
            .unwrap_err()
            .iter()
            .any(|diagnostic| diagnostic.code == code));
    }
    let ast = parse("module t; @id(\"t.widen\") fn i64_from_u8(value:u8)->i64 { 0 } @id(\"app.main\") fn main()->i64 { 0 }", Path::new("reserved.spx")).unwrap();
    assert!(verify::verify(&ast)
        .iter()
        .any(|diagnostic| diagnostic.code == "SPX-S113"));
    // The old fallible numeric family remains separately refused by Wasm.
    let old = parse(
        "module t; @id(\"app.main\") fn main()->i64 { i64_from_usize(1usize) }",
        Path::new("old.spx"),
    )
    .unwrap();
    assert_eq!(wasm::emit_module(&old).unwrap_err().code, "SPX-W116");
}

#[test]
fn byte_widening_interpreter_and_native_all_bytes_and_borrowed_text() {
    for (source, expected) in [(SCALARS, "32640"), (BORROWED, "360")] {
        let mut fixture = Fixture::new(source);
        let result = interpreter::interpret(
            &fixture.source,
            "app.main",
            &[],
            &InterpreterOptions::default(),
        )
        .unwrap();
        let envelope: serde_json::Value = serde_json::from_str(&result.envelope).unwrap();
        assert_eq!(
            envelope["payload"]["outcome"]["kind"], "returned",
            "{}",
            result.envelope
        );
        assert_eq!(envelope["payload"]["outcome"]["value"], expected);
        if available("clang") {
            let ast = parse(source, Path::new("native.spx")).unwrap();
            let generated = codegen::emit_c(&ast).unwrap();
            let symbol = "app.main"
                .bytes()
                .map(|byte| format!("{byte:02x}"))
                .collect::<String>();
            let allocations = if source == SCALARS { 0 } else { 2 };
            let probe = format!("{}\n{}\n{generated}\n#undef malloc\n#undef free\nint main(void) {{\nREQUIRE(fixture_binary_stdout());\nstruct spx_status_entry entries[32]; struct spx_context context={{0}}; REQUIRE(spx_context_init(&context,19,entries,32,NULL,NULL,NULL));\nint64_t value=-1; REQUIRE(spx_decl_{symbol}(&context,&value)==0); REQUIRE(value==INT64_C({expected})); REQUIRE(fixture_allocations=={allocations} && fixture_frees=={allocations} && fixture_live==0); (void)fixture_malloc; (void)fixture_free; return 0; }}\n", include_str!("../support/native_fixture_stdio.c"), include_str!("../native_owned_utf8_settlement_v1/allocations.c"));
            for optimization in ["-O0", "-O2"] {
                assert_eq!(fixture.native(&probe, optimization), "");
            }
        }
        fixture.cleanup();
    }
}

#[test]
fn byte_widening_core_wasm_all_bytes_and_borrowed_text() {
    if !available("node") {
        return;
    }
    for (source, expected) in [(SCALARS, "32640"), (BORROWED, "360")] {
        let ast = parse(source, Path::new("wasm.spx")).unwrap();
        let bytes = wasm::emit_module(&ast).unwrap();
        assert_eq!(bytes, wasm::emit_module(&ast).unwrap());
        let mut fixture = Fixture::new(source);
        fixture.write("program.wasm", bytes);
        let script = fixture.write("probe.mjs", r#"import {readFileSync} from 'node:fs';
const fail = () => { throw new Error('unexpected host call'); };
const checked = value => {
  if(value < -9223372036854775808n || value > 9223372036854775807n) throw new Error('arithmetic overflow');
  return value;
};
const bytes = readFileSync('program.wasm');
let memory,next=1,allocations=0,drops=0;
const owners=new Map();
const span = carrier => {
  const bits=BigInt.asUintN(64,carrier), pointer=Number(bits>>32n), length=Number(bits&0xffffffffn);
  if(pointer&0x80000000) {
    const owned=owners.get(pointer&0x7fffffff);
    if(!owned||owned.length!==length) throw new Error('stale owner');
    return owned;
  }
  if(pointer>memory.buffer.byteLength || length>memory.buffer.byteLength-pointer) throw new Error('span');
  return new Uint8Array(memory.buffer,pointer,length);
};
const {instance} = await WebAssembly.instantiate(bytes, {env: {
spx_add:(a,b)=>checked(a+b), spx_sub:(a,b)=>checked(a-b), spx_mul:(a,b)=>checked(a*b),
spx_div:(a,b)=>checked(a/b), spx_rem:(a,b)=>checked(a%b), spx_neg:a=>checked(-a), spx_contract_fail:fail,
spx_bytes_copy:carrier=>{
  const bytes=new Uint8Array(span(carrier)),id=next++;owners.set(id,bytes);allocations++;
  return BigInt.asIntN(64,((0x80000000n|BigInt(id))<<32n)|BigInt(bytes.length));
},
spx_bytes_drop:carrier=>{
  span(carrier);const id=Number(BigInt.asUintN(64,carrier)>>32n)&0x7fffffff;
  if(!owners.delete(id)) throw new Error('double drop');drops++;
},
spx_bytes_as_slice:carrier=>{span(carrier);return carrier;},
spx_bytes_get:(carrier,index)=>{
  const bytes=span(carrier);
  if(typeof index!=='bigint') throw new Error('index');
  const offset=BigInt.asUintN(64,index);
  return offset>=BigInt(bytes.length)?-1:bytes[Number(offset)];
},
}});
memory=instance.exports.__spx_byte_memory||instance.exports.memory;
for (let repeat=0;repeat<3;repeat++) {
  const value=instance.exports.semaprax_main();
  if(value!==BigInt(process.argv[2])) throw new Error(`result ${value}`);
  if(owners.size!==0 || allocations!==drops) throw new Error('leaked owner');
}
"#);
        let output = Command::new("node")
            .current_dir(&fixture.root)
            .arg(script)
            .arg(expected)
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{}: {}",
            fixture.root.display(),
            String::from_utf8_lossy(&output.stderr)
        );
        fixture.cleanup();
    }
}

#[test]
fn byte_widening_internal_string_profile_selection_stays_explicit() {
    use wasm::internal_strings::{emit_copy_variant_module, emit_module, InternalStringOptions};
    let source = "module t; @id(\"app.main\") fn main()->i64 { i64_from_u8(255u8) }";
    let ast = parse(source, Path::new("profile.spx")).unwrap();
    let ids = ["app.main".to_owned()];
    assert_eq!(
        emit_module(&ast, &ids, InternalStringOptions::default())
            .unwrap_err()
            .code,
        "SPX-W111"
    );
    let artifact = emit_copy_variant_module(&ast, &ids, InternalStringOptions::default()).unwrap();
    if !available("node") {
        return;
    }
    let mut fixture = Fixture::new(source);
    fixture.write("program.wasm", artifact.wasm_bytes());
    fixture.write("program.mjs", artifact.runtime_source());
    let script = fixture.write(
        "probe.mjs",
        r#"import {readFileSync} from 'node:fs';
import {instantiate} from './program.mjs';
const api=await instantiate(Uint8Array.from(readFileSync('program.wasm')));
for(let repeat=0;repeat<3;repeat++) {
  const result=api.call('app.main');
  if(result.kind!=='success'||result.value!==255n) throw new Error(JSON.stringify(result));
}
"#,
    );
    let output = Command::new("node")
        .current_dir(&fixture.root)
        .arg(script)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    fixture.cleanup();
}
