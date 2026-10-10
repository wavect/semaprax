//! Construct only after full validation, using ordinary L2R constructors and Vec renewals.
use super::*;
use descriptor::{Kind, Record};
pub(super) fn source(root: &TypeDeclaration, record: &Record<'_>) -> String {
    let name = &root.name;
    let id = &root.stable_id;
    let mut out = String::new();
    writeln!(out,"@id(\"{id}.json.nested.member\") fn json_{name}_nested_member(input:borrow Slice<u8>,object:usize,wanted:borrow Slice<u8>)->usize {{let length=byte_len(input);let mut found=length;let mut key=jv_first_member(input,object);while found==length && key<length{{found=if jv_key_eq(input,key,wanted){{jv_member_value(input,key)}}else{{found}};key=if found==length{{jv_next_member(input,key,32usize)}}else{{length}};found==length && key<length\n}}found\n}}").unwrap();
    writeln!(out,"@id(\"{id}.json.nested.usize\") fn json_{name}_nested_usize(input:borrow Slice<u8>,start:usize)->usize {{let end=jv_decimal_end(input,start);let mut number=0usize;let mut at=start;while at<end{{let digit=match byte_get(input,at){{Option::Some{{value}}=>usize_from_u8(value-48u8),Option::None{{}}=>0usize,}};number=number*10usize+digit;at=at+1usize;at<end\n}}number\n}}").unwrap();
    writeln!(out,"@id(\"{id}.json.nested.array-length\") fn json_{name}_nested_array_length(input:borrow Slice<u8>,start:usize)->usize {{let length=byte_len(input);let mut count=0usize;let mut cursor=jv_first_element(input,start);while cursor<length{{count=count+1usize;cursor=jv_next_element(input,cursor,32usize);cursor<length\n}}count\n}}").unwrap();
    out.push_str(&build(root, record));
    out
}
fn build(root: &TypeDeclaration, record: &Record<'_>) -> String {
    let mut out = String::new();
    let name = &root.name;
    let id = &root.stable_id;
    let ordinal = record.ordinal;
    for field in &record.fields {
        children(&mut out, root, &field.kind, field.ordinal);
    }
    writeln!(out,"@id(\"{id}.json.nested.build.{ordinal}\") fn json_{name}_nested_build_{ordinal}(input:borrow Slice<u8>,object:usize)->{} {{",record.declaration.name).unwrap();
    for field in &record.fields {
        let n = field.ordinal;
        writeln!(out,"let key_{n}={};let start_{n}=json_{name}_nested_member(input,object,array_as_slice(key_{n}));",key_array(&field.declaration.name)).unwrap();
    }
    writeln!(out, "{}{{", record.declaration.name).unwrap();
    for field in &record.fields {
        writeln!(
            out,
            "{}:{},",
            field.declaration.name,
            value(
                root,
                &field.kind,
                field.ordinal,
                &format!("start_{}", field.ordinal)
            )
        )
        .unwrap();
    }
    out.push_str("}\n}\n");
    out
}
fn children(out: &mut String, root: &TypeDeclaration, kind: &Kind<'_>, ordinal: usize) {
    match kind {
        Kind::Record(record) => out.push_str(&build(root, record)),
        Kind::Vector { element, kind } => {
            children(out, root, kind, ordinal);
            let name = &root.name;
            let id = &root.stable_id;
            let ty = type_name(element);
            writeln!(out,"@id(\"{id}.json.nested.array-build.{ordinal}\") fn json_{name}_nested_array_build_{ordinal}(input:borrow Slice<u8>,start:usize)->Vec<{ty}> {{let length=byte_len(input);let count=json_{name}_nested_array_length(input,start);let mut values=vec_with_capacity<{ty}>(count);let mut cursor=jv_first_element(input,start);while cursor<length{{let value={};values=vec_push<{ty}>(values,value);cursor=jv_next_element(input,cursor,32usize);cursor<length\n}}values\n}}",value(root,kind,ordinal,"cursor")).unwrap();
        }
        _ => {}
    }
}
fn value(root: &TypeDeclaration, kind: &Kind<'_>, ordinal: usize, start: &str) -> String {
    let name = &root.name;
    match kind {
        Kind::Text => {
            format!("json_{name}_nested_text(input,{start},jv_scan_string(input,{start},false))")
        }
        Kind::Record(record) => {
            format!("json_{name}_nested_build_{}(input,{start})", record.ordinal)
        }
        Kind::Vector { .. } => format!("json_{name}_nested_array_build_{ordinal}(input,{start})"),
        Kind::Scalar(Type::Bool) => format!("jv_kind(input,{start})==5"),
        Kind::Scalar(Type::U8) => format!("u8_from_i64(jv_i64_or(input,{start},0))"),
        Kind::Scalar(Type::Usize) => format!("json_{name}_nested_usize(input,{start})"),
        Kind::Scalar(Type::I64) => format!("jv_i64_or(input,{start},0)"),
        Kind::Scalar(_) => unreachable!("validated JSON scalar"),
    }
}
