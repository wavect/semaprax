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
Self::Compact7 => Spec { data: include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/docs/evidence/main8a-historical-readers-257-268-20261016/Compact7.json")), length: 1057998, sha256: [165, 249, 42, 209, 8, 113, 36, 86, 60, 151, 44, 92, 36, 232, 73, 56, 136, 87, 177, 54, 4, 205, 68, 60, 129, 41, 153, 73, 125, 136, 61, 203], rows: 7 },
Self::Receivers12 => Spec { data: include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/docs/evidence/main8a-historical-readers-257-268-20261016/Receivers12.json")), length: 2096990, sha256: [100, 42, 173, 43, 81, 205, 7, 158, 201, 140, 95, 172, 42, 49, 235, 198, 139, 124, 82, 87, 249, 186, 119, 140, 162, 45, 165, 27, 64, 28, 139, 253], rows: 12 },
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
