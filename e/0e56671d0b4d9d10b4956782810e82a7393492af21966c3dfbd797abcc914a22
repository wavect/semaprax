//! Pure canonical-row references for the same one bounded metadata page.
use serde_json::Value;
use std::collections::{BTreeMap, BTreeSet};
pub(super) const SCHEMA: &str = "semaprax.finite-source-overlay-page.v3";
pub(super) const SCHEMA_V4: &str = "semaprax.finite-source-overlay-page.v4";
pub(super) const SCHEMA_V8: &str = "semaprax.finite-source-overlay-page.v8";
pub(super) const SCHEMA_V7: &str = "semaprax.finite-source-overlay-page.v7";
pub(super) const SCHEMA_V6: &str = "semaprax.finite-source-overlay-page.v6";
pub(super) const SCHEMA_V5: &str = "semaprax.finite-source-overlay-page.v5";
const EXTRA519: &[(&str, &str)] = &[
    (
        "src/cleanup_plan/replay/expression_facts/arena/retained/tests.rs",
        "e9f1208c4daeeb93e1200a127c9dee49e57c3ee370c10c5c0305c8c4266d1443",
    ),
    (
        "src/cleanup_plan/replay.rs",
        "85efc15e0205f0f2c4070c9d80b6f30b50f73bc6e4070854484bdd75e8fb9a8a",
    ),
];
const OLD519: &[(&str, &str)] = &[
    (
        "src/source_reflows_merge.rs",
        "3ef2bb440f23c573007567c98b4bfc608b555bd2cc8f2591d365476b850bd46a",
    ),
    (
        "src/source_reflows/incoming67107_semantic_receivers.rs",
        "383eff4edb39826861991d6dcd84dc0c489471e90e66ed214b4d5ad46a907ac1",
    ),
];
const REFERENCE: &str = "canonical_source";
const ROOTS: &[&str] = &[
    "tests/hir_module_boundaries.rs",
    "tests/workspace/semantic_graph/source_inventory.rs",
    "crates/semaprax-native-rust-interop-builder/src/implementation/tests/ledger_capacity/source_inventory.rs",
];

// No path is opened, normalized, followed or granted here. All targets already
// are complete canonical rows in the predecessor or this same literal page.
pub(super) fn expand(before: &Value, policy: &Value, page: &Value) -> Result<Value, &'static str> {
    if page["schema"] != SCHEMA {
        return Err("v3 schema");
    }
    let old = before["rows"].as_array().ok_or("v3 old rows")?;
    let mut canonical = BTreeMap::new();
    for row in old {
        let source = row["source"].as_str().ok_or("v3 old source")?;
        if canonical.insert(source, row).is_some() {
            return Err("v3 duplicate old source");
        }
    }
    let mut overlay = BTreeSet::new();
    for group in ["replacements", "additions"] {
        for row in page[group].as_array().ok_or("v3 rows")? {
            let source = row["source"].as_str().ok_or("v3 source")?;
            if !overlay.insert(source) {
                return Err("v3 duplicate canonical source");
            }
            canonical.insert(source, row);
        }
    }
    let mut allowed: BTreeSet<_> = super::edges(before)?.into_iter().collect();
    for edge in policy["new_fragment_edges"].as_array().ok_or("v3 edges")? {
        if edge.as_object().ok_or("v3 edge")?.len() != 2 {
            return Err("v3 edge fields");
        }
        allowed.insert((
            edge["parent"].as_str().ok_or("v3 parent")?.to_owned(),
            edge["child"].as_str().ok_or("v3 child")?.to_owned(),
        ));
    }
    let mut expanded = page.clone();
    expanded["schema"] = Value::String("semaprax.finite-source-overlay-page.v2".to_owned());
    for group in ["replacements", "additions"] {
        for row in expanded[group].as_array_mut().ok_or("v3 rows")? {
            let parent = row["source"].as_str().ok_or("v3 parent")?.to_owned();
            let Some(fragments) = row.get_mut("fragments") else {
                continue;
            };
            let mut seen = BTreeSet::new();
            for fragment in fragments.as_array_mut().ok_or("v3 fragments")? {
                if fragment.get(REFERENCE).is_none() {
                    continue;
                }
                if !ROOTS.contains(&parent.as_str()) {
                    return Err("v3 nonliteral root");
                }
                if fragment.as_object().ok_or("v3 reference")?.len() != 1 {
                    return Err("v3 reference fields");
                }
                let child = fragment[REFERENCE].as_str().ok_or("v3 reference source")?;
                let edge = (parent.clone(), child.to_owned());
                if parent == child || !allowed.contains(&edge) || !seen.insert(edge) {
                    return Err("v3 undeclared or duplicate reference");
                }
                let target = *canonical.get(child).ok_or("v3 missing canonical row")?;
                if target["compiled"] != true || target.get("fragments").is_some() {
                    return Err("v3 noncompiled or nested reference");
                }
                *fragment = target.clone();
            }
        }
    }
    Ok(expanded)
}

// Only these two already authenticated old519 records can be inherited. The
// tuple is [literal index, current length, current SHA256, additional views].
// Its current archive is exactly the existing e/<full SHA256> image channel.
// No path, file, permission, parsed authority or accepted result is acquired.
pub(super) fn expand_v4(
    before: &Value,
    policy: &Value,
    page: &Value,
) -> Result<Value, &'static str> {
    if page["schema"] != SCHEMA_V4 {
        return Err("v4 schema");
    }
    let old = before["rows"].as_array().ok_or("v4 old rows")?;
    if old.len() != 519 {
        return Err("v4 whole old519");
    }
    let mut expanded = page.clone();
    expanded["schema"] = SCHEMA.into();
    let mut seen = BTreeSet::new();
    for row in expanded["replacements"]
        .as_array_mut()
        .ok_or("v4 replacements")?
    {
        let Some(reference) = row.get("old519") else {
            if OLD519.iter().any(|(source, _)| row["source"] == *source) {
                return Err("v4 missing literal inheritance");
            }
            continue;
        };
        if row.as_object().ok_or("v4 record")?.len() != 1 {
            return Err("v4 reference fields");
        }
        let tuple = reference.as_array().ok_or("v4 tuple")?;
        if tuple.len() != 4 {
            return Err("v4 tuple fields");
        }
        let index = usize::try_from(tuple[0].as_u64().ok_or("v4 literal index")?)
            .map_err(|_| "v4 literal index")?;
        let (source, sha) = *OLD519.get(index).ok_or("v4 foreign literal index")?;
        if !seen.insert(index) {
            return Err("v4 duplicate literal index");
        }
        let mut matches = old.iter().filter(|r| r["source"] == source);
        let prior = matches.next().ok_or("v4 missing old row")?;
        if matches.next().is_some() || super::digest(&super::canonical(prior)) != sha {
            return Err("v4 complete old row drift");
        }
        let hash = tuple[2].as_str().ok_or("v4 current hash")?;
        let current = serde_json::json!({"length":tuple[1].clone(), "sha256":hash,
            "archive":format!("e/{hash}")});
        super::pin(&current)?;
        if current == prior["current"] {
            return Err("v4 unchanged replacement");
        }
        let extra = tuple[3].as_array().ok_or("v4 appended views")?;
        for view in extra {
            super::pin(view)?;
        }
        let mut full = prior.clone();
        full["current"] = current;
        let mut retained = prior["current"].clone();
        retained["name"] = "whole-pre-overlay-v2-current".into();
        let views = full["views"].as_array_mut().ok_or("v4 old views")?;
        views.push(retained);
        views.extend(extra.iter().cloned());
        *row = full;
    }
    if seen.len() != OLD519.len() {
        return Err("v4 missing literal reference");
    }
    if expanded["additions"]
        .as_array()
        .ok_or("v4 additions")?
        .iter()
        .any(|row| row.get("old519").is_some())
    {
        return Err("v4 reference in additions");
    }
    expand(before, policy, &expanded)
}

// Exactly two additional immutable rows; the complete V4 reader below remains
// the authority for its original two references and every downstream check.
pub(super) fn expand_v5(
    before: &Value,
    policy: &Value,
    page: &Value,
) -> Result<Value, &'static str> {
    if page["schema"] != SCHEMA_V5 {
        return Err("v5 schema");
    }
    let old = before["rows"].as_array().ok_or("v5 old rows")?;
    if old.len() != 519 {
        return Err("v5 whole old519");
    }
    let mut expanded = page.clone();
    expanded["schema"] = SCHEMA_V4.into();
    let mut seen = BTreeSet::new();
    for row in expanded["replacements"]
        .as_array_mut()
        .ok_or("v5 replacements")?
    {
        let Some(reference) = row.get("old519") else {
            if EXTRA519.iter().any(|(source, _)| row["source"] == *source) {
                return Err("v5 missing literal inheritance");
            }
            continue;
        };
        let tuple = reference.as_array().ok_or("v5 tuple")?;
        if tuple.len() != 4 {
            return Err("v5 tuple fields");
        }
        let index = usize::try_from(tuple[0].as_u64().ok_or("v5 literal index")?)
            .map_err(|_| "v5 literal index")?;
        if index < 2 {
            continue;
        } // V4 validates these complete records, unchanged.
        if row.as_object().ok_or("v5 record")?.len() != 1 {
            return Err("v5 reference fields");
        }
        let (source, sha) = *EXTRA519.get(index - 2).ok_or("v5 foreign literal index")?;
        if !seen.insert(index) {
            return Err("v5 duplicate literal index");
        }
        let mut matches = old.iter().filter(|r| r["source"] == source);
        let prior = matches.next().ok_or("v5 missing old row")?;
        if matches.next().is_some() || super::digest(&super::canonical(prior)) != sha {
            return Err("v5 complete old row drift");
        }
        let hash = tuple[2].as_str().ok_or("v5 current hash")?;
        let current = serde_json::json!({"length":tuple[1].clone(),"sha256":hash,"archive":format!("e/{hash}")});
        super::pin(&current)?;
        if current == prior["current"] {
            return Err("v5 unchanged replacement");
        }
        let extra = tuple[3].as_array().ok_or("v5 appended views")?;
        for view in extra {
            super::pin(view)?;
        }
        let mut full = prior.clone();
        full["current"] = current;
        let mut retained = prior["current"].clone();
        retained["name"] = "whole-pre-overlay-v2-current".into();
        let views = full["views"].as_array_mut().ok_or("v5 old views")?;
        views.push(retained);
        views.extend(extra.iter().cloned());
        *row = full;
    }
    if seen.len() != EXTRA519.len() {
        return Err("v5 missing literal reference");
    }
    expand_v4(before, policy, &expanded)
}

// General references index only the same complete authenticated old519 object.
// No source name, compiled flag, path, or unknown old field can be overridden.
pub(super) fn expand_v6(
    before: &Value,
    policy: &Value,
    page: &Value,
) -> Result<Value, &'static str> {
    if page["schema"] != SCHEMA_V6 {
        return Err("v6 schema");
    }
    let old = before["rows"].as_array().ok_or("v6 old rows")?;
    if old.len() != 519
        || policy["predecessor"]["rows"].as_u64() != Some(519)
        || before["source_cap"].as_u64() != Some(super::SOURCE_CAP as u64)
        || policy["predecessor"]["logical_sha256"].as_str()
            != Some(super::digest(&super::canonical(before)).as_str())
    {
        return Err("v6 authenticated old519");
    }
    let mut expanded = page.clone();
    expanded["schema"] = SCHEMA.into();
    let mut seen = BTreeSet::new();
    for row in expanded["replacements"]
        .as_array_mut()
        .ok_or("v6 replacements")?
    {
        if row.get("old519").is_some() {
            return Err("v6 legacy reference");
        }
        let Some(reference) = row.get("old519_index") else {
            continue;
        };
        if row.as_object().ok_or("v6 record")?.len() != 1 {
            return Err("v6 reference fields");
        }
        let tuple = reference.as_array().ok_or("v6 tuple")?;
        if tuple.len() != 4 {
            return Err("v6 tuple fields");
        }
        let index =
            usize::try_from(tuple[0].as_u64().ok_or("v6 index")?).map_err(|_| "v6 index")?;
        let prior = old.get(index).ok_or("v6 foreign index")?;
        if !seen.insert(index) {
            return Err("v6 duplicate index");
        }
        super::pin(&tuple[1])?;
        let suffix = tuple[2].as_array().ok_or("v6 appended views")?;
        for view in suffix {
            super::pin(view)?;
        }
        let mut full = prior.clone();
        full["current"] = tuple[1].clone();
        full["views"]
            .as_array_mut()
            .ok_or("v6 old views")?
            .extend(suffix.iter().cloned());
        if !tuple[3].is_null() {
            if !ROOTS.iter().any(|source| prior["source"] == *source) || !tuple[3].is_array() {
                return Err("v6 fragment override root or shape");
            }
            full["fragments"] = tuple[3].clone();
        }
        *row = full;
    }
    if expanded["additions"]
        .as_array()
        .ok_or("v6 additions")?
        .iter()
        .any(|row| row.get("old519_index").is_some() || row.get("old519").is_some())
    {
        return Err("v6 reference in additions");
    }
    // V3 still restricts reference roots, declared edges and compiled leaf
    // targets. V2 then checks every literal policy row and the complete inverse.
    expand(before, policy, &expanded)
}

// V8 alone factors these two complete pinned old519 carriers. Legacy V6/V3
// restrictions remain byte-for-byte unchanged; only full ordinary rows reach them.
const V8_CARRIERS: &[(usize, &str, &str, usize, &str)] = &[
    (
        38,
        "src/source_reflows_merge/main67107_inputs.rs",
        "ae571c1bf1f461c0485d0e5f4c5b231af8d7f5889e99e81d04e55ef86bd561d8",
        366,
        "src/source_reflows_merge/main67107_inputs_tail.rs",
    ),
    (
        314,
        "src/hir/resolve_program.rs",
        "4bc3d8948fadbac1745748fa65dec109e2e2f2aad21147bdc41b9b4fb0b76913",
        316,
        "src/hir/resolve_program_methods.rs",
    ),
];
fn v8_inherited_row(old: &[Value], row: &Value) -> Result<Value, &'static str> {
    let tuple = row["old519_index"].as_array().ok_or("v8 carrier tuple")?;
    if row.as_object().ok_or("v8 carrier fields")?.len() != 1 || tuple.len() != 4 {
        return Err("v8 carrier fields");
    }
    let index = usize::try_from(tuple[0].as_u64().ok_or("v8 carrier index")?)
        .map_err(|_| "v8 carrier index")?;
    let mut full = old.get(index).ok_or("v8 carrier foreign index")?.clone();
    super::pin(&tuple[1])?;
    full["current"] = tuple[1].clone();
    let extra = tuple[2].as_array().ok_or("v8 carrier views")?;
    for view in extra {
        super::pin(view)?;
    }
    full["views"]
        .as_array_mut()
        .ok_or("v8 carrier old views")?
        .extend(extra.iter().cloned());
    if !tuple[3].is_null() {
        full["fragments"] = tuple[3].clone();
    }
    Ok(full)
}
fn v8_carriers(before: &Value, page: &Value) -> Result<Value, &'static str> {
    let old = before["rows"].as_array().ok_or("v8 carrier old rows")?;
    let rows = page["replacements"]
        .as_array()
        .ok_or("v8 carrier replacements")?;
    let mut expanded = page.clone();
    let mut seen = BTreeSet::new();
    for (position, row) in rows.iter().enumerate() {
        let Some(tuple) = row.get("old519_index") else {
            continue;
        };
        let tuple = tuple.as_array().ok_or("v8 carrier tuple")?;
        if tuple.len() != 4 {
            return Err("v8 carrier tuple fields");
        }
        let index = usize::try_from(tuple[0].as_u64().ok_or("v8 carrier index")?)
            .map_err(|_| "v8 carrier index")?;
        let Some(&(index, source, hash, child_index, child)) =
            V8_CARRIERS.iter().find(|entry| entry.0 == index)
        else {
            continue;
        };
        if tuple[3].is_null() {
            continue;
        }
        if !seen.insert(index) {
            return Err("v8 duplicate carrier");
        }
        let prior = old.get(index).ok_or("v8 carrier missing old row")?;
        if prior["source"] != source
            || super::digest(&super::canonical(prior)) != hash
            || old.get(child_index).ok_or("v8 carrier missing child")?["source"] != child
        {
            return Err("v8 carrier old identity");
        }
        let fragments = tuple[3].as_array().ok_or("v8 carrier fragments")?;
        if fragments.len() != 1
            || fragments[0]
                .as_object()
                .ok_or("v8 carrier reference")?
                .len()
                != 1
            || fragments[0][REFERENCE].as_str() != Some(child)
        {
            return Err("v8 carrier exact child");
        }
        if !super::edges(before)?.contains(&(source.to_owned(), child.to_owned())) {
            return Err("v8 carrier old edge");
        }
        let mut target = old[child_index].clone();
        let mut found = false;
        for candidate in rows {
            let full = if candidate.get("old519_index").is_some() {
                let t = candidate["old519_index"]
                    .as_array()
                    .ok_or("v8 child tuple")?;
                if t.len() != 4 {
                    return Err("v8 child tuple fields");
                }
                if t[0].as_u64() != Some(child_index as u64) {
                    continue;
                }
                if !t[3].is_null() {
                    return Err("v8 nested child override");
                }
                v8_inherited_row(old, candidate)?
            } else {
                if candidate["source"] != child {
                    continue;
                }
                candidate.clone()
            };
            if found {
                return Err("v8 duplicate child");
            }
            found = true;
            target = full;
        }
        if !found {
            return Err("v8 missing canonical child");
        }
        if target["source"] != child
            || target["compiled"] != true
            || target.get("fragments").is_some()
        {
            return Err("v8 noncompiled or nested child");
        }
        let mut full = v8_inherited_row(old, row)?;
        full["fragments"] = serde_json::json!([target]);
        expanded["replacements"][position] = full;
    }
    Ok(expanded)
}

// Numeric names are transport within this one fixed page, never new paths.
// The table is authenticated whole519 followed by this page's ordered additions.
// All V6, V3 and V2 row, leaf, edge, predecessor and inverse checks replay.
pub(super) fn expand_v7(
    before: &Value,
    policy: &Value,
    page: &Value,
) -> Result<Value, &'static str> {
    if page["schema"] != SCHEMA_V7 {
        return Err("v7 schema");
    }
    let old = before["rows"].as_array().ok_or("v7 old rows")?;
    if old.len() != 519
        || policy["predecessor"]["rows"].as_u64() != Some(519)
        || before["source_cap"].as_u64() != Some(super::SOURCE_CAP as u64)
        || policy["predecessor"]["logical_sha256"].as_str()
            != Some(super::digest(&super::canonical(before)).as_str())
    {
        return Err("v7 authenticated old519");
    }
    let mut names = Vec::new();
    let mut unique = BTreeSet::new();
    for row in old
        .iter()
        .chain(page["additions"].as_array().ok_or("v7 additions")?)
    {
        let source = row["source"].as_str().ok_or("v7 table source")?;
        if !unique.insert(source) {
            return Err("v7 duplicate table source");
        }
        names.push(source);
    }
    let mut expanded = page.clone();
    expanded["schema"] = SCHEMA_V6.into();
    for group in ["replacements", "additions"] {
        for row in expanded[group].as_array_mut().ok_or("v7 rows")? {
            let inherited = row.get("old519_index").is_some();
            let parent = if inherited {
                let tuple = row["old519_index"].as_array().ok_or("v7 tuple")?;
                if tuple.len() != 4 {
                    return Err("v7 tuple fields");
                }
                let index = usize::try_from(tuple[0].as_u64().ok_or("v7 old index")?)
                    .map_err(|_| "v7 old index")?;
                old.get(index).ok_or("v7 foreign old index")?["source"]
                    .as_str()
                    .ok_or("v7 parent")?
                    .to_owned()
            } else {
                row["source"].as_str().ok_or("v7 parent")?.to_owned()
            };
            let fragments = if inherited {
                let tuple = row["old519_index"].as_array_mut().ok_or("v7 tuple")?;
                if tuple[3].is_null() {
                    continue;
                }
                &mut tuple[3]
            } else {
                let Some(fragments) = row.get_mut("fragments") else {
                    continue;
                };
                fragments
            };
            for fragment in fragments.as_array_mut().ok_or("v7 fragments")? {
                let Some(reference) = fragment.get(REFERENCE) else {
                    continue;
                };
                if reference.is_string() {
                    continue;
                }
                if !ROOTS.contains(&parent.as_str())
                    || fragment.as_object().ok_or("v7 reference")?.len() != 1
                {
                    return Err("v7 literal root or reference fields");
                }
                let index = usize::try_from(reference.as_u64().ok_or("v7 numeric index")?)
                    .map_err(|_| "v7 numeric index")?;
                if index < 519 {
                    return Err("v7 old numeric index");
                }
                let source = names.get(index).ok_or("v7 foreign numeric index")?;
                fragment[REFERENCE] = (*source).into();
            }
        }
    }
    expand_v6(before, policy, &expanded)
}

// Only V8 admits the two pinned carrier/leaf pairs. Entire legacy readers follow.
pub(super) fn expand_v8(
    before: &Value,
    policy: &Value,
    page: &Value,
) -> Result<Value, &'static str> {
    if page["schema"] != SCHEMA_V8 {
        return Err("v8 schema");
    }
    let old = before["rows"].as_array().ok_or("v8 old rows")?;
    if old.len() != 519
        || policy["predecessor"]["rows"].as_u64() != Some(519)
        || before["source_cap"].as_u64() != Some(super::SOURCE_CAP as u64)
        || policy["predecessor"]["logical_sha256"].as_str()
            != Some(super::digest(&super::canonical(before)).as_str())
    {
        return Err("v8 authenticated old519");
    }
    let mut expanded = v8_carriers(before, page)?;
    expanded["schema"] = SCHEMA_V7.into();
    expand_v7(before, policy, &expanded)
}

#[path = "main_e177_overlay_pins_v9.rs"]
mod pins_v9;
pub(super) use pins_v9::{expand_v9, SCHEMA_V9};
#[path = "main_e177_overlay_pins_v10.rs"]
mod pins_v10;
pub(super) use pins_v10::{expand_v10, expand_v11, SCHEMA_V10, SCHEMA_V11};

#[cfg(test)]
mod tests {
    use super::super::{canonical, compose_values, compose_values_v2, digest, held_4a6eb};
    use super::*;
    fn record(source: &str) -> Value {
        serde_json::json!({"source":source,"compiled":true,"current":{"length":1,
            "sha256":"a".repeat(64),"archive":"e/fixture"},"views":[],"unknown":{"kept":[1,2]}})
    }
    fn fixture() -> (Value, Value, Value, Value) {
        let child = record("src/v3_fixture.rs");
        let mut root = record(ROOTS[0]);
        root["fragments"] = serde_json::json!([child]);
        let before = serde_json::json!({"rows":[root,child],"source_cap":9753094,"unknown":[3]});
        let pred = serde_json::json!({"rows":2,"logical_sha256":digest(&canonical(&before))});
        let page = serde_json::json!({"schema":"semaprax.finite-source-overlay-page.v2",
            "index":1,"total_pages":1,"predecessor":pred,"replacements":[root],"additions":[]});
        let policy = serde_json::json!({"predecessor":pred,"replacements":[ROOTS[0]],
            "additions":[],"new_fragment_edges":[]});
        let mut compact = page.clone();
        compact["schema"] = SCHEMA.into();
        compact["replacements"][0]["fragments"][0] =
            serde_json::json!({"canonical_source":"src/v3_fixture.rs"});
        (before, policy, page, compact)
    }
    #[test]
    fn v3_exact_canonical_expansion_keeps_every_v2_field_order_and_inverse() {
        let (before, policy, page, compact) = fixture();
        assert_eq!(expand(&before, &policy, &compact).unwrap(), page);
        assert_eq!(
            compose_values(before.clone(), &policy, &compact).unwrap(),
            compose_values_v2(before.clone(), &policy, &page).unwrap()
        );
        assert_eq!(
            compose_values(before.clone(), &policy, &compact).unwrap(),
            before
        );
    }
    #[test]
    fn v3_refuses_foreign_missing_duplicate_nested_and_noncompiled_references() {
        let (before, policy, _, compact) = fixture();
        for mode in 0..8 {
            let b = before.clone();
            let mut p = compact.clone();
            match mode {
                0 => p["replacements"][0]["source"] = "src/foreign_root.rs".into(),
                1 => p["replacements"][0]["fragments"][0][REFERENCE] = "../foreign.rs".into(),
                2 => {
                    let item = p["replacements"][0]["fragments"][0].clone();
                    p["replacements"][0]["fragments"]
                        .as_array_mut()
                        .unwrap()
                        .push(item);
                }
                3 => p["replacements"][0]["fragments"][0]["extra"] = true.into(),
                4 => {
                    let mut nested = b["rows"][1].clone();
                    nested["fragments"] = serde_json::json!([]);
                    p["replacements"].as_array_mut().unwrap().push(nested);
                }
                5 => {
                    let mut foreign = b["rows"][1].clone();
                    foreign["compiled"] = false.into();
                    p["replacements"].as_array_mut().unwrap().push(foreign);
                }
                6 => {
                    let item = p["replacements"][0].clone();
                    p["replacements"].as_array_mut().unwrap().push(item);
                }
                _ => p["replacements"][0]["fragments"][0][REFERENCE] = ROOTS[0].into(),
            }
            assert!(expand(&b, &policy, &p).is_err(), "mode={mode}");
        }
        let mut p = compact.clone();
        p["index"] = 2.into();
        assert!(compose_values(before.clone(), &policy, &p).is_err());
        let mut p = compact;
        p["replacements"][0]["unknown"] = false.into();
        assert!(compose_values(before, &policy, &p).is_err());
    }
    #[test]
    fn v3_replays_the_complete_frozen_v2_page_through_unchanged_composer() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
        let read = |name: &str, n, sha| {
            assert!(n <= 262144);
            held_4a6eb(&root.join(name), n, sha).unwrap()
        };
        let raw = read(
            "e/381c6d9fb1b93e2c92c2863ecdeeee84ea8cf4e5eaa08e7e4381fe85d5ca66fc",
            247814,
            "381c6d9fb1b93e2c92c2863ecdeeee84ea8cf4e5eaa08e7e4381fe85d5ca66fc",
        );
        let policy = read(
            "e/1d3bcd7ebbb484f1909e120b8c2f1e9af399d5b4306e0f9789371df8154f7965",
            28483,
            "1d3bcd7ebbb484f1909e120b8c2f1e9af399d5b4306e0f9789371df8154f7965",
        );
        let page: Value = serde_json::from_str(&raw).unwrap();
        let policy: Value = serde_json::from_str(&policy).unwrap();
        let before = super::super::super::packet_before_overlay();
        let mut compact = page.clone();
        compact["schema"] = SCHEMA.into();
        for group in ["replacements", "additions"] {
            for row in compact[group].as_array_mut().unwrap() {
                if ROOTS.contains(&row["source"].as_str().unwrap()) {
                    for child in row["fragments"].as_array_mut().unwrap() {
                        *child = serde_json::json!({"canonical_source":child["source"].clone()});
                    }
                }
            }
        }
        assert_eq!(expand(&before, &policy, &compact).unwrap(), page);
        assert_eq!(
            compose_values(before.clone(), &policy, &compact).unwrap(),
            compose_values_v2(before, &policy, &page).unwrap()
        );
        let source = include_str!("main_e177_overlay_v2.rs");
        let body = source
            .split_once("fn compose_values_v2(")
            .unwrap()
            .1
            .split_once("\npub(super) fn compose")
            .unwrap()
            .0;
        let restored = format!("fn compose_values({body}");
        assert_eq!(restored.len(), 5763);
        assert_eq!(
            digest(restored.as_bytes()),
            "db2c2411d57188a8397ca9e5e87f192e99be0f1d6cd7105f5c90ac4fe7419402"
        );
    }

    fn v4_fixture() -> (Value, Value, Value, Value) {
        let before = super::super::super::packet_before_overlay();
        let predecessor =
            serde_json::json!({"rows":519,"logical_sha256":digest(&canonical(&before))});
        let mut rows = Vec::new();
        let mut refs = Vec::new();
        let mut paths = Vec::new();
        for prior in before["rows"].as_array().unwrap() {
            let Some(index) = OLD519
                .iter()
                .position(|(source, _)| prior["source"] == *source)
            else {
                continue;
            };
            let hash = digest(format!("fixed-v4-current-{index}").as_bytes());
            let current =
                serde_json::json!({"length":1,"sha256":hash,"archive":format!("e/{hash}")});
            let extra = serde_json::json!([{"name":"complete-extra","length":1,
                "sha256":hash,"archive":format!("e/{hash}"),"opaque":{"kept":[3,1,2]}}]);
            let mut row = prior.clone();
            row["current"] = current;
            let mut retained = prior["current"].clone();
            retained["name"] = "whole-pre-overlay-v2-current".into();
            row["views"].as_array_mut().unwrap().push(retained);
            row["views"]
                .as_array_mut()
                .unwrap()
                .extend(extra.as_array().unwrap().iter().cloned());
            paths.push(prior["source"].clone());
            rows.push(row);
            refs.push(serde_json::json!({"old519":[index,1,hash,extra]}));
        }
        assert_eq!(rows.len(), 2);
        let full = serde_json::json!({"schema":"semaprax.finite-source-overlay-page.v2",
            "index":1,"total_pages":1,"predecessor":predecessor,"replacements":rows,"additions":[]});
        let policy = serde_json::json!({"predecessor":predecessor,"replacements":paths,
            "additions":[],"new_fragment_edges":[]});
        let mut compact = full.clone();
        compact["schema"] = SCHEMA_V4.into();
        compact["replacements"] = refs.into();
        (before, policy, full, compact)
    }

    #[test]
    fn v4_fixed_old_rows_expand_to_every_original_field_and_whole_inverse() {
        let (before, policy, full, compact) = v4_fixture();
        assert_eq!(expand_v4(&before, &policy, &compact).unwrap(), full);
        assert_eq!(
            compose_values(before.clone(), &policy, &compact).unwrap(),
            compose_values_v2(before.clone(), &policy, &full).unwrap()
        );
        let current = compose_values(before.clone(), &policy, &compact).unwrap();
        let mut restored = current.clone();
        restored["rows"] = before["rows"].clone();
        assert_eq!(
            restored, before,
            "entire predecessor object and ordered inventories"
        );
        for prior in before["rows"].as_array().unwrap() {
            let actual = current["rows"]
                .as_array()
                .unwrap()
                .iter()
                .find(|r| r["source"] == prior["source"])
                .unwrap();
            if OLD519.iter().any(|(source, _)| prior["source"] == *source) {
                let mut inverse = actual.clone();
                inverse["current"] = prior["current"].clone();
                inverse["views"] = prior["views"].clone();
                assert_eq!(&inverse, prior, "every inherited field and unknown field");
            } else {
                assert_eq!(actual, prior);
            }
        }
    }

    #[test]
    fn v4_refuses_missing_foreign_duplicate_tampered_drift_and_unknown_version() {
        let (before, policy, full, compact) = v4_fixture();
        for mode in 0..15 {
            let mut b = before.clone();
            let mut p = compact.clone();
            match mode {
                0 => {
                    p["replacements"].as_array_mut().unwrap().pop();
                }
                1 => p["replacements"][0]["old519"][0] = 2.into(),
                2 => p["replacements"][1] = p["replacements"][0].clone(),
                3 => p["replacements"][0]["extra"] = true.into(),
                4 => p["replacements"][0]["old519"][2] = "A".repeat(64).into(),
                5 => p["replacements"][0]["old519"][1] = 9753095.into(),
                6 => p["replacements"][0]["old519"][3] = Value::Null,
                7 => p["replacements"][0]["old519"]
                    .as_array_mut()
                    .unwrap()
                    .push(true.into()),
                8 => p["replacements"][0] = full["replacements"][0].clone(),
                9 => p["schema"] = "semaprax.finite-source-overlay-page.v5".into(),
                10 => {
                    let index = b["rows"]
                        .as_array()
                        .unwrap()
                        .iter()
                        .position(|r| r["source"] == OLD519[0].0)
                        .unwrap();
                    b["rows"][index]["unknown"] = true.into();
                }
                11 => {
                    let index = b["rows"]
                        .as_array()
                        .unwrap()
                        .iter()
                        .position(|r| r["source"] == OLD519[0].0)
                        .unwrap();
                    b["rows"][index]["current"]["length"] = 0.into();
                }
                12 => {
                    let item = p["replacements"][0].clone();
                    p["additions"].as_array_mut().unwrap().push(item);
                }
                13 => p["replacements"][0]["old519"][0] = true.into(),
                _ => p["replacements"][0]["old519"][3][0]["archive"] = "../foreign".into(),
            }
            assert!(compose_values(b, &policy, &p).is_err(), "mode={mode}");
        }
        let mut p = full;
        p["replacements"][0]["unknown"] = true.into();
        assert!(compose_values_v2(before, &policy, &p).is_err());
    }
    fn v5_fixture() -> (Value, Value, Value, Value) {
        let (before, mut policy, mut full, mut compact) = v4_fixture();
        compact["schema"] = SCHEMA_V5.into();
        for (offset, (source, _)) in EXTRA519.iter().enumerate() {
            let prior = before["rows"]
                .as_array()
                .unwrap()
                .iter()
                .find(|r| r["source"] == *source)
                .unwrap();
            let hash = digest(format!("fixed-v5-current-{offset}").as_bytes());
            let mut row = prior.clone();
            row["current"] =
                serde_json::json!({"length":1,"sha256":hash,"archive":format!("e/{hash}")});
            let mut retained = prior["current"].clone();
            retained["name"] = "whole-pre-overlay-v2-current".into();
            row["views"].as_array_mut().unwrap().push(retained);
            let extra = serde_json::json!([{"name":"v5-extra","length":1,"sha256":hash,"archive":format!("e/{hash}"),"opaque":{"kept":[8,3,1]}}]);
            row["views"]
                .as_array_mut()
                .unwrap()
                .extend(extra.as_array().unwrap().iter().cloned());
            policy["replacements"]
                .as_array_mut()
                .unwrap()
                .push((*source).into());
            full["replacements"].as_array_mut().unwrap().push(row);
            compact["replacements"]
                .as_array_mut()
                .unwrap()
                .push(serde_json::json!({"old519":[offset + 2,1,hash,extra]}));
        }
        // Both protocol generations obey the same predecessor row order.
        let names = policy["replacements"].as_array().unwrap().clone();
        let mut order: Vec<_> = (0..names.len()).collect();
        order.sort_by_key(|&index| {
            before["rows"]
                .as_array()
                .unwrap()
                .iter()
                .position(|row| row["source"] == names[index])
                .unwrap()
        });
        policy["replacements"] = order
            .iter()
            .map(|&i| names[i].clone())
            .collect::<Vec<_>>()
            .into();
        for page in [&mut full, &mut compact] {
            let rows = page["replacements"].as_array().unwrap().clone();
            page["replacements"] = order
                .iter()
                .map(|&i| rows[i].clone())
                .collect::<Vec<_>>()
                .into();
        }
        (before, policy, full, compact)
    }
    #[test]
    fn v5_fixed_extra_rows_expand_exactly_and_keep_complete_v4_and_old_inverse() {
        let (before, policy, full, compact) = v5_fixture();
        assert_eq!(expand_v5(&before, &policy, &compact).unwrap(), full);
        let result = compose_values(before.clone(), &policy, &compact).unwrap();
        assert_eq!(
            result,
            compose_values_v2(before.clone(), &policy, &full).unwrap()
        );
        for (row, prior) in result["rows"]
            .as_array()
            .unwrap()
            .iter()
            .zip(before["rows"].as_array().unwrap())
        {
            assert_eq!(row["source"], prior["source"]);
            let mut restored = row.clone();
            restored["current"] = prior["current"].clone();
            restored["views"] = prior["views"].clone();
            assert_eq!(restored, *prior, "complete unknown fields and row order");
            let count = prior["views"].as_array().unwrap().len();
            assert_eq!(
                &row["views"].as_array().unwrap()[..count],
                prior["views"].as_array().unwrap()
            );
        }
        // V4 is still independently accepted with its original exact two rows.
        let (old, policy, whole, v4) = v4_fixture();
        assert_eq!(expand_v4(&old, &policy, &v4).unwrap(), whole);
    }
    #[test]
    fn v5_refuses_missing_foreign_duplicate_tampered_drift_and_unknown_version() {
        let (before, policy, full, compact) = v5_fixture();
        let at = compact["replacements"]
            .as_array()
            .unwrap()
            .iter()
            .position(|r| r["old519"][0] == 2)
            .unwrap();
        for mode in 0..15 {
            let mut b = before.clone();
            let mut p = compact.clone();
            match mode {
                0 => {
                    p["replacements"].as_array_mut().unwrap().remove(at);
                }
                1 => p["replacements"][at]["old519"][0] = 4.into(),
                2 => {
                    let item = p["replacements"][at].clone();
                    p["replacements"].as_array_mut().unwrap().push(item);
                }
                3 => p["replacements"][at]["extra"] = true.into(),
                4 => p["replacements"][at]["old519"][2] = "bad".into(),
                5 => p["replacements"][at]["old519"][1] = 9753095.into(),
                6 => p["replacements"][at]["old519"][3] = Value::Null,
                7 => p["replacements"][at]["old519"]
                    .as_array_mut()
                    .unwrap()
                    .push(true.into()),
                8 => p["schema"] = "semaprax.finite-source-overlay-page.v500".into(),
                9 | 10 => {
                    let row = b["rows"]
                        .as_array_mut()
                        .unwrap()
                        .iter_mut()
                        .find(|r| r["source"] == EXTRA519[0].0)
                        .unwrap();
                    if mode == 9 {
                        row["unknown"] = true.into();
                    } else {
                        row["current"]["length"] = 1.into();
                    }
                }
                11 => {
                    let item = p["replacements"][at].clone();
                    p["additions"].as_array_mut().unwrap().push(item);
                }
                12 => p["replacements"][at] = full["replacements"][at].clone(),
                13 => p["replacements"][at]["old519"][0] = true.into(),
                _ => {
                    let item = b["rows"]
                        .as_array()
                        .unwrap()
                        .iter()
                        .find(|r| r["source"] == EXTRA519[0].0)
                        .unwrap()
                        .clone();
                    b["rows"].as_array_mut().unwrap().push(item);
                }
            }
            assert!(compose_values(b, &policy, &p).is_err(), "mode={mode}");
        }
    }
    fn v6_fixture() -> (Value, Value, Value, Value) {
        let before = super::super::super::packet_before_overlay();
        let predecessor =
            serde_json::json!({"rows":519,"logical_sha256":digest(&canonical(&before))});
        let indices = [0usize, 518];
        let rows: Vec<_> = indices.iter().map(|&i| before["rows"][i].clone()).collect();
        let names: Vec<_> = rows.iter().map(|r| r["source"].clone()).collect();
        let full = serde_json::json!({"schema":"semaprax.finite-source-overlay-page.v2","index":1,
            "total_pages":1,"predecessor":predecessor,"replacements":rows,"additions":[]});
        let policy = serde_json::json!({"predecessor":predecessor,"replacements":names,
            "additions":[],"new_fragment_edges":[]});
        let mut compact = full.clone();
        compact["schema"] = SCHEMA_V6.into();
        compact["replacements"] = indices
            .iter()
            .map(|&i| serde_json::json!({"old519_index":[i,before["rows"][i]["current"],[],null]}))
            .collect::<Vec<_>>()
            .into();
        (before, policy, full, compact)
    }
    #[test]
    fn v6_general_indices_preserve_complete_boundary_rows_aliases_and_unknown_fields() {
        let (before, policy, full, compact) = v6_fixture();
        assert_eq!(expand_v6(&before, &policy, &compact).unwrap(), full);
        assert_eq!(
            compose_values(before.clone(), &policy, &compact).unwrap(),
            before
        );
        let mut unknown = before.clone();
        unknown["v6_opaque_outer"] = serde_json::json!({"keep":[3,1,8]});
        unknown["rows"][0]["v6_opaque_row"] = serde_json::json!({"nested":[true,null,7]});
        let mut p = compact;
        let mut rules = policy;
        rules["predecessor"]["logical_sha256"] = digest(&canonical(&unknown)).into();
        p["predecessor"] = rules["predecessor"].clone();
        assert_eq!(
            compose_values(unknown.clone(), &rules, &p).unwrap(),
            unknown
        );
        let mut alias = unknown["rows"][0]["current"].clone();
        alias["archive"] = "e/v6-existing-alias".into();
        alias["opaque_current"] = serde_json::json!({"kept":[4,2]});
        let mut historical = unknown["rows"][0]["current"].clone();
        historical["name"] = "whole-pre-overlay-v2-current".into();
        historical["opaque_view"] = serde_json::json!({"kept":[2,4]});
        // Keep the exact required old-current record independently of extra fields.
        let mut required = unknown["rows"][0]["current"].clone();
        required["name"] = "whole-pre-overlay-v2-current".into();
        p["replacements"][0]["old519_index"][1] = alias.clone();
        p["replacements"][0]["old519_index"][2] = serde_json::json!([required, historical]);
        let expanded = expand_v6(&unknown, &rules, &p).unwrap();
        assert_eq!(expanded["replacements"][0]["current"], alias);
        let result = compose_values(unknown.clone(), &rules, &p).unwrap();
        let mut restored = result.clone();
        restored["rows"][0]["current"] = unknown["rows"][0]["current"].clone();
        restored["rows"][0]["views"] = unknown["rows"][0]["views"].clone();
        assert_eq!(restored, unknown, "complete whole519 object inverse");
    }
    #[test]
    fn v6_fixed_root_overrides_keep_original_v3_fragment_authority() {
        let before = super::super::super::packet_before_overlay();
        let index = before["rows"]
            .as_array()
            .unwrap()
            .iter()
            .position(|r| r["source"] == ROOTS[2])
            .unwrap();
        let prior = &before["rows"][index];
        let fragments = prior["fragments"].as_array().unwrap();
        assert!(!fragments.is_empty());
        let refs: Vec<_> = fragments
            .iter()
            .map(|r| serde_json::json!({"canonical_source":r["source"]}))
            .collect();
        let predecessor =
            serde_json::json!({"rows":519,"logical_sha256":digest(&canonical(&before))});
        let policy = serde_json::json!({"predecessor":predecessor,"replacements":[ROOTS[2]],"additions":[],"new_fragment_edges":[]});
        let page = serde_json::json!({"schema":SCHEMA_V6,"index":1,"total_pages":1,"predecessor":predecessor,
            "replacements":[{"old519_index":[index,prior["current"],[],refs]}],"additions":[]});
        assert_eq!(
            compose_values(before.clone(), &policy, &page).unwrap(),
            before
        );
        for mode in 0..5 {
            let mut p = page.clone();
            match mode {
                0 => {
                    p["replacements"][0]["old519_index"][3][0]["canonical_source"] =
                        "../foreign.rs".into()
                }
                1 => {
                    let item = p["replacements"][0]["old519_index"][3][0].clone();
                    p["replacements"][0]["old519_index"][3]
                        .as_array_mut()
                        .unwrap()
                        .push(item);
                }
                2 => p["replacements"][0]["old519_index"][3][0]["extra"] = true.into(),
                3 => {
                    p["replacements"][0]["old519_index"][3][0]["canonical_source"] = ROOTS[2].into()
                }
                _ => p["replacements"][0]["old519_index"][3] = serde_json::json!([]),
            }
            assert!(
                compose_values(before.clone(), &policy, &p).is_err(),
                "fragment mode={mode}"
            );
        }
        let (old, rules, _, mut p) = v6_fixture();
        p["replacements"][0]["old519_index"][3] = serde_json::json!([]);
        assert!(
            compose_values(old, &rules, &p).is_err(),
            "non-root fragment override"
        );
    }
    #[test]
    fn v6_refuses_foreign_indices_missing_rows_duplicates_and_changed_predecessors() {
        let (before, policy, _, compact) = v6_fixture();
        for mode in 0..25 {
            let mut b = before.clone();
            let mut rules = policy.clone();
            let mut p = compact.clone();
            match mode {
                0 => {
                    p["replacements"].as_array_mut().unwrap().pop();
                }
                1 => p["replacements"][1]["old519_index"][0] = 0.into(),
                2 => p["replacements"][0]["old519_index"][0] = 519.into(),
                3 => p["replacements"][0]["old519_index"][0] = (-1).into(),
                4 => p["replacements"][0]["old519_index"][0] = true.into(),
                5 => p["replacements"][0]["old519_index"][0] = serde_json::json!(0.0),
                6 => p["replacements"][0]["old519_index"][0] = u64::MAX.into(),
                7 => p["replacements"][0]["old519_index"] = serde_json::json!([]),
                8 => p["replacements"][0]["old519_index"]
                    .as_array_mut()
                    .unwrap()
                    .push(true.into()),
                9 => p["replacements"][0]["source"] = "foreign.rs".into(),
                10 => p["replacements"][0]["old519_index"][1] = Value::Null,
                11 => p["replacements"][0]["old519_index"][1]["length"] = 9753095.into(),
                12 => p["replacements"][0]["old519_index"][1]["sha256"] = "bad".into(),
                13 => p["replacements"][0]["old519_index"][1]["archive"] = "../foreign".into(),
                14 => p["replacements"][0]["old519_index"][2] = Value::Null,
                15 => p["replacements"][0]["old519_index"][3] = true.into(),
                16 => rules["replacements"][0] = "foreign.rs".into(),
                17 => b["rows"][0]["foreign"] = true.into(),
                18 => {
                    b["rows"].as_array_mut().unwrap().pop();
                }
                19 => {
                    let item = p["replacements"][0].clone();
                    p["additions"].as_array_mut().unwrap().push(item);
                }
                20 => p["replacements"][0] = serde_json::json!({"old519":[0,1,"a".repeat(64),[]]}),
                21 => p["schema"] = "semaprax.finite-source-overlay-page.v600".into(),
                22 => p["replacements"][0]["old519_index"][1]["sha256"] = "a".repeat(64).into(),
                23 => rules["predecessor"]["rows"] = true.into(),
                _ => b["source_cap"] = 9753095.into(),
            }
            assert!(compose_values(b, &rules, &p).is_err(), "mode={mode}");
        }
    }

    fn v7_fixture() -> (Value, Value, Value, Value, usize) {
        let mut before = super::super::super::packet_before_overlay();
        let index = before["rows"]
            .as_array()
            .unwrap()
            .iter()
            .position(|row| row["source"] == ROOTS[2])
            .unwrap();
        before["opaque_v7_outer"] = serde_json::json!({"kept":[8,3,1]});
        before["rows"][index]["opaque_v7_row"] = serde_json::json!({"kept":[true,null]});
        let prior = &before["rows"][index];
        let new = vec![record("src/v7_numeric_a.rs"), record("src/v7_numeric_b.rs")];
        let mut root = prior.clone();
        root["fragments"]
            .as_array_mut()
            .unwrap()
            .extend(new.iter().cloned());
        let predecessor =
            serde_json::json!({"rows":519,"logical_sha256":digest(&canonical(&before))});
        let full = serde_json::json!({"schema":"semaprax.finite-source-overlay-page.v2","index":1,
            "total_pages":1,"predecessor":predecessor,"replacements":[root],"additions":new});
        let policy = serde_json::json!({"predecessor":predecessor,"replacements":[ROOTS[2]],
            "additions":[{"source":"src/v7_numeric_a.rs","compiled":true},{"source":"src/v7_numeric_b.rs","compiled":true}],
            "new_fragment_edges":[{"parent":ROOTS[2],"child":"src/v7_numeric_a.rs"},{"parent":ROOTS[2],"child":"src/v7_numeric_b.rs"}]});
        let mut refs: Vec<_> = prior["fragments"]
            .as_array()
            .unwrap()
            .iter()
            .map(|row| serde_json::json!({"canonical_source":row["source"]}))
            .collect();
        refs.extend([
            serde_json::json!({"canonical_source":519}),
            serde_json::json!({"canonical_source":520}),
        ]);
        let mut compact = full.clone();
        compact["schema"] = SCHEMA_V7.into();
        compact["replacements"] =
            serde_json::json!([{"old519_index":[index,prior["current"],[],refs]}]);
        (before, policy, full, compact, index)
    }
    #[test]
    fn v7_numeric_addition_refs_keep_full_rows_unknowns_old_strings_and_complete_inverse() {
        v8_complete_carriers_and_legacy_boundaries();
        let (before, policy, full, compact, index) = v7_fixture();
        assert_eq!(expand_v7(&before, &policy, &compact).unwrap(), full);
        let result = compose_values(before.clone(), &policy, &compact).unwrap();
        assert_eq!(
            result,
            compose_values_v2(before.clone(), &policy, &full).unwrap()
        );
        assert_eq!(result["opaque_v7_outer"], before["opaque_v7_outer"]);
        assert_eq!(
            result["rows"][index]["opaque_v7_row"],
            before["rows"][index]["opaque_v7_row"]
        );
        assert_eq!(
            &result["rows"].as_array().unwrap()[519..],
            compact["additions"].as_array().unwrap()
        );
        let mut restored = result;
        restored["rows"][index] = before["rows"][index].clone();
        restored["rows"].as_array_mut().unwrap().truncate(519);
        assert_eq!(restored, before, "whole predecessor object, not row counts");
        let mut legacy = compact.clone();
        legacy["schema"] = SCHEMA_V6.into();
        let refs = legacy["replacements"][0]["old519_index"][3]
            .as_array_mut()
            .unwrap();
        let n = refs.len();
        refs[n - 2][REFERENCE] = "src/v7_numeric_a.rs".into();
        refs[n - 1][REFERENCE] = "src/v7_numeric_b.rs".into();
        assert_eq!(expand_v6(&before, &policy, &legacy).unwrap(), full);
        let mut direct = compact.clone();
        direct["replacements"][0] = full["replacements"][0].clone();
        direct["replacements"][0]["fragments"] =
            compact["replacements"][0]["old519_index"][3].clone();
        assert_eq!(expand_v7(&before, &policy, &direct).unwrap(), full);
    }
    #[test]
    fn v7_numeric_refs_refuse_types_old_foreign_duplicate_reordered_and_uncompiled_targets() {
        v8_finite_carrier_hostiles();
        let (before, policy, _, compact, _) = v7_fixture();
        let n = compact["replacements"][0]["old519_index"][3]
            .as_array()
            .unwrap()
            .len();
        for mode in 0..18 {
            let mut old = before.clone();
            let mut rules = policy.clone();
            let mut page = compact.clone();
            match mode {
                0 => page["replacements"][0]["old519_index"][3][n - 1][REFERENCE] = true.into(),
                1 => page["replacements"][0]["old519_index"][3][n - 1][REFERENCE] = (-1).into(),
                2 => page["replacements"][0]["old519_index"][3][n - 1][REFERENCE] = 519.0.into(),
                3 => page["replacements"][0]["old519_index"][3][n - 1][REFERENCE] = 0.into(),
                4 => page["replacements"][0]["old519_index"][3][n - 1][REFERENCE] = 518.into(),
                5 => page["replacements"][0]["old519_index"][3][n - 1][REFERENCE] = 521.into(),
                6 => page["replacements"][0]["old519_index"][3][n - 1][REFERENCE] = u64::MAX.into(),
                7 => page["replacements"][0]["old519_index"][3][n - 1]["extra"] = true.into(),
                8 => page["replacements"][0]["old519_index"][3][n - 1][REFERENCE] = 519.into(),
                9 => page["replacements"][0]["old519_index"][3]
                    .as_array_mut()
                    .unwrap()
                    .swap(n - 2, n - 1),
                10 => page["additions"][0]["compiled"] = false.into(),
                11 => page["additions"][0]["fragments"] = serde_json::json!([]),
                12 => rules["predecessor"]["logical_sha256"] = "0".repeat(64).into(),
                13 => page["additions"][1]["source"] = "src/v7_numeric_a.rs".into(),
                14 => {
                    page["additions"].as_array_mut().unwrap().pop();
                }
                15 => rules["new_fragment_edges"]
                    .as_array_mut()
                    .unwrap()
                    .swap(0, 1),
                16 => old["source_cap"] = (super::super::SOURCE_CAP + 1).into(),
                _ => {
                    let nonroot = old["rows"]
                        .as_array()
                        .unwrap()
                        .iter()
                        .position(|row| !ROOTS.iter().any(|root| row["source"] == *root))
                        .unwrap();
                    page["replacements"][0]["old519_index"][0] = nonroot.into();
                }
            }
            assert!(compose_values(old, &rules, &page).is_err(), "mode={mode}");
        }
    }
    fn v8_fixture() -> (Value, Value, Value, Value) {
        let before = super::super::super::packet_before_overlay();
        let predecessor =
            serde_json::json!({"rows":519,"logical_sha256":digest(&canonical(&before))});
        let mut rows = Vec::new();
        let mut compact_rows = Vec::new();
        for &(index, source, _, child_index, _) in V8_CARRIERS {
            let prior = &before["rows"][index];
            let child_prior = &before["rows"][child_index];
            let pin = |label: &str| {
                let h = digest(label.as_bytes());
                serde_json::json!({"length":7,"sha256":h,"archive":format!("e/{h}")})
            };
            let current = pin(source);
            let child_current = pin(child_prior["source"].as_str().unwrap());
            let child_view = serde_json::json!({"name":"v8-child-held","length":3,"sha256":"b".repeat(64),"archive":"e/v8-child-held"});
            let mut retained = child_prior["current"].clone();
            retained["name"] = "whole-pre-overlay-v2-current".into();
            let child_views = serde_json::json!([retained, child_view]);
            let mut child = child_prior.clone();
            child["current"] = child_current.clone();
            child["views"]
                .as_array_mut()
                .unwrap()
                .extend(child_views.as_array().unwrap().iter().cloned());
            let mut carrier = prior.clone();
            carrier["current"] = current.clone();
            let mut carrier_retained = prior["current"].clone();
            carrier_retained["name"] = "whole-pre-overlay-v2-current".into();
            carrier["views"]
                .as_array_mut()
                .unwrap()
                .push(carrier_retained.clone());
            carrier["fragments"] = serde_json::json!([child]);
            rows.extend([carrier, child]);
            compact_rows.extend([
                serde_json::json!({"old519_index":[index,current,[carrier_retained],[{"canonical_source":child_prior["source"]}]]}),
                serde_json::json!({"old519_index":[child_index,child_current,child_views,null]}),
            ]);
        }
        rows.sort_by_key(|row| {
            before["rows"]
                .as_array()
                .unwrap()
                .iter()
                .position(|r| r["source"] == row["source"])
                .unwrap()
        });
        compact_rows.sort_by_key(|row| row["old519_index"][0].as_u64().unwrap());
        let policy = serde_json::json!({"predecessor":predecessor,
            "replacements":rows.iter().map(|r|r["source"].clone()).collect::<Vec<_>>(),
            "additions":[],"new_fragment_edges":[]});
        let full = serde_json::json!({"schema":"semaprax.finite-source-overlay-page.v2",
            "index":1,"total_pages":1,"predecessor":predecessor,"replacements":rows,"additions":[]});
        let mut compact = full.clone();
        compact["schema"] = SCHEMA_V8.into();
        compact["replacements"] = compact_rows.into();
        (before, policy, full, compact)
    }
    fn v8_complete_carriers_and_legacy_boundaries() {
        let (before, policy, full, compact) = v8_fixture();
        assert_eq!(expand_v8(&before, &policy, &compact).unwrap(), full);
        let result = compose_values(before.clone(), &policy, &compact).unwrap();
        assert_eq!(
            result,
            compose_values_v2(before.clone(), &policy, &full).unwrap()
        );
        let mut restored = result;
        for &(i, _, _, child, _) in V8_CARRIERS {
            restored["rows"][i] = before["rows"][i].clone();
            restored["rows"][child] = before["rows"][child].clone();
        }
        assert_eq!(restored, before, "ENTIRE predecessor and every old field");
        for schema in [SCHEMA_V7, SCHEMA_V6, SCHEMA] {
            let mut legacy = compact.clone();
            legacy["schema"] = schema.into();
            assert!(compose_values(before.clone(), &policy, &legacy).is_err());
        }
    }
    fn v8_finite_carrier_hostiles() {
        let (before, policy, _, compact) = v8_fixture();
        let mut missing = compact.clone();
        missing["replacements"].as_array_mut().unwrap().pop();
        assert!(expand_v8(&before, &policy, &missing).is_err());
        for mode in 0..21 {
            let mut b = before.clone();
            let mut rules = policy.clone();
            let mut page = compact.clone();
            match mode {
                0 => page["replacements"][0]["old519_index"][0] = true.into(),
                1 => page["replacements"][0]["old519_index"][0] = 39.into(),
                2 => page["replacements"][0]["old519_index"][3][0][REFERENCE] = 366.into(),
                3 => {
                    page["replacements"][0]["old519_index"][3][0][REFERENCE] =
                        "src/hir/resolve_program_methods.rs".into()
                }
                4 => page["replacements"][0]["old519_index"][3][0]["unknown"] = true.into(),
                5 => page["replacements"][0]["old519_index"][3] = serde_json::json!([]),
                6 => {
                    let x = page["replacements"][0]["old519_index"][3][0].clone();
                    page["replacements"][0]["old519_index"][3]
                        .as_array_mut()
                        .unwrap()
                        .push(x);
                }
                7 => page["replacements"][0]["unknown"] = true.into(),
                8 => page["replacements"][3]["old519_index"][3] = serde_json::json!([]),
                9 => {
                    let mut child = b["rows"][366].clone();
                    child["compiled"] = false.into();
                    page["replacements"][3] = child;
                }
                10 => {
                    let mut child = b["rows"][366].clone();
                    child["fragments"] = serde_json::json!([]);
                    page["replacements"][3] = child;
                }
                11 => {
                    let child = page["replacements"][3].clone();
                    page["replacements"].as_array_mut().unwrap().push(child);
                }
                12 => b["rows"][38]["unknown"] = true.into(),
                13 => b["rows"][366]["source"] = "src/foreign.rs".into(),
                14 => rules["predecessor"]["logical_sha256"] = "0".repeat(64).into(),
                15 => page["replacements"][0]["old519_index"][1]["archive"] = "../foreign".into(),
                16 => page["replacements"][0]["old519_index"][2] = true.into(),
                17 => {
                    let row = page["replacements"][0].clone();
                    page["additions"].as_array_mut().unwrap().push(row);
                }
                18 => page["schema"] = "semaprax.finite-source-overlay-page.v800".into(),
                19 => {
                    page["replacements"].as_array_mut().unwrap().pop();
                }
                _ => page["replacements"].as_array_mut().unwrap().swap(0, 1),
            }
            assert!(compose_values(b, &rules, &page).is_err(), "v8 mode={mode}");
        }
    }
}
