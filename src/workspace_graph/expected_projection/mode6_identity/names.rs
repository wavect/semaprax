use std::mem::size_of;

pub(super) struct Names<'a, V> {
    first: Option<(&'a str, V)>,
    rest: Vec<(&'a str, V)>,
}

impl<V> Default for Names<'_, V> {
    fn default() -> Self {
        Self {
            first: None,
            rest: Vec::new(),
        }
    }
}

impl<'a, V: Default> Names<'a, V> {
    pub(super) fn len(&self) -> usize {
        usize::from(self.first.is_some()) + self.rest.len()
    }

    pub(super) fn get(&self, name: &str) -> Option<&V> {
        if let Some((first, value)) = &self.first {
            if *first == name {
                return Some(value);
            }
        }
        let slot = self.rest.binary_search_by(|row| row.0.cmp(name)).ok()?;
        Some(&self.rest[slot].1)
    }

    pub(super) fn contains(&self, name: &str) -> bool {
        self.get(name).is_some()
    }

    pub(super) fn get_mut(&mut self, name: &str) -> Option<&mut V> {
        if let Some((first, value)) = &mut self.first {
            if *first == name {
                return Some(value);
            }
        }
        let slot = self.rest.binary_search_by(|row| row.0.cmp(name)).ok()?;
        Some(&mut self.rest[slot].1)
    }

    pub(super) fn entry(&mut self, name: &'a str) -> Option<&mut V> {
        let (first, value) = self.first.get_or_insert_with(|| (name, V::default()));
        if *first == name {
            return Some(value);
        }
        let slot = match self.rest.binary_search_by(|row| row.0.cmp(name)) {
            Ok(slot) => return Some(&mut self.rest[slot].1),
            Err(slot) => slot,
        };
        if self.rest.len() == self.rest.capacity() {
            let capacity = self.rest.capacity().checked_mul(2)?.max(2);
            reserve_exact(&mut self.rest, capacity)?;
        }
        self.rest.insert(slot, (name, V::default()));
        Some(&mut self.rest[slot].1)
    }
}

pub(super) fn reserve_exact<T>(rows: &mut Vec<T>, capacity: usize) -> Option<()> {
    let bytes = capacity.checked_mul(size_of::<T>())?;
    if bytes == 0 {
        return Some(());
    }
    if !crate::bounded_output::reserve_active_required(bytes) {
        return None;
    }
    if rows.try_reserve_exact(capacity - rows.len()).is_err() || rows.capacity() != capacity {
        crate::bounded_output::refuse_active_required(bytes);
        return None;
    }
    Some(())
}
