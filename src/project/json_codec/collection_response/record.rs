//! Declaration-order object length and rendering; no schema-name special cases.
use super::*;

fn value(record: &TypeDeclaration, field: &FieldDeclaration, indexed: bool) -> String {
    if indexed {
        format!(
            "vec_field<{}>(values,at,{})",
            record.name,
            literal(&field.name)
        )
    } else if field.ty == Type::String {
        format!("string_as_str(value.{})", field.name)
    } else {
        format!("value.{}", field.name)
    }
}

fn length(record: &TypeDeclaration, field: &FieldDeclaration, indexed: bool) -> String {
    let value = value(record, field, indexed);
    match field.ty {
        Type::String => format!("json_{}_response_utf8_quoted_len({value})", record.name),
        Type::Bool => format!("if {value}{{4usize}}else{{5usize}}"),
        Type::Usize => format!("jv_usize_len({value})"),
        Type::U8 => format!("usize_from_i64(jv_i64_len(i64_from_u8({value})))"),
        Type::I64 => format!("usize_from_i64(jv_i64_len({value}))"),
        _ => unreachable!("validated scalar/string"),
    }
}

fn rendered(record: &TypeDeclaration, field: &FieldDeclaration, indexed: bool) -> String {
    let value = value(record, field, indexed);
    match field.ty {
        Type::String => format!("json_{}_response_utf8_quote({value})", record.name),
        Type::Bool => format!("if {value}{{\"true\"}}else{{\"false\"}}"),
        Type::Usize => format!("string_from_usize({value})"),
        Type::U8 => format!("string_from_i64(i64_from_u8({value}))"),
        Type::I64 => format!("string_from_i64({value})"),
        _ => unreachable!("validated scalar/string"),
    }
}

pub(super) fn source(record: &TypeDeclaration) -> String {
    object_source(record, false)
}

pub(super) fn indexed_source(record: &TypeDeclaration) -> String {
    object_source(record, true)
}

fn object_source(record: &TypeDeclaration, indexed: bool) -> String {
    let fs = fields(record);
    let name = &record.name;
    let id = &record.stable_id;
    // Copy metrics use Value parameters; only the owning Row needs a borrow.
    // StringAsStr authenticates this named borrow root and exact field path;
    // no synthetic String local, record match or clone grants view authority.
    let ownership = if fs.iter().any(|field| field.ty == Type::String) {
        "borrow "
    } else {
        ""
    };
    // Indexed helpers retain the exact borrowed Vec generation. The selector
    // is a compile-time field literal; String reads return a scoped Str directly.
    let (suffix, identity_suffix, parameters) = if indexed {
        ("_at", "-at", format!("values:borrow Vec<{name}>,at:usize"))
    } else {
        ("", "", format!("value:{ownership}{name}"))
    };
    let punctuation = 2 + fs.iter().map(|f| f.name.len() + 3).sum::<usize>() + fs.len() - 1;
    let string_fields: Vec<_> = fs.iter().filter(|field| field.ty == Type::String).collect();
    let (validity, validity_bindings) = match string_fields.as_slice() {
        [] => ("true".to_owned(), String::new()),
        [field] => (
            format!(
                "json_{name}_response_owned_valid({})",
                value(record, field, indexed)
            ),
            String::new(),
        ),
        _ => (
            (0..string_fields.len())
                .map(|index| format!("response_string_valid_{index}"))
                .collect::<Vec<_>>()
                .join("&&"),
            string_fields
                .iter()
                .enumerate()
                .map(|(index, field)| {
                    format!(
                        "let response_string_valid_{index}=json_{name}_response_owned_valid({});",
                        value(record, field, indexed)
                    )
                })
                .collect::<String>(),
        ),
    };
    let mut out = format!(
        "@id(\"{id}.json.collection-response.object-len{identity_suffix}\")
fn json_{name}_response_object_len{suffix}({parameters})->usize {{
{validity_bindings}if !({validity}){{18446744073709551615usize}}else{{{punctuation}usize"
    );
    for field in fs {
        write!(out, "+({})", length(record, field, indexed)).unwrap();
    }
    writeln!(
        out,
        "}}}}
@id(\"{id}.json.collection-response.object-render{identity_suffix}\")
fn json_{name}_response_object_render{suffix}({parameters})->string {{
{validity_bindings}if !({validity}){{\"\"}}else{{let output_0=\"{{\";"
    )
    .unwrap();
    for (index, field) in fs.iter().enumerate() {
        let label = literal(&format!(
            "{}\"{}\":",
            if index == 0 { "" } else { "," },
            field.name
        ));
        writeln!(out, "let label_{index}=string_concat(output_{index},{label});let rendered_{index}={};let output_{}=string_concat(label_{index},rendered_{index});", rendered(record, field, indexed), index+1).unwrap();
    }
    writeln!(out, "string_concat(output_{},\"}}\")}}}}", fs.len()).unwrap();
    out
}
