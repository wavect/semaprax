//! Coordinate-qualified checked Agent metadata; no lifecycle ABI admission.
use super::{binding, limit, ConstructionBudget};
use crate::diagnostic::Diagnostic;
use crate::package_lock_v2::Coordinate;
use serde_json::{json, Value};

pub(super) const SCHEMA: &str = "semaprax.package-semantic-graph.v4";
const MAX_FACTS: usize = 256; // Four selected packages, 64 Agents per source.

pub(super) fn retain(
    raw: &str,
    coordinate: &Coordinate,
    rows: &mut Vec<Value>,
    budget: &mut ConstructionBudget,
) -> Result<bool, Vec<Diagnostic>> {
    if !raw.starts_with("{\"agent\":") {
        return Ok(false);
    }
    if rows.len() == MAX_FACTS {
        return Err(limit("package graph Agent inventory exceeds its bound"));
    }
    budget.charge(
        raw.len()
            .saturating_add(coordinate.package.len())
            .saturating_add(coordinate.version.len()),
        2048,
    )?;
    let mut value: Value =
        serde_json::from_str(raw).map_err(|_| binding("package graph Agent fact is invalid"))?;
    let object = value
        .as_object_mut()
        .ok_or_else(|| binding("package graph Agent fact is not an object"))?;
    if !matches!(object.len(), 2 | 3)
        || !object.contains_key("agent")
        || !object.contains_key("operations")
        || (object.len() == 3 && !object.contains_key("model_wait"))
    {
        return Err(binding("package graph Agent fact has unknown fields"));
    }
    let roles = ["initialize", "observe", "authorize", "reduce"];
    let operations = object
        .get("operations")
        .and_then(Value::as_array)
        .filter(|v| v.len() == roles.len())
        .ok_or_else(|| binding("package graph Agent operations are invalid"))?;
    for (operation, role) in operations.iter().zip(roles) {
        let op = operation
            .as_object()
            .filter(|o| o.len() == 4)
            .ok_or_else(|| binding("package graph Agent operation is invalid"))?;
        if op.get("role").and_then(Value::as_str) != Some(role)
            || op.get("operation_id").and_then(Value::as_str).is_none()
            || op.get("operation_id") != op.get("function_id")
            || !matches!(
                op.get("origin").and_then(Value::as_str),
                Some("embedded" | "reference")
            )
        {
            return Err(binding(
                "package graph Agent operation association is invalid",
            ));
        }
    }
    if object.get("agent").and_then(Value::as_str).is_none() {
        return Err(binding("package graph Agent identity is invalid"));
    }
    if let Some(wait) = object.get("model_wait") {
        let wait = wait
            .as_object()
            .filter(|o| o.len() == 2)
            .ok_or_else(|| binding("package graph Agent wait is invalid"))?;
        if wait
            .get("model_operation_id")
            .and_then(Value::as_str)
            .is_none()
            || wait.get("helper_id").and_then(Value::as_str).is_none()
        {
            return Err(binding("package graph Agent wait identity is invalid"));
        }
    }
    object.insert("package".to_owned(), json!(coordinate.package));
    object.insert("version".to_owned(), json!(coordinate.version));
    rows.push(value);
    Ok(true)
}

pub(super) fn attach(
    facts: &mut Value,
    base: &str,
    rows: Vec<Value>,
) -> Result<(), Vec<Diagnostic>> {
    if rows.is_empty() {
        return Ok(());
    }
    let object = facts
        .as_object_mut()
        .ok_or_else(|| binding("package graph retained facts are invalid"))?;
    object.insert("schema".to_owned(), json!(SCHEMA));
    object.insert(
        "agent_execution".to_owned(),
        json!({"base_schema":base,"authority":"none","agents":rows}),
    );
    Ok(())
}
