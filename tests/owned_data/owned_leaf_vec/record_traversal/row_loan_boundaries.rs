//! Additional OPT-775 acceptance definitions; execution remains pending.
use std::process::Command;
use std::sync::atomic::Ordering;

const DECLARATION: &str = r#"module app.loan_boundary;
@id("boundary.Row") record LoanRow {
 @id("boundary.Row.title") title:string,
 @id("boundary.Row.priority") priority:i64,
}
@id("boundary.weight") fn weight(row:borrow LoanRow)->i64 {row.priority}
"#;

#[test]
fn maximum_string_record_row_loans_preserve_order_and_nonallocating_reentry() {
    // One owned leaf and one scalar admit 8192 rows under the existing carrier
    // envelopes. No Bytes allocation in a loop or selected-profile widening.
    let source = format!(
        "{DECLARATION}{}",
        r#"
@id("app.main") fn main()->i64 {
 let mut items=vec_with_capacity<LoanRow>(8192usize);
 let mut next=0;
 while next<8192 {
  items=vec_push<LoanRow>(items,LoanRow{title:"é\u{0}",priority:next});
  next=next+1;
  0
 }
 let rows=items;
 let mut total=0; let mut count=0; let mut good=true;
 for borrow row at index in rows {
  let value=weight(row);
  good=good && value==i64_from_usize(index) && str_len_bytes(row.title)==3;
  total=total+value; count=count+1;
  0
 }
 let renewed=vec_clear<LoanRow>(rows);
 if good && count==8192 && vec_len<LoanRow>(renewed)==0usize {total}else{-1}
}"#
    );
    let expected: i64 = (0..8192).sum();
    assert_eq!(expected, 33_550_336);
    let path = std::env::temp_dir().join(format!(
        "row-loan-maximum-{}-{}.spx",
        std::process::id(),
        super::super::SEQUENCE.fetch_add(1, Ordering::Relaxed),
    ));
    std::fs::write(&path, &source).unwrap();
    for _ in 0..4 {
        let report =
            semaprax::interpreter::interpret(&path, "app.main", &[], &Default::default()).unwrap();
        semaprax::interpreter::verify_envelope(&report.envelope).unwrap();
        assert!(report.returned);
        let wire: serde_json::Value = serde_json::from_str(&report.envelope).unwrap();
        assert_eq!(wire["payload"]["outcome"]["value"], expected.to_string());
    }
    std::fs::remove_file(path).unwrap();
    super::super::super::scoped_vec_field_native::run(&source, 0, expected);
    // The existing strict private host checks zero copies during every read,
    // exact values and complete owner settlement; it adds no public admission.
    super::super::run_wasm(
        source.strip_prefix("module app.loan_boundary;\n").unwrap(),
        0,
        expected,
        "none",
    );
}

const THREE_OWNERS: &str = r#"
@id("app.main") fn main()->i64 {
 let first=vec_push<LoanRow>(vec_with_capacity<LoanRow>(1usize),LoanRow{title:"a",priority:7});
 let second=vec_push<LoanRow>(vec_with_capacity<LoanRow>(1usize),LoanRow{title:"b",priority:11});
 let third=vec_push<LoanRow>(vec_with_capacity<LoanRow>(1usize),LoanRow{title:"c",priority:13});
 let mut total=0;
 for borrow row in first {total=total+weight(row);0}
 for borrow row in second {total=total+weight(row);0}
 for borrow row in third {total=total+weight(row);0}
 total
}"#;

#[test]
fn each_fallible_storage_frontier_preserves_status_drop_and_same_context_reentry() {
    let source = format!("{DECLARATION}{THREE_OWNERS}");
    let parsed = semaprax::check(&source, "row-loan-storage-faults.spx").unwrap();
    let generated = semaprax::codegen::emit_c(&parsed).unwrap();
    let storage = "spx_leaf_storage_v1 *storage = calloc(1, bytes);";
    assert_eq!(generated.matches(storage).count(), 1);
    assert!(generated.contains("spx_row_loan_v1"));
    let read = "    (void)spx_leaf_check(c, s, d);\n    if (!r || field >= d->count)";
    assert_eq!(generated.matches(read).count(), 1);
    let tracked=generated
        .replace(storage,"spx_leaf_storage_v1 *storage = spx_fault_storage(1, bytes);")
        .replace(read,"    spx_fault_read = true;\n    (void)spx_leaf_check(c, s, d);\n    if (!r || field >= d->count)")
        .replace("malloc(","spx_fault_malloc(")
        .replace("calloc(","spx_fault_calloc(")
        .replace("realloc(","spx_fault_realloc(")
        .replace("#define SPX_VEC_REALLOC realloc","#define SPX_VEC_REALLOC spx_fault_realloc")
        .replace("free(","spx_fault_free(");
    let allocator = r#"#include <stdbool.h>
#include <stdint.h>
#include <stdlib.h>
static uint64_t spx_fault_live,spx_fault_attempt,spx_fault_cut;
static bool spx_fault_read;
static __attribute__((unused)) void *spx_fault_malloc(size_t n){
 if(spx_fault_read)abort();void*p=malloc(n);if(p)++spx_fault_live;return p;
}
static __attribute__((unused)) void *spx_fault_calloc(size_t n,size_t s){
 if(spx_fault_read)abort();void*p=calloc(n,s);if(p)++spx_fault_live;return p;
}
static __attribute__((unused)) void *spx_fault_realloc(void*p,size_t n){
 if(spx_fault_read)abort();void*r=realloc(p,n);if(r&&!p)++spx_fault_live;return r;
}
static void *spx_fault_storage(size_t n,size_t s){
 ++spx_fault_attempt;if(spx_fault_attempt==spx_fault_cut)return NULL;
 return spx_fault_calloc(n,s);
}
static __attribute__((unused)) void spx_fault_free(void*p){
 if(p){if(!spx_fault_live)abort();--spx_fault_live;free(p);}
}
"#;
    // Sweep EVERY fallible storage allocation reached by this exact caller:
    // three constructors. Push commits allocate no native storage. Payload
    // allocation fail-stop is intentionally not relabeled as a Vec refusal.
    let probe = r#"
int main(void){
 struct spx_status_entry entries[32];struct spx_context context={0};
 if(!spx_context_init(&context,17,entries,32,NULL,NULL,NULL))return 1;
 for(uint64_t cut=0;cut<=3;++cut)for(uint32_t repeat=0;repeat<4;++repeat){
  spx_fault_cut=cut;spx_fault_attempt=0;spx_fault_read=false;
  uint32_t before=context.status_arena.length;int64_t result=INT64_C(252525);
  spx_status_token status=spx_decl_6170702e6d61696e(&context,&result);
  if(!cut){
   if(status!=SPX_STATUS_SUCCESS||result!=31||!spx_fault_read||spx_fault_attempt!=3||context.status_arena.length!=before)return 2;
  }else{
   const struct spx_normalized_status *entry=spx_status_resolve(&context,status);
   if(status==SPX_STATUS_SUCCESS||result!=INT64_C(252525)||spx_fault_read||spx_fault_attempt!=cut||!entry||entry->code!=3||strcmp(entry->domain_id,"semaprax.vec.v1")||context.status_arena.length!=before+1)return 3;
  }
  if(spx_fault_live)return 4;
  for(uint32_t j=0;j<SPX_VEC_AUTHORITY_CAPACITY;++j)if(context.vec_authority[j].live)return 5;
 }return 0;
}
"#;
    for optimization in ["-O0", "-O2"] {
        let base = std::env::temp_dir().join(format!(
            "row-loan-fault-{}-{}",
            std::process::id(),
            super::super::SEQUENCE.fetch_add(1, Ordering::Relaxed)
        ));
        let c = base.with_extension("c");
        let binary = base.with_extension(std::env::consts::EXE_EXTENSION);
        std::fs::write(&c, format!("{allocator}\n{tracked}\n{probe}")).unwrap();
        let built = Command::new("clang")
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
            built.status.success(),
            "{}",
            String::from_utf8_lossy(&built.stderr)
        );
        let ran = Command::new(&binary).status().unwrap();
        let _ = std::fs::remove_file(c);
        let _ = std::fs::remove_file(binary);
        assert!(ran.success(), "{optimization}: {ran}");
    }
}

#[test]
fn each_private_wasm_push_refusal_settles_all_preceding_loan_owners() {
    let source = format!(
        "{}{}",
        DECLARATION
            .strip_prefix("module app.loan_boundary;\n")
            .unwrap(),
        THREE_OWNERS
    );
    // Existing host ordinal modes preserve consumed/retained owner accounting;
    // each of this caller's three push boundaries is exercised separately.
    super::super::run_wasm(&source, 0, 31, "none");
    for mode in ["codec-push-1", "codec-push-2", "codec-push-3"] {
        super::super::run_wasm(&source, 15, 0, mode);
    }
}
