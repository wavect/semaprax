//! Shared exact source corpus for the two existing backend harness owners.
pub(crate) fn cases() -> [(&'static str, &'static str, &'static str); 8] {
    [
        (
            "i64",
            "-9223372036854775807-1",
            "got==-9223372036854775807-1",
        ),
        ("i32", "-2147483647i32-1i32", "got==-2147483647i32-1i32"),
        ("u8", "255u8", "got==255u8"),
        (
            "usize",
            "18446744073709551615usize",
            "got==18446744073709551615usize",
        ),
        ("char", "'🦀'", "got=='🦀'"),
        ("f32", "-0.0f32", "1.0f32/got<0.0f32"),
        ("f64", "0.0/0.0", "got!=got"),
        ("bool", "false", "!got"),
    ]
}
pub(crate) fn declaration(ty: &str) -> String {
    format!(
        r#"module field.update;
@id("field.row") record Row {{
 @id("field.row.title") title:string,
 @id("field.row.scalar") scalar:{ty},
 @id("field.row.payload") payload:Bytes,
}}
"#
    )
}
pub(crate) fn source(ty: &str, literal: &str, expected: &str) -> String {
    let initial = match ty {
        "i64" => "7",
        "i32" => "7i32",
        "u8" => "0u8",
        "usize" => "0usize",
        "char" => "'A'",
        "f32" => "1.0f32",
        "f64" => "1.0",
        "bool" => "true",
        _ => unreachable!(),
    };
    let second_expected = expected.replace("got", "second");
    format!(
        r#"{}@id("owned.leaf.main") fn main()->i64 {{
 let a=vec_with_capacity<Row>(2usize);
 let b=vec_push<Row>(a,Row{{title:"a\u{{0}}é",scalar:{initial},payload:bytes_zeroed(3usize)}});
 let c=vec_push<Row>(b,Row{{title:"second",scalar:{initial},payload:bytes_zeroed(1usize)}});
 let mut rows=vec_update_field<Row>(c,1usize,{literal},"scalar");
 let mut n=0usize;
 while n<3usize {{
  rows=vec_update_field<Row>(rows,0usize,{literal},"scalar");
  n=n+1usize;
  0
 }}
 let got=vec_field<Row>(rows,0usize,"scalar");
 let second=vec_field<Row>(rows,1usize,"scalar");
 let title=vec_field<Row>(rows,0usize,"title");
 let payload=vec_field<Row>(rows,0usize,"payload");
 if {expected} && {second_expected} && str_len_bytes(title)==4 && byte_len(payload)==3usize
  && vec_len<Row>(rows)==2usize && vec_capacity<Row>(rows)==2usize {{29}}else{{0}}
}}
"#,
        declaration(ty)
    )
}
pub(crate) fn failure(index: &str, value: &str, helpers: &str) -> String {
    format!(
        r#"{}{helpers}
@id("owned.leaf.main") fn main()->i64 {{
 let a=vec_with_capacity<Row>(1usize);
 let b=vec_push<Row>(a,Row{{title:"held",scalar:7,payload:bytes_zeroed(3usize)}});
 let c=vec_update_field<Row>(b,{index},{value},"scalar");
 if vec_len<Row>(c)==1usize {{29}}else{{0}}
}}
"#,
        declaration("i64")
    )
}
pub(crate) fn evaluation_order_source() -> String {
    format!(
        r#"{}@id("owned.leaf.main") fn main()->i64 {{
 let a=vec_with_capacity<Row>(2usize);
 let b=vec_push<Row>(a,Row{{title:"first",scalar:7,payload:bytes_zeroed(1usize)}});
 let c=vec_push<Row>(b,Row{{title:"second",scalar:7,payload:bytes_zeroed(1usize)}});
 let mut index=0usize;
 let rows=vec_update_field<Row>(c,index,{{index=1usize;9}},"scalar");
 if index==1usize && vec_field<Row>(rows,0usize,"scalar")==9 && vec_field<Row>(rows,1usize,"scalar")==7 {{29}}else{{0}}
}}
"#,
        declaration("i64")
    )
}
pub(crate) fn empty_failure() -> String {
    format!(
        r#"{}@id("owned.leaf.main") fn main()->i64 {{
 let empty=vec_with_capacity<Row>(0usize);
 let rows=vec_update_field<Row>(empty,0usize,9,"scalar");
 if vec_len<Row>(rows)==0usize {{29}}else{{0}}
}}
"#,
        declaration("i64")
    )
}
pub(crate) const OLD_ROW_ORACLE: &str = r#"@id("field.before")
fn before(rows:own Vec<Row>,index:usize,value:i64)->Vec<Row> {
 let old=vec_clone_at<Row>(rows,index);
 let replacement=Row{title:string_from_str(string_as_str(old.title)),scalar:value,payload:bytes_copy(bytes_as_slice(old.payload))};
 vec_replace<Row>(rows,index,replacement)
}
"#;
pub(crate) fn oracle_source() -> String {
    format!(
        r#"{}{OLD_ROW_ORACLE}
@id("owned.leaf.main") fn main()->i64 {{
 let a=vec_with_capacity<Row>(1usize);
 let b=vec_push<Row>(a,Row{{title:"a\u{{0}}é",scalar:7,payload:bytes_zeroed(3usize)}});
 let rows=before(b,0usize,9);
 let text=vec_field<Row>(rows,0usize,"title");
 let payload=vec_field<Row>(rows,0usize,"payload");
 if vec_field<Row>(rows,0usize,"scalar")==9 && str_len_bytes(text)==4 && byte_len(payload)==3usize {{29}}else{{0}}
}}
"#,
        declaration("i64")
    )
}

pub(crate) fn assert_sources() {
    use sha2::{Digest, Sha256};
    let packet: serde_json::Value = serde_json::from_str(include_str!(
        "../../docs/evidence/record-copy-field-update-836-backends/source-inverses.json"
    ))
    .unwrap();
    assert_eq!(packet["owners"].as_array().unwrap().len(), 13);
    assert_eq!(packet["children"].as_array().unwrap().len(), 6);
    for row in packet["owners"].as_array().unwrap() {
        let (mut current, original) = match row["source"].as_str().unwrap() {
            "src/codegen/native_emit/expression/vec_ops.rs" => (include_str!("../../src/codegen/native_emit/expression/vec_ops.rs").to_owned(), include_str!("../../docs/evidence/record-copy-field-update-836-backends/original/src__codegen__native_emit__expression__vec_ops.rs")),
            "src/codegen/native_emit/expression/vec_ops/owned_payload.rs" => (include_str!("../../src/codegen/native_emit/expression/vec_ops/owned_payload.rs").to_owned(), include_str!("../../docs/evidence/record-copy-field-update-836-backends/original/src__codegen__native_emit__expression__vec_ops__owned_payload.rs")),
            "src/codegen/native_emit/expression/vec_ops/record_payload.rs" => (include_str!("../../src/codegen/native_emit/expression/vec_ops/record_payload.rs").to_owned(), include_str!("../../docs/evidence/record-copy-field-update-836-backends/original/src__codegen__native_emit__expression__vec_ops__record_payload.rs")),
            "src/codegen/native_vec.rs" => (include_str!("../../src/codegen/native_vec.rs").to_owned(), include_str!("../../docs/evidence/record-copy-field-update-836-backends/original/src__codegen__native_vec.rs")),
            "src/codegen/native_vec/owned_leaf.rs" => (include_str!("../../src/codegen/native_vec/owned_leaf.rs").to_owned(), include_str!("../../docs/evidence/record-copy-field-update-836-backends/original/src__codegen__native_vec__owned_leaf.rs")),
            "src/wasm/aggregate.rs" => (include_str!("../../src/wasm/aggregate.rs").to_owned(), include_str!("../../docs/evidence/record-copy-field-update-836-backends/original/src__wasm__aggregate.rs")),
            "src/wasm/aggregate/vec_field.rs" => (include_str!("../../src/wasm/aggregate/vec_field.rs").to_owned(), include_str!("../../docs/evidence/record-copy-field-update-836-backends/original/src__wasm__aggregate__vec_field.rs")),
            "src/wasm/aggregate/vec_owned_leaf.rs" => (include_str!("../../src/wasm/aggregate/vec_owned_leaf.rs").to_owned(), include_str!("../../docs/evidence/record-copy-field-update-836-backends/original/src__wasm__aggregate__vec_owned_leaf.rs")),
            "src/wasm/aggregate/vec_owned_payload.rs" => (include_str!("../../src/wasm/aggregate/vec_owned_payload.rs").to_owned(), include_str!("../../docs/evidence/record-copy-field-update-836-backends/original/src__wasm__aggregate__vec_owned_payload.rs")),
            "src/wasm/aggregate/vec_record_payload.rs" => (include_str!("../../src/wasm/aggregate/vec_record_payload.rs").to_owned(), include_str!("../../docs/evidence/record-copy-field-update-836-backends/original/src__wasm__aggregate__vec_record_payload.rs")),
            "tests/cleanup_backends/owned_leaf_collections.rs" => (include_str!("../../tests/cleanup_backends/owned_leaf_collections.rs").to_owned(), include_str!("../../docs/evidence/record-copy-field-update-836-backends/original/tests__cleanup_backends__owned_leaf_collections.rs")),
            "tests/owned_data/owned_leaf_vec.rs" => (include_str!("../../tests/owned_data/owned_leaf_vec.rs").to_owned(), include_str!("../../docs/evidence/record-copy-field-update-836-backends/original/tests__owned_data__owned_leaf_vec.rs")),
            "tests/owned_data/owned_leaf_vec/host.js" => (include_str!("../../tests/owned_data/owned_leaf_vec/host.js").to_owned(), include_str!("../../docs/evidence/record-copy-field-update-836-backends/original/tests__owned_data__owned_leaf_vec__host.js")),
            _ => panic!("unexpected backend owner"),
        };
        for edit in row["edits"].as_array().unwrap() {
            let new = edit["new"].as_str().unwrap();
            assert_eq!(current.matches(new).count(), 1);
            current = current.replacen(new, edit["before"].as_str().unwrap(), 1);
        }
        assert_eq!(current, original);
    }
    for (path, bytes) in [
        (
            "src/codegen/native_emit/expression/vec_ops/field_update.rs",
            include_str!("../../src/codegen/native_emit/expression/vec_ops/field_update.rs"),
        ),
        (
            "src/codegen/native_vec/field_update.c",
            include_str!("../../src/codegen/native_vec/field_update.c"),
        ),
        (
            "src/wasm/aggregate/vec_owned_leaf/field_update.rs",
            include_str!("../../src/wasm/aggregate/vec_owned_leaf/field_update.rs"),
        ),
        (
            "tests/support/record_copy_field_update.rs",
            include_str!("../../tests/support/record_copy_field_update.rs"),
        ),
        (
            "tests/cleanup_backends/owned_leaf_collections/copy_field_update.rs",
            include_str!(
                "../../tests/cleanup_backends/owned_leaf_collections/copy_field_update.rs"
            ),
        ),
        (
            "tests/owned_data/owned_leaf_vec/copy_field_update.rs",
            include_str!("../../tests/owned_data/owned_leaf_vec/copy_field_update.rs"),
        ),
    ] {
        let row = packet["children"]
            .as_array()
            .unwrap()
            .iter()
            .find(|r| r["source"] == path)
            .unwrap();
        assert_eq!(
            format!("{:x}", Sha256::digest(bytes.as_bytes())),
            row["sha256"].as_str().unwrap()
        );
    }
}
