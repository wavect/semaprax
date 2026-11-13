//! Closed whole-view registry receiver. No source installer is granted here.
use super::{authority::LoanCapacity, ContextSlot, InvocationRuntime, Owner, Registry, State};
use crate::construction_fusion::domain::{owner_core::Core, Issuer, ModuleIdentity, Refusal};
use std::alloc::Layout;
impl<R: InvocationRuntime> Registry<R> {
    pub(crate) fn hold_whole(
        &mut self,
        owner: R::Owner,
        capacity: &LoanCapacity<'_>,
    ) -> Result<Owner, (Refusal, R::Owner)> {
        let preparation = (|| {
            self.active()?;
            if R::bytes(&owner).len() > crate::byte_ops::MAX_OWNED_BYTE_VALUE_BYTES as usize {
                return Err(Refusal::Range);
            }
            let replacement = if self.contexts.len() == self.contexts.capacity() {
                Some(
                    self.contexts
                        .capacity()
                        .checked_mul(2)
                        .ok_or(Refusal::Metadata)?
                        .max(1),
                )
            } else {
                None
            };
            let mut runtime = self.runtime.held_account();
            if !self.header_paid {
                runtime.reserve_metadata(Layout::new::<Self>())?;
            }
            if let Some(count) = replacement {
                runtime.reserve_metadata(Layout::new::<Vec<ContextSlot<R>>>())?;
                runtime.reserve_metadata(
                    Layout::array::<ContextSlot<R>>(count).map_err(|_| Refusal::Metadata)?,
                )?;
            }
            runtime.reserve_metadata(Layout::new::<State<R>>())?;
            Ok((runtime, replacement))
        })();
        let (runtime, replacement) = match preparation {
            Ok(value) => value,
            Err(error) => {
                let _ = self.select::<()>(Err(error));
                return Err((error, owner));
            }
        };
        let (core, children) = match Core::hold_whole_ordinary(
            owner,
            Issuer::new(),
            ModuleIdentity::new(),
            runtime,
            capacity,
        ) {
            Ok(value) => value,
            Err((error, owner)) => {
                let _ = self.select::<()>(Err(error));
                return Err((error, owner));
            }
        };
        if let Some(count) = replacement {
            let mut directory = Vec::new();
            directory
                .try_reserve_exact(count)
                .expect("whole registry allocation invariant failure");
            assert_eq!(directory.capacity(), count);
            directory.append(&mut self.contexts);
            self.contexts = directory;
        }
        let context = self.contexts.len();
        let receipt = children[0].receipt();
        self.contexts.push(Some(Box::new(State {
            core,
            children,
            table_slots: 0,
        })));
        self.header_paid = true;
        Ok(Owner {
            invocation: self.invocation,
            context,
            child: 0,
            receipt,
        })
    }
    pub(crate) fn take_whole(&mut self, owner: &mut Owner) -> Result<R::Owner, Refusal> {
        self.apply(owner, false, |core, child| core.take_whole(child))
    }
}

#[cfg(test)]
mod tests;
