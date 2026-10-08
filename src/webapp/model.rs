//! The webapp model: entities, enumerations, and every convention function
//! classified, bound, and translated.

use std::collections::BTreeMap;

mod v3;

use super::translate::{self, Bound, Translator};
use super::{shape_error, snake, Ty, FIELD_TYPE_HELP};
use crate::ast::{Function, ParamMode, Program, Span, Type, TypeDeclarationKind};
use crate::diagnostic::Diagnostic;

#[derive(Debug)]
pub(super) struct Field {
    pub(super) name: String,
    pub(super) ty: Ty,
    /// The referenced entity path for an `x_id: i64` reference field.
    pub(super) reference: Option<String>,
}

#[derive(Debug)]
pub(super) struct Rollup {
    pub(super) name: String,
    pub(super) kind: &'static str,
    pub(super) child: String,
    pub(super) via: String,
    pub(super) field: Option<String>,
    pub(super) ty: Ty,
}

#[derive(Debug, Default)]
pub(super) struct Entity {
    pub(super) name: String,
    pub(super) path: String,
    pub(super) fields: Vec<Field>,
    pub(super) rules: Vec<String>,
    pub(super) computed: Vec<String>,
    pub(super) keys: Vec<String>,
    pub(super) steps: Vec<String>,
    pub(super) constraints: Vec<String>,
    pub(super) migrations: Vec<String>,
    pub(super) rollups: Vec<Rollup>,
    pub(super) can_read: Option<String>,
    pub(super) can_write: Option<String>,
    /// Plain-text facts for `webapp --api`: computed names with types,
    /// key fields, and workflow fields.
    pub(super) summary: Vec<String>,
}

pub(super) struct Model {
    pub(super) enums: BTreeMap<String, Vec<String>>,
    pub(super) entities: Vec<Entity>,
    pub(super) account: Option<String>,
    /// The account entity's path and sign-in field, for the API listing.
    pub(super) login: Option<(String, String)>,
    pub(super) helpers: String,
}

/// What a convention function means, from its name after the entity prefix.
#[derive(Clone, Debug, PartialEq)]
enum Kind {
    Valid,
    Constraint,
    Migration(String),
    Key,
    Step(String),
    Account,
    CanRead,
    CanWrite,
    Computed,
}

/// A computed field's signature, known before any body is translated so
/// rollups may name computed fields of other entities.
struct ComputedSignature {
    entity: usize,
    name: String,
    ty: Option<Ty>,
    uses_rollups: bool,
}

pub(super) fn field_type(
    ty: &Type,
    name: &str,
    enums: &BTreeMap<String, Vec<String>>,
    paths: &BTreeMap<String, String>,
) -> Option<(Ty, Option<String>)> {
    Some(match ty {
        Type::I64 => {
            let reference = name
                .strip_suffix("_id")
                .filter(|prefix| paths.contains_key(*prefix))
                .map(str::to_owned);
            (Ty::Int, reference)
        }
        Type::F64 => (Ty::Float, None),
        Type::Bool => (Ty::Bool, None),
        Type::String => (Ty::Str, None),
        Type::Char => (Ty::Char, None),
        Type::Named { name, arguments } if arguments.is_empty() && enums.contains_key(name) => {
            (Ty::Enum(name.clone()), None)
        }
        _ => return None,
    })
}

/// The entity whose `<path>_` or run-together `<lowercase name>_` prefix is
/// the longest prefix of `name`, with that prefix's length.
pub(super) fn owning_entity(entities: &[Entity], name: &str) -> Option<(usize, usize)> {
    entities
        .iter()
        .enumerate()
        .flat_map(|(index, entity)| {
            [entity.path.clone(), entity.name.to_ascii_lowercase()]
                .into_iter()
                .map(move |prefix| (index, prefix))
        })
        .filter(|(_, prefix)| {
            name.len() > prefix.len() + 1
                && name.starts_with(prefix.as_str())
                && name.as_bytes()[prefix.len()] == b'_'
        })
        .map(|(index, prefix)| (index, prefix.len()))
        .max_by_key(|(_, length)| *length)
}

fn kind(entity: &Entity, suffix: &str) -> Kind {
    match suffix {
        "valid" => Kind::Valid,
        "account" => Kind::Account,
        "can_read" => Kind::CanRead,
        "can_write" => Kind::CanWrite,
        "constraint" => Kind::Constraint,
        _ if suffix.starts_with("constraint_") => Kind::Constraint,
        _ if suffix.starts_with("migrate_") => Kind::Migration(suffix[8..].to_owned()),
        "key" => Kind::Key,
        _ if suffix.starts_with("key_") => Kind::Key,
        _ => match suffix.strip_suffix("_step") {
            Some(field)
                if entity
                    .fields
                    .iter()
                    .any(|f| f.name == field && matches!(f.ty, Ty::Enum(_))) =>
            {
                Kind::Step(field.to_owned())
            }
            _ => Kind::Computed,
        },
    }
}

fn entities_of(
    program: &Program,
    enums: &BTreeMap<String, Vec<String>>,
    errors: &mut Vec<Diagnostic>,
) -> Vec<Entity> {
    let mut paths = BTreeMap::new();
    for declaration in &program.types {
        if let TypeDeclarationKind::Record { .. } = declaration.kind {
            let path = snake(&declaration.name);
            if let Some(previous) = paths.insert(path.clone(), declaration.name.clone()) {
                errors.push(shape_error(
                    format!(
                        "records `{previous}` and `{}` share webapp route `{path}`",
                        declaration.name
                    ),
                    declaration.name_span,
                    "rename one record so its snake_case route is unique",
                ));
            }
        }
    }
    let mut entities = Vec::new();
    for declaration in &program.types {
        let TypeDeclarationKind::Record { fields } = &declaration.kind else {
            continue;
        };
        if !declaration.type_parameters.is_empty() {
            errors.push(shape_error(
                format!(
                    "generic record `{}` cannot be a webapp entity",
                    declaration.name
                ),
                declaration.name_span,
                "declare a concrete record",
            ));
            continue;
        }
        if matches!(snake(&declaration.name).as_str(), "audit" | "session") {
            errors.push(shape_error(
                format!("record `{}` uses a reserved webapp route", declaration.name),
                declaration.name_span,
                "`/api/audit` and `/api/session` belong to the server; rename the record",
            ));
            continue;
        }
        let mut projected = Vec::new();
        for field in fields {
            if field.name == "id" || field.name == "password" {
                errors.push(shape_error(
                    format!("field `{}.{}` is reserved", declaration.name, field.name),
                    field.name_span,
                    "the webapp assigns every row an `id` and keeps account passwords itself; remove this field",
                ));
                continue;
            }
            match field_type(&field.ty, &field.name, enums, &paths) {
                Some((ty, reference)) => projected.push(Field {
                    name: field.name.clone(),
                    ty,
                    reference,
                }),
                None => errors.push(shape_error(
                    format!(
                        "field `{}.{}` has a type the webapp projection does not admit",
                        declaration.name, field.name
                    ),
                    field.name_span,
                    FIELD_TYPE_HELP,
                )),
            }
        }
        entities.push(Entity {
            name: declaration.name.clone(),
            path: snake(&declaration.name),
            fields: projected,
            ..Entity::default()
        });
    }
    if entities.is_empty() {
        errors.push(shape_error(
            "the module declares no record to serve",
            Span::default(),
            "declare at least one `record`; each record becomes a webapp entity",
        ));
    }
    entities
}

const PARAM_HELP: &str = "a parameter is a field of the entity (same name and type); in `can_read`/`can_write` also `me: i64` or `my_<account field>`; in a computed field also a rollup `count_<child>`, `count_<child>_<bool field>`, or `sum_<child>_<number field>`; a `_step` function takes two values of the field's variant";

/// Build the whole model, or every diagnostic that prevents it.
pub(super) fn build(program: &Program, source: &str) -> Result<Model, Vec<Diagnostic>> {
    let mut errors = Vec::new();
    let mut enums: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for declaration in &program.types {
        if let TypeDeclarationKind::Variant { cases } = &declaration.kind {
            if declaration.type_parameters.is_empty() && cases.iter().all(|c| c.fields.is_empty()) {
                enums.insert(
                    declaration.name.clone(),
                    cases.iter().map(|case| case.name.clone()).collect(),
                );
            }
        }
    }
    let mut entities = entities_of(program, &enums, &mut errors);
    if !errors.is_empty() {
        return Err(errors);
    }

    // Classify every entity function and record computed signatures first.
    let mut classified = Vec::new();
    let mut signatures = Vec::new();
    for function in &program.functions {
        let Some((index, prefix)) = owning_entity(&entities, &function.name) else {
            continue;
        };
        let suffix = function.name[prefix + 1..].to_owned();
        let kind = kind(&entities[index], &suffix);
        if kind == Kind::Computed {
            let fields = &entities[index].fields;
            signatures.push(ComputedSignature {
                entity: index,
                name: suffix.clone(),
                ty: scalar(&function.return_type, &enums),
                uses_rollups: function.params.iter().any(|param| {
                    !fields.iter().any(|field| field.name == param.name)
                        && (param.name.starts_with("count_") || param.name.starts_with("sum_"))
                }),
            });
        }
        if !function.effects.is_empty() || !function.type_parameters.is_empty() {
            errors.push(shape_error(
                format!(
                    "entity function `{}` must be pure and monomorphic",
                    function.name
                ),
                function.name_span,
                "remove `uses { … }` and type parameters",
            ));
        }
        classified.push((function, index, suffix, kind));
    }
    let accounts: Vec<&(&Function, usize, String, Kind)> = classified
        .iter()
        .filter(|(_, _, _, kind)| *kind == Kind::Account)
        .collect();
    if accounts.len() > 1 {
        errors.push(shape_error(
            "more than one `<entity>_account` function",
            accounts[1].0.name_span,
            "exactly one entity holds the accounts",
        ));
    }
    let account_entity = accounts.first().map(|(_, index, _, _)| *index);
    if !errors.is_empty() {
        return Err(errors);
    }

    let functions: BTreeMap<&str, &Function> = program
        .functions
        .iter()
        .map(|function| (function.name.as_str(), function))
        .collect();
    let invariants = invariant_rules(program, &entities);
    let mut translator = Translator::new(source, &enums, &functions);
    // Record Invariants v1: a record's `requires` clauses are entity rules
    // over all of its fields, exactly like a `<entity>_valid` function's.
    for (index, context, bound) in &invariants {
        match translator.rules(context, bound) {
            Ok(rules) => entities[*index].rules.extend(rules),
            Err(mut more) => errors.append(&mut more),
        }
    }
    let mut account = None;
    let mut login_field = None;
    let mut migration_fields = std::collections::BTreeSet::new();
    for (function, index, suffix, kind) in &classified {
        if let Kind::Migration(field) = kind {
            if !migration_fields.insert((*index, field.clone())) {
                errors.push(shape_error(
                    format!("duplicate migration for `{field}`"),
                    function.name_span,
                    "keep exactly one migration for each destination field",
                ));
                continue;
            }
        }
        if matches!(kind, Kind::Constraint | Kind::Migration(_)) {
            match v3::project(
                function,
                *index,
                suffix,
                kind,
                &entities,
                &enums,
                &mut translator,
            ) {
                Ok((constraint, value)) => {
                    if constraint {
                        entities[*index]
                            .summary
                            .push(format!("constraint({suffix})"));
                        entities[*index].constraints.push(value);
                    } else {
                        entities[*index].summary.push(format!("{suffix}()"));
                        entities[*index].migrations.push(value);
                    }
                }
                Err(mut more) => errors.append(&mut more),
            }
            continue;
        }
        let bound = match bind(
            function,
            *index,
            kind,
            &entities,
            account_entity,
            &signatures,
            &enums,
        ) {
            Ok((bound, rollups)) => {
                for rollup in rollups {
                    if !entities[*index]
                        .rollups
                        .iter()
                        .any(|r| r.name == rollup.name)
                    {
                        entities[*index].rollups.push(rollup);
                    }
                }
                bound
            }
            Err(mut more) => {
                errors.append(&mut more);
                continue;
            }
        };
        let returns_bool = || {
            if function.return_type == Type::Bool {
                Ok(())
            } else {
                Err(vec![shape_error(
                    format!("`{}` must return bool", function.name),
                    function.name_span,
                    "keys may return any webapp scalar; `_step`, `_account`, `can_read`, and `can_write` return bool",
                )])
            }
        };
        let entity = &mut entities[*index];
        let result = match kind {
            Kind::Constraint | Kind::Migration(_) => unreachable!("v3 conventions handled above"),
            Kind::Valid => translator
                .rules(function, &bound)
                .map(|rules| entity.rules.extend(rules)),
            Kind::Computed if matches!(suffix.as_str(), "id" | "password") => {
                Err(vec![shape_error(
                    format!("computed field `{suffix}` is reserved"),
                    function.name_span,
                    "rename the function; id and password belong to the runtime",
                )])
            }
            Kind::Computed
                if entity.fields.iter().any(|field| field.name == *suffix)
                    || entity.computed.iter().any(|field| {
                        field.starts_with(&format!("{{ name: {},", translate::js_string(suffix)))
                    }) =>
            {
                Err(vec![shape_error(
                    format!(
                        "computed field `{suffix}` repeats a field of `{}`",
                        entity.name
                    ),
                    function.name_span,
                    "rename the function",
                )])
            }
            Kind::Computed => translator
                .computed(function, suffix, &bound)
                .map(|computed| {
                    let ty = scalar(&function.return_type, &enums)
                        .map(|ty| ty.js_name().to_owned())
                        .unwrap_or_default();
                    entity.summary.push(format!("{suffix}:{ty}"));
                    entity.computed.push(computed);
                }),
            Kind::Key => translator.body(function, &bound).map(|(body, fields)| {
                entity.summary.push(format!("unique({})", fields.join(",")));
                let fields: Vec<String> = fields.iter().map(|f| translate::js_string(f)).collect();
                entity.keys.push(format!(
                    "{{ name: {}, fields: [{}], value: (r) => {body} }}",
                    translate::js_string(suffix),
                    fields.join(", ")
                ));
            }),
            Kind::Step(field) => returns_bool().and_then(|()| {
                translator.body(function, &bound).map(|(body, _)| {
                    entity.summary.push(format!("workflow({field})"));
                    entity.steps.push(format!(
                        "{{ field: {}, test: (from, to) => {body} }}",
                        translate::js_string(field)
                    ));
                })
            }),
            Kind::CanRead | Kind::CanWrite => returns_bool().and_then(|()| {
                translator.body(function, &bound).map(|(body, fields)| {
                    let create = if *kind == Kind::CanWrite && !fields.is_empty() {
                        format!(", create: {}", translator.creation(function, &bound))
                    } else {
                        String::new()
                    };
                    let test = format!(
                        "{{ row: {}, test: (r, u) => {body}{create} }}",
                        !fields.is_empty()
                    );
                    if *kind == Kind::CanRead {
                        entity.can_read = Some(test);
                    } else {
                        entity.can_write = Some(test);
                    }
                })
            }),
            Kind::Account => returns_bool().and_then(|()| {
                let login = function.params.first().filter(|param| {
                    entity
                        .fields
                        .iter()
                        .any(|field| field.name == param.name && field.ty == Ty::Str)
                });
                let Some(login) = login else {
                    return Err(vec![shape_error(
                        format!("`{}` must take the sign-in field first", function.name),
                        function.name_span,
                        "start with the account's string login field, such as `email: string`",
                    )]);
                };
                login_field = Some((entity.path.clone(), login.name.clone()));
                translator.body(function, &bound).map(|(body, _)| {
                    account = Some(format!(
                        "{{ entity: {}, login: {}, allowed: (r) => {body} }}",
                        translate::js_string(&entity.path),
                        translate::js_string(&login.name)
                    ));
                })
            }),
        };
        if let Err(mut more) = result {
            errors.append(&mut more);
        }
    }

    // Unprefixed `can_read[_<name>]` / `can_write[_<name>]` are defaults for
    // entities without a policy of their own. They see the signed-in account
    // (`me`, `my_<field>`) and may name row fields: such a default applies to
    // every entity that has all of those fields, and the default naming the
    // most row fields wins.
    let mut defaults: Vec<DefaultPolicy> = Vec::new();
    for function in &program.functions {
        let Some(kind) = default_policy_kind(&function.name) else {
            continue;
        };
        if owning_entity(&entities, &function.name).is_some() {
            continue;
        }
        let Some(account_index) = account_entity else {
            errors.push(shape_error(
                format!("`{}` needs an `<entity>_account` function", function.name),
                function.name_span,
                "declare which entity holds the accounts first",
            ));
            continue;
        };
        let bound = match bind_account(function, &entities, account_index, &enums) {
            Ok(bound) => bound,
            Err(mut more) => {
                errors.append(&mut more);
                continue;
            }
        };
        if function.return_type != Type::Bool {
            errors.push(shape_error(
                format!("`{}` must return bool", function.name),
                function.name_span,
                "a permission is a bool",
            ));
            continue;
        }
        let row: Vec<(String, Ty)> = bound
            .iter()
            .filter(|b| b.field)
            .map(|b| (b.name.clone(), b.ty.clone()))
            .collect();
        match translator.body(function, &bound) {
            Ok((body, _)) => defaults.push((kind.clone(), row.clone(), {
                let create = if kind == Kind::CanWrite && !row.is_empty() {
                    format!(", create: {}", translator.creation(function, &bound))
                } else {
                    String::new()
                };
                format!(
                    "{{ row: {}, test: (r, u) => {body}{create} }}",
                    !row.is_empty()
                )
            })),
            Err(mut more) => errors.append(&mut more),
        }
    }
    for entity in &mut entities {
        for kind in [Kind::CanRead, Kind::CanWrite] {
            let applicable = defaults
                .iter()
                .filter(|(k, row, _)| {
                    *k == kind
                        && row.iter().all(|(name, ty)| {
                            entity.fields.iter().any(|f| f.name == *name && f.ty == *ty)
                        })
                })
                .fold(
                    None::<&(Kind, Vec<(String, Ty)>, String)>,
                    |best, candidate| match best {
                        Some(best) if best.1.len() >= candidate.1.len() => Some(best),
                        _ => Some(candidate),
                    },
                );
            let slot = if kind == Kind::CanRead {
                &mut entity.can_read
            } else {
                &mut entity.can_write
            };
            if let (None, Some((_, _, test))) = (slot.as_ref(), applicable) {
                *slot = Some(test.clone());
            }
        }
    }

    // A function that is neither an entity function nor reached from one
    // would be silently ignored, which usually means a misspelled prefix.
    for function in &program.functions {
        if function.name != "main"
            && default_policy_kind(&function.name).is_none()
            && owning_entity(&entities, &function.name).is_none()
            && !translator.reached(&function.name)
        {
            let prefixes: Vec<String> = entities
                .iter()
                .map(|entity| format!("{}_", entity.path))
                .collect();
            errors.push(
                Diagnostic::error(
                    "SPX-WA105",
                    format!(
                        "function `{}` is not an entity function and no entity function calls it",
                        function.name
                    ),
                    function.name_span,
                )
                .with_help(format!(
                    "start its name with an entity prefix ({}) or call it from one",
                    prefixes.join(", ")
                )),
            );
        }
    }
    if !errors.is_empty() {
        return Err(errors);
    }
    Ok(Model {
        helpers: translator.helpers(),
        enums,
        entities,
        account,
        login: login_field,
    })
}

/// A default policy: its kind, the row fields it names, and its test object.
type DefaultPolicy = (Kind, Vec<(String, Ty)>, String);

/// `can_read`, `can_write`, `can_read_<name>`, or `can_write_<name>`.
fn default_policy_kind(name: &str) -> Option<Kind> {
    match name {
        "can_read" => Some(Kind::CanRead),
        "can_write" => Some(Kind::CanWrite),
        _ if name.starts_with("can_read_") => Some(Kind::CanRead),
        _ if name.starts_with("can_write_") => Some(Kind::CanWrite),
        _ => None,
    }
}

/// Bind a default policy's parameters: `me`, `my_<account field>`, or a row
/// field that at least one entity declares with the same type.
fn bind_account(
    function: &Function,
    entities: &[Entity],
    account: usize,
    enums: &BTreeMap<String, Vec<String>>,
) -> Result<Vec<Bound>, Vec<Diagnostic>> {
    let mut bound = Vec::new();
    let mut errors = Vec::new();
    for param in &function.params {
        let ty = scalar(&param.ty, enums).filter(|_| param.mode == ParamMode::Value);
        let binding = ty.as_ref().and_then(|ty| {
            if param.name == "me" && *ty == Ty::Int {
                return Some(("u.id".to_owned(), false));
            }
            if let Some(field) = param.name.strip_prefix("my_") {
                if entities[account]
                    .fields
                    .iter()
                    .any(|f| f.name == field && f.ty == *ty)
                {
                    return Some((format!("u.{field}"), false));
                }
            }
            entities
                .iter()
                .any(|e| e.fields.iter().any(|f| f.name == param.name && f.ty == *ty))
                .then(|| (format!("r.{}", param.name), true))
        });
        match (binding, ty) {
            (Some((js, field)), Some(ty)) => bound.push(Bound {
                name: param.name.clone(),
                js,
                ty,
                field,
            }),
            _ => errors.push(shape_error(
                format!(
                    "parameter `{}` of `{}` is not `me`, `my_<account field>`, or a field some entity declares",
                    param.name, function.name
                ),
                param.span,
                PARAM_HELP,
            )),
        }
    }
    if errors.is_empty() {
        Ok(bound)
    } else {
        Err(errors)
    }
}

fn scalar(ty: &Type, enums: &BTreeMap<String, Vec<String>>) -> Option<Ty> {
    field_type(ty, "", enums, &BTreeMap::new()).map(|(ty, _)| ty)
}

/// Resolve every parameter of one convention function to a binding.
fn bind(
    function: &Function,
    index: usize,
    kind: &Kind,
    entities: &[Entity],
    account: Option<usize>,
    signatures: &[ComputedSignature],
    enums: &BTreeMap<String, Vec<String>>,
) -> Result<(Vec<Bound>, Vec<Rollup>), Vec<Diagnostic>> {
    let entity = &entities[index];
    let mut bound = Vec::new();
    let mut rollups = Vec::new();
    let mut errors = Vec::new();
    for (position, param) in function.params.iter().enumerate() {
        let ty = scalar(&param.ty, enums).filter(|_| param.mode == ParamMode::Value);
        let resolved = ty.and_then(|ty| {
            if let Kind::Step(field) = kind {
                let field_ty = entity.fields.iter().find(|f| f.name == *field)?.ty.clone();
                let js = ["from", "to"].get(position)?;
                return (ty == field_ty && function.params.len() == 2).then(|| {
                    (
                        Bound {
                            name: param.name.clone(),
                            js: (*js).to_owned(),
                            ty,
                            field: false,
                        },
                        None,
                    )
                });
            }
            if entity
                .fields
                .iter()
                .any(|f| f.name == param.name && f.ty == ty)
            {
                let js = format!("r.{}", param.name);
                return Some((
                    Bound {
                        name: param.name.clone(),
                        js,
                        ty,
                        field: true,
                    },
                    None,
                ));
            }
            if matches!(kind, Kind::CanRead | Kind::CanWrite) {
                let account = &entities[account?];
                if param.name == "me" && ty == Ty::Int {
                    return Some((
                        Bound {
                            name: param.name.clone(),
                            js: "u.id".to_owned(),
                            ty,
                            field: false,
                        },
                        None,
                    ));
                }
                let field = param.name.strip_prefix("my_")?;
                return account
                    .fields
                    .iter()
                    .any(|f| f.name == field && f.ty == ty)
                    .then(|| {
                        (
                            Bound {
                                name: param.name.clone(),
                                js: format!("u.{field}"),
                                ty,
                                field: false,
                            },
                            None,
                        )
                    });
            }
            if *kind == Kind::Computed {
                let rollup = rollup(&param.name, &ty, index, entities, signatures)?;
                let js = format!("r.{}", param.name);
                return Some((
                    Bound {
                        name: param.name.clone(),
                        js,
                        ty,
                        field: false,
                    },
                    Some(rollup),
                ));
            }
            None
        });
        match resolved {
            Some((binding, rollup)) => {
                bound.push(binding);
                rollups.extend(rollup);
            }
            None => errors.push(
                Diagnostic::error(
                    "SPX-WA102",
                    format!(
                        "parameter `{}` of `{}` does not bind to `{}`",
                        param.name, function.name, entity.name
                    ),
                    param.span,
                )
                .with_help(param_help(index, kind, entities, account, signatures)),
            ),
        }
    }
    if errors.is_empty() {
        Ok((bound, rollups))
    } else {
        Err(errors)
    }
}

/// The exact parameters a convention function of this kind may take here.
fn param_help(
    index: usize,
    kind: &Kind,
    entities: &[Entity],
    account: Option<usize>,
    signatures: &[ComputedSignature],
) -> String {
    let entity = &entities[index];
    let typed = |fields: &[Field]| -> Vec<String> {
        fields
            .iter()
            .map(|f| match &f.ty {
                Ty::Enum(name) => format!("{}: {name}", f.name),
                Ty::Int => format!("{}: i64", f.name),
                Ty::Float => format!("{}: f64", f.name),
                Ty::Bool => format!("{}: bool", f.name),
                Ty::Str => format!("{}: string", f.name),
                Ty::Char => format!("{}: char", f.name),
            })
            .collect()
    };
    if let Kind::Step(field) = kind {
        let ty = entity
            .fields
            .iter()
            .find(|f| f.name == *field)
            .map(|f| f.ty.clone());
        let name = match ty {
            Some(Ty::Enum(name)) => name,
            _ => "the field's variant".to_owned(),
        };
        return format!(
            "a `_step` function takes exactly two values: `(from: {name}, to: {name})`"
        );
    }
    let mut options = typed(&entity.fields);
    if matches!(kind, Kind::CanRead | Kind::CanWrite) {
        match account {
            Some(account) => {
                options.push("me: i64".to_owned());
                options.extend(
                    typed(&entities[account].fields)
                        .into_iter()
                        .map(|f| format!("my_{f}")),
                );
            }
            None => {
                options.push("(`me`/`my_<field>` need an `<entity>_account` function)".to_owned())
            }
        }
    }
    if *kind == Kind::Computed {
        for (child_index, child) in entities.iter().enumerate() {
            let mut via = child
                .fields
                .iter()
                .filter(|f| f.reference.as_deref() == Some(entity.path.as_str()));
            if via.next().is_none() || via.next().is_some() {
                continue;
            }
            options.push(format!("count_{}: i64", child.path));
            let computed = signatures
                .iter()
                .filter(|s| s.entity == child_index && !s.uses_rollups)
                .filter_map(|s| s.ty.clone().map(|ty| (s.name.clone(), ty)));
            let stored = child
                .fields
                .iter()
                .filter(|f| f.reference.is_none())
                .map(|f| (f.name.clone(), f.ty.clone()));
            for (name, ty) in stored.chain(computed) {
                match ty {
                    Ty::Bool => options.push(format!("count_{}_{name}: i64", child.path)),
                    Ty::Int => options.push(format!("sum_{}_{name}: i64", child.path)),
                    Ty::Float => options.push(format!("sum_{}_{name}: f64", child.path)),
                    _ => {}
                }
            }
        }
    }
    format!("parameters here are exactly one of: {}", options.join(", "))
}

/// `count_<child>`, `count_<child>_<bool>`, or `sum_<child>_<number>` over
/// the rows of `child` whose single reference field names the parent.
fn rollup(
    name: &str,
    ty: &Ty,
    parent: usize,
    entities: &[Entity],
    signatures: &[ComputedSignature],
) -> Option<Rollup> {
    let (kind, rest) = name
        .strip_prefix("count_")
        .map(|rest| ("count", rest))
        .or_else(|| name.strip_prefix("sum_").map(|rest| ("sum", rest)))?;
    let (child_index, child) = entities
        .iter()
        .enumerate()
        .filter(|(_, child)| {
            rest == child.path
                || rest
                    .strip_prefix(child.path.as_str())
                    .is_some_and(|tail| tail.starts_with('_'))
        })
        .max_by_key(|(_, child)| child.path.len())?;
    let field = rest[child.path.len()..].strip_prefix('_');
    let mut via = child
        .fields
        .iter()
        .filter(|f| f.reference.as_deref() == Some(entities[parent].path.as_str()));
    let via = via.next().filter(|_| via.next().is_none())?;
    let field_ty = |field: &str| {
        child
            .fields
            .iter()
            .find(|f| f.name == field)
            .map(|f| f.ty.clone())
            .or_else(|| {
                signatures
                    .iter()
                    .find(|s| s.entity == child_index && s.name == field && !s.uses_rollups)
                    .and_then(|s| s.ty.clone())
            })
    };
    let admitted = match (kind, field) {
        ("count", None) => *ty == Ty::Int,
        ("count", Some(field)) => *ty == Ty::Int && field_ty(field) == Some(Ty::Bool),
        ("sum", Some(field)) => {
            matches!(ty, Ty::Int | Ty::Float) && field_ty(field).as_ref() == Some(ty)
        }
        _ => false,
    };
    admitted.then(|| Rollup {
        name: name.to_owned(),
        kind,
        child: child.path.clone(),
        via: via.name.clone(),
        field: field.map(str::to_owned),
        ty: ty.clone(),
    })
}

/// Each entity whose record declares invariants, with a rule context whose
/// preconditions are those clauses and every entity field bound by name.
fn invariant_rules(program: &Program, entities: &[Entity]) -> Vec<(usize, Function, Vec<Bound>)> {
    let mut contexts = Vec::new();
    for declaration in &program.types {
        if declaration.invariants().is_empty() {
            continue;
        }
        let Some(index) = entities
            .iter()
            .position(|entity| entity.name == declaration.name)
        else {
            continue;
        };
        let bound = entities[index]
            .fields
            .iter()
            .map(|field| Bound {
                name: field.name.clone(),
                js: format!("r.{}", field.name),
                ty: field.ty.clone(),
                field: true,
            })
            .collect();
        let context = Function {
            stable_id: declaration.stable_id.clone(),
            explicit_id: declaration.explicit_id,
            name: declaration.name.clone(),
            name_span: declaration.name_span,
            type_parameters: Vec::new(),
            params: Vec::new(),
            return_type: Type::Bool,
            effects: Vec::new(),
            yields: None,
            follows: None,
            requires: declaration.invariants().to_vec(),
            ensures: Vec::new(),
            body: crate::ast::Expr {
                kind: crate::ast::ExprKind::Bool(true),
                span: declaration.span,
            },
            span: declaration.span,
        };
        contexts.push((index, context, bound));
    }
    contexts
}
