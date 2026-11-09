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

fn actual_before_v20() -> (Value, Value, Value) {
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
        header(&page).unwrap()["current"]
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
    let admitted = compose(before.clone(), &policy, &page).unwrap();
    let owner_index = admitted["rows"]
        .as_array()
        .unwrap()
        .iter()
        .position(|row| row["source"] == OWNER)
        .expect("actual canonical OWNER");
    let base_rows = before["rows"].as_array().unwrap().len();
    assert!(
        owner_index >= base_rows,
        "OWNER is an actual current addition"
    );
    let mut owner_page = page.clone();
    // Mutate the actual producer-emitted V18 typed row through its admitted
    // public 0/1 compiled slot; every ordinary decoder and cap check still runs.
    let owner = &mut owner_page["page"]["additions"][owner_index - base_rows];
    assert!(owner.is_array());
    assert_eq!(owner[1], 1);
    owner[1] = 0.into();
    assert_eq!(
        compose(before.clone(), &policy, &owner_page).unwrap_err(),
        // The ordinary V3 fragment census rejects this actual noncompiled
        // owner before the later successor-owner check can grant authority.
        "v3 noncompiled or nested reference",
        "an uncompiled actual canonical declaration owner cannot authorize the successor"
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

fn actual() -> (Value, Value, Value) {
    let (policy, page) = super::super::load_current().unwrap();
    if !is_successor20(&page) {
        return actual_before_v20();
    }
    let before = super::super::super::packet_before_overlay();
    compose20(before.clone(), &policy, &page).unwrap();
    let raw_page = include_bytes!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/e/9f61257911904ae42695cf2994813c97f403cadd5f71bdd330e1160d3db9b6c6"
    ));
    let raw_policy = include_bytes!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/e/c26bd8ae837d06c6b0048df2ce0c82bccd79f740af59719a39b8c3b8dc957ff0"
    ));
    for (raw, length, sha) in [
        (
            raw_page.as_slice(),
            261145,
            "9f61257911904ae42695cf2994813c97f403cadd5f71bdd330e1160d3db9b6c6",
        ),
        (
            raw_policy.as_slice(),
            196922,
            "c26bd8ae837d06c6b0048df2ce0c82bccd79f740af59719a39b8c3b8dc957ff0",
        ),
    ] {
        assert_eq!(raw.len(), length);
        assert_eq!(digest(raw), sha);
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join(format!("e/{sha}"));
        assert_eq!(
            super::super::held_4a6eb(&path, length, sha)
                .unwrap()
                .as_bytes(),
            raw
        );
    }
    let retained_page: Value = serde_json::from_slice(raw_page).unwrap();
    let retained_policy: Value = serde_json::from_slice(raw_policy).unwrap();
    assert_eq!(canonical(&retained_page).as_slice(), raw_page.as_slice());
    assert_eq!(
        canonical(&retained_policy).as_slice(),
        raw_policy.as_slice()
    );
    assert_eq!(retained_page["schema"], SCHEMA);
    compose(before.clone(), &retained_policy, &retained_page).unwrap();
    (before, retained_policy, retained_page)
}

fn auth_fixture20(owner: &str) -> Value {
    let mut fixture = auth_fixture(owner);
    fixture["schema"] = SCHEMA_V20.into();
    let (_, _, current) = actual20();
    fixture["page"] = current["page"].clone();
    fixture
}
fn actual20() -> (Value, Value, Value) {
    let (policy, page) = super::super::load_current().unwrap();
    let before = super::super::super::packet_before_overlay();
    if page["schema"] == SCHEMA_V20 {
        return (before, policy, (*page).clone());
    }
    assert!(is_compressed(&page));
    compose20(before.clone(), &policy, &page).unwrap();
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let sealed = |length, sha| {
        json(&held_4a6eb(&root.join(format!("e/{sha}")), length, sha).unwrap()).unwrap()
    };
    let page = sealed(
        261257,
        "5a2793a4da2c6447a9b2942148de346b0b5750e06207efe23477c05408f78944",
    );
    let policy = sealed(
        204884,
        "3702f3b3e89e58bb66cd205f312792b75ec7469487dc5a0a22cd27f70767cfa3",
    );
    compose20(before.clone(), &policy, &page).unwrap();
    (before, policy, page)
}
fn compressed_fixture20(page: &Value) -> Value {
    use base64::Engine as _;
    let raw = canonical(page);
    let mut stream = flate2::write::ZlibEncoder::new(Vec::new(), flate2::Compression::best());
    stream.write_all(&raw).unwrap();
    let payload = base64::engine::general_purpose::STANDARD.encode(stream.finish().unwrap());
    serde_json::json!({"schema":SCHEMA_V21,"length":raw.len(),"sha256":digest(&raw),"payload":payload})
}
fn packed20(page: &Value) -> Value {
    let entry = header20(page).unwrap();
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
fn exercise20_singleton_registration_authenticates_both_trusted_current_channels_before_declaration(
) {
    let page = auth_fixture20(OWNER_TEXT);
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
    authenticate20_with(&page, &registry, &acquire).unwrap();
    assert_eq!(reads.get(), 2);
    let tuple = packed20(&page);
    assert_eq!(
        canonical(&header20(&tuple).unwrap()),
        canonical(&header20(&page).unwrap()),
        "ENTIRE closed object reconstructed from the strict tuple"
    );
    reads.set(0);
    authenticate20_with(&tuple, &registry, &acquire).unwrap();
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
        assert!(authenticate20_with(&page, &incomplete, &acquire).is_err());
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
        assert!(authenticate20_with(&stale, &registry, &acquire).is_err());
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
        assert!(authenticate20_with(&page, &registry, &acquire).is_err());
    }
}
fn exercise20_singleton_declaration_rejects_hidden_attributes_duplicates_and_nonbyte_spans() {
    let ordinary = auth_fixture20(OWNER_TEXT);
    for owner in [
        format!("\n\n#[cfg(test)]\n{DECLARATION}"),
        format!("\n\n{DECLARATION}\n{DECLARATION}"),
    ] {
        let page = auth_fixture20(&owner);
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
        assert!(authenticate20_with(&page, &registry, &acquire).is_err());
    }
    for start in [
        serde_json::json!(true),
        serde_json::json!(-1),
        serde_json::json!(0),
        serde_json::json!(u64::MAX),
    ] {
        let mut page = ordinary.clone();
        page["compiled_successor"]["registration"]["start"] = start;
        assert!(header20(&page).is_err());
    }
}
fn exercise20_actual_v19_refuses_unknown_multiple_stale_and_legacy_classification_changes() {
    let (before, policy, page) = actual20();
    // The old closed-object route retains every object-only hostile. These
    // cannot be expressed by the fixed tuple and remain checked independently.
    let object = auth_fixture20(OWNER_TEXT);
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
        assert!(header20(&bad).is_err());
    }
    let page = packed20(&page);
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
        assert!(compose20(before.clone(), &policy, &bad).is_err());
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
            compose20(before.clone(), &changed_policy, &page).unwrap_err(),
            "v19 explicit replacement",
            "the singleton must occur exactly once in the explicit replacement scope"
        );
    }
    let admitted = compose20(before.clone(), &policy, &page).unwrap();
    let owner_index = admitted["rows"]
        .as_array()
        .unwrap()
        .iter()
        .position(|row| row["source"] == OWNER)
        .expect("actual canonical OWNER");
    let base_rows = before["rows"].as_array().unwrap().len();
    assert!(
        owner_index >= base_rows,
        "OWNER is an actual current addition"
    );
    let mut owner_page = page.clone();
    // Mutate the actual producer-emitted V18 typed row through its admitted
    // public 0/1 compiled slot; every ordinary decoder and cap check still runs.
    let owner = &mut owner_page["page"]["additions"][owner_index - base_rows];
    assert!(owner.is_array());
    assert_eq!(owner[1], 1);
    owner[1] = 0.into();
    assert_eq!(
        compose20(before.clone(), &policy, &owner_page).unwrap_err(),
        // The ordinary V3 fragment census rejects this actual noncompiled
        // owner before the later successor-owner check can grant authority.
        "v3 noncompiled or nested reference",
        "an uncompiled actual canonical declaration owner cannot authorize the successor"
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
        compose20(changed, &internal_policy, &bad).is_err(),
        "no true-to-true or repeated successor"
    );
    let mut over = page.clone();
    over["padding"] = "x".repeat(PAGE_CAP).into();
    assert!(
        header20(&over).is_err(),
        "the complete wrapper retains the original wire cap"
    );
}

#[test]
fn v20_entire_authority_hostile_corpus_and_warm_injected_reader_stay_fail_closed() {
    exercise20_singleton_registration_authenticates_both_trusted_current_channels_before_declaration();
    exercise20_singleton_declaration_rejects_hidden_attributes_duplicates_and_nonbyte_spans();
    exercise20_actual_v19_refuses_unknown_multiple_stale_and_legacy_classification_changes();
    let (_, page) = super::super::load_current().unwrap();
    let (logical, cap) = decoded20(&page).unwrap();
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let reads = Cell::new(0usize);
    let read = |path: &std::path::Path, n: usize, h: &str| {
        reads.set(reads.get() + 1);
        super::super::held_4a6eb(path, n, h)
    };
    authenticate20_using(&page, &read).unwrap();
    assert_eq!(reads.get(), 4);
    let entry = header20_with_bound(&logical, cap).unwrap();
    for path in [
        SOURCE,
        OWNER,
        entry["current"]["archive"].as_str().unwrap(),
        entry["registration"]["current"]["archive"]
            .as_str()
            .unwrap(),
    ] {
        let fail = root.join(path);
        assert!(authenticate20_using(&page, &|p, n, h| if p == fail {
            Err("hostile injected drift")
        } else {
            super::super::held_4a6eb(p, n, h)
        })
        .is_err());
    }
    // The same trusted-binding-before-I/O condition applies to the injected
    // warm reader; complete early/late routing is exercised by owning940 tests.
    {
        let mut stale = (*logical).clone();
        stale["compiled_successor"] = entry.into_owned();
        stale["compiled_successor"]["current"]["sha256"] = "0".repeat(64).into();
        stale["compiled_successor"]["current"]["archive"] = format!("e/{}", "0".repeat(64)).into();
        let stale = if is_compressed(&page) {
            compressed_fixture20(&stale)
        } else {
            stale
        };
        let reads = Cell::new(0usize);
        assert!(authenticate20_using(&stale, &|_, _, _| {
            reads.set(reads.get() + 1);
            Err("must not acquire")
        })
        .is_err());
        assert_eq!(reads.get(), 0);
    }
}

#[test]
fn compressed_overlay_keeps_both_whole_histories_and_refuses_damaged_streams() {
    use base64::Engine as _;
    let (policy, wire) = super::super::load_current().unwrap();
    let before = super::super::super::packet_before_overlay();
    let (logical, cap) = decoded20(&wire).unwrap();
    assert_eq!(cap, V21_OUTPUT_CAP);
    assert!(canonical(&logical).len() > PAGE_CAP);
    assert!(compose20(before.clone(), &policy, &logical).is_err());
    let joined = compose20(before.clone(), &policy, &wire).unwrap();
    let joined_rows: std::collections::BTreeMap<_, _> = joined["rows"]
        .as_array()
        .unwrap()
        .iter()
        .map(|row| (row["source"].as_str().unwrap(), row))
        .collect();
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let sealed = |length, sha| {
        let text = held_4a6eb(&root.join(format!("e/{sha}")), length, sha).unwrap();
        json(&text).unwrap()
    };
    for (page, old_policy) in [
        (
            sealed(
                237379,
                "52d7d59a0f6423e8da8bb3b0b9c9b0d5952af3c94e65d80ea531970f2b53d3a4",
            ),
            sealed(
                119583,
                "ded96b1a93ead67f80663037c8e8458ee57a3fdd03a807a1887d84192d1eeaef",
            ),
        ),
        (
            sealed(
                261257,
                "5a2793a4da2c6447a9b2942148de346b0b5750e06207efe23477c05408f78944",
            ),
            sealed(
                204884,
                "3702f3b3e89e58bb66cd205f312792b75ec7469487dc5a0a22cd27f70767cfa3",
            ),
        ),
    ] {
        let prior = compose_values(before.clone(), &old_policy, &page).unwrap();
        for old in prior["rows"].as_array().unwrap() {
            let row = joined_rows[old["source"].as_str().unwrap()];
            let views = row["views"].as_array().unwrap();
            assert!(old["views"]
                .as_array()
                .unwrap()
                .iter()
                .all(|view| views.contains(view)));
            let current = old["current"].as_object().unwrap();
            assert!(std::iter::once(&row["current"])
                .chain(views)
                .any(|pin| current.iter().all(|(key, value)| pin[key] == *value)));
        }
    }
    let mut inverse = joined;
    inverse["rows"] = before["rows"].clone();
    assert_eq!(inverse, before);

    let engine = base64::engine::general_purpose::STANDARD;
    let payload = wire["payload"].as_str().unwrap();
    let base85 = wire["schema"] == SCHEMA_V22;
    let mut compressed = if base85 {
        decode_base85(payload).unwrap()
    } else {
        engine.decode(payload).unwrap()
    };
    compressed.push(0);
    let damaged = if base85 {
        let mut encoded = Vec::new();
        for chunk in compressed.chunks(4) {
            let mut bytes = [0; 4];
            bytes[..chunk.len()].copy_from_slice(chunk);
            let digits = base85_digits(u32::from_be_bytes(bytes));
            encoded.extend_from_slice(&digits[..chunk.len() + 1]);
        }
        String::from_utf8(encoded).unwrap()
    } else {
        engine.encode(compressed)
    };
    for (key, value) in [
        ("length", 0.into()),
        ("length", (V21_OUTPUT_CAP + 1).into()),
        ("length", (canonical(&logical).len() - 1).into()),
        ("sha256", "0".repeat(64).into()),
        ("payload", damaged.into()),
        ("payload", "AAAA\"".into()),
        ("unknown", true.into()),
    ] {
        let mut bad = (*wire).clone();
        bad[key] = value;
        assert!(compose20(before.clone(), &policy, &bad).is_err(), "{key}");
    }
}

#[test]
fn base85_enforces_canonical_groups_and_tails() {
    for (encoded, bytes) in [
        ("", &[][..]),
        ("00", &[0][..]),
        ("000", &[0, 0][..]),
        ("0000", &[0, 0, 0][..]),
        ("00000", &[0, 0, 0, 0][..]),
        ("|NsC0", &[255, 255, 255, 255][..]),
    ] {
        assert_eq!(decode_base85(encoded).unwrap(), bytes);
    }
    for encoded in ["0", "01", "0001", "~~~~~", "00/00", "0 000"] {
        assert!(decode_base85(encoded).is_err(), "{encoded}");
    }
}
