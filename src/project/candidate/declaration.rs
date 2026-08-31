//! Typed declaration construction inside a private candidate AST set.
//!
//! Appending is not admission. The candidate owner must format, reparse and
//! rebuild the whole Project before exposing a revision or evidence.

use std::collections::{BTreeMap, BTreeSet};

use serde_json::Value;

use crate::ast::{Function, Param, ParamMode, Program, Span, Type};
use crate::diagnostic::Diagnostic;
use crate::hir::{DeclarationKind, OwnershipMode, ResolvedType};
use crate::project::ProjectRevision;

use super::intent::{self, IntentSummary};

const MAX_ITEMS: usize = 64;
const MAX_IDENTIFIER_BYTES: usize = 128;
type Result<T> = std::result::Result<T, Vec<Diagnostic>>;

pub(super) struct DeclarationAddition {
    pub(super) id: String,
    pub(super) name: String,
    pub(super) path: String,
    pub(super) module: String,
    pub(super) effects: Vec<String>,
    pub(super) requires_count: usize,
    pub(super) ensures_count: usize,
}

pub(super) fn apply(
    revision: &ProjectRevision,
    programs: &mut [Program],
    request: &Value,
) -> Result<(IntentSummary, DeclarationAddition)> {
    object(request, &["kind", "target", "declaration"])?;
    if text(request, "kind")? != "add_declaration" {
        return Err(grammar("declaration constructor requires add_declaration"));
    }
    let target = text(request, "target")?;
    let (owner, _) = anchor(programs, target)?;
    let declaration = &request["declaration"];
    object(
        declaration,
        &[
            "id",
            "name",
            "parameters",
            "return_type",
            "effects",
            "requires",
            "ensures",
            "body",
        ],
    )?;
    let id = stable_id(text(declaration, "id")?)?.to_owned();
    let name = identifier(text(declaration, "name")?)?.to_owned();
    let mut scope = BTreeSet::new();
    let mut params = Vec::new();
    for parameter in array(declaration, "parameters")? {
        object(parameter, &["name", "type", "mode"])?;
        let name = identifier(text(parameter, "name")?)?;
        if name == "result" || !scope.insert(name.to_owned()) {
            return Err(grammar(
                "declaration parameters must be unique and may not shadow result",
            ));
        }
        let ty = requested_type(revision, &programs[owner], &parameter["type"])?;
        let mode = match text(parameter, "mode")? {
            "value" => ParamMode::Value,
            "own" => ParamMode::Own,
            "borrow" => ParamMode::Borrow,
            _ => return Err(grammar("declaration parameter mode is not supported")),
        };
        validate_parameter(&ty, mode)?;
        params.push(Param {
            name: name.to_owned(),
            mode,
            ty,
            span: Span::default(),
        });
    }
    let return_type = requested_type(revision, &programs[owner], &declaration["return_type"])?;
    validate_return(&return_type)?;
    let effects = array(declaration, "effects")?
        .iter()
        .map(|value| {
            let effect = value
                .as_str()
                .ok_or_else(|| grammar("declaration effects must be text"))?;
            if effect.is_empty() || effect.len() > MAX_IDENTIFIER_BYTES {
                return Err(grammar(
                    "declaration effect must be a bounded admitted name",
                ));
            }
            Ok(effect.to_owned())
        })
        .collect::<Result<Vec<_>>>()?;
    let program = &programs[owner];
    let requires = array(declaration, "requires")?
        .iter()
        .map(|predicate| {
            let nominal_scope =
                intent::parameter_nominal_scope(revision, program, &params, predicate)?;
            intent::construct_expression_with_scope(
                revision,
                program,
                &scope,
                nominal_scope,
                predicate,
            )
        })
        .collect::<Result<Vec<_>>>()?;
    let body_scope =
        intent::parameter_nominal_scope(revision, program, &params, &declaration["body"])?;
    let body = intent::construct_expression_with_scope(
        revision,
        program,
        &scope,
        body_scope,
        &declaration["body"],
    )?;
    scope.insert("result".to_owned());
    let ensures = array(declaration, "ensures")?
        .iter()
        .map(|predicate| {
            let mut nominal_scope =
                intent::parameter_nominal_scope(revision, program, &params, predicate)?;
            if intent::uses_field_places(predicate) {
                intent::insert_nominal_type(
                    revision,
                    program,
                    &mut nominal_scope,
                    "result",
                    &return_type,
                )?;
            }
            intent::construct_expression_with_scope(
                revision,
                program,
                &scope,
                nominal_scope,
                predicate,
            )
        })
        .collect::<Result<Vec<_>>>()?;
    let addition = append_function(
        revision,
        programs,
        target,
        Function {
            stable_id: id,
            explicit_id: true,
            name,
            name_span: Span::default(),
            type_parameters: Vec::new(),
            params,
            return_type,
            effects,
            requires,
            ensures,
            body,
            span: Span::default(),
        },
    )?;
    Ok((
        IntentSummary {
            target_id: target.to_owned(),
            kind: "add_declaration".to_owned(),
            migrated_calls: 0,
        },
        addition,
    ))
}

/// Shared with compiler-derived extraction. Callers own the disposable ASTs;
/// this helper adds one function only after structural checks, not verification.
pub(super) fn append_function(
    revision: &ProjectRevision,
    programs: &mut [Program],
    target: &str,
    function: Function,
) -> Result<DeclarationAddition> {
    let (owner, anchor_index) = anchor(programs, target)?;
    stable_id(&function.stable_id)?;
    identifier(&function.name)?;
    if !function.explicit_id || !function.type_parameters.is_empty() || function.name == "main" {
        return Err(grammar(
            "added declaration must be an explicit monomorphic non-main function",
        ));
    }
    if function.params.len() > MAX_ITEMS
        || function.effects.len() > MAX_ITEMS
        || function.requires.len() > MAX_ITEMS
        || function.ensures.len() > MAX_ITEMS
    {
        return Err(capacity(
            "declaration lists permit at most sixty-four items each",
        ));
    }
    let mut names = BTreeSet::new();
    let program = &programs[owner];
    let mut checked_owners = BTreeSet::new();
    for parameter in &function.params {
        identifier(&parameter.name)?;
        if parameter.name == "result" || !names.insert(&parameter.name) {
            return Err(grammar(
                "declaration parameters must be unique and may not shadow result",
            ));
        }
        validate_parameter(&parameter.ty, parameter.mode)?;
        if parameter.mode == ParamMode::Own && matches!(parameter.ty, Type::Named { .. }) {
            validate_owned_nominal(revision, program, &parameter.ty, &mut checked_owners)?;
        }
    }
    validate_return(&function.return_type)?;
    for ty in function
        .params
        .iter()
        .map(|parameter| &parameter.ty)
        .chain(std::iter::once(&function.return_type))
    {
        intent::validate_nominal_ast(revision, program, ty)?;
    }
    if function.effects.windows(2).any(|pair| pair[0] >= pair[1])
        || function.effects.iter().any(|effect| {
            effect.is_empty()
                || effect.len() > MAX_IDENTIFIER_BYTES
                || !program.permits.contains(effect)
                || !program.functions[anchor_index].effects.contains(effect)
        })
    {
        return Err(grammar(
            "added declaration effects must be sorted unique and within anchor and module budgets",
        ));
    }
    // The retained graph covers nested declarations as well as top-level
    // functions. A type, field, case, import, or method ID is not available for
    // reuse merely because this constructor creates only top-level functions.
    let graph: Value = serde_json::from_str(revision.semantic_graph())
        .map_err(|_| grammar("retained declaration graph is invalid"))?;
    let declarations = graph["declarations"]
        .as_array()
        .ok_or_else(|| grammar("retained declaration graph lacks its identity inventory"))?;
    if declarations
        .iter()
        .any(|entry| entry["id"].as_str() == Some(function.stable_id.as_str()))
        || programs.iter().any(|program| {
            program
                .functions
                .iter()
                .any(|f| f.stable_id == function.stable_id)
                || program
                    .module_uses
                    .iter()
                    .any(|binding| binding.persistent_id == function.stable_id)
        })
    {
        return Err(grammar(
            "added declaration ID is already bound in this Project",
        ));
    }
    if program
        .functions
        .iter()
        .any(|entry| entry.name == function.name)
        || program
            .types
            .iter()
            .any(|entry| entry.name == function.name)
        || program
            .interfaces
            .iter()
            .any(|entry| entry.name == function.name)
        || program
            .protocols
            .iter()
            .any(|entry| entry.name == function.name)
        || program
            .module_uses
            .iter()
            .any(|entry| entry.alias == function.name)
    {
        return Err(grammar(
            "added declaration name conflicts with a module binding",
        ));
    }
    let addition = DeclarationAddition {
        id: function.stable_id.clone(),
        name: function.name.clone(),
        path: program.path.clone(),
        module: program.module.clone(),
        effects: function.effects.clone(),
        requires_count: function.requires.len(),
        ensures_count: function.ensures.len(),
    };
    programs[owner].functions.push(function);
    Ok(addition)
}

pub(super) fn anchor(programs: &[Program], target: &str) -> Result<(usize, usize)> {
    if target.is_empty() || target.len() > intent::MAX_ID_BYTES {
        return Err(grammar("declaration anchor must be a bounded stable ID"));
    }
    let mut found = None;
    for (owner, program) in programs.iter().enumerate() {
        for (index, function) in program.functions.iter().enumerate() {
            if function.stable_id == target
                && (!function.explicit_id
                    || !function.type_parameters.is_empty()
                    || found.replace((owner, index)).is_some())
            {
                return Err(grammar(
                    "declaration anchor must be one explicit monomorphic top-level function",
                ));
            }
        }
    }
    found.ok_or_else(|| grammar("declaration anchor is absent from this Project"))
}

fn type_name(name: &str) -> Result<Type> {
    match name {
        "i64" => Ok(Type::I64),
        "i32" => Ok(Type::I32),
        "u8" => Ok(Type::U8),
        "usize" => Ok(Type::Usize),
        "bool" => Ok(Type::Bool),
        "string" => Ok(Type::String),
        "Bytes" => Ok(Type::Bytes),
        "str" => Ok(Type::Str),
        "Slice<u8>" => Ok(Type::SliceU8),
        _ => Err(grammar(
            "declaration type is outside the admitted constructor vocabulary",
        )),
    }
}

fn requested_type(revision: &ProjectRevision, program: &Program, value: &Value) -> Result<Type> {
    if let Some(name) = value.as_str() {
        return type_name(name);
    }
    object(value, &["kind", "target", "type_arguments"])?;
    if text(value, "kind")? != "nominal" {
        return Err(grammar("declaration type object requires nominal kind"));
    }
    intent::nominal_type_plan(
        revision,
        program,
        text(value, "target")?,
        &value["type_arguments"],
    )
}

/// Every addition, including compiler-derived extraction, passes this gate
/// after full source rebuild. Nominal syntax alone proves neither Copy nor
/// owning admission; canonical source modes must match the checked signature.
pub(super) fn validate_added_signature(
    revision: &ProjectRevision,
    addition: &DeclarationAddition,
) -> Result<()> {
    let mut selected = None;
    for module in revision.semantic.image_modules() {
        for function in module
            .functions()
            .iter()
            .filter(|function| function.id.as_str() == addition.id)
        {
            if selected.replace((module, function)).is_some() {
                return Err(grammar("added checked function identity is ambiguous"));
            }
        }
    }
    let (module, function) = selected.ok_or_else(|| grammar("added checked function is absent"))?;
    if module.path() != addition.path
        || module.module() != addition.module
        || function.name != addition.name
    {
        return Err(grammar(
            "added checked function disagrees with its source owner",
        ));
    }
    let source = revision
        .sources()
        .iter()
        .find(|source| source.path() == module.path())
        .ok_or_else(|| grammar("added function canonical source is absent"))?;
    let program = crate::parse(source.source(), source.path()).map_err(|error| vec![error])?;
    let authored = program
        .functions
        .iter()
        .find(|source| source.stable_id == addition.id)
        .ok_or_else(|| grammar("added function canonical declaration is absent"))?;
    if authored.name != function.name
        || authored.span != function.span
        || authored.params.len() != function.params.len()
    {
        return Err(grammar("added source and checked signatures disagree"));
    }
    for (source, parameter) in authored.params.iter().zip(&function.params) {
        validate_parameter(&source.ty, source.mode)?;
        let expected = if source.mode == ParamMode::Value && source.ty == Type::String {
            OwnershipMode::Own
        } else {
            match source.mode {
                ParamMode::Value => OwnershipMode::Value,
                ParamMode::Own => OwnershipMode::Own,
                ParamMode::Borrow => OwnershipMode::Borrow,
                ParamMode::Shared => {
                    return Err(grammar(
                        "added signature cannot introduce shared parameters",
                    ))
                }
            }
        };
        if source.name != parameter.name
            || source.span != parameter.span
            || parameter.ownership != expected
        {
            return Err(grammar(
                "added parameter source mode disagrees with checked ownership",
            ));
        }
        checked_signature_type(revision, module, &parameter.ty, Some(expected))?;
    }
    validate_return(&authored.return_type)?;
    checked_signature_type(revision, module, &function.return_type, None)
}

fn checked_signature_type(
    revision: &ProjectRevision,
    module: &crate::workspace_graph::WorkspaceGraphProjectionModule,
    ty: &ResolvedType,
    ownership: Option<OwnershipMode>,
) -> Result<()> {
    if matches!(ty, ResolvedType::Nominal { .. }) {
        let (kind, facts) = module
            .signature_type_facts(ty)
            .ok_or_else(|| grammar("added nominal signature has no retained checked type facts"))?;
        let copy = facts.copy && !facts.needs_drop;
        let owned = !facts.copy && facts.needs_drop;
        let admitted = match ownership {
            Some(OwnershipMode::Value) => copy,
            Some(OwnershipMode::Own) => owned,
            Some(OwnershipMode::Borrow | OwnershipMode::Shared) => false,
            None => copy || owned,
        };
        if !matches!(kind, DeclarationKind::Record | DeclarationKind::Variant)
            || !facts.sized
            || facts.contains_resource
            || !admitted
        {
            return Err(grammar("added nominal signature requires checked sized resource-free Copy values or explicit non-Copy owning parameters"));
        }
    } else if *ty == ResolvedType::String {
        let facts = revision
            .entry_program()
            .declarations
            .type_facts(ty)
            .ok_or_else(|| grammar("added String signature has no compiler TypeFacts"))?;
        if ownership.is_some_and(|mode| mode != OwnershipMode::Own)
            || facts.copy
            || !facts.needs_drop
            || !facts.sized
            || facts.contains_resource
        {
            return Err(grammar(
                "added String signature requires checked resource-free ownership",
            ));
        }
    }
    Ok(())
}

fn validate_owned_nominal(
    revision: &ProjectRevision,
    program: &Program,
    ty: &Type,
    checked: &mut BTreeSet<String>,
) -> Result<()> {
    intent::validate_nominal_ast(revision, program, ty)?;
    let Type::Named { name, arguments } = ty else {
        unreachable!("caller selected nominal parameter")
    };
    if !arguments.is_empty() {
        return Err(grammar(
            "added owning nominal parameters require monomorphic source types",
        ));
    }
    let id = program
        .types
        .iter()
        .find(|declaration| declaration.name == *name)
        .map(|declaration| declaration.stable_id.as_str())
        .or_else(|| {
            program
                .module_uses
                .iter()
                .find(|binding| {
                    binding.kind == crate::ast::ModuleUseKind::Type && binding.alias == *name
                })
                .map(|binding| binding.persistent_id.as_str())
        })
        .ok_or_else(|| grammar("added owning nominal parameter has no source owner binding"))?;
    if checked.contains(id) {
        return Ok(());
    }
    let declarations = revision
        .semantic
        .image_modules()
        .iter()
        .flat_map(|module| module.types())
        .map(|declaration| (declaration.id.as_str(), declaration))
        .collect::<BTreeMap<_, _>>();
    let declaration = declarations
        .get(id)
        .ok_or_else(|| grammar("added owning nominal source declaration is absent"))?;
    if !declaration.type_parameters.is_empty() {
        return Err(grammar(
            "added owning nominal parameters require monomorphic declarations",
        ));
    }
    // Preflight preserves the established G225 for `own` Copy parameters,
    // before source verification would reject them as O002. The final gate
    // still checks the freshly retained signature facts after complete replay.
    let facts =
        crate::hir::DeclarationIndex::record_evolution_type_facts(&declaration.id, &declarations)
            .map_err(|diagnostic| vec![diagnostic])?
            .ok_or_else(|| {
                grammar("added owning nominal parameter lacks bounded checked TypeFacts")
            })?;
    if facts.copy || !facts.needs_drop || !facts.sized || facts.contains_resource {
        return Err(grammar(
            "added owning nominal parameter requires a checked non-Copy resource-free owner",
        ));
    }
    checked.insert(id.to_owned());
    Ok(())
}

fn scalar(ty: &Type) -> bool {
    matches!(
        ty,
        Type::I64 | Type::I32 | Type::U8 | Type::Usize | Type::Bool
    )
}

fn validate_parameter(ty: &Type, mode: ParamMode) -> Result<()> {
    if ((scalar(ty) || matches!(ty, Type::String | Type::Named { .. })) && mode == ParamMode::Value)
        || (matches!(ty, Type::Bytes | Type::Named { .. }) && mode == ParamMode::Own)
        || (matches!(ty, Type::Str | Type::SliceU8) && mode == ParamMode::Borrow)
    {
        Ok(())
    } else {
        Err(grammar(
            "declaration parameter type and ownership mode are not supported together",
        ))
    }
}

fn validate_return(ty: &Type) -> Result<()> {
    if scalar(ty) || matches!(ty, Type::String | Type::Bytes | Type::Named { .. }) {
        Ok(())
    } else {
        Err(grammar(
            "declaration return type must be an admitted scalar, String, Bytes or checked resource-free nominal type",
        ))
    }
}

pub(super) fn stable_id(id: &str) -> Result<&str> {
    if id.is_empty()
        || id.len() > MAX_IDENTIFIER_BYTES
        || !id.bytes().all(|b| {
            b.is_ascii_lowercase() || b.is_ascii_digit() || matches!(b, b'.' | b'_' | b'-')
        })
    {
        return Err(grammar(
            "new declaration ID must be one to 128 lowercase ASCII ID characters",
        ));
    }
    Ok(id)
}

pub(super) fn identifier(name: &str) -> Result<&str> {
    if name.is_empty()
        || name.len() > MAX_IDENTIFIER_BYTES
        || !name
            .bytes()
            .enumerate()
            .all(|(i, b)| b == b'_' || b.is_ascii_alphabetic() || (i != 0 && b.is_ascii_digit()))
        || matches!(
            name,
            "module"
                | "use"
                | "fn"
                | "let"
                | "mut"
                | "if"
                | "else"
                | "while"
                | "match"
                | "true"
                | "false"
                | "requires"
                | "ensures"
                | "uses"
                | "permit"
                | "unsafe"
                | "return"
                | "own"
                | "borrow"
                | "shared"
                | "self"
                | "super"
        )
    {
        return Err(grammar(
            "declaration name must be a bounded ordinary identifier",
        ));
    }
    Ok(name)
}

fn text<'a>(value: &'a Value, key: &str) -> Result<&'a str> {
    value
        .get(key)
        .and_then(Value::as_str)
        .ok_or_else(|| grammar("declaration field must be text"))
}

fn array<'a>(value: &'a Value, key: &str) -> Result<&'a [Value]> {
    let items = value
        .get(key)
        .and_then(Value::as_array)
        .ok_or_else(|| grammar("declaration field must be an array"))?;
    if items.len() > MAX_ITEMS {
        return Err(capacity(
            "declaration lists permit at most sixty-four items each",
        ));
    }
    Ok(items)
}

fn object(value: &Value, keys: &[&str]) -> Result<()> {
    let object = value
        .as_object()
        .ok_or_else(|| grammar("declaration must be an object"))?;
    if object.len() != keys.len() || keys.iter().any(|key| !object.contains_key(*key)) {
        return Err(grammar("declaration contains missing or unknown fields"));
    }
    Ok(())
}

fn grammar(message: &'static str) -> Vec<Diagnostic> {
    vec![Diagnostic::io("SPX-G225", message)]
}

fn capacity(message: &'static str) -> Vec<Diagnostic> {
    vec![Diagnostic::io("SPX-G226", message)]
}
