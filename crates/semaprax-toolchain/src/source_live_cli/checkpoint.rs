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
    use semaprax::digest_hex::LowerHex;
    use semaprax::live_invocation::source_journal::MAX_SOURCE_DOCUMENT_BYTES;
    use serde_json::Value;
    use sha2::{Digest, Sha256};
    use std::fs::File;
    use std::io::Write;
    use std::os::unix::fs::{MetadataExt, PermissionsExt};
    use std::path::{Component, Path, PathBuf};

    #[cfg(test)]
    use std::cell::RefCell;

    const DOCUMENT: &str = "checkpoint.json";
    const LOCK: &str = "writer.lock";
    const CLAIM: &str = "handoff.claim";
    const TERMINAL_PATCH_RECEIPT: &str = "terminal-patch-receipt.json";
    const TERMINAL_PATCH_RECEIPT_COMMITMENT: &str = "terminal-patch-receipt-commitment.json";
    const TERMINAL_PATCH_RECEIPT_COMMITMENT_SCHEMA: &str =
        "semaprax.source-live-cli.terminal-patch-receipt-commitment.v1";
    const TERMINAL_PATCH_RECEIPT_DOCUMENT_DOMAIN: &[u8] =
        b"semaprax.source-live-cli.terminal-patch-receipt-document.v1\0";
    const CHECKPOINT_DOCUMENT_DOMAIN: &[u8] =
        b"semaprax.source-live-cli.terminal-patch-receipt-checkpoint.v1\0";
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

    /// A test-only predecessor-claim fault. This is separate from journal
    /// commit faults because the claim is a one-way handoff exclusion record,
    /// not a checkpoint generation.
    #[cfg(test)]
    #[derive(Clone, Copy)]
    pub(in crate::source_live_cli) enum HandoffClaimFault {
        BeforeWrite,
        BeforeSync,
    }

    #[cfg(test)]
    thread_local! {
        static COMMIT_FAULT: RefCell<Option<CommitFault>> = const { RefCell::new(None) };
        static HANDOFF_CLAIM_FAULT: RefCell<Option<HandoffClaimFault>> = const { RefCell::new(None) };
    }

    #[cfg(test)]
    pub(in crate::source_live_cli) fn inject_commit_fault(fault: CommitFault) {
        COMMIT_FAULT.with(|slot| *slot.borrow_mut() = Some(fault));
    }

    #[cfg(test)]
    pub(in crate::source_live_cli) fn commit_fault_pending() -> bool {
        COMMIT_FAULT.with(|slot| slot.borrow().is_some())
    }

    #[cfg(test)]
    pub(in crate::source_live_cli) fn inject_handoff_claim_fault(fault: HandoffClaimFault) {
        HANDOFF_CLAIM_FAULT.with(|slot| *slot.borrow_mut() = Some(fault));
    }

    #[cfg(test)]
    fn take_handoff_claim_fault(expected: HandoffClaimFault) -> bool {
        HANDOFF_CLAIM_FAULT.with(|slot| {
            let actual = slot.borrow_mut().take();
            match actual {
                Some(actual)
                    if std::mem::discriminant(&actual) == std::mem::discriminant(&expected) =>
                {
                    true
                }
                other => {
                    *slot.borrow_mut() = other;
                    false
                }
            }
        })
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

    fn document_digest(domain: &[u8], bytes: &[u8]) -> String {
        let mut hash = Sha256::new();
        hash.update(domain);
        hash.update((bytes.len() as u64).to_le_bytes());
        hash.update(bytes);
        format!("sha256:{:x}", LowerHex(hash.finalize()))
    }

    fn terminal_receipt_commitment(
        receipt_document: &str,
        receipt_digest: &str,
        checkpoint_document: &str,
    ) -> Result<String, CliError> {
        let document = serde_json::to_string(&serde_json::json!({
            "schema": TERMINAL_PATCH_RECEIPT_COMMITMENT_SCHEMA,
            "checkpoint_document_digest": document_digest(
                CHECKPOINT_DOCUMENT_DOMAIN,
                checkpoint_document.as_bytes(),
            ),
            "terminal_receipt_document_digest": document_digest(
                TERMINAL_PATCH_RECEIPT_DOCUMENT_DOMAIN,
                receipt_document.as_bytes(),
            ),
            "receipt_digest": receipt_digest,
        }))
        .map(|document| format!("{document}\n"))
        .map_err(|_| CliError::refused("terminal patch receipt commitment cannot be rendered"))?;
        (document.len() <= 2048)
            .then_some(document)
            .ok_or(CliError::refused(
                "terminal patch receipt commitment exceeds limit",
            ))
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

        fn retain_terminal_document(
            &self,
            name: &str,
            document: &str,
            maximum: usize,
            label: &str,
        ) -> Result<(), CliError> {
            if document.len() > maximum {
                return Err(CliError::detail(format!("{label} exceeds limit")));
            }
            if let Some(existing) = read_at(&self.directory, name, maximum)? {
                if existing != document.as_bytes() {
                    return Err(CliError::detail(format!(
                        "{label} conflicts with retained checkpoint",
                    )));
                }
                let retained = File::from(
                    openat(&self.directory, name, READ, Mode::empty())
                        .map_err(|_| CliError::detail(format!("cannot reopen {label}")))?,
                );
                retained
                    .sync_all()
                    .and_then(|_| self.directory.sync_all())
                    .map_err(|_| CliError::detail(format!("cannot acknowledge {label}")))?;
                return Ok(());
            }
            let scratch = format!(".{name}.{}.tmp", std::process::id());
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
                .map_err(|_| CliError::detail(format!("cannot stage {label}")))?,
            );
            staged
                .write_all(document.as_bytes())
                .and_then(|_| staged.sync_all())
                .map_err(|_| CliError::detail(format!("cannot retain {label}")))?;
            renameat(&self.directory, scratch.as_str(), &self.directory, name)
                .map_err(|_| CliError::detail(format!("cannot retain {label}")))?;
            self.directory
                .sync_all()
                .map_err(|_| CliError::detail(format!("cannot retain {label}")))
        }

        /// Retains the exact compiler-owned patch receipt produced alongside a
        /// terminal repair checkpoint. The held checkpoint directory and its
        /// writer lock also retain a separate commitment to the receipt bytes
        /// and the exact journal document that admitted the terminal state.
        pub(in crate::source_live_cli) fn retain_terminal_patch_receipt(
            &self,
            document: &str,
            receipt_digest: &str,
            checkpoint_document: &str,
        ) -> Result<(), CliError> {
            self.retain_terminal_document(
                TERMINAL_PATCH_RECEIPT,
                document,
                16 * 1024,
                "terminal patch receipt",
            )?;
            let commitment =
                terminal_receipt_commitment(document, receipt_digest, checkpoint_document)?;
            self.retain_terminal_document(
                TERMINAL_PATCH_RECEIPT_COMMITMENT,
                &commitment,
                2048,
                "terminal patch receipt commitment",
            )
        }

        pub(in crate::source_live_cli) fn terminal_patch_receipt(
            &self,
        ) -> Result<Option<String>, CliError> {
            read_at(&self.directory, TERMINAL_PATCH_RECEIPT, 16 * 1024)?
                .map(|bytes| {
                    String::from_utf8(bytes)
                        .map_err(|_| CliError::refused("terminal patch receipt is not UTF-8"))
                })
                .transpose()
        }

        pub(in crate::source_live_cli) fn terminal_patch_receipt_commitment(
            &self,
            receipt_document: &str,
            checkpoint_document: &str,
        ) -> Result<Option<String>, CliError> {
            let Some(document) = read_at(&self.directory, TERMINAL_PATCH_RECEIPT_COMMITMENT, 2048)?
            else {
                return Ok(None);
            };
            let document = String::from_utf8(document)
                .map_err(|_| CliError::refused("terminal patch receipt commitment is not UTF-8"))?;
            let value: Value = serde_json::from_str(&document)
                .map_err(|_| CliError::refused("terminal patch receipt commitment is malformed"))?;
            let object = value.as_object().ok_or(CliError::refused(
                "terminal patch receipt commitment is malformed",
            ))?;
            let keys = object.keys().map(String::as_str).collect::<Vec<_>>();
            if keys.as_slice()
                != [
                    "checkpoint_document_digest",
                    "receipt_digest",
                    "schema",
                    "terminal_receipt_document_digest",
                ]
                || value["schema"] != TERMINAL_PATCH_RECEIPT_COMMITMENT_SCHEMA
                || value["receipt_digest"].as_str().is_none()
                || value["checkpoint_document_digest"]
                    != document_digest(CHECKPOINT_DOCUMENT_DOMAIN, checkpoint_document.as_bytes())
                || value["terminal_receipt_document_digest"]
                    != document_digest(
                        TERMINAL_PATCH_RECEIPT_DOCUMENT_DOMAIN,
                        receipt_document.as_bytes(),
                    )
            {
                return Err(CliError::refused(
                    "terminal patch receipt commitment is stale or mismatched",
                ));
            }
            Ok(Some(document))
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
            #[cfg(test)]
            if take_handoff_claim_fault(HandoffClaimFault::BeforeWrite) {
                return Err(CliError::refused("injected handoff claim write loss"));
            }
            file.write_all(expected.as_bytes())
                .and_then(|_| {
                    #[cfg(test)]
                    if take_handoff_claim_fault(HandoffClaimFault::BeforeSync) {
                        return Err(std::io::Error::other("injected handoff claim sync loss"));
                    }
                    Ok(())
                })
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
pub(super) use platform::{
    commit_fault_pending, inject_commit_fault, inject_handoff_claim_fault, CommitFault,
    HandoffClaimFault,
};
