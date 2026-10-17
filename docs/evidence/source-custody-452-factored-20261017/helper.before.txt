//! Complete file custody assertions belong to the test source-reader layer.
mod receivers {
    use super::main_std as std;
    use sha2::{Digest, Sha256};
    macro_rules! include_str {
        ("tests.rs") => {
            super::MAIN_CORE
        };
    }
    pub(crate) fn complete_predecessors_and_new_target_sources_are_bound() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
        let evidence = root.join("docs/evidence/factored-flat-targets-v1");
        let proof: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(evidence.join("sources.json")).unwrap())
                .unwrap();
        let digest = |text: &str| {
            format!(
                "{:x}",
                crate::digest_hex::LowerHex(Sha256::digest(text.as_bytes()))
            )
        };
        for record in proof["predecessors"].as_array().unwrap() {
            let before =
                std::fs::read_to_string(evidence.join(record["archive"].as_str().unwrap()))
                    .unwrap();
            let current =
                std::fs::read_to_string(root.join(record["path"].as_str().unwrap())).unwrap();
            assert_eq!(digest(&before), record["before_sha256"]);
            assert_eq!(digest(&current), record["after_sha256"]);
            let restore = |input: &str| -> Option<String> {
                let mut output = input.to_owned();
                for edit in record["inverse"].as_array().unwrap() {
                    let after = edit["after"].as_str().unwrap();
                    if output.matches(after).count() != 1 {
                        return None;
                    }
                    output = output.replacen(after, edit["before"].as_str().unwrap(), 1);
                }
                Some(output)
            };
            assert_eq!(restore(&current).as_deref(), Some(before.as_str()));
            assert_ne!(
                restore(&(current.clone() + "\n")).as_deref(),
                Some(before.as_str())
            );
            let mut changed = current.clone();
            changed.replace_range(..1, "!");
            assert_ne!(restore(&changed).as_deref(), Some(before.as_str()));
        }
        for record in proof["new_files"].as_array().unwrap() {
            let current =
                std::fs::read_to_string(root.join(record["path"].as_str().unwrap())).unwrap();
            assert_eq!(digest(&current), record["sha256"]);
            assert_ne!(digest(&(current + "\n")), record["sha256"]);
        }
    }
    pub(crate) fn complete_outer_label_sources_and_original_producer_remain_exact() {
        use sha2::{Digest, Sha256};
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
        let evidence = root.join("docs/evidence/factored-flat-labels-v1");
        let proof: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(evidence.join("sources.json")).unwrap())
                .unwrap();
        let digest = |text: &str| {
            format!(
                "{:x}",
                crate::digest_hex::LowerHex(Sha256::digest(text.as_bytes()))
            )
        };
        assert_eq!(proof["predecessors"].as_array().unwrap().len(), 4);
        for record in proof["predecessors"].as_array().unwrap() {
            let before =
                std::fs::read_to_string(evidence.join(record["archive"].as_str().unwrap()))
                    .unwrap();
            let current =
                std::fs::read_to_string(root.join(record["path"].as_str().unwrap())).unwrap();
            assert_eq!(digest(&before), record["before_sha256"]);
            assert_eq!(digest(&current), record["after_sha256"]);
            let restore = |input: &str| -> Option<String> {
                let mut output = input.to_owned();
                for edit in record["inverse"].as_array().unwrap() {
                    let after = edit["after"].as_str().unwrap();
                    if output.matches(after).count() != 1 {
                        return None;
                    }
                    output = output.replacen(after, edit["before"].as_str().unwrap(), 1);
                }
                Some(output)
            };
            assert_eq!(restore(&current).as_deref(), Some(before.as_str()));
            assert_ne!(
                restore(&(current.clone() + "\n")).as_deref(),
                Some(before.as_str())
            );
            let mut changed = current.clone();
            changed.replace_range(..1, "!");
            assert_ne!(restore(&changed).as_deref(), Some(before.as_str()));
        }
        for record in proof["new_files"].as_array().unwrap() {
            let current =
                std::fs::read_to_string(root.join(record["path"].as_str().unwrap())).unwrap();
            assert_eq!(digest(&current), record["sha256"]);
            assert_ne!(digest(&(current + "\n")), record["sha256"]);
        }
        let before = std::fs::read_to_string(
            evidence.join("src__cleanup_plan__replay__factored__canonical_shared.rs.before.txt"),
        )
        .unwrap();
        let start = before
            .find("        let mut successors = BTreeMap::<Label, targets::Targets>::new();\n")
            .unwrap();
        let end = start
            + before[start..]
                .find("\n    }\n\n    pub(super) fn id(")
                .unwrap();
        let old_body = before[start..end].replace("self.automaton", "automaton");
        assert_eq!(include_str!("tests.rs").matches(&old_body).count(), 1);
    }
}
pub(crate) use receivers::{
    complete_outer_label_sources_and_original_producer_remain_exact,
    complete_predecessors_and_new_target_sources_are_bound,
};

#[test]
fn complete_factored_custody_assertions_remain_executable_and_byte_exact() {
    use sha2::{Digest, Sha256};
    receivers::complete_predecessors_and_new_target_sources_are_bound();
    let original =
        include_str!("../../docs/evidence/factored-flat-targets-v1/custody-function.before.txt");
    assert_eq!(
        format!(
            "{:x}",
            crate::digest_hex::LowerHex(Sha256::digest(original.as_bytes()))
        ),
        "5054407bbf53475c5d9af764cf2b94018609ef6f13ee4e07e7987ae9d79ec116"
    );
    let relocated = original.replacen("    fn ", "    pub(crate) fn ", 1);
    assert_eq!(
        include_str!("factored_sources.rs")
            .matches(&relocated)
            .count(),
        1
    );
    receivers::complete_outer_label_sources_and_original_producer_remain_exact();
    let original =
        include_str!("../../docs/evidence/factored-flat-labels-v1/custody-function.before.txt");
    assert_eq!(
        format!(
            "{:x}",
            crate::digest_hex::LowerHex(Sha256::digest(original.as_bytes()))
        ),
        "591d2884914ff7a1d43ad796f27f4fca6a96de19e82f373e7d1eeaaa4af8ea42"
    );
    let relocated = original.replacen("    fn ", "    pub(crate) fn ", 1);
    assert_eq!(
        include_str!("factored_sources.rs")
            .matches(&relocated)
            .count(),
        1
    );
}

// The OURS receivers keep every original assertion and old acquisition stage.
mod ours_receivers {
    use sha2::{Digest, Sha256};
    use super::ours_std as std;
    macro_rules! include_str {
        ("tests.rs") => { super::OURS_CORE };
    }
    pub(crate) fn complete_predecessors_and_new_target_sources_are_bound(incoming_input: fn(&str, &str) -> String) {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
        let evidence = root.join("docs/evidence/factored-flat-targets-v1");
        let proof: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(evidence.join("sources.json")).unwrap())
                .unwrap();
        let digest = |text: &str| {
            format!(
                "{:x}",
                crate::digest_hex::LowerHex(Sha256::digest(text.as_bytes()))
            )
        };
        for record in proof["predecessors"].as_array().unwrap() {
            let before =
                std::fs::read_to_string(evidence.join(record["archive"].as_str().unwrap()))
                    .unwrap();
            let current =
                std::fs::read_to_string(root.join(record["path"].as_str().unwrap())).unwrap();
            let current = crate::source_reflows_merge::main67107_inputs::compile35f_input(
                record["path"].as_str().unwrap(), &current,
            );
            let current = incoming_input(record["path"].as_str().unwrap(), &current);
            assert_eq!(digest(&before), record["before_sha256"]);
            assert_eq!(digest(&current), record["after_sha256"]);
            let restore = |input: &str| -> Option<String> {
                let mut output = input.to_owned();
                for edit in record["inverse"].as_array().unwrap() {
                    let after = edit["after"].as_str().unwrap();
                    if output.matches(after).count() != 1 {
                        return None;
                    }
                    output = output.replacen(after, edit["before"].as_str().unwrap(), 1);
                }
                Some(output)
            };
            assert_eq!(restore(&current).as_deref(), Some(before.as_str()));
            assert_ne!(
                restore(&(current.clone() + "\n")).as_deref(),
                Some(before.as_str())
            );
            let mut changed = current.clone();
            changed.replace_range(..1, "!");
            assert_ne!(restore(&changed).as_deref(), Some(before.as_str()));
        }
        for record in proof["new_files"].as_array().unwrap() {
            let current =
                std::fs::read_to_string(root.join(record["path"].as_str().unwrap())).unwrap();
            let current = crate::source_reflows_merge::main67107_inputs::compile35f_input(
                record["path"].as_str().unwrap(), &current,
            );
            assert_eq!(digest(&current), record["sha256"]);
            assert_ne!(digest(&(current + "\n")), record["sha256"]);
        }
    }
    pub(crate) fn complete_outer_label_sources_and_original_producer_remain_exact() {
        use sha2::{Digest, Sha256};
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
        let evidence = root.join("docs/evidence/factored-flat-labels-v1");
        let proof: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(evidence.join("sources.json")).unwrap())
                .unwrap();
        let digest = |text: &str| {
            format!(
                "{:x}",
                crate::digest_hex::LowerHex(Sha256::digest(text.as_bytes()))
            )
        };
        assert_eq!(proof["predecessors"].as_array().unwrap().len(), 4);
        for record in proof["predecessors"].as_array().unwrap() {
            let before =
                std::fs::read_to_string(evidence.join(record["archive"].as_str().unwrap()))
                    .unwrap();
            let current =
                std::fs::read_to_string(root.join(record["path"].as_str().unwrap())).unwrap();
            let current = crate::source_reflows_merge::main67107_inputs::compile35f_input(
                record["path"].as_str().unwrap(), &current,
            );
            assert_eq!(digest(&before), record["before_sha256"]);
            assert_eq!(digest(&current), record["after_sha256"]);
            let restore = |input: &str| -> Option<String> {
                let mut output = input.to_owned();
                for edit in record["inverse"].as_array().unwrap() {
                    let after = edit["after"].as_str().unwrap();
                    if output.matches(after).count() != 1 {
                        return None;
                    }
                    output = output.replacen(after, edit["before"].as_str().unwrap(), 1);
                }
                Some(output)
            };
            assert_eq!(restore(&current).as_deref(), Some(before.as_str()));
            assert_ne!(
                restore(&(current.clone() + "\n")).as_deref(),
                Some(before.as_str())
            );
            let mut changed = current.clone();
            changed.replace_range(..1, "!");
            assert_ne!(restore(&changed).as_deref(), Some(before.as_str()));
        }
        for record in proof["new_files"].as_array().unwrap() {
            let current =
                std::fs::read_to_string(root.join(record["path"].as_str().unwrap())).unwrap();
            let current = crate::source_reflows_merge::main67107_inputs::compile35f_input(
                record["path"].as_str().unwrap(), &current,
            );
            assert_eq!(digest(&current), record["sha256"]);
            assert_ne!(digest(&(current + "\n")), record["sha256"]);
        }
        let before = std::fs::read_to_string(
            evidence.join("src__cleanup_plan__replay__factored__canonical_shared.rs.before.txt"),
        )
        .unwrap();
        let start = before
            .find("        let mut successors = BTreeMap::<Label, targets::Targets>::new();\n")
            .unwrap();
        let end = start
            + before[start..]
                .find("\n    }\n\n    pub(super) fn id(")
                .unwrap();
        let old_body = before[start..end].replace("self.automaton", "automaton");
        assert_eq!(crate::source_reflows_merge::main67107_inputs::project("testsMAIN", include_str!("tests.rs")).matches(&old_body).count(), 1);
    }

}
pub(crate) fn joined_predecessors(incoming_input: fn(&str, &str) -> String) {
    ours_receivers::complete_predecessors_and_new_target_sources_are_bound(incoming_input);
    complete_predecessors_and_new_target_sources_are_bound();
}
pub(crate) fn joined_labels() {
    ours_receivers::complete_outer_label_sources_and_original_producer_remain_exact();
    complete_outer_label_sources_and_original_producer_remain_exact();
}

// Separate private transport: each immutable complete logical input is <= 2 MiB.
// This does not change any existing MAIN335 or MAIN34d reader domain/cap.
const CUSTODY_CAP: usize = 2_097_152;
const EVIDENCE: &str = "docs/evidence/main-d285-factored-custody-20261017";
const CORE_PATH: &str = "src/cleanup_plan/replay/factored/canonical_shared/tests.rs";
const SELF_PATH: &str = "src/source_reflows/factored_sources.rs";
const LIVE_CORE: &str = include_str!("../cleanup_plan/replay/factored/canonical_shared/tests.rs");
const LIVE_SELF: &str = include_str!("factored_sources.rs");
const SUBJECTS: &str = include_str!("../../docs/evidence/main-d285-factored-custody-20261017/subjects.json");
const OURS_CORE: &str = include_str!("../../docs/evidence/main-d285-factored-custody-20261017/ours-core.before.txt");
const MAIN_CORE: &str = include_str!("../../docs/evidence/main-d285-factored-custody-20261017/main-core.before.txt");
use sha2::{Digest, Sha256};
fn custody_body(reader: &mut impl std::io::Read, length: usize, expected: &str) -> Result<String, &'static str> {
    if length > CUSTODY_CAP { return Err("bound"); }
    let mut bytes = Vec::new();
    bytes.try_reserve_exact(length).map_err(|_| "allocation")?;
    bytes.resize(length, 0);
    reader.read_exact(&mut bytes).map_err(|_| "truncated")?;
    let mut extra = [0u8; 1];
    if reader.read(&mut extra).map_err(|_| "read")? != 0 { return Err("extra"); }
    if custody_digest(&bytes) != expected { return Err("digest"); }
    String::from_utf8(bytes).map_err(|_| "utf8")
}
fn custody_digest(bytes: &[u8]) -> String {
    format!("{:x}", crate::digest_hex::LowerHex(Sha256::digest(bytes)))
}
fn custody_same_file(a: &std::fs::Metadata, b: &std::fs::Metadata) -> bool {
    if !a.is_file() || !b.is_file() || a.len() != b.len() || a.modified().ok() != b.modified().ok() { return false; }
    #[cfg(unix)]
    { use std::os::unix::fs::MetadataExt; if a.dev() != b.dev() || a.ino() != b.ino() || a.ctime() != b.ctime() || a.ctime_nsec() != b.ctime_nsec() { return false; } }
    #[cfg(windows)]
    { use std::os::windows::fs::MetadataExt; if a.file_attributes() & 0x400 != 0 || b.file_attributes() & 0x400 != 0 || a.creation_time() != b.creation_time() || a.last_write_time() != b.last_write_time() { return false; } }
    true
}
fn custody_file(path: &::std::path::Path, length: usize, expected: &str) -> Result<String, &'static str> {
    if length > CUSTODY_CAP { return Err("bound"); }
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
    if !custody_same_file(&path_before, &metadata) || metadata.len() != u64::try_from(length).map_err(|_| "bound")? {
        return Err("length-or-nonregular");
    }
    // The one opened no-follow regular handle is checked before any buffer.
    let value = custody_body(&mut file, length, expected)?;
    let after = file.metadata().map_err(|_| "metadata")?;
    let path_after = std::fs::symlink_metadata(path).map_err(|_| "metadata")?;
    if path_after.file_type().is_symlink() || !custody_same_file(&metadata, &after) || !custody_same_file(&after, &path_after) { return Err("metadata-drift"); }
    Ok(value)
}

fn compiled(path: &str, text: &str) -> Result<(), &'static str> {
    let actual = custody_file(&::std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join(path), text.len(), &custody_digest(text.as_bytes()))?;
    if actual != text { return Err("compiled-source-drift"); }
    Ok(())
}
fn subject<'a>(value: &'a serde_json::Value, path: &str, domain: &str) -> Result<&'a serde_json::Value, &'static str> {
    if !matches!(domain, "ours" | "main") { return Err("domain"); }
    if !path.split('/').all(|v| !v.is_empty() && v != "." && v != "..") { return Err("path"); }
    let rows = value["inputs"].as_array().ok_or("rows")?;
    if rows.len() != 13 { return Err("row-count"); }
    let sealed: serde_json::Value = serde_json::from_str(SUBJECTS).map_err(|_| "manifest")?;
    for (row, original) in rows.iter().zip(sealed["inputs"].as_array().ok_or("rows")?) {
        if row != original { return Err("sealed-row-drift"); }
    }
    let mut seen = ::std::collections::BTreeSet::new();
    for row in rows { if !seen.insert(row["path"].as_str().ok_or("path")?) { return Err("duplicate"); } }
    let row = rows.iter().find(|r| r["path"] == path).ok_or("foreign")?;
    if row[domain].is_null() { return Err("absent-domain"); }
    Ok(row)
}
fn archive(record: &serde_json::Value) -> Result<String, &'static str> {
    let sha = record["sha256"].as_str().ok_or("sha")?;
    if sha.len() != 64 || !sha.bytes().all(|c| c.is_ascii_digit() || (b'a'..=b'f').contains(&c)) { return Err("sha"); }
    let name = record["archive"].as_str().ok_or("archive")?;
    if name != format!("objects/{sha}.txt") { return Err("archive-path"); }
    let length = usize::try_from(record["length"].as_u64().ok_or("length")?).map_err(|_| "length")?;
    custody_file(&::std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join(EVIDENCE).join(name), length, sha)
}
fn domain_read(path: &::std::path::Path, domain: &str) -> Result<String, &'static str> {
    compiled(CORE_PATH, LIVE_CORE)?;
    compiled(SELF_PATH, LIVE_SELF)?;
    compiled(&format!("{EVIDENCE}/subjects.json"), SUBJECTS)?;
    let relative = path.strip_prefix(env!("CARGO_MANIFEST_DIR")).map_err(|_| "root")?;
    let relative = relative.to_str().ok_or("utf8-path")?;
    let manifest: serde_json::Value = serde_json::from_str(SUBJECTS).map_err(|_| "manifest")?;
    let row = subject(&manifest, relative, domain)?;
    // Core relocation has an authenticated live compiled endpoint; all other
    // physical inputs must match their full sealed current endpoint first.
    if relative != CORE_PATH {
        let current = &row["current"];
        let length = usize::try_from(current["length"].as_u64().ok_or("length")?).map_err(|_| "length")?;
        let actual = custody_file(path, length, current["sha256"].as_str().ok_or("sha")?)?;
        if actual != archive(current)? { return Err("current-archive"); }
    }
    // Authenticate the complete original OURS snapshot too, independently of
    // the selected MAIN endpoint. No original table/assertion is retargeted.
    archive(&row["ours"])?;
    let result = archive(&row[domain])?;
    if relative == CORE_PATH && result != if domain == "ours" { OURS_CORE } else { MAIN_CORE } { return Err("core-archive"); }
    Ok(result)
}
mod main_std {
    pub use ::std::path;
    pub mod fs {
        pub fn read_to_string(path: impl AsRef<::std::path::Path>) -> Result<String, &'static str> {
            super::super::domain_read(path.as_ref(), "main")
        }
    }
}
mod ours_std {
    pub use ::std::path;
    pub mod fs {
        pub fn read_to_string(path: impl AsRef<::std::path::Path>) -> Result<String, &'static str> {
            super::super::domain_read(path.as_ref(), "ours")
        }
    }
}
#[test]
fn joined_factored_domains_preserve_complete_receivers_and_refuse_foreign_inputs() {
    use ::std::io::Cursor;
    let manifest: serde_json::Value = serde_json::from_str(SUBJECTS).unwrap();
    for row in manifest["inputs"].as_array().unwrap() {
        let path = row["path"].as_str().unwrap();
        for domain in ["ours", "main"] {
            if !row[domain].is_null() {
                let restored = domain_read(&::std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join(path), domain).unwrap();
                assert_eq!(restored, archive(&row[domain]).unwrap());
            }
        }
    }
    assert!(subject(&manifest, "foreign", "ours").is_err());
    assert!(subject(&manifest, CORE_PATH, "foreign").is_err());
    assert!(subject(&manifest, "../foreign", "ours").is_err());
    assert!(subject(&manifest, "src/cleanup_plan/replay/factored/canonical_shared/borrowed_labels.rs", "main").is_err());
    let mut duplicate = manifest.clone();
    duplicate["inputs"][1] = duplicate["inputs"][0].clone();
    assert!(subject(&duplicate, CORE_PATH, "ours").is_err());
    let mut reordered = manifest.clone();
    reordered["inputs"].as_array_mut().unwrap().swap(0, 1);
    assert!(subject(&reordered, CORE_PATH, "ours").is_err());
    let mut forged_row = manifest.clone();
    forged_row["inputs"][0]["main"]["sha256"] = serde_json::Value::String("0".repeat(64));
    assert!(subject(&forged_row, CORE_PATH, "main").is_err());
    let bytes = b"complete immutable source";
    let digest = custody_digest(bytes);
    assert_eq!(custody_body(&mut Cursor::new(bytes), bytes.len(), &digest).unwrap().as_bytes(), bytes);
    assert!(custody_body(&mut Cursor::new(bytes), CUSTODY_CAP + 1, &digest).is_err());
    assert!(custody_body(&mut Cursor::new(&bytes[..bytes.len()-1]), bytes.len(), &digest).is_err());
    assert!(custody_body(&mut Cursor::new(bytes), bytes.len()-1, &digest).is_err());
    assert!(custody_body(&mut Cursor::new(bytes), bytes.len(), &"0".repeat(64)).is_err());
    assert!(custody_body(&mut Cursor::new([255]), 1, &custody_digest(&[255])).is_err());
    let root = ::std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    assert!(custody_file(&root.join(EVIDENCE).join("missing-closed-control"), 0, &custody_digest(b"")).is_err());
    assert!(custody_file(&root.join(EVIDENCE), 0, &custody_digest(b"")).is_err());
    assert!(compiled(CORE_PATH, &(LIVE_CORE.to_owned()+"\r\n")).is_err());
    #[cfg(unix)]
    {
        let tmp = ::std::path::PathBuf::from(::std::env::var_os("TMPDIR").expect("SSD TMPDIR required"));
        assert!(tmp.starts_with("/Volumes/Semaprax"));
        let leaf = tmp.join(format!("factored-custody-link-{}-{}", ::std::process::id(), ::std::time::SystemTime::now().duration_since(::std::time::UNIX_EPOCH).unwrap().as_nanos()));
        ::std::os::unix::fs::symlink(root.join(CORE_PATH), &leaf).unwrap();
        let refused = custody_file(&leaf, LIVE_CORE.len(), &custody_digest(LIVE_CORE.as_bytes()));
        ::std::fs::remove_file(&leaf).unwrap();
        assert!(refused.is_err());
    }
    let mut forged = manifest["inputs"][0]["main"].clone();
    forged["archive"] = serde_json::Value::String("../foreign".to_owned());
    assert!(archive(&forged).is_err());
}
