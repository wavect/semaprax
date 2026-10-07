//! The compiler-owned prelude the workspace linker links every retained
//! function against.
//!
//! A workspace linker profile retains no authored type declaration. The
//! declaration index and type table it does hold come from here, built from
//! the compiler's own prelude rather than from any project source, so a
//! retained function's `match` and capacity validation still has the nominal
//! types the compiler itself owns.

use super::link_error;
use crate::hir::*;

pub(super) fn selected_for_scalar(
    functions: &[ResolvedFunction],
    parts: Option<&super::LinkedScalarProjectParts>,
    uses_owned_result: bool,
) -> Result<(DeclarationIndex, Vec<ResolvedTypeDeclaration>), Diagnostic> {
    let instances = || {
        parts
            .into_iter()
            .flat_map(|parts| &parts.function_instances)
            .map(|instance| &instance.function)
    };
    let uses_vec = functions.iter().chain(instances()).any(|function| {
        std::iter::once(&function.body)
            .chain(function.requires.iter())
            .chain(function.ensures.iter())
            .any(|expression| {
                let mut found = false;
                visit_resolved_calls(expression, &mut |callee, instance, arguments| {
                    found |= instance.is_none()
                        && arguments.len() == 1
                        && crate::vec_ops::by_id(callee.as_str()).is_some();
                });
                found
            })
    });
    let uses_box = functions
        .iter()
        .chain(instances())
        .any(super::resolved_function_uses_box);
    let uses_iterator = functions
        .iter()
        .chain(instances())
        .any(super::resolved_function_uses_iterator);
    let uses_list = functions
        .iter()
        .chain(instances())
        .any(super::resolved_function_uses_list);
    let uses_record_iterator = functions
        .iter()
        .chain(instances())
        .any(crate::iterator_ops::function_uses_record_iterator)
        || parts
            .into_iter()
            .flat_map(|parts| &parts.function_templates)
            .any(crate::iterator_ops::template_uses_record_iterator);
    if uses_vec
        || uses_box
        || uses_iterator
        || uses_list
        || uses_record_iterator
        || uses_owned_result
    {
        workspace_compiler_prelude_for(
            uses_vec,
            uses_box,
            uses_iterator,
            uses_list,
            uses_record_iterator,
        )
    } else {
        Ok((DeclarationIndex::default(), Vec::new()))
    }
}

pub(super) fn selected_for_owned_data(
    functions: &[ResolvedFunction],
    parts: &LinkedOwnedDataParts,
) -> Result<(DeclarationIndex, Vec<ResolvedTypeDeclaration>), Diagnostic> {
    let instances = || {
        parts
            .function_instances
            .iter()
            .map(|instance| &instance.function)
    };
    let uses_vec = functions
        .iter()
        .chain(instances())
        .any(super::resolved_function_uses_vec);
    let uses_box = functions
        .iter()
        .chain(instances())
        .any(super::resolved_function_uses_box);
    let uses_iterator = functions
        .iter()
        .chain(instances())
        .any(super::resolved_function_uses_iterator);
    let uses_list = functions
        .iter()
        .chain(instances())
        .any(super::resolved_function_uses_list);
    let uses_record_iterator = functions
        .iter()
        .chain(instances())
        .any(crate::iterator_ops::function_uses_record_iterator)
        || parts
            .function_templates
            .iter()
            .any(crate::iterator_ops::template_uses_record_iterator);
    workspace_compiler_prelude_for(
        uses_vec,
        uses_box,
        uses_iterator,
        uses_list,
        uses_record_iterator,
    )
}

pub(super) fn workspace_linker_prelude_program() -> Program {
    Program {
        path: "<workspace-linker>".to_owned(),
        module: "compiler.prelude".to_owned(),
        module_uses: Vec::new(),
        permits: Vec::new(),
        types: Vec::new(),
        interfaces: Vec::new(),
        protocols: Vec::new(),
        implementations: Vec::new(),
        session_protocols: Vec::new(),
        agents: Vec::new(),
        functions: Vec::new(),
    }
}

pub(crate) fn compiler_prelude_declarations() -> Result<DeclarationIndex, Diagnostic> {
    compiler_prelude_declarations_for_vec(false)
}

pub(super) fn compiler_prelude_declarations_for_vec(
    include_vec: bool,
) -> Result<DeclarationIndex, Diagnostic> {
    let mut declarations = DeclarationIndex::from_verified(&workspace_linker_prelude_program())?;
    if include_vec {
        let id = DeclarationId::new(crate::prelude::VEC_ID);
        declarations.insert_top_level(
            "Vec".to_owned(),
            id.clone(),
            DeclarationKind::Record,
            IdentityOrigin::CompilerOwned,
        );
        declarations.type_parameters.insert(
            id.clone(),
            vec![ResolvedTypeParameterDeclaration {
                name: "T".to_owned(),
                index: 0,
                span: Span::default(),
            }],
        );
        declarations.record_fields.insert(id, Vec::new());
    }
    Ok(declarations)
}

pub(super) fn compiler_prelude_declarations_for(
    include_vec: bool,
    include_box: bool,
) -> Result<DeclarationIndex, Diagnostic> {
    // The Box-bearing v4 prelude is additive over the Vec-bearing predecessor.
    let mut declarations = compiler_prelude_declarations_for_vec(include_vec || include_box)?;
    if include_box {
        let id = DeclarationId::new(crate::prelude::BOX_ID);
        declarations.insert_top_level(
            "Box".to_owned(),
            id.clone(),
            DeclarationKind::Record,
            IdentityOrigin::CompilerOwned,
        );
        declarations.type_parameters.insert(
            id.clone(),
            vec![ResolvedTypeParameterDeclaration {
                name: "T".to_owned(),
                index: 0,
                span: Span::default(),
            }],
        );
        declarations.record_fields.insert(id, Vec::new());
    }
    Ok(declarations)
}

pub(super) fn workspace_compiler_prelude(
) -> Result<(DeclarationIndex, Vec<ResolvedTypeDeclaration>), Diagnostic> {
    workspace_compiler_prelude_for_vec(false)
}

pub(super) fn workspace_compiler_prelude_for_vec(
    include_vec: bool,
) -> Result<(DeclarationIndex, Vec<ResolvedTypeDeclaration>), Diagnostic> {
    workspace_compiler_prelude_for(include_vec, false, false, false, false)
}

pub(super) fn workspace_compiler_prelude_for(
    include_vec: bool,
    include_box: bool,
    include_iterator: bool,
    include_list: bool,
    include_record_iterator: bool,
) -> Result<(DeclarationIndex, Vec<ResolvedTypeDeclaration>), Diagnostic> {
    workspace_compiler_prelude_selected(
        include_vec,
        include_box,
        include_iterator,
        include_list,
        false,
        include_record_iterator,
    )
}
pub(super) fn workspace_compiler_prelude_for_stream(
) -> Result<(DeclarationIndex, Vec<ResolvedTypeDeclaration>), Diagnostic> {
    workspace_compiler_prelude_selected(false, false, false, false, true, false)
}
fn workspace_compiler_prelude_selected(
    include_vec: bool,
    include_box: bool,
    include_iterator: bool,
    include_list: bool,
    include_stream: bool,
    include_record_iterator: bool,
) -> Result<(DeclarationIndex, Vec<ResolvedTypeDeclaration>), Diagnostic> {
    let prelude_program = workspace_linker_prelude_program();
    let compiler_declarations = if include_stream || include_record_iterator {
        crate::prelude::declarations()
    } else if include_list {
        &crate::prelude::declarations()[..8]
    } else if include_iterator {
        &crate::prelude::declarations()[..6]
    } else if include_box {
        &crate::prelude::declarations()[..4]
    } else if include_vec {
        &crate::prelude::declarations()[..3]
    } else {
        crate::prelude::declarations_for_program(&prelude_program)
    };
    let declarations =
        if include_stream || include_record_iterator || include_iterator || include_list {
            DeclarationIndex::from_verified_with_prelude(&prelude_program, compiler_declarations)?
        } else {
            compiler_prelude_declarations_for(include_vec, include_box)?
        };
    let compiler_types = compiler_declarations
        .iter()
        .map(|declaration| {
            let id = DeclarationId::new(declaration.stable_id.clone());
            let kind = match &declaration.kind {
                TypeDeclarationKind::Variant { .. } => ResolvedTypeDeclarationKind::Variant {
                    cases: declarations
                        .variant_cases(&id)
                        .ok_or_else(|| link_error("workspace prelude variant cases are absent"))?
                        .to_vec(),
                },
                TypeDeclarationKind::Record { .. } => ResolvedTypeDeclarationKind::Record {
                    fields: declarations
                        .record_fields(&id)
                        .ok_or_else(|| link_error("workspace prelude record fields are absent"))?
                        .to_vec(),
                },
                _ => {
                    return Err(link_error(
                        "workspace linker prelude contains an unsupported type kind",
                    ));
                }
            };
            Ok(ResolvedTypeDeclaration {
                type_parameters: declarations
                    .type_parameters(&id)
                    .ok_or_else(|| link_error("workspace prelude type parameters are absent"))?
                    .to_vec(),
                kind,
                id,
                name: declaration.name.clone(),
                span: declaration.span,
            })
        })
        .collect::<Result<Vec<_>, Diagnostic>>()?;
    Ok((declarations, compiler_types))
}
