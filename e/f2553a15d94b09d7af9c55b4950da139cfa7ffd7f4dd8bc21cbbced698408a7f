use super::{expand_v9, REFERENCE, SCHEMA_V9};
use serde_json::Value;
use std::collections::BTreeSet;

pub(in super::super) const SCHEMA_V10: &str = "semaprax.finite-source-overlay-page.v10";
const PREFIXES: &str = "archive_prefixes";
const MAX_PREFIXES: usize = 32;

fn prefix_table(page: &Value) -> Result<Vec<String>, &'static str> {
    let values = page[PREFIXES].as_array().ok_or("v10 archive prefixes")?;
    if values.is_empty() || values.len() > MAX_PREFIXES {
        return Err("v10 archive prefix count");
    }
    let mut result = Vec::with_capacity(values.len());
    let mut unique = BTreeSet::new();
    for value in values {
        let prefix = value.as_str().ok_or("v10 archive prefix type")?;
        let Some(directory) = prefix.strip_suffix('/') else {
            return Err("v10 archive prefix directory");
        };
        if !super::super::normal(directory) || !unique.insert(prefix.to_owned()) {
            return Err("v10 archive prefix directory");
        }
        result.push(prefix.to_owned());
    }
    Ok(result)
}

fn pin_slot<F>(value: &mut Value, named: bool, archive: &mut F) -> Result<(), &'static str>
where
    F: FnMut(&mut Value, bool) -> Result<(), &'static str>,
{
    if value.is_array() {
        archive(value, named)?;
    }
    Ok(())
}

fn views<F>(value: &mut Value, archive: &mut F) -> Result<(), &'static str>
where
    F: FnMut(&mut Value, bool) -> Result<(), &'static str>,
{
    for view in value.as_array_mut().ok_or("v10 views")? {
        pin_slot(view, true, archive)?;
    }
    Ok(())
}

fn row<F>(value: &mut Value, archive: &mut F) -> Result<(), &'static str>
where
    F: FnMut(&mut Value, bool) -> Result<(), &'static str>,
{
    let object = value.as_object_mut().ok_or("v10 literal row")?;
    if let Some(current) = object.get_mut("current") {
        pin_slot(current, false, archive)?;
    }
    if let Some(value) = object.get_mut("views") {
        views(value, archive)?;
    }
    Ok(())
}

fn fragments<F>(value: &mut Value, archive: &mut F) -> Result<(), &'static str>
where
    F: FnMut(&mut Value, bool) -> Result<(), &'static str>,
{
    if value.is_null() {
        return Ok(());
    }
    for fragment in value.as_array_mut().ok_or("v10 fragments")? {
        if !fragment.is_object() {
            return Err("v10 fragment row");
        }
        if fragment.get(super::REFERENCE).is_none() {
            row(fragment, archive)?;
        }
    }
    Ok(())
}

fn archive_slots<F>(page: &Value, mut archive: F) -> Result<Value, &'static str>
where
    F: FnMut(&mut Value, bool) -> Result<(), &'static str>,
{
    let mut result = page.clone();
    for group in ["replacements", "additions"] {
        for value in result[group].as_array_mut().ok_or("v10 rows")? {
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
                pin_slot(&mut tuple[1], false, &mut archive)?;
                views(&mut tuple[2], &mut archive)?;
                fragments(&mut tuple[3], &mut archive)?;
            } else {
                row(value, &mut archive)?;
                if let Some(value) = value.get_mut("fragments") {
                    fragments(value, &mut archive)?;
                }
            }
        }
    }
    Ok(result)
}

fn expand_archive(
    value: &mut Value,
    named: bool,
    prefixes: &[String],
    uses: &mut [usize],
) -> Result<(), &'static str> {
    let tuple = value.as_array_mut().ok_or("v10 pin tuple")?;
    let count = if named { 3 } else { 2 };
    if tuple.len() != count + 1 {
        return Ok(());
    }
    if let Some(path) = tuple[count].as_str() {
        if prefixes.iter().any(|prefix| path.starts_with(prefix)) {
            return Err("v10 uninterned archive");
        }
        return Ok(());
    }
    let reference = tuple[count].as_array().ok_or("v10 archive reference")?;
    if reference.len() != 2 {
        return Err("v10 archive reference");
    }
    let index = usize::try_from(reference[0].as_u64().ok_or("v10 archive prefix index")?)
        .map_err(|_| "v10 archive prefix index")?;
    let prefix = prefixes.get(index).ok_or("v10 archive prefix index")?;
    let suffix = reference[1].as_str().ok_or("v10 archive suffix type")?;
    let path = format!("{prefix}{suffix}");
    if !super::super::normal(&path) {
        return Err("v10 expanded archive path");
    }
    let longest = prefixes
        .iter()
        .enumerate()
        .filter(|(_, candidate)| path.starts_with(candidate.as_str()))
        .max_by_key(|(_, candidate)| candidate.len())
        .map(|(position, _)| position)
        .ok_or("v10 archive prefix index")?;
    if longest != index {
        return Err("v10 noncanonical archive prefix");
    }
    uses[index] += 1;
    tuple[count] = Value::String(path);
    Ok(())
}

fn expand_v10_to_v9(page: &Value) -> Result<Value, &'static str> {
    if page["schema"] != SCHEMA_V10 {
        return Err("v10 schema");
    }
    let prefixes = prefix_table(page)?;
    let mut uses = vec![0usize; prefixes.len()];
    let mut expanded = archive_slots(page, |value, named| {
        expand_archive(value, named, &prefixes, &mut uses)
    })?;
    if uses.iter().any(|count| *count < 2) {
        return Err("v10 unused archive prefix");
    }
    expanded
        .as_object_mut()
        .ok_or("v10 page object")?
        .remove(PREFIXES)
        .ok_or("v10 archive prefixes")?;
    expanded["schema"] = SCHEMA_V9.into();
    Ok(expanded)
}

pub(in super::super) fn expand_v10(
    before: &Value,
    policy: &Value,
    page: &Value,
) -> Result<Value, &'static str> {
    // V10 has no composition semantics. It restores one exact V9 page and
    // enters the complete unchanged V9 -> V2 reader chain.
    expand_v9(before, policy, &expand_v10_to_v9(page)?)
}

#[cfg(test)]
mod tests {
    use super::super::super::canonical;
    use super::*;

    fn encode_archive(
        value: &mut Value,
        named: bool,
        prefixes: &[String],
    ) -> Result<(), &'static str> {
        let tuple = value.as_array_mut().ok_or("test pin")?;
        let count = if named { 3 } else { 2 };
        if tuple.len() != count + 1 {
            return Ok(());
        }
        let Some(path) = tuple[count].as_str().map(str::to_owned) else {
            return Ok(());
        };
        let Some((index, prefix)) = prefixes
            .iter()
            .enumerate()
            .filter(|(_, prefix)| path.starts_with(prefix.as_str()))
            .max_by_key(|(_, prefix)| prefix.len())
        else {
            return Ok(());
        };
        tuple[count] = serde_json::json!([index, &path[prefix.len()..]]);
        Ok(())
    }

    fn encode_v10(page: &Value, prefixes: &[&str]) -> Result<Value, &'static str> {
        if page["schema"] != SCHEMA_V9 {
            return Err("test v9 schema");
        }
        let values: Vec<String> = prefixes.iter().map(|value| (*value).to_owned()).collect();
        let mut encoded =
            archive_slots(page, |value, named| encode_archive(value, named, &values))?;
        encoded["schema"] = SCHEMA_V10.into();
        encoded[PREFIXES] = values.into();
        if canonical(&expand_v10_to_v9(&encoded)?) != canonical(page) {
            return Err("test entire v9 inverse");
        }
        Ok(encoded)
    }

    fn fixture() -> Value {
        let h = "a".repeat(64);
        let reference_unknown = serde_json::json!({
            "canonical_source": 521,
            "current": [7, h, [0, "must-stay-literal"]],
            "unknown": {"archive":[0,"also-literal"]}
        });
        serde_json::json!({
            "schema": SCHEMA_V9,
            "index": 1,
            "total_pages": 1,
            "predecessor": {"rows":519},
            "replacements": [{"old519_index":[38,
                [7,h,"docs/evidence/a/one.txt"],
                [["held",7,h,"docs/evidence/fmt/two.txt"]],null]}],
            "additions": [{
                "source":"src/new.rs","compiled":true,
                "current":[7,h,"docs/evidence/b/three.txt"],
                "views":[["held",7,h,"docs/evidence/fmt/four.txt"]],
                "fragments":[reference_unknown],
                "unknown":{"current":[7,h,[0,"literal"]]}
            }],
            "unknown":{"archive":[0,"literal"]}
        })
    }

    #[test]
    fn v10_frozen_full_union_restores_the_whole_canonical_v9_value() {
        const V9: &str = include_str!("../../docs/evidence/finite-source-overlay-v2-20261018/v10-unqualified-full-union-v9.json");
        const V10: &str = include_str!(
            "../../docs/evidence/finite-source-overlay-v2-20261018/v10-unqualified-full-union.json"
        );
        let v9: Value = serde_json::from_str(V9).unwrap();
        let v10: Value = serde_json::from_str(V10).unwrap();
        // This logical fixture freezes the 519 + 188 parent-metadata union.
        // The eventual source rebind separately admits this V10 Rust child.
        assert_eq!(519 + v9["additions"].as_array().unwrap().len(), 707);
        let restored = expand_v10_to_v9(&v10).unwrap();
        assert_eq!(restored, v9);
        assert_eq!(canonical(&restored), canonical(&v9));
        assert_eq!(canonical(&v9).as_slice(), V9.as_bytes());
        assert_eq!(canonical(&v10).as_slice(), V10.as_bytes());
        assert_eq!(V9.len(), 267_520);
        assert_eq!(V10.len(), 251_683);
        assert!(V10.len() <= super::super::super::PAGE_CAP - 10 * 1024);
    }

    #[test]
    fn v10_archive_prefixes_restore_the_entire_v9_page_before_old_readers() {
        let full = fixture();
        let compact = encode_v10(&full, &["docs/evidence/", "docs/evidence/fmt/"]).unwrap();
        assert_eq!(expand_v10_to_v9(&compact).unwrap(), full);
        assert_eq!(
            compact["replacements"][0]["old519_index"][1][2],
            serde_json::json!([0, "a/one.txt"])
        );
        assert_eq!(
            compact["replacements"][0]["old519_index"][2][0][3],
            serde_json::json!([1, "two.txt"])
        );
        assert_eq!(compact["unknown"], full["unknown"]);
        assert_eq!(
            compact["additions"][0]["fragments"][0],
            full["additions"][0]["fragments"][0]
        );
        assert_eq!(
            canonical(&compact),
            canonical(&encode_v10(&full, &["docs/evidence/", "docs/evidence/fmt/"]).unwrap())
        );
    }

    #[test]
    fn v10_refuses_foreign_prefix_tables_and_archive_references() {
        let compact = encode_v10(&fixture(), &["docs/evidence/", "docs/evidence/fmt/"]).unwrap();
        fn current(page: &mut Value) -> &mut Value {
            &mut page["replacements"][0]["old519_index"][1][2]
        }
        for mode in 0..11 {
            let mut page = compact.clone();
            match mode {
                0 => page["schema"] = SCHEMA_V9.into(),
                1 => page[PREFIXES] = Value::Array(Vec::new()),
                2 => {
                    page[PREFIXES] = Value::Array(
                        (0..=MAX_PREFIXES)
                            .map(|i| format!("e/{i}/").into())
                            .collect(),
                    )
                }
                3 => page[PREFIXES] = serde_json::json!(["docs/evidence/", "docs/evidence/"]),
                4 => page[PREFIXES] = serde_json::json!(["../unsafe/"]),
                5 => *current(&mut page) = serde_json::json!([99, "a/one.txt"]),
                6 => *current(&mut page) = serde_json::json!([false, "a/one.txt"]),
                7 => *current(&mut page) = serde_json::json!([0]),
                8 => *current(&mut page) = serde_json::json!([0, false]),
                9 => *current(&mut page) = "docs/evidence/a/one.txt".into(),
                _ => {
                    page[PREFIXES]
                        .as_array_mut()
                        .unwrap()
                        .push("docs/evidence/unused/".into());
                }
            }
            assert!(expand_v10_to_v9(&page).is_err(), "v10 mode={mode}");
        }
        let mut nonlongest = compact.clone();
        nonlongest["replacements"][0]["old519_index"][2][0][3] =
            serde_json::json!([0, "fmt/two.txt"]);
        assert_eq!(
            expand_v10_to_v9(&nonlongest).unwrap_err(),
            "v10 noncanonical archive prefix"
        );
        let mut traversal = compact;
        *current(&mut traversal) = serde_json::json!([0, "../escape"]);
        assert_eq!(
            expand_v10_to_v9(&traversal).unwrap_err(),
            "v10 expanded archive path"
        );
    }
}

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
pub(in super::super) fn expand_v10_old519(
    before: &Value,
    policy: &Value,
    page: &Value,
) -> Result<Value, &'static str> {
    expand_v9(before, policy, &expand_v10_pins(before, policy, page)?)
}
#[cfg(test)]
mod old519_tests {
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
    #[test]
    fn v10_envelope_dispatch_preserves_both_formats_and_refuses_malformed_prefixes() {
        let (before, policy, page) = fixture();
        let compact = encode(&before, &policy, &page).unwrap();
        assert_eq!(
            compose_values(before.clone(), &policy, &compact).unwrap(),
            compose_values(before.clone(), &policy, &page).unwrap(),
        );
        assert_eq!(
            expand_v10_compatible(&before, &policy, &compact).unwrap(),
            expand_v10_old519(&before, &policy, &compact).unwrap()
        );
        for marker in [
            Value::Null,
            serde_json::json!([]),
            serde_json::json!(false),
            serde_json::json!(["bad"]),
        ] {
            let mut malformed = compact.clone();
            malformed["archive_prefixes"] = marker;
            assert!(expand_v10_compatible(&before, &policy, &malformed).is_err());
            assert!(compose_values(before.clone(), &policy, &malformed).is_err());
        }
        let current = include_str!("main_e177_overlay_pins_v10.rs");
        let prior = crate::source_reflows::next_custody_before(
            "main295-prefix-v10",
            "src/source_reflows_merge/main_e177_overlay_pins_v10.rs",
            current,
        );
        assert_eq!(prior.len(), 13007);
        assert_eq!(
            digest(prior.as_bytes()),
            "328669b88dbafd489e7efaba97b2fd42e48f3d6661e475b02020b4752ea89672"
        );
        let prior = crate::source_reflows::next_custody_before(
            "main295-old519-v10",
            "src/source_reflows_merge/main_e177_overlay_pins_v10.rs",
            current,
        );
        assert_eq!(prior.len(), 11088);
        assert_eq!(
            digest(prior.as_bytes()),
            "139e342c746175490471415451808f4882720303027f6373dd48562e6a814440"
        );
        for schema in [super::super::SCHEMA_V11, super::super::SCHEMA_V12] {
            let malformed = serde_json::json!({"schema":schema,"replacements":[],"additions":[]});
            assert!(compose_values(before.clone(), &policy, &malformed).is_err());
        }
    }
}

// Only the standalone V10 envelope admits the old519 reference encoding.
// A present malformed prefix table never falls through to another decoder.
pub(in super::super) fn expand_v10_compatible(
    before: &Value,
    policy: &Value,
    page: &Value,
) -> Result<Value, &'static str> {
    if page.get(PREFIXES).is_some() {
        expand_v10(before, policy, page)
    } else {
        expand_v10_old519(before, policy, page)
    }
}
