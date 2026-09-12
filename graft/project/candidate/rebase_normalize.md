# project/candidate/rebase_normalize.rs

- MARKER_PREFIX · constant · L10-L10 — const MARKER_PREFIX: &str = "spx_rebase_ref_";
- MAX_SIDECAR_BYTES · constant · L11-L11 — const MAX_SIDECAR_BYTES: usize = 64 * 1024 * 1024;
- MAX_OCCURRENCES · constant · L12-L12 — const MAX_OCCURRENCES: usize = 1_048_576;
- programs · function · L14-L192 — pub(super) fn programs(
- add · function · L194-L212 — fn add<'a>(
- append · function · L214-L223 — fn append(output: &mut String, value: &str, total: &mut usize) -> Result<(), Vec<Diagnostic>>
- descriptor · function · L228-L264 — pub(super) fn descriptor(value: &mut Value)
