//! Exact custody for later receiver changes, separate from CI reformatting.
//! These snapshots are used only by historical source assertions. Live source
//! inventories continue to include the current modules and their new children.
use super::*;
use super::incoming67107_semantic_receivers::{mainbcc_read, mainbcc_invert};
use super::incoming67107_semantic_receivers::invert_ordinary;
use super::incoming67107_semantic_receivers::{main34d_project, main34d_read, main34d_proofs, main34d_predecessor};

const PROOFS: &str = include_str!("../../docs/evidence/receiver-source-custody-v1/inverse.json");
static PROOF_RECORDS: OnceLock<serde_json::Value> = OnceLock::new();

// A later test receiver has its own semantic inverse before the unchanged
// formatting registry. Keep all original twelve plus two custody proofs intact.
fn followups() -> &'static [serde_json::Value] {
    static RECORDS: OnceLock<Vec<serde_json::Value>> = OnceLock::new();
    RECORDS.get_or_init(|| {
        serde_json::from_str(include_str!(
            "../../docs/evidence/source-reader-followups-20261016/allocation.json"
        ))
        .unwrap()
    })
}

// Physical setup successors have their own complete-input custody before the
// frozen formatting input. They do not edit or replace any formatting repair,
// original twelve-plus-two receiver proof, or allocation followup.
fn ordinary_setup() -> &'static [serde_json::Value] {
    static RECORDS: OnceLock<Vec<serde_json::Value>> = OnceLock::new();
    RECORDS.get_or_init(|| {
        serde_json::from_str(
            &std::fs::read_to_string(
                Path::new(env!("CARGO_MANIFEST_DIR"))
                    .join("docs/evidence/ordinary-replay-setup-v1/format-inputs.json"),
            )
            .unwrap(),
        )
        .unwrap()
    })
}

// Other promoted helper inputs also retain their complete historical view.
// Read evidence at runtime so these archived modules are not embedded in each
// compiler test binary.
fn ordinary_helpers() -> &'static [serde_json::Value] {
    static RECORDS: OnceLock<Vec<serde_json::Value>> = OnceLock::new();
    RECORDS.get_or_init(|| {
        serde_json::from_str(
            &std::fs::read_to_string(
                Path::new(env!("CARGO_MANIFEST_DIR"))
                    .join("docs/evidence/ordinary-replay-setup-v1/helper-inputs.json"),
            )
            .unwrap(),
        )
        .unwrap()
    })
}

// Separate semantic custody for the later held status-prefix receiver.
fn held_status() -> &'static [serde_json::Value] {
    static RECORDS: OnceLock<Vec<serde_json::Value>> = OnceLock::new();
    RECORDS.get_or_init(|| {
        serde_json::from_str(
            &main34d_read(
                "held-status",
                &Path::new(env!("CARGO_MANIFEST_DIR"))
                    .join("docs/evidence/held-status-prefix-v1/views.json"),
            )
            .unwrap(),
        )
        .unwrap()
    })
}

// Separate semantic inputs for invocation-held continuation backing.
fn held_continuations() -> &'static [serde_json::Value] {
    static RECORDS: OnceLock<Vec<serde_json::Value>> = OnceLock::new();
    RECORDS.get_or_init(|| {
        serde_json::from_str(
            &main34d_read(
                "held-continuation",
                &Path::new(env!("CARGO_MANIFEST_DIR"))
                    .join("docs/evidence/held-continuation-growth-v1/views.json"),
            )
            .unwrap(),
        )
        .unwrap()
    })
}

// Separate complete-input custody for the paid lookup comparison state.
fn receiving_lookups() -> &'static [serde_json::Value] {
    static RECORDS: OnceLock<Vec<serde_json::Value>> = OnceLock::new();
    RECORDS.get_or_init(|| {
        serde_json::from_str(
            &main34d_read(
                "receiving-lookup",
                &Path::new(env!("CARGO_MANIFEST_DIR"))
                    .join("docs/evidence/paid-expression-lookup-state-v1/views.json"),
            )
            .unwrap(),
        )
        .unwrap()
    })
}

fn proofs() -> &'static [serde_json::Value] {
    PROOF_RECORDS
        .get_or_init(|| serde_json::from_str(PROOFS).unwrap())
        .as_array()
        .unwrap()
}

fn digest(source: &str) -> String {
    format!(
        "{:x}",
        crate::digest_hex::LowerHex(Sha256::digest(source.as_bytes()))
    )
}

fn invert(proof: &serde_json::Value, current: &str) -> String {
    let acquired = incoming_main21::ours_expected_input(proof, current);
    let current = acquired.as_str();
    let path = proof["path"].as_str().unwrap();
    let joined = crate::source_reflows_merge::main8fb_input(path, current);
    let current = joined.as_str();
    assert_eq!(
        digest(current),
        proof["after_sha256"],
        "receiver input drifted: {path}"
    );
    let mut restored = current.to_owned();
    for edit in proof["inverse"].as_array().unwrap() {
        let after = edit["after"].as_str().unwrap();
        assert_eq!(
            restored.matches(after).count(),
            1,
            "nonunique receiver inverse: {path}"
        );
        restored = restored.replacen(after, edit["before"].as_str().unwrap(), 1);
    }
    assert_eq!(
        digest(&restored),
        proof["before_sha256"],
        "receiver predecessor drifted: {path}"
    );
    restored
}

mod retained_predecessor {
use super::*;
pub(super) fn predecessor(path: &str, current: &str) -> Option<String> {
    if let Some(proof) = receiving_lookups()
        .iter()
        .find(|proof| proof["path"] == path)
    {
        let view = main34d_project("receiving-lookup", path, current);
        return Some(invert_ordinary(proof, &view));
    }
    if let Some(proof) = held_continuations()
        .iter()
        .find(|proof| proof["path"] == path)
    {
        let view = main34d_project("held-continuation", path, current);
        return Some(invert_ordinary(proof, &view));
    }
    if let Some(proof) = held_status().iter().find(|proof| proof["path"] == path) {
        let view = main34d_project("held-status", path, current);
        return Some(invert_ordinary(proof, &view));
    }
    if let Some(proof) = ordinary_setup()
        .iter()
        .chain(ordinary_helpers())
        .find(|proof| proof["path"] == path)
    {
        return Some(invert_ordinary(proof, current));
    }
    if let Some(proof) = followups().iter().find(|proof| proof["path"] == path) {
        return Some(invert(proof, current));
    }
    let proof = proofs().iter().find(|proof| proof["path"] == path)?;
    Some(invert(proof, current))
}
}


mod retained_contains {
use super::*;
pub(super) fn contains(path: &str) -> bool {
    proofs()
        .iter()
        .chain(followups())
        .chain(ordinary_setup())
        .chain(ordinary_helpers())
        .chain(held_status())
        .chain(held_continuations())
        .chain(receiving_lookups())
        .any(|proof| proof["path"] == path)
}
}


#[test]
fn ordinary_helper_views_authenticate_every_complete_input() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    assert_eq!(ordinary_helpers().len(), 20);
    let mut paths = std::collections::BTreeSet::new();
    for proof in ordinary_helpers() {
        let path = proof["path"].as_str().unwrap();
        assert!(paths.insert(path));
        assert!(!proofs()
            .iter()
            .chain(followups())
            .chain(ordinary_setup())
            .any(|old| old["path"] == path));
        let current = std::fs::read_to_string(root.join(path)).unwrap();
        let before = invert_ordinary(proof, &current);
        assert_eq!(
            before,
            std::fs::read_to_string(root.join(proof["archive"].as_str().unwrap())).unwrap()
        );
        assert!(contains(path));
        assert_eq!(predecessor(path, &current), Some(before));
        assert!(std::panic::catch_unwind(|| invert_ordinary(proof, &(current.clone() + "\n"))).is_err());
        let mut forged = proof.clone();
        forged["before_sha256"] = serde_json::Value::String("0".repeat(64));
        assert!(std::panic::catch_unwind(|| invert_ordinary(&forged, &current)).is_err());
    }
}

#[test]
fn ordinary_setup_complete_sources_and_original_legacy_constructor_remain_exact() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let evidence = root.join("docs/evidence/ordinary-replay-setup-v1");
    let rows: Vec<serde_json::Value> =
        serde_json::from_str(&std::fs::read_to_string(evidence.join("sources.json")).unwrap())
            .unwrap();
    assert_eq!(rows.len(), 34);
    let mut paths = std::collections::BTreeSet::new();
    for row in &rows {
        let path = row["path"].as_str().unwrap();
        assert!(paths.insert(path));
        let current = std::fs::read_to_string(root.join(path)).unwrap();
        let before = invert_ordinary(row, &current);
        assert_eq!(
            before,
            std::fs::read_to_string(evidence.join(row["archive"].as_str().unwrap())).unwrap()
        );
        assert!(std::panic::catch_unwind(|| invert_ordinary(row, &(current.clone() + "\n"))).is_err());
        if path == "src/cleanup_plan/replay/skeleton_work.rs" {
            let start = before.find("        Ok(Self {\n").unwrap();
            let end = before[start..]
                .find("\n    pub(super) fn checked_with_index(")
                .unwrap()
                + start;
            let legacy = &before[start..end];
            assert_eq!(current.matches(legacy).count(), 1);
        }
    }
}

#[test]
fn ordinary_setup_bridges_preserve_every_complete_original_format_input() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    assert_eq!(ordinary_setup().len(), 6);
    let mut paths = std::collections::BTreeSet::new();
    for proof in ordinary_setup() {
        let path = proof["path"].as_str().unwrap();
        assert!(paths.insert(path));
        assert!(!proofs()
            .iter()
            .chain(followups())
            .any(|old| old["path"] == path));
        let original_format = records()
            .iter()
            .find(|record| record["path"] == path)
            .unwrap();
        assert_eq!(proof["before_sha256"], original_format["after_sha256"]);
        let current = std::fs::read_to_string(root.join(path)).unwrap();
        let before = invert_ordinary(proof, &current);
        assert_eq!(
            before,
            std::fs::read_to_string(root.join(proof["archive"].as_str().unwrap())).unwrap()
        );
        assert_eq!(
            digest(&restore(original_format, &before)),
            original_format["before_sha256"]
        );
        assert!(std::panic::catch_unwind(|| invert_ordinary(proof, &(current.clone() + "\n"))).is_err());
    }
}

#[test]
fn every_receiver_inverse_preserves_the_entire_reflow_input_and_archive() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let mut paths = std::collections::BTreeSet::new();
    assert_eq!(proofs().len(), 5);
    for proof in proofs() {
        let path = proof["path"].as_str().unwrap();
        assert!(paths.insert(path));
        let repair = records().iter().find(|repair| repair["path"] == path);
        if let Some(repair) = repair {
            assert_eq!(proof["before_sha256"], repair["after_sha256"]);
        }
        let current = std::fs::read_to_string(root.join(path)).unwrap();
        let restored = invert(proof, &current);
        let archive = root
            .join("docs/evidence/receiver-source-custody-v1")
            .join(proof["archive"].as_str().unwrap());
        assert_eq!(restored, std::fs::read_to_string(archive).unwrap());
        assert_ne!(current, restored);
        // Execute the unchanged formatting inverse and its whole-file digest.
        if let Some(repair) = repair {
            assert_ne!(restore_formatting(repair, &restored), restored);
        }
    }
    assert!(predecessor("src/source_reflows.rs", "unregistered source").is_none());
}

#[test]
fn receiver_mutation_and_forged_predecessor_fail_closed() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    for proof in proofs() {
        let current = std::fs::read_to_string(root.join(proof["path"].as_str().unwrap())).unwrap();
        let changed = format!("{current} ");
        assert!(std::panic::catch_unwind(|| invert(proof, &changed)).is_err());
        let mut forged = proof.clone();
        forged["before_sha256"] = serde_json::Value::String("0".repeat(64));
        assert!(std::panic::catch_unwind(|| invert(&forged, &current)).is_err());
    }
}

#[test]
fn allocation_followup_preserves_the_complete_original_format_input() {
    assert_eq!(followups().len(), 1);
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    for proof in followups() {
        let path = proof["path"].as_str().unwrap();
        assert!(!proofs().iter().any(|original| original["path"] == path));
        let current = std::fs::read_to_string(root.join(path)).unwrap();
        let restored = invert(proof, &current);
        let archive = root
            .join("docs/evidence/source-reader-followups-20261016")
            .join(proof["archive"].as_str().unwrap());
        assert_eq!(restored, std::fs::read_to_string(archive).unwrap());
        let format = records()
            .iter()
            .find(|record| record["path"] == path)
            .unwrap();
        assert_eq!(proof["before_sha256"], format["after_sha256"]);
        assert_ne!(restore(format, &restored), restored);
        assert!(std::panic::catch_unwind(|| invert(proof, &(current.clone() + "\n"))).is_err());
        let mut forged = proof.clone();
        forged["before_sha256"] = serde_json::Value::String("0".repeat(64));
        assert!(std::panic::catch_unwind(|| invert(&forged, &current)).is_err());
    }
}

#[test]
fn complete_followup_receivers_keep_all_original_source_bytes() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let evidence = root.join("docs/evidence/source-reader-followups-20261016");
    let records: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(evidence.join("sources.json")).unwrap())
            .unwrap();
    for record in records.as_array().unwrap() {
        let original =
            std::fs::read_to_string(evidence.join(record["archive"].as_str().unwrap())).unwrap();
        assert_eq!(digest(&original), record["sha256"]);
        let current = std::fs::read_to_string(root.join(record["path"].as_str().unwrap())).unwrap();
        let current = crate::source_reflows_merge::main7f8_source_inputs::source("source-followups", record["path"].as_str().unwrap(), &current);
        let restore = |source: &str| -> Option<String> {
            let mut restored = source.to_owned();
            for edit in record["edits"].as_array().unwrap().iter().rev() {
                let new = edit["new"].as_str().unwrap();
                if restored.matches(new).count() != 1 {
                    return None;
                }
                restored = restored.replacen(new, edit["old"].as_str().unwrap(), 1);
            }
            Some(restored)
        };
        assert_eq!(restore(&current).as_deref(), Some(original.as_str()));
        assert_ne!(
            restore(&(current.clone() + "\n")).as_deref(),
            Some(original.as_str())
        );
        assert_ne!(
            restore(&current.replacen(&current[..1], "!", 1)).as_deref(),
            Some(original.as_str())
        );
    }
}

#[test]
fn held_status_views_preserve_complete_original_inputs_and_validation_loops() {
    // Explicit historical MAIN34d source view; actual raw roles remain live.
    let main_read = |path: std::path::PathBuf| main34d_read("held-status", &path);
    let invert = invert_ordinary;
    let proofs = main34d_proofs;
    let predecessor = main34d_predecessor;
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    assert_eq!(held_status().len(), 3);
    let mut paths = std::collections::BTreeSet::new();
    for proof in held_status() {
        let path = proof["path"].as_str().unwrap();
        assert!(paths.insert(path));
        assert!(!proofs()
            .iter()
            .chain(followups())
            .chain(ordinary_setup())
            .chain(ordinary_helpers())
            .any(|old| old["path"] == path));
        let current = main_read(root.join(path)).unwrap();
        let before = invert(proof, &current);
        assert_eq!(
            before,
            main_read(root.join(proof["archive"].as_str().unwrap())).unwrap()
        );
        if let Some(format) = records().iter().find(|old| old["path"] == path) {
            assert_eq!(proof["before_sha256"], format["after_sha256"]);
            assert_eq!(digest(&restore(format, &before)), format["before_sha256"]);
        }
        assert_eq!(predecessor(path, &current), Some(before));
        assert!(contains(path));
        assert!(std::panic::catch_unwind(|| invert(proof, &(current.clone() + "\n"))).is_err());
        let mut forged = proof.clone();
        forged["before_sha256"] = serde_json::Value::String("0".repeat(64));
        assert!(std::panic::catch_unwind(|| invert(&forged, &current)).is_err());
    }
    let whole: Vec<serde_json::Value> = serde_json::from_str(
        &main_read(root.join("docs/evidence/held-status-prefix-v1/sources.json"))
            .unwrap(),
    )
    .unwrap();
    assert_eq!(whole.len(), 6);
    for proof in &whole {
        let current = main_read(root.join(proof["path"].as_str().unwrap())).unwrap();
        let before = invert(proof, &current);
        assert_eq!(
            before,
            main_read(root.join(proof["archive"].as_str().unwrap())).unwrap()
        );
        assert!(std::panic::catch_unwind(|| invert(proof, &(current.clone() + "\n"))).is_err());
    }
    let proof = held_status()
        .iter()
        .find(|p| p["path"] == "src/cleanup_plan/replay/expression_facts/receiving/retained.rs")
        .unwrap();
    let before = main_read(root.join(proof["archive"].as_str().unwrap())).unwrap();
    let current = main_read(root.join(proof["path"].as_str().unwrap())).unwrap();
    let guard="    if budget.pays_allocations() && function.cleanup_plan.status_sources.len() >= 3 {\n        return validate_status_sources_held_prefix(function, facts, budget);\n    }\n";
    assert_eq!(current.matches(guard).count(), 1);
    assert!(current.replacen(guard, "", 1).starts_with(&before));
    let start = before.find("fn validate_status_sources_retained").unwrap();
    let end = start + before[start..].find("\n// The retained key proof").unwrap();
    let expected = before[start..end]
        .replace(
            "fn validate_status_sources_retained",
            "fn validate_status_sources_held_prefix",
        )
        .replace(
            "let prefix_owner = directory::StatusInsertionOwner::new(function, &ids, budget)?;",
            "let mut prefix_owner = ids.held_prefix(function, budget)?;",
        )
        .replace(
            "ids.insert_certified(function, &source.id, budget, &prefix_owner)?",
            "prefix_owner.insert(function, &source.id, budget)?",
        );
    let start = current
        .find("fn validate_status_sources_held_prefix")
        .unwrap();
    assert_eq!(current[start..].trim_end(), expected.trim_end());
}

#[test]
fn held_continuation_inputs_preserve_the_complete_machine_and_all_original_controls() {
    // Explicit historical MAIN34d source view; actual raw roles remain live.
    let main_read = |path: std::path::PathBuf| main34d_read("held-continuation", &path);
    let invert = invert_ordinary;
    let proofs = main34d_proofs;
    let predecessor = main34d_predecessor;
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    assert_eq!(held_continuations().len(), 2);
    let mut paths = std::collections::BTreeSet::new();
    for proof in held_continuations() {
        let path = proof["path"].as_str().unwrap();
        assert!(paths.insert(path));
        assert!(!proofs()
            .iter()
            .chain(followups())
            .chain(ordinary_setup())
            .chain(ordinary_helpers())
            .chain(held_status())
            .any(|old| old["path"] == path));
        let current = main_read(root.join(path)).unwrap();
        let before = invert(proof, &current);
        assert_eq!(
            before,
            main_read(root.join(proof["archive"].as_str().unwrap())).unwrap()
        );
        assert!(contains(path));
        assert_eq!(predecessor(path, &current), Some(before));
        assert!(std::panic::catch_unwind(|| invert(proof, &(current.clone() + "\n"))).is_err());
        let mut forged = proof.clone();
        forged["before_sha256"] = serde_json::Value::String("0".repeat(64));
        assert!(std::panic::catch_unwind(|| invert(&forged, &current)).is_err());
    }
    let whole: Vec<serde_json::Value> = serde_json::from_str(
        &main_read(
            root.join("docs/evidence/held-continuation-growth-v1/sources.json"),
        )
        .unwrap(),
    )
    .unwrap();
    assert_eq!(whole.len(), 10);
    for proof in &whole {
        let current = main_read(root.join(proof["path"].as_str().unwrap())).unwrap();
        let before = invert(proof, &current);
        assert_eq!(
            before,
            main_read(root.join(proof["archive"].as_str().unwrap())).unwrap()
        );
        assert!(std::panic::catch_unwind(|| invert(proof, &(current.clone() + "\n"))).is_err());
    }
    let before_file = |path: &str| {
        let proof = whole.iter().find(|p| p["path"] == path).unwrap();
        main_read(root.join(proof["archive"].as_str().unwrap())).unwrap()
    };
    let old_tests = before_file("src/cleanup_plan/replay/continuation_storage/tests.rs");
    let current_tests =
        main_read(root.join("src/cleanup_plan/replay/continuation_storage/tests.rs"))
            .unwrap();
    assert!(current_tests.starts_with(&old_tests));
    let old_storage = before_file("src/cleanup_plan/replay/continuation_storage.rs");
    let current_storage =
        main_read(root.join("src/cleanup_plan/replay/continuation_storage.rs"))
            .unwrap();
    let fixed = old_storage.find("pub(super) fn fresh").unwrap();
    let fixed_end = fixed
        + old_storage[fixed..]
            .find("/// An owning traversal")
            .unwrap();
    assert_eq!(
        current_storage
            .matches(&old_storage[fixed..fixed_end])
            .count(),
        1
    );
    let machine_types = old_storage.find("pub(super) type OwnedStep").unwrap();
    assert!(current_storage.contains(&old_storage[machine_types..]));
    let mut machine = main_read(root.join("src/cleanup_plan/replay.rs")).unwrap();
    let after = r###"            if $frames.len() == $frames.capacity() {
                continuation_storage::reserve_push($frames, function, work.budget)?;
            }"###;
    let before = r###"            if $frames.len() == $frames.capacity() {
                return Err(replay_error(
                    function,
                    "typed-HIR skeleton traversal exceeds the admitted depth",
                ));
            }"###;
    assert_eq!(machine.matches(after).count(), 1);
    machine = machine.replacen(after, before, 1);
    let after = r###"            if frames.len() == frames.capacity() {
                continuation_storage::reserve_push(frames, function, work.budget)?;
            }"###;
    let before = r###"            if frames.len() == frames.capacity() {
                return Err(replay_error(
                    function,
                    "typed-HIR skeleton traversal exceeds the admitted depth",
                ));
            }"###;
    assert_eq!(machine.matches(after).count(), 1);
    machine = machine.replacen(after, before, 1);
    assert_eq!(machine, before_file("src/cleanup_plan/replay.rs"));
}

#[test]
fn receiving_lookup_views_keep_complete_prior_modules_and_all_owning_inputs() {
    // Explicit historical MAIN34d source view; actual raw roles remain live.
    let main_read = |path: std::path::PathBuf| main34d_read("receiving-lookup", &path);
    let invert = invert_ordinary;
    let proofs = main34d_proofs;
    let predecessor = main34d_predecessor;
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    assert_eq!(receiving_lookups().len(), 4);
    let mut paths = std::collections::BTreeSet::new();
    for proof in receiving_lookups() {
        let path = proof["path"].as_str().unwrap();
        assert!(paths.insert(path));
        assert!(!proofs()
            .iter()
            .chain(followups())
            .chain(ordinary_setup())
            .chain(ordinary_helpers())
            .chain(held_status())
            .chain(held_continuations())
            .any(|old| old["path"] == path));
        let current = main_read(root.join(path)).unwrap();
        let before = invert(proof, &current);
        assert_eq!(
            before,
            main_read(root.join(proof["archive"].as_str().unwrap())).unwrap()
        );
        assert!(contains(path));
        assert_eq!(predecessor(path, &current), Some(before));
        if let Some(format) = records().iter().find(|old| old["path"] == path) {
            assert_eq!(proof["before_sha256"], format["after_sha256"]);
            let before = invert(proof, &current);
            assert_eq!(digest(&restore(format, &before)), format["before_sha256"]);
        }
        assert!(std::panic::catch_unwind(|| invert(proof, &(current.clone() + "\n"))).is_err());
        let mut forged = proof.clone();
        forged["before_sha256"] = serde_json::Value::String("0".repeat(64));
        assert!(std::panic::catch_unwind(|| invert(&forged, &current)).is_err());
    }
    let whole: Vec<serde_json::Value> = serde_json::from_str(
        &main_read(
            root.join("docs/evidence/paid-expression-lookup-state-v1/sources.json"),
        )
        .unwrap(),
    )
    .unwrap();
    assert_eq!(whole.len(), 10);
    for proof in &whole {
        let current = main_read(root.join(proof["path"].as_str().unwrap())).unwrap();
        let before = invert(proof, &current);
        assert_eq!(
            before,
            main_read(root.join(proof["archive"].as_str().unwrap())).unwrap()
        );
        assert!(std::panic::catch_unwind(|| invert(proof, &(current.clone() + "\n"))).is_err());
    }
    let before_file = |path: &str| {
        let proof = whole.iter().find(|p| p["path"] == path).unwrap();
        main_read(root.join(proof["archive"].as_str().unwrap())).unwrap()
    };
    for path in [
        "src/cleanup_plan/replay/expression_facts/arena/prefix.rs",
        "src/cleanup_plan/replay/expression_facts/arena/prefix/tests.rs",
    ] {
        assert!(main_read(root.join(path))
            .unwrap()
            .starts_with(&before_file(path)));
    }
    let path = "src/cleanup_plan/replay/expression_facts/arena.rs";
    let current = main_read(root.join(path)).unwrap();
    let guard = "        if budget.payer == AllocationPayer::ActiveBuilder && !self.nodes.is_empty() {\n            prefix::find_receiving(self, function, key, budget)\n        } else {\n            prefix::find(self, function, key, budget)\n        }";
    assert_eq!(current.matches(guard).count(), 1);
    assert_eq!(
        current.replacen(
            guard,
            "        prefix::find(self, function, key, budget)",
            1
        ),
        before_file(path)
    );
    let path = "src/cleanup_plan/replay/expression_facts/arena/cursor_tests.rs";
    let current = main_read(root.join(path)).unwrap();
    assert_eq!(
        current
            .matches("let arena_now = ci_str!(\"../arena.rs\");")
            .count(),
        1
    );
    assert_eq!(
        current.replacen(
            "let arena_now = ci_str!(\"../arena.rs\");",
            "let arena_now = include_str!(\"../arena.rs\");",
            1
        ),
        before_file(path)
    );
}

#[test]
fn ordinary_replay_routes_keep_complete_predecessors_and_existing_controls() {
    // This complete incoming owner reads only its independent immutable BCC domain.
    let main_read = |path: std::path::PathBuf| mainbcc_read(&path);
    let invert = mainbcc_invert;
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let records: Vec<serde_json::Value> = serde_json::from_str(
        &main_read(root.join("docs/evidence/ordinary-replay-routes-v1/sources.json"))
            .unwrap(),
    )
    .unwrap();
    assert_eq!(records.len(), 6);
    let mut paths = std::collections::BTreeSet::new();
    for proof in records {
        let path = proof["path"].as_str().unwrap();
        assert!(paths.insert(path.to_owned()));
        let current = main_read(root.join(path)).unwrap();
        let before = invert(&proof, &current);
        assert_eq!(
            before,
            main_read(root.join(proof["archive"].as_str().unwrap())).unwrap()
        );
        if path.ends_with("/tests.rs") {
            assert!(current.starts_with(&before));
        }
        assert!(std::panic::catch_unwind(|| invert(&proof, &(current.clone() + "\n"))).is_err());
        let mut forged = proof.clone();
        forged["before_sha256"] = serde_json::Value::String("0".repeat(64));
        assert!(std::panic::catch_unwind(|| invert(&forged, &current)).is_err());
    }
}

// Independent MAIN21 namespace and whole-source acquisitions precede all old domains.
#[path = "incoming21_semantic_receivers.rs"]
pub(super) mod incoming_main21;

pub(super) fn predecessor(path: &str, current: &str) -> Option<String> {
    if retained_contains::contains(path) {
        return retained_predecessor::predecessor(path, current);
    }
    let acquired = incoming_main21::ours_input(path, current);
    (acquired != current).then_some(acquired)
}
pub(super) fn contains(path: &str) -> bool {
    incoming_main21::ours_contains(path) || retained_contains::contains(path)
}

// Semantic inverse owners pass complete, authenticated formatting AFTERimages.
// Keep raw-current acquisition at source_reflows::restore for its live owners;
// this namespace's archived-stage calls retain the full original formatter.
fn restore(record: &serde_json::Value, current: &str) -> String {
    super::restore_formatting(record, current)
}

#[path = "incoming090_semantic_receivers.rs"]
pub(super) mod incoming_main090;

#[path = "main_c883_receivers.rs"]
mod main_c883;

// Independent1e controller: only explicit acquisition expressions differ.
mod main_1e {
use super::*;
use crate::source_reflows_merge::main_c883_inputs::read_1e as main_1e_read;

fn invert(proof: &serde_json::Value, current: &str) -> String {
    let path = proof["path"].as_str().unwrap();
    assert_eq!(
        digest(current),
        proof["after_sha256"],
        "receiver input drifted: {path}"
    );
    let mut restored = current.to_owned();
    for edit in proof["inverse"].as_array().unwrap() {
        let after = edit["after"].as_str().unwrap();
        assert_eq!(
            restored.matches(after).count(),
            1,
            "nonunique receiver inverse: {path}"
        );
        restored = restored.replacen(after, edit["before"].as_str().unwrap(), 1);
    }
    assert_eq!(
        digest(&restored),
        proof["before_sha256"],
        "receiver predecessor drifted: {path}"
    );
    restored
}

#[test]
fn replay_operation_totals_preserve_complete_predecessors_and_production_text() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let proofs: Vec<serde_json::Value> = serde_json::from_str(
        &main_1e_read(
            root.join("docs/evidence/replay-operation-totals-v1/sources.json"),
        )
        .unwrap(),
    )
    .unwrap();
    assert_eq!(proofs.len(), 10);
    let mut paths = std::collections::BTreeSet::new();
    for proof in &proofs {
        let path = proof["path"].as_str().unwrap();
        assert!(paths.insert(path));
        let current = main_1e_read(root.join(path)).unwrap();
        let before = invert(proof, &current);
        assert_eq!(
            before,
            main_1e_read(root.join(proof["archive"].as_str().unwrap())).unwrap()
        );
        assert!(std::panic::catch_unwind(|| invert(proof, &(current.clone() + "\n"))).is_err());
        let mut forged = proof.clone();
        forged["before_sha256"] = serde_json::Value::String("0".repeat(64));
        assert!(std::panic::catch_unwind(|| invert(&forged, &current)).is_err());
    }
    let guards: Vec<serde_json::Value> = serde_json::from_str(
        &main_1e_read(root.join("docs/evidence/replay-operation-totals-v1/guards.json"))
            .unwrap(),
    )
    .unwrap();
    assert_eq!(guards.len(), 8);
    for path in guards
        .iter()
        .map(|row| row["path"].as_str().unwrap())
        .collect::<std::collections::BTreeSet<_>>()
    {
        let mut current = main_1e_read(root.join(path)).unwrap();
        for guard in guards.iter().filter(|row| row["path"] == path) {
            let text = guard["statement"].as_str().unwrap();
            assert!(text.contains("#[cfg(test)]"));
            assert_eq!(current.matches(text).count(), 1);
            current = current.replacen(text, "", 1);
        }
        let proof = proofs.iter().find(|proof| proof["path"] == path).unwrap();
        assert_eq!(
            current,
            main_1e_read(root.join(proof["archive"].as_str().unwrap())).unwrap()
        );
    }
}

}

#[test]
fn current_replay_operation_guards_bind_the_actual_receivers_without_production_changes() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let sources = [
        ("src/cleanup_plan/replay.rs", include_str!("../../src/cleanup_plan/replay.rs")),
        ("src/cleanup_plan/replay/expression_facts.rs", include_str!("../../src/cleanup_plan/replay/expression_facts.rs")),
        ("src/cleanup_plan/replay/path_join.rs", include_str!("../../src/cleanup_plan/replay/path_join.rs")),
        ("src/cleanup_plan/replay/factored.rs", include_str!("../../src/cleanup_plan/replay/factored.rs")),
        ("src/cleanup_plan/replay/factored/canonical_shared.rs", include_str!("../../src/cleanup_plan/replay/factored/canonical_shared.rs")),
    ];
    // Entries bind exact CURRENT function spans; main guards.json stays immutable
    // in its own epoch, where the canonical name was id rather than id_with_receiving.
    let guards = [
        (0, "validate_structure_with_budget_storage", "branch_sensitive_cfg_bounds", "ReplayStructure", 4usize),
        (0, "hir_skeleton_paths_with_index_storage", "sequence_expression", "ReplayHirSkeleton", 4),
        (0, "plan_skeleton_paths", "storage_regions", "ReplayPlanSkeleton", 4),
        (1, "with_budget", "paid_arena", "ReplayExpressionFacts", 4),
        (2, "validate_path_states", "validate_control_group", "ReplayPathStates", 4),
        (3, "validate_with_index_storage", "skeleton_mismatch", "ReplayFactored", 4),
        (3, "build_comparison", "transition_observation", "ReplayFactoredConstruction", 4),
        (4, "id_with_receiving", "id_inner_into", "ReplayFactoredCanonical", 8),
    ];
    for (path, compiled) in sources {
        assert_eq!(std::fs::read_to_string(root.join(path)).unwrap(), compiled);
    }
    let mut stripped = sources.map(|(_, source)| source.to_owned());
    for (index, name, next, operation, indent) in guards {
        let source = sources[index].1;
        let marker = format!("fn {name}");
        let next_marker = format!("fn {next}");
        assert_eq!(source.matches(&marker).count(), 1);
        let tail = source.split_once(&marker).unwrap().1;
        let body = tail.split_once(&next_marker).unwrap().0;
        let pad = " ".repeat(indent);
        let statement = format!("\n{pad}#[cfg(test)]\n{pad}let _operation = crate::bounded_output::stage_debits::operation_guard(\n{pad}    crate::bounded_output::stage_debits::Operation::{operation},\n{pad});");
        assert_eq!(source.matches(&statement).count(), 1);
        assert_eq!(body.matches(&statement).count(), 1, "actual receiver span: {path}:{name}", path=sources[index].0);
        assert_eq!(stripped[index].matches(&statement).count(), 1);
        stripped[index] = stripped[index].replacen(&statement, "", 1);
    }
    let sealed = include_str!("../../docs/evidence/main-1e-next-source-merge-20261017/guards-current-before.json");
    assert_eq!(sealed.len(), 1473);
    assert_eq!(digest(sealed), "655603541979ed3030d7def4c70bdbfb4d71555ad446e8d934618c058a8097ec");
    let before: Vec<serde_json::Value> = serde_json::from_str(sealed).unwrap();
    assert_eq!(before.len(), sources.len());
    for (index, row) in before.iter().enumerate() {
        assert_eq!(row["path"], sources[index].0);
        let original = std::fs::read_to_string(root.join(row["archive"].as_str().unwrap())).unwrap();
        assert_eq!(original.len() as u64, row["length"].as_u64().unwrap());
        assert_eq!(digest(&original), row["sha256"]);
        assert_eq!(stripped[index], original, "complete pre-1e CURRENT source, not main historical source");
    }
}

pub(crate) use main_f247::helper_digest_capture_predecessor;
mod main_f247 {
use super::*;
use crate::source_reflows_merge::main_c883_inputs::read_f247 as main_f247_read;
/// The original source-work assertion still reads its complete prior input.
/// The separate custody control below authenticates all current changed files.
pub(crate) fn helper_digest_capture_predecessor(current: &str) -> String {
    let authenticated = crate::source_reflows_merge::main_c883_inputs::f247_expected(
        "src/workspace_graph/static_text/scalar_helpers.rs", current,
    );
    let current = authenticated.as_str();
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let proofs: Vec<serde_json::Value> = serde_json::from_str(
        &main_f247_read(
            root.join("docs/evidence/optional-helper-digest-work-v1/sources.json"),
        )
        .unwrap(),
    )
    .unwrap();
    let proof = proofs
        .iter()
        .find(|proof| proof["path"] == "src/workspace_graph/static_text/scalar_helpers.rs")
        .unwrap();
    let before = invert(proof, current);
    assert_eq!(
        before,
        main_f247_read(root.join(proof["archive"].as_str().unwrap())).unwrap()
    );
    before
}

#[test]
fn optional_helper_digest_preserves_complete_predecessors_and_all_old_controls() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let proofs: Vec<serde_json::Value> = serde_json::from_str(
        &main_f247_read(
            root.join("docs/evidence/optional-helper-digest-work-v1/sources.json"),
        )
        .unwrap(),
    )
    .unwrap();
    assert_eq!(proofs.len(), 6);
    let mut paths = std::collections::BTreeSet::new();
    for proof in &proofs {
        let path = proof["path"].as_str().unwrap();
        assert!(paths.insert(path));
        let current = main_f247_read(root.join(path)).unwrap();
        let before = invert(proof, &current);
        assert_eq!(
            before,
            main_f247_read(root.join(proof["archive"].as_str().unwrap())).unwrap()
        );
        assert!(std::panic::catch_unwind(|| invert(proof, &(current.clone() + "\n"))).is_err());
        let mut forged = proof.clone();
        forged["before_sha256"] = serde_json::Value::String("0".repeat(64));
        assert!(std::panic::catch_unwind(|| invert(&forged, &current)).is_err());
        if path == "src/workspace_graph/static_text/scalar_helpers.rs" {
            assert_eq!(helper_digest_capture_predecessor(&current), before);
            assert!(
                std::panic::catch_unwind(|| helper_digest_capture_predecessor(
                    &(current.clone() + "\n")
                ))
                .is_err()
            );
        }
    }
}

#[test]
fn isolated_lexical_path_reuse_preserves_complete_merger_and_all_prior_controls() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let proofs: Vec<serde_json::Value> = serde_json::from_str(
        &main_f247_read(
            root.join("docs/evidence/isolated-skeleton-paths-v1/sources.json"),
        )
        .unwrap(),
    )
    .unwrap();
    assert_eq!(proofs.len(), 7);
    let mut paths = std::collections::BTreeSet::new();
    let mut created = std::collections::BTreeSet::new();
    let mut merger = String::new();
    for proof in &proofs {
        let path = proof["path"].as_str().unwrap();
        assert!(paths.insert(path.to_owned()));
        let current = main_f247_read(root.join(path)).unwrap();
        let before = invert(proof, &current);
        assert_eq!(
            before,
            main_f247_read(root.join(proof["archive"].as_str().unwrap())).unwrap()
        );
        assert!(std::panic::catch_unwind(|| invert(proof, &(current.clone() + "\n"))).is_err());
        let mut forged = proof.clone();
        forged["before_sha256"] = serde_json::Value::String("0".repeat(64));
        assert!(std::panic::catch_unwind(|| invert(&forged, &current)).is_err());
        if before.is_empty() {
            created.insert(path.to_owned());
        } else if path == "src/source_reflows/semantic_receivers.rs" {
            assert!(current.starts_with(&before));
        } else if path == "src/cleanup_plan/replay/skeleton_work.rs" {
            let hook = "        if isolated_paths::can_reuse(self, &paths) {\n            self.budget.reserve_array::<u8>(\n                self.function,\n                isolated_paths::RECEIVING_BYTES,\n                \"isolated path receiving owners\",\n            )?;\n            return Ok(paths);\n        }\n";
            assert_eq!(current.matches(hook).count(), 1);
            assert_eq!(current.matches("mod isolated_paths;\n").count(), 1);
            assert_eq!(
                current
                    .replacen(hook, "", 1)
                    .replacen("mod isolated_paths;\n", "", 1),
                before
            );
            merger = before;
        }
    }
    let marker = "    pub(super) fn merged_paths(";
    assert_eq!(merger.matches(marker).count(), 1);
    let body = merger
        .split_once(marker)
        .unwrap()
        .1
        .split_once("\n    pub(super) fn merged_suffixes")
        .unwrap()
        .0;
    let saved = format!("use super::super::*;\n\nimpl SkeletonWork<'_, '_> {{\n    pub(super) fn saved_merged_paths({}\n}}\n", body.trim_end());
    assert_eq!(
        saved,
        main_f247_read(
            root.join("src/cleanup_plan/replay/skeleton_work/isolated_paths/tests/before.rs")
        )
        .unwrap()
    );
    fn inventory(root: &Path, directory: &Path, out: &mut std::collections::BTreeSet<String>) {
        for entry in std::fs::read_dir(directory).unwrap() {
            let path = entry.unwrap().path();
            if path.is_dir() {
                inventory(root, &path, out);
            } else {
                assert!(out.insert(
                    path.strip_prefix(root)
                        .unwrap()
                        .to_str()
                        .unwrap()
                        .replace('\\', "/")
                ));
            }
        }
    }
    let helper = "src/cleanup_plan/replay/skeleton_work/isolated_paths.rs";
    let mut live = std::collections::BTreeSet::from([helper.to_owned()]);
    inventory(
        root,
        &root.join("src/cleanup_plan/replay/skeleton_work/isolated_paths"),
        &mut live,
    );
    assert_eq!(created, live);
    let authority = main_f247_read(root.join("tests/hir_module_boundaries.rs")).unwrap();
    for path in created {
        assert_eq!(authority.matches(&format!("\"../{path}\"")).count(), 1);
    }
}

}
