# image_transport/vnext/candidate_dependency_navigation.rs

- SUMMARY_SCHEMA · constant · L5-L5 — const SUMMARY_SCHEMA: &str = "semaprax.project-candidate-dependency-summary.v1";
- PAGE_SCHEMA · constant · L6-L6 — const PAGE_SCHEMA: &str = "semaprax.project-candidate-dependency-page.v1";
- MAX_SUMMARY_BYTES · constant · L7-L7 — const MAX_SUMMARY_BYTES: usize = 64 * 1024;
- MAX_PAGE_BYTES · constant · L8-L8 — const MAX_PAGE_BYTES: usize = 1024 * 1024;
- METHODS · constant · L10-L66 — const METHODS: &[Method] = &[
- methods · function · L68-L70 — pub(super) fn methods() -> &'static [Method]
- prepare · function · L72-L83 — pub(super) fn prepare(
- for_candidate · function · L85-L129 — pub(super) fn for_candidate(
