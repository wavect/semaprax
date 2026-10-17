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
    let raw = factored_core_physical(source);
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
fn factored_core_file(path: &::std::path::Path, length: usize, expected: &str) -> Result<String, &'static str> {
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


fn factored_core_physical(source: &str) -> String {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
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
    let physical = include_str!("../cleanup_plan/replay/factored/canonical_shared.rs");
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
const SUPPLEMENT_RECEIVER_LENGTH: usize = 607;
const SUPPLEMENT_RECEIVER_SHA: &str = "4f73b6091d11a7bc7b9427234698fd4fea4f88e4c2e6c160947e1ac1ebaf7ac4";
