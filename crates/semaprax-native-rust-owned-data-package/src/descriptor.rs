use std::collections::BTreeSet;

use serde_json::{Map, Value};

use super::{
    descriptor_digest, PackageError, MAX_DESCRIPTOR_BYTES, PUBLIC_OWNED_DATA_API_SCHEMA,
    PUBLIC_OWNED_DATA_PROJECT_SCHEMA,
};

const MAX_EXPORTS: usize = 32;
const MAX_PARAMETERS: usize = 8;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ParameterKind {
    I64,
    Bool,
    BorrowStr,
    BorrowSliceU8,
}

impl ParameterKind {
    pub(crate) const fn wire_name(self) -> &'static str {
        match self {
            Self::I64 => "i64",
            Self::Bool => "bool",
            Self::BorrowStr => "borrow-str",
            Self::BorrowSliceU8 => "borrow-slice-u8",
        }
    }

    fn parse(value: &str) -> Option<Self> {
        match value {
            "i64" => Some(Self::I64),
            "bool" => Some(Self::Bool),
            "borrow-str" => Some(Self::BorrowStr),
            "borrow-slice-u8" => Some(Self::BorrowSliceU8),
            _ => None,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ResultKind {
    I64,
    Bool,
    Usize,
    OwnedBytes,
    OptionOwnedBytes,
    ResultOwnedBytesI64,
}

impl ResultKind {
    pub(crate) const fn wire_name(self) -> &'static str {
        match self {
            Self::I64 => "i64",
            Self::Bool => "bool",
            Self::Usize => "usize",
            Self::OwnedBytes => "owned-bytes",
            Self::OptionOwnedBytes => "option-owned-bytes",
            Self::ResultOwnedBytesI64 => "result-owned-bytes-i64",
        }
    }

    fn parse(value: &str) -> Option<Self> {
        match value {
            "i64" => Some(Self::I64),
            "bool" => Some(Self::Bool),
            "usize" => Some(Self::Usize),
            "owned-bytes" => Some(Self::OwnedBytes),
            "option-owned-bytes" => Some(Self::OptionOwnedBytes),
            "result-owned-bytes-i64" => Some(Self::ResultOwnedBytesI64),
            _ => None,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Parameter {
    pub(crate) stable_id: String,
    pub(crate) source_name: String,
    pub(crate) kind: ParameterKind,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Export {
    pub(crate) stable_id: String,
    pub(crate) rust_method_name: String,
    pub(crate) parameters: Vec<Parameter>,
    pub(crate) result: ResultKind,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Descriptor {
    project_revision: String,
    workspace_revision: String,
    project_graph_digest: String,
    pub(crate) exports: Vec<Export>,
}

impl Descriptor {
    pub fn exports_len(&self) -> usize {
        self.exports.len()
    }
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
        || descriptor_digest(bytes) != digest
    {
        return Err(PackageError::descriptor());
    }
    let value: Value = serde_json::from_slice(bytes).map_err(|_| PackageError::descriptor())?;
    let root = exact_object(&value, 7)?;
    if string(root, "schema")? != PUBLIC_OWNED_DATA_API_SCHEMA
        || string(root, "project_schema")? != PUBLIC_OWNED_DATA_PROJECT_SCHEMA
    {
        return Err(PackageError::descriptor());
    }
    let project_revision = digest_fact(root, "project_revision")?.to_owned();
    let workspace_revision = digest_fact(root, "workspace_revision")?.to_owned();
    let project_graph_digest = digest_fact(root, "project_graph_digest")?.to_owned();
    let rows = root
        .get("exports")
        .and_then(Value::as_array)
        .filter(|rows| (1..=MAX_EXPORTS).contains(&rows.len()))
        .ok_or_else(PackageError::descriptor)?;
    if selected.len() != rows.len() {
        return Err(PackageError::descriptor());
    }
    let mut exports = Vec::with_capacity(rows.len());
    let mut previous: Option<&str> = None;
    let mut rust_names = BTreeSet::new();
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
        let rust_method_name = rust_method_name(stable_id)?;
        if string(row, "rust_method_name")? != rust_method_name
            || !rust_names.insert(rust_method_name.clone())
        {
            return Err(PackageError::descriptor());
        }
        let parameter_rows = row
            .get("parameters")
            .and_then(Value::as_array)
            .filter(|rows| rows.len() <= MAX_PARAMETERS)
            .ok_or_else(PackageError::descriptor)?;
        let mut parameters = Vec::with_capacity(parameter_rows.len());
        for (ordinal, parameter) in parameter_rows.iter().enumerate() {
            let parameter = exact_object(parameter, 4)?;
            let stable_id = string(parameter, "stable_id")?;
            let source_name = string(parameter, "source_name")?;
            if !valid_parameter_id(stable_id)
                || !valid_source_name(source_name)
                || parameter.get("ordinal").and_then(Value::as_u64) != Some(ordinal as u64)
            {
                return Err(PackageError::descriptor());
            }
            let kind = ParameterKind::parse(string(parameter, "type")?)
                .ok_or_else(PackageError::descriptor)?;
            parameters.push(Parameter {
                stable_id: stable_id.to_owned(),
                source_name: source_name.to_owned(),
                kind,
            });
        }
        let result =
            ResultKind::parse(string(row, "result")?).ok_or_else(PackageError::descriptor)?;
        exports.push(Export {
            stable_id: stable_id.to_owned(),
            rust_method_name,
            parameters,
            result,
        });
    }
    validate_limits(root.get("limits").ok_or_else(PackageError::descriptor)?)?;
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
    json_string(&mut output, PUBLIC_OWNED_DATA_API_SCHEMA);
    output.push_str(",\"project_schema\":");
    json_string(&mut output, PUBLIC_OWNED_DATA_PROJECT_SCHEMA);
    output.push_str(",\"project_revision\":");
    json_string(&mut output, &descriptor.project_revision);
    output.push_str(",\"workspace_revision\":");
    json_string(&mut output, &descriptor.workspace_revision);
    output.push_str(",\"project_graph_digest\":");
    json_string(&mut output, &descriptor.project_graph_digest);
    output.push_str(",\"exports\":[");
    for (index, export) in descriptor.exports.iter().enumerate() {
        if index != 0 {
            output.push(',');
        }
        output.push_str("{\"stable_id\":");
        json_string(&mut output, &export.stable_id);
        output.push_str(",\"typescript_name\":");
        json_string(&mut output, &export.stable_id);
        output.push_str(",\"rust_method_name\":");
        json_string(&mut output, &export.rust_method_name);
        output.push_str(",\"parameters\":[");
        for (ordinal, parameter) in export.parameters.iter().enumerate() {
            if ordinal != 0 {
                output.push(',');
            }
            output.push_str("{\"stable_id\":");
            json_string(&mut output, &parameter.stable_id);
            output.push_str(",\"source_name\":");
            json_string(&mut output, &parameter.source_name);
            output.push_str(",\"ordinal\":");
            output.push_str(&ordinal.to_string());
            output.push_str(",\"type\":");
            json_string(&mut output, parameter.kind.wire_name());
            output.push('}');
        }
        output.push_str("],\"result\":");
        json_string(&mut output, export.result.wire_name());
        output.push('}');
    }
    output.push_str("],\"limits\":{\"max_exports\":32,\"max_parameters\":8,\"max_closure_functions\":256,\"max_borrowed_input_bytes\":65536,\"max_owned_output_bytes\":65536,\"max_descriptor_bytes\":1048576}}\n");
    output.into_bytes()
}

fn validate_limits(value: &Value) -> Result<(), PackageError> {
    let row = exact_object(value, 6)?;
    for (name, expected) in [
        ("max_exports", 32),
        ("max_parameters", 8),
        ("max_closure_functions", 256),
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
    let mut output = String::with_capacity(stable_id.len().saturating_mul(12).saturating_add(4));
    output.push_str("spx_");
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

pub(crate) fn json_string(output: &mut String, value: &str) {
    output.push_str(&serde_json::to_string(value).expect("JSON string serialization cannot fail"));
}
