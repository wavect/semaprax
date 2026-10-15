//! One paid declaration predicate per immutable phase. This proves only that
//! an explicit record exists; consuming ownership checks still walk the body.
use super::{Admission, Refusal};
use crate::hir::{DeclarationIndex, DeclarationKind, IdentityOrigin};

pub(crate) struct RecordPresence<'a> {
    index: &'a DeclarationIndex,
    found: bool,
}
impl<'a> RecordPresence<'a> {
    pub(crate) fn new(
        index: &'a DeclarationIndex,
        admission: &mut Admission<'_>,
    ) -> Result<Self, Refusal> {
        admission.reserve(std::mem::size_of::<Self>())?;
        let mut found = false;
        // Exact old ordered prefix and early-exit predicate, paid once before
        // publication. The shared borrow prevents mutation of this index.
        for declaration in index.declarations() {
            admission.units(1)?;
            if declaration.kind == DeclarationKind::Record
                && declaration.identity_origin == IdentityOrigin::Explicit
            {
                found = true;
                break;
            }
        }
        Ok(Self { index, found })
    }
    pub(crate) fn get(
        &self,
        index: &DeclarationIndex,
        admission: &mut Admission<'_>,
    ) -> Result<bool, Refusal> {
        admission.reserve(0)?;
        admission.units(1)?;
        if !std::ptr::eq(self.index, index) {
            return admission.refuse(Refusal::Subject);
        }
        Ok(self.found)
    }
}

#[cfg(test)]
mod tests;
