//! Independent replay of the closed Project-v11 nested-record descriptor.

use std::collections::{BTreeMap, BTreeSet};

use serde_json::{Map, Value};

use super::{nested_descriptor_digest, PackageError};

mod render;
#[cfg(test)]
mod tests;

pub(crate) const API_SCHEMA: &str = "semaprax.public-nested-owned-record-api.v1";
pub(crate) const PROJECT_SCHEMA: &str = "semaprax.project.v11";

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum FieldType {
    I64,
    Bool,
    Usize,
    OwnedBytes,
    Record(String),
}

impl FieldType {
    fn parse(row: &Map<String, Value>) -> Result<Self, PackageError> {
        match string(row, "type")? {
            "i64" if row.len() == 5 => Ok(Self::I64),
            "bool" if row.len() == 5 => Ok(Self::Bool),
            "usize" if row.len() == 5 => Ok(Self::Usize),
            "owned-bytes" if row.len() == 5 => Ok(Self::OwnedBytes),
            "record" if row.len() == 6 => Ok(Self::Record(string(row, "record_id")?.to_owned())),
            _ => Err(PackageError::descriptor()),
        }
    }

    pub(crate) fn wire_name(&self) -> &'static str {
        match self {
            Self::I64 => "i64",
            Self::Bool => "bool",
            Self::Usize => "usize",
            Self::OwnedBytes => "owned-bytes",
            Self::Record(_) => "record",
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct Field {
    pub(crate) stable_id: String,
    pub(crate) source_name: String,
    pub(crate) host_name: String,
    pub(crate) ordinal: usize,
    pub(crate) ty: FieldType,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct Record {
    pub(crate) stable_id: String,
    pub(crate) source_name: String,
    pub(crate) host_name: String,
    pub(crate) fields: Vec<Field>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct Leaf {
    pub(crate) path: Vec<String>,
    pub(crate) ordinal: usize,
    pub(crate) ty: FieldType,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct Export {
    pub(crate) stable_id: String,
    pub(crate) rust_method_name: String,
    pub(crate) parameters: Vec<super::descriptor::Parameter>,
    pub(crate) result_record_id: String,
    pub(crate) leaves: Vec<Leaf>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct Descriptor {
    pub(crate) project_revision: String,
    pub(crate) workspace_revision: String,
    pub(crate) project_graph_digest: String,
    pub(crate) exports: Vec<Export>,
    pub(crate) records: Vec<Record>,
}

pub(crate) fn replay(
    bytes: &[u8],
    digest: &str,
    selected: &[String],
) -> Result<Descriptor, PackageError> {
    super::descriptor::validate_input(bytes)?;
    if nested_descriptor_digest(bytes) != digest {
        return Err(PackageError::descriptor());
    }
    let value: Value = serde_json::from_slice(bytes).map_err(|_| PackageError::descriptor())?;
    let root = exact(&value, 9)?;
    if string(root, "schema")? != API_SCHEMA || string(root, "project_schema")? != PROJECT_SCHEMA {
        return Err(PackageError::descriptor());
    }
    validate_limits(root.get("limits").ok_or_else(PackageError::descriptor)?)?;
    validate_settlement(
        root.get("settlement")
            .ok_or_else(PackageError::descriptor)?,
    )?;
    let records = parse_records(root.get("records"))?;
    let by_id = records
        .iter()
        .map(|record| (record.stable_id.as_str(), record))
        .collect::<BTreeMap<_, _>>();
    validate_record_graph(&records, &by_id)?;
    let exports = parse_exports(root.get("exports"), selected, &by_id)?;
    let mut reachable = BTreeSet::new();
    let mut work = exports
        .iter()
        .map(|export| export.result_record_id.as_str())
        .collect::<Vec<_>>();
    while let Some(id) = work.pop() {
        if !reachable.insert(id) {
            continue;
        }
        let record = by_id.get(id).ok_or_else(PackageError::descriptor)?;
        for field in &record.fields {
            if let FieldType::Record(child) = &field.ty {
                work.push(child);
            }
        }
    }
    if reachable.len() != records.len() {
        return Err(PackageError::descriptor());
    }
    let descriptor = Descriptor {
        project_revision: digest_fact(root, "project_revision")?.to_owned(),
        workspace_revision: digest_fact(root, "workspace_revision")?.to_owned(),
        project_graph_digest: digest_fact(root, "project_graph_digest")?.to_owned(),
        exports,
        records,
    };
    if render::canonical(&descriptor) != bytes {
        return Err(PackageError::descriptor());
    }
    Ok(descriptor)
}

fn parse_records(value: Option<&Value>) -> Result<Vec<Record>, PackageError> {
    let rows = value
        .and_then(Value::as_array)
        .filter(|rows| !rows.is_empty() && rows.len() <= 4096)
        .ok_or_else(PackageError::descriptor)?;
    let mut records = Vec::with_capacity(rows.len());
    let mut previous: Option<&str> = None;
    let mut host_names = BTreeSet::new();
    let mut examined = 0usize;
    for value in rows {
        let row = exact(value, 4)?;
        let stable_id = string(row, "stable_id")?;
        if !valid_identity(stable_id)
            || previous.is_some_and(|old| old.as_bytes() >= stable_id.as_bytes())
        {
            return Err(PackageError::descriptor());
        }
        previous = Some(stable_id);
        let host_name = string(row, "host_name")?;
        if host_name != host_record_name(stable_id) || !host_names.insert(host_name) {
            return Err(PackageError::descriptor());
        }
        let fields = row
            .get("fields")
            .and_then(Value::as_array)
            .ok_or_else(PackageError::descriptor)?;
        examined = examined
            .checked_add(fields.len())
            .filter(|count| *count <= 4096)
            .ok_or_else(PackageError::descriptor)?;
        let mut parsed = Vec::with_capacity(fields.len());
        let mut field_ids = BTreeSet::new();
        let mut field_hosts = BTreeSet::new();
        for (ordinal, value) in fields.iter().enumerate() {
            let row = value
                .as_object()
                .filter(|row| row.len() == 5 || row.len() == 6)
                .ok_or_else(PackageError::descriptor)?;
            let field_id = string(row, "stable_id")?;
            let field_host = string(row, "host_name")?;
            if !valid_identity(field_id)
                || field_host != host_field_name(field_id)
                || row.get("ordinal").and_then(Value::as_u64) != Some(ordinal as u64)
                || !field_ids.insert(field_id)
                || !field_hosts.insert(field_host)
            {
                return Err(PackageError::descriptor());
            }
            parsed.push(Field {
                stable_id: field_id.to_owned(),
                source_name: string(row, "source_name")?.to_owned(),
                host_name: field_host.to_owned(),
                ordinal,
                ty: FieldType::parse(row)?,
            });
        }
        records.push(Record {
            stable_id: stable_id.to_owned(),
            source_name: string(row, "source_name")?.to_owned(),
            host_name: host_name.to_owned(),
            fields: parsed,
        });
    }
    Ok(records)
}

fn validate_record_graph<'a>(
    records: &'a [Record],
    by_id: &BTreeMap<&'a str, &'a Record>,
) -> Result<(), PackageError> {
    let mut state = BTreeMap::<&str, u8>::new();
    for record in records {
        if state.get(record.stable_id.as_str()) == Some(&2) {
            continue;
        }
        let mut stack = vec![(record.stable_id.as_str(), true)];
        while let Some((id, entering)) = stack.pop() {
            if !entering {
                state.insert(id, 2);
                continue;
            }
            match state.get(id).copied().unwrap_or(0) {
                1 => return Err(PackageError::descriptor()),
                2 => continue,
                _ => {}
            }
            state.insert(id, 1);
            let row = by_id.get(id).ok_or_else(PackageError::descriptor)?;
            stack.push((id, false));
            for field in row.fields.iter().rev() {
                if let FieldType::Record(child) = &field.ty {
                    stack.push((child, true));
                }
            }
        }
    }
    Ok(())
}

fn parse_exports<'a>(
    value: Option<&Value>,
    selected: &[String],
    records: &BTreeMap<&'a str, &'a Record>,
) -> Result<Vec<Export>, PackageError> {
    let rows = value
        .and_then(Value::as_array)
        .filter(|rows| (1..=32).contains(&rows.len()) && rows.len() == selected.len())
        .ok_or_else(PackageError::descriptor)?;
    let mut exports = Vec::with_capacity(rows.len());
    let mut previous: Option<&str> = None;
    let mut methods = BTreeSet::new();
    for (index, value) in rows.iter().enumerate() {
        let row = exact(value, 6)?;
        let stable_id = string(row, "stable_id")?;
        let method = rust_method_name(stable_id)?;
        if !valid_stable_id(stable_id)
            || previous.is_some_and(|old| old.as_bytes() >= stable_id.as_bytes())
            || selected.get(index).map(String::as_str) != Some(stable_id)
            || string(row, "typescript_name")? != stable_id
            || string(row, "rust_method_name")? != method
            || !methods.insert(method.clone())
        {
            return Err(PackageError::descriptor());
        }
        previous = Some(stable_id);
        let result_record_id = string(row, "result_record_id")?;
        if !records.contains_key(result_record_id) {
            return Err(PackageError::descriptor());
        }
        let parameters = parse_parameters(row.get("parameters"))?;
        let leaves = parse_leaves(row.get("leaves"))?;
        let expected = flatten_leaves(result_record_id, records)?;
        if leaves != expected {
            return Err(PackageError::descriptor());
        }
        exports.push(Export {
            stable_id: stable_id.to_owned(),
            rust_method_name: method,
            parameters,
            result_record_id: result_record_id.to_owned(),
            leaves,
        });
    }
    Ok(exports)
}

fn flatten_leaves<'a>(
    root: &str,
    records: &BTreeMap<&'a str, &'a Record>,
) -> Result<Vec<Leaf>, PackageError> {
    let mut leaves = Vec::new();
    let mut stack = vec![(FieldType::Record(root.to_owned()), Vec::<String>::new())];
    let mut examined = 0usize;
    while let Some((ty, path)) = stack.pop() {
        if let FieldType::Record(record_id) = ty {
            let record = records
                .get(record_id.as_str())
                .ok_or_else(PackageError::descriptor)?;
            for field in record.fields.iter().rev() {
                let mut next = path.clone();
                next.push(field.stable_id.clone());
                examined = examined
                    .checked_add(1)
                    .filter(|count| *count <= 4096)
                    .ok_or_else(PackageError::descriptor)?;
                if next.len() > 64 {
                    return Err(PackageError::descriptor());
                }
                stack.push((field.ty.clone(), next));
            }
        } else {
            if leaves.len() == 4096 {
                return Err(PackageError::descriptor());
            }
            leaves.push(Leaf {
                path,
                ordinal: 0,
                ty,
            });
        }
    }
    for (ordinal, leaf) in leaves.iter_mut().enumerate() {
        leaf.ordinal = ordinal;
    }
    let owned = leaves
        .iter()
        .filter(|leaf| leaf.ty == FieldType::OwnedBytes)
        .count();
    if owned == 0 || owned > 256 {
        return Err(PackageError::descriptor());
    }
    Ok(leaves)
}

fn parse_leaves(value: Option<&Value>) -> Result<Vec<Leaf>, PackageError> {
    let rows = value
        .and_then(Value::as_array)
        .filter(|rows| !rows.is_empty() && rows.len() <= 4096)
        .ok_or_else(PackageError::descriptor)?;
    rows.iter()
        .enumerate()
        .map(|(ordinal, value)| {
            let row = exact(value, 3)?;
            let path = row
                .get("path")
                .and_then(Value::as_array)
                .filter(|path| !path.is_empty() && path.len() <= 64)
                .ok_or_else(PackageError::descriptor)?
                .iter()
                .map(|part| {
                    part.as_str()
                        .filter(|part| valid_identity(part))
                        .map(str::to_owned)
                        .ok_or_else(PackageError::descriptor)
                })
                .collect::<Result<Vec<_>, _>>()?;
            if row.get("ordinal").and_then(Value::as_u64) != Some(ordinal as u64) {
                return Err(PackageError::descriptor());
            }
            let ty = match string(row, "type")? {
                "i64" => FieldType::I64,
                "bool" => FieldType::Bool,
                "usize" => FieldType::Usize,
                "owned-bytes" => FieldType::OwnedBytes,
                _ => return Err(PackageError::descriptor()),
            };
            Ok(Leaf { path, ordinal, ty })
        })
        .collect()
}

fn parse_parameters(
    value: Option<&Value>,
) -> Result<Vec<super::descriptor::Parameter>, PackageError> {
    let rows = value
        .and_then(Value::as_array)
        .filter(|rows| rows.len() <= 8)
        .ok_or_else(PackageError::descriptor)?;
    let mut ids = BTreeSet::new();
    rows.iter()
        .enumerate()
        .map(|(ordinal, value)| {
            let row = exact(value, 4)?;
            let id = string(row, "stable_id")?;
            if !valid_identity(id)
                || !ids.insert(id)
                || row.get("ordinal").and_then(Value::as_u64) != Some(ordinal as u64)
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
                stable_id: id.to_owned(),
                source_name: string(row, "source_name")?.to_owned(),
                kind,
            })
        })
        .collect()
}

fn validate_limits(value: &Value) -> Result<(), PackageError> {
    let row = exact(value, 9)?;
    for (name, expected) in [
        ("max_exports", 32),
        ("max_parameters", 8),
        ("max_closure_functions", 256),
        ("max_record_depth", 64),
        ("max_owned_leaves", 256),
        ("max_examined_fields", 4096),
        ("max_borrowed_input_bytes", 65536),
        ("max_owned_output_bytes", 65536),
        ("max_descriptor_bytes", 1048576),
    ] {
        if row.get(name).and_then(Value::as_u64) != Some(expected) {
            return Err(PackageError::descriptor());
        }
    }
    Ok(())
}
fn validate_settlement(value: &Value) -> Result<(), PackageError> {
    let row = exact(value, 5)?;
    if string(row, "carrier")? != "opaque-multi-handle-plus-scalars.v1"
        || [
            "preflight_all_handles",
            "batch_attach",
            "copy_all_before_settle",
            "publish_after_settle",
        ]
        .iter()
        .any(|name| row.get(*name).and_then(Value::as_bool) != Some(true))
    {
        return Err(PackageError::descriptor());
    }
    Ok(())
}
fn exact(value: &Value, len: usize) -> Result<&Map<String, Value>, PackageError> {
    value
        .as_object()
        .filter(|row| row.len() == len)
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
            .all(|b| b.is_ascii_hexdigit() && !b.is_ascii_uppercase())
    {
        Ok(value)
    } else {
        Err(PackageError::descriptor())
    }
}
fn valid_stable_id(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 128
        && value.bytes().all(|b| {
            b.is_ascii_lowercase() || b.is_ascii_digit() || matches!(b, b'_' | b'.' | b'-')
        })
}
fn valid_identity(value: &str) -> bool {
    !value.contains('\0')
}
fn rust_method_name(id: &str) -> Result<String, PackageError> {
    let mut out = String::from("spx_");
    for byte in id.bytes() {
        match byte {
            b'a'..=b'z' | b'0'..=b'9' => out.push(char::from(byte)),
            b'_' => out.push_str("_underscore_"),
            b'.' => out.push_str("_dot_"),
            b'-' => out.push_str("_hyphen_"),
            _ => return Err(PackageError::descriptor()),
        }
    }
    Ok(out)
}
fn stable_host_name(prefix: &str, id: &str) -> String {
    use std::fmt::Write as _;
    let mut out = if prefix == "record" {
        String::from("SpxRecordId")
    } else {
        String::from("spx_field_id_")
    };
    for byte in id.bytes() {
        write!(out, "{byte:02x}").unwrap();
    }
    out
}
fn host_record_name(id: &str) -> String {
    stable_host_name("record", id)
}
fn host_field_name(id: &str) -> String {
    stable_host_name("field", id)
}
