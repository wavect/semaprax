//! Versioned finite key descriptors. A reserved identity is not authority:
//! replay binds the complete declaration layout before the call may execute.
use crate::hir::{DeclarationId, DeclarationIndex, ResolvedType};
use std::fmt::Write as _;
pub(crate) const NAME: &str = "vec_sort_by";
pub(crate) const PREFIX: &str = "core.vec.sort-by.v1:";
pub(crate) const MAX_SELECTOR_BYTES: usize = 1024;
pub(crate) const MAX_DESCRIPTOR_BYTES: usize = 16_384;
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct Key {
    pub(crate) field: DeclarationId,
    pub(crate) descending: bool,
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct Descriptor {
    pub(crate) element: String,
    pub(crate) layout: String,
    pub(crate) keys: Vec<Key>,
}
fn decimal_digits(value: usize) -> usize {
    value.checked_ilog10().unwrap_or(0) as usize + 1
}

fn canonical_decimal(value: &str) -> Option<usize> {
    let parsed = value.parse::<usize>().ok()?;
    // usize parsing accepts a leading '+' and redundant zeroes. Either
    // spelling is longer than the minimal decimal representation.
    (value.len() == decimal_digits(parsed)).then_some(parsed)
}

pub(super) fn part_len(value: &str) -> Option<usize> {
    let length = value.len();
    length.checked_add(decimal_digits(length))?.checked_add(1)
}
pub(super) fn part(out: &mut String, value: &str) {
    write!(out, "{}:{value}", value.len()).unwrap();
}
pub(crate) fn selector(value: &str) -> Option<Vec<(&str, bool)>> {
    if value.is_empty() || value.len() > MAX_SELECTOR_BYTES {
        return None;
    }
    let mut keys = Vec::new();
    for item in value.split(',') {
        let (name, direction) = item.split_once(':')?;
        if name.is_empty()
            || !name.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'_')
            || name.as_bytes()[0].is_ascii_digit()
            || keys.iter().any(|(old, _)| *old == name)
        {
            return None;
        }
        let descending = match direction {
            "asc" => false,
            "desc" => true,
            _ => return None,
        };
        if keys.len() == 8 {
            return None;
        }
        keys.push((name, descending));
    }
    Some(keys)
}
impl Descriptor {
    pub(super) fn reserve_identity(&self) -> Option<usize> {
        if !(1..=8).contains(&self.keys.len()) {
            return None;
        }
        let mut length = PREFIX
            .len()
            .checked_add(part_len(&self.element)?)?
            .checked_add(part_len(&self.layout)?)?
            .checked_add(2)?;
        for key in &self.keys {
            length = length
                .checked_add(part_len(key.field.as_str())?)?
                .checked_add(1)?;
        }
        if length > MAX_DESCRIPTOR_BYTES {
            return None;
        }
        if !crate::bounded_output::reserve_active_required(length) {
            return None;
        }
        Some(length)
    }

    pub(crate) fn identity(&self) -> Option<String> {
        let length = self.reserve_identity()?;
        let mut out = String::with_capacity(length);
        out.push_str(PREFIX);
        part(&mut out, &self.element);
        part(&mut out, &self.layout);
        write!(out, "{}:", self.keys.len()).unwrap();
        for key in &self.keys {
            part(&mut out, key.field.as_str());
            out.push(if key.descending { 'd' } else { 'a' });
        }
        (out.len() <= MAX_DESCRIPTOR_BYTES).then_some(out)
    }
}
pub(super) fn take<'a>(input: &mut &'a str) -> Option<&'a str> {
    let (size, tail) = input.split_once(':')?;
    let size_value = canonical_decimal(size)?;
    let value = tail.get(..size_value)?;
    *input = tail.get(size_value..)?;
    Some(value)
}
pub(crate) fn decode(identity: &str) -> Option<Descriptor> {
    if identity.len() > MAX_DESCRIPTOR_BYTES {
        return None;
    }
    let mut input = identity.strip_prefix(PREFIX)?;
    let element = take(&mut input)?.to_owned();
    let layout = take(&mut input)?.to_owned();
    let (count, tail) = input.split_once(':')?;
    let n = canonical_decimal(count)?;
    if !(1..=8).contains(&n) {
        return None;
    }
    input = tail;
    let mut keys = Vec::with_capacity(n);
    for _ in 0..n {
        let field = DeclarationId::new(take(&mut input)?);
        let d = *input.as_bytes().first()?;
        input = input.get(1..)?;
        let descending = match d {
            b'a' => false,
            b'd' => true,
            _ => return None,
        };
        if keys.iter().any(|key: &Key| key.field == field) {
            return None;
        }
        keys.push(Key { field, descending });
    }
    if !input.is_empty() {
        return None;
    }
    let result = Descriptor {
        element,
        layout,
        keys,
    };
    // Every part was read with a canonical byte length. Keep the original
    // re-encoding reservation and sticky refusal without copying the wire.
    crate::bounded_output::reserve_active_required(identity.len()).then_some(result)
}
fn field_identity_key(ty: &ResolvedType) -> Option<&'static str> {
    Some(match ty {
        ResolvedType::I64 => "i64",
        ResolvedType::I32 => "i32",
        ResolvedType::U8 => "u8",
        ResolvedType::Usize => "usize",
        ResolvedType::Char => "char",
        ResolvedType::F32 => "f32",
        ResolvedType::F64 => "f64",
        ResolvedType::Bool => "bool",
        ResolvedType::String => "string",
        ResolvedType::Bytes => "bytes",
        _ => return None,
    })
}

fn layout_key<'a>(
    element: &str,
    fields: impl Iterator<Item = Option<(&'a str, &'static str)>> + Clone,
    family: &str,
    capacity: u64,
) -> Option<String> {
    let mut length = part_len(family)?
        .checked_add(part_len(element)?)?
        .checked_add(21)?;
    for field in fields.clone() {
        let (id, ty) = field?;
        length = length
            .checked_add(part_len(id)?)?
            .checked_add(part_len(ty)?)?;
    }
    if length > MAX_DESCRIPTOR_BYTES || !crate::bounded_output::reserve_active_required(length) {
        return None;
    }
    let mut out = String::with_capacity(length);
    part(&mut out, family);
    part(&mut out, element);
    write!(out, "{capacity}:").unwrap();
    for field in fields {
        let (id, ty) = field?;
        part(&mut out, id);
        part(&mut out, ty);
    }
    Some(out)
}
pub(crate) fn resolve(
    index: &DeclarationIndex,
    element: &ResolvedType,
    value: &str,
) -> Option<Descriptor> {
    let ResolvedType::Nominal {
        declaration,
        arguments,
    } = element
    else {
        return None;
    };
    if !arguments.is_empty() {
        return None;
    }
    let copy = crate::hir::copy_record_collection::admitted(index, element);
    let owned = crate::hir::owned_leaf_collection::layout(index, element);
    if !copy && owned.is_none() {
        return None;
    }
    let fields = index.record_fields(declaration)?;
    if declaration.as_str().len() > MAX_DESCRIPTOR_BYTES
        || fields
            .iter()
            .any(|f| f.id.as_str().len() > MAX_DESCRIPTOR_BYTES)
    {
        return None;
    }
    if fields.iter().any(|field| {
        index
            .declaration(&field.id)
            .is_none_or(|d| d.identity_origin != crate::hir::IdentityOrigin::Explicit)
    }) {
        return None;
    }
    let capacity = if copy {
        crate::vec_ops::MAX_CAPACITY / fields.len() as u64
    } else {
        owned?.capacity()
    };
    let element = element.identity_key();
    let layout = layout_key(
        &element,
        fields
            .iter()
            .map(|f| Some((f.id.as_str(), field_identity_key(&f.ty)?))),
        if copy { "copy-record" } else { "owned-record" },
        capacity,
    )?;
    let mut keys = Vec::new();
    for (name, descending) in selector(value)? {
        let field = fields
            .iter()
            .find(|f| index.declaration(&f.id).is_some_and(|d| d.name == name))?;
        keys.push(Key {
            field: field.id.clone(),
            descending,
        });
    }
    let result = Descriptor {
        element,
        layout,
        keys,
    };
    result.reserve_identity()?;
    Some(result)
}
pub(crate) fn replay(
    index: &DeclarationIndex,
    element: &ResolvedType,
    identity: &str,
) -> Option<Descriptor> {
    let descriptor = decode(identity)?;
    let ResolvedType::Nominal { declaration, .. } = element else {
        return None;
    };
    let fields = index.record_fields(declaration)?;
    let mut literal = String::new();
    for key in &descriptor.keys {
        let field = fields.iter().find(|f| f.id == key.field)?;
        if !literal.is_empty() {
            literal.push(',');
        }
        let name = &index.declaration(&field.id)?.name;
        write!(
            literal,
            "{name}:{}",
            if key.descending { "desc" } else { "asc" }
        )
        .unwrap();
    }
    let expected = resolve(index, element, &literal)?;
    (expected == descriptor).then_some(descriptor)
}
pub(crate) fn positions(
    index: &DeclarationIndex,
    element: &ResolvedType,
    identity: &str,
) -> Option<Vec<(usize, bool)>> {
    let descriptor = replay(index, element, identity)?;
    let ResolvedType::Nominal { declaration, .. } = element else {
        return None;
    };
    let fields = index.record_fields(declaration)?;
    descriptor
        .keys
        .iter()
        .map(|key| {
            Some((
                fields.iter().position(|f| f.id == key.field)?,
                key.descending,
            ))
        })
        .collect()
}

/// Independent source layout derivation; no resolved or generated facts enter.
pub(crate) fn source(declaration: &crate::ast::TypeDeclaration, value: &str) -> Option<Descriptor> {
    use crate::ast::{Type, TypeDeclarationKind};
    let TypeDeclarationKind::Record { fields } = &declaration.kind else {
        return None;
    };
    if !declaration.explicit_id
        || !declaration.type_parameters.is_empty()
        || !(1..=8).contains(&fields.len())
        || fields.iter().any(|f| !f.explicit_id)
    {
        return None;
    }
    let copy = crate::source_verify::copy_record_declaration_admitted(declaration);
    if !copy && !crate::source_verify::owned_leaf_declaration_admitted(declaration) {
        return None;
    }
    if declaration.stable_id.len() > MAX_DESCRIPTOR_BYTES
        || fields
            .iter()
            .any(|f| f.stable_id.len() > MAX_DESCRIPTOR_BYTES)
    {
        return None;
    }
    let field_keys = fields.iter().map(|field| {
        let ty = match field.ty {
            Type::I64 => ResolvedType::I64,
            Type::I32 => ResolvedType::I32,
            Type::U8 => ResolvedType::U8,
            Type::Usize => ResolvedType::Usize,
            Type::Bool => ResolvedType::Bool,
            Type::Char => ResolvedType::Char,
            Type::F32 => ResolvedType::F32,
            Type::F64 => ResolvedType::F64,
            Type::String => ResolvedType::String,
            Type::Bytes => ResolvedType::Bytes,
            _ => return None,
        };
        Some((field.stable_id.as_str(), field_identity_key(&ty)?))
    });
    let capacity = if copy {
        crate::vec_ops::MAX_CAPACITY / fields.len() as u64
    } else {
        let owned = fields
            .iter()
            .filter(|field| matches!(field.ty, Type::String | Type::Bytes))
            .count() as u64;
        let scalar = fields.len() as u64 - owned;
        let scalar = crate::vec_ops::MAX_CAPACITY
            .checked_div(scalar)
            .unwrap_or(crate::vec_ops::MAX_CAPACITY);
        scalar
            .min(
                crate::vec_ops::MAX_OWNED_PAYLOAD_BYTES
                    / (owned * crate::vec_ops::OWNED_PAYLOAD_BYTES_PER_ELEMENT),
            )
            .min(crate::vec_ops::MAX_CAPACITY)
    };
    let element = ResolvedType::Nominal {
        declaration: DeclarationId::new(&declaration.stable_id),
        arguments: vec![],
    }
    .identity_key();
    let layout = layout_key(
        &element,
        field_keys,
        if copy { "copy-record" } else { "owned-record" },
        capacity,
    )?;
    let mut keys = Vec::new();
    for (name, descending) in selector(value)? {
        let field = fields.iter().find(|f| f.name == name)?;
        keys.push(Key {
            field: DeclarationId::new(&field.stable_id),
            descending,
        });
    }
    let descriptor = Descriptor {
        element,
        layout,
        keys,
    };
    descriptor.reserve_identity()?;
    Some(descriptor)
}

pub(crate) fn expression_positions(
    index: &DeclarationIndex,
    expression: &crate::hir::ResolvedExpr,
) -> Option<Vec<(usize, bool)>> {
    let crate::hir::ResolvedExprKind::Call {
        callee,
        type_arguments,
        instance: None,
        args,
    } = &expression.kind
    else {
        return None;
    };
    let [element] = type_arguments.as_slice() else {
        return None;
    };
    if args.len() != 1
        || args[0].ty != crate::vec_ops::resolved_vec(element.clone())
        || args[0].ownership != crate::hir::OwnershipMode::Own
        || expression.ty != args[0].ty
        || expression.ownership != crate::hir::OwnershipMode::Own
    {
        return None;
    }
    positions(index, element, callee.as_str())
}

pub(crate) fn uses(program: &crate::hir::ResolvedProgram) -> bool {
    program
        .functions
        .iter()
        .chain(
            program
                .function_instances
                .iter()
                .map(|instance| &instance.function),
        )
        .any(|function| {
            function
                .requires
                .iter()
                .chain(std::iter::once(&function.body))
                .chain(function.ensures.iter())
                .any(|root| {
                    let mut found = false;
                    crate::hir::visit_resolved_calls(root, &mut |callee, _, _| {
                        found |= callee.as_str().starts_with(PREFIX)
                    });
                    found
                })
        })
}

#[cfg(test)]
mod tests;
