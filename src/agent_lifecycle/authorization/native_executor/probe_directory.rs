//! Private 0700 probe directory used by `NativeStageExecutor` to compile and
//! run one generated native C stage. Split out of `native_executor.rs`
//! verbatim (module-size cap) -- see that module's doc comment for the wider
//! scope this executor operates in.

use std::fs::{File, OpenOptions};
use std::io::Write;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

use crate::diagnostic::Diagnostic;

use crate::agent_lifecycle::stages::invariant;

static NEXT_PROBE: AtomicU64 = AtomicU64::new(0);

fn probe_root() -> PathBuf {
    let ordinal = NEXT_PROBE.fetch_add(1, Ordering::Relaxed);
    std::env::temp_dir().join(format!(
        "semaprax-native-stage-executor-{}-{ordinal}",
        std::process::id()
    ))
}

/// One private 0700 probe directory held by descriptor. Every transition from
/// generated source to compiled child rechecks this held directory and opens
/// its child by `openat(..., NOFOLLOW)`, so replacing the path cannot redirect
/// the native stage executor into an attacker-selected file.
#[cfg(unix)]
pub(super) struct ProbeDirectory {
    path: PathBuf,
    pub(super) held: File,
    device: u64,
    inode: u64,
}

#[cfg(unix)]
impl ProbeDirectory {
    pub(super) fn create() -> Result<Self, Diagnostic> {
        use rustix::fs::{mkdir, open, Mode, OFlags};
        use std::os::unix::fs::{MetadataExt, PermissionsExt};
        let path = probe_root();
        mkdir(&path, Mode::from_bits_truncate(0o700))
            .map_err(|_| invariant("native_executor.probe_directory"))?;
        let held = open(
            &path,
            OFlags::RDONLY | OFlags::DIRECTORY | OFlags::NOFOLLOW | OFlags::CLOEXEC,
            Mode::empty(),
        )
        .map(File::from)
        .map_err(|_| invariant("native_executor.probe_directory"))?;
        let metadata = held
            .metadata()
            .map_err(|_| invariant("native_executor.probe_directory"))?;
        if !metadata.is_dir() || metadata.permissions().mode() & 0o077 != 0 {
            return Err(invariant("native_executor.probe_directory"));
        }
        Ok(Self {
            path,
            held,
            device: metadata.dev(),
            inode: metadata.ino(),
        })
    }

    pub(super) fn recheck(&self) -> Result<(), Diagnostic> {
        use std::os::unix::fs::{MetadataExt, PermissionsExt};
        let metadata = self
            .held
            .metadata()
            .map_err(|_| invariant("native_executor.probe_directory"))?;
        if !metadata.is_dir()
            || metadata.permissions().mode() & 0o077 != 0
            || metadata.dev() != self.device
            || metadata.ino() != self.inode
        {
            return Err(invariant("native_executor.probe_directory"));
        }
        Ok(())
    }

    pub(super) fn write_source(&self, source: &[u8]) -> Result<(), Diagnostic> {
        use rustix::fs::{openat, Mode, OFlags};
        self.recheck()?;
        let file = openat(
            &self.held,
            c"native_executor.c",
            OFlags::WRONLY | OFlags::CREATE | OFlags::EXCL | OFlags::NOFOLLOW | OFlags::CLOEXEC,
            Mode::from_bits_truncate(0o600),
        )
        .map(File::from)
        .map_err(|_| invariant("native_executor.write_source"))?;
        let mut file = file;
        file.write_all(source)
            .and_then(|()| file.sync_all())
            .map_err(|_| invariant("native_executor.write_source"))
    }

    pub(super) fn open_child(&self, name: &std::ffi::CStr) -> Result<File, Diagnostic> {
        use rustix::fs::{openat, Mode, OFlags};
        self.recheck()?;
        let child = openat(
            &self.held,
            name,
            OFlags::RDONLY | OFlags::NOFOLLOW | OFlags::CLOEXEC,
            Mode::empty(),
        )
        .map(File::from)
        .map_err(|_| invariant("native_executor.host.program_open"))?;
        if !child
            .metadata()
            .map_err(|_| invariant("native_executor.host.program_open"))?
            .is_file()
        {
            return Err(invariant("native_executor.host.program_open"));
        }
        Ok(child)
    }

    pub(super) fn cleanup(&self) {
        use std::os::unix::fs::MetadataExt;
        // Never recursively remove a path that could have been replaced by a
        // same-UID adversary. A drifted probe is intentionally left for the
        // host's temporary-file cleanup rather than deleting foreign data.
        let Ok(metadata) = std::fs::symlink_metadata(&self.path) else {
            return;
        };
        if metadata.is_dir() && metadata.dev() == self.device && metadata.ino() == self.inode {
            let _ = std::fs::remove_dir_all(&self.path);
        }
    }
}

#[cfg(not(unix))]
pub(super) struct ProbeDirectory {
    path: PathBuf,
    pub(super) held: File,
}

#[cfg(not(unix))]
impl ProbeDirectory {
    pub(super) fn create() -> Result<Self, Diagnostic> {
        let path = probe_root();
        std::fs::create_dir(&path).map_err(|_| invariant("native_executor.probe_directory"))?;
        let held = OpenOptions::new()
            .read(true)
            .open(&path)
            .map_err(|_| invariant("native_executor.probe_directory"))?;
        Ok(Self { path, held })
    }
    pub(super) fn recheck(&self) -> Result<(), Diagnostic> {
        Ok(())
    }
    pub(super) fn write_source(&self, source: &[u8]) -> Result<(), Diagnostic> {
        std::fs::write(self.path.join("native_executor.c"), source)
            .map_err(|_| invariant("native_executor.write_source"))
    }
    pub(super) fn open_child(&self, _name: &std::ffi::CStr) -> Result<File, Diagnostic> {
        OpenOptions::new()
            .read(true)
            .open(self.path.join("native_executor"))
            .map_err(|_| invariant("native_executor.host.program_open"))
    }
    pub(super) fn cleanup(&self) {
        let _ = std::fs::remove_dir_all(&self.path);
    }
}
