//! Closed complete source-reader domains for independent OURS8fbb and MAINbcc custody.
//! Core HIR retains only callable test entries; every old body remains executable.
use sha2::{Digest, Sha256};
const EVIDENCE: &str = "docs/evidence/mainbcc-nested-custody-20261017";
const HIR: &str = "src/hir/validation/byte_traversal/nested/tests.rs";
const HELPER: &str = "src/hir/validation/byte_traversal/nested/tests/source_composition.rs";
const READER: &str = "src/source_reflows/nested_scope.rs";
const LIVE_HIR: &str = include_str!("../hir/validation/byte_traversal/nested/tests.rs");
const LIVE_HELPER: &str =
    include_str!("../hir/validation/byte_traversal/nested/tests/source_composition.rs");
const LIVE_READER: &str = include_str!("nested_scope.rs");
const OURS_TESTS: &str =
    include_str!("../../docs/evidence/mainbcc-nested-custody-20261017/ours-tests.before.txt");
const OURS_HELPER: &str =
    include_str!("../../docs/evidence/mainbcc-nested-custody-20261017/ours-helper.before.txt");
const MAIN_TESTS: &str =
    include_str!("../../docs/evidence/mainbcc-nested-custody-20261017/main-tests.before.txt");
const MAIN_TABLE: &str =
    include_str!("../../docs/evidence/mainbcc-nested-custody-20261017/main-ci-scope-original.json");
const SUBJECTS: &str =
    include_str!("../../docs/evidence/mainbcc-nested-custody-20261017/subjects.json");
fn digest(source: &str) -> String {
    format!(
        "{:x}",
        crate::digest_hex::LowerHex(Sha256::digest(source.as_bytes()))
    )
}
fn actual(path: &str) -> std::io::Result<String> {
    std::fs::read_to_string(std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join(path))
}
fn complete_inputs(hir: &str, helper: &str, reader: &str) -> Option<()> {
    (hir == LIVE_HIR && helper == LIVE_HELPER && reader == LIVE_READER).then_some(())
}
fn authenticate() -> Option<()> {
    complete_inputs(
        &actual(HIR).ok()?,
        &actual(HELPER).ok()?,
        &actual(READER).ok()?,
    )?;
    (actual(&format!("{EVIDENCE}/subjects.json")).ok()? == SUBJECTS).then_some(())
}
fn archive(path: &str) -> Option<String> {
    let rows: serde_json::Value = serde_json::from_str(SUBJECTS).ok()?;
    let rows = rows.as_array()?;
    if rows.len() != 8 {
        return None;
    }
    let mut matching = rows.iter().filter(|row| row["path"].as_str() == Some(path));
    let row = matching.next()?;
    if matching.next().is_some() {
        return None;
    }
    let name = row["archive"].as_str()?;
    if name.contains('/') || name.contains("..") {
        return None;
    }
    let value = actual(&format!("{EVIDENCE}/{name}")).ok()?;
    (digest(&value) == row["sha256"].as_str()?).then_some(value)
}
fn domain_read(main: bool, path: &std::path::Path) -> std::io::Result<String> {
    let refuse = || {
        std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            "closed nested source domain refused",
        )
    };
    authenticate().ok_or_else(refuse)?;
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let relative = path.strip_prefix(root).map_err(|_| refuse())?;
    if relative
        .components()
        .any(|c| !matches!(c, std::path::Component::Normal(_)))
    {
        return Err(refuse());
    }
    let logical = relative.to_str().ok_or_else(refuse)?;
    // Lookup before filesystem access: this adapter never grants a generic read.
    let original = archive(logical).ok_or_else(refuse)?;
    let raw = actual(logical)?;
    match logical {
        HIR if raw == LIVE_HIR => Ok(if main { MAIN_TESTS } else { OURS_TESTS }.to_owned()),
        HELPER if !main && raw == LIVE_HELPER => Ok(OURS_HELPER.to_owned()),
        "docs/evidence/nested-byte-traversal-780/ci-scope-original.json" if raw == original => {
            if main {
                let value = actual(&format!("{EVIDENCE}/main-ci-scope-original.json"))?;
                if value != MAIN_TABLE {
                    return Err(refuse());
                }
                Ok(value)
            } else {
                Ok(raw)
            }
        }
        _ if raw == original => Ok(raw),
        _ => Err(refuse()),
    }
}
mod main_std {
    pub(super) use std::path;
    pub(super) mod fs {
        pub(crate) fn read_to_string(path: impl AsRef<std::path::Path>) -> std::io::Result<String> {
            super::super::domain_read(true, path.as_ref())
        }
    }
}
mod ours_std {
    pub(super) use std::path;
    pub(super) mod fs {
        pub(crate) fn read_to_string(path: impl AsRef<std::path::Path>) -> std::io::Result<String> {
            super::super::domain_read(false, path.as_ref())
        }
    }
}
mod ci_scope {
    use super::main_std as std;
    pub(crate) fn nested_scope_correction_retains_complete_original_hostile_and_loan_controls() {
        use sha2::{Digest, Sha256};
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
        let evidence = root.join("docs/evidence/nested-byte-traversal-780");
        let record: serde_json::Value = serde_json::from_str(
            &std::fs::read_to_string(evidence.join("ci-scope-original.json")).unwrap(),
        )
        .unwrap();
        let original =
            std::fs::read_to_string(evidence.join(record["archive"].as_str().unwrap())).unwrap();
        assert_eq!(
            format!(
                "{:x}",
                crate::digest_hex::LowerHex(Sha256::digest(original.as_bytes()))
            ),
            record["sha256"].as_str().unwrap()
        );
        let current = std::fs::read_to_string(root.join(record["path"].as_str().unwrap())).unwrap();
        let restore = |source: &str| -> Option<String> {
            let mut result = source.to_owned();
            for edit in record["edits"].as_array().unwrap().iter().rev() {
                let new = edit["new"].as_str().unwrap();
                if result.matches(new).count() != 1 {
                    return None;
                }
                result = result.replacen(new, edit["old"].as_str().unwrap(), 1);
            }
            Some(result)
        };
        assert_eq!(restore(&current).as_deref(), Some(original.as_str()));
        assert_ne!(
            restore(&(current.clone() + "\n")).as_deref(),
            Some(original.as_str())
        );
        let start = original.find("    let freezes = sum\n").unwrap();
        let end = start
            + original[start..]
                .find("    for id in outer.shape.frozen {")
                .unwrap();
        assert_eq!(current.matches(&original[start..end]).count(), 1);
        let tail_end = end + original[end..].find("\n#[test]").unwrap();
        assert_eq!(current.matches(&original[end..tail_end]).count(), 1);
        let tampered = current.replacen(
            "after_inner.live_after.contains",
            "after_inner.live_before.contains",
            1,
        );
        assert_ne!(restore(&tampered).as_deref(), Some(original.as_str()));
        assert!(current.contains("let scope_metrics = ci_scope::empty_nested_budget_metrics();"));
        assert!(current.contains("assert_eq!(metrics, scope_metrics);"));
        assert!(current.contains("assert_eq!(metrics.allocated - scope_metrics.allocated, 0);"));
    }
}
#[test]
fn complete_nested_custody_assertions_remain_executable_and_byte_exact() {
    use sha2::{Digest, Sha256};
    ci_scope::nested_scope_correction_retains_complete_original_hostile_and_loan_controls();
    let original = include_str!(
        "../../docs/evidence/nested-byte-traversal-780/ci-scope-custody-function.before.txt"
    );
    assert_eq!(
        format!(
            "{:x}",
            crate::digest_hex::LowerHex(Sha256::digest(original.as_bytes()))
        ),
        "e42c74a514afe34fef7c9b1fd0bc17368f59dce64d2447d4b0580bf73756064e"
    );
    let relocated = original.replacen("    fn ", "    pub(crate) fn ", 1);
    assert_eq!(
        include_str!("nested_scope.rs").matches(&relocated).count(),
        1
    );
}

mod ours_ci_scope {
    use super::ours_std as std;
    pub(crate) fn nested_scope_correction_retains_complete_original_hostile_and_loan_controls() {
        use sha2::{Digest, Sha256};
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
        let evidence = root.join("docs/evidence/nested-byte-traversal-780");
        let record: serde_json::Value = serde_json::from_str(
            &std::fs::read_to_string(evidence.join("ci-scope-original.json")).unwrap(),
        )
        .unwrap();
        let original =
            std::fs::read_to_string(evidence.join(record["archive"].as_str().unwrap())).unwrap();
        assert_eq!(
            format!(
                "{:x}",
                crate::digest_hex::LowerHex(Sha256::digest(original.as_bytes()))
            ),
            record["sha256"].as_str().unwrap()
        );
        let raw_current =
            std::fs::read_to_string(root.join(record["path"].as_str().unwrap())).unwrap();
        let current = super::source_composition::incoming_current(&raw_current)
            .expect("complete actual joined source and both immutable parent inputs");
        let restore = |source: &str| -> Option<String> {
            let mut result = source.to_owned();
            for edit in record["edits"].as_array().unwrap().iter().rev() {
                let new = edit["new"].as_str().unwrap();
                if result.matches(new).count() != 1 {
                    return None;
                }
                result = result.replacen(new, edit["old"].as_str().unwrap(), 1);
            }
            Some(result)
        };
        assert_eq!(restore(&current).as_deref(), Some(original.as_str()));
        assert_ne!(
            restore(&(current.clone() + "\n")).as_deref(),
            Some(original.as_str())
        );
        let start = original.find("    let freezes = sum\n").unwrap();
        let end = start
            + original[start..]
                .find("    for id in outer.shape.frozen {")
                .unwrap();
        assert_eq!(current.matches(&original[start..end]).count(), 1);
        let tail_end = end + original[end..].find("\n#[test]").unwrap();
        assert_eq!(current.matches(&original[end..tail_end]).count(), 1);
        let tampered = current.replacen(
            "after_inner.live_after.contains",
            "after_inner.live_before.contains",
            1,
        );
        assert_ne!(restore(&tampered).as_deref(), Some(original.as_str()));
        assert!(current.contains("let scope_metrics = ci_scope::empty_nested_budget_metrics();"));
        assert!(current.contains("assert_eq!(metrics, scope_metrics);"));
        assert!(current.contains("assert_eq!(metrics.allocated - scope_metrics.allocated, 0);"));
    }
}

mod source_composition {
    //! Full actual joined input first; immutable parent replays remain independent.
    //! No production receiver, source predicate, old archive or recipe is replaced.
    use super::ours_std as std;
    use serde_json::Value;
    use sha2::{Digest, Sha256};

    const PATH: &str = "src/hir/validation/byte_traversal/nested/tests.rs";
    const HELPER: &str = "src/hir/validation/byte_traversal/nested/tests/source_composition.rs";
    const EVIDENCE: &str = "docs/evidence/incoming-main-a9a75e1-20261016";
    const INCOMING_SHA: &str = "9a8b41dff3fd9bb3211e62f7180beabe0a0251da41266ed6a2a83e69f67af63f";
    const OURS_SHA: &str = "bb7ae50a94f03a334dab03412baeb1986a98d7e2d4694bcd033d6f03008cf431";
    const OURS940_SHA: &str = "16a6195ebf472b9d328c9da1f6267a400d3f34c783e39080641e8714219f7caf";
    const ORIGINAL940_SHA: &str =
        "7fe239c8c7c5afe12db2c3125286b45cf8b168cac45a408d0c443cc4ecfe5427";
    const BASE_SHA: &str = "4731dd5a006a7b5fdb906795a01c61045f183bf03efce70f7fbfc7164e17b145";
    const LIVE: &str =
        include_str!("../../docs/evidence/mainbcc-nested-custody-20261017/ours-tests.before.txt");
    const SELF: &str =
        include_str!("../../docs/evidence/mainbcc-nested-custody-20261017/ours-helper.before.txt");
    const RECIPE: &str = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/docs/evidence/mainbcc-nested-custody-20261017/ours-composition-recipe.json"
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
        if row["after_sha256"].as_str()? != digest(source)
            || row["before_sha256"].as_str()? != expected
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
        if row["archive"]
            != "docs/evidence/nested-byte-actual-fixture-correction-940/original/0.before"
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
    pub(crate) fn joined_nested_source_keeps_full_live_union_and_both_complete_parent_custodies() {
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
    pub(crate) fn joined_nested_source_refuses_foreign_truncated_drifted_predicates_and_partial_recipes(
    ) {
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
}

pub(crate) fn nested_scope_correction_retains_complete_original_hostile_and_loan_controls() {
    ours_ci_scope::nested_scope_correction_retains_complete_original_hostile_and_loan_controls();
}
pub(crate) fn nested_source_incoming_current(raw: &str) -> Option<String> {
    authenticate()?;
    if raw != LIVE_HIR {
        return None;
    }
    source_composition::incoming_current(OURS_TESTS)
}
pub(crate) use source_composition::joined_nested_source_keeps_full_live_union_and_both_complete_parent_custodies as nested_source_keeps_full_live_union_and_both_complete_parent_custodies;
pub(crate) use source_composition::joined_nested_source_refuses_foreign_truncated_drifted_predicates_and_partial_recipes as nested_source_refuses_foreign_truncated_drifted_predicates_and_partial_recipes;

#[test]
fn complete_joined_nested_domains_preserve_both_full_receivers_and_refuse_foreign_inputs() {
    authenticate().expect("all complete actual joined HIR/helper/reader sources");
    ci_scope::nested_scope_correction_retains_complete_original_hostile_and_loan_controls();
    ours_ci_scope::nested_scope_correction_retains_complete_original_hostile_and_loan_controls();
    source_composition::joined_nested_source_keeps_full_live_union_and_both_complete_parent_custodies();
    source_composition::joined_nested_source_refuses_foreign_truncated_drifted_predicates_and_partial_recipes();
    let original = include_str!(
        "../../docs/evidence/mainbcc-nested-custody-20261017/ours-function.before.txt"
    );
    assert_eq!(
        LIVE_READER
            .matches(&original.replacen("    fn ", "    pub(crate) fn ", 1))
            .count(),
        1
    );
    for changed in [
        OURS_TESTS.to_owned(),
        MAIN_TESTS.to_owned(),
        LIVE_HIR[..LIVE_HIR.len() - 1].to_owned(),
        format!("{LIVE_HIR}\n"),
    ] {
        assert!(complete_inputs(&changed, LIVE_HELPER, LIVE_READER).is_none());
        assert!(nested_source_incoming_current(&changed).is_none());
    }
    assert!(complete_inputs(LIVE_HIR, OURS_HELPER, LIVE_READER).is_none());
    assert!(complete_inputs(LIVE_HIR, LIVE_HELPER, "foreign reader").is_none());
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    assert!(domain_read(false, &root.join("Cargo.toml")).is_err());
    assert!(domain_read(true, &root.join("src/../Cargo.toml")).is_err());
}
