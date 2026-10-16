//! One function's monotone required-effect set and completed exact roots.
//! Generic instances, unknown identities and inconsistent metadata remain uncached.
//! The original funded collector owns first traversal, diagnostics and effect order.
use super::*;

const INLINE: usize = 8;

pub(in crate::hir::validation::function_scope) struct Collector<'a> {
    program: &'a ResolvedProgram,
    effects: BTreeSet<String>,
    completed: [Option<&'a DeclarationId>; INLINE],
    spill: Vec<&'a DeclarationId>,
    #[cfg(test)]
    traversals: usize,
}

impl<'a> Collector<'a> {
    pub(in crate::hir::validation::function_scope) fn new(program: &'a ResolvedProgram) -> Self {
        Self {
            program,
            effects: BTreeSet::new(),
            completed: [None; INLINE],
            spill: Vec::new(),
            #[cfg(test)]
            traversals: 0,
        }
    }

    // Only the complete zero-argument instantiation has a borrowed identity key.
    // The immutable program holds it through collection and receiving Drop.
    fn key(&self, ty: &ResolvedType) -> Option<&'a DeclarationId> {
        let ResolvedType::Nominal {
            declaration,
            arguments,
        } = ty
        else {
            return None;
        };
        if !arguments.is_empty() {
            return None;
        }
        let program: &'a ResolvedProgram = self.program;
        let item = program.types.iter().find(|item| item.id == *declaration)?;
        if !item.type_parameters.is_empty()
            || !program
                .declarations
                .type_parameters(declaration)
                .is_some_and(|p| p.is_empty())
            || program
                .declarations
                .declaration(declaration)
                .is_none_or(|indexed| indexed.id != item.id)
        {
            return None;
        }
        Some(&item.id)
    }

    pub(in crate::hir::validation::function_scope) fn extend(
        &mut self,
        ty: &ResolvedType,
    ) -> Result<(), Diagnostic> {
        fund(Some(0))?;
        let key = self.key(ty);
        if key.is_some_and(|key| {
            self.completed.iter().flatten().any(|known| *known == key) || self.spill.contains(&key)
        }) {
            return Ok(());
        }
        // Errors may have extended effects partially. Never mark those roots complete.
        #[cfg(test)]
        {
            self.traversals += 1;
        }
        extend_funded(self.program, ty, &mut self.effects)?;
        if let Some(key) = key {
            if let Some(slot) = self.completed.iter_mut().find(|slot| slot.is_none()) {
                *slot = Some(key);
            } else {
                if self.spill.len() == self.spill.capacity() {
                    let Some(capacity) = self.spill.capacity().checked_mul(2).map(|n| n.max(4))
                    else {
                        return fund(None);
                    };
                    fund(capacity.checked_mul(std::mem::size_of::<&DeclarationId>()))?;
                    if self
                        .spill
                        .try_reserve_exact(capacity - self.spill.len())
                        .is_err()
                        || self.spill.capacity() != capacity
                    {
                        crate::bounded_output::refuse_active_required(usize::MAX);
                        return Err(budget());
                    }
                }
                self.spill.push(key);
            }
        }
        Ok(())
    }

    pub(in crate::hir::validation::function_scope) fn iter(&self) -> impl Iterator<Item = &String> {
        self.effects.iter()
    }
}

#[cfg(test)]
mod tests;
