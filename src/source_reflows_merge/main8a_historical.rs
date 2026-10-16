//! Whole-current input views used only by two historical Main8a metadata readers.
//! Live source inventories continue to read complete raw sources directly.
use sha2::{Digest, Sha256};

#[derive(Clone, Copy)]
pub(crate) enum Domain { Compact7, Receivers12 }
#[derive(Clone, Copy)]
struct Spec { data: &'static str, length: usize, sha256: [u8; 32], rows: usize }
impl Domain {
    fn spec(self) -> Spec {
        match self {
            // SPECS_BEGIN
Self::Compact7 => Spec { data: include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/docs/evidence/main8a-historical-readers-257-268-20261016/Compact7.json")), length: 1045784, sha256: [150, 110, 196, 5, 220, 215, 206, 82, 195, 198, 234, 13, 71, 240, 29, 138, 35, 161, 140, 215, 147, 233, 6, 225, 173, 23, 71, 191, 152, 234, 190, 7], rows: 7 },
Self::Receivers12 => Spec { data: include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/docs/evidence/main8a-historical-readers-257-268-20261016/Receivers12.json")), length: 2081589, sha256: [84, 171, 139, 209, 114, 112, 100, 206, 3, 195, 174, 129, 113, 10, 217, 78, 237, 134, 46, 117, 153, 188, 117, 226, 168, 116, 88, 232, 43, 69, 169, 35], rows: 12 },
            // SPECS_END
        }
    }
}
fn checked(spec: Spec, data: &str, path: &str, current: &str) -> String {
    assert_eq!(data.len(), spec.length, "complete historical input metadata");
    assert_eq!(<[u8; 32]>::from(Sha256::digest(data.as_bytes())), spec.sha256);
    let rows: serde_json::Value = serde_json::from_str(data).unwrap();
    let rows = rows.as_array().unwrap();
    assert_eq!(rows.len(), spec.rows);
    assert_eq!(rows.iter().filter(|r| r["source"] == path).count(), 1,
        "registered historical input domain");
    super::project(data, path, current)
}
pub(crate) fn input(domain: Domain, path: &str, current: &str) -> String {
    let spec = domain.spec();
    checked(spec, spec.data, path, current)
}

#[cfg(test)]
mod tests {
    use super::*;
    fn current(path: &str) -> &'static str {
        match path {
            // INPUTS_BEGIN
"src/cleanup_plan/replay.rs" => include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/cleanup_plan/replay.rs")),
"src/cleanup_plan/replay/branch_clone/tests.rs" => include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/cleanup_plan/replay/branch_clone/tests.rs")),
"src/cleanup_plan/replay/factored.rs" => include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/cleanup_plan/replay/factored.rs")),
"src/cleanup_plan/replay/factored/borrowed_canonical/dictionary/keys.rs" => include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/cleanup_plan/replay/factored/borrowed_canonical/dictionary/keys.rs")),
"src/cleanup_plan/replay/factored/borrowed_canonical/tests.rs" => include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/cleanup_plan/replay/factored/borrowed_canonical/tests.rs")),
"src/cleanup_plan/replay/factored/oracle_hook.rs" => include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/cleanup_plan/replay/factored/oracle_hook.rs")),
"src/hir.rs" => include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/hir.rs")),
"src/hir/expr_nodes.rs" => include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/hir/expr_nodes.rs")),
"src/hir/validation.rs" => include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/hir/validation.rs")),
"src/source_verify/declaration.rs" => include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/source_verify/declaration.rs")),
"tests/hir_module_boundaries.rs" => include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/tests/hir_module_boundaries.rs")),
"tests/workspace/semantic_graph/source_inventory.rs" => include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/tests/workspace/semantic_graph/source_inventory.rs")),
            // INPUTS_END
            _ => panic!("unregistered complete test input"),
        }
    }
    #[test]
    fn complete_historical_domains_refuse_changed_truncated_foreign_and_stale_inputs() {
        for domain in [Domain::Compact7, Domain::Receivers12] {
            let rows: serde_json::Value = serde_json::from_str(domain.spec().data).unwrap();
            for row in rows.as_array().unwrap() {
                let path = row["source"].as_str().unwrap();
                let raw = current(path);
                let historical = input(domain, path, raw);
                let changed = format!("{raw} ");
                let truncated = raw.get(..raw.len() / 2).unwrap_or("");
                for wrong in [changed.as_str(), "", truncated] {
                    assert!(std::panic::catch_unwind(|| input(domain, path, wrong)).is_err());
                }
                if historical != raw {
                    assert!(std::panic::catch_unwind(|| input(domain, path, &historical)).is_err());
                }
            }
            assert!(std::panic::catch_unwind(|| input(domain, "src/foreign.rs", "foreign")).is_err());
            let data = domain.spec().data;
            assert!(std::panic::catch_unwind(|| checked(domain.spec(), &data[..data.len()-1], "src/foreign.rs", "")).is_err());
            let changed = data.replacen('s', "t", 1);
            assert!(std::panic::catch_unwind(|| checked(domain.spec(), &changed, "src/foreign.rs", "")).is_err());
        }
    }
}
