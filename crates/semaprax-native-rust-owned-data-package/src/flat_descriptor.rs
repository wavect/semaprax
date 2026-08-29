use std::collections::BTreeSet;

use serde_json::{Map, Value};
use sha2::{Digest, Sha256};

use super::{flat_descriptor_digest, PackageError, MAX_DESCRIPTOR_BYTES};

pub(crate) const API_SCHEMA: &str = "semaprax.public-flat-owned-record-api.v1";
pub(crate) const PROJECT_SCHEMA: &str = "semaprax.project.v9";

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum FieldKind {
    I64,
    Bool,
    Usize,
    OwnedBytes,
}

impl FieldKind {
    fn parse(value: &str) -> Option<Self> {
        match value {
            "i64" => Some(Self::I64),
            "bool" => Some(Self::Bool),
            "usize" => Some(Self::Usize),
            "owned-bytes" => Some(Self::OwnedBytes),
            _ => None,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct Field {
    pub(crate) stable_id: String,
    source_name: String,
    pub(crate) host_name: String,
    pub(crate) kind: FieldKind,
    pub(crate) ordinal: usize,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct Export {
    pub(crate) stable_id: String,
    pub(crate) rust_method_name: String,
    pub(crate) parameters: Vec<super::descriptor::Parameter>,
    pub(crate) record_id: String,
    record_source_name: String,
    pub(crate) record_host_name: String,
    pub(crate) fields: Vec<Field>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct Descriptor {
    project_revision: String,
    workspace_revision: String,
    project_graph_digest: String,
    pub(crate) exports: Vec<Export>,
}

pub(crate) fn replay(
    bytes: &[u8],
    digest: &str,
    selected: &[String],
) -> Result<Descriptor, PackageError> {
    if bytes.is_empty()
        || bytes.len() > MAX_DESCRIPTOR_BYTES
        || !bytes.ends_with(b"\n")
        || bytes.contains(&0)
        || flat_descriptor_digest(bytes) != digest
    {
        return Err(PackageError::descriptor());
    }
    let value: Value = serde_json::from_slice(bytes).map_err(|_| PackageError::descriptor())?;
    let root = exact_object(&value, 8)?;
    if string(root, "schema")? != API_SCHEMA || string(root, "project_schema")? != PROJECT_SCHEMA {
        return Err(PackageError::descriptor());
    }
    let project_revision = digest_fact(root, "project_revision")?.to_owned();
    let workspace_revision = digest_fact(root, "workspace_revision")?.to_owned();
    let project_graph_digest = digest_fact(root, "project_graph_digest")?.to_owned();
    validate_limits(root.get("limits").ok_or_else(PackageError::descriptor)?)?;
    validate_settlement(
        root.get("settlement")
            .ok_or_else(PackageError::descriptor)?,
    )?;
    let rows = root
        .get("exports")
        .and_then(Value::as_array)
        .filter(|rows| (1..=32).contains(&rows.len()) && rows.len() == selected.len())
        .ok_or_else(PackageError::descriptor)?;
    let mut exports = Vec::with_capacity(rows.len());
    let mut previous: Option<&str> = None;
    let mut methods = BTreeSet::new();
    for (index, row) in rows.iter().enumerate() {
        let row = exact_object(row, 5)?;
        let stable_id = string(row, "stable_id")?;
        if !valid_stable_id(stable_id)
            || previous.is_some_and(|value| value.as_bytes() >= stable_id.as_bytes())
            || selected.get(index).map(String::as_str) != Some(stable_id)
            || string(row, "typescript_name")? != stable_id
        {
            return Err(PackageError::descriptor());
        }
        previous = Some(stable_id);
        let method = rust_method_name(stable_id)?;
        if string(row, "rust_method_name")? != method || !methods.insert(method.clone()) {
            return Err(PackageError::descriptor());
        }
        let parameters = parse_parameters(row.get("parameters"))?;
        let result = exact_object(row.get("result").ok_or_else(PackageError::descriptor)?, 5)?;
        if string(result, "type")? != "flat-owned-record" {
            return Err(PackageError::descriptor());
        }
        let record_id = string(result, "record_id")?;
        let source_name = string(result, "record_source_name")?;
        let record_host_name = string(result, "record_host_name")?;
        if !valid_stable_id(record_id)
            || !valid_source_name(source_name)
            || record_host_name != host_record_name(source_name, record_id)
        {
            return Err(PackageError::descriptor());
        }
        let field_rows = result
            .get("fields")
            .and_then(Value::as_array)
            .filter(|rows| (1..=64).contains(&rows.len()))
            .ok_or_else(PackageError::descriptor)?;
        let mut fields = Vec::with_capacity(field_rows.len());
        let mut field_ids = BTreeSet::new();
        let mut field_names = BTreeSet::new();
        for (ordinal, field) in field_rows.iter().enumerate() {
            let field = exact_object(field, 5)?;
            let field_id = string(field, "stable_id")?;
            let field_source = string(field, "source_name")?;
            let host_name = string(field, "host_name")?;
            if !valid_stable_id(field_id)
                || !valid_source_name(field_source)
                || host_name != host_field_name(field_source, field_id)
                || field.get("ordinal").and_then(Value::as_u64) != Some(ordinal as u64)
                || !field_ids.insert(field_id)
                || !field_names.insert(host_name)
            {
                return Err(PackageError::descriptor());
            }
            let kind =
                FieldKind::parse(string(field, "type")?).ok_or_else(PackageError::descriptor)?;
            fields.push(Field {
                stable_id: field_id.to_owned(),
                source_name: field_source.to_owned(),
                host_name: host_name.to_owned(),
                kind,
                ordinal,
            });
        }
        if fields
            .iter()
            .filter(|field| field.kind == FieldKind::OwnedBytes)
            .count()
            != 1
        {
            return Err(PackageError::descriptor());
        }
        exports.push(Export {
            stable_id: stable_id.to_owned(),
            rust_method_name: method,
            parameters,
            record_id: record_id.to_owned(),
            record_source_name: source_name.to_owned(),
            record_host_name: record_host_name.to_owned(),
            fields,
        });
    }
    let mut records = std::collections::BTreeMap::<&str, (&str, &str, &Vec<Field>)>::new();
    let mut host_records = std::collections::BTreeMap::<&str, &str>::new();
    for export in &exports {
        if let Some(previous) = host_records.insert(&export.record_host_name, &export.record_id) {
            if previous != export.record_id {
                return Err(PackageError::descriptor());
            }
        }
        if let Some((source, host, fields)) = records.insert(
            &export.record_id,
            (
                &export.record_source_name,
                &export.record_host_name,
                &export.fields,
            ),
        ) {
            if source != export.record_source_name
                || host != export.record_host_name
                || fields != &export.fields
            {
                return Err(PackageError::descriptor());
            }
        }
    }
    let descriptor = Descriptor {
        project_revision,
        workspace_revision,
        project_graph_digest,
        exports,
    };
    if render(&descriptor) != bytes {
        return Err(PackageError::descriptor());
    }
    Ok(descriptor)
}

fn render(descriptor: &Descriptor) -> Vec<u8> {
    let mut output = String::new();
    output.push_str("{\"schema\":");
    super::descriptor::json_string(&mut output, API_SCHEMA);
    output.push_str(",\"project_schema\":");
    super::descriptor::json_string(&mut output, PROJECT_SCHEMA);
    output.push_str(",\"project_revision\":");
    super::descriptor::json_string(&mut output, &descriptor.project_revision);
    output.push_str(",\"workspace_revision\":");
    super::descriptor::json_string(&mut output, &descriptor.workspace_revision);
    output.push_str(",\"project_graph_digest\":");
    super::descriptor::json_string(&mut output, &descriptor.project_graph_digest);
    output.push_str(",\"exports\":[");
    for (export_index, export) in descriptor.exports.iter().enumerate() {
        if export_index != 0 {
            output.push(',');
        }
        output.push_str("{\"stable_id\":");
        super::descriptor::json_string(&mut output, &export.stable_id);
        output.push_str(",\"typescript_name\":");
        super::descriptor::json_string(&mut output, &export.stable_id);
        output.push_str(",\"rust_method_name\":");
        super::descriptor::json_string(&mut output, &export.rust_method_name);
        output.push_str(",\"parameters\":[");
        for (ordinal, parameter) in export.parameters.iter().enumerate() {
            if ordinal != 0 {
                output.push(',');
            }
            output.push_str("{\"stable_id\":");
            super::descriptor::json_string(&mut output, &parameter.stable_id);
            output.push_str(",\"source_name\":");
            super::descriptor::json_string(&mut output, &parameter.source_name);
            output.push_str(",\"ordinal\":");
            output.push_str(&ordinal.to_string());
            output.push_str(",\"type\":");
            super::descriptor::json_string(&mut output, parameter.kind.wire_name());
            output.push('}');
        }
        output.push_str("],\"result\":{\"type\":\"flat-owned-record\",\"record_id\":");
        super::descriptor::json_string(&mut output, &export.record_id);
        output.push_str(",\"record_source_name\":");
        super::descriptor::json_string(&mut output, &export.record_source_name);
        output.push_str(",\"record_host_name\":");
        super::descriptor::json_string(&mut output, &export.record_host_name);
        output.push_str(",\"fields\":[");
        for (ordinal, field) in export.fields.iter().enumerate() {
            if ordinal != 0 {
                output.push(',');
            }
            output.push_str("{\"stable_id\":");
            super::descriptor::json_string(&mut output, &field.stable_id);
            output.push_str(",\"source_name\":");
            super::descriptor::json_string(&mut output, &field.source_name);
            output.push_str(",\"host_name\":");
            super::descriptor::json_string(&mut output, &field.host_name);
            output.push_str(",\"ordinal\":");
            output.push_str(&ordinal.to_string());
            output.push_str(",\"type\":");
            super::descriptor::json_string(
                &mut output,
                match field.kind {
                    FieldKind::I64 => "i64",
                    FieldKind::Bool => "bool",
                    FieldKind::Usize => "usize",
                    FieldKind::OwnedBytes => "owned-bytes",
                },
            );
            output.push('}');
        }
        output.push_str("]}}");
    }
    output.push_str("],\"limits\":{\"max_exports\":32,\"max_parameters\":8,\"max_closure_functions\":256,\"max_record_fields\":64,\"max_borrowed_input_bytes\":65536,\"max_owned_output_bytes\":65536,\"max_descriptor_bytes\":1048576},\"settlement\":{\"carrier\":\"opaque-handle-plus-scalars.v1\",\"copy_before_settle\":true,\"publish_after_settle\":true,\"exactly_one_owned_field\":true}}\n");
    output.into_bytes()
}

fn parse_parameters(
    value: Option<&Value>,
) -> Result<Vec<super::descriptor::Parameter>, PackageError> {
    let rows = value
        .and_then(Value::as_array)
        .filter(|rows| rows.len() <= 8)
        .ok_or_else(PackageError::descriptor)?;
    let mut stable_ids = BTreeSet::new();
    rows.iter()
        .enumerate()
        .map(|(ordinal, value)| {
            let row = exact_object(value, 4)?;
            let stable_id = string(row, "stable_id")?;
            let source_name = string(row, "source_name")?;
            if !valid_parameter_id(stable_id)
                || !valid_source_name(source_name)
                || row.get("ordinal").and_then(Value::as_u64) != Some(ordinal as u64)
                || !stable_ids.insert(stable_id)
            {
                return Err(PackageError::descriptor());
            }
            let kind = match string(row, "type")? {
                "i64" => super::ParameterKind::I64,
                "bool" => super::ParameterKind::Bool,
                "borrow-str" => super::ParameterKind::BorrowStr,
                "borrow-slice-u8" => super::ParameterKind::BorrowSliceU8,
                _ => return Err(PackageError::descriptor()),
            };
            Ok(super::descriptor::Parameter {
                stable_id: stable_id.to_owned(),
                source_name: source_name.to_owned(),
                kind,
            })
        })
        .collect()
}

fn validate_limits(value: &Value) -> Result<(), PackageError> {
    let row = exact_object(value, 7)?;
    for (name, expected) in [
        ("max_exports", 32),
        ("max_parameters", 8),
        ("max_closure_functions", 256),
        ("max_record_fields", 64),
        ("max_borrowed_input_bytes", 65_536),
        ("max_owned_output_bytes", 65_536),
        ("max_descriptor_bytes", 1_048_576),
    ] {
        if row.get(name).and_then(Value::as_u64) != Some(expected) {
            return Err(PackageError::descriptor());
        }
    }
    Ok(())
}

fn validate_settlement(value: &Value) -> Result<(), PackageError> {
    let row = exact_object(value, 4)?;
    if string(row, "carrier")? != "opaque-handle-plus-scalars.v1"
        || row.get("copy_before_settle").and_then(Value::as_bool) != Some(true)
        || row.get("publish_after_settle").and_then(Value::as_bool) != Some(true)
        || row.get("exactly_one_owned_field").and_then(Value::as_bool) != Some(true)
    {
        return Err(PackageError::descriptor());
    }
    Ok(())
}

fn exact_object(value: &Value, length: usize) -> Result<&Map<String, Value>, PackageError> {
    value
        .as_object()
        .filter(|row| row.len() == length)
        .ok_or_else(PackageError::descriptor)
}

fn string<'a>(row: &'a Map<String, Value>, name: &str) -> Result<&'a str, PackageError> {
    row.get(name)
        .and_then(Value::as_str)
        .ok_or_else(PackageError::descriptor)
}

fn digest_fact<'a>(row: &'a Map<String, Value>, name: &str) -> Result<&'a str, PackageError> {
    let value = string(row, name)?;
    if value.len() == 71
        && value.starts_with("sha256:")
        && value[7..]
            .bytes()
            .all(|byte| byte.is_ascii_hexdigit() && !byte.is_ascii_uppercase())
    {
        Ok(value)
    } else {
        Err(PackageError::descriptor())
    }
}

fn valid_stable_id(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 128
        && value.bytes().all(|byte| {
            byte.is_ascii_lowercase() || byte.is_ascii_digit() || matches!(byte, b'_' | b'.' | b'-')
        })
}

fn valid_parameter_id(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 512
        && value.bytes().all(|byte| {
            byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'.' | b'-' | b':' | b'#')
        })
}

fn valid_source_name(value: &str) -> bool {
    !value.is_empty() && value.len() <= 128 && !value.chars().any(char::is_control)
}

fn rust_method_name(stable_id: &str) -> Result<String, PackageError> {
    let mut output = String::from("spx_");
    for byte in stable_id.bytes() {
        match byte {
            b'a'..=b'z' | b'0'..=b'9' => output.push(char::from(byte)),
            b'_' => output.push_str("_underscore_"),
            b'.' => output.push_str("_dot_"),
            b'-' => output.push_str("_hyphen_"),
            _ => return Err(PackageError::descriptor()),
        }
    }
    Ok(output)
}

fn stable_host_name(prefix: &str, stable_id: &str) -> String {
    let digest = format!(
        "{:x}",
        super::LowerHex(Sha256::digest(stable_id.as_bytes()))
    );
    match prefix {
        "record" => format!("SpxRecordH{digest}"),
        "field" => format!("spx_field_h{digest}"),
        _ => unreachable!("closed host-name family"),
    }
}

fn host_record_name(source_name: &str, stable_id: &str) -> String {
    if source_name
        .bytes()
        .next()
        .is_some_and(|byte| byte.is_ascii_uppercase())
        && source_name.bytes().all(|byte| byte.is_ascii_alphanumeric())
    {
        source_name.to_owned()
    } else {
        stable_host_name("record", stable_id)
    }
}

fn host_field_name(source_name: &str, stable_id: &str) -> String {
    const KEYWORDS: &[&str] = &[
        "as", "break", "const", "continue", "crate", "else", "enum", "extern", "false", "fn",
        "for", "if", "impl", "in", "let", "loop", "match", "mod", "move", "mut", "pub", "ref",
        "return", "self", "Self", "static", "struct", "super", "trait", "true", "type", "unsafe",
        "use", "where", "while", "async", "await", "dyn",
    ];
    if source_name
        .bytes()
        .next()
        .is_some_and(|byte| byte.is_ascii_lowercase() || byte == b'_')
        && source_name
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || byte == b'_')
        && !KEYWORDS.contains(&source_name)
    {
        source_name.to_owned()
    } else {
        stable_host_name("field", stable_id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn field(id: &str, source: &str, kind: FieldKind, ordinal: usize) -> Field {
        Field {
            stable_id: id.to_owned(),
            source_name: source.to_owned(),
            host_name: host_field_name(source, id),
            kind,
            ordinal,
        }
    }

    fn export(id: &str, record_id: &str, record_source: &str, fields: Vec<Field>) -> Export {
        Export {
            stable_id: id.to_owned(),
            rust_method_name: rust_method_name(id).unwrap(),
            parameters: Vec::new(),
            record_id: record_id.to_owned(),
            record_source_name: record_source.to_owned(),
            record_host_name: host_record_name(record_source, record_id),
            fields,
        }
    }

    fn descriptor(exports: Vec<Export>) -> Descriptor {
        let digest = format!("sha256:{}", "0".repeat(64));
        Descriptor {
            project_revision: digest.clone(),
            workspace_revision: digest.clone(),
            project_graph_digest: digest,
            exports,
        }
    }

    #[test]
    fn replay_requires_exact_canonical_bytes_even_with_a_reminted_digest() {
        let value = descriptor(vec![export(
            "api.first",
            "record.first",
            "Packet",
            vec![field("field.bytes", "bytes", FieldKind::OwnedBytes, 0)],
        )]);
        let canonical = render(&value);
        replay(
            &canonical,
            &flat_descriptor_digest(&canonical),
            &["api.first".to_owned()],
        )
        .unwrap();
        let mut drifted = canonical;
        drifted.splice(1..1, b" ".iter().copied());
        assert!(replay(
            &drifted,
            &flat_descriptor_digest(&drifted),
            &["api.first".to_owned()],
        )
        .is_err());
    }

    #[test]
    fn replay_rejects_cross_export_record_identity_disagreement() {
        let host_collision = descriptor(vec![
            export(
                "api.first",
                "record.first",
                "Packet",
                vec![field("field.first", "bytes", FieldKind::OwnedBytes, 0)],
            ),
            export(
                "api.second",
                "record.second",
                "Packet",
                vec![field("field.second", "bytes", FieldKind::OwnedBytes, 0)],
            ),
        ]);
        let bytes = render(&host_collision);
        assert!(replay(
            &bytes,
            &flat_descriptor_digest(&bytes),
            &["api.first".to_owned(), "api.second".to_owned()],
        )
        .is_err());

        let inconsistent = descriptor(vec![
            export(
                "api.first",
                "record.shared",
                "Packet",
                vec![field("field.first", "bytes", FieldKind::OwnedBytes, 0)],
            ),
            export(
                "api.second",
                "record.shared",
                "Packet",
                vec![
                    field("field.flag", "flag", FieldKind::Bool, 0),
                    field("field.first", "bytes", FieldKind::OwnedBytes, 1),
                ],
            ),
        ]);
        let bytes = render(&inconsistent);
        assert!(replay(
            &bytes,
            &flat_descriptor_digest(&bytes),
            &["api.first".to_owned(), "api.second".to_owned()],
        )
        .is_err());
    }
}
