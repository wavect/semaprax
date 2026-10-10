//! Bounded array collection over independently checked, source-relative views.

use super::*;

pub(super) fn source(record: &TypeDeclaration) -> String {
    let name = &record.name;
    let id = &record.stable_id;
    let fs = fields(record);
    let identity = fs.iter().find(|field| field.ty == Type::String).unwrap();
    let mut out=format!("@id(\"{id}.json.array-result\") variant {name}JsonArrayDecode {{ @id(\"{id}.json.array-decoded\") Decoded{{@id(\"{id}.json.array-values\")values:Vec<{name}JsonView>,}}, @id(\"{id}.json.array-error\") Error{{@id(\"{id}.json.array-code\")code:i64,@id(\"{id}.json.array-offset\")offset:usize,@id(\"{id}.json.array-field\")field:i64,}}, }}\n");
    writeln!(out,"@id(\"{id}.json.array.decode\") fn json_{name}_view_array_decode(input:borrow Slice<u8>,input_limit:usize,count_limit:usize)->{name}JsonArrayDecode{{
let length=byte_len(input);let checked=if length<=input_limit{{jv_strict_end(input,32usize,0)}}else{{length}};
let mut error=if length>input_limit{{7}}else{{if checked>length{{1}}else{{0}}}};
let mut offset=if checked>length{{checked-length-1usize}}else{{0usize}};let mut field=0;
let root=if error==0{{jv_root(input)}}else{{0usize}};
let _ = if error==0 && jv_kind(input,root)!=2{{error=5;offset=root;false}}else{{true}};
let mut values=vec_with_capacity<{name}JsonView>(256usize);
let limit=if count_limit<=256usize{{count_limit}}else{{256usize}};
let mut cursor=if error==0{{jv_first_element(input,root)}}else{{length}};
while error==0 && cursor<length{{
let _ = if vec_len<{name}JsonView>(values)>=limit{{error=8;offset=cursor;false}}else{{
let end=jv_value_end(input,cursor,32usize);
let item=byte_range(input,cursor,end);
let decoded=json_{name}_view_decode(item,byte_len(item));
match decoded{{
{name}JsonViewDecode::Error{{code,offset:at,field:target}}=>{{error=code;offset=cursor+at;field=target;false}},
{name}JsonViewDecode::Decoded{{value}}=>{{
let adjusted={name}JsonView{{").unwrap();
    for f in fs {
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
    writeln!(out,"}};
let mut prior=0usize;let mut duplicate=false;
while !duplicate && prior<vec_len<{name}JsonView>(values){{
let other=vec_get<{name}JsonView>(values,prior);
duplicate=jv_decoded_token_eq(input,other.{}_start,adjusted.{}_start);
prior=prior+1usize;!duplicate && prior<vec_len<{name}JsonView>(values)
}}
if duplicate{{error=10;offset=adjusted.{}_start;false}}else{{values=vec_push<{name}JsonView>(values,adjusted);true}}
}},
}}
}};
cursor=if error==0{{jv_next_element(input,cursor,32usize)}}else{{length}};
error==0 && cursor<length
}}
if error==0{{{name}JsonArrayDecode::Decoded{{values:values}}}}else{{{name}JsonArrayDecode::Error{{code:error,offset:offset,field:field}}}}
}}
@id(\"{id}.json.view.render\") fn json_{name}_view_render(input:borrow Slice<u8>,value:{name}JsonView)->string{{
let rendered=json_{name}_view_encode(input,value,131072usize);
let mut output=\"\";let accepted=match own rendered{{{name}JsonViewEncode::Encoded{{text:encoded_text}}=>{{output=encoded_text;true}},{name}JsonViewEncode::Refused{{required:refused_size}}=>false,}};output
}}
@id(\"{id}.json.array.encoded-len\") fn json_{name}_view_array_encoded_len(input:borrow Slice<u8>,values:borrow Vec<{name}JsonView>)->usize{{
let count=vec_len<{name}JsonView>(values);let mut total=if count<=256usize{{2usize}}else{{18446744073709551615usize}};let mut index=0usize;
while total!=18446744073709551615usize && index<count{{
let value=vec_get<{name}JsonView>(values,index);let size=json_{name}_view_encoded_len(input,value);
let mut prior=0usize;let mut duplicate=false;
while size!=18446744073709551615usize && !duplicate && prior<index{{let other=vec_get<{name}JsonView>(values,prior);duplicate=jv_decoded_token_eq(input,other.__IDENTITY___start,value.__IDENTITY___start);prior=prior+1usize;!duplicate && prior<index}}
let comma=if index>0usize{{1usize}}else{{0usize}};
let _ = if duplicate || size==18446744073709551615usize || total>131072usize-comma || size>131072usize-total-comma{{total=18446744073709551615usize;false}}else{{total=total+size+comma;true}};
index=index+1usize;total!=18446744073709551615usize && index<count
}}
total
}}
@id(\"{id}.json.array.encode\") fn json_{name}_view_array_encode(input:borrow Slice<u8>,values:borrow Vec<{name}JsonView>,output_limit:usize)->{name}JsonViewEncode{{
let required=json_{name}_view_array_encoded_len(input,values);
if required==18446744073709551615usize || required>output_limit{{{name}JsonViewEncode::Refused{{required:required}}}}else{{
let mut text=\"[\";let mut index=0usize;
while index<vec_len<{name}JsonView>(values){{
let _ = if index>0usize{{text=string_concat(text,\",\");true}}else{{true}};
let value=vec_get<{name}JsonView>(values,index);
text=string_concat(text,json_{name}_view_render(input,value));
index=index+1usize;index<vec_len<{name}JsonView>(values)
}}
{name}JsonViewEncode::Encoded{{text:string_concat(text,\"]\")}}
}}
}}",identity.name,identity.name,identity.name).unwrap();
    out.replace("__IDENTITY__", &identity.name)
}
