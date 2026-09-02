//! Cleanup region proofs: update regions, parent-local prefixes, typed
//! roots, and pattern binding lookup.

use super::*;

#[test]
fn cleanup_source_exit_events_upper_bounds_lowerer_families() {
    let source = r#"
module capacity.cleanup_exit_events;
@id("exit.resource") resource R { @id("exit.resource.drop") drop trivial; }
@id("exit.box") record Box { @id("exit.box.value") value: R, }
@id("exit.choice") variant Choice {
    @id("exit.choice.first") First,
    @id("exit.choice.second") Second,
}
@id("exit.helper") fn helper(value: i64) -> i64 { value }
@id("exit.call") fn call_case(value: i64) -> i64 { helper(value) }
@id("exit.neg") fn neg_case(value: i64) -> i64 { -value }
@id("exit.add") fn add_case(value: i64) -> i64 { value + 1 }
@id("exit.lazy") fn lazy_case(condition: bool) -> bool { condition && true }
@id("exit.if") fn if_case(condition: bool) -> i64 { if condition { 1 } else { 2 } }
@id("exit.match") fn match_case(value: Choice) -> i64 {
    match value {
        Choice::First {} => 0,
        Choice::Second {} => 1,
    }
}
@id("exit.update") fn update_case(base: own Box, replacement: own R) -> Box {
    base with { value: replacement }
}
@id("app.main") fn main() -> i64 { 0 }
"#;
    let program = crate::parse(source, Path::new("cleanup-exit-events.spx")).unwrap();
    let resolved = hir::resolve(&program).unwrap();
    let expected_tail_events = [
        ("helper", 0usize),
        ("call_case", 1),
        ("neg_case", 1),
        ("add_case", 1),
        ("lazy_case", 0),
        ("if_case", 0),
        ("match_case", 0),
        ("update_case", 1),
        ("main", 0),
    ];
    let mut traversal = [None; MAX_SEMANTIC_EXPRESSION_DEPTH + 1];
    for (name, expected_tail) in expected_tail_events {
        let function = program
            .functions
            .iter()
            .find(|function| function.name == name)
            .unwrap();
        let crate::ast::ExprKind::Block { tail, .. } = &function.body.kind else {
            panic!("function body must retain its authored block");
        };
        assert_eq!(cleanup_source_exit_events(tail), expected_tail, "{name}");
        let source_events = cleanup_function_exit_events(function, &mut traversal).unwrap();
        let actual_exits = resolved
            .functions
            .iter()
            .find(|candidate| candidate.name == name)
            .unwrap()
            .cleanup_plan
            .exits
            .len();
        assert_eq!(source_events, actual_exits, "{name}");
    }
}

#[test]
fn cleanup_retained_census_covers_update_region_with_live_long_id_roots() {
    let long = "x".repeat(128);
    let parameters = (0..MAX_PARAMETERS)
        .map(|index| format!("live{index}: own R"))
        .collect::<Vec<_>>()
        .join(", ");
    let source = format!(
        "module capacity.cleanup_update;\n@id(\"resource.{long}\") resource R {{ @id(\"lifecycle.{long}\") drop trivial; }}\n@id(\"box.{long}\") record Box {{ @id(\"box.value.{long}\") value: R, }}\n@id(\"update.stress\") fn stress(base: own Box, replacement: own R, {parameters}) -> Box {{ base with {{ value: replacement }} }}\n@id(\"app.main\") fn main() -> i64 {{ 0 }}\n"
    );
    let program = crate::parse(&source, Path::new("cleanup-update-live.spx")).unwrap();
    let canonical = crate::format::canonical(&program);
    let mut scan = [None; MAX_SEMANTIC_EXPRESSION_DEPTH + 1];
    let capacity = hir_pre_resolve_capacity(&program, canonical.len(), &mut scan).unwrap();
    assert!(capacity.complete().unwrap() <= MAX_BUILDER_BYTES);
    let resolved = hir::resolve(&program).unwrap();
    let stress = resolved
        .functions
        .iter()
        .find(|function| function.name == "stress")
        .unwrap();
    assert!(
        stress
            .cleanup_plan
            .exits
            .iter()
            .map(|exit| exit.finalize_in_order.len())
            .max()
            .unwrap_or(0)
            >= MAX_PARAMETERS
    );
    let actual = resolved
        .functions
        .iter()
        .try_fold(0usize, |bytes, function| {
            bytes
                .checked_add(
                    crate::private_capacity_contract::cleanup_inventory_owned_capacity(
                        &function.cleanup,
                    )?,
                )?
                .checked_add(
                    crate::private_capacity_contract::cleanup_plan_owned_capacity(
                        &function.cleanup_plan,
                    )?,
                )
        })
        .unwrap();
    let actual_exits = resolved
        .functions
        .iter()
        .try_fold(0usize, |count, function| {
            count.checked_add(function.cleanup_plan.exits.len())
        })
        .unwrap();
    assert!(actual_exits <= capacity.cleanup_exit_events_upper);
    assert!(
        actual <= capacity.cleanup_authority_upper,
        "update actual {actual} exceeds authority {} (retained {}, structural {}, call epoch {})",
        capacity.cleanup_authority_upper,
        capacity.cleanup_retained_upper,
        capacity.cleanup_authority_upper - capacity.cleanup_retained_upper,
        capacity.cleanup_call_argument_owned_upper
    );
    assert_eq!(capacity.cleanup_fallback_roots, 0);
}

#[test]
fn cleanup_update_staged_base_survives_replacement_failure() {
    let long = "x".repeat(128);
    let source = format!(
        "module capacity.cleanup_update_failure;\n@id(\"resource.{long}\") resource R {{ @id(\"lifecycle.{long}\") drop trivial; }}\n@id(\"box.{long}\") record Box {{ @id(\"box.value.{long}\") value: R, }}\n@id(\"update.failure\") fn stress(base: own Box, replacement: own R, checked: i64) -> Box {{ base with {{ value: {{ let observed = checked + 1; replacement }} }} }}\n@id(\"app.main\") fn main() -> i64 {{ 0 }}\n"
    );
    let program = crate::parse(&source, Path::new("cleanup-update-failure.spx")).unwrap();
    let canonical = crate::format::canonical(&program);
    let mut scan = [None; MAX_SEMANTIC_EXPRESSION_DEPTH + 1];
    let capacity = hir_pre_resolve_capacity(&program, canonical.len(), &mut scan).unwrap();
    let resolved = hir::resolve(&program).unwrap();
    let stress = resolved
        .functions
        .iter()
        .find(|function| function.name == "stress")
        .unwrap();
    assert!(stress.cleanup_plan.exits.iter().any(|exit| {
        matches!(
            exit.continuation,
            semaprax::cleanup_plan::ExitContinuation::ReturnFailure { .. }
        ) && exit.finalize_in_order.iter().any(|action| {
            matches!(
                &action.source.storage,
                semaprax::cleanup_plan::StorageId::Temporary(expression)
                    if expression.as_str().contains(".base")
            ) && action.lifecycle_id.as_str() == format!("lifecycle.{long}")
                && action
                    .source
                    .projections
                    .iter()
                    .any(|projection| projection.as_str() == format!("box.value.{long}"))
        })
    }));
    let actual = resolved
        .functions
        .iter()
        .try_fold(0usize, |bytes, function| {
            bytes
                .checked_add(
                    crate::private_capacity_contract::cleanup_inventory_owned_capacity(
                        &function.cleanup,
                    )?,
                )?
                .checked_add(
                    crate::private_capacity_contract::cleanup_plan_owned_capacity(
                        &function.cleanup_plan,
                    )?,
                )
        })
        .unwrap();
    assert!(
        actual <= capacity.cleanup_authority_upper,
        "update staged-base cleanup {actual} exceeds authority {}",
        capacity.cleanup_authority_upper
    );
    assert!(capacity.complete().unwrap() <= MAX_BUILDER_BYTES);
}

#[test]
fn cleanup_parent_local_update_prefix_survives_later_replacement_failure() {
    let long = "x".repeat(128);
    let left_field_id = format!("pair.left.{long}");
    let lifecycle_id = format!("resource.drop.{long}");
    let source = format!(
        "module capacity.cleanup_update_prefix;\n@id(\"resource.{long}\") resource R {{ @id(\"{lifecycle_id}\") drop trivial; }}\n@id(\"pair.{long}\") record Pair {{ @id(\"{left_field_id}\") left: R, @id(\"pair.right.{long}\") right: R, }}\n@id(\"update.prefix.stress.{long}\") fn stress(base: own Pair, new_left: own R, new_right: own R, checked: i64) -> Pair {{ base with {{ left: new_left, right: {{ let observed = checked + 1; new_right }}, }} }}\n@id(\"app.main\") fn main() -> i64 {{ 0 }}\n"
    );
    let program = crate::parse(&source, Path::new("cleanup-update-prefix.spx")).unwrap();
    let canonical = crate::format::canonical(&program);
    let mut scan = [None; MAX_SEMANTIC_EXPRESSION_DEPTH + 1];
    let capacity = hir_pre_resolve_capacity(&program, canonical.len(), &mut scan).unwrap();
    let stats = capacity.cleanup_proof.stats;
    assert_eq!(stats.parent_local_update_prefix_fields, 1);
    assert_eq!(stats.parent_local_update_prefix_exit_groups, 1);
    assert_eq!(stats.parent_local_update_prefix_finalizer_copies, 1);
    assert_eq!(
        stats.parent_local_update_prefix_finalizer_projection_segments,
        1
    );
    assert_eq!(
        stats.parent_local_update_prefix_finalizer_lifecycle_ids,
        lifecycle_id.len()
    );
    assert_eq!(
        stats.parent_local_update_prefix_finalizer_projection_ids,
        left_field_id.len()
    );

    let resolved = hir::resolve(&program).unwrap();
    let stress = resolved
        .functions
        .iter()
        .find(|function| function.name == "stress")
        .unwrap();
    let is_left = |action: &semaprax::cleanup_plan::FinalizeAction| {
        action.lifecycle_id.as_str() == lifecycle_id
            && action
                .source
                .projections
                .iter()
                .any(|projection| projection.as_str() == left_field_id)
    };
    let is_destination = |action: &semaprax::cleanup_plan::FinalizeAction| {
        is_left(action)
            && matches!(
                &action.source.storage,
                semaprax::cleanup_plan::StorageId::Temporary(expression)
                    if !expression.as_str().ends_with(".base")
            )
    };
    let is_staged_base = |action: &semaprax::cleanup_plan::FinalizeAction| {
        is_left(action)
            && matches!(
                &action.source.storage,
                semaprax::cleanup_plan::StorageId::Temporary(expression)
                    if expression.as_str().ends_with(".base")
            )
    };
    let failure = stress
        .cleanup_plan
        .exits
        .iter()
        .find(|exit| {
            matches!(
                exit.continuation,
                semaprax::cleanup_plan::ExitContinuation::ReturnFailure { .. }
            ) && exit.finalize_in_order.iter().any(&is_destination)
                && exit.finalize_in_order.iter().any(&is_staged_base)
        })
        .expect("later replacement failure retains new destination and staged old base");
    let destination_actions = failure
        .finalize_in_order
        .iter()
        .filter(|action| is_destination(action))
        .collect::<Vec<_>>();
    assert_eq!(destination_actions.len(), 1);
    let observed_named = destination_actions
        .iter()
        .try_fold(0usize, |bytes, action| {
            let storage_bytes = match &action.source.storage {
                semaprax::cleanup_plan::StorageId::Temporary(expression) => {
                    expression.as_str().len()
                }
                _ => 0,
            };
            bytes
                .checked_add(std::mem::size_of::<semaprax::cleanup_plan::FinalizeAction>())?
                .checked_add(storage_bytes)?
                .checked_add(
                    action
                        .source
                        .projections
                        .capacity()
                        .checked_mul(std::mem::size_of::<DeclarationId>())?,
                )?
                .checked_add(
                    action
                        .source
                        .projections
                        .iter()
                        .try_fold(0usize, |bytes, projection| {
                            bytes.checked_add(projection.as_str().len())
                        })?,
                )?
                .checked_add(action.lifecycle_id.as_str().len())
        })
        .unwrap();
    assert!(
        observed_named <= capacity.cleanup_parent_local_update_prefix_lifetime_upper,
        "update-prefix actual {observed_named} exceeds named authority {}",
        capacity.cleanup_parent_local_update_prefix_lifetime_upper
    );
    let actual = resolved
        .functions
        .iter()
        .try_fold(0usize, |bytes, function| {
            bytes
                .checked_add(
                    crate::private_capacity_contract::cleanup_inventory_owned_capacity(
                        &function.cleanup,
                    )?,
                )?
                .checked_add(
                    crate::private_capacity_contract::cleanup_plan_owned_capacity(
                        &function.cleanup_plan,
                    )?,
                )
        })
        .unwrap();
    assert!(actual <= capacity.cleanup_authority_upper);
    assert!(capacity.complete().unwrap() <= MAX_BUILDER_BYTES);
}

#[test]
fn cleanup_parent_local_record_prefix_survives_later_field_failure() {
    let long = "x".repeat(128);
    let first_field_id = format!("pair.first.{long}");
    let lifecycle_id = format!("resource.drop.{long}");
    let source = format!(
        "module capacity.cleanup_record_prefix;\n@id(\"resource.{long}\") resource R {{ @id(\"{lifecycle_id}\") drop trivial; }}\n@id(\"pair.{long}\") record Pair {{ @id(\"{first_field_id}\") first: R, @id(\"pair.second.{long}\") second: R, }}\n@id(\"record.prefix.stress.{long}\") fn stress(first: own R, second: own R, checked: i64) -> Pair {{ Pair {{ first: first, second: {{ let observed = checked + 1; second }}, }} }}\n@id(\"app.main\") fn main() -> i64 {{ 0 }}\n"
    );
    let program = crate::parse(&source, Path::new("cleanup-record-prefix.spx")).unwrap();
    let canonical = crate::format::canonical(&program);
    let mut scan = [None; MAX_SEMANTIC_EXPRESSION_DEPTH + 1];
    let capacity = hir_pre_resolve_capacity(&program, canonical.len(), &mut scan).unwrap();
    let stats = capacity.cleanup_proof.stats;
    assert_eq!(stats.parent_local_partial_fields, 1);
    assert_eq!(stats.parent_local_finalizer_copies, 1);
    assert_eq!(stats.parent_local_finalizer_projection_segments, 1);
    assert_eq!(
        stats.parent_local_finalizer_lifecycle_ids,
        lifecycle_id.len()
    );
    assert_eq!(
        stats.parent_local_finalizer_projection_ids,
        first_field_id.len()
    );
    assert!(capacity.cleanup_parent_local_lifetime_upper > 0);

    let resolved = hir::resolve(&program).unwrap();
    let stress = resolved
        .functions
        .iter()
        .find(|function| function.name == "stress")
        .unwrap();
    assert!(stress.cleanup_plan.exits.iter().any(|exit| {
        matches!(
            exit.continuation,
            semaprax::cleanup_plan::ExitContinuation::ReturnFailure { .. }
        ) && exit.finalize_in_order.iter().any(|action| {
            matches!(
                action.source.storage,
                semaprax::cleanup_plan::StorageId::Temporary(_)
            ) && action.lifecycle_id.as_str() == lifecycle_id
                && action
                    .source
                    .projections
                    .iter()
                    .any(|projection| projection.as_str() == first_field_id)
        })
    }));
    let actual = resolved
        .functions
        .iter()
        .try_fold(0usize, |bytes, function| {
            bytes
                .checked_add(
                    crate::private_capacity_contract::cleanup_inventory_owned_capacity(
                        &function.cleanup,
                    )?,
                )?
                .checked_add(
                    crate::private_capacity_contract::cleanup_plan_owned_capacity(
                        &function.cleanup_plan,
                    )?,
                )
        })
        .unwrap();
    assert!(actual <= capacity.cleanup_authority_upper);
    assert!(capacity.complete().unwrap() <= MAX_BUILDER_BYTES);
}

#[test]
fn cleanup_parent_local_projection_residual_survives_failure_and_success() {
    let long = "x".repeat(128);
    let right_field_id = format!("pair.right.{long}");
    let lifecycle_id = format!("resource.drop.{long}");
    let source = format!(
        "module capacity.cleanup_projection_residual;\n@id(\"resource.{long}\") resource R {{ @id(\"{lifecycle_id}\") drop trivial; }}\n@id(\"pair.{long}\") record Pair {{ @id(\"pair.left.{long}\") left: R, @id(\"{right_field_id}\") right: R, }}\n@id(\"projection.residual.stress.{long}\") fn stress(left: own R, right: own R, checked: i64) -> R {{ let selected = Pair {{ left: left, right: right, }}.left; let observed = checked + 1; selected }}\n@id(\"app.main\") fn main() -> i64 {{ 0 }}\n"
    );
    let program = crate::parse(&source, Path::new("cleanup-projection-residual.spx")).unwrap();
    let canonical = crate::format::canonical(&program);
    let mut scan = [None; MAX_SEMANTIC_EXPRESSION_DEPTH + 1];
    let capacity = hir_pre_resolve_capacity(&program, canonical.len(), &mut scan).unwrap();
    let stats = capacity.cleanup_proof.stats;
    assert_eq!(stats.parent_local_projection_epochs, 1);
    assert_eq!(stats.parent_local_projection_exit_groups, 2);
    assert_eq!(stats.parent_local_projection_finalizer_copies, 2);
    assert_eq!(
        stats.parent_local_projection_finalizer_projection_segments,
        2
    );
    assert_eq!(
        stats.parent_local_projection_finalizer_lifecycle_ids,
        lifecycle_id.len() * 2
    );
    assert_eq!(
        stats.parent_local_projection_finalizer_projection_ids,
        right_field_id.len() * 2
    );

    let resolved = hir::resolve(&program).unwrap();
    let stress = resolved
        .functions
        .iter()
        .find(|function| function.name == "stress")
        .unwrap();
    let is_residual = |action: &semaprax::cleanup_plan::FinalizeAction| {
        matches!(
            action.source.storage,
            semaprax::cleanup_plan::StorageId::Temporary(_)
        ) && action.lifecycle_id.as_str() == lifecycle_id
            && action
                .source
                .projections
                .iter()
                .any(|projection| projection.as_str() == right_field_id)
    };
    assert!(stress.cleanup_plan.exits.iter().any(|exit| {
        matches!(
            exit.continuation,
            semaprax::cleanup_plan::ExitContinuation::ReturnFailure { .. }
        ) && exit.finalize_in_order.iter().any(&is_residual)
    }));
    assert!(stress.cleanup_plan.exits.iter().any(|exit| {
        matches!(
            exit.continuation,
            semaprax::cleanup_plan::ExitContinuation::CommitResult { .. }
        ) && exit.finalize_in_order.iter().any(&is_residual)
    }));
    let residual_actions = stress
        .cleanup_plan
        .exits
        .iter()
        .flat_map(|exit| &exit.finalize_in_order)
        .filter(|action| is_residual(action))
        .collect::<Vec<_>>();
    assert_eq!(residual_actions.len(), 2);
    let observed_named = residual_actions
        .iter()
        .try_fold(0usize, |bytes, action| {
            let storage_bytes = match &action.source.storage {
                semaprax::cleanup_plan::StorageId::Temporary(expression) => {
                    expression.as_str().len()
                }
                _ => 0,
            };
            bytes
                .checked_add(std::mem::size_of::<semaprax::cleanup_plan::FinalizeAction>())?
                .checked_add(storage_bytes)?
                .checked_add(
                    action
                        .source
                        .projections
                        .capacity()
                        .checked_mul(std::mem::size_of::<DeclarationId>())?,
                )?
                .checked_add(
                    action
                        .source
                        .projections
                        .iter()
                        .try_fold(0usize, |bytes, projection| {
                            bytes.checked_add(projection.as_str().len())
                        })?,
                )?
                .checked_add(action.lifecycle_id.as_str().len())
        })
        .unwrap();
    assert!(
        observed_named <= capacity.cleanup_parent_local_projection_lifetime_upper,
        "projection-residual actual {observed_named} exceeds named authority {}",
        capacity.cleanup_parent_local_projection_lifetime_upper
    );
    let actual = resolved
        .functions
        .iter()
        .try_fold(0usize, |bytes, function| {
            bytes
                .checked_add(
                    crate::private_capacity_contract::cleanup_inventory_owned_capacity(
                        &function.cleanup,
                    )?,
                )?
                .checked_add(
                    crate::private_capacity_contract::cleanup_plan_owned_capacity(
                        &function.cleanup_plan,
                    )?,
                )
        })
        .unwrap();
    assert!(actual <= capacity.cleanup_authority_upper);
    assert!(capacity.complete().unwrap() <= MAX_BUILDER_BYTES);
}

#[test]
fn cleanup_typed_roots_resolve_nested_and_later_arm_bindings() {
    let source = r#"
module capacity.cleanup_lexical_types;
@id("lexical.resource") resource R { @id("lexical.resource.drop") drop trivial; }
@id("lexical.choice") variant Choice {
    @id("lexical.choice.first") First { @id("lexical.choice.first.value") value: i64, },
    @id("lexical.choice.second") Second { @id("lexical.choice.second.value") value: i64, },
}
@id("lexical.identity") fn identity(value: own R) -> R { value }
@id("lexical.consume") fn consume(value: own R) -> i64 { 1 }
@id("lexical.stress") fn stress(value: own R, choice: Choice) -> i64 {
    let outer = identity(value);
    let nested = {
        let inner = identity(outer);
        consume(inner)
    };
    nested + match choice {
        Choice::First { value: first } => first,
        Choice::Second { value: second } => second,
    }
}
@id("app.main") fn main() -> i64 { 0 }
"#;
    let program = crate::parse(source, Path::new("cleanup-lexical-types.spx")).unwrap();
    let canonical = crate::format::canonical(&program);
    let mut scan = [None; MAX_SEMANTIC_EXPRESSION_DEPTH + 1];
    let capacity = hir_pre_resolve_capacity(&program, canonical.len(), &mut scan).unwrap();
    assert_eq!(capacity.cleanup_fallback_roots, 0);
    let resolved = hir::resolve(&program).unwrap();
    let actual = resolved
        .functions
        .iter()
        .try_fold(0usize, |bytes, function| {
            bytes
                .checked_add(
                    crate::private_capacity_contract::cleanup_inventory_owned_capacity(
                        &function.cleanup,
                    )?,
                )?
                .checked_add(
                    crate::private_capacity_contract::cleanup_plan_owned_capacity(
                        &function.cleanup_plan,
                    )?,
                )
        })
        .unwrap();
    assert!(
        actual <= capacity.cleanup_authority_upper,
        "lexical actual cleanup {actual} exceeds authority {} (retained {}, structural {})",
        capacity.cleanup_authority_upper,
        capacity.cleanup_retained_upper,
        capacity.cleanup_authority_upper - capacity.cleanup_retained_upper
    );
}

#[test]
fn cleanup_typed_roots_treat_generic_and_prelude_copy_types_as_no_drop() {
    let source = r#"
module capacity.cleanup_copy_types;
@id("copy.resource") resource R { @id("copy.resource.drop") drop trivial; }
@id("copy.generic") fn generic<T>(value: T) -> T { value }
@id("copy.option") fn option(value: i64) -> Option<i64> {
    Option<i64>::Some { value: value }
}
@id("copy.result") fn make_result(value: i64) -> Result<i64, bool> {
    Result<i64, bool>::Ok { value: value }
}
@id("copy.outer") fn outer(value: own R) -> R { { value } }
@id("app.main") fn main() -> i64 {
    let first = generic<i64>(1);
    let second = option(first);
    let third = make_result(first);
    first
}
"#;
    let program = crate::parse(source, Path::new("cleanup-copy-types.spx")).unwrap();
    let canonical = crate::format::canonical(&program);
    let mut scan = [None; MAX_SEMANTIC_EXPRESSION_DEPTH + 1];
    let capacity = hir_pre_resolve_capacity(&program, canonical.len(), &mut scan).unwrap();
    assert_eq!(capacity.cleanup_fallback_roots, 0);
    hir::resolve(&program).unwrap();

    // Same-name shadowing is rejected by the language, but the
    // pre-resolution census must still resolve the outer initializer and
    // remain conservative without falling back to an unrelated resource.
    let shadow = crate::parse(
        r#"
module capacity.cleanup_shadow;
@id("shadow.resource") resource R { @id("shadow.resource.drop") drop trivial; }
@id("shadow.invalid") fn invalid(value: own R) -> R {
    let outer = value;
    { let outer = outer; outer }
}
@id("app.main") fn main() -> i64 { 0 }
"#,
        Path::new("cleanup-shadow.spx"),
    )
    .unwrap();
    let shadow_canonical = crate::format::canonical(&shadow);
    let shadow_capacity =
        hir_pre_resolve_capacity(&shadow, shadow_canonical.len(), &mut scan).unwrap();
    assert_eq!(shadow_capacity.cleanup_fallback_roots, 0);
    let diagnostics = hir::resolve(&shadow).unwrap_err();
    assert!(diagnostics
        .iter()
        .any(|diagnostic| diagnostic.code == "SPX-T209"));
}

#[test]
fn cleanup_pattern_binding_lookup_is_iterative_at_exact_depth() {
    use crate::ast::{
        Expr, ExprKind, FieldDeclaration, MatchArm, MatchPattern, Param, ParamMode,
        RecordMatchFieldPattern, RecordMatchPatternField, Type, TypeDeclaration,
        TypeDeclarationKind,
    };

    fn program_with_pattern_depth(depth: usize) -> Program {
        let span = crate::ast::Span::default();
        let mut program = crate::parse(
                "module cleanup.pattern.depth; @id(\"app.inspect\") fn inspect(scrutinee: R0) -> i64 { 0 } @id(\"app.main\") fn main() -> i64 { 0 }",
                Path::new("cleanup-pattern-depth.spx"),
            )
            .unwrap();
        let mut pattern = RecordMatchFieldPattern::Binding {
            name: "value".into(),
            span,
        };
        for index in (1..depth).rev() {
            pattern = RecordMatchFieldPattern::Record {
                type_name: format!("R{index}"),
                type_span: span,
                fields: vec![RecordMatchPatternField {
                    name: "next".into(),
                    name_span: span,
                    pattern,
                    span,
                }],
                span,
            };
        }
        program.types = (0..depth)
            .map(|index| TypeDeclaration {
                stable_id: format!("cleanup.pattern.r{index}"),
                explicit_id: true,
                name: format!("R{index}"),
                name_span: span,
                type_parameters: Vec::new(),
                kind: TypeDeclarationKind::Record {
                    fields: vec![FieldDeclaration {
                        stable_id: format!("cleanup.pattern.r{index}.next"),
                        explicit_id: true,
                        name: "next".into(),
                        name_span: span,
                        ty: if index + 1 == depth {
                            Type::I64
                        } else {
                            Type::Named {
                                name: format!("R{}", index + 1),
                                arguments: Vec::new(),
                            }
                        },
                        span,
                    }],
                },
                extends: None,
                span,
            })
            .collect();
        program.functions[0].params = vec![Param {
            name: "scrutinee".into(),
            mode: ParamMode::Value,
            ty: Type::Named {
                name: "R0".into(),
                arguments: Vec::new(),
            },
            span,
        }];
        program.functions[0].body = Expr {
            kind: ExprKind::Match {
                mode: crate::ast::MatchMode::Value,
                scrutinee: Box::new(Expr {
                    kind: ExprKind::Var("scrutinee".into()),
                    span,
                }),
                arms: vec![MatchArm {
                    guard: None,
                    pattern: MatchPattern::Record {
                        type_name: "R0".into(),
                        type_span: span,
                        fields: vec![RecordMatchPatternField {
                            name: "next".into(),
                            name_span: span,
                            pattern,
                            span,
                        }],
                        span,
                    },
                    value: Expr {
                        kind: ExprKind::Var("value".into()),
                        span,
                    },
                    span,
                }],
            },
            span,
        };
        program
    }

    const CHILD_ENV: &str = "SEMAPRAX_TEST_CLEANUP_PATTERN_DEPTH";
    if let Some(depth) = std::env::var_os(CHILD_ENV) {
        let depth = depth.to_string_lossy().parse::<usize>().unwrap();
        let program = program_with_pattern_depth(depth);
        let canonical = crate::format::canonical(&program);
        HIR_RESOLVE_PASS_COUNT.with(|count| count.set(0));
        POST_HIR_FACTS_ENTRY_COUNT.with(|count| count.set(0));
        RESOLVED_DISPOSE_COMPLETIONS.with(|count| count.set(0));
        if depth == MAX_SEMANTIC_EXPRESSION_DEPTH {
            let mut stack = [None; MAX_SEMANTIC_EXPRESSION_DEPTH + 1];
            let capacity = hir_pre_resolve_capacity(&program, canonical.len(), &mut stack)
                .expect("depth-512 pattern capacity");
            assert_eq!(capacity.cleanup_fallback_roots, 0);
            note_hir_resolve_pass();
            let resolved = hir::resolve(&program).unwrap();
            let frames = Vec::with_capacity(capacity.disposal_frames);
            assert_eq!(frames.capacity(), capacity.disposal_frames);
            drop(ResolvedProgramOwner::new(
                resolved,
                frames,
                capacity.disposal_frames,
            ));
            assert_eq!(HIR_RESOLVE_PASS_COUNT.with(std::cell::Cell::get), 1);
            assert_eq!(RESOLVED_DISPOSE_COMPLETIONS.with(std::cell::Cell::get), 1);
        } else {
            let mut stack = [None; MAX_SEMANTIC_EXPRESSION_DEPTH + 1];
            let diagnostic = match hir_pre_resolve_capacity(&program, canonical.len(), &mut stack) {
                Err(diagnostic) => diagnostic,
                Ok(_) => panic!("depth-513 nested record pattern was admitted"),
            };
            assert_eq!(diagnostic.code, "SPX-B109");
            assert_eq!(HIR_RESOLVE_PASS_COUNT.with(std::cell::Cell::get), 0);
            assert_eq!(POST_HIR_FACTS_ENTRY_COUNT.with(std::cell::Cell::get), 0);
            assert_eq!(RESOLVED_DISPOSE_COMPLETIONS.with(std::cell::Cell::get), 0);
        }
        std::mem::forget(program);
        std::process::exit(0);
    }

    for depth in [
        MAX_SEMANTIC_EXPRESSION_DEPTH,
        MAX_SEMANTIC_EXPRESSION_DEPTH + 1,
    ] {
        let output = Command::new(std::env::current_exe().unwrap())
            .arg(
                "implementation::tests::cleanup_regions::cleanup_pattern_binding_lookup_is_iterative_at_exact_depth",
            )
            .arg("--exact")
            .arg("--nocapture")
            .env(CHILD_ENV, depth.to_string())
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "pattern depth {depth}: {}",
            String::from_utf8_lossy(&output.stderr)
        );
    }
}

#[test]
fn cleanup_call_argument_epoch_covers_later_argument_failure() {
    let long = "x".repeat(128);
    let source = format!(
        "module capacity.cleanup_call_epoch;\n@id(\"resource.{long}\") resource R {{ @id(\"lifecycle.{long}\") drop trivial; }}\n@id(\"identity\") fn identity(value: own R) -> R {{ value }}\n@id(\"consume\") fn consume(value: own R) -> i64 {{ 1 }}\n@id(\"combine\") fn combine(first: own R, second: own R) -> i64 {{ let left = consume(first); let right = consume(second); left + right }}\n@id(\"stress\") fn stress(first: own R, second: own R, checked: i64) -> i64 {{ combine(identity(first), {{ let observed = checked + 1; let staged = identity(second); staged }}) }}\n@id(\"app.main\") fn main() -> i64 {{ 0 }}\n"
    );
    let program = crate::parse(&source, Path::new("cleanup-call-epoch.spx")).unwrap();
    let canonical = crate::format::canonical(&program);
    let mut scan = [None; MAX_SEMANTIC_EXPRESSION_DEPTH + 1];
    let capacity = hir_pre_resolve_capacity(&program, canonical.len(), &mut scan).unwrap();
    assert_eq!(capacity.cleanup_fallback_roots, 0);
    let resolved = hir::resolve(&program).unwrap();
    let stress = resolved
        .functions
        .iter()
        .find(|function| function.name == "stress")
        .unwrap();
    assert!(stress.cleanup_plan.exits.iter().any(|exit| {
        exit.finalize_in_order.iter().any(|action| {
            matches!(
                action.source.storage,
                semaprax::cleanup_plan::StorageId::CallArgument { .. }
            )
        })
    }));
    let actual_inventory = resolved
        .functions
        .iter()
        .try_fold(0usize, |bytes, function| {
            bytes.checked_add(
                crate::private_capacity_contract::cleanup_inventory_owned_capacity(
                    &function.cleanup,
                )?,
            )
        })
        .unwrap();
    let actual_plan = resolved
        .functions
        .iter()
        .try_fold(0usize, |bytes, function| {
            bytes.checked_add(
                crate::private_capacity_contract::cleanup_plan_owned_capacity(
                    &function.cleanup_plan,
                )?,
            )
        })
        .unwrap();
    let actual = actual_inventory.checked_add(actual_plan).unwrap();
    assert!(
        actual <= capacity.cleanup_authority_upper,
        "call-epoch inventory {actual_inventory} + plan {actual_plan} = {actual} exceeds authority {} (retained {}, structural {}, call epoch {})",
        capacity.cleanup_authority_upper,
        capacity.cleanup_retained_upper,
        capacity.cleanup_authority_upper - capacity.cleanup_retained_upper,
        capacity.cleanup_call_argument_owned_upper
    );
    assert!(capacity.complete().unwrap() <= MAX_BUILDER_BYTES);
}

#[test]
fn inventory_and_cleanup_hostile_envelopes_bind_the_shared_fixture() {
    let source = include_str!("../../../../../tests/fixtures/native_rust_hir_capacity.spx");
    let program = crate::parse(source, Path::new("native-rust-hir-capacity.spx")).unwrap();
    let canonical = crate::format::canonical(&program);
    assert_eq!(
        raw_digest(canonical.as_bytes()),
        "sha256:2a012464bb1bdb624a79972d558fe837f6d55a9cd9f40d2ead16bfbba615f316"
    );
    let mut stack = [None; MAX_SEMANTIC_EXPRESSION_DEPTH + 1];
    let capacity = hir_pre_resolve_capacity(&program, canonical.len(), &mut stack).unwrap();
    let peaks = capacity.phase_peaks();
    assert_eq!(
        [
            capacity.retained_upper,
            peaks[3],
            capacity.retained_upper.checked_add(peaks[3]).unwrap(),
            peaks[4],
            capacity.retained_upper.checked_add(peaks[4]).unwrap(),
        ],
        [2_928_343, 38_760, 2_967_103, 299_312, 3_227_655],
        "retained/inventory/cleanup envelope terms drifted"
    );
    let complete = capacity.complete().unwrap();
    HIR_RESOLVE_PASS_COUNT.with(|count| count.set(0));
    let (result, overflowed, consumed) =
        crate::bounded_output::with_limit_usage(complete - 1, || {
            let _budget = reserve_temporary_exact(complete)?;
            note_hir_resolve_pass();
            Ok::<_, Diagnostic>(())
        });
    assert_eq!(result.unwrap_err().code, "SPX-B109");
    assert!(!overflowed);
    assert_eq!(consumed, 0);
    HIR_RESOLVE_PASS_COUNT.with(|count| assert_eq!(count.get(), 0));
}
