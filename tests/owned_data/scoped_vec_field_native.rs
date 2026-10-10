//! Native read-only row witnesses; allocator aborts from the first field read onward.
use std::process::Command;
use std::sync::atomic::{AtomicU64, Ordering};
static SEQUENCE: AtomicU64 = AtomicU64::new(0);

fn run(source: &str, expected_code: u32, expected_value: i64) {
    let program = semaprax::check(source, "scoped-vec-field-native.spx").unwrap();
    let generated = semaprax::codegen::emit_hir_c(&program).unwrap();
    assert!(generated.contains("spx_leaf_read_field(spx_ctx, &"));
    let boundary = "    (void)spx_leaf_check(c, s, d);\n    if (!r || field >= d->count)";
    assert_eq!(generated.matches(boundary).count(), 1);
    let tracked = generated
        .replace(boundary, "    spx_test_read_started = true;\n    (void)spx_leaf_check(c, s, d);\n    if (!r || field >= d->count)")
        .replace("malloc(", "spx_test_malloc(")
        .replace("calloc(", "spx_test_calloc(")
        .replace("realloc(", "spx_test_realloc(")
        .replace("#define SPX_VEC_REALLOC realloc", "#define SPX_VEC_REALLOC spx_test_realloc")
        .replace("free(", "spx_test_free(");
    let allocator = r#"#include <stdbool.h>
#include <stdint.h>
#include <stdlib.h>
static bool spx_test_read_started = false;
static uint64_t spx_test_live = 0;
static __attribute__((unused)) void *spx_test_malloc(size_t n) {
 if (spx_test_read_started) abort(); void *p=malloc(n); if(p)++spx_test_live; return p;
}
static __attribute__((unused)) void *spx_test_calloc(size_t n,size_t s) {
 if (spx_test_read_started) abort(); void *p=calloc(n,s); if(p)++spx_test_live; return p;
}
static __attribute__((unused)) void *spx_test_realloc(void *p,size_t n) {
 if (spx_test_read_started) abort(); void *r=realloc(p,n); if(r&&!p)++spx_test_live; return r;
}
static __attribute__((unused)) void spx_test_free(void *p) {
 if(p){if(!spx_test_live)abort();--spx_test_live;free(p);}
}
"#;
    let probe = format!(
        r#"
int main(void) {{
 struct spx_status_entry entries[32]; struct spx_context context={{0}};
 if(!spx_context_init(&context,17,entries,32,NULL,NULL,NULL))return 1;
 for(uint32_t i=0;i<4;++i){{
  spx_test_read_started=false;
  int64_t result=INT64_C(252525); uint32_t before=context.status_arena.length;
  spx_status_token status=spx_decl_6170702e6d61696e(&context,&result);
  if(!spx_test_read_started)return 2;
  if({expected_code}==0){{
   if(status!=SPX_STATUS_SUCCESS||result!=INT64_C({expected_value})||context.status_arena.length!=before)return 3;
  }}else{{
   const struct spx_normalized_status *entry=spx_status_resolve(&context,status);
   if(status==SPX_STATUS_SUCCESS||result!=INT64_C(252525)||!entry||entry->code!={expected_code}||strcmp(entry->domain_id,"semaprax.vec.v1")||context.status_arena.length!=before+1)return 4;
  }}
  if(spx_test_live)return 5;
  for(uint32_t j=0;j<SPX_VEC_AUTHORITY_CAPACITY;++j)if(context.vec_authority[j].live)return 6;
 }}return 0;
}}
"#
    );
    for optimization in ["-O0", "-O2"] {
        let base = std::env::temp_dir().join(format!(
            "spx-vec-field-{}-{}",
            std::process::id(),
            SEQUENCE.fetch_add(1, Ordering::Relaxed)
        ));
        let c = base.with_extension("c");
        let binary = base.with_extension(std::env::consts::EXE_EXTENSION);
        std::fs::write(&c, format!("{allocator}\n{tracked}\n{probe}")).unwrap();
        let output = Command::new("clang")
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
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        let result = Command::new(&binary).status().unwrap();
        let _ = std::fs::remove_file(c);
        let _ = std::fs::remove_file(binary);
        assert!(result.success(), "{optimization}: {result}");
    }
}

#[test]
fn repeated_scalar_and_utf8_byte_reads_never_materialize_owners() {
    run(
        r#"module test.scoped_native;
@id("row") record Row {
 @id("row.title") title:string, @id("row.payload") payload:Bytes, @id("row.marker") marker:i64,
}
@id("app.main") fn main()->i64 {
 let empty=vec_with_capacity<Row>(1usize);
 let rows=vec_push<Row>(empty,Row{title:"é\u{0}",payload:bytes_zeroed(3usize),marker:-17});
 let mut total:i64=0; let mut index:usize=0usize;
 while index<5000usize {
  let text=str_as_bytes(vec_field<Row>(rows,0usize,"title"));
  let bytes=vec_field<Row>(rows,0usize,"payload");
  if byte_len(text)==3usize && byte_len(bytes)==3usize && str_len_bytes(vec_field<Row>(rows,0usize,"title"))==3 && vec_field<Row>(rows,0usize,"marker")==-17 {total=total+1;}
  index=index+1usize;
 }
 total
}
"#,
        0,
        5000,
    );
}

#[test]
fn legacy_two_bytes_carrier_reads_preserve_generation_and_cleanup() {
    run(
        r#"module test.scoped_legacy;
@id("row") record Row {
 @id("row.id") id:Bytes,@id("row.payload") payload:Bytes,@id("row.marker") marker:i64,
}
@id("app.main") fn main()->i64 {
 let empty=vec_with_capacity<Row>(1usize);
 let rows=vec_push<Row>(empty,Row{id:bytes_zeroed(2usize),payload:bytes_zeroed(5usize),marker:-23});
 let bytes=vec_field<Row>(rows,0usize,"payload");
 if byte_len(bytes)==5usize && vec_field<Row>(rows,0usize,"marker")==-23 {-23}else{0}
}
"#,
        0,
        -23,
    );
}

#[test]
fn out_of_bounds_field_read_keeps_selected_status_and_settles_source() {
    run(
        r#"module test.scoped_bounds;
@id("row") record Row {@id("row.title") title:string,@id("row.marker") marker:i64,}
@id("app.main") fn main()->i64 {
 let empty=vec_with_capacity<Row>(1usize);
 let rows=vec_push<Row>(empty,Row{title:"live",marker:7});
 vec_field<Row>(rows,1usize,"marker")
}
"#,
        2,
        0,
    );
}
