use super::*;
use serde_json::json;
use sha2::{Digest, Sha256};

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
    let raw = include_bytes!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/docs/evidence/finite-source-overlay-v2-20261018/page.json"
    ));
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
        "actual complete source acquisition"
    );
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
        before_source(source, current(source));
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
