//! Ordered checked Copy/resource-free-owner evolution with left-to-right staging.
//!
//! Every old argument is evaluated once even when its parameter is removed.
//! Parameter identity is selected by its old name; display renaming follows
//! lexical binding scopes. No implicit conversion is performed. Project replay is
//! caller's responsibility.

use super::{
    array, call_bindings, capacity, grammar, identifier, literal, member, object, scalar_type,
    text, walk_program, Result, MAX_WALK_DEPTH, MAX_WALK_NODES,
};
use crate::ast::{
    Expr, ExprKind, Function, MatchPattern, ModuleUseKind, Param, ParamMode, Program,
    RecordMatchFieldPattern, Span, Statement, Type, TypeDeclarationKind,
};
use crate::hir::{DeclarationKind, OwnershipMode, ResolvedType};
use crate::project::ProjectRevision;
use serde_json::{json, Value};
use std::collections::{BTreeMap, BTreeSet};

#[path = "signature_arguments.rs"]
mod computed;
pub(in crate::project::candidate) use computed::validate_computed_signature;
#[path = "signature_rename.rs"]
mod rename;

// This bounds internal declarations independently of narrower export-profile
// limits. Public ABI parameter ceilings are rechecked by ordinary Project
// admission; this route never raises them.
const MAX_PARAMETERS: usize = 4096;

#[derive(Clone)]
enum Argument {
    Existing(usize),
    Literal(Expr),
    Computed {
        template: Value,
        type_request: Value,
    },
}

pub(super) fn apply(
    revision: Option<&ProjectRevision>,
    programs: &mut [Program],
    intent: &Value,
    owner: usize,
    function_index: usize,
) -> Result<usize> {
    object(intent, &["kind", "target", "parameters"])?;
    let target = text(intent, "target")?;
    let function = &programs[owner].functions[function_index];
    let owner_module = programs[owner].module.clone();
    let original_params = function.params.clone();
    // Template variables name provider parameters, even while lowering the
    // template against an importing caller's available declaration bindings.
    let original_nominal_scope = match revision {
        Some(revision) => {
            super::parameter_nominal_scope(revision, &programs[owner], &original_params, intent)?
        }
        None => BTreeMap::new(),
    };
    if original_params.len() > MAX_PARAMETERS {
        return Err(capacity(
            "signature evolution exceeds its original parameter limit",
        ));
    }
    let legacy = original_params.iter().all(legacy_parameter);
    if !legacy
        && revision
            .map(|revision| ordered_signature_parameters(revision, function))
            .transpose()?
            .flatten()
            .is_none()
    {
        return Err(grammar(
            "ordered signature evolution requires checked Copy values or resource-free owned data",
        ));
    }
    let original = original_params
        .iter()
        .enumerate()
        .map(|(index, param)| (param.name.as_str(), index))
        .collect::<BTreeMap<_, _>>();
    if original.len() != original_params.len() {
        return Err(grammar(
            "signature evolution has ambiguous original parameter names",
        ));
    }
    let requested = array(intent, "parameters")?;
    if requested.len() > MAX_PARAMETERS {
        return Err(capacity(
            "signature evolution exceeds its mapped parameter limit",
        ));
    }
    let mut selected = BTreeSet::new();
    let mut names = BTreeSet::new();
    let mut params = Vec::with_capacity(requested.len());
    let mut arguments = Vec::with_capacity(requested.len());
    let mut template_nodes = 0usize;
    for mapping in requested {
        if mapping.get("from").is_some() {
            if mapping.get("name").is_some() {
                object(mapping, &["from", "name"])?;
            } else {
                object(mapping, &["from"])?;
            }
            let name = text(mapping, "from")?;
            let index = *original
                .get(name)
                .ok_or_else(|| grammar("signature mapping names an unknown original parameter"))?;
            let destination = if mapping.get("name").is_some() {
                let name = identifier(text(mapping, "name")?)?;
                if name == "result" {
                    return Err(rename::invalid(
                        "parameter rename cannot capture the contract result binding",
                    ));
                }
                name
            } else {
                name
            };
            if !selected.insert(index) || !names.insert(destination.to_owned()) {
                return Err(grammar(
                    "signature mapping duplicates an original parameter",
                ));
            }
            let mut param = original_params[index].clone();
            param.name = destination.to_owned();
            params.push(param);
            arguments.push(Argument::Existing(index));
        } else {
            let is_computed = mapping.get("argument_expression").is_some();
            if is_computed {
                object(mapping, &["name", "type", "argument_expression"])?;
            } else {
                object(mapping, &["name", "type", "argument"])?;
            }
            let name = identifier(text(mapping, "name")?)?;
            if original.contains_key(name) || !names.insert(name.to_owned()) {
                return Err(grammar(
                    "new signature parameter must not rename or reinterpret an old binding",
                ));
            }
            let type_request = member(mapping, "type")?;
            let ty = if is_computed {
                computed::requested_type(revision, &programs[owner], type_request)?
            } else {
                scalar_type(text(mapping, "type")?)?
            };
            if is_computed {
                computed::charge(&mut template_nodes, computed::nominal_type_nodes(&ty))?;
                let template = member(mapping, "argument_expression")?;
                // Preflight even when no source call instantiates this default.
                // Only instantiated callers receive ordinary semantic checks.
                computed::prepare(
                    revision,
                    &programs[owner],
                    &original_params,
                    &original_nominal_scope,
                    template,
                    &mut BTreeSet::new(),
                    &mut template_nodes,
                )?;
                arguments.push(Argument::Computed {
                    template: template.clone(),
                    type_request: type_request.clone(),
                });
            } else {
                let argument = member(mapping, "argument")?;
                if text(argument, "kind")? != text(mapping, "type")? {
                    return Err(grammar(
                        "new signature argument must be an exact typed scalar literal",
                    ));
                }
                arguments.push(Argument::Literal(literal(argument)?));
            }
            params.push(Param {
                name: name.to_owned(),
                mode: ParamMode::Value,
                ty,
                span: Span::default(),
            });
        }
    }
    if original_params
        .iter()
        .enumerate()
        .any(|(index, param)| owning_parameter(param) && !selected.contains(&index))
    {
        return Err(vec![crate::diagnostic::Diagnostic::io(
            "SPX-G260",
            "signature evolution must retain every owning parameter exactly once",
        )]);
    }
    let old_arity = original_params.len();
    let mut occupied = names.clone();
    // Reserve lexical names across the complete candidate, including unrelated
    // and shadowed binders. This conservative whole-program set avoids capture
    // before any scope-aware substitution or staging introduces new names.
    reserve_names(programs, &mut occupied)?;
    let renames = arguments
        .iter()
        .zip(&params)
        .filter_map(|(argument, param)| match argument {
            Argument::Existing(index) => {
                Some((original_params[*index].name.clone(), param.name.clone()))
            }
            Argument::Literal(_) | Argument::Computed { .. } => None,
        })
        .collect::<BTreeMap<_, _>>();
    if renames.iter().any(|(old, new)| old != new) {
        rename::apply(
            &mut programs[owner].functions[function_index],
            &original_params,
            &renames,
            &names,
            &mut occupied,
        )?;
    }
    let mut generated = 0usize;
    let mut nodes = 0usize;
    let mut added_nodes = 0usize;
    let mut migrated_calls = 0usize;
    let has_computed = arguments
        .iter()
        .any(|argument| matches!(argument, Argument::Computed { .. }));
    for program in programs.iter_mut() {
        for import in &program.module_uses {
            if import.kind == ModuleUseKind::Function
                && import.persistent_id == target
                && import.target_module != owner_module
            {
                return Err(grammar(
                    "signature caller import disagrees with its provider module",
                ));
            }
        }
        let bindings = call_bindings(program)?;
        // Constructor binding lookup must see the immutable caller context,
        // not partially rewritten argument or parameter subtrees.
        let caller_context = has_computed.then(|| program.clone());
        walk_program(program, &mut nodes, &mut |expression| {
            let ExprKind::Call {
                name,
                type_arguments,
                args,
            } = &expression.kind
            else {
                return Ok(());
            };
            if !bindings.get(name).is_some_and(|id| id == target) {
                return Ok(());
            }
            if !type_arguments.is_empty() || args.len() != old_arity {
                return Err(grammar(
                    "ordered signature caller does not match the original monomorphic arity",
                ));
            }
            // Original argument expressions move into let initializers. New
            // expression nodes are the block and the mapped Vars/literals;
            // the existing Call becomes the block's tail Call.
            let argument_nodes = arguments
                .iter()
                .map(|argument| match argument {
                    Argument::Literal(expression) => super::literal_nodes(expression),
                    Argument::Existing(_) | Argument::Computed { .. } => 1,
                })
                .sum::<usize>();
            added_nodes = added_nodes
                .checked_add(1 + argument_nodes)
                .ok_or_else(|| capacity("signature migration node count overflow"))?;
            if added_nodes > MAX_WALK_NODES {
                return Err(capacity(
                    "signature migration generated nodes exceed the limit",
                ));
            }
            let call_name = name.clone();
            let span = expression.span;
            let mut templates = Vec::with_capacity(arguments.len());
            for argument in &arguments {
                templates.push(
                    if let Argument::Computed {
                        template,
                        type_request,
                    } = argument
                    {
                        let caller = caller_context.as_ref().expect("computed context retained");
                        // A provider's display spelling is not an imported type
                        // binding. Resolve the same stable selector in this caller.
                        let ty = computed::requested_type(revision, caller, type_request)?;
                        computed::charge(&mut template_nodes, computed::nominal_type_nodes(&ty))?;
                        let (body, count) = computed::prepare(
                            revision,
                            caller,
                            &original_params,
                            &original_nominal_scope,
                            template,
                            &mut occupied,
                            &mut template_nodes,
                        )?;
                        Some((body, count, ty))
                    } else {
                        None
                    },
                );
            }
            if has_computed {
                computed::charge(&mut added_nodes, old_arity)?;
            }
            let mut stages = Vec::with_capacity(old_arity);
            for _ in 0..old_arity {
                stages.push(fresh_name(&mut occupied, &mut generated)?);
            }
            let mut defaults = Vec::new();
            let mut mapped = Vec::with_capacity(arguments.len());
            for (argument, template) in arguments.iter().zip(templates) {
                mapped.push(match argument {
                    Argument::Existing(index) => Expr {
                        kind: ExprKind::Var(stages[*index].clone()),
                        span,
                    },
                    Argument::Literal(expression) => expression.clone(),
                    Argument::Computed { .. } => {
                        let (body, body_nodes, ty) = template.expect("computed template prepared");
                        computed::charge(&mut added_nodes, computed::nominal_type_nodes(&ty))?;
                        computed::charge(&mut added_nodes, body_nodes + 1)?;
                        let body =
                            computed::substitute(body, &original_params, &stages, &mut occupied)?;
                        let name = fresh_name(&mut occupied, &mut generated)?;
                        defaults.push(Statement::Let {
                            name: name.clone(),
                            name_span: span,
                            mutable: false,
                            declared: Some(ty),
                            value: body,
                            span,
                        });
                        Expr {
                            kind: ExprKind::Var(name),
                            span,
                        }
                    }
                });
            }
            // Move, never duplicate or omit, every original argument subtree.
            let ExprKind::Call { args, .. } =
                std::mem::replace(&mut expression.kind, ExprKind::Int(0))
            else {
                unreachable!("call inspected immediately before replacement");
            };
            let mut statements = args
                .into_iter()
                .zip(stages)
                .map(|(value, name)| Statement::Let {
                    name,
                    name_span: span,
                    mutable: false,
                    declared: None,
                    value,
                    span,
                })
                .collect::<Vec<_>>();
            statements.extend(defaults);
            expression.kind = ExprKind::Block {
                statements,
                tail: Box::new(Expr {
                    kind: ExprKind::Call {
                        name: call_name,
                        type_arguments: Vec::new(),
                        args: mapped,
                    },
                    span,
                }),
            };
            migrated_calls += 1;
            Ok(())
        })?;
        if nodes
            .checked_add(added_nodes)
            .is_none_or(|total| total > MAX_WALK_NODES)
        {
            return Err(capacity(
                "signature migration complete expression inventory exceeds the limit",
            ));
        }
    }
    programs[owner].functions[function_index].params = params;
    Ok(migrated_calls)
}

fn copy_type(ty: &Type) -> bool {
    matches!(
        ty,
        Type::I64
            | Type::I32
            | Type::Char
            | Type::U8
            | Type::Usize
            | Type::ArrayU8(_)
            | Type::F32
            | Type::F64
            | Type::Bool
    )
}

fn legacy_parameter(parameter: &Param) -> bool {
    (parameter.mode == ParamMode::Value && copy_type(&parameter.ty))
        || (parameter.mode == ParamMode::Own && parameter.ty == Type::Bytes)
}

fn owning_parameter(parameter: &Param) -> bool {
    parameter.mode == ParamMode::Own
        || (parameter.mode == ParamMode::Value && parameter.ty == Type::String)
}

/// One authority shared by candidate admission and catalogue discovery. Legacy
/// parameters have no additive metadata. Nominal facts come from the original
/// module's validated index, including internal functions outside linked roots.
pub(in crate::project::candidate) fn ordered_signature_parameters(
    revision: &ProjectRevision,
    function: &Function,
) -> Result<Option<Vec<Option<Value>>>> {
    if !function.explicit_id
        || function.name == "main"
        || !function.type_parameters.is_empty()
        || function.params.len() > MAX_PARAMETERS
    {
        return Ok(None);
    }
    if function.params.iter().all(legacy_parameter) {
        return Ok(Some(vec![None; function.params.len()]));
    }
    // The source language spells an owned String parameter as bare `string`;
    // its checked HIR mode is Own. No new ownership mode is invented here.
    // All newly admitted subjects below still require exact checked facts.
    if function.params.iter().any(|parameter| {
        !legacy_parameter(parameter)
            && !matches!(
                (&parameter.mode, &parameter.ty),
                (ParamMode::Value, Type::String | Type::Named { .. })
                    | (ParamMode::Own, Type::Named { .. })
            )
    }) {
        return Ok(None);
    }
    let mut subject = None;
    for module in revision.semantic.image_modules() {
        for checked in module
            .functions()
            .iter()
            .filter(|f| f.id.as_str() == function.stable_id)
        {
            if subject.replace((module, checked)).is_some() {
                return Err(grammar(
                    "ordered signature checked function identity is ambiguous",
                ));
            }
        }
    }
    let Some((module, checked)) = subject else {
        return Ok(None);
    };
    if checked.name != function.name
        || checked.span != function.span
        || checked.params.len() != function.params.len()
    {
        return Err(grammar(
            "ordered signature source and checked function disagree",
        ));
    }
    // Authenticate the complete AST subject, including every source type alias,
    // against retained source before using its checked counterpart's facts.
    let source = revision
        .sources()
        .iter()
        .find(|s| s.path() == module.path())
        .ok_or_else(|| grammar("ordered signature checked source is absent"))?;
    let program = crate::parse(source.source(), source.path()).map_err(|error| vec![error])?;
    if !program
        .functions
        .iter()
        .any(|original| original == function)
    {
        return Err(grammar(
            "ordered signature AST differs from authenticated source",
        ));
    }
    let mut metadata = Vec::with_capacity(function.params.len());
    for (parameter, resolved) in function.params.iter().zip(&checked.params) {
        let source_ownership = if parameter.mode == ParamMode::Value && parameter.ty == Type::String
        {
            OwnershipMode::Own
        } else {
            OwnershipMode::from(parameter.mode)
        };
        if parameter.name != resolved.name
            || parameter.span != resolved.span
            || source_ownership != resolved.ownership
        {
            return Err(grammar(
                "ordered signature source parameter and checked binding disagree",
            ));
        }
        if legacy_parameter(parameter) {
            metadata.push(None);
            continue;
        }
        if parameter.mode == ParamMode::Value && parameter.ty == Type::String {
            if resolved.ty != ResolvedType::String || resolved.ownership != OwnershipMode::Own {
                return Err(grammar(
                    "ordered String parameter disagrees with checked ownership",
                ));
            }
            // The retained nominal inventory intentionally has no fabricated
            // String declaration. Ask the compiler's existing scalar TypeFacts
            // owner using the exact authenticated checked parameter type.
            let facts = revision
                .entry_program()
                .declarations
                .type_facts(&resolved.ty)
                .ok_or_else(|| grammar("ordered String parameter has no compiler TypeFacts"))?;
            if facts.copy || !facts.sized || facts.contains_resource || !facts.needs_drop {
                return Ok(None);
            }
            metadata.push(Some(json!({"type_identity":resolved.ty.identity_key(),
                "type_provenance":{"declaration":null,"arguments":[],
                    "ownership":"own","evidence_owner":"retained_checked_hir","copy":facts.copy,"sized":facts.sized,
                    "contains_resource":facts.contains_resource,"needs_drop":facts.needs_drop}})));
            continue;
        }
        let ResolvedType::Nominal {
            declaration,
            arguments,
        } = &resolved.ty
        else {
            return Ok(None);
        };
        let Some((kind, facts)) = module.signature_type_facts(&resolved.ty) else {
            return Err(grammar(
                "ordered signature retained nominal type facts are absent",
            ));
        };
        let copy = resolved.ownership == OwnershipMode::Value && facts.copy && !facts.needs_drop;
        let owned = resolved.ownership == OwnershipMode::Own && !facts.copy && facts.needs_drop;
        if !matches!(kind, DeclarationKind::Record | DeclarationKind::Variant)
            || !facts.sized
            || facts.contains_resource
            || !(copy || owned)
        {
            return Ok(None);
        }
        metadata.push(Some(json!({"type_identity":resolved.ty.identity_key(),
            "type_provenance":{"declaration":declaration.as_str(),"arguments":arguments.iter().map(ResolvedType::identity_key).collect::<Vec<_>>(),
                "ownership":if owned {"own"} else {"copy"},"evidence_owner":"retained_checked_hir","copy":facts.copy,"sized":facts.sized,
                "contains_resource":facts.contains_resource,"needs_drop":facts.needs_drop}})));
    }
    Ok(Some(metadata))
}

fn fresh_name(occupied: &mut BTreeSet<String>, counter: &mut usize) -> Result<String> {
    loop {
        let name = format!("spx_sig_stage_{counter}");
        *counter = counter
            .checked_add(1)
            .ok_or_else(|| capacity("signature staging name count overflow"))?;
        if *counter > MAX_WALK_NODES {
            return Err(capacity("signature staging names exceed the limit"));
        }
        if occupied.insert(name.clone()) {
            return Ok(name);
        }
    }
}

fn reserve_function(function: &Function, names: &mut BTreeSet<String>) {
    names.insert(function.name.clone());
    names.extend(function.params.iter().map(|param| param.name.clone()));
}

fn reserve_names(programs: &mut [Program], names: &mut BTreeSet<String>) -> Result<()> {
    let mut nodes = 0;
    let mut pattern_nodes = 0;
    for program in programs {
        for function in &program.functions {
            reserve_function(function, names);
        }
        for import in &program.module_uses {
            names.insert(import.alias.clone());
        }
        for interface in &program.interfaces {
            for import in &interface.imports {
                names.insert(import.name.clone());
            }
        }
        for ty in &program.types {
            if let TypeDeclarationKind::Class { methods, .. } = &ty.kind {
                for method in methods {
                    reserve_function(method, names);
                }
            }
        }
        walk_program(program, &mut nodes, &mut |expression| {
            match &expression.kind {
                ExprKind::Var(name) | ExprKind::Call { name, .. } => {
                    names.insert(name.clone());
                }
                ExprKind::Block { statements, .. } => {
                    for statement in statements {
                        match statement {
                            Statement::Let { name, .. } | Statement::Assign { name, .. } => {
                                names.insert(name.clone());
                            }
                            Statement::Unsafe { .. } | Statement::While { .. } => {}
                        }
                    }
                }
                ExprKind::Match { arms, .. } => {
                    for arm in arms {
                        reserve_pattern(&arm.pattern, names, 0, &mut pattern_nodes)?;
                    }
                }
                _ => {}
            }
            Ok(())
        })?;
    }
    Ok(())
}

fn pattern_budget(depth: usize, nodes: &mut usize) -> Result<()> {
    *nodes += 1;
    if depth > MAX_WALK_DEPTH || *nodes > MAX_WALK_NODES {
        return Err(capacity(
            "signature lexical pattern inventory exceeds its limit",
        ));
    }
    Ok(())
}

fn reserve_pattern(
    pattern: &MatchPattern,
    names: &mut BTreeSet<String>,
    depth: usize,
    nodes: &mut usize,
) -> Result<()> {
    pattern_budget(depth, nodes)?;
    match pattern {
        MatchPattern::Binding { name, .. } => {
            names.insert(name.clone());
        }
        MatchPattern::Variant { fields, .. } => {
            names.extend(fields.iter().map(|field| field.binding.clone()));
        }
        MatchPattern::Record { fields, .. } => {
            for field in fields {
                reserve_record_pattern(&field.pattern, names, depth + 1, nodes)?;
            }
        }
        MatchPattern::Or { alternatives, .. } => {
            for alternative in alternatives {
                reserve_pattern(alternative, names, depth + 1, nodes)?;
            }
        }
        MatchPattern::Wildcard { .. } | MatchPattern::Literal { .. } => {}
    }
    Ok(())
}

fn reserve_record_pattern(
    pattern: &RecordMatchFieldPattern,
    names: &mut BTreeSet<String>,
    depth: usize,
    nodes: &mut usize,
) -> Result<()> {
    pattern_budget(depth, nodes)?;
    match pattern {
        RecordMatchFieldPattern::Binding { name, .. } => {
            names.insert(name.clone());
        }
        RecordMatchFieldPattern::Record { fields, .. } => {
            for field in fields {
                reserve_record_pattern(&field.pattern, names, depth + 1, nodes)?;
            }
        }
        RecordMatchFieldPattern::Wildcard { .. } => {}
    }
    Ok(())
}

#[cfg(test)]
#[path = "signature/tests.rs"]
mod tests;
