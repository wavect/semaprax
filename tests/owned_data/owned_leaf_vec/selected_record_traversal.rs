//! The full maximum fixture requires the explicit fused-source profile.
//! Keep its original row count, UTF-8/NUL oracle, read boundary and reentry.
use std::path::Path;
use std::process::Command;
use std::sync::atomic::Ordering;

const ACCOUNT: usize = 16 * 1024 * 1024;
const ALLOCATOR: &str = r#"#include <stdbool.h>
#include <stdint.h>
#include <stdlib.h>
static bool spx_test_read_started;
static uint64_t spx_test_live;
static void *spx_test_malloc(size_t n) {
 if(spx_test_read_started)abort();void *p=malloc(n);if(p)++spx_test_live;return p;
}
static void *spx_test_calloc(size_t n,size_t s) {
 if(spx_test_read_started)abort();void *p=calloc(n,s);if(p)++spx_test_live;return p;
}
static __attribute__((unused)) void *spx_test_realloc(void *p,size_t n) {
 if(spx_test_read_started)abort();void *r=realloc(p,n);if(r&&!p)++spx_test_live;return r;
}
static void spx_test_free(void *p) {
 if(p){if(!spx_test_live)abort();--spx_test_live;free(p);}
}
#define malloc spx_test_malloc
#define calloc spx_test_calloc
#define realloc spx_test_realloc
#define free spx_test_free
#define main spx_test_generated_main
"#;

pub(super) fn run(source: &str, expected: i64) {
    let account = semaprax::fused_source::RuntimeAccount::new(ACCOUNT).unwrap();
    let ordinary = semaprax::check(source, "maximum-ordinary.spx").unwrap_err();
    assert!(ordinary.iter().any(|error| error.code == "SPX-T267"));
    for _ in 0..4 {
        let report = semaprax::fused_source::run_interpreter(
            source,
            Path::new("maximum-selected.spx"),
            account,
            semaprax::interpreter::DEFAULT_MAX_STEPS,
        )
        .unwrap();
        assert!(report.returned());
        let value: serde_json::Value = serde_json::from_str(report.envelope()).unwrap();
        assert_eq!(value["result"], expected.to_string());
    }
    let directory = std::env::temp_dir().join(format!(
        "maximum-selected-{}-{}",
        std::process::id(),
        super::SEQUENCE.fetch_add(1, Ordering::Relaxed)
    ));
    std::fs::create_dir(&directory).unwrap();
    let generated =
        semaprax::fused_source::emit_native(source, Path::new("maximum-selected.spx"), account)
            .unwrap();
    let boundary = "    if (!out || !spx_fused_rows_valid(d,rows)) SPX_FUSED_FAIL_STOP(\"fused row read without authority\");";
    assert_eq!(generated.matches(boundary).count(), 1);
    let tracked = generated.replace(
        boundary,
        &format!("    spx_test_read_started=true;\n{boundary}"),
    );
    let driver = format!(
        r#"
#undef main
int main(void) {{
 spx_profile_account paid={{0}};
 if(!spx_profile_account_init(&paid,SPX_PROFILE_REQUIRED*4u))return 1;
 for(uint32_t i=0;i<4;++i){{
  struct spx_status_entry entries[32]={{0}};struct spx_context context={{0}};
  if(!spx_context_init(&context,17+i,entries,32,NULL,NULL,NULL))return 2;
  spx_test_read_started=false;
  int64_t result=INT64_C(252525);spx_status_token status=SPX_STATUS_SUCCESS;
  if(spx_profile_invoke(&paid,&context,&result,&status)!=SPX_FUSED_PUBLISHED)return 3;
  if(!spx_test_read_started||status!=SPX_STATUS_SUCCESS||result!=INT64_C({expected}))return 4;
  if(context.status_arena.length||spx_test_live||paid.issuer.active)return 5;
  if(paid.debited!=(i+1u)*SPX_PROFILE_REQUIRED)return 6;
  for(uint32_t j=0;j<SPX_VEC_AUTHORITY_CAPACITY;++j)if(context.vec_authority[j].live)return 7;
 }}return 0;
}}
"#
    );
    let c = directory.join("maximum.c");
    std::fs::write(&c, format!("{ALLOCATOR}\n{tracked}\n{driver}")).unwrap();
    for optimization in ["-O0", "-O2"] {
        let binary = directory.join(format!(
            "maximum{optimization}{}",
            std::env::consts::EXE_SUFFIX
        ));
        let built = Command::new(std::env::var_os("CLANG").unwrap_or_else(|| "clang".into()))
            .args(["-std=c11", optimization, "-Wall", "-Wextra", "-Werror"])
            .arg(&c)
            .arg("-o")
            .arg(&binary)
            .output()
            .unwrap();
        assert!(
            built.status.success(),
            "{}",
            String::from_utf8_lossy(&built.stderr)
        );
        let output = Command::new(binary).output().unwrap();
        assert!(
            output.status.success(),
            "{optimization}: {:?}: {}",
            output.status,
            String::from_utf8_lossy(&output.stderr)
        );
    }
    let package = semaprax::fused_source::emit_wasm_package(
        source,
        Path::new("maximum-selected.spx"),
        account,
    )
    .unwrap();
    let package_path = directory.join("package");
    package.publish(&package_path).unwrap();
    let script = directory.join("maximum.mjs");
    std::fs::write(
        &script,
        r#"
import assert from 'node:assert/strict';
import {pathToFileURL} from 'node:url';
import path from 'node:path';
const [directory, expected, limit] = process.argv.slice(2);
const entry = await import(pathToFileURL(path.join(directory, 'fused-construction-v1.mjs')).href);
const runner = await entry.install(Number(limit));
for(let index=0;index<4;++index){
 const result=runner.invoke();
 assert.equal(result.outcome,'returned');
 assert.equal(result.result,expected);
 assert.equal(result.source_sha256,entry.MANIFEST.source_sha256);
 assert.equal('status' in result,false);
}
"#,
    )
    .unwrap();
    let output = Command::new("node")
        .arg(script)
        .arg(package_path)
        .arg(expected.to_string())
        .arg(ACCOUNT.to_string())
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    std::fs::remove_dir_all(directory).unwrap();
}
