//! Explicit caller-payer flag receiver; complete original owned classifier stays
//! independent. No cache admission, quota, or new phase allowance is introduced.
use super::{DeclarationIndex, ResolvedType, TypeFlags};
use crate::diagnostic::Diagnostic;
use std::mem::size_of;
mod admission;
mod builtin;
mod compute;
mod key;
#[path = "../nominal_paid_facts.rs"]
mod nominal_constructor;
mod storage;
mod substitute;
pub(crate) use storage::Payer as TypeFactsPayer;
use storage::Storage;

impl DeclarationIndex {
    pub(crate) fn type_flags_paid<P: TypeFactsPayer>(
        &self,
        ty: &ResolvedType,
        mut payer: P,
    ) -> Result<Option<TypeFlags>, Diagnostic> {
        // Authentication/sticky first-refusal still runs even when this query
        // needs no classifier Storage carrier. No allocation or allowance.
        payer.bytes(0)?;
        #[cfg(test)]
        payer.oracle_work(0, "classifier fast receiver authority")?;
        if matches!(
            ty,
            ResolvedType::Never
                | ResolvedType::Unit
                | ResolvedType::I64
                | ResolvedType::I32
                | ResolvedType::Char
                | ResolvedType::U8
                | ResolvedType::Usize
                | ResolvedType::F32
                | ResolvedType::F64
                | ResolvedType::Bool
                | ResolvedType::String
                | ResolvedType::Bytes
                | ResolvedType::Str
                | ResolvedType::SliceU8
                | ResolvedType::StringMap
                | ResolvedType::OnceFunction
                | ResolvedType::OnceFunctionI64
                | ResolvedType::OnceFunctionI64Pair
                | ResolvedType::MutFunctionI64
        ) {
            return Ok(self.type_flags(ty));
        }
        if let Some(Some(facts)) = super::cached_facts(self, ty) {
            return Ok(Some(TypeFlags::from(facts)));
        }
        self.type_flags_paid_construct(ty, payer)
    }

    /// Invocation-local lazy state: authenticates the same caller before the
    /// complete fast observations, and constructs its paid carrier only for
    /// the existing fallback. The ordinary payer receiver remains independent.
    pub(crate) fn type_flags_paid_lazy<P: TypeFactsPayer>(
        &self,
        ty: &ResolvedType,
        authenticate: impl FnOnce() -> Result<(), Diagnostic>,
        make_payer: impl FnOnce() -> Result<P, Diagnostic>,
    ) -> Result<Option<TypeFlags>, Diagnostic> {
        authenticate()?;
        if matches!(
            ty,
            ResolvedType::Never
                | ResolvedType::Unit
                | ResolvedType::I64
                | ResolvedType::I32
                | ResolvedType::Char
                | ResolvedType::U8
                | ResolvedType::Usize
                | ResolvedType::F32
                | ResolvedType::F64
                | ResolvedType::Bool
                | ResolvedType::String
                | ResolvedType::Bytes
                | ResolvedType::Str
                | ResolvedType::SliceU8
                | ResolvedType::StringMap
                | ResolvedType::OnceFunction
                | ResolvedType::OnceFunctionI64
                | ResolvedType::OnceFunctionI64Pair
                | ResolvedType::MutFunctionI64
        ) {
            return Ok(self.type_flags(ty));
        }
        if let Some(Some(facts)) = super::cached_facts(self, ty) {
            return Ok(Some(TypeFlags::from(facts)));
        }
        self.type_flags_paid_construct(ty, make_payer()?)
    }

    // Keeping this constructor out of the fast receiving frame ensures its
    // complete Storage<P> object is initialized only for a real miss/fallback.
    #[inline(never)]
    pub(crate) fn type_flags_paid_construct<P: TypeFactsPayer>(
        &self,
        ty: &ResolvedType,
        mut payer: P,
    ) -> Result<Option<TypeFlags>, Diagnostic> {
        payer.bytes(size_of::<Storage<P>>())?;
        let mut storage = Storage { payer, first: None };
        let result = self.flags_paid(ty, &mut storage);
        match storage.first {
            Some(error) => Err(error),
            None => Ok(result),
        }
    }

    #[cfg(test)]
    pub(crate) fn type_flags_paid_original_receiver<P: TypeFactsPayer>(
        &self,
        ty: &ResolvedType,
        mut payer: P,
    ) -> Result<Option<TypeFlags>, Diagnostic> {
        payer.bytes(size_of::<Storage<P>>())?;
        let mut storage = Storage { payer, first: None };
        let result = self.flags_paid(ty, &mut storage);
        match storage.first {
            Some(error) => Err(error),
            None => Ok(result),
        }
    }
    // Same complete classifier substitution, also usable by outer cleanup walks
    // that already discard the inner semantic Diagnostic. None is semantic;
    // the original payer's first physical refusal stays a held Diagnostic.
    pub(crate) fn substitute_type_paid<P: TypeFactsPayer>(
        &self,
        ty: &ResolvedType,
        owner: &crate::hir::DeclarationId,
        arguments: &[ResolvedType],
        mut payer: P,
    ) -> Result<Option<ResolvedType>, Diagnostic> {
        payer.bytes(size_of::<Storage<P>>())?;
        let mut storage = Storage { payer, first: None };
        let value = substitute::resolve(ty, owner, arguments, &mut storage);
        match storage.first {
            Some(error) => Err(error),
            None => Ok(value),
        }
    }

    fn flags_paid<P: TypeFactsPayer>(
        &self,
        ty: &ResolvedType,
        storage: &mut Storage<P>,
    ) -> Option<TypeFlags> {
        // The existing leaf receiver already removes all discarded key/layout
        // storage on both cache hit and miss. Keep that exact zero-allocation
        // path; paying a reconstructed leaf TypeFacts would add work/storage.
        if matches!(
            ty,
            ResolvedType::Never
                | ResolvedType::Unit
                | ResolvedType::I64
                | ResolvedType::I32
                | ResolvedType::Char
                | ResolvedType::U8
                | ResolvedType::Usize
                | ResolvedType::F32
                | ResolvedType::F64
                | ResolvedType::Bool
                | ResolvedType::String
                | ResolvedType::Bytes
                | ResolvedType::Str
                | ResolvedType::SliceU8
                | ResolvedType::StringMap
                | ResolvedType::OnceFunction
                | ResolvedType::OnceFunctionI64
                | ResolvedType::OnceFunctionI64Pair
                | ResolvedType::MutFunctionI64
        ) {
            return self.type_flags(ty);
        }
        // Preserve the exact current cache precedence, even for forged facts.
        // Declined bounded census keeps the complete owned-key fallback, paid
        // before construction; an exact miss is never confused with decline.
        let cached = match ty {
            ResolvedType::Nominal {
                declaration,
                arguments,
            } if arguments.is_empty() => {
                super::nominal_key::get(&self.type_facts_by_id, declaration)
            }
            _ => match super::nominal_key::compound_get(&self.type_facts_by_id, ty) {
                Some(facts) => facts,
                None => {
                    let key = key::identity(ty, storage)?;
                    self.type_facts_by_id.get(&key)
                }
            },
        };
        if let Some(facts) = cached {
            return Some(TypeFlags::from(facts));
        }
        compute::compute(self, ty, storage)
            .as_ref()
            .map(TypeFlags::from)
    }
}

#[cfg(test)]
impl DeclarationIndex {
    pub(crate) fn classifier_uncached_for_test(&self) -> Self {
        let mut index = self.clone();
        index.type_facts_by_id.clear();
        index
    }
    pub(crate) fn classifier_old_facts_for_test(
        &self,
        ty: &ResolvedType,
    ) -> Option<crate::hir::TypeFacts> {
        self.recompute_type_facts(ty)
    }
    pub(crate) fn classifier_compute_for_test<P: TypeFactsPayer>(
        &self,
        ty: &ResolvedType,
        mut payer: P,
    ) -> Result<Option<crate::hir::TypeFacts>, Diagnostic> {
        payer.bytes(size_of::<Storage<P>>())?;
        let mut storage = Storage { payer, first: None };
        let facts = compute::compute(self, ty, &mut storage);
        match storage.first {
            Some(error) => Err(error),
            None => Ok(facts),
        }
    }
    pub(crate) fn classifier_cache_for_test(
        &mut self,
        ty: &ResolvedType,
        facts: crate::hir::TypeFacts,
    ) {
        self.type_facts_by_id.insert(ty.identity_key(), facts);
    }
    pub(crate) fn classifier_key_for_test<P: TypeFactsPayer>(
        &self,
        ty: &ResolvedType,
        mut payer: P,
    ) -> Result<Option<String>, Diagnostic> {
        payer.bytes(size_of::<Storage<P>>())?;
        let mut storage = Storage { payer, first: None };
        let value = key::identity(ty, &mut storage);
        match storage.first {
            Some(error) => Err(error),
            None => Ok(value),
        }
    }
    pub(crate) fn classifier_substitute_for_test<P: TypeFactsPayer>(
        &self,
        ty: &ResolvedType,
        owner: &crate::hir::DeclarationId,
        arguments: &[ResolvedType],
        mut payer: P,
    ) -> Result<Option<ResolvedType>, Diagnostic> {
        payer.bytes(size_of::<Storage<P>>())?;
        let mut storage = Storage { payer, first: None };
        let value = substitute::resolve(ty, owner, arguments, &mut storage);
        match storage.first {
            Some(error) => Err(error),
            None => Ok(value),
        }
    }
    pub(crate) fn classifier_admission_for_test(&self, ty: &ResolvedType) -> (bool, bool) {
        (
            admission::native_owned(self, ty),
            admission::authored_byte_variant(self, ty),
        )
    }
}

mod inventory_constructor;
pub(crate) use inventory_constructor::{
    identity as inventory_identity, substitute as inventory_substitute, InventoryPayer,
};
