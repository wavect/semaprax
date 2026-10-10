//! Explicit bounded UTF-8 owned request policy; no identifier domain or authority.
mod encode;
mod request;
#[cfg(test)]
mod tests;
mod text;

use crate::ast::{FieldDeclaration, Program, Type, TypeDeclaration, TypeDeclarationKind};
use crate::diagnostic::Diagnostic;
use std::fmt::Write as _;

fn fields(record: &TypeDeclaration) -> &[FieldDeclaration] {
    match &record.kind {
        TypeDeclarationKind::Record { fields } => fields,
        _ => unreachable!(),
    }
}
fn literal(value: &str) -> String {
    format!("\"{}\"", value.replace('\\', "\\\\").replace('\"', "\\\""))
}
fn key_array(value: &str) -> String {
    format!(
        "[{}]",
        value
            .bytes()
            .map(|byte| format!("{byte}u8"))
            .collect::<Vec<_>>()
            .join(",")
    )
}

pub(super) fn stream_source(
    program: &Program,
    root: &TypeDeclaration,
    max_string_bytes: usize,
) -> Result<String, Vec<Diagnostic>> {
    // The unchanged normalizer selects raw grammar faults; the decoder selects
    // schema faults in the normalized immutable bytes supplied by its caller.
    let normalization = super::views::stream_normalizer_source(program, root)?;
    let mut result = source(program, root, max_string_bytes)?;
    result.push_str(&normalization);
    Ok(result)
}

pub(super) fn source(
    program: &Program,
    root: &TypeDeclaration,
    max_string_bytes: usize,
) -> Result<String, Vec<Diagnostic>> {
    // This successor uses plain bounded UTF-8 Strings, not identifier domains.
    // The schema-derived worst-case escaped storage bound stays fixed;
    // no name grants authority or inherits an application identifier domain.
    if !(1..=64).contains(&max_string_bytes) {
        return Err(super::refusal(
            "UTF-8 owned request requires max_string_bytes in 1..=64",
        ));
    }
    let mut out = request::source(program, root, max_string_bytes)?;
    let root_fields = fields(root);
    let Type::Named { arguments, .. } = &root_fields[1].ty else {
        unreachable!()
    };
    let Type::Named { name: row_name, .. } = &arguments[0] else {
        unreachable!()
    };
    let row = program
        .types
        .iter()
        .find(|ty| &ty.name == row_name)
        .expect("validated row");
    let string = fields(row)
        .iter()
        .find(|field| field.ty == Type::String)
        .expect("validated String field");
    let name = &root.name;
    let id = &root.stable_id;
    let row_name = &row.name;
    let row_id = &row.stable_id;
    let first = &root_fields[0].name;
    let second = &root_fields[1].name;
    writeln!(
        out,
        "@id(\"{id}.json.utf8.owned-result\") variant {name}JsonUtf8OwnedDecode {{
@id(\"{id}.json.utf8.owned-ready\") Decoded {{
@id(\"{id}.json.utf8.owned-first\") {first}:Vec<string>,
@id(\"{id}.json.utf8.owned-second\") {second}:Vec<{row_name}>,
}},
@id(\"{id}.json.utf8.owned-error\") Error {{
@id(\"{id}.json.utf8.owned-code\") code:i64,
@id(\"{id}.json.utf8.owned-offset\") offset:usize,
@id(\"{id}.json.utf8.owned-field\") field:i64,
}},
}}"
    )
    .unwrap();
    writeln!(
        out,
        "@id(\"{row_id}.json.utf8.owned-materialize\")
fn json_{row_name}_utf8_materialize(input:borrow Slice<u8>,value:{row_name}JsonView)->{row_name} {{
{row_name} {{"
    )
    .unwrap();
    for field in fields(row) {
        if field.ty == Type::String {
            writeln!(
                out,
                "{}:json_{row_name}_utf8_text(input,value.{}_start,value.{}_end),",
                field.name, field.name, field.name
            )
            .unwrap();
        } else {
            writeln!(out, "{}:value.{},", field.name, field.name).unwrap();
        }
    }
    writeln!(out, "}}
}}
@id(\"{id}.json.utf8.owned.decode\")
fn json_{name}_utf8_owned_decode(input:borrow Slice<u8>)->{name}JsonUtf8OwnedDecode {{
let parsed=json_{name}_request_decode(input);
match own parsed {{
{name}JsonRequestDecode::Error{{code,offset,field}}=>{name}JsonUtf8OwnedDecode::Error{{code:code,offset:offset,field:field}},
{name}JsonRequestDecode::Decoded{{servers:spans,patients:views}}=>{{
let mut words=vec_with_capacity<string>(vec_len<{name}JsonIdentifierSpan>(spans));
let mut rows=vec_with_capacity<{row_name}>(vec_len<{row_name}JsonView>(views));
let mut index=0usize;
while index<vec_len<{name}JsonIdentifierSpan>(spans) {{
let span=vec_get<{name}JsonIdentifierSpan>(spans,index);
let word=json_{row_name}_utf8_text(input,span.start,span.end);
words=vec_push<string>(words,word);
index=index+1usize;index<vec_len<{name}JsonIdentifierSpan>(spans)
}}
let mut at=0usize;
while at<vec_len<{row_name}JsonView>(views) {{
let view=vec_get<{row_name}JsonView>(views,at);
let row=json_{row_name}_utf8_materialize(input,view);
rows=vec_push<{row_name}>(rows,row);
at=at+1usize;at<vec_len<{row_name}JsonView>(views)
}}
{name}JsonUtf8OwnedDecode::Decoded{{{first}:words,{second}:rows}}
}},
}}
}}").unwrap();
    out.push_str(&encode::source(root, row, string));
    Ok(out)
}
