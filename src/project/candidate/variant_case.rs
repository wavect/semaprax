//! Append one bounded owning case to an authenticated Copy variant.
//! Existing constructors are evidence only and are never rewritten.

use std::collections::BTreeMap;

use serde_json::Value;

use crate::ast::{
    FieldDeclaration, MatchPattern, ModuleUseKind, Program, Span, Type, TypeDeclarationKind,
    VariantCaseDeclaration,
};
use crate::diagnostic::Diagnostic;
use crate::hir::{
    DeclarationIndex, ResolvedType, ResolvedTypeDeclaration, ResolvedTypeDeclarationKind, TypeFacts,
};
use crate::project::{ProjectRevision, MAX_TOTAL_SOURCE_BYTES};

use super::{intent, parse_revision};

type Result<T> = std::result::Result<T, Vec<Diagnostic>>;
const MAX_CASES: usize = 64;
const MAX_DEPTH: usize = 256;

pub(super) struct VariantCaseAddition {
    pub(super) id: String,
    pub(super) name: String,
    pub(super) field_id: String,
    pub(super) field_name: String,
    pub(super) owner: String,
    pub(super) path: String,
    pub(super) module: String,
    expected_type: ResolvedType,
}

struct Variant<'a> {
    path: &'a str,
    module: &'a str,
    declaration: &'a ResolvedTypeDeclaration,
}

pub(super) fn eligible(revision: &ProjectRevision, target: &str) -> Result<bool> {
    let variants = type_inventory(revision);
    let Some(variant) = variants.get(target) else {
        return Ok(false);
    };
    let ResolvedTypeDeclarationKind::Variant { cases } = &variant.declaration.kind else {
        return Ok(false);
    };
    let graph: Value = serde_json::from_str(revision.semantic_graph())
        .map_err(|_| authentication("retained variant graph is invalid"))?;
    let declarations = graph["declarations"]
        .as_array()
        .ok_or_else(|| authentication("retained variant graph lacks declarations"))?;
    let structurally_eligible = cases.len() < MAX_CASES
        && declarations.iter().any(|entry| {
            entry["id"].as_str() == Some(target)
                && entry["identity_origin"].as_str() == Some("explicit")
        })
        && admitted_variant(target, &variants)
            .is_ok_and(|facts| type_flags(&facts) == (true, false, false, true));
    if !structurally_eligible {
        return Ok(false);
    }
    let programs = parse_revision(revision)?;
    let mut constructors = 0usize;
    let mut nodes = 0usize;
    for program in &programs {
        let bindings = type_bindings(program, &variants)?;
        let mut pattern_found = false;
        let mut probe = program.clone();
        intent::walk_program(&mut probe, &mut nodes, &mut |expression| {
            if let crate::ast::ExprKind::ConstructVariant { type_name, .. } = &expression.kind {
                if bindings.get(type_name).is_some_and(|id| id == target) {
                    constructors = constructors.saturating_add(1);
                }
            }
            if let crate::ast::ExprKind::Match { arms, .. } = &expression.kind {
                for arm in arms {
                    pattern_found |= pattern_mentions_target(&arm.pattern, &bindings, target, 0)?;
                }
            }
            Ok(())
        })?;
        if pattern_found {
            return Ok(false);
        }
    }
    Ok(constructors > 0)
}

pub(super) fn apply(
    revision: &ProjectRevision,
    programs: &mut [Program],
    request: &Value,
) -> Result<(intent::IntentSummary, VariantCaseAddition)> {
    object(request, &["kind", "target", "case"])?;
    if text(request, "kind")? != "add_variant_case" {
        return Err(invalid("variant case change requires add_variant_case"));
    }
    let target = text(request, "target")?;
    let requested = &request["case"];
    object(requested, &["id", "name", "field"])?;
    let case_id = identifier(text(requested, "id")?, true)?;
    let case_name = identifier(text(requested, "name")?, false)?;
    let field = &requested["field"];
    object(field, &["id", "name", "type"])?;
    let field_id = identifier(text(field, "id")?, true)?;
    let field_name = identifier(text(field, "name")?, false)?;
    if case_id == field_id || case_name == field_name {
        return Err(invalid(
            "variant case and field identities and names must be distinct",
        ));
    }
    let (field_type, expected_type) = match text(field, "type")? {
        "Bytes" => (Type::Bytes, ResolvedType::Bytes),
        "string" => {
            return Err(invalid(
                "owning String variant cases remain outside the admitted runtime profile",
            ))
        }
        _ => return Err(invalid("new variant case field must be Bytes")),
    };

    let variants = type_inventory(revision);
    let facts = admitted_variant(target, &variants)?;
    if type_flags(&facts) != (true, false, false, true) {
        return Err(invalid(
            "owning case addition requires an originally Copy, drop-free, sized, resource-free variant",
        ));
    }
    let variant = variants
        .get(target)
        .ok_or_else(|| invalid("target variant is absent"))?;
    let ResolvedTypeDeclarationKind::Variant { cases } = &variant.declaration.kind else {
        return Err(invalid("target is not an admitted variant"));
    };
    if cases.len() >= MAX_CASES {
        return Err(capacity(
            "variant case change permits at most sixty-four cases",
        ));
    }

    let graph: Value = serde_json::from_str(revision.semantic_graph())
        .map_err(|_| authentication("retained variant graph is invalid"))?;
    let declarations = graph["declarations"]
        .as_array()
        .ok_or_else(|| authentication("retained variant graph lacks declarations"))?;
    if declarations
        .iter()
        .any(|entry| matches!(entry["id"].as_str(), Some(id) if id == case_id || id == field_id))
    {
        return Err(invalid(
            "new variant case or field identity is already bound",
        ));
    }
    if !declarations.iter().any(|entry| {
        entry["id"].as_str() == Some(target)
            && entry["identity_origin"].as_str() == Some("explicit")
            && entry["path"].as_str() == Some(variant.path)
            && entry["module"].as_str() == Some(variant.module)
    }) {
        return Err(authentication(
            "variant case change requires an exact explicit authored variant identity",
        ));
    }

    // Names are intentionally globally fresh within the authored type-member
    // inventory, rather than relying on the narrower source namespace alone.
    for program in programs.iter() {
        for declaration in &program.types {
            match &declaration.kind {
                TypeDeclarationKind::Variant { cases } => {
                    for case in cases {
                        if case.name == case_name || case.name == field_name {
                            return Err(invalid("new variant case names must be globally fresh"));
                        }
                        for existing in &case.fields {
                            if existing.name == case_name || existing.name == field_name {
                                return Err(invalid(
                                    "new variant case names must be globally fresh",
                                ));
                            }
                        }
                    }
                }
                TypeDeclarationKind::Record { fields }
                | TypeDeclarationKind::Class { fields, .. } => {
                    if fields
                        .iter()
                        .any(|existing| existing.name == case_name || existing.name == field_name)
                    {
                        return Err(invalid("new variant case names must be globally fresh"));
                    }
                }
                TypeDeclarationKind::Resource { .. } => {}
            }
        }
    }

    let mut owner = None;
    for (program_index, program) in programs.iter().enumerate() {
        authenticate_module(revision, program)?;
        for (type_index, declaration) in program.types.iter().enumerate() {
            if declaration.stable_id == target {
                if owner.is_some()
                    || program.path != variant.path
                    || program.module != variant.module
                    || declaration.name != variant.declaration.name
                    || declaration.span != variant.declaration.span
                    || !declaration.explicit_id
                    || !declaration.type_parameters.is_empty()
                {
                    return Err(authentication(
                        "variant source and retained HIR origin disagree",
                    ));
                }
                let TypeDeclarationKind::Variant {
                    cases: source_cases,
                } = &declaration.kind
                else {
                    return Err(authentication("variant source declaration kind disagrees"));
                };
                authenticate_case_prefix(source_cases, cases)?;
                owner = Some((program_index, type_index));
            }
        }
    }
    let (owner, type_index) =
        owner.ok_or_else(|| authentication("target variant source is absent"))?;

    let mut constructors = 0usize;
    let mut nodes = 0usize;
    for program in programs.iter() {
        let bindings = type_bindings(program, &variants)?;
        let mut probe = program.clone();
        intent::walk_program(&mut probe, &mut nodes, &mut |expression| {
            if let crate::ast::ExprKind::ConstructVariant { type_name, .. } = &expression.kind {
                if bindings.get(type_name).is_some_and(|id| id == target) {
                    constructors = constructors
                        .checked_add(1)
                        .ok_or_else(|| capacity("variant constructor inventory overflowed"))?;
                }
            }
            if let crate::ast::ExprKind::Match { arms, .. } = &expression.kind {
                for arm in arms {
                    if pattern_mentions_target(&arm.pattern, &bindings, target, 0)? {
                        return Err(invalid(
                            "owning case addition rejects existing target variant patterns",
                        ));
                    }
                }
            }
            Ok(())
        })?;
    }
    if constructors == 0 {
        return Err(invalid(
            "owning case addition requires an authenticated existing target constructor",
        ));
    }

    let TypeDeclarationKind::Variant { cases } = &mut programs[owner].types[type_index].kind else {
        return Err(authentication(
            "variant source kind changed during migration",
        ));
    };
    cases.push(VariantCaseDeclaration {
        stable_id: case_id.to_owned(),
        explicit_id: true,
        name: case_name.to_owned(),
        name_span: Span::default(),
        fields: vec![FieldDeclaration {
            stable_id: field_id.to_owned(),
            explicit_id: true,
            name: field_name.to_owned(),
            name_span: Span::default(),
            ty: field_type,
            span: Span::default(),
        }],
        span: Span::default(),
    });
    Ok((
        intent::IntentSummary {
            target_id: target.to_owned(),
            kind: "add_variant_case".to_owned(),
            migrated_calls: 0,
        },
        VariantCaseAddition {
            id: case_id.to_owned(),
            name: case_name.to_owned(),
            field_id: field_id.to_owned(),
            field_name: field_name.to_owned(),
            owner: target.to_owned(),
            path: variant.path.to_owned(),
            module: variant.module.to_owned(),
            expected_type,
        },
    ))
}

pub(super) fn validate(
    before: &ProjectRevision,
    after: &ProjectRevision,
    request: &Value,
) -> Result<()> {
    let mut expected = parse_revision(before)?;
    let (_, addition) = apply(before, &mut expected, request)?;
    if expected.len() != after.sources().len() {
        return Err(authentication(
            "variant case change altered source inventory",
        ));
    }
    for (program, source) in expected.iter().zip(after.sources()) {
        let (canonical, overflow) =
            crate::bounded_output::with_limit(MAX_TOTAL_SOURCE_BYTES, || {
                crate::format::canonical(program)
            });
        if overflow {
            return Err(capacity("variant case replay exceeds source bounds"));
        }
        if program.path != source.path() || canonical != source.source() {
            return Err(authentication(
                "variant case change differs from independent reconstruction",
            ));
        }
    }
    validate_checked_cases(before, after, &addition)
}

fn validate_checked_cases(
    before: &ProjectRevision,
    after: &ProjectRevision,
    addition: &VariantCaseAddition,
) -> Result<()> {
    let old_variants = type_inventory(before);
    let new_variants = type_inventory(after);
    let old = old_variants
        .get(&addition.owner)
        .ok_or_else(|| authentication("original variant lacks retained HIR"))?;
    let new = new_variants
        .get(&addition.owner)
        .ok_or_else(|| authentication("evolved variant lacks retained HIR"))?;
    let (
        ResolvedTypeDeclarationKind::Variant { cases: old_cases },
        ResolvedTypeDeclarationKind::Variant { cases: new_cases },
    ) = (&old.declaration.kind, &new.declaration.kind)
    else {
        return Err(authentication("variant case change altered owner kind"));
    };
    if new.path != old.path
        || new.module != old.module
        || new.declaration.name != old.declaration.name
        || new_cases.len() != old_cases.len() + 1
        || !old_cases.iter().zip(new_cases).all(|(old, new)| {
            old.id == new.id
                && old.name == new.name
                && old.index == new.index
                && old.fields == new.fields
        })
    {
        return Err(authentication(
            "variant case change did not preserve the exact retained case prefix",
        ));
    }
    let case = new_cases
        .last()
        .ok_or_else(|| authentication("evolved variant lacks appended case"))?;
    if case.id.as_str() != addition.id
        || case.name != addition.name
        || case.index as usize != old_cases.len()
        || case.fields.len() != 1
    {
        return Err(authentication(
            "appended checked variant case differs from request",
        ));
    }
    let field = &case.fields[0];
    if field.id.as_str() != addition.field_id
        || field.name != addition.field_name
        || field.index != 0
        || field.ty != addition.expected_type
    {
        return Err(authentication(
            "appended checked case field differs from request",
        ));
    }
    let facts = admitted_variant(&addition.owner, &new_variants)?;
    if type_flags(&facts) != (false, true, false, true) {
        return Err(authentication(
            "variant case change did not produce the exact Copy-to-needs-drop transition",
        ));
    }
    Ok(())
}

fn type_inventory(revision: &ProjectRevision) -> BTreeMap<String, Variant<'_>> {
    revision
        .semantic
        .image_modules()
        .iter()
        .flat_map(|module| {
            module.types().iter().map(move |declaration| {
                (
                    declaration.id.as_str().to_owned(),
                    Variant {
                        path: module.path(),
                        module: module.module(),
                        declaration,
                    },
                )
            })
        })
        .collect()
}

fn admitted_variant(id: &str, variants: &BTreeMap<String, Variant<'_>>) -> Result<TypeFacts> {
    let variant = variants
        .get(id)
        .ok_or_else(|| invalid("variant type is not an authored retained declaration"))?;
    if !variant.declaration.type_parameters.is_empty() {
        return Err(invalid(
            "variant case change does not support generic variants",
        ));
    }
    let ResolvedTypeDeclarationKind::Variant { .. } = &variant.declaration.kind else {
        return Err(invalid(
            "variant case change requires a checked source variant",
        ));
    };
    let declarations = variants
        .iter()
        .map(|(id, variant)| (id.as_str(), variant.declaration))
        .collect();
    let facts =
        DeclarationIndex::record_evolution_type_facts(&variant.declaration.id, &declarations)
            .map_err(|_| capacity("selected variant TypeFacts closure exceeds its bound"))?
            .ok_or_else(|| authentication("variant lacks bounded checked TypeFacts"))?;
    if facts.sized && !facts.contains_resource {
        Ok(facts)
    } else {
        Err(invalid(
            "variant case change requires a checked sized resource-free variant",
        ))
    }
}

fn type_flags(facts: &TypeFacts) -> (bool, bool, bool, bool) {
    (
        facts.copy,
        facts.needs_drop,
        facts.contains_resource,
        facts.sized,
    )
}

fn authenticate_module(revision: &ProjectRevision, program: &Program) -> Result<()> {
    let module = revision
        .semantic
        .image_modules()
        .iter()
        .find(|module| module.path() == program.path)
        .ok_or_else(|| authentication("variant source has no retained module"))?;
    let source = revision
        .sources()
        .iter()
        .find(|source| source.path() == program.path)
        .ok_or_else(|| authentication("variant source is absent"))?;
    if program.module != module.module()
        || source.source_revision() != module.source_revision()
        || source.source_digest() != module.source_digest()
    {
        return Err(authentication("variant module provenance disagrees"));
    }
    Ok(())
}

fn authenticate_case_prefix(
    source: &[VariantCaseDeclaration],
    checked: &[crate::hir::ResolvedVariantCaseDeclaration],
) -> Result<()> {
    if source.len() != checked.len()
        || source
            .iter()
            .zip(checked)
            .enumerate()
            .any(|(index, (source, checked))| {
                !source.explicit_id
                    || source.stable_id.as_str() != checked.id.as_str()
                    || source.name != checked.name
                    || checked.index as usize != index
                    || source.fields.len() != checked.fields.len()
                    || source.fields.iter().zip(&checked.fields).enumerate().any(
                        |(field_index, (source, checked))| {
                            !source.explicit_id
                                || source.stable_id.as_str() != checked.id.as_str()
                                || source.name != checked.name
                                || checked.index as usize != field_index
                        },
                    )
            })
    {
        return Err(authentication(
            "source variant cases disagree with retained checked HIR",
        ));
    }
    Ok(())
}

fn type_bindings(
    program: &Program,
    variants: &BTreeMap<String, Variant<'_>>,
) -> Result<BTreeMap<String, String>> {
    let mut bindings = BTreeMap::new();
    for declaration in &program.types {
        if bindings
            .insert(declaration.name.clone(), declaration.stable_id.clone())
            .is_some()
        {
            return Err(authentication("variant type bindings are ambiguous"));
        }
    }
    for usage in &program.module_uses {
        if usage.kind == ModuleUseKind::Type {
            let declaration = variants
                .get(&usage.persistent_id)
                .ok_or_else(|| authentication("variant alias target lacks retained HIR"))?;
            if declaration.module != usage.target_module
                || bindings
                    .insert(usage.alias.clone(), usage.persistent_id.clone())
                    .is_some()
            {
                return Err(authentication("variant alias provenance is ambiguous"));
            }
        }
    }
    Ok(bindings)
}

fn pattern_mentions_target(
    pattern: &MatchPattern,
    bindings: &BTreeMap<String, String>,
    target: &str,
    depth: usize,
) -> Result<bool> {
    if depth > MAX_DEPTH {
        return Err(capacity(
            "variant pattern inspection exceeds its depth bound",
        ));
    }
    match pattern {
        MatchPattern::Variant { type_name, .. } => {
            Ok(bindings.get(type_name).is_some_and(|id| id == target))
        }
        MatchPattern::Or { alternatives, .. } => {
            for alternative in alternatives {
                if pattern_mentions_target(alternative, bindings, target, depth + 1)? {
                    return Ok(true);
                }
            }
            Ok(false)
        }
        MatchPattern::Record { .. }
        | MatchPattern::Wildcard { .. }
        | MatchPattern::Literal { .. }
        | MatchPattern::Binding { .. } => Ok(false),
    }
}

fn identifier(value: &str, id: bool) -> Result<&str> {
    if value.is_empty()
        || value.len() > 128
        || !value.bytes().enumerate().all(|(index, byte)| {
            if id {
                byte.is_ascii_lowercase()
                    || byte.is_ascii_digit()
                    || matches!(byte, b'.' | b'_' | b'-')
            } else {
                byte == b'_' || byte.is_ascii_alphabetic() || (index > 0 && byte.is_ascii_digit())
            }
        })
    {
        return Err(invalid(
            "new variant case identity or name has invalid grammar",
        ));
    }
    Ok(value)
}

fn object(value: &Value, fields: &[&str]) -> Result<()> {
    let object = value
        .as_object()
        .ok_or_else(|| invalid("variant case intention requires an object"))?;
    if object.len() != fields.len() || fields.iter().any(|field| !object.contains_key(*field)) {
        return Err(invalid(
            "variant case intention has missing or unknown fields",
        ));
    }
    Ok(())
}

fn text<'a>(value: &'a Value, key: &str) -> Result<&'a str> {
    value
        .get(key)
        .and_then(Value::as_str)
        .ok_or_else(|| invalid("variant case intention requires text"))
}

fn invalid(message: &'static str) -> Vec<Diagnostic> {
    vec![Diagnostic::io("SPX-G516", message)]
}
fn authentication(message: &'static str) -> Vec<Diagnostic> {
    vec![Diagnostic::io("SPX-G517", message)]
}
fn capacity(message: &'static str) -> Vec<Diagnostic> {
    vec![Diagnostic::io("SPX-G518", message)]
}
