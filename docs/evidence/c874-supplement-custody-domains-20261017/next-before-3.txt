//! Complete immediate caller/body reconstruction; no closed helper replaces it.
use sha2::{Digest, Sha256};
const RECIPE: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/docs/evidence/opt870-inline-paid-many-51c1-20261017/frozen-receivers.json"
));
const OLD: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/docs/evidence/opt870-inline-paid-many-51c1-20261017/complete-canonical-before.rs.txt"
));
const CALLER: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/docs/evidence/opt870-inline-paid-many-51c1-20261017/complete-factored-before.rs.txt"
));
fn digest(bytes: &[u8]) -> String {
    format!("{:x}", crate::digest_hex::LowerHex(Sha256::digest(bytes)))
}
fn restore(frozen: &str, row: &serde_json::Value) -> String {
    let body = frozen
        .strip_prefix(row["header"].as_str().unwrap())
        .unwrap()
        .strip_suffix(row["suffix"].as_str().unwrap())
        .unwrap();
    let before = row["visibility_before"].as_str().unwrap();
    let after = row["visibility_after"].as_str().unwrap();
    assert_eq!(body.matches(after).count(), 1);
    body.replacen(after, before, 1)
}
#[test]
fn complete_immediate_canonicalizer_caller_terminal_helpers_and_all_original_tests_are_exact() {
    let recipe: serde_json::Value = serde_json::from_str(RECIPE).unwrap();
    assert_eq!(recipe["schema"], "semaprax.opt870.full-frozen-receivers.v1");
    assert_eq!(recipe["base"], "51c1dfc49962e046566ad21ec31f20c6b3a9bdae");
    let canonical = &recipe["canonical"];
    assert_eq!(digest(OLD.as_bytes()), canonical["sha256"]);
    let body = restore(include_str!("../before_paid_many.rs"), canonical);
    assert_eq!(
        body,
        OLD.get(
            canonical["start"].as_u64().unwrap() as usize
                ..canonical["end"].as_u64().unwrap() as usize
        )
        .unwrap()
    );
    let caller = &recipe["compare"];
    assert_eq!(digest(CALLER.as_bytes()), caller["sha256"]);
    assert_eq!(
        CALLER,
        include_str!("../../../factored.rs"),
        "whole actual comparison caller unchanged"
    );
    let body = restore(include_str!("../before_paid_many/compare.rs"), caller);
    assert_eq!(
        body,
        CALLER
            .get(
                caller["start"].as_u64().unwrap() as usize
                    ..caller["end"].as_u64().unwrap() as usize
            )
            .unwrap()
    );
    assert_eq!(
        include_str!("../before_paid_many/terminal_leaf.rs"),
        include_str!("../terminal_leaf.rs")
    );
    assert_eq!(
        digest(include_bytes!("../terminal_leaf.rs")),
        recipe["terminal_sha256"]
    );
    let helpers = recipe["helpers"].as_array().unwrap();
    assert_eq!(helpers.len(), 115);
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let mut previous = "";
    for helper in helpers {
        let path = helper["path"].as_str().unwrap();
        assert!(path > previous && path.starts_with("src/cleanup_plan/replay/factored/"));
        assert!(!path.contains(".."));
        previous = path;
        let bytes = std::fs::read(root.join(path)).unwrap();
        assert_eq!(bytes.len() as u64, helper["length"].as_u64().unwrap());
        assert_eq!(
            digest(&bytes),
            helper["sha256"],
            "all old helpers and test bodies: {path}"
        );
    }
}

#[test]
fn composed_production_prepays_both_real_promotion_search_and_finish_receivers() {
    let current = include_str!("../../canonical_shared.rs");
    let recipe: serde_json::Value = serde_json::from_str(RECIPE).unwrap();
    assert_eq!(
        digest(current.as_bytes()),
        recipe["canonical"]["current_sha256"]
    );
    for (phase, receiver, call) in [
        (
            "factored composed promotion receivers",
            "paid_many::PROMOTE_RECEIVING",
            "paid_many::promote(",
        ),
        (
            "factored composed flat add receivers",
            "paid_many::ADD_RECEIVING",
            "paid_many::add(",
        ),
        (
            "factored composed flat finish receivers",
            "paid_many::FINISH_RECEIVING",
            "own_weighted(function, budget, flat)",
        ),
    ] {
        assert_eq!(current.matches(phase).count(), 2);
        assert_eq!(current.matches(receiver).count(), 2);
        let mut offset = 0;
        for _ in 0..2 {
            let at = current[offset..].find(receiver).unwrap() + offset;
            let end = current[at..].find(call).unwrap() + at;
            assert!(current[at..end].contains(phase));
            assert!(
                current[at..end].contains("?;"),
                "entry denied before helper/Vec borrow is formed"
            );
            offset = end + call.len();
        }
    }
    let repeated = "// Same repeated-child checked_add before any later\n                                // target acquisition or error, as in the old map.\n                                let Some(sum) = total.checked_add(*weight) else {\n                                    return Ok(None);\n                                };\n                                *total = sum;";
    assert_eq!(OLD.matches(repeated).count(), 1);
    assert_eq!(current.matches(repeated).count(), 1);
    // The second production family has its original, shallower indentation.
    let repeated = repeated.replace(
        "\n                                ",
        "\n                            ",
    );
    assert_eq!(OLD.matches(&repeated).count(), 1);
    assert_eq!(current.matches(&repeated).count(), 1);
    assert_eq!(
        current
            .matches("if budget.pays_allocations() { paid_many::note_branch(")
            .count(),
        2
    );
    assert!(!include_str!("../before_paid_many.rs").contains("paid_many::Targets"));
    assert!(!include_str!("../successor_targets.rs").contains("PaidMany"));
}
