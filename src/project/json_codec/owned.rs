//! Owned request materialization over the independently checked identifier policy.
//! No generated declaration is privileged at source, HIR, profile or backend gates.
mod encode;
#[cfg(test)]
mod tests;

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
    format!("\"{}\"", value.replace('\\', "\\\\").replace('"', "\\\""))
}

pub(super) fn source(
    program: &Program,
    root: &TypeDeclaration,
    stream: bool,
) -> Result<String, Vec<Diagnostic>> {
    // This closed selector inherits the exact existing wire policy and its
    // schema-derived normalized storage bound; no benchmark-name exception.
    let mut out = if stream {
        super::views::stream_request_source(program, root)?
    } else {
        super::views::request_source(program, root)?
    };
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
        .expect("validated identifier");
    let name = &root.name;
    let id = &root.stable_id;
    let row_name = &row.name;
    let row_id = &row.stable_id;
    let first = &root_fields[0].name;
    let second = &root_fields[1].name;
    writeln!(
        out,
        "@id(\"{id}.json.owned-result\") variant {name}JsonOwnedDecode {{
@id(\"{id}.json.owned-ready\") Decoded {{
@id(\"{id}.json.owned-first\") {first}:Vec<string>,
@id(\"{id}.json.owned-second\") {second}:Vec<{row_name}>,
}},
@id(\"{id}.json.owned-error\") Error {{
@id(\"{id}.json.owned-code\") code:i64,
@id(\"{id}.json.owned-offset\") offset:usize,
@id(\"{id}.json.owned-field\") field:i64,
}},
}}"
    )
    .unwrap();
    // Revalidate even the private materializer: plain offsets never become
    // authenticated views merely by passing through a generated helper.
    writeln!(
        out,
        "@id(\"{row_id}.json.owned-identifier\")
fn json_{row_name}_owned_identifier(input:borrow Slice<u8>,start:usize,end:usize)->string {{
if !json_{row_name}_identifier_valid(input,start,end) {{\"\"}} else {{
let mut text=\"\";let mut cursor=start+1usize;
while cursor<end-1usize {{
let byte=u8_from_i64(jv_emit_at(input,cursor,0usize));
text=string_concat(text,string_from_char(char_from_u8(byte)));
cursor=jv_token_end(input,cursor);cursor<end-1usize
}}
text
}}
}}
@id(\"{row_id}.json.owned-materialize\")
fn json_{row_name}_owned_materialize(input:borrow Slice<u8>,value:{row_name}JsonView)->{row_name} {{
{row_name} {{"
    )
    .unwrap();
    for field in fields(row) {
        if field.ty == Type::String {
            writeln!(
                out,
                "{}:json_{row_name}_owned_identifier(input,value.{}_start,value.{}_end),",
                field.name, field.name, field.name
            )
            .unwrap();
        } else {
            writeln!(out, "{}:value.{},", field.name, field.name).unwrap();
        }
    }
    writeln!(out, "}}
}}
@id(\"{id}.json.owned.decode\")
fn json_{name}_owned_decode(input:borrow Slice<u8>)->{name}JsonOwnedDecode {{
let parsed=json_{name}_request_decode(input);
match own parsed {{
{name}JsonRequestDecode::Error{{code,offset,field}}=>{name}JsonOwnedDecode::Error{{code:code,offset:offset,field:field}},
{name}JsonRequestDecode::Decoded{{servers:spans,patients:views}}=>{{
let mut words=vec_with_capacity<string>(vec_len<{name}JsonIdentifierSpan>(spans));
let mut rows=vec_with_capacity<{row_name}>(vec_len<{row_name}JsonView>(views));
let mut index=0usize;
while index<vec_len<{name}JsonIdentifierSpan>(spans) {{
let span=vec_get<{name}JsonIdentifierSpan>(spans,index);
let word=json_{row_name}_owned_identifier(input,span.start,span.end);
words=vec_push<string>(words,word);
index=index+1usize;index<vec_len<{name}JsonIdentifierSpan>(spans)
}}
let mut at=0usize;
while at<vec_len<{row_name}JsonView>(views) {{
let view=vec_get<{row_name}JsonView>(views,at);
let row=json_{row_name}_owned_materialize(input,view);
rows=vec_push<{row_name}>(rows,row);
at=at+1usize;at<vec_len<{row_name}JsonView>(views)
}}
{name}JsonOwnedDecode::Decoded{{{first}:words,{second}:rows}}
}},
}}
}}").unwrap();
    out.push_str(&encode::source(root, row, string));
    Ok(out)
}
