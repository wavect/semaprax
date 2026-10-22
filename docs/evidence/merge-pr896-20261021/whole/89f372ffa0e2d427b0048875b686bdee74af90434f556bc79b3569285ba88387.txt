//! Exact custody for later receiver changes, separate from CI reformatting.
//! These snapshots are used only by historical source assertions. Live source
//! inventories continue to include the current modules and their new children.
use super::main_e177_support::{records, restore};
use super::*;
use crate::source_reflows_merge::main_e177_inputs::read as main_e177_read;

static PROOF_RECORDS: OnceLock<Vec<serde_json::Value>> = OnceLock::new();

// A later test receiver has its own semantic inverse before the unchanged
// formatting registry. Keep all original twelve plus two custody proofs intact.
fn followups() -> &'static [serde_json::Value] {
    static RECORDS: OnceLock<Vec<serde_json::Value>> = OnceLock::new();
    RECORDS.get_or_init(|| {
        serde_json::from_str(
            &main_e177_read(
                Path::new(env!("CARGO_MANIFEST_DIR"))
                    .join("docs/evidence/source-reader-followups-20261016/allocation.json"),
            )
            .unwrap(),
        )
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
            &main_e177_read(
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
            &main_e177_read(
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
            &main_e177_read(
                Path::new(env!("CARGO_MANIFEST_DIR"))
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
            &main_e177_read(
                Path::new(env!("CARGO_MANIFEST_DIR"))
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
            &main_e177_read(
                Path::new(env!("CARGO_MANIFEST_DIR"))
                    .join("docs/evidence/paid-expression-lookup-state-v1/views.json"),
            )
            .unwrap(),
        )
        .unwrap()
    })
}

// Complete predecessors for borrowed exit-region comparison and its test entry.
fn exit_region_matches() -> &'static [serde_json::Value] {
    static RECORDS: OnceLock<Vec<serde_json::Value>> = OnceLock::new();
    RECORDS.get_or_init(|| {
        serde_json::from_str(
            &main_e177_read(
                Path::new(env!("CARGO_MANIFEST_DIR"))
                    .join("docs/evidence/borrowed-exit-region-match-v1/views.json"),
            )
            .unwrap(),
        )
        .unwrap()
    })
}

fn proofs() -> &'static [serde_json::Value] {
    PROOF_RECORDS.get_or_init(|| {
        let mut original: Vec<serde_json::Value> = serde_json::from_str(
            &main_e177_read(
                Path::new(env!("CARGO_MANIFEST_DIR"))
                    .join("docs/evidence/receiver-source-custody-v1/inverse.json"),
            )
            .unwrap(),
        )
        .unwrap();
        let additional: Vec<serde_json::Value> = serde_json::from_str(
            &main_e177_read(
                Path::new(env!("CARGO_MANIFEST_DIR"))
                    .join("docs/evidence/live-source-audit-custody-v1/inverse.json"),
            )
            .unwrap(),
        )
        .unwrap();
        assert_eq!(original.len(), 12);
        assert_eq!(additional.len(), 2);
        original.extend(additional);
        original
    })
}

fn digest(source: &str) -> String {
    format!(
        "{:x}",
        crate::digest_hex::LowerHex(Sha256::digest(source.as_bytes()))
    )
}

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

pub(super) fn predecessor(path: &str, current: &str) -> Option<String> {
    if let Some(proof) = exit_region_matches()
        .iter()
        .find(|proof| proof["path"] == path)
    {
        return Some(invert(proof, current));
    }
    if let Some(proof) = receiving_lookups()
        .iter()
        .find(|proof| proof["path"] == path)
    {
        return Some(invert(proof, current));
    }
    if let Some(proof) = held_continuations()
        .iter()
        .find(|proof| proof["path"] == path)
    {
        return Some(invert(proof, current));
    }
    if let Some(proof) = held_status().iter().find(|proof| proof["path"] == path) {
        return Some(invert(proof, current));
    }
    if let Some(proof) = ordinary_setup()
        .iter()
        .chain(ordinary_helpers())
        .find(|proof| proof["path"] == path)
    {
        return Some(invert(proof, current));
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
        .chain(exit_region_matches())
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
        let current = main_e177_read(root.join(path)).unwrap();
        let before = invert(proof, &current);
        assert_eq!(
            before,
            main_e177_read(root.join(proof["archive"].as_str().unwrap())).unwrap()
        );
        assert!(contains(path));
        assert_eq!(predecessor(path, &current), Some(before));
        assert!(std::panic::catch_unwind(|| invert(proof, &(current.clone() + "\n"))).is_err());
        let mut forged = proof.clone();
        forged["before_sha256"] = serde_json::Value::String("0".repeat(64));
        assert!(std::panic::catch_unwind(|| invert(&forged, &current)).is_err());
    }
}

#[test]
fn ordinary_setup_complete_sources_and_original_legacy_constructor_remain_exact() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let evidence = root.join("docs/evidence/ordinary-replay-setup-v1");
    let rows: Vec<serde_json::Value> =
        serde_json::from_str(&main_e177_read(evidence.join("sources.json")).unwrap()).unwrap();
    assert_eq!(rows.len(), 34);
    let mut paths = std::collections::BTreeSet::new();
    for row in &rows {
        let path = row["path"].as_str().unwrap();
        assert!(paths.insert(path));
        let current = main_e177_read(root.join(path)).unwrap();
        let before = invert(row, &current);
        assert_eq!(
            before,
            main_e177_read(evidence.join(row["archive"].as_str().unwrap())).unwrap()
        );
        assert!(std::panic::catch_unwind(|| invert(row, &(current.clone() + "\n"))).is_err());
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
        let current = main_e177_read(root.join(path)).unwrap();
        let before = invert(proof, &current);
        assert_eq!(
            before,
            main_e177_read(root.join(proof["archive"].as_str().unwrap())).unwrap()
        );
        assert_eq!(
            digest(&restore(original_format, &before)),
            original_format["before_sha256"]
        );
        assert!(std::panic::catch_unwind(|| invert(proof, &(current.clone() + "\n"))).is_err());
    }
}

#[test]
fn every_receiver_inverse_preserves_the_entire_reflow_input_and_archive() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let mut paths = std::collections::BTreeSet::new();
    assert_eq!(proofs().len(), 14);
    for proof in proofs() {
        let path = proof["path"].as_str().unwrap();
        assert!(paths.insert(path));
        let repair = records().iter().find(|repair| repair["path"] == path);
        if let Some(repair) = repair {
            assert_eq!(proof["before_sha256"], repair["after_sha256"]);
        }
        let current = main_e177_read(root.join(path)).unwrap();
        let restored = invert(proof, &current);
        let source_audits: Vec<serde_json::Value> = serde_json::from_str(
            &main_e177_read(
                Path::new(env!("CARGO_MANIFEST_DIR"))
                    .join("docs/evidence/live-source-audit-custody-v1/inverse.json"),
            )
            .unwrap(),
        )
        .unwrap();
        let archive_root = if source_audits.iter().any(|audit| audit["path"] == path) {
            "docs/evidence/live-source-audit-custody-v1"
        } else {
            "docs/evidence/receiver-source-custody-v1"
        };
        let archive = root
            .join(archive_root)
            .join(proof["archive"].as_str().unwrap());
        assert_eq!(restored, main_e177_read(archive).unwrap());
        assert_ne!(current, restored);
        // Execute the unchanged formatting inverse and its whole-file digest.
        if let Some(repair) = repair {
            assert_ne!(restore(repair, &restored), restored);
        }
    }
    assert!(predecessor("src/source_reflows.rs", "unregistered source").is_none());
}

#[test]
fn receiver_mutation_and_forged_predecessor_fail_closed() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    for proof in proofs() {
        let current = main_e177_read(root.join(proof["path"].as_str().unwrap())).unwrap();
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
        let current = main_e177_read(root.join(path)).unwrap();
        let restored = invert(proof, &current);
        let archive = root
            .join("docs/evidence/source-reader-followups-20261016")
            .join(proof["archive"].as_str().unwrap());
        assert_eq!(restored, main_e177_read(archive).unwrap());
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
        serde_json::from_str(&main_e177_read(evidence.join("sources.json")).unwrap()).unwrap();
    for record in records.as_array().unwrap() {
        let original = main_e177_read(evidence.join(record["archive"].as_str().unwrap())).unwrap();
        assert_eq!(digest(&original), record["sha256"]);
        let current = main_e177_read(root.join(record["path"].as_str().unwrap())).unwrap();
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
        let current = main_e177_read(root.join(path)).unwrap();
        let before = invert(proof, &current);
        assert_eq!(
            before,
            main_e177_read(root.join(proof["archive"].as_str().unwrap())).unwrap()
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
        &main_e177_read(root.join("docs/evidence/held-status-prefix-v1/sources.json")).unwrap(),
    )
    .unwrap();
    assert_eq!(whole.len(), 6);
    for proof in &whole {
        let current = main_e177_read(root.join(proof["path"].as_str().unwrap())).unwrap();
        let before = invert(proof, &current);
        assert_eq!(
            before,
            main_e177_read(root.join(proof["archive"].as_str().unwrap())).unwrap()
        );
        assert!(std::panic::catch_unwind(|| invert(proof, &(current.clone() + "\n"))).is_err());
    }
    let proof = held_status()
        .iter()
        .find(|p| p["path"] == "src/cleanup_plan/replay/expression_facts/receiving/retained.rs")
        .unwrap();
    let before = main_e177_read(root.join(proof["archive"].as_str().unwrap())).unwrap();
    let current = main_e177_read(root.join(proof["path"].as_str().unwrap())).unwrap();
    let current = crate::source_reflows_merge::main99f_inputs::held_status_before_freeze(&current);
    // Reconstruct the authenticated predecessor dispatch before applying the
    // original complete-validator assertions below.
    let receiving_guard = "if function.cleanup_plan.status_sources.len() >= 3 {";
    assert_eq!(current.matches(receiving_guard).count(), 1);
    let current = current.replacen(
        receiving_guard,
        "if budget.pays_allocations() && function.cleanup_plan.status_sources.len() >= 3 {",
        1,
    );
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
        let current = main_e177_read(root.join(path)).unwrap();
        let before = invert(proof, &current);
        assert_eq!(
            before,
            main_e177_read(root.join(proof["archive"].as_str().unwrap())).unwrap()
        );
        assert!(contains(path));
        assert_eq!(predecessor(path, &current), Some(before));
        assert!(std::panic::catch_unwind(|| invert(proof, &(current.clone() + "\n"))).is_err());
        let mut forged = proof.clone();
        forged["before_sha256"] = serde_json::Value::String("0".repeat(64));
        assert!(std::panic::catch_unwind(|| invert(&forged, &current)).is_err());
    }
    let whole: Vec<serde_json::Value> = serde_json::from_str(
        &main_e177_read(root.join("docs/evidence/held-continuation-growth-v1/sources.json"))
            .unwrap(),
    )
    .unwrap();
    assert_eq!(whole.len(), 10);
    for proof in &whole {
        let current = main_e177_read(root.join(proof["path"].as_str().unwrap())).unwrap();
        let before = invert(proof, &current);
        assert_eq!(
            before,
            main_e177_read(root.join(proof["archive"].as_str().unwrap())).unwrap()
        );
        assert!(std::panic::catch_unwind(|| invert(proof, &(current.clone() + "\n"))).is_err());
    }
    let before_file = |path: &str| {
        let proof = whole.iter().find(|p| p["path"] == path).unwrap();
        main_e177_read(root.join(proof["archive"].as_str().unwrap())).unwrap()
    };
    let old_tests = before_file("src/cleanup_plan/replay/continuation_storage/tests.rs");
    let current_tests =
        main_e177_read(root.join("src/cleanup_plan/replay/continuation_storage/tests.rs")).unwrap();
    assert!(current_tests.starts_with(&old_tests));
    let old_storage = before_file("src/cleanup_plan/replay/continuation_storage.rs");
    let current_storage =
        main_e177_read(root.join("src/cleanup_plan/replay/continuation_storage.rs")).unwrap();
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
    let mut machine =
        before_last_branch_root(&main_e177_read(root.join("src/cleanup_plan/replay.rs")).unwrap());
    let standalone = "if work.budget.payer == AllocationPayer::ActiveBuilder {\n                continuation_storage::fresh_receiving(function, work.budget)?\n            } else {\n                continuation_storage::fresh(function, work.budget)?\n            }";
    assert_eq!(machine.matches(standalone).count(), 1);
    machine = machine.replacen(
        standalone,
        "continuation_storage::fresh(function, work.budget)?",
        1,
    );
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
        let current = main_e177_read(root.join(path)).unwrap();
        let before = invert(proof, &current);
        assert_eq!(
            before,
            main_e177_read(root.join(proof["archive"].as_str().unwrap())).unwrap()
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
        &main_e177_read(root.join("docs/evidence/paid-expression-lookup-state-v1/sources.json"))
            .unwrap(),
    )
    .unwrap();
    assert_eq!(whole.len(), 10);
    for proof in &whole {
        let current = main_e177_read(root.join(proof["path"].as_str().unwrap())).unwrap();
        let before = invert(proof, &current);
        assert_eq!(
            before,
            main_e177_read(root.join(proof["archive"].as_str().unwrap())).unwrap()
        );
        assert!(std::panic::catch_unwind(|| invert(proof, &(current.clone() + "\n"))).is_err());
    }
    let before_file = |path: &str| {
        let proof = whole.iter().find(|p| p["path"] == path).unwrap();
        main_e177_read(root.join(proof["archive"].as_str().unwrap())).unwrap()
    };
    for path in [
        "src/cleanup_plan/replay/expression_facts/arena/prefix.rs",
        "src/cleanup_plan/replay/expression_facts/arena/prefix/tests.rs",
    ] {
        assert!(main_e177_read(root.join(path))
            .unwrap()
            .starts_with(&before_file(path)));
    }
    let path = "src/cleanup_plan/replay/expression_facts/arena.rs";
    let current = main_e177_read(root.join(path)).unwrap();
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
    let current = main_e177_read(root.join(path)).unwrap();
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
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let records: Vec<serde_json::Value> = serde_json::from_str(
        &main_e177_read(root.join("docs/evidence/ordinary-replay-routes-v1/sources.json")).unwrap(),
    )
    .unwrap();
    assert_eq!(records.len(), 6);
    let mut paths = std::collections::BTreeSet::new();
    for proof in records {
        let path = proof["path"].as_str().unwrap();
        assert!(paths.insert(path.to_owned()));
        let current = main_e177_read(root.join(path)).unwrap();
        let before = invert(&proof, &current);
        assert_eq!(
            before,
            main_e177_read(root.join(proof["archive"].as_str().unwrap())).unwrap()
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

#[test]
fn borrowed_exit_region_views_preserve_all_complete_predecessors_and_controls() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    assert_eq!(exit_region_matches().len(), 5);
    let mut paths = std::collections::BTreeSet::new();
    for proof in exit_region_matches() {
        let path = proof["path"].as_str().unwrap();
        assert!(paths.insert(path));
        assert!(!proofs()
            .iter()
            .chain(followups())
            .chain(ordinary_setup())
            .chain(ordinary_helpers())
            .chain(held_status())
            .chain(held_continuations())
            .chain(receiving_lookups())
            .any(|old| old["path"] == path));
        let current = main_e177_read(root.join(path)).unwrap();
        let before = invert(proof, &current);
        assert_eq!(
            before,
            main_e177_read(root.join(proof["archive"].as_str().unwrap())).unwrap()
        );
        assert_eq!(predecessor(path, &current), Some(before.clone()));
        assert!(contains(path));
        if let Some(format) = records().iter().find(|record| record["path"] == path) {
            assert_eq!(proof["before_sha256"], format["after_sha256"]);
            assert_eq!(digest(&restore(format, &before)), format["before_sha256"]);
        }
        if path.ends_with("helpers/tests.rs") {
            assert_eq!(
                current.replacen("pub(super) fn program()", "fn program()", 1),
                before
            );
        }
        assert!(std::panic::catch_unwind(|| invert(proof, &(current.clone() + "\n"))).is_err());
        let mut forged = proof.clone();
        forged["before_sha256"] = serde_json::Value::String("0".repeat(64));
        assert!(std::panic::catch_unwind(|| invert(&forged, &current)).is_err());
    }
    let evidence = root.join("docs/evidence/borrowed-exit-region-match-v1");
    let sources: Vec<serde_json::Value> =
        serde_json::from_str(&main_e177_read(evidence.join("sources.json")).unwrap()).unwrap();
    assert_eq!(sources.len(), 7);
    for proof in sources {
        let current = main_e177_read(root.join(proof["path"].as_str().unwrap())).unwrap();
        assert_eq!(
            invert(&proof, &current),
            main_e177_read(root.join(proof["archive"].as_str().unwrap())).unwrap()
        );
    }
    for manifest in ["new-files.json", "retained-sources.json"] {
        let records: Vec<serde_json::Value> =
            serde_json::from_str(&main_e177_read(evidence.join(manifest)).unwrap()).unwrap();
        assert_eq!(records.len(), 2);
        for record in records {
            let current = main_e177_read(root.join(record["path"].as_str().unwrap())).unwrap();
            let current = if record["path"] == "src/cleanup_plan/replay.rs" {
                before_last_branch_root(&current)
            } else {
                current
            };
            assert_eq!(digest(&current), record["sha256"]);
            assert_ne!(digest(&(current + "\n")), record["sha256"]);
        }
    }
}

#[test]
fn slot_byte_helper_and_generic_bound_frontier_keep_complete_source_receivers() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let evidence = root.join("docs/evidence/string-slot-byte-helper-v1");
    let sources: Vec<serde_json::Value> =
        serde_json::from_str(&main_e177_read(evidence.join("sources.json")).unwrap()).unwrap();
    assert_eq!(sources.len(), 9);
    let mut paths = std::collections::BTreeSet::new();
    for proof in &sources {
        let path = proof["path"].as_str().unwrap();
        assert!(paths.insert(path));
        let current = main_e177_read(root.join(path)).unwrap();
        let before = invert(proof, &current);
        assert_eq!(
            before,
            main_e177_read(root.join(proof["archive"].as_str().unwrap())).unwrap()
        );
        assert!(std::panic::catch_unwind(|| invert(proof, &(current.clone() + "\n"))).is_err());
        let mut forged = proof.clone();
        forged["before_sha256"] = serde_json::Value::String("0".repeat(64));
        assert!(std::panic::catch_unwind(|| invert(&forged, &current)).is_err());
        if path.ends_with("byte_writer/tests.rs") {
            assert_eq!(
                current.strip_suffix("\nmod string_slots;\n").unwrap(),
                before
            );
        }
    }
    let new_files: Vec<serde_json::Value> =
        serde_json::from_str(&main_e177_read(evidence.join("new-files.json")).unwrap()).unwrap();
    assert_eq!(new_files.len(), 1);
    for file in new_files {
        let current = main_e177_read(root.join(file["path"].as_str().unwrap())).unwrap();
        assert_eq!(digest(&current), file["sha256"]);
        assert_ne!(digest(&(current + "\n")), file["sha256"]);
    }
}

fn before_last_branch_root(current: &str) -> String {
    let sources: Vec<serde_json::Value> = serde_json::from_str(
        &main_e177_read(
            Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("docs/evidence/last-branch-prefix-v1/sources.json"),
        )
        .unwrap(),
    )
    .unwrap();
    let proof = sources
        .iter()
        .find(|proof| proof["path"] == "src/cleanup_plan/replay.rs")
        .unwrap();
    invert(proof, current)
}

#[test]
fn last_branch_prefix_keeps_complete_predecessors_and_original_source_controls() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let sources: Vec<serde_json::Value> = serde_json::from_str(
        &main_e177_read(root.join("docs/evidence/last-branch-prefix-v1/sources.json")).unwrap(),
    )
    .unwrap();
    assert_eq!(sources.len(), 4);
    let mut paths = std::collections::BTreeSet::new();
    for proof in &sources {
        let path = proof["path"].as_str().unwrap();
        assert!(paths.insert(path));
        let current = main_e177_read(root.join(path)).unwrap();
        let before = invert(proof, &current);
        assert_eq!(
            before,
            main_e177_read(root.join(proof["archive"].as_str().unwrap())).unwrap()
        );
        assert!(std::panic::catch_unwind(|| invert(proof, &(current.clone() + "\n"))).is_err());
        let mut forged = proof.clone();
        forged["before_sha256"] = serde_json::Value::String("0".repeat(64));
        assert!(std::panic::catch_unwind(|| invert(&forged, &current)).is_err());
        if path.ends_with("branch_clone/tests.rs") {
            assert!(current.starts_with(&before));
        }
    }
    let helper = main_e177_read(root.join("src/cleanup_plan/replay/branch_clone.rs")).unwrap();
    let proof = sources
        .iter()
        .find(|proof| proof["path"] == "src/cleanup_plan/replay/branch_clone.rs")
        .unwrap();
    let before = invert(proof, &helper);
    assert_eq!(
        helper.split("/// A pure probe").next().unwrap(),
        before.split("#[cfg(test)]\nmod tests;").next().unwrap()
    );
}

#[test]
fn plan_branch_capacity_preserves_complete_sources_and_all_existing_controls() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let sources: Vec<serde_json::Value> = serde_json::from_str(
        &main_e177_read(root.join("docs/evidence/plan-branch-capacity-v1/sources.json")).unwrap(),
    )
    .unwrap();
    assert_eq!(sources.len(), 5);
    let mut paths = std::collections::BTreeSet::new();
    for proof in &sources {
        let path = proof["path"].as_str().unwrap();
        assert!(paths.insert(path));
        let current = main_e177_read(root.join(path)).unwrap();
        let before = invert(proof, &current);
        assert_eq!(
            before,
            main_e177_read(root.join(proof["archive"].as_str().unwrap())).unwrap()
        );
        assert!(std::panic::catch_unwind(|| invert(proof, &(current.clone() + "\n"))).is_err());
        let mut forged = proof.clone();
        forged["before_sha256"] = serde_json::Value::String("0".repeat(64));
        assert!(std::panic::catch_unwind(|| invert(&forged, &current)).is_err());
        if path.ends_with("branch_clone/tests.rs") {
            assert!(current.starts_with(&before));
        }
        if path.ends_with("branch_clone.rs") {
            assert_eq!(
                current.split("/// A pure probe").next().unwrap(),
                before.split("/// A pure probe").next().unwrap()
            );
        }
    }
}

#[test]
fn replay_work_refusal_capture_retains_all_complete_predecessors_and_assertions() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let sources: Vec<serde_json::Value> = serde_json::from_str(
        &main_e177_read(root.join("docs/evidence/replay-work-refusal-v1/sources.json")).unwrap(),
    )
    .unwrap();
    assert_eq!(sources.len(), 4);
    let mut paths = std::collections::BTreeSet::new();
    for proof in &sources {
        let path = proof["path"].as_str().unwrap();
        assert!(paths.insert(path));
        let current = main_e177_read(root.join(path)).unwrap();
        let before = invert(proof, &current);
        assert_eq!(
            before,
            main_e177_read(root.join(proof["archive"].as_str().unwrap())).unwrap()
        );
        assert!(std::panic::catch_unwind(|| invert(proof, &(current.clone() + "\n"))).is_err());
        let mut forged = proof.clone();
        forged["before_sha256"] = serde_json::Value::String("0".repeat(64));
        assert!(std::panic::catch_unwind(|| invert(&forged, &current)).is_err());
    }
}

#[test]
fn held_status_index_keeps_complete_predecessor_receivers_and_assertions() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let sources: Vec<serde_json::Value> = serde_json::from_str(
        &main_e177_read(root.join("docs/evidence/status-identity-index-v1/sources.json")).unwrap(),
    )
    .unwrap();
    assert_eq!(sources.len(), 3);
    let mut paths = std::collections::BTreeSet::new();
    for proof in &sources {
        let path = proof["path"].as_str().unwrap();
        assert!(paths.insert(path));
        let current = main_e177_read(root.join(path)).unwrap();
        let before = invert(proof, &current);
        assert_eq!(
            before,
            main_e177_read(root.join(proof["archive"].as_str().unwrap())).unwrap()
        );
        assert!(std::panic::catch_unwind(|| invert(proof, &(current.clone() + "\n"))).is_err());
        let mut forged = proof.clone();
        forged["before_sha256"] = serde_json::Value::String("0".repeat(64));
        assert!(std::panic::catch_unwind(|| invert(&forged, &current)).is_err());
    }
}

#[test]
fn projection_audit_scope_preserves_complete_sources_and_all_historical_receivers() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let sources: Vec<serde_json::Value> = serde_json::from_str(
        &main_e177_read(root.join("docs/evidence/projection-audit-scope-v1/sources.json")).unwrap(),
    )
    .unwrap();
    assert_eq!(sources.len(), 5);
    let mut paths = std::collections::BTreeSet::new();
    for proof in &sources {
        let path = proof["path"].as_str().unwrap();
        assert!(paths.insert(path));
        let current = main_e177_read(root.join(path)).unwrap();
        let before = invert(proof, &current);
        assert_eq!(
            before,
            main_e177_read(root.join(proof["archive"].as_str().unwrap())).unwrap()
        );
        assert!(std::panic::catch_unwind(|| invert(proof, &(current.clone() + "\n"))).is_err());
        let mut forged = proof.clone();
        forged["before_sha256"] = serde_json::Value::String("0".repeat(64));
        assert!(std::panic::catch_unwind(|| invert(&forged, &current)).is_err());
    }
    let sealed = main_e177_read(
        root.join("docs/evidence/projection-audit-scope-v1/touched-live-inverses.before.json"),
    )
    .unwrap();
    assert_eq!(
        digest(&sealed),
        "c9e1e2dc687a910bc8527e31f15284774ed999104126aa1b0b52cd3f98495f37"
    );
    let sealed: serde_json::Value = serde_json::from_str(&sealed).unwrap();
    for (packet, old_rows) in sealed.as_object().unwrap() {
        let live: Vec<serde_json::Value> =
            serde_json::from_str(&main_e177_read(root.join(packet)).unwrap()).unwrap();
        for old in old_rows.as_array().unwrap() {
            let path = old["path"].as_str().unwrap();
            let row = live.iter().find(|row| row["path"] == path).unwrap();
            assert_eq!(row["sha256"], old["sha256"]);
            assert_eq!(row["archive"], old["archive"]);
            let old_edits = old["edits"].as_array().unwrap();
            let edits = row["edits"].as_array().unwrap();
            assert_eq!(&edits[..old_edits.len()], old_edits.as_slice());
            let (before, after) = if old_edits[0].get("after").is_some() {
                ("before", "after")
            } else {
                ("old", "new")
            };
            let mut restored = main_e177_read(root.join(path)).unwrap();
            for edit in edits.iter().rev() {
                let receiving = edit[after].as_str().unwrap();
                assert_eq!(restored.matches(receiving).count(), 1);
                restored = restored.replacen(receiving, edit[before].as_str().unwrap(), 1);
            }
            let relative = old["archive"].as_str().unwrap();
            let direct = root.join(relative);
            let archive = if direct.is_file() {
                direct
            } else {
                root.join(packet).parent().unwrap().join(relative)
            };
            let original = main_e177_read(archive).unwrap();
            assert_eq!(digest(&original), old["sha256"]);
            assert_eq!(restored, original);
        }
    }
}

#[test]
fn historical_pin_capture_chains_restore_every_original_source() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let evidence = root.join("docs/evidence/legacy-pin-chain-repair-v1");
    let before = main_e177_read(evidence.join("semantic_receivers.rs.before.txt")).unwrap();
    assert_eq!(
        digest(&before),
        "6af473f412874b7c2e09e4e1effa1f18357f74cf8e31b7cab4a16bd85b4a947d"
    );
    assert!(
        main_e177_read(root.join("src/source_reflows/semantic_receivers.rs"))
            .unwrap()
            .starts_with(&before)
    );
    let sealed = main_e177_read(evidence.join("rows.before.json")).unwrap();
    assert_eq!(
        digest(&sealed),
        "7f69e5a087703aa6ab433c52bf7fe5ffd595a5ff4b208d4bb6357fdc6703c5d3"
    );
    let sealed: serde_json::Value = serde_json::from_str(&sealed).unwrap();
    assert_eq!(sealed.as_object().unwrap().len(), 2);
    for (packet, count) in [
        (
            "docs/evidence/prelude-forecast-286/exact-source-inverses.json",
            5,
        ),
        (
            "docs/evidence/prelude-forecast-286/pin-capture/exact-source-inverses.json",
            2,
        ),
        (
            "docs/evidence/prelude-fixture-source-guard-4ec/exact-source-inverses.json",
            1,
        ),
        (
            "docs/evidence/prelude-forecast-actual-pins-a184d-20261015/exact-source-inverses.json",
            2,
        ),
        (
            "docs/evidence/source-reader-followups-20261016/sources.json",
            3,
        ),
    ] {
        let rows: Vec<serde_json::Value> =
            serde_json::from_str(&main_e177_read(root.join(packet)).unwrap()).unwrap();
        assert_eq!(rows.len(), count);
        let mut paths = std::collections::BTreeSet::new();
        for row in &rows {
            let path = row["path"].as_str().unwrap();
            assert!(paths.insert(path));
            let edits = row["edits"].as_array().unwrap();
            assert!(!edits.is_empty());
            if let Some(old) = sealed.get(packet).filter(|old| old["path"] == path) {
                assert_eq!(row["archive"], old["archive"]);
                assert_eq!(row["sha256"], old["sha256"]);
                let original_edits = old["edits"].as_array().unwrap();
                assert_eq!(&edits[..original_edits.len()], original_edits.as_slice());
            }
            let (before, after) = if edits[0].get("after").is_some() {
                ("before", "after")
            } else {
                ("old", "new")
            };
            let mut restored = main_e177_read(root.join(path)).unwrap();
            for edit in edits.iter().rev() {
                let receiving = edit[after].as_str().unwrap();
                assert!(!receiving.is_empty());
                assert_eq!(restored.matches(receiving).count(), 1, "{packet}: {path}");
                restored = restored.replacen(receiving, edit[before].as_str().unwrap(), 1);
            }
            let relative = row["archive"].as_str().unwrap();
            let direct = root.join(relative);
            let archive = if direct.is_file() {
                direct
            } else {
                root.join(packet).parent().unwrap().join(relative)
            };
            let original = main_e177_read(archive).unwrap();
            assert_eq!(digest(&original), row["sha256"]);
            assert_eq!(restored, original);
        }
    }
}

#[test]
fn status_membership_index_preserves_complete_predecessors_and_all_original_controllers() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let proofs: Vec<serde_json::Value> = serde_json::from_str(
        &main_e177_read(root.join("docs/evidence/status-membership-index-v1/sources.json"))
            .unwrap(),
    )
    .unwrap();
    assert_eq!(proofs.len(), 5);
    let mut paths = std::collections::BTreeSet::new();
    for proof in &proofs {
        let path = proof["path"].as_str().unwrap();
        assert!(paths.insert(path));
        let current = main_e177_read(root.join(path)).unwrap();
        let before = invert(proof, &current);
        assert_eq!(
            before,
            main_e177_read(root.join(proof["archive"].as_str().unwrap())).unwrap()
        );
        assert!(std::panic::catch_unwind(|| invert(proof, &(current.clone() + "\n"))).is_err());
        let mut forged = proof.clone();
        forged["before_sha256"] = serde_json::Value::String("0".repeat(64));
        assert!(std::panic::catch_unwind(|| invert(&forged, &current)).is_err());
    }
}

#[test]
fn replay_operation_totals_preserve_complete_predecessors_and_production_text() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let proofs: Vec<serde_json::Value> = serde_json::from_str(
        &main_e177_read(root.join("docs/evidence/replay-operation-totals-v1/sources.json"))
            .unwrap(),
    )
    .unwrap();
    assert_eq!(proofs.len(), 10);
    let mut paths = std::collections::BTreeSet::new();
    for proof in &proofs {
        let path = proof["path"].as_str().unwrap();
        assert!(paths.insert(path));
        let current = main_e177_read(root.join(path)).unwrap();
        let before = invert(proof, &current);
        assert_eq!(
            before,
            main_e177_read(root.join(proof["archive"].as_str().unwrap())).unwrap()
        );
        assert!(std::panic::catch_unwind(|| invert(proof, &(current.clone() + "\n"))).is_err());
        let mut forged = proof.clone();
        forged["before_sha256"] = serde_json::Value::String("0".repeat(64));
        assert!(std::panic::catch_unwind(|| invert(&forged, &current)).is_err());
    }
    let guards: Vec<serde_json::Value> = serde_json::from_str(
        &main_e177_read(root.join("docs/evidence/replay-operation-totals-v1/guards.json")).unwrap(),
    )
    .unwrap();
    assert_eq!(guards.len(), 8);
    for path in guards
        .iter()
        .map(|row| row["path"].as_str().unwrap())
        .collect::<std::collections::BTreeSet<_>>()
    {
        let mut current = main_e177_read(root.join(path)).unwrap();
        for guard in guards.iter().filter(|row| row["path"] == path) {
            let text = guard["statement"].as_str().unwrap();
            assert!(text.contains("#[cfg(test)]"));
            assert_eq!(current.matches(text).count(), 1);
            current = current.replacen(text, "", 1);
        }
        let proof = proofs.iter().find(|proof| proof["path"] == path).unwrap();
        assert_eq!(
            current,
            main_e177_read(root.join(proof["archive"].as_str().unwrap())).unwrap()
        );
    }
}

/// The original source-work assertion still reads its complete prior input.
/// The separate custody control below authenticates all current changed files.
pub(crate) fn helper_digest_capture_predecessor(current: &str) -> String {
    let authenticated = crate::source_reflows_merge::main_e177_inputs::expected(
        "src/workspace_graph/static_text/scalar_helpers.rs",
        current,
    );
    let current = authenticated.as_str();
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let proofs: Vec<serde_json::Value> = serde_json::from_str(
        &main_e177_read(root.join("docs/evidence/optional-helper-digest-work-v1/sources.json"))
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
        main_e177_read(root.join(proof["archive"].as_str().unwrap())).unwrap()
    );
    before
}

#[test]
fn optional_helper_digest_preserves_complete_predecessors_and_all_old_controls() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let proofs: Vec<serde_json::Value> = serde_json::from_str(
        &main_e177_read(root.join("docs/evidence/optional-helper-digest-work-v1/sources.json"))
            .unwrap(),
    )
    .unwrap();
    assert_eq!(proofs.len(), 6);
    let mut paths = std::collections::BTreeSet::new();
    for proof in &proofs {
        let path = proof["path"].as_str().unwrap();
        assert!(paths.insert(path));
        let current = main_e177_read(root.join(path)).unwrap();
        let before = invert(proof, &current);
        assert_eq!(
            before,
            main_e177_read(root.join(proof["archive"].as_str().unwrap())).unwrap()
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
        &main_e177_read(root.join("docs/evidence/isolated-skeleton-paths-v1/sources.json"))
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
        let current = main_e177_read(root.join(path)).unwrap();
        let before = invert(proof, &current);
        assert_eq!(
            before,
            main_e177_read(root.join(proof["archive"].as_str().unwrap())).unwrap()
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
        main_e177_read(
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
    let authority = main_e177_read(root.join("tests/hir_module_boundaries.rs")).unwrap();
    for path in created {
        assert_eq!(authority.matches(&format!("\"../{path}\"")).count(), 1);
    }
}

#[test]
fn core_attempt_receipts_preserve_all_original_retry_and_source_controls() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let proofs: Vec<serde_json::Value> = serde_json::from_str(
        &main_e177_read(root.join("docs/evidence/core-attempt-receipts-v1/sources.json")).unwrap(),
    )
    .unwrap();
    assert_eq!(proofs.len(), 5);
    let mut paths = std::collections::BTreeSet::new();
    for proof in &proofs {
        let path = proof["path"].as_str().unwrap();
        assert!(paths.insert(path));
        let current = main_e177_read(root.join(path)).unwrap();
        let before = invert(proof, &current);
        assert_eq!(
            before,
            main_e177_read(root.join(proof["archive"].as_str().unwrap())).unwrap()
        );
        assert!(std::panic::catch_unwind(|| invert(proof, &(current.clone() + "\n"))).is_err());
        let mut forged = proof.clone();
        forged["before_sha256"] = serde_json::Value::String("0".repeat(64));
        assert!(std::panic::catch_unwind(|| invert(&forged, &current)).is_err());
        match path {
            "src/workspace_graph.rs" => {
                let hook = "        #[cfg(test)]\n        tests::record_core_attempt(\n            fallback_mode,\n            resolve_builder_bytes,\n            consumed,\n            overflowed,\n            &refusal,\n            &site,\n            &core.as_ref().err(),\n        );\n";
                assert_eq!(current.matches(hook).count(), 1);
                assert!(hook.contains("#[cfg(test)]"));
                assert_eq!(current.replacen(hook, "", 1), before);
            }
            "src/workspace_graph/tests.rs" => {
                assert_eq!(current.strip_prefix("#[path = \"tests/core_attempt_trace.rs\"]\nmod core_attempt_trace;\npub(super) use core_attempt_trace::{\n    observe as observe_core_attempts, record as record_core_attempt,\n};\n").unwrap(), before);
            }
            "src/workspace_graph/tests/builder_limits.rs" => {
                let hook = "    let built_result = observe_core_attempts(|| {\n        build_owned_with_builder_limit(sources.clone(), MAX_BUILDER_BYTES)\n    });";
                assert_eq!(current.matches(hook).count(), 1);
                assert_eq!(current.replacen(hook, "    let built_result = build_owned_with_builder_limit(sources.clone(), MAX_BUILDER_BYTES);", 1), before);
                assert!(current.contains("assert_eq!(CORE_BUILD_ATTEMPTS.with(Cell::get), 2);"));
            }
            "src/source_reflows/semantic_receivers.rs" => assert!(current.starts_with(&before)),
            "src/workspace_graph/tests/core_attempt_trace.rs" => assert!(before.is_empty()),
            _ => panic!("unexpected receipt source"),
        }
    }
}

#[test]
fn tail_merger_helper_name_preserves_both_complete_reference_bodies_and_all_assertions() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let proofs: Vec<serde_json::Value> = serde_json::from_str(
        &main_e177_read(root.join("docs/evidence/tail-merge-helper-name-v1/sources.json")).unwrap(),
    )
    .unwrap();
    assert_eq!(proofs.len(), 3);
    let mut paths = std::collections::BTreeSet::new();
    for proof in &proofs {
        let path = proof["path"].as_str().unwrap();
        assert!(paths.insert(path));
        let current = main_e177_read(root.join(path)).unwrap();
        let before = invert(proof, &current);
        assert_eq!(
            before,
            main_e177_read(root.join(proof["archive"].as_str().unwrap())).unwrap()
        );
        assert!(std::panic::catch_unwind(|| invert(proof, &(current.clone() + "\n"))).is_err());
        let mut forged = proof.clone();
        forged["before_sha256"] = serde_json::Value::String("0".repeat(64));
        assert!(std::panic::catch_unwind(|| invert(&forged, &current)).is_err());
        if path == "src/source_reflows/semantic_receivers.rs" {
            assert!(current.starts_with(&before));
        } else {
            let old = "saved_merged_paths";
            let new = "tail_merge_control";
            let count = if path.ends_with("/old.rs") { 1 } else { 4 };
            assert_eq!(current.matches(new).count(), count);
            assert!(!current.contains(old));
            assert_eq!(current.replace(new, old), before);
        }
    }
}

#[test]
fn reused_status_cursor_preserves_complete_predecessors_and_all_original_receivers() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let proofs: Vec<serde_json::Value> = serde_json::from_str(
        &main_e177_read(root.join("docs/evidence/status-cursor-reuse-v1/sources.json")).unwrap(),
    )
    .unwrap();
    assert_eq!(proofs.len(), 5);
    let mut paths = std::collections::BTreeSet::new();
    for proof in &proofs {
        let path = proof["path"].as_str().unwrap();
        assert!(paths.insert(path));
        let current = main_e177_read(root.join(path)).unwrap();
        let before = invert(proof, &current);
        assert_eq!(
            before,
            main_e177_read(root.join(proof["archive"].as_str().unwrap())).unwrap()
        );
        assert!(std::panic::catch_unwind(|| invert(proof, &(current.clone() + "\n"))).is_err());
        let mut forged = proof.clone();
        forged["before_sha256"] = serde_json::Value::String("0".repeat(64));
        assert!(std::panic::catch_unwind(|| invert(&forged, &current)).is_err());
    }
}

#[test]
fn public_wire_receipts_preserve_complete_sources_and_every_original_assertion() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let proofs: Vec<serde_json::Value> = serde_json::from_str(
        &main_e177_read(root.join("docs/evidence/public-wire-receipts-v1/sources.json")).unwrap(),
    )
    .unwrap();
    assert_eq!(proofs.len(), 6);
    let mut paths = std::collections::BTreeSet::new();
    for proof in &proofs {
        let path = proof["path"].as_str().unwrap();
        assert!(paths.insert(path));
        let current = main_e177_read(root.join(path)).unwrap();
        let before = invert(proof, &current);
        assert_eq!(
            before,
            main_e177_read(root.join(proof["archive"].as_str().unwrap())).unwrap()
        );
        assert!(std::panic::catch_unwind(|| invert(proof, &(current.clone() + "\n"))).is_err());
        let mut forged = proof.clone();
        forged["before_sha256"] = serde_json::Value::String("0".repeat(64));
        assert!(std::panic::catch_unwind(|| invert(&forged, &current)).is_err());
    }
}
