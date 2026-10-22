//! Complete immediate-source custody and invertible independent receiver glue.
use sha2::{Digest, Sha256};

const ROOT: &str = "docs/evidence/borrowed-top-level-identity-785-20261017/";
const INVERSES: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/docs/evidence/borrowed-top-level-identity-785-20261017/inverse-edits.json"
));

fn digest(text: &str) -> String {
    format!(
        "{:x}",
        crate::digest_hex::LowerHex(Sha256::digest(text.as_bytes()))
    )
}

fn restore(source: &str, current: &str) -> String {
    let acquired = crate::source_reflows_merge::main_e177_final_inputs::combined_side_value(
        source, "whole785", current,
    );
    let current = acquired.as_str();
    let record: serde_json::Value = serde_json::from_str(INVERSES).unwrap();
    let rows = record["files"].as_array().unwrap();
    let row = rows
        .iter()
        .find(|row| row["path"] == source)
        .expect("finite 785 input");
    assert_eq!(digest(current), row["after_sha256"].as_str().unwrap());
    assert_eq!(current, row["after"].as_str().unwrap());
    let before = row["before"].as_str().unwrap();
    assert_eq!(digest(before), row["before_sha256"].as_str().unwrap());
    before.to_owned()
}

#[test]
fn complete_immediate_sources_and_old_clone_and_both_callers_are_exactly_preserved() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let record: serde_json::Value = serde_json::from_str(INVERSES).unwrap();
    for row in record["files"].as_array().unwrap() {
        let source = row["path"].as_str().unwrap();
        let current = std::fs::read_to_string(root.join(source)).unwrap();
        let before = restore(source, &current);
        let archived =
            std::fs::read_to_string(root.join(ROOT).join(row["archive"].as_str().unwrap()))
                .unwrap();
        assert_eq!(before, archived, "whole predecessor {source}");
        assert!(std::panic::catch_unwind(|| restore(source, &(current.clone() + "\n"))).is_err());
        assert!(std::panic::catch_unwind(|| restore(source, &before)).is_err());
    }
    let snapshot = |path: &str| {
        std::fs::read_to_string(
            root.join(ROOT)
                .join("whole-before")
                .join(format!("{path}.txt")),
        )
        .unwrap()
    };
    assert_eq!(
        include_str!("before785.rs").replace(
            "pub(in crate::workspace_graph) fn top_level_declaration(",
            "pub(super) fn top_level_declaration("
        ),
        snapshot("src/workspace_graph/shape_identity.rs")
    );
    let old_validation = snapshot("src/workspace_graph/validation.rs");
    let rebuilt_validation = include_str!("before_validation785.rs")
        .replace("#[path = \"../validation/before_roots.rs\"]\n", "")
        .replace("\nuse super::before785::top_level_declaration;\n", "")
        .replace(
            "    graph_error, prelude_binding, reserve_builder_structure,\n",
            "    graph_error, prelude_binding, reserve_builder_structure, top_level_declaration,\n",
        );
    assert_eq!(rebuilt_validation, old_validation);
    assert_eq!(
        std::fs::read_to_string(root.join("src/workspace_graph/validation.rs")).unwrap(),
        old_validation
    );
    let old_root = snapshot("src/workspace_graph.rs");
    assert_eq!(
        std::fs::read_to_string(root.join("src/workspace_graph.rs")).unwrap(),
        old_root
    );
    let start = old_root
        .find("fn reconstruct_workspace_declaration_facts_with_authority(")
        .unwrap();
    let end = start + 3 + old_root[start + 3..].find("\nfn ").unwrap();
    let expected = format!(
        "use super::*;\nuse super::before785::top_level_declaration;\npub(super) {}\n",
        &old_root[start..end]
    );
    assert_eq!(include_str!("before_caller785.rs"), expected);
    // The new view cannot quietly reprice or alter the original allocating API.
    let old_budget = snapshot("src/bounded_output.rs");
    let current_budget = std::fs::read_to_string(root.join("src/bounded_output.rs")).unwrap();
    let clone = |s: &str| {
        s.split("pub(crate) fn budgeted_clone(value: &str) -> String {")
            .nth(1)
            .unwrap()
            .split("pub(crate) trait BudgetedJoin")
            .next()
            .unwrap()
            .to_owned()
    };
    assert_eq!(clone(&current_budget), clone(&old_budget));
    assert!(current_budget.starts_with(&old_budget));
}

#[test]
fn all_five_test_children_remain_joined_under_the_original_raw_source_owner() {
    let inventories = [
        (include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/tests/workspace/semantic_graph/source_inventory.rs")), "../../"),
        (include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/tests/hir_module_boundaries.rs")), "../"),
        (include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/crates/semaprax-native-rust-interop-builder/src/implementation/tests/ledger_capacity/source_inventory.rs")), "../../../../../"),
    ];
    for (inventory, prefix) in inventories {
        for child in [
            "before785.rs",
            "before_caller785.rs",
            "before_validation785.rs",
            "source_controls.rs",
            "tests.rs",
        ] {
            let literal =
                format!("include_str!(\"{prefix}src/workspace_graph/shape_identity/{child}\")");
            assert_eq!(
                inventory.matches(&literal).count(),
                1,
                "complete raw owner {prefix}{child}"
            );
        }
    }
    let source = include_str!("../shape_identity.rs");
    assert!(source.contains("TopLevelDeclaration(crate::bounded_output::borrowed_clone_view(\n        current.id.as_str(),\n    ))"));
    assert!(!source.contains("DeclarationId::new("));
    assert!(!source.contains("budgeted_clone("));
}
