//! Held-root cursor publication for the semantic-retention registry.

use std::io::{Read, Seek, SeekFrom, Write};
use std::os::fd::{AsFd, OwnedFd};
use std::os::unix::ffi::OsStrExt;
use std::path::{Component, Path};

use rustix::fs::{self, AtFlags, Dir, FileType, Mode, OFlags, RenameFlags, Stat, CWD};

use super::{
    binding, capacity, io, post_pivot, stale, validate_stage_relationship, Result,
    MAX_RETENTION_REGISTRY_CURSOR_BYTES,
};

const CURRENT: &[u8] = b"CURRENT";
const STAGE: &[u8] = b".CURRENT-stage";
const METADATA: &[u8] = b"metadata";
const MAX_PATH_BYTES: usize = 4096;
const MAX_PATH_DEPTH: usize = 64;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct Identity {
    device: u64,
    inode: u64,
    mode: u32,
    uid: u32,
}

impl Identity {
    #[allow(
        clippy::unnecessary_cast,
        reason = "stat field widths vary across Unix ABIs"
    )]
    fn from_stat(stat: &Stat) -> Self {
        Self {
            device: stat.st_dev as u64,
            inode: stat.st_ino as u64,
            mode: stat.st_mode as u32,
            uid: stat.st_uid as u32,
        }
    }
}

pub(super) struct Root {
    chain: Vec<(OwnedFd, Identity)>,
    names: Vec<Vec<u8>>,
    metadata_fd: OwnedFd,
    metadata_identity: Identity,
}

impl Root {
    pub(super) fn open(path: &Path) -> Result<Self> {
        let raw = path.as_os_str().as_bytes();
        if !path.is_absolute() || raw.len() > MAX_PATH_BYTES {
            return Err(binding(
                "retention registry requires a bounded absolute normalized root",
            ));
        }
        let mut normalized = Vec::new();
        let mut names = Vec::new();
        for component in path.components() {
            match component {
                Component::RootDir => {}
                Component::Normal(name) => {
                    normalized.push(b'/');
                    normalized.extend_from_slice(name.as_bytes());
                    names.push(name.as_bytes().to_vec());
                }
                _ => return Err(binding("retention registry root is not normalized")),
            }
        }
        if names.is_empty() || normalized != raw || names.len() > MAX_PATH_DEPTH {
            return Err(binding(
                "retention registry root is not canonical or bounded",
            ));
        }
        let first = open_dir(CWD, b"/")?;
        let first_identity = identity(&first)?;
        let mut chain = vec![(first, first_identity)];
        for name in &names {
            let child = open_dir(&chain.last().expect("root held").0, name)?;
            let fact = identity(&child)?;
            chain.push((child, fact));
        }
        let metadata_fd = open_dir(&chain.last().expect("root held").0, METADATA)?;
        let metadata_identity = identity(&metadata_fd)?;
        let root = Self {
            chain,
            names,
            metadata_fd,
            metadata_identity,
        };
        root.validate()?;
        Ok(root)
    }

    fn fd(&self) -> &OwnedFd {
        &self.chain.last().expect("root held").0
    }

    pub(super) fn identity_key(&self) -> (u64, u64) {
        let identity = self.chain.last().expect("root held").1;
        (identity.device, identity.inode)
    }

    fn validate(&self) -> Result<()> {
        for (index, (held, expected)) in self.chain.iter().enumerate() {
            if identity(held)? != *expected {
                return Err(binding("retention registry held ancestor changed"));
            }
            if index > 0 {
                let fact = fs::statat(
                    &self.chain[index - 1].0,
                    self.names[index - 1].as_slice(),
                    AtFlags::SYMLINK_NOFOLLOW,
                )
                .map_err(|_| binding("retention registry ancestor disappeared"))?;
                if Identity::from_stat(&fact) != *expected {
                    return Err(binding("retention registry path and held root disagree"));
                }
            }
        }
        let root = self.chain.last().expect("root held").1;
        if root.uid != rustix::process::geteuid().as_raw() || root.mode & 0o7777 != 0o700 {
            return Err(binding(
                "retention registry root must be current-euid-owned exact 0700",
            ));
        }
        let metadata = fs::statat(self.fd(), METADATA, AtFlags::SYMLINK_NOFOLLOW)
            .map_err(|_| binding("retention registry metadata directory is absent"))?;
        if !FileType::from_raw_mode(metadata.st_mode).is_dir()
            || metadata.st_uid != rustix::process::geteuid().as_raw()
            || metadata.st_mode & 0o7777 != 0o700
        {
            return Err(binding(
                "retention registry metadata child must be current-euid-owned exact 0700",
            ));
        }
        if Identity::from_stat(&metadata) != self.metadata_identity
            || identity(&self.metadata_fd)? != self.metadata_identity
        {
            return Err(binding(
                "retention registry metadata path and held directory disagree",
            ));
        }
        inventory(self)?;
        Ok(())
    }

    fn lock(&self, exclusive: bool) -> Result<Lock> {
        let file = std::fs::File::from(
            rustix::io::dup(self.fd())
                .map_err(|_| io("cannot duplicate retention registry root"))?,
        );
        if exclusive {
            fs2::FileExt::try_lock_exclusive(&file)
        } else {
            fs2::FileExt::try_lock_shared(&file)
        }
        .map_err(|_| stale("retention registry root is busy"))?;
        Ok(Lock(file))
    }
}

struct Lock(std::fs::File);
impl Lock {
    fn release(self) -> Result<()> {
        fs2::FileExt::unlock(&self.0).map_err(|_| io("cannot release retention registry lock"))
    }
}
impl Drop for Lock {
    fn drop(&mut self) {
        let _ = fs2::FileExt::unlock(&self.0);
    }
}

pub(super) fn read_held<T>(
    root: &Root,
    operation: impl FnOnce(&[u8], &OwnedFd) -> Result<T>,
) -> Result<T> {
    root.validate()?;
    let lock = root.lock(false)?;
    let current =
        read_current(root)?.ok_or_else(|| stale("retention registry is not initialized"))?;
    validate_stage(root, Some(&current), false)?;
    let result = operation(&current, &root.metadata_fd)?;
    if read_current(root)?.as_deref() != Some(current.as_slice()) {
        return Err(stale("retention registry CURRENT changed during recovery"));
    }
    root.validate()?;
    lock.release()?;
    Ok(result)
}

#[cfg(test)]
pub(super) fn transaction<T>(
    root_path: &Path,
    operation: impl FnOnce(Option<&[u8]>, &OwnedFd) -> Result<(Vec<u8>, T)>,
) -> Result<T> {
    let root = Root::open(root_path)?;
    transaction_held(&root, operation)
}

pub(super) fn transaction_held<T>(
    root: &Root,
    operation: impl FnOnce(Option<&[u8]>, &OwnedFd) -> Result<(Vec<u8>, T)>,
) -> Result<T> {
    root.validate()?;
    let lock = root.lock(true)?;
    let current = read_current(root)?;
    validate_stage(root, current.as_deref(), true)?;
    if read_current(root)? != current {
        return Err(stale(
            "retention registry CURRENT changed during stage recovery",
        ));
    }
    let (next, result) = operation(current.as_deref(), &root.metadata_fd)?;
    if next.is_empty() || next.len() > MAX_RETENTION_REGISTRY_CURSOR_BYTES {
        return Err(capacity(
            "retention registry CURRENT bytes exceed their bound",
        ));
    }
    if read_current(root)? != current {
        return Err(stale("retention registry CURRENT changed before pivot"));
    }
    // The held metadata fd prevents nested store operations from being
    // redirected, but CURRENT must never select a pair stranded in a displaced
    // child. Rebind the child immediately before the cursor pivot.
    root.validate()?;
    publish(root, current.as_deref(), &next)?;
    let final_check = (|| -> Result<()> {
        fs::fsync(root.fd()).map_err(|_| io("cannot settle retention registry root"))?;
        if read_current(root)?.as_deref() != Some(next.as_slice()) {
            return Err(binding("retention registry CURRENT differs after pivot"));
        }
        root.validate()?;
        lock.release()?;
        Ok(())
    })();
    final_check.map_err(|_| {
        post_pivot(
            "retention registry CURRENT may have advanced; recover the explicit root before retry",
        )
    })?;
    Ok(result)
}

fn publish(root: &Root, previous: Option<&[u8]>, bytes: &[u8]) -> Result<()> {
    let fd = fs::openat(
        root.fd(),
        STAGE,
        OFlags::RDWR | OFlags::CREATE | OFlags::EXCL | OFlags::NOFOLLOW | OFlags::CLOEXEC,
        Mode::from_raw_mode(0o600),
    )
    .map_err(|_| io("cannot create retention registry CURRENT stage"))?;
    let mut file = std::fs::File::from(fd);
    file.write_all(bytes)
        .and_then(|()| file.sync_all())
        .map_err(|_| io("cannot settle retention registry CURRENT stage"))?;
    if read_file(root, STAGE)?.as_slice() != bytes {
        return Err(binding("retention registry CURRENT stage bytes disagree"));
    }
    fs::fsync(root.fd()).map_err(|_| io("cannot settle retention registry stage directory"))?;
    match previous {
        None => fs::renameat_with(root.fd(), STAGE, root.fd(), CURRENT, RenameFlags::NOREPLACE)
            .map_err(|_| io("cannot publish initial retention registry CURRENT"))?,
        Some(previous) => {
            fs::renameat_with(root.fd(), STAGE, root.fd(), CURRENT, RenameFlags::EXCHANGE)
                .map_err(|_| io("cannot atomically exchange retention registry CURRENT"))?;
            if read_file(root, STAGE)?.as_slice() != previous {
                return Err(post_pivot(
                    "retention registry CURRENT changed at the pivot boundary",
                ));
            }
            fs::unlinkat(root.fd(), STAGE, AtFlags::empty()).map_err(|_| {
                post_pivot(
                    "retention registry CURRENT advanced but old cursor cleanup is uncertain",
                )
            })?;
        }
    }
    Ok(())
}

fn inventory(root: &Root) -> Result<()> {
    let entries = Dir::new(open_dir(root.fd(), b".")?)
        .map_err(|_| io("cannot enumerate retention registry root"))?;
    let mut metadata = 0usize;
    let mut current = 0usize;
    let mut stage = 0usize;
    for entry in entries {
        let entry = entry.map_err(|_| io("cannot read retention registry inventory"))?;
        match entry.file_name().to_bytes() {
            b"." | b".." => {}
            METADATA => metadata += 1,
            CURRENT => current += 1,
            STAGE => stage += 1,
            _ => {
                return Err(binding(
                    "retention registry root contains an unexpected entry",
                ))
            }
        }
    }
    if metadata != 1 || current > 1 || stage > 1 {
        return Err(binding(
            "retention registry root inventory is not canonical",
        ));
    }
    Ok(())
}

fn validate_stage(root: &Root, current: Option<&[u8]>, remove: bool) -> Result<()> {
    match fs::statat(root.fd(), STAGE, AtFlags::SYMLINK_NOFOLLOW) {
        Ok(stat) => {
            if !FileType::from_raw_mode(stat.st_mode).is_file()
                || stat.st_nlink != 1
                || stat.st_uid != rustix::process::geteuid().as_raw()
                || stat.st_mode & 0o7777 != 0o600
                || stat.st_size <= 0
                || stat.st_size as usize > MAX_RETENTION_REGISTRY_CURSOR_BYTES
            {
                return Err(binding(
                    "interrupted retention registry CURRENT stage facts disagree",
                ));
            }
            let expected = Identity::from_stat(&stat);
            let stage = read_file(root, STAGE)?;
            validate_stage_relationship(&root.metadata_fd, current, &stage)?;
            if !remove {
                return Ok(());
            }
            let rebound = fs::statat(root.fd(), STAGE, AtFlags::SYMLINK_NOFOLLOW)
                .map_err(|_| binding("interrupted retention registry stage disappeared"))?;
            if !FileType::from_raw_mode(rebound.st_mode).is_file()
                || rebound.st_nlink != 1
                || rebound.st_uid != rustix::process::geteuid().as_raw()
                || rebound.st_mode & 0o7777 != 0o600
                || rebound.st_size != stat.st_size
                || Identity::from_stat(&rebound) != expected
            {
                return Err(binding(
                    "interrupted retention registry stage identity changed before cleanup",
                ));
            }
            fs::unlinkat(root.fd(), STAGE, AtFlags::empty()).map_err(|_| {
                io("cannot remove authenticated interrupted retention registry CURRENT stage")
            })?;
            fs::fsync(root.fd()).map_err(|_| {
                io("cannot settle authenticated interrupted retention registry stage cleanup")
            })?;
            root.validate()
        }
        Err(error) if error == rustix::io::Errno::NOENT => Ok(()),
        Err(_) => Err(io("cannot inspect retention registry CURRENT stage")),
    }
}

fn read_current(root: &Root) -> Result<Option<Vec<u8>>> {
    match fs::statat(root.fd(), CURRENT, AtFlags::SYMLINK_NOFOLLOW) {
        Ok(stat) => {
            if !FileType::from_raw_mode(stat.st_mode).is_file()
                || stat.st_nlink != 1
                || stat.st_uid != rustix::process::geteuid().as_raw()
                || stat.st_mode & 0o7777 != 0o600
                || stat.st_size <= 0
                || stat.st_size as usize > MAX_RETENTION_REGISTRY_CURSOR_BYTES
            {
                return Err(binding("retention registry CURRENT file facts disagree"));
            }
            Ok(Some(read_file(root, CURRENT)?))
        }
        Err(error) if error == rustix::io::Errno::NOENT => Ok(None),
        Err(_) => Err(io("cannot inspect retention registry CURRENT")),
    }
}

fn read_file(root: &Root, name: &[u8]) -> Result<Vec<u8>> {
    let path_stat = fs::statat(root.fd(), name, AtFlags::SYMLINK_NOFOLLOW)
        .map_err(|_| io("cannot inspect retention registry file path"))?;
    let expected = Identity::from_stat(&path_stat);
    let fd = fs::openat(
        root.fd(),
        name,
        OFlags::RDONLY | OFlags::NOFOLLOW | OFlags::CLOEXEC | OFlags::NONBLOCK,
        Mode::empty(),
    )
    .map_err(|_| io("cannot open retention registry file"))?;
    let mut file = std::fs::File::from(fd);
    let stat = fs::fstat(&file).map_err(|_| io("cannot inspect held retention registry file"))?;
    if !FileType::from_raw_mode(stat.st_mode).is_file()
        || stat.st_nlink != 1
        || stat.st_uid != rustix::process::geteuid().as_raw()
        || stat.st_mode & 0o7777 != 0o600
        || stat.st_size < 0
        || stat.st_size as usize > MAX_RETENTION_REGISTRY_CURSOR_BYTES
        || Identity::from_stat(&stat) != expected
    {
        return Err(binding("held retention registry file facts disagree"));
    }
    let mut bytes = Vec::new();
    bytes
        .try_reserve_exact(stat.st_size as usize + 1)
        .map_err(|_| capacity("cannot reserve retention registry cursor bytes"))?;
    file.seek(SeekFrom::Start(0))
        .and_then(|_| {
            (&mut file)
                .take(MAX_RETENTION_REGISTRY_CURSOR_BYTES as u64 + 1)
                .read_to_end(&mut bytes)
        })
        .map_err(|_| io("cannot read retention registry file"))?;
    if bytes.len() != stat.st_size as usize {
        return Err(binding("retention registry file length changed"));
    }
    let after = fs::fstat(&file).map_err(|_| io("cannot recheck held retention registry file"))?;
    let rebound = fs::statat(root.fd(), name, AtFlags::SYMLINK_NOFOLLOW)
        .map_err(|_| binding("retention registry file path disappeared"))?;
    if Identity::from_stat(&after) != expected || Identity::from_stat(&rebound) != expected {
        return Err(binding(
            "retention registry file path or held identity changed",
        ));
    }
    Ok(bytes)
}

fn open_dir(parent: impl AsFd, name: &[u8]) -> Result<OwnedFd> {
    fs::openat(
        parent,
        name,
        OFlags::RDONLY | OFlags::DIRECTORY | OFlags::NOFOLLOW | OFlags::CLOEXEC,
        Mode::empty(),
    )
    .map_err(|_| binding("cannot hold retention registry directory"))
}

fn identity(fd: impl AsFd) -> Result<Identity> {
    fs::fstat(fd)
        .map(|stat| Identity::from_stat(&stat))
        .map_err(|_| io("cannot inspect retention registry directory"))
}

#[cfg(test)]
mod tests {
    use std::os::unix::fs::PermissionsExt;

    use super::*;

    #[test]
    fn held_metadata_child_rejects_same_owner_path_replacement() {
        let root_path = std::env::temp_dir().join(format!(
            "semaprax-retention-registry-metadata-swap-{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&root_path);
        std::fs::create_dir(&root_path).unwrap();
        std::fs::create_dir(root_path.join("metadata")).unwrap();
        std::fs::set_permissions(&root_path, std::fs::Permissions::from_mode(0o700)).unwrap();
        std::fs::set_permissions(
            root_path.join("metadata"),
            std::fs::Permissions::from_mode(0o700),
        )
        .unwrap();

        let root = Root::open(&root_path).unwrap();
        std::fs::rename(root_path.join("metadata"), root_path.join("displaced")).unwrap();
        std::fs::create_dir(root_path.join("metadata")).unwrap();
        std::fs::set_permissions(
            root_path.join("metadata"),
            std::fs::Permissions::from_mode(0o700),
        )
        .unwrap();

        assert_eq!(root.validate().unwrap_err()[0].code, "SPX-G466");

        drop(root);
        std::fs::remove_dir_all(root_path).unwrap();
    }

    #[test]
    fn transaction_rejects_metadata_replacement_before_current_pivot() {
        let root_path = std::env::temp_dir().join(format!(
            "semaprax-retention-registry-pre-pivot-swap-{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&root_path);
        std::fs::create_dir(&root_path).unwrap();
        std::fs::create_dir(root_path.join("metadata")).unwrap();
        std::fs::set_permissions(&root_path, std::fs::Permissions::from_mode(0o700)).unwrap();
        std::fs::set_permissions(
            root_path.join("metadata"),
            std::fs::Permissions::from_mode(0o700),
        )
        .unwrap();

        let errors = transaction(&root_path, |_current, _held_metadata| {
            std::fs::rename(root_path.join("metadata"), root_path.join("displaced")).unwrap();
            std::fs::create_dir(root_path.join("metadata")).unwrap();
            std::fs::set_permissions(
                root_path.join("metadata"),
                std::fs::Permissions::from_mode(0o700),
            )
            .unwrap();
            Ok((b"cursor bytes never published".to_vec(), ()))
        })
        .unwrap_err();

        assert_eq!(errors[0].code, "SPX-G466");
        assert!(!root_path.join("CURRENT").exists());
        std::fs::remove_dir_all(root_path).unwrap();
    }
}
