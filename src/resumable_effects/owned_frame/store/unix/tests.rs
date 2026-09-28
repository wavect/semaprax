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

#[test]
fn owned_frame_inherited_lease_refuses_before_io_and_child_drop_preserves_parent_lock() {
    let directory = Directory::new();
    let lease =
        RegisteredJournalLease::fresh(directory.file(), directory.identity(), &scope()).unwrap();
    let identity = lease.identity();
    let mut child = std::process::Command::new(std::env::current_exe().unwrap());
    child.args([
        "owned_frame_inherited_child_probe",
        "--test-threads=1",
        "--nocapture",
    ]);
    child.env("SPX_OWNED_FRAME_CHILD_DIRECTORY", &directory.0);
    child.env(
        "SPX_OWNED_FRAME_CHILD_CREATOR",
        std::process::id().to_string(),
    );
    child.env(
        "SPX_OWNED_FRAME_CHILD_PINS",
        format!(
            "{}:{}:{}:{}",
            identity.directory_device,
            identity.directory_inode,
            identity.file_device,
            identity.file_inode
        ),
    );
    // Safe spawn passes this real shared open-file description as stdin. No
    // Rust callback runs between fork and exec and no raw FD escapes the test.
    child.stdin(std::process::Stdio::from(lease.file.try_clone().unwrap()));
    let output = child.output().unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        String::from_utf8_lossy(&output.stdout)
            .contains("SPX_INHERITED_LEASE_GUARD_AND_DROP_COMPLETED"),
        "child probe did not execute: {}",
        String::from_utf8_lossy(&output.stdout)
    );
    assert_eq!(
        RegisteredJournalLease::recover(directory.file(), grant(identity), &scope()).err(),
        Some(Error::Busy)
    );
    assert!(
        std::fs::read(directory.0.join(super::super::name(&scope())))
            .unwrap()
            .is_empty()
    );
    drop(child);
    drop(lease);
    let mut reopened =
        RegisteredJournalLease::recover(directory.file(), grant(identity), &scope()).unwrap();
    assert!(reopened.read().unwrap().is_empty());
}

#[test]
fn owned_frame_inherited_child_probe() {
    use std::os::fd::AsFd;
    let Some(directory) = std::env::var_os("SPX_OWNED_FRAME_CHILD_DIRECTORY") else {
        return;
    };
    let creator_process: u32 = std::env::var("SPX_OWNED_FRAME_CHILD_CREATOR")
        .unwrap()
        .parse()
        .unwrap();
    assert_ne!(creator_process, std::process::id());
    let pins: Vec<u64> = std::env::var("SPX_OWNED_FRAME_CHILD_PINS")
        .unwrap()
        .split(':')
        .map(|n| n.parse().unwrap())
        .collect();
    assert_eq!(pins.len(), 4);
    let identity = OwnedFrameStoreIdentity {
        directory_device: pins[0],
        directory_inode: pins[1],
        file_device: pins[2],
        file_inode: pins[3],
    };
    let mut inherited = RegisteredJournalLease {
        directory: File::open(directory).unwrap(),
        file: File::from(rustix::io::dup(std::io::stdin().as_fd()).unwrap()),
        identity,
        name: super::super::name(&scope()),
        scope: codec::scope(&scope()).unwrap(),
        profile: StoreProfile::OwnedFrameV1,
        length: 0,
        poisoned: false,
        creator_process,
        fault: None,
        writes: 0,
    };
    assert_eq!(inherited.validate_current(), Err(Error::Policy));
    assert_eq!(inherited.read(), Err(Error::Policy));
    assert_eq!(
        inherited.append(b"forbidden-child-write\n"),
        Err(Error::Policy)
    );
    assert_eq!(inherited.writes, 0);
    // Actual whole Drop in a different process must never unlock the parent's
    // shared flock. The parent tests Busy before releasing its own live lease.
    drop(inherited);
    println!("SPX_INHERITED_LEASE_GUARD_AND_DROP_COMPLETED");
}

#[test]
fn owned_frame_foreign_process_guard_and_whole_drop_do_not_unlock_shared_description() {
    let directory = Directory::new();
    let lease =
        RegisteredJournalLease::fresh(directory.file(), directory.identity(), &scope()).unwrap();
    let identity = lease.identity();
    let mut foreign = RegisteredJournalLease {
        directory: lease.directory.try_clone().unwrap(),
        file: lease.file.try_clone().unwrap(),
        identity,
        name: lease.name.clone(),
        scope: lease.scope.clone(),
        profile: lease.profile,
        length: lease.length,
        poisoned: false,
        creator_process: std::process::id().wrapping_add(1),
        fault: None,
        writes: 0,
    };
    assert_eq!(foreign.validate_current(), Err(Error::Policy));
    assert_eq!(foreign.read(), Err(Error::Policy));
    assert_eq!(foreign.append(b"forbidden\n"), Err(Error::Policy));
    drop(foreign); // actual whole Drop, safely exercised in the parent process
    assert_eq!(
        RegisteredJournalLease::recover(directory.file(), grant(identity), &scope()).err(),
        Some(Error::Busy)
    );
    drop(lease);
    assert!(RegisteredJournalLease::recover(directory.file(), grant(identity), &scope()).is_ok());
}
