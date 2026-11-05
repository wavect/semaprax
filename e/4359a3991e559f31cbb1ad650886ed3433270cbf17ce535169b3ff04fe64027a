//! Funded lifecycle-effect collection for the current function chain only.
//! `extend_resolved_lifecycle_effects` in inspection.rs remains the complete original.
use super::*;

fn fund(bytes: Option<usize>) -> Result<(), Diagnostic> {
    #[cfg(test)]
    let _operation = crate::bounded_output::stage_debits::operation_guard(
        crate::bounded_output::stage_debits::Operation::Lifecycle,
    );
    match bytes {
        Some(bytes) if crate::bounded_output::reserve_active_required(bytes) => Ok(()),
        Some(_) => Err(budget()),
        None => {
            crate::bounded_output::refuse_active_required(usize::MAX);
            Err(budget())
        }
    }
}
fn budget() -> Diagnostic {
    crate::hir::hir_error_static("lifecycle collection exceeds the active validation budget")
}
// A node is paid at the tree upper bound before the insertion that may allocate it.
fn node<K, V>() -> Result<(), Diagnostic> {
    fund(crate::cleanup_plan::replay_btree_node_upper::<K, V>())
}
/// Borrowed effect keys; each genuinely new key pays its node before insertion.
pub(super) fn declared_effects(effects: &[String]) -> Result<BTreeSet<&str>, Diagnostic> {
    fund(Some(0))?;
    let mut keys = BTreeSet::new();
    for effect in effects {
        if !keys.contains(effect.as_str()) {
            node::<&str, ()>()?;
            keys.insert(effect.as_str());
        }
    }
    Ok(keys)
}
#[cfg(test)]
pub(super) fn extend_funded(
    program: &ResolvedProgram,
    ty: &ResolvedType,
    effects: &mut BTreeSet<String>,
) -> Result<(), Diagnostic> {
    fund(Some(0))?;
    collect(program, ty, &mut BTreeSet::new(), effects)
}
#[cfg(test)]
fn collect<'a>(
    program: &'a ResolvedProgram,
    ty: &'a ResolvedType,
    visiting: &mut BTreeSet<&'a DeclarationId>,
    effects: &mut BTreeSet<String>,
) -> Result<(), Diagnostic> {
    let Some(id) = ty.nominal_id() else {
        return Ok(());
    };
    if visiting.contains(&id) {
        return Ok(());
    }
    node::<&DeclarationId, ()>()?;
    visiting.insert(id);
    let declaration = program
        .types
        .iter()
        .find(|item| item.id == *id)
        .ok_or_else(|| hir_error(format!("type `{id}` has no lifecycle declaration")))?;
    match &declaration.kind {
        ResolvedTypeDeclarationKind::Resource { drop } => {
            if let ResolvedResourceDropKind::Imported { import, .. } = &drop.kind {
                let resolved = program
                    .interfaces
                    .iter()
                    .flat_map(|interface| &interface.imports)
                    .find(|item| item.id == *import)
                    .ok_or_else(|| {
                        hir_error(format!(
                            "resource `{id}` references missing import `{import}`"
                        ))
                    })?;
                for effect in &resolved.effects {
                    if !effects.contains(effect.as_str()) {
                        node::<String, ()>()?;
                        fund(Some(effect.len()))?;
                        effects.insert(effect.clone());
                    }
                }
            }
        }
        ResolvedTypeDeclarationKind::Record { fields }
        | ResolvedTypeDeclarationKind::Class { fields, .. } => {
            for field in fields {
                collect(program, &field.ty, visiting, effects)?;
            }
        }
        ResolvedTypeDeclarationKind::Variant { cases } => {
            for case in cases {
                for field in &case.fields {
                    collect(program, &field.ty, visiting, effects)?;
                }
            }
        }
    }
    visiting.remove(&id);
    Ok(())
}

pub(super) mod completed;
