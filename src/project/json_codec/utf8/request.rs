//! Two-array request views retain ordinary owner transfer and borrowed source.

use super::super::{refusal, validate_record, views};
use super::*;
use views::validate;

pub(super) fn source(
    program: &Program,
    root: &TypeDeclaration,
    max_string_bytes: usize,
) -> Result<String, Vec<Diagnostic>> {
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
            "UTF-8 request requires exactly a String array and a flat String-record array",
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
                    "UTF-8 request arrays must contain plain Strings and one flat String record",
                ))
            }
        }
    }
    let servers = servers.ok_or_else(|| refusal("UTF-8 request lacks its String array"))?;
    let (patients, record) =
        patients.ok_or_else(|| refusal("UTF-8 request lacks its flat String-record array"))?;
    if root_fields[0].stable_id != servers.stable_id {
        return Err(refusal(
            "UTF-8 request policy declares String array first, record array second",
        ));
    }
    // This is a canonical OUTPUT bound, not a bound on every possible raw
    // spelling. Authored identifier keys need no JSON escaping on output;
    // decoded String bytes can each require six bytes (e.g. NUL -> \u0000).
    // Raw keys/values can use longer equivalent escape spellings. A direct
    // decoder borrows that actual input; the stream normalizer independently
    // validates all grammar and selects its existing physical-buffer refusal.
    let per_record = 2 + fields(record)
        .iter()
        .map(|field| {
            field.name.len()
                + 4
                + if field.ty == Type::String {
                    6 * max_string_bytes + 2
                } else {
                    20
                }
        })
        .sum::<usize>();
    let canonical_output_upper = 256usize
        .checked_mul(per_record)
        .and_then(|bytes| {
            bytes.checked_add(
                8 * (6 * max_string_bytes + 3) + servers.name.len() + patients.name.len() + 32,
            )
        })
        .ok_or_else(|| refusal("UTF-8 request canonical output bound overflow"))?;
    if canonical_output_upper > 131072 {
        return Err(refusal("UTF-8 request schema exceeds the existing canonical output capacity at maximum cardinality"));
    }
    let name = &root.name;
    let id = &root.stable_id;
    let row = &record.name;
    let mut out = views::imports(program);
    out.push_str(&text::imports());
    out.push_str(&views::body_with_text(
        record,
        Some(&text::source(record, max_string_bytes)),
    ));
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
let servers_key_view=array_as_slice(servers_key);let patients_key_view=array_as_slice(patients_key);
let mut key=if error==0{{jv_first_member(input,root)}}else{{length}};
while error==0 && key<length{{
let selected=if jv_key_eq(input,key,servers_key_view){{1}}else{{if jv_key_eq(input,key,patients_key_view){{2}}else{{0}}}};
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
servers=vec_push<{name}JsonIdentifierSpan>(servers,{name}JsonIdentifierSpan{{start:cursor,end:end}});true
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
    writeln!(out,"}};patients=vec_push<{row}JsonView>(patients,adjusted);true
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
if error==0{{{name}JsonRequestDecode::Decoded{{servers:servers,patients:patients}}}}else{{{name}JsonRequestDecode::Error{{code:error,offset:offset,field:field}}}}
}}").unwrap();
    Ok(out)
}
