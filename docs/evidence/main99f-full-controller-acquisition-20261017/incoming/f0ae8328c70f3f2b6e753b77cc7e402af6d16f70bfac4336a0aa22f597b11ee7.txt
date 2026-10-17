//! Lookup data grants no declaration/body verdict; the old constructor checks remain.
use super::*;
use crate::hir::function_directory::Directory;
mod permits;
pub(super) use permits::Permits;

#[derive(Clone)]
pub(super) enum Lookup<'a> {
    Legacy(BTreeMap<DeclarationId, &'a ResolvedFunction>),
    Owned {
        directory: std::rc::Rc<Directory<'a>>,
        visible: usize,
    },
    Borrowed {
        directory: &'a Directory<'a>,
        visible: usize,
    },
}
impl<'a> Lookup<'a> {
    pub(super) fn shared_directory(&self) -> Option<std::rc::Rc<Directory<'a>>> {
        match self {
            Self::Owned { directory, .. } => Some(directory.clone()),
            _ => None,
        }
    }
    pub(super) fn permits_borrowed(&self, program: &'a ResolvedProgram) -> Permits<'a> {
        match self {
            Self::Borrowed { directory, .. } => Permits::Borrowed((*directory).permits()),
            Self::Legacy(_) | Self::Owned { .. } => Permits::Source(&program.permits),
        }
    }
    // Complete original BTree receiver for independently compiled saved validators.
    #[cfg(test)]
    pub(super) fn permits(&self, program: &'a ResolvedProgram) -> Permits<'a> {
        match self {
            Self::Borrowed { directory, .. } => Permits::Borrowed((*directory).permits()),
            Self::Legacy(_) | Self::Owned { .. } => {
                Permits::Legacy(program.permits.iter().map(String::as_str).collect())
            }
        }
    }
    pub(super) fn directory(&self) -> Option<&'a Directory<'a>> {
        match self {
            Self::Legacy(_) | Self::Owned { .. } => None,
            Self::Borrowed { directory, .. } => Some(directory),
        }
    }
    fn insert(&mut self, function: &'a ResolvedFunction) -> Result<bool, Diagnostic> {
        match self {
            Self::Legacy(map) => Ok(map.insert(function.id.clone(), function).is_some()),
            Self::Borrowed { directory, visible } => {
                // Only the original current source-order member may enter.
                if !directory.member_is(*visible, function)
                    || directory.first(&function.id).is_none()
                {
                    return Err(hir_error(
                        "function directory does not index the current function",
                    ));
                }
                let duplicate = directory
                    .first(&function.id)
                    .is_some_and(|entry| entry.ordinal < *visible);
                *visible += 1;
                Ok(duplicate)
            }
            Self::Owned { directory, visible } => {
                // Only the original current source-order member may enter.
                if !directory.member_is(*visible, function)
                    || directory.first(&function.id).is_none()
                {
                    return Err(hir_error_static(
                        "function directory does not index the current function",
                    ));
                }
                let duplicate = directory
                    .first(&function.id)
                    .is_some_and(|entry| entry.ordinal < *visible);
                *visible += 1;
                Ok(duplicate)
            }
        }
    }
    pub(super) fn get(&self, id: &DeclarationId) -> Option<&&'a ResolvedFunction> {
        match self {
            Self::Legacy(map) => map.get(id),
            Self::Borrowed { directory, visible } => directory
                .first(id)
                .filter(|entry| entry.ordinal < *visible)
                .map(|entry| &entry.function),
            Self::Owned { directory, visible } => directory
                .first(id)
                .filter(|entry| entry.ordinal < *visible)
                .map(|entry| &entry.function),
        }
    }
    pub(super) fn contains_key(&self, id: &DeclarationId) -> bool {
        self.get(id).is_some()
    }
    #[cfg(test)]
    pub(super) fn len(&self) -> usize {
        match self {
            Self::Legacy(map) => map.len(),
            Self::Borrowed { visible, .. } | Self::Owned { visible, .. } => *visible,
        }
    }
    #[cfg(test)]
    pub(super) fn keys(&self) -> Keys<'_, 'a> {
        match self {
            Self::Legacy(map) => Keys::Legacy(map.keys()),
            Self::Borrowed { directory, visible } => Keys::Borrowed {
                entries: directory.entries().iter(),
                visible: *visible,
            },
            Self::Owned { directory, visible } => Keys::Borrowed {
                entries: directory.entries().iter(),
                visible: *visible,
            },
        }
    }
}

#[cfg(test)]
pub(super) enum Keys<'s, 'a> {
    Legacy(std::collections::btree_map::Keys<'s, DeclarationId, &'a ResolvedFunction>),
    Borrowed {
        entries: std::slice::Iter<'s, crate::hir::function_directory::Entry<'a>>,
        visible: usize,
    },
}
#[cfg(test)]
impl<'s, 'a: 's> Iterator for Keys<'s, 'a> {
    type Item = &'s DeclarationId;
    fn next(&mut self) -> Option<Self::Item> {
        match self {
            Self::Legacy(keys) => keys.next(),
            Self::Borrowed { entries, visible } => {
                for entry in entries.by_ref() {
                    if entry.ordinal < *visible {
                        return Some(&entry.function.id);
                    }
                }
                None
            }
        }
    }
}

pub(super) fn authenticate<'a>(
    program: &'a ResolvedProgram,
    directory: Option<&'a Directory<'a>>,
) -> Result<Lookup<'a>, Diagnostic> {
    authenticate_shared(program, directory, None)
}
pub(super) fn authenticate_shared<'a>(
    program: &'a ResolvedProgram,
    directory: Option<&'a Directory<'a>>,
    shared: Option<std::rc::Rc<Directory<'a>>>,
) -> Result<Lookup<'a>, Diagnostic> {
    if shared.is_some() {
        crate::hir::ordinary_scan::fund(Some(0))?;
    }
    let mut functions = match directory {
        Some(directory) if directory.is_current(program) => Lookup::Borrowed {
            directory,
            visible: 0,
        },
        Some(_) => {
            return Err(hir_error(
                "function directory is not bound to the current resolved program",
            ))
        }
        None => {
            let shared = match shared {
                Some(directory) if directory.is_current(program) => Some(directory),
                Some(_) => {
                    return Err(hir_error_static(
                        "function directory is not bound to the current resolved program",
                    ))
                }
                None if crate::bounded_output::active_remaining().is_some() => Some(
                    crate::hir::function_directory::caller::prepare_shared(program).map_err(
                        |_| {
                            hir_error_static(
                                "function directory storage exceeds the active validation budget",
                            )
                        },
                    )?,
                ),
                None => None,
            };
            shared.map_or_else(
                || Lookup::Legacy(BTreeMap::new()),
                |directory| Lookup::Owned {
                    directory,
                    visible: 0,
                },
            )
        }
    };
    for function in &program.functions {
        if crate::host_io_ops::by_id(function.id.as_str()).is_some()
            || crate::command_io_ops::by_id(function.id.as_str()).is_some()
        {
            return Err(lookup_error(
                &function.id,
                "resolved function `",
                "` aliases a compiler-owned host I/O operation",
            )?);
        }
        if functions.insert(function)? {
            return Err(lookup_error(
                &function.id,
                "duplicate resolved function identity `",
                "`",
            )?);
        }
        match program.declarations.declaration(&function.id) {
            Some(declaration)
                if declaration.kind == DeclarationKind::Function
                    && declaration.name == function.name => {}
            Some(_) => {
                return Err(lookup_error(
                    &function.id,
                    "resolved function `",
                    "` disagrees with its declaration index entry",
                )?);
            }
            None => {
                return Err(lookup_error(
                    &function.id,
                    "resolved function `",
                    "` is absent from the declaration index",
                )?);
            }
        }
    }
    Ok(functions)
}

#[cfg(test)]
mod tests;

#[cfg(test)]
#[allow(
    dead_code,
    reason = "complete historical receiver retained for independent test comparisons"
)]
pub(super) mod original;

fn lookup_error(
    id: &DeclarationId,
    before: &'static str,
    after: &'static str,
) -> Result<Diagnostic, Diagnostic> {
    if crate::bounded_output::active_remaining().is_none() {
        return Ok(hir_error(format!("{before}{id}{after}")));
    }
    let bytes = before
        .len()
        .checked_add(id.as_str().len())
        .and_then(|n| n.checked_add(after.len()));
    crate::hir::ordinary_scan::fund(bytes)?;
    let bytes = bytes.expect("paid representable text layout");
    let mut message = String::new();
    if message.try_reserve_exact(bytes).is_err() {
        crate::bounded_output::refuse_active_required(bytes);
        return Err(hir_error_static(
            "ordinary HIR scan backing allocation failed",
        ));
    }
    if message.capacity() != bytes {
        let excess = message.capacity().saturating_sub(bytes);
        if excess != 0 {
            crate::bounded_output::reserve_active_required(excess);
        }
        crate::bounded_output::refuse_active_required(bytes);
        return Err(hir_error_static(
            "ordinary HIR scan backing allocation failed",
        ));
    }
    message.push_str(before);
    message.push_str(id.as_str());
    message.push_str(after);
    Ok(hir_error(message))
}

#[cfg(test)]
pub(super) mod paid_custody;
