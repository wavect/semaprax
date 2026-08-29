use std::collections::{BTreeMap, BTreeSet};
use std::io::{Read, Seek, SeekFrom, Write};
use std::os::fd::{AsFd, OwnedFd};
use std::os::unix::ffi::OsStrExt;
use std::path::{Component, Path};

use rustix::fs::{self, AtFlags, Dir, FileType, Mode, OFlags, RenameFlags, CWD};

use super::{
    authentication, io, post_pivot, source_inventory, PreparedEntry, StoredEntry,
    MAX_STORE_ENTRIES, MAX_STORE_ENTRY_JSON_BYTES, MAX_STORE_INVENTORY_ENTRIES,
    MAX_STORE_MANIFEST_BYTES, MAX_STORE_TOTAL_SOURCE_BYTES, MAX_STORE_WORKSPACE_MANIFEST_BYTES,
};
use crate::diagnostic::Diagnostic;

#[cfg(test)]
std::thread_local! {
    static RETAINED_METADATA_AUTHENTICATIONS: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct Identity {
    device: u64,
    inode: u64,
    mode: u32,
}

struct RetainedEntryFact {
    identity: Identity,
    _structural_inventory: BTreeMap<String, (bool, Identity)>,
}

struct RootInventory {
    names: BTreeSet<String>,
    retained: BTreeMap<String, RetainedEntryFact>,
}

struct CreatedEntry {
    inventory: BTreeMap<String, (bool, Identity)>,
    directories: Vec<OwnedFd>,
}

struct AdvisoryLock {
    file: std::fs::File,
    held: bool,
}

impl AdvisoryLock {
    fn new(file: std::fs::File) -> Self {
        Self { file, held: true }
    }

    fn release(mut self) -> Result<(), std::io::Error> {
        let result = fs2::FileExt::unlock(&self.file);
        if result.is_ok() {
            self.held = false;
        }
        result
    }
}

impl Drop for AdvisoryLock {
    fn drop(&mut self) {
        if self.held {
            let _ = fs2::FileExt::unlock(&self.file);
        }
    }
}

pub(super) fn persist(root: &Path, prepared: &PreparedEntry) -> Result<(), Vec<Diagnostic>> {
    persist_with_hook(root, prepared, |_, _| Ok(()))
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum StorePoint {
    AfterStageCreate,
    AfterStageWrite,
    BeforePublish,
    AfterPublish,
}

pub(super) fn persist_with_hook(
    root_path: &Path,
    prepared: &PreparedEntry,
    mut hook: impl FnMut(StorePoint, &Path) -> Result<(), std::io::Error>,
) -> Result<(), Vec<Diagnostic>> {
    let root = open_root(root_path)?;
    let root_identity = identity(&root)?;
    require_root_path_identity(root_path, root_identity)?;
    let lock_file = std::fs::File::from(
        rustix::io::dup(&root)
            .map_err(|error| io(format!("cannot duplicate held store root: {error}")))?,
    );
    fs2::FileExt::try_lock_exclusive(&lock_file)
        .map_err(|error| authentication(format!("Project Revision Store root is busy: {error}")))?;
    let lock = AdvisoryLock::new(lock_file);
    let initial = root_inventory(&root)?;
    require_publication_capacity(initial.names.len())?;
    let destination = prepared.entry_hex();
    if initial.names.contains(destination) {
        return Err(authentication(
            "Project Revision Store content-addressed destination already exists",
        ));
    }
    require_root_path_identity(root_path, root_identity)?;
    let stage = format!(".stage-{destination}");
    fs::mkdirat(&root, stage.as_bytes(), Mode::from_raw_mode(0o700)).map_err(|error| {
        io(format!(
            "cannot create Project Revision Store stage without clobber: {error}"
        ))
    })?;
    let stage_fd = open_directory_at(&root, stage.as_bytes())?;
    let stage_identity = identity(&stage_fd)?;
    require_entry_directory_mode(&stage_fd)?;
    hook(StorePoint::AfterStageCreate, root_path)
        .map_err(|error| io(format!("Project Revision Store stage hook failed: {error}")))?;
    let mut with_stage = initial.names.clone();
    with_stage.insert(stage.clone());
    require_root_path_identity(root_path, root_identity)?;
    require_identity_at(
        &root,
        stage.as_bytes(),
        stage_identity,
        "store stage changed before writing",
    )?;
    require_root_inventory(&root, &with_stage, &initial.retained)?;
    let created = write_entry(&stage_fd, prepared)?;
    let prepared_paths = prepared
        .sources
        .iter()
        .map(|source| source.path.clone())
        .collect::<Vec<_>>();
    let expected_stage_inventory = expected_inventory(&prepared_paths)?;
    require_exact_inventory(&stage_fd, &expected_stage_inventory, &created.inventory)?;
    hook(StorePoint::AfterStageWrite, root_path)
        .map_err(|error| io(format!("Project Revision Store write hook failed: {error}")))?;
    require_root_path_identity(root_path, root_identity)?;
    require_identity_at(
        &root,
        stage.as_bytes(),
        stage_identity,
        "store stage changed after writing",
    )?;
    require_root_inventory(&root, &with_stage, &initial.retained)?;
    require_exact_inventory(&stage_fd, &expected_stage_inventory, &created.inventory)?;
    sync_created_directories(&stage_fd, &created.directories)?;
    fs::fsync(&root).map_err(|error| {
        io(format!(
            "cannot settle Project Revision Store stage: {error}"
        ))
    })?;
    require_identity(
        &root,
        root_identity,
        "store root changed before publication",
    )?;
    require_root_path_identity(root_path, root_identity)?;
    require_identity_at(
        &root,
        stage.as_bytes(),
        stage_identity,
        "store stage changed before publication",
    )?;
    require_root_inventory(&root, &with_stage, &initial.retained)?;
    require_exact_inventory(&stage_fd, &expected_stage_inventory, &created.inventory)?;
    authenticate_prepared(&stage_fd, prepared)?;
    hook(StorePoint::BeforePublish, root_path).map_err(|error| {
        io(format!(
            "Project Revision Store pre-publish hook failed: {error}"
        ))
    })?;
    require_root_path_identity(root_path, root_identity)?;
    require_identity_at(
        &root,
        stage.as_bytes(),
        stage_identity,
        "store stage changed immediately before publication",
    )?;
    require_root_inventory(&root, &with_stage, &initial.retained)?;
    require_exact_inventory(&stage_fd, &expected_stage_inventory, &created.inventory)?;
    sync_created_directories(&stage_fd, &created.directories)?;
    fs::fsync(&root).map_err(|error| {
        io(format!(
            "cannot resettle Project Revision Store stage: {error}"
        ))
    })?;
    require_identity(
        &root,
        root_identity,
        "store root changed before publication",
    )?;
    require_root_path_identity(root_path, root_identity)?;
    require_identity_at(
        &root,
        stage.as_bytes(),
        stage_identity,
        "store stage changed before publication",
    )?;
    require_root_inventory(&root, &with_stage, &initial.retained)?;
    require_exact_inventory(&stage_fd, &expected_stage_inventory, &created.inventory)?;
    authenticate_prepared(&stage_fd, prepared)?;
    fs::renameat_with(
        &root,
        stage.as_bytes(),
        &root,
        destination.as_bytes(),
        RenameFlags::NOREPLACE,
    )
    .map_err(|error| {
        io(format!(
            "cannot publish Project Revision Store entry without replacement: {error}"
        ))
    })?;
    fs::fsync(&root).map_err(|error| {
        post_pivot(format!(
            "cannot settle Project Revision Store publication: {error}"
        ))
    })?;
    hook(StorePoint::AfterPublish, root_path).map_err(|error| {
        post_pivot(format!(
            "Project Revision Store post-publication hook failed: {error}"
        ))
    })?;
    let published = open_directory_at(&root, destination.as_bytes())
        .map_err(|_| post_pivot("cannot reopen published Project Revision Store entry"))?;
    require_entry_directory_mode(&published)
        .map_err(|_| post_pivot("published Project Revision Store entry permissions changed"))?;
    if identity(&published)
        .map_err(|_| post_pivot("cannot identify published Project Revision Store entry"))?
        != stage_identity
    {
        return Err(post_pivot(
            "published Project Revision Store identity differs from the exact stage",
        ));
    }
    authenticate_prepared(&published, prepared).map_err(|_| {
        post_pivot("published Project Revision Store entry failed exact authentication")
    })?;
    require_exact_inventory(&published, &expected_stage_inventory, &created.inventory)
        .map_err(|_| post_pivot("published Project Revision Store identity inventory changed"))?;
    let mut published_inventory = initial.names;
    published_inventory.insert(destination.to_owned());
    require_identity(&root, root_identity, "store root changed after publication")
        .map_err(|_| post_pivot("Project Revision Store root changed after publication"))?;
    require_identity_at(
        &root,
        destination.as_bytes(),
        stage_identity,
        "published entry identity changed",
    )
    .map_err(|_| post_pivot("Project Revision Store published entry identity changed"))?;
    require_root_path_identity(root_path, root_identity)
        .map_err(|_| post_pivot("Project Revision Store root path changed after publication"))?;
    require_root_inventory(&root, &published_inventory, &initial.retained).map_err(|_| {
        post_pivot("Project Revision Store root inventory changed after publication")
    })?;
    lock.release().map_err(|error| {
        post_pivot(format!(
            "cannot release Project Revision Store root: {error}"
        ))
    })
}

pub(super) fn require_publication_capacity(entries: usize) -> Result<(), Vec<Diagnostic>> {
    if entries >= MAX_STORE_ENTRIES {
        return Err(super::limit("retained_entries", MAX_STORE_ENTRIES));
    }
    Ok(())
}

#[cfg(test)]
pub(super) fn reset_retained_metadata_authentications() {
    RETAINED_METADATA_AUTHENTICATIONS.with(|count| count.set(0));
}

#[cfg(test)]
pub(super) fn retained_metadata_authentications() -> usize {
    RETAINED_METADATA_AUTHENTICATIONS.with(std::cell::Cell::get)
}

pub(super) fn load(root_path: &Path, entry_digest: &str) -> Result<StoredEntry, Vec<Diagnostic>> {
    let root = open_root(root_path)?;
    let root_identity = identity(&root)?;
    require_root_path_identity(root_path, root_identity)?;
    let lock_file = std::fs::File::from(
        rustix::io::dup(&root)
            .map_err(|error| io(format!("cannot duplicate held store root: {error}")))?,
    );
    fs2::FileExt::try_lock_shared(&lock_file)
        .map_err(|error| authentication(format!("Project Revision Store root is busy: {error}")))?;
    let lock = AdvisoryLock::new(lock_file);
    let inventory = root_inventory(&root)?;
    let entry_hex = entry_digest
        .strip_prefix("sha256:")
        .ok_or_else(|| authentication("Project Revision Store digest prefix is absent"))?;
    if !inventory.names.contains(entry_hex) {
        return Err(authentication(
            "Project Revision Store entry is absent from the exact root inventory",
        ));
    }
    let entry = open_directory_at(&root, entry_hex.as_bytes())?;
    require_entry_directory_mode(&entry)?;
    let entry_identity = identity(&entry)?;
    let stored = read_stored(&entry)?;
    require_identity(&root, root_identity, "store root changed while loading")?;
    require_root_path_identity(root_path, root_identity)?;
    require_identity_at(
        &root,
        entry_hex.as_bytes(),
        entry_identity,
        "store entry changed while loading",
    )?;
    require_root_inventory(&root, &inventory.names, &inventory.retained)?;
    lock.release().map_err(|error| {
        io(format!(
            "cannot release Project Revision Store root: {error}"
        ))
    })?;
    Ok(stored)
}

fn open_root(path: &Path) -> Result<OwnedFd, Vec<Diagnostic>> {
    if !path.is_absolute() {
        return Err(authentication(
            "Project Revision Store root must be absolute",
        ));
    }
    let normalized = path
        .components()
        .filter_map(|component| match component {
            Component::Normal(name) => Some(name.as_bytes()),
            _ => None,
        })
        .fold(Vec::new(), |mut bytes, component| {
            bytes.push(b'/');
            bytes.extend_from_slice(component);
            bytes
        });
    let normalized = if normalized.is_empty() {
        b"/".as_slice()
    } else {
        normalized.as_slice()
    };
    if path.as_os_str().as_bytes() != normalized {
        return Err(authentication(
            "Project Revision Store root is not an absolute normalized path",
        ));
    }
    let mut current = fs::openat(
        CWD,
        b"/".as_slice(),
        OFlags::RDONLY | OFlags::DIRECTORY | OFlags::NOFOLLOW | OFlags::CLOEXEC,
        Mode::empty(),
    )
    .map_err(|error| io(format!("cannot hold filesystem root: {error}")))?;
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
                    authentication(format!(
                        "cannot authenticate Project Revision Store root: {error}"
                    ))
                })?;
            }
            _ => {
                return Err(authentication(
                    "Project Revision Store root is not an absolute normalized path",
                ));
            }
        }
    }
    let stat = fs::fstat(&current)
        .map_err(|error| authentication(format!("cannot inspect store root: {error}")))?;
    if !FileType::from_raw_mode(stat.st_mode).is_dir()
        || stat.st_uid != rustix::process::geteuid().as_raw()
        || stat.st_mode as u32 & 0o777 != 0o700
    {
        return Err(authentication(
            "Project Revision Store root must be one current-euid-owned 0700 directory",
        ));
    }
    Ok(current)
}

fn require_root_path_identity(path: &Path, expected: Identity) -> Result<(), Vec<Diagnostic>> {
    let rebound = open_root(path)?;
    if identity(&rebound)? != expected {
        return Err(authentication(
            "Project Revision Store path and held root authority disagree",
        ));
    }
    Ok(())
}

fn root_inventory(root: &OwnedFd) -> Result<RootInventory, Vec<Diagnostic>> {
    let names = directory_names(root)?;
    if names.len() > MAX_STORE_ENTRIES {
        return Err(super::limit("retained_entries", MAX_STORE_ENTRIES));
    }
    let mut retained = BTreeMap::new();
    for name in names {
        if name.len() != 64
            || !name
                .as_bytes()
                .iter()
                .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(byte))
        {
            return Err(authentication(
                "Project Revision Store root inventory contains foreign bytes",
            ));
        }
        let stat =
            fs::statat(root, name.as_bytes(), AtFlags::SYMLINK_NOFOLLOW).map_err(|error| {
                authentication(format!("cannot inspect store entry identity: {error}"))
            })?;
        if !FileType::from_raw_mode(stat.st_mode).is_dir() {
            return Err(authentication(
                "Project Revision Store root entry is not one real directory",
            ));
        }
        let fact = authenticate_retained_entry(root, &name)?;
        retained.insert(name, fact);
    }
    Ok(RootInventory {
        names: retained.keys().cloned().collect(),
        retained,
    })
}

fn require_root_inventory(
    root: &OwnedFd,
    expected: &BTreeSet<String>,
    retained: &BTreeMap<String, RetainedEntryFact>,
) -> Result<(), Vec<Diagnostic>> {
    let observed = directory_names(root)?.into_iter().collect::<BTreeSet<_>>();
    if observed != *expected {
        return Err(authentication(
            "Project Revision Store root inventory changed",
        ));
    }
    for (name, fact) in retained {
        require_identity_at(
            root,
            name.as_bytes(),
            fact.identity,
            "Project Revision Store retained entry identity changed",
        )?;
    }
    Ok(())
}

fn authenticate_retained_entry(
    root: &OwnedFd,
    name: &str,
) -> Result<RetainedEntryFact, Vec<Diagnostic>> {
    #[cfg(test)]
    RETAINED_METADATA_AUTHENTICATIONS.with(|count| count.set(count.get() + 1));
    let entry = open_directory_at(root, name.as_bytes()).map_err(|_| {
        authentication("Project Revision Store retained entry cannot be opened exactly")
    })?;
    require_entry_directory_mode(&entry)?;
    let entry_identity = identity(&entry)?;
    let initial_inventory = entry_inventory(&entry)?;
    let entry_json = read_file(&entry, "entry.json", MAX_STORE_ENTRY_JSON_BYTES).map_err(|_| {
        authentication("Project Revision Store retained metadata cannot be read exactly")
    })?;
    let header = super::parse_entry_header(&entry_json).map_err(|_| {
        authentication("Project Revision Store retained entry metadata is not canonical")
    })?;
    let digest = format!("sha256:{name}");
    let sources = header
        .sources
        .iter()
        .map(|source| super::StoredSource {
            path: source.path.clone(),
            source_graph_schema: source.source_graph_schema.clone(),
            source_revision: source.source_revision.clone(),
            source_digest: source.source_digest.clone(),
            bytes: source.bytes,
            source: Vec::new(),
        })
        .collect::<Vec<_>>();
    let prepared = PreparedEntry {
        project_schema: header.project_schema.clone(),
        project_revision: header.project_revision.clone(),
        workspace_revision: header.workspace_revision.clone(),
        project_graph_digest: header.project_graph_digest.clone(),
        manifest: vec![0; header.manifest_bytes],
        manifest_digest: header.manifest_digest.clone(),
        workspace_manifest: vec![0; header.workspace_manifest_bytes],
        workspace_manifest_digest: header.workspace_manifest_digest.clone(),
        inventory_entries: super::inventory_entries(&sources)?,
        sources,
        entry_json: Vec::new(),
        entry_digest: digest.clone(),
    };
    if super::render_entry_fixed_point(&prepared)? != entry_json
        || super::framed_digest(super::ENTRY_DIGEST_DOMAIN, &entry_json) != digest
    {
        return Err(authentication(
            "Project Revision Store retained metadata digest is not content addressed",
        ));
    }
    let paths = header
        .sources
        .iter()
        .map(|source| source.path.clone())
        .collect::<Vec<_>>();
    let expected = expected_inventory(&paths)?;
    require_inventory_shape(&initial_inventory, &expected)?;
    require_file_size(&entry, "semaprax.toml", header.manifest_bytes)?;
    require_file_size(
        &entry,
        "workspace-manifest.json",
        header.workspace_manifest_bytes,
    )?;
    let sources_root = open_directory_at(&entry, b"sources")?;
    for source in &header.sources {
        let (parent, leaf) = open_parent(&sources_root, &source.path)?;
        require_file_size(&parent, &leaf, source.bytes)?;
    }
    let after = require_entry_inventory(&entry, &expected)?;
    if initial_inventory != after {
        return Err(authentication(
            "Project Revision Store retained structural inventory changed",
        ));
    }
    require_identity(
        &entry,
        entry_identity,
        "Project Revision Store retained entry identity changed",
    )?;
    require_identity_at(
        root,
        name.as_bytes(),
        entry_identity,
        "Project Revision Store retained entry path changed",
    )?;
    Ok(RetainedEntryFact {
        identity: entry_identity,
        _structural_inventory: initial_inventory,
    })
}

fn require_file_size(
    parent: &OwnedFd,
    name: &str,
    expected_bytes: usize,
) -> Result<(), Vec<Diagnostic>> {
    let stat = fs::statat(parent, name.as_bytes(), AtFlags::SYMLINK_NOFOLLOW)
        .map_err(|error| authentication(format!("cannot inspect retained store file: {error}")))?;
    if !FileType::from_raw_mode(stat.st_mode).is_file()
        || stat.st_nlink != 1
        || stat.st_mode as u32 & 0o777 != 0o600
        || stat.st_size < 0
        || stat.st_size as u64 != expected_bytes as u64
    {
        return Err(authentication(
            "Project Revision Store retained file metadata is not exact",
        ));
    }
    Ok(())
}

fn directory_names(directory: &OwnedFd) -> Result<Vec<String>, Vec<Diagnostic>> {
    let reopened = open_directory_at(directory, b".")?;
    let entries = Dir::new(reopened)
        .map_err(|error| authentication(format!("cannot inspect store inventory: {error}")))?;
    let mut names = Vec::new();
    for entry in entries {
        let entry = entry
            .map_err(|error| authentication(format!("cannot inspect store inventory: {error}")))?;
        let name = entry.file_name();
        if name == c"." || name == c".." {
            continue;
        }
        let name = std::str::from_utf8(name.to_bytes())
            .map_err(|_| authentication("store inventory name is not canonical UTF-8"))?;
        names.push(name.to_owned());
        if names.len() > MAX_STORE_INVENTORY_ENTRIES {
            return Err(super::limit(
                "inventory_entries",
                MAX_STORE_INVENTORY_ENTRIES,
            ));
        }
    }
    names.sort();
    if names.windows(2).any(|pair| pair[0] == pair[1]) {
        return Err(authentication("store inventory contains duplicate names"));
    }
    Ok(names)
}

fn write_entry(
    directory: &OwnedFd,
    prepared: &PreparedEntry,
) -> Result<CreatedEntry, Vec<Diagnostic>> {
    let mut created = BTreeMap::new();
    created.insert(
        "entry.json".to_owned(),
        (
            false,
            write_file(directory, b"entry.json", &prepared.entry_json)?,
        ),
    );
    created.insert(
        "semaprax.toml".to_owned(),
        (
            false,
            write_file(directory, b"semaprax.toml", &prepared.manifest)?,
        ),
    );
    created.insert(
        "workspace-manifest.json".to_owned(),
        (
            false,
            write_file(
                directory,
                b"workspace-manifest.json",
                &prepared.workspace_manifest,
            )?,
        ),
    );
    fs::mkdirat(directory, b"sources".as_slice(), Mode::from_raw_mode(0o700)).map_err(|error| {
        io(format!(
            "cannot create Project Revision Store sources: {error}"
        ))
    })?;
    let sources = open_directory_at(directory, b"sources")?;
    let sources_identity = identity(&sources)?;
    require_identity_at(
        directory,
        b"sources",
        sources_identity,
        "created sources path changed",
    )?;
    created.insert("sources".to_owned(), (true, sources_identity));
    let mut held_parent_indexes = BTreeMap::new();
    held_parent_indexes.insert(String::new(), 0usize);
    let mut held_directories = vec![sources];
    let mut parents = BTreeSet::new();
    for source in &prepared.sources {
        let segments = source.path.split('/').collect::<Vec<_>>();
        let mut current = String::new();
        for segment in &segments[..segments.len().saturating_sub(1)] {
            if !current.is_empty() {
                current.push('/');
            }
            current.push_str(segment);
            parents.insert(current.clone());
        }
    }
    let mut parents = parents.into_iter().collect::<Vec<_>>();
    parents.sort_by_key(|path| (path.split('/').count(), path.clone()));
    for parent in parents {
        let (container_path, leaf) = parent
            .rsplit_once('/')
            .map_or(("", parent.as_str()), |(container, leaf)| (container, leaf));
        let container_index = *held_parent_indexes
            .get(container_path)
            .ok_or_else(|| authentication("created source parent authority is absent"))?;
        let container = &held_directories[container_index];
        fs::mkdirat(container, leaf.as_bytes(), Mode::from_raw_mode(0o700)).map_err(|error| {
            io(format!(
                "cannot create Project Revision Store source path: {error}"
            ))
        })?;
        let held = open_directory_at(container, leaf.as_bytes())?;
        let held_identity = identity(&held)?;
        require_identity_at(
            container,
            leaf.as_bytes(),
            held_identity,
            "created source directory path changed",
        )?;
        created.insert(format!("sources/{parent}"), (true, held_identity));
        let held_index = held_directories.len();
        held_directories.push(held);
        held_parent_indexes.insert(parent, held_index);
    }
    for source in &prepared.sources {
        let (parent_path, leaf) = source
            .path
            .rsplit_once('/')
            .map_or(("", source.path.as_str()), |(parent, leaf)| (parent, leaf));
        let parent_index = *held_parent_indexes
            .get(parent_path)
            .ok_or_else(|| authentication("created source parent authority is absent"))?;
        let parent = &held_directories[parent_index];
        let file_identity = write_file(parent, leaf.as_bytes(), &source.source)?;
        created.insert(format!("sources/{}", source.path), (false, file_identity));
    }
    Ok(CreatedEntry {
        inventory: created,
        directories: held_directories,
    })
}

fn sync_created_directories(
    stage: &OwnedFd,
    directories: &[OwnedFd],
) -> Result<(), Vec<Diagnostic>> {
    for directory in directories.iter().rev() {
        fs::fsync(directory).map_err(|error| {
            io(format!(
                "cannot settle Project Revision Store directory: {error}"
            ))
        })?;
    }
    fs::fsync(stage).map_err(|error| {
        io(format!(
            "cannot settle Project Revision Store stage directory: {error}"
        ))
    })
}

fn authenticate_prepared(
    directory: &OwnedFd,
    prepared: &PreparedEntry,
) -> Result<(), Vec<Diagnostic>> {
    let stored = read_stored(directory)?;
    if stored.entry_json != prepared.entry_json
        || stored.manifest != prepared.manifest
        || stored.workspace_manifest != prepared.workspace_manifest
        || stored.sources.len() != prepared.sources.len()
        || stored
            .sources
            .iter()
            .zip(&prepared.sources)
            .any(|((path, bytes), source)| path != &source.path || bytes != &source.source)
    {
        return Err(authentication(
            "Project Revision Store entry bytes differ from the prepared subject",
        ));
    }
    let replayed =
        super::replay_stored(stored, &prepared.entry_digest, &prepared.project_revision)?;
    if replayed.project_revision() != prepared.project_revision
        || replayed.workspace_revision() != prepared.workspace_revision
        || replayed.semantic_graph_digest() != prepared.project_graph_digest
    {
        return Err(authentication(
            "Project Revision Store rebuilt subject differs from the prepared subject",
        ));
    }
    Ok(())
}

fn read_stored(directory: &OwnedFd) -> Result<StoredEntry, Vec<Diagnostic>> {
    let before = identity(directory)?;
    let initial_inventory = entry_inventory(directory)?;
    let entry_json = read_file(directory, "entry.json", MAX_STORE_ENTRY_JSON_BYTES)?;
    let source_inventory = source_inventory(&entry_json)?;
    let paths = source_inventory
        .iter()
        .map(|(path, _)| path.clone())
        .collect::<Vec<_>>();
    let expected = expected_inventory(&paths)?;
    require_inventory_shape(&initial_inventory, &expected)?;
    let manifest = read_file(directory, "semaprax.toml", MAX_STORE_MANIFEST_BYTES)?;
    let workspace_manifest = read_file(
        directory,
        "workspace-manifest.json",
        MAX_STORE_WORKSPACE_MANIFEST_BYTES,
    )?;
    let sources_root = open_directory_at(directory, b"sources")?;
    let mut total = 0usize;
    let mut sources = Vec::with_capacity(paths.len());
    for (path, expected_bytes) in source_inventory {
        let remaining = MAX_STORE_TOTAL_SOURCE_BYTES - total;
        if expected_bytes > remaining {
            return Err(super::limit(
                "total_source_bytes",
                MAX_STORE_TOTAL_SOURCE_BYTES,
            ));
        }
        let bytes = read_nested_file(&sources_root, &path, expected_bytes)?;
        total = total
            .checked_add(bytes.len())
            .ok_or_else(|| super::limit("total_source_bytes", MAX_STORE_TOTAL_SOURCE_BYTES))?;
        if total > MAX_STORE_TOTAL_SOURCE_BYTES {
            return Err(super::limit(
                "total_source_bytes",
                MAX_STORE_TOTAL_SOURCE_BYTES,
            ));
        }
        sources.push((path, bytes));
    }
    let after_inventory = require_entry_inventory(directory, &expected)?;
    if after_inventory != initial_inventory {
        return Err(authentication(
            "Project Revision Store recursive identity or permission inventory changed",
        ));
    }
    require_identity(directory, before, "store entry changed while reading")?;
    Ok(StoredEntry {
        entry_json,
        manifest,
        workspace_manifest,
        sources,
    })
}

fn expected_inventory(paths: &[String]) -> Result<BTreeMap<String, bool>, Vec<Diagnostic>> {
    let mut expected = BTreeMap::from([
        ("entry.json".to_owned(), false),
        ("semaprax.toml".to_owned(), false),
        ("workspace-manifest.json".to_owned(), false),
        ("sources".to_owned(), true),
    ]);
    for path in paths {
        let mut current = "sources".to_owned();
        let segments = path.split('/').collect::<Vec<_>>();
        for segment in &segments[..segments.len().saturating_sub(1)] {
            current.push('/');
            current.push_str(segment);
            expected.insert(current.clone(), true);
        }
        expected.insert(format!("sources/{path}"), false);
    }
    if expected.len() > MAX_STORE_INVENTORY_ENTRIES {
        return Err(super::limit(
            "inventory_entries",
            MAX_STORE_INVENTORY_ENTRIES,
        ));
    }
    Ok(expected)
}

fn require_entry_inventory(
    directory: &OwnedFd,
    expected: &BTreeMap<String, bool>,
) -> Result<BTreeMap<String, (bool, Identity)>, Vec<Diagnostic>> {
    let observed = entry_inventory(directory)?;
    require_inventory_shape(&observed, expected)?;
    Ok(observed)
}

fn entry_inventory(
    directory: &OwnedFd,
) -> Result<BTreeMap<String, (bool, Identity)>, Vec<Diagnostic>> {
    let mut observed = BTreeMap::new();
    walk(directory, "", 0, &mut observed)?;
    Ok(observed)
}

fn require_inventory_shape(
    observed: &BTreeMap<String, (bool, Identity)>,
    expected: &BTreeMap<String, bool>,
) -> Result<(), Vec<Diagnostic>> {
    let observed_shape = observed
        .iter()
        .map(|(path, (directory, _))| (path.clone(), *directory))
        .collect::<BTreeMap<_, _>>();
    if observed_shape != *expected {
        return Err(authentication(
            "Project Revision Store recursive inventory is not exact",
        ));
    }
    Ok(())
}

fn require_exact_inventory(
    directory: &OwnedFd,
    expected: &BTreeMap<String, bool>,
    identity_inventory: &BTreeMap<String, (bool, Identity)>,
) -> Result<(), Vec<Diagnostic>> {
    let observed = require_entry_inventory(directory, expected)?;
    if observed != *identity_inventory {
        return Err(authentication(
            "Project Revision Store created identity inventory changed",
        ));
    }
    Ok(())
}

fn walk(
    directory: &OwnedFd,
    prefix: &str,
    depth: usize,
    observed: &mut BTreeMap<String, (bool, Identity)>,
) -> Result<(), Vec<Diagnostic>> {
    if depth > super::MAX_STORE_SOURCE_PATH_DEPTH + 1 {
        return Err(super::limit(
            "source_path_depth",
            super::MAX_STORE_SOURCE_PATH_DEPTH,
        ));
    }
    for name in directory_names(directory)? {
        let path = if prefix.is_empty() {
            name.clone()
        } else {
            format!("{prefix}/{name}")
        };
        let stat =
            fs::statat(directory, name.as_bytes(), AtFlags::SYMLINK_NOFOLLOW).map_err(|error| {
                authentication(format!("cannot inspect stored inventory entry: {error}"))
            })?;
        let file_type = FileType::from_raw_mode(stat.st_mode);
        if !file_type.is_dir() && !file_type.is_file() {
            return Err(authentication(
                "Project Revision Store inventory contains a link or special object",
            ));
        }
        let permissions = stat.st_mode as u32 & 0o777;
        if (file_type.is_dir() && permissions != 0o700)
            || (file_type.is_file() && permissions != 0o600)
        {
            return Err(authentication(
                "Project Revision Store inventory permissions are not exact",
            ));
        }
        if file_type.is_file() && stat.st_nlink != 1 {
            return Err(authentication(
                "Project Revision Store file must have exactly one hard link",
            ));
        }
        observed.insert(
            path.clone(),
            (
                file_type.is_dir(),
                Identity {
                    device: stat.st_dev as u64,
                    inode: stat.st_ino as u64,
                    mode: stat.st_mode as u32,
                },
            ),
        );
        if observed.len() > MAX_STORE_INVENTORY_ENTRIES {
            return Err(super::limit(
                "inventory_entries",
                MAX_STORE_INVENTORY_ENTRIES,
            ));
        }
        if file_type.is_dir() {
            let child = open_directory_at(directory, name.as_bytes())?;
            walk(&child, &path, depth + 1, observed)?;
        }
    }
    Ok(())
}

fn write_file(parent: &OwnedFd, name: &[u8], bytes: &[u8]) -> Result<Identity, Vec<Diagnostic>> {
    let fd = fs::openat(
        parent,
        name,
        OFlags::RDWR | OFlags::CREATE | OFlags::EXCL | OFlags::NOFOLLOW | OFlags::CLOEXEC,
        Mode::from_raw_mode(0o600),
    )
    .map_err(|error| {
        io(format!(
            "cannot create Project Revision Store file: {error}"
        ))
    })?;
    let expected_identity = identity(&fd)?;
    let mut file = std::fs::File::from(fd);
    file.write_all(bytes)
        .and_then(|()| file.sync_all())
        .map_err(|error| {
            io(format!(
                "cannot settle Project Revision Store file: {error}"
            ))
        })?;
    file.seek(SeekFrom::Start(0)).map_err(|error| {
        io(format!(
            "cannot replay Project Revision Store file: {error}"
        ))
    })?;
    let mut observed = Vec::with_capacity(bytes.len());
    file.read_to_end(&mut observed).map_err(|error| {
        io(format!(
            "cannot replay Project Revision Store file: {error}"
        ))
    })?;
    if observed != bytes || identity(&file)? != expected_identity {
        return Err(authentication(
            "Project Revision Store file changed while being settled",
        ));
    }
    require_identity_at(
        parent,
        name,
        expected_identity,
        "Project Revision Store file path changed while being settled",
    )?;
    Ok(expected_identity)
}

fn read_nested_file(root: &OwnedFd, path: &str, limit: usize) -> Result<Vec<u8>, Vec<Diagnostic>> {
    let (parent, leaf) = open_parent(root, path)?;
    read_file(&parent, &leaf, limit)
}

fn read_file(parent: &OwnedFd, name: &str, limit: usize) -> Result<Vec<u8>, Vec<Diagnostic>> {
    let fd = fs::openat(
        parent,
        name.as_bytes(),
        OFlags::RDONLY | OFlags::NOFOLLOW | OFlags::CLOEXEC,
        Mode::empty(),
    )
    .map_err(|error| io(format!("cannot open Project Revision Store file: {error}")))?;
    let expected_identity = identity(&fd)?;
    let stat = fs::fstat(&fd)
        .map_err(|error| authentication(format!("cannot inspect stored file: {error}")))?;
    if !FileType::from_raw_mode(stat.st_mode).is_file()
        || stat.st_nlink != 1
        || stat.st_mode as u32 & 0o777 != 0o600
    {
        return Err(authentication(
            "Project Revision Store input is not one regular non-linked file",
        ));
    }
    if stat.st_size < 0 || stat.st_size as u64 > limit as u64 {
        return Err(super::limit("stored_file_bytes", limit));
    }
    let mut file = std::fs::File::from(fd);
    let mut bytes = Vec::with_capacity(stat.st_size as usize);
    std::io::Read::by_ref(&mut file)
        .take(limit as u64 + 1)
        .read_to_end(&mut bytes)
        .map_err(|error| io(format!("cannot read Project Revision Store file: {error}")))?;
    if bytes.len() > limit {
        return Err(super::limit("stored_file_bytes", limit));
    }
    if identity(&file)? != expected_identity {
        return Err(authentication(
            "Project Revision Store file identity changed while reading",
        ));
    }
    require_identity_at(
        parent,
        name.as_bytes(),
        expected_identity,
        "Project Revision Store file path changed while reading",
    )?;
    Ok(bytes)
}

fn require_entry_directory_mode(directory: &OwnedFd) -> Result<(), Vec<Diagnostic>> {
    let stat = fs::fstat(directory)
        .map_err(|error| authentication(format!("cannot inspect store directory: {error}")))?;
    if !FileType::from_raw_mode(stat.st_mode).is_dir() || stat.st_mode as u32 & 0o777 != 0o700 {
        return Err(authentication(
            "Project Revision Store entry directory permissions are not exact",
        ));
    }
    Ok(())
}

fn open_parent(root: &OwnedFd, path: &str) -> Result<(OwnedFd, String), Vec<Diagnostic>> {
    let mut current = open_directory_at(root, b".")?;
    let mut segments = path.split('/').peekable();
    let mut leaf = None;
    while let Some(segment) = segments.next() {
        if segments.peek().is_none() {
            leaf = Some(segment.to_owned());
        } else {
            current = open_directory_at(&current, segment.as_bytes())?;
        }
    }
    Ok((
        current,
        leaf.ok_or_else(|| authentication("stored path has no leaf"))?,
    ))
}

fn open_directory_at<Fd: AsFd>(parent: Fd, name: &[u8]) -> Result<OwnedFd, Vec<Diagnostic>> {
    fs::openat(
        parent,
        name,
        OFlags::RDONLY | OFlags::DIRECTORY | OFlags::NOFOLLOW | OFlags::CLOEXEC,
        Mode::empty(),
    )
    .map_err(|error| authentication(format!("cannot hold store directory: {error}")))
}

fn identity<Fd: AsFd>(fd: Fd) -> Result<Identity, Vec<Diagnostic>> {
    let stat = fs::fstat(fd)
        .map_err(|error| authentication(format!("cannot identify store object: {error}")))?;
    Ok(Identity {
        device: stat.st_dev as u64,
        inode: stat.st_ino as u64,
        mode: stat.st_mode as u32,
    })
}

fn identity_at<Fd: AsFd>(parent: Fd, name: &[u8]) -> Result<Identity, Vec<Diagnostic>> {
    let stat = fs::statat(parent, name, AtFlags::SYMLINK_NOFOLLOW)
        .map_err(|error| authentication(format!("cannot identify store path: {error}")))?;
    Ok(Identity {
        device: stat.st_dev as u64,
        inode: stat.st_ino as u64,
        mode: stat.st_mode as u32,
    })
}

fn require_identity<Fd: AsFd>(
    fd: Fd,
    expected: Identity,
    message: &str,
) -> Result<(), Vec<Diagnostic>> {
    if identity(fd)? != expected {
        return Err(authentication(message));
    }
    Ok(())
}

fn require_identity_at<Fd: AsFd>(
    parent: Fd,
    name: &[u8],
    expected: Identity,
    message: &str,
) -> Result<(), Vec<Diagnostic>> {
    if identity_at(parent, name)? != expected {
        return Err(authentication(message));
    }
    Ok(())
}
