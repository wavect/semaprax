//! Every size is checked before arithmetic; rendering starts after complete preflight.
use super::*;

const INVALID: &str = "18446744073709551615usize";
const LIMIT: &str = "131072usize";

pub(super) fn source(root: &TypeDeclaration, record: &Record<'_>, bound: usize) -> String {
    let mut out = object(root, record, bound, false, Some(""));
    let name = &root.name;
    let id = &root.stable_id;
    writeln!(out, "@id(\"{id}.json.nested-response.encode-result\") variant {name}JsonNestedResponseEncode {{
@id(\"{id}.json.nested-response.encoded\") Encoded{{@id(\"{id}.json.nested-response.text\") text:string,}},
@id(\"{id}.json.nested-response.refused\") Refused{{@id(\"{id}.json.nested-response.required\") required:usize,}},
}}
@id(\"{id}.json.nested-response.encoded-len\") fn json_{name}_nested_response_encoded_len(value:borrow {name})->usize {{json_{name}_nested_response_object_len_0(value)}}
@id(\"{id}.json.nested-response.encode\") fn json_{name}_nested_response_encode(value:borrow {name},output_limit:usize)->{name}JsonNestedResponseEncode {{
let required=json_{name}_nested_response_encoded_len(value);
if required=={INVALID} || required>output_limit {{{name}JsonNestedResponseEncode::Refused{{required:required}}}}else{{
{name}JsonNestedResponseEncode::Encoded{{text:json_{name}_nested_response_object_render_0(value)}}
}}
}}" ).unwrap();
    out
}

fn object(
    root: &TypeDeclaration,
    record: &Record<'_>,
    bound: usize,
    indexed: bool,
    root_path: Option<&str>,
) -> String {
    let mut out = String::new();
    for field in &record.fields {
        match &field.kind {
            Kind::Record(child) => {
                let path = format!(
                    "{}.{}",
                    root_path.expect("flat vector rows have no child records"),
                    field.declaration.name
                );
                out.push_str(&object(root, child, bound, false, Some(&path)));
            }
            Kind::Vector { element, kind } => {
                if let Kind::Record(child) = &**kind {
                    out.push_str(&object(root, child, bound, owns(child), None));
                }
                out.push_str(&vector(root, element, kind, field.ordinal, bound));
            }
            _ => {}
        }
    }
    let name = &root.name;
    let id = &root.stable_id;
    let n = record.ordinal;
    let parameters = if root_path.is_some() {
        format!("value:borrow {}", root.name)
    } else if indexed {
        format!("values:borrow Vec<{}>,at:usize", record.declaration.name)
    } else {
        format!(
            "value:{}{}",
            if owns(record) { "borrow " } else { "" },
            record.declaration.name
        )
    };
    let punctuation = 2 + record.fields.len() - 1
        + record
            .fields
            .iter()
            .map(|field| field.declaration.name.len() + 3)
            .sum::<usize>();
    writeln!(out, "@id(\"{id}.json.nested-response.object-len.{n}\") fn json_{name}_nested_response_object_len_{n}({parameters})->usize {{let mut total={punctuation}usize;let mut valid=true;").unwrap();
    for field in &record.fields {
        let access = access(record, field, indexed, root_path);
        let operand = if matches!(field.kind, Kind::Record(_)) && root_path.is_some() {
            "value"
        } else {
            &access
        };
        let size = length(root, &field.kind, field.ordinal, operand);
        // Lazy guards ensure a refused child cannot overflow or underflow the sum.
        writeln!(out, "let _ = if valid{{let size={size};if size=={INVALID} || size>{LIMIT}-total{{valid=false;false}}else{{total=total+size;true}}}}else{{false}};").unwrap();
    }
    writeln!(out, "if valid{{total}}else{{{INVALID}}}\n}}
@id(\"{id}.json.nested-response.object-render.{n}\") fn json_{name}_nested_response_object_render_{n}({parameters})->string {{let mut text=\"{{\";").unwrap();
    for (index, field) in record.fields.iter().enumerate() {
        let key = literal(&format!(
            "{}\"{}\":",
            if index == 0 { "" } else { "," },
            field.declaration.name
        ));
        let access = access(record, field, indexed, root_path);
        let operand = if matches!(field.kind, Kind::Record(_)) && root_path.is_some() {
            "value"
        } else {
            &access
        };
        let rendered = render(root, &field.kind, field.ordinal, operand);
        writeln!(
            out,
            "text=string_concat(text,{key});text=string_concat(text,{rendered});"
        )
        .unwrap();
    }
    out.push_str("string_concat(text,\"}\")\n}\n");
    out
}

fn access(
    record: &Record<'_>,
    field: &descriptor::Field<'_>,
    indexed: bool,
    root_path: Option<&str>,
) -> String {
    if indexed {
        format!(
            "vec_field<{}>(values,at,{})",
            record.declaration.name,
            literal(&field.declaration.name)
        )
    } else {
        let place = format!(
            "value{}.{}",
            root_path.unwrap_or(""),
            field.declaration.name
        );
        if matches!(field.kind, Kind::Text) {
            format!("string_as_str({place})")
        } else {
            place
        }
    }
}

fn length(root: &TypeDeclaration, kind: &Kind<'_>, ordinal: usize, access: &str) -> String {
    let name = &root.name;
    match kind {
        Kind::Text => format!("json_{name}_nested_response_utf8_quoted_len({access})"),
        Kind::Scalar(Type::Bool) => format!("if {access}{{4usize}}else{{5usize}}"),
        Kind::Scalar(Type::Usize) => format!("jv_usize_len({access})"),
        Kind::Scalar(Type::U8) => format!("usize_from_i64(jv_i64_len(i64_from_u8({access})))"),
        Kind::Scalar(Type::I64) => format!("usize_from_i64(jv_i64_len({access}))"),
        Kind::Record(child) => format!(
            "json_{name}_nested_response_object_len_{}({access})",
            child.ordinal
        ),
        Kind::Vector { .. } => format!("json_{name}_nested_response_array_len_{ordinal}({access})"),
        Kind::Scalar(_) => unreachable!("validated scalar"),
    }
}
fn render(root: &TypeDeclaration, kind: &Kind<'_>, ordinal: usize, access: &str) -> String {
    let name = &root.name;
    match kind {
        Kind::Text => format!("json_{name}_nested_response_utf8_quote({access})"),
        Kind::Scalar(Type::Bool) => format!("if {access}{{\"true\"}}else{{\"false\"}}"),
        Kind::Scalar(Type::Usize) => format!("string_from_usize({access})"),
        Kind::Scalar(Type::U8) => format!("string_from_i64(i64_from_u8({access}))"),
        Kind::Scalar(Type::I64) => format!("string_from_i64({access})"),
        Kind::Record(child) => format!(
            "json_{name}_nested_response_object_render_{}({access})",
            child.ordinal
        ),
        Kind::Vector { .. } => {
            format!("json_{name}_nested_response_array_render_{ordinal}({access})")
        }
        Kind::Scalar(_) => unreachable!("validated scalar"),
    }
}

fn vector(
    root: &TypeDeclaration,
    element: &Type,
    kind: &Kind<'_>,
    ordinal: usize,
    bound: usize,
) -> String {
    let name = &root.name;
    let id = &root.stable_id;
    let ty = type_name(element);
    let (bindings, size, rendered) = match kind {
        Kind::Record(child) if owns(child) => (
            String::new(),
            format!(
                "json_{name}_nested_response_object_len_{}(values,at)",
                child.ordinal
            ),
            format!(
                "json_{name}_nested_response_object_render_{}(values,at)",
                child.ordinal
            ),
        ),
        _ => (
            format!("let value=vec_get<{ty}>(values,at);"),
            length(root, kind, ordinal, "value"),
            render(root, kind, ordinal, "value"),
        ),
    };
    format!("@id(\"{id}.json.nested-response.array-len.{ordinal}\") fn json_{name}_nested_response_array_len_{ordinal}(values:borrow Vec<{ty}>)->usize {{
let count=vec_len<{ty}>(values);let mut valid=count<={bound}usize;let mut total=2usize;let mut at=0usize;
while valid && at<count {{{bindings}let size={size};let comma=if at>0usize{{1usize}}else{{0usize}};
let _ = if size=={INVALID} || total>{LIMIT}-comma || size>{LIMIT}-total-comma{{valid=false;false}}else{{total=total+size+comma;true}};
at=at+1usize;valid && at<count
}}if valid{{total}}else{{{INVALID}}}
}}
@id(\"{id}.json.nested-response.array-render.{ordinal}\") fn json_{name}_nested_response_array_render_{ordinal}(values:borrow Vec<{ty}>)->string {{
let mut text=\"[\";let count=vec_len<{ty}>(values);let mut at=0usize;
while at<count {{{bindings}let _ = if at>0usize{{text=string_concat(text,\",\");true}}else{{true}};
text=string_concat(text,{rendered});at=at+1usize;at<count
}}string_concat(text,\"]\")
}}\n")
}
