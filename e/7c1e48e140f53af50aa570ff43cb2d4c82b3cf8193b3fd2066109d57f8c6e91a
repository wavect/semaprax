//! V13 interns repeated positional digests; its complete inverse is logical V12.
use super::super::{canonical, SOURCE_CAP};
use super::labels_v12::SCHEMA_V12;
use serde_json::Value;
use std::collections::{BTreeMap, BTreeSet};

pub(in super::super) const SCHEMA_V13: &str = "semaprax.finite-source-overlay-page.v13";
const PAGE_CAP: usize = 262144;
const KEYS: [&str; 8] = [
    "schema",
    "index",
    "total_pages",
    "predecessor",
    "replacements",
    "additions",
    "archive_prefixes",
    "view_labels",
];

fn envelope(page: &Value, schema: &str, table: bool) -> Result<(), &'static str> {
    let object = page.as_object().ok_or("v13 envelope")?;
    if page["schema"] != schema
        || object.len() != KEYS.len() + usize::from(table)
        || !KEYS.iter().all(|key| object.contains_key(*key))
        || (table && !object.contains_key("digest_table"))
    {
        return Err("v13 envelope");
    }
    Ok(())
}

fn digest(value: &Value) -> Result<&str, &'static str> {
    const ALPHABET: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-_";
    let text = value.as_str().ok_or("v13 digest type")?;
    if text.len() != 43 || !text.bytes().all(|byte| ALPHABET.contains(&byte)) {
        return Err("v13 digest spelling");
    }
    // 32 bytes leave two zero pad bits in the last unpadded base64 digit.
    let last = ALPHABET
        .iter()
        .position(|byte| *byte == text.as_bytes()[42])
        .unwrap();
    if last & 3 != 0 {
        return Err("v13 digest spelling");
    }
    Ok(text)
}

type Convert<'a> = dyn FnMut(&mut Value) -> Result<(), &'static str> + 'a;
fn pin(value: &mut Value, named: bool, convert: &mut Convert<'_>) -> Result<(), &'static str> {
    // Object pins retain all fields, including unknown metadata, without walking them.
    if value.is_object() {
        return Ok(());
    }
    let tuple = value.as_array_mut().ok_or("v13 pin arity")?;
    let offset = usize::from(named);
    if tuple.len() != offset + 2 && tuple.len() != offset + 3 {
        return Err("v13 pin arity");
    }
    let length = tuple[offset].as_u64().ok_or("v13 pin length")?;
    if length > SOURCE_CAP as u64 {
        return Err("v13 pin length");
    }
    convert(&mut tuple[offset + 1])
}
fn row(value: &mut Value, convert: &mut Convert<'_>) -> Result<(), &'static str> {
    let object = value.as_object_mut().ok_or("v13 literal row")?;
    if let Some(current) = object.get_mut("current") {
        pin(current, false, convert)?;
    }
    if let Some(views) = object.get_mut("views") {
        for view in views.as_array_mut().ok_or("v13 views")? {
            pin(view, true, convert)?;
        }
    }
    Ok(())
}
fn fragments(value: &mut Value, convert: &mut Convert<'_>) -> Result<(), &'static str> {
    if value.is_null() {
        return Ok(());
    }
    for fragment in value.as_array_mut().ok_or("v13 fragments")? {
        if !fragment.is_object() {
            return Err("v13 fragment row");
        }
        if fragment.get(super::REFERENCE).is_none() {
            row(fragment, convert)?;
        }
    }
    Ok(())
}
fn digest_slots(page: &mut Value, convert: &mut Convert<'_>) -> Result<(), &'static str> {
    for group in ["replacements", "additions"] {
        for value in page[group].as_array_mut().ok_or("v13 rows")? {
            let object = value.as_object_mut().ok_or("v13 row")?;
            if object.contains_key("old519_index") {
                if group != "replacements" || object.len() != 1 {
                    return Err("v13 indexed row");
                }
                let tuple = object
                    .get_mut("old519_index")
                    .unwrap()
                    .as_array_mut()
                    .ok_or("v13 old519 arity")?;
                if tuple.len() != 4 {
                    return Err("v13 old519 arity");
                }
                pin(&mut tuple[1], false, convert)?;
                for view in tuple[2].as_array_mut().ok_or("v13 views")? {
                    pin(view, true, convert)?;
                }
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
    let index = usize::try_from(value.as_u64().ok_or("v13 unknown digest index")?)
        .map_err(|_| "v13 unknown digest index")?;
    if index >= count {
        return Err("v13 unknown digest index");
    }
    Ok(index)
}

fn expand_to_v12(page: &Value) -> Result<Value, &'static str> {
    envelope(page, SCHEMA_V13, true)?;
    if canonical(page).len() > PAGE_CAP {
        return Err("v13 same one-page cap");
    }
    let declarations = page["digest_table"].as_array().ok_or("v13 dictionary")?;
    let mut digests = Vec::new();
    let mut unique = BTreeSet::new();
    for declaration in declarations {
        let text = digest(declaration)?;
        if !unique.insert(text) {
            return Err("v13 duplicate declaration");
        }
        digests.push(text);
    }
    let mut result = page.clone();
    result.as_object_mut().unwrap().remove("digest_table");
    result["schema"] = SCHEMA_V12.into();
    let mut expanded = canonical(&result).len();
    let mut uses = vec![0usize; digests.len()];
    let mut next = 0usize;
    let mut literals = BTreeSet::new();
    // Validate every declaration/use and the complete expansion length before
    // publishing any dictionary string. Only exact positional slots are visited.
    digest_slots(&mut result, &mut |value| {
        if value.is_string() {
            let text = digest(value)?;
            if unique.contains(text) || !literals.insert(text.to_owned()) {
                return Err("v13 uninterned or aliased digest");
            }
        } else {
            let index = index(value, digests.len())?;
            if uses[index] == 0 {
                if index != next {
                    return Err("v13 noncanonical first-use order");
                }
                next = next.checked_add(1).ok_or("v13 index overflow")?;
            }
            uses[index] = uses[index].checked_add(1).ok_or("v13 count overflow")?;
            let number_length = canonical(value).len() - 1;
            expanded = expanded
                .checked_add(45 - number_length)
                .ok_or("v13 expanded bound")?;
        }
        if expanded > SOURCE_CAP {
            return Err("v13 expanded bound");
        }
        Ok(())
    })?;
    if uses.iter().any(|count| *count < 2) {
        return Err("v13 declaration requires repeated use");
    }
    digest_slots(&mut result, &mut |value| {
        if !value.is_string() {
            *value = digests[index(value, digests.len())?].into();
        }
        Ok(())
    })?;
    if canonical(&result).len() != expanded || expanded > SOURCE_CAP {
        return Err("v13 expanded bound");
    }
    Ok(result)
}

// Explicit opt-in encoder: the public V12 route and every older encoder retain
// their exact bodies and caps. This input is an unpublished logical V12 value.
#[cfg_attr(not(test), allow(dead_code))]
fn encode_v13(page: &Value) -> Result<Value, &'static str> {
    envelope(page, SCHEMA_V12, false)?;
    if canonical(page).len() > SOURCE_CAP {
        return Err("v13 logical input bound");
    }
    let mut result = page.clone();
    let mut counts = BTreeMap::<String, usize>::new();
    digest_slots(&mut result, &mut |value| {
        let count = counts.entry(digest(value)?.to_owned()).or_default();
        *count = count.checked_add(1).ok_or("v13 count overflow")?;
        Ok(())
    })?;
    let mut indices = BTreeMap::new();
    let mut table = Vec::new();
    digest_slots(&mut result, &mut |value| {
        let text = digest(value)?;
        if counts[text] >= 2 {
            let index = if let Some(index) = indices.get(text) {
                *index
            } else {
                let index = table.len();
                indices.insert(text.to_owned(), index);
                table.push(Value::from(text));
                index
            };
            *value = index.into();
        }
        Ok(())
    })?;
    result["schema"] = SCHEMA_V13.into();
    result["digest_table"] = table.into();
    if canonical(&result).len() > PAGE_CAP {
        return Err("v13 same one-page cap");
    }
    if expand_to_v12(&result)? != *page {
        return Err("v13 ENTIRE V12 page inverse");
    }
    Ok(result)
}

// The complete prior V12 implementation remains separately compiled. Only its
// explicitly internal input frontier, imports and test-module glue are adapted;
// the exact whole-source inverse is executable in this module's owning tests.
#[path = "main_e177_overlay_digests_v13/logical_v12.rs"]
#[allow(dead_code)]
mod logical_v12;
pub(in super::super) fn expand_v13(
    before: &Value,
    policy: &Value,
    page: &Value,
) -> Result<Value, &'static str> {
    let logical = expand_to_v12(page)?;
    super::expand_v11(before, policy, &logical_v12::expand_to_v11(&logical)?)
}

#[cfg(test)]
#[path = "main_e177_overlay_digests_v13/tests.rs"]
mod tests;
#[cfg(test)]
pub(in super::super) use tests::{before_source, verify_current_v13, verify_current_v14};

// Private V14 intermediate only; standalone V13 entry points above are unchanged.
fn expand_to_v12_logical(page: &Value) -> Result<Value, &'static str> {
    envelope(page, SCHEMA_V13, true)?;
    if canonical(page).len() > SOURCE_CAP {
        return Err("v13 same one-page cap");
    }
    let declarations = page["digest_table"].as_array().ok_or("v13 dictionary")?;
    let mut digests = Vec::new();
    let mut unique = BTreeSet::new();
    for declaration in declarations {
        let text = digest(declaration)?;
        if !unique.insert(text) {
            return Err("v13 duplicate declaration");
        }
        digests.push(text);
    }
    let mut result = page.clone();
    result.as_object_mut().unwrap().remove("digest_table");
    result["schema"] = SCHEMA_V12.into();
    let mut expanded = canonical(&result).len();
    let mut uses = vec![0usize; digests.len()];
    let mut next = 0usize;
    let mut literals = BTreeSet::new();
    // Validate every declaration/use and the complete expansion length before
    // publishing any dictionary string. Only exact positional slots are visited.
    digest_slots(&mut result, &mut |value| {
        if value.is_string() {
            let text = digest(value)?;
            if unique.contains(text) || !literals.insert(text.to_owned()) {
                return Err("v13 uninterned or aliased digest");
            }
        } else {
            let index = index(value, digests.len())?;
            if uses[index] == 0 {
                if index != next {
                    return Err("v13 noncanonical first-use order");
                }
                next = next.checked_add(1).ok_or("v13 index overflow")?;
            }
            uses[index] = uses[index].checked_add(1).ok_or("v13 count overflow")?;
            let number_length = canonical(value).len() - 1;
            expanded = expanded
                .checked_add(45 - number_length)
                .ok_or("v13 expanded bound")?;
        }
        if expanded > SOURCE_CAP {
            return Err("v13 expanded bound");
        }
        Ok(())
    })?;
    if uses.iter().any(|count| *count < 2) {
        return Err("v13 declaration requires repeated use");
    }
    digest_slots(&mut result, &mut |value| {
        if !value.is_string() {
            *value = digests[index(value, digests.len())?].into();
        }
        Ok(())
    })?;
    if canonical(&result).len() != expanded || expanded > SOURCE_CAP {
        return Err("v13 expanded bound");
    }
    Ok(result)
}

fn encode_v13_logical(page: &Value) -> Result<Value, &'static str> {
    envelope(page, SCHEMA_V12, false)?;
    if canonical(page).len() > SOURCE_CAP {
        return Err("v13 logical input bound");
    }
    let mut result = page.clone();
    let mut counts = BTreeMap::<String, usize>::new();
    digest_slots(&mut result, &mut |value| {
        let count = counts.entry(digest(value)?.to_owned()).or_default();
        *count = count.checked_add(1).ok_or("v13 count overflow")?;
        Ok(())
    })?;
    let mut indices = BTreeMap::new();
    let mut table = Vec::new();
    digest_slots(&mut result, &mut |value| {
        let text = digest(value)?;
        if counts[text] >= 2 {
            let index = if let Some(index) = indices.get(text) {
                *index
            } else {
                let index = table.len();
                indices.insert(text.to_owned(), index);
                table.push(Value::from(text));
                index
            };
            *value = index.into();
        }
        Ok(())
    })?;
    result["schema"] = SCHEMA_V13.into();
    result["digest_table"] = table.into();
    if canonical(&result).len() > SOURCE_CAP {
        return Err("v13 same one-page cap");
    }
    if expand_to_v12_logical(&result)? != *page {
        return Err("v13 ENTIRE V12 page inverse");
    }
    Ok(result)
}

// V14 visits the same closed positional pin domain, replacing only complete
// tails. Literal object pins (including unknown fields) remain opaque.
pub(in super::super) const SCHEMA_V14: &str = "semaprax.finite-source-overlay-page.v14";
type TailConvert<'a> = dyn FnMut(Vec<Value>) -> Result<Vec<Value>, &'static str> + 'a;
fn tail_pin(
    value: &mut Value,
    named: bool,
    convert: &mut TailConvert<'_>,
) -> Result<(), &'static str> {
    if value.is_object() {
        return Ok(());
    }
    let tuple = value.as_array_mut().ok_or("v14 pin arity")?;
    let offset = usize::from(named);
    if !(offset + 1..=offset + 3).contains(&tuple.len()) {
        return Err("v14 pin arity");
    }
    let tail = tuple.split_off(offset);
    tuple.extend(convert(tail)?);
    Ok(())
}
fn tail_row(value: &mut Value, convert: &mut TailConvert<'_>) -> Result<(), &'static str> {
    let object = value.as_object_mut().ok_or("v14 literal row")?;
    if let Some(current) = object.get_mut("current") {
        tail_pin(current, false, convert)?;
    }
    if let Some(views) = object.get_mut("views") {
        for view in views.as_array_mut().ok_or("v14 views")? {
            tail_pin(view, true, convert)?;
        }
    }
    Ok(())
}
fn tail_fragments(value: &mut Value, convert: &mut TailConvert<'_>) -> Result<(), &'static str> {
    if value.is_null() {
        return Ok(());
    }
    for fragment in value.as_array_mut().ok_or("v14 fragments")? {
        if !fragment.is_object() {
            return Err("v14 fragment row");
        }
        if fragment.get(super::REFERENCE).is_none() {
            tail_row(fragment, convert)?;
        }
    }
    Ok(())
}
fn tail_slots(page: &mut Value, convert: &mut TailConvert<'_>) -> Result<(), &'static str> {
    for group in ["replacements", "additions"] {
        for value in page[group].as_array_mut().ok_or("v14 rows")? {
            let object = value.as_object_mut().ok_or("v14 row")?;
            if object.contains_key("old519_index") {
                if group != "replacements" || object.len() != 1 {
                    return Err("v14 indexed row");
                }
                let tuple = object
                    .get_mut("old519_index")
                    .unwrap()
                    .as_array_mut()
                    .ok_or("v14 old519 arity")?;
                if tuple.len() != 4 {
                    return Err("v14 old519 arity");
                }
                tail_pin(&mut tuple[1], false, convert)?;
                for view in tuple[2].as_array_mut().ok_or("v14 views")? {
                    tail_pin(view, true, convert)?;
                }
                tail_fragments(&mut tuple[3], convert)?;
            } else {
                tail_row(value, convert)?;
                if let Some(value) = value.get_mut("fragments") {
                    tail_fragments(value, convert)?;
                }
            }
        }
    }
    Ok(())
}
fn tail_key(tail: &[Value]) -> Result<Vec<u8>, &'static str> {
    if tail.len() != 2 && tail.len() != 3 {
        return Err("v14 tail arity");
    }
    if tail[0]
        .as_u64()
        .filter(|length| *length <= SOURCE_CAP as u64)
        .is_none()
    {
        return Err("v14 tail length");
    }
    if !tail[1].is_string() && tail[1].as_u64().is_none() {
        return Err("v14 tail digest type");
    }
    Ok(canonical(&Value::Array(tail.to_vec())))
}
fn expand_to_v13(page: &Value) -> Result<Value, &'static str> {
    let object = page.as_object().ok_or("v14 envelope")?;
    if page["schema"] != SCHEMA_V14
        || object.len() != KEYS.len() + 2
        || !KEYS.iter().all(|key| object.contains_key(*key))
        || !object.contains_key("digest_table")
        || !object.contains_key("pin_tails")
    {
        return Err("v14 envelope");
    }
    if canonical(page).len() > PAGE_CAP {
        return Err("v14 same one-page cap");
    }
    let declarations = page["pin_tails"].as_array().ok_or("v14 dictionary")?;
    let mut unique = BTreeSet::new();
    for value in declarations {
        if !unique.insert(tail_key(value.as_array().ok_or("v14 tail arity")?)?) {
            return Err("v14 duplicate declaration");
        }
    }
    let mut result = page.clone();
    result.as_object_mut().unwrap().remove("pin_tails");
    result["schema"] = SCHEMA_V13.into();
    let mut expanded = canonical(&result).len();
    let mut uses = vec![0usize; declarations.len()];
    let mut next = 0usize;
    let mut literals = BTreeSet::new();
    tail_slots(&mut result, &mut |tail| {
        if tail.len() == 1 {
            let position = usize::try_from(tail[0].as_u64().ok_or("v14 unknown tail index")?)
                .map_err(|_| "v14 unknown tail index")?;
            let declaration = declarations.get(position).ok_or("v14 unknown tail index")?;
            if uses[position] == 0 {
                if position != next {
                    return Err("v14 noncanonical first-use order");
                }
                next = next.checked_add(1).ok_or("v14 index overflow")?;
            }
            uses[position] = uses[position].checked_add(1).ok_or("v14 count overflow")?;
            expanded = expanded
                .checked_sub(canonical(&Value::Array(tail.clone())).len())
                .and_then(|n| n.checked_add(canonical(declaration).len()))
                .ok_or("v14 expanded bound")?;
        } else {
            let key = tail_key(&tail)?;
            if unique.contains(&key) || !literals.insert(key) {
                return Err("v14 uninterned or aliased tail");
            }
        }
        if expanded > SOURCE_CAP {
            return Err("v14 expanded bound");
        }
        Ok(tail)
    })?;
    if uses.iter().any(|count| *count < 2) {
        return Err("v14 declaration requires repeated use");
    }
    if expanded > SOURCE_CAP {
        return Err("v14 expanded bound");
    }
    tail_slots(&mut result, &mut |tail| {
        if tail.len() == 1 {
            let position = usize::try_from(tail[0].as_u64().ok_or("v14 unknown tail index")?)
                .map_err(|_| "v14 unknown tail index")?;
            Ok(declarations
                .get(position)
                .and_then(Value::as_array)
                .ok_or("v14 unknown tail index")?
                .clone())
        } else {
            Ok(tail)
        }
    })?;
    if canonical(&result).len() != expanded {
        return Err("v14 expanded bound");
    }
    expand_to_v12_logical(&result)?;
    Ok(result)
}
#[cfg_attr(not(test), allow(dead_code))]
fn encode_v14_tails(page: &Value) -> Result<Value, &'static str> {
    envelope(page, SCHEMA_V13, true)?;
    if canonical(page).len() > SOURCE_CAP {
        return Err("v14 logical input bound");
    }
    expand_to_v12_logical(page)?;
    let mut result = page.clone();
    let mut counts = BTreeMap::<Vec<u8>, usize>::new();
    tail_slots(&mut result, &mut |tail| {
        let count = counts.entry(tail_key(&tail)?).or_default();
        *count = count.checked_add(1).ok_or("v14 count overflow")?;
        Ok(tail)
    })?;
    let mut indices = BTreeMap::new();
    let mut table = Vec::new();
    tail_slots(&mut result, &mut |tail| {
        let key = tail_key(&tail)?;
        if counts[&key] < 2 {
            return Ok(tail);
        }
        let index = if let Some(index) = indices.get(&key) {
            *index
        } else {
            let index = table.len();
            indices.insert(key, index);
            table.push(Value::Array(tail));
            index
        };
        Ok(vec![Value::from(index)])
    })?;
    result["schema"] = SCHEMA_V14.into();
    result["pin_tails"] = table.into();
    if canonical(&result).len() > PAGE_CAP {
        return Err("v14 same one-page cap");
    }
    if expand_to_v13(&result)? != *page {
        return Err("v14 ENTIRE V13 page inverse");
    }
    Ok(result)
}
#[cfg_attr(not(test), allow(dead_code))]
fn encode_v14(page: &Value) -> Result<Value, &'static str> {
    encode_v14_tails(&encode_v13_logical(page)?)
}
pub(in super::super) fn expand_v14(
    before: &Value,
    policy: &Value,
    page: &Value,
) -> Result<Value, &'static str> {
    let logical = expand_to_v12_logical(&expand_to_v13(page)?)?;
    super::expand_v11(before, policy, &logical_v12::expand_to_v11(&logical)?)
}
