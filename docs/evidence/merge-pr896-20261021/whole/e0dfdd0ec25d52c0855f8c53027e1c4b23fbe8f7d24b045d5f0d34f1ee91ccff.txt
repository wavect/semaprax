//! Complete preceding cold receiver; only method visibility/name differ.
use super::{
    debit, explicit_only, ledger, names, owner_index, private_name, refusal,
    reserve_builder_structure, sorting, visit, Absence, AuthoredDeclaration, BTreeMap, Diagnostic,
    Entry, Frame, LayoutClosure, Program, DEPTH,
};
impl<'a> LayoutClosure<'a> {
    pub(super) fn derive_with_work_before_capacity(
        caller: &Program,
        authored: &'a BTreeMap<&'a str, AuthoredDeclaration<'a>>,
        programs: &'a [Program],
        remaining: &mut usize,
        ledger: Option<&std::rc::Rc<ledger::Ledger>>,
    ) -> Result<Self, Vec<Diagnostic>> {
        if explicit_only(caller, authored, programs, remaining)? != Absence::Possible {
            return Ok(Self {
                entries: Vec::new(),
                owners: None,
            });
        }
        // Borrowed root traversal precedes naming. Explicit-only programs do
        // not acquire the new identifier census or private String backing.
        reserve_builder_structure(DEPTH * std::mem::size_of::<Frame<'_>>())?;
        let mut frames = Vec::with_capacity(DEPTH);
        if frames.capacity() != DEPTH {
            return Err(refusal());
        }
        let mut count = 0usize;
        visit(
            caller,
            authored,
            programs,
            &mut frames,
            remaining,
            |_, _, _| {
                count = count.checked_add(1usize).ok_or_else(refusal)?;
                Ok(())
            },
        )?;
        if count == 0 {
            return Ok(Self {
                entries: Vec::new(),
                owners: None,
            });
        }
        let mut ordinal = names::first(caller)?;
        debit(remaining, programs.len())?;
        let capacity = programs
            .iter()
            .try_fold(0usize, |n, p| n.checked_add(p.types.len()))
            .ok_or_else(refusal)?;
        reserve_builder_structure(
            capacity
                .checked_mul(std::mem::size_of::<Entry<'_>>())
                .ok_or_else(refusal)?,
        )?;
        let mut entries: Vec<Entry<'a>> = Vec::with_capacity(capacity);
        if frames.capacity() != DEPTH || entries.capacity() != capacity {
            return Err(refusal());
        }
        visit(
            caller,
            authored,
            programs,
            &mut frames,
            remaining,
            |module, declaration, remaining| {
                let scan = entries
                    .iter()
                    .try_fold(0usize, |n, e| {
                        n.checked_add(
                            e.declaration
                                .stable_id
                                .len()
                                .checked_add(declaration.stable_id.len())?,
                        )
                    })
                    .ok_or_else(refusal)?;
                *remaining = remaining.checked_sub(scan).ok_or_else(refusal)?;
                if entries
                    .iter()
                    .any(|e| e.declaration.stable_id == declaration.stable_id)
                {
                    return Ok(());
                }
                if entries.len() == capacity {
                    return Err(refusal());
                }
                let alias = private_name(ordinal)?;
                ordinal = ordinal.checked_add(1).ok_or_else(refusal)?;
                entries.push(Entry {
                    module,
                    declaration,
                    alias,
                });
                Ok(())
            },
        )?;
        sorting::sort(&mut entries, remaining, |a, b, work| {
            sorting::identity(&a.declaration.stable_id, &b.declaration.stable_id, work)
        })?;
        let owners =
            owner_index::Owners::build(caller, authored, programs, &entries, remaining, ledger)?;
        Ok(Self {
            entries,
            owners: Some(owners),
        })
    }
}
