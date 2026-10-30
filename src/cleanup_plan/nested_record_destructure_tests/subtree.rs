//! Independent plan and graph receiver for a hidden owned descendant.
use super::*;

#[test]
fn whole_subtree_uses_one_exact_prefix_transfer_and_refuses_plan_forgeries() {
    let source = SOURCE
        .replace(
            "left: Branch { leaf: Leaf { payload: left, marker: left_marker }, enabled: _ },",
            "left,",
        )
        .replace(
            "left_marker + right_marker + sequence",
            "right_marker + sequence",
        );
    let ast = crate::check(&source, Path::new("cleanup-subtree.spx")).unwrap();
    let canonical = crate::format::canonical(&ast);
    assert_eq!(
        crate::format::canonical(
            &crate::check(&canonical, Path::new("cleanup-subtree.spx")).unwrap()
        ),
        canonical
    );
    let program = hir::resolve(&ast).unwrap();
    hir::validate(&program).unwrap();
    let own = function(&program, "envelope.consume");
    assert_eq!(own.cleanup_plan.schema, CLEANUP_PLAN_SCHEMA_V8);
    assert_eq!(
        crate::graph::graph_schema(&program).unwrap(),
        "semaprax.graph.v28"
    );
    validate_structure(&program, own).unwrap();
    let graph = crate::graph::to_json(&ast).unwrap();
    let graph_again =
        crate::graph::to_json(&crate::check(&canonical, Path::new("cleanup-subtree.spx")).unwrap())
            .unwrap();
    assert_eq!(
        graph, graph_again,
        "canonical source and agent projection agree"
    );
    let transfers = own
        .cleanup_plan
        .blocks
        .iter()
        .flat_map(|b| &b.transitions)
        .filter_map(|t| match t {
            CleanupTransition::Transfer {
                source,
                destination,
                ..
            } if source
                .projections
                .first()
                .is_some_and(|f| f.as_str() == "envelope.left") =>
            {
                Some((source, destination))
            }
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(
        transfers.len(),
        1,
        "one subtree transition, not unpack/rebuild"
    );
    let (source, destination) = transfers[0];
    assert_eq!(
        source
            .projections
            .iter()
            .map(|f| f.as_str())
            .collect::<Vec<_>>(),
        ["envelope.left"]
    );
    assert!(destination.projections.is_empty());
    let slot = own
        .cleanup_plan
        .slots
        .iter()
        .find(|s| s.storage == destination.storage)
        .unwrap();
    assert!(
        matches!(&slot.ty, hir::ResolvedType::Nominal { declaration, arguments }
        if declaration.as_str() == "branch.type" && arguments.is_empty())
    );
    let crate::cleanup::FieldLivenessShape::Record { fields, .. } = &slot.field_liveness_shape
    else {
        panic!("whole child slot");
    };
    assert_eq!(fields[0].field.as_str(), "branch.leaf");
    let crate::cleanup::FieldLivenessShape::Record { fields, .. } = &fields[0].shape else {
        panic!("retained grandchild");
    };
    assert_eq!(fields[0].field.as_str(), "leaf.payload");
    assert!(matches!(
        fields[0].shape,
        crate::cleanup::FieldLivenessShape::Leaf { .. }
    ));
    for kind in ["path", "destination", "missing", "duplicate", "schema"] {
        let mut changed = own.clone();
        if kind == "schema" {
            changed.cleanup_plan.schema = CLEANUP_PLAN_SCHEMA_V7;
        } else {
            let block = changed
                .cleanup_plan
                .blocks
                .iter_mut()
                .find(|b| {
                    b.transitions.iter().any(|t| {
                        matches!(t, CleanupTransition::Transfer { source, .. }
                    if source.projections.first().is_some_and(|f| f.as_str() == "envelope.left"))
                    })
                })
                .unwrap();
            let position = block
                .transitions
                .iter()
                .position(|t| {
                    matches!(t, CleanupTransition::Transfer { source, .. }
                    if source.projections.first().is_some_and(|f| f.as_str() == "envelope.left"))
                })
                .unwrap();
            if kind == "missing" {
                block.transitions.remove(position);
            } else if kind == "duplicate" {
                block
                    .transitions
                    .insert(position, block.transitions[position].clone());
            } else if let CleanupTransition::Transfer {
                source,
                destination,
                ..
            } = &mut block.transitions[position]
            {
                if kind == "path" {
                    source.projections[0] = hir::DeclarationId::new("envelope.right");
                } else {
                    destination.storage = source.storage.clone();
                }
            }
        }
        let error = validate_structure(&program, &changed).expect_err(kind);
        assert_eq!(error.code, "SPX-H006", "{kind}: {error:?}");
    }
}

/// The funded production resolution route: one identity owner on the active
/// ledger funds source verification, identities, the base prelude and HIR.
/// Public `hir::resolve` verifies with `SourceFunding::ordinary()`, which is
/// unfunded by design, so it cannot carry a reserve-before-allocate bound.
fn resolve_funded(ast: &crate::ast::Program) -> Result<hir::ResolvedProgram, ()> {
    let mut owner =
        crate::bounded_output::identity_reservation::IdentityReservation::acquire().map_err(drop)?;
    let mut invocation = crate::hir::prelude_bundle::Invocation::new();
    let resolved = crate::hir::identity_resolution::resolve_source_checked_previous_with_owner(
        ast,
        None,
        None,
        None,
        None,
        &mut invocation,
        &mut owner,
    );
    drop(invocation);
    drop(owner);
    resolved.map(|(program, _)| program).map_err(drop)
}

#[test]
fn subtree_resolution_and_independent_replay_use_exact_sticky_builder_budget_through_drop() {
    let source = SOURCE
        .replace(
            "left: Branch { leaf: Leaf { payload: left, marker: left_marker }, enabled: _ },",
            "left,",
        )
        .replace(
            "left_marker + right_marker + sequence",
            "right_marker + sequence",
        );
    // Authored parsing precedes this constructor scope. The observation covers
    // funded resolution (source verification through resolved HIR), independent
    // cleanup replay and complete resolved Drop; it excludes the held input AST
    // and fixed test tracking metadata.
    let ast = crate::check(&source, Path::new("subtree-budget.spx")).unwrap();
    let run = || {
        let program = resolve_funded(&ast)?;
        hir::validate(&program).map_err(drop)?;
        validate_structure(&program, function(&program, "envelope.consume")).map_err(drop)?;
        drop(program);
        Ok::<(), ()>(())
    };
    let (wide, exhausted, required) =
        crate::bounded_output::with_limit_usage(crate::workspace_graph::MAX_BUILDER_BYTES, run);
    wide.unwrap();
    assert!(!exhausted && required > 0);
    // Fixed Copy instrumentation records the existing whole observation; it
    // excludes no stage and grants no allocation credit or semantic authority.
    // Since g171 cleanup replay inside resolution and HIR validation debits a
    // detached per-program scratch ledger (cleanup_plan::validate::
    // scratch_ledger) instead of the builder ledger. Those receipts are read
    // from the detached ledgers themselves and added to the builder debit.
    let mut stages = [crate::test_allocation_observer::Metrics::default(); 5];
    let mut debits = [0usize; 5];
    let detached_start = crate::bounded_output::detached::detached_debits();
    let ((exact, observed), exhausted, spent) =
        crate::bounded_output::with_limit_usage(required, || {
            crate::test_allocation_observer::measure(|| {
                let mut snapshot = |index| {
                    stages[index] = crate::test_allocation_observer::current_metrics();
                    debits[index] = crate::bounded_output::active_limit().unwrap()
                        - crate::bounded_output::active_remaining().unwrap()
                        + (crate::bounded_output::detached::detached_debits() - detached_start);
                };
                snapshot(0);
                let program = resolve_funded(&ast)?;
                snapshot(1);
                hir::validate(&program).map_err(drop)?;
                snapshot(2);
                validate_structure(&program, function(&program, "envelope.consume"))
                    .map_err(drop)?;
                snapshot(3);
                drop(program);
                snapshot(4);
                Ok::<(), ()>(())
            })
        });
    let detached = crate::bounded_output::detached::detached_debits() - detached_start;
    exact.unwrap();
    assert!(!exhausted);
    assert_eq!(spent, required);
    assert_eq!(debits[4], required + detached);
    assert!(!observed.invalid && observed.live == 0);
    let debited = required + detached;
    assert!(
        observed.allocated <= debited && observed.highwater <= debited,
        "whole observation {observed:?}, builder {required}, detached {detached}, stages initial/resolve/HIR/replay/Drop {stages:?}, cumulative debits {debits:?}"
    );
    // Required is the measured implementation debit, not an analytical total.
    // The separate prefix/shape owner above supplies independent literal facts.
    let ((first, retry), exhausted, _) =
        crate::bounded_output::with_limit_usage(required - 1, || {
            let first = run();
            let retry = run();
            (first, retry)
        });
    assert!(exhausted && first.is_err() && retry.is_err());
}
