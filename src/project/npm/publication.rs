//! No-clobber npm package publication.
//!
//! Unix publication resolves the parent once, then performs every effect
//! relative to held directory descriptors with `O_NOFOLLOW`. The new data
//! profile remains unavailable on Windows until the public crate has an
//! equally strong safe handle-relative primitive.

use std::path::Path;
#[cfg(unix)]
use std::path::{Component, PathBuf};

use crate::diagnostic::Diagnostic;

use super::{package_error, NpmArtifact};
#[cfg(windows)]
use super::{
    PROJECT_NPM_BUILD_SCHEMA_V2, PROJECT_NPM_BUILD_SCHEMA_V3, PROJECT_NPM_BUILD_SCHEMA_V4,
};

#[cfg(test)]
type TestHook = Box<dyn FnOnce() + Send + 'static>;
#[cfg(test)]
static TEST_AFTER_CREATE: std::sync::Mutex<Option<TestHook>> = std::sync::Mutex::new(None);

#[cfg(test)]
pub(super) fn set_test_after_create(hook: TestHook) {
    *TEST_AFTER_CREATE.lock().expect("publication hook lock") = Some(hook);
}

#[cfg(test)]
fn run_test_after_create() {
    if let Some(hook) = TEST_AFTER_CREATE
        .lock()
        .expect("publication hook lock")
        .take()
    {
        hook();
    }
}

#[cfg(not(test))]
fn run_test_after_create() {}

#[cfg(unix)]
pub(super) fn publish(
    output: &Path,
    artifacts: &[NpmArtifact],
    _schema: &str,
) -> Result<(), Diagnostic> {
    unix::publish(output, artifacts)
}

#[cfg(windows)]
pub(super) fn publish(
    output: &Path,
    artifacts: &[NpmArtifact],
    schema: &str,
) -> Result<(), Diagnostic> {
    if matches!(
        schema,
        PROJECT_NPM_BUILD_SCHEMA_V2 | PROJECT_NPM_BUILD_SCHEMA_V3 | PROJECT_NPM_BUILD_SCHEMA_V4
    ) {
        return Err(package_error(
            "useful-data npm publication requires safe handle-relative Windows authority",
        ));
    }
    legacy_windows_publish(output, artifacts)
}

#[cfg(windows)]
fn legacy_windows_publish(output: &Path, artifacts: &[NpmArtifact]) -> Result<(), Diagnostic> {
    use std::io::Write;

    match std::fs::symlink_metadata(output) {
        Ok(_) => return Err(package_error("npm package destination already exists")),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
        Err(error) => return Err(package_error(format!("cannot inspect npm output: {error}"))),
    }
    std::fs::create_dir(output)
        .map_err(|error| package_error(format!("cannot create npm output: {error}")))?;
    run_test_after_create();
    for artifact in artifacts {
        let path = output.join(artifact.path);
        let mut file = std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&path)
            .map_err(|error| package_error(format!("cannot create npm artifact: {error}")))?;
        file.write_all(&artifact.bytes)
            .and_then(|()| file.sync_all())
            .map_err(|error| package_error(format!("cannot settle npm artifact: {error}")))?;
    }
    Ok(())
}

#[cfg(unix)]
mod unix {
    use std::ffi::CStr;
    use std::io::{Read, Seek, SeekFrom, Write};
    use std::os::fd::OwnedFd;
    use std::os::unix::ffi::OsStrExt;

    use rustix::fs::{self, AtFlags, Dir, Mode, OFlags, CWD};

    use super::*;

    pub(super) fn publish(output: &Path, artifacts: &[NpmArtifact]) -> Result<(), Diagnostic> {
        let absolute = absolute_normalized(output)?;
        let name = absolute
            .file_name()
            .ok_or_else(|| package_error("npm package output must name one directory"))?;
        let requested_parent = absolute
            .parent()
            .ok_or_else(|| package_error("npm package output has no parent directory"))?;
        // Pre-existing aliases are resolved before the first effect. Every
        // later operation is relative to this held canonical parent.
        let canonical_parent = std::fs::canonicalize(requested_parent).map_err(|error| {
            package_error(format!("cannot resolve npm package output parent: {error}"))
        })?;
        let parent = open_absolute_directory(&canonical_parent)?;
        fs::mkdirat(&parent, name.as_bytes(), Mode::from_raw_mode(0o700)).map_err(|error| {
            package_error(format!(
                "cannot create fresh npm package destination: {error}"
            ))
        })?;
        let destination = fs::openat(
            &parent,
            name.as_bytes(),
            OFlags::RDONLY | OFlags::DIRECTORY | OFlags::NOFOLLOW | OFlags::CLOEXEC,
            Mode::empty(),
        )
        .map_err(|error| package_error(format!("cannot hold npm package destination: {error}")))?;
        let destination_identity = identity(&destination)?;
        run_test_after_create();

        let mut file_identities = Vec::with_capacity(artifacts.len());
        for artifact in artifacts {
            validate_leaf(artifact.path)?;
            let fd = fs::openat(
                &destination,
                artifact.path,
                OFlags::RDWR | OFlags::CREATE | OFlags::EXCL | OFlags::NOFOLLOW | OFlags::CLOEXEC,
                Mode::from_raw_mode(0o600),
            )
            .map_err(|error| {
                package_error(format!("cannot create npm package artifact: {error}"))
            })?;
            let mut file = std::fs::File::from(fd);
            file.write_all(&artifact.bytes)
                .and_then(|()| file.sync_all())
                .map_err(|error| {
                    package_error(format!("cannot settle npm package artifact: {error}"))
                })?;
            file.seek(SeekFrom::Start(0)).map_err(|error| {
                package_error(format!("cannot authenticate npm artifact: {error}"))
            })?;
            let mut observed = Vec::with_capacity(artifact.bytes.len());
            file.read_to_end(&mut observed).map_err(|error| {
                package_error(format!("cannot authenticate npm artifact: {error}"))
            })?;
            if observed != artifact.bytes {
                return Err(package_error(
                    "npm package artifact bytes disagree after write",
                ));
            }
            let metadata = file.metadata().map_err(|error| {
                package_error(format!("cannot authenticate npm artifact: {error}"))
            })?;
            if !metadata.is_file() || metadata.len() != artifact.bytes.len() as u64 {
                return Err(package_error("npm package artifact identity is invalid"));
            }
            file_identities.push(identity(&file)?);
        }

        authenticate_inventory(&destination, artifacts, &file_identities)?;
        if identity(&destination)? != destination_identity
            || identity_at(&parent, name.as_bytes())? != destination_identity
        {
            return Err(package_error(
                "npm package destination identity changed during publication",
            ));
        }
        let rebound_parent = std::fs::canonicalize(requested_parent).map_err(|error| {
            package_error(format!("cannot rebind npm package output parent: {error}"))
        })?;
        if rebound_parent != canonical_parent {
            return Err(package_error(
                "npm package parent identity changed during publication",
            ));
        }
        Ok(())
    }

    fn open_absolute_directory(path: &Path) -> Result<OwnedFd, Diagnostic> {
        let mut current = fs::openat(
            CWD,
            "/",
            OFlags::RDONLY | OFlags::DIRECTORY | OFlags::NOFOLLOW | OFlags::CLOEXEC,
            Mode::empty(),
        )
        .map_err(|error| package_error(format!("cannot hold filesystem root: {error}")))?;
        for component in path.components() {
            match component {
                Component::RootDir => {}
                Component::Normal(name) => {
                    current = fs::openat(
                        &current,
                        name.as_bytes(),
                        OFlags::RDONLY | OFlags::DIRECTORY | OFlags::NOFOLLOW | OFlags::CLOEXEC,
                        Mode::empty(),
                    )
                    .map_err(|error| {
                        package_error(format!("cannot authenticate npm parent: {error}"))
                    })?;
                }
                _ => return Err(package_error("npm package parent path is not canonical")),
            }
        }
        Ok(current)
    }

    fn authenticate_inventory(
        directory: &OwnedFd,
        artifacts: &[NpmArtifact],
        identities: &[(u64, u64)],
    ) -> Result<(), Diagnostic> {
        let duplicate = rustix::io::dup(directory)
            .map_err(|error| package_error(format!("cannot hold npm inventory: {error}")))?;
        let entries = Dir::new(duplicate)
            .map_err(|error| package_error(format!("cannot inspect npm inventory: {error}")))?;
        let mut seen = vec![false; artifacts.len()];
        for entry in entries {
            let entry = entry
                .map_err(|error| package_error(format!("cannot inspect npm inventory: {error}")))?;
            let name = entry.file_name();
            if name == c"." || name == c".." {
                continue;
            }
            let index = artifacts
                .iter()
                .position(|artifact| cstr_eq(name, artifact.path.as_bytes()))
                .ok_or_else(|| {
                    package_error("npm package inventory contains an unexpected entry")
                })?;
            if seen[index] || identity_at(directory, artifacts[index].path)? != identities[index] {
                return Err(package_error("npm package inventory identity changed"));
            }
            seen[index] = true;
        }
        if seen.iter().any(|seen| !seen) {
            return Err(package_error("npm package artifact inventory is not exact"));
        }
        Ok(())
    }

    fn cstr_eq(value: &CStr, expected: &[u8]) -> bool {
        value.to_bytes() == expected
    }

    fn identity<Fd: std::os::fd::AsFd>(fd: Fd) -> Result<(u64, u64), Diagnostic> {
        let stat = fs::fstat(fd).map_err(|error| {
            package_error(format!("cannot authenticate filesystem identity: {error}"))
        })?;
        Ok((stat.st_dev as u64, stat.st_ino as u64))
    }

    fn identity_at<Fd: std::os::fd::AsFd, P: rustix::path::Arg>(
        directory: Fd,
        name: P,
    ) -> Result<(u64, u64), Diagnostic> {
        let stat = fs::statat(directory, name, AtFlags::SYMLINK_NOFOLLOW).map_err(|error| {
            package_error(format!("cannot authenticate filesystem entry: {error}"))
        })?;
        Ok((stat.st_dev as u64, stat.st_ino as u64))
    }
}

#[cfg(unix)]
fn validate_leaf(path: &str) -> Result<(), Diagnostic> {
    let candidate = Path::new(path);
    if candidate.components().count() != 1
        || !matches!(candidate.components().next(), Some(Component::Normal(_)))
    {
        return Err(package_error("npm artifact path must be one ordinary leaf"));
    }
    Ok(())
}

#[cfg(unix)]
fn absolute_normalized(path: &Path) -> Result<PathBuf, Diagnostic> {
    let absolute = if path.is_absolute() {
        path.to_path_buf()
    } else {
        std::env::current_dir()
            .map_err(|error| {
                package_error(format!("cannot resolve npm output directory: {error}"))
            })?
            .join(path)
    };
    let mut normalized = PathBuf::new();
    for component in absolute.components() {
        match component {
            Component::Prefix(prefix) => normalized.push(prefix.as_os_str()),
            Component::RootDir => normalized.push(component.as_os_str()),
            Component::Normal(value) => normalized.push(value),
            Component::CurDir => {}
            Component::ParentDir => {
                return Err(package_error(
                    "npm package output may not contain parent traversal",
                ))
            }
        }
    }
    if normalized.file_name().is_none() {
        return Err(package_error("npm package output must name one directory"));
    }
    Ok(normalized)
}
