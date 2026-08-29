use serde_json::Value;

use crate::diagnostic::Diagnostic;

use super::{fact, push, strings, Finding};

pub(super) fn compare_export(
    id: &str,
    left: &Value,
    right: &Value,
    findings: &mut Vec<Finding>,
    breaking: &mut bool,
) -> Result<(), Diagnostic> {
    if left["name"] != right["name"] {
        push(
            findings,
            "informational",
            "display",
            id,
            left["name"].as_str().unwrap_or(""),
            right["name"].as_str().unwrap_or(""),
            "display_name_changed",
        )?;
    }
    let mut left_parameters = left["parameters"].clone();
    let mut right_parameters = right["parameters"].clone();
    scrub_parameter_names(&mut left_parameters);
    scrub_parameter_names(&mut right_parameters);
    if left_parameters != right_parameters {
        *breaking = true;
        push(
            findings,
            "breaking",
            "interface",
            &format!("{id}:parameters"),
            &fact(&left_parameters),
            &fact(&right_parameters),
            "recursive_type_or_ownership_changed",
        )?;
    } else if left["parameters"] != right["parameters"] {
        push(
            findings,
            "informational",
            "display",
            &format!("{id}:parameters"),
            &fact(&left["parameters"]),
            &fact(&right["parameters"]),
            "parameter_display_name_changed",
        )?;
    }
    for key in ["result"] {
        if left[key] != right[key] {
            *breaking = true;
            push(
                findings,
                "breaking",
                "interface",
                &format!("{id}:{key}"),
                &fact(&left[key]),
                &fact(&right[key]),
                "recursive_type_or_ownership_changed",
            )?;
        }
    }
    let le = strings(&left["effects"]);
    let re = strings(&right["effects"]);
    for effect in re.difference(&le) {
        *breaking = true;
        push(
            findings,
            "breaking",
            "effects",
            &format!("{id}:{effect}"),
            "absent",
            "present",
            "effect_added",
        )?;
    }
    for effect in le.difference(&re) {
        push(
            findings,
            "nonbreaking",
            "effects",
            &format!("{id}:{effect}"),
            "present",
            "absent",
            "effect_removed",
        )?;
    }
    for key in ["requires", "ensures"] {
        if left[key] != right[key] {
            *breaking = true;
            push(
                findings,
                "breaking",
                "contracts",
                &format!("{id}:{key}"),
                &fact(&left[key]),
                &fact(&right[key]),
                "ordered_contract_vector_changed",
            )?;
        }
    }
    Ok(())
}

fn scrub_parameter_names(value: &mut Value) {
    if let Some(parameters) = value.as_array_mut() {
        for parameter in parameters {
            if let Some(object) = parameter.as_object_mut() {
                object.remove("name");
            }
        }
    }
}
