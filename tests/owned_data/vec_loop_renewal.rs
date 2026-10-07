//! Ordinary Vec updates retain their exact cleanup position among untouched owners.
use semaprax::{codegen, graph, hir, interpreter, wasm};
use std::process::Command;

const SOURCE: &str = r#"module test.vec_loop_renewal;
@id("app.main") fn main()->i64 {
 let mut values=vec_with_capacity<i64>(3usize);
 let mut untouched=vec_with_capacity<i64>(1usize);
 let mut i=0;
 while i<3 {
  if i!=1 { values=vec_push<i64>(values,i); 0 } else { 0 }
  i=i+1;
  0
 }
 let mut j=0;
 while j<2 {
  values=vec_set<i64>(values,0usize,7);
  values=vec_reserve_exact<i64>(values,2usize);
  j=j+1;
  0
 }
 let observed=vec_get<i64>(values,0usize)==7 && vec_len<i64>(values)==2usize;
 while false { values=vec_clear<i64>(values); 0 }
 if observed && vec_len<i64>(untouched)==0usize {7}else{0}
}
"#;

fn failure(operation: &str) -> String {
    format!(
        r#"module test.vec_loop_renewal_failure;
@id("app.main") fn main()->i64 {{
 let mut values=vec_push<i64>(vec_with_capacity<i64>(1usize),1);
 let mut untouched=vec_with_capacity<i64>(1usize);
 let mut i=0;
 while i<2 {{ values={operation}; i=i+1; 0 }}
 if vec_len<i64>(untouched)==0usize {{7}}else{{0}}
}}
"#
    )
}

#[test]
fn ordinary_vec_renewal_graph_and_source_refusal_are_exact() {
    let checked = semaprax::check(SOURCE, "vec-loop-renewal.spx").unwrap();
    let canonical = semaprax::format::canonical(&checked);
    let roundtrip = semaprax::check(&canonical, "vec-loop-renewal-roundtrip.spx").unwrap();
    assert_eq!(semaprax::format::canonical(&roundtrip), canonical);
    let resolved = hir::resolve(&checked).unwrap();
    assert_eq!(
        resolved.functions[0].cleanup_plan.schema,
        "semaprax.cleanup-plan.v15"
    );
    let json = graph::to_json(&checked).unwrap();
    graph::verify_json(&checked, &json).unwrap();
    let doc: serde_json::Value = serde_json::from_str(&json).unwrap();
    assert_eq!(doc["schema"], "semaprax.graph.v66");
    assert_eq!(
        graph::reject_evidence_schema("semaprax.graph.v66")
            .unwrap_err()
            .code,
        "SPX-G410"
    );
    assert_eq!(
        doc["vec_loop_renewal"]["schema"],
        "semaprax.vec-loop-renewal.v1"
    );
    assert_eq!(
        doc["vec_loop_renewal"]["updates"].as_array().unwrap().len(),
        4
    );
    assert!(graph::verify_json(
        &checked,
        &json.replace("semaprax.graph.v66", "semaprax.graph.v65")
    )
    .is_err());
    let composed = SOURCE.replace("@id(\"app.main\")",r#"@id("legacy.main") fn legacy()->i64 {
 let input=vec_push<i64>(vec_with_capacity<i64>(1usize),1);
 let mut output=vec_with_capacity<i64>(1usize);
 for own item in vec_into_iter<i64>(input) { if item>0 {output=vec_push<i64>(output,item);0}else{0} }
 if vec_len<i64>(output)==1usize {1}else{0}
}
@id("app.main")"#);
    let composed = semaprax::check(&composed, "vec-loop-renewal-composed.spx").unwrap();
    let resolved = hir::resolve(&composed).unwrap();
    assert_eq!(
        resolved
            .functions
            .iter()
            .find(|f| f.id.as_str() == "legacy.main")
            .unwrap()
            .cleanup_plan
            .schema,
        "semaprax.cleanup-plan.v12"
    );
    let json = graph::to_json(&composed).unwrap();
    graph::verify_json(&composed, &json).unwrap();
    assert!(
        json.contains("semaprax.cleanup-plan.v12") && json.contains("semaprax.cleanup-plan.v15")
    );
    let other_owner = SOURCE.replace("vec_push<i64>(values,i)", "vec_push<i64>(untouched,i)");
    let errors = semaprax::check(&other_owner, "vec-loop-renewal-other-owner.spx").unwrap_err();
    assert!(
        errors.iter().any(|error| error.code == "SPX-U105"),
        "{errors:?}"
    );
    let immutable = SOURCE.replace("let mut values=", "let values=");
    let errors = semaprax::check(&immutable, "vec-loop-renewal-immutable.spx").unwrap_err();
    assert!(
        errors.iter().any(|error| error.code == "SPX-U101"),
        "{errors:?}"
    );
}

#[test]
fn ordinary_vec_renewal_settles_untouched_owners_on_every_engine() {
    let both = SOURCE.replace("i=i+1;", "untouched=vec_clear<i64>(untouched); i=i+1;");
    let clear = SOURCE.replace("while false", "while j<3").replace(
        "values=vec_clear<i64>(values); 0",
        "values=vec_clear<i64>(values); j=j+1; 0",
    );
    for (index, source, code) in [
        (0, SOURCE.to_owned(), 0),
        (1, both, 0),
        (2, clear, 0),
        (3, failure("vec_push<i64>(values,i)"), 1),
        (4, failure("vec_set<i64>(values,1usize,i)"), 2),
        (5, failure("vec_reserve_exact<i64>(values,8192usize)"), 3),
    ] {
        let root = std::env::temp_dir().join(format!(
            "semaprax-vec-loop-renewal-{}-{index}",
            std::process::id()
        ));
        std::fs::create_dir_all(&root).unwrap();
        let path = root.join("program.spx");
        std::fs::write(&path, &source).unwrap();
        let checked = semaprax::check(&source, &path).unwrap();
        for _ in 0..4 {
            let result = interpreter::interpret(
                &path,
                "app.main",
                &[],
                &interpreter::InterpreterOptions::default(),
            )
            .unwrap();
            let envelope: serde_json::Value = serde_json::from_str(&result.envelope).unwrap();
            if code == 0 {
                assert!(result.returned);
                assert_eq!(envelope["payload"]["outcome"]["value"], "7");
            } else {
                assert!(!result.returned);
                assert_eq!(
                    envelope["payload"]["outcome"]["status"]["domain_id"],
                    "semaprax.vec.v1"
                );
                assert_eq!(envelope["payload"]["outcome"]["status"]["code"], code);
            }
        }
        native(&checked, &root, code);
        core_wasm(&checked, &root, code);
        std::fs::remove_dir_all(root).unwrap();
    }
}

fn native(checked: &semaprax::ast::Program, root: &std::path::Path, code: u32) {
    let generated = codegen::emit_c(checked)
        .unwrap()
        .replace("calloc(", "spx_test_calloc(")
        .replace(
            "#define SPX_VEC_REALLOC realloc",
            "#define SPX_VEC_REALLOC spx_test_realloc",
        )
        .replace("free(", "spx_test_free(");
    let allocator = r#"#include <stdint.h>
#include <stdlib.h>
static uint64_t live=0;
static void *spx_test_calloc(size_t n,size_t s){void*p=calloc(n,s);if(p)++live;return p;}
static __attribute__((unused)) void *spx_test_realloc(void*p,size_t n){void*r=realloc(p,n);if(r&&!p)++live;return r;}
static void spx_test_free(void*p){if(p){if(!live)abort();--live;free(p);}}
"#;
    let probe = format!(
        r#"
int main(void){{
 struct spx_status_entry entries[32]; struct spx_context context={{0}};
 if(!spx_context_init(&context,17,entries,32,NULL,NULL,NULL))return 1;
 for(int i=0;i<4;++i){{
  int64_t result=99; spx_status_token token=spx_decl_6170702e6d61696e(&context,&result);
  if({code}==0){{if(token!=SPX_STATUS_SUCCESS||result!=7)return 2;}}
  else{{const struct spx_normalized_status*s=spx_status_resolve(&context,token);if(token==SPX_STATUS_SUCCESS||result!=99||!s||s->code!={code}||strcmp(s->domain_id,"semaprax.vec.v1"))return 3;}}
  if(live)return 4;
  for(uint32_t j=0;j<SPX_VEC_AUTHORITY_CAPACITY;++j)if(context.vec_authority[j].live)return 5;
 }}return 0;
}}
"#
    );
    let path = root.join("program.c");
    std::fs::write(&path, format!("{allocator}\n{generated}\n{probe}")).unwrap();
    for opt in ["-O0", "-O2"] {
        let binary = root.join(format!("native-{opt}"));
        let result = Command::new("clang")
            .args([
                "-std=c11",
                "-Wall",
                "-Wextra",
                "-Werror",
                "-DSPX_NO_ENTRY_WRAPPER",
                opt,
            ])
            .arg(&path)
            .arg("-o")
            .arg(&binary)
            .output()
            .unwrap();
        assert!(
            result.status.success(),
            "{}",
            String::from_utf8_lossy(&result.stderr)
        );
        let result = Command::new(&binary).output().unwrap();
        assert!(result.status.success(), "{opt}: {result:?}");
    }
}

fn core_wasm(checked: &semaprax::ast::Program, root: &std::path::Path, code: u32) {
    let path = root.join("program.wasm");
    std::fs::write(&path, wasm::emit_module(checked).unwrap()).unwrap();
    let script = r#"
const fs=require('fs'),bytes=fs.readFileSync(process.argv[1]),expected=Number(process.argv[2]);
let next=1n;const entries=new Map(),key=v=>v.toString();
const read=(v,t)=>{const e=entries.get(key(v));if(!e||e.tag!==t)throw Error('carrier');return e};
const alloc=(tag,capacity,values=[])=>{const token=next++;entries.set(key(token),{tag,capacity,values});return token};
const env={spx_add:(a,b)=>a+b,spx_sub:(a,b)=>a-b,spx_mul:(a,b)=>a*b,spx_div:(a,b)=>a/b,spx_rem:(a,b)=>a%b,spx_neg:a=>-a,
spx_contract_fail:selector=>{const code=selector-12;if(selector<13||selector>15)throw Error('selector');throw Object.assign(Error('vec failure'),{domain_id:'semaprax.vec.v1',code})},
spx_vec_with_capacity:(tag,c)=>{const n=Number(c);return n<=8192?alloc(tag,n):0n},
spx_vec_push:(v,t,b)=>{const e=read(v,t);if(e.values.length>=e.capacity)return 0n;entries.delete(key(v));return alloc(t,e.capacity,e.values.concat([b]))},
spx_vec_len:(v,t)=>BigInt(read(v,t).values.length),spx_vec_capacity:(v,t)=>BigInt(read(v,t).capacity),
spx_vec_get:(v,t,i)=>{const e=read(v,t),n=Number(i);if(n<0||n>=e.values.length)throw Error('get');return e.values[n]},
spx_vec_drop:v=>{if(!entries.delete(key(v)))throw Error('drop')}};
env.spx_vec_reserve_exact=(v,t,a)=>{const e=read(v,t),n=Number(a),capacity=Math.max(e.capacity,e.values.length+n);if(!Number.isSafeInteger(n)||n<0||capacity>8192)return 0n;entries.delete(key(v));return alloc(t,capacity,e.values.slice())};
env.spx_vec_set=(v,t,i,b)=>{const e=read(v,t),n=Number(i);if(!Number.isSafeInteger(n)||n<0||n>=e.values.length)return 0n;const values=e.values.slice();values[n]=b;entries.delete(key(v));return alloc(t,e.capacity,values)};
env.spx_vec_clear=(v,t)=>{const e=read(v,t);entries.delete(key(v));return alloc(t,e.capacity,[])};
WebAssembly.instantiate(bytes,{env}).then(({instance})=>{for(let i=0;i<4;i+=1){let failed=false;try{const value=instance.exports.semaprax_main();if(expected!==0||value!==7n)throw Error('value')}catch(error){if(expected===0||error.domain_id!=='semaprax.vec.v1'||error.code!==expected)throw error;failed=true}if(failed!==(expected!==0)||entries.size!==0)throw Error(`settlement:${entries.size}`)}}).catch(error=>{console.error(error);process.exit(2)});
"#;
    let result = Command::new("node")
        .arg("-e")
        .arg(script)
        .arg(&path)
        .arg(code.to_string())
        .output()
        .unwrap();
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
}
