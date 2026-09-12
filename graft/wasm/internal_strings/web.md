# wasm/internal_strings/web.rs

- render · module · L2-L2 — mod render;
- tests · module · L4-L4 — mod tests;
- SOURCE_LIMIT · constant · L10-L10 — const SOURCE_LIMIT: usize = 16 * 1024 * 1024;
- DESCRIPTOR_LIMIT · constant · L11-L11 — const DESCRIPTOR_LIMIT: usize = 1024 * 1024;
- PACKAGE_LIMIT · constant · L12-L12 — const PACKAGE_LIMIT: usize = 32 * 1024 * 1024;
- build_web_from_source · function · L17-L23 — pub fn build_web_from_source(
- build · function · L25-L60 — fn build(
- bounded · function · L62-L70 — fn bounded(size: usize, limit: usize, label: &str) -> Result<(), Diagnostic>
- package_size · function · L72-L78 — fn package_size(lengths: impl IntoIterator<Item = usize>) -> Result<(), Diagnostic>
