//! Immutable byte custody for the complete independently executable predecessor.
use sha2::{Digest, Sha256};
#[test]
fn complete_immediate_previous_driver_and_every_local_body_remain_byte_exact() {
    let original = include_str!("before.txt");
    assert_eq!(format!("{:x}", crate::digest_hex::LowerHex(Sha256::digest(original.as_bytes()))),
        "a14dc6133316f333a2aa9130dacb510932e508ba7da2a589d44c0f22fe885dc8");
    let root = include_str!("before.rs");
    let start = root.find("    #[allow(clippy::too_many_arguments)]").unwrap();
    let mut restored = root[start..].strip_suffix("\n}\n").unwrap().to_owned();
    let definitions = [
        include_str!("before_0.rs"),
        include_str!("before_1.rs"),
        include_str!("before_2.rs"),
        include_str!("before_3.rs"),
        include_str!("before_4.rs"),
        include_str!("before_5.rs"),
        include_str!("before_6.rs"),
        include_str!("before_7.rs"),
        include_str!("before_8.rs"),
        include_str!("before_9.rs"),
        include_str!("before_10.rs"),
        include_str!("before_11.rs"),
        include_str!("before_12.rs"),
        include_str!("before_13.rs"),
        include_str!("before_14.rs"),
        include_str!("before_15.rs"),
        include_str!("before_16.rs"),
        include_str!("before_17.rs"),

    ];
    for (index, definition) in definitions.iter().enumerate() {
        let marker = format!("        before_definition_{index}!(");
        let start = restored.find(&marker).unwrap();
        let end = start + restored[start..].find(");").unwrap() + 2;
        let arguments = &restored[start+marker.len()..end-2];
        let mut body = definition.split_once(" => {\n").unwrap().1
            .strip_suffix("\n    };\n}\n").unwrap().to_owned();
        for argument in arguments.split(", ") {
            body = body.replace(&format!("${argument}"), argument);
        }
        restored.replace_range(start..end, &body);
    }
    restored = restored.replace("pub(in crate::hir::validation) fn validate_expr_before_frame_reuse(",
        "fn validate_expr_iterative_effects(")
        .replace("validator.validate_expr_before_frame_reuse(", "validator.validate_expr_iterative_effects(");
    for name in ["VALIDATOR_FRAME_BYTES", "lexical_exit", "yield_aggregate", "type_reachability",
        "collection_outcome", "function_value", "copy_discard", "vec_constructor"] {
        restored = restored.replace(&format!("crate::hir::{name}"), &format!("super::{name}"));
    }
    assert_eq!(restored, original, "every complete original literal and local body survives");
}

#[test]
fn complete_immediate_function_and_contract_receivers_keep_original_body_hashes() {
    let hash = |source: &str| format!("{:x}", crate::digest_hex::LowerHex(Sha256::digest(source.as_bytes())));
    let function = include_str!("function_before.rs")
        .replace("#[rustfmt::skip]\n", "")
        .replace("use super::super::*;\nuse super::super::function_scope::lifecycle;", "use super::*;")
        .replace("validate_function_before_frame_reuse(", "validate_function(")
        .replace("validate_contracts_before_frame_reuse(", "validate_contracts_persistent(")
        .replace("validate_expr_before_frame_reuse(", "validate_expr_persistent(")
        .replace("\"body\".into(),", "\"body\",");
    assert_eq!(hash(&function), "e2ef630aea80423eb1ad424d4ee7adc0dc7109fe7dc4e058f0f7eca7072d66ba");
    let contract = include_str!("contracts_before.rs");
    let contract = contract[contract.find("    pub(super) fn").unwrap()..]
        .strip_suffix("\n}\n").unwrap()
        .replace("validate_contracts_before_frame_reuse(", "validate_contracts_persistent(")
        .replace("validate_expr_before_frame_reuse(", "validate_expr_iterative_effects(");
    assert_eq!(hash(&contract), "4cfe0845d078389c497bdc665420b1bbdaa1e706efaae6841a52092dc52aad9d");
}
