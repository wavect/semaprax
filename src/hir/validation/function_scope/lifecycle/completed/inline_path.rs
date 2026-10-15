//! Borrowed inline active ancestors; larger paths retain the original paid tree.
use super::*;
const INLINE: usize = 16;
struct Active<'a> {
    inline: [Option<&'a DeclarationId>; INLINE],
    spill: BTreeSet<&'a DeclarationId>,
}
impl<'a> Active<'a> {
    fn new() -> Result<Self, Diagnostic> {
        // Pay the entire simultaneous inline/header carrier, without subtracting
        // the prior tree header or treating a future spill as already funded.
        fund(Some(std::mem::size_of::<Self>()))?;
        Ok(Self {
            inline: [None; INLINE],
            spill: BTreeSet::new(),
        })
    }
    fn contains(&self, id: &DeclarationId) -> bool {
        self.inline.iter().flatten().any(|known| *known == id) || self.spill.contains(&id)
    }
    fn insert(&mut self, id: &'a DeclarationId) -> Result<(), Diagnostic> {
        fund(Some(0))?;
        if let Some(slot) = self.inline.iter_mut().find(|slot| slot.is_none()) {
            *slot = Some(id);
        } else {
            // The exact old acquisition remains before every spill insertion.
            node::<&DeclarationId, ()>()?;
            self.spill.insert(id);
        }
        Ok(())
    }
    fn remove(&mut self, id: &'a DeclarationId) {
        if let Some(slot) = self
            .inline
            .iter_mut()
            .find(|slot| slot.is_some_and(|known| known == id))
        {
            *slot = None;
        } else {
            self.spill.remove(&id);
        }
    }
}
pub(super) fn extend(
    program: &ResolvedProgram,
    ty: &ResolvedType,
    effects: &mut BTreeSet<String>,
) -> Result<(), Diagnostic> {
    fund(Some(0))?;
    // The complete old collector also returns immediately for nonnominal roots.
    // No active-path carrier is constructed on that genuinely unused route.
    if ty.nominal_id().is_none() {
        return Ok(());
    }
    collect(program, ty, &mut Active::new()?, effects)
}
fn collect<'a>(
    program: &'a ResolvedProgram,
    ty: &'a ResolvedType,
    visiting: &mut Active<'a>,
    effects: &mut BTreeSet<String>,
) -> Result<(), Diagnostic> {
    fund(Some(0))?;
    let Some(id) = ty.nominal_id() else {
        return Ok(());
    };
    if visiting.contains(id) {
        return Ok(());
    }
    visiting.insert(id)?;
    let declaration = program
        .types
        .iter()
        .find(|item| item.id == *id)
        .ok_or_else(|| hir_error(format!("type `{id}` has no lifecycle declaration")))?;
    match &declaration.kind {
        ResolvedTypeDeclarationKind::Resource { drop } => {
            resource_effects(program, id, &drop.kind, effects)?;
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
    visiting.remove(id);
    Ok(())
}

// Keep resource lookup/diagnostic temporaries off every recursive record frame.
// Deep nominal chains never execute this arm until reaching their leaf.
#[inline(never)]
fn resource_effects(
    program: &ResolvedProgram,
    id: &DeclarationId,
    drop: &ResolvedResourceDropKind,
    effects: &mut BTreeSet<String>,
) -> Result<(), Diagnostic> {
    if let ResolvedResourceDropKind::Imported { import, .. } = drop {
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
    Ok(())
}

#[cfg(test)]
mod tests;
