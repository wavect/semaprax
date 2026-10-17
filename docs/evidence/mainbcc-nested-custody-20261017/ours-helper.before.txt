//! Full actual joined input first; immutable parent replays remain independent.
//! No production receiver, source predicate, old archive or recipe is replaced.
use serde_json::Value;
use sha2::{Digest, Sha256};

const PATH: &str = "src/hir/validation/byte_traversal/nested/tests.rs";
const HELPER: &str = "src/hir/validation/byte_traversal/nested/tests/source_composition.rs";
const EVIDENCE: &str = "docs/evidence/incoming-main-a9a75e1-20261016";
const INCOMING_SHA: &str = "9a8b41dff3fd9bb3211e62f7180beabe0a0251da41266ed6a2a83e69f67af63f";
const OURS_SHA: &str = "bb7ae50a94f03a334dab03412baeb1986a98d7e2d4694bcd033d6f03008cf431";
const OURS940_SHA: &str = "16a6195ebf472b9d328c9da1f6267a400d3f34c783e39080641e8714219f7caf";
const ORIGINAL940_SHA: &str = "7fe239c8c7c5afe12db2c3125286b45cf8b168cac45a408d0c443cc4ecfe5427";
const BASE_SHA: &str = "4731dd5a006a7b5fdb906795a01c61045f183bf03efce70f7fbfc7164e17b145";
const LIVE: &str = include_str!("../tests.rs");
const SELF: &str = include_str!("source_composition.rs");
const RECIPE: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/docs/evidence/incoming-main-a9a75e1-20261016/nested-source-composition.json"
));
fn digest(source: &str) -> String {
    format!(
        "{:x}",
        crate::digest_hex::LowerHex(Sha256::digest(source.as_bytes()))
    )
}
fn file(path: &str) -> Option<String> {
    std::fs::read_to_string(std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join(path)).ok()
}
fn checked_file(path: &str, expected: &str) -> Option<String> {
    let source = file(path)?;
    (digest(&source) == expected).then_some(source)
}
fn restore(source: &str, row: &Value, expected: &str) -> Option<String> {
    if row["after_sha256"].as_str()? != digest(source) || row["before_sha256"].as_str()? != expected
    {
        return None;
    }
    let mut result = source.to_owned();
    let mut previous_start = source.len();
    for edit in row["inverse"].as_array()? {
        let start = usize::try_from(edit["start"].as_u64()?).ok()?;
        let end = usize::try_from(edit["end"].as_u64()?).ok()?;
        if start > end
            || end > previous_start
            || !result.is_char_boundary(start)
            || !result.is_char_boundary(end)
        {
            return None;
        }
        let current = result.get(start..end)?;
        if digest(current) != edit["after_sha256"].as_str()? {
            return None;
        }
        result.replace_range(start..end, edit["before"].as_str()?);
        previous_start = start;
    }
    (digest(&result) == expected).then_some(result)
}
fn old_ci(source: &str) -> Option<()> {
    let record: Value = serde_json::from_str(&file(
        "docs/evidence/nested-byte-traversal-780/ci-scope-original.json",
    )?)
    .ok()?;
    if record["path"] != PATH
        || record["archive"] != "ci-scope-tests.before.txt"
        || record["sha256"] != BASE_SHA
    {
        return None;
    }
    let original = checked_file(
        "docs/evidence/nested-byte-traversal-780/ci-scope-tests.before.txt",
        BASE_SHA,
    )?;
    let mut restored = source.to_owned();
    for edit in record["edits"].as_array()?.iter().rev() {
        let new = edit["new"].as_str()?;
        if restored.matches(new).count() != 1 {
            return None;
        }
        restored = restored.replacen(new, edit["old"].as_str()?, 1);
    }
    (restored == original).then_some(())
}
fn old_940(source: &str) -> Option<()> {
    let records: Value = serde_json::from_str(&file(
        "docs/evidence/nested-byte-actual-fixture-correction-940/source-inverses.json",
    )?)
    .ok()?;
    let row = records.as_array()?.iter().find(|r| r["source"] == PATH)?;
    if row["archive"] != "docs/evidence/nested-byte-actual-fixture-correction-940/original/0.before"
        || row["current_sha256"] != OURS940_SHA
        || row["original_sha256"] != ORIGINAL940_SHA
    {
        return None;
    }
    let original = checked_file(
        "docs/evidence/nested-byte-actual-fixture-correction-940/original/0.before",
        ORIGINAL940_SHA,
    )?;
    let mut restored = source.to_owned();
    for edit in row["edits"].as_array()? {
        let new = edit["new"].as_str()?;
        if restored.matches(new).count() != 1 {
            return None;
        }
        restored = restored.replacen(new, edit["before"].as_str()?, 1);
    }
    (restored == original).then_some(())
}
fn views(raw: &str, recipe: &Value) -> Option<(String, String)> {
    // Compiled full source and exact helper bytes authenticate every live clause
    // before any prior-input replay. This does not accept a historical raw file.
    if raw != LIVE
        || recipe["schema"] != "semaprax.nested-a9a-joined-input.v1"
        || recipe["source"] != PATH
        || recipe["helper"] != HELPER
        || recipe["joined_sha256"].as_str()? != digest(raw)
        || recipe["helper_sha256"].as_str()? != digest(SELF)
    {
        return None;
    }
    let incoming = restore(raw, &recipe["joined_to_incoming"], INCOMING_SHA)?;
    let ours = restore(raw, &recipe["joined_to_ours"], OURS_SHA)?;
    let incoming_archive = checked_file(&format!("{EVIDENCE}/incoming/{PATH}"), INCOMING_SHA)?;
    let ours_archive = checked_file(&format!("{EVIDENCE}/ours/{PATH}"), OURS_SHA)?;
    if incoming != incoming_archive || ours != ours_archive {
        return None;
    }
    old_ci(&incoming)?;
    let ours940 = restore(&ours, &recipe["ours_to_940"], OURS940_SHA)?;
    old_940(&ours940)?;
    Some((incoming, ours))
}
pub(super) fn incoming_current(raw: &str) -> Option<String> {
    let recipe: Value = serde_json::from_str(RECIPE).ok()?;
    views(raw, &recipe).map(|pair| pair.0)
}
#[test]
fn joined_nested_source_keeps_full_live_union_and_both_complete_parent_custodies() {
    let actual = file(PATH).unwrap();
    assert_eq!(actual, LIVE);
    assert_eq!(file(HELPER).unwrap(), SELF);
    let recipe: Value = serde_json::from_str(RECIPE).unwrap();
    let (incoming, ours) = views(&actual, &recipe).unwrap();
    assert_eq!(digest(&incoming), INCOMING_SHA);
    assert_eq!(digest(&ours), OURS_SHA);
    for predicate in [
        "assert_eq!(census.rows.len(), 2);",
        "assert!(outer.start < inner.start && inner.end < outer.end);",
        "assert!(sum.loan_plan.loans.is_empty());",
        "assert_eq!(storage.ty, crate::hir::ResolvedType::Bytes);",
        "assert_eq!(storage.ownership, crate::hir::OwnershipMode::Own);",
        "assert!(after_inner.live_after.contains(&outer_loan.id));",
        "assert!(after_outer.live_before.contains(&outer_loan.id));",
        "assert_eq!(setup, scope_metrics);",
        "assert_eq!(observed.1, setup);",
        "assert_eq!(current_metrics().allocated, before);",
        "assert_eq!(current_metrics(), before_metrics);",
        "assert_eq!(metrics, scope_metrics);",
        "assert_eq!(metrics.allocated - external_setup, 0);",
        "assert_eq!(metrics.allocated - scope_metrics.allocated, 0);",
    ] {
        assert!(
            actual.contains(predicate),
            "complete live predicate: {predicate}"
        );
    }
    assert_eq!(
        actual
            .matches("assert_eq!(current_metrics().allocated, before);")
            .count(),
        2
    );
    assert_eq!(
        actual
            .matches("assert_eq!(current_metrics(), before_metrics);")
            .count(),
        2
    );
}
#[test]
fn joined_nested_source_refuses_foreign_truncated_drifted_predicates_and_partial_recipes() {
    let recipe: Value = serde_json::from_str(RECIPE).unwrap();
    let (incoming, ours) = views(LIVE, &recipe).unwrap();
    for changed in [
        incoming,
        ours,
        LIVE[..LIVE.len() - 1].to_owned(),
        format!("{LIVE}\n"),
        LIVE.replacen(
            "after_inner.live_after.contains",
            "after_inner.live_before.contains",
            1,
        ),
        LIVE.replacen(
            "assert_eq!(metrics, scope_metrics);",
            "assert_eq!(metrics.allocated, scope_metrics.allocated);",
            1,
        ),
    ] {
        assert!(views(&changed, &recipe).is_none());
        assert!(incoming_current(&changed).is_none());
    }
    let mut broken = recipe.clone();
    broken["joined_to_incoming"]["inverse"] = serde_json::json!([]);
    assert!(views(LIVE, &broken).is_none());
    let mut broken = recipe.clone();
    broken["joined_to_ours"]["before_sha256"] = Value::String(INCOMING_SHA.to_owned());
    assert!(views(LIVE, &broken).is_none());
    let mut broken = recipe;
    broken["ours_to_940"]["inverse"] = serde_json::json!([]);
    assert!(views(LIVE, &broken).is_none());
}
