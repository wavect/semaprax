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
pub(in super::super) use tests::{before_source, verify_current_v13, verify_current_v14, verify_current_v15, verify_current_v16};

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

// Private V15 logical input; public V14 wire entry remains unchanged.
fn expand_to_v13_logical(page: &Value) -> Result<Value, &'static str> {
    let object = page.as_object().ok_or("v14 envelope")?;
    if page["schema"] != SCHEMA_V14
        || object.len() != KEYS.len() + 2
        || !KEYS.iter().all(|key| object.contains_key(*key))
        || !object.contains_key("digest_table")
        || !object.contains_key("pin_tails")
    {
        return Err("v14 envelope");
    }
    if canonical(page).len() > SOURCE_CAP {
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

fn encode_v14_tails_logical(page: &Value) -> Result<Value, &'static str> {
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
    if canonical(&result).len() > SOURCE_CAP {
        return Err("v14 same one-page cap");
    }
    if expand_to_v13_logical(&result)? != *page {
        return Err("v14 ENTIRE V13 page inverse");
    }
    Ok(result)
}

pub(in super::super) const SCHEMA_V15: &str = "semaprax.finite-source-overlay-page.v15";
const V15_CODES: &[u8] = b"0123456789ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefgh";
const SOURCE_PREFIXES: [&str; 44] = [
    "src/cleanup_plan/replay/ordinary_plan_history/comparison_owner/",
    "src/cleanup_plan/replay/",
    "src/workspace_graph/expected_projection/",
    "src/cleanup_plan/replay/factored/canonical_shared/inline_singleton/",
    "src/hir/validation/frame_reuse/",
    "src/workspace_graph/expected_projection/shared_local_callees/",
    "src/cleanup_plan/replay/path_join/",
    "src/cleanup_plan/replay/ordinary_plan_history/completed_fork/",
    "src/cleanup_plan/replay/ordinary_plan_history/comparison_owner/tests/tail_source/prepared_links/reachable_primitives/",
    "src/source_reflows_merge/",
    "src/cleanup_plan/replay/expression_facts/",
    "tests/wasm/scalar_browser_ci_contract/",
    "src/cleanup_plan/replay/skeleton_work/enumerated_prefix/",
    "src/cleanup_plan/replay/factored/canonical_shared/inline_singleton/lexical_successors/frame_loop/",
    "src/cleanup_plan/replay/expression_facts/receiving/retained/",
    "src/cleanup_plan/replay/ordinary_plan_history/",
    "src/hir/inspection/paid_nul/",
    "src/cleanup_plan/replay/path_join/before_frontier/",
    "src/cleanup_plan/replay/skeleton_work/",
    "src/cleanup_plan/replay/path_join/selected/before_frontier/",
    "src/workspace_graph/",
    "src/bounded_output/",
    "src/workspace_graph/expected_projection/borrowed_imports/typed/",
    "src/cleanup_plan/replay/factored/",
    "src/cleanup_plan/replay/continuation_storage/",
    "src/cleanup_plan/replay/path_join/selected/frontier_tests/",
    "src/source_reflows_merge/main_e177_overlay_digests_v13/",
    "src/hir/validation/",
    "src/codegen/native_owned_data_provider/",
    "src/ast/program_source/",
    "src/cleanup_plan/",
    "src/workspace_graph/expected_projection/shared_local_callees/before903/",
    "src/hir/resolve_expr/sort_keys/",
    "src/workspace_graph/expected_projection/private_layout/",
    "src/test_allocation_observer/",
    "src/loan_plan/",
    "src/cleanup_plan/replay/path_join/shared_insertions_before/",
    "src/string_ops/conditions/",
    "src/cli/help/authoring/",
    "src/hir/lexical_exit/",
    "src/bounded_output/stage_debits/route_debits/",
    "src/literal_format/",
    "src/hir/validation/scope_checkpoint/marks/",
    "src/cleanup_plan/replay/ordinary_plan_history/comparison_owner/retained_main_289/",
];
fn source_slots(page: &mut Value, convert: &mut dyn FnMut(&Value) -> Result<Value, &'static str>) -> Result<(), &'static str> {
    fn row(value: &mut Value, convert: &mut dyn FnMut(&Value) -> Result<Value, &'static str>) -> Result<(), &'static str> {
        let object = value.as_object_mut().ok_or("v15 literal row")?;
        if let Some(value) = object.get_mut("source") { *value = convert(value)?; }
        Ok(())
    }
    fn fragments(value: &mut Value, convert: &mut dyn FnMut(&Value) -> Result<Value, &'static str>) -> Result<(), &'static str> {
        if value.is_null() { return Ok(()); }
        for item in value.as_array_mut().ok_or("v15 fragments")? {
            let object = item.as_object().ok_or("v15 fragment row")?;
            if !object.contains_key("canonical_source") { row(item, convert)?; }
        }
        Ok(())
    }
    for group in ["replacements", "additions"] {
        for value in page[group].as_array_mut().ok_or("v15 rows")? {
            let object = value.as_object_mut().ok_or("v15 row")?;
            if object.contains_key("old519_index") {
                if group != "replacements" || object.len() != 1 { return Err("v15 indexed row"); }
                let tuple = object.get_mut("old519_index").unwrap().as_array_mut().ok_or("v15 old519 arity")?;
                if tuple.len() != 4 { return Err("v15 old519 arity"); }
                fragments(&mut tuple[3], convert)?;
            } else {
                row(value, convert)?;
                if let Some(items) = value.get_mut("fragments") { fragments(items, convert)?; }
            }
        }
    }
    Ok(())
}
fn source_prefix(path: &str) -> Option<usize> {
    SOURCE_PREFIXES.iter().enumerate().filter(|(_, prefix)| path.starts_with(**prefix)).max_by_key(|(_, prefix)| prefix.len()).map(|(index, _)| index)
}
fn source_path(value: &Value) -> Result<String, &'static str> {
    let text = value.as_str().ok_or("v15 source type")?;
    if !text.contains(':') {
        if !super::super::normal(text) { return Err("v15 source path"); }
        if source_prefix(text).is_some() { return Err("v15 uninterned source"); }
        return Ok(text.to_owned());
    }
    let bytes = text.as_bytes();
    if bytes.len() < 3 || bytes[1] != b':' { return Err("v15 source reference"); }
    let index = V15_CODES.iter().position(|code| *code == bytes[0]).ok_or("v15 source reference")?;
    let prefix = SOURCE_PREFIXES.get(index).ok_or("v15 source reference")?;
    let path = format!("{}{}", prefix, &text[2..]);
    if !super::super::normal(&path) { return Err("v15 expanded source path"); }
    if source_prefix(&path) != Some(index) { return Err("v15 noncanonical source prefix"); }
    Ok(path)
}
fn source_envelope(page: &Value, schema: &str) -> Result<(), &'static str> {
    let object = page.as_object().ok_or("v15 envelope")?;
    if page["schema"] != schema || object.len() != KEYS.len()+2 || !KEYS.iter().all(|key| object.contains_key(*key)) || !object.contains_key("digest_table") || !object.contains_key("pin_tails") { return Err("v15 envelope"); }
    Ok(())
}
fn expand_to_v14(page: &Value) -> Result<Value, &'static str> {
    source_envelope(page, SCHEMA_V15)?;
    if canonical(page).len() > PAGE_CAP { return Err("v15 same one-page cap"); }
    let mut result = page.clone(); result["schema"] = SCHEMA_V14.into();
    let mut expanded = canonical(&result).len();
    source_slots(&mut result, &mut |value| {
        let path = Value::String(source_path(value)?);
        expanded = expanded.checked_sub(canonical(value).len()).and_then(|n| n.checked_add(canonical(&path).len())).ok_or("v15 expanded bound")?;
        if expanded > SOURCE_CAP { return Err("v15 expanded bound"); }
        Ok(value.clone())
    })?;
    if expanded > SOURCE_CAP { return Err("v15 expanded bound"); }
    source_slots(&mut result, &mut |value| Ok(source_path(value)?.into()))?;
    if canonical(&result).len() != expanded { return Err("v15 expanded bound"); }
    expand_to_v13_logical(&result)?;
    Ok(result)
}
#[cfg_attr(not(test), allow(dead_code))]
fn encode_v15_sources(page: &Value) -> Result<Value, &'static str> {
    source_envelope(page, SCHEMA_V14)?;
    if canonical(page).len() > SOURCE_CAP { return Err("v15 logical input bound"); }
    expand_to_v13_logical(page)?;
    let mut result = page.clone();
    source_slots(&mut result, &mut |value| {
        let path = value.as_str().ok_or("v15 source type")?;
        if !super::super::normal(path) { return Err("v15 source path"); }
        Ok(match source_prefix(path) {
            Some(index) => format!("{}:{}", char::from(V15_CODES[index]), &path[SOURCE_PREFIXES[index].len()..]).into(),
            None => value.clone(),
        })
    })?;
    result["schema"] = SCHEMA_V15.into();
    if canonical(&result).len() > PAGE_CAP { return Err("v15 same one-page cap"); }
    if canonical(&expand_to_v14(&result)?) != canonical(page) { return Err("v15 ENTIRE V14 page inverse"); }
    Ok(result)
}
#[cfg_attr(not(test), allow(dead_code))]
fn encode_v15(page: &Value) -> Result<Value, &'static str> {
    encode_v15_sources(&encode_v14_tails_logical(&encode_v13_logical(page)?)?)
}
pub(in super::super) fn expand_v15(before: &Value, policy: &Value, page: &Value) -> Result<Value, &'static str> {
    let logical = expand_to_v12_logical(&expand_to_v13_logical(&expand_to_v14(page)?)?)?;
    super::expand_v11(before, policy, &logical_v12::expand_to_v11(&logical)?)
}

// Private V16 frontier: complete original public V15 entries remain unchanged.
fn expand_to_v14_logical(page: &Value) -> Result<Value, &'static str> {
    source_envelope(page, SCHEMA_V15)?;
    if canonical(page).len() > SOURCE_CAP { return Err("v15 same one-page cap"); }
    let mut result = page.clone(); result["schema"] = SCHEMA_V14.into();
    let mut expanded = canonical(&result).len();
    source_slots(&mut result, &mut |value| {
        let path = Value::String(source_path(value)?);
        expanded = expanded.checked_sub(canonical(value).len()).and_then(|n| n.checked_add(canonical(&path).len())).ok_or("v15 expanded bound")?;
        if expanded > SOURCE_CAP { return Err("v15 expanded bound"); }
        Ok(value.clone())
    })?;
    if expanded > SOURCE_CAP { return Err("v15 expanded bound"); }
    source_slots(&mut result, &mut |value| Ok(source_path(value)?.into()))?;
    if canonical(&result).len() != expanded { return Err("v15 expanded bound"); }
    expand_to_v13_logical(&result)?;
    Ok(result)
}
#[cfg_attr(not(test), allow(dead_code))]
fn encode_v15_sources_logical(page: &Value) -> Result<Value, &'static str> {
    source_envelope(page, SCHEMA_V14)?;
    if canonical(page).len() > SOURCE_CAP { return Err("v15 logical input bound"); }
    expand_to_v13_logical(page)?;
    let mut result = page.clone();
    source_slots(&mut result, &mut |value| {
        let path = value.as_str().ok_or("v15 source type")?;
        if !super::super::normal(path) { return Err("v15 source path"); }
        Ok(match source_prefix(path) {
            Some(index) => format!("{}:{}", char::from(V15_CODES[index]), &path[SOURCE_PREFIXES[index].len()..]).into(),
            None => value.clone(),
        })
    })?;
    result["schema"] = SCHEMA_V15.into();
    if canonical(&result).len() > SOURCE_CAP { return Err("v15 same one-page cap"); }
    if canonical(&expand_to_v14_logical(&result)?) != canonical(page) { return Err("v15 ENTIRE V14 page inverse"); }
    Ok(result)
}

pub(in super::super) const SCHEMA_V16: &str = "semaprax.finite-source-overlay-page.v16";
fn is_row_envelope(value: &Value) -> bool {
    value
        .as_object()
        .is_some_and(|object| object.len() == 1 && object.contains_key("source_row"))
}
const ROW_FIELDS: [&str; 5] = ["source", "compiled", "current", "views", "fragments"];
fn eligible_row(value: &Value) -> bool {
    value.as_object().is_some_and(|object| {
        (object.len() == 4 || object.len() == 5)
            && ROW_FIELDS[..4].iter().all(|key| object.contains_key(*key))
            && (object.len() == 4 || object.contains_key("fragments"))
    })
}
fn row_tuple(value: &Value) -> Result<&Vec<Value>, &'static str> {
    let object = value.as_object().ok_or("v16 row envelope")?;
    if object.len() != 1 {
        return Err("v16 row envelope");
    }
    let items = object
        .get("source_row")
        .and_then(Value::as_array)
        .ok_or("v16 row arity")?;
    if items.len() != 4 && items.len() != 5 {
        return Err("v16 row arity");
    }
    if !items[0].is_string() || !items[1].is_boolean() {
        return Err("v16 source/compiled type");
    }
    if !(items[2].is_array() || items[2].is_object()) || !items[3].is_array() {
        return Err("v16 current/views type");
    }
    // The visitor validates top-level fragments; a fragment row's fifth payload is opaque.
    Ok(items)
}
// Visits borrowed payloads only. No expanded row or repeated payload is allocated.
fn visit_source_rows(
    page: &Value,
    inspect: &mut dyn FnMut(&Value) -> Result<(), &'static str>,
) -> Result<(), &'static str> {
    fn fragments(
        value: &Value,
        inspect: &mut dyn FnMut(&Value) -> Result<(), &'static str>,
    ) -> Result<(), &'static str> {
        if value.is_null() {
            return Ok(());
        }
        for value in value.as_array().ok_or("v16 fragments")? {
            let object = value.as_object().ok_or("v16 fragment row")?;
            if !object.contains_key("canonical_source") {
                literal(value, inspect, false)?;
            }
        }
        Ok(())
    }
    fn literal(
        value: &Value,
        inspect: &mut dyn FnMut(&Value) -> Result<(), &'static str>,
        outer: bool,
    ) -> Result<(), &'static str> {
        inspect(value)?;
        if outer && is_row_envelope(value) {
            let items = row_tuple(value)?;
            if items.len() == 5 {
                fragments(&items[4], inspect)?;
            }
        } else if outer {
            if let Some(value) = value.get("fragments") {
                fragments(value, inspect)?;
            }
        }
        Ok(())
    }
    for group in ["replacements", "additions"] {
        for value in page[group].as_array().ok_or("v16 rows")? {
            let object = value.as_object().ok_or("v16 row")?;
            if let Some(reference) = object.get("old519_index") {
                if group != "replacements" || object.len() != 1 {
                    return Err("v16 indexed row");
                }
                let reference = reference.as_array().ok_or("v16 old519 arity")?;
                if reference.len() != 4 {
                    return Err("v16 old519 arity");
                }
                fragments(&reference[3], inspect)?;
            } else {
                literal(value, inspect, true)?;
            }
        }
    }
    Ok(())
}
fn row_overhead(arity: usize) -> usize {
    // Canonical object keys, colons, braces and commas minus fixed envelope punctuation.
    2 + ROW_FIELDS[..arity]
        .iter()
        .map(|field| field.len() + 3)
        .sum::<usize>()
        + arity
        - 1
        - (b"{\"source_row\":[]}".len() + arity - 1)
}
fn transform_source_rows(page: &mut Value, decode: bool) -> Result<(), &'static str> {
    fn fragments(value: &mut Value, decode: bool) -> Result<(), &'static str> {
        if value.is_null() {
            return Ok(());
        }
        for value in value.as_array_mut().ok_or("v16 fragments")? {
            if !value
                .as_object()
                .ok_or("v16 fragment row")?
                .contains_key("canonical_source")
            {
                literal(value, decode, false)?;
            }
        }
        Ok(())
    }
    fn literal(value: &mut Value, decode: bool, outer: bool) -> Result<(), &'static str> {
        if decode && is_row_envelope(value) {
            row_tuple(value)?;
            let mut envelope = std::mem::take(value)
                .as_object_mut()
                .unwrap()
                .remove("source_row")
                .unwrap();
            let items = std::mem::take(envelope.as_array_mut().unwrap());
            *value = Value::Object(
                ROW_FIELDS
                    .iter()
                    .zip(items)
                    .map(|(key, value)| ((*key).to_owned(), value))
                    .collect(),
            );
        }
        if outer {
            if let Some(items) = value.get_mut("fragments") {
                fragments(items, decode)?;
            }
        }
        if !decode && eligible_row(value) {
            let Value::Object(mut object) = std::mem::take(value) else {
                unreachable!()
            };
            let items = ROW_FIELDS
                .iter()
                .filter_map(|key| object.remove(*key))
                .collect::<Vec<_>>();
            *value = serde_json::json!({"source_row":items});
        }
        Ok(())
    }
    for group in ["replacements", "additions"] {
        for value in page[group].as_array_mut().ok_or("v16 rows")? {
            if value.get("old519_index").is_some() {
                fragments(&mut value["old519_index"][3], decode)?;
            } else {
                literal(value, decode, true)?;
            }
        }
    }
    Ok(())
}
fn expand_to_v15(page: &Value) -> Result<Value, &'static str> {
    source_envelope(page, SCHEMA_V16)?;
    let mut expanded = canonical(page).len();
    if expanded > PAGE_CAP {
        return Err("v16 same one-page cap");
    }
    visit_source_rows(page, &mut |value| {
        if is_row_envelope(value) {
            expanded = expanded
                .checked_add(row_overhead(row_tuple(value)?.len()))
                .ok_or("v16 expanded bound")?;
            if expanded > SOURCE_CAP {
                return Err("v16 expanded bound");
            }
        } else if eligible_row(value) {
            return Err("v16 literal row alias");
        }
        Ok(())
    })?;
    // Allocation/publication of reconstructed rows starts only after the full bound proof.
    let mut result = page.clone();
    result["schema"] = SCHEMA_V15.into();
    transform_source_rows(&mut result, true)?;
    if canonical(&result).len() != expanded {
        return Err("v16 expanded bound");
    }
    expand_to_v14_logical(&result)?;
    Ok(result)
}
#[cfg_attr(not(test), allow(dead_code))]
fn encode_v16_rows(page: &Value) -> Result<Value, &'static str> {
    source_envelope(page, SCHEMA_V15)?;
    if canonical(page).len() > SOURCE_CAP {
        return Err("v16 logical input bound");
    }
    expand_to_v14_logical(page)?;
    visit_source_rows(page, &mut |value| {
        if eligible_row(value) {
            if !value["source"].is_string() || !value["compiled"].is_boolean() {
                return Err("v16 source/compiled type");
            }
            if !(value["current"].is_array() || value["current"].is_object())
                || !value["views"].is_array()
            {
                return Err("v16 current/views type");
            }
        }
        Ok(())
    })?;
    let mut result = page.clone();
    transform_source_rows(&mut result, false)?;
    result["schema"] = SCHEMA_V16.into();
    if canonical(&result).len() > PAGE_CAP {
        return Err("v16 same one-page cap");
    }
    if canonical(&expand_to_v15(&result)?) != canonical(page) {
        return Err("v16 ENTIRE V15 page inverse");
    }
    Ok(result)
}
#[cfg_attr(not(test), allow(dead_code))]
fn encode_v16(page: &Value) -> Result<Value, &'static str> {
    encode_v16_rows(&encode_v15_sources_logical(&encode_v14_tails_logical(
        &encode_v13_logical(page)?,
    )?)?)
}
pub(in super::super) fn expand_v16(
    before: &Value,
    policy: &Value,
    page: &Value,
) -> Result<Value, &'static str> {
    let logical = expand_to_v12_logical(&expand_to_v13_logical(&expand_to_v14_logical(
        &expand_to_v15(page)?,
    )?)?)?;
    super::expand_v11(before, policy, &logical_v12::expand_to_v11(&logical)?)
}

// V17 elides only V16's redundant one-key wrapper at the same admitted slots.
// The complete public V16 reader and its wire cap remain unchanged above.
pub(in super::super) const SCHEMA_V17: &str = "semaprax.finite-source-overlay-page.v17";
fn positional_row(value: &Value) -> Result<&Vec<Value>, &'static str> {
    let items = value.as_array().ok_or("v17 row arity")?;
    if items.len() != 4 && items.len() != 5 {
        return Err("v17 row arity");
    }
    if !items[0].is_string() || !items[1].is_boolean() {
        return Err("v17 source/compiled type");
    }
    if !(items[2].is_array() || items[2].is_object()) || !items[3].is_array() {
        return Err("v17 current/views type");
    }
    Ok(items)
}
// Borrowed first pass: no expanded row or repeated payload is constructed.
fn visit_positional_rows(
    page: &Value,
    inspect: &mut dyn FnMut(&Value) -> Result<(), &'static str>,
) -> Result<(), &'static str> {
    fn fragments(
        value: &Value,
        inspect: &mut dyn FnMut(&Value) -> Result<(), &'static str>,
    ) -> Result<(), &'static str> {
        if value.is_null() {
            return Ok(());
        }
        for value in value.as_array().ok_or("v17 fragments")? {
            if !(value.is_array() || value.is_object()) {
                return Err("v17 fragment row");
            }
            if value.is_array() || value.get("canonical_source").is_none() {
                literal(value, inspect, false)?;
            }
        }
        Ok(())
    }
    fn literal(
        value: &Value,
        inspect: &mut dyn FnMut(&Value) -> Result<(), &'static str>,
        outer: bool,
    ) -> Result<(), &'static str> {
        if !(value.is_array() || value.is_object()) {
            return Err("v17 row type");
        }
        inspect(value)?;
        if outer && value.is_array() {
            let items = positional_row(value)?;
            if items.len() == 5 {
                fragments(&items[4], inspect)?;
            }
        } else if outer {
            if let Some(value) = value.get("fragments") {
                fragments(value, inspect)?;
            }
        }
        Ok(())
    }
    for group in ["replacements", "additions"] {
        for value in page[group].as_array().ok_or("v17 rows")? {
            if let Some(reference) = value.get("old519_index") {
                if group != "replacements" || value.as_object().is_none_or(|o| o.len() != 1) {
                    return Err("v17 indexed row");
                }
                let reference = reference.as_array().ok_or("v17 old519 arity")?;
                if reference.len() != 4 {
                    return Err("v17 old519 arity");
                }
                fragments(&reference[3], inspect)?;
            } else {
                literal(value, inspect, true)?;
            }
        }
    }
    Ok(())
}
fn transform_positional_rows(page: &mut Value, decode: bool) -> Result<(), &'static str> {
    fn fragments(value: &mut Value, decode: bool) -> Result<(), &'static str> {
        if value.is_null() {
            return Ok(());
        }
        for value in value.as_array_mut().ok_or("v17 fragments")? {
            if value.is_array() || value.get("canonical_source").is_none() {
                literal(value, decode, false)?;
            }
        }
        Ok(())
    }
    fn literal(value: &mut Value, decode: bool, outer: bool) -> Result<(), &'static str> {
        if decode && value.is_array() {
            positional_row(value)?;
            let Value::Array(items) = std::mem::take(value) else {
                unreachable!()
            };
            *value = Value::Object(
                ROW_FIELDS
                    .iter()
                    .zip(items)
                    .map(|(key, value)| ((*key).to_owned(), value))
                    .collect(),
            );
        }
        if outer {
            if let Some(value) = value.get_mut("fragments") {
                fragments(value, decode)?;
            }
        }
        if !decode && eligible_row(value) {
            let Value::Object(mut object) = std::mem::take(value) else {
                unreachable!()
            };
            *value = Value::Array(
                ROW_FIELDS
                    .iter()
                    .filter_map(|key| object.remove(*key))
                    .collect(),
            );
        }
        Ok(())
    }
    for group in ["replacements", "additions"] {
        for value in page[group].as_array_mut().ok_or("v17 rows")? {
            if value.get("old519_index").is_some() {
                fragments(&mut value["old519_index"][3], decode)?;
            } else {
                literal(value, decode, true)?;
            }
        }
    }
    Ok(())
}
fn expand_positional_to_v15(page: &Value) -> Result<Value, &'static str> {
    source_envelope(page, SCHEMA_V17)?;
    let mut expanded = canonical(page).len();
    if expanded > PAGE_CAP {
        return Err("v17 same one-page cap");
    }
    visit_positional_rows(page, &mut |value| {
        if value.is_array() {
            let overhead = row_overhead(positional_row(value)?.len())
                .checked_add(b"{\"source_row\":[]}".len() - 2)
                .ok_or("v17 expanded bound")?;
            expanded = expanded.checked_add(overhead).ok_or("v17 expanded bound")?;
            if expanded > SOURCE_CAP {
                return Err("v17 expanded bound");
            }
        } else if eligible_row(value) || is_row_envelope(value) {
            return Err("v17 literal row alias");
        }
        Ok(())
    })?;
    // The entire expanded bound is proved before cloning any payload.
    let mut result = page.clone();
    result["schema"] = SCHEMA_V15.into();
    transform_positional_rows(&mut result, true)?;
    if canonical(&result).len() != expanded {
        return Err("v17 expanded bound");
    }
    expand_to_v14_logical(&result)?;
    Ok(result)
}
#[cfg_attr(not(test), allow(dead_code))]
fn encode_v17_rows(page: &Value) -> Result<Value, &'static str> {
    source_envelope(page, SCHEMA_V15)?;
    if canonical(page).len() > SOURCE_CAP {
        return Err("v17 logical input bound");
    }
    expand_to_v14_logical(page)?;
    visit_source_rows(page, &mut |value| {
        if eligible_row(value) {
            if !value["source"].is_string() || !value["compiled"].is_boolean() {
                return Err("v17 source/compiled type");
            }
            if !(value["current"].is_array() || value["current"].is_object())
                || !value["views"].is_array()
            {
                return Err("v17 current/views type");
            }
        } else if is_row_envelope(value) {
            return Err("v17 literal row alias");
        }
        Ok(())
    })?;
    let mut result = page.clone();
    transform_positional_rows(&mut result, false)?;
    result["schema"] = SCHEMA_V17.into();
    if canonical(&result).len() > PAGE_CAP {
        return Err("v17 same one-page cap");
    }
    if canonical(&expand_positional_to_v15(&result)?) != canonical(page) {
        return Err("v17 ENTIRE V15 page inverse");
    }
    Ok(result)
}
#[cfg_attr(not(test), allow(dead_code))]
fn encode_v17(page: &Value) -> Result<Value, &'static str> {
    encode_v17_rows(&encode_v15_sources_logical(&encode_v14_tails_logical(
        &encode_v13_logical(page)?,
    )?)?)
}
pub(in super::super) fn expand_v17(
    before: &Value,
    policy: &Value,
    page: &Value,
) -> Result<Value, &'static str> {
    let logical = expand_to_v12_logical(&expand_to_v13_logical(&expand_to_v14_logical(
        &expand_positional_to_v15(page)?,
    )?)?)?;
    super::expand_v11(before, policy, &logical_v12::expand_to_v11(&logical)?)
}
