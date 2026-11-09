use super::{expand_v8, REFERENCE, SCHEMA_V8};
use serde_json::Value;
// Positional transport is decoded only at the documented pin slots. Unknown
// metadata, absent fields and reference objects never enter this pin decoder.
pub(in super::super) const SCHEMA_V9: &str = "semaprax.finite-source-overlay-page.v9";
fn v9_pin(value: &Value, named: bool) -> Result<Value, &'static str> {
    let Some(tuple) = value.as_array() else {
        return Ok(value.clone());
    };
    let count = if named { 3 } else { 2 };
    if tuple.len() != count && tuple.len() != count + 1 {
        return Err("v9 pin arity");
    }
    let offset = usize::from(named);
    if named && !tuple[0].is_string() {
        return Err("v9 view name");
    }
    let sha = tuple[offset + 1].as_str().ok_or("v9 digest type")?;
    let archive = if tuple.len() == count + 1 {
        tuple[count].clone()
    } else {
        Value::String(format!("e/{sha}"))
    };
    let mut pin = serde_json::json!({"length":tuple[offset],"sha256":sha,"archive":archive});
    if named {
        pin["name"] = tuple[0].clone();
    }
    super::super::pin(&pin)?;
    Ok(pin)
}
fn v9_slots(
    page: &Value,
    pin: fn(&Value, bool) -> Result<Value, &'static str>,
) -> Result<Value, &'static str> {
    fn views(
        value: &mut Value,
        pin: fn(&Value, bool) -> Result<Value, &'static str>,
    ) -> Result<(), &'static str> {
        for view in value.as_array_mut().ok_or("v9 views")? {
            *view = pin(view, true)?;
        }
        Ok(())
    }
    fn row(
        value: &mut Value,
        pin: fn(&Value, bool) -> Result<Value, &'static str>,
    ) -> Result<(), &'static str> {
        let object = value.as_object_mut().ok_or("v9 literal row")?;
        if let Some(current) = object.get_mut("current") {
            *current = pin(current, false)?;
        }
        if let Some(value) = object.get_mut("views") {
            views(value, pin)?;
        }
        Ok(())
    }
    fn fragments(
        value: &mut Value,
        pin: fn(&Value, bool) -> Result<Value, &'static str>,
    ) -> Result<(), &'static str> {
        if value.is_null() {
            return Ok(());
        }
        for fragment in value.as_array_mut().ok_or("v9 fragments")? {
            if !fragment.is_object() {
                return Err("v9 fragment row");
            }
            if fragment.get(REFERENCE).is_none() {
                row(fragment, pin)?;
            }
        }
        Ok(())
    }
    let mut expanded = page.clone();
    for group in ["replacements", "additions"] {
        for value in expanded[group].as_array_mut().ok_or("v9 rows")? {
            if !value.is_object() {
                return Err("v9 row");
            }
            if value.get("old519_index").is_some() {
                if group != "replacements" || value.as_object().unwrap().len() != 1 {
                    return Err("v9 indexed row");
                }
                let tuple = value["old519_index"]
                    .as_array_mut()
                    .ok_or("v9 old519 arity")?;
                if tuple.len() != 4 {
                    return Err("v9 old519 arity");
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
fn expand_v9_pins(page: &Value) -> Result<Value, &'static str> {
    if page["schema"] != SCHEMA_V9 {
        return Err("v9 schema");
    }
    let mut expanded = v9_slots(page, v9_pin)?;
    expanded["schema"] = SCHEMA_V8.into();
    Ok(expanded)
}
pub(in super::super) fn expand_v9(
    before: &Value,
    policy: &Value,
    page: &Value,
) -> Result<Value, &'static str> {
    expand_v8(before, policy, &expand_v9_pins(page)?)
}

#[cfg(test)]
mod tests {
    use super::super::super::{canonical, compose_values, digest, held_4a6eb};
    use super::*;
    fn encode_v9_pin(value: &Value, named: bool) -> Result<Value, &'static str> {
        let fields: &[&str] = if named {
            &["name", "length", "sha256", "archive"]
        } else {
            &["length", "sha256", "archive"]
        };
        if !value
            .as_object()
            .is_some_and(|o| o.len() == fields.len() && fields.iter().all(|k| o.contains_key(*k)))
            || (named && !value["name"].is_string())
        {
            return Ok(value.clone());
        }
        super::super::super::pin(value)?;
        let mut tuple = Vec::new();
        if named {
            tuple.push(value["name"].clone());
        }
        tuple.extend([value["length"].clone(), value["sha256"].clone()]);
        if value["archive"] != format!("e/{}", value["sha256"].as_str().unwrap()) {
            tuple.push(value["archive"].clone());
        }
        Ok(tuple.into())
    }
    #[test]
    fn v9_positional_pins_restore_entire_frozen_v8_page_and_complete_composed_packet() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
        let read = |sha: &str, len| held_4a6eb(&root.join(format!("e/{sha}")), len, sha).unwrap();
        let raw = read(
            "3513e9377da7c932ca29e1ae0abc7f816c245a47f05f8a5d84b6b8887d5bbfd5",
            262082,
        );
        let full: Value = serde_json::from_str(&raw).unwrap();
        let policy: Value = serde_json::from_str(&read(
            "654893fb38f3ac2f78bc8ad07c497a9e2f4f23537b22b794e1e8189fd6c3e4f7",
            102315,
        ))
        .unwrap();
        let mut compact = v9_slots(&full, encode_v9_pin).unwrap();
        compact["schema"] = SCHEMA_V9.into();
        assert_eq!(canonical(&compact).len(), 171274);
        assert!(canonical(&compact).len() <= super::super::super::PAGE_CAP);
        assert_eq!(
            canonical(&expand_v9_pins(&compact).unwrap()),
            raw.as_bytes()
        );
        let before = super::super::super::super::packet_before_overlay();
        let old = compose_values(before.clone(), &policy, &full).unwrap();
        let actual = compose_values(before, &policy, &compact).unwrap();
        assert_eq!(
            actual, old,
            "ENTIRE packet including all predecessor data/views/order"
        );
        assert_eq!(
            digest(&canonical(&actual)),
            "c299b0cad6a55178132a94ccbaeecd725025acda330e9e448d9d268ea822d59b"
        );
    }
    #[test]
    fn v9_pin_slots_preserve_unknowns_nulls_literal_archives_and_refuse_wrong_tuples() {
        let h = "a".repeat(64);
        let pin = serde_json::json!({"length":3,"sha256":h,"archive":format!("e/{h}")});
        let mut view = pin.clone();
        view["name"] = "held".into();
        view["archive"] = "e/literal".into();
        let extra = serde_json::json!({"current":[3,h],"views":[["held",3,h]],"unknown":null});
        let mut full = serde_json::json!({"schema":SCHEMA_V8,"replacements":[{"old519_index":[38,pin,[view],null]}],
            "additions":[{"current":pin,"views":[view],"unknown":extra,"fragments":[{"current":pin,"views":[view],"fragments":extra},{"canonical_source":"kept","current":[3,h]}]}],"unknown":extra});
        full["additions"][0]["views"].as_array_mut().unwrap().push(serde_json::json!({"name":null,"length":3,"sha256":h,"archive":"e/literal","extra":null}));
        let mut compact = v9_slots(&full, encode_v9_pin).unwrap();
        compact["schema"] = SCHEMA_V9.into();
        assert_eq!(expand_v9_pins(&compact).unwrap(), full);
        assert_eq!(compact["unknown"], full["unknown"]);
        assert_eq!(compact["additions"][0]["unknown"], extra);
        assert_eq!(
            compact["additions"][0]["fragments"][1],
            full["additions"][0]["fragments"][1]
        );
        assert_eq!(
            compact["additions"][0]["views"][1],
            full["additions"][0]["views"][1]
        );
        for bad in [
            serde_json::json!([]),
            serde_json::json!([3]),
            serde_json::json!([3, h, "e/a", 0]),
            serde_json::json!([true, h]),
            serde_json::json!([-1, h]),
            serde_json::json!([1.5, h]),
            serde_json::json!([u64::MAX, h]),
            serde_json::json!([super::super::super::SOURCE_CAP + 1, h]),
            serde_json::json!([3, "A".repeat(64)]),
            serde_json::json!([3, "a"]),
            serde_json::json!([3, 3]),
            serde_json::json!([3, h, "../unsafe"]),
            serde_json::json!([3, h, false]),
        ] {
            let mut page = compact.clone();
            page["additions"][0]["current"] = bad;
            assert!(expand_v9_pins(&page).is_err());
        }
        for mode in 0..10 {
            let mut page = compact.clone();
            match mode {
                0 => page["additions"][0]["views"][0] = serde_json::json!([false, 3, h]),
                1 => page["additions"][0]["views"] = Value::Null,
                2 => page["replacements"][0]["old519_index"] = serde_json::json!([38, pin, []]),
                3 => page["replacements"][0]["extra"] = true.into(),
                4 => page["replacements"][0]["old519_index"][2] = false.into(),
                5 => page["additions"][0]["fragments"] = false.into(),
                6 => page["additions"][0]["fragments"][0] = true.into(),
                7 => {
                    let row = page["replacements"][0].clone();
                    page["additions"][0] = row;
                }
                8 => page["schema"] = "semaprax.finite-source-overlay-page.v900".into(),
                _ => page["replacements"] = true.into(),
            }
            assert!(expand_v9_pins(&page).is_err(), "v9 mode={mode}");
        }
        let mut absent = serde_json::json!({"schema":SCHEMA_V9,"replacements":[],"additions":[{"current":null,"unknown":null}]});
        let decoded = expand_v9_pins(&absent).unwrap();
        absent["schema"] = SCHEMA_V8.into();
        assert_eq!(decoded, absent);
    }
}
