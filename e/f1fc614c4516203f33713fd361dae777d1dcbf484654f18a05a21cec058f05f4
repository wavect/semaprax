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
// A distinct physical frontier. Public V18 functions above remain unchanged.
pub(in super::super::super) const SCHEMA_V19_INDEXED: &str =
    "semaprax.finite-source-overlay-page.v19-indexed";
fn indexed_replacement(value: &Value) -> Result<Option<&Vec<Value>>, &'static str> {
    if value.get("old519_index").is_some() {
        return Err("v19 indexed wrapped alias");
    }
    let Some(items) = value
        .as_array()
        .filter(|a| a.first().is_some_and(Value::is_number))
    else {
        return Ok(None);
    };
    if items.len() != 4 || items[0].as_u64().is_none_or(|n| n >= 519) {
        return Err("v19 indexed replacement");
    }
    Ok(Some(items))
}
fn walk_indexed(
    page: &Value,
    indexed: &mut dyn FnMut() -> Result<(), &'static str>,
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
            let reference = indexed_replacement(value)?;
            if let Some(reference) = reference {
                if group != "replacements" {
                    return Err("v19 indexed replacement slot");
                }
                indexed()?;
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
pub(in super::super::super) fn preflight_indexed(page: &Value, limit: usize) -> Result<usize, &'static str> {
    source_envelope(page, SCHEMA_V19_INDEXED)?;
    let physical = canonical(page).len();
    if physical > PAGE_CAP {
        return Err("v19 indexed same one-page cap");
    }
    let total = 519usize
        .checked_add(page["additions"].as_array().ok_or("v18 rows")?.len())
        .ok_or("v18 expanded bound")?;
    // Both counters are borrowed proofs. No restored wrapper or cloned page is
    // formed until the complete V15 size, including all row/ref deltas, fits.
    let wrappers = std::cell::Cell::new(0usize);
    let overhead = std::cell::Cell::new(0usize);
    walk_indexed(
        page,
        &mut || {
            wrappers.set(wrappers.get().checked_add(17).ok_or("v18 expanded bound")?);
            Ok(())
        },
        &mut |value, reference| {
            let delta = if reference {
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
            overhead.set(
                overhead
                    .get()
                    .checked_add(delta)
                    .ok_or("v18 expanded bound")?,
            );
            Ok(())
        },
    )?;
    let expanded = physical
        .checked_sub(SCHEMA_V19_INDEXED.len() - SCHEMA_V15.len())
        .and_then(|n| n.checked_add(wrappers.get()))
        .and_then(|n| n.checked_add(overhead.get()))
        .ok_or("v18 expanded bound")?;
    if expanded > limit.min(SOURCE_CAP) {
        return Err("v18 expanded bound");
    }
    Ok(expanded)
}
pub(super) fn expand_indexed_to_v15(page: &Value) -> Result<Value, &'static str> {
    let expanded = preflight_indexed(page, SOURCE_CAP)?;
    let total = 519usize
        .checked_add(page["additions"].as_array().ok_or("v18 rows")?.len())
        .ok_or("v18 expanded bound")?;
    let mut result = page.clone();
    for value in result["replacements"].as_array_mut().ok_or("v18 rows")? {
        if indexed_replacement(value)?.is_some() {
            *value = serde_json::json!({"old519_index": std::mem::take(value)});
        }
    }
    refs_mut(&mut result, true, total)?;
    transform_positional_rows(&mut result, true)?;
    result["schema"] = SCHEMA_V15.into();
    if canonical(&result).len() != expanded {
        return Err("v18 expanded bound");
    }
    expand_to_v14_logical(&result)?;
    Ok(result)
}
pub(super) fn encode_indexed_rows(page: &Value) -> Result<Value, &'static str> {
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
    for value in result["replacements"].as_array_mut().ok_or("v18 rows")? {
        if value.get("old519_index").is_some() {
            if value.as_object().is_none_or(|o| o.len() != 1) {
                return Err("v18 indexed row");
            }
            let reference = value["old519_index"].as_array().ok_or("v18 old519 arity")?;
            if reference.len() != 4 || reference[0].as_u64().is_none_or(|n| n >= 519) {
                return Err("v19 indexed replacement");
            }
            *value = value
                .as_object_mut()
                .unwrap()
                .remove("old519_index")
                .unwrap();
        }
    }
    result["schema"] = SCHEMA_V19_INDEXED.into();
    if canonical(&result).len() > PAGE_CAP {
        return Err("v19 indexed same one-page cap");
    }
    if canonical(&expand_indexed_to_v15(&result)?) != canonical(page) {
        return Err("v19 indexed ENTIRE V15 page inverse");
    }
    Ok(result)
}
#[cfg_attr(not(test), allow(dead_code))]
pub(super) fn encode_indexed(page: &Value) -> Result<Value, &'static str> {
    encode_indexed_rows(&encode_v15_sources_logical(&encode_v14_tails_logical(
        &encode_v13_logical(page)?,
    )?)?)
}
pub(in super::super::super) fn expand_v19_indexed(
    before: &Value,
    policy: &Value,
    page: &Value,
) -> Result<Value, &'static str> {
    let logical = expand_to_v12_logical(&expand_to_v13_logical(&expand_to_v14_logical(
        &expand_indexed_to_v15(page)?,
    )?)?)?;
    super::super::expand_v11(before, policy, &logical_v12::expand_to_v11(&logical)?)
}

#[cfg(test)]
pub(super) fn python_indexed(full: &Value) -> Value {
    use std::io::{Read, Write};
    use std::process::{Command, Stdio};
    let mut child = Command::new("python3").args(["-B", "-c", r#"
import json,runpy,sys
from pathlib import Path
n=runpy.run_path(str(Path(sys.argv[1])/'docs/evidence/finite-source-overlay-v2-20261018/overlay_page.py'),run_name='indexed_owning_parity')
value=json.load(sys.stdin);wire=n['encode_v19_indexed_inner'](value)
sys.stdout.buffer.write(n['canonical']({'encoded':wire,'v15':n['expand_v19_indexed_fragment_refs'](wire)}))
"#]).arg(env!("CARGO_MANIFEST_DIR")).stdin(Stdio::piped()).stdout(Stdio::piped()).spawn().unwrap();
    child
        .stdin
        .take()
        .unwrap()
        .write_all(&canonical(full))
        .unwrap();
    let mut raw = Vec::new();
    let read = child
        .stdout
        .take()
        .unwrap()
        .take((SOURCE_CAP + 1) as u64)
        .read_to_end(&mut raw);
    let status = child.wait().unwrap();
    read.unwrap();
    assert!(status.success());
    assert!(raw.len() <= SOURCE_CAP);
    serde_json::from_slice(&raw).unwrap()
}
#[cfg(test)]
mod indexed_tests {
    use super::*;
    fn current() -> Value {
        let page: Value = serde_json::from_slice(include_bytes!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/docs/evidence/finite-source-overlay-v2-20261018/page.json"
        )))
        .unwrap();
        assert_eq!(page["schema"], "semaprax.finite-source-overlay-page.v19");
        assert_eq!(page["page"]["schema"], SCHEMA_V19_INDEXED);
        page["page"].clone()
    }
    #[test]
    fn indexed_current_entire_v15_inverse_and_exact_expansion_frontier() {
        let wire = current();
        let size = preflight_indexed(&wire, SOURCE_CAP).unwrap();
        assert_eq!(preflight_indexed(&wire, size), Ok(size));
        assert_eq!(
            preflight_indexed(&wire, size - 1),
            Err("v18 expanded bound")
        );
        let logical = expand_indexed_to_v15(&wire).unwrap();
        assert_eq!(canonical(&logical).len(), size);
        assert_eq!(
            canonical(&encode_indexed_rows(&logical).unwrap()),
            canonical(&wire)
        );
        let v12 = expand_to_v12_logical(
            &expand_to_v13_logical(&expand_to_v14_logical(&logical).unwrap()).unwrap(),
        )
        .unwrap();
        assert_eq!(canonical(&encode_indexed(&v12).unwrap()), canonical(&wire));
        assert!(
            preflight(&wire, SOURCE_CAP).is_err(),
            "public V18 never accepts the new schema"
        );
    }
    #[test]
    fn indexed_slots_reject_aliases_wrong_types_arities_and_wrong_domains() {
        let wire = current();
        let at = wire["replacements"]
            .as_array()
            .unwrap()
            .iter()
            .position(|value| indexed_replacement(value).unwrap().is_some())
            .unwrap();
        for index in [
            serde_json::json!(true),
            serde_json::json!(false),
            serde_json::json!(-1),
            serde_json::json!(1.0),
            serde_json::json!(519),
            serde_json::json!(u64::MAX),
        ] {
            let mut bad = wire.clone();
            bad["replacements"][at][0] = index;
            assert!(expand_indexed_to_v15(&bad).is_err());
        }
        for arity in [0, 1, 2, 3, 5] {
            let mut bad = wire.clone();
            let row = bad["replacements"][at].as_array_mut().unwrap();
            if arity < row.len() {
                row.truncate(arity);
            } else {
                row.push(Value::Null);
            }
            assert!(expand_indexed_to_v15(&bad).is_err());
        }
        let mut alias = wire.clone();
        alias["replacements"][at] =
            serde_json::json!({"old519_index":wire["replacements"][at].clone()});
        assert_eq!(
            expand_indexed_to_v15(&alias),
            Err("v19 indexed wrapped alias")
        );
        let mut addition = wire.clone();
        addition["additions"][0] = wire["replacements"][at].clone();
        assert_eq!(
            expand_indexed_to_v15(&addition),
            Err("v19 indexed replacement slot")
        );
        let mut fragment = wire.clone();
        fragment["replacements"][at][3] = serde_json::json!([wire["replacements"][at].clone()]);
        assert!(expand_indexed_to_v15(&fragment).is_err());
    }
    #[test]
    fn indexed_physical_and_public_v18_caps_remain_independent() {
        let mut wire = current();
        wire["additions"]
            .as_array_mut()
            .unwrap()
            .push(serde_json::json!({"literal_unknown": "x".repeat(PAGE_CAP)}));
        assert_eq!(
            expand_indexed_to_v15(&wire),
            Err("v19 indexed same one-page cap")
        );
        wire["schema"] = SCHEMA_V18.into();
        assert_eq!(expand_to_v15(&wire), Err("v18 same one-page cap"));
    }
}

#[cfg(test)]
mod indexed_opaque_tests {
    use super::*;
    #[test]
    fn indexed_opaque_nested_tags_survive_without_recursive_rewriting() {
        let outer: Value = serde_json::from_slice(include_bytes!(concat!(env!("CARGO_MANIFEST_DIR"), "/docs/evidence/finite-source-overlay-v2-20261018/page.json"))).unwrap();
        let mut wire = outer["page"].clone();
        let at = wire["replacements"].as_array().unwrap().iter().position(|v| indexed_replacement(v).unwrap().is_some()).unwrap();
        wire["replacements"].as_array_mut().unwrap().truncate(at + 1);
        let opaque = serde_json::json!({"opaque": {"old519_index": [true, -1, "not an index"], "canonical_source": 519}, "unknown": ["é\u{0}", null, true]});
        let fragments = &mut wire["replacements"][at][3];
        if fragments.is_null() { *fragments = serde_json::json!([]); }
        fragments.as_array_mut().unwrap().push(opaque.clone());
        let logical = expand_indexed_to_v15(&wire).unwrap();
        assert_eq!(logical["replacements"][at]["old519_index"][3].as_array().unwrap().last(), Some(&opaque));
        assert_eq!(canonical(&encode_indexed_rows(&logical).unwrap()), canonical(&wire));
    }
}
