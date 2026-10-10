//! OPT-723 private Core-Wasm host qualification for directly owned leaves.
#[path = "owned_leaf_vec/field_reads.rs"]
mod field_reads;

use std::process::Command;
use std::sync::atomic::{AtomicU64, Ordering};

static SEQUENCE: AtomicU64 = AtomicU64::new(0);
const HOST: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/tests/owned_data/owned_leaf_vec/host.js"
);
const DECLARATION: &str = r#"module app.leaf;
@id("app.leaf.row") record Row {
 @id("app.leaf.row.priority") priority:i64,
 @id("app.leaf.row.title") title:string,
 @id("app.leaf.row.payload") payload:Bytes,
}
"#;

fn run_wasm(body: &str, status: u32, value: i64, refusal: &str) {
    let source = format!("{DECLARATION}{body}");
    let program = semaprax::check(&source, "owned-leaf-vec-wasm.spx")
        .expect("source and HIR must admit the owned-leaf profile");
    let bytes =
        semaprax::wasm::emit_module(&program).expect("Core Wasm must lower the owned-leaf profile");
    let path = std::env::temp_dir().join(format!(
        "owned-leaf-vec-{}-{}.wasm",
        std::process::id(),
        SEQUENCE.fetch_add(1, Ordering::Relaxed),
    ));
    std::fs::write(&path, bytes).unwrap();
    let output = Command::new("node")
        .arg(HOST)
        .arg(&path)
        .arg(status.to_string())
        .arg(value.to_string())
        .arg(refusal)
        .output()
        .expect("Node must execute the private host");
    std::fs::remove_file(path).unwrap();
    assert!(
        output.status.success(),
        "{refusal}: {}",
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn owned_string_vec_sorts_clones_replaces_reserves_and_clears() {
    run_wasm(
        r#"@id("app.main") fn main()->i64 {
 let empty=vec_with_capacity<string>(2usize);
 let one=vec_push<string>(empty,"z");
 let two=vec_push<string>(one,"a");
 let sorted=vec_sort_owned<string>(two);
 let first=vec_clone_at<string>(sorted,0usize);
 let renewed=vec_replace<string>(sorted,1usize,"m");
 let reserved=vec_reserve_owned<string>(renewed,2usize);
 let good=first=="a" && vec_len<string>(reserved)==2usize && vec_capacity<string>(reserved)==4usize;
 let cleared=vec_clear<string>(reserved);
 if good && vec_len<string>(cleared)==0usize && vec_capacity<string>(cleared)==4usize {29}else{0}
}
"#,
        0,
        29,
        "none",
    );
}

#[test]
fn owned_string_order_is_unsigned_utf8_with_prefix_and_nul() {
    run_wasm(
        r#"@id("app.main") fn main()->i64 {
 let a=vec_with_capacity<string>(4usize);
 let b=vec_push<string>(a,"é");
 let c=vec_push<string>(b,"a\u{0}");
 let d=vec_push<string>(c,"z");
 let e=vec_push<string>(d,"a");
 let sorted=vec_sort_owned<string>(e);
 let first=vec_clone_at<string>(sorted,0usize);
 let second=vec_clone_at<string>(sorted,1usize);
 let third=vec_clone_at<string>(sorted,2usize);
 let fourth=vec_clone_at<string>(sorted,3usize);
 if first=="a" && second=="a\u{0}" && third=="z" && fourth=="é" {29}else{0}
}
"#,
        0,
        29,
        "none",
    );
}

#[test]
fn element_capacity_uses_existing_carrier_envelopes() {
    run_wasm(
        r#"@id("app.main") fn main()->i64 {
 let a=vec_with_capacity<string>(8192usize);
 if vec_capacity<string>(a)==8192usize {29}else{0}
}
"#,
        0,
        29,
        "none",
    );
    run_wasm(
        r#"@id("app.main") fn main()->i64 {
 let a=vec_with_capacity<string>(8193usize);
 if vec_capacity<string>(a)==8193usize {29}else{0}
}
"#,
        15,
        0,
        "none",
    );
}

#[test]
fn heterogeneous_record_sort_and_copy_out_preserve_declaration_order() {
    run_wasm(
        r#"@id("app.main") fn main()->i64 {
 let empty=vec_with_capacity<Row>(2usize);
 let one=vec_push<Row>(empty,Row{priority:2,title:"z",payload:bytes_zeroed(2usize)});
 let two=vec_push<Row>(one,Row{priority:-1,title:"a",payload:bytes_zeroed(1usize)});
 let sorted=vec_sort_owned<Row>(two);
 let first=vec_clone_at<Row>(sorted,0usize);
 let good=match own first { Row{priority,title,payload}=>{
  let view=bytes_as_slice(payload);
  priority==-1 && title=="a" && byte_len(view)==1usize
 }, };
 let renewed=vec_replace<Row>(sorted,1usize,Row{priority:3,title:"m",payload:bytes_zeroed(3usize)});
 if good && vec_len<Row>(renewed)==2usize {29}else{0}
}
"#,
        0,
        29,
        "none",
    );
}

#[test]
fn byte_leaf_order_uses_unsigned_prefix_after_equal_prior_fields() {
    run_wasm(
        r#"@id("app.main") fn main()->i64 {
 let a=vec_with_capacity<Row>(2usize);
 let b=vec_push<Row>(a,Row{priority:1,title:"tie",payload:bytes_zeroed(2usize)});
 let c=vec_push<Row>(b,Row{priority:1,title:"tie",payload:bytes_zeroed(1usize)});
 let sorted=vec_sort_owned<Row>(c);
 let first=vec_clone_at<Row>(sorted,0usize);
 let good=match own first { Row{priority,title,payload}=>{
  let view=bytes_as_slice(payload);
  priority==1 && title=="tie" && byte_len(view)==1usize
 }, };
 if good {29}else{0}
}
"#,
        0,
        29,
        "none",
    );
}

#[test]
fn one_owned_leaf_records_sort_each_copy_scalar_width() {
    for (ty, high, low) in [
        ("i64", "3", "-7"),
        ("i32", "3i32", "-7i32"),
        ("u8", "255u8", "0u8"),
        ("usize", "99usize", "0usize"),
        ("char", "'z'", "'a'"),
        ("f32", "1.5f32", "-0.0f32"),
        ("f64", "1.5", "-0.0"),
        ("bool", "true", "false"),
    ] {
        let body = format!(
            r#"@id("app.leaf.width") record Width {{
 @id("app.leaf.width.key") key:{ty},
 @id("app.leaf.width.name") name:string,
}}
@id("app.main") fn main()->i64 {{
 let a=vec_with_capacity<Width>(2usize);
 let b=vec_push<Width>(a,Width{{key:{high},name:"same"}});
 let c=vec_push<Width>(b,Width{{key:{low},name:"same"}});
 let sorted=vec_sort_owned<Width>(c);
 let first=vec_clone_at<Width>(sorted,0usize);
 let good=match own first {{ Width{{key,name}}=>key=={low} && name=="same", }};
 if good {{29}}else{{0}}
}}
"#
        );
        run_wasm(&body, 0, 29, "none");
    }
}

#[test]
fn second_owned_leaf_clone_refusal_settles_partial_result_and_source() {
    let body = r#"@id("app.main") fn main()->i64 {
 let empty=vec_with_capacity<Row>(1usize);
 let one=vec_push<Row>(empty,Row{priority:1,title:"text",payload:bytes_zeroed(1usize)});
 let cloned=vec_clone_at<Row>(one,0usize);
 if cloned.priority==1 {29}else{0}
}
"#;
    run_wasm(body, 15, 0, "string-clone-allocation");
    run_wasm(body, 15, 0, "bytes-clone-allocation");
    run_wasm(body, 15, 0, "second-clone");
}

#[test]
fn push_allocation_failure_is_distinct_from_full_capacity() {
    run_wasm(
        r#"@id("app.main") fn main()->i64 {
 let empty=vec_with_capacity<string>(1usize);
 let one=vec_push<string>(empty,"value");
 if vec_len<string>(one)==1usize {29}else{0}
}
"#,
        15,
        0,
        "push-allocation",
    );
    run_wasm(
        r#"@id("app.main") fn main()->i64 {
 let empty=vec_with_capacity<string>(0usize);
 let one=vec_push<string>(empty,"value");
 if vec_len<string>(one)==1usize {29}else{0}
}
"#,
        13,
        0,
        "none",
    );
}

#[test]
fn replacement_bounds_and_reserve_overflow_settle_staged_owners() {
    run_wasm(
        r#"@id("app.main") fn main()->i64 {
 let a=vec_with_capacity<string>(1usize);
 let b=vec_push<string>(a,"old");
 let c=vec_replace<string>(b,1usize,"replacement");
 if vec_len<string>(c)==1usize {29}else{0}
}
"#,
        14,
        0,
        "none",
    );
    run_wasm(
        r#"@id("app.main") fn main()->i64 {
 let a=vec_push<string>(vec_with_capacity<string>(1usize),"old");
 let out=vec_clone_at<string>(a,1usize);
 if out=="old" {29}else{0}
}
"#,
        14,
        0,
        "none",
    );
    run_wasm(
        r#"@id("app.main") fn main()->i64 {
 let a=vec_with_capacity<string>(1usize);
 let b=vec_push<string>(a,"old");
 let c=vec_reserve_owned<string>(b,8192usize);
 if vec_capacity<string>(c)>1usize {29}else{0}
}
"#,
        15,
        0,
        "none",
    );
    run_wasm(
        r#"@id("app.main") fn main()->i64 {
 let a=vec_with_capacity<string>(1usize);
 let b=vec_push<string>(a,"old");
 let c=vec_reserve_owned<string>(b,1usize);
 if vec_capacity<string>(c)==2usize {29}else{0}
}
"#,
        15,
        0,
        "reserve-allocation",
    );
}

#[test]
fn owned_sort_succeeds_without_payload_row_or_authority_allocation() {
    run_wasm(
        r#"@id("app.main") fn main()->i64 {
 let a=vec_with_capacity<string>(2usize);
 let b=vec_push<string>(a,"b");
 let c=vec_push<string>(b,"a");
 let sorted=vec_sort_owned<string>(c);
 let first=vec_clone_at<string>(sorted,0usize);
 let last=vec_clone_at<string>(sorted,1usize);
 if first=="a" && last=="b" {29}else{0}
}
"#,
        0,
        29,
        "sort-no-allocation",
    );
}

#[test]
fn replacement_allocation_failure_keeps_old_generation_live_for_cleanup() {
    run_wasm(
        r#"@id("app.main") fn main()->i64 {
 let a=vec_with_capacity<string>(1usize);
 let b=vec_push<string>(a,"old");
 let c=vec_replace<string>(b,0usize,"new");
 if vec_len<string>(c)==1usize {29}else{0}
}
"#,
        15,
        0,
        "replacement-allocation",
    );
}

#[test]
fn additive_sort_of_legacy_record_carriers_is_also_allocation_free() {
    run_wasm(
        r#"@id("legacy") record Legacy {
 @id("legacy.a") a:Bytes,@id("legacy.key") key:i64,@id("legacy.b") b:Bytes,
}
@id("app.main") fn main()->i64 {
 let rows=vec_with_capacity<Legacy>(2usize);
 let one=vec_push<Legacy>(rows,Legacy{a:bytes_zeroed(1usize),key:3,b:bytes_zeroed(1usize)});
 let two=vec_push<Legacy>(one,Legacy{a:bytes_zeroed(1usize),key:-7,b:bytes_zeroed(1usize)});
 let sorted=vec_sort_owned<Legacy>(two);
 let first=vec_clone_at<Legacy>(sorted,0usize);
 match own first {Legacy{a,key,b}=>if key == -7 {29}else{0},}
}
"#,
        0,
        29,
        "sort-no-allocation",
    );
}

#[test]
fn null_infallible_sort_result_is_a_host_invariant_trap() {
    run_wasm(
        r#"@id("app.main") fn main()->i64 {
 let rows=vec_with_capacity<string>(1usize);
 let one=vec_push<string>(rows,"owned");
 let sorted=vec_sort_owned<string>(one);
 if vec_len<string>(sorted)==1usize {29}else{0}
}
"#,
        0,
        0,
        "sort-null",
    );
}

#[test]
fn owned_leaf_iterator_transfers_item_and_drops_unvisited_suffix() {
    run_wasm(
        r#"@id("app.main") fn main()->i64 {
 let empty=vec_with_capacity<string>(2usize);
 let one=vec_push<string>(empty,"first");
 let two=vec_push<string>(one,"tail");
 match own iter_next<string>(vec_into_iter<string>(two)) {
  IterStep::Done{}=>0,
  IterStep::Yield{item,rest}=>if item=="first" {29}else{0},
 }
}
"#,
        0,
        29,
        "none",
    );
}

#[test]
fn iterator_host_refusals_preserve_uncommitted_vector_and_suffix() {
    let body = r#"@id("app.main") fn main()->i64 {
 let a=vec_with_capacity<string>(1usize);
 let b=vec_push<string>(a,"owned");
 match own iter_next<string>(vec_into_iter<string>(b)) {
  IterStep::Done{}=>0,
  IterStep::Yield{item,rest}=>if item=="owned" {29}else{0},
 }
}
"#;
    run_wasm(body, 15, 0, "iterator-allocation");
    run_wasm(body, 15, 0, "iterator-next-allocation");
}

#[test]
fn header_only_owned_iterator_selects_private_drop_import() {
    run_wasm(
        r#"@id("app.discard") fn discard(it: own Iter<string>)->i64 {0}
@id("app.main") fn main()->i64 {29}
"#,
        0,
        29,
        "none",
    );
}

#[test]
fn legacy_record_new_ops_preserve_old_physical_carrier() {
    let legacy = DECLARATION.replace("title:string,", "title:Bytes,");
    let body = r#"@id("app.main") fn main()->i64 {
 let empty=vec_with_capacity<Row>(2usize);
 let one=vec_push<Row>(empty,Row{priority:7,title:bytes_zeroed(2usize),payload:bytes_zeroed(1usize)});
 let two=vec_push<Row>(one,Row{priority:2,title:bytes_zeroed(3usize),payload:bytes_zeroed(1usize)});
 let sorted=vec_sort_owned<Row>(two);
 let copied=vec_clone_at<Row>(sorted,0usize);
 let copied_ok=match own copied { Row{priority,title,payload}=>{
  let view=bytes_as_slice(title);
  priority==2 && byte_len(view)==3usize
 }, };
 match own iter_next<Row>(vec_into_iter<Row>(sorted)) {
  IterStep::Done{}=>0,
  IterStep::Yield{item,rest}=>if copied_ok && item.priority==2 {29}else{0},
 }
}
"#;
    let source = format!("{legacy}{body}");
    let program = semaprax::check(&source, "owned-leaf-legacy-wasm.spx").unwrap();
    let bytes = semaprax::wasm::emit_module(&program).unwrap();
    let path = std::env::temp_dir().join(format!("owned-leaf-legacy-{}.wasm", std::process::id()));
    std::fs::write(&path, bytes).unwrap();
    let output = Command::new("node")
        .arg(HOST)
        .arg(&path)
        .arg("0")
        .arg("29")
        .arg("none")
        .output()
        .unwrap();
    std::fs::remove_file(path).unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
}
