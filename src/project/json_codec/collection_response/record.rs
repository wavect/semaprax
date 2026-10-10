//! Declaration-order object length and rendering; no schema-name special cases.
use super::*;

fn length(record: &TypeDeclaration, field: &FieldDeclaration) -> String {
    let value = format!("value.{}", field.name);
    match field.ty {
        Type::String => format!(
            "json_{}_response_utf8_quoted_len(string_as_str({value}))",
            record.name
        ),
        Type::Bool => format!("if {value}{{4usize}}else{{5usize}}"),
        Type::Usize => format!("jv_usize_len({value})"),
        Type::U8 => format!("usize_from_i64(jv_i64_len(i64_from_u8({value})))"),
        Type::I64 => format!("usize_from_i64(jv_i64_len({value}))"),
        _ => unreachable!("validated scalar/string"),
    }
}

fn rendered(record: &TypeDeclaration, field: &FieldDeclaration) -> String {
    let value = format!("value.{}", field.name);
    match field.ty {
        Type::String => format!(
            "json_{}_response_utf8_quote(string_as_str({value}))",
            record.name
        ),
        Type::Bool => format!("if {value}{{\"true\"}}else{{\"false\"}}"),
        Type::Usize => format!("string_from_usize({value})"),
        Type::U8 => format!("string_from_i64(i64_from_u8({value}))"),
        Type::I64 => format!("string_from_i64({value})"),
        _ => unreachable!("validated scalar/string"),
    }
}

pub(super) fn source(record: &TypeDeclaration) -> String {
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
    let punctuation = 2 + fs.iter().map(|f| f.name.len() + 3).sum::<usize>() + fs.len() - 1;
    let valid = fs
        .iter()
        .find(|f| f.ty == Type::String)
        .map(|field| {
            format!(
                "json_{name}_response_owned_valid(string_as_str(value.{}))",
                field.name
            )
        })
        .unwrap_or_else(|| "true".to_owned());
    let mut out = format!(
        "@id(\"{id}.json.collection-response.object-len\")
fn json_{name}_response_object_len(value:{ownership}{name})->usize {{
if !({valid}){{18446744073709551615usize}}else{{{punctuation}usize"
    );
    for field in fs {
        write!(out, "+({})", length(record, field)).unwrap();
    }
    writeln!(
        out,
        "}}}}
@id(\"{id}.json.collection-response.object-render\")
fn json_{name}_response_object_render(value:{ownership}{name})->string {{
if !({valid}){{\"\"}}else{{let output_0=\"{{\";"
    )
    .unwrap();
    for (index, field) in fs.iter().enumerate() {
        let label = literal(&format!(
            "{}\"{}\":",
            if index == 0 { "" } else { "," },
            field.name
        ));
        writeln!(out, "let label_{index}=string_concat(output_{index},{label});let rendered_{index}={};let output_{}=string_concat(label_{index},rendered_{index});", rendered(record, field), index+1).unwrap();
    }
    writeln!(out, "string_concat(output_{},\"}}\")}}}}", fs.len()).unwrap();
    out
}
