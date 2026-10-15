//! Exact range/ordinal views of immutable checked HIR; no validation authority.
use super::*;
use crate::hir::items::{ItemSource, Iter, View};
use crate::project::incremental::checked_hir::CheckedModuleLease;
use std::ops::{Index, Range};
mod retain;
pub(super) use retain::retain_checked_module;

pub(super) enum Selection {
    Range(Range<usize>),
    Ordinals(Vec<usize>),
}
pub(super) enum Retained<T> {
    Owned(Vec<T>),
    Checked {
        owner: CheckedModuleLease,
        selection: Selection,
        read: fn(&hir::ResolvedProgram) -> &[T],
    },
}
impl<T> From<Vec<T>> for Retained<T> {
    fn from(items: Vec<T>) -> Self {
        Self::Owned(items)
    }
}
impl<T> ItemSource<T> for Retained<T> {
    fn view(&self) -> View<'_, T> {
        match self {
            Self::Owned(items) => View::slice(items),
            Self::Checked {
                owner,
                selection,
                read,
            } => match selection {
                Selection::Range(range) => View::slice(&read(owner)[range.clone()]),
                Selection::Ordinals(ordinals) => View::indexed(read(owner), ordinals),
            },
        }
    }
}
impl<T> Retained<T> {
    pub(super) fn iter(&self) -> Iter<'_, T> {
        self.view().iter()
    }
    pub(super) fn len(&self) -> usize {
        self.view().len()
    }
    pub(super) fn is_empty(&self) -> bool {
        self.len() == 0
    }
    pub(super) fn first(&self) -> Option<&T> {
        self.get(0)
    }
    pub(super) fn get(&self, index: usize) -> Option<&T> {
        self.view().get(index)
    }
    #[cfg(test)]
    fn as_ptr(&self) -> *const T {
        match self {
            Self::Owned(items) => items.as_ptr(),
            Self::Checked {
                owner,
                selection: Selection::Range(range),
                read,
            } => read(owner)[range.clone()].as_ptr(),
            _ => panic!("an ordinal selection has no contiguous pointer"),
        }
    }
    fn select(
        owner: CheckedModuleLease,
        read: fn(&hir::ResolvedProgram) -> &[T],
        keep: impl Fn(&T) -> bool,
    ) -> Result<Self, Vec<Diagnostic>> {
        Self::select_prepaid(owner, read, keep, 0)
    }
    /// Pay the original complete-input retention contract on this exact owner,
    /// then use its fixed carrier allowance for this call's smaller index only.
    /// No transferable receipt or credit crosses an active-ledger boundary.
    fn select_accounted(
        owner: CheckedModuleLease,
        read: fn(&hir::ResolvedProgram) -> &[T],
        keep: impl Fn(&T) -> bool,
        fixed: usize,
        extra: impl FnMut(&T) -> Result<usize, Vec<Diagnostic>>,
    ) -> Result<Self, Vec<Diagnostic>> {
        let prepaid = retained_vectors::reserve_full_input(read(&owner), fixed, extra)?;
        Self::select_prepaid(owner, read, keep, prepaid)
    }
    fn select_prepaid(
        owner: CheckedModuleLease,
        read: fn(&hir::ResolvedProgram) -> &[T],
        keep: impl Fn(&T) -> bool,
        prepaid: usize,
    ) -> Result<Self, Vec<Diagnostic>> {
        reserve_builder_structure(0)?;
        let items = read(&owner);
        let mut start = None;
        let mut end = 0;
        let mut count = 0usize;
        let mut gap = false;
        let mut contiguous = true;
        for (index, item) in items.iter().enumerate() {
            if keep(item) {
                count += 1;
                if gap {
                    contiguous = false;
                }
                start.get_or_insert(index);
                end = index + 1;
            } else if start.is_some() {
                gap = true;
            }
        }
        let selection = if contiguous {
            Selection::Range(start.unwrap_or(0)..end)
        } else {
            // Exact ordinal backing only: no T/HIR body materialization. The
            // enlarged retained carrier is paid by the actual module header.
            // This local Vec header overlaps that carrier during construction.
            let payload = count
                .checked_mul(std::mem::size_of::<usize>())
                .ok_or_else(|| vec![limit_error("builder_bytes", active_builder_limit())])?;
            let requested = std::mem::size_of::<Vec<usize>>()
                .checked_add(payload)
                .ok_or_else(|| vec![limit_error("builder_bytes", active_builder_limit())])?;
            reserve_builder_structure(requested.saturating_sub(prepaid))?;
            let mut ordinals = Vec::with_capacity(count);
            let actual = ordinals
                .capacity()
                .checked_mul(std::mem::size_of::<usize>())
                .ok_or_else(|| vec![limit_error("builder_bytes", active_builder_limit())])?;
            if actual > payload {
                let actual_request = std::mem::size_of::<Vec<usize>>()
                    .checked_add(actual)
                    .ok_or_else(|| vec![limit_error("builder_bytes", active_builder_limit())])?;
                reserve_builder_structure(actual_request.saturating_sub(prepaid.max(requested)))?;
            }
            for (index, item) in items.iter().enumerate() {
                if keep(item) {
                    ordinals.push(index);
                }
            }
            debug_assert_eq!(ordinals.len(), count);
            Selection::Ordinals(ordinals)
        };
        Ok(Self::Checked {
            owner,
            selection,
            read,
        })
    }
}
impl<T> Index<usize> for Retained<T> {
    type Output = T;
    fn index(&self, index: usize) -> &T {
        self.get(index).expect("retained ordinal out of bounds")
    }
}
impl<T: Clone> Retained<T> {
    pub(super) fn into_owned(self) -> Vec<T> {
        match self {
            Self::Owned(items) => items,
            shared => shared.iter().cloned().collect(),
        }
    }
    #[cfg(test)]
    pub(super) fn to_vec(&self) -> Vec<T> {
        self.iter().cloned().collect()
    }
    #[cfg(test)]
    pub(super) fn push(&mut self, item: T) {
        self.materialize().push(item);
    }
    #[cfg(test)]
    pub(super) fn iter_mut(&mut self) -> std::slice::IterMut<'_, T> {
        self.materialize().iter_mut()
    }
    #[cfg(test)]
    fn materialize(&mut self) -> &mut Vec<T> {
        if let Self::Checked { .. } = self {
            *self = Self::Owned(self.to_vec());
        }
        let Self::Owned(items) = self else {
            unreachable!()
        };
        items
    }
}
#[cfg(test)]
impl<T: Clone> std::ops::IndexMut<usize> for Retained<T> {
    fn index_mut(&mut self, index: usize) -> &mut T {
        &mut self.materialize()[index]
    }
}
impl<'a, T> IntoIterator for &'a Retained<T> {
    type Item = &'a T;
    type IntoIter = Iter<'a, T>;
    fn into_iter(self) -> Self::IntoIter {
        self.iter()
    }
}
impl<T: Clone> IntoIterator for Retained<T> {
    type Item = T;
    type IntoIter = std::vec::IntoIter<T>;
    fn into_iter(self) -> Self::IntoIter {
        self.into_owned().into_iter()
    }
}
#[cfg(test)]
impl<'a, T: Clone> IntoIterator for &'a mut Retained<T> {
    type Item = &'a mut T;
    type IntoIter = std::slice::IterMut<'a, T>;
    fn into_iter(self) -> Self::IntoIter {
        self.materialize().iter_mut()
    }
}
impl<T: std::fmt::Debug> std::fmt::Debug for Retained<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_list().entries(self.iter()).finish()
    }
}

#[cfg(test)]
mod tests;

// Preserve the legacy full-program carrier price and pay any actual enum tag
// growth. The new representation is not authority to reduce the old forecast.
pub(super) fn reserve_input_carriers(count: usize) -> Result<(), Vec<Diagnostic>> {
    let old = std::mem::size_of::<(String, hir::ResolvedProgram)>();
    let new = std::mem::size_of::<(String, ResolvedModuleInput)>();
    let each = std::mem::size_of::<String>()
        .checked_add(GRAPH_ACCOUNTED_RESOLVED_PROGRAM_BYTES)
        .and_then(|bytes| bytes.checked_add(new.saturating_sub(old)))
        .ok_or_else(|| vec![limit_error("builder_bytes", active_builder_limit())])?;
    reserve_builder_structure(
        count
            .checked_mul(each)
            .ok_or_else(|| vec![limit_error("builder_bytes", active_builder_limit())])?,
    )
}

/// Actual new uncached selector controls; the old carrier reservation remains
/// independently callable and unchanged. Workspace admission already bounds
/// count by MAX_FILES, so this multiplication cannot overflow.
pub(super) fn reserve_alias_input_carriers(count: usize) -> Result<(), Vec<Diagnostic>> {
    reserve_builder_structure(
        2 * std::mem::size_of::<Result<(), Vec<Diagnostic>>>()
            + 2 * std::mem::size_of::<usize>()
            + count * 2 * std::mem::size_of::<bool>(),
    )?;
    reserve_input_carriers(count)
}
