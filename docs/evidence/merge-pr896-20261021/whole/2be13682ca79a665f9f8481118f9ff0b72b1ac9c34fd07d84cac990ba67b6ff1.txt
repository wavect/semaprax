//! Closed final-batch historical acquisition. No production authority or raw-audit projection.
use sha2::{Digest, Sha256};
use std::path::Path;
const SOURCE_CAP: usize = 9730253;
fn digest(bytes: &[u8]) -> String {
    format!("{:x}", crate::digest_hex::LowerHex(Sha256::digest(bytes)))
}
fn packet() -> serde_json::Value {
    let data = include_str!(
        "../../docs/evidence/next-big-batch-source-binding-20261017/current-inputs.json"
    );
    assert_eq!(data.len(), 112927);
    let compiled_digest = digest(data.as_bytes());
    assert_eq!(
        compiled_digest,
        "336d79ff6cddc852d4750aa304b85a31494ac98c2e015bb52dfb615db7d98fe3"
    );
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let physical = held(
        &root.join("docs/evidence/next-big-batch-source-binding-20261017/current-inputs.json"),
        data.len(),
        &compiled_digest,
    )
    .unwrap();
    assert_eq!(
        physical, data,
        "complete physical and compiled current manifest"
    );
    let packet: serde_json::Value = serde_json::from_str(data).unwrap();
    assert_eq!(packet["schema"], "semaprax.next-big-batch-whole-inputs.v1");
    assert_eq!(packet["rows"].as_array().unwrap().len(), 54);
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
        "docs/evidence/earned-managed-pin42-source-coupling-20261017/inputs.json" => Some(include_str!("../../docs/evidence/earned-managed-pin42-source-coupling-20261017/inputs.json")),
        "docs/evidence/future831-f247-final-source-binding-20261017/current-inputs.json" => Some(include_str!("../../docs/evidence/future831-f247-final-source-binding-20261017/current-inputs.json")),
        "docs/evidence/main-c883-next-source-merge-20261017/main-inputs.json" => Some(include_str!("../../docs/evidence/main-c883-next-source-merge-20261017/main-inputs.json")),
        "docs/evidence/main-1e-next-source-merge-20261017/main-inputs.json" => Some(include_str!("../../docs/evidence/main-1e-next-source-merge-20261017/main-inputs.json")),
        "docs/evidence/future831-f247-final-source-binding-20261017/main-f247-inputs.json" => Some(include_str!("../../docs/evidence/future831-f247-final-source-binding-20261017/main-f247-inputs.json")),
        "docs/evidence/main99f-full-controller-acquisition-20261017/inputs.json" => Some(include_str!("../../docs/evidence/main99f-full-controller-acquisition-20261017/inputs.json")),
        _ => None,
    }
}
fn compiled(source: &str) -> Option<&'static str> {
    match source {
        "crates/semaprax-native-rust-interop-builder/src/implementation/tests/ledger_capacity/source_inventory.rs" => Some(include_str!("../../crates/semaprax-native-rust-interop-builder/src/implementation/tests/ledger_capacity/source_inventory.rs")),
        "src/bounded_output/stage_debits/route_debits.rs" => Some(include_str!("../../src/bounded_output/stage_debits/route_debits.rs")),
        "src/cleanup_plan/replay.rs" => Some(include_str!("../../src/cleanup_plan/replay.rs")),
        "src/cleanup_plan/replay/branch_clone.rs" => Some(include_str!("../../src/cleanup_plan/replay/branch_clone.rs")),
        "src/cleanup_plan/replay/branch_clone/direct.rs" => Some(include_str!("../../src/cleanup_plan/replay/branch_clone/direct.rs")),
        "src/cleanup_plan/replay/branch_clone/direct/before.rs" => Some(include_str!("../../src/cleanup_plan/replay/branch_clone/direct/before.rs")),
        "src/cleanup_plan/replay/branch_clone/direct/tests.rs" => Some(include_str!("../../src/cleanup_plan/replay/branch_clone/direct/tests.rs")),
        "src/cleanup_plan/replay/branch_clone/production_capacity.rs" => Some(include_str!("../../src/cleanup_plan/replay/branch_clone/production_capacity.rs")),
        "src/cleanup_plan/replay/compact_stage.rs" => Some(include_str!("../../src/cleanup_plan/replay/compact_stage.rs")),
        "src/cleanup_plan/replay/compact_stage/main_f247_source_custody.rs" => Some(include_str!("../../src/cleanup_plan/replay/compact_stage/main_f247_source_custody.rs")),
        "src/cleanup_plan/replay/expression_facts/receiving/directory/status_prefix.rs" => Some(include_str!("../../src/cleanup_plan/replay/expression_facts/receiving/directory/status_prefix.rs")),
        "src/cleanup_plan/replay/factored.rs" => Some(include_str!("../../src/cleanup_plan/replay/factored.rs")),
        "src/cleanup_plan/replay/factored/canonical_shared.rs" => Some(include_str!("../../src/cleanup_plan/replay/factored/canonical_shared.rs")),
        "src/cleanup_plan/replay/factored/canonical_shared/before_selected_labels.rs" => Some(include_str!("../../src/cleanup_plan/replay/factored/canonical_shared/before_selected_labels.rs")),
        "src/cleanup_plan/replay/factored/canonical_shared/before_selected_labels/terminal_leaf.rs" => Some(include_str!("../../src/cleanup_plan/replay/factored/canonical_shared/before_selected_labels/terminal_leaf.rs")),
        "src/cleanup_plan/replay/factored/canonical_shared/paid_many/tests.rs" => Some(include_str!("../../src/cleanup_plan/replay/factored/canonical_shared/paid_many/tests.rs")),
        "src/cleanup_plan/replay/factored/canonical_shared/selected_labels.rs" => Some(include_str!("../../src/cleanup_plan/replay/factored/canonical_shared/selected_labels.rs")),
        "src/cleanup_plan/replay/factored/canonical_shared/selected_labels/tests.rs" => Some(include_str!("../../src/cleanup_plan/replay/factored/canonical_shared/selected_labels/tests.rs")),
        "src/cleanup_plan/replay/ordinary_plan_history.rs" => Some(include_str!("../../src/cleanup_plan/replay/ordinary_plan_history.rs")),
        "src/cleanup_plan/replay/ordinary_plan_history/before_comparison.rs" => Some(include_str!("../../src/cleanup_plan/replay/ordinary_plan_history/before_comparison.rs")),
        "src/cleanup_plan/replay/ordinary_plan_history/comparison_owner.rs" => Some(include_str!("../../src/cleanup_plan/replay/ordinary_plan_history/comparison_owner.rs")),
        "src/cleanup_plan/replay/ordinary_plan_history/comparison_owner/tests.rs" => Some(include_str!("../../src/cleanup_plan/replay/ordinary_plan_history/comparison_owner/tests.rs")),
        "src/cleanup_plan/replay/ordinary_plan_history/frontier_tests.rs" => Some(include_str!("../../src/cleanup_plan/replay/ordinary_plan_history/frontier_tests.rs")),
        "src/cleanup_plan/replay/skeleton_work.rs" => Some(include_str!("../../src/cleanup_plan/replay/skeleton_work.rs")),
        "src/cleanup_plan/replay/skeleton_work/dag_storage/tests/tail_moves.rs" => Some(include_str!("../../src/cleanup_plan/replay/skeleton_work/dag_storage/tests/tail_moves.rs")),
        "src/cleanup_plan/replay/skeleton_work/dag_storage/tests/tail_moves/old.rs" => Some(include_str!("../../src/cleanup_plan/replay/skeleton_work/dag_storage/tests/tail_moves/old.rs")),
        "src/cleanup_plan/replay/skeleton_work/enumerated_prefix/tests.rs" => Some(include_str!("../../src/cleanup_plan/replay/skeleton_work/enumerated_prefix/tests.rs")),
        "src/cleanup_plan/replay/skeleton_work/isolated_paths.rs" => Some(include_str!("../../src/cleanup_plan/replay/skeleton_work/isolated_paths.rs")),
        "src/cleanup_plan/replay/skeleton_work/isolated_paths/tests.rs" => Some(include_str!("../../src/cleanup_plan/replay/skeleton_work/isolated_paths/tests.rs")),
        "src/cleanup_plan/replay/skeleton_work/isolated_paths/tests/before.rs" => Some(include_str!("../../src/cleanup_plan/replay/skeleton_work/isolated_paths/tests/before.rs")),
        "src/source_reflows.rs" => Some(include_str!("../../src/source_reflows.rs")),
        "src/source_reflows/main99f_controllers.rs" => Some(include_str!("../../src/source_reflows/main99f_controllers.rs")),
        "src/source_reflows/main_f247_controllers.rs" => Some(include_str!("../../src/source_reflows/main_f247_controllers.rs")),
        "src/source_reflows/semantic_receivers.rs" => Some(include_str!("../../src/source_reflows/semantic_receivers.rs")),
        "src/source_reflows_merge.rs" => Some(include_str!("../../src/source_reflows_merge.rs")),
        "src/source_reflows_merge/main67107_inputs.rs" => Some(include_str!("../../src/source_reflows_merge/main67107_inputs.rs")),
        "src/source_reflows_merge/main99f_inputs.rs" => Some(include_str!("../../src/source_reflows_merge/main99f_inputs.rs")),
        "src/source_reflows_merge/main_c883_inputs.rs" => Some(include_str!("../../src/source_reflows_merge/main_c883_inputs.rs")),
        "src/workspace_graph/initial_report/build.rs" => Some(include_str!("../../src/workspace_graph/initial_report/build.rs")),
        "src/workspace_graph/static_text/scalar_helpers.rs" => Some(include_str!("../../src/workspace_graph/static_text/scalar_helpers.rs")),
        "src/workspace_graph/static_text/scalar_helpers/capture_directory/tests.rs" => Some(include_str!("../../src/workspace_graph/static_text/scalar_helpers/capture_directory/tests.rs")),
        "src/workspace_graph/tests.rs" => Some(include_str!("../../src/workspace_graph/tests.rs")),
        "src/workspace_graph/tests/builder_limits.rs" => Some(include_str!("../../src/workspace_graph/tests/builder_limits.rs")),
        "src/workspace_graph/tests/core_attempt_trace.rs" => Some(include_str!("../../src/workspace_graph/tests/core_attempt_trace.rs")),
        "src/workspace_graph/type_proof_tests.rs" => Some(include_str!("../../src/workspace_graph/type_proof_tests.rs")),
        "tests/hir_module_boundaries.rs" => Some(include_str!("../../tests/hir_module_boundaries.rs")),
        "tests/wasm/scalar_browser_ci_contract.rs" => Some(include_str!("../../tests/wasm/scalar_browser_ci_contract.rs")),
        "tests/wasm/scalar_browser_ci_contract/before_3e66.rs" => Some(include_str!("../../tests/wasm/scalar_browser_ci_contract/before_3e66.rs")),
        "tests/wasm/scalar_browser_ci_contract/incoming_main3e66.rs" => Some(include_str!("../../tests/wasm/scalar_browser_ci_contract/incoming_main3e66.rs")),
        "tests/workspace/semantic_change/reporting.rs" => Some(include_str!("../../tests/workspace/semantic_change/reporting.rs")),
        "tests/workspace/semantic_graph.rs" => Some(include_str!("../../tests/workspace/semantic_graph.rs")),
        "tests/workspace/semantic_graph/source_inventory.rs" => Some(include_str!("../../tests/workspace/semantic_graph/source_inventory.rs")),
        "tests/workspace/semantic_graph/source_inventory_controls.rs" => Some(include_str!("../../tests/workspace/semantic_graph/source_inventory_controls.rs")),
        "tests/workspace/semantic_structural_change/reporting.rs" => Some(include_str!("../../tests/workspace/semantic_structural_change/reporting.rs")),
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
    let value = crate::source_reflows_merge::main_e177_final_inputs::combined_side_file(
        &root.join(source),
        current["length"].as_u64().unwrap() as usize,
        current["sha256"].as_str().unwrap(),
    )
    .or_else(|| {
        crate::source_reflows_merge::main_e177_final_inputs::file(
            &root.join(source),
            current["length"].as_u64().unwrap() as usize,
            current["sha256"].as_str().unwrap(),
        )
    })
    .unwrap_or_else(|| {
        held(
            &root.join(source),
            current["length"].as_u64().unwrap() as usize,
            current["sha256"].as_str().unwrap(),
        )
        .unwrap()
    });
    if row["compiled"].as_bool().unwrap() {
        let compiled =
            crate::source_reflows_merge::main_e177_final_inputs::combined_side_expected_value(
                source,
                current["sha256"].as_str().unwrap(),
                compiled(source).unwrap(),
            );
        let compiled = crate::source_reflows_merge::main_e177_final_inputs::value_direct(
            source,
            current["sha256"].as_str().unwrap(),
            &compiled,
        );
        assert_eq!(value, compiled, "complete actual compiled source");
    }
    assert_eq!(value, read_view(current), "complete actual archive");
    value
}
pub(crate) fn file(path: &Path, length: usize, expected: &str) -> Option<String> {
    if let Some(value) = crate::source_reflows_merge::main_e177_final_inputs::combined_side_file(
        path, length, expected,
    ) {
        return Some(value);
    }
    if let Some(value) =
        crate::source_reflows_merge::main_e177_final_inputs::file(path, length, expected)
    {
        return Some(value);
    }
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
    let side = crate::source_reflows_merge::main_e177_final_inputs::combined_side_expected_value(
        source, expected, supplied,
    );
    let outer = crate::source_reflows_merge::main_e177_final_inputs::value(source, expected, &side);
    let supplied = outer.as_str();
    let packet = packet();
    let Some(row) = packet["rows"]
        .as_array()
        .unwrap()
        .iter()
        .find(|r| r["source"] == source)
    else {
        return supplied.to_owned();
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
    let supplied = crate::source_reflows_merge::main_e177_final_inputs::value(
        source,
        row["current"]["sha256"].as_str().unwrap(),
        supplied,
    );
    if supplied == current || supplied == previous {
        previous
    } else {
        supplied.to_owned()
    }
}
pub(crate) fn value_local(source: &str, expected: &str, supplied: &str) -> String {
    let side = crate::source_reflows_merge::main_e177_final_inputs::combined_side_expected_value(
        source, expected, supplied,
    );
    let supplied = side.as_str();
    let packet = packet();
    let Some(row) = packet["rows"]
        .as_array()
        .unwrap()
        .iter()
        .find(|r| r["source"] == source)
    else {
        return supplied.to_owned();
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
    let supplied = crate::source_reflows_merge::main_e177_final_inputs::value_direct(
        source,
        row["current"]["sha256"].as_str().unwrap(),
        supplied,
    );
    if supplied == current || supplied == previous {
        previous
    } else {
        supplied.to_owned()
    }
}
#[test]
fn complete_current_and_parent_views_refuse_foreign_truncated_duplicate_and_wrong_stages() {
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

    // The newest packet deliberately has no legacy row. Its previous owner
    // must still authenticate and return the complete frozen 90-row view.
    {
        let source = "docs/evidence/projection-audit-scope-v1/legacy-inputs.tsv";
        let expected = "fb307a41c4702828798accb2a7abf4fa0fb78da7d0d1782fe6f5fb2135ba7ddb";
        let physical =
            include_str!("../../docs/evidence/projection-audit-scope-v1/legacy-inputs.tsv");
        let previous = include_str!("../../docs/evidence/main-e177-final-source-binding-20261017/views/fb307a41c4702828798accb2a7abf4fa0fb78da7d0d1782fe6f5fb2135ba7ddb.txt");
        assert_eq!(value(source, expected, physical), previous);
        assert_eq!(previous.lines().count(), 90);
        for invalid in [format!("{physical}\n"), format!("!{physical}")] {
            assert_eq!(value(source, expected, &invalid), invalid);
        }
        assert_eq!(value("foreign/source.tsv", expected, physical), physical);
        assert_eq!(value(source, &"0".repeat(64), physical), physical);
    }
}

#[test]
fn final_only_membership_precedes_the_complete_older_table_without_accepting_foreign_values() {
    let source = "docs/evidence/projection-audit-scope-v1/legacy-inputs.tsv";
    let expected = "fb307a41c4702828798accb2a7abf4fa0fb78da7d0d1782fe6f5fb2135ba7ddb";
    let previous=include_str!("../../docs/evidence/main-e177-final-source-binding-20261017/views/fb307a41c4702828798accb2a7abf4fa0fb78da7d0d1782fe6f5fb2135ba7ddb.txt");
    let current = include_str!("../../docs/evidence/projection-audit-scope-v1/legacy-inputs.tsv");
    assert_eq!(previous.lines().count(), 90);
    assert_ne!(current, previous);
    assert!(!packet()["rows"]
        .as_array()
        .unwrap()
        .iter()
        .any(|row| row["source"] == source));
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join(source);
    assert_eq!(
        file(&path, previous.len(), expected).as_deref(),
        Some(previous)
    );
    assert_eq!(value(source, expected, current), previous);
    assert_eq!(value(source, expected, previous), previous);
    for foreign in [
        String::new(),
        format!("{current} "),
        format!("{current}{current}"),
        "foreign".to_owned(),
    ] {
        assert_eq!(value(source, expected, &foreign), foreign);
    }
    assert!(file(&path, previous.len() + 1, expected).is_none());
    assert!(file(&path, previous.len(), &"0".repeat(64)).is_none());
}
