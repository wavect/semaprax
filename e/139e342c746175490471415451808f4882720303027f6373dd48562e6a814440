//! Lossless references to exact current pins in the authenticated old519 table.
//! Only positional pin slots are decoded; all other metadata remains literal.
use super::{expand_v9, REFERENCE, SCHEMA_V9};
use serde_json::Value;
pub(in super::super) const SCHEMA_V10: &str = "semaprax.finite-source-overlay-page.v10";
type PinTransform<'a> = dyn Fn(&Value, bool) -> Result<Value, &'static str> + 'a;
fn old_rows<'a>(before: &'a Value, policy: &Value) -> Result<&'a [Value], &'static str> {
    let rows = before["rows"].as_array().ok_or("v10 old rows")?;
    if rows.len() != 519
        || policy["predecessor"]["rows"].as_u64() != Some(519)
        || before["source_cap"].as_u64() != Some(super::super::SOURCE_CAP as u64)
        || policy["predecessor"]["logical_sha256"].as_str()
            != Some(super::super::digest(&super::super::canonical(before)).as_str())
    {
        return Err("v10 authenticated old519");
    }
    Ok(rows)
}
fn packed_pin(current: &Value, name: Option<&Value>) -> Result<Value, &'static str> {
    if !super::super::keys(current, &["length", "sha256", "archive"]) {
        return Err("v10 exact current pin");
    }
    super::super::pin(current)?;
    let mut tuple = Vec::new();
    if let Some(name) = name {
        if !name.is_string() {
            return Err("v10 view name");
        }
        tuple.push(name.clone());
    }
    tuple.extend([current["length"].clone(), current["sha256"].clone()]);
    if current["archive"] != format!("e/{}", current["sha256"].as_str().unwrap()) {
        tuple.push(current["archive"].clone());
    }
    Ok(tuple.into())
}
fn expand_pin(value: &Value, named: bool, old: &[Value]) -> Result<Value, &'static str> {
    let Some(tuple) = value.as_array() else {
        return Ok(value.clone());
    };
    let offset = usize::from(named);
    if tuple.len() != offset + 1 {
        return Ok(value.clone());
    }
    let index = usize::try_from(tuple[offset].as_u64().ok_or("v10 pin index")?)
        .map_err(|_| "v10 pin index")?;
    let current = &old.get(index).ok_or("v10 pin index")?["current"];
    packed_pin(current, named.then_some(&tuple[0]))
}
fn v10_slots(page: &Value, pin: &PinTransform<'_>) -> Result<Value, &'static str> {
    fn views(value: &mut Value, pin: &PinTransform<'_>) -> Result<(), &'static str> {
        for view in value.as_array_mut().ok_or("v10 views")? {
            *view = pin(view, true)?;
        }
        Ok(())
    }
    fn row(value: &mut Value, pin: &PinTransform<'_>) -> Result<(), &'static str> {
        let object = value.as_object_mut().ok_or("v10 literal row")?;
        if let Some(current) = object.get_mut("current") {
            *current = pin(current, false)?;
        }
        if let Some(value) = object.get_mut("views") {
            views(value, pin)?;
        }
        Ok(())
    }
    fn fragments(value: &mut Value, pin: &PinTransform<'_>) -> Result<(), &'static str> {
        if value.is_null() {
            return Ok(());
        }
        for fragment in value.as_array_mut().ok_or("v10 fragments")? {
            if !fragment.is_object() {
                return Err("v10 fragment row");
            }
            if fragment.get(REFERENCE).is_none() {
                row(fragment, pin)?;
            }
        }
        Ok(())
    }
    let mut expanded = page.clone();
    for group in ["replacements", "additions"] {
        for value in expanded[group].as_array_mut().ok_or("v10 rows")? {
            if !value.is_object() {
                return Err("v10 row");
            }
            if value.get("old519_index").is_some() {
                if group != "replacements" || value.as_object().unwrap().len() != 1 {
                    return Err("v10 indexed row");
                }
                let tuple = value["old519_index"]
                    .as_array_mut()
                    .ok_or("v10 old519 arity")?;
                if tuple.len() != 4 {
                    return Err("v10 old519 arity");
                }
                tuple[1] = pin(&tuple[1], false)?;
                views(&mut tuple[2], pin)?;
                fragments(&mut tuple[3], pin)?;
            } else {
                row(value, pin)?;
                if let Some(value) = value.get_mut("fragments") {
                    fragments(value, pin)?;
                }
            }
        }
    }
    Ok(expanded)
}

fn expand_v10_pins(before: &Value, policy: &Value, page: &Value) -> Result<Value, &'static str> {
    if page["schema"] != SCHEMA_V10 {
        return Err("v10 schema");
    }
    let old = old_rows(before, policy)?;
    let mut expanded = v10_slots(page, &|value, named| expand_pin(value, named, old))?;
    expanded["schema"] = SCHEMA_V9.into();
    Ok(expanded)
}
pub(in super::super) fn expand_v10(
    before: &Value,
    policy: &Value,
    page: &Value,
) -> Result<Value, &'static str> {
    expand_v9(before, policy, &expand_v10_pins(before, policy, page)?)
}
#[cfg(test)]
mod tests {
    use super::super::super::{canonical, compose_values, digest, held_4a6eb};
    use super::*;
    fn encode(before: &Value, policy: &Value, page: &Value) -> Result<Value, &'static str> {
        if page["schema"] != SCHEMA_V9 {
            return Err("v10 encoding schema");
        }
        let old = old_rows(before, policy)?;
        let mut result = v10_slots(page, &|value, named| {
            let Some(tuple) = value.as_array() else {
                return Ok(value.clone());
            };
            let offset = usize::from(named);
            if tuple.len() != offset + 2 && tuple.len() != offset + 3 {
                return Ok(value.clone());
            }
            if named && !tuple[0].is_string() {
                return Ok(value.clone());
            }
            let Some(sha) = tuple[offset + 1].as_str() else {
                return Ok(value.clone());
            };
            let archive = if tuple.len() == offset + 3 {
                tuple[offset + 2].clone()
            } else {
                format!("e/{sha}").into()
            };
            let pin = serde_json::json!({"length":tuple[offset],"sha256":sha,"archive":archive});
            if packed_pin(&pin, named.then_some(&tuple[0]))? != *value {
                return Ok(value.clone());
            }
            let Some(index) = old.iter().position(|row| row["current"] == pin) else {
                return Ok(value.clone());
            };
            Ok(if named {
                serde_json::json!([tuple[0], index])
            } else {
                serde_json::json!([index])
            })
        })?;
        result["schema"] = SCHEMA_V10.into();
        Ok(result)
    }
    fn fixture() -> (Value, Value, Value) {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
        let read = |sha: &str, len| held_4a6eb(&root.join(format!("e/{sha}")), len, sha).unwrap();
        let page = serde_json::from_str(&read(
            "6a0abb50814885c388eedc3d4a5458354d7b3d1ba3d6087fa7d3590c7e1a1804",
            262067,
        ))
        .unwrap();
        let policy = serde_json::from_str(&read(
            "44c1f91356680c34ac2a3c5e97a47419bf7705ef956560a6ef05f63034738d4a",
            108303,
        ))
        .unwrap();
        (
            super::super::super::super::packet_before_overlay(),
            policy,
            page,
        )
    }
    #[test]
    fn v10_current_pin_references_restore_entire_frozen_v9_page_and_composed_packet() {
        let (before, policy, page) = fixture();
        let compact = encode(&before, &policy, &page).unwrap();
        assert_eq!(canonical(&compact).len(), 220641);
        assert!(canonical(&compact).len() <= super::super::super::PAGE_CAP);
        assert_eq!(
            canonical(&expand_v10_pins(&before, &policy, &compact).unwrap()),
            canonical(&page)
        );
        let old = compose_values(before.clone(), &policy, &page).unwrap();
        let actual = compose_values(before, &policy, &compact).unwrap();
        assert_eq!(
            actual, old,
            "ENTIRE prior packet, including unknown fields, all views, fragments and order"
        );
        assert_eq!(digest(&canonical(&actual)), digest(&canonical(&old)));
    }
    #[test]
    fn v10_references_preserve_unknowns_nonminimal_literal_pins_and_reference_objects() {
        let (before, policy, mut page) = fixture();
        let pin = &before["rows"][0]["current"];

        let opaque = serde_json::json!({"current":[0],"views":[["opaque",0]],"arbitrary":[0],"nested":{"current":[0]}});
        let mut full_pin = pin.clone();
        full_pin["unknown"] = opaque.clone();
        let h = "a".repeat(64);
        let nonminimal = serde_json::json!([3, h, format!("e/{h}")]);
        page["unknown"] = opaque.clone();
        page["additions"].as_array_mut().unwrap().push(serde_json::json!({"source":"src/literal.rs","current":full_pin,"views":[],"unknown":opaque,"fragments":[{"current":nonminimal,"unknown":opaque},{"canonical_source":"held","current":[0],"unknown":opaque}]}));
        let compact = encode(&before, &policy, &page).unwrap();
        assert_eq!(expand_v10_pins(&before, &policy, &compact).unwrap(), page);
    }
    #[test]
    fn v10_reference_indices_names_base_drift_and_legacy_tuple_errors_fail_closed() {
        let (before, policy, _) = fixture();
        let valid = serde_json::json!({"schema":SCHEMA_V10,"replacements":[],"additions":[{"current":[0],"views":[["held",0]]}]});
        assert!(expand_v10_pins(&before, &policy, &valid).is_ok());
        for index in [
            serde_json::json!(519),
            serde_json::json!(u64::MAX),
            serde_json::json!(-1),
            serde_json::json!(true),
            serde_json::json!(null),
            serde_json::json!("0"),
        ] {
            for named in [false, true] {
                let mut page = valid.clone();
                if named {
                    page["additions"][0]["views"][0][1] = index.clone();
                } else {
                    page["additions"][0]["current"][0] = index.clone();
                }
                assert!(
                    expand_v10_pins(&before, &policy, &page).is_err(),
                    "named={named} index={index}"
                );
            }
        }
        let mut bad = valid.clone();
        bad["additions"][0]["views"][0][0] = false.into();
        assert!(expand_v10_pins(&before, &policy, &bad).is_err());
        for mode in 0..4 {
            let mut b = before.clone();
            let mut p = policy.clone();
            match mode {
                0 => b["source_cap"] = 0.into(),
                1 => {
                    b["rows"].as_array_mut().unwrap().pop();
                }
                2 => p["predecessor"]["rows"] = true.into(),
                _ => b["rows"][0]["current"]["length"] = 0.into(),
            };
            assert!(expand_v10_pins(&b, &p, &valid).is_err(), "base mode={mode}");
        }
        let mut bad = valid;
        bad["schema"] = SCHEMA_V9.into();
        assert!(expand_v10_pins(&before, &policy, &bad).is_err());
    }
}
