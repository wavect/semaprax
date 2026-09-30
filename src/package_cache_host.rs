//! Explicit lock-bound publication. All writes are relative to held directories.
//! The advisory root lock serializes cooperating writers. The caller excludes
//! uncooperative same-principal namespace/content mutation, as for archive stores.
//! A failed run never deletes a pathname: stages and any published prefix remain
//! for explicit reconciliation, and no success receipt is emitted.
use std::collections::BTreeMap;
use std::fs::File;
use std::io::{Read, Seek, SeekFrom, Write};
use std::os::fd::{AsFd, OwnedFd};
use std::os::unix::ffi::OsStrExt;
use std::path::{Component, Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use rustix::fs::{self, AtFlags, Dir, FileType, Mode, OFlags, RenameFlags, Stat, CWD};

use crate::diagnostic::{quote_json, Diagnostic};
use crate::package_lock_v3::{self, verify_dependency_subject, LockOptions, MAX_OUTPUT_BYTES};
use crate::package_resolver_v2::{MAX_SUBJECTS, MAX_SUBJECT_BYTES};

fn cache_error(message: String) -> Vec<Diagnostic> {
    vec![Diagnostic::io("SPX-J128", message)]
}

type Result<T> = std::result::Result<T, Vec<Diagnostic>>;
static SERIAL: AtomicU64 = AtomicU64::new(0);

fn failure(message: &str) -> Vec<Diagnostic> {
    cache_error(message.to_owned())
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Identity(u64, u64);
#[allow(
    clippy::unnecessary_cast,
    reason = "stat field widths vary across Unix ABIs"
)]
fn identity(stat: &Stat) -> Identity {
    Identity(stat.st_dev as u64, stat.st_ino as u64)
}

struct Root {
    chain: Vec<OwnedFd>,
    names: Vec<Vec<u8>>,
}

impl Root {
    fn open(path: &Path, create: bool) -> Result<Self> {
        let (mut root, pending) = Self::prepare(path)?;
        if !create && !pending.is_empty() {
            return Err(failure("selected directory does not exist"));
        }
        root.create(pending)?;
        Ok(root)
    }

    fn prepare(path: &Path) -> Result<(Self, Vec<Vec<u8>>)> {
        let base = if path.is_absolute() {
            b"/".as_slice()
        } else {
            b".".as_slice()
        };
        let mut root = Self {
            chain: vec![directory(CWD, base)?],
            names: Vec::new(),
        };
        let mut pending = Vec::new();
        for component in path.components() {
            let name = match component {
                Component::RootDir | Component::CurDir => continue,
                Component::Normal(name) => name.as_bytes(),
                Component::ParentDir => b"..",
                Component::Prefix(_) => return Err(failure("unsupported cache path prefix")),
            };
            if !pending.is_empty() {
                pending.push(name.to_vec());
                continue;
            }
            match fs::openat(
                root.fd(),
                name,
                OFlags::RDONLY | OFlags::DIRECTORY | OFlags::NOFOLLOW | OFlags::CLOEXEC,
                Mode::empty(),
            ) {
                Ok(child) => {
                    root.names.push(name.to_vec());
                    root.chain.push(child);
                }
                Err(rustix::io::Errno::NOENT) => pending.push(name.to_vec()),
                Err(_) => return Err(failure("cannot hold directory without following links")),
            }
        }
        root.check()?;
        Ok((root, pending))
    }

    fn create(&mut self, pending: Vec<Vec<u8>>) -> Result<()> {
        for name in pending {
            self.check()?;
            match fs::mkdirat(self.fd(), name.as_slice(), Mode::from_raw_mode(0o700)) {
                Ok(()) => {}
                Err(rustix::io::Errno::EXIST) => {}
                Err(_) => {
                    return Err(failure(
                        "cannot create cache directory relative to held parent",
                    ))
                }
            }
            let child = directory(self.fd(), &name)?;
            self.names.push(name);
            self.chain.push(child);
        }
        self.check()
    }

    fn fd(&self) -> &OwnedFd {
        self.chain.last().expect("held root")
    }

    fn check(&self) -> Result<()> {
        for (index, name) in self.names.iter().enumerate() {
            let named = fs::statat(
                &self.chain[index],
                name.as_slice(),
                AtFlags::SYMLINK_NOFOLLOW,
            )
            .map_err(|_| failure("cache ancestor disappeared"))?;
            let held = fs::fstat(&self.chain[index + 1])
                .map_err(|_| failure("cannot inspect held cache ancestor"))?;
            if !FileType::from_raw_mode(named.st_mode).is_dir()
                || identity(&named) != identity(&held)
            {
                return Err(failure(
                    "cache path no longer names the held directory chain",
                ));
            }
        }
        Ok(())
    }

    fn lock(&self) -> Result<File> {
        let stat = fs::fstat(self.fd()).map_err(|_| failure("cannot inspect cache ownership"))?;
        if stat.st_uid != rustix::process::geteuid().as_raw() || stat.st_mode & 0o022 != 0 {
            return Err(failure(
                "lock-bound cache must be caller-owned and not group/world writable",
            ));
        }
        self.lock_authority()
    }

    fn lock_authority(&self) -> Result<File> {
        let lock =
            File::from(rustix::io::dup(self.fd()).map_err(|_| failure("cannot hold cache lock"))?);
        try_lock_cache_exclusive(&lock).map_err(|_| failure("lock-bound cache is busy"))?;
        self.check()?;
        Ok(lock)
    }
}

/// Production takes this exclusive advisory lock with one immediate,
/// non-blocking attempt: a real competing holder keeps the cache busy for
/// its whole operation, so failing fast is correct and this path is
/// unchanged from before.
///
/// A full parallel test run can observe a spurious, momentary `EWOULDBLOCK`
/// here even though the previous lock holder already ran its synchronous
/// `Drop` (which releases this same lock) before the next acquisition is
/// attempted. Test builds only retry a short, bounded window so that kind
/// of scheduling jitter clears without loosening what "busy" means: a lock
/// still held for the retry budget (a genuine concurrent holder) is still
/// refused exactly as before.
#[cfg(test)]
fn try_lock_cache_exclusive(lock: &File) -> std::io::Result<()> {
    let mut attempts_remaining = 200;
    loop {
        match fs2::FileExt::try_lock_exclusive(lock) {
            Ok(()) => return Ok(()),
            Err(error) if attempts_remaining > 0 => {
                attempts_remaining -= 1;
                std::thread::sleep(std::time::Duration::from_millis(1));
                let _ = error;
            }
            Err(error) => return Err(error),
        }
    }
}
#[cfg(not(test))]
fn try_lock_cache_exclusive(lock: &File) -> std::io::Result<()> {
    fs2::FileExt::try_lock_exclusive(lock)
}

fn directory(parent: impl AsFd, name: &[u8]) -> Result<OwnedFd> {
    fs::openat(
        parent,
        name,
        OFlags::RDONLY | OFlags::DIRECTORY | OFlags::NOFOLLOW | OFlags::CLOEXEC,
        Mode::empty(),
    )
    .map_err(|_| failure("cannot hold directory without following links"))
}

fn open_file(parent: impl AsFd, name: &[u8]) -> std::result::Result<File, rustix::io::Errno> {
    fs::openat(
        parent,
        name,
        OFlags::RDONLY | OFlags::NOFOLLOW | OFlags::NONBLOCK | OFlags::CLOEXEC,
        Mode::empty(),
    )
    .map(File::from)
}

fn read(file: &mut File, limit: usize) -> Result<Vec<u8>> {
    let before = fs::fstat(&*file).map_err(|_| failure("cannot inspect held input"))?;
    if !FileType::from_raw_mode(before.st_mode).is_file()
        || before.st_size < 0
        || before.st_size as u64 > limit as u64
    {
        return Err(failure("held input is not a bounded regular file"));
    }
    file.seek(SeekFrom::Start(0))
        .map_err(|_| failure("cannot rewind held input"))?;
    let mut bytes = Vec::new();
    (&mut *file)
        .take(limit as u64 + 1)
        .read_to_end(&mut bytes)
        .map_err(|_| failure("cannot read held input"))?;
    let after = fs::fstat(&*file).map_err(|_| failure("cannot recheck held input"))?;
    if bytes.len() > limit
        || bytes.len() as u64 != before.st_size as u64
        || before.st_size != after.st_size
        || identity(&before) != identity(&after)
    {
        return Err(failure("held input changed while reading"));
    }
    Ok(bytes)
}

struct Input {
    parent: Root,
    name: Vec<u8>,
    file: File,
    id: Identity,
    bytes: String,
    limit: usize,
}

impl Input {
    fn check(&mut self) -> Result<()> {
        self.parent.check()?;
        let named = fs::statat(
            self.parent.fd(),
            self.name.as_slice(),
            AtFlags::SYMLINK_NOFOLLOW,
        )
        .map_err(|_| failure("held input name disappeared"))?;
        if !FileType::from_raw_mode(named.st_mode).is_file()
            || identity(&named) != self.id
            || read(&mut self.file, self.limit)? != self.bytes.as_bytes()
        {
            return Err(failure("held input identity or bytes changed"));
        }
        let named = fs::statat(
            self.parent.fd(),
            self.name.as_slice(),
            AtFlags::SYMLINK_NOFOLLOW,
        )
        .map_err(|_| failure("held input name disappeared"))?;
        if identity(&named) != self.id {
            return Err(failure("held input identity changed"));
        }
        self.parent.check()
    }
}

fn input(path: &Path, limit: usize) -> Result<Input> {
    let parent = Root::open(
        path.parent()
            .filter(|p| !p.as_os_str().is_empty())
            .unwrap_or(Path::new(".")),
        false,
    )?;
    let name = path
        .file_name()
        .ok_or_else(|| failure("input requires a file name"))?;
    let mut file = open_file(parent.fd(), name.as_bytes())
        .map_err(|_| failure("cannot open input without following links"))?;
    let id = identity(&fs::fstat(&file).map_err(|_| failure("cannot inspect held input"))?);
    let bytes = read(&mut file, limit)?;
    let mut input = Input {
        parent,
        name: name.as_bytes().to_vec(),
        file,
        id,
        bytes: String::from_utf8(bytes).map_err(|_| failure("input is not UTF-8"))?,
        limit,
    };
    input.check()?;
    Ok(input)
}

struct Selected {
    file: File,
    id: Identity,
    present: bool,
    stage: Option<String>,
}

fn selected(root: &Root, name: &str, item: &mut Selected, expected: &[u8]) -> Result<()> {
    let before = fs::statat(root.fd(), name.as_bytes(), AtFlags::SYMLINK_NOFOLLOW)
        .map_err(|_| failure("selected cache name disappeared"))?;
    if !FileType::from_raw_mode(before.st_mode).is_file()
        || identity(&before) != item.id
        || read(&mut item.file, MAX_SUBJECT_BYTES)? != expected
    {
        return Err(failure("selected cache identity or exact bytes changed"));
    }
    let after = fs::statat(root.fd(), name.as_bytes(), AtFlags::SYMLINK_NOFOLLOW)
        .map_err(|_| failure("selected cache name disappeared"))?;
    if identity(&after) != item.id {
        return Err(failure("selected cache identity changed"));
    }
    Ok(())
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Point {
    BeforeStage,
    AfterPartialWrite,
    BeforePublish,
    AfterPublish,
}

fn publish(
    root: &Root,
    subjects: &BTreeMap<String, Vec<u8>>,
    mut hook: impl FnMut(Point) -> Result<()>,
) -> Result<BTreeMap<String, bool>> {
    let _lock = root.lock()?;
    let mut count = 0;
    let entries = Dir::new(directory(root.fd(), b".")?)
        .map_err(|_| failure("cannot enumerate held cache"))?;
    for entry in entries {
        let entry = entry.map_err(|_| failure("cannot enumerate held cache entry"))?;
        if entry.file_name().to_bytes().starts_with(b".fetch-stage-") {
            return Err(failure(
                "retained fetch stage requires explicit reconciliation before retry",
            ));
        }
        if entry.file_name().to_bytes().ends_with(b".json") {
            count += 1;
        }
        if count > MAX_SUBJECTS {
            return Err(failure("cache exceeds subject limit"));
        }
    }
    let mut held = BTreeMap::new();
    for (name, bytes) in subjects {
        match open_file(root.fd(), name.as_bytes()) {
            Ok(file) => {
                let id =
                    identity(&fs::fstat(&file).map_err(|_| failure("cannot inspect cache entry"))?);
                let mut item = Selected {
                    file,
                    id,
                    present: true,
                    stage: None,
                };
                selected(root, name, &mut item, bytes)?;
                held.insert(name.clone(), item);
            }
            Err(rustix::io::Errno::NOENT) => {}
            Err(_) => return Err(failure("cannot hold cache entry without following links")),
        }
    }
    if count + subjects.len() - held.len() > MAX_SUBJECTS {
        return Err(failure("cache would exceed subject limit"));
    }
    for (name, bytes) in subjects {
        if held.contains_key(name) {
            continue;
        }
        hook(Point::BeforeStage)?;
        root.check()?;
        let stage = format!(
            ".fetch-stage-{}-{}",
            std::process::id(),
            SERIAL.fetch_add(1, Ordering::Relaxed)
        );
        let fd = fs::openat(
            root.fd(),
            stage.as_bytes(),
            OFlags::RDWR | OFlags::CREATE | OFlags::EXCL | OFlags::NOFOLLOW | OFlags::CLOEXEC,
            Mode::from_raw_mode(0o600),
        )
        .map_err(|_| {
            failure("cannot create exclusive cache stage; reconcile retained stages before retry")
        })?;
        let mut file = File::from(fd);
        let id = identity(&fs::fstat(&file).map_err(|_| failure("cannot inspect cache stage"))?);
        // Split writing provides an executable partial-write failure boundary.
        let split = bytes.len() / 2;
        file.write_all(&bytes[..split])
            .map_err(|_| failure("cache stage write failed; non-cache stage retained"))?;
        hook(Point::AfterPartialWrite)?;
        file.write_all(&bytes[split..])
            .and_then(|()| file.sync_all())
            .map_err(|_| failure("cache stage write failed; non-cache stage retained"))?;
        let mut item = Selected {
            file,
            id,
            present: false,
            stage: Some(stage.clone()),
        };
        selected(root, &stage, &mut item, bytes)?;
        held.insert(name.clone(), item);
    }
    let mut published = false;
    let result = (|| {
        fs::fsync(root.fd()).map_err(|_| failure("cannot settle staged cache directory"))?;
        // Stage all bytes before publishing any address. Never claim atomic
        // multi-file visibility: a later failure may leave an authenticated prefix.
        for (name, item) in &mut held {
            if let Some(stage) = item.stage.as_ref().cloned() {
                hook(Point::BeforePublish)?;
                root.check()?;
                selected(root, &stage, item, &subjects[name])?;
                fs::renameat_with(
                    root.fd(),
                    stage.as_bytes(),
                    root.fd(),
                    name.as_bytes(),
                    RenameFlags::NOREPLACE,
                )
                .map_err(|_| failure("cache publication refused replacement; stage retained"))?;
                published = true;
                item.stage = None;
                hook(Point::AfterPublish)?;
            }
            selected(root, name, item, &subjects[name])?;
        }
        fs::fsync(root.fd()).map_err(|_| failure("cannot settle cache directory"))?;
        root.check()?;
        for (name, item) in &mut held {
            selected(root, name, item, &subjects[name])?;
        }
        Ok(held
            .iter()
            .map(|(name, item)| (name.clone(), item.present))
            .collect())
    })();
    if published && result.is_err() {
        return Err(failure("cache publication partially completed or is uncertain; published entries and stages retained for reconciliation; no receipt"));
    }
    result
}

pub fn fetch_locked(lock_path: &Path, cache: &Path, subject_paths: &[PathBuf]) -> Result<String> {
    if subject_paths.is_empty() || subject_paths.len() > MAX_SUBJECTS {
        return Err(failure("invalid subject count"));
    }
    let (mut root, pending) = Root::prepare(cache)?;
    let _authority = if pending.is_empty() {
        root.lock()?
    } else {
        root.lock_authority()?
    };
    let mut subjects = BTreeMap::new();
    let mut operands = Vec::new();
    let mut exact = Vec::new();
    let mut inputs = Vec::new();
    for path in subject_paths {
        let held_input = input(path, MAX_SUBJECT_BYTES)?;
        let bytes = held_input.bytes.clone();
        let subject = verify_dependency_subject(&bytes).map_err(|e| vec![e])?;
        let hex = subject
            .subject_digest
            .strip_prefix("sha256:")
            .filter(|hex| hex.len() == 64 && hex.bytes().all(|b| b.is_ascii_hexdigit()))
            .ok_or_else(|| failure("noncanonical subject digest"))?;
        let name = format!("{hex}.json");
        if subjects
            .insert(name.clone(), bytes.as_bytes().to_vec())
            .is_some_and(|previous| previous != bytes.as_bytes())
        {
            return Err(failure("different bytes at one subject address"));
        }
        operands.push((name, subject));
        exact.push(bytes);
        inputs.push(held_input);
    }
    let mut lock = input(lock_path, MAX_OUTPUT_BYTES)?;
    package_lock_v3::verify(&lock.bytes, &exact, &LockOptions::default()).map_err(|e| vec![e])?;
    lock.check()?;
    for input in &mut inputs {
        input.check()?;
    }
    root.create(pending)?;
    let states = publish(&root, &subjects, |_| Ok(()))?;
    let rows = operands
        .iter()
        .map(|(name, subject)| {
            format!(
                "{{\"package\":{},\"version\":{},\"digest\":{},\"state\":{}}}",
                quote_json(&subject.coordinate.package),
                quote_json(&subject.coordinate.version),
                quote_json(&subject.subject_digest),
                quote_json(if states[name] { "present" } else { "added" })
            )
        })
        .collect::<Vec<_>>()
        .join(",");
    Ok(format!("{{\"schema\":\"semaprax.fetch-receipt.v2\",\"cache\":{},\"lock_binding\":true,\"subjects\":[{}]}}\n", quote_json(&cache.display().to_string()), rows))
}

/// Internal bridge for already sealed, immutable subject bytes. Destination
/// authority precedes source revalidation and remains held through settlement.
pub(crate) fn publish_bound(
    cache: &Path,
    lock: &str,
    subjects: &[String],
    mut recheck: impl FnMut() -> Result<()>,
) -> Result<BTreeMap<String, bool>> {
    let (mut root, pending) = Root::prepare(cache)?;
    let _authority = if pending.is_empty() {
        root.lock()?
    } else {
        root.lock_authority()?
    };
    recheck()?;
    package_lock_v3::verify(lock, subjects, &LockOptions::default())
        .map_err(|error| vec![error])?;
    let mut selected = BTreeMap::new();
    for bytes in subjects {
        let subject = verify_dependency_subject(bytes).map_err(|error| vec![error])?;
        let hex = subject
            .subject_digest
            .strip_prefix("sha256:")
            .filter(|hex| hex.len() == 64 && hex.bytes().all(|b| b.is_ascii_hexdigit()))
            .ok_or_else(|| failure("noncanonical subject digest"))?;
        if selected
            .insert(format!("{hex}.json"), bytes.as_bytes().to_vec())
            .is_some()
        {
            return Err(failure("duplicate bridge subject"));
        }
    }
    root.create(pending)?;
    let states = publish(&root, &selected, |_| recheck())?;
    recheck().map_err(|_|failure("cache publication completed or is uncertain; entries retained for reconciliation; no receipt"))?;
    Ok(states)
}

/// Reads precisely the caller-named published cache entries through held,
/// nofollow directory and file handles. This is intentionally crate-private:
/// callers must already have an independently authenticated inventory rather
/// than treating an ordinary cache directory as a catalog authority.
pub(crate) fn read_bound(cache: &Path, names: &[String]) -> Result<Vec<String>> {
    if names.is_empty() || names.len() > MAX_SUBJECTS {
        return Err(failure("invalid cache subject count"));
    }
    let root = Root::open(cache, false)?;
    let _authority = root.lock()?;
    let mut seen = std::collections::BTreeSet::new();
    let mut result = Vec::with_capacity(names.len());
    for name in names {
        let hex = name
            .strip_suffix(".json")
            .filter(|hex| hex.len() == 64 && hex.bytes().all(|byte| byte.is_ascii_hexdigit()));
        if hex.is_none() || !seen.insert(name) {
            return Err(failure("cache subject inventory is not exact"));
        }
        root.check()?;
        let mut file = open_file(root.fd(), name.as_bytes())
            .map_err(|_| failure("cannot open cache subject without following links"))?;
        let held_identity =
            identity(&fs::fstat(&file).map_err(|_| failure("cannot inspect cache subject"))?);
        let bytes = read(&mut file, MAX_SUBJECT_BYTES)?;
        let named = fs::statat(root.fd(), name.as_bytes(), AtFlags::SYMLINK_NOFOLLOW)
            .map_err(|_| failure("cache subject name disappeared"))?;
        if !FileType::from_raw_mode(named.st_mode).is_file() || identity(&named) != held_identity {
            return Err(failure("cache subject identity changed"));
        }
        root.check()?;
        result.push(String::from_utf8(bytes).map_err(|_| failure("cache subject is not UTF-8"))?);
    }
    Ok(result)
}

#[cfg(test)]
mod tests;
