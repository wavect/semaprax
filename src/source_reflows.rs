//! Test-only exact inverses for reviewed CI source-reader, formatting and lint repairs.
//! Production semantic changes use separately named whole-file proofs.
//! Historical source assertions still compare every original byte. Each
//! current input is authenticated in full before applying explicit inverses.
use std::collections::HashMap;
use std::path::{Component, Path, PathBuf};
use std::sync::{Mutex, OnceLock};

use sha2::{Digest, Sha256};

macro_rules! ci_str {
    ($path:expr) => {
        $crate::source_reflows::source(file!(), $path, ::std::include_str!($path))
    };
}

const REPAIRS: &str =
    include_str!("../docs/evidence/mainf045-source-successor-20261015/live-mainf045-ci93.json");
type Cache = HashMap<(&'static str, &'static str), &'static str>;
static CACHE: OnceLock<Mutex<Cache>> = OnceLock::new();
static RECORDS: OnceLock<serde_json::Value> = OnceLock::new();

// Semantic successors have their own named, whole-file custody proof. Keep
// those edits separate from the original formatting/lint repair registry.
#[path = "source_reflows/semantic_receivers.rs"]
mod semantic_receivers;
#[cfg(test)]
#[path = "source_reflows/semantic_receivers_mainb80.rs"]
mod semantic_receivers_mainb80;

#[path = "source_reflows/semantic_receivers_main7c6.rs"]
mod semantic_receivers_main7c6;

fn records() -> &'static [serde_json::Value] {
    RECORDS
        .get_or_init(|| serde_json::from_str(REPAIRS).unwrap())
        .as_array()
        .unwrap()
}

fn logical_path(caller: &str, included: &str) -> String {
    let path = if Path::new(included).is_absolute() {
        Path::new(included).to_path_buf()
    } else {
        Path::new(caller).parent().unwrap().join(included)
    };
    let path = if path.is_absolute() {
        path.strip_prefix(env!("CARGO_MANIFEST_DIR"))
            .unwrap()
            .to_path_buf()
    } else {
        path
    };
    let mut normalized = PathBuf::new();
    for part in path.components() {
        match part {
            Component::Normal(part) => normalized.push(part),
            Component::ParentDir => assert!(normalized.pop()),
            Component::CurDir => {}
            _ => panic!("invalid preserved source path: {}", path.display()),
        }
    }
    normalized.to_str().unwrap().replace('\\', "/")
}

fn restore(record: &serde_json::Value, current: &str) -> String {
    let path = record["path"].as_str().unwrap();
    let merged = crate::source_reflows_merge::main(path, current);
    let current = merged.as_str();
    restore_formatting(record, current)
}

// A semantic custody predecessor is already authenticated at its named front.
fn restore_formatting(record: &serde_json::Value, current: &str) -> String {
    let path = record["path"].as_str().unwrap();
    assert_eq!(
        format!(
            "{:x}",
            crate::digest_hex::LowerHex(Sha256::digest(current.as_bytes()))
        ),
        record["after_sha256"],
        "reviewed CI input drifted: {path}"
    );
    let mut restored = current.to_owned();
    for edit in record["inverse"].as_array().unwrap() {
        let after = edit["after"].as_str().unwrap();
        assert_eq!(
            restored.matches(after).count(),
            1,
            "nonunique CI inverse: {path}"
        );
        restored = restored.replacen(after, edit["before"].as_str().unwrap(), 1);
    }
    assert_eq!(
        format!(
            "{:x}",
            crate::digest_hex::LowerHex(Sha256::digest(restored.as_bytes()))
        ),
        record["before_sha256"],
        "CI inverse did not preserve the complete predecessor: {path}"
    );
    restored
}

pub(crate) fn source(
    caller: &'static str,
    included: &'static str,
    current: &'static str,
) -> &'static str {
    let path = logical_path(caller, included);
    let record = records().iter().find(|record| record["path"] == path);
    if record.is_none() && !semantic_receivers::contains(&path) {
        return current;
    }
    let mut cache = CACHE.get_or_init(Mutex::default).lock().unwrap();
    cache.entry((caller, included)).or_insert_with(|| {
        let predecessor = semantic_receivers::predecessor(&path, current);
        let current = predecessor.as_deref().unwrap_or(current);
        // One bounded fixture allocation per included file, retained only for
        // this test process. This helper is absent from production builds.
        let restored = match record {
            Some(record) if predecessor.is_some() => restore_formatting(record, current),
            Some(record) => restore(record, current),
            None => predecessor.unwrap(),
        };
        Box::leak(restored.into_boxed_str())
    })
}

#[test]
fn approved_reflows_authenticate_full_inputs_and_reconstruct_full_predecessors() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let mut paths = std::collections::BTreeSet::new();
    for record in records() {
        let path = record["path"].as_str().unwrap();
        assert!(paths.insert(path));
        let current = std::fs::read_to_string(root.join(path)).unwrap();
        let predecessor = semantic_receivers::predecessor(path, &current);
        let old = match predecessor {
            Some(ref previous) => restore_formatting(record, previous),
            None => restore(record, &current),
        };
        assert_ne!(current, old);
    }
    let mut main_paths = std::collections::BTreeSet::new();
    for record in main927_records() {
        let path = record["path"].as_str().unwrap();
        assert!(main_paths.insert(path));
        let current = std::fs::read_to_string(root.join(path)).unwrap();
        let old = restore_main927(record, &current);
        assert_ne!(current, old);
    }
}

#[test]
fn a_changed_current_byte_is_refused_before_an_inverse_can_hide_it() {
    let record = records().first().unwrap();
    let current = std::fs::read_to_string(
        Path::new(env!("CARGO_MANIFEST_DIR")).join(record["path"].as_str().unwrap()),
    )
    .unwrap();
    let mut changed = current.clone();
    changed.push(' ');
    assert!(std::panic::catch_unwind(|| restore(record, &changed)).is_err());
    let mut forged = record.clone();
    forged["before_sha256"] = serde_json::Value::String("0".repeat(64));
    assert!(std::panic::catch_unwind(|| restore(&forged, &current)).is_err());
    let record = main927_records().first().unwrap();
    let current = std::fs::read_to_string(
        Path::new(env!("CARGO_MANIFEST_DIR")).join(record["path"].as_str().unwrap()),
    )
    .unwrap();
    let mut changed = current.clone();
    changed.push(' ');
    assert!(std::panic::catch_unwind(|| restore_main927(record, &changed)).is_err());
    let mut forged = record.clone();
    forged["before_sha256"] = serde_json::Value::String("0".repeat(64));
    assert!(std::panic::catch_unwind(|| restore_main927(&forged, &current)).is_err());
}

// Separate MAIN927 semantic/source-reader domain; the old93 table stays immutable.
fn main927_records() -> &'static [serde_json::Value] {
    static RECORDS: OnceLock<serde_json::Value> = OnceLock::new();
    RECORDS
        .get_or_init(|| {
            serde_json::from_str(include_str!(
                "../docs/evidence/main927-source-successor-20261015/actual-main927-reflows.json"
            ))
            .unwrap()
        })
        .as_array()
        .unwrap()
}
fn restore_main927(record: &serde_json::Value, current: &str) -> String {
    let path = record["path"].as_str().unwrap();
    let mut restored = crate::source_reflows_merge::main927_input(path, current);
    assert_eq!(
        format!(
            "{:x}",
            crate::digest_hex::LowerHex(Sha256::digest(restored.as_bytes()))
        ),
        record["after_sha256"],
        "MAIN927 complete input drifted: {path}"
    );
    for edit in record["inverse"].as_array().unwrap() {
        let after = edit["after"].as_str().unwrap();
        assert_eq!(
            restored.matches(after).count(),
            1,
            "unique MAIN927 inverse: {path}"
        );
        restored = restored.replacen(after, edit["before"].as_str().unwrap(), 1);
    }
    assert_eq!(
        format!(
            "{:x}",
            crate::digest_hex::LowerHex(Sha256::digest(restored.as_bytes()))
        ),
        record["before_sha256"],
        "MAIN927 complete predecessor drifted: {path}"
    );
    restored
}

#[path = "source_reflows/semantic_receivers_main_afba.rs"]
mod semantic_receivers_main_afba;

#[path = "source_reflows/semantic_receivers_main_f1.rs"]
mod semantic_receivers_main_f1;
