use super::*;
use crate::cleanup_plan::replay::{expression_facts::Lookup, structural_maps};
#[test]
fn dynamic_expression_child_keeps_full_old_text_pointer_and_zero_payload_allocation() {
    let program = program(SCALAR);
    let function = program
        .functions
        .iter()
        .find(|f| f.id.as_str() == "family.main")
        .unwrap();
    let key = ExpressionId::from_owned("unmapped.λ\0.expression".to_owned());
    let mut before = ReplayBudget::with_physical(function, LIMIT).unwrap();
    let mut after = ReplayBudget::with_physical(function, LIMIT).unwrap();
    let facts =
        crate::cleanup_plan::replay::expression_facts::paid_arena(function, &mut before).unwrap();
    let expected = facts.require(function, &key, &mut before).unwrap_err();
    let (error, observed) = measure(|| {
        super::super::helpers::require_expression(&facts, function, &key, &mut after).unwrap_err()
    });
    assert_eq!((observed.allocations, observed.reallocations), (0, 0));
    assert!(!observed.invalid);
    let Error::Structural(receipt) = error else {
        panic!("typed semantic error")
    };
    let (source, Reason::UnknownExpression(held)) = receipt.borrowed_control_receipt().unwrap()
    else {
        panic!("dynamic expression")
    };
    assert_eq!(source.as_str().as_ptr(), function.body.id.as_str().as_ptr());
    assert_eq!(held.as_str().as_ptr(), key.as_str().as_ptr());
    assert_eq!(receipt.legacy(function), expected);
}
#[test]
fn direct_block_edge_status_and_call_children_keep_immediate_old_six_fields() {
    for case in 0..5 {
        let mut program = program(SCALAR);
        let function = program
            .functions
            .iter_mut()
            .find(|f| f.id.as_str() == "family.bytes")
            .unwrap();
        match case {
            0 => {}
            1 => {
                let transfer = function
                    .cleanup_plan
                    .blocks
                    .iter_mut()
                    .flat_map(|b| &mut b.transitions)
                    .find_map(|t| match t {
                        CleanupTransition::CallCommit { arguments, .. } => arguments.first_mut(),
                        _ => None,
                    })
                    .unwrap();
                transfer.parameter_index = u32::MAX;
            }
            2 => {
                let transfer = function
                    .cleanup_plan
                    .blocks
                    .iter_mut()
                    .flat_map(|b| &mut b.transitions)
                    .find_map(|t| match t {
                        CleanupTransition::CallCommit { arguments, .. } => arguments.first_mut(),
                        _ => None,
                    })
                    .unwrap();
                transfer.source.storage = StorageId::ProvisionalResult;
            }
            3 => {
                let mut status = function
                    .cleanup_plan
                    .status_sources
                    .first()
                    .unwrap()
                    .id
                    .clone();
                status.expression = ExpressionId::from_owned("absent.λ\0.status".to_owned());
                function.cleanup_plan.edges[0].condition = EdgeCondition::StatusZero(status);
            }
            4 => function.cleanup_plan.edges[0].id = EdgeId(u32::MAX),
            _ => unreachable!(),
        }
        let source = primitive_fixture::Context::new(&program);
        let function = program
            .functions
            .iter()
            .find(|f| f.id.as_str() == "family.bytes")
            .unwrap();
        let mut results = Vec::new();
        for old in [true, false] {
            let mut budget = ReplayBudget::with_physical(function, LIMIT).unwrap();
            let context =
                selected_context::Context::new_paid(&source, function, &mut budget, false).unwrap();
            let storage = structural_maps::paid::slots(function, &mut budget).unwrap();
            let leaves = structural_maps::paid::leaves(function, &mut budget).unwrap();
            let facts =
                crate::cleanup_plan::replay::expression_facts::paid_arena(function, &mut budget)
                    .unwrap();
            let renewals = context.primitive_renewals(function).unwrap();
            let result = if old {
                super::super::super::validate_blocks_and_edges_paid::<Error>(
                    &context,
                    function,
                    &facts,
                    &storage,
                    &leaves,
                    renewals,
                    &mut budget,
                )
            } else {
                validate_blocks_and_edges_paid(
                    &context,
                    function,
                    &facts,
                    &storage,
                    &leaves,
                    renewals,
                    &mut budget,
                )
            };
            if !old && case == 3 {
                let Err(Error::Structural(receipt)) = &result else {
                    panic!("actual missing status must remain typed: {result:?}")
                };
                let (_, Reason::UnknownStatus(held)) = receipt.borrowed_control_receipt().unwrap()
                else {
                    panic!("missing status retains its actual operand")
                };
                let EdgeCondition::StatusZero(source) = &function.cleanup_plan.edges[0].condition
                else {
                    unreachable!()
                };
                assert_eq!(held.as_str().as_ptr(), source.expression.as_str().as_ptr());
            }
            results.push(result.map_err(|e| render(e, function)));
        }
        assert_eq!(results[0], results[1], "full block/call/status case {case}");
        assert_eq!(
            results[1].is_ok(),
            case == 0,
            "case {case}: {:?}",
            results[1]
        );
        same_owner(&program, "family.bytes", case == 0);
    }
}
#[test]
fn direct_cfg_success_unknown_cycle_and_unreachable_keep_old_exact_diagnostics() {
    for case in 0..5 {
        let mut program = program(LEXICAL);
        let function = program
            .functions
            .iter_mut()
            .find(|f| f.id.as_str() == "probe")
            .unwrap();
        let entry = function.cleanup_plan.entry;
        match case {
            0 => {}
            1 => {
                function.cleanup_plan.blocks[entry.0 as usize].terminator =
                    CleanupTerminator::Goto(EdgeId(u32::MAX))
            }
            2 => function.cleanup_plan.entry = BlockId(u32::MAX),
            3 => {
                let edge = EdgeId(function.cleanup_plan.edges.len().try_into().unwrap());
                function
                    .cleanup_plan
                    .edges
                    .push(crate::cleanup_plan::CleanupEdge {
                        id: edge,
                        from: entry,
                        to: entry,
                        condition: EdgeCondition::Always,
                    });
                function.cleanup_plan.blocks[entry.0 as usize].terminator =
                    CleanupTerminator::Goto(edge);
            }
            4 => {
                let mut block = function.cleanup_plan.blocks[entry.0 as usize].clone();
                block.id = BlockId(function.cleanup_plan.blocks.len().try_into().unwrap());
                function.cleanup_plan.blocks.push(block);
            }
            _ => unreachable!(),
        }
        let mut before = ReplayBudget::with_physical(function, LIMIT).unwrap();
        let mut after = ReplayBudget::with_physical(function, LIMIT).unwrap();
        let old = super::super::super::validate_reachable_acyclic_cfg(function, &mut before);
        let new =
            validate_reachable_acyclic_cfg(function, &mut after).map_err(|e| render(e, function));
        assert_eq!(new, old, "complete CFG case {case}");
        assert_eq!(new.is_ok(), case == 0, "{new:?}");
    }
}
#[test]
fn copy_result_target_child_keeps_body_identity_before_type_diagnostic() {
    let program = program(SCALAR);
    let function = program
        .functions
        .iter()
        .find(|f| f.id.as_str() == "family.main")
        .unwrap();
    for case in 0..3 {
        let source = StagedCopyResultSource::Body {
            expression: if case == 1 {
                ExpressionId::from_owned("unknown.body".to_owned())
            } else {
                function.body.id.clone()
            },
            instance: if case == 0 {
                function.return_type.clone()
            } else {
                ResolvedType::Bool
            },
        };
        let old =
            crate::cleanup_plan::replay::transition::validate_staged_target(function, &source);
        let mut budget = ReplayBudget::with_physical(function, LIMIT).unwrap();
        let new = super::super::helpers::validate_staged_target(function, &source, &mut budget)
            .map_err(|e| render(e, function));
        assert_eq!(new, old, "staged target {case}");
        assert_eq!(new.is_ok(), case == 0);
    }
}

#[test]
fn selected_place_and_finalizer_children_keep_complete_old_ordered_diagnostics() {
    let program = program(SCALAR);
    let function = program
        .functions
        .iter()
        .find(|f| f.id.as_str() == "family.bytes")
        .unwrap();
    let mut setup = ReplayBudget::with_physical(function, LIMIT).unwrap();
    let storage = structural_maps::paid::slots(function, &mut setup).unwrap();
    let leaves = structural_maps::paid::leaves(function, &mut setup).unwrap();
    let original = leaves.values().next().unwrap().place.clone();
    for case in 0..3 {
        let mut place = original.clone();
        if case == 1 {
            place.storage = StorageId::ProvisionalResult;
        }
        if case == 2 {
            place.projections.push(function.id.clone());
        }
        let mut before = ReplayBudget::with_physical(function, LIMIT).unwrap();
        let mut after = ReplayBudget::with_physical(function, LIMIT).unwrap();
        let old = crate::cleanup_plan::replay::path_join::storage::validate_place(
            function,
            &place,
            &storage,
            &leaves,
            &mut before,
        );
        let new =
            super::super::helpers::validate_place(function, &place, &storage, &leaves, &mut after)
                .map_err(|e| render(e, function));
        assert_eq!(new, old, "place child {case}");
        assert_eq!(new.is_ok(), case == 0);
    }
    let original = function
        .cleanup_plan
        .exits
        .iter()
        .flat_map(|e| &e.finalize_in_order)
        .next()
        .unwrap();
    for case in 0..5 {
        let mut actions = vec![original.clone()];
        match case {
            0 => {}
            1 => actions[0].lifecycle_id = function.id.clone(),
            2 => actions[0].guard_flag = LivenessFlagId(u32::MAX),
            3 => actions[0].source.projections.push(function.id.clone()),
            4 => actions.push(original.clone()),
            _ => unreachable!(),
        }
        let mut before = ReplayBudget::with_physical(function, LIMIT).unwrap();
        let mut after = ReplayBudget::with_physical(function, LIMIT).unwrap();
        let old = crate::cleanup_plan::replay::lexical_cert::storage::finalizer_actions(
            function,
            &actions,
            &storage,
            &leaves,
            &mut before,
        );
        let new = super::super::helpers::finalizer_actions(
            function, &actions, &storage, &leaves, &mut after,
        )
        .map_err(|e| render(e, function));
        assert_eq!(new, old, "finalizer child {case}");
        assert_eq!(new.is_ok(), case == 0);
    }
}
