//! Exact source-only custody domains for the composed MAIN receiver.
use sha2::{Digest, Sha256};
fn digest(text: &str) -> String { format!("{:x}", crate::digest_hex::LowerHex(Sha256::digest(text.as_bytes()))) }
fn records() -> serde_json::Value {
    let data = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/docs/evidence/main67107-fronts-6e45-20261017/stages.json"));
    assert!(data.len() <= 1_048_576);
    assert_eq!(digest(data), "bfbacf2777c310ee34ad6acd34352a9c58777ec11d39f2649be584711d72f120");
    let rows: serde_json::Value = serde_json::from_str(data).unwrap();
    assert_eq!(rows.as_array().unwrap().len(), 9);
    rows
}
fn restore(rows: &serde_json::Value, domain: &str, current: &str) -> String {
    let rows = rows.as_array().unwrap();
    assert_eq!(rows.iter().filter(|r| r["domain"] == domain).count(), 1);
    let row = rows.iter().find(|r| r["domain"] == domain).unwrap();
    let source = match domain {
        "canonical870" | "canonicalMAIN" => "src/cleanup_plan/replay/factored/canonical_shared.rs",
        "testsMAIN" => "src/cleanup_plan/replay/factored/canonical_shared/tests.rs",
        "ownedGrowth246fd230" => "src/workspace_graph/expected_projection/borrowed_imports/typed/alias/owned_growth_tests.rs",
        "followupReceiverMAIN" => "src/source_reflows/semantic_receivers.rs",
        "followupIdentityMAIN" => "src/workspace_graph/identity_resolution.rs",
        "followupCoreMAIN" => "src/workspace_graph/expected_projection/import_stub/core_proofs/tests.rs",
        "liveSessionMAIN" => "src/cleanup_plan/replay/selected_entry/tests/session_errors/source.rs",
        "liveNominalMAIN" => "src/hir/nominal_payload/legacy/source_tests.rs",
        _ => panic!("foreign MAIN source domain"),
    };
    assert_eq!(row["source"], source);
    let selected = selected_successor_expected_input(source, row["current_sha256"].as_str().unwrap(), current);
    let current = selected.as_str();
    let singleton = singleton_successor_expected_input(source, row["current_sha256"].as_str().unwrap(), current);
    let current = singleton.as_str();
    let stage = supplemental_receiver_expected_input(source, row["current_sha256"].as_str().unwrap(), current);
    let acquired = compile35f_expected_input(source, row["current_sha256"].as_str().unwrap(), &stage);
    let current = acquired.as_str();
    assert_eq!(digest(current), row["current_sha256"]);
    assert_eq!(current, row["current"].as_str().unwrap());
    let edits = row["inverse"].as_array().unwrap();
    assert!(!edits.is_empty() && edits.len() <= 512);
    let mut lines: Vec<&str> = current.split_inclusive('\n').collect();
    let mut last = lines.len();
    for edit in edits.iter().rev() {
        let a = edit["start"].as_u64().unwrap() as usize;
        let z = edit["end"].as_u64().unwrap() as usize;
        assert!(a <= z && z <= last && z <= lines.len());
        assert_eq!(lines[a..z].concat(), edit["current"].as_str().unwrap());
        lines.splice(a..z, edit["previous"].as_str().unwrap().split_inclusive('\n'));
        last = a;
    }
    let before = lines.concat();
    assert_eq!(digest(&before), row["previous_sha256"]);
    assert_eq!(before.len() as u64, row["previous_length"].as_u64().unwrap());
    before
}
pub(crate) fn project(domain: &str, current: &str) -> String { restore(&records(), domain, current) }
#[test]
fn complete_main_domains_and_both_unit_namespaces_are_preserved() {
    let rows = records();
    for row in rows.as_array().unwrap() {
        let domain = row["domain"].as_str().unwrap();
        let raw = std::fs::read_to_string(std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join(row["source"].as_str().unwrap())).unwrap();
        let old = restore(&rows, domain, &raw);
        let archived = std::fs::read_to_string(std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("docs/evidence/main67107-fronts-6e45-20261017").join(row["archive"].as_str().unwrap())).unwrap();
        assert_eq!(old, archived);
    }
    let root = include_str!("../cleanup_plan/replay/factored/canonical_shared.rs");
    let mut original = project("canonicalMAIN", root);
    let inverse: serde_json::Value = serde_json::from_str(include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/docs/evidence/main67107-production-composition-20261017/unit-main10.json"))).unwrap();
    assert_eq!(inverse.as_array().unwrap().len(), 10);
    for change in inverse.as_array().unwrap().iter().rev() {
        let a = change[0].as_u64().unwrap() as usize; let now = change[1].as_str().unwrap();
        assert_eq!(original.get(a..a + now.len()), Some(now));
        original.replace_range(a..a + now.len(), change[2].as_str().unwrap());
    }
    assert_eq!(original, include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/docs/evidence/unit-weight-closure-v1/canonical_shared.before.rs.txt")));
    let old: serde_json::Value = serde_json::from_str(include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/docs/evidence/main67107-production-composition-20261017/unit-ours8.json"))).unwrap();
    assert_eq!(old.as_array().unwrap().len(), 8);
    assert_eq!(&inverse.as_array().unwrap()[..8], old.as_array().unwrap().as_slice());
}
#[test]
fn foreign_truncated_duplicate_reordered_and_forged_main_domains_fail_closed() {
    let rows = records();
    for row in rows.as_array().unwrap() {
        let domain = row["domain"].as_str().unwrap(); let raw = row["current"].as_str().unwrap();
        for bad in [String::new(), format!("{raw} "), format!("{raw}{raw}"), raw[..raw.len()-1].to_owned(), "foreign".to_owned()] {
            assert!(std::panic::catch_unwind(|| restore(&rows, domain, &bad)).is_err());
        }
        let mut forged = rows.clone();
        let record = forged.as_array_mut().unwrap().iter_mut().find(|r| r["domain"] == domain).unwrap();
        record["previous_sha256"] = serde_json::Value::String("0".repeat(64));
        assert!(std::panic::catch_unwind(|| restore(&forged, domain, raw)).is_err());
        let mut duplicate = rows.clone(); duplicate.as_array_mut().unwrap().push(row.clone());
        assert!(std::panic::catch_unwind(|| restore(&duplicate, domain, raw)).is_err());
        let mut reordered = rows.clone();
        let record = reordered.as_array_mut().unwrap().iter_mut().find(|r| r["domain"] == domain).unwrap();
        if record["inverse"].as_array().unwrap().len() > 1 {
            record["inverse"].as_array_mut().unwrap().reverse();
            assert!(std::panic::catch_unwind(|| restore(&reordered, domain, raw)).is_err());
        }
    }
    assert!(std::panic::catch_unwind(|| project("foreign", "foreign")).is_err());
}


// Distinct from followupReceiverMAIN: the old043 self-source proof expects
// stage457d, not frozen MAIN68c9. Only historical source equality uses this.
fn main043_receiver_restore(row: &serde_json::Value, source: &str, current: &str) -> String {
    assert_eq!(row.as_object().unwrap().len(), 7);
    assert_eq!(source, "src/source_reflows/semantic_receivers.rs");
    assert_eq!(row["source"], source);
    let acquired = supplemental_receiver_expected_input(source, row["current_sha256"].as_str().unwrap(), current);
    let current = acquired.as_str();
    assert!(current.len() <= 65_536);
    assert_eq!(digest(current), row["current_sha256"]);
    assert_eq!(current, row["current"].as_str().unwrap());
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("docs/evidence/main043-semantic-acquisition-20261017");
    assert_eq!(current, std::fs::read_to_string(root.join("current.rs.txt")).unwrap());
    let previous = std::fs::read_to_string(root.join("prior-stage.rs.txt")).unwrap();
    assert_eq!(digest(&previous), row["previous_sha256"]);
    let edits = row["inverse"].as_array().unwrap();
    assert!(!edits.is_empty() && edits.len() <= 32);
    let mut lines: Vec<&str> = current.split_inclusive('\n').collect();
    let mut last = lines.len();
    for edit in edits.iter().rev() {
        assert_eq!(edit.as_object().unwrap().len(), 4);
        let a = usize::try_from(edit["start"].as_u64().unwrap()).unwrap();
        let z = usize::try_from(edit["end"].as_u64().unwrap()).unwrap();
        assert!(a <= z && z <= last && z <= lines.len());
        assert_eq!(lines[a..z].concat(), edit["current"].as_str().unwrap());
        lines.splice(a..z, edit["previous"].as_str().unwrap().split_inclusive('\n'));
        last = a;
    }
    let restored = lines.concat();
    assert_eq!(digest(&restored), row["previous_sha256"]);
    assert_eq!(restored, row["previous"].as_str().unwrap());
    assert_eq!(restored, previous);
    assert_eq!(row["previous_length"].as_u64().unwrap(), previous.len() as u64);
    restored
}
fn main043_receiver_record() -> serde_json::Value {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let raw = std::fs::read_to_string(root.join("docs/evidence/main043-semantic-acquisition-20261017/current-to-043-stage.json")).unwrap();
    assert!(raw.len() <= 131_072);
    assert_eq!(digest(&raw), "63bd9f8a4f8d19369a78798a41278e5b96ab0732361598656884efd6af159ff9");
    let mut row: serde_json::Value = serde_json::from_str(&raw).unwrap();
    assert_eq!(row.as_object().unwrap().len(), 6, "complete original immutable record");
    let previous = row["previous"].as_str().unwrap();
    assert_eq!(digest(previous), row["previous_sha256"]);
    let length = previous.len() as u64;
    row["previous_length"] = serde_json::Value::from(length);
    row
}
#[test]
fn main043_receiver_complete_stage_and_original_historical_inverse_are_retained() {
    let source = "src/source_reflows/semantic_receivers.rs";
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let raw = std::fs::read_to_string(root.join(source)).unwrap();
    let row = main043_receiver_record();
    let current = main043_receiver_restore(&row, source, &raw);
    let packet = root.join("docs/evidence/main043-independent-custody-20261016/semantic-receiver-historical-view");
    let old = std::fs::read_to_string(packet.join("current-to-main043.json")).unwrap();
    assert_eq!(digest(&old), "1aa0092ae0b2503a0fc398efe2dd3454f9940d052f6c3e9cf5861c57e9d05b5d");
    let old: serde_json::Value = serde_json::from_str(&old).unwrap();
    // Original whole-stage assertions and line inverse, independently checked.
    let historical = |old: &serde_json::Value| {
    assert_eq!(old["source"], source);
    assert_eq!(digest(&current), old["current_sha256"]);
    assert_eq!(current, old["current"].as_str().unwrap());
    assert!(current.len() <= 65_536);
    let edits = old["inverse"].as_array().unwrap();
    assert!(edits.len() <= 32);
    let mut lines: Vec<&str> = current.split_inclusive('\n').collect();
    let mut last = lines.len();
    for edit in edits.iter().rev() {
        let a = usize::try_from(edit["start"].as_u64().unwrap()).unwrap();
        let z = usize::try_from(edit["end"].as_u64().unwrap()).unwrap();
        assert!(a <= z && z <= last && z <= lines.len());
        assert_eq!(lines[a..z].concat(), edit["current"].as_str().unwrap());
        lines.splice(a..z, edit["previous"].as_str().unwrap().split_inclusive('\n'));
        last = a;
    }
    let restored = lines.concat();
    assert_eq!(digest(&restored), old["historical_sha256"]);
    restored
    };
    let restored = historical(&old);
    let mut forged_old = old.clone(); forged_old["historical_sha256"] = serde_json::Value::String("0".repeat(64));
    assert!(std::panic::catch_unwind(|| historical(&forged_old)).is_err());
    assert_eq!(digest(&restored), "651d174d04fad97da5c08ad215a869fd395a7d691d4a78dc901a99e814fc8252");
    assert_eq!(restored, std::fs::read_to_string(packet.join("main043-semantic_receivers.rs.txt")).unwrap());
    for bad in [String::new(), format!("{raw} "), format!("{raw}{raw}"), current, "foreign".to_owned()] {
        assert!(std::panic::catch_unwind(|| main043_receiver_restore(&row, source, &bad)).is_err());
    }
    assert!(std::panic::catch_unwind(|| main043_receiver_restore(&row, "foreign", &raw)).is_err());
    let mut forged = row.clone(); forged["previous_sha256"] = serde_json::Value::String("0".repeat(64));
    assert!(std::panic::catch_unwind(|| main043_receiver_restore(&forged, source, &raw)).is_err());
    let mut altered = row.clone(); altered["inverse"][0]["previous"] = serde_json::Value::String("foreign".to_owned());
    assert!(std::panic::catch_unwind(|| main043_receiver_restore(&altered, source, &raw)).is_err());
}

// Exact compile-repair acquisition. Historical domains retain their original bytes.
const COMPILE35F_KEYS: &[&str] = &[
    "src/cleanup_plan/replay/factored/canonical_shared/before_paid_many.rs",
    "src/cleanup_plan/replay/factored/canonical_shared/tests.rs",
    "src/cleanup_plan/replay/sequence_prefix.rs",
    "src/cleanup_plan/replay/sequence_vector/funded_tests.rs",
    "src/cleanup_plan/replay/skeleton_work/prefix_fusion.rs",
    "src/workspace_graph/expected_projection/borrowed_imports/typed/alias/owned_growth_tests.rs",
    "src/workspace_graph/expected_projection/borrowed_imports/typed/alias/rename_tests.rs",
];
fn compile35f_records() -> serde_json::Value {
    let data = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/docs/evidence/compile-acquisition-35f-20261017/sources.json"));
    assert!(data.len() <= 1_048_576);
    assert_eq!(digest(data), "61af497922931df374510a7d8acd4b8691481f960407f1670c4d4e4dc37625fe");
    let rows: serde_json::Value = serde_json::from_str(data).unwrap();
    let array = rows.as_array().unwrap();
    assert_eq!(array.len(), COMPILE35F_KEYS.len());
    assert_eq!(array.iter().map(|r| r["source"].as_str().unwrap()).collect::<Vec<_>>(), COMPILE35F_KEYS);
    rows
}
fn compile35f_restore(row: &serde_json::Value, current: &str) -> String {
    let source = row["source"].as_str().unwrap();
    assert!(COMPILE35F_KEYS.binary_search(&source).is_ok());
    let repaired = compile_repair_previous(source, current);
    let current = repaired.as_str();
    let owned = owned_compile35f_input(source, current);
    let current = owned.as_str();
    // A preselected35f view still acquires/authenticates physical current below.
    // Do not project it again through the distinct new-to-c874 fixture stage.
    let fixture = if digest(current) == row["before_sha256"].as_str().unwrap() {
        current.to_owned()
    } else { corrected_alias_fixture_input(source, current) };
    let current = fixture.as_str();
    let acquired = factored_core_input(source, current);
    let current = acquired.as_str();
    // A35f intermediate requires the actual physical new source to remain bound.
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let raw = if source == "src/cleanup_plan/replay/sequence_prefix.rs" {
        compile_repair_previous(source, include_str!("../cleanup_plan/replay/sequence_prefix.rs"))
    } else { factored_core_physical(source) };
    assert_eq!(digest(&raw), row["current_sha256"], "actual new compile subject");
    let before = std::fs::read_to_string(root.join(row["before_archive"].as_str().unwrap())).unwrap();
    assert_eq!(digest(&before), row["before_sha256"]);
    if digest(current) == row["before_sha256"].as_str().unwrap() {
        assert_eq!(current, before);
        return current.to_owned();
    }
    assert_eq!(digest(current), row["current_sha256"], "complete compile input");
    let mut lines: Vec<&str> = current.split_inclusive('\n').collect();
    let mut last = lines.len();
    for edit in row["inverse"].as_array().unwrap().iter().rev() {
        let a = edit["start"].as_u64().unwrap() as usize;
        let z = edit["end"].as_u64().unwrap() as usize;
        assert!(a <= z && z <= last && z <= lines.len());
        assert_eq!(lines[a..z].concat(), edit["current"].as_str().unwrap());
        lines.splice(a..z, edit["previous"].as_str().unwrap().split_inclusive('\n'));
        last = a;
    }
    let restored = lines.concat();
    assert_eq!(digest(&restored), row["before_sha256"]);
    assert_eq!(restored, before);
    restored
}
pub(crate) fn compile35f_input(source: &str, current: &str) -> String {
    if COMPILE35F_KEYS.binary_search(&source).is_err() { return current.to_owned(); }
    let rows = compile35f_records();
    let row = rows.as_array().unwrap().iter().find(|r| r["source"] == source).unwrap();
    compile35f_restore(row, current)
}
pub(super) fn compile35f_expected_input(source: &str, expected: &str, current: &str) -> String {
    let selected = selected_successor_expected_input(source, expected, current);
    let current = selected.as_str();
    let singleton = singleton_successor_expected_input(source, expected, current);
    let current = singleton.as_str();
    let ordinary = super::ordinary_plan_inputs::acquire_expected(source, expected, current);
    let current = ordinary.as_str();
    let accepted = accepted_expected_value(source, expected, current);
    let current = accepted.as_str();
    if COMPILE35F_KEYS.binary_search(&source).is_err() { return current.to_owned(); }
    let rows = compile35f_records();
    let row = rows.as_array().unwrap().iter().find(|r| r["source"] == source).unwrap();
    // Only a receiver explicitly expecting full35f owns this outer acquisition.
    if row["before_sha256"] != expected { return current.to_owned(); }
    compile35f_restore(row, current)
}
#[test]
fn complete_seven_compile_inputs_restore_35f_and_refuse_foreign_receivers() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let rows = compile35f_records();
    for row in rows.as_array().unwrap() {
        let source = row["source"].as_str().unwrap();
        let raw = std::fs::read_to_string(root.join(source)).unwrap();
        let before = std::fs::read_to_string(root.join(row["before_archive"].as_str().unwrap())).unwrap();
        assert_eq!(compile35f_input(source, &raw), before);
        assert_eq!(compile35f_expected_input(source, row["before_sha256"].as_str().unwrap(), &raw), before);
        assert_eq!(compile35f_input(source, &before), before);
        for bad in [String::new(), format!("{raw} "), format!("{raw}{raw}"), format!("!{}", &raw[1..])] {
            assert!(std::panic::catch_unwind(|| compile35f_input(source, &bad)).is_err());
        }
        for foreign in rows.as_array().unwrap().iter().filter(|r| r["source"] != source) {
            let other = std::fs::read_to_string(root.join(foreign["source"].as_str().unwrap())).unwrap();
            assert!(std::panic::catch_unwind(|| compile35f_input(source, &other)).is_err());
        }
    }
    assert_eq!(compile35f_input("foreign", "unchanged"), "unchanged");
}

// One closed outer acquisition for the original seven-source compile domain.
// Path-included workspace harnesses also compile this module: no private lib dependency.
const FACTORED_CORE_PATH: &str = "src/cleanup_plan/replay/factored/canonical_shared/tests.rs";
const FACTORED_CORE_CAP: usize = 2_097_152;
const FACTORED_CORE_RAW: &str = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/cleanup_plan/replay/factored/canonical_shared/tests.rs"));
const FACTORED_CORE_OURS: &str = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/docs/evidence/main-d285-factored-custody-20261017/ours-core.before.txt"));
fn factored_core_body(reader: &mut impl std::io::Read, length: usize, expected: &str) -> Result<String, &'static str> {
    if length > FACTORED_CORE_CAP { return Err("bound"); }
    let mut bytes = Vec::new();
    bytes.try_reserve_exact(length).map_err(|_| "allocation")?;
    bytes.resize(length, 0);
    reader.read_exact(&mut bytes).map_err(|_| "truncated")?;
    let mut extra = [0u8; 1];
    if reader.read(&mut extra).map_err(|_| "read")? != 0 { return Err("extra"); }
    if factored_core_digest(&bytes) != expected { return Err("digest"); }
    String::from_utf8(bytes).map_err(|_| "utf8")
}
fn factored_core_digest(bytes: &[u8]) -> String {
    format!("{:x}", crate::digest_hex::LowerHex(Sha256::digest(bytes)))
}
fn factored_core_same_file(a: &std::fs::Metadata, b: &std::fs::Metadata) -> bool {
    if !a.is_file() || !b.is_file() || a.len() != b.len() || a.modified().ok() != b.modified().ok() { return false; }
    #[cfg(unix)]
    { use std::os::unix::fs::MetadataExt; if a.dev() != b.dev() || a.ino() != b.ino() || a.ctime() != b.ctime() || a.ctime_nsec() != b.ctime_nsec() { return false; } }
    #[cfg(windows)]
    { use std::os::windows::fs::MetadataExt; if a.file_attributes() & 0x400 != 0 || b.file_attributes() & 0x400 != 0 || a.creation_time() != b.creation_time() || a.last_write_time() != b.last_write_time() { return false; } }
    true
}
fn factored_core_file_raw(path: &::std::path::Path, length: usize, expected: &str) -> Result<String, &'static str> {
    if length > FACTORED_CORE_CAP { return Err("bound"); }
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
    if !factored_core_same_file(&path_before, &metadata) || metadata.len() != u64::try_from(length).map_err(|_| "bound")? {
        return Err("length-or-nonregular");
    }
    // The one opened no-follow regular handle is checked before any buffer.
    let value = factored_core_body(&mut file, length, expected)?;
    let after = file.metadata().map_err(|_| "metadata")?;
    let path_after = std::fs::symlink_metadata(path).map_err(|_| "metadata")?;
    if path_after.file_type().is_symlink() || !factored_core_same_file(&metadata, &after) || !factored_core_same_file(&after, &path_after) { return Err("metadata-drift"); }
    Ok(value)
}
fn factored_core_file(path: &::std::path::Path, length: usize, expected: &str) -> Result<String, &'static str> {
    if let Some(value) = accepted_expected_file(path, length, expected) { return Ok(value); }
    factored_core_file_raw(path, length, expected)
}



fn factored_core_physical(source: &str) -> String {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    if source == OWNED_COMPILE35F_PATH { return owned_compile35f_input(source, OWNED_COMPILE35F_RAW); }
    if source != FACTORED_CORE_PATH {
        let raw=std::fs::read_to_string(root.join(source)).unwrap();
        return corrected_alias_fixture_input(source,&raw);
    }
    let actual = factored_core_file(&root.join(source), FACTORED_CORE_RAW.len(), &factored_core_digest(FACTORED_CORE_RAW.as_bytes())).unwrap();
    assert_eq!(actual, FACTORED_CORE_RAW);
    assert_eq!(digest(FACTORED_CORE_OURS), "c151768408bd95f3a56ae5eb48b013ac010c221d0b43df9a141ad47cd97ca953");
    let old = factored_core_file(&root.join("docs/evidence/main-d285-factored-custody-20261017/ours-core.before.txt"), FACTORED_CORE_OURS.len(), &digest(FACTORED_CORE_OURS)).unwrap();
    assert_eq!(old, FACTORED_CORE_OURS);
    old
}
fn factored_core_input(source: &str, current: &str) -> String {
    if source != FACTORED_CORE_PATH { return current.to_owned(); }
    let old = factored_core_physical(source);
    if current == FACTORED_CORE_RAW { old } else { current.to_owned() }
}
#[test]
fn factored_core_relocation_preserves_complete_seven_source_domain_and_refuses_drift() {
    use std::io::Cursor;
    let old = factored_core_physical(FACTORED_CORE_PATH);
    assert_eq!(old, FACTORED_CORE_OURS);
    let before = compile35f_input(FACTORED_CORE_PATH, FACTORED_CORE_RAW);
    assert_eq!(before, compile35f_input(FACTORED_CORE_PATH, &old));
    assert_eq!(compile35f_input(FACTORED_CORE_PATH, &before), before);
    let rows = compile35f_records();
    let row = rows.as_array().unwrap().iter().find(|r| r["source"] == FACTORED_CORE_PATH).unwrap();
    assert_eq!(compile35f_expected_input(FACTORED_CORE_PATH, row["before_sha256"].as_str().unwrap(), FACTORED_CORE_RAW), before);
    for bad in [String::new(), format!("{FACTORED_CORE_RAW} "), format!("!{}", &FACTORED_CORE_RAW[1..]), format!("{FACTORED_CORE_RAW}{FACTORED_CORE_RAW}"), "foreign".to_owned()] {
        assert!(std::panic::catch_unwind(|| compile35f_input(FACTORED_CORE_PATH, &bad)).is_err());
    }
    assert_eq!(compile35f_input("foreign", "unchanged"), "unchanged");
    assert!(factored_core_body(&mut Cursor::new(b""), FACTORED_CORE_CAP+1, &digest("")).is_err());
    assert!(factored_core_body(&mut Cursor::new(b"wrong"), 5, &digest(FACTORED_CORE_RAW)).is_err());
}


// A new ordinary production receiver has its own closed current acquisition.
// Immutable canonical870/MAIN rows continue to name their full c874 inputs.
fn singleton_successor_expected_input(source: &str, expected: &str, current: &str) -> String {
    if source != "src/cleanup_plan/replay/factored/canonical_shared.rs" { return current.to_owned(); }
    let data = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/docs/evidence/ordinary-singleton-successors-20261017/source-acquisition.json"));
    assert!(data.len() <= 262_144);
    assert_eq!(digest(data), "e5a1c1e0946aab3a63896abe4fbc9d35b81be51ab76d5d50ad55af4649b35dad");
    let rows: serde_json::Value = serde_json::from_str(data).unwrap();
    let row = &rows.as_array().unwrap()[0];
    assert_eq!(rows.as_array().unwrap().len(), 2);
    assert_eq!(row["source"],source);
    // Do not change an older, already selected historical namespace.
    if row["before_sha256"] != expected { return current.to_owned(); }
    let selected = selected_successor_expected_input(source, expected, current);
    let current = selected.as_str();
    let compiled = include_str!("../cleanup_plan/replay/factored/canonical_shared.rs");
    let selected_physical = selected_successor_expected_input(source, row["current_sha256"].as_str().unwrap(), compiled);
    let physical = selected_physical.as_str();
    assert_eq!(digest(physical),row["current_sha256"]);
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let before = factored_core_file(&root.join(row["archive"].as_str().unwrap()), 63023, row["before_sha256"].as_str().unwrap()).unwrap();
    assert_eq!(digest(&before),row["before_sha256"]);
    assert!(current == physical || current == before,"foreign singleton successor stage");
    let mut restored = physical.to_owned();
    for edit in row["inverse"].as_array().unwrap() {
        let after=edit["after"].as_str().unwrap();assert_eq!(restored.matches(after).count(),1);
        restored=restored.replacen(after,edit["before"].as_str().unwrap(),1);
    }
    assert_eq!(restored,before);before
}


#[test]
fn singleton_successor_whole_c874_acquisition_is_scoped_and_refuses_foreign_stages() {
    let source="src/cleanup_plan/replay/factored/canonical_shared.rs";
    let raw=include_str!("../cleanup_plan/replay/factored/canonical_shared.rs");
    let before=include_str!(concat!(env!("CARGO_MANIFEST_DIR"),"/docs/evidence/ordinary-singleton-successors-20261017/before/src/cleanup_plan/replay/factored/canonical_shared.rs"));
    let expected=digest(before);
    assert_eq!(singleton_successor_expected_input(source,&expected,raw),before);
    assert_eq!(singleton_successor_expected_input(source,&expected,before),before);
    for bad in ["foreign".to_owned(),format!("{raw}\n"),raw[..raw.len()-1].to_owned()] {
        assert!(std::panic::catch_unwind(||singleton_successor_expected_input(source,&expected,&bad)).is_err());
    }
    assert_eq!(singleton_successor_expected_input(source,"older historical domain","older staged input"),"older staged input");
    assert_eq!(singleton_successor_expected_input("foreign",&expected,"unchanged"),"unchanged");
}


fn corrected_alias_fixture_input(source: &str, current: &str) -> String {
    let physical = match source {
        "src/workspace_graph/expected_projection/borrowed_imports/typed/alias/rename_tests.rs" => include_str!("../workspace_graph/expected_projection/borrowed_imports/typed/alias/rename_tests.rs"),
        "src/hir/resolve_program/tests/reservation_stages.rs" => include_str!("../hir/resolve_program/tests/reservation_stages.rs"),
        _ => return current.to_owned(),
    };
    let data=include_str!(concat!(env!("CARGO_MANIFEST_DIR"),"/docs/evidence/alias-fixtures-c874-20261017/acquisition.json"));
    assert!(data.len()<=262_144);assert_eq!(digest(data),"97332f8c15c3e824634ac248f307bd755ed69319773d10c363b99df4e0e12bae");
    let rows:serde_json::Value=serde_json::from_str(data).unwrap();assert_eq!(rows.as_array().unwrap().len(),2);
    let row=rows.as_array().unwrap().iter().find(|r|r["source"]==source).unwrap();
    assert_eq!(digest(physical),row["current_sha256"]);
    let root=std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let actual=factored_core_file(&root.join(source),physical.len(),&digest(physical)).unwrap();assert_eq!(actual,physical);
    let before_len=usize::try_from(row["before_len"].as_u64().unwrap()).unwrap();
    assert!(before_len<=262_144);
    let before=factored_core_file(&root.join(row["archive"].as_str().unwrap()),before_len,row["before_sha256"].as_str().unwrap()).unwrap();
    assert_eq!(digest(&before),row["before_sha256"]);
    assert!(current==physical || current==before,"foreign corrected fixture stage");
    let mut restored=physical.to_owned();
    for edit in row["inverse"].as_array().unwrap() {
        let after=edit["after"].as_str().unwrap();assert_eq!(restored.matches(after).count(),1);
        restored=restored.replacen(after,edit["before"].as_str().unwrap(),1);
    }
    assert_eq!(restored,before);before
}

#[test]
fn corrected_alias_fixtures_keep_complete_c874_sources_and_refuse_foreign_stages() {
    for (source,raw) in [
        ("src/workspace_graph/expected_projection/borrowed_imports/typed/alias/rename_tests.rs",include_str!("../workspace_graph/expected_projection/borrowed_imports/typed/alias/rename_tests.rs")),
        ("src/hir/resolve_program/tests/reservation_stages.rs",include_str!("../hir/resolve_program/tests/reservation_stages.rs")),
    ] {
        let before=corrected_alias_fixture_input(source,raw);
        assert_ne!(before,raw);assert_eq!(corrected_alias_fixture_input(source,&before),before);
        if COMPILE35F_KEYS.binary_search(&source).is_ok() {
            let rows=compile35f_records();
            let row=rows.as_array().unwrap().iter().find(|r|r["source"]==source).unwrap();
            let root=std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
            let old=factored_core_file(&root.join(row["before_archive"].as_str().unwrap()),38678,row["before_sha256"].as_str().unwrap()).unwrap();
            assert_eq!(compile35f_input(source,raw),old);
            assert_eq!(compile35f_input(source,&before),old);
            assert_eq!(compile35f_input(source,&old),old);
        }
        for wrong in ["foreign".to_owned(),format!("{raw}\n"),raw[..raw.len()-1].to_owned()] {
            assert!(std::panic::catch_unwind(||corrected_alias_fixture_input(source,&wrong)).is_err());
        }
    }
    assert_eq!(corrected_alias_fixture_input("foreign","unchanged"),"unchanged");
}
// Separate actual endpoint before immutable followup/043 d196 source inputs.
const SUPPLEMENT_RECEIVER_PREFIX: &str = "docs/evidence/c874-supplement-custody-domains-20261017";
fn supplemental_receiver_expected_input(source: &str, expected: &str, current: &str) -> String {
    if source != "src/source_reflows/semantic_receivers.rs" || expected != "d19607ffcbca7e4d676a774eb2f21d81293430f6c5985a7afd6409d9bb7589df" {
        return current.to_owned();
    }
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let text = factored_core_file(&root.join(SUPPLEMENT_RECEIVER_PREFIX).join("receiver-input.json"), SUPPLEMENT_RECEIVER_LENGTH, SUPPLEMENT_RECEIVER_SHA).unwrap();
    let row: serde_json::Value = serde_json::from_str(&text).unwrap();
    assert_eq!(row["source"], source);
    assert_eq!(row["stage_sha256"], expected);
    let length = usize::try_from(row["current_length"].as_u64().unwrap()).unwrap();
    assert!(length <= 65_536);
    let sha = row["current_sha256"].as_str().unwrap();
    let raw = factored_core_file(&root.join(source), length, sha).unwrap();
    let held = factored_core_file(&root.join(SUPPLEMENT_RECEIVER_PREFIX).join(row["current_archive"].as_str().unwrap()), length, sha).unwrap();
    assert_eq!(raw, held, "complete independently sealed actual receiver");
    assert_eq!(raw, include_str!("../source_reflows/semantic_receivers.rs"), "complete actual compiled receiver");
    let stage_len = usize::try_from(row["stage_length"].as_u64().unwrap()).unwrap();
    assert!(stage_len <= 65_536);
    let stage = factored_core_file(&root.join(SUPPLEMENT_RECEIVER_PREFIX).join(row["stage_archive"].as_str().unwrap()), stage_len, expected).unwrap();
    assert!(current == raw || current == stage, "foreign receiver input stage");
    stage
}

#[test]
fn complete_supplemental_receiver_stages_and_original_six_field_record_remain_bound() {
    let source = "src/source_reflows/semantic_receivers.rs";
    let raw = include_str!("../source_reflows/semantic_receivers.rs");
    let expected = "d19607ffcbca7e4d676a774eb2f21d81293430f6c5985a7afd6409d9bb7589df";
    let stage = supplemental_receiver_expected_input(source, expected, raw);
    assert_eq!(digest(&stage), expected);
    assert_eq!(supplemental_receiver_expected_input(source, expected, &stage), stage);
    assert_eq!(project("followupReceiverMAIN", raw), project("followupReceiverMAIN", &stage));
    let record = main043_receiver_record();
    assert_eq!(record.as_object().unwrap().len(), 7);
    assert_eq!(record["previous_length"].as_u64().unwrap(), record["previous"].as_str().unwrap().len() as u64);
    assert_eq!(main043_receiver_restore(&record, source, raw), record["previous"].as_str().unwrap());
    let mut forged = record.clone(); forged["previous_length"] = serde_json::Value::from(0);
    assert!(std::panic::catch_unwind(|| main043_receiver_restore(&forged, source, raw)).is_err());
    for bad in [String::new(), raw[..raw.len()-1].to_owned(), format!("{raw} "), format!("{raw}{raw}"), record["previous"].as_str().unwrap().to_owned()] {
        assert!(std::panic::catch_unwind(|| supplemental_receiver_expected_input(source, expected, &bad)).is_err());
    }
    assert_eq!(supplemental_receiver_expected_input("foreign", expected, "unchanged"), "unchanged");
}

// Selected876 is a separate complete generation. Only these exact predecessor
// namespaces acquire it; every existing singleton and arena inverse stays fixed.
fn selected_successor_expected_input(source: &str, expected: &str, current: &str) -> String {
    let root_source = "src/cleanup_plan/replay/factored/canonical_shared.rs";
    let arena_source = "src/cleanup_plan/replay/factored/canonical_shared/arena_tests.rs";
    let admitted = match source {
        "src/cleanup_plan/replay/factored/canonical_shared.rs" => matches!(expected,
            "bf9c7cf18aafd539daaac00db9253eff6287a562d76e41dfc91c56bd5bd0337b" |
            "bbe5d1ff75b6b3bc811d181275181994027cd83188d6096e5b7716efbec753d3"),
        "src/cleanup_plan/replay/factored/canonical_shared/arena_tests.rs" =>
            expected == "891edcee186f1c7b3fb59bf7d2adf2443c9d230343569865d941b90410b04dd8",
        _ => false,
    };
    if !admitted { return current.to_owned(); }
    let packet = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/docs/evidence/selected-successor-generation-20261017/full-source-inverses.json"));
    assert_eq!(packet.len(), 40_083);
    assert_eq!(digest(packet), "947c41698347ca27788aae5985e63f19c85d3f7eb3f25fee4374dac9d0d743d1");
    let rows: serde_json::Value = serde_json::from_str(packet).unwrap();
    let rows = rows.as_array().unwrap();
    assert_eq!(rows.len(), 2);
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let compiled = [
        include_str!("../cleanup_plan/replay/factored/canonical_shared.rs"),
        include_str!("../cleanup_plan/replay/factored/canonical_shared/arena_tests.rs"),
    ];
    let mut selected = None;
    for ((row, path), compiled) in rows.iter().zip([root_source, arena_source]).zip(compiled) {
        assert_eq!(row["path"], path);
        let length = usize::try_from(row["current_length"].as_u64().unwrap()).unwrap();
        let before_length = usize::try_from(row["before_length"].as_u64().unwrap()).unwrap();
        assert!(length <= 131_072 && before_length <= 131_072);
        let raw_compiled = compiled;
        let compiled_stage = canonical_import_input(path, compiled);
        let compiled = compiled_stage.as_str();
        let physical = compiled_stage.clone();
        assert_eq!(physical.len(), length);
        assert_eq!(digest(&physical), row["current_sha256"]);
        assert_eq!(physical, compiled, "complete compiled and physical selected source");
        let before = factored_core_file(&root.join(row["archive"].as_str().unwrap()), before_length, row["before_sha256"].as_str().unwrap()).unwrap();
        let mut restored = physical.clone();
        let edits = row["inverse"].as_array().unwrap();
        assert!(!edits.is_empty() && edits.len() <= 2);
        for edit in edits {
            let after = edit["after"].as_str().unwrap();
            assert_eq!(restored.matches(after).count(), 1);
            restored = restored.replacen(after, edit["before"].as_str().unwrap(), 1);
        }
        assert_eq!(restored, before, "complete selected predecessor");
        if path == source {
            let current = if current == raw_compiled { physical.as_str() } else { current };
            if source == root_source && expected == "bbe5d1ff75b6b3bc811d181275181994027cd83188d6096e5b7716efbec753d3" && digest(current) == expected {
                // The independently authenticated older singleton namespace is
                // already at its requested stage; do not project it a second time.
                let historical = factored_core_file(&root.join("docs/evidence/ordinary-singleton-successors-20261017/before/src/cleanup_plan/replay/factored/canonical_shared.rs"), 63_023, expected).unwrap();
                assert_eq!(current, historical);
                selected = Some(historical);
            } else {
                assert!(current == physical || current == before, "foreign selected successor stage");
                selected = Some(before);
            }
        }
    }
    selected.expect("one exact selected predecessor namespace")
}

#[test]
fn selected_successor_complete_root_and_arena_acquisition_refuses_foreign_stages() {
    for (source, expected, raw, before) in [
        ("src/cleanup_plan/replay/factored/canonical_shared.rs",
         "bf9c7cf18aafd539daaac00db9253eff6287a562d76e41dfc91c56bd5bd0337b",
         include_str!("../cleanup_plan/replay/factored/canonical_shared.rs"),
         include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/docs/evidence/selected-successor-generation-20261017/before/src/cleanup_plan/replay/factored/canonical_shared.rs"))),
        ("src/cleanup_plan/replay/factored/canonical_shared/arena_tests.rs",
         "891edcee186f1c7b3fb59bf7d2adf2443c9d230343569865d941b90410b04dd8",
         include_str!("../cleanup_plan/replay/factored/canonical_shared/arena_tests.rs"),
         include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/docs/evidence/selected-successor-generation-20261017/before/src/cleanup_plan/replay/factored/canonical_shared/arena_tests.rs"))),
    ] {
        assert_eq!(selected_successor_expected_input(source, expected, raw), before);
        assert_eq!(selected_successor_expected_input(source, expected, before), before);
        for bad in [String::new(), "foreign".to_owned(), format!("{raw} "), raw[..raw.len()-1].to_owned(), format!("{before} ")] {
            assert!(std::panic::catch_unwind(|| selected_successor_expected_input(source, expected, &bad)).is_err());
        }
        assert_eq!(selected_successor_expected_input(source, "older historical namespace", "older bytes"), "older bytes");
        assert_eq!(compile35f_expected_input(source, expected, raw), before);
    }
    assert_eq!(selected_successor_expected_input("foreign", "older historical namespace", "unchanged"), "unchanged");
}

const SUPPLEMENT_RECEIVER_LENGTH: usize = 607;
const SUPPLEMENT_RECEIVER_SHA: &str = "1c120e26c852966dba70f8d67b5869465bedd655aa4eb4612986453a2f8f1035";


const OWNED_COMPILE35F_PATH: &str = "src/workspace_graph/expected_projection/borrowed_imports/typed/alias/owned_growth_tests.rs";
const OWNED_COMPILE35F_RAW: &str = include_str!("../workspace_graph/expected_projection/borrowed_imports/typed/alias/owned_growth_tests.rs");
fn owned_compile35f_input(source: &str, current: &str) -> String {
    if source != OWNED_COMPILE35F_PATH { return current.to_owned(); }
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let data = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/docs/evidence/compile35f-owned-growth-ingress-20261017/seal.json"));
    assert!(data.len() <= 65_536);
    assert_eq!(digest(data), "1481bc58a11f7e39d53cea0790414c98a6be1ccaa6509e990ac1e64084d095c8");
    let seal: serde_json::Value = serde_json::from_str(data).unwrap();
    assert_eq!(seal["schema"], "semaprax.compile35f-owned-growth-ingress.v1");
    assert_eq!(seal["source"], source);
    let rows = compile35f_records();
    let row = rows.as_array().unwrap().iter().find(|r| r["source"] == source).unwrap();
    assert_eq!(row["current_sha256"], seal["previous_sha256"]);
    assert_eq!(row["before_sha256"], seal["original35f_sha256"]);
    assert_eq!(row["before_archive"], seal["original35f_archive"]);
    let read = |archive: &str, length: &str, expected: &str| {
        let length = usize::try_from(seal[length].as_u64().unwrap()).unwrap();
        assert!(length <= 262_144);
        factored_core_file(&root.join(archive), length, seal[expected].as_str().unwrap()).unwrap()
    };
    let actual = read(source, "current_length", "current_sha256");
    assert_eq!(actual, OWNED_COMPILE35F_RAW);
    assert_eq!(actual, read(seal["current_archive"].as_str().unwrap(), "current_length", "current_sha256"));
    let previous = read(seal["previous_archive"].as_str().unwrap(), "previous_length", "previous_sha256");
    let original = read(seal["original35f_archive"].as_str().unwrap(), "original35f_length", "original35f_sha256");
    let mut restored = actual.as_bytes().to_vec();
    let mut last = restored.len();
    for edit in seal["inverse"].as_array().unwrap() {
        let at = usize::try_from(edit["at"].as_u64().unwrap()).unwrap();
        let after = edit["after"].as_str().unwrap().as_bytes();
        let end = at.checked_add(after.len()).unwrap();
        assert!(at <= end && end <= last);
        assert_eq!(&restored[at..end], after);
        restored.splice(at..end, edit["before"].as_str().unwrap().bytes());
        last = at;
    }
    assert_eq!(restored, previous.as_bytes());
    if current == original { return original; }
    assert!(current == actual || current == previous, "foreign whole owned compile input");
    previous
}

#[test]
fn owned_compile35f_current_acquisition_preserves_complete_seven_subjects_and_refuses_foreign() {
    let source = OWNED_COMPILE35F_PATH;
    let previous = owned_compile35f_input(source, OWNED_COMPILE35F_RAW);
    assert_eq!(digest(&previous), "fa95c61e8eb3ba3c300c4fee92e3da1777ad360483309c6f0ac9c2c5cf46e956");
    assert_eq!(owned_compile35f_input(source, &previous), previous);
    let rows = compile35f_records();
    let row = rows.as_array().unwrap().iter().find(|r| r["source"] == source).unwrap();
    let before = compile35f_input(source, OWNED_COMPILE35F_RAW);
    assert_eq!(digest(&before), row["before_sha256"]);
    assert_eq!(compile35f_input(source, &previous), before);
    assert_eq!(compile35f_input(source, &before), before);
    for bad in [String::new(), "foreign".to_owned(), format!("{previous} "), OWNED_COMPILE35F_RAW[..OWNED_COMPILE35F_RAW.len()-1].to_owned()] {
        assert!(std::panic::catch_unwind(|| owned_compile35f_input(source, &bad)).is_err());
    }
    assert_eq!(owned_compile35f_input("foreign", "unchanged"), "unchanged");
}


// Import-only compile generation before the immutable Selected947 source rows.
pub(crate) fn canonical_import_input(source: &str, current: &str) -> String {
    let compiled = match source {
        "src/cleanup_plan/replay/factored/canonical_shared.rs" => include_str!("../cleanup_plan/replay/factored/canonical_shared.rs"),
        "src/cleanup_plan/replay/factored/canonical_shared/arena_tests.rs" => include_str!("../cleanup_plan/replay/factored/canonical_shared/arena_tests.rs"),
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
    let data = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/docs/evidence/canonical-import-acquisition-7827-20261017/sources.json"));
    assert!(data.len() <= 65_536);
    assert_eq!(digest(data), "c0ec361ae90245676b4804403a4fe7bf65a34df06347109b086dfd0762deb7e0");
    let rows: serde_json::Value = serde_json::from_str(data).unwrap();
    assert_eq!(rows.as_array().unwrap().len(), 2);
    let row = rows.as_array().unwrap().iter().find(|r| r["source"] == source).unwrap();
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let read = |archive: &str, length: &str, hash: &str| {
        let length = usize::try_from(row[length].as_u64().unwrap()).unwrap();
        assert!(length <= 131_072);
        if archive == source && source == "src/cleanup_plan/replay/factored/canonical_shared.rs" {
            return paid_many_canonical_previous(source, include_str!("../cleanup_plan/replay/factored/canonical_shared.rs"));
        }
        semantic_batch_file(&root.join(archive), length, row[hash].as_str().unwrap()).unwrap()
    };
    let actual = read(source, "current_length", "current_sha256");
    assert_eq!(actual, compiled);
    assert_eq!(actual, read(row["current_archive"].as_str().unwrap(), "current_length", "current_sha256"));
    let stage = read(row["stage_archive"].as_str().unwrap(), "stage_length", "stage_sha256");
    let mut restored = actual.clone();
    for edit in row["inverse"].as_array().unwrap() {
        let after = edit["after"].as_str().unwrap();
        assert_eq!(restored.matches(after).count(), 1);
        restored = restored.replacen(after, edit["before"].as_str().unwrap(), 1);
    }
    assert_eq!(restored, stage);
    assert!(current == actual || current == stage, "foreign canonical import generation");
    stage
}

#[test]
fn canonical_import_generation_authenticates_current_and_complete_selected_stages() {
    for (source, raw) in [
        ("src/cleanup_plan/replay/factored/canonical_shared.rs", include_str!("../cleanup_plan/replay/factored/canonical_shared.rs")),
        ("src/cleanup_plan/replay/factored/canonical_shared/arena_tests.rs", include_str!("../cleanup_plan/replay/factored/canonical_shared/arena_tests.rs")),
    ] {
        let stage = canonical_import_input(source, raw);
        assert_ne!(stage, raw);
        assert_eq!(canonical_import_input(source, &stage), stage);
        for bad in [String::new(), "foreign".to_owned(), format!("{raw} "), raw[..raw.len()-1].to_owned()] {
            assert!(std::panic::catch_unwind(|| canonical_import_input(source, &bad)).is_err());
        }
    }
    assert_eq!(canonical_import_input("foreign", "unchanged"), "unchanged");
}

// Complete PaidMany source -> frozen compile-import input, a separate namespace.
fn paid_many_canonical_previous(source: &str, current: &str) -> String {
    if source != "src/cleanup_plan/replay/factored/canonical_shared.rs" { return current.to_owned(); }
    let batch = semantic_batch_previous(source, current, "base851");
    let current = batch.as_str();
    let data = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/docs/evidence/paid-many-canonical-ingress-20261017/seal.json"));
    assert_eq!(data.len(), 2923);
    assert_eq!(digest(data), "890336922097f28cddddbcfe85a3c267bf148acd9419289b198eb081fc8060be");
    let seal: serde_json::Value = serde_json::from_str(data).unwrap();
    assert_eq!(seal["schema"], "semaprax.paid-many-canonical-ingress.v1");
    assert_eq!(seal["source"], source);
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let read = |path: &str, kind: &str| {
        let length = seal[format!("{kind}_length")].as_u64().unwrap() as usize;
        assert!(length <= 131_072);
        semantic_batch_file(&root.join(path), length, seal[format!("{kind}_sha256")].as_str().unwrap()).unwrap()
    };
    let actual = read(source, "current");
    assert_eq!(actual, semantic_batch_previous(source, include_str!("../cleanup_plan/replay/factored/canonical_shared.rs"), "base851"));
    assert_eq!(actual, read(seal["current_archive"].as_str().unwrap(), "current"));
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
    if current == actual || current == previous { previous } else { current.to_owned() }
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
    for bad in [String::new(), "foreign".to_owned(), format!("{raw} "), raw[..raw.len()-1].to_owned(), format!("{previous} ")] {
        assert!(std::panic::catch_unwind(|| canonical_import_input(source, &bad)).is_err());
    }
    assert_eq!(paid_many_canonical_previous("foreign", "unchanged"), "unchanged");
}

// Exact assembled physical inputs precede immutable historical source domains.
// This test-only source transport grants no production or raw-audit authority.
fn semantic_batch_records() -> serde_json::Value {
    let data = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/docs/evidence/next-semantic-source-views-20261017/inputs.json"));
    assert_eq!(data.len(), 84888);
    assert_eq!(digest(data), "c876c6b5f15969fe55bee55b31be8b6174db300d6b4f9f18cd412044273b6305");
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
    let Some(row) = rows.iter().find(|r| r["source"] == source) else { return current.to_owned(); };
    let views = row["views"].as_array().unwrap();
    assert!(views.iter().filter(|r| r["name"] == view).count() <= 1);
    let Some(prior) = views.iter().find(|r| r["name"] == view) else { return current.to_owned(); };
    let root = ::std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let read = |path: &str, length: &serde_json::Value, sha: &serde_json::Value| {
        compile_repair_file(&root.join(path), usize::try_from(length.as_u64().unwrap()).unwrap(), sha.as_str().unwrap()).unwrap()
    };
    let actual = read(source, &row["current_length"], &row["current_sha256"]);
    assert_eq!(actual, read(row["current_archive"].as_str().unwrap(), &row["current_length"], &row["current_sha256"]));
    let compiled = match source {
        "src/cleanup_plan/replay.rs" => include_str!("../cleanup_plan/replay.rs"),
        "src/cleanup_plan/replay/factored/canonical_shared.rs" => include_str!("../cleanup_plan/replay/factored/canonical_shared.rs"),
        "src/cleanup_plan/replay/factored/canonical_shared/arena_tests.rs" => include_str!("../cleanup_plan/replay/factored/canonical_shared/arena_tests.rs"),
        "src/cleanup_plan/replay/factored/canonical_shared/borrowed_labels.rs" => include_str!("../cleanup_plan/replay/factored/canonical_shared/borrowed_labels.rs"),
        "src/cleanup_plan/replay/factored/canonical_shared/terminal_leaf.rs" => include_str!("../cleanup_plan/replay/factored/canonical_shared/terminal_leaf.rs"),
        "src/cleanup_plan/replay/ordinary_plan_history.rs" => include_str!("../cleanup_plan/replay/ordinary_plan_history.rs"),
        "src/cleanup_plan/replay/ordinary_plan_history/owned_tests.rs" => include_str!("../cleanup_plan/replay/ordinary_plan_history/owned_tests.rs"),
        "src/cleanup_plan/replay/physical_budget/setup.rs" => include_str!("../cleanup_plan/replay/physical_budget/setup.rs"),
        "src/cleanup_plan/replay/physical_storage.rs" => include_str!("../cleanup_plan/replay/physical_storage.rs"),
        "src/cleanup_plan/replay/skeleton_work.rs" => include_str!("../cleanup_plan/replay/skeleton_work.rs"),
        "src/cleanup_plan/replay/skeleton_work/ordinary_dag.rs" => include_str!("../cleanup_plan/replay/skeleton_work/ordinary_dag.rs"),
        "src/cleanup_plan/replay/skeleton_work/ordinary_dag/frontier.rs" => include_str!("../cleanup_plan/replay/skeleton_work/ordinary_dag/frontier.rs"),
        "src/cleanup_plan/replay/skeleton_work/prefix_fusion.rs" => include_str!("../cleanup_plan/replay/skeleton_work/prefix_fusion.rs"),
        "src/source_verify/declaration.rs" => include_str!("../source_verify/declaration.rs"),
        "src/source_verify/declaration/record_shape.rs" => include_str!("../source_verify/declaration/record_shape.rs"),
        _ => unreachable!("sealed finite semantic source"),
    };
    let compiled = compile_repair_previous(source, compiled);
    assert_eq!(actual, compiled, "complete physical/compiled semantic source");
    let previous = read(prior["archive"].as_str().unwrap(), &prior["length"], &prior["sha256"]);
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
    if current == actual || current == previous { previous } else { current.to_owned() }
}
pub(crate) fn semantic_batch_file(path: &::std::path::Path, length: usize, expected: &str) -> Result<String, &'static str> {
    let root = ::std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let packet = semantic_batch_records();
    if let Ok(relative) = path.strip_prefix(root) {
        if let Some(source) = relative.to_str() {
            if let Some(row) = packet["rows"].as_array().unwrap().iter().find(|r| r["source"] == source) {
                if let Some(prior) = row["views"].as_array().unwrap().iter().find(|r|
                    r["sha256"] == expected && r["length"].as_u64() == Some(length as u64)) {
                    let actual = compile_repair_file(path, usize::try_from(row["current_length"].as_u64().unwrap()).map_err(|_| "semantic-length")?, row["current_sha256"].as_str().ok_or("semantic-sha")?)?;
                    return Ok(semantic_batch_previous(source, &actual, prior["name"].as_str().ok_or("semantic-view")?));
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
        let raw = compile_repair_file(&root.join(source), row["current_length"].as_u64().unwrap() as usize, row["current_sha256"].as_str().unwrap()).unwrap();
        for prior in row["views"].as_array().unwrap() {
            let name = prior["name"].as_str().unwrap();
            let old = semantic_batch_previous(source, &raw, name);
            assert_eq!(digest(&old), prior["sha256"]);
            assert_eq!(semantic_batch_previous(source, &old, name), old);
            for bad in [String::new(), format!("{raw} "), raw[..raw.len()-1].to_owned(), format!("{raw}{raw}")] {
                assert_ne!(semantic_batch_previous(source, &bad, name), old);
            }
        }
    }
    assert_eq!(semantic_batch_previous("foreign", "unchanged", "base851"), "unchanged");
    // Existing owning gate also authenticates every exact compilation ingress.
    let repaired = compile_repair_records();
    for row in repaired["rows"].as_array().unwrap() {
        let source = row["source"].as_str().unwrap();
        let raw = factored_core_file(&root.join(source), row["current_length"].as_u64().unwrap() as usize, row["current_sha256"].as_str().unwrap()).unwrap();
        let old = compile_repair_previous(source, &raw);
        assert_eq!(digest(&old), row["previous_sha256"]);
        assert_eq!(old.len() as u64, row["previous_length"].as_u64().unwrap());
        assert_eq!(compile_repair_previous(source, &old), old);
        assert_eq!(compile_repair_file(&root.join(source), old.len(), row["previous_sha256"].as_str().unwrap()).unwrap(), old);
        for bad in [String::new(), format!("{raw} "), raw[..raw.len()-1].to_owned(), format!("{raw}{raw}")] {
            assert_ne!(compile_repair_previous(source, &bad), old);
        }
    }
    assert_eq!(compile_repair_previous("foreign", "unchanged"), "unchanged");
}

// Exact complete compilation repair ingress, before every frozen semantic view.
// This is test-source custody only; no allocator, payer or runtime authority.
fn compile_repair_records() -> serde_json::Value {
    let data = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/docs/evidence/semantic162-compile-source-ingress-20261017/inputs.json"));
    assert_eq!(data.len(), 13993);
    assert_eq!(digest(data), "e3870d5c067c099197b21f5b749b3124eaa4174701fe3eb47ca73db88ac44f14");
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
    let Some(row) = rows.iter().find(|r| r["source"] == source) else { return current.to_owned(); };
    let root = ::std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let read = |path: &str, length: &serde_json::Value, sha: &serde_json::Value| {
        factored_core_file(&root.join(path), usize::try_from(length.as_u64().unwrap()).unwrap(), sha.as_str().unwrap()).unwrap()
    };
    let actual = read(source, &row["current_length"], &row["current_sha256"]);
    assert_eq!(actual, read(row["current_archive"].as_str().unwrap(), &row["current_length"], &row["current_sha256"]));
    let compiled = match source {
        "src/cleanup_plan/replay/factored/canonical_shared/borrowed_labels.rs" => include_str!("../cleanup_plan/replay/factored/canonical_shared/borrowed_labels.rs"),
        "src/cleanup_plan/replay/skeleton_work/merge_keys_tests.rs" => include_str!("../cleanup_plan/replay/skeleton_work/merge_keys_tests.rs"),
        "src/cleanup_plan/replay/physical_storage/zero_tests.rs" => include_str!("../cleanup_plan/replay/physical_storage/zero_tests.rs"),
        "src/cleanup_plan/replay/owned_suffix.rs" => include_str!("../cleanup_plan/replay/owned_suffix.rs"),
        "src/cleanup_plan/replay/skeleton_work/ordinary_dag.rs" => include_str!("../cleanup_plan/replay/skeleton_work/ordinary_dag.rs"),
        "src/cleanup_plan/replay/skeleton_work.rs" => include_str!("../cleanup_plan/replay/skeleton_work.rs"),
        "src/cleanup_plan/replay/owned_suffix/tests.rs" => include_str!("../cleanup_plan/replay/owned_suffix/tests.rs"),
        "src/cleanup_plan/replay/skeleton_work/enumerated_prefix/tests.rs" => include_str!("../cleanup_plan/replay/skeleton_work/enumerated_prefix/tests.rs"),
        "src/cleanup_plan/replay/sequence_prefix.rs" => include_str!("../cleanup_plan/replay/sequence_prefix.rs"),
        "src/cleanup_plan/replay/owned_suffix/sequence_vector.rs" => include_str!("../cleanup_plan/replay/owned_suffix/sequence_vector.rs"),
        "src/cleanup_plan/replay/lexical_cert/paid/chains_tests.rs" => include_str!("../cleanup_plan/replay/lexical_cert/paid/chains_tests.rs"),
        "src/cleanup_plan/replay/sequence_prefix/main_c11/wrapper_context.rs" => include_str!("../cleanup_plan/replay/sequence_prefix/main_c11/wrapper_context.rs"),
        "src/cleanup_plan/replay/sequence_prefix/main8a/wrapper_context.rs" => include_str!("../cleanup_plan/replay/sequence_prefix/main8a/wrapper_context.rs"),
        "src/cleanup_plan/replay/sequence_vector/main_a3/path_array_source.rs" => include_str!("../cleanup_plan/replay/sequence_vector/main_a3/path_array_source.rs"),
        "src/workspace_graph/source_schema/nominal_context/tests.rs" => include_str!("../workspace_graph/source_schema/nominal_context/tests.rs"),
        "src/cleanup_plan/replay/inventory_coverage/typed_tests.rs" => include_str!("../cleanup_plan/replay/inventory_coverage/typed_tests.rs"),
        _ => unreachable!("sealed sixteen compilation subjects"),
    };
    let compiled = accepted_base_previous(source, compiled);
    assert_eq!(actual, compiled, "whole physical/compiled compilation repair");
    let previous = read(row["previous_archive"].as_str().unwrap(), &row["previous_length"], &row["previous_sha256"]);
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
    assert_eq!(restored, previous, "complete independently retained precompile source");
    if current == actual || current == previous { previous } else { current.to_owned() }
}
fn compile_repair_file(path: &::std::path::Path, length: usize, expected: &str) -> Result<String, &'static str> {
    let root = ::std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let packet = compile_repair_records();
    if let Ok(relative) = path.strip_prefix(root) {
        if let Some(source) = relative.to_str() {
            if let Some(row) = packet["rows"].as_array().unwrap().iter().find(|r| r["source"] == source
                && r["previous_sha256"] == expected && r["previous_length"].as_u64() == Some(length as u64)) {
                let actual = factored_core_file(path, usize::try_from(row["current_length"].as_u64().unwrap()).map_err(|_| "compile-length")?, row["current_sha256"].as_str().ok_or("compile-sha")?)?;
                return Ok(compile_repair_previous(source, &actual));
            }
        }
    }
    factored_core_file(path, length, expected)
}

// Whole-source finite admission in front of the independently frozen prior chains.
fn accepted_records() -> serde_json::Value {
    let previous = accepted_records_before_earned_pins();
    let data = include_str!("../../docs/evidence/earned-managed-pin42-source-coupling-20261017/inputs.json");
    assert_eq!(data.len(), 34641);
    assert_eq!(digest(data), "27ec261cbc4f2b00a6ff0be7084f8c1a8c76e42523043b180f49701f8540abd2");
    let packet: serde_json::Value = serde_json::from_str(data).unwrap();
    assert_eq!(packet["rows"].as_array().unwrap().len(), 42);
    let mut inverse = packet.clone();
    let added = inverse["rows"].as_array_mut().unwrap().pop().unwrap();
    assert_eq!(added["source"], "src/workspace_graph/type_proof_tests.rs");
    let row = inverse["rows"].as_array_mut().unwrap().iter_mut().find(|row| row["source"] == "tests/workspace/semantic_graph.rs").unwrap();
    let mut prior = row["priors"].as_array_mut().unwrap().pop().unwrap();
    assert_eq!(prior["name"], "earned_pin_producer");
    prior.as_object_mut().unwrap().remove("name");
    row["current"] = prior;
    assert_eq!(inverse, previous, "complete original41 source table changed");
    packet
}
fn accepted_records_before_earned_pins() -> serde_json::Value {
    let data = include_str!("../../docs/evidence/compile3-second-borrow-bindings-20261017/inputs.json");
    assert_eq!(data.len(), 33632);
    assert_eq!(digest(data), "9a4c35c3f31965ee34408cc88584f2e81c61d1a6c1e77d21248e1baad66cb270");
    let packet: serde_json::Value = serde_json::from_str(data).unwrap();
    assert_eq!(packet["schema"], "semaprax.accepted-whole-source-ingress.v1");
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
    factored_core_file_raw(&root.join(row["archive"].as_str().unwrap()), row["length"].as_u64().unwrap() as usize, row["sha256"].as_str().unwrap()).unwrap()
}
fn accepted_actual(row: &serde_json::Value) -> String {
    let source = row["source"].as_str().unwrap();
    let current = &row["current"];
    let root = ::std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let actual = factored_core_file_raw(&root.join(source), current["length"].as_u64().unwrap() as usize, current["sha256"].as_str().unwrap()).unwrap();
    assert_eq!(actual, accepted_compiled(source), "complete compiled actual source before historical selection");
    assert_eq!(actual, accepted_read(current), "complete current archive");
    actual
}
fn accepted_select(row: &serde_json::Value, prior: &serde_json::Value, supplied: &str) -> String {
    let actual = accepted_actual(row);
    let previous = accepted_read(prior);
    // Whole inverse: the complete authenticated current is replaced by the complete
    // independently retained prior. Foreign supplied bytes never acquire a view.
    if supplied == actual || supplied == previous { previous } else { supplied.to_owned() }
}
fn accepted_base_previous(source: &str, supplied: &str) -> String {
    let packet = accepted_records();
    let Some(row) = packet["rows"].as_array().unwrap().iter().find(|row| row["source"] == source) else { return supplied.to_owned(); };
    let Some(prior) = row["priors"].as_array().unwrap().iter().find(|prior| prior["name"] == "base5f") else { return supplied.to_owned(); };
    accepted_select(row, prior, supplied)
}
pub(crate) fn accepted_expected_value(source: &str, expected: &str, supplied: &str) -> String {
    let packet = accepted_records();
    let Some(row) = packet["rows"].as_array().unwrap().iter().find(|row| row["source"] == source) else { return supplied.to_owned(); };
    let Some(prior) = row["priors"].as_array().unwrap().iter().find(|prior| prior["sha256"] == expected) else { return supplied.to_owned(); };
    accepted_select(row, prior, supplied)
}
pub(crate) fn accepted_expected_file(path: &::std::path::Path, length: usize, expected: &str) -> Option<String> {
    let root = ::std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let source = path.strip_prefix(root).ok()?.to_str()?;
    let packet = accepted_records();
    let row = packet["rows"].as_array().unwrap().iter().find(|row| row["source"] == source)?;
    let prior = row["priors"].as_array().unwrap().iter().find(|prior| prior["sha256"] == expected && prior["length"].as_u64() == Some(length as u64))?;
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
            for bad in [String::new(), format!("{actual} "), actual[..actual.len()-1].to_owned(), format!("{actual}{actual}"), "foreign".to_owned()] {
                assert_eq!(accepted_select(row, prior, &bad), bad);
            }
            let length = prior["length"].as_u64().unwrap() as usize;
            let expected = prior["sha256"].as_str().unwrap();
            assert_eq!(accepted_expected_file(&root.join(source), length, expected), Some(previous));
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
    let data = include_str!("../../docs/evidence/future831-f247-final-source-binding-20261017/current-inputs.json");
    assert_eq!(data.len(), 46861);
    assert_eq!(digest(data), "41cdb5d63b4b405b85f71b563326f6eb500971c7a59a64420acb226c7c448b69");
    let packet: serde_json::Value = serde_json::from_str(data).unwrap();
    assert_eq!(packet["schema"], "semaprax.accepted-whole-source-ingress.v1");
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
    factored_core_file_raw(&root.join(row["archive"].as_str().unwrap()), row["length"].as_u64().unwrap() as usize, row["sha256"].as_str().unwrap()).unwrap()
}
fn accepted_actual(row: &serde_json::Value) -> String {
    let source = row["source"].as_str().unwrap();
    let current = &row["current"];
    let root = ::std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let actual = factored_core_file_raw(&root.join(source), current["length"].as_u64().unwrap() as usize, current["sha256"].as_str().unwrap()).unwrap();
    assert_eq!(actual, accepted_compiled(source), "complete compiled actual source before historical selection");
    assert_eq!(actual, accepted_read(current), "complete current archive");
    actual
}
fn accepted_select(row: &serde_json::Value, prior: &serde_json::Value, supplied: &str) -> String {
    let actual = accepted_actual(row);
    let previous = accepted_read(prior);
    // Whole inverse: the complete authenticated current is replaced by the complete
    // independently retained prior. Foreign supplied bytes never acquire a view.
    if supplied == actual || supplied == previous { previous } else { supplied.to_owned() }
}
fn accepted_base_previous(source: &str, supplied: &str) -> String {
    let packet = accepted_records();
    let Some(row) = packet["rows"].as_array().unwrap().iter().find(|row| row["source"] == source) else { return supplied.to_owned(); };
    let Some(prior) = row["priors"].as_array().unwrap().iter().find(|prior| prior["name"] == "base5f") else { return supplied.to_owned(); };
    accepted_select(row, prior, supplied)
}
pub(crate) fn accepted_expected_value(source: &str, expected: &str, supplied: &str) -> String {
    let packet = accepted_records();
    let Some(row) = packet["rows"].as_array().unwrap().iter().find(|row| row["source"] == source) else { return supplied.to_owned(); };
    let Some(prior) = row["priors"].as_array().unwrap().iter().find(|prior| prior["sha256"] == expected) else { return supplied.to_owned(); };
    accepted_select(row, prior, supplied)
}
pub(crate) fn accepted_expected_file(path: &::std::path::Path, length: usize, expected: &str) -> Option<String> {
    let root = ::std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let source = path.strip_prefix(root).ok()?.to_str()?;
    let packet = accepted_records();
    let row = packet["rows"].as_array().unwrap().iter().find(|row| row["source"] == source)?;
    let prior = row["priors"].as_array().unwrap().iter().find(|prior| prior["sha256"] == expected && prior["length"].as_u64() == Some(length as u64))?;
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
            for bad in [String::new(), format!("{actual} "), actual[..actual.len()-1].to_owned(), format!("{actual}{actual}"), "foreign".to_owned()] {
                assert_eq!(accepted_select(row, prior, &bad), bad);
            }
            let length = prior["length"].as_u64().unwrap() as usize;
            let expected = prior["sha256"].as_str().unwrap();
            assert_eq!(accepted_expected_file(&root.join(source), length, expected), Some(previous));
            assert!(accepted_expected_file(&root.join(source), length + 1, expected).is_none());
            assert!(accepted_expected_file(&root.join(source), length, &"0".repeat(64)).is_none());
        }
    }
    assert!(accepted_expected_file(&root.join("foreign"), 0, "foreign").is_none());
}

// Distinct exact incoming f247 cap; existing historical caps stay unchanged.
const MAIN_F247_CAP: usize = 9_703_917;
fn main_f247_body(reader: &mut impl std::io::Read, length: usize, expected: &str) -> Result<String, &'static str> {
    if length > MAIN_F247_CAP { return Err("bound"); }
    let mut bytes = Vec::new();
    bytes.try_reserve_exact(length).map_err(|_| "allocation")?;
    bytes.resize(length, 0);
    reader.read_exact(&mut bytes).map_err(|_| "truncated")?;
    let mut extra = [0u8; 1];
    if reader.read(&mut extra).map_err(|_| "read")? != 0 { return Err("extra"); }
    if main_f247_digest(&bytes) != expected { return Err("digest"); }
    String::from_utf8(bytes).map_err(|_| "utf8")
}
fn main_f247_digest(bytes: &[u8]) -> String {
    format!("{:x}", crate::digest_hex::LowerHex(Sha256::digest(bytes)))
}
fn main_f247_same_file(a: &std::fs::Metadata, b: &std::fs::Metadata) -> bool {
    if !a.is_file() || !b.is_file() || a.len() != b.len() || a.modified().ok() != b.modified().ok() { return false; }
    #[cfg(unix)]
    { use std::os::unix::fs::MetadataExt; if a.dev() != b.dev() || a.ino() != b.ino() || a.ctime() != b.ctime() || a.ctime_nsec() != b.ctime_nsec() { return false; } }
    #[cfg(windows)]
    { use std::os::windows::fs::MetadataExt; if a.file_attributes() & 0x400 != 0 || b.file_attributes() & 0x400 != 0 || a.creation_time() != b.creation_time() || a.last_write_time() != b.last_write_time() { return false; } }
    true
}
pub(crate) fn main_f247_file_raw(path: &::std::path::Path, length: usize, expected: &str) -> Result<String, &'static str> {
    if length > MAIN_F247_CAP { return Err("bound"); }
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
    if !main_f247_same_file(&path_before, &metadata) || metadata.len() != u64::try_from(length).map_err(|_| "bound")? {
        return Err("length-or-nonregular");
    }
    // The one opened no-follow regular handle is checked before any buffer.
    let value = main_f247_body(&mut file, length, expected)?;
    let after = file.metadata().map_err(|_| "metadata")?;
    let path_after = std::fs::symlink_metadata(path).map_err(|_| "metadata")?;
    if path_after.file_type().is_symlink() || !main_f247_same_file(&metadata, &after) || !main_f247_same_file(&after, &path_after) { return Err("metadata-drift"); }
    Ok(value)
}

// Current compiler text for the finite accepted-owned source domain only.
pub(crate) fn accepted_physical_compiled(source: &str) -> Option<&'static str> {
    let packet = accepted_records();
    if packet["rows"].as_array().unwrap().iter().any(|row| row["source"] == source) {
        Some(accepted_compiled(source))
    } else { None }
}

}
