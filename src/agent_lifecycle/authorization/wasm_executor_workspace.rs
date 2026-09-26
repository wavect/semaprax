//! Descriptor-held private workspace for one Core Wasm stage invocation.
//!
//! Creation is exclusive and mode 0700. Files are exclusive, no-follow, and
//! inventoried by stable identity plus exact bytes. Cleanup removes only that
//! inventory; foreign or replaced entries make cleanup fail closed.

use std::fs::{self, File, OpenOptions};
use std::io::{Read, Write};
use std::path::{Component, Path, PathBuf};

use sha2::{Digest, Sha256};

use crate::agent_lifecycle::stages::invariant;
use crate::diagnostic::Diagnostic;

#[derive(Debug)]
struct FileRecord {
    name: PathBuf,
    digest: [u8; 32],
    len: u64,
    #[cfg(unix)]
    device: u64,
    #[cfg(unix)]
    inode: u64,
}

/// One private directory whose descriptor, identity, and exact file inventory
/// remain held until explicit cleanup.
#[derive(Debug)]
pub(super) struct WasmStageWorkspace {
    path: PathBuf,
    held: File,
    files: Vec<FileRecord>,
    #[cfg(unix)]
    device: u64,
    #[cfg(unix)]
    inode: u64,
}

impl WasmStageWorkspace {
    pub(super) fn create() -> Result<Self, Diagnostic> {
        let mut entropy = [0_u8; 16];
        getrandom::fill(&mut entropy).map_err(|_| invariant("wasm_executor.workspace.entropy"))?;
        let nonce = entropy
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect::<String>();
        let path = std::env::temp_dir().join(format!("semaprax-wasm-stage-{nonce}"));
        fs::create_dir(&path).map_err(|_| invariant("wasm_executor.workspace.create"))?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            fs::set_permissions(&path, fs::Permissions::from_mode(0o700))
                .map_err(|_| invariant("wasm_executor.workspace.permissions"))?;
        }
        let held = open_directory(&path)?;
        let metadata = held
            .metadata()
            .map_err(|_| invariant("wasm_executor.workspace.metadata"))?;
        Ok(Self {
            path,
            held,
            files: Vec::new(),
            #[cfg(unix)]
            device: {
                use std::os::unix::fs::MetadataExt;
                metadata.dev()
            },
            #[cfg(unix)]
            inode: {
                use std::os::unix::fs::MetadataExt;
                metadata.ino()
            },
        })
    }

    pub(super) fn path(&self) -> &Path {
        &self.path
    }

    pub(super) fn held_directory(&self) -> Result<File, Diagnostic> {
        self.recheck()?;
        self.held
            .try_clone()
            .map_err(|_| invariant("wasm_executor.workspace.clone"))
    }

    /// Create one direct child. NPM stage artifacts have a closed, flat
    /// inventory; rejecting separators prevents an artifact name from
    /// selecting a second directory authority.
    pub(super) fn write(&mut self, name: &Path, bytes: &[u8]) -> Result<(), Diagnostic> {
        self.recheck()?;
        let mut components = name.components();
        let Component::Normal(_) = components
            .next()
            .ok_or_else(|| invariant("wasm_executor.workspace.name"))?
        else {
            return Err(invariant("wasm_executor.workspace.name"));
        };
        if components.next().is_some() {
            return Err(invariant("wasm_executor.workspace.name"));
        }
        let path = self.path.join(name);
        let mut options = OpenOptions::new();
        options.write(true).create_new(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options
                .mode(0o600)
                .custom_flags(libc::O_NOFOLLOW | libc::O_CLOEXEC);
        }
        let mut file = options
            .open(&path)
            .map_err(|_| invariant("wasm_executor.workspace.write"))?;
        file.write_all(bytes)
            .map_err(|_| invariant("wasm_executor.workspace.write"))?;
        file.sync_all()
            .map_err(|_| invariant("wasm_executor.workspace.write"))?;
        let metadata = file
            .metadata()
            .map_err(|_| invariant("wasm_executor.workspace.metadata"))?;
        self.files.push(FileRecord {
            name: name.to_path_buf(),
            digest: Sha256::digest(bytes).into(),
            len: metadata.len(),
            #[cfg(unix)]
            device: {
                use std::os::unix::fs::MetadataExt;
                metadata.dev()
            },
            #[cfg(unix)]
            inode: {
                use std::os::unix::fs::MetadataExt;
                metadata.ino()
            },
        });
        Ok(())
    }

    pub(super) fn recheck(&self) -> Result<(), Diagnostic> {
        let metadata = self
            .held
            .metadata()
            .map_err(|_| invariant("wasm_executor.workspace.metadata"))?;
        if !metadata.is_dir() {
            return Err(invariant("wasm_executor.workspace.changed"));
        }
        #[cfg(unix)]
        {
            use std::os::unix::fs::{MetadataExt, PermissionsExt};
            if metadata.dev() != self.device
                || metadata.ino() != self.inode
                || metadata.permissions().mode() & 0o777 != 0o700
            {
                return Err(invariant("wasm_executor.workspace.changed"));
            }
            let path = fs::symlink_metadata(&self.path)
                .map_err(|_| invariant("wasm_executor.workspace.changed"))?;
            if !path.is_dir() || path.dev() != self.device || path.ino() != self.inode {
                return Err(invariant("wasm_executor.workspace.changed"));
            }
        }
        Ok(())
    }

    /// Remove exactly the unchanged files created through this value, then
    /// the still-identical directory. Nothing happens implicitly on Drop.
    pub(super) fn cleanup(mut self) -> Result<(), Diagnostic> {
        self.recheck()?;
        for record in self.files.drain(..).rev() {
            let path = self.path.join(&record.name);
            let mut file = OpenOptions::new()
                .read(true)
                .open(&path)
                .map_err(|_| invariant("wasm_executor.workspace.cleanup_changed"))?;
            let metadata = file
                .metadata()
                .map_err(|_| invariant("wasm_executor.workspace.cleanup_changed"))?;
            #[cfg(unix)]
            {
                use std::os::unix::fs::MetadataExt;
                if metadata.dev() != record.device || metadata.ino() != record.inode {
                    return Err(invariant("wasm_executor.workspace.cleanup_changed"));
                }
            }
            if metadata.len() != record.len || digest_reader(&mut file)? != record.digest {
                return Err(invariant("wasm_executor.workspace.cleanup_changed"));
            }
            fs::remove_file(path).map_err(|_| invariant("wasm_executor.workspace.cleanup"))?;
        }
        self.recheck()?;
        fs::remove_dir(&self.path).map_err(|_| invariant("wasm_executor.workspace.cleanup"))
    }
}

fn open_directory(path: &Path) -> Result<File, Diagnostic> {
    let mut options = OpenOptions::new();
    options.read(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.custom_flags(libc::O_DIRECTORY | libc::O_NOFOLLOW | libc::O_CLOEXEC);
    }
    #[cfg(windows)]
    {
        use std::os::windows::fs::OpenOptionsExt;
        // Windows requires BACKUP_SEMANTICS to hold a directory handle. Open
        // the reparse point itself so a swapped link cannot become authority.
        const FILE_FLAG_BACKUP_SEMANTICS: u32 = 0x0200_0000;
        const FILE_FLAG_OPEN_REPARSE_POINT: u32 = 0x0020_0000;
        options.custom_flags(FILE_FLAG_BACKUP_SEMANTICS | FILE_FLAG_OPEN_REPARSE_POINT);
    }
    options
        .open(path)
        .map_err(|_| invariant("wasm_executor.workspace.open"))
}

#[cfg(all(test, windows))]
mod windows_tests {
    use super::*;

    #[test]
    fn private_workspace_directory_can_be_held_on_windows() {
        let path = std::env::temp_dir().join(format!(
            "semaprax-wasm-stage-directory-handle-{}",
            std::process::id()
        ));
        fs::create_dir(&path).unwrap();
        let held = open_directory(&path).unwrap();
        assert!(held.metadata().unwrap().is_dir());
        drop(held);
        fs::remove_dir(path).unwrap();
    }
}

fn digest_reader(file: &mut File) -> Result<[u8; 32], Diagnostic> {
    let mut hash = Sha256::new();
    let mut bytes = [0_u8; 8 * 1024];
    loop {
        let read = file
            .read(&mut bytes)
            .map_err(|_| invariant("wasm_executor.workspace.cleanup_changed"))?;
        if read == 0 {
            break;
        }
        hash.update(&bytes[..read]);
    }
    Ok(hash.finalize().into())
}
