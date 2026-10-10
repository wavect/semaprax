//! Ordinary source lowering; all emitted operations already have checked backends.

use std::fmt::Write as _;

use crate::ast::{Program, Type, TypeDeclaration, TypeDeclarationKind};

pub(super) fn source(program: &Program, record: &TypeDeclaration) -> String {
    let TypeDeclarationKind::Record { fields } = &record.kind else {
        unreachable!()
    };
    let name = &record.name;
    let id = &record.stable_id;
    let mut out = format!("module {};\n", program.module);
    for (module, stable, alias) in [
        ("scan", "strict_end", "jc_strict_end"),
        ("scan", "root", "jc_root"),
        ("scan", "kind", "jc_kind"),
        ("scan", "first_member", "jc_first_member"),
        ("scan", "member_value", "jc_member_value"),
        ("scan", "next_member", "jc_next_member"),
        ("scan", "key_eq", "jc_key_eq"),
        ("scan", "decimal_end", "jc_decimal_end"),
        ("token", "integer_end", "jc_integer_end"),
        ("token", "i64_or", "jc_i64_or"),
        ("digits", "i64_len", "jc_i64_len"),
        ("write", "usize_len", "jc_usize_len"),
    ] {
        writeln!(out, "use function @id(\"std.data.json.{module}.{stable}\") from std.data.json.{module} as {alias};").unwrap();
    }
    writeln!(
        out,
        "@id(\"{id}.json.decode-result\")\nvariant {name}JsonDecode {{"
    )
    .unwrap();
    writeln!(
        out,
        "@id(\"{id}.json.decoded\") Decoded {{ @id(\"{id}.json.decoded.value\") value: {name}, }},"
    )
    .unwrap();
    writeln!(out, "@id(\"{id}.json.decode-error\") Error {{ @id(\"{id}.json.error.code\") code: i64, @id(\"{id}.json.error.offset\") offset: usize, @id(\"{id}.json.error.field\") field: i64, }},\n}}").unwrap();
    writeln!(
        out,
        "@id(\"{id}.json.encode-result\")\nvariant {name}JsonEncode {{"
    )
    .unwrap();
    writeln!(
        out,
        "@id(\"{id}.json.encoded\") Encoded {{ @id(\"{id}.json.encoded.text\") text: string, }},"
    )
    .unwrap();
    writeln!(out, "@id(\"{id}.json.refused\") Refused {{ @id(\"{id}.json.refused.required\") required: usize, }},\n}}").unwrap();
    writeln!(out, "@id(\"{id}.json.decode\")\nfn json_{name}_decode(input: borrow Slice<u8>, input_limit: usize) -> {name}JsonDecode {{").unwrap();
    out.push_str("let length = byte_len(input);\nlet mut error = if length > input_limit { 7 } else { 0 };\nlet mut offset = 0usize;\nlet mut field = 0;\nlet valid = if error == 0 { jc_strict_end(input, 32usize, 0) } else { length };\nerror = if error == 0 && valid > length { 1 } else { error };\noffset = if valid > length { valid - length - 1usize } else { offset };\nlet object = if error == 0 { jc_root(input) } else { 0usize };\nerror = if error == 0 && jc_kind(input, object) != 1 { 5 } else { error };\noffset = if error == 5 { object } else { offset };\n");
    for (index, f) in fields.iter().enumerate() {
        let zero = match f.ty {
            Type::Bool => "false",
            Type::U8 => "0u8",
            Type::Usize => "0usize",
            _ => "0",
        };
        writeln!(
            out,
            "let mut seen_{index} = false;\nlet mut value_{index} = {zero};"
        )
        .unwrap();
    }
    for (index, f) in fields.iter().enumerate() {
        let bytes = f
            .name
            .bytes()
            .map(|b| format!("{b}u8"))
            .collect::<Vec<_>>()
            .join(", ");
        writeln!(
            out,
            "let key_{index} = [{bytes}];\nlet key_view_{index} = array_as_slice(key_{index});"
        )
        .unwrap();
    }
    out.push_str("let mut key = if error == 0 { jc_first_member(input, object) } else { length };\nwhile error == 0 && key < length {\nlet start = jc_member_value(input, key);\nlet mut selected = 0;\n");
    for (index, _) in fields.iter().enumerate() {
        writeln!(
            out,
            "selected = if jc_key_eq(input, key, key_view_{index}) {{ {} }} else {{ selected }};",
            index + 1
        )
        .unwrap();
    }
    out.push_str("let _ = if selected == 0 { error = 4; offset = key; field = 0; false } else {\nfield = selected;\n");
    for (index, f) in fields.iter().enumerate() {
        writeln!(out, "let _ = if selected == {} {{\nif seen_{index} {{ error = 2; offset = key; false }} else {{\nseen_{index} = true;\nlet kind = jc_kind(input, start);", index + 1).unwrap();
        if f.ty == Type::Bool {
            writeln!(out, "if kind == 5 || kind == 6 {{ value_{index} = kind == 5; true }} else {{ error = 5; offset = start; false }}").unwrap();
        } else {
            out.push_str("if kind != 4 { error = 5; offset = start; false } else {\nlet end = jc_decimal_end(input, start);\nif jc_integer_end(input, start) != end { error = 6; offset = start; false } else {\n");
            if f.ty == Type::Usize {
                out.push_str("let negative = match byte_get(input, start) { Option::Some { value: byte } => byte == 45u8, Option::None {} => false, };\nlet mut number = 0usize;\nlet mut digit_index = start;\nlet _ = if negative { error = 6; offset = start; false } else {\nwhile error == 0 && digit_index < end {\nlet digit = match byte_get(input, digit_index) { Option::Some { value: byte } => usize_from_u8(byte - 48u8), Option::None {} => 0usize, };\nlet over = number > 1844674407370955161usize || number == 1844674407370955161usize && digit > 5usize;\nif over { error = 6; offset = start; false } else { number = number * 10usize + digit; digit_index = digit_index + 1usize; true }\n}\ntrue\n};\n");
                writeln!(
                    out,
                    "value_{index} = if error == 0 {{ number }} else {{ value_{index} }};\nerror == 0"
                )
                .unwrap();
            } else {
                out.push_str("let number = jc_i64_or(input, start, 0);\nlet other = jc_i64_or(input, start, 1);\n");
                let condition = if f.ty == Type::U8 {
                    "number != other || number < 0 || number > 255"
                } else {
                    "number != other"
                };
                let conversion = if f.ty == Type::U8 {
                    "u8_from_i64(number)"
                } else {
                    "number"
                };
                writeln!(out, "if {condition} {{ error = 6; offset = start; false }} else {{ value_{index} = {conversion}; true }}").unwrap();
            }
            out.push_str("}\n}\n");
        }
        out.push_str("}\n} else { true };\n");
    }
    out.push_str("true\n};\nkey = if error == 0 { jc_next_member(input, key, 32usize) } else { length };\nerror == 0 && key < length\n}\n");
    for (index, _) in fields.iter().enumerate() {
        writeln!(out, "let _ = if error == 0 && !seen_{index} {{ error = 3; offset = length; field = {}; false }} else {{ true }};", index + 1).unwrap();
    }
    write!(
        out,
        "if error == 0 {{ {name}JsonDecode::Decoded {{ value: {name} {{ "
    )
    .unwrap();
    for (index, f) in fields.iter().enumerate() {
        write!(out, "{}: value_{index}, ", f.name).unwrap();
    }
    writeln!(out, "}} }} }} else {{ {name}JsonDecode::Error {{ code: error, offset: offset, field: field }} }}\n}}").unwrap();
    let punctuation = 2 + fields.iter().map(|f| f.name.len() + 3).sum::<usize>() + fields.len() - 1;
    writeln!(
        out,
        "@id(\"{id}.json.encoded-len\")\nfn json_{name}_encoded_len(value: {name}) -> usize {{"
    )
    .unwrap();
    write!(out, "{punctuation}usize").unwrap();
    for f in fields {
        let field = format!("value.{}", f.name);
        let length = match f.ty {
            Type::Bool => format!("if {field} {{ 4usize }} else {{ 5usize }}"),
            Type::Usize => format!("jc_usize_len({field})"),
            Type::U8 => format!("usize_from_i64(jc_i64_len(i64_from_u8({field})))"),
            _ => format!("usize_from_i64(jc_i64_len({field}))"),
        };
        write!(out, " + ({length})").unwrap();
    }
    writeln!(out, "\n}}\n@id(\"{id}.json.encode\")\nfn json_{name}_encode(value: {name}, output_limit: usize) -> {name}JsonEncode {{\nlet required = json_{name}_encoded_len(value);\nif required > output_limit {{ {name}JsonEncode::Refused {{ required: required }} }} else {{").unwrap();
    // Separate immutable owner bindings avoid whole-String renewal admission.
    out.push_str("let output_0 = \"{\";\n");
    for (index, f) in fields.iter().enumerate() {
        let field = format!("value.{}", f.name);
        let rendered = match f.ty {
            Type::Bool => format!("if {field} {{ \"true\" }} else {{ \"false\" }}"),
            Type::Usize => format!("string_from_usize({field})"),
            Type::U8 => format!("string_from_i64(i64_from_u8({field}))"),
            _ => format!("string_from_i64({field})"),
        };
        let label = format!("{}\"{}\":", if index == 0 { "" } else { "," }, f.name);
        let literal = format!("\"{}\"", label.replace('"', "\\\""));
        writeln!(out, "let label_{index} = string_concat(output_{index}, {literal});\nlet rendered_{index} = {rendered};\nlet output_{} = string_concat(label_{index}, rendered_{index});", index + 1).unwrap();
    }
    writeln!(
        out,
        "{name}JsonEncode::Encoded {{ text: string_concat(output_{}, \"}}\") }}\n}}\n}}",
        fields.len()
    )
    .unwrap();
    out
}
