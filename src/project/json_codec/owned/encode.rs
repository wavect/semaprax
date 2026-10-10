//! Deterministic ordinary-source encoding of independently owned request values.
use super::*;

pub(super) fn source(
    root: &TypeDeclaration,
    row: &TypeDeclaration,
    string: &FieldDeclaration,
) -> String {
    let name = &root.name;
    let id = &root.stable_id;
    let row_name = &row.name;
    let row_id = &row.stable_id;
    let key = &string.name;
    let fs = fields(row);
    let mut out = format!("@id(\"{row_id}.json.owned-valid\")
fn json_{row_name}_owned_valid(text:borrow str)->bool {{
let length=str_len_bytes(text);let mut valid=length>=1 && length<=16;let mut at=0usize;
while valid && at<usize_from_i64(length) {{
let byte=match str_byte_at(text,at){{Option::Some{{value}}=>i64_from_u8(value),Option::None{{}}=>-1,}};
valid=byte>=65 && byte<=90 || byte>=97 && byte<=122 || byte>=48 && byte<=57 || byte==95 || byte==45;
at=at+1usize;valid && at<usize_from_i64(length)
}}
valid
}}
@id(\"{row_id}.json.owned-row-len\")
fn json_{row_name}_owned_row_len(value:borrow {row_name})->usize {{
if !json_{row_name}_owned_valid(string_as_str(value.{key})) {{18446744073709551615usize}} else {{\n");
    let punctuation = 2 + fs.iter().map(|f| f.name.len() + 3).sum::<usize>() + fs.len() - 1;
    write!(out, "{punctuation}usize").unwrap();
    for field in fs {
        let value = format!("value.{}", field.name);
        let length = match field.ty {
            Type::String => format!("2usize+usize_from_i64(string_len({value}))"),
            Type::Bool => format!("if {value}{{4usize}}else{{5usize}}"),
            Type::Usize => format!("jv_usize_len({value})"),
            Type::U8 => format!("usize_from_i64(jv_i64_len(i64_from_u8({value})))"),
            _ => format!("usize_from_i64(jv_i64_len({value}))"),
        };
        write!(out, "+({length})").unwrap();
    }
    writeln!(
        out,
        "\n}}}}
@id(\"{row_id}.json.owned-row-render\")
fn json_{row_name}_owned_row_render(value:own {row_name})->string {{
if !json_{row_name}_owned_valid(string_as_str(value.{key})) {{\"\"}} else {{
match own value {{{row_name}{{"
    )
    .unwrap();
    for (index, field) in fs.iter().enumerate() {
        writeln!(out, "{}:value_{index},", field.name).unwrap();
    }
    out.push_str("}=>{\nlet output_0=\"{\";\n");
    for (i, field) in fs.iter().enumerate() {
        let value = format!("value_{i}");
        let rendered = match field.ty {
            Type::String => format!("string_concat(string_concat(\"\\\"\",{value}),\"\\\"\")"),
            Type::Bool => format!("if {value}{{\"true\"}}else{{\"false\"}}"),
            Type::Usize => format!("string_from_usize({value})"),
            Type::U8 => format!("string_from_i64(i64_from_u8({value}))"),
            _ => format!("string_from_i64({value})"),
        };
        let label = literal(&format!(
            "{}\"{}\":",
            if i == 0 { "" } else { "," },
            field.name
        ));
        writeln!(out, "let label_{i}=string_concat(output_{i},{label});let rendered_{i}={rendered};let output_{}=string_concat(label_{i},rendered_{i});", i + 1).unwrap();
    }
    writeln!(
        out,
        "string_concat(output_{},\"}}\")\n}},}}\n}}}}",
        fs.len()
    )
    .unwrap();
    let root_fields = fields(root);
    let punctuation = root_fields[0].name.len() + root_fields[1].name.len() + 13;
    writeln!(out, "@id(\"{id}.json.owned.encoded-len\")
fn json_{name}_owned_encoded_len(words:borrow Vec<string>,rows:borrow Vec<{row_name}>)->usize {{
let count=vec_len<string>(words);let row_count=vec_len<{row_name}>(rows);
let mut valid=count<=8usize && row_count<=256usize && (count>0usize || row_count==0usize);
let mut total={punctuation}usize;let mut index=0usize;
while valid && index<count {{
let word=vec_clone_at<string>(words,index);valid=json_{row_name}_owned_valid(string_as_str(word));
let mut prior=0usize;
while valid && prior<index {{let other=vec_clone_at<string>(words,prior);valid=word!=other;prior=prior+1usize;valid && prior<index}}
let _ = if valid {{total=total+usize_from_i64(string_len(word))+2usize+(if index>0usize{{1usize}}else{{0usize}});true}}else{{false}};
index=index+1usize;valid && index<count
}}
let mut at=0usize;
while valid && at<row_count {{
let row=vec_clone_at<{row_name}>(rows,at);let size=json_{row_name}_owned_row_len(row);
let mut prior=0usize;
while valid && prior<at {{let other=vec_clone_at<{row_name}>(rows,prior);valid=string_compare(row.{key},other.{key})!=0;prior=prior+1usize;valid && prior<at}}
let comma=if at>0usize{{1usize}}else{{0usize}};
let _ = if !valid || size==18446744073709551615usize || total>131072usize-comma || size>131072usize-total-comma {{valid=false;false}}else{{total=total+size+comma;true}};
at=at+1usize;valid && at<row_count
}}
if valid{{total}}else{{18446744073709551615usize}}
}}
@id(\"{id}.json.owned.encode\")
fn json_{name}_owned_encode(words:borrow Vec<string>,rows:borrow Vec<{row_name}>,output_limit:usize)->{row_name}JsonViewEncode {{
let required=json_{name}_owned_encoded_len(words,rows);
if required==18446744073709551615usize || required>output_limit {{{row_name}JsonViewEncode::Refused{{required:required}}}}else{{
let mut text={};let mut index=0usize;
while index<vec_len<string>(words) {{
let _ = if index>0usize{{text=string_concat(text,\",\");true}}else{{true}};
let word=vec_clone_at<string>(words,index);
text=string_concat(text,string_concat(string_concat(\"\\\"\",word),\"\\\"\"));
index=index+1usize;index<vec_len<string>(words)
}}
text=string_concat(text,{});let mut at=0usize;
while at<vec_len<{row_name}>(rows) {{
let _ = if at>0usize{{text=string_concat(text,\",\");true}}else{{true}};
let row=vec_clone_at<{row_name}>(rows,at);text=string_concat(text,json_{row_name}_owned_row_render(row));
at=at+1usize;at<vec_len<{row_name}>(rows)
}}
{row_name}JsonViewEncode::Encoded{{text:string_concat(text,\"]}}\")}}
}}
}}", literal(&format!("{{\"{}\":[", root_fields[0].name)),
        literal(&format!("],\"{}\":[", root_fields[1].name))).unwrap();
    out
}
