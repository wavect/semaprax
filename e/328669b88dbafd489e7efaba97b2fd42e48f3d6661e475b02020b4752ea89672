use super::{expand_v9, SCHEMA_V9};
use serde_json::Value;
use std::collections::BTreeSet;

pub(in super::super) const SCHEMA_V10: &str = "semaprax.finite-source-overlay-page.v10";
const PREFIXES: &str = "archive_prefixes";
const MAX_PREFIXES: usize = 32;

fn prefix_table(page: &Value) -> Result<Vec<String>, &'static str> {
    let values = page[PREFIXES]
        .as_array()
        .ok_or("v10 archive prefixes")?;
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

fn pin_slot<F>(
    value: &mut Value,
    named: bool,
    archive: &mut F,
) -> Result<(), &'static str>
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
    let reference = tuple[count]
        .as_array()
        .ok_or("v10 archive reference")?;
    if reference.len() != 2 {
        return Err("v10 archive reference");
    }
    let index = usize::try_from(
        reference[0]
            .as_u64()
            .ok_or("v10 archive prefix index")?,
    )
    .map_err(|_| "v10 archive prefix index")?;
    let prefix = prefixes
        .get(index)
        .ok_or("v10 archive prefix index")?;
    let suffix = reference[1]
        .as_str()
        .ok_or("v10 archive suffix type")?;
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
        let mut encoded = archive_slots(page, |value, named| {
            encode_archive(value, named, &values)
        })?;
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
        const V10: &str = include_str!("../../docs/evidence/finite-source-overlay-v2-20261018/v10-unqualified-full-union.json");
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
        let compact = encode_v10(&full, &["docs/evidence/", "docs/evidence/fmt/"])
            .unwrap();
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
        let compact = encode_v10(&fixture(), &["docs/evidence/", "docs/evidence/fmt/"])
            .unwrap();
        fn current(page: &mut Value) -> &mut Value {
            &mut page["replacements"][0]["old519_index"][1][2]
        }
        for mode in 0..11 {
            let mut page = compact.clone();
            match mode {
                0 => page["schema"] = SCHEMA_V9.into(),
                1 => page[PREFIXES] = Value::Array(Vec::new()),
                2 => page[PREFIXES] = Value::Array(
                    (0..=MAX_PREFIXES).map(|i| format!("e/{i}/").into()).collect(),
                ),
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
