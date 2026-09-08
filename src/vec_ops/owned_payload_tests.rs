use super::*;
use serde_json::Value as JsonValue;

const SOURCE: &str = r#"module vec.owned;
@id("vec.owned.main") fn main() -> i64 {
 let input = [9u8];
 let mut values = vec_with_capacity<Bytes>(2usize);
 values = vec_push<Bytes>(values, bytes_copy(array_as_slice(input)));
 values = vec_set<Bytes>(values, 0usize, bytes_copy(array_as_slice(input)));
 values = vec_reserve_exact<Bytes>(values, 1usize);
 values = vec_clear<Bytes>(values);
 if vec_len<Bytes>(values) == 0usize && vec_capacity<Bytes>(values) == 2usize { 0 } else { 1 }
}
"#;

#[test]
fn owned_bytes_vec_has_consuming_mutations_and_closed_get() {
    let program = crate::check(SOURCE, "owned-vec.spx").unwrap();
    let canonical = crate::format::canonical(&program);
    let checked = crate::check(&canonical, "owned-vec.spx").unwrap();
    assert_eq!(canonical, crate::format::canonical(&checked));
    let resolved = crate::hir::resolve(&program).unwrap();
    crate::hir::validate(&resolved).unwrap();
    assert!(program_uses_owned_payload(&program));
    assert!(resolved_program_uses_owned_payload(&resolved));
    assert_eq!(
        crate::prelude::contract_bytes_v6(),
        include_bytes!("../../tests/fixtures/prelude-v6.contract")
    );
    let graph_bytes = crate::graph::to_json(&program).unwrap();
    crate::graph::verify_json(&program, &graph_bytes).unwrap();
    let graph: JsonValue = serde_json::from_str(&graph_bytes).unwrap();
    assert_eq!(graph["prelude"]["schema"], crate::prelude::SCHEMA_V6);
    assert_eq!(
        graph["prelude"]["digest"],
        "sha256:924f67b773e3dc4d26b3891fe2415b6e567842a2c07b02c8eae55ae5467c56c4"
    );
    let forged_v5 = graph_bytes.replacen("semaprax.prelude.v6", "semaprax.prelude.v5", 1);
    assert_ne!(forged_v5, graph_bytes);
    assert!(crate::graph::verify_json(&program, &forged_v5).is_err());
    assert_eq!(
        resolved_params(VecOp::Push, &ResolvedType::Bytes)[1].ownership,
        OwnershipMode::Own
    );
    assert_eq!(
        resolved_params(VecOp::Set, &ResolvedType::Bytes)[2].ownership,
        OwnershipMode::Own
    );
    assert!(!resolved_operation_element_is_admitted(
        VecOp::Get,
        &ResolvedType::Bytes
    ));
    let rejected = SOURCE.replace("vec_len<Bytes>(values)", "vec_get<Bytes>(values, 0usize)");
    let errors = crate::check(&rejected, "closed-vec-get.spx").unwrap_err();
    assert!(
        errors.iter().any(|error| error.code == "SPX-T281"),
        "{errors:?}"
    );
}

#[test]
fn lexical_owned_bytes_vec_selects_v6_without_a_vec_call() {
    let source = r#"module vec.signature;
@id("vec.signature.relay") fn relay(values: own Vec<Bytes>) -> Vec<Bytes> { values }
@id("vec.signature.main") fn main() -> i64 { 0 }
"#;
    let program = crate::check(source, "owned-vec-signature.spx").unwrap();
    assert!(program_uses_owned_payload(&program));
    let resolved = crate::hir::resolve(&program).unwrap();
    assert!(resolved_program_uses_owned_payload(&resolved));
    let graph: JsonValue = serde_json::from_str(&crate::graph::to_json(&program).unwrap()).unwrap();
    assert_eq!(graph["prelude"]["schema"], crate::prelude::SCHEMA_V6);
}

#[test]
fn owned_bytes_vec_record_storage_remains_closed() {
    let source = r#"module vec.field;
@id("vec.field.holder") record Holder {
 @id("vec.field.values") values: Vec<Bytes>,
}
@id("vec.field.main") fn main() -> i64 { 0 }
"#;
    let errors = crate::check(source, "owned-vec-field.spx").unwrap_err();
    assert!(
        errors.iter().any(|error| error.code == "SPX-T223"),
        "{errors:?}"
    );
}

#[test]
fn owned_bytes_vec_keeps_copy_traversal_closed() {
    let source = r#"module vec.closed_for;
@id("vec.closed_for.main") fn main() -> i64 {
 let values = vec_with_capacity<Bytes>(1usize);
 for item in values { 0 }
 0
}
"#;
    let errors = crate::check(source, "owned-vec-for.spx").unwrap_err();
    assert!(
        errors.iter().any(|error| error.code == "SPX-T284"),
        "{errors:?}"
    );
}
