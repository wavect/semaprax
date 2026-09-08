//! Graph v34: exact concrete instance ownership, layered over frozen graph bytes.
use super::*;
use crate::cleanup::{CleanupStorageOrigin, FieldLivenessShape};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};

/// Render the pre-v34 graph contract, for consumers with frozen versioned bytes.
/// This retains validation and does not grant admission to any new source shape.
pub fn to_legacy_json(program: &Program) -> Result<String, Vec<Diagnostic>> {
    let resolved = hir::resolve(program)?;
    hir::validate(&resolved).map_err(|e| vec![e])?;
    let functions = resolved
        .functions
        .iter()
        .map(|f| f.id.clone())
        .chain(resolved.function_templates.iter().map(|f| f.id.clone()))
        .collect();
    let types = resolved.types.iter().map(|t| t.id.clone()).collect();
    legacy_graph_json(
        &resolved,
        &revision(program),
        &functions,
        &types,
        &GraphView::Module,
    )
    .map_err(|e| vec![e])
}

/// Render a context slice under the frozen pre-v34 contract.
pub fn legacy_context_json(
    program: &Program,
    symbol: &str,
    depth: usize,
) -> Result<Option<String>, Vec<Diagnostic>> {
    reject_source_native_rust_imports(program)?;
    let resolved = hir::resolve(program)?;
    reject_native_rust_imports(&resolved).map_err(|e| vec![e])?;
    context_hir_json(&resolved, &revision(program), symbol, depth, true).map_err(|e| vec![e])
}

/// Frozen evidence products render their contracted graph version from checked HIR.
pub(crate) fn to_legacy_hir_json(
    program: &ResolvedProgram,
    source_revision: &str,
) -> Result<String, Diagnostic> {
    hir::validate(program)?;
    let functions = program
        .functions
        .iter()
        .map(|f| f.id.clone())
        .chain(program.function_templates.iter().map(|f| f.id.clone()))
        .collect();
    let types = program.types.iter().map(|t| t.id.clone()).collect();
    legacy_graph_json(
        program,
        source_revision,
        &functions,
        &types,
        &GraphView::Module,
    )
}

/// Replay submitted canonical graph bytes against retained source. No submitted
/// digest, revision, ownership fact, or internally consistent remint is trusted.
pub fn verify_json(program: &Program, submitted: &str) -> Result<(), Vec<Diagnostic>> {
    if to_json(program)? == submitted {
        Ok(())
    } else {
        Err(vec![Diagnostic::io(
            "SPX-G411",
            "semantic graph differs from exact retained source replay",
        )])
    }
}

pub(super) fn legacy_graph_json(
    program: &ResolvedProgram,
    source_revision: &str,
    selected_functions: &BTreeSet<DeclarationId>,
    selected_types: &BTreeSet<DeclarationId>,
    view: &GraphView<'_>,
) -> Result<String, Diagnostic> {
    if hir::function_value::requires_function_values(program) {
        return Err(Diagnostic::io(
            "SPX-G411",
            "function values require Graph v36",
        ));
    }
    render_graph_json(
        program,
        source_revision,
        selected_functions,
        selected_types,
        view,
        false,
    )
}

pub(super) fn graph_json(
    program: &ResolvedProgram,
    source_revision: &str,
    selected_functions: &BTreeSet<DeclarationId>,
    selected_types: &BTreeSet<DeclarationId>,
    view: &GraphView<'_>,
) -> Result<String, Diagnostic> {
    if program.function_instances.is_empty()
        && !nested_owned::requires_generic_result_schema(program)
        && !generic_mapping::requires_v35(&program.function_templates)
        && !hir::function_value::requires_function_values(program)
    {
        return legacy_graph_json(
            program,
            source_revision,
            selected_functions,
            selected_types,
            view,
        );
    }
    let base = nested_owned::generic_payload_schema(program)?;
    let mut graph = render_graph_json(
        program,
        source_revision,
        selected_functions,
        selected_types,
        view,
        true,
    )?;
    graph = graph.replacen(
        &quote_json(base),
        &quote_json(nested_owned::graph_schema(program)?),
        1,
    );
    let mut instances = program
        .function_instances
        .iter()
        .filter(|i| selected_functions.contains(&i.template))
        .collect::<Vec<_>>();
    // Identity-set presentation order, never cleanup inventory/plan order.
    instances.sort_by_key(|i| (&i.template, &i.type_arguments));
    let facts = instances
        .into_iter()
        .map(|i| instance_json(program, source_revision, i))
        .collect::<Result<Vec<_>, _>>()?
        .budgeted_join(",");
    if generic_mapping::requires_v35(&program.function_templates) {
        graph.pop();
        write!(
            graph,
            ",\"generic_template_forwarding\":{}}}",
            serde_json::to_string(&generic_mapping::template_facts(
                program,
                selected_functions
            )?)
            .expect("JSON values serialize")
        )
        .expect("string write");
    }
    graph.pop();
    let graph = format!(
        "{},\"base_schema\":{},\"generic_instance_ownership\":[{}]}}",
        graph,
        quote_json(base),
        facts
    );
    let graph = if hir::function_value::requires_function_values(program) {
        function_values::append_targets(graph, program)
    } else {
        graph
    };
    if hir::closure::requires_closure_projection(program) {
        let graph = function_values::append_closures(graph, program)?;
        if program
            .function_templates
            .iter()
            .any(hir::closure::template_has_closure)
        {
            function_values::append_template_closures(graph, program)
        } else {
            Ok(graph)
        }
    } else {
        Ok(graph)
    }
}

fn digest(domain: &str, parts: &[&str]) -> String {
    let mut hash = Sha256::new();
    hash.update(domain.as_bytes());
    hash.update([0]);
    for part in parts {
        hash.update((part.len() as u64).to_le_bytes());
        hash.update(part.as_bytes());
    }
    format!("sha256:{:x}", crate::digest_hex::LowerHex(hash.finalize()))
}

fn identity(revision: &str, template: &DeclarationId, args: &[ResolvedType]) -> String {
    let args = args
        .iter()
        .map(ResolvedType::identity_key)
        .collect::<Vec<_>>();
    let mut parts = vec![revision, template.as_str()];
    parts.extend(args.iter().map(String::as_str));
    digest("semaprax.generic-instance-identity.v1", &parts)
}

fn leaves(shape: &FieldLivenessShape, path: &mut Vec<String>, output: &mut Vec<Value>) {
    match shape {
        FieldLivenessShape::NoDrop => {}
        FieldLivenessShape::Leaf { flag, lifecycle } => output.push(json!({
            "field_path": path, "flag": flag.0, "lifecycle": lifecycle.as_str()
        })),
        FieldLivenessShape::Record { fields, .. } => {
            for field in fields {
                path.push(field.field.as_str().to_owned());
                leaves(&field.shape, path, output);
                path.pop();
            }
        }
        FieldLivenessShape::Variant { cases, .. } => {
            for case in cases {
                path.push(case.case.as_str().to_owned());
                for field in &case.fields {
                    path.push(field.field.as_str().to_owned());
                    leaves(&field.shape, path, output);
                    path.pop();
                }
                path.pop();
            }
        }
    }
}

fn signature(
    program: &ResolvedProgram,
    function: &ResolvedFunction,
    id: &ValueId,
    declared: &ResolvedType,
    substituted: &ResolvedType,
    ownership: OwnershipMode,
) -> Result<Value, Diagnostic> {
    let slot = function
        .cleanup
        .slots
        .iter()
        .find(|slot| match &slot.origin {
            CleanupStorageOrigin::Parameter { value, .. }
            | CleanupStorageOrigin::ProvisionalResult { value } => value == id,
            _ => false,
        });
    let mut owned = Vec::new();
    if let Some(slot) = slot {
        leaves(&slot.shape, &mut Vec::new(), &mut owned);
    }
    Ok(json!({
        "id": id.as_str(), "declared_type": declared.identity_key(),
        "substituted_type": substituted.identity_key(),
        "type_identity": substituted.identity_key(), "ownership_mode": ownership_text(ownership),
        "concrete_record_identity": match substituted { ResolvedType::Nominal { declaration, .. } if program.types.iter().any(|t| &t.id == declaration && matches!(t.kind, ResolvedTypeDeclarationKind::Record { .. })) => Some(substituted.identity_key()), _ => None },
        "cleanup_root": slot.map(|s| s.id.0), "owned_descendant_count": owned.len(),
        "owned_leaf_paths": owned,
    }))
}

pub(super) fn instance_json(
    program: &ResolvedProgram,
    revision: &str,
    instance: &hir::ResolvedFunctionInstance,
) -> Result<String, Diagnostic> {
    let template = program
        .function_templates
        .iter()
        .find(|t| t.id == instance.template)
        .ok_or_else(|| Diagnostic::io("SPX-G411", "generic instance has no checked template"))?;
    let function = &instance.function;
    let semantic_id = identity(revision, &instance.template, &instance.type_arguments);
    let schema = crate::cleanup_plan::selected_schema(program, function)?;
    if schema != function.cleanup_plan.schema {
        return Err(Diagnostic::io(
            "SPX-G411",
            "generic instance cleanup profile disagrees with selected plan",
        ));
    }
    let parameters = function
        .params
        .iter()
        .zip(&template.params)
        .map(|(p, t)| signature(program, function, &p.id, &t.ty, &p.ty, p.ownership))
        .collect::<Result<Vec<_>, _>>()?;
    let result = signature(
        program,
        function,
        &function.result_id,
        &template.return_type,
        &function.return_type,
        result_ownership(program, &function.return_type)?,
    )?;
    let inventory = function.cleanup.slots.iter().map(|slot| {
        let mut owned = Vec::new();
        leaves(&slot.shape, &mut Vec::new(), &mut owned);
        let origin = match &slot.origin {
            CleanupStorageOrigin::Parameter { value, parameter_index } => json!({"kind":"parameter","value":value.as_str(),"parameter_index":parameter_index}),
            CleanupStorageOrigin::Binding { value } => json!({"kind":"binding","value":value.as_str()}),
            CleanupStorageOrigin::Temporary { expression } => json!({"kind":"temporary","expression":expression.as_str()}),
            CleanupStorageOrigin::ProvisionalResult { value } => json!({"kind":"provisional_result","value":value.as_str()}),
        };
        let mut metadata = json!({"storage":slot.id.0,"discovery_index":slot.discovery_index,"origin":origin,
            "type_identity":slot.ty.identity_key(),"owned_leaf_paths":owned}).to_string();
        metadata.pop();
        format!("{},\"shape\":{}}}", metadata, crate::graph_cleanup::liveness_shape_json(&slot.shape))
    }).collect::<Vec<_>>().budgeted_join(",");
    let metadata = json!({"schema":function.cleanup.schema,
        "live_owned_parameters":function.cleanup.entry_state.live_owned_parameters.iter().map(|s|s.0).collect::<Vec<_>>(),
        "conditional_owned_parameters":function.cleanup.entry_state.conditional_owned_parameters.iter().map(|entry| json!({
            "storage":entry.storage.0,"variant":entry.variant.as_str(),
            "cases":entry.cases.iter().map(|case|json!({"case":case.case.as_str(),"live_flags":case.live_flags.iter().map(|f|f.0).collect::<Vec<_>>()})).collect::<Vec<_>>()
        })).collect::<Vec<_>>(),
        "flags":function.cleanup.flags.iter().map(|flag|json!({"id":flag.id.0,"storage":flag.place.storage.0,
            "field_path":flag.place.projections.iter().map(DeclarationId::as_str).collect::<Vec<_>>(),"lifecycle":flag.lifecycle.as_str()})).collect::<Vec<_>>()});
    let mut metadata = metadata.to_string();
    metadata.pop();
    let inventory = format!("{},\"slots\":[{}]}}", metadata, inventory);
    let plan = crate::graph_cleanup::cleanup_plan_json(&function.cleanup_plan);
    let mappings = if generic_mapping::requires_v35(&program.function_templates) {
        Some(generic_mapping::instance_mappings(
            program, template, instance,
        )?)
    } else {
        None
    };
    let mut calls = Vec::new();
    let mut mapping_error = None;
    for expression in function
        .requires
        .iter()
        .chain(std::iter::once(&function.body))
        .chain(&function.ensures)
    {
        visit_expr_call_instances(
            expression,
            &mut |expression, callee, args, callee_instance| {
                if mappings
                    .as_ref()
                    .is_some_and(|m| !m.contains_key(expression.id.as_str()))
                {
                    mapping_error = Some(Diagnostic::io(
                        "SPX-G411",
                        "missing structural forwarding association",
                    ));
                    return;
                }
                let mapping = mappings
                    .as_ref()
                    .map(|mappings| {
                        mappings
                            .get(expression.id.as_str())
                            .cloned()
                            .expect("validated structural call mapping")
                    })
                    .unwrap_or_else(|| {
                        template
                            .type_parameters
                            .iter()
                            .enumerate()
                            .map(|(index, _)| {
                                json!({
                                    "caller_owner":template.id.as_str(),"caller_index":index,
                                    "callee_owner":callee.as_str(),"callee_index":index,
                                })
                            })
                            .collect::<Vec<_>>()
                    });
                let target = program
                    .function_instances
                    .iter()
                    .find(|i| &i.id == callee_instance);
                calls.push(json!({"expression":expression.id.as_str(),"caller_instance":semantic_id,
                "callee_template":callee.as_str(),"forwarded_argument_mapping":mapping,
                "callee_concrete_arguments":args.iter().map(ResolvedType::identity_key).collect::<Vec<_>>(),
                "callee_instance":identity(revision,callee,args),"callee_execution_instance":callee_instance.as_str(),
                "transfer_kind":if target.is_some_and(|i|i.function.params.iter().any(|p|p.ownership == OwnershipMode::Own)) {"whole_owner"} else {"copy"},
            }));
            },
        );
    }
    if let Some(error) = mapping_error {
        return Err(error);
    }
    let facts = json!({
        "template":instance.template.as_str(),"concrete_instance":semantic_id,
        "execution_instance":instance.id.as_str(),
        "execution_id":FunctionExecutionId::Generic(instance.id.clone()).identity_key(),
        "type_arguments":instance.type_arguments.iter().enumerate().map(|(index,t)| json!({"owner":template.id.as_str(),"index":index,"type_identity":t.identity_key()})).collect::<Vec<_>>(),
        "parameters":parameters,"result":result,"call_edges":calls,
        "cleanup_plan_schema":schema,
        "cleanup_inventory_digest":digest("semaprax.generic-instance-inventory.v1", &[&inventory]),
        "cleanup_plan_digest":digest("semaprax.generic-instance-plan.v1", &[&plan]),
        "contracts":{"execution_id":FunctionExecutionId::Generic(instance.id.clone()).identity_key(),
            "requires":"requires_graph","ensures":"ensures_graph"},
        "effects":function.effects,"body_reference":FunctionExecutionId::Generic(instance.id.clone()).identity_key(),
        "source_revision":revision,
        "program_root_association":{"kind":"enclosing_semantic_program","source_revision":revision},
        "target_admission":{"profile":"checked_internal","public_generic_abi":false},
    });
    let mut text = facts.to_string();
    text.pop();
    Ok(format!(
        "{},\"cleanup_inventory\":{},\"cleanup_plan\":{},\"loan_relationships\":{}}}",
        text,
        inventory,
        plan,
        crate::graph_loan::loan_plan_json(&function.loan_plan)
    ))
}

pub(super) fn type_facts_json(
    program: &ResolvedProgram,
    selected_functions: &BTreeSet<DeclarationId>,
    selected_types: &BTreeSet<DeclarationId>,
    include_instances: bool,
) -> Result<String, Diagnostic> {
    let mut types = BTreeMap::new();
    for declaration in &program.types {
        if !selected_types.contains(&declaration.id) {
            continue;
        }
        if declaration.type_parameters.is_empty() {
            collect_type(
                &ResolvedType::Nominal {
                    declaration: declaration.id.clone(),
                    arguments: Vec::new(),
                },
                &mut types,
            );
        }
        if declaration.type_parameters.is_empty() {
            if let ResolvedTypeDeclarationKind::Record { fields }
            | ResolvedTypeDeclarationKind::Class { fields, .. } = &declaration.kind
            {
                for field in fields {
                    collect_type(&field.ty, &mut types);
                }
            }
        }
        if declaration.type_parameters.is_empty() {
            if let ResolvedTypeDeclarationKind::Variant { cases } = &declaration.kind {
                for case in cases {
                    for field in &case.fields {
                        collect_type(&field.ty, &mut types);
                    }
                }
            }
        }
    }
    for function in &program.functions {
        if !selected_functions.contains(&function.id) {
            continue;
        }
        for param in &function.params {
            collect_type(&param.ty, &mut types);
        }
        collect_type(&function.return_type, &mut types);
        for expression in &function.requires {
            collect_expr_types(expression, &mut types);
        }
        collect_expr_types(&function.body, &mut types);
        for expression in &function.ensures {
            collect_expr_types(expression, &mut types);
        }
    }
    if include_instances {
        for instance in &program.function_instances {
            if !selected_functions.contains(&instance.template) {
                continue;
            }
            let function = &instance.function;
            for parameter in &function.params {
                collect_type(&parameter.ty, &mut types);
            }
            collect_type(&function.return_type, &mut types);
            for expression in function
                .requires
                .iter()
                .chain(std::iter::once(&function.body))
                .chain(&function.ensures)
            {
                collect_expr_types(expression, &mut types);
            }
        }
    }
    types
        .values()
        .map(|ty| {
            Ok(format!(
                "{{\"id\":{},\"type\":{},\"facts\":{}}}",
                quote_json(&ty.identity_key()),
                type_json(ty),
                facts_json(program, ty)?
            ))
        })
        .collect::<Result<Vec<_>, Diagnostic>>()
        .map(|items| items.budgeted_join(","))
}
