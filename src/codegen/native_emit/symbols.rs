use std::fmt::Write as _;

use sha2::{Digest as _, Sha256};

use crate::hir::{DeclarationId, ResolvedType};

pub(in crate::codegen) fn c_record_symbol(ty: &ResolvedType) -> String {
    let ResolvedType::Nominal {
        declaration,
        arguments,
    } = ty
    else {
        unreachable!("record C symbols require nominal types");
    };
    let mut symbol = stable_c_symbol("spx_record_", declaration);
    if !arguments.is_empty() {
        let mut digest = Sha256::new();
        digest.update(b"semaprax.native-record-instance.v1\0");
        digest.update(ty.identity_key().as_bytes());
        symbol.push_str("_inst_");
        for byte in digest.finalize() {
            write!(symbol, "{byte:02x}").expect("writing to a string cannot fail");
        }
    }
    symbol
}

pub(in crate::codegen) fn c_variant_symbol(ty: &ResolvedType) -> String {
    let ResolvedType::Nominal {
        declaration,
        arguments,
    } = ty
    else {
        unreachable!("variant C symbols require nominal types");
    };
    let mut symbol = stable_c_symbol("spx_variant_", declaration);
    if !arguments.is_empty() {
        let mut digest = Sha256::new();
        digest.update(b"semaprax.native-variant-instance.v1\0");
        digest.update(ty.identity_key().as_bytes());
        symbol.push_str("_inst_");
        for byte in digest.finalize() {
            write!(symbol, "{byte:02x}").expect("writing to a string cannot fail");
        }
    }
    symbol
}

pub(in crate::codegen) fn c_case_symbol(id: &DeclarationId) -> String {
    stable_c_symbol("spx_case_", id)
}

pub(in crate::codegen) fn c_field_symbol(id: &DeclarationId) -> String {
    stable_c_symbol("spx_field_", id)
}

fn stable_c_symbol(prefix: &str, id: &DeclarationId) -> String {
    let mut symbol = crate::bounded_output::CappedString::new();
    symbol.push_str(prefix);
    for byte in id.as_str().bytes() {
        write!(symbol, "{byte:02x}").expect("writing to a string cannot fail");
    }
    symbol.into_string()
}
