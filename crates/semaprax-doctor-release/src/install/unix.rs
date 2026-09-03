use std::collections::BTreeMap;
use std::io::{Read, Write};
use std::os::fd::{AsFd, OwnedFd};
use std::os::unix::ffi::OsStrExt as _;
use std::path::{Component, Path, PathBuf};

use rustix::fs::{self, AtFlags, Dir, FileType, Mode, OFlags, RenameFlags, Stat, CWD};
use sha2::{Digest as _, Sha256};

use super::model::{
    GenerationId, InstallReceipt, RecoveryReceipt, StoreExpectation, MAX_GENERATIONS_LIMIT,
};
use crate::{
    directory::verify_release_bytes, ReleaseExpectation, BUNDLE_FILE, CAPSULE_FILE, COLLECTOR_FILE,
    LAUNCHER_FILE, MANIFEST_FILE, MANIFEST_SIGNATURE_FILE, PROVISIONER_FILE, REQUEST_FILE,
    WORKER_FILE,
};

const ACTIVE: &str = "ACTIVE";
const ACTIVE_STAGE: &str = ".ACTIVE.stage";
const GENERATION_PREFIX: &str = "generation-";
const STAGE_PREFIX: &str = ".stage-";
const MAX_PATH_BYTES: usize = 4096;
const MAX_PATH_DEPTH: usize = 64;
const MAX_FILE_BYTES: usize = 512 * 1024 * 1024;
const INVENTORY: [&str; 9] = [
    BUNDLE_FILE,
    COLLECTOR_FILE,
    LAUNCHER_FILE,
    PROVISIONER_FILE,
    MANIFEST_FILE,
    MANIFEST_SIGNATURE_FILE,
    CAPSULE_FILE,
    REQUEST_FILE,
    WORKER_FILE,
];
type LoadedRelease = Vec<(&'static str, Vec<u8>, bool)>;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum EffectPoint {
    BeforeStageCreate,
    AfterMemberFsync,
    AfterStageFsync,
    AfterGenerationRename,
    AfterGenerationRootFsync,
    AfterActiveStageFsync,
    AfterActiveRename,
    AfterActiveRootFsync,
    BeforeRecoveryEffect,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct Identity {
    device: u64,
    inode: u64,
    mode: u32,
    uid: u32,
}

impl Identity {
    #[allow(clippy::unnecessary_cast, reason = "stat widths vary across Unix ABIs")]
    fn from_stat(value: &Stat) -> Self {
        Self {
            device: value.st_dev as u64,
            inode: value.st_ino as u64,
            mode: value.st_mode as u32,
            uid: value.st_uid as u32,
        }
    }
}

pub(super) struct HeldRoot {
    path: PathBuf,
    chain: Vec<(OwnedFd, Identity)>,
    names: Vec<Vec<u8>>,
}

pub struct DoctorStore {
    root: HeldRoot,
    expectation: StoreExpectation,
}

impl DoctorStore {
    pub fn root(&self) -> &Path {
        &self.root.path
    }
}

impl HeldRoot {
    fn open(path: &Path, private: bool) -> Result<Self, String> {
        let raw = path.as_os_str().as_bytes();
        if !path.is_absolute() || raw.len() > MAX_PATH_BYTES {
            return Err("doctor store path must be bounded, absolute and normalized".into());
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
                _ => return Err("doctor store path contains a noncanonical component".into()),
            }
        }
        if names.is_empty() || normalized != raw || names.len() > MAX_PATH_DEPTH {
            return Err("doctor store path is not an exact bounded absolute path".into());
        }
        let first = open_dir(CWD, b"/")?;
        let first_identity = identity(&first)?;
        let mut chain = vec![(first, first_identity)];
        for name in &names {
            let child = open_dir(&chain.last().expect("held root").0, name)?;
            let fact = identity(&child)?;
            chain.push((child, fact));
        }
        let root = Self {
            path: path.to_owned(),
            chain,
            names,
        };
        if private {
            let fact = root.chain.last().expect("held root").1;
            if fact.uid != rustix::process::geteuid().as_raw() || fact.mode & 0o7777 != 0o700 {
                return Err("doctor store root must be current-euid-owned exact 0700".into());
            }
        }
        root.recheck_chain()?;
        Ok(root)
    }

    fn fd(&self) -> &OwnedFd {
        &self.chain.last().expect("held root").0
    }

    fn recheck_chain(&self) -> Result<(), String> {
        for (index, (held, expected)) in self.chain.iter().enumerate() {
            if identity(held)? != *expected {
                return Err("held doctor directory identity or mode changed".into());
            }
            if index > 0 {
                let stat = fs::statat(
                    &self.chain[index - 1].0,
                    self.names[index - 1].as_slice(),
                    AtFlags::SYMLINK_NOFOLLOW,
                )
                .map_err(|_| "doctor directory path disappeared")?;
                if Identity::from_stat(&stat) != *expected {
                    return Err("doctor directory path and held identity disagree".into());
                }
            }
        }
        Ok(())
    }

    fn recheck(&self, private: bool) -> Result<(), String> {
        self.recheck_chain()?;
        let rebound = Self::open(&self.path, private)?;
        if self
            .chain
            .iter()
            .map(|row| row.1)
            .ne(rebound.chain.iter().map(|row| row.1))
        {
            return Err("doctor directory absolute path no longer names held authority".into());
        }
        Ok(())
    }

    fn lock(&self) -> Result<Lock, String> {
        let file = std::fs::File::from(
            rustix::io::dup(self.fd()).map_err(|_| "cannot duplicate doctor store root")?,
        );
        fs2::FileExt::try_lock_exclusive(&file).map_err(|_| "doctor store is busy")?;
        Ok(Lock(file))
    }
}

struct Lock(std::fs::File);
impl Drop for Lock {
    fn drop(&mut self) {
        let _ = fs2::FileExt::unlock(&self.0);
    }
}

fn open_dir(parent: impl AsFd, name: &[u8]) -> Result<OwnedFd, String> {
    fs::openat(
        parent,
        name,
        OFlags::RDONLY | OFlags::DIRECTORY | OFlags::NOFOLLOW | OFlags::CLOEXEC,
        Mode::empty(),
    )
    .map_err(|_| "cannot hold doctor directory without following links".into())
}

fn identity(fd: impl AsFd) -> Result<Identity, String> {
    fs::fstat(fd)
        .map(|stat| Identity::from_stat(&stat))
        .map_err(|_| "cannot inspect held doctor object".into())
}

#[allow(clippy::unnecessary_cast, reason = "stat widths vary across Unix ABIs")]
fn link_count(stat: &Stat) -> u64 {
    stat.st_nlink as u64
}

fn canonical_id(raw: &str) -> bool {
    raw.len() == 64
        && raw
            .as_bytes()
            .iter()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(byte))
}

fn generation_name(id: &GenerationId) -> String {
    format!("{GENERATION_PREFIX}{}", id.0)
}

fn stage_name(id: &GenerationId) -> String {
    format!("{STAGE_PREFIX}{}", id.0)
}

fn parse_entry(name: &str) -> Result<EntryKind, String> {
    if name == ACTIVE {
        Ok(EntryKind::Active)
    } else if name == ACTIVE_STAGE {
        Ok(EntryKind::ActiveStage)
    } else if let Some(id) = name.strip_prefix(GENERATION_PREFIX) {
        canonical_id(id)
            .then_some(EntryKind::Generation)
            .ok_or_else(|| "doctor store generation name is not canonical".into())
    } else if let Some(id) = name.strip_prefix(STAGE_PREFIX) {
        canonical_id(id)
            .then(|| EntryKind::Stage(GenerationId(id.to_owned())))
            .ok_or_else(|| "doctor store stage name is not canonical".into())
    } else {
        Err("doctor store contains a foreign entry".into())
    }
}

enum EntryKind {
    Active,
    ActiveStage,
    Generation,
    Stage(GenerationId),
}

fn inventory(root: &HeldRoot) -> Result<Vec<(String, EntryKind)>, String> {
    let entries =
        Dir::new(open_dir(root.fd(), b".")?).map_err(|_| "cannot enumerate doctor store")?;
    let mut result = Vec::new();
    for entry in entries {
        let entry = entry.map_err(|_| "cannot read doctor store inventory")?;
        let bytes = entry.file_name().to_bytes();
        if matches!(bytes, b"." | b"..") {
            continue;
        }
        let name = std::str::from_utf8(bytes)
            .map_err(|_| "doctor store entry name is not UTF-8")?
            .to_owned();
        let kind = parse_entry(&name)?;
        result.push((name, kind));
        if result.len() > usize::from(MAX_GENERATIONS_LIMIT) + 2 {
            return Err("doctor store inventory exceeds the fixed bound".into());
        }
    }
    result.sort_by(|left, right| left.0.cmp(&right.0));
    Ok(result)
}

pub(super) fn open_store(
    path: &Path,
    expectation: StoreExpectation,
) -> Result<DoctorStore, String> {
    if expectation.maximum_generations == 0
        || expectation.maximum_generations > MAX_GENERATIONS_LIMIT
    {
        return Err("doctor store generation limit is outside 1..=32".into());
    }
    let root = HeldRoot::open(path, true)?;
    let _lock = root.lock()?;
    validate_inventory(&root, expectation)?;
    root.recheck(true)?;
    Ok(DoctorStore { root, expectation })
}

fn validate_inventory(root: &HeldRoot, expectation: StoreExpectation) -> Result<(), String> {
    let entries = inventory(root)?;
    let generations = entries
        .iter()
        .filter(|(_, kind)| matches!(kind, EntryKind::Generation))
        .count();
    let active = entries
        .iter()
        .filter(|(_, kind)| matches!(kind, EntryKind::Active))
        .count();
    let active_stages = entries
        .iter()
        .filter(|(_, kind)| matches!(kind, EntryKind::ActiveStage))
        .count();
    let stages = entries
        .iter()
        .filter(|(_, kind)| matches!(kind, EntryKind::Stage(_)))
        .count();
    if generations > usize::from(expectation.maximum_generations)
        || active > 1
        || active_stages > 1
        || stages > 1
    {
        return Err("doctor store inventory cardinality is invalid".into());
    }
    for (name, kind) in entries {
        let stat = fs::statat(root.fd(), name.as_bytes(), AtFlags::SYMLINK_NOFOLLOW)
            .map_err(|_| "doctor store entry disappeared")?;
        match kind {
            EntryKind::Generation | EntryKind::Stage(_) => {
                if !FileType::from_raw_mode(stat.st_mode).is_dir()
                    || stat.st_uid != rustix::process::geteuid().as_raw()
                    || stat.st_mode & 0o7777 != 0o700
                {
                    return Err(
                        "doctor generation must be a current-euid-owned 0700 directory".into(),
                    );
                }
            }
            EntryKind::Active | EntryKind::ActiveStage => {
                if !FileType::from_raw_mode(stat.st_mode).is_file()
                    || stat.st_nlink != 1
                    || stat.st_uid != rustix::process::geteuid().as_raw()
                    || stat.st_mode & 0o7777 != 0o600
                    || stat.st_size != 65
                {
                    return Err("doctor ACTIVE record must be a single-link exact 0600 file".into());
                }
            }
        }
    }
    Ok(())
}

pub(super) fn install(
    store: &DoctorStore,
    source: &Path,
    expected: &ReleaseExpectation,
) -> Result<InstallReceipt, String> {
    install_with_hook(store, source, expected, |_| Ok(()))
}

fn install_with_hook(
    store: &DoctorStore,
    source: &Path,
    expected: &ReleaseExpectation,
    mut hook: impl FnMut(EffectPoint) -> Result<(), String>,
) -> Result<InstallReceipt, String> {
    let root = &store.root;
    root.recheck(true)?;
    let _lock = root.lock()?;
    validate_inventory(root, store.expectation)?;

    let source_root = HeldRoot::open(source, true)?;
    let source_identity = release_identity(&source_root)?;
    let source_files = read_release(source_root.fd())?;
    verify_loaded(&source_files, expected)?;
    source_root.recheck(true)?;
    let id = generation_id(&source_files);
    let generation = generation_name(&id);
    if entry_exists(root.fd(), &generation)? {
        return Err("doctor generation already exists; no adoption or overwrite".into());
    }
    let count = inventory(root)?
        .iter()
        .filter(|(_, kind)| matches!(kind, EntryKind::Generation))
        .count();
    if count >= usize::from(store.expectation.maximum_generations) {
        return Err("doctor store has no generation slot".into());
    }
    let stage = stage_name(&id);
    hook(EffectPoint::BeforeStageCreate)?;
    source_root.recheck(true)?;
    if release_identity(&source_root)? != source_identity {
        return Err("doctor release source identity changed before staging".into());
    }
    fs::mkdirat(root.fd(), stage.as_bytes(), Mode::from_raw_mode(0o700))
        .map_err(|_| "cannot create doctor stage without replacement")?;
    let stage_fd = open_dir(root.fd(), stage.as_bytes())?;
    for (name, bytes, executable) in &source_files {
        write_file(&stage_fd, name, bytes, *executable)?;
        hook(EffectPoint::AfterMemberFsync)?;
    }
    fs::fsync(&stage_fd).map_err(|_| "cannot settle doctor stage directory")?;
    hook(EffectPoint::AfterStageFsync)?;
    fs::fsync(root.fd()).map_err(|_| "cannot settle doctor store after stage creation")?;
    source_root.recheck(true)?;
    if read_release(source_root.fd())? != source_files {
        return Err("doctor release source changed during installation".into());
    }
    if release_identity(&source_root)? != source_identity {
        return Err("doctor release source identity changed during installation".into());
    }
    let staged_files = read_release(&stage_fd)?;
    verify_loaded(&staged_files, expected)?;
    if generation_id(&staged_files) != id {
        return Err("doctor staged generation digest disagrees".into());
    }
    fs::renameat_with(
        root.fd(),
        stage.as_bytes(),
        root.fd(),
        generation.as_bytes(),
        RenameFlags::NOREPLACE,
    )
    .map_err(|_| "cannot publish doctor generation without replacement; stage retained")?;
    if hook(EffectPoint::AfterGenerationRename).is_err() {
        return Err(
            "doctor generation publication occurred; inspect before retry or cleanup".into(),
        );
    }
    fs::fsync(root.fd()).map_err(|_| {
        "doctor generation publication is uncertain; inspect before any retry".to_owned()
    })?;
    if hook(EffectPoint::AfterGenerationRootFsync).is_err() {
        return Err(
            "doctor generation publication occurred; inspect before retry or cleanup".into(),
        );
    }
    verify_generation(root, &id, expected)?;
    Ok(InstallReceipt {
        generation: id,
        installed_new: true,
    })
}

fn read_release(root: &OwnedFd) -> Result<LoadedRelease, String> {
    let mut entries = BTreeMap::new();
    for entry in
        Dir::new(open_dir(root, b".")?).map_err(|_| "cannot enumerate release directory")?
    {
        let entry = entry.map_err(|_| "cannot read release inventory")?;
        let raw = entry.file_name().to_bytes();
        if matches!(raw, b"." | b"..") {
            continue;
        }
        let name = std::str::from_utf8(raw).map_err(|_| "release name is not UTF-8")?;
        if !INVENTORY.contains(&name) || entries.insert(name.to_owned(), ()).is_some() {
            return Err("release inventory is not exact".into());
        }
    }
    if entries.len() != INVENTORY.len() {
        return Err("release inventory is not exact".into());
    }
    INVENTORY
        .iter()
        .map(|name| {
            let executable = matches!(
                *name,
                LAUNCHER_FILE | WORKER_FILE | COLLECTOR_FILE | PROVISIONER_FILE
            );
            read_file(root, name, executable).map(|bytes| (*name, bytes, executable))
        })
        .collect()
}

fn release_identity(root: &HeldRoot) -> Result<Vec<(&'static str, Identity, i64)>, String> {
    INVENTORY
        .iter()
        .map(|name| {
            let stat = fs::statat(root.fd(), name.as_bytes(), AtFlags::SYMLINK_NOFOLLOW)
                .map_err(|_| "cannot inspect release member path")?;
            Ok((*name, Identity::from_stat(&stat), stat.st_size))
        })
        .collect()
}

fn verify_loaded(
    files: &[(&'static str, Vec<u8>, bool)],
    expected: &ReleaseExpectation,
) -> Result<(), String> {
    let borrowed: Vec<_> = files
        .iter()
        .map(|(name, bytes, executable)| (*name, bytes.as_slice(), *executable))
        .collect();
    verify_release_bytes(&borrowed, expected)
}

fn read_file(parent: &OwnedFd, name: &str, executable: bool) -> Result<Vec<u8>, String> {
    let fd = fs::openat(
        parent,
        name.as_bytes(),
        OFlags::RDONLY | OFlags::NOFOLLOW | OFlags::CLOEXEC | OFlags::NONBLOCK,
        Mode::empty(),
    )
    .map_err(|_| "cannot open release member without following links")?;
    let file = std::fs::File::from(fd);
    let before = fs::fstat(&file).map_err(|_| "cannot inspect held release member")?;
    let named_before = fs::statat(parent, name.as_bytes(), AtFlags::SYMLINK_NOFOLLOW)
        .map_err(|_| "cannot inspect selected release member path")?;
    if !FileType::from_raw_mode(before.st_mode).is_file()
        || before.st_nlink != 1
        || before.st_uid != rustix::process::geteuid().as_raw()
        || before.st_mode & 0o7777 != if executable { 0o700 } else { 0o600 }
        || before.st_size < 0
        || before.st_size as u64 > MAX_FILE_BYTES as u64
    {
        return Err("release member is not a bounded single-link regular file".into());
    }
    if Identity::from_stat(&named_before) != Identity::from_stat(&before)
        || named_before.st_size != before.st_size
        || named_before.st_nlink != before.st_nlink
    {
        return Err("release member path and held identity disagree".into());
    }
    let mut bytes = Vec::new();
    bytes
        .try_reserve_exact(before.st_size as usize + 1)
        .map_err(|_| "cannot reserve release member buffer")?;
    (&file)
        .take(before.st_size as u64 + 1)
        .read_to_end(&mut bytes)
        .map_err(|_| "cannot read release member")?;
    let after = fs::fstat(&file).map_err(|_| "cannot recheck held release member")?;
    let named_after = fs::statat(parent, name.as_bytes(), AtFlags::SYMLINK_NOFOLLOW)
        .map_err(|_| "cannot recheck selected release member path")?;
    if bytes.len() != before.st_size as usize
        || Identity::from_stat(&before) != Identity::from_stat(&after)
        || Identity::from_stat(&after) != Identity::from_stat(&named_after)
        || after.st_size != named_after.st_size
        || after.st_nlink != named_after.st_nlink
    {
        return Err("release member changed while reading".into());
    }
    Ok(bytes)
}

fn generation_id(files: &[(&'static str, Vec<u8>, bool)]) -> GenerationId {
    let mut hash = Sha256::new();
    hash.update(b"semaprax.doctor-installed-generation.v1\0");
    for (name, bytes, executable) in files {
        hash.update((*name).as_bytes());
        hash.update([0]);
        hash.update([u8::from(*executable)]);
        hash.update((bytes.len() as u64).to_le_bytes());
        hash.update(bytes);
    }
    GenerationId(hex(&hash.finalize()))
}

fn hex(bytes: &[u8]) -> String {
    const DIGITS: &[u8; 16] = b"0123456789abcdef";
    let mut value = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        value.push(DIGITS[(byte >> 4) as usize] as char);
        value.push(DIGITS[(byte & 15) as usize] as char);
    }
    value
}

fn write_file(parent: &OwnedFd, name: &str, bytes: &[u8], executable: bool) -> Result<(), String> {
    let fd = fs::openat(
        parent,
        name.as_bytes(),
        OFlags::WRONLY | OFlags::CREATE | OFlags::EXCL | OFlags::NOFOLLOW | OFlags::CLOEXEC,
        Mode::from_raw_mode(if executable { 0o700 } else { 0o600 }),
    )
    .map_err(|_| "cannot create doctor generation member without replacement")?;
    let mut file = std::fs::File::from(fd);
    file.write_all(bytes)
        .and_then(|()| file.sync_all())
        .map_err(|_| "cannot write and settle doctor generation member".to_owned())
}

fn entry_exists(parent: &OwnedFd, name: &str) -> Result<bool, String> {
    match fs::statat(parent, name.as_bytes(), AtFlags::SYMLINK_NOFOLLOW) {
        Ok(_) => Ok(true),
        Err(error) if error == rustix::io::Errno::NOENT => Ok(false),
        Err(_) => Err("cannot inspect doctor store entry".into()),
    }
}

fn verify_generation(
    root: &HeldRoot,
    id: &GenerationId,
    expected: &ReleaseExpectation,
) -> Result<(), String> {
    let name = generation_name(id);
    let held = open_dir(root.fd(), name.as_bytes())?;
    let stat = fs::fstat(&held).map_err(|_| "cannot inspect installed generation")?;
    if stat.st_uid != rustix::process::geteuid().as_raw() || stat.st_mode & 0o7777 != 0o700 {
        return Err("installed doctor generation directory identity is invalid".into());
    }
    let files = read_release(&held)?;
    if generation_id(&files) != *id {
        return Err("installed doctor generation digest disagrees".into());
    }
    verify_loaded(&files, expected)?;
    let rebound = fs::statat(root.fd(), name.as_bytes(), AtFlags::SYMLINK_NOFOLLOW)
        .map_err(|_| "installed doctor generation path disappeared")?;
    if Identity::from_stat(&rebound) != Identity::from_stat(&stat) {
        return Err("installed doctor generation path and held identity disagree".into());
    }
    Ok(())
}

fn read_active_locked(root: &HeldRoot) -> Result<Option<GenerationId>, String> {
    if !entry_exists(root.fd(), ACTIVE)? {
        return Ok(None);
    }
    let bytes = read_file(root.fd(), ACTIVE, false)?;
    if bytes.len() != 65 || bytes[64] != b'\n' {
        return Err("doctor ACTIVE record is not canonical".into());
    }
    let raw = std::str::from_utf8(&bytes[..64]).map_err(|_| "doctor ACTIVE is not UTF-8")?;
    if !canonical_id(raw) {
        return Err("doctor ACTIVE generation ID is invalid".into());
    }
    Ok(Some(GenerationId(raw.to_owned())))
}

pub(super) fn inspect_active(store: &DoctorStore) -> Result<Option<GenerationId>, String> {
    let root = &store.root;
    root.recheck(true)?;
    let _lock = root.lock()?;
    validate_inventory(root, store.expectation)?;
    read_active_locked(root)
}

pub(super) fn activate(
    store: &DoctorStore,
    generation: &GenerationId,
    expected_active: Option<&GenerationId>,
    expected_release: &ReleaseExpectation,
) -> Result<(), String> {
    activate_with_hook(store, generation, expected_active, expected_release, |_| {
        Ok(())
    })
}

fn activate_with_hook(
    store: &DoctorStore,
    generation: &GenerationId,
    expected_active: Option<&GenerationId>,
    expected_release: &ReleaseExpectation,
    mut hook: impl FnMut(EffectPoint) -> Result<(), String>,
) -> Result<(), String> {
    if !canonical_id(&generation.0) {
        return Err("doctor generation ID is invalid".into());
    }
    let root = &store.root;
    root.recheck(true)?;
    let _lock = root.lock()?;
    validate_inventory(root, store.expectation)?;
    let current = read_active_locked(root)?;
    if current.as_ref() != expected_active {
        return Err("doctor ACTIVE compare-and-swap expectation disagrees".into());
    }
    verify_generation(root, generation, expected_release)?;
    if entry_exists(root.fd(), ACTIVE_STAGE)? {
        return Err("doctor ACTIVE stage already exists; recovery is required".into());
    }
    let mut record = generation.0.as_bytes().to_vec();
    record.push(b'\n');
    write_file(root.fd(), ACTIVE_STAGE, &record, false)?;
    fs::fsync(root.fd()).map_err(|_| "cannot settle doctor ACTIVE stage")?;
    hook(EffectPoint::AfterActiveStageFsync)?;
    verify_generation(root, generation, expected_release)?;
    if read_active_locked(root)?.as_ref() != expected_active {
        return Err("doctor ACTIVE changed before the compare-and-swap pivot".into());
    }
    let flags = if current.is_some() {
        RenameFlags::empty()
    } else {
        RenameFlags::NOREPLACE
    };
    fs::renameat_with(
        root.fd(),
        ACTIVE_STAGE.as_bytes(),
        root.fd(),
        ACTIVE.as_bytes(),
        flags,
    )
    .map_err(|_| "doctor ACTIVE pivot failed; stage retained")?;
    if hook(EffectPoint::AfterActiveRename).is_err() {
        return Err("doctor ACTIVE pivot occurred; inspect before retry or cleanup".into());
    }
    fs::fsync(root.fd()).map_err(|_| "doctor ACTIVE pivot is uncertain; inspect before retry")?;
    if hook(EffectPoint::AfterActiveRootFsync).is_err() {
        return Err("doctor ACTIVE pivot occurred; inspect before retry or cleanup".into());
    }
    if read_active_locked(root)?.as_ref() != Some(generation) {
        return Err("doctor ACTIVE pivot result disagrees".into());
    }
    Ok(())
}

pub(super) fn recover(
    store: &DoctorStore,
    expected: &ReleaseExpectation,
) -> Result<RecoveryReceipt, String> {
    recover_with_hook(store, expected, |_| Ok(()))
}

fn recover_with_hook(
    store: &DoctorStore,
    expected: &ReleaseExpectation,
    mut hook: impl FnMut(EffectPoint) -> Result<(), String>,
) -> Result<RecoveryReceipt, String> {
    let root = &store.root;
    root.recheck(true)?;
    let _lock = root.lock()?;
    validate_inventory(root, store.expectation)?;
    let entries = inventory(root)?;
    if entries
        .iter()
        .any(|(_, kind)| matches!(kind, EntryKind::ActiveStage))
    {
        return Err(
            "doctor ACTIVE stage is ambiguous; inspect it without automatic deletion".into(),
        );
    }
    let stages: Vec<_> = entries
        .iter()
        .filter_map(|(_, kind)| match kind {
            EntryKind::Stage(id) => Some(id.clone()),
            _ => None,
        })
        .collect();
    let Some(id) = stages.first() else {
        return Ok(RecoveryReceipt {
            removed_generation: None,
        });
    };
    let stage = stage_name(id);
    let held = open_dir(root.fd(), stage.as_bytes())?;
    let stage_stat = fs::fstat(&held).map_err(|_| "cannot inspect doctor stage")?;
    let files = read_release(&held)?;
    if generation_id(&files) != *id {
        return Err("doctor stage is not an authenticated owned generation".into());
    }
    verify_loaded(&files, expected)?;
    let rebound = fs::statat(root.fd(), stage.as_bytes(), AtFlags::SYMLINK_NOFOLLOW)
        .map_err(|_| "doctor stage path disappeared")?;
    if Identity::from_stat(&rebound) != Identity::from_stat(&stage_stat) {
        return Err("doctor stage path and held identity disagree".into());
    }
    let held_members = hold_members(&held)?;
    hook(EffectPoint::BeforeRecoveryEffect)?;
    revalidate_members(&held, &held_members)?;
    for name in INVENTORY {
        fs::unlinkat(&held, name.as_bytes(), AtFlags::empty())
            .map_err(|_| "cannot remove authenticated doctor stage member")?;
    }
    fs::fsync(&held).map_err(|_| "cannot settle emptied doctor stage")?;
    fs::unlinkat(root.fd(), stage.as_bytes(), AtFlags::REMOVEDIR)
        .map_err(|_| "cannot remove authenticated doctor stage directory")?;
    fs::fsync(root.fd()).map_err(|_| "cannot settle doctor stage recovery")?;
    Ok(RecoveryReceipt {
        removed_generation: Some(id.clone()),
    })
}

struct HeldMember {
    name: &'static str,
    file: std::fs::File,
    identity: Identity,
    size: i64,
    links: u64,
}

fn hold_members(root: &OwnedFd) -> Result<Vec<HeldMember>, String> {
    INVENTORY
        .iter()
        .map(|name| {
            let fd = fs::openat(
                root,
                name.as_bytes(),
                OFlags::RDONLY | OFlags::NOFOLLOW | OFlags::CLOEXEC | OFlags::NONBLOCK,
                Mode::empty(),
            )
            .map_err(|_| "cannot hold authenticated recovery member")?;
            let file = std::fs::File::from(fd);
            let stat = fs::fstat(&file).map_err(|_| "cannot inspect recovery member")?;
            Ok(HeldMember {
                name,
                file,
                identity: Identity::from_stat(&stat),
                size: stat.st_size,
                links: link_count(&stat),
            })
        })
        .collect()
}

fn revalidate_members(root: &OwnedFd, members: &[HeldMember]) -> Result<(), String> {
    for member in members {
        let held = fs::fstat(&member.file).map_err(|_| "cannot recheck held recovery member")?;
        let named = fs::statat(root, member.name.as_bytes(), AtFlags::SYMLINK_NOFOLLOW)
            .map_err(|_| "cannot recheck recovery member path")?;
        if Identity::from_stat(&held) != member.identity
            || Identity::from_stat(&named) != member.identity
            || held.st_size != member.size
            || named.st_size != member.size
            || link_count(&held) != member.links
            || link_count(&named) != member.links
        {
            return Err("recovery member path or held identity changed before deletion".into());
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::os::unix::fs::PermissionsExt as _;
    use std::path::{Path, PathBuf};
    use std::sync::atomic::{AtomicU64, Ordering};

    use super::*;
    use crate::{create_release, key_information, ReleaseInputs};

    static SERIAL: AtomicU64 = AtomicU64::new(0);

    struct Fixture(PathBuf);
    impl Fixture {
        fn new() -> Self {
            let path = std::env::temp_dir().join(format!(
                "semaprax-doctor-install-fault-{}-{}",
                std::process::id(),
                SERIAL.fetch_add(1, Ordering::Relaxed)
            ));
            fs::create_dir(&path).unwrap();
            Self(fs::canonicalize(path).unwrap())
        }
        fn dir(&self, name: &str) -> PathBuf {
            let path = self.0.join(name);
            fs::create_dir(&path).unwrap();
            fs::set_permissions(&path, fs::Permissions::from_mode(0o700)).unwrap();
            path
        }
        fn file(&self, name: &str, bytes: &[u8], executable: bool) -> PathBuf {
            let path = self.0.join(name);
            fs::write(&path, bytes).unwrap();
            fs::set_permissions(
                &path,
                fs::Permissions::from_mode(if executable { 0o700 } else { 0o600 }),
            )
            .unwrap();
            path
        }
    }
    impl Drop for Fixture {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    fn elf() -> Vec<u8> {
        let mut bytes = vec![0; 120];
        bytes[..7].copy_from_slice(b"\x7fELF\x02\x01\x01");
        bytes[16..18].copy_from_slice(&2_u16.to_le_bytes());
        bytes[18..20].copy_from_slice(&62_u16.to_le_bytes());
        bytes[20..24].copy_from_slice(&1_u32.to_le_bytes());
        bytes[32..40].copy_from_slice(&64_u64.to_le_bytes());
        bytes[52..54].copy_from_slice(&64_u16.to_le_bytes());
        bytes[54..56].copy_from_slice(&56_u16.to_le_bytes());
        bytes[56..58].copy_from_slice(&1_u16.to_le_bytes());
        bytes[64..68].copy_from_slice(&1_u32.to_le_bytes());
        bytes[96..104].copy_from_slice(&120_u64.to_le_bytes());
        bytes
    }

    fn release(fixture: &Fixture) -> (PathBuf, ReleaseExpectation) {
        let output = fixture.dir("release");
        let key = fixture.file("key", format!("{}\n", "35".repeat(32)).as_bytes(), false);
        let input = ReleaseInputs {
            request: fixture.file("request", b"request", false),
            bundle: fixture.file("bundle", b"bundle", false),
            launcher: fixture.file("launcher", &elf(), true),
            worker: fixture.file("worker", &elf(), true),
            collector: fixture.file("collector", &elf(), true),
            provisioner: fixture.file("provisioner", &elf(), true),
            selector: "release-linux-v1".into(),
            architecture: 1,
            target: 3,
            release_version: "0.2.0".into(),
            release_commit: "0123456789abcdef0123456789abcdef01234567".into(),
            target_triple: "x86_64-unknown-linux-musl".into(),
            signing_key: key,
            output_directory: output.clone(),
        };
        create_release(&input).unwrap();
        for (source, name) in [
            (&input.request, REQUEST_FILE),
            (&input.bundle, BUNDLE_FILE),
            (&input.launcher, LAUNCHER_FILE),
            (&input.worker, WORKER_FILE),
            (&input.collector, COLLECTOR_FILE),
            (&input.provisioner, PROVISIONER_FILE),
        ] {
            fs::copy(source, output.join(name)).unwrap();
        }
        let key_info = key_information(&input.signing_key).unwrap();
        let marker = "\"public_key_hex\":\"";
        let start = key_info.find(marker).unwrap() + marker.len();
        let expected = ReleaseExpectation {
            release_version: input.release_version,
            release_commit: input.release_commit,
            target_triple: input.target_triple,
            architecture: input.architecture,
            target: input.target,
            selector: input.selector,
            public_key_hex: key_info[start..start + 64].into(),
        };
        (output, expected)
    }

    fn opened_store(path: &Path) -> DoctorStore {
        open_store(path, StoreExpectation::default()).unwrap()
    }

    #[test]
    fn recovery_substitution_before_first_effect_deletes_nothing() {
        let fixture = Fixture::new();
        let store_path = fixture.dir("store");
        let store = opened_store(&store_path);
        let (source, expected) = release(&fixture);
        let installed = install(&store, &source, &expected).unwrap();
        let generation = generation_name(&installed.generation);
        let stage = stage_name(&installed.generation);
        fs::rename(store_path.join(generation), store_path.join(&stage)).unwrap();
        let request = store_path.join(&stage).join(REQUEST_FILE);
        let original = fs::read(&request).unwrap();
        let result = recover_with_hook(&store, &expected, |point| {
            if point == EffectPoint::BeforeRecoveryEffect {
                fs::remove_file(&request).unwrap();
                fs::write(&request, &original).unwrap();
                fs::set_permissions(&request, fs::Permissions::from_mode(0o600)).unwrap();
            }
            Ok(())
        });
        assert!(result.is_err());
        for name in INVENTORY {
            assert!(store_path.join(&stage).join(name).exists());
        }
    }

    #[test]
    fn post_pivot_faults_are_sticky_and_resolvable_by_inspection() {
        let fixture = Fixture::new();
        let (source, expected) = release(&fixture);
        for point in [
            EffectPoint::AfterGenerationRename,
            EffectPoint::AfterGenerationRootFsync,
        ] {
            let store_path = fixture.dir(&format!("store-{point:?}"));
            let store = opened_store(&store_path);
            let result = install_with_hook(&store, &source, &expected, |seen| {
                (seen != point)
                    .then_some(())
                    .ok_or_else(|| "injected".into())
            });
            assert!(result.is_err());
            assert!(fs::read_dir(&store_path).unwrap().any(|entry| entry
                .unwrap()
                .file_name()
                .to_string_lossy()
                .starts_with(GENERATION_PREFIX)));
        }

        let store_path = fixture.dir("active-store");
        let store = opened_store(&store_path);
        let installed = install(&store, &source, &expected).unwrap();

        let pre_active_path = fixture.dir("pre-active-store");
        let pre_active_store = opened_store(&pre_active_path);
        let pre_active_installed = install(&pre_active_store, &source, &expected).unwrap();
        let pre_active = activate_with_hook(
            &pre_active_store,
            &pre_active_installed.generation,
            None,
            &expected,
            |seen| {
                (seen != EffectPoint::AfterActiveStageFsync)
                    .then_some(())
                    .ok_or_else(|| "injected".into())
            },
        );
        assert!(pre_active.is_err());
        assert_eq!(inspect_active(&pre_active_store).unwrap(), None);
        assert!(pre_active_path.join(ACTIVE_STAGE).exists());
        assert!(activate(
            &pre_active_store,
            &pre_active_installed.generation,
            None,
            &expected
        )
        .is_err());

        for point in [
            EffectPoint::AfterActiveRename,
            EffectPoint::AfterActiveRootFsync,
        ] {
            if inspect_active(&store).unwrap().is_some() {
                fs::remove_file(store_path.join(ACTIVE)).unwrap();
            }
            let result =
                activate_with_hook(&store, &installed.generation, None, &expected, |seen| {
                    (seen != point)
                        .then_some(())
                        .ok_or_else(|| "injected".into())
                });
            assert!(result.is_err());
            assert_eq!(
                inspect_active(&store).unwrap(),
                Some(installed.generation.clone())
            );
        }
    }

    #[test]
    fn pre_pivot_faults_never_create_a_completed_generation() {
        let fixture = Fixture::new();
        let (source, expected) = release(&fixture);
        for point in [
            EffectPoint::BeforeStageCreate,
            EffectPoint::AfterMemberFsync,
            EffectPoint::AfterStageFsync,
        ] {
            let store_path = fixture.dir(&format!("pre-store-{point:?}"));
            let store = opened_store(&store_path);
            let result = install_with_hook(&store, &source, &expected, |seen| {
                (seen != point)
                    .then_some(())
                    .ok_or_else(|| "injected".into())
            });
            assert!(result.is_err());
            assert!(!fs::read_dir(&store_path).unwrap().any(|entry| entry
                .unwrap()
                .file_name()
                .to_string_lossy()
                .starts_with(GENERATION_PREFIX)));
        }

        let store_path = fixture.dir("source-substitution-store");
        let store = opened_store(&store_path);
        let request = source.join(REQUEST_FILE);
        let bytes = fs::read(&request).unwrap();
        let result = install_with_hook(&store, &source, &expected, |point| {
            if point == EffectPoint::BeforeStageCreate {
                fs::remove_file(&request).unwrap();
                fs::write(&request, &bytes).unwrap();
                fs::set_permissions(&request, fs::Permissions::from_mode(0o600)).unwrap();
            }
            Ok(())
        });
        assert!(result.is_err());
        assert!(fs::read_dir(&store_path).unwrap().next().is_none());
    }
}
