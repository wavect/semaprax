//! Additive Wasm text/numeric operation gates; frozen standalone profiles stay separate.
use super::owned_string_loops_v1::support::Fixture;
use semaprax::{parse, verify, wasm};
use std::path::Path;
use std::process::Command;

fn wasm_case(body: &str, expected: &str) {
    let source =
        format!("module test.wasm_conversions;\n@id(\"app.main\") fn main() -> i64 {{ {body} }}\n");
    let program = parse(&source, Path::new("wasm-conversions.spx")).unwrap();
    assert!(verify::verify(&program).is_empty());
    let fixture = Fixture::new(&source);
    let root = fixture.root.join("web");
    wasm::build_web(&program, &root).unwrap();
    // The first text fixture peaks at six canonical owners: text, its slice
    // argument clone, piece, its byte-at argument clone, the find text clone,
    // and the needle literal. Own String reads clone even for borrowed builtin
    // parameters, and CleanupPlan retains them until the lexical scope ends.
    // Instrument only the fixture host ledger to assert zero live owners after
    // every entry independently of the exact six-entry quota.
    let runtime_path = root.join("semaprax.js");
    let runtime = std::fs::read_to_string(&runtime_path).unwrap();
    assert_eq!(runtime.matches("entries.set(token, owned);").count(), 1);
    assert_eq!(runtime.matches("entries.delete(decoded.token);").count(), 1);
    let runtime = runtime
        .replace(
            "entries.set(token, owned);",
            "entries.set(token, owned); globalThis.__toolkitLiveOwners++;",
        )
        .replace(
            "entries.delete(decoded.token);",
            "entries.delete(decoded.token); globalThis.__toolkitLiveOwners--;",
        );
    std::fs::write(runtime_path, runtime).unwrap();
    let probe = format!(
        r#"import {{readFile}} from 'node:fs/promises';
import {{instantiateBytes,semanticStatus}} from './semaprax.js';
const bytes=await readFile('./app.wasm');
globalThis.__toolkitLiveOwners=0;
const {{instance}}=await instantiateBytes(bytes,{{maxOwnedByteEntries:6}});
for(let i=0;i<8;i++) {{
  {expected}
  if(globalThis.__toolkitLiveOwners!==0) throw Error('checked text owner leak');
}}
"#
    );
    std::fs::write(root.join("probe.mjs"), probe).unwrap();
    let output = Command::new("node")
        .arg(root.join("probe.mjs"))
        .current_dir(&root)
        .output()
        .expect("Node is required for additive Wasm conversion parity");
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(output.stdout.is_empty());
    let names = [
        "app.wasm",
        "semaprax.js",
        "index.html",
        "package.json",
        "semaprax.manifest.json",
        "probe.mjs",
    ];
    assert_eq!(std::fs::read_dir(&root).unwrap().count(), names.len());
    for name in names {
        std::fs::remove_file(root.join(name)).unwrap();
    }
    std::fs::remove_dir(root).unwrap();
    fixture.cleanup();
}

#[test]
fn wasm_numeric_conversions_round_trip_extrema_and_ties_to_even() {
    for (body, value) in [
        (
            "i64_from_f64(f64_from_i64(9007199254740993))",
            "9007199254740992n",
        ),
        (
            "i64_from_f64(-9223372036854775808.0)",
            "-9223372036854775808n",
        ),
        ("i64_from_f64(-1.9)", "-1n"),
        (
            "i64_from_usize(usize_from_i64(9223372036854775807))",
            "9223372036854775807n",
        ),
    ] {
        wasm_case(body, &format!("if(instance.exports.semaprax_main()!=={value}) throw Error('numeric conversion value changed');"));
    }
}

#[test]
fn wasm_numeric_conversion_failures_keep_their_domain_and_settle_owners() {
    // An authored record selects aggregate lowering for an owned String held
    // across the checked conversion. Repeated failed entries require cleanup.
    for (operation, code) in [
        ("i64_from_f64(9223372036854775808.0)", 1),
        ("i64_from_f64(0.0 / 0.0)", 2),
        ("usize_from_i64(-1)", 1),
        ("i64_from_usize(18446744073709551615usize)", 1),
    ] {
        let body =
            format!("let text = \"held\"; let converted = {operation}; string_len(text) + 0");
        let expected = format!("let failed=false;try{{instance.exports.semaprax_main();}}catch(error){{const status=semanticStatus(error);if(status===null||status.domain_id!=='semaprax.convert.v1'||status.code!=={code})throw error;failed=true;}}if(!failed)throw Error('missing checked conversion failure');");
        // String-only Web packages retain the frozen scalar route. Adding a
        // harmless record is an explicit aggregate selection, not a bypass.
        let source_body = body;
        let source = format!("module test.wasm_conversion_cleanup;\n@id(\"marker\") record Marker {{ @id(\"marker.code\") code: i64, }}\n@id(\"app.main\") fn main() -> i64 {{ {source_body} }}\n");
        let program = parse(&source, Path::new("conversion-cleanup.spx")).unwrap();
        assert!(verify::verify(&program).is_empty());
        let fixture = Fixture::new(&source);
        let root = fixture.root.join("web");
        wasm::build_web(&program, &root).unwrap();
        std::fs::write(root.join("probe.mjs"), format!("import {{readFile}} from 'node:fs/promises'; import {{instantiateBytes,semanticStatus}} from './semaprax.js'; const {{instance}}=await instantiateBytes(await readFile('./app.wasm'),{{maxOwnedByteEntries:2}});for(let i=0;i<8;i++){{{expected}}}")).unwrap();
        let output = Command::new("node")
            .arg(root.join("probe.mjs"))
            .current_dir(&root)
            .output()
            .expect("Node is required for conversion failure settlement");
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert!(output.stdout.is_empty());
        let names = [
            "app.wasm",
            "semaprax.js",
            "index.html",
            "package.json",
            "semaprax.manifest.json",
            "probe.mjs",
        ];
        assert_eq!(std::fs::read_dir(&root).unwrap().count(), names.len());
        for name in names {
            std::fs::remove_file(root.join(name)).unwrap();
        }
        std::fs::remove_dir(root).unwrap();
        fixture.cleanup();
    }
}

#[test]
fn wasm_checked_text_byte_semantics_parse_and_borrowed_copy() {
    for (body, value) in [
        (
            r#"let text = "a\u{0}é😀"; let piece = string_slice(text, 2, 4); string_byte_at(piece, 0) + string_find(text, "😀", 1)"#,
            "199n",
        ),
        (
            r#"let text = " \t\u{b}é\r\n"; let trimmed = string_trim(text); string_len(trimmed)"#,
            "2n",
        ),
        (
            r#"let text = "copy\u{0}é"; let view = string_as_str(text); let copy = string_from_str(view); string_len(copy)"#,
            "7n",
        ),
        (
            r#"match string_to_i64("-9223372036854775808") { Option::Some { value: n } => if n == -9223372036854775808 { 1 } else { 0 }, Option::None {} => 0, }"#,
            "1n",
        ),
        (
            r#"match string_to_i64("9223372036854775808") { Option::Some { value: n } => 0, Option::None {} => 1, }"#,
            "1n",
        ),
        (r#"string_find("é", "", 1)"#, "1n"),
    ] {
        wasm_case(body, &format!("if(instance.exports.semaprax_main()!=={value})throw Error('checked text result changed');"));
    }
}

#[test]
fn wasm_checked_text_failures_settle_borrowed_owners_before_status() {
    for (body, code) in [
        (
            r#"let text = "held"; string_len(string_slice(text, -1, 2))"#,
            1,
        ),
        (r#"let text = "é"; string_len(string_slice(text, 1, 2))"#, 2),
        (r#"let text = "held"; string_byte_at(text, 4)"#, 1),
        (r#"let text = "held"; string_find(text, "", 5)"#, 1),
    ] {
        wasm_case(body, &format!("let caught=false;try{{instance.exports.semaprax_main();}}catch(error){{const status=semanticStatus(error);if(status===null||status.domain_id!=='semaprax.text.v1'||status.code!=={code})throw error;caught=true;}}if(!caught)throw Error('text failure missing');"));
    }
}

fn standalone_case(source: &str, ids: &[&str], probe: &str) {
    standalone_profile_case(
        source,
        ids,
        probe,
        wasm::internal_strings::emit_text_toolkit_module,
        "semaprax.wasm-text-toolkit.v1",
    );
}

fn standalone_profile_case(
    source: &str,
    ids: &[&str],
    probe: &str,
    emitter: fn(
        &semaprax::ast::Program,
        &[String],
        wasm::internal_strings::InternalStringOptions,
    ) -> Result<
        wasm::internal_strings::InternalStringModule,
        semaprax::diagnostic::Diagnostic,
    >,
    schema: &str,
) {
    use wasm::internal_strings::InternalStringOptions;
    let program = parse(source, Path::new("standalone-toolkit.spx")).unwrap();
    let diagnostics = verify::verify(&program);
    assert!(diagnostics.is_empty(), "{diagnostics:?}");
    let canonical = semaprax::format::canonical(&program);
    let round_trip = parse(&canonical, Path::new("standalone-toolkit-roundtrip.spx")).unwrap();
    assert_eq!(semaprax::format::canonical(&round_trip), canonical);
    assert_eq!(
        semaprax::graph::to_json(&program).unwrap(),
        semaprax::graph::to_json(&round_trip).unwrap()
    );
    let ids = ids.iter().map(|id| (*id).to_owned()).collect::<Vec<_>>();
    let artifact = emitter(&program, &ids, InternalStringOptions::default()).unwrap();
    let repeated = emitter(&round_trip, &ids, InternalStringOptions::default()).unwrap();
    assert_eq!(artifact.wasm_bytes(), repeated.wasm_bytes());
    assert_eq!(artifact.descriptor(), repeated.descriptor());
    assert_eq!(artifact.runtime_source(), repeated.runtime_source());
    assert!(artifact.descriptor().contains(schema));
    let fixture = Fixture::new(source);
    std::fs::write(fixture.root.join("app.wasm"), artifact.wasm_bytes()).unwrap();
    std::fs::write(fixture.root.join("runtime.mjs"), artifact.runtime_source()).unwrap();
    std::fs::write(fixture.root.join("probe.mjs"), format!("import {{readFile}} from 'node:fs/promises';import {{webcrypto}} from 'node:crypto';if(globalThis.crypto===undefined)Object.defineProperty(globalThis,'crypto',{{value:webcrypto}});import {{instantiate}} from './runtime.mjs';const bytes=await readFile('./app.wasm');const runtime=await instantiate(new Uint8Array(bytes));{probe}")).unwrap();
    let output = Command::new("node")
        .arg(fixture.root.join("probe.mjs"))
        .current_dir(&fixture.root)
        .output()
        .expect("Node is required for standalone toolkit settlement");
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(output.stdout.is_empty());
    for name in ["app.wasm", "runtime.mjs", "probe.mjs"] {
        std::fs::remove_file(fixture.root.join(name)).unwrap();
    }
    fixture.cleanup();
}

const STANDALONE: &str = r#"
module test.standalone_toolkit;
@id("choice") variant Choice {
    @id("choice.empty") Empty,
    @id("choice.text") Text { @id("choice.text.value") value: string, @id("choice.text.marker") marker: i64, },
}
@id("text-option") variant TextOption {
    @id("text-option.absent") Absent,
    @id("text-option.present") Present { @id("text-option.present.value") value: string, },
}
@id("make") fn make() -> Choice { Choice::Text { value: "a\u{0}é😀", marker: 2 } }
@id("borrow") fn measure(value: borrow Choice) -> i64 {
    match borrow value { Choice::Empty {} => 0, Choice::Text { value: text, marker } => string_len(text) + marker, }
}
@id("consume") fn consume(value: own Choice) -> i64 {
    match own value { Choice::Empty {} => 0, Choice::Text { value: text, marker } => string_len(string_slice(text, 2, 4)) + marker, }
}
@id("app.variants") fn variants() -> i64 {
    let value = make(); let borrowed = measure(value); borrowed + consume(value) + consume(Choice::Empty {})
}
@id("app.option") fn option() -> i64 { let value=TextOption::Present { value:"text" }; match own value { TextOption::Present { value:text } => string_len(text), TextOption::Absent {} => 0, } }
@id("app.text") fn text() -> i64 {
    let owned = " \té\r\n"; let trimmed = string_trim(owned);
    let view = string_as_str(trimmed); let copied = string_from_str(view);
    let number = match string_to_i64("-9223372036854775808") { Option::Some { value: n } => if n == -9223372036854775808 { 10 } else { 0 }, Option::None {} => 0, };
    string_byte_at(copied, 0) + string_find(copied, "", 1) + number
}
@id("app.ordering") fn ordering() -> bool {
    "a" < "a\u{0}" && "a\u{0}" <= "a\u{0}" && "😀" > "\u{e000}" && "é" >= "z"
}
@id("app.failed") fn failed() -> i64 { let value = make(); let borrowed = measure(value); string_len(string_slice("é", 1, 2)) + borrowed }
@id("app.convert") fn convert() -> i64 { let owned = "held"; i64_from_f64(f64_from_i64(9007199254740993)) + string_len(owned) }
@id("app.integer") fn integer() -> i64 { let owned = "held"; i64_from_i32(-3i32) + i64_from_usize(usize_from_u8(255u8)) + string_len(owned) }
@id("app.replace") fn replace() -> i64 { let mut value=""; let mut i=0; while i<3 { value=string_concat(value,string_from_i64(i)); i=i+1; 0 } string_len(value) }
@id("app.guard") fn guarded() -> i64 {
    let mut i=0; let mut sum=0;
    while i<3 { let choice=Option<i64>::Some { value:i }; sum=sum+match choice { Option::Some { value:n } if string_len(string_trim(" x "))>0 => n, _ => 99, }; i=i+1; 0 }
    sum
}
@id("app.main") fn main() -> i64 { 0 }
"#;

#[test]
fn standalone_toolkit_transports_owned_variants_and_checked_text_with_bounded_settlement() {
    standalone_case(
        STANDALONE,
        &[
            "app.variants",
            "app.text",
            "app.ordering",
            "app.failed",
            "app.convert",
            "app.option",
            "app.integer",
            "app.replace",
            "app.guard",
        ],
        r#"
for(let i=0;i<8;i++){
  for(const [id,value] of [['app.variants',14n],['app.text',206n],['app.ordering',true],['app.convert',9007199254740996n],['app.option',4n],['app.integer',256n],['app.replace',3n],['app.guard',3n]]){
    const result=runtime.call(id);if(result.kind!=='success'||result.value!==value)throw Error('toolkit value '+id);
  }
  const failed=runtime.call('app.failed');if(failed.kind!=='failure'||failed.domain!=='semaprax.text.v1'||failed.code!==2)throw Error('toolkit failure changed');
}
const altered=new Uint8Array(bytes);altered[altered.length-1]^=1;
let rejected=false;try{await instantiate(altered)}catch{rejected=true}if(!rejected)throw Error('forged artifact accepted');
"#,
    );
}

#[test]
fn standalone_toolkit_condition_temporaries_settle_before_both_bool_outcomes() {
    let source = r#"module test.toolkit_condition;
@id("app.loop") fn looped() -> i64 { let mut i=0; while i<3 && string_len(string_concat("a",string_from_i64(i)))>0 { i=i+1; 0 } i }
@id("app.false") fn empty() -> i64 { let mut i=0; while string_len(string_trim(" \t"))>0 { i=i+1; 0 } i }
@id("app.lazy") fn lazy() -> i64 { let mut i=0; while false && string_len(string_slice("é",1,2))>0 { i=i+1; 0 } i }
@id("app.text_failure") fn text_failure() -> i64 { let mut i=0; while string_len(string_slice(string_concat("é","x"),1,2))>0 { i=i+1; 0 } i }
@id("app.arithmetic_failure") fn arithmetic_failure() -> i64 { let mut i=0; while string_len(string_concat("held",""))>0 && 9223372036854775807+1>0 { i=i+1; 0 } i }
@id("app.main") fn main() -> i64 { 0 }
"#;
    standalone_case(
        source,
        &[
            "app.loop",
            "app.false",
            "app.lazy",
            "app.text_failure",
            "app.arithmetic_failure",
        ],
        r#"
for(let i=0;i<8;i++){
 for(const [id,value] of [['app.loop',3n],['app.false',0n],['app.lazy',0n]]){const result=runtime.call(id);if(result.kind!=='success'||result.value!==value)throw Error('condition value changed')}
 for(const [id,domain,code] of [['app.text_failure','semaprax.text.v1',2],['app.arithmetic_failure','semaprax.arithmetic.v1',1]]){const result=runtime.call(id);if(result.kind!=='failure'||result.domain!==domain||result.code!==code)throw Error('condition failure changed')}
}
"#,
    );
}

#[test]
fn standalone_toolkit_file_text_requires_explicit_provider_and_checks_bytes() {
    let source = r#"module test.toolkit_files;
permit { fs.read }
@id("app.file") fn file() -> i64 uses { fs.read } { let path="folder/data.txt"; let view=string_as_str(path); let text=file_read_text(view); string_len(text) }
@id("app.path") fn path() -> i64 uses { fs.read } { let path="../data.txt"; let view=string_as_str(path); string_len(file_read_text(view)) }
@id("app.main") fn main() -> i64 { 0 }
"#;
    standalone_case(
        source,
        &["app.file", "app.path"],
        r#"
for(let i=0;i<8;i++){const result=runtime.call('app.file');if(result.kind!=='failure'||result.domain!=='semaprax.filesystem.v1'||result.code!==6)throw Error('ambient file authority acquired')}
let calls=0;
const provider=await instantiate(new Uint8Array(bytes),{fileReadText:{read(path,maximum){calls++;if(new TextDecoder().decode(path)!=='folder/data.txt'||maximum!==65536)throw Error('provider request changed');return {ok:true,bytes:new Uint8Array([97,0,195,169])}}}});
for(let i=0;i<8;i++){const result=provider.call('app.file');if(result.kind!=='success'||result.value!==4n)throw Error('file text changed')}
const badPath=provider.call('app.path');if(badPath.kind!=='failure'||badPath.domain!=='semaprax.filesystem.v1'||badPath.code!==1||calls!==8)throw Error('invalid path reached provider');
const invalid=await instantiate(new Uint8Array(bytes),{fileReadText:{read(){return {ok:true,bytes:new Uint8Array([192,128])}}}});
for(let i=0;i<8;i++){const result=invalid.call('app.file');if(result.kind!=='failure'||result.domain!=='semaprax.text.v1'||result.code!==3)throw Error('invalid UTF8 file accepted')}
const oversized=await instantiate(new Uint8Array(bytes),{fileReadText:{read(){return {ok:true,bytes:new Uint8Array(65537)}}}});
const large=oversized.call('app.file');if(large.kind!=='failure'||large.domain!=='semaprax.filesystem.v1'||large.code!==4)throw Error('file bound changed');
const forged=await instantiate(new Uint8Array(bytes),{fileReadText:{read(){return {ok:false,code:99}}}});
let rejected=false;try{forged.call('app.file')}catch{rejected=true}if(!rejected)throw Error('forged provider status accepted');
let poisoned=false;try{forged.call('app.file')}catch{poisoned=true}if(!poisoned)throw Error('forged provider status did not poison instance');
"#,
    );
}

#[test]
fn toolkit_web_route_authenticates_descriptor_and_keeps_fresh_publication() {
    let source = r#"module test.toolkit_web;
@id("app.main") fn main() -> i64 { string_len(string_trim(" é ")) }
"#;
    let fixture = Fixture::new(source);
    let output = fixture.root.join("web");
    wasm::internal_strings::build_toolkit_web_from_source(
        &fixture.source,
        &output,
        &["app.main".to_owned()],
    )
    .unwrap();
    let descriptor =
        std::fs::read_to_string(output.join("semaprax.internal-strings.json")).unwrap();
    let manifest = std::fs::read_to_string(output.join("semaprax.manifest.json")).unwrap();
    assert!(descriptor.contains("semaprax.wasm-text-toolkit.v1"));
    assert!(manifest.contains("semaprax.web-text-toolkit.v1"));
    assert!(manifest.contains("\"capabilities\":[]"));
    let declarations = std::fs::read_to_string(output.join("semaprax.d.ts")).unwrap();
    assert!(declarations.contains("semaprax.text.v1"));
    assert!(declarations.contains("fileReadText?"));
    let files = [
        "app.wasm",
        "semaprax.js",
        "semaprax.d.ts",
        "semaprax.internal-strings.json",
        "semaprax.manifest.json",
        "package.json",
        "index.html",
        "app.js",
    ];
    assert_eq!(std::fs::read_dir(&output).unwrap().count(), files.len());
    assert!(wasm::internal_strings::build_toolkit_web_from_source(
        &fixture.source,
        &output,
        &["app.main".to_owned()]
    )
    .is_err());
    assert_eq!(
        std::fs::read_to_string(output.join("semaprax.internal-strings.json")).unwrap(),
        descriptor
    );
    for file in files {
        std::fs::remove_file(output.join(file)).unwrap();
    }
    std::fs::remove_dir(output).unwrap();
    fixture.cleanup();
}

#[test]
fn general_loop_match_scalar_helpers_admit_i32_f32_f64_without_widening_frozen_selectors() {
    use wasm::internal_strings::{
        emit_copy_variant_module, emit_general_loop_match_module, emit_module,
        InternalStringOptions,
    };
    let source = r#"module test.general_copy_scalars;
@id("guard.i32") fn i32_guard(value:i32) -> bool { value+1i32 == -2i32 }
@id("guard.f32") fn f32_guard(value:f32) -> bool { value+0.25f32 == 1.75f32 }
@id("guard.f64") fn f64_guard(value:f64) -> bool { value*2.0 == 4.5 }
@id("app.main") fn main() -> i64 {
    let mut i=0; let mut sum=0;
    while i<2 {
        let choice=Option<i64>::Some { value:i };
        sum=sum+match choice { Option::Some { value:n } if i32_guard(-3i32) && f32_guard(1.5f32) && f64_guard(2.25) => n, _ => 99, };
        i=i+1;
        0
    }
    sum
}
"#;
    let program = parse(source, Path::new("general-copy-scalars.spx")).unwrap();
    let ids = ["app.main".to_owned()];
    for emitter in [emit_module, emit_copy_variant_module] {
        assert_eq!(
            emitter(&program, &ids, InternalStringOptions::default())
                .unwrap_err()
                .code,
            "SPX-W111"
        );
    }
    standalone_profile_case(source, &["app.main"], "for(let i=0;i<8;i++){const result=runtime.call('app.main');if(result.kind!=='success'||result.value!==1n)throw Error('general Copy scalar guard changed');}", emit_general_loop_match_module, "semaprax.wasm-internal-strings.v1");
}

const TOOLKIT_COLLECTIONS: &str = r#"module test.generated_toolkit_collections;
@id("carrier") record Carrier { @id("carrier.values") values: Map<i64, string>, @id("carrier.label") label: string, }
@id("carrier.unpack") fn unpack(value: own Carrier) -> Map<i64, string> { match own value { Carrier { values, label } => { let ignored=string_len(label); values }, } }
@id("collection.forward") fn forward(values: own Map<i64, string>) -> Map<i64, string> { values }
@id("collection.count") fn count(values: borrow Map<i64, string>) -> i64 { i64_from_usize(map_len<i64, string>(values)) }
@id("app.typed") fn typed() -> i64 {
 let values0=map_new<i64, string>(2usize);
 let values1=map_set<i64, string>(values0,-3,"a\u{0}é");
 let values2=forward(unpack(Carrier { values:values1, label:"discarded" }));
 let read=map_get_or<i64, string>(values2,-3,"");
 let extended=string_concat(read,"!");
 let values3=map_set<i64, string>(values2,-3,"z");
 let output=map_value_at<i64, string>(values3,0usize);
 let size=count(values3);
 let values4=map_remove<i64, string>(values3,-3);
 if string_len(extended)==5 && output=="z" && size==1 && map_len<i64,string>(values4)==0usize { 9 } else { -1 }
}
@id("app.legacy") fn legacy() -> i64 {
 let values0=map_new(2usize);let values1=map_add(values0,"é",2);let values2=map_add(values1,"é",5);
 let values3=map_set(values2,"a\u{0}",11);let key=map_key_at(values3,0usize);
 let values4=map_remove(values3,"é");if string_len(key)==2 && map_len(values4)==1usize { map_get_or(values4,key,0) } else { -1 }
}
@id("app.set") fn set() -> i64 {
 let values0=set_new<string>(2usize);let values1=set_insert<string>(values0,"é");let values2=set_insert<string>(values1,"a\u{0}");
 let values3=set_insert<string>(values2,"é");let values4=set_remove<string>(values3,"missing");let key=set_key_at<string>(values4,0usize);
 let values5=set_remove<string>(values4,"é");if string_len(key)==2 && set_has<string>(values5,key) && set_len<string>(values5)==1usize { 12 } else { -1 }
}
@id("app.typed_full") fn typed_full() -> i64 { let values=map_new<i64,i64>(0usize);let rejected=map_set<i64,i64>(values,1,2);0 }
@id("app.legacy_full") fn legacy_full() -> i64 { let values=map_new(0usize);let rejected=map_set(values,"held",1);0 }
@id("app.typed_index") fn typed_index() -> i64 { let values=map_new<i64,i64>(0usize);map_value_at<i64,i64>(values,0usize) }
@id("app.legacy_index") fn legacy_index() -> i64 { let values=map_new(0usize);string_len(map_key_at(values,0usize)) }
@id("app.typed_capacity") fn typed_capacity() -> i64 { let values=map_new<i64,i64>(65537usize);0 }
@id("app.legacy_capacity") fn legacy_capacity() -> i64 { let values=map_new(65537usize);0 }
@id("app.typed_overflow") fn typed_overflow() -> i64 { let values0=map_new<i64,i64>(1usize);let values1=map_add<i64,i64>(values0,1,9223372036854775807);let rejected=map_add<i64,i64>(values1,1,1);0 }
@id("app.legacy_overflow") fn legacy_overflow() -> i64 { let values0=map_new(1usize);let values1=map_add(values0,"x",9223372036854775807);let rejected=map_add(values1,"x",1);0 }
@id("app.text_failure") fn text_failure() -> i64 { let values0=set_new<i64>(1usize);let values1=set_insert<i64>(values0,2);string_len(string_slice("é",1,2)) }
@id("app.owner_quota") fn owner_quota() -> i64 { let first=map_new<i64,i64>(1usize);let second=set_new<i64>(1usize);i64_from_usize(map_len<i64,i64>(first)) }
@id("app.byte_quota") fn byte_quota() -> i64 { let values0=map_new<i64,string>(1usize);let values1=map_set<i64,string>(values0,1,"oversized");0 }
@id("app.main") fn main() -> i64 { 0 }
"#;

#[test]
fn generated_toolkit_web_collections_copy_order_remove_and_settle_every_status() {
    use wasm::internal_strings::{
        emit_copy_variant_module, emit_general_loop_match_module, emit_module,
        InternalStringOptions,
    };
    let program = parse(
        TOOLKIT_COLLECTIONS,
        Path::new("generated-toolkit-collections.spx"),
    )
    .unwrap();
    assert!(verify::verify(&program).is_empty());
    let selected = [
        "app.typed",
        "app.legacy",
        "app.set",
        "app.typed_full",
        "app.legacy_full",
        "app.typed_index",
        "app.legacy_index",
        "app.typed_capacity",
        "app.legacy_capacity",
        "app.typed_overflow",
        "app.legacy_overflow",
        "app.text_failure",
        "app.owner_quota",
        "app.byte_quota",
    ];
    let ids = selected
        .iter()
        .map(|id| (*id).to_owned())
        .collect::<Vec<_>>();
    for emitter in [
        emit_module,
        emit_copy_variant_module,
        emit_general_loop_match_module,
    ] {
        assert!(emitter(&program, &ids, InternalStringOptions::default()).is_err());
    }
    let fixture = Fixture::new(TOOLKIT_COLLECTIONS);
    let output = fixture.root.join("web");
    wasm::internal_strings::build_toolkit_web_from_source(&fixture.source, &output, &ids).unwrap();
    let declarations = std::fs::read_to_string(output.join("semaprax.d.ts")).unwrap();
    assert!(declarations.contains("'semaprax.map.v1'|'semaprax.map.v2'"));
    assert!(declarations.contains("maxOwnedCollections?: number"));
    let runtime = std::fs::read_to_string(output.join("semaprax.js")).unwrap();
    assert!(runtime.contains("collections.settle()"));
    assert!(!runtime.contains("collections.clear("));
    std::fs::write(output.join("probe.mjs"),r#"import {readFile} from 'node:fs/promises';
import {webcrypto} from 'node:crypto';if(globalThis.crypto===undefined)Object.defineProperty(globalThis,'crypto',{value:webcrypto});
import {instantiate} from './semaprax.js';
const bytes=new Uint8Array(await readFile('./app.wasm'));
const module=await WebAssembly.compile(bytes),imports=WebAssembly.Module.imports(module);
const tail=imports.slice(-2);if(tail[0].module!=='env'||tail[0].name!=='spx_collection_checked_v2'||tail[1].module!=='env'||tail[1].name!=='spx_collection_drop_v2')throw Error('collection import tail changed');
const runtime=await instantiate(bytes,{maxOwnedCollections:1,maxOwnedCollectionBytes:8});
for(let repeat=0;repeat<8;repeat++){
 for(const [id,value] of [['app.typed',9n],['app.legacy',11n],['app.set',12n]]){const result=runtime.call(id);if(result.kind!=='success'||result.value!==value)throw Error('collection result '+id)}
 for(const prefix of ['typed','legacy'])for(const [suffix,code] of [['full',1],['index',2],['capacity',3],['overflow',4]]){
  const result=runtime.call('app.'+prefix+'_'+suffix);if(result.kind!=='failure'||result.domain!==(prefix==='legacy'?'semaprax.map.v1':'semaprax.map.v2')||result.code!==code)throw Error('collection status '+prefix+' '+suffix);
 }
 const text=runtime.call('app.text_failure');if(text.kind!=='failure'||text.domain!=='semaprax.text.v1'||text.code!==2)throw Error('selected Text failure changed');
 for(const [id,cause] of [['app.owner_quota','collection_owners'],['app.byte_quota','collection_bytes']]){const result=runtime.call(id);if(result.kind!=='capacity'||result.cause!==cause)throw Error('collection quota did not settle '+id)}
}
"#).unwrap();
    let status = Command::new("node")
        .arg(output.join("probe.mjs"))
        .current_dir(&output)
        .output()
        .unwrap();
    assert!(
        status.status.success(),
        "{}",
        String::from_utf8_lossy(&status.stderr)
    );
    assert!(status.stdout.is_empty());
    for file in [
        "app.wasm",
        "semaprax.js",
        "semaprax.d.ts",
        "semaprax.internal-strings.json",
        "semaprax.manifest.json",
        "package.json",
        "index.html",
        "app.js",
        "probe.mjs",
    ] {
        std::fs::remove_file(output.join(file)).unwrap();
    }
    std::fs::remove_dir(output).unwrap();
    fixture.cleanup();
}

#[test]
fn standalone_toolkit_collection_string_quota_refusal_settles_prior_owners() {
    use wasm::internal_strings::{emit_text_toolkit_module, InternalStringOptions};
    let source = r#"module test.collection_string_quota;
@id("app.quota") fn quota() -> i64 { let values0=map_new<i64,string>(1usize);let values1=map_set<i64,string>(values0,1,"text");string_len(map_get_or<i64,string>(values1,1,"")) }
@id("app.scalar") fn scalar() -> i64 { 7 }
@id("app.main") fn main() -> i64 { 0 }
"#;
    let program = parse(source, Path::new("collection-string-quota.spx")).unwrap();
    assert!(verify::verify(&program).is_empty());
    let artifact = emit_text_toolkit_module(
        &program,
        &["app.quota".to_owned(), "app.scalar".to_owned()],
        InternalStringOptions {
            max_cumulative_bytes: 4,
            ..Default::default()
        },
    )
    .unwrap();
    let fixture = Fixture::new(source);
    std::fs::write(fixture.root.join("app.wasm"), artifact.wasm_bytes()).unwrap();
    std::fs::write(fixture.root.join("runtime.mjs"), artifact.runtime_source()).unwrap();
    std::fs::write(fixture.root.join("probe.mjs"),r#"import {readFile} from 'node:fs/promises';import {webcrypto} from 'node:crypto';if(globalThis.crypto===undefined)Object.defineProperty(globalThis,'crypto',{value:webcrypto});import {instantiate} from './runtime.mjs';
const runtime=await instantiate(new Uint8Array(await readFile('./app.wasm')),{maxOwnedCollections:1});
for(let i=0;i<8;i++){const result=runtime.call('app.quota');if(result.kind!=='capacity'||result.cause!=='cumulative_bytes')throw Error('String result quota changed');const scalar=runtime.call('app.scalar');if(scalar.kind!=='success'||scalar.value!==7n)throw Error('quota poisoned settled instance')}
"#).unwrap();
    let status = Command::new("node")
        .arg(fixture.root.join("probe.mjs"))
        .current_dir(&fixture.root)
        .output()
        .unwrap();
    assert!(
        status.status.success(),
        "{}",
        String::from_utf8_lossy(&status.stderr)
    );
    for file in ["app.wasm", "runtime.mjs", "probe.mjs"] {
        std::fs::remove_file(fixture.root.join(file)).unwrap();
    }
    fixture.cleanup();
}

#[test]
fn standalone_toolkit_collection_atoms_preserve_every_closed_scalar_value() {
    let mut source = String::from("module test.toolkit_collection_atoms;\n");
    let atoms = [
        ("i64", "-9223372036854775808", "0"),
        ("i32", "-2147483647i32", "0i32"),
        ("u8", "255u8", "0u8"),
        ("usize", "18446744073709551615usize", "0usize"),
        ("char", "'😀'", "'x'"),
        ("bool", "true", "false"),
        ("f32", "2.5f32", "0.0f32"),
        ("f64", "-2.5", "0.0"),
    ];
    let mut ids = Vec::new();
    for (index, (ty, value, fallback)) in atoms.into_iter().enumerate() {
        let id = format!("app.atom{index}");
        ids.push(id.clone());
        source.push_str(&format!("@id(\"{id}\") fn atom{index}() -> i64 {{ let values0=map_new<bool,{ty}>(1usize);let values1=map_set<bool,{ty}>(values0,false,{value});if map_get_or<bool,{ty}>(values1,false,{fallback})=={value} && map_value_at<bool,{ty}>(values1,0usize)=={value} && !map_key_at<bool,{ty}>(values1,0usize) {{ 1 }} else {{ 0 }} }}\n"));
    }
    source.push_str("@id(\"app.main\") fn main() -> i64 {0}\n");
    let selected = ids.iter().map(String::as_str).collect::<Vec<_>>();
    standalone_case(&source,&selected,"for(let repeat=0;repeat<8;repeat++)for(let i=0;i<8;i++){const result=runtime.call('app.atom'+i);if(result.kind!=='success'||result.value!==1n)throw Error('collection scalar atom changed '+i)}");
}
