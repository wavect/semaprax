//! Actual affine receiving backing; no detachable pointer/capacity ticket.
//! Only this owner can mutate its Vec, and it never lends &mut Vec or a seal.
use super::{active, record_refusal, reserve, Budget};
use std::mem::size_of;
use std::ops::Deref;
use std::rc::Rc;

pub(crate) struct KeyDirectory<'a, T> {
    budget: Rc<Budget>,
    values: Vec<T>,
    authored: &'a [T],
    generated_len: usize,
}
impl<'a, T> KeyDirectory<'a, T> {
    pub(crate) fn allocate(authored: &'a [T], generated_len: usize) -> Result<Self, ()> {
        let budget = active().ok_or(())?;
        let count = authored
            .len()
            .checked_add(generated_len)
            .ok_or_else(|| poison(&budget, usize::MAX))?;
        let layout =
            std::alloc::Layout::array::<T>(count).map_err(|_| poison(&budget, usize::MAX))?;
        let bytes = layout
            .size()
            .checked_add(size_of::<Self>())
            .ok_or_else(|| poison(&budget, usize::MAX))?;
        required(&budget, bytes)?;
        let mut values = Vec::new();
        values
            .try_reserve_exact(count)
            .map_err(|_| poison(&budget, 0))?;
        if values.capacity() != count {
            let excess = values
                .capacity()
                .saturating_sub(count)
                .checked_mul(size_of::<T>())
                .ok_or_else(|| poison(&budget, usize::MAX))?;
            required(&budget, excess)?;
            poison(&budget, 0);
            return Err(());
        }
        Ok(Self {
            budget,
            values,
            authored,
            generated_len,
        })
    }
    fn current(&self) -> Result<(), ()> {
        if !active().is_some_and(|current| Rc::ptr_eq(&current, &self.budget)) {
            poison(&self.budget, 0);
            return Err(());
        }
        required(&self.budget, 0)
    }
    /// No growth, replacement or backing extraction is possible through push.
    pub(crate) fn push(&mut self, value: T) -> Result<(), ()> {
        self.current()?;
        if self.values.len() >= self.generated_len || self.values.len() >= self.values.capacity() {
            poison(&self.budget, 0);
            return Err(());
        }
        self.values.push(value);
        Ok(())
    }
    /// Construction rewrites elements only; no mutable Vec escapes.
    pub(crate) fn last_mut(&mut self) -> Option<&mut T> {
        self.values.last_mut()
    }
    pub(crate) fn validate(&self, authored: &[T]) -> Result<(), ()> {
        self.current()?;
        if !std::ptr::eq(self.authored, authored)
            || self.values.len() != self.generated_len
            || authored.len().checked_add(self.generated_len) != Some(self.values.capacity())
        {
            poison(&self.budget, 0);
            return Err(());
        }
        Ok(())
    }
    /// Terminal transfer used by teardown and the explicit old allocating
    /// receiver. The owner is consumed, so no receipt can authorize another Vec.
    pub(crate) fn into_vec(self) -> Vec<T> {
        self.values
    }
    /// Caller validates both root owners before beginning either payload clone.
    /// Exact spare capacity already belongs to this structurally affine owner.
    pub(crate) fn finish(mut self, authored: &[T]) -> Result<Vec<T>, ()>
    where
        T: Clone,
    {
        self.validate(authored)?;
        self.values.extend_from_slice(self.authored);
        self.values.rotate_left(self.generated_len);
        Ok(self.values)
    }
}
impl<T> Deref for KeyDirectory<'_, T> {
    type Target = [T];
    fn deref(&self) -> &[T] {
        &self.values
    }
}
fn poison(budget: &Budget, requested: usize) {
    record_refusal(budget, requested);
    budget.overflowed.set(true);
}
fn required(budget: &Budget, bytes: usize) -> Result<(), ()> {
    if reserve(Some(budget), bytes) {
        return Ok(());
    }
    poison(budget, bytes);
    Err(())
}
#[cfg(test)]
mod tests;
