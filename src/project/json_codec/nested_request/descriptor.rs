//! Field paths, policy bounds, and concrete Vec shapes come only from source.
use super::*;
use std::collections::BTreeSet;

pub(super) struct Shape<'a> {
    pub root: Record<'a>,
}
pub(super) struct Record<'a> {
    pub declaration: &'a TypeDeclaration,
    pub ordinal: usize,
    pub fields: Vec<Field<'a>>,
}
pub(super) struct Field<'a> {
    pub declaration: &'a FieldDeclaration,
    pub ordinal: usize,
    pub kind: Kind<'a>,
}
pub(super) enum Kind<'a> {
    Scalar(Type),
    Text,
    Record(Box<Record<'a>>),
    Vector { element: Type, kind: Box<Kind<'a>> },
}
struct Census {
    ordinal: usize,
    arrays: usize,
    strings: usize,
    owners: usize,
}

pub(super) fn validate<'a>(
    program: &'a Program,
    root: &'a TypeDeclaration,
    string_bound: usize,
    array_bound: usize,
) -> Result<Shape<'a>, Vec<Diagnostic>> {
    if !(1..=64).contains(&string_bound) || !(1..=256).contains(&array_bound) {
        return Err(refusal(
            "nested request bounds require max_string_bytes in 1..=64 and max_array_items in 1..=256",
        ));
    }
    let mut census = Census {
        ordinal: 0,
        arrays: 0,
        strings: 0,
        owners: 0,
    };
    let root = record(
        program,
        root,
        0,
        1,
        1,
        array_bound,
        &mut BTreeSet::new(),
        &mut census,
    )?;
    if census.owners == 0 {
        return Err(refusal(
            "nested request requires an owning String or existing Vec carrier in its root closure",
        ));
    }
    Ok(Shape { root })
}
fn find<'a>(program: &'a Program, name: &str) -> Result<&'a TypeDeclaration, Vec<Diagnostic>> {
    let mut found = program.types.iter().filter(|item| item.name == name);
    let record = found.next().ok_or_else(|| {
        refusal("nested request records must be declared in the same schema source")
    })?;
    if found.next().is_some() {
        return Err(refusal("nested request type reference is ambiguous"));
    }
    Ok(record)
}
#[allow(clippy::too_many_arguments)]
fn record<'a>(
    program: &'a Program,
    declaration: &'a TypeDeclaration,
    ordinal: usize,
    depth: usize,
    multiplier: usize,
    array_bound: usize,
    active: &mut BTreeSet<String>,
    census: &mut Census,
) -> Result<Record<'a>, Vec<Diagnostic>> {
    if depth > 8 || !active.insert(declaration.stable_id.clone()) {
        return Err(refusal(
            "nested request record closure is cyclic or exceeds depth 8",
        ));
    }
    let mut scalar = declaration.clone();
    let TypeDeclarationKind::Record {
        fields: scalar_fields,
    } = &mut scalar.kind
    else {
        return Err(refusal(
            "nested request children must be monomorphic invariant-free records",
        ));
    };
    for field in scalar_fields {
        field.ty = Type::Usize;
    }
    super::super::validate_record(&scalar)?;
    let mut output = Vec::new();
    for field in fields(declaration) {
        census.ordinal += 1;
        let field_ordinal = census.ordinal;
        if census.ordinal > 64 {
            return Err(refusal("nested request exceeds 64 expanded field paths"));
        }
        let kind = kind(
            program,
            &field.ty,
            field_ordinal,
            depth,
            multiplier,
            array_bound,
            active,
            census,
        )?;
        output.push(Field {
            declaration: field,
            ordinal: field_ordinal,
            kind,
        });
    }
    active.remove(&declaration.stable_id);
    Ok(Record {
        declaration,
        ordinal,
        fields: output,
    })
}
#[allow(clippy::too_many_arguments)]
fn kind<'a>(
    program: &'a Program,
    ty: &Type,
    ordinal: usize,
    depth: usize,
    multiplier: usize,
    array_bound: usize,
    active: &mut BTreeSet<String>,
    census: &mut Census,
) -> Result<Kind<'a>, Vec<Diagnostic>> {
    match ty {
        Type::I64 | Type::U8 | Type::Usize | Type::Bool => Ok(Kind::Scalar(ty.clone())),
        Type::String => {
            census.strings += multiplier;
            census.owners += multiplier;
            if census.strings > 264 {
                return Err(refusal(
                    "nested request exceeds 264 simultaneous decoded String owners",
                ));
            }
            Ok(Kind::Text)
        }
        Type::Named { name, arguments } if name == "Vec" && arguments.len() == 1 => {
            census.arrays += 1;
            census.owners += 1;
            if census.arrays > 1 {
                return Err(refusal(
                    "nested request admits at most one expanded Vec field",
                ));
            }
            let element = &arguments[0];
            match element {
                Type::I64 | Type::U8 | Type::Usize | Type::Bool | Type::String => {}
                Type::Named { name, arguments } if arguments.is_empty() => {
                    let row = find(program, name)?;
                    let TypeDeclarationKind::Record { fields } = &row.kind else {
                        return Err(refusal(
                            "nested request Vec element must be a flat admitted record",
                        ));
                    };
                    let strings = fields
                        .iter()
                        .filter(|field| field.ty == Type::String)
                        .count();
                    if fields.is_empty()
                        || fields.len() > 8
                        || strings > 2
                        || fields.iter().any(|field| {
                            !matches!(
                                field.ty,
                                Type::I64 | Type::U8 | Type::Usize | Type::Bool | Type::String
                            )
                        })
                    {
                        return Err(refusal(
                            "nested request Vec rows require 1..8 scalar fields with at most two String leaves",
                        ));
                    }
                }
                _ => {
                    return Err(refusal(
                        "nested request does not broaden existing Vec element shapes",
                    ));
                }
            }
            let element_kind = kind(
                program,
                element,
                ordinal,
                depth,
                multiplier * array_bound,
                array_bound,
                active,
                census,
            )?;
            Ok(Kind::Vector {
                element: element.clone(),
                kind: Box::new(element_kind),
            })
        }
        Type::Named { name, arguments } if arguments.is_empty() => {
            Ok(Kind::Record(Box::new(record(
                program,
                find(program, name)?,
                ordinal,
                depth + 1,
                multiplier,
                array_bound,
                active,
                census,
            )?)))
        }
        _ => Err(refusal(
            "nested request supports only explicit acyclic records, bounded Unicode Strings, i64/u8/usize/bool, and one admitted Vec",
        )),
    }
}
