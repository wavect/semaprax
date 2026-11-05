//! Deterministic declaration lookup, type facts, and inheritance materialization.
use super::*;
use super::nominal_payload::map::{CaseFields, IndexMap};

mod flags;
mod nominal;
mod nominal_base;
mod nominal_construction;
mod nominal_paid_values;
mod nominal_paid_inheritance;
mod nominal_rollback;
#[cfg(test)]
pub(crate) mod nominal_original;
pub(crate) use flags::{inventory_identity, inventory_substitute, InventoryPayer};
pub(crate) use flags::{TypeFactsPayer, TypeFlags};
pub(super) mod identity_payer;
mod inventory_lookup;
mod linked_owned_data;
mod owned_builtin;
mod owner_name;
mod primitive_replay;
mod projection;
mod source;
use owned_builtin::owned_builtin_facts;
/// A deterministic, display-name-to-identity index.
///
/// Types and values occupy distinct namespaces so future record/variant type
/// declarations can coexist with functions without ambiguous lookup.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct DeclarationIndex {
    pub(super) nominal_drop_workspace: nominal_rollback::Workspace,
    pub(super) declarations: BTreeMap<DeclarationId, Declaration>,
    pub(super) types_by_name: BTreeMap<String, DeclarationId>,
    pub(super) functions_by_name: BTreeMap<String, DeclarationId>,
    pub(super) fields_by_owner_name: BTreeMap<(DeclarationId, String), DeclarationId>,
    pub(super) record_fields: IndexMap<NominalPayload<ResolvedFieldDeclaration>>,
    pub(super) cases_by_owner_name: BTreeMap<(DeclarationId, String), DeclarationId>,
    pub(super) variant_cases: IndexMap<NominalPayload<ResolvedVariantCaseDeclaration>>,
    pub(super) case_fields: IndexMap<CaseFields>,
    pub(super) type_parameters: IndexMap<NominalPayload<ResolvedTypeParameterDeclaration>>,
    pub(super) imports_by_key: BTreeMap<String, DeclarationId>,
    pub(super) native_rust_imports_by_name: BTreeMap<String, DeclarationId>,
    pub(super) type_facts_by_id: BTreeMap<String, TypeFacts>,
    /// Class Inheritance v1: the declared parent of each extending class.
    /// Classes without `extends` have no entry.
    pub(super) class_parents: BTreeMap<DeclarationId, DeclarationId>,
    pub(super) byte_slice_roots: BTreeMap<ValueId, ByteSliceProvenance>,
}
impl DeclarationIndex {
    pub(super) fn contains_declaration_id(&self, id: &DeclarationId) -> bool {
        self.declarations.contains_key(id)
    }

    /// Moves every recursively nested resolved type out of the declaration
    /// index before ordinary field drop glue runs. Private bounded owners use
    /// this hook to feed their existing preallocated iterative disposer.
    fn drain_recursive_types_for_private_contract(
        &mut self,
        mut dispose: impl FnMut(ResolvedType),
    ) {
        // Release case views before asking whether the outer case owner is
        // unique. Legacy full field inventories retain their original roots.
        for (_, fields) in std::mem::take(&mut self.case_fields) {
            fields.drain(&mut dispose);
        }
        for (_, fields) in std::mem::take(&mut self.record_fields) {
            fields.drain_recursive_types(&mut dispose);
        }
        for (_, cases) in std::mem::take(&mut self.variant_cases) {
            cases.drain_recursive_types(&mut dispose);
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
            .values().map(|values|values.as_slice())
            .chain(self.case_fields.values().map(|values|values.as_slice()))
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
            .iter().map(|(owner,fields)|(owner,fields.capacity()))
            .chain(self.case_fields.iter().map(|(owner,fields)|(owner,fields.capacity())))
            .map(|(owner, capacity)| {
                std::mem::size_of::<(DeclarationId, NominalPayload<ResolvedFieldDeclaration>)>()
                    + owner.as_str().len()
                    + capacity * std::mem::size_of::<ResolvedFieldDeclaration>()
            })
            .sum::<usize>();
        let variant_case_map = self
            .variant_cases
            .iter()
            .map(|(owner, cases)| {
                std::mem::size_of::<(DeclarationId, NominalPayload<ResolvedVariantCaseDeclaration>)>()
                    + owner.as_str().len()
                    + cases.capacity() * std::mem::size_of::<ResolvedVariantCaseDeclaration>()
            })
            .sum::<usize>();
        let type_parameter_map = self
            .type_parameters
            .iter()
            .map(|(owner, parameters)| {
                std::mem::size_of::<(DeclarationId, NominalPayload<ResolvedTypeParameterDeclaration>)>()
                    + owner.as_str().len()
                    + parameters.capacity()
                        * std::mem::size_of::<ResolvedTypeParameterDeclaration>()
                    + parameters
                        .iter()
                        .map(|parameter| parameter.name.capacity())
                        .sum::<usize>()
            })
            .sum::<usize>();
        let byte_slice_provenance_map = self
            .byte_slice_roots
            .iter()
            .map(|(value, provenance)| {
                std::mem::size_of::<(ValueId, ByteSliceProvenance)>()
                    + value.as_str().len()
                    + provenance.root.as_str().len()
                    + provenance.projections.capacity() * std::mem::size_of::<PlaceProjection>()
                    + provenance
                        .projections
                        .iter()
                        .map(place_projection_owned_capacity)
                        .sum::<usize>()
                    + super::capacity_probe::vector_field_owned_capacity(provenance)
                    + resolved_type_owned_capacity(&provenance.projected_type)
                    + provenance.ranges.capacity() * std::mem::size_of::<ByteSliceRangeStep>()
                    + provenance
                        .ranges
                        .iter()
                        .map(|range| {
                            range.source.as_str().len()
                                + range.producer.as_str().len()
                                + range.start.as_str().len()
                                + range.end.as_str().len()
                        })
                        .sum::<usize>()
            })
            .sum::<usize>();
        declaration_map_backing
            + record_field_maps
            + variant_case_map
            + type_parameter_map
            + byte_slice_provenance_map
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
        owner_name::get(&self.fields_by_owner_name, owner, name)
    }

    pub fn record_fields(&self, owner: &DeclarationId) -> Option<&[ResolvedFieldDeclaration]> {
        self.record_fields.get(owner).map(|values| values.as_slice())
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
        owner_name::get(&self.cases_by_owner_name, owner, name)
    }

    pub fn variant_cases(
        &self,
        owner: &DeclarationId,
    ) -> Option<&[ResolvedVariantCaseDeclaration]> {
        self.variant_cases.get(owner).map(|values| values.as_slice())
    }

    pub fn case_fields(&self, case: &DeclarationId) -> Option<&[ResolvedFieldDeclaration]> {
        if let Some(fields) = self.case_fields.get(case) {
            return Some(fields.as_slice());
        }
        if !self.nominal_drop_workspace.building { return None; }
        // During canonical construction the same complete semantic metadata
        // is available before any shared allocation is attempted.
        let declaration = self.declarations.get(case)?;
        if declaration.kind != DeclarationKind::VariantCase { return None; }
        let owner = declaration.owner.as_ref()?;
        self.variant_cases.get(owner)?.iter().find(|row| row.id == *case).map(|row| row.fields.as_slice())
    }

    pub fn type_parameters(
        &self,
        declaration: &DeclarationId,
    ) -> Option<&[ResolvedTypeParameterDeclaration]> {
        self.type_parameters.get(declaration).map(|values| values.as_slice())
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
    /// `None` covers unresolved type parameters, malformed HIR, and the
    /// non-storable `Never` completion marker. Verified value types have facts.
    pub fn type_facts(&self, ty: &ResolvedType) -> Option<TypeFacts> {
        #[cfg(test)]
        if flags::old_key_enabled() {
            return self.type_facts_owned_key_oracle(ty);
        }
        if let Some(facts) = flags::cached_facts(self, ty) {
            return facts.cloned().or_else(|| self.recompute_type_facts(ty));
        }
        self.type_facts_by_id
            .get(&ty.identity_key())
            .cloned()
            .or_else(|| self.recompute_type_facts(ty))
    }

    fn compute_type_facts(
        &self,
        ty: &ResolvedType,
        visiting: &mut BTreeSet<String>,
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
            visiting: &BTreeSet<String>,
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
                    * (std::mem::size_of::<String>() + std::mem::size_of::<BTreeSet<String>>())
                + visiting.iter().map(String::capacity).sum::<usize>()
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
                    if crate::map_ops::is_typed_collection(&ty) {
                        results.push(TypeFacts {
                            copy: false,
                            contains_resource: false,
                            sized: true,
                            needs_drop: true,
                            layout_key: format!("owned:collection:{}", ty.identity_key()),
                        });
                        continue;
                    }
                    let scalar = match &ty {
                        ResolvedType::Function { .. } => {
                            if !super::function_value::is_signature(&ty) {
                                return None;
                            }
                            results.push(TypeFacts {
                                copy: true,
                                contains_resource: false,
                                sized: true,
                                needs_drop: false,
                                layout_key: identity.clone(),
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
                    let native_owned_container =
                        super::ResolvedImportResultKind::is_owned_container_type(&ty, self)
                            || owned_builtin::resource_result_shape(self, &ty);
                    let authored_owned_byte_variant =
                        super::type_reachability::is_admitted_concrete_owned_byte_variant(
                            self, &ty,
                        );
                    let ResolvedType::Nominal {
                        declaration,
                        arguments,
                    } = ty
                    else {
                        return None;
                    };
                    let item = self.declaration(&declaration)?;
                    if let Some(facts) = owned_builtin_facts(self, &declaration, &arguments) {
                        memo.insert(identity, facts.clone());
                        results.push(facts);
                        continue;
                    }
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
                    visiting.remove(&identity);
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
                            let mut copy = true;
                            let mut contains_resource = false;
                            let mut sized = true;
                            let mut needs_drop = false;
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
                            }
                            #[cfg(test)]
                            let encoded_capacity = encoded.allocated_capacity();
                            let facts = TypeFacts {
                                copy,
                                contains_resource,
                                sized,
                                needs_drop,
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

    /// Ordinary compatibility construction is explicitly unfunded. The
    /// shared implementation never discovers a payer through ambient state.
    pub(super) fn from_verified_with_prelude(
        program: &dyn ProgramSource,
        prelude: &[crate::ast::TypeDeclaration],
    ) -> Result<Self, Diagnostic> {
        Self::from_verified_with_identity_payer(program, prelude, &mut |text| {
            Ok::<_, std::convert::Infallible>(DeclarationId::new(text))
        })
        .map_err(|error| match error {
            identity_payer::IdentityIndexError::Semantic(error) => error,
            identity_payer::IdentityIndexError::Identity(impossible) => match impossible {},
        })
    }

    /// Class Inheritance v1: resolves `extends` links, rejects unknown or
    /// non-class parents and inheritance cycles, then prepends each class's
    /// inherited field prefix so layouts, semantic facts, projections, and
    /// cleanup treat an inherited member exactly like a declared one.
    fn materialize_class_inheritance(
        &mut self,
        program: &dyn ProgramSource,
    ) -> Result<(), Diagnostic> {
        for declaration in program.types() {
            let Some(extends) = &declaration.extends else {
                continue;
            };
            let child = self.inserted_identity(&declaration.stable_id);
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
                .at_path(program.path()));
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
                .at_path(program.path()));
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
                .at_path(program.path())
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
                .at_path(program.path()));
            }
            self.class_parents.insert(child, parent.clone());
        }
        for declaration in program.types() {
            if !matches!(declaration.kind, TypeDeclarationKind::Class { .. }) {
                continue;
            }
            let child = self.inserted_identity(&declaration.stable_id);
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
                    .at_path(program.path()));
                }
                cursor = parent.clone();
            }
        }
        // Materialize effective fields root-first so a child's prefix equals
        // its standalone ancestor layout at every depth. Declared-only lists
        // are snapshotted first because ancestors are materialized in place.
        let mut declared_fields = BTreeMap::new();
        for declaration in program.types() {
            if matches!(declaration.kind, TypeDeclarationKind::Class { .. }) {
                let id = self.inserted_identity(&declaration.stable_id);
                if let Some(fields) = self.record_fields(&id) {
                    declared_fields.insert(id, fields.to_vec());
                }
            }
        }
        for declaration in program.types() {
            let TypeDeclarationKind::Class { .. } = &declaration.kind else {
                continue;
            };
            let child = self.inserted_identity(&declaration.stable_id);
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
                    .at_path(program.path())
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
                        .at_path(program.path()));
                    }
                    // Effective positions are canonical for the extending
                    // class; backends consume them as declaration order.
                    field.index = u32::try_from(effective.len()).map_err(|_| {
                        Diagnostic::error(
                            "SPX-H006",
                            format!("class `{}` has too many fields", declaration.name),
                            declaration.span,
                        )
                        .at_path(program.path())
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

    #[cfg(test)]
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

    #[cfg(test)]
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
                    ty @ Type::Function { .. } => {
                        resolved.push(super::function_value::resolve::source_type(ty)?)
                    }
                    Type::Never => return None,
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
                    Type::OnceFunction => resolved.push(ResolvedType::OnceFunction),
                    Type::OnceFunctionI64 => resolved.push(ResolvedType::OnceFunctionI64),
                    Type::OnceFunctionI64Pair => resolved.push(ResolvedType::OnceFunctionI64Pair),
                    Type::MutFunctionI64 => resolved.push(ResolvedType::MutFunctionI64),
                    Type::Str => resolved.push(ResolvedType::Str),
                    Type::SliceU8 => resolved.push(ResolvedType::SliceU8),
                    Type::StringMap => resolved.push(ResolvedType::StringMap),
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

mod legacy_drop;

impl Drop for DeclarationIndex {
    fn drop(&mut self) {
        if let Some(scratch) = self.nominal_drop_workspace.scratch.take() {
            let mut scratch=scratch.lock().expect("construction disposal lock");
            self.drain_recursive_types_for_private_contract(|ty| nominal_payload::dispose(ty, &mut scratch));
            return;
        }
        self.drain_legacy_type_roots(legacy_drop::dispose);
    }
}

#[cfg(test)]
mod leaf_drop_tests;

// Keep this small exact-key receiver inline so existing source-locked readers
// continue covering the entire declaration-map lookup contract.
mod invariant_lookup {
    use super::{DeclarationId, DeclarationIndex};
    use std::borrow::Borrow;
    use std::cmp::Ordering;

    type Bytes<'a> = std::iter::Chain<
        std::iter::Copied<std::slice::Iter<'a, u8>>,
        std::iter::Copied<std::slice::Iter<'a, u8>>,
    >;
    trait Key {
        fn bytes(&self) -> Bytes<'_>;
    }
    impl Key for DeclarationId {
        fn bytes(&self) -> Bytes<'_> {
            self.as_str()
                .as_bytes()
                .iter()
                .copied()
                .chain([].iter().copied())
        }
    }
    // DeclarationId's Eq/Ord are exact text equality/UTF-8 byte ordering.
    // The borrowed trait object must use that same order for EVERY stored id.
    impl<'a> Borrow<dyn Key + 'a> for DeclarationId {
        fn borrow(&self) -> &(dyn Key + 'a) {
            self
        }
    }
    impl PartialEq for dyn Key + '_ {
        fn eq(&self, other: &Self) -> bool {
            self.cmp(other) == Ordering::Equal
        }
    }
    impl Eq for dyn Key + '_ {}
    impl PartialOrd for dyn Key + '_ {
        fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
            Some(self.cmp(other))
        }
    }
    impl Ord for dyn Key + '_ {
        fn cmp(&self, other: &Self) -> Ordering {
            self.bytes().cmp(other.bytes())
        }
    }
    struct Invariant<'a>(&'a DeclarationId);
    impl Key for Invariant<'_> {
        fn bytes(&self) -> Bytes<'_> {
            self.0
                .as_str()
                .as_bytes()
                .iter()
                .copied()
                .chain(b"#invariant".iter().copied())
        }
    }
    impl DeclarationIndex {
        /// Exact reserved declaration presence, with the existing map's lookup
        /// and arbitrary stored declaration precedence. No index or map scan.
        pub(crate) fn has_record_invariant(&self, declaration: &DeclarationId) -> bool {
            self.declarations
                .contains_key(&Invariant(declaration) as &dyn Key)
        }
        #[cfg(test)]
        fn has_record_invariant_owned_oracle(&self, declaration: &DeclarationId) -> bool {
            self.declaration(&DeclarationId::new(format!("{declaration}#invariant")))
                .is_some()
        }
    }
    #[cfg(test)]
    mod tests {
        use super::*;
        use crate::hir::{Declaration, DeclarationKind, IdentityOrigin};
        fn declaration(id: DeclarationId, kind: DeclarationKind) -> Declaration {
            Declaration {
                id,
                name: "arbitrary stored declaration".into(),
                kind,
                identity_origin: IdentityOrigin::Explicit,
                owner: None,
            }
        }
        fn names() -> Vec<String> {
            vec![
                "".into(),
                "a".into(),
                ":".into(),
                "::#".into(),
                "#invariant".into(),
                "a#invariant".into(),
                "a#invariant#invariant".into(),
                "a:0:".into(),
                "α:0:🦀".into(),
                "é".into(),
                "e\u{301}".into(),
                "a\0b".into(),
                "a".repeat(65_536),
                "α".repeat(32_768),
            ]
        }
        #[test]
        fn suffix_lookup_matches_full_owned_key_for_arbitrary_ids_and_declarations() {
            let names = names();
            let queries: Vec<_> = names
                .iter()
                .map(|name| DeclarationId::new(name.clone()))
                .collect();
            let mut index = DeclarationIndex::default();
            for id in &queries {
                index.declarations.insert(
                    id.clone(),
                    declaration(id.clone(), DeclarationKind::Resource),
                );
                for suffix in ["#invarian", "#invariant!", "#invariant\0", "#invariantα"] {
                    let neighbor = DeclarationId::new(format!("{id}{suffix}"));
                    index.declarations.insert(
                        neighbor.clone(),
                        declaration(neighbor, DeclarationKind::Record),
                    );
                }
            }
            for query in &queries {
                assert_eq!(
                    index.has_record_invariant(query),
                    index.has_record_invariant_owned_oracle(query)
                );
            }
            // Kind/name/origin carry no authority for this presence probe: an
            // arbitrary exact stored declaration selects the original refusal.
            for (position, query) in queries.iter().enumerate() {
                let id = DeclarationId::new(format!("{query}#invariant"));
                index
                    .declarations
                    .insert(id.clone(), declaration(id, DeclarationKind::Function));
                for (probe_position, probe) in queries.iter().enumerate() {
                    assert_eq!(
                        index.has_record_invariant(probe),
                        index.has_record_invariant_owned_oracle(probe)
                    );
                    if probe_position <= position {
                        assert!(index.has_record_invariant(probe));
                    }
                }
            }
            let (_, metrics) = crate::test_allocation_observer::measure(|| {
                for query in &queries {
                    assert!(index.has_record_invariant(query));
                }
            });
            assert!(!metrics.invalid);
            assert_eq!(
                (
                    metrics.allocations,
                    metrics.allocated,
                    metrics.live,
                    metrics.freed
                ),
                (0, 0, 0, 0)
            );
            let (_, old) = crate::test_allocation_observer::measure(|| {
                for query in &queries {
                    assert!(index.has_record_invariant_owned_oracle(query));
                }
            });
            assert!(!old.invalid);
            assert!(old.allocated > 0 && old.allocations > 0);
            assert_eq!(old.live, 0);
            assert_eq!(old.allocated, old.freed);
            let (_, overflowed, used, first) =
                crate::bounded_output::with_limit_usage_refusal(0, || {
                    assert!(!crate::bounded_output::reserve_active_required(1));
                    for query in &queries {
                        assert!(index.has_record_invariant(query));
                    }
                    assert!(!crate::bounded_output::reserve_active_required(0));
                });
            assert!(overflowed);
            assert_eq!(used, 0);
            assert_eq!(first.unwrap().requested, 1);
            // Observe destruction of a complete index, separately from queries.
            let mut drop_index = DeclarationIndex::default();
            let (_, dropped) = crate::test_allocation_observer::measure(|| {
                let id = DeclarationId::new("drop#invariant");
                drop_index
                    .declarations
                    .insert(id.clone(), declaration(id, DeclarationKind::Function));
                assert!(drop_index.has_record_invariant(&DeclarationId::new("drop")));
                drop(drop_index);
            });
            assert!(!dropped.invalid);
            assert_eq!(dropped.live, 0);
            assert_eq!(dropped.allocated, dropped.freed);
        }
        #[test]
        fn streamed_order_is_exact_for_stored_keys_and_suffix_queries() {
            let names = names();
            let ids: Vec<_> = names
                .iter()
                .map(|name| DeclarationId::new(name.clone()))
                .collect();
            for left in &ids {
                for right in &ids {
                    let stored_left = left as &dyn Key;
                    let stored_right = right as &dyn Key;
                    assert_eq!(stored_left.cmp(stored_right), left.cmp(right));
                    let query = Invariant(left);
                    assert_eq!(
                        (&query as &dyn Key).cmp(stored_right),
                        format!("{left}#invariant").as_str().cmp(right.as_str())
                    );
                }
            }
        }
    }
}
