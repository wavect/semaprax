use super::*;
use std::sync::atomic::{AtomicU64, Ordering};

static NEXT: AtomicU64 = AtomicU64::new(0);

fn temp_file(label: &str, bytes: &[u8]) -> PathBuf {
    let path = std::env::temp_dir().join(format!(
        "semaprax-package-resolver-read-hook-{label}-{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    ));
    std::fs::write(&path, bytes).unwrap();
    path
}

struct TruncateBeforeRead {
    path: PathBuf,
    before: usize,
    after: usize,
}

impl SubjectReadHook for TruncateBeforeRead {
    fn before_read(&mut self, index: usize, file: &std::fs::File) {
        assert_eq!(index, 0);
        self.before += 1;
        let _ = file;
        std::fs::OpenOptions::new()
            .write(true)
            .open(&self.path)
            .unwrap()
            .set_len(1)
            .unwrap();
    }

    fn after_read(&mut self, index: usize, _file: &std::fs::File) {
        assert_eq!(index, 0);
        self.after += 1;
    }
}

#[test]
fn deterministic_short_read_rejects_before_subject_processing() {
    let path = temp_file("truncate", b"{}");
    let later = temp_file("truncate-later", b"{}");
    let mut hook = TruncateBeforeRead {
        path: path.clone(),
        before: 0,
        after: 0,
    };
    let error = read_subjects_with_hook(&[path.clone(), later.clone()], &mut hook).unwrap_err();
    assert_eq!(error.code, "SPX-I215");
    assert_eq!((hook.before, hook.after), (1, 1));
    std::fs::remove_file(path).unwrap();
    std::fs::remove_file(later).unwrap();
}

struct GrowAfterRead {
    path: PathBuf,
    before: usize,
    after: usize,
}

impl SubjectReadHook for GrowAfterRead {
    fn before_read(&mut self, index: usize, _file: &std::fs::File) {
        assert_eq!(index, 0);
        self.before += 1;
    }

    fn after_read(&mut self, index: usize, _file: &std::fs::File) {
        assert_eq!(index, 0);
        self.after += 1;
        std::fs::OpenOptions::new()
            .write(true)
            .open(&self.path)
            .unwrap()
            .set_len(3)
            .unwrap();
    }
}

#[test]
fn deterministic_post_read_growth_rejects_metadata_drift() {
    let path = temp_file("growth", b"{}");
    let later = temp_file("growth-later", b"{}");
    let mut hook = GrowAfterRead {
        path: path.clone(),
        before: 0,
        after: 0,
    };
    let error = read_subjects_with_hook(&[path.clone(), later.clone()], &mut hook).unwrap_err();
    assert_eq!(error.code, "SPX-I215");
    assert_eq!((hook.before, hook.after), (1, 1));
    std::fs::remove_file(path).unwrap();
    std::fs::remove_file(later).unwrap();
}

#[test]
fn windows_reparse_attribute_admission_has_exact_bit_boundary() {
    assert!(windows_file_attributes_are_admitted(0));
    assert!(windows_file_attributes_are_admitted(
        WINDOWS_FILE_ATTRIBUTE_REPARSE_POINT - 1
    ));
    assert!(windows_file_attributes_are_admitted(
        WINDOWS_FILE_ATTRIBUTE_REPARSE_POINT << 1
    ));
    assert!(!windows_file_attributes_are_admitted(
        WINDOWS_FILE_ATTRIBUTE_REPARSE_POINT
    ));
    assert!(!windows_file_attributes_are_admitted(
        WINDOWS_FILE_ATTRIBUTE_REPARSE_POINT | 1
    ));
    assert!(!windows_file_attributes_are_admitted(u32::MAX));
}
