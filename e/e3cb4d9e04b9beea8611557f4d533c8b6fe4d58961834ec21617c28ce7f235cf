//! Faithful independent physical counterpart of the original owned classifier.
use super::super::super::{
    admitted_owned_byte_prelude_instance, owned_builtin, DeclarationId, DeclarationIndex,
    DeclarationKind, ResolvedType, TypeFacts,
};
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
) -> Option<TypeFacts> {
    storage.bytes(
        std::mem::size_of::<BTreeSet<String>>()
            + std::mem::size_of::<BTreeMap<String, TypeFacts>>()
            + 2 * std::mem::size_of::<Vec<()>>(),
    )?;
    index.compute_paid_type_facts(ty, &mut BTreeSet::new(), &mut BTreeMap::new(), storage)
}
impl DeclarationIndex {
    fn compute_paid_type_facts<P: Payer>(
        &self,
        ty: &ResolvedType,
        visiting: &mut BTreeSet<String>,
        memo: &mut BTreeMap<String, TypeFacts>,
        storage: &mut Storage<P>,
    ) -> Option<TypeFacts> {
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
        let mut results = Vec::<TypeFacts>::new();
        while let Some(frame) = frames.pop() {
            #[cfg(test)]
            storage.oracle_work(1, "classifier compute frame")?;
            match frame {
                Frame::Enter(ty) => {
                    let identity = key::identity(&ty, storage)?;

                    if let Some(facts) = memo.get(&identity) {
                        let facts = storage.facts_clone(facts)?;
                        storage.push(&mut results, facts)?;
                        continue;
                    }
                    if crate::map_ops::is_typed_collection(&ty) {
                        storage.grow(&mut results, 1)?;
                        results.push(TypeFacts {
                            copy: false,
                            contains_resource: false,
                            sized: true,
                            needs_drop: true,
                            layout_key: {
                                let key = key::identity(&ty, storage)?;
                                storage.text(format_args!("owned:collection:{}", key))?
                            },
                        });
                        continue;
                    }
                    let scalar = match &ty {
                        ResolvedType::Function { .. } => {
                            if !crate::hir::function_value::is_signature(&ty) {
                                return None;
                            }
                            storage.grow(&mut results, 1)?;
                            results.push(TypeFacts {
                                copy: true,
                                contains_resource: false,
                                sized: true,
                                needs_drop: false,
                                layout_key: storage.literal(&identity)?,
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
                        ResolvedType::SliceU8 => Some((false, false, false, "borrowed:slice-u8")),
                        ResolvedType::StringMap => {
                            Some((false, false, true, "owned:map:string:i64"))
                        }
                        ResolvedType::TypeParameter { .. } | ResolvedType::Nominal { .. } => None,
                    };
                    if let ResolvedType::ArrayU8(length) = &ty {
                        storage.grow(&mut results, 1)?;
                        results.push(TypeFacts {
                            copy: true,
                            contains_resource: false,
                            sized: true,
                            needs_drop: false,
                            layout_key: storage.text(format_args!("array:u8:{length}"))?,
                        });
                        continue;
                    }
                    if let Some((copy, contains_resource, needs_drop, key)) = scalar {
                        storage.grow(&mut results, 1)?;
                        results.push(TypeFacts {
                            copy,
                            contains_resource,
                            sized: true,
                            needs_drop,
                            layout_key: storage.literal(key)?,
                        });
                        continue;
                    }
                    let native_owned_container = admission::native_owned(self, &ty)
                        || owned_builtin::resource_result_shape(self, &ty);
                    let authored_owned_byte_variant = admission::authored_byte_variant(self, &ty);
                    let ResolvedType::Nominal {
                        declaration,
                        arguments,
                    } = ty
                    else {
                        return None;
                    };
                    let item = self.declaration(&declaration)?;
                    if let Some(facts) = builtin::facts(self, &declaration, &arguments, storage) {
                        if !memo.contains_key(&identity) {
                            storage.node::<String, TypeFacts>()?;
                        }
                        let retained = storage.facts_clone(&facts)?;
                        memo.insert(identity, retained);
                        storage.push(&mut results, facts)?;
                        continue;
                    }
                    if storage.first.is_some() {
                        return None;
                    }
                    if item.kind == DeclarationKind::Resource && arguments.is_empty() {
                        let facts = TypeFacts {
                            copy: false,
                            contains_resource: true,
                            sized: true,
                            needs_drop: true,
                            layout_key: storage.text(format_args!("resource:{identity}"))?,
                        };
                        if !memo.contains_key(&identity) {
                            storage.node::<String, TypeFacts>()?;
                        }
                        let retained = storage.facts_clone(&facts)?;
                        memo.insert(identity, retained);
                        storage.push(&mut results, facts)?;
                        continue;
                    }
                    if !matches!(
                        item.kind,
                        DeclarationKind::Record | DeclarationKind::Class | DeclarationKind::Variant
                    ) {
                        return None;
                    }
                    let parameters = self.type_parameters.get(&declaration)?;
                    let compiler_byte_option = declaration.as_str() == crate::prelude::OPTION_ID
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
                    let mut encoded = storage.encoded()?;
                    match kind {
                        DeclarationKind::Record | DeclarationKind::Class => {
                            let fields = self.record_fields.get(&declaration)?;
                            let mut copy = true;
                            let mut contains_resource = false;
                            let mut sized = true;
                            let mut needs_drop = false;
                            for (field, facts) in fields.iter().zip(child_facts) {
                                #[cfg(test)]
                                storage.oracle_work(1, "classifier declared field receive")?;
                                copy &= facts.copy;
                                contains_resource |= facts.contains_resource;
                                sized &= facts.sized;
                                needs_drop |= facts.needs_drop;
                                storage.append(
                                    &mut encoded,
                                    format_args!(
                                        "{}:{}:{}:{}",
                                        field.id.as_str().len(),
                                        field.id,
                                        facts.layout_key.len(),
                                        facts.layout_key
                                    ),
                                )?;
                            }

                            let prefix = if kind == DeclarationKind::Class {
                                "class"
                            } else {
                                "record"
                            };
                            let facts = TypeFacts {
                                copy,
                                contains_resource,
                                sized,
                                needs_drop,
                                layout_key: storage.text(format_args!(
                                    "{prefix}:{}:{}:{}:{}",
                                    declaration.as_str().len(),
                                    declaration,
                                    fields.len(),
                                    encoded.into_string()
                                ))?,
                            };

                            if !memo.contains_key(&identity) {
                                storage.node::<String, TypeFacts>()?;
                            }
                            let retained = storage.facts_clone(&facts)?;
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
                                storage.append(
                                    &mut encoded,
                                    format_args!(
                                        "{}:{}:{}:",
                                        case.id.as_str().len(),
                                        case.id,
                                        case.fields.len()
                                    ),
                                )?;
                                for field in &case.fields {
                                    #[cfg(test)]
                                    storage.oracle_work(1, "classifier declared field receive")?;
                                    let facts = facts_iter.next()?;
                                    copy &= facts.copy;
                                    contains_resource |= facts.contains_resource;
                                    sized &= facts.sized;
                                    needs_drop |= facts.needs_drop;
                                    storage.append(
                                        &mut encoded,
                                        format_args!(
                                            "{}:{}:{}:{}",
                                            field.id.as_str().len(),
                                            field.id,
                                            facts.layout_key.len(),
                                            facts.layout_key
                                        ),
                                    )?;
                                }
                            }

                            let facts = TypeFacts {
                                copy,
                                contains_resource,
                                sized,
                                needs_drop,
                                layout_key: storage.text(format_args!(
                                    "variant:{}:{}:{}:{}",
                                    declaration.as_str().len(),
                                    declaration,
                                    cases.len(),
                                    encoded.into_string()
                                ))?,
                            };

                            if !memo.contains_key(&identity) {
                                storage.node::<String, TypeFacts>()?;
                            }
                            let retained = storage.facts_clone(&facts)?;
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
