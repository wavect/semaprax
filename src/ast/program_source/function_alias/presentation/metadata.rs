//! Explicit name lookup and membership without constructing presentation values.
//! Payload lookup is for name-invariant structural consumers only. It grants no
//! wrapper/provider authority; those consumers receive the real FunctionRef.
use super::{Function, FunctionEntry, FunctionRef, FunctionSlices};

impl<'a> FunctionSlices<'a> {
    pub(crate) fn has_stored_aliases(&self) -> bool {
        match self {
            Self::Ordinary { .. } => false,
            Self::Projected {
                directory, start, ..
            } => directory.entries.get(*start..).is_some_and(|entries| {
                entries
                    .iter()
                    .any(|entry| matches!(entry, FunctionEntry::Borrowed(_)))
            }),
        }
    }
    /// Raw byte-helper proof cursors support ordinary payload subjects only.
    /// Stop at a matching alias rather than skipping to a later duplicate.
    pub(crate) fn unaliased_payload_by_name(&self, name: &str) -> Option<&'a Function> {
        match self {
            Self::Ordinary { .. } => self.payload_by_name(name),
            Self::Projected {
                authored,
                directory,
                start,
            } => {
                if let Some(function) = authored.iter().find(|function| function.name == name) {
                    return Some(function);
                }
                for entry in directory.entries.get(*start..)? {
                    match entry {
                        FunctionEntry::Owned(index) => {
                            if let Some(function) = directory.roots.functions.get(*index) {
                                if function.name == name {
                                    return Some(function);
                                }
                            }
                        }
                        FunctionEntry::Borrowed(view) if view._alias == name => return None,
                        FunctionEntry::Borrowed(_) => {}
                    }
                }
                None
            }
        }
    }

    pub(crate) fn contains_unaliased_payload(&self, target: &Function) -> bool {
        let same = |function: &Function| std::ptr::eq(function, target);
        match self {
            Self::Ordinary {
                authored,
                indices,
                generated,
            } => {
                let authored_match = match indices {
                    Some(indices) => indices
                        .iter()
                        .any(|index| authored.get(*index).is_some_and(same)),
                    None => authored.iter().any(same),
                };
                authored_match || generated.iter().any(same)
            }
            Self::Projected {
                authored,
                directory,
                start,
            } => {
                authored.iter().any(same)
                    || directory.entries.get(*start..).is_some_and(|entries| {
                        entries.iter().any(|entry| match entry {
                            FunctionEntry::Owned(index) => {
                                directory.roots.functions.get(*index).is_some_and(same)
                            }
                            FunctionEntry::Borrowed(_) => false,
                        })
                    })
            }
        }
    }

    /// The old target receiver returned the actual directory member, not a
    /// caller reference with an erased lifetime. Keep its pointer-find order
    /// and Option/reference widths, excluding every stored alias entry.
    pub(crate) fn unaliased_payload_member(&self, target: &Function) -> Option<&'a Function> {
        let same = |function: &Function| std::ptr::eq(function, target);
        match self {
            Self::Ordinary {
                authored,
                indices,
                generated,
            } => {
                let authored_match = match indices {
                    Some(indices) => indices
                        .iter()
                        .find_map(|index| authored.get(*index).filter(|function| same(function))),
                    None => authored.iter().find(|function| same(function)),
                };
                authored_match.or_else(|| generated.iter().find(|function| same(function)))
            }
            Self::Projected {
                authored,
                directory,
                start,
            } => authored.iter().find(|function| same(function)).or_else(|| {
                directory
                    .entries
                    .get(*start..)?
                    .iter()
                    .find_map(|entry| match entry {
                        FunctionEntry::Owned(index) => directory
                            .roots
                            .functions
                            .get(*index)
                            .filter(|function| same(function)),
                        FunctionEntry::Borrowed(_) => None,
                    })
            }),
        }
    }

    pub(crate) fn payload_by_name(&self, name: &str) -> Option<&'a Function> {
        self.find_named_payload(name, |_| true)
    }

    /// Preserve `find(name && predicate)` order, including malformed duplicate
    /// spellings; this never substitutes last-wins declaration-map behavior.
    pub(crate) fn find_named_payload(
        &self,
        name: &str,
        mut predicate: impl FnMut(&Function) -> bool,
    ) -> Option<&'a Function> {
        match self {
            Self::Ordinary {
                authored,
                indices,
                generated,
            } => {
                let authored = match indices {
                    Some(indices) => indices.iter().find_map(|index| {
                        authored
                            .get(*index)
                            .filter(|function| function.name == name && predicate(function))
                    }),
                    None => authored
                        .iter()
                        .find(|function| function.name == name && predicate(function)),
                };
                authored.or_else(|| {
                    generated
                        .iter()
                        .find(|function| function.name == name && predicate(function))
                })
            }
            Self::Projected {
                authored,
                directory,
                start,
            } => {
                if let Some(function) = authored
                    .iter()
                    .find(|function| function.name == name && predicate(function))
                {
                    return Some(function);
                }
                directory
                    .entries
                    .get(*start..)?
                    .iter()
                    .find_map(|entry| match entry {
                        FunctionEntry::Owned(index) => directory
                            .roots
                            .functions
                            .get(*index)
                            .filter(|function| function.name == name && predicate(function)),
                        FunctionEntry::Borrowed(view) => (view._alias == name
                            && predicate(view.original))
                        .then_some(view.original),
                    })
            }
        }
    }

    pub(crate) fn contains_presentation(&self, target: &FunctionRef<'_>) -> bool {
        let ordinary = |function: &Function| {
            target.alias_cell.is_none() && std::ptr::eq(function, target.payload)
        };
        match self {
            Self::Ordinary {
                authored,
                indices,
                generated,
            } => {
                let authored_match = match indices {
                    Some(indices) => indices
                        .iter()
                        .any(|index| authored.get(*index).is_some_and(ordinary)),
                    None => authored.iter().any(ordinary),
                };
                authored_match || generated.iter().any(ordinary)
            }
            Self::Projected {
                authored,
                directory,
                start,
            } => {
                authored.iter().any(ordinary)
                    || directory.entries.get(*start..).is_some_and(|entries| {
                        entries.iter().any(|entry| match entry {
                            FunctionEntry::Owned(index) => {
                                directory.roots.functions.get(*index).is_some_and(ordinary)
                            }
                            FunctionEntry::Borrowed(view) => {
                                std::ptr::eq(view.original, target.payload)
                                    && target
                                        .alias_cell
                                        .is_some_and(|cell| std::ptr::eq(cell, view))
                            }
                        })
                    })
            }
        }
    }
}

#[cfg(test)]
#[path = "metadata/tests.rs"]
mod tests;
