//! Complete length preflight precedes response construction/publication.
use super::*;

pub(super) fn source(root: &TypeDeclaration, shape: &descriptor::Shape<'_>) -> String {
    let name = &root.name;
    let id = &root.stable_id;
    let row = &shape.row.name;
    let metrics = &shape.metrics.name;
    let items = &shape.items.name;
    let metrics_field = &shape.metrics_field.name;
    // Root braces, quoted keys/colons, one comma, and vector brackets.
    // Child object lengths include their own braces and field punctuation.
    let punctuation = 2
        + fields(root)
            .iter()
            .map(|field| field.name.len() + 3)
            .sum::<usize>()
        + 1
        + 2;
    let mut out = format!("@id(\"{id}.json.collection-response.encode-result\")
variant {name}JsonCollectionResponseEncode {{
@id(\"{id}.json.collection-response.encoded\") Encoded{{@id(\"{id}.json.collection-response.text\") text:string,}},
@id(\"{id}.json.collection-response.refused\") Refused{{@id(\"{id}.json.collection-response.required\") required:usize,}},
}}
@id(\"{id}.json.collection-response.encoded-len\")
fn json_{name}_collection_response_encoded_len(value:borrow {name})->usize {{
let count=vec_len<{row}>(value.{items});
let mut valid=count<=256usize;
let mut total={punctuation}usize+json_{metrics}_response_object_len(value.{metrics_field});
let mut at=0usize;
while valid && at<count {{
let size=json_{row}_response_object_len_at(value.{items},at);
let comma=if at>0usize{{1usize}}else{{0usize}};
let _ = if size==18446744073709551615usize || total>131072usize-comma || size>131072usize-total-comma {{valid=false;false}}else{{total=total+size+comma;true}};
at=at+1usize;valid && at<count
}}
if valid && total<=131072usize{{total}}else{{18446744073709551615usize}}
}}
@id(\"{id}.json.collection-response.encode\")
fn json_{name}_collection_response_encode(value:borrow {name},output_limit:usize)->{name}JsonCollectionResponseEncode {{
let required=json_{name}_collection_response_encoded_len(value);
if required==18446744073709551615usize || required>output_limit {{{name}JsonCollectionResponseEncode::Refused{{required:required}}}}else{{
let mut text=\"{{\";");
    // Root declaration order controls wire order, independently of vector sorting.
    for (index, field) in fields(root).iter().enumerate() {
        let key = literal(&format!(
            "{}\"{}\":",
            if index == 0 { "" } else { "," },
            field.name
        ));
        writeln!(out, "text=string_concat(text,{key});").unwrap();
        if field.stable_id == shape.items.stable_id {
            writeln!(
                out,
                "text=string_concat(text,\"[\");let mut at=0usize;
while at<vec_len<{row}>(value.{items}) {{
let _ = if at>0usize{{text=string_concat(text,\",\");true}}else{{true}};
text=string_concat(text,json_{row}_response_object_render_at(value.{items},at));
at=at+1usize;at<vec_len<{row}>(value.{items})
}}
text=string_concat(text,\"]\");"
            )
            .unwrap();
        } else {
            writeln!(out, "text=string_concat(text,json_{metrics}_response_object_render(value.{metrics_field}));").unwrap();
        }
    }
    writeln!(
        out,
        "{name}JsonCollectionResponseEncode::Encoded{{text:string_concat(text,\"}}\")}}
}}
}}"
    )
    .unwrap();
    out
}
