//! Same-source scoped reads through the strict private Wasm host.
use super::run_wasm;

const REPEATED: &str = r#"@id("app.main") fn main()->i64 {
 let bytes=[7u8,0u8,255u8];
 let empty=vec_with_capacity<Row>(1usize);
 let rows=vec_push<Row>(empty,Row{priority:-9,title:"a\u{0}é",payload:bytes_copy(array_as_slice(bytes))});
 let mut index=0usize;
 let selected=vec_field<Row>(rows,index,"title");
 index=1usize;
 let first_ok=str_len_bytes(selected)==4usize;
 let mut repeat=0usize;
 let mut good=first_ok;
 while repeat<64usize {
  let text=str_as_bytes(vec_field<Row>(rows,0usize,"title"));
  let data=vec_field<Row>(rows,0usize,"payload");
  good=good && vec_field<Row>(rows,0usize,"priority")==-9
   && byte_len(text)==4usize && byte_get(text,1usize)==0u8
   && byte_get(text,2usize)==195u8 && byte_get(text,3usize)==169u8
   && byte_len(data)==3usize && byte_get(data,0usize)==7u8 && byte_get(data,2usize)==255u8;
  repeat=repeat+1usize;
  0
 }
 let renewed=vec_sort_owned<Row>(rows);
 if good && index==1usize && vec_len<Row>(renewed)==1usize {29}else{0}
}
"#;

#[test]
fn repeated_scalar_string_and_bytes_reads_retain_payload_and_vector_identity() {
    run_wasm(REPEATED, 0, 29, "field-no-allocation");
}

#[test]
fn every_copy_field_shape_retains_its_scalar_bits() {
    for (ty, literal, expected) in [
        ("i64", "-9", "got==-9"),
        ("i32", "-7i32", "got==-7i32"),
        ("u8", "255u8", "got==255u8"),
        ("usize", "18446744073709551615usize", "got==18446744073709551615usize"),
        ("char", "'🦀'", "got=='🦀'"),
        ("f32", "-0.0f32", "1.0f32/got<0.0f32"),
        ("f64", "-0.0", "1.0/got<0.0"),
        ("bool", "true", "got"),
    ] {
        let body = format!(r#"@id("field.row") record ScalarRow {{
 @id("field.row.text") text:string,@id("field.row.scalar") scalar:{ty},
}}
@id("app.main") fn main()->i64 {{
 let empty=vec_with_capacity<ScalarRow>(1usize);
 let rows=vec_push<ScalarRow>(empty,ScalarRow{{text:"",scalar:{literal}}});
 let got=vec_field<ScalarRow>(rows,0usize,"scalar");
 let text=vec_field<ScalarRow>(rows,0usize,"text");
 if {expected} && str_len_bytes(text)==0usize {{29}}else{{0}}
}}
"#);
        run_wasm(&body, 0, 29, "none");
    }
}

#[test]
fn legacy_tag10_permuted_fields_are_bound_at_construction() {
    run_wasm(r#"@id("legacy") record Legacy {
 @id("legacy.scalar") scalar:f32,@id("legacy.first") first:Bytes,@id("legacy.last") last:Bytes,
}
@id("app.main") fn main()->i64 {
 let initial=vec_with_capacity<Legacy>(1usize);
 let rows=vec_push<Legacy>(initial,Legacy{scalar:-2.5f32,first:bytes_zeroed(1usize),last:bytes_zeroed(2usize)});
 let a=vec_field<Legacy>(rows,0usize,"first");
 let b=vec_field<Legacy>(rows,0usize,"last");
 if vec_field<Legacy>(rows,0usize,"scalar")==-2.5f32 && byte_len(a)==1usize && byte_len(b)==2usize {29}else{0}
}
"#, 0, 29, "none");
}

#[test]
fn projected_vector_borrow_is_retained_until_last_view_use() {
    run_wasm(r#"@id("field.report") record Report { @id("field.report.rows") rows:Vec<Row>, }
@id("field.inspect") fn inspect(value:borrow Report)->i64 {
 let title=vec_field<Row>(value.rows,0usize,"title");
 let payload=vec_field<Row>(value.rows,0usize,"payload");
 if str_len_bytes(title)==4usize && byte_len(payload)==2usize && vec_field<Row>(value.rows,0usize,"priority")==3 {29}else{0}
}
@id("app.main") fn main()->i64 {
 let rows=vec_with_capacity<Row>(1usize);
 let one=vec_push<Row>(rows,Row{priority:3,title:"a\u{0}é",payload:bytes_zeroed(2usize)});
 let report=Report{rows:one};
 let good=inspect(report)==29;
 match own report {Report{rows}=>if good && vec_len<Row>(rows)==1usize {29}else{0},}
}
"#, 0, 29, "none");
}

#[test]
fn index_refusal_and_index_evaluation_failure_keep_source_cleanup() {
    for index in ["1usize", "18446744073709551615usize"] {
        run_wasm(&format!(r#"@id("app.main") fn main()->i64 {{
 let empty=vec_with_capacity<Row>(1usize);
 let rows=vec_push<Row>(empty,Row{{priority:7,title:"owned",payload:bytes_zeroed(2usize)}});
 vec_field<Row>(rows,{index},"priority")
}}
"#), 14, 0, "none");
    }
    run_wasm(r#"@id("app.main") fn main()->i64 {
 let rows=vec_with_capacity<Row>(1usize);
 vec_field<Row>(rows,0usize,"priority")
}
"#, 14, 0, "none");
    run_wasm(r#"@id("field.index") fn index()->usize {1usize/0usize}
@id("app.main") fn main()->i64 {
 let empty=vec_with_capacity<Row>(1usize);
 let rows=vec_push<Row>(empty,Row{priority:7,title:"owned",payload:bytes_zeroed(2usize)});
 vec_field<Row>(rows,index(),"priority")
}
"#, 4, 0, "field-index-failure");
}

#[test]
fn malformed_host_leaf_and_descriptor_cannot_be_published() {
    let body = r#"@id("app.main") fn main()->i64 {
 let empty=vec_with_capacity<Row>(1usize);
 let rows=vec_push<Row>(empty,Row{priority:7,title:"owned",payload:bytes_zeroed(2usize)});
 let text=vec_field<Row>(rows,0usize,"title");
 if str_len_bytes(text)==5usize {29}else{0}
}
"#;
    run_wasm(body, 0, 0, "field-bad-word");
    run_wasm(body, 0, 0, "field-forged-shape");
    run_wasm(body, 0, 0, "field-unknown-status");
}

#[test]
fn malformed_scalar_host_words_trap_before_scalar_publication() {
    run_wasm(r#"@id("field.bool") record Flag { @id("field.bool.text") text:string,@id("field.bool.flag") flag:bool, }
@id("app.main") fn main()->i64 {
 let empty=vec_with_capacity<Flag>(1usize);
 let rows=vec_push<Flag>(empty,Flag{text:"owned",flag:true});
 if vec_field<Flag>(rows,0usize,"flag") {29}else{0}
}
"#, 0, 0, "field-bad-word");
}

#[test]
fn private_read_import_is_additive_and_absent_from_old_programs() {
    let old = r#"@id("app.main") fn main()->i64 {
 let empty=vec_with_capacity<Row>(1usize);
 let rows=vec_push<Row>(empty,Row{priority:7,title:"owned",payload:bytes_zeroed(2usize)});
 if vec_len<Row>(rows)==1usize {29}else{0}
}
"#;
    let new = old.replace("vec_len<Row>(rows)==1usize", "vec_field<Row>(rows,0usize,\"priority\")==7");
    let module = |body: &str| {
        let source = format!("{}{body}", super::DECLARATION);
        let program = semaprax::check(&source, "field-imports.spx").unwrap();
        let bytes = semaprax::wasm::emit_module(&program).unwrap();
        wasmparser::Validator::new().validate_all(&bytes).unwrap();
        bytes
    };
    let old = module(old);
    let new = module(&new);
    let count = |bytes: &[u8], name: &str| bytes.windows(name.len()).filter(|part| *part == name.as_bytes()).count();
    for name in ["spx_vec_leaf_new_v1", "spx_vec_leaf_push_v1", "spx_vec_leaf_clone_at_v1",
        "spx_vec_leaf_replace_v1", "spx_vec_leaf_reserve_v1", "spx_vec_leaf_sort_v1",
        "spx_vec_leaf_into_iter_v1", "spx_vec_leaf_iter_next_v1", "spx_vec_leaf_iter_drop_v1"] {
        assert_eq!(count(&old, name), 1, "old {name}");
        assert_eq!(count(&new, name), 1, "new {name}");
    }
    assert_eq!(count(&old, "spx_vec_leaf_field_read_v1"), 0);
    assert_eq!(count(&new, "spx_vec_leaf_field_read_v1"), 1);
}
