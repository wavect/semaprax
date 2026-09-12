# cli/package_resolver/held_read_tests.rs

- NEXT · constant · L4-L4 — static NEXT: AtomicU64 = AtomicU64::new(0);
- temp_file · function · L6-L14 — fn temp_file(label: &str, bytes: &[u8]) -> PathBuf
- TruncateBeforeRead · struct · L16-L20 — struct TruncateBeforeRead
- before_read · function · L23-L33 — fn before_read(&mut self, index: usize, file: &std::fs::File)
- after_read · function · L35-L38 — fn after_read(&mut self, index: usize, _file: &std::fs::File)
- deterministic_short_read_rejects_before_subject_processing · function · L42-L55 — fn deterministic_short_read_rejects_before_subject_processing()
- GrowAfterRead · struct · L57-L61 — struct GrowAfterRead
- before_read · function · L64-L67 — fn before_read(&mut self, index: usize, _file: &std::fs::File)
- after_read · function · L69-L78 — fn after_read(&mut self, index: usize, _file: &std::fs::File)
- deterministic_post_read_growth_rejects_metadata_drift · function · L82-L95 — fn deterministic_post_read_growth_rejects_metadata_drift()
- windows_reparse_attribute_admission_has_exact_bit_boundary · function · L98-L113 — fn windows_reparse_attribute_admission_has_exact_bit_boundary()
