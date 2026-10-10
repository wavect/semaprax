//! Helpers are ordinary explicit declarations; the checked Project rebuild owns admission.
use super::*;
use descriptor::Shape;
pub(super) fn source(root: &TypeDeclaration, shape: &Shape<'_>, array_bound: usize) -> String {
    let name = &root.name;
    let id = &root.stable_id;
    let mut out = String::new();
    writeln!(out,"@id(\"{id}.json.nested.status\") record {name}JsonNestedStatus {{@id(\"{id}.json.nested.status-code\") code:i64,@id(\"{id}.json.nested.status-offset\") offset:usize,@id(\"{id}.json.nested.status-field\") field:i64,}}\n@id(\"{id}.json.nested.decode-result\") variant {name}JsonNestedDecode {{@id(\"{id}.json.nested.ready\") Ready{{@id(\"{id}.json.nested.value\") value:{name},}},@id(\"{id}.json.nested.error\") Error{{@id(\"{id}.json.nested.code\") code:i64,@id(\"{id}.json.nested.offset\") offset:usize,@id(\"{id}.json.nested.field\") field:i64,}},}}").unwrap();
    out.push_str(&validate::source(root, &shape.root, array_bound));
    out.push_str(&materialize::source(root, &shape.root));
    writeln!(out,"@id(\"{id}.json.nested.decode\") fn json_{name}_nested_decode(input:borrow Slice<u8>,input_limit:usize)->{name}JsonNestedDecode {{
let length=byte_len(input);
if length>input_limit {{{name}JsonNestedDecode::Error{{code:7,offset:0usize,field:0}}}}else{{
let checked=jv_strict_end(input,32usize,0);
if checked>length {{{name}JsonNestedDecode::Error{{code:1,offset:checked-length-1usize,field:0}}}}else{{
let object=jv_root(input);let status=json_{name}_nested_check_0(input,object);
if status.code!=0 {{{name}JsonNestedDecode::Error{{code:status.code,offset:status.offset,field:status.field}}}}else{{
{name}JsonNestedDecode::Ready{{value:json_{name}_nested_build_0(input,object)}}
}}
}}
}}
}}").unwrap();
    out
}
