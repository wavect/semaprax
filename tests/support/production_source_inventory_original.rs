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
    let interop_epoch = path == Path::new("crates/semaprax-native-rust-interop-builder/src/implementation/tests/ledger_capacity/source_inventory.rs");
    let read_length = if interop_epoch {
        assert_eq!(length, 229209);
        assert_eq!(row["sha256"], "d1edee35f9a7a97e9478264a44b7e77e4b3903bcbb87bb327602053f22afa6c8");
        233111
    } else { length };
    assert_eq!(metadata.len(), read_length as u64);
    let mut bytes = Vec::new();
    bytes.try_reserve_exact(read_length).unwrap();
    bytes.resize(read_length, 0);
    file.read_exact(&mut bytes).unwrap();
    let mut tail = [0];
    assert_eq!(file.read(&mut tail).unwrap(), 0);
    if interop_epoch {
        let compiled = include_bytes!("../../crates/semaprax-native-rust-interop-builder/src/implementation/tests/ledger_capacity/source_inventory.rs");
        assert_eq!(bytes.as_slice(), compiled.as_slice());
        assert_eq!(format!("{:x}", semaprax::digest_hex::LowerHex(Sha256::digest(&bytes))), "efd27c7b797d0f0414bc7ae292be0c518455d6f1dbef4f849350318181b20b60");
        let mut field_epoch = String::from_utf8(bytes).unwrap();
        let field_raw_arms = "include_str!(\"../../../../../src/cleanup_plan/replay/path_join/field_local/mod.rs\"),include_str!(\"../../../../../src/cleanup_plan/replay/path_join/field_local/frontier.rs\"),include_str!(\"../../../../../src/cleanup_plan/replay/path_join/field_local/boxed_flow.rs\"),include_str!(\"../../../../../src/cleanup_plan/replay/path_join/field_local/tests.rs\"),";
        assert_eq!(field_epoch.matches(field_raw_arms).count(), 1);
        field_epoch = field_epoch.replacen(field_raw_arms, "", 1);
        bytes = field_epoch.into_bytes();
        assert_eq!(bytes.len(), 232761);
        assert_eq!(format!("{:x}", semaprax::digest_hex::LowerHex(Sha256::digest(&bytes))), "0f8b4dff2deb10dc0989ddf58723fb7137442cacffc18870d9e2afe566c8d3d0");
        let mut retained_epoch = String::from_utf8(bytes).unwrap();
        let retained_raw_arm = ",include_str!(\"../../../../../src/cleanup_plan/replay/ordinary_plan_history/completed_fork/emission/retained.rs\")";
        assert_eq!(retained_epoch.matches(retained_raw_arm).count(), 1);
        retained_epoch = retained_epoch.replacen(retained_raw_arm, "", 1);
        bytes = retained_epoch.into_bytes();
        assert_eq!(bytes.len(), 232648);
        assert_eq!(format!("{:x}", semaprax::digest_hex::LowerHex(Sha256::digest(&bytes))), "208348c0444014d05129580b71eef50e9837425c6b9cb7286cf21bc90c9d1e3f");
        let original = include_bytes!("../../e/d1edee35f9a7a97e9478264a44b7e77e4b3903bcbb87bb327602053f22afa6c8");
        assert_eq!(original.len(), length);
        assert_eq!(format!("{:x}", semaprax::digest_hex::LowerHex(Sha256::digest(original))), row["sha256"].as_str().unwrap());
        bytes = original.to_vec();
        assert_eq!(std::fs::read(Path::new(env!("CARGO_MANIFEST_DIR")).join(path)).unwrap().as_slice(), compiled.as_slice());
    }
    if path == Path::new("Cargo.toml") {
        // Acquire the complete physical/compiled dependency-update epoch first.
        assert_eq!(bytes.as_slice(), include_bytes!("../../Cargo.toml").as_slice());
        assert_eq!(bytes.len(), 4411);
        assert_eq!(format!("{:x}", semaprax::digest_hex::LowerHex(Sha256::digest(&bytes))), "206f6e4ac63533b6f1e136ffe44c3d80f580a1009cc05688c771ab4ea4e5e99b");
        let mut manifest = String::from_utf8(bytes).unwrap();
        for (current, before) in [
            ("wasmparser = { version = \"=0.262.0\", default-features = false, features = [\"component-model\", \"features\", \"validate\"] }\nwasm-encoder = { version = \"=0.262.0\", default-features = false, features = [\"std\", \"component-model\"] }\n", "wasmparser = { version = \"=0.261.0\", default-features = false, features = [\"component-model\", \"features\", \"validate\"] }\nwasm-encoder = { version = \"=0.261.0\", default-features = false, features = [\"std\", \"component-model\"] }\n"),
            ("libc = \"=0.2.191\"\n", "libc = \"=0.2.190\"\n"),
        ] {
            assert_eq!(manifest.matches(current).count(), 1);
            manifest = manifest.replacen(current, before, 1);
        }
        bytes = manifest.into_bytes();
        assert_eq!(bytes.len(), length);
        assert_eq!(std::fs::read(Path::new(env!("CARGO_MANIFEST_DIR")).join(path)).unwrap().as_slice(), include_bytes!("../../Cargo.toml").as_slice());
    }
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
        "bin" => {
            assert_eq!(root, "src/main.rs");
            assert_eq!(chain["package"].as_str().unwrap(), "semaprax");
            false
        }
        "test" => {
            // An ordinary Cargo test harness is compiled with cfg(test).
            assert!((root.starts_with("tests/") && root.ends_with(".rs"))
                || (root == "crates/semaprax-toolchain/tests/cli_help_surface_v1.rs"
                    && chain["package"].as_str().unwrap() == "semaprax-toolchain"));
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
