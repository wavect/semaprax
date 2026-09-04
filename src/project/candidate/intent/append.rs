//! The appended-parameter signature lane, shared by the ordinary candidate
//! route and the detached package-consumer corpus route.

use std::collections::BTreeSet;

use serde_json::Value;

use crate::ast::{ExprKind, ModuleUseKind, Param, ParamMode, Program, Span};

use super::{
    array, call_bindings, capacity, grammar, identifier, literal, member, object, scalar_type,
    text, walk_program, Result, MAX_APPEND_PARAMETERS,
};

/// Append exactly the intention's typed scalar parameters to the selected
/// function and extend every authenticated call site with their exact literal
/// arguments. The append lane migrates a whole corpus, so the ordinary
/// candidate route and the detached package-consumer route read this one
/// implementation rather than diverging. Migrated calls are returned per
/// program, in program order, so a caller that owns a narrower inventory than
/// the corpus can select without rewalking any source.
pub(in crate::project::candidate) fn append_parameters(
    programs: &mut [Program],
    intent: &Value,
    target: &str,
    owner: usize,
    owner_module: &str,
    function_index: usize,
) -> Result<Vec<usize>> {
    object(intent, &["kind", "target", "append_parameters"])?;
    let additions = array(intent, "append_parameters")?;
    if additions.is_empty() || additions.len() > MAX_APPEND_PARAMETERS {
        return Err(capacity(
            "candidate signature requires one to sixteen appended parameters",
        ));
    }
    let function = &programs[owner].functions[function_index];
    let old_arity = function.params.len();
    let mut names = function
        .params
        .iter()
        .map(|p| p.name.clone())
        .collect::<BTreeSet<_>>();
    let mut params = Vec::with_capacity(additions.len());
    let mut arguments = Vec::with_capacity(additions.len());
    for addition in additions {
        object(addition, &["name", "type", "argument"])?;
        let name = identifier(text(addition, "name")?)?;
        if !names.insert(name.to_owned()) {
            return Err(grammar(
                "candidate signature parameter names must remain unique",
            ));
        }
        let ty = scalar_type(text(addition, "type")?)?;
        let argument = member(addition, "argument")?;
        if text(argument, "kind")? != text(addition, "type")? {
            return Err(grammar(
                "appended argument must be an exact typed scalar literal",
            ));
        }
        let expression = literal(argument)?;
        params.push(Param {
            name: name.to_owned(),
            mode: ParamMode::Value,
            ty,
            span: Span::default(),
        });
        arguments.push(expression);
    }
    let mut migrated_calls = Vec::with_capacity(programs.len());
    let mut nodes = 0;
    for program in programs.iter_mut() {
        let mut migrated = 0;
        // Existing imports select both provider identity and module;
        // an alias is never inferred from a provider's display name.
        for import in &program.module_uses {
            if import.kind == ModuleUseKind::Function
                && import.persistent_id == target
                && import.target_module != owner_module
            {
                return Err(grammar(
                    "candidate call provider module does not match its stable ID",
                ));
            }
        }
        let bindings = call_bindings(program)?;
        walk_program(program, &mut nodes, &mut |expression| {
            if let ExprKind::Call {
                name,
                type_arguments,
                args,
            } = &mut expression.kind
            {
                if bindings.get(name).is_some_and(|id| id == target) {
                    if !type_arguments.is_empty() || args.len() != old_arity {
                        return Err(grammar(
                            "candidate call migration has an unsupported signature",
                        ));
                    }
                    args.extend(arguments.iter().cloned());
                    migrated += 1;
                }
            }
            Ok(())
        })?;
        migrated_calls.push(migrated);
    }
    programs[owner].functions[function_index]
        .params
        .extend(params);
    Ok(migrated_calls)
}
