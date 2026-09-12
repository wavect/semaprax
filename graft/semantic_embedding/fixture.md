# semantic_embedding/fixture.rs

- COMPONENT_DOMAIN · constant · L20-L20 — const COMPONENT_DOMAIN: &[u8] = b"semaprax.semantic-embedding.fixture-component.v1\0";
- fixture_component · function · L33-L47 — fn fixture_component(request_digest: &str, index: u32) -> f32
- FixtureEmbeddingProvider · struct · L61-L61 — pub struct FixtureEmbeddingProvider;
- embed · function · L64-L74 — fn embed(
- ScriptedEmbeddingProvider · struct · L82-L85 — pub struct ScriptedEmbeddingProvider
- scripted · function · L89-L94 — pub fn scripted(script: Vec<EmbeddingOutcome>) -> Self
- must_not_be_called · function · L100-L105 — pub fn must_not_be_called() -> Self
- embed · function · L109-L118 — fn embed(
- tests · module · L122-L173 — mod tests
- request · function · L125-L132 — fn request(dimensions: u32) -> EmbeddingRequest
- fixture_component_is_a_stable_known_answer · function · L135-L144 — fn fixture_component_is_a_stable_known_answer()
- every_component_is_finite_and_within_the_documented_range · function · L147-L162 — fn every_component_is_finite_and_within_the_documented_range()
- scripted_provider_must_not_be_called_panics_if_reached · function · L165-L172 — fn scripted_provider_must_not_be_called_panics_if_reached()
