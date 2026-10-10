//! Reconstruct the entire finite descriptor from authenticated ordinary source.
use super::*;

pub(super) struct Shape<'a> {
    pub items: &'a FieldDeclaration,
    pub row: &'a TypeDeclaration,
    pub metrics_field: &'a FieldDeclaration,
    pub metrics: &'a TypeDeclaration,
}

fn validate_row(record: &TypeDeclaration) -> Result<(), Vec<Diagnostic>> {
    let TypeDeclarationKind::Record { fields } = &record.kind else {
        return Err(refusal("collection response Row must be a record"));
    };
    let strings = fields
        .iter()
        .filter(|field| field.ty == Type::String)
        .count();
    let scalars = fields.len().saturating_sub(strings);
    if !(1..=2).contains(&strings) || scalars > 6 {
        return Err(refusal(
            "collection response Row requires one or two String fields and at most six scalar fields",
        ));
    }
    let mut scalar = record.clone();
    let TypeDeclarationKind::Record { fields } = &mut scalar.kind else {
        unreachable!()
    };
    for field in fields {
        if field.ty == Type::String {
            field.ty = Type::Usize;
        }
    }
    super::super::validate_record(&scalar)
}

pub(super) fn validate<'a>(
    program: &'a Program,
    root: &'a TypeDeclaration,
    bound: usize,
) -> Result<Shape<'a>, Vec<Diagnostic>> {
    if !(1..=64).contains(&bound) {
        return Err(refusal(
            "collection response requires max_string_bytes in 1..=64",
        ));
    }
    let TypeDeclarationKind::Record { fields: fs } = &root.kind else {
        return Err(refusal("collection response root must be a record"));
    };
    let mut scalar = root.clone();
    let TypeDeclarationKind::Record {
        fields: scalar_fields,
    } = &mut scalar.kind
    else {
        unreachable!()
    };
    for field in scalar_fields {
        field.ty = Type::Usize;
    }
    super::super::validate_record(&scalar)?;
    if fs.len() != 2 {
        return Err(refusal(
            "collection response requires exactly one Row vector and one scalar metrics record",
        ));
    }
    let mut items = None;
    let mut metrics = None;
    for field in fs {
        let Type::Named { name, arguments } = &field.ty else {
            return Err(refusal(
                "collection response fields must be a direct Vec<Row> and a monomorphic record",
            ));
        };
        let (type_name, vector) = if name == "Vec" && arguments.len() == 1 {
            let Type::Named { name, arguments } = &arguments[0] else {
                return Err(refusal(
                    "collection response vector must contain one authored Row record",
                ));
            };
            if !arguments.is_empty() {
                return Err(refusal("collection response Row must be monomorphic"));
            }
            (name, true)
        } else if arguments.is_empty() {
            (name, false)
        } else {
            return Err(refusal(
                "collection response contains an unsupported type application",
            ));
        };
        let mut matches = program.types.iter().filter(|ty| ty.name == *type_name);
        let record = matches.next().ok_or_else(|| {
            refusal(
                "collection response children must be identified records in the same schema source",
            )
        })?;
        if matches.next().is_some() {
            return Err(refusal("collection response child type is ambiguous"));
        }
        if vector && items.is_none() {
            validate_row(record)?;
            items = Some((field, record));
        } else if !vector && metrics.is_none() {
            super::super::validate_record(record)?;
            metrics = Some((field, record));
        } else {
            return Err(refusal(
                "collection response must have exactly one vector and one metrics record",
            ));
        }
    }
    let (items, row) = items.ok_or_else(|| refusal("collection response lacks its Row vector"))?;
    let (metrics_field, metrics) =
        metrics.ok_or_else(|| refusal("collection response lacks its scalar metrics record"))?;
    Ok(Shape {
        items,
        row,
        metrics_field,
        metrics,
    })
}
