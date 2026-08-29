use std::collections::BTreeSet;

use serde_json::Value;

use super::super::model::Report;

pub(super) fn reachable_shared_types(
    shared: &BTreeSet<String>,
    report: &Report,
) -> BTreeSet<String> {
    let mut seen = BTreeSet::new();
    let mut pending = Vec::new();
    for id in shared {
        if let Some(export) = report.exports.get(id) {
            collect_declarations(export, &mut pending);
        }
    }
    while let Some(id) = pending.pop() {
        if seen.insert(id.clone()) {
            if let Some(row) = report.types.get(&id) {
                collect_declarations(row, &mut pending);
            }
        }
    }
    seen
}

fn collect_declarations(value: &Value, out: &mut Vec<String>) {
    match value {
        Value::Object(map) => {
            if let Some(id) = map.get("declaration").and_then(Value::as_str) {
                out.push(id.to_owned());
            }
            for value in map.values() {
                collect_declarations(value, out);
            }
        }
        Value::Array(values) => {
            for value in values {
                collect_declarations(value, out)
            }
        }
        _ => {}
    }
}

pub(super) fn scrub_type_display(mut value: Value) -> Value {
    if let Some(row) = value.as_object_mut() {
        row.remove("name");
        if let Some(parameters) = row.get_mut("type_parameters").and_then(Value::as_array_mut) {
            for parameter in parameters {
                if let Some(object) = parameter.as_object_mut() {
                    object.remove("name");
                }
            }
        }
        if let Some(definition) = row.get_mut("definition").and_then(Value::as_object_mut) {
            if let Some(fields) = definition.get_mut("fields").and_then(Value::as_array_mut) {
                for field in fields {
                    if let Some(object) = field.as_object_mut() {
                        object.remove("name");
                    }
                }
            }
            if let Some(cases) = definition.get_mut("cases").and_then(Value::as_array_mut) {
                for case in cases {
                    if let Some(object) = case.as_object_mut() {
                        object.remove("name");
                        if let Some(fields) = object.get_mut("fields").and_then(Value::as_array_mut)
                        {
                            for field in fields {
                                if let Some(field) = field.as_object_mut() {
                                    field.remove("name");
                                }
                            }
                        }
                    }
                }
            }
        }
    }
    value
}
