//! Closed V18 fragment-reference slots; all V17 row and older payload rules survive.
use super::*;

pub(in super::super::super) const SCHEMA_V18: &str = "semaprax.finite-source-overlay-page.v18";
fn index(value: &Value, total: usize) -> Result<u64, &'static str> {
    let value = value.as_u64().ok_or("v18 canonical addition index")?;
    if value < 519 || value >= total as u64 {
        return Err("v18 canonical addition index");
    }
    Ok(value)
}
fn walk(
    page: &Value,
    inspect: &mut dyn FnMut(&Value, bool) -> Result<(), &'static str>,
) -> Result<(), &'static str> {
    fn fragments(
        value: &Value,
        inspect: &mut dyn FnMut(&Value, bool) -> Result<(), &'static str>,
    ) -> Result<(), &'static str> {
        if value.is_null() {
            return Ok(());
        }
        for value in value.as_array().ok_or("v18 fragments")? {
            if value.is_number() || value.is_boolean() || value.get("canonical_source").is_some() {
                inspect(value, true)?;
            } else {
                if !(value.is_array() || value.is_object()) {
                    return Err("v18 fragment row");
                }
                inspect(value, false)?;
            }
        }
        Ok(())
    }
    for group in ["replacements", "additions"] {
        for value in page[group].as_array().ok_or("v18 rows")? {
            if let Some(reference) = value.get("old519_index") {
                if group != "replacements" || value.as_object().is_none_or(|o| o.len() != 1) {
                    return Err("v18 indexed row");
                }
                let reference = reference.as_array().ok_or("v18 old519 arity")?;
                if reference.len() != 4 {
                    return Err("v18 old519 arity");
                }
                fragments(&reference[3], inspect)?;
            } else {
                if !(value.is_array() || value.is_object()) {
                    return Err("v18 row type");
                }
                inspect(value, false)?;
                if value.is_array() {
                    let items = positional_row(value)?;
                    if items.len() == 5 {
                        fragments(&items[4], inspect)?;
                    }
                } else if let Some(value) = value.get("fragments") {
                    fragments(value, inspect)?;
                }
            }
        }
    }
    Ok(())
}
fn refs_mut(page: &mut Value, decode: bool, total: usize) -> Result<(), &'static str> {
    fn fragments(value: &mut Value, decode: bool, total: usize) -> Result<(), &'static str> {
        if value.is_null() {
            return Ok(());
        }
        for value in value.as_array_mut().ok_or("v18 fragments")? {
            if decode && value.is_number() {
                *value = serde_json::json!({"canonical_source":index(value,total)?});
            } else if !decode
                && value
                    .as_object()
                    .is_some_and(|o| o.len() == 1 && o.contains_key("canonical_source"))
                && !value["canonical_source"].is_string()
            {
                *value = index(&value["canonical_source"], total)?.into();
            }
        }
        Ok(())
    }
    for group in ["replacements", "additions"] {
        for value in page[group].as_array_mut().ok_or("v18 rows")? {
            if value.get("old519_index").is_some() {
                fragments(&mut value["old519_index"][3], decode, total)?;
            } else if let Some(items) = value.as_array_mut() {
                if items.len() == 5 {
                    fragments(&mut items[4], decode, total)?;
                }
            } else if let Some(value) = value.get_mut("fragments") {
                fragments(value, decode, total)?;
            }
        }
    }
    Ok(())
}
pub(super) fn preflight(page: &Value, limit: usize) -> Result<usize, &'static str> {
    source_envelope(page, SCHEMA_V18)?;
    let mut expanded = canonical(page).len();
    if expanded > PAGE_CAP {
        return Err("v18 same one-page cap");
    }
    let total = 519usize
        .checked_add(page["additions"].as_array().ok_or("v18 rows")?.len())
        .ok_or("v18 expanded bound")?;
    walk(page, &mut |value, reference| {
        let overhead = if reference {
            if let Some(object) = value.as_object() {
                if object.len() == 1 && !value["canonical_source"].is_string() {
                    index(&value["canonical_source"], total)?;
                    return Err("v18 wrapped integer alias");
                }
                0
            } else {
                index(value, total)?;
                21
            }
        } else if value.is_array() {
            let items = positional_row(value)?;
            row_overhead(items.len())
                .checked_add(b"{\"source_row\":[]}".len() - 2)
                .and_then(|n| n.checked_add(if items[1].as_u64() == Some(0) { 4 } else { 3 }))
                .ok_or("v18 expanded bound")?
        } else {
            if eligible_row(value) || is_row_envelope(value) {
                return Err("v18 literal row alias");
            }
            0
        };
        expanded = expanded.checked_add(overhead).ok_or("v18 expanded bound")?;
        if expanded > limit.min(SOURCE_CAP) {
            return Err("v18 expanded bound");
        }
        Ok(())
    })?;
    if expanded > limit.min(SOURCE_CAP) {
        return Err("v18 expanded bound");
    }
    Ok(expanded)
}
pub(super) fn expand_to_v15(page: &Value) -> Result<Value, &'static str> {
    let expanded = preflight(page, SOURCE_CAP)?;
    let total = 519usize
        .checked_add(page["additions"].as_array().ok_or("v18 rows")?.len())
        .ok_or("v18 expanded bound")?;
    // No expanded row/reference payload exists before the complete borrowed proof.
    let mut result = page.clone();
    refs_mut(&mut result, true, total)?;
    transform_positional_rows(&mut result, true)?;
    result["schema"] = SCHEMA_V15.into();
    if canonical(&result).len() != expanded {
        return Err("v18 expanded bound");
    }
    expand_to_v14_logical(&result)?;
    Ok(result)
}
pub(super) fn encode_rows(page: &Value) -> Result<Value, &'static str> {
    source_envelope(page, SCHEMA_V15)?;
    if canonical(page).len() > SOURCE_CAP {
        return Err("v18 logical input bound");
    }
    expand_to_v14_logical(page)?;
    visit_source_rows(page, &mut |value| {
        if eligible_row(value) {
            if !value["source"].is_string() || !value["compiled"].is_boolean() {
                return Err("v18 source/compiled type");
            }
            if !(value["current"].is_array() || value["current"].is_object())
                || !value["views"].is_array()
            {
                return Err("v18 current/views type");
            }
        } else if is_row_envelope(value) {
            return Err("v18 literal row alias");
        }
        Ok(())
    })?;
    let mut result = page.clone();
    transform_positional_rows(&mut result, false)?;
    let total = 519usize
        .checked_add(result["additions"].as_array().ok_or("v18 rows")?.len())
        .ok_or("v18 expanded bound")?;
    walk(&result, &mut |value, reference| {
        if reference {
            let object = value.as_object().ok_or("v18 literal integer alias")?;
            if object.len() == 1 && !value["canonical_source"].is_string() {
                index(&value["canonical_source"], total)?;
            }
        }
        Ok(())
    })?;
    refs_mut(&mut result, false, total)?;
    result["schema"] = SCHEMA_V18.into();
    if canonical(&result).len() > PAGE_CAP {
        return Err("v18 same one-page cap");
    }
    if canonical(&expand_to_v15(&result)?) != canonical(page) {
        return Err("v18 ENTIRE V15 page inverse");
    }
    Ok(result)
}
#[cfg_attr(not(test), allow(dead_code))]
pub(super) fn encode(page: &Value) -> Result<Value, &'static str> {
    encode_rows(&encode_v15_sources_logical(&encode_v14_tails_logical(
        &encode_v13_logical(page)?,
    )?)?)
}
pub(in super::super::super) fn expand_v18(
    before: &Value,
    policy: &Value,
    page: &Value,
) -> Result<Value, &'static str> {
    let logical = expand_to_v12_logical(&expand_to_v13_logical(&expand_to_v14_logical(
        &expand_to_v15(page)?,
    )?)?)?;
    super::super::expand_v11(before, policy, &logical_v12::expand_to_v11(&logical)?)
}
