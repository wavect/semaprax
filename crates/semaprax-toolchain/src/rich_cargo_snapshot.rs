//! Bounded pre-spawn replay of the concrete rich Cargo build inputs.
//!
//! This compares a prepared source tree and direct tool images at admission and
//! immediately before build dispatch. It is not a sandbox or a race-free file
//! handle protocol: the embedding host still owns the workspace and tools.

use crate::rich_cargo_execution::{CargoExecutionError, ExplicitCargoInvocation};
use crate::rich_cargo_preparation::PreparedCargoClosure;
use sha2::{Digest, Sha256};
use std::fs::{self, File, Metadata};
use std::io::Read;
use std::path::{Path, PathBuf};

const SNAPSHOT_DOMAIN: &[u8] = b"semaprax.rich-cargo-build-inputs.v1\0";
const MAX_SNAPSHOT_FILES: usize = 4_096;
const MAX_SNAPSHOT_BYTES: u64 = 128 * 1024 * 1024;

pub fn prepared_build_identity(
    invocation: &ExplicitCargoInvocation,
    prepared: &PreparedCargoClosure,
) -> Result<[u8; 32], CargoExecutionError> {
    let mut state = Snapshot {
        hash: Sha256::new(),
        files: 0,
        bytes: 0,
    };
    state.hash.update(SNAPSHOT_DOMAIN);
    state.bytes("prepared", prepared.bytes())?;
    for (label, path) in [
        ("workspace-root", invocation.workspace.as_path()),
        ("cargo-home-root", invocation.cargo_home.as_path()),
        ("target-root", invocation.target_dir.as_path()),
    ] {
        state.bytes(
            label,
            path.to_str()
                .ok_or(CargoExecutionError::BuildInputsChanged)?
                .as_bytes(),
        )?;
    }
    for path in &invocation.execution_path {
        state.bytes(
            "execution-path",
            path.to_str()
                .ok_or(CargoExecutionError::BuildInputsChanged)?
                .as_bytes(),
        )?;
    }
    for (label, path) in [
        ("cargo", invocation.cargo.as_path()),
        ("rustc", invocation.rustc.as_path()),
    ] {
        state.file(label, path)?;
    }
    // A lock is required for the advertised --locked build. The workspace
    // inventory also includes Cargo.toml, .cargo/config.toml when present,
    // build.rs, proc-macro sources, and vendored crate files.
    if !invocation.workspace.join("Cargo.lock").is_file() {
        return Err(CargoExecutionError::BuildInputsChanged);
    }
    state.tree("workspace", &invocation.workspace, invocation)?;
    state.tree("cargo-home", &invocation.cargo_home, invocation)?;
    Ok(state.hash.finalize().into())
}

struct Snapshot {
    hash: Sha256,
    files: usize,
    bytes: u64,
}

impl Snapshot {
    fn bytes(&mut self, label: &str, bytes: &[u8]) -> Result<(), CargoExecutionError> {
        self.bytes = self
            .bytes
            .checked_add(bytes.len() as u64)
            .ok_or(CargoExecutionError::BuildInputsChanged)?;
        if self.bytes > MAX_SNAPSHOT_BYTES {
            return Err(CargoExecutionError::BuildInputsChanged);
        }
        frame(&mut self.hash, label.as_bytes());
        frame(&mut self.hash, bytes);
        Ok(())
    }

    fn file(&mut self, label: &str, path: &Path) -> Result<(), CargoExecutionError> {
        let metadata =
            fs::symlink_metadata(path).map_err(|_| CargoExecutionError::BuildInputsChanged)?;
        if !path.is_absolute() || !metadata.file_type().is_file() {
            return Err(CargoExecutionError::BuildInputsChanged);
        }
        self.files = self
            .files
            .checked_add(1)
            .ok_or(CargoExecutionError::BuildInputsChanged)?;
        if self.files > MAX_SNAPSHOT_FILES || metadata.len() > MAX_SNAPSHOT_BYTES - self.bytes {
            return Err(CargoExecutionError::BuildInputsChanged);
        }
        let path_text = path
            .to_str()
            .ok_or(CargoExecutionError::BuildInputsChanged)?;
        let mut held = File::open(path).map_err(|_| CargoExecutionError::BuildInputsChanged)?;
        if !same_metadata(
            &metadata,
            &held
                .metadata()
                .map_err(|_| CargoExecutionError::BuildInputsChanged)?,
        ) {
            return Err(CargoExecutionError::BuildInputsChanged);
        }
        let mut bytes = Vec::with_capacity(metadata.len() as usize);
        held.read_to_end(&mut bytes)
            .map_err(|_| CargoExecutionError::BuildInputsChanged)?;
        if bytes.len() as u64 != metadata.len()
            || !same_metadata(
                &metadata,
                &held
                    .metadata()
                    .map_err(|_| CargoExecutionError::BuildInputsChanged)?,
            )
            || !same_metadata(
                &metadata,
                &fs::symlink_metadata(path).map_err(|_| CargoExecutionError::BuildInputsChanged)?,
            )
        {
            return Err(CargoExecutionError::BuildInputsChanged);
        }
        self.bytes(path_text, &bytes)?;
        frame(&mut self.hash, label.as_bytes());
        Ok(())
    }

    fn tree(
        &mut self,
        label: &str,
        root: &Path,
        invocation: &ExplicitCargoInvocation,
    ) -> Result<(), CargoExecutionError> {
        let mut pending = vec![(root.to_owned(), PathBuf::new())];
        while let Some((directory, relative)) = pending.pop() {
            self.files = self
                .files
                .checked_add(1)
                .ok_or(CargoExecutionError::BuildInputsChanged)?;
            if self.files > MAX_SNAPSHOT_FILES {
                return Err(CargoExecutionError::BuildInputsChanged);
            }
            let metadata = fs::symlink_metadata(&directory)
                .map_err(|_| CargoExecutionError::BuildInputsChanged)?;
            if !metadata.file_type().is_dir() {
                return Err(CargoExecutionError::BuildInputsChanged);
            }
            let relative_text = relative
                .to_str()
                .ok_or(CargoExecutionError::BuildInputsChanged)?;
            frame(&mut self.hash, label.as_bytes());
            frame(&mut self.hash, relative_text.as_bytes());
            let mut entries = fs::read_dir(&directory)
                .map_err(|_| CargoExecutionError::BuildInputsChanged)?
                .collect::<Result<Vec<_>, _>>()
                .map_err(|_| CargoExecutionError::BuildInputsChanged)?;
            entries.sort_by_key(|entry| entry.file_name());
            for entry in entries.into_iter().rev() {
                let path = entry.path();
                if label == "workspace"
                    && (path == invocation.target_dir || path == invocation.cargo_home)
                {
                    continue;
                }
                let name = entry.file_name();
                let name = name
                    .to_str()
                    .ok_or(CargoExecutionError::BuildInputsChanged)?;
                if name.bytes().any(|byte| byte.is_ascii_control()) {
                    return Err(CargoExecutionError::BuildInputsChanged);
                }
                let next = relative.join(name);
                let kind = entry
                    .file_type()
                    .map_err(|_| CargoExecutionError::BuildInputsChanged)?;
                if kind.is_dir() {
                    pending.push((path, next));
                } else if kind.is_file() {
                    let next_text = next
                        .to_str()
                        .ok_or(CargoExecutionError::BuildInputsChanged)?;
                    self.file(&format!("{label}/{next_text}"), &path)?;
                } else {
                    return Err(CargoExecutionError::BuildInputsChanged);
                }
            }
            if !same_metadata(
                &metadata,
                &fs::symlink_metadata(&directory)
                    .map_err(|_| CargoExecutionError::BuildInputsChanged)?,
            ) {
                return Err(CargoExecutionError::BuildInputsChanged);
            }
        }
        Ok(())
    }
}

fn same_metadata(left: &Metadata, right: &Metadata) -> bool {
    if left.file_type().is_file() != right.file_type().is_file()
        || left.file_type().is_dir() != right.file_type().is_dir()
        || left.len() != right.len()
        || left.modified().ok() != right.modified().ok()
    {
        return false;
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        left.dev() == right.dev()
            && left.ino() == right.ino()
            && left.mtime() == right.mtime()
            && left.mtime_nsec() == right.mtime_nsec()
            && left.ctime() == right.ctime()
            && left.ctime_nsec() == right.ctime_nsec()
    }
    #[cfg(not(unix))]
    {
        left.created().ok() == right.created().ok()
    }
}

fn frame(hash: &mut Sha256, bytes: &[u8]) {
    hash.update((bytes.len() as u64).to_be_bytes());
    hash.update(bytes);
}
