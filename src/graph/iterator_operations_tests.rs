//! Two-parameter iterator helpers retain ordered scoped identities in every projection.
const SOURCE: &str = include_str!("../../examples/iterator-operations.spx");

#[test]
fn iterator_operations_graph_binds_ordered_two_parameter_instances() {
    let checked = crate::check(SOURCE, "iterator-operations.spx").unwrap();
    let program = crate::hir::resolve(&checked).unwrap();
    let map = program
        .function_instances
        .iter()
        .find(|instance| instance.template.as_str() == "iterator.map")
        .unwrap();
    assert_eq!(
        map.type_arguments,
        vec![
            crate::hir::ResolvedType::I64,
            crate::hir::ResolvedType::Bool
        ]
    );
    let fold = program
        .function_instances
        .iter()
        .find(|instance| instance.template.as_str() == "iterator.fold")
        .unwrap();
    assert_eq!(
        fold.type_arguments,
        vec![
            crate::hir::ResolvedType::Bool,
            crate::hir::ResolvedType::Usize
        ]
    );
    assert_eq!(
        map.function.cleanup_plan.schema,
        crate::cleanup_plan::CLEANUP_PLAN_SCHEMA_V11
    );
    assert_eq!(
        fold.function.cleanup_plan.schema,
        crate::cleanup_plan::CLEANUP_PLAN_SCHEMA_V11
    );
    assert_eq!(super::graph_schema(&program).unwrap(), "semaprax.graph.v40");
    let graph = crate::graph::to_json(&checked).unwrap();
    crate::graph::verify_json(&checked, &graph).unwrap();

    let mut reordered = program.clone();
    let instance = reordered
        .function_instances
        .iter_mut()
        .find(|instance| instance.template.as_str() == "iterator.map")
        .unwrap();
    instance.type_arguments.swap(0, 1);
    assert!(crate::hir::validate(&reordered).is_err());

    let mut forged_scope = program.clone();
    let template = forged_scope
        .function_templates
        .iter_mut()
        .find(|template| template.id.as_str() == "iterator.map")
        .unwrap();
    let crate::hir::ResolvedType::Function { result, .. } = &mut template.params[2].ty else {
        panic!("map retains its scalar callback")
    };
    **result = crate::hir::ResolvedType::TypeParameter {
        owner: template.id.clone(),
        index: 2,
    };
    assert!(crate::hir::validate(&forged_scope).is_err());

    let changed = SOURCE.replace("value > 0", "value > 1");
    let changed = crate::check(&changed, "iterator-operations.spx").unwrap();
    assert!(crate::graph::verify_json(&changed, &graph).is_err());
}

#[test]
fn iterator_operations_renewal_proof_cannot_be_omitted_or_downgraded() {
    use crate::cleanup_plan::CleanupTransition;
    let checked = crate::check(SOURCE, "iterator-renewal.spx").unwrap();
    let program = crate::hir::resolve(&checked).unwrap();
    let filter_index = program
        .function_instances
        .iter()
        .position(|instance| instance.template.as_str() == "iterator.filter")
        .unwrap();
    let plan = &program.function_instances[filter_index]
        .function
        .cleanup_plan;
    assert_eq!(plan.schema, crate::cleanup_plan::CLEANUP_PLAN_SCHEMA_V12);
    assert!(plan
        .blocks
        .iter()
        .flat_map(|block| &block.transitions)
        .any(|transition| matches!(transition, CleanupTransition::ReserveRenewal { .. })));
    assert!(plan
        .blocks
        .iter()
        .flat_map(|block| &block.transitions)
        .any(|transition| matches!(transition, CleanupTransition::Renew { .. })));

    let mut omitted = program.clone();
    for block in &mut omitted.function_instances[filter_index]
        .function
        .cleanup_plan
        .blocks
    {
        block
            .transitions
            .retain(|transition| !matches!(transition, CleanupTransition::ReserveRenewal { .. }));
    }
    assert!(crate::hir::validate(&omitted).is_err());

    let mut replaced = program.clone();
    for transition in replaced.function_instances[filter_index]
        .function
        .cleanup_plan
        .blocks
        .iter_mut()
        .flat_map(|block| &mut block.transitions)
    {
        if let CleanupTransition::Renew {
            at,
            source,
            destination,
        } = transition
        {
            *transition = CleanupTransition::Transfer {
                at: at.clone(),
                source: source.clone(),
                destination: destination.clone(),
            };
        }
    }
    assert!(crate::hir::validate(&replaced).is_err());

    let mut downgraded = program.clone();
    downgraded.function_instances[filter_index]
        .function
        .cleanup_plan
        .schema = crate::cleanup_plan::CLEANUP_PLAN_SCHEMA_V11;
    assert!(crate::hir::validate(&downgraded).is_err());
    assert!(super::graph_schema(&downgraded).is_err());

    let graph = crate::graph::to_json(&checked).unwrap();
    assert!(crate::graph::verify_json(
        &checked,
        &graph.replacen("semaprax.graph.v40", "semaprax.graph.v39", 1)
    )
    .is_err());
}
