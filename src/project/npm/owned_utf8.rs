//! Project-v10-only npm presentation facts. Keeping these out of the v8
//! renderer makes the legacy package bytes visibly independent of UTF-8.

pub(super) const API_SCHEMA: &str = "semaprax.owned-utf8-api.v1";
pub(super) const MEMORY_BYTES: usize = 262_144;
pub(super) const DECODER_DECLARATION: &str =
    ",utf8Decoder=new TextDecoder(\"utf-8\",{fatal:true,ignoreBOM:true})";
pub(super) const RESULT_CASE: &str = "case \"owned-utf8\":{const copied=linked.arena.consume(view.getBigInt64(RESULT,true));answer=utf8Decoder.decode(copied);break}";

pub(super) fn presentation(enabled: bool) -> (usize, &'static str, &'static str) {
    if enabled {
        (MEMORY_BYTES, DECODER_DECLARATION, RESULT_CASE)
    } else {
        (131_072, "", "")
    }
}
