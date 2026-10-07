//! Checked Map host statuses and private arena settlement across repeated calls.
use super::owned_string_loops_v1::support::Fixture;
use semaprax::{parse, verify, wasm};
use std::path::Path;
use std::process::Command;
fn run(source: &str, expectation: &str) {
    let program = parse(source, Path::new("wasm-map-v2.spx")).unwrap();
    assert!(verify::verify(&program).is_empty());
    let fixture = Fixture::new(source);
    let root = fixture.root.join("web");
    wasm::build_web(&program, &root).unwrap();
    let probe = format!(
        r#"import {{readFile}} from 'node:fs/promises';
import {{instantiateBytes,semanticStatus}} from './semaprax.js';
const bytes=await readFile('./app.wasm');
const {{instance}}=await instantiateBytes(bytes,{{maxOwnedCollections:1,maxOwnedByteEntries:8,maxOwnedCollectionBytes:1024}});
for(let i=0;i<12;i++){{ {expectation} }}
"#
    );
    std::fs::write(root.join("probe.mjs"), probe).unwrap();
    let output = Command::new("node")
        .arg(root.join("probe.mjs"))
        .current_dir(&root)
        .output()
        .expect("Node required for checked Map arena gate");
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(output.stdout.is_empty());
    for name in [
        "app.wasm",
        "semaprax.js",
        "index.html",
        "package.json",
        "semaprax.manifest.json",
        "probe.mjs",
    ] {
        std::fs::remove_file(root.join(name)).unwrap();
    }
    std::fs::remove_dir(root).unwrap();
    fixture.cleanup();
}
#[test]
fn wasm_typed_map_string_values_and_record_transport_settle_each_invocation() {
    run(r#"module test.wasm_map;
@id("map.carrier") record Carrier {@id("map.carrier.words") words:Map<i64,string>,@id("map.carrier.name") name:string,}
@id("map.wrap") fn wrap(words:own Map<i64,string>)->Carrier {Carrier {words:words,name:"tag\u{0}"}}
@id("map.read") fn read(carrier:borrow Carrier)->i64 {string_len(map_get_or<i64,string>(carrier.words,-3,"missing"))+string_len(carrier.name)}
@id("map.main") fn main()->i64 {let mut words=map_new<i64,string>(2usize);words=map_set<i64,string>(words,9,"last");words=map_set<i64,string>(words,-3,"hello");let carrier=wrap(words);read(carrier)}"#,
        "if(instance.exports.semaprax_main()!==9n)throw Error('Map record transport value changed');");
}
#[test]
fn wasm_set_order_duplicates_and_missing_removal_are_deterministic() {
    run(
        r#"module test.wasm_set;
@id("set.main") fn main()->i64 {let mut keys=set_new<i64>(2usize);keys=set_insert<i64>(keys,7);keys=set_insert<i64>(keys,-2);keys=set_insert<i64>(keys,7);keys=set_remove<i64>(keys,99);if set_len<i64>(keys)==2usize && set_key_at<i64>(keys,0usize)==-2 && set_has<i64>(keys,7) {11}else{-1}}"#,
        "if(instance.exports.semaprax_main()!==11n)throw Error('Set order/duplicate changed');",
    );
}
#[test]
fn wasm_map_checked_failures_keep_domain_and_release_staged_owners() {
    for (body,domain,code) in [
        ("let mut m=map_new<i64,bool>(1usize);m=map_set<i64,bool>(m,1,true);m=map_set<i64,bool>(m,2,false);0","semaprax.map.v2",1),
        ("let m=map_new<i64,string>(1usize);let s=map_value_at<i64,string>(m,0usize);string_len(s)","semaprax.map.v2",2),
        ("let m=set_new<bool>(65537usize);0","semaprax.map.v2",3),
        ("let mut m=map_new(1usize);m=map_set(m,\"x\",9223372036854775807);m=map_add(m,\"x\",1);0","semaprax.map.v1",4),
    ] {
        let source=format!("module test.wasm_map_failure; @id(\"marker\") record Marker {{ @id(\"marker.code\") code:i64, }} @id(\"map.main\") fn main()->i64 {{{body}}}");
        run(&source,&format!("let failed=false;try{{instance.exports.semaprax_main();}}catch(error){{const status=semanticStatus(error);if(status===null||status.domain_id!=={domain:?}||status.code!=={code})throw error;failed=true;}}if(!failed)throw Error('Map checked failure missing');"));
    }
}
#[test]
fn wasm_map_floating_values_survive_storage() {
    run(
        r#"module test.wasm_map_float;
@id("map.main") fn main()->i64 {let mut m=map_new<bool,f64>(2usize);m=map_set<bool,f64>(m,false,-0.0);m=map_set<bool,f64>(m,true,3.5);if map_get_or<bool,f64>(m,true,0.0)==3.5 && map_value_at<bool,f64>(m,0usize)==-0.0 {17}else{-1}}"#,
        "if(instance.exports.semaprax_main()!==17n)throw Error('Map floating atom changed');",
    );
}

#[test]
fn wasm_set_and_string_record_fields_settle_together() {
    run(
        r#"module test.wasm_set_record;
@id("set.carrier") record Carrier {@id("set.carrier.keys") keys:Set<string>,@id("set.carrier.label") label:string,}
@id("set.read") fn read(carrier:borrow Carrier)->i64 {if set_has<string>(carrier.keys,"key\u{0}") {string_len(carrier.label)}else{-1}}
@id("set.main") fn main()->i64 {let mut keys=set_new<string>(1usize);keys=set_insert<string>(keys,"key\u{0}");let carrier=Carrier {keys:keys,label:"ready"};read(carrier)}"#,
        "if(instance.exports.semaprax_main()!==5n)throw Error('Set and String record changed');",
    );
}

#[test]
fn wasm_owned_record_match_extracts_map_set_and_legacy_map_results() {
    // One collection remains live across the unpack helper; its discarded
    // String companion must settle before result publication. The one-entry
    // collection quota and twelve reentries detect a retained old owner.
    for (ty, setup, value) in [
        ("Map<i64,string>", "let mut values=map_new<i64,string>(1usize);values=map_set<i64,string>(values,2,\"kept\");", "string_len(map_get_or<i64,string>(kept,2,\"\"))"),
        ("Set<i64>", "let mut values=set_new<i64>(1usize);values=set_insert<i64>(values,4);", "set_key_at<i64>(kept,0usize)"),
        ("Map<string,i64>", "let mut values=map_new(1usize);values=map_set(values,\"kept\",4);", "map_get_or(kept,\"kept\",0)"),
    ] {
        let source=format!(r#"module test.wasm_map_unpack;
@id("carrier") record Carrier {{ @id("carrier.values") values:{ty}, @id("carrier.label") label:string, }}
@id("unpack") fn unpack(carrier:own Carrier)->{ty} {{ match own carrier {{ Carrier {{values:values,label:label}} => values, }} }}
@id("app.main") fn main()->i64 {{ {setup} let kept=unpack(Carrier {{values:values,label:"discarded"}}); {value} }}"#);
        run(&source,"if(instance.exports.semaprax_main()!==4n)throw Error('owned record collection result changed');");
    }
}
