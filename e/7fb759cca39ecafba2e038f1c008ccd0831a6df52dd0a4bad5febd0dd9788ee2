//! Immutable nominal sequences. Backing reuse is storage, never type authority.
use super::{
    ResolvedFieldDeclaration, ResolvedType, ResolvedTypeParameterDeclaration,
    ResolvedVariantCaseDeclaration,
};
use std::{
    alloc::Layout,
    fmt,
    mem::size_of,
    ops::{Deref, DerefMut},
    sync::Arc,
};
mod codec;
mod elements;
pub(crate) mod map;

/// Declaration elements whose recursive types have an iterative disposer.
#[doc(hidden)]
mod sealed {
    pub trait Sealed {}
}
impl sealed::Sealed for ResolvedFieldDeclaration {}
impl sealed::Sealed for ResolvedVariantCaseDeclaration {}
impl sealed::Sealed for ResolvedTypeParameterDeclaration {}
pub trait NominalElement: sealed::Sealed + Clone + fmt::Debug + Eq {
    fn drop_capacity<E>(
        &self,
        reserve: &mut impl FnMut(usize) -> Result<(), E>,
    ) -> Result<usize, E>;
    fn drain_types(&mut self, dispose: &mut impl FnMut(ResolvedType));
    fn clone_bytes(&self) -> Option<usize>;
    /// Stack-safe compatibility cloning for implicit legacy COW only.
    #[doc(hidden)]
    fn clone_legacy_unfunded(&self) -> Self {
        self.clone()
    }
}
#[derive(Clone, Copy, Eq, PartialEq)]
enum OwnerMode {
    Funded,
    LegacyUnfunded,
}
struct Owner<T: NominalElement> {
    mode: OwnerMode,
    values: Vec<T>,
    scratch: Vec<ResolvedType>,
}
impl<T: NominalElement> Owner<T> {
    fn drain(&mut self, dispose: &mut impl FnMut(ResolvedType)) {
        for value in &mut self.values {
            value.drain_types(dispose);
        }
    }
}
impl<T: NominalElement> Drop for Owner<T> {
    fn drop(&mut self) {
        let scratch = &mut self.scratch;
        for value in &mut self.values {
            value.drain_types(&mut |ty| dispose(ty, scratch));
        }
    }
}
/// An owning legacy Vec or one acquired immutable owner. Equality/Debug/wire
/// inspect complete ordered values, never addresses or representation tags.
/// `From<Vec<_>>` moves legacy storage without an Arc allocation or new payer.
/// New admitted producers must instead use `from_funded_vec`.
pub struct NominalPayload<T: NominalElement> {
    storage: Storage<T>,
}
#[derive(Debug, Eq, PartialEq)]
pub enum NominalMutationError<E> {
    Reservation(E),
    Capacity,
}
enum Storage<T: NominalElement> {
    Owned(Vec<T>),
    Shared(Arc<Owner<T>>),
}
impl<T: NominalElement> NominalPayload<T> {
    pub fn as_slice(&self) -> &[T] {
        self.deref().as_slice()
    }
    /// Vec backing/payload must already belong to the caller. Refusal leaves
    /// that complete Vec untouched; only successful acquisition moves it. This acquires
    /// census scratch, last-owner disposal scratch, Arc backing, initializer,
    /// and the receiving carrier before each corresponding allocation.
    pub fn from_funded_vec<E>(
        values: &mut Vec<T>,
        mut reserve: impl FnMut(usize) -> Result<(), E>,
    ) -> Result<Self, E> {
        let mut capacity = 0;
        for value in values.iter() {
            capacity = capacity.max(value.drop_capacity(&mut reserve)?);
        }
        #[repr(C)]
        struct ArcCarrier<T> {
            strong: usize,
            weak: usize,
            value: T,
        }
        let backing = Layout::new::<ArcCarrier<Owner<T>>>().size();
        let bytes = backing
            .saturating_add(size_of::<Owner<T>>())
            .saturating_add(size_of::<Self>())
            .saturating_add(size_of::<Result<Self, E>>())
            .saturating_add(capacity.saturating_mul(size_of::<ResolvedType>()));
        reserve(bytes)?;
        let scratch = Vec::with_capacity(capacity);
        assert_eq!(
            scratch.capacity(),
            capacity,
            "nominal exact disposal backing"
        );
        Ok(Self {
            storage: Storage::Shared(Arc::new(Owner {
                mode: OwnerMode::Funded,
                values: std::mem::take(values),
                scratch,
            })),
        })
    }
    /// Explicit paid copy before mutation. Refusal leaves both old owners and
    /// their complete values unchanged; no implicit Cow/Arc::make_mut exists.
    pub fn try_make_mut<E>(
        &mut self,
        mut reserve: impl FnMut(usize) -> Result<(), E>,
    ) -> Result<&mut Vec<T>, NominalMutationError<E>> {
        if matches!(&self.storage, Storage::Shared(_)) {
            // The existing iterative clone census uses this actual fixed
            // cursor; acquire its stack carrier before invoking it.
            reserve(size_of::<[Option<(&ResolvedType, usize)>; 514]>())
                .map_err(NominalMutationError::Reservation)?;
            let bytes = self.len().checked_mul(size_of::<T>()).and_then(|initial| {
                self.iter()
                    .try_fold(initial, |n, v| n.checked_add(v.clone_bytes()?))
            });
            let bytes = bytes
                .and_then(|bytes| size_of::<Vec<T>>().checked_add(bytes))
                .ok_or(NominalMutationError::Capacity)?;
            reserve(bytes).map_err(NominalMutationError::Reservation)?;
            let mut values = Vec::with_capacity(self.len());
            assert_eq!(
                values.capacity(),
                self.len(),
                "nominal exact materialization backing"
            );
            values.extend(self.iter().cloned());
            self.storage = Storage::Owned(values);
        } else {
            reserve(0).map_err(NominalMutationError::Reservation)?;
        }
        Ok(self.deref_mut())
    }
    /// Moves recursive roots only from the last owner into an already-paid
    /// external disposal machine. Other immutable owners keep the full value.
    #[doc(hidden)]
    pub fn drain_recursive_types(mut self, mut dispose: impl FnMut(ResolvedType)) {
        match &mut self.storage {
            Storage::Owned(values) => {
                for value in values {
                    value.drain_types(&mut dispose);
                }
            }
            Storage::Shared(owner) => {
                if let Some(owner) = Arc::get_mut(owner) {
                    owner.drain(&mut dispose);
                }
            }
        }
    }
    /// Paid provenance only. Physical legacy sharing never satisfies this.
    pub(crate) fn is_shared(&self) -> bool {
        matches!(&self.storage, Storage::Shared(owner) if owner.mode == OwnerMode::Funded)
    }
    pub(crate) fn is_legacy_shared(&self) -> bool {
        matches!(&self.storage, Storage::Shared(owner) if owner.mode == OwnerMode::LegacyUnfunded)
    }
    pub(crate) fn has_shared_owner(&self) -> bool {
        matches!(&self.storage, Storage::Shared(_))
    }
    #[cfg(test)]
    pub(crate) fn same_owner(&self, other: &Self) -> bool {
        matches!((&self.storage, &other.storage), (Storage::Shared(a), Storage::Shared(b)) if Arc::ptr_eq(a,b))
    }
}
impl<T: NominalElement> From<Vec<T>> for NominalPayload<T> {
    fn from(values: Vec<T>) -> Self {
        Self {
            storage: Storage::Owned(values),
        }
    }
}
impl<T: NominalElement> FromIterator<T> for NominalPayload<T> {
    fn from_iter<I: IntoIterator<Item = T>>(values: I) -> Self {
        values.into_iter().collect::<Vec<T>>().into()
    }
}
impl<T: NominalElement> Default for NominalPayload<T> {
    fn default() -> Self {
        Vec::new().into()
    }
}
impl<T: NominalElement> Clone for NominalPayload<T> {
    fn clone(&self) -> Self {
        match &self.storage {
            Storage::Owned(v) => v.clone().into(),
            Storage::Shared(v) => Self {
                storage: Storage::Shared(v.clone()),
            },
        }
    }
}
impl<T: NominalElement> Deref for NominalPayload<T> {
    type Target = Vec<T>;
    fn deref(&self) -> &Vec<T> {
        match &self.storage {
            Storage::Owned(v) => v,
            Storage::Shared(v) => &v.values,
        }
    }
}
impl<T: NominalElement> DerefMut for NominalPayload<T> {
    fn deref_mut(&mut self) -> &mut Vec<T> {
        // A legacy mutation always leaves shared precomputed disposal scratch.
        // The detached Owned Vec uses its original iterative fallback instead.
        if self.is_legacy_shared() {
            self.detach_legacy_unfunded();
        }
        match &mut self.storage {
            Storage::Owned(v) => v,
            Storage::Shared(_) => panic!("shared nominal mutation requires try_make_mut"),
        }
    }
}
impl<T: NominalElement> fmt::Debug for NominalPayload<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.deref().fmt(f)
    }
}
impl<T: NominalElement> PartialEq for NominalPayload<T> {
    fn eq(&self, other: &Self) -> bool {
        self.as_slice() == other.as_slice()
    }
}
impl<T: NominalElement> Eq for NominalPayload<T> {}
impl<T: NominalElement> PartialEq<Vec<T>> for NominalPayload<T> {
    fn eq(&self, other: &Vec<T>) -> bool {
        self.as_slice() == other.as_slice()
    }
}
impl<'a, T: NominalElement> IntoIterator for &'a NominalPayload<T> {
    type Item = &'a T;
    type IntoIter = std::slice::Iter<'a, T>;
    fn into_iter(self) -> Self::IntoIter {
        self.iter()
    }
}
impl<'a, T: NominalElement> IntoIterator for &'a mut NominalPayload<T> {
    type Item = &'a mut T;
    type IntoIter = std::slice::IterMut<'a, T>;
    fn into_iter(self) -> Self::IntoIter {
        self.deref_mut().iter_mut()
    }
}

// This is the old public fallback disposal policy for legacy Vec storage.
// Funded shared owners use their preacquired scratch and never grow on Drop.
impl<T: NominalElement> Drop for NominalPayload<T> {
    fn drop(&mut self) {
        if let Storage::Owned(values) = &mut self.storage {
            let mut pending = Vec::new();
            for value in values {
                value.drain_types(&mut |ty| match ty {
                    ResolvedType::Nominal { arguments, .. } => pending.extend(arguments),
                    ResolvedType::Function { parameters, result } => {
                        pending.extend(parameters);
                        pending.push(*result);
                    }
                    _ => {}
                });
            }
            while let Some(ty) = pending.pop() {
                match ty {
                    ResolvedType::Nominal { arguments, .. } => pending.extend(arguments),
                    ResolvedType::Function { parameters, result } => {
                        pending.extend(parameters);
                        pending.push(*result);
                    }
                    _ => {}
                }
            }
        }
    }
}
pub(crate) fn dispose(ty: ResolvedType, scratch: &mut Vec<ResolvedType>) {
    fn children(ty: ResolvedType, scratch: &mut Vec<ResolvedType>) {
        match ty {
            ResolvedType::Nominal { arguments, .. } => {
                for ty in arguments {
                    assert!(scratch.len() < scratch.capacity());
                    scratch.push(ty);
                }
            }
            ResolvedType::Function { parameters, result } => {
                for ty in parameters {
                    assert!(scratch.len() < scratch.capacity());
                    scratch.push(ty);
                }
                assert!(scratch.len() < scratch.capacity());
                scratch.push(*result);
            }
            _ => {}
        }
    }
    children(ty, scratch);
    while let Some(ty) = scratch.pop() {
        children(ty, scratch);
    }
}
#[cfg(test)]
mod tests;

mod legacy;

pub(super) fn legacy_fields_equal(
    left: &[ResolvedFieldDeclaration],
    right: &[ResolvedFieldDeclaration],
) -> bool {
    legacy::clone::fields_equal(left, right)
}
