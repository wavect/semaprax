//! Typed declaration construction inside a private candidate AST set.
//!
//! Appending is not admission. The candidate owner must format, reparse and
//! rebuild the whole Project before exposing a revision or evidence.

use std::collections::BTreeSet;

use serde_json::Value;

use crate::ast::{Function, Param, ParamMode, Program, Span, Type};
use crate::diagnostic::Diagnostic;
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
        let ty = type_name(text(parameter, "type")?)?;
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
    let return_type = type_name(text(declaration, "return_type")?)?;
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
        .map(|predicate| intent::construct_expression(program, &scope, predicate))
        .collect::<Result<Vec<_>>>()?;
    let body = intent::construct_expression(program, &scope, &declaration["body"])?;
    scope.insert("result".to_owned());
    let ensures = array(declaration, "ensures")?
        .iter()
        .map(|predicate| intent::construct_expression(program, &scope, predicate))
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
    for parameter in &function.params {
        identifier(&parameter.name)?;
        if parameter.name == "result" || !names.insert(&parameter.name) {
            return Err(grammar(
                "declaration parameters must be unique and may not shadow result",
            ));
        }
        validate_parameter(&parameter.ty, parameter.mode)?;
    }
    validate_return(&function.return_type)?;
    let program = &programs[owner];
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

fn anchor(programs: &[Program], target: &str) -> Result<(usize, usize)> {
    if target.is_empty() || target.len() > intent::MAX_ID_BYTES {
        return Err(grammar("declaration anchor must be a bounded stable ID"));
    }
    let mut found = None;
    for (owner, program) in programs.iter().enumerate() {
        for (index, function) in program.functions.iter().enumerate() {
            if function.stable_id == target {
                if !function.explicit_id
                    || !function.type_parameters.is_empty()
                    || found.replace((owner, index)).is_some()
                {
                    return Err(grammar(
                        "declaration anchor must be one explicit monomorphic top-level function",
                    ));
                }
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
        "Bytes" => Ok(Type::Bytes),
        "str" => Ok(Type::Str),
        "Slice<u8>" => Ok(Type::SliceU8),
        _ => Err(grammar(
            "declaration type is outside the admitted constructor vocabulary",
        )),
    }
}

fn scalar(ty: &Type) -> bool {
    matches!(
        ty,
        Type::I64 | Type::I32 | Type::U8 | Type::Usize | Type::Bool
    )
}

fn validate_parameter(ty: &Type, mode: ParamMode) -> Result<()> {
    if (scalar(ty) && mode == ParamMode::Value)
        || (*ty == Type::Bytes && mode == ParamMode::Own)
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
    if scalar(ty) || *ty == Type::Bytes {
        Ok(())
    } else {
        Err(grammar(
            "declaration return type must be an admitted scalar or owned Bytes",
        ))
    }
}

fn stable_id(id: &str) -> Result<&str> {
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

fn identifier(name: &str) -> Result<&str> {
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
