//! Compatibility map insertion moves old Vec producers without new allocation.
use super::*;
use crate::hir::DeclarationId;
use std::{
    collections::BTreeMap,
    ops::{Deref, DerefMut},
};
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct IndexMap<V>(pub(crate) BTreeMap<DeclarationId, V>);
impl<V> Default for IndexMap<V> {
    fn default() -> Self {
        Self(BTreeMap::new())
    }
}
impl<V> IndexMap<V> {
    pub(crate) fn insert(&mut self, key: DeclarationId, values: impl Into<V>) -> Option<V> {
        self.0.insert(key, values.into())
    }
}
impl<V> Deref for IndexMap<V> {
    type Target = BTreeMap<DeclarationId, V>;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}
impl<V> DerefMut for IndexMap<V> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}
impl<'a, V> IntoIterator for &'a IndexMap<V> {
    type Item = (&'a DeclarationId, &'a V);
    type IntoIter = std::collections::btree_map::Iter<'a, DeclarationId, V>;
    fn into_iter(self) -> Self::IntoIter {
        self.0.iter()
    }
}
impl<V> IntoIterator for IndexMap<V> {
    type Item = (DeclarationId, V);
    type IntoIter = std::collections::btree_map::IntoIter<DeclarationId, V>;
    fn into_iter(self) -> Self::IntoIter {
        self.0.into_iter()
    }
}
/// One case ordinal retains the same canonical outer cases owner. It allocates
/// no per-case Arc or field Vec; legacy hostile/decoded indexes keep full Vecs.
#[derive(Clone)]
pub(crate) struct CaseFields {
    storage: CaseStorage,
}
#[derive(Clone)]
enum CaseStorage {
    Owned(NominalPayload<ResolvedFieldDeclaration>),
    Case {
        owner: NominalPayload<ResolvedVariantCaseDeclaration>,
        ordinal: usize,
    },
}
impl CaseFields {
    pub(crate) fn canonical(
        owner: NominalPayload<ResolvedVariantCaseDeclaration>,
        ordinal: usize,
    ) -> Self {
        assert!(owner.is_shared());
        assert!(ordinal < owner.len());
        Self {
            storage: CaseStorage::Case { owner, ordinal },
        }
    }
    pub(crate) fn drain(self, dispose: &mut impl FnMut(ResolvedType)) {
        if let CaseStorage::Owned(fields) = self.storage {
            fields.drain_recursive_types(dispose);
        }
    }
}
impl From<Vec<ResolvedFieldDeclaration>> for CaseFields {
    fn from(v: Vec<ResolvedFieldDeclaration>) -> Self {
        Self {
            storage: CaseStorage::Owned(v.into()),
        }
    }
}
impl Deref for CaseFields {
    type Target = Vec<ResolvedFieldDeclaration>;
    fn deref(&self) -> &Self::Target {
        match &self.storage {
            CaseStorage::Owned(v) => v,
            CaseStorage::Case { owner, ordinal } => &owner[*ordinal].fields,
        }
    }
}
impl DerefMut for CaseFields {
    fn deref_mut(&mut self) -> &mut Self::Target {
        match &mut self.storage {
            CaseStorage::Owned(values) => values,
            CaseStorage::Case { .. } => {
                panic!("shared case-field mutation requires explicit paid materialization")
            }
        }
    }
}
impl std::fmt::Debug for CaseFields {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.deref().fmt(f)
    }
}
impl PartialEq for CaseFields {
    fn eq(&self, other: &Self) -> bool {
        self.as_slice() == other.as_slice()
    }
}
impl Eq for CaseFields {}
impl<'a> IntoIterator for &'a CaseFields {
    type Item = &'a ResolvedFieldDeclaration;
    type IntoIter = std::slice::Iter<'a, ResolvedFieldDeclaration>;
    fn into_iter(self) -> Self::IntoIter {
        self.iter()
    }
}
