//! Exact custody for later receiver changes, separate from CI reformatting.
//! These snapshots are used only by historical source assertions. Live source
//! inventories continue to include the current modules and their new children.
use super::*;

const PROOFS: &str = include_str!("../../docs/evidence/receiver-source-custody-v1/inverse.json");
const LIVE_AUDITS: &str =
    include_str!("../../docs/evidence/live-source-audit-custody-v1/inverse.json");
static PROOF_RECORDS: OnceLock<Vec<serde_json::Value>> = OnceLock::new();

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
            &std::fs::read_to_string(
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
            &std::fs::read_to_string(
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
            &std::fs::read_to_string(
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
            &std::fs::read_to_string(
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
        let mut original: Vec<serde_json::Value> = serde_json::from_str(PROOFS).unwrap();
        let additional: Vec<serde_json::Value> = serde_json::from_str(LIVE_AUDITS).unwrap();
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
        let current = std::fs::read_to_string(root.join(path)).unwrap();
        let before = invert(proof, &current);
        assert_eq!(
            before,
            std::fs::read_to_string(root.join(proof["archive"].as_str().unwrap())).unwrap()
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
        serde_json::from_str(&std::fs::read_to_string(evidence.join("sources.json")).unwrap())
            .unwrap();
    assert_eq!(rows.len(), 34);
    let mut paths = std::collections::BTreeSet::new();
    for row in &rows {
        let path = row["path"].as_str().unwrap();
        assert!(paths.insert(path));
        let current = std::fs::read_to_string(root.join(path)).unwrap();
        let before = invert(row, &current);
        assert_eq!(
            before,
            std::fs::read_to_string(evidence.join(row["archive"].as_str().unwrap())).unwrap()
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
        let current = std::fs::read_to_string(root.join(path)).unwrap();
        let before = invert(proof, &current);
        assert_eq!(
            before,
            std::fs::read_to_string(root.join(proof["archive"].as_str().unwrap())).unwrap()
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
        let current = std::fs::read_to_string(root.join(path)).unwrap();
        let restored = invert(proof, &current);
        let source_audits: Vec<serde_json::Value> = serde_json::from_str(LIVE_AUDITS).unwrap();
        let archive_root = if source_audits.iter().any(|audit| audit["path"] == path) {
            "docs/evidence/live-source-audit-custody-v1"
        } else {
            "docs/evidence/receiver-source-custody-v1"
        };
        let archive = root
            .join(archive_root)
            .join(proof["archive"].as_str().unwrap());
        assert_eq!(restored, std::fs::read_to_string(archive).unwrap());
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
        let current = std::fs::read_to_string(root.join(path)).unwrap();
        let before = invert(proof, &current);
        assert_eq!(
            before,
            std::fs::read_to_string(root.join(proof["archive"].as_str().unwrap())).unwrap()
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
        &std::fs::read_to_string(root.join("docs/evidence/held-status-prefix-v1/sources.json"))
            .unwrap(),
    )
    .unwrap();
    assert_eq!(whole.len(), 6);
    for proof in &whole {
        let current = std::fs::read_to_string(root.join(proof["path"].as_str().unwrap())).unwrap();
        let before = invert(proof, &current);
        assert_eq!(
            before,
            std::fs::read_to_string(root.join(proof["archive"].as_str().unwrap())).unwrap()
        );
        assert!(std::panic::catch_unwind(|| invert(proof, &(current.clone() + "\n"))).is_err());
    }
    let proof = held_status()
        .iter()
        .find(|p| p["path"] == "src/cleanup_plan/replay/expression_facts/receiving/retained.rs")
        .unwrap();
    let before = std::fs::read_to_string(root.join(proof["archive"].as_str().unwrap())).unwrap();
    let current = std::fs::read_to_string(root.join(proof["path"].as_str().unwrap())).unwrap();
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
        let current = std::fs::read_to_string(root.join(path)).unwrap();
        let before = invert(proof, &current);
        assert_eq!(
            before,
            std::fs::read_to_string(root.join(proof["archive"].as_str().unwrap())).unwrap()
        );
        assert!(contains(path));
        assert_eq!(predecessor(path, &current), Some(before));
        assert!(std::panic::catch_unwind(|| invert(proof, &(current.clone() + "\n"))).is_err());
        let mut forged = proof.clone();
        forged["before_sha256"] = serde_json::Value::String("0".repeat(64));
        assert!(std::panic::catch_unwind(|| invert(&forged, &current)).is_err());
    }
    let whole: Vec<serde_json::Value> = serde_json::from_str(
        &std::fs::read_to_string(
            root.join("docs/evidence/held-continuation-growth-v1/sources.json"),
        )
        .unwrap(),
    )
    .unwrap();
    assert_eq!(whole.len(), 10);
    for proof in &whole {
        let current = std::fs::read_to_string(root.join(proof["path"].as_str().unwrap())).unwrap();
        let before = invert(proof, &current);
        assert_eq!(
            before,
            std::fs::read_to_string(root.join(proof["archive"].as_str().unwrap())).unwrap()
        );
        assert!(std::panic::catch_unwind(|| invert(proof, &(current.clone() + "\n"))).is_err());
    }
    let before_file = |path: &str| {
        let proof = whole.iter().find(|p| p["path"] == path).unwrap();
        std::fs::read_to_string(root.join(proof["archive"].as_str().unwrap())).unwrap()
    };
    let old_tests = before_file("src/cleanup_plan/replay/continuation_storage/tests.rs");
    let current_tests =
        std::fs::read_to_string(root.join("src/cleanup_plan/replay/continuation_storage/tests.rs"))
            .unwrap();
    assert!(current_tests.starts_with(&old_tests));
    let old_storage = before_file("src/cleanup_plan/replay/continuation_storage.rs");
    let current_storage =
        std::fs::read_to_string(root.join("src/cleanup_plan/replay/continuation_storage.rs"))
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
    let mut machine = before_last_branch_root(
        &std::fs::read_to_string(root.join("src/cleanup_plan/replay.rs")).unwrap(),
    );
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
        let current = std::fs::read_to_string(root.join(path)).unwrap();
        let before = invert(proof, &current);
        assert_eq!(
            before,
            std::fs::read_to_string(root.join(proof["archive"].as_str().unwrap())).unwrap()
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
        &std::fs::read_to_string(
            root.join("docs/evidence/paid-expression-lookup-state-v1/sources.json"),
        )
        .unwrap(),
    )
    .unwrap();
    assert_eq!(whole.len(), 10);
    for proof in &whole {
        let current = std::fs::read_to_string(root.join(proof["path"].as_str().unwrap())).unwrap();
        let before = invert(proof, &current);
        assert_eq!(
            before,
            std::fs::read_to_string(root.join(proof["archive"].as_str().unwrap())).unwrap()
        );
        assert!(std::panic::catch_unwind(|| invert(proof, &(current.clone() + "\n"))).is_err());
    }
    let before_file = |path: &str| {
        let proof = whole.iter().find(|p| p["path"] == path).unwrap();
        std::fs::read_to_string(root.join(proof["archive"].as_str().unwrap())).unwrap()
    };
    for path in [
        "src/cleanup_plan/replay/expression_facts/arena/prefix.rs",
        "src/cleanup_plan/replay/expression_facts/arena/prefix/tests.rs",
    ] {
        assert!(std::fs::read_to_string(root.join(path))
            .unwrap()
            .starts_with(&before_file(path)));
    }
    let path = "src/cleanup_plan/replay/expression_facts/arena.rs";
    let current = std::fs::read_to_string(root.join(path)).unwrap();
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
    let current = std::fs::read_to_string(root.join(path)).unwrap();
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
        &std::fs::read_to_string(root.join("docs/evidence/ordinary-replay-routes-v1/sources.json"))
            .unwrap(),
    )
    .unwrap();
    assert_eq!(records.len(), 6);
    let mut paths = std::collections::BTreeSet::new();
    for proof in records {
        let path = proof["path"].as_str().unwrap();
        assert!(paths.insert(path.to_owned()));
        let current = std::fs::read_to_string(root.join(path)).unwrap();
        let before = invert(&proof, &current);
        assert_eq!(
            before,
            std::fs::read_to_string(root.join(proof["archive"].as_str().unwrap())).unwrap()
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
        let current = std::fs::read_to_string(root.join(path)).unwrap();
        let before = invert(proof, &current);
        assert_eq!(
            before,
            std::fs::read_to_string(root.join(proof["archive"].as_str().unwrap())).unwrap()
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
        serde_json::from_str(&std::fs::read_to_string(evidence.join("sources.json")).unwrap())
            .unwrap();
    assert_eq!(sources.len(), 7);
    for proof in sources {
        let current = std::fs::read_to_string(root.join(proof["path"].as_str().unwrap())).unwrap();
        assert_eq!(
            invert(&proof, &current),
            std::fs::read_to_string(root.join(proof["archive"].as_str().unwrap())).unwrap()
        );
    }
    for manifest in ["new-files.json", "retained-sources.json"] {
        let records: Vec<serde_json::Value> =
            serde_json::from_str(&std::fs::read_to_string(evidence.join(manifest)).unwrap())
                .unwrap();
        assert_eq!(records.len(), 2);
        for record in records {
            let current =
                std::fs::read_to_string(root.join(record["path"].as_str().unwrap())).unwrap();
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
        serde_json::from_str(&std::fs::read_to_string(evidence.join("sources.json")).unwrap())
            .unwrap();
    assert_eq!(sources.len(), 9);
    let mut paths = std::collections::BTreeSet::new();
    for proof in &sources {
        let path = proof["path"].as_str().unwrap();
        assert!(paths.insert(path));
        let current = std::fs::read_to_string(root.join(path)).unwrap();
        let before = invert(proof, &current);
        assert_eq!(
            before,
            std::fs::read_to_string(root.join(proof["archive"].as_str().unwrap())).unwrap()
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
        serde_json::from_str(&std::fs::read_to_string(evidence.join("new-files.json")).unwrap())
            .unwrap();
    assert_eq!(new_files.len(), 1);
    for file in new_files {
        let current = std::fs::read_to_string(root.join(file["path"].as_str().unwrap())).unwrap();
        assert_eq!(digest(&current), file["sha256"]);
        assert_ne!(digest(&(current + "\n")), file["sha256"]);
    }
}

fn before_last_branch_root(current: &str) -> String {
    let sources: Vec<serde_json::Value> = serde_json::from_str(
        &std::fs::read_to_string(
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
        &std::fs::read_to_string(root.join("docs/evidence/last-branch-prefix-v1/sources.json"))
            .unwrap(),
    )
    .unwrap();
    assert_eq!(sources.len(), 4);
    let mut paths = std::collections::BTreeSet::new();
    for proof in &sources {
        let path = proof["path"].as_str().unwrap();
        assert!(paths.insert(path));
        let current = std::fs::read_to_string(root.join(path)).unwrap();
        let before = invert(proof, &current);
        assert_eq!(
            before,
            std::fs::read_to_string(root.join(proof["archive"].as_str().unwrap())).unwrap()
        );
        assert!(std::panic::catch_unwind(|| invert(proof, &(current.clone() + "\n"))).is_err());
        let mut forged = proof.clone();
        forged["before_sha256"] = serde_json::Value::String("0".repeat(64));
        assert!(std::panic::catch_unwind(|| invert(&forged, &current)).is_err());
        if path.ends_with("branch_clone/tests.rs") {
            assert!(current.starts_with(&before));
        }
    }
    let helper =
        std::fs::read_to_string(root.join("src/cleanup_plan/replay/branch_clone.rs")).unwrap();
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
        &std::fs::read_to_string(root.join("docs/evidence/plan-branch-capacity-v1/sources.json"))
            .unwrap(),
    )
    .unwrap();
    assert_eq!(sources.len(), 5);
    let mut paths = std::collections::BTreeSet::new();
    for proof in &sources {
        let path = proof["path"].as_str().unwrap();
        assert!(paths.insert(path));
        let current = std::fs::read_to_string(root.join(path)).unwrap();
        let before = invert(proof, &current);
        assert_eq!(
            before,
            std::fs::read_to_string(root.join(proof["archive"].as_str().unwrap())).unwrap()
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
        &std::fs::read_to_string(root.join("docs/evidence/replay-work-refusal-v1/sources.json"))
            .unwrap(),
    )
    .unwrap();
    assert_eq!(sources.len(), 4);
    let mut paths = std::collections::BTreeSet::new();
    for proof in &sources {
        let path = proof["path"].as_str().unwrap();
        assert!(paths.insert(path));
        let current = std::fs::read_to_string(root.join(path)).unwrap();
        let before = invert(proof, &current);
        assert_eq!(
            before,
            std::fs::read_to_string(root.join(proof["archive"].as_str().unwrap())).unwrap()
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
        &std::fs::read_to_string(root.join("docs/evidence/status-identity-index-v1/sources.json"))
            .unwrap(),
    )
    .unwrap();
    assert_eq!(sources.len(), 3);
    let mut paths = std::collections::BTreeSet::new();
    for proof in &sources {
        let path = proof["path"].as_str().unwrap();
        assert!(paths.insert(path));
        let current = std::fs::read_to_string(root.join(path)).unwrap();
        let before = invert(proof, &current);
        assert_eq!(
            before,
            std::fs::read_to_string(root.join(proof["archive"].as_str().unwrap())).unwrap()
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
        &std::fs::read_to_string(root.join("docs/evidence/projection-audit-scope-v1/sources.json"))
            .unwrap(),
    )
    .unwrap();
    assert_eq!(sources.len(), 5);
    let mut paths = std::collections::BTreeSet::new();
    for proof in &sources {
        let path = proof["path"].as_str().unwrap();
        assert!(paths.insert(path));
        let current = std::fs::read_to_string(root.join(path)).unwrap();
        let before = invert(proof, &current);
        assert_eq!(
            before,
            std::fs::read_to_string(root.join(proof["archive"].as_str().unwrap())).unwrap()
        );
        assert!(std::panic::catch_unwind(|| invert(proof, &(current.clone() + "\n"))).is_err());
        let mut forged = proof.clone();
        forged["before_sha256"] = serde_json::Value::String("0".repeat(64));
        assert!(std::panic::catch_unwind(|| invert(&forged, &current)).is_err());
    }
    let sealed = std::fs::read_to_string(
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
            serde_json::from_str(&std::fs::read_to_string(root.join(packet)).unwrap()).unwrap();
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
            let mut restored = std::fs::read_to_string(root.join(path)).unwrap();
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
            let original = std::fs::read_to_string(archive).unwrap();
            assert_eq!(digest(&original), old["sha256"]);
            assert_eq!(restored, original);
        }
    }
}

#[test]
fn historical_pin_capture_chains_restore_every_original_source() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let evidence = root.join("docs/evidence/legacy-pin-chain-repair-v1");
    let before =
        std::fs::read_to_string(evidence.join("semantic_receivers.rs.before.txt")).unwrap();
    assert_eq!(
        digest(&before),
        "6af473f412874b7c2e09e4e1effa1f18357f74cf8e31b7cab4a16bd85b4a947d"
    );
    assert!(
        std::fs::read_to_string(root.join("src/source_reflows/semantic_receivers.rs"))
            .unwrap()
            .starts_with(&before)
    );
    let sealed = std::fs::read_to_string(evidence.join("rows.before.json")).unwrap();
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
            serde_json::from_str(&std::fs::read_to_string(root.join(packet)).unwrap()).unwrap();
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
            let mut restored = std::fs::read_to_string(root.join(path)).unwrap();
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
            let original = std::fs::read_to_string(archive).unwrap();
            assert_eq!(digest(&original), row["sha256"]);
            assert_eq!(restored, original);
        }
    }
}
