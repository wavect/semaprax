# project/npm/owned_utf8.rs

- API_SCHEMA · constant · L4-L4 — pub(super) const API_SCHEMA: &str = "semaprax.owned-utf8-api.v1";
- MEMORY_BYTES · constant · L5-L5 — pub(super) const MEMORY_BYTES: usize = 262_144;
- DECODER_DECLARATION · constant · L6-L7 — pub(super) const DECODER_DECLARATION: &str =
- RESULT_CASE · constant · L8-L8 — pub(super) const RESULT_CASE: &str = "case \"owned-utf8\":{const copied=linked.arena.consume(view.getBigInt64(RESULT,true));answer=utf8Decoder.decode(copied);break}";
- presentation · function · L10-L16 — pub(super) fn presentation(enabled: bool) -> (usize, &'static str, &'static str)
