//! Borrowed old top-level authority over the caller-held authored index.
//!
//! `index_authored` also holds fields, cases, lifecycles, class methods,
//! interfaces and protocols, so the raw map is not the old private authority.
//! This view answers exactly the old `program.functions`/`program.types` map:
//! the caller and every member are proven by exact current pointers, the held
//! index size equals this slice's complete authored census, its charged count
//! of function/type entries equals the old member count, and each lookup
//! accepts only a function or type entry keyed by its own identity. A slice
//! holding class methods keeps the legacy owned map, because a method shares
//! the function kind in the held index. No map, key, declaration, digest or
//! layout witness is copied, cached or treated as authority.
use super::{debit, refusal, AuthoredDeclaration, BTreeMap, Diagnostic, Program};
use crate::ast::TypeDeclarationKind;
use crate::workspace_graph::AuthoredKind;

/// The only lookup the private traversal performs. The legacy owned map and
/// the proven borrowed view both answer through it.
pub(in crate::workspace_graph) trait Lookup<'a> {
    fn get(&self, id: &str) -> Option<&AuthoredDeclaration<'a>>;
}
impl<'a> Lookup<'a> for BTreeMap<&str, AuthoredDeclaration<'a>> {
    fn get(&self, id: &str) -> Option<&AuthoredDeclaration<'a>> {
        BTreeMap::get(self, id)
    }
}

pub(super) struct TopLevel<'a> {
    index: &'a BTreeMap<&'a str, AuthoredDeclaration<'a>>,
}
impl<'a> TopLevel<'a> {
    /// `Ok(None)` selects the legacy owned map. `indexed` is the old member
    /// count the caller has already debited; one exact lookup per member
    /// replaces the old insertion. The class scan, the census of child
    /// entries and the whole-index kind census are separately charged before
    /// they run.
    pub(super) fn prove(
        caller: &Program,
        programs: &'a [Program],
        index: &'a BTreeMap<&'a str, AuthoredDeclaration<'a>>,
        types: usize,
        indexed: usize,
        work: &mut usize,
    ) -> Result<Option<Self>, Vec<Diagnostic>> {
        let stride = std::mem::size_of::<Program>();
        let member = (caller as *const Program as usize)
            .checked_sub(programs.as_ptr() as usize)
            .filter(|offset| offset % stride == 0)
            .and_then(|offset| programs.get(offset / stride));
        if !member.is_some_and(|held| std::ptr::eq(held, caller)) {
            return Err(refusal());
        }
        debit(work, programs.len().checked_add(types).ok_or_else(refusal)?)?;
        let classes = programs.iter().flat_map(|p| &p.types).any(|ty| {
            matches!(&ty.kind, TypeDeclarationKind::Class { methods, .. } if !methods.is_empty())
        });
        if classes {
            return Ok(None);
        }
        let mut census = indexed;
        for program in programs {
            for function in &program.functions {
                let (key, entry) = index
                    .get_key_value(function.stable_id.as_str())
                    .ok_or_else(refusal)?;
                if !std::ptr::eq(*key, function.stable_id.as_str())
                    || entry.kind != AuthoredKind::Function
                    || !entry
                        .function
                        .is_some_and(|held| std::ptr::eq(held, function))
                    || entry.ty.is_some()
                    || !exact(entry, program, function.explicit_id)
                {
                    return Err(refusal());
                }
            }
            for ty in &program.types {
                let (key, entry) = index
                    .get_key_value(ty.stable_id.as_str())
                    .ok_or_else(refusal)?;
                if !std::ptr::eq(*key, ty.stable_id.as_str())
                    || entry.kind != AuthoredKind::Type
                    || !entry.ty.is_some_and(|held| std::ptr::eq(held, ty))
                    || entry.function.is_some()
                    || !exact(entry, program, ty.explicit_id)
                {
                    return Err(refusal());
                }
                let children = match &ty.kind {
                    TypeDeclarationKind::Record { fields } => fields.len(),
                    TypeDeclarationKind::Class { fields, methods } => fields
                        .len()
                        .checked_add(methods.len())
                        .ok_or_else(refusal)?,
                    TypeDeclarationKind::Variant { cases } => {
                        debit(work, cases.len())?;
                        cases
                            .iter()
                            .try_fold(cases.len(), |n, case| n.checked_add(case.fields.len()))
                            .ok_or_else(refusal)?
                    }
                    TypeDeclarationKind::Resource { lifecycles } => {
                        debit(work, lifecycles.len())?;
                        lifecycles.iter().filter(|l| l.stable_id.is_some()).count()
                    }
                };
                census = census.checked_add(children).ok_or_else(refusal)?;
            }
            debit(
                work,
                program
                    .interfaces
                    .len()
                    .checked_add(program.protocols.len())
                    .ok_or_else(refusal)?,
            )?;
            census = program
                .interfaces
                .iter()
                .try_fold(census, |n, i| {
                    n.checked_add(1)?.checked_add(i.imports.len())
                })
                .and_then(|n| {
                    program
                        .protocols
                        .iter()
                        .try_fold(n, |n, p| n.checked_add(1)?.checked_add(p.methods.len()))
                })
                .ok_or_else(refusal)?;
        }
        // Any other size is a stale, partial or wider source slice.
        if index.len() != census {
            return Err(refusal());
        }
        // Total size alone admits a same-size replacement of a child entry by
        // a foreign function or type keyed by its own identity. The proven
        // members are `indexed` distinct function/type entries, so an equal
        // charged kind census leaves no other entry `get` could return.
        debit(work, index.len())?;
        let kinds = index
            .values()
            .filter(|entry| matches!(entry.kind, AuthoredKind::Function | AuthoredKind::Type))
            .count();
        if kinds != indexed {
            return Err(refusal());
        }
        Ok(Some(Self { index }))
    }
}
fn exact(entry: &AuthoredDeclaration<'_>, program: &Program, explicit: bool) -> bool {
    std::ptr::eq(entry.path, program.path.as_str())
        && std::ptr::eq(entry.module, program.module.as_str())
        && entry.explicit == explicit
}
impl<'a> Lookup<'a> for TopLevel<'a> {
    // Constant work per lookup, like the old owned map's uncharged search.
    // The kind census leaves exactly the proven members as function/type
    // entries.
    fn get(&self, id: &str) -> Option<&AuthoredDeclaration<'a>> {
        let (key, entry) = self.index.get_key_value(id)?;
        let own = match (entry.kind, entry.function, entry.ty) {
            (AuthoredKind::Function, Some(function), None) => function.stable_id.as_str(),
            (AuthoredKind::Type, None, Some(ty)) => ty.stable_id.as_str(),
            _ => return None,
        };
        std::ptr::eq(*key, own).then_some(entry)
    }
}

#[cfg(test)]
thread_local! {
    static LEGACY_CALCULATED: std::cell::Cell<(usize, usize)> =
        const { std::cell::Cell::new((0, 0)) };
}
/// Test-only label for the old owned-map byte formula a borrowed view did not
/// request. It is never reported as an actual authority-index request.
#[cfg(test)]
pub(super) fn record_legacy_calculated(indexed: usize) {
    let bytes = crate::cleanup_plan::replay_btree_node_upper::<&str, AuthoredDeclaration<'_>>()
        .and_then(|node| indexed.checked_mul(node))
        .unwrap_or(usize::MAX);
    LEGACY_CALCULATED.with(|cell| {
        let (views, total) = cell.get();
        cell.set((views.saturating_add(1), total.saturating_add(bytes)));
    });
}
/// Returns borrowed-view derivations and their summed legacy formula bytes.
#[cfg(test)]
pub(super) fn observe_legacy_calculated<T>(operation: impl FnOnce() -> T) -> (T, usize, usize) {
    let saved = LEGACY_CALCULATED.with(|cell| cell.replace((0, 0)));
    let result = operation();
    let (views, bytes) = LEGACY_CALCULATED.with(|cell| cell.replace(saved));
    (result, views, bytes)
}
