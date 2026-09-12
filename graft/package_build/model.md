# package_build/model.rs

- MANIFEST_SCHEMA · constant · L4-L4 — pub const MANIFEST_SCHEMA: &str = "semaprax.offline-effect-free-wasm-package-build.v1";
- EVIDENCE_SCHEMA · constant · L5-L5 — pub const EVIDENCE_SCHEMA: &str = "semaprax.offline-effect-free-wasm-package-build-evidence.v1";
- PROFILE · constant · L6-L6 — pub const PROFILE: &str = "effect-free-core-wasm-scalar.v1";
- MAX_ARTIFACT_BYTES · constant · L7-L7 — pub const MAX_ARTIFACT_BYTES: usize = 16 * 1024 * 1024;
- MAX_EVIDENCE_BYTES · constant · L8-L8 — pub const MAX_EVIDENCE_BYTES: usize = 16 * 1024 * 1024;
- MAX_EVIDENCE_RENDER_BYTES · constant · L12-L12 — pub const MAX_EVIDENCE_RENDER_BYTES: usize = 64 * 1024 * 1024;
- MIN_LIMIT_BYTES · constant · L13-L13 — pub const MIN_LIMIT_BYTES: usize = 4 * 1024;
- MAX_EXPORTS · constant · L14-L14 — pub const MAX_EXPORTS: usize = 32;
- MAX_STABLE_ID_BYTES · constant · L15-L15 — pub const MAX_STABLE_ID_BYTES: usize = 128;
- RUNTIME_IMPORTS · constant · L17-L25 — pub(crate) const RUNTIME_IMPORTS: [&str; 7] = [
- NONCLAIMS · constant · L27-L37 — pub(crate) const NONCLAIMS: [&str; 9] = [
- OfflinePackageBuildOptions · struct · L40-L45 — pub struct OfflinePackageBuildOptions
- new · function · L48-L62 — pub fn new(
- OfflinePackageBuild · struct · L66-L70 — pub struct OfflinePackageBuild
- VerifiedOfflinePackageBuild · struct · L73-L78 — pub struct VerifiedOfflinePackageBuild
- BuildFacts · struct · L81-L95 — pub(crate) struct BuildFacts
