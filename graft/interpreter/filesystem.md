# interpreter/filesystem.rs

- command · module · L2-L2 — pub(crate) mod command;
- FileState · struct · L10-L14 — pub(super) struct FileState<'a>
- new · function · L16-L22 — pub(super) fn new(provider: &'a mut dyn FileProvider) -> Self
- settle · function · L23-L25 — pub(super) fn settle(self)
- reserve · function · L26-L39 — fn reserve(&mut self, count: u64) -> Result<(), Flow>
- failure · function · L41-L51 — fn failure(error: FileFailure) -> Flow
- prefix · function · L52-L57 — fn prefix(bytes: &[u8], length: u64) -> Result<&[u8], Flow>
- evaluate_filesystem_operation · function · L59-L178 — pub(super) fn evaluate_filesystem_operation(
- tests · module · L182-L273 — mod tests
- Provider · struct · L184-L188 — struct Provider
- read · function · L190-L192 — fn read(&mut self, _: &[u8], _: usize) -> Result<Vec<u8>, FileFailure>
- write_new · function · L193-L195 — fn write_new(&mut self, _: &[u8], _: &[u8]) -> Result<usize, FileFailure>
- stat · function · L196-L203 — fn stat(&mut self, path: &[u8]) -> Result<FileMetadata, FileFailure>
- list · function · L204-L212 — fn list(&mut self, path: &[u8], _: usize) -> Result<Vec<u8>, FileFailure>
- settle · function · L213-L215 — fn settle(&mut self)
- filesystem_v2_root_metadata_and_invalid_listing_settle · function · L218-L272 — fn filesystem_v2_root_metadata_and_invalid_listing_settle()
