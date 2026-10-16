//! Existing randomized declaration map, lending immutable presentation cells.
//! Insertion order, first stored key and last stored value remain HashMap's.
//! The backing inventory belongs to the verifier invocation, never this map.
use crate::ast::program_source::FunctionRef;
use std::collections::HashMap;

#[derive(Default)]
pub(super) struct FunctionMap<'a> {
    entries: HashMap<&'a str, &'a FunctionRef<'a>>,
}
impl<'a> FunctionMap<'a> {
    pub(super) fn new() -> Self {
        Self::default()
    }
    pub(super) fn insert(
        &mut self,
        name: &'a str,
        function: &'a FunctionRef<'a>,
    ) -> Option<&'a FunctionRef<'a>> {
        self.entries.insert(name, function)
    }
    pub(super) fn get(&self, name: &str) -> Option<&'a FunctionRef<'a>> {
        self.entries.get(name).copied()
    }
    pub(super) fn iter(
        &self,
    ) -> std::collections::hash_map::Iter<'_, &'a str, &'a FunctionRef<'a>> {
        self.entries.iter()
    }
    pub(super) fn keys(
        &self,
    ) -> std::collections::hash_map::Keys<'_, &'a str, &'a FunctionRef<'a>> {
        self.entries.keys()
    }
}
impl<'a> FromIterator<(&'a str, &'a FunctionRef<'a>)> for FunctionMap<'a> {
    fn from_iter<T: IntoIterator<Item = (&'a str, &'a FunctionRef<'a>)>>(iter: T) -> Self {
        Self {
            entries: iter.into_iter().collect(),
        }
    }
}
impl<'a> std::ops::Index<&str> for FunctionMap<'a> {
    type Output = FunctionRef<'a>;
    fn index(&self, name: &str) -> &Self::Output {
        self.entries[name]
    }
}

#[cfg(test)]
#[path = "function_map/tests.rs"]
mod tests;
