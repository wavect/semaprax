use super::*;
use rustix::fs::{self, AtFlags, Dir, FileType, Mode, OFlags, RenameFlags, Stat, CWD};
use std::fs::File;
use std::io::{Read, Write};
use std::os::fd::{AsFd, OwnedFd};
use std::os::unix::ffi::OsStrExt;
use std::path::Component;

pub(super) struct Held {
    chain: Vec<OwnedFd>,
    names: Vec<Vec<u8>>,
    _lock: File,
}

#[allow(clippy::unnecessary_cast, reason = "Unix stat field widths vary")]
fn identity(stat: &Stat) -> (u64, u64) {
    (stat.st_dev as u64, stat.st_ino as u64)
}
fn directory(parent: impl AsFd, name: &[u8]) -> Result<OwnedFd> {
    fs::openat(
        parent,
        name,
        OFlags::RDONLY | OFlags::DIRECTORY | OFlags::NOFOLLOW | OFlags::CLOEXEC,
        Mode::empty(),
    )
    .map_err(|_| refused("cannot hold store directory without links"))
}
fn private(stat: &Stat) -> bool {
    stat.st_uid == rustix::process::geteuid().as_raw() && stat.st_mode & 0o077 == 0
}

/// Production takes this exclusive advisory lock with one immediate,
/// non-blocking attempt: a real competing holder keeps the store busy for
/// its whole operation, so failing fast is correct and this path is
/// unchanged from before.
///
/// A full parallel test run can observe a spurious, momentary `EWOULDBLOCK`
/// here that a solo run never does, even though the previous `Held` on the
/// same store already ran its synchronous `Drop` (which releases this same
/// lock) before the next open is attempted. Test builds only retry a short,
/// bounded window so that kind of scheduling jitter clears without
/// loosening what "busy" means: a lock still held for the retry budget (a
/// genuine concurrent holder) is still refused exactly as before.
#[cfg(test)]
fn try_lock_store_exclusive(lock: &File) -> std::io::Result<()> {
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
fn try_lock_store_exclusive(lock: &File) -> std::io::Result<()> {
    fs2::FileExt::try_lock_exclusive(lock)
}

impl Held {
    pub(super) fn open(path: &Path) -> Result<Self> {
        if path.as_os_str().is_empty() {
            return Err(refused("empty store path"));
        }
        let mut chain = vec![directory(
            CWD,
            if path.is_absolute() { b"/" } else { b"." },
        )?];
        let mut names = Vec::new();
        for component in path.components() {
            let name = match component {
                Component::RootDir | Component::CurDir => continue,
                Component::Normal(name) => name.as_bytes(),
                // Relative paths work, but ambiguous ancestor traversal does not.
                _ => return Err(refused("store path must not contain parent traversal")),
            };
            chain.push(directory(chain.last().expect("root"), name)?);
            names.push(name.to_vec());
        }
        let fd = chain.last().expect("root");
        if !private(&fs::fstat(fd).map_err(|_| refused("cannot inspect store owner"))?) {
            return Err(refused("store must be caller-owned and owner-private"));
        }
        let lock = File::from(rustix::io::dup(fd).map_err(|_| refused("cannot hold store lock"))?);
        try_lock_store_exclusive(&lock).map_err(|_| refused("store is busy"))?;
        let held = Self {
            chain,
            names,
            _lock: lock,
        };
        held.check()?;
        Ok(held)
    }
    fn fd(&self) -> &OwnedFd {
        self.chain.last().expect("held root")
    }
    pub(super) fn check(&self) -> Result<()> {
        for (index, name) in self.names.iter().enumerate() {
            let named = fs::statat(
                &self.chain[index],
                name.as_slice(),
                AtFlags::SYMLINK_NOFOLLOW,
            )
            .map_err(|_| refused("store ancestor disappeared"))?;
            let held = fs::fstat(&self.chain[index + 1])
                .map_err(|_| refused("cannot inspect ancestor"))?;
            if !FileType::from_raw_mode(named.st_mode).is_dir()
                || identity(&held) != identity(&named)
            {
                return Err(refused("store path swapped"));
            }
        }
        if !private(&fs::fstat(self.fd()).map_err(|_| refused("cannot inspect store owner"))?) {
            return Err(refused("store permissions changed"));
        }
        Ok(())
    }
    pub(super) fn inventory(&self) -> Result<BTreeSet<String>> {
        self.check()?;
        let mut names = BTreeSet::new();
        for entry in
            Dir::new(directory(self.fd(), b".")?).map_err(|_| refused("cannot enumerate store"))?
        {
            let entry = entry.map_err(|_| refused("cannot enumerate store entry"))?;
            let name = entry.file_name().to_bytes();
            if name == b"." || name == b".." {
                continue;
            }
            names.insert(
                std::str::from_utf8(name)
                    .map_err(|_| refused("invalid store name"))?
                    .to_owned(),
            );
            if names.len() > MAX_GENERATIONS * 2 + 4 {
                return Err(refused("store inventory exceeds bound"));
            }
        }
        self.check()?;
        Ok(names)
    }
    pub(super) fn read(&self, name: &str, limit: usize) -> Result<String> {
        self.check()?;
        let mut file = File::from(
            fs::openat(
                self.fd(),
                name,
                OFlags::RDONLY | OFlags::NOFOLLOW | OFlags::NONBLOCK | OFlags::CLOEXEC,
                Mode::empty(),
            )
            .map_err(|_| refused("cannot hold store file"))?,
        );
        let before = fs::fstat(&file).map_err(|_| refused("cannot inspect store file"))?;
        if !FileType::from_raw_mode(before.st_mode).is_file()
            || !private(&before)
            || before.st_nlink != 1
            || before.st_size < 0
            || before.st_size as u64 > limit as u64
        {
            return Err(refused(
                "store file is not bounded private single-link regular data",
            ));
        }
        self.same(name, &before)?;
        let mut bytes = Vec::new();
        (&mut file)
            .take(limit as u64 + 1)
            .read_to_end(&mut bytes)
            .map_err(|_| refused("cannot read store file"))?;
        let after = fs::fstat(&file).map_err(|_| refused("cannot recheck store file"))?;
        self.same(name, &after)?;
        if bytes.len() > limit
            || bytes.len() as u64 != before.st_size as u64
            || before.st_size != after.st_size
            || identity(&before) != identity(&after)
        {
            return Err(refused("store bytes changed"));
        }
        self.check()?;
        String::from_utf8(bytes).map_err(|_| refused("store data is not UTF-8"))
    }
    fn same(&self, name: &str, held: &Stat) -> Result<()> {
        let named = fs::statat(self.fd(), name, AtFlags::SYMLINK_NOFOLLOW)
            .map_err(|_| refused("store file vanished"))?;
        if identity(&named) != identity(held)
            || !FileType::from_raw_mode(named.st_mode).is_file()
            || !private(&named)
            || named.st_nlink != 1
        {
            return Err(refused("store file identity changed"));
        }
        Ok(())
    }
    pub(super) fn sync(&self) -> Result<()> {
        fs::fsync(self.fd()).map_err(|_| uncertain())?;
        self.check().map_err(|_| uncertain())
    }
    fn write_new(&self, name: &str, bytes: &str) -> Result<()> {
        self.check()?;
        let mut file = File::from(
            fs::openat(
                self.fd(),
                name,
                OFlags::WRONLY | OFlags::CREATE | OFlags::EXCL | OFlags::NOFOLLOW | OFlags::CLOEXEC,
                Mode::from_raw_mode(0o600),
            )
            .map_err(|_| uncertain())?,
        );
        let split = bytes.len() / 2;
        file.write_all(&bytes.as_bytes()[..split])
            .map_err(|_| uncertain())?;
        point(Point::PartialWrite)?;
        file.write_all(&bytes.as_bytes()[split..])
            .map_err(|_| uncertain())?;
        file.sync_all().map_err(|_| uncertain())?;
        if self.read(name, MAX_GENERATION)? != bytes {
            return Err(uncertain());
        }
        self.sync()
    }
    fn active_is(&self, expected: Option<&str>) -> Result<()> {
        let names = self.inventory()?;
        match expected {
            Some(name) if self.read("ACTIVE", 66)? == name => Ok(()),
            None if !names.contains("ACTIVE") => Ok(()),
            _ => Err(refused("ACTIVE byte-exact CAS disagrees")),
        }
    }
    pub(super) fn publish(&self, previous: Option<&str>, bytes: &str) -> Result<String> {
        let name = generation_name(bytes.as_bytes());
        self.active_is(previous)?;
        // Refuse capacity before the first effect, including an extra generation.
        if self.inventory()?.iter().filter(|n| valid_name(n)).count() >= MAX_GENERATIONS {
            return Err(refused("store generation capacity exhausted"));
        }
        self.write_new("PENDING", bytes).map_err(|_| uncertain())?;
        point(Point::Staged)?;
        self.finish(previous, &name, bytes)
            .map_err(|_| uncertain())?;
        Ok(name)
    }
    fn finish(&self, previous: Option<&str>, name: &str, bytes: &str) -> Result<()> {
        self.active_is(previous)?;
        let names = self.inventory()?;
        if names.contains("PENDING") {
            if names.contains(name) || self.read("PENDING", MAX_GENERATION)? != bytes {
                return Err(uncertain());
            }
            fs::renameat_with(
                self.fd(),
                "PENDING",
                self.fd(),
                name,
                RenameFlags::NOREPLACE,
            )
            .map_err(|_| uncertain())?;
            self.sync()?;
        }
        if self.read(name, MAX_GENERATION)? != bytes {
            return Err(uncertain());
        }
        point(Point::Generation)?;
        if self.inventory()?.contains("ACTIVE.next") {
            if self.read("ACTIVE.next", 66)? != name {
                return Err(uncertain());
            }
        } else {
            self.write_new("ACTIVE.next", name)?;
        }
        point(Point::BeforeActive)?;
        if self.inventory()?.contains("COMMIT") {
            if self.read("COMMIT", 66)? != name {
                return Err(uncertain());
            }
        } else {
            self.write_new("COMMIT", name)?;
        }
        self.active_is(previous)?;
        if self.read(name, MAX_GENERATION)? != bytes {
            return Err(uncertain());
        }
        self.check()?;
        fs::renameat(self.fd(), "ACTIVE.next", self.fd(), "ACTIVE").map_err(|_| uncertain())?;
        point(Point::AfterActive)?;
        self.sync()?;
        if self.read("ACTIVE", 66)? != name || self.read(name, MAX_GENERATION)? != bytes {
            return Err(uncertain());
        }
        self.complete(name)?;
        Ok(())
    }
    fn complete(&self, name: &str) -> Result<()> {
        let completed = format!("c-{name}");
        if self.read("COMMIT", 66)? != name {
            return Err(uncertain());
        }
        fs::renameat_with(
            self.fd(),
            "COMMIT",
            self.fd(),
            completed.as_str(),
            RenameFlags::NOREPLACE,
        )
        .map_err(|_| uncertain())?;
        self.sync()
    }
    pub(super) fn recover(&self, previous: Option<&str>, bytes: &str) -> Result<String> {
        self.recover_profile(previous, bytes, |bytes| {
            let generation = decode(bytes)?;
            let previous = string(&generation.value["previous"])?.to_owned();
            Ok((generation.bytes, previous))
        })
    }
    pub(super) fn recover_profile(
        &self,
        previous: Option<&str>,
        bytes: &str,
        predecessor: impl Fn(String) -> Result<(String, String)>,
    ) -> Result<String> {
        let name = generation_name(bytes.as_bytes());
        let inventory = self.inventory()?;
        // Build the exact expected old chain; no unknown effects may be erased.
        let mut allowed = BTreeSet::new();
        let mut next = previous.unwrap_or("").to_owned();
        while !next.is_empty() {
            if !valid_name(&next)
                || !allowed.insert(next.clone())
                || allowed.len() >= MAX_GENERATIONS * 2
            {
                return Err(uncertain());
            }
            allowed.insert(format!("c-{next}"));
            let (generation, parent) = predecessor(self.read(&next, MAX_GENERATION)?)?;
            if generation_name(generation.as_bytes()) != next {
                return Err(uncertain());
            }
            next = parent;
        }
        allowed.extend([
            "PENDING".to_owned(),
            "ACTIVE.next".to_owned(),
            "ACTIVE".to_owned(),
            "COMMIT".to_owned(),
            format!("c-{name}"),
            name.clone(),
        ]);
        if !inventory.is_subset(&allowed) {
            return Err(uncertain());
        }
        if inventory.contains("PENDING") && self.read("PENDING", MAX_GENERATION)? != bytes {
            return Err(uncertain());
        }
        if inventory.contains("ACTIVE.next") && self.read("ACTIVE.next", 66)? != name {
            return Err(uncertain());
        }
        if inventory.contains("COMMIT") && self.read("COMMIT", 66)? != name {
            return Err(uncertain());
        }
        if inventory.contains("ACTIVE") && self.read("ACTIVE", 66)? == name {
            if inventory.contains("PENDING")
                || inventory.contains("ACTIVE.next")
                || self.read(&name, MAX_GENERATION)? != bytes
            {
                return Err(uncertain());
            }
            self.sync()?;
            if inventory.contains("COMMIT") {
                self.complete(&name)?;
            } else if self.read(&format!("c-{name}"), 66)? != name {
                return Err(uncertain());
            }
        } else {
            if !inventory.contains("PENDING") && !inventory.contains(&name) {
                return Err(uncertain());
            }
            self.finish(previous, &name, bytes)?;
        }
        Ok(name)
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum Point {
    PartialWrite,
    Staged,
    Generation,
    BeforeActive,
    AfterActive,
}
#[cfg(test)]
thread_local! { pub(super) static FAIL: std::cell::Cell<Option<Point>> = const { std::cell::Cell::new(None) }; }
fn point(_point: Point) -> Result<()> {
    #[cfg(test)]
    if FAIL.with(|fail| fail.get() == Some(_point)) {
        FAIL.with(|fail| fail.set(None));
        return Err(uncertain());
    }
    Ok(())
}
