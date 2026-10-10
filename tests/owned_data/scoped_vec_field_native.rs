//! Native read-only row witnesses; allocator aborts from the first field read onward.
use std::process::Command;
use std::sync::atomic::{AtomicU64, Ordering};
static SEQUENCE: AtomicU64 = AtomicU64::new(0);

fn run(source: &str, expected_code: u32, expected_value: i64) {
    let program = semaprax::check(source, "scoped-vec-field-native.spx").unwrap();
    let resolved = semaprax::hir::resolve(&program).unwrap();
    let generated = semaprax::codegen::emit_hir_c(&resolved).unwrap();
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
@id("app.marker") fn marker(rows:borrow Vec<Row>)->i64 {vec_field<Row>(rows,0usize,"marker")}
@id("app.main") fn main()->i64 {
 let empty=vec_with_capacity<Row>(1usize);
 let rows=vec_push<Row>(empty,Row{title:"é\u{0}",payload:bytes_zeroed(3usize),marker:-17});
 let mut total:i64=0; let mut index:usize=0usize;
 while index<5000usize {
  let text=str_as_bytes(vec_field<Row>(rows,0usize,"title"));
  let bytes=vec_field<Row>(rows,0usize,"payload");
  if byte_len(text)==3usize && byte_len(bytes)==3usize && str_len_bytes(vec_field<Row>(rows,0usize,"title"))==3 && marker(rows)==-17 {total=total+1;}
  index=index+1usize;
  0
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

#[test]
fn legacy_field_reads_reject_substituted_descriptors_and_stale_generations() {
    let source = r#"module test.scoped_descriptor;
@id("row.a") record A {
 @id("row.a.left") left:Bytes,@id("row.a.right") right:Bytes,@id("row.a.marker") marker:i64,
}
@id("row.b") record B {
 @id("row.b.left") left:Bytes,@id("row.b.right") right:Bytes,@id("row.b.marker") marker:i64,
}
@id("app.other") fn other(rows:borrow Vec<B>)->i64 {vec_field<B>(rows,0usize,"marker")}
@id("app.main") fn main()->i64 {
 let rows=vec_push<A>(vec_with_capacity<A>(1usize),A{left:bytes_zeroed(0usize),right:bytes_zeroed(0usize),marker:-17});
 vec_field<A>(rows,0usize,"marker")
}
"#;
    let checked = semaprax::check(source, "scoped-descriptor.spx").unwrap();
    let resolved = semaprax::hir::resolve(&checked).unwrap();
    let generated = semaprax::codegen::emit_hir_c(&resolved).unwrap();
    assert!(generated.contains("spx_leaf_legacy_new(spx_ctx, &spx_record_726f772e61_leaf_v1"));
    let old_source = source
        .replace("vec_field<A>(rows,0usize,\"marker\")", "1")
        .replace("vec_field<B>(rows,0usize,\"marker\")", "1");
    let old = semaprax::codegen::emit_hir_c(
        &semaprax::hir::resolve(&semaprax::check(&old_source, "unscoped-descriptor.spx").unwrap())
            .unwrap(),
    )
    .unwrap();
    assert!(!old.contains("owned_leaf_layout"));
    assert!(!old.contains("spx_leaf_legacy_new"));
    assert!(old.contains("spx_vec_record_with_capacity(spx_ctx,"));
    // Turn only the two expected fail-stop reasons into observable exit codes.
    // A different failure, early successful return or silent repair cannot pass.
    let mismatch = "spx_runtime_invariant_failure(\"legacy owned record descriptor mismatch\");";
    let stale =
        "spx_runtime_invariant_failure(\"stale or forged owned bounded Vec Bytes carrier\");";
    assert_eq!(generated.matches(mismatch).count(), 1);
    assert_eq!(generated.matches(stale).count(), 1);
    let tracked = generated
        .replace(mismatch, "exit(73);")
        .replace(stale, "exit(74);");
    let probe = r#"
int main(int argc,char **argv) {
 if(argc!=2)return 1;
 struct spx_status_entry entries[8]; struct spx_context context={0};
 if(!spx_context_init(&context,17,entries,8,NULL,NULL,NULL))return 2;
 spx_vec_v1 empty={0},rows={0},cleared={0};
 const spx_leaf_layout_v1 *a=&spx_record_726f772e61_leaf_v1;
 const spx_leaf_layout_v1 *b=&spx_record_726f772e62_leaf_v1;
 int mode=atoi(argv[1]);
 if(mode==4){if(spx_vec_record_with_capacity(&context,1,&empty)!=SPX_STATUS_SUCCESS)return 3;}
 else if(spx_leaf_legacy_new(&context,a,1,&empty)!=SPX_STATUS_SUCCESS)return 4;
 spx_vec_record_v1 row={0};row.spx_scalar=(uint64_t)INT64_C(-17);
 if(spx_vec_record_push(&context,&empty,&row,&rows)!=SPX_STATUS_SUCCESS)return 5;
 const unsigned char *selected=NULL;
 if(mode==3){
  spx_vec_v1 stale=rows;
  if(spx_vec_record_clear(&context,&rows,&cleared)!=SPX_STATUS_SUCCESS)return 6;
  (void)spx_leaf_read_field(&context,&stale,a,0,2,&selected);return 7;
 }
 spx_leaf_layout_v1 copy=*a;
 const spx_leaf_layout_v1 *requested=mode==1?b:mode==2?&copy:a;
 uint64_t generation=rows.generation;uint32_t authority=rows.authority;
 if(spx_leaf_read_field(&context,&rows,requested,0,2,&selected)!=SPX_STATUS_SUCCESS)return 8;
 uint64_t bits=0;memcpy(&bits,selected,sizeof(bits));
 if(mode!=0||bits!=(uint64_t)INT64_C(-17)||rows.generation!=generation
    ||rows.authority!=authority||context.vec_authority[authority-1].owned_leaf_layout!=a)return 9;
 spx_vec_drop(&context,&rows);
 if(context.vec_authority[authority-1].live||context.vec_authority[authority-1].owned_leaf_layout!=NULL)return 10;
 return 0;
}
"#;
    for optimization in ["-O0", "-O2"] {
        let base = std::env::temp_dir().join(format!(
            "spx-field-descriptor-{}-{}",
            std::process::id(),
            SEQUENCE.fetch_add(1, Ordering::Relaxed)
        ));
        let c = base.with_extension("c");
        let binary = base.with_extension(std::env::consts::EXE_EXTENSION);
        std::fs::write(&c, format!("{tracked}\n{probe}")).unwrap();
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
        for (mode, expected) in [(0, 0), (1, 73), (2, 73), (3, 74), (4, 73)] {
            let result = Command::new(&binary)
                .arg(mode.to_string())
                .status()
                .unwrap();
            assert_eq!(result.code(), Some(expected), "{optimization}: mode {mode}");
        }
        let _ = std::fs::remove_file(c);
        let _ = std::fs::remove_file(binary);
    }
}
