//! Stable-ID aggregate construction over retained checked type declarations.
//! Source bindings choose spellings; neither spellings nor HIR come from requests.
use std::borrow::Cow;
use std::collections::{BTreeMap, BTreeSet};

use serde_json::{json, Value};

use super::{capacity, grammar, Result, MAX_EXPRESSION_NODES, MAX_ID_BYTES};
use crate::ast::{ModuleUseKind, Program, Type};
use crate::hir::{
    DeclarationIndex, DeclarationKind, IdentityOrigin, ResolvedFieldDeclaration, ResolvedType,
    ResolvedTypeDeclarationKind, ResolvedTypeParameterDeclaration,
};
use crate::project::ProjectRevision;

#[path = "aggregate_match.rs"]
mod matching;
#[path = "aggregate_nominal.rs"]
mod nominal;
pub(super) use matching::match_plan;
pub(in crate::project::candidate) use matching::{
    aggregate_match_dependency_fingerprint, aggregate_matches,
};
pub(in crate::project::candidate) use nominal::{
    nominal_type_dependency_fingerprint, nominal_type_plan, nominal_types, validate_nominal_ast,
};

const MAX_FIELDS: usize = MAX_EXPRESSION_NODES - 1;
pub(in crate::project::candidate) const MAX_AGGREGATE_TYPE_ARGUMENTS: usize =
    MAX_EXPRESSION_NODES - 1;
const MAX_ITEMS: usize = 65_536;
const MAX_CATALOG_BYTES: usize = 1024 * 1024;

pub(super) struct Plan {
    pub(super) type_name: String,
    pub(super) case_name: Option<String>,
    pub(super) fields: BTreeMap<String, String>,
    pub(super) type_arguments: Vec<Type>,
}

pub(super) struct ProjectionPlan {
    pub(super) owner_type: Type,
    pub(super) field_name: String,
}

pub(super) fn projection_plan(
    revision: &ProjectRevision,
    program: &Program,
    target: &str,
    type_arguments: Option<&Value>,
) -> Result<ProjectionPlan> {
    let subject = projection_subject(revision, target)?
        .ok_or_else(|| grammar("projection target must be an explicit checked record field"))?;
    let plan = plan(revision, program, "record", subject.owner, type_arguments)?;
    let field_name =
        plan.fields.get(target).cloned().ok_or_else(|| {
            grammar("projection field is not a member of its exact checked record")
        })?;
    Ok(ProjectionPlan {
        owner_type: Type::Named {
            name: plan.type_name,
            arguments: plan.type_arguments,
        },
        field_name,
    })
}

fn projection_subject<'a>(
    revision: &'a ProjectRevision,
    target: &str,
) -> Result<Option<Subject<'a>>> {
    selector(target)?;
    let mut owner = None;
    for module in revision.semantic.image_modules() {
        for ty in module.types() {
            if let ResolvedTypeDeclarationKind::Record { fields } = &ty.kind {
                for field in fields {
                    if field.id.as_str() == target && owner.replace(ty.id.as_str()).is_some() {
                        return Err(grammar("projection field identity is ambiguous"));
                    }
                }
            }
        }
    }
    match owner {
        Some(owner) => subject(revision, owner),
        None => Ok(None),
    }
}

/// Whole nominal owner shape plus the selected field, independent of aliases.
pub(in crate::project::candidate) fn aggregate_projection_dependency_fingerprint(
    revision: &ProjectRevision,
    target: &str,
) -> Result<Option<Value>> {
    projection_subject(revision, target)?
        .map(|subject| Ok(json!({"field":target,"record":descriptor(&subject,None)?})))
        .transpose()
}

/// Record projections preserve nominal ownership through an exact typed local.
/// This describes ordinary value binding, not a borrowing operation.
pub(in crate::project::candidate) fn aggregate_projections(
    revision: &ProjectRevision,
    program: &Program,
) -> Result<Vec<Value>> {
    let mut result = Vec::new();
    let mut bytes = 2usize;
    let mut items = 0usize;
    for module in revision.semantic.image_modules() {
        for ty in module.types() {
            let ResolvedTypeDeclarationKind::Record { fields } = &ty.kind else {
                continue;
            };
            if fields.len() > MAX_FIELDS || ty.type_parameters.len() > MAX_AGGREGATE_TYPE_ARGUMENTS
            {
                continue;
            }
            let Some(visible) = binding(program, ty.id.as_str(), module.module())? else {
                continue;
            };
            let Some(subject) = subject(revision, ty.id.as_str())? else {
                continue;
            };
            // Each descriptor repeats template metadata; charge that expansion.
            items = items.saturating_add(
                fields
                    .len()
                    .saturating_mul(1 + subject.type_parameters.len()),
            );
            if items > MAX_ITEMS {
                return Err(capacity(
                    "aggregate projection catalogue exceeds its item bound",
                ));
            }
            let owner = descriptor(&subject, Some(&visible))?;
            for field in fields {
                let mut value = json!({"kind":"project","target":field.id.as_str(),"owner":subject.owner,
                    "name":field.name,"index":field.index,"type_identity":field.ty.identity_key(),
                    "path":subject.path,"module":subject.module,"generic":subject.generic,"binding":visible,
                    "evidence_owner":"retained_checked_hir","requires_full_candidate_validation":true,
                    "base_evaluation":"once_into_typed_value_binding"});
                if let Some(parameters) = owner.get("type_parameters") {
                    value["type_parameters"] = parameters.clone();
                }
                let encoded = super::super::wire::render(value.clone(), MAX_CATALOG_BYTES)?;
                bytes = bytes.saturating_add(encoded.len());
                if bytes > MAX_CATALOG_BYTES {
                    return Err(capacity(
                        "aggregate projection catalogue exceeds its byte bound",
                    ));
                }
                result.push(value);
            }
        }
    }
    result.sort_by(|left, right| left["target"].as_str().cmp(&right["target"].as_str()));
    Ok(result)
}

/// Complete record inventories describe the admissible replacement subset;
/// ordinary source checking still owns update, transfer and cleanup semantics.
pub(in crate::project::candidate) fn aggregate_updates(
    revision: &ProjectRevision,
    program: &Program,
) -> Result<Vec<Value>> {
    let mut targets = BTreeSet::new();
    for module in revision.semantic.image_modules() {
        for ty in module.types() {
            if !matches!(&ty.kind, ResolvedTypeDeclarationKind::Record { fields } if fields.len() <= MAX_FIELDS)
                || ty.type_parameters.len() > MAX_AGGREGATE_TYPE_ARGUMENTS
                || binding(program, ty.id.as_str(), module.module())?.is_none()
            {
                continue;
            }
            targets.insert(ty.id.as_str().to_owned());
            if targets.len() > MAX_ITEMS {
                return Err(capacity(
                    "aggregate update catalogue exceeds its item bound",
                ));
            }
        }
    }
    let mut result = Vec::new();
    let mut bytes = 2usize;
    let mut items = 0usize;
    for target in targets {
        let Some(subject) = subject(revision, &target)? else {
            continue;
        };
        items = items.saturating_add(1 + subject.fields.len() + subject.type_parameters.len());
        if items > MAX_ITEMS {
            return Err(capacity(
                "aggregate update catalogue exceeds its member bound",
            ));
        }
        let Some(binding) = visible_binding(program, &subject)? else {
            continue;
        };
        let mut value = descriptor(&subject, Some(&binding))?;
        value["kind"] = json!("update");
        value["base_evaluation"] = json!("once_into_typed_value_binding");
        value["field_coverage"] = json!("subset");
        let encoded = super::super::wire::render(value.clone(), MAX_CATALOG_BYTES)?;
        bytes = bytes.saturating_add(encoded.len());
        if bytes > MAX_CATALOG_BYTES {
            return Err(capacity(
                "aggregate update catalogue exceeds its byte bound",
            ));
        }
        result.push(value);
    }
    Ok(result)
}

struct Subject<'a> {
    kind: &'static str,
    target: &'a str,
    owner: &'a str,
    name: &'a str,
    path: &'a str,
    module: &'a str,
    generic: bool,
    fields: Cow<'a, [ResolvedFieldDeclaration]>,
    type_parameters: Cow<'a, [ResolvedTypeParameterDeclaration]>,
    prelude_binding: Option<&'a str>,
}

pub(super) fn plan(
    revision: &ProjectRevision,
    program: &Program,
    kind: &str,
    target: &str,
    type_arguments: Option<&Value>,
) -> Result<Plan> {
    selector(target)?;
    let subject = subject(revision, target)?.ok_or_else(|| {
        grammar("aggregate constructor target is not a checked record or variant case")
    })?;
    if subject.kind != kind {
        return Err(grammar(
            "aggregate constructor requires the exact record or variant case kind",
        ));
    }
    let arguments = match type_arguments {
        Some(value) => value
            .as_array()
            .ok_or_else(|| grammar("aggregate type_arguments must be an explicit array"))?
            .as_slice(),
        None => &[],
    };
    if arguments.len() > MAX_AGGREGATE_TYPE_ARGUMENTS
        || subject.type_parameters.len() > MAX_AGGREGATE_TYPE_ARGUMENTS
    {
        return Err(capacity(
            "aggregate type argument inventory exceeds its constructor bound",
        ));
    }
    if arguments.len() != subject.type_parameters.len() {
        return Err(grammar("aggregate constructor requires exact explicit generic arity; no type inference is performed"));
    }
    let type_arguments = arguments
        .iter()
        .map(|value| match value.as_str() {
            Some("i64") => Ok(Type::I64),
            Some("bool") => Ok(Type::Bool),
            _ => Err(grammar(
                "aggregate type arguments admit only direct i64 or bool",
            )),
        })
        .collect::<Result<Vec<_>>>()?;
    if subject.fields.len() > MAX_FIELDS {
        return Err(capacity(
            "aggregate constructor field inventory exceeds its node bound",
        ));
    }
    let type_name = visible_binding(program, &subject)?.ok_or_else(|| {
        grammar("aggregate constructor type requires one existing local or imported binding")
    })?;
    let mut fields = BTreeMap::new();
    for field in subject.fields.iter() {
        if fields
            .insert(field.id.as_str().to_owned(), field.name.clone())
            .is_some()
        {
            return Err(grammar("checked aggregate field identity is duplicated"));
        }
    }
    Ok(Plan {
        type_name,
        case_name: (kind == "variant").then(|| subject.name.to_owned()),
        fields,
        type_arguments,
    })
}

/// Exact shape dependency for rebase, independent of a caller's type aliases.
/// Type identities bind ownership-bearing types, not inferred runtime liveness.
pub(in crate::project::candidate) fn aggregate_dependency_fingerprint(
    revision: &ProjectRevision,
    target: &str,
) -> Result<Option<Value>> {
    selector(target)?;
    subject(revision, target)?
        .map(|subject| descriptor(&subject, None))
        .transpose()
}

/// Only uniquely visible, currently supported constructor subjects are listed.
pub(in crate::project::candidate) fn aggregate_constructors(
    revision: &ProjectRevision,
    program: &Program,
) -> Result<Vec<Value>> {
    let mut targets = BTreeSet::new();
    for module in revision.semantic.image_modules() {
        for ty in module.types() {
            if ty.type_parameters.len() > MAX_AGGREGATE_TYPE_ARGUMENTS
                || binding(program, ty.id.as_str(), module.module())?.is_none()
            {
                continue;
            }
            match &ty.kind {
                ResolvedTypeDeclarationKind::Record { fields } if fields.len() <= MAX_FIELDS => {
                    targets.insert(ty.id.as_str().to_owned());
                }
                ResolvedTypeDeclarationKind::Variant { cases } => {
                    for case in cases {
                        if case.fields.len() <= MAX_FIELDS {
                            targets.insert(case.id.as_str().to_owned());
                        }
                    }
                }
                _ => {}
            }
            if targets.len() > MAX_ITEMS {
                return Err(capacity(
                    "aggregate constructor catalogue exceeds its item bound",
                ));
            }
        }
    }
    targets.extend(
        [
            crate::prelude::OPTION_NONE_ID,
            crate::prelude::OPTION_SOME_ID,
            crate::prelude::RESULT_OK_ID,
            crate::prelude::RESULT_ERR_ID,
        ]
        .into_iter()
        .map(str::to_owned),
    );
    if targets.len() > MAX_ITEMS {
        return Err(capacity(
            "aggregate constructor catalogue exceeds its item bound",
        ));
    }
    let mut result = Vec::new();
    let mut bytes = 2usize;
    let mut items = 0usize;
    for target in targets {
        let Some(subject) = subject(revision, &target)? else {
            continue;
        };
        items = items.saturating_add(1 + subject.fields.len() + subject.type_parameters.len());
        if items > MAX_ITEMS {
            return Err(capacity(
                "aggregate constructor catalogue field inventory exceeds its bound",
            ));
        }
        let visible = visible_binding(program, &subject)?
            .ok_or_else(|| grammar("aggregate catalogue binding disappeared"))?;
        let value = descriptor(&subject, Some(&visible))?;
        let encoded = super::super::wire::render(value.clone(), MAX_CATALOG_BYTES)?;
        bytes = bytes.saturating_add(encoded.len());
        if bytes > MAX_CATALOG_BYTES {
            return Err(capacity(
                "aggregate constructor catalogue exceeds its byte bound",
            ));
        }
        result.push(value);
    }
    Ok(result)
}

fn subject<'a>(revision: &'a ProjectRevision, target: &str) -> Result<Option<Subject<'a>>> {
    if let Some(subject) = prelude_subject(revision, target)? {
        return Ok(Some(subject));
    }
    let mut selected = None;
    for module in revision.semantic.image_modules() {
        for ty in module.types() {
            let found = match &ty.kind {
                ResolvedTypeDeclarationKind::Record { fields } if ty.id.as_str() == target => {
                    Some(Subject {
                        kind: "record",
                        target: ty.id.as_str(),
                        owner: ty.id.as_str(),
                        name: &ty.name,
                        path: module.path(),
                        module: module.module(),
                        generic: !ty.type_parameters.is_empty(),
                        fields: Cow::Borrowed(fields),
                        type_parameters: Cow::Borrowed(&ty.type_parameters),
                        prelude_binding: None,
                    })
                }
                ResolvedTypeDeclarationKind::Variant { cases } => cases
                    .iter()
                    .find(|case| case.id.as_str() == target)
                    .map(|case| Subject {
                        kind: "variant",
                        target: case.id.as_str(),
                        owner: ty.id.as_str(),
                        name: &case.name,
                        path: module.path(),
                        module: module.module(),
                        generic: !ty.type_parameters.is_empty(),
                        fields: Cow::Borrowed(&case.fields),
                        type_parameters: Cow::Borrowed(&ty.type_parameters),
                        prelude_binding: None,
                    }),
                _ => None,
            };
            if let Some(found) = found {
                if !explicit_subject(revision, &found) {
                    continue;
                }
                if selected.replace(found).is_some() {
                    return Err(grammar(
                        "aggregate constructor target identity is ambiguous",
                    ));
                }
            }
        }
    }
    Ok(selected)
}

/// This exception authenticates the exact compiler-owned algebraic inventory;
/// it never weakens the explicit source identity checks for authored subjects.
fn prelude_index<'a>(
    revision: &'a ProjectRevision,
    name: &str,
) -> Result<Cow<'a, DeclarationIndex>> {
    // Scalar-only linked closures intentionally omit unused algebraic types.
    // Reuse the fixed compiler builder without changing the linked closure or
    // caching a failure that could depend on an invocation's output budget.
    let retained = &revision.entry_program().declarations;
    if retained.type_id(name).is_some() {
        Ok(Cow::Borrowed(retained))
    } else {
        crate::hir::compiler_prelude_declarations()
            .map(Cow::Owned)
            .map_err(|error| vec![error])
    }
}

fn prelude_subject<'a>(revision: &'a ProjectRevision, target: &str) -> Result<Option<Subject<'a>>> {
    use crate::prelude::*;
    type Payload = (&'static str, &'static str, u32);
    type Case = (&'static str, &'static str, Option<Payload>);
    type PreludeShape = (
        &'static str,
        &'static str,
        &'static [&'static str],
        &'static [Case],
    );
    let (owner, name, parameter_names, cases): PreludeShape = match target {
        OPTION_NONE_ID | OPTION_SOME_ID => (
            OPTION_ID,
            "Option",
            &["T"],
            &[
                (OPTION_NONE_ID, "None", None),
                (
                    OPTION_SOME_ID,
                    "Some",
                    Some((OPTION_SOME_VALUE_ID, "value", 0)),
                ),
            ],
        ),
        RESULT_OK_ID | RESULT_ERR_ID => (
            RESULT_ID,
            "Result",
            &["T", "E"],
            &[
                (RESULT_OK_ID, "Ok", Some((RESULT_OK_VALUE_ID, "value", 0))),
                (
                    RESULT_ERR_ID,
                    "Err",
                    Some((RESULT_ERR_ERROR_ID, "error", 1)),
                ),
            ],
        ),
        _ => return Ok(None),
    };
    let index = prelude_index(revision, name)?;
    let id = index
        .type_id(name)
        .ok_or_else(|| grammar("checked compiler prelude type is absent"))?;
    let declaration = index
        .declaration(id)
        .ok_or_else(|| grammar("checked compiler prelude declaration is absent"))?;
    if id.as_str() != owner
        || declaration.id != *id
        || declaration.name != name
        || declaration.kind != DeclarationKind::Variant
        || declaration.identity_origin != IdentityOrigin::CompilerOwned
        || declaration.owner.is_some()
    {
        return Err(grammar(
            "compiler prelude type identity does not match its checked owner",
        ));
    }
    let parameters = index
        .type_parameters(id)
        .ok_or_else(|| grammar("checked compiler prelude parameters are absent"))?;
    if parameters.len() != parameter_names.len()
        || parameters.iter().zip(parameter_names).enumerate().any(
            |(position, (parameter, name))| {
                parameter.name != *name || parameter.index as usize != position
            },
        )
    {
        return Err(grammar(
            "compiler prelude parameter inventory does not match",
        ));
    }
    let actual = index
        .variant_cases(id)
        .ok_or_else(|| grammar("checked compiler prelude cases are absent"))?;
    if actual.len() != cases.len() {
        return Err(grammar("compiler prelude case inventory does not match"));
    }
    for (position, (case, (expected_id, expected_name, payload))) in
        actual.iter().zip(cases).enumerate()
    {
        let fact = index
            .declaration(&case.id)
            .ok_or_else(|| grammar("checked compiler prelude case identity is absent"))?;
        if case.id.as_str() != *expected_id
            || case.name != *expected_name
            || case.index as usize != position
            || fact.id != case.id
            || fact.name != case.name
            || fact.kind != DeclarationKind::VariantCase
            || fact.identity_origin != IdentityOrigin::CompilerOwned
            || fact.owner.as_ref() != Some(id)
            || case.fields.len() != usize::from(payload.is_some())
        {
            return Err(grammar(
                "compiler prelude case ownership or payload inventory does not match",
            ));
        }
        if let Some((field_id, field_name, parameter_index)) = payload {
            let field = &case.fields[0];
            let fact = index
                .declaration(&field.id)
                .ok_or_else(|| grammar("checked compiler prelude payload identity is absent"))?;
            if field.id.as_str() != *field_id
                || field.name != *field_name
                || field.index != 0
                || fact.id != field.id
                || fact.name != field.name
                || fact.kind != DeclarationKind::CaseField
                || fact.identity_origin != IdentityOrigin::CompilerOwned
                || fact.owner.as_ref() != Some(&case.id)
                || !matches!(&field.ty,ResolvedType::TypeParameter{owner,index} if owner==id && index==parameter_index)
            {
                return Err(grammar(
                    "compiler prelude payload type or ownership does not match",
                ));
            }
        }
    }
    let case = actual
        .iter()
        .find(|case| case.id.as_str() == target)
        .ok_or_else(|| grammar("checked compiler prelude case is absent"))?;
    let (target, case_name, _) = cases
        .iter()
        .find(|(id, _, _)| *id == target)
        .ok_or_else(|| grammar("checked compiler prelude case is absent"))?;
    Ok(Some(Subject {
        kind: "variant",
        target,
        owner,
        name: case_name,
        path: "",
        module: "",
        generic: true,
        fields: Cow::Owned(case.fields.clone()),
        type_parameters: Cow::Owned(parameters.to_vec()),
        prelude_binding: Some(name),
    }))
}

fn visible_binding(program: &Program, subject: &Subject<'_>) -> Result<Option<String>> {
    if let Some(name) = subject.prelude_binding {
        // The source checker reserves these two names. Keep this local join
        // explicit rather than admitting a caller-supplied spelling or alias.
        if program.types.iter().any(|ty| ty.name == name)
            || program
                .module_uses
                .iter()
                .any(|binding| binding.kind == ModuleUseKind::Type && binding.alias == name)
        {
            return Err(grammar("compiler prelude constructor binding is shadowed"));
        }
        Ok(Some(name.to_owned()))
    } else {
        binding(program, subject.owner, subject.module)
    }
}

// These source-origin facts are independently joined during Project admission.
// HIR alone has no explicit-ID bit; require exact graph ownership and origin for
// the type, selected case, and every field before exposing a constructor.
fn explicit_subject(revision: &ProjectRevision, subject: &Subject<'_>) -> bool {
    let explicit = |id: &str, owner: Option<&str>| {
        revision.semantic.image_symbol(id).is_some_and(|fact| {
            fact["identity_origin"] == "explicit"
                && fact["path"] == subject.path
                && fact["module"] == subject.module
                && fact["owner"].as_str() == owner
        })
    };
    explicit(subject.owner, None)
        && (subject.kind != "variant" || explicit(subject.target, Some(subject.owner)))
        && subject
            .fields
            .iter()
            .all(|field| explicit(field.id.as_str(), Some(subject.target)))
}

fn binding(program: &Program, owner: &str, provider: &str) -> Result<Option<String>> {
    let mut names = BTreeSet::new();
    for ty in &program.types {
        if ty.stable_id == owner {
            if program.module != provider {
                return Err(grammar(
                    "aggregate local type provider disagrees with checked identity",
                ));
            }
            names.insert(ty.name.clone());
        }
    }
    for imported in &program.module_uses {
        if imported.kind == ModuleUseKind::Type && imported.persistent_id == owner {
            if imported.target_module != provider {
                return Err(grammar(
                    "aggregate imported type provider disagrees with checked identity",
                ));
            }
            names.insert(imported.alias.clone());
        }
    }
    Ok(if names.len() == 1 {
        names.into_iter().next()
    } else {
        None
    })
}

fn descriptor(subject: &Subject<'_>, binding: Option<&str>) -> Result<Value> {
    if subject.fields.len() > MAX_FIELDS
        || subject.type_parameters.len() > MAX_AGGREGATE_TYPE_ARGUMENTS
    {
        return Err(capacity(
            "aggregate type fingerprint exceeds its field bound",
        ));
    }
    let mut fields = Vec::with_capacity(subject.fields.len());
    let mut bytes = subject.target.len()
        + subject.owner.len()
        + subject.name.len()
        + subject.path.len()
        + subject.module.len()
        + 512;
    for field in subject.fields.iter() {
        let identity = field.ty.identity_key();
        bytes = bytes
            .saturating_add(field.id.as_str().len())
            .saturating_add(field.name.len())
            .saturating_add(identity.len())
            .saturating_add(256);
        if bytes > MAX_CATALOG_BYTES / 6 {
            return Err(capacity(
                "aggregate checked field descriptors exceed their conservative construction bound",
            ));
        }
        fields.push(json!({"target":field.id.as_str(),"name":field.name,"index":field.index,"type_identity":identity}));
    }
    let mut parameters = Vec::new();
    for parameter in subject.type_parameters.iter() {
        bytes = bytes
            .saturating_add(parameter.name.len())
            .saturating_add(256);
        if bytes > MAX_CATALOG_BYTES / 6 {
            return Err(capacity(
                "aggregate template parameter descriptors exceed their construction bound",
            ));
        }
        parameters.push(
            json!({"name":parameter.name,"index":parameter.index,"allowed_types":["i64","bool"]}),
        );
    }
    let mut value = json!({"kind":subject.kind,"target":subject.target,"owner":subject.owner,"name":subject.name,
        "path":subject.path,"module":subject.module,"generic":subject.generic,"fields":fields,
        "evidence_owner":"retained_checked_hir","requires_full_candidate_validation":true});
    if !parameters.is_empty() {
        value["type_parameters"] = json!(parameters);
    }
    if subject.prelude_binding.is_some() {
        value["evidence_owner"] = json!("compiler_checked_prelude");
        value["path"] = Value::Null;
        value["module"] = Value::Null;
        value["identity_origin"] = json!("compiler_owned");
        value["compiler_prelude"] =
            json!({"schema":crate::prelude::SCHEMA_V1,"digest":crate::prelude::digest_text_v1()});
    }
    if let Some(binding) = binding {
        value["binding"] = json!(binding);
    }
    Ok(value)
}

fn selector(id: &str) -> Result<()> {
    if id.is_empty() || id.len() > MAX_ID_BYTES || id.contains('\0') {
        return Err(grammar(
            "aggregate selector must be a bounded stable identity",
        ));
    }
    Ok(())
}
