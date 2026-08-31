//! Handle-relative single-file publication. The host excludes uncooperative
//! same-principal namespace/content mutation; the advisory lock is cooperative.
use std::collections::BTreeMap;
use std::io::{Read, Seek, SeekFrom, Write};
use std::os::fd::{AsFd, OwnedFd};
use std::os::unix::ffi::OsStrExt;
use std::path::{Component, Path, PathBuf};

use rustix::fs::{self, AtFlags, Dir, FileType, Mode, OFlags, RenameFlags, Stat, CWD};

use super::{
    binding, canonical_hex, capacity, digest_hex, invalid, io, post_pivot, Result,
    MAX_CANDIDATE_ARCHIVE_STORE_ENTRIES, MAX_CANDIDATE_ARCHIVE_STORE_PATH_BYTES,
    MAX_CANDIDATE_ARCHIVE_STORE_PATH_DEPTH,
};
use crate::project::{
    ProjectCandidate, ProjectCandidateArchive, ProjectCandidateDraft, ProjectCandidateDraftArchive,
    MAX_PROJECT_CANDIDATE_ARCHIVE_BYTES,
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
                "archive store file must be current-euid-owned regular single-link 0600",
            ));
        }
        if stat.st_size < 0 || stat.st_size as u64 > MAX_PROJECT_CANDIDATE_ARCHIVE_BYTES as u64 {
            return Err(capacity("archive store file exceeds the fixed byte limit"));
        }
        Ok(Self {
            identity: Identity::from_stat(stat),
            bytes: stat.st_size as usize,
        })
    }
}
struct Root {
    path: PathBuf,
    chain: Vec<(OwnedFd, Identity)>,
    names: Vec<Vec<u8>>,
}
impl Root {
    fn open(path: &Path) -> Result<Self> {
        let raw = path.as_os_str().as_bytes();
        if !path.is_absolute() || raw.len() > MAX_CANDIDATE_ARCHIVE_STORE_PATH_BYTES {
            return Err(invalid(
                "archive store requires a bounded absolute normalized root",
            ));
        }
        let mut normalized = Vec::new();
        let mut names = Vec::new();
        for part in path.components() {
            match part {
                Component::RootDir => {}
                Component::Normal(name) => {
                    normalized.push(b'/');
                    normalized.extend_from_slice(name.as_bytes());
                    names.push(name.as_bytes().to_vec());
                }
                _ => {
                    return Err(invalid(
                        "archive store root cannot contain dot or parent components",
                    ))
                }
            }
        }
        if names.is_empty()
            || normalized != raw
            || names.len() > MAX_CANDIDATE_ARCHIVE_STORE_PATH_DEPTH
        {
            return Err(invalid(
                "archive store root must have exact normalized bounded components",
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
            path: path.to_owned(),
            chain,
            names,
        };
        let fact = root.chain.last().expect("root held").1;
        if fact.uid != rustix::process::geteuid().as_raw() || fact.mode & 0o7777 != 0o700 {
            return Err(binding(
                "archive store root must be a current-euid-owned exact 0700 directory",
            ));
        }
        root.check_chain()?;
        Ok(root)
    }
    fn fd(&self) -> &OwnedFd {
        &self.chain.last().expect("root held").0
    }
    fn check_chain(&self) -> Result<()> {
        for (index, (held, expected)) in self.chain.iter().enumerate() {
            if identity(held)? != *expected {
                return Err(binding(
                    "archive store held directory identity or mode changed",
                ));
            }
            if index > 0 {
                let stat = fs::statat(
                    &self.chain[index - 1].0,
                    self.names[index - 1].as_slice(),
                    AtFlags::SYMLINK_NOFOLLOW,
                )
                .map_err(|_| binding("archive store ancestor path disappeared"))?;
                if Identity::from_stat(&stat) != *expected {
                    return Err(binding("archive store path and held ancestor disagree"));
                }
            }
        }
        Ok(())
    }
    fn recheck(&self) -> Result<()> {
        self.check_chain()?;
        let rebound = Self::open(&self.path)?;
        if self
            .chain
            .iter()
            .map(|x| x.1)
            .ne(rebound.chain.iter().map(|x| x.1))
        {
            return Err(binding(
                "archive store absolute path no longer names the held directory chain",
            ));
        }
        Ok(())
    }
    fn lock(&self, exclusive: bool) -> Result<Lock> {
        let file = std::fs::File::from(
            rustix::io::dup(self.fd()).map_err(|_| io("cannot duplicate archive store root"))?,
        );
        if exclusive {
            fs2::FileExt::try_lock_exclusive(&file)
        } else {
            fs2::FileExt::try_lock_shared(&file)
        }
        .map_err(|_| binding("archive store root is busy"))?;
        Ok(Lock(file))
    }
}
struct Lock(std::fs::File);
impl Lock {
    fn release(self) -> Result<()> {
        fs2::FileExt::unlock(&self.0).map_err(|_| io("cannot release archive store lock"))
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
        .map_err(|_| io("cannot enumerate archive store root"))?;
    let mut result = BTreeMap::new();
    let mut completed = 0;
    let mut stages = 0;
    for entry in entries {
        let entry = entry.map_err(|_| io("cannot read archive store inventory"))?;
        let raw = entry.file_name().to_bytes();
        if raw == b"." || raw == b".." {
            continue;
        }
        let name = std::str::from_utf8(raw)
            .map_err(|_| binding("archive store has a noncanonical entry name"))?;
        let stage = name.strip_prefix(".stage-").is_some_and(canonical_hex);
        if stage {
            stages += 1;
            if !allow_stage || stages > 1 {
                return Err(binding(
                    "archive store contains a failed or excess stage; no automatic recovery",
                ));
            }
        } else if name.strip_suffix(".json").is_some_and(canonical_hex) {
            completed += 1;
            if completed > MAX_CANDIDATE_ARCHIVE_STORE_ENTRIES {
                return Err(capacity(
                    "archive store completed-entry inventory exceeds 32",
                ));
            }
        } else {
            return Err(binding("archive store contains an unexpected entry"));
        }
        let fact = file_at(root.fd(), name)?;
        if !stage && fact.bytes == 0 {
            return Err(binding("completed archive store entry is empty"));
        }
        if result.insert(name.to_owned(), fact).is_some() {
            return Err(binding("archive store inventory repeated an entry"));
        }
    }
    Ok(result)
}
fn unchanged(root: &Root, expected: &Inventory) -> Result<()> {
    root.recheck()?;
    if inventory(root, true)? != *expected {
        return Err(binding(
            "archive store inventory identity, mode, or size changed",
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
    .map_err(|_| binding("cannot hold archive store directory without following links"))
}
fn identity(fd: impl AsFd) -> Result<Identity> {
    fs::fstat(fd)
        .map(|stat| Identity::from_stat(&stat))
        .map_err(|_| io("cannot inspect archive store object"))
}
fn file_at(root: &OwnedFd, name: &str) -> Result<FileFact> {
    let stat = fs::statat(root, name.as_bytes(), AtFlags::SYMLINK_NOFOLLOW)
        .map_err(|_| binding("cannot inspect archive store file path"))?;
    FileFact::from_stat(&stat)
}
fn file_fact(file: &std::fs::File) -> Result<FileFact> {
    FileFact::from_stat(&fs::fstat(file).map_err(|_| io("cannot inspect held archive store file"))?)
}
fn read_exact(file: &mut std::fs::File, expected: FileFact) -> Result<Vec<u8>> {
    if file_fact(file)? != expected {
        return Err(binding("held archive file metadata changed"));
    }
    file.seek(SeekFrom::Start(0))
        .map_err(|_| io("cannot rewind archive store file"))?;
    let mut bytes = Vec::new();
    (&mut *file)
        .take(expected.bytes as u64 + 1)
        .read_to_end(&mut bytes)
        .map_err(|_| io("cannot read archive store bytes"))?;
    if bytes.len() != expected.bytes || file_fact(file)? != expected {
        return Err(binding("archive store file changed while reading"));
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
        return Err(binding("selected archive bytes or path identity changed"));
    }
    Ok(())
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum StorePoint {
    AfterStageCreate,
    AfterStageWrite,
    BeforePublish,
    AfterPublish,
}
pub(super) fn persist(root: &Path, archive: &ProjectCandidateArchive) -> Result<()> {
    persist_with_hook(root, archive, |_, _| Ok(()))
}
pub(super) fn persist_with_hook(
    root_path: &Path,
    archive: &ProjectCandidateArchive,
    hook: impl FnMut(StorePoint, &Path) -> std::io::Result<()>,
) -> Result<()> {
    persist_bytes_with_hook(
        root_path,
        archive.archive_digest(),
        archive.to_json().as_bytes(),
        hook,
    )
}
pub(super) fn persist_draft(root: &Path, archive: &ProjectCandidateDraftArchive) -> Result<()> {
    persist_bytes_with_hook(
        root,
        archive.archive_digest(),
        archive.to_json().as_bytes(),
        |_, _| Ok(()),
    )
}
// Private IO seam: callers have independently replayed their typed archive.
// No public raw-byte publication or generic authority is introduced.
fn persist_bytes_with_hook(
    root_path: &Path,
    archive_digest: &str,
    bytes: &[u8],
    mut hook: impl FnMut(StorePoint, &Path) -> std::io::Result<()>,
) -> Result<()> {
    let root = Root::open(root_path)?;
    let lock = root.lock(true)?;
    let initial = inventory(&root, false)?;
    if initial.len() >= MAX_CANDIDATE_ARCHIVE_STORE_ENTRIES {
        return Err(capacity("archive store has no publication slot"));
    }
    let hex = digest_hex(archive_digest)?;
    let destination = format!("{hex}.json");
    if initial.contains_key(&destination) {
        return Err(binding(
            "archive store destination already exists; no adoption or overwrite",
        ));
    }
    let stage = format!(".stage-{hex}");
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
    .map_err(|_| io("cannot create archive store stage without replacement"))?;
    let mut file = std::fs::File::from(fd);
    let created = file_fact(&file)?;
    if created.bytes != 0 {
        return Err(binding("new archive stage is not empty"));
    }
    let mut staged = initial.clone();
    staged.insert(stage.clone(), created);
    hook(StorePoint::AfterStageCreate, root_path)
        .map_err(|_| io("archive stage creation hook failed"))?;
    unchanged(&root, &staged)?;
    selected(&root, &stage, &mut file, created, b"")?;
    file.write_all(bytes)
        .and_then(|()| file.sync_all())
        .map_err(|_| io("cannot write and settle archive stage"))?;
    let written = file_fact(&file)?;
    if written.identity != created.identity || written.bytes != bytes.len() {
        return Err(binding("archive stage metadata disagrees after writing"));
    }
    staged.insert(stage.clone(), written);
    hook(StorePoint::AfterStageWrite, root_path)
        .map_err(|_| io("archive stage write hook failed"))?;
    unchanged(&root, &staged)?;
    selected(&root, &stage, &mut file, written, bytes)?;
    fs::fsync(root.fd()).map_err(|_| io("cannot settle archive stage directory"))?;
    hook(StorePoint::BeforePublish, root_path)
        .map_err(|_| io("archive pre-publication hook failed"))?;
    unchanged(&root, &staged)?;
    selected(&root, &stage, &mut file, written, bytes)?;
    file.sync_all()
        .map_err(|_| io("cannot resettle archive stage"))?;
    fs::renameat_with(
        root.fd(),
        stage.as_bytes(),
        root.fd(),
        destination.as_bytes(),
        RenameFlags::NOREPLACE,
    )
    .map_err(|_| io("cannot publish archive without replacement; stage is retained"))?;
    // Everything after the successful namespace pivot is uncertainty on failure.
    let final_check = (|| -> Result<()> {
        fs::fsync(root.fd()).map_err(|_| io("cannot settle published archive directory"))?;
        hook(StorePoint::AfterPublish, root_path)
            .map_err(|_| io("archive post-publication hook failed"))?;
        let mut published = initial;
        published.insert(destination.clone(), written);
        unchanged(&root, &published)?;
        selected(&root, &destination, &mut file, written, bytes)?;
        lock.release()?;
        Ok(())
    })();
    final_check.map_err(|_|post_pivot("archive publication may have occurred; retain its digest and resolve with exact load, never blind retry"))
}

pub(super) fn load(
    root_path: &Path,
    archive_digest: &str,
    candidate_digest: &str,
) -> Result<ProjectCandidate> {
    load_with(root_path, archive_digest, |bytes| {
        ProjectCandidateArchive::restore(bytes, archive_digest, candidate_digest)
    })
}

pub(super) fn load_draft(
    root_path: &Path,
    archive_digest: &str,
    draft_digest: &str,
) -> Result<ProjectCandidateDraft> {
    load_with(root_path, archive_digest, |bytes| {
        ProjectCandidateDraftArchive::restore(bytes, archive_digest, draft_digest)
    })
}

// Typed replay runs inside the held lock and descriptors; successful results
// cannot leave this scope before the original selected bytes are rechecked.
fn load_with<T>(
    root_path: &Path,
    archive_digest: &str,
    restore: impl FnOnce(&[u8]) -> Result<T>,
) -> Result<T> {
    let root = Root::open(root_path)?;
    let lock = root.lock(false)?;
    let initial = inventory(&root, true)?;
    let name = format!("{}.json", digest_hex(archive_digest)?);
    let expected = *initial
        .get(&name)
        .ok_or_else(|| binding("selected archive is absent from the store"))?;
    unchanged(&root, &initial)?;
    let fd = fs::openat(
        root.fd(),
        name.as_bytes(),
        OFlags::RDONLY | OFlags::NOFOLLOW | OFlags::NONBLOCK | OFlags::CLOEXEC,
        Mode::empty(),
    )
    .map_err(|_| binding("cannot open selected archive without following links"))?;
    let mut file = std::fs::File::from(fd);
    let bytes = read_exact(&mut file, expected)?;
    let value = restore(&bytes)?;
    unchanged(&root, &initial)?;
    selected(&root, &name, &mut file, expected, &bytes)?;
    lock.release()?;
    Ok(value)
}
