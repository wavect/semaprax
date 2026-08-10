mod source_index;

use std::collections::{BTreeMap, BTreeSet};
use std::ffi::OsString;
use std::fs::{File, Metadata, OpenOptions, Permissions};
use std::io::{Read, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};

use crate::ast::{Program, Type, TypeDeclaration, TypeDeclarationKind};
use crate::diagnostic::Diagnostic;
use crate::{format, graph, hir, lexer, parse, verify};

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
    operations: Vec<PreflightOperation>,
}

#[derive(Clone, Debug)]
pub(crate) enum PreflightOperation {
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
enum CommitPhase {
    BeforeFinalCheck,
    BeforeRename,
}

pub fn apply(source_path: &Path, patch_path: &Path) -> Result<String, Vec<Diagnostic>> {
    apply_with_commit_hook(source_path, patch_path, |_, _, _| Ok(()))
}

pub(crate) fn preflight_owned(
    source: String,
    patch_source: String,
    diagnostic_path: PathBuf,
) -> Result<PatchPreflight, Vec<Diagnostic>> {
    let patch = parse_patch(&patch_source)?;
    let before = parse(&source, &diagnostic_path).map_err(|error| vec![error])?;
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

    let before_resolved = if patch.schema == PatchSchema::V2 {
        Some(hir::resolve(&before)?)
    } else {
        None
    };
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
                previous
                    .operation_indices
                    .extend(edit.operation_indices.into_iter());
                continue;
            }
        }
        checked_planned_edits.push(edit);
    }
    assign_source_consumers(&before, &mut checked_planned_edits);
    let mut changed = source.clone();
    for (start, end, replacement) in checked_replacements.into_iter().rev() {
        changed.replace_range(start..end, &replacement);
    }

    let candidate = parse(&changed, &diagnostic_path).map_err(|error| vec![error])?;
    let diagnostics = verify::verify(&candidate);
    if diagnostics.iter().any(|item| item.severity.is_error()) {
        return Err(diagnostics);
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
    let canonical_candidate = format::canonical(&candidate);
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
    })
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
    let preflight = preflight_owned(source, patch_source, source_path.to_path_buf())?;
    let canonical_bytes = preflight.canonical_candidate().as_bytes();
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
        preflight.base_revision(),
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
        preflight.base_revision(),
    )?;
    staging.validate_contents(canonical_bytes)?;
    std::fs::rename(&staging.path, &canonical_source_path).map_err(|error| {
        vec![Diagnostic::io(
            "SPX-I204",
            format!("cannot atomically commit semantic patch: {error}"),
        )]
    })?;
    staging.committed();
    Ok(preflight.candidate_revision().to_owned())
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

pub(crate) fn read_source_snapshot(path: &Path) -> Result<SourceSnapshot, Vec<Diagnostic>> {
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
                reject_duplicate_selector(schema, &mut selectors, format!("rename:{stable_id}"))?;
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
                    format!("member:{owner}:{member}"),
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
                reject_duplicate_selector(schema, &mut selectors, format!("case:{owner}:{case}"))?;
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
                    format!("call:{expression}:{index}"),
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
                    "require:no-new-effects".to_owned(),
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
        operations,
    })
}

fn reject_duplicate_selector(
    schema: PatchSchema,
    selectors: &mut BTreeSet<String>,
    selector: String,
) -> Result<(), Vec<Diagnostic>> {
    if schema == PatchSchema::V2 && !selectors.insert(selector.clone()) {
        return Err(patch_conflict(format!(
            "duplicate semantic patch selector `{selector}`"
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
            TypeDeclarationKind::Resource { .. } | TypeDeclarationKind::Record { .. } => {}
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
