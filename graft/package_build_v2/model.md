# package_build_v2/model.rs

- MANIFEST_SCHEMA · constant · L4-L4 — pub const MANIFEST_SCHEMA: &str = "semaprax.offline-linked-scalar-wasm-package-build.v2";
- EVIDENCE_SCHEMA · constant · L5-L5 — pub const EVIDENCE_SCHEMA: &str = "semaprax.offline-linked-scalar-wasm-package-build-evidence.v2";
- PROFILE · constant · L6-L6 — pub const PROFILE: &str = "linked-effect-free-core-wasm-scalar.v2";
- MAX_ARTIFACT_BYTES · constant · L7-L7 — pub const MAX_ARTIFACT_BYTES: usize = 16 * 1024 * 1024;
- MAX_EVIDENCE_BYTES · constant · L8-L8 — pub const MAX_EVIDENCE_BYTES: usize = 16 * 1024 * 1024;
- MAX_EVIDENCE_RENDER_BYTES · constant · L9-L9 — pub const MAX_EVIDENCE_RENDER_BYTES: usize = 64 * 1024 * 1024;
- MIN_LIMIT_BYTES · constant · L10-L10 — pub(crate) const MIN_LIMIT_BYTES: usize = 4 * 1024;
- MAX_EXPORTS · constant · L11-L11 — pub(crate) const MAX_EXPORTS: usize = 32;
- MAX_STABLE_ID_BYTES · constant · L12-L12 — pub(crate) const MAX_STABLE_ID_BYTES: usize = 128;
- NONCLAIMS · constant · L14-L24 — pub(crate) const NONCLAIMS: [&str; 9] = [
- LinkedOfflinePackageBuildOptions · struct · L27-L32 — pub struct LinkedOfflinePackageBuildOptions
- new · function · L35-L49 — pub fn new(
- LinkedOfflinePackageBuild · struct · L53-L57 — pub struct LinkedOfflinePackageBuild
- VerifiedLinkedOfflinePackageBuild · struct · L60-L66 — pub struct VerifiedLinkedOfflinePackageBuild
- BuildFacts · struct · L68-L78 — pub(crate) struct BuildFacts
