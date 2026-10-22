#[test]
fn owned_compile35f_current_acquisition_preserves_complete_seven_subjects_and_refuses_foreign() {
    let source = OWNED_COMPILE35F_PATH;
    let previous = owned_compile35f_input(source, OWNED_COMPILE35F_RAW);
    assert_eq!(
        digest(&previous),
        "fa95c61e8eb3ba3c300c4fee92e3da1777ad360483309c6f0ac9c2c5cf46e956"
    );
    assert_eq!(owned_compile35f_input(source, &previous), previous);
    let rows = compile35f_records();
    let row = rows
        .as_array()
        .unwrap()
        .iter()
        .find(|r| r["source"] == source)
        .unwrap();
    let before = compile35f_input(source, OWNED_COMPILE35F_RAW);
    assert_eq!(digest(&before), row["before_sha256"]);
    assert_eq!(compile35f_input(source, &previous), before);
    assert_eq!(compile35f_input(source, &before), before);
    for bad in [
        String::new(),
        "foreign".to_owned(),
        format!("{previous} "),
        OWNED_COMPILE35F_RAW[..OWNED_COMPILE35F_RAW.len() - 1].to_owned(),
    ] {
        assert!(std::panic::catch_unwind(|| owned_compile35f_input(source, &bad)).is_err());
    }
    assert_eq!(owned_compile35f_input("foreign", "unchanged"), "unchanged");
}

// Import-only compile generation before the immutable Selected947 source rows.
pub(crate) fn canonical_import_input(source: &str, current: &str) -> String {
    let compiled = match source {
        "src/cleanup_plan/replay/factored/canonical_shared.rs" => {
            include_str!("../cleanup_plan/replay/factored/canonical_shared.rs")
        }
        "src/cleanup_plan/replay/factored/canonical_shared/arena_tests.rs" => {
            include_str!("../cleanup_plan/replay/factored/canonical_shared/arena_tests.rs")
        }
        _ => return current.to_owned(),
    };
    let batch_compiled = semantic_batch_previous(source, compiled, "base851");
    let compiled = batch_compiled.as_str();
    let batch_current = semantic_batch_previous(source, current, "base851");
    let current = batch_current.as_str();
    let lexical_compiled = paid_many_canonical_previous(source, compiled);
    let compiled = lexical_compiled.as_str();
    let lexical_current = paid_many_canonical_previous(source, current);
    let current = lexical_current.as_str();
    let data = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/docs/evidence/canonical-import-acquisition-7827-20261017/sources.json"
    ));
    assert!(data.len() <= 65_536);
    assert_eq!(
        digest(data),
        "c0ec361ae90245676b4804403a4fe7bf65a34df06347109b086dfd0762deb7e0"
    );
    let rows: serde_json::Value = serde_json::from_str(data).unwrap();
    assert_eq!(rows.as_array().unwrap().len(), 2);
    let row = rows
        .as_array()
        .unwrap()
        .iter()
        .find(|r| r["source"] == source)
        .unwrap();
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let read = |archive: &str, length: &str, hash: &str| {
        let length = usize::try_from(row[length].as_u64().unwrap()).unwrap();
        assert!(length <= 131_072);
        if archive == source && source == "src/cleanup_plan/replay/factored/canonical_shared.rs" {
            return paid_many_canonical_previous(
                source,
                include_str!("../cleanup_plan/replay/factored/canonical_shared.rs"),
            );
        }
        semantic_batch_file(&root.join(archive), length, row[hash].as_str().unwrap()).unwrap()
    };
    let actual = read(source, "current_length", "current_sha256");
    assert_eq!(actual, compiled);
    assert_eq!(
        actual,
        read(
            row["current_archive"].as_str().unwrap(),
            "current_length",
            "current_sha256"
        )
    );
    let stage = read(
        row["stage_archive"].as_str().unwrap(),
        "stage_length",
        "stage_sha256",
    );
    let mut restored = actual.clone();
    for edit in row["inverse"].as_array().unwrap() {
        let after = edit["after"].as_str().unwrap();
        assert_eq!(restored.matches(after).count(), 1);
        restored = restored.replacen(after, edit["before"].as_str().unwrap(), 1);
    }
    assert_eq!(restored, stage);
    assert!(
        current == actual || current == stage,
        "foreign canonical import generation"
    );
    stage
}

#[test]
fn canonical_import_generation_authenticates_current_and_complete_selected_stages() {
    for (source, raw) in [
        (
            "src/cleanup_plan/replay/factored/canonical_shared.rs",
            include_str!("../cleanup_plan/replay/factored/canonical_shared.rs"),
        ),
        (
            "src/cleanup_plan/replay/factored/canonical_shared/arena_tests.rs",
            include_str!("../cleanup_plan/replay/factored/canonical_shared/arena_tests.rs"),
        ),
    ] {
        let stage = canonical_import_input(source, raw);
        assert_ne!(stage, raw);
        assert_eq!(canonical_import_input(source, &stage), stage);
        for bad in [
            String::new(),
            "foreign".to_owned(),
            format!("{raw} "),
            raw[..raw.len() - 1].to_owned(),
        ] {
            assert!(std::panic::catch_unwind(|| canonical_import_input(source, &bad)).is_err());
        }
    }
    assert_eq!(canonical_import_input("foreign", "unchanged"), "unchanged");
}

// Complete PaidMany source -> frozen compile-import input, a separate namespace.
fn paid_many_canonical_previous(source: &str, current: &str) -> String {
    if source != "src/cleanup_plan/replay/factored/canonical_shared.rs" {
        return current.to_owned();
    }
    let batch = semantic_batch_previous(source, current, "base851");
    let current = batch.as_str();
    let data = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/docs/evidence/paid-many-canonical-ingress-20261017/seal.json"
    ));
    assert_eq!(data.len(), 2923);
    assert_eq!(
        digest(data),
        "890336922097f28cddddbcfe85a3c267bf148acd9419289b198eb081fc8060be"
    );
    let seal: serde_json::Value = serde_json::from_str(data).unwrap();
    assert_eq!(seal["schema"], "semaprax.paid-many-canonical-ingress.v1");
    assert_eq!(seal["source"], source);
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let read = |path: &str, kind: &str| {
        let length = seal[format!("{kind}_length")].as_u64().unwrap() as usize;
        assert!(length <= 131_072);
        semantic_batch_file(
            &root.join(path),
            length,
            seal[format!("{kind}_sha256")].as_str().unwrap(),
        )
        .unwrap()
    };
    let actual = read(source, "current");
    assert_eq!(
        actual,
        semantic_batch_previous(
            source,
            include_str!("../cleanup_plan/replay/factored/canonical_shared.rs"),
            "base851"
        )
    );
    assert_eq!(
        actual,
        read(seal["current_archive"].as_str().unwrap(), "current")
    );
    let previous = read(seal["previous_archive"].as_str().unwrap(), "previous");
    let mut restored = actual.clone();
    let mut last = restored.len();
    for edit in seal["inverse"].as_array().unwrap() {
        let start = edit["start_byte"].as_u64().unwrap() as usize;
        let after = edit["after"].as_str().unwrap();
        let end = start.checked_add(after.len()).unwrap();
        assert!(end <= last);
        assert_eq!(restored.get(start..end), Some(after));
        restored.replace_range(start..end, edit["before"].as_str().unwrap());
        last = start;
    }
    assert_eq!(restored, previous);
    if current == actual || current == previous {
        previous
    } else {
        current.to_owned()
    }
}
#[test]
fn paid_many_canonical_ingress_keeps_whole_import_and_selected_controls() {
    let source = "src/cleanup_plan/replay/factored/canonical_shared.rs";
    let raw = include_str!("../cleanup_plan/replay/factored/canonical_shared.rs");
    let previous = paid_many_canonical_previous(source, raw);
    assert_ne!(previous, raw);
    assert_eq!(paid_many_canonical_previous(source, &previous), previous);
    let stage = canonical_import_input(source, raw);
    assert_eq!(canonical_import_input(source, &previous), stage);
    assert_eq!(canonical_import_input(source, &stage), stage);
    for bad in [
        String::new(),
        "foreign".to_owned(),
        format!("{raw} "),
        raw[..raw.len() - 1].to_owned(),
        format!("{previous} "),
    ] {
        assert!(std::panic::catch_unwind(|| canonical_import_input(source, &bad)).is_err());
    }
    assert_eq!(
        paid_many_canonical_previous("foreign", "unchanged"),
        "unchanged"
    );
}

// Exact assembled physical inputs precede immutable historical source domains.
// This test-only source transport grants no production or raw-audit authority.
fn semantic_batch_records() -> serde_json::Value {
    let data = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/docs/evidence/next-semantic-source-views-20261017/inputs.json"
    ));
    assert_eq!(data.len(), 84888);
    assert_eq!(
        digest(data),
        "c876c6b5f15969fe55bee55b31be8b6174db300d6b4f9f18cd412044273b6305"
    );
    let packet: serde_json::Value = serde_json::from_str(data).unwrap();
    assert_eq!(packet["schema"], "semaprax.next-semantic-source-views.v1");
    assert_eq!(packet["rows"].as_array().unwrap().len(), 15);
    packet
}
pub(crate) fn semantic_batch_previous(source: &str, current: &str, view: &str) -> String {
    let repaired = compile_repair_previous(source, current);
    let current = repaired.as_str();
    let packet = semantic_batch_records();
    let rows = packet["rows"].as_array().unwrap();
    assert!(rows.iter().filter(|r| r["source"] == source).count() <= 1);
    let Some(row) = rows.iter().find(|r| r["source"] == source) else {
        return current.to_owned();
    };
    let views = row["views"].as_array().unwrap();
    assert!(views.iter().filter(|r| r["name"] == view).count() <= 1);
    let Some(prior) = views.iter().find(|r| r["name"] == view) else {
        return current.to_owned();
    };
    let root = ::std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let read = |path: &str, length: &serde_json::Value, sha: &serde_json::Value| {
        compile_repair_file(
            &root.join(path),
            usize::try_from(length.as_u64().unwrap()).unwrap(),
            sha.as_str().unwrap(),
        )
        .unwrap()
    };
    let actual = crate::source_reflows_merge::next_big_batch_inputs::file(
        &root.join(source),
        row["current_length"].as_u64().unwrap() as usize,
        row["current_sha256"].as_str().unwrap(),
    )
    .unwrap_or_else(|| read(source, &row["current_length"], &row["current_sha256"]));
    assert_eq!(
        actual,
        read(
            row["current_archive"].as_str().unwrap(),
            &row["current_length"],
            &row["current_sha256"]
        )
    );
    let compiled = match source {
        "src/cleanup_plan/replay.rs" => include_str!("../cleanup_plan/replay.rs"),
        "src/cleanup_plan/replay/factored/canonical_shared.rs" => {
            include_str!("../cleanup_plan/replay/factored/canonical_shared.rs")
        }
        "src/cleanup_plan/replay/factored/canonical_shared/arena_tests.rs" => {
            include_str!("../cleanup_plan/replay/factored/canonical_shared/arena_tests.rs")
        }
        "src/cleanup_plan/replay/factored/canonical_shared/borrowed_labels.rs" => {
            include_str!("../cleanup_plan/replay/factored/canonical_shared/borrowed_labels.rs")
        }
        "src/cleanup_plan/replay/factored/canonical_shared/terminal_leaf.rs" => {
            include_str!("../cleanup_plan/replay/factored/canonical_shared/terminal_leaf.rs")
        }
        "src/cleanup_plan/replay/ordinary_plan_history.rs" => {
            include_str!("../cleanup_plan/replay/ordinary_plan_history.rs")
        }
        "src/cleanup_plan/replay/ordinary_plan_history/owned_tests.rs" => {
            include_str!("../cleanup_plan/replay/ordinary_plan_history/owned_tests.rs")
        }
        "src/cleanup_plan/replay/physical_budget/setup.rs" => {
            include_str!("../cleanup_plan/replay/physical_budget/setup.rs")
        }
        "src/cleanup_plan/replay/physical_storage.rs" => {
            include_str!("../cleanup_plan/replay/physical_storage.rs")
        }
        "src/cleanup_plan/replay/skeleton_work.rs" => {
            include_str!("../cleanup_plan/replay/skeleton_work.rs")
        }
        "src/cleanup_plan/replay/skeleton_work/ordinary_dag.rs" => {
            include_str!("../cleanup_plan/replay/skeleton_work/ordinary_dag.rs")
        }
        "src/cleanup_plan/replay/skeleton_work/ordinary_dag/frontier.rs" => {
            include_str!("../cleanup_plan/replay/skeleton_work/ordinary_dag/frontier.rs")
        }
        "src/cleanup_plan/replay/skeleton_work/prefix_fusion.rs" => {
            include_str!("../cleanup_plan/replay/skeleton_work/prefix_fusion.rs")
        }
        "src/source_verify/declaration.rs" => include_str!("../source_verify/declaration.rs"),
        "src/source_verify/declaration/record_shape.rs" => {
            include_str!("../source_verify/declaration/record_shape.rs")
        }
        _ => unreachable!("sealed finite semantic source"),
    };
    let compiled = crate::source_reflows_merge::next_big_batch_inputs::value(
        source,
        row["current_sha256"].as_str().unwrap(),
        compiled,
    );
    let compiled = compile_repair_previous(source, &compiled);
    assert_eq!(
        actual, compiled,
        "complete physical/compiled semantic source"
    );
    let previous = read(
        prior["archive"].as_str().unwrap(),
        &prior["length"],
        &prior["sha256"],
    );
    let mut restored = actual.clone();
    let mut last = restored.len();
    for edit in prior["inverse"].as_array().unwrap() {
        let start = usize::try_from(edit["start"].as_u64().unwrap()).unwrap();
        let after = edit["after"].as_str().unwrap();
        let end = start.checked_add(after.len()).unwrap();
        assert!(end <= last);
        assert_eq!(restored.get(start..end), Some(after));
        restored.replace_range(start..end, edit["before"].as_str().unwrap());
        last = start;
    }
    assert_eq!(restored, previous, "whole independent semantic predecessor");
    let current = crate::source_reflows_merge::next_big_batch_inputs::value(
        source,
        row["current_sha256"].as_str().unwrap(),
        current,
    );
    if current == actual || current == previous {
        previous
    } else {
        current.to_owned()
    }
}
pub(crate) fn semantic_batch_file(
    path: &::std::path::Path,
    length: usize,
    expected: &str,
) -> Result<String, &'static str> {
    let root = ::std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let packet = semantic_batch_records();
    if let Ok(relative) = path.strip_prefix(root) {
        if let Some(source) = relative.to_str() {
            if let Some(row) = packet["rows"]
                .as_array()
                .unwrap()
                .iter()
                .find(|r| r["source"] == source)
            {
                if let Some(prior) = row["views"].as_array().unwrap().iter().find(|r| {
                    r["sha256"] == expected && r["length"].as_u64() == Some(length as u64)
                }) {
                    let actual = compile_repair_file(
                        path,
                        usize::try_from(row["current_length"].as_u64().unwrap())
                            .map_err(|_| "semantic-length")?,
                        row["current_sha256"].as_str().ok_or("semantic-sha")?,
                    )?;
                    return Ok(semantic_batch_previous(
                        source,
                        &actual,
                        prior["name"].as_str().ok_or("semantic-view")?,
                    ));
                }
            }
        }
    }
    compile_repair_file(path, length, expected)
}
#[test]
fn semantic_batch_complete_physical_views_preserve_old_sources_and_refuse_supplied_drift() {
    let packet = semantic_batch_records();
    let root = ::std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    for row in packet["rows"].as_array().unwrap() {
        let source = row["source"].as_str().unwrap();
        let raw = compile_repair_file(
            &root.join(source),
            row["current_length"].as_u64().unwrap() as usize,
            row["current_sha256"].as_str().unwrap(),
        )
        .unwrap();
        for prior in row["views"].as_array().unwrap() {
            let name = prior["name"].as_str().unwrap();
            let old = semantic_batch_previous(source, &raw, name);
            assert_eq!(digest(&old), prior["sha256"]);
            assert_eq!(semantic_batch_previous(source, &old, name), old);
            for bad in [
                String::new(),
                format!("{raw} "),
                raw[..raw.len() - 1].to_owned(),
                format!("{raw}{raw}"),
            ] {
                assert_ne!(semantic_batch_previous(source, &bad, name), old);
            }
        }
    }
    assert_eq!(
        semantic_batch_previous("foreign", "unchanged", "base851"),
        "unchanged"
    );
    // Existing owning gate also authenticates every exact compilation ingress.
    let repaired = compile_repair_records();
    for row in repaired["rows"].as_array().unwrap() {
        let source = row["source"].as_str().unwrap();
        let raw = factored_core_file(
            &root.join(source),
            row["current_length"].as_u64().unwrap() as usize,
            row["current_sha256"].as_str().unwrap(),
        )
        .unwrap();
        let old = compile_repair_previous(source, &raw);
        assert_eq!(digest(&old), row["previous_sha256"]);
        assert_eq!(old.len() as u64, row["previous_length"].as_u64().unwrap());
        assert_eq!(compile_repair_previous(source, &old), old);
        assert_eq!(
            compile_repair_file(
                &root.join(source),
                old.len(),
                row["previous_sha256"].as_str().unwrap()
            )
            .unwrap(),
            old
        );
        for bad in [
            String::new(),
            format!("{raw} "),
            raw[..raw.len() - 1].to_owned(),
            format!("{raw}{raw}"),
        ] {
            assert_ne!(compile_repair_previous(source, &bad), old);
        }
    }
    assert_eq!(compile_repair_previous("foreign", "unchanged"), "unchanged");
}

// Exact complete compilation repair ingress, before every frozen semantic view.
// This is test-source custody only; no allocator, payer or runtime authority.
fn compile_repair_records() -> serde_json::Value {
    let data = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/docs/evidence/semantic162-compile-source-ingress-20261017/inputs.json"
    ));
    assert_eq!(data.len(), 13993);
    assert_eq!(
        digest(data),
        "e3870d5c067c099197b21f5b749b3124eaa4174701fe3eb47ca73db88ac44f14"
    );
    let packet: serde_json::Value = serde_json::from_str(data).unwrap();
    assert_eq!(packet["schema"], "semaprax.semantic162-compile-ingress.v1");
    assert_eq!(packet["rows"].as_array().unwrap().len(), 16);
    packet
}
fn compile_repair_previous(source: &str, current: &str) -> String {
    let accepted = accepted_base_previous(source, current);
    let current = accepted.as_str();
    let packet = compile_repair_records();
    let rows = packet["rows"].as_array().unwrap();
    assert!(rows.iter().filter(|r| r["source"] == source).count() <= 1);
    let Some(row) = rows.iter().find(|r| r["source"] == source) else {
        return current.to_owned();
    };
    let root = ::std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let read = |path: &str, length: &serde_json::Value, sha: &serde_json::Value| {
        factored_core_file(
            &root.join(path),
            usize::try_from(length.as_u64().unwrap()).unwrap(),
            sha.as_str().unwrap(),
        )
        .unwrap()
    };
    let actual = crate::source_reflows_merge::next_big_batch_inputs::file(
        &root.join(source),
        row["current_length"].as_u64().unwrap() as usize,
        row["current_sha256"].as_str().unwrap(),
    )
    .unwrap_or_else(|| read(source, &row["current_length"], &row["current_sha256"]));
    assert_eq!(
        actual,
        read(
            row["current_archive"].as_str().unwrap(),
            &row["current_length"],
            &row["current_sha256"]
        )
    );
    let compiled = match source {
        "src/cleanup_plan/replay/factored/canonical_shared/borrowed_labels.rs" => {
            include_str!("../cleanup_plan/replay/factored/canonical_shared/borrowed_labels.rs")
        }
        "src/cleanup_plan/replay/skeleton_work/merge_keys_tests.rs" => {
            include_str!("../cleanup_plan/replay/skeleton_work/merge_keys_tests.rs")
        }
        "src/cleanup_plan/replay/physical_storage/zero_tests.rs" => {
            include_str!("../cleanup_plan/replay/physical_storage/zero_tests.rs")
        }
        "src/cleanup_plan/replay/owned_suffix.rs" => {
            include_str!("../cleanup_plan/replay/owned_suffix.rs")
        }
        "src/cleanup_plan/replay/skeleton_work/ordinary_dag.rs" => {
            include_str!("../cleanup_plan/replay/skeleton_work/ordinary_dag.rs")
        }
        "src/cleanup_plan/replay/skeleton_work.rs" => {
            include_str!("../cleanup_plan/replay/skeleton_work.rs")
        }
        "src/cleanup_plan/replay/owned_suffix/tests.rs" => {
            include_str!("../cleanup_plan/replay/owned_suffix/tests.rs")
        }
        "src/cleanup_plan/replay/skeleton_work/enumerated_prefix/tests.rs" => {
            include_str!("../cleanup_plan/replay/skeleton_work/enumerated_prefix/tests.rs")
        }
        "src/cleanup_plan/replay/sequence_prefix.rs" => {
            include_str!("../cleanup_plan/replay/sequence_prefix.rs")
        }
        "src/cleanup_plan/replay/owned_suffix/sequence_vector.rs" => {
            include_str!("../cleanup_plan/replay/owned_suffix/sequence_vector.rs")
        }
        "src/cleanup_plan/replay/lexical_cert/paid/chains_tests.rs" => {
            include_str!("../cleanup_plan/replay/lexical_cert/paid/chains_tests.rs")
        }
        "src/cleanup_plan/replay/sequence_prefix/main_c11/wrapper_context.rs" => {
            include_str!("../cleanup_plan/replay/sequence_prefix/main_c11/wrapper_context.rs")
        }
        "src/cleanup_plan/replay/sequence_prefix/main8a/wrapper_context.rs" => {
            include_str!("../cleanup_plan/replay/sequence_prefix/main8a/wrapper_context.rs")
        }
        "src/cleanup_plan/replay/sequence_vector/main_a3/path_array_source.rs" => {
            include_str!("../cleanup_plan/replay/sequence_vector/main_a3/path_array_source.rs")
        }
        "src/workspace_graph/source_schema/nominal_context/tests.rs" => {
            include_str!("../workspace_graph/source_schema/nominal_context/tests.rs")
        }
        "src/cleanup_plan/replay/inventory_coverage/typed_tests.rs" => {
            include_str!("../cleanup_plan/replay/inventory_coverage/typed_tests.rs")
        }
        _ => unreachable!("sealed sixteen compilation subjects"),
    };
    let compiled = crate::source_reflows_merge::next_big_batch_inputs::value(
        source,
        row["current_sha256"].as_str().unwrap(),
        compiled,
    );
    let compiled = accepted_base_previous(source, &compiled);
    assert_eq!(
        actual, compiled,
        "whole physical/compiled compilation repair"
    );
    let previous = read(
        row["previous_archive"].as_str().unwrap(),
        &row["previous_length"],
        &row["previous_sha256"],
    );
    let mut restored = actual.clone();
    let mut last = restored.len();
    for edit in row["inverse"].as_array().unwrap() {
        let start = usize::try_from(edit["start"].as_u64().unwrap()).unwrap();
        let after = edit["after"].as_str().unwrap();
        let end = start.checked_add(after.len()).unwrap();
        assert!(end <= last);
        assert_eq!(restored.get(start..end), Some(after));
        restored.replace_range(start..end, edit["before"].as_str().unwrap());
        last = start;
    }
    assert_eq!(
        restored, previous,
        "complete independently retained precompile source"
    );
    let current = crate::source_reflows_merge::next_big_batch_inputs::value(
        source,
        row["current_sha256"].as_str().unwrap(),
        current,
    );
    if current == actual || current == previous {
        previous
    } else {
        current.to_owned()
    }
}
fn compile_repair_file(
    path: &::std::path::Path,
    length: usize,
    expected: &str,
) -> Result<String, &'static str> {
    let root = ::std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let packet = compile_repair_records();
    if let Ok(relative) = path.strip_prefix(root) {
        if let Some(source) = relative.to_str() {
            if let Some(row) = packet["rows"].as_array().unwrap().iter().find(|r| {
                r["source"] == source
                    && r["previous_sha256"] == expected
                    && r["previous_length"].as_u64() == Some(length as u64)
            }) {
                let actual = factored_core_file(
                    path,
                    usize::try_from(row["current_length"].as_u64().unwrap())
                        .map_err(|_| "compile-length")?,
                    row["current_sha256"].as_str().ok_or("compile-sha")?,
                )?;
                return Ok(compile_repair_previous(source, &actual));
            }
        }
    }
    factored_core_file(path, length, expected)
}

// Whole-source finite admission in front of the independently frozen prior chains.
fn accepted_records() -> serde_json::Value {
    let previous = accepted_records_before_earned_pins();
    let data = include_str!(
        "../../docs/evidence/earned-managed-pin42-source-coupling-20261017/inputs.json"
    );
    assert_eq!(data.len(), 34641);
    assert_eq!(
        digest(data),
        "27ec261cbc4f2b00a6ff0be7084f8c1a8c76e42523043b180f49701f8540abd2"
    );
    let packet: serde_json::Value = serde_json::from_str(data).unwrap();
    assert_eq!(packet["rows"].as_array().unwrap().len(), 42);
    let mut inverse = packet.clone();
    let added = inverse["rows"].as_array_mut().unwrap().pop().unwrap();
    assert_eq!(added["source"], "src/workspace_graph/type_proof_tests.rs");
    let row = inverse["rows"]
        .as_array_mut()
        .unwrap()
        .iter_mut()
        .find(|row| row["source"] == "tests/workspace/semantic_graph.rs")
        .unwrap();
    let mut prior = row["priors"].as_array_mut().unwrap().pop().unwrap();
    assert_eq!(prior["name"], "earned_pin_producer");
    prior.as_object_mut().unwrap().remove("name");
    row["current"] = prior;
    assert_eq!(
        inverse, previous,
        "complete original41 source table changed"
    );
    packet
}
fn accepted_records_before_earned_pins() -> serde_json::Value {
    let data =
        include_str!("../../docs/evidence/compile3-second-borrow-bindings-20261017/inputs.json");
    assert_eq!(data.len(), 33632);
    assert_eq!(
        digest(data),
        "9a4c35c3f31965ee34408cc88584f2e81c61d1a6c1e77d21248e1baad66cb270"
    );
    let packet: serde_json::Value = serde_json::from_str(data).unwrap();
    assert_eq!(
        packet["schema"],
        "semaprax.accepted-whole-source-ingress.v1"
    );
    assert_eq!(packet["rows"].as_array().unwrap().len(), 41);
    packet
}
fn accepted_compiled(source: &str) -> &'static str {
    match source {
        "src/workspace_graph/type_proof_tests.rs" => include_str!("../../src/workspace_graph/type_proof_tests.rs"),
        "crates/semaprax-native-rust-interop-builder/src/implementation/tests/ledger_capacity/source_inventory.rs" => include_str!("../../crates/semaprax-native-rust-interop-builder/src/implementation/tests/ledger_capacity/source_inventory.rs"),
        "src/bounded_output/classifier_string.rs" => include_str!("../../src/bounded_output/classifier_string.rs"),
        "src/bounded_output/stage_debits.rs" => include_str!("../../src/bounded_output/stage_debits.rs"),
        "src/bounded_output/stage_debits/tests.rs" => include_str!("../../src/bounded_output/stage_debits/tests.rs"),
        "src/cleanup_plan.rs" => include_str!("../../src/cleanup_plan.rs"),
        "src/cleanup_plan/replay.rs" => include_str!("../../src/cleanup_plan/replay.rs"),
        "src/cleanup_plan/replay/branch_clone.rs" => include_str!("../../src/cleanup_plan/replay/branch_clone.rs"),
        "src/cleanup_plan/replay/branch_clone/tests.rs" => include_str!("../../src/cleanup_plan/replay/branch_clone/tests.rs"),
        "src/cleanup_plan/replay/expression_facts.rs" => include_str!("../../src/cleanup_plan/replay/expression_facts.rs"),
        "src/cleanup_plan/replay/expression_facts/receiving/directory.rs" => include_str!("../../src/cleanup_plan/replay/expression_facts/receiving/directory.rs"),
        "src/cleanup_plan/replay/expression_facts/receiving/directory/status_prefix.rs" => include_str!("../../src/cleanup_plan/replay/expression_facts/receiving/directory/status_prefix.rs"),
        "src/cleanup_plan/replay/expression_facts/receiving/directory/status_prefix/tests.rs" => include_str!("../../src/cleanup_plan/replay/expression_facts/receiving/directory/status_prefix/tests.rs"),
        "src/cleanup_plan/replay/expression_facts/receiving/retained.rs" => include_str!("../../src/cleanup_plan/replay/expression_facts/receiving/retained.rs"),
        "src/cleanup_plan/replay/expression_facts/receiving/retained/case_certified.rs" => include_str!("../../src/cleanup_plan/replay/expression_facts/receiving/retained/case_certified.rs"),
        "src/cleanup_plan/replay/expression_facts/receiving/retained/case_certified/counter_tests.rs" => include_str!("../../src/cleanup_plan/replay/expression_facts/receiving/retained/case_certified/counter_tests.rs"),
        "src/cleanup_plan/replay/factored.rs" => include_str!("../../src/cleanup_plan/replay/factored.rs"),
        "src/cleanup_plan/replay/factored/canonical_shared.rs" => include_str!("../../src/cleanup_plan/replay/factored/canonical_shared.rs"),
        "src/cleanup_plan/replay/factored/canonical_shared/arena_tests.rs" => include_str!("../../src/cleanup_plan/replay/factored/canonical_shared/arena_tests.rs"),
        "src/cleanup_plan/replay/factored/canonical_shared/borrowed_labels.rs" => include_str!("../../src/cleanup_plan/replay/factored/canonical_shared/borrowed_labels.rs"),
        "src/cleanup_plan/replay/factored/canonical_shared/empty_memo.rs" => include_str!("../../src/cleanup_plan/replay/factored/canonical_shared/empty_memo.rs"),
        "src/cleanup_plan/replay/factored/canonical_shared/intern_arena.rs" => include_str!("../../src/cleanup_plan/replay/factored/canonical_shared/intern_arena.rs"),
        "src/cleanup_plan/replay/factored/canonical_shared/paid_many/tests.rs" => include_str!("../../src/cleanup_plan/replay/factored/canonical_shared/paid_many/tests.rs"),
        "src/cleanup_plan/replay/factored/continuation_receiver.rs" => include_str!("../../src/cleanup_plan/replay/factored/continuation_receiver.rs"),
        "src/cleanup_plan/replay/factored/oracle_hook.rs" => include_str!("../../src/cleanup_plan/replay/factored/oracle_hook.rs"),
        "src/cleanup_plan/replay/owned_suffix.rs" => include_str!("../../src/cleanup_plan/replay/owned_suffix.rs"),
        "src/cleanup_plan/replay/owned_suffix/tests.rs" => include_str!("../../src/cleanup_plan/replay/owned_suffix/tests.rs"),
        "src/cleanup_plan/replay/path_join.rs" => include_str!("../../src/cleanup_plan/replay/path_join.rs"),
        "src/cleanup_plan/replay/skeleton_work/enumerated_prefix.rs" => include_str!("../../src/cleanup_plan/replay/skeleton_work/enumerated_prefix.rs"),
        "src/cleanup_plan/replay/skeleton_work/enumerated_prefix/tests.rs" => include_str!("../../src/cleanup_plan/replay/skeleton_work/enumerated_prefix/tests.rs"),
        "src/source_reflows/semantic_receivers.rs" => include_str!("../../src/source_reflows/semantic_receivers.rs"),
        "src/source_reflows_merge.rs" => include_str!("../../src/source_reflows_merge.rs"),
        "src/source_reflows_merge/opt858_sequence_input.rs" => include_str!("../../src/source_reflows_merge/opt858_sequence_input.rs"),
        "src/workspace_graph/tests/builder_limits.rs" => include_str!("../../src/workspace_graph/tests/builder_limits.rs"),
        "tests/hir_module_boundaries.rs" => include_str!("../../tests/hir_module_boundaries.rs"),
        "tests/native/string_settlement/codec_allocation/codec808_frontier.rs" => include_str!("../../tests/native/string_settlement/codec_allocation/codec808_frontier.rs"),
        "tests/project/standard_library/application_json/nested_response/string_vectors.rs" => include_str!("../../tests/project/standard_library/application_json/nested_response/string_vectors.rs"),
        "tests/support/production_source_inventory.rs" => include_str!("../../tests/support/production_source_inventory.rs"),
        "tests/useful_data/byte_slice_for_runtime/nested.rs" => include_str!("../../tests/useful_data/byte_slice_for_runtime/nested.rs"),
        "tests/workspace/semantic_graph.rs" => include_str!("../../tests/workspace/semantic_graph.rs"),
        "tests/workspace/semantic_graph/source_inventory.rs" => include_str!("../../tests/workspace/semantic_graph/source_inventory.rs"),
        "tests/workspace/semantic_graph/source_inventory_controls.rs" => include_str!("../../tests/workspace/semantic_graph/source_inventory_controls.rs"),
        _ => unreachable!("closed accepted whole-source membership"),
    }
}
fn accepted_read(row: &serde_json::Value) -> String {
    let root = ::std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    factored_core_file_raw(
        &root.join(row["archive"].as_str().unwrap()),
        row["length"].as_u64().unwrap() as usize,
        row["sha256"].as_str().unwrap(),
    )
    .unwrap()
}
fn accepted_actual(row: &serde_json::Value) -> String {
    let source = row["source"].as_str().unwrap();
    let current = &row["current"];
    let root = ::std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let actual = crate::source_reflows_merge::next_big_batch_inputs::file(
        &root.join(source),
        current["length"].as_u64().unwrap() as usize,
        current["sha256"].as_str().unwrap(),
    )
    .unwrap_or_else(|| {
        factored_core_file_raw(
            &root.join(source),
            current["length"].as_u64().unwrap() as usize,
            current["sha256"].as_str().unwrap(),
        )
        .unwrap()
    });
    let compiled = crate::source_reflows_merge::next_big_batch_inputs::value(
        source,
        current["sha256"].as_str().unwrap(),
        accepted_compiled(source),
    );
    assert_eq!(
        actual, compiled,
        "complete compiled actual source before historical selection"
    );
    assert_eq!(actual, accepted_read(current), "complete current archive");
    actual
}
fn accepted_select(row: &serde_json::Value, prior: &serde_json::Value, supplied: &str) -> String {
    let actual = accepted_actual(row);
    let previous = accepted_read(prior);
    let supplied = crate::source_reflows_merge::next_big_batch_inputs::value(
        row["source"].as_str().unwrap(),
        row["current"]["sha256"].as_str().unwrap(),
        supplied,
    );
    // Whole inverse: the complete authenticated current is replaced by the complete
    // independently retained prior. Foreign supplied bytes never acquire a view.
    if supplied == actual || supplied == previous {
        previous
    } else {
        supplied.to_owned()
    }
}
fn accepted_base_previous(source: &str, supplied: &str) -> String {
    let packet = accepted_records();
    let Some(row) = packet["rows"]
        .as_array()
        .unwrap()
        .iter()
        .find(|row| row["source"] == source)
    else {
        return supplied.to_owned();
    };
    let Some(prior) = row["priors"]
        .as_array()
        .unwrap()
        .iter()
        .find(|prior| prior["name"] == "base5f")
    else {
        return supplied.to_owned();
    };
    accepted_select(row, prior, supplied)
}
pub(crate) fn accepted_expected_value(source: &str, expected: &str, supplied: &str) -> String {
    let packet = accepted_records();
    let Some(row) = packet["rows"]
        .as_array()
        .unwrap()
        .iter()
        .find(|row| row["source"] == source)
    else {
        return supplied.to_owned();
    };
    let Some(prior) = row["priors"]
        .as_array()
        .unwrap()
        .iter()
        .find(|prior| prior["sha256"] == expected)
    else {
        return supplied.to_owned();
    };
    accepted_select(row, prior, supplied)
}
pub(crate) fn accepted_expected_file(
    path: &::std::path::Path,
    length: usize,
    expected: &str,
) -> Option<String> {
    let root = ::std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let source = path.strip_prefix(root).ok()?.to_str()?;
    let packet = accepted_records();
    let row = packet["rows"]
        .as_array()
        .unwrap()
        .iter()
        .find(|row| row["source"] == source)?;
    let prior = row["priors"].as_array().unwrap().iter().find(|prior| {
        prior["sha256"] == expected && prior["length"].as_u64() == Some(length as u64)
    })?;
    let actual = accepted_actual(row);
    Some(accepted_select(row, prior, &actual))
}
#[test]
fn accepted_whole_sources_authenticate_actual_owners_and_keep_every_prior_independent() {
    let packet = accepted_records();
    let root = ::std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    for row in packet["rows"].as_array().unwrap() {
        let source = row["source"].as_str().unwrap();
        let actual = accepted_actual(row);
        for prior in row["priors"].as_array().unwrap() {
            let previous = accepted_read(prior);
            assert_eq!(accepted_select(row, prior, &actual), previous);
            assert_eq!(accepted_select(row, prior, &previous), previous);
            for bad in [
                String::new(),
                format!("{actual} "),
                actual[..actual.len() - 1].to_owned(),
                format!("{actual}{actual}"),
                "foreign".to_owned(),
            ] {
                assert_eq!(accepted_select(row, prior, &bad), bad);
            }
            let length = prior["length"].as_u64().unwrap() as usize;
            let expected = prior["sha256"].as_str().unwrap();
            assert_eq!(
                accepted_expected_file(&root.join(source), length, expected),
                Some(previous)
            );
            assert!(accepted_expected_file(&root.join(source), length + 1, expected).is_none());
            assert!(accepted_expected_file(&root.join(source), length, &"0".repeat(64)).is_none());
        }
    }
    assert!(accepted_expected_file(&root.join("foreign"), 0, "foreign").is_none());
}

// Independently compiled whole future57 tail; shared prefix above is byte-exact.
mod before_future57 {
    use super::*;
    // Whole-source finite admission in front of the independently frozen prior chains.
    fn accepted_records() -> serde_json::Value {
        let data = include_str!(
            "../../docs/evidence/future831-f247-final-source-binding-20261017/current-inputs.json"
        );
        assert_eq!(data.len(), 46861);
        assert_eq!(
            digest(data),
            "41cdb5d63b4b405b85f71b563326f6eb500971c7a59a64420acb226c7c448b69"
        );
        let packet: serde_json::Value = serde_json::from_str(data).unwrap();
        assert_eq!(
            packet["schema"],
            "semaprax.accepted-whole-source-ingress.v1"
        );
        assert_eq!(packet["rows"].as_array().unwrap().len(), 57);
        packet
    }
    fn accepted_compiled(source: &str) -> &'static str {
        match source {
        "crates/semaprax-native-rust-interop-builder/src/implementation/tests/ledger_capacity/source_inventory.rs" => include_str!("../../crates/semaprax-native-rust-interop-builder/src/implementation/tests/ledger_capacity/source_inventory.rs"),
        "src/bounded_output/classifier_string.rs" => include_str!("../../src/bounded_output/classifier_string.rs"),
        "src/bounded_output/stage_debits.rs" => include_str!("../../src/bounded_output/stage_debits.rs"),
        "src/bounded_output/stage_debits/tests.rs" => include_str!("../../src/bounded_output/stage_debits/tests.rs"),
        "src/cleanup_plan.rs" => include_str!("../../src/cleanup_plan.rs"),
        "src/cleanup_plan/replay.rs" => include_str!("../../src/cleanup_plan/replay.rs"),
        "src/cleanup_plan/replay/branch_clone.rs" => include_str!("../../src/cleanup_plan/replay/branch_clone.rs"),
        "src/cleanup_plan/replay/branch_clone/tests.rs" => include_str!("../../src/cleanup_plan/replay/branch_clone/tests.rs"),
        "src/cleanup_plan/replay/expression_facts.rs" => include_str!("../../src/cleanup_plan/replay/expression_facts.rs"),
        "src/cleanup_plan/replay/expression_facts/receiving/directory.rs" => include_str!("../../src/cleanup_plan/replay/expression_facts/receiving/directory.rs"),
        "src/cleanup_plan/replay/expression_facts/receiving/directory/status_prefix.rs" => include_str!("../../src/cleanup_plan/replay/expression_facts/receiving/directory/status_prefix.rs"),
        "src/cleanup_plan/replay/expression_facts/receiving/directory/status_prefix/tests.rs" => include_str!("../../src/cleanup_plan/replay/expression_facts/receiving/directory/status_prefix/tests.rs"),
        "src/cleanup_plan/replay/expression_facts/receiving/retained.rs" => include_str!("../../src/cleanup_plan/replay/expression_facts/receiving/retained.rs"),
        "src/cleanup_plan/replay/expression_facts/receiving/retained/case_certified.rs" => include_str!("../../src/cleanup_plan/replay/expression_facts/receiving/retained/case_certified.rs"),
        "src/cleanup_plan/replay/expression_facts/receiving/retained/case_certified/counter_tests.rs" => include_str!("../../src/cleanup_plan/replay/expression_facts/receiving/retained/case_certified/counter_tests.rs"),
        "src/cleanup_plan/replay/factored.rs" => include_str!("../../src/cleanup_plan/replay/factored.rs"),
        "src/cleanup_plan/replay/factored/canonical_shared.rs" => include_str!("../../src/cleanup_plan/replay/factored/canonical_shared.rs"),
        "src/cleanup_plan/replay/factored/canonical_shared/arena_tests.rs" => include_str!("../../src/cleanup_plan/replay/factored/canonical_shared/arena_tests.rs"),
        "src/cleanup_plan/replay/factored/canonical_shared/borrowed_labels.rs" => include_str!("../../src/cleanup_plan/replay/factored/canonical_shared/borrowed_labels.rs"),
        "src/cleanup_plan/replay/factored/canonical_shared/empty_memo.rs" => include_str!("../../src/cleanup_plan/replay/factored/canonical_shared/empty_memo.rs"),
        "src/cleanup_plan/replay/factored/canonical_shared/intern_arena.rs" => include_str!("../../src/cleanup_plan/replay/factored/canonical_shared/intern_arena.rs"),
        "src/cleanup_plan/replay/factored/canonical_shared/paid_many/tests.rs" => include_str!("../../src/cleanup_plan/replay/factored/canonical_shared/paid_many/tests.rs"),
        "src/cleanup_plan/replay/factored/continuation_receiver.rs" => include_str!("../../src/cleanup_plan/replay/factored/continuation_receiver.rs"),
        "src/cleanup_plan/replay/factored/oracle_hook.rs" => include_str!("../../src/cleanup_plan/replay/factored/oracle_hook.rs"),
        "src/cleanup_plan/replay/owned_suffix.rs" => include_str!("../../src/cleanup_plan/replay/owned_suffix.rs"),
        "src/cleanup_plan/replay/owned_suffix/tests.rs" => include_str!("../../src/cleanup_plan/replay/owned_suffix/tests.rs"),
        "src/cleanup_plan/replay/path_join.rs" => include_str!("../../src/cleanup_plan/replay/path_join.rs"),
        "src/cleanup_plan/replay/skeleton_work/enumerated_prefix.rs" => include_str!("../../src/cleanup_plan/replay/skeleton_work/enumerated_prefix.rs"),
        "src/cleanup_plan/replay/skeleton_work/enumerated_prefix/tests.rs" => include_str!("../../src/cleanup_plan/replay/skeleton_work/enumerated_prefix/tests.rs"),
        "src/source_reflows/semantic_receivers.rs" => include_str!("../../src/source_reflows/semantic_receivers.rs"),
        "src/source_reflows_merge.rs" => include_str!("../../src/source_reflows_merge.rs"),
        "src/source_reflows_merge/opt858_sequence_input.rs" => include_str!("../../src/source_reflows_merge/opt858_sequence_input.rs"),
        "src/workspace_graph/tests/builder_limits.rs" => include_str!("../../src/workspace_graph/tests/builder_limits.rs"),
        "tests/hir_module_boundaries.rs" => include_str!("../../tests/hir_module_boundaries.rs"),
        "tests/native/string_settlement/codec_allocation/codec808_frontier.rs" => include_str!("../../tests/native/string_settlement/codec_allocation/codec808_frontier.rs"),
        "tests/project/standard_library/application_json/nested_response/string_vectors.rs" => include_str!("../../tests/project/standard_library/application_json/nested_response/string_vectors.rs"),
        "tests/support/production_source_inventory.rs" => include_str!("../../tests/support/production_source_inventory.rs"),
        "tests/useful_data/byte_slice_for_runtime/nested.rs" => include_str!("../../tests/useful_data/byte_slice_for_runtime/nested.rs"),
        "tests/workspace/semantic_graph.rs" => include_str!("../../tests/workspace/semantic_graph.rs"),
        "tests/workspace/semantic_graph/source_inventory.rs" => include_str!("../../tests/workspace/semantic_graph/source_inventory.rs"),
        "tests/workspace/semantic_graph/source_inventory_controls.rs" => include_str!("../../tests/workspace/semantic_graph/source_inventory_controls.rs"),
        "src/cleanup_plan/replay/factored/canonical_shared/before_selected_labels.rs" => include_str!("../../src/cleanup_plan/replay/factored/canonical_shared/before_selected_labels.rs"),
        "src/cleanup_plan/replay/factored/canonical_shared/before_selected_labels/terminal_leaf.rs" => include_str!("../../src/cleanup_plan/replay/factored/canonical_shared/before_selected_labels/terminal_leaf.rs"),
        "src/cleanup_plan/replay/factored/canonical_shared/selected_labels.rs" => include_str!("../../src/cleanup_plan/replay/factored/canonical_shared/selected_labels.rs"),
        "src/cleanup_plan/replay/factored/canonical_shared/selected_labels/tests.rs" => include_str!("../../src/cleanup_plan/replay/factored/canonical_shared/selected_labels/tests.rs"),
        "src/cleanup_plan/replay/skeleton_work.rs" => include_str!("../../src/cleanup_plan/replay/skeleton_work.rs"),
        "src/cleanup_plan/replay/skeleton_work/isolated_paths.rs" => include_str!("../../src/cleanup_plan/replay/skeleton_work/isolated_paths.rs"),
        "src/cleanup_plan/replay/skeleton_work/isolated_paths/tests.rs" => include_str!("../../src/cleanup_plan/replay/skeleton_work/isolated_paths/tests.rs"),
        "src/cleanup_plan/replay/skeleton_work/isolated_paths/tests/before.rs" => include_str!("../../src/cleanup_plan/replay/skeleton_work/isolated_paths/tests/before.rs"),
        "src/source_reflows.rs" => include_str!("../../src/source_reflows.rs"),
        "src/workspace_graph/static_text/scalar_helpers.rs" => include_str!("../../src/workspace_graph/static_text/scalar_helpers.rs"),
        "src/workspace_graph/static_text/scalar_helpers/capture_directory/tests.rs" => include_str!("../../src/workspace_graph/static_text/scalar_helpers/capture_directory/tests.rs"),
        "tests/workspace/semantic_change/reporting.rs" => include_str!("../../tests/workspace/semantic_change/reporting.rs"),
        "tests/workspace/semantic_structural_change/reporting.rs" => include_str!("../../tests/workspace/semantic_structural_change/reporting.rs"),
        "src/cleanup_plan/replay/compact_stage.rs" => include_str!("../../src/cleanup_plan/replay/compact_stage.rs"),
        "src/cleanup_plan/replay/compact_stage/main_f247_source_custody.rs" => include_str!("../../src/cleanup_plan/replay/compact_stage/main_f247_source_custody.rs"),
        "src/source_reflows/main_f247_controllers.rs" => include_str!("../../src/source_reflows/main_f247_controllers.rs"),
        _ => unreachable!("closed accepted whole-source membership"),
    }
    }
    fn accepted_read(row: &serde_json::Value) -> String {
        let root = ::std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
        factored_core_file_raw(
            &root.join(row["archive"].as_str().unwrap()),
            row["length"].as_u64().unwrap() as usize,
            row["sha256"].as_str().unwrap(),
        )
        .unwrap()
    }
    fn accepted_actual(row: &serde_json::Value) -> String {
        let source = row["source"].as_str().unwrap();
        let current = &row["current"];
        let root = ::std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
        let actual = crate::source_reflows_merge::next_big_batch_inputs::file(
            &root.join(source),
            current["length"].as_u64().unwrap() as usize,
            current["sha256"].as_str().unwrap(),
        )
        .unwrap_or_else(|| {
            factored_core_file_raw(
                &root.join(source),
                current["length"].as_u64().unwrap() as usize,
                current["sha256"].as_str().unwrap(),
            )
            .unwrap()
        });
        let compiled = crate::source_reflows_merge::next_big_batch_inputs::value(
            source,
            current["sha256"].as_str().unwrap(),
            accepted_compiled(source),
        );
        assert_eq!(
            actual, compiled,
            "complete compiled actual source before historical selection"
        );
        assert_eq!(actual, accepted_read(current), "complete current archive");
        actual
    }
    fn accepted_select(
        row: &serde_json::Value,
        prior: &serde_json::Value,
        supplied: &str,
    ) -> String {
        let actual = accepted_actual(row);
        let previous = accepted_read(prior);
        let supplied = crate::source_reflows_merge::next_big_batch_inputs::value(
            row["source"].as_str().unwrap(),
            row["current"]["sha256"].as_str().unwrap(),
            supplied,
        );
        // Whole inverse: the complete authenticated current is replaced by the complete
        // independently retained prior. Foreign supplied bytes never acquire a view.
        if supplied == actual || supplied == previous {
            previous
        } else {
            supplied.to_owned()
        }
    }
    #[allow(
        dead_code,
        reason = "Retained compatibility entry for independently compiled historical and semantic controls."
    )]
    fn accepted_base_previous(source: &str, supplied: &str) -> String {
        let packet = accepted_records();
        let Some(row) = packet["rows"]
            .as_array()
            .unwrap()
            .iter()
            .find(|row| row["source"] == source)
        else {
            return supplied.to_owned();
        };
        let Some(prior) = row["priors"]
            .as_array()
            .unwrap()
            .iter()
            .find(|prior| prior["name"] == "base5f")
        else {
            return supplied.to_owned();
        };
        accepted_select(row, prior, supplied)
    }
    #[allow(
        dead_code,
        reason = "Retained compatibility entry for independently compiled historical and semantic controls."
    )]
    pub(crate) fn accepted_expected_value(source: &str, expected: &str, supplied: &str) -> String {
        let packet = accepted_records();
        let Some(row) = packet["rows"]
            .as_array()
            .unwrap()
            .iter()
            .find(|row| row["source"] == source)
        else {
            return supplied.to_owned();
        };
        let Some(prior) = row["priors"]
            .as_array()
            .unwrap()
            .iter()
            .find(|prior| prior["sha256"] == expected)
        else {
            return supplied.to_owned();
        };
        accepted_select(row, prior, supplied)
    }
    pub(crate) fn accepted_expected_file(
        path: &::std::path::Path,
        length: usize,
        expected: &str,
    ) -> Option<String> {
        let root = ::std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
        let source = path.strip_prefix(root).ok()?.to_str()?;
        let packet = accepted_records();
        let row = packet["rows"]
            .as_array()
            .unwrap()
            .iter()
            .find(|row| row["source"] == source)?;
        let prior = row["priors"].as_array().unwrap().iter().find(|prior| {
            prior["sha256"] == expected && prior["length"].as_u64() == Some(length as u64)
        })?;
        let actual = accepted_actual(row);
        Some(accepted_select(row, prior, &actual))
    }
    #[test]
    fn accepted_whole_sources_authenticate_actual_owners_and_keep_every_prior_independent() {
        let packet = accepted_records();
        let root = ::std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
        for row in packet["rows"].as_array().unwrap() {
            let source = row["source"].as_str().unwrap();
            let actual = accepted_actual(row);
            for prior in row["priors"].as_array().unwrap() {
                let previous = accepted_read(prior);
                assert_eq!(accepted_select(row, prior, &actual), previous);
                assert_eq!(accepted_select(row, prior, &previous), previous);
                for bad in [
                    String::new(),
                    format!("{actual} "),
                    actual[..actual.len() - 1].to_owned(),
                    format!("{actual}{actual}"),
                    "foreign".to_owned(),
                ] {
                    assert_eq!(accepted_select(row, prior, &bad), bad);
                }
                let length = prior["length"].as_u64().unwrap() as usize;
                let expected = prior["sha256"].as_str().unwrap();
                assert_eq!(
                    accepted_expected_file(&root.join(source), length, expected),
                    Some(previous)
                );
                assert!(accepted_expected_file(&root.join(source), length + 1, expected).is_none());
                assert!(
                    accepted_expected_file(&root.join(source), length, &"0".repeat(64)).is_none()
                );
            }
        }
        assert!(accepted_expected_file(&root.join("foreign"), 0, "foreign").is_none());
    }

    // Distinct exact incoming f247 cap; existing historical caps stay unchanged.
    const MAIN_F247_CAP: usize = 9_703_917;
    fn main_f247_body(
        reader: &mut impl std::io::Read,
        length: usize,
        expected: &str,
    ) -> Result<String, &'static str> {
        if length > MAIN_F247_CAP {
            return Err("bound");
        }
        let mut bytes = Vec::new();
        bytes.try_reserve_exact(length).map_err(|_| "allocation")?;
        bytes.resize(length, 0);
        reader.read_exact(&mut bytes).map_err(|_| "truncated")?;
        let mut extra = [0u8; 1];
        if reader.read(&mut extra).map_err(|_| "read")? != 0 {
            return Err("extra");
        }
        if main_f247_digest(&bytes) != expected {
            return Err("digest");
        }
        String::from_utf8(bytes).map_err(|_| "utf8")
    }
    fn main_f247_digest(bytes: &[u8]) -> String {
        format!("{:x}", crate::digest_hex::LowerHex(Sha256::digest(bytes)))
    }
    fn main_f247_same_file(a: &std::fs::Metadata, b: &std::fs::Metadata) -> bool {
        if !a.is_file()
            || !b.is_file()
            || a.len() != b.len()
            || a.modified().ok() != b.modified().ok()
        {
            return false;
        }
        #[cfg(unix)]
        {
            use std::os::unix::fs::MetadataExt;
            if a.dev() != b.dev()
                || a.ino() != b.ino()
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
    pub(crate) fn main_f247_file_raw(
        path: &::std::path::Path,
        length: usize,
        expected: &str,
    ) -> Result<String, &'static str> {
        if length > MAIN_F247_CAP {
            return Err("bound");
        }
        let path_before = std::fs::symlink_metadata(path).map_err(|_| "missing")?;
        if path_before.file_type().is_symlink() || !path_before.is_file() {
            return Err("length-or-nonregular");
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
        let metadata = file.metadata().map_err(|_| "metadata")?;
        if !main_f247_same_file(&path_before, &metadata)
            || metadata.len() != u64::try_from(length).map_err(|_| "bound")?
        {
            return Err("length-or-nonregular");
        }
        // The one opened no-follow regular handle is checked before any buffer.
        let value = main_f247_body(&mut file, length, expected)?;
        let after = file.metadata().map_err(|_| "metadata")?;
        let path_after = std::fs::symlink_metadata(path).map_err(|_| "metadata")?;
        if path_after.file_type().is_symlink()
            || !main_f247_same_file(&metadata, &after)
            || !main_f247_same_file(&after, &path_after)
        {
            return Err("metadata-drift");
        }
        Ok(value)
    }

    // Current compiler text for the finite accepted-owned source domain only.
    pub(crate) fn accepted_physical_compiled(source: &str) -> Option<&'static str> {
        let packet = accepted_records();
        if packet["rows"]
            .as_array()
            .unwrap()
            .iter()
            .any(|row| row["source"] == source)
        {
            Some(accepted_compiled(source))
        } else {
            None
        }
    }
}

pub(crate) fn main_f247_file_raw(
    path: &::std::path::Path,
    length: usize,
    expected: &str,
) -> Result<String, &'static str> {
    if let Some(value) =
        crate::source_reflows_merge::next_big_batch_inputs::file(path, length, expected)
    {
        return Ok(value);
    }
    before_future57::main_f247_file_raw(path, length, expected)
}

#[allow(
    unused_imports,
    reason = "Retain the complete old receiver API in independently compiled source-control harnesses."
)]
pub(crate) use before_future57::accepted_physical_compiled;
