//! Bounded Project/workspace HIR linking profiles.
//!
//! This module links already-resolved HIR and rebuilds cleanup metadata. It has
//! no filesystem, process, publication, transport, or runtime authority.

use super::*;

mod compiler_prelude;
mod profile_diagnostics;
mod stdin_stream;
mod stream_owned;
mod stream_record;
pub(crate) use stdin_stream::{
    link_stdin_stream_command_workspace, link_stdin_stream_data_command_workspace,
    link_stdin_stream_data_entry_workspace, link_stdin_stream_exit_command_workspace,
    link_stdin_stream_text_command_workspace, link_stdin_stream_text_entry_workspace,
    stream_data_parameter_admitted, stream_text_parameter_admitted,
    stream_text_parameter_with_index, stream_text_return_admitted, stream_text_return_with_index,
    validate_stream_data_program, validate_stream_text_program,
};
pub(crate) use stream_owned::function_requires_owned_profile as stream_owned_function_requires_profile;
pub(crate) use stream_owned::{stream_owned_signature_admitted, validate_stream_owned_program};
pub(crate) use stream_record::{stream_record_signature_admitted, validate_stream_record_program};
pub(in crate::hir) mod native_owner;

pub(crate) use compiler_prelude::compiler_prelude_declarations;
use compiler_prelude::{
    compiler_prelude_declarations_for, compiler_prelude_declarations_for_vec,
    workspace_compiler_prelude, workspace_compiler_prelude_for,
    workspace_compiler_prelude_for_stream_data, workspace_compiler_prelude_for_vec,
    workspace_linker_prelude_program,
};

/// Assemble one backend-ready scalar program from real resolved workspace
/// functions. This is intentionally narrower than general cross-file linking:
/// callers must have already resolved the complete provider closure, and only
/// effect-free functions over by-value Copy scalars are admitted. That surface
/// is exactly the Public Scalar Export Profile v1 ABI this linker feeds.
///
pub(crate) fn link_scalar_project_workspace(
    module: String,
    entrypoint: DeclarationId,
    linked_functions: Vec<LinkedScalarFunction>,
    parts: LinkedScalarProjectParts,
) -> Result<ResolvedProgram, Diagnostic> {
    link_scalar_workspace_impl(module, entrypoint, linked_functions, true, Some(parts))
}

pub(crate) fn link_scalar_project_exports(
    module: String,
    entrypoint: DeclarationId,
    linked_functions: Vec<LinkedScalarFunction>,
    parts: LinkedScalarProjectParts,
) -> Result<ResolvedProgram, Diagnostic> {
    link_scalar_workspace_impl(module, entrypoint, linked_functions, false, Some(parts))
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

pub(crate) struct LinkedScalarProjectParts {
    pub(crate) private_callable_functions: BTreeSet<DeclarationId>,
    pub(crate) types: Vec<ResolvedTypeDeclaration>,
    pub(crate) interfaces: Vec<ResolvedInterface>,
    pub(crate) function_templates: Vec<ResolvedFunctionTemplate>,
    pub(crate) function_instances: Vec<ResolvedFunctionInstance>,
    pub(crate) declaration_facts: BTreeMap<DeclarationId, LinkedDeclarationFact>,
}

fn link_scalar_workspace_impl(
    module: String,
    entrypoint: DeclarationId,
    mut linked_functions: Vec<LinkedScalarFunction>,
    require_main_display_name: bool,
    parts: Option<LinkedScalarProjectParts>,
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
    for interface in parts.iter().flat_map(|parts| &parts.interfaces) {
        for import in &interface.imports {
            if !import.native_rust
                && !parts
                    .as_ref()
                    .is_some_and(|parts| native_owner::admitted_finalizer(parts, import))
            {
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
        let is_owned_method = function
            .params
            .first()
            .is_some_and(|parameter| parameter.name == "self");
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
            || (!is_owned_method
                && !parts.as_ref().is_some_and(|parts| {
                    (function.id != entrypoint
                        && parts.private_callable_functions.contains(&function.id)
                        && function_value::private_helper_signature(function))
                        || (function.id != entrypoint
                            && native_owner::admitted_helper(
                                &parts.types,
                                &parts.interfaces,
                                function,
                            ))
                        || generic_result::concrete_signature(function)
                        || generic_collection::concrete_signature(function)
                        || (generic_variant::concrete_signature(&parts.types, function)
                            && function
                                .params
                                .iter()
                                .map(|p| &p.ty)
                                .chain(std::iter::once(&function.return_type))
                                .all(|ty| {
                                    let ResolvedType::Nominal { declaration, .. } = ty else {
                                        return true;
                                    };
                                    parts
                                        .declaration_facts
                                        .get(declaration)
                                        .is_some_and(|fact| {
                                            fact.kind == DeclarationKind::Variant
                                                && fact.origin == IdentityOrigin::Explicit
                                        })
                                }))
                })
                && (function
                    .params
                    .iter()
                    .any(|parameter| parameter.ownership != OwnershipMode::Value)
                    || !copy_scalar_type(&function.return_type)
                    || function
                        .params
                        .iter()
                        .any(|parameter| !copy_scalar_type(&parameter.ty))))
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
    // Selected native Result owners may occur only inside a scalar body.
    // Retain their compiler-owned Result declaration before rebuilding cleanup.
    let uses_owned_result = parts.as_ref().is_some_and(|parts| {
        parts
            .types
            .iter()
            .any(|ty| native_owner::admitted_ri06_regex_resource(ty, &parts.interfaces))
    }) || functions.iter().any(generic_result::concrete_signature)
        || parts.as_ref().is_some_and(|parts| {
            functions
                .iter()
                .any(|function| generic_variant::concrete_signature(&parts.types, function))
        })
        || parts
            .iter()
            .flat_map(|parts| &parts.function_templates)
            .any(generic_result::profile);
    let (mut declarations, mut compiler_types) =
        compiler_prelude::selected_for_scalar(&functions, parts.as_ref(), uses_owned_result)?;
    match &parts {
        Some(parts) => declarations.extend_linked_scalar_data(
            &parts.types,
            &parts.interfaces,
            &functions,
            &parts.function_templates,
            &parts.declaration_facts,
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
    let (mut types, interfaces, function_templates, function_instances) = parts.map_or_else(
        || (Vec::new(), Vec::new(), Vec::new(), Vec::new()),
        |parts| {
            (
                parts.types,
                parts.interfaces,
                parts.function_templates,
                parts.function_instances,
            )
        },
    );
    compiler_types.extend(types);
    types = compiler_types;
    let has_generic_instances = !function_instances.is_empty();
    if has_generic_instances {
        let mut cleanup_functions = functions.clone();
        cleanup_functions.extend(
            function_instances
                .iter()
                .map(|instance| instance.function.clone()),
        );
        declarations.byte_slice_roots =
            derive_byte_slice_provenance(&cleanup_functions, &declarations)?;
    } else {
        // Scalar signatures may contain checked native returned views and byte
        // inspections. Rebuild their facts from the exact retained bodies too.
        declarations.byte_slice_roots = derive_byte_slice_provenance(&functions, &declarations)?;
    }
    let mut linked = ResolvedProgram {
        module,
        permits: import_effects.into_iter().collect(),
        agents: Vec::new(),
        entrypoint,
        declarations,
        types,
        interfaces,
        function_templates,
        functions,
        function_instances,
    };
    if has_generic_instances {
        analyze_byte_data_capacity(&linked)?;
    }
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

    // A borrowed-`str` body may convert its view with `str_as_bytes` and
    // inspect it with `byte_get`, whose result is the compiler-owned
    // `Option<u8>`. That match never crosses a declared signature above (the
    // profile's admitted parameter/return surface stays exactly `i64`/`bool`
    // by value and `borrow str`), but the internal HIR still names the
    // canonical `core.option` declaration and needs it registered the same
    // way the Useful Data linker registers it: rebuild the canonical prelude
    // declaration facts before inserting retained workspace functions, so a
    // default index does not lose the nominal type behind match validation.
    let origins = linked_functions
        .iter()
        .map(|linked| (linked.function.id.clone(), linked.origin))
        .collect::<BTreeMap<_, _>>();
    let functions = linked_functions
        .drain(..)
        .map(|linked| linked.function)
        .collect::<Vec<_>>();
    let (mut declarations, compiler_types) = workspace_compiler_prelude()?;
    for function in &functions {
        let origin = origins
            .get(&function.id)
            .copied()
            .ok_or_else(|| link_error("workspace text function origin is absent"))?;
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
            "workspace text linker could not construct type facts",
        ));
    }
    let mut linked = ResolvedProgram {
        module,
        permits: Vec::new(),
        agents: Vec::new(),
        entrypoint,
        declarations,
        types: compiler_types,
        interfaces: Vec::new(),
        function_templates: Vec::new(),
        functions,
        function_instances: Vec::new(),
    };
    analyze_byte_data_capacity(&linked)?;
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
    let uses_private_collections = functions.iter().any(|f| {
        crate::map_ops::is_collection(&f.return_type)
            || f.params
                .iter()
                .any(|p| crate::map_ops::is_collection(&p.ty))
    }) || functions.iter().any(|f| {
        f.requires
            .iter()
            .chain(std::iter::once(&f.body))
            .chain(&f.ensures)
            .any(|root| {
                let mut found = false;
                crate::hir::visit_resolved_calls(root, &mut |callee, _, _| {
                    found |= crate::map_ops::by_id(callee.as_str()).is_some()
                });
                found
            })
    }) || parts
        .types
        .iter()
        .any(|t| crate::map_ops::is_declaration(t.id.as_str()));
    let uses_stream = functions
        .iter()
        .any(crate::stdin_stream_ops::resolved_function_uses);
    let uses_vec = functions.iter().any(resolved_function_uses_vec);
    let (mut declarations, mut types) = if uses_private_collections && uses_vec {
        compiler_prelude::workspace_compiler_prelude_for_stream_data_collections()?
    } else if uses_private_collections {
        compiler_prelude::workspace_compiler_prelude_for_collections()?
    } else if uses_stream && uses_vec {
        compiler_prelude::workspace_compiler_prelude_for_stream_data()?
    } else if uses_stream {
        compiler_prelude::workspace_compiler_prelude_for_stream()?
    } else {
        compiler_prelude::selected_for_owned_data(&functions, &parts)?
    };
    declarations.extend_linked_owned_data(
        &parts.types,
        &parts.interfaces,
        &functions,
        &parts.function_templates,
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
        agents: Vec::new(),
        entrypoint,
        declarations,
        types,
        interfaces: parts.interfaces,
        function_templates: parts.function_templates,
        functions,
        function_instances: parts.function_instances,
    };
    analyze_byte_data_capacity(&linked)?;
    rebuild_cleanup_metadata(&mut linked)?;
    validate(&linked)?;
    Ok(linked)
}

fn resolved_function_uses_vec(function: &ResolvedFunction) -> bool {
    function
        .requires
        .iter()
        .chain(std::iter::once(&function.body))
        .chain(&function.ensures)
        .any(|expression| {
            let mut found = false;
            visit_resolved_calls(expression, &mut |callee, instance, arguments| {
                found |= instance.is_none()
                    && arguments.len() == 1
                    && crate::vec_ops::by_id(callee.as_str()).is_some();
            });
            found
        })
}
fn resolved_function_uses_box(function: &ResolvedFunction) -> bool {
    function
        .requires
        .iter()
        .chain(std::iter::once(&function.body))
        .chain(&function.ensures)
        .any(|expression| {
            let mut found = false;
            visit_resolved_calls(expression, &mut |callee, instance, arguments| {
                found |= instance.is_none()
                    && arguments.len() == 1
                    && crate::box_ops::by_id(callee.as_str()).is_some()
            });
            found
        })
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

/// Assemble the Project-v12 bounded network-command closure. Target adapters
/// still require an explicitly injected provider before any network authority
/// exists.
pub(crate) fn link_network_command_io_workspace(
    module: String,
    entrypoint: DeclarationId,
    command: DeclarationId,
    linked_functions: Vec<LinkedScalarFunction>,
) -> Result<ResolvedProgram, Diagnostic> {
    link_useful_data_workspace_profile(
        module,
        entrypoint,
        linked_functions,
        WorkspaceIoProfile::NetworkCommand { command },
    )
}

/// Retain the Project-v12 entry module before the selected command is added
/// as an authenticated public root. No command identity is inferred here.
pub(crate) fn link_network_entry_workspace(
    module: String,
    entrypoint: DeclarationId,
    linked_functions: Vec<LinkedScalarFunction>,
) -> Result<ResolvedProgram, Diagnostic> {
    link_useful_data_workspace_profile(
        module,
        entrypoint,
        linked_functions,
        WorkspaceIoProfile::NetworkEntry,
    )
}

/// Assemble the Project-v13 HTTPS command closure. Network authority remains
/// an invocation-owned provider capability rather than a linker grant.
pub(crate) fn link_https_command_io_workspace(
    module: String,
    entrypoint: DeclarationId,
    command: DeclarationId,
    linked_functions: Vec<LinkedScalarFunction>,
) -> Result<ResolvedProgram, Diagnostic> {
    link_useful_data_workspace_profile(
        module,
        entrypoint,
        linked_functions,
        WorkspaceIoProfile::HttpsCommand { command },
    )
}

pub(crate) fn link_https_entry_workspace(
    module: String,
    entrypoint: DeclarationId,
    linked_functions: Vec<LinkedScalarFunction>,
) -> Result<ResolvedProgram, Diagnostic> {
    link_useful_data_workspace_profile(
        module,
        entrypoint,
        linked_functions,
        WorkspaceIoProfile::HttpsEntry,
    )
}

#[derive(Clone, Eq, PartialEq)]
enum WorkspaceIoProfile {
    Pure,
    Stdout,
    LanguageCommand { command: DeclarationId },
    StdinStreamCommand(DeclarationId, bool),
    StdinStreamTextCommand(DeclarationId),
    StdinStreamTextEntry,
    StdinStreamDataCommand(DeclarationId),
    StdinStreamDataEntry,
    LineCommand { command: DeclarationId },
    NetworkCommand { command: DeclarationId },
    NetworkEntry,
    HttpsCommand { command: DeclarationId },
    HttpsEntry,
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
            WorkspaceIoProfile::Pure
            | WorkspaceIoProfile::StdinStreamTextEntry
            | WorkspaceIoProfile::StdinStreamDataEntry => function.effects.is_empty(),
            WorkspaceIoProfile::Stdout => {
                function.effects.is_empty()
                    || function.effects == [crate::host_io_ops::STDOUT_WRITE_EFFECT]
            }
            WorkspaceIoProfile::LanguageCommand { .. }
            | WorkspaceIoProfile::StdinStreamCommand(..)
            | WorkspaceIoProfile::StdinStreamTextCommand(_)
            | WorkspaceIoProfile::StdinStreamDataCommand(_) => {
                function.effects.iter().all(|effect| {
                    matches!(
                        effect.as_str(),
                        crate::command_io_ops::ARGS_READ_EFFECT
                            | crate::command_io_ops::STDIN_READ_EFFECT
                            | crate::command_io_ops::STDERR_WRITE_EFFECT
                            | crate::host_io_ops::STDOUT_WRITE_EFFECT
                    )
                })
            }
            WorkspaceIoProfile::LineCommand { .. } => function.effects.iter().all(|effect| {
                matches!(
                    effect.as_str(),
                    crate::command_io_ops::ARGS_READ_EFFECT
                        | crate::command_io_ops::STDIN_READ_EFFECT
                        | crate::command_io_ops::STDERR_WRITE_EFFECT
                        | crate::command_io_ops::STDOUT_WRITE_EFFECT
                )
            }),
            WorkspaceIoProfile::NetworkCommand { .. } => function.effects.iter().all(|effect| {
                crate::project::PROJECT_NETWORK_COMMAND_CAPABILITIES_V1.contains(&effect.as_str())
            }),
            WorkspaceIoProfile::NetworkEntry => function.effects.iter().all(|effect| {
                crate::project::PROJECT_NETWORK_COMMAND_CAPABILITIES_V1.contains(&effect.as_str())
            }),
            WorkspaceIoProfile::HttpsCommand { .. } | WorkspaceIoProfile::HttpsEntry => {
                function.effects.iter().all(|effect| {
                    crate::project::PROJECT_HTTPS_COMMAND_CAPABILITIES_V1.contains(&effect.as_str())
                })
            }
        };
        if !effects_admitted || !profile.signature_admitted(function, &entrypoint) {
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
    stdin_stream::validate_selected_command(&profile, &linked_functions)?;

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
    let (mut declarations, compiler_types) = if matches!(
        profile,
        WorkspaceIoProfile::StdinStreamDataCommand(_) | WorkspaceIoProfile::StdinStreamDataEntry
    ) {
        workspace_compiler_prelude_for_stream_data()?
    } else if profile.is_stream() {
        compiler_prelude::workspace_compiler_prelude_for_stream()?
    } else {
        workspace_compiler_prelude()?
    };
    // This profile retains no authored type declaration. A retained function
    // that still mentions one -- a bundled dependency member reached from the
    // established v1 inventory, say -- is refused here by name, rather than
    // linked against an incomplete declaration set during capacity analysis.
    for function in &functions {
        if let Some(missing) = super::authored_nominal_declarations(function)
            .into_iter()
            .next()
        {
            return Err(profile_diagnostics::uses_authored_type(function, &missing));
        }
    }
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
            WorkspaceIoProfile::Pure
            | WorkspaceIoProfile::StdinStreamTextEntry
            | WorkspaceIoProfile::StdinStreamDataEntry => Vec::new(),
            WorkspaceIoProfile::Stdout => vec![crate::host_io_ops::STDOUT_WRITE_EFFECT.to_owned()],
            WorkspaceIoProfile::LanguageCommand { .. }
            | WorkspaceIoProfile::StdinStreamCommand(..)
            | WorkspaceIoProfile::StdinStreamTextCommand(_)
            | WorkspaceIoProfile::StdinStreamDataCommand(_) => vec![
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
            WorkspaceIoProfile::NetworkCommand { .. } => {
                crate::project::PROJECT_NETWORK_COMMAND_CAPABILITIES_V1
                    .iter()
                    .map(|effect| (*effect).to_owned())
                    .collect()
            }
            WorkspaceIoProfile::NetworkEntry => {
                crate::project::PROJECT_NETWORK_COMMAND_CAPABILITIES_V1
                    .iter()
                    .map(|effect| (*effect).to_owned())
                    .collect()
            }
            WorkspaceIoProfile::HttpsCommand { .. } | WorkspaceIoProfile::HttpsEntry => {
                crate::project::PROJECT_HTTPS_COMMAND_CAPABILITIES_V1
                    .iter()
                    .map(|effect| (*effect).to_owned())
                    .collect()
            }
        },
        agents: Vec::new(),
        entrypoint,
        declarations,
        types: compiler_types,
        interfaces: Vec::new(),
        function_templates: Vec::new(),
        functions,
        function_instances: Vec::new(),
    };
    match &profile {
        WorkspaceIoProfile::StdinStreamCommand(command, _)
        | WorkspaceIoProfile::StdinStreamTextCommand(command)
        | WorkspaceIoProfile::StdinStreamDataCommand(command) => {
            crate::command_io_ops::validate_operation_profile(
                &linked,
                command,
                crate::command_io_ops::CommandOperationProfile::StdinStreamV1,
            )?;
        }
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
        WorkspaceIoProfile::NetworkCommand { command } => {
            crate::command_io_ops::validate_operation_profile(
                &linked,
                command,
                crate::command_io_ops::CommandOperationProfile::NetworkV1,
            )?;
        }
        WorkspaceIoProfile::HttpsCommand { command } => {
            crate::command_io_ops::validate_operation_profile(
                &linked,
                command,
                crate::command_io_ops::CommandOperationProfile::HttpV1,
            )?;
        }
        WorkspaceIoProfile::Pure
        | WorkspaceIoProfile::StdinStreamTextEntry
        | WorkspaceIoProfile::StdinStreamDataEntry
        | WorkspaceIoProfile::Stdout
        | WorkspaceIoProfile::NetworkEntry
        | WorkspaceIoProfile::HttpsEntry => {}
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
    let instance_loan_plans = program
        .function_instances
        .iter()
        .map(|instance| crate::loan_plan::build_plan(program, &instance.function))
        .collect::<Result<Vec<_>, _>>()?;
    for (instance, loan_plan) in program
        .function_instances
        .iter_mut()
        .zip(instance_loan_plans)
    {
        instance.function.loan_plan = loan_plan;
    }
    let inventories = program
        .functions
        .iter()
        .map(|function| crate::cleanup::build_inventory(program, function))
        .collect::<Result<Vec<_>, _>>()?;
    for (function, inventory) in program.functions.iter_mut().zip(inventories) {
        function.cleanup = inventory;
    }
    let instance_inventories = program
        .function_instances
        .iter()
        .map(|instance| crate::cleanup::build_inventory(program, &instance.function))
        .collect::<Result<Vec<_>, _>>()?;
    for (instance, inventory) in program
        .function_instances
        .iter_mut()
        .zip(instance_inventories)
    {
        instance.function.cleanup = inventory;
    }
    let cleanup_plans = program
        .functions
        .iter()
        .map(|function| crate::cleanup_plan::build_plan(program, function))
        .collect::<Result<Vec<_>, _>>()?;
    for (function, cleanup_plan) in program.functions.iter_mut().zip(cleanup_plans) {
        function.cleanup_plan = cleanup_plan;
    }
    let instance_cleanup_plans = program
        .function_instances
        .iter()
        .map(|instance| crate::cleanup_plan::build_plan(program, &instance.function))
        .collect::<Result<Vec<_>, _>>()?;
    for (instance, cleanup_plan) in program
        .function_instances
        .iter_mut()
        .zip(instance_cleanup_plans)
    {
        instance.function.cleanup_plan = cleanup_plan;
    }
    for function in &program.functions {
        if let Some(yields) = &function.yields {
            crate::hir::yield_aggregate::check_bytes_request_site_count(
                &program.declarations,
                function,
            )?;
            if crate::cleanup_plan::owned_frame_parameter(&program.declarations, &function.params) {
                crate::cleanup_plan::owned_frame_liveness(&program.declarations, function)?;
            } else if crate::cleanup_plan::owned_frame_v2_parameter(
                &program.declarations,
                &function.params,
            ) {
                crate::cleanup_plan::owned_frame_v2_liveness(&program.declarations, function)?;
            } else {
                crate::cleanup_plan::admit_owned_bytes_profile(function)?;
            }
            if crate::hir::yield_aggregate::has_bytes_leaf(
                &program.declarations,
                &yields.request_type,
            ) || crate::hir::yield_aggregate::has_bytes_leaf(
                &program.declarations,
                &yields.response_type,
            ) {
                crate::cleanup_plan::admit_sequential_aggregate_bytes_profile(function)?;
            }
        }
    }
    Ok(())
}

fn resolved_function_uses_iterator(function: &ResolvedFunction) -> bool {
    crate::iterator_ops::resolved_type_uses_iterator(&function.return_type)
        || function
            .params
            .iter()
            .any(|param| crate::iterator_ops::resolved_type_uses_iterator(&param.ty))
        || function
            .requires
            .iter()
            .chain(function.ensures.iter())
            .chain(std::iter::once(&function.body))
            .any(crate::iterator_ops::resolved_expression_uses_iterator)
}

fn resolved_function_uses_list(function: &ResolvedFunction) -> bool {
    crate::list_ops::is_list(&function.return_type)
        || crate::list_ops::is_step(&function.return_type)
        || function
            .params
            .iter()
            .any(|param| crate::list_ops::is_list(&param.ty) || crate::list_ops::is_step(&param.ty))
        || function
            .requires
            .iter()
            .chain(function.ensures.iter())
            .chain(std::iter::once(&function.body))
            .any(|expression| {
                let mut found = false;
                visit_resolved_calls(expression, &mut |callee, _, _| {
                    found |= crate::list_ops::by_id(callee.as_str()).is_some();
                });
                found
            })
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use crate::workspace_graph::{WorkspaceSource, build_owned};

    mod profile_tests;

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

    // Issue #101: `useful-text-consumer.v1` admitted borrowed `str` bodies but
    // never registered the compiler-owned `core.option` prelude declaration
    // the linker gives `useful-data.v1`, so any `match byte_get(...)` inside a
    // Project-linked text package failed closed with SPX-H006 even though the
    // identical body compiled standalone and inside `useful-data.v1`. These
    // regressions pin the internal repair in `link_useful_text_workspace`
    // (which now shares `workspace_compiler_prelude()` with the Useful Data
    // linker) without touching the profile's public parameter/return
    // vocabulary validated in `workspace_graph.rs`.

    fn useful_text_consumer_workspace() -> WorkspaceSource {
        source(
            "src/text.spx",
            r#"
module text.check;

@id("text.check.first_byte_is_space")
fn first_byte_is_space(value: borrow str) -> bool
{
    let view = str_as_bytes(value);
    match byte_get(view, 0usize) { Option::Some { value: byte } => byte == 32u8, Option::None {} => false, }
}

@id("text.check.count_leading_spaces")
fn count_leading_spaces(value: borrow str) -> i64
{
    let bytes = str_as_bytes(value);
    let length = byte_len(bytes);
    let mut index = 0usize;
    let mut progress = 0;
    let mut scanning = index < length;
    while scanning {
        let blank = match byte_get(bytes, index) { Option::Some { value: byte } => byte == 32u8, Option::None {} => false, };
        index = if blank { index + 1usize } else { index };
        progress = if blank { progress + 1 } else { progress };
        scanning = blank && index < length;
        scanning
    }
    progress
}

@id("text.check.main")
fn main() -> i64
{
    let sample = " x";
    let trimmed = "  x";
    if first_byte_is_space(string_as_str(sample)) && count_leading_spaces(string_as_str(trimmed)) == 2 { 0 } else { 1 }
}
"#,
        )
    }

    #[test]
    fn useful_text_consumer_closure_resolves_core_option_for_byte_get_match() {
        let linked = build_owned(vec![useful_text_consumer_workspace(), test_module()])
            .expect("workspace graph must build")
            .linked_scalar_program_with_roots(
                "text.check",
                &[],
                crate::project::ProjectProfile::UsefulTextConsumerV1,
                false,
            )
            .expect(
                "the Useful Text Consumer linker must resolve core.option for a \
                 match byte_get(...) body exactly as the Useful Data linker does",
            );
        assert!(
            linked
                .types
                .iter()
                .any(|declaration| declaration.id.as_str() == crate::prelude::OPTION_ID),
            "the linked program must retain the canonical core.option type declaration"
        );
        assert_eq!(
            linked
                .declarations
                .declaration(&crate::hir::DeclarationId::new(crate::prelude::OPTION_ID))
                .map(|declaration| declaration.identity_origin),
            Some(crate::hir::IdentityOrigin::CompilerOwned),
            "core.option must be retained with its compiler-owned identity, not a profile-local nominal type"
        );
        crate::hir::validate(&linked)
            .expect("the retained program must pass independent HIR validation");

        for (value, expect_leading_space, expect_leading_count) in [
            ("", false, 0i64),
            (" x", true, 1),
            ("x y", false, 0),
            ("   z", true, 3),
        ] {
            let first_byte = crate::interpreter::evaluate_resolved_public_api(
                &linked,
                "text.check.first_byte_is_space",
                &[crate::interpreter::PublicApiArgument::BorrowStr(value)],
                1_000,
            )
            .unwrap_or_else(|error| {
                panic!("first_byte_is_space({value:?}) must evaluate: {error:?}")
            });
            assert_eq!(
                first_byte.outcome,
                crate::interpreter::PublicApiEvaluationOutcome::Returned(
                    crate::interpreter::PublicApiValue::Bool(expect_leading_space)
                ),
                "first_byte_is_space({value:?}) selected the wrong Option arm"
            );

            let leading_spaces = crate::interpreter::evaluate_resolved_public_api(
                &linked,
                "text.check.count_leading_spaces",
                &[crate::interpreter::PublicApiArgument::BorrowStr(value)],
                1_000,
            )
            .unwrap_or_else(|error| {
                panic!("count_leading_spaces({value:?}) must evaluate: {error:?}")
            });
            assert_eq!(
                leading_spaces.outcome,
                crate::interpreter::PublicApiEvaluationOutcome::Returned(
                    crate::interpreter::PublicApiValue::I64(expect_leading_count)
                ),
                "count_leading_spaces({value:?}) (while-loop byte_get match) returned the wrong count"
            );
        }
    }

    /// The profile's public boundary is a separate concern from the internal
    /// `core.option` closure this module owns, and this repair must not
    /// widen it. `link_useful_text_workspace`'s own admission check (the
    /// `!matches!((&parameter.ty, parameter.ownership), ...)` guard a few
    /// lines above, which this diff does not touch) still rejects a function
    /// whose own signature names `u8`; the higher `workspace_graph.rs`
    /// project-validation pass enforces the same boundary again with its own
    /// `SPX-G174` diagnostic (`project function ... has a signature outside
    /// the selected profile`), which this diff also never edits.
    #[test]
    fn useful_text_consumer_public_signature_boundary_is_unchanged() {
        let source_text = source(
            "src/text_public_u8.spx",
            r#"
module text.check.public_boundary;

@id("text.check.public_boundary.is_space")
fn is_space(byte: u8) -> bool
{
    byte == 32u8
}

@id("text.check.public_boundary.main")
fn main() -> i64
{
    if is_space(32u8) { 0 } else { 1 }
}
"#,
        );
        let error = build_owned(vec![source_text, test_module()])
            .expect("workspace graph must build")
            .linked_scalar_program_with_roots(
                "text.check.public_boundary",
                &[],
                crate::project::ProjectProfile::UsefulTextConsumerV1,
                false,
            )
            .expect_err(
                "an own-signature u8 parameter must still be rejected: this repair only \
                 resolves core.option inside admitted bodies, it does not widen the profile's \
                 public i64/bool/borrow-str parameter and return vocabulary",
            );
        assert_eq!(error[0].code, "SPX-H006");
        assert_eq!(
            error[0].message,
            "workspace function `text.check.public_boundary.is_space` is outside the Useful Text Consumer linker profile"
        );
    }
}
