//! Bounded Project/workspace HIR linking profiles.
//!
//! This module links already-resolved HIR and rebuilds cleanup metadata. It has
//! no filesystem, process, publication, transport, or runtime authority.

use super::*;

/// Assemble one backend-ready scalar program from real resolved workspace
/// functions. This is intentionally narrower than general cross-file linking:
/// callers must have already resolved the complete provider closure, and only
/// effect-free functions over by-value Copy scalars are admitted. That surface
/// is exactly the Public Scalar Export Profile v1 ABI this linker feeds.
///
pub(crate) fn link_scalar_workspace(
    module: String,
    entrypoint: DeclarationId,
    linked_functions: Vec<LinkedScalarFunction>,
) -> Result<ResolvedProgram, Diagnostic> {
    link_scalar_workspace_impl(module, entrypoint, linked_functions, true, None)
}

/// Assemble one scalar program that additionally retains the authenticated
/// Native Rust import interfaces its closure calls. The scalar linker has no
/// callable ABI for an ordinary interface import, so every retained import
/// must be a native Rust callback; the retained imports' declared effects are
/// the only effects a retained function may itself declare.
pub(crate) fn link_scalar_native_rust_workspace(
    module: String,
    entrypoint: DeclarationId,
    linked_functions: Vec<LinkedScalarFunction>,
    natives: LinkedScalarNatives,
) -> Result<ResolvedProgram, Diagnostic> {
    link_scalar_workspace_impl(module, entrypoint, linked_functions, true, Some(natives))
}

/// Package builds need an internal `fn() -> i64` HIR anchor, but package
/// exports are identified persistently rather than by the source display name
/// `main`. Project callers continue through `link_scalar_workspace` above.
pub(crate) fn link_package_scalar_workspace(
    module: String,
    entrypoint: DeclarationId,
    linked_functions: Vec<LinkedScalarFunction>,
) -> Result<ResolvedProgram, Diagnostic> {
    link_scalar_workspace_impl(module, entrypoint, linked_functions, false, None)
}

/// Exact Native Rust interface inventory retained beside one scalar closure.
pub(crate) struct LinkedScalarNatives {
    pub(crate) interfaces: Vec<ResolvedInterface>,
    pub(crate) declaration_facts: BTreeMap<DeclarationId, LinkedDeclarationFact>,
}

fn link_scalar_workspace_impl(
    module: String,
    entrypoint: DeclarationId,
    mut linked_functions: Vec<LinkedScalarFunction>,
    require_main_display_name: bool,
    natives: Option<LinkedScalarNatives>,
) -> Result<ResolvedProgram, Diagnostic> {
    if linked_functions.is_empty() {
        return Err(link_error("workspace scalar closure has no functions"));
    }
    linked_functions.sort_by(|left, right| left.function.id.cmp(&right.function.id));

    // Native Rust callbacks are the only authority this profile can retain.
    // Their declared effects are exactly the effects an admitted function may
    // declare, so a closure with no retained interface stays effect-free and
    // links byte-identically to the original pure scalar profile.
    let mut import_effects = BTreeSet::new();
    for interface in natives.iter().flat_map(|natives| &natives.interfaces) {
        for import in &interface.imports {
            if !import.native_rust {
                return Err(link_error(format!(
                    "workspace interface import `{}` is outside the pure scalar linker profile",
                    import.id
                )));
            }
            import_effects.extend(import.effects.iter().cloned());
        }
    }

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
        if !function
            .effects
            .iter()
            .all(|effect| import_effects.contains(effect))
            || function
                .params
                .iter()
                .any(|parameter| parameter.ownership != OwnershipMode::Value)
            || !copy_scalar_type(&function.return_type)
            || function
                .params
                .iter()
                .any(|parameter| !copy_scalar_type(&parameter.ty))
        {
            return Err(link_error(format!(
                "workspace function `{}` is outside the pure scalar linker profile",
                function.id
            )));
        }
        if function.id == entrypoint {
            entry_origin = Some(linked.origin);
            if require_main_display_name && function.name != "main" {
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

    let origins = linked_functions
        .iter()
        .map(|linked| (linked.function.id.clone(), linked.origin))
        .collect::<BTreeMap<_, _>>();
    let functions = linked_functions
        .drain(..)
        .map(|linked| linked.function)
        .collect::<Vec<_>>();
    let mut declarations = DeclarationIndex::default();
    match &natives {
        Some(natives) => declarations.extend_linked_owned_data(
            &[],
            &natives.interfaces,
            &functions,
            &natives.declaration_facts,
        )?,
        None => {
            for function in &functions {
                let origin = origins
                    .get(&function.id)
                    .copied()
                    .ok_or_else(|| link_error("workspace scalar function origin is absent"))?;
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
        }
    }
    if !declarations.populate_type_facts() {
        return Err(link_error(
            "workspace scalar linker could not construct scalar type facts",
        ));
    }
    let mut linked = ResolvedProgram {
        module,
        permits: import_effects.into_iter().collect(),
        entrypoint,
        declarations,
        types: Vec::new(),
        interfaces: natives.map_or_else(Vec::new, |natives| natives.interfaces),
        function_templates: Vec::new(),
        functions,
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

pub(crate) fn compiler_prelude_declarations() -> Result<DeclarationIndex, Diagnostic> {
    let prelude_only = Program {
        path: "<workspace-linker>".to_owned(),
        module: "compiler.prelude".to_owned(),
        module_uses: Vec::new(),
        permits: Vec::new(),
        types: Vec::new(),
        interfaces: Vec::new(),
        protocols: Vec::new(),
        implementations: Vec::new(),
        functions: Vec::new(),
    };
    DeclarationIndex::from_verified(&prelude_only)
}

fn workspace_compiler_prelude(
) -> Result<(DeclarationIndex, Vec<ResolvedTypeDeclaration>), Diagnostic> {
    let declarations = compiler_prelude_declarations()?;
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

/// The narrow `i64`/`bool` result surface of the Useful Text Consumer profile,
/// which is unrelated to the public scalar export ABI and is not widened here.
fn scalar_type(ty: &ResolvedType) -> bool {
    matches!(ty, ResolvedType::I64 | ResolvedType::Bool)
}

/// The canonical SEMAPRAX spellings of [`copy_scalar_type`], in the profile's
/// canonical order. Every projection that names the admitted surface as wire
/// text reads this list, so the vocabulary cannot drift from the predicate.
pub(crate) const COPY_SCALAR_NAMES: [&str; 7] = ["i64", "i32", "u8", "char", "f32", "f64", "bool"];

/// The Copy scalars the Public Scalar Export Profile v1 admits. `usize` stays
/// outside: its width is a host fact, not a public fact of the profile, and
/// every remaining exclusion needs the owned-data memory ABI.
pub(crate) fn copy_scalar_type(ty: &ResolvedType) -> bool {
    matches!(
        ty,
        ResolvedType::I64
            | ResolvedType::I32
            | ResolvedType::U8
            | ResolvedType::Char
            | ResolvedType::F32
            | ResolvedType::F64
            | ResolvedType::Bool
    )
}

/// The one canonical SEMAPRAX spelling [`package_scalar_type`] adds to
/// [`COPY_SCALAR_NAMES`]. Every projection that names the admitted package
/// surface as wire text reads both lists, so neither can drift from its
/// predicate.
pub(crate) const PACKAGE_SCALAR_NAME: &str = "usize";

/// The by-value scalars a package-source interface admits. A package interface
/// is a SEMAPRAX-to-SEMAPRAX fact linked from exact source rather than a host
/// ABI boundary, so `usize` stays inside it: the length type is what the
/// built-in byte operations return. The host-facing Public Scalar Export
/// Profile v1 keeps excluding it.
pub(crate) fn package_scalar_type(ty: &ResolvedType) -> bool {
    copy_scalar_type(ty) || matches!(ty, ResolvedType::Usize)
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

#[cfg(test)]
mod tests {
    use std::path::Path;

    use crate::workspace_graph::{build_owned, WorkspaceSource};

    const HOST_EFFECT: &str = "host.adjust";

    fn source(path: &str, text: &str) -> WorkspaceSource {
        let program = crate::parse(text, Path::new(path)).expect("fixture source must parse");
        WorkspaceSource {
            path: path.to_owned(),
            source: crate::format::canonical(&program),
        }
    }

    fn test_module() -> WorkspaceSource {
        source(
            "src/tests.spx",
            "module test.main;\n\n@id(\"test.main\")\nfn main() -> i64 { 0 }\n",
        )
    }

    #[test]
    fn scalar_project_closure_retains_native_rust_imports_and_their_effectful_callers() {
        let app = source(
            "src/app.spx",
            r#"
module app.main;

permit { host.adjust }

@id("app.host.interface")
interface AppHost permits { host.adjust } {
    @id("app.host.adjust")
    import rust fn adjust(value: i64) -> i64
        effects { host.adjust }
        failure status "app.host.v1";
}

@id("app.apply")
fn apply(value: i64) -> i64 uses { host.adjust } { adjust(value) }

@id("app.main")
fn main() -> i64 uses { host.adjust } { apply(41) }
"#,
        );
        let (entry, test) = build_owned(vec![app, test_module()])
            .expect("workspace graph must build")
            .into_linked_scalar_programs("app.main", "test.main")
            .expect("scalar project closure must link");

        let [interface] = entry.interfaces.as_slice() else {
            panic!("the linked entry closure must retain exactly one interface");
        };
        assert_eq!(interface.id.as_str(), "app.host.interface");
        let [import] = interface.imports.as_slice() else {
            panic!("the retained interface must keep its one import");
        };
        assert!(import.native_rust);
        assert_eq!(import.id.as_str(), "app.host.adjust");
        assert_eq!(import.effects, [HOST_EFFECT.to_owned()]);
        assert_eq!(
            entry.declarations.native_rust_import_id("adjust"),
            Some(&import.id)
        );
        assert_eq!(entry.permits, [HOST_EFFECT.to_owned()]);
        let apply = entry
            .functions
            .iter()
            .find(|function| function.id.as_str() == "app.apply")
            .expect("the reachable effectful caller must stay admitted");
        assert_eq!(apply.effects, [HOST_EFFECT.to_owned()]);

        // An unrelated closure keeps the historical effect-free shape exactly.
        assert!(test.interfaces.is_empty());
        assert!(test.permits.is_empty());
    }

    #[test]
    fn scalar_project_closure_rejects_an_ordinary_interface_import() {
        let app = source(
            "src/app.spx",
            r#"
module app.main;

@id("app.token")
resource Token {
    @id("app.token.drop")
    drop import "app.host.release";
}

@id("app.host.interface")
interface AppHost permits {  } {
    @id("app.host.release")
    import fn release(token: own Token) -> unit
        effects {  }
        failure infallible
        consumes token always;
}

@id("app.main")
fn main() -> i64 { 0 }
"#,
        );
        let error = build_owned(vec![app, test_module()])
            .expect("workspace graph must build")
            .into_linked_scalar_programs("app.main", "test.main")
            .expect_err("the scalar linker has no ABI for an ordinary import");
        assert_eq!(error[0].code, "SPX-H006");
        assert_eq!(
            error[0].message,
            "workspace interface import `app.host.release` is outside the pure scalar linker profile"
        );
    }
}
