//! Flags-only ownership observations paid by the same active HIR invocation.
use super::*;
use crate::cleanup_plan::ReplayClonePayload;
use std::mem::size_of;

pub(super) struct Payer {
    clone_cursor_paid: bool,
}
impl Payer {
    // Constructed only for one lexical type_flags_paid invocation. The inline
    // payer is itself funded before publication; Storage's containing carrier
    // keeps its unchanged independent reservation when this payer moves in.
    pub(super) fn new() -> Result<Self, Diagnostic> {
        pay(Some(size_of::<Self>()))?;
        Ok(Self {
            clone_cursor_paid: false,
        })
    }
    pub(super) fn authenticate() -> Result<(), Diagnostic> {
        pay(Some(0))
    }
}
fn pay(bytes: Option<usize>) -> Result<(), Diagnostic> {
    let Some(bytes) = bytes else {
        return Err(scope_checkpoint::charge::failure(
            usize::MAX,
            "ownership type flags layout overflowed",
        ));
    };
    if crate::bounded_output::reserve_active_required(bytes) {
        Ok(())
    } else {
        Err(hir_error_static(
            "ownership type flags exceed the active validation budget",
        ))
    }
}
impl TypeFactsPayer for Payer {
    fn bytes(&mut self, bytes: usize) -> Result<(), Diagnostic> {
        pay(Some(bytes))
    }
    fn grow<T>(&mut self, values: &mut Vec<T>, additional: usize) -> Result<(), Diagnostic> {
        Self::authenticate()?;
        let Some(required) = values.len().checked_add(additional) else {
            return pay(None);
        };
        if required <= values.capacity() {
            return Ok(());
        }
        let Some(target) = values
            .capacity()
            .checked_mul(2)
            .map(|n| n.max(4).max(required))
        else {
            return pay(None);
        };
        pay(target.checked_mul(size_of::<T>()))?;
        if values.try_reserve_exact(target - values.len()).is_err() || values.capacity() != target {
            return Err(self.construction_refusal("ownership type flags growth failed"));
        }
        Ok(())
    }
    fn node<K, V>(&mut self) -> Result<(), Diagnostic> {
        pay(crate::cleanup_plan::replay_btree_node_upper::<K, V>())
    }
    fn clone_type(&mut self, ty: &ResolvedType) -> Result<ResolvedType, Diagnostic> {
        Self::authenticate()?;
        if !self.clone_cursor_paid {
            pay(Some(size_of::<[Option<(&ResolvedType, usize)>; 514]>()))?;
            self.clone_cursor_paid = true;
        }
        pay(ty.clone_payload_bytes())?;
        Ok(ty.clone())
    }
    fn boxed_type(&mut self, ty: ResolvedType) -> Result<Box<ResolvedType>, Diagnostic> {
        pay(Some(size_of::<ResolvedType>()))?;
        Ok(Box::new(ty))
    }
    fn construction_refusal(&mut self, message: &'static str) -> Diagnostic {
        scope_checkpoint::charge::failure(usize::MAX, message)
    }
}

#[cfg(test)]
mod before;
#[cfg(test)]
mod tests;
#[cfg(test)]
thread_local! { static ORIGINAL: std::cell::Cell<bool> = const { std::cell::Cell::new(false) }; }
#[cfg(test)]
pub(super) fn original_selected() -> bool {
    ORIGINAL.with(std::cell::Cell::get)
}
#[cfg(test)]
pub(super) struct OriginalGuard(bool);
#[cfg(test)]
impl Drop for OriginalGuard {
    fn drop(&mut self) {
        ORIGINAL.with(|selected| selected.set(self.0));
    }
}
#[cfg(test)]
pub(super) fn original_guard() -> OriginalGuard {
    OriginalGuard(ORIGINAL.with(|selected| selected.replace(true)))
}
