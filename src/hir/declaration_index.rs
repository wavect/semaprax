//! Deterministic declaration lookup, type facts, and inheritance materialization.

use super::*;

/// A deterministic, display-name-to-identity index.
///
/// Types and values occupy distinct namespaces so future record/variant type
/// declarations can coexist with functions without ambiguous lookup.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct DeclarationIndex {
    pub(super) declarations: BTreeMap<DeclarationId, Declaration>,
    pub(super) types_by_name: BTreeMap<String, DeclarationId>,
    pub(super) functions_by_name: BTreeMap<String, DeclarationId>,
    pub(super) fields_by_owner_name: BTreeMap<(DeclarationId, String), DeclarationId>,
    pub(super) record_fields: BTreeMap<DeclarationId, Vec<ResolvedFieldDeclaration>>,
    pub(super) cases_by_owner_name: BTreeMap<(DeclarationId, String), DeclarationId>,
    pub(super) variant_cases: BTreeMap<DeclarationId, Vec<ResolvedVariantCaseDeclaration>>,
    pub(super) case_fields: BTreeMap<DeclarationId, Vec<ResolvedFieldDeclaration>>,
    pub(super) type_parameters: BTreeMap<DeclarationId, Vec<ResolvedTypeParameterDeclaration>>,
    pub(super) imports_by_key: BTreeMap<String, DeclarationId>,
    pub(super) native_rust_imports_by_name: BTreeMap<String, DeclarationId>,
    pub(super) type_facts_by_id: BTreeMap<String, TypeFacts>,
    /// Class Inheritance v1: the declared parent of each extending class.
    /// Classes without `extends` have no entry.
    pub(super) class_parents: BTreeMap<DeclarationId, DeclarationId>,
    pub(super) byte_slice_roots: BTreeMap<ValueId, ByteSliceProvenance>,
}

impl DeclarationIndex {
    /// Moves every recursively nested resolved type out of the declaration
    /// index before ordinary field drop glue runs. Private bounded owners use
    /// this hook to feed their existing preallocated iterative disposer.
    fn drain_recursive_types_for_private_contract(
        &mut self,
        mut dispose: impl FnMut(ResolvedType),
    ) {
        for (_, fields) in std::mem::take(&mut self.record_fields) {
            for field in fields {
                dispose(field.ty);
            }
        }
        for (_, cases) in std::mem::take(&mut self.variant_cases) {
            for case in cases {
                for field in case.fields {
                    dispose(field.ty);
                }
            }
        }
        for (_, fields) in std::mem::take(&mut self.case_fields) {
            for field in fields {
                dispose(field.ty);
            }
        }
    }

    pub fn byte_slice_provenance(&self, value: &ValueId) -> Option<&ByteSliceProvenance> {
        self.byte_slice_roots.get(value)
    }

    pub fn byte_slice_provenances(
        &self,
    ) -> impl ExactSizeIterator<Item = (&ValueId, &ByteSliceProvenance)> {
        self.byte_slice_roots.iter()
    }
    #[cfg(test)]
    pub(super) fn type_facts_layout_capacity(&self) -> usize {
        self.type_facts_by_id
            .values()
            .map(|facts| facts.layout_key.capacity())
            .sum()
    }

    #[cfg(test)]
    pub(super) fn owned_capacity_for_private_contract(&self) -> usize {
        fn string_map_capacity<V>(map: &BTreeMap<String, V>) -> usize {
            map.len() * std::mem::size_of::<(String, V)>()
                + map.keys().map(String::capacity).sum::<usize>()
        }
        fn named_id_map_capacity(map: &BTreeMap<String, DeclarationId>) -> usize {
            string_map_capacity(map) + map.values().map(|id| id.as_str().len()).sum::<usize>()
        }
        fn owner_name_map_capacity(
            map: &BTreeMap<(DeclarationId, String), DeclarationId>,
        ) -> usize {
            map.len() * std::mem::size_of::<((DeclarationId, String), DeclarationId)>()
                + map
                    .iter()
                    .map(|((owner, name), value)| {
                        owner.as_str().len() + name.capacity() + value.as_str().len()
                    })
                    .sum::<usize>()
        }
        fn field_capacity(field: &ResolvedFieldDeclaration) -> usize {
            field.id.as_str().len()
                + field.name.capacity()
                + resolved_type_owned_capacity(&field.ty)
        }
        let declaration_bytes = self
            .declarations
            .iter()
            .map(|(id, declaration)| {
                id.as_str().len()
                    + declaration.id.as_str().len()
                    + declaration.name.capacity()
                    + declaration
                        .owner
                        .as_ref()
                        .map_or(0, |owner| owner.as_str().len())
            })
            .sum::<usize>();
        let field_bytes = self
            .record_fields
            .values()
            .chain(self.case_fields.values())
            .flatten()
            .map(field_capacity)
            .sum::<usize>();
        let case_bytes = self
            .variant_cases
            .values()
            .flatten()
            .map(|case| {
                case.id.as_str().len()
                    + case.name.capacity()
                    + case.fields.capacity() * std::mem::size_of::<ResolvedFieldDeclaration>()
                    + case.fields.iter().map(field_capacity).sum::<usize>()
            })
            .sum::<usize>();
        let fact_bytes = self
            .type_facts_by_id
            .values()
            .map(|facts| facts.layout_key.capacity())
            .sum::<usize>();
        let declaration_map_backing =
            self.declarations.len() * std::mem::size_of::<(DeclarationId, Declaration)>();
        let record_field_maps = self
            .record_fields
            .iter()
            .chain(self.case_fields.iter())
            .map(|(owner, fields)| {
                std::mem::size_of::<(DeclarationId, Vec<ResolvedFieldDeclaration>)>()
                    + owner.as_str().len()
                    + fields.capacity() * std::mem::size_of::<ResolvedFieldDeclaration>()
            })
            .sum::<usize>();
        let variant_case_map = self
            .variant_cases
            .iter()
            .map(|(owner, cases)| {
                std::mem::size_of::<(DeclarationId, Vec<ResolvedVariantCaseDeclaration>)>()
                    + owner.as_str().len()
                    + cases.capacity() * std::mem::size_of::<ResolvedVariantCaseDeclaration>()
            })
            .sum::<usize>();
        let type_parameter_map = self
            .type_parameters
            .iter()
            .map(|(owner, parameters)| {
                std::mem::size_of::<(DeclarationId, Vec<ResolvedTypeParameterDeclaration>)>()
                    + owner.as_str().len()
                    + parameters.capacity()
                        * std::mem::size_of::<ResolvedTypeParameterDeclaration>()
                    + parameters
                        .iter()
                        .map(|parameter| parameter.name.capacity())
                        .sum::<usize>()
            })
            .sum::<usize>();
        declaration_map_backing
            + record_field_maps
            + variant_case_map
            + type_parameter_map
            + declaration_bytes
            + field_bytes
            + case_bytes
            + fact_bytes
            + named_id_map_capacity(&self.types_by_name)
            + named_id_map_capacity(&self.functions_by_name)
            + named_id_map_capacity(&self.imports_by_key)
            + named_id_map_capacity(&self.native_rust_imports_by_name)
            + string_map_capacity(&self.type_facts_by_id)
            + owner_name_map_capacity(&self.fields_by_owner_name)
            + owner_name_map_capacity(&self.cases_by_owner_name)
    }

    pub(crate) fn workspace_declarations(&self) -> Vec<Declaration> {
        self.declarations.values().cloned().collect()
    }

    pub fn declaration(&self, id: &DeclarationId) -> Option<&Declaration> {
        self.declarations.get(id)
    }

    pub fn type_id(&self, name: &str) -> Option<&DeclarationId> {
        self.types_by_name.get(name)
    }

    pub fn function_id(&self, name: &str) -> Option<&DeclarationId> {
        self.functions_by_name.get(name)
    }

    pub fn field_id(&self, owner: &DeclarationId, name: &str) -> Option<&DeclarationId> {
        self.fields_by_owner_name
            .get(&(owner.clone(), name.to_owned()))
    }

    pub fn record_fields(&self, owner: &DeclarationId) -> Option<&[ResolvedFieldDeclaration]> {
        self.record_fields.get(owner).map(Vec::as_slice)
    }

    /// Class Inheritance v1: the declared parent of `class`, if any.
    pub fn class_parent(&self, class: &DeclarationId) -> Option<&DeclarationId> {
        self.class_parents.get(class)
    }

    /// The ancestor chain of `class`, nearest parent first. Cycle-safe by
    /// construction; inheritance cycles are diagnosed before resolution.
    pub fn class_ancestors(&self, class: &DeclarationId) -> Vec<DeclarationId> {
        let mut ancestors = Vec::new();
        let mut visited = BTreeSet::new();
        let mut cursor = class.clone();
        while let Some(parent) = self.class_parents.get(&cursor) {
            if !visited.insert(parent.clone()) {
                break;
            }
            ancestors.push(parent.clone());
            cursor = parent.clone();
        }
        ancestors
    }

    /// `true` when `ancestor` names a class that `class` transitively extends.
    pub fn class_extends(&self, class: &DeclarationId, ancestor: &DeclarationId) -> bool {
        self.class_ancestors(class)
            .iter()
            .any(|item| item == ancestor)
    }

    pub fn case_id(&self, owner: &DeclarationId, name: &str) -> Option<&DeclarationId> {
        self.cases_by_owner_name
            .get(&(owner.clone(), name.to_owned()))
    }

    pub fn variant_cases(
        &self,
        owner: &DeclarationId,
    ) -> Option<&[ResolvedVariantCaseDeclaration]> {
        self.variant_cases.get(owner).map(Vec::as_slice)
    }

    pub fn case_fields(&self, case: &DeclarationId) -> Option<&[ResolvedFieldDeclaration]> {
        self.case_fields.get(case).map(Vec::as_slice)
    }

    pub fn type_parameters(
        &self,
        declaration: &DeclarationId,
    ) -> Option<&[ResolvedTypeParameterDeclaration]> {
        self.type_parameters.get(declaration).map(Vec::as_slice)
    }

    pub fn import_id(&self, key: &str) -> Option<&DeclarationId> {
        self.imports_by_key.get(key)
    }

    pub fn native_rust_import_id(&self, name: &str) -> Option<&DeclarationId> {
        self.native_rust_imports_by_name.get(name)
    }

    pub fn declarations(&self) -> impl ExactSizeIterator<Item = &Declaration> {
        self.declarations.values()
    }

    /// Computes the recursive semantic facts shared by ownership and backends.
    ///
    /// `None` is reserved for unresolved type parameters and future malformed
    /// HIR. Every type produced for today's verified language has facts.
    pub fn type_facts(&self, ty: &ResolvedType) -> Option<TypeFacts> {
        self.type_facts_by_id
            .get(&ty.identity_key())
            .cloned()
            .or_else(|| self.recompute_type_facts(ty))
    }

    fn compute_type_facts(
        &self,
        ty: &ResolvedType,
        visiting: &mut BTreeSet<DeclarationId>,
        memo: &mut BTreeMap<String, TypeFacts>,
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

        #[cfg(test)]
        fn frame_owned_capacity(frame: &Frame) -> usize {
            match frame {
                Frame::Enter(ty) => resolved_type_owned_capacity(ty),
                Frame::Finish {
                    identity,
                    declaration,
                    ..
                } => identity.capacity() + declaration.as_str().len(),
            }
        }

        #[cfg(test)]
        fn retained_capacity(
            frames: &Vec<Frame>,
            results: &Vec<TypeFacts>,
            memo: &BTreeMap<String, TypeFacts>,
            visiting: &BTreeSet<DeclarationId>,
        ) -> usize {
            type_facts_outer_baseline()
                + frames.capacity() * std::mem::size_of::<Frame>()
                + frames.iter().map(frame_owned_capacity).sum::<usize>()
                + results.capacity() * std::mem::size_of::<TypeFacts>()
                + results
                    .iter()
                    .map(|facts| facts.layout_key.capacity())
                    .sum::<usize>()
                + memo.len()
                    * (std::mem::size_of::<(String, TypeFacts)>()
                        + std::mem::size_of::<BTreeMap<String, TypeFacts>>())
                + memo
                    .iter()
                    .map(|(key, facts)| key.capacity() + facts.layout_key.capacity())
                    .sum::<usize>()
                + visiting.len()
                    * (std::mem::size_of::<DeclarationId>()
                        + std::mem::size_of::<BTreeSet<DeclarationId>>())
                + visiting.iter().map(|id| id.as_str().len()).sum::<usize>()
        }

        let mut frames = vec![Frame::Enter(ty.clone())];
        let mut results = Vec::<TypeFacts>::new();
        while let Some(frame) = frames.pop() {
            #[cfg(test)]
            note_iterative_phase_capacity(
                2,
                retained_capacity(&frames, &results, memo, visiting) + frame_owned_capacity(&frame),
            );
            match frame {
                Frame::Enter(ty) => {
                    let identity = ty.identity_key();
                    #[cfg(test)]
                    note_iterative_phase_capacity(
                        2,
                        retained_capacity(&frames, &results, memo, visiting)
                            + resolved_type_owned_capacity(&ty)
                            + identity.capacity(),
                    );
                    if let Some(facts) = memo.get(&identity) {
                        results.push(facts.clone());
                        continue;
                    }
                    let scalar = match &ty {
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
                        ResolvedType::Str => Some((false, false, false, "borrowed:str")),
                        ResolvedType::SliceU8 => Some((false, false, false, "borrowed:slice-u8")),
                        ResolvedType::TypeParameter { .. } | ResolvedType::Nominal { .. } => None,
                    };
                    if let ResolvedType::ArrayU8(length) = &ty {
                        results.push(TypeFacts {
                            copy: true,
                            contains_resource: false,
                            sized: true,
                            needs_drop: false,
                            layout_key: format!("array:u8:{length}"),
                        });
                        continue;
                    }
                    if let Some((copy, contains_resource, needs_drop, key)) = scalar {
                        results.push(TypeFacts {
                            copy,
                            contains_resource,
                            sized: true,
                            needs_drop,
                            layout_key: key.to_string(),
                        });
                        continue;
                    }
                    let ResolvedType::Nominal {
                        declaration,
                        arguments,
                    } = ty
                    else {
                        return None;
                    };
                    let item = self.declaration(&declaration)?;
                    if item.kind == DeclarationKind::Resource && arguments.is_empty() {
                        let facts = TypeFacts {
                            copy: false,
                            contains_resource: true,
                            sized: true,
                            needs_drop: true,
                            layout_key: format!("resource:{identity}"),
                        };
                        memo.insert(identity, facts.clone());
                        results.push(facts);
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
                    if arguments.len() != parameters.len()
                        || (!compiler_byte_option
                            && arguments.iter().any(|argument| {
                                !matches!(argument, ResolvedType::I64 | ResolvedType::Bool)
                            }))
                        || !visiting.insert(declaration.clone())
                    {
                        return None;
                    }
                    let children = match item.kind {
                        DeclarationKind::Record | DeclarationKind::Class => self
                            .record_fields
                            .get(&declaration)?
                            .iter()
                            .map(|field| substitute_type(&field.ty, &declaration, &arguments).ok())
                            .collect::<Option<Vec<_>>>()?,
                        DeclarationKind::Variant => self
                            .variant_cases
                            .get(&declaration)?
                            .iter()
                            .flat_map(|case| &case.fields)
                            .map(|field| substitute_type(&field.ty, &declaration, &arguments).ok())
                            .collect::<Option<Vec<_>>>()?,
                        _ => unreachable!(),
                    };
                    #[cfg(test)]
                    note_iterative_phase_capacity(
                        2,
                        retained_capacity(&frames, &results, memo, visiting)
                            + identity.capacity()
                            + declaration.as_str().len()
                            + children.capacity() * std::mem::size_of::<ResolvedType>()
                            + children
                                .iter()
                                .map(resolved_type_owned_capacity)
                                .sum::<usize>(),
                    );
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
                    #[cfg(test)]
                    let finish_identity_bytes = identity.capacity() + declaration.as_str().len();
                    let split = results.len().checked_sub(child_count)?;
                    let child_facts = results.drain(split..).collect::<Vec<_>>();
                    #[cfg(test)]
                    note_iterative_phase_capacity(
                        2,
                        retained_capacity(&frames, &results, memo, visiting)
                            + finish_identity_bytes
                            + child_facts.capacity() * std::mem::size_of::<TypeFacts>()
                            + child_facts
                                .iter()
                                .map(|facts| facts.layout_key.capacity())
                                .sum::<usize>(),
                    );
                    visiting.remove(&declaration);
                    let mut encoded = crate::bounded_output::CappedString::new();
                    match kind {
                        DeclarationKind::Record | DeclarationKind::Class => {
                            let fields = self.record_fields.get(&declaration)?;
                            let mut copy = true;
                            let mut contains_resource = false;
                            let mut sized = true;
                            let mut needs_drop = false;
                            for (field, facts) in fields.iter().zip(&child_facts) {
                                copy &= facts.copy;
                                contains_resource |= facts.contains_resource;
                                sized &= facts.sized;
                                needs_drop |= facts.needs_drop;
                                write!(
                                    encoded,
                                    "{}:{}:{}:{}",
                                    field.id.as_str().len(),
                                    field.id,
                                    facts.layout_key.len(),
                                    facts.layout_key
                                )
                                .ok()?;
                            }
                            #[cfg(test)]
                            let encoded_capacity = encoded.allocated_capacity();
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
                                layout_key: format!(
                                    "{prefix}:{}:{}:{}:{}",
                                    declaration.as_str().len(),
                                    declaration,
                                    fields.len(),
                                    encoded.into_string()
                                ),
                            };
                            #[cfg(test)]
                            note_iterative_phase_capacity(
                                2,
                                retained_capacity(&frames, &results, memo, visiting)
                                    + finish_identity_bytes
                                    + child_facts.capacity() * std::mem::size_of::<TypeFacts>()
                                    + child_facts
                                        .iter()
                                        .map(|facts| facts.layout_key.capacity())
                                        .sum::<usize>()
                                    + encoded_capacity
                                    + facts.layout_key.capacity(),
                            );
                            memo.insert(identity, facts.clone());
                            results.push(facts);
                        }
                        DeclarationKind::Variant => {
                            let cases = self.variant_cases.get(&declaration)?;
                            let mut facts_iter = child_facts.iter();
                            for case in cases {
                                write!(
                                    encoded,
                                    "{}:{}:{}:",
                                    case.id.as_str().len(),
                                    case.id,
                                    case.fields.len()
                                )
                                .ok()?;
                                for field in &case.fields {
                                    let facts = facts_iter.next()?;
                                    if !facts.copy || facts.contains_resource || facts.needs_drop {
                                        return None;
                                    }
                                    write!(
                                        encoded,
                                        "{}:{}:{}:{}",
                                        field.id.as_str().len(),
                                        field.id,
                                        facts.layout_key.len(),
                                        facts.layout_key
                                    )
                                    .ok()?;
                                }
                            }
                            #[cfg(test)]
                            let encoded_capacity = encoded.allocated_capacity();
                            let facts = TypeFacts {
                                copy: true,
                                contains_resource: false,
                                sized: true,
                                needs_drop: false,
                                layout_key: format!(
                                    "variant:{}:{}:{}:{}",
                                    declaration.as_str().len(),
                                    declaration,
                                    cases.len(),
                                    encoded.into_string()
                                ),
                            };
                            #[cfg(test)]
                            note_iterative_phase_capacity(
                                2,
                                retained_capacity(&frames, &results, memo, visiting)
                                    + finish_identity_bytes
                                    + child_facts.capacity() * std::mem::size_of::<TypeFacts>()
                                    + child_facts
                                        .iter()
                                        .map(|facts| facts.layout_key.capacity())
                                        .sum::<usize>()
                                    + encoded_capacity
                                    + facts.layout_key.capacity(),
                            );
                            memo.insert(identity, facts.clone());
                            results.push(facts);
                        }
                        _ => unreachable!(),
                    }
                }
            }
        }
        (results.len() == 1).then(|| results.pop().expect("type fact count checked above"))
    }

    pub(super) fn populate_type_facts(&mut self) -> bool {
        let mut memo = BTreeMap::new();
        for ty in [ResolvedType::I64, ResolvedType::Bool] {
            let Some(facts) = self.compute_type_facts(&ty, &mut BTreeSet::new(), &mut memo) else {
                return false;
            };
            memo.insert(ty.identity_key(), facts);
        }
        let declarations = self.types_by_name.values().cloned().collect::<Vec<_>>();
        #[cfg(test)]
        let declarations_capacity = declarations.capacity() * std::mem::size_of::<DeclarationId>()
            + declarations
                .iter()
                .map(|id| id.as_str().len())
                .sum::<usize>();
        #[cfg(test)]
        TYPE_FACTS_OUTER_BASELINE.with(|baseline| baseline.set(declarations_capacity));
        for declaration in declarations {
            #[cfg(test)]
            note_iterative_phase_capacity(
                2,
                declarations_capacity.saturating_add(declaration.as_str().len()),
            );
            if self
                .type_parameters
                .get(&declaration)
                .is_some_and(|parameters| !parameters.is_empty())
            {
                continue;
            }
            let ty = ResolvedType::Nominal {
                declaration,
                arguments: Vec::new(),
            };
            if self
                .compute_type_facts(&ty, &mut BTreeSet::new(), &mut memo)
                .is_none()
            {
                return false;
            }
        }
        #[cfg(test)]
        TYPE_FACTS_OUTER_BASELINE.with(|baseline| baseline.set(0));
        self.type_facts_by_id = memo;
        true
    }

    pub(super) fn recompute_type_facts(&self, ty: &ResolvedType) -> Option<TypeFacts> {
        self.compute_type_facts(ty, &mut BTreeSet::new(), &mut BTreeMap::new())
    }

    pub(super) fn from_verified(program: &Program) -> Result<Self, Diagnostic> {
        let mut index = Self::default();
        for declaration in program.types.iter().chain(crate::prelude::declarations()) {
            let kind = match declaration.kind {
                TypeDeclarationKind::Resource { .. } => DeclarationKind::Resource,
                TypeDeclarationKind::Record { .. } => DeclarationKind::Record,
                TypeDeclarationKind::Class { .. } => DeclarationKind::Class,
                TypeDeclarationKind::Variant { .. } => DeclarationKind::Variant,
            };
            index.insert_top_level(
                declaration.name.clone(),
                DeclarationId::new(declaration.stable_id.clone()),
                kind,
                if crate::prelude::is_compiler_owned_id(&declaration.stable_id) {
                    IdentityOrigin::CompilerOwned
                } else if declaration.explicit_id {
                    IdentityOrigin::Explicit
                } else {
                    IdentityOrigin::Automatic
                },
            );
            let owner = DeclarationId::new(declaration.stable_id.clone());
            let parameters = declaration
                .type_parameters
                .iter()
                .enumerate()
                .map(|(ordinal, parameter)| {
                    Ok(ResolvedTypeParameterDeclaration {
                        name: parameter.name.clone(),
                        index: u32::try_from(ordinal).map_err(|_| {
                            Diagnostic::error(
                                "SPX-H006",
                                format!("type `{}` has too many parameters", declaration.name),
                                declaration.span,
                            )
                            .at_path(&program.path)
                        })?,
                        span: parameter.span,
                    })
                })
                .collect::<Result<Vec<_>, Diagnostic>>()?;
            index.type_parameters.insert(owner, parameters);
        }
        for interface in &program.interfaces {
            let interface_id = DeclarationId::new(interface.stable_id.clone());
            index.insert_top_level(
                interface.name.clone(),
                interface_id.clone(),
                DeclarationKind::Interface,
                IdentityOrigin::Explicit,
            );
            for import in &interface.imports {
                let import_id = DeclarationId::new(import.stable_id.clone());
                index
                    .imports_by_key
                    .insert(import.stable_id.clone(), import_id.clone());
                if import.native_rust {
                    index
                        .native_rust_imports_by_name
                        .insert(import.name.clone(), import_id.clone());
                }
                index.insert_owned_declaration(
                    interface_id.clone(),
                    import.name.clone(),
                    import_id,
                    DeclarationKind::Import,
                    IdentityOrigin::Explicit,
                );
            }
        }
        for declaration in program.types.iter().chain(crate::prelude::declarations()) {
            let TypeDeclarationKind::Resource { lifecycles } = &declaration.kind else {
                continue;
            };
            if let [lifecycle] = lifecycles.as_slice() {
                let lifecycle_id = lifecycle
                    .stable_id
                    .as_ref()
                    .expect("verified lifecycle has an explicit identity");
                index.insert_owned_declaration(
                    DeclarationId::new(declaration.stable_id.clone()),
                    "drop".to_owned(),
                    DeclarationId::new(lifecycle_id.clone()),
                    DeclarationKind::ResourceDrop,
                    IdentityOrigin::Explicit,
                );
            }
        }
        for function in &program.functions {
            let owner = DeclarationId::new(function.stable_id.clone());
            index.insert_top_level(
                function.name.clone(),
                owner.clone(),
                DeclarationKind::Function,
                if function.explicit_id {
                    IdentityOrigin::Explicit
                } else {
                    IdentityOrigin::Automatic
                },
            );
            let parameters = function
                .type_parameters
                .iter()
                .enumerate()
                .map(|(ordinal, parameter)| {
                    Ok(ResolvedTypeParameterDeclaration {
                        name: parameter.name.clone(),
                        index: u32::try_from(ordinal).map_err(|_| {
                            Diagnostic::error(
                                "SPX-H006",
                                format!("function `{}` has too many parameters", function.name),
                                function.span,
                            )
                            .at_path(&program.path)
                        })?,
                        span: parameter.span,
                    })
                })
                .collect::<Result<Vec<_>, Diagnostic>>()?;
            index.type_parameters.insert(owner, parameters);
        }
        if program
            .interfaces
            .iter()
            .flat_map(|interface| &interface.imports)
            .filter(|import| import.native_rust)
            .any(|import| index.functions_by_name.contains_key(&import.name))
        {
            return Err(Diagnostic::error(
                "SPX-B107",
                "Native Rust Interop declaration set is unsupported: symbol collision",
                Span::default(),
            )
            .at_path(&program.path));
        }
        for declaration in program.types.iter().chain(crate::prelude::declarations()) {
            let (TypeDeclarationKind::Record { fields }
            | TypeDeclarationKind::Class { fields, .. }) = &declaration.kind
            else {
                continue;
            };
            let owner = DeclarationId::new(declaration.stable_id.clone());
            let mut resolved_fields = Vec::with_capacity(fields.len());
            for (ordinal, field) in fields.iter().enumerate() {
                let ty = index
                    .resolve_source_type(&field.ty, Some(&owner))
                    .ok_or_else(|| {
                        Diagnostic::error(
                            "SPX-H001",
                            format!("unresolved field type `{}`", field.ty),
                            field.span,
                        )
                        .at_path(&program.path)
                    })?;
                let field_index = u32::try_from(ordinal).map_err(|_| {
                    Diagnostic::error(
                        "SPX-H006",
                        format!("record `{}` has too many fields", declaration.name),
                        declaration.span,
                    )
                    .at_path(&program.path)
                })?;
                let resolved = ResolvedFieldDeclaration {
                    id: DeclarationId::new(field.stable_id.clone()),
                    name: field.name.clone(),
                    index: field_index,
                    ty,
                    span: field.span,
                };
                index.insert_field(
                    owner.clone(),
                    resolved.clone(),
                    if field.explicit_id {
                        IdentityOrigin::Explicit
                    } else {
                        IdentityOrigin::Automatic
                    },
                );
                resolved_fields.push(resolved);
            }
            index.record_fields.insert(owner, resolved_fields);
        }
        for declaration in program.types.iter() {
            let TypeDeclarationKind::Class { fields, methods } = &declaration.kind else {
                continue;
            };
            let owner = DeclarationId::new(declaration.stable_id.clone());
            let mut resolved_fields = Vec::with_capacity(fields.len());
            for (ordinal, field) in fields.iter().enumerate() {
                let ty = index
                    .resolve_source_type(&field.ty, Some(&owner))
                    .ok_or_else(|| {
                        Diagnostic::error(
                            "SPX-H001",
                            format!("unresolved class field type `{}`", field.ty),
                            field.span,
                        )
                        .at_path(&program.path)
                    })?;
                let field_index = u32::try_from(ordinal).map_err(|_| {
                    Diagnostic::error(
                        "SPX-H006",
                        format!("class `{}` has too many fields", declaration.name),
                        declaration.span,
                    )
                    .at_path(&program.path)
                })?;
                let resolved = ResolvedFieldDeclaration {
                    id: DeclarationId::new(field.stable_id.clone()),
                    name: field.name.clone(),
                    index: field_index,
                    ty,
                    span: field.span,
                };
                index.insert_field(
                    owner.clone(),
                    resolved.clone(),
                    if field.explicit_id {
                        IdentityOrigin::Explicit
                    } else {
                        IdentityOrigin::Automatic
                    },
                );
                resolved_fields.push(resolved);
            }
            index.record_fields.insert(owner.clone(), resolved_fields);
            for method in methods {
                let method_id = DeclarationId::new(method.stable_id.clone());
                if index.declarations.contains_key(&method_id) {
                    return Err(Diagnostic::error(
                        "SPX-S102",
                        format!("duplicate stable id `{}`", method.stable_id),
                        method.span,
                    )
                    .at_path(&program.path));
                }
                index.insert_owned_declaration(
                    owner.clone(),
                    method.name.clone(),
                    method_id,
                    DeclarationKind::Function,
                    if method.explicit_id {
                        IdentityOrigin::Explicit
                    } else {
                        IdentityOrigin::Automatic
                    },
                );
                if !method.type_parameters.is_empty() {
                    return Err(Diagnostic::error(
                        "SPX-T224",
                        format!(
                            "class method `{}` cannot declare generic parameters in this slice",
                            method.name
                        ),
                        method.span,
                    )
                    .at_path(&program.path));
                }
                index
                    .type_parameters
                    .insert(DeclarationId::new(method.stable_id.clone()), Vec::new());
            }
        }
        for declaration in program.types.iter().chain(crate::prelude::declarations()) {
            let TypeDeclarationKind::Variant { cases } = &declaration.kind else {
                continue;
            };
            let owner = DeclarationId::new(declaration.stable_id.clone());
            let mut resolved_cases = Vec::with_capacity(cases.len());
            for (case_ordinal, case) in cases.iter().enumerate() {
                let case_id = DeclarationId::new(case.stable_id.clone());
                let case_index = u32::try_from(case_ordinal).map_err(|_| {
                    Diagnostic::error(
                        "SPX-H006",
                        format!("variant `{}` has too many cases", declaration.name),
                        declaration.span,
                    )
                    .at_path(&program.path)
                })?;
                index.insert_case(
                    owner.clone(),
                    case.name.clone(),
                    case_id.clone(),
                    if crate::prelude::is_compiler_owned_id(&case.stable_id) {
                        IdentityOrigin::CompilerOwned
                    } else if case.explicit_id {
                        IdentityOrigin::Explicit
                    } else {
                        IdentityOrigin::Automatic
                    },
                );
                let mut resolved_fields = Vec::with_capacity(case.fields.len());
                for (field_ordinal, field) in case.fields.iter().enumerate() {
                    let ty = index
                        .resolve_source_type(&field.ty, Some(&owner))
                        .ok_or_else(|| {
                            Diagnostic::error(
                                "SPX-H001",
                                format!("unresolved case field type `{}`", field.ty),
                                field.span,
                            )
                            .at_path(&program.path)
                        })?;
                    let field_index = u32::try_from(field_ordinal).map_err(|_| {
                        Diagnostic::error(
                            "SPX-H006",
                            format!(
                                "variant case `{}::{}` has too many fields",
                                declaration.name, case.name
                            ),
                            case.span,
                        )
                        .at_path(&program.path)
                    })?;
                    let resolved = ResolvedFieldDeclaration {
                        id: DeclarationId::new(field.stable_id.clone()),
                        name: field.name.clone(),
                        index: field_index,
                        ty,
                        span: field.span,
                    };
                    index.insert_case_field(
                        case_id.clone(),
                        resolved.clone(),
                        if crate::prelude::is_compiler_owned_id(&field.stable_id) {
                            IdentityOrigin::CompilerOwned
                        } else if field.explicit_id {
                            IdentityOrigin::Explicit
                        } else {
                            IdentityOrigin::Automatic
                        },
                    );
                    resolved_fields.push(resolved);
                }
                index
                    .case_fields
                    .insert(case_id.clone(), resolved_fields.clone());
                resolved_cases.push(ResolvedVariantCaseDeclaration {
                    id: case_id,
                    name: case.name.clone(),
                    index: case_index,
                    fields: resolved_fields,
                    span: case.span,
                });
            }
            index.variant_cases.insert(owner, resolved_cases);
        }
        index.materialize_class_inheritance(program)?;
        if !index.populate_type_facts() {
            return Err(Diagnostic::error(
                "SPX-T217",
                "record declarations contain an illegal by-value recursive layout",
                Span::default(),
            )
            .at_path(&program.path));
        }
        Ok(index)
    }

    /// Class Inheritance v1: resolves `extends` links, rejects unknown or
    /// non-class parents and inheritance cycles, then prepends each class's
    /// inherited field prefix so layouts, semantic facts, projections, and
    /// cleanup treat an inherited member exactly like a declared one.
    fn materialize_class_inheritance(&mut self, program: &Program) -> Result<(), Diagnostic> {
        for declaration in &program.types {
            let Some(extends) = &declaration.extends else {
                continue;
            };
            let child = DeclarationId::new(declaration.stable_id.clone());
            let Type::Named {
                name: parent_name,
                arguments: parent_arguments,
            } = extends
            else {
                return Err(Diagnostic::error(
                    "SPX-T227",
                    format!(
                        "class `{}` must extend a named class type",
                        declaration.name
                    ),
                    declaration.name_span,
                )
                .at_path(&program.path));
            };
            if !parent_arguments.is_empty() {
                return Err(Diagnostic::error(
                    "SPX-T227",
                    format!(
                        "class `{}` extends generic type `{parent_name}`; inheritance over generic classes is closed in this slice",
                        declaration.name
                    ),
                    declaration.name_span,
                )
                .at_path(&program.path));
            }
            let parent = self.types_by_name.get(parent_name).ok_or_else(|| {
                Diagnostic::error(
                    "SPX-T227",
                    format!(
                        "class `{}` extends unknown type `{parent_name}`",
                        declaration.name
                    ),
                    declaration.name_span,
                )
                .at_path(&program.path)
            })?;
            if self
                .declaration(parent)
                .is_none_or(|item| item.kind != DeclarationKind::Class)
            {
                return Err(Diagnostic::error(
                    "SPX-T227",
                    format!(
                        "class `{}` extends non-class type `{parent_name}`",
                        declaration.name
                    ),
                    declaration.name_span,
                )
                .at_path(&program.path));
            }
            self.class_parents.insert(child, parent.clone());
        }
        for declaration in &program.types {
            if !matches!(declaration.kind, TypeDeclarationKind::Class { .. }) {
                continue;
            }
            let child = DeclarationId::new(declaration.stable_id.clone());
            let mut visited = BTreeSet::new();
            let mut cursor = child.clone();
            while let Some(parent) = self.class_parents.get(&cursor) {
                if parent == &child || !visited.insert(parent.clone()) {
                    return Err(Diagnostic::error(
                        "SPX-T228",
                        format!(
                            "class `{}` participates in an inheritance cycle",
                            declaration.name
                        ),
                        declaration.span,
                    )
                    .at_path(&program.path));
                }
                cursor = parent.clone();
            }
        }
        // Materialize effective fields root-first so a child's prefix equals
        // its standalone ancestor layout at every depth. Declared-only lists
        // are snapshotted first because ancestors are materialized in place.
        let mut declared_fields = BTreeMap::new();
        for declaration in &program.types {
            if matches!(declaration.kind, TypeDeclarationKind::Class { .. }) {
                let id = DeclarationId::new(declaration.stable_id.clone());
                if let Some(fields) = self.record_fields(&id) {
                    declared_fields.insert(id, fields.to_vec());
                }
            }
        }
        for declaration in &program.types {
            let TypeDeclarationKind::Class { .. } = &declaration.kind else {
                continue;
            };
            let child = DeclarationId::new(declaration.stable_id.clone());
            let mut chain = self.class_ancestors(&child);
            chain.reverse();
            chain.push(child.clone());
            let mut effective = Vec::new();
            let mut seen_names = BTreeSet::new();
            let mut seen_ids = BTreeSet::new();
            for member in &chain {
                let is_child = member == &child;
                let fields = declared_fields.get(member).ok_or_else(|| {
                    Diagnostic::error(
                        "SPX-H006",
                        format!("class `{member}` has no resolved fields"),
                        declaration.span,
                    )
                    .at_path(&program.path)
                })?;
                for mut field in fields.iter().cloned() {
                    if !seen_names.insert(field.name.clone()) || !seen_ids.insert(field.id.clone())
                    {
                        return Err(Diagnostic::error(
                            "SPX-T229",
                            format!(
                                "class `{}` redeclares member `{}` from an ancestor",
                                declaration.name, field.name
                            ),
                            field.span,
                        )
                        .at_path(&program.path));
                    }
                    // Effective positions are canonical for the extending
                    // class; backends consume them as declaration order.
                    field.index = u32::try_from(effective.len()).map_err(|_| {
                        Diagnostic::error(
                            "SPX-H006",
                            format!("class `{}` has too many fields", declaration.name),
                            declaration.span,
                        )
                        .at_path(&program.path)
                    })?;
                    if !is_child {
                        self.fields_by_owner_name
                            .insert((child.clone(), field.name.clone()), field.id.clone());
                    }
                    effective.push(field);
                }
            }
            self.record_fields.insert(child, effective);
        }
        Ok(())
    }

    pub(super) fn insert_top_level(
        &mut self,
        name: String,
        id: DeclarationId,
        kind: DeclarationKind,
        identity_origin: IdentityOrigin,
    ) {
        match kind {
            DeclarationKind::Resource
            | DeclarationKind::Record
            | DeclarationKind::Class
            | DeclarationKind::Variant => {
                self.types_by_name.insert(name.clone(), id.clone());
            }
            DeclarationKind::Function => {
                self.functions_by_name.insert(name.clone(), id.clone());
            }
            DeclarationKind::Interface => {}
            DeclarationKind::ResourceDrop
            | DeclarationKind::Import
            | DeclarationKind::Field
            | DeclarationKind::VariantCase
            | DeclarationKind::CaseField => {
                unreachable!("owned declarations use owner-scoped insertion")
            }
        }
        self.declarations.insert(
            id.clone(),
            Declaration {
                id,
                name,
                kind,
                identity_origin,
                owner: None,
            },
        );
    }

    fn insert_owned_declaration(
        &mut self,
        owner: DeclarationId,
        name: String,
        id: DeclarationId,
        kind: DeclarationKind,
        identity_origin: IdentityOrigin,
    ) {
        self.declarations.insert(
            id.clone(),
            Declaration {
                id,
                name,
                kind,
                identity_origin,
                owner: Some(owner),
            },
        );
    }

    fn insert_field(
        &mut self,
        owner: DeclarationId,
        field: ResolvedFieldDeclaration,
        identity_origin: IdentityOrigin,
    ) {
        self.fields_by_owner_name
            .insert((owner.clone(), field.name.clone()), field.id.clone());
        self.declarations.insert(
            field.id.clone(),
            Declaration {
                id: field.id,
                name: field.name,
                kind: DeclarationKind::Field,
                identity_origin,
                owner: Some(owner),
            },
        );
    }

    fn insert_case(
        &mut self,
        owner: DeclarationId,
        name: String,
        id: DeclarationId,
        identity_origin: IdentityOrigin,
    ) {
        self.cases_by_owner_name
            .insert((owner.clone(), name.clone()), id.clone());
        self.declarations.insert(
            id.clone(),
            Declaration {
                id,
                name,
                kind: DeclarationKind::VariantCase,
                identity_origin,
                owner: Some(owner),
            },
        );
    }

    fn insert_case_field(
        &mut self,
        owner: DeclarationId,
        field: ResolvedFieldDeclaration,
        identity_origin: IdentityOrigin,
    ) {
        self.fields_by_owner_name
            .insert((owner.clone(), field.name.clone()), field.id.clone());
        self.declarations.insert(
            field.id.clone(),
            Declaration {
                id: field.id,
                name: field.name,
                kind: DeclarationKind::CaseField,
                identity_origin,
                owner: Some(owner),
            },
        );
    }

    fn resolve_source_type(
        &self,
        ty: &Type,
        parameter_owner: Option<&DeclarationId>,
    ) -> Option<ResolvedType> {
        enum Frame<'a> {
            Enter(&'a Type),
            Finish(DeclarationId, usize),
        }
        let mut frames = vec![Frame::Enter(ty)];
        let mut resolved = Vec::new();
        while let Some(frame) = frames.pop() {
            match frame {
                Frame::Enter(ty) => match ty {
                    Type::I64 => resolved.push(ResolvedType::I64),
                    Type::I32 => resolved.push(ResolvedType::I32),
                    Type::Char => resolved.push(ResolvedType::Char),
                    Type::U8 => resolved.push(ResolvedType::U8),
                    Type::Usize => resolved.push(ResolvedType::Usize),
                    Type::ArrayU8(length) => resolved.push(ResolvedType::ArrayU8(*length)),
                    Type::F32 => resolved.push(ResolvedType::F32),
                    Type::F64 => resolved.push(ResolvedType::F64),
                    Type::Bool => resolved.push(ResolvedType::Bool),
                    Type::String => resolved.push(ResolvedType::String),
                    Type::Bytes => resolved.push(ResolvedType::Bytes),
                    Type::Str => resolved.push(ResolvedType::Str),
                    Type::SliceU8 => resolved.push(ResolvedType::SliceU8),
                    Type::Named { name, arguments } => {
                        if arguments.is_empty() {
                            if let Some(owner) = parameter_owner {
                                if let Some(parameter) = self
                                    .type_parameters(owner)?
                                    .iter()
                                    .find(|parameter| parameter.name == *name)
                                {
                                    resolved.push(ResolvedType::TypeParameter {
                                        owner: owner.clone(),
                                        index: parameter.index,
                                    });
                                    continue;
                                }
                            }
                        }
                        frames.push(Frame::Finish(self.type_id(name)?.clone(), arguments.len()));
                        frames.extend(arguments.iter().rev().map(Frame::Enter));
                    }
                },
                Frame::Finish(declaration, count) => {
                    let split = resolved.len().checked_sub(count)?;
                    let arguments = resolved.drain(split..).collect();
                    resolved.push(ResolvedType::Nominal {
                        declaration,
                        arguments,
                    });
                }
            }
        }
        (resolved.len() == 1).then(|| resolved.pop().expect("type count checked above"))
    }
}

/// Consumes an opaque declaration index while moving every recursive type
/// through the caller's bounded iterative disposer.
#[doc(hidden)]
pub fn dispose_declaration_index_for_private_contract(
    mut index: DeclarationIndex,
    dispose: impl FnMut(ResolvedType),
) {
    index.drain_recursive_types_for_private_contract(dispose);
}

impl Drop for DeclarationIndex {
    fn drop(&mut self) {
        let mut pending = Vec::new();
        self.drain_recursive_types_for_private_contract(|ty| pending.push(ty));
        while let Some(ty) = pending.pop() {
            if let ResolvedType::Nominal { arguments, .. } = ty {
                pending.extend(arguments);
            }
        }
    }
}
