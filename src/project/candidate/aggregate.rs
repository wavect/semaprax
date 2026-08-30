//! Stable-ID aggregate construction over retained checked type declarations.
//! Source bindings choose spellings; neither spellings nor HIR come from requests.
use std::collections::{BTreeMap, BTreeSet};

use serde_json::{json, Value};

use super::{capacity, grammar, Result, MAX_EXPRESSION_NODES, MAX_ID_BYTES};
use crate::ast::{ModuleUseKind, Program};
use crate::hir::{ResolvedFieldDeclaration, ResolvedTypeDeclarationKind};
use crate::project::ProjectRevision;

const MAX_FIELDS: usize = MAX_EXPRESSION_NODES - 1;
const MAX_ITEMS: usize = 65_536;
const MAX_CATALOG_BYTES: usize = 1024 * 1024;

pub(super) struct Plan {
    pub(super) type_name: String,
    pub(super) case_name: Option<String>,
    pub(super) fields: BTreeMap<String, String>,
}

struct Subject<'a> {
    kind: &'static str,
    target: &'a str,
    owner: &'a str,
    name: &'a str,
    path: &'a str,
    module: &'a str,
    generic: bool,
    fields: &'a [ResolvedFieldDeclaration],
}

pub(super) fn plan(
    revision: &ProjectRevision,
    program: &Program,
    kind: &str,
    target: &str,
) -> Result<Plan> {
    selector(target)?;
    let subject = subject(revision, target)?.ok_or_else(|| {
        grammar("aggregate constructor target is not a checked record or variant case")
    })?;
    if subject.kind != kind || subject.generic {
        return Err(grammar(
            "aggregate constructor requires the exact monomorphic record or variant case kind",
        ));
    }
    if subject.fields.len() > MAX_FIELDS {
        return Err(capacity(
            "aggregate constructor field inventory exceeds its node bound",
        ));
    }
    let type_name = binding(program, subject.owner, subject.module)?.ok_or_else(|| {
        grammar("aggregate constructor type requires one existing local or imported binding")
    })?;
    let mut fields = BTreeMap::new();
    for field in subject.fields {
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
            if !ty.type_parameters.is_empty()
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
    let mut result = Vec::new();
    let mut bytes = 2usize;
    let mut items = 0usize;
    for target in targets {
        let Some(subject) = subject(revision, &target)? else {
            continue;
        };
        items = items.saturating_add(1 + subject.fields.len());
        if items > MAX_ITEMS {
            return Err(capacity(
                "aggregate constructor catalogue field inventory exceeds its bound",
            ));
        }
        let visible = binding(program, subject.owner, subject.module)?
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
                        fields,
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
                        fields: &case.fields,
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
    if subject.fields.len() > MAX_FIELDS {
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
    for field in subject.fields {
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
    let mut value = json!({"kind":subject.kind,"target":subject.target,"owner":subject.owner,"name":subject.name,
        "path":subject.path,"module":subject.module,"generic":subject.generic,"fields":fields,
        "evidence_owner":"retained_checked_hir","requires_full_candidate_validation":true});
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
