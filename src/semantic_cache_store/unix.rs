//! Handle-relative single-file publication. The host excludes uncooperative
//! same-principal namespace/content mutation; the advisory lock is cooperative.
use std::collections::BTreeMap;
use std::io::{Read, Seek, SeekFrom, Write};
use std::os::fd::{AsFd, OwnedFd};
use std::os::unix::ffi::OsStrExt;
use std::path::{Component, Path, PathBuf};

use rustix::fs::{self, AtFlags, Dir, FileType, Mode, OFlags, RenameFlags, Stat, CWD};
use sha2::{Digest, Sha256};

use super::{
    binding, canonical_hex, capacity, digest_hex, invalid, io, post_pivot, Result,
    SemanticCacheEvictionReceipt, SemanticCacheReceipt, MAX_ENVELOPE_BYTES,
    MAX_SEMANTIC_CACHE_COMPILER_BYTES, MAX_SEMANTIC_CACHE_STORE_ENTRIES,
};
use crate::project::ProjectFrontendCache;
const MAX_SEMANTIC_CACHE_STORE_PATH_BYTES: usize = 4096;
const MAX_SEMANTIC_CACHE_STORE_PATH_DEPTH: usize = 64;
const KEY: &str = "compiler-cache.key";

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
                "semantic cache store file must be current-euid-owned regular single-link 0600",
            ));
        }
        if stat.st_size < 0 || stat.st_size as u64 > MAX_ENVELOPE_BYTES as u64 {
            return Err(capacity(
                "semantic cache store file exceeds the fixed byte limit",
            ));
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
    private: bool,
}
impl Root {
    fn open(path: &Path) -> Result<Self> {
        Self::open_chain(path, true)
    }
    fn open_chain(path: &Path, private: bool) -> Result<Self> {
        let raw = path.as_os_str().as_bytes();
        if !path.is_absolute() || raw.len() > MAX_SEMANTIC_CACHE_STORE_PATH_BYTES {
            return Err(invalid(
                "semantic cache store requires a bounded absolute normalized root",
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
                        "semantic cache store root cannot contain dot or parent components",
                    ))
                }
            }
        }
        if names.is_empty() && !private {
            normalized.push(b'/');
        }
        if (names.is_empty() && private)
            || normalized != raw
            || names.len() > MAX_SEMANTIC_CACHE_STORE_PATH_DEPTH
        {
            return Err(invalid(
                "semantic cache store root must have exact normalized bounded components",
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
            private,
        };
        let fact = root.chain.last().expect("root held").1;
        if private
            && (fact.uid != rustix::process::geteuid().as_raw() || fact.mode & 0o7777 != 0o700)
        {
            return Err(binding(
                "semantic cache store root must be a current-euid-owned exact 0700 directory",
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
                    "semantic cache store held directory identity or mode changed",
                ));
            }
            if index > 0 {
                let stat = fs::statat(
                    &self.chain[index - 1].0,
                    self.names[index - 1].as_slice(),
                    AtFlags::SYMLINK_NOFOLLOW,
                )
                .map_err(|_| binding("semantic cache store ancestor path disappeared"))?;
                if Identity::from_stat(&stat) != *expected {
                    return Err(binding(
                        "semantic cache store path and held ancestor disagree",
                    ));
                }
            }
        }
        Ok(())
    }
    fn recheck(&self) -> Result<()> {
        self.check_chain()?;
        let rebound = Self::open_chain(&self.path, self.private)?;
        if self
            .chain
            .iter()
            .map(|x| x.1)
            .ne(rebound.chain.iter().map(|x| x.1))
        {
            return Err(binding(
                "semantic cache store absolute path no longer names the held directory chain",
            ));
        }
        Ok(())
    }
    fn lock(&self, exclusive: bool) -> Result<Lock> {
        let file = std::fs::File::from(
            rustix::io::dup(self.fd())
                .map_err(|_| io("cannot duplicate semantic cache store root"))?,
        );
        if exclusive {
            fs2::FileExt::try_lock_exclusive(&file)
        } else {
            fs2::FileExt::try_lock_shared(&file)
        }
        .map_err(|_| binding("semantic cache store root is busy"))?;
        Ok(Lock(file))
    }
}
struct Lock(std::fs::File);
impl Lock {
    fn release(self) -> Result<()> {
        fs2::FileExt::unlock(&self.0).map_err(|_| io("cannot release semantic cache store lock"))
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
        .map_err(|_| io("cannot enumerate semantic cache store root"))?;
    let mut result = BTreeMap::new();
    let mut completed = 0;
    let mut stages = 0;
    for entry in entries {
        let entry = entry.map_err(|_| io("cannot read semantic cache store inventory"))?;
        let raw = entry.file_name().to_bytes();
        if raw == b"." || raw == b".." {
            continue;
        }
        let name = std::str::from_utf8(raw)
            .map_err(|_| binding("semantic cache store has a noncanonical entry name"))?;
        let stage = name == ".stage-key" || name.strip_prefix(".stage-").is_some_and(canonical_hex);
        if stage {
            stages += 1;
            if !allow_stage || stages > 1 {
                return Err(binding(
                    "semantic cache store contains a failed or excess stage; no automatic recovery",
                ));
            }
        } else if name == KEY {
            // The fixed key is mandatory for load/persist, but initialize must
            // be able to authenticate an empty inventory before provisioning.
        } else if name.strip_suffix(".bin").is_some_and(canonical_hex) {
            completed += 1;
            if completed > MAX_SEMANTIC_CACHE_STORE_ENTRIES {
                return Err(capacity(
                    "semantic cache store completed-entry inventory exceeds 32",
                ));
            }
        } else {
            return Err(binding("semantic cache store contains an unexpected entry"));
        }
        let fact = file_at(root.fd(), name)?;
        if name == KEY && fact.bytes != 32 {
            return Err(binding("semantic cache key must contain exactly32bytes"));
        }
        if name == ".stage-key" && fact.bytes > 32 {
            return Err(binding("semantic cache key stage exceeds32bytes"));
        }
        if !stage && fact.bytes == 0 {
            return Err(binding("completed semantic cache store entry is empty"));
        }
        if result.insert(name.to_owned(), fact).is_some() {
            return Err(binding("semantic cache store inventory repeated an entry"));
        }
    }
    Ok(result)
}
fn unchanged(root: &Root, expected: &Inventory) -> Result<()> {
    root.recheck()?;
    if inventory(root, true)? != *expected {
        return Err(binding(
            "semantic cache store inventory identity, mode, or size changed",
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
    .map_err(|_| binding("cannot hold semantic cache store directory without following links"))
}
fn identity(fd: impl AsFd) -> Result<Identity> {
    fs::fstat(fd)
        .map(|stat| Identity::from_stat(&stat))
        .map_err(|_| io("cannot inspect semantic cache store object"))
}
fn file_at(root: &OwnedFd, name: &str) -> Result<FileFact> {
    let stat = fs::statat(root, name.as_bytes(), AtFlags::SYMLINK_NOFOLLOW)
        .map_err(|_| binding("cannot inspect semantic cache store file path"))?;
    FileFact::from_stat(&stat)
}
fn file_fact(file: &std::fs::File) -> Result<FileFact> {
    FileFact::from_stat(
        &fs::fstat(file).map_err(|_| io("cannot inspect held semantic cache store file"))?,
    )
}
fn read_exact(file: &mut std::fs::File, expected: FileFact) -> Result<Vec<u8>> {
    if file_fact(file)? != expected {
        return Err(binding("held semantic cache file metadata changed"));
    }
    file.seek(SeekFrom::Start(0))
        .map_err(|_| io("cannot rewind semantic cache store file"))?;
    let mut bytes = Vec::new();
    bytes
        .try_reserve_exact(expected.bytes + 1)
        .map_err(|_| capacity("cannot reserve bounded semantic cache input"))?;
    (&mut *file)
        .take(expected.bytes as u64 + 1)
        .read_to_end(&mut bytes)
        .map_err(|_| io("cannot read semantic cache store bytes"))?;
    if bytes.len() != expected.bytes || file_fact(file)? != expected {
        return Err(binding("semantic cache store file changed while reading"));
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
            "selected semantic cache bytes or path identity changed",
        ));
    }
    Ok(())
}

fn publish(
    root: &Root,
    lock: Lock,
    initial: Inventory,
    destination: &str,
    stage: &str,
    bytes: &[u8],
    mut verify: impl FnMut() -> Result<()>,
) -> Result<()> {
    if initial.contains_key(destination) {
        return Err(binding(
            "semantic cache destination already exists; no adoption or overwrite",
        ));
    }
    unchanged(root, &initial)?;
    verify()?;
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
    .map_err(|_| io("cannot create semantic cache store stage without replacement"))?;
    let mut file = std::fs::File::from(fd);
    let created = file_fact(&file)?;
    if created.bytes != 0 {
        return Err(binding("new semantic cache stage is not empty"));
    }
    let mut staged = initial.clone();
    staged.insert(stage.to_owned(), created);
    unchanged(root, &staged)?;
    selected(root, stage, &mut file, created, b"")?;
    file.write_all(bytes)
        .and_then(|()| file.sync_all())
        .map_err(|_| io("cannot write and settle semantic cache stage"))?;
    let written = file_fact(&file)?;
    if written.identity != created.identity || written.bytes != bytes.len() {
        return Err(binding(
            "semantic cache stage metadata disagrees after writing",
        ));
    }
    staged.insert(stage.to_owned(), written);
    unchanged(root, &staged)?;
    selected(root, stage, &mut file, written, bytes)?;
    fs::fsync(root.fd()).map_err(|_| io("cannot settle semantic cache stage directory"))?;
    verify()?;
    unchanged(root, &staged)?;
    selected(root, stage, &mut file, written, bytes)?;
    file.sync_all()
        .map_err(|_| io("cannot resettle semantic cache stage"))?;
    fs::renameat_with(
        root.fd(),
        stage.as_bytes(),
        root.fd(),
        destination.as_bytes(),
        RenameFlags::NOREPLACE,
    )
    .map_err(|_| io("cannot publish semantic cache without replacement; stage is retained"))?;
    // Everything after the successful namespace pivot is uncertainty on failure.
    let final_check = (|| -> Result<()> {
        fs::fsync(root.fd()).map_err(|_| io("cannot settle published semantic cache directory"))?;
        let mut published = initial;
        published.insert(destination.to_owned(), written);
        unchanged(root, &published)?;
        selected(root, destination, &mut file, written, bytes)?;
        verify()?;
        lock.release()?;
        Ok(())
    })();
    final_check.map_err(|_|post_pivot("semantic cache publication may have occurred; inspect the selected identity without blind retry or cleanup"))
}

struct Secret([u8; 32]);
impl Drop for Secret {
    fn drop(&mut self) {
        self.0.fill(0);
    }
}
struct Key {
    secret: Secret,
    file: std::fs::File,
    fact: FileFact,
}
impl Key {
    fn open(root: &Root, entries: &Inventory) -> Result<Self> {
        let fact = *entries
            .get(KEY)
            .ok_or_else(|| binding("semantic cache root is not initialized"))?;
        let mut file = open_file(root, KEY)?;
        let mut bytes = read_exact(&mut file, fact)?;
        let secret: [u8; 32] = bytes
            .as_slice()
            .try_into()
            .map_err(|_| binding("semantic cache key is not32bytes"))?;
        bytes.fill(0);
        if secret.iter().all(|b| *b == 0) {
            return Err(binding("semantic cache key is invalid"));
        }
        let mut key = Self {
            secret: Secret(secret),
            file,
            fact,
        };
        key.recheck(root)?;
        Ok(key)
    }
    fn recheck(&mut self, root: &Root) -> Result<()> {
        selected(root, KEY, &mut self.file, self.fact, &self.secret.0)
    }
}
fn open_file(root: &Root, name: &str) -> Result<std::fs::File> {
    fs::openat(
        root.fd(),
        name.as_bytes(),
        OFlags::RDONLY | OFlags::NOFOLLOW | OFlags::NONBLOCK | OFlags::CLOEXEC,
        Mode::empty(),
    )
    .map(std::fs::File::from)
    .map_err(|_| binding("cannot hold selected semantic cache file without following links"))
}

pub(super) fn initialize(root_path: &Path) -> Result<()> {
    let root = Root::open(root_path)?;
    let lock = root.lock(true)?;
    let initial = inventory(&root, false)?;
    if !initial.is_empty() {
        return Err(binding(
            "semantic cache initialization requires an EMPTY dedicated root",
        ));
    }
    let mut secret = Secret([0; 32]);
    getrandom::fill(&mut secret.0)
        .map_err(|_| io("cannot obtain operating-system cache key entropy"))?;
    if secret.0.iter().all(|byte| *byte == 0) {
        return Err(io("operating-system cache key entropy was invalid"));
    }
    publish(
        &root,
        lock,
        initial,
        KEY,
        ".stage-key",
        &secret.0,
        || Ok(()),
    )
}

pub(super) fn persist(root_path: &Path, payload: &[u8]) -> Result<SemanticCacheReceipt> {
    let mut compiler = Compiler::capture()?;
    let root = Root::open(root_path)?;
    let lock = root.lock(true)?;
    let initial = inventory(&root, false)?;
    if initial.keys().filter(|name| name.ends_with(".bin")).count()
        >= MAX_SEMANTIC_CACHE_STORE_ENTRIES
    {
        return Err(capacity("semantic cache store has no publication slot"));
    }
    let mut key = Key::open(&root, &initial)?;
    let (bytes, receipt) = super::seal(payload, &key.secret.0, &compiler.digest)?;
    let hex = digest_hex(receipt.entry_digest())?;
    let destination = format!("{hex}.bin");
    let stage = format!(".stage-{hex}");
    publish(&root, lock, initial, &destination, &stage, &bytes, || {
        key.recheck(&root)?;
        compiler.recheck()
    })?;
    Ok(receipt)
}

pub(super) fn load(root_path: &Path, expected_digest: &str) -> Result<ProjectFrontendCache> {
    let mut compiler = Compiler::capture()?;
    let root = Root::open(root_path)?;
    let lock = root.lock(false)?;
    let initial = inventory(&root, true)?;
    let mut key = Key::open(&root, &initial)?;
    let name = format!("{}.bin", digest_hex(expected_digest)?);
    let expected = *initial
        .get(&name)
        .ok_or_else(|| binding("selected semantic cache is absent from the store"))?;
    unchanged(&root, &initial)?;
    let mut file = open_file(&root, &name)?;
    let bytes = read_exact(&mut file, expected)?;
    let payload = super::authenticate(&bytes, expected_digest, &key.secret.0, &compiler.digest)?;
    // This is the sole filesystem path to private HIR decoding: authentication
    // has succeeded over every payload byte and the current compiler context.
    let cache = crate::project::incremental::decode_snapshot(payload)?;
    unchanged(&root, &initial)?;
    selected(&root, &name, &mut file, expected, &bytes)?;
    key.recheck(&root)?;
    compiler.recheck()?;
    lock.release()?;
    Ok(cache)
}

pub(super) fn evict(
    root_path: &Path,
    expected_digest: &str,
) -> Result<SemanticCacheEvictionReceipt> {
    let root = Root::open(root_path)?;
    let lock = root.lock(true)?;
    let initial = inventory(&root, false)?;
    if !initial.contains_key(KEY) {
        return Err(binding("semantic cache root is not initialized"));
    }
    let name = format!("{}.bin", digest_hex(expected_digest)?);
    let expected = *initial
        .get(&name)
        .ok_or_else(|| binding("selected semantic cache is absent from the store"))?;
    unchanged(&root, &initial)?;
    let mut file = open_file(&root, &name)?;
    let bytes = read_exact(&mut file, expected)?;
    if super::hash(&bytes) != expected_digest {
        return Err(binding(
            "selected semantic cache bytes do not match the requested digest",
        ));
    }
    selected(&root, &name, &mut file, expected, &bytes)?;
    let entries_remaining = initial
        .keys()
        .filter(|entry| entry.ends_with(".bin"))
        .count()
        - 1;
    fs::unlinkat(root.fd(), name.as_bytes(), AtFlags::empty())
        .map_err(|_| io("cannot remove selected semantic cache entry"))?;
    // Everything after successful unlink is uncertainty on failure. The exact
    // held bytes and namespace identity were authenticated immediately before
    // the pivot; unlink necessarily changes the held file's link count.
    let final_check = (|| -> Result<()> {
        fs::fsync(root.fd()).map_err(|_| io("cannot settle semantic cache eviction"))?;
        let mut final_inventory = initial;
        final_inventory.remove(&name);
        unchanged(&root, &final_inventory)?;
        lock.release()?;
        Ok(())
    })();
    final_check.map_err(|_| {
        post_pivot(
            "semantic cache eviction may have occurred; inspect the selected identity without blind retry or cleanup",
        )
    })?;
    Ok(SemanticCacheEvictionReceipt {
        entry: expected_digest.to_owned(),
        envelope_bytes: bytes.len(),
        entries_remaining,
    })
}

#[derive(Clone, Copy, Eq, PartialEq)]
struct CompilerFact {
    identity: Identity,
    bytes: u64,
    links: u64,
}
#[allow(
    clippy::unnecessary_cast,
    reason = "stat field widths vary across Unix ABIs"
)]
fn compiler_fact(stat: &Stat) -> Result<CompilerFact> {
    if !FileType::from_raw_mode(stat.st_mode).is_file()
        || stat.st_size <= 0
        || stat.st_size as u64 > MAX_SEMANTIC_CACHE_COMPILER_BYTES as u64
        || stat.st_nlink != 1
        || stat.st_mode as u32 & 0o111 == 0
        || stat.st_mode as u32 & 0o022 != 0
    {
        return Err(binding("compiler installation must be a bounded executable regular single-link file without group/other write authority"));
    }
    Ok(CompilerFact {
        identity: Identity::from_stat(stat),
        bytes: stat.st_size as u64,
        links: stat.st_nlink as u64,
    })
}
struct Compiler {
    path: PathBuf,
    parent: Root,
    name: Vec<u8>,
    file: std::fs::File,
    fact: CompilerFact,
    digest: [u8; 32],
}
impl Compiler {
    fn capture() -> Result<Self> {
        let path = std::env::current_exe()
            .map_err(|_| io("cannot locate current compiler executable"))?
            .canonicalize()
            .map_err(|_| binding("cannot resolve trusted immutable compiler installation"))?;
        if path.as_os_str().as_bytes().len() > MAX_SEMANTIC_CACHE_STORE_PATH_BYTES {
            return Err(capacity(
                "current compiler executable path exceeds4096bytes",
            ));
        }
        let parent = Root::open_chain(
            path.parent()
                .ok_or_else(|| binding("compiler executable has no parent"))?,
            false,
        )?;
        let name = path
            .file_name()
            .ok_or_else(|| binding("compiler executable has no leaf"))?
            .as_bytes()
            .to_vec();
        let fd = fs::openat(
            parent.fd(),
            name.as_slice(),
            OFlags::RDONLY | OFlags::NOFOLLOW | OFlags::NONBLOCK | OFlags::CLOEXEC,
            Mode::empty(),
        )
        .map_err(|_| binding("cannot hold current compiler executable"))?;
        let file = std::fs::File::from(fd);
        let fact = compiler_fact(
            &fs::fstat(&file).map_err(|_| io("cannot inspect compiler executable"))?,
        )?;
        let mut held = Self {
            path,
            parent,
            name,
            file,
            fact,
            digest: [0; 32],
        };
        held.digest = held.read_digest()?;
        Ok(held)
    }
    fn read_digest(&mut self) -> Result<[u8; 32]> {
        self.parent.recheck()?;
        self.file
            .seek(SeekFrom::Start(0))
            .map_err(|_| io("cannot rewind compiler executable"))?;
        let mut digest = Sha256::new();
        let mut buffer = [0u8; 16384];
        let mut count = 0u64;
        loop {
            let length = self
                .file
                .read(&mut buffer)
                .map_err(|_| io("cannot hash compiler executable"))?;
            if length == 0 {
                break;
            }
            count = count
                .checked_add(length as u64)
                .ok_or_else(|| capacity("compiler executable length overflow"))?;
            if count > self.fact.bytes {
                return Err(binding("compiler executable grew during hashing"));
            }
            digest.update(&buffer[..length]);
        }
        if count != self.fact.bytes {
            return Err(binding("compiler executable length changed during hashing"));
        }
        let held = compiler_fact(
            &fs::fstat(&self.file).map_err(|_| io("cannot recheck compiler executable"))?,
        )?;
        let named = compiler_fact(
            &fs::statat(
                self.parent.fd(),
                self.name.as_slice(),
                AtFlags::SYMLINK_NOFOLLOW,
            )
            .map_err(|_| binding("compiler executable path changed"))?,
        )?;
        if held != self.fact || named != self.fact {
            return Err(binding("compiler executable held/path identity changed"));
        }
        self.parent.recheck()?;
        Ok(digest.finalize().into())
    }
    fn recheck(&mut self) -> Result<()> {
        let current = std::env::current_exe()
            .map_err(|_| io("cannot recheck current compiler path"))?
            .canonicalize()
            .map_err(|_| binding("current compiler installation path disappeared"))?;
        if current != self.path || self.read_digest()? != self.digest {
            return Err(binding("exact compiler installation bytes changed"));
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs as disk;
    use std::os::unix::fs::{symlink, PermissionsExt};
    use std::sync::atomic::{AtomicU64, Ordering};
    static SERIAL: AtomicU64 = AtomicU64::new(0);
    struct Fixture(PathBuf);
    impl Fixture {
        fn new() -> Self {
            let path = std::env::temp_dir().join(format!(
                "spx-auth-semantic-cache-{}-{}",
                std::process::id(),
                SERIAL.fetch_add(1, Ordering::Relaxed)
            ));
            disk::create_dir(&path).unwrap();
            disk::set_permissions(&path, disk::Permissions::from_mode(0o700)).unwrap();
            Self(path.canonicalize().unwrap())
        }
    }
    impl Drop for Fixture {
        fn drop(&mut self) {
            let _ = disk::remove_dir_all(&self.0);
        }
    }
    fn code<T>(result: Result<T>, expected: &str) {
        match result {
            Ok(_) => panic!("expected {expected}"),
            Err(errors) => assert!(errors.iter().any(|e| e.code == expected), "{errors:?}"),
        }
    }
    fn file(path: &Path, bytes: &[u8]) {
        disk::write(path, bytes).unwrap();
        disk::set_permissions(path, disk::Permissions::from_mode(0o600)).unwrap();
    }
    #[test]
    fn key_initialization_is_explicit_private_and_never_adopts_or_overwrites() {
        let fixture = Fixture::new();
        disk::set_permissions(&fixture.0, disk::Permissions::from_mode(0o755)).unwrap();
        code(initialize(&fixture.0), "SPX-G308");
        assert!(disk::read_dir(&fixture.0).unwrap().next().is_none());
        disk::set_permissions(&fixture.0, disk::Permissions::from_mode(0o700)).unwrap();
        initialize(&fixture.0).unwrap();
        let key = disk::read(fixture.0.join(KEY)).unwrap();
        assert_eq!(key.len(), 32);
        assert!(key.iter().any(|b| *b != 0));
        assert_eq!(
            disk::metadata(fixture.0.join(KEY))
                .unwrap()
                .permissions()
                .mode()
                & 0o7777,
            0o600
        );
        code(initialize(&fixture.0), "SPX-G308");
        assert_eq!(disk::read(fixture.0.join(KEY)).unwrap(), key);
        assert_eq!(disk::read_dir(&fixture.0).unwrap().count(), 1);
    }
    #[test]
    fn key_links_and_unauthenticated_payload_reject_before_decode() {
        let fixture = Fixture::new();
        initialize(&fixture.0).unwrap();
        let root = Root::open(&fixture.0).unwrap();
        let inventory_before = inventory(&root, true).unwrap();
        let key = Key::open(&root, &inventory_before).unwrap();
        let compiler = Compiler::capture().unwrap();
        let (mut bytes, _) = super::super::seal(
            b"deliberately not a valid snapshot",
            &key.secret.0,
            &compiler.digest,
        )
        .unwrap();
        let index = bytes.len() - 33;
        bytes[index] ^= 1;
        let expected = super::super::hash(&bytes);
        file(
            &fixture
                .0
                .join(format!("{}.bin", digest_hex(&expected).unwrap())),
            &bytes,
        );
        // G309, rather than a snapshot grammar error, demonstrates ordering.
        code(load(&fixture.0, &expected), "SPX-G309");
        drop(key);
        drop(root);
        let outside = Fixture::new();
        let saved = disk::read(fixture.0.join(KEY)).unwrap();
        file(&outside.0.join("key"), &saved);
        disk::remove_file(fixture.0.join(KEY)).unwrap();
        symlink(outside.0.join("key"), fixture.0.join(KEY)).unwrap();
        code(load(&fixture.0, &expected), "SPX-G308");
        assert_eq!(disk::read(outside.0.join("key")).unwrap(), saved);
    }
    #[test]
    fn private_publication_does_not_replace_an_existing_envelope() {
        let fixture = Fixture::new();
        initialize(&fixture.0).unwrap();
        let root = Root::open(&fixture.0).unwrap();
        let lock = root.lock(true).unwrap();
        let initial = inventory(&root, false).unwrap();
        let name = format!("{}.bin", "1".repeat(64));
        let stage = format!(".stage-{}", "1".repeat(64));
        publish(
            &root,
            lock,
            initial,
            &name,
            &stage,
            b"inert private fixture",
            || Ok(()),
        )
        .unwrap();
        let lock = root.lock(true).unwrap();
        let initial = inventory(&root, false).unwrap();
        code(
            publish(&root, lock, initial, &name, &stage, b"replacement", || {
                Ok(())
            }),
            "SPX-G308",
        );
        assert_eq!(
            disk::read(fixture.0.join(name)).unwrap(),
            b"inert private fixture"
        );
        assert!(!fixture.0.join(stage).exists());
    }
}
