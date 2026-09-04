use std::path::Path;

use super::{prepare, prepare_with_stdout_transcript, raw_symbol, DataResultType};

const SOURCE: &str = r#"
module test.data_exports;
@id("data.main")
fn main() -> i64 { 0 }
@id("data.length")
fn length(value: borrow Slice<u8>) -> usize { byte_len(value) }
@id("data.present")
fn present(value: borrow Slice<u8>) -> bool {
    match byte_get(value, 0usize) {
        Option::Some { value: byte } => byte == 255u8,
        Option::None {} => false,
    }
}
@id("data.copy")
fn copy(value: borrow Slice<u8>) -> i64 {
    let owned = bytes_copy(value);
    let view = bytes_as_slice(owned);
    if byte_len(view) == 0usize { 0 } else { 1 }
}
@id("data.total")
fn total(left: borrow Slice<u8>, right: borrow Slice<u8>) -> usize {
    byte_len(left) + byte_len(right)
}
@id("data.fail")
fn fail(value: borrow Slice<u8>) -> i64 {
    let owned = bytes_copy(value);
    let view = bytes_as_slice(owned);
    if byte_len(view) == 3usize { 9223372036854775807 + 1 } else { 0 }
}
"#;

#[test]
fn exact_public_abi_is_sorted_and_stable() {
    let parsed = crate::parse(SOURCE, Path::new("data-exports.spx")).unwrap();
    let resolved = crate::hir::resolve(&parsed).unwrap();
    let plans = prepare(
        &resolved,
        &["data.present".to_owned(), "data.length".to_owned()],
    )
    .unwrap();
    assert_eq!(plans[0].stable_id, "data.length");
    assert_eq!(plans[0].result, DataResultType::Usize);
    assert_eq!(plans[1].result, DataResultType::Bool);
    assert_eq!(raw_symbol("data.length"), "spx_data_646174612e6c656e677468");
}

#[test]
fn hostile_public_shapes_and_closed_programs_are_rejected() {
    let parsed = crate::parse(SOURCE, Path::new("data-exports.spx")).unwrap();
    let resolved = crate::hir::resolve(&parsed).unwrap();
    assert!(prepare(&resolved, &[]).is_err());
    assert!(prepare(&resolved, &["missing".to_owned()]).is_err());
    assert!(prepare(
        &resolved,
        &["data.length".to_owned(), "data.length".to_owned()]
    )
    .is_err());
    assert!(prepare(&resolved, &["data.main".to_owned()]).is_err());

    let scalar = crate::parse(
        &SOURCE.replace(
            "fn length(value: borrow Slice<u8>) -> usize { byte_len(value) }",
            "fn length(value: i64) -> usize { 0usize }",
        ),
        Path::new("data-export-scalar.spx"),
    )
    .unwrap();
    assert!(prepare(
        &crate::hir::resolve(&scalar).unwrap(),
        &["data.length".to_owned()]
    )
    .is_err());

    let contracted = crate::parse(
        &SOURCE.replace(
            "fn length(value: borrow Slice<u8>) -> usize {",
            "fn length(value: borrow Slice<u8>) -> usize requires true {",
        ),
        Path::new("data-export-contract.spx"),
    )
    .unwrap();
    assert!(prepare(
        &crate::hir::resolve(&contracted).unwrap(),
        &["data.length".to_owned()]
    )
    .is_ok());
    let effectful = crate::parse(
        &SOURCE
            .replace(
                "module test.data_exports;",
                "module test.data_exports;\npermit { clock.read }",
            )
            .replace(
                "fn length(value: borrow Slice<u8>) -> usize {",
                "fn length(value: borrow Slice<u8>) -> usize uses { clock.read } {",
            ),
        Path::new("data-export-effect.spx"),
    )
    .unwrap();
    let error = prepare(
        &crate::hir::resolve(&effectful).unwrap(),
        &["data.length".to_owned()],
    )
    .unwrap_err();
    assert_eq!(error.code, "SPX-W121");
    assert!(error.message.contains("does not admit permits"));
}

/// A false `requires` publishes status 9 and a false `ensures` status 10
/// through the data status global with a zero result carrier, exactly like an
/// arithmetic failure, and the module stays usable afterwards. Contract
/// callees and types meet the same closed profile as bodies.
#[test]
fn contract_failures_publish_contract_statuses_and_leave_the_module_usable() {
    use std::process::Command;

    const CONTRACTED: &str = r#"
module test.data_contracts;
@id("data.main")
fn main() -> i64 { 0 }
@id("data.bounded")
fn bounded(value: borrow Slice<u8>) -> usize
    requires byte_len(value) <= 2usize
    ensures result <= 2usize
{
    byte_len(value)
}
@id("data.positive")
fn positive(value: borrow Slice<u8>) -> i64
    ensures result > 0
{
    if byte_len(value) == 0usize { 0 } else { 1 }
}
@id("data.helper")
fn helper(value: borrow Slice<u8>) -> bool
    requires byte_len(value) <= 4usize
{
    byte_len(value) > 1usize
}
@id("data.via_helper")
fn via_helper(value: borrow Slice<u8>) -> bool { helper(value) }
"#;
    let parsed = crate::parse(CONTRACTED, Path::new("data-contracts.spx")).unwrap();
    let resolved = crate::hir::resolve(&parsed).unwrap();
    let exports = [
        "data.bounded".to_owned(),
        "data.positive".to_owned(),
        "data.via_helper".to_owned(),
    ];
    prepare(&resolved, &exports).unwrap();
    let wasm = crate::wasm::emit_resolved_module_with_byte_exports(&resolved, &exports).unwrap();
    assert_eq!(
        wasm,
        crate::wasm::emit_resolved_module_with_byte_exports(&resolved, &exports).unwrap()
    );
    if Command::new("node").arg("--version").output().is_err() {
        return;
    }
    let root = std::env::temp_dir().join(format!(
        "semaprax-data-contracts-{}-{}",
        std::process::id(),
        wasm.len()
    ));
    std::fs::create_dir(&root).unwrap();
    let wasm_path = root.join("app.wasm");
    let script_path = root.join("test.mjs");
    std::fs::write(&wasm_path, &wasm).unwrap();
    let script = format!(
        r#"import {{ readFile }} from "node:fs/promises";
const entries = new Map(); let next = 1; let instance;
const decode = carrier => {{ const word=BigInt.asUintN(64,carrier), length=Number(word&0xffffffffn), root=Number((word>>32n)&0xffffffffn); if(length>65536)throw Error("length"); return {{word,length,root,tagged:(root&0x80000000)!==0,token:root&0x7fffffff}}; }};
const read = decoded => {{ if(decoded.tagged){{const value=entries.get(decoded.token);if(!(value instanceof Uint8Array)||value.length!==decoded.length)throw Error("stale");return value;}} const memory=new Uint8Array(instance.exports.memory.buffer);if(decoded.root>memory.length-decoded.length)throw Error("range");return memory.slice(decoded.root,decoded.root+decoded.length); }};
const allocate = bytes => {{const token=next++, owned=new Uint8Array(bytes);entries.set(token,owned);return BigInt.asIntN(64,((0x80000000n|BigInt(token))<<32n)|BigInt(owned.length));}};
const imports={{env:{{
spx_add:(a,b)=>a+b,spx_sub:(a,b)=>a-b,spx_mul:(a,b)=>a*b,spx_div:(a,b)=>a/b,spx_rem:(a,b)=>a%b,spx_neg:a=>-a,spx_contract_fail:()=>{{throw Error("contract import reached");}},
spx_bytes_copy:c=>allocate(read(decode(c))),spx_bytes_get:(c,i)=>{{const b=read(decode(c)),u=BigInt.asUintN(64,i);return u>=BigInt(b.length)?-1:b[Number(u)];}},spx_bytes_drop:c=>{{const d=decode(c);read(d);entries.delete(d.token);}},spx_bytes_as_slice:c=>{{const d=decode(c);read(d);return BigInt.asIntN(64,d.word);}}
}}}};
({{instance}}=await WebAssembly.instantiate(await readFile(process.argv[2]),imports));
const e=instance.exports, memory=new Uint8Array(e.memory.buffer);
memory.set([1,2,3,4,5],0);
if(e["{bounded}"](0,2)!==2n||e.__spx_data_status_v1.value!==0)throw Error("bounded-ok");
if(e["{bounded}"](0,3)!==0n||e.__spx_data_status_v1.value!==9)throw Error("requires-status");
if(e["{bounded}"](0,1)!==1n||e.__spx_data_status_v1.value!==0)throw Error("usable-after-requires");
if(e["{positive}"](0,1)!==1n||e.__spx_data_status_v1.value!==0)throw Error("positive-ok");
if(e["{positive}"](0,0)!==0n||e.__spx_data_status_v1.value!==10)throw Error("ensures-status");
if(e["{positive}"](0,2)!==1n||e.__spx_data_status_v1.value!==0)throw Error("usable-after-ensures");
if(e["{via_helper}"](0,2)!==1||e.__spx_data_status_v1.value!==0)throw Error("helper-ok");
if(e["{via_helper}"](0,5)!==0||e.__spx_data_status_v1.value!==9)throw Error("helper-requires-status");
if(e["{via_helper}"](0,1)!==0||e.__spx_data_status_v1.value!==0)throw Error("usable-after-helper");
console.log("public-data-contracts-ok");
"#,
        bounded = raw_symbol("data.bounded"),
        positive = raw_symbol("data.positive"),
        via_helper = raw_symbol("data.via_helper"),
    );
    std::fs::write(&script_path, script).unwrap();
    let output = Command::new("node")
        .arg(&script_path)
        .arg(&wasm_path)
        .output()
        .unwrap();
    let _ = std::fs::remove_file(&script_path);
    let _ = std::fs::remove_file(&wasm_path);
    let _ = std::fs::remove_dir(&root);
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(output.stdout, b"public-data-contracts-ok\n");
}

#[test]
fn command_stdout_accepts_only_selected_external_slice_roots() {
    const EXTERNAL: &str = r#"
module test.command_external;
permit { process.stdout.write }
@id("command.run")
fn run(input: borrow Slice<u8>) -> bool uses { process.stdout.write } {
    let alias = input;
    stdout_write(alias) == byte_len(input)
}
@id("app.main") fn main() -> i64 { 0 }
"#;
    let parsed = crate::parse(EXTERNAL, Path::new("command-external.spx")).unwrap();
    let resolved = crate::hir::resolve(&parsed).unwrap();
    prepare_with_stdout_transcript(&resolved, &["command.run".to_owned()]).unwrap();

    for (name, replacement) in [
            (
                "array",
                "let local = [65u8]; let view = array_as_slice(local); stdout_write(view) == 1usize",
            ),
            (
                "owned",
                "let owned = bytes_copy(input); let view = bytes_as_slice(owned); stdout_write(view) == byte_len(input)",
            ),
        ] {
            let hostile = EXTERNAL.replace(
                "let alias = input;\n    stdout_write(alias) == byte_len(input)",
                replacement,
            );
            let parsed = crate::parse(
                &hostile,
                Path::new(&format!("command-hostile-{name}.spx")),
            )
            .unwrap();
            let resolved = crate::hir::resolve(&parsed).unwrap();
            let error = prepare_with_stdout_transcript(&resolved, &["command.run".to_owned()])
                .unwrap_err();
            assert_eq!(error.code, "SPX-W121");
            assert!(error.message.contains("external Slice parameter"));
        }

    let helper = EXTERNAL
        .replace(
            "@id(\"command.run\")",
            r#"@id("command.helper")
fn helper(input: borrow Slice<u8>) -> usize uses { process.stdout.write } {
    stdout_write(input)
}
@id("command.run")"#,
        )
        .replace("stdout_write(alias)", "helper(alias)");
    let parsed = crate::parse(&helper, Path::new("command-helper-write.spx")).unwrap();
    let resolved = crate::hir::resolve(&parsed).unwrap();
    let error = prepare_with_stdout_transcript(&resolved, &["command.run".to_owned()]).unwrap_err();
    assert_eq!(error.code, "SPX-W121");
    assert!(error.message.contains("selected command boundary"));
}

#[test]
fn throwing_checked_import_cannot_expose_staged_stdout_bytes() {
    use std::process::Command;

    if Command::new("node").arg("--version").output().is_err() {
        return;
    }
    const COMMAND: &str = r#"
module test.command_throw;
permit { process.stdout.write }
@id("command.run")
fn run(input: borrow Slice<u8>) -> bool uses { process.stdout.write } {
    let written = stdout_write(input);
    match byte_get(input, 0usize) {
        Option::Some { value } => written == byte_len(input) && value == 65u8,
        Option::None {} => false,
    }
}
@id("app.main") fn main() -> i64 { 0 }
"#;
    let parsed = crate::parse(COMMAND, Path::new("command-throw.spx")).unwrap();
    let resolved = crate::hir::resolve(&parsed).unwrap();
    let plans = prepare_with_stdout_transcript(&resolved, &["command.run".to_owned()]).unwrap();
    let wasm = crate::wasm::aggregate::emit_byte_exports_with_stdout_transcript(&resolved, &plans)
        .unwrap();
    wasmparser::Validator::new().validate_all(&wasm).unwrap();

    let root = std::env::temp_dir().join(format!(
        "semaprax-command-throw-{}-{}",
        std::process::id(),
        wasm.len()
    ));
    std::fs::create_dir(&root).unwrap();
    let wasm_path = root.join("app.wasm");
    let script_path = root.join("probe.mjs");
    std::fs::write(&wasm_path, wasm).unwrap();
    let script = format!(
        r#"import {{ readFile }} from "node:fs/promises";
let instance;
const imports={{env:{{
spx_add:(a,b)=>a+b,spx_sub:(a,b)=>a-b,spx_mul:(a,b)=>a*b,spx_div:(a,b)=>a/b,spx_rem:(a,b)=>a%b,spx_neg:a=>-a,spx_contract_fail:()=>{{throw Error("contract");}},
spx_bytes_copy:()=>{{throw Error("unused copy");}},spx_bytes_get:()=>{{throw Error("injected checked read failure");}},spx_bytes_drop:()=>{{throw Error("unused drop");}},spx_bytes_as_slice:()=>{{throw Error("unused slice");}}
}}}};
({{instance}}=await WebAssembly.instantiate(await readFile(process.argv[2]),imports));
const e=instance.exports,memory=new Uint8Array(e.memory.buffer);memory.set([65,0,66],0);
let failed=false;try{{e["{symbol}"](0,3)}}catch{{failed=true}}if(!failed)throw Error("import failure hidden");
if(e.__spx_stdout_length_v1.value!==0)throw Error("failed length published");
if(memory.subarray(131072,196608).some(byte=>byte!==0))throw Error("staged bytes escaped");
console.log("command-throw-pristine");
"#,
        symbol = raw_symbol("command.run")
    );
    std::fs::write(&script_path, script).unwrap();
    let output = Command::new("node")
        .arg(&script_path)
        .arg(&wasm_path)
        .output()
        .unwrap();
    let _ = std::fs::remove_file(&script_path);
    let _ = std::fs::remove_file(&wasm_path);
    let _ = std::fs::remove_dir(&root);
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(output.stdout, b"command-throw-pristine\n");
}

#[test]
fn core_wasm_exports_only_the_exact_public_boundary_and_node_executes_it() {
    use std::process::Command;

    use wasmparser::{ExternalKind, Parser, Payload, TypeRef, Validator};

    let parsed = crate::parse(SOURCE, Path::new("data-exports-node.spx")).unwrap();
    let selected = [
        "data.copy".to_owned(),
        "data.fail".to_owned(),
        "data.length".to_owned(),
        "data.present".to_owned(),
        "data.total".to_owned(),
    ];
    let first = crate::wasm::emit_module_with_byte_exports(&parsed, &selected).unwrap();
    let second = crate::wasm::emit_module_with_byte_exports(&parsed, &selected).unwrap();
    assert_eq!(first, second);
    Validator::new().validate_all(&first).unwrap();

    let mut imports = Vec::new();
    let mut exports = Vec::new();
    for payload in Parser::new(0).parse_all(&first) {
        match payload.unwrap() {
            Payload::ImportSection(section) => {
                for import in section.into_imports() {
                    let import = import.unwrap();
                    let TypeRef::Func(_) = import.ty else {
                        panic!("data profile imported non-function authority")
                    };
                    imports.push((import.module.to_owned(), import.name.to_owned()));
                }
            }
            Payload::ExportSection(section) => {
                for export in section {
                    let export = export.unwrap();
                    exports.push((export.name.to_owned(), export.kind));
                }
            }
            _ => {}
        }
    }
    assert_eq!(
        imports,
        [
            "spx_add",
            "spx_sub",
            "spx_mul",
            "spx_div",
            "spx_rem",
            "spx_neg",
            "spx_contract_fail",
            "spx_bytes_copy",
            "spx_bytes_get",
            "spx_bytes_drop",
            "spx_bytes_as_slice",
        ]
        .map(|name| ("env".to_owned(), name.to_owned()))
    );
    assert_eq!(
        exports,
        vec![
            ("memory".to_owned(), ExternalKind::Memory),
            ("__spx_data_status_v1".to_owned(), ExternalKind::Global),
            (
                "__spx_data_scratch_base_v1".to_owned(),
                ExternalKind::Global
            ),
            (
                "__spx_data_scratch_capacity_v1".to_owned(),
                ExternalKind::Global
            ),
            (raw_symbol("data.copy"), ExternalKind::Func),
            (raw_symbol("data.fail"), ExternalKind::Func),
            (raw_symbol("data.length"), ExternalKind::Func),
            (raw_symbol("data.present"), ExternalKind::Func),
            (raw_symbol("data.total"), ExternalKind::Func),
        ]
    );

    if Command::new("node").arg("--version").output().is_err() {
        return;
    }
    let root = std::env::temp_dir().join(format!(
        "semaprax-data-exports-{}-{}",
        std::process::id(),
        first.len()
    ));
    std::fs::create_dir(&root).unwrap();
    let wasm_path = root.join("app.wasm");
    let script_path = root.join("test.mjs");
    std::fs::write(&wasm_path, &first).unwrap();
    let script = format!(
        r#"import {{ readFile }} from "node:fs/promises";
const entries = new Map(); let next = 1; let instance;
const decode = carrier => {{ const word=BigInt.asUintN(64,carrier), length=Number(word&0xffffffffn), root=Number((word>>32n)&0xffffffffn); if(length>65536)throw Error("length"); return {{word,length,root,tagged:(root&0x80000000)!==0,token:root&0x7fffffff}}; }};
const read = decoded => {{ if(decoded.tagged){{const value=entries.get(decoded.token);if(!(value instanceof Uint8Array)||value.length!==decoded.length)throw Error("stale");return value;}} const memory=new Uint8Array(instance.exports.memory.buffer);if(decoded.root>memory.length-decoded.length)throw Error("range");return memory.slice(decoded.root,decoded.root+decoded.length); }};
const allocate = bytes => {{const token=next++, owned=new Uint8Array(bytes);entries.set(token,owned);return BigInt.asIntN(64,((0x80000000n|BigInt(token))<<32n)|BigInt(owned.length));}};
const imports={{env:{{
spx_add:(a,b)=>a+b,spx_sub:(a,b)=>a-b,spx_mul:(a,b)=>a*b,spx_div:(a,b)=>a/b,spx_rem:(a,b)=>a%b,spx_neg:a=>-a,spx_contract_fail:()=>{{throw Error("contract");}},
spx_bytes_copy:c=>allocate(read(decode(c))),spx_bytes_get:(c,i)=>{{const b=read(decode(c)),u=BigInt.asUintN(64,i);return u>=BigInt(b.length)?-1:b[Number(u)];}},spx_bytes_drop:c=>{{const d=decode(c);read(d);entries.delete(d.token);}},spx_bytes_as_slice:c=>{{const d=decode(c);read(d);return BigInt.asIntN(64,d.word);}}
}}}};
({{instance}}=await WebAssembly.instantiate(await readFile(process.argv[2]),imports));
const e=instance.exports, memory=new Uint8Array(e.memory.buffer);
if(memory.length!==131072)throw Error("memory"); let fixed=false;try{{e.memory.grow(1)}}catch{{fixed=true}}if(!fixed)throw Error("grow");
if(e.__spx_data_scratch_base_v1.value!==0||e.__spx_data_scratch_capacity_v1.value!==65536)throw Error("metadata");
memory.set([255,0,7],0);
if(e["{length}"](0,3)!==3n||e.__spx_data_status_v1.value!==0)throw Error("length");
if(e["{present}"](0,3)!==1||e.__spx_data_status_v1.value!==0)throw Error("bool");
if(e["{copy}"](0,3)!==1n||entries.size!==0||e.__spx_data_status_v1.value!==0)throw Error("copy-cleanup");
if(e["{fail}"](0,3)!==0n||entries.size!==0||e.__spx_data_status_v1.value!==1)throw Error("failure-cleanup");
if(e["{length}"](65536,0)!==0n||e.__spx_data_status_v1.value!==0)throw Error("empty-boundary");
if(e["{length}"](0,65536)!==65536n||e.__spx_data_status_v1.value!==0)throw Error("exact-root-capacity");
if(e["{length}"](0,65537)!==0n||e.__spx_data_status_v1.value!==11)throw Error("root-capacity-plus-one");
if(e["{length}"](65536,1)!==0n||e.__spx_data_status_v1.value!==11)throw Error("range-status");
if(e["{length}"](-1,0)!==0n||e.__spx_data_status_v1.value!==11)throw Error("unsigned-offset");
if(e["{total}"](0,40000,0,30000)!==0n||e.__spx_data_status_v1.value!==11)throw Error("cumulative");
if(e["{total}"](0,32768,32768,32768)!==65536n||e.__spx_data_status_v1.value!==0)throw Error("exact-capacity");
console.log("public-data-core-wasm-ok");
"#,
        length = raw_symbol("data.length"),
        present = raw_symbol("data.present"),
        copy = raw_symbol("data.copy"),
        fail = raw_symbol("data.fail"),
        total = raw_symbol("data.total"),
    );
    std::fs::write(&script_path, script).unwrap();
    let output = Command::new("node")
        .arg(&script_path)
        .arg(&wasm_path)
        .output()
        .unwrap();
    let _ = std::fs::remove_file(&script_path);
    let _ = std::fs::remove_file(&wasm_path);
    let _ = std::fs::remove_dir(&root);
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(output.stdout, b"public-data-core-wasm-ok\n");
}
