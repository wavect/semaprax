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
    let interop_path = path == Path::new("crates/semaprax-native-rust-interop-builder/src/implementation/tests/ledger_capacity/source_inventory.rs");
    let interop_registered = interop_path && row["sha256"] == "c62b6af0b063d006f709d5eec28c804f367fbee9f96319861c9d4b50394281c3";
    if interop_registered { assert_eq!(length, 242643); }
    let interop_next990 = interop_path && row["sha256"] == "e3af6b601614a9a7e902a718d6d9cca752a5d68d0e89862f28a4c6b39a6b6e98";
    if interop_next990 { assert_eq!(length, 241368); }
    let interop_next977 = interop_path && row["sha256"] == "677cc0d000dfc7a179d0dc07956f9b3f9ca68c4ee2f14e1d134e1c474004f1b6";
    if interop_next977 { assert_eq!(length, 240728); }
    let interop_next = interop_path && row["sha256"] == "a056e62b5dc75528b2d19f5469b08ab831fbf48c2f8348154a3e360c341c5b4b";
    if interop_next { assert_eq!(length, 239622); }
    let interop_epoch = interop_path && row["sha256"] == "d1edee35f9a7a97e9478264a44b7e77e4b3903bcbb87bb327602053f22afa6c8";
    let interop_prior_current = interop_path && row["sha256"] == "760872c3ea2924d4accd70e0b42b8573debaa312cfa4cd8a11cd61455b4701c5";
    let interop_current = interop_path && row["sha256"] == "22c4e696c83d5e53b8b2fdb5e0f5b00a7e6b40e9bb53fffbc0fc4e005152b257";
    if interop_current { assert_eq!(length, 239547); }
    if interop_path && !interop_epoch && !interop_prior_current && !interop_current && !interop_next && !interop_next977 && !interop_next990 && !interop_registered {
        assert_eq!(length, 235724);
        assert_eq!(row["sha256"], "23ea864c426901f3774b0a89bc3f2014e8cd9e784ccdc300f7718b2c174cd6b5");
    }
    let read_length = if interop_epoch {
        assert_eq!(length, 229209);
        assert_eq!(row["sha256"], "d1edee35f9a7a97e9478264a44b7e77e4b3903bcbb87bb327602053f22afa6c8");
        235724
    } else if interop_prior_current {
        assert_eq!(length, 233409);
        235724
    } else { length };
    let read_length = if interop_path { 242643 } else { read_length };
    assert_eq!(metadata.len(), read_length as u64);
    let mut bytes = Vec::new();
    bytes.try_reserve_exact(read_length).unwrap();
    bytes.resize(read_length, 0);
    file.read_exact(&mut bytes).unwrap();
    let mut tail = [0];
    assert_eq!(file.read(&mut tail).unwrap(), 0);
    if interop_path {
        assert_eq!(bytes.as_slice(), include_bytes!("../../crates/semaprax-native-rust-interop-builder/src/implementation/tests/ledger_capacity/source_inventory.rs").as_slice());
    }
    if interop_path {
        assert_eq!(bytes.len(), 242643);
        assert_eq!(format!("{:x}", semaprax::digest_hex::LowerHex(Sha256::digest(&bytes))), "c62b6af0b063d006f709d5eec28c804f367fbee9f96319861c9d4b50394281c3");
        if !interop_registered { bytes = native_before_registered_raw(); }
    }
    if !interop_registered {
    if interop_path {
        assert_eq!(bytes.len(), 241368);
        assert_eq!(format!("{:x}", semaprax::digest_hex::LowerHex(Sha256::digest(&bytes))), "e3af6b601614a9a7e902a718d6d9cca752a5d68d0e89862f28a4c6b39a6b6e98");
        if !interop_next990 {
            let prior = include_bytes!("../../e/677cc0d000dfc7a179d0dc07956f9b3f9ca68c4ee2f14e1d134e1c474004f1b6");
            assert_eq!(prior.len(), 240728);
            assert_eq!(format!("{:x}", semaprax::digest_hex::LowerHex(Sha256::digest(prior))), "677cc0d000dfc7a179d0dc07956f9b3f9ca68c4ee2f14e1d134e1c474004f1b6");
            bytes = prior.to_vec();
        }
    }
    if !interop_next990 {
    if interop_path {
        assert_eq!(bytes.len(), 240728);
        assert_eq!(format!("{:x}", semaprax::digest_hex::LowerHex(Sha256::digest(&bytes))), "677cc0d000dfc7a179d0dc07956f9b3f9ca68c4ee2f14e1d134e1c474004f1b6");
        if !interop_next977 {
            let prior = include_bytes!("../../e/a056e62b5dc75528b2d19f5469b08ab831fbf48c2f8348154a3e360c341c5b4b");
            assert_eq!(prior.len(), 239622);
            assert_eq!(format!("{:x}", semaprax::digest_hex::LowerHex(Sha256::digest(prior))), "a056e62b5dc75528b2d19f5469b08ab831fbf48c2f8348154a3e360c341c5b4b");
            bytes = prior.to_vec();
        }
    }
    if interop_path && !interop_next977 {
        assert_eq!(bytes.len(), 239622);
        assert_eq!(format!("{:x}", semaprax::digest_hex::LowerHex(Sha256::digest(&bytes))), "a056e62b5dc75528b2d19f5469b08ab831fbf48c2f8348154a3e360c341c5b4b");
        if !interop_next {
            let prior = include_bytes!("../../e/22c4e696c83d5e53b8b2fdb5e0f5b00a7e6b40e9bb53fffbc0fc4e005152b257");
            assert_eq!(prior.len(), 239547);
            assert_eq!(format!("{:x}", semaprax::digest_hex::LowerHex(Sha256::digest(prior))), "22c4e696c83d5e53b8b2fdb5e0f5b00a7e6b40e9bb53fffbc0fc4e005152b257");
            bytes = prior.to_vec();
        }
    }
    if interop_path && !interop_next && !interop_next977 {
        assert_eq!(bytes.len(), 239547);
        assert_eq!(format!("{:x}", semaprax::digest_hex::LowerHex(Sha256::digest(&bytes))), "22c4e696c83d5e53b8b2fdb5e0f5b00a7e6b40e9bb53fffbc0fc4e005152b257");
        if !interop_current {
            let prior = include_bytes!("../../e/23ea864c426901f3774b0a89bc3f2014e8cd9e784ccdc300f7718b2c174cd6b5");
            assert_eq!(prior.len(), 235724);
            assert_eq!(format!("{:x}", semaprax::digest_hex::LowerHex(Sha256::digest(prior))), "23ea864c426901f3774b0a89bc3f2014e8cd9e784ccdc300f7718b2c174cd6b5");
            bytes = prior.to_vec();
        }
    }
    let native_before = |raw: Vec<u8>| {
        assert_eq!(raw.len(), 235724);
        assert_eq!(format!("{:x}", semaprax::digest_hex::LowerHex(Sha256::digest(&raw))), "23ea864c426901f3774b0a89bc3f2014e8cd9e784ccdc300f7718b2c174cd6b5");
        let mut text = String::from_utf8(raw).unwrap();
        for literal in [
            ",include_str!(\"../../../../../src/workspace_graph/expected_projection/retained_scratch.rs\")",
            ",include_str!(\"../../../../../src/workspace_graph/expected_projection/shared_local_callees/before903.rs\")",
            ",include_str!(\"../../../../../src/workspace_graph/expected_projection/shared_local_callees/before903/call_carriers.rs\")",
            ",include_str!(\"../../../../../src/workspace_graph/expected_projection/shared_local_callees/before903/call_carriers_directory.rs\")",
            ",include_str!(\"../../../../../src/workspace_graph/expected_projection/shared_local_callees/before903/call_carriers_directory/modular.rs\")",
            ",include_str!(\"../../../../../src/workspace_graph/expected_projection/shared_local_callees/before903/call_carriers_directory/reused.rs\")",
            ",include_str!(\"../../../../../src/workspace_graph/expected_projection/shared_local_callees/before903/call_carriers_joint.rs\")",
            ",include_str!(\"../../../../../src/workspace_graph/expected_projection/shared_local_callees/before903/held_projection/modular_projection.rs\")",
            ",include_str!(\"../../../../../src/workspace_graph/expected_projection/shared_local_callees/before903/held_projection/paired_projection.rs\")",
            ",include_str!(\"../../../../../src/workspace_graph/expected_projection/shared_local_callees/before903/modular_pair.rs\")",
            ",include_str!(\"../../../../../src/workspace_graph/expected_projection/shared_local_callees/before903/paired.rs\")",
            ",include_str!(\"../../../../../src/workspace_graph/expected_projection/shared_local_callees/before903/shared_local_callees/modular_census.rs\")",
            ",include_str!(\"../../../../../src/workspace_graph/expected_projection/uncached_peak/retained_scratch_tests.rs\")",
            ",include_str!(\"../../../../../src/workspace_graph/expected_projection/uncached_peak_before.rs\")",
            ",include_str!(\"../../../../../src/loan_plan/owned_capacity.rs\")",
            ",include_str!(\"../../../../../src/workspace_graph/type_proof_tests.rs\")",
            ",include_str!(\"../../../../../src/workspace_graph/tests/builder_limits.rs\")",
            ",include_str!(\"../../../../../src/bounded_output/stage_debits.rs\")",
            ",include_str!(\"../../../../../src/cleanup_plan/replay/factored/oracle_order/tests.rs\")",
            ",include_str!(\"../../../../../src/workspace_graph/diagnostics.rs\")",
            ",include_str!(\"../../../../../src/workspace_graph/expected_projection/import_stub/core_proofs/tests.rs\")",
            ",include_str!(\"../../../../../src/workspace_graph/diagnostics/borrowed_help_tests.rs\")",
        ] {
            assert_eq!(text.matches(literal).count(), 1);
            text = text.replacen(literal, "", 1);
        }
        assert_eq!(text.len(), 233409);
        assert_eq!(format!("{:x}", semaprax::digest_hex::LowerHex(Sha256::digest(text.as_bytes()))), "760872c3ea2924d4accd70e0b42b8573debaa312cfa4cd8a11cd61455b4701c5");
        text.into_bytes()
    };
    if interop_prior_current { bytes = native_before(bytes); }
    if interop_epoch {
        let compiled = include_bytes!("../../crates/semaprax-native-rust-interop-builder/src/implementation/tests/ledger_capacity/source_inventory.rs");
        assert_eq!(bytes.as_slice(), include_bytes!("../../e/23ea864c426901f3774b0a89bc3f2014e8cd9e784ccdc300f7718b2c174cd6b5").as_slice());
        bytes = native_before(bytes);
        // Complete two-child raw epoch precedes every unchanged historical leg.
        assert_eq!(format!("{:x}", semaprax::digest_hex::LowerHex(Sha256::digest(&bytes))), "760872c3ea2924d4accd70e0b42b8573debaa312cfa4cd8a11cd61455b4701c5");
        let mut next_epoch = String::from_utf8(bytes).unwrap();
        assert_eq!(next_epoch.matches(",include_str!(\"../../../../../tests/wasm/scalar_browser_ci_contract/current_profiles3d81.rs\")").count(), 1);
        next_epoch = next_epoch.replacen(",include_str!(\"../../../../../tests/wasm/scalar_browser_ci_contract/current_profiles3d81.rs\")", "", 1);
        assert_eq!(next_epoch.matches(",include_str!(\"../../../../../src/cleanup_plan/replay/ordinary_plan_history/completed_fork/emission/immediate_before.rs\")").count(), 1);
        next_epoch = next_epoch.replacen(",include_str!(\"../../../../../src/cleanup_plan/replay/ordinary_plan_history/completed_fork/emission/immediate_before.rs\")", "", 1);
        bytes = next_epoch.into_bytes();
        assert_eq!(bytes.len(), 233195);
        assert_eq!(format!("{:x}", semaprax::digest_hex::LowerHex(Sha256::digest(&bytes))), "0e793e87c5913d29d090949527031177cf4ed760e1051bb1f032f21580f1782c");
        let mut main3d81_epoch = String::from_utf8(bytes).unwrap();
        let main3d81_raw_arm = "main3d81_before.rs\"),include_str!(\"../../../../../src/cleanup_plan/replay/path_join/";
        assert_eq!(main3d81_epoch.matches(main3d81_raw_arm).count(), 1);
        main3d81_epoch = main3d81_epoch.replacen(main3d81_raw_arm, "", 1);
        bytes = main3d81_epoch.into_bytes();
        assert_eq!(bytes.len(), 233111);
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
    }
    }
    if path == Path::new("Cargo.toml") {
        // Acquire the complete physical/compiled dependency-update epoch first.
        assert_eq!(bytes.as_slice(), include_bytes!("../../Cargo.toml").as_slice());
        assert_eq!(bytes.len(), 4411);
        assert_eq!(format!("{:x}", semaprax::digest_hex::LowerHex(Sha256::digest(&bytes))), "206f6e4ac63533b6f1e136ffe44c3d80f580a1009cc05688c771ab4ea4e5e99b");
        if row["sha256"] != "206f6e4ac63533b6f1e136ffe44c3d80f580a1009cc05688c771ab4ea4e5e99b" {
        assert_eq!(row["sha256"], "4a04317123051b85eecd20f4840e760fa9494c4d3e59d5146669cd5916348a67");
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
        }
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
    if interop_path {
        let physical_path = Path::new(env!("CARGO_MANIFEST_DIR")).join(path);
        let mut options = std::fs::OpenOptions::new();
        options.read(true);
        #[cfg(unix)] {
            use std::os::unix::fs::OpenOptionsExt;
            options.custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK | libc::O_CLOEXEC);
        }
        #[cfg(windows)] {
            use std::os::windows::fs::OpenOptionsExt;
            options.custom_flags(0x0020_0000); // FILE_FLAG_OPEN_REPARSE_POINT: inspect, never follow.
        }
        #[cfg(not(any(unix, windows)))]
        panic!("native990 custody requires a nofollow regular-file reader");
        let identity = |m: std::fs::Metadata| {
            assert!(m.is_file() && !m.file_type().is_symlink());
            #[cfg(unix)] {
                use std::os::unix::fs::MetadataExt;
                (m.dev(), m.ino(), m.mode(), m.len(), m.mtime(), m.mtime_nsec(), m.ctime(), m.ctime_nsec())
            }
            #[cfg(not(unix))] { (m.len(), m.created().unwrap(), m.modified().unwrap()) }
        };
        let file = options.open(&physical_path).unwrap();
        let metadata = file.metadata().unwrap();
        assert_eq!(metadata.len(), 242643);
        let before_identity = identity(metadata);
        assert_eq!(before_identity, identity(std::fs::symlink_metadata(&physical_path).unwrap()));
        let mut later = Vec::new();
        later.try_reserve_exact(242644).unwrap();
        (&file).take(242644).read_to_end(&mut later).unwrap();
        assert_eq!(later.len(), 242643);
        assert_eq!(later.as_slice(), include_bytes!("../../crates/semaprax-native-rust-interop-builder/src/implementation/tests/ledger_capacity/source_inventory.rs").as_slice());
        assert_eq!(before_identity, identity(file.metadata().unwrap()));
        assert_eq!(before_identity, identity(std::fs::symlink_metadata(&physical_path).unwrap()));
    }
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

// Exact immediate predecessor archive; no generic source alias or discovery.
fn native_before_registered_raw() -> Vec<u8> {
    let compiled = include_bytes!("../../e/e3af6b601614a9a7e902a718d6d9cca752a5d68d0e89862f28a4c6b39a6b6e98");
    assert_eq!(compiled.len(), 241368);
    assert_eq!(format!("{:x}", semaprax::digest_hex::LowerHex(Sha256::digest(compiled))), "e3af6b601614a9a7e902a718d6d9cca752a5d68d0e89862f28a4c6b39a6b6e98");
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("e/e3af6b601614a9a7e902a718d6d9cca752a5d68d0e89862f28a4c6b39a6b6e98");
    let mut options = std::fs::OpenOptions::new();
    options.read(true);
    #[cfg(unix)] {
        use std::os::unix::fs::OpenOptionsExt;
        options.custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK | libc::O_CLOEXEC);
    }
    #[cfg(windows)] {
        use std::os::windows::fs::OpenOptionsExt;
        options.custom_flags(0x0020_0000);
    }
    #[cfg(not(any(unix, windows)))]
    panic!("registered RAW custody requires a nofollow regular-file reader");
    let identity = |m: std::fs::Metadata| {
        assert!(m.is_file() && !m.file_type().is_symlink());
        #[cfg(unix)] {
            use std::os::unix::fs::MetadataExt;
            (m.dev(), m.ino(), m.mode(), m.len(), m.mtime(), m.mtime_nsec(), m.ctime(), m.ctime_nsec())
        }
        #[cfg(not(unix))] { (m.len(), m.created().unwrap(), m.modified().unwrap()) }
    };
    let file = options.open(&path).unwrap();
    let before = identity(file.metadata().unwrap());
    assert_eq!(before, identity(std::fs::symlink_metadata(&path).unwrap()));
    assert_eq!(file.metadata().unwrap().len(), 241368);
    assert!(241369 <= 2 * 1024 * 1024);
    let mut raw = Vec::new();
    raw.try_reserve_exact(241369).unwrap();
    (&file).take(241369).read_to_end(&mut raw).unwrap();
    assert_eq!(raw.as_slice(), compiled.as_slice());
    assert_eq!(before, identity(file.metadata().unwrap()));
    assert_eq!(before, identity(std::fs::symlink_metadata(&path).unwrap()));
    raw
}
