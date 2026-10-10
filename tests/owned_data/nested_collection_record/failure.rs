use super::*;
use std::process::Command;
const PREFIX: &str = r#"module failure.nested_collection;
@id("metrics") record Metrics { @id("metrics.total") total:i64, }
@id("report") record Report { @id("report.words") words:Vec<string>, @id("report.metrics") metrics:Metrics, }
@id("envelope") record Envelope {
 @id("envelope.text") text:string,
 @id("envelope.report") report:Report,
 @id("envelope.bytes") bytes:Bytes,
 @id("envelope.tail") tail:i64,
}
@id("boom") fn boom()->i64 {9223372036854775807+1}
@id("text") fn text()->string {let ignored=boom();"unpublished"}
@id("report.fail") fn report_fail()->Report {let ignored=boom();Report{words:vec_with_capacity<string>(0usize),metrics:Metrics{total:0}}}
@id("bytes") fn bytes()->Bytes {let ignored=boom();bytes_zeroed(0usize)}
@id("make") fn make()->Report {
 let words=vec_push<string>(vec_with_capacity<string>(1usize),"owned");
 Report{words:words,metrics:Metrics{total:1}}
}
@id("consume") fn consume(value:own Report, marker:i64)->i64 {boom()}
@id("guarded") fn guarded(value:own Report)->Report ensures false {value}
"#;

#[test]
fn every_nested_constructor_prefix_and_owned_call_failure_preserves_status_and_settles() {
    for (label, body) in [
        (
            "first",
            "let v=Envelope{text:text(),report:make(),bytes:bytes_zeroed(2usize),tail:1};0",
        ),
        (
            "report",
            "let v=Envelope{text:\"owned\",report:report_fail(),bytes:bytes_zeroed(2usize),tail:1};0",
        ),
        (
            "bytes",
            "let v=Envelope{text:\"owned\",report:make(),bytes:bytes(),tail:1};0",
        ),
        (
            "tail",
            "let v=Envelope{text:\"owned\",report:make(),bytes:bytes_zeroed(2usize),tail:boom()};0",
        ),
        ("staging", "let r=make();consume(r,boom())"),
        ("callee", "let r=make();consume(r,1)"),
        ("postcondition", "let r=make();let unpublished=guarded(r);0"),
    ] {
        let contract = label == "postcondition";
        let domain = if contract {
            "semaprax.contract.v1"
        } else {
            "semaprax.arithmetic.v1"
        };
        let code = if contract { 2 } else { 1 };
        let root = directory(label);
        let path = root.join("app.spx");
        let source = format!("{PREFIX}@id(\"app.main\") fn main()->i64 {{{body}}}");
        std::fs::write(&path, &source).unwrap();
        let ast = semaprax::check(&source, &path).unwrap();
        for _ in 0..3 {
            let observed = interpreter::internal_strings::interpret(
                &path,
                "app.main",
                &[],
                &InterpreterOptions::default(),
            )
            .unwrap();
            assert!(!observed.returned, "{}", observed.envelope);
            interpreter::internal_strings::verify_envelope(&observed.envelope).unwrap();
            assert!(
                observed
                    .envelope
                    .contains(&format!("\"domain_id\":\"{domain}\""))
            );
            assert!(observed.envelope.contains(&format!("\"code\":{code}")));
        }
        native_failure(&ast, &root, domain, code);
        let wasm = semaprax::wasm::emit_module(&ast).unwrap();
        wasmparser::Validator::new().validate_all(&wasm).unwrap();
        let module = root.join("app.wasm");
        std::fs::write(&module, wasm).unwrap();
        let host = root.join("owned-leaf-host.js");
        std::fs::write(&host, include_str!("../owned_leaf_vec/host.js")).unwrap();
        let output = Command::new("node")
            .arg(host)
            .arg(module)
            .args([if contract { "10" } else { "1" }, "0", "prefix-failure"])
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{label}: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        std::fs::remove_dir_all(root).unwrap();
    }
}

pub(crate) fn native_failure(
    ast: &semaprax::ast::Program,
    root: &std::path::Path,
    domain: &str,
    code: u32,
) {
    let generated = semaprax::codegen::emit_c(ast)
        .unwrap()
        .replace("malloc(", "probe_malloc(")
        .replace("calloc(", "probe_calloc(")
        .replace("realloc(", "probe_realloc(")
        .replace(
            "#define SPX_VEC_REALLOC realloc",
            "#define SPX_VEC_REALLOC probe_realloc",
        )
        .replace("free(", "probe_free(");
    let allocator = r#"#include <stdint.h>
#include <stdlib.h>
#include <string.h>
static uint64_t live=0;
static void *probe_malloc(size_t n){void*p=malloc(n);if(p)++live;return p;}
static void *probe_calloc(size_t n,size_t s){void*p=calloc(n,s);if(p)++live;return p;}
static void *probe_realloc(void*p,size_t n){void*r=realloc(p,n);if(r&&!p)++live;return r;}
static void probe_free(void*p){if(p){if(!live)abort();--live;free(p);}}
"#;
    let probe = r#"
int main(void){
 struct spx_status_entry entries[32];struct spx_context context={0};
 if(!spx_context_init(&context,17,entries,32,NULL,NULL,NULL))return 1;
 for(unsigned repeat=0;repeat<3;++repeat){
  int64_t result=INT64_C(123456789);
  spx_status_token token=spx_decl_6170702e6d61696e(&context,&result);
  if(token==SPX_STATUS_SUCCESS||result!=INT64_C(123456789))return 2;
  const struct spx_normalized_status *status=spx_status_resolve(&context,token);
  if(!status||strcmp(status->domain_id,"semaprax.arithmetic.v1")!=0||status->code!=UINT32_C(1))return 3;
  if(live)return 4;
  for(uint32_t j=0;j<SPX_VEC_AUTHORITY_CAPACITY;++j)if(context.vec_authority[j].live)return 5;
 }
 return 0;
}
"#;
    let probe = probe.replace("semaprax.arithmetic.v1", domain).replace(
        "status->code!=UINT32_C(1)",
        &format!("status->code!=UINT32_C({code})"),
    );
    let c = root.join("failure.c");
    std::fs::write(&c, format!("{allocator}\n{generated}\n{probe}")).unwrap();
    for optimization in ["-O0", "-O2"] {
        let binary = root.join(format!("failure-{optimization}"));
        let output = Command::new("clang")
            .args([
                "-std=c11",
                "-Wall",
                "-Wextra",
                "-Werror",
                "-DSPX_NO_ENTRY_WRAPPER",
                optimization,
            ])
            .arg(&c)
            .arg("-o")
            .arg(&binary)
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        let output = Command::new(binary).output().unwrap();
        assert!(
            output.status.success(),
            "{:?}: {}",
            output.status.code(),
            String::from_utf8_lossy(&output.stderr)
        );
    }
}
