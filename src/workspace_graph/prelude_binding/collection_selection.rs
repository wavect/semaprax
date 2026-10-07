//! Source-only collection selection, including exact imported signature types.
use super::*;

pub(super) fn module_uses(source: &Program, programs: &[Program]) -> bool {
    crate::map_ops::program_uses(source)
        || source.module_uses.iter().any(|import| {
            let Some(owner) = programs.iter().find(|p| p.module == import.target_module) else {
                return false;
            };
            match import.kind {
                ModuleUseKind::Function => owner
                    .functions
                    .iter()
                    .filter(|f| f.stable_id == import.persistent_id)
                    .any(|f| {
                        contains(owner, &f.return_type, programs)
                            || f.params.iter().any(|p| contains(owner, &p.ty, programs))
                    }),
                ModuleUseKind::Type => owner
                    .types
                    .iter()
                    .filter(|d| d.stable_id == import.persistent_id)
                    .any(|d| fields_contain(owner, d, programs)),
                ModuleUseKind::Protocol => false,
            }
        })
}

fn fields_contain(owner: &Program, declaration: &TypeDeclaration, programs: &[Program]) -> bool {
    match &declaration.kind {
        TypeDeclarationKind::Record { fields } | TypeDeclarationKind::Class { fields, .. } => {
            fields.iter().any(|f| contains(owner, &f.ty, programs))
        }
        _ => false,
    }
}

fn contains<'a>(owner: &'a Program, root: &'a Type, programs: &'a [Program]) -> bool {
    let mut pending = vec![(owner, root)];
    let mut visited = BTreeSet::new();
    let mut fields = 0usize;
    while let Some((owner, ty)) = pending.pop() {
        if crate::map_ops::ast_collection(ty) {
            return true;
        }
        let Type::Named { name, arguments } = ty else {
            continue;
        };
        if !arguments.is_empty() || !visited.insert((owner.module.as_str(), name.as_str())) {
            continue;
        }
        let target = owner
            .types
            .iter()
            .find(|d| d.name == *name)
            .map(|d| (owner, d))
            .or_else(|| {
                owner
                    .module_uses
                    .iter()
                    .find(|i| i.kind == ModuleUseKind::Type && i.alias == *name)
                    .and_then(|i| {
                        programs
                            .iter()
                            .find(|p| p.module == i.target_module)
                            .and_then(|p| {
                                p.types
                                    .iter()
                                    .find(|d| d.stable_id == i.persistent_id)
                                    .map(|d| (p, d))
                            })
                    })
            });
        let Some((owner, declaration)) = target else {
            continue;
        };
        let TypeDeclarationKind::Record { fields: declared } = &declaration.kind else {
            continue;
        };
        fields = fields.saturating_add(declared.len());
        if fields > crate::cleanup::MAX_CLEANUP_VISITED_FIELDS {
            return false;
        }
        pending.extend(declared.iter().map(|f| (owner, &f.ty)));
    }
    false
}
