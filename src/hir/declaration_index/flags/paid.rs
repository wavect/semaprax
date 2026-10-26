//! Explicit caller-payer flag receiver; complete original owned classifier stays
//! independent. No cache admission, quota, or new phase allowance is introduced.
use super::{DeclarationIndex, ResolvedType, TypeFlags};
use crate::diagnostic::Diagnostic;
use std::mem::size_of;
mod admission;
mod builtin;
#[cfg_attr(
    not(test),
    expect(
        dead_code,
        reason = "Preserve the complete independent owned classifier in production builds."
    )
)]
mod compute;
mod query {
    //! Faithful independent physical counterpart of the original owned classifier.
    use super::super::super::{
        admitted_owned_byte_prelude_instance, owned_builtin, DeclarationId, DeclarationIndex,
        DeclarationKind, ResolvedType,
    };
    use super::super::TypeFlags;
    use super::storage::{Payer, Storage};
    use super::{admission, builtin, key};
    use std::collections::{BTreeMap, BTreeSet};

    fn children<'a, P: Payer>(
        types: impl Iterator<Item = &'a ResolvedType>,
        owner: &DeclarationId,
        arguments: &[ResolvedType],
        storage: &mut Storage<P>,
    ) -> Option<Vec<ResolvedType>> {
        storage.bytes(std::mem::size_of::<Vec<ResolvedType>>())?;
        let mut children = Vec::new();
        for ty in types {
            #[cfg(test)]
            storage.oracle_work(1, "classifier declared field receive")?;
            let child = storage.substitute(ty, owner, arguments)?;
            storage.push(&mut children, child)?;
        }
        Some(children)
    }

    pub(super) fn compute<P: Payer>(
        index: &DeclarationIndex,
        ty: &ResolvedType,
        storage: &mut Storage<P>,
    ) -> Option<TypeFlags> {
        storage.bytes(
            std::mem::size_of::<BTreeSet<String>>()
                + std::mem::size_of::<BTreeMap<String, TypeFlags>>()
                + 2 * std::mem::size_of::<Vec<()>>(),
        )?;
        index.compute_paid_flag_values(ty, &mut BTreeSet::new(), &mut BTreeMap::new(), storage)
    }
    impl DeclarationIndex {
        fn compute_paid_flag_values<P: Payer>(
            &self,
            ty: &ResolvedType,
            visiting: &mut BTreeSet<String>,
            memo: &mut BTreeMap<String, TypeFlags>,
            storage: &mut Storage<P>,
        ) -> Option<TypeFlags> {
            enum Frame {
                Enter(ResolvedType),
                Finish {
                    identity: String,
                    declaration: DeclarationId,
                    kind: DeclarationKind,
                    child_count: usize,
                },
            }

            let mut frames = Vec::new();
            let root = storage.clone_type(ty)?;
            storage.push(&mut frames, Frame::Enter(root))?;
            let mut results = Vec::<TypeFlags>::new();
            while let Some(frame) = frames.pop() {
                #[cfg(test)]
                storage.oracle_work(1, "classifier compute frame")?;
                match frame {
                    Frame::Enter(ty) => {
                        let identity = key::identity(&ty, storage)?;

                        if let Some(facts) = memo.get(&identity) {
                            let facts = *facts;
                            storage.push(&mut results, facts)?;
                            continue;
                        }
                        if crate::map_ops::is_typed_collection(&ty) {
                            storage.grow(&mut results, 1)?;
                            results.push(TypeFlags {
                                copy: false,
                                contains_resource: false,
                                sized: true,
                                needs_drop: true,
                            });
                            continue;
                        }
                        let scalar = match &ty {
                            ResolvedType::Function { .. } => {
                                if !crate::hir::function_value::is_signature(&ty) {
                                    return None;
                                }
                                storage.grow(&mut results, 1)?;
                                results.push(TypeFlags {
                                    copy: true,
                                    contains_resource: false,
                                    sized: true,
                                    needs_drop: false,
                                });
                                continue;
                            }
                            ResolvedType::Never => None,
                            ResolvedType::Unit => {
                                Some((true, false, false, "native-rust-import-result:unit"))
                            }
                            ResolvedType::I64 => Some((true, false, false, "scalar:i64")),
                            ResolvedType::I32 => Some((true, false, false, "scalar:i32")),
                            ResolvedType::Char => Some((true, false, false, "scalar:char")),
                            ResolvedType::U8 => Some((true, false, false, "scalar:u8")),
                            ResolvedType::Usize => Some((true, false, false, "scalar:usize")),
                            ResolvedType::ArrayU8(_) => None,
                            ResolvedType::F32 => Some((true, false, false, "scalar:f32")),
                            ResolvedType::F64 => Some((true, false, false, "scalar:f64")),
                            ResolvedType::Bool => Some((true, false, false, "scalar:bool")),
                            ResolvedType::String => Some((false, false, true, "owned:string")),
                            ResolvedType::Bytes => Some((false, false, true, "owned:bytes")),
                            ResolvedType::OnceFunctionI64 => {
                                Some((false, false, true, "owned:fn-once:bytes+i64:i64:v2"))
                            }
                            ResolvedType::MutFunctionI64 => {
                                Some((false, false, false, "owned:fn-mut:i64+i64:i64:v1"))
                            }
                            ResolvedType::OnceFunctionI64Pair => {
                                Some((false, false, true, "owned:fn-once:bytes+i64+i64:i64:v3"))
                            }
                            ResolvedType::OnceFunction => {
                                Some((false, false, true, "owned:fn-once:bytes:i64:v1"))
                            }
                            ResolvedType::Str => Some((false, false, false, "borrowed:str")),
                            ResolvedType::SliceU8 => {
                                Some((false, false, false, "borrowed:slice-u8"))
                            }
                            ResolvedType::StringMap => {
                                Some((false, false, true, "owned:map:string:i64"))
                            }
                            ResolvedType::TypeParameter { .. } | ResolvedType::Nominal { .. } => {
                                None
                            }
                        };
                        if let ResolvedType::ArrayU8(_length) = &ty {
                            storage.grow(&mut results, 1)?;
                            results.push(TypeFlags {
                                copy: true,
                                contains_resource: false,
                                sized: true,
                                needs_drop: false,
                            });
                            continue;
                        }
                        if let Some((copy, contains_resource, needs_drop, _key)) = scalar {
                            storage.grow(&mut results, 1)?;
                            results.push(TypeFlags {
                                copy,
                                contains_resource,
                                sized: true,
                                needs_drop,
                            });
                            continue;
                        }
                        let native_owned_container = admission::native_owned(self, &ty)
                            || owned_builtin::resource_result_shape(self, &ty);
                        let authored_owned_byte_variant =
                            admission::authored_byte_variant(self, &ty);
                        let ResolvedType::Nominal {
                            declaration,
                            arguments,
                        } = ty
                        else {
                            return None;
                        };
                        let item = self.declaration(&declaration)?;
                        if let Some(full_facts) =
                            builtin::facts(self, &declaration, &arguments, storage)
                        {
                            let facts = TypeFlags::from(&full_facts);
                            if !memo.contains_key(&identity) {
                                storage.node::<String, TypeFlags>()?;
                            }
                            let retained = facts;
                            memo.insert(identity, retained);
                            storage.push(&mut results, facts)?;
                            continue;
                        }
                        if storage.first.is_some() {
                            return None;
                        }
                        if item.kind == DeclarationKind::Resource && arguments.is_empty() {
                            let facts = TypeFlags {
                                copy: false,
                                contains_resource: true,
                                sized: true,
                                needs_drop: true,
                            };
                            if !memo.contains_key(&identity) {
                                storage.node::<String, TypeFlags>()?;
                            }
                            let retained = facts;
                            memo.insert(identity, retained);
                            storage.push(&mut results, facts)?;
                            continue;
                        }
                        if !matches!(
                            item.kind,
                            DeclarationKind::Record
                                | DeclarationKind::Class
                                | DeclarationKind::Variant
                        ) {
                            return None;
                        }
                        let parameters = self.type_parameters.get(&declaration)?;
                        let compiler_byte_option = declaration.as_str()
                            == crate::prelude::OPTION_ID
                            && arguments.as_slice() == [ResolvedType::U8];
                        let owned_byte_variant =
                            admitted_owned_byte_prelude_instance(&declaration, &arguments)
                                || authored_owned_byte_variant;
                        if arguments.len() != parameters.len()
                            || (!compiler_byte_option
                                && !owned_byte_variant
                                && !native_owned_container
                                && item.kind != DeclarationKind::Record
                                && arguments.iter().any(|argument| {
                                    !matches!(argument, ResolvedType::I64 | ResolvedType::Bool)
                                }))
                        {
                            return None;
                        }
                        let visiting_key = storage.literal(&identity)?;
                        if !visiting.contains(&identity) {
                            storage.node::<String, ()>()?;
                        }
                        if !visiting.insert(visiting_key) {
                            return None;
                        }
                        let children = match item.kind {
                            DeclarationKind::Record | DeclarationKind::Class => children(
                                self.record_fields
                                    .get(&declaration)?
                                    .iter()
                                    .map(|field| &field.ty),
                                &declaration,
                                &arguments,
                                storage,
                            )?,
                            DeclarationKind::Variant => children(
                                self.variant_cases
                                    .get(&declaration)?
                                    .iter()
                                    .flat_map(|case| &case.fields)
                                    .map(|field| &field.ty),
                                &declaration,
                                &arguments,
                                storage,
                            )?,
                            _ => unreachable!(),
                        };

                        storage.grow(&mut frames, children.len().checked_add(1)?)?;
                        frames.push(Frame::Finish {
                            identity,
                            declaration,
                            kind: item.kind,
                            child_count: children.len(),
                        });
                        frames.extend(children.into_iter().rev().map(Frame::Enter));
                    }
                    Frame::Finish {
                        identity,
                        declaration,
                        kind,
                        child_count,
                    } => {
                        let split = results.len().checked_sub(child_count)?;
                        let child_facts = &results[split..];

                        visiting.remove(&identity);

                        match kind {
                            DeclarationKind::Record | DeclarationKind::Class => {
                                let fields = self.record_fields.get(&declaration)?;
                                let mut copy = true;
                                let mut contains_resource = false;
                                let mut sized = true;
                                let mut needs_drop = false;
                                for (_field, facts) in fields.iter().zip(child_facts) {
                                    #[cfg(test)]
                                    storage.oracle_work(1, "classifier declared field receive")?;
                                    copy &= facts.copy;
                                    contains_resource |= facts.contains_resource;
                                    sized &= facts.sized;
                                    needs_drop |= facts.needs_drop;
                                }

                                let facts = TypeFlags {
                                    copy,
                                    contains_resource,
                                    sized,
                                    needs_drop,
                                };

                                if !memo.contains_key(&identity) {
                                    storage.node::<String, TypeFlags>()?;
                                }
                                let retained = facts;
                                memo.insert(identity, retained);
                                results.truncate(split);
                                storage.push(&mut results, facts)?;
                            }
                            DeclarationKind::Variant => {
                                let cases = self.variant_cases.get(&declaration)?;
                                let mut facts_iter = child_facts.iter();
                                let mut copy = true;
                                let mut contains_resource = false;
                                let mut sized = true;
                                let mut needs_drop = false;
                                for case in cases {
                                    #[cfg(test)]
                                    storage.oracle_work(1, "classifier declared field receive")?;

                                    for _field in &case.fields {
                                        #[cfg(test)]
                                        storage
                                            .oracle_work(1, "classifier declared field receive")?;
                                        let facts = facts_iter.next()?;
                                        copy &= facts.copy;
                                        contains_resource |= facts.contains_resource;
                                        sized &= facts.sized;
                                        needs_drop |= facts.needs_drop;
                                    }
                                }

                                let facts = TypeFlags {
                                    copy,
                                    contains_resource,
                                    sized,
                                    needs_drop,
                                };

                                if !memo.contains_key(&identity) {
                                    storage.node::<String, TypeFlags>()?;
                                }
                                let retained = facts;
                                memo.insert(identity, retained);
                                results.truncate(split);
                                storage.push(&mut results, facts)?;
                            }
                            _ => unreachable!(),
                        }
                    }
                }
            }
            (results.len() == 1).then(|| results.pop().expect("type fact count checked above"))
        }
    }
}
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
        let result = self.flags_paid_original(ty, &mut storage);
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
        // Complete childless builtin facts do not need a root clone, memo or
        // traversal. Admission remains the existing full builtin receiver.
        if let ResolvedType::Nominal {
            declaration,
            arguments,
        } = ty
        {
            if matches!(
                declaration.as_str(),
                crate::stdin_stream_ops::READER_ID
                    | crate::iterator_ops::ITER_ID
                    | crate::iterator_ops::STEP_ID
                    | crate::prelude::VEC_ID
                    | crate::prelude::BOX_ID
            ) {
                self.declaration(declaration)?;
                // Fund the real complete result, its receiving value and the
                // projected flags before the original builtin constructor.
                storage.bytes(
                    size_of::<Option<crate::hir::TypeFacts>>()
                        + size_of::<crate::hir::TypeFacts>()
                        + size_of::<Option<TypeFlags>>()
                        + size_of::<TypeFlags>(),
                )?;
                if let Some(facts) = builtin::facts(self, declaration, arguments, storage) {
                    return Some(TypeFlags::from(&facts));
                }
                if storage.first.is_some() {
                    return None;
                }
            }
        }
        query::compute(self, ty, storage)
    }

    #[cfg(test)]
    fn flags_paid_original<P: TypeFactsPayer>(
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

// Shared borrowed admission predicates: no query owns the discarded comparison
// trees. The complete old classifier and its admission calls stay independent.
pub(super) fn admitted_native_owned(index: &DeclarationIndex, ty: &ResolvedType) -> bool {
    admission::native_owned(index, ty)
}
pub(super) fn admitted_authored_byte_variant(index: &DeclarationIndex, ty: &ResolvedType) -> bool {
    admission::authored_byte_variant(index, ty)
}
