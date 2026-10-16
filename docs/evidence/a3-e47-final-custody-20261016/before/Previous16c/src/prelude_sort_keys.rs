//! Selective v15: old programs retain their exact prior prelude bytes.
use sha2::{Digest, Sha256};
pub(crate) fn contract_bytes() -> Vec<u8> {
    let old = crate::prelude::owned_leaf::contract_bytes();
    let mut bytes = std::str::from_utf8(&old)
        .expect("prelude UTF-8")
        .replacen(crate::prelude::SCHEMA_V14, crate::prelude::SCHEMA_V15, 1)
        .into_bytes();
    bytes.extend_from_slice(b"operation core.vec.sort-by.v1:<descriptor> vec_sort_by <T>(own:Vec<T>,literal:keys)->own:Vec<T>\nrule explicit_record_sort keys:1..8 selector_bytes:1024 descriptor_bytes:16384 monomorphic_explicit_nominal complete_layout field_ids unique asc_desc stable reverse_operands same_owner_one_generation no_payload_copies no_replacement_array\nrule explicit_record_sort_metadata literal_is_not_runtime_String canonical_length_delimited_v1 independently_replayed_before_use\n");
    bytes
}
pub(crate) fn digest_text() -> String {
    format!(
        "sha256:{:x}",
        crate::digest_hex::LowerHex(Sha256::digest(contract_bytes()))
    )
}
pub(crate) fn selection(
    program: &dyn crate::ast::program_source::ProgramSource,
) -> (bool, bool, bool) {
    let mut selected = (false, false, false);
    let mut uses = |function: &crate::ast::Function| {
        let mut pending = function
            .requires
            .iter()
            .chain(std::iter::once(&function.body))
            .chain(&function.ensures)
            .collect::<Vec<_>>();
        while let Some(expr) = pending.pop() {
            if matches!(&expr.kind, crate::ast::ExprKind::Call {name,..} if name == crate::vec_ops::sort_keys::NAME)
            {
                selected.0 = true;
            }
            if matches!(&expr.kind, crate::ast::ExprKind::BorrowedUtf8Literal(_)) {
                selected.1 = true;
            }
            if matches!(&expr.kind, crate::ast::ExprKind::Call {name,..} if name == crate::vec_ops::string_slot::NAME)
            {
                selected.2 = true;
            }
            let mut i = 0;
            while let Some(child) = expr.child(i) {
                pending.push(child);
                i += 1;
            }
        }
    };
    for function in program.functions() {
        uses(function);
    }
    for ty in program.types() {
        if let crate::ast::TypeDeclarationKind::Class { methods, .. } = &ty.kind {
            for function in methods {
                uses(function);
            }
        }
    }
    selected
}
