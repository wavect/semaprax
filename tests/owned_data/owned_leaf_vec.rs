//! OPT-723 private Core-Wasm host qualification for directly owned leaves.
use std::process::Command;
use std::sync::atomic::{AtomicU64, Ordering};

static SEQUENCE: AtomicU64 = AtomicU64::new(0);
const HOST: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/owned_data/owned_leaf_vec/host.js");
const DECLARATION: &str = r#"module app.leaf;
@id("app.leaf.row") record Row {
 @id("app.leaf.row.priority") priority:i64,
 @id("app.leaf.row.title") title:String,
 @id("app.leaf.row.payload") payload:Bytes,
}
"#;

fn run_wasm(body: &str, status: u32, value: i64, refusal: &str) {
    let source = format!("{DECLARATION}{body}");
    let program = semaprax::check(&source, "owned-leaf-vec-wasm.spx")
        .expect("source and HIR must admit the owned-leaf profile");
    let bytes = semaprax::wasm::emit_module(&program)
        .expect("Core Wasm must lower the owned-leaf profile");
    let path = std::env::temp_dir().join(format!(
        "owned-leaf-vec-{}-{}.wasm", std::process::id(), SEQUENCE.fetch_add(1, Ordering::Relaxed),
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
    assert!(output.status.success(), "{refusal}: {}", String::from_utf8_lossy(&output.stderr));
}

#[test]
fn owned_string_vec_sorts_clones_replaces_reserves_and_clears() {
    run_wasm(r#"@id("app.main") fn main()->i64 {
 let empty=vec_with_capacity<String>(2usize);
 let one=vec_push<String>(empty,"z");
 let two=vec_push<String>(one,"a");
 let sorted=vec_sort_owned<String>(two);
 let first=vec_clone_at<String>(sorted,0usize);
 let renewed=vec_replace<String>(sorted,1usize,"m");
 let reserved=vec_reserve_owned<String>(renewed,2usize);
 let good=first=="a" && vec_len<String>(reserved)==2usize && vec_capacity<String>(reserved)==4usize;
 let cleared=vec_clear<String>(reserved);
 if good && vec_len<String>(cleared)==0usize && vec_capacity<String>(cleared)==4usize {29}else{0}
}

#[test]
fn owned_string_order_is_unsigned_utf8_with_prefix_and_nul() {
    run_wasm(r#"@id("app.main") fn main()->i64 {
 let a=vec_with_capacity<String>(4usize);
 let b=vec_push<String>(a,"é");
 let c=vec_push<String>(b,"a\u{0}");
 let d=vec_push<String>(c,"z");
 let e=vec_push<String>(d,"a");
 let sorted=vec_sort_owned<String>(e);
 let first=vec_clone_at<String>(sorted,0usize);
 let second=vec_clone_at<String>(sorted,1usize);
 let third=vec_clone_at<String>(sorted,2usize);
 let fourth=vec_clone_at<String>(sorted,3usize);
 if first=="a" && second=="a\u{0}" && third=="z" && fourth=="é" {29}else{0}
}

#[test]
fn element_capacity_uses_existing_carrier_envelopes() {
    run_wasm(r#"@id("app.main") fn main()->i64 {
 let a=vec_with_capacity<String>(8192usize);
 if vec_capacity<String>(a)==8192usize {29}else{0}
}
"#, 0, 29, "none");
    run_wasm(r#"@id("app.main") fn main()->i64 {
 let a=vec_with_capacity<String>(8193usize);
 if vec_capacity<String>(a)==8193usize {29}else{0}
}
"#, 15, 0, "none");
}
"#, 0, 29, "none");
}
"#, 0, 29, "none");
}

#[test]
fn heterogeneous_record_sort_and_copy_out_preserve_declaration_order() {
    run_wasm(r#"@id("app.main") fn main()->i64 {
 let empty=vec_with_capacity<Row>(2usize);
 let one=vec_push<Row>(empty,Row{priority:2,title:"z",payload:bytes_zeroed(2usize)});
 let two=vec_push<Row>(one,Row{priority:-1,title:"a",payload:bytes_zeroed(1usize)});
 let sorted=vec_sort_owned<Row>(two);
 let first=vec_clone_at<Row>(sorted,0usize);
 let good=first.priority==-1 && first.title=="a" && byte_len(bytes_as_slice(first.payload))==1usize;
 let renewed=vec_replace<Row>(sorted,1usize,Row{priority:3,title:"m",payload:bytes_zeroed(3usize)});
 if good && vec_len<Row>(renewed)==2usize {29}else{0}
}

#[test]
fn byte_leaf_order_uses_unsigned_prefix_after_equal_prior_fields() {
    run_wasm(r#"@id("app.main") fn main()->i64 {
 let a=vec_with_capacity<Row>(2usize);
 let b=vec_push<Row>(a,Row{priority:1,title:"tie",payload:bytes_zeroed(2usize)});
 let c=vec_push<Row>(b,Row{priority:1,title:"tie",payload:bytes_zeroed(1usize)});
 let sorted=vec_sort_owned<Row>(c);
 let first=vec_clone_at<Row>(sorted,0usize);
 if byte_len(bytes_as_slice(first.payload))==1usize {29}else{0}
}
"#, 0, 29, "none");
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
        let body = format!(r#"@id("app.leaf.width") record Width {{
 @id("app.leaf.width.key") key:{ty},
 @id("app.leaf.width.name") name:String,
}}
@id("app.main") fn main()->i64 {{
 let a=vec_with_capacity<Width>(2usize);
 let b=vec_push<Width>(a,Width{{key:{high},name:"same"}});
 let c=vec_push<Width>(b,Width{{key:{low},name:"same"}});
 let sorted=vec_sort_owned<Width>(c);
 let first=vec_clone_at<Width>(sorted,0usize);
 if first.key=={low} && first.name=="same" {{29}}else{{0}}
}}
"#);
        run_wasm(&body, 0, 29, "none");
    }
}
"#, 0, 29, "none");
}

#[test]
fn second_owned_leaf_clone_refusal_settles_partial_result_and_source() {
    run_wasm(r#"@id("app.main") fn main()->i64 {
 let empty=vec_with_capacity<Row>(1usize);
 let one=vec_push<Row>(empty,Row{priority:1,title:"text",payload:bytes_zeroed(1usize)});
 let cloned=vec_clone_at<Row>(one,0usize);
 if cloned.priority==1 {29}else{0}
}

#[test]
fn push_allocation_failure_is_distinct_from_full_capacity() {
    run_wasm(r#"@id("app.main") fn main()->i64 {
 let empty=vec_with_capacity<String>(1usize);
 let one=vec_push<String>(empty,"value");
 if vec_len<String>(one)==1usize {29}else{0}
}

#[test]
fn replacement_bounds_and_reserve_overflow_settle_staged_owners() {
    run_wasm(r#"@id("app.main") fn main()->i64 {
 let a=vec_with_capacity<String>(1usize);
 let b=vec_push<String>(a,"old");
 let c=vec_replace<String>(b,1usize,"replacement");
 if vec_len<String>(c)==1usize {29}else{0}
}
"#, 14, 0, "none");
    run_wasm(r#"@id("app.main") fn main()->i64 {
 let a=vec_push<String>(vec_with_capacity<String>(1usize),"old");
 let out=vec_clone_at<String>(a,1usize);
 if out=="old" {29}else{0}
}
"#, 14, 0, "none");
    run_wasm(r#"@id("app.main") fn main()->i64 {
 let a=vec_with_capacity<String>(1usize);
 let b=vec_push<String>(a,"old");
 let c=vec_reserve_owned<String>(b,8192usize);
 if vec_capacity<String>(c)>1usize {29}else{0}
}
"#, 15, 0, "none");
    run_wasm(r#"@id("app.main") fn main()->i64 {
 let a=vec_with_capacity<String>(1usize);
 let b=vec_push<String>(a,"old");
 let c=vec_reserve_owned<String>(b,1usize);
 if vec_capacity<String>(c)==2usize {29}else{0}
}
"#, 15, 0, "reserve-allocation");
}

#[test]
fn sort_and_replacement_allocation_failures_keep_old_generation_live_for_cleanup() {
    run_wasm(r#"@id("app.main") fn main()->i64 {
 let a=vec_with_capacity<String>(2usize);
 let b=vec_push<String>(a,"b");
 let c=vec_push<String>(b,"a");
 let sorted=vec_sort_owned<String>(c);
 if vec_len<String>(sorted)==2usize {29}else{0}
}
"#, 15, 0, "sort-allocation");
    run_wasm(r#"@id("app.main") fn main()->i64 {
 let a=vec_with_capacity<String>(1usize);
 let b=vec_push<String>(a,"old");
 let c=vec_replace<String>(b,0usize,"new");
 if vec_len<String>(c)==1usize {29}else{0}
}
"#, 15, 0, "replacement-allocation");
}
"#, 15, 0, "push-allocation");
    run_wasm(r#"@id("app.main") fn main()->i64 {
 let empty=vec_with_capacity<String>(0usize);
 let one=vec_push<String>(empty,"value");
 if vec_len<String>(one)==1usize {29}else{0}
}
"#, 13, 0, "none");
}
"#, 15, 0, "second-clone");
}

#[test]
fn owned_leaf_iterator_transfers_item_and_drops_unvisited_suffix() {
    run_wasm(r#"@id("app.main") fn main()->i64 {
 let empty=vec_with_capacity<String>(2usize);
 let one=vec_push<String>(empty,"first");
 let two=vec_push<String>(one,"tail");
 match own iter_next<String>(vec_into_iter<String>(two)) {
  IterStep::Done{}=>0,
  IterStep::Yield{item,rest}=>if item=="first" {29}else{0},
 }
}

#[test]
fn iterator_host_refusals_preserve_uncommitted_vector_and_suffix() {
    let body = r#"@id("app.main") fn main()->i64 {
 let a=vec_with_capacity<String>(1usize);
 let b=vec_push<String>(a,"owned");
 match own iter_next<String>(vec_into_iter<String>(b)) {
  IterStep::Done{}=>0,
  IterStep::Yield{item,rest}=>if item=="owned" {29}else{0},
 }
}
"#;
    run_wasm(body, 15, 0, "iterator-allocation");
    run_wasm(body, 15, 0, "iterator-next-allocation");
}
"#, 0, 29, "none");
}

#[test]
fn legacy_record_new_ops_preserve_old_physical_carrier() {
    let legacy = DECLARATION.replace("title:String,", "title:Bytes,");
    let body = r#"@id("app.main") fn main()->i64 {
 let empty=vec_with_capacity<Row>(2usize);
 let one=vec_push<Row>(empty,Row{priority:7,title:bytes_zeroed(2usize),payload:bytes_zeroed(1usize)});
 let two=vec_push<Row>(one,Row{priority:2,title:bytes_zeroed(3usize),payload:bytes_zeroed(1usize)});
 let sorted=vec_sort_owned<Row>(two);
 let copied=vec_clone_at<Row>(sorted,0usize);
 let copied_ok=copied.priority==2 && byte_len(bytes_as_slice(copied.title))==3usize;
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
    let output = Command::new("node").arg(HOST).arg(&path).arg("0").arg("29")
        .arg("none").output().unwrap();
    std::fs::remove_file(path).unwrap();
    assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));
}
