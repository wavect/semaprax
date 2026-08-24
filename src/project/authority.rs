use std::fs::{File, Metadata};
use std::io::{Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};

use same_file::Handle;
use sha2::{Digest, Sha256};

use crate::diagnostic::Diagnostic;

use super::{capacity, grammar, MAX_HELD_DIRECTORIES};

pub(super) struct HeldFile {
    path: PathBuf,
    file: File,
    pub(super) identity: Handle,
    permissions: PermissionFingerprint,
    limit: usize,
    expected_len: usize,
    expected_sha256: [u8; 32],
    bytes: Vec<u8>,
}

impl HeldFile {
    pub(super) fn open(path: PathBuf, limit: usize) -> Result<Self, Vec<Diagnostic>> {
        let before = std::fs::symlink_metadata(&path).map_err(|error| {
            authentication(format!("cannot inspect {}: {error}", path.display()))
        })?;
        if !plain_regular(&before) {
            return Err(authentication(format!(
                "Project v1 input {} must be a regular non-symlink file",
                path.display()
            )));
        }
        if !single_link(&path, &before) {
            return Err(authentication(format!(
                "Project v1 input {} must have exactly one hard link",
                path.display()
            )));
        }
        if before.len() > limit as u64 {
            return Err(capacity("input_bytes", limit));
        }
        let mut file = File::open(&path)
            .map_err(|error| authentication(format!("cannot open {}: {error}", path.display())))?;
        let identity = Handle::from_file(file.try_clone().map_err(|error| {
            authentication(format!("cannot retain {}: {error}", path.display()))
        })?)
        .map_err(|error| authentication(format!("cannot identify {}: {error}", path.display())))?;
        let after = std::fs::symlink_metadata(&path).map_err(|error| {
            authentication(format!("cannot recheck {}: {error}", path.display()))
        })?;
        if !plain_regular(&after)
            || !single_link(&path, &after)
            || Handle::from_path(&path).map_err(|error| {
                authentication(format!("cannot identify {}: {error}", path.display()))
            })? != identity
        {
            return Err(authentication(format!(
                "Project v1 input {} changed while opening",
                path.display()
            )));
        }
        let mut bytes = Vec::new();
        file.by_ref()
            .take((limit as u64) + 1)
            .read_to_end(&mut bytes)
            .map_err(|error| authentication(format!("cannot read {}: {error}", path.display())))?;
        if bytes.len() > limit {
            return Err(capacity("input_bytes", limit));
        }
        let expected_sha256 = Sha256::digest(&bytes).into();
        let expected_len = bytes.len();
        Ok(Self {
            path,
            file,
            identity,
            permissions: PermissionFingerprint::from_metadata(&after),
            limit,
            expected_len,
            expected_sha256,
            bytes,
        })
    }

    pub(super) fn utf8(&mut self) -> Result<String, Vec<Diagnostic>> {
        String::from_utf8(std::mem::take(&mut self.bytes))
            .map_err(|_| authentication("Project v1 input is not UTF-8"))
    }

    pub(super) fn recheck(&mut self) -> Result<(), Vec<Diagnostic>> {
        let metadata = std::fs::symlink_metadata(&self.path).map_err(|error| {
            authentication(format!("cannot recheck {}: {error}", self.path.display()))
        })?;
        if !plain_regular(&metadata)
            || !single_link(&self.path, &metadata)
            || PermissionFingerprint::from_metadata(&metadata) != self.permissions
            || Handle::from_path(&self.path).map_err(|error| {
                authentication(format!("cannot identify {}: {error}", self.path.display()))
            })? != self.identity
        {
            return Err(authentication(format!(
                "Project v1 input {} identity or permissions changed",
                self.path.display()
            )));
        }
        self.file
            .seek(SeekFrom::Start(0))
            .map_err(|error| authentication(format!("cannot seek held input: {error}")))?;
        let mut observed = Vec::new();
        self.file
            .by_ref()
            .take((self.limit as u64) + 1)
            .read_to_end(&mut observed)
            .map_err(|error| authentication(format!("cannot reread held input: {error}")))?;
        if observed.len() > self.limit {
            return Err(authentication(format!(
                "Project v1 input {} grew beyond its authenticated bound",
                self.path.display()
            )));
        }
        if observed.len() != self.expected_len
            || <[u8; 32]>::from(Sha256::digest(&observed)) != self.expected_sha256
        {
            return Err(authentication(format!(
                "Project v1 input {} bytes changed",
                self.path.display()
            )));
        }
        Ok(())
    }
}

pub(super) struct DeclaredPathSelection {
    declared_path: PathBuf,
    pub(super) canonical_path: PathBuf,
    pub(super) identity: Handle,
    parents: Vec<HeldDirectory>,
}

impl DeclaredPathSelection {
    pub(super) fn open(path: &Path, subject: &str) -> Result<Self, Vec<Diagnostic>> {
        let declared_path = declared_absolute_path(path, subject)?;
        let parent = declared_path.parent().ok_or_else(|| {
            grammar(format!(
                "Project v1 {subject} path must have an explicit parent"
            ))
        })?;
        let mut parent_paths = parent
            .ancestors()
            .map(Path::to_path_buf)
            .collect::<Vec<_>>();
        if parent_paths.len() > MAX_HELD_DIRECTORIES {
            return Err(capacity(
                "declared_parent_directories",
                MAX_HELD_DIRECTORIES,
            ));
        }
        parent_paths.reverse();
        let parents = parent_paths
            .into_iter()
            .map(HeldDirectory::open)
            .collect::<Result<Vec<_>, _>>()?;
        let metadata = std::fs::symlink_metadata(&declared_path).map_err(|error| {
            authentication(format!(
                "cannot inspect declared Project v1 {subject} {}: {error}",
                declared_path.display()
            ))
        })?;
        if !plain_regular(&metadata) || !single_link(&declared_path, &metadata) {
            return Err(authentication(format!(
                "declared Project v1 {subject} {} must select one regular non-link file",
                declared_path.display()
            )));
        }
        let identity = Handle::from_path(&declared_path).map_err(|error| {
            authentication(format!(
                "cannot identify declared Project v1 {subject} {}: {error}",
                declared_path.display()
            ))
        })?;
        let canonical_path = std::fs::canonicalize(&declared_path).map_err(|error| {
            authentication(format!(
                "cannot canonicalize Project v1 {subject} {}: {error}",
                declared_path.display()
            ))
        })?;
        #[cfg(not(windows))]
        if canonical_path != declared_path {
            return Err(authentication(format!(
                "Project v1 {subject} path {} is not canonically spelled",
                path.display()
            )));
        }
        Ok(Self {
            declared_path,
            canonical_path,
            identity,
            parents,
        })
    }

    pub(super) fn recheck(&self) -> Result<(), Vec<Diagnostic>> {
        for parent in &self.parents {
            parent.recheck()?;
        }
        let metadata = std::fs::symlink_metadata(&self.declared_path).map_err(|error| {
            authentication(format!(
                "cannot recheck declared input {}: {error}",
                self.declared_path.display()
            ))
        })?;
        if !plain_regular(&metadata)
            || !single_link(&self.declared_path, &metadata)
            || Handle::from_path(&self.declared_path).map_err(|error| {
                authentication(format!(
                    "cannot identify declared input {}: {error}",
                    self.declared_path.display()
                ))
            })? != self.identity
        {
            return Err(authentication(format!(
                "Project v1 declared input {} selection changed",
                self.declared_path.display()
            )));
        }
        Ok(())
    }
}

pub(super) fn declared_absolute_path(
    path: &Path,
    subject: &str,
) -> Result<PathBuf, Vec<Diagnostic>> {
    if has_declared_alias_component(path) {
        return Err(grammar(format!(
            "Project v1 {subject} path must not contain `.` or `..` components"
        )));
    }
    Ok(if path.is_absolute() {
        path.to_path_buf()
    } else {
        std::env::current_dir()
            .map_err(|error| authentication(format!("cannot inspect current directory: {error}")))?
            .join(path)
    })
}

#[cfg(windows)]
pub(super) fn has_declared_alias_component(path: &Path) -> bool {
    use std::os::windows::ffi::OsStrExt;

    let mut units = path.as_os_str().encode_wide();
    let first = units.next();
    let second = units.next();
    let raw_alias = match (first, second) {
        (Some(drive), Some(colon))
            if ((u16::from(b'A')..=u16::from(b'Z')).contains(&drive)
                || (u16::from(b'a')..=u16::from(b'z')).contains(&drive))
                && colon == u16::from(b':') =>
        {
            windows_units_have_alias_component(units)
        }
        (first, second) => {
            windows_units_have_alias_component(first.into_iter().chain(second).chain(units))
        }
    };
    raw_alias
        || path.components().any(|component| {
            matches!(
                component,
                std::path::Component::CurDir | std::path::Component::ParentDir
            )
        })
}

#[cfg(windows)]
fn windows_units_have_alias_component(units: impl Iterator<Item = u16>) -> bool {
    let mut segment_len = 0usize;
    let mut segment_is_dots = true;
    for unit in units.chain(std::iter::once(u16::from(b'/'))) {
        if unit == u16::from(b'/') || unit == u16::from(b'\\') {
            if segment_is_dots && matches!(segment_len, 1 | 2) {
                return true;
            }
            segment_len = 0;
            segment_is_dots = true;
        } else {
            segment_len += 1;
            segment_is_dots &= unit == u16::from(b'.');
        }
    }
    false
}

#[cfg(not(windows))]
pub(super) fn has_declared_alias_component(path: &Path) -> bool {
    path.components().any(|component| {
        matches!(
            component,
            std::path::Component::CurDir | std::path::Component::ParentDir
        )
    })
}

pub(super) struct HeldDirectory {
    path: PathBuf,
    identity: Handle,
    permissions: PermissionFingerprint,
}

impl HeldDirectory {
    pub(super) fn open(path: PathBuf) -> Result<Self, Vec<Diagnostic>> {
        let metadata = std::fs::symlink_metadata(&path).map_err(|error| {
            authentication(format!(
                "cannot inspect directory {}: {error}",
                path.display()
            ))
        })?;
        if !plain_directory(&metadata) {
            return Err(authentication(format!(
                "Project v1 ancestor {} must be a real directory",
                path.display()
            )));
        }
        let identity = Handle::from_path(&path).map_err(|error| {
            authentication(format!(
                "cannot identify directory {}: {error}",
                path.display()
            ))
        })?;
        Ok(Self {
            path,
            identity,
            permissions: PermissionFingerprint::from_metadata(&metadata),
        })
    }

    pub(super) fn recheck(&self) -> Result<(), Vec<Diagnostic>> {
        let metadata = std::fs::symlink_metadata(&self.path).map_err(|error| {
            authentication(format!(
                "cannot recheck directory {}: {error}",
                self.path.display()
            ))
        })?;
        if !plain_directory(&metadata)
            || PermissionFingerprint::from_metadata(&metadata) != self.permissions
            || Handle::from_path(&self.path).map_err(|error| {
                authentication(format!(
                    "cannot identify directory {}: {error}",
                    self.path.display()
                ))
            })? != self.identity
        {
            return Err(authentication(format!(
                "Project v1 ancestor {} changed",
                self.path.display()
            )));
        }
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct PermissionFingerprint {
    readonly: bool,
    #[cfg(unix)]
    mode: u32,
}

impl PermissionFingerprint {
    fn from_metadata(metadata: &Metadata) -> Self {
        #[cfg(unix)]
        use std::os::unix::fs::PermissionsExt;

        Self {
            readonly: metadata.permissions().readonly(),
            #[cfg(unix)]
            mode: metadata.permissions().mode(),
        }
    }
}

fn plain_regular(metadata: &Metadata) -> bool {
    metadata.is_file() && !metadata.file_type().is_symlink() && !metadata_is_reparse(metadata)
}

fn plain_directory(metadata: &Metadata) -> bool {
    metadata.is_dir() && !metadata.file_type().is_symlink() && !metadata_is_reparse(metadata)
}

#[cfg(unix)]
fn single_link(_: &Path, metadata: &Metadata) -> bool {
    use std::os::unix::fs::MetadataExt;
    metadata.nlink() == 1
}

#[cfg(windows)]
fn single_link(path: &Path, _: &Metadata) -> bool {
    winapi_util::Handle::from_path_any(path)
        .and_then(winapi_util::file::information)
        .is_ok_and(|information| information.number_of_links() == 1)
}

#[cfg(not(any(unix, windows)))]
fn single_link(_: &Path, _: &Metadata) -> bool {
    true
}

#[cfg(windows)]
fn metadata_is_reparse(metadata: &Metadata) -> bool {
    use std::os::windows::fs::MetadataExt;
    metadata.file_attributes() & 0x400 != 0
}

#[cfg(not(windows))]
fn metadata_is_reparse(_: &Metadata) -> bool {
    false
}

pub(super) fn authentication(message: impl Into<String>) -> Vec<Diagnostic> {
    vec![Diagnostic::io("SPX-J102", message)]
}
