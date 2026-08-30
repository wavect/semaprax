//! Ordered Copy-parameter evolution with left-to-right argument staging.
//!
//! Every old argument is evaluated once even when its parameter is removed.
//! Parameter identity is selected by its old name; no lexical body renaming or
//! implicit conversion is performed. Full Project source replay remains the
//! caller's responsibility.

use super::{
    array, call_bindings, capacity, grammar, identifier, literal, member, object, scalar_type,
    text, walk_program, Result, MAX_WALK_DEPTH, MAX_WALK_NODES,
};
use crate::ast::{
    Expr, ExprKind, Function, MatchPattern, ModuleUseKind, Param, ParamMode, Program,
    RecordMatchFieldPattern, Span, Statement, Type, TypeDeclarationKind,
};
use serde_json::Value;
use std::collections::{BTreeMap, BTreeSet};

// This bounds internal declarations independently of narrower export-profile
// limits. Public ABI parameter ceilings are rechecked by ordinary Project
// admission; this route never raises them.
const MAX_PARAMETERS: usize = 4096;

#[derive(Clone)]
enum Argument {
    Existing(usize),
    Literal(Expr),
}

pub(super) fn apply(
    programs: &mut [Program],
    intent: &Value,
    owner: usize,
    function_index: usize,
) -> Result<usize> {
    object(intent, &["kind", "target", "parameters"])?;
    let target = text(intent, "target")?;
    let function = &programs[owner].functions[function_index];
    let owner_module = programs[owner].module.clone();
    let original_params = &function.params;
    if original_params.len() > MAX_PARAMETERS {
        return Err(capacity(
            "signature evolution exceeds its original parameter limit",
        ));
    }
    if original_params
        .iter()
        .any(|param| param.mode != ParamMode::Value || !copy_type(&param.ty))
    {
        return Err(grammar(
            "ordered signature evolution requires by-value built-in Copy parameters",
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
    for mapping in requested {
        if mapping.get("from").is_some() {
            object(mapping, &["from"])?;
            let name = text(mapping, "from")?;
            let index = *original
                .get(name)
                .ok_or_else(|| grammar("signature mapping names an unknown original parameter"))?;
            if !selected.insert(index) || !names.insert(name.to_owned()) {
                return Err(grammar(
                    "signature mapping duplicates an original parameter",
                ));
            }
            params.push(original_params[index].clone());
            arguments.push(Argument::Existing(index));
        } else {
            object(mapping, &["name", "type", "argument"])?;
            let name = identifier(text(mapping, "name")?)?;
            if original.contains_key(name) || !names.insert(name.to_owned()) {
                return Err(grammar(
                    "new signature parameter must not rename or reinterpret an old binding",
                ));
            }
            let type_name = text(mapping, "type")?;
            let ty = scalar_type(type_name)?;
            let argument = member(mapping, "argument")?;
            if text(argument, "kind")? != type_name {
                return Err(grammar(
                    "new signature argument must be an exact typed scalar literal",
                ));
            }
            arguments.push(Argument::Literal(literal(argument)?));
            params.push(Param {
                name: name.to_owned(),
                mode: ParamMode::Value,
                ty,
                span: Span::default(),
            });
        }
    }
    let old_arity = original_params.len();
    let mut occupied = names;
    // Reserve lexical names across the complete candidate, including unrelated
    // and shadowed binders. This conservative whole-program set avoids capture
    // without trying to rename any original lexical reference.
    reserve_names(programs, &mut occupied)?;
    let mut generated = 0usize;
    let mut nodes = 0usize;
    let mut added_nodes = 0usize;
    let mut migrated_calls = 0usize;
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
            added_nodes = added_nodes
                .checked_add(1 + arguments.len())
                .ok_or_else(|| capacity("signature migration node count overflow"))?;
            if added_nodes > MAX_WALK_NODES {
                return Err(capacity(
                    "signature migration generated nodes exceed the limit",
                ));
            }
            let call_name = name.clone();
            let span = expression.span;
            let mut stages = Vec::with_capacity(old_arity);
            for _ in 0..old_arity {
                stages.push(fresh_name(&mut occupied, &mut generated)?);
            }
            let mapped = arguments
                .iter()
                .map(|argument| match argument {
                    Argument::Existing(index) => Expr {
                        kind: ExprKind::Var(stages[*index].clone()),
                        span,
                    },
                    Argument::Literal(expression) => expression.clone(),
                })
                .collect::<Vec<_>>();
            // Move, never duplicate or omit, every original argument subtree.
            let ExprKind::Call { args, .. } =
                std::mem::replace(&mut expression.kind, ExprKind::Int(0))
            else {
                unreachable!("call inspected immediately before replacement");
            };
            let statements = args
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
                .collect();
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
mod tests {
    use super::*;
    use crate::interpreter::{evaluate_resolved_zero_arg_i64, ResolvedEvaluationOutcome};
    use crate::{format, hir, parse};
    use serde_json::json;

    fn program(source: &str) -> Program {
        parse(source, "signature.spx").unwrap()
    }

    fn outcome(program: &Program) -> ResolvedEvaluationOutcome {
        let canonical = format::canonical(program);
        let reparsed = parse(&canonical, "signature.spx").unwrap();
        let resolved = hir::resolve(&reparsed).unwrap();
        evaluate_resolved_zero_arg_i64(&resolved, "app.main", 100_000)
            .unwrap()
            .outcome
    }

    fn evolve(programs: &mut [Program], parameters: Value) -> Result<super::super::IntentSummary> {
        super::super::apply(
            programs,
            &json!({
                "kind":"change_function_signature","target":"math.select","parameters":parameters
            }),
        )
    }

    #[test]
    fn reordered_copy_parameters_preserve_value_and_stage_all_original_arguments() {
        let mut programs = vec![program(
            r#"module test.signature;
@id("math.select") fn select(left: i64, right: i64) -> i64 { left * 10 + right }
@id("app.main") fn main() -> i64 { select(2, 3) }
"#,
        )];
        let before = outcome(&programs[0]);
        let summary = evolve(&mut programs, json!([{"from":"right"},{"from":"left"}])).unwrap();
        assert_eq!(summary.migrated_calls, 1);
        assert_eq!(before, ResolvedEvaluationOutcome::ReturnedI64(23));
        assert_eq!(outcome(&programs[0]), before);
        let canonical = format::canonical(&programs[0]);
        assert!(canonical.contains("fn select(right: i64, left: i64)"));
        assert!(canonical.contains("let spx_sig_stage_0 = 2; let spx_sig_stage_1 = 3; select(spx_sig_stage_1, spx_sig_stage_0)"));
        assert_eq!(format::canonical(&program(&canonical)), canonical);
    }

    #[test]
    fn dropped_argument_and_reordered_arguments_preserve_first_checked_failure() {
        for parameters in [
            json!([{"from":"right"}]),
            json!([{"from":"right"},{"from":"left"}]),
        ] {
            let mut programs = vec![program(
                r#"module test.signature;
@id("math.select") fn select(left: i64, right: i64) -> i64 { right }
@id("app.main") fn main() -> i64 { select(1 / 0, 9223372036854775807 + 1) }
"#,
            )];
            let before = outcome(&programs[0]);
            assert!(matches!(
                before,
                ResolvedEvaluationOutcome::LanguageFailure(_)
            ));
            evolve(&mut programs, parameters).unwrap();
            assert_eq!(outcome(&programs[0]), before);
            let canonical = format::canonical(&programs[0]);
            assert!(
                canonical.find("= 1 / 0;").unwrap()
                    < canonical.find("= 9223372036854775807 + 1;").unwrap()
            );
        }
    }

    #[test]
    fn generated_staging_names_do_not_capture_existing_parameters_or_local_bindings() {
        let mut programs = vec![program(
            r#"module test.signature;
@id("math.select") fn select(left: i64, right: i64) -> i64 { left * 10 + right }
@id("math.nested") fn nested(spx_sig_stage_0: i64) -> i64 {
    let spx_sig_stage_1 = 3;
    select(spx_sig_stage_0, spx_sig_stage_1)
}
@id("app.main") fn main() -> i64 { nested(2) }
"#,
        )];
        let before = outcome(&programs[0]);
        evolve(&mut programs, json!([{"from":"right"},{"from":"left"}])).unwrap();
        assert_eq!(outcome(&programs[0]), before);
        let canonical = format::canonical(&programs[0]);
        assert!(canonical.contains(
            "let spx_sig_stage_2 = spx_sig_stage_0; let spx_sig_stage_3 = spx_sig_stage_1;"
        ));
        assert!(canonical.contains("select(spx_sig_stage_3, spx_sig_stage_2)"));
    }

    #[test]
    fn import_alias_and_declared_effect_calls_keep_original_staging_order() {
        let mut programs = vec![
            program(
                r#"module test.signature;
@id("math.select") fn select(left: i64, right: i64) -> i64 { left + right }
"#,
            ),
            program(
                r#"module test.consumer;
use function @id("math.select") from test.signature as choose;
permit { clock.read }
@id("math.first") fn first() -> i64 uses { clock.read } { 2 }
@id("math.second") fn second() -> i64 uses { clock.read } { 3 }
@id("app.main") fn main() -> i64 uses { clock.read } { choose(first(), second()) }
"#,
            ),
        ];
        evolve(&mut programs, json!([{"from":"right"},{"from":"left"},{"name":"extra","type":"bool","argument":{"kind":"bool","value":true}}])).unwrap();
        let canonical = format::canonical(&programs[1]);
        assert!(canonical.contains("from test.signature as choose"));
        assert!(canonical.contains("let spx_sig_stage_0 = first(); let spx_sig_stage_1 = second(); choose(spx_sig_stage_1, spx_sig_stage_0, true)"));
        assert_eq!(programs[1].permits, ["clock.read"]);
        assert_eq!(programs[1].functions[2].effects, ["clock.read"]);
    }

    #[test]
    fn removal_of_used_parameter_fails_real_verifier_and_rename_or_type_guesses_reject() {
        let base = r#"module test.signature;
@id("math.select") fn select(left: i64, right: i64) -> i64 { left + right }
@id("app.main") fn main() -> i64 { select(2, 3) }
"#;
        let mut used = vec![program(base)];
        evolve(&mut used, json!([{"from":"right"}])).unwrap();
        assert!(hir::resolve(&program(&format::canonical(&used[0]))).is_err());
        for parameters in [
            json!([{"from":"left","name":"renamed"},{"from":"right"}]),
            json!([{"from":"left","type":"bool"},{"from":"right"}]),
            json!([{"name":"left","type":"bool","argument":{"kind":"bool","value":true}}]),
            json!([{"from":"left"},{"from":"left"}]),
        ] {
            let mut programs = vec![program(base)];
            let before = format::canonical(&programs[0]);
            let errors = match evolve(&mut programs, parameters) {
                Ok(_) => panic!("unsupported signature mapping succeeded"),
                Err(errors) => errors,
            };
            assert!(errors.iter().any(|error| error.code == "SPX-G225"));
            assert_eq!(format::canonical(&programs[0]), before);
        }
    }

    #[test]
    fn owned_and_borrowed_signature_migrations_reject_before_mutation() {
        for parameter in ["own Bytes", "borrow str", "shared Bytes", "string"] {
            let source = format!("module test.signature;\n@id(\"math.select\") fn select(value: {parameter}) -> i64 {{ 0 }}\n");
            let mut programs = vec![program(&source)];
            let before = format::canonical(&programs[0]);
            let errors = match evolve(&mut programs, json!([])) {
                Ok(_) => panic!("non-Copy signature mapping succeeded"),
                Err(errors) => errors,
            };
            assert!(errors.iter().any(|error| error.code == "SPX-G225"));
            assert_eq!(format::canonical(&programs[0]), before);
        }
    }
}
