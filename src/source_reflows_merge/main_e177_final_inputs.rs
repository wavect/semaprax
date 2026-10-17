//! Closed final-batch historical acquisition. No production authority or raw-audit projection.
use sha2::{Digest, Sha256};
use std::path::Path;
const SOURCE_CAP: usize = 9753094;
fn digest(bytes: &[u8]) -> String {
    format!("{:x}", crate::digest_hex::LowerHex(Sha256::digest(bytes)))
}
fn packet() -> serde_json::Value {
    let data = include_str!("../../docs/evidence/main-e177-final-source-binding-20261017/current-inputs.json");
    assert_eq!(data.len(), 33454);
    assert_eq!(digest(data.as_bytes()), "c7ef5558171837e13132324c39d047d9dac2f0f210322fd0b104e2e843be3209");
    let root=Path::new(env!("CARGO_MANIFEST_DIR"));
    let physical=held(&root.join("docs/evidence/main-e177-final-source-binding-20261017/current-inputs.json"),data.len(),&digest(data.as_bytes())).unwrap();
    assert_eq!(physical,data,"complete physical and compiled current manifest");
    let packet: serde_json::Value = serde_json::from_str(data).unwrap();
    assert_eq!(packet["schema"], "semaprax.main-e177-final-whole-inputs.v1");
    assert_eq!(packet["rows"].as_array().unwrap().len(), 49);
    for anchor in packet["complete_old_table_anchors"].as_array().unwrap() {
        let path=anchor["path"].as_str().unwrap();
        let raw=held(&root.join(path),anchor["length"].as_u64().unwrap()as usize,anchor["sha256"].as_str().unwrap()).unwrap();
        assert_eq!(Some(raw.as_str()),table_compiled(path),"whole predecessor table physical and compiled bytes");
        assert_eq!(raw,read_view(anchor),"whole predecessor table archive");
    }
    packet
}
fn same_file(a: &std::fs::Metadata, b: &std::fs::Metadata) -> bool {
    if !a.is_file() || !b.is_file() || a.len()!=b.len() || a.modified().ok()!=b.modified().ok() { return false; }
    #[cfg(unix)] {
        use std::os::unix::fs::MetadataExt;
        if a.dev()!=b.dev() || a.ino()!=b.ino() || a.mode()!=b.mode() || a.mtime()!=b.mtime() || a.mtime_nsec()!=b.mtime_nsec() || a.ctime()!=b.ctime() || a.ctime_nsec()!=b.ctime_nsec() { return false; }
    }
    #[cfg(windows)] {
        use std::os::windows::fs::MetadataExt;
        if a.file_attributes()&0x400!=0 || b.file_attributes()&0x400!=0 || a.creation_time()!=b.creation_time() || a.last_write_time()!=b.last_write_time() { return false; }
    }
    true
}
fn held(path: &Path, length: usize, expected: &str) -> Result<String, &'static str> {
    use std::io::Read;
    if length>SOURCE_CAP { return Err("bound"); }
    let before=std::fs::symlink_metadata(path).map_err(|_|"missing")?;
    if before.file_type().is_symlink() || !before.is_file() { return Err("nonregular"); }
    let mut options=std::fs::OpenOptions::new(); options.read(true);
    #[cfg(unix)] { use std::os::unix::fs::OpenOptionsExt; options.custom_flags(libc::O_NOFOLLOW|libc::O_CLOEXEC|libc::O_NONBLOCK); }
    #[cfg(windows)] { use std::os::windows::fs::OpenOptionsExt; options.custom_flags(0x0020_0000); }
    #[cfg(not(any(unix,windows)))] { return Err("platform"); }
    let mut file=options.open(path).map_err(|_|"open")?;
    let opened=file.metadata().map_err(|_|"metadata")?;
    if !same_file(&before,&opened) || opened.len()!=u64::try_from(length).map_err(|_|"bound")? { return Err("length-or-identity"); }
    let mut bytes=Vec::new(); bytes.try_reserve_exact(length).map_err(|_|"allocation")?;
    let mut limited=(&mut file).take(length as u64+1);
    limited.read_to_end(&mut bytes).map_err(|_|"read")?;
    if bytes.len()!=length || digest(&bytes)!=expected { return Err("length-or-digest"); }
    let after=file.metadata().map_err(|_|"metadata")?;
    let path_after=std::fs::symlink_metadata(path).map_err(|_|"metadata")?;
    if path_after.file_type().is_symlink() || !same_file(&opened,&after) || !same_file(&after,&path_after) { return Err("identity-drift"); }
    String::from_utf8(bytes).map_err(|_|"utf8")
}
fn table_compiled(source: &str) -> Option<&'static str> {
    match source {
        "docs/evidence/next-big-batch-source-binding-20261017/current-inputs.json" => Some(include_str!("../../docs/evidence/next-big-batch-source-binding-20261017/current-inputs.json")),
        "docs/evidence/main99f-full-controller-acquisition-20261017/inputs.json" => Some(include_str!("../../docs/evidence/main99f-full-controller-acquisition-20261017/inputs.json")),
        _ => None,
    }
}
fn compiled(source: &str) -> Option<&'static str> {
    match source {
        "src/source_reflows_merge/main67107_inputs.rs" => Some(include_str!("../../src/source_reflows_merge/main67107_inputs.rs")),
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
        _ => None,
    }
}
fn read_view(view: &serde_json::Value) -> String {
    let root=Path::new(env!("CARGO_MANIFEST_DIR"));
    held(&root.join(view["archive"].as_str().unwrap()),view["length"].as_u64().unwrap()as usize,view["sha256"].as_str().unwrap()).unwrap()
}
fn actual(row: &serde_json::Value) -> String {
    let root=Path::new(env!("CARGO_MANIFEST_DIR"));let source=row["source"].as_str().unwrap();let current=&row["current"];
    let value=held(&root.join(source),current["length"].as_u64().unwrap()as usize,current["sha256"].as_str().unwrap()).unwrap();
    if row["compiled"].as_bool().unwrap() { assert_eq!(Some(value.as_str()),compiled(source),"complete actual compiled source"); }
    assert_eq!(value,read_view(current),"complete actual archive");value
}
pub(crate) fn file(path: &Path,length: usize,expected: &str) -> Option<String> {
    let root=Path::new(env!("CARGO_MANIFEST_DIR"));let relative=path.strip_prefix(root).ok()?;
    if !relative.components().all(|p|matches!(p,std::path::Component::Normal(_))) { return None; }
    let source=relative.to_str()?;let packet=packet();let row=packet["rows"].as_array().unwrap().iter().find(|r|r["source"]==source)?;
    let prior=row["views"].as_array().unwrap().iter().find(|v|v["length"].as_u64()==Some(length as u64)&&v["sha256"]==expected)?;
    actual(row);Some(read_view(prior))
}
pub(crate) fn value(source: &str,expected: &str,supplied: &str) -> String {
    let packet=packet();let Some(row)=packet["rows"].as_array().unwrap().iter().find(|r|r["source"]==source)else{return supplied.to_owned()};
    let Some(prior)=row["views"].as_array().unwrap().iter().find(|v|v["sha256"]==expected)else{return supplied.to_owned()};
    let current=actual(row);let previous=read_view(prior);
    if supplied==current || supplied==previous {previous}else{supplied.to_owned()}
}
#[test]
fn latest_main_and_afa_complete_views_preserve_all_prior_controls_and_refuse_wrong_stages() {
    let packet=packet();let root=Path::new(env!("CARGO_MANIFEST_DIR"));
    let mut seen=std::collections::BTreeSet::new();
    for row in packet["rows"].as_array().unwrap() {
        let source=row["source"].as_str().unwrap();assert!(seen.insert(source));let current=actual(row);
        for prior in row["views"].as_array().unwrap() {
            let expected=prior["sha256"].as_str().unwrap();let length=prior["length"].as_u64().unwrap()as usize;let previous=read_view(prior);
            assert_eq!(file(&root.join(source),length,expected),Some(previous.clone()));
            assert_eq!(value(source,expected,&current),previous);
            assert_eq!(value(source,expected,&previous),previous);
            for foreign in [String::new(),format!("{current} "),format!("{current}{current}"),current[..current.len()-1].to_owned(),"foreign".to_owned()] {
                assert_eq!(value(source,expected,&foreign),foreign);
            }
            assert!(file(&root.join(source),length+1,expected).is_none());
            assert!(file(&root.join(source),length,&"0".repeat(64)).is_none());
        }
    }
    assert!(file(&root.join("foreign"),0,"foreign").is_none());
    assert!(file(&root.join("src/../Cargo.toml"),0,"foreign").is_none());
}
