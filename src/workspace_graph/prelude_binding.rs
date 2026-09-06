//! Selects the exact shared prelude declaration inventory retained by a workspace.

use super::*;

pub(super) fn uses_vec(programs: &[Program]) -> bool {
    programs.iter().any(prelude::program_uses_vec)
}

pub(super) fn ids(programs: &[Program]) -> BTreeSet<&'static str> {
    if uses_vec(programs) {
        prelude::all_type_ids_v2().into_iter().collect()
    } else {
        prelude::all_ids_v1().into_iter().collect()
    }
}

pub(super) fn expected_declaration_facts(
    include_vec: bool,
) -> Result<BTreeMap<String, WorkspaceDeclarationFact>, Vec<Diagnostic>> {
    let mut facts = BTreeMap::new();
    for declaration in prelude::declarations()
        .iter()
        .filter(|declaration| include_vec || declaration.stable_id != prelude::VEC_ID)
    {
        let kind = match &declaration.kind {
            TypeDeclarationKind::Record { .. } => hir::DeclarationKind::Record,
            TypeDeclarationKind::Class { .. } => hir::DeclarationKind::Class,
            TypeDeclarationKind::Variant { .. } => hir::DeclarationKind::Variant,
            TypeDeclarationKind::Resource { .. } => {
                return Err(vec![graph_error(
                    "SPX-G173",
                    "compiler prelude unexpectedly declares a resource authority",
                )]);
            }
        };
        insert_expected_compiler_declaration(&mut facts, &declaration.stable_id, kind, None)?;
        match &declaration.kind {
            TypeDeclarationKind::Record { fields } | TypeDeclarationKind::Class { fields, .. } => {
                for field in fields {
                    insert_expected_compiler_declaration(
                        &mut facts,
                        &field.stable_id,
                        hir::DeclarationKind::Field,
                        Some(&declaration.stable_id),
                    )?;
                }
            }
            TypeDeclarationKind::Variant { cases } => {
                for case in cases {
                    insert_expected_compiler_declaration(
                        &mut facts,
                        &case.stable_id,
                        hir::DeclarationKind::VariantCase,
                        Some(&declaration.stable_id),
                    )?;
                    for field in &case.fields {
                        insert_expected_compiler_declaration(
                            &mut facts,
                            &field.stable_id,
                            hir::DeclarationKind::CaseField,
                            Some(&case.stable_id),
                        )?;
                    }
                }
            }
            TypeDeclarationKind::Resource { .. } => unreachable!("resource rejected above"),
        }
    }
    Ok(facts)
}
