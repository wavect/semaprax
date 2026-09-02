//! Non-binding statements must not terminate structural inventory discovery.
//! These are compiler/graph gates, not target allocation or loop execution.
use semaprax::cleanup::{CleanupStorageId, CleanupStorageOrigin};
use semaprax::{codegen, format, graph, hir, wasm};

#[derive(Clone, Copy)]
enum Origin {
    Temporary(&'static str),
    Binding(&'static str),
    Result,
}

use Origin::{Binding, Result as Provisional, Temporary};

// Literal structural paths are the oracle; do not derive them by walking the
// same HIR children or rebuilding the cleanup inventory under test.
const TAIL: &[Origin] = &[Temporary("body.tail"), Temporary("body"), Provisional];
const BOUND: &[Origin] = &[
    Temporary("body.s2.value"),
    Binding("body.s2"),
    Temporary("body"),
    Provisional,
];

fn identity(kind: &str, path: &str) -> String {
    format!("declaration:4:case:{kind}:{}:{path}", path.len())
}

fn source(body: &str) -> String {
    format!(
        "module test.nonbinding_inventory;\npermit {{ unsafe }}\n\
         @id(\"case\") fn value(input: borrow Slice<u8>) -> Bytes {{ {body} }}\n\
         @id(\"main\") fn main() -> i64 {{ 0 }}\n"
    )
}

fn checked(body: &str, expected: &[Origin]) -> hir::ResolvedProgram {
    let source = source(body);
    let parsed = semaprax::check(&source, "nonbinding-inventory.spx").unwrap();
    let canonical = format::canonical(&parsed);
    let reparsed = semaprax::check(&canonical, "canonical.spx").unwrap();
    assert_eq!(format::canonical(&reparsed), canonical);
    let program = hir::resolve(&parsed).unwrap();
    hir::validate(&program).unwrap();
    let round_trip = hir::resolve(&reparsed).unwrap();

    let function = program
        .functions
        .iter()
        .find(|f| f.id.as_str() == "case")
        .unwrap();
    let round_trip_function = round_trip
        .functions
        .iter()
        .find(|f| f.id.as_str() == "case")
        .unwrap();
    // Source spans may change under formatting; ownership identities and plans
    // must not. Compare the semantic projections rather than span-bearing HIR.
    assert_eq!(function.cleanup, round_trip_function.cleanup);
    assert_eq!(function.cleanup_plan, round_trip_function.cleanup_plan);
    assert!(function
        .cleanup
        .entry_state
        .live_owned_parameters
        .is_empty());
    assert_eq!(function.cleanup.slots.len(), expected.len());
    for (index, (slot, expected)) in function.cleanup.slots.iter().zip(expected).enumerate() {
        assert_eq!(slot.id, CleanupStorageId(index as u32));
        assert_eq!(slot.discovery_index, index as u32);
        assert_eq!(slot.ty, hir::ResolvedType::Bytes);
        match (&slot.origin, expected) {
            (CleanupStorageOrigin::Temporary { expression }, Temporary(path)) => {
                assert_eq!(expression.as_str(), identity("expression", path));
            }
            (CleanupStorageOrigin::Binding { value }, Binding(path)) => {
                assert_eq!(value.as_str(), identity("value:local", path));
            }
            (CleanupStorageOrigin::ProvisionalResult { value }, Provisional) => {
                assert_eq!(value.as_str(), identity("value:result", ""));
            }
            _ => panic!("wrong structural origin at slot {index}: {:?}", slot.origin),
        }
    }

    let rendered = graph::to_json(&parsed).unwrap();
    assert_eq!(rendered, graph::to_json(&reparsed).unwrap());
    let document: serde_json::Value = serde_json::from_str(&rendered).unwrap();
    let node = document["nodes"]
        .as_array()
        .unwrap()
        .iter()
        .find(|node| node["id"] == "case" && node["kind"] == "function")
        .unwrap();
    let slots = node["cleanup"]["slots"].as_array().unwrap();
    assert!(slots.len() >= expected.len());
    for (index, expected) in expected.iter().enumerate() {
        let storage = match expected {
            Temporary(path) => {
                serde_json::json!({"kind":"temporary", "expression":identity("expression", path)})
            }
            Binding(path) => {
                serde_json::json!({"kind":"value", "value":identity("value:local", path)})
            }
            Provisional => serde_json::json!({"kind":"provisional_result"}),
        };
        assert_eq!(slots[index]["storage_index"], index);
        assert_eq!(slots[index]["storage"], storage);
    }
    // Every subject carries Bytes/Slice<u8>; Portable Indexed Byte Data v1
    // gives v17 precedence over the lower v15 while-node extension.
    assert_eq!(document["schema"], "semaprax.graph.v17");
    if body.contains("while ") {
        assert!(rendered.contains("\"kind\":\"while\""));
    }
    if body.contains("@audit") {
        assert!(rendered.contains("\"kind\":\"unsafe\""));
        assert!(rendered.contains("\"audit\":\"scalar inventory witness\""));
    }
    codegen::emit_hir_c(&program).unwrap();
    program
}

fn after(statement: &str) {
    checked(
        &format!("let mut counter = 0; {statement} bytes_copy(input)"),
        TAIL,
    );
    checked(
        &format!("let mut counter = 0; {statement} let bytes = bytes_copy(input); bytes"),
        BOUND,
    );
}

#[test]
fn assignment_preserves_later_owned_initializers_and_tail() {
    after("counter = counter + 1;");
}

#[test]
fn while_preserves_later_owned_initializers_and_tail() {
    after("while counter < 2 { counter = counter + 1; 0 }");
}

#[test]
fn unsafe_boundary_preserves_later_owned_initializers_and_tail() {
    after("@audit(\"scalar inventory witness\") unsafe { counter = counter + 1; 0 }");
}

const MIXED: &str = r#"
    let mut counter = 0;
    counter = counter + 1;
    while counter < 2 { counter = counter + 1; 0 }
    @audit("scalar inventory witness") unsafe { counter = counter + 1; 0 }
    let first = bytes_copy(input);
    let second = {
        let mut inner = 0;
        inner = inner + 1;
        while inner < 2 { inner = inner + 1; 0 }
        @audit("scalar inventory witness") unsafe { 0 }
        let copy = bytes_copy(input);
        copy
    };
    second
"#;

const MIXED_ORIGINS: &[Origin] = &[
    Temporary("body.s4.value"),
    Binding("body.s4"),
    Temporary("body.s5.value.s4.value"),
    Binding("body.s5.value.s4"),
    Temporary("body.s5.value"),
    Binding("body.s5"),
    Temporary("body"),
    Provisional,
];

#[test]
fn mixed_and_nested_nonbinding_statements_keep_exact_discovery_order() {
    checked(MIXED, MIXED_ORIGINS);
}

#[test]
fn later_owned_inventory_is_not_repaired_after_hostile_mutation() {
    let program = checked(MIXED, MIXED_ORIGINS);
    for mutation in ["missing", "reordered", "extra"] {
        let mut hostile = program.clone();
        let inventory = &mut hostile
            .functions
            .iter_mut()
            .find(|f| f.id.as_str() == "case")
            .unwrap()
            .cleanup;
        match mutation {
            "missing" => {
                inventory.slots.remove(2);
            }
            // Preserve slot numbering/shapes and change only ownership origins,
            // so this is not merely a duplicate-ID rejection.
            "reordered" => {
                let first = inventory.slots[0].origin.clone();
                inventory.slots[0].origin = inventory.slots[2].origin.clone();
                inventory.slots[2].origin = first;
            }
            "extra" => {
                let mut extra = inventory.slots[0].clone();
                extra.id = CleanupStorageId(inventory.slots.len() as u32);
                extra.discovery_index = extra.id.0;
                inventory.slots.push(extra);
            }
            _ => unreachable!(),
        }
        assert_eq!(
            hir::validate(&hostile).unwrap_err().code,
            "SPX-H006",
            "{mutation}"
        );
        assert_eq!(
            codegen::emit_hir_c(&hostile).unwrap_err().code,
            "SPX-H006",
            "{mutation}"
        );
        assert_eq!(
            wasm::emit_resolved_module(&hostile).unwrap_err().code,
            "SPX-H006",
            "{mutation}"
        );
    }
}
