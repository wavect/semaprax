use super::*;

const SOURCE: &str = r#"module box.owned;
@id("box.owned.main") fn main()->i64 {
 let a=[9u8];
 let owner=box_new<Bytes>(bytes_copy(array_as_slice(a)));
 let bytes=box_into_inner<Bytes>(owner);
 if byte_len(bytes_as_slice(bytes))==1usize {7}else{0}
}
"#;

#[test]
fn owned_bytes_box_has_exact_owned_boundaries_and_v5_prelude() {
    let program = crate::check(SOURCE, "owned-box.spx").unwrap();
    let canonical = crate::format::canonical(&program);
    let checked = crate::check(&canonical, "owned-box.spx").unwrap();
    assert_eq!(canonical, crate::format::canonical(&checked));
    let resolved = crate::hir::resolve(&program).unwrap();
    crate::hir::validate(&resolved).unwrap();
    assert!(resolved_program_uses_owned_payload(&resolved));
    assert_eq!(
        resolved_params(BoxOp::New, &ResolvedType::Bytes)[0].ownership,
        OwnershipMode::Own
    );
    assert_eq!(
        resolved_params(BoxOp::IntoInner, &ResolvedType::Bytes)[0].ownership,
        OwnershipMode::Own
    );
    assert!(!resolved_operation_element_is_admitted(
        BoxOp::Get,
        &ResolvedType::Bytes
    ));
    let graph = crate::graph::to_json(&program).unwrap();
    crate::graph::verify_json(&program, &graph).unwrap();
    let forged = graph.replace("semaprax.prelude.v5", "semaprax.prelude.v4");
    assert_ne!(forged, graph);
    assert!(crate::graph::verify_json(&program, &forged).is_err());
    let document: serde_json::Value = serde_json::from_str(&graph).unwrap();
    assert_eq!(document["prelude"]["schema"], crate::prelude::SCHEMA_V5);
    assert_eq!(
        document["prelude"]["digest"],
        crate::prelude::digest_text_v5()
    );
    assert_eq!(
        crate::prelude::contract_bytes_v5(),
        include_bytes!("../../tests/fixtures/prelude-v5.contract")
    );
    let contract = String::from_utf8(crate::prelude::contract_bytes_v5()).unwrap();
    assert!(contract.starts_with("semaprax.prelude.v5\n"));
    assert!(contract.contains("<Bytes>(own:Bytes)->own:Box<Bytes>"));
    assert!(contract.contains("spx_box_drop_v2"));
    assert_eq!(
        crate::prelude::digest_text_v5(),
        "sha256:deeb4ca14e4a5a14e4b427bd75b4ca953ce2a335e725f616bcdad3c9e6fe1a58"
    );
    assert_eq!(
        crate::prelude::selected_for_program(&program).0,
        crate::prelude::SCHEMA_V5
    );
    let rejected = SOURCE.replace(
        "let bytes=box_into_inner<Bytes>(owner);",
        "let bytes=box_get<Bytes>(owner);",
    );
    assert!(crate::check(&rejected, "closed-get.spx").is_err());
    let reused = SOURCE.replace(
        "let bytes=box_into_inner<Bytes>(owner);",
        "let bytes=box_into_inner<Bytes>(owner); let again=box_into_inner<Bytes>(owner);",
    );
    let errors = crate::check(&reused, "reused-box.spx").unwrap_err();
    assert!(errors.iter().any(|d| d.code == "SPX-O101"), "{errors:?}");
}

#[test]
fn owned_bytes_box_lexical_only_signature_selects_v5() {
    let source = r#"module box.lexical;
@id("box.lexical.drop") fn settle(value: own Box<Bytes>)->i64 { 0 }
@id("app.main") fn main()->i64 { 0 }
"#;
    let program = crate::check(source, "lexical-box.spx").unwrap();
    assert_eq!(
        crate::prelude::selected_for_program(&program).0,
        crate::prelude::SCHEMA_V5
    );
    let resolved = crate::hir::resolve(&program).unwrap();
    assert!(resolved_program_uses_owned_payload(&resolved));
    let graph = crate::graph::to_json(&program).unwrap();
    crate::graph::verify_json(&program, &graph).unwrap();
    let document: serde_json::Value = serde_json::from_str(&graph).unwrap();
    assert_eq!(document["prelude"]["schema"], crate::prelude::SCHEMA_V5);
}
