use super::*;
use serde_json::json;
use sha2::{Digest, Sha256};

#[path = "controller_transport.rs"]
mod controller_transport;

fn hash(bytes: &[u8]) -> String {
    format!("{:x}", crate::digest_hex::LowerHex(Sha256::digest(bytes)))
}
fn fixture() -> Value {
    let a = "A".repeat(43);
    let b = format!("{}A", "B".repeat(42));
    let c = format!("{}A", "C".repeat(42));
    json!({"schema":SCHEMA_V12,"index":1,"total_pages":1,"predecessor":"kept",
        "archive_prefixes":["e/"],"view_labels":[[0,"same"]],
        "replacements":[{"old519_index":[0,[1,a],[[0,2,a,[0,"archive"]],
            {"length":2,"sha256":a,"unknown":[a,0]}],
            [{"current":[3,b],"views":[[0,4,b]],"unknown":{"sha256":a}},
            {"canonical_source":"opaque","current":[false,"not-a-digest"]}]]}],
        "additions":[{"source":"literal.rs","compiled":true,"current":[5,c],
            "views":[],"extra":{"current":[1,a],"views":[[0,2,a]]},"fragments":null}]})
}
fn first_pin(page: &mut Value) -> &mut Value {
    &mut page["replacements"][0]["old519_index"][1][1]
}
fn hostile_cases(compact: &Value) -> Vec<Value> {
    let mut cases = Vec::new();
    for value in [
        json!(true),
        json!(false),
        json!(-1),
        json!(u64::MAX),
        json!(0.5),
        json!(null),
        json!([]),
        json!({}),
        json!("A".repeat(42)),
        json!(format!("{}B", "A".repeat(42))),
        json!("/".repeat(43)),
        json!("é".repeat(43)),
    ] {
        let mut bad = compact.clone();
        *first_pin(&mut bad) = value;
        cases.push(bad);
    }
    for value in [
        json!(true),
        json!(0),
        json!(null),
        json!([]),
        json!({}),
        json!("A".repeat(44)),
        json!("=".repeat(43)),
        json!("+".repeat(43)),
        json!(format!("{}B", "A".repeat(42))),
    ] {
        let mut bad = compact.clone();
        bad["digest_table"][0] = value;
        cases.push(bad);
    }
    let mut bad = compact.clone();
    bad["digest_table"][1] = compact["digest_table"][0].clone();
    cases.push(bad);
    let mut bad = compact.clone();
    *first_pin(&mut bad) = 1.into(); // valid index, wrong first use
    cases.push(bad);
    let mut bad = compact.clone();
    *first_pin(&mut bad) = compact["digest_table"][0].clone(); // literal/reference alias
    cases.push(bad);
    let mut bad = compact.clone();
    bad["digest_table"]
        .as_array_mut()
        .unwrap()
        .push(json!("D".repeat(42) + "A"));
    cases.push(bad); // unused declaration
    let mut bad = compact.clone();
    bad["additions"][0]["current"][1] = json!(2);
    bad["digest_table"]
        .as_array_mut()
        .unwrap()
        .push(fixture()["additions"][0]["current"][1].clone());
    cases.push(bad); // single-use declaration
    let mut bad = compact.clone();
    let literal = bad["additions"][0]["current"][1].clone();
    bad["additions"][0]["views"] = json!([[0, 1, literal]]);
    cases.push(bad); // repeated uninterned literal
    for value in [json!(false), json!([[0]]), json!([[0, 1, 0, 0, 0]])] {
        let mut bad = compact.clone();
        bad["additions"][0]["views"] = value;
        cases.push(bad);
    }
    for value in [json!(-1), json!(true), json!(SOURCE_CAP + 1)] {
        let mut bad = compact.clone();
        bad["additions"][0]["current"][0] = value;
        cases.push(bad);
    }
    let mut bad = compact.clone();
    bad["unknown"] = true.into();
    cases.push(bad);
    let mut bad = compact.clone();
    bad["replacements"][0]["unknown"] = true.into();
    cases.push(bad);
    let mut bad = compact.clone();
    bad.as_object_mut().unwrap().remove("digest_table");
    cases.push(bad);
    let mut bad = compact.clone();
    bad["digest_table"] = json!({});
    cases.push(bad);
    let mut bad = compact.clone();
    bad["schema"] = SCHEMA_V12.into();
    cases.push(bad);
    cases
}
fn python(full: &Value, hostile: &[Value]) -> Value {
    use std::io::{Read, Write};
    use std::process::{Command, Stdio};
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let mut child = Command::new("python3").args(["-B", "-c", r#"
from pathlib import Path
import runpy,sys,json
root=Path(sys.argv[1])
n=runpy.run_path(str(root/'docs/evidence/finite-source-overlay-v2-20261018/overlay_page.py'),run_name='v13_owning_parity')
v=json.load(sys.stdin)
encoded=n['encode_v13_digest_table'](v['full'])
refused=[]
for bad in v['hostile']:
 try:n['expand_v13_digest_table'](bad)
 except (AssertionError,KeyError,TypeError,ValueError):refused.append(True)
 else:refused.append(False)
sys.stdout.buffer.write(n['canonical']({'encoded':encoded,'decoded':n['expand_v13_digest_table'](encoded),'logical':n['_expand_v12_logical_view_labels'](n['expand_v13_digest_table'](encoded)),'refused':refused}))
"#]).arg(root).stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::inherit())
        .spawn().expect("V13 parity requires standard-library Python 3");
    child
        .stdin
        .take()
        .unwrap()
        .write_all(&canonical(&json!({"full":full,"hostile":hostile})))
        .unwrap();
    let mut bytes = Vec::new();
    let read = child
        .stdout
        .take()
        .unwrap()
        .take((SOURCE_CAP + 1) as u64)
        .read_to_end(&mut bytes);
    let status = child.wait().unwrap();
    read.unwrap();
    assert!(status.success(), "V13 Python parity failed: {status}");
    assert!(bytes.len() <= SOURCE_CAP);
    serde_json::from_slice(&bytes).unwrap()
}

#[test]
fn repeated_digest_first_use_preserves_complete_values_slots_order_and_multiplicity() {
    let full = fixture();
    let compact = encode_v13(&full).unwrap();
    assert_eq!(compact["digest_table"].as_array().unwrap().len(), 2);
    assert_eq!(compact["digest_table"][0], "A".repeat(43));
    assert_eq!(compact["replacements"][0]["old519_index"][2][0][2], 0);
    assert_eq!(
        compact["replacements"][0]["old519_index"][2][1],
        full["replacements"][0]["old519_index"][2][1]
    );
    assert_eq!(
        compact["replacements"][0]["old519_index"][3][1],
        full["replacements"][0]["old519_index"][3][1]
    );
    assert_eq!(
        compact["additions"][0]["extra"],
        full["additions"][0]["extra"]
    );
    assert_eq!(
        compact["additions"][0]["current"],
        full["additions"][0]["current"]
    );
    assert_eq!(
        canonical(&expand_to_v12(&compact).unwrap()),
        canonical(&full)
    );
    assert!(canonical(&compact).len() < canonical(&full).len());
}

#[test]
fn malformed_alias_duplicate_unused_single_use_index_type_order_and_arity_refuse() {
    let compact = encode_v13(&fixture()).unwrap();
    for bad in hostile_cases(&compact) {
        assert!(expand_to_v12(&bad).is_err(), "accepted {bad}");
    }
    for group in ["replacements", "additions"] {
        let mut bad = compact.clone();
        bad[group] = false.into();
        assert!(expand_to_v12(&bad).is_err());
    }
}

#[test]
fn python_rust_canonical_bytes_and_all_hostile_refusals_match() {
    let full = fixture();
    let compact = encode_v13(&full).unwrap();
    let hostile = hostile_cases(&compact);
    let actual = python(&full, &hostile);
    assert_eq!(canonical(&actual["encoded"]), canonical(&compact));
    assert_eq!(canonical(&actual["decoded"]), canonical(&full));
    assert_eq!(actual["refused"], json!(vec![true; hostile.len()]));
}

#[test]
fn logical_v12_above_public_cap_is_internal_only_and_roundtrips_in_both_languages() {
    let mut full = fixture();
    full["replacements"] = json!([]);
    full["additions"][0]["views"] = vec![json!([0, 1, "A".repeat(43)]); 6000].into();
    assert!(canonical(&full).len() > PAGE_CAP);
    assert!(canonical(&full).len() < SOURCE_CAP);
    assert_eq!(
        super::super::expand_v12(&Value::Null, &Value::Null, &full).unwrap_err(),
        "v12 same one-page cap"
    );
    let compact = encode_v13(&full).unwrap();
    assert!(canonical(&compact).len() < PAGE_CAP);
    assert_eq!(
        canonical(&expand_to_v12(&compact).unwrap()),
        canonical(&full)
    );
    let logical = logical_v12::expand_to_v11(&full).unwrap();
    assert_eq!(
        logical["additions"][0]["views"].as_array().unwrap().len(),
        6000
    );
    let actual = python(&full, &[]);
    assert_eq!(canonical(&actual["encoded"]), canonical(&compact));
    assert_eq!(canonical(&actual["decoded"]), canonical(&full));
}

#[test]
fn public_wire_boundary_and_internal_source_bound_cannot_be_overridden() {
    let mut compact = encode_v13(&fixture()).unwrap();
    compact["additions"][0]["padding"] = "".into();
    let padding = PAGE_CAP - canonical(&compact).len();
    compact["additions"][0]["padding"] = "x".repeat(padding).into();
    assert_eq!(canonical(&compact).len(), PAGE_CAP);
    assert!(expand_to_v12(&compact).is_ok());
    compact["additions"][0]["padding"] = "x".repeat(padding + 1).into();
    assert_eq!(
        expand_to_v12(&compact).unwrap_err(),
        "v13 same one-page cap"
    );
    let mut full = fixture();
    full["additions"][0]["padding"] = "x".repeat(SOURCE_CAP).into();
    assert_eq!(encode_v13(&full).unwrap_err(), "v13 logical input bound");
    assert_eq!(
        logical_v12::expand_to_v11(&full).unwrap_err(),
        "v12 same one-page cap"
    );
    assert_eq!((PAGE_CAP, SOURCE_CAP), (262144, 9753094));
}

#[test]
fn whole_current_page_reconstructs_every_v12_byte_and_python_rust_encoding() {
    // Independent historical V13 gate: preserve its entire accepted 424e page
    // and policy. A current V14 page has its own complete owning gate below.
    let raw = include_bytes!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/e/83c4e5fb3a31a6664df685cce09fa027c96a1910addaa6ccb884f6518ee6e3fa"
    ));
    assert_eq!(raw.len(), 259341);
    assert_eq!(
        hash(raw),
        "83c4e5fb3a31a6664df685cce09fa027c96a1910addaa6ccb884f6518ee6e3fa"
    );
    let policy_raw = include_bytes!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/e/98c659a9041d91d3da67b98f165d931a2efe11808f4e7ea5f4efd21c4987e210"
    ));
    assert_eq!(policy_raw.len(), 136074);
    assert_eq!(
        hash(policy_raw),
        "98c659a9041d91d3da67b98f165d931a2efe11808f4e7ea5f4efd21c4987e210"
    );
    let policy: Value = serde_json::from_slice(policy_raw).unwrap();
    assert_eq!(policy["page"]["length"], raw.len());
    assert_eq!(policy["page"]["sha256"], hash(raw));
    let page: Value = serde_json::from_slice(raw).unwrap();
    let full = if page["schema"] == SCHEMA_V13 {
        expand_to_v12(&page).unwrap()
    } else {
        page.clone()
    };
    assert_eq!(full["schema"], SCHEMA_V12);
    let compact = encode_v13(&full).unwrap();
    assert_eq!(
        canonical(&expand_to_v12(&compact).unwrap()),
        canonical(&full)
    );
    if page["schema"] == SCHEMA_V13 {
        assert_eq!(canonical(&compact).as_slice(), raw.as_slice());
    } else {
        assert_eq!(canonical(&full).as_slice(), raw.as_slice());
    }
    assert_eq!(
        compact["replacements"].as_array().unwrap().len(),
        full["replacements"].as_array().unwrap().len()
    );
    assert_eq!(
        compact["additions"].as_array().unwrap().len(),
        full["additions"].as_array().unwrap().len()
    );
    let actual = python(&full, &[]);
    assert_eq!(canonical(&actual["encoded"]), canonical(&compact));
    assert_eq!(canonical(&actual["decoded"]), canonical(&full));
}

// Retain the old V12 current-page gate when it is a V12 public page. A V13
// successor has a distinct internal logical path and must qualify that path;
// the original frozen V11/V12 controls still run before this dispatch.
pub(in super::super::super) fn verify_current_v13(page: &Value) {
    let full = expand_to_v12(page).unwrap();
    assert_eq!(canonical(&encode_v13(&full).unwrap()), canonical(page));
    let logical = logical_v12::expand_to_v11(&full).unwrap();
    let actual = python(&full, &[]);
    assert_eq!(canonical(&actual["encoded"]), canonical(page));
    assert_eq!(canonical(&actual["decoded"]), canonical(&full));
    assert_eq!(canonical(&actual["logical"]), canonical(&logical));
}

fn current(source: &str) -> &'static str {
    match source {
        "src/source_reflows_merge/main_e177_overlay_v2.rs" => {
            include_str!("../main_e177_overlay_v2.rs")
        }
        "src/source_reflows_merge/main_e177_overlay_fragments_v3.rs" => {
            include_str!("../main_e177_overlay_fragments_v3.rs")
        }
        "src/source_reflows_merge/main_e177_overlay_compiled.rs" => {
            include_str!("../main_e177_overlay_compiled.rs")
        }
        "src/source_reflows_merge/main_e177_overlay_labels_v12/tests.rs" => {
            include_str!("../main_e177_overlay_labels_v12/tests.rs")
        }
        "docs/evidence/finite-source-overlay-v2-20261018/overlay_page.py" => include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/docs/evidence/finite-source-overlay-v2-20261018/overlay_page.py"
        )),
        "docs/evidence/finite-source-overlay-v2-20261018/overlay_page_codecs.py" => {
            include_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/docs/evidence/finite-source-overlay-v2-20261018/overlay_page_codecs.py"
            ))
        }
        "logical-v12" => include_str!("logical_v12.rs"),
        _ => panic!("not a V13 source transport owner"),
    }
}
fn before(name: &str) -> &'static str {
    match name {
        "parent-0.before.txt" => include_str!("parent-0.before.txt"),
        "parent-1.before.txt" => include_str!("parent-1.before.txt"),
        "parent-2.before.txt" => include_str!("parent-2.before.txt"),
        "parent-3.before.txt" => include_str!("parent-3.before.txt"),
        "parent-4.before.txt" => include_str!("parent-4.before.txt"),
        "parent-5.before.txt" => include_str!("parent-5.before.txt"),
        "v12.before.rs.txt" => include_str!("v12.before.rs.txt"),
        _ => panic!("not a V13 full preimage"),
    }
}
pub(in super::super::super) fn before_source(source: &str, supplied: &str) -> String {
    assert_eq!(
        supplied,
        current(source),
        "actual complete caller source acquisition"
    );
    controller_transport::caller(source, supplied)
        .unwrap_or_else(|| original_before_source(source, supplied))
}

fn original_before_source(source: &str, supplied: &str) -> String {
    assert_eq!(
        supplied,
        current(source),
        "actual complete source acquisition"
    );
    let transported = controller_transport::original(source, supplied);
    let supplied = transported.as_str();
    let rows: Value = serde_json::from_str(include_str!("inverse.json")).unwrap();
    let rows = rows.as_array().unwrap();
    let records: Vec<_> = rows.iter().filter(|row| row["source"] == source).collect();
    assert_eq!(records.len(), 1);
    let row = records[0];
    assert_eq!(supplied.len() as u64, row["length"].as_u64().unwrap());
    assert_eq!(hash(supplied.as_bytes()), row["sha256"].as_str().unwrap());
    let mut result = supplied.to_owned();
    for edit in row["edits"].as_array().unwrap() {
        let after = edit["after"].as_str().unwrap();
        assert!(!after.is_empty());
        assert_eq!(result.matches(after).count(), 1);
        result = result.replacen(after, edit["before"].as_str().unwrap(), 1);
    }
    let expected = before(row["before_file"].as_str().unwrap());
    assert_eq!(
        expected.len() as u64,
        row["before_length"].as_u64().unwrap()
    );
    assert_eq!(
        hash(expected.as_bytes()),
        row["before_sha256"].as_str().unwrap()
    );
    assert_eq!(result, expected, "ENTIRE exact V13 predecessor");
    result
}

#[test]
fn complete_prior_v12_compiled_body_and_all_parent_inverses_remain_byte_exact() {
    let original = include_str!("v12.before.rs.txt");
    assert_eq!(
        before_source("logical-v12", current("logical-v12")),
        original
    );
    assert_eq!(original, include_str!("../main_e177_overlay_labels_v12.rs"));
    let rows: Value = serde_json::from_str(include_str!("inverse.json")).unwrap();
    assert_eq!(rows.as_array().unwrap().len(), 7);
    for row in rows.as_array().unwrap() {
        let source = row["source"].as_str().unwrap();
        original_before_source(source, current(source));
    }
    let python = current("docs/evidence/finite-source-overlay-v2-20261018/overlay_page_codecs.py");
    let old = before("parent-5.before.txt");
    assert_eq!(
        &python[..old.len()],
        old,
        "entire original Python codec remains executable"
    );
    assert_eq!(old.len(), 10905);
}

fn v14_fixture() -> Value {
    let mut value = fixture();
    let pin = value["replacements"][0]["old519_index"][1].clone();
    let tuple = pin.as_array().unwrap();
    value["replacements"][0]["old519_index"][2][0] = json!([0, tuple[0], tuple[1]]);
    value
}

#[test]
fn v14_preserves_entire_v13_v12_and_opaque_pin_metadata() {
    let logical = v14_fixture();
    let prior = encode_v13_logical(&logical).unwrap();
    let compact = encode_v14(&logical).unwrap();
    assert_eq!(expand_to_v13(&compact).unwrap(), prior);
    assert_eq!(
        expand_to_v12_logical(&expand_to_v13(&compact).unwrap()).unwrap(),
        logical
    );
    assert_eq!(compact["additions"], prior["additions"]);
    assert_eq!(
        compact["replacements"][0]["old519_index"][3],
        prior["replacements"][0]["old519_index"][3]
    );

    // The genuine over-cap 930 frontier remains an explicit unqualified codec
    // fixture; it is never handed to the standalone public V13 decoder.
    let v13_raw = include_bytes!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/e/48fd031ca82ec1114e93257ed8217d3848cf8de4b1308f195385bfa4cbf09831"
    ));
    let v12_raw = include_bytes!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/e/7fb2312ae816b3338d800acfcea9c99bb521ce9656a590c659e84183af8d4fec"
    ));
    assert_eq!(v13_raw.len(), 263377);
    assert_eq!(
        hash(v13_raw),
        "48fd031ca82ec1114e93257ed8217d3848cf8de4b1308f195385bfa4cbf09831"
    );
    assert_eq!(v12_raw.len(), 289964);
    assert_eq!(
        hash(v12_raw),
        "7fb2312ae816b3338d800acfcea9c99bb521ce9656a590c659e84183af8d4fec"
    );
    let v13: Value = serde_json::from_slice(v13_raw).unwrap();
    let v12: Value = serde_json::from_slice(v12_raw).unwrap();
    assert_eq!(expand_to_v12(&v13).unwrap_err(), "v13 same one-page cap");
    assert_eq!(
        canonical(&expand_to_v12_logical(&v13).unwrap()).as_slice(),
        v12_raw.as_slice()
    );
    let wire = encode_v14(&v12).unwrap();
    assert_eq!(canonical(&wire).len(), 246390);
    assert_eq!(
        canonical(&expand_to_v13(&wire).unwrap()).as_slice(),
        v13_raw.as_slice()
    );
    let actual = python_v14(&v12);
    assert_eq!(canonical(&actual["encoded"]), canonical(&wire));
    assert_eq!(canonical(&actual["v13"]).as_slice(), v13_raw.as_slice());
    assert_eq!(canonical(&actual["v12"]).as_slice(), v12_raw.as_slice());
    let current_raw = include_bytes!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/e/b512649322d5a7623b6fa516ae6edf922badf1e4c369ad959fd33a3608d33cdd"
    ));
    let current: Value = serde_json::from_slice(current_raw).unwrap();
    assert_eq!(current["schema"], SCHEMA_V14);
    assert_eq!(canonical(&current).as_slice(), current_raw.as_slice());
    assert_eq!(current_raw.len(), 249641);
    assert_eq!(hash(current_raw), "b512649322d5a7623b6fa516ae6edf922badf1e4c369ad959fd33a3608d33cdd");
    let policy_raw = include_bytes!(concat!(env!("CARGO_MANIFEST_DIR"), "/e/11eb976472f112c926f275ab28244f4cdb3e964178aa019c309b1a402062eea5"));
    assert_eq!(policy_raw.len(), 137177);
    assert_eq!(hash(policy_raw), "11eb976472f112c926f275ab28244f4cdb3e964178aa019c309b1a402062eea5");
    let historical_policy: Value = serde_json::from_slice(policy_raw).unwrap();
    assert_eq!(historical_policy["page"]["length"], current_raw.len());
    assert_eq!(historical_policy["page"]["sha256"], hash(current_raw));
    verify_current_v14(&current);
}

#[test]
fn v14_rejects_malformed_alias_unused_noncanonical_and_unknown_envelopes() {
    let compact = encode_v14(&v14_fixture()).unwrap();
    for value in [
        json!(true),
        json!(false),
        json!(-1),
        json!(u64::MAX),
        json!(0.5),
        json!(null),
        json!("0"),
        json!([]),
        json!({}),
    ] {
        let mut bad = compact.clone();
        bad["replacements"][0]["old519_index"][1] = json!([value]);
        assert!(expand_to_v13(&bad).is_err());
    }
    let mut bad = compact.clone();
    bad["pin_tails"]
        .as_array_mut()
        .unwrap()
        .push(compact["pin_tails"][0].clone());
    assert_eq!(
        expand_to_v13(&bad).unwrap_err(),
        "v14 duplicate declaration"
    );
    let mut bad = compact.clone();
    bad["pin_tails"]
        .as_array_mut()
        .unwrap()
        .push(json!([999, 0]));
    assert_eq!(
        expand_to_v13(&bad).unwrap_err(),
        "v14 declaration requires repeated use"
    );
    let mut bad = compact.clone();
    bad["pin_tails"]
        .as_array_mut()
        .unwrap()
        .insert(0, json!([999, 0]));
    bad["replacements"][0]["old519_index"][1] = json!([1]);
    assert_eq!(
        expand_to_v13(&bad).unwrap_err(),
        "v14 noncanonical first-use order"
    );
    let mut bad = compact.clone();
    bad["replacements"][0]["old519_index"][1] = compact["pin_tails"][0].clone();
    assert_eq!(
        expand_to_v13(&bad).unwrap_err(),
        "v14 uninterned or aliased tail"
    );
    for value in [
        json!(null),
        json!([]),
        json!([true, 0]),
        json!([-1, 0]),
        json!([SOURCE_CAP + 1, 0]),
        json!([1, true]),
        json!([1]),
        json!([1, 0, "e/a", 4]),
    ] {
        let mut bad = compact.clone();
        bad["pin_tails"][0] = value;
        assert!(expand_to_v13(&bad).is_err());
    }
    let mut bad = compact.clone();
    bad["unknown"] = json!(null);
    assert_eq!(expand_to_v13(&bad).unwrap_err(), "v14 envelope");
    let mut bad = compact.clone();
    bad.as_object_mut().unwrap().remove("pin_tails");
    assert_eq!(expand_to_v13(&bad).unwrap_err(), "v14 envelope");
    let mut bad = compact.clone();
    bad["digest_table"][0] = json!(format!("{}B", "A".repeat(42)));
    assert_eq!(expand_to_v13(&bad).unwrap_err(), "v13 digest spelling");
}

#[test]
fn v14_keeps_public_v13_wire_and_complete_source_bounds_independent() {
    let mut logical = v14_fixture();
    logical["additions"][0]["extra"]["payload"] = json!("x".repeat(PAGE_CAP));
    assert_eq!(encode_v13(&logical).unwrap_err(), "v13 same one-page cap");
    let prior = encode_v13_logical(&logical).unwrap();
    assert_eq!(expand_to_v12(&prior).unwrap_err(), "v13 same one-page cap");
    let mut compact = encode_v14(&v14_fixture()).unwrap();
    compact["predecessor"] = json!("x".repeat(PAGE_CAP));
    assert_eq!(
        expand_to_v13(&compact).unwrap_err(),
        "v14 same one-page cap"
    );
    logical["additions"][0]["extra"]["payload"] = json!("x".repeat(SOURCE_CAP));
    assert_eq!(encode_v14(&logical).unwrap_err(), "v13 logical input bound");
}

#[test]
fn v14_rejects_expanded_tail_amplification_before_publication() {
    let mut compact = encode_v14(&v14_fixture()).unwrap();
    compact["pin_tails"] = json!([[7, 0, format!("e/{}", "x".repeat(100000))]]);
    let tuple = compact["replacements"][0]["old519_index"]
        .as_array_mut()
        .unwrap();
    tuple[1] = json!([0]);
    tuple[2] = Value::Array((0..100).map(|_| json!([0, 0])).collect());
    tuple[3] = Value::Null;
    assert!(canonical(&compact).len() <= PAGE_CAP);
    assert_eq!(expand_to_v13(&compact).unwrap_err(), "v14 expanded bound");
}

fn python_v14(full: &Value) -> Value {
    use std::io::{Read, Write};
    use std::process::{Command, Stdio};
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let mut child = Command::new("python3").args(["-B", "-c", r#"
from pathlib import Path
import runpy,sys,json
root=Path(sys.argv[1])
n=runpy.run_path(str(root/'docs/evidence/finite-source-overlay-v2-20261018/overlay_page.py'),run_name='v14_owning_parity')
full=json.load(sys.stdin)
encoded=n['encode_v14'](full)
v13=n['expand_v14_pin_tails'](encoded)
v12=n['_expand_v13_logical_digest_table'](v13)
sys.stdout.buffer.write(n['canonical']({'encoded':encoded,'v13':v13,'v12':v12,'logical':n['_expand_v12_logical_view_labels'](v12)}))
"#]).arg(root).stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::inherit())
        .spawn().expect("V14 parity requires standard-library Python 3");
    child
        .stdin
        .take()
        .unwrap()
        .write_all(&canonical(full))
        .unwrap();
    let mut bytes = Vec::new();
    let read = child
        .stdout
        .take()
        .unwrap()
        .take((SOURCE_CAP + 1) as u64)
        .read_to_end(&mut bytes);
    let status = child.wait().unwrap();
    read.unwrap();
    assert!(status.success(), "V14 Python parity failed: {status}");
    assert!(bytes.len() <= SOURCE_CAP);
    serde_json::from_slice(&bytes).unwrap()
}

pub(in super::super::super) fn verify_current_v14(page: &Value) {
    assert_eq!(page["schema"], SCHEMA_V14);
    assert!(canonical(page).len() <= PAGE_CAP);
    let v13 = expand_to_v13(page).unwrap();
    let v12 = expand_to_v12_logical(&v13).unwrap();
    assert!(canonical(&v13).len() <= SOURCE_CAP);
    assert!(canonical(&v12).len() <= SOURCE_CAP);
    assert_eq!(canonical(&encode_v14(&v12).unwrap()), canonical(page));
    let logical = logical_v12::expand_to_v11(&v12).unwrap();
    let actual = python_v14(&v12);
    assert_eq!(canonical(&actual["encoded"]), canonical(page));
    assert_eq!(canonical(&actual["v13"]), canonical(&v13));
    assert_eq!(canonical(&actual["v12"]), canonical(&v12));
    assert_eq!(canonical(&actual["logical"]), canonical(&logical));
}

fn v15_fixture() -> Value {
    let mut value = v14_fixture();
    value["additions"][0]["source"] = (SOURCE_PREFIXES[0].to_owned() + "unicode-λ.rs").into();
    value["additions"][0]["extra"]["source"] = "0:opaque".into();
    value["replacements"][0]["old519_index"][3][0]["source"] = (SOURCE_PREFIXES[2].to_owned() + "a.rs").into();
    value["replacements"][0]["old519_index"][3][1]["source"] = "0:opaque".into();
    value
}
#[test]
fn v15_preserves_entire_v14_v13_v12_and_opaque_source_metadata() {
    assert_eq!(SOURCE_PREFIXES.len(), V15_CODES.len());
    let unique: BTreeSet<_> = SOURCE_PREFIXES.iter().collect();
    assert_eq!(unique.len(), SOURCE_PREFIXES.len());
    let v12 = v15_fixture();
    let v14 = encode_v14(&v12).unwrap();
    let wire = encode_v15(&v12).unwrap();
    assert_eq!(expand_to_v14(&wire).unwrap(), v14);
    let v13 = expand_to_v13_logical(&v14).unwrap();
    assert_eq!(expand_to_v12_logical(&v13).unwrap(), v12);
    assert_eq!(wire["additions"][0]["extra"], v14["additions"][0]["extra"]);
    assert_eq!(wire["replacements"][0]["old519_index"][3][1], v14["replacements"][0]["old519_index"][3][1]);
    let actual = python_v15(&v12);
    assert_eq!(canonical(&actual["encoded"]), canonical(&wire));
    assert_eq!(canonical(&actual["v14"]), canonical(&v14));
}
#[test]
fn v15_rejects_malformed_uninterned_noncanonical_and_unknown_envelopes() {
    let wire = encode_v15(&v15_fixture()).unwrap();
    for source in [json!(false), json!(null), json!([0, "x"]), json!("i:x"), json!("0:"), json!("0:/x"), json!("0:../x"), json!("0:x\\y"), json!("0:x:y"), json!(SOURCE_PREFIXES[0].to_owned()+"x"), json!("1:ordinary_plan_history/comparison_owner/x")] {
        let mut bad = wire.clone();bad["additions"][0]["source"] = source;
        assert!(expand_to_v14(&bad).is_err(), "accepted {bad}");
    }
    let mut bad=wire.clone();bad["source_prefixes"]=json!([]);
    assert_eq!(expand_to_v14(&bad).unwrap_err(), "v15 envelope");
    let mut bad=wire;bad["schema"]=SCHEMA_V14.into();
    assert_eq!(expand_to_v14(&bad).unwrap_err(), "v15 envelope");
}
#[test]
fn v15_keeps_public_v14_cap_and_tail_amplification_bounds_independent() {
    let mut bad=encode_v15(&v15_fixture()).unwrap();
    bad["pin_tails"]=json!([[7, 0, format!("e/{}", "x".repeat(100000))]]);
    let tuple=bad["replacements"][0]["old519_index"].as_array_mut().unwrap();
    tuple[1]=json!([0]);tuple[2]=Value::Array((0..100).map(|_|json!([0,0])).collect());tuple[3]=Value::Null;
    assert!(canonical(&bad).len()<=PAGE_CAP);
    assert_eq!(expand_to_v14(&bad).unwrap_err(), "v14 expanded bound");
    let mut large=encode_v15(&v15_fixture()).unwrap();large["predecessor"]="x".repeat(PAGE_CAP).into();
    assert_eq!(expand_to_v14(&large).unwrap_err(), "v15 same one-page cap");
}
#[test]
fn v15_actual_overcap_frontier_and_current_whole_page_keep_python_rust_parity() {
    let v14_raw=include_bytes!(concat!(env!("CARGO_MANIFEST_DIR"), "/e/6a06db716fe5fa91c8233ade79203b29457dadb46b6e1083ecfe6b0df3952d61"));
    assert_eq!(v14_raw.len(), 280154); assert_eq!(hash(v14_raw), "6a06db716fe5fa91c8233ade79203b29457dadb46b6e1083ecfe6b0df3952d61");
    let v13_raw=include_bytes!(concat!(env!("CARGO_MANIFEST_DIR"), "/e/8b843ea5514147508b8a1676e1e1b6e8078d0b5e0f26af744a61814351f307e1"));
    assert_eq!(v13_raw.len(), 297229); assert_eq!(hash(v13_raw), "8b843ea5514147508b8a1676e1e1b6e8078d0b5e0f26af744a61814351f307e1");
    let v12_raw=include_bytes!(concat!(env!("CARGO_MANIFEST_DIR"), "/e/571ec88fa5e7037cae6d7b6e2ea592acaf3c97380a07a07cafe366bf923d295b"));
    assert_eq!(v12_raw.len(), 327382); assert_eq!(hash(v12_raw), "571ec88fa5e7037cae6d7b6e2ea592acaf3c97380a07a07cafe366bf923d295b");
    let v14:Value=serde_json::from_slice(v14_raw).unwrap();
    let v12:Value=serde_json::from_slice(v12_raw).unwrap();
    assert_eq!(expand_to_v13(&v14).unwrap_err(), "v14 same one-page cap");
    let wire=encode_v15(&v12).unwrap();
    assert_eq!(canonical(&expand_to_v14(&wire).unwrap()).as_slice(), v14_raw.as_slice());
    assert_eq!(canonical(&expand_to_v13_logical(&v14).unwrap()).as_slice(), v13_raw.as_slice());
    assert_eq!(canonical(&expand_to_v12_logical(&serde_json::from_slice(v13_raw).unwrap()).unwrap()).as_slice(), v12_raw.as_slice());
    assert_eq!(canonical(&wire).len(), 260553);
    let actual=python_v15(&v12);
    assert_eq!(canonical(&actual["encoded"]), canonical(&wire));
    assert_eq!(canonical(&actual["v14"]).as_slice(), v14_raw.as_slice());
    assert_eq!(canonical(&actual["v13"]).as_slice(), v13_raw.as_slice());
    assert_eq!(canonical(&actual["v12"]).as_slice(), v12_raw.as_slice());
    let current_raw=include_bytes!(concat!(env!("CARGO_MANIFEST_DIR"), "/docs/evidence/finite-source-overlay-v2-20261018/page.json"));
    let current:Value=serde_json::from_slice(current_raw).unwrap();
    assert_eq!(canonical(&current).as_slice(), current_raw.as_slice());
    let current = retain_v16_page_for_v17(retain_current_v17_for_v18(current));
    if current["schema"]==SCHEMA_V16 { verify_current_v16(&current); }
    else { verify_current_v15(&current); }
}
fn python_v15(full: &Value) -> Value {
    use std::io::{Read,Write}; use std::process::{Command,Stdio};
    let mut child=Command::new("python3").args(["-B","-c",r#"
import json,runpy,sys
from pathlib import Path
root=Path(sys.argv[1]);n=runpy.run_path(str(root/'docs/evidence/finite-source-overlay-v2-20261018/overlay_page.py'),run_name='v15_owning_parity')
full=json.load(sys.stdin);encoded=n['encode_v15'](full);v14=n['expand_v15_source_paths'](encoded);v13=n['_expand_v14_logical_pin_tails'](v14);v12=n['_expand_v13_logical_digest_table'](v13)
sys.stdout.buffer.write(n['canonical']({'encoded':encoded,'v14':v14,'v13':v13,'v12':v12,'logical':n['_expand_v12_logical_view_labels'](v12)}))
"#]).arg(env!("CARGO_MANIFEST_DIR")).stdin(Stdio::piped()).stdout(Stdio::piped()).spawn().unwrap();
    child.stdin.take().unwrap().write_all(&canonical(full)).unwrap();
    let mut raw=Vec::new();let read=child.stdout.take().unwrap().take((SOURCE_CAP+1) as u64).read_to_end(&mut raw);let status=child.wait().unwrap();read.unwrap();assert!(status.success());assert!(raw.len()<=SOURCE_CAP);
    serde_json::from_slice(&raw).unwrap()
}
pub(in super::super::super) fn verify_current_v15(page:&Value) {
    assert_eq!(page["schema"],SCHEMA_V15);assert!(canonical(page).len()<=PAGE_CAP);
    let v14=expand_to_v14(page).unwrap();let v13=expand_to_v13_logical(&v14).unwrap();let v12=expand_to_v12_logical(&v13).unwrap();
    let logical=logical_v12::expand_to_v11(&v12).unwrap();
    assert_eq!(canonical(&encode_v15(&v12).unwrap()),canonical(page));
    let actual=python_v15(&v12);
    for (key,value) in [("encoded",page),("v14",&v14),("v13",&v13),("v12",&v12),("logical",&logical)] {assert_eq!(canonical(&actual[key]),canonical(value));}
}

fn v16_fixture() -> Value {
    let mut full = v15_fixture();
    let known = json!({"source":"src/source_reflows_merge/v16-λ.rs", "compiled":false,
        "current":{"length":7,"sha256":"A".repeat(43),"unknown":{"source_row":[false]}},"views":[]});
    full["additions"]
        .as_array_mut()
        .unwrap()
        .push(known.clone());
    full["replacements"][0]["old519_index"][3]
        .as_array_mut()
        .unwrap()
        .push(known);
    full
}
#[test]
fn v16_preserves_entire_v15_and_unknown_literal_row_fallback() {
    let full = v16_fixture();
    let prior = encode_v15(&full).unwrap();
    let wire = encode_v16(&full).unwrap();
    assert_eq!(canonical(&expand_to_v15(&wire).unwrap()), canonical(&prior));
    assert_eq!(
        wire["additions"][0], prior["additions"][0],
        "unknown-key row stays whole and literal"
    );
    assert_eq!(wire["additions"][1]["source_row"][1], false);
    assert_eq!(
        wire["additions"][1]["source_row"][2],
        prior["additions"][1]["current"]
    );
    assert_eq!(
        wire["replacements"][0]["old519_index"][3][1],
        prior["replacements"][0]["old519_index"][3][1]
    );
    for metadata in [
        json!(17),
        json!([false,{"source_row":null}]),
        json!({"source":"0:opaque","compiled":0}),
    ] {
        for fragments in [
            json!({"source_row":[null]}),
            json!([{"source_row":[null]}]),
            json!(17),
        ] {
            let mut unknown = full.clone();
            unknown["additions"][1]["source_row"] = metadata.clone();
            unknown["additions"][1]["another_unknown"] = json!({"fragments":false});
            // A fragment row's nested fragments value is opaque in original V15.
            unknown["replacements"][0]["old519_index"][3][2]["fragments"] = fragments;
            let before = encode_v15(&unknown).unwrap();
            let after = encode_v16(&unknown).unwrap();
            assert_eq!(after["additions"][1], before["additions"][1]);
            assert_eq!(
                canonical(&expand_to_v15(&after).unwrap()),
                canonical(&before)
            );
        }
    }
    let mut nested = full;
    nested["additions"][1]["fragments"] = json!([nested["additions"][1].clone()]);
    let nested_prior = encode_v15(&nested).unwrap();
    assert_eq!(
        expand_to_v15(&encode_v16(&nested).unwrap()).unwrap(),
        nested_prior
    );
}
#[test]
fn v16_rejects_row_alias_arity_types_tags_and_inherited_amplification() {
    let wire = encode_v16(&v16_fixture()).unwrap();
    let prior = expand_to_v15(&wire).unwrap();
    for row in [
        json!({"source_row":[]}),
        json!({"source_row":["9:x",true,[],[],null,null]}),
        json!({"source_row":[false,true,[],[]]}),
        json!({"source_row":["9:x",1,[],[]]}),
        json!({"source_row":["9:x",true,null,[]]}),
        json!({"source_row":["9:x",true,[],null]}),
        json!({"source_row":["9:x",true,[],[],false]}),
        prior["additions"][1].clone(),
    ] {
        let mut bad = wire.clone();
        bad["additions"][1] = row;
        assert!(expand_to_v15(&bad).is_err());
    }
    let mut bad = wire.clone();
    bad["unknown"] = json!(null);
    assert_eq!(expand_to_v15(&bad).unwrap_err(), "v15 envelope");
    let mut bad = wire.clone();
    bad["predecessor"] = "x".repeat(PAGE_CAP).into();
    assert_eq!(expand_to_v15(&bad).unwrap_err(), "v16 same one-page cap");
    let mut bad = wire;
    bad["pin_tails"] = json!([[7, 0, format!("e/{}", "x".repeat(100000))]]);
    let tuple = bad["replacements"][0]["old519_index"]
        .as_array_mut()
        .unwrap();
    tuple[1] = json!([0]);
    tuple[2] = Value::Array((0..100).map(|_| json!([0, 0])).collect());
    tuple[3] = Value::Null;
    assert!(canonical(&bad).len() <= PAGE_CAP);
    assert_eq!(expand_to_v15(&bad).unwrap_err(), "v14 expanded bound");
    // Every known row adds only fixed key punctuation: no dictionary-reference amplification.
    assert_eq!(row_overhead(5) - row_overhead(4), "\"fragments\":".len());
}
#[test]
fn v16_keeps_public_v15_overcap_refusal_and_complete_logical_preimage() {
    let mut prior = encode_v15(&v16_fixture()).unwrap();
    let row = prior["additions"][1].clone();
    while canonical(&prior).len() <= PAGE_CAP {
        prior["additions"].as_array_mut().unwrap().push(row.clone());
    }
    let v14 = expand_to_v14_logical(&prior).unwrap();
    assert_eq!(expand_to_v14(&prior).unwrap_err(), "v15 same one-page cap");
    assert_eq!(
        encode_v15_sources(&v14).unwrap_err(),
        "v15 same one-page cap"
    );
    let wire = encode_v16_rows(&prior).unwrap();
    assert!(canonical(&wire).len() <= PAGE_CAP);
    assert_eq!(canonical(&expand_to_v15(&wire).unwrap()), canonical(&prior));
    assert!(canonical(&wire).len() < canonical(&prior).len());
}
#[test]
fn v16_current_whole_page_and_python_rust_transport_are_equal() {
    let full = v16_fixture();
    let wire = encode_v16(&full).unwrap();
    let actual = python_v16(&full);
    assert_eq!(canonical(&actual["encoded"]), canonical(&wire));
    assert_eq!(
        canonical(&actual["v15"]),
        canonical(&expand_to_v15(&wire).unwrap())
    );
    let raw = include_bytes!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/docs/evidence/finite-source-overlay-v2-20261018/page.json"
    ));
    let current: Value = serde_json::from_slice(raw).unwrap();
    assert_eq!(canonical(&current).as_slice(), raw.as_slice());
    let current = retain_v16_page_for_v17(retain_current_v17_for_v18(current));
    if current["schema"] == SCHEMA_V16 {
        verify_current_v16(&current);
    } else {
        assert_eq!(current["schema"], SCHEMA_V15);
        verify_current_v16(&encode_v16_rows(&current).unwrap());
    }
}
fn python_v16(full: &Value) -> Value {
    use std::io::{Read, Write};
    use std::process::{Command, Stdio};
    let mut child=Command::new("python3").args(["-B","-c",r#"
import json,runpy,sys
from pathlib import Path
n=runpy.run_path(str(Path(sys.argv[1])/'docs/evidence/finite-source-overlay-v2-20261018/overlay_page.py'),run_name='v16_owning_parity')
full=json.load(sys.stdin);wire=n['encode_v16'](full);v15=n['expand_v16_source_rows'](wire)
sys.stdout.buffer.write(n['canonical']({'encoded':wire,'v15':v15}))
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
pub(in super::super::super) fn verify_current_v16(page: &Value) {
    assert_eq!(page["schema"], SCHEMA_V16);
    assert!(canonical(page).len() <= PAGE_CAP);
    let v15 = expand_to_v15(page).unwrap();
    let v14 = expand_to_v14_logical(&v15).unwrap();
    let v12 = expand_to_v12_logical(&expand_to_v13_logical(&v14).unwrap()).unwrap();
    assert_eq!(canonical(&encode_v16(&v12).unwrap()), canonical(page));
    let actual = python_v16(&v12);
    assert_eq!(canonical(&actual["encoded"]), canonical(page));
    assert_eq!(canonical(&actual["v15"]), canonical(&v15));
}

fn retain_v16_page_for_v17(current: Value) -> Value {
    if current["schema"] != SCHEMA_V17 {
        return current;
    }
    let raw = include_bytes!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/e/4434b53d2afd56bf387f57712ec627cb0af7e3439d81d00c6e39d147ee6b4f93"
    ));
    assert_eq!(raw.len(), 260344);
    assert_eq!(
        hash(raw),
        "4434b53d2afd56bf387f57712ec627cb0af7e3439d81d00c6e39d147ee6b4f93"
    );
    let prior: Value = serde_json::from_slice(raw).unwrap();
    assert_eq!(prior["schema"], SCHEMA_V16);
    assert_eq!(canonical(&prior).as_slice(), raw.as_slice());
    prior
}
#[test]
fn v17_preserves_entire_v15_opaque_metadata_and_one_level_fragments() {
    let full = v16_fixture();
    let prior = encode_v15(&full).unwrap();
    let wire = encode_v17(&full).unwrap();
    assert_eq!(expand_positional_to_v15(&wire).unwrap(), prior);
    assert!(wire["additions"][1].is_array());
    assert_eq!(
        canonical(&wire).len() + 2 * (15 + 4),
        canonical(&encode_v16(&full).unwrap()).len()
    );
    assert_eq!(wire["additions"][1][1], 0);
    for flag in [false, true] {
        let mut logical = prior.clone();
        logical["additions"][1]["compiled"] = flag.into();
        let encoded = encode_v17_rows(&logical).unwrap();
        assert_eq!(encoded["additions"][1][1].as_u64(), Some(u64::from(flag)));
        assert_eq!(expand_positional_to_v15(&encoded).unwrap(), logical);
    }
    for metadata in [
        json!(17),
        json!([false,{"source_row":null}]),
        json!({"source":"0:opaque","compiled":0}),
    ] {
        for fragments in [
            json!({"source_row":[null]}),
            json!([{"source_row":[null]}]),
            json!(17),
        ] {
            let mut unknown = full.clone();
            unknown["additions"][1]["source_row"] = metadata.clone();
            unknown["additions"][1]["another_unknown"] = json!({"fragments":false});
            unknown["replacements"][0]["old519_index"][3][2]["fragments"] = fragments;
            let old = encode_v15(&unknown).unwrap();
            let encoded = encode_v17(&unknown).unwrap();
            assert_eq!(encoded["additions"][1], old["additions"][1]);
            assert_eq!(
                canonical(&expand_positional_to_v15(&encoded).unwrap()),
                canonical(&old)
            );
        }
    }
    let mut nested = full;
    nested["additions"][1]["fragments"] = json!([nested["additions"][1].clone()]);
    assert_eq!(
        expand_positional_to_v15(&encode_v17(&nested).unwrap()).unwrap(),
        encode_v15(&nested).unwrap()
    );
}
#[test]
fn v17_refuses_closed_row_alias_types_and_inherited_amplification() {
    let wire = encode_v17(&v16_fixture()).unwrap();
    let prior = expand_positional_to_v15(&wire).unwrap();
    for row in [
        json!([]),
        json!(["9:x", 0, [], [], null, null]),
        json!([false, 0, [], []]),
        json!(["9:x", true, [], []]),
        json!(["9:x", false, [], []]),
        json!(["9:x", 2, [], []]),
        json!(["9:x", -1, [], []]),
        json!(["9:x", 0.0, [], []]),
        json!(["9:x", -0.0, [], []]),
        json!(["9:x", 1.0, [], []]),
        json!(["9:x", null, [], []]),
        json!(["9:x", "0", [], []]),
        json!(["9:x", 0, null, []]),
        json!(["9:x", 0, [], null]),
        json!(["9:x", 0, [], [], false]),
        json!({"source_row":["9:x",true,[],[]]}),
        prior["additions"][1].clone(),
    ] {
        let mut bad = wire.clone();
        bad["additions"][1] = row;
        assert!(expand_positional_to_v15(&bad).is_err());
    }
    // Parsing preserves JSON negative zero as a floating number; it is no integer alias.
    let negative_zero: Value = serde_json::from_str(r#"["9:x",-0,[],[]]"#).unwrap();
    assert!(negative_zero[1].as_u64().is_none());
    let mut bad = wire.clone();
    bad["additions"][1] = negative_zero;
    assert!(expand_positional_to_v15(&bad).is_err());
    let mut bad = wire.clone();
    bad["unknown"] = json!(null);
    assert_eq!(expand_positional_to_v15(&bad).unwrap_err(), "v15 envelope");
    let mut bad = wire.clone();
    bad["predecessor"] = "x".repeat(PAGE_CAP).into();
    assert_eq!(
        expand_positional_to_v15(&bad).unwrap_err(),
        "v17 same one-page cap"
    );
    let mut bad = wire;
    bad["pin_tails"] = json!([[7, 0, format!("e/{}", "x".repeat(100000))]]);
    let tuple = bad["replacements"][0]["old519_index"]
        .as_array_mut()
        .unwrap();
    tuple[1] = json!([0]);
    tuple[2] = Value::Array((0..100).map(|_| json!([0, 0])).collect());
    tuple[3] = Value::Null;
    assert!(canonical(&bad).len() <= PAGE_CAP);
    assert_eq!(
        expand_positional_to_v15(&bad).unwrap_err(),
        "v14 expanded bound"
    );
    assert_eq!(
        row_overhead(5) + b"{\"source_row\":[]}".len()
            - 2
            - (row_overhead(4) + b"{\"source_row\":[]}".len() - 2),
        "\"fragments\":".len()
    );
}
#[test]
fn v17_keeps_complete_overcap_v16_preimage_and_python_rust_parity() {
    let mut prior = encode_v15(&v16_fixture()).unwrap();
    let row = prior["additions"][1].clone();
    let mut v16 = prior.clone();
    transform_source_rows(&mut v16, false).unwrap();
    v16["schema"] = SCHEMA_V16.into();
    let base_bytes = canonical(&v16).len();
    assert!(base_bytes <= PAGE_CAP);
    // Existing additions are nonempty: each appended identical encoded row
    // contributes its JSON width plus one comma, exactly canonical(row).len().
    let row_bytes = canonical(&v16["additions"][1]).len();
    let extra = (PAGE_CAP - base_bytes) / row_bytes + 1;
    let additions = prior["additions"].as_array_mut().unwrap();
    additions.resize(additions.len() + extra, row);
    assert_eq!(
        encode_v16_rows(&prior).unwrap_err(),
        "v16 same one-page cap"
    );
    let wire = encode_v17_rows(&prior).unwrap();
    assert!(canonical(&wire).len() <= PAGE_CAP);
    assert_eq!(
        canonical(&expand_positional_to_v15(&wire).unwrap()),
        canonical(&prior)
    );
    let v12 = expand_to_v12_logical(
        &expand_to_v13_logical(&expand_to_v14_logical(&prior).unwrap()).unwrap(),
    )
    .unwrap();
    let actual = python_v17(&v12);
    assert_eq!(canonical(&actual["encoded"]), canonical(&wire));
    assert_eq!(canonical(&actual["v15"]), canonical(&prior));
    let raw = include_bytes!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/docs/evidence/finite-source-overlay-v2-20261018/page.json"
    ));
    let current: Value = serde_json::from_slice(raw).unwrap();
    assert_eq!(canonical(&current).as_slice(), raw.as_slice());
    let current = retain_current_v17_for_v18(current);
    if current["schema"] == SCHEMA_V17 {
        verify_current_v17(&current);
    } else {
        let v15 = if current["schema"] == SCHEMA_V16 {
            expand_to_v15(&current).unwrap()
        } else {
            assert_eq!(current["schema"], SCHEMA_V15);
            current
        };
        verify_current_v17(&encode_v17_rows(&v15).unwrap());
    }
}
fn python_v17(full: &Value) -> Value {
    use std::io::{Read, Write};
    use std::process::{Command, Stdio};
    let mut child = Command::new("python3").args(["-B","-c",r#"
import json,runpy,sys
from pathlib import Path
n=runpy.run_path(str(Path(sys.argv[1])/'docs/evidence/finite-source-overlay-v2-20261018/overlay_page.py'),run_name='v17_owning_parity')
full=json.load(sys.stdin);wire=n['encode_v17'](full);v15=n['expand_v17_source_rows'](wire)
sys.stdout.buffer.write(n['canonical']({'encoded':wire,'v15':v15}))
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
pub(in super::super::super) fn verify_current_v17(page: &Value) {
    assert_eq!(page["schema"], SCHEMA_V17);
    assert!(canonical(page).len() <= PAGE_CAP);
    let v15 = expand_positional_to_v15(page).unwrap();
    let v12 = expand_to_v12_logical(
        &expand_to_v13_logical(&expand_to_v14_logical(&v15).unwrap()).unwrap(),
    )
    .unwrap();
    assert_eq!(canonical(&encode_v17(&v12).unwrap()), canonical(page));
    let actual = python_v17(&v12);
    assert_eq!(canonical(&actual["encoded"]), canonical(page));
    assert_eq!(canonical(&actual["v15"]), canonical(&v15));
}

fn v18_fixture() -> Value {
    let mut full = v16_fixture();
    full["replacements"][0]["old519_index"][3] = serde_json::json!([
        {"canonical_source":519},{"canonical_source":520},
        {"canonical_source":"opaque","unknown":{"canonical_source":519}}]);
    full["additions"][1]["opaque_reference"] = serde_json::json!({"canonical_source":519});
    full
}
#[test]
fn v18_entire_preimage_keeps_unknown_fields_one_level_domains_and_order() {
    let logical = encode_v15(&v18_fixture()).unwrap();
    let legacy = encode_v17_rows(&logical).unwrap();
    let wire = typed_refs_v18::encode_rows(&logical).unwrap();
    assert_eq!(wire["replacements"][0]["old519_index"][3][0], 519);
    assert_eq!(wire["replacements"][0]["old519_index"][3][1], 520);
    assert_eq!(
        canonical(&typed_refs_v18::expand_to_v15(&wire).unwrap()),
        canonical(&logical)
    );
    assert_eq!(
        canonical(&encode_v17_rows(&logical).unwrap()),
        canonical(&legacy)
    );
    assert_eq!(canonical(&legacy).len() - canonical(&wire).len(), 42);
    assert_eq!(
        wire["additions"][1]["opaque_reference"],
        logical["additions"][1]["opaque_reference"]
    );
    let mut nested = logical.clone();
    nested["additions"][1]["fragments"] = serde_json::json!([{ "source":"nested.rs", "compiled":true,"current":[1,"A".repeat(43)],"views":[],"fragments":[519,{"canonical_source":519}]}]);
    assert_eq!(
        typed_refs_v18::encode_rows(&nested).unwrap_err(),
        "v13 uninterned or aliased digest"
    );
    nested["additions"][1]["fragments"][0]["current"] = serde_json::json!([1, 0]);
    assert_eq!(
        typed_refs_v18::encode_rows(&nested).unwrap_err(),
        "v14 uninterned or aliased tail"
    );
    let admitted_tail = logical["replacements"][0]["old519_index"][1].clone();
    assert_eq!(admitted_tail.as_array().unwrap().len(), 1);
    assert!(admitted_tail[0].is_u64());
    nested["additions"][1]["fragments"][0]["current"] = admitted_tail;
    assert_eq!(
        typed_refs_v18::expand_to_v15(&typed_refs_v18::encode_rows(&nested).unwrap()).unwrap(),
        nested
    );
}
#[test]
fn v18_rejects_aliases_bad_indices_wrong_slots_and_truncation() {
    let wire = typed_refs_v18::encode(&v18_fixture()).unwrap();
    let total = 519 + wire["additions"].as_array().unwrap().len();
    for bad in [
        serde_json::json!(true),
        serde_json::json!(false),
        serde_json::json!(-1),
        serde_json::json!(0),
        serde_json::json!(518),
        serde_json::json!(519.0),
        serde_json::json!(-0.0),
        serde_json::json!(total),
        serde_json::json!({"canonical_source":519}),
    ] {
        let mut changed = wire.clone();
        changed["replacements"][0]["old519_index"][3][0] = bad;
        assert!(typed_refs_v18::expand_to_v15(&changed).is_err());
    }
    let mut changed = wire.clone();
    changed["additions"][0] = 519.into();
    assert!(typed_refs_v18::expand_to_v15(&changed).is_err());
    changed = wire.clone();
    changed["replacements"][0]["old519_index"] = serde_json::json!([0, [], []]);
    assert!(typed_refs_v18::expand_to_v15(&changed).is_err());
    changed = wire;
    changed["unknown"] = 519.into();
    assert!(typed_refs_v18::expand_to_v15(&changed).is_err());
}
#[test]
fn v18_exact_short_expanded_bound_and_wire_cap_are_independent() {
    let wire = typed_refs_v18::encode(&v18_fixture()).unwrap();
    let logical = typed_refs_v18::expand_to_v15(&wire).unwrap();
    let exact = canonical(&logical).len();
    assert_eq!(typed_refs_v18::preflight(&wire, exact).unwrap(), exact);
    assert_eq!(
        typed_refs_v18::preflight(&wire, exact - 1).unwrap_err(),
        "v18 expanded bound"
    );
    let mut large = wire;
    large["predecessor"] = "x".repeat(PAGE_CAP).into();
    assert_eq!(
        typed_refs_v18::expand_to_v15(&large).unwrap_err(),
        "v18 same one-page cap"
    );
}
fn python_v18(full: &Value, hostile: &[Value]) -> Value {
    use std::io::{Read, Write};
    use std::process::{Command, Stdio};
    let mut child=Command::new("python3").args(["-B","-c",r#"
import json,runpy,sys
from pathlib import Path
n=runpy.run_path(str(Path(sys.argv[1])/'docs/evidence/finite-source-overlay-v2-20261018/overlay_page.py'),run_name='v18_owning_parity')
value=json.load(sys.stdin);wire=n['encode_v18'](value['full']);v15=n['expand_v18_fragment_refs'](wire)
refused=[]
for bad in value['hostile']:
 try:n['expand_v18_fragment_refs'](bad)
 except (AssertionError,KeyError,TypeError,ValueError):refused.append(True)
 else:refused.append(False)
sys.stdout.buffer.write(n['canonical']({'encoded':wire,'v15':v15,'refused':refused}))
"#]).arg(env!("CARGO_MANIFEST_DIR")).stdin(Stdio::piped()).stdout(Stdio::piped()).spawn().unwrap();
    child
        .stdin
        .take()
        .unwrap()
        .write_all(&canonical(
            &serde_json::json!({"full":full,"hostile":hostile}),
        ))
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
#[test]
fn v18_complete_current_page_and_python_rust_transport_match() {
    let raw = include_bytes!(concat!(env!("CARGO_MANIFEST_DIR"),
        "/docs/evidence/finite-source-overlay-v2-20261018/page.json"));
    let current: Value = serde_json::from_slice(raw).unwrap();
    assert_eq!(canonical(&current).as_slice(),raw.as_slice());
    verify_current_v18(&current);
}
pub(in super::super::super) fn verify_current_v18(current: &Value) {
    if current["schema"] != super::super::super::compiled_successor_v19::SCHEMA {
        return verify_current_v18_before_v19(current);
    }
    authenticate_current_page_for_retained_controls(current);
    let (mut policy, _) = super::super::super::load_current().unwrap();
    let before = super::super::super::super::packet_before_overlay();
    let expected = super::super::super::compose_values(before.clone(), &policy, current).unwrap();
    // Only the private singleton base is derived, after full V19 validation.
    let mut derived = before.clone();
    assert_eq!(derived["rows"][389]["source"], "src/workspace_graph/expected_projection/borrowed_imports.rs");
    assert_eq!(derived["rows"][389]["compiled"], false);
    derived["rows"][389]["compiled"] = true.into();
    let mut inverse = derived.clone();
    inverse["rows"][389]["compiled"] = false.into();
    assert_eq!(canonical(&inverse), canonical(&before));
    policy["predecessor"]["logical_sha256"] = hash(&canonical(&derived)).into();
    let mut inner = current["page"].clone();
    inner["predecessor"] = policy["predecessor"].clone();
    let indexed = inner["schema"] == typed_refs_v18::SCHEMA_V19_INDEXED;
    assert!(indexed || inner["schema"] == typed_refs_v18::SCHEMA_V18);
    let decode = |page: &Value| if indexed { typed_refs_v18::expand_indexed_to_v15(page) } else { typed_refs_v18::expand_to_v15(page) };
    let logical = decode(&inner).unwrap();
    let v12 = expand_to_v12_logical(
        &expand_to_v13_logical(&expand_to_v14_logical(&logical).unwrap()).unwrap(),
    ).unwrap();
    let wire = if indexed { typed_refs_v18::encode_indexed(&v12) } else { typed_refs_v18::encode(&v12) }.unwrap();
    let actual = if indexed { typed_refs_v18::python_indexed(&v12) } else { python_v18(&v12, &[]) };
    assert_eq!(canonical(&actual["encoded"]), canonical(&wire));
    assert_eq!(canonical(&actual["v15"]), canonical(&logical));
    assert_eq!(canonical(&decode(&wire).unwrap()), canonical(&logical));
    assert_eq!(canonical(&inner), canonical(&wire));
    let compose = |page: &Value| if indexed {
        let expanded = typed_refs_v18::expand_v19_indexed(&derived, &policy, page)?;
        super::super::super::compose_values_v2(derived.clone(), &policy, &expanded)
    } else { super::super::super::compose_values(derived.clone(), &policy, page) };
    let original = compose(&inner).unwrap();
    let restored = compose(&wire).unwrap();
    assert_eq!(canonical(&restored), canonical(&original));
    assert_eq!(canonical(&original), canonical(&expected));
}
fn verify_current_v18_before_v19(current: &Value) {
    authenticate_current_page_for_retained_controls(current);
    let logical = if current["schema"] == typed_refs_v18::SCHEMA_V18 {
        typed_refs_v18::expand_to_v15(current).unwrap()
    } else {
        assert_eq!(current["schema"], SCHEMA_V17);
        expand_positional_to_v15(current).unwrap()
    };
    let v12 = expand_to_v12_logical(
        &expand_to_v13_logical(&expand_to_v14_logical(&logical).unwrap()).unwrap(),
    )
    .unwrap();
    let wire = typed_refs_v18::encode(&v12).unwrap();
    let actual = python_v18(&v12, &[]);
    assert_eq!(canonical(&actual["encoded"]), canonical(&wire));
    assert_eq!(canonical(&actual["v15"]), canonical(&logical));
    assert_eq!(
        canonical(&typed_refs_v18::expand_to_v15(&wire).unwrap()),
        canonical(&logical)
    );
    if current["schema"] == typed_refs_v18::SCHEMA_V18 {
        assert_eq!(canonical(current), canonical(&wire));
    }
    let before = super::super::super::super::packet_before_overlay();
    let (policy, _) = super::super::super::load().unwrap();
    let original = super::super::super::compose_values(before.clone(), &policy, current).unwrap();
    let restored = super::super::super::compose_values(before, &policy, &wire).unwrap();
    assert_eq!(canonical(&restored), canonical(&original));
}

#[test]
fn v18_python_rust_hostile_reference_decisions_match() {
    let full = v18_fixture();
    let wire = typed_refs_v18::encode(&full).unwrap();
    let mut hostile = Vec::new();
    for bad in [
        serde_json::json!(true),
        serde_json::json!(-1),
        serde_json::json!(518),
        serde_json::json!(519.0),
        serde_json::json!(-0.0),
        serde_json::json!(u64::MAX),
        serde_json::json!({"canonical_source":519}),
    ] {
        let mut value = wire.clone();
        value["replacements"][0]["old519_index"][3][0] = bad;
        hostile.push(value);
    }
    let actual = python_v18(&full, &hostile);
    let refused: Vec<Value> = hostile
        .iter()
        .map(|p| typed_refs_v18::expand_to_v15(p).is_err().into())
        .collect();
    assert_eq!(actual["refused"], Value::Array(refused));
    assert!(actual["refused"]
        .as_array()
        .unwrap()
        .iter()
        .all(|v| v.as_bool() == Some(true)));
}


// Historical controls acquire the actual live wire first, then an exact
// independently pinned full predecessor. Their evidence remains historical.
fn authenticate_current_page_for_retained_controls(current: &Value) {
    if current["schema"] == super::super::super::compiled_successor_v19::SCHEMA {
        let (policy, loaded) = super::super::super::load_current().unwrap();
        assert_eq!(canonical(current), canonical(&loaded));
        let before = super::super::super::super::packet_before_overlay();
        super::super::super::compose_values(before, &policy, current).unwrap();
        return;
    }
    assert!(current["schema"] == SCHEMA_V17 || current["schema"] == typed_refs_v18::SCHEMA_V18);
    let compiled = include_bytes!(concat!(env!("CARGO_MANIFEST_DIR"),
        "/docs/evidence/finite-source-overlay-v2-20261018/page.json"));
    assert!(compiled.len() <= PAGE_CAP);
    assert_eq!(canonical(current).as_slice(), compiled.as_slice());
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("docs/evidence/finite-source-overlay-v2-20261018/page.json");
    let physical = super::super::super::held_4a6eb(&path,compiled.len(),&hash(compiled)).unwrap();
    assert_eq!(physical.as_bytes(),compiled.as_slice());
    if current["schema"] == typed_refs_v18::SCHEMA_V18 {
        typed_refs_v18::expand_to_v15(current).unwrap();
    } else {
        expand_positional_to_v15(current).unwrap();
    }
}
fn retain_current_v17_for_v18(current: Value) -> Value {
    if current["schema"] != typed_refs_v18::SCHEMA_V18 && current["schema"] != super::super::super::compiled_successor_v19::SCHEMA { return current; }
    authenticate_current_page_for_retained_controls(&current);
    let raw=include_bytes!(concat!(env!("CARGO_MANIFEST_DIR"),
        "/e/ab3c0bc973d3720e5afe6eb2b1594cd4080bc391a8ff6cae9773dadb2cdd979a"));
    assert_eq!(raw.len(),261018);
    assert_eq!(hash(raw),"ab3c0bc973d3720e5afe6eb2b1594cd4080bc391a8ff6cae9773dadb2cdd979a");
    let path=std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("e/ab3c0bc973d3720e5afe6eb2b1594cd4080bc391a8ff6cae9773dadb2cdd979a");
    let physical=super::super::super::held_4a6eb(&path,raw.len(),&hash(raw)).unwrap();
    assert_eq!(physical.as_bytes(),raw.as_slice());
    let prior:Value=serde_json::from_slice(raw).unwrap();
    assert_eq!(prior["schema"],SCHEMA_V17);
    assert_eq!(canonical(&prior).as_slice(),raw.as_slice());
    prior
}
