# semantic_embedding/request.rs

- REQUEST_DOMAIN · constant · L9-L9 — const REQUEST_DOMAIN: &[u8] = b"semaprax.semantic-embedding.request.v1\0";
- EmbeddingRequest · struct · L19-L38 — pub struct EmbeddingRequest
- digest · function · L45-L54 — pub fn digest(&self) -> String
- hex · function · L59-L65 — fn hex(bytes: &[u8]) -> String
- digest · function · L70-L75 — pub(crate) fn digest(domain: &[u8], bytes: &[u8]) -> String
- EmbeddingFailure · enum · L83-L99 — pub enum EmbeddingFailure
- as_str · function · L103-L112 — pub fn as_str(self) -> &'static str
- EmbeddingOutcome · enum · L118-L131 — pub enum EmbeddingOutcome
- tests · module · L134-L196 — mod tests
- base_request · function · L137-L144 — fn base_request() -> EmbeddingRequest
- digest_is_a_stable_known_answer_for_a_fixed_request · function · L147-L155 — fn digest_is_a_stable_known_answer_for_a_fixed_request()
- digest_changes_with_every_field_independently · function · L158-L174 — fn digest_changes_with_every_field_independently()
- digest_is_identical_for_byte_identical_requests · function · L177-L179 — fn digest_is_identical_for_byte_identical_requests()
- failure_as_str_is_a_closed_stable_vocabulary · function · L182-L195 — fn failure_as_str_is_a_closed_stable_vocabulary()
