# assurance_manifest/model_checking/digest.rs

- MODEL_DIGEST_DOMAIN · constant · L27-L27 — const MODEL_DIGEST_DOMAIN: &[u8] = b"semaprax.assurance-manifest.model-checking.model.v1\0";
- ModelDescriptor · struct · L34-L39 — pub struct ModelDescriptor
- write_len_prefixed · function · L41-L44 — fn write_len_prefixed(hasher: &mut Sha256, bytes: &[u8])
- model_digest · function · L52-L72 — pub fn model_digest(descriptor: &ModelDescriptor, bounds: Bounds) -> String
- tests · module · L75-L153 — mod tests
- DESCRIPTOR · constant · L78-L83 — const DESCRIPTOR: ModelDescriptor = ModelDescriptor
- BOUNDS · constant · L84-L88 — const BOUNDS: Bounds = Bounds
- digest_is_deterministic_across_repeated_calls · function · L91-L97 — fn digest_is_deterministic_across_repeated_calls()
- digest_changes_with_bounds · function · L100-L109 — fn digest_changes_with_bounds()
- digest_changes_with_version · function · L112-L121 — fn digest_changes_with_version()
- digest_changes_with_invariant_set · function · L124-L133 — fn digest_changes_with_invariant_set()
- length_prefixing_prevents_field_boundary_aliasing · function · L136-L152 — fn length_prefixing_prevents_field_boundary_aliasing()
