//! Checked ASCII identifier views. Integer spans never authenticate a source.

mod arrays;
mod request;
mod stream;
#[cfg(test)]
mod tests;

use std::fmt::Write as _;

use sha2::{Digest, Sha256};

use crate::ast::{Program, Type, TypeDeclaration, TypeDeclarationKind};
use crate::diagnostic::Diagnostic;

use super::{refusal, validate_record};

pub(super) fn request_source(
    program: &Program,
    root: &TypeDeclaration,
) -> Result<String, Vec<Diagnostic>> {
    request::source(program, root)
}

pub(super) fn stream_request_source(
    program: &Program,
    root: &TypeDeclaration,
) -> Result<String, Vec<Diagnostic>> {
    if !program
        .permits
        .iter()
        .any(|permit| permit == "process.stdin.read")
    {
        return Err(refusal("stream request derivation requires the original schema module's explicit process.stdin.read permit"));
    }
    let mut source = request::source(program, root)?;
    source.push_str(&stream::source(root));
    Ok(source)
}

/// Reuse the exact incremental grammar template without selecting identifier policy.
pub(super) fn stream_normalizer_source(
    program: &Program,
    root: &TypeDeclaration,
) -> Result<String, Vec<Diagnostic>> {
    if !program
        .permits
        .iter()
        .any(|permit| permit == "process.stdin.read")
    {
        return Err(refusal("stream request derivation requires the original schema module's explicit process.stdin.read permit"));
    }
    Ok(stream::source(root))
}

fn fields(record: &TypeDeclaration) -> &[crate::ast::FieldDeclaration] {
    match &record.kind {
        TypeDeclarationKind::Record { fields } => fields,
        _ => unreachable!(),
    }
}

pub(super) fn validate(record: &TypeDeclaration) -> Result<(), Vec<Diagnostic>> {
    let TypeDeclarationKind::Record { fields } = &record.kind else {
        return Err(refusal("identifier view root must be a record"));
    };
    if fields
        .iter()
        .filter(|field| field.ty == Type::String)
        .count()
        != 1
        || fields.len() > 7
    {
        return Err(refusal(
            "identifier views require exactly one String identifier and at most six scalar fields",
        ));
    }
    let mut scalar = record.clone();
    let TypeDeclarationKind::Record { fields } = &mut scalar.kind else {
        unreachable!()
    };
    for field in fields {
        if field.ty == Type::String {
            field.ty = Type::Usize;
        }
    }
    validate_record(&scalar)
}

pub(super) fn imports(program: &Program) -> String {
    let mut out = format!("module {};\n", program.module);
    for (module, name) in [
        ("scan", "strict_end"),
        ("scan", "root"),
        ("scan", "kind"),
        ("scan", "first_member"),
        ("scan", "member_value"),
        ("scan", "next_member"),
        ("scan", "key_eq"),
        ("scan", "decimal_end"),
        ("scan", "value_end"),
        ("scan", "first_element"),
        ("scan", "next_element"),
        ("token", "integer_end"),
        ("token", "i64_or"),
        ("digits", "i64_len"),
        ("write", "usize_len"),
        ("query", "scan_string"),
        ("query", "decoded_len"),
        ("query", "emit_at"),
        ("query", "token_end"),
        ("query", "decoded_token_eq"),
    ] {
        writeln!(out, "use function @id(\"std.data.json.{module}.{name}\") from std.data.json.{module} as jv_{name};").unwrap();
    }
    out
}

fn field_id(root: &str, field: &str, suffix: &str) -> String {
    let digest = format!(
        "{:x}",
        crate::digest_hex::LowerHex(Sha256::digest(field.as_bytes()))
    );
    format!("{root}.json.view.{}.{suffix}", &digest[..16])
}

fn literal(value: &str) -> String {
    format!("\"{}\"", value.replace('\\', "\\\\").replace('"', "\\\""))
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

pub(super) fn source(
    program: &Program,
    record: &TypeDeclaration,
) -> Result<String, Vec<Diagnostic>> {
    validate(record)?;
    let mut out = imports(program);
    out.push_str(&body(record));
    out.push_str(&arrays::source(record));
    Ok(out)
}

fn body(record: &TypeDeclaration) -> String {
    body_with_text(record, None)
}

pub(super) fn body_with_text(record: &TypeDeclaration, helpers: Option<&str>) -> String {
    let name = &record.name;
    let id = &record.stable_id;
    let fs = fields(record);
    let mut out = String::new();
    writeln!(out, "@id(\"{id}.json.view\") record {name}JsonView {{").unwrap();
    for f in fs {
        if f.ty == Type::String {
            for suffix in ["start", "end"] {
                writeln!(
                    out,
                    "@id(\"{}\") {}_{suffix}:usize,",
                    field_id(id, &f.stable_id, suffix),
                    f.name
                )
                .unwrap();
            }
        } else {
            let ty = match f.ty {
                Type::I64 => "i64",
                Type::U8 => "u8",
                Type::Usize => "usize",
                Type::Bool => "bool",
                _ => unreachable!(),
            };
            writeln!(
                out,
                "@id(\"{}\") {}:{ty},",
                field_id(id, &f.stable_id, "scalar"),
                f.name
            )
            .unwrap();
        }
    }
    writeln!(out, "}}\n@id(\"{id}.json.view-decode\") variant {name}JsonViewDecode {{ @id(\"{id}.json.view-decoded\") Decoded{{@id(\"{id}.json.view-value\")value:{name}JsonView,}}, @id(\"{id}.json.view-error\") Error{{@id(\"{id}.json.view-code\")code:i64,@id(\"{id}.json.view-offset\")offset:usize,@id(\"{id}.json.view-field\")field:i64,}}, }}").unwrap();
    writeln!(out, "@id(\"{id}.json.view-encode\") variant {name}JsonViewEncode {{ @id(\"{id}.json.view-encoded\") Encoded{{@id(\"{id}.json.view-text\")text:string,}}, @id(\"{id}.json.view-refused\") Refused{{@id(\"{id}.json.view-required\")required:usize,}}, }}").unwrap();
    if let Some(helpers) = helpers {
        out.push_str(helpers);
    } else {
        // All exported helpers revalidate token extent, decoded length and alphabet.
        writeln!(out, "@id(\"{id}.json.identifier-valid\") fn json_{name}_identifier_valid(input:borrow Slice<u8>,start:usize,end:usize)->bool {{
let length=byte_len(input);
let mut valid=start<end && end<=length;
let closed=if valid {{ jv_scan_string(input,start,false) }} else {{ length+1usize }};
let total=if valid && closed==end {{ jv_decoded_len(input,start) }} else {{ 0usize }};
valid=valid && closed==end && total>=1usize && total<=16usize;
let mut cursor=if valid {{ start+1usize }} else {{ end }};
while valid && cursor<end-1usize {{
let byte=jv_emit_at(input,cursor,0usize);
valid=byte>=65 && byte<=90 || byte>=97 && byte<=122 || byte>=48 && byte<=57 || byte==95 || byte==45;
cursor=if valid {{ jv_token_end(input,cursor) }} else {{ end }};
valid && cursor<end-1usize
}}
valid
}}\n@id(\"{id}.json.identifier-render\") fn json_{name}_identifier_render(input:borrow Slice<u8>,start:usize,end:usize)->string {{
if !json_{name}_identifier_valid(input,start,end) {{ \"\" }} else {{
let mut text=\"\\\"\";
let mut cursor=start+1usize;
while cursor<end-1usize {{
let byte=u8_from_i64(jv_emit_at(input,cursor,0usize));
text=string_concat(text,string_from_char(char_from_u8(byte)));
cursor=jv_token_end(input,cursor);
cursor<end-1usize
}}
string_concat(text,\"\\\"\")
}}
}}").unwrap();
    }
    writeln!(out, "@id(\"{id}.json.view.decode\") fn json_{name}_view_decode(input:borrow Slice<u8>,input_limit:usize)->{name}JsonViewDecode {{
let length=byte_len(input); let mut error=if length>input_limit {{7}}else{{0}};let mut offset=0usize;let mut field=0;
let checked=if error==0 {{jv_strict_end(input,32usize,0)}}else{{length}};
error=if checked>length {{1}}else{{error}};offset=if checked>length {{checked-length-1usize}}else{{offset}};
let object=if error==0 {{jv_root(input)}}else{{0usize}};
let _ = if error==0 && jv_kind(input,object)!=1 {{error=5;offset=object;false}}else{{true}};").unwrap();
    for (i, f) in fs.iter().enumerate() {
        writeln!(out, "let mut seen_{i}=false;").unwrap();
        if f.ty == Type::String {
            writeln!(
                out,
                "let mut value_{i}_start=0usize;let mut value_{i}_end=0usize;"
            )
            .unwrap();
        } else {
            let zero = match f.ty {
                Type::Usize => "0usize",
                Type::U8 => "0u8",
                Type::Bool => "false",
                _ => "0",
            };
            writeln!(out, "let mut value_{i}={zero};").unwrap();
        }
    }
    for (i, f) in fs.iter().enumerate() {
        writeln!(
            out,
            "let key_{i}={};\nlet key_view_{i}=array_as_slice(key_{i});",
            key_array(&f.name)
        )
        .unwrap();
    }
    out.push_str("let mut key=if error==0{jv_first_member(input,object)}else{length};\nwhile error==0 && key<length {let start=jv_member_value(input,key);let mut selected=0;\n");
    for (i, _) in fs.iter().enumerate() {
        writeln!(
            out,
            "selected=if jv_key_eq(input,key,key_view_{i}){{{}}}else{{selected}};",
            i + 1
        )
        .unwrap();
    }
    out.push_str("let _ = if selected==0{error=4;offset=key;field=0;false}else{field=selected;\n");
    for (i, f) in fs.iter().enumerate() {
        writeln!(out,"let _ = if selected=={}{{if seen_{i}{{error=2;offset=key;false}}else{{seen_{i}=true;let kind=jv_kind(input,start);",i+1).unwrap();
        if f.ty == Type::String {
            writeln!(out,"if kind!=3{{error=5;offset=start;false}}else{{let end=jv_scan_string(input,start,false);if !json_{name}_identifier_valid(input,start,end){{error=6;offset=start;false}}else{{value_{i}_start=start;value_{i}_end=end;true}}}}").unwrap();
        } else {
            out.push_str(&scalar(i, &f.ty));
        }
        out.push_str("}}else{true};\n");
    }
    out.push_str("true};key=if error==0{jv_next_member(input,key,32usize)}else{length};error==0 && key<length\n}\n");
    for (i, _) in fs.iter().enumerate() {
        writeln!(
            out,
            "let _ = if error==0 && !seen_{i}{{error=3;offset=length;field={};false}}else{{true}};",
            i + 1
        )
        .unwrap();
    }
    write!(
        out,
        "if error==0{{{name}JsonViewDecode::Decoded{{value:{name}JsonView{{"
    )
    .unwrap();
    for (i, f) in fs.iter().enumerate() {
        if f.ty == Type::String {
            write!(
                out,
                "{}_start:value_{i}_start,{}_end:value_{i}_end,",
                f.name, f.name
            )
            .unwrap();
        } else {
            write!(out, "{}:value_{i},", f.name).unwrap();
        }
    }
    writeln!(
        out,
        "}}}}}}else{{{name}JsonViewDecode::Error{{code:error,offset:offset,field:field}}}}\n}}"
    )
    .unwrap();
    if helpers.is_none() {
        out.push_str(&encoder(record));
    }
    out
}

fn scalar(i: usize, ty: &Type) -> String {
    if *ty == Type::Bool {
        return format!(
            "if kind==5 || kind==6{{value_{i}=kind==5;true}}else{{error=5;offset=start;false}}\n"
        );
    }
    let mut out=String::from("if kind!=4{error=5;offset=start;false}else{let end=jv_decimal_end(input,start);if jv_integer_end(input,start)!=end{error=6;offset=start;false}else{\n");
    if *ty == Type::Usize {
        writeln!(out,"let negative=match byte_get(input,start){{Option::Some{{value:byte}}=>byte==45u8,Option::None{{}}=>false,}};let mut number=0usize;let mut cursor=start;
let _ = if negative{{error=6;offset=start;false}}else{{while error==0 && cursor<end{{let digit=match byte_get(input,cursor){{Option::Some{{value:byte}}=>usize_from_u8(byte-48u8),Option::None{{}}=>0usize,}};
if number>1844674407370955161usize || number==1844674407370955161usize && digit>5usize{{error=6;offset=start;false}}else{{number=number*10usize+digit;cursor=cursor+1usize;true}}
}}true}};value_{i}=if error==0{{number}}else{{value_{i}}};true").unwrap();
    } else {
        let condition = if *ty == Type::U8 {
            "number!=other || number<0 || number>255"
        } else {
            "number!=other"
        };
        let value = if *ty == Type::U8 {
            "u8_from_i64(number)"
        } else {
            "number"
        };
        writeln!(out,"let number=jv_i64_or(input,start,0);let other=jv_i64_or(input,start,1);if {condition}{{error=6;offset=start;false}}else{{value_{i}={value};true}}").unwrap();
    }
    out.push_str("}}\n");
    out
}

fn encoder(record: &TypeDeclaration) -> String {
    let name = &record.name;
    let id = &record.stable_id;
    let fs = fields(record);
    let string = fs.iter().find(|f| f.ty == Type::String).unwrap();
    let mut out=format!("@id(\"{id}.json.view.encoded-len\") fn json_{name}_view_encoded_len(input:borrow Slice<u8>,value:{name}JsonView)->usize {{\nif !json_{name}_identifier_valid(input,value.{}_start,value.{}_end){{18446744073709551615usize}}else{{\n",string.name,string.name);
    let punctuation = 2 + fs.iter().map(|f| f.name.len() + 3).sum::<usize>() + fs.len() - 1;
    write!(out, "{punctuation}usize").unwrap();
    for f in fs {
        let v = format!("value.{}", f.name);
        let n = match f.ty {
            Type::String => format!("2usize+jv_decoded_len(input,{v}_start)"),
            Type::Bool => format!("if {v}{{4usize}}else{{5usize}}"),
            Type::Usize => format!("jv_usize_len({v})"),
            Type::U8 => format!("usize_from_i64(jv_i64_len(i64_from_u8({v})))"),
            _ => format!("usize_from_i64(jv_i64_len({v}))"),
        };
        write!(out, "+({n})").unwrap();
    }
    writeln!(out,"\n}}}}\n@id(\"{id}.json.view.encode\") fn json_{name}_view_encode(input:borrow Slice<u8>,value:{name}JsonView,output_limit:usize)->{name}JsonViewEncode{{let required=json_{name}_view_encoded_len(input,value);if required==18446744073709551615usize || required>output_limit{{{name}JsonViewEncode::Refused{{required:required}}}}else{{\nlet output_0=\"{{\";").unwrap();
    for (i, f) in fs.iter().enumerate() {
        let v = format!("value.{}", f.name);
        let rendered = match f.ty {
            Type::String => format!("json_{name}_identifier_render(input,{v}_start,{v}_end)"),
            Type::Bool => format!("if {v}{{\"true\"}}else{{\"false\"}}"),
            Type::Usize => format!("string_from_usize({v})"),
            Type::U8 => format!("string_from_i64(i64_from_u8({v}))"),
            _ => format!("string_from_i64({v})"),
        };
        let label = literal(&format!("{}\"{}\":", if i == 0 { "" } else { "," }, f.name));
        writeln!(out,"let label_{i}=string_concat(output_{i},{label});let rendered_{i}={rendered};let output_{}=string_concat(label_{i},rendered_{i});",i+1).unwrap();
    }
    writeln!(
        out,
        "{name}JsonViewEncode::Encoded{{text:string_concat(output_{},\"}}\")}}}}}}",
        fs.len()
    )
    .unwrap();
    out
}
