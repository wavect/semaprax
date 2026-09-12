# cli/project_image.rs

- derive · function · L12-L18 — pub(crate) fn derive(manifest: &Path) -> Result<String, Vec<Diagnostic>>
- verify · function · L20-L34 — pub(crate) fn verify(manifest: &Path, path: &Path) -> Result<String, Vec<Diagnostic>>
- symbol · function · L36-L44 — pub(crate) fn symbol(manifest: &Path, stable_id: &str) -> Result<String, Vec<Diagnostic>>
- persist · function · L46-L53 — pub(crate) fn persist(manifest: &Path, store: &Path) -> Result<String, Vec<Diagnostic>>
- load · function · L55-L64 — pub(crate) fn load(
- read_image · function · L66-L68 — fn read_image(path: &Path) -> Result<Vec<u8>, Diagnostic>
- read_bounded · function · L70-L101 — pub(super) fn read_bounded(path: &Path, limit: usize) -> Result<Vec<u8>, Diagnostic>
- FILE_ATTRIBUTE_REPARSE_POINT · constant · L83-L83 — const FILE_ATTRIBUTE_REPARSE_POINT: u32 = 0x0000_0400;
- open_image · function · L104-L113 — pub(super) fn open_image(path: &Path) -> Result<std::fs::File, Diagnostic>
- open_image · function · L116-L124 — pub(super) fn open_image(path: &Path) -> Result<std::fs::File, Diagnostic>
- FILE_FLAG_OPEN_REPARSE_POINT · constant · L118-L118 — const FILE_FLAG_OPEN_REPARSE_POINT: u32 = 0x0020_0000;
- open_image · function · L127-L131 — pub(super) fn open_image(_path: &Path) -> Result<std::fs::File, Diagnostic>
- input_error · function · L133-L135 — fn input_error(message: &str) -> Diagnostic
- capacity_error · function · L137-L139 — fn capacity_error() -> Diagnostic
