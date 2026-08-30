//! Complete checked variant inventories for typed, exhaustive value matching.
use super::*;
use crate::hir::ResolvedVariantCaseDeclaration;

pub(in crate::project::candidate::intent) struct MatchCasePlan {
    pub(in crate::project::candidate::intent) name: String,
    pub(in crate::project::candidate::intent) fields: BTreeMap<String, String>,
}

pub(in crate::project::candidate::intent) struct MatchPlan {
    pub(in crate::project::candidate::intent) type_name: String,
    pub(in crate::project::candidate::intent) owner_type: Type,
    pub(in crate::project::candidate::intent) cases: BTreeMap<String, MatchCasePlan>,
}

struct VariantSubject<'a> {
    first: Subject<'a>,
    name: &'a str,
    cases: &'a [ResolvedVariantCaseDeclaration],
}

fn variant_subject<'a>(
    revision: &'a ProjectRevision,
    target: &str,
) -> Result<Option<VariantSubject<'a>>> {
    selector(target)?;
    let prelude_case = match target {
        crate::prelude::OPTION_ID => Some(crate::prelude::OPTION_NONE_ID),
        crate::prelude::RESULT_ID => Some(crate::prelude::RESULT_OK_ID),
        _ => None,
    };
    if let Some(case) = prelude_case {
        let first = prelude_subject(revision, case)?
            .ok_or_else(|| grammar("checked match prelude is absent"))?;
        let name = first
            .prelude_binding
            .ok_or_else(|| grammar("checked match prelude binding is absent"))?;
        let index = &revision.entry_program().declarations;
        let id = index
            .type_id(name)
            .ok_or_else(|| grammar("checked match prelude owner is absent"))?;
        let cases = index
            .variant_cases(id)
            .ok_or_else(|| grammar("checked match prelude cases are absent"))?;
        return Ok(Some(VariantSubject { first, name, cases }));
    }
    let mut selected = None;
    for module in revision.semantic.image_modules() {
        for ty in module.types() {
            if ty.id.as_str() != target {
                continue;
            }
            let ResolvedTypeDeclarationKind::Variant { cases } = &ty.kind else {
                continue;
            };
            let Some(first_case) = cases.first() else {
                continue;
            };
            let make = |case: &'a ResolvedVariantCaseDeclaration| Subject {
                kind: "variant",
                target: case.id.as_str(),
                owner: ty.id.as_str(),
                name: &case.name,
                path: module.path(),
                module: module.module(),
                generic: !ty.type_parameters.is_empty(),
                fields: &case.fields,
                type_parameters: &ty.type_parameters,
                prelude_binding: None,
            };
            if !cases
                .iter()
                .all(|case| explicit_subject(revision, &make(case)))
            {
                continue;
            }
            if selected
                .replace(VariantSubject {
                    first: make(first_case),
                    name: &ty.name,
                    cases,
                })
                .is_some()
            {
                return Err(grammar("match variant identity is ambiguous"));
            }
        }
    }
    Ok(selected)
}

fn inventory(subject: &VariantSubject<'_>) -> Result<usize> {
    let mut items = subject
        .cases
        .len()
        .saturating_add(subject.first.type_parameters.len());
    for case in subject.cases {
        items = items.saturating_add(case.fields.len());
    }
    if subject.cases.is_empty() || items > MAX_FIELDS {
        return Err(capacity(
            "match variant inventory exceeds the shared constructor bound",
        ));
    }
    Ok(items)
}

pub(in crate::project::candidate::intent) fn match_plan(
    revision: &ProjectRevision,
    program: &Program,
    target: &str,
    arguments: Option<&Value>,
) -> Result<MatchPlan> {
    let subject = variant_subject(revision, target)?
        .ok_or_else(|| grammar("match target must be an exact checked variant owner"))?;
    inventory(&subject)?;
    let base = plan(
        revision,
        program,
        "variant",
        subject.first.target,
        arguments,
    )?;
    let mut cases = BTreeMap::new();
    for case in subject.cases {
        let mut fields = BTreeMap::new();
        for field in &case.fields {
            if fields
                .insert(field.id.as_str().to_owned(), field.name.clone())
                .is_some()
            {
                return Err(grammar("checked match payload identity is duplicated"));
            }
        }
        if cases
            .insert(
                case.id.as_str().to_owned(),
                MatchCasePlan {
                    name: case.name.clone(),
                    fields,
                },
            )
            .is_some()
        {
            return Err(grammar("checked match case identity is duplicated"));
        }
    }
    Ok(MatchPlan {
        owner_type: Type::Named {
            name: base.type_name.clone(),
            arguments: base.type_arguments,
        },
        type_name: base.type_name,
        cases,
    })
}

fn match_descriptor(subject: &VariantSubject<'_>, binding: Option<&str>) -> Result<Value> {
    inventory(subject)?;
    let mut value = descriptor(&subject.first, binding)?;
    let object = value
        .as_object_mut()
        .expect("compiler descriptor is an object");
    object.remove("owner");
    object.remove("fields");
    object.insert("kind".to_owned(), json!("match"));
    object.insert("target".to_owned(), json!(subject.first.owner));
    object.insert("name".to_owned(), json!(subject.name));
    object.insert(
        "base_evaluation".to_owned(),
        json!("once_into_typed_value_binding"),
    );
    let mut cases = Vec::new();
    let mut bytes = 0usize;
    for case in subject.cases {
        bytes = bytes
            .saturating_add(case.id.as_str().len())
            .saturating_add(case.name.len())
            .saturating_add(256);
        let mut fields = Vec::new();
        for field in &case.fields {
            let identity = field.ty.identity_key();
            bytes = bytes
                .saturating_add(field.id.as_str().len())
                .saturating_add(field.name.len())
                .saturating_add(identity.len())
                .saturating_add(256);
            if bytes > MAX_CATALOG_BYTES / 6 {
                return Err(capacity(
                    "match descriptor exceeds its conservative construction bound",
                ));
            }
            fields.push(json!({"target":field.id.as_str(),"name":field.name,"index":field.index,"type_identity":identity}));
        }
        if bytes > MAX_CATALOG_BYTES / 6 {
            return Err(capacity(
                "match descriptor exceeds its conservative construction bound",
            ));
        }
        cases.push(
            json!({"target":case.id.as_str(),"name":case.name,"index":case.index,"fields":fields}),
        );
    }
    value["cases"] = json!(cases);
    // The inherited template metadata and complete case inventory share one
    // exact wire bound, including on the fingerprint-only path.
    super::super::super::wire::render(value.clone(), MAX_CATALOG_BYTES)?;
    Ok(value)
}

pub(in crate::project::candidate) fn aggregate_match_dependency_fingerprint(
    revision: &ProjectRevision,
    target: &str,
) -> Result<Option<Value>> {
    variant_subject(revision, target)?
        .map(|subject| match_descriptor(&subject, None))
        .transpose()
}

pub(in crate::project::candidate) fn aggregate_matches(
    revision: &ProjectRevision,
    program: &Program,
) -> Result<Vec<Value>> {
    let mut targets = BTreeSet::new();
    for module in revision.semantic.image_modules() {
        for ty in module.types() {
            if matches!(&ty.kind, ResolvedTypeDeclarationKind::Variant { .. })
                && binding(program, ty.id.as_str(), module.module())?.is_some()
            {
                targets.insert(ty.id.as_str().to_owned());
                if targets.len() > MAX_ITEMS {
                    return Err(capacity("match catalogue exceeds its item bound"));
                }
            }
        }
    }
    targets.extend([
        crate::prelude::OPTION_ID.to_owned(),
        crate::prelude::RESULT_ID.to_owned(),
    ]);
    let mut result = Vec::new();
    let mut bytes = 2usize;
    let mut items = 0usize;
    for target in targets {
        let Some(subject) = variant_subject(revision, &target)? else {
            continue;
        };
        items = items.saturating_add(inventory(&subject)? + 1);
        if items > MAX_ITEMS {
            return Err(capacity("match catalogue exceeds its member bound"));
        }
        let Some(binding) = visible_binding(program, &subject.first)? else {
            continue;
        };
        let value = match_descriptor(&subject, Some(&binding))?;
        let encoded = super::super::super::wire::render(value.clone(), MAX_CATALOG_BYTES)?;
        bytes = bytes.saturating_add(encoded.len());
        if bytes > MAX_CATALOG_BYTES {
            return Err(capacity("match catalogue exceeds its byte bound"));
        }
        result.push(value);
    }
    Ok(result)
}
