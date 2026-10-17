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
// Only historical source expressions receive the distinct frozen views.
macro_rules! include_str {
    ("../before_paid_many.rs") => { &paid_view(&paid_rows(), "src/cleanup_plan/replay/factored/canonical_shared/before_paid_many.rs", ::std::include_str!("../before_paid_many.rs")) };
    ("../../canonical_shared.rs") => { &paid_view(&paid_rows(), "src/cleanup_plan/replay/factored/canonical_shared.rs", ::std::include_str!("../../canonical_shared.rs")) };
    ($path:literal) => { ::std::include_str!($path) };
}

#[test]
fn complete_immediate_canonicalizer_caller_terminal_helpers_and_all_original_tests_are_exact() {
    use self::historical_std as std;
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
    // The historical whole-module equality above remains exact. Every actual
    // production receiving/order/predicate assertion below uses RAW shipping
    // source, independently authenticated by paid_view before it yields a view.
    let current = ::std::include_str!("../../canonical_shared.rs");
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

#[path = "../../../../../workspace_graph/identity_resolution/base/cache_billing/main7f8_reader.rs"]
mod paid_reader;
const PAID_PREFIX: &str = "docs/evidence/c874-supplement-custody-domains-20261017";
fn paid_rows() -> serde_json::Value {
    let path = ::std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join(PAID_PREFIX).join("paid-inputs.json");
    let value = paid_reader::read(&path, PAID_INPUT_LENGTH, PAID_INPUT_SHA, 2_097_152).unwrap();
    let rows: serde_json::Value = serde_json::from_str(&value).unwrap();
    assert_eq!(rows.as_array().unwrap().len(), 7);
    rows
}
fn paid_archive(row: &serde_json::Value) -> String {
    paid_reader::read(&::std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join(PAID_PREFIX).join(row["archive"].as_str().unwrap()), usize::try_from(row["length"].as_u64().unwrap()).unwrap(), row["sha256"].as_str().unwrap(), 2_097_152).unwrap()
}
fn paid_view(rows: &serde_json::Value, path: &str, current: &str) -> String {
    assert_eq!(rows, &paid_rows(), "whole sealed paid-many source domain");
    let rows = rows.as_array().unwrap();
    assert_eq!(rows.iter().filter(|r| r["path"] == path).count(), 1);
    let row = rows.iter().find(|r| r["path"] == path).unwrap();
    let field = &row["current"];
    let raw = paid_reader::read(&::std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join(path), usize::try_from(field["length"].as_u64().unwrap()).unwrap(), field["sha256"].as_str().unwrap(), 2_097_152).unwrap();
    assert_eq!(raw, paid_archive(field), "complete independently archived current source");
    assert_eq!(current, raw, "complete actual paid-many source input");
    paid_archive(&row["view"])
}
mod historical_std {
    pub use ::std::path;
    pub mod fs {
        pub fn read(path: impl AsRef<::std::path::Path>) -> ::std::io::Result<Vec<u8>> {
            let root = ::std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
            let path = path.as_ref();
            let logical = path.strip_prefix(root).ok().and_then(|p| p.to_str()).ok_or_else(|| ::std::io::Error::new(::std::io::ErrorKind::InvalidInput, "foreign paid-many source root"))?;
            let rows = super::super::paid_rows();
            if let Some(row) = rows.as_array().unwrap().iter().find(|r| r["path"] == logical) {
                let raw = super::super::paid_archive(&row["current"]);
                return Ok(super::super::paid_view(&rows, logical, &raw).into_bytes());
            }
            // The remaining original helper reads retain their fixed115-key
            // domain and original full byte/hash checks, with bounded IO.
            let recipe: serde_json::Value = serde_json::from_str(super::super::RECIPE).unwrap();
            let helpers = recipe["helpers"].as_array().unwrap();
            assert_eq!(helpers.len(), 115);
            let row = helpers.iter().find(|r| r["path"] == logical).ok_or_else(|| ::std::io::Error::new(::std::io::ErrorKind::InvalidInput, "unknown paid-many helper"))?;
            Ok(super::super::paid_reader::read(path, usize::try_from(row["length"].as_u64().unwrap()).unwrap(), row["sha256"].as_str().unwrap(), 2_097_152).map_err(|_| ::std::io::Error::new(::std::io::ErrorKind::InvalidData,"whole unchanged paid-many helper"))?.into_bytes())
        }
    }
}

#[test]
fn complete_paid_many_source_domains_keep_all_original_inputs_and_raw_production_checks() {
    paid_reader::controls();
    let rows = paid_rows();
    for row in rows.as_array().unwrap() {
        let path = row["path"].as_str().unwrap();
        let raw = paid_archive(&row["current"]);
        let old = paid_archive(&row["view"]);
        assert_ne!(raw, old);
        assert_eq!(paid_view(&rows, path, &raw), old);
        for bad in [String::new(), raw[..raw.len()-1].to_owned(), format!("{raw} "), format!("{raw}{raw}"), old] {
            assert!(::std::panic::catch_unwind(|| paid_view(&rows, path, &bad)).is_err());
        }
        let mut forged = rows.clone(); forged[0]["view"]["sha256"] = serde_json::Value::String("0".repeat(64));
        assert!(::std::panic::catch_unwind(|| paid_view(&forged, path, &raw)).is_err());
    }
    let root = ::std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    assert!(historical_std::fs::read(root.join("foreign.rs")).is_err());
    complete_immediate_canonicalizer_caller_terminal_helpers_and_all_original_tests_are_exact();
    composed_production_prepays_both_real_promotion_search_and_finish_receivers();
}
const PAID_INPUT_LENGTH: usize = 4272;
const PAID_INPUT_SHA: &str = "abe19258899810c8aac85239cfb32e4ccde2b0623161959bed13d11e6e9e75b4";
