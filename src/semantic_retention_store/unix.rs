//! Held-root, handle-relative immutable publication for retention metadata.

use std::collections::BTreeMap;
use std::io::{Read, Seek, SeekFrom, Write};
use std::os::fd::{AsFd, OwnedFd};
use std::os::unix::ffi::OsStrExt;
use std::path::{Component, Path, PathBuf};

use rustix::fs::{self, AtFlags, Dir, FileType, Mode, OFlags, RenameFlags, Stat, CWD};

use super::{
    binding, capacity, invalid, io, pair_name, post_pivot, Result,
    MAX_RETENTION_METADATA_ENVELOPE_BYTES, MAX_RETENTION_METADATA_STORE_ENTRIES,
    MAX_RETENTION_METADATA_STORE_PATH_BYTES, MAX_RETENTION_METADATA_STORE_PATH_DEPTH,
};

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

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct FileFact {
    identity: Identity,
    bytes: usize,
}

impl FileFact {
    fn from_stat(stat: &Stat) -> Result<Self> {
        if !FileType::from_raw_mode(stat.st_mode).is_file()
            || stat.st_nlink != 1
            || stat.st_uid != rustix::process::geteuid().as_raw()
            || stat.st_mode & 0o7777 != 0o600
        {
            return Err(binding(
                "retention metadata file must be current-euid-owned regular single-link 0600",
            ));
        }
        if stat.st_size < 0 || stat.st_size as u64 > MAX_RETENTION_METADATA_ENVELOPE_BYTES as u64 {
            return Err(capacity(
                "retention metadata file exceeds the fixed envelope bound",
            ));
        }
        Ok(Self {
            identity: Identity::from_stat(stat),
            bytes: stat.st_size as usize,
        })
    }
}

struct Root {
    path: Option<PathBuf>,
    chain: Vec<(OwnedFd, Identity)>,
    names: Vec<Vec<u8>>,
}

impl Root {
    fn open(path: &Path) -> Result<Self> {
        let raw = path.as_os_str().as_bytes();
        if !path.is_absolute() || raw.len() > MAX_RETENTION_METADATA_STORE_PATH_BYTES {
            return Err(invalid(
                "retention metadata store requires a bounded absolute normalized root",
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
                _ => {
                    return Err(invalid(
                        "retention metadata store root cannot contain dot or parent components",
                    ))
                }
            }
        }
        if names.is_empty()
            || normalized != raw
            || names.len() > MAX_RETENTION_METADATA_STORE_PATH_DEPTH
        {
            return Err(invalid(
                "retention metadata store root must have exact normalized bounded components",
            ));
        }
        let first = open_directory(CWD, b"/")?;
        let first_identity = identity(&first)?;
        let mut chain = vec![(first, first_identity)];
        for name in &names {
            let child = open_directory(&chain.last().expect("root held").0, name)?;
            let fact = identity(&child)?;
            chain.push((child, fact));
        }
        let root = Self {
            path: Some(path.to_owned()),
            chain,
            names,
        };
        let fact = root.chain.last().expect("root held").1;
        if fact.uid != rustix::process::geteuid().as_raw() || fact.mode & 0o7777 != 0o700 {
            return Err(binding(
                "retention metadata store root must be current-euid-owned exact 0700",
            ));
        }
        root.check_chain()?;
        Ok(root)
    }

    fn held(fd: impl AsFd) -> Result<Self> {
        let fd =
            rustix::io::dup(fd).map_err(|_| io("cannot duplicate held retention metadata root"))?;
        let fact = identity(&fd)?;
        if fact.uid != rustix::process::geteuid().as_raw() || fact.mode & 0o7777 != 0o700 {
            return Err(binding(
                "held retention metadata root must be current-euid-owned exact 0700",
            ));
        }
        Ok(Self {
            path: None,
            chain: vec![(fd, fact)],
            names: Vec::new(),
        })
    }

    fn fd(&self) -> &OwnedFd {
        &self.chain.last().expect("root held").0
    }

    fn check_chain(&self) -> Result<()> {
        for (index, (held, expected)) in self.chain.iter().enumerate() {
            if identity(held)? != *expected {
                return Err(binding(
                    "retention metadata held directory identity or mode changed",
                ));
            }
            if index > 0 {
                let stat = fs::statat(
                    &self.chain[index - 1].0,
                    self.names[index - 1].as_slice(),
                    AtFlags::SYMLINK_NOFOLLOW,
                )
                .map_err(|_| binding("retention metadata ancestor path disappeared"))?;
                if Identity::from_stat(&stat) != *expected {
                    return Err(binding(
                        "retention metadata path and held ancestor disagree",
                    ));
                }
            }
        }
        Ok(())
    }

    fn recheck(&self) -> Result<()> {
        self.check_chain()?;
        if let Some(path) = &self.path {
            let rebound = Self::open(path)?;
            if self
                .chain
                .iter()
                .map(|entry| entry.1)
                .ne(rebound.chain.iter().map(|entry| entry.1))
            {
                return Err(binding(
                    "retention metadata absolute path no longer names the held root",
                ));
            }
        }
        Ok(())
    }

    fn lock(&self, exclusive: bool) -> Result<Lock> {
        let file = std::fs::File::from(
            rustix::io::dup(self.fd())
                .map_err(|_| io("cannot duplicate retention metadata root"))?,
        );
        if exclusive {
            fs2::FileExt::try_lock_exclusive(&file)
        } else {
            fs2::FileExt::try_lock_shared(&file)
        }
        .map_err(|_| binding("retention metadata store root is busy"))?;
        Ok(Lock(file))
    }
}

struct Lock(std::fs::File);
impl Lock {
    fn release(self) -> Result<()> {
        fs2::FileExt::unlock(&self.0)
            .map_err(|_| io("cannot release retention metadata store lock"))
    }
}
impl Drop for Lock {
    fn drop(&mut self) {
        let _ = fs2::FileExt::unlock(&self.0);
    }
}

type Inventory = BTreeMap<String, FileFact>;

fn inventory(root: &Root, allow_stage: bool) -> Result<Inventory> {
    let entries = Dir::new(open_directory(root.fd(), b".")?)
        .map_err(|_| io("cannot enumerate retention metadata store root"))?;
    let mut result = BTreeMap::new();
    let mut completed = 0usize;
    let mut stages = 0usize;
    for entry in entries {
        let entry = entry.map_err(|_| io("cannot read retention metadata inventory"))?;
        let raw = entry.file_name().to_bytes();
        if raw == b"." || raw == b".." {
            continue;
        }
        let name = std::str::from_utf8(raw)
            .map_err(|_| binding("retention metadata store has a noncanonical entry name"))?;
        let stage = name.strip_prefix(".stage-").is_some_and(valid_pair_stem);
        if stage {
            stages += 1;
            if !allow_stage || stages > 1 {
                return Err(binding(
                    "retention metadata store contains interrupted or excess stage metadata",
                ));
            }
        } else if name.strip_suffix(".spxr").is_some_and(valid_pair_stem) {
            completed += 1;
            if completed > MAX_RETENTION_METADATA_STORE_ENTRIES {
                return Err(capacity(
                    "retention metadata completed inventory exceeds 32 entries",
                ));
            }
        } else {
            return Err(binding(
                "retention metadata store contains an unexpected entry",
            ));
        }
        let fact = file_at(root.fd(), name)?;
        if !stage && fact.bytes == 0 {
            return Err(binding("completed retention metadata entry is empty"));
        }
        if result.insert(name.to_owned(), fact).is_some() {
            return Err(binding("retention metadata inventory repeated an entry"));
        }
    }
    Ok(result)
}

fn valid_pair_stem(value: &str) -> bool {
    value.len() == 129
        && value.as_bytes()[64] == b'-'
        && canonical_hex(&value[..64])
        && canonical_hex(&value[65..])
}

fn canonical_hex(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

fn unchanged(root: &Root, expected: &Inventory) -> Result<()> {
    root.recheck()?;
    if inventory(root, true)? != *expected {
        return Err(binding(
            "retention metadata inventory identity, mode, or size changed",
        ));
    }
    Ok(())
}

fn open_directory(parent: impl AsFd, name: &[u8]) -> Result<OwnedFd> {
    fs::openat(
        parent,
        name,
        OFlags::RDONLY | OFlags::DIRECTORY | OFlags::NOFOLLOW | OFlags::CLOEXEC,
        Mode::empty(),
    )
    .map_err(|_| binding("cannot hold retention metadata directory without following links"))
}

fn identity(fd: impl AsFd) -> Result<Identity> {
    fs::fstat(fd)
        .map(|stat| Identity::from_stat(&stat))
        .map_err(|_| io("cannot inspect retention metadata store object"))
}

fn file_at(root: &OwnedFd, name: &str) -> Result<FileFact> {
    let stat = fs::statat(root, name.as_bytes(), AtFlags::SYMLINK_NOFOLLOW)
        .map_err(|_| binding("cannot inspect retention metadata file path"))?;
    FileFact::from_stat(&stat)
}

fn file_fact(file: &std::fs::File) -> Result<FileFact> {
    FileFact::from_stat(
        &fs::fstat(file).map_err(|_| io("cannot inspect held retention metadata file"))?,
    )
}

fn read_exact(file: &mut std::fs::File, expected: FileFact) -> Result<Vec<u8>> {
    if file_fact(file)? != expected {
        return Err(binding("held retention metadata file facts changed"));
    }
    file.seek(SeekFrom::Start(0))
        .map_err(|_| io("cannot rewind retention metadata file"))?;
    let mut bytes = Vec::new();
    bytes
        .try_reserve_exact(expected.bytes.saturating_add(1))
        .map_err(|_| capacity("cannot reserve bounded retention metadata input"))?;
    (&mut *file)
        .take(expected.bytes as u64 + 1)
        .read_to_end(&mut bytes)
        .map_err(|_| io("cannot read retention metadata bytes"))?;
    if bytes.len() != expected.bytes || file_fact(file)? != expected {
        return Err(binding("retention metadata file changed while reading"));
    }
    Ok(bytes)
}

fn selected(
    root: &Root,
    name: &str,
    file: &mut std::fs::File,
    expected: FileFact,
    bytes: &[u8],
) -> Result<()> {
    if file_at(root.fd(), name)? != expected
        || read_exact(file, expected)? != bytes
        || file_at(root.fd(), name)? != expected
    {
        return Err(binding(
            "selected retention metadata bytes or path identity changed",
        ));
    }
    Ok(())
}

pub(super) fn persist(root_path: &Path, checkpoint: &str, plan: &str, bytes: &[u8]) -> Result<()> {
    persist_root(Root::open(root_path)?, checkpoint, plan, bytes)
}

pub(super) fn persist_held(
    root_fd: impl AsFd,
    checkpoint: &str,
    plan: &str,
    bytes: &[u8],
) -> Result<()> {
    persist_root(Root::held(root_fd)?, checkpoint, plan, bytes)
}

fn persist_root(root: Root, checkpoint: &str, plan: &str, bytes: &[u8]) -> Result<()> {
    let lock = root.lock(true)?;
    let initial = inventory(&root, false)?;
    if initial.len() >= MAX_RETENTION_METADATA_STORE_ENTRIES {
        return Err(capacity("retention metadata store has no publication slot"));
    }
    let destination = pair_name(checkpoint, plan)?;
    if initial.contains_key(&destination) {
        return Err(binding(
            "retention metadata destination already exists; no adoption or overwrite",
        ));
    }
    let stage = format!(
        ".stage-{}",
        destination
            .strip_suffix(".spxr")
            .expect("closed pair filename")
    );
    unchanged(&root, &initial)?;
    let fd = fs::openat(
        root.fd(),
        stage.as_bytes(),
        OFlags::RDWR
            | OFlags::CREATE
            | OFlags::EXCL
            | OFlags::NOFOLLOW
            | OFlags::CLOEXEC
            | OFlags::NONBLOCK,
        Mode::from_raw_mode(0o600),
    )
    .map_err(|_| io("cannot create retention metadata stage without replacement"))?;
    let mut file = std::fs::File::from(fd);
    let created = file_fact(&file)?;
    if created.bytes != 0 {
        return Err(binding("new retention metadata stage is not empty"));
    }
    let mut staged = initial.clone();
    staged.insert(stage.clone(), created);
    unchanged(&root, &staged)?;
    selected(&root, &stage, &mut file, created, b"")?;
    file.write_all(bytes)
        .and_then(|()| file.sync_all())
        .map_err(|_| io("cannot write and settle retention metadata stage"))?;
    let written = file_fact(&file)?;
    if written.identity != created.identity || written.bytes != bytes.len() {
        return Err(binding(
            "retention metadata stage facts disagree after writing",
        ));
    }
    staged.insert(stage.clone(), written);
    unchanged(&root, &staged)?;
    selected(&root, &stage, &mut file, written, bytes)?;
    fs::fsync(root.fd()).map_err(|_| io("cannot settle retention metadata stage directory"))?;
    unchanged(&root, &staged)?;
    selected(&root, &stage, &mut file, written, bytes)?;
    file.sync_all()
        .map_err(|_| io("cannot resettle retention metadata stage"))?;
    fs::renameat_with(
        root.fd(),
        stage.as_bytes(),
        root.fd(),
        destination.as_bytes(),
        RenameFlags::NOREPLACE,
    )
    .map_err(|_| io("cannot publish retention metadata without replacement; stage is retained"))?;
    let final_check = (|| -> Result<()> {
        fs::fsync(root.fd()).map_err(|_| io("cannot settle retention metadata directory"))?;
        let mut published = initial;
        published.insert(destination.clone(), written);
        unchanged(&root, &published)?;
        selected(&root, &destination, &mut file, written, bytes)?;
        lock.release()?;
        Ok(())
    })();
    final_check.map_err(|_| {
        post_pivot(
            "retention metadata publication may have occurred; resolve by exact load, never blind retry",
        )
    })
}

pub(super) fn load<T>(
    root_path: &Path,
    checkpoint: &str,
    plan: &str,
    restore: impl FnOnce(&[u8]) -> Result<T>,
) -> Result<T> {
    load_root(Root::open(root_path)?, checkpoint, plan, restore)
}

pub(super) fn load_held<T>(
    root_fd: impl AsFd,
    checkpoint: &str,
    plan: &str,
    restore: impl FnOnce(&[u8]) -> Result<T>,
) -> Result<T> {
    load_root(Root::held(root_fd)?, checkpoint, plan, restore)
}

fn load_root<T>(
    root: Root,
    checkpoint: &str,
    plan: &str,
    restore: impl FnOnce(&[u8]) -> Result<T>,
) -> Result<T> {
    let lock = root.lock(false)?;
    let initial = inventory(&root, true)?;
    let name = pair_name(checkpoint, plan)?;
    let expected = *initial
        .get(&name)
        .ok_or_else(|| binding("selected retention metadata pair is absent"))?;
    unchanged(&root, &initial)?;
    let fd = fs::openat(
        root.fd(),
        name.as_bytes(),
        OFlags::RDONLY | OFlags::NOFOLLOW | OFlags::NONBLOCK | OFlags::CLOEXEC,
        Mode::empty(),
    )
    .map_err(|_| binding("cannot open selected retention metadata without following links"))?;
    let mut file = std::fs::File::from(fd);
    let bytes = read_exact(&mut file, expected)?;
    let value = restore(&bytes)?;
    unchanged(&root, &initial)?;
    selected(&root, &name, &mut file, expected, &bytes)?;
    lock.release()?;
    Ok(value)
}
