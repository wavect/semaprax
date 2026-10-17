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
