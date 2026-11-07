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
// The original projected-Bytes spelling above remains a refusal control.
// Clone the same row, then transfer its complete fields into named owners
// before copying the same title and payload into the replacement row.
const ADMITTED_ROW_ORACLE: &str = r#"@id("field.before")
fn before(rows:own Vec<Row>,index:usize,value:i64)->Vec<Row> {
 let old=vec_clone_at<Row>(rows,index);
 match own old { Row{title,scalar,payload}=>{
  let replacement=Row{title:string_from_str(string_as_str(title)),scalar:value,payload:bytes_copy(bytes_as_slice(payload))};
  vec_replace<Row>(rows,index,replacement)
 }, }
}
"#;
pub(crate) fn rejected_oracle_source() -> String {
    oracle_source().replacen(ADMITTED_ROW_ORACLE, OLD_ROW_ORACLE, 1)
}
pub(crate) fn oracle_source() -> String {
    format!(
        r#"{}{ADMITTED_ROW_ORACLE}
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

/// Extra scalar lanes exist only in the additive corpus; the original legacy
/// three-field fixtures and the complete clone/replace oracle remain exact.
pub(crate) fn multi_source(ty: &str, literal: &str, expected: &str) -> String {
    let mut text = source(ty, literal, expected);
    text=text.replace(" @id(\"field.row.payload\") payload:Bytes,", " @id(\"field.row.payload\") payload:Bytes,\n @id(\"field.row.marker\") marker:i64,\n @id(\"field.row.minus\") minus:f32,\n @id(\"field.row.nan\") nan:f64,");
    for size in [1, 3] {
        text=text.replace(&format!("payload:bytes_zeroed({size}usize)}}"), &format!("payload:bytes_zeroed({size}usize),marker:-9223372036854775807-1,minus:-0.0f32,nan:0.0/0.0}}"));
    }
    let checks="vec_field<Row>(rows,0usize,\"marker\")==-9223372036854775807-1 && vec_field<Row>(rows,1usize,\"marker\")==-9223372036854775807-1 && 1.0f32/vec_field<Row>(rows,0usize,\"minus\")<0.0f32 && 1.0f32/vec_field<Row>(rows,1usize,\"minus\")<0.0f32 && vec_field<Row>(rows,0usize,\"nan\")!=vec_field<Row>(rows,0usize,\"nan\") && vec_field<Row>(rows,1usize,\"nan\")!=vec_field<Row>(rows,1usize,\"nan\")";
    text = text.replace(
        &format!(" if {expected} &&"),
        &format!(" if {checks} && {expected} &&"),
    );
    text
}
/// An ordinary source owner produces real payloads/Vec authority for trusted
/// runtime word probes. No nonexistent source bitcast intrinsic is introduced.
pub(crate) fn boundary_source(ty: &str) -> String {
    let source = multi_source(ty, if ty == "f32" { "1.0f32" } else { "1.0" }, "got==got");
    let split = source.find("@id(\"owned.leaf.main\")").unwrap();
    let head = &source[..split];
    format!(
        r#"{head}
@id("field.seed") fn seed()->Vec<Row> {{
 let empty=vec_with_capacity<Row>(1usize);
 vec_push<Row>(empty,Row{{title:"held",scalar:{},payload:bytes_zeroed(3usize),marker:-9223372036854775807-1,minus:-0.0f32,nan:0.0/0.0}})
}}
@id("owned.leaf.main") fn main()->i64 {{let a=seed();let b=vec_update_field<Row>(a,0usize,{},"scalar");if vec_len<Row>(b)==1usize {{29}}else{{0}}}}
"#,
        if ty == "f32" { "1.0f32" } else { "1.0" },
        if ty == "f32" { "2.0f32" } else { "2.0" }
    )
}

pub(crate) fn assert_sources() {
    assert_coverage_sources();
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
        current = source_input("backends", row["source"].as_str().unwrap(), &current);
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
        let bytes = source_input("backends", path, bytes);
        assert_eq!(
            format!(
                "{:x}",
                semaprax::digest_hex::LowerHex(Sha256::digest(bytes.as_bytes()))
            ),
            row["sha256"].as_str().unwrap()
        );
    }
}

fn assert_coverage_sources() {
    use sha2::{Digest, Sha256};
    let packet: serde_json::Value = serde_json::from_str(include_str!(
        "../../docs/evidence/record-copy-field-update-836-backend-controls/source-inverses.json"
    ))
    .unwrap();
    assert_eq!(packet["owners"].as_array().unwrap().len(), 4);
    assert_eq!(packet["children"].as_array().unwrap().len(), 2);
    for row in packet["owners"].as_array().unwrap() {
        let (mut current,original)=match row["source"].as_str().unwrap(){
            "tests/support/record_copy_field_update.rs" => (include_str!("../../tests/support/record_copy_field_update.rs").to_owned(), include_str!("../../docs/evidence/record-copy-field-update-836-backend-controls/original/tests__support__record_copy_field_update.rs")),
            "tests/cleanup_backends/owned_leaf_collections/copy_field_update.rs" => (include_str!("../../tests/cleanup_backends/owned_leaf_collections/copy_field_update.rs").to_owned(), include_str!("../../docs/evidence/record-copy-field-update-836-backend-controls/original/tests__cleanup_backends__owned_leaf_collections__copy_field_update.rs")),
            "tests/owned_data/owned_leaf_vec/copy_field_update.rs" => (include_str!("../../tests/owned_data/owned_leaf_vec/copy_field_update.rs").to_owned(), include_str!("../../docs/evidence/record-copy-field-update-836-backend-controls/original/tests__owned_data__owned_leaf_vec__copy_field_update.rs")),
            "tests/owned_data/owned_leaf_vec/host.js" => (include_str!("../../tests/owned_data/owned_leaf_vec/host.js").to_owned(), include_str!("../../docs/evidence/record-copy-field-update-836-backend-controls/original/tests__owned_data__owned_leaf_vec__host.js")),
            _=>panic!("unexpected coverage owner"),
        };
        current = source_input("coverage", row["source"].as_str().unwrap(), &current);
        for edit in row["edits"].as_array().unwrap() {
            let new = edit["new"].as_str().unwrap();
            assert_eq!(current.matches(new).count(), 1);
            current = current.replacen(new, edit["before"].as_str().unwrap(), 1);
        }
        assert_eq!(current, original);
    }
    for (path, bytes) in [
        (
            "tests/cleanup_backends/owned_leaf_collections/copy_field_update_boundary.rs",
            include_str!(
                "../../tests/cleanup_backends/owned_leaf_collections/copy_field_update_boundary.rs"
            ),
        ),
        (
            "tests/cleanup_backends/owned_leaf_collections/copy_field_update_boundary.c",
            include_str!(
                "../../tests/cleanup_backends/owned_leaf_collections/copy_field_update_boundary.c"
            ),
        ),
    ] {
        let row = packet["children"]
            .as_array()
            .unwrap()
            .iter()
            .find(|r| r["source"] == path)
            .unwrap();
        let bytes = source_input("coverage", path, bytes);
        assert_eq!(
            format!(
                "{:x}",
                semaprax::digest_hex::LowerHex(Sha256::digest(bytes.as_bytes()))
            ),
            row["sha256"].as_str().unwrap()
        );
    }
}

// Each original full-source table retains its own authenticated physical domain.
fn source_input(domain: &str, source: &str, current: &str) -> String {
    use sha2::{Digest, Sha256};
    let current_stage;
    let current = if domain == "fixture-current" {
        let data: serde_json::Value = serde_json::from_str(include_str!(
            "../../docs/evidence/ci-d6f-followups-20261019/record-inputs.json"
        ))
        .unwrap();
        if let Some(row) = data
            .as_array()
            .unwrap()
            .iter()
            .find(|row| row["source"] == source)
        {
            assert_eq!(
                format!(
                    "{:x}",
                    semaprax::digest_hex::LowerHex(Sha256::digest(current.as_bytes()))
                ),
                row["current_sha256"],
                "whole current record input: {source}"
            );
            let mut restored = current.to_owned();
            for edit in row["edits"].as_array().unwrap() {
                let start = edit["start"].as_u64().unwrap() as usize;
                let new = edit["new"].as_str().unwrap();
                assert_eq!(
                    &restored[start..start + new.len()],
                    new,
                    "exact record input inverse"
                );
                restored.replace_range(start..start + new.len(), edit["old"].as_str().unwrap());
            }
            let before = match source {
                "tests/support/record_copy_field_update.rs" => include_str!("../../docs/evidence/ci-d6f-followups-20261019/whole/212604e4b3eff8e25169e2c87eac39c560bd0973241d3e088a8d4ff63a1b2a60.txt"),
                "tests/cleanup_backends/owned_leaf_collections/copy_field_update.rs" => include_str!("../../docs/evidence/ci-d6f-followups-20261019/whole/b385f28f7d6862a3ca7209cd54fea521d6281ba728cb29c8e60e58c18b1b0974.txt"),
                "src/wasm/aggregate.rs" => include_str!("../../docs/evidence/ci-d6f-followups-20261019/whole/84971b8a067d718beee6a131d4b87c4c1549c0c9c12345f447592a1ef5d06c59.txt"),
                "tests/owned_data/owned_leaf_vec.rs" => include_str!("../../docs/evidence/ci-d6f-followups-20261019/whole/0d1e84bf3548f95bdce59aaa80f17bf89f295cc6dd38dd089669f756ab0d8859.txt"),
                _ => unreachable!(),
            };
            assert_eq!(restored, before, "every complete prior record input byte");
            current_stage = restored;
            current_stage.as_str()
        } else {
            current
        }
    } else {
        current
    };
    let predecessor;
    let current = if domain == "fixture-current" {
        current
    } else if domain == "assembly" {
        predecessor = source_input("fixture-current", source, current);
        predecessor.as_str()
    } else {
        predecessor = source_input("assembly", source, current);
        predecessor.as_str()
    };
    let data = if domain == "fixture-current" {
        include_str!("../../docs/evidence/record-copy-field-update-admitted-oracle-20261015/current-to-35cc.json")
    } else if domain == "assembly" {
        include_str!(
            "../../docs/evidence/ready-main-844-846-836-assembly-20261015/final-to-836.json"
        )
    } else {
        include_str!("../../docs/evidence/record-field-while-836-20261015/backend-input-views.json")
    };
    let views: serde_json::Value = serde_json::from_str(data).unwrap();
    let rows = views[domain].as_array().expect("named whole-input domain");
    let mut matching = rows.iter().filter(|row| row["source"] == source);
    let Some(row) = matching.next() else {
        return current.to_owned();
    };
    assert!(matching.next().is_none(), "unique whole-input view");
    assert_eq!(
        format!(
            "{:x}",
            semaprax::digest_hex::LowerHex(Sha256::digest(current.as_bytes()))
        ),
        row["current_sha256"],
        "authenticated current whole input: {source}"
    );
    let mut original = current.to_owned();
    for edit in row["edits"].as_array().unwrap() {
        let new = edit["new"].as_str().unwrap();
        assert_eq!(
            original.matches(new).count(),
            1,
            "unique complete input inverse"
        );
        original = original.replacen(new, edit["old"].as_str().unwrap(), 1);
    }
    assert_eq!(
        format!(
            "{:x}",
            semaprax::digest_hex::LowerHex(Sha256::digest(original.as_bytes()))
        ),
        row["original_sha256"],
        "authenticated original whole input: {source}"
    );
    original
}
