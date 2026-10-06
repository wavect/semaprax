//! The one durability rule ADR 0005 names as "the single most commonly
//! botched detail here": fsync the file *and* its parent directory before
//! acknowledging a write.
//!
//! A file-only `fsync` can leave the directory entry that names the file
//! unwritten on some filesystems after a crash — the rename that publishes a
//! new generation, or the pointer flip that selects it, can be lost even
//! though the file's own bytes are safely on disk. This module is the only
//! place in `durable_jobs` that touches a filesystem, so every acknowledged
//! write in the store goes through exactly this sequence:
//!
//! 1. Write the complete bytes to a staging file in the *same* directory the
//!    final name lives in (so the rename in step 3 is same-filesystem and
//!    therefore atomic).
//! 2. `flush` and `sync_all` (fsync) the staging file.
//! 3. Atomically rename the staging file onto its final name.
//! 4. Open the containing directory and `sync_all` (fsync) it, so the
//!    directory entry created by the rename is itself durable.
//!
//! Only after step 4 returns does [`commit_bytes`] return `Ok`. A crash
//! injected before step 3 leaves the prior final file completely untouched
//! (see `store::tests` for the fault-injection regressions that exercise
//! this file's `HookPoint`s); a crash after step 3 but before step 4 is the
//! exact gap this module exists to close.
//!
//! Directory fsync has no portable API outside Unix-family targets; on
//! other targets step 4 is skipped and this module does not claim the same
//! durability there. This mirrors the existing precedent in
//! `src/candidate_archive_store.rs` and `src/job_runtime.rs`
//! (`FileJobCheckpointStore`), which gate their own physical stores to Unix
//! targets for the same reason.

use std::fs::{self, File, OpenOptions};
use std::io::{self, Write};
use std::path::{Path, PathBuf};

/// Points in [`commit_bytes`]'s sequence a test may inject a fault at. Before
/// rename the destination is untouched; after rename it is publication
/// uncertainty.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HookPoint {
    AfterStageWrite,
    AfterStageFsync,
    AfterRename,
}

pub type Hook<'a> = dyn FnMut(HookPoint) -> io::Result<()> + 'a;

/// A failed durable write distinguishes the point before its destination was
/// renamed from the point after it became visible.  The latter cannot be
/// reported as a retry-safe rollback to the generation store.
#[derive(Debug)]
pub(crate) enum CommitBytesError {
    NotPublished(io::Error),
    PublishedUncertain(io::Error),
}

fn run_hook(hook: &mut Option<&mut Hook<'_>>, point: HookPoint) -> io::Result<()> {
    match hook {
        Some(hook) => hook(point),
        None => Ok(()),
    }
}

/// Durably write `bytes` to `destination`, which must already have a parent
/// directory that exists. `stage_name` selects the sibling staging file name
/// (callers pick something unique per attempt so concurrent commits to
/// different destinations in the same directory cannot collide).
///
/// On success, `destination` contains exactly `bytes` and that fact has
/// survived an fsync of both the file and its parent directory. A failure
/// before rename leaves `destination` unchanged. A failure after rename is
/// [`CommitBytesError::PublishedUncertain`]: the new destination is already
/// visible and must not be treated as a retry-safe rollback.
pub(crate) fn commit_bytes(
    destination: &Path,
    stage_name: &str,
    bytes: &[u8],
) -> Result<(), CommitBytesError> {
    commit_bytes_with_hook(destination, stage_name, bytes, &mut None)
}

pub(crate) fn commit_bytes_with_hook(
    destination: &Path,
    stage_name: &str,
    bytes: &[u8],
    hook: &mut Option<&mut Hook<'_>>,
) -> Result<(), CommitBytesError> {
    let dir = destination
        .parent()
        .ok_or_else(|| io::Error::other("destination has no parent directory"))
        .map_err(CommitBytesError::NotPublished)?;
    let stage_path = dir.join(stage_name);
    // `create_new` refuses to clobber a stray leftover from a prior crashed
    // attempt silently; the caller is expected to pick a fresh stage name
    // per attempt (see `store::GenerationJobStore` callers).
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&stage_path)
        .map_err(CommitBytesError::NotPublished)?;
    let mut published = false;
    let result = (|| -> io::Result<()> {
        file.write_all(bytes)?;
        file.flush()?;
        run_hook(hook, HookPoint::AfterStageWrite)?;
        file.sync_all()?;
        run_hook(hook, HookPoint::AfterStageFsync)?;
        drop_and_forget(file);
        fs::rename(&stage_path, destination)?;
        published = true;
        run_hook(hook, HookPoint::AfterRename)?;
        sync_directory(dir)?;
        Ok(())
    })();
    let result = match result {
        Ok(()) => Ok(()),
        Err(error) if !published => Err(CommitBytesError::NotPublished(error)),
        Err(error) => Err(CommitBytesError::PublishedUncertain(error)),
    };
    if matches!(&result, Err(CommitBytesError::NotPublished(_))) {
        // Best-effort cleanup of the stage file on any failure path; if the
        // rename already happened this is a no-op (the stage path no
        // longer exists), and if it did not, the destination is untouched.
        let _ = fs::remove_file(&stage_path);
    }
    result
}

/// `File`'s value isn't needed after `sync_all`; this exists only to make
/// the drop point explicit at the call site above rather than relying on
/// end-of-closure drop order, since the file must be closed before rename
/// on some platforms' locking semantics.
fn drop_and_forget(file: File) {
    drop(file);
}

#[cfg(unix)]
pub(crate) fn sync_directory(dir: &Path) -> io::Result<()> {
    File::open(dir)?.sync_all()
}

#[cfg(not(unix))]
pub(crate) fn sync_directory(_dir: &Path) -> io::Result<()> {
    Ok(())
}

/// Read a small pointer/generation file fully into memory. A missing file is
/// reported as `Ok(None)` (the store's "no generation committed yet" case),
/// never a hard error.
pub fn read_optional(path: &Path) -> io::Result<Option<Vec<u8>>> {
    match fs::read(path) {
        Ok(bytes) => Ok(Some(bytes)),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(None),
        Err(error) => Err(error),
    }
}

/// Ensure `dir` exists (creating it and any missing ancestors), returning
/// its canonical-enough form for joining. This module never creates
/// anything outside the directory the caller explicitly names.
pub fn ensure_dir(dir: &Path) -> io::Result<PathBuf> {
    fs::create_dir_all(dir)?;
    Ok(dir.to_path_buf())
}

/// Canonicalize a store root after creation, so relative spellings and
/// symlink aliases contend on the same writer-lock file.
pub(crate) fn canonical_existing_dir(dir: &Path) -> io::Result<PathBuf> {
    let canonical = fs::canonicalize(dir)?;
    if !canonical.is_dir() {
        return Err(io::Error::other("job store root is not a directory"));
    }
    Ok(canonical)
}

/// A live exclusive writer claim for one canonical generation-store root.
/// The lock is advisory but OS-backed; its file may survive process death,
/// while the kernel lock is released with the dead process's descriptor.
pub(crate) struct JobWriterLock {
    file: File,
}

impl Drop for JobWriterLock {
    fn drop(&mut self) {
        let _ = fs2::FileExt::unlock(&self.file);
    }
}

/// Acquire the one supported writer role for this store root.  The fixed
/// filename is only the lock rendezvous point, never evidence that an owner
/// is alive; `try_lock_exclusive` decides that from the OS-held lock.
pub(crate) fn acquire_job_writer_lock(root: &Path) -> io::Result<JobWriterLock> {
    let file = OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .open(root.join(".generation-job-store.lock"))?;
    fs2::FileExt::try_lock_exclusive(&file)?;
    Ok(JobWriterLock { file })
}

pub(crate) fn is_writer_contention(error: &io::Error) -> bool {
    if error.kind() == io::ErrorKind::WouldBlock {
        return true;
    }
    #[cfg(windows)]
    {
        // LockFileEx and CreateFileW report contention with these raw errors.
        matches!(error.raw_os_error(), Some(32) | Some(33))
    }
    #[cfg(not(windows))]
    false
}

/// Find the first stage sequence that cannot name a stage left by an earlier
/// store instance.  A process can stop after creating a stage file but before
/// the best-effort cleanup in [`commit_bytes_with_hook`]; recovery must leave
/// that unknown file alone and choose a later name instead.
///
/// Only the two names owned by `GenerationJobStore` are considered.  The
/// scan is capped so a malformed root cannot turn opening the small local
/// store into unbounded work.  Reaching the cap is an I/O refusal rather than
/// permission to reuse an ambiguous stage name.
pub(crate) fn next_job_stage_sequence(directories: [&Path; 2]) -> io::Result<u64> {
    const MAX_OWNED_STAGES: usize = 1024;
    const PREFIXES: [&str; 2] = [".stage-generation-", ".stage-active-"];

    let mut owned_stages = 0usize;
    let mut highest = None;
    for directory in directories {
        for entry in fs::read_dir(directory)? {
            let name = entry?.file_name();
            let Some(name) = name.to_str() else {
                continue;
            };
            let Some(suffix) = PREFIXES.iter().find_map(|prefix| name.strip_prefix(prefix)) else {
                continue;
            };
            let Ok(sequence) = suffix.parse::<u64>() else {
                continue;
            };
            owned_stages = owned_stages
                .checked_add(1)
                .ok_or_else(|| io::Error::other("owned stage count overflow"))?;
            if owned_stages > MAX_OWNED_STAGES {
                return Err(io::Error::other("too many abandoned job stages"));
            }
            highest = Some(highest.map_or(sequence, |current: u64| current.max(sequence)));
        }
    }
    match highest {
        Some(sequence) => sequence
            .checked_add(1)
            .ok_or_else(|| io::Error::other("job stage sequence exhausted")),
        None => Ok(0),
    }
}

/// Return the greatest committed-generation filename. Generation files are
/// immutable once renamed, including an unreachable one left when the later
/// `ACTIVE` publication fails. The bounded scan prevents a malformed root
/// from making recovery unbounded.
pub(crate) fn highest_job_generation(directory: &Path) -> io::Result<u64> {
    const MAX_GENERATION_FILES: usize = 65_536;
    let mut count = 0usize;
    let mut highest = 0u64;
    for entry in fs::read_dir(directory)? {
        let name = entry?.file_name();
        let Some(name) = name.to_str() else {
            continue;
        };
        let Ok(generation) = name.parse::<u64>() else {
            continue;
        };
        count += 1;
        if count > MAX_GENERATION_FILES {
            return Err(io::Error::other("too many job generations"));
        }
        highest = highest.max(generation);
    }
    Ok(highest)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Read;

    fn tempdir(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "semaprax-durable-jobs-durable-fs-{name}-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn commit_writes_exactly_the_given_bytes() {
        let dir = tempdir("basic");
        let dest = dir.join("generation-1");
        commit_bytes(&dest, "stage-1", b"hello durable world").unwrap();
        let mut got = Vec::new();
        File::open(&dest).unwrap().read_to_end(&mut got).unwrap();
        assert_eq!(got, b"hello durable world");
        fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn commit_overwrites_a_prior_generation_atomically() {
        let dir = tempdir("overwrite");
        let dest = dir.join("generation-1");
        commit_bytes(&dest, "stage-1", b"first").unwrap();
        commit_bytes(&dest, "stage-2", b"second-and-longer").unwrap();
        let mut got = Vec::new();
        File::open(&dest).unwrap().read_to_end(&mut got).unwrap();
        assert_eq!(got, b"second-and-longer");
        fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn a_fault_before_rename_leaves_the_destination_completely_untouched() {
        let dir = tempdir("fault-before-rename");
        let dest = dir.join("generation-1");
        commit_bytes(&dest, "stage-1", b"original").unwrap();

        for point in [HookPoint::AfterStageWrite, HookPoint::AfterStageFsync] {
            let mut closure = |seen: HookPoint| {
                if seen == point {
                    Err(io::Error::other("injected"))
                } else {
                    Ok(())
                }
            };
            let mut hook: Option<&mut Hook<'_>> = Some(&mut closure);
            let result =
                commit_bytes_with_hook(&dest, "stage-fault", b"should never land", &mut hook);
            assert!(result.is_err(), "{point:?}");
            let mut got = Vec::new();
            File::open(&dest).unwrap().read_to_end(&mut got).unwrap();
            assert_eq!(got, b"original", "destination changed at {point:?}");
            // No leftover stage file from the failed attempt.
            assert!(!dir.join("stage-fault").exists());
        }
        fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn a_fault_after_rename_still_leaves_the_new_bytes_visible() {
        // Once the rename has happened the new generation IS the
        // destination; a subsequent directory-fsync failure is a durability
        // risk for that specific write's acknowledgement, not a data loss
        // of what is already readable in this process.
        let dir = tempdir("fault-after-rename");
        let dest = dir.join("generation-1");
        let mut closure = |seen: HookPoint| {
            if seen == HookPoint::AfterRename {
                Err(io::Error::other("injected"))
            } else {
                Ok(())
            }
        };
        let mut hook: Option<&mut Hook<'_>> = Some(&mut closure);
        let result = commit_bytes_with_hook(&dest, "stage-1", b"landed", &mut hook);
        assert!(result.is_err());
        let mut got = Vec::new();
        File::open(&dest).unwrap().read_to_end(&mut got).unwrap();
        assert_eq!(got, b"landed");
        fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn read_optional_reports_none_for_a_missing_file_not_an_error() {
        let dir = tempdir("missing");
        assert_eq!(read_optional(&dir.join("nope")).unwrap(), None);
        fs::remove_dir_all(&dir).ok();
    }
}
