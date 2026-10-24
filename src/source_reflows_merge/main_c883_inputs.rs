//! Closed acquisition for the complete main-c883 historical controllers.
//! This read boundary is test-only and never feeds a current raw source audit.
#[allow(
    clippy::duplicate_mod,
    reason = "The same closed reader must compile in independent library and integration harness namespaces."
)]
#[path = "../source_reflows/ci_inputs.rs"]
mod legacy_ci_inputs;
use sha2::{Digest, Sha256};
use std::path::Path;
const CAP: usize = 32 * 1024 * 1024;
fn digest(bytes: &[u8]) -> String {
    format!("{:x}", crate::digest_hex::LowerHex(Sha256::digest(bytes)))
}
fn held(path: &Path, length: usize, expected: &str) -> std::io::Result<String> {
    if let Some(value) =
        crate::source_reflows_merge::next_big_batch_inputs::file(path, length, expected)
    {
        return Ok(value);
    }
    if let Some(value) = legacy_ci_inputs::file_expected(path, length, expected) {
        return Ok(value);
    }
    if let Some(value) = super::main67107_inputs::accepted_expected_file(path, length, expected) {
        return Ok(value);
    }
    use std::io::Read;
    let before = std::fs::symlink_metadata(path)?;
    assert!(before.is_file() && !before.file_type().is_symlink());
    assert!(length <= CAP && before.len() == length as u64);
    let mut source = String::new();
    std::fs::File::open(path)?
        .take(CAP as u64 + 1)
        .read_to_string(&mut source)?;
    assert_eq!(source.len(), length);
    assert_eq!(digest(source.as_bytes()), expected);
    let after = std::fs::symlink_metadata(path)?;
    assert!(after.is_file() && !after.file_type().is_symlink());
    assert_eq!(before.len(), after.len());
    assert_eq!(before.modified()?, after.modified()?);
    Ok(source)
}
fn packet() -> serde_json::Value {
    let data =
        include_str!("../../docs/evidence/main-c883-next-source-merge-20261017/main-inputs.json");
    assert_eq!(data.len(), 119129);
    assert_eq!(
        digest(data.as_bytes()),
        "83ac5f029587334507d16b67b075e1382b7cfa3df0ae4e704e1fe7fc3c6c019d"
    );
    let packet: serde_json::Value = serde_json::from_str(data).unwrap();
    assert_eq!(packet["schema"], "semaprax.main-c883-source-acquisition.v1");
    assert_eq!(packet["rows"].as_array().unwrap().len(), 240);
    packet
}
fn compiled(path: &str) -> Option<&'static str> {
    match path {
        "src/byte_ops.rs" => Some(include_str!("../../src/byte_ops.rs")),
        "src/cleanup_plan.rs" => Some(include_str!("../../src/cleanup_plan.rs")),
        "src/cleanup_plan/renewal_profile.rs" => Some(include_str!("../../src/cleanup_plan/renewal_profile.rs")),
        "src/cleanup_plan/renewal_profile/funded.rs" => Some(include_str!("../../src/cleanup_plan/renewal_profile/funded.rs")),
        "src/cleanup_plan/replay.rs" => Some(include_str!("../../src/cleanup_plan/replay.rs")),
        "src/cleanup_plan/replay/branch_clone.rs" => Some(include_str!("../../src/cleanup_plan/replay/branch_clone.rs")),
        "src/cleanup_plan/replay/branch_clone/tests.rs" => Some(include_str!("../../src/cleanup_plan/replay/branch_clone/tests.rs")),
        "src/cleanup_plan/replay/continuation_storage.rs" => Some(include_str!("../../src/cleanup_plan/replay/continuation_storage.rs")),
        "src/cleanup_plan/replay/continuation_storage/tests.rs" => Some(include_str!("../../src/cleanup_plan/replay/continuation_storage/tests.rs")),
        "src/cleanup_plan/replay/expression_facts/arena.rs" => Some(include_str!("../../src/cleanup_plan/replay/expression_facts/arena.rs")),
        "src/cleanup_plan/replay/expression_facts/arena/cursor_tests.rs" => Some(include_str!("../../src/cleanup_plan/replay/expression_facts/arena/cursor_tests.rs")),
        "src/cleanup_plan/replay/expression_facts/arena/prefix.rs" => Some(include_str!("../../src/cleanup_plan/replay/expression_facts/arena/prefix.rs")),
        "src/cleanup_plan/replay/expression_facts/arena/prefix/tests.rs" => Some(include_str!("../../src/cleanup_plan/replay/expression_facts/arena/prefix/tests.rs")),
        "src/cleanup_plan/replay/expression_facts/receiving/directory.rs" => Some(include_str!("../../src/cleanup_plan/replay/expression_facts/receiving/directory.rs")),
        "src/cleanup_plan/replay/expression_facts/receiving/directory/status_prefix.rs" => Some(include_str!("../../src/cleanup_plan/replay/expression_facts/receiving/directory/status_prefix.rs")),
        "src/cleanup_plan/replay/expression_facts/receiving/directory/status_prefix/tests.rs" => Some(include_str!("../../src/cleanup_plan/replay/expression_facts/receiving/directory/status_prefix/tests.rs")),
        "src/cleanup_plan/replay/expression_facts/receiving/helpers.rs" => Some(include_str!("../../src/cleanup_plan/replay/expression_facts/receiving/helpers.rs")),
        "src/cleanup_plan/replay/expression_facts/receiving/helpers/tests.rs" => Some(include_str!("../../src/cleanup_plan/replay/expression_facts/receiving/helpers/tests.rs")),
        "src/cleanup_plan/replay/expression_facts/receiving/remaining.rs" => Some(include_str!("../../src/cleanup_plan/replay/expression_facts/receiving/remaining.rs")),
        "src/cleanup_plan/replay/expression_facts/receiving/retained.rs" => Some(include_str!("../../src/cleanup_plan/replay/expression_facts/receiving/retained.rs")),
        "src/cleanup_plan/replay/expression_facts/receiving/retained/exits.rs" => Some(include_str!("../../src/cleanup_plan/replay/expression_facts/receiving/retained/exits.rs")),
        "src/cleanup_plan/replay/factored.rs" => Some(include_str!("../../src/cleanup_plan/replay/factored.rs")),
        "src/cleanup_plan/replay/factored/borrowed_canonical/dictionary/keys.rs" => Some(include_str!("../../src/cleanup_plan/replay/factored/borrowed_canonical/dictionary/keys.rs")),
        "src/cleanup_plan/replay/factored/borrowed_canonical/tests.rs" => Some(include_str!("../../src/cleanup_plan/replay/factored/borrowed_canonical/tests.rs")),
        "src/cleanup_plan/replay/factored/oracle_hook.rs" => Some(include_str!("../../src/cleanup_plan/replay/factored/oracle_hook.rs")),
        "src/cleanup_plan/replay/live_oracle/source_custody/b.rs" => Some(include_str!("../../src/cleanup_plan/replay/live_oracle/source_custody/b.rs")),
        "src/cleanup_plan/replay/skeleton_work.rs" => Some(include_str!("../../src/cleanup_plan/replay/skeleton_work.rs")),
        "src/hir.rs" => Some(include_str!("../../src/hir.rs")),
        "src/hir/expr_nodes.rs" => Some(include_str!("../../src/hir/expr_nodes.rs")),
        "src/hir/function_value.rs" => Some(include_str!("../../src/hir/function_value.rs")),
        "src/hir/function_value/funded.rs" => Some(include_str!("../../src/hir/function_value/funded.rs")),
        "src/hir/funded.rs" => Some(include_str!("../../src/hir/funded.rs")),
        "src/hir/iterator_loop.rs" => Some(include_str!("../../src/hir/iterator_loop.rs")),
        "src/hir/iterator_loop/renewal.rs" => Some(include_str!("../../src/hir/iterator_loop/renewal.rs")),
        "src/hir/iterator_loop/renewal/funded.rs" => Some(include_str!("../../src/hir/iterator_loop/renewal/funded.rs")),
        "src/hir/lexical_exit.rs" => Some(include_str!("../../src/hir/lexical_exit.rs")),
        "src/hir/lexical_exit/owner_funded.rs" => Some(include_str!("../../src/hir/lexical_exit/owner_funded.rs")),
        "src/hir/lexical_exit/walk.rs" => Some(include_str!("../../src/hir/lexical_exit/walk.rs")),
        "src/hir/lexical_exit/walk/tests.rs" => Some(include_str!("../../src/hir/lexical_exit/walk/tests.rs")),
        "src/hir/validation.rs" => Some(include_str!("../../src/hir/validation.rs")),
        "src/hir/vec_loop_renewal.rs" => Some(include_str!("../../src/hir/vec_loop_renewal.rs")),
        "src/hir/vec_loop_renewal/consuming.rs" => Some(include_str!("../../src/hir/vec_loop_renewal/consuming.rs")),
        "src/hir/vec_loop_renewal/consuming/funded.rs" => Some(include_str!("../../src/hir/vec_loop_renewal/consuming/funded.rs")),
        "src/hir/vec_loop_renewal/funded.rs" => Some(include_str!("../../src/hir/vec_loop_renewal/funded.rs")),
        "src/literal_format.rs" => Some(include_str!("../../src/literal_format.rs")),
        "src/literal_format/funded.rs" => Some(include_str!("../../src/literal_format/funded.rs")),
        "src/loop_calls/byte_writer.rs" => Some(include_str!("../../src/loop_calls/byte_writer.rs")),
        "src/loop_calls/byte_writer/resolved.rs" => Some(include_str!("../../src/loop_calls/byte_writer/resolved.rs")),
        "src/loop_calls/byte_writer/resolved/funded.rs" => Some(include_str!("../../src/loop_calls/byte_writer/resolved/funded.rs")),
        "src/prelude.rs" => Some(include_str!("../../src/prelude.rs")),
        "src/source_reflows/semantic_receivers.rs" => Some(include_str!("../../src/source_reflows/semantic_receivers.rs")),
        "src/source_verify/constructor_tests.rs" => Some(include_str!("../../src/source_verify/constructor_tests.rs")),
        "src/source_verify/declaration.rs" => Some(include_str!("../../src/source_verify/declaration.rs")),
        "src/string_ops.rs" => Some(include_str!("../../src/string_ops.rs")),
        "src/string_ops/concat_funded.rs" => Some(include_str!("../../src/string_ops/concat_funded.rs")),
        "src/string_ops/conditions.rs" => Some(include_str!("../../src/string_ops/conditions.rs")),
        "src/string_ops/conditions/funded.rs" => Some(include_str!("../../src/string_ops/conditions/funded.rs")),
        "src/string_ops/replacement.rs" => Some(include_str!("../../src/string_ops/replacement.rs")),
        "src/string_ops/replacement/funded.rs" => Some(include_str!("../../src/string_ops/replacement/funded.rs")),
        "src/workspace_graph/expected_projection.rs" => Some(include_str!("../../src/workspace_graph/expected_projection.rs")),
        "src/workspace_graph/expected_projection/held_projection.rs" => Some(include_str!("../../src/workspace_graph/expected_projection/held_projection.rs")),
        "src/workspace_graph/expected_projection/import_stub/core_proofs/tests.rs" => Some(include_str!("../../src/workspace_graph/expected_projection/import_stub/core_proofs/tests.rs")),
        "src/workspace_graph/identity_resolution.rs" => Some(include_str!("../../src/workspace_graph/identity_resolution.rs")),
        "src/workspace_graph/tests/builder_limits.rs" => Some(include_str!("../../src/workspace_graph/tests/builder_limits.rs")),
        "src/workspace_graph/type_proof_tests.rs" => Some(include_str!("../../src/workspace_graph/type_proof_tests.rs")),
        "tests/hir_module_boundaries.rs" => Some(include_str!("../../tests/hir_module_boundaries.rs")),
        "tests/project/standard_library/application_json/nested_response/string_vectors.rs" => Some(include_str!("../../tests/project/standard_library/application_json/nested_response/string_vectors.rs")),
        "tests/wasm/scalar_browser_ci_contract.rs" => Some(include_str!("../../tests/wasm/scalar_browser_ci_contract.rs")),
        "tests/workspace/semantic_graph.rs" => Some(include_str!("../../tests/workspace/semantic_graph.rs")),
        "tests/workspace/semantic_graph/source_inventory.rs" => Some(include_str!("../../tests/workspace/semantic_graph/source_inventory.rs")),
        "tests/workspace/semantic_graph/source_inventory_controls.rs" => Some(include_str!("../../tests/workspace/semantic_graph/source_inventory_controls.rs")),
        "src/bounded_output/stage_debits.rs" => Some(include_str!("../../src/bounded_output/stage_debits.rs")),
        "src/bounded_output/stage_debits/tests.rs" => Some(include_str!("../../src/bounded_output/stage_debits/tests.rs")),
        "src/cleanup_plan/replay/expression_facts.rs" => Some(include_str!("../../src/cleanup_plan/replay/expression_facts.rs")),
        "src/cleanup_plan/replay/factored/canonical_shared.rs" => Some(include_str!("../../src/cleanup_plan/replay/factored/canonical_shared.rs")),
        "src/cleanup_plan/replay/path_join.rs" => Some(include_str!("../../src/cleanup_plan/replay/path_join.rs")),
        "tests/useful_data/byte_slice_for_runtime/nested.rs" => Some(include_str!("../../tests/useful_data/byte_slice_for_runtime/nested.rs")),
        _ => None,
    }
}
/// Authenticate the actual complete owner before selecting its named whole
/// historical input. No fallback or arbitrary path projection is admitted.
pub(crate) fn read(path: impl AsRef<Path>) -> std::io::Result<String> {
    read_packet(path, packet())
}
pub(crate) fn read_1e(path: impl AsRef<Path>) -> std::io::Result<String> {
    read_packet(path, packet_1e())
}
// Exact current-side 1e epoch for the independent operation-guard inverse.
// It is distinct from read_1e's incoming-main result and its complete archive.
pub(crate) fn current_1e_value(source: &str, supplied: &str) -> String {
    let packet = packet_1e();
    let rows = packet["rows"].as_array().unwrap();
    assert_eq!(rows.iter().filter(|row| row["path"] == source).count(), 1);
    let row = rows.iter().find(|row| row["path"] == source).unwrap();
    let current = crate::source_reflows_merge::next_big_batch_inputs::value(
        source,
        row["current_sha256"].as_str().unwrap(),
        supplied,
    );
    // This complete owner gained the optional observer after the 1e epoch.
    // Invert only that exact addition; the old whole-source digest authenticates
    // every remaining byte before the original guard-span assertions run.
    if source == "src/cleanup_plan/replay/expression_facts.rs"
        && digest(current.as_bytes()) != row["current_sha256"].as_str().unwrap()
    {
        assert_eq!(
            digest(current.as_bytes()),
            "8728d164bd61e2a7a8e8165bc5d0e1a1b6a652aafd56302406cc2db0a386864f"
        );
        let observer = "\n    #[cfg(feature = \"unstable-workflow-profiling\")]\n    let _exclusive_operation = crate::bounded_output::exclusive_debits::operation_guard(\n        crate::bounded_output::exclusive_debits::Operation::ReplayExpressionFacts,\n    );";
        assert_eq!(current.matches(observer).count(), 1);
        let previous = current.replacen(observer, "", 1);
        assert_eq!(
            digest(previous.as_bytes()),
            row["current_sha256"].as_str().unwrap()
        );
        previous
    } else {
        current
    }
}
// The cursor source gained exactly this later acquisition call. Restore its
// complete packet epoch before the existing full-source comparisons on both
// independently acquired channels.
fn before_cursor_call(source: &str, expected: &str, supplied: &str) -> String {
    if source != "src/cleanup_plan/replay/expression_facts/arena/cursor_tests.rs"
        || expected != "fdb672ec67714810ba990a306ec8b282ce7c628d627b272b322d21e51293da34"
        || digest(supplied.as_bytes()) == expected
    {
        return supplied.to_owned();
    }
    assert_eq!(
        digest(supplied.as_bytes()),
        "2c91db9d0da06846703b41d7221a5daf16b56aa73933f0a88b714d23d08552a2"
    );
    let current = "    let capacity_now =\n        capacity::type_cursor_tests::immediate_before(include_str!(\"../capacity.rs\"));";
    assert_eq!(supplied.matches(current).count(), 1);
    let previous = supplied.replacen(
        current,
        "    let capacity_now = include_str!(\"../capacity.rs\");",
        1,
    );
    assert_eq!(digest(previous.as_bytes()), expected);
    previous
}

// Only the current side of the two immutable packets uses this front. The
// complete live physical/compiled pair precedes the original historical
// normalization; archive readers and current RAW audits never use it.
fn latest_compiled(source: &str) -> Option<&'static str> {
    compiled(source)
        .or_else(|| super::main67107_inputs::accepted_physical_compiled(source))
        .or_else(|| {
            (source == "docs/evidence/projection-audit-scope-v1/legacy-inputs.tsv").then_some(
                include_str!("../../docs/evidence/projection-audit-scope-v1/legacy-inputs.tsv"),
            )
        })
}
fn latest_current(
    path: &Path,
    row: &serde_json::Value,
    f247: bool,
) -> Option<std::io::Result<String>> {
    let source = row["path"].as_str().unwrap();
    let compiled = latest_compiled(source)?;
    let expected = row["current_sha256"].as_str().unwrap();
    let length = row["current_length"].as_u64().unwrap() as usize;
    if compiled.len() == length && digest(compiled.as_bytes()) == expected {
        return None;
    }
    Some((|| {
        let raw = super::main67107_inputs::main_f247_file_raw(
            path,
            compiled.len(),
            &digest(compiled.as_bytes()),
        )
        .map_err(|message| std::io::Error::new(std::io::ErrorKind::InvalidData, message))?;
        assert_eq!(
            raw, compiled,
            "complete latest physical/compiled packet owner"
        );
        let historical = if f247 {
            let value = legacy_ci_inputs::for_expected(source, expected, &raw);
            super::next_big_batch_inputs::value(source, expected, &value)
        } else {
            let value = super::next_big_batch_inputs::value(source, expected, &raw);
            legacy_ci_inputs::for_expected(source, expected, &value).into_owned()
        };
        let historical =
            super::main67107_inputs::accepted_expected_value(source, expected, &historical);
        let historical = before_cursor_call(source, expected, &historical);
        assert_eq!(
            historical.len(),
            length,
            "complete historical current length"
        );
        assert_eq!(
            digest(historical.as_bytes()),
            expected,
            "complete historical current digest"
        );
        Ok(historical)
    })())
}

fn read_packet(path: impl AsRef<Path>, packet: serde_json::Value) -> std::io::Result<String> {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let path = path.as_ref();
    let relative = path.strip_prefix(root).expect("main-c883 confined source");
    assert!(relative
        .components()
        .all(|part| matches!(part, std::path::Component::Normal(_))));
    let relative = relative.to_str().unwrap();
    let rows = packet["rows"].as_array().unwrap();
    assert_eq!(
        rows.iter().filter(|row| row["path"] == relative).count(),
        1,
        "closed main-c883 source membership"
    );
    let row = rows.iter().find(|row| row["path"] == relative).unwrap();
    let latest = if packet["schema"] == "semaprax.main-c883-source-acquisition.v1" {
        latest_current(path, row, false)
    } else {
        None
    };
    let current = if let Some(latest) = latest {
        latest?
    } else if let Some(previous) = crate::source_reflows_merge::next_big_batch_inputs::file(
        path,
        row["current_length"].as_u64().unwrap() as usize,
        row["current_sha256"].as_str().unwrap(),
    ) {
        previous
    } else {
        held(
            path,
            row["current_length"].as_u64().unwrap() as usize,
            row["current_sha256"].as_str().unwrap(),
        )?
    };
    if let Some(compiled) = compiled(relative) {
        let compiled = crate::source_reflows_merge::next_big_batch_inputs::value(
            relative,
            row["current_sha256"].as_str().unwrap(),
            compiled,
        );
        let compiled = legacy_ci_inputs::for_expected(
            relative,
            row["current_sha256"].as_str().unwrap(),
            &compiled,
        );
        let compiled = super::main67107_inputs::accepted_expected_value(
            relative,
            row["current_sha256"].as_str().unwrap(),
            &compiled,
        );
        let compiled =
            before_cursor_call(relative, row["current_sha256"].as_str().unwrap(), &compiled);
        assert_eq!(
            current, compiled,
            "complete physical/compiled main-c883 owner"
        );
    }
    held(
        &root.join(row["main_archive"].as_str().unwrap()),
        row["main_length"].as_u64().unwrap() as usize,
        row["main_sha256"].as_str().unwrap(),
    )
}

#[test]
fn closed_main_c883_acquisition_authenticates_all_complete_current_owners() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let packet = packet();
    for row in packet["rows"].as_array().unwrap() {
        let current = read(root.join(row["path"].as_str().unwrap())).unwrap();
        assert_eq!(current.len() as u64, row["main_length"].as_u64().unwrap());
        assert_eq!(digest(current.as_bytes()), row["main_sha256"]);
    }
    for path in [
        root.join("not-a-main-c883-input"),
        root.join("src/../Cargo.toml"),
    ] {
        assert!(std::panic::catch_unwind(|| read(path)).is_err());
    }
}

fn packet_1e() -> serde_json::Value {
    let data =
        include_str!("../../docs/evidence/main-1e-next-source-merge-20261017/main-inputs.json");
    assert_eq!(data.len(), 10780);
    assert_eq!(
        digest(data.as_bytes()),
        "d322aa4ae642d4d0176b174d4a076ec7cdc9240cfc38bf256bd402c23d9312a3"
    );
    let packet: serde_json::Value = serde_json::from_str(data).unwrap();
    assert_eq!(packet["schema"], "semaprax.main-1e-source-acquisition.v1");
    assert_eq!(packet["rows"].as_array().unwrap().len(), 22);
    packet
}
#[test]
fn closed_main_1e_acquisition_authenticates_all_complete_current_owners() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let packet = packet_1e();
    for row in packet["rows"].as_array().unwrap() {
        let value = read_1e(root.join(row["path"].as_str().unwrap())).unwrap();
        assert_eq!(value.len() as u64, row["main_length"].as_u64().unwrap());
        assert_eq!(digest(value.as_bytes()), row["main_sha256"]);
    }
    assert!(std::panic::catch_unwind(|| read_1e(root.join("not-a-main-1e-input"))).is_err());
}

// Separate f247 acquisition. Current physical bytes are authenticated first;
// complete incoming JSON/source images remain a distinct immutable namespace.
fn packet_f247() -> serde_json::Value {
    let data = include_str!(
        "../../docs/evidence/future831-f247-final-source-binding-20261017/main-f247-inputs.json"
    );
    assert_eq!(data.len(), 218113); // F247_PACKET_LENGTH
    assert_eq!(
        digest(data.as_bytes()),
        "6f23ce8906ae025b4bc263885b423c999219a659618f401144e89b95bcadf8de"
    );
    let packet: serde_json::Value = serde_json::from_str(data).unwrap();
    assert_eq!(packet["schema"], "semaprax.main-f247-source-acquisition.v1");
    assert_eq!(packet["rows"].as_array().unwrap().len(), 408); // F247_PACKET_ROWS
    packet
}
pub(crate) fn read_f247(path: impl AsRef<Path>) -> std::io::Result<String> {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let relative = path
        .as_ref()
        .strip_prefix(root)
        .expect("confined f247 source");
    assert!(relative
        .components()
        .all(|p| matches!(p, std::path::Component::Normal(_))));
    let relative = relative.to_str().unwrap();
    let packet = packet_f247();
    let rows = packet["rows"].as_array().unwrap();
    assert_eq!(rows.iter().filter(|r| r["path"] == relative).count(), 1);
    let row = rows.iter().find(|r| r["path"] == relative).unwrap();
    let read = |path: &Path, length, expected| {
        super::main67107_inputs::main_f247_file_raw(path, length, expected)
            .map_err(|message| std::io::Error::new(std::io::ErrorKind::InvalidData, message))
    };
    let current = if let Some(latest) = latest_current(&root.join(relative), row, true) {
        latest?
    } else {
        read(
            &root.join(relative),
            row["current_length"].as_u64().unwrap() as usize,
            row["current_sha256"].as_str().unwrap(),
        )?
    };
    if let Some(compiled) =
        compiled(relative).or_else(|| super::main67107_inputs::accepted_physical_compiled(relative))
    {
        let projected = legacy_ci_inputs::for_expected(
            relative,
            row["current_sha256"].as_str().unwrap(),
            compiled,
        );
        let compiled = projected.as_ref();
        let compiled = crate::source_reflows_merge::next_big_batch_inputs::value(
            relative,
            row["current_sha256"].as_str().unwrap(),
            compiled,
        );
        let compiled = super::main67107_inputs::accepted_expected_value(
            relative,
            row["current_sha256"].as_str().unwrap(),
            &compiled,
        );
        let compiled =
            before_cursor_call(relative, row["current_sha256"].as_str().unwrap(), &compiled);
        assert_eq!(
            current, compiled,
            "actual complete compiled f247 current owner"
        );
    }
    read(
        &root.join(row["main_archive"].as_str().unwrap()),
        row["main_length"].as_u64().unwrap() as usize,
        row["main_sha256"].as_str().unwrap(),
    )
}
pub(crate) fn f247_expected(path: &str, supplied: &str) -> String {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let incoming = read_f247(root.join(path)).unwrap();
    let packet = packet_f247();
    let row = packet["rows"]
        .as_array()
        .unwrap()
        .iter()
        .find(|r| r["path"] == path)
        .unwrap();
    let actual = if let Some(latest) = latest_current(&root.join(path), row, true) {
        latest.unwrap()
    } else {
        super::main67107_inputs::main_f247_file_raw(
            &root.join(path),
            row["current_length"].as_u64().unwrap() as usize,
            row["current_sha256"].as_str().unwrap(),
        )
        .unwrap()
    };
    let supplied = if latest_compiled(path).is_some_and(|current| current == supplied) {
        actual.as_str()
    } else {
        supplied
    };
    let projected =
        legacy_ci_inputs::for_expected(path, row["current_sha256"].as_str().unwrap(), supplied);
    let supplied = projected.as_ref();
    let supplied = crate::source_reflows_merge::next_big_batch_inputs::value(
        path,
        row["current_sha256"].as_str().unwrap(),
        supplied,
    );
    assert!(
        supplied == actual || supplied == incoming,
        "foreign f247 supplied source"
    );
    incoming
}
#[test]
fn closed_f247_acquisition_authenticates_whole_current_and_incoming_domains() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let packet = packet_f247();
    for row in packet["rows"].as_array().unwrap() {
        let value = read_f247(root.join(row["path"].as_str().unwrap())).unwrap();
        assert_eq!(value.len() as u64, row["main_length"].as_u64().unwrap());
        assert_eq!(digest(value.as_bytes()), row["main_sha256"]);
    }
    assert!(std::panic::catch_unwind(|| read_f247(root.join("foreign-f247-source"))).is_err());
}

#[test]
fn latest_packet_front_keeps_whole_history_and_rejects_changed_physical_bytes() {
    let packet = packet_f247();
    let row = packet["rows"]
        .as_array()
        .unwrap()
        .iter()
        .find(|row| row["path"] == "src/cleanup_plan/replay.rs")
        .unwrap();
    let compiled = latest_compiled(row["path"].as_str().unwrap()).unwrap();
    let directory =
        std::env::temp_dir().join(format!("semaprax-closed-packet-{}", std::process::id()));
    std::fs::create_dir(&directory).unwrap();
    struct Remove(std::path::PathBuf);
    impl Drop for Remove {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }
    let _remove = Remove(directory.clone());
    let file = directory.join("owner.rs");
    std::fs::write(&file, compiled).unwrap();
    let before = latest_current(&file, row, true).unwrap().unwrap();
    assert_eq!(before.len() as u64, row["current_length"].as_u64().unwrap());
    assert_eq!(digest(before.as_bytes()), row["current_sha256"]);
    let mut changed = compiled.as_bytes().to_vec();
    changed[0] ^= 1;
    for bad in [Vec::new(), changed, [compiled.as_bytes(), b" "].concat()] {
        std::fs::write(&file, bad).unwrap();
        assert!(latest_current(&file, row, true).unwrap().is_err());
    }
    std::fs::write(&file, compiled).unwrap();
    #[cfg(unix)]
    {
        let link = directory.join("symlink.rs");
        std::os::unix::fs::symlink(&file, &link).unwrap();
        assert!(latest_current(&link, row, true).unwrap().is_err());
    }
    assert!(latest_current(&directory, row, true).unwrap().is_err());
    assert!(latest_current(&directory.join("missing.rs"), row, true)
        .unwrap()
        .is_err());
    let source = row["path"].as_str().unwrap();
    let incoming = read_f247(Path::new(env!("CARGO_MANIFEST_DIR")).join(source)).unwrap();
    assert_eq!(f247_expected(source, compiled), incoming);
    assert_eq!(f247_expected(source, &incoming), incoming);
    for foreign in [
        String::new(),
        format!("{compiled} "),
        format!("{incoming} "),
    ] {
        assert!(std::panic::catch_unwind(|| f247_expected(source, &foreign)).is_err());
    }
}

#[test]
fn exact_cursor_epoch_keeps_complete_preimage_and_refuses_foreign_bytes() {
    let source = "src/cleanup_plan/replay/expression_facts/arena/cursor_tests.rs";
    let expected = "fdb672ec67714810ba990a306ec8b282ce7c628d627b272b322d21e51293da34";
    let current = latest_compiled(source).unwrap();
    let old = before_cursor_call(source, expected, current);
    assert_eq!(old.len(), 8127);
    assert_eq!(before_cursor_call(source, expected, &old), old);
    assert_eq!(before_cursor_call("foreign", expected, current), current);
    assert_eq!(before_cursor_call(source, "foreign", current), current);
    for bad in [String::new(), format!("{current} "), format!("{old} ")] {
        assert!(std::panic::catch_unwind(|| before_cursor_call(source, expected, &bad)).is_err());
    }
}
