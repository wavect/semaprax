//! Authenticated whole-source views for the unchanged original inverse guards.
use sha2::{Digest, Sha256};

pub(super) fn project(source: &str, current: &str) -> String {
    let held = crate::cleanup_plan::replay::physical_budget::authentication::previous(source, current);
    let current = held.as_str();
    let merged = crate::source_reflows_merge::ours(source, current);
    let current = merged.as_str();
    let rows: serde_json::Value = serde_json::from_str(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/docs/evidence/source36-after-next-838-839-20261015/source-projections.json"
    )))
    .expect("fixed current source projections");
    let rows = rows.as_array().expect("source projection array");
    let matching: Vec<_> = rows.iter().filter(|row| row["source"] == source).collect();
    if matching.is_empty() {
        return current.to_owned();
    }
    assert_eq!(matching.len(), 1, "unique authenticated source view");
    let row = matching[0];
    assert_eq!(
        format!(
            "{:x}",
            crate::digest_hex::LowerHex(Sha256::digest(current.as_bytes()))
        ),
        row["current_sha256"]
    );
    let mut projected = current.to_owned();
    for edit in row["edits"].as_array().expect("complete source inverse") {
        let new = edit["new"].as_str().expect("current source bytes");
        let old = edit["old"].as_str().expect("checkpoint source bytes");
        assert_eq!(projected.matches(new).count(), 1, "unique complete inverse");
        projected = projected.replacen(new, old, 1);
    }
    assert_eq!(
        format!(
            "{:x}",
            crate::digest_hex::LowerHex(Sha256::digest(projected.as_bytes()))
        ),
        row["original_sha256"]
    );
    projected
}

// These two production promotions precede the original complete #810 inverses.
// Authenticate the entire live module, restore only its exact cfg/export edit,
// and leave every original inverse and full-source assertion in a/b unchanged.
pub(super) fn before_production_promotion(source: &str, current: &str) -> String {
    let acquired;
    let current = if source == "src/hir/lexical_exit.rs" {
        acquired = lexical_before_later_promotions(current);
        acquired.as_str()
    } else { current };
    let rows: serde_json::Value = serde_json::from_str(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/docs/evidence/fade-custody-promotions-20261017/bindings.json"
    )))
    .expect("fixed complete promotion bindings");
    let rows = rows.as_array().expect("promotion array");
    assert_eq!(rows.len(), 2, "closed promotion domain");
    let matching: Vec<_> = rows.iter().filter(|row| row["source"] == source).collect();
    assert_eq!(matching.len(), 1, "one exact production promotion subject");
    let row = matching[0];
    let digest = |bytes: &str| {
        format!(
            "{:x}",
            crate::digest_hex::LowerHex(Sha256::digest(bytes.as_bytes()))
        )
    };
    assert_eq!(
        digest(current),
        row["current_sha256"],
        "whole production module"
    );
    let new = row["new"].as_str().expect("complete promoted bytes");
    let old = row["old"].as_str().expect("complete previous bytes");
    assert_eq!(current.matches(new).count(), 1, "unique promotion boundary");
    let restored = current.replacen(new, old, 1);
    assert_eq!(
        digest(&restored),
        row["prior_sha256"],
        "whole previous module"
    );
    restored
}

#[test]
fn production_promotion_views_restore_whole_modules_and_reject_drift() {
    let subjects = [
        (
            "src/hir/function_value.rs",
            include_str!("../../../../hir/function_value.rs"),
            include_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/docs/evidence/fade-custody-promotions-20261017/src__hir__function_value.rs.prior"
            )),
        ),
        (
            "src/hir/lexical_exit.rs",
            include_str!("../../../../hir/lexical_exit.rs"),
            include_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/docs/evidence/fade-custody-promotions-20261017/src__hir__lexical_exit.rs.prior"
            )),
        ),
    ];
    for (source, current, prior) in subjects {
        assert_eq!(before_production_promotion(source, current), prior);
        for hostile in [
            format!("{current}\n"),
            current.replacen("pub(crate)", "pub(super)", 1),
            prior.to_owned(),
        ] {
            assert!(
                std::panic::catch_unwind(|| before_production_promotion(source, &hostile)).is_err()
            );
        }
        assert!(
            std::panic::catch_unwind(|| before_production_promotion("foreign.rs", current))
                .is_err()
        );
    }
}

// Closed compiled-source input before the original two-row promotion domain.
// Current production exports remain live; old promotion hashes stay unchanged.
fn lexical_before_later_promotions(current: &str) -> String {
    const RAW: &str = include_str!(concat!(env!("CARGO_MANIFEST_DIR"),
        "/docs/evidence/c874-lexical-promotion-input-20261017/lexical-exit.c874-current.rs.txt"));
    const BEFORE: &str = include_str!(concat!(env!("CARGO_MANIFEST_DIR"),
        "/docs/evidence/c874-lexical-promotion-input-20261017/lexical-exit.2d87-before.rs.txt"));
    let sha = |text: &str| format!("{:x}", crate::digest_hex::LowerHex(Sha256::digest(text.as_bytes())));
    assert_eq!(RAW.len(), 6_956);
    assert_eq!(sha(RAW), "246cb0df50653502b3cf26ab9348e6db14508dad14c870852a22aab95db1e05d");
    assert_eq!(current.len(), RAW.len(), "whole later lexical module length");
    assert_eq!(sha(current), sha(RAW), "whole later lexical module");
    assert_eq!(current, RAW, "complete compiled-source input");
    assert_eq!(BEFORE.len(), 6_885);
    assert_eq!(sha(BEFORE), "4fcfdef9e15555a8356841b45746e23650e5811f115fe3f40c1c43914aa9d9a9");
    BEFORE.to_owned()
}

#[test]
fn lexical_later_promotions_retain_complete_original_input_and_refuse_foreign_stages() {
    let current = include_str!("../../../../hir/lexical_exit.rs");
    let before = include_str!(concat!(env!("CARGO_MANIFEST_DIR"),
        "/docs/evidence/c874-lexical-promotion-input-20261017/lexical-exit.2d87-before.rs.txt"));
    assert_eq!(lexical_before_later_promotions(current), before);
    let prior = include_str!(concat!(env!("CARGO_MANIFEST_DIR"),
        "/docs/evidence/fade-custody-promotions-20261017/src__hir__lexical_exit.rs.prior"));
    assert_eq!(before_production_promotion("src/hir/lexical_exit.rs", current), prior);
    for bad in [String::new(), current[..current.len()-1].to_owned(), format!("{current}\n"),
        current.replacen("pub(crate)", "pub(super)", 1), before.to_owned(), prior.to_owned(),
        include_str!("../../../../hir/function_value.rs").to_owned(), current.replace('\n', "\r\n")] {
        assert!(std::panic::catch_unwind(|| lexical_before_later_promotions(&bad)).is_err());
        assert!(std::panic::catch_unwind(|| before_production_promotion("src/hir/lexical_exit.rs", &bad)).is_err());
    }
    assert!(std::panic::catch_unwind(|| before_production_promotion("foreign.rs", current)).is_err());
}
