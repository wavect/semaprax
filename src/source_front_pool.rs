//! Test-only lossless string pooling for the existing complete custody packet.
//! Physical readers authenticate the wire before calling this decoder.
use serde::de::{self, MapAccess, SeqAccess, Visitor};
use serde::{Deserialize, Deserializer};
use serde_json::{Map, Value};
use std::collections::BTreeMap;
use std::fmt;

const SCHEMA: &str = "semaprax.source-edit-string-pool.v1";
const LEGACY: &str = "semaprax.next-custody-four-fronts.v1";
const TAG: &str = "$source_edit_string_v1";
const WIRE_CAP: usize = 2_097_152;
const SOURCE_CAP: usize = 9_753_094;

// The old serde_json parser remains the legacy branch. Only the new envelope
// passes this visitor, which rejects duplicate keys even in opaque metadata.
struct Unique(Value);
impl<'de> Deserialize<'de> for Unique {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        struct V;
        impl<'de> Visitor<'de> for V {
            type Value = Unique;
            fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                f.write_str("JSON with unique object keys")
            }
            fn visit_bool<E: de::Error>(self, v: bool) -> Result<Unique, E> {
                Ok(Unique(v.into()))
            }
            fn visit_i64<E: de::Error>(self, v: i64) -> Result<Unique, E> {
                Ok(Unique(v.into()))
            }
            fn visit_u64<E: de::Error>(self, v: u64) -> Result<Unique, E> {
                Ok(Unique(v.into()))
            }
            fn visit_f64<E: de::Error>(self, v: f64) -> Result<Unique, E> {
                serde_json::Number::from_f64(v)
                    .map(|n| Unique(Value::Number(n)))
                    .ok_or_else(|| E::custom("nonfinite JSON"))
            }
            fn visit_str<E: de::Error>(self, v: &str) -> Result<Unique, E> {
                Ok(Unique(v.into()))
            }
            fn visit_string<E: de::Error>(self, v: String) -> Result<Unique, E> {
                Ok(Unique(v.into()))
            }
            fn visit_unit<E: de::Error>(self) -> Result<Unique, E> {
                Ok(Unique(Value::Null))
            }
            fn visit_none<E: de::Error>(self) -> Result<Unique, E> {
                Ok(Unique(Value::Null))
            }
            fn visit_seq<A: SeqAccess<'de>>(self, mut a: A) -> Result<Unique, A::Error> {
                let mut v = Vec::new();
                while let Some(Unique(item)) = a.next_element()? {
                    v.push(item);
                }
                Ok(Unique(Value::Array(v)))
            }
            fn visit_map<A: MapAccess<'de>>(self, mut a: A) -> Result<Unique, A::Error> {
                let mut v = Map::new();
                while let Some(k) = a.next_key::<String>()? {
                    if v.contains_key(&k) {
                        return Err(de::Error::custom("duplicate JSON key"));
                    }
                    let Unique(item) = a.next_value()?;
                    v.insert(k, item);
                }
                Ok(Unique(Value::Object(v)))
            }
        }
        d.deserialize_any(V)
    }
}

fn exact_edit(v: &Value) -> bool {
    v.as_object().is_some_and(|m| {
        m.len() == 4
            && ["start", "end", "after", "before"]
                .iter()
                .all(|k| m.contains_key(*k))
    })
}
fn edits<'a>(v: &'a Value, key: &str, out: &mut Vec<&'a Value>) -> Result<(), &'static str> {
    if let Some(a) = v.get(key) {
        for edit in a.as_array().ok_or("edit array")? {
            if exact_edit(edit) {
                out.extend([&edit["after"], &edit["before"]]);
            }
        }
    }
    Ok(())
}
fn payloads(body: &Value) -> Result<Vec<&Value>, &'static str> {
    let mut out = Vec::new();
    body.get("rows").and_then(Value::as_array).ok_or("rows")?;
    body.get("constructor_retention_903")
        .and_then(|v| v.get("records"))
        .and_then(Value::as_object)
        .ok_or("records")?;
    body.get("main8_raw_current_fronts")
        .and_then(|v| v.get("rows"))
        .and_then(Value::as_array)
        .ok_or("raw rows")?;
    if let Some(rows) = body.get("rows") {
        for row in rows.as_array().ok_or("rows")? {
            // Canonical domain order is target then middle, not JSON key order.
            for key in ["target", "middle"] {
                if let Some(stage) = row.get(key).filter(|v| !v.is_null()) {
                    edits(stage, "edits", &mut out)?;
                }
            }
        }
    }
    if let Some(records) = body
        .get("constructor_retention_903")
        .and_then(|v| v.get("records"))
    {
        let records = records.as_object().ok_or("records")?;
        let mut names: Vec<_> = records.keys().collect();
        names.sort_unstable();
        for name in names {
            edits(&records[name], "edits", &mut out)?;
        }
    }
    if let Some(rows) = body
        .get("main8_raw_current_fronts")
        .and_then(|v| v.get("rows"))
    {
        for row in rows.as_array().ok_or("raw rows")? {
            edits(row, "inverse", &mut out)?;
        }
    }
    Ok(out)
}
fn reference(v: &Value) -> Result<Option<usize>, &'static str> {
    let Some(m) = v.as_object() else {
        return Ok(None);
    };
    if m.len() != 1 || !m.contains_key(TAG) {
        return Err("payload object");
    }
    Ok(Some(
        usize::try_from(m[TAG].as_u64().ok_or("reference integer")?)
            .map_err(|_| "reference range")?,
    ))
}
fn json_string_len(s: &str) -> Result<usize, &'static str> {
    s.bytes().try_fold(2usize, |n, b| {
        n.checked_add(match b {
            b'"' | b'\\' | b'\x08' | b'\x0c' | b'\n' | b'\r' | b'\t' => 2,
            0..=31 => 6,
            _ => 1,
        })
        .ok_or("expanded overflow")
    })
}
fn expand_edits(v: &mut Value, key: &str, strings: &[String]) {
    if let Some(a) = v.get_mut(key).and_then(Value::as_array_mut) {
        for edit in a {
            if exact_edit(edit) {
                for key in ["after", "before"] {
                    if let Some(index) = reference(&edit[key]).expect("validated payload") {
                        edit[key] = Value::String(strings[index].clone());
                    }
                }
            }
        }
    }
}
fn expand(body: &mut Value, strings: &[String]) {
    if let Some(rows) = body.get_mut("rows").and_then(Value::as_array_mut) {
        for row in rows {
            for key in ["target", "middle"] {
                if let Some(stage) = row.get_mut(key) {
                    expand_edits(stage, "edits", strings);
                }
            }
        }
    }
    if let Some(records) = body
        .get_mut("constructor_retention_903")
        .and_then(|v| v.get_mut("records"))
        .and_then(Value::as_object_mut)
    {
        for row in records.values_mut() {
            expand_edits(row, "edits", strings);
        }
    }
    if let Some(rows) = body
        .get_mut("main8_raw_current_fronts")
        .and_then(|v| v.get_mut("rows"))
        .and_then(Value::as_array_mut)
    {
        for row in rows {
            expand_edits(row, "inverse", strings);
        }
    }
}

pub(crate) fn decode(raw: &str) -> Result<Value, &'static str> {
    decode_with_cap(raw, SOURCE_CAP)
}
fn decode_with_cap(raw: &str, expanded_cap: usize) -> Result<Value, &'static str> {
    if raw.len() > WIRE_CAP {
        return Err("wire bound");
    }
    let old: Value = serde_json::from_str(raw).map_err(|_| "JSON")?;
    if old.get("schema").and_then(Value::as_str) == Some(LEGACY) {
        return Ok(old);
    }
    if old.get("schema").and_then(Value::as_str) != Some(SCHEMA) {
        return Err("schema");
    }
    drop(old);
    let Unique(value) = serde_json::from_str(raw).map_err(|_| "unique JSON")?;
    let Value::Object(mut envelope) = value else {
        return Err("envelope");
    };
    if envelope.len() != 3
        || !["schema", "strings", "body"]
            .iter()
            .all(|k| envelope.contains_key(*k))
    {
        return Err("envelope fields");
    }
    let mut body = envelope.remove("body").ok_or("body")?;
    if body.get("schema").and_then(Value::as_str) != Some(LEGACY) {
        return Err("body schema");
    }
    let strings: Vec<String> = serde_json::from_value(envelope.remove("strings").ok_or("strings")?)
        .map_err(|_| "strings")?;
    let mut unique = std::collections::BTreeSet::new();
    for s in &strings {
        if s.len() < 64 || !unique.insert(s.as_str()) {
            return Err("pool string");
        }
    }
    let slots = payloads(&body)?;
    let mut counts = vec![0usize; strings.len()];
    let mut all: BTreeMap<&str, (usize, bool)> = BTreeMap::new();
    let mut next = 0usize;
    let mut expanded = serde_json::to_vec(&body)
        .map_err(|_| "body JSON")?
        .len()
        .checked_add(1)
        .ok_or("expanded overflow")?;
    for slot in slots {
        let (text, pooled) = if let Some(index) = reference(slot)? {
            let s = strings.get(index).ok_or("reference bounds")?;
            if counts[index] == 0 {
                if index != next {
                    return Err("first use order");
                }
                next += 1;
            }
            counts[index] = counts[index].checked_add(1).ok_or("use overflow")?;
            expanded = expanded
                .checked_sub(
                    serde_json::to_vec(slot)
                        .map_err(|_| "reference JSON")?
                        .len(),
                )
                .and_then(|n| n.checked_add(json_string_len(s).ok()?))
                .ok_or("expanded overflow")?;
            (s.as_str(), true)
        } else {
            (slot.as_str().ok_or("payload string")?, false)
        };
        if text.len() >= 64 {
            let entry = all.entry(text).or_insert((0, true));
            entry.0 = entry.0.checked_add(1).ok_or("count overflow")?;
            entry.1 &= pooled;
        }
    }
    if counts.iter().any(|n| *n < 2) {
        return Err("unused or single pool string");
    }
    if all.values().any(|(n, pooled)| *n >= 2 && !pooled) {
        return Err("unpooled repeated string");
    }
    if expanded > expanded_cap {
        return Err("expanded bound");
    }
    // All reference/type/canonicality checks and the complete expanded byte
    // bound precede the first expanded string clone.
    expand(&mut body, &strings);
    debug_assert_eq!(
        serde_json::to_vec(&body).expect("JSON value").len() + 1,
        expanded
    );
    Ok(body)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    fn edit(after: Value, before: Value) -> Value {
        json!({"start":0,"end":0,"after":after,"before":before})
    }
    fn reference_at(n: usize) -> Value {
        json!({(TAG):n})
    }
    fn envelope(mut body: Value, strings: Vec<String>) -> Value {
        let m = body.as_object_mut().unwrap();
        m.entry("rows").or_insert_with(|| json!([]));
        m.entry("constructor_retention_903")
            .or_insert_with(|| json!({"records":{}}));
        m.entry("main8_raw_current_fronts")
            .or_insert_with(|| json!({"rows":[]}));
        json!({"schema":SCHEMA,"strings":strings,"body":body})
    }
    fn wire(v: &Value) -> String {
        serde_json::to_string(v).unwrap()
    }
    #[test]
    fn full_legacy_unknowns_and_known_pool_domains_roundtrip() {
        let text = "é\\\n\"".repeat(32);
        let mut body = json!({"schema":LEGACY,"rows":[{"target":{"edits":[edit(reference_at(0),reference_at(0))]},"middle":null}],"constructor_retention_903":{"records":{"z":{"edits":[edit(reference_at(0),json!("short"))]}}},"main8_raw_current_fronts":{"rows":[{"inverse":[edit(reference_at(0),json!("short"))]}]},"unknown":{"after":{(TAG):9}},"opaque":[1,true,null]});
        let encoded = envelope(body.clone(), vec![text.clone()]);
        expand(&mut body, &[text]);
        assert_eq!(decode(&wire(&encoded)).unwrap(), body);
        assert_eq!(decode(&wire(&body)).unwrap(), body);
        let mut opaque = edit(json!("x".repeat(90)), json!("x".repeat(90)));
        opaque["unknown"] = json!({(TAG):999});
        let value = json!({"schema":LEGACY,"rows":[{"target":{"edits":[opaque.clone()]}}],"constructor_retention_903":{"records":{}},"main8_raw_current_fronts":{"rows":[]}});
        assert_eq!(
            decode(&wire(&envelope(value.clone(), vec![]))).unwrap(),
            value
        );
    }
    #[test]
    fn canonical_first_use_lexical_records_and_all_refusals() {
        let a = "a".repeat(64);
        let b = "b".repeat(64);
        let body = json!({"schema":LEGACY,"constructor_retention_903":{"records":{"z":{"edits":[edit(reference_at(1),reference_at(1))]},"a":{"edits":[edit(reference_at(0),reference_at(0))]}}}});
        let good = envelope(body.clone(), vec![a.clone(), b.clone()]);
        assert!(decode(&wire(&good)).is_ok());
        for bad in [
            envelope(body.clone(), vec![b.clone(), b.clone()]),
            envelope(body.clone(), vec![a.clone()]),
            envelope(body.clone(), vec!["short".into(), b.clone()]),
            envelope(body.clone(), vec![a.clone(), b.clone(), "c".repeat(64)]),
        ] {
            assert!(decode(&wire(&bad)).is_err());
        }
        let mut bad = good.clone();
        bad["body"]["constructor_retention_903"]["records"]["a"]["edits"][0]["after"] =
            reference_at(1);
        assert!(decode(&wire(&bad)).is_err());
        for payload in [
            json!({(TAG):true}),
            json!({(TAG):-1}),
            json!({(TAG):0,"extra":0}),
            json!({"wrong":0}),
        ] {
            let v = envelope(
                json!({"schema":LEGACY,"rows":[{"target":{"edits":[edit(payload,reference_at(0))]}}]}),
                vec![a.clone()],
            );
            assert!(decode(&wire(&v)).is_err());
        }
        let v = envelope(
            json!({"schema":LEGACY,"rows":[{"target":{"edits":[edit(json!(a.clone()),json!(a))]}}]}),
            vec![],
        );
        assert!(decode(&wire(&v)).is_err());
        let raw = wire(&good);
        let duplicate = raw.replacen("\"schema\":", "\"schema\":\"duplicate\",\"schema\":", 1);
        assert!(decode(&duplicate).is_err());
    }
    #[test]
    fn closed_envelope_legacy_semantics_and_unknown_metadata_are_distinct() {
        let text = "é".repeat(32); // 64 UTF8 bytes, not 64 characters.
        let body = json!({"schema":LEGACY,"rows":[{"target":{"edits":[edit(reference_at(0),reference_at(0))]}}]});
        let good = envelope(body, vec![text]);
        for key in ["schema", "strings", "body"] {
            let mut bad = good.clone();
            bad.as_object_mut().unwrap().remove(key);
            assert!(decode(&wire(&bad)).is_err());
        }
        let mut bad = good.clone();
        bad["extra"] = json!(0);
        assert!(decode(&wire(&bad)).is_err());
        let mut bad = good.clone();
        bad["schema"] = json!("unknown");
        assert!(decode(&wire(&bad)).is_err());
        let mut bad = good.clone();
        bad["body"]["schema"] = json!("unknown");
        assert!(decode(&wire(&bad)).is_err());
        for key in [
            "rows",
            "constructor_retention_903",
            "main8_raw_current_fronts",
        ] {
            let mut bad = good.clone();
            bad["body"].as_object_mut().unwrap().remove(key);
            assert!(decode(&wire(&bad)).is_err());
        }
        let mut opaque = good.clone();
        opaque["body"]["unknown"] = json!({(TAG):[true,{(TAG):999}]});
        assert_eq!(
            decode(&wire(&opaque)).unwrap()["unknown"],
            opaque["body"]["unknown"]
        );
        // The historical serde parser's last-key behavior is retained only for
        // the complete legacy branch; new-wrapper duplicate keys refuse.
        let legacy = r#"{"schema":"semaprax.next-custody-four-fronts.v1","unknown":1,"unknown":2}"#;
        assert_eq!(
            decode(legacy).unwrap(),
            serde_json::from_str::<Value>(legacy).unwrap()
        );
        let duplicate = wire(&opaque).replacen("\"unknown\":", "\"unknown\":null,\"unknown\":", 1);
        assert!(decode(&duplicate).is_err());
    }
    #[test]
    fn full_expanded_size_exact_short_unicode_escape_and_amplification_bounds() {
        let text = "é\n\u{0000}\\\"".repeat(32);
        assert_eq!(
            json_string_len(&text).unwrap(),
            serde_json::to_string(&text).unwrap().len()
        );
        let body = json!({"schema":LEGACY,"rows":[{"target":{"edits":[edit(reference_at(0),reference_at(0))]}}]});
        let raw = wire(&envelope(body, vec![text]));
        let expanded = decode(&raw).unwrap();
        let size = wire(&expanded).len() + 1;
        assert_eq!(decode_with_cap(&raw, size).unwrap(), expanded);
        assert_eq!(decode_with_cap(&raw, size - 1), Err("expanded bound"));
        let huge = "h".repeat(100_000);
        let edits: Vec<_> = (0..60)
            .map(|_| edit(reference_at(0), reference_at(0)))
            .collect();
        let raw = wire(&envelope(
            json!({"schema":LEGACY,"rows":[{"target":{"edits":edits}}]}),
            vec![huge],
        ));
        assert!(raw.len() < WIRE_CAP);
        assert_eq!(decode(&raw), Err("expanded bound"));
        assert_eq!(decode(&" ".repeat(WIRE_CAP + 1)), Err("wire bound"));
    }
}
