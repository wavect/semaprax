//! Borrowed Record-Field Uniqueness v1 (#786): one source through the
//! interpreter, native C11 O0/O2 and the strict private Core-Wasm host. The
//! saved clone/sort helper is an independent oracle, not a producer.
use super::{DECLARATION, HOST, SEQUENCE};
use semaprax::{codegen, interpreter, wasm};
use std::process::Command;
use std::sync::atomic::Ordering;

/// The prior ShiftSim helper shape, adapted to `Row.title`: cloned owned keys,
/// consuming `vec_sort_owned` and a neighbor scan.
const CLONE_SORT_ORACLE: &str = r#"@id("app.oracle.titles")
fn oracle_titles(values: borrow Vec<Row>) -> bool
{
    let length = vec_len<Row>(values);
    let mut identifiers = vec_with_capacity<string>(length);
    let mut index = 0usize;
    while index < length {
        let title_view = vec_field<Row>(values, index, "title");
        identifiers = vec_push<string>(identifiers, string_from_str(title_view));
        index = index + 1usize;
        0
    }
    let sorted = vec_sort_owned<string>(identifiers);
    let count = vec_len<string>(sorted);
    let mut at = 1usize;
    let mut valid = true;
    while valid && at < count {
        let previous = vec_str_at(sorted, at - 1usize);
        let current = vec_str_at(sorted, at);
        valid = previous != current;
        at = at + 1usize;
        valid && at < count
    }
    valid
}
"#;

const ORIGINAL_MAIN: &str = r#"@id("app.main") fn main()->i64 {
 let empty=vec_with_capacity<Row>(0usize);
 let empty_ok=vec_unique_by<Row>(empty,"title") && vec_unique_by<Row>(empty,"priority") && oracle_titles(empty);
 let single0=vec_with_capacity<Row>(1usize);
 let single=vec_push<Row>(single0,Row{priority:7,title:"a\u{0}\u{e9}",payload:bytes_zeroed(0usize)});
 let single_ok=vec_unique_by<Row>(single,"title") && oracle_titles(single);
 let mut rows=vec_with_capacity<Row>(7usize);
 rows=vec_push<Row>(rows,Row{priority:-9223372036854775807-1,title:"a",payload:bytes_zeroed(1usize)});
 rows=vec_push<Row>(rows,Row{priority:9223372036854775807,title:"a\u{0}",payload:bytes_zeroed(1usize)});
 rows=vec_push<Row>(rows,Row{priority:0,title:"\u{e9}",payload:bytes_zeroed(0usize)});
 rows=vec_push<Row>(rows,Row{priority:-1,title:"e\u{301}",payload:bytes_zeroed(0usize)});
 rows=vec_push<Row>(rows,Row{priority:1,title:"ab",payload:bytes_zeroed(2usize)});
 rows=vec_push<Row>(rows,Row{priority:2,title:"a\u{0}b",payload:bytes_zeroed(3usize)});
 let distinct=vec_unique_by<Row>(rows,"title") && vec_unique_by<Row>(rows,"priority") && oracle_titles(rows);
 rows=vec_push<Row>(rows,Row{priority:3,title:"a",payload:bytes_zeroed(4usize)});
 let duplicate=!vec_unique_by<Row>(rows,"title") && vec_unique_by<Row>(rows,"priority") && !oracle_titles(rows);
 let order=vec_field<Row>(rows,0usize,"priority")==-9223372036854775807-1 && vec_field<Row>(rows,6usize,"priority")==3 && vec_len<Row>(rows)==7usize;
 let ranked=vec_sort_by<Row>(rows,"priority:desc");
 let renewed=vec_field<Row>(ranked,0usize,"priority")==9223372036854775807 && !vec_unique_by<Row>(ranked,"title");
 let mut wide=vec_with_capacity<Row>(4096usize);
 let mut index=0usize;
 while index<4096usize {
  wide=vec_push<Row>(wide,Row{priority:i64_from_usize(index),title:string_format("T{}",index),payload:bytes_zeroed(0usize)});
  index=index+1usize;
  0
 }
 let full=vec_unique_by<Row>(wide,"title") && vec_unique_by<Row>(wide,"priority") && vec_len<Row>(wide)==vec_capacity<Row>(wide);
 let mut late=vec_with_capacity<Row>(4096usize);
 index=0usize;
 while index<4096usize {
  let title=if index==4095usize {"T0"}else{string_format("T{}",index)};
  late=vec_push<Row>(late,Row{priority:i64_from_usize(index % 4095usize),title:title,payload:bytes_zeroed(0usize)});
  index=index+1usize;
  0
 }
 let last=!vec_unique_by<Row>(late,"title") && !vec_unique_by<Row>(late,"priority");
 if empty_ok && single_ok && distinct && duplicate && order && renewed && full && last {42}else{0}
}
"#;

// The large corpus retains two owned leaves and one i64 field (capacity 4096).
// Its unselected empty payload uses String: ordinary Bytes construction is
// closed in loops and counts against the unchanged 32-allocation envelope.
// Small Row cases retain their Bytes payloads, including nonempty values.
const LARGE_ROW: &str = r#"@id("app.large.row") record LargeRow {
 @id("app.large.row.priority") priority:i64,
 @id("app.large.row.title") title:string,
 @id("app.large.row.payload") payload:string,
}
"#;

fn admitted_source() -> String {
    let large_oracle = CLONE_SORT_ORACLE
        .replace("app.oracle.titles", "app.oracle.large_titles")
        .replace("oracle_titles", "oracle_large_titles")
        .replace("Vec<Row>", "Vec<LargeRow>")
        .replace("<Row>", "<LargeRow>");
    let (small, large) = ORIGINAL_MAIN.split_once(" let mut wide=").unwrap();
    let large = large
        .replace("<Row>", "<LargeRow>")
        .replace("Row{", "LargeRow{")
        .replace("payload:bytes_zeroed(0usize)", "payload:\"\"")
        .replace(
            "vec_len<LargeRow>(wide)==vec_capacity<LargeRow>(wide)",
            r#"vec_len<LargeRow>(wide)==vec_capacity<LargeRow>(wide) && oracle_large_titles(wide) && vec_field<LargeRow>(wide,0usize,"priority")==0 && vec_field<LargeRow>(wide,4095usize,"priority")==4095 && vec_field<LargeRow>(wide,0usize,"payload")==str"""#,
        )
        .replace(
            r#"!vec_unique_by<LargeRow>(late,"priority")"#,
            r#"!vec_unique_by<LargeRow>(late,"priority") && !oracle_large_titles(late) && vec_len<LargeRow>(late)==4096usize && vec_capacity<LargeRow>(late)==4096usize && vec_field<LargeRow>(late,0usize,"priority")==0 && vec_field<LargeRow>(late,4095usize,"priority")==0 && vec_field<LargeRow>(late,4095usize,"title")==str"T0" && vec_field<LargeRow>(late,4095usize,"payload")==str"""#,
        );
    format!("{DECLARATION}{LARGE_ROW}{CLONE_SORT_ORACLE}{large_oracle}{small} let mut wide={large}")
}

const COPY_MAIN: &str = r#"@id("app.copy.row") record CopyRow {
 @id("app.copy.row.rank") rank:i32, @id("app.copy.row.small") small:u8,
 @id("app.copy.row.mark") mark:char, @id("app.copy.row.flag") flag:bool,
}
@id("app.main") fn main()->i64 {
 let mut rows=vec_with_capacity<CopyRow>(3usize);
 rows=vec_push<CopyRow>(rows,CopyRow{rank:-2147483647i32-1i32,small:255u8,mark:'a',flag:true});
 rows=vec_push<CopyRow>(rows,CopyRow{rank:2147483647i32,small:0u8,mark:'é',flag:false});
 rows=vec_push<CopyRow>(rows,CopyRow{rank:-1i32,small:254u8,mark:'a',flag:true});
 let distinct=vec_unique_by<CopyRow>(rows,"rank") && vec_unique_by<CopyRow>(rows,"small");
 let duplicates=!vec_unique_by<CopyRow>(rows,"mark") && !vec_unique_by<CopyRow>(rows,"flag");
 if distinct && duplicates && vec_len<CopyRow>(rows)==3usize {42}else{0}
}
"#;

const ONE_CALL: &str = r#"@id("app.main") fn main()->i64 {
 let mut rows=vec_with_capacity<Row>(2usize);
 rows=vec_push<Row>(rows,Row{priority:1,title:"x",payload:bytes_zeroed(0usize)});
 rows=vec_push<Row>(rows,Row{priority:2,title:"y",payload:bytes_zeroed(0usize)});
 if vec_unique_by<Row>(rows,"title") {42}else{0}
}
"#;

fn root() -> std::path::PathBuf {
    let root = std::env::temp_dir().join(format!(
        "field-unique-{}-{}",
        std::process::id(),
        SEQUENCE.fetch_add(1, Ordering::Relaxed)
    ));
    std::fs::create_dir_all(&root).unwrap();
    root
}

fn interpret(root: &std::path::Path, source: &str) {
    let path = root.join("app.spx");
    std::fs::write(&path, source).unwrap();
    for _ in 0..3 {
        let result = interpreter::interpret(&path, "app.main", &[], &Default::default()).unwrap();
        interpreter::verify_envelope(&result.envelope).unwrap();
        assert!(result.returned, "{}", result.envelope);
        let wire: serde_json::Value = serde_json::from_str(&result.envelope).unwrap();
        assert_eq!(wire["payload"]["outcome"]["value"], "42");
    }
}

fn host(
    root: &std::path::Path,
    ast: &semaprax::ast::Program,
    status: &str,
    value: &str,
    mode: &str,
) {
    let bytes = wasm::emit_module(ast).expect("Core Wasm emission");
    wasmparser::Validator::new()
        .validate_all(&bytes)
        .expect("strict Core Wasm validation");
    let mut names = Vec::new();
    for payload in wasmparser::Parser::new(0).parse_all(&bytes) {
        if let wasmparser::Payload::ImportSection(section) = payload.unwrap() {
            for import in section.into_imports() {
                names.push(import.unwrap().name.to_owned());
            }
        }
    }
    assert_eq!(
        names
            .iter()
            .filter(|n| *n == "spx_vec_unique_by_v1")
            .count(),
        1
    );
    let module = root.join(format!("app-{mode}.wasm"));
    std::fs::write(&module, bytes).unwrap();
    let output = Command::new("node")
        .arg(HOST)
        .arg(&module)
        .args([status, value, mode])
        .output()
        .expect("Node must execute the strict private host");
    assert!(
        output.status.success(),
        "{mode}: {}",
        String::from_utf8_lossy(&output.stderr)
    );
}

/// Native probe: only the uniqueness scratch request is redirected, so the
/// requested bytes are observed exactly and may be refused; every other
/// allocation stays on the counted ordinary allocator.
fn native(root: &std::path::Path, ast: &semaprax::ast::Program, refuse: bool, scratch: &[u64]) {
    let generated = codegen::emit_c(ast).unwrap();
    let anchor = "uint32_t *o = malloc(";
    assert_eq!(generated.matches(anchor).count(), 1);
    let generated = generated
        .replace(anchor, "uint32_t *o = probe_unique_malloc(")
        .replace("malloc(", "probe_malloc(")
        .replace("probe_unique_probe_malloc(", "probe_unique_malloc(")
        .replace("calloc(", "probe_calloc(")
        .replace("realloc(", "probe_realloc(")
        .replace(
            "#define SPX_VEC_REALLOC realloc",
            "#define SPX_VEC_REALLOC probe_realloc",
        )
        .replace("free(", "probe_free(");
    let expected = scratch
        .iter()
        .map(|bytes| format!("UINT64_C({bytes})"))
        .collect::<Vec<_>>()
        .join(",");
    let requested = if refuse {
        scratch[0]
    } else {
        scratch.iter().sum()
    };
    let peak = if refuse {
        0
    } else {
        scratch.iter().copied().max().unwrap_or(0)
    };
    let allocator = format!(
        r#"#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
static uint64_t live=0,unique_requests=0,unique_bytes_mismatch=0,unique_refuse={refuse};
static uint64_t unique_requested_bytes=0,unique_peak_bytes=0;
static void *unique_live=NULL;
static const uint64_t unique_expected[]={{{expected}0}};
static void *probe_malloc(size_t n){{void*p=malloc(n);if(p)++live;return p;}}
static void *probe_calloc(size_t n,size_t s){{void*p=calloc(n,s);if(p)++live;return p;}}
static __attribute__((unused)) void *probe_realloc(void*p,size_t n){{
 if(p&&p==unique_live)abort();
 void*r=realloc(p,n);if(r&&!p)++live;return r;
}}
static void probe_free(void*p){{
 if(p){{if(!live)abort();if(p==unique_live)unique_live=NULL;--live;free(p);}}
}}
static void *probe_unique_malloc(size_t n){{
 if(unique_live)abort();
 if(unique_requests>={count}u||unique_expected[unique_requests]!=(uint64_t)n)++unique_bytes_mismatch;
 ++unique_requests;
 unique_requested_bytes+=(uint64_t)n;
 if(unique_refuse)return NULL;
 unique_live=probe_malloc(n);
 if(unique_live&&(uint64_t)n>unique_peak_bytes)unique_peak_bytes=(uint64_t)n;
 return unique_live;
}}
"#,
        refuse = u8::from(refuse),
        expected = if expected.is_empty() {
            String::new()
        } else {
            format!("{expected},")
        },
        count = scratch.len(),
    );
    let probe = format!(
        r#"
int main(void){{
 struct spx_status_entry entries[32];struct spx_context context={{0}};
 if(!spx_context_init(&context,17,entries,32,NULL,NULL,NULL))return 1;
 for(unsigned repeat=0;repeat<3;++repeat){{
  int64_t result=INT64_C(123456789);
  unique_requests=0;unique_requested_bytes=0;unique_peak_bytes=0;
  spx_status_token token=spx_decl_6170702e6d61696e(&context,&result);
  if({refuse}){{
   const struct spx_normalized_status *status=spx_status_resolve(&context,token);
   if(token==SPX_STATUS_SUCCESS||result!=INT64_C(123456789)||!status
      ||strcmp(status->domain_id,"semaprax.vec.v1")!=0||status->code!=UINT32_C(3))return 2;
   if(unique_requests!=1u)return 3;
  }}else if(token!=SPX_STATUS_SUCCESS||result!=INT64_C(42)||unique_requests!={count}u)return 4;
  if(unique_bytes_mismatch||live||unique_live)return 5;
  for(uint32_t j=0;j<SPX_VEC_AUTHORITY_CAPACITY;++j)if(context.vec_authority[j].live)return 6;
  printf("%llu %llu\n",(unsigned long long)unique_requested_bytes,
         (unsigned long long)unique_peak_bytes);
 }}
 return 0;
}}
"#,
        refuse = u8::from(refuse),
        count = scratch.len(),
    );
    let c = root.join(if refuse { "refuse.c" } else { "unique.c" });
    std::fs::write(&c, format!("{allocator}\n{generated}\n{probe}")).unwrap();
    for optimization in ["-O0", "-O2"] {
        let binary = root.join(format!("unique-{refuse}-{optimization}"));
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
            .expect("Clang must compile the uniqueness probe");
        assert!(
            output.status.success(),
            "{optimization}: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        let output = Command::new(&binary).output().unwrap();
        assert!(
            output.status.success(),
            "{optimization}: {:?}: {}",
            output.status.code(),
            String::from_utf8_lossy(&output.stderr)
        );
        let observed = String::from_utf8(output.stdout).unwrap();
        assert_eq!(observed, format!("{requested} {peak}\n").repeat(3));
        eprintln!(
            "{optimization} uniqueness scratch requested and peak live bytes per invocation:\n{observed}"
        );
    }
}

#[test]
fn original_full_bytes_loop_fixture_retains_complete_stable_diagnostics() {
    // Keep the exact formerly positive source: moving these allocations into
    // helpers or changing requirements must never grant loop admission.
    let source = format!("{DECLARATION}{CLONE_SORT_ORACLE}{ORIGINAL_MAIN}");
    let diagnostics = semaprax::check(&source, "field-unique.spx")
        .expect_err("ordinary Bytes allocation in while stays closed");
    assert_eq!(
        format!("{diagnostics:?}"),
        r#"[Diagnostic { code: "SPX-T252", severity: Error, message: "byte operation `bytes_zeroed` is not admitted in while bodies; only exact byte_len, byte_get and byte_range reads and the loop-carried bytes_set/bytes_set5/bytes_set1_or5_from_slice/bytes_set1_or6_or48_from_slice fills qualify", path: Some("field-unique.spx"), span: Some(Span { start: 2832, end: 2852, line: 54, column: 103 }), help: None }, Diagnostic { code: "SPX-T252", severity: Error, message: "byte operation `bytes_zeroed` is not admitted in while bodies; only exact byte_len, byte_get and byte_range reads and the loop-carried bytes_set/bytes_set5/bytes_set1_or5_from_slice/bytes_set1_or6_or48_from_slice fills qualify", path: Some("field-unique.spx"), span: Some(Span { start: 3269, end: 3289, line: 63, column: 94 }), help: None }, Diagnostic { code: "SPX-T267", severity: Error, message: "bytes_copy is reachable from a while condition or body", path: Some("field-unique.spx"), span: Some(Span { start: 1013, end: 3507, line: 32, column: 17 }), help: None }]"#,
    );
}

#[test]
fn same_source_uniqueness_matches_clone_sort_oracle_and_preserves_owner_on_every_engine() {
    let source = admitted_source();
    let ast = semaprax::check(&source, "field-unique.spx").expect("checked uniqueness source");
    let root = root();
    interpret(&root, &source);
    // Requested scratch per call, independently computed as 4 * len for every
    // call with at least two rows, in authored evaluation order. Empty and
    // single-row calls request nothing.
    let mut scratch = vec![4 * 6, 4 * 6, 4 * 7, 4 * 7, 4 * 7];
    scratch.extend([4 * 4096; 4]);
    native(&root, &ast, false, &scratch);
    host(&root, &ast, "0", "42", "none");
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn copy_record_integer_char_and_bool_keys_use_exact_values() {
    let source = format!("{DECLARATION}{COPY_MAIN}");
    let ast = semaprax::check(&source, "copy-unique.spx").expect("checked Copy uniqueness");
    let root = root();
    interpret(&root, &source);
    native(&root, &ast, false, &[4 * 3; 4]);
    // Copy rows cross the exact legacy scalar Vec boundary; the strict codec
    // mode supplies it, refuses any owned-leaf import, proves the capacity
    // import is required and adds Copy-carrier uniqueness refusal controls.
    host(&root, &ast, "0", "42", "copy-record-codec");
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn scratch_refusal_selects_vec_code_three_and_settles_owner_on_native_and_wasm() {
    let source = format!("{DECLARATION}{ONE_CALL}");
    let ast = semaprax::check(&source, "unique-refusal.spx").unwrap();
    let root = root();
    native(&root, &ast, true, &[4 * 2]);
    // Core-Wasm STATUS_VEC_ALLOCATION_FAILURE; the strict host proves no
    // scratch was observed allocated and every owner settled.
    host(&root, &ast, "15", "0", "unique-scratch");
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn strict_host_refuses_forged_results_shapes_positions_and_stale_authority() {
    let source = format!("{DECLARATION}{ONE_CALL}");
    let ast = semaprax::check(&source, "unique-hostile.spx").unwrap();
    let root = root();
    for mode in [
        "unique-bad-result",
        "unique-forged-shape",
        "unique-forged-position",
        "unique-controls",
    ] {
        // Hostile modes return after their own trap/teardown assertions; the
        // controls mode runs the ordinary corpus first, then exact host checks.
        host(&root, &ast, "0", "42", mode);
    }
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn live_root_moves_and_while_bodies_stay_closed() {
    for body in [
        r#"@id("app.main") fn main()->i64 { let rows=vec_with_capacity<Row>(0usize); let moved=rows; if vec_unique_by<Row>(rows,"title") {42}else{0} }"#,
        r#"@id("app.main") fn main()->i64 { let rows=vec_with_capacity<Row>(0usize); let mut i=0usize; let mut ok=true; while i<1usize { ok=vec_unique_by<Row>(rows,"title"); i=i+1usize; 0 } if ok {42}else{0} }"#,
        r#"@id("app.main") fn main()->i64 { if vec_unique_by<Row>(vec_with_capacity<Row>(0usize),"title") {42}else{0} }"#,
        r#"@id("app.main") fn main()->i64 { let rows=vec_with_capacity<Row>(0usize); if vec_unique_by<Row>(rows,"payload") {42}else{0} }"#,
    ] {
        assert!(
            semaprax::check(&format!("{DECLARATION}{body}"), "closed.spx").is_err(),
            "{body}"
        );
    }
}
