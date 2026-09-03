use serde_json::{Map, Value};

use crate::diagnostic::Diagnostic;

use super::{error, MAX_NESTED_RECORD_DEPTH, MAX_NESTED_RECORD_VISITED_FIELDS};

pub(super) fn validate_closed_descriptor(root: &Map<String, Value>) -> Result<(), Diagnostic> {
    exact_keys(
        root,
        &[
            "schema",
            "project_schema",
            "project_revision",
            "workspace_revision",
            "project_graph_digest",
            "exports",
            "records",
            "limits",
            "settlement",
        ],
    )?;
    for key in [
        "project_revision",
        "workspace_revision",
        "project_graph_digest",
    ] {
        require_string(root, key)?;
    }
    let exports = array(root, "exports")?;
    if exports.is_empty() || exports.len() > 32 {
        return Err(error(
            "nested owned-record descriptor export inventory is invalid",
        ));
    }
    for export in exports {
        let export = object(export)?;
        exact_keys(
            export,
            &[
                "stable_id",
                "typescript_name",
                "rust_method_name",
                "parameters",
                "result_record_id",
                "leaves",
            ],
        )?;
        for key in [
            "stable_id",
            "typescript_name",
            "rust_method_name",
            "result_record_id",
        ] {
            require_string(export, key)?;
        }
        let parameters = array(export, "parameters")?;
        if parameters.len() > 8 {
            return Err(error(
                "nested owned-record descriptor parameter inventory is invalid",
            ));
        }
        for (ordinal, parameter) in parameters.iter().enumerate() {
            let parameter = object(parameter)?;
            exact_keys(parameter, &["stable_id", "source_name", "ordinal", "type"])?;
            require_string(parameter, "stable_id")?;
            require_string(parameter, "source_name")?;
            if require_u64(parameter, "ordinal")? != ordinal as u64 {
                return Err(error(
                    "nested owned-record descriptor parameter order is invalid",
                ));
            }
            require_tag(
                parameter,
                "type",
                &["i64", "bool", "borrow-str", "borrow-slice-u8"],
            )?;
        }
        let leaves = array(export, "leaves")?;
        if leaves.is_empty() || leaves.len() > MAX_NESTED_RECORD_VISITED_FIELDS {
            return Err(error(
                "nested owned-record descriptor leaf inventory is invalid",
            ));
        }
        let mut owned_leaves = 0usize;
        for (ordinal, leaf) in leaves.iter().enumerate() {
            let leaf = object(leaf)?;
            exact_keys(leaf, &["path", "ordinal", "type"])?;
            let path = array(leaf, "path")?;
            if path.is_empty()
                || path.len() > MAX_NESTED_RECORD_DEPTH
                || path.iter().any(|part| part.as_str().is_none())
            {
                return Err(error("nested owned-record descriptor leaf path is invalid"));
            }
            if require_u64(leaf, "ordinal")? != ordinal as u64 {
                return Err(error(
                    "nested owned-record descriptor leaf order is invalid",
                ));
            }
            require_tag(leaf, "type", &["i64", "bool", "usize", "owned-bytes"])?;
            if leaf.get("type").and_then(Value::as_str) == Some("owned-bytes") {
                owned_leaves = owned_leaves.checked_add(1).ok_or_else(|| {
                    error("nested owned-record descriptor owned-leaf inventory is invalid")
                })?;
            }
        }
        if owned_leaves == 0 || owned_leaves > 256 {
            return Err(error(
                "nested owned-record descriptor owned-leaf inventory is invalid",
            ));
        }
    }
    let records = array(root, "records")?;
    if records.is_empty() || records.len() > MAX_NESTED_RECORD_VISITED_FIELDS {
        return Err(error(
            "nested owned-record descriptor record inventory is invalid",
        ));
    }
    let mut examined_fields = 0usize;
    let mut previous_record_id: Option<&str> = None;
    for record in records {
        let record = object(record)?;
        exact_keys(record, &["stable_id", "source_name", "host_name", "fields"])?;
        for key in ["stable_id", "source_name", "host_name"] {
            require_string(record, key)?;
        }
        let record_id = require_string(record, "stable_id")?;
        if previous_record_id.is_some_and(|previous| previous >= record_id) {
            return Err(error(
                "nested owned-record descriptor record order is invalid",
            ));
        }
        previous_record_id = Some(record_id);
        for (ordinal, field) in array(record, "fields")?.iter().enumerate() {
            examined_fields = examined_fields.checked_add(1).ok_or_else(|| {
                error("nested owned-record descriptor field inventory is invalid")
            })?;
            if examined_fields > MAX_NESTED_RECORD_VISITED_FIELDS {
                return Err(error(
                    "nested owned-record descriptor field inventory is invalid",
                ));
            }
            let field = object(field)?;
            let tag = field
                .get("type")
                .and_then(Value::as_str)
                .ok_or_else(|| error("nested owned-record descriptor field type is invalid"))?;
            if tag == "record" {
                exact_keys(
                    field,
                    &[
                        "stable_id",
                        "source_name",
                        "host_name",
                        "ordinal",
                        "type",
                        "record_id",
                    ],
                )?;
                require_string(field, "record_id")?;
            } else {
                exact_keys(
                    field,
                    &["stable_id", "source_name", "host_name", "ordinal", "type"],
                )?;
                if !matches!(tag, "i64" | "bool" | "usize" | "owned-bytes") {
                    return Err(error(
                        "nested owned-record descriptor field type is invalid",
                    ));
                }
            }
            for key in ["stable_id", "source_name", "host_name"] {
                require_string(field, key)?;
            }
            if require_u64(field, "ordinal")? != ordinal as u64 {
                return Err(error(
                    "nested owned-record descriptor field order is invalid",
                ));
            }
        }
    }
    let limits = object(
        root.get("limits")
            .ok_or_else(|| error("nested owned-record descriptor limits are absent"))?,
    )?;
    exact_keys(
        limits,
        &[
            "max_exports",
            "max_parameters",
            "max_closure_functions",
            "max_record_depth",
            "max_owned_leaves",
            "max_examined_fields",
            "max_borrowed_input_bytes",
            "max_owned_output_bytes",
            "max_descriptor_bytes",
        ],
    )?;
    for (key, expected) in [
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
        if require_u64(limits, key)? != expected {
            return Err(error(
                "nested owned-record descriptor limits are not canonical",
            ));
        }
    }
    let settlement = object(
        root.get("settlement")
            .ok_or_else(|| error("nested owned-record descriptor settlement is absent"))?,
    )?;
    exact_keys(
        settlement,
        &[
            "carrier",
            "preflight_all_handles",
            "batch_attach",
            "copy_all_before_settle",
            "publish_after_settle",
        ],
    )?;
    if settlement.get("carrier").and_then(Value::as_str)
        != Some("opaque-multi-handle-plus-scalars.v1")
        || [
            "preflight_all_handles",
            "batch_attach",
            "copy_all_before_settle",
            "publish_after_settle",
        ]
        .iter()
        .any(|key| settlement.get(*key).and_then(Value::as_bool) != Some(true))
    {
        return Err(error(
            "nested owned-record descriptor settlement is not canonical",
        ));
    }
    Ok(())
}

fn exact_keys(object: &Map<String, Value>, expected: &[&str]) -> Result<(), Diagnostic> {
    if object.len() != expected.len() || object.keys().any(|key| !expected.contains(&key.as_str()))
    {
        return Err(error("nested owned-record descriptor object is not closed"));
    }
    Ok(())
}
fn object(value: &Value) -> Result<&Map<String, Value>, Diagnostic> {
    value
        .as_object()
        .ok_or_else(|| error("nested owned-record descriptor object is invalid"))
}
fn array<'a>(object: &'a Map<String, Value>, key: &str) -> Result<&'a Vec<Value>, Diagnostic> {
    object
        .get(key)
        .and_then(Value::as_array)
        .ok_or_else(|| error("nested owned-record descriptor array is invalid"))
}
fn require_string<'a>(object: &'a Map<String, Value>, key: &str) -> Result<&'a str, Diagnostic> {
    object
        .get(key)
        .and_then(Value::as_str)
        .ok_or_else(|| error("nested owned-record descriptor string is invalid"))
}
fn require_u64(object: &Map<String, Value>, key: &str) -> Result<u64, Diagnostic> {
    object
        .get(key)
        .and_then(Value::as_u64)
        .ok_or_else(|| error("nested owned-record descriptor integer is invalid"))
}
fn require_tag(object: &Map<String, Value>, key: &str, tags: &[&str]) -> Result<(), Diagnostic> {
    let value = require_string(object, key)?;
    if tags.contains(&value) {
        Ok(())
    } else {
        Err(error("nested owned-record descriptor tag is invalid"))
    }
}
