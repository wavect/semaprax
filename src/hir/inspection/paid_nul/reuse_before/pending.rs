//! Same inline32 borrowed LIFO with funded fallible spill; no traversal cap.
use super::text::{physical, reserve};
use super::Diagnostic;
const INLINE: usize = 32;
pub(super) struct Pending<'a, T> {
    inline: [Option<&'a T>; INLINE],
    len: usize,
    spill: Vec<&'a T>,
}
impl<'a, T> Pending<'a, T> {
    pub(super) fn new(root: &'a T) -> Result<Self, Diagnostic> {
        // Modeled inline frame owner, not requested Global heap Layout bytes.
        reserve(std::mem::size_of::<Self>())?;
        let mut pending = Self {
            inline: [None; INLINE],
            len: 0,
            spill: Vec::new(),
        };
        pending.push(root)?;
        Ok(pending)
    }
    pub(super) fn push(&mut self, value: &'a T) -> Result<(), Diagnostic> {
        reserve(0)?;
        if self.len < INLINE && self.spill.is_empty() {
            self.inline[self.len] = Some(value);
            self.len += 1;
        } else {
            if self.spill.len() == self.spill.capacity() {
                let capacity = self
                    .spill
                    .capacity()
                    .checked_mul(2)
                    .ok_or_else(physical)?
                    .max(4);
                let layout = std::alloc::Layout::array::<&T>(capacity).map_err(|_| physical())?;
                // Prepay complete new/replacement backing; old backing remains
                // retained until Global grow settles. No amortized credit.
                reserve(layout.size())?;
                self.spill
                    .try_reserve_exact(capacity - self.spill.len())
                    .map_err(|_| physical())?;
                if self.spill.capacity() != capacity {
                    return Err(physical());
                }
            }
            self.spill.push(value);
        }
        Ok(())
    }
    pub(super) fn extend(
        &mut self,
        values: impl IntoIterator<Item = &'a T>,
    ) -> Result<(), Diagnostic> {
        reserve(0)?;
        for value in values {
            self.push(value)?;
        }
        Ok(())
    }
    pub(super) fn pop(&mut self) -> Result<Option<&'a T>, Diagnostic> {
        reserve(0)?;
        if let Some(value) = self.spill.pop() {
            return Ok(Some(value));
        }
        if self.len == 0 {
            return Ok(None);
        }
        self.len -= 1;
        Ok(self.inline[self.len].take())
    }
    #[cfg(test)]
    pub(super) fn capacity(&self) -> usize {
        self.spill.capacity()
    }
}
