//! Cooperating-writer latest storage: every store operation uses a held directory.
//! Digests do not authenticate an operator-replaced store or grant freshness.
use super::CliError;
use std::fs::File;
use std::io::Read;
use std::path::Path;

fn read_file(mut file: File, maximum: usize) -> Result<Vec<u8>, CliError> {
    let metadata = file
        .metadata()
        .map_err(|_| CliError::refused("cannot inspect opened input"))?;
    if !metadata.is_file() || metadata.len() > maximum as u64 {
        return Err(CliError::refused(
            "bounded input is not a regular file within limit",
        ));
    }
    let mut bytes = Vec::new();
    file.by_ref()
        .take(maximum as u64 + 1)
        .read_to_end(&mut bytes)
        .map_err(|_| CliError::refused("cannot read bounded input"))?;
    if bytes.len() > maximum {
        return Err(CliError::refused("bounded input grew beyond limit"));
    }
    Ok(bytes)
}

pub(super) fn bounded_read(path: &Path, maximum: usize) -> Result<Vec<u8>, CliError> {
    #[cfg(unix)]
    let file = File::from(
        rustix::fs::open(
            path,
            rustix::fs::OFlags::RDONLY
                | rustix::fs::OFlags::NOFOLLOW
                | rustix::fs::OFlags::NONBLOCK
                | rustix::fs::OFlags::CLOEXEC,
            rustix::fs::Mode::empty(),
        )
        .map_err(|_| CliError::refused("cannot open bounded input"))?,
    );
    #[cfg(not(unix))]
    let file = {
        let metadata = path
            .symlink_metadata()
            .map_err(|_| CliError::refused("cannot inspect bounded input"))?;
        if !metadata.is_file() {
            return Err(CliError::refused("bounded input is not a regular file"));
        }
        File::open(path).map_err(|_| CliError::refused("cannot open bounded input"))?
    };
    read_file(file, maximum)
}

#[cfg(unix)]
mod platform {
    use super::{read_file, CliError};
    use rustix::fs::{flock, mkdirat, open, openat, renameat, FlockOperation, Mode, OFlags};
    use rustix::io::Errno;
    use semaprax::agent_lifecycle::{CheckpointStore, CheckpointStoreError};
    use semaprax::live_invocation::source_journal::MAX_SOURCE_DOCUMENT_BYTES;
    use std::fs::File;
    use std::io::Write;
    use std::os::unix::fs::{MetadataExt, PermissionsExt};
    use std::path::{Component, Path, PathBuf};

    #[cfg(test)]
    use std::cell::RefCell;

    const DOCUMENT: &str = "checkpoint.json";
    const LOCK: &str = "writer.lock";
    const CLAIM: &str = "handoff.claim";
    const READ: OFlags = OFlags::RDONLY
        .union(OFlags::NOFOLLOW)
        .union(OFlags::NONBLOCK)
        .union(OFlags::CLOEXEC);
    const DIRECTORY: OFlags = READ.union(OFlags::DIRECTORY);
    const PRIVATE: Mode = Mode::RUSR.union(Mode::WUSR);

    /// A test-only physical commit fault. The matching document has already
    /// been rendered by the real journal and is still written by the held
    /// directory store; this controls only which durability boundary loses its
    /// acknowledgement.
    #[cfg(test)]
    #[derive(Clone, Copy)]
    pub(in crate::source_live_cli) enum CommitFault {
        BeforeWrite(&'static str),
        AfterRename(&'static str),
    }

    #[cfg(test)]
    thread_local! {
        static COMMIT_FAULT: RefCell<Option<CommitFault>> = const { RefCell::new(None) };
    }

    #[cfg(test)]
    pub(in crate::source_live_cli) fn inject_commit_fault(fault: CommitFault) {
        COMMIT_FAULT.with(|slot| *slot.borrow_mut() = Some(fault));
    }

    #[cfg(test)]
    fn take_commit_fault(document: &str, after_rename: bool) -> bool {
        COMMIT_FAULT.with(|slot| {
            let matched = match *slot.borrow() {
                Some(CommitFault::BeforeWrite(kind)) => !after_rename && document.contains(kind),
                Some(CommitFault::AfterRename(kind)) => after_rename && document.contains(kind),
                None => false,
            };
            if matched {
                *slot.borrow_mut() = None;
            }
            matched
        })
    }

    pub(in crate::source_live_cli) struct CheckpointDir {
        path: PathBuf,
        directory: File,
        _lock: File,
        generation: u64,
        poisoned: bool,
    }

    fn is_absolute_like(path: &Path) -> bool {
        path.is_absolute() || path.to_string_lossy().starts_with('/')
    }

    // Walk physical components using held descriptors. A path check followed
    // by reopening its spelling would lose this authority under rename.
    fn directory(path: &Path, project_root: &Path) -> Result<File, CliError> {
        if !is_absolute_like(path) {
            return Err(CliError::refused("checkpoint directory must be absolute"));
        }
        let project = project_root
            .metadata()
            .map_err(|_| CliError::refused("Project root unavailable"))?;
        let mut current = File::from(
            open("/", DIRECTORY, Mode::empty())
                .map_err(|_| CliError::refused("cannot open filesystem root"))?,
        );
        for component in path.components() {
            match component {
                Component::RootDir => {}
                Component::Normal(name) => {
                    current = File::from(
                        openat(&current, name, DIRECTORY, Mode::empty()).map_err(|_| {
                            CliError::refused("checkpoint ancestor is not a physical directory")
                        })?,
                    );
                }
                _ => {
                    return Err(CliError::refused(
                        "checkpoint path contains a noncanonical component",
                    ))
                }
            }
            let meta = current
                .metadata()
                .map_err(|_| CliError::refused("cannot inspect checkpoint ancestor"))?;
            if meta.dev() == project.dev() && meta.ino() == project.ino() {
                return Err(CliError::refused(
                    "checkpoint directory must be outside the Project",
                ));
            }
        }
        Ok(current)
    }

    fn read_at(directory: &File, name: &str, maximum: usize) -> Result<Option<Vec<u8>>, CliError> {
        match openat(directory, name, READ, Mode::empty()) {
            Ok(fd) => read_file(File::from(fd), maximum).map(Some),
            Err(Errno::NOENT) => Ok(None),
            Err(_) => Err(CliError::refused("cannot open physical checkpoint input")),
        }
    }

    impl CheckpointDir {
        pub(in crate::source_live_cli) fn fresh(
            path: &Path,
            project_root: &Path,
        ) -> Result<Self, CliError> {
            let parent = path
                .parent()
                .ok_or(CliError::refused("checkpoint directory has no parent"))?;
            let name = path
                .file_name()
                .ok_or(CliError::refused("checkpoint directory has no name"))?;
            let parent = directory(parent, project_root)?;
            mkdirat(&parent, name, PRIVATE | Mode::XUSR)
                .map_err(|_| CliError::refused("checkpoint directory must be new"))?;
            let held = File::from(
                openat(&parent, name, DIRECTORY, Mode::empty())
                    .map_err(|_| CliError::refused("cannot hold new checkpoint directory"))?,
            );
            parent
                .sync_all()
                .map_err(|_| CliError::refused("cannot acknowledge checkpoint creation"))?;
            Self::from_directory(path, held)
        }

        pub(in crate::source_live_cli) fn existing(
            path: &Path,
            project_root: &Path,
        ) -> Result<Self, CliError> {
            Self::from_directory(path, directory(path, project_root)?)
        }

        fn from_directory(path: &Path, directory: File) -> Result<Self, CliError> {
            let meta = directory
                .metadata()
                .map_err(|_| CliError::refused("cannot inspect checkpoint directory"))?;
            if !meta.is_dir()
                || meta.permissions().mode() & 0o077 != 0
                || meta.uid() != rustix::process::geteuid().as_raw()
            {
                return Err(CliError::refused(
                    "checkpoint directory is not private and owned",
                ));
            }
            let lock = File::from(
                openat(
                    &directory,
                    LOCK,
                    OFlags::RDWR
                        | OFlags::CREATE
                        | OFlags::NOFOLLOW
                        | OFlags::NONBLOCK
                        | OFlags::CLOEXEC,
                    PRIVATE,
                )
                .map_err(|_| CliError::refused("cannot open checkpoint lock"))?,
            );
            let meta = lock
                .metadata()
                .map_err(|_| CliError::refused("cannot inspect checkpoint lock"))?;
            if !meta.is_file() || meta.nlink() != 1 {
                return Err(CliError::refused(
                    "checkpoint lock is not a private regular file",
                ));
            }
            flock(&lock, FlockOperation::NonBlockingLockExclusive)
                .map_err(|_| CliError::refused("checkpoint directory already has a writer"))?;
            Ok(Self {
                path: path.to_owned(),
                directory,
                _lock: lock,
                generation: 0,
                poisoned: false,
            })
        }

        pub(in crate::source_live_cli) fn path(&self) -> &Path {
            &self.path
        }
        pub(in crate::source_live_cli) fn latest(&self) -> Result<Option<String>, CliError> {
            read_at(&self.directory, DOCUMENT, MAX_SOURCE_DOCUMENT_BYTES)?
                .map(|bytes| {
                    String::from_utf8(bytes)
                        .map_err(|_| CliError::refused("checkpoint document is not UTF-8"))
                })
                .transpose()
        }
        pub(in crate::source_live_cli) fn set_generation(&mut self, generation: u64) {
            self.generation = generation;
        }

        pub(in crate::source_live_cli) fn claim_handoff(
            &self,
            handoff: &str,
            destination: &Path,
            invocation: &str,
        ) -> Result<(), CliError> {
            let destination = destination
                .to_str()
                .ok_or(CliError::refused("destination path is not UTF-8"))?;
            let expected = format!("{{\"schema\":\"semaprax.source-live-cli.handoff-claim.v1\",\"handoff\":{},\"destination\":{},\"invocation\":{}}}\n",
                serde_json::to_string(handoff).unwrap(), serde_json::to_string(destination).unwrap(), serde_json::to_string(invocation).unwrap());
            if expected.len() > 2048 {
                return Err(CliError::refused("handoff claim exceeds limit"));
            }
            if let Some(old) = read_at(&self.directory, CLAIM, 2048)? {
                if old != expected.as_bytes() {
                    return Err(CliError::refused(
                        "predecessor already claimed a different destination",
                    ));
                }
                // A prior invocation may have written the exact bytes but lost
                // its durability ACK. Re-establish it before using the claim.
                let claim = File::from(
                    openat(&self.directory, CLAIM, READ, Mode::empty())
                        .map_err(|_| CliError::refused("cannot reopen handoff claim"))?,
                );
                claim
                    .sync_all()
                    .and_then(|_| self.directory.sync_all())
                    .map_err(|_| CliError::refused("cannot acknowledge retained handoff claim"))?;
                return Ok(());
            }
            let mut file = File::from(
                openat(
                    &self.directory,
                    CLAIM,
                    OFlags::WRONLY
                        | OFlags::CREATE
                        | OFlags::EXCL
                        | OFlags::NOFOLLOW
                        | OFlags::CLOEXEC,
                    PRIVATE,
                )
                .map_err(|_| CliError::refused("cannot create handoff claim"))?,
            );
            file.write_all(expected.as_bytes())
                .and_then(|_| file.sync_all())
                .and_then(|_| self.directory.sync_all())
                .map_err(|_| CliError::refused("cannot acknowledge handoff claim"))
        }
    }

    impl CheckpointStore for CheckpointDir {
        fn commit(&mut self, generation: u64, document: &str) -> Result<(), CheckpointStoreError> {
            if self.poisoned
                || self.generation.checked_add(1) != Some(generation)
                || document.len() > MAX_SOURCE_DOCUMENT_BYTES
            {
                return Err(CheckpointStoreError);
            }
            // Any failed attempt may have changed physical state. Only a new
            // invocation may recover latest; this instance never retries it.
            self.poisoned = true;
            let scratch = format!(".checkpoint.{generation}.{}.tmp", std::process::id());
            let mut staged = File::from(
                openat(
                    &self.directory,
                    scratch.as_str(),
                    OFlags::WRONLY
                        | OFlags::CREATE
                        | OFlags::EXCL
                        | OFlags::NOFOLLOW
                        | OFlags::CLOEXEC,
                    PRIVATE,
                )
                .map_err(|_| CheckpointStoreError)?,
            );
            #[cfg(test)]
            if take_commit_fault(document, false) {
                return Err(CheckpointStoreError);
            }
            staged
                .write_all(document.as_bytes())
                .and_then(|_| staged.sync_all())
                .map_err(|_| CheckpointStoreError)?;
            renameat(&self.directory, scratch.as_str(), &self.directory, DOCUMENT)
                .map_err(|_| CheckpointStoreError)?;
            #[cfg(test)]
            if take_commit_fault(document, true) {
                return Err(CheckpointStoreError);
            }
            self.directory
                .sync_all()
                .map_err(|_| CheckpointStoreError)?;
            self.generation = generation;
            self.poisoned = false;
            Ok(())
        }
    }
}

#[cfg(not(unix))]
mod platform {
    use super::CliError;
    use semaprax::agent_lifecycle::{CheckpointStore, CheckpointStoreError};
    use std::path::Path;
    pub(in crate::source_live_cli) struct CheckpointDir;
    impl CheckpointDir {
        pub(in crate::source_live_cli) fn fresh(_: &Path, _: &Path) -> Result<Self, CliError> {
            Err(CliError::refused(
                "source-live CLI requires a Unix checkpoint host",
            ))
        }
        pub(in crate::source_live_cli) fn existing(_: &Path, _: &Path) -> Result<Self, CliError> {
            Err(CliError::refused(
                "source-live CLI requires a Unix checkpoint host",
            ))
        }
        pub(in crate::source_live_cli) fn path(&self) -> &Path {
            unreachable!()
        }
        pub(in crate::source_live_cli) fn latest(&self) -> Result<Option<String>, CliError> {
            unreachable!()
        }
        pub(in crate::source_live_cli) fn set_generation(&mut self, _: u64) {
            unreachable!()
        }
        pub(in crate::source_live_cli) fn claim_handoff(
            &self,
            _: &str,
            _: &Path,
            _: &str,
        ) -> Result<(), CliError> {
            unreachable!()
        }
    }
    impl CheckpointStore for CheckpointDir {
        fn commit(&mut self, _: u64, _: &str) -> Result<(), CheckpointStoreError> {
            Err(CheckpointStoreError)
        }
    }
}
pub(super) use platform::CheckpointDir;
#[cfg(all(test, unix))]
pub(super) use platform::{inject_commit_fault, CommitFault};
