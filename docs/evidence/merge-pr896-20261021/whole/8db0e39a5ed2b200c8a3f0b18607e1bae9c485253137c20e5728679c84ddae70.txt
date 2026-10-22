use super::*;
#[test]
fn complete_scalar_and_lexical_success_keep_ordinary_and_immediate_old_owners() {
    for (source, ids) in [
        (LEXICAL, &["probe", "main"][..]),
        (SCALAR, &["family.one", "family.main", "family.bytes"][..]),
    ] {
        let program = program(source);
        crate::cleanup_plan::replay::validate_program(&program).unwrap();
        for id in ids {
            same_owner(&program, id, true);
        }
    }
}
#[test]
fn actual_held_unknown_entry_keeps_complete_old_diagnostic_and_prior_profile_order() {
    for (source, id) in [(LEXICAL, "probe"), (SCALAR, "family.main")] {
        let mut program = program(source);
        let index = program
            .functions
            .iter()
            .position(|f| f.id.as_str() == id)
            .unwrap();
        program.functions[index].cleanup_plan.entry = BlockId(u32::MAX);
        same_owner(&program, id, false);
        let source = primitive_fixture::Context::new(&program);
        let function = &program.functions[index];
        let selected = owner(&source, function, false).unwrap_err();
        assert!(
            selected.message.contains("cleanup entry block is unknown"),
            "{selected:?}"
        );
        assert_eq!(
            crate::cleanup_plan::replay::validate_program(&program).unwrap_err(),
            selected
        );
    }
    let mut program = program(SCALAR);
    let function = program
        .functions
        .iter_mut()
        .find(|f| f.id.as_str() == "family.main")
        .unwrap();
    function.cleanup_plan.entry = BlockId(u32::MAX);
    function.cleanup_plan.schema = "hostile.preceding.schema";
    same_owner(&program, "family.main", false);
}
#[test]
fn complete_lexical_hostile_structure_keeps_all_six_fields_and_full_drop() {
    for case in 0..10 {
        let mut program = program(LEXICAL);
        let function = program
            .functions
            .iter_mut()
            .find(|f| f.id.as_str() == "probe")
            .unwrap();
        let plan = &mut function.cleanup_plan;
        match case {
            0 => plan.edges[0].id = EdgeId(u32::MAX),
            1 => plan.edges[0].from = BlockId(u32::MAX),
            2 => plan.edges[0].to = BlockId(u32::MAX),
            3 => plan.blocks[0].id = BlockId(u32::MAX),
            4 => plan.blocks[0].region = CleanupRegionId(u32::MAX),
            5 => plan.blocks[0].terminator = CleanupTerminator::Goto(EdgeId(u32::MAX)),
            6 => plan.exits[0].from = BlockId(u32::MAX),
            7 => plan.exits[0].leaves_regions.push(CleanupRegionId(0)),
            8 => {
                let edges = plan
                    .blocks
                    .iter()
                    .find_map(|block| match &block.terminator {
                        CleanupTerminator::Branch(edges) => Some(edges.clone()),
                        _ => None,
                    })
                    .unwrap();
                plan.edges[edges[0].0 as usize].condition = EdgeCondition::Always;
                plan.edges[edges[1].0 as usize].condition = EdgeCondition::Always;
            }
            9 => {
                plan.edges[0].condition = EdgeCondition::BooleanResult(
                    ExpressionId::from_owned("unknown.λ\0identity".to_owned()),
                    true,
                )
            }
            _ => unreachable!(),
        }
        same_owner(&program, "probe", false);
    }
}

#[test]
fn whole_scalar_finalizer_hostiles_keep_first_six_fields_and_complete_drop() {
    for case in 0..4 {
        let mut program = program(SCALAR);
        let function = program
            .functions
            .iter_mut()
            .find(|f| f.id.as_str() == "family.bytes")
            .unwrap();
        let exit = function
            .cleanup_plan
            .exits
            .iter_mut()
            .find(|e| !e.finalize_in_order.is_empty())
            .unwrap();
        match case {
            0 => exit.finalize_in_order[0].lifecycle_id = function.id.clone(),
            1 => exit.finalize_in_order[0].guard_flag = LivenessFlagId(u32::MAX),
            2 => exit.finalize_in_order[0]
                .source
                .projections
                .push(function.id.clone()),
            3 => exit
                .finalize_in_order
                .push(exit.finalize_in_order[0].clone()),
            _ => unreachable!(),
        }
        same_owner(&program, "family.bytes", false);
    }
}
#[test]
fn held_duplicate_initialize_is_closed_by_selected_path_receipt() {
    let mut program = program(SCALAR);
    let function = program
        .functions
        .iter_mut()
        .find(|f| f.id.as_str() == "family.bytes")
        .unwrap();
    let block = function
        .cleanup_plan
        .blocks
        .iter_mut()
        .find(|b| {
            b.transitions
                .iter()
                .any(|t| matches!(t, CleanupTransition::Initialize { .. }))
        })
        .unwrap();
    let index = block
        .transitions
        .iter()
        .position(|t| matches!(t, CleanupTransition::Initialize { .. }))
        .unwrap();
    block
        .transitions
        .insert(index + 1, block.transitions[index].clone());
    // Structural checks independently authenticate both copies. State replay
    // must still reject the second live initialization on its existing route.
    same_owner(&program, "family.bytes", false);
    let source = primitive_fixture::Context::new(&program);
    let function = program
        .functions
        .iter()
        .find(|f| f.id.as_str() == "family.bytes")
        .unwrap();
    let error = owner(&source, function, false).unwrap_err();
    assert!(
        error
            .message
            .contains("initialize transition initializes an empty or live cleanup place"),
        "{error:?}"
    );
}
