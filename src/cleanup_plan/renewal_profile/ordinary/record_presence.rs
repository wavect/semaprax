//! One paid declaration predicate per immutable phase. This proves only that
//! an explicit record exists; consuming ownership checks still walk the body.
use super::{Admission, Refusal};
use crate::hir::{DeclarationIndex, DeclarationKind, IdentityOrigin};

pub(crate) struct RecordPresence<'a> {
    index: &'a DeclarationIndex,
    found: bool,
}
impl<'a> RecordPresence<'a> {
    pub(crate) fn acquisition_receiving_controls() -> usize {
        use std::mem::size_of;
        // Paid by the caller before new(): even a rejected constructor has
        // its Result and bootstrap reservation receiver, without an owner.
        size_of::<Result<Self, Refusal>>()
            + size_of::<Result<(), Refusal>>()
            + size_of::<&mut Admission<'_>>()
    }
    pub(crate) fn constructor_controls() -> usize {
        // Initializer and received owner plus the named ordered-prefix result.
        2 * std::mem::size_of::<Self>() + std::mem::size_of::<bool>()
    }
    pub(crate) fn caller_controls() -> usize {
        use std::mem::size_of;
        // The production factory's new mapped Result and its own payment
        // boundary, before constructing the borrowed acquisition arguments.
        Self::acquisition_receiving_controls()
            + size_of::<Result<Self, crate::diagnostic::Diagnostic>>()
            + size_of::<Result<(), Refusal>>()
            + size_of::<Result<(), crate::diagnostic::Diagnostic>>()
            + 2 * size_of::<&DeclarationIndex>()
            + 2 * size_of::<&mut Admission<'_>>()
            + size_of::<Refusal>()
    }
    pub(crate) fn transport_controls() -> usize {
        use std::mem::size_of;
        // Actual None/Some initializer, parameter and match operand, captured
        // by borrow, plus this single common reservation boundary.
        3 * size_of::<Option<&Self>>()
            + size_of::<&Option<&Self>>()
            + size_of::<&mut Admission<'_>>()
            + size_of::<Result<(), Refusal>>()
    }
    pub(crate) fn shared_transport_controls() -> usize {
        // Only Some reaches get: None never reserves a statically dead read.
        // Five refs: caller initializer, factory parameter, Some payload,
        // match binding, consuming parameter. get's own receiver is below.
        Self::transport_controls() + 5 * std::mem::size_of::<&Self>() + Self::read_controls()
    }
    pub(crate) fn read_controls() -> usize {
        use std::mem::size_of;
        // get and its possible refuse forwarding Result, authentication reserve and
        // work Results, the new receiver/argument and exact-pointer borrows.
        2 * size_of::<Result<bool, Refusal>>()
            + 2 * size_of::<Result<(), Refusal>>()
            + size_of::<&Self>()
            + 4 * size_of::<&DeclarationIndex>()
            + 4 * size_of::<&mut Admission<'_>>()
    }

    pub(crate) fn new(
        index: &'a DeclarationIndex,
        admission: &mut Admission<'_>,
    ) -> Result<Self, Refusal> {
        admission.reserve(Self::constructor_controls())?;
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
        // The Directory caller prepays one read bank before Some/closure/get
        // evaluation. This exact original gateway keeps sticky/work/Subject
        // ordering; it creates no new allowance and retains no donor payer.
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
