//! Exact custody for later receiver changes, separate from CI reformatting.
//! These snapshots are used only by historical source assertions. Live source
//! inventories continue to include the current modules and their new children.
use super::*;

const PROOFS: &str = include_str!("../../docs/evidence/main09013-source-namespaces-v1/main8c-inputs/docs/evidence/receiver-source-custody-v1/inverse.json");
const LIVE_AUDITS: &str =
    include_str!("../../docs/evidence/main09013-source-namespaces-v1/main8c-inputs/docs/evidence/live-source-audit-custody-v1/inverse.json");
static PROOF_RECORDS: OnceLock<Vec<serde_json::Value>> = OnceLock::new();

// A later test receiver has its own semantic inverse before the unchanged
// formatting registry. Keep all original twelve plus two custody proofs intact.
fn followups() -> &'static [serde_json::Value] {
    static RECORDS: OnceLock<Vec<serde_json::Value>> = OnceLock::new();
    RECORDS.get_or_init(|| {
        assert_eq!(include_str!("../../docs/evidence/main21-exit-region-integration-20261017/main-inputs/docs/evidence/source-reader-followups-20261016/allocation.json").len(), 24409);
        assert_eq!(digest(include_str!("../../docs/evidence/main21-exit-region-integration-20261017/main-inputs/docs/evidence/source-reader-followups-20261016/allocation.json")), "3d2ca74e38fbdee818f5a1e2789b4943ed8748d3541b484d267ca6c2c56bc511");
        serde_json::from_str(include_str!(
            "../../docs/evidence/main21-exit-region-integration-20261017/main-inputs/docs/evidence/source-reader-followups-20261016/allocation.json"
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
            &main090_read(
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
            &main090_read(
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
            &main090_read(
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
            &main090_read(
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
            &main090_read(
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
            &main090_read(
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
        assert_eq!(LIVE_AUDITS.len(), 24038);
        assert_eq!(
            digest(LIVE_AUDITS),
            "b2437b4145d12bc9fe20fb1dbf2c888c1a6832f2e699a50064017479b1124e4a"
        );
        assert_eq!(PROOFS.len(), 305847);
        assert_eq!(
            digest(PROOFS),
            "c78ab917232bf92e83004d3cff18876476007fe69e8376c5e40ea7209a5f3053"
        );
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
    let staged = main090_expected(proof, current);
    let current = staged.as_str();
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
        let current = main090_read(root.join(path)).unwrap();
        let before = invert(proof, &current);
        assert_eq!(
            before,
            main090_read(root.join(proof["archive"].as_str().unwrap())).unwrap()
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
        serde_json::from_str(&main090_read(evidence.join("sources.json")).unwrap()).unwrap();
    assert_eq!(rows.len(), 34);
    let mut paths = std::collections::BTreeSet::new();
    for row in &rows {
        let path = row["path"].as_str().unwrap();
        assert!(paths.insert(path));
        let current = main090_read(root.join(path)).unwrap();
        let before = invert(row, &current);
        assert_eq!(
            before,
            main090_read(evidence.join(row["archive"].as_str().unwrap())).unwrap()
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
        let current = main090_read(root.join(path)).unwrap();
        let before = invert(proof, &current);
        assert_eq!(
            before,
            main090_read(root.join(proof["archive"].as_str().unwrap())).unwrap()
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
        let current = main090_read(root.join(path)).unwrap();
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
        assert_eq!(restored, main090_read(archive).unwrap());
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
        let current = main090_read(root.join(proof["path"].as_str().unwrap())).unwrap();
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
        let current = main090_read(root.join(path)).unwrap();
        let restored = invert(proof, &current);
        let archive = root
            .join("docs/evidence/source-reader-followups-20261016")
            .join(proof["archive"].as_str().unwrap());
        assert_eq!(restored, main090_read(archive).unwrap());
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
        serde_json::from_str(&main090_read(evidence.join("sources.json")).unwrap()).unwrap();
    for record in records.as_array().unwrap() {
        let original = main090_read(evidence.join(record["archive"].as_str().unwrap())).unwrap();
        assert_eq!(digest(&original), record["sha256"]);
        let current = main090_read(root.join(record["path"].as_str().unwrap())).unwrap();
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
        let current = main090_read(root.join(path)).unwrap();
        let before = invert(proof, &current);
        assert_eq!(
            before,
            main090_read(root.join(proof["archive"].as_str().unwrap())).unwrap()
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
        &main090_read(root.join("docs/evidence/held-status-prefix-v1/sources.json")).unwrap(),
    )
    .unwrap();
    assert_eq!(whole.len(), 6);
    for proof in &whole {
        let current = main090_read(root.join(proof["path"].as_str().unwrap())).unwrap();
        let before = invert(proof, &current);
        assert_eq!(
            before,
            main090_read(root.join(proof["archive"].as_str().unwrap())).unwrap()
        );
        assert!(std::panic::catch_unwind(|| invert(proof, &(current.clone() + "\n"))).is_err());
    }
    let proof = held_status()
        .iter()
        .find(|p| p["path"] == "src/cleanup_plan/replay/expression_facts/receiving/retained.rs")
        .unwrap();
    let before = main090_read(root.join(proof["archive"].as_str().unwrap())).unwrap();
    let current = main090_read(root.join(proof["path"].as_str().unwrap())).unwrap();
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
        let current = main090_read(root.join(path)).unwrap();
        let before = invert(proof, &current);
        assert_eq!(
            before,
            main090_read(root.join(proof["archive"].as_str().unwrap())).unwrap()
        );
        assert!(contains(path));
        assert_eq!(predecessor(path, &current), Some(before));
        assert!(std::panic::catch_unwind(|| invert(proof, &(current.clone() + "\n"))).is_err());
        let mut forged = proof.clone();
        forged["before_sha256"] = serde_json::Value::String("0".repeat(64));
        assert!(std::panic::catch_unwind(|| invert(&forged, &current)).is_err());
    }
    let whole: Vec<serde_json::Value> = serde_json::from_str(
        &main090_read(root.join("docs/evidence/held-continuation-growth-v1/sources.json")).unwrap(),
    )
    .unwrap();
    assert_eq!(whole.len(), 10);
    for proof in &whole {
        let current = main090_read(root.join(proof["path"].as_str().unwrap())).unwrap();
        let before = invert(proof, &current);
        assert_eq!(
            before,
            main090_read(root.join(proof["archive"].as_str().unwrap())).unwrap()
        );
        assert!(std::panic::catch_unwind(|| invert(proof, &(current.clone() + "\n"))).is_err());
    }
    let before_file = |path: &str| {
        let proof = whole.iter().find(|p| p["path"] == path).unwrap();
        main090_read(root.join(proof["archive"].as_str().unwrap())).unwrap()
    };
    let old_tests = before_file("src/cleanup_plan/replay/continuation_storage/tests.rs");
    let current_tests =
        main090_read(root.join("src/cleanup_plan/replay/continuation_storage/tests.rs")).unwrap();
    assert!(current_tests.starts_with(&old_tests));
    let old_storage = before_file("src/cleanup_plan/replay/continuation_storage.rs");
    let current_storage =
        main090_read(root.join("src/cleanup_plan/replay/continuation_storage.rs")).unwrap();
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
        before_last_branch_root(&main090_read(root.join("src/cleanup_plan/replay.rs")).unwrap());
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
        let current = main090_read(root.join(path)).unwrap();
        let before = invert(proof, &current);
        assert_eq!(
            before,
            main090_read(root.join(proof["archive"].as_str().unwrap())).unwrap()
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
        &main090_read(root.join("docs/evidence/paid-expression-lookup-state-v1/sources.json"))
            .unwrap(),
    )
    .unwrap();
    assert_eq!(whole.len(), 10);
    for proof in &whole {
        let current = main090_read(root.join(proof["path"].as_str().unwrap())).unwrap();
        let before = invert(proof, &current);
        assert_eq!(
            before,
            main090_read(root.join(proof["archive"].as_str().unwrap())).unwrap()
        );
        assert!(std::panic::catch_unwind(|| invert(proof, &(current.clone() + "\n"))).is_err());
    }
    let before_file = |path: &str| {
        let proof = whole.iter().find(|p| p["path"] == path).unwrap();
        main090_read(root.join(proof["archive"].as_str().unwrap())).unwrap()
    };
    for path in [
        "src/cleanup_plan/replay/expression_facts/arena/prefix.rs",
        "src/cleanup_plan/replay/expression_facts/arena/prefix/tests.rs",
    ] {
        assert!(main090_read(root.join(path))
            .unwrap()
            .starts_with(&before_file(path)));
    }
    let path = "src/cleanup_plan/replay/expression_facts/arena.rs";
    let current = main090_read(root.join(path)).unwrap();
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
    let current = main090_read(root.join(path)).unwrap();
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
        &main090_read(root.join("docs/evidence/ordinary-replay-routes-v1/sources.json")).unwrap(),
    )
    .unwrap();
    assert_eq!(records.len(), 6);
    let mut paths = std::collections::BTreeSet::new();
    for proof in records {
        let path = proof["path"].as_str().unwrap();
        assert!(paths.insert(path.to_owned()));
        let current = main090_read(root.join(path)).unwrap();
        let before = invert(&proof, &current);
        assert_eq!(
            before,
            main090_read(root.join(proof["archive"].as_str().unwrap())).unwrap()
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
        let current = main090_read(root.join(path)).unwrap();
        let before = invert(proof, &current);
        assert_eq!(
            before,
            main090_read(root.join(proof["archive"].as_str().unwrap())).unwrap()
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
        serde_json::from_str(&main090_read(evidence.join("sources.json")).unwrap()).unwrap();
    assert_eq!(sources.len(), 7);
    for proof in sources {
        let current = main090_read(root.join(proof["path"].as_str().unwrap())).unwrap();
        assert_eq!(
            invert(&proof, &current),
            main090_read(root.join(proof["archive"].as_str().unwrap())).unwrap()
        );
    }
    for manifest in ["new-files.json", "retained-sources.json"] {
        let records: Vec<serde_json::Value> =
            serde_json::from_str(&main090_read(evidence.join(manifest)).unwrap()).unwrap();
        assert_eq!(records.len(), 2);
        for record in records {
            let current = main090_read(root.join(record["path"].as_str().unwrap())).unwrap();
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
        serde_json::from_str(&main090_read(evidence.join("sources.json")).unwrap()).unwrap();
    assert_eq!(sources.len(), 9);
    let mut paths = std::collections::BTreeSet::new();
    for proof in &sources {
        let path = proof["path"].as_str().unwrap();
        assert!(paths.insert(path));
        let current = main090_read(root.join(path)).unwrap();
        let before = invert(proof, &current);
        assert_eq!(
            before,
            main090_read(root.join(proof["archive"].as_str().unwrap())).unwrap()
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
        serde_json::from_str(&main090_read(evidence.join("new-files.json")).unwrap()).unwrap();
    assert_eq!(new_files.len(), 1);
    for file in new_files {
        let current = main090_read(root.join(file["path"].as_str().unwrap())).unwrap();
        assert_eq!(digest(&current), file["sha256"]);
        assert_ne!(digest(&(current + "\n")), file["sha256"]);
    }
}

fn before_last_branch_root(current: &str) -> String {
    let sources: Vec<serde_json::Value> = serde_json::from_str(
        &main090_read(
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
        &main090_read(root.join("docs/evidence/last-branch-prefix-v1/sources.json")).unwrap(),
    )
    .unwrap();
    assert_eq!(sources.len(), 4);
    let mut paths = std::collections::BTreeSet::new();
    for proof in &sources {
        let path = proof["path"].as_str().unwrap();
        assert!(paths.insert(path));
        let current = main090_read(root.join(path)).unwrap();
        let before = invert(proof, &current);
        assert_eq!(
            before,
            main090_read(root.join(proof["archive"].as_str().unwrap())).unwrap()
        );
        assert!(std::panic::catch_unwind(|| invert(proof, &(current.clone() + "\n"))).is_err());
        let mut forged = proof.clone();
        forged["before_sha256"] = serde_json::Value::String("0".repeat(64));
        assert!(std::panic::catch_unwind(|| invert(&forged, &current)).is_err());
        if path.ends_with("branch_clone/tests.rs") {
            assert!(current.starts_with(&before));
        }
    }
    let helper = main090_read(root.join("src/cleanup_plan/replay/branch_clone.rs")).unwrap();
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

// Separate finite bounded acquisition; complete original body above.
const MAIN090_MANIFEST: &str = "docs/evidence/main09013-source-namespaces-v1/input-domains.json";
const MAIN090_MANIFEST_LEN: usize = 241196;
const MAIN090_MANIFEST_SHA: &str =
    "05bc465dafa95860e40b9b76cd7a01d0542e169606e6e5e11720c1985fa7fdc2";
const MAIN090_CAP: usize = 9464242; // Exact largest sealed MAIN8c input, distinct historical namespace.
fn main090_manifest() -> serde_json::Value {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let text = main090_file(
        &root.join(MAIN090_MANIFEST),
        MAIN090_MANIFEST_LEN,
        MAIN090_MANIFEST_SHA,
    )
    .unwrap();
    serde_json::from_str(&text).unwrap()
}
fn main090_checked(path: &Path, length: usize, sha: &str) -> String {
    assert!(length <= MAIN090_CAP);
    main090_file(path, length, sha).unwrap()
}
fn main090_row<'a>(
    manifest: &'a serde_json::Value,
    domain: &str,
    path: &str,
) -> Option<&'a serde_json::Value> {
    manifest[domain]
        .as_array()
        .unwrap()
        .iter()
        .find(|r| r["path"] == path)
}
fn main090_entry(entry: &serde_json::Value, current: &str) -> String {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    // Every staged value is authenticated against actual physical current input.
    let raw = main090_checked(
        &root.join(entry["path"].as_str().unwrap()),
        entry["current_len"].as_u64().unwrap() as usize,
        entry["current_sha256"].as_str().unwrap(),
    );
    let before = main090_checked(
        &root.join(entry["archive"].as_str().unwrap()),
        entry["view_len"].as_u64().unwrap() as usize,
        entry["view_sha256"].as_str().unwrap(),
    );
    assert!(current == raw || current == before, "foreign MAIN090 stage");
    before
}
#[allow(
    dead_code,
    reason = "Retained compatibility entry for independently compiled historical and semantic controls."
)]
fn main090_project(path: &str, current: &str) -> String {
    let manifest = main090_manifest();
    match main090_row(&manifest, "main", path) {
        Some(row) => main090_entry(row, current),
        None => current.to_owned(),
    }
}
fn main090_read(path: impl AsRef<Path>) -> std::io::Result<String> {
    let path = path.as_ref();
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let logical = path
        .strip_prefix(root)
        .ok()
        .and_then(|p| p.to_str())
        .ok_or_else(|| {
            std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                "foreign MAIN090 read root",
            )
        })?;
    let manifest = main090_manifest();
    if let Some(row) = main090_row(&manifest, "main", logical) {
        let raw = main090_checked(
            path,
            row["current_len"].as_u64().unwrap() as usize,
            row["current_sha256"].as_str().unwrap(),
        );
        return Ok(main090_entry(row, &raw));
    }
    Err(std::io::Error::new(
        std::io::ErrorKind::InvalidInput,
        "unknown MAIN090 read subject",
    ))
}
#[allow(
    dead_code,
    reason = "Retained compatibility entry for independently compiled historical and semantic controls."
)]
pub(in crate::source_reflows) fn ours_input(path: &str, current: &str) -> String {
    let manifest = main090_manifest();
    match main090_row(&manifest, "ours", path) {
        Some(row) => main090_entry(row, current),
        None => current.to_owned(),
    }
}
#[allow(
    dead_code,
    reason = "Retained compatibility entry for independently compiled historical and semantic controls."
)]
pub(in crate::source_reflows) fn ours_expected_input(
    proof: &serde_json::Value,
    current: &str,
) -> String {
    let manifest = main090_manifest();
    let path = proof["path"].as_str().unwrap();
    match main090_row(&manifest, "ours", path) {
        Some(row) if proof["after_sha256"] == row["view_sha256"] => main090_entry(row, current),
        _ => current.to_owned(), // Preserve an already selected older MAIN domain.
    }
}
#[allow(
    dead_code,
    reason = "Retained compatibility entry for independently compiled historical and semantic controls."
)]
pub(super) fn ours_contains(path: &str) -> bool {
    main090_row(&main090_manifest(), "ours", path).is_some()
}
fn records() -> &'static [serde_json::Value] {
    static RECORDS: OnceLock<Vec<serde_json::Value>> = OnceLock::new();
    RECORDS.get_or_init(|| {
        let root = Path::new(env!("CARGO_MANIFEST_DIR"));
        serde_json::from_str(&main090_read(root.join("src/source_reflows.json")).unwrap()).unwrap()
    })
}
fn restore(record: &serde_json::Value, current: &str) -> String {
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

fn main090_body(
    reader: &mut impl std::io::Read,
    length: usize,
    expected: &str,
) -> Result<String, &'static str> {
    if length > MAIN090_CAP {
        return Err("bound");
    }
    let mut bytes = Vec::new();
    bytes.try_reserve_exact(length).map_err(|_| "allocation")?;
    bytes.resize(length, 0);
    reader.read_exact(&mut bytes).map_err(|_| "truncated")?;
    let mut extra = [0u8; 1];
    if reader.read(&mut extra).map_err(|_| "read")? != 0 {
        return Err("extra");
    }
    if digest_bytes(&bytes) != expected {
        return Err("digest");
    }
    String::from_utf8(bytes).map_err(|_| "utf8")
}
fn digest_bytes(bytes: &[u8]) -> String {
    format!("{:x}", crate::digest_hex::LowerHex(Sha256::digest(bytes)))
}
fn main090_same_file(a: &std::fs::Metadata, b: &std::fs::Metadata) -> bool {
    if !a.is_file() || !b.is_file() || a.len() != b.len() || a.modified().ok() != b.modified().ok()
    {
        return false;
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        if a.dev() != b.dev()
            || a.ino() != b.ino()
            || a.ctime() != b.ctime()
            || a.ctime_nsec() != b.ctime_nsec()
        {
            return false;
        }
    }
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt;
        if a.file_attributes() & 0x400 != 0
            || b.file_attributes() & 0x400 != 0
            || a.creation_time() != b.creation_time()
            || a.last_write_time() != b.last_write_time()
        {
            return false;
        }
    }
    true
}
fn main090_file(path: &Path, length: usize, expected: &str) -> Result<String, &'static str> {
    if length > MAIN090_CAP {
        return Err("bound");
    }
    let path_before = std::fs::symlink_metadata(path).map_err(|_| "missing")?;
    if path_before.file_type().is_symlink() || !path_before.is_file() {
        return Err("length-or-nonregular");
    }
    let mut options = std::fs::OpenOptions::new();
    options.read(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.custom_flags(libc::O_NOFOLLOW | libc::O_CLOEXEC | libc::O_NONBLOCK);
    }
    #[cfg(windows)]
    {
        use std::os::windows::fs::OpenOptionsExt;
        options.custom_flags(0x0020_0000);
    }
    #[cfg(not(any(unix, windows)))]
    {
        return Err("platform");
    }
    let mut file = options.open(path).map_err(|_| "open")?;
    let metadata = file.metadata().map_err(|_| "metadata")?;
    if !main090_same_file(&path_before, &metadata)
        || metadata.len() != u64::try_from(length).map_err(|_| "bound")?
    {
        return Err("length-or-nonregular");
    }
    // The one opened no-follow regular handle is checked before any buffer.
    let value = main090_body(&mut file, length, expected)?;
    let after = file.metadata().map_err(|_| "metadata")?;
    let path_after = std::fs::symlink_metadata(path).map_err(|_| "metadata")?;
    if path_after.file_type().is_symlink()
        || !main090_same_file(&metadata, &after)
        || !main090_same_file(&after, &path_after)
    {
        return Err("metadata-drift");
    }
    Ok(value)
}

fn main090_expected(proof: &serde_json::Value, current: &str) -> String {
    let manifest = main090_manifest();
    let path = proof["path"].as_str().unwrap();
    let expected = proof["after_sha256"].as_str().unwrap();
    if let Some(stage) = manifest["expected"]
        .as_array()
        .unwrap()
        .iter()
        .find(|row| row["path"] == path && row["view_sha256"] == expected)
    {
        let row = main090_row(&manifest, "main", path).unwrap();
        let root = Path::new(env!("CARGO_MANIFEST_DIR"));
        let _physical = main090_checked(
            &root.join(path),
            row["current_len"].as_u64().unwrap() as usize,
            row["current_sha256"].as_str().unwrap(),
        );
        if digest(current) == expected {
            return current.to_owned();
        }
        let selected = main090_entry(row, current);
        assert_eq!(
            selected,
            main090_checked(
                &Path::new(env!("CARGO_MANIFEST_DIR")).join(row["archive"].as_str().unwrap()),
                row["view_len"].as_u64().unwrap() as usize,
                row["view_sha256"].as_str().unwrap()
            )
        );
        return main090_checked(
            &Path::new(env!("CARGO_MANIFEST_DIR")).join(stage["archive"].as_str().unwrap()),
            stage["view_len"].as_u64().unwrap() as usize,
            expected,
        );
    }
    current.to_owned()
}
pub(super) fn legacy_checked(path: &Path, length: usize, expected: &str) -> Option<String> {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let logical = path.strip_prefix(root).ok()?.to_str()?;
    let manifest = main090_manifest();
    let old = main090_row(&manifest, "legacy_current", logical)?;
    if old["view_sha256"] != expected || old["view_len"].as_u64()? as usize != length {
        return None;
    }
    let row = main090_row(&manifest, "physical", logical).unwrap();
    let _raw = main090_checked(
        path,
        row["current_len"].as_u64().unwrap() as usize,
        row["current_sha256"].as_str().unwrap(),
    );
    Some(main090_checked(
        &root.join(old["archive"].as_str().unwrap()),
        length,
        expected,
    ))
}
pub(super) fn legacy_expected(entry: &serde_json::Value, current: &str) -> String {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let path = entry["path"].as_str().unwrap();
    let manifest = main090_manifest();
    if let Some(row) = main090_row(&manifest, "physical", path) {
        let raw = main090_checked(
            &root.join(path),
            row["current_len"].as_u64().unwrap() as usize,
            row["current_sha256"].as_str().unwrap(),
        );
        if current == raw {
            return legacy_checked(
                &root.join(path),
                entry["current_len"].as_u64().unwrap() as usize,
                entry["current_sha256"].as_str().unwrap(),
            )
            .unwrap();
        }
    }
    current.to_owned()
}
#[test]
fn main090_sealed_domains_refuse_foreign_and_incomplete_inputs() {
    assert_eq!(
        main090_body(&mut &b"abc"[..], 3, &digest_bytes(b"abc")).unwrap(),
        "abc"
    );
    for bytes in [&b"ab"[..], &b"abcd"[..]] {
        assert!(main090_body(&mut &bytes[..], 3, &digest_bytes(b"abc")).is_err());
    }
    assert!(main090_body(&mut &b"abc"[..], 3, &"0".repeat(64)).is_err());
    assert!(main090_body(&mut &b"\xff"[..], 1, &digest_bytes(b"\xff")).is_err());
    assert!(main090_body(&mut &b""[..], MAIN090_CAP + 1, &"0".repeat(64)).is_err());
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    assert!(main090_read(root.join("foreign-main090-input.rs")).is_err());
    for row in main090_manifest()["main"].as_array().unwrap() {
        let raw = main090_checked(
            &root.join(row["path"].as_str().unwrap()),
            row["current_len"].as_u64().unwrap() as usize,
            row["current_sha256"].as_str().unwrap(),
        );
        let view = main090_entry(row, &raw);
        assert_eq!(main090_entry(row, &view), view);
        assert!(std::panic::catch_unwind(|| main090_entry(row, &(raw.clone() + "\n"))).is_err());
    }
}
