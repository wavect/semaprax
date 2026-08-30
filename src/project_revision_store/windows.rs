use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use semaprax_project_revision_store_windows_sys as sys;

use super::{
    authentication, io, post_pivot, PreparedEntry, StoredEntry, MAX_STORE_ENTRIES,
    MAX_STORE_ENTRY_JSON_BYTES, MAX_STORE_INVENTORY_ENTRIES, MAX_STORE_MANIFEST_BYTES,
    MAX_STORE_TOTAL_SOURCE_BYTES, MAX_STORE_WORKSPACE_MANIFEST_BYTES,
};
use crate::diagnostic::Diagnostic;

type Inventory = BTreeMap<String, (sys::Kind, sys::Fact)>;

struct CreatedEntry {
    inventory: Inventory,
    files: Vec<sys::RegularFile>,
    directories: Vec<sys::Directory>,
    sources: sys::Directory,
}

impl CreatedEntry {
    fn authenticate(
        &self,
        root: &sys::Root,
        prepared: &PreparedEntry,
    ) -> Result<(), Vec<Diagnostic>> {
        let expected = [
            &prepared.entry_json,
            &prepared.manifest,
            &prepared.workspace_manifest,
        ]
        .into_iter()
        .map(Vec::as_slice)
        .chain(
            prepared
                .sources
                .iter()
                .map(|source| source.source.as_slice()),
        )
        .collect::<Vec<_>>();
        if expected.len() != self.files.len() {
            return Err(authentication(
                "Project Revision Store created file inventory changed",
            ));
        }
        for (file, bytes) in self.files.iter().zip(expected) {
            if file.read_bounded(root, bytes.len()).map_err(map_auth)? != bytes {
                return Err(authentication(
                    "Project Revision Store held created file bytes changed",
                ));
            }
        }
        for directory in &self.directories {
            directory.recheck_against(root).map_err(map_auth)?;
        }
        self.sources.recheck_against(root).map_err(map_auth)
    }

    fn settle(self) -> Result<(), Vec<Diagnostic>> {
        for file in self.files {
            file.settle().map_err(map_pre)?;
        }
        for directory in self.directories.into_iter().rev() {
            directory.settle().map_err(map_pre)?;
        }
        self.sources.settle().map_err(map_pre)
    }
}

pub(super) fn persist(root_path: &Path, prepared: &PreparedEntry) -> Result<(), Vec<Diagnostic>> {
    let root = sys::hold_root(root_path).map_err(map_auth)?;
    let initial = root_inventory(&root, false)?;
    if published_names(&initial).len() >= MAX_STORE_ENTRIES {
        return Err(super::limit("retained_entries", MAX_STORE_ENTRIES));
    }
    let destination = prepared.entry_hex();
    if initial.contains_key(destination) {
        return Err(authentication(
            "Project Revision Store content-addressed destination already exists",
        ));
    }
    root.recheck_path(root_path).map_err(map_auth)?;
    let stage_name = format!(".stage-{destination}");
    let stage = root.create_directory(&stage_name).map_err(map_pre)?;
    let stage_fact = stage.fact();
    require_root_inventory(
        &root,
        with_name(&initial, &stage_name, sys::Kind::Directory, stage_fact)?,
    )?;

    let created = write_entry(&root, &stage, prepared)?;
    stage.flush().map_err(map_pre)?;
    root.flush().map_err(map_pre)?;
    created.authenticate(&root, prepared)?;
    authenticate_prepared(&root, &stage, prepared)?;
    require_exact_inventory(&root, &stage, &created.inventory)?;
    root.recheck_path(root_path).map_err(map_auth)?;
    require_root_inventory(
        &root,
        with_name(&initial, &stage_name, sys::Kind::Directory, stage_fact)?,
    )?;

    created.authenticate(&root, prepared)?;
    let created_inventory = created.inventory.clone();
    created.settle()?;
    match root.rename_no_replace(&stage, destination) {
        Ok(()) => {}
        Err(sys::Error::Exists) => {
            return Err(io(
                "cannot publish Project Revision Store entry without replacement",
            ))
        }
        Err(sys::Error::Uncertain) => {
            return Err(post_pivot(
                "Project Revision Store publication rename is uncertain",
            ))
        }
        Err(error) => {
            return Err(io(format!(
                "cannot publish Project Revision Store entry: {error:?}"
            )))
        }
    }
    root.flush()
        .map_err(|_| post_pivot("cannot settle Project Revision Store publication"))?;
    let published = root
        .open_directory(destination)
        .map_err(|_| post_pivot("cannot reopen published Project Revision Store entry"))?;
    if published.fact() != stage_fact {
        return Err(post_pivot(
            "published Project Revision Store identity differs from the exact stage",
        ));
    }
    authenticate_prepared(&root, &published, prepared).map_err(|_| {
        post_pivot("published Project Revision Store entry failed exact authentication")
    })?;
    require_exact_inventory(&root, &published, &created_inventory)
        .map_err(|_| post_pivot("published Project Revision Store inventory changed"))?;
    root.recheck_path(root_path)
        .map_err(|_| post_pivot("Project Revision Store root path changed after publication"))?;
    let mut final_inventory = initial;
    final_inventory.insert(destination.to_owned(), (sys::Kind::Directory, stage_fact));
    require_root_inventory(&root, final_inventory).map_err(|_| {
        post_pivot("Project Revision Store root inventory changed after publication")
    })?;
    published
        .settle()
        .map_err(|_| post_pivot("cannot close published Project Revision Store entry"))?;
    stage
        .settle()
        .map_err(|_| post_pivot("cannot close renamed Project Revision Store stage"))?;
    root.settle()
        .map_err(|_| post_pivot("cannot release Project Revision Store root authority"))
}

pub(super) fn load(root_path: &Path, entry_digest: &str) -> Result<StoredEntry, Vec<Diagnostic>> {
    let root = sys::hold_root(root_path).map_err(map_auth)?;
    let inventory = root_inventory(&root, true)?;
    let entry_hex = entry_digest
        .strip_prefix("sha256:")
        .ok_or_else(|| authentication("Project Revision Store digest prefix is absent"))?;
    let Some((sys::Kind::Directory, expected)) = inventory.get(entry_hex).copied() else {
        return Err(authentication(
            "Project Revision Store entry is absent from the exact root inventory",
        ));
    };
    let entry = root.open_directory(entry_hex).map_err(map_auth)?;
    if entry.fact() != expected {
        return Err(authentication(
            "Project Revision Store entry identity changed",
        ));
    }
    let stored = read_stored(&root, &entry)?;
    root.recheck_path(root_path).map_err(map_auth)?;
    require_root_inventory(&root, inventory)?;
    entry.settle().map_err(map_pre)?;
    root.settle().map_err(map_pre)?;
    Ok(stored)
}

fn root_inventory(root: &sys::Root, allow_stage: bool) -> Result<Inventory, Vec<Diagnostic>> {
    let mut output = Inventory::new();
    let mut stages = 0usize;
    for item in root.inventory().map_err(map_auth)? {
        let name = item.name();
        let valid_entry = canonical_entry_hex(name);
        let valid_stage = name
            .strip_prefix(".stage-")
            .is_some_and(canonical_entry_hex);
        if valid_stage {
            stages += 1;
            if !allow_stage || stages > 1 || item.kind() != sys::Kind::Directory {
                return Err(authentication(
                    "Project Revision Store root contains an inadmissible stage",
                ));
            }
        } else if !valid_entry || item.kind() != sys::Kind::Directory {
            return Err(authentication(
                "Project Revision Store root inventory is not closed",
            ));
        }
        output.insert(name.to_owned(), (item.kind(), item.fact()));
    }
    if published_names(&output).len() > MAX_STORE_ENTRIES {
        return Err(super::limit("retained_entries", MAX_STORE_ENTRIES));
    }
    for name in published_names(&output) {
        let directory = root.open_directory(name).map_err(map_auth)?;
        let before = walk_facts(root, &directory, "", &mut 0usize)?;
        let entry_json = read_file(root, &directory, "entry.json", MAX_STORE_ENTRY_JSON_BYTES)?;
        let expected = super::framed_digest(super::WINDOWS_ENTRY_DIGEST_DOMAIN, &entry_json);
        if expected.strip_prefix("sha256:") != Some(name) {
            return Err(authentication(
                "retained Project Revision Store entry digest differs from its name",
            ));
        }
        let header =
            super::parse_entry_header_for_profile(&entry_json, super::EntryProfile::Windows)?;
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
            profile: super::EntryProfile::Windows,
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
            entry_digest: expected.clone(),
        };
        if super::render_entry_fixed_point(&prepared)? != entry_json {
            return Err(authentication(
                "retained Project Revision Store metadata is not canonical",
            ));
        }
        let expected_inventory =
            expected_inventory(header.sources.iter().map(|source| source.path.as_str()))?;
        require_inventory_shape(root, &directory, &expected_inventory)?;
        for (path, length) in [
            ("semaprax.toml", header.manifest_bytes),
            ("workspace-manifest.json", header.workspace_manifest_bytes),
        ] {
            if before.get(path).map(|(_, fact)| fact.length()) != Some(length as u64) {
                return Err(authentication(
                    "retained Project Revision Store fixed-file size changed",
                ));
            }
        }
        for source in &header.sources {
            if before
                .get(&format!("sources/{}", source.path))
                .map(|(_, fact)| fact.length())
                != Some(source.bytes as u64)
            {
                return Err(authentication(
                    "retained Project Revision Store source size changed",
                ));
            }
        }
        if walk_facts(root, &directory, "", &mut 0usize)? != before {
            return Err(authentication(
                "retained Project Revision Store structural inventory changed",
            ));
        }
        directory.settle().map_err(map_pre)?;
    }
    Ok(output)
}

fn published_names(inventory: &Inventory) -> Vec<&str> {
    inventory
        .keys()
        .filter(|name| canonical_entry_hex(name))
        .map(String::as_str)
        .collect()
}

fn canonical_entry_hex(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

fn with_name(
    initial: &Inventory,
    name: &str,
    kind: sys::Kind,
    fact: sys::Fact,
) -> Result<Inventory, Vec<Diagnostic>> {
    let mut output = initial.clone();
    if output.insert(name.to_owned(), (kind, fact)).is_some() {
        return Err(authentication(
            "Project Revision Store stage unexpectedly existed",
        ));
    }
    Ok(output)
}

fn require_root_inventory(root: &sys::Root, expected: Inventory) -> Result<(), Vec<Diagnostic>> {
    let observed = root
        .inventory()
        .map_err(map_auth)?
        .into_iter()
        .map(|item| (item.name().to_owned(), (item.kind(), item.fact())))
        .collect::<Inventory>();
    if observed != expected {
        return Err(authentication(
            "Project Revision Store root inventory changed",
        ));
    }
    Ok(())
}

fn write_entry(
    root: &sys::Root,
    stage: &sys::Directory,
    prepared: &PreparedEntry,
) -> Result<CreatedEntry, Vec<Diagnostic>> {
    let sources = stage.create_directory(root, "sources").map_err(map_pre)?;
    let mut created = CreatedEntry {
        inventory: Inventory::new(),
        files: Vec::new(),
        directories: Vec::new(),
        sources,
    };
    created.inventory.insert(
        "sources".to_owned(),
        (sys::Kind::Directory, created.sources.fact()),
    );
    write_file(
        root,
        stage,
        "entry.json",
        &prepared.entry_json,
        "entry.json",
        &mut created.inventory,
        &mut created.files,
    )?;
    write_file(
        root,
        stage,
        "semaprax.toml",
        &prepared.manifest,
        "semaprax.toml",
        &mut created.inventory,
        &mut created.files,
    )?;
    write_file(
        root,
        stage,
        "workspace-manifest.json",
        &prepared.workspace_manifest,
        "workspace-manifest.json",
        &mut created.inventory,
        &mut created.files,
    )?;
    let mut directory_paths = BTreeSet::new();
    for source in &prepared.sources {
        let mut relative = String::new();
        let segments = source.path.split('/').collect::<Vec<_>>();
        for segment in &segments[..segments.len() - 1] {
            if !relative.is_empty() {
                relative.push('/');
            }
            relative.push_str(segment);
            directory_paths.insert(relative.clone());
        }
    }
    let mut directory_indices = BTreeMap::<String, usize>::new();
    for path in directory_paths {
        let (parent_path, name) = path
            .rsplit_once('/')
            .map_or((None, path.as_str()), |(parent, name)| (Some(parent), name));
        let parent = match parent_path {
            Some(parent) => {
                &created.directories[*directory_indices
                    .get(parent)
                    .expect("canonical ancestor was created first")]
            }
            None => &created.sources,
        };
        let directory = parent.create_directory(root, name).map_err(map_pre)?;
        created.inventory.insert(
            format!("sources/{path}"),
            (sys::Kind::Directory, directory.fact()),
        );
        directory_indices.insert(path, created.directories.len());
        created.directories.push(directory);
    }
    for source in &prepared.sources {
        let (parent_path, name) = source
            .path
            .rsplit_once('/')
            .map_or((None, source.path.as_str()), |(parent, name)| {
                (Some(parent), name)
            });
        let path = format!("sources/{}", source.path);
        let parent = match parent_path {
            Some(parent) => {
                &created.directories[*directory_indices
                    .get(parent)
                    .expect("canonical source parent was created")]
            }
            None => &created.sources,
        };
        write_file(
            root,
            parent,
            name,
            &source.source,
            &path,
            &mut created.inventory,
            &mut created.files,
        )?;
    }
    for directory in created.directories.iter().rev() {
        directory.flush().map_err(map_pre)?;
    }
    created.sources.flush().map_err(map_pre)?;
    Ok(created)
}

fn write_file(
    root: &sys::Root,
    parent: &sys::Directory,
    name: &str,
    bytes: &[u8],
    path: &str,
    expected: &mut Inventory,
    files: &mut Vec<sys::RegularFile>,
) -> Result<(), Vec<Diagnostic>> {
    let file = parent.create_file(root, name, bytes).map_err(map_pre)?;
    expected.insert(path.to_owned(), (sys::Kind::File, file.fact()));
    files.push(file);
    Ok(())
}

fn authenticate_prepared(
    root: &sys::Root,
    directory: &sys::Directory,
    prepared: &PreparedEntry,
) -> Result<(), Vec<Diagnostic>> {
    let stored = read_stored(root, directory)?;
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
    let replayed = super::replay_stored_for_profile(
        stored,
        &prepared.entry_digest,
        &prepared.project_revision,
        super::EntryProfile::Windows,
    )?;
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

fn read_stored(
    root: &sys::Root,
    directory: &sys::Directory,
) -> Result<StoredEntry, Vec<Diagnostic>> {
    let before = walk_facts(root, directory, "", &mut 0usize)?;
    let entry_json = read_file(root, directory, "entry.json", MAX_STORE_ENTRY_JSON_BYTES)?;
    let manifest = read_file(root, directory, "semaprax.toml", MAX_STORE_MANIFEST_BYTES)?;
    let workspace_manifest = read_file(
        root,
        directory,
        "workspace-manifest.json",
        MAX_STORE_WORKSPACE_MANIFEST_BYTES,
    )?;
    let source_facts =
        super::parse_entry_header_for_profile(&entry_json, super::EntryProfile::Windows)?
            .sources
            .into_iter()
            .map(|source| (source.path, source.bytes))
            .collect::<Vec<_>>();
    require_inventory_shape(
        root,
        directory,
        &expected_inventory(source_facts.iter().map(|(path, _)| path.as_str()))?,
    )?;
    let sources_directory = directory
        .open_directory(root, "sources")
        .map_err(map_auth)?;
    let mut sources = Vec::with_capacity(source_facts.len());
    let mut total = 0usize;
    for (path, expected) in source_facts {
        let bytes = read_nested_file(root, &sources_directory, &path, expected)?;
        total = total
            .checked_add(bytes.len())
            .ok_or_else(|| super::limit("total_source_bytes", MAX_STORE_TOTAL_SOURCE_BYTES))?;
        sources.push((path, bytes));
    }
    if total > MAX_STORE_TOTAL_SOURCE_BYTES {
        return Err(super::limit(
            "total_source_bytes",
            MAX_STORE_TOTAL_SOURCE_BYTES,
        ));
    }
    sources_directory.settle().map_err(map_pre)?;
    if walk_facts(root, directory, "", &mut 0usize)? != before {
        return Err(authentication(
            "Project Revision Store entry inventory changed while reading",
        ));
    }
    Ok(StoredEntry {
        entry_json,
        manifest,
        workspace_manifest,
        sources,
    })
}

fn read_nested_file(
    root: &sys::Root,
    base: &sys::Directory,
    path: &str,
    limit: usize,
) -> Result<Vec<u8>, Vec<Diagnostic>> {
    let segments = path.split('/').collect::<Vec<_>>();
    read_nested_segments(root, base, &segments, limit)
}

fn read_nested_segments(
    root: &sys::Root,
    parent: &sys::Directory,
    segments: &[&str],
    limit: usize,
) -> Result<Vec<u8>, Vec<Diagnostic>> {
    if segments.len() == 1 {
        return read_file(root, parent, segments[0], limit);
    }
    let child = parent.open_directory(root, segments[0]).map_err(map_auth)?;
    let bytes = read_nested_segments(root, &child, &segments[1..], limit)?;
    child.settle().map_err(map_pre)?;
    Ok(bytes)
}

fn read_file(
    root: &sys::Root,
    parent: &sys::Directory,
    name: &str,
    limit: usize,
) -> Result<Vec<u8>, Vec<Diagnostic>> {
    let file = parent.open_file(root, name).map_err(map_auth)?;
    let bytes = file.read_bounded(root, limit).map_err(map_auth)?;
    file.settle().map_err(map_pre)?;
    Ok(bytes)
}

fn expected_inventory<'a>(
    paths: impl Iterator<Item = &'a str>,
) -> Result<BTreeMap<String, sys::Kind>, Vec<Diagnostic>> {
    let mut output = BTreeMap::new();
    output.insert("entry.json".to_owned(), sys::Kind::File);
    output.insert("semaprax.toml".to_owned(), sys::Kind::File);
    output.insert("workspace-manifest.json".to_owned(), sys::Kind::File);
    output.insert("sources".to_owned(), sys::Kind::Directory);
    for path in paths {
        let segments = path.split('/').collect::<Vec<_>>();
        let mut current = String::from("sources");
        for segment in &segments[..segments.len() - 1] {
            current.push('/');
            current.push_str(segment);
            output.insert(current.clone(), sys::Kind::Directory);
        }
        current.push('/');
        current.push_str(segments.last().expect("validated source path"));
        output.insert(current, sys::Kind::File);
    }
    if output.len() > MAX_STORE_INVENTORY_ENTRIES {
        return Err(super::limit(
            "inventory_entries",
            MAX_STORE_INVENTORY_ENTRIES,
        ));
    }
    Ok(output)
}

fn require_inventory_shape(
    root: &sys::Root,
    directory: &sys::Directory,
    expected: &BTreeMap<String, sys::Kind>,
) -> Result<(), Vec<Diagnostic>> {
    let observed = walk(root, directory, "", &mut 0usize)?;
    if &observed != expected {
        return Err(authentication(
            "Project Revision Store recursive inventory is not exact",
        ));
    }
    Ok(())
}

fn require_exact_inventory(
    root: &sys::Root,
    directory: &sys::Directory,
    expected: &Inventory,
) -> Result<(), Vec<Diagnostic>> {
    let observed_shape = walk_facts(root, directory, "", &mut 0usize)?;
    if &observed_shape != expected {
        return Err(authentication(
            "Project Revision Store recursive identity inventory changed",
        ));
    }
    Ok(())
}

fn walk(
    root: &sys::Root,
    directory: &sys::Directory,
    prefix: &str,
    count: &mut usize,
) -> Result<BTreeMap<String, sys::Kind>, Vec<Diagnostic>> {
    Ok(walk_facts(root, directory, prefix, count)?
        .into_iter()
        .map(|(path, (kind, _))| (path, kind))
        .collect())
}

fn walk_facts(
    root: &sys::Root,
    directory: &sys::Directory,
    prefix: &str,
    count: &mut usize,
) -> Result<Inventory, Vec<Diagnostic>> {
    let mut output = Inventory::new();
    for item in directory.inventory(root).map_err(map_auth)? {
        *count = count
            .checked_add(1)
            .ok_or_else(|| super::limit("inventory_entries", MAX_STORE_INVENTORY_ENTRIES))?;
        if *count > MAX_STORE_INVENTORY_ENTRIES {
            return Err(super::limit(
                "inventory_entries",
                MAX_STORE_INVENTORY_ENTRIES,
            ));
        }
        let path = if prefix.is_empty() {
            item.name().to_owned()
        } else {
            format!("{prefix}/{}", item.name())
        };
        output.insert(path.clone(), (item.kind(), item.fact()));
        if item.kind() == sys::Kind::Directory {
            let child = directory
                .open_directory(root, item.name())
                .map_err(map_auth)?;
            output.extend(walk_facts(root, &child, &path, count)?);
            child.settle().map_err(map_pre)?;
        }
    }
    Ok(output)
}

fn map_auth(error: sys::Error) -> Vec<Diagnostic> {
    if error == sys::Error::Limit {
        return vec![Diagnostic::io(
            "SPX-G191",
            "Project Revision Store Windows authority bound exceeded",
        )];
    }
    authentication(format!(
        "Project Revision Store Windows authority rejected: {error:?}"
    ))
}

fn map_pre(error: sys::Error) -> Vec<Diagnostic> {
    match error {
        sys::Error::Invalid
        | sys::Error::Limit
        | sys::Error::Busy
        | sys::Error::Exists
        | sys::Error::Changed => map_auth(error),
        sys::Error::Io | sys::Error::Uncertain => io(format!(
            "Project Revision Store Windows filesystem failure: {error:?}"
        )),
    }
}
