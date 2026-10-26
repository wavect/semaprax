//! Flag-only receiver; the owned layout/fact API remains the independent oracle.
use super::{DeclarationIndex, ResolvedType, TypeFacts};
mod nominal_key;
mod recursive {
    //! Recursive flag queries keep full type admission and traversal order.
    //! Complete owned layout construction remains the independent TypeFacts API.
    use super::super::{
        admitted_owned_byte_prelude_instance, owned_builtin, owned_builtin_facts, substitute_type,
        DeclarationId, DeclarationKind,
    };
    use super::*;
    use std::collections::{BTreeMap, BTreeSet};
    impl DeclarationIndex {
        pub(super) fn recursive_flags(&self, ty: &ResolvedType) -> Option<TypeFlags> {
            // A complete builtin result has no recursive children. Borrow the
            // original root and project its full facts directly; a local memo,
            // cloned root and traversal worklist would be immediately discarded.
            // The existing outer query has already applied current-cache precedence.
            if let ResolvedType::Nominal {
                declaration,
                arguments,
            } = ty
            {
                self.declaration(declaration)?;
                if let Some(facts) = owned_builtin_facts(self, declaration, arguments) {
                    return Some(TypeFlags::from(&facts));
                }
            }
            self.compute_flag_values(ty, &mut BTreeSet::new(), &mut BTreeMap::new())
        }
        fn compute_flag_values(
            &self,
            ty: &ResolvedType,
            visiting: &mut BTreeSet<String>,
            memo: &mut BTreeMap<String, TypeFlags>,
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

            let mut frames = vec![Frame::Enter(ty.clone())];
            let mut results = Vec::<TypeFlags>::new();
            while let Some(frame) = frames.pop() {
                match frame {
                    Frame::Enter(ty) => {
                        let identity = ty.identity_key();

                        if let Some(facts) = memo.get(&identity) {
                            results.push(*facts);
                            continue;
                        }
                        if crate::map_ops::is_typed_collection(&ty) {
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
                            results.push(TypeFlags {
                                copy: true,
                                contains_resource: false,
                                sized: true,
                                needs_drop: false,
                            });
                            continue;
                        }
                        if let Some((copy, contains_resource, needs_drop, _key)) = scalar {
                            results.push(TypeFlags {
                                copy,
                                contains_resource,
                                sized: true,
                                needs_drop,
                            });
                            continue;
                        }
                        let native_owned_container = super::paid::admitted_native_owned(self, &ty)
                            || owned_builtin::resource_result_shape(self, &ty);
                        let authored_owned_byte_variant =
                            super::paid::admitted_authored_byte_variant(self, &ty);
                        let ResolvedType::Nominal {
                            declaration,
                            arguments,
                        } = ty
                        else {
                            return None;
                        };
                        let item = self.declaration(&declaration)?;
                        if let Some(full_facts) =
                            owned_builtin_facts(self, &declaration, &arguments)
                        {
                            let facts = TypeFlags::from(&full_facts);
                            memo.insert(identity, facts);
                            results.push(facts);
                            continue;
                        }
                        if item.kind == DeclarationKind::Resource && arguments.is_empty() {
                            let facts = TypeFlags {
                                copy: false,
                                contains_resource: true,
                                sized: true,
                                needs_drop: true,
                            };
                            memo.insert(identity, facts);
                            results.push(facts);
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
                            || !visiting.insert(identity.clone())
                        {
                            return None;
                        }
                        let children = match item.kind {
                            DeclarationKind::Record | DeclarationKind::Class => self
                                .record_fields
                                .get(&declaration)?
                                .iter()
                                .map(|field| {
                                    substitute_type(&field.ty, &declaration, &arguments).ok()
                                })
                                .collect::<Option<Vec<_>>>()?,
                            DeclarationKind::Variant => self
                                .variant_cases
                                .get(&declaration)?
                                .iter()
                                .flat_map(|case| &case.fields)
                                .map(|field| {
                                    substitute_type(&field.ty, &declaration, &arguments).ok()
                                })
                                .collect::<Option<Vec<_>>>()?,
                            _ => unreachable!(),
                        };

                        frames.try_reserve(children.len() + 1).ok()?;
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
                        let child_facts = results.drain(split..).collect::<Vec<_>>();

                        visiting.remove(&identity);

                        match kind {
                            DeclarationKind::Record | DeclarationKind::Class => {
                                let fields = self.record_fields.get(&declaration)?;
                                let mut copy = true;
                                let mut contains_resource = false;
                                let mut sized = true;
                                let mut needs_drop = false;
                                for (_field, facts) in fields.iter().zip(&child_facts) {
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

                                memo.insert(identity, facts);
                                results.push(facts);
                            }
                            DeclarationKind::Variant => {
                                let cases = self.variant_cases.get(&declaration)?;
                                let mut facts_iter = child_facts.iter();
                                let mut copy = true;
                                let mut contains_resource = false;
                                let mut sized = true;
                                let mut needs_drop = false;
                                for case in cases {
                                    for _field in &case.fields {
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

                                memo.insert(identity, facts);
                                results.push(facts);
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
mod paid;
pub(crate) use paid::TypeFactsPayer;
pub(crate) use paid::{inventory_identity, inventory_substitute, InventoryPayer};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct TypeFlags {
    pub(crate) copy: bool,
    pub(crate) contains_resource: bool,
    pub(crate) sized: bool,
    pub(crate) needs_drop: bool,
}
impl From<&TypeFacts> for TypeFlags {
    fn from(facts: &TypeFacts) -> Self {
        Self {
            copy: facts.copy,
            contains_resource: facts.contains_resource,
            sized: facts.sized,
            needs_drop: facts.needs_drop,
        }
    }
}
impl DeclarationIndex {
    pub(crate) fn cached_zero_argument_facts(
        &self,
        declaration: &super::DeclarationId,
    ) -> Option<&TypeFacts> {
        nominal_key::get(&self.type_facts_by_id, declaration)
    }
    fn recompute_flags(&self, ty: &ResolvedType) -> Option<TypeFlags> {
        if matches!(ty, ResolvedType::ArrayU8(_)) || matches!(ty, ResolvedType::Function { .. }) {
            if matches!(ty, ResolvedType::Function { .. })
                && !crate::hir::function_value::is_signature(ty)
            {
                return None;
            }
            return Some(TypeFlags {
                copy: true,
                contains_resource: false,
                sized: true,
                needs_drop: false,
            });
        }
        self.recursive_flags(ty)
    }
    /// Preserve current-index cache precedence, including hostile cached facts.
    /// Dynamic identities retain the old key traversal and miss computation.
    /// A cache hit borrows the facts instead of cloning their layout payload.
    pub(crate) fn type_flags(&self, ty: &ResolvedType) -> Option<TypeFlags> {
        let key = match ty {
            ResolvedType::Never => "never:lexical-exit:v1",
            ResolvedType::Unit => "unit",
            ResolvedType::I64 => "i64",
            ResolvedType::I32 => "i32",
            ResolvedType::Char => "char",
            ResolvedType::U8 => "u8",
            ResolvedType::Usize => "usize",
            ResolvedType::F32 => "f32",
            ResolvedType::F64 => "f64",
            ResolvedType::Bool => "bool",
            ResolvedType::String => "string",
            ResolvedType::Bytes => "bytes",
            ResolvedType::Str => "str",
            ResolvedType::SliceU8 => "slice-u8",
            ResolvedType::StringMap => "map:string:i64:v1",
            ResolvedType::OnceFunction => "fn-once:bytes:i64:v1",
            ResolvedType::OnceFunctionI64 => "fn-once:bytes+i64:i64:v2",
            ResolvedType::OnceFunctionI64Pair => "fn-once:bytes+i64+i64:i64:v3",
            ResolvedType::MutFunctionI64 => "fn-mut:i64+i64:i64:v1",
            ResolvedType::Nominal {
                declaration,
                arguments,
            } if arguments.is_empty() => {
                return nominal_key::get(&self.type_facts_by_id, declaration)
                    .map(TypeFlags::from)
                    .or_else(|| self.recompute_flags(ty));
            }
            _ => {
                if let Some(facts) = nominal_key::compound_get(&self.type_facts_by_id, ty) {
                    return facts
                        .map(TypeFlags::from)
                        .or_else(|| self.recompute_flags(ty));
                }
                let key = ty.identity_key();
                return self
                    .type_facts_by_id
                    .get(&key)
                    .map(TypeFlags::from)
                    .or_else(|| self.recompute_flags(ty));
            }
        };
        if let Some(facts) = self.type_facts_by_id.get(key) {
            return Some(TypeFlags::from(facts));
        }
        // These are exactly compute_type_facts' leaf branches after cache miss.
        let (copy, needs_drop) = match ty {
            ResolvedType::Never => return None,
            ResolvedType::Unit
            | ResolvedType::I64
            | ResolvedType::I32
            | ResolvedType::Char
            | ResolvedType::U8
            | ResolvedType::Usize
            | ResolvedType::F32
            | ResolvedType::F64
            | ResolvedType::Bool => (true, false),
            ResolvedType::String
            | ResolvedType::Bytes
            | ResolvedType::StringMap
            | ResolvedType::OnceFunction
            | ResolvedType::OnceFunctionI64
            | ResolvedType::OnceFunctionI64Pair => (false, true),
            ResolvedType::Str | ResolvedType::SliceU8 | ResolvedType::MutFunctionI64 => {
                (false, false)
            }
            _ => unreachable!("only static leaf identities reach leaf facts"),
        };
        Some(TypeFlags {
            copy,
            contains_resource: false,
            sized: true,
            needs_drop,
        })
    }
}
#[cfg(test)]
mod tests;

/// None means the bounded key census declined; Some(None) is a genuine exact
/// cache miss. Neither route changes the original semantic miss computation.
pub(super) fn cached_facts<'a>(
    index: &'a DeclarationIndex,
    ty: &ResolvedType,
) -> Option<Option<&'a TypeFacts>> {
    match ty {
        ResolvedType::Nominal {
            declaration,
            arguments,
        } if arguments.is_empty() => Some(nominal_key::get(&index.type_facts_by_id, declaration)),
        _ => nominal_key::compound_get(&index.type_facts_by_id, ty),
    }
}
#[cfg(test)]
mod owned_key_oracle;
#[cfg(test)]
pub(super) use owned_key_oracle::enabled as old_key_enabled;
