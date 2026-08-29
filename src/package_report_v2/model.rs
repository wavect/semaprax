use std::collections::{BTreeMap, BTreeSet, VecDeque};

use crate::ast::Program;
use crate::bounded_output::{self, BudgetedJoin as _};
use crate::diagnostic::Diagnostic;
use crate::hir::{
    self, DeclarationId, OwnershipMode, ResolvedExpr, ResolvedExprKind, ResolvedFunction,
    ResolvedProgram, ResolvedResourceDropKind, ResolvedType, ResolvedTypeDeclaration,
    ResolvedTypeDeclarationKind,
};
use crate::{codegen, wasm};

use super::contract;
use super::report_quote_json as quote_json;
use super::wire::{domain_digest, CONTRACT_DIGEST_DOMAIN, SOURCE_DIGEST_DOMAIN, SOURCE_SCHEMA};
use super::{
    limit_error, projection_error, PackageReportV2Options, MAX_CONTRACT_DEPTH, MAX_CONTRACT_NODES,
    MAX_FUNCTIONS, MAX_REACHABLE_TYPES, MAX_SOURCE_BYTES, SCHEMA,
};

const NONCLAIMS_JSON: &str = "[\
\"self_contained_canonical_source_subject_not_signed_provenance\",\
\"semantic_facts_rebuilt_from_embedded_verified_source\",\
\"contracts_are_identity_based_structural_facts_not_logical_implication_proofs\",\
\"target_available_means_compiler_projection_not_execution_or_conformance\",\
\"unproven_facts_are_never_silently_available_or_compatible\",\
\"no_dependency_model_resolver_registry_network_fetch_or_cache\",\
\"no_build_script_compilation_linking_or_target_execution\",\
\"no_signature_trusted_provenance_license_sbom_or_policy\",\
\"no_source_mutation_publication_commit_or_migration_authority\",\
\"no_new_language_graph_cleanup_backend_or_runtime_semantics\"]";

macro_rules! bf {
    ($($argument:tt)*) => { bounded_output::budgeted_format(format_args!($($argument)*)) };
}

#[derive(Clone, Copy)]
pub(super) enum TargetProof {
    Available {
        target: &'static str,
        proof: &'static str,
    },
    Unavailable {
        target: &'static str,
        proof: &'static str,
        reason: &'static str,
    },
    Unproven {
        target: &'static str,
        reason: &'static str,
    },
}

pub(super) fn target_proofs(program: &Program, resolved: &ResolvedProgram) -> [TargetProof; 2] {
    let has_export_candidate = program
        .functions
        .iter()
        .any(|function| function.explicit_id && function.type_parameters.is_empty());
    [
        target_projection(
            "native64",
            "production_c11_projection",
            has_export_candidate,
            || codegen::emit_c(program).map(|_| ()),
        ),
        target_projection(
            "wasm32",
            "production_core_wasm_projection",
            has_export_candidate,
            || wasm::emit_resolved_module(resolved).map(|_| ()),
        ),
    ]
}

pub(super) fn target_projection(
    target: &'static str,
    proof: &'static str,
    has_export_candidate: bool,
    operation: impl FnOnce() -> Result<(), Diagnostic>,
) -> TargetProof {
    if !has_export_candidate {
        return TargetProof::Unavailable {
            target,
            proof: "closed_source_export_inventory",
            reason: "no_explicit_monomorphic_export",
        };
    }
    let (projected, overflowed, _) =
        bounded_output::with_limit_usage(super::TARGET_PROJECTION_MAX_BYTES, operation);
    if overflowed {
        TargetProof::Unproven {
            target,
            reason: "projection_limit_exceeded",
        }
    } else if projected.is_ok() {
        TargetProof::Available { target, proof }
    } else {
        // Rejection is not evidence of target absence.
        TargetProof::Unproven {
            target,
            reason: "projection_rejected",
        }
    }
}

pub(super) fn render_payload(
    canonical_source: &str,
    revision: &str,
    program: &Program,
    resolved: &ResolvedProgram,
    options: &PackageReportV2Options,
    target_proofs: &[TargetProof; 2],
) -> Result<String, Diagnostic> {
    if program.functions.len() > MAX_FUNCTIONS {
        return Err(limit_error(bf!(
            "functions exceeds the {MAX_FUNCTIONS} v2 limit"
        )));
    }

    let mut source_functions = program
        .functions
        .iter()
        .map(|function| (function.stable_id.as_str(), function))
        .collect::<BTreeMap<_, _>>();
    let mut functions = resolved.functions.iter().collect::<Vec<_>>();
    functions.sort_by(|left, right| {
        left.id
            .as_str()
            .as_bytes()
            .cmp(right.id.as_str().as_bytes())
    });

    let mut admitted = Vec::<(&ResolvedFunction, Vec<String>, Vec<String>)>::new();
    let mut unproven = BTreeMap::<String, String>::new();
    let mut contract_nodes = 0usize;
    for function in functions {
        let mut normalized_contracts = None;
        let source = source_functions
            .remove(function.id.as_str())
            .ok_or_else(|| {
                projection_error(bf!(
                    "resolved function `{}` has no exact source declaration",
                    function.id
                ))
            })?;
        let reason = if !source.explicit_id {
            Some("automatic_identity")
        } else if !source.type_parameters.is_empty() {
            Some("generic_function")
        } else if function
            .params
            .iter()
            .map(|parameter| &parameter.ty)
            .chain(std::iter::once(&function.return_type))
            .any(|ty| type_depth(ty) > MAX_CONTRACT_DEPTH)
        {
            Some("type_depth_unproven")
        } else {
            let (nodes, depth) = contract_shape(function);
            if !admit_contract_shape(&mut contract_nodes, nodes, depth)? {
                Some("contract_depth_unproven")
            } else {
                match contract::normalize(function) {
                    Ok(normalized) => {
                        normalized_contracts = Some(normalized);
                        None
                    }
                    Err(_) => Some("contract_identity_unproven"),
                }
            }
        };
        if let Some(reason) = reason {
            unproven.insert(
                bounded_output::budgeted_clone(function.id.as_str()),
                bf!(
                    "{{\"stable_id\":{},\"name\":{},\"reason\":{}}}",
                    quote_json(function.id.as_str()),
                    quote_json(&function.name),
                    quote_json(reason)
                ),
            );
        } else {
            let (requires, ensures) = normalized_contracts.ok_or_else(|| {
                projection_error("admitted contract normalization was not retained")
            })?;
            admitted.push((function, requires, ensures));
        }
    }
    for source in source_functions.into_values() {
        let reason = if !source.explicit_id {
            "automatic_identity"
        } else {
            "generic_function"
        };
        unproven.insert(
            bounded_output::budgeted_clone(&source.stable_id),
            bf!(
                "{{\"stable_id\":{},\"name\":{},\"reason\":{}}}",
                quote_json(&source.stable_id),
                quote_json(&source.name),
                quote_json(reason)
            ),
        );
    }
    let targets = target_proofs
        .iter()
        .map(render_target_fact)
        .collect::<Vec<_>>();
    let exports = admitted
        .iter()
        .map(|(function, requires, ensures)| render_export(resolved, function, requires, ensures))
        .collect::<Result<Vec<_>, _>>()?;
    let admitted_functions = admitted
        .iter()
        .map(|(function, _, _)| *function)
        .collect::<Vec<_>>();
    let (types, unproven_types) = reachable_type_closure(resolved, &admitted_functions)?;
    let source_digest = domain_digest(SOURCE_DIGEST_DOMAIN, canonical_source.as_bytes());
    Ok(bf!(
        "{{\"schema\":{},\"source\":{{\"schema\":{},\"bytes\":{},\"sha256\":{},\"revision\":{},\"text\":{}}},\"limits\":{{\"max_source_bytes\":{},\"max_functions\":{},\"max_contract_depth\":{},\"max_contract_nodes\":{},\"max_reachable_types\":{},\"max_output_bytes\":{},\"max_render_string_bytes\":{},\"requested_max_bytes\":{}}},\"package\":{{\"name\":{},\"exports_admitted\":{},\"exports_unproven\":{},\"reachable_types_proven\":{},\"reachable_types_unproven\":{}}},\"targets\":[{}],\"exports\":[{}],\"unproven_exports\":[{}],\"types\":[{}],\"unproven_types\":[{}],\"nonclaims\":{}}}",
        quote_json(SCHEMA),
        quote_json(SOURCE_SCHEMA),
        canonical_source.len(),
        quote_json(&source_digest),
        quote_json(revision),
        quote_json(canonical_source),
        MAX_SOURCE_BYTES,
        MAX_FUNCTIONS,
        MAX_CONTRACT_DEPTH,
        MAX_CONTRACT_NODES,
        MAX_REACHABLE_TYPES,
        super::MAX_OUTPUT_BYTES,
        super::MAX_RENDER_STRING_BYTES,
        options.max_bytes,
        quote_json(&program.module),
        exports.len(),
        unproven.len(),
        types.len(),
        unproven_types.len(),
        targets.budgeted_join(","),
        exports.budgeted_join(","),
        unproven.into_values().collect::<Vec<_>>().budgeted_join(","),
        types.budgeted_join(","),
        unproven_types.budgeted_join(","),
        NONCLAIMS_JSON
    ))
}

fn render_export(
    program: &ResolvedProgram,
    function: &ResolvedFunction,
    normalized_requires: &[String],
    normalized_ensures: &[String],
) -> Result<String, Diagnostic> {
    let parameters = function
        .params
        .iter()
        .enumerate()
        .map(|(index, parameter)| {
            bf!(
                "{{\"index\":{index},\"name\":{},\"type\":{},\"ownership\":{}}}",
                quote_json(&parameter.name),
                type_json(&parameter.ty),
                quote_json(ownership_text(parameter.ownership))
            )
        })
        .collect::<Vec<_>>();
    let result_ownership = program
        .declarations
        .type_facts(&function.return_type)
        .map(|facts| {
            if facts.copy {
                OwnershipMode::Value
            } else {
                OwnershipMode::Own
            }
        })
        .ok_or_else(|| {
            projection_error(bf!(
                "return type of `{}` has no authenticated ownership facts",
                function.id
            ))
        })?;
    let effects = function
        .effects
        .iter()
        .map(String::as_str)
        .collect::<BTreeSet<_>>()
        .into_iter()
        .map(quote_json)
        .collect::<Vec<_>>();
    let requires = render_contracts(normalized_requires);
    let ensures = render_contracts(normalized_ensures);
    Ok(bf!(
        "{{\"stable_id\":{},\"name\":{},\"parameters\":[{}],\"result\":{{\"type\":{},\"ownership\":{}}},\"effects\":[{}],\"requires\":[{}],\"ensures\":[{}]}}",
        quote_json(function.id.as_str()),
        quote_json(&function.name),
        parameters.budgeted_join(","),
        type_json(&function.return_type),
        quote_json(ownership_text(result_ownership)),
        effects.budgeted_join(","),
        requires.budgeted_join(","),
        ensures.budgeted_join(",")
    ))
}

fn render_contracts(facts: &[String]) -> Vec<String> {
    facts
        .iter()
        .enumerate()
        .map(|(index, fact)| {
            bf!(
                "{{\"index\":{index},\"sha256\":{},\"fact\":{fact},\"proof\":\"structural_identity_fact_only\"}}",
                quote_json(&domain_digest(CONTRACT_DIGEST_DOMAIN, fact.as_bytes()))
            )
        })
        .collect()
}

pub(super) fn render_target_fact(fact: &TargetProof) -> String {
    let (target, status, proof, reason) = match fact {
        TargetProof::Available { target, proof } => (*target, "available", *proof, "none"),
        TargetProof::Unavailable {
            target,
            proof,
            reason,
        } => (*target, "unavailable", *proof, *reason),
        TargetProof::Unproven { target, reason } => (*target, "unproven", "none", *reason),
    };
    bf!(
        "{{\"target\":{},\"status\":{},\"proof\":{},\"reason\":{},\"execution\":\"unproven\"}}",
        quote_json(target),
        quote_json(status),
        quote_json(proof),
        quote_json(reason)
    )
}

fn reachable_type_closure(
    program: &ResolvedProgram,
    functions: &[&ResolvedFunction],
) -> Result<(Vec<String>, Vec<String>), Diagnostic> {
    let declarations = program
        .types
        .iter()
        .map(|declaration| (declaration.id.clone(), declaration))
        .collect::<BTreeMap<_, _>>();
    let mut pending = VecDeque::<DeclarationId>::new();
    let mut seen = BTreeSet::<DeclarationId>::new();
    for function in functions {
        for parameter in &function.params {
            collect_nominals(&parameter.ty, &mut pending);
        }
        collect_nominals(&function.return_type, &mut pending);
        for contract in function.requires.iter().chain(&function.ensures) {
            collect_expression_types(contract, &mut pending);
        }
    }
    let mut proven = BTreeMap::<DeclarationId, String>::new();
    let mut unproven = BTreeSet::<DeclarationId>::new();
    while let Some(id) = pending.pop_front() {
        if !seen.insert(id.clone()) {
            continue;
        }
        admit_reachable_type_count(seen.len())?;
        let Some(declaration) = declarations.get(&id) else {
            unproven.insert(id);
            continue;
        };
        collect_declaration_types(program, declaration, &mut pending);
        proven.insert(
            declaration.id.clone(),
            render_type_declaration(program, declaration),
        );
    }
    Ok((
        proven.into_values().collect(),
        unproven
            .into_iter()
            .map(|id| {
                bf!(
                    "{{\"declaration\":{},\"reason\":\"definition_unavailable_in_source_subject\"}}",
                    quote_json(id.as_str())
                )
            })
            .collect(),
    ))
}

fn collect_declaration_types(
    program: &ResolvedProgram,
    declaration: &ResolvedTypeDeclaration,
    pending: &mut VecDeque<DeclarationId>,
) {
    match &declaration.kind {
        ResolvedTypeDeclarationKind::Resource { .. } => {}
        ResolvedTypeDeclarationKind::Record { fields }
        | ResolvedTypeDeclarationKind::Class { fields, .. } => {
            for field in fields {
                collect_nominals(&field.ty, pending);
            }
            if let Some(parent) = program.declarations.class_parent(&declaration.id) {
                pending.push_back(parent.clone());
            }
        }
        ResolvedTypeDeclarationKind::Variant { cases } => {
            for field in cases.iter().flat_map(|case| &case.fields) {
                collect_nominals(&field.ty, pending);
            }
        }
    }
}

fn render_type_declaration(
    program: &ResolvedProgram,
    declaration: &ResolvedTypeDeclaration,
) -> String {
    let parameters = declaration
        .type_parameters
        .iter()
        .map(|parameter| {
            bf!(
                "{{\"index\":{},\"name\":{}}}",
                parameter.index,
                quote_json(&parameter.name)
            )
        })
        .collect::<Vec<_>>()
        .budgeted_join(",");
    let kind = match &declaration.kind {
        ResolvedTypeDeclarationKind::Resource { drop } => {
            let lifecycle = match &drop.kind {
                ResolvedResourceDropKind::Trivial => {
                    bf!(
                        "{{\"id\":{},\"kind\":\"trivial\"}}",
                        quote_json(drop.id.as_str())
                    )
                }
                ResolvedResourceDropKind::Imported { import, import_key } => bf!(
                    "{{\"id\":{},\"kind\":\"imported\",\"import\":{},\"import_key\":{}}}",
                    quote_json(drop.id.as_str()),
                    quote_json(import.as_str()),
                    quote_json(import_key)
                ),
            };
            bf!("{{\"kind\":\"resource\",\"lifecycle\":{lifecycle}}}")
        }
        ResolvedTypeDeclarationKind::Record { fields } => bf!(
            "{{\"kind\":\"record\",\"fields\":[{}]}}",
            fields
                .iter()
                .map(render_field)
                .collect::<Vec<_>>()
                .budgeted_join(",")
        ),
        ResolvedTypeDeclarationKind::Class { fields, methods } => bf!(
            "{{\"kind\":\"class\",\"parent\":{},\"fields\":[{}],\"methods\":[{}]}}",
            program
                .declarations
                .class_parent(&declaration.id)
                .map_or_else(
                    || bounded_output::budgeted_clone("\"none\""),
                    |parent| quote_json(parent.as_str())
                ),
            fields
                .iter()
                .map(render_field)
                .collect::<Vec<_>>()
                .budgeted_join(","),
            methods
                .iter()
                .map(|method| quote_json(method.as_str()))
                .collect::<Vec<_>>()
                .budgeted_join(",")
        ),
        ResolvedTypeDeclarationKind::Variant { cases } => bf!(
            "{{\"kind\":\"variant\",\"cases\":[{}]}}",
            cases
                .iter()
                .map(|case| bf!(
                    "{{\"id\":{},\"name\":{},\"index\":{},\"fields\":[{}]}}",
                    quote_json(case.id.as_str()),
                    quote_json(&case.name),
                    case.index,
                    case.fields
                        .iter()
                        .map(render_field)
                        .collect::<Vec<_>>()
                        .budgeted_join(",")
                ))
                .collect::<Vec<_>>()
                .budgeted_join(",")
        ),
    };
    bf!(
        "{{\"stable_id\":{},\"name\":{},\"type_parameters\":[{}],\"definition\":{kind}}}",
        quote_json(declaration.id.as_str()),
        quote_json(&declaration.name),
        parameters
    )
}

fn render_field(field: &hir::ResolvedFieldDeclaration) -> String {
    bf!(
        "{{\"id\":{},\"name\":{},\"index\":{},\"type\":{}}}",
        quote_json(field.id.as_str()),
        quote_json(&field.name),
        field.index,
        type_json(&field.ty)
    )
}

fn collect_expression_types(expression: &ResolvedExpr, pending: &mut VecDeque<DeclarationId>) {
    let mut expressions = vec![expression];
    while let Some(expression) = expressions.pop() {
        collect_nominals(&expression.ty, pending);
        match &expression.kind {
            ResolvedExprKind::Call { type_arguments, .. } => {
                for argument in type_arguments {
                    collect_nominals(argument, pending);
                }
            }
            ResolvedExprKind::Try { residual_type, .. }
            | ResolvedExprKind::TryOption { residual_type, .. } => {
                collect_nominals(residual_type, pending);
            }
            _ => {}
        }
        hir::push_resolved_expression_children_in_authored_order(expression, &mut expressions);
    }
}

fn collect_nominals(ty: &ResolvedType, pending: &mut VecDeque<DeclarationId>) {
    let mut types = vec![ty];
    while let Some(ty) = types.pop() {
        if let ResolvedType::Nominal {
            declaration,
            arguments,
        } = ty
        {
            pending.push_back(declaration.clone());
            types.extend(arguments.iter().rev());
        }
    }
}

fn contract_shape(function: &ResolvedFunction) -> (usize, usize) {
    let mut count = 0usize;
    let mut maximum = 0usize;
    let mut pending = function
        .requires
        .iter()
        .chain(&function.ensures)
        .map(|expression| (expression, 1usize))
        .collect::<Vec<_>>();
    while let Some((expression, depth)) = pending.pop() {
        count = count.saturating_add(1);
        maximum = maximum.max(depth);
        let mut children = Vec::new();
        hir::push_resolved_expression_children_in_authored_order(expression, &mut children);
        pending.extend(
            children
                .into_iter()
                .map(|child| (child, depth.saturating_add(1))),
        );
    }
    (count, maximum)
}

pub(super) fn admit_contract_shape(
    cumulative_nodes: &mut usize,
    nodes: usize,
    depth: usize,
) -> Result<bool, Diagnostic> {
    *cumulative_nodes = cumulative_nodes
        .checked_add(nodes)
        .ok_or_else(|| limit_error("contract node accounting overflow"))?;
    if *cumulative_nodes > MAX_CONTRACT_NODES {
        return Err(limit_error(bf!(
            "contract_nodes exceeds the {MAX_CONTRACT_NODES} v2 limit"
        )));
    }
    Ok(depth <= MAX_CONTRACT_DEPTH)
}

pub(super) fn admit_reachable_type_count(count: usize) -> Result<(), Diagnostic> {
    if count > MAX_REACHABLE_TYPES {
        return Err(limit_error(bf!(
            "reachable_types exceeds the {MAX_REACHABLE_TYPES} v2 limit"
        )));
    }
    Ok(())
}

fn type_depth(ty: &ResolvedType) -> usize {
    let mut maximum = 0usize;
    let mut pending = vec![(ty, 1usize)];
    while let Some((ty, depth)) = pending.pop() {
        maximum = maximum.max(depth);
        if let ResolvedType::Nominal { arguments, .. } = ty {
            pending.extend(
                arguments
                    .iter()
                    .map(|argument| (argument, depth.saturating_add(1))),
            );
        }
    }
    maximum
}

fn ownership_text(ownership: OwnershipMode) -> &'static str {
    match ownership {
        OwnershipMode::Value => "value",
        OwnershipMode::Own => "own",
        OwnershipMode::Borrow => "borrow",
        OwnershipMode::Shared => "shared",
    }
}

pub(super) fn type_json(ty: &ResolvedType) -> String {
    match ty {
        ResolvedType::Unit => bounded_output::budgeted_clone("{\"kind\":\"unit\"}"),
        ResolvedType::I64 => primitive("i64"),
        ResolvedType::I32 => primitive("i32"),
        ResolvedType::Char => primitive("char"),
        ResolvedType::U8 => primitive("u8"),
        ResolvedType::Usize => primitive("usize"),
        ResolvedType::ArrayU8(length) => bf!(
            "{{\"kind\":\"fixed_array\",\"element\":{},\"length\":{length}}}",
            primitive("u8")
        ),
        ResolvedType::F32 => primitive("f32"),
        ResolvedType::F64 => primitive("f64"),
        ResolvedType::Bool => primitive("bool"),
        ResolvedType::String => bounded_output::budgeted_clone("{\"kind\":\"owned_string\"}"),
        ResolvedType::Bytes => bounded_output::budgeted_clone("{\"kind\":\"owned_bytes\"}"),
        ResolvedType::Str => bounded_output::budgeted_clone("{\"kind\":\"borrowed_str\"}"),
        ResolvedType::SliceU8 => bounded_output::budgeted_clone(
            "{\"kind\":\"borrowed_slice\",\"element\":{\"kind\":\"primitive\",\"name\":\"u8\"}}",
        ),
        ResolvedType::TypeParameter { owner, index } => bf!(
            "{{\"kind\":\"type_parameter\",\"owner\":{},\"index\":{index}}}",
            quote_json(owner.as_str())
        ),
        ResolvedType::Nominal {
            declaration,
            arguments,
        } => bf!(
            "{{\"kind\":\"nominal\",\"declaration\":{},\"arguments\":[{}]}}",
            quote_json(declaration.as_str()),
            arguments
                .iter()
                .map(type_json)
                .collect::<Vec<_>>()
                .budgeted_join(",")
        ),
    }
}

fn primitive(name: &str) -> String {
    bf!("{{\"kind\":\"primitive\",\"name\":{}}}", quote_json(name))
}
