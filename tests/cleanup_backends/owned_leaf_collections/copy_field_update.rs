//! Actual generated C11 O0/O2 stores, full construction through settlement.
#[path = "../../support/record_copy_field_update.rs"]
mod corpus;
use semaprax::{codegen, hir};
use std::process::Command;
use std::sync::atomic::{AtomicU64, Ordering};
static NEXT: AtomicU64 = AtomicU64::new(0);
fn run(source: &str, domain: &str, code: u32, expected: i64, stores: u32, refused: u32) {
    let ast = semaprax::check(source, "field-update-native.spx").unwrap();
    let resolved = hir::resolve(&ast).unwrap();
    hir::validate(&resolved).unwrap();
    let generated = codegen::emit_c(&ast).unwrap();
    let mut sites = 0;
    let observed = generated.lines().map(|line| {
        if !line.contains("spx_status = spx_leaf_update_field(") { return line.to_owned(); }
        sites += 1;
        let arguments=line.split("spx_leaf_update_field(").nth(1).unwrap().trim_end_matches(");").split(',').map(str::trim).collect::<Vec<_>>();
        assert_eq!(arguments.len(),8);
        let (owner,layout,index,field,result)=(arguments[1],arguments[2],arguments[3],arguments[4],arguments[7]);
        format!(r#"{{
 spx_vec_v1 before=*({owner});const spx_leaf_layout_v1 *desc={layout};
 uint64_t index={index},next=spx_ctx->vec_next_generation;uint32_t field={field};
 unsigned char prior[80]={{0}};size_t offset=desc->offsets[field];
 if(index<before.len)memcpy(prior,(unsigned char *)(void *)before.ptr+index*desc->stride,desc->stride);
 fixture_sort_active=1;
 {line}
 fixture_sort_active=0;
 if(spx_status==SPX_STATUS_SUCCESS){{
  ++fixture_stores;
  if(({result})->ptr!=before.ptr||({result})->authority!=before.authority||({result})->len!=before.len
   ||({result})->capacity!=before.capacity||({result})->generation!=next||spx_ctx->vec_next_generation!=next+1
   ||({owner})->generation!=0)abort();
  const unsigned char *after=(const unsigned char *)(const void *)({result})->ptr+index*desc->stride;
  if(memcmp(prior,after,offset)||memcmp(prior+offset+8,after+offset+8,desc->stride-offset-8))abort();
 }}else{{
  ++fixture_store_refusals;
  if(memcmp({owner},&before,sizeof(before))||spx_ctx->vec_next_generation!=next)abort();
  if(index<before.len&&memcmp(prior,(unsigned char *)(void *)before.ptr+index*desc->stride,desc->stride))abort();
 }}
}}"#)
    }).collect::<Vec<_>>().join("\n");
    assert!(sites > 0);
    let probe = format!(
        r#"
#undef malloc
#undef calloc
#undef realloc
#undef free
int main(void){{
 struct spx_context c={{0}};struct spx_status_entry entries[32];
 if(!spx_context_init(&c,UINT64_C(31),entries,32,NULL,NULL,NULL))return 1;
 for(unsigned run=0;run<3;++run){{
  fixture_stores=fixture_store_refusals=0;int64_t result=INT64_C(123456789);
  uint32_t count=c.status_arena.length;
  spx_status_token status=spx_decl_6f776e65642e6c6561662e6d61696e(&c,&result);
  if({code}==0){{if(status!=SPX_STATUS_SUCCESS||result!={expected}||c.status_arena.length!=count)return 2;}}
  else{{const struct spx_normalized_status *entry=spx_status_resolve(&c,status);
   if(status==SPX_STATUS_SUCCESS||result!=INT64_C(123456789)||!entry||entry->code!={code}
    ||strcmp(entry->domain_id,"{domain}")||c.status_arena.length!=count+1)return 3;}}
  if(fixture_live||fixture_stores!={stores}||fixture_store_refusals!={refused})return 4;
  for(uint32_t i=0;i<SPX_VEC_AUTHORITY_CAPACITY;++i)if(c.vec_authority[i].live)return 5;
 }}return 0;
}}
"#
    );
    let root = std::env::temp_dir().join(format!(
        "field-update-native-{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    ));
    std::fs::create_dir_all(&root).unwrap();
    let path = root.join("case.c");
    std::fs::write(
        &path,
        format!(
            "{}\nstatic unsigned fixture_stores,fixture_store_refusals;\n{observed}\n{probe}",
            include_str!("allocations.c")
        ),
    )
    .unwrap();
    for optimization in ["-O0", "-O2"] {
        let binary = root.join(format!(
            "case{optimization}{}",
            std::env::consts::EXE_SUFFIX
        ));
        let built = Command::new("clang")
            .args([
                "-std=c11",
                optimization,
                "-Wall",
                "-Wextra",
                "-Werror",
                "-DSPX_NO_ENTRY_WRAPPER",
            ])
            .arg(&path)
            .arg("-o")
            .arg(&binary)
            .output()
            .unwrap();
        assert!(
            built.status.success(),
            "{}",
            String::from_utf8_lossy(&built.stderr)
        );
        let output = Command::new(&binary).output().unwrap();
        assert!(
            output.status.success(),
            "{optimization}: {:?} {}",
            output.status.code(),
            String::from_utf8_lossy(&output.stderr)
        );
    }
    std::fs::remove_dir_all(root).unwrap();
}
#[test]
fn copy_field_updates_preserve_eight_scalar_lanes_owned_leaf_bits_and_zero_store_heap_at_o0_o2() {
    corpus::assert_sources();
    run(&corpus::evaluation_order_source(), "", 0, 29, 1, 0);
    for (ty, value, expected) in corpus::cases() {
        run(&corpus::source(ty, value, expected), "", 0, 29, 4, 0);
    }
    let legacy = corpus::source("i64", "-9", "got==-9")
        .replace("title:string", "title:Bytes")
        .replace("title:\"a\\u{0}é\"", "title:bytes_zeroed(4usize)")
        .replace("title:\"second\"", "title:bytes_zeroed(6usize)")
        .replace("str_len_bytes(title)==4", "byte_len(title)==4usize");
    run(&legacy, "", 0, 29, 4, 0);
    super::native::run(
        &corpus::oracle_source(),
        0,
        29,
        super::native::Failure::None,
    );
}
#[test]
fn copy_field_update_index_and_rhs_failures_select_once_before_commit_and_drop_all_native_owners() {
    run(&corpus::empty_failure(), "semaprax.vec.v1", 2, 0, 0, 1);
    for index in ["1usize", "18446744073709551615usize"] {
        run(
            &corpus::failure(index, "9", ""),
            "semaprax.vec.v1",
            2,
            0,
            0,
            1,
        );
    }
    let helpers = "@id(\"field.rhs\") fn rhs()->i64 {1/0}";
    run(
        &corpus::failure("1usize", "rhs()", helpers),
        "semaprax.arithmetic.v1",
        4,
        0,
        0,
        0,
    );
    let helpers = "@id(\"field.index\") fn index()->usize {1usize/0usize}";
    run(
        &corpus::failure("index()", "9", helpers),
        "semaprax.arithmetic.v1",
        4,
        0,
        0,
        0,
    );
}
