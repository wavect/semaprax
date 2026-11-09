//! V12 interns only named positional views, then restores the entire V11 value.
use super::super::{expand_v11, SCHEMA_V11};
use super::{canonical, SOURCE_CAP};
use serde_json::Value;
use std::collections::{BTreeMap, BTreeSet};

pub(in super::super) const SCHEMA_V12: &str = "semaprax.finite-source-overlay-page.v12";
const PAGE_CAP: usize = 262144;
const KEYS: [&str; 7] = [
    "schema",
    "index",
    "total_pages",
    "predecessor",
    "replacements",
    "additions",
    "archive_prefixes",
];

fn envelope(page: &Value, schema: &str, dictionary: bool) -> Result<(), &'static str> {
    let object = page.as_object().ok_or("v12 envelope")?;
    if page["schema"] != schema
        || object.len() != KEYS.len() + usize::from(dictionary)
        || !KEYS.iter().all(|key| object.contains_key(*key))
        || (dictionary && !object.contains_key("view_labels"))
    {
        return Err("v12 envelope");
    }
    Ok(())
}

type Convert<'a> = dyn FnMut(&mut Value) -> Result<(), &'static str> + 'a;
fn views(value: &mut Value, convert: &mut Convert<'_>) -> Result<(), &'static str> {
    for view in value.as_array_mut().ok_or("v12 views")? {
        // Full object pins, including every unknown field, stay opaque.
        if view.is_object() {
            continue;
        }
        let tuple = view.as_array_mut().ok_or("v12 named pin arity")?;
        if tuple.len() != 3 && tuple.len() != 4 {
            return Err("v12 named pin arity");
        }
        convert(&mut tuple[0])?;
    }
    Ok(())
}
fn row(value: &mut Value, convert: &mut Convert<'_>) -> Result<(), &'static str> {
    let object = value.as_object_mut().ok_or("v12 literal row")?;
    if let Some(value) = object.get_mut("views") {
        views(value, convert)?;
    }
    Ok(())
}
fn fragments(value: &mut Value, convert: &mut Convert<'_>) -> Result<(), &'static str> {
    if value.is_null() {
        return Ok(());
    }
    for fragment in value.as_array_mut().ok_or("v12 fragments")? {
        if !fragment.is_object() {
            return Err("v12 fragment row");
        }
        if fragment.get(super::super::REFERENCE).is_none() {
            row(fragment, convert)?;
        }
    }
    Ok(())
}
fn view_slots(page: &mut Value, convert: &mut Convert<'_>) -> Result<(), &'static str> {
    for group in ["replacements", "additions"] {
        for value in page[group].as_array_mut().ok_or("v12 rows")? {
            let object = value.as_object_mut().ok_or("v12 row")?;
            if object.contains_key("old519_index") {
                if group != "replacements" || object.len() != 1 {
                    return Err("v12 indexed row");
                }
                let tuple = object
                    .get_mut("old519_index")
                    .unwrap()
                    .as_array_mut()
                    .ok_or("v12 old519 arity")?;
                if tuple.len() != 4 {
                    return Err("v12 old519 arity");
                }
                views(&mut tuple[2], convert)?;
                fragments(&mut tuple[3], convert)?;
            } else {
                row(value, convert)?;
                if let Some(value) = value.get_mut("fragments") {
                    fragments(value, convert)?;
                }
            }
        }
    }
    Ok(())
}

fn index(value: &Value, count: usize) -> Result<usize, &'static str> {
    let index = usize::try_from(value.as_u64().ok_or("v12 unknown label index")?)
        .map_err(|_| "v12 unknown label index")?;
    if index >= count {
        return Err("v12 unknown label index");
    }
    Ok(index)
}

pub(super) fn expand_to_v11(page: &Value) -> Result<Value, &'static str> {
    envelope(page, SCHEMA_V12, true)?;
    if canonical(page).len() > SOURCE_CAP {
        return Err("v12 same one-page cap");
    }
    let declarations = page["view_labels"].as_array().ok_or("v12 dictionary")?;
    let mut labels = Vec::new();
    let mut sizes = Vec::new();
    let mut unique = BTreeSet::new();
    for (position, declaration) in declarations.iter().enumerate() {
        let tuple = declaration.as_array().ok_or("v12 declaration arity")?;
        if tuple.len() != 2 {
            return Err("v12 declaration arity");
        }
        if tuple[0].as_u64() != u64::try_from(position).ok() {
            return Err("v12 duplicate or noncontiguous index");
        }
        let label = tuple[1]
            .as_str()
            .ok_or("v12 duplicate or nonstring label")?;
        if !unique.insert(label) {
            return Err("v12 duplicate or nonstring label");
        }
        labels.push(label);
        sizes.push(
            serde_json::to_string(label)
                .map_err(|_| "v12 label JSON")?
                .len(),
        );
    }
    let mut result = page.clone();
    let mut seen = vec![false; labels.len()];
    let mut next = 0usize;
    let mut expanded = 0usize;
    // Bound all quoted label expansion before publishing any expanded string.
    view_slots(&mut result, &mut |value| {
        let index = index(value, labels.len())?;
        if !seen[index] {
            if index != next {
                return Err("v12 noncanonical first-use order");
            }
            seen[index] = true;
            next = next.checked_add(1).ok_or("v12 index overflow")?;
        }
        expanded = expanded
            .checked_add(sizes[index])
            .ok_or("v12 expanded label bound")?;
        if expanded > SOURCE_CAP {
            return Err("v12 expanded label bound");
        }
        Ok(())
    })?;
    if next != labels.len() {
        return Err("v12 unreferenced declaration");
    }
    view_slots(&mut result, &mut |value| {
        *value = labels[index(value, labels.len())?].into();
        Ok(())
    })?;
    result.as_object_mut().unwrap().remove("view_labels");
    result["schema"] = SCHEMA_V11.into();
    Ok(result)
}

// Explicit opt-in only. Existing V11 serialization and all old encoders retain
// their exact implementations and limits. The full V11 inverse is mandatory.
fn encode_v12(page: &Value) -> Result<Value, &'static str> {
    envelope(page, SCHEMA_V11, false)?;
    let mut result = page.clone();
    let mut indices = BTreeMap::new();
    let mut labels = Vec::new();
    view_slots(&mut result, &mut |value| {
        let label = value.as_str().ok_or("v12 V11 named label")?;
        let index = if let Some(index) = indices.get(label) {
            *index
        } else {
            let index = labels.len();
            indices.insert(label.to_owned(), index);
            labels.push(serde_json::json!([index, label]));
            index
        };
        *value = index.into();
        Ok(())
    })?;
    result["schema"] = SCHEMA_V12.into();
    result["view_labels"] = labels.into();
    if canonical(&result).len() > PAGE_CAP {
        return Err("v12 same one-page cap");
    }
    if expand_to_v11(&result)? != *page {
        return Err("v12 ENTIRE V11 page inverse");
    }
    Ok(result)
}

pub(in super::super) fn expand_v12(
    before: &Value,
    policy: &Value,
    page: &Value,
) -> Result<Value, &'static str> {
    expand_v11(before, policy, &expand_to_v11(page)?)
}
