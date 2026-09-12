# economic_agent/address.rs

- BASE58_ALPHABET · constant · L4-L5 — pub(super) const BASE58_ALPHABET: &[u8] =
- encode_base58 · function · L7-L31 — pub(super) fn encode_base58(bytes: &[u8]) -> String
- decode_base58_32 · function · L33-L53 — pub(super) fn decode_base58_32(value: &str) -> Option<[u8; 32]>
- decode_regtest_p2wpkh · function · L54-L82 — pub(super) fn decode_regtest_p2wpkh(value: &str) -> Option<Vec<u8>>
- bech32_verify · function · L83-L107 — pub(super) fn bech32_verify(hrp: &str, data: &[u8]) -> bool
- convert_bits · function · L108-L132 — pub(super) fn convert_bits(data: &[u8], from: u32, to: u32, pad: bool) -> Option<Vec<u8>>
