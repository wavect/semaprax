use super::*;
use std::cell::Cell;
use std::io::Write;
use std::process::{Command, Stdio};

const SOURCE_TEXT: &str =
    include_str!("../../workspace_graph/expected_projection/borrowed_imports.rs");
const OWNER_TEXT: &str = include_str!("../../workspace_graph/expected_projection.rs");

fn descriptor(raw: &str) -> Value {
    let sha = digest(raw.as_bytes());
    serde_json::json!({"length":raw.len(),"sha256":sha,"archive":format!("e/{sha}")})
}

fn auth_fixture(owner: &str) -> Value {
    serde_json::json!({
        "schema":SCHEMA,
        "compiled_successor":{
            "source":SOURCE,"prior_row_sha256":"0".repeat(64),"current":descriptor(SOURCE_TEXT),
            "registration":{"owner":OWNER,"current":descriptor(owner),
                "start":owner.find(DECLARATION).unwrap(),"text":DECLARATION}
        },
        "page":{"schema":super::super::fragments_v3::SCHEMA_V18}
    })
}

fn packed(page: &Value) -> Value {
    let entry = header(page).unwrap();
    let mut result = page.clone();
    result["compiled_successor"] = serde_json::json!([
        "m",
        entry["prior_row_sha256"],
        entry["current"]["length"],
        entry["current"]["sha256"],
        entry["registration"]["current"]["length"],
        entry["registration"]["current"]["sha256"],
        entry["registration"]["start"]
    ]);
    result
}

#[test]
fn singleton_registration_authenticates_both_trusted_current_channels_before_declaration() {
    let page = auth_fixture(OWNER_TEXT);
    let reads = Cell::new(0);
    let registry = |source: &str| match source {
        SOURCE => Some(SOURCE_TEXT),
        OWNER => Some(OWNER_TEXT),
        _ => None,
    };
    let acquire = |row: &Value| {
        reads.set(reads.get() + 1);
        assert_eq!(row["compiled"], true);
        Ok(registry(row["source"].as_str().unwrap())
            .unwrap()
            .to_owned())
    };
    authenticate_with(&page, &registry, &acquire).unwrap();
    assert_eq!(reads.get(), 2);
    let tuple = packed(&page);
    assert_eq!(
        canonical(&header(&tuple).unwrap()),
        canonical(&header(&page).unwrap()),
        "ENTIRE closed object reconstructed from the strict tuple"
    );
    reads.set(0);
    authenticate_with(&tuple, &registry, &acquire).unwrap();
    assert_eq!(reads.get(), 2);
    for missing in [SOURCE, OWNER] {
        reads.set(0);
        let incomplete = |source: &str| {
            if source == missing {
                None
            } else {
                registry(source)
            }
        };
        assert!(authenticate_with(&page, &incomplete, &acquire).is_err());
        assert_eq!(
            reads.get(),
            0,
            "both trusted bindings precede the first read"
        );
    }
    for channel in ["source", "owner"] {
        let mut stale = page.clone();
        let current = if channel == "source" {
            &mut stale["compiled_successor"]["current"]
        } else {
            &mut stale["compiled_successor"]["registration"]["current"]
        };
        current["sha256"] = "0".repeat(64).into();
        current["archive"] = format!("e/{}", "0".repeat(64)).into();
        reads.set(0);
        assert!(authenticate_with(&stale, &registry, &acquire).is_err());
        assert_eq!(
            reads.get(),
            0,
            "a wire pin cannot authorize physical acquisition"
        );
    }
    for broken in [SOURCE, OWNER] {
        let acquire = |row: &Value| {
            if row["source"] == broken {
                Ok("foreign physical bytes".to_owned())
            } else {
                Ok(registry(row["source"].as_str().unwrap())
                    .unwrap()
                    .to_owned())
            }
        };
        assert!(authenticate_with(&page, &registry, &acquire).is_err());
    }
}

#[test]
fn singleton_declaration_rejects_hidden_attributes_duplicates_and_nonbyte_spans() {
    let ordinary = auth_fixture(OWNER_TEXT);
    for owner in [
        format!("\n\n#[cfg(test)]\n{DECLARATION}"),
        format!("\n\n{DECLARATION}\n{DECLARATION}"),
    ] {
        let page = auth_fixture(&owner);
        let registry = |source: &str| match source {
            SOURCE => Some(SOURCE_TEXT),
            OWNER => Some(owner.as_str()),
            _ => None,
        };
        let acquire = |row: &Value| {
            Ok(registry(row["source"].as_str().unwrap())
                .unwrap()
                .to_owned())
        };
        assert!(authenticate_with(&page, &registry, &acquire).is_err());
    }
    for start in [
        serde_json::json!(true),
        serde_json::json!(-1),
        serde_json::json!(0),
        serde_json::json!(u64::MAX),
    ] {
        let mut page = ordinary.clone();
        page["compiled_successor"]["registration"]["start"] = start;
        assert!(header(&page).is_err());
    }
}

fn actual() -> (Value, Value, Value) {
    let (policy, page) = super::super::load_current().unwrap();
    assert_eq!(
        page["schema"], SCHEMA,
        "the eventual current source cut must use its actual V19 wire"
    );
    let before = super::super::super::packet_before_overlay();
    (before, policy, (*page).clone())
}

#[test]
fn actual_complete_v19_preserves_the_old_base_and_matches_python_composition() {
    let (before, policy, page) = actual();
    let saved = canonical(&before);
    assert_eq!(before["rows"][POSITION]["compiled"], false);
    let result = compose(before.clone(), &policy, &page).unwrap();
    assert_eq!(canonical(&before), saved);
    assert_eq!(result["rows"][POSITION]["compiled"], true);
    assert_eq!(
        result["rows"][POSITION]["current"],
        page["compiled_successor"]["current"]
    );
    let mut inverse = result.clone();
    inverse["rows"] = before["rows"].clone();
    assert_eq!(
        canonical(&inverse),
        saved,
        "every old top-level and row unknown survives the full inverse"
    );
    let script = "import json,runpy,sys\nn=runpy.run_path(sys.argv[1],run_name='v19_parity');v=json.load(sys.stdin);sys.stdout.buffer.write(n['canonical'](n['compose_values'](v['before'],v['policy'],v['page'])))\n";
    let mut child = Command::new("python3")
        .arg("-c")
        .arg(script)
        .arg(
            std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("docs/evidence/finite-source-overlay-v2-20261018/overlay_page.py"),
        )
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    child
        .stdin
        .take()
        .unwrap()
        .write_all(&canonical(
            &serde_json::json!({"before":before,"policy":policy,"page":page}),
        ))
        .unwrap();
    let output = child.wait_with_output().unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(
        output.stdout,
        canonical(&result),
        "entire actual Python/Rust output bytes"
    );
}

#[test]
fn actual_v19_refuses_unknown_multiple_stale_and_legacy_classification_changes() {
    let (before, policy, page) = actual();
    // The old closed-object route retains every object-only hostile. These
    // cannot be expressed by the fixed tuple and remain checked independently.
    let object = auth_fixture(OWNER_TEXT);
    let mut object_hostiles = Vec::new();
    let mut bad = object.clone();
    bad["compiled_successor"]["compiled"] = true.into();
    object_hostiles.push(bad);
    let mut bad = object.clone();
    bad["compiled_successor"]["source"] = OWNER.into();
    object_hostiles.push(bad);
    let mut bad = object.clone();
    bad["compiled_successor"]["current"]["archive"] = "src/foreign.rs".into();
    object_hostiles.push(bad);
    let mut bad = object.clone();
    bad["compiled_successor"]["registration"]["owner"] = SOURCE.into();
    object_hostiles.push(bad);
    let mut bad = object.clone();
    bad["compiled_successor"]["registration"]["text"] =
        format!("#[cfg(test)]\n{DECLARATION}").into();
    object_hostiles.push(bad);
    for bad in object_hostiles {
        assert!(header(&bad).is_err());
    }
    let page = packed(&page);
    let mut hostiles = Vec::new();
    let mut bad = page.clone();
    bad["unknown"] = true.into();
    hostiles.push(bad);
    let mut bad = page.clone();
    bad["compiled_successor"] = serde_json::json!([page["compiled_successor"].clone()]);
    hostiles.push(bad);
    let mut bad = page.clone();
    bad["compiled_successor"][1] = "0".repeat(64).into();
    hostiles.push(bad);
    let mut bad = page.clone();
    bad["compiled_successor"][2] = (super::super::SOURCE_CAP + 1).into();
    hostiles.push(bad);
    let mut bad = page.clone();
    bad["page"]["schema"] = SCHEMA.into();
    hostiles.push(bad);
    let mut bad = page.clone();
    bad["compiled_successor"][5] = "0".repeat(64).into();
    hostiles.push(bad);
    for slot in [2, 4, 6] {
        for value in [
            serde_json::json!(true),
            serde_json::json!(-1),
            serde_json::json!(1.0),
            serde_json::json!(u64::MAX),
        ] {
            let mut bad = page.clone();
            bad["compiled_successor"][slot] = value;
            hostiles.push(bad);
        }
    }
    for slot in [1, 3, 5] {
        for value in [
            serde_json::json!(true),
            serde_json::json!("G".repeat(64)),
            serde_json::json!("0".repeat(63)),
        ] {
            let mut bad = page.clone();
            bad["compiled_successor"][slot] = value;
            hostiles.push(bad);
        }
    }
    let mut bad = page.clone();
    bad["compiled_successor"][0] = "unknown".into();
    hostiles.push(bad);
    let mut bad = page.clone();
    bad["compiled_successor"].as_array_mut().unwrap().pop();
    hostiles.push(bad);
    let mut bad = page.clone();
    bad["compiled_successor"]
        .as_array_mut()
        .unwrap()
        .push(false.into());
    hostiles.push(bad);
    for bad in hostiles {
        assert!(compose(before.clone(), &policy, &bad).is_err());
    }
    for duplicate in [false, true] {
        let mut changed_policy = policy.clone();
        let paths = changed_policy["replacements"].as_array_mut().unwrap();
        if duplicate {
            paths.push(SOURCE.into());
        } else {
            paths.retain(|source| source.as_str() != Some(SOURCE));
        }
        assert_eq!(
            compose(before.clone(), &changed_policy, &page).unwrap_err(),
            "v19 explicit replacement",
            "the singleton must occur exactly once in the explicit replacement scope"
        );
    }
    let mut owner_false = before.clone();
    owner_false["rows"]
        .as_array_mut()
        .unwrap()
        .iter_mut()
        .find(|row| row["source"] == OWNER)
        .unwrap()["compiled"] = false.into();
    let mut owner_policy = policy.clone();
    owner_policy["predecessor"]["logical_sha256"] = digest(&canonical(&owner_false)).into();
    let mut owner_page = page.clone();
    owner_page["page"]["predecessor"] = owner_policy["predecessor"].clone();
    assert!(
        compose(owner_false, &owner_policy, &owner_page).is_err(),
        "an uncompiled canonical declaration owner cannot authorize a compiled successor"
    );
    assert!(
        super::super::compose_values(before.clone(), &policy, &page["page"]).is_err(),
        "V18/V2 still refuse a changed compiled flag"
    );
    let mut changed = before.clone();
    changed["rows"][POSITION]["compiled"] = true.into();
    let mut internal_policy = policy.clone();
    internal_policy["predecessor"]["logical_sha256"] = digest(&canonical(&changed)).into();
    let mut bad = page.clone();
    bad["page"]["predecessor"] = internal_policy["predecessor"].clone();
    assert!(
        compose(changed, &internal_policy, &bad).is_err(),
        "no true-to-true or repeated successor"
    );
    let mut over = page.clone();
    over["padding"] = "x".repeat(PAGE_CAP).into();
    assert!(
        header(&over).is_err(),
        "the complete wrapper retains the original wire cap"
    );
}
