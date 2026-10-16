//! Test-harness custody and production-authority partition. No compiler API.
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};
use std::io::Read;
use std::path::{Component, Path};
fn current_source(path: &str, row: &serde_json::Value) -> String {
    let path = Path::new(path);
    assert!(path
        .components()
        .all(|part| matches!(part, Component::Normal(_))));
    let length = usize::try_from(row["length"].as_u64().unwrap()).unwrap();
    assert!(length <= 16 * 1024 * 1024, "bounded current source");
    let mut file = std::fs::File::open(Path::new(env!("CARGO_MANIFEST_DIR")).join(path)).unwrap();
    let metadata = file.metadata().unwrap();
    assert!(metadata.is_file());
    assert_eq!(metadata.len(), length as u64);
    let mut bytes = Vec::new();
    bytes.try_reserve_exact(length).unwrap();
    bytes.resize(length, 0);
    file.read_exact(&mut bytes).unwrap();
    let mut tail = [0];
    assert_eq!(file.read(&mut tail).unwrap(), 0);
    let digest = format!(
        "{:x}",
        semaprax::digest_hex::LowerHex(Sha256::digest(&bytes))
    );
    assert_eq!(
        digest,
        row["sha256"].as_str().unwrap(),
        "actual current source drift: {}",
        path.display()
    );
    String::from_utf8(bytes).unwrap()
}
fn registration_is_test_only(
    source: &str,
    chain: &serde_json::Value,
    sources: &BTreeMap<String, String>,
) -> bool {
    let root = chain["root"].as_str().unwrap();
    assert!(sources.contains_key(root));
    let mut test_only = match chain["kind"].as_str().unwrap() {
        "lib" => {
            assert_eq!(root, "src/lib.rs");
            false
        }
        "test" => {
            // An ordinary Cargo test harness is compiled with cfg(test).
            assert!(root.starts_with("tests/") && root.ends_with(".rs"));
            assert!(sources.contains_key("Cargo.toml"));
            true
        }
        _ => panic!("unregistered source root kind"),
    };
    let mut current = root;
    for edge in chain["edges"].as_array().unwrap() {
        assert_eq!(
            edge["parent"].as_str().unwrap(),
            current,
            "contiguous registration chain"
        );
        let parent = sources.get(current).unwrap();
        let start = usize::try_from(edge["start"].as_u64().unwrap()).unwrap();
        let text = edge["text"].as_str().unwrap();
        assert_eq!(
            parent.get(start..start + text.len()),
            Some(text),
            "complete actual registration bytes"
        );
        current = edge["child"].as_str().unwrap();
        assert!(sources.contains_key(current));
        for attribute in edge["attributes"].as_array().unwrap() {
            let attribute = attribute.as_str().unwrap();
            assert!(text.contains(attribute));
            if attribute.split_whitespace().collect::<String>() == "#[cfg(test)]" {
                test_only = true;
            }
        }
    }
    assert_eq!(
        current, source,
        "registration must reach the classified source"
    );
    test_only
}
pub(super) fn production(raw: &str, manifest: &str) -> String {
    let manifest: serde_json::Value = serde_json::from_str(manifest).unwrap();
    assert_eq!(manifest["schema"], "semaprax.source-authority-partition.v1");
    let sources: BTreeMap<String, String> = manifest["sources"]
        .as_object()
        .unwrap()
        .iter()
        .map(|(path, row)| (path.clone(), current_source(path, row)))
        .collect();
    let classes = manifest["classes"].as_object().unwrap();
    let mut production = BTreeSet::new();
    let mut test_only = BTreeSet::new();
    for (path, row) in classes {
        let chains = row["chains"].as_array().unwrap();
        assert!(!chains.is_empty(), "no unclassified source: {path}");
        let registrations: Vec<_> = chains
            .iter()
            .map(|chain| registration_is_test_only(path, chain, &sources))
            .collect();
        let all_test = registrations.iter().all(|value| *value);
        match row["role"].as_str().unwrap() {
            "production" => {
                assert!(!all_test);
                assert!(production.insert(path.as_str()));
            }
            "test-only" => {
                assert!(all_test);
                assert!(test_only.insert(path.as_str()));
            }
            _ => panic!("unclassified source role"),
        }
    }
    assert!(production.is_disjoint(&test_only));
    let mut observed = BTreeSet::new();
    let mut complete = String::new();
    let mut pure = String::new();
    for chunk in manifest["chunks"].as_array().unwrap() {
        if let Some(path) = chunk["path"].as_str() {
            assert!(
                classes.contains_key(path),
                "new unclassified inventory source: {path}"
            );
            observed.insert(path);
            let text = sources.get(path).unwrap();
            complete.push_str(text);
            if production.contains(path) {
                pure.push_str(text);
            }
        } else {
            let literal = chunk["literal"].as_str().unwrap();
            complete.push_str(literal);
            pure.push_str(literal);
        }
    }
    for path in manifest["closure_only"].as_array().unwrap() {
        let path = path.as_str().unwrap();
        assert!(
            classes.contains_key(path),
            "unclassified registered child: {path}"
        );
        assert!(
            observed.insert(path),
            "closure child cannot hide a duplicate raw input"
        );
        if production.contains(path) {
            pure.push_str(sources.get(path).unwrap());
        }
    }
    assert_eq!(observed.len(), production.len() + test_only.len());
    assert!(production
        .union(&test_only)
        .all(|path| observed.contains(path)));
    // Exact full concatenation: separators, duplicates and test bytes survive.
    assert_eq!(
        complete, raw,
        "complete raw source inventory changed or lost a child"
    );
    pure
}
