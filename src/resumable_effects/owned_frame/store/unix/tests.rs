use super::*;
use std::os::unix::fs::{MetadataExt, PermissionsExt};
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
static NEXT: AtomicU64 = AtomicU64::new(0);
struct Directory(PathBuf);
impl Directory {
    fn new() -> Self {
        let path = std::env::temp_dir().join(format!(
            "spx-owned-frame-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir(&path).unwrap();
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o700)).unwrap();
        Self(path)
    }
    fn file(&self) -> File {
        File::open(&self.0).unwrap()
    }
    fn identity(&self) -> (u64, u64) {
        let m = std::fs::metadata(&self.0).unwrap();
        (m.dev(), m.ino())
    }
}
impl Drop for Directory {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}
fn scope() -> SourceCheckpointScope {
    SourceCheckpointScope::new("sha256:program", "owned-store", 1).unwrap()
}
fn grant(identity: OwnedFrameStoreIdentity) -> OwnedFrameStoreRegistration {
    OwnedFrameStoreRegistration::grant_for_trusted_host(identity, &scope(), true).unwrap()
}
#[test]
fn owned_frame_store_locks_registered_identity_and_survives_directory_rename() {
    let directory = Directory::new();
    let mut lease =
        RegisteredJournalLease::fresh(directory.file(), directory.identity(), &scope()).unwrap();
    let identity = lease.identity();
    assert_eq!(
        RegisteredJournalLease::recover(directory.file(), grant(identity), &scope()).err(),
        Some(Error::Busy)
    );
    let moved = directory.0.with_extension("moved");
    std::fs::rename(&directory.0, &moved).unwrap();
    lease.append(b"held\n").unwrap();
    assert_eq!(
        std::fs::read(moved.join(super::super::name(&scope()))).unwrap(),
        b"held\n"
    );
    std::fs::create_dir(&directory.0).unwrap();
    std::fs::set_permissions(&directory.0, std::fs::Permissions::from_mode(0o700)).unwrap();
    assert_eq!(
        RegisteredJournalLease::recover(directory.file(), grant(identity), &scope()).err(),
        Some(Error::Binding)
    );
    drop(lease);
    let mut recovered =
        RegisteredJournalLease::recover(File::open(&moved).unwrap(), grant(identity), &scope())
            .unwrap();
    assert_eq!(recovered.read().unwrap(), b"held\n");
    drop(recovered);
    std::fs::remove_dir_all(moved).unwrap();
}
#[test]
fn owned_frame_store_refuses_copy_hardlink_file_substitution_and_unavailable_history() {
    let directory = Directory::new();
    let mut lease =
        RegisteredJournalLease::fresh(directory.file(), directory.identity(), &scope()).unwrap();
    let identity = lease.identity();
    let path = directory.0.join(super::super::name(&scope()));
    assert_eq!(
        OwnedFrameStoreRegistration::grant_for_trusted_host(identity, &scope(), false)
            .err()
            .map(|_| Error::Policy),
        Some(Error::Policy)
    );
    let other = Directory::new();
    std::fs::copy(&path, other.0.join(super::super::name(&scope()))).unwrap();
    assert_eq!(
        RegisteredJournalLease::recover(other.file(), grant(identity), &scope()).err(),
        Some(Error::Binding)
    );
    std::fs::hard_link(&path, directory.0.join("alias")).unwrap();
    assert_eq!(lease.append(b"denied\n"), Err(Error::Binding));
    assert!(std::fs::read(&path).unwrap().is_empty());
    std::fs::remove_file(directory.0.join("alias")).unwrap();
    std::fs::rename(&path, directory.0.join("original")).unwrap();
    let replacement = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&path)
        .unwrap();
    replacement
        .set_permissions(std::fs::Permissions::from_mode(0o600))
        .unwrap();
    assert_eq!(lease.append(b"denied\n"), Err(Error::Binding));
    drop(lease);
    assert_eq!(
        RegisteredJournalLease::recover(directory.file(), grant(identity), &scope()).err(),
        Some(Error::Binding)
    );
}
#[test]
fn owned_frame_store_uncertain_append_before_and_after_persistence_requires_new_lease() {
    for after in [false, true] {
        let directory = Directory::new();
        let mut lease =
            RegisteredJournalLease::fresh(directory.file(), directory.identity(), &scope())
                .unwrap();
        let identity = lease.identity();
        lease.fail_append(1, after);
        assert_eq!(lease.append(b"candidate\n"), Err(Error::InDoubt));
        assert_eq!(lease.append(b"retry\n"), Err(Error::InDoubt));
        assert_eq!(lease.read(), Err(Error::InDoubt));
        drop(lease);
        let mut recovered =
            RegisteredJournalLease::recover(directory.file(), grant(identity), &scope()).unwrap();
        assert_eq!(
            recovered.read().unwrap(),
            if after {
                b"candidate\n".to_vec()
            } else {
                Vec::new()
            }
        );
    }
}
