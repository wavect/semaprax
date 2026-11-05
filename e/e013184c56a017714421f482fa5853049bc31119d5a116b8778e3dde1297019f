//! Closed final-batch historical acquisition. No production authority or raw-audit projection.
use sha2::{Digest, Sha256};
use std::path::Path;
const SOURCE_CAP: usize = 9753094;
fn digest(bytes: &[u8]) -> String {
    format!("{:x}", crate::digest_hex::LowerHex(Sha256::digest(bytes)))
}
fn packet() -> serde_json::Value {
    let data = include_str!(
        "../../docs/evidence/main-e177-final-source-binding-20261017/current-inputs.json"
    );
    assert_eq!(data.len(), 260977);
    assert_eq!(
        digest(data.as_bytes()),
        "de6103a37cb677be139de99b4a945ddf2d588b5292a01ca058715b3d7f8734dc"
    );
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    assert_eq!(
        held(
            &root.join("docs/evidence/main-e177-final-source-binding-20261017/current-inputs.json"),
            data.len(),
            &digest(data.as_bytes())
        )
        .unwrap(),
        data,
        "complete physical and compiled current manifest"
    );
    let packet: serde_json::Value = serde_json::from_str(data).unwrap();
    assert_eq!(packet["schema"], "semaprax.main-e177-final-whole-inputs.v1");
    assert_eq!(packet["source_cap"].as_u64(), Some(SOURCE_CAP as u64));
    let rows = packet["rows"].as_array().unwrap();
    assert_eq!(rows.len(), 433);
    let mut seen = std::collections::BTreeSet::new();
    let mut fragment_edges = 0;
    for row in rows {
        assert!(seen.insert(row["source"].as_str().unwrap()));
        if let Some(children) = row.get("fragments") {
            for child in children.as_array().unwrap() {
                assert!(
                    child.get("fragments").is_none(),
                    "fragment dependencies are leaves"
                );
                assert_eq!(child["compiled"], true);
                assert_eq!(
                    rows.iter()
                        .find(|candidate| candidate["source"] == child["source"])
                        .unwrap(),
                    child
                );
                fragment_edges += 1;
            }
        }
    }
    assert_eq!(fragment_edges, 10);
    // Replay every complete previous table and its original checks unchanged.
    let previous = packet_a110();
    let old_rows = previous["rows"].as_array().unwrap();
    assert!(rows.len() >= old_rows.len());
    for (before, after) in old_rows.iter().zip(rows) {
        assert_eq!(after["source"], before["source"], "original source order");
        let views = after["views"].as_array().unwrap();
        let old_views = before["views"].as_array().unwrap();
        assert!(views.len() >= old_views.len());
        assert_eq!(
            &views[..old_views.len()],
            old_views.as_slice(),
            "all original view fields and order"
        );
        if after["current"] != before["current"] {
            assert!(
                views.iter().any(|view| {
                    let mut view = view.clone();
                    view.as_object_mut().unwrap().remove("name");
                    view == before["current"]
                }),
                "complete previous current record must remain replayable"
            );
        }
        let mut restored = after.clone();
        restored["current"] = before["current"].clone();
        restored["views"] = before["views"].clone();
        restored.as_object_mut().unwrap().remove("fragments");
        assert_eq!(&restored, before, "all other original row fields");
    }
    let mut restored = packet.clone();
    restored["rows"] = previous["rows"].clone();
    assert_eq!(
        restored, previous,
        "all original packet fields and whole-table anchors"
    );
    packet
}
fn packet_a110() -> serde_json::Value {
    let data = include_str!(
        "../../docs/evidence/fmt-a110/packet-wV4m7PTunyjrIa8VFcfB0eaqPxmD03tBMXOGHXLXakY.json.txt"
    );
    assert_eq!(data.len(), 112949);
    assert_eq!(
        digest(data.as_bytes()),
        "c15e26ecf4ee9f28eb21af1515c7c1d1e6aa3f1983d37b413173861d72d76a46"
    );
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let physical = held(
        &root.join(
            "docs/evidence/fmt-a110/packet-wV4m7PTunyjrIa8VFcfB0eaqPxmD03tBMXOGHXLXakY.json.txt",
        ),
        data.len(),
        &digest(data.as_bytes()),
    )
    .unwrap();
    assert_eq!(
        physical, data,
        "complete physical and compiled current manifest"
    );
    let packet: serde_json::Value = serde_json::from_str(data).unwrap();
    assert_eq!(packet["schema"], "semaprax.main-e177-final-whole-inputs.v1");
    assert_eq!(packet["rows"].as_array().unwrap().len(), 124);
    {
        let packet: serde_json::Value =
            serde_json::from_str(&read_view(&packet["whole_current99_before"])).unwrap();
        let before = read_view(&packet["immediate_before"]);
        let mut restored = packet.clone();
        let added = restored["rows"].as_array_mut().unwrap().pop().unwrap();
        assert_eq!(
            added["source"],
            "tests/native/string_settlement/codec_allocation/codec808_frontier.rs"
        );
        let route = restored["rows"]
            .as_array_mut()
            .unwrap()
            .iter_mut()
            .find(|row| row["source"] == "src/source_reflows_merge/next_big_batch_inputs.rs")
            .unwrap();
        let mut previous = route["views"].as_array_mut().unwrap().pop().unwrap();
        assert_eq!(previous["name"], "whole886-next-big");
        previous.as_object_mut().unwrap().remove("name");
        route["current"] = previous;
        restored.as_object_mut().unwrap().remove("immediate_before");
        assert_eq!(
            restored,
            serde_json::from_str::<serde_json::Value>(&before).unwrap(),
            "complete original98 source table retained"
        );
    }
    {
        let previous: serde_json::Value =
            serde_json::from_str(&read_view(&packet["whole_3bf_current123_before"])).unwrap();
        let mut restored = packet.clone();
        let added = restored["rows"].as_array_mut().unwrap().pop().unwrap();
        assert_eq!(added["source"], "src/workspace_graph/tests.rs");
        let row=restored["rows"].as_array_mut().unwrap().iter_mut().find(|r|r["source"]=="src/cleanup_plan/replay/expression_facts/receiving/retained/case_certified/lookup_caller_before.rs").unwrap();
        let mut prior = row["views"].as_array_mut().unwrap().pop().unwrap();
        assert_eq!(prior["name"], "whole3bf-before-private-lookup-import");
        prior.as_object_mut().unwrap().remove("name");
        row["current"] = prior;
        let anchor = restored["complete_old_table_anchors"]
            .as_array_mut()
            .unwrap()
            .pop()
            .unwrap();
        assert_eq!(
            anchor["sha256"],
            packet["whole_3bf_current123_before"]["sha256"]
        );
        restored
            .as_object_mut()
            .unwrap()
            .remove("whole_3bf_current123_before");
        restored["current_stage"] = previous["current_stage"].clone();
        assert_eq!(
            restored, previous,
            "complete previous123 table retained without retargeting old inputs"
        );
    }
    let _remote = packet_remote_d88();
    for anchor in packet["complete_old_table_anchors"].as_array().unwrap() {
        let path = anchor["path"].as_str().unwrap();
        let raw = held(
            &root.join(path),
            anchor["length"].as_u64().unwrap() as usize,
            anchor["sha256"].as_str().unwrap(),
        )
        .unwrap();
        assert_eq!(
            Some(raw.as_str()),
            table_compiled(path),
            "whole predecessor table physical and compiled bytes"
        );
        assert_eq!(raw, read_view(anchor), "whole predecessor table archive");
    }
    packet
}
fn packet_remote_d88() -> serde_json::Value {
    let data = include_str!("../../docs/evidence/main-e177-final-source-binding-20261017/views/f3591ff220c338d79adaa9106b347daef4bea5b4227ef47063d58dabdeade469.txt");
    assert_eq!(data.len(), 68224);
    assert_eq!(
        digest(data.as_bytes()),
        "f3591ff220c338d79adaa9106b347daef4bea5b4227ef47063d58dabdeade469"
    );
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    assert_eq!(held(&root.join("docs/evidence/main-e177-final-source-binding-20261017/views/f3591ff220c338d79adaa9106b347daef4bea5b4227ef47063d58dabdeade469.txt"), data.len(), &digest(data.as_bytes())).unwrap(), data, "complete physical and compiled current manifest");
    let packet: serde_json::Value = serde_json::from_str(data).unwrap();
    let mut restored = packet.clone();
    let row = restored["rows"].as_array_mut().unwrap().iter_mut().find(|row| row["source"] == "src/cleanup_plan/replay/factored/canonical_shared/before_lexical_successors/compare.rs").unwrap();
    let mut previous = row["views"].as_array_mut().unwrap().pop().unwrap();
    assert_eq!(previous["name"], "whole-f47-msrv-parent-import");
    previous.as_object_mut().unwrap().remove("name");
    row["current"] = previous;
    assert_eq!(
        restored,
        packet_previous_remote_d88(),
        "complete predecessor table and all original controls retained"
    );
    packet
}
fn packet_previous_remote_d88() -> serde_json::Value {
    let data = include_str!("../../docs/evidence/ci-msrv-parent-import-20261018/whole/3384be6e49144e547e28e613294b2aa53aa8b538acb9cfd84b3dd003a9126a71.txt");
    assert_eq!(data.len(), 84171);
    assert_eq!(
        digest(data.as_bytes()),
        "3384be6e49144e547e28e613294b2aa53aa8b538acb9cfd84b3dd003a9126a71"
    );
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let physical=held(&root.join("docs/evidence/ci-msrv-parent-import-20261018/whole/3384be6e49144e547e28e613294b2aa53aa8b538acb9cfd84b3dd003a9126a71.txt"),data.len(),&digest(data.as_bytes())).unwrap();
    assert_eq!(
        physical, data,
        "complete physical and compiled current manifest"
    );
    let packet: serde_json::Value = serde_json::from_str(data).unwrap();
    assert_eq!(packet["schema"], "semaprax.main-e177-final-whole-inputs.v1");
    assert_eq!(packet["rows"].as_array().unwrap().len(), 99);
    let before = read_view(&packet["immediate_before"]);
    let mut restored = packet.clone();
    let added = restored["rows"].as_array_mut().unwrap().pop().unwrap();
    assert_eq!(
        added["source"],
        "tests/native/string_settlement/codec_allocation/codec808_frontier.rs"
    );
    let route = restored["rows"]
        .as_array_mut()
        .unwrap()
        .iter_mut()
        .find(|row| row["source"] == "src/source_reflows_merge/next_big_batch_inputs.rs")
        .unwrap();
    let mut previous = route["views"].as_array_mut().unwrap().pop().unwrap();
    assert_eq!(previous["name"], "whole886-next-big");
    previous.as_object_mut().unwrap().remove("name");
    route["current"] = previous;
    restored.as_object_mut().unwrap().remove("immediate_before");
    assert_eq!(
        restored,
        serde_json::from_str::<serde_json::Value>(&before).unwrap(),
        "complete original98 source table retained"
    );
    for anchor in packet["complete_old_table_anchors"].as_array().unwrap() {
        let path = anchor["path"].as_str().unwrap();
        let raw = held(
            &root.join(path),
            anchor["length"].as_u64().unwrap() as usize,
            anchor["sha256"].as_str().unwrap(),
        )
        .unwrap();
        assert_eq!(
            Some(raw.as_str()),
            table_compiled(path),
            "whole predecessor table physical and compiled bytes"
        );
        assert_eq!(raw, read_view(anchor), "whole predecessor table archive");
    }
    packet
}
fn same_file(a: &std::fs::Metadata, b: &std::fs::Metadata) -> bool {
    if !a.is_file() || !b.is_file() || a.len() != b.len() || a.modified().ok() != b.modified().ok()
    {
        return false;
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        if a.dev() != b.dev()
            || a.ino() != b.ino()
            || a.mode() != b.mode()
            || a.mtime() != b.mtime()
            || a.mtime_nsec() != b.mtime_nsec()
            || a.ctime() != b.ctime()
            || a.ctime_nsec() != b.ctime_nsec()
        {
            return false;
        }
    }
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt;
        if a.file_attributes() & 0x400 != 0
            || b.file_attributes() & 0x400 != 0
            || a.creation_time() != b.creation_time()
            || a.last_write_time() != b.last_write_time()
        {
            return false;
        }
    }
    true
}
fn held(path: &Path, length: usize, expected: &str) -> Result<String, &'static str> {
    use std::io::Read;
    if length > SOURCE_CAP {
        return Err("bound");
    }
    let before = std::fs::symlink_metadata(path).map_err(|_| "missing")?;
    if before.file_type().is_symlink() || !before.is_file() {
        return Err("nonregular");
    }
    let mut options = std::fs::OpenOptions::new();
    options.read(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.custom_flags(libc::O_NOFOLLOW | libc::O_CLOEXEC | libc::O_NONBLOCK);
    }
    #[cfg(windows)]
    {
        use std::os::windows::fs::OpenOptionsExt;
        options.custom_flags(0x0020_0000);
    }
    #[cfg(not(any(unix, windows)))]
    {
        return Err("platform");
    }
    let mut file = options.open(path).map_err(|_| "open")?;
    let opened = file.metadata().map_err(|_| "metadata")?;
    if !same_file(&before, &opened) || opened.len() != u64::try_from(length).map_err(|_| "bound")? {
        return Err("length-or-identity");
    }
    let mut bytes = Vec::new();
    bytes.try_reserve_exact(length).map_err(|_| "allocation")?;
    let mut limited = (&mut file).take(length as u64 + 1);
    limited.read_to_end(&mut bytes).map_err(|_| "read")?;
    if bytes.len() != length || digest(&bytes) != expected {
        return Err("length-or-digest");
    }
    let after = file.metadata().map_err(|_| "metadata")?;
    let path_after = std::fs::symlink_metadata(path).map_err(|_| "metadata")?;
    if path_after.file_type().is_symlink()
        || !same_file(&opened, &after)
        || !same_file(&after, &path_after)
    {
        return Err("identity-drift");
    }
    String::from_utf8(bytes).map_err(|_| "utf8")
}
fn table_compiled(source: &str) -> Option<&'static str> {
    match source {
        "docs/evidence/main-e177-final-source-binding-20261017/views/a1e425d6dec0ae2619b7c2f6964966a15e287aeecbdc27507166f13245a75fa6.txt" => Some(include_str!("../../docs/evidence/main-e177-final-source-binding-20261017/views/a1e425d6dec0ae2619b7c2f6964966a15e287aeecbdc27507166f13245a75fa6.txt")),
        "docs/evidence/main-e177-final-source-binding-20261017/views/3fa32fc7c4b039abed780ccc39b34d759e329705e2674f62cc56016ab08007bd.txt" => Some(include_str!("../../docs/evidence/main-e177-final-source-binding-20261017/views/3fa32fc7c4b039abed780ccc39b34d759e329705e2674f62cc56016ab08007bd.txt")),
        "docs/evidence/main-e177-final-source-binding-20261017/views/de9c72c5fba416e93d5fba422174cd0888c0bd6991fb2fc4367396c541bf92a7.txt" => Some(include_str!("../../docs/evidence/main-e177-final-source-binding-20261017/views/de9c72c5fba416e93d5fba422174cd0888c0bd6991fb2fc4367396c541bf92a7.txt")),
        "docs/evidence/next-big-batch-source-binding-20261017/current-inputs.json" => Some(include_str!("../../docs/evidence/next-big-batch-source-binding-20261017/current-inputs.json")),
        "docs/evidence/main99f-full-controller-acquisition-20261017/inputs.json" => Some(include_str!("../../docs/evidence/main99f-full-controller-acquisition-20261017/inputs.json")),
        "docs/evidence/main-e177-final-source-binding-20261017/views/7ce1ea34daa31a1eda756cf7c0cd542adcfa3d351cbc3e0d12a37c51e5036986.txt" => Some(include_str!("../../docs/evidence/main-e177-final-source-binding-20261017/views/7ce1ea34daa31a1eda756cf7c0cd542adcfa3d351cbc3e0d12a37c51e5036986.txt")),
        "docs/evidence/earned-managed-pin42-source-coupling-20261017/inputs.json" => Some(include_str!("../../docs/evidence/earned-managed-pin42-source-coupling-20261017/inputs.json")),
        "docs/evidence/semantic162-compile-source-ingress-20261017/inputs.json" => Some(include_str!("../../docs/evidence/semantic162-compile-source-ingress-20261017/inputs.json")),
        "docs/evidence/next-semantic-source-views-20261017/inputs.json" => Some(include_str!("../../docs/evidence/next-semantic-source-views-20261017/inputs.json")),
        "docs/evidence/main-e177-final-source-binding-20261017/views/bc66e360cefb55655815018153316bca517702bc32791462f2dfb9ca70b90d41.txt" => Some(include_str!("../../docs/evidence/main-e177-final-source-binding-20261017/views/bc66e360cefb55655815018153316bca517702bc32791462f2dfb9ca70b90d41.txt")),
        "docs/evidence/main-e177-final-source-binding-20261017/views/3384be6e49144e547e28e613294b2aa53aa8b538acb9cfd84b3dd003a9126a71.txt" => Some(include_str!("../../docs/evidence/main-e177-final-source-binding-20261017/views/3384be6e49144e547e28e613294b2aa53aa8b538acb9cfd84b3dd003a9126a71.txt")),
        "docs/evidence/main-e177-final-source-binding-20261017/views/2e4b3b5441418b7e21a1424445e19bb4473d15f196ab3a897ca920417dce730f.txt" => Some(include_str!("../../docs/evidence/main-e177-final-source-binding-20261017/views/2e4b3b5441418b7e21a1424445e19bb4473d15f196ab3a897ca920417dce730f.txt")),
        "docs/evidence/next-large-side-source-composition-20261017/side-inputs.json" => Some(include_str!("../../docs/evidence/next-large-side-source-composition-20261017/side-inputs.json")),
        "docs/evidence/main-e177-final-source-binding-20261017/views/996aa3de35097ef764384db3b8fbaffa57bb500fb18368280a20dac9c3ff5f52.txt" => Some(include_str!("../../docs/evidence/main-e177-final-source-binding-20261017/views/996aa3de35097ef764384db3b8fbaffa57bb500fb18368280a20dac9c3ff5f52.txt")),
        "docs/evidence/main-e177-final-source-binding-20261017/views/f3591ff220c338d79adaa9106b347daef4bea5b4227ef47063d58dabdeade469.txt" => Some(include_str!("../../docs/evidence/main-e177-final-source-binding-20261017/views/f3591ff220c338d79adaa9106b347daef4bea5b4227ef47063d58dabdeade469.txt")),
        "docs/evidence/main-e177-final-source-binding-20261017/views/3740fb709bdfb10aeb16a6159312f12b8752d2c910ac7ea6aae727ff01847d09.txt" => Some(include_str!("../../docs/evidence/main-e177-final-source-binding-20261017/views/3740fb709bdfb10aeb16a6159312f12b8752d2c910ac7ea6aae727ff01847d09.txt")),
        "docs/evidence/main-e177-final-source-binding-20261017/views/802a9e325c6d4e9da033593b365119441fb2c1431cb981793fe4026e29ff6734.txt" => Some(include_str!("../../docs/evidence/main-e177-final-source-binding-20261017/views/802a9e325c6d4e9da033593b365119441fb2c1431cb981793fe4026e29ff6734.txt")),
        _ => None,
    }
}
fn compiled(source: &str) -> Option<&'static str> {
    match source {
        "crates/semaprax-native-rust-interop-builder/src/implementation/tests/ledger_capacity/source_inventory_leaf_views.rs" => Some(include_str!("../../crates/semaprax-native-rust-interop-builder/src/implementation/tests/ledger_capacity/source_inventory_leaf_views.rs")),
        "src/cleanup_plan/replay/factored/canonical_shared/arena_tests_tail.rs" => Some(include_str!("../../src/cleanup_plan/replay/factored/canonical_shared/arena_tests_tail.rs")),
        "src/cleanup_plan/replay/factored/canonical_shared/before_lexical_successors_methods.rs" => Some(include_str!("../../src/cleanup_plan/replay/factored/canonical_shared/before_lexical_successors_methods.rs")),
        "src/cleanup_plan/replay/factored/canonical_shared/before_selected_labels_methods.rs" => Some(include_str!("../../src/cleanup_plan/replay/factored/canonical_shared/before_selected_labels_methods.rs")),
        "src/cleanup_plan/replay/factored/canonical_shared/before_selected_route_methods.rs" => Some(include_str!("../../src/cleanup_plan/replay/factored/canonical_shared/before_selected_route_methods.rs")),
        "src/cleanup_plan/replay/factored/canonical_shared/borrowed_labels_tail.rs" => Some(include_str!("../../src/cleanup_plan/replay/factored/canonical_shared/borrowed_labels_tail.rs")),
        "src/cleanup_plan/replay/live_oracle/source_custody/a_tail.rs" => Some(include_str!("../../src/cleanup_plan/replay/live_oracle/source_custody/a_tail.rs")),
        "src/cleanup_plan/replay/live_oracle/source_custody/b_tail.rs" => Some(include_str!("../../src/cleanup_plan/replay/live_oracle/source_custody/b_tail.rs")),
        "src/hir/resolve_program_methods.rs" => Some(include_str!("../../src/hir/resolve_program_methods.rs")),
        "src/source_reflows_merge/main67107_inputs_tail.rs" => Some(include_str!("../../src/source_reflows_merge/main67107_inputs_tail.rs")),
        "tests/native/string_settlement/codec_allocation/codec808_frontier.rs" => Some(include_str!("../../tests/native/string_settlement/codec_allocation/codec808_frontier.rs")),
        "crates/semaprax-native-rust-interop-builder/src/implementation/tests/ledger_capacity/source_inventory.rs" => Some(include_str!("../../crates/semaprax-native-rust-interop-builder/src/implementation/tests/ledger_capacity/source_inventory.rs")),
        "src/cleanup_plan/replay.rs" => Some(include_str!("../../src/cleanup_plan/replay.rs")),
        "src/cleanup_plan/replay/branch_clone/direct.rs" => Some(include_str!("../../src/cleanup_plan/replay/branch_clone/direct.rs")),
        "src/cleanup_plan/replay/branch_clone/direct/before.rs" => Some(include_str!("../../src/cleanup_plan/replay/branch_clone/direct/before.rs")),
        "src/cleanup_plan/replay/branch_clone/direct/tests.rs" => Some(include_str!("../../src/cleanup_plan/replay/branch_clone/direct/tests.rs")),
        "src/cleanup_plan/replay/compact_stage/main_f247_source_custody.rs" => Some(include_str!("../../src/cleanup_plan/replay/compact_stage/main_f247_source_custody.rs")),
        "src/cleanup_plan/replay/expression_facts/receiving/directory.rs" => Some(include_str!("../../src/cleanup_plan/replay/expression_facts/receiving/directory.rs")),
        "src/cleanup_plan/replay/expression_facts/receiving/directory/before_cursor.rs" => Some(include_str!("../../src/cleanup_plan/replay/expression_facts/receiving/directory/before_cursor.rs")),
        "src/cleanup_plan/replay/expression_facts/receiving/directory/before_cursor/qualification.rs" => Some(include_str!("../../src/cleanup_plan/replay/expression_facts/receiving/directory/before_cursor/qualification.rs")),
        "src/cleanup_plan/replay/expression_facts/receiving/directory/before_cursor/status_prefix.rs" => Some(include_str!("../../src/cleanup_plan/replay/expression_facts/receiving/directory/before_cursor/status_prefix.rs")),
        "src/cleanup_plan/replay/expression_facts/receiving/directory/status_prefix.rs" => Some(include_str!("../../src/cleanup_plan/replay/expression_facts/receiving/directory/status_prefix.rs")),
        "src/cleanup_plan/replay/expression_facts/receiving/directory/status_prefix/reused_cursor.rs" => Some(include_str!("../../src/cleanup_plan/replay/expression_facts/receiving/directory/status_prefix/reused_cursor.rs")),
        "src/cleanup_plan/replay/expression_facts/receiving/directory/status_prefix/reused_cursor/tests.rs" => Some(include_str!("../../src/cleanup_plan/replay/expression_facts/receiving/directory/status_prefix/reused_cursor/tests.rs")),
        "src/cleanup_plan/replay/factored/canonical_shared/before_selected_labels.rs" => Some(include_str!("../../src/cleanup_plan/replay/factored/canonical_shared/before_selected_labels.rs")),
        "src/cleanup_plan/replay/factored/canonical_shared/before_selected_labels/terminal_leaf.rs" => Some(include_str!("../../src/cleanup_plan/replay/factored/canonical_shared/before_selected_labels/terminal_leaf.rs")),
        "src/cleanup_plan/replay/factored/canonical_shared/selected_labels.rs" => Some(include_str!("../../src/cleanup_plan/replay/factored/canonical_shared/selected_labels.rs")),
        "src/cleanup_plan/replay/factored/canonical_shared/selected_labels/tests.rs" => Some(include_str!("../../src/cleanup_plan/replay/factored/canonical_shared/selected_labels/tests.rs")),
        "src/cleanup_plan/replay/merge_census.rs" => Some(include_str!("../../src/cleanup_plan/replay/merge_census.rs")),
        "src/cleanup_plan/replay/ordinary_plan_history/before_comparison.rs" => Some(include_str!("../../src/cleanup_plan/replay/ordinary_plan_history/before_comparison.rs")),
        "src/cleanup_plan/replay/ordinary_plan_history/comparison_owner.rs" => Some(include_str!("../../src/cleanup_plan/replay/ordinary_plan_history/comparison_owner.rs")),
        "src/cleanup_plan/replay/ordinary_plan_history/comparison_owner/tests.rs" => Some(include_str!("../../src/cleanup_plan/replay/ordinary_plan_history/comparison_owner/tests.rs")),
        "src/cleanup_plan/replay/owned_suffix_878.rs" => Some(include_str!("../../src/cleanup_plan/replay/owned_suffix_878.rs")),
        "src/cleanup_plan/replay/owned_suffix_878_vector.rs" => Some(include_str!("../../src/cleanup_plan/replay/owned_suffix_878_vector.rs")),
        "src/cleanup_plan/replay/skeleton_work.rs" => Some(include_str!("../../src/cleanup_plan/replay/skeleton_work.rs")),
        "src/cleanup_plan/replay/skeleton_work/isolated_paths.rs" => Some(include_str!("../../src/cleanup_plan/replay/skeleton_work/isolated_paths.rs")),
        "src/cleanup_plan/replay/skeleton_work/isolated_paths/tests.rs" => Some(include_str!("../../src/cleanup_plan/replay/skeleton_work/isolated_paths/tests.rs")),
        "src/cleanup_plan/replay/skeleton_work/isolated_paths/tests/before.rs" => Some(include_str!("../../src/cleanup_plan/replay/skeleton_work/isolated_paths/tests/before.rs")),
        "src/cleanup_plan/replay/skeleton_work/merge_keys_878.rs" => Some(include_str!("../../src/cleanup_plan/replay/skeleton_work/merge_keys_878.rs")),
        "src/cleanup_plan/replay/skeleton_work/merge_search.rs" => Some(include_str!("../../src/cleanup_plan/replay/skeleton_work/merge_search.rs")),
        "src/source_reflows.rs" => Some(include_str!("../../src/source_reflows.rs")),
        "src/source_reflows/main99f_controllers.rs" => Some(include_str!("../../src/source_reflows/main99f_controllers.rs")),
        "src/source_reflows/main_e177_controllers.rs" => Some(include_str!("../../src/source_reflows/main_e177_controllers.rs")),
        "src/source_reflows/main_e177_support.rs" => Some(include_str!("../../src/source_reflows/main_e177_support.rs")),
        "src/source_reflows/main_f247_controllers.rs" => Some(include_str!("../../src/source_reflows/main_f247_controllers.rs")),
        "src/source_reflows_merge.rs" => Some(include_str!("../../src/source_reflows_merge.rs")),
        "src/source_reflows_merge/main67107_inputs.rs" => Some(include_str!("../../src/source_reflows_merge/main67107_inputs.rs")),
        "src/source_reflows_merge/main99f_inputs.rs" => Some(include_str!("../../src/source_reflows_merge/main99f_inputs.rs")),
        "src/source_reflows_merge/main_e177_inputs.rs" => Some(include_str!("../../src/source_reflows_merge/main_e177_inputs.rs")),
        "src/source_reflows_merge/next_big_batch_inputs.rs" => Some(include_str!("../../src/source_reflows_merge/next_big_batch_inputs.rs")),
        "src/workspace_graph/tests/core_attempt_trace.rs" => Some(include_str!("../../src/workspace_graph/tests/core_attempt_trace.rs")),
        "tests/hir_module_boundaries.rs" => Some(include_str!("../../tests/hir_module_boundaries.rs")),
        "tests/wasm/scalar_browser_ci_contract/before_3e66.rs" => Some(include_str!("../../tests/wasm/scalar_browser_ci_contract/before_3e66.rs")),
        "tests/wasm/scalar_browser_ci_contract/incoming_main3e66.rs" => Some(include_str!("../../tests/wasm/scalar_browser_ci_contract/incoming_main3e66.rs")),
        "tests/workspace/pin_capture.rs" => Some(include_str!("../../tests/workspace/pin_capture.rs")),
        "tests/workspace/semantic_graph.rs" => Some(include_str!("../../tests/workspace/semantic_graph.rs")),
        "tests/workspace/semantic_graph/source_inventory.rs" => Some(include_str!("../../tests/workspace/semantic_graph/source_inventory.rs")),
        "src/cleanup_plan/replay/branch_clone/tests.rs" => Some(include_str!("../../src/cleanup_plan/replay/branch_clone/tests.rs")),
        "src/cleanup_plan/replay/expression_facts/receiving/directory/status_prefix/inline.rs" => Some(include_str!("../../src/cleanup_plan/replay/expression_facts/receiving/directory/status_prefix/inline.rs")),
        "src/cleanup_plan/replay/expression_facts/receiving/directory/status_prefix/inline/tests.rs" => Some(include_str!("../../src/cleanup_plan/replay/expression_facts/receiving/directory/status_prefix/inline/tests.rs")),
        "src/cleanup_plan/replay/expression_facts/receiving/retained.rs" => Some(include_str!("../../src/cleanup_plan/replay/expression_facts/receiving/retained.rs")),
        "src/cleanup_plan/replay/expression_facts/receiving/retained/inline_status.rs" => Some(include_str!("../../src/cleanup_plan/replay/expression_facts/receiving/retained/inline_status.rs")),
        "src/cleanup_plan/replay/expression_facts/receiving/retained/inline_status/tests.rs" => Some(include_str!("../../src/cleanup_plan/replay/expression_facts/receiving/retained/inline_status/tests.rs")),
        "src/cleanup_plan/replay/factored/canonical_shared.rs" => Some(include_str!("../../src/cleanup_plan/replay/factored/canonical_shared.rs")),
        "src/cleanup_plan/replay/factored/canonical_shared/arena_tests.rs" => Some(include_str!("../../src/cleanup_plan/replay/factored/canonical_shared/arena_tests.rs")),
        "src/cleanup_plan/replay/factored/canonical_shared/before_paid_many.rs" => Some(include_str!("../../src/cleanup_plan/replay/factored/canonical_shared/before_paid_many.rs")),
        "src/cleanup_plan/replay/factored/canonical_shared/before_paid_many/empty_memo.rs" => Some(include_str!("../../src/cleanup_plan/replay/factored/canonical_shared/before_paid_many/empty_memo.rs")),
        "src/cleanup_plan/replay/factored/canonical_shared/before_scope_memo.rs" => Some(include_str!("../../src/cleanup_plan/replay/factored/canonical_shared/before_scope_memo.rs")),
        "src/cleanup_plan/replay/factored/canonical_shared/before_selected_route.rs" => Some(include_str!("../../src/cleanup_plan/replay/factored/canonical_shared/before_selected_route.rs")),
        "src/cleanup_plan/replay/factored/canonical_shared/borrowed_labels.rs" => Some(include_str!("../../src/cleanup_plan/replay/factored/canonical_shared/borrowed_labels.rs")),
        "src/cleanup_plan/replay/factored/canonical_shared/empty_memo.rs" => Some(include_str!("../../src/cleanup_plan/replay/factored/canonical_shared/empty_memo.rs")),
        "src/cleanup_plan/replay/factored/canonical_shared/empty_memo/owned_tests.rs" => Some(include_str!("../../src/cleanup_plan/replay/factored/canonical_shared/empty_memo/owned_tests.rs")),
        "src/cleanup_plan/replay/factored/canonical_shared/intern_arena.rs" => Some(include_str!("../../src/cleanup_plan/replay/factored/canonical_shared/intern_arena.rs")),
        "src/cleanup_plan/replay/factored/canonical_shared/paid_many/tests.rs" => Some(include_str!("../../src/cleanup_plan/replay/factored/canonical_shared/paid_many/tests.rs")),
        "src/cleanup_plan/replay/factored/canonical_shared/paid_route_tests.rs" => Some(include_str!("../../src/cleanup_plan/replay/factored/canonical_shared/paid_route_tests.rs")),
        "src/cleanup_plan/replay/factored/canonical_shared/terminal_leaf.rs" => Some(include_str!("../../src/cleanup_plan/replay/factored/canonical_shared/terminal_leaf.rs")),
        "src/cleanup_plan/replay/factored/continuation_receiver.rs" => Some(include_str!("../../src/cleanup_plan/replay/factored/continuation_receiver.rs")),
        "src/cleanup_plan/replay/factored/oracle_hook.rs" => Some(include_str!("../../src/cleanup_plan/replay/factored/oracle_hook.rs")),
        "src/source_reflows/semantic_receivers.rs" => Some(include_str!("../../src/source_reflows/semantic_receivers.rs")),
        "src/source_reflows/semantic_receivers/status_inline.rs" => Some(include_str!("../../src/source_reflows/semantic_receivers/status_inline.rs")),
        "src/cleanup_plan/replay/skeleton_work/enumerated_prefix/tests.rs" => Some(include_str!("../../src/cleanup_plan/replay/skeleton_work/enumerated_prefix/tests.rs")),
        "platform-tests/wasm-scalar-browser-v1/test-fixtures.mjs" => Some(include_str!("../../platform-tests/wasm-scalar-browser-v1/test-fixtures.mjs")),
        "src/workspace_graph/type_proof_tests.rs" => Some(include_str!("../../src/workspace_graph/type_proof_tests.rs")),
        "src/cleanup_plan.rs" => Some(include_str!("../../src/cleanup_plan.rs")),
        "src/cleanup_plan/replay/factored.rs" => Some(include_str!("../../src/cleanup_plan/replay/factored.rs")),
        "src/cleanup_plan/replay/factored/canonical_shared/before_lexical_successors.rs" => Some(include_str!("../../src/cleanup_plan/replay/factored/canonical_shared/before_lexical_successors.rs")),
        "src/cleanup_plan/replay/factored/canonical_shared/before_lexical_successors/compare.rs" => Some(include_str!("../../src/cleanup_plan/replay/factored/canonical_shared/before_lexical_successors/compare.rs")),
        "src/cleanup_plan/replay/factored/canonical_shared/lexical_successors.rs" => Some(include_str!("../../src/cleanup_plan/replay/factored/canonical_shared/lexical_successors.rs")),
        "src/cleanup_plan/replay/factored/canonical_shared/lexical_successors/census.rs" => Some(include_str!("../../src/cleanup_plan/replay/factored/canonical_shared/lexical_successors/census.rs")),
        "src/cleanup_plan/replay/factored/canonical_shared/lexical_successors/frame_loop.rs" => Some(include_str!("../../src/cleanup_plan/replay/factored/canonical_shared/lexical_successors/frame_loop.rs")),
        "src/cleanup_plan/replay/factored/canonical_shared/lexical_successors/frame_loop/before_borrowed_single.rs" => Some(include_str!("../../src/cleanup_plan/replay/factored/canonical_shared/lexical_successors/frame_loop/before_borrowed_single.rs")),
        "src/cleanup_plan/replay/factored/canonical_shared/lexical_successors/frame_loop/borrowed_single.rs" => Some(include_str!("../../src/cleanup_plan/replay/factored/canonical_shared/lexical_successors/frame_loop/borrowed_single.rs")),
        "src/cleanup_plan/replay/factored/canonical_shared/lexical_successors/frame_loop/borrowed_single/tests.rs" => Some(include_str!("../../src/cleanup_plan/replay/factored/canonical_shared/lexical_successors/frame_loop/borrowed_single/tests.rs")),
        "src/cleanup_plan/replay/factored/canonical_shared/lexical_successors/tests.rs" => Some(include_str!("../../src/cleanup_plan/replay/factored/canonical_shared/lexical_successors/tests.rs")),
        "src/cleanup_plan/replay/ordinary_plan_history.rs" => Some(include_str!("../../src/cleanup_plan/replay/ordinary_plan_history.rs")),
        "src/cleanup_plan/replay/ordinary_plan_history/comparison_owner/before_lexical_cursor.rs" => Some(include_str!("../../src/cleanup_plan/replay/ordinary_plan_history/comparison_owner/before_lexical_cursor.rs")),
        "src/cleanup_plan/replay/ordinary_plan_history/comparison_owner/lexical_cursor.rs" => Some(include_str!("../../src/cleanup_plan/replay/ordinary_plan_history/comparison_owner/lexical_cursor.rs")),
        "src/cleanup_plan/replay/owned_suffix.rs" => Some(include_str!("../../src/cleanup_plan/replay/owned_suffix.rs")),
        "src/cleanup_plan/replay/owned_suffix/tests.rs" => Some(include_str!("../../src/cleanup_plan/replay/owned_suffix/tests.rs")),
        "src/cleanup_plan/replay/skeleton_work/merge_keys_tests.rs" => Some(include_str!("../../src/cleanup_plan/replay/skeleton_work/merge_keys_tests.rs")),
        "src/cleanup_plan/replay/skeleton_work/ordinary_dag.rs" => Some(include_str!("../../src/cleanup_plan/replay/skeleton_work/ordinary_dag.rs")),
        "src/workspace_graph/expected_projection/held_projection.rs" => Some(include_str!("../../src/workspace_graph/expected_projection/held_projection.rs")),
        "src/workspace_graph/static_text/scalar_helpers/capture_directory/tests.rs" => Some(include_str!("../../src/workspace_graph/static_text/scalar_helpers/capture_directory/tests.rs")),
        "src/workspace_graph/tests/builder_limits.rs" => Some(include_str!("../../src/workspace_graph/tests/builder_limits.rs")),
        "tests/workspace/semantic_graph/source_inventory_controls.rs" => Some(include_str!("../../tests/workspace/semantic_graph/source_inventory_controls.rs")),
        "src/cleanup_plan/replay/expression_facts/arena.rs" => Some(include_str!("../../src/cleanup_plan/replay/expression_facts/arena.rs")),
        "src/cleanup_plan/replay/expression_facts/arena/prefix.rs" => Some(include_str!("../../src/cleanup_plan/replay/expression_facts/arena/prefix.rs")),
        "src/cleanup_plan/replay/expression_facts/arena/prefix/retained_before.rs" => Some(include_str!("../../src/cleanup_plan/replay/expression_facts/arena/prefix/retained_before.rs")),
        "src/cleanup_plan/replay/expression_facts/arena/prefix/tests.rs" => Some(include_str!("../../src/cleanup_plan/replay/expression_facts/arena/prefix/tests.rs")),
        "src/cleanup_plan/replay/expression_facts/arena/retained.rs" => Some(include_str!("../../src/cleanup_plan/replay/expression_facts/arena/retained.rs")),
        "src/cleanup_plan/replay/expression_facts/arena/retained/tests.rs" => Some(include_str!("../../src/cleanup_plan/replay/expression_facts/arena/retained/tests.rs")),
        "src/cleanup_plan/replay/expression_facts/receiving/retained/case_certified/lookup_before.rs" => Some(include_str!("../../src/cleanup_plan/replay/expression_facts/receiving/retained/case_certified/lookup_before.rs")),
        "src/cleanup_plan/replay/expression_facts/receiving/retained/case_certified/lookup_caller_before.rs" => Some(include_str!("../../src/cleanup_plan/replay/expression_facts/receiving/retained/case_certified/lookup_caller_before.rs")),
        "src/cleanup_plan/replay/expression_facts/receiving/retained/case_certified/lookup_tests.rs" => Some(include_str!("../../src/cleanup_plan/replay/expression_facts/receiving/retained/case_certified/lookup_tests.rs")),
        "src/cleanup_plan/replay/expression_facts/receiving/retained/case_certified/retained_lookup.rs" => Some(include_str!("../../src/cleanup_plan/replay/expression_facts/receiving/retained/case_certified/retained_lookup.rs")),
        "src/cleanup_plan/replay/factored/canonical_shared/before_paid_many/compare.rs" => Some(include_str!("../../src/cleanup_plan/replay/factored/canonical_shared/before_paid_many/compare.rs")),
        "src/cleanup_plan/replay/factored/canonical_shared/lexical_child_scratch/current_tests.rs" => Some(include_str!("../../src/cleanup_plan/replay/factored/canonical_shared/lexical_child_scratch/current_tests.rs")),
        "src/cleanup_plan/replay/factored/canonical_shared/lexical_child_scratch/matched.rs" => Some(include_str!("../../src/cleanup_plan/replay/factored/canonical_shared/lexical_child_scratch/matched.rs")),
        "src/cleanup_plan/replay/factored/canonical_shared/lexical_successors/frame_loop/before_arity_dispatch.rs" => Some(include_str!("../../src/cleanup_plan/replay/factored/canonical_shared/lexical_successors/frame_loop/before_arity_dispatch.rs")),
        "src/cleanup_plan/replay/ordinary_plan_history/comparison_owner/before_pair_scope/lexical_cursor.rs" => Some(include_str!("../../src/cleanup_plan/replay/ordinary_plan_history/comparison_owner/before_pair_scope/lexical_cursor.rs")),
        "src/cleanup_plan/replay/ordinary_plan_history/comparison_owner/before_pair_scope/mod.rs" => Some(include_str!("../../src/cleanup_plan/replay/ordinary_plan_history/comparison_owner/before_pair_scope/mod.rs")),
        "src/cleanup_plan/replay/ordinary_plan_history/comparison_owner/pair_scope_census.rs" => Some(include_str!("../../src/cleanup_plan/replay/ordinary_plan_history/comparison_owner/pair_scope_census.rs")),
        "src/cleanup_plan/replay/ordinary_plan_history/comparison_owner/pair_scope_tests.rs" => Some(include_str!("../../src/cleanup_plan/replay/ordinary_plan_history/comparison_owner/pair_scope_tests.rs")),
        "src/source_reflows_merge/main_c883_inputs.rs" => Some(include_str!("../../src/source_reflows_merge/main_c883_inputs.rs")),
        "src/workspace_graph/shape_identity/before785.rs" => Some(include_str!("../../src/workspace_graph/shape_identity/before785.rs")),
        "src/workspace_graph/shape_identity/before_caller785.rs" => Some(include_str!("../../src/workspace_graph/shape_identity/before_caller785.rs")),
        "src/workspace_graph/shape_identity/before_validation785.rs" => Some(include_str!("../../src/workspace_graph/shape_identity/before_validation785.rs")),
        "src/workspace_graph/shape_identity/source_controls.rs" => Some(include_str!("../../src/workspace_graph/shape_identity/source_controls.rs")),
        "src/workspace_graph/shape_identity/tests.rs" => Some(include_str!("../../src/workspace_graph/shape_identity/tests.rs")),
        "src/workspace_graph/tests.rs" => Some(include_str!("../../src/workspace_graph/tests.rs")),
        _ => None,
    }
}
fn read_view(view: &serde_json::Value) -> String {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    held(
        &root.join(view["archive"].as_str().unwrap()),
        view["length"].as_u64().unwrap() as usize,
        view["sha256"].as_str().unwrap(),
    )
    .unwrap()
}
fn actual(row: &serde_json::Value) -> String {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let source = row["source"].as_str().unwrap();
    let current = &row["current"];
    let value = held(
        &root.join(source),
        current["length"].as_u64().unwrap() as usize,
        current["sha256"].as_str().unwrap(),
    )
    .unwrap();
    if row["compiled"].as_bool().unwrap() {
        assert_eq!(
            Some(value.as_str()),
            compiled(source),
            "complete actual compiled source"
        );
    }
    assert_eq!(value, read_view(current), "complete actual archive");
    if let Some(children) = row.get("fragments") {
        for child in children.as_array().unwrap() {
            assert!(child.get("fragments").is_none());
            // Authenticate actual same-scope compiled bytes before returning
            // the shortened parent's complete historical source view.
            actual(child);
        }
    }

    value
}
pub(crate) fn file(path: &Path, length: usize, expected: &str) -> Option<String> {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let relative = path.strip_prefix(root).ok()?;
    if !relative
        .components()
        .all(|p| matches!(p, std::path::Component::Normal(_)))
    {
        return None;
    }
    let source = relative.to_str()?;
    let packet = packet();
    let row = packet["rows"]
        .as_array()
        .unwrap()
        .iter()
        .find(|r| r["source"] == source)?;
    let prior = row["views"]
        .as_array()
        .unwrap()
        .iter()
        .find(|v| v["length"].as_u64() == Some(length as u64) && v["sha256"] == expected)?;
    actual(row);
    Some(read_view(prior))
}
pub(crate) fn value(source: &str, expected: &str, supplied: &str) -> String {
    let packet = packet();
    let Some(row) = packet["rows"]
        .as_array()
        .unwrap()
        .iter()
        .find(|r| r["source"] == source)
    else {
        return crate::source_reflows_merge::next_big_batch_inputs::value_local(
            source, expected, supplied,
        );
    };
    let Some(prior) = row["views"]
        .as_array()
        .unwrap()
        .iter()
        .find(|v| v["sha256"] == expected)
    else {
        return supplied.to_owned();
    };
    let current = actual(row);
    let previous = read_view(prior);
    if supplied == current || supplied == previous {
        previous
    } else {
        supplied.to_owned()
    }
}
#[test]
fn latest_main_and_afa_complete_views_preserve_all_prior_controls_and_refuse_wrong_stages() {
    let packet = packet();
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let mut seen = std::collections::BTreeSet::new();
    for row in packet["rows"].as_array().unwrap() {
        let source = row["source"].as_str().unwrap();
        assert!(seen.insert(source));
        let current = actual(row);
        for prior in row["views"].as_array().unwrap() {
            let expected = prior["sha256"].as_str().unwrap();
            let length = prior["length"].as_u64().unwrap() as usize;
            let previous = read_view(prior);
            assert_eq!(
                file(&root.join(source), length, expected),
                Some(previous.clone())
            );
            assert_eq!(value(source, expected, &current), previous);
            assert_eq!(value(source, expected, &previous), previous);
            for foreign in [
                String::new(),
                format!("{current} "),
                format!("{current}{current}"),
                current[..current.len() - 1].to_owned(),
                "foreign".to_owned(),
            ] {
                assert_eq!(value(source, expected, &foreign), foreign);
            }
            assert!(file(&root.join(source), length + 1, expected).is_none());
            assert!(file(&root.join(source), length, &"0".repeat(64)).is_none());
        }
    }
    assert!(file(&root.join("foreign"), 0, "foreign").is_none());
    assert!(file(&root.join("src/../Cargo.toml"), 0, "foreign").is_none());
}
// Independent complete785/885/886 side views. These never feed a current raw audit.
fn combined_side_packet() -> serde_json::Value {
    let data = include_str!(
        "../../docs/evidence/next-large-side-source-composition-20261017/side-inputs.json"
    );
    assert_eq!(data.len(), 18033);
    assert_eq!(
        digest(data.as_bytes()),
        "c185ed85f164ff3295e55d29d69e47de8616d3a582ffe359abb959b5d693713c"
    );
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    assert_eq!(
        held(
            &root
                .join("docs/evidence/next-large-side-source-composition-20261017/side-inputs.json"),
            data.len(),
            &digest(data.as_bytes())
        )
        .unwrap(),
        data
    );
    let value: serde_json::Value = serde_json::from_str(data).unwrap();
    assert_eq!(
        value["schema"],
        "semaprax.next-large-complete-side-projections.v1"
    );
    assert_eq!(value["rows"].as_array().unwrap().len(), 9);
    value
}
fn combined_side_compiled(source: &str) -> Option<&'static str> {
    match source {
        "src/cleanup_plan/replay/factored/canonical_shared/before_paid_many/compare.rs" => Some(include_str!("../../src/cleanup_plan/replay/factored/canonical_shared/before_paid_many/compare.rs")),
        "src/source_reflows_merge/main_c883_inputs.rs" => Some(include_str!("../../src/source_reflows_merge/main_c883_inputs.rs")),
        "src/cleanup_plan/replay/expression_facts/arena.rs" => Some(include_str!("../../src/cleanup_plan/replay/expression_facts/arena.rs")),
        "src/cleanup_plan/replay/expression_facts/arena/prefix.rs" => Some(include_str!("../../src/cleanup_plan/replay/expression_facts/arena/prefix.rs")),
        "src/cleanup_plan/replay/expression_facts/arena/prefix/tests.rs" => Some(include_str!("../../src/cleanup_plan/replay/expression_facts/arena/prefix/tests.rs")),
        _ => compiled(source),
    }
}
fn combined_side_actual(row: &serde_json::Value) -> String {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let source = row["source"].as_str().unwrap();
    let current = &row["current"];
    let actual = held(
        &root.join(source),
        current["length"].as_u64().unwrap() as usize,
        current["sha256"].as_str().unwrap(),
    )
    .unwrap();
    assert_eq!(
        Some(actual.as_str()),
        combined_side_compiled(source),
        "complete combined current compiled source"
    );
    assert_eq!(
        actual,
        read_view(current),
        "complete combined raw-carrier archive"
    );
    actual
}
pub(crate) fn combined_side_value(source: &str, view: &str, supplied: &str) -> String {
    let packet = combined_side_packet();
    let Some(row) = packet["rows"]
        .as_array()
        .unwrap()
        .iter()
        .find(|row| row["source"] == source)
    else {
        return supplied.to_owned();
    };
    let views = row["views"].as_array().unwrap();
    assert_eq!(views.iter().filter(|old| old["name"] == view).count(), 1);
    let previous = read_view(views.iter().find(|old| old["name"] == view).unwrap());
    let actual = combined_side_actual(row);
    if supplied == actual || supplied == previous {
        previous
    } else {
        supplied.to_owned()
    }
}
pub(crate) fn combined_side_expected_value(source: &str, expected: &str, supplied: &str) -> String {
    let packet = combined_side_packet();
    let Some(row) = packet["rows"]
        .as_array()
        .unwrap()
        .iter()
        .find(|row| row["source"] == source)
    else {
        return supplied.to_owned();
    };
    let Some(previous) = row["views"]
        .as_array()
        .unwrap()
        .iter()
        .find(|view| view["sha256"] == expected)
    else {
        return supplied.to_owned();
    };
    let old = read_view(previous);
    let actual = combined_side_actual(row);
    if supplied == actual || supplied == old {
        old
    } else {
        supplied.to_owned()
    }
}
pub(crate) fn combined_side_file(path: &Path, length: usize, expected: &str) -> Option<String> {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let relative = path.strip_prefix(root).ok()?;
    if !relative
        .components()
        .all(|part| matches!(part, std::path::Component::Normal(_)))
    {
        return None;
    }
    let source = relative.to_str()?;
    let packet = combined_side_packet();
    let row = packet["rows"]
        .as_array()
        .unwrap()
        .iter()
        .find(|row| row["source"] == source)?;
    let previous =
        row["views"].as_array().unwrap().iter().find(|old| {
            old["length"].as_u64() == Some(length as u64) && old["sha256"] == expected
        })?;
    combined_side_actual(row);
    Some(read_view(previous))
}
#[test]
fn combined_785_885_and_projection_carriers_authenticate_each_whole_side_before_old_guards() {
    let packet = combined_side_packet();
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    for row in packet["rows"].as_array().unwrap() {
        let source = row["source"].as_str().unwrap();
        let current = combined_side_actual(row);
        for previous in row["views"].as_array().unwrap() {
            let old = read_view(previous);
            let name = previous["name"].as_str().unwrap();
            let sha = previous["sha256"].as_str().unwrap();
            let length = previous["length"].as_u64().unwrap() as usize;
            assert_eq!(combined_side_value(source, name, &current), old);
            assert_eq!(combined_side_value(source, name, &old), old);
            assert_eq!(
                combined_side_file(&root.join(source), length, sha),
                Some(old)
            );
            for foreign in [
                String::new(),
                format!("{current} "),
                format!("{current}{current}"),
                current[..current.len() - 1].to_owned(),
                "foreign".to_owned(),
            ] {
                assert_eq!(combined_side_value(source, name, &foreign), foreign);
            }
            assert!(combined_side_file(&root.join(source), length + 1, sha).is_none());
            assert!(combined_side_file(&root.join(source), length, &"0".repeat(64)).is_none());
        }
    }
    assert_eq!(
        combined_side_value("unlisted", "whole785", "unchanged"),
        "unchanged"
    );
    assert!(combined_side_file(&root.join("unlisted"), 0, "unlisted").is_none());
}

// Nondelegating entry for the older owner's compiled-channel normalization.
// Membership is tested before calling the public delegating route.
pub(crate) fn value_direct(source: &str, expected: &str, supplied: &str) -> String {
    let current = packet();
    if current["rows"]
        .as_array()
        .unwrap()
        .iter()
        .any(|row| row["source"] == source)
    {
        value(source, expected, supplied)
    } else {
        supplied.to_owned()
    }
}
