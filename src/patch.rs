mod direct_apply;
mod source_index;

use std::collections::{BTreeMap, BTreeSet};
use std::ffi::OsString;
use std::fs::{File, Metadata, OpenOptions, Permissions};
use std::io::{Read, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};

use crate::ast::{Program, Type, TypeDeclaration, TypeDeclarationKind};
use crate::diagnostic::Diagnostic;
use crate::{graph, hir, lexer, parse, verify};

use self::direct_apply::apply_with_commit_hook;
use self::source_index::SemanticSourceIndex;

#[derive(Debug)]
struct Rename {
    stable_id: String,
    new_name: String,
    operation_index: usize,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum PatchSchema {
    V1,
    V2,
    V3,
}

#[allow(
    dead_code,
    reason = "consumed by the held Semantic Workspace Transaction v1 module"
)]
#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
enum PatchSelector {
    AssignFunctionId(String),
    Rename(String),
    RenameMember(String, String),
    RenameCase(String, String),
    ReplaceCallTypeArgument(String, u32),
    RequireNoNewEffects,
}

impl PatchSelector {
    fn label(&self) -> String {
        match self {
            Self::AssignFunctionId(target) => format!("assign:{target}"),
            Self::Rename(target) => format!("rename:{target}"),
            Self::RenameMember(owner, member) => format!("member:{owner}:{member}"),
            Self::RenameCase(owner, case) => format!("case:{owner}:{case}"),
            Self::ReplaceCallTypeArgument(expression, index) => {
                format!("call:{expression}:{index}")
            }
            Self::RequireNoNewEffects => "require:no-new-effects".to_owned(),
        }
    }
}

#[derive(Debug)]
struct AssignFunctionId {
    repair_id: String,
    target: String,
    name: String,
    to: String,
}

#[derive(Debug)]
struct RenameMember {
    owner: String,
    member: String,
    new_name: String,
    operation_index: usize,
}

#[derive(Debug)]
struct RenameCase {
    owner: String,
    case: String,
    new_name: String,
    operation_index: usize,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum ScalarType {
    I64,
    Bool,
}

impl ScalarType {
    fn parse(value: &str) -> Option<Self> {
        match value {
            "i64" => Some(Self::I64),
            "bool" => Some(Self::Bool),
            _ => None,
        }
    }

    pub(crate) fn text(self) -> &'static str {
        match self {
            Self::I64 => "i64",
            Self::Bool => "bool",
        }
    }

    fn resolved(self) -> hir::ResolvedType {
        match self {
            Self::I64 => hir::ResolvedType::I64,
            Self::Bool => hir::ResolvedType::Bool,
        }
    }
}

#[derive(Debug)]
struct ReplaceCallTypeArgument {
    expression: String,
    template: String,
    old_instance: String,
    index: u32,
    from: ScalarType,
    to: ScalarType,
    operation_index: usize,
}

#[derive(Debug)]
struct SemanticPatch {
    schema: PatchSchema,
    base: String,
    renames: Vec<Rename>,
    member_renames: Vec<RenameMember>,
    case_renames: Vec<RenameCase>,
    call_type_argument_replacements: Vec<ReplaceCallTypeArgument>,
    no_new_effects: bool,
    assign_function_id: Option<AssignFunctionId>,
    operations: Vec<PreflightOperation>,
}

#[derive(Clone, Debug)]
pub(crate) enum PreflightOperation {
    AssignFunctionId {
        index: usize,
        repair_id: String,
        target: String,
        name: String,
        to: String,
    },
    Rename {
        index: usize,
        target: String,
        to: String,
    },
    RenameMember {
        index: usize,
        owner: String,
        member: String,
        to: String,
    },
    RenameCase {
        index: usize,
        owner: String,
        case: String,
        to: String,
    },
    ReplaceCallTypeArgument {
        index: usize,
        expression: String,
        template: String,
        old_instance: String,
        argument_index: u32,
        from: ScalarType,
        to: ScalarType,
    },
    RequireNoNewEffects {
        index: usize,
    },
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub(crate) enum SourceConsumerKind {
    Function,
    FunctionTemplate,
    Resource,
    Record,
    Class,
    Field,
    Variant,
    VariantCase,
    CaseField,
    Interface,
    Import,
}

impl SourceConsumerKind {
    pub(crate) const fn text(self) -> &'static str {
        match self {
            Self::Function => "function",
            Self::FunctionTemplate => "function_template",
            Self::Resource => "resource",
            Self::Record => "record",
            Self::Class => "class",
            Self::Field => "field",
            Self::Variant => "variant",
            Self::VariantCase => "variant_case",
            Self::CaseField => "case_field",
            Self::Interface => "interface",
            Self::Import => "import",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub(crate) enum SourceConsumerRole {
    Declaration,
    Reference,
}

impl SourceConsumerRole {
    pub(crate) const fn text(self) -> &'static str {
        match self {
            Self::Declaration => "declaration",
            Self::Reference => "reference",
        }
    }
}

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub(crate) struct SourceConsumerKey {
    pub(crate) kind: SourceConsumerKind,
    pub(crate) id: String,
}

#[derive(Clone, Debug)]
pub(crate) struct PlannedEdit {
    pub(crate) start: usize,
    pub(crate) end: usize,
    pub(crate) replacement: String,
    pub(crate) operation_indices: BTreeSet<usize>,
    pub(crate) consumer: Option<SourceConsumerKey>,
    pub(crate) role: Option<SourceConsumerRole>,
    pub(crate) change: usize,
}

#[derive(Clone, Debug)]
pub(crate) enum PreflightChange {
    Rename {
        target: String,
        target_kind: SourceConsumerKind,
        before: String,
        after: String,
        operation_indices: BTreeSet<usize>,
    },
    CallInstance {
        expression: String,
        template: String,
        before_arguments: Vec<hir::ResolvedType>,
        after_arguments: Vec<hir::ResolvedType>,
        before_instance: String,
        after_instance: String,
        operation_indices: BTreeSet<usize>,
    },
}

/// Pure semantic result of applying one owned patch buffer to one owned source
/// buffer. This type deliberately carries no filesystem handle or commit path;
/// `apply` must independently retain the complete A0 commit protocol.
pub(crate) struct PatchPreflight {
    source: String,
    patch_source: String,
    patch: SemanticPatch,
    before: Program,
    candidate: Program,
    base_revision: String,
    candidate_revision: String,
    canonical_candidate: String,
    operations: Vec<PreflightOperation>,
    changes: Vec<PreflightChange>,
    planned_edits: Vec<PlannedEdit>,
    identity_rebase: Option<crate::repair::IdentityRebaseEvidence>,
}

impl PatchPreflight {
    pub(crate) fn source(&self) -> &str {
        &self.source
    }

    pub(crate) fn patch_source(&self) -> &str {
        &self.patch_source
    }

    pub(crate) fn before(&self) -> &Program {
        &self.before
    }

    pub(crate) fn candidate(&self) -> &Program {
        &self.candidate
    }

    pub(crate) fn base_revision(&self) -> &str {
        &self.base_revision
    }

    pub(crate) fn candidate_revision(&self) -> &str {
        &self.candidate_revision
    }

    pub(crate) fn canonical_candidate(&self) -> &str {
        &self.canonical_candidate
    }

    pub(crate) fn schema_label(&self) -> &'static str {
        match self.patch.schema {
            PatchSchema::V1 => "semaprax.semantic-patch.v1",
            PatchSchema::V2 => "semaprax.semantic-patch.v2",
            PatchSchema::V3 => "semaprax.semantic-patch.v3",
        }
    }

    pub(crate) fn operations(&self) -> &[PreflightOperation] {
        &self.operations
    }

    pub(crate) fn changes(&self) -> &[PreflightChange] {
        &self.changes
    }

    pub(crate) fn planned_edits(&self) -> &[PlannedEdit] {
        &self.planned_edits
    }

    pub(crate) fn identity_rebase(&self) -> Option<&crate::repair::IdentityRebaseEvidence> {
        self.identity_rebase.as_ref()
    }
}

fn planned_edit(
    start: usize,
    end: usize,
    replacement: String,
    operation_indices: BTreeSet<usize>,
    change: usize,
) -> PlannedEdit {
    PlannedEdit {
        start,
        end,
        replacement,
        operation_indices,
        consumer: None,
        role: None,
        change,
    }
}

#[derive(Eq, Ord, PartialEq, PartialOrd)]
enum ChangeKey {
    Rename(String, SourceConsumerKind, String, String),
    CallInstance(String),
}

fn change_key(change: &PreflightChange) -> ChangeKey {
    match change {
        PreflightChange::Rename {
            target,
            target_kind,
            before,
            after,
            ..
        } => ChangeKey::Rename(target.clone(), *target_kind, before.clone(), after.clone()),
        PreflightChange::CallInstance { expression, .. } => {
            ChangeKey::CallInstance(expression.clone())
        }
    }
}

fn change_operation_indices(change: &PreflightChange) -> &BTreeSet<usize> {
    match change {
        PreflightChange::Rename {
            operation_indices, ..
        }
        | PreflightChange::CallInstance {
            operation_indices, ..
        } => operation_indices,
    }
}

fn change_operation_indices_mut(change: &mut PreflightChange) -> &mut BTreeSet<usize> {
    match change {
        PreflightChange::Rename {
            operation_indices, ..
        }
        | PreflightChange::CallInstance {
            operation_indices, ..
        } => operation_indices,
    }
}

fn normalize_change_order(changes: &mut Vec<PreflightChange>, edits: &mut [PlannedEdit]) {
    let original = std::mem::take(changes);
    let mut merged = Vec::<PreflightChange>::new();
    let mut merged_by_key = BTreeMap::<ChangeKey, usize>::new();
    let mut old_to_merged = Vec::with_capacity(original.len());
    for change in original {
        let key = change_key(&change);
        if let Some(index) = merged_by_key.get(&key).copied() {
            change_operation_indices_mut(&mut merged[index])
                .extend(change_operation_indices(&change).iter().copied());
            old_to_merged.push(index);
        } else {
            let index = merged.len();
            merged_by_key.insert(key, index);
            old_to_merged.push(index);
            merged.push(change);
        }
    }
    for edit in edits.iter_mut() {
        edit.change = old_to_merged[edit.change];
    }
    let mut order = (0..merged.len()).collect::<Vec<_>>();
    order.sort_by_key(|index| {
        change_operation_indices(&merged[*index])
            .iter()
            .next()
            .copied()
            .unwrap_or(usize::MAX)
    });
    let mut merged_to_sorted = vec![0usize; merged.len()];
    for (sorted, original) in order.iter().copied().enumerate() {
        merged_to_sorted[original] = sorted;
    }
    *changes = order
        .into_iter()
        .map(|index| merged[index].clone())
        .collect();
    for edit in edits {
        edit.change = merged_to_sorted[edit.change];
    }
}

#[derive(Clone)]
struct SourceContainer {
    start: usize,
    end: usize,
    name_start: usize,
    name_end: usize,
    key: SourceConsumerKey,
}

fn assign_source_consumers(program: &Program, edits: &mut [PlannedEdit]) {
    let mut containers = Vec::<SourceContainer>::new();
    let mut add = |span: crate::ast::Span,
                   name_span: crate::ast::Span,
                   id: &str,
                   kind: SourceConsumerKind| {
        containers.push(SourceContainer {
            start: span.start,
            end: span.end,
            name_start: name_span.start,
            name_end: name_span.end,
            key: SourceConsumerKey {
                id: id.to_owned(),
                kind,
            },
        });
    };
    for declaration in &program.types {
        let kind = match &declaration.kind {
            TypeDeclarationKind::Resource { .. } => SourceConsumerKind::Resource,
            TypeDeclarationKind::Record { fields } => {
                for field in fields {
                    add(
                        field.span,
                        field.name_span,
                        &field.stable_id,
                        SourceConsumerKind::Field,
                    );
                }
                SourceConsumerKind::Record
            }
            TypeDeclarationKind::Class { fields, methods } => {
                for field in fields {
                    add(
                        field.span,
                        field.name_span,
                        &field.stable_id,
                        SourceConsumerKind::Field,
                    );
                }
                for method in methods {
                    add(
                        method.span,
                        method.name_span,
                        &method.stable_id,
                        SourceConsumerKind::Function,
                    );
                }
                SourceConsumerKind::Class
            }
            TypeDeclarationKind::Variant { cases } => {
                for case in cases {
                    for field in &case.fields {
                        add(
                            field.span,
                            field.name_span,
                            &field.stable_id,
                            SourceConsumerKind::CaseField,
                        );
                    }
                    add(
                        case.span,
                        case.name_span,
                        &case.stable_id,
                        SourceConsumerKind::VariantCase,
                    );
                }
                SourceConsumerKind::Variant
            }
        };
        add(
            declaration.span,
            declaration.name_span,
            &declaration.stable_id,
            kind,
        );
    }
    for interface in &program.interfaces {
        for import in &interface.imports {
            add(
                import.span,
                import.name_span,
                &import.stable_id,
                SourceConsumerKind::Import,
            );
        }
        add(
            interface.span,
            interface.name_span,
            &interface.stable_id,
            SourceConsumerKind::Interface,
        );
    }
    for function in &program.functions {
        add(
            function.span,
            function.name_span,
            &function.stable_id,
            if function.type_parameters.is_empty() {
                SourceConsumerKind::Function
            } else {
                SourceConsumerKind::FunctionTemplate
            },
        );
    }
    containers.sort_by_key(|container| (container.start, usize::MAX - container.end));
    let mut next = 0usize;
    let mut active = Vec::<usize>::new();
    for edit in edits {
        while next < containers.len() && containers[next].start <= edit.start {
            let candidate = &containers[next];
            while active
                .last()
                .is_some_and(|index| containers[*index].end < candidate.end)
            {
                active.pop();
            }
            active.push(next);
            next += 1;
        }
        while active
            .last()
            .is_some_and(|index| containers[*index].end < edit.end)
        {
            active.pop();
        }
        if let Some(container) = active.last().map(|index| &containers[*index]) {
            if container.start <= edit.start && edit.end <= container.end {
                edit.consumer = Some(container.key.clone());
                edit.role = Some(
                    if container.name_start == edit.start && container.name_end == edit.end {
                        SourceConsumerRole::Declaration
                    } else {
                        SourceConsumerRole::Reference
                    },
                );
            }
        }
    }
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

pub(crate) struct SourceSnapshot {
    source: String,
    identity: FileIdentity,
    permissions: Permissions,
}

impl SourceSnapshot {
    pub(crate) fn source(&self) -> &str {
        &self.source
    }
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
pub(crate) enum CommitPhase {
    BeforeFinalCheck,
    BeforeRename,
}

pub(crate) struct A0CommitGuard {
    canonical_source_path: PathBuf,
    diagnostic_path: PathBuf,
    _lock: OwnedSiblingFile,
}

impl A0CommitGuard {
    pub(crate) fn canonical_source_path(&self) -> &Path {
        &self.canonical_source_path
    }
}

pub(crate) fn acquire_a0_commit_guard(
    source_path: &Path,
) -> Result<A0CommitGuard, Vec<Diagnostic>> {
    let canonical_source_path = canonical_source_path(source_path)?;
    let lock_path = sibling_path(&canonical_source_path, ".semaprax-patch.lock")?;
    let lock = OwnedSiblingFile::create(lock_path, "SPX-I205", "semantic patch lock", false)?
        .expect("existing lock is reported as an error");
    Ok(A0CommitGuard {
        canonical_source_path,
        diagnostic_path: source_path.to_path_buf(),
        _lock: lock,
    })
}

pub(crate) struct A0AuthenticatedSource<'a> {
    guard: &'a A0CommitGuard,
    snapshot: SourceSnapshot,
    max_source_bytes: Option<usize>,
}

impl A0AuthenticatedSource<'_> {
    pub(crate) fn source(&self) -> &str {
        self.snapshot.source()
    }
}

pub(crate) fn authenticate_a0_source<'a>(
    guard: &'a A0CommitGuard,
    limit: Option<(usize, &'static str)>,
) -> Result<A0AuthenticatedSource<'a>, Vec<Diagnostic>> {
    let snapshot = match limit {
        Some((max_source_bytes, diagnostic_code)) => read_source_snapshot_bounded(
            guard.canonical_source_path(),
            max_source_bytes,
            diagnostic_code,
        ),
        None => read_source_snapshot(guard.canonical_source_path()),
    }?;
    Ok(A0AuthenticatedSource {
        guard,
        snapshot,
        max_source_bytes: limit.map(|(max_source_bytes, _)| max_source_bytes),
    })
}

pub(crate) struct A0PreparedCommit<'a> {
    authenticated: &'a A0AuthenticatedSource<'a>,
    preflight: &'a PatchPreflight,
}

/// One complete, invocation-owned A0 handoff for a server-derived patch.
///
/// Unlike [`A0PreparedCommit`], this authority owns the lock, authenticated
/// source snapshot, and semantic preflight so it may outlive the Project
/// request snapshot that produced the patch bytes. It is deliberately neither
/// `Clone` nor reusable: committing consumes the authority.
pub(crate) struct A0OwnedPreparedCommit {
    guard: A0CommitGuard,
    snapshot: SourceSnapshot,
    max_source_bytes: Option<usize>,
    preflight: PatchPreflight,
}

#[cfg(test)]
impl A0OwnedPreparedCommit {
    pub(crate) fn base_revision(&self) -> &str {
        self.preflight.base_revision()
    }

    pub(crate) fn candidate_revision(&self) -> &str {
        self.preflight.candidate_revision()
    }

    pub(crate) fn canonical_candidate(&self) -> &str {
        self.preflight.canonical_candidate()
    }
}

pub(crate) fn prepare_a0_commit<'a>(
    authenticated: &'a A0AuthenticatedSource<'a>,
    preflight: &'a PatchPreflight,
) -> Result<A0PreparedCommit<'a>, Vec<Diagnostic>> {
    if preflight.source() != authenticated.snapshot.source() {
        return Err(vec![Diagnostic::io(
            "SPX-G133",
            "semantic patch preflight source is not bound to the authenticated A0 snapshot",
        )]);
    }
    Ok(A0PreparedCommit {
        authenticated,
        preflight,
    })
}

/// Acquire and retain A0 authority for one owned, server-derived patch buffer.
///
/// No proposal path is opened or created. The source lock and authenticated
/// source identity are acquired before this function returns, and the patch is
/// parsed and preflighted against that exact retained source snapshot.
#[cfg(test)]
pub(crate) fn prepare_owned_a0_patch_bytes(
    source_path: &Path,
    patch_bytes: Vec<u8>,
) -> Result<A0OwnedPreparedCommit, Vec<Diagnostic>> {
    let guard = acquire_a0_commit_guard(source_path)?;
    let patch_source = String::from_utf8(patch_bytes).map_err(|error| {
        vec![Diagnostic::io(
            "SPX-I202",
            format!("semantic patch bytes are not UTF-8: {error}"),
        )]
    })?;
    let parsed_patch = parse_patch(&patch_source)?;
    let bounded_v3 = parsed_patch.schema == PatchSchema::V3;
    let authenticated = if bounded_v3 {
        authenticate_a0_source(&guard, Some((crate::repair::MAX_SOURCE_BYTES, "SPX-R101")))?
    } else {
        authenticate_a0_source(&guard, None)?
    };
    let preflight = preflight_parsed_owned(
        authenticated.source().to_owned(),
        patch_source,
        source_path.to_path_buf(),
        parsed_patch,
        None,
        None,
        CandidateValidation::Standalone,
    )?;
    if preflight.source() != authenticated.snapshot.source() {
        return Err(vec![Diagnostic::io(
            "SPX-G133",
            "semantic patch preflight source is not bound to the authenticated A0 snapshot",
        )]);
    }
    let A0AuthenticatedSource {
        snapshot,
        max_source_bytes,
        ..
    } = authenticated;
    Ok(A0OwnedPreparedCommit {
        guard,
        snapshot,
        max_source_bytes,
        preflight,
    })
}

/// Acquire A0 for one opaque, completely validated Project rename plan.
///
/// A manifest-owned module is not a standalone executable and therefore must
/// not be rejected for lacking `main` or for having Project-resolved imports.
/// The private-field plan type is the seal: callers cannot select raw paths or
/// bytes for this deferred profile. This acquisition independently replays the
/// exact Patch-v1 operation against A0's retained source and matches all source
/// facts before returning effect authority.
pub(crate) fn acquire_prepared_project_rename(
    prepared: &crate::project::PreparedProjectRename,
) -> Result<A0OwnedPreparedCommit, Vec<Diagnostic>> {
    let guard = acquire_a0_commit_guard(prepared.target_path())?;
    let patch_source = prepared.patch_bytes().to_owned();
    let authenticated = authenticate_a0_source(&guard, None)?;
    let preflight = preflight_project_rename_parts(
        authenticated.source().to_owned(),
        patch_source,
        prepared.target_path().to_path_buf(),
    )?;
    if preflight.source() != authenticated.snapshot.source() {
        return Err(vec![Diagnostic::io(
            "SPX-G133",
            "Project rename preflight source is not bound to the authenticated A0 snapshot",
        )]);
    }
    if preflight.base_revision() != prepared.base_source().source_revision()
        || preflight.candidate_revision() != prepared.candidate_source().source_revision()
        || preflight.canonical_candidate() != prepared.candidate_source().source()
    {
        return Err(vec![Diagnostic::io(
            "SPX-J109",
            "retained Project rename plan disagrees with the A0 handoff",
        )]);
    }
    let A0AuthenticatedSource {
        snapshot,
        max_source_bytes,
        ..
    } = authenticated;
    Ok(A0OwnedPreparedCommit {
        guard,
        snapshot,
        max_source_bytes,
        preflight,
    })
}

/// Consume one owned A0 handoff through the unchanged staging and commit core.
pub(crate) fn commit_owned_a0(prepared: A0OwnedPreparedCommit) -> Result<String, Vec<Diagnostic>> {
    commit_owned_a0_with_hook(prepared, |_, _, _| Ok(()))
}

pub fn apply(source_path: &Path, patch_path: &Path) -> Result<String, Vec<Diagnostic>> {
    apply_with_commit_hook(source_path, patch_path, |_, _, _| Ok(()))
}

pub(crate) fn preflight_impact_owned(
    source: String,
    patch_source: String,
    diagnostic_path: PathBuf,
) -> Result<PatchPreflight, Vec<Diagnostic>> {
    let patch = parse_patch(&patch_source)?;
    if patch.schema == PatchSchema::V3 {
        return Err(vec![Diagnostic::io(
            "SPX-G110",
            "Semantic Impact v1 accepts only Semantic Patch v1/v2",
        )]);
    }
    preflight_parsed_owned(
        source,
        patch_source,
        diagnostic_path,
        patch,
        None,
        None,
        CandidateValidation::Standalone,
    )
}

pub(crate) fn preflight_project_rename_owned(
    derivation: &crate::project::ProjectRenameDerivation,
) -> Result<PatchPreflight, Vec<Diagnostic>> {
    preflight_project_rename_parts(
        derivation.source().to_owned(),
        derivation.patch_bytes().to_owned(),
        derivation.diagnostic_path().to_path_buf(),
    )
}

fn preflight_project_rename_parts(
    source: String,
    patch_source: String,
    diagnostic_path: PathBuf,
) -> Result<PatchPreflight, Vec<Diagnostic>> {
    let patch = parse_patch(&patch_source)?;
    if patch.schema != PatchSchema::V1
        || patch.operations.len() != 1
        || patch.renames.len() != 1
        || patch.no_new_effects
    {
        return Err(vec![Diagnostic::io(
            "SPX-J109",
            "Project rename handoff requires exactly one Patch-v1 function rename",
        )]);
    }
    preflight_parsed_owned(
        source,
        patch_source,
        diagnostic_path,
        patch,
        None,
        None,
        CandidateValidation::ProjectModule,
    )
}

pub(crate) fn preflight_review_owned(
    source: String,
    patch_source: String,
    diagnostic_path: PathBuf,
    max_operations: usize,
) -> Result<PatchPreflight, Vec<Diagnostic>> {
    let patch = parse_patch(&patch_source)?;
    if patch.operations.len() > max_operations {
        return Err(vec![Diagnostic::io(
            "SPX-G120",
            format!("semantic review patch exceeds {max_operations} operations"),
        )]);
    }
    preflight_parsed_owned(
        source,
        patch_source,
        diagnostic_path,
        patch,
        None,
        None,
        CandidateValidation::Standalone,
    )
}

pub(crate) fn preflight_target_owned(
    source: String,
    patch_source: String,
    diagnostic_path: PathBuf,
    max_operations: usize,
    max_candidate_bytes: usize,
) -> Result<PatchPreflight, Vec<Diagnostic>> {
    let patch = parse_patch(&patch_source)?;
    if patch.operations.len() > max_operations {
        return Err(vec![Diagnostic::io(
            "SPX-G140",
            format!("semantic target evidence patch exceeds {max_operations} operations"),
        )]);
    }
    preflight_parsed_owned(
        source,
        patch_source,
        diagnostic_path,
        patch,
        Some(max_candidate_bytes),
        None,
        CandidateValidation::Standalone,
    )
}

#[derive(Clone, Copy)]
struct WorkspaceAstLimits {
    declarations: usize,
    callables: usize,
    call_sites: usize,
}

const WORKSPACE_FORMATTER_WORK_BYTES: usize = 16 * 1024 * 1024;

#[allow(
    dead_code,
    reason = "consumed by the held Semantic Workspace Transaction v1 module"
)]
pub(crate) struct WorkspacePreflightLimits {
    max_operations: usize,
    max_candidate_bytes: usize,
    remaining_declarations: usize,
    remaining_callables: usize,
    remaining_call_sites: usize,
}

#[allow(
    dead_code,
    reason = "consumed by the held Semantic Workspace Transaction v1 module"
)]
impl WorkspacePreflightLimits {
    pub(crate) fn new(
        max_operations: usize,
        max_candidate_bytes: usize,
        remaining_declarations: usize,
        remaining_callables: usize,
        remaining_call_sites: usize,
    ) -> Self {
        Self {
            max_operations,
            max_candidate_bytes,
            remaining_declarations,
            remaining_callables,
            remaining_call_sites,
        }
    }
}

/// Parses one embedded workspace patch, requires the exact canonical workspace
/// spelling, and performs the existing pure semantic preflight. Ordinary
/// single-file Patch v1/v2 parsing deliberately remains trivia-tolerant.
#[allow(
    dead_code,
    reason = "consumed by the held Semantic Workspace Transaction v1 module"
)]
pub(crate) fn preflight_workspace_owned(
    source: String,
    patch_source: String,
    diagnostic_path: PathBuf,
    limits: WorkspacePreflightLimits,
) -> Result<PatchPreflight, Vec<Diagnostic>> {
    preflight_workspace_owned_with_formatter_limit(
        source,
        patch_source,
        diagnostic_path,
        limits,
        WORKSPACE_FORMATTER_WORK_BYTES,
    )
}

fn preflight_workspace_owned_with_formatter_limit(
    source: String,
    patch_source: String,
    diagnostic_path: PathBuf,
    limits: WorkspacePreflightLimits,
    formatter_work_bytes: usize,
) -> Result<PatchPreflight, Vec<Diagnostic>> {
    let patch = parse_patch(&patch_source)?;
    let canonical = canonical_patch(&patch);
    if patch_source != canonical {
        return Err(vec![Diagnostic::io(
            "SPX-G150",
            "embedded workspace patch is not canonical",
        )]);
    }
    let mut selectors = BTreeSet::new();
    for operation in &patch.operations {
        let selector = operation_selector(operation);
        if !selectors.insert(selector.clone()) {
            return Err(vec![Diagnostic::io(
                "SPX-G150",
                format!(
                    "embedded workspace patch duplicates selector `{}`",
                    selector.label()
                ),
            )]);
        }
    }
    if patch.operations.len() > limits.max_operations {
        return Err(vec![Diagnostic::io(
            "SPX-G151",
            format!(
                "workspace patch exceeds {} operations",
                limits.max_operations
            ),
        )]);
    }
    let max_candidate_bytes = limits.max_candidate_bytes;
    let (preflight, formatter_overflowed) =
        crate::bounded_output::with_limit(formatter_work_bytes, || {
            preflight_parsed_owned(
                source,
                patch_source,
                diagnostic_path,
                patch,
                Some(max_candidate_bytes),
                Some(WorkspaceAstLimits {
                    declarations: limits.remaining_declarations,
                    callables: limits.remaining_callables,
                    call_sites: limits.remaining_call_sites,
                }),
                CandidateValidation::Standalone,
            )
        });
    if formatter_overflowed {
        return Err(vec![Diagnostic::io(
            "SPX-G151",
            "workspace patch preflight exceeds its bounded formatter-work limit",
        )]);
    }
    let preflight = preflight.map_err(|mut diagnostics| {
        for diagnostic in &mut diagnostics {
            if diagnostic.code == "SPX-G140" {
                diagnostic.code = "SPX-G151";
                diagnostic.message =
                    "workspace candidate exceeds the total candidate-source byte limit".to_owned();
            }
        }
        diagnostics
    })?;
    if preflight.canonical_candidate().len() > max_candidate_bytes {
        return Err(vec![Diagnostic::io(
            "SPX-G151",
            "workspace candidate exceeds the total candidate-source byte limit",
        )]);
    }
    if preflight.base_revision() == preflight.candidate_revision() {
        return Err(vec![Diagnostic::io(
            "SPX-G153",
            "workspace changed-file patch produces no semantic revision change",
        )]);
    }
    Ok(preflight)
}

#[allow(
    dead_code,
    reason = "consumed by the held Semantic Workspace Transaction v1 module"
)]
fn operation_selector(operation: &PreflightOperation) -> PatchSelector {
    match operation {
        PreflightOperation::AssignFunctionId { target, .. } => {
            PatchSelector::AssignFunctionId(target.clone())
        }
        PreflightOperation::Rename { target, .. } => PatchSelector::Rename(target.clone()),
        PreflightOperation::RenameMember { owner, member, .. } => {
            PatchSelector::RenameMember(owner.clone(), member.clone())
        }
        PreflightOperation::RenameCase { owner, case, .. } => {
            PatchSelector::RenameCase(owner.clone(), case.clone())
        }
        PreflightOperation::ReplaceCallTypeArgument {
            expression,
            argument_index,
            ..
        } => PatchSelector::ReplaceCallTypeArgument(expression.clone(), *argument_index),
        PreflightOperation::RequireNoNewEffects { .. } => PatchSelector::RequireNoNewEffects,
    }
}

#[allow(
    dead_code,
    reason = "consumed by the held Semantic Workspace Transaction v1 module"
)]
fn canonical_patch(patch: &SemanticPatch) -> String {
    let mut output = String::new();
    if patch.schema == PatchSchema::V2 {
        output.push_str("schema semaprax.semantic-patch.v2\n");
    } else if patch.schema == PatchSchema::V3 {
        output.push_str("schema semaprax.semantic-patch.v3\n");
    }
    output.push_str("base ");
    output.push_str(&patch.base);
    output.push('\n');
    for operation in &patch.operations {
        match operation {
            PreflightOperation::AssignFunctionId {
                repair_id,
                target,
                name,
                to,
                ..
            } => {
                output.push_str("assign-function-id repair ");
                output.push_str(repair_id);
                output.push_str(" diagnostic SPX-S103");
                output.push_str(" target ");
                output.push_str(target);
                output.push_str(" name ");
                output.push_str(name);
                output.push_str(" to ");
                output.push_str(to);
                output.push('\n');
            }
            PreflightOperation::Rename { target, to, .. } => {
                output.push_str("rename ");
                output.push_str(target);
                output.push_str(" to ");
                output.push_str(to);
                output.push('\n');
            }
            PreflightOperation::RenameMember {
                owner, member, to, ..
            } => {
                output.push_str("rename-member owner ");
                output.push_str(owner);
                output.push_str(" member ");
                output.push_str(member);
                output.push_str(" to ");
                output.push_str(to);
                output.push('\n');
            }
            PreflightOperation::RenameCase {
                owner, case, to, ..
            } => {
                output.push_str("rename-case owner ");
                output.push_str(owner);
                output.push_str(" case ");
                output.push_str(case);
                output.push_str(" to ");
                output.push_str(to);
                output.push('\n');
            }
            PreflightOperation::ReplaceCallTypeArgument {
                expression,
                template,
                old_instance,
                argument_index,
                from,
                to,
                ..
            } => {
                output.push_str("replace-call-type-argument expression ");
                output.push_str(expression);
                output.push_str(" template ");
                output.push_str(template);
                output.push_str(" old-instance ");
                output.push_str(old_instance);
                output.push_str(" index ");
                output.push_str(&argument_index.to_string());
                output.push_str(" from ");
                output.push_str(from.text());
                output.push_str(" to ");
                output.push_str(to.text());
                output.push('\n');
            }
            PreflightOperation::RequireNoNewEffects { .. } => {
                output.push_str("require no-new-effects\n");
            }
        }
    }
    output
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum CandidateValidation {
    Standalone,
    ProjectModule,
}

fn preflight_parsed_owned(
    source: String,
    patch_source: String,
    diagnostic_path: PathBuf,
    patch: SemanticPatch,
    max_candidate_bytes: Option<usize>,
    workspace_ast_limits: Option<WorkspaceAstLimits>,
    candidate_validation: CandidateValidation,
) -> Result<PatchPreflight, Vec<Diagnostic>> {
    let before = parse(&source, &diagnostic_path).map_err(|error| vec![error])?;
    if let Some(limits) = workspace_ast_limits {
        let (declarations, callables, call_sites) = crate::review::workspace_ast_counts(&before)
            .map_err(|_| {
                vec![Diagnostic::io(
                    "SPX-G151",
                    "workspace source exceeds its parsed-AST work limits",
                )]
            })?;
        if declarations > limits.declarations
            || callables > limits.callables
            || call_sites > limits.call_sites
        {
            return Err(vec![Diagnostic::io(
                "SPX-G151",
                "workspace source exceeds its remaining parsed-AST work budget",
            )]);
        }
    }
    let base_revision = graph::revision(&before);
    if base_revision != patch.base {
        return Err(vec![Diagnostic::io(
            "SPX-G409",
            format!(
                "stale semantic patch: expected graph {}, current graph {base_revision}",
                patch.base
            ),
        )
        .with_help("regenerate the patch against the current semantic graph")]);
    }

    if patch.schema == PatchSchema::V3 {
        crate::repair::precheck_program(&before)?;
    }

    let before_resolved = if matches!(patch.schema, PatchSchema::V2 | PatchSchema::V3) {
        Some(hir::resolve(&before)?)
    } else {
        None
    };
    if patch.schema == PatchSchema::V3 {
        let assignment = patch
            .assign_function_id
            .as_ref()
            .expect("v3 grammar admits exactly one assignment");
        let candidate =
            crate::repair::preflight_patch_assignment(crate::repair::PatchAssignmentInput {
                source: &source,
                source_path: &diagnostic_path,
                before: &before,
                before_resolved: before_resolved
                    .as_ref()
                    .expect("v3 resolves before assignment"),
                base_revision: &base_revision,
                repair_id: &assignment.repair_id,
                target_id: &assignment.target,
                target_name: &assignment.name,
                persistent_id: &assignment.to,
            })?;
        let (candidate, canonical_candidate, candidate_revision, identity_rebase) =
            candidate.into_parts();
        if max_candidate_bytes.is_some_and(|limit| canonical_candidate.len() > limit) {
            return Err(vec![Diagnostic::io(
                "SPX-G140",
                "semantic target evidence candidate exceeds its bounded construction limit",
            )]);
        }
        let operations = patch.operations.clone();
        return Ok(PatchPreflight {
            source,
            patch_source,
            patch,
            before,
            candidate,
            base_revision,
            candidate_revision,
            canonical_candidate,
            operations,
            changes: Vec::new(),
            planned_edits: Vec::new(),
            identity_rebase: Some(identity_rebase),
        });
    }
    let before_effects = effect_set(&before);
    let mut replacements = Vec::new();
    let mut planned_edits = Vec::new();
    let mut changes = Vec::new();
    let tokens =
        lexer::lex(&source, &diagnostic_path.display().to_string()).map_err(|error| vec![error])?;
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
            let operation_indices = BTreeSet::from([rename.operation_index]);
            let change = changes.len();
            changes.push(PreflightChange::Rename {
                target: rename.stable_id.clone(),
                target_kind: if function.type_parameters.is_empty() {
                    SourceConsumerKind::Function
                } else {
                    SourceConsumerKind::FunctionTemplate
                },
                before: function.name.clone(),
                after: rename.new_name.clone(),
                operation_indices: operation_indices.clone(),
            });
            for (start, end) in function_name_positions(&before, &tokens, function) {
                let replacement = rename.new_name.clone();
                planned_edits.push(planned_edit(
                    start,
                    end,
                    replacement.clone(),
                    operation_indices.clone(),
                    change,
                ));
                replacements.push((start, end, replacement));
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
            let operation_indices = BTreeSet::from([rename.operation_index]);
            let change = changes.len();
            changes.push(PreflightChange::Rename {
                target: rename.stable_id.clone(),
                target_kind: SourceConsumerKind::Resource,
                before: resource.name.clone(),
                after: rename.new_name.clone(),
                operation_indices: operation_indices.clone(),
            });
            for (start, end) in resource_type_positions(&before, &tokens, resource) {
                let replacement = rename.new_name.clone();
                planned_edits.push(planned_edit(
                    start,
                    end,
                    replacement.clone(),
                    operation_indices.clone(),
                    change,
                ));
                replacements.push((start, end, replacement));
            }
            continue;
        }
        return Err(vec![Diagnostic::io(
            "SPX-G404",
            format!("stable id `{}` does not exist", rename.stable_id),
        )]);
    }
    let source_index = match &before_resolved {
        Some(resolved) => Some(
            SemanticSourceIndex::build(&before, resolved, &tokens).ok_or_else(|| {
                vec![Diagnostic::io(
                    "SPX-G108",
                    "semantic patch source/HIR identity index is inconsistent",
                )]
            })?,
        ),
        None => None,
    };
    for rename in &patch.member_renames {
        validate_new_name(&rename.new_name)?;
        let identity =
            member_identity(&before, &rename.owner, &rename.member).ok_or_else(|| {
                vec![Diagnostic::io(
                    "SPX-G107",
                    format!(
                        "member `{}` does not belong to stable owner `{}`",
                        rename.member, rename.owner
                    ),
                )]
            })?;
        if !identity.owner_explicit || !identity.member_explicit {
            return Err(vec![Diagnostic::io(
                "SPX-G107",
                format!(
                    "member `{}` and owner `{}` need explicit @id identities",
                    rename.member, rename.owner
                ),
            )]);
        }
        let operation_indices = BTreeSet::from([rename.operation_index]);
        let change = changes.len();
        changes.push(PreflightChange::Rename {
            target: rename.member.clone(),
            target_kind: identity.kind,
            before: identity.name.clone(),
            after: rename.new_name.clone(),
            operation_indices: operation_indices.clone(),
        });
        let sites = source_index
            .as_ref()
            .expect("v2 operations have a semantic source index")
            .members
            .get(&(rename.owner.clone(), rename.member.clone()))
            .ok_or_else(|| {
                vec![Diagnostic::io(
                    "SPX-G108",
                    format!(
                        "member identity `{}` under `{}` has no exact source sites",
                        rename.member, rename.owner
                    ),
                )]
            })?;
        for site in sites {
            let replacement = site.shorthand_binding.as_ref().map_or_else(
                || rename.new_name.clone(),
                |binding| format!("{}: {binding}", rename.new_name),
            );
            planned_edits.push(planned_edit(
                site.span.start,
                site.span.end,
                replacement.clone(),
                operation_indices.clone(),
                change,
            ));
            replacements.push((site.span.start, site.span.end, replacement));
        }
    }
    for rename in &patch.case_renames {
        validate_new_name(&rename.new_name)?;
        let identity = case_identity(&before, &rename.owner, &rename.case).ok_or_else(|| {
            vec![Diagnostic::io(
                "SPX-G107",
                format!(
                    "case `{}` does not belong to stable variant `{}`",
                    rename.case, rename.owner
                ),
            )]
        })?;
        if !identity.owner_explicit || !identity.case_explicit {
            return Err(vec![Diagnostic::io(
                "SPX-G107",
                format!(
                    "case `{}` and variant `{}` need explicit @id identities",
                    rename.case, rename.owner
                ),
            )]);
        }
        let operation_indices = BTreeSet::from([rename.operation_index]);
        let change = changes.len();
        changes.push(PreflightChange::Rename {
            target: rename.case.clone(),
            target_kind: SourceConsumerKind::VariantCase,
            before: identity.name.clone(),
            after: rename.new_name.clone(),
            operation_indices: operation_indices.clone(),
        });
        let sites = source_index
            .as_ref()
            .expect("v2 operations have a semantic source index")
            .cases
            .get(&(rename.owner.clone(), rename.case.clone()))
            .ok_or_else(|| {
                vec![Diagnostic::io(
                    "SPX-G108",
                    format!(
                        "case identity `{}` under `{}` has no exact source sites",
                        rename.case, rename.owner
                    ),
                )]
            })?;
        for span in sites {
            let replacement = rename.new_name.clone();
            planned_edits.push(planned_edit(
                span.start,
                span.end,
                replacement.clone(),
                operation_indices.clone(),
                change,
            ));
            replacements.push((span.start, span.end, replacement));
        }
    }

    let mut expected_call_arguments = BTreeMap::<String, Vec<hir::ResolvedType>>::new();
    let mut call_change_indices = BTreeMap::<String, usize>::new();
    for replacement in &patch.call_type_argument_replacements {
        if replacement.from == replacement.to {
            return Err(patch_conflict(format!(
                "call type argument {} is already `{}`",
                replacement.index,
                replacement.from.text()
            )));
        }
        let site = source_index
            .as_ref()
            .expect("v2 operations have a semantic source index")
            .calls
            .get(&replacement.expression)
            .ok_or_else(|| {
                call_selector_error(replacement, "expression does not identify a source call")
            })?;
        let index = usize::try_from(replacement.index).map_err(|_| {
            call_selector_error(replacement, "type argument index is not addressable")
        })?;
        if site.template != replacement.template
            || site.instance.as_deref() != Some(replacement.old_instance.as_str())
            || site.type_arguments.get(index) != Some(&replacement.from.resolved())
        {
            return Err(call_selector_error(
                replacement,
                "expression/template/old-instance/index/from tuple does not match resolved HIR",
            ));
        }
        let span = site.type_argument_spans.get(index).ok_or_else(|| {
            call_selector_error(replacement, "type argument has no exact source token")
        })?;
        if source.get(span.start..span.end) != Some(replacement.from.text()) {
            return Err(call_selector_error(
                replacement,
                "resolved type argument does not match its exact source token",
            ));
        }
        let arguments = expected_call_arguments
            .entry(replacement.expression.clone())
            .or_insert_with(|| site.type_arguments.clone());
        arguments[index] = replacement.to.resolved();
        let operation_indices = BTreeSet::from([replacement.operation_index]);
        let change = if let Some(change) = call_change_indices.get(&replacement.expression) {
            *change
        } else {
            let change = changes.len();
            changes.push(PreflightChange::CallInstance {
                expression: replacement.expression.clone(),
                template: replacement.template.clone(),
                before_arguments: site.type_arguments.clone(),
                after_arguments: site.type_arguments.clone(),
                before_instance: replacement.old_instance.clone(),
                after_instance: replacement.old_instance.clone(),
                operation_indices: BTreeSet::new(),
            });
            call_change_indices.insert(replacement.expression.clone(), change);
            change
        };
        let PreflightChange::CallInstance {
            after_arguments,
            operation_indices: contributing,
            ..
        } = &mut changes[change]
        else {
            unreachable!("call change index identifies a call-instance change")
        };
        *after_arguments = arguments.clone();
        contributing.extend(operation_indices.iter().copied());
        let edit_replacement = replacement.to.text().to_owned();
        planned_edits.push(planned_edit(
            span.start,
            span.end,
            edit_replacement.clone(),
            operation_indices,
            change,
        ));
        replacements.push((span.start, span.end, edit_replacement));
    }
    let expected_call_instances = expected_call_arguments
        .iter()
        .map(|(expression, arguments)| {
            let site = source_index
                .as_ref()
                .expect("v2 operations have a semantic source index")
                .calls
                .get(expression)
                .expect("validated call remains indexed");
            (
                expression.clone(),
                hir::FunctionInstanceId::derive(
                    &hir::DeclarationId::new(site.template.clone()),
                    arguments,
                ),
            )
        })
        .collect::<BTreeMap<_, _>>();
    for (expression, instance) in &expected_call_instances {
        let change = call_change_indices
            .get(expression)
            .expect("each expected call instance has a grouped change");
        let PreflightChange::CallInstance { after_instance, .. } = &mut changes[*change] else {
            unreachable!("call change index identifies a call-instance change")
        };
        *after_instance = instance.as_str().to_owned();
    }

    replacements.sort_by_key(|replacement| (replacement.0, replacement.1));
    let mut checked_replacements = Vec::with_capacity(replacements.len());
    for replacement in replacements {
        if let Some(previous) = checked_replacements.last() {
            let previous: &(usize, usize, String) = previous;
            if replacement.0 < previous.1 {
                if replacement == *previous {
                    continue;
                }
                return Err(patch_conflict(format!(
                    "semantic patch edits overlap at source byte {}",
                    replacement.0
                )));
            }
        }
        checked_replacements.push(replacement);
    }
    normalize_change_order(&mut changes, &mut planned_edits);
    planned_edits.sort_by(|left, right| {
        (left.start, left.end, &left.replacement, left.change).cmp(&(
            right.start,
            right.end,
            &right.replacement,
            right.change,
        ))
    });
    let mut checked_planned_edits: Vec<PlannedEdit> = Vec::with_capacity(planned_edits.len());
    for edit in planned_edits {
        if let Some(previous) = checked_planned_edits.last_mut() {
            if edit.start < previous.end
                && edit.start == previous.start
                && edit.end == previous.end
                && edit.replacement == previous.replacement
                && edit.consumer == previous.consumer
                && edit.role == previous.role
                && edit.change == previous.change
            {
                previous.operation_indices.extend(edit.operation_indices);
                continue;
            }
        }
        checked_planned_edits.push(edit);
    }
    assign_source_consumers(&before, &mut checked_planned_edits);
    let predicted_candidate_bytes = if let Some(limit) = max_candidate_bytes {
        let predicted = checked_replacements
            .iter()
            .try_fold(source.len(), |length, edit| {
                length
                    .checked_sub(edit.1 - edit.0)
                    .and_then(|value| value.checked_add(edit.2.len()))
            });
        if predicted.is_none_or(|length| length > limit) {
            return Err(vec![Diagnostic::io(
                "SPX-G140",
                "semantic target evidence candidate exceeds its bounded construction limit",
            )]);
        }
        predicted
    } else {
        None
    };
    let changed = if let Some(predicted) = predicted_candidate_bytes {
        let mut changed = String::with_capacity(predicted);
        let mut cursor = 0usize;
        for (start, end, replacement) in &checked_replacements {
            changed.push_str(&source[cursor..*start]);
            changed.push_str(replacement);
            cursor = *end;
        }
        changed.push_str(&source[cursor..]);
        debug_assert_eq!(changed.len(), predicted);
        changed
    } else {
        let mut changed = source.clone();
        for (start, end, replacement) in checked_replacements.into_iter().rev() {
            changed.replace_range(start..end, &replacement);
        }
        changed
    };

    let (candidate, canonical_candidate) =
        crate::parse_canonical(&changed, &diagnostic_path).map_err(|error| vec![error])?;
    if candidate_validation == CandidateValidation::Standalone {
        let diagnostics = verify::verify(&candidate);
        if diagnostics.iter().any(|item| item.severity.is_error()) {
            return Err(diagnostics);
        }
    }
    if patch.no_new_effects && !effect_set(&candidate).is_subset(&before_effects) {
        return Err(vec![Diagnostic::io(
            "SPX-G105",
            "semantic patch violates requirement `no-new-effects`",
        )]);
    }
    if let Some(before_resolved) = &before_resolved {
        let after_resolved = hir::resolve(&candidate)?;
        validate_semantic_delta(SemanticDeltaInputs {
            before_source: &source,
            after_source: &changed,
            before: &before,
            after: &candidate,
            before_resolved,
            after_resolved: &after_resolved,
            patch: &patch,
            expected_call_instances: &expected_call_instances,
            expected_call_arguments: &expected_call_arguments,
        })?;
    }
    let candidate_revision = graph::revision(&candidate);
    let operations = patch.operations.clone();
    Ok(PatchPreflight {
        source,
        patch_source,
        patch,
        before,
        candidate,
        base_revision,
        candidate_revision,
        canonical_candidate,
        operations,
        changes,
        planned_edits: checked_planned_edits,
        identity_rebase: None,
    })
}

pub(crate) fn commit_prepared_a0(
    prepared: A0PreparedCommit<'_>,
    hook: impl FnMut(CommitPhase, &Path, &Path) -> std::io::Result<()>,
) -> Result<String, Vec<Diagnostic>> {
    let authenticated = prepared.authenticated;
    commit_a0_parts(
        authenticated.guard,
        &authenticated.snapshot,
        authenticated.max_source_bytes,
        prepared.preflight,
        hook,
    )
}

fn commit_owned_a0_with_hook(
    prepared: A0OwnedPreparedCommit,
    hook: impl FnMut(CommitPhase, &Path, &Path) -> std::io::Result<()>,
) -> Result<String, Vec<Diagnostic>> {
    let A0OwnedPreparedCommit {
        guard,
        snapshot,
        max_source_bytes,
        preflight,
    } = prepared;
    commit_a0_parts(&guard, &snapshot, max_source_bytes, &preflight, hook)
}

fn commit_a0_parts(
    guard: &A0CommitGuard,
    snapshot: &SourceSnapshot,
    max_source_bytes: Option<usize>,
    preflight: &PatchPreflight,
    mut hook: impl FnMut(CommitPhase, &Path, &Path) -> std::io::Result<()>,
) -> Result<String, Vec<Diagnostic>> {
    let canonical_source_path = guard.canonical_source_path();
    let canonical_bytes = preflight.canonical_candidate().as_bytes();
    let mut staging = create_staging_file(canonical_source_path)?;
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
        file.set_permissions(snapshot.permissions.clone())
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
        canonical_source_path,
        &staging.path,
    )
    .map_err(|error| {
        vec![Diagnostic::io(
            "SPX-I207",
            format!("semantic patch pre-commit check failed: {error}"),
        )]
    })?;
    validate_commit_source_unchanged(
        canonical_source_path,
        &guard.diagnostic_path,
        snapshot,
        preflight.base_revision(),
        max_source_bytes,
    )?;
    staging.validate_contents(canonical_bytes)?;
    hook(
        CommitPhase::BeforeRename,
        canonical_source_path,
        &staging.path,
    )
    .map_err(|error| {
        vec![Diagnostic::io(
            "SPX-I204",
            format!("cannot atomically commit semantic patch: {error}"),
        )]
    })?;
    validate_commit_source_unchanged(
        canonical_source_path,
        &guard.diagnostic_path,
        snapshot,
        preflight.base_revision(),
        max_source_bytes,
    )?;
    staging.validate_contents(canonical_bytes)?;
    std::fs::rename(&staging.path, canonical_source_path).map_err(|error| {
        vec![Diagnostic::io(
            "SPX-I204",
            format!("cannot atomically commit semantic patch: {error}"),
        )]
    })?;
    staging.committed();
    Ok(preflight.candidate_revision().to_owned())
}

fn validate_commit_source_unchanged(
    canonical_source_path: &Path,
    diagnostic_path: &Path,
    before: &SourceSnapshot,
    revision: &str,
    max_source_bytes: Option<usize>,
) -> Result<(), Vec<Diagnostic>> {
    if let Some(max_source_bytes) = max_source_bytes {
        validate_source_unchanged_bounded(
            canonical_source_path,
            diagnostic_path,
            before,
            revision,
            max_source_bytes,
        )
    } else {
        validate_source_unchanged(canonical_source_path, diagnostic_path, before, revision)
    }
}

pub(crate) fn canonical_source_path(source_path: &Path) -> Result<PathBuf, Vec<Diagnostic>> {
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

pub(crate) fn validate_source_unchanged(
    canonical_source_path: &Path,
    diagnostic_path: &Path,
    before: &SourceSnapshot,
    revision: &str,
) -> Result<(), Vec<Diagnostic>> {
    validate_source_unchanged_inner(
        canonical_source_path,
        diagnostic_path,
        before,
        revision,
        None,
    )
}

pub(crate) fn validate_source_unchanged_bounded(
    canonical_source_path: &Path,
    diagnostic_path: &Path,
    before: &SourceSnapshot,
    revision: &str,
    max_bytes: usize,
) -> Result<(), Vec<Diagnostic>> {
    validate_source_unchanged_inner(
        canonical_source_path,
        diagnostic_path,
        before,
        revision,
        Some(max_bytes),
    )
}

fn validate_source_unchanged_inner(
    canonical_source_path: &Path,
    diagnostic_path: &Path,
    before: &SourceSnapshot,
    revision: &str,
    max_bytes: Option<usize>,
) -> Result<(), Vec<Diagnostic>> {
    let current = match max_bytes {
        Some(max_bytes) => {
            read_source_snapshot_bounded(canonical_source_path, max_bytes, "SPX-I207")
        }
        None => read_source_snapshot(canonical_source_path),
    }
    .map_err(|_| source_changed_error())?;
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

pub(crate) fn read_source_snapshot(path: &Path) -> Result<SourceSnapshot, Vec<Diagnostic>> {
    read_source_snapshot_inner(path, None)
}

pub(crate) fn read_source_snapshot_bounded(
    path: &Path,
    max_bytes: usize,
    diagnostic_code: &'static str,
) -> Result<SourceSnapshot, Vec<Diagnostic>> {
    read_source_snapshot_inner(path, Some((max_bytes, diagnostic_code)))
}

fn read_source_snapshot_inner(
    path: &Path,
    limit: Option<(usize, &'static str)>,
) -> Result<SourceSnapshot, Vec<Diagnostic>> {
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
    if let Some((max_bytes, diagnostic_code)) = limit {
        if handle_metadata.len() > max_bytes as u64 {
            return Err(vec![Diagnostic::io(
                diagnostic_code,
                format!("diagnostic repair source exceeds {max_bytes} bytes"),
            )]);
        }
    }
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
    let read_result = if let Some((max_bytes, _)) = limit {
        Read::by_ref(&mut file)
            .take(max_bytes.saturating_add(1) as u64)
            .read_to_string(&mut source)
    } else {
        file.read_to_string(&mut source)
    };
    read_result.map_err(|error| {
        vec![Diagnostic::io(
            "SPX-I201",
            format!("cannot read {}: {error}", path.display()),
        )]
    })?;
    if let Some((max_bytes, diagnostic_code)) = limit {
        if source.len() > max_bytes {
            return Err(vec![Diagnostic::io(
                diagnostic_code,
                format!("diagnostic repair source exceeds {max_bytes} bytes"),
            )]);
        }
    }
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
    if source.lines().next() == Some("schema semaprax.semantic-patch.v3") {
        return parse_patch_v3(source);
    }
    let meaningful: Vec<_> = source
        .lines()
        .enumerate()
        .filter_map(|(index, line)| {
            let line = line.trim();
            (!line.is_empty() && !line.starts_with('#')).then_some((index, line))
        })
        .collect();
    let schema = match meaningful.first().map(|(_, line)| *line) {
        Some("schema semaprax.semantic-patch.v2") => PatchSchema::V2,
        Some(line) if line.starts_with("schema ") => {
            return Err(vec![Diagnostic::io(
                "SPX-G101",
                format!("unknown semantic patch schema: {line}"),
            )]);
        }
        _ => PatchSchema::V1,
    };
    let mut base = None;
    let mut renames = Vec::new();
    let mut member_renames = Vec::new();
    let mut case_renames = Vec::new();
    let mut call_type_argument_replacements = Vec::new();
    let mut no_new_effects = false;
    let mut selectors = BTreeSet::new();
    let mut operations = Vec::new();
    for (meaningful_index, (line_index, line)) in meaningful.into_iter().enumerate() {
        if meaningful_index == 0 && schema == PatchSchema::V2 {
            continue;
        }
        let words: Vec<_> = line.split_whitespace().collect();
        match words.as_slice() {
            ["base", revision] => {
                if schema == PatchSchema::V2 && base.is_some() {
                    return Err(patch_conflict("duplicate `base` instruction"));
                }
                base = Some((*revision).to_owned());
            }
            ["rename", stable_id, "to", new_name] => {
                reject_duplicate_selector(
                    schema,
                    &mut selectors,
                    PatchSelector::Rename((*stable_id).to_owned()),
                )?;
                let operation_index = operations.len();
                operations.push(PreflightOperation::Rename {
                    index: operation_index,
                    target: (*stable_id).to_owned(),
                    to: (*new_name).to_owned(),
                });
                renames.push(Rename {
                    stable_id: (*stable_id).to_owned(),
                    new_name: (*new_name).to_owned(),
                    operation_index,
                });
            }
            ["rename-member", "owner", owner, "member", member, "to", new_name]
                if schema == PatchSchema::V2 =>
            {
                reject_duplicate_selector(
                    schema,
                    &mut selectors,
                    PatchSelector::RenameMember((*owner).to_owned(), (*member).to_owned()),
                )?;
                let operation_index = operations.len();
                operations.push(PreflightOperation::RenameMember {
                    index: operation_index,
                    owner: (*owner).to_owned(),
                    member: (*member).to_owned(),
                    to: (*new_name).to_owned(),
                });
                member_renames.push(RenameMember {
                    owner: (*owner).to_owned(),
                    member: (*member).to_owned(),
                    new_name: (*new_name).to_owned(),
                    operation_index,
                });
            }
            ["rename-case", "owner", owner, "case", case, "to", new_name]
                if schema == PatchSchema::V2 =>
            {
                reject_duplicate_selector(
                    schema,
                    &mut selectors,
                    PatchSelector::RenameCase((*owner).to_owned(), (*case).to_owned()),
                )?;
                let operation_index = operations.len();
                operations.push(PreflightOperation::RenameCase {
                    index: operation_index,
                    owner: (*owner).to_owned(),
                    case: (*case).to_owned(),
                    to: (*new_name).to_owned(),
                });
                case_renames.push(RenameCase {
                    owner: (*owner).to_owned(),
                    case: (*case).to_owned(),
                    new_name: (*new_name).to_owned(),
                    operation_index,
                });
            }
            ["replace-call-type-argument", "expression", expression, "template", template, "old-instance", old_instance, "index", index, "from", from, "to", to]
                if schema == PatchSchema::V2 =>
            {
                let parsed_index = index
                    .parse::<u32>()
                    .ok()
                    .filter(|value| value.to_string() == *index);
                let (Some(index), Some(from), Some(to)) =
                    (parsed_index, ScalarType::parse(from), ScalarType::parse(to))
                else {
                    return Err(vec![Diagnostic::io(
                        "SPX-G101",
                        format!(
                            "invalid semantic patch instruction on line {}: {line}",
                            line_index + 1
                        ),
                    )]);
                };
                reject_duplicate_selector(
                    schema,
                    &mut selectors,
                    PatchSelector::ReplaceCallTypeArgument((*expression).to_owned(), index),
                )?;
                let operation_index = operations.len();
                operations.push(PreflightOperation::ReplaceCallTypeArgument {
                    index: operation_index,
                    expression: (*expression).to_owned(),
                    template: (*template).to_owned(),
                    old_instance: (*old_instance).to_owned(),
                    argument_index: index,
                    from,
                    to,
                });
                call_type_argument_replacements.push(ReplaceCallTypeArgument {
                    expression: (*expression).to_owned(),
                    template: (*template).to_owned(),
                    old_instance: (*old_instance).to_owned(),
                    index,
                    from,
                    to,
                    operation_index,
                });
            }
            ["require", "no-new-effects"] => {
                reject_duplicate_selector(
                    schema,
                    &mut selectors,
                    PatchSelector::RequireNoNewEffects,
                )?;
                operations.push(PreflightOperation::RequireNoNewEffects {
                    index: operations.len(),
                });
                no_new_effects = true;
            }
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
        schema,
        base,
        renames,
        member_renames,
        case_renames,
        call_type_argument_replacements,
        no_new_effects,
        assign_function_id: None,
        operations,
    })
}

fn parse_patch_v3(source: &str) -> Result<SemanticPatch, Vec<Diagnostic>> {
    let Some(body) = source.strip_suffix('\n') else {
        return Err(vec![Diagnostic::io(
            "SPX-G101",
            "semantic patch v3 must be exactly three LF-terminated lines",
        )]);
    };
    if body.contains('\r') {
        return Err(vec![Diagnostic::io(
            "SPX-G101",
            "semantic patch v3 must use LF line endings",
        )]);
    }
    let lines = body.split('\n').collect::<Vec<_>>();
    if lines.len() != 3 || lines[0] != "schema semaprax.semantic-patch.v3" {
        return Err(vec![Diagnostic::io(
            "SPX-G101",
            "semantic patch v3 must contain exactly schema, base, and assign-function-id lines",
        )]);
    }
    let base_words = lines[1].split_whitespace().collect::<Vec<_>>();
    let ["base", base_revision] = base_words.as_slice() else {
        return Err(vec![Diagnostic::io(
            "SPX-G101",
            "semantic patch v3 has an invalid base line",
        )]);
    };
    if lines[1] != format!("base {base_revision}") {
        return Err(vec![Diagnostic::io(
            "SPX-G101",
            "semantic patch v3 base must use one canonical separator",
        )]);
    }
    let words = lines[2].split_whitespace().collect::<Vec<_>>();
    let ["assign-function-id", "repair", repair_id, "diagnostic", "SPX-S103", "target", target, "name", name, "to", to] =
        words.as_slice()
    else {
        return Err(vec![Diagnostic::io(
            "SPX-G101",
            "semantic patch v3 has an invalid assign-function-id line",
        )]);
    };
    if lines[2]
        != format!(
            "assign-function-id repair {repair_id} diagnostic SPX-S103 target {target} name {name} to {to}"
        )
    {
        return Err(vec![Diagnostic::io(
            "SPX-G101",
            "semantic patch v3 assignment must use canonical single-space separators",
        )]);
    }
    let operation = PreflightOperation::AssignFunctionId {
        index: 0,
        repair_id: (*repair_id).to_owned(),
        target: (*target).to_owned(),
        name: (*name).to_owned(),
        to: (*to).to_owned(),
    };
    Ok(SemanticPatch {
        schema: PatchSchema::V3,
        base: (*base_revision).to_owned(),
        renames: Vec::new(),
        member_renames: Vec::new(),
        case_renames: Vec::new(),
        call_type_argument_replacements: Vec::new(),
        no_new_effects: false,
        assign_function_id: Some(AssignFunctionId {
            repair_id: (*repair_id).to_owned(),
            target: (*target).to_owned(),
            name: (*name).to_owned(),
            to: (*to).to_owned(),
        }),
        operations: vec![operation],
    })
}

fn reject_duplicate_selector(
    schema: PatchSchema,
    selectors: &mut BTreeSet<PatchSelector>,
    selector: PatchSelector,
) -> Result<(), Vec<Diagnostic>> {
    if schema == PatchSchema::V2 && !selectors.insert(selector.clone()) {
        return Err(patch_conflict(format!(
            "duplicate semantic patch selector `{}`",
            selector.label()
        )));
    }
    Ok(())
}

fn patch_conflict(message: impl Into<String>) -> Vec<Diagnostic> {
    vec![Diagnostic::io("SPX-G106", message)]
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

fn validate_new_name(value: &str) -> Result<(), Vec<Diagnostic>> {
    if is_identifier(value) {
        Ok(())
    } else {
        Err(vec![Diagnostic::io(
            "SPX-G103",
            format!("`{value}` is not a valid symbol name"),
        )])
    }
}

struct MemberIdentity {
    owner_explicit: bool,
    member_explicit: bool,
    name: String,
    kind: SourceConsumerKind,
}

fn member_identity(
    program: &crate::ast::Program,
    owner: &str,
    member: &str,
) -> Option<MemberIdentity> {
    for declaration in &program.types {
        match &declaration.kind {
            TypeDeclarationKind::Record { fields } if declaration.stable_id == owner => {
                let field = fields.iter().find(|field| field.stable_id == member)?;
                return Some(MemberIdentity {
                    owner_explicit: declaration.explicit_id,
                    member_explicit: field.explicit_id,
                    name: field.name.clone(),
                    kind: SourceConsumerKind::Field,
                });
            }
            TypeDeclarationKind::Class { fields, .. } if declaration.stable_id == owner => {
                let field = fields.iter().find(|field| field.stable_id == member)?;
                return Some(MemberIdentity {
                    owner_explicit: declaration.explicit_id,
                    member_explicit: field.explicit_id,
                    name: field.name.clone(),
                    kind: SourceConsumerKind::Field,
                });
            }
            TypeDeclarationKind::Variant { cases } => {
                if let Some(case) = cases.iter().find(|case| case.stable_id == owner) {
                    let field = case.fields.iter().find(|field| field.stable_id == member)?;
                    return Some(MemberIdentity {
                        owner_explicit: case.explicit_id,
                        member_explicit: field.explicit_id,
                        name: field.name.clone(),
                        kind: SourceConsumerKind::CaseField,
                    });
                }
            }
            TypeDeclarationKind::Resource { .. }
            | TypeDeclarationKind::Record { .. }
            | TypeDeclarationKind::Class { .. } => {}
        }
    }
    None
}

struct CaseIdentity {
    owner_explicit: bool,
    case_explicit: bool,
    name: String,
}

fn case_identity(program: &crate::ast::Program, owner: &str, case: &str) -> Option<CaseIdentity> {
    program.types.iter().find_map(|declaration| {
        let TypeDeclarationKind::Variant { cases } = &declaration.kind else {
            return None;
        };
        if declaration.stable_id != owner {
            return None;
        }
        let case = cases.iter().find(|candidate| candidate.stable_id == case)?;
        Some(CaseIdentity {
            owner_explicit: declaration.explicit_id,
            case_explicit: case.explicit_id,
            name: case.name.clone(),
        })
    })
}

fn call_selector_error(replacement: &ReplaceCallTypeArgument, reason: &str) -> Vec<Diagnostic> {
    vec![Diagnostic::io(
        "SPX-G108",
        format!(
            "generic call selector for expression `{}` is stale or invalid: {reason}",
            replacement.expression
        ),
    )
    .with_help("regenerate the patch from the current semantic graph")]
}

struct SemanticDeltaInputs<'a> {
    before_source: &'a str,
    after_source: &'a str,
    before: &'a crate::ast::Program,
    after: &'a crate::ast::Program,
    before_resolved: &'a hir::ResolvedProgram,
    after_resolved: &'a hir::ResolvedProgram,
    patch: &'a SemanticPatch,
    expected_call_instances: &'a BTreeMap<String, hir::FunctionInstanceId>,
    expected_call_arguments: &'a BTreeMap<String, Vec<hir::ResolvedType>>,
}

fn validate_semantic_delta(inputs: SemanticDeltaInputs<'_>) -> Result<(), Vec<Diagnostic>> {
    let SemanticDeltaInputs {
        before_source,
        after_source,
        before,
        after,
        before_resolved,
        after_resolved,
        patch,
        expected_call_instances,
        expected_call_arguments,
    } = inputs;
    let targeted = expected_call_instances
        .keys()
        .cloned()
        .collect::<BTreeSet<_>>();
    let mut allowed_instances = expected_call_instances
        .values()
        .map(|instance| instance.as_str().to_owned())
        .collect::<BTreeSet<_>>();
    allowed_instances.extend(
        patch
            .call_type_argument_replacements
            .iter()
            .map(|replacement| replacement.old_instance.clone()),
    );
    let renamed_declarations = patch
        .renames
        .iter()
        .map(|rename| rename.stable_id.clone())
        .chain(
            patch
                .member_renames
                .iter()
                .map(|rename| rename.member.clone()),
        )
        .chain(patch.case_renames.iter().map(|rename| rename.case.clone()))
        .collect::<BTreeSet<_>>();
    let before_graph =
        normalized_semantic_graph(before, &targeted, &allowed_instances, &renamed_declarations)?;
    let after_graph =
        normalized_semantic_graph(after, &targeted, &allowed_instances, &renamed_declarations)?;
    if before_graph != after_graph {
        return Err(vec![Diagnostic::io(
            "SPX-G108",
            "semantic patch changed meaning outside its admitted identity-scoped delta",
        )]);
    }

    if patch.call_type_argument_replacements.is_empty() {
        return Ok(());
    }
    let before_instances = before_resolved
        .function_instances
        .iter()
        .map(|instance| instance.id.as_str().to_owned())
        .collect::<BTreeSet<_>>();
    let after_instances = after_resolved
        .function_instances
        .iter()
        .map(|instance| instance.id.as_str().to_owned())
        .collect::<BTreeSet<_>>();
    if before_instances
        .symmetric_difference(&after_instances)
        .any(|instance| !allowed_instances.contains(instance))
    {
        return Err(vec![Diagnostic::io(
            "SPX-G108",
            "generic call patch changed an unaddressed reachable function instance",
        )]);
    }
    let after_tokens =
        lexer::lex(after_source, "<semantic-delta>").map_err(|diagnostic| vec![diagnostic])?;
    let after_index =
        SemanticSourceIndex::build(after, after_resolved, &after_tokens).ok_or_else(|| {
            vec![Diagnostic::io(
                "SPX-G108",
                "post-patch source/HIR identity index is inconsistent",
            )]
        })?;
    let before_tokens =
        lexer::lex(before_source, "<semantic-delta>").map_err(|diagnostic| vec![diagnostic])?;
    let before_index = SemanticSourceIndex::build(before, before_resolved, &before_tokens)
        .ok_or_else(|| {
            vec![Diagnostic::io(
                "SPX-G108",
                "pre-patch source/HIR identity index is inconsistent",
            )]
        })?;

    for replacement in &patch.call_type_argument_replacements {
        let before_call = before_index
            .calls
            .get(&replacement.expression)
            .ok_or_else(|| call_selector_error(replacement, "pre-patch call disappeared"))?;
        let after_call = after_index
            .calls
            .get(&replacement.expression)
            .ok_or_else(|| call_selector_error(replacement, "post-patch call disappeared"))?;
        let index = replacement.index as usize;
        let expected = expected_call_instances
            .get(&replacement.expression)
            .expect("each replacement records an expected instance");
        let expected_arguments = expected_call_arguments
            .get(&replacement.expression)
            .expect("each replacement records expected arguments");
        if after_call.template != before_call.template
            || after_call.instance.as_deref() != Some(expected.as_str())
            || &after_call.type_arguments != expected_arguments
            || expected_arguments.get(index) != Some(&replacement.to.resolved())
        {
            return Err(call_selector_error(
                replacement,
                "post-HIR call delta exceeds the selected argument and derived instance",
            ));
        }
    }
    Ok(())
}

fn normalized_semantic_graph(
    program: &crate::ast::Program,
    targeted_calls: &BTreeSet<String>,
    allowed_instances: &BTreeSet<String>,
    renamed_declarations: &BTreeSet<String>,
) -> Result<serde_json::Value, Vec<Diagnostic>> {
    let source = graph::to_json(program)?;
    let mut value: serde_json::Value = serde_json::from_str(&source).map_err(|error| {
        vec![Diagnostic::io(
            "SPX-G108",
            format!("cannot inspect semantic graph delta: {error}"),
        )]
    })?;
    scrub_semantic_graph(
        &mut value,
        targeted_calls,
        allowed_instances,
        renamed_declarations,
    );
    Ok(value)
}

fn scrub_semantic_graph(
    value: &mut serde_json::Value,
    targeted_calls: &BTreeSet<String>,
    allowed_instances: &BTreeSet<String>,
    renamed_declarations: &BTreeSet<String>,
) {
    match value {
        serde_json::Value::Array(values) => {
            values.retain(|value| {
                let materialized_instance = value.get("kind").and_then(serde_json::Value::as_str)
                    == Some("function_instance")
                    && value.get("params").is_some()
                    && value.get("result_id").is_some();
                !materialized_instance
                    || !value
                        .get("instance")
                        .and_then(serde_json::Value::as_str)
                        .is_some_and(|instance| allowed_instances.contains(instance))
            });
            for value in values {
                scrub_semantic_graph(
                    value,
                    targeted_calls,
                    allowed_instances,
                    renamed_declarations,
                );
            }
        }
        serde_json::Value::Object(object) => {
            object.remove("revision");
            let kind = object
                .get("kind")
                .and_then(serde_json::Value::as_str)
                .map(str::to_owned);
            let id = object
                .get("id")
                .and_then(serde_json::Value::as_str)
                .map(str::to_owned);
            let declaration_kind = matches!(
                kind.as_deref(),
                Some(
                    "function"
                        | "function_template"
                        | "resource"
                        | "record"
                        | "field"
                        | "variant"
                        | "variant_case"
                        | "case_field"
                )
            );
            if declaration_kind
                && id
                    .as_deref()
                    .is_some_and(|id| renamed_declarations.contains(id))
            {
                object.remove("name");
            }
            if kind.as_deref() == Some("call_instance")
                && id.as_deref().is_some_and(|id| targeted_calls.contains(id))
            {
                object.remove("instance");
                object.remove("type_arguments");
            }
            for value in object.values_mut() {
                scrub_semantic_graph(
                    value,
                    targeted_calls,
                    allowed_instances,
                    renamed_declarations,
                );
            }
        }
        serde_json::Value::Null
        | serde_json::Value::Bool(_)
        | serde_json::Value::Number(_)
        | serde_json::Value::String(_) => {}
    }
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
#[path = "patch/commit_tests.rs"]
mod commit_tests;
