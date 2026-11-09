//! Two closed source acquisitions; neither retargets an original V13 endpoint.
use serde_json::Value;

const OWNERS: [&str; 3] = [
    "docs/evidence/finite-source-overlay-v2-20261018/overlay_page.py",
    "src/source_reflows_merge/main_e177_overlay_v2.rs",
    "src/source_reflows_merge/main_e177_overlay_compiled.rs",
];
const TARGET: &str = "ca1b14fcf0f108f4c085b11a0b370faa33cde96c";

#[derive(Clone, Copy)]
enum Route {
    Original,
    Caller,
}

pub(super) fn original(source: &str, supplied: &str) -> String {
    if OWNERS.contains(&source) {
        restore(source, supplied, Route::Original)
    } else {
        supplied.to_owned()
    }
}

pub(super) fn caller(source: &str, supplied: &str) -> Option<String> {
    OWNERS
        .contains(&source)
        .then(|| restore(source, supplied, Route::Caller))
}

fn restore(source: &str, supplied: &str, route: Route) -> String {
    // The caller first authenticates its complete actual include_str! source.
    let packet: Value = serde_json::from_str(include_str!("controller_transport.json")).unwrap();
    assert_eq!(packet["schema"], "semaprax.v13-controller-transport.v1");
    assert_eq!(packet["target_commit"], TARGET);
    let records = packet["records"].as_array().unwrap();
    assert_eq!(records.len(), OWNERS.len());
    let names: Vec<_> = records
        .iter()
        .map(|record| record["source"].as_str().unwrap())
        .collect();
    assert_eq!(names, OWNERS);
    let record = &records[OWNERS.iter().position(|owner| *owner == source).unwrap()];
    assert_eq!(supplied.len() as u64, record["length"].as_u64().unwrap());
    assert_eq!(
        super::hash(supplied.as_bytes()),
        record["sha256"].as_str().unwrap()
    );
    let original: Value = serde_json::from_str(include_str!("inverse.json")).unwrap();
    let rows: Vec<_> = original
        .as_array()
        .unwrap()
        .iter()
        .filter(|row| row["source"] == source)
        .collect();
    assert_eq!(rows.len(), 1);
    let endpoint = match route {
        Route::Original => &record["original"],
        Route::Caller => &record["caller"],
    };
    let mut restored = supplied.to_owned();
    let edits = match route {
        Route::Original => &endpoint["edits"],
        Route::Caller => {
            assert_eq!(endpoint["edits_from_original"], source);
            &rows[0]["edits"]
        }
    };
    for edit in edits.as_array().unwrap() {
        let after = edit["after"].as_str().unwrap();
        assert!(!after.is_empty());
        assert_eq!(edit["count"].as_u64(), Some(1));
        assert_eq!(restored.matches(after).count(), 1);
        restored = restored.replacen(after, edit["before"].as_str().unwrap(), 1);
    }
    assert_eq!(restored.len() as u64, endpoint["length"].as_u64().unwrap());
    assert_eq!(
        super::hash(restored.as_bytes()),
        endpoint["sha256"].as_str().unwrap()
    );
    if let Route::Original = route {
        // Full ca1b inputs precede the independently replayed seven original
        // inverses. Their whole lower endpoints and records stay unchanged.
        assert_eq!(endpoint["length"], rows[0]["length"]);
        assert_eq!(endpoint["sha256"], rows[0]["sha256"]);
    }
    // Caller restores ONLY the V13 instrumentation delta. Its separately
    // pinned endpoint retains all merged metadata and MAIN_C907 cleanup input.
    restored
}
