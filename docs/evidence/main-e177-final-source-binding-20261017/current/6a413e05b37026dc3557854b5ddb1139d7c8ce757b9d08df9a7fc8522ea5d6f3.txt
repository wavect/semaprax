//! Closed current-byte authentication before the complete incominge177 view.
use sha2::{Digest,Sha256};
use std::path::Path;
struct LowerHex<T>(T);
impl<T: AsRef<[u8]>> std::fmt::LowerHex for LowerHex<T> {
    fn fmt(&self, output: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        for byte in self.0.as_ref() { write!(output,"{byte:02x}")?; } Ok(())
    }
}
const MAIN_E177_CAP: usize = 9_753_094;
fn main_e177_body(reader: &mut impl std::io::Read, length: usize, expected: &str) -> Result<String, &'static str> {
    if length > MAIN_E177_CAP { return Err("bound"); }
    let mut bytes = Vec::new();
    bytes.try_reserve_exact(length).map_err(|_| "allocation")?;
    bytes.resize(length, 0);
    reader.read_exact(&mut bytes).map_err(|_| "truncated")?;
    let mut extra = [0u8; 1];
    if reader.read(&mut extra).map_err(|_| "read")? != 0 { return Err("extra"); }
    if main_e177_digest(&bytes) != expected { return Err("digest"); }
    String::from_utf8(bytes).map_err(|_| "utf8")
}
fn main_e177_digest(bytes: &[u8]) -> String {
    format!("{:x}", LowerHex(Sha256::digest(bytes)))
}
fn main_e177_same_file(a: &std::fs::Metadata, b: &std::fs::Metadata) -> bool {
    if !a.is_file() || !b.is_file() || a.len() != b.len() || a.modified().ok() != b.modified().ok() { return false; }
    #[cfg(unix)]
    { use std::os::unix::fs::MetadataExt; if a.dev() != b.dev() || a.ino() != b.ino() || a.ctime() != b.ctime() || a.ctime_nsec() != b.ctime_nsec() { return false; } }
    #[cfg(windows)]
    { use std::os::windows::fs::MetadataExt; if a.file_attributes() & 0x400 != 0 || b.file_attributes() & 0x400 != 0 || a.creation_time() != b.creation_time() || a.last_write_time() != b.last_write_time() { return false; } }
    true
}
pub(crate) fn main_e177_file_raw(path: &::std::path::Path, length: usize, expected: &str) -> Result<String, &'static str> {
    if length > MAIN_E177_CAP { return Err("bound"); }
    let path_before = std::fs::symlink_metadata(path).map_err(|_| "missing")?;
    if path_before.file_type().is_symlink() || !path_before.is_file() { return Err("length-or-nonregular"); }
    let mut options = std::fs::OpenOptions::new(); options.read(true);
    #[cfg(unix)]
    { use std::os::unix::fs::OpenOptionsExt; options.custom_flags(libc::O_NOFOLLOW | libc::O_CLOEXEC | libc::O_NONBLOCK); }
    #[cfg(windows)]
    { use std::os::windows::fs::OpenOptionsExt; options.custom_flags(0x0020_0000); }
    #[cfg(not(any(unix, windows)))]
    { return Err("platform"); }
    let mut file = options.open(path).map_err(|_| "open")?;
    let metadata = file.metadata().map_err(|_| "metadata")?;
    if !main_e177_same_file(&path_before, &metadata) || metadata.len() != u64::try_from(length).map_err(|_| "bound")? {
        return Err("length-or-nonregular");
    }
    // The one opened no-follow regular handle is checked before any buffer.
    let value = main_e177_body(&mut file, length, expected)?;
    let after = file.metadata().map_err(|_| "metadata")?;
    let path_after = std::fs::symlink_metadata(path).map_err(|_| "metadata")?;
    if path_after.file_type().is_symlink() || !main_e177_same_file(&metadata, &after) || !main_e177_same_file(&after, &path_after) { return Err("metadata-drift"); }
    Ok(value)
}


fn digest(bytes: &[u8]) -> String { format!("{:x}", LowerHex(Sha256::digest(bytes))) }
fn packet() -> serde_json::Value {
    let data=include_str!(concat!(env!("CARGO_MANIFEST_DIR"),"/docs/evidence/main-e177-full-controller-acquisition-20261017/inputs.json"));
    assert_eq!(data.len(), 237694); // MAIN_E177_PACKET_LENGTH
    assert_eq!(digest(data.as_bytes()), "eba5f197707cd4b1e8229907011a2d1640af433f12f41927c1496d022aa52d05"); // MAIN_E177_PACKET_SHA
    let held=main_e177_file_raw(&Path::new(env!("CARGO_MANIFEST_DIR")).join("docs/evidence/main-e177-full-controller-acquisition-20261017/inputs.json"),
        data.len(), &digest(data.as_bytes())).unwrap();
    assert_eq!(held,data,"whole actual and compiled MAIN_E177 namespace");
    let packet:serde_json::Value=serde_json::from_str(data).unwrap();
    assert_eq!(packet["schema"],"semaprax.main-e177-whole-source-acquisition.v1");
    assert_eq!(packet["rows"].as_array().unwrap().len(),471);
    packet
}
fn compiled(path: &str) -> Option<&'static str> {
    match path {
        "crates/semaprax-native-rust-interop-builder/src/implementation/tests/ledger_capacity/source_inventory.rs" => Some(include_str!(concat!(env!("CARGO_MANIFEST_DIR"),"/crates/semaprax-native-rust-interop-builder/src/implementation/tests/ledger_capacity/source_inventory.rs"))),
        "src/ast.rs" => Some(include_str!(concat!(env!("CARGO_MANIFEST_DIR"),"/src/ast.rs"))),
        "src/ast/drop_method_original.rs" => Some(include_str!(concat!(env!("CARGO_MANIFEST_DIR"),"/src/ast/drop_method_original.rs"))),
        "src/ast/drop_original.rs" => Some(include_str!(concat!(env!("CARGO_MANIFEST_DIR"),"/src/ast/drop_original.rs"))),
        "src/bounded_output/stage_debits.rs" => Some(include_str!(concat!(env!("CARGO_MANIFEST_DIR"),"/src/bounded_output/stage_debits.rs"))),
        "src/bounded_output/stage_debits/tests.rs" => Some(include_str!(concat!(env!("CARGO_MANIFEST_DIR"),"/src/bounded_output/stage_debits/tests.rs"))),
        "src/byte_ops.rs" => Some(include_str!(concat!(env!("CARGO_MANIFEST_DIR"),"/src/byte_ops.rs"))),
        "src/cleanup_plan.rs" => Some(include_str!(concat!(env!("CARGO_MANIFEST_DIR"),"/src/cleanup_plan.rs"))),
        "src/cleanup_plan/renewal_profile.rs" => Some(include_str!(concat!(env!("CARGO_MANIFEST_DIR"),"/src/cleanup_plan/renewal_profile.rs"))),
        "src/cleanup_plan/renewal_profile/funded.rs" => Some(include_str!(concat!(env!("CARGO_MANIFEST_DIR"),"/src/cleanup_plan/renewal_profile/funded.rs"))),
        "src/cleanup_plan/replay.rs" => Some(include_str!(concat!(env!("CARGO_MANIFEST_DIR"),"/src/cleanup_plan/replay.rs"))),
        "src/cleanup_plan/replay/allocation.rs" => Some(include_str!(concat!(env!("CARGO_MANIFEST_DIR"),"/src/cleanup_plan/replay/allocation.rs"))),
        "src/cleanup_plan/replay/allocation/oracle.rs" => Some(include_str!(concat!(env!("CARGO_MANIFEST_DIR"),"/src/cleanup_plan/replay/allocation/oracle.rs"))),
        "src/cleanup_plan/replay/allocation/tests.rs" => Some(include_str!(concat!(env!("CARGO_MANIFEST_DIR"),"/src/cleanup_plan/replay/allocation/tests.rs"))),
        "src/cleanup_plan/replay/allocation/tests/accounting_scope.rs" => Some(include_str!(concat!(env!("CARGO_MANIFEST_DIR"),"/src/cleanup_plan/replay/allocation/tests/accounting_scope.rs"))),
        "src/cleanup_plan/replay/branch_clone.rs" => Some(include_str!(concat!(env!("CARGO_MANIFEST_DIR"),"/src/cleanup_plan/replay/branch_clone.rs"))),
        "src/cleanup_plan/replay/branch_clone/tests.rs" => Some(include_str!(concat!(env!("CARGO_MANIFEST_DIR"),"/src/cleanup_plan/replay/branch_clone/tests.rs"))),
        "src/cleanup_plan/replay/continuation_storage.rs" => Some(include_str!(concat!(env!("CARGO_MANIFEST_DIR"),"/src/cleanup_plan/replay/continuation_storage.rs"))),
        "src/cleanup_plan/replay/continuation_storage/tests.rs" => Some(include_str!(concat!(env!("CARGO_MANIFEST_DIR"),"/src/cleanup_plan/replay/continuation_storage/tests.rs"))),
        "src/cleanup_plan/replay/expression_facts.rs" => Some(include_str!(concat!(env!("CARGO_MANIFEST_DIR"),"/src/cleanup_plan/replay/expression_facts.rs"))),
        "src/cleanup_plan/replay/expression_facts/arena.rs" => Some(include_str!(concat!(env!("CARGO_MANIFEST_DIR"),"/src/cleanup_plan/replay/expression_facts/arena.rs"))),
        "src/cleanup_plan/replay/expression_facts/arena/cursor_tests.rs" => Some(include_str!(concat!(env!("CARGO_MANIFEST_DIR"),"/src/cleanup_plan/replay/expression_facts/arena/cursor_tests.rs"))),
        "src/cleanup_plan/replay/expression_facts/arena/prefix.rs" => Some(include_str!(concat!(env!("CARGO_MANIFEST_DIR"),"/src/cleanup_plan/replay/expression_facts/arena/prefix.rs"))),
        "src/cleanup_plan/replay/expression_facts/arena/prefix/tests.rs" => Some(include_str!(concat!(env!("CARGO_MANIFEST_DIR"),"/src/cleanup_plan/replay/expression_facts/arena/prefix/tests.rs"))),
        "src/cleanup_plan/replay/expression_facts/ordinary_before.rs" => Some(include_str!(concat!(env!("CARGO_MANIFEST_DIR"),"/src/cleanup_plan/replay/expression_facts/ordinary_before.rs"))),
        "src/cleanup_plan/replay/expression_facts/receiving.rs" => Some(include_str!(concat!(env!("CARGO_MANIFEST_DIR"),"/src/cleanup_plan/replay/expression_facts/receiving.rs"))),
        "src/cleanup_plan/replay/expression_facts/receiving/directory.rs" => Some(include_str!(concat!(env!("CARGO_MANIFEST_DIR"),"/src/cleanup_plan/replay/expression_facts/receiving/directory.rs"))),
        "src/cleanup_plan/replay/expression_facts/receiving/directory/status_prefix.rs" => Some(include_str!(concat!(env!("CARGO_MANIFEST_DIR"),"/src/cleanup_plan/replay/expression_facts/receiving/directory/status_prefix.rs"))),
        "src/cleanup_plan/replay/expression_facts/receiving/directory/status_prefix/tests.rs" => Some(include_str!(concat!(env!("CARGO_MANIFEST_DIR"),"/src/cleanup_plan/replay/expression_facts/receiving/directory/status_prefix/tests.rs"))),
        "src/cleanup_plan/replay/expression_facts/receiving/helpers.rs" => Some(include_str!(concat!(env!("CARGO_MANIFEST_DIR"),"/src/cleanup_plan/replay/expression_facts/receiving/helpers.rs"))),
        "src/cleanup_plan/replay/expression_facts/receiving/helpers/before_remaining.rs" => Some(include_str!(concat!(env!("CARGO_MANIFEST_DIR"),"/src/cleanup_plan/replay/expression_facts/receiving/helpers/before_remaining.rs"))),
        "src/cleanup_plan/replay/expression_facts/receiving/helpers/matching.rs" => Some(include_str!(concat!(env!("CARGO_MANIFEST_DIR"),"/src/cleanup_plan/replay/expression_facts/receiving/helpers/matching.rs"))),
        "src/cleanup_plan/replay/expression_facts/receiving/helpers/matching/tests.rs" => Some(include_str!(concat!(env!("CARGO_MANIFEST_DIR"),"/src/cleanup_plan/replay/expression_facts/receiving/helpers/matching/tests.rs"))),
        "src/cleanup_plan/replay/expression_facts/receiving/helpers/regions.rs" => Some(include_str!(concat!(env!("CARGO_MANIFEST_DIR"),"/src/cleanup_plan/replay/expression_facts/receiving/helpers/regions.rs"))),
        "src/cleanup_plan/replay/expression_facts/receiving/helpers/tests.rs" => Some(include_str!(concat!(env!("CARGO_MANIFEST_DIR"),"/src/cleanup_plan/replay/expression_facts/receiving/helpers/tests.rs"))),
        "src/cleanup_plan/replay/expression_facts/receiving/remaining.rs" => Some(include_str!(concat!(env!("CARGO_MANIFEST_DIR"),"/src/cleanup_plan/replay/expression_facts/receiving/remaining.rs"))),
        "src/cleanup_plan/replay/expression_facts/receiving/remaining/selected.rs" => Some(include_str!(concat!(env!("CARGO_MANIFEST_DIR"),"/src/cleanup_plan/replay/expression_facts/receiving/remaining/selected.rs"))),
        "src/cleanup_plan/replay/expression_facts/receiving/remaining/tests.rs" => Some(include_str!(concat!(env!("CARGO_MANIFEST_DIR"),"/src/cleanup_plan/replay/expression_facts/receiving/remaining/tests.rs"))),
        "src/cleanup_plan/replay/expression_facts/receiving/retained.rs" => Some(include_str!(concat!(env!("CARGO_MANIFEST_DIR"),"/src/cleanup_plan/replay/expression_facts/receiving/retained.rs"))),
        "src/cleanup_plan/replay/expression_facts/receiving/retained/edge_before.rs" => Some(include_str!(concat!(env!("CARGO_MANIFEST_DIR"),"/src/cleanup_plan/replay/expression_facts/receiving/retained/edge_before.rs"))),
        "src/cleanup_plan/replay/expression_facts/receiving/retained/edge_tests.rs" => Some(include_str!(concat!(env!("CARGO_MANIFEST_DIR"),"/src/cleanup_plan/replay/expression_facts/receiving/retained/edge_tests.rs"))),
        "src/cleanup_plan/replay/expression_facts/receiving/retained/exit_before.rs" => Some(include_str!(concat!(env!("CARGO_MANIFEST_DIR"),"/src/cleanup_plan/replay/expression_facts/receiving/retained/exit_before.rs"))),
        "src/cleanup_plan/replay/expression_facts/receiving/retained/exit_tests.rs" => Some(include_str!(concat!(env!("CARGO_MANIFEST_DIR"),"/src/cleanup_plan/replay/expression_facts/receiving/retained/exit_tests.rs"))),
        "src/cleanup_plan/replay/expression_facts/receiving/retained/exits.rs" => Some(include_str!(concat!(env!("CARGO_MANIFEST_DIR"),"/src/cleanup_plan/replay/expression_facts/receiving/retained/exits.rs"))),
        "src/cleanup_plan/replay/expression_facts/receiving/retained/status_before.rs" => Some(include_str!(concat!(env!("CARGO_MANIFEST_DIR"),"/src/cleanup_plan/replay/expression_facts/receiving/retained/status_before.rs"))),
        "src/cleanup_plan/replay/expression_facts/receiving/retained/tests.rs" => Some(include_str!(concat!(env!("CARGO_MANIFEST_DIR"),"/src/cleanup_plan/replay/expression_facts/receiving/retained/tests.rs"))),
        "src/cleanup_plan/replay/factored.rs" => Some(include_str!(concat!(env!("CARGO_MANIFEST_DIR"),"/src/cleanup_plan/replay/factored.rs"))),
        "src/cleanup_plan/replay/factored/borrowed_canonical/dictionary/keys.rs" => Some(include_str!(concat!(env!("CARGO_MANIFEST_DIR"),"/src/cleanup_plan/replay/factored/borrowed_canonical/dictionary/keys.rs"))),
        "src/cleanup_plan/replay/factored/borrowed_canonical/tests.rs" => Some(include_str!(concat!(env!("CARGO_MANIFEST_DIR"),"/src/cleanup_plan/replay/factored/borrowed_canonical/tests.rs"))),
        "src/cleanup_plan/replay/factored/canonical_before.rs" => Some(include_str!(concat!(env!("CARGO_MANIFEST_DIR"),"/src/cleanup_plan/replay/factored/canonical_before.rs"))),
        "src/cleanup_plan/replay/factored/canonical_shared.rs" => Some(include_str!(concat!(env!("CARGO_MANIFEST_DIR"),"/src/cleanup_plan/replay/factored/canonical_shared.rs"))),
        "src/cleanup_plan/replay/factored/oracle_hook.rs" => Some(include_str!(concat!(env!("CARGO_MANIFEST_DIR"),"/src/cleanup_plan/replay/factored/oracle_hook.rs"))),
        "src/cleanup_plan/replay/live_oracle/source_custody/a.rs" => Some(include_str!(concat!(env!("CARGO_MANIFEST_DIR"),"/src/cleanup_plan/replay/live_oracle/source_custody/a.rs"))),
        "src/cleanup_plan/replay/live_oracle/source_custody/b.rs" => Some(include_str!(concat!(env!("CARGO_MANIFEST_DIR"),"/src/cleanup_plan/replay/live_oracle/source_custody/b.rs"))),
        "src/cleanup_plan/replay/paid_type_facts/inventory_tests.rs" => Some(include_str!(concat!(env!("CARGO_MANIFEST_DIR"),"/src/cleanup_plan/replay/paid_type_facts/inventory_tests.rs"))),
        "src/cleanup_plan/replay/path_join.rs" => Some(include_str!(concat!(env!("CARGO_MANIFEST_DIR"),"/src/cleanup_plan/replay/path_join.rs"))),
        "src/cleanup_plan/replay/schema/paid_summary/tests.rs" => Some(include_str!(concat!(env!("CARGO_MANIFEST_DIR"),"/src/cleanup_plan/replay/schema/paid_summary/tests.rs"))),
        "src/cleanup_plan/replay/selected_entry.rs" => Some(include_str!(concat!(env!("CARGO_MANIFEST_DIR"),"/src/cleanup_plan/replay/selected_entry.rs"))),
        "src/cleanup_plan/replay/selected_entry/tests/fact_errors.rs" => Some(include_str!(concat!(env!("CARGO_MANIFEST_DIR"),"/src/cleanup_plan/replay/selected_entry/tests/fact_errors.rs"))),
        "src/cleanup_plan/replay/selected_entry/tests/session_errors.rs" => Some(include_str!(concat!(env!("CARGO_MANIFEST_DIR"),"/src/cleanup_plan/replay/selected_entry/tests/session_errors.rs"))),
        "src/cleanup_plan/replay/selected_entry/tests/session_errors/source.rs" => Some(include_str!(concat!(env!("CARGO_MANIFEST_DIR"),"/src/cleanup_plan/replay/selected_entry/tests/session_errors/source.rs"))),
        "src/cleanup_plan/replay/selected_profile.rs" => Some(include_str!(concat!(env!("CARGO_MANIFEST_DIR"),"/src/cleanup_plan/replay/selected_profile.rs"))),
        "src/cleanup_plan/replay/skeleton_work.rs" => Some(include_str!(concat!(env!("CARGO_MANIFEST_DIR"),"/src/cleanup_plan/replay/skeleton_work.rs"))),
        "src/cleanup_plan/replay/skeleton_work/dag_storage/tests/tail_moves.rs" => Some(include_str!(concat!(env!("CARGO_MANIFEST_DIR"),"/src/cleanup_plan/replay/skeleton_work/dag_storage/tests/tail_moves.rs"))),
        "src/cleanup_plan/replay/skeleton_work/dag_storage/tests/tail_moves/old.rs" => Some(include_str!(concat!(env!("CARGO_MANIFEST_DIR"),"/src/cleanup_plan/replay/skeleton_work/dag_storage/tests/tail_moves/old.rs"))),
        "src/cleanup_plan/replay/skeleton_work/isolated_paths.rs" => Some(include_str!(concat!(env!("CARGO_MANIFEST_DIR"),"/src/cleanup_plan/replay/skeleton_work/isolated_paths.rs"))),
        "src/cleanup_plan/replay/skeleton_work/isolated_paths/tests.rs" => Some(include_str!(concat!(env!("CARGO_MANIFEST_DIR"),"/src/cleanup_plan/replay/skeleton_work/isolated_paths/tests.rs"))),
        "src/cleanup_plan/replay/skeleton_work/isolated_paths/tests/before.rs" => Some(include_str!(concat!(env!("CARGO_MANIFEST_DIR"),"/src/cleanup_plan/replay/skeleton_work/isolated_paths/tests/before.rs"))),
        "src/cleanup_plan/replay/skeleton_work/ordinary_dag/tests.rs" => Some(include_str!(concat!(env!("CARGO_MANIFEST_DIR"),"/src/cleanup_plan/replay/skeleton_work/ordinary_dag/tests.rs"))),
        "src/graph.rs" => Some(include_str!(concat!(env!("CARGO_MANIFEST_DIR"),"/src/graph.rs"))),
        "src/hir.rs" => Some(include_str!(concat!(env!("CARGO_MANIFEST_DIR"),"/src/hir.rs"))),
        "src/hir/borrowed_root_oracle/resolve_expr.rs" => Some(include_str!(concat!(env!("CARGO_MANIFEST_DIR"),"/src/hir/borrowed_root_oracle/resolve_expr.rs"))),
        "src/hir/borrowed_root_oracle/resolve_expr/capacity.rs" => Some(include_str!(concat!(env!("CARGO_MANIFEST_DIR"),"/src/hir/borrowed_root_oracle/resolve_expr/capacity.rs"))),
        "src/hir/borrowed_root_oracle/resolve_expr/capacity/tests.rs" => Some(include_str!(concat!(env!("CARGO_MANIFEST_DIR"),"/src/hir/borrowed_root_oracle/resolve_expr/capacity/tests.rs"))),
        "src/hir/borrowed_root_oracle/resolve_expr/vec_constructor.rs" => Some(include_str!(concat!(env!("CARGO_MANIFEST_DIR"),"/src/hir/borrowed_root_oracle/resolve_expr/vec_constructor.rs"))),
        "src/hir/borrowed_root_oracle/resolve_for/record.rs" => Some(include_str!(concat!(env!("CARGO_MANIFEST_DIR"),"/src/hir/borrowed_root_oracle/resolve_for/record.rs"))),
        "src/hir/declaration_index.rs" => Some(include_str!(concat!(env!("CARGO_MANIFEST_DIR"),"/src/hir/declaration_index.rs"))),
        "src/hir/declaration_index/flags/nominal_key.rs" => Some(include_str!(concat!(env!("CARGO_MANIFEST_DIR"),"/src/hir/declaration_index/flags/nominal_key.rs"))),
        "src/hir/expr_nodes.rs" => Some(include_str!(concat!(env!("CARGO_MANIFEST_DIR"),"/src/hir/expr_nodes.rs"))),
        "src/hir/function_value.rs" => Some(include_str!(concat!(env!("CARGO_MANIFEST_DIR"),"/src/hir/function_value.rs"))),
        "src/hir/function_value/funded.rs" => Some(include_str!(concat!(env!("CARGO_MANIFEST_DIR"),"/src/hir/function_value/funded.rs"))),
        "src/hir/funded.rs" => Some(include_str!(concat!(env!("CARGO_MANIFEST_DIR"),"/src/hir/funded.rs"))),
        "src/hir/identity_resolution/nominal_tests/legacy.rs" => Some(include_str!(concat!(env!("CARGO_MANIFEST_DIR"),"/src/hir/identity_resolution/nominal_tests/legacy.rs"))),
        "src/hir/iterator_loop.rs" => Some(include_str!(concat!(env!("CARGO_MANIFEST_DIR"),"/src/hir/iterator_loop.rs"))),
        "src/hir/iterator_loop/renewal.rs" => Some(include_str!(concat!(env!("CARGO_MANIFEST_DIR"),"/src/hir/iterator_loop/renewal.rs"))),
        "src/hir/iterator_loop/renewal/funded.rs" => Some(include_str!(concat!(env!("CARGO_MANIFEST_DIR"),"/src/hir/iterator_loop/renewal/funded.rs"))),
        "src/hir/lexical_exit.rs" => Some(include_str!(concat!(env!("CARGO_MANIFEST_DIR"),"/src/hir/lexical_exit.rs"))),
        "src/hir/lexical_exit/owner_funded.rs" => Some(include_str!(concat!(env!("CARGO_MANIFEST_DIR"),"/src/hir/lexical_exit/owner_funded.rs"))),
        "src/hir/lexical_exit/walk.rs" => Some(include_str!(concat!(env!("CARGO_MANIFEST_DIR"),"/src/hir/lexical_exit/walk.rs"))),
        "src/hir/lexical_exit/walk/tests.rs" => Some(include_str!(concat!(env!("CARGO_MANIFEST_DIR"),"/src/hir/lexical_exit/walk/tests.rs"))),
        "src/hir/nominal_payload/legacy/source_tests.rs" => Some(include_str!(concat!(env!("CARGO_MANIFEST_DIR"),"/src/hir/nominal_payload/legacy/source_tests.rs"))),
        "src/hir/owned_collection_record/saved.rs" => Some(include_str!(concat!(env!("CARGO_MANIFEST_DIR"),"/src/hir/owned_collection_record/saved.rs"))),
        "src/hir/owned_text_record/paid/tests.rs" => Some(include_str!(concat!(env!("CARGO_MANIFEST_DIR"),"/src/hir/owned_text_record/paid/tests.rs"))),
        "src/hir/owned_text_record/saved.rs" => Some(include_str!(concat!(env!("CARGO_MANIFEST_DIR"),"/src/hir/owned_text_record/saved.rs"))),
        "src/hir/validation.rs" => Some(include_str!(concat!(env!("CARGO_MANIFEST_DIR"),"/src/hir/validation.rs"))),
        "src/hir/validation/canonical_loans/ownership_tests.rs" => Some(include_str!(concat!(env!("CARGO_MANIFEST_DIR"),"/src/hir/validation/canonical_loans/ownership_tests.rs"))),
        "src/hir/validation/closure.rs" => Some(include_str!(concat!(env!("CARGO_MANIFEST_DIR"),"/src/hir/validation/closure.rs"))),
        "src/hir/validation/constructor_seed.rs" => Some(include_str!(concat!(env!("CARGO_MANIFEST_DIR"),"/src/hir/validation/constructor_seed.rs"))),
        "src/hir/validation/constructor_seed/intrinsic_tests.rs" => Some(include_str!(concat!(env!("CARGO_MANIFEST_DIR"),"/src/hir/validation/constructor_seed/intrinsic_tests.rs"))),
        "src/hir/validation/constructor_seed/original.rs" => Some(include_str!(concat!(env!("CARGO_MANIFEST_DIR"),"/src/hir/validation/constructor_seed/original.rs"))),
        "src/hir/validation/executable_profiles/before_812.rs" => Some(include_str!(concat!(env!("CARGO_MANIFEST_DIR"),"/src/hir/validation/executable_profiles/before_812.rs"))),
        "src/hir/validation/function_lookup.rs" => Some(include_str!(concat!(env!("CARGO_MANIFEST_DIR"),"/src/hir/validation/function_lookup.rs"))),
        "src/hir/validation/function_lookup/paid_custody.rs" => Some(include_str!(concat!(env!("CARGO_MANIFEST_DIR"),"/src/hir/validation/function_lookup/paid_custody.rs"))),
        "src/hir/validation/function_scope/empty_ensures/before.rs" => Some(include_str!(concat!(env!("CARGO_MANIFEST_DIR"),"/src/hir/validation/function_scope/empty_ensures/before.rs"))),
        "src/hir/validation/function_scope/lifecycle/completed.rs" => Some(include_str!(concat!(env!("CARGO_MANIFEST_DIR"),"/src/hir/validation/function_scope/lifecycle/completed.rs"))),
        "src/hir/validation/function_scope/tests.rs" => Some(include_str!(concat!(env!("CARGO_MANIFEST_DIR"),"/src/hir/validation/function_scope/tests.rs"))),
        "src/hir/validation/generic_template/call_graph/tests.rs" => Some(include_str!(concat!(env!("CARGO_MANIFEST_DIR"),"/src/hir/validation/generic_template/call_graph/tests.rs"))),
        "src/hir/vec_loop_renewal.rs" => Some(include_str!(concat!(env!("CARGO_MANIFEST_DIR"),"/src/hir/vec_loop_renewal.rs"))),
        "src/hir/vec_loop_renewal/consuming.rs" => Some(include_str!(concat!(env!("CARGO_MANIFEST_DIR"),"/src/hir/vec_loop_renewal/consuming.rs"))),
        "src/hir/vec_loop_renewal/consuming/funded.rs" => Some(include_str!(concat!(env!("CARGO_MANIFEST_DIR"),"/src/hir/vec_loop_renewal/consuming/funded.rs"))),
        "src/hir/vec_loop_renewal/funded.rs" => Some(include_str!(concat!(env!("CARGO_MANIFEST_DIR"),"/src/hir/vec_loop_renewal/funded.rs"))),
        "src/literal_format.rs" => Some(include_str!(concat!(env!("CARGO_MANIFEST_DIR"),"/src/literal_format.rs"))),
        "src/literal_format/funded.rs" => Some(include_str!(concat!(env!("CARGO_MANIFEST_DIR"),"/src/literal_format/funded.rs"))),
        "src/loan_plan/paid_plan_legacy.rs" => Some(include_str!(concat!(env!("CARGO_MANIFEST_DIR"),"/src/loan_plan/paid_plan_legacy.rs"))),
        "src/loop_calls/byte_writer.rs" => Some(include_str!(concat!(env!("CARGO_MANIFEST_DIR"),"/src/loop_calls/byte_writer.rs"))),
        "src/loop_calls/byte_writer/resolved.rs" => Some(include_str!(concat!(env!("CARGO_MANIFEST_DIR"),"/src/loop_calls/byte_writer/resolved.rs"))),
        "src/loop_calls/byte_writer/resolved/funded.rs" => Some(include_str!(concat!(env!("CARGO_MANIFEST_DIR"),"/src/loop_calls/byte_writer/resolved/funded.rs"))),
        "src/loop_calls/byte_writer/source.rs" => Some(include_str!(concat!(env!("CARGO_MANIFEST_DIR"),"/src/loop_calls/byte_writer/source.rs"))),
        "src/loop_calls/byte_writer/source_index.rs" => Some(include_str!(concat!(env!("CARGO_MANIFEST_DIR"),"/src/loop_calls/byte_writer/source_index.rs"))),
        "src/loop_calls/byte_writer/source_index_original.rs" => Some(include_str!(concat!(env!("CARGO_MANIFEST_DIR"),"/src/loop_calls/byte_writer/source_index_original.rs"))),
        "src/loop_calls/byte_writer/tests.rs" => Some(include_str!(concat!(env!("CARGO_MANIFEST_DIR"),"/src/loop_calls/byte_writer/tests.rs"))),
        "src/loop_calls/byte_writer/tests/string_slots.rs" => Some(include_str!(concat!(env!("CARGO_MANIFEST_DIR"),"/src/loop_calls/byte_writer/tests/string_slots.rs"))),
        "src/prelude.rs" => Some(include_str!(concat!(env!("CARGO_MANIFEST_DIR"),"/src/prelude.rs"))),
        "src/source_reflows.rs" => Some(include_str!(concat!(env!("CARGO_MANIFEST_DIR"),"/src/source_reflows.rs"))),
        "src/source_reflows/semantic_receivers.rs" => Some(include_str!(concat!(env!("CARGO_MANIFEST_DIR"),"/src/source_reflows/semantic_receivers.rs"))),
        "src/source_verify/constructor_tests.rs" => Some(include_str!(concat!(env!("CARGO_MANIFEST_DIR"),"/src/source_verify/constructor_tests.rs"))),
        "src/source_verify/declaration.rs" => Some(include_str!(concat!(env!("CARGO_MANIFEST_DIR"),"/src/source_verify/declaration.rs"))),
        "src/source_verify/declaration/declarations/tests.rs" => Some(include_str!(concat!(env!("CARGO_MANIFEST_DIR"),"/src/source_verify/declaration/declarations/tests.rs"))),
        "src/source_verify/declaration/functions.rs" => Some(include_str!(concat!(env!("CARGO_MANIFEST_DIR"),"/src/source_verify/declaration/functions.rs"))),
        "src/source_verify/declaration/functions/public_signature_before.rs" => Some(include_str!(concat!(env!("CARGO_MANIFEST_DIR"),"/src/source_verify/declaration/functions/public_signature_before.rs"))),
        "src/source_verify/iterative/calls/calls_before_components.rs" => Some(include_str!(concat!(env!("CARGO_MANIFEST_DIR"),"/src/source_verify/iterative/calls/calls_before_components.rs"))),
        "src/source_verify/iterative/enter.rs" => Some(include_str!(concat!(env!("CARGO_MANIFEST_DIR"),"/src/source_verify/iterative/enter.rs"))),
        "src/source_verify/iterative/enter/enter_before_components.rs" => Some(include_str!(concat!(env!("CARGO_MANIFEST_DIR"),"/src/source_verify/iterative/enter/enter_before_components.rs"))),
        "src/source_verify/iterative/match_arms/arms_before_799.rs" => Some(include_str!(concat!(env!("CARGO_MANIFEST_DIR"),"/src/source_verify/iterative/match_arms/arms_before_799.rs"))),
        "src/source_verify/recovery.rs" => Some(include_str!(concat!(env!("CARGO_MANIFEST_DIR"),"/src/source_verify/recovery.rs"))),
        "src/string_ops.rs" => Some(include_str!(concat!(env!("CARGO_MANIFEST_DIR"),"/src/string_ops.rs"))),
        "src/string_ops/concat_funded.rs" => Some(include_str!(concat!(env!("CARGO_MANIFEST_DIR"),"/src/string_ops/concat_funded.rs"))),
        "src/string_ops/conditions.rs" => Some(include_str!(concat!(env!("CARGO_MANIFEST_DIR"),"/src/string_ops/conditions.rs"))),
        "src/string_ops/conditions/funded.rs" => Some(include_str!(concat!(env!("CARGO_MANIFEST_DIR"),"/src/string_ops/conditions/funded.rs"))),
        "src/string_ops/replacement.rs" => Some(include_str!(concat!(env!("CARGO_MANIFEST_DIR"),"/src/string_ops/replacement.rs"))),
        "src/string_ops/replacement/funded.rs" => Some(include_str!(concat!(env!("CARGO_MANIFEST_DIR"),"/src/string_ops/replacement/funded.rs"))),
        "src/vec_ops/string_slot.rs" => Some(include_str!(concat!(env!("CARGO_MANIFEST_DIR"),"/src/vec_ops/string_slot.rs"))),
        "src/workspace_graph.rs" => Some(include_str!(concat!(env!("CARGO_MANIFEST_DIR"),"/src/workspace_graph.rs"))),
        "src/workspace_graph/expected_projection.rs" => Some(include_str!(concat!(env!("CARGO_MANIFEST_DIR"),"/src/workspace_graph/expected_projection.rs"))),
        "src/workspace_graph/expected_projection/borrowed_imports.rs" => Some(include_str!(concat!(env!("CARGO_MANIFEST_DIR"),"/src/workspace_graph/expected_projection/borrowed_imports.rs"))),
        "src/workspace_graph/expected_projection/borrowed_imports/inline.rs" => Some(include_str!(concat!(env!("CARGO_MANIFEST_DIR"),"/src/workspace_graph/expected_projection/borrowed_imports/inline.rs"))),
        "src/workspace_graph/expected_projection/borrowed_imports_original.rs" => Some(include_str!(concat!(env!("CARGO_MANIFEST_DIR"),"/src/workspace_graph/expected_projection/borrowed_imports_original.rs"))),
        "src/workspace_graph/expected_projection/borrowed_leaf.rs" => Some(include_str!(concat!(env!("CARGO_MANIFEST_DIR"),"/src/workspace_graph/expected_projection/borrowed_leaf.rs"))),
        "src/workspace_graph/expected_projection/defaults/inline_tests.rs" => Some(include_str!(concat!(env!("CARGO_MANIFEST_DIR"),"/src/workspace_graph/expected_projection/defaults/inline_tests.rs"))),
        "src/workspace_graph/expected_projection/held_projection.rs" => Some(include_str!(concat!(env!("CARGO_MANIFEST_DIR"),"/src/workspace_graph/expected_projection/held_projection.rs"))),
        "src/workspace_graph/expected_projection/import_stub/byte_helper.rs" => Some(include_str!(concat!(env!("CARGO_MANIFEST_DIR"),"/src/workspace_graph/expected_projection/import_stub/byte_helper.rs"))),
        "src/workspace_graph/expected_projection/import_stub/byte_helper_original.rs" => Some(include_str!(concat!(env!("CARGO_MANIFEST_DIR"),"/src/workspace_graph/expected_projection/import_stub/byte_helper_original.rs"))),
        "src/workspace_graph/expected_projection/import_stub/core_proofs/tests.rs" => Some(include_str!(concat!(env!("CARGO_MANIFEST_DIR"),"/src/workspace_graph/expected_projection/import_stub/core_proofs/tests.rs"))),
        "src/workspace_graph/identity_resolution.rs" => Some(include_str!(concat!(env!("CARGO_MANIFEST_DIR"),"/src/workspace_graph/identity_resolution.rs"))),
        "src/workspace_graph/identity_resolution/tests.rs" => Some(include_str!(concat!(env!("CARGO_MANIFEST_DIR"),"/src/workspace_graph/identity_resolution/tests.rs"))),
        "src/workspace_graph/source_schema/static_text/tests/retained_lexical_composition.rs" => Some(include_str!(concat!(env!("CARGO_MANIFEST_DIR"),"/src/workspace_graph/source_schema/static_text/tests/retained_lexical_composition.rs"))),
        "src/workspace_graph/static_text/scalar_helpers.rs" => Some(include_str!(concat!(env!("CARGO_MANIFEST_DIR"),"/src/workspace_graph/static_text/scalar_helpers.rs"))),
        "src/workspace_graph/static_text/scalar_helpers/capture_directory/tests.rs" => Some(include_str!(concat!(env!("CARGO_MANIFEST_DIR"),"/src/workspace_graph/static_text/scalar_helpers/capture_directory/tests.rs"))),
        "src/workspace_graph/tests.rs" => Some(include_str!(concat!(env!("CARGO_MANIFEST_DIR"),"/src/workspace_graph/tests.rs"))),
        "src/workspace_graph/tests/builder_limits.rs" => Some(include_str!(concat!(env!("CARGO_MANIFEST_DIR"),"/src/workspace_graph/tests/builder_limits.rs"))),
        "src/workspace_graph/tests/core_attempt_trace.rs" => Some(include_str!(concat!(env!("CARGO_MANIFEST_DIR"),"/src/workspace_graph/tests/core_attempt_trace.rs"))),
        "src/workspace_graph/type_proof_tests.rs" => Some(include_str!(concat!(env!("CARGO_MANIFEST_DIR"),"/src/workspace_graph/type_proof_tests.rs"))),
        "tests/hir_module_boundaries.rs" => Some(include_str!(concat!(env!("CARGO_MANIFEST_DIR"),"/tests/hir_module_boundaries.rs"))),
        "tests/project/standard_library/application_json/nested_response/string_vectors.rs" => Some(include_str!(concat!(env!("CARGO_MANIFEST_DIR"),"/tests/project/standard_library/application_json/nested_response/string_vectors.rs"))),
        "tests/useful_data/byte_slice_for_runtime/nested.rs" => Some(include_str!(concat!(env!("CARGO_MANIFEST_DIR"),"/tests/useful_data/byte_slice_for_runtime/nested.rs"))),
        "tests/wasm/scalar_browser_ci_contract.rs" => Some(include_str!(concat!(env!("CARGO_MANIFEST_DIR"),"/tests/wasm/scalar_browser_ci_contract.rs"))),
        "tests/wasm/scalar_browser_ci_contract/before_21abc.rs" => Some(include_str!(concat!(env!("CARGO_MANIFEST_DIR"),"/tests/wasm/scalar_browser_ci_contract/before_21abc.rs"))),
        "tests/wasm/scalar_browser_ci_contract/before_3e66.rs" => Some(include_str!(concat!(env!("CARGO_MANIFEST_DIR"),"/tests/wasm/scalar_browser_ci_contract/before_3e66.rs"))),
        "tests/wasm/scalar_browser_ci_contract/historical_builder.rs" => Some(include_str!(concat!(env!("CARGO_MANIFEST_DIR"),"/tests/wasm/scalar_browser_ci_contract/historical_builder.rs"))),
        "tests/workspace.rs" => Some(include_str!(concat!(env!("CARGO_MANIFEST_DIR"),"/tests/workspace.rs"))),
        "tests/workspace/semantic_change.rs" => Some(include_str!(concat!(env!("CARGO_MANIFEST_DIR"),"/tests/workspace/semantic_change.rs"))),
        "tests/workspace/semantic_change/reporting.rs" => Some(include_str!(concat!(env!("CARGO_MANIFEST_DIR"),"/tests/workspace/semantic_change/reporting.rs"))),
        "tests/workspace/semantic_graph.rs" => Some(include_str!(concat!(env!("CARGO_MANIFEST_DIR"),"/tests/workspace/semantic_graph.rs"))),
        "tests/workspace/semantic_graph/source_inventory.rs" => Some(include_str!(concat!(env!("CARGO_MANIFEST_DIR"),"/tests/workspace/semantic_graph/source_inventory.rs"))),
        "tests/workspace/semantic_graph/source_inventory_controls.rs" => Some(include_str!(concat!(env!("CARGO_MANIFEST_DIR"),"/tests/workspace/semantic_graph/source_inventory_controls.rs"))),
        "tests/workspace/semantic_operations.rs" => Some(include_str!(concat!(env!("CARGO_MANIFEST_DIR"),"/tests/workspace/semantic_operations.rs"))),
        "tests/workspace/semantic_structural_change.rs" => Some(include_str!(concat!(env!("CARGO_MANIFEST_DIR"),"/tests/workspace/semantic_structural_change.rs"))),
        "tests/workspace/semantic_structural_change/reporting.rs" => Some(include_str!(concat!(env!("CARGO_MANIFEST_DIR"),"/tests/workspace/semantic_structural_change/reporting.rs"))),
        "platform-tests/wasm-scalar-browser-v1/test-fixtures.mjs" => Some(include_str!(concat!(env!("CARGO_MANIFEST_DIR"),"/platform-tests/wasm-scalar-browser-v1/test-fixtures.mjs"))),
        _ => None,
    }
}
fn acquire(path: &Path) -> std::io::Result<(String,String)> {
    let root=Path::new(env!("CARGO_MANIFEST_DIR"));
    let relative=path.strip_prefix(root).expect("confined MAIN_E177 source");
    assert!(relative.components().all(|part|matches!(part,std::path::Component::Normal(_))));
    let relative=relative.to_str().unwrap();let packet=packet();let rows=packet["rows"].as_array().unwrap();
    assert_eq!(rows.iter().filter(|row|row["path"]==relative).count(),1);
    let row=rows.iter().find(|row|row["path"]==relative).unwrap();
    let read=|path:&Path,len,sha|main_e177_file_raw(path,len,sha).map_err(|message|std::io::Error::new(std::io::ErrorKind::InvalidData,message));
    let actual=read(path,row["current_length"].as_u64().unwrap()as usize,row["current_sha256"].as_str().unwrap())?;
    if let Some(compiled)=compiled(relative) {assert_eq!(actual,compiled,"whole current compiled MAIN_E177 input");}
    let incoming=read(&root.join(row["main_archive"].as_str().unwrap()),row["main_length"].as_u64().unwrap()as usize,row["main_sha256"].as_str().unwrap())?;
    Ok((actual,incoming))
}
pub(crate) fn read(path: impl AsRef<Path>) -> std::io::Result<String> {acquire(path.as_ref()).map(|(_,incoming)|incoming)}
pub(crate) fn expected(path: &str,supplied: &str) -> String {
    let (actual,incoming)=acquire(&Path::new(env!("CARGO_MANIFEST_DIR")).join(path)).unwrap();
    assert!(supplied==actual || supplied==incoming,"foreign MAIN_E177 supplied source");incoming
}
