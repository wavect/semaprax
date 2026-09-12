# package_resolver/semver.rs

- Version · struct · L7-L7 — pub(super) struct Version(pub(super) u32, pub(super) u32, pub(super) u32);
- Range · enum · L10-L13 — pub(super) enum Range
- contains · function · L16-L21 — pub(super) fn contains(self, version: Version) -> bool
- parse_version · function · L24-L41 — pub(super) fn parse_version(value: &str) -> Result<Version, Diagnostic>
- MAX_VERSION_BYTES · constant · L25-L25 — const MAX_VERSION_BYTES: usize = 10 + 1 + 10 + 1 + 10;
- parse_range · function · L43-L92 — pub(super) fn parse_range(value: &str) -> Result<Range, Diagnostic>
- component · function · L94-L112 — fn component(value: Option<&str>, label: &str) -> Result<u32, Diagnostic>
- compare_coordinates · function · L114-L123 — pub(super) fn compare_coordinates(
