//! Exact-program borrowed function lookup; construction is owned by the caller.
use super::{DeclarationId, ResolvedFunction, ResolvedProgram};

pub(super) struct Entry<'a> {
    pub(super) ordinal: usize,
    pub(super) function: &'a ResolvedFunction,
}
pub(super) struct Directory<'a> {
    program: &'a ResolvedProgram,
    entries: Vec<Entry<'a>>,
    permits: Vec<&'a str>,
}
impl<'a> Directory<'a> {
    pub(super) fn is_current(&self, program: &ResolvedProgram) -> bool {
        std::ptr::eq(self.program, program)
    }
    #[cfg(test)]
    pub(super) fn entries(&self) -> &[Entry<'a>] {
        &self.entries
    }
    pub(super) fn permits(&self) -> &[&'a str] {
        &self.permits
    }
    pub(super) fn member_is(&self, ordinal: usize, function: &ResolvedFunction) -> bool {
        self.program
            .functions
            .get(ordinal)
            .is_some_and(|current| std::ptr::eq(current, function))
    }
    pub(super) fn first(&self, id: &DeclarationId) -> Option<&Entry<'a>> {
        let index = self
            .entries
            .partition_point(|entry| entry.function.id < *id);
        self.entries.get(index).filter(|entry| {
            entry.function.id == *id
                && self
                    .program
                    .functions
                    .get(entry.ordinal)
                    .is_some_and(|current| std::ptr::eq(current, entry.function))
        })
    }
}

pub(super) mod caller;
