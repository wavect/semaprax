use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

static SERIAL: AtomicU64 = AtomicU64::new(0);

// Keep Cargo's objects out of deeply nested product fixture directories.
// This models the same legacy link.exe boundary as project_product.rs, with
// the longer owned-data package name. A verbatim prefix does not remove it.
const OBJECT_SUFFIX: &str = concat!(
    r"\debug\build\semaprax-generated-native-rust-owned-data-sdk-0000000000000000",
    r"\build_script_build-0000000000000000.build_script_build.",
    "0000000000000000-cgu.0.rcgu.o",
);

pub struct CargoTarget(PathBuf);

impl CargoTarget {
    pub fn new() -> Self {
        let parent = std::env::temp_dir().canonicalize().unwrap();
        for _ in 0..64 {
            let serial = SERIAL.fetch_add(1, Ordering::Relaxed);
            let path = parent.join(format!("spxc-{:x}-{serial:x}", std::process::id()));
            #[cfg(windows)]
            {
                use std::os::windows::ffi::OsStrExt;
                let units = path.as_os_str().encode_wide().count();
                assert!(
                    object_path_fits(units),
                    "nested owned-data Cargo target exceeds the legacy link.exe object path budget; configure a shorter TEMP directory"
                );
            }
            match std::fs::create_dir(&path) {
                Ok(()) => return Self(path),
                Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
                Err(error) => panic!("create isolated nested Cargo target: {error}"),
            }
        }
        panic!("cannot reserve a fresh isolated nested Cargo target");
    }

    pub fn path(&self) -> &Path {
        &self.0
    }
}

impl Drop for CargoTarget {
    fn drop(&mut self) {
        // Only the fresh directory reserved by this guard is owned here.
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn object_path_fits(target_utf16_units: usize) -> bool {
    target_utf16_units
        .checked_add(OBJECT_SUFFIX.encode_utf16().count())
        .is_some_and(|units| units < 260)
}

#[test]
fn windows_object_path_budget_is_exact_and_rejects_the_failed_fixture() {
    let suffix_units = OBJECT_SUFFIX.encode_utf16().count();
    assert!(object_path_fits(259 - suffix_units));
    assert!(!object_path_fits(260 - suffix_units));
    assert!(!object_path_fits(usize::MAX));
    let previous = r"\\?\C:\Users\runneradmin\AppData\Local\Temp\semaprax-frame-payload-project-v8-routes-7984-0\before-rust\target";
    assert!(!object_path_fits(previous.encode_utf16().count()));
    let compact = r"\\?\C:\Users\runneradmin\AppData\Local\Temp\spxc-ffffffff-ffffffffffffffff";
    assert!(object_path_fits(compact.encode_utf16().count()));
}

#[test]
fn cargo_targets_are_fresh_isolated_and_owned_until_drop() {
    let first = CargoTarget::new();
    let second = CargoTarget::new();
    assert_ne!(first.path(), second.path());
    assert!(first.path().is_absolute() && first.path().is_dir());
    let first_path = first.path().to_owned();
    drop(first);
    assert!(!first_path.exists());
    assert!(second.path().is_dir());
}
