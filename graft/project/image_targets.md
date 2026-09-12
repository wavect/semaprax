# project/image_targets.rs

- c · module · L7-L7 — pub(super) mod c;
- openapi · module · L8-L8 — mod openapi;
- IMAGE_TARGET_ADMISSION_SCHEMA · constant · L10-L10 — pub const IMAGE_TARGET_ADMISSION_SCHEMA: &str = "semaprax.image-target-admission.v1";
- IMAGE_ARTIFACT_PROJECTION_SCHEMA · constant · L11-L11 — pub const IMAGE_ARTIFACT_PROJECTION_SCHEMA: &str = "semaprax.image-artifact-projection.v1";
- MAX_IMAGE_ARTIFACT_BUILD_BYTES · constant · L12-L12 — pub const MAX_IMAGE_ARTIFACT_BUILD_BYTES: usize = 16 * 1024 * 1024;
- MAX_IMAGE_ARTIFACT_REPORT_BYTES · constant · L13-L13 — pub const MAX_IMAGE_ARTIFACT_REPORT_BYTES: usize = 1024 * 1024;
- ImageArtifactKind · enum · L16-L21 — pub enum ImageArtifactKind
- name · function · L23-L30 — pub fn name(self) -> &'static str
- target_admission · function · L37-L101 — pub fn target_admission(
- artifact_projection · function · L106-L311 — pub fn artifact_projection(
- verify_artifact_projection · function · L313-L335 — pub fn verify_artifact_projection(
- error · function · L337-L339 — fn error(code: &'static str, message: &'static str) -> Vec<Diagnostic>
