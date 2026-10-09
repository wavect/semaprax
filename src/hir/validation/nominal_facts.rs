//! Validation-local borrowed facts for exact zero-argument nominal types.
//!
//! The immutable Program retains the identities and facts. Only sorted reference
//! metadata is owned here; validator clones share that metadata. Other types and
//! absent retained entries keep the original owning/recomputation path.
use super::*;
use std::borrow::Cow;
use std::cell::OnceCell;
use std::fmt::Write as _;
use std::sync::Arc;

type Entry<'a> = (&'a DeclarationId, &'a TypeFacts);
type SharedEntries<'a> = Arc<Vec<Entry<'a>>>;

// Arc<Vec<_>> has the same audited two-counter/three-word carrier layout as
// Arc<String>. The separate Vec backing is charged below, including excess.
const _: () = assert!(std::mem::size_of::<Vec<Entry<'static>>>() == std::mem::size_of::<String>());
const _: () =
    assert!(std::mem::align_of::<Vec<Entry<'static>>>() == std::mem::align_of::<String>());
const CARRIER_BYTES: usize = ExpressionId::SHARED_ALLOCATION_CARRIER_BYTES;

#[derive(Clone, Default)]
pub(super) struct Lookup<'a> {
    entries: OnceCell<Option<SharedEntries<'a>>>,
}

fn refused() -> Diagnostic {
    hir_error("nominal type-facts lookup metadata exceeds the active validation budget")
}

fn reserve(bytes: usize) -> Result<(), Diagnostic> {
    crate::bounded_output::reserve_active_required(bytes)
        .then_some(())
        .ok_or_else(refused)
}

fn eligible(program: &ResolvedProgram, declaration: &ResolvedTypeDeclaration) -> bool {
    declaration.type_parameters.is_empty()
        && program
            .declarations
            .type_parameters(&declaration.id)
            .is_some_and(|parameters| parameters.is_empty())
}

fn nominal_key(identity: &DeclarationId) -> Result<String, Diagnostic> {
    let length = identity.as_str().len();
    let mut remaining = length;
    let mut digits = 1usize;
    while remaining >= 10 {
        remaining /= 10;
        digits += 1;
    }
    // Exact identity_key spelling: nominal:<byte-length>:<identity>:0:.
    let bytes = length
        .checked_add(digits)
        .and_then(|bytes| bytes.checked_add(11))
        .ok_or_else(refused)?;
    reserve(bytes)?;
    let mut key = String::with_capacity(bytes);
    reserve(key.capacity() - bytes)?;
    write!(key, "nominal:{length}:{}:0:", identity.as_str()).map_err(|_| refused())?;
    Ok(key)
}

impl<'a> Lookup<'a> {
    fn prepare(program: &'a ResolvedProgram) -> Result<Option<SharedEntries<'a>>, Diagnostic> {
        let count = program
            .types
            .iter()
            .filter(|declaration| eligible(program, declaration))
            .count();
        if count == 0 {
            return Ok(None);
        }
        let bytes = count
            .checked_mul(std::mem::size_of::<Entry<'a>>())
            .ok_or_else(refused)?;
        reserve(bytes.checked_add(CARRIER_BYTES).ok_or_else(refused)?)?;
        let mut entries = Vec::with_capacity(count);
        let excess = entries
            .capacity()
            .checked_sub(count)
            .and_then(|slots| slots.checked_mul(std::mem::size_of::<Entry<'a>>()))
            .ok_or_else(refused)?;
        reserve(excess)?;
        for declaration in program
            .types
            .iter()
            .filter(|declaration| eligible(program, declaration))
        {
            let key = nominal_key(&declaration.id)?;
            if let Some(facts) = program.declarations.type_facts_by_id.get(&key) {
                entries.push((&declaration.id, facts));
            }
        }
        if entries.is_empty() {
            return Ok(None);
        }
        entries.sort_unstable_by(|left, right| left.0.cmp(right.0));
        Ok(Some(Arc::new(entries)))
    }

    fn get(
        &self,
        program: &'a ResolvedProgram,
        ty: &ResolvedType,
    ) -> Result<Option<&'a TypeFacts>, Diagnostic> {
        let ResolvedType::Nominal {
            declaration,
            arguments,
        } = ty
        else {
            return Ok(None);
        };
        if !arguments.is_empty() {
            return Ok(None);
        }
        if self.entries.get().is_none() {
            // Do not build unrelated metadata for an unknown or generic
            // zero-argument request. Prepared entries already prove eligibility.
            if !program
                .declarations
                .type_parameters(declaration)
                .is_some_and(|parameters| parameters.is_empty())
            {
                return Ok(None);
            }
            self.entries
                .set(Self::prepare(program)?)
                .map_err(|_| hir_error("nominal facts lookup initialized twice"))?;
        }
        let Some(entries) = self.entries.get().and_then(Option::as_ref) else {
            return Ok(None);
        };
        Ok(entries
            .binary_search_by(|entry| entry.0.cmp(declaration))
            .ok()
            .map(|index| entries[index].1))
    }

    #[cfg(test)]
    pub(super) fn owned_capacity(&self) -> usize {
        self.entries
            .get()
            .and_then(Option::as_ref)
            .map_or(0, |entries| {
                CARRIER_BYTES + entries.capacity() * std::mem::size_of::<Entry<'a>>()
            })
    }
}

impl<'a> HirValidator<'a> {
    pub(super) fn borrowed_type_facts(
        &self,
        ty: &ResolvedType,
    ) -> Result<Option<Cow<'a, TypeFacts>>, Diagnostic> {
        if let Some(facts) = self.nominal_facts.get(self.program, ty)? {
            Ok(Some(Cow::Borrowed(facts)))
        } else {
            Ok(self.program.declarations.type_facts(ty).map(Cow::Owned))
        }
    }
}

#[cfg(test)]
mod tests;
