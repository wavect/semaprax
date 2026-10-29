//! Exact Frame requires-false failure publication and fresh-call controls.
use semaprax::{codegen, diagnostic::Diagnostic, hir::ResolvedProgram, wasm};
use std::{path::Path, process::Command};

pub(super) fn check(program: &ResolvedProgram, root: &Path) -> Result<(), Vec<Diagnostic>> {
    let generated = codegen::emit_hir_c(program)
        .map_err(|error| vec![error])?
        .replace("malloc(", "probe_malloc(")
        .replace("calloc(", "probe_calloc(")
        .replace("realloc(", "probe_realloc(")
        .replace("free(", "probe_free(");
    let allocator = r#"#include <stdint.h>
#include <stdlib.h>
#include <string.h>
static uint64_t live=0,allocations=0;
static __attribute__((unused)) void *probe_malloc(size_t n){void*p=malloc(n);if(p){++live;++allocations;}return p;}
static __attribute__((unused)) void *probe_calloc(size_t n,size_t s){void*p=calloc(n,s);if(p){++live;++allocations;}return p;}
static __attribute__((unused)) void *probe_realloc(void*p,size_t n){void*r=realloc(p,n);if(r&&!p){++live;++allocations;}return r;}
static __attribute__((unused)) void probe_free(void*p){if(p){if(!live)abort();--live;free(p);}}
"#;
    let symbol = program
        .entrypoint
        .as_str()
        .as_bytes()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect::<String>();
    let probe = r#"
int main(void){
 struct spx_status_entry entries[32];struct spx_context context={0};
 if(!spx_context_init(&context,17,entries,32,NULL,NULL,NULL))return 1;
 for(unsigned repeat=0;repeat<3;++repeat){
  int64_t result=INT64_C(123456789);
  spx_status_token token=spx_decl_SYMBOL(&context,&result);
  if(token==SPX_STATUS_SUCCESS||result!=INT64_C(123456789))return 2;
  const struct spx_normalized_status *status=spx_status_resolve(&context,token);
  if(!status||strcmp(status->domain_id,"semaprax.contract.v1")!=0||status->code!=UINT32_C(1))return 3;
  if(live||allocations)return 4;
 }
 return 0;
}
"#.replace("SYMBOL",&symbol);
    let c = root.join("frame-refusal.c");
    std::fs::write(&c, format!("{allocator}\n{generated}\n{probe}")).unwrap();
    for optimization in ["-O0", "-O2"] {
        let binary = root.join(format!("frame-refusal-{optimization}"));
        let compiled = Command::new("clang")
            .args([
                "-std=c11",
                optimization,
                "-Wall",
                "-Wextra",
                "-Werror",
                "-DSPX_NO_ENTRY_WRAPPER",
            ])
            .arg(&c)
            .arg("-o")
            .arg(&binary)
            .output()
            .unwrap();
        assert!(
            compiled.status.success(),
            "{}",
            String::from_utf8_lossy(&compiled.stderr)
        );
        let output = Command::new(binary).output().unwrap();
        assert!(
            output.status.success(),
            "{:?}: {}",
            output.status.code(),
            String::from_utf8_lossy(&output.stderr)
        );
    }
    let bytes = wasm::emit_resolved_module(program).map_err(|error| vec![error])?;
    std::fs::write(root.join("frame-refusal.wasm"), bytes).unwrap();
    let original = super::super::wasm_host::wasm_conformance_js("frame-refusal.wasm", 0, 0, None);
    let mut runner = original.replace(
        "const bytes = await readFile",
        "const selected = new Error('requires false');\nconst bytes = await readFile",
    );
    runner = runner.replace(
        "spx_contract_fail:()=>{throw new Error();}",
        "spx_contract_fail:selector=>{assert.equal(selector,9);throw selected;}",
    );
    assert_ne!(runner, original);
    let success = "assert.equal(linked.instance.exports.semaprax_main(), 0n);";
    assert!(runner.contains(success));
    runner=runner.replace(success,"let result=123456789n;assert.throws(()=>{result=linked.instance.exports.semaprax_main();},error=>error===selected);assert.equal(result,123456789n);");
    assert!(runner.contains("spx_contract_fail:selector"));
    let script = root.join("frame-refusal.mjs");
    std::fs::write(&script, runner).unwrap();
    // Pass the bare name like every sibling runner: `root` is canonical, and a
    // Windows verbatim `\\?\C:\` entry path makes Node's realpath fail on `C:`.
    let output = Command::new("node")
        .arg(script.file_name().unwrap())
        .current_dir(root)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    Ok(())
}
