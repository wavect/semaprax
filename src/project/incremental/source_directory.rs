//! Immutable current parsed owners. The receiving route shares the one paid
//! directory; historical path order is validated, never sorted or repaired.
use crate::workspace_graph::current_source::CurrentParsedSource;
use std::collections::{btree_map, BTreeMap};
use std::sync::Arc;
type Owner = Arc<CurrentParsedSource>;
#[derive(Clone)]
pub(super) enum SourceDirectory {
    Owned(BTreeMap<String, Owner>),
    Current(Arc<Vec<Owner>>),
}
impl Default for SourceDirectory {
    fn default() -> Self {
        Self::Owned(BTreeMap::new())
    }
}
impl From<BTreeMap<String, Owner>> for SourceDirectory {
    fn from(entries: BTreeMap<String, Owner>) -> Self {
        Self::Owned(entries)
    }
}
impl SourceDirectory {
    pub(super) fn current(owners: &Arc<Vec<Owner>>) -> Result<Self, ()> {
        if owners
            .windows(2)
            .any(|pair| pair[0].input().path >= pair[1].input().path)
            || owners
                .iter()
                .any(|owner| owner.input().path != owner.program().path)
        {
            return Err(());
        }
        Ok(Self::Current(Arc::clone(owners)))
    }
    pub(super) fn len(&self) -> usize {
        match self {
            Self::Owned(entries) => entries.len(),
            Self::Current(owners) => owners.len(),
        }
    }
    #[cfg(test)]
    pub(super) fn is_empty(&self) -> bool {
        self.len() == 0
    }
    pub(super) fn get(&self, path: &str) -> Option<&Owner> {
        match self {
            Self::Owned(entries) => entries.get(path),
            Self::Current(owners) => owners
                .binary_search_by(|owner| owner.input().path.as_str().cmp(path))
                .ok()
                .map(|index| &owners[index]),
        }
    }
    pub(super) fn contains_key(&self, path: &str) -> bool {
        self.get(path).is_some()
    }
    pub(super) fn iter(&self) -> Entries<'_> {
        match self {
            Self::Owned(entries) => Entries::Owned(entries.iter()),
            Self::Current(owners) => Entries::Current(owners.iter()),
        }
    }
    pub(super) fn keys(&self) -> impl ExactSizeIterator<Item = &String> + DoubleEndedIterator {
        self.iter().map(|(path, _)| path)
    }
    /// Snapshot restoration has an independent authenticated owning directory.
    /// Receiving directories are never changed after their publication.
    pub(super) fn insert(&mut self, path: String, owner: Owner) -> Option<Owner> {
        match self {
            Self::Owned(entries) => entries.insert(path, owner),
            Self::Current(_) => panic!("immutable current source directory cannot be mutated"),
        }
    }
}
pub(super) enum Entries<'a> {
    Owned(btree_map::Iter<'a, String, Owner>),
    Current(std::slice::Iter<'a, Owner>),
}
impl<'a> Iterator for Entries<'a> {
    type Item = (&'a String, &'a Owner);
    fn next(&mut self) -> Option<Self::Item> {
        match self {
            Self::Owned(iter) => iter.next(),
            Self::Current(iter) => iter.next().map(|owner| (&owner.input().path, owner)),
        }
    }
    fn size_hint(&self) -> (usize, Option<usize>) {
        match self {
            Self::Owned(iter) => iter.size_hint(),
            Self::Current(iter) => iter.size_hint(),
        }
    }
}
impl DoubleEndedIterator for Entries<'_> {
    fn next_back(&mut self) -> Option<Self::Item> {
        match self {
            Self::Owned(iter) => iter.next_back(),
            Self::Current(iter) => iter.next_back().map(|owner| (&owner.input().path, owner)),
        }
    }
}
impl ExactSizeIterator for Entries<'_> {}
impl std::iter::FusedIterator for Entries<'_> {}
impl<'a> IntoIterator for &'a SourceDirectory {
    type Item = (&'a String, &'a Owner);
    type IntoIter = Entries<'a>;
    fn into_iter(self) -> Self::IntoIter {
        self.iter()
    }
}
impl std::ops::Index<&String> for SourceDirectory {
    type Output = Owner;
    fn index(&self, path: &String) -> &Owner {
        self.get(path).expect("source directory key exists")
    }
}
impl std::ops::Index<&str> for SourceDirectory {
    type Output = Owner;
    fn index(&self, path: &str) -> &Owner {
        self.get(path).expect("source directory key exists")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn immutable_current_directory_keeps_full_order_lookup_and_receiving_identity() {
        let empty = SourceDirectory::default();
        assert!(empty.is_empty());
        let owners = ["a.spx", "z.spx"]
            .into_iter()
            .map(|path| {
                let source = "module source; @id(\"source.main\") fn main()->i64 {0}";
                super::super::fixture_current_owner(
                    source.to_owned(),
                    crate::parse(source, path).unwrap(),
                )
            })
            .collect::<Vec<_>>();
        let owners = Arc::new(owners);
        let directory = SourceDirectory::current(&owners).unwrap();
        assert_eq!(
            directory.keys().map(String::as_str).collect::<Vec<_>>(),
            ["a.spx", "z.spx"]
        );
        assert!(Arc::ptr_eq(directory.get("a.spx").unwrap(), &owners[0]));
        assert!(directory.get("unknown.spx").is_none());
        assert_eq!(directory.iter().next_back().unwrap().0, "z.spx");
        let reversed = Arc::new(vec![Arc::clone(&owners[1]), Arc::clone(&owners[0])]);
        assert!(SourceDirectory::current(&reversed).is_err());
        let duplicates = Arc::new(vec![Arc::clone(&owners[0]), Arc::clone(&owners[0])]);
        assert!(SourceDirectory::current(&duplicates).is_err());
    }
}
