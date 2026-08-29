//! Bounded Project/workspace HIR linking profiles.
//!
//! This module links already-resolved HIR and rebuilds cleanup metadata. It has
//! no filesystem, process, publication, transport, or runtime authority.

use super::*;

/// Assemble one backend-ready scalar program from real resolved workspace
/// functions. This is intentionally narrower than general cross-file linking:
/// callers must have already resolved the complete provider closure, and only
/// value `i64`/`bool` functions without effects are admitted.
///
pub(crate) fn link_scalar_workspace(
    module: String,
    entrypoint: DeclarationId,
    mut linked_functions: Vec<LinkedScalarFunction>,
) -> Result<ResolvedProgram, Diagnostic> {
    if linked_functions.is_empty() {
        return Err(link_error("workspace scalar closure has no functions"));
    }
    linked_functions.sort_by(|left, right| left.function.id.cmp(&right.function.id));

    let mut seen = BTreeSet::new();
    let mut entry_origin = None;
    for linked in &linked_functions {
        let function = &linked.function;
        if !seen.insert(function.id.clone()) {
            return Err(link_error(format!(
                "workspace scalar closure duplicates function `{}`",
                function.id
            )));
        }
        if !function.effects.is_empty()
            || function
                .params
                .iter()
                .any(|parameter| parameter.ownership != OwnershipMode::Value)
            || !scalar_type(&function.return_type)
            || function
                .params
                .iter()
                .any(|parameter| !scalar_type(&parameter.ty))
        {
            return Err(link_error(format!(
                "workspace function `{}` is outside the pure scalar linker profile",
                function.id
            )));
        }
        if function.id == entrypoint {
            entry_origin = Some(linked.origin);
            if function.name != "main" {
                return Err(link_error(
                    "workspace scalar entry point is not an authored `main` function",
                ));
            }
        }
    }
    if entry_origin != Some(IdentityOrigin::Explicit) {
        return Err(link_error(
            "workspace scalar entry point must have an explicit authored identity",
        ));
    }

    let mut declarations = DeclarationIndex::default();
    for linked in &linked_functions {
        declarations.insert_top_level(
            linked.function.name.clone(),
            linked.function.id.clone(),
            DeclarationKind::Function,
            linked.origin,
        );
        declarations
            .type_parameters
            .insert(linked.function.id.clone(), Vec::new());
    }
    if !declarations.populate_type_facts() {
        return Err(link_error(
            "workspace scalar linker could not construct scalar type facts",
        ));
    }
    let mut linked = ResolvedProgram {
        module,
        permits: Vec::new(),
        entrypoint,
        declarations,
        types: Vec::new(),
        interfaces: Vec::new(),
        function_templates: Vec::new(),
        functions: linked_functions
            .drain(..)
            .map(|linked| linked.function)
            .collect(),
        function_instances: Vec::new(),
    };
    rebuild_cleanup_metadata(&mut linked)?;
    validate(&linked)?;
    Ok(linked)
}

/// Assemble one backend-ready Useful Text Consumer program from authenticated
/// workspace functions. The authored entry remains the exact scalar `main`,
/// while additional selected roots may accept only non-escaping `borrow str`
/// views and return scalar values.
pub(crate) fn link_useful_text_workspace(
    module: String,
    entrypoint: DeclarationId,
    mut linked_functions: Vec<LinkedScalarFunction>,
) -> Result<ResolvedProgram, Diagnostic> {
    if linked_functions.is_empty() {
        return Err(link_error("workspace text closure has no functions"));
    }
    linked_functions.sort_by(|left, right| left.function.id.cmp(&right.function.id));

    let mut seen = BTreeSet::new();
    let mut entry_origin = None;
    for linked in &linked_functions {
        let function = &linked.function;
        if !seen.insert(function.id.clone()) {
            return Err(link_error(format!(
                "workspace text closure duplicates function `{}`",
                function.id
            )));
        }
        if !function.effects.is_empty()
            || !scalar_type(&function.return_type)
            || function.params.iter().any(|parameter| {
                !matches!(
                    (&parameter.ty, parameter.ownership),
                    (ResolvedType::I64 | ResolvedType::Bool, OwnershipMode::Value)
                        | (ResolvedType::Str, OwnershipMode::Borrow)
                )
            })
        {
            return Err(link_error(format!(
                "workspace function `{}` is outside the Useful Text Consumer linker profile",
                function.id
            )));
        }
        if function.id == entrypoint {
            entry_origin = Some(linked.origin);
            if function.name != "main"
                || !function.params.is_empty()
                || function.return_type != ResolvedType::I64
            {
                return Err(link_error(
                    "workspace text entry point must be an authored `fn main() -> i64`",
                ));
            }
        }
    }
    if entry_origin != Some(IdentityOrigin::Explicit) {
        return Err(link_error(
            "workspace text entry point must have an explicit authored identity",
        ));
    }

    let mut declarations = DeclarationIndex::default();
    for linked in &linked_functions {
        declarations.insert_top_level(
            linked.function.name.clone(),
            linked.function.id.clone(),
            DeclarationKind::Function,
            linked.origin,
        );
        declarations
            .type_parameters
            .insert(linked.function.id.clone(), Vec::new());
    }
    if !declarations.populate_type_facts() {
        return Err(link_error(
            "workspace text linker could not construct type facts",
        ));
    }
    let mut linked = ResolvedProgram {
        module,
        permits: Vec::new(),
        entrypoint,
        declarations,
        types: Vec::new(),
        interfaces: Vec::new(),
        function_templates: Vec::new(),
        functions: linked_functions
            .drain(..)
            .map(|linked| linked.function)
            .collect(),
        function_instances: Vec::new(),
    };
    rebuild_cleanup_metadata(&mut linked)?;
    validate(&linked)?;
    Ok(linked)
}

/// Assemble one backend-ready Useful Data program from authenticated
/// workspace functions. The authored entry remains the exact scalar `main`;
/// additional closure functions may use only the closed byte-data value and
/// borrow kinds. Slice provenance is reconstructed from retained expressions
/// before cleanup and hostile-HIR validation, never copied from the source
/// modules' attached declaration indexes.
pub(crate) fn link_useful_data_workspace(
    module: String,
    entrypoint: DeclarationId,
    linked_functions: Vec<LinkedScalarFunction>,
) -> Result<ResolvedProgram, Diagnostic> {
    link_useful_data_workspace_profile(
        module,
        entrypoint,
        linked_functions,
        WorkspaceIoProfile::Pure,
    )
}

/// Assemble the exact Project-v8 entry plus selected public-function closure.
/// Public signature, effect, import, contract, and acyclicity policy remains
/// owned by the canonical API descriptor. This route retains only the
/// ordinary verified record/variant declarations and interfaces structurally
/// required to validate and lower that already-selected closure.
pub(crate) fn link_owned_data_api_workspace(
    module: String,
    entrypoint: DeclarationId,
    mut linked_functions: Vec<LinkedScalarFunction>,
    parts: LinkedOwnedDataParts,
) -> Result<ResolvedProgram, Diagnostic> {
    if linked_functions.is_empty() {
        return Err(link_error("workspace owned-data closure has no functions"));
    }
    linked_functions.sort_by(|left, right| left.function.id.cmp(&right.function.id));
    let mut seen = BTreeSet::new();
    let mut entry_origin = None;
    for linked in &linked_functions {
        if !seen.insert(linked.function.id.clone()) {
            return Err(link_error(format!(
                "workspace owned-data closure duplicates function `{}`",
                linked.function.id
            )));
        }
        if linked.function.id == entrypoint {
            entry_origin = Some(linked.origin);
            if linked.function.name != "main"
                || !linked.function.params.is_empty()
                || linked.function.return_type != ResolvedType::I64
            {
                return Err(link_error(
                    "workspace owned-data entry point must be an authored `fn main() -> i64`",
                ));
            }
        }
    }
    if entry_origin != Some(IdentityOrigin::Explicit) {
        return Err(link_error(
            "workspace owned-data entry point must have an explicit authored identity",
        ));
    }

    let functions = linked_functions
        .drain(..)
        .map(|linked| linked.function)
        .collect::<Vec<_>>();
    let (mut declarations, mut types) = workspace_compiler_prelude()?;
    declarations.extend_linked_owned_data(
        &parts.types,
        &parts.interfaces,
        &functions,
        &parts.declaration_facts,
    )?;
    declarations.byte_slice_roots = derive_byte_slice_provenance(&functions, &declarations)?;
    if !declarations.populate_type_facts() {
        return Err(link_error(
            "workspace owned-data linker could not construct exact type facts",
        ));
    }
    types.extend(parts.types);
    let mut linked = ResolvedProgram {
        module,
        permits: parts.permits,
        entrypoint,
        declarations,
        types,
        interfaces: parts.interfaces,
        function_templates: Vec::new(),
        functions,
        function_instances: Vec::new(),
    };
    analyze_byte_data_capacity(&linked)?;
    rebuild_cleanup_metadata(&mut linked)?;
    validate(&linked)?;
    Ok(linked)
}

/// Assemble the additive Project v4 command closure. This keeps the Useful
/// Data value/profile boundary intact while reconstructing exactly the one
/// compiler-owned stdout capability authenticated by the manifest/linker.
pub(crate) fn link_useful_data_command_workspace(
    module: String,
    entrypoint: DeclarationId,
    linked_functions: Vec<LinkedScalarFunction>,
) -> Result<ResolvedProgram, Diagnostic> {
    link_useful_data_workspace_profile(
        module,
        entrypoint,
        linked_functions,
        WorkspaceIoProfile::Stdout,
    )
}

/// Assemble the exact Project-v6 bounded language-command closure. This is a
/// separate admission route: it cannot inherit the older stdout-only profile
/// or broaden that profile's entrypoint/result contract.
pub(crate) fn link_language_command_io_workspace(
    module: String,
    entrypoint: DeclarationId,
    command: DeclarationId,
    linked_functions: Vec<LinkedScalarFunction>,
) -> Result<ResolvedProgram, Diagnostic> {
    link_useful_data_workspace_profile(
        module,
        entrypoint,
        linked_functions,
        WorkspaceIoProfile::LanguageCommand { command },
    )
}

/// Assemble the additive Project-v7 line-command closure. The carrier and
/// permits match Project v6, but admission additionally proves that the
/// selected command closure uses the bounded byte-range and append primitives
/// and cannot fall back to either legacy transcript-write operation.
pub(crate) fn link_line_command_io_workspace(
    module: String,
    entrypoint: DeclarationId,
    command: DeclarationId,
    linked_functions: Vec<LinkedScalarFunction>,
) -> Result<ResolvedProgram, Diagnostic> {
    link_useful_data_workspace_profile(
        module,
        entrypoint,
        linked_functions,
        WorkspaceIoProfile::LineCommand { command },
    )
}

#[derive(Clone, Eq, PartialEq)]
enum WorkspaceIoProfile {
    Pure,
    Stdout,
    LanguageCommand { command: DeclarationId },
    LineCommand { command: DeclarationId },
}

fn link_useful_data_workspace_profile(
    module: String,
    entrypoint: DeclarationId,
    mut linked_functions: Vec<LinkedScalarFunction>,
    profile: WorkspaceIoProfile,
) -> Result<ResolvedProgram, Diagnostic> {
    if linked_functions.is_empty() {
        return Err(link_error("workspace useful-data closure has no functions"));
    }
    linked_functions.sort_by(|left, right| left.function.id.cmp(&right.function.id));

    let mut seen = BTreeSet::new();
    let mut entry_origin = None;
    for linked in &linked_functions {
        let function = &linked.function;
        if !seen.insert(function.id.clone()) {
            return Err(link_error(format!(
                "workspace useful-data closure duplicates function `{}`",
                function.id
            )));
        }
        let effects_admitted = match &profile {
            WorkspaceIoProfile::Pure => function.effects.is_empty(),
            WorkspaceIoProfile::Stdout => {
                function.effects.is_empty()
                    || function.effects == [crate::host_io_ops::STDOUT_WRITE_EFFECT]
            }
            WorkspaceIoProfile::LanguageCommand { .. } => function.effects.iter().all(|effect| {
                matches!(
                    effect.as_str(),
                    crate::command_io_ops::ARGS_READ_EFFECT
                        | crate::command_io_ops::STDIN_READ_EFFECT
                        | crate::command_io_ops::STDERR_WRITE_EFFECT
                        | crate::host_io_ops::STDOUT_WRITE_EFFECT
                )
            }),
            WorkspaceIoProfile::LineCommand { .. } => function.effects.iter().all(|effect| {
                matches!(
                    effect.as_str(),
                    crate::command_io_ops::ARGS_READ_EFFECT
                        | crate::command_io_ops::STDIN_READ_EFFECT
                        | crate::command_io_ops::STDERR_WRITE_EFFECT
                        | crate::command_io_ops::STDOUT_WRITE_EFFECT
                )
            }),
        };
        let return_admitted = useful_data_workspace_return_admitted(&function.return_type);
        if !effects_admitted
            || !return_admitted
            || function.params.iter().any(|parameter| {
                !useful_data_workspace_parameter_admitted(&parameter.ty, parameter.ownership)
            })
        {
            return Err(link_error(format!(
                "workspace function `{}` is outside the Useful Data linker profile",
                function.id
            )));
        }
        if function.id == entrypoint {
            entry_origin = Some(linked.origin);
            if function.name != "main"
                || !function.params.is_empty()
                || function.return_type != ResolvedType::I64
            {
                return Err(link_error(
                    "workspace useful-data entry point must be an authored `fn main() -> i64`",
                ));
            }
        }
    }
    if entry_origin != Some(IdentityOrigin::Explicit) {
        return Err(link_error(
            "workspace useful-data entry point must have an explicit authored identity",
        ));
    }
    if let WorkspaceIoProfile::LanguageCommand { command }
    | WorkspaceIoProfile::LineCommand { command } = &profile
    {
        let selected = linked_functions
            .iter()
            .find(|linked| &linked.function.id == command)
            .ok_or_else(|| link_error("workspace language-command identity is absent"))?;
        if selected.origin != IdentityOrigin::Explicit
            || !selected.function.params.is_empty()
            || selected.function.return_type != ResolvedType::Bool
        {
            return Err(link_error(
                "workspace language command must be an explicit stable-ID `fn () -> bool`",
            ));
        }
    }

    let origins = linked_functions
        .iter()
        .map(|linked| (linked.function.id.clone(), linked.origin))
        .collect::<BTreeMap<_, _>>();
    let functions = linked_functions
        .drain(..)
        .map(|linked| linked.function)
        .collect::<Vec<_>>();
    // Useful-data expressions can carry the compiler-owned `Option<u8>`
    // result of `byte_get`. Rebuild the canonical prelude declaration facts
    // before inserting retained workspace functions; a default index would
    // lose the nominal type behind match/capacity validation.
    let (mut declarations, compiler_types) = workspace_compiler_prelude()?;
    for function in &functions {
        let origin = origins
            .get(&function.id)
            .copied()
            .ok_or_else(|| link_error("workspace useful-data function origin is absent"))?;
        declarations.insert_top_level(
            function.name.clone(),
            function.id.clone(),
            DeclarationKind::Function,
            origin,
        );
        declarations
            .type_parameters
            .insert(function.id.clone(), Vec::new());
    }
    declarations.byte_slice_roots = derive_byte_slice_provenance(&functions, &declarations)?;
    if !declarations.populate_type_facts() {
        return Err(link_error(
            "workspace useful-data linker could not construct type facts",
        ));
    }
    let mut linked = ResolvedProgram {
        module,
        permits: match &profile {
            WorkspaceIoProfile::Pure => Vec::new(),
            WorkspaceIoProfile::Stdout => vec![crate::host_io_ops::STDOUT_WRITE_EFFECT.to_owned()],
            WorkspaceIoProfile::LanguageCommand { .. } => vec![
                crate::command_io_ops::ARGS_READ_EFFECT.to_owned(),
                crate::command_io_ops::STDERR_WRITE_EFFECT.to_owned(),
                crate::command_io_ops::STDIN_READ_EFFECT.to_owned(),
                crate::host_io_ops::STDOUT_WRITE_EFFECT.to_owned(),
            ],
            WorkspaceIoProfile::LineCommand { .. } => vec![
                crate::command_io_ops::ARGS_READ_EFFECT.to_owned(),
                crate::command_io_ops::STDERR_WRITE_EFFECT.to_owned(),
                crate::command_io_ops::STDIN_READ_EFFECT.to_owned(),
                crate::command_io_ops::STDOUT_WRITE_EFFECT.to_owned(),
            ],
        },
        entrypoint,
        declarations,
        types: compiler_types,
        interfaces: Vec::new(),
        function_templates: Vec::new(),
        functions,
        function_instances: Vec::new(),
    };
    match &profile {
        WorkspaceIoProfile::LanguageCommand { command } => {
            crate::command_io_ops::validate_operation_profile(
                &linked,
                command,
                crate::command_io_ops::CommandOperationProfile::LanguageV1,
            )?;
        }
        WorkspaceIoProfile::LineCommand { command } => {
            crate::command_io_ops::validate_operation_profile(
                &linked,
                command,
                crate::command_io_ops::CommandOperationProfile::LineV1,
            )?;
        }
        WorkspaceIoProfile::Pure | WorkspaceIoProfile::Stdout => {}
    }
    analyze_byte_data_capacity(&linked)?;
    rebuild_cleanup_metadata(&mut linked)?;
    validate(&linked)?;
    Ok(linked)
}

pub(crate) fn useful_data_workspace_parameter_admitted(
    ty: &ResolvedType,
    ownership: OwnershipMode,
) -> bool {
    matches!(
        (ty, ownership),
        (
            ResolvedType::I64
                | ResolvedType::Bool
                | ResolvedType::U8
                | ResolvedType::Usize
                | ResolvedType::ArrayU8(_),
            OwnershipMode::Value
        ) | (ResolvedType::Bytes, OwnershipMode::Own)
            | (
                ResolvedType::Str | ResolvedType::SliceU8,
                OwnershipMode::Borrow
            )
    )
}

pub(crate) fn useful_data_workspace_return_admitted(ty: &ResolvedType) -> bool {
    matches!(
        ty,
        ResolvedType::I64
            | ResolvedType::Bool
            | ResolvedType::U8
            | ResolvedType::Usize
            | ResolvedType::ArrayU8(_)
            | ResolvedType::Bytes
    )
}

pub(crate) fn owned_data_api_workspace_return_admitted(ty: &ResolvedType) -> bool {
    useful_data_workspace_return_admitted(ty)
        || matches!(
            ty,
            ResolvedType::Nominal {
                declaration,
                arguments,
            } if (declaration.as_str() == crate::prelude::OPTION_ID
                && arguments.as_slice() == [ResolvedType::Bytes])
                || (declaration.as_str() == crate::prelude::RESULT_ID
                    && arguments.as_slice() == [ResolvedType::Bytes, ResolvedType::I64])
        )
}

fn workspace_compiler_prelude(
) -> Result<(DeclarationIndex, Vec<ResolvedTypeDeclaration>), Diagnostic> {
    let prelude_only = Program {
        path: "<workspace-linker>".to_owned(),
        module: "compiler.prelude".to_owned(),
        module_uses: Vec::new(),
        permits: Vec::new(),
        types: Vec::new(),
        interfaces: Vec::new(),
        protocols: Vec::new(),
        functions: Vec::new(),
    };
    let declarations = DeclarationIndex::from_verified(&prelude_only)?;
    let compiler_types = crate::prelude::declarations()
        .iter()
        .map(|declaration| {
            let id = DeclarationId::new(declaration.stable_id.clone());
            let TypeDeclarationKind::Variant { .. } = &declaration.kind else {
                return Err(link_error(
                    "workspace linker prelude contains an unsupported type kind",
                ));
            };
            Ok(ResolvedTypeDeclaration {
                type_parameters: declarations
                    .type_parameters(&id)
                    .ok_or_else(|| link_error("workspace prelude type parameters are absent"))?
                    .to_vec(),
                kind: ResolvedTypeDeclarationKind::Variant {
                    cases: declarations
                        .variant_cases(&id)
                        .ok_or_else(|| link_error("workspace prelude variant cases are absent"))?
                        .to_vec(),
                },
                id,
                name: declaration.name.clone(),
                span: declaration.span,
            })
        })
        .collect::<Result<Vec<_>, Diagnostic>>()?;
    Ok((declarations, compiler_types))
}

fn scalar_type(ty: &ResolvedType) -> bool {
    matches!(ty, ResolvedType::I64 | ResolvedType::Bool)
}

fn link_error(message: impl Into<String>) -> Diagnostic {
    Diagnostic::io("SPX-H006", message)
}

fn rebuild_cleanup_metadata(program: &mut ResolvedProgram) -> Result<(), Diagnostic> {
    let loan_plans = program
        .functions
        .iter()
        .map(|function| crate::loan_plan::build_plan(program, function))
        .collect::<Result<Vec<_>, _>>()?;
    for (function, loan_plan) in program.functions.iter_mut().zip(loan_plans) {
        function.loan_plan = loan_plan;
    }
    let inventories = program
        .functions
        .iter()
        .map(|function| crate::cleanup::build_inventory(program, function))
        .collect::<Result<Vec<_>, _>>()?;
    for (function, inventory) in program.functions.iter_mut().zip(inventories) {
        function.cleanup = inventory;
    }
    let cleanup_plans = program
        .functions
        .iter()
        .map(|function| crate::cleanup_plan::build_plan(program, function))
        .collect::<Result<Vec<_>, _>>()?;
    for (function, cleanup_plan) in program.functions.iter_mut().zip(cleanup_plans) {
        function.cleanup_plan = cleanup_plan;
    }
    Ok(())
}
