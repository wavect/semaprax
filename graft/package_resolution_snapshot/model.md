# package_resolution_snapshot/model.rs

- preflight_requirements · function · L5-L13 — pub(super) fn preflight_requirements(
- validate_requirement_count · function · L15-L22 — pub(super) fn validate_requirement_count(count: usize) -> Result<(), Diagnostic>
- validate_range_length · function · L24-L33 — pub(super) fn validate_range_length(bytes: usize) -> Result<(), Diagnostic>
- ResolutionSnapshot · struct · L36-L40 — pub struct ResolutionSnapshot
- validate_cumulative · function · L42-L48 — pub(super) fn validate_cumulative(snapshot: &ResolutionSnapshot) -> Result<(), Diagnostic>
- validate_lengths · function · L50-L67 — pub(super) fn validate_lengths(
- admit_subject_slot · function · L69-L76 — pub(super) fn admit_subject_slot(existing: usize) -> Result<(), Diagnostic>
- add_subject_bytes · function · L78-L93 — pub(super) fn add_subject_bytes(total: usize, next: usize) -> Result<usize, Diagnostic>
