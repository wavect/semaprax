# cli/semantic_cache.rs

- absolute · function · L9-L21 — fn absolute(path: &Path) -> Result<PathBuf, Vec<Diagnostic>>
- initialize · function · L22-L29 — pub(crate) fn initialize(root: &Path) -> Result<String, Vec<Diagnostic>>
- persist · function · L30-L49 — pub(crate) fn persist(manifest: &Path, root: &Path) -> Result<String, Vec<Diagnostic>>
- load · function · L50-L58 — pub(crate) fn load(root: &Path, expected: &str) -> Result<String, Vec<Diagnostic>>
- evict · function · L60-L74 — pub(crate) fn evict(root: &Path, expected: &str) -> Result<String, Vec<Diagnostic>>
- MAX_LIFECYCLE_REPORT_BYTES · constant · L76-L76 — const MAX_LIFECYCLE_REPORT_BYTES: usize = 512 * 1024;
- lifecycle · function · L81-L173 — pub(crate) fn lifecycle(manifest: &Path, root: &Path) -> Result<String, Vec<Diagnostic>>
- initial_work · function · L175-L180 — fn initial_work(session: &VNextSession) -> Result<Value, Vec<Diagnostic>>
- text · function · L182-L187 — fn text<'a>(value: &'a Value, key: &str) -> Result<&'a str, Vec<Diagnostic>>
- work_count · function · L189-L195 — fn work_count(value: &Value, key: &str) -> Result<u64, Vec<Diagnostic>>
- require_work_profile · function · L197-L215 — fn require_work_profile(
- same_source_refresh · function · L217-L246 — fn same_source_refresh(
- lifecycle_error · function · L248-L250 — fn lifecycle_error(message: &'static str) -> Vec<Diagnostic>
