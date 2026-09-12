# live_invocation/identity.rs

- IDENTITY_DOMAIN · constant · L23-L23 — const IDENTITY_DOMAIN: &[u8] = b"semaprax.live-invocation.identity.v1\0";
- LiveInvocationSeed · struct · L33-L51 — pub struct LiveInvocationSeed
- canonical · function · L54-L69 — fn canonical(&self) -> String
- LiveInvocationId · struct · L80-L80 — pub struct LiveInvocationId(String);
- derive · function · L85-L90 — pub fn derive(seed: &LiveInvocationSeed) -> Self
- digest · function · L94-L96 — pub fn digest(&self) -> &str
- fmt · function · L100-L102 — fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result
- hex · function · L106-L112 — pub(crate) fn hex(bytes: &[u8]) -> String
- unhex · function · L117-L136 — pub(crate) fn unhex(text: &str) -> Option<Vec<u8>>
- digest · function · L141-L146 — pub(crate) fn digest(domain: &[u8], bytes: &[u8]) -> String
- looks_like_digest · function · L150-L156 — pub(crate) fn looks_like_digest(text: &str) -> bool
- tests · module · L159-L214 — mod tests
- seed · function · L162-L171 — fn seed() -> LiveInvocationSeed
- identity_is_stable_for_the_same_seed_and_changes_with_any_field · function · L174-L200 — fn identity_is_stable_for_the_same_seed_and_changes_with_any_field()
- identity_never_depends_on_response_bytes · function · L203-L213 — fn identity_never_depends_on_response_bytes()
