# project_transport/sdk.rs

- PROJECT_PUBLIC_API_DISCOVERY_SCHEMA · constant · L3-L4 — pub const PROJECT_PUBLIC_API_DISCOVERY_SCHEMA: &str =
- MAX_CLIENT_BYTES · constant · L5-L5 — const MAX_CLIENT_BYTES: usize = 256 * 1024;
- DISCOVERY · constant · L7-L19 — const DISCOVERY: &str = concat!(
- ProjectTransportClientLanguage · enum · L22-L26 — pub enum ProjectTransportClientLanguage
- project_public_api_transport_discovery · function · L28-L30 — pub fn project_public_api_transport_discovery() -> &'static str
- generate_project_public_api_transport_client · function · L32-L46 — pub fn generate_project_public_api_transport_client(
- tests · module · L49-L104 — mod tests
- discovery_is_closed_canonical_and_matches_frozen_transport · function · L53-L71 — fn discovery_is_closed_canonical_and_matches_frozen_transport()
- all_clients_are_deterministic_bounded_and_authority_free · function · L74-L103 — fn all_clients_are_deterministic_bounded_and_authority_free()
