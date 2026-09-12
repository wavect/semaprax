# project/standard_dependencies.rs

- VERSION · constant · L16-L16 — const VERSION: Version = Version(0, 1, 0);
- BundledPackage · struct · L18-L23 — struct BundledPackage
- PACKAGES · constant · L25-L212 — const PACKAGES: &[BundledPackage] = &[
- extend_sources · function · L214-L260 — pub(super) fn extend_sources(
- package · function · L262-L264 — fn package(name: &str) -> Option<&'static BundledPackage>
- is_bundled · function · L266-L268 — pub(super) fn is_bundled(name: &str) -> bool
- unresolved · function · L270-L274 — fn unresolved(message: String) -> Vec<Diagnostic>
- range_error · function · L276-L278 — fn range_error(message: String) -> Diagnostic
