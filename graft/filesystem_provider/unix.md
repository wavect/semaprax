# filesystem_provider/unix.rs

- NEXT_ATOMIC_TEMP · constant · L14-L14 — static NEXT_ATOMIC_TEMP: AtomicU64 = AtomicU64::new(0);
- ATOMIC_TEMP_ATTEMPTS · constant · L15-L15 — const ATOMIC_TEMP_ATTEMPTS: usize = 64;
- ScopedFileProvider · struct · L21-L24 — pub struct ScopedFileProvider
- open · function · L27-L34 — pub fn open(root: impl AsRef<Path>, access: FileAccess) -> Result<Self, FileFailure>
- parent · function · L36-L53 — fn parent(&self, path: &[u8]) -> Result<(OwnedFd, Vec<u8>), FileFailure>
- read · function · L57-L86 — fn read(&mut self, path: &[u8], max: usize) -> Result<Vec<u8>, FileFailure>
- write_new · function · L88-L111 — fn write_new(&mut self, path: &[u8], data: &[u8]) -> Result<usize, FileFailure>
- stat · function · L113-L148 — fn stat(&mut self, path: &[u8]) -> Result<FileMetadata, FileFailure>
- list · function · L150-L202 — fn list(&mut self, path: &[u8], max: usize) -> Result<Vec<u8>, FileFailure>
- create_dir · function · L204-L216 — fn create_dir(&mut self, path: &[u8]) -> Result<usize, FileFailure>
- remove · function · L218-L236 — fn remove(&mut self, path: &[u8]) -> Result<usize, FileFailure>
- write_atomic · function · L238-L300 — fn write_atomic(&mut self, path: &[u8], data: &[u8]) -> Result<usize, FileFailure>
- directory_flags · function · L303-L305 — fn directory_flags() -> OFlags
- io_failure · function · L307-L315 — fn io_failure(error: rustix::io::Errno) -> FileFailure
