//! Additive sort binding; all v1-v10 contract bytes remain frozen.
use sha2::{Digest, Sha256};
pub(crate) fn contract_bytes_v11() -> Vec<u8> {
    let old = crate::stdin_stream_ops::contract_bytes();
    let old = std::str::from_utf8(&old).expect("prelude contract is UTF-8");
    let mut bytes = old
        .replacen(crate::prelude::SCHEMA_V10, crate::prelude::SCHEMA_V11, 1)
        .into_bytes();
    bytes.extend_from_slice(b"operation core.vec.sort vec_sort <T>(own:Vec<T>)->own:Vec<T>\nrule sort Copy_scalars_only ascending numeric_char_bool floating_IEEE_total_order no_payload_clones no_capacity_change generation=next\n");
    bytes
}
pub(crate) fn digest_text_v11() -> String {
    format!(
        "sha256:{:x}",
        crate::digest_hex::LowerHex(Sha256::digest(contract_bytes_v11()))
    )
}
