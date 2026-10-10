//! Existing owned payloads retain the vector cell's exact cleanup history.
use super::*;
use crate::cleanup_plan::CleanupTransition;

const SOURCE: &str = r#"module owned_loop;
@id("row") record Row { @id("row.text") text:string, @id("row.n") n:i64, }
@id("main") fn main()->i64 {
 let mut words=vec_with_capacity<string>(2usize);
 let mut rows=vec_with_capacity<Row>(2usize);
 let mut i=0;
 while i<2 {
  if i==0 { words=vec_push<string>(words,"first"); 0 } else { 0 }
  rows=vec_push<Row>(rows,Row{text:string_concat("row", "!"),n:i});
  i=i+1; 0
 }
 if vec_len<string>(words)==1usize && vec_len<Row>(rows)==2usize {7}else{0}
}
"#;
fn checked() -> ResolvedProgram {
    let parsed = crate::check(SOURCE, "owned-loop.spx").unwrap();
    let resolved = crate::hir::resolve(&parsed).unwrap();
    crate::hir::validate(&resolved).unwrap();
    resolved
}
#[test]
fn owned_payload_renewal_replays_exact_history_and_rejects_missing_proof() {
    let program = checked();
    let function = &program.functions[0];
    assert_eq!(bindings_in(&program, function).len(), 2);
    assert!(requires_with_admission(function, |op, element| {
        element_mode(&program.declarations, op, element)
    }));
    assert!(!requires_with_admission(function, |_, _| None));
    assert!(
        !requires(function),
        "declaration-free classifier cannot admit owned payloads"
    );
    let reservations = function
        .cleanup_plan
        .blocks
        .iter()
        .flat_map(|b| &b.transitions)
        .filter(|t| matches!(t, CleanupTransition::ReserveRenewal { .. }))
        .count();
    assert_eq!(reservations, 2);
    for mode in 0..3 {
        let mut forged = program.clone();
        for block in &mut forged.functions[0].cleanup_plan.blocks {
            if mode == 0 {
                block
                    .transitions
                    .retain(|t| !matches!(t, CleanupTransition::ReserveRenewal { .. }));
            }
            for transition in &mut block.transitions {
                if let CleanupTransition::Renew {
                    at,
                    source,
                    destination,
                } = transition
                {
                    if mode == 1 {
                        *transition = CleanupTransition::Transfer {
                            at: at.clone(),
                            source: source.clone(),
                            destination: destination.clone(),
                        };
                    } else if mode == 2 {
                        *destination = source.clone();
                    }
                }
            }
        }
        assert!(
            crate::cleanup_plan::validate_program(&forged).is_err(),
            "forgery {mode}"
        );
    }
    let mut drift = program.clone();
    forge_record_field_for_test(
        &mut drift.declarations,
        &DeclarationId::new("row"),
        ResolvedType::I64,
    );
    assert_eq!(bindings_in(&drift, function).len(), 1);
    assert!(crate::hir::validate(&drift).is_err());
}
#[test]
fn owned_payload_renewal_requires_exact_element_mode_and_whole_source() {
    let program = checked();
    let function = &program.functions[0];
    let sites = bindings_in(&program, function);
    let at = sites
        .keys()
        .find(|at| sites[*at].ty == crate::vec_ops::resolved_vec(ResolvedType::String))
        .unwrap();
    let mut pending = vec![&function.body];
    let expression = loop {
        let expression = pending.pop().unwrap();
        if &expression.id == at {
            break expression;
        }
        push_resolved_expression_children_in_authored_order(expression, &mut pending);
    };
    let binding = sites[at];
    for mode in 0..3 {
        let mut forged = expression.clone();
        let ResolvedExprKind::Call { args, .. } = &mut forged.kind else {
            panic!("call")
        };
        match mode {
            0 => args[1].ownership = OwnershipMode::Value,
            1 => args[0].ownership = OwnershipMode::Borrow,
            2 => {
                let ResolvedExprKind::Place(place) = &mut args[0].kind else {
                    panic!("place")
                };
                place.root = function.result_id.clone();
            }
            _ => unreachable!(),
        }
        assert!(!same_cell(
            binding,
            &forged,
            Some(&|op, element| element_mode(&program.declarations, op, element))
        ));
    }
}
