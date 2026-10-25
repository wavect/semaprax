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

// V11_VIEW_NAMES_BEGIN: all preceding V10 code remains byte-exact.
pub(in super::super) const SCHEMA_V11: &str = "semaprax.finite-source-overlay-page.v11";
fn expand_v11_names(envelope: &Value) -> Result<Value, &'static str> {
    use std::cell::{Cell, RefCell};
    if !super::super::keys(envelope, &["schema", "names", "page"])
        || envelope["schema"] != SCHEMA_V11
    {
        return Err("v11 envelope");
    }
    if super::super::canonical(envelope).len() > super::super::PAGE_CAP {
        return Err("v11 wire cap");
    }
    let names = envelope["names"].as_array().ok_or("v11 names")?;
    let mut sizes = Vec::new();
    let mut previous = None;
    for name in names {
        let text = name.as_str().ok_or("v11 names")?;
        if previous.is_some_and(|old| old >= text) {
            return Err("v11 unique sorted names");
        }
        previous = Some(text);
        sizes.push(super::super::canonical(name).len());
    }
    let page = &envelope["page"];
    if !page.is_object() || page["schema"] != SCHEMA_V10 {
        return Err("v11 inner schema");
    }
    let used = RefCell::new(vec![false; names.len()]);
    let materialized = Cell::new(0usize);
    let result = v10_slots(page, &|value, named| {
        if !named || !value.is_array() {
            // Literal view objects, current pins, references and opaque data
            // retain their exact old representation, including non-string names.
            return Ok(value.clone());
        }
        let tuple = value.as_array().unwrap();
        let name = tuple.first().ok_or("v11 named tuple")?;
        let (text, size) = if let Some(text) = name.as_str() {
            (text, super::super::canonical(name).len())
        } else {
            let index = usize::try_from(name.as_u64().ok_or("v11 name index")?)
                .map_err(|_| "v11 name index")?;
            let name = names.get(index).ok_or("v11 name index")?;
            used.borrow_mut()[index] = true;
            (name.as_str().unwrap(), sizes[index])
        };
        let bytes = materialized
            .get()
            .checked_add(size)
            .filter(|bytes| *bytes <= super::super::PAGE_CAP)
            .ok_or("v11 materialized name cap")?;
        materialized.set(bytes);
        // Check the complete canonical-name debit before copying table text.
        let mut restored = tuple.clone();
        restored[0] = text.into();
        Ok(restored.into())
    })?;
    if used.borrow().iter().any(|used| !used) {
        return Err("v11 unused names");
    }
    Ok(result)
}
pub(in super::super) fn expand_v11(
    before: &Value,
    policy: &Value,
    envelope: &Value,
) -> Result<Value, &'static str> {
    expand_v10(before, policy, &expand_v11_names(envelope)?)
}
#[cfg(test)]
fn encode_v11_names(page: &Value) -> Result<Value, &'static str> {
    use std::cell::RefCell;
    use std::collections::BTreeMap;
    if !page.is_object() || page["schema"] != SCHEMA_V10 {
        return Err("v11 encoding schema");
    }
    let counts = RefCell::new(BTreeMap::<String, usize>::new());
    v10_slots(page, &|value, named| {
        if named {
            if let Some(tuple) = value.as_array() {
                let name = tuple
                    .first()
                    .and_then(Value::as_str)
                    .ok_or("v11 encoding name")?;
                *counts.borrow_mut().entry(name.into()).or_default() += 1;
            }
        }
        Ok(value.clone())
    })?;
    let names: Vec<_> = counts
        .into_inner()
        .into_iter()
        .filter_map(|(name, count)| (count > 1).then_some(name))
        .collect();
    let indices: BTreeMap<_, _> = names
        .iter()
        .enumerate()
        .map(|(index, name)| (name.as_str(), index))
        .collect();
    let encoded = v10_slots(page, &|value, named| {
        if named {
            if let Some(tuple) = value.as_array() {
                let name = tuple
                    .first()
                    .and_then(Value::as_str)
                    .ok_or("v11 encoding name")?;
                if let Some(index) = indices.get(name) {
                    let mut result = tuple.clone();
                    result[0] = (*index).into();
                    return Ok(result.into());
                }
            }
        }
        Ok(value.clone())
    })?;
    drop(indices);
    let result = serde_json::json!({"schema":SCHEMA_V11,"names":names,"page":encoded});
    if expand_v11_names(&result)? != *page {
        return Err("v11 ENTIRE V10 page inverse");
    }
    Ok(result)
}

#[cfg(test)]
mod v11_tests {
    use super::super::super::{canonical, compose_values, digest, held_4a6eb, PAGE_CAP};
    use super::*;
    fn frozen() -> (Value, Value, Value) {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
        let read = |sha, length| {
            serde_json::from_str::<Value>(
                &held_4a6eb(&root.join(format!("e/{sha}")), length, sha).unwrap(),
            )
            .unwrap()
        };
        (
            super::super::super::super::packet_before_overlay(),
            read(
                "f79b365ce48f1bfe3422feb73453795fe30d563c2d2167486ba85965d7b5d2dc",
                112303,
            ),
            read(
                "2018302f513448433f418a128169982a6bbe55652a843397e37fc5ed9ab9ac64",
                261855,
            ),
        )
    }
    fn page(name: &str, count: usize) -> Value {
        serde_json::json!({"schema":SCHEMA_V10,"replacements":[],"additions":[{
            "current":[0],"views":vec![serde_json::json!([name,0]);count]
        }]})
    }
    #[test]
    fn v11_frozen_entire_v10_page_and_complete_packet_restore_without_changed_limits() {
        let (before, policy, page) = frozen();
        let compact = encode_v11_names(&page).unwrap();
        assert_eq!(canonical(&compact).len(), 239085);
        assert_eq!(compact["names"].as_array().unwrap().len(), 134);
        assert!(canonical(&compact).len() < canonical(&page).len());
        assert_eq!(
            canonical(&expand_v11_names(&compact).unwrap()),
            canonical(&page)
        );
        let old = compose_values(before.clone(), &policy, &page).unwrap();
        let current = compose_values(before, &policy, &compact).unwrap();
        assert_eq!(canonical(&current), canonical(&old));
        assert_eq!(digest(&canonical(&current)), digest(&canonical(&old)));
        assert_eq!(PAGE_CAP, 262144);
        assert_eq!(super::super::super::SOURCE_CAP, 9753094);
    }
    #[test]
    fn v11_only_packed_name_slots_change_and_all_opaque_objects_remain_literal() {
        let mut original = page("λ\0🙂", 2);
        let opaque = serde_json::json!({"name":0,"views":[["λ\0🙂",0]],"current":[0]});
        original["unknown"] = opaque.clone();
        original["names"] = opaque.clone();
        original["additions"][0]["views"]
            .as_array_mut()
            .unwrap()
            .extend([
                serde_json::json!({"name":0,"unknown":opaque}),
                serde_json::json!({"name":["λ\0🙂",0],"unknown":opaque}),
                serde_json::json!({"name":"λ\0🙂","unknown":opaque}),
            ]);
        original["additions"][0]["fragments"] = serde_json::json!([
            {"current":[0],"views":[["λ\0🙂",0]],"unknown":opaque},
            {"canonical_source":"held","views":[["λ\0🙂",0]],"unknown":opaque}
        ]);
        original["replacements"] = serde_json::json!([{"old519_index":[0,[0],[["λ\0🙂",0]],null]}]);
        let compact = encode_v11_names(&original).unwrap();
        assert_eq!(compact["names"], serde_json::json!(["λ\0🙂"]));
        assert_eq!(compact["page"]["unknown"], opaque);
        assert_eq!(compact["page"]["names"], opaque);
        assert_eq!(
            compact["page"]["additions"][0]["views"][2],
            original["additions"][0]["views"][2]
        );
        assert_eq!(
            compact["page"]["additions"][0]["fragments"][1],
            original["additions"][0]["fragments"][1]
        );
        assert_eq!(expand_v11_names(&compact).unwrap(), original);
    }
    #[test]
    fn v11_bad_indices_tables_envelopes_and_positional_shapes_fail_closed() {
        let valid = encode_v11_names(&page("held", 2)).unwrap();
        for bad in [
            serde_json::json!(true),
            serde_json::json!(-1),
            serde_json::json!(0.0),
            serde_json::json!(1),
            serde_json::json!(u64::MAX),
            Value::Null,
            serde_json::json!({}),
        ] {
            let mut changed = valid.clone();
            changed["page"]["additions"][0]["views"][0][0] = bad;
            assert!(expand_v11_names(&changed).is_err());
        }
        for names in [
            Value::Null,
            serde_json::json!([0]),
            serde_json::json!(["held", "held"]),
            serde_json::json!(["z", "held"]),
            serde_json::json!(["held", "unused"]),
        ] {
            let mut changed = valid.clone();
            changed["names"] = names;
            assert!(expand_v11_names(&changed).is_err());
        }
        let mut changed = valid.clone();
        changed["extra"] = true.into();
        assert!(expand_v11_names(&changed).is_err());
        let mut changed = valid.clone();
        changed["schema"] = SCHEMA_V10.into();
        assert!(expand_v11_names(&changed).is_err());
        let mut changed = valid.clone();
        changed["page"]["schema"] = SCHEMA_V9.into();
        assert!(expand_v11_names(&changed).is_err());
        let mut changed = valid.clone();
        changed["page"]["additions"][0]["views"][0] = serde_json::json!([]);
        assert!(expand_v11_names(&changed).is_err());
        let mut changed = valid.clone();
        changed["page"]["replacements"] = serde_json::json!([{"old519_index":[0,[],[]]}]);
        assert!(expand_v11_names(&changed).is_err());
        let mut changed = valid.clone();
        changed["page"]["additions"] = serde_json::json!([{"old519_index":[0,[],[],null]}]);
        assert!(expand_v11_names(&changed).is_err());
        let literal = serde_json::json!({"schema":SCHEMA_V11,"names":[],"page":page("held",1)});
        assert_eq!(expand_v11_names(&literal).unwrap(), page("held", 1));
    }
    #[test]
    fn v11_wire_exact_plus_one_and_materialized_names_exact_plus_one_are_bounded() {
        let mut exact = serde_json::json!({"schema":SCHEMA_V11,"names":[],"page":{"schema":SCHEMA_V10,"replacements":[],"additions":[],"opaque":""}});
        let padding = PAGE_CAP - canonical(&exact).len();
        exact["page"]["opaque"] = "x".repeat(padding).into();
        assert_eq!(canonical(&exact).len(), PAGE_CAP);
        assert!(expand_v11_names(&exact).is_ok());
        exact["page"]["opaque"] = "x".repeat(padding + 1).into();
        assert_eq!(expand_v11_names(&exact), Err("v11 wire cap"));
        let name = "x".repeat(PAGE_CAP / 2 - 3);
        let compact = encode_v11_names(&page(&name, 2)).unwrap();
        assert!(canonical(&compact).len() < PAGE_CAP);
        assert_eq!(expand_v11_names(&compact).unwrap(), page(&name, 2));
        let mut over = compact;
        over["names"][0] = "x".repeat(PAGE_CAP / 2 - 2).into();
        assert_eq!(expand_v11_names(&over), Err("v11 materialized name cap"));
        let escaped = "\0".repeat((PAGE_CAP / 2 - 3) / 6);
        let mut compact = encode_v11_names(&page(&escaped, 2)).unwrap();
        compact["names"][0] = format!("{escaped}\0").into();
        assert_eq!(expand_v11_names(&compact), Err("v11 materialized name cap"));
    }
    #[test]
    fn v11_keeps_original_predecessor_and_pin_refusals() {
        let (before, policy, page) = frozen();
        let compact = encode_v11_names(&page).unwrap();
        let mut bad = before.clone();
        bad["rows"][0]["compiled"] = Value::Null;
        assert!(expand_v11(&bad, &policy, &compact).is_err());
        let mut bad = policy.clone();
        bad["predecessor"]["logical_sha256"] = "0".repeat(64).into();
        assert!(expand_v11(&before, &bad, &compact).is_err());
        let mut bad = compact;
        bad["page"]["additions"][0]["current"] = serde_json::json!([true]);
        assert!(expand_v11(&before, &policy, &bad).is_err());
    }
    #[test]
    fn v11_appends_to_the_entire_original_v10_rust_decoder_without_rewriting_it() {
        let source = std::include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/src/source_reflows_merge/main_e177_overlay_pins_v10.rs"
        ));
        let (original, _) = source.split_once("\n// V11_VIEW_NAMES_BEGIN:").unwrap();
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
        let frozen = held_4a6eb(
            &root.join("e/139e342c746175490471415451808f4882720303027f6373dd48562e6a814440"),
            11088,
            "139e342c746175490471415451808f4882720303027f6373dd48562e6a814440",
        )
        .unwrap();
        assert_eq!(original.as_bytes(), frozen.as_bytes());
    }
    #[test]
    fn v11_shared_python_fixture_has_identical_encoding_and_refusals() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
        let data = held_4a6eb(
            &root.join("e/55b7035de0ddbcfca1e466205cca1ecab30e553bc7636f25604885c8e459564f"),
            3276,
            "55b7035de0ddbcfca1e466205cca1ecab30e553bc7636f25604885c8e459564f",
        )
        .unwrap();
        let fixture: Value = serde_json::from_str(&data).unwrap();
        assert_eq!(fixture["schema"], "semaprax.source-view-name-fixtures.v1");
        for case in fixture["cases"].as_array().unwrap() {
            let actual = encode_v11_names(&case["input"]).unwrap();
            assert_eq!(canonical(&actual), canonical(&case["encoded"]));
            assert_eq!(expand_v11_names(&actual).unwrap(), case["input"]);
        }
        for case in fixture["refusals"].as_array().unwrap() {
            assert!(expand_v11_names(case).is_err());
        }
    }
}
