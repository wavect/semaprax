//! Declared-type lookup for source verification: record, variant, class, and
//! resource declarations, the merged class-field prefix used by inheritance,
//! and the structural type predicates built on them.

use crate::ast::{
    FieldDeclaration, Function, ImportDeclaration, InterfaceDeclaration, Program,
    ResourceLifecycleKind, Type, TypeDeclaration, TypeDeclarationKind, VariantCaseDeclaration,
};
use std::collections::{HashMap, HashSet};

/// Class Inheritance v1: resolves a method against a receiver class's
/// ancestor chain, nearest definition first. Returns the declaring class name
/// and the method. Cycle-safe; `None` for unknown classes/methods.
pub(super) fn resolve_class_method<'a>(
    types: &'a TypeTable<'a>,
    class_name: &str,
    method: &str,
) -> Option<(&'a str, &'a Function)> {
    let mut visited = HashSet::new();
    let mut cursor = class_name.to_owned();
    loop {
        let declaration = types.declaration(cursor.as_str())?;
        let TypeDeclarationKind::Class { methods, .. } = &declaration.kind else {
            return None;
        };
        if let Some(found) = methods.iter().find(|candidate| candidate.name == method) {
            return Some((declaration.name.as_str(), found));
        }
        let Type::Named { name: parent, .. } = declaration.extends.as_ref()? else {
            return None;
        };
        if !visited.insert(parent.clone()) {
            return None;
        }
        cursor = parent.clone();
    }
}

/// Class Inheritance v1: the effective member fields of `ty` — declared
/// fields for records and parentless classes, the ancestor-merged prefix list
/// for extending classes.
pub(super) fn effective_record_fields<'t>(
    types: &'t TypeTable<'t>,
    ty: &Type,
) -> Option<&'t [FieldDeclaration]> {
    let Type::Named { name, .. } = ty else {
        return None;
    };
    match &types.declaration(name)?.kind {
        TypeDeclarationKind::Record { .. } => types.record_fields(ty),
        TypeDeclarationKind::Class { .. } => types
            .merged_class_fields
            .get(name.as_str())
            .map(Vec::as_slice),
        TypeDeclarationKind::Resource { .. } | TypeDeclarationKind::Variant { .. } => None,
    }
}

pub(super) struct TypeTable<'a> {
    pub(super) declarations: HashMap<&'a str, &'a TypeDeclaration>,
    declared_fields: HashMap<&'a str, HashMap<&'a str, &'a FieldDeclaration>>,
    /// Class Inheritance v1: declared parent name per extending class.
    pub(super) class_parents: HashMap<&'a str, &'a str>,
    /// Class Inheritance v1: the effective declared-field list of every
    /// extending class, root ancestor first, so construction, projection,
    /// and pattern checks treat inherited members like declared ones.
    pub(super) merged_class_fields: HashMap<&'a str, Vec<FieldDeclaration>>,
}

impl<'a> TypeTable<'a> {
    pub(super) fn new(program: &'a Program) -> Self {
        let declarations: HashMap<&'a str, &'a TypeDeclaration> = program
            .types
            .iter()
            .chain(crate::prelude::declarations())
            .map(|declaration| (declaration.name.as_str(), declaration))
            .collect();
        let mut merged_class_fields = HashMap::new();
        let mut declared_fields = HashMap::new();
        for (name, declaration) in &declarations {
            if let TypeDeclarationKind::Record { fields }
            | TypeDeclarationKind::Class { fields, .. } = &declaration.kind
            {
                declared_fields.insert(
                    *name,
                    fields
                        .iter()
                        .map(|field| (field.name.as_str(), field))
                        .collect(),
                );
            }
        }
        for (name, declaration) in &declarations {
            if !matches!(declaration.kind, TypeDeclarationKind::Class { .. }) {
                continue;
            }
            let mut chain = Vec::new();
            let mut visited = HashSet::new();
            let mut cursor = *name;
            let rooted = loop {
                let Some(ancestor) = declarations.get(cursor).copied() else {
                    break false;
                };
                if !matches!(ancestor.kind, TypeDeclarationKind::Class { .. })
                    || !visited.insert(cursor)
                {
                    break false;
                }
                chain.push(ancestor);
                let Some(Type::Named {
                    name: parent_name, ..
                }) = ancestor.extends.as_ref()
                else {
                    break true;
                };
                cursor = parent_name.as_str();
            };
            if !rooted {
                continue;
            }
            let mut fields = Vec::new();
            for ancestor in chain.into_iter().rev() {
                let TypeDeclarationKind::Class {
                    fields: declared, ..
                } = &ancestor.kind
                else {
                    unreachable!("class kind checked above")
                };
                fields.extend(declared.iter().cloned());
            }
            merged_class_fields.insert(*name, fields);
        }
        let mut class_parents = HashMap::new();
        for (name, declaration) in &declarations {
            if !matches!(declaration.kind, TypeDeclarationKind::Class { .. }) {
                continue;
            }
            if let Some(Type::Named {
                name: parent_name, ..
            }) = declaration.extends.as_ref()
            {
                class_parents.insert(*name, parent_name.as_str());
            }
        }
        Self {
            declarations,
            declared_fields,
            merged_class_fields,
            class_parents,
        }
    }

    pub(super) fn declaration(&self, name: &str) -> Option<&'a TypeDeclaration> {
        self.declarations.get(name).copied()
    }

    pub(super) fn record_fields(&self, ty: &Type) -> Option<&'a [FieldDeclaration]> {
        let Type::Named { name, .. } = ty else {
            return None;
        };
        match &self.declaration(name)?.kind {
            TypeDeclarationKind::Record { fields } | TypeDeclarationKind::Class { fields, .. } => {
                Some(fields)
            }
            TypeDeclarationKind::Resource { .. } | TypeDeclarationKind::Variant { .. } => None,
        }
    }

    pub(super) fn declared_field(
        &self,
        type_name: &str,
        field_name: &str,
    ) -> Option<&'a FieldDeclaration> {
        self.declared_fields
            .get(type_name)
            .and_then(|fields| fields.get(field_name))
            .copied()
    }

    pub(super) fn record_field_type(
        &self,
        instance: &Type,
        field: &FieldDeclaration,
    ) -> Option<Type> {
        let Type::Named { name, arguments } = instance else {
            return None;
        };
        let declaration = self.declaration(name)?;
        Self::substitute_variant_type(declaration, arguments, &field.ty)
    }

    pub(super) fn variant_cases(&self, ty: &Type) -> Option<&'a [VariantCaseDeclaration]> {
        let Type::Named { name, .. } = ty else {
            return None;
        };
        match &self.declaration(name)?.kind {
            TypeDeclarationKind::Variant { cases } => Some(cases),
            TypeDeclarationKind::Resource { .. }
            | TypeDeclarationKind::Record { .. }
            | TypeDeclarationKind::Class { .. } => None,
        }
    }

    pub(super) fn substitute_variant_type(
        declaration: &TypeDeclaration,
        arguments: &[Type],
        template: &Type,
    ) -> Option<Type> {
        enum Frame<'a> {
            Enter(&'a Type),
            Finish(&'a str, usize),
        }
        let mut frames = vec![Frame::Enter(template)];
        let mut resolved = Vec::new();
        while let Some(frame) = frames.pop() {
            match frame {
                Frame::Enter(template) => match template {
                    Type::I64 => resolved.push(Type::I64),
                    Type::I32 => resolved.push(Type::I32),
                    Type::Char => resolved.push(Type::Char),
                    Type::U8 => resolved.push(Type::U8),
                    Type::Usize => resolved.push(Type::Usize),
                    Type::ArrayU8(length) => resolved.push(Type::ArrayU8(*length)),
                    Type::F32 => resolved.push(Type::F32),
                    Type::F64 => resolved.push(Type::F64),
                    Type::Bool => resolved.push(Type::Bool),
                    Type::String => resolved.push(Type::String),
                    Type::Bytes => resolved.push(Type::Bytes),
                    Type::Str => resolved.push(Type::Str),
                    Type::SliceU8 => resolved.push(Type::SliceU8),
                    Type::Named {
                        name,
                        arguments: nested,
                    } => {
                        if nested.is_empty() {
                            if let Some(index) = declaration
                                .type_parameters
                                .iter()
                                .position(|parameter| parameter.name == *name)
                            {
                                resolved.push(arguments.get(index)?.clone());
                                continue;
                            }
                        }
                        frames.push(Frame::Finish(name, nested.len()));
                        frames.extend(nested.iter().rev().map(Frame::Enter));
                    }
                },
                Frame::Finish(name, count) => {
                    let split = resolved.len().checked_sub(count)?;
                    let nested = resolved.drain(split..).collect();
                    resolved.push(Type::Named {
                        name: name.to_owned(),
                        arguments: nested,
                    });
                }
            }
        }
        (resolved.len() == 1).then(|| resolved.pop().expect("type count checked above"))
    }

    /// Class Inheritance v1: `true` when `child_name` transitively extends
    /// `ancestor_name`. Cycle-safe.
    pub(super) fn class_extends(&self, child_name: &str, ancestor_name: &str) -> bool {
        let mut visited = HashSet::new();
        let mut cursor = child_name;
        while let Some(parent) = self.class_parents.get(cursor).copied() {
            if parent == ancestor_name {
                return true;
            }
            if !visited.insert(parent) {
                return false;
            }
            cursor = parent;
        }
        false
    }

    /// `true` when `ty` is or transitively contains an owned `string`.
    pub(super) fn contains_string(&self, ty: &Type) -> bool {
        let mut visiting = HashSet::new();
        self.contains_string_inner(ty, &mut visiting)
    }

    pub(super) fn contains_string_inner(&self, ty: &Type, visiting: &mut HashSet<String>) -> bool {
        match ty {
            Type::String => true,
            Type::I64
            | Type::I32
            | Type::Char
            | Type::U8
            | Type::Usize
            | Type::ArrayU8(_)
            | Type::F32
            | Type::F64
            | Type::Bool
            | Type::Bytes
            | Type::Str
            | Type::SliceU8 => false,
            Type::Named { name, arguments } => {
                if !visiting.insert(name.clone()) {
                    return false;
                }
                if arguments
                    .iter()
                    .any(|argument| self.contains_string_inner(argument, visiting))
                {
                    return true;
                }
                let Some(declaration) = self.declaration(name) else {
                    return false;
                };
                match &declaration.kind {
                    TypeDeclarationKind::Record { fields }
                    | TypeDeclarationKind::Class { fields, .. } => fields
                        .iter()
                        .any(|field| self.contains_string_inner(&field.ty, visiting)),
                    TypeDeclarationKind::Resource { .. } | TypeDeclarationKind::Variant { .. } => {
                        false
                    }
                }
            }
        }
    }

    /// Whether a type is or transitively contains an authored resource.
    ///
    /// This deliberately excludes compiler-owned `string` and `Bytes` values:
    /// they have drop-aware storage, but are not authored resource types and
    /// retain their existing parameter-mode rules.
    pub(super) fn contains_resource(&self, ty: &Type) -> bool {
        enum Frame {
            Enter(Type),
            Exit(String),
        }

        let mut visiting = HashSet::new();
        let mut frames = vec![Frame::Enter(ty.clone())];
        while let Some(frame) = frames.pop() {
            match frame {
                Frame::Exit(instance) => {
                    visiting.remove(&instance);
                }
                Frame::Enter(ty) => {
                    let Type::Named { name, arguments } = &ty else {
                        continue;
                    };
                    let Some(declaration) = self.declaration(name) else {
                        continue;
                    };
                    if matches!(declaration.kind, TypeDeclarationKind::Resource { .. }) {
                        return true;
                    }
                    let instance = ty.to_string();
                    if !visiting.insert(instance.clone()) {
                        return true;
                    }
                    frames.push(Frame::Exit(instance));
                    let fields: Box<dyn DoubleEndedIterator<Item = &FieldDeclaration>> =
                        match &declaration.kind {
                            TypeDeclarationKind::Record { fields }
                            | TypeDeclarationKind::Class { fields, .. } => Box::new(fields.iter()),
                            TypeDeclarationKind::Variant { cases } => {
                                Box::new(cases.iter().flat_map(|case| &case.fields))
                            }
                            TypeDeclarationKind::Resource { .. } => unreachable!(),
                        };
                    for field in fields.rev() {
                        let Some(field_ty) =
                            Self::substitute_variant_type(declaration, arguments, &field.ty)
                        else {
                            return true;
                        };
                        frames.push(Frame::Enter(field_ty));
                    }
                }
            }
        }
        false
    }

    /// Whether a value carries unique destruction authority, either directly
    /// or through an aggregate field. This is deliberately broader than
    /// `contains_resource`: compiler-owned `Bytes` is not an opaque resource,
    /// but records and variants containing it are still non-Copy owners.
    pub(super) fn needs_drop(&self, ty: &Type) -> bool {
        enum Frame {
            Enter(Type),
            Exit(String),
        }

        let mut visiting = HashSet::new();
        let mut frames = vec![Frame::Enter(ty.clone())];
        while let Some(frame) = frames.pop() {
            match frame {
                Frame::Exit(instance) => {
                    visiting.remove(&instance);
                }
                Frame::Enter(ty) => match ty {
                    Type::String | Type::Bytes => return true,
                    Type::Named { name, arguments } => {
                        let Some(declaration) = self.declaration(&name) else {
                            continue;
                        };
                        if matches!(declaration.kind, TypeDeclarationKind::Resource { .. }) {
                            return true;
                        }
                        let instance = Type::Named {
                            name: name.clone(),
                            arguments: arguments.clone(),
                        }
                        .to_string();
                        if !visiting.insert(instance.clone()) {
                            return true;
                        }
                        frames.push(Frame::Exit(instance));
                        let fields: Box<dyn DoubleEndedIterator<Item = &FieldDeclaration>> =
                            match &declaration.kind {
                                TypeDeclarationKind::Record { fields }
                                | TypeDeclarationKind::Class { fields, .. } => {
                                    Box::new(fields.iter())
                                }
                                TypeDeclarationKind::Variant { cases } => {
                                    Box::new(cases.iter().flat_map(|case| &case.fields))
                                }
                                TypeDeclarationKind::Resource { .. } => unreachable!(),
                            };
                        for field in fields.rev() {
                            let Some(field_ty) =
                                Self::substitute_variant_type(declaration, &arguments, &field.ty)
                            else {
                                return true;
                            };
                            frames.push(Frame::Enter(field_ty));
                        }
                    }
                    Type::I64
                    | Type::I32
                    | Type::Char
                    | Type::U8
                    | Type::Usize
                    | Type::ArrayU8(_)
                    | Type::F32
                    | Type::F64
                    | Type::Bool
                    | Type::Str
                    | Type::SliceU8 => {}
                },
            }
        }
        false
    }

    /// Detect compiler-owned byte authority below a type boundary. The first
    /// owned-byte aggregate profile admits `Bytes` only as a direct field;
    /// nested, generic-authored, and resource-bearing carriers remain closed.
    pub(super) fn contains_owned_bytes(&self, ty: &Type) -> bool {
        let mut pending = vec![ty.clone()];
        let mut visited = HashSet::new();
        while let Some(current) = pending.pop() {
            match current {
                Type::Bytes => return true,
                Type::Named { name, arguments } => {
                    let identity = Type::Named {
                        name: name.clone(),
                        arguments: arguments.clone(),
                    }
                    .to_string();
                    if !visited.insert(identity) {
                        continue;
                    }
                    let Some(declaration) = self.declaration(&name) else {
                        continue;
                    };
                    let fields: Box<dyn Iterator<Item = &FieldDeclaration>> =
                        match &declaration.kind {
                            TypeDeclarationKind::Record { fields }
                            | TypeDeclarationKind::Class { fields, .. } => Box::new(fields.iter()),
                            TypeDeclarationKind::Variant { cases } => {
                                Box::new(cases.iter().flat_map(|case| &case.fields))
                            }
                            TypeDeclarationKind::Resource { .. } => continue,
                        };
                    for field in fields {
                        let Some(field_ty) =
                            Self::substitute_variant_type(declaration, &arguments, &field.ty)
                        else {
                            return true;
                        };
                        pending.push(field_ty);
                    }
                }
                Type::I64
                | Type::I32
                | Type::Char
                | Type::U8
                | Type::Usize
                | Type::ArrayU8(_)
                | Type::F32
                | Type::F64
                | Type::Bool
                | Type::String
                | Type::Str
                | Type::SliceU8 => {}
            }
        }
        false
    }

    pub(super) fn is_nested_owned_byte_record(&self, ty: &Type) -> bool {
        classify_nested_owned_byte_record(self, ty) == NestedOwnedRecordAdmission::Admitted
    }

    pub(super) fn is_flat_owned_byte_record(&self, ty: &Type) -> bool {
        let Type::Named { name, arguments } = ty else {
            return false;
        };
        let Some(declaration) = self.declaration(name) else {
            return false;
        };
        let TypeDeclarationKind::Record { fields } = &declaration.kind else {
            return false;
        };
        if arguments.len() != declaration.type_parameters.len() {
            return false;
        }
        if !arguments.is_empty()
            && arguments.iter().any(|argument| {
                *argument != Type::Bytes && !owned_byte_record_copy_field_is_admitted(argument)
            })
        {
            return false;
        }
        let Some(fields) = fields
            .iter()
            .map(|field| Self::substitute_variant_type(declaration, arguments, &field.ty))
            .collect::<Option<Vec<_>>>()
        else {
            return false;
        };
        fields.contains(&Type::Bytes)
            && fields.iter().all(|field| {
                *field == Type::Bytes || owned_byte_record_copy_field_is_admitted(field)
            })
    }

    pub(super) fn is_flat_owned_byte_record_template(
        &self,
        ty: &Type,
        function_parameters: &HashSet<&str>,
    ) -> bool {
        let Type::Named { name, arguments } = ty else {
            return false;
        };
        let Some(declaration) = self.declaration(name) else {
            return false;
        };
        let TypeDeclarationKind::Record { fields } = &declaration.kind else {
            return false;
        };
        if arguments.len() != declaration.type_parameters.len()
            || arguments.iter().any(|argument| {
                *argument != Type::Bytes
                    && !owned_byte_record_copy_field_is_admitted(argument)
                    && !matches!(argument, Type::Named { name, arguments }
                        if arguments.is_empty() && function_parameters.contains(name.as_str()))
            })
        {
            return false;
        }
        fields
            .iter()
            .map(|field| Self::substitute_variant_type(declaration, arguments, &field.ty))
            .collect::<Option<Vec<_>>>()
            .is_some_and(|fields| {
                fields.contains(&Type::Bytes)
                    && fields.iter().all(|field| {
                        *field == Type::Bytes
                            || owned_byte_record_copy_field_is_admitted(field)
                            || matches!(field, Type::Named { name, arguments }
                                if arguments.is_empty()
                                    && function_parameters.contains(name.as_str()))
                    })
            })
    }

    /// Exact non-Copy variant profile admitted by Owned Byte Variant Algebra
    /// v1. Authored variants are monomorphic and flat. The only generic
    /// carriers are the compiler-owned prelude identities, whose source names
    /// are reserved and authenticated again in resolved HIR.
    pub(super) fn is_flat_owned_byte_variant(&self, ty: &Type) -> bool {
        let Type::Named { name, arguments } = ty else {
            return false;
        };
        if owned_byte_prelude_instance_is_admitted(name, arguments) {
            return true;
        }
        if !arguments.is_empty() {
            return false;
        }
        matches!(
            self.declaration(name).map(|declaration| &declaration.kind),
            Some(TypeDeclarationKind::Variant { cases })
                if cases.iter().flat_map(|case| &case.fields).any(|field| field.ty == Type::Bytes)
                    && cases.iter().flat_map(|case| &case.fields).all(|field|
                        field.ty == Type::Bytes
                            || owned_byte_record_copy_field_is_admitted(&field.ty))
        )
    }

    pub(super) fn is_opaque_resource(&self, ty: &Type) -> bool {
        let Type::Named { name, .. } = ty else {
            return false;
        };
        self.declaration(name).is_some_and(|declaration| {
            matches!(declaration.kind, TypeDeclarationKind::Resource { .. })
        })
    }

    pub(super) fn lifecycle_effects(
        &self,
        ty: &Type,
        imports: &HashMap<&str, (&InterfaceDeclaration, &ImportDeclaration)>,
    ) -> HashSet<String> {
        let mut effects = HashSet::new();
        self.lifecycle_effects_inner(ty, imports, &mut HashSet::new(), &mut effects);
        effects
    }

    pub(super) fn lifecycle_effects_inner(
        &self,
        ty: &Type,
        imports: &HashMap<&str, (&InterfaceDeclaration, &ImportDeclaration)>,
        visiting: &mut HashSet<String>,
        effects: &mut HashSet<String>,
    ) {
        enum Frame {
            Enter(Type),
            Exit(String),
        }
        let mut frames = vec![Frame::Enter(ty.clone())];
        while let Some(frame) = frames.pop() {
            match frame {
                Frame::Exit(instance) => {
                    visiting.remove(&instance);
                }
                Frame::Enter(ty) => {
                    let Type::Named { name, arguments } = &ty else {
                        continue;
                    };
                    let Some(declaration) = self.declaration(name) else {
                        continue;
                    };
                    let instance = ty.to_string();
                    if !visiting.insert(instance.clone()) {
                        continue;
                    }
                    frames.push(Frame::Exit(instance));
                    match &declaration.kind {
                        TypeDeclarationKind::Resource { lifecycles } => {
                            if let Some(crate::ast::ResourceLifecycleDeclaration {
                                kind: ResourceLifecycleKind::Imported { import_key },
                                ..
                            }) = lifecycles.first()
                            {
                                if let Some((_, import)) = imports.get(import_key.as_str()) {
                                    effects.extend(import.effects.iter().cloned());
                                }
                            }
                        }
                        TypeDeclarationKind::Record { fields }
                        | TypeDeclarationKind::Class { fields, .. } => {
                            for field in fields.iter().rev() {
                                if let Some(field_ty) =
                                    Self::substitute_variant_type(declaration, arguments, &field.ty)
                                {
                                    frames.push(Frame::Enter(field_ty));
                                }
                            }
                        }
                        TypeDeclarationKind::Variant { cases } => {
                            for field in cases.iter().flat_map(|case| &case.fields).rev() {
                                if let Some(field_ty) =
                                    Self::substitute_variant_type(declaration, arguments, &field.ty)
                                {
                                    frames.push(Frame::Enter(field_ty));
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}

pub(super) const MAX_NESTED_OWNED_RECORD_DEPTH: usize = 64;
pub(super) const MAX_NESTED_OWNED_BYTE_LEAVES: usize = 256;
pub(super) const MAX_NESTED_OWNED_RECORD_FIELDS: usize = 4_096;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum NestedOwnedRecordAdmission {
    Admitted,
    NoOwnedBytes,
    OutsideProfile,
    Recursive,
    LimitExceeded,
}

/// Classify one concrete source type without consulting resolved HIR or its
/// type-fact cache. Every record occurrence is charged independently: a type
/// used by two fields represents two storage subtrees, not one memoized node.
pub(super) fn classify_nested_owned_byte_record(
    types: &TypeTable<'_>,
    root: &Type,
) -> NestedOwnedRecordAdmission {
    if matches!(root, Type::Named { arguments, .. } if !arguments.is_empty())
        && !types.is_flat_owned_byte_record(root)
    {
        return NestedOwnedRecordAdmission::OutsideProfile;
    }
    enum Frame<'a> {
        Type(Type, usize),
        Fields(
            &'a TypeDeclaration,
            &'a [FieldDeclaration],
            Vec<Type>,
            usize,
            usize,
        ),
        LeaveRecord(String),
    }

    let mut frames = vec![Frame::Type(root.clone(), 1)];
    let mut active = HashSet::new();
    let mut owned_leaves = 0usize;
    let mut visited_fields = 0usize;

    while let Some(frame) = frames.pop() {
        match frame {
            Frame::Type(Type::Bytes, _) => {
                owned_leaves += 1;
                if owned_leaves > MAX_NESTED_OWNED_BYTE_LEAVES {
                    return NestedOwnedRecordAdmission::LimitExceeded;
                }
            }
            Frame::Type(ref ty, _) if owned_byte_record_copy_field_is_admitted(ty) => {}
            Frame::Type(Type::Named { name, arguments }, depth) => {
                if depth > MAX_NESTED_OWNED_RECORD_DEPTH {
                    return NestedOwnedRecordAdmission::LimitExceeded;
                }
                if depth > 1 && !arguments.is_empty() {
                    return NestedOwnedRecordAdmission::OutsideProfile;
                }
                let Some(declaration) = types.declaration(&name) else {
                    return NestedOwnedRecordAdmission::OutsideProfile;
                };
                if arguments.len() != declaration.type_parameters.len()
                    || arguments.iter().any(|argument| {
                        *argument != Type::Bytes
                            && !owned_byte_record_copy_field_is_admitted(argument)
                    })
                {
                    return NestedOwnedRecordAdmission::OutsideProfile;
                }
                let TypeDeclarationKind::Record { fields } = &declaration.kind else {
                    return NestedOwnedRecordAdmission::OutsideProfile;
                };
                let identity = Type::Named {
                    name: name.clone(),
                    arguments: arguments.clone(),
                }
                .to_string();
                if !active.insert(identity.clone()) {
                    return NestedOwnedRecordAdmission::Recursive;
                }
                frames.push(Frame::LeaveRecord(identity));
                frames.push(Frame::Fields(declaration, fields, arguments, 0, depth));
            }
            Frame::Type(
                Type::I64
                | Type::I32
                | Type::Char
                | Type::U8
                | Type::Usize
                | Type::F32
                | Type::F64
                | Type::Bool,
                _,
            ) => unreachable!("admitted scalar handled above"),
            Frame::Type(Type::ArrayU8(_) | Type::String | Type::Str | Type::SliceU8, _) => {
                return NestedOwnedRecordAdmission::OutsideProfile
            }
            Frame::Fields(declaration, fields, arguments, index, depth) => {
                let Some(field) = fields.get(index) else {
                    continue;
                };
                visited_fields += 1;
                if visited_fields > MAX_NESTED_OWNED_RECORD_FIELDS {
                    return NestedOwnedRecordAdmission::LimitExceeded;
                }
                frames.push(Frame::Fields(
                    declaration,
                    fields,
                    arguments.clone(),
                    index + 1,
                    depth,
                ));
                let Some(field_ty) =
                    TypeTable::substitute_variant_type(declaration, &arguments, &field.ty)
                else {
                    return NestedOwnedRecordAdmission::OutsideProfile;
                };
                frames.push(Frame::Type(field_ty, depth + 1));
            }
            Frame::LeaveRecord(identity) => {
                active.remove(&identity);
            }
        }
    }

    if owned_leaves == 0 {
        NestedOwnedRecordAdmission::NoOwnedBytes
    } else {
        NestedOwnedRecordAdmission::Admitted
    }
}

pub(super) fn owned_byte_record_copy_field_is_admitted(ty: &Type) -> bool {
    matches!(
        ty,
        Type::I64
            | Type::I32
            | Type::U8
            | Type::Usize
            | Type::F32
            | Type::F64
            | Type::Char
            | Type::Bool
    )
}

pub(super) fn owned_byte_prelude_instance_is_admitted(name: &str, arguments: &[Type]) -> bool {
    matches!(
        (name, arguments),
        ("Option", [Type::Bytes])
            | ("Result", [Type::Bytes, Type::I64 | Type::Bool])
            | ("Result", [Type::I64 | Type::Bool, Type::Bytes])
    )
}

#[cfg(test)]
mod tests;
