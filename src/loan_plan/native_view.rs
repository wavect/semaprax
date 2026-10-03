//! Provenance of exact native receiver-tied views through borrowed Result arms.
use super::*;

pub(super) fn inventory_aliases(
    program: &ResolvedProgram,
    expression: &ResolvedExpr,
    aliases: &mut BTreeMap<ValueId, Place>,
) {
    let ResolvedExprKind::Match {
        mode: crate::hir::ResolvedMatchMode::Borrow,
        scrutinee,
        arms,
    } = &expression.kind
    else {
        return;
    };
    let Some(origin) = expression_place(scrutinee) else {
        return;
    };
    for arm in arms {
        let ResolvedMatchPattern::Variant { case, fields, .. } = &arm.pattern else {
            continue;
        };
        for field in fields {
            let ResolvedType::Nominal {
                declaration,
                arguments,
            } = &field.binding.ty
            else {
                continue;
            };
            if field.binding.ownership != OwnershipMode::Borrow || !arguments.is_empty()
                || !program.interfaces.iter().flat_map(|interface| &interface.imports).any(|import|
                    matches!(&import.result.kind, crate::hir::ResolvedImportResultKind::BorrowedStr { resource } if resource == declaration)) { continue; }
            let mut place = origin.clone();
            place.projections.push(PlaceProjection::VariantField {
                case: case.clone(),
                field: field.field.clone(),
            });
            aliases.insert(field.binding.id.clone(), place);
        }
    }
}

pub(super) fn match_parents(
    expressions: &[&ResolvedExpr],
    drafts: &[CfgDraft],
) -> BTreeMap<ValueId, LoanId> {
    let sites = drafts
        .iter()
        .enumerate()
        .map(|(index, draft)| {
            (
                (draft.site.clone(), draft.cause.clone()),
                LoanId(index as u16),
            )
        })
        .collect::<BTreeMap<_, _>>();
    let mut parents = BTreeMap::new();
    for expression in expressions {
        let ResolvedExprKind::Match {
            mode: crate::hir::ResolvedMatchMode::Borrow,
            arms,
            ..
        } = &expression.kind
        else {
            continue;
        };
        for (arm_index, arm) in arms.iter().enumerate() {
            let ResolvedMatchPattern::Variant { fields, .. } = &arm.pattern else {
                continue;
            };
            let Some(id) = sites.get(&(
                expression.id.clone(),
                LoanCause::MatchBorrow {
                    arm: arm_index as u16,
                },
            )) else {
                continue;
            };
            for field in fields {
                if field.binding.ownership == OwnershipMode::Borrow {
                    parents.insert(field.binding.id.clone(), *id);
                }
            }
        }
    }
    parents
}
