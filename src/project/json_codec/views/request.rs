//! Two-array request views retain ordinary owner transfer and borrowed source.

use super::*;

pub(super) fn source(program: &Program, root: &TypeDeclaration) -> Result<String, Vec<Diagnostic>> {
    let TypeDeclarationKind::Record {
        fields: root_fields,
    } = &root.kind
    else {
        return Err(refusal("request view root must be a record"));
    };
    let mut scalar = root.clone();
    let TypeDeclarationKind::Record {
        fields: scalar_fields,
    } = &mut scalar.kind
    else {
        unreachable!()
    };
    for field in scalar_fields {
        field.ty = Type::Usize;
    }
    validate_record(&scalar)?;
    if root_fields.len() != 2 {
        return Err(refusal(
            "request views require exactly an identifier array and an identifier-record array",
        ));
    }
    let mut servers = None;
    let mut patients = None;
    for field in root_fields {
        let Type::Named { name, arguments } = &field.ty else {
            return Err(refusal("request fields must be Vec arrays"));
        };
        if name != "Vec" || arguments.len() != 1 {
            return Err(refusal(
                "request fields must be direct monomorphic Vec arrays",
            ));
        }
        match &arguments[0] {
            Type::String if servers.is_none() => servers = Some(field),
            Type::Named { name, arguments } if arguments.is_empty() && patients.is_none() => {
                let record=program.types.iter().find(|record|record.name==*name).ok_or_else(||refusal("request record array must reference an authored record in the same schema module"))?;
                validate(record)?;
                patients = Some((field, record));
            }
            _ => {
                return Err(refusal(
                    "request arrays must contain String identifiers and one flat identifier record",
                ))
            }
        }
    }
    let servers = servers.ok_or_else(|| refusal("request lacks its String identifier array"))?;
    let (patients, record) =
        patients.ok_or_else(|| refusal("request lacks its identifier-record array"))?;
    if root_fields[0].stable_id != servers.stable_id {
        return Err(refusal(
            "request view profile declares identifier array first, record array second",
        ));
    }
    // A source-derived bound on every semantically admissible raw non-space
    // token: six ASCII escape bytes per key/identifier byte, signed i64 width
    // twenty, fixed punctuation. Outside-string whitespace is not counted.
    let per_record = 2 + fields(record)
        .iter()
        .map(|field| 6 * field.name.len() + 4 + if field.ty == Type::String { 98 } else { 20 })
        .sum::<usize>();
    let upper = 256usize
        .checked_mul(per_record)
        .and_then(|bytes| {
            bytes.checked_add(8 * 99 + 6 * (servers.name.len() + patients.name.len()) + 32)
        })
        .ok_or_else(|| refusal("request normalized source bound overflow"))?;
    if upper > 131072 {
        return Err(refusal("request schema exceeds the existing owned byte capacity even after outside-string whitespace normalization"));
    }
    let name = &root.name;
    let id = &root.stable_id;
    let row = &record.name;
    let string = fields(record)
        .iter()
        .find(|field| field.ty == Type::String)
        .unwrap();
    let mut out = imports(program);
    out.push_str(&body(record));
    out.push_str(&arrays::source(record));
    writeln!(out,"@id(\"{id}.json.identifier-span\") record {name}JsonIdentifierSpan{{@id(\"{id}.json.identifier-start\")start:usize,@id(\"{id}.json.identifier-end\")end:usize,}}
@id(\"{id}.json.request-result\") variant {name}JsonRequestDecode{{
@id(\"{id}.json.request-decoded\") Decoded{{@id(\"{id}.json.request-servers\")servers:Vec<{name}JsonIdentifierSpan>,@id(\"{id}.json.request-patients\")patients:Vec<{row}JsonView>,}},
@id(\"{id}.json.request-error\") Error{{@id(\"{id}.json.request-code\")code:i64,@id(\"{id}.json.request-offset\")offset:usize,@id(\"{id}.json.request-field\")field:i64,}},}}
@id(\"{id}.json.request.decode\") fn json_{name}_request_decode(input:borrow Slice<u8>)->{name}JsonRequestDecode{{
let length=byte_len(input);let checked=jv_strict_end(input,32usize,0);
let mut error=if checked>length{{1}}else{{0}};let mut offset=if checked>length{{checked-length-1usize}}else{{0usize}};let mut field=0;
let root=if error==0{{jv_root(input)}}else{{0usize}};
let _ = if error==0 && jv_kind(input,root)!=1{{error=5;offset=root;false}}else{{true}};
let mut servers=vec_with_capacity<{name}JsonIdentifierSpan>(8usize);let mut patients=vec_with_capacity<{row}JsonView>(256usize);
let mut seen_servers=false;let mut seen_patients=false;
let servers_key={};let patients_key={};
let mut key=if error==0{{jv_first_member(input,root)}}else{{length}};
while error==0 && key<length{{
let selected=if jv_key_eq(input,key,array_as_slice(servers_key)){{1}}else{{if jv_key_eq(input,key,array_as_slice(patients_key)){{2}}else{{0}}}};
let start=jv_member_value(input,key);
let _ = if selected==0{{error=4;offset=key;field=0;false}}else{{field=selected;
let repeated=if selected==1{{seen_servers}}else{{seen_patients}};
if repeated{{error=2;offset=key;false}}else{{
seen_servers=seen_servers || selected==1;seen_patients=seen_patients || selected==2;
if jv_kind(input,start)!=2{{error=5;offset=start;false}}else{{
let mut cursor=jv_first_element(input,start);
while error==0 && cursor<length{{
let _ = if selected==1{{
if vec_len<{name}JsonIdentifierSpan>(servers)>=8usize{{error=8;offset=cursor;false}}else{{
let end=if jv_kind(input,cursor)==3{{jv_scan_string(input,cursor,false)}}else{{cursor}};
if jv_kind(input,cursor)!=3{{error=5;offset=cursor;false}}else{{
if !json_{row}_identifier_valid(input,cursor,end){{error=6;offset=cursor;false}}else{{
let mut prior=0usize;let mut duplicate=false;
while !duplicate && prior<vec_len<{name}JsonIdentifierSpan>(servers){{
let other=vec_get<{name}JsonIdentifierSpan>(servers,prior);duplicate=jv_decoded_token_eq(input,other.start,cursor);prior=prior+1usize;!duplicate && prior<vec_len<{name}JsonIdentifierSpan>(servers)
}}
if duplicate{{error=10;offset=cursor;false}}else{{servers=vec_push<{name}JsonIdentifierSpan>(servers,{name}JsonIdentifierSpan{{start:cursor,end:end}});true}}
}}
}}
}}
}}else{{
if vec_len<{row}JsonView>(patients)>=256usize{{error=8;offset=cursor;false}}else{{
let end=jv_value_end(input,cursor,32usize);let item=byte_range(input,cursor,end);
let decoded=json_{row}_view_decode(item,byte_len(item));
match decoded{{{row}JsonViewDecode::Error{{code,offset:at,field:target}}=>{{error=code;offset=cursor+at;field=target;false}},
{row}JsonViewDecode::Decoded{{value}}=>{{let adjusted={row}JsonView{{",key_array(&servers.name),key_array(&patients.name)).unwrap();
    for f in fields(record) {
        if f.ty == Type::String {
            writeln!(
                out,
                "{}_start:cursor+value.{}_start,{}_end:cursor+value.{}_end,",
                f.name, f.name, f.name, f.name
            )
            .unwrap();
        } else {
            writeln!(out, "{}:value.{},", f.name, f.name).unwrap();
        }
    }
    writeln!(out,"}};let mut prior=0usize;let mut duplicate=false;
while !duplicate && prior<vec_len<{row}JsonView>(patients){{let other=vec_get<{row}JsonView>(patients,prior);duplicate=jv_decoded_token_eq(input,other.{}_start,adjusted.{}_start);prior=prior+1usize;!duplicate && prior<vec_len<{row}JsonView>(patients)}}
if duplicate{{error=10;offset=adjusted.{}_start;false}}else{{patients=vec_push<{row}JsonView>(patients,adjusted);true}}
}},}}
}}
}};
cursor=if error==0{{jv_next_element(input,cursor,32usize)}}else{{length}};error==0 && cursor<length
}}
true
}}
}}
}};
key=if error==0{{jv_next_member(input,key,32usize)}}else{{length}};error==0 && key<length
}}
let _ = if error==0 && !seen_servers{{error=3;offset=length;field=1;false}}else{{true}};
let _ = if error==0 && !seen_patients{{error=3;offset=length;field=2;false}}else{{true}};
let _ = if error==0 && vec_len<{name}JsonIdentifierSpan>(servers)==0usize && vec_len<{row}JsonView>(patients)>0usize{{error=11;offset=length;field=1;false}}else{{true}};
if error==0{{{name}JsonRequestDecode::Decoded{{servers:servers,patients:patients}}}}else{{{name}JsonRequestDecode::Error{{code:error,offset:offset,field:field}}}}
}}",string.name,string.name,string.name).unwrap();
    // Encoder receives the exact caller-selected immutable source and every
    // view is revalidated. It cannot authenticate spans by helper identity.
    out.push_str(&encoder(root, servers, patients, record));
    Ok(out)
}

fn encoder(
    root: &TypeDeclaration,
    servers: &crate::ast::FieldDeclaration,
    patients: &crate::ast::FieldDeclaration,
    record: &TypeDeclaration,
) -> String {
    let name = &root.name;
    let id = &root.stable_id;
    let row = &record.name;
    let head = literal(&format!("{{\"{}\":[", servers.name));
    let middle = literal(&format!("],\"{}\":", patients.name));
    let punctuation = servers.name.len() + patients.name.len() + 11;
    format!("@id(\"{id}.json.request.encode\") fn json_{name}_request_encode(input:borrow Slice<u8>,servers:borrow Vec<{name}JsonIdentifierSpan>,patients:borrow Vec<{row}JsonView>,output_limit:usize)->{row}JsonViewEncode{{
let mut required={punctuation}usize;let mut index=0usize;let mut valid=vec_len<{name}JsonIdentifierSpan>(servers)<=8usize && (vec_len<{name}JsonIdentifierSpan>(servers)>0usize || vec_len<{row}JsonView>(patients)==0usize);
while valid && index<vec_len<{name}JsonIdentifierSpan>(servers){{let span=vec_get<{name}JsonIdentifierSpan>(servers,index);valid=json_{row}_identifier_valid(input,span.start,span.end);let mut prior=0usize;while valid && prior<index{{let other=vec_get<{name}JsonIdentifierSpan>(servers,prior);valid=!jv_decoded_token_eq(input,other.start,span.start);prior=prior+1usize;valid && prior<index}}let _ = if valid{{required=required+2usize+jv_decoded_len(input,span.start);if index>0usize{{required=required+1usize;true}}else{{true}}}}else{{false}};index=index+1usize;valid && index<vec_len<{name}JsonIdentifierSpan>(servers)}}
let patient_size=if valid{{json_{row}_view_array_encoded_len(input,patients)}}else{{18446744073709551615usize}};
valid=valid && patient_size!=18446744073709551615usize;
required=if valid{{required+patient_size}}else{{18446744073709551615usize}};
if !valid || required>output_limit{{{row}JsonViewEncode::Refused{{required:required}}}}else{{
let mut text={head};let mut at=0usize;
while at<vec_len<{name}JsonIdentifierSpan>(servers){{let _ = if at>0usize{{text=string_concat(text,\",\");true}}else{{true}};let span=vec_get<{name}JsonIdentifierSpan>(servers,at);text=string_concat(text,json_{row}_identifier_render(input,span.start,span.end));at=at+1usize;at<vec_len<{name}JsonIdentifierSpan>(servers)}}
let prefix=string_concat(text,{middle});let rendered=json_{row}_view_array_encode(input,patients,131072usize);
match own rendered{{{row}JsonViewEncode::Refused{{required:count}}=>{row}JsonViewEncode::Refused{{required:count}},{row}JsonViewEncode::Encoded{{text:items}}=>{{let whole=string_concat(prefix,items);{row}JsonViewEncode::Encoded{{text:string_concat(whole,\"}}\")}}}},}}
}}
}}")
}
