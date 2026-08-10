use std::collections::BTreeSet;
use std::ffi::OsString;
use std::fs::{File, Metadata, OpenOptions, Permissions};
use std::io::{Read, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};

use crate::ast::{Type, TypeDeclaration, TypeDeclarationKind};
use crate::diagnostic::Diagnostic;
use crate::{format, graph, lexer, parse, verify};

#[derive(Debug)]
struct Rename {
    stable_id: String,
    new_name: String,
}

#[derive(Debug)]
struct SemanticPatch {
    base: String,
    renames: Vec<Rename>,
    no_new_effects: bool,
}

const STAGING_ATTEMPTS: usize = 32;

// Unix uses device/inode identity. Windows retains a `same_file::Handle` so
// volume/file-index reuse cannot occur while a transaction is live. The
// Windows key is intentionally a bounded identity: ReFS 128-bit file IDs and
// hostile/non-unique 64-bit indices are outside this portable protocol's claim.
#[derive(Debug, Eq, PartialEq)]
struct FileIdentity {
    #[cfg(unix)]
    device: u64,
    #[cfg(unix)]
    inode: u64,
    #[cfg(windows)]
    handle: same_file::Handle,
}

struct SourceSnapshot {
    source: String,
    identity: FileIdentity,
    permissions: Permissions,
}

struct OwnedSiblingFile {
    path: PathBuf,
    identity: FileIdentity,
    file: Option<File>,
    cleanup: bool,
}

struct ProvisionalSiblingFile {
    path: PathBuf,
    file: Option<File>,
    cleanup: bool,
}

impl ProvisionalSiblingFile {
    fn into_owned(mut self, identity: FileIdentity) -> OwnedSiblingFile {
        self.cleanup = false;
        OwnedSiblingFile {
            path: self.path.clone(),
            identity,
            file: self.file.take(),
            cleanup: true,
        }
    }
}

impl Drop for ProvisionalSiblingFile {
    fn drop(&mut self) {
        if !self.cleanup {
            return;
        }
        let Some(handle_identity) = self.file.as_ref().and_then(|file| {
            file.metadata()
                .ok()
                .and_then(|metadata| platform_handle_identity(file, &metadata).ok())
        }) else {
            self.file.take();
            return;
        };
        self.file.take();
        let Ok(metadata) = std::fs::symlink_metadata(&self.path) else {
            return;
        };
        if regular_metadata(&metadata)
            && platform_path_identity(&self.path, &metadata).ok() == Some(handle_identity)
        {
            let _ = std::fs::remove_file(&self.path);
        }
    }
}

impl OwnedSiblingFile {
    fn create(
        path: PathBuf,
        diagnostic_code: &'static str,
        description: &str,
        existing_is_collision: bool,
    ) -> Result<Option<Self>, Vec<Diagnostic>> {
        let file = match OpenOptions::new()
            .read(true)
            .write(true)
            .create_new(true)
            .open(&path)
        {
            Ok(file) => file,
            Err(error)
                if existing_is_collision && error.kind() == std::io::ErrorKind::AlreadyExists =>
            {
                return Ok(None);
            }
            Err(error) => {
                return Err(vec![Diagnostic::io(
                    diagnostic_code,
                    format!("cannot create {description} {}: {error}", path.display()),
                )]);
            }
        };
        // Install cleanup ownership immediately after create_new. Held identity
        // replaces this short provisional guard as soon as handle metadata is
        // available; the containing directory is the documented portable trust
        // boundary for these path-based operations.
        let provisional = ProvisionalSiblingFile {
            path,
            file: Some(file),
            cleanup: true,
        };
        let metadata = provisional
            .file
            .as_ref()
            .expect("new sibling file remains open")
            .metadata()
            .map_err(|error| {
                vec![Diagnostic::io(
                    diagnostic_code,
                    format!(
                        "cannot inspect {description} {}: {error}",
                        provisional.path.display()
                    ),
                )]
            })?;
        validate_regular_metadata(&metadata, &provisional.path, description, diagnostic_code)?;
        let identity = file_handle_identity(
            provisional
                .file
                .as_ref()
                .expect("new sibling file remains open"),
            &metadata,
            &provisional.path,
            description,
            diagnostic_code,
        )?;
        let owned = provisional.into_owned(identity);
        let path_metadata = safe_symlink_metadata(&owned.path, description, diagnostic_code)?;
        validate_regular_metadata(&path_metadata, &owned.path, description, diagnostic_code)?;
        if file_identity(&path_metadata, &owned.path, description, diagnostic_code)?
            != owned.identity
        {
            return Err(vec![Diagnostic::io(
                diagnostic_code,
                format!(
                    "{description} {} changed during creation",
                    owned.path.display()
                ),
            )]);
        }
        Ok(Some(owned))
    }

    fn file_mut(&mut self) -> Result<&mut File, Vec<Diagnostic>> {
        self.file.as_mut().ok_or_else(|| {
            vec![Diagnostic::io(
                "SPX-I203",
                format!(
                    "semantic patch staging file {} is closed",
                    self.path.display()
                ),
            )]
        })
    }

    fn validate_path(
        &self,
        description: &str,
        diagnostic_code: &'static str,
    ) -> Result<(), Vec<Diagnostic>> {
        let metadata = safe_symlink_metadata(&self.path, description, diagnostic_code)?;
        validate_regular_metadata(&metadata, &self.path, description, diagnostic_code)?;
        if file_identity(&metadata, &self.path, description, diagnostic_code)? != self.identity {
            return Err(vec![Diagnostic::io(
                diagnostic_code,
                format!(
                    "{description} {} changed before commit",
                    self.path.display()
                ),
            )]);
        }
        Ok(())
    }

    fn validate_contents(&mut self, expected: &[u8]) -> Result<(), Vec<Diagnostic>> {
        self.validate_path("semantic patch staging file", "SPX-I203")?;
        let path = self.path.clone();
        let file = self.file_mut()?;
        file.seek(SeekFrom::Start(0)).map_err(|error| {
            vec![Diagnostic::io(
                "SPX-I203",
                format!("cannot rewind semantic patch staging file: {error}"),
            )]
        })?;
        let mut actual = Vec::new();
        file.read_to_end(&mut actual).map_err(|error| {
            vec![Diagnostic::io(
                "SPX-I203",
                format!("cannot verify semantic patch staging file: {error}"),
            )]
        })?;
        if actual != expected {
            return Err(vec![Diagnostic::io(
                "SPX-I203",
                "semantic patch staging bytes changed before commit",
            )]);
        }
        let metadata = file.metadata().map_err(|error| {
            vec![Diagnostic::io(
                "SPX-I203",
                format!("cannot inspect semantic patch staging file: {error}"),
            )]
        })?;
        if file_handle_identity(
            file,
            &metadata,
            &path,
            "semantic patch staging file",
            "SPX-I203",
        )? != self.identity
        {
            return Err(vec![Diagnostic::io(
                "SPX-I203",
                "semantic patch staging handle changed before commit",
            )]);
        }
        Ok(())
    }

    fn committed(&mut self) {
        self.cleanup = false;
        self.file.take();
    }

    fn remove_if_owned(&mut self) {
        self.file.take();
        if !self.cleanup {
            return;
        }
        let Ok(metadata) = std::fs::symlink_metadata(&self.path) else {
            return;
        };
        if !regular_metadata(&metadata) {
            return;
        }
        let Ok(identity) = platform_path_identity(&self.path, &metadata) else {
            return;
        };
        if identity == self.identity {
            let _ = std::fs::remove_file(&self.path);
        }
    }
}

impl Drop for OwnedSiblingFile {
    fn drop(&mut self) {
        self.remove_if_owned();
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum CommitPhase {
    BeforeFinalCheck,
    BeforeRename,
}

pub fn apply(source_path: &Path, patch_path: &Path) -> Result<String, Vec<Diagnostic>> {
    apply_with_commit_hook(source_path, patch_path, |_, _, _| Ok(()))
}

fn apply_with_commit_hook(
    source_path: &Path,
    patch_path: &Path,
    mut hook: impl FnMut(CommitPhase, &Path, &Path) -> std::io::Result<()>,
) -> Result<String, Vec<Diagnostic>> {
    let canonical_source_path = canonical_source_path(source_path)?;
    let lock_path = sibling_path(&canonical_source_path, ".semaprax-patch.lock")?;
    let _lock = OwnedSiblingFile::create(lock_path, "SPX-I205", "semantic patch lock", false)?
        .expect("existing lock is reported as an error");
    let before_snapshot = read_source_snapshot(&canonical_source_path)?;
    let source = before_snapshot.source.clone();
    let patch_source = std::fs::read_to_string(patch_path).map_err(|error| {
        vec![Diagnostic::io(
            "SPX-I202",
            format!("cannot read {}: {error}", patch_path.display()),
        )]
    })?;
    let patch = parse_patch(&patch_source)?;
    let before = parse(&source, source_path).map_err(|error| vec![error])?;
    let revision = graph::revision(&before);
    if revision != patch.base {
        return Err(vec![Diagnostic::io(
            "SPX-G409",
            format!(
                "stale semantic patch: expected graph {}, current graph {revision}",
                patch.base
            ),
        )
        .with_help("regenerate the patch against the current semantic graph")]);
    }

    let before_effects = effect_set(&before);
    let mut replacements = Vec::new();
    let tokens =
        lexer::lex(&source, &source_path.display().to_string()).map_err(|error| vec![error])?;
    for rename in &patch.renames {
        if !is_identifier(&rename.new_name) {
            return Err(vec![Diagnostic::io(
                "SPX-G103",
                format!("`{}` is not a valid symbol name", rename.new_name),
            )]);
        }
        if let Some(function) = before
            .functions
            .iter()
            .find(|function| function.stable_id == rename.stable_id)
        {
            if !function.explicit_id {
                return Err(vec![Diagnostic::io(
                    "SPX-G104",
                    format!(
                        "`{}` needs an explicit @id before it can be renamed",
                        function.name
                    ),
                )]);
            }
            for (start, end) in function_name_positions(&before, &tokens, function) {
                replacements.push((start, end, rename.new_name.clone()));
            }
            continue;
        }
        if let Some(resource) = before.types.iter().find(|declaration| {
            declaration.stable_id == rename.stable_id
                && matches!(declaration.kind, TypeDeclarationKind::Resource { .. })
        }) {
            if !resource.explicit_id {
                return Err(vec![Diagnostic::io(
                    "SPX-G104",
                    format!(
                        "`{}` needs an explicit @id before it can be renamed",
                        resource.name
                    ),
                )]);
            }
            for (start, end) in resource_type_positions(&before, &tokens, resource) {
                replacements.push((start, end, rename.new_name.clone()));
            }
            continue;
        }
        return Err(vec![Diagnostic::io(
            "SPX-G404",
            format!("stable id `{}` does not exist", rename.stable_id),
        )]);
    }
    replacements.sort_by_key(|replacement| replacement.0);
    replacements.dedup_by_key(|replacement| (replacement.0, replacement.1));
    let mut changed = source.clone();
    for (start, end, replacement) in replacements.into_iter().rev() {
        changed.replace_range(start..end, &replacement);
    }

    let after = parse(&changed, source_path).map_err(|error| vec![error])?;
    let diagnostics = verify::verify(&after);
    if diagnostics.iter().any(|item| item.severity.is_error()) {
        return Err(diagnostics);
    }
    if patch.no_new_effects && !effect_set(&after).is_subset(&before_effects) {
        return Err(vec![Diagnostic::io(
            "SPX-G105",
            "semantic patch violates requirement `no-new-effects`",
        )]);
    }
    let canonical = format::canonical(&after);
    let canonical_bytes = canonical.as_bytes();
    let mut staging = create_staging_file(&canonical_source_path)?;
    {
        let file = staging.file_mut()?;
        file.write_all(canonical_bytes).map_err(|error| {
            vec![Diagnostic::io(
                "SPX-I203",
                format!("cannot stage semantic patch: {error}"),
            )]
        })?;
        file.flush().map_err(|error| {
            vec![Diagnostic::io(
                "SPX-I203",
                format!("cannot flush semantic patch staging file: {error}"),
            )]
        })?;
        file.set_permissions(before_snapshot.permissions.clone())
            .map_err(|error| {
                vec![Diagnostic::io(
                    "SPX-I203",
                    format!("cannot preserve semantic patch source permissions: {error}"),
                )]
            })?;
        file.sync_all().map_err(|error| {
            vec![Diagnostic::io(
                "SPX-I203",
                format!("cannot synchronize semantic patch staging file: {error}"),
            )]
        })?;
    }
    staging.validate_contents(canonical_bytes)?;
    hook(
        CommitPhase::BeforeFinalCheck,
        &canonical_source_path,
        &staging.path,
    )
    .map_err(|error| {
        vec![Diagnostic::io(
            "SPX-I207",
            format!("semantic patch pre-commit check failed: {error}"),
        )]
    })?;
    validate_source_unchanged(
        &canonical_source_path,
        source_path,
        &before_snapshot,
        &revision,
    )?;
    staging.validate_contents(canonical_bytes)?;
    hook(
        CommitPhase::BeforeRename,
        &canonical_source_path,
        &staging.path,
    )
    .map_err(|error| {
        vec![Diagnostic::io(
            "SPX-I204",
            format!("cannot atomically commit semantic patch: {error}"),
        )]
    })?;
    validate_source_unchanged(
        &canonical_source_path,
        source_path,
        &before_snapshot,
        &revision,
    )?;
    staging.validate_contents(canonical_bytes)?;
    std::fs::rename(&staging.path, &canonical_source_path).map_err(|error| {
        vec![Diagnostic::io(
            "SPX-I204",
            format!("cannot atomically commit semantic patch: {error}"),
        )]
    })?;
    staging.committed();
    Ok(graph::revision(&after))
}

fn canonical_source_path(source_path: &Path) -> Result<PathBuf, Vec<Diagnostic>> {
    let supplied_metadata =
        safe_symlink_metadata(source_path, "semantic patch source", "SPX-I201")?;
    validate_regular_metadata(
        &supplied_metadata,
        source_path,
        "semantic patch source",
        "SPX-I201",
    )?;
    let supplied_identity = file_identity(
        &supplied_metadata,
        source_path,
        "semantic patch source",
        "SPX-I201",
    )?;
    let canonical = std::fs::canonicalize(source_path).map_err(|error| {
        vec![Diagnostic::io(
            "SPX-I201",
            format!(
                "cannot canonicalize semantic patch source {}: {error}",
                source_path.display()
            ),
        )]
    })?;
    let canonical_metadata =
        safe_symlink_metadata(&canonical, "canonical semantic patch source", "SPX-I201")?;
    validate_regular_metadata(
        &canonical_metadata,
        &canonical,
        "canonical semantic patch source",
        "SPX-I201",
    )?;
    if file_identity(
        &canonical_metadata,
        &canonical,
        "canonical semantic patch source",
        "SPX-I201",
    )? != supplied_identity
    {
        return Err(source_changed_error());
    }
    Ok(canonical)
}

fn validate_source_unchanged(
    canonical_source_path: &Path,
    diagnostic_path: &Path,
    before: &SourceSnapshot,
    revision: &str,
) -> Result<(), Vec<Diagnostic>> {
    let current =
        read_source_snapshot(canonical_source_path).map_err(|_| source_changed_error())?;
    let current_program =
        parse(&current.source, diagnostic_path).map_err(|_| source_changed_error())?;
    if current.identity != before.identity
        || current.source != before.source
        || graph::revision(&current_program) != revision
    {
        return Err(source_changed_error());
    }
    Ok(())
}

fn read_source_snapshot(path: &Path) -> Result<SourceSnapshot, Vec<Diagnostic>> {
    let path_metadata = safe_symlink_metadata(path, "semantic patch source", "SPX-I201")?;
    validate_regular_metadata(&path_metadata, path, "semantic patch source", "SPX-I201")?;
    let path_identity = file_identity(&path_metadata, path, "semantic patch source", "SPX-I201")?;
    let mut file = File::open(path).map_err(|error| {
        vec![Diagnostic::io(
            "SPX-I201",
            format!("cannot read {}: {error}", path.display()),
        )]
    })?;
    let handle_metadata = file.metadata().map_err(|error| {
        vec![Diagnostic::io(
            "SPX-I201",
            format!(
                "cannot inspect semantic patch source {}: {error}",
                path.display()
            ),
        )]
    })?;
    validate_regular_metadata(&handle_metadata, path, "semantic patch source", "SPX-I201")?;
    if file_handle_identity(
        &file,
        &handle_metadata,
        path,
        "semantic patch source",
        "SPX-I201",
    )? != path_identity
    {
        return Err(source_changed_error());
    }
    let mut source = String::new();
    file.read_to_string(&mut source).map_err(|error| {
        vec![Diagnostic::io(
            "SPX-I201",
            format!("cannot read {}: {error}", path.display()),
        )]
    })?;
    let final_path_metadata = safe_symlink_metadata(path, "semantic patch source", "SPX-I201")?;
    validate_regular_metadata(
        &final_path_metadata,
        path,
        "semantic patch source",
        "SPX-I201",
    )?;
    if file_identity(
        &final_path_metadata,
        path,
        "semantic patch source",
        "SPX-I201",
    )? != path_identity
    {
        return Err(source_changed_error());
    }
    Ok(SourceSnapshot {
        source,
        identity: path_identity,
        permissions: path_metadata.permissions(),
    })
}

fn create_staging_file(source_path: &Path) -> Result<OwnedSiblingFile, Vec<Diagnostic>> {
    for attempt in 0..STAGING_ATTEMPTS {
        let suffix = format!(".semaprax-stage.{attempt}.tmp");
        let path = sibling_path(source_path, &suffix)?;
        if let Some(staging) =
            OwnedSiblingFile::create(path, "SPX-I203", "semantic patch staging file", true)?
        {
            return Ok(staging);
        }
    }
    Err(vec![Diagnostic::io(
        "SPX-I203",
        format!(
            "cannot stage semantic patch: all {STAGING_ATTEMPTS} create-new candidates already exist"
        ),
    )])
}

fn sibling_path(source_path: &Path, suffix: &str) -> Result<PathBuf, Vec<Diagnostic>> {
    let Some(name) = source_path.file_name() else {
        return Err(vec![Diagnostic::io(
            "SPX-I201",
            format!(
                "semantic patch source {} has no file name",
                source_path.display()
            ),
        )]);
    };
    let mut sibling = OsString::from(".");
    sibling.push(name);
    sibling.push(suffix);
    Ok(source_path
        .parent()
        .unwrap_or_else(|| Path::new("."))
        .join(sibling))
}

fn safe_symlink_metadata(
    path: &Path,
    description: &str,
    diagnostic_code: &'static str,
) -> Result<Metadata, Vec<Diagnostic>> {
    std::fs::symlink_metadata(path).map_err(|error| {
        vec![Diagnostic::io(
            diagnostic_code,
            format!("cannot inspect {description} {}: {error}", path.display()),
        )]
    })
}

fn validate_regular_metadata(
    metadata: &Metadata,
    path: &Path,
    description: &str,
    diagnostic_code: &'static str,
) -> Result<(), Vec<Diagnostic>> {
    if !regular_metadata(metadata) {
        return Err(vec![Diagnostic::io(
            diagnostic_code,
            format!(
                "{description} {} must be a regular non-symlink file",
                path.display()
            ),
        )]);
    }
    Ok(())
}

fn regular_metadata(metadata: &Metadata) -> bool {
    if !metadata.file_type().is_file() || metadata.file_type().is_symlink() {
        return false;
    }
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt;
        const FILE_ATTRIBUTE_REPARSE_POINT: u32 = 0x400;
        if metadata.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT != 0 {
            return false;
        }
    }
    true
}

fn file_identity(
    metadata: &Metadata,
    path: &Path,
    description: &str,
    diagnostic_code: &'static str,
) -> Result<FileIdentity, Vec<Diagnostic>> {
    platform_path_identity(path, metadata).map_err(|message| {
        vec![Diagnostic::io(
            diagnostic_code,
            format!(
                "cannot authenticate {description} {} identity: {message}",
                path.display()
            ),
        )]
    })
}

fn file_handle_identity(
    file: &File,
    metadata: &Metadata,
    path: &Path,
    description: &str,
    diagnostic_code: &'static str,
) -> Result<FileIdentity, Vec<Diagnostic>> {
    platform_handle_identity(file, metadata).map_err(|message| {
        vec![Diagnostic::io(
            diagnostic_code,
            format!(
                "cannot authenticate {description} {} handle identity: {message}",
                path.display()
            ),
        )]
    })
}

#[cfg(unix)]
fn platform_path_identity(_path: &Path, metadata: &Metadata) -> Result<FileIdentity, String> {
    use std::os::unix::fs::MetadataExt;
    Ok(FileIdentity {
        device: metadata.dev(),
        inode: metadata.ino(),
    })
}

#[cfg(unix)]
fn platform_handle_identity(_file: &File, metadata: &Metadata) -> Result<FileIdentity, String> {
    platform_path_identity(Path::new("."), metadata)
}

#[cfg(windows)]
fn platform_path_identity(path: &Path, metadata: &Metadata) -> Result<FileIdentity, String> {
    if !regular_metadata(metadata) {
        return Err("path is not a regular non-reparse file".to_owned());
    }
    let first_file = File::open(path).map_err(|error| error.to_string())?;
    let first = platform_handle_identity(&first_file, metadata)?;
    let after_first = std::fs::symlink_metadata(path).map_err(|error| error.to_string())?;
    if !regular_metadata(&after_first) {
        return Err("path changed to a non-regular or reparse file".to_owned());
    }
    let second_file = File::open(path).map_err(|error| error.to_string())?;
    let second = platform_handle_identity(&second_file, &after_first)?;
    let after_second = std::fs::symlink_metadata(path).map_err(|error| error.to_string())?;
    if !regular_metadata(&after_second) {
        return Err("path changed to a non-regular or reparse file".to_owned());
    }
    if first != second {
        return Err("path identity changed while it was opened".to_owned());
    }
    Ok(first)
}

#[cfg(windows)]
fn platform_handle_identity(file: &File, _metadata: &Metadata) -> Result<FileIdentity, String> {
    let clone = file.try_clone().map_err(|error| error.to_string())?;
    let handle = same_file::Handle::from_file(clone).map_err(|error| error.to_string())?;
    Ok(FileIdentity { handle })
}

#[cfg(not(any(unix, windows)))]
fn platform_path_identity(_path: &Path, _metadata: &Metadata) -> Result<FileIdentity, String> {
    Err("exact file identity is unsupported on this platform".to_owned())
}

#[cfg(not(any(unix, windows)))]
fn platform_handle_identity(_file: &File, _metadata: &Metadata) -> Result<FileIdentity, String> {
    Err("exact file identity is unsupported on this platform".to_owned())
}

fn source_changed_error() -> Vec<Diagnostic> {
    vec![Diagnostic::io(
        "SPX-I207",
        "semantic patch source changed after its initial revision check",
    )
    .with_help("regenerate the patch against the current semantic graph")]
}

fn parse_patch(source: &str) -> Result<SemanticPatch, Vec<Diagnostic>> {
    let mut base = None;
    let mut renames = Vec::new();
    let mut no_new_effects = false;
    for (line_index, line) in source.lines().enumerate() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let words: Vec<_> = line.split_whitespace().collect();
        match words.as_slice() {
            ["base", revision] => base = Some((*revision).to_owned()),
            ["rename", stable_id, "to", new_name] => renames.push(Rename {
                stable_id: (*stable_id).to_owned(),
                new_name: (*new_name).to_owned(),
            }),
            ["require", "no-new-effects"] => no_new_effects = true,
            _ => {
                return Err(vec![Diagnostic::io(
                    "SPX-G101",
                    format!(
                        "invalid semantic patch instruction on line {}: {line}",
                        line_index + 1
                    ),
                )]);
            }
        }
    }
    let Some(base) = base else {
        return Err(vec![Diagnostic::io(
            "SPX-G102",
            "semantic patch is missing a `base <revision>` instruction",
        )]);
    };
    Ok(SemanticPatch {
        base,
        renames,
        no_new_effects,
    })
}

fn effect_set(program: &crate::ast::Program) -> BTreeSet<&str> {
    program
        .functions
        .iter()
        .flat_map(|function| function.effects.iter().map(String::as_str))
        .collect()
}

fn is_identifier(value: &str) -> bool {
    let mut chars = value.chars();
    matches!(chars.next(), Some(first) if first == '_' || first.is_ascii_alphabetic())
        && chars.all(|character| character == '_' || character.is_ascii_alphanumeric())
}

fn function_name_positions(
    program: &crate::ast::Program,
    tokens: &[lexer::Token],
    target: &crate::ast::Function,
) -> BTreeSet<(usize, usize)> {
    let mut positions = BTreeSet::from([(target.name_span.start, target.name_span.end)]);
    for function in &program.functions {
        let mut collect = |callee: &str, span: crate::ast::Span| {
            if callee != target.name {
                return;
            }
            if let Some(token) = tokens.iter().find(|token| {
                token.span.start == span.start
                    && matches!(&token.kind, lexer::TokenKind::Ident(name) if name == callee)
            }) {
                positions.insert((token.span.start, token.span.end));
            }
        };
        for contract in &function.requires {
            contract.visit_calls(&mut collect);
        }
        for contract in &function.ensures {
            contract.visit_calls(&mut collect);
        }
        function.body.visit_calls(&mut collect);
    }
    positions
}

fn resource_type_positions(
    program: &crate::ast::Program,
    tokens: &[lexer::Token],
    resource: &TypeDeclaration,
) -> BTreeSet<(usize, usize)> {
    let mut positions = BTreeSet::from([(resource.name_span.start, resource.name_span.end)]);
    let resource_type = Type::Named {
        name: resource.name.clone(),
        arguments: Vec::new(),
    };

    for declaration in &program.types {
        let TypeDeclarationKind::Record { fields } = &declaration.kind else {
            continue;
        };
        for field in fields {
            if field.ty == resource_type {
                insert_named_type_token(
                    &mut positions,
                    tokens,
                    field.name_span.end,
                    field.span.end,
                    &resource.name,
                );
            }
        }
    }

    for interface in &program.interfaces {
        for import in &interface.imports {
            for param in &import.params {
                if param.ty != resource_type {
                    continue;
                }
                let end = tokens
                    .iter()
                    .find(|token| {
                        token.span.start >= param.span.end
                            && token.span.end <= import.span.end
                            && matches!(
                                token.kind,
                                lexer::TokenKind::Comma | lexer::TokenKind::RParen
                            )
                    })
                    .map_or(import.span.end, |token| token.span.start);
                insert_named_type_token(
                    &mut positions,
                    tokens,
                    param.span.end,
                    end,
                    &resource.name,
                );
            }
        }
    }

    for function in &program.functions {
        for param in &function.params {
            if param.ty != resource_type {
                continue;
            }
            let end = tokens
                .iter()
                .find(|token| {
                    token.span.start >= param.span.end
                        && matches!(
                            token.kind,
                            lexer::TokenKind::Comma | lexer::TokenKind::RParen
                        )
                })
                .map_or(function.body.span.start, |token| token.span.start);
            insert_named_type_token(&mut positions, tokens, param.span.end, end, &resource.name);
        }

        if function.return_type == resource_type {
            if let Some(arrow) = tokens.iter().find(|token| {
                token.span.start >= function.name_span.end
                    && token.span.end <= function.body.span.start
                    && matches!(token.kind, lexer::TokenKind::Arrow)
            }) {
                insert_named_type_token(
                    &mut positions,
                    tokens,
                    arrow.span.end,
                    function.body.span.start,
                    &resource.name,
                );
            }
        }
    }

    positions
}

fn insert_named_type_token(
    positions: &mut BTreeSet<(usize, usize)>,
    tokens: &[lexer::Token],
    start: usize,
    end: usize,
    name: &str,
) {
    if let Some(token) = tokens.iter().find(|token| {
        token.span.start >= start
            && token.span.end <= end
            && matches!(&token.kind, lexer::TokenKind::Ident(candidate) if candidate == name)
    }) {
        positions.insert((token.span.start, token.span.end));
    }
}

#[cfg(test)]
mod commit_tests {
    use std::sync::atomic::{AtomicUsize, Ordering};

    use super::*;

    static NEXT_FIXTURE: AtomicUsize = AtomicUsize::new(0);

    const SOURCE: &str = r#"module patch.commit;

@id("helper.answer")
fn answer() -> i64
{
    42
}

@id("app.main")
fn main() -> i64
{
    answer()
}
"#;

    const CONCURRENT_SOURCE: &str = r#"module patch.commit;

@id("helper.answer")
fn answer() -> i64
{
    41
}

@id("app.main")
fn main() -> i64
{
    answer()
}
"#;

    fn fixture(label: &str) -> (PathBuf, PathBuf) {
        let sequence = NEXT_FIXTURE.fetch_add(1, Ordering::Relaxed);
        let directory = std::env::temp_dir().join(format!(
            "semaprax-patch-commit-{}-{label}-{sequence}",
            std::process::id()
        ));
        std::fs::create_dir_all(&directory).unwrap();
        let source_path = directory.join("module.spx");
        let patch_path = directory.join("rename.spatch");
        let revision = graph::revision(&parse(SOURCE, &source_path).unwrap());
        std::fs::write(&source_path, SOURCE).unwrap();
        std::fs::write(
            &patch_path,
            format!("base {revision}\nrename helper.answer to computed\n"),
        )
        .unwrap();
        (source_path, patch_path)
    }

    fn assert_owned_artifacts_removed(source_path: &Path) {
        let canonical = std::fs::canonicalize(source_path).unwrap();
        assert!(!sibling_path(&canonical, ".semaprax-patch.lock")
            .unwrap()
            .exists());
        for index in 0..STAGING_ATTEMPTS {
            assert!(
                !sibling_path(&canonical, &format!(".semaprax-stage.{index}.tmp"))
                    .unwrap()
                    .exists()
            );
        }
    }

    #[test]
    fn concurrent_edit_is_preserved_and_rejected_before_commit() {
        let (source_path, patch_path) = fixture("concurrent-edit");
        let error = apply_with_commit_hook(&source_path, &patch_path, |phase, source, _| {
            if phase == CommitPhase::BeforeFinalCheck {
                std::fs::write(source, CONCURRENT_SOURCE)?;
            }
            Ok(())
        })
        .unwrap_err();

        assert_eq!(error[0].code, "SPX-I207");
        assert_eq!(
            std::fs::read_to_string(&source_path).unwrap(),
            CONCURRENT_SOURCE
        );
        assert_owned_artifacts_removed(&source_path);
    }

    #[test]
    fn same_bytes_with_replaced_source_identity_are_rejected_after_final_check() {
        let (source_path, patch_path) = fixture("concurrent-replacement");
        let backup_path = source_path.with_extension("original.spx");
        let error = apply_with_commit_hook(&source_path, &patch_path, |phase, source, _| {
            if phase == CommitPhase::BeforeRename {
                std::fs::rename(source, &backup_path)?;
                std::fs::write(source, SOURCE)?;
            }
            Ok(())
        })
        .unwrap_err();

        assert_eq!(error[0].code, "SPX-I207");
        assert_eq!(std::fs::read_to_string(&source_path).unwrap(), SOURCE);
        assert_eq!(std::fs::read_to_string(&backup_path).unwrap(), SOURCE);
        assert_owned_artifacts_removed(&source_path);
    }

    #[test]
    fn staging_bytes_changed_after_final_check_are_rejected_and_cleaned() {
        let (source_path, patch_path) = fixture("stage-mutation");
        let error = apply_with_commit_hook(&source_path, &patch_path, |phase, _, staging| {
            if phase == CommitPhase::BeforeRename {
                std::fs::write(staging, b"attacker bytes")?;
            }
            Ok(())
        })
        .unwrap_err();

        assert_eq!(error[0].code, "SPX-I203");
        assert_eq!(std::fs::read_to_string(&source_path).unwrap(), SOURCE);
        assert_owned_artifacts_removed(&source_path);
    }

    #[test]
    fn foreign_stage_path_replacement_is_rejected_and_never_deleted() {
        let (source_path, patch_path) = fixture("stage-path-replacement");
        let displaced_owned_stage = source_path.with_extension("owned-stage");
        let error = apply_with_commit_hook(&source_path, &patch_path, |phase, _, staging| {
            if phase == CommitPhase::BeforeRename {
                std::fs::rename(staging, &displaced_owned_stage)?;
                std::fs::write(staging, b"foreign path object")?;
            }
            Ok(())
        })
        .unwrap_err();

        assert_eq!(error[0].code, "SPX-I203");
        assert_eq!(std::fs::read_to_string(&source_path).unwrap(), SOURCE);
        assert_eq!(
            std::fs::read_to_string(
                sibling_path(
                    &std::fs::canonicalize(&source_path).unwrap(),
                    ".semaprax-stage.0.tmp"
                )
                .unwrap()
            )
            .unwrap(),
            "foreign path object"
        );
        assert!(displaced_owned_stage.exists());
        assert!(!sibling_path(
            &std::fs::canonicalize(&source_path).unwrap(),
            ".semaprax-patch.lock"
        )
        .unwrap()
        .exists());
    }

    #[test]
    fn injected_rename_failure_preserves_source_and_cleans_owned_artifacts() {
        let (source_path, patch_path) = fixture("rename-failure");
        let error = apply_with_commit_hook(&source_path, &patch_path, |phase, _, _| {
            if phase == CommitPhase::BeforeRename {
                return Err(std::io::Error::new(
                    std::io::ErrorKind::PermissionDenied,
                    "injected rename rejection",
                ));
            }
            Ok(())
        })
        .unwrap_err();

        assert_eq!(error[0].code, "SPX-I204");
        assert_eq!(std::fs::read_to_string(&source_path).unwrap(), SOURCE);
        assert_owned_artifacts_removed(&source_path);
    }

    #[cfg(windows)]
    #[test]
    fn windows_held_identity_matches_hardlinks_and_rejects_distinct_files() {
        let (source_path, _) = fixture("windows-held-identity");
        let source_file = File::open(&source_path).unwrap();
        let source_metadata = source_file.metadata().unwrap();
        let source_identity = platform_handle_identity(&source_file, &source_metadata).unwrap();

        let hardlink_path = source_path.with_extension("hardlink.spx");
        std::fs::hard_link(&source_path, &hardlink_path).unwrap();
        let hardlink_metadata = std::fs::symlink_metadata(&hardlink_path).unwrap();
        let hardlink_identity = platform_path_identity(&hardlink_path, &hardlink_metadata).unwrap();
        assert_eq!(source_identity, hardlink_identity);

        let distinct_path = source_path.with_extension("distinct.spx");
        std::fs::write(&distinct_path, SOURCE).unwrap();
        let distinct_metadata = std::fs::symlink_metadata(&distinct_path).unwrap();
        let distinct_identity = platform_path_identity(&distinct_path, &distinct_metadata).unwrap();
        assert_ne!(source_identity, distinct_identity);
    }
}
