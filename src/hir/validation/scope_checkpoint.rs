//! Private funded persistent validator scopes. Checkpoints share one immutable
//! root; ordered reads and full-binding mutation remain independent of HIR facts.
use super::*;
use std::rc::Rc;
pub(super) mod charge;
#[cfg(test)]
mod coarse;
mod fresh;
mod marks;
pub(super) mod merge;
pub(super) mod paths;
mod storage;
mod transaction;
mod tree;
pub(super) use storage::Storage;
mod views;
pub(super) use views::{IdSnapshot, Ids, Owned, Read, Write};
#[derive(Clone, Default)]
pub(super) struct Scope {
    root: Option<Rc<tree::Node>>,
}
impl Scope {
    #[cfg(test)]
    pub(super) fn fresh_baseline(values: &dyn ScopeRead) -> Result<Self, Diagnostic> {
        charge::fresh_baseline(values)?;
        Ok(Self {
            root: tree::build_fresh(&mut values.iter(), values.len()),
        })
    }
    pub(super) fn fresh_baseline_persistent(values: &Scope) -> Result<Self, Diagnostic> {
        fresh::baseline(values)
    }
    pub(super) fn prepare(count: usize) -> Result<(), Diagnostic> {
        charge::initial(count)
    }
    pub(super) fn from_prepaid(values: BTreeMap<ValueId, ValidationBinding>) -> Self {
        let count = values.len();
        Self {
            root: tree::build(&mut values.into_iter(), count),
        }
    }
    pub(super) fn get_mut(
        &mut self,
        key: &ValueId,
    ) -> Result<Option<&mut ValidationBinding>, Diagnostic> {
        if self.get(key).is_none() {
            return Ok(None);
        }
        charge::edit(&self.root, key)?;
        Ok(tree::get_mut(&mut self.root, key))
    }
    pub(super) fn copy_binding_from(
        &mut self,
        source: &Scope,
        key: &ValueId,
    ) -> Result<(), Diagnostic> {
        let Some(value) = tree::value(source.root.as_deref(), key) else {
            return Ok(());
        };
        transaction::insert_current(&mut self.root, key.clone(), value.clone())
    }
    pub(super) fn insert(
        &mut self,
        key: ValueId,
        value: ValidationBinding,
    ) -> Result<(), Diagnostic> {
        self.prepare_insert()?.insert(key, value)
    }
    pub(super) fn prepare_insert(&mut self) -> Result<PreparedInsert<'_>, Diagnostic> {
        charge::reserve(Some(charge::carrier::<ValidationBinding>()))?;
        Ok(PreparedInsert { scope: self })
    }
}
// The carrier is paid before the caller clones owned type/payload data. This
// exclusive borrow survives every fallible path constructor until one commit.
pub(super) struct PreparedInsert<'a> {
    scope: &'a mut Scope,
}
impl PreparedInsert<'_> {
    pub(super) fn insert(self, key: ValueId, value: ValidationBinding) -> Result<(), Diagnostic> {
        charge::reserve(Some(0))?;
        transaction::insert_current(&mut self.scope.root, key, Rc::new(value))
    }
}

impl PartialEq for Scope {
    fn eq(&self, other: &Self) -> bool {
        match (&self.root, &other.root) {
            (None, None) => true,
            (Some(a), Some(b)) if Rc::ptr_eq(a, b) => true,
            _ => self.len() == other.len() && self.iter().eq(other.iter()),
        }
    }
}
impl Eq for Scope {}
impl std::fmt::Debug for Scope {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_map().entries(self.iter()).finish()
    }
}
pub(super) struct Publication<'a> {
    target: views::Target<'a>,
    pub(super) published: Scope,
    pub(super) enabled: bool,
    slots: usize,
}
impl<'a> Publication<'a> {
    pub(super) fn take(scope: &'a mut dyn Write) -> Result<Self, Diagnostic> {
        match scope.publication_target() {
            views::Target::Map(target) => {
                Scope::prepare(target.len())?;
                let published = Scope::from_prepaid(std::mem::take(target));
                let slots = published.len();
                Ok(Self {
                    target: views::Target::Map(target),
                    published,
                    enabled: true,
                    slots,
                })
            }
            views::Target::Checkpoint(target) => {
                let published = std::mem::take(target);
                Ok(Self {
                    target: views::Target::Checkpoint(target),
                    published,
                    enabled: true,
                    slots: 0,
                })
            }
        }
    }
    #[cfg(test)]
    pub(super) fn new(
        target: &'a mut BTreeMap<ValueId, ValidationBinding>,
        published: Scope,
    ) -> Self {
        let slots = published.len();
        Self {
            target: views::Target::Map(target),
            published,
            enabled: true,
            slots,
        }
    }
    pub(super) fn publish(&mut self, scope: &Scope) -> Result<(), Diagnostic> {
        if self.enabled {
            if matches!(&self.target, views::Target::Map(_)) && scope.len() > self.slots {
                charge::publication_slots(scope.len() - self.slots)?;
                self.slots = scope.len();
            }
            self.published = scope.clone();
        }
        Ok(())
    }
}
impl Drop for Publication<'_> {
    fn drop(&mut self) {
        // Continuations and current scopes are declared after this guard, so
        // they drop first on success and every early return. The private root
        // is then uniquely held; payloads move into already charged map slots.
        match &mut self.target {
            views::Target::Map(target) => tree::materialize(self.published.root.take(), target),
            views::Target::Checkpoint(target) => **target = std::mem::take(&mut self.published),
        }
    }
}

#[cfg(test)]
impl Scope {
    pub(super) fn retained_capacity(&self) -> usize {
        let carriers =
            self.len() * (charge::carrier::<tree::Node>() + charge::carrier::<ValidationBinding>());
        carriers
            + self
                .values()
                .map(|value| {
                    resolved_type_owned_capacity(&value.ty)
                        + value
                            .moved_places
                            .iter()
                            .map(|(path, _)| {
                                path.capacity() * std::mem::size_of::<PlaceProjection>()
                                    + std::mem::size_of::<(Vec<PlaceProjection>, Availability)>()
                            })
                            .sum::<usize>()
                        + value
                            .definitely_partial
                            .iter()
                            .map(|path| {
                                path.capacity() * std::mem::size_of::<PlaceProjection>()
                                    + std::mem::size_of::<Vec<PlaceProjection>>()
                            })
                            .sum::<usize>()
                        + value.active_loans.len() * std::mem::size_of::<LoanId>()
                })
                .sum::<usize>()
    }
}
#[cfg(test)]
impl IdSnapshot {
    pub(super) fn retained_capacity(&self) -> usize {
        match self {
            Self::Old(values) => values.capacity() * std::mem::size_of::<ValueId>(),
            Self::Checkpoint(scope) => scope.retained_capacity(),
        }
    }
}

#[cfg(test)]
pub(super) fn insert_loan(target: &mut ValidationBinding, loan: LoanId) -> Result<(), Diagnostic> {
    if !target.active_loans.contains(&loan) {
        charge::reserve(crate::cleanup_plan::replay_btree_node_upper::<LoanId, ()>())?;
    }
    target.active_loans.insert(loan);
    Ok(())
}

pub(super) fn move_projection(
    target: &mut ValidationBinding,
    path: &[PlaceProjection],
) -> Result<(), Diagnostic> {
    let node = if target.moved_places.contains_key(path) {
        Some(0)
    } else {
        crate::cleanup_plan::replay_btree_node_upper::<Vec<PlaceProjection>, Availability>()
    };
    charge::reserve(node.and_then(|n| {
        path.len()
            .checked_mul(std::mem::size_of::<PlaceProjection>())?
            .checked_add(n)
    }))?;
    target
        .moved_places
        .insert(path.to_vec(), Availability::Moved);
    Ok(())
}

#[cfg(test)]
impl HirValidator<'_> {
    pub(in crate::hir) fn assert_scope_auxiliary_oracle(left: &Self, right: &Self) {
        assert_eq!(left.byte_slice_aliases, right.byte_slice_aliases);
        assert_eq!(left.borrowed_str_aliases, right.borrowed_str_aliases);
        assert_eq!(left.buffer_reopen_sites, right.buffer_reopen_sites);
        assert_eq!(left.static_text.get(), right.static_text.get());
        assert_eq!(left.selected.is_some(), right.selected.is_some());
    }
}

#[cfg(test)]
#[path = "scope_checkpoint/tests.rs"]
mod tests;

#[cfg(test)]
#[path = "scope_checkpoint/refusal_tests.rs"]
mod refusal_tests;

#[cfg(test)]
#[path = "scope_checkpoint/persistent_input_tests.rs"]
mod persistent_input_tests;

#[cfg(test)]
#[path = "scope_checkpoint/transaction_tests.rs"]
mod transaction_tests;

#[cfg(test)]
#[path = "scope_checkpoint/fresh/tests.rs"]
mod fresh_tests;
