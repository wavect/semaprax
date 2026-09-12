# project/npm/publication.rs

- TestHook · type · L24-L24 — type TestHook = Box<dyn FnOnce() + Send + 'static>;
- tests · module · L31-L31 — mod tests;
- hook_tests · module · L34-L34 — mod hook_tests;
- set_test_after_create · function · L37-L39 — pub(super) fn set_test_after_create(hook: TestHook)
- run_test_after_create · function · L42-L47 — fn run_test_after_create()
- run_test_after_create · function · L50-L50 — fn run_test_after_create() {}
- publish · function · L53-L60 — pub(super) fn publish(
- publish · function · L63-L95 — pub(super) fn publish(
- legacy_windows_publish · function · L98-L126 — fn legacy_windows_publish(
- unix · module · L129-L317 — mod unix
- publish · function · L139-L233 — pub(super) fn publish(
- open_absolute_directory · function · L235-L261 — fn open_absolute_directory(path: &Path) -> Result<OwnedFd, Diagnostic>
- authenticate_inventory · function · L263-L295 — fn authenticate_inventory(
- cstr_eq · function · L297-L299 — fn cstr_eq(value: &CStr, expected: &[u8]) -> bool
- identity · function · L301-L306 — fn identity<Fd: std::os::fd::AsFd>(fd: Fd) -> Result<(u64, u64), Diagnostic>
- identity_at · function · L308-L316 — fn identity_at<Fd: std::os::fd::AsFd, P: rustix::path::Arg>(
- validate_leaf · function · L320-L328 — fn validate_leaf(path: &str) -> Result<(), Diagnostic>
- absolute_normalized · function · L331-L359 — fn absolute_normalized(path: &Path) -> Result<PathBuf, Diagnostic>
