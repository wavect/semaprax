//! Authored marker spellings remain nominal source identities on every backend.
use super::*;

#[test]
fn marker_like_schema_names_keep_exact_derivation_and_detached_owned_behavior() {
    let name = "__ROW_ID____BOUND__";
    let id = "orders.__BOUND__.__ROW_ID__";
    let root = install_named("nested-order-template-names", name, id);
    let source = std::fs::read_to_string(root.join("src/schema.spx")).unwrap();
    let parsed = parse(&source, "schema.spx").unwrap();
    let text = parsed
        .functions
        .iter()
        .find(|function| function.name == format!("json_{name}_nested_text"))
        .unwrap();
    assert_eq!(text.stable_id, format!("{id}.json.nested.text"));
    assert!(parsed
        .types
        .iter()
        .any(|record| record.name == name && record.stable_id == id));
    // install_named already checks original-source retention and policy mismatch;
    // qualify_named checks detached owners, Unicode/NUL, and the independent 729 oracle.
    qualify_named(&root, id);
    std::fs::remove_dir_all(root).unwrap();
}
