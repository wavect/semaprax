//! Additive selective v19; programs without field updates keep exact old bytes.
use sha2::{Digest, Sha256};
pub(crate) fn contract_bytes() -> Vec<u8> {
    let old = crate::prelude::field_unique::contract_bytes();
    let mut bytes = std::str::from_utf8(&old)
        .expect("prelude UTF-8")
        .replacen(crate::prelude::SCHEMA_V18, crate::prelude::SCHEMA_V19, 1)
        .into_bytes();
    bytes.extend_from_slice(b"operation core.vec.update-field.v1:<descriptor> vec_update_field <T>(own:Vec<T>,value:usize,value:Copy,literal:field)->own:Vec<T>\nrule scalar_field_update.v1 invariant_free_explicit_flat_owned_leaf_record exact_field_identity complete_layout eight_Copy_scalars staged_left_to_right preflight_before_single_commit same_owner_generation no_clone_no_allocation_no_drop_at_store no_capacity_change whole_root_loans_end_before_update\n");
    bytes
}
pub(crate) fn digest_text() -> String {
    format!(
        "sha256:{:x}",
        crate::digest_hex::LowerHex(Sha256::digest(contract_bytes()))
    )
}
