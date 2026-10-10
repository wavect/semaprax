//! Strict whole grammar precedes these allocation-free, source-derived checks.
use super::*;
use descriptor::{Kind, Record};

pub(super) fn source(root: &TypeDeclaration, record: &Record<'_>, bound: usize) -> String {
    let name = &root.name;
    let id = &root.stable_id;
    let ordinal = record.ordinal;
    let mut out = String::new();
    for field in &record.fields {
        emit_children(&mut out, root, &field.kind, field.ordinal, bound);
    }
    writeln!(out,"@id(\"{id}.json.nested.check.{ordinal}\") fn json_{name}_nested_check_{ordinal}(input:borrow Slice<u8>,object:usize)->{name}JsonNestedStatus {{\nlet length=byte_len(input);let mut error=if jv_kind(input,object)==1{{0}}else{{5}};let mut offset=if error==0{{0usize}}else{{object}};let mut field={ordinal};").unwrap();
    for field in &record.fields {
        writeln!(out, "let mut seen_{}=false;", field.ordinal).unwrap();
        writeln!(
            out,
            "let key_{}={};\nlet key_view_{}=array_as_slice(key_{});",
            field.ordinal,
            key_array(&field.declaration.name),
            field.ordinal,
            field.ordinal
        )
        .unwrap();
    }
    out.push_str("let mut key=if error==0{jv_first_member(input,object)}else{length};\nwhile error==0 && key<length{\nlet start=jv_member_value(input,key);let mut selected=0;\n");
    for field in &record.fields {
        let n = field.ordinal;
        writeln!(
            out,
            "selected=if jv_key_eq(input,key,key_view_{n}){{{n}}}else{{selected}};"
        )
        .unwrap();
    }
    out.push_str("let _ = if selected==0{error=4;offset=key;field=0;false}else{field=selected;\n");
    for f in &record.fields {
        let n = f.ordinal;
        writeln!(
            out,
            "let _ = if selected=={n}{{if seen_{n}{{error=2;offset=key;false}}else{{seen_{n}=true;"
        )
        .unwrap();
        out.push_str(&check(root, &f.kind, n, bound));
        out.push_str("true\n}}else{true};\n");
    }
    out.push_str("true};\nkey=if error==0{jv_next_member(input,key,32usize)}else{length};error==0 && key<length\n}\n");
    for f in &record.fields {
        let n = f.ordinal;
        writeln!(
            out,
            "let _ = if error==0 && !seen_{n}{{error=3;offset=length;field={n};false}}else{{true}};"
        )
        .unwrap();
    }
    writeln!(
        out,
        "{name}JsonNestedStatus{{code:error,offset:offset,field:if error==0{{0}}else{{field}}}}\n}}"
    )
    .unwrap();
    out
}
fn emit_children(
    out: &mut String,
    root: &TypeDeclaration,
    kind: &Kind<'_>,
    ordinal: usize,
    bound: usize,
) {
    match kind {
        Kind::Record(record) => out.push_str(&source(root, record, bound)),
        Kind::Vector { kind, .. } => {
            emit_children(out, root, kind, ordinal, bound);
            let name = &root.name;
            let id = &root.stable_id;
            writeln!(out,"@id(\"{id}.json.nested.array-check.{ordinal}\") fn json_{name}_nested_array_check_{ordinal}(input:borrow Slice<u8>,array_start:usize)->{name}JsonNestedStatus {{let length=byte_len(input);let mut error=if jv_kind(input,array_start)==2{{0}}else{{5}};let mut offset=if error==0{{0usize}}else{{array_start}};let mut field={ordinal};let mut count=0usize;let mut cursor=if error==0{{jv_first_element(input,array_start)}}else{{length}};\nwhile error==0 && cursor<length{{let start=cursor;\nlet _ = if count>={bound}usize{{error=8;offset=cursor;false}}else{{").unwrap();
            out.push_str(&check(root, kind, ordinal, bound));
            out.push_str("count=count+1usize;true};\ncursor=if error==0{jv_next_element(input,cursor,32usize)}else{length};error==0 && cursor<length\n}\n");
            writeln!(out,"{name}JsonNestedStatus{{code:error,offset:offset,field:if error==0{{0}}else{{field}}}}\n}}").unwrap();
        }
        _ => {}
    }
}
fn check(root: &TypeDeclaration, kind: &Kind<'_>, ordinal: usize, _bound: usize) -> String {
    let name = &root.name;
    let mut out = String::new();
    match kind {
  Kind::Record(record)=>writeln!(out,"let status=json_{name}_nested_check_{}(input,start);error=status.code;offset=status.offset;field=if status.code==0{{field}}else{{status.field}};",record.ordinal).unwrap(),
  Kind::Vector{..}=>writeln!(out,"let status=json_{name}_nested_array_check_{ordinal}(input,start);error=status.code;offset=status.offset;field=if status.code==0{{field}}else{{status.field}};").unwrap(),
  Kind::Text=>writeln!(out,"let _ = if jv_kind(input,start)!=3{{error=5;offset=start;false}}else{{let end=jv_scan_string(input,start,false);if !json_{name}_nested_text_valid(input,start,end){{error=6;offset=start;false}}else{{true}}}};").unwrap(),
  Kind::Scalar(Type::Bool)=>out.push_str("let kind=jv_kind(input,start);let _ = if kind!=5 && kind!=6{error=5;offset=start;false}else{true};\n"),
  Kind::Scalar(ty)=>{
   out.push_str("let _ = if jv_kind(input,start)!=4{error=5;offset=start;false}else{let end=jv_decimal_end(input,start);if jv_integer_end(input,start)!=end{error=6;offset=start;false}else{\n");
   if *ty==Type::Usize {out.push_str("let negative=match byte_get(input,start){Option::Some{value}=>value==45u8,Option::None{}=>false,};let mut number=0usize;let mut at=start;let _ = if negative{error=6;offset=start;false}else{\nwhile error==0 && at<end{let digit=match byte_get(input,at){Option::Some{value}=>usize_from_u8(value-48u8),Option::None{}=>0usize,};let over=number>1844674407370955161usize || number==1844674407370955161usize && digit>5usize;let _ = if over{error=6;offset=start;false}else{number=number*10usize+digit;at=at+1usize;true};error==0 && at<end\n}true};\n");}
   else {let extra=if *ty==Type::U8{" || number<0 || number>255"}else{""};writeln!(out,"let number=jv_i64_or(input,start,0);let other=jv_i64_or(input,start,1);let _ = if number!=other{extra}{{error=6;offset=start;false}}else{{true}};").unwrap();}
   out.push_str("true}};\n");
  },
 }
    out
}
