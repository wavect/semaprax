//! Append-only record evolution over authenticated canonical Project ASTs.
//! Defaults are inert literals; existing field evaluation and bindings stay put.
use std::collections::{BTreeMap, BTreeSet};

use serde_json::Value;

use crate::ast::{
    Expr, ExprKind, FieldDeclaration, FieldInitializer, MatchPattern, ModuleUseKind, Program,
    RecordMatchFieldPattern, RecordMatchPatternField, Span, Type, TypeDeclarationKind,
};
use crate::diagnostic::Diagnostic;
use crate::hir::{
    DeclarationIndex, ResolvedType, ResolvedTypeDeclaration, ResolvedTypeDeclarationKind, TypeFacts,
};
use crate::project::{ProjectRevision, MAX_TOTAL_SOURCE_BYTES};

use super::{intent, parse_revision};

type Result<T> = std::result::Result<T, Vec<Diagnostic>>;
const MAX_FIELDS: usize = 64;
const MAX_DEPTH: usize = 256;
const MAX_ITEMS: usize = 1_048_576;

pub(super) struct FieldAddition {
    pub(super) id: String,
    pub(super) name: String,
    pub(super) owner: String,
    pub(super) path: String,
    pub(super) module: String,
    type_flags: (bool, bool, bool, bool),
}

struct Record<'a> {
    path: &'a str,
    module: &'a str,
    declaration: &'a ResolvedTypeDeclaration,
}

pub(super) fn eligible(revision: &ProjectRevision, target: &str) -> Result<bool> {
    let records = type_inventory(revision);
    let Some(record) = records.get(target) else {
        return Ok(false);
    };
    let ResolvedTypeDeclarationKind::Record { fields } = &record.declaration.kind else {
        return Ok(false);
    };
    let graph: Value = serde_json::from_str(revision.semantic_graph())
        .map_err(|_| invalid("retained record graph is invalid"))?;
    let declarations = graph["declarations"]
        .as_array()
        .ok_or_else(|| invalid("retained record graph lacks declarations"))?;
    Ok(fields.len() < MAX_FIELDS
        && declarations.iter().any(|entry| {
            entry["id"].as_str() == Some(target)
                && entry["identity_origin"].as_str() == Some("explicit")
        })
        && admitted_record(target, &records).is_ok())
}

pub(super) fn apply(
    revision: &ProjectRevision,
    programs: &mut [Program],
    request: &Value,
) -> Result<(intent::IntentSummary, FieldAddition)> {
    object(request, &["kind", "target", "field"])?;
    if text(request, "kind")? != "add_record_field" {
        return Err(invalid("record field change requires add_record_field"));
    }
    let target = text(request, "target")?;
    let field = &request["field"];
    object(field, &["id", "name", "type", "default"])?;
    let id = identifier(text(field, "id")?, true)?;
    let name = identifier(text(field, "name")?, false)?;
    let (ty, default) = default_literal(field)?;
    let records = type_inventory(revision);
    let facts = admitted_record(target, &records)?;
    let record = records
        .get(target)
        .ok_or_else(|| invalid("target record is absent"))?;
    let ResolvedTypeDeclarationKind::Record { fields } = &record.declaration.kind else {
        return Err(invalid("target is not an admitted record"));
    };
    if fields.len() >= MAX_FIELDS {
        return Err(capacity(
            "record field change permits at most sixty-four fields",
        ));
    }
    let old_names = fields
        .iter()
        .map(|field| field.name.clone())
        .collect::<BTreeSet<_>>();
    if old_names.contains(name) {
        return Err(invalid(
            "new field name is already present on the target record",
        ));
    }
    let graph: Value = serde_json::from_str(revision.semantic_graph())
        .map_err(|_| invalid("retained record graph is invalid"))?;
    let declarations = graph["declarations"]
        .as_array()
        .ok_or_else(|| invalid("retained record graph lacks declarations"))?;
    if declarations
        .iter()
        .any(|entry| entry["id"].as_str() == Some(id))
    {
        return Err(invalid(
            "new field identity is already bound in the Project",
        ));
    }
    if !declarations.iter().any(|entry| {
        entry["id"].as_str() == Some(target)
            && entry["identity_origin"].as_str() == Some("explicit")
            && entry["path"].as_str() == Some(record.path)
            && entry["module"].as_str() == Some(record.module)
    }) {
        return Err(invalid(
            "record field change requires an explicit authored record identity",
        ));
    }
    let mut owner = None;
    for (index, program) in programs.iter().enumerate() {
        authenticate_module(revision, program)?;
        for (type_index, declaration) in program.types.iter().enumerate() {
            if declaration.stable_id == target {
                if owner.is_some()
                    || program.path != record.path
                    || program.module != record.module
                    || declaration.name != record.declaration.name
                    || declaration.span != record.declaration.span
                    || !declaration.explicit_id
                    || !declaration.type_parameters.is_empty()
                {
                    return Err(invalid("record source and retained HIR origin disagree"));
                }
                let TypeDeclarationKind::Record { fields } = &declaration.kind else {
                    return Err(invalid("record source declaration kind disagrees"));
                };
                exact_fields(fields.iter().map(|field| field.name.as_str()), &old_names)?;
                owner = Some((index, type_index));
            }
        }
    }
    let (owner, type_index) = owner.ok_or_else(|| invalid("target record source is absent"))?;
    let addition = FieldAddition {
        id: id.to_owned(),
        name: name.to_owned(),
        owner: target.to_owned(),
        path: record.path.to_owned(),
        module: record.module.to_owned(),
        type_flags: type_flags(&facts),
    };
    let mut nodes = 0;
    let mut additions = 0;
    let mut pattern_nodes = 0;
    for program in programs.iter_mut() {
        let bindings = type_bindings(program, &records)?;
        intent::walk_program(program, &mut nodes, &mut |expression| {
            match &mut expression.kind {
                ExprKind::ConstructRecord {
                    type_name,
                    type_arguments,
                    fields,
                    ..
                } => {
                    if bindings.get(type_name).is_some_and(|id| id == target) {
                        if !type_arguments.is_empty() {
                            return Err(invalid(
                                "monomorphic target constructor has type arguments",
                            ));
                        }
                        exact_fields(fields.iter().map(|field| field.name.as_str()), &old_names)?;
                        charge(&mut additions)?;
                        fields.push(FieldInitializer {
                            name: name.to_owned(),
                            name_span: Span::default(),
                            value: default.clone(),
                            span: Span::default(),
                        });
                    }
                }
                ExprKind::Match { arms, .. } => {
                    for arm in arms {
                        migrate_pattern(
                            &mut arm.pattern,
                            &bindings,
                            target,
                            name,
                            &old_names,
                            0,
                            &mut pattern_nodes,
                            &mut additions,
                        )?;
                    }
                }
                _ => {}
            }
            Ok(())
        })?;
    }
    if nodes
        .checked_add(additions)
        .is_none_or(|count| count > MAX_ITEMS)
    {
        return Err(capacity(
            "record migration and inserted literals exceed the node bound",
        ));
    }
    let TypeDeclarationKind::Record { fields } = &mut programs[owner].types[type_index].kind else {
        return Err(invalid("record source kind changed during migration"));
    };
    fields.push(FieldDeclaration {
        stable_id: id.to_owned(),
        explicit_id: true,
        name: name.to_owned(),
        name_span: Span::default(),
        ty,
        span: Span::default(),
    });
    Ok((
        intent::IntentSummary {
            target_id: target.to_owned(),
            kind: "add_record_field".to_owned(),
            migrated_calls: 0,
        },
        addition,
    ))
}

/// This is an exact independent structural reconstruction after ordinary full
/// Project admission; it does not replace ownership/layout/backend validation.
pub(super) fn validate(
    before: &ProjectRevision,
    after: &ProjectRevision,
    request: &Value,
) -> Result<()> {
    let mut expected = parse_revision(before)?;
    let (_, addition) = apply(before, &mut expected, request)?;
    if expected.len() != after.sources().len() {
        return Err(invalid(
            "record field migration changed the source inventory",
        ));
    }
    for (program, source) in expected.iter().zip(after.sources()) {
        let (canonical, overflow) =
            crate::bounded_output::with_limit(MAX_TOTAL_SOURCE_BYTES, || {
                crate::format::canonical(program)
            });
        if overflow {
            return Err(capacity("record migration replay exceeds source bounds"));
        }
        if program.path != source.path() || canonical != source.source() {
            return Err(invalid(
                "record migration differs from exact independent reconstruction",
            ));
        }
    }
    validate_checked_fields(before, after, request, &addition)
}

fn validate_checked_fields(
    before: &ProjectRevision,
    after: &ProjectRevision,
    request: &Value,
    addition: &FieldAddition,
) -> Result<()> {
    let old_records = type_inventory(before);
    let new_records = type_inventory(after);
    let old = old_records
        .get(&addition.owner)
        .ok_or_else(|| invalid("original record has no retained checked declaration"))?;
    let new = new_records
        .get(&addition.owner)
        .ok_or_else(|| invalid("evolved record has no retained checked declaration"))?;
    let (
        ResolvedTypeDeclarationKind::Record { fields: old_fields },
        ResolvedTypeDeclarationKind::Record { fields: new_fields },
    ) = (&old.declaration.kind, &new.declaration.kind)
    else {
        return Err(invalid(
            "record field migration changed its checked owner kind",
        ));
    };
    if new.path != old.path
        || new.module != old.module
        || new.declaration.name != old.declaration.name
        || new_fields.len() != old_fields.len() + 1
        || !old_fields.iter().zip(new_fields).all(|(old, new)| {
            old.id == new.id && old.name == new.name && old.index == new.index && old.ty == new.ty
        })
    {
        return Err(invalid(
            "record migration changed existing checked fields or their order",
        ));
    }
    let field = new_fields
        .last()
        .ok_or_else(|| invalid("evolved record lacks its appended checked field"))?;
    let ty = match text(&request["field"], "type")? {
        "i64" => ResolvedType::I64,
        "bool" => ResolvedType::Bool,
        "i32" => ResolvedType::I32,
        "u8" => ResolvedType::U8,
        "usize" => ResolvedType::Usize,
        _ => {
            return Err(invalid(
                "appended checked field must retain its scalar type",
            ))
        }
    };
    if field.id.as_str() != addition.id
        || field.name != addition.name
        || field.index as usize != old_fields.len()
        || field.ty != ty
    {
        return Err(invalid(
            "appended checked field differs from its authenticated request",
        ));
    }
    let facts = admitted_record(&addition.owner, &new_records)?;
    if type_flags(&facts) != addition.type_flags {
        return Err(invalid(
            "inert scalar addition changed checked ownership or resource flags",
        ));
    }
    // Layout intentionally changes. Cleanup plans are independently rebuilt by
    // admission; neither their order nor byte equality is inferred here.
    Ok(())
}

fn type_flags(facts: &TypeFacts) -> (bool, bool, bool, bool) {
    (
        facts.copy,
        facts.needs_drop,
        facts.contains_resource,
        facts.sized,
    )
}

fn type_inventory(revision: &ProjectRevision) -> BTreeMap<String, Record<'_>> {
    revision
        .semantic
        .image_modules()
        .iter()
        .flat_map(|module| {
            module.types().iter().map(move |declaration| {
                (
                    declaration.id.as_str().to_owned(),
                    Record {
                        path: module.path(),
                        module: module.module(),
                        declaration,
                    },
                )
            })
        })
        .collect()
}

fn admitted_record(id: &str, records: &BTreeMap<String, Record<'_>>) -> Result<TypeFacts> {
    let record = records
        .get(id)
        .ok_or_else(|| invalid("record type is not an authored retained declaration"))?;
    if !record.declaration.type_parameters.is_empty() {
        return Err(invalid(
            "record field change does not support generic records",
        ));
    }
    let ResolvedTypeDeclarationKind::Record { .. } = &record.declaration.kind else {
        return Err(invalid(
            "record field change requires a checked source record",
        ));
    };
    let declarations = records
        .iter()
        .map(|(id, record)| (id.as_str(), record.declaration))
        .collect();
    let facts =
        DeclarationIndex::record_evolution_type_facts(&record.declaration.id, &declarations)
            .map_err(|diagnostic| vec![diagnostic])?
            .ok_or_else(|| invalid("record has no admitted bounded checked TypeFacts"))?;
    if facts.sized && !facts.contains_resource {
        Ok(facts)
    } else {
        Err(invalid(
            "record field change requires a checked sized resource-free record",
        ))
    }
}

fn authenticate_module(revision: &ProjectRevision, program: &Program) -> Result<()> {
    let module = revision
        .semantic
        .image_modules()
        .iter()
        .find(|module| module.path() == program.path)
        .ok_or_else(|| invalid("record migration source has no retained module"))?;
    let source = revision
        .sources()
        .iter()
        .find(|source| source.path() == program.path)
        .ok_or_else(|| invalid("record migration source is absent"))?;
    if program.module != module.module()
        || source.source_revision() != module.source_revision()
        || source.source_digest() != module.source_digest()
    {
        return Err(invalid("record migration module provenance disagrees"));
    }
    Ok(())
}

fn type_bindings(
    program: &Program,
    records: &BTreeMap<String, Record<'_>>,
) -> Result<BTreeMap<String, String>> {
    let mut bindings = BTreeMap::new();
    for declaration in &program.types {
        if bindings
            .insert(declaration.name.clone(), declaration.stable_id.clone())
            .is_some()
        {
            return Err(invalid(
                "record migration found ambiguous local type bindings",
            ));
        }
    }
    for usage in &program.module_uses {
        if usage.kind == ModuleUseKind::Type {
            let declaration = records
                .get(&usage.persistent_id)
                .ok_or_else(|| invalid("type alias target is absent from authenticated HIR"))?;
            if declaration.module != usage.target_module
                || bindings
                    .insert(usage.alias.clone(), usage.persistent_id.clone())
                    .is_some()
            {
                return Err(invalid(
                    "record migration type alias provenance is ambiguous",
                ));
            }
        }
    }
    Ok(bindings)
}

#[allow(clippy::too_many_arguments)]
fn migrate_pattern(
    pattern: &mut MatchPattern,
    bindings: &BTreeMap<String, String>,
    target: &str,
    name: &str,
    old: &BTreeSet<String>,
    depth: usize,
    nodes: &mut usize,
    additions: &mut usize,
) -> Result<()> {
    pattern_budget(depth, nodes)?;
    match pattern {
        MatchPattern::Record {
            type_name, fields, ..
        } => {
            migrate_record_pattern(
                type_name, fields, bindings, target, name, old, depth, nodes, additions,
            )?;
        }
        MatchPattern::Or { alternatives, .. } => {
            for pattern in alternatives {
                migrate_pattern(
                    pattern,
                    bindings,
                    target,
                    name,
                    old,
                    depth + 1,
                    nodes,
                    additions,
                )?;
            }
        }
        MatchPattern::Variant { .. }
        | MatchPattern::Wildcard { .. }
        | MatchPattern::Literal { .. }
        | MatchPattern::Binding { .. } => {}
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn migrate_record_pattern(
    type_name: &str,
    fields: &mut Vec<RecordMatchPatternField>,
    bindings: &BTreeMap<String, String>,
    target: &str,
    name: &str,
    old: &BTreeSet<String>,
    depth: usize,
    nodes: &mut usize,
    additions: &mut usize,
) -> Result<()> {
    for field in fields.iter_mut() {
        pattern_budget(depth + 1, nodes)?;
        if let RecordMatchFieldPattern::Record {
            type_name, fields, ..
        } = &mut field.pattern
        {
            migrate_record_pattern(
                type_name,
                fields,
                bindings,
                target,
                name,
                old,
                depth + 1,
                nodes,
                additions,
            )?;
        }
    }
    if bindings.get(type_name).is_some_and(|id| id == target) {
        exact_fields(fields.iter().map(|field| field.name.as_str()), old)?;
        charge(additions)?;
        fields.push(RecordMatchPatternField {
            name: name.to_owned(),
            name_span: Span::default(),
            pattern: RecordMatchFieldPattern::Wildcard {
                span: Span::default(),
            },
            span: Span::default(),
        });
    }
    Ok(())
}
fn exact_fields<'a>(names: impl Iterator<Item = &'a str>, old: &BTreeSet<String>) -> Result<()> {
    let mut seen = BTreeSet::new();
    for name in names {
        if !seen.insert(name.to_owned()) {
            return Err(invalid("record use repeats a field"));
        }
    }
    if &seen != old {
        return Err(invalid(
            "record use does not match the authenticated original field inventory",
        ));
    }
    Ok(())
}
fn pattern_budget(depth: usize, nodes: &mut usize) -> Result<()> {
    if depth > MAX_DEPTH {
        return Err(capacity("record pattern migration exceeds depth bound"));
    }
    charge(nodes)
}
fn charge(nodes: &mut usize) -> Result<()> {
    *nodes += 1;
    if *nodes > MAX_ITEMS {
        return Err(capacity("record migration exceeds its item bound"));
    }
    Ok(())
}
fn default_literal(field: &Value) -> Result<(Type, Expr)> {
    let value = &field["default"];
    object(value, &["kind", "value"])?;
    let kind = text(value, "kind")?;
    if kind != text(field, "type")? {
        return Err(invalid(
            "new field type and default literal kind must match exactly",
        ));
    }
    let (ty, kind) = match kind {
        "i64" => (
            Type::I64,
            ExprKind::Int(
                value["value"]
                    .as_i64()
                    .filter(|value| *value != i64::MIN)
                    .ok_or_else(|| {
                        invalid("i64 default must have a representable source literal magnitude")
                    })?,
            ),
        ),
        "bool" => (
            Type::Bool,
            ExprKind::Bool(
                value["value"]
                    .as_bool()
                    .ok_or_else(|| invalid("bool default must be a boolean"))?,
            ),
        ),
        "i32" => (
            Type::I32,
            ExprKind::Int32(
                value["value"]
                    .as_i64()
                    .and_then(|raw| i32::try_from(raw).ok())
                    .filter(|value| *value != i32::MIN)
                    .ok_or_else(|| {
                        invalid("i32 default must have a representable source literal magnitude")
                    })?,
            ),
        ),
        "u8" => (
            Type::U8,
            ExprKind::Uint8(
                value["value"]
                    .as_u64()
                    .and_then(|raw| u8::try_from(raw).ok())
                    .ok_or_else(|| invalid("u8 default must be an exact unsigned 8-bit integer"))?,
            ),
        ),
        "usize" => (
            Type::Usize,
            ExprKind::Usize(value["value"].as_u64().ok_or_else(|| {
                invalid("usize default must be an exact unsigned 64-bit integer")
            })?),
        ),
        _ => {
            return Err(invalid(
                "new record field supports only i64/bool/i32/u8/usize literal defaults",
            ))
        }
    };
    Ok((
        ty,
        Expr {
            kind,
            span: Span::default(),
        },
    ))
}
fn identifier(value: &str, id: bool) -> Result<&str> {
    if value.is_empty()
        || value.len() > 128
        || !value.bytes().enumerate().all(|(index, b)| {
            if id {
                b.is_ascii_lowercase() || b.is_ascii_digit() || matches!(b, b'.' | b'_' | b'-')
            } else {
                b == b'_' || b.is_ascii_alphabetic() || (index > 0 && b.is_ascii_digit())
            }
        })
    {
        return Err(invalid(
            "new field identity/name has invalid bounded grammar",
        ));
    }
    Ok(value)
}
fn object(value: &Value, fields: &[&str]) -> Result<()> {
    let object = value
        .as_object()
        .ok_or_else(|| invalid("record field intention requires an object"))?;
    if object.len() != fields.len() || fields.iter().any(|field| !object.contains_key(*field)) {
        return Err(invalid(
            "record field intention has missing or unknown fields",
        ));
    }
    Ok(())
}
fn text<'a>(value: &'a Value, key: &str) -> Result<&'a str> {
    value
        .get(key)
        .and_then(Value::as_str)
        .ok_or_else(|| invalid("record field intention requires text"))
}
fn invalid(message: &'static str) -> Vec<Diagnostic> {
    vec![Diagnostic::io("SPX-G225", message)]
}
fn capacity(message: &'static str) -> Vec<Diagnostic> {
    vec![Diagnostic::io("SPX-G226", message)]
}
