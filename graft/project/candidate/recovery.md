# project/candidate/recovery.rs

- PROJECT_CANDIDATE_RECOVERY_SCHEMA · constant · L16-L16 — pub const PROJECT_CANDIDATE_RECOVERY_SCHEMA: &str = "semaprax.project-candidate-recovery.v1";
- PROJECT_CANDIDATE_RECOVERY_COMPATIBILITY · constant · L17-L18 — pub const PROJECT_CANDIDATE_RECOVERY_COMPATIBILITY: &str =
- MAX_PROJECT_CANDIDATE_RECOVERY_BYTES · constant · L19-L19 — pub const MAX_PROJECT_CANDIDATE_RECOVERY_BYTES: usize = MAX_PROJECT_CANDIDATE_BYTES;
- MAX_JSON_NODES · constant · L20-L20 — pub(super) const MAX_JSON_NODES: usize = MAX_CHANGES * (2 * 8192 + 128) + 256;
- MAX_JSON_DEPTH · constant · L21-L21 — pub(super) const MAX_JSON_DEPTH: usize = 128;
- DOMAIN · constant · L22-L22 — const DOMAIN: &[u8] = b"semaprax.project-candidate-recovery.payload.v1\0";
- recovery_capsule · function · L27-L49 — pub fn recovery_capsule(&self) -> Result<String, Vec<Diagnostic>>
- restore · function · L54-L167 — pub fn restore(
- KEYS · constant · L70-L80 — const KEYS: &[&str] = &[
- compiler · function · L170-L172 — pub(super) fn compiler() -> Value
- digest · function · L174-L189 — fn digest<'a>(value: &'a Value, field: &str) -> Result<&'a str, Vec<Diagnostic>>
- render · function · L191-L194 — fn render(value: Value) -> Result<String, Vec<Diagnostic>>
- preflight · function · L199-L246 — fn preflight(bytes: &[u8]) -> Result<(), Vec<Diagnostic>>
- invalid · function · L247-L249 — fn invalid(message: &'static str) -> Vec<Diagnostic>
- capacity · function · L250-L252 — fn capacity(message: &'static str) -> Vec<Diagnostic>
- stale · function · L253-L255 — fn stale(message: &'static str) -> Vec<Diagnostic>
