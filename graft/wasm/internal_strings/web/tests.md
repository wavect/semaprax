# wasm/internal_strings/web/tests.rs

- descriptor_bounds · module · L7-L7 — mod descriptor_bounds;
- package_bounds · module · L8-L8 — mod package_bounds;
- SOURCE · constant · L10-L11 — const SOURCE: &str =
- NEXT · constant · L12-L12 — static NEXT: AtomicU64 = AtomicU64::new(0);
- INVENTORY · constant · L13-L22 — const INVENTORY: [&str; 8] = [
- plain · function · L24-L35 — fn plain(path: &Path, directory: bool) -> std::fs::Metadata
- exact_entries · function · L37-L47 — fn exact_entries(path: &Path, expected: &[&str])
- reopened_package · function · L49-L69 — fn reopened_package(path: &Path) -> BTreeMap<&'static str, Vec<u8>>
- directory · function · L71-L79 — fn directory() -> std::path::PathBuf
- descriptor_and_complete_package_bounds_are_exact_and_checked · function · L82-L96 — fn descriptor_and_complete_package_bounds_are_exact_and_checked()
- exact_source_bound_is_read_and_plus_one_fails_before_output · function · L99-L161 — fn exact_source_bound_is_read_and_plus_one_fails_before_output()
- source_drift_and_growth_are_rejected_before_destination_creation · function · L164-L183 — fn source_drift_and_growth_are_rejected_before_destination_creation()
- empty_identity_rejects_and_leading_dash_identity_remains_exact · function · L186-L216 — fn empty_identity_rejects_and_leading_dash_identity_remains_exact()
