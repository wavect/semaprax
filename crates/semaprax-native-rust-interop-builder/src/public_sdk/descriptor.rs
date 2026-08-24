//! Canonical build intent and exact private-descriptor replay.

use super::*;

pub(super) fn canonical_spec(
    module: &str,
    source_revision: &str,
    target: &str,
    options: &NativeRustSdkOptions,
) -> Result<String, Diagnostic> {
    let mut output = String::with_capacity(8192);
    output.push_str("{\"schema\":");
    json_string(&mut output, SPEC_SCHEMA);
    output.push_str(",\"module\":");
    json_string(&mut output, module);
    output.push_str(",\"source_revision\":");
    json_string(&mut output, source_revision);
    output.push_str(",\"target\":{\"triple\":");
    json_string(&mut output, target);
    output.push_str(",\"pointer_width\":64,\"endian\":\"little\",\"panic_strategy\":\"unwind\",\"thread_policy\":\"same_thread\"},\"exports\":");
    string_array(&mut output, &options.exports);
    output.push_str(",\"imports\":");
    string_array(&mut output, &options.imports);
    output.push_str(",\"capabilities\":");
    string_array(&mut output, &options.capabilities);
    output.push_str(",\"limits\":");
    output.push_str(LIMITS_JSON);
    output.push_str(",\"nonclaims\":[");
    for (index, value) in INNER_NONCLAIMS.iter().enumerate() {
        if index != 0 {
            output.push(',');
        }
        json_string(&mut output, value);
    }
    output.push_str("]}\n");
    if output.len() > MAX_SPEC_BYTES {
        return Err(sdk_error(
            "Native Rust SDK canonical intent exceeds its bound",
        ));
    }
    Ok(output)
}

fn descriptor_object<'a>(
    value: &'a Value,
    key: &str,
) -> Result<&'a Map<String, Value>, Diagnostic> {
    value
        .get(key)
        .and_then(Value::as_object)
        .ok_or_else(|| sdk_error("Native Rust SDK descriptor replay failed"))
}

fn descriptor_scalar(value: &Value, allow_unit: bool) -> Result<Scalar, Diagnostic> {
    let row = value
        .as_object()
        .ok_or_else(|| sdk_error("Native Rust SDK descriptor replay failed"))?;
    if row.len() != 2 || row.get("out_slot").and_then(Value::as_bool).is_none() {
        return Err(sdk_error("Native Rust SDK descriptor replay failed"));
    }
    let scalar = match row.get("type").and_then(Value::as_str) {
        Some("unit") if allow_unit => Scalar::Unit,
        Some("i64") => Scalar::I64,
        Some("bool") => Scalar::Bool,
        _ => return Err(sdk_error("Native Rust SDK descriptor replay failed")),
    };
    if row.get("out_slot").and_then(Value::as_bool) != Some(scalar != Scalar::Unit) {
        return Err(sdk_error("Native Rust SDK descriptor replay failed"));
    }
    Ok(scalar)
}

fn descriptor_parameters(value: &Value) -> Result<Vec<Parameter>, Diagnostic> {
    let rows = value
        .as_array()
        .filter(|rows| rows.len() <= 8)
        .ok_or_else(|| sdk_error("Native Rust SDK descriptor replay failed"))?;
    rows.iter()
        .map(|row| {
            let row = row
                .as_object()
                .ok_or_else(|| sdk_error("Native Rust SDK descriptor replay failed"))?;
            if row.len() != 3 || row.get("mode").and_then(Value::as_str) != Some("value") {
                return Err(sdk_error("Native Rust SDK descriptor replay failed"));
            }
            let name = row
                .get("name")
                .and_then(Value::as_str)
                .filter(|name| !name.is_empty() && name.len() <= MAX_IDENTIFIER_BYTES)
                .ok_or_else(|| sdk_error("Native Rust SDK descriptor replay failed"))?;
            let ty = match row.get("type").and_then(Value::as_str) {
                Some("i64") => Scalar::I64,
                Some("bool") => Scalar::Bool,
                _ => return Err(sdk_error("Native Rust SDK descriptor replay failed")),
            };
            Ok(Parameter {
                name: name.to_owned(),
                ty,
            })
        })
        .collect()
}

pub(super) fn parse_descriptor(
    bytes: &[u8],
    expected_module: &str,
    expected_revision: &str,
    expected_target: &str,
    options: &NativeRustSdkOptions,
) -> Result<DescriptorFacts, Diagnostic> {
    parse_descriptor_for_subject(
        bytes,
        expected_module,
        expected_target,
        options,
        DescriptorSubjectExpectation::Source {
            revision: expected_revision,
        },
    )
}

pub(super) fn parse_project_descriptor(
    bytes: &[u8],
    expected_module: &str,
    expected_subject_digest: &str,
    expected_target: &str,
    options: &NativeRustSdkOptions,
) -> Result<DescriptorFacts, Diagnostic> {
    parse_descriptor_for_subject(
        bytes,
        expected_module,
        expected_target,
        options,
        DescriptorSubjectExpectation::Project {
            subject_digest: expected_subject_digest,
        },
    )
}

#[derive(Clone, Copy)]
enum DescriptorSubjectExpectation<'a> {
    Source { revision: &'a str },
    Project { subject_digest: &'a str },
}

impl<'a> DescriptorSubjectExpectation<'a> {
    const fn schema(self) -> &'static str {
        match self {
            Self::Source { .. } => DESCRIPTOR_SCHEMA,
            Self::Project { .. } => PROJECT_DESCRIPTOR_SCHEMA,
        }
    }

    const fn revision_key(self) -> &'static str {
        match self {
            Self::Source { .. } => "source_revision",
            Self::Project { .. } => "project_subject_digest",
        }
    }

    const fn revision(self) -> &'a str {
        match self {
            Self::Source { revision } => revision,
            Self::Project { subject_digest } => subject_digest,
        }
    }
}

fn parse_descriptor_for_subject(
    bytes: &[u8],
    expected_module: &str,
    expected_target: &str,
    options: &NativeRustSdkOptions,
    subject: DescriptorSubjectExpectation<'_>,
) -> Result<DescriptorFacts, Diagnostic> {
    if bytes.len() > MAX_DESCRIPTOR_BYTES || !bytes.ends_with(b"\n") {
        return Err(sdk_error("Native Rust SDK descriptor replay failed"));
    }
    let value: Value = serde_json::from_slice(bytes)
        .map_err(|_| sdk_error("Native Rust SDK descriptor replay failed"))?;
    let root = value
        .as_object()
        .filter(|root| root.len() == 11)
        .ok_or_else(|| sdk_error("Native Rust SDK descriptor replay failed"))?;
    let target = descriptor_object(&value, "target")?;
    if root.get("schema").and_then(Value::as_str) != Some(subject.schema())
        || root.get("module").and_then(Value::as_str) != Some(expected_module)
        || root.get(subject.revision_key()).and_then(Value::as_str) != Some(subject.revision())
        || target.len() != 5
        || target.get("triple").and_then(Value::as_str) != Some(expected_target)
        || target.get("pointer_width").and_then(Value::as_u64) != Some(64)
        || target.get("endian").and_then(Value::as_str) != Some("little")
        || target.get("panic_strategy").and_then(Value::as_str) != Some("unwind")
        || target.get("thread_policy").and_then(Value::as_str) != Some("same_thread")
    {
        return Err(sdk_error("Native Rust SDK descriptor replay failed"));
    }
    let exports = root
        .get("exports")
        .and_then(Value::as_array)
        .filter(|rows| rows.len() == options.exports.len())
        .ok_or_else(|| sdk_error("Native Rust SDK descriptor replay failed"))?;
    let imports = root
        .get("imports")
        .and_then(Value::as_array)
        .filter(|rows| rows.len() == options.imports.len())
        .ok_or_else(|| sdk_error("Native Rust SDK descriptor replay failed"))?;

    let exports = exports
        .iter()
        .zip(&options.exports)
        .map(|(row, expected)| {
            let row = row
                .as_object()
                .ok_or_else(|| sdk_error("Native Rust SDK descriptor replay failed"))?;
            let id = row.get("id").and_then(Value::as_str).unwrap_or_default();
            let inner = row
                .get("rust_method")
                .and_then(Value::as_str)
                .unwrap_or_default();
            if id != expected || inner != format!("export_{}", full_hash(id)) {
                return Err(sdk_error("Native Rust SDK descriptor replay failed"));
            }
            Ok(Export {
                id: id.to_owned(),
                public_method: encode_stable_id(id)?,
                inner_method: inner.to_owned(),
                parameters: descriptor_parameters(
                    row.get("parameters")
                        .ok_or_else(|| sdk_error("Native Rust SDK descriptor replay failed"))?,
                )?,
                result: descriptor_scalar(
                    row.get("result")
                        .ok_or_else(|| sdk_error("Native Rust SDK descriptor replay failed"))?,
                    false,
                )?,
            })
        })
        .collect::<Result<Vec<_>, _>>()?;
    let imports = imports
        .iter()
        .zip(&options.imports)
        .map(|(row, expected)| {
            let row = row
                .as_object()
                .ok_or_else(|| sdk_error("Native Rust SDK descriptor replay failed"))?;
            let id = row.get("id").and_then(Value::as_str).unwrap_or_default();
            let inner = row
                .get("rust_method")
                .and_then(Value::as_str)
                .unwrap_or_default();
            if id != expected || inner != format!("import_{}", full_hash(id)) {
                return Err(sdk_error("Native Rust SDK descriptor replay failed"));
            }
            Ok(Import {
                id: id.to_owned(),
                public_method: encode_stable_id(id)?,
                inner_method: inner.to_owned(),
                parameters: descriptor_parameters(
                    row.get("parameters")
                        .ok_or_else(|| sdk_error("Native Rust SDK descriptor replay failed"))?,
                )?,
                result: descriptor_scalar(
                    row.get("result")
                        .ok_or_else(|| sdk_error("Native Rust SDK descriptor replay failed"))?,
                    true,
                )?,
                failure_domain: {
                    let failure = row
                        .get("failure")
                        .and_then(Value::as_object)
                        .ok_or_else(|| sdk_error("Native Rust SDK descriptor replay failed"))?;
                    match failure.get("kind").and_then(Value::as_str) {
                        Some("infallible") if failure.len() == 1 => None,
                        Some("status") if failure.len() == 2 => Some(
                            failure
                                .get("domain_id")
                                .and_then(Value::as_str)
                                .ok_or_else(|| {
                                    sdk_error("Native Rust SDK descriptor replay failed")
                                })?
                                .to_owned(),
                        ),
                        _ => {
                            return Err(sdk_error("Native Rust SDK descriptor replay failed"));
                        }
                    }
                },
            })
        })
        .collect::<Result<Vec<_>, _>>()?;
    let mut public_names = BTreeSet::new();
    if exports
        .iter()
        .map(|fact| &fact.public_method)
        .chain(imports.iter().map(|fact| &fact.public_method))
        .any(|name| !public_names.insert(name.clone()))
    {
        return Err(sdk_error("Native Rust SDK stable method encoding collided"));
    }
    Ok(DescriptorFacts {
        module: expected_module.to_owned(),
        source_revision: subject.revision().to_owned(),
        target: expected_target.to_owned(),
        exports,
        imports,
    })
}
