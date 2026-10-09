//! #724 executable schema-derived identifier/array views, not trusted spans.

use super::*;

pub(super) const SCHEMA: &str = r#"module consumer.schema;
@id("application.patient") record Patient {
 @id("application.patient.id") id:string,
 @id("application.patient.arrival") arrival:i64,
 @id("application.patient.service") service:i64,
 @id("application.patient.priority") priority:i64,
 @id("application.patient.deadline") deadline:i64,
}
@id("application.request") record Request {
 @id("application.request.servers") servers:Vec<string>,
 @id("application.request.patients") patients:Vec<Patient>,
}
@id("consumer.schema.anchor") fn schema_anchor()->i64 {0}
"#;

fn app() -> String {
    let imports = r#"module consumer.app;
use type @id("application.request.json.request-result") from consumer.schema as RequestJsonRequestDecode;
use type @id("application.patient.json.view-encode") from consumer.schema as PatientJsonViewEncode;
use type @id("application.patient.json.view") from consumer.schema as PatientJsonView;
use type @id("application.request.json.identifier-span") from consumer.schema as ServerSpan;
use function @id("application.request.json.request.decode") from consumer.schema as decode;
use function @id("application.request.json.request.encode") from consumer.schema as encode;
use function @id("application.patient.json.view.encode") from consumer.schema as encode_view;
"#;
    let valid=br#"{"patients":[{"deadline":8,"priority":1,"service":5,"arrival":0,"id":"\u0050\u0031"}],"servers":["\u0053\u0031"]}"#;
    let expected=br#"{"servers":["S1"],"patients":[{"id":"P1","arrival":0,"service":5,"priority":1,"deadline":8}]}"#;
    let mut out =
        imports.to_owned() + "@id(\"consumer.main\") fn main()->i64 {\nlet mut failures=0;\n";
    let errors: Vec<(&[u8], i64, usize, i64)> = vec![
        (b"{", 1, 1, 0),
        (b"{}", 3, 2, 1),
        (br#"{"servers":[],"servers":[]}"#, 2, 14, 1),
        (br#"{"extra":0}"#, 4, 1, 0),
        (br#"{"servers":true}"#, 5, 11, 1),
        (br#"{"servers":[""]}"#, 6, 12, 1),
        (br#"{"servers":["A","\u0041"]}"#, 10, 16, 1),
        (br#"{"servers":["not allowed"]}"#, 6, 12, 1),
    ];
    for (i, (input, code, offset, field)) in errors.iter().enumerate() {
        out.push_str(&format!("let bad_{i}={};let outcome_{i}=decode(array_as_slice(bad_{i}));failures=failures+match own outcome_{i}{{RequestJsonRequestDecode::Decoded{{servers,patients}}=>1,RequestJsonRequestDecode::Error{{code:actual,offset:at,field:target}}=>if actual=={code} && at=={offset}usize && target=={field}{{0}}else{{1}},}};\n",array(input)));
    }
    out.push_str(&format!("let input={};let source=array_as_slice(input);let outcome=decode(source);\nfailures=failures+match own outcome{{RequestJsonRequestDecode::Error{{code,offset,field}}=>1,RequestJsonRequestDecode::Decoded{{servers,patients}}=>{{\nlet count=vec_len<PatientJsonView>(patients);let first=vec_get<PatientJsonView>(patients,0usize);let good=count==1usize && first.arrival==0 && first.service==5 && first.priority==1 && first.deadline==8;\nlet forged=PatientJsonView{{id_start:0usize,id_end:1usize,arrival:0,service:5,priority:1,deadline:8}};let invalid=encode_view(source,forged,131072usize);\nlet refused=match own invalid{{PatientJsonViewEncode::Refused{{required}}=>required==18446744073709551615usize,PatientJsonViewEncode::Encoded{{text}}=>false,}};\nlet repeated=vec_with_capacity<PatientJsonView>(2usize);let one=vec_push<PatientJsonView>(repeated,first);let two=vec_push<PatientJsonView>(one,first);let duplicate=encode(source,servers,two,131072usize);let duplicate_ok=match own duplicate{{PatientJsonViewEncode::Refused{{required}}=>required==18446744073709551615usize,PatientJsonViewEncode::Encoded{{text}}=>false,}};
let empty=vec_with_capacity<ServerSpan>(8usize);let missing=encode(source,empty,patients,131072usize);let missing_ok=match own missing{{PatientJsonViewEncode::Refused{{required}}=>required==18446744073709551615usize,PatientJsonViewEncode::Encoded{{text}}=>false,}};
let exact=encode(source,servers,patients,{}usize);let short=encode(source,servers,patients,{}usize);\nlet short_ok=match own short{{PatientJsonViewEncode::Refused{{required}}=>required=={}usize,PatientJsonViewEncode::Encoded{{text}}=>false,}};\nlet exact_ok=match own exact{{PatientJsonViewEncode::Refused{{required}}=>false,PatientJsonViewEncode::Encoded{{text}}=>{{let view=string_as_str(text);let expected={};let mut index=0usize;let mut same=usize_from_i64(str_len_bytes(view))==byte_len(array_as_slice(expected));while same && index<byte_len(array_as_slice(expected)){{let actual=str_byte_at(view,index);let wanted=byte_get(array_as_slice(expected),index);same=match actual{{Option::None{{}}=>false,Option::Some{{value:a}}=>match wanted{{Option::None{{}}=>false,Option::Some{{value:b}}=>a==b,}},}};index=index+1usize;same && index<byte_len(array_as_slice(expected))}}same}},}};if good && refused && duplicate_ok && missing_ok && short_ok && exact_ok{{0}}else{{1}}\n}},}};if failures==0{{725}}else{{0-failures}}\n}}",array(valid),expected.len(),expected.len()-1,expected.len(),array(expected)));
    out
}

#[test]
fn request_identifier_views_revalidate_spans_and_encode_exact_arrays_on_three_backends() {
    let root = fixture("json-request-views", SCHEMA);
    let source = project::with_authenticated_project(&root.join("semaprax.toml"), |snapshot| {
        let revision = snapshot.retain_revision();
        let source = project::derive_json_codec_source_with_profile(
            &revision,
            "src/schema.spx",
            "application.request",
            project::JsonCodecProfile::RequestViews,
        )?;
        project::verify_json_codec_source_with_profile(
            &revision,
            "src/schema.spx",
            "application.request",
            &source,
            project::JsonCodecProfile::RequestViews,
        )?;
        assert_eq!(canonical(&source), source);
        let changed = source.replace("16usize", "17usize");
        assert_ne!(changed, source);
        assert!(project::verify_json_codec_source_with_profile(
            &revision,
            "src/schema.spx",
            "application.request",
            &changed,
            project::JsonCodecProfile::RequestViews
        )
        .is_err());
        Ok(source)
    })
    .unwrap();
    std::fs::write(root.join("src/schema.spx"), source).unwrap();
    std::fs::write(root.join("src/app.spx"), canonical(&app())).unwrap();
    project::with_authenticated_project(&root.join("semaprax.toml"),|snapshot|{
        let graph=snapshot.retain_revision();
        assert!(graph.semantic_graph().contains("application.request.json.request.decode"));
        assert!(graph.semantic_graph().contains("application.patient.json.view"));
        assert!(graph.semantic_graph().contains("core.vec.push"));
        semaprax::hir::validate(snapshot.entry_program()).map_err(|error|vec![error])?;
        assert_eq!(snapshot.execute_entry(&project::ProjectExecutionOptions::default())?.outcome(),&project::ProjectExecutionOutcome::Returned(725));
        let c=codegen::emit_hir_c(snapshot.entry_program()).map_err(|error|vec![error])?;
        for optimization in ["-O0","-O2"] { super::super::compile_and_run_c(&c,&root,optimization,"725"); }
        let bytes=wasm::emit_resolved_module(snapshot.entry_program()).map_err(|error|vec![error])?;
        let digest=format!("{:x}",semaprax::digest_hex::LowerHex(Sha256::digest(&bytes)));
        std::fs::write(root.join("app.wasm"),bytes).unwrap();
        let runtime=include_str!("../../../../src/wasm/browser_runtime.js").replace("__SEMAPRAX_OWNED_EXPORTS__","{}").replace("__SEMAPRAX_WASM_SHA256__",&digest);
        std::fs::write(root.join("runtime.mjs"),runtime).unwrap();
        std::fs::write(root.join("run.mjs"),"import {readFile} from 'node:fs/promises';import {instantiateBytes} from './runtime.mjs';const {instance}=await instantiateBytes(await readFile('./app.wasm'),{maxOwnedByteEntries:16});for(let i=0;i<16;i++){if(instance.exports.semaprax_main()!==725n)throw Error('request codec parity');}").unwrap();
        let output=Command::new("node").arg("run.mjs").current_dir(&root).output().unwrap();
        assert!(output.status.success(),"{}",String::from_utf8_lossy(&output.stderr));
        Ok(())
    }).unwrap();
    std::fs::remove_dir_all(root).unwrap();
}
